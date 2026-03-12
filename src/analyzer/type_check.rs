//! Type checker for Fajar Lang.
//!
//! Verifies type correctness, context annotations, and tensor shape compatibility.
//! Walks the AST and produces `SemanticError`s for any inconsistencies.

use std::collections::HashMap;

use crate::lexer::token::Span;
use crate::parser::ast::{
    AssignOp, BinOp, CallArg, ConstDef, Expr, ExternFn, FStringExprPart, FnDef, ImplBlock, Item,
    LiteralKind, MatchArm, ModDecl, Pattern, Program, Stmt, TypeAlias, TypeExpr, UnaryOp, UseDecl,
    UseKind,
};

use super::scope::{Symbol, SymbolTable};

// ═══════════════════════════════════════════════════════════════════════
// Suggestion engine (string similarity)
// ═══════════════════════════════════════════════════════════════════════

/// Computes the Levenshtein edit distance between two strings.
fn levenshtein_distance(a: &str, b: &str) -> usize {
    let a_len = a.len();
    let b_len = b.len();
    if a_len == 0 {
        return b_len;
    }
    if b_len == 0 {
        return a_len;
    }

    let mut prev_row: Vec<usize> = (0..=b_len).collect();
    let mut curr_row = vec![0; b_len + 1];

    for (i, a_ch) in a.chars().enumerate() {
        curr_row[0] = i + 1;
        for (j, b_ch) in b.chars().enumerate() {
            let cost = if a_ch == b_ch { 0 } else { 1 };
            curr_row[j + 1] = (prev_row[j + 1] + 1)
                .min(curr_row[j] + 1)
                .min(prev_row[j] + cost);
        }
        std::mem::swap(&mut prev_row, &mut curr_row);
    }

    prev_row[b_len]
}

/// Finds the closest match to `name` from `candidates` within an edit distance threshold.
///
/// Returns `Some("did you mean 'X'?")` if a close match is found, `None` otherwise.
/// The threshold is min(3, name.len() / 2) to avoid spurious suggestions for short names.
fn suggest_similar(name: &str, candidates: &[String]) -> Option<String> {
    let threshold = 3.min(name.len() / 2 + 1);
    let mut best: Option<(usize, &str)> = None;

    for candidate in candidates {
        // Skip exact matches and very short names
        if candidate == name || candidate.starts_with('_') {
            continue;
        }
        let dist = levenshtein_distance(name, candidate);
        if dist <= threshold && (best.is_none() || dist < best.as_ref().map_or(usize::MAX, |b| b.0))
        {
            best = Some((dist, candidate));
        }
    }

    best.map(|(_, suggestion)| format!("did you mean '{suggestion}'?"))
}

/// Generates a hint for a type mismatch between expected and found types.
fn type_mismatch_hint(expected: &str, found: &str) -> Option<String> {
    match (expected, found) {
        ("i32", "f64") | ("i64", "f64") | ("i32", "f32") | ("i64", "f32") => {
            Some(format!("use `{found} as {expected}` to convert"))
        }
        ("f64", "i32") | ("f64", "i64") | ("f32", "i32") | ("f32", "i64") => {
            Some(format!("use `{found} as {expected}` to convert"))
        }
        ("str", _) => Some(format!("use `to_string({found})` to convert")),
        (_, "str") => Some("use `parse_int()` or `parse_float()` to convert".to_string()),
        ("bool", _) => Some(format!("use a comparison like `{found} != 0`")),
        _ => None,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Type representation
// ═══════════════════════════════════════════════════════════════════════

/// Internal type representation for the Fajar Lang type system.
///
/// Every expression is assigned a `Type` during analysis. Types are
/// structural — two types are equal if they have the same structure.
/// Integer and float types are distinct: `i32 ≠ i64`, `f32 ≠ f64`.
#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    /// The null/void type (no value).
    Void,
    /// The never type (diverges, never returns).
    Never,
    /// Signed integers.
    I8,
    I16,
    I32,
    I64,
    I128,
    /// Unsigned integers.
    U8,
    U16,
    U32,
    U64,
    U128,
    /// Platform-sized integers.
    ISize,
    USize,
    /// Floating point.
    F16,
    Bf16,
    F32,
    F64,
    /// Unsuffixed integer literal — compatible with any integer type.
    IntLiteral,
    /// Unsuffixed float literal — compatible with any float type.
    FloatLiteral,
    /// Boolean.
    Bool,
    /// Character.
    Char,
    /// String.
    Str,
    /// Fixed-size array: `[T; N]`.
    Array(Box<Type>),
    /// Tuple: `(T1, T2, ...)`.
    Tuple(Vec<Type>),
    /// A named struct type.
    Struct {
        /// Struct name.
        name: String,
        /// Field name → type mapping.
        fields: HashMap<String, Type>,
    },
    /// A named enum type.
    Enum {
        /// Enum name.
        name: String,
    },
    /// A function type: `fn(params) -> ret`.
    Function {
        /// Parameter types.
        params: Vec<Type>,
        /// Return type.
        ret: Box<Type>,
    },
    /// Immutable reference: `&T`.
    Ref(Box<Type>),
    /// Mutable reference: `&mut T`.
    RefMut(Box<Type>),
    /// A tensor type with element type and optional shape dimensions.
    /// `None` dimensions are dynamic (unknown at compile time).
    Tensor {
        /// Element type (e.g., F32, F64).
        element: Box<Type>,
        /// Shape dimensions. `None` = dynamic, `Some(n)` = known size.
        dims: Vec<Option<u64>>,
    },
    /// A future type: `Future<T>` — produced by `async fn`.
    Future {
        /// The output type when the future resolves.
        inner: Box<Type>,
    },
    /// A type that couldn't be determined (error recovery).
    Unknown,
    /// A named type reference (not yet resolved).
    Named(String),
    /// A type variable from generic parameters (e.g., `T` in `fn max<T>`).
    TypeVar(String),
    /// A trait object type: `dyn Trait`.
    DynTrait(String),
}

impl Type {
    /// Returns `true` if this type is numeric (integer or float).
    pub fn is_numeric(&self) -> bool {
        self.is_integer() || self.is_float()
    }

    /// Returns `true` if this type is an integer type.
    pub fn is_integer(&self) -> bool {
        matches!(
            self,
            Type::I8
                | Type::I16
                | Type::I32
                | Type::I64
                | Type::I128
                | Type::U8
                | Type::U16
                | Type::U32
                | Type::U64
                | Type::U128
                | Type::ISize
                | Type::USize
                | Type::IntLiteral
        )
    }

    /// Returns `true` if this type is a float type.
    pub fn is_float(&self) -> bool {
        matches!(
            self,
            Type::F16 | Type::Bf16 | Type::F32 | Type::F64 | Type::FloatLiteral
        )
    }

    /// Returns `true` if this type is `Send` — safe to transfer between threads.
    ///
    /// All primitive types, strings, arrays of Send types, and functions are Send.
    /// Raw pointers are NOT Send.
    pub fn is_send(&self) -> bool {
        match self {
            // Primitives: always Send
            Type::Void
            | Type::Never
            | Type::I8
            | Type::I16
            | Type::I32
            | Type::I64
            | Type::I128
            | Type::U8
            | Type::U16
            | Type::U32
            | Type::U64
            | Type::U128
            | Type::ISize
            | Type::USize
            | Type::F16
            | Type::Bf16
            | Type::F32
            | Type::F64
            | Type::IntLiteral
            | Type::FloatLiteral
            | Type::Bool
            | Type::Char
            | Type::Str => true,
            // Arrays/tuples: Send if element types are Send
            Type::Array(elem) => elem.is_send(),
            Type::Tuple(elems) => elems.iter().all(|e| e.is_send()),
            // Structs: Send if all field types are Send
            Type::Struct { fields, .. } => fields.values().all(|f| f.is_send()),
            // Enums: Send (data payloads not tracked in type system)
            Type::Enum { .. } => true,
            // Functions: always Send
            Type::Function { .. } => true,
            // Immutable references: Send if inner is Send
            Type::Ref(inner) => inner.is_send(),
            // Mutable references: NOT Send — sharing &mut across threads is a data race
            Type::RefMut(_) => false,
            // Tensors: Send
            Type::Tensor { .. } => true,
            // Futures: Send if inner is Send
            Type::Future { inner } => inner.is_send(),
            // Trait objects: Send (concrete type was Send)
            Type::DynTrait(_) => true,
            // Unknown/Named: assume Send (error recovery)
            Type::Unknown | Type::Named(_) | Type::TypeVar(_) => true,
        }
    }

    /// Returns `true` if this type is `Sync` — safe to share between threads via &T.
    ///
    /// Same rules as Send for most types.
    pub fn is_sync(&self) -> bool {
        // For now, Sync == Send (most types in Fajar Lang are both or neither)
        self.is_send()
    }

    /// Resolves two compatible types to the most concrete one.
    ///
    /// When one side is a literal type and the other is concrete, returns the concrete type.
    /// When both are literals, returns the literal type. Otherwise returns `self`.
    pub fn resolve_with(&self, other: &Type) -> Type {
        match (self, other) {
            (Type::IntLiteral, t) if t.is_integer() && !matches!(t, Type::IntLiteral) => {
                other.clone()
            }
            (t, Type::IntLiteral) if t.is_integer() && !matches!(t, Type::IntLiteral) => {
                self.clone()
            }
            (Type::FloatLiteral, t) if t.is_float() && !matches!(t, Type::FloatLiteral) => {
                other.clone()
            }
            (t, Type::FloatLiteral) if t.is_float() && !matches!(t, Type::FloatLiteral) => {
                self.clone()
            }
            _ => self.clone(),
        }
    }

    /// Defaults unsuffixed literal types to their canonical form.
    ///
    /// `IntLiteral` → `I64`, `FloatLiteral` → `F64`.
    /// All other types are returned unchanged.
    pub fn default_literal(self) -> Type {
        match self {
            Type::IntLiteral => Type::I64,
            Type::FloatLiteral => Type::F64,
            other => other,
        }
    }

    /// Returns `true` if this type is a tensor.
    pub fn is_tensor(&self) -> bool {
        matches!(self, Type::Tensor { .. })
    }

    /// Returns a dynamic tensor type with unknown shape.
    /// Empty dims = any rank/shape, compatible with all tensor shapes.
    pub fn dynamic_tensor() -> Type {
        Type::Tensor {
            element: Box::new(Type::F64),
            dims: vec![],
        }
    }

    /// Computes the result shape for matmul: `[M,K] x [K,N] → [M,N]`.
    /// Returns `None` if shapes are incompatible.
    pub fn matmul_shape(&self, other: &Type) -> Option<Type> {
        if let (
            Type::Tensor {
                element: ea,
                dims: da,
            },
            Type::Tensor {
                element: eb,
                dims: db,
            },
        ) = (self, other)
        {
            if !ea.is_compatible(eb) || da.len() != 2 || db.len() != 2 {
                return None;
            }
            // Check K dimensions match
            match (&da[1], &db[0]) {
                (Some(k1), Some(k2)) if k1 != k2 => return None,
                _ => {} // dynamic or matching
            }
            Some(Type::Tensor {
                element: ea.clone(),
                dims: vec![da[0], db[1]],
            })
        } else {
            None
        }
    }

    /// Computes element-wise result shape (both tensors must have same shape).
    /// Empty dims = unknown rank → always compatible (returns dynamic tensor).
    /// Returns `None` if shapes are incompatible.
    pub fn elementwise_shape(&self, other: &Type) -> Option<Type> {
        if let (
            Type::Tensor {
                element: ea,
                dims: da,
            },
            Type::Tensor { dims: db, .. },
        ) = (self, other)
        {
            // Empty dims = unknown rank → always compatible
            if da.is_empty() || db.is_empty() {
                return Some(Type::Tensor {
                    element: ea.clone(),
                    dims: if da.is_empty() {
                        db.clone()
                    } else {
                        da.clone()
                    },
                });
            }
            if da.len() != db.len() {
                return None;
            }
            for (a, b) in da.iter().zip(db.iter()) {
                if let (Some(x), Some(y)) = (a, b) {
                    if x != y {
                        return None;
                    }
                }
            }
            Some(Type::Tensor {
                element: ea.clone(),
                dims: da.clone(),
            })
        } else {
            None
        }
    }

    /// Returns a human-readable name for this type.
    pub fn display_name(&self) -> String {
        match self {
            Type::Void => "void".into(),
            Type::Never => "never".into(),
            Type::I8 => "i8".into(),
            Type::I16 => "i16".into(),
            Type::I32 => "i32".into(),
            Type::I64 => "i64".into(),
            Type::I128 => "i128".into(),
            Type::U8 => "u8".into(),
            Type::U16 => "u16".into(),
            Type::U32 => "u32".into(),
            Type::U64 => "u64".into(),
            Type::U128 => "u128".into(),
            Type::ISize => "isize".into(),
            Type::USize => "usize".into(),
            Type::F16 => "f16".into(),
            Type::Bf16 => "bf16".into(),
            Type::F32 => "f32".into(),
            Type::F64 => "f64".into(),
            Type::IntLiteral => "{integer}".into(),
            Type::FloatLiteral => "{float}".into(),
            Type::Bool => "bool".into(),
            Type::Char => "char".into(),
            Type::Str => "str".into(),
            Type::Array(inner) => format!("[{}]", inner.display_name()),
            Type::Tuple(elems) => {
                let parts: Vec<String> = elems.iter().map(|t| t.display_name()).collect();
                format!("({})", parts.join(", "))
            }
            Type::Struct { name, .. } => name.clone(),
            Type::Enum { name } => name.clone(),
            Type::Function { params, ret } => {
                let parts: Vec<String> = params.iter().map(|t| t.display_name()).collect();
                format!("fn({}) -> {}", parts.join(", "), ret.display_name())
            }
            Type::Ref(inner) => format!("&{}", inner.display_name()),
            Type::RefMut(inner) => format!("&mut {}", inner.display_name()),
            Type::Tensor { element, dims } => {
                let dim_strs: Vec<String> = dims
                    .iter()
                    .map(|d| match d {
                        Some(n) => n.to_string(),
                        None => "*".into(),
                    })
                    .collect();
                format!(
                    "Tensor<{}>[{}]",
                    element.display_name(),
                    dim_strs.join(", ")
                )
            }
            Type::Future { inner } => format!("Future<{}>", inner.display_name()),
            Type::Unknown => "<unknown>".into(),
            Type::Named(n) => n.clone(),
            Type::TypeVar(n) => n.clone(),
            Type::DynTrait(n) => format!("dyn {n}"),
        }
    }

    /// Returns true if two types are compatible for assignment/comparison.
    ///
    /// `Unknown` is compatible with everything (error recovery).
    /// `Never` is compatible with everything (diverging expressions).
    pub fn is_compatible(&self, other: &Type) -> bool {
        if matches!(self, Type::Unknown | Type::TypeVar(_))
            || matches!(other, Type::Unknown | Type::TypeVar(_))
        {
            return true;
        }
        if matches!(self, Type::Never) || matches!(other, Type::Never) {
            return true;
        }
        // Unsuffixed integer literals are compatible with any numeric type
        // (integer or float — bidirectional: `let x: f64 = 1` is valid)
        if matches!(self, Type::IntLiteral) && other.is_numeric() {
            return true;
        }
        if matches!(other, Type::IntLiteral) && self.is_numeric() {
            return true;
        }
        // Unsuffixed float literals are compatible with any float type
        if matches!(self, Type::FloatLiteral) && other.is_float() {
            return true;
        }
        if matches!(other, Type::FloatLiteral) && self.is_float() {
            return true;
        }
        // Recursive compatibility for compound types
        if let (Type::Array(a), Type::Array(b)) = (self, other) {
            return a.is_compatible(b);
        }
        if let (Type::Tuple(a), Type::Tuple(b)) = (self, other) {
            return a.len() == b.len() && a.iter().zip(b.iter()).all(|(x, y)| x.is_compatible(y));
        }
        if let (Type::Ref(a), Type::Ref(b)) = (self, other) {
            return a.is_compatible(b);
        }
        if let (Type::RefMut(a), Type::RefMut(b)) = (self, other) {
            return a.is_compatible(b);
        }
        if let (
            Type::Tensor {
                element: ea,
                dims: da,
            },
            Type::Tensor {
                element: eb,
                dims: db,
            },
        ) = (self, other)
        {
            if !ea.is_compatible(eb) {
                return false;
            }
            // Empty dims = unknown rank/shape — compatible with any tensor shape
            if da.is_empty() || db.is_empty() {
                return true;
            }
            if da.len() != db.len() {
                return false;
            }
            return da.iter().zip(db.iter()).all(|(a, b)| match (a, b) {
                (Some(x), Some(y)) => x == y,
                _ => true, // dynamic dims are always compatible
            });
        }
        // Unknown is compatible with Tensor and vice versa (runtime-typed tensors)
        if matches!(
            (self, other),
            (Type::Unknown, Type::Tensor { .. }) | (Type::Tensor { .. }, Type::Unknown)
        ) {
            return true;
        }
        // Recursive compatibility for Future<T>
        if let (Type::Future { inner: a }, Type::Future { inner: b }) = (self, other) {
            return a.is_compatible(b);
        }
        // Recursive compatibility for Function types
        if let (
            Type::Function {
                params: pa,
                ret: ra,
            },
            Type::Function {
                params: pb,
                ret: rb,
            },
        ) = (self, other)
        {
            return pa.len() == pb.len()
                && pa.iter().zip(pb.iter()).all(|(a, b)| a.is_compatible(b))
                && ra.is_compatible(rb);
        }
        // dyn Trait compatibility: dyn T == dyn T
        if let (Type::DynTrait(a), Type::DynTrait(b)) = (self, other) {
            return a == b;
        }
        // A concrete struct/named type is compatible with dyn Trait
        // (actual trait impl check happens at assignment site in the checker)
        if matches!(other, Type::DynTrait(_))
            && matches!(self, Type::Struct { .. } | Type::Named(_))
        {
            return true;
        }
        if matches!(self, Type::DynTrait(_))
            && matches!(other, Type::Struct { .. } | Type::Named(_))
        {
            return true;
        }
        self == other
    }
}

impl std::fmt::Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Semantic errors
// ═══════════════════════════════════════════════════════════════════════

/// A semantic error detected during type checking.
#[derive(Debug, Clone, thiserror::Error)]
pub enum SemanticError {
    /// SE001: Undefined variable.
    #[error("SE001: undefined variable '{name}'{}", suggestion.as_ref().map(|s| format!(" — {s}")).unwrap_or_default())]
    UndefinedVariable {
        /// Variable name.
        name: String,
        /// Source location.
        span: Span,
        /// Optional suggestion ("did you mean 'X'?").
        suggestion: Option<String>,
    },

    /// SE002: Undefined function.
    #[error("SE002: undefined function '{name}'{}", suggestion.as_ref().map(|s| format!(" — {s}")).unwrap_or_default())]
    UndefinedFunction {
        /// Function name.
        name: String,
        /// Source location.
        span: Span,
        /// Optional suggestion ("did you mean 'X'?").
        suggestion: Option<String>,
    },

    /// SE003: Undefined type.
    #[error("SE003: undefined type '{name}'{}", suggestion.as_ref().map(|s| format!(" — {s}")).unwrap_or_default())]
    UndefinedType {
        /// Type name.
        name: String,
        /// Source location.
        span: Span,
        /// Optional suggestion ("did you mean 'X'?").
        suggestion: Option<String>,
    },

    /// SE004: Type mismatch.
    #[error("SE004: type mismatch: expected {expected}, found {found}{}", hint.as_ref().map(|h| format!(" ({h})")).unwrap_or_default())]
    TypeMismatch {
        /// Expected type.
        expected: String,
        /// Actual type.
        found: String,
        /// Source location.
        span: Span,
        /// Optional hint about possible fix.
        hint: Option<String>,
    },

    /// SE005: Argument count mismatch.
    #[error("SE005: expected {expected} arguments, found {found}")]
    ArgumentCountMismatch {
        /// Expected count.
        expected: usize,
        /// Actual count.
        found: usize,
        /// Source location.
        span: Span,
    },

    /// SE006: Duplicate definition.
    #[error("SE006: '{name}' is already defined in this scope")]
    DuplicateDefinition {
        /// Name that's duplicated.
        name: String,
        /// Source location.
        span: Span,
    },

    /// SE007: Assignment to immutable variable.
    #[error("SE007: cannot assign to immutable variable '{name}'")]
    ImmutableAssignment {
        /// Variable name.
        name: String,
        /// Source location.
        span: Span,
    },

    /// SE008: Missing return value.
    #[error("SE008: function '{name}' must return {expected}")]
    MissingReturn {
        /// Function name.
        name: String,
        /// Expected return type.
        expected: String,
        /// Source location.
        span: Span,
    },

    /// SE009: Unused variable (warning).
    #[error("SE009: unused variable '{name}'")]
    UnusedVariable {
        /// Variable name.
        name: String,
        /// Source location.
        span: Span,
    },

    /// SE010: Unreachable code (warning).
    #[error("SE010: unreachable code")]
    UnreachableCode {
        /// Source location.
        span: Span,
    },

    /// SE011: Non-exhaustive match.
    #[error("SE011: non-exhaustive match — add a wildcard `_` pattern")]
    NonExhaustiveMatch {
        /// Source location.
        span: Span,
    },

    /// `break` or `continue` used outside of a loop.
    #[error("break/continue outside of loop")]
    BreakOutsideLoop {
        /// Source location.
        span: Span,
    },

    /// `return` used outside of a function.
    #[error("return outside of function")]
    ReturnOutsideFunction {
        /// Source location.
        span: Span,
    },

    /// SE012: Missing field in struct initialization.
    #[error("SE012: missing field '{field}' in struct '{struct_name}'")]
    MissingField {
        /// Struct name.
        struct_name: String,
        /// Missing field.
        field: String,
        /// Source location.
        span: Span,
    },

    /// KE001: Heap allocation in @kernel context.
    #[error("KE001: heap allocation not allowed in @kernel context")]
    HeapAllocInKernel {
        /// Source location.
        span: Span,
    },

    /// KE002: Tensor operation in @kernel context.
    #[error("KE002: tensor operations not allowed in @kernel context")]
    TensorInKernel {
        /// Source location.
        span: Span,
    },

    /// KE003: Calling @device function from @kernel context.
    #[error("KE003: cannot call @device function from @kernel context")]
    DeviceCallInKernel {
        /// Source location.
        span: Span,
    },

    /// DE001: Raw pointer operation in @device context.
    #[error("DE001: raw pointer operations not allowed in @device context")]
    RawPointerInDevice {
        /// Source location.
        span: Span,
    },

    /// DE002: Calling @kernel function from @device context.
    #[error("DE002: cannot call @kernel function from @device context")]
    KernelCallInDevice {
        /// Source location.
        span: Span,
    },

    /// NE001: Raw pointer operation in @npu context.
    #[error("NE001: raw pointer operations not allowed in @npu context")]
    RawPointerInNpu {
        /// Source location.
        span: Span,
    },

    /// NE002: Heap allocation in @npu context.
    #[error("NE002: heap allocation not allowed in @npu context")]
    HeapAllocInNpu {
        /// Source location.
        span: Span,
    },

    /// NE003: OS primitive in @npu context.
    #[error("NE003: OS primitives not allowed in @npu context")]
    OsPrimitiveInNpu {
        /// Source location.
        span: Span,
    },

    /// NE004: Calling @kernel function from @npu context.
    #[error("NE004: cannot call @kernel function from @npu context")]
    KernelCallInNpu {
        /// Source location.
        span: Span,
    },

    /// KE005: Inline assembly in @safe context.
    #[error("KE005: inline assembly not allowed in @safe context")]
    AsmInSafeContext {
        /// Source location.
        span: Span,
    },

    /// KE006: Inline assembly in @device context.
    #[error("KE006: inline assembly not allowed in @device context")]
    AsmInDeviceContext {
        /// Source location.
        span: Span,
    },

    /// SE017: Await outside async context.
    #[error("SE017: `.await` is only valid inside `async fn`")]
    AwaitOutsideAsync {
        /// Source location.
        span: Span,
    },

    /// SE018: Non-Send type in thread::spawn argument.
    #[error("SE018: type '{ty}' is not `Send` and cannot be transferred to another thread")]
    NotSendType {
        /// The offending type name.
        ty: String,
        /// Source location.
        span: Span,
    },

    /// SE013: Non-FFI-safe type in extern function declaration.
    #[error("SE013: type '{ty}' is not FFI-safe in extern function '{func}'")]
    FfiUnsafeType {
        /// The offending type.
        ty: String,
        /// The function name.
        func: String,
        /// Source location.
        span: Span,
    },

    /// ME001: Use after move.
    #[error("ME001: use of moved variable '{name}'")]
    UseAfterMove {
        /// Variable name.
        name: String,
        /// Where it was used.
        span: Span,
        /// Where it was moved.
        move_span: Span,
    },

    /// ME003: Cannot move while borrowed.
    #[error("ME003: cannot move '{name}' because it is borrowed")]
    MoveWhileBorrowed {
        /// Variable name.
        name: String,
        /// Where the move was attempted.
        span: Span,
        /// Where the borrow was created.
        borrow_span: Span,
    },

    /// ME004: Cannot borrow mutably while already borrowed.
    #[error("ME004: cannot borrow '{name}' as mutable because it is also borrowed")]
    MutBorrowConflict {
        /// Variable name.
        name: String,
        /// Where the conflicting borrow was attempted.
        span: Span,
        /// Where the existing borrow was created.
        borrow_span: Span,
    },

    /// ME005: Cannot borrow immutably while mutably borrowed.
    #[error("ME005: cannot borrow '{name}' as immutable because it is mutably borrowed")]
    ImmBorrowConflict {
        /// Variable name.
        name: String,
        /// Where the conflicting borrow was attempted.
        span: Span,
        /// Where the mutable borrow was created.
        borrow_span: Span,
    },

    /// SE014: Trait bound not satisfied.
    #[error("SE014: type '{concrete_type}' does not implement trait '{trait_name}' (required by generic bound on '{param_name}')")]
    TraitBoundNotSatisfied {
        /// The concrete type that doesn't satisfy the bound.
        concrete_type: String,
        /// The trait that's required.
        trait_name: String,
        /// The generic parameter with the bound.
        param_name: String,
        /// Source location.
        span: Span,
    },

    /// SE015: Unknown trait referenced in bound.
    #[error("SE015: unknown trait '{name}' in generic bound")]
    UnknownTrait {
        /// Trait name.
        name: String,
        /// Source location.
        span: Span,
    },

    /// SE013: Cannot infer type parameter.
    #[error("SE013: cannot infer type for '{param}': {reason}")]
    CannotInferType {
        /// Type parameter name.
        param: String,
        /// Reason for inference failure.
        reason: String,
        /// Source location.
        span: Span,
    },

    /// SE016: Trait method signature mismatch.
    #[error("SE016: method '{method}' in impl {trait_name} for {target_type} has wrong signature: {detail}")]
    TraitMethodSignatureMismatch {
        /// Method name.
        method: String,
        /// Trait name.
        trait_name: String,
        /// Target type name.
        target_type: String,
        /// Mismatch detail.
        detail: String,
        /// Source location.
        span: Span,
    },

    /// TE001: Tensor shape mismatch.
    #[error("TE001: tensor shape mismatch: {detail}")]
    TensorShapeMismatch {
        /// Mismatch detail.
        detail: String,
        /// Source location.
        span: Span,
    },

    /// SE019: Unused import (warning).
    #[error("SE019: unused import '{name}'")]
    UnusedImport {
        /// Import name.
        name: String,
        /// Source location.
        span: Span,
    },

    /// SE020: Unreachable match pattern (warning).
    #[error("SE020: unreachable pattern — previous pattern already matches all values")]
    UnreachablePattern {
        /// Source location.
        span: Span,
    },

    /// SE021: Lifetime mismatch — a reference's lifetime doesn't match the required lifetime.
    #[error("SE021: lifetime mismatch: expected '{expected}, found '{found}")]
    LifetimeMismatch {
        /// Expected lifetime name.
        expected: String,
        /// Found lifetime name.
        found: String,
        /// Source location.
        span: Span,
    },

    /// ME009: Lifetime conflict — two lifetimes in the same scope are incompatible.
    #[error("ME009: lifetime '{name}' conflicts with another lifetime in scope")]
    LifetimeConflict {
        /// Conflicting lifetime name.
        name: String,
        /// Source location.
        span: Span,
    },

    /// ME010: Dangling reference — a reference outlives its referent.
    #[error(
        "ME010: dangling reference: reference with lifetime '{lifetime}' outlives its referent"
    )]
    DanglingReference {
        /// The lifetime that causes the dangling reference.
        lifetime: String,
        /// Source location.
        span: Span,
    },
}

