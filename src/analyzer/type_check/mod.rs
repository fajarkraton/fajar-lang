//! Type checker for Fajar Lang.
//!
//! Verifies type correctness, context annotations, and tensor shape compatibility.
//! Walks the AST and produces `SemanticError`s for any inconsistencies.
//!
//! Split into submodules:
//! - `check.rs` — expression and statement type checking
//! - `register.rs` — builtin registration and symbol table initialization

mod check;
mod register;

use std::collections::HashMap;

use crate::lexer::token::Span;
use crate::parser::ast::Program;

use crate::analyzer::scope::{Symbol, SymbolTable};

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
    #[error(
        "SE014: type '{concrete_type}' does not implement trait '{trait_name}' (required by generic bound on '{param_name}')"
    )]
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
    #[error(
        "SE016: method '{method}' in impl {trait_name} for {target_type} has wrong signature: {detail}"
    )]
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
    moves: crate::analyzer::borrow_lite::MoveTracker,
    /// NLL liveness info for the current function body (None outside functions).
    nll_info: Option<crate::analyzer::cfg::NllInfo>,
    /// Enum definitions: enum name → list of variant names (for exhaustiveness).
    enum_variants: HashMap<String, Vec<String>>,
    /// Tracked imports: (import name, span, used) — for unused import detection.
    imports: Vec<(String, Span, bool)>,
}