impl SemanticError {
    /// Returns the source span for this error.
    pub fn span(&self) -> Span {
        match self {
            SemanticError::UndefinedVariable { span, .. }
            | SemanticError::UndefinedFunction { span, .. }
            | SemanticError::UndefinedType { span, .. }
            | SemanticError::TypeMismatch { span, .. }
            | SemanticError::ArgumentCountMismatch { span, .. }
            | SemanticError::DuplicateDefinition { span, .. }
            | SemanticError::ImmutableAssignment { span, .. }
            | SemanticError::MissingReturn { span, .. }
            | SemanticError::UnusedVariable { span, .. }
            | SemanticError::UnreachableCode { span, .. }
            | SemanticError::NonExhaustiveMatch { span, .. }
            | SemanticError::BreakOutsideLoop { span, .. }
            | SemanticError::ReturnOutsideFunction { span, .. }
            | SemanticError::MissingField { span, .. }
            | SemanticError::HeapAllocInKernel { span, .. }
            | SemanticError::TensorInKernel { span, .. }
            | SemanticError::DeviceCallInKernel { span, .. }
            | SemanticError::RawPointerInDevice { span, .. }
            | SemanticError::KernelCallInDevice { span, .. }
            | SemanticError::AsmInSafeContext { span, .. }
            | SemanticError::AsmInDeviceContext { span, .. }
            | SemanticError::AwaitOutsideAsync { span, .. }
            | SemanticError::NotSendType { span, .. }
            | SemanticError::FfiUnsafeType { span, .. }
            | SemanticError::UseAfterMove { span, .. }
            | SemanticError::MoveWhileBorrowed { span, .. }
            | SemanticError::MutBorrowConflict { span, .. }
            | SemanticError::ImmBorrowConflict { span, .. }
            | SemanticError::TraitBoundNotSatisfied { span, .. }
            | SemanticError::UnknownTrait { span, .. }
            | SemanticError::CannotInferType { span, .. }
            | SemanticError::TraitMethodSignatureMismatch { span, .. }
            | SemanticError::TensorShapeMismatch { span, .. }
            | SemanticError::UnusedImport { span, .. }
            | SemanticError::UnreachablePattern { span, .. }
            | SemanticError::LifetimeMismatch { span, .. }
            | SemanticError::LifetimeConflict { span, .. }
            | SemanticError::DanglingReference { span, .. }
            | SemanticError::RawPointerInNpu { span, .. }
            | SemanticError::HeapAllocInNpu { span, .. }
            | SemanticError::OsPrimitiveInNpu { span, .. }
            | SemanticError::KernelCallInNpu { span, .. } => *span,
        }
    }

    /// Returns `true` if this is a warning (not a hard error).
    pub fn is_warning(&self) -> bool {
        matches!(
            self,
            SemanticError::UnusedVariable { .. }
                | SemanticError::UnreachableCode { .. }
                | SemanticError::UnusedImport { .. }
                | SemanticError::UnreachablePattern { .. }
        )
    }
}

/// Checks if two types are compatible for trait signature matching.
///
/// `Unknown` types (from unresolved generics or `self`) are treated as
/// compatible with any other type.
fn types_compatible(a: &Type, b: &Type) -> bool {
    matches!((a, b), (Type::Unknown, _) | (_, Type::Unknown)) || a == b
}

// ═══════════════════════════════════════════════════════════════════════
// Type Checker
// ═══════════════════════════════════════════════════════════════════════

/// The semantic analyzer / type checker.
///
/// Walks the AST, builds a symbol table, and checks for type errors.
/// Collects all errors (does not stop at first error).
pub struct TypeChecker {
    /// The symbol table for name resolution.
    symbols: SymbolTable,
    /// Collected semantic errors.
    errors: Vec<SemanticError>,
    /// Functions annotated with `@kernel`.
    kernel_fns: std::collections::HashSet<String>,
    /// Functions annotated with `@device`.
    device_fns: std::collections::HashSet<String>,
    /// Functions annotated with `@npu`.
    npu_fns: std::collections::HashSet<String>,
    /// Functions that are OS builtins (only callable from @kernel/@unsafe).
    os_builtins: std::collections::HashSet<String>,
    /// Builtins that perform heap allocation (forbidden in @kernel).
    heap_builtins: std::collections::HashSet<String>,
    /// Builtins that perform tensor/ML operations (forbidden in @kernel).
    tensor_builtins: std::collections::HashSet<String>,
    /// Registered trait definitions: trait name → method signatures.
    traits: HashMap<String, Vec<TraitMethodSig>>,
    /// Registered trait implementations: (trait_name, type_name) → implemented.
    trait_impls: std::collections::HashSet<(String, String)>,
    /// Type aliases: alias name → resolved type.
    type_aliases: HashMap<String, Type>,
    /// Move tracker for ownership analysis.
    moves: super::borrow_lite::MoveTracker,
    /// NLL liveness info for the current function body (None outside functions).
    nll_info: Option<super::cfg::NllInfo>,
    /// Enum definitions: enum name → list of variant names (for exhaustiveness).
    enum_variants: HashMap<String, Vec<String>>,
    /// Tracked imports: (import name, span, used) — for unused import detection.
    imports: Vec<(String, Span, bool)>,
}

/// A trait method signature for validation.
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct TraitMethodSig {
    /// Method name.
    name: String,
    /// Parameter types (including self).
    param_types: Vec<Type>,
    /// Return type.
    ret_type: Type,
}