/// A trait method signature for validation.
#[derive(Debug, Clone)]
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
            // x86_64 port I/O builtins (FajarOS Nova)
            "port_outb",
            "port_inb",
            "x86_serial_init",
            "set_uart_mode_x86",
            "cpuid_eax",
            "cpuid_ebx",
            "cpuid_ecx",
            "cpuid_edx",
            "sse_enable",
            "read_cr0",
            "read_cr4",
            "idt_init",
            "pic_remap",
            "pic_eoi",
            "pit_init",
            "read_timer_ticks",
            "str_byte_at",
            "str_len",
            // Process scheduler builtins (Phase 4)
            "proc_table_addr",
            "get_current_pid",
            "set_current_pid",
            "get_proc_count",
            "proc_create",
            "yield_proc",
            "tss_init",
            "syscall_init",
            "proc_create_user",
            "kb_read_scancode",
            "kb_has_data",
            "pci_read32",
            "pci_write32",
            "volatile_read_u64",
            "volatile_write_u64",
            "buffer_read_u16_le",
            "buffer_read_u32_le",
            "buffer_read_u64_le",
            "buffer_write_u16_le",
            "buffer_write_u32_le",
            "buffer_write_u64_le",
            "buffer_read_u16_be",
            "buffer_read_u32_be",
            "buffer_read_u64_be",
            "buffer_write_u16_be",
            "buffer_write_u32_be",
            "buffer_write_u64_be",
            "acpi_shutdown",
            "acpi_find_rsdp",
            "acpi_get_cpu_count",
            "rdtsc",
            // Phase 5+8: MSR, CR4, INVLPG
            "read_msr",
            "write_msr",
            "read_cr4",
            "write_cr4",
            "invlpg",
            "fxsave",
            "fxrstor",
            "iretq_to_user",
            "rdrand",
            // FajarOS Nova v0.2 system builtins
            "hlt",
            "cli",
            "sti",
            "cpuid",
            "rdmsr",
            "wrmsr",
            // Phase 3 HAL builtins (v3.0 FajarOS)
            "gpio_config",
            "gpio_set_output",
            "gpio_set_input",
            "gpio_set_pull",
            "gpio_set_irq",
            "uart_init",
            "uart_available",
            "spi_init",
            "spi_cs_set",
            "i2c_init",
            "timer_get_ticks",
            "timer_get_freq",
            "timer_set_deadline",
            "timer_enable_virtual",
            "timer_disable_virtual",
            "sleep_us",
            "time_since_boot",
            "timer_mark_boot",
            "dma_alloc",
            "dma_free",
            "dma_config",
            "dma_start",
            "dma_wait",
            "dma_status",
            "dma_barrier",
            // Phase 4: Storage
            "nvme_init",
            "nvme_read",
            "nvme_write",
            "sd_init",
            "sd_read_block",
            "sd_write_block",
            "vfs_mount",
            "vfs_open",
            "vfs_read",
            "vfs_write",
            "vfs_close",
            "vfs_stat",
            // Phase 5: Network
            "eth_init",
            "net_socket",
            "net_bind",
            "net_listen",
            "net_accept",
            "net_connect",
            "net_send",
            "net_recv",
            "net_close",
            // Phase 6: Display & Input
            "fb_init",
            "fb_write_pixel",
            "fb_fill_rect",
            "fb_width",
            "fb_height",
            "kb_init",
            "kb_read",
            "kb_available",
            // Phase 8: OS Services
            "proc_spawn",
            "proc_wait",
            "proc_kill",
            "proc_self",
            "proc_yield",
            "sys_poweroff",
            "sys_reboot",
            "sys_cpu_temp",
            "sys_ram_total",
            "sys_ram_free",
            // Context switch
            "sched_get_saved_sp",
            "sched_set_next_sp",
            "sched_read_proc",
            "sched_write_proc",
            "syscall_arg0",
            "syscall_arg1",
            "syscall_arg2",
            "syscall_set_return",
            "svc",
            "switch_ttbr0",
            "read_ttbr0",
            "tlbi_va",
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
            "tensor_free",
            "tensor_rows",
            "tensor_cols",
            "tensor_set",
            "tensor_row",
            "tensor_normalize",
            "tensor_scale",
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
            moves: crate::analyzer::borrow_lite::MoveTracker::new(),
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
        assert!(
            errors
                .iter()
                .any(|e| matches!(e, SemanticError::UndefinedVariable { .. }))
        );
    }

    #[test]
    fn error_type_mismatch_let() {
        let errors = check_errors("let x: i64 = \"hello\"");
        assert!(
            errors
                .iter()
                .any(|e| matches!(e, SemanticError::TypeMismatch { .. }))
        );
    }

    #[test]
    fn error_immutable_assignment() {
        let errors = check_errors("let x = 1\nx = 2");
        assert!(
            errors
                .iter()
                .any(|e| matches!(e, SemanticError::ImmutableAssignment { .. }))
        );
    }

    #[test]
    fn error_arity_mismatch() {
        let src = "fn f(a: i64) -> i64 { a }\nf(1, 2)";
        let errors = check_errors(src);
        assert!(
            errors
                .iter()
                .any(|e| matches!(e, SemanticError::ArgumentCountMismatch { .. }))
        );
    }

    #[test]
    fn error_missing_struct_field() {
        let src = "struct Point { x: f64, y: f64 }\nlet p = Point { x: 1.0 }";
        let errors = check_errors(src);
        assert!(
            errors
                .iter()
                .any(|e| matches!(e, SemanticError::MissingField { .. }))
        );
    }

    #[test]
    fn error_struct_field_type_mismatch() {
        let src = "struct Point { x: f64, y: f64 }\nlet p = Point { x: \"hi\", y: 2.0 }";
        let errors = check_errors(src);
        assert!(
            errors
                .iter()
                .any(|e| matches!(e, SemanticError::TypeMismatch { .. }))
        );
    }

    #[test]
    fn error_mixed_array_types() {
        let errors = check_errors("[1, \"hello\"]");
        assert!(
            errors
                .iter()
                .any(|e| matches!(e, SemanticError::TypeMismatch { .. }))
        );
    }

    #[test]
    fn error_fn_return_type_mismatch() {
        let src = "fn f() -> i64 { \"hello\" }";
        let errors = check_errors(src);
        assert!(
            errors
                .iter()
                .any(|e| matches!(e, SemanticError::TypeMismatch { .. }))
        );
    }

    #[test]
    fn error_argument_type_mismatch() {
        let src = "fn f(a: i64) -> i64 { a }\nf(\"hello\")";
        let errors = check_errors(src);
        assert!(
            errors
                .iter()
                .any(|e| matches!(e, SemanticError::TypeMismatch { .. }))
        );
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
        assert!(
            errors
                .iter()
                .any(|e| matches!(e, SemanticError::TypeMismatch { .. }))
        );
    }

    #[test]
    fn error_f32_not_assignable_to_f64() {
        let src = "fn f(x: f32) -> f32 { x }\nlet y: f64 = f(1.0)";
        let errors = check_errors(src);
        assert!(
            errors
                .iter()
                .any(|e| matches!(e, SemanticError::TypeMismatch { .. }))
        );
    }

    #[test]
    fn error_mixed_int_arithmetic() {
        let src = r#"
            fn get_i32() -> i32 { 1 }
            fn get_i64() -> i64 { 2 }
            let x = get_i32() + get_i64()
        "#;
        let errors = check_errors(src);
        assert!(
            errors
                .iter()
                .any(|e| matches!(e, SemanticError::TypeMismatch { .. }))
        );
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
        assert!(
            errors
                .iter()
                .any(|e| matches!(e, SemanticError::TypeMismatch { .. }))
        );
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
        assert!(
            diags
                .iter()
                .any(|e| matches!(e, SemanticError::UnreachableCode { .. }))
        );
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
        assert!(
            errors
                .iter()
                .any(|e| matches!(e, SemanticError::NonExhaustiveMatch { .. }))
        );
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
        assert!(
            errors
                .iter()
                .any(|e| matches!(e, SemanticError::BreakOutsideLoop { .. }))
        );
    }

    #[test]
    fn error_continue_outside_loop() {
        let src = r#"
            fn f() -> void {
                continue
            }
        "#;
        let errors = check_errors(src);
        assert!(
            errors
                .iter()
                .any(|e| matches!(e, SemanticError::BreakOutsideLoop { .. }))
        );
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
        assert!(
            errors
                .iter()
                .any(|e| matches!(e, SemanticError::ReturnOutsideFunction { .. }))
        );
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
        assert!(
            errors
                .iter()
                .any(|e| matches!(e, SemanticError::RawPointerInDevice { .. }))
        );
    }

    #[test]
    fn device_fn_cannot_call_kernel_fn() {
        let src = r#"
            @kernel fn kern_init() -> i64 { 0 }
            @device fn bad() -> i64 { kern_init() }
        "#;
        let errors = check_errors(src);
        assert!(
            errors
                .iter()
                .any(|e| matches!(e, SemanticError::KernelCallInDevice { .. }))
        );
    }

    #[test]
    fn kernel_fn_cannot_call_device_fn() {
        let src = r#"
            @device fn infer() -> i64 { 0 }
            @kernel fn bad() -> i64 { infer() }
        "#;
        let errors = check_errors(src);
        assert!(
            errors
                .iter()
                .any(|e| matches!(e, SemanticError::DeviceCallInKernel { .. }))
        );
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
        assert!(
            errors
                .iter()
                .any(|e| matches!(e, SemanticError::RawPointerInDevice { .. }))
        );
    }

    #[test]
    fn device_fn_cannot_call_port_write() {
        let src = "@device fn bad() { port_write(128, 42) }";
        let errors = check_errors(src);
        assert!(
            errors
                .iter()
                .any(|e| matches!(e, SemanticError::RawPointerInDevice { .. }))
        );
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
        assert!(
            errors
                .iter()
                .any(|e| matches!(e, SemanticError::HeapAllocInKernel { .. }))
        );
    }

    #[test]
    fn kernel_fn_cannot_call_to_string() {
        let src = r#"@kernel fn bad() { to_string(42) }"#;
        let errors = check_errors(src);
        assert!(
            errors
                .iter()
                .any(|e| matches!(e, SemanticError::HeapAllocInKernel { .. }))
        );
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
        assert!(
            errors
                .iter()
                .any(|e| matches!(e, SemanticError::HeapAllocInKernel { .. }))
        );
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
        assert!(
            errors
                .iter()
                .any(|e| matches!(e, SemanticError::HeapAllocInKernel { .. }))
        );
        assert!(
            errors
                .iter()
                .any(|e| matches!(e, SemanticError::DeviceCallInKernel { .. }))
        );
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
        assert!(
            errors
                .iter()
                .any(|e| matches!(e, SemanticError::DuplicateDefinition { .. }))
        );
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
        assert!(
            !diagnostics
                .iter()
                .any(|e| matches!(e, SemanticError::MissingField { .. }))
        );
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
        assert!(
            !errors
                .iter()
                .any(|e| matches!(e, SemanticError::TraitMethodSignatureMismatch { .. }))
        );
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
        assert!(
            errs.iter()
                .any(|e| matches!(e, SemanticError::FfiUnsafeType { .. }))
        );
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
        assert!(
            errs.iter()
                .any(|e| matches!(e, SemanticError::FfiUnsafeType { .. }))
        );
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
        assert!(
            errs.iter()
                .any(|e| matches!(e, SemanticError::NonExhaustiveMatch { .. }))
        );
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
        assert!(
            errors
                .iter()
                .any(|e| matches!(e, SemanticError::NonExhaustiveMatch { .. }))
        );
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
        assert!(
            errors
                .iter()
                .any(|e| matches!(e, SemanticError::NonExhaustiveMatch { .. }))
        );
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
        assert!(
            errors
                .iter()
                .any(|e| matches!(e, SemanticError::NonExhaustiveMatch { .. }))
        );
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
        assert!(
            errors
                .iter()
                .any(|e| matches!(e, SemanticError::NonExhaustiveMatch { .. }))
        );
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
        assert!(
            errors
                .iter()
                .any(|e| matches!(e, SemanticError::AwaitOutsideAsync { .. }))
        );
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
        assert!(
            errs.iter()
                .any(|e| matches!(e, SemanticError::UseAfterMove { name, .. } if name == "a"))
        );
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
        assert!(
            errs.iter()
                .any(|e| matches!(e, SemanticError::UseAfterMove { name, .. } if name == "a"))
        );
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
        assert!(
            errs.iter()
                .any(|e| matches!(e, SemanticError::UseAfterMove { name, .. } if name == "x"))
        );
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
        assert!(
            diags
                .iter()
                .any(|e| matches!(e, SemanticError::UnreachableCode { .. }))
        );
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
        assert!(
            diags
                .iter()
                .any(|e| matches!(e, SemanticError::UnreachableCode { .. }))
        );
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
        assert!(
            errs.iter()
                .any(|e| matches!(e, SemanticError::MoveWhileBorrowed { name, .. } if name == "a"))
        );
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
        assert!(
            errs.iter()
                .any(|e| matches!(e, SemanticError::MutBorrowConflict { name, .. } if name == "x"))
        );
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
        assert!(
            errs.iter()
                .any(|e| matches!(e, SemanticError::ImmBorrowConflict { name, .. } if name == "x"))
        );
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
        assert!(
            errs.iter()
                .any(|e| matches!(e, SemanticError::MutBorrowConflict { name, .. } if name == "x"))
        );
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
        assert!(
            errors
                .iter()
                .any(|e| matches!(e, SemanticError::TensorShapeMismatch { .. }))
        );
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
        assert!(
            errors
                .iter()
                .any(|e| matches!(e, SemanticError::TensorShapeMismatch { .. }))
        );
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
        assert!(
            errors
                .iter()
                .any(|e| matches!(e, SemanticError::TensorShapeMismatch { .. }))
        );
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
        assert!(
            errors
                .iter()
                .any(|e| matches!(e, SemanticError::TypeMismatch { .. }))
        );
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
        assert!(
            errors
                .iter()
                .any(|e| matches!(e, SemanticError::TensorShapeMismatch { .. }))
        );
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
        assert!(
            Type::Enum {
                name: "Option".into()
            }
            .is_send()
        );
        assert!(
            Type::Function {
                params: vec![Type::I64],
                ret: Box::new(Type::I64),
            }
            .is_send()
        );
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