impl TypeChecker {
    /// Creates a new type checker.
    pub fn new() -> Self {
        let os_builtins: std::collections::HashSet<String> = [
            "mem_alloc",
            "mem_free",
            "mem_read_u8",
            "mem_read_u32",
            "mem_read_u64",
            "mem_write_u8",
            "mem_write_u32",
            "mem_write_u64",
            "page_map",
            "page_unmap",
            "irq_register",
            "irq_unregister",
            "irq_enable",
            "irq_disable",
            "port_read",
            "port_write",
            "syscall_define",
            "syscall_dispatch",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();

        let heap_builtins: std::collections::HashSet<String> = ["push", "pop", "to_string"]
            .iter()
            .map(|s| s.to_string())
            .collect();

        let tensor_builtins: std::collections::HashSet<String> = [
            "tensor_zeros",
            "tensor_ones",
            "tensor_randn",
            "tensor_eye",
            "tensor_full",
            "tensor_from_data",
            "tensor_shape",
            "tensor_reshape",
            "tensor_numel",
            "tensor_add",
            "tensor_sub",
            "tensor_mul",
            "tensor_div",
            "tensor_neg",
            "tensor_matmul",
            "tensor_transpose",
            "tensor_sum",
            "tensor_mean",
            "tensor_relu",
            "tensor_sigmoid",
            "tensor_tanh",
            "tensor_softmax",
            "tensor_gelu",
            "tensor_leaky_relu",
            "tensor_mse_loss",
            "tensor_cross_entropy",
            "tensor_bce_loss",
            "tensor_flatten",
            "tensor_squeeze",
            "tensor_unsqueeze",
            "tensor_max",
            "tensor_min",
            "tensor_argmax",
            "tensor_arange",
            "tensor_linspace",
            "tensor_xavier",
            "tensor_l1_loss",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();

        let mut tc = TypeChecker {
            symbols: SymbolTable::new(),
            errors: Vec::new(),
            kernel_fns: std::collections::HashSet::new(),
            device_fns: std::collections::HashSet::new(),
            npu_fns: std::collections::HashSet::new(),
            os_builtins,
            heap_builtins,
            tensor_builtins,
            traits: HashMap::new(),
            trait_impls: std::collections::HashSet::new(),
            type_aliases: HashMap::new(),
            moves: super::borrow_lite::MoveTracker::new(),
            nll_info: None,
            enum_variants: HashMap::new(),
            imports: Vec::new(),
        };
        tc.register_builtins();
        tc.register_builtin_traits();
        // Register built-in enum variants for exhaustiveness checking
        tc.enum_variants.insert(
            "Option".to_string(),
            vec!["Some".to_string(), "None".to_string()],
        );
        tc.enum_variants.insert(
            "Result".to_string(),
            vec!["Ok".to_string(), "Err".to_string()],
        );
        tc.enum_variants.insert(
            "Poll".to_string(),
            vec!["Ready".to_string(), "Pending".to_string()],
        );
        tc
    }

    /// Registers built-in functions in the global scope.
    fn register_builtins(&mut self) {
        let builtins = vec![
            (
                "print",
                Type::Function {
                    params: vec![Type::Unknown],
                    ret: Box::new(Type::Void),
                },
            ),
            (
                "println",
                Type::Function {
                    params: vec![Type::Unknown],
                    ret: Box::new(Type::Void),
                },
            ),
            (
                "len",
                Type::Function {
                    params: vec![Type::Unknown],
                    ret: Box::new(Type::USize),
                },
            ),
            (
                "type_of",
                Type::Function {
                    params: vec![Type::Unknown],
                    ret: Box::new(Type::Str),
                },
            ),
            (
                "to_string",
                Type::Function {
                    params: vec![Type::Unknown],
                    ret: Box::new(Type::Str),
                },
            ),
            (
                "to_int",
                Type::Function {
                    params: vec![Type::Unknown],
                    ret: Box::new(Type::I64),
                },
            ),
            (
                "to_float",
                Type::Function {
                    params: vec![Type::Unknown],
                    ret: Box::new(Type::F64),
                },
            ),
            (
                "format",
                Type::Function {
                    params: vec![Type::Unknown],
                    ret: Box::new(Type::Str),
                },
            ),
            (
                "assert",
                Type::Function {
                    params: vec![Type::Bool],
                    ret: Box::new(Type::Void),
                },
            ),
            (
                "assert_eq",
                Type::Function {
                    params: vec![Type::Unknown, Type::Unknown],
                    ret: Box::new(Type::Void),
                },
            ),
            (
                "push",
                Type::Function {
                    params: vec![Type::Unknown, Type::Unknown],
                    ret: Box::new(Type::Unknown),
                },
            ),
            (
                "pop",
                Type::Function {
                    params: vec![Type::Unknown],
                    ret: Box::new(Type::Unknown),
                },
            ),
        ];

        for (name, ty) in builtins {
            self.symbols.define(Symbol {
                name: name.to_string(),
                ty,
                mutable: false,
                span: Span::new(0, 0),
                used: false,
            });
        }

        // OS runtime builtins (all return Unknown for now — proper typing later)
        let os_fns: Vec<(&str, Vec<Type>, Type)> = vec![
            ("mem_alloc", vec![Type::I64, Type::I64], Type::Unknown),
            ("mem_free", vec![Type::Unknown], Type::Void),
            ("mem_read_u8", vec![Type::Unknown], Type::I64),
            ("mem_read_u32", vec![Type::Unknown], Type::I64),
            ("mem_read_u64", vec![Type::Unknown], Type::I64),
            ("mem_write_u8", vec![Type::Unknown, Type::I64], Type::Void),
            ("mem_write_u32", vec![Type::Unknown, Type::I64], Type::Void),
            ("mem_write_u64", vec![Type::Unknown, Type::I64], Type::Void),
            (
                "page_map",
                vec![Type::Unknown, Type::Unknown, Type::I64],
                Type::Void,
            ),
            ("page_unmap", vec![Type::Unknown], Type::Void),
            ("irq_register", vec![Type::I64, Type::Str], Type::Void),
            ("irq_unregister", vec![Type::I64], Type::Void),
            ("irq_enable", vec![], Type::Void),
            ("irq_disable", vec![], Type::Void),
            ("port_read", vec![Type::I64], Type::I64),
            ("port_write", vec![Type::I64, Type::I64], Type::Void),
            (
                "syscall_define",
                vec![Type::I64, Type::Str, Type::I64],
                Type::Void,
            ),
            ("syscall_dispatch", vec![Type::Unknown], Type::Str),
        ];
        for (name, params, ret) in os_fns {
            self.symbols.define(Symbol {
                name: name.to_string(),
                ty: Type::Function {
                    params,
                    ret: Box::new(ret),
                },
                mutable: false,
                span: Span::new(0, 0),
                used: false,
            });
        }

        // ML runtime builtins (tensor operations)
        // Dynamic tensor type: Tensor<f64>[] — unknown shape, compatible with all tensors
        let dyn_t = Type::dynamic_tensor();
        let ml_fns: Vec<(&str, Vec<Type>, Type)> = vec![
            // Creation functions → return dynamic tensor
            ("tensor_zeros", vec![Type::Unknown], dyn_t.clone()),
            ("tensor_ones", vec![Type::Unknown], dyn_t.clone()),
            ("tensor_randn", vec![Type::Unknown], dyn_t.clone()),
            ("zeros", vec![Type::Unknown], dyn_t.clone()),
            ("ones", vec![Type::Unknown], dyn_t.clone()),
            ("randn", vec![Type::Unknown], dyn_t.clone()),
            ("tensor_rand", vec![Type::Unknown], dyn_t.clone()),
            ("tensor_eye", vec![Type::I64], dyn_t.clone()),
            ("tensor_full", vec![Type::Unknown, Type::F64], dyn_t.clone()),
            (
                "tensor_from_data",
                vec![Type::Unknown, Type::Unknown],
                dyn_t.clone(),
            ),
            // Shape query
            ("tensor_shape", vec![dyn_t.clone()], Type::Unknown), // returns array
            (
                "tensor_reshape",
                vec![dyn_t.clone(), Type::Unknown],
                dyn_t.clone(),
            ),
            ("tensor_numel", vec![dyn_t.clone()], Type::I64),
            // Tensor arithmetic → return dynamic tensor
            (
                "tensor_add",
                vec![dyn_t.clone(), dyn_t.clone()],
                dyn_t.clone(),
            ),
            (
                "tensor_sub",
                vec![dyn_t.clone(), dyn_t.clone()],
                dyn_t.clone(),
            ),
            (
                "tensor_mul",
                vec![dyn_t.clone(), dyn_t.clone()],
                dyn_t.clone(),
            ),
            (
                "tensor_div",
                vec![dyn_t.clone(), dyn_t.clone()],
                dyn_t.clone(),
            ),
            ("tensor_neg", vec![dyn_t.clone()], dyn_t.clone()),
            (
                "tensor_matmul",
                vec![dyn_t.clone(), dyn_t.clone()],
                dyn_t.clone(),
            ),
            ("tensor_transpose", vec![dyn_t.clone()], dyn_t.clone()),
            ("tensor_sum", vec![dyn_t.clone()], dyn_t.clone()),
            ("tensor_mean", vec![dyn_t.clone()], dyn_t.clone()),
            // Activation functions → return dynamic tensor (same shape as input)
            ("tensor_relu", vec![dyn_t.clone()], dyn_t.clone()),
            ("tensor_sigmoid", vec![dyn_t.clone()], dyn_t.clone()),
            ("tensor_tanh", vec![dyn_t.clone()], dyn_t.clone()),
            ("tensor_softmax", vec![dyn_t.clone()], dyn_t.clone()),
            ("tensor_gelu", vec![dyn_t.clone()], dyn_t.clone()),
            (
                "tensor_leaky_relu",
                vec![dyn_t.clone(), Type::F64],
                dyn_t.clone(),
            ),
            // Loss functions → return dynamic tensor
            (
                "tensor_mse_loss",
                vec![dyn_t.clone(), dyn_t.clone()],
                dyn_t.clone(),
            ),
            (
                "tensor_cross_entropy",
                vec![dyn_t.clone(), dyn_t.clone()],
                dyn_t.clone(),
            ),
            (
                "tensor_bce_loss",
                vec![dyn_t.clone(), dyn_t.clone()],
                dyn_t.clone(),
            ),
            // Shape manipulation → return dynamic tensor
            ("tensor_flatten", vec![dyn_t.clone()], dyn_t.clone()),
            (
                "tensor_squeeze",
                vec![dyn_t.clone(), Type::I64],
                dyn_t.clone(),
            ),
            (
                "tensor_unsqueeze",
                vec![dyn_t.clone(), Type::I64],
                dyn_t.clone(),
            ),
            // Reductions → return dynamic tensor
            ("tensor_max", vec![dyn_t.clone()], dyn_t.clone()),
            ("tensor_min", vec![dyn_t.clone()], dyn_t.clone()),
            ("tensor_argmax", vec![dyn_t.clone()], dyn_t.clone()),
            // Creation
            (
                "tensor_arange",
                vec![Type::F64, Type::F64, Type::F64],
                dyn_t.clone(),
            ),
            (
                "tensor_linspace",
                vec![Type::F64, Type::F64, Type::I64],
                dyn_t.clone(),
            ),
            ("tensor_xavier", vec![Type::I64, Type::I64], dyn_t.clone()),
            // Loss
            (
                "tensor_l1_loss",
                vec![dyn_t.clone(), dyn_t.clone()],
                dyn_t.clone(),
            ),
        ];
        for (name, params, ret) in ml_fns {
            self.symbols.define(Symbol {
                name: name.to_string(),
                ty: Type::Function {
                    params,
                    ret: Box::new(ret),
                },
                mutable: false,
                span: Span::new(0, 0),
                used: false,
            });
        }

        // Built-in enum constructors (Some, None, Ok, Err, Ready, Pending)
        let enum_constructors: Vec<(&str, Type)> = vec![
            (
                "Some",
                Type::Function {
                    params: vec![Type::Unknown],
                    ret: Box::new(Type::Unknown),
                },
            ),
            ("None", Type::Unknown),
            (
                "Ok",
                Type::Function {
                    params: vec![Type::Unknown],
                    ret: Box::new(Type::Unknown),
                },
            ),
            (
                "Err",
                Type::Function {
                    params: vec![Type::Unknown],
                    ret: Box::new(Type::Unknown),
                },
            ),
            (
                "Ready",
                Type::Function {
                    params: vec![Type::Unknown],
                    ret: Box::new(Type::Enum {
                        name: "Poll".to_string(),
                    }),
                },
            ),
            (
                "Pending",
                Type::Enum {
                    name: "Poll".to_string(),
                },
            ),
        ];
        for (name, ty) in enum_constructors {
            self.symbols.define(Symbol {
                name: name.to_string(),
                ty,
                mutable: false,
                span: Span::new(0, 0),
                used: false,
            });
        }

        // Built-in constants (PI, E)
        self.symbols.define(Symbol {
            name: "PI".to_string(),
            ty: Type::F64,
            mutable: false,
            span: Span::new(0, 0),
            used: false,
        });
        self.symbols.define(Symbol {
            name: "E".to_string(),
            ty: Type::F64,
            mutable: false,
            span: Span::new(0, 0),
            used: false,
        });

        // Error/debug builtins
        let debug_fns: Vec<(&str, Vec<Type>, Type)> = vec![
            ("panic", vec![Type::Unknown], Type::Never),
            ("todo", vec![], Type::Never),
            ("dbg", vec![Type::Unknown], Type::Unknown),
            ("eprint", vec![Type::Unknown], Type::Void),
            ("eprintln", vec![Type::Unknown], Type::Void),
        ];
        for (name, params, ret) in debug_fns {
            self.symbols.define(Symbol {
                name: name.to_string(),
                ty: Type::Function {
                    params,
                    ret: Box::new(ret),
                },
                mutable: false,
                span: Span::new(0, 0),
                used: false,
            });
        }

        // Math builtins (accept Unknown to allow both int and float args)
        let math_fns: Vec<(&str, Vec<Type>, Type)> = vec![
            ("abs", vec![Type::Unknown], Type::Unknown),
            ("sqrt", vec![Type::Unknown], Type::F64),
            ("pow", vec![Type::Unknown, Type::Unknown], Type::F64),
            ("log", vec![Type::Unknown], Type::F64),
            ("log2", vec![Type::Unknown], Type::F64),
            ("log10", vec![Type::Unknown], Type::F64),
            ("sin", vec![Type::Unknown], Type::F64),
            ("cos", vec![Type::Unknown], Type::F64),
            ("tan", vec![Type::Unknown], Type::F64),
            ("floor", vec![Type::Unknown], Type::F64),
            ("ceil", vec![Type::Unknown], Type::F64),
            ("round", vec![Type::Unknown], Type::F64),
            (
                "clamp",
                vec![Type::Unknown, Type::Unknown, Type::Unknown],
                Type::Unknown,
            ),
            ("min", vec![Type::Unknown, Type::Unknown], Type::Unknown),
            ("max", vec![Type::Unknown, Type::Unknown], Type::Unknown),
        ];
        for (name, params, ret) in math_fns {
            self.symbols.define(Symbol {
                name: name.to_string(),
                ty: Type::Function {
                    params,
                    ret: Box::new(ret),
                },
                mutable: false,
                span: Span::new(0, 0),
                used: false,
            });
        }

        // Collection builtins (HashMap)
        let collection_fns: Vec<(&str, Vec<Type>, Type)> = vec![
            ("map_new", vec![], Type::Unknown),
            (
                "map_insert",
                vec![Type::Unknown, Type::Str, Type::Unknown],
                Type::Unknown,
            ),
            ("map_get", vec![Type::Unknown, Type::Str], Type::Unknown),
            ("map_remove", vec![Type::Unknown, Type::Str], Type::Unknown),
            (
                "map_contains_key",
                vec![Type::Unknown, Type::Str],
                Type::Bool,
            ),
            ("map_keys", vec![Type::Unknown], Type::Unknown),
            ("map_values", vec![Type::Unknown], Type::Unknown),
            ("map_len", vec![Type::Unknown], Type::I64),
        ];
        for (name, params, ret) in collection_fns {
            self.symbols.define(Symbol {
                name: name.to_string(),
                ty: Type::Function {
                    params,
                    ret: Box::new(ret),
                },
                mutable: false,
                span: Span::new(0, 0),
                used: false,
            });
        }

        // File I/O builtins
        let io_fns: Vec<(&str, Vec<Type>, Type)> = vec![
            ("read_file", vec![Type::Str], Type::Unknown),
            ("write_file", vec![Type::Str, Type::Str], Type::Unknown),
            ("append_file", vec![Type::Str, Type::Str], Type::Unknown),
            ("file_exists", vec![Type::Str], Type::Bool),
        ];
        for (name, params, ret) in io_fns {
            self.symbols.define(Symbol {
                name: name.to_string(),
                ty: Type::Function {
                    params,
                    ret: Box::new(ret),
                },
                mutable: false,
                span: Span::new(0, 0),
                used: false,
            });
        }

        // Metrics builtins
        let metrics_fns: Vec<(&str, Vec<Type>, Type)> = vec![
            (
                "metric_accuracy",
                vec![Type::Unknown, Type::Unknown],
                Type::F64,
            ),
            (
                "metric_precision",
                vec![Type::Unknown, Type::Unknown, Type::I64],
                Type::F64,
            ),
            (
                "metric_recall",
                vec![Type::Unknown, Type::Unknown, Type::I64],
                Type::F64,
            ),
            (
                "metric_f1_score",
                vec![Type::Unknown, Type::Unknown, Type::I64],
                Type::F64,
            ),
        ];
        for (name, params, ret) in metrics_fns {
            self.symbols.define(Symbol {
                name: name.to_string(),
                ty: Type::Function {
                    params,
                    ret: Box::new(ret),
                },
                mutable: false,
                span: Span::new(0, 0),
                used: false,
            });
        }

        // Autograd builtins
        let autograd_fns: Vec<(&str, Vec<Type>, Type)> = vec![
            ("tensor_backward", vec![dyn_t.clone()], Type::Void),
            ("tensor_grad", vec![dyn_t.clone()], dyn_t.clone()),
            ("tensor_requires_grad", vec![dyn_t.clone()], Type::Bool),
            (
                "tensor_set_requires_grad",
                vec![dyn_t.clone(), Type::Bool],
                dyn_t.clone(),
            ),
            ("tensor_detach", vec![dyn_t.clone()], dyn_t.clone()),
            ("tensor_clear_tape", vec![], Type::Void),
            ("tensor_no_grad_begin", vec![], Type::Void),
            ("tensor_no_grad_end", vec![], Type::Void),
        ];
        for (name, params, ret) in autograd_fns {
            self.symbols.define(Symbol {
                name: name.to_string(),
                ty: Type::Function {
                    params,
                    ret: Box::new(ret),
                },
                mutable: false,
                span: Span::new(0, 0),
                used: false,
            });
        }

        // Optimizer builtins
        let optim_fns: Vec<(&str, Vec<Type>, Type)> = vec![
            (
                "optimizer_sgd",
                vec![Type::Unknown, Type::Unknown],
                Type::Unknown,
            ),
            ("optimizer_adam", vec![Type::F64], Type::Unknown),
            (
                "optimizer_step",
                vec![Type::Unknown, Type::Unknown],
                Type::Void,
            ),
            ("optimizer_zero_grad", vec![Type::Unknown], Type::Void),
        ];
        for (name, params, ret) in optim_fns {
            self.symbols.define(Symbol {
                name: name.to_string(),
                ty: Type::Function {
                    params,
                    ret: Box::new(ret),
                },
                mutable: false,
                span: Span::new(0, 0),
                used: false,
            });
        }

        // Layer builtins
        let layer_fns: Vec<(&str, Vec<Type>, Type)> = vec![
            ("layer_dense", vec![Type::I64, Type::I64], Type::Unknown),
            (
                "layer_forward",
                vec![Type::Unknown, Type::Unknown],
                Type::Unknown,
            ),
            ("layer_params", vec![Type::Unknown], Type::Unknown),
        ];
        for (name, params, ret) in layer_fns {
            self.symbols.define(Symbol {
                name: name.to_string(),
                ty: Type::Function {
                    params,
                    ret: Box::new(ret),
                },
                mutable: false,
                span: Span::new(0, 0),
                used: false,
            });
        }

        // Hardware detection builtins (v1.1)
        let hw_fns: Vec<(&str, Vec<Type>, Type)> = vec![
            ("hw_cpu_vendor", vec![], Type::Str),
            ("hw_cpu_arch", vec![], Type::Str),
            ("hw_has_avx2", vec![], Type::Bool),
            ("hw_has_avx512", vec![], Type::Bool),
            ("hw_has_amx", vec![], Type::Bool),
            ("hw_has_neon", vec![], Type::Bool),
            ("hw_has_sve", vec![], Type::Bool),
            ("hw_simd_width", vec![], Type::I64),
            // Accelerator registry builtins (v1.1 S4)
            ("hw_gpu_count", vec![], Type::I64),
            ("hw_npu_count", vec![], Type::I64),
            ("hw_best_accelerator", vec![], Type::Str),
        ];
        for (name, params, ret) in hw_fns {
            self.symbols.define(Symbol {
                name: name.to_string(),
                ty: Type::Function {
                    params,
                    ret: Box::new(ret),
                },
                mutable: false,
                span: Span::new(0, 0),
                used: false,
            });
        }
    }

    /// Registers built-in traits and their implementations for primitive types.
    fn register_builtin_traits(&mut self) {
        // Built-in trait names (no methods needed for bound checking)
        let builtin_trait_names = [
            "Display",
            "Debug",
            "Clone",
            "Copy",
            "PartialEq",
            "Eq",
            "PartialOrd",
            "Ord",
            "Default",
            "Hash",
        ];
        for name in &builtin_trait_names {
            self.traits.entry(name.to_string()).or_default();
        }

        // Primitive types that implement all common traits
        let primitive_types = [
            "i8", "i16", "i32", "i64", "i128", "u8", "u16", "u32", "u64", "u128", "isize", "usize",
            "f32", "f64", "bool", "char",
        ];
        let all_traits = [
            "Display",
            "Debug",
            "Clone",
            "Copy",
            "PartialEq",
            "Eq",
            "PartialOrd",
            "Ord",
            "Default",
            "Hash",
        ];
        for ty in &primitive_types {
            for tr in &all_traits {
                self.trait_impls.insert((tr.to_string(), ty.to_string()));
            }
        }

        // String implements Display, Debug, Clone, PartialEq, Eq, Hash, Default
        for tr in &[
            "Display",
            "Debug",
            "Clone",
            "PartialEq",
            "Eq",
            "Hash",
            "Default",
        ] {
            self.trait_impls.insert((tr.to_string(), "str".to_string()));
            self.trait_impls
                .insert((tr.to_string(), "String".to_string()));
        }

        // Built-in Future<T> trait: fn poll(&mut self) -> Poll<T> (S4.2)
        self.traits.insert(
            "Future".to_string(),
            vec![TraitMethodSig {
                name: "poll".to_string(),
                param_types: vec![],
                ret_type: Type::Enum {
                    name: "Poll".to_string(),
                },
            }],
        );

        // Built-in Drop trait (already handled elsewhere, register name)
        self.traits.entry("Drop".to_string()).or_default();
    }

    /// Checks whether a concrete type satisfies a trait bound.
    #[allow(dead_code)]
    fn type_satisfies_trait(&self, type_name: &str, trait_name: &str) -> bool {
        self.trait_impls
            .contains(&(trait_name.to_string(), type_name.to_string()))
    }

    /// Pre-registers additional known names as `Type::Unknown` symbols.
    ///
    /// Used by REPL / `eval_source()` to prevent false "undefined variable" errors
    /// for names defined in prior evaluation rounds.
    pub fn register_known_names(&mut self, names: &[String]) {
        for name in names {
            if self.symbols.lookup(name).is_none() {
                self.symbols.define(Symbol {
                    name: name.clone(),
                    ty: Type::Unknown,
                    mutable: true,
                    span: Span::new(0, 0),
                    used: true, // don't warn about unused
                });
            }
        }
    }

    /// Analyzes a complete program.
    ///
    /// Returns `Ok(())` if no hard errors, or `Err(errors)` with all collected errors.
    /// Warnings (SE009, SE010) are included in errors but do not cause failure on their own.
    pub fn analyze(&mut self, program: &Program) -> Result<(), Vec<SemanticError>> {
        // First pass: register all top-level function and type definitions
        for item in &program.items {
            self.register_item(item);
        }

        // Second pass: check all items
        for item in &program.items {
            self.check_item(item);
        }

        // SE019: Check for unused imports
        for (import_name, import_span, _) in &self.imports {
            // Extract the short name (last segment) and check if used
            let short_name = import_name.rsplit("::").next().unwrap_or(import_name);
            if let Some(sym) = self.symbols.lookup(short_name) {
                if !sym.used {
                    self.errors.push(SemanticError::UnusedImport {
                        name: import_name.clone(),
                        span: *import_span,
                    });
                }
            }
        }

        let has_errors = self.errors.iter().any(|e| !e.is_warning());
        if has_errors {
            Err(self.errors.clone())
        } else {
            Ok(())
        }
    }

    /// Returns all warnings collected during analysis.
    pub fn warnings(&self) -> Vec<&SemanticError> {
        self.errors.iter().filter(|e| e.is_warning()).collect()
    }

    /// Returns all diagnostics (errors + warnings) collected during analysis.
    pub fn diagnostics(&self) -> &[SemanticError] {
        &self.errors
    }

    /// First pass: register top-level declarations.
    fn register_item(&mut self, item: &Item) {
        match item {
            Item::FnDef(fndef) => {
                // For generic functions, temporarily register type params so resolve_type works
                let generic_names: Vec<String> = fndef
                    .generic_params
                    .iter()
                    .map(|g| g.name.clone())
                    .collect();
                if !generic_names.is_empty() {
                    self.symbols.push_scope_kind(super::scope::ScopeKind::Block);
                    for gp in &fndef.generic_params {
                        self.symbols.define(Symbol {
                            name: gp.name.clone(),
                            ty: Type::TypeVar(gp.name.clone()),
                            mutable: false,
                            span: gp.span,
                            used: true,
                        });
                    }
                }

                let param_types: Vec<Type> = fndef
                    .params
                    .iter()
                    .map(|p| self.resolve_type(&p.ty))
                    .collect();
                let ret_type = fndef
                    .return_type
                    .as_ref()
                    .map(|t| self.resolve_type(t))
                    .unwrap_or(Type::Void);
                // async fn wraps return type in Future<T>
                let effective_ret = if fndef.is_async {
                    Type::Future {
                        inner: Box::new(ret_type),
                    }
                } else {
                    ret_type
                };
                let fn_type = Type::Function {
                    params: param_types,
                    ret: Box::new(effective_ret),
                };

                if !generic_names.is_empty() {
                    let _ = self.symbols.pop_scope_unused();
                }

                self.symbols.define(Symbol {
                    name: fndef.name.clone(),
                    ty: fn_type,
                    mutable: false,
                    span: fndef.span,
                    used: false,
                });
                // Track annotation context
                if let Some(ann) = &fndef.annotation {
                    match ann.name.as_str() {
                        "kernel" => {
                            self.kernel_fns.insert(fndef.name.clone());
                        }
                        "device" => {
                            self.device_fns.insert(fndef.name.clone());
                        }
                        "npu" => {
                            self.npu_fns.insert(fndef.name.clone());
                        }
                        _ => {}
                    }
                }
            }
            Item::StructDef(sdef) => {
                let mut fields = HashMap::new();
                for field in &sdef.fields {
                    fields.insert(field.name.clone(), self.resolve_type(&field.ty));
                }
                self.symbols.define(Symbol {
                    name: sdef.name.clone(),
                    ty: Type::Struct {
                        name: sdef.name.clone(),
                        fields,
                    },
                    mutable: false,
                    span: sdef.span,
                    used: false,
                });
            }
            Item::UnionDef(udef) => {
                let mut fields = HashMap::new();
                for field in &udef.fields {
                    fields.insert(field.name.clone(), self.resolve_type(&field.ty));
                }
                self.symbols.define(Symbol {
                    name: udef.name.clone(),
                    ty: Type::Struct {
                        name: udef.name.clone(),
                        fields,
                    },
                    mutable: false,
                    span: udef.span,
                    used: false,
                });
            }
            Item::EnumDef(edef) => {
                // For generic enums, temporarily register type params as Unknown
                let has_generics = !edef.generic_params.is_empty();
                if has_generics {
                    self.symbols.push_scope_kind(super::scope::ScopeKind::Block);
                    for gp in &edef.generic_params {
                        self.symbols.define(Symbol {
                            name: gp.name.clone(),
                            ty: Type::TypeVar(gp.name.clone()),
                            mutable: false,
                            span: gp.span,
                            used: true,
                        });
                    }
                }

                self.symbols.define(Symbol {
                    name: edef.name.clone(),
                    ty: Type::Enum {
                        name: edef.name.clone(),
                    },
                    mutable: false,
                    span: edef.span,
                    used: false,
                });
                // Track variant names for exhaustiveness checking
                let variant_names: Vec<String> =
                    edef.variants.iter().map(|v| v.name.clone()).collect();
                self.enum_variants.insert(edef.name.clone(), variant_names);
                // Register variants
                for variant in &edef.variants {
                    if variant.fields.is_empty() {
                        self.symbols.define(Symbol {
                            name: variant.name.clone(),
                            ty: Type::Enum {
                                name: edef.name.clone(),
                            },
                            mutable: false,
                            span: variant.span,
                            used: false,
                        });
                    } else {
                        let field_types: Vec<Type> = variant
                            .fields
                            .iter()
                            .map(|f| self.resolve_type(f))
                            .collect();
                        self.symbols.define(Symbol {
                            name: variant.name.clone(),
                            ty: Type::Function {
                                params: field_types,
                                ret: Box::new(Type::Enum {
                                    name: edef.name.clone(),
                                }),
                            },
                            mutable: false,
                            span: variant.span,
                            used: false,
                        });
                    }
                }

                if has_generics {
                    let _ = self.symbols.pop_scope_unused();
                }
            }
            Item::ConstDef(cdef) => {
                let ty = self.resolve_type(&cdef.ty);
                self.symbols.define(Symbol {
                    name: cdef.name.clone(),
                    ty,
                    mutable: false,
                    span: cdef.span,
                    used: false,
                });
            }
            Item::ImplBlock(impl_block) => {
                self.register_impl_block(impl_block);
            }
            Item::ModDecl(mod_decl) => {
                self.register_mod_decl(mod_decl);
            }
            Item::UseDecl(use_decl) => {
                self.register_use_decl(use_decl);
            }
            Item::TraitDef(tdef) => {
                self.register_trait_def(tdef);
            }
            Item::ExternFn(efn) => {
                self.register_extern_fn(efn);
            }
            Item::TypeAlias(ta) => {
                self.register_type_alias(ta);
            }
            _ => {}
        }
    }

    /// Registers a trait definition, storing its method signatures.
    fn register_trait_def(&mut self, tdef: &crate::parser::ast::TraitDef) {
        let mut method_sigs = Vec::new();
        let mut seen_methods = std::collections::HashSet::new();

        for method in &tdef.methods {
            if !seen_methods.insert(method.name.clone()) {
                self.errors.push(SemanticError::DuplicateDefinition {
                    name: method.name.clone(),
                    span: method.span,
                });
                continue;
            }

            let param_types: Vec<Type> = method
                .params
                .iter()
                .map(|p| self.resolve_type(&p.ty))
                .collect();
            let ret_type = method
                .return_type
                .as_ref()
                .map(|t| self.resolve_type(t))
                .unwrap_or(Type::Void);

            method_sigs.push(TraitMethodSig {
                name: method.name.clone(),
                param_types,
                ret_type,
            });
        }

        self.traits.insert(tdef.name.clone(), method_sigs);
    }

    /// Registers a type alias, resolving the target type.
    fn register_type_alias(&mut self, ta: &TypeAlias) {
        let resolved = self.resolve_type(&ta.ty);
        self.type_aliases.insert(ta.name.clone(), resolved);
    }

    /// Registers an extern function declaration in the symbol table.
    ///
    /// Validates that all parameter types and the return type are FFI-safe.
    /// FFI-safe types: bool, i8-i64, u8-u64, isize, usize, f32, f64, void.
    fn register_extern_fn(&mut self, efn: &ExternFn) {
        let param_types: Vec<Type> = efn
            .params
            .iter()
            .map(|p| self.resolve_type(&p.ty))
            .collect();
        let ret_type = efn
            .return_type
            .as_ref()
            .map(|t| self.resolve_type(t))
            .unwrap_or(Type::Void);

        // Validate FFI-safe types
        for (i, ty) in param_types.iter().enumerate() {
            if !self.is_ffi_safe(ty) {
                self.errors.push(SemanticError::FfiUnsafeType {
                    ty: format!("{:?}", ty),
                    func: efn.name.clone(),
                    span: efn.params[i].span,
                });
            }
        }
        if !self.is_ffi_safe(&ret_type) {
            self.errors.push(SemanticError::FfiUnsafeType {
                ty: format!("{:?}", ret_type),
                func: efn.name.clone(),
                span: efn.span,
            });
        }

        let fn_type = Type::Function {
            params: param_types,
            ret: Box::new(ret_type),
        };
        self.symbols.define(Symbol {
            name: efn.name.clone(),
            ty: fn_type,
            mutable: false,
            span: efn.span,
            used: false,
        });
    }

    /// Returns `true` if the type is FFI-safe (can cross the C ABI boundary).
    fn is_ffi_safe(&self, ty: &Type) -> bool {
        matches!(
            ty,
            Type::Void
                | Type::Bool
                | Type::I8
                | Type::I16
                | Type::I32
                | Type::I64
                | Type::U8
                | Type::U16
                | Type::U32
                | Type::U64
                | Type::ISize
                | Type::USize
                | Type::F32
                | Type::F64
                | Type::IntLiteral
                | Type::FloatLiteral
        )
    }

    /// Registers impl block methods in the symbol table.
    fn register_impl_block(&mut self, impl_block: &ImplBlock) {
        let mut impl_method_names: Vec<String> = Vec::new();

        for method in &impl_block.methods {
            impl_method_names.push(method.name.clone());

            // Validate `self` parameter placement
            for (i, param) in method.params.iter().enumerate() {
                if param.name == "self" && i != 0 {
                    self.errors.push(SemanticError::TypeMismatch {
                        expected: "self must be the first parameter".into(),
                        found: format!("self at position {}", i + 1),
                        span: param.span,
                        hint: None,
                    });
                }
            }

            let param_types: Vec<Type> = method
                .params
                .iter()
                .map(|p| {
                    if p.name == "self" {
                        let struct_ty = Type::Struct {
                            name: impl_block.target_type.clone(),
                            fields: HashMap::new(),
                        };
                        // Handle &self / &mut self sugar
                        match &p.ty {
                            crate::parser::ast::TypeExpr::Reference { mutable, .. } => {
                                if *mutable {
                                    Type::RefMut(Box::new(struct_ty))
                                } else {
                                    Type::Ref(Box::new(struct_ty))
                                }
                            }
                            _ => struct_ty,
                        }
                    } else {
                        self.resolve_type(&p.ty)
                    }
                })
                .collect();
            let ret_type = method
                .return_type
                .as_ref()
                .map(|t| self.resolve_type(t))
                .unwrap_or(Type::Void);

            // Register as qualified name: TypeName::method
            let qualified = format!("{}::{}", impl_block.target_type, method.name);
            self.symbols.define(Symbol {
                name: qualified,
                ty: Type::Function {
                    params: param_types,
                    ret: Box::new(ret_type),
                },
                mutable: false,
                span: method.span,
                used: false,
            });
        }

        // If this is a trait impl (`impl Trait for Type`), validate completeness + signatures
        if let Some(trait_name) = &impl_block.trait_name {
            if let Some(trait_methods) = self.traits.get(trait_name).cloned() {
                for tm in &trait_methods {
                    if !impl_method_names.contains(&tm.name) {
                        // Missing method
                        self.errors.push(SemanticError::MissingField {
                            struct_name: format!(
                                "impl {} for {}",
                                trait_name, impl_block.target_type
                            ),
                            field: tm.name.clone(),
                            span: impl_block.span,
                        });
                    } else {
                        // Method exists — verify signature matches
                        if let Some(impl_method) =
                            impl_block.methods.iter().find(|m| m.name == tm.name)
                        {
                            let impl_param_types: Vec<Type> = impl_method
                                .params
                                .iter()
                                .map(|p| {
                                    if p.name == "self" {
                                        Type::Struct {
                                            name: impl_block.target_type.clone(),
                                            fields: HashMap::new(),
                                        }
                                    } else {
                                        self.resolve_type(&p.ty)
                                    }
                                })
                                .collect();
                            let impl_ret = impl_method
                                .return_type
                                .as_ref()
                                .map(|t| self.resolve_type(t))
                                .unwrap_or(Type::Void);

                            // Check parameter count (excluding self for comparison)
                            let trait_non_self: Vec<&Type> = tm
                                .param_types
                                .iter()
                                .filter(|t| !matches!(t, Type::Unknown))
                                .collect();
                            let impl_non_self: Vec<&Type> = impl_param_types
                                .iter()
                                .filter(|t| {
                                    !matches!(t, Type::Struct { name, .. } if name == &impl_block.target_type)
                                })
                                .collect();

                            if impl_method.params.len() != tm.param_types.len() {
                                self.errors
                                    .push(SemanticError::TraitMethodSignatureMismatch {
                                        method: tm.name.clone(),
                                        trait_name: trait_name.clone(),
                                        target_type: impl_block.target_type.clone(),
                                        detail: format!(
                                            "expected {} parameters, found {}",
                                            tm.param_types.len(),
                                            impl_method.params.len()
                                        ),
                                        span: impl_method.span,
                                    });
                            } else {
                                // Check return type
                                if !types_compatible(&tm.ret_type, &impl_ret) {
                                    self.errors
                                        .push(SemanticError::TraitMethodSignatureMismatch {
                                            method: tm.name.clone(),
                                            trait_name: trait_name.clone(),
                                            target_type: impl_block.target_type.clone(),
                                            detail: format!(
                                                "expected return type {:?}, found {:?}",
                                                tm.ret_type, impl_ret
                                            ),
                                            span: impl_method.span,
                                        });
                                }

                                // Check non-self parameter types
                                for (i, (trait_t, impl_t)) in
                                    trait_non_self.iter().zip(impl_non_self.iter()).enumerate()
                                {
                                    if !types_compatible(trait_t, impl_t) {
                                        self.errors.push(
                                            SemanticError::TraitMethodSignatureMismatch {
                                                method: tm.name.clone(),
                                                trait_name: trait_name.clone(),
                                                target_type: impl_block.target_type.clone(),
                                                detail: format!(
                                                    "parameter {} has type {:?}, expected {:?}",
                                                    i + 1,
                                                    impl_t,
                                                    trait_t
                                                ),
                                                span: impl_method.span,
                                            },
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
                // Record that this type implements this trait
                self.trait_impls
                    .insert((trait_name.clone(), impl_block.target_type.clone()));
            }
            // If trait not found, it might just not be defined yet — no error
        }
    }

    /// Registers items inside a module declaration with qualified names.
    fn register_mod_decl(&mut self, mod_decl: &ModDecl) {
        self.register_mod_items(&mod_decl.name, &mod_decl.body);
    }

    /// Registers module items with a given prefix for qualified names.
    fn register_mod_items(&mut self, prefix: &str, body: &Option<Vec<Item>>) {
        if let Some(items) = body {
            for item in items {
                match item {
                    Item::FnDef(fndef) => {
                        let param_types: Vec<Type> = fndef
                            .params
                            .iter()
                            .map(|p| self.resolve_type(&p.ty))
                            .collect();
                        let ret_type = fndef
                            .return_type
                            .as_ref()
                            .map(|t| self.resolve_type(t))
                            .unwrap_or(Type::Void);
                        let qualified = format!("{}::{}", prefix, fndef.name);
                        self.symbols.define(Symbol {
                            name: qualified,
                            ty: Type::Function {
                                params: param_types,
                                ret: Box::new(ret_type),
                            },
                            mutable: false,
                            span: fndef.span,
                            used: false,
                        });
                    }
                    Item::ConstDef(cdef) => {
                        let ty = self.resolve_type(&cdef.ty);
                        let qualified = format!("{}::{}", prefix, cdef.name);
                        self.symbols.define(Symbol {
                            name: qualified,
                            ty,
                            mutable: false,
                            span: cdef.span,
                            used: false,
                        });
                    }
                    Item::ModDecl(inner) => {
                        // Nested module: register with outer::inner:: prefix
                        let nested_prefix = format!("{}::{}", prefix, inner.name);
                        self.register_mod_items(&nested_prefix, &inner.body);
                    }
                    _ => {
                        self.register_item(item);
                    }
                }
            }
        }
    }

    /// Registers use declarations by aliasing qualified names to short names.
    fn register_use_decl(&mut self, use_decl: &UseDecl) {
        let path = &use_decl.path;
        match &use_decl.kind {
            UseKind::Simple => {
                if path.len() >= 2 {
                    let mod_path = path[..path.len() - 1].join("::");
                    let item_name = &path[path.len() - 1];
                    let qualified = format!("{}::{}", mod_path, item_name);
                    if let Some(sym) = self.symbols.lookup(&qualified) {
                        self.symbols.define(Symbol {
                            name: item_name.clone(),
                            ty: sym.ty.clone(),
                            mutable: false,
                            span: use_decl.span,
                            used: false,
                        });
                    }
                    // Track for unused import detection
                    self.imports.push((path.join("::"), use_decl.span, false));
                }
            }
            UseKind::Glob => {
                // Glob import: find all symbols with the module prefix
                // and register them with their short names
                let mod_path = path.join("::");
                let symbols = self.symbols.find_with_prefix(&mod_path);
                let prefix_len = mod_path.len() + 2; // "mod::" prefix
                for sym in symbols {
                    if sym.name.len() > prefix_len {
                        let short_name = sym.name[prefix_len..].to_string();
                        // Only import direct children (no nested ::)
                        if !short_name.contains("::") {
                            self.symbols.define(Symbol {
                                name: short_name,
                                ty: sym.ty.clone(),
                                mutable: false,
                                span: use_decl.span,
                                used: false,
                            });
                        }
                    }
                }
            }
            UseKind::Group(names) => {
                let mod_path = path.join("::");
                for name in names {
                    let qualified = format!("{}::{}", mod_path, name);
                    if let Some(sym) = self.symbols.lookup(&qualified) {
                        self.symbols.define(Symbol {
                            name: name.clone(),
                            ty: sym.ty.clone(),
                            mutable: false,
                            span: use_decl.span,
                            used: false,
                        });
                    }
                }
            }
        }
    }

    /// Second pass: type-check an item.
    fn check_item(&mut self, item: &Item) {
        match item {
            Item::FnDef(fndef) => {
                // `self` parameter is not allowed in free functions
                for param in &fndef.params {
                    if param.name == "self" {
                        self.errors.push(SemanticError::TypeMismatch {
                            expected: "regular parameter".into(),
                            found: "`self` outside impl block".into(),
                            span: param.span,
                            hint: None,
                        });
                    }
                }
                self.check_fn_def(fndef);
            }
            Item::ConstDef(cdef) => self.check_const_def(cdef),
            Item::ImplBlock(impl_block) => {
                for method in &impl_block.methods {
                    self.check_fn_def(method);
                }
            }
            Item::ModDecl(mod_decl) => {
                if let Some(items) = &mod_decl.body {
                    for item in items {
                        self.check_item(item);
                    }
                }
            }
            Item::Stmt(stmt) => {
                self.check_stmt(stmt);
            }
            _ => {}
        }
    }

    /// Checks a function definition.
    fn check_fn_def(&mut self, fndef: &FnDef) {
        let scope_kind = match &fndef.annotation {
            Some(ann) if ann.name == "kernel" => super::scope::ScopeKind::Kernel,
            Some(ann) if ann.name == "device" => super::scope::ScopeKind::Device,
            Some(ann) if ann.name == "npu" => super::scope::ScopeKind::Npu,
            Some(ann) if ann.name == "unsafe" => super::scope::ScopeKind::Unsafe,
            _ if fndef.is_async => super::scope::ScopeKind::AsyncFn,
            _ => super::scope::ScopeKind::Function,
        };
        self.symbols.push_scope_kind(scope_kind);

        // Register generic type parameters as TypeVar in scope.
        // This allows `T` to be resolved as a type variable in parameter and return positions.
        // Also validate that trait bounds reference known traits.
        for gp in &fndef.generic_params {
            for bound in &gp.bounds {
                if !self.traits.contains_key(&bound.name) {
                    self.errors.push(SemanticError::UnknownTrait {
                        name: bound.name.clone(),
                        span: bound.span,
                    });
                }
            }
            self.symbols.define(Symbol {
                name: gp.name.clone(),
                ty: Type::TypeVar(gp.name.clone()),
                mutable: false,
                span: gp.span,
                used: true, // type params are always "used"
            });
        }

        // Check lifetime annotations (always run — also catches undeclared lifetimes)
        self.check_lifetime_elision(fndef);

        // Validate where clause bounds reference known traits.
        for wc in &fndef.where_clauses {
            for bound in &wc.bounds {
                if !self.traits.contains_key(&bound.name) {
                    self.errors.push(SemanticError::UnknownTrait {
                        name: bound.name.clone(),
                        span: bound.span,
                    });
                }
            }
        }

        // Define parameters in function scope
        for param in &fndef.params {
            let ty = self.resolve_type(&param.ty);
            self.symbols.define(Symbol {
                name: param.name.clone(),
                ty,
                mutable: false,
                span: param.span,
                used: false,
            });
        }

        // Compute NLL liveness info for this function body
        let outer_nll = self.nll_info.take();
        self.nll_info = Some(super::cfg::NllInfo::analyze(&fndef.body));

        let body_type = self.check_expr(&fndef.body);

        let declared_ret = fndef
            .return_type
            .as_ref()
            .map(|t| self.resolve_type(t))
            .unwrap_or(Type::Void);

        // Check return type compatibility
        if !declared_ret.is_compatible(&body_type)
            && !matches!(declared_ret, Type::Void)
            && !matches!(body_type, Type::Void | Type::Never)
        {
            self.errors.push(SemanticError::TypeMismatch {
                expected: declared_ret.display_name(),
                found: body_type.display_name(),
                span: fndef.span,
                hint: None,
            });
        }

        // Restore outer NLL info (for nested functions)
        self.nll_info = outer_nll;

        self.emit_unused_warnings();
    }

    /// Pops the current scope and emits SE009 warnings for unused variables.
    fn emit_unused_warnings(&mut self) {
        let unused = self.symbols.pop_scope_unused();
        for sym in unused {
            self.errors.push(SemanticError::UnusedVariable {
                name: sym.name,
                span: sym.span,
            });
        }
    }

    /// Releases borrows whose binding variable is no longer live (NLL).
    ///
    /// Called before each statement in a block. Checks all active borrow
    /// bindings against the NLL liveness info. If a borrow binding's last
    /// use position is before `current_pos`, its borrow is released early.
    fn release_dead_borrows_nll(&mut self, current_pos: usize) {
        if let Some(nll) = &self.nll_info {
            let active_refs = self.moves.active_borrow_refs();
            let mut to_release = Vec::new();
            for ref_name in active_refs {
                if !nll.is_live_at(&ref_name, current_pos) {
                    to_release.push(ref_name);
                }
            }
            for ref_name in to_release {
                self.moves.release_borrow_by_ref(&ref_name);
            }
        }
    }

    /// Checks a const definition.
    fn check_const_def(&mut self, cdef: &ConstDef) {
        let val_type = self.check_expr(&cdef.value);
        let declared = self.resolve_type(&cdef.ty);
        if !declared.is_compatible(&val_type) {
            self.errors.push(SemanticError::TypeMismatch {
                expected: declared.display_name(),
                found: val_type.display_name(),
                span: cdef.span,
                hint: None,
            });
        }
    }

    /// Checks a statement, returns the type of its value (Void for most stmts).
    fn check_stmt(&mut self, stmt: &Stmt) -> Type {
        match stmt {
            Stmt::Let {
                mutable,
                name,
                ty,
                value,
                span,
            } => {
                let val_type = self.check_expr(value);

                if let Some(ty_expr) = ty {
                    let declared = self.resolve_type(ty_expr);
                    if !declared.is_compatible(&val_type) {
                        let hint =
                            type_mismatch_hint(&declared.display_name(), &val_type.display_name());
                        self.errors.push(SemanticError::TypeMismatch {
                            expected: declared.display_name(),
                            found: val_type.display_name(),
                            span: *span,
                            hint,
                        });
                    }
                    self.symbols.define(Symbol {
                        name: name.clone(),
                        ty: declared,
                        mutable: *mutable,
                        span: *span,
                        used: false,
                    });
                } else {
                    // No explicit type annotation: default unsuffixed literals
                    // (IntLiteral → i64, FloatLiteral → f64)
                    self.symbols.define(Symbol {
                        name: name.clone(),
                        ty: val_type.default_literal(),
                        mutable: *mutable,
                        span: *span,
                        used: false,
                    });
                }

                // Move tracking: if RHS is a variable of Move type, mark it moved
                if let Expr::Ident {
                    name: src_name,
                    span: src_span,
                } = &**value
                {
                    let src_type = self
                        .symbols
                        .lookup(src_name)
                        .map(|s| s.ty.clone())
                        .unwrap_or(Type::Unknown);
                    if !super::borrow_lite::is_copy_type(&src_type) {
                        // ME003: cannot move while borrowed
                        if let Some(borrow_span) = self.moves.check_can_move(src_name) {
                            self.errors.push(SemanticError::MoveWhileBorrowed {
                                name: src_name.clone(),
                                span: *src_span,
                                borrow_span,
                            });
                        }
                        self.moves.mark_moved(src_name, *src_span);
                    }
                }
                // Declare new binding as Owned
                self.moves.declare(name, *span);

                // Track borrow refs: if RHS is &target or &mut target,
                // register that this binding holds a borrow.
                if let Expr::Unary { op, operand, .. } = &**value {
                    if matches!(op, UnaryOp::Ref | UnaryOp::RefMut) {
                        if let Expr::Ident {
                            name: target_name, ..
                        } = &**operand
                        {
                            self.moves.register_borrow_ref(
                                name,
                                target_name,
                                matches!(op, UnaryOp::RefMut),
                            );
                        }
                    }
                }

                Type::Void
            }
            Stmt::Const {
                name,
                ty,
                value,
                span,
            } => {
                let val_type = self.check_expr(value);
                let declared = self.resolve_type(ty);
                if !declared.is_compatible(&val_type) {
                    self.errors.push(SemanticError::TypeMismatch {
                        expected: declared.display_name(),
                        found: val_type.display_name(),
                        span: *span,
                        hint: None,
                    });
                }
                self.symbols.define(Symbol {
                    name: name.clone(),
                    ty: declared,
                    mutable: false,
                    span: *span,
                    used: false,
                });
                Type::Void
            }
            Stmt::Expr { expr, .. } => self.check_expr(expr),
            Stmt::Return { value, span } => {
                if !self.symbols.is_inside_function() {
                    self.errors
                        .push(SemanticError::ReturnOutsideFunction { span: *span });
                }
                if let Some(v) = value {
                    self.check_expr(v)
                } else {
                    Type::Void
                }
            }
            Stmt::Break { value, span } => {
                if !self.symbols.is_inside_loop() {
                    self.errors
                        .push(SemanticError::BreakOutsideLoop { span: *span });
                }
                if let Some(v) = value {
                    self.check_expr(v)
                } else {
                    Type::Void
                }
            }
            Stmt::Continue { span } => {
                if !self.symbols.is_inside_loop() {
                    self.errors
                        .push(SemanticError::BreakOutsideLoop { span: *span });
                }
                Type::Void
            }
            Stmt::Item(item) => {
                self.register_item(item);
                self.check_item(item);
                Type::Void
            }
        }
    }

    /// Type-checks an expression and returns its inferred type.
    pub fn check_expr(&mut self, expr: &Expr) -> Type {
        match expr {
            Expr::Literal { kind, .. } => self.check_literal(kind),
            Expr::Ident { name, span } => self.check_ident(name, *span),
            Expr::Binary {
                left, op, right, ..
            } => self.check_binary(left, *op, right),
            Expr::Unary { op, operand, .. } => self.check_unary(*op, operand),
            Expr::Call {
                callee, args, span, ..
            } => self.check_call(callee, args, *span),
            Expr::Block { stmts, expr, .. } => self.check_block(stmts, expr),
            Expr::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => self.check_if(condition, then_branch, else_branch),
            Expr::While {
                condition, body, ..
            } => {
                let cond_ty = self.check_expr(condition);
                if !cond_ty.is_compatible(&Type::Bool) && !cond_ty.is_integer() {
                    self.errors.push(SemanticError::TypeMismatch {
                        expected: "bool".into(),
                        found: cond_ty.display_name(),
                        span: condition.span(),
                        hint: None,
                    });
                }
                self.symbols.push_scope_kind(super::scope::ScopeKind::Loop);
                self.check_expr(body);
                self.symbols.pop_scope();
                Type::Void
            }
            Expr::For {
                variable,
                iterable,
                body,
                ..
            } => {
                let iter_ty = self.check_expr(iterable);
                let elem_ty = match &iter_ty {
                    Type::Array(inner) => *inner.clone(),
                    Type::Str => Type::Char,
                    _ => Type::Unknown,
                };
                self.symbols.push_scope_kind(super::scope::ScopeKind::Loop);
                self.symbols.define(Symbol {
                    name: variable.clone(),
                    ty: elem_ty,
                    mutable: false,
                    span: iterable.span(),
                    used: false,
                });
                self.check_expr(body);
                self.symbols.pop_scope();
                Type::Void
            }
            Expr::Loop { body, .. } => {
                self.symbols.push_scope_kind(super::scope::ScopeKind::Loop);
                self.check_expr(body);
                self.symbols.pop_scope();
                Type::Void
            }
            Expr::Assign {
                target,
                op,
                value,
                span,
            } => self.check_assign(target, *op, value, *span),
            Expr::Match {
                subject,
                arms,
                span,
            } => self.check_match(subject, arms, *span),
            Expr::Array { elements, span } => self.check_array(elements, *span),
            Expr::Tuple { elements, .. } => {
                let types: Vec<Type> = elements.iter().map(|e| self.check_expr(e)).collect();
                Type::Tuple(types)
            }
            Expr::Pipe { left, right, span } => self.check_pipe(left, right, *span),
            Expr::StructInit {
                name, fields, span, ..
            } => self.check_struct_init(name, fields, *span),
            Expr::Field {
                object,
                field,
                span,
            } => self.check_field(object, field, *span),
            Expr::Index {
                object,
                index,
                span,
            } => self.check_index(object, index, *span),
            Expr::Range { start, end, .. } => {
                let mut range_ty = Type::I64; // default if no bounds
                if let Some(s) = start {
                    let st = self.check_expr(s);
                    if !st.is_integer() && !matches!(st, Type::Unknown) {
                        self.errors.push(SemanticError::TypeMismatch {
                            expected: "integer".into(),
                            found: st.display_name(),
                            span: s.span(),
                            hint: None,
                        });
                    }
                    range_ty = st;
                }
                if let Some(e) = end {
                    let et = self.check_expr(e);
                    if !et.is_integer() && !matches!(et, Type::Unknown) {
                        self.errors.push(SemanticError::TypeMismatch {
                            expected: "integer".into(),
                            found: et.display_name(),
                            span: e.span(),
                            hint: None,
                        });
                    }
                }
                Type::Array(Box::new(range_ty))
            }
            Expr::Grouped { expr, .. } => self.check_expr(expr),
            Expr::Closure { params, body, .. } => {
                self.symbols
                    .push_scope_kind(super::scope::ScopeKind::Function);
                let param_types: Vec<Type> = params
                    .iter()
                    .map(|cp| {
                        let ty = cp
                            .ty
                            .as_ref()
                            .map(|t| self.resolve_type(t))
                            .unwrap_or(Type::Unknown);
                        self.symbols.define(Symbol {
                            name: cp.name.clone(),
                            ty: ty.clone(),
                            mutable: false,
                            span: cp.span,
                            used: false,
                        });
                        ty
                    })
                    .collect();
                let ret_type = self.check_expr(body);
                self.symbols.pop_scope();
                Type::Function {
                    params: param_types,
                    ret: Box::new(ret_type),
                }
            }
            Expr::Path { segments, span } => {
                // Try full qualified name first (e.g., "math::square")
                let qualified = segments.join("::");
                if self.symbols.lookup(&qualified).is_some() {
                    return self.check_ident(&qualified, *span);
                }
                // Fall back to last segment only (e.g., "square")
                let name = segments.last().map_or("", |s| s.as_str());
                self.check_ident(name, *span)
            }
            Expr::MethodCall {
                receiver,
                method,
                args,
                span,
            } => self.check_method_call(receiver, method, args, *span),
            Expr::Cast {
                expr: inner,
                ty: target_ty,
                span,
            } => {
                let src = self.check_expr(inner);
                let target = self.resolve_type(target_ty);

                // Validate cast compatibility: numeric↔numeric, bool↔int OK
                let src_numeric = src.is_numeric() || src == Type::Bool || src == Type::Unknown;
                let tgt_numeric =
                    target.is_numeric() || target == Type::Bool || target == Type::Unknown;

                if !src_numeric || !tgt_numeric {
                    self.errors.push(SemanticError::TypeMismatch {
                        expected: format!(
                            "numeric type for cast, got {} as {}",
                            src.display_name(),
                            target.display_name()
                        ),
                        found: src.display_name(),
                        span: *span,
                        hint: None,
                    });
                }
                target
            }
            Expr::Try { .. } => Type::Unknown,
            Expr::Await { expr, span } => {
                if !self.symbols.is_inside_async() {
                    self.errors
                        .push(SemanticError::AwaitOutsideAsync { span: *span });
                }
                // Unwrap Future<T> → T
                let inner_ty = self.check_expr(expr);
                match inner_ty {
                    Type::Future { inner } => *inner,
                    _ => Type::Unknown,
                }
            }
            Expr::AsyncBlock { body, .. } => {
                // async { body } is effectively an async context
                self.symbols
                    .push_scope_kind(super::scope::ScopeKind::AsyncFn);
                let inner_ty = self.check_expr(body);
                let _ = self.symbols.pop_scope_unused();
                Type::Future {
                    inner: Box::new(inner_ty),
                }
            }
            Expr::FString { parts, .. } => {
                for part in parts {
                    if let FStringExprPart::Expr(expr) = part {
                        self.check_expr(expr);
                    }
                }
                Type::Str
            }
            Expr::InlineAsm { span, .. } => {
                // KE005/KE006: asm! only allowed in @kernel or @unsafe context
                let in_kernel = self.symbols.is_inside_kernel();
                let in_unsafe = self.symbols.is_inside_unsafe();
                if !in_kernel && !in_unsafe {
                    let in_device = self.symbols.is_inside_device();
                    if in_device {
                        self.errors
                            .push(SemanticError::AsmInDeviceContext { span: *span });
                    } else {
                        self.errors
                            .push(SemanticError::AsmInSafeContext { span: *span });
                    }
                }
                Type::Unknown
            }
        }
    }

    // ── Expression checkers ──

    /// Returns the type of a literal.
    fn check_literal(&self, kind: &LiteralKind) -> Type {
        match kind {
            LiteralKind::Int(_) => Type::IntLiteral,
            LiteralKind::Float(_) => Type::FloatLiteral,
            LiteralKind::String(_) | LiteralKind::RawString(_) => Type::Str,
            LiteralKind::Char(_) => Type::Char,
            LiteralKind::Bool(_) => Type::Bool,
            LiteralKind::Null => Type::Void,
        }
    }

    /// Looks up an identifier in the symbol table.
    fn check_ident(&mut self, name: &str, span: Span) -> Type {
        // Check for use-after-move
        if let Some(move_span) = self.moves.check_use(name) {
            self.errors.push(SemanticError::UseAfterMove {
                name: name.to_string(),
                span,
                move_span,
            });
        }
        match self.symbols.lookup(name) {
            Some(sym) => {
                let ty = sym.ty.clone();
                self.symbols.mark_used(name);
                ty
            }
            None => {
                let suggestion = suggest_similar(name, &self.symbols.all_names());
                self.errors.push(SemanticError::UndefinedVariable {
                    name: name.to_string(),
                    span,
                    suggestion,
                });
                Type::Unknown
            }
        }
    }

    /// Checks a binary expression.
    fn check_binary(&mut self, left: &Expr, op: BinOp, right: &Expr) -> Type {
        let lt = self.check_expr(left);
        let rt = self.check_expr(right);

        match op {
            // Arithmetic: both sides must be the same numeric type (no implicit promotion)
            BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Rem | BinOp::Pow => {
                // String concatenation with +
                if op == BinOp::Add && lt == Type::Str && rt == Type::Str {
                    return Type::Str;
                }
                // Tensor elementwise ops: shape check when both have known dims
                if lt.is_tensor() && rt.is_tensor() {
                    match lt.elementwise_shape(&rt) {
                        Some(result) => return result,
                        None => {
                            let op_span = Span::new(left.span().start, right.span().end);
                            self.errors.push(SemanticError::TensorShapeMismatch {
                                detail: format!(
                                    "elementwise: {} and {}",
                                    lt.display_name(),
                                    rt.display_name()
                                ),
                                span: op_span,
                            });
                            return Type::dynamic_tensor();
                        }
                    }
                }
                if lt.is_numeric() && rt.is_numeric() {
                    if lt.is_compatible(&rt) {
                        // Resolve to the concrete type when mixing literal with concrete
                        lt.resolve_with(&rt)
                    } else {
                        let hint = type_mismatch_hint(&lt.display_name(), &rt.display_name());
                        self.errors.push(SemanticError::TypeMismatch {
                            expected: lt.display_name(),
                            found: rt.display_name(),
                            span: right.span(),
                            hint,
                        });
                        Type::Unknown
                    }
                } else if matches!(lt, Type::Unknown | Type::TypeVar(_))
                    || matches!(rt, Type::Unknown | Type::TypeVar(_))
                {
                    // TypeVar: inside generic fn body, defer check to call site
                    if matches!(lt, Type::TypeVar(_)) {
                        lt
                    } else if matches!(rt, Type::TypeVar(_)) {
                        rt
                    } else {
                        Type::Unknown
                    }
                } else {
                    self.errors.push(SemanticError::TypeMismatch {
                        expected: "numeric".into(),
                        found: format!("{} and {}", lt.display_name(), rt.display_name()),
                        span: left.span(),
                        hint: None,
                    });
                    Type::Unknown
                }
            }
            // Comparison: both sides same type, result is bool
            BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => {
                if !lt.is_compatible(&rt) {
                    self.errors.push(SemanticError::TypeMismatch {
                        expected: lt.display_name(),
                        found: rt.display_name(),
                        span: right.span(),
                        hint: None,
                    });
                }
                Type::Bool
            }
            // Logical: both bool, result bool
            BinOp::And | BinOp::Or => Type::Bool,
            // Bitwise: both same integer type, result same integer type
            BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor | BinOp::Shl | BinOp::Shr => {
                if lt.is_integer() && rt.is_integer() && lt.is_compatible(&rt) {
                    lt.resolve_with(&rt)
                } else if lt.is_integer() && rt.is_integer() {
                    self.errors.push(SemanticError::TypeMismatch {
                        expected: lt.display_name(),
                        found: rt.display_name(),
                        span: right.span(),
                        hint: None,
                    });
                    Type::Unknown
                } else if matches!(lt, Type::Unknown | Type::TypeVar(_))
                    || matches!(rt, Type::Unknown | Type::TypeVar(_))
                {
                    if matches!(lt, Type::TypeVar(_)) {
                        lt
                    } else if matches!(rt, Type::TypeVar(_)) {
                        rt
                    } else {
                        Type::Unknown
                    }
                } else {
                    self.errors.push(SemanticError::TypeMismatch {
                        expected: "i64".into(),
                        found: format!("{} and {}", lt.display_name(), rt.display_name()),
                        span: left.span(),
                        hint: None,
                    });
                    Type::Unknown
                }
            }
            BinOp::MatMul => {
                // Shape checking for @ operator when both operands have tensor types
                if lt.is_tensor() && rt.is_tensor() {
                    // Skip shape check when either has unknown rank (empty dims)
                    let lt_empty = matches!(&lt, Type::Tensor { dims, .. } if dims.is_empty());
                    let rt_empty = matches!(&rt, Type::Tensor { dims, .. } if dims.is_empty());
                    if lt_empty || rt_empty {
                        return Type::dynamic_tensor();
                    }
                    match lt.matmul_shape(&rt) {
                        Some(result) => result,
                        None => {
                            let op_span = Span::new(left.span().start, right.span().end);
                            self.errors.push(SemanticError::TensorShapeMismatch {
                                detail: format!(
                                    "matmul: {} @ {}",
                                    lt.display_name(),
                                    rt.display_name()
                                ),
                                span: op_span,
                            });
                            Type::dynamic_tensor()
                        }
                    }
                } else {
                    Type::dynamic_tensor()
                }
            }
        }
    }

    /// Checks a unary expression.
    fn check_unary(&mut self, op: UnaryOp, operand: &Expr) -> Type {
        let ty = self.check_expr(operand);
        match op {
            UnaryOp::Neg => {
                if ty.is_numeric() || matches!(ty, Type::Unknown | Type::TypeVar(_)) {
                    ty
                } else {
                    self.errors.push(SemanticError::TypeMismatch {
                        expected: "numeric".into(),
                        found: ty.display_name(),
                        span: operand.span(),
                        hint: None,
                    });
                    Type::Unknown
                }
            }
            UnaryOp::Not => Type::Bool,
            UnaryOp::BitNot => {
                if ty.is_integer() || matches!(ty, Type::Unknown | Type::TypeVar(_)) {
                    ty
                } else {
                    self.errors.push(SemanticError::TypeMismatch {
                        expected: "integer".into(),
                        found: ty.display_name(),
                        span: operand.span(),
                        hint: None,
                    });
                    Type::Unknown
                }
            }
            UnaryOp::Ref => self.check_borrow_ref(operand, &ty, false),
            UnaryOp::RefMut => self.check_borrow_ref(operand, &ty, true),
            UnaryOp::Deref => match &ty {
                Type::Ref(inner) | Type::RefMut(inner) => *inner.clone(),
                Type::Unknown => Type::Unknown,
                _ => {
                    self.errors.push(SemanticError::TypeMismatch {
                        expected: "reference type".into(),
                        found: ty.display_name(),
                        span: operand.span(),
                        hint: None,
                    });
                    Type::Unknown
                }
            },
        }
    }

    /// Checks a borrow expression (`&x` or `&mut x`).
    fn check_borrow_ref(&mut self, operand: &Expr, ty: &Type, is_mutable: bool) -> Type {
        let span = operand.span();

        // Extract the target variable name (only idents supported)
        if let Expr::Ident { name, .. } = operand {
            if is_mutable {
                // Check variable is declared as `mut`
                if let Some(sym) = self.symbols.lookup(name) {
                    if !sym.mutable {
                        self.errors.push(SemanticError::TypeMismatch {
                            expected: "mutable variable for &mut".into(),
                            found: format!("immutable variable '{name}'"),
                            span,
                            hint: None,
                        });
                    }
                }
                // Try to create mutable borrow
                if let Err(err) = self.moves.borrow_mut(name, span) {
                    match err {
                        super::borrow_lite::BorrowError::MutWhileImmBorrowed { imm_span } => {
                            self.errors.push(SemanticError::MutBorrowConflict {
                                name: name.clone(),
                                span,
                                borrow_span: imm_span,
                            });
                        }
                        super::borrow_lite::BorrowError::DoubleMutBorrow { existing_span } => {
                            self.errors.push(SemanticError::MutBorrowConflict {
                                name: name.clone(),
                                span,
                                borrow_span: existing_span,
                            });
                        }
                        _ => {}
                    }
                }
                Type::RefMut(Box::new(ty.clone()))
            } else {
                // Try to create immutable borrow
                if let Err(super::borrow_lite::BorrowError::ImmWhileMutBorrowed { mut_span }) =
                    self.moves.borrow_imm(name, span)
                {
                    self.errors.push(SemanticError::ImmBorrowConflict {
                        name: name.clone(),
                        span,
                        borrow_span: mut_span,
                    });
                }
                Type::Ref(Box::new(ty.clone()))
            }
        } else {
            // Non-identifier operand (e.g., &expr) — no borrow tracking
            if is_mutable {
                Type::RefMut(Box::new(ty.clone()))
            } else {
                Type::Ref(Box::new(ty.clone()))
            }
        }
    }

    /// Checks a function call.
    fn check_call(&mut self, callee: &Expr, args: &[CallArg], span: Span) -> Type {
        // Context checks for @kernel/@device isolation
        if let Expr::Ident { name, .. } = callee {
            self.check_context_call(name, span);
        }
        // Also check path-based calls (e.g., module::func)
        if let Expr::Path { segments, .. } = callee {
            if let Some(last) = segments.last() {
                self.check_context_call(last, span);
            }
        }

        let callee_ty = self.check_expr(callee);

        // Evaluate argument types
        let arg_types: Vec<Type> = args.iter().map(|a| self.check_expr(&a.value)).collect();

        // SE018: thread::spawn Send check — all arguments must be Send
        if let Expr::Path { segments, .. } = callee {
            if segments.len() == 2 && segments[0] == "thread" && segments[1] == "spawn" {
                // Skip the first arg (function pointer — always Send)
                for (i, arg_ty) in arg_types.iter().enumerate().skip(1) {
                    if !arg_ty.is_send() {
                        self.errors.push(SemanticError::NotSendType {
                            ty: arg_ty.display_name(),
                            span: args[i].value.span(),
                        });
                    }
                }
            }
        }

        // Move tracking: if a move-type variable is passed as arg, mark moved
        // Exempt non-consuming builtins that logically take &T (read-only inspection)
        let is_non_consuming_builtin = if let Expr::Ident { name, .. } = callee {
            matches!(
                name.as_str(),
                "len" | "type_of" | "println" | "print" | "dbg" | "assert" | "assert_eq"
            ) || name.starts_with("tensor_")
        } else {
            false
        };
        if !is_non_consuming_builtin {
            for (arg, arg_ty) in args.iter().zip(arg_types.iter()) {
                if let Expr::Ident {
                    name: arg_name,
                    span: arg_span,
                } = &arg.value
                {
                    if !super::borrow_lite::is_copy_type(arg_ty) {
                        if let Some(borrow_span) = self.moves.check_can_move(arg_name) {
                            self.errors.push(SemanticError::MoveWhileBorrowed {
                                name: arg_name.clone(),
                                span: *arg_span,
                                borrow_span,
                            });
                        }
                        self.moves.mark_moved(arg_name, *arg_span);
                    }
                }
            }
        }

        // Check if any args are named — if so, skip positional type checking
        // because the analyzer doesn't have parameter names to reorder against
        let has_named_args = args.iter().any(|a| a.name.is_some());

        match callee_ty {
            Type::Function { params, ret } => {
                // Check arity (skip for variadic builtins with Unknown params)
                let is_variadic =
                    params.len() == 1 && matches!(params.first(), Some(Type::Unknown));
                if !is_variadic && params.len() != arg_types.len() {
                    self.errors.push(SemanticError::ArgumentCountMismatch {
                        expected: params.len(),
                        found: arg_types.len(),
                        span,
                    });
                }

                // Generic function inference: if params contain TypeVars, use unification
                let is_generic = params.iter().any(super::inference::has_type_vars)
                    || super::inference::has_type_vars(&ret);

                if is_generic && !is_variadic && !has_named_args {
                    let generic_names = super::inference::extract_generic_names(&Type::Function {
                        params: params.clone(),
                        ret: ret.clone(),
                    });
                    match super::inference::infer_type_args(
                        &params,
                        &arg_types,
                        &generic_names,
                        span,
                    ) {
                        Ok(subst) => {
                            // Apply substitution to return type
                            subst.apply(&ret)
                        }
                        Err(err) => match *err {
                            super::inference::InferError::UnificationFailed {
                                expected,
                                found,
                                span: err_span,
                                ..
                            } => {
                                self.errors.push(SemanticError::TypeMismatch {
                                    expected: expected.display_name(),
                                    found: found.display_name(),
                                    span: err_span,
                                    hint: None,
                                });
                                Type::Unknown
                            }
                            super::inference::InferError::Unbound {
                                param,
                                span: err_span,
                            } => {
                                self.errors.push(SemanticError::CannotInferType {
                                    param,
                                    reason: "type parameter not used in function arguments".into(),
                                    span: err_span,
                                });
                                *ret
                            }
                        },
                    }
                } else {
                    // Non-generic: check argument types directly (skip Unknown params and named args)
                    if !is_variadic && !has_named_args {
                        for (i, (expected, found)) in
                            params.iter().zip(arg_types.iter()).enumerate()
                        {
                            if !expected.is_compatible(found) {
                                self.errors.push(SemanticError::TypeMismatch {
                                    expected: expected.display_name(),
                                    found: found.display_name(),
                                    span: args.get(i).map_or(span, |a| a.span),
                                    hint: None,
                                });
                            }
                        }
                    }
                    *ret
                }
            }
            Type::Unknown => Type::Unknown,
            _ => {
                let suggestion =
                    suggest_similar(&format!("{callee_ty}"), &self.symbols.all_names());
                self.errors.push(SemanticError::UndefinedFunction {
                    name: format!("{callee_ty}"),
                    span,
                    suggestion,
                });
                Type::Unknown
            }
        }
    }

    /// Checks context isolation rules for a function call.
    fn check_context_call(&mut self, callee_name: &str, span: Span) {
        let in_kernel = self.symbols.is_inside_kernel();
        let in_device = self.symbols.is_inside_device();

        if in_kernel {
            // @kernel cannot call @device functions
            if self.device_fns.contains(callee_name) {
                self.errors.push(SemanticError::DeviceCallInKernel { span });
            }
            // KE001: @kernel cannot use heap-allocating builtins
            if self.heap_builtins.contains(callee_name) {
                self.errors.push(SemanticError::HeapAllocInKernel { span });
            }
            // KE002: @kernel cannot use tensor/ML builtins
            if self.tensor_builtins.contains(callee_name) {
                self.errors.push(SemanticError::TensorInKernel { span });
            }
        }

        if in_device {
            // @device cannot call @kernel functions
            if self.kernel_fns.contains(callee_name) {
                self.errors.push(SemanticError::KernelCallInDevice { span });
            }
            // @device cannot use OS builtins (raw pointer operations)
            if self.os_builtins.contains(callee_name) {
                self.errors.push(SemanticError::RawPointerInDevice { span });
            }
        }

        let in_npu = self.symbols.is_inside_npu();
        if in_npu {
            // NE001: @npu cannot use raw pointer/OS builtins
            if self.os_builtins.contains(callee_name) {
                self.errors.push(SemanticError::RawPointerInNpu { span });
            }
            // NE002: @npu cannot use heap-allocating builtins
            if self.heap_builtins.contains(callee_name) {
                self.errors.push(SemanticError::HeapAllocInNpu { span });
            }
            // NE003: @npu cannot use OS primitives
            if self.os_builtins.contains(callee_name) {
                self.errors.push(SemanticError::OsPrimitiveInNpu { span });
            }
            // NE004: @npu cannot call @kernel functions
            if self.kernel_fns.contains(callee_name) {
                self.errors.push(SemanticError::KernelCallInNpu { span });
            }
        }

        if in_kernel {
            // @kernel cannot call @npu functions
            if self.npu_fns.contains(callee_name) {
                self.errors.push(SemanticError::DeviceCallInKernel { span });
            }
        }
    }

    /// Returns the span start of a statement (for NLL ordering).
    fn stmt_span_start(stmt: &Stmt) -> usize {
        match stmt {
            Stmt::Let { span, .. }
            | Stmt::Const { span, .. }
            | Stmt::Expr { span, .. }
            | Stmt::Return { span, .. }
            | Stmt::Break { span, .. }
            | Stmt::Continue { span, .. } => span.start,
            Stmt::Item(_) => 0,
        }
    }

    /// Checks a block expression.
    fn check_block(&mut self, stmts: &[Stmt], tail: &Option<Box<Expr>>) -> Type {
        self.symbols.push_scope();
        self.moves.push_scope();
        let mut diverged = false;
        for stmt in stmts {
            if diverged {
                // SE010: Unreachable code after return/break/continue
                let span = match stmt {
                    Stmt::Let { span, .. }
                    | Stmt::Const { span, .. }
                    | Stmt::Expr { span, .. }
                    | Stmt::Return { span, .. }
                    | Stmt::Break { span, .. }
                    | Stmt::Continue { span, .. } => *span,
                    Stmt::Item(_) => Span::new(0, 0),
                };
                if span.start != 0 || span.end != 0 {
                    self.errors.push(SemanticError::UnreachableCode { span });
                }
                break;
            }
            // NLL: release borrows whose binding is no longer live
            self.release_dead_borrows_nll(Self::stmt_span_start(stmt));
            let stmt_ty = self.check_stmt(stmt);
            // Check if this statement diverges
            match stmt {
                Stmt::Return { .. } | Stmt::Break { .. } | Stmt::Continue { .. } => {
                    diverged = true;
                }
                Stmt::Expr { .. } => {
                    if matches!(stmt_ty, Type::Never) {
                        diverged = true;
                    }
                }
                _ => {}
            }
        }
        let ty = match tail {
            Some(e) => {
                if diverged {
                    self.errors
                        .push(SemanticError::UnreachableCode { span: e.span() });
                }
                // NLL: release dead borrows before tail expression
                self.release_dead_borrows_nll(e.span().start);
                self.check_expr(e)
            }
            None => Type::Void,
        };
        self.emit_unused_warnings();
        self.moves.pop_scope();
        ty
    }

    /// Checks an if expression.
    fn check_if(
        &mut self,
        condition: &Expr,
        then_branch: &Expr,
        else_branch: &Option<Box<Expr>>,
    ) -> Type {
        let cond_ty = self.check_expr(condition);
        if !cond_ty.is_compatible(&Type::Bool) && !cond_ty.is_integer() {
            self.errors.push(SemanticError::TypeMismatch {
                expected: "bool".into(),
                found: cond_ty.display_name(),
                span: condition.span(),
                hint: None,
            });
        }

        let then_ty = self.check_expr(then_branch);

        if let Some(else_e) = else_branch {
            let else_ty = self.check_expr(else_e);
            // If used as expression, both branches should match
            if !then_ty.is_compatible(&else_ty) && !matches!(then_ty, Type::Void) {
                self.errors.push(SemanticError::TypeMismatch {
                    expected: then_ty.display_name(),
                    found: else_ty.display_name(),
                    span: else_e.span(),
                    hint: None,
                });
            }
            then_ty
        } else {
            Type::Void
        }
    }

    /// Checks an assignment expression.
    fn check_assign(&mut self, target: &Expr, op: AssignOp, value: &Expr, span: Span) -> Type {
        let val_ty = self.check_expr(value);

        if let Expr::Ident {
            name,
            span: id_span,
        } = target
        {
            // Check mutability
            if let Some(sym) = self.symbols.lookup(name) {
                if !sym.mutable {
                    self.errors.push(SemanticError::ImmutableAssignment {
                        name: name.clone(),
                        span: *id_span,
                    });
                }
                // For simple assignment, check type compatibility
                if op == AssignOp::Assign && !sym.ty.is_compatible(&val_ty) {
                    self.errors.push(SemanticError::TypeMismatch {
                        expected: sym.ty.display_name(),
                        found: val_ty.display_name(),
                        span,
                        hint: None,
                    });
                }
            } else {
                let suggestion = suggest_similar(name, &self.symbols.all_names());
                self.errors.push(SemanticError::UndefinedVariable {
                    name: name.clone(),
                    span: *id_span,
                    suggestion,
                });
            }

            // ME004/ME005: assignment to a borrowed variable conflicts with active borrows
            if let Some(borrow_span) = self.moves.check_can_move(name) {
                self.errors.push(SemanticError::MutBorrowConflict {
                    name: name.clone(),
                    span,
                    borrow_span,
                });
            }
        } else {
            // Field/index assignment — just check the value
            self.check_expr(target);
        }

        Type::Void
    }

    /// Checks a match expression.
    fn check_match(&mut self, subject: &Expr, arms: &[MatchArm], span: Span) -> Type {
        let subject_ty = self.check_expr(subject);
        let mut result_ty: Option<Type> = None;
        let mut has_wildcard_or_catch_all = false;
        let mut matched_variants: Vec<String> = Vec::new();
        let mut matched_enum_name: Option<String> = None;

        // Move tracking: if the subject is a move-type variable and any arm
        // destructures it (Enum/Tuple/Struct pattern), mark it as moved.
        if let Expr::Ident {
            name: subject_name,
            span: subject_span,
        } = subject
        {
            if !super::borrow_lite::is_copy_type(&subject_ty) {
                let has_destructure = arms.iter().any(|arm| {
                    matches!(
                        arm.pattern,
                        Pattern::Enum { .. } | Pattern::Tuple { .. } | Pattern::Struct { .. }
                    )
                });
                if has_destructure {
                    if let Some(borrow_span) = self.moves.check_can_move(subject_name) {
                        self.errors.push(SemanticError::MoveWhileBorrowed {
                            name: subject_name.clone(),
                            span: *subject_span,
                            borrow_span,
                        });
                    }
                    self.moves.mark_moved(subject_name, *subject_span);
                }
            }
        }

        for arm in arms {
            // SE020: detect unreachable patterns (arms after a catch-all)
            if has_wildcard_or_catch_all {
                self.errors.push(SemanticError::UnreachablePattern {
                    span: arm.pattern.span(),
                });
            }

            // Check if this arm is a catch-all (wildcard or bare ident without guard)
            if arm.guard.is_none() {
                match &arm.pattern {
                    Pattern::Wildcard { .. } => has_wildcard_or_catch_all = true,
                    Pattern::Ident { name, .. } => {
                        // Check if this ident is a known enum variant (e.g. "None")
                        let is_variant = self.enum_variants.values().any(|vs| vs.contains(name));
                        if is_variant {
                            matched_variants.push(name.clone());
                            // Infer enum name from variant
                            if matched_enum_name.is_none() {
                                for (ename, vs) in &self.enum_variants {
                                    if vs.contains(name) {
                                        matched_enum_name = Some(ename.clone());
                                        break;
                                    }
                                }
                            }
                        } else {
                            has_wildcard_or_catch_all = true;
                        }
                    }
                    Pattern::Enum {
                        enum_name, variant, ..
                    } => {
                        matched_variants.push(variant.clone());
                        if matched_enum_name.is_none() {
                            if !enum_name.is_empty() {
                                matched_enum_name = Some(enum_name.clone());
                            } else {
                                // Infer enum name from variant
                                for (ename, vs) in &self.enum_variants {
                                    if vs.contains(variant) {
                                        matched_enum_name = Some(ename.clone());
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }

            self.symbols.push_scope();
            self.check_pattern(&arm.pattern);
            if let Some(guard) = &arm.guard {
                self.check_expr(guard);
            }
            let arm_ty = self.check_expr(&arm.body);
            if result_ty.is_none() {
                result_ty = Some(arm_ty);
            }
            self.symbols.pop_scope();
        }

        // SE011: Non-exhaustive match
        if !has_wildcard_or_catch_all && !arms.is_empty() {
            if let Some(ref ename) = matched_enum_name {
                // Check if all variants of the known enum are covered
                if let Some(all_variants) = self.enum_variants.get(ename) {
                    let all_covered = all_variants.iter().all(|v| matched_variants.contains(v));
                    if !all_covered {
                        self.errors.push(SemanticError::NonExhaustiveMatch { span });
                    }
                }
                // Unknown enum — skip (can't verify)
            } else if matched_variants.is_empty() {
                // No enum variants and no wildcard — definitely non-exhaustive
                self.errors.push(SemanticError::NonExhaustiveMatch { span });
            }
        }

        result_ty.unwrap_or(Type::Void)
    }

    /// Registers pattern bindings in the current scope.
    fn check_pattern(&mut self, pattern: &Pattern) {
        match pattern {
            Pattern::Ident { name, span } => {
                self.symbols.define(Symbol {
                    name: name.clone(),
                    ty: Type::Unknown,
                    mutable: false,
                    span: *span,
                    used: false,
                });
            }
            Pattern::Tuple { elements, .. } => {
                for elem in elements {
                    self.check_pattern(elem);
                }
            }
            Pattern::Enum { fields, .. } => {
                for field in fields {
                    self.check_pattern(field);
                }
            }
            Pattern::Struct { fields, .. } => {
                for fp in fields {
                    if let Some(ref pat) = fp.pattern {
                        self.check_pattern(pat);
                    } else {
                        self.symbols.define(Symbol {
                            name: fp.name.clone(),
                            ty: Type::Unknown,
                            mutable: false,
                            span: fp.span,
                            used: false,
                        });
                    }
                }
            }
            Pattern::Wildcard { .. } | Pattern::Literal { .. } | Pattern::Range { .. } => {}
        }
    }

    /// Checks an array literal.
    fn check_array(&mut self, elements: &[Expr], _span: Span) -> Type {
        if elements.is_empty() {
            return Type::Array(Box::new(Type::Unknown));
        }
        let first_ty = self.check_expr(&elements[0]);
        for elem in elements.iter().skip(1) {
            let elem_ty = self.check_expr(elem);
            if !first_ty.is_compatible(&elem_ty) {
                self.errors.push(SemanticError::TypeMismatch {
                    expected: first_ty.display_name(),
                    found: elem_ty.display_name(),
                    span: elem.span(),
                    hint: None,
                });
            }
        }
        Type::Array(Box::new(first_ty))
    }

    /// Checks a pipeline expression: `x |> f`.
    fn check_pipe(&mut self, left: &Expr, right: &Expr, span: Span) -> Type {
        let arg_ty = self.check_expr(left);
        let fn_ty = self.check_expr(right);

        match fn_ty {
            Type::Function { params, ret } => {
                if params.len() != 1 {
                    self.errors.push(SemanticError::ArgumentCountMismatch {
                        expected: 1,
                        found: params.len(),
                        span,
                    });
                } else if !params[0].is_compatible(&arg_ty) {
                    self.errors.push(SemanticError::TypeMismatch {
                        expected: params[0].display_name(),
                        found: arg_ty.display_name(),
                        span: left.span(),
                        hint: None,
                    });
                }
                *ret
            }
            Type::Unknown => Type::Unknown,
            _ => {
                self.errors.push(SemanticError::UndefinedFunction {
                    name: format!("{fn_ty}"),
                    span: right.span(),
                    suggestion: None,
                });
                Type::Unknown
            }
        }
    }

    /// Checks struct initialization.
    fn check_struct_init(
        &mut self,
        name: &str,
        fields: &[crate::parser::ast::FieldInit],
        span: Span,
    ) -> Type {
        let struct_ty = self.symbols.lookup(name).map(|s| s.ty.clone());

        match struct_ty {
            Some(Type::Struct {
                name: sname,
                fields: def_fields,
            }) => {
                // Check each provided field
                for fi in fields {
                    let val_ty = self.check_expr(&fi.value);
                    if let Some(expected_ty) = def_fields.get(&fi.name) {
                        if !expected_ty.is_compatible(&val_ty) {
                            self.errors.push(SemanticError::TypeMismatch {
                                expected: expected_ty.display_name(),
                                found: val_ty.display_name(),
                                span: fi.value.span(),
                                hint: None,
                            });
                        }
                    }
                }
                // Check for missing required fields
                for fname in def_fields.keys() {
                    if !fields.iter().any(|fi| &fi.name == fname) {
                        self.errors.push(SemanticError::MissingField {
                            struct_name: sname.clone(),
                            field: fname.clone(),
                            span,
                        });
                    }
                }
                Type::Struct {
                    name: sname,
                    fields: def_fields,
                }
            }
            _ => {
                // Struct not defined — duck-type it
                for fi in fields {
                    self.check_expr(&fi.value);
                }
                Type::Struct {
                    name: name.to_string(),
                    fields: HashMap::new(),
                }
            }
        }
    }

    /// Checks field access.
    fn check_field(&mut self, object: &Expr, field: &str, span: Span) -> Type {
        let obj_ty = self.check_expr(object);
        match &obj_ty {
            Type::Struct { fields, name } => {
                if let Some(ft) = fields.get(field) {
                    ft.clone()
                } else {
                    let field_names: Vec<String> = fields.keys().cloned().collect();
                    let suggestion = suggest_similar(field, &field_names);
                    self.errors.push(SemanticError::UndefinedVariable {
                        name: format!("{name}.{field}"),
                        span,
                        suggestion,
                    });
                    Type::Unknown
                }
            }
            Type::Tuple(elems) => {
                if let Ok(idx) = field.parse::<usize>() {
                    elems.get(idx).cloned().unwrap_or(Type::Unknown)
                } else {
                    Type::Unknown
                }
            }
            _ => Type::Unknown,
        }
    }

    /// Checks index access.
    fn check_index(&mut self, object: &Expr, index: &Expr, _span: Span) -> Type {
        let obj_ty = self.check_expr(object);
        let idx_ty = self.check_expr(index);

        if !idx_ty.is_integer() && !matches!(idx_ty, Type::Unknown) {
            self.errors.push(SemanticError::TypeMismatch {
                expected: "integer".into(),
                found: idx_ty.display_name(),
                span: index.span(),
                hint: None,
            });
        }

        match obj_ty {
            Type::Array(inner) => *inner,
            Type::Str => Type::Char,
            _ => Type::Unknown,
        }
    }

    /// Checks a method call.
    fn check_method_call(
        &mut self,
        receiver: &Expr,
        method: &str,
        args: &[CallArg],
        _span: Span,
    ) -> Type {
        let obj_ty = self.check_expr(receiver);
        for arg in args {
            self.check_expr(&arg.value);
        }

        match (&obj_ty, method) {
            // String methods
            (Type::Str, "len") => Type::USize,
            (Type::Str, "contains" | "starts_with" | "ends_with" | "is_empty") => Type::Bool,
            (
                Type::Str,
                "trim" | "trim_start" | "trim_end" | "to_uppercase" | "to_lowercase" | "replace"
                | "rev" | "repeat",
            ) => Type::Str,
            (Type::Str, "split") => Type::Array(Box::new(Type::Str)),
            (Type::Str, "bytes") => Type::Array(Box::new(Type::I64)),
            (Type::Str, "chars") => Type::Array(Box::new(Type::Char)),
            (Type::Str, "index_of") => Type::Unknown, // returns Option
            (Type::Str, "parse_int") => Type::Unknown, // returns Result
            (Type::Str, "parse_float") => Type::Unknown, // returns Result
            (Type::Str, "substring" | "char_at") => Type::Str,
            // Array methods
            (Type::Array(_), "len") => Type::USize,
            (Type::Array(inner), "push") => Type::Array(inner.clone()),
            (Type::Array(_), "is_empty") => Type::Bool,
            (Type::Array(_), "contains") => Type::Bool,
            (Type::Array(_), "first" | "last") => Type::Unknown, // returns Option
            (Type::Array(_), "pop") => Type::Unknown,            // returns Option
            (Type::Array(_), "reverse") => obj_ty.clone(),
            (Type::Array(_), "join") => Type::Str,
            // Map methods
            (Type::Unknown, "get" | "insert" | "remove" | "keys" | "values" | "entries") => {
                Type::Unknown
            }
            _ => Type::Unknown,
        }
    }

    // ── Type resolution ──

    /// Resolves a TypeExpr from the AST to a Type.
    fn resolve_type(&mut self, ty: &TypeExpr) -> Type {
        match ty {
            TypeExpr::Simple { name, .. } => match name.as_str() {
                "void" => Type::Void,
                "never" => Type::Never,
                "bool" => Type::Bool,
                "i8" => Type::I8,
                "i16" => Type::I16,
                "i32" => Type::I32,
                "i64" => Type::I64,
                "i128" => Type::I128,
                "u1" | "u2" | "u3" | "u4" | "u5" | "u6" | "u7" => Type::U8,
                "u8" => Type::U8,
                "u16" => Type::U16,
                "u32" => Type::U32,
                "u64" => Type::U64,
                "u128" => Type::U128,
                "isize" => Type::ISize,
                "usize" => Type::USize,
                "f16" => Type::F16,
                "bf16" => Type::Bf16,
                "f32" => Type::F32,
                "f64" => Type::F64,
                "char" => Type::Char,
                "str" | "String" => Type::Str,
                "any" => Type::Unknown,
                other => {
                    // Check type aliases first
                    if let Some(aliased) = self.type_aliases.get(other) {
                        return aliased.clone();
                    }
                    // Check if it's a known struct/enum
                    if let Some(sym) = self.symbols.lookup(other) {
                        sym.ty.clone()
                    } else {
                        Type::Named(other.to_string())
                    }
                }
            },
            TypeExpr::Array { element, .. } => Type::Array(Box::new(self.resolve_type(element))),
            TypeExpr::Tuple { elements, .. } => {
                Type::Tuple(elements.iter().map(|t| self.resolve_type(t)).collect())
            }
            TypeExpr::Fn {
                params,
                return_type,
                ..
            } => Type::Function {
                params: params.iter().map(|t| self.resolve_type(t)).collect(),
                ret: Box::new(self.resolve_type(return_type)),
            },
            TypeExpr::Reference { mutable, inner, .. } => {
                let inner_ty = self.resolve_type(inner);
                if *mutable {
                    Type::RefMut(Box::new(inner_ty))
                } else {
                    Type::Ref(Box::new(inner_ty))
                }
            }
            TypeExpr::Slice { element, .. } => Type::Array(Box::new(self.resolve_type(element))),
            TypeExpr::Generic { name, args, .. } => {
                // Resolve known generic containers
                match name.as_str() {
                    "Array" | "Vec" if args.len() == 1 => {
                        Type::Array(Box::new(self.resolve_type(&args[0])))
                    }
                    "Future" if args.len() == 1 => Type::Future {
                        inner: Box::new(self.resolve_type(&args[0])),
                    },
                    other => {
                        // Check type aliases
                        if let Some(aliased) = self.type_aliases.get(other) {
                            return aliased.clone();
                        }
                        Type::Named(other.to_string())
                    }
                }
            }
            TypeExpr::Path { segments, .. } => {
                Type::Named(segments.last().cloned().unwrap_or_default())
            }
            TypeExpr::Tensor {
                element_type,
                dims,
                span,
            } => {
                let elem = self.resolve_type(element_type);
                if elem.is_tensor() {
                    self.errors.push(SemanticError::TypeMismatch {
                        expected: "scalar element type (f32, f64, i32, ...)".into(),
                        found: elem.display_name(),
                        span: *span,
                        hint: None,
                    });
                }
                Type::Tensor {
                    element: Box::new(elem),
                    dims: dims.clone(),
                }
            }
            TypeExpr::Pointer { .. } => Type::Unknown,
            TypeExpr::DynTrait {
                trait_name, span, ..
            } => {
                // Validate the trait exists
                if !self.traits.contains_key(trait_name) {
                    self.errors.push(SemanticError::UnknownTrait {
                        name: trait_name.clone(),
                        span: *span,
                    });
                }
                Type::DynTrait(trait_name.clone())
            }
        }
    }

    /// Checks lifetime elision rules for a function definition.
    ///
    /// Implements the three Rust-inspired lifetime elision rules:
    /// 1. Each elided lifetime in input position becomes a distinct lifetime parameter.
    /// 2. If there is exactly one input lifetime, it is assigned to all elided output lifetimes.
    /// 3. If there is a `&self` or `&mut self` parameter, its lifetime is assigned to all
    ///    elided output lifetimes.
    ///
    /// This function validates that explicitly annotated lifetimes are consistent
    /// and warns about unnecessary annotations that match elision rules.
    pub fn check_lifetime_elision(&mut self, fndef: &crate::parser::ast::FnDef) {
        // Collect lifetimes from parameters
        let mut input_lifetimes = Vec::new();
        let mut has_self_ref = false;
        let mut self_lifetime: Option<String> = None;

        for param in &fndef.params {
            let mut param_lifetimes = Vec::new();
            collect_lifetimes_from_type(&param.ty, &mut param_lifetimes);
            if param.name == "self" {
                has_self_ref = true;
                if let Some(lt) = param_lifetimes.first() {
                    self_lifetime = Some(lt.clone());
                }
            }
            input_lifetimes.extend(param_lifetimes);
        }

        // Check for duplicate lifetime param declarations
        let mut seen_lifetimes = std::collections::HashSet::new();
        for lp in &fndef.lifetime_params {
            if !seen_lifetimes.insert(lp.name.clone()) {
                self.errors.push(SemanticError::LifetimeConflict {
                    name: lp.name.clone(),
                    span: lp.span,
                });
            }
        }

        // Validate that lifetimes used in params are declared
        let declared: std::collections::HashSet<String> = fndef
            .lifetime_params
            .iter()
            .map(|lp| lp.name.clone())
            .collect();

        for lt in &input_lifetimes {
            if lt != "static" && lt != "_" && !declared.contains(lt) {
                self.errors.push(SemanticError::LifetimeMismatch {
                    expected: "declared lifetime".into(),
                    found: lt.clone(),
                    span: fndef.span,
                });
            }
        }

        // Collect output lifetimes (from return type)
        if let Some(ref ret_ty) = fndef.return_type {
            let mut output_lifetimes = Vec::new();
            collect_lifetimes_from_type(ret_ty, &mut output_lifetimes);

            for lt in &output_lifetimes {
                if lt != "static" && lt != "_" && !declared.contains(lt) {
                    self.errors.push(SemanticError::LifetimeMismatch {
                        expected: "declared lifetime".into(),
                        found: lt.clone(),
                        span: fndef.span,
                    });
                }
            }

            // Elision rule 2 & 3: validate output lifetimes
            // If there's exactly one input lifetime or &self, output can use it
            let elision_source = if has_self_ref {
                self_lifetime.clone()
            } else if input_lifetimes.len() == 1 {
                Some(input_lifetimes[0].clone())
            } else {
                None
            };

            // If there are output lifetimes and no elision source, they must all be declared
            if !output_lifetimes.is_empty() && elision_source.is_none() && input_lifetimes.len() > 1
            {
                // Multiple input lifetimes, no &self — output lifetime is ambiguous
                // This is fine as long as all output lifetimes are explicitly declared
                // (already checked above)
            }
        }
    }
}

/// Collects all lifetime names referenced in a type expression.
///
/// Walks the type expression tree and extracts lifetime annotations
/// from reference types. Used by lifetime elision checking.
pub fn collect_lifetimes_from_type(ty: &crate::parser::ast::TypeExpr, out: &mut Vec<String>) {
    match ty {
        crate::parser::ast::TypeExpr::Reference {
            lifetime, inner, ..
        } => {
            if let Some(lt) = lifetime {
                out.push(lt.clone());
            }
            collect_lifetimes_from_type(inner, out);
        }
        crate::parser::ast::TypeExpr::Generic { args, .. } => {
            for arg in args {
                collect_lifetimes_from_type(arg, out);
            }
        }
        crate::parser::ast::TypeExpr::Tuple { elements, .. } => {
            for elem in elements {
                collect_lifetimes_from_type(elem, out);
            }
        }
        crate::parser::ast::TypeExpr::Array { element, .. }
        | crate::parser::ast::TypeExpr::Slice { element, .. } => {
            collect_lifetimes_from_type(element, out);
        }
        crate::parser::ast::TypeExpr::Pointer { inner, .. } => {
            collect_lifetimes_from_type(inner, out);
        }
        crate::parser::ast::TypeExpr::Fn {
            params,
            return_type,
            ..
        } => {
            for p in params {
                collect_lifetimes_from_type(p, out);
            }
            collect_lifetimes_from_type(return_type, out);
        }
        _ => {}
    }
}

impl Default for TypeChecker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::tokenize;
    use crate::parser::parse;

    fn check(source: &str) -> Result<(), Vec<SemanticError>> {
        let tokens = tokenize(source).expect("lex error");
        let program = parse(tokens).expect("parse error");
        let mut tc = TypeChecker::new();
        tc.analyze(&program)
    }

    fn check_errors(source: &str) -> Vec<SemanticError> {
        check(source).unwrap_err()
    }

    /// Returns all diagnostics (errors + warnings) from analysis.
    fn check_all_diagnostics(source: &str) -> Vec<SemanticError> {
        let tokens = tokenize(source).expect("lex error");
        let program = parse(tokens).expect("parse error");
        let mut tc = TypeChecker::new();
        let _ = tc.analyze(&program);
        tc.errors
    }

    // ── Valid programs ──

    #[test]
    fn valid_int_arithmetic() {
        assert!(check("1 + 2").is_ok());
    }

    #[test]
    fn valid_let_binding() {
        assert!(check("let x = 42").is_ok());
    }

    #[test]
    fn valid_let_with_type() {
        assert!(check("let x: i64 = 42").is_ok());
    }

    #[test]
    fn valid_string_concat() {
        assert!(check("\"a\" + \"b\"").is_ok());
    }

    #[test]
    fn valid_boolean_comparison() {
        assert!(check("1 < 2").is_ok());
    }

    #[test]
    fn valid_function_def_and_call() {
        let src = "fn add(a: i64, b: i64) -> i64 { a + b }\nadd(1, 2)";
        assert!(check(src).is_ok());
    }

    #[test]
    fn valid_if_else() {
        assert!(check("if true { 1 } else { 2 }").is_ok());
    }

    #[test]
    fn valid_while_loop() {
        assert!(check("let mut x = 0\nwhile x < 5 { x += 1 }").is_ok());
    }

    #[test]
    fn valid_for_loop() {
        assert!(check("for i in [1, 2, 3] { println(i) }").is_ok());
    }

    #[test]
    fn valid_array_literal() {
        assert!(check("[1, 2, 3]").is_ok());
    }

    #[test]
    fn valid_closure() {
        assert!(check("let f = |x: i64| -> i64 { x * 2 }\nf(5)").is_ok());
    }

    #[test]
    fn valid_println() {
        assert!(check("println(42)").is_ok());
    }

    #[test]
    fn valid_pipeline() {
        let src = "fn double(x: i64) -> i64 { x * 2 }\n5 |> double";
        assert!(check(src).is_ok());
    }

    #[test]
    fn valid_match() {
        let src = "match 1 { 1 => true, _ => false }";
        assert!(check(src).is_ok());
    }

    #[test]
    fn valid_struct_def_and_init() {
        let src = "struct Point { x: f64, y: f64 }\nlet p = Point { x: 1.0, y: 2.0 }";
        assert!(check(src).is_ok());
    }

    // ── Type errors ──

    #[test]
    fn error_undefined_variable() {
        let errors = check_errors("x + 1");
        assert!(errors
            .iter()
            .any(|e| matches!(e, SemanticError::UndefinedVariable { .. })));
    }

    #[test]
    fn error_type_mismatch_let() {
        let errors = check_errors("let x: i64 = \"hello\"");
        assert!(errors
            .iter()
            .any(|e| matches!(e, SemanticError::TypeMismatch { .. })));
    }

    #[test]
    fn error_immutable_assignment() {
        let errors = check_errors("let x = 1\nx = 2");
        assert!(errors
            .iter()
            .any(|e| matches!(e, SemanticError::ImmutableAssignment { .. })));
    }

    #[test]
    fn error_arity_mismatch() {
        let src = "fn f(a: i64) -> i64 { a }\nf(1, 2)";
        let errors = check_errors(src);
        assert!(errors
            .iter()
            .any(|e| matches!(e, SemanticError::ArgumentCountMismatch { .. })));
    }

    #[test]
    fn error_missing_struct_field() {
        let src = "struct Point { x: f64, y: f64 }\nlet p = Point { x: 1.0 }";
        let errors = check_errors(src);
        assert!(errors
            .iter()
            .any(|e| matches!(e, SemanticError::MissingField { .. })));
    }

    #[test]
    fn error_struct_field_type_mismatch() {
        let src = "struct Point { x: f64, y: f64 }\nlet p = Point { x: \"hi\", y: 2.0 }";
        let errors = check_errors(src);
        assert!(errors
            .iter()
            .any(|e| matches!(e, SemanticError::TypeMismatch { .. })));
    }

    #[test]
    fn error_mixed_array_types() {
        let errors = check_errors("[1, \"hello\"]");
        assert!(errors
            .iter()
            .any(|e| matches!(e, SemanticError::TypeMismatch { .. })));
    }

    #[test]
    fn error_fn_return_type_mismatch() {
        let src = "fn f() -> i64 { \"hello\" }";
        let errors = check_errors(src);
        assert!(errors
            .iter()
            .any(|e| matches!(e, SemanticError::TypeMismatch { .. })));
    }

    #[test]
    fn error_argument_type_mismatch() {
        let src = "fn f(a: i64) -> i64 { a }\nf(\"hello\")";
        let errors = check_errors(src);
        assert!(errors
            .iter()
            .any(|e| matches!(e, SemanticError::TypeMismatch { .. })));
    }

    #[test]
    fn valid_mutable_assignment() {
        assert!(check("let mut x = 1\nx = 2").is_ok());
    }

    #[test]
    fn valid_nested_functions() {
        let src = r#"
            fn outer(x: i64) -> i64 {
                fn inner(y: i64) -> i64 { y * 2 }
                inner(x) + 1
            }
            outer(5)
        "#;
        assert!(check(src).is_ok());
    }

    #[test]
    fn valid_fibonacci() {
        let src = r#"
            fn fib(n: i64) -> i64 {
                if n <= 1 { n } else { fib(n - 1) + fib(n - 2) }
            }
            fib(10)
        "#;
        assert!(check(src).is_ok());
    }

    #[test]
    fn valid_multiple_errors_collected() {
        let src = "let x: i64 = \"hi\"\nlet y: bool = 42";
        let errors = check_errors(src);
        assert!(errors.len() >= 2, "should collect multiple errors");
    }

    // ── Sprint 2.6: Distinct integer/float types ──

    #[test]
    fn error_i32_not_assignable_to_i64() {
        let src = "fn f(x: i32) -> i32 { x }\nlet y: i64 = f(1)";
        let errors = check_errors(src);
        assert!(errors
            .iter()
            .any(|e| matches!(e, SemanticError::TypeMismatch { .. })));
    }

    #[test]
    fn error_f32_not_assignable_to_f64() {
        let src = "fn f(x: f32) -> f32 { x }\nlet y: f64 = f(1.0)";
        let errors = check_errors(src);
        assert!(errors
            .iter()
            .any(|e| matches!(e, SemanticError::TypeMismatch { .. })));
    }

    #[test]
    fn error_mixed_int_arithmetic() {
        let src = r#"
            fn get_i32() -> i32 { 1 }
            fn get_i64() -> i64 { 2 }
            let x = get_i32() + get_i64()
        "#;
        let errors = check_errors(src);
        assert!(errors
            .iter()
            .any(|e| matches!(e, SemanticError::TypeMismatch { .. })));
    }

    #[test]
    fn valid_same_type_arithmetic() {
        let src = r#"
            fn a() -> i32 { 1 }
            fn b() -> i32 { 2 }
            let x = a() + b()
        "#;
        assert!(check(src).is_ok());
    }

    #[test]
    fn valid_i32_binding() {
        assert!(check("let x: i32 = 42").is_ok());
    }

    #[test]
    fn valid_f32_binding() {
        assert!(check("let x: f32 = 3.14").is_ok());
    }

    #[test]
    fn valid_u8_binding() {
        assert!(check("let x: u8 = 255").is_ok());
    }

    #[test]
    fn error_bitnot_on_float() {
        let src = "fn f(x: f64) -> f64 { x }\nlet x = ~f(1.0)";
        let errors = check_errors(src);
        assert!(errors.iter().any(
            |e| matches!(e, SemanticError::TypeMismatch { expected, .. } if expected == "integer")
        ));
    }

    #[test]
    fn valid_bitnot_preserves_type() {
        let src = r#"
            fn get_u32() -> u32 { 42 }
            let x: u32 = ~get_u32()
        "#;
        assert!(check(src).is_ok());
    }

    #[test]
    fn error_bitwise_mixed_int_types() {
        let src = r#"
            fn a() -> i32 { 1 }
            fn b() -> i64 { 2 }
            let x = a() & b()
        "#;
        let errors = check_errors(src);
        assert!(errors
            .iter()
            .any(|e| matches!(e, SemanticError::TypeMismatch { .. })));
    }

    #[test]
    fn valid_all_integer_types_resolve() {
        let src = r#"
            let a: i8 = 1
            let b: i16 = 2
            let c: i32 = 3
            let d: i64 = 4
            let e: i128 = 5
            let f: u8 = 6
            let g: u16 = 7
            let h: u32 = 8
            let i: u64 = 9
            let j: u128 = 10
            let k: isize = 11
            let l: usize = 12
        "#;
        assert!(check(src).is_ok());
    }

    #[test]
    fn valid_both_float_types_resolve() {
        let src = "let a: f32 = 1.0\nlet b: f64 = 2.0";
        assert!(check(src).is_ok());
    }

    // ── Sprint 2.7: SE009 UnusedVariable ──

    #[test]
    fn warning_unused_variable_in_function() {
        let src = r#"
            fn f() -> void {
                let unused_var = 42
            }
        "#;
        let diags = check_all_diagnostics(src);
        assert!(diags.iter().any(
            |e| matches!(e, SemanticError::UnusedVariable { name, .. } if name == "unused_var")
        ));
    }

    #[test]
    fn no_warning_for_used_variable() {
        let src = r#"
            fn f() -> i64 {
                let x = 42
                x
            }
        "#;
        let diags = check_all_diagnostics(src);
        assert!(
            !diags
                .iter()
                .any(|e| matches!(e, SemanticError::UnusedVariable { .. })),
            "should not warn about used variables"
        );
    }

    #[test]
    fn no_warning_for_underscore_prefix() {
        let src = r#"
            fn f() -> void {
                let _unused = 42
            }
        "#;
        let diags = check_all_diagnostics(src);
        assert!(
            !diags
                .iter()
                .any(|e| matches!(e, SemanticError::UnusedVariable { .. })),
            "_ prefix should suppress unused warning"
        );
    }

    #[test]
    fn unused_variable_is_warning_not_error() {
        let src = r#"
            fn f() -> void {
                let unused = 42
            }
        "#;
        // Should not cause analyze() to fail
        assert!(check(src).is_ok());
    }

    // ── Sprint 2.7: SE010 UnreachableCode ──

    #[test]
    fn warning_unreachable_after_return() {
        let src = r#"
            fn f() -> i64 {
                return 1
                let x = 2
                x
            }
        "#;
        let diags = check_all_diagnostics(src);
        assert!(diags
            .iter()
            .any(|e| matches!(e, SemanticError::UnreachableCode { .. })));
    }

    #[test]
    fn no_warning_without_early_return() {
        let src = r#"
            fn f() -> i64 {
                let x = 1
                let y = 2
                x + y
            }
        "#;
        let diags = check_all_diagnostics(src);
        assert!(
            !diags
                .iter()
                .any(|e| matches!(e, SemanticError::UnreachableCode { .. })),
            "no unreachable code here"
        );
    }

    #[test]
    fn unreachable_code_is_warning_not_error() {
        let src = r#"
            fn f() -> i64 {
                return 1
                2
            }
        "#;
        assert!(check(src).is_ok());
    }

    // ── Sprint 2.7: SE011 NonExhaustiveMatch ──

    #[test]
    fn error_non_exhaustive_match() {
        let src = r#"
            fn f(x: i64) -> str {
                match x {
                    0 => "zero",
                    1 => "one"
                }
            }
        "#;
        let errors = check_errors(src);
        assert!(errors
            .iter()
            .any(|e| matches!(e, SemanticError::NonExhaustiveMatch { .. })));
    }

    #[test]
    fn valid_exhaustive_match_with_wildcard() {
        let src = r#"
            fn f(x: i64) -> str {
                match x {
                    0 => "zero",
                    _ => "other"
                }
            }
        "#;
        assert!(check(src).is_ok());
    }

    #[test]
    fn valid_exhaustive_match_with_binding() {
        let src = r#"
            fn f(x: i64) -> i64 {
                match x {
                    0 => 0,
                    n => n * 2
                }
            }
        "#;
        assert!(check(src).is_ok());
    }

    #[test]
    fn non_exhaustive_match_is_error() {
        let src = r#"
            fn f(x: i64) -> str {
                match x {
                    0 => "zero"
                }
            }
        "#;
        assert!(check(src).is_err());
    }

    // ── Sprint 2.8: ScopeKind & Context Tracking ──

    #[test]
    fn error_break_outside_loop() {
        let src = r#"
            fn f() -> void {
                break
            }
        "#;
        let errors = check_errors(src);
        assert!(errors
            .iter()
            .any(|e| matches!(e, SemanticError::BreakOutsideLoop { .. })));
    }

    #[test]
    fn error_continue_outside_loop() {
        let src = r#"
            fn f() -> void {
                continue
            }
        "#;
        let errors = check_errors(src);
        assert!(errors
            .iter()
            .any(|e| matches!(e, SemanticError::BreakOutsideLoop { .. })));
    }

    #[test]
    fn valid_break_inside_while() {
        let src = r#"
            fn f() -> void {
                let mut i = 0
                while i < 10 {
                    if i == 5 { break }
                    i = i + 1
                }
            }
        "#;
        assert!(check(src).is_ok());
    }

    #[test]
    fn valid_break_inside_for() {
        let src = r#"
            fn f() -> void {
                for i in [1, 2, 3] {
                    if i == 2 { break }
                }
            }
        "#;
        assert!(check(src).is_ok());
    }

    #[test]
    fn valid_continue_inside_loop() {
        let src = r#"
            fn f() -> void {
                for i in [1, 2, 3] {
                    if i == 2 { continue }
                    println(i)
                }
            }
        "#;
        assert!(check(src).is_ok());
    }

    #[test]
    fn error_return_outside_function() {
        let errors = check_errors("return 42");
        assert!(errors
            .iter()
            .any(|e| matches!(e, SemanticError::ReturnOutsideFunction { .. })));
    }

    #[test]
    fn valid_return_inside_function() {
        let src = r#"
            fn f() -> i64 {
                return 42
            }
        "#;
        assert!(check(src).is_ok());
    }

    // ── Sprint 3.7: Context enforcement (@kernel/@device) ──

    #[test]
    fn kernel_fn_can_call_os_builtins() {
        let src = "@kernel fn init() { mem_alloc(4096, 8) }";
        assert!(check(src).is_ok());
    }

    #[test]
    fn device_fn_cannot_call_os_builtins() {
        let src = "@device fn bad() { mem_alloc(4096, 8) }";
        let errors = check_errors(src);
        assert!(errors
            .iter()
            .any(|e| matches!(e, SemanticError::RawPointerInDevice { .. })));
    }

    #[test]
    fn device_fn_cannot_call_kernel_fn() {
        let src = r#"
            @kernel fn kern_init() -> i64 { 0 }
            @device fn bad() -> i64 { kern_init() }
        "#;
        let errors = check_errors(src);
        assert!(errors
            .iter()
            .any(|e| matches!(e, SemanticError::KernelCallInDevice { .. })));
    }

    #[test]
    fn kernel_fn_cannot_call_device_fn() {
        let src = r#"
            @device fn infer() -> i64 { 0 }
            @kernel fn bad() -> i64 { infer() }
        "#;
        let errors = check_errors(src);
        assert!(errors
            .iter()
            .any(|e| matches!(e, SemanticError::DeviceCallInKernel { .. })));
    }

    #[test]
    fn safe_fn_can_call_both_kernel_and_device() {
        let src = r#"
            @kernel fn kern_fn() -> i64 { 0 }
            @device fn dev_fn() -> i64 { 0 }
            fn bridge() -> i64 { kern_fn() + dev_fn() }
        "#;
        assert!(check(src).is_ok());
    }

    #[test]
    fn device_fn_cannot_call_irq_register() {
        let src = r#"@device fn bad() { irq_register(32, "handler") }"#;
        let errors = check_errors(src);
        assert!(errors
            .iter()
            .any(|e| matches!(e, SemanticError::RawPointerInDevice { .. })));
    }

    #[test]
    fn device_fn_cannot_call_port_write() {
        let src = "@device fn bad() { port_write(128, 42) }";
        let errors = check_errors(src);
        assert!(errors
            .iter()
            .any(|e| matches!(e, SemanticError::RawPointerInDevice { .. })));
    }

    // ── Sprint 3.10: KE001/KE002 enforcement ──

    #[test]
    fn kernel_fn_cannot_call_push() {
        let src = r#"
            @kernel fn bad() {
                let mut arr = [1, 2, 3]
                push(arr, 4)
            }
        "#;
        let errors = check_errors(src);
        assert!(errors
            .iter()
            .any(|e| matches!(e, SemanticError::HeapAllocInKernel { .. })));
    }

    #[test]
    fn kernel_fn_cannot_call_to_string() {
        let src = r#"@kernel fn bad() { to_string(42) }"#;
        let errors = check_errors(src);
        assert!(errors
            .iter()
            .any(|e| matches!(e, SemanticError::HeapAllocInKernel { .. })));
    }

    #[test]
    fn kernel_fn_cannot_call_pop() {
        let src = r#"
            @kernel fn bad() {
                let mut arr = [1, 2, 3]
                pop(arr)
            }
        "#;
        let errors = check_errors(src);
        assert!(errors
            .iter()
            .any(|e| matches!(e, SemanticError::HeapAllocInKernel { .. })));
    }

    #[test]
    fn safe_fn_can_call_push() {
        let src = r#"
            fn ok() {
                let mut arr = [1, 2, 3]
                push(arr, 4)
            }
        "#;
        assert!(check(src).is_ok());
    }

    #[test]
    fn kernel_fn_still_allows_non_heap_builtins() {
        let src = r#"@kernel fn ok() { println(42) }"#;
        assert!(check(src).is_ok());
    }

    #[test]
    fn kernel_fn_ke001_and_ke003_both_detected() {
        let src = r#"
            @device fn infer() -> i64 { 0 }
            @kernel fn bad() {
                push([1], 2)
                infer()
            }
        "#;
        let errors = check_errors(src);
        assert!(errors
            .iter()
            .any(|e| matches!(e, SemanticError::HeapAllocInKernel { .. })));
        assert!(errors
            .iter()
            .any(|e| matches!(e, SemanticError::DeviceCallInKernel { .. })));
    }

    // ── S6.1/S6.2 Trait system ──

    #[test]
    fn trait_def_is_registered() {
        // Trait definition should not produce errors
        let src = r#"
            trait Summary {
                fn summarize(self: str) -> str { "default" }
            }
        "#;
        assert!(check(src).is_ok());
    }

    #[test]
    fn trait_def_duplicate_method_error() {
        let src = r#"
            trait Bad {
                fn method(self: i64) -> i64 { 0 }
                fn method(self: i64) -> i64 { 1 }
            }
        "#;
        let errors = check_errors(src);
        assert!(errors
            .iter()
            .any(|e| matches!(e, SemanticError::DuplicateDefinition { .. })));
    }

    #[test]
    fn impl_trait_missing_method_error() {
        let src = r#"
            trait Greetable {
                fn greet(self: str) -> str { "hi" }
                fn farewell(self: str) -> str { "bye" }
            }
            struct Person { name: str }
            impl Greetable for Person {
                fn greet(self: str) -> str { "hello" }
            }
        "#;
        let diagnostics = check_all_diagnostics(src);
        assert!(diagnostics.iter().any(|e| matches!(
            e,
            SemanticError::MissingField {
                field,
                ..
            } if field == "farewell"
        )));
    }

    #[test]
    fn impl_trait_complete_passes() {
        let src = r#"
            trait Greetable {
                fn greet(self: str) -> str { "hi" }
            }
            struct Person { name: str }
            impl Greetable for Person {
                fn greet(self: str) -> str { "hello" }
            }
        "#;
        // No missing method errors
        let diagnostics = check_all_diagnostics(src);
        assert!(!diagnostics
            .iter()
            .any(|e| matches!(e, SemanticError::MissingField { .. })));
    }

    #[test]
    fn impl_trait_wrong_param_count_se016() {
        let src = r#"
            trait Adder {
                fn add(self: i64, a: i64, b: i64) -> i64 { 0 }
            }
            struct Calc { x: i64 }
            impl Adder for Calc {
                fn add(self: i64, a: i64) -> i64 { a }
            }
        "#;
        let errors = check_all_diagnostics(src);
        assert!(errors.iter().any(|e| matches!(
            e,
            SemanticError::TraitMethodSignatureMismatch { method, .. } if method == "add"
        )));
    }

    #[test]
    fn impl_trait_wrong_return_type_se016() {
        let src = r#"
            trait Stringify {
                fn to_s(self: i64) -> str { "x" }
            }
            struct Num { val: i64 }
            impl Stringify for Num {
                fn to_s(self: i64) -> i64 { 42 }
            }
        "#;
        let errors = check_all_diagnostics(src);
        assert!(errors.iter().any(|e| matches!(
            e,
            SemanticError::TraitMethodSignatureMismatch { method, detail, .. }
                if method == "to_s" && detail.contains("return type")
        )));
    }

    #[test]
    fn impl_trait_matching_signature_passes() {
        let src = r#"
            trait Doubler {
                fn double(self: i64, x: i64) -> i64 { 0 }
            }
            struct MyDoubler { val: i64 }
            impl Doubler for MyDoubler {
                fn double(self: i64, x: i64) -> i64 { x * 2 }
            }
        "#;
        let errors = check_all_diagnostics(src);
        assert!(!errors
            .iter()
            .any(|e| matches!(e, SemanticError::TraitMethodSignatureMismatch { .. })));
    }

    #[test]
    fn generic_fn_passes_through_analyzer() {
        let src = r#"
            fn identity<T>(x: T) -> T { x }
            fn main() -> void { let y = identity(42) }
        "#;
        assert!(check(src).is_ok());
    }

    // ── Extern function / FFI tests (S7.1) ──────────────────────────────

    #[test]
    fn extern_fn_with_ffi_safe_types_passes() {
        let src = r#"
            extern fn abs(x: i32) -> i32
            fn main() -> void { let y = 1 }
        "#;
        assert!(check(src).is_ok());
    }

    #[test]
    fn extern_fn_registered_in_symbol_table() {
        let src = r#"
            extern fn abs(x: i32) -> i32
            fn main() -> void { let y = abs(42) }
        "#;
        assert!(check(src).is_ok());
    }

    #[test]
    fn extern_fn_rejects_string_param() {
        let src = r#"
            extern fn bad(s: str) -> i32
            fn main() -> void { let y = 1 }
        "#;
        let result = check(src);
        assert!(result.is_err());
        let errs = result.unwrap_err();
        assert!(errs
            .iter()
            .any(|e| matches!(e, SemanticError::FfiUnsafeType { .. })));
    }

    #[test]
    fn extern_fn_rejects_string_return() {
        let src = r#"
            extern fn bad(x: i32) -> str
            fn main() -> void { let y = 1 }
        "#;
        let result = check(src);
        assert!(result.is_err());
        let errs = result.unwrap_err();
        assert!(errs
            .iter()
            .any(|e| matches!(e, SemanticError::FfiUnsafeType { .. })));
    }

    #[test]
    fn extern_fn_multiple_ffi_safe_params() {
        let src = r#"
            extern fn memcpy(dst: u64, src: u64, n: u64) -> u64
            fn main() -> void { let y = 1 }
        "#;
        assert!(check(src).is_ok());
    }

    // ── Type inference tests (S8.1) ─────────────────────────────────────

    #[test]
    fn type_inference_let_int_defaults_to_i64() {
        // `let x = 42` should infer x as i64 (not {integer})
        let src = r#"
            fn takes_i64(x: i64) -> void {}
            fn main() -> void {
                let x = 42
                takes_i64(x)
            }
        "#;
        assert!(check(src).is_ok());
    }

    #[test]
    fn type_inference_let_float_defaults_to_f64() {
        // `let x = 3.14` should infer x as f64 (not {float})
        let src = r#"
            fn takes_f64(x: f64) -> void {}
            fn main() -> void {
                let x = 3.14
                takes_f64(x)
            }
        "#;
        assert!(check(src).is_ok());
    }

    #[test]
    fn type_inference_explicit_annotation_preserved() {
        // `let x: i32 = 42` should use i32, not default to i64
        let src = r#"
            fn takes_i32(x: i32) -> void {}
            fn main() -> void {
                let x: i32 = 42
                takes_i32(x)
            }
        "#;
        assert!(check(src).is_ok());
    }

    #[test]
    fn default_literal_method() {
        assert_eq!(Type::IntLiteral.default_literal(), Type::I64);
        assert_eq!(Type::FloatLiteral.default_literal(), Type::F64);
        assert_eq!(Type::Bool.default_literal(), Type::Bool);
        assert_eq!(Type::Str.default_literal(), Type::Str);
    }

    // ── Type alias tests (S8.3) ─────────────────────────────────────────

    #[test]
    fn type_alias_resolves_in_function_signature() {
        let src = r#"
            type Meters = f64
            fn distance(m: Meters) -> Meters { m }
            fn main() -> void {
                let d: f64 = distance(3.14)
            }
        "#;
        assert!(check(src).is_ok());
    }

    #[test]
    fn type_alias_of_alias_resolves() {
        let src = r#"
            type Count = i64
            type Total = Count
            fn sum(a: Total, b: Total) -> Total { a + b }
            fn main() -> void {
                let x: i64 = sum(1, 2)
            }
        "#;
        assert!(check(src).is_ok());
    }

    #[test]
    fn type_alias_transparent_no_mismatch() {
        // Meters and f64 should be interchangeable
        let src = r#"
            type Meters = f64
            fn add_f64(a: f64, b: f64) -> f64 { a + b }
            fn main() -> void {
                let m: Meters = 1.0
                let n: f64 = 2.0
                let total = add_f64(m, n)
            }
        "#;
        assert!(check(src).is_ok());
    }

    // ── Never type & exhaustiveness (S8.4) ──────────────────────────────

    #[test]
    fn never_type_in_return_position() {
        // `fn diverge() -> ! { loop {} }` should be accepted
        let src = r#"
            fn diverge() -> ! {
                while true { 0 }
            }
            fn main() -> void { let x = 1 }
        "#;
        assert!(check(src).is_ok());
    }

    #[test]
    fn non_exhaustive_match_detected() {
        let src = r#"
            fn main() -> void {
                let x = 1
                match x {
                    1 => 10,
                    2 => 20
                }
            }
        "#;
        let result = check(src);
        assert!(result.is_err());
        let errs = result.unwrap_err();
        assert!(errs
            .iter()
            .any(|e| matches!(e, SemanticError::NonExhaustiveMatch { .. })));
    }

    #[test]
    fn exhaustive_match_with_wildcard_passes() {
        let src = r#"
            fn main() -> void {
                let x = 1
                match x {
                    1 => 10,
                    _ => 0
                }
            }
        "#;
        assert!(check(src).is_ok());
    }

    // ── v0.4 S2.4: Match exhaustiveness for generic enums ──

    #[test]
    fn exhaustive_option_match_all_variants() {
        // Match on Option with both Some and None → exhaustive, no error
        let src = r#"
            enum Option<T> { Some(T), None }
            fn check(x: i64) -> i64 {
                let opt = Some(x)
                match opt {
                    Some(v) => v,
                    None => 0
                }
            }
        "#;
        assert!(check(src).is_ok());
    }

    #[test]
    fn non_exhaustive_option_match_missing_none() {
        // Match on Option with only Some → non-exhaustive
        let src = r#"
            enum Option<T> { Some(T), None }
            fn check(x: i64) -> i64 {
                let opt = Some(x)
                match opt {
                    Some(v) => v
                }
            }
        "#;
        let errors = check_errors(src);
        assert!(errors
            .iter()
            .any(|e| matches!(e, SemanticError::NonExhaustiveMatch { .. })));
    }

    #[test]
    fn exhaustive_result_match_ok_err() {
        // Match on Result with Ok and Err → exhaustive
        let src = r#"
            enum Result<T, E> { Ok(T), Err(E) }
            fn check(x: i64) -> i64 {
                let r = Ok(x)
                match r {
                    Ok(v) => v,
                    Err(e) => 0
                }
            }
        "#;
        assert!(check(src).is_ok());
    }

    #[test]
    fn non_exhaustive_result_match_missing_err() {
        // Match on Result with only Ok → non-exhaustive
        let src = r#"
            enum Result<T, E> { Ok(T), Err(E) }
            fn check(x: i64) -> i64 {
                let r = Ok(x)
                match r {
                    Ok(v) => v
                }
            }
        "#;
        let errors = check_errors(src);
        assert!(errors
            .iter()
            .any(|e| matches!(e, SemanticError::NonExhaustiveMatch { .. })));
    }

    #[test]
    fn exhaustive_user_enum_all_variants() {
        // User-defined enum with all variants matched
        let src = r#"
            enum Color { Red, Green, Blue }
            fn name(c: i64) -> i64 {
                match c {
                    Color::Red => 1,
                    Color::Green => 2,
                    Color::Blue => 3
                }
            }
        "#;
        assert!(check(src).is_ok());
    }

    #[test]
    fn non_exhaustive_user_enum_missing_variant() {
        // User-defined enum missing Blue → non-exhaustive
        let src = r#"
            enum Color { Red, Green, Blue }
            fn name(c: i64) -> i64 {
                match c {
                    Color::Red => 1,
                    Color::Green => 2
                }
            }
        "#;
        let errors = check_errors(src);
        assert!(errors
            .iter()
            .any(|e| matches!(e, SemanticError::NonExhaustiveMatch { .. })));
    }

    // ── v0.4 S4: Future/Poll type system ──

    #[test]
    fn poll_enum_exhaustive_ready_pending() {
        // Poll<T> is a built-in enum — matching Ready + Pending is exhaustive
        let src = r#"
            fn check(x: i64) -> i64 {
                match x {
                    Ready(v) => v,
                    Pending => 0
                }
            }
        "#;
        assert!(check(src).is_ok());
    }

    #[test]
    fn poll_enum_non_exhaustive_missing_pending() {
        // Missing Pending → non-exhaustive
        let src = r#"
            fn check(x: i64) -> i64 {
                match x {
                    Ready(v) => v
                }
            }
        "#;
        let errors = check_errors(src);
        assert!(errors
            .iter()
            .any(|e| matches!(e, SemanticError::NonExhaustiveMatch { .. })));
    }

    #[test]
    fn await_outside_async_is_error() {
        // .await outside async fn → SE017
        let src = r#"
            fn not_async() -> i64 {
                let x = 42
                x.await
            }
        "#;
        let errors = check_errors(src);
        assert!(errors
            .iter()
            .any(|e| matches!(e, SemanticError::AwaitOutsideAsync { .. })));
    }

    #[test]
    fn await_inside_async_is_ok() {
        // .await inside async fn is valid
        let src = r#"
            async fn inner() -> i64 { 42 }
            async fn outer() -> i64 {
                inner().await
            }
        "#;
        assert!(check(src).is_ok());
    }

    // ── Move semantics (S9.1-S9.2) ─────────────────────────────────────

    #[test]
    fn copy_type_not_moved() {
        // i64 is Copy, so `let y = x; println(x)` should work
        let src = r#"
            fn main() -> void {
                let x = 42
                let y = x
                println(x)
            }
        "#;
        assert!(check(src).is_ok());
    }

    #[test]
    fn move_type_use_after_move_detected() {
        // str is now Copy (Rc-based cloning), so `let t = s; println(s)` is fine.
        // Test with an array (still Move) instead.
        let src = r#"
            fn main() -> void {
                let a: [i64] = [1, 2, 3]
                let b = a
                len(a)
            }
        "#;
        let result = check(src);
        assert!(result.is_err());
        let errs = result.unwrap_err();
        assert!(errs
            .iter()
            .any(|e| matches!(e, SemanticError::UseAfterMove { name, .. } if name == "a")));
    }

    #[test]
    fn move_type_ok_when_not_used_after() {
        // str is Copy now, so this always works. Kept for backward compat.
        let src = r#"
            fn main() -> void {
                let s: str = "hello"
                let t = s
                println(t)
            }
        "#;
        assert!(check(src).is_ok());
    }

    #[test]
    fn fn_call_moves_move_type_arg() {
        // str is now Copy, so passing it to a function doesn't move it.
        // Test with an array (still Move) instead.
        let src = r#"
            fn consume(a: [i64]) -> void {
                println(len(a))
            }
            fn main() -> void {
                let a: [i64] = [1, 2, 3]
                consume(a)
                len(a)
            }
        "#;
        let result = check(src);
        assert!(result.is_err());
        let errs = result.unwrap_err();
        assert!(errs
            .iter()
            .any(|e| matches!(e, SemanticError::UseAfterMove { name, .. } if name == "a")));
    }

    #[test]
    fn fn_call_copy_type_arg_not_moved() {
        // Passing a copy-type (i64) to a function should NOT mark it moved
        let src = r#"
            fn use_val(x: i64) -> i64 {
                x + 1
            }
            fn main() -> void {
                let x = 42
                use_val(x)
                println(x)
            }
        "#;
        assert!(check(src).is_ok());
    }

    #[test]
    fn fn_call_move_type_ok_when_not_used_after() {
        // Passing a move-type to a function is fine if not used afterward
        let src = r#"
            fn consume(s: str) -> void {
                println(s)
            }
            fn main() -> void {
                let s: str = "hello"
                consume(s)
            }
        "#;
        assert!(check(src).is_ok());
    }

    // ── Trait bounds ──

    #[test]
    fn generic_fn_with_known_trait_bound_ok() {
        let src = r#"
            fn max_val<T: PartialOrd>(a: T, b: T) -> T {
                if a > b { a } else { b }
            }
        "#;
        assert!(check(src).is_ok());
    }

    #[test]
    fn generic_fn_with_multiple_bounds_ok() {
        let src = r#"
            fn display_and_compare<T: Display + PartialEq>(a: T, b: T) -> void {
                println(a)
            }
        "#;
        assert!(check(src).is_ok());
    }

    #[test]
    fn generic_fn_unknown_trait_bound_error() {
        let src = r#"
            fn sort<T: NonexistentTrait>(arr: T) -> T {
                arr
            }
        "#;
        let errors = check_errors(src);
        assert!(errors.iter().any(
            |e| matches!(e, SemanticError::UnknownTrait { name, .. } if name == "NonexistentTrait")
        ));
    }

    #[test]
    fn builtin_traits_registered() {
        let tc = TypeChecker::new();
        // Built-in traits should be registered
        assert!(tc.traits.contains_key("Display"));
        assert!(tc.traits.contains_key("Clone"));
        assert!(tc.traits.contains_key("PartialEq"));
        assert!(tc.traits.contains_key("Ord"));
        assert!(tc.traits.contains_key("Debug"));
        assert!(tc.traits.contains_key("Default"));
        assert!(tc.traits.contains_key("Hash"));
        assert!(tc.traits.contains_key("Copy"));
    }

    #[test]
    fn primitive_types_implement_builtin_traits() {
        let tc = TypeChecker::new();
        // i64 should implement all common traits
        assert!(tc.type_satisfies_trait("i64", "Display"));
        assert!(tc.type_satisfies_trait("i64", "Clone"));
        assert!(tc.type_satisfies_trait("i64", "Copy"));
        assert!(tc.type_satisfies_trait("i64", "PartialEq"));
        assert!(tc.type_satisfies_trait("i64", "Ord"));

        // bool
        assert!(tc.type_satisfies_trait("bool", "Display"));
        assert!(tc.type_satisfies_trait("bool", "Eq"));

        // String
        assert!(tc.type_satisfies_trait("String", "Display"));
        assert!(tc.type_satisfies_trait("String", "Clone"));
    }

    #[test]
    fn user_defined_trait_with_bound_ok() {
        let src = r#"
            trait Printable {
                fn to_str() -> str { "default" }
            }
            fn show<T: Printable>(x: T) -> void {
                println(x)
            }
        "#;
        assert!(check(src).is_ok());
    }

    #[test]
    fn generic_fn_no_bounds_still_works() {
        let src = r#"
            fn identity<T>(x: T) -> T {
                x
            }
        "#;
        assert!(check(src).is_ok());
    }

    // ── S9.4 Move semantics in pattern matching ──

    #[test]
    fn match_destructure_moves_subject() {
        // Destructuring a move-type via pattern matching should mark it moved
        let src = r#"
            fn main() -> void {
                let x: str = "hello"
                match x {
                    _ => println("matched")
                }
                println(x)
            }
        "#;
        // str is a move type. However, wildcard `_` doesn't destructure,
        // so this should NOT trigger use-after-move.
        assert!(check(src).is_ok());
    }

    #[test]
    fn match_enum_destructure_moves_subject() {
        // Destructuring via enum pattern should move the subject (for non-Copy types)
        // Use array instead of str (str is now Copy).
        let src = r#"
            fn main() -> void {
                let x: [i64] = [1, 2, 3]
                match x {
                    Some(inner) => println("got")
                    _ => println("none")
                }
                len(x)
            }
        "#;
        let result = check(src);
        assert!(result.is_err());
        let errs = result.unwrap_err();
        assert!(errs
            .iter()
            .any(|e| matches!(e, SemanticError::UseAfterMove { name, .. } if name == "x")));
    }

    #[test]
    fn match_copy_type_no_move() {
        // Copy types (i64) should not be moved by pattern matching
        let src = r#"
            fn main() -> void {
                let x: i64 = 42
                match x {
                    0 => println("zero")
                    _ => println("other")
                }
                println(x)
            }
        "#;
        assert!(check(src).is_ok());
    }

    // ── Unreachable code after diverging expression ──────────────────

    #[test]
    fn unreachable_after_return_tail_expr() {
        // Tail expression after return should be flagged
        let src = r#"
            fn f() -> i64 {
                return 1
                99
            }
        "#;
        let diags = check_all_diagnostics(src);
        assert!(diags
            .iter()
            .any(|e| matches!(e, SemanticError::UnreachableCode { .. })));
    }

    #[test]
    fn unreachable_after_return_with_multiple_stmts() {
        let src = r#"
            fn f() -> i64 {
                return 1
                let x = 2
                x
            }
        "#;
        let diags = check_all_diagnostics(src);
        assert!(diags
            .iter()
            .any(|e| matches!(e, SemanticError::UnreachableCode { .. })));
    }

    // ── self/&self validation ──────────────────────────────────────────

    #[test]
    fn self_in_free_fn_rejected() {
        let src = r#"
            fn bad(self) -> void {
                0
            }
        "#;
        let result = check(src);
        assert!(result.is_err());
    }

    #[test]
    fn self_in_impl_method_ok() {
        let src = r#"
            struct Point { x: f64, y: f64 }
            impl Point {
                fn get_x(self) -> f64 {
                    self.x
                }
            }
        "#;
        assert!(check(src).is_ok());
    }

    #[test]
    fn self_must_be_first_param() {
        let src = r#"
            struct Foo { val: i64 }
            impl Foo {
                fn bad(x: i64, self) -> void {
                    0
                }
            }
        "#;
        let result = check(src);
        assert!(result.is_err());
    }

    // ── S10.1: Immutable borrows ───────────────────────────────────────

    #[test]
    fn immutable_borrow_returns_ref_type() {
        let src = r#"
            fn main() -> void {
                let x = 42
                let r = &x
            }
        "#;
        assert!(check(src).is_ok());
    }

    #[test]
    fn multiple_immutable_borrows_ok() {
        let src = r#"
            fn main() -> void {
                let x = 42
                let r1 = &x
                let r2 = &x
            }
        "#;
        assert!(check(src).is_ok());
    }

    #[test]
    fn move_while_immutably_borrowed_me003() {
        // str is now Copy, so passing it to a function doesn't move it.
        // Use array (still Move) to test MoveWhileBorrowed.
        let src = r#"
            fn consume(a: [i64]) -> void { println(len(a)) }
            fn main() -> void {
                let a: [i64] = [1, 2, 3]
                let r = &a
                consume(a)
                println(r)
            }
        "#;
        let result = check(src);
        assert!(result.is_err());
        let errs = result.unwrap_err();
        assert!(errs
            .iter()
            .any(|e| matches!(e, SemanticError::MoveWhileBorrowed { name, .. } if name == "a")));
    }

    #[test]
    fn nll_move_after_borrow_last_use_ok() {
        // NLL: r is NOT used after consume(s), so borrow is dead → move OK
        let src = r#"
            fn consume(s: str) -> void { println(s) }
            fn main() -> void {
                let s: str = "hello"
                let r = &s
                println(r)
                consume(s)
            }
        "#;
        assert!(check(src).is_ok());
    }

    #[test]
    fn move_after_borrow_scope_ends_ok() {
        let src = r#"
            fn consume(s: str) -> void { println(s) }
            fn main() -> void {
                let s: str = "hello"
                {
                    let r = &s
                }
                consume(s)
            }
        "#;
        assert!(check(src).is_ok());
    }

    // ── S10.2: Mutable borrows ─────────────────────────────────────────

    #[test]
    fn exclusive_mut_borrow_rejects_second_mut_me004() {
        // r1 is used AFTER r2 creation, so r1's borrow is live → ME004
        let src = r#"
            fn main() -> void {
                let mut x = 42
                let r1 = &mut x
                let r2 = &mut x
                println(r1)
            }
        "#;
        let result = check(src);
        assert!(result.is_err());
        let errs = result.unwrap_err();
        assert!(errs
            .iter()
            .any(|e| matches!(e, SemanticError::MutBorrowConflict { name, .. } if name == "x")));
    }

    #[test]
    fn nll_mut_reborrow_after_last_use_ok() {
        // NLL: r1's last use is before r2 creation → borrow released → OK
        let src = r#"
            fn main() -> void {
                let mut x = 42
                let r1 = &mut x
                println(r1)
                let r2 = &mut x
                println(r2)
            }
        "#;
        assert!(check(src).is_ok());
    }

    #[test]
    fn mut_borrow_rejects_imm_borrow_me005() {
        // r1 (&mut) is used AFTER r2 (&) creation → ME005
        let src = r#"
            fn main() -> void {
                let mut x = 42
                let r1 = &mut x
                let r2 = &x
                println(r1)
            }
        "#;
        let result = check(src);
        assert!(result.is_err());
        let errs = result.unwrap_err();
        assert!(errs
            .iter()
            .any(|e| matches!(e, SemanticError::ImmBorrowConflict { name, .. } if name == "x")));
    }

    #[test]
    fn imm_borrow_rejects_mut_borrow_me004() {
        // r1 (&) is used AFTER r2 (&mut) creation → ME004
        let src = r#"
            fn main() -> void {
                let mut x = 42
                let r1 = &x
                let r2 = &mut x
                println(r1)
            }
        "#;
        let result = check(src);
        assert!(result.is_err());
        let errs = result.unwrap_err();
        assert!(errs
            .iter()
            .any(|e| matches!(e, SemanticError::MutBorrowConflict { name, .. } if name == "x")));
    }

    #[test]
    fn nll_imm_then_mut_after_last_use_ok() {
        // NLL: r1 (&) last use is println(r1), before r2 (&mut) → OK
        let src = r#"
            fn main() -> void {
                let mut x = 42
                let r1 = &x
                println(r1)
                let r2 = &mut x
                println(r2)
            }
        "#;
        assert!(check(src).is_ok());
    }

    #[test]
    fn nll_unused_borrow_immediately_dead() {
        // NLL: r is never used, so borrow is immediately dead → mut borrow OK
        let src = r#"
            fn main() -> void {
                let mut x = 42
                let r = &x
                let r2 = &mut x
                println(r2)
            }
        "#;
        assert!(check(src).is_ok());
    }

    // ── S10.3: Borrow scoping ──────────────────────────────────────────

    #[test]
    fn mut_borrow_after_imm_scope_ends_ok() {
        let src = r#"
            fn main() -> void {
                let mut x = 42
                {
                    let r = &x
                }
                let r2 = &mut x
            }
        "#;
        assert!(check(src).is_ok());
    }

    #[test]
    fn imm_borrow_after_mut_scope_ends_ok() {
        let src = r#"
            fn main() -> void {
                let mut x = 42
                {
                    let r = &mut x
                }
                let r2 = &x
            }
        "#;
        assert!(check(src).is_ok());
    }

    // ── S10.4: NLL borrow checker ────────────────────────────────────────

    #[test]
    fn nll_borrow_live_across_if_branch() {
        // r is used in if branch, so it's live at x = 10 (if x=10 comes after if)
        // Actually r's last use is inside the if, so after if, r is dead → OK
        let src = r#"
            fn main() -> void {
                let mut x = 42
                let r = &x
                if true {
                    println(r)
                }
                x = 10
            }
        "#;
        assert!(check(src).is_ok());
    }

    #[test]
    fn nll_borrow_still_live_in_loop() {
        // r is used inside while loop → extended to loop end → still live
        let src = r#"
            fn main() -> void {
                let mut x = 42
                let r = &x
                let mut i = 0
                while i < 3 {
                    println(r)
                    i = i + 1
                }
                x = 10
            }
        "#;
        // After the loop, r's uses were extended to loop end.
        // x = 10 is after the loop, so r should be dead → OK
        assert!(check(src).is_ok());
    }

    #[test]
    fn nll_reassign_after_last_use_same_scope() {
        // Classic NLL pattern: borrow used then reassign in same scope
        let src = r#"
            fn main() -> void {
                let mut x = 42
                let r = &x
                println(r)
                x = 100
                println(x)
            }
        "#;
        assert!(check(src).is_ok());
    }

    #[test]
    fn nll_reassign_before_use_still_error() {
        // Reassign x BEFORE using r → error (r is live at x = 100)
        let src = r#"
            fn main() -> void {
                let mut x = 42
                let r = &x
                x = 100
                println(r)
            }
        "#;
        let result = check(src);
        assert!(result.is_err());
    }

    // ── B.1: Tensor shape type tests ──

    #[test]
    fn b1_tensor_type_display_known_dims() {
        let t = Type::Tensor {
            element: Box::new(Type::F64),
            dims: vec![Some(3), Some(4)],
        };
        assert_eq!(t.display_name(), "Tensor<f64>[3, 4]");
    }

    #[test]
    fn b1_tensor_type_display_dynamic_dims() {
        let t = Type::Tensor {
            element: Box::new(Type::F32),
            dims: vec![None, Some(10)],
        };
        assert_eq!(t.display_name(), "Tensor<f32>[*, 10]");
    }

    #[test]
    fn b1_tensor_is_tensor() {
        let t = Type::Tensor {
            element: Box::new(Type::F64),
            dims: vec![Some(3)],
        };
        assert!(t.is_tensor());
        assert!(!Type::I64.is_tensor());
    }

    #[test]
    fn b1_tensor_compatible_same_shape() {
        let a = Type::Tensor {
            element: Box::new(Type::F64),
            dims: vec![Some(3), Some(4)],
        };
        let b = Type::Tensor {
            element: Box::new(Type::F64),
            dims: vec![Some(3), Some(4)],
        };
        assert!(a.is_compatible(&b));
    }

    #[test]
    fn b1_tensor_compatible_dynamic_dim() {
        let a = Type::Tensor {
            element: Box::new(Type::F64),
            dims: vec![None, Some(4)],
        };
        let b = Type::Tensor {
            element: Box::new(Type::F64),
            dims: vec![Some(3), Some(4)],
        };
        assert!(a.is_compatible(&b));
    }

    #[test]
    fn b1_tensor_incompatible_different_shape() {
        let a = Type::Tensor {
            element: Box::new(Type::F64),
            dims: vec![Some(3), Some(4)],
        };
        let b = Type::Tensor {
            element: Box::new(Type::F64),
            dims: vec![Some(5), Some(4)],
        };
        assert!(!a.is_compatible(&b));
    }

    #[test]
    fn b1_tensor_incompatible_different_ndims() {
        let a = Type::Tensor {
            element: Box::new(Type::F64),
            dims: vec![Some(3)],
        };
        let b = Type::Tensor {
            element: Box::new(Type::F64),
            dims: vec![Some(3), Some(4)],
        };
        assert!(!a.is_compatible(&b));
    }

    #[test]
    fn b1_tensor_incompatible_different_element() {
        let a = Type::Tensor {
            element: Box::new(Type::F64),
            dims: vec![Some(3)],
        };
        let b = Type::Tensor {
            element: Box::new(Type::I64),
            dims: vec![Some(3)],
        };
        assert!(!a.is_compatible(&b));
    }

    #[test]
    fn b1_matmul_shape_valid() {
        let a = Type::Tensor {
            element: Box::new(Type::F64),
            dims: vec![Some(3), Some(4)],
        };
        let b = Type::Tensor {
            element: Box::new(Type::F64),
            dims: vec![Some(4), Some(5)],
        };
        let result = a.matmul_shape(&b).unwrap();
        assert_eq!(
            result,
            Type::Tensor {
                element: Box::new(Type::F64),
                dims: vec![Some(3), Some(5)],
            }
        );
    }

    #[test]
    fn b1_matmul_shape_dynamic_k() {
        let a = Type::Tensor {
            element: Box::new(Type::F64),
            dims: vec![Some(3), None],
        };
        let b = Type::Tensor {
            element: Box::new(Type::F64),
            dims: vec![None, Some(5)],
        };
        let result = a.matmul_shape(&b).unwrap();
        assert_eq!(
            result,
            Type::Tensor {
                element: Box::new(Type::F64),
                dims: vec![Some(3), Some(5)],
            }
        );
    }

    #[test]
    fn b1_matmul_shape_k_mismatch() {
        let a = Type::Tensor {
            element: Box::new(Type::F64),
            dims: vec![Some(3), Some(4)],
        };
        let b = Type::Tensor {
            element: Box::new(Type::F64),
            dims: vec![Some(7), Some(5)],
        };
        assert!(a.matmul_shape(&b).is_none());
    }

    #[test]
    fn b1_matmul_shape_not_2d() {
        let a = Type::Tensor {
            element: Box::new(Type::F64),
            dims: vec![Some(3)],
        };
        let b = Type::Tensor {
            element: Box::new(Type::F64),
            dims: vec![Some(3), Some(5)],
        };
        assert!(a.matmul_shape(&b).is_none());
    }

    #[test]
    fn b1_elementwise_shape_valid() {
        let a = Type::Tensor {
            element: Box::new(Type::F64),
            dims: vec![Some(3), Some(4)],
        };
        let b = Type::Tensor {
            element: Box::new(Type::F64),
            dims: vec![Some(3), Some(4)],
        };
        let result = a.elementwise_shape(&b).unwrap();
        assert_eq!(
            result,
            Type::Tensor {
                element: Box::new(Type::F64),
                dims: vec![Some(3), Some(4)],
            }
        );
    }

    #[test]
    fn b1_elementwise_shape_mismatch() {
        let a = Type::Tensor {
            element: Box::new(Type::F64),
            dims: vec![Some(3), Some(4)],
        };
        let b = Type::Tensor {
            element: Box::new(Type::F64),
            dims: vec![Some(3), Some(5)],
        };
        assert!(a.elementwise_shape(&b).is_none());
    }

    #[test]
    fn b1_resolve_tensor_type_annotation() {
        // Tensor<f64>[3, 4] type annotation resolves to Type::Tensor
        let src = r#"
            fn process(t: Tensor<f64>[3, 4]) -> void {
                println(t)
            }
        "#;
        assert!(check(src).is_ok());
    }

    #[test]
    fn b1_tensor_zeros_shape_inferred() {
        // tensor_zeros(3, 4) should type-check as Tensor with dynamic shape
        let src = "let t = tensor_zeros(3, 4)";
        assert!(check(src).is_ok());
    }

    #[test]
    fn b1_tensor_matmul_shape_check() {
        // tensor_matmul should accept two tensor arguments
        let src = r#"
            let a = tensor_zeros(3, 4)
            let b = tensor_zeros(4, 5)
            let c = tensor_matmul(a, b)
        "#;
        assert!(check(src).is_ok());
    }

    #[test]
    fn b1_tensor_type_annotation_tensor_param() {
        // Function with Tensor type param accepts tensor values
        let src = r#"
            fn transform(t: Tensor<f64>[*, *]) -> void {
                println(t)
            }
            let a = tensor_zeros(3, 4)
            transform(a)
        "#;
        assert!(check(src).is_ok());
    }

    // ── B.4: Tensor shape hardening tests ──

    #[test]
    fn b4_matmul_operator_annotated_valid() {
        // @ operator with compatible annotated tensor params
        let src = r#"
            fn f(a: Tensor<f64>[3, 4], b: Tensor<f64>[4, 5]) -> void {
                let c = a @ b
                println(c)
            }
        "#;
        assert!(check(src).is_ok());
    }

    #[test]
    fn b4_matmul_operator_annotated_k_mismatch() {
        // @ operator with incompatible K dims → TE001
        let src = r#"
            fn f(a: Tensor<f64>[3, 4], b: Tensor<f64>[7, 5]) -> void {
                let c = a @ b
                println(c)
            }
        "#;
        let errors = check_errors(src);
        assert!(errors
            .iter()
            .any(|e| matches!(e, SemanticError::TensorShapeMismatch { .. })));
    }

    #[test]
    fn b4_matmul_operator_dynamic_no_error() {
        // @ with dynamic tensors (from builtins) → no shape error
        let src = r#"
            let a = tensor_zeros(3, 4)
            let b = tensor_zeros(4, 5)
            let c = a @ b
        "#;
        assert!(check(src).is_ok());
    }

    #[test]
    fn b4_matmul_operator_1d_error() {
        // @ with 1D tensor → TE001 (matmul requires 2D)
        let src = r#"
            fn f(a: Tensor<f64>[3], b: Tensor<f64>[3, 5]) -> void {
                let c = a @ b
                println(c)
            }
        "#;
        let errors = check_errors(src);
        assert!(errors
            .iter()
            .any(|e| matches!(e, SemanticError::TensorShapeMismatch { .. })));
    }

    #[test]
    fn b4_elementwise_annotated_valid() {
        // tensor + tensor with same annotated shapes → OK
        let src = r#"
            fn f(a: Tensor<f64>[3, 4], b: Tensor<f64>[3, 4]) -> void {
                let c = a + b
                println(c)
            }
        "#;
        assert!(check(src).is_ok());
    }

    #[test]
    fn b4_elementwise_annotated_mismatch() {
        // tensor + tensor with different annotated shapes → TE001
        let src = r#"
            fn f(a: Tensor<f64>[3, 4], b: Tensor<f64>[5, 6]) -> void {
                let c = a + b
                println(c)
            }
        "#;
        let errors = check_errors(src);
        assert!(errors
            .iter()
            .any(|e| matches!(e, SemanticError::TensorShapeMismatch { .. })));
    }

    #[test]
    fn b4_elementwise_dynamic_bypass() {
        // tensor + tensor from builtins (dynamic) → no shape error
        let src = r#"
            let a = tensor_zeros(3, 4)
            let b = tensor_zeros(5, 6)
            let c = a + b
        "#;
        assert!(check(src).is_ok());
    }

    #[test]
    fn b4_nested_tensor_type_rejected() {
        // Tensor<Tensor<f64>[3]>[2] → error
        let src = r#"
            fn f(t: Tensor<Tensor<f64>[3]>[2]) -> void {
                println(t)
            }
        "#;
        let errors = check_errors(src);
        assert!(errors
            .iter()
            .any(|e| matches!(e, SemanticError::TypeMismatch { .. })));
    }

    #[test]
    fn b4_elementwise_sub_annotated_mismatch() {
        // tensor - tensor with different shapes → TE001
        let src = r#"
            fn f(a: Tensor<f64>[2, 3], b: Tensor<f64>[4, 3]) -> void {
                let c = a - b
                println(c)
            }
        "#;
        let errors = check_errors(src);
        assert!(errors
            .iter()
            .any(|e| matches!(e, SemanticError::TensorShapeMismatch { .. })));
    }

    #[test]
    fn b4_matmul_result_shape_propagated() {
        // @ result shape: [3,4] @ [4,5] → [3,5]
        let a = Type::Tensor {
            element: Box::new(Type::F64),
            dims: vec![Some(3), Some(4)],
        };
        let b = Type::Tensor {
            element: Box::new(Type::F64),
            dims: vec![Some(4), Some(5)],
        };
        let result = a.matmul_shape(&b).unwrap();
        if let Type::Tensor { dims, .. } = result {
            assert_eq!(dims, vec![Some(3), Some(5)]);
        } else {
            panic!("expected Tensor type");
        }
    }

    // ── F.4: Missing builtin registration tests ──

    #[test]
    fn f4_tensor_detach_registered() {
        let src = r#"
            let t = tensor_zeros(2, 3)
            let d = tensor_detach(t)
        "#;
        assert!(check(src).is_ok(), "tensor_detach should be registered");
    }

    #[test]
    fn f4_tensor_clear_tape_registered() {
        let src = r#"
            tensor_clear_tape()
        "#;
        assert!(check(src).is_ok(), "tensor_clear_tape should be registered");
    }

    #[test]
    fn f4_tensor_no_grad_registered() {
        let src = r#"
            tensor_no_grad_begin()
            tensor_no_grad_end()
        "#;
        assert!(check(src).is_ok(), "tensor_no_grad should be registered");
    }

    // ── F.5: Cast expression type validation tests ──

    #[test]
    fn f5_cast_int_to_float() {
        let src = r#"
            let x: f64 = 42 as f64
        "#;
        assert!(check(src).is_ok(), "int as f64 should be valid");
    }

    #[test]
    fn f5_cast_float_to_int() {
        let src = r#"
            let x: i64 = 3.14 as i64
        "#;
        assert!(check(src).is_ok(), "float as i64 should be valid");
    }

    #[test]
    fn f5_cast_bool_to_int() {
        let src = r#"
            let x: i64 = true as i64
        "#;
        assert!(check(src).is_ok(), "bool as i64 should be valid");
    }

    #[test]
    fn f5_cast_int_to_bool() {
        let src = r#"
            let x: bool = 1 as bool
        "#;
        assert!(check(src).is_ok(), "int as bool should be valid");
    }

    // ── F.6: Missing method registration tests ──

    #[test]
    fn f6_string_trim_start() {
        let src = r#"
            let s = "  hello  "
            let t = s.trim_start()
        "#;
        assert!(check(src).is_ok(), "trim_start should be registered");
    }

    #[test]
    fn f6_string_trim_end() {
        let src = r#"
            let s = "  hello  "
            let t = s.trim_end()
        "#;
        assert!(check(src).is_ok(), "trim_end should be registered");
    }

    #[test]
    fn f6_string_chars() {
        let src = r#"
            let s = "hello"
            let c = s.chars()
        "#;
        assert!(check(src).is_ok(), "chars should be registered");
    }

    #[test]
    fn f6_string_repeat() {
        let src = r#"
            let s = "abc"
            let r = s.repeat(3)
        "#;
        assert!(check(src).is_ok(), "repeat should be registered");
    }

    // ── S14.3: Inline assembly context checks ──

    #[test]
    fn asm_rejected_in_safe_context() {
        let src = r#"
            fn main() {
                asm!("nop")
            }
        "#;
        let errors = check_errors(src);
        assert!(
            errors
                .iter()
                .any(|e| matches!(e, SemanticError::AsmInSafeContext { .. })),
            "asm! should be rejected in @safe context"
        );
    }

    #[test]
    fn asm_rejected_in_device_context() {
        let src = r#"
            @device
            fn compute() {
                asm!("nop")
            }
        "#;
        let errors = check_errors(src);
        assert!(
            errors
                .iter()
                .any(|e| matches!(e, SemanticError::AsmInDeviceContext { .. })),
            "asm! should be rejected in @device context"
        );
    }

    #[test]
    fn asm_allowed_in_kernel_context() {
        let src = r#"
            @kernel
            fn handler() {
                asm!("nop")
            }
        "#;
        let diagnostics = check_all_diagnostics(src);
        assert!(
            !diagnostics.iter().any(|e| matches!(
                e,
                SemanticError::AsmInSafeContext { .. } | SemanticError::AsmInDeviceContext { .. }
            )),
            "asm! should be allowed in @kernel context"
        );
    }

    #[test]
    fn asm_allowed_in_unsafe_context() {
        let src = r#"
            @unsafe
            fn raw_stuff() {
                asm!("nop")
            }
        "#;
        let diagnostics = check_all_diagnostics(src);
        assert!(
            !diagnostics.iter().any(|e| matches!(
                e,
                SemanticError::AsmInSafeContext { .. } | SemanticError::AsmInDeviceContext { .. }
            )),
            "asm! should be allowed in @unsafe context"
        );
    }

    // ── S9.5: Async type checking ──

    #[test]
    fn await_rejected_outside_async() {
        let src = r#"
            fn main() {
                let x = foo().await
            }
            fn foo() -> i64 { 42 }
        "#;
        let errors = check_errors(src);
        assert!(
            errors
                .iter()
                .any(|e| matches!(e, SemanticError::AwaitOutsideAsync { .. })),
            ".await should be rejected outside async fn"
        );
    }

    #[test]
    fn await_allowed_in_async_fn() {
        let src = r#"
            async fn compute() {
                let x = foo().await
            }
            fn foo() -> i64 { 42 }
        "#;
        let diagnostics = check_all_diagnostics(src);
        assert!(
            !diagnostics
                .iter()
                .any(|e| matches!(e, SemanticError::AwaitOutsideAsync { .. })),
            ".await should be allowed in async fn"
        );
    }

    #[test]
    fn await_rejected_in_regular_fn_nested() {
        let src = r#"
            fn outer() {
                let val = something().await
            }
        "#;
        let errors = check_errors(src);
        assert!(
            errors
                .iter()
                .any(|e| matches!(e, SemanticError::AwaitOutsideAsync { .. })),
            ".await in regular fn should error"
        );
    }

    // ── S6.5: Mutex/sync allowed in all contexts ──

    #[test]
    fn mutex_allowed_in_kernel_context() {
        // Mutex::new() call is a path call → analyzer allows it in @kernel
        let src = r#"
            @kernel
            fn smp_handler() {
                let m = Mutex::new(0)
            }
        "#;
        let diagnostics = check_all_diagnostics(src);
        // No KE001/KE002 errors for Mutex in kernel context
        assert!(
            !diagnostics.iter().any(|e| matches!(
                e,
                SemanticError::HeapAllocInKernel { .. } | SemanticError::TensorInKernel { .. }
            )),
            "Mutex should be allowed in @kernel context"
        );
    }

    #[test]
    fn mutex_allowed_in_device_context() {
        let src = r#"
            @device
            fn gpu_sync() {
                let m = Mutex::new(0)
            }
        "#;
        let diagnostics = check_all_diagnostics(src);
        assert!(
            !diagnostics
                .iter()
                .any(|e| matches!(e, SemanticError::RawPointerInDevice { .. })),
            "Mutex should be allowed in @device context"
        );
    }

    // ── S9.2: Future and Poll types ──

    #[test]
    fn future_type_display() {
        let t = Type::Future {
            inner: Box::new(Type::I64),
        };
        assert_eq!(t.display_name(), "Future<i64>");
    }

    #[test]
    fn future_type_compatible_with_self() {
        let a = Type::Future {
            inner: Box::new(Type::I64),
        };
        let b = Type::Future {
            inner: Box::new(Type::I64),
        };
        assert!(a.is_compatible(&b));
    }

    #[test]
    fn future_type_incompatible_different_inner() {
        let a = Type::Future {
            inner: Box::new(Type::I64),
        };
        let b = Type::Future {
            inner: Box::new(Type::Str),
        };
        assert!(!a.is_compatible(&b));
    }

    #[test]
    fn async_fn_has_future_return_type() {
        // async fn foo() -> i64 should have type fn() -> Future<i64>
        let src = r#"
            async fn compute() -> i64 {
                42
            }
        "#;
        let diagnostics = check_all_diagnostics(src);
        // Should compile without type errors (Future<i64> wraps return)
        assert!(
            !diagnostics
                .iter()
                .any(|e| matches!(e, SemanticError::TypeMismatch { .. })),
            "async fn should not produce type mismatch"
        );
    }

    // ── Function pointer types ──

    #[test]
    fn fn_pointer_assignment_valid() {
        let src = r#"
            fn add(a: i64, b: i64) -> i64 { a + b }
            fn main() {
                let f: fn(i64, i64) -> i64 = add
                let result = f(3, 4)
            }
        "#;
        assert!(check(src).is_ok());
    }

    #[test]
    fn fn_pointer_as_parameter() {
        let src = r#"
            fn apply(f: fn(i64) -> i64, x: i64) -> i64 {
                f(x)
            }
            fn double(x: i64) -> i64 { x * 2 }
            fn main() {
                let result = apply(double, 5)
            }
        "#;
        assert!(check(src).is_ok());
    }

    #[test]
    fn fn_pointer_type_mismatch() {
        let src = r#"
            fn add(a: i64, b: i64) -> i64 { a + b }
            fn main() {
                let f: fn(i64) -> i64 = add
            }
        "#;
        let errs = check_errors(src);
        assert!(
            errs.iter()
                .any(|e| matches!(e, SemanticError::TypeMismatch { .. })),
            "should report type mismatch for wrong fn pointer arity"
        );
    }

    #[test]
    fn fn_pointer_call_type_check() {
        let src = r#"
            fn apply(f: fn(i64) -> i64, x: i64) -> i64 {
                f(x)
            }
            fn main() {
                let result: i64 = apply(|x| x + 1, 5)
            }
        "#;
        assert!(check(src).is_ok());
    }

    // ── S5.6: Send/Sync Thread Safety ──

    #[test]
    fn send_check_pass() {
        // i64 is Send — no SE018 error should be emitted
        let src = r#"
            fn worker(x: i64) -> i64 { x * 2 }
            fn main() {
                let h = thread::spawn(worker, 42)
            }
        "#;
        let diagnostics = check_all_diagnostics(src);
        assert!(
            !diagnostics
                .iter()
                .any(|e| matches!(e, SemanticError::NotSendType { .. })),
            "i64 is Send — should not produce SE018"
        );
    }

    #[test]
    fn send_check_fn_only() {
        // Function-only spawn (no data arg) — no Send issue
        let src = r#"
            fn worker() -> i64 { 42 }
            fn main() {
                let h = thread::spawn(worker)
            }
        "#;
        let diagnostics = check_all_diagnostics(src);
        assert!(
            !diagnostics
                .iter()
                .any(|e| matches!(e, SemanticError::NotSendType { .. })),
            "No data arg — should not produce SE018"
        );
    }

    #[test]
    fn sync_check_pass() {
        // f64 is Send+Sync — no SE018 error
        let src = r#"
            fn compute(x: f64) -> i64 { 1 }
            fn main() {
                let h = thread::spawn(compute, 3.14)
            }
        "#;
        let diagnostics = check_all_diagnostics(src);
        assert!(
            !diagnostics
                .iter()
                .any(|e| matches!(e, SemanticError::NotSendType { .. })),
            "f64 is Send — should not produce SE018"
        );
    }

    #[test]
    fn is_send_returns_true_for_primitives() {
        assert!(Type::I64.is_send());
        assert!(Type::F64.is_send());
        assert!(Type::Bool.is_send());
        assert!(Type::Str.is_send());
        assert!(Type::Char.is_send());
        assert!(Type::U8.is_send());
        assert!(Type::I128.is_send());
    }

    #[test]
    fn is_send_returns_true_for_composites() {
        assert!(Type::Array(Box::new(Type::I64)).is_send());
        assert!(Type::Tuple(vec![Type::I64, Type::F64]).is_send());
        assert!(Type::Enum {
            name: "Option".into()
        }
        .is_send());
        assert!(Type::Function {
            params: vec![Type::I64],
            ret: Box::new(Type::I64),
        }
        .is_send());
    }

    #[test]
    fn is_sync_matches_send() {
        assert!(Type::I64.is_sync());
        assert!(Type::Str.is_sync());
        assert!(Type::Array(Box::new(Type::I64)).is_sync());
    }

    // ── S13.3: Borrow checker + concurrency ──

    #[test]
    fn reject_mut_ref_is_not_send() {
        // &mut T is NOT Send — mutable references cannot be shared across threads
        assert!(!Type::RefMut(Box::new(Type::I64)).is_send());
        assert!(!Type::RefMut(Box::new(Type::Str)).is_send());
        assert!(!Type::RefMut(Box::new(Type::F64)).is_send());
    }

    #[test]
    fn immutable_ref_is_send() {
        // &T is Send if T is Send
        assert!(Type::Ref(Box::new(Type::I64)).is_send());
        assert!(Type::Ref(Box::new(Type::Str)).is_send());
    }

    #[test]
    fn allow_move_capture_in_spawn() {
        // Moving a value (i64) to thread::spawn is fine — i64 is Send
        let src = r#"
            fn worker(x: i64) -> i64 { x + 1 }
            fn main() {
                let val = 42
                let h = thread::spawn(worker, val)
            }
        "#;
        let diagnostics = check_all_diagnostics(src);
        assert!(
            !diagnostics
                .iter()
                .any(|e| matches!(e, SemanticError::NotSendType { .. })),
            "Move capture of i64 should be allowed"
        );
    }

    // ── Lifetime annotation tests ───────────────────────────────────────

    #[test]
    fn lifetime_valid_single_input_output() {
        // Single input lifetime, output uses same — valid via elision rule 2
        let src = "fn first<'a>(x: &'a i32) -> &'a i32 { x }";
        let diagnostics = check_all_diagnostics(src);
        assert!(
            !diagnostics
                .iter()
                .any(|e| matches!(e, SemanticError::LifetimeMismatch { .. })),
            "Valid single-lifetime function should pass: {:?}",
            diagnostics
        );
    }

    #[test]
    fn lifetime_undeclared_in_param() {
        // 'b used in param but not declared — should report LifetimeMismatch
        let src = "fn foo(x: &'b i32) -> i32 { 0 }";
        let diagnostics = check_all_diagnostics(src);
        assert!(
            diagnostics
                .iter()
                .any(|e| matches!(e, SemanticError::LifetimeMismatch { .. })),
            "Undeclared lifetime 'b should produce LifetimeMismatch: {:?}",
            diagnostics
        );
    }

    #[test]
    fn lifetime_undeclared_in_return() {
        // 'a used in return type but not declared
        let src = "fn foo(x: i32) -> &'a i32 { x }";
        let diagnostics = check_all_diagnostics(src);
        assert!(
            diagnostics
                .iter()
                .any(|e| matches!(e, SemanticError::LifetimeMismatch { .. })),
            "Undeclared lifetime in return should produce LifetimeMismatch: {:?}",
            diagnostics
        );
    }

    #[test]
    fn lifetime_static_is_always_valid() {
        // 'static is a special built-in lifetime, never requires declaration
        let src = "fn foo(x: &'static i32) -> i32 { 0 }";
        let diagnostics = check_all_diagnostics(src);
        assert!(
            !diagnostics
                .iter()
                .any(|e| matches!(e, SemanticError::LifetimeMismatch { .. })),
            "'static should be valid without declaration: {:?}",
            diagnostics
        );
    }

    #[test]
    fn lifetime_wildcard_is_always_valid() {
        // '_ is a wildcard lifetime, never requires declaration
        let src = "fn foo(x: &'_ i32) -> i32 { 0 }";
        let diagnostics = check_all_diagnostics(src);
        assert!(
            !diagnostics
                .iter()
                .any(|e| matches!(e, SemanticError::LifetimeMismatch { .. })),
            "'_ should be valid without declaration: {:?}",
            diagnostics
        );
    }

    #[test]
    fn lifetime_duplicate_declaration_reports_conflict() {
        // Declaring 'a twice should produce LifetimeConflict
        let src = "fn foo<'a, 'a>(x: &'a i32) -> &'a i32 { x }";
        let diagnostics = check_all_diagnostics(src);
        assert!(
            diagnostics
                .iter()
                .any(|e| matches!(e, SemanticError::LifetimeConflict { .. })),
            "Duplicate lifetime 'a should produce LifetimeConflict: {:?}",
            diagnostics
        );
    }

    #[test]
    fn lifetime_no_annotations_passes() {
        // Functions without any lifetime annotations should pass fine
        let src = "fn add(a: i32, b: i32) -> i32 { a + b }";
        let diagnostics = check_all_diagnostics(src);
        assert!(
            !diagnostics.iter().any(|e| matches!(
                e,
                SemanticError::LifetimeMismatch { .. }
                    | SemanticError::LifetimeConflict { .. }
                    | SemanticError::DanglingReference { .. }
            )),
            "No-lifetime function should have no lifetime errors: {:?}",
            diagnostics
        );
    }
}
