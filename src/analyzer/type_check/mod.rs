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

use crate::const_generics;
use crate::const_traits;
use crate::dependent;
use crate::lexer::token::Span;
use crate::parser::ast::{Program, TensorDimExpr};

use crate::analyzer::scope::{Symbol, SymbolTable};

/// P3 (Compass §6.3): annotation-level shape info of one user fn whose
/// signature mentions a symbolic dim. Captured at registration
/// (`register_item`) because symbolic dims erase to dynamic in `Type`.
#[derive(Debug, Clone, Default)]
pub(crate) struct SymbolicFnShape {
    /// Per-param: `Some(dims)` when the param annotation is a Tensor type.
    pub(crate) params: Vec<Option<Vec<TensorDimExpr>>>,
    /// Return annotation dims when the return type is a Tensor.
    pub(crate) ret: Option<Vec<TensorDimExpr>>,
}

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
    /// Immutable reference: `&'a T`. The `Option<u32>` is the lifetime ID
    /// (`None` = elided/anonymous, `Some(0)` = 'static, `Some(n)` = named).
    Ref(Box<Type>, Option<u32>),
    /// Mutable reference: `&'a mut T`. Same lifetime semantics as `Ref`.
    RefMut(Box<Type>, Option<u32>),
    /// A tensor type with element type and optional shape dimensions.
    /// `None` dimensions are dynamic (unknown at compile time).
    Tensor {
        /// Element type (e.g., F32, F64).
        element: Box<Type>,
        /// Shape dimensions. `None` = dynamic, `Some(n)` = known size.
        dims: Vec<Option<u64>>,
    },
    /// A quantized tensor type: `Quantized<T, BITS>`.
    /// Stays quantized until explicitly dequantized. Using a Quantized
    /// value where a Tensor is expected is SE017.
    Quantized {
        /// Element type (e.g., F32, F64).
        element: Box<Type>,
        /// Bit width (2, 3, 4, or 8).
        bits: u8,
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
            Type::Ref(inner, _) => inner.is_send(),
            // Mutable references: NOT Send — sharing &mut across threads is a data race
            Type::RefMut(..) => false,
            // Tensors/Quantized: Send
            Type::Tensor { .. } | Type::Quantized { .. } => true,
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
            Type::Ref(inner, _) => format!("&{}", inner.display_name()),
            Type::RefMut(inner, _) => format!("&mut {}", inner.display_name()),
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
            Type::Quantized { element, bits } => {
                format!("Quantized<{}, {}>", element.display_name(), bits)
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
        if let (Type::Ref(a, _), Type::Ref(b, _)) = (self, other) {
            return a.is_compatible(b);
        }
        if let (Type::RefMut(a, _), Type::RefMut(b, _)) = (self, other) {
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
        // Quantized compatibility: bits=0 is polymorphic (matches any bit width)
        if let (
            Type::Quantized {
                element: ea,
                bits: ba,
            },
            Type::Quantized {
                element: eb,
                bits: bb,
            },
        ) = (self, other)
        {
            return ea.is_compatible(eb) && (*ba == 0 || *bb == 0 || ba == bb);
        }
        // Unknown is compatible with Quantized
        if matches!(
            (self, other),
            (Type::Unknown, Type::Quantized { .. }) | (Type::Quantized { .. }, Type::Unknown)
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
    #[error("SE005: expected {expected} arguments, found {found}{}", hint.as_ref().map(|h| format!(" ({h})")).unwrap_or_default())]
    ArgumentCountMismatch {
        /// Expected count.
        expected: usize,
        /// Actual count.
        found: usize,
        /// Source location.
        span: Span,
        /// Optional hint.
        hint: Option<String>,
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
    #[error(
        "SE007: cannot assign to immutable variable '{name}' — declare with `let mut {name}` to allow mutation"
    )]
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
    #[error("SE009: unused variable '{name}' — prefix with underscore `_{name}` if intentional")]
    UnusedVariable {
        /// Variable name.
        name: String,
        /// Source location.
        span: Span,
    },

    /// ME010: Linear value not consumed (error).
    /// Linear types must be used exactly once. Failing to consume a linear
    /// value means a resource was leaked (e.g., file handle not closed).
    #[error("ME010: linear value '{name}' not consumed — must be used exactly once")]
    LinearNotConsumed {
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

    /// SE020: Hardware access in @safe context (microkernel isolation).
    #[error("SE020: hardware access not allowed in @safe context — use syscall instead")]
    HardwareAccessInSafe {
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
    #[error(
        "ME001: use of moved variable '{name}' (moved at byte {move_start})",
        move_start = move_span.start
    )]
    UseAfterMove {
        /// Variable name.
        name: String,
        /// Where it was used.
        span: Span,
        /// Where it was moved.
        move_span: Span,
    },

    /// ME003: Cannot move while borrowed.
    #[error(
        "ME003: cannot move '{name}' because it is borrowed (borrow at byte {borrow_start})",
        borrow_start = borrow_span.start
    )]
    MoveWhileBorrowed {
        /// Variable name.
        name: String,
        /// Where the move was attempted.
        span: Span,
        /// Where the borrow was created.
        borrow_span: Span,
    },

    /// ME004: Cannot borrow mutably while already borrowed.
    #[error(
        "ME004: cannot borrow '{name}' as mutable because it is also borrowed (borrow at byte {borrow_start})",
        borrow_start = borrow_span.start
    )]
    MutBorrowConflict {
        /// Variable name.
        name: String,
        /// Where the conflicting borrow was attempted.
        span: Span,
        /// Where the existing borrow was created.
        borrow_span: Span,
    },

    /// ME005: Cannot borrow immutably while mutably borrowed.
    #[error(
        "ME005: cannot borrow '{name}' as immutable because it is mutably borrowed (mutable borrow at byte {borrow_start})",
        borrow_start = borrow_span.start
    )]
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

    /// TE011: Symbolic dimension mismatch at a call site (P3, Compass §6.3).
    /// A signature symbol (e.g. `I` in `Tensor<f64>[B, I]`) bound to two
    /// different sizes by the arguments of one call.
    #[error(
        "TE011: symbolic dim mismatch in call to `{fn_name}`: {symbol} bound to {first}, but argument {arg_index} requires {symbol} = {second}"
    )]
    SymbolicDimMismatch {
        /// Callee function name.
        fn_name: String,
        /// The symbolic dimension name.
        symbol: String,
        /// First bound size.
        first: u64,
        /// Conflicting size.
        second: u64,
        /// 1-based argument index that introduced the conflict.
        arg_index: usize,
        /// Span of the conflicting argument.
        span: Span,
    },

    /// SE022: Compile-time array index out of bounds.
    #[error("SE022: array index {index} out of bounds (length {length})")]
    IndexOutOfBounds {
        /// The index value.
        index: i64,
        /// The array length.
        length: u64,
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

    /// SE023: Quantized tensor used where Tensor expected — must dequantize first.
    #[error(
        "SE023: cannot use Quantized<{element}, {bits}> where Tensor is expected — call dequantize() first"
    )]
    QuantizedNotDequantized {
        /// The element type of the quantized tensor.
        element: String,
        /// The bit width.
        bits: u8,
        /// Source location.
        span: Span,
    },

    /// SE024: Use of `[T]` array after it was moved (consumed by prior use).
    ///
    /// Per FJARR_LEAK Phase 2 (Strategy D / linear-types-lite), `[T]`
    /// values are affine: each binding is consumed exactly once.
    /// Triggers always-on (independent of `strict_ownership` flag),
    /// because Phase 1's arena-still-heap caveat (Compass §4.1) means
    /// arrays specifically need compile-time use-after-move enforcement.
    /// For non-array non-Copy types, see ME001 (gated by strict mode).
    #[error(
        "SE024: use of moved `[T]` array '{name}' (moved at byte {move_start})",
        move_start = move_span.start
    )]
    UseAfterMoveArray {
        /// Variable name.
        name: String,
        /// Where it was used.
        span: Span,
        /// Where it was moved.
        move_span: Span,
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

    // ── Effect System Errors ────────────────────────────────────────────
    /// EE001: Undeclared effect — function performs an effect not in its `with` clause.
    #[error(
        "EE001: function '{function}' performs effect '{effect}' not declared in its `with` clause"
    )]
    UndeclaredEffect {
        /// Function name.
        function: String,
        /// Effect name that was performed but not declared.
        effect: String,
        /// Source location.
        span: Span,
    },

    /// EE002: Unknown effect — effect name not found in registry.
    #[error("EE002: unknown effect '{name}'")]
    UnknownEffect {
        /// The unknown effect name.
        name: String,
        /// Source location.
        span: Span,
    },

    /// EE006: Effect forbidden by context — effect not allowed in @kernel/@device/@safe.
    #[error("EE006: effect '{effect}' is forbidden in {context} context")]
    EffectForbiddenInContext {
        /// The forbidden effect.
        effect: String,
        /// The context (e.g., "@kernel").
        context: String,
        /// Source location.
        span: Span,
    },

    /// EE005: Resume outside handler — `resume()` used outside a `handle` expression.
    #[error("EE005: `resume` can only be used inside a `handle` expression")]
    ResumeOutsideHandler {
        /// Source location.
        span: Span,
    },

    /// EE004: Duplicate effect declaration.
    #[error("EE004: effect '{name}' is already declared")]
    DuplicateEffectDecl {
        /// The duplicate effect name.
        name: String,
        /// Source location.
        span: Span,
    },

    // ── IPC Type Safety Errors ──────────────────────────────────────────
    /// IPC001: @message struct exceeds 64-byte limit.
    #[error("IPC001: @message struct '{name}' exceeds 64-byte IPC limit ({size} bytes)")]
    MessageTooLarge {
        /// Struct name.
        name: String,
        /// Actual size in bytes.
        size: usize,
        /// Source location.
        span: Span,
    },

    /// IPC002: ipc_send/recv with non-@message type.
    #[error("IPC002: ipc_send/recv expects @message struct, got '{found}'")]
    IpcTypeMismatch {
        /// The type that was passed.
        found: String,
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
            | SemanticError::LinearNotConsumed { span, .. }
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
            | SemanticError::HardwareAccessInSafe { span, .. }
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
            | SemanticError::SymbolicDimMismatch { span, .. }
            | SemanticError::IndexOutOfBounds { span, .. }
            | SemanticError::UnusedImport { span, .. }
            | SemanticError::UnreachablePattern { span, .. }
            | SemanticError::LifetimeMismatch { span, .. }
            | SemanticError::LifetimeConflict { span, .. }
            | SemanticError::DanglingReference { span, .. }
            | SemanticError::RawPointerInNpu { span, .. }
            | SemanticError::HeapAllocInNpu { span, .. }
            | SemanticError::OsPrimitiveInNpu { span, .. }
            | SemanticError::KernelCallInNpu { span, .. }
            | SemanticError::UndeclaredEffect { span, .. }
            | SemanticError::UnknownEffect { span, .. }
            | SemanticError::EffectForbiddenInContext { span, .. }
            | SemanticError::ResumeOutsideHandler { span, .. }
            | SemanticError::DuplicateEffectDecl { span, .. }
            | SemanticError::MessageTooLarge { span, .. }
            | SemanticError::IpcTypeMismatch { span, .. }
            | SemanticError::QuantizedNotDequantized { span, .. }
            | SemanticError::UseAfterMoveArray { span, .. } => *span,
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

    /// Returns a suggestion hint for this error, if applicable.
    ///
    /// Hints guide the user toward fixing ownership/borrow errors with
    /// actionable suggestions (e.g., "consider cloning the value").
    pub fn hint(&self) -> Option<String> {
        match self {
            // P4 (Compass §6.3): domain hints for compile-time shape errors.
            SemanticError::TensorShapeMismatch { .. } => Some(
                "help: tensor shapes are inferred from constructor literals (`zeros(2, 3)`) \
                 and annotations (`Tensor<f64>[2, 3]`), and propagate through \
                 matmul/reshape/transpose/elementwise. Trace each operand back to its \
                 constructor or annotation; use non-literal dims or `Tensor<f64>[*, n]` \
                 for shapes only known at runtime"
                    .to_string(),
            ),
            SemanticError::SymbolicDimMismatch { symbol, .. } => Some(format!(
                "help: every occurrence of `{symbol}` in one call must bind the same size \
                 (symbolic dims are fn-signature-scoped, unified per call site). Fix the \
                 mismatched argument, or replace `{symbol}` with `*` in the signature if \
                 the dims are genuinely unrelated"
            )),
            SemanticError::UseAfterMove {
                name, move_span, ..
            } => Some(format!(
                "help: `{name}` was moved at byte offset {}. Consider cloning: `let copy = {name}.clone()` before the move, or use a reference instead",
                move_span.start
            )),
            SemanticError::UseAfterMoveArray {
                name, move_span, ..
            } => Some(format!(
                "help: `[T]` array `{name}` was moved at byte offset {}. Per FJARR_LEAK Phase 2 (Strategy D), `[T]` is affine. Insert `{name}.clone()` at the prior consume site to keep `{name}` available, or restructure so each array binding is used exactly once",
                move_span.start
            )),
            SemanticError::MoveWhileBorrowed {
                name, borrow_span, ..
            } => Some(format!(
                "help: `{name}` is borrowed (from byte offset {}). Ensure the borrow is no longer used before moving, or clone the value",
                borrow_span.start
            )),
            SemanticError::MutBorrowConflict {
                name, borrow_span, ..
            } => Some(format!(
                "help: `{name}` is already borrowed (from byte offset {}). Only one mutable borrow, or multiple immutable borrows, are allowed at a time. Consider narrowing the borrow scope",
                borrow_span.start
            )),
            SemanticError::ImmBorrowConflict {
                name, borrow_span, ..
            } => Some(format!(
                "help: `{name}` is mutably borrowed (from byte offset {}). Cannot create an immutable borrow while a mutable borrow is active. Consider reordering operations",
                borrow_span.start
            )),
            SemanticError::DanglingReference { lifetime, .. } => Some(format!(
                "help: reference with lifetime '{lifetime}' would outlive the data it points to. Return an owned value instead of a reference, or ensure the referent lives long enough"
            )),
            SemanticError::LifetimeConflict { name, .. } => Some(format!(
                "help: lifetime '{name}' conflicts with another in scope. Use distinct lifetime names for independent references"
            )),
            SemanticError::LifetimeMismatch {
                expected, found, ..
            } => Some(format!(
                "help: expected lifetime '{expected} but found '{found}. Ensure the reference's lifetime matches the required bound"
            )),
            SemanticError::LinearNotConsumed { name, .. } => Some(format!(
                "help: linear resource `{name}` must be consumed (moved or explicitly dropped) before it goes out of scope"
            )),
            _ => None,
        }
    }

    /// Returns the secondary span for this error (e.g., the move/borrow origin).
    ///
    /// Used by diagnostic renderers to show "previously moved here" or
    /// "borrow created here" labels.
    pub fn secondary_span(&self) -> Option<(Span, &'static str)> {
        match self {
            SemanticError::UseAfterMove { move_span, .. } => Some((*move_span, "value moved here")),
            SemanticError::UseAfterMoveArray { move_span, .. } => {
                Some((*move_span, "array moved here"))
            }
            SemanticError::MoveWhileBorrowed { borrow_span, .. } => {
                Some((*borrow_span, "borrow created here"))
            }
            SemanticError::MutBorrowConflict { borrow_span, .. } => {
                Some((*borrow_span, "previous borrow here"))
            }
            SemanticError::ImmBorrowConflict { borrow_span, .. } => {
                Some((*borrow_span, "mutable borrow here"))
            }
            _ => None,
        }
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
    /// Hardware builtins blocked in @safe context (microkernel isolation).
    safe_blocked_builtins: std::collections::HashSet<String>,
    /// Structs annotated with @message (IPC message types, max 64 bytes).
    message_structs: std::collections::HashSet<String>,
    /// Auto-generated protocol client struct names (e.g., "VfsProtocolClient").
    protocol_clients: std::collections::HashSet<String>,
    /// Message ID assignments: @message struct name → unique ID (auto-incremented).
    message_ids: HashMap<String, u32>,
    /// Next message ID to assign.
    next_message_id: u32,
    /// Capability sets: which hardware builtins each capability allows.
    /// @device functions have a capability parameter (e.g., @device("net"))
    /// that restricts which builtins they can call.
    /// Current @device capability parameter (e.g., "net", "blk", "port_io").
    current_device_cap: Option<String>,
    cap_port_io: std::collections::HashSet<String>,
    cap_irq: std::collections::HashSet<String>,
    cap_dma: std::collections::HashSet<String>,
    cap_net: std::collections::HashSet<String>,
    cap_blk: std::collections::HashSet<String>,
    /// Builtins that perform heap allocation (forbidden in @kernel).
    heap_builtins: std::collections::HashSet<String>,
    /// Builtins that perform tensor/ML operations (forbidden in @kernel).
    tensor_builtins: std::collections::HashSet<String>,
    /// V18: Functions that transitively use tensor ops (tainted for @kernel).
    tensor_tainted_fns: std::collections::HashSet<String>,
    /// V18: Functions that transitively use OS builtins (tainted for @device).
    os_tainted_fns: std::collections::HashSet<String>,
    /// V25: Functions that transitively use heap builtins (tainted for @kernel).
    heap_tainted_fns: std::collections::HashSet<String>,
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
    /// Linear variables: name → (span, consumed). Must be consumed exactly once.
    linear_vars: HashMap<String, (Span, bool)>,
    /// Effect registry: tracks declared effects and their operations.
    effect_registry: crate::analyzer::effects::EffectRegistry,
    /// Function effect signatures: fn name → declared effect names.
    fn_effects: HashMap<String, Vec<String>>,
    /// Whether we are inside a handle expression (allows resume).
    in_handle_expr: bool,
    /// V15: Expected return type for `resume(val)` in the current handler arm.
    /// Set when checking a handler arm body, based on the effect op's declared return type.
    current_handler_resume_type: Option<Type>,
    /// V14: Current function name being checked (for effect inference).
    current_fn_name: Option<String>,
    /// P2 (Compass §6.3): declared return type of the function whose body
    /// is being checked, so explicit `return` statements can be verified.
    /// `None` inside closures (their return type is inferred, not declared).
    current_fn_return: Option<Type>,
    /// P3 (Compass §6.3 / D3a): symbolic shape signatures of user fns whose
    /// annotations mention a symbolic dim (`Tensor<f64>[B, I]`), keyed by fn
    /// name. Used for per-call-site unification (TE011) + return
    /// substitution in `check_call`.
    symbolic_fn_shapes: HashMap<String, SymbolicFnShape>,
    /// V14: Effects inferred from the current function body.
    current_fn_inferred_effects: std::collections::BTreeSet<String>,
    /// V14: Effects handled by enclosing handle blocks (don't require `with`).
    handled_effects_in_scope: std::collections::BTreeSet<String>,
    /// Strict ownership mode: String/Array/Struct/Tensor are Move types.
    /// When false (default), all types are Copy (interpreter semantics).
    /// Enabled by `--strict-ownership` CLI flag.
    strict_ownership: bool,
    /// Lifetime environment: maps lifetime name → unique ID.
    /// Populated per-function from `lifetime_params`. `'static` is always ID 0.
    lifetime_env: HashMap<String, u32>,
    /// Next lifetime ID to assign (starts at 1; 0 = 'static).
    next_lifetime_id: u32,
    /// Const trait registry for compile-time trait bound checking.
    #[allow(dead_code)]
    const_trait_registry: const_traits::ConstTraitRegistry,
    /// Dependent type shape checker for tensor/array shape verification.
    dep_shape_env: HashMap<String, dependent::nat::NatValue>,
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
            // FajarOS Nova v0.3 Stage A: Extended Port I/O
            "port_inw",
            "port_ind",
            "port_outw",
            "port_outd",
            // FajarOS Nova v0.3 Stage A: CPU Control
            "ltr",
            "lgdt_mem",
            "lidt_mem",
            "swapgs",
            "int_n",
            "pause",
            "stac",
            "clac",
            // FajarOS Nova v0.3 Stage A: Buffer Operations
            "memcmp_buf",
            "memcpy_buf",
            "memset_buf",
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
            "http_listen",
            // Phase 6: Display & Input
            "fb_init",
            "fb_write_pixel",
            "fb_fill_rect",
            "fb_width",
            "fb_height",
            "fb_set_base",
            "fb_scroll",
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
            // V27.5 P1.2: AI scheduler
            "tensor_workload_hint",
            "schedule_ai_task",
            // V27.5 P4.2: Capability builtins
            "cap_new",
            "cap_unwrap",
            "cap_is_valid",
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
            // Volatile memory access (hardware-level)
            "volatile_read",
            "volatile_write",
            "volatile_read_u8",
            "volatile_write_u8",
            "volatile_read_u16",
            "volatile_write_u16",
            "volatile_read_u32",
            "volatile_write_u32",
            // Note: volatile_read_u64/write_u64 already listed above
            "read_cr3",
            "write_cr3",
            "read_cr2",
            "memory_fence",
            "fn_addr",
            "sleep_ms",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();

        // @safe blocked builtins = os_builtins + volatile + CR3/CR2 + buffer LE/BE
        // — MINUS names whose semantics are purely-functional (no hardware
        // access, no global mutable state, no kernel side effect). These were
        // inherited from os_builtins because they originated in OS-prep phases
        // but are language-level operations on language-managed values.
        //
        // Audit per `docs/SAFE_BLOCKED_BUILTINS_AUDIT.md` (2026-05-10).
        // Carve-out criterion: (1) no real-world side effect, (2) operates on
        // language-managed values (not raw memory or kernel state),
        // (3) needed by safe code legitimately.
        //
        // Carved-out names:
        // - `str_byte_at`, `str_len`           : pure Rust-`str` byte ops (v35.6.0)
        // - `tensor_workload_hint`             : pure FLOP-count estimator math
        // - `cap_new`, `cap_unwrap`, `cap_is_valid`: language-level Cap<T> type ops
        //                                          (analogous to Option's Some/None)
        //
        // KEPT-BLOCKED (despite read-only semantics): rdtsc, cpuid_*, read_cr*,
        // read_msr, time_since_boot, timer_get_freq, sys_cpu_temp, sys_ram_*,
        // get_current_pid, get_proc_count, proc_self, proc_table_addr —
        // these read privileged CPU/kernel state and information leaks are
        // conservatively a @safe violation. Native codegen also emits real
        // hw instructions for these, so they are NOT pure-functional in the
        // codegen-output sense even if the language-level effect is read-only.
        let mut safe_blocked_builtins = os_builtins.clone();
        safe_blocked_builtins.remove("str_byte_at");
        safe_blocked_builtins.remove("str_len");
        safe_blocked_builtins.remove("tensor_workload_hint");
        safe_blocked_builtins.remove("cap_new");
        safe_blocked_builtins.remove("cap_unwrap");
        safe_blocked_builtins.remove("cap_is_valid");
        for extra in [
            "volatile_read",
            "volatile_write",
            "volatile_read_u8",
            "volatile_write_u8",
            "volatile_read_u16",
            "volatile_write_u16",
            "volatile_read_u32",
            "volatile_write_u32",
            "volatile_read_u64",
            "volatile_write_u64",
            "read_cr3",
            "write_cr3",
            "read_cr2",
            "write_cr4",
            "invlpg",
            "memory_fence",
            "fn_addr",
            "read_msr",
            "write_msr",
            "rdtsc",
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
            "acpi_find_rsdp",
            "acpi_get_cpu_count",
            "acpi_shutdown",
            "sleep_ms",
            // V18 2.10: FFI builtins — unsafe by nature
            "ffi_load_library",
            "ffi_call",
            "ffi_close",
        ] {
            safe_blocked_builtins.insert(extra.to_string());
        }

        let heap_builtins: std::collections::HashSet<String> = [
            "push",
            "pop",
            "to_string",
            "map_insert",
            "map_get",
            "map_get_or",
            "map_remove",
            "map_contains",
            "map_keys",
            "map_values",
            "map_len",
        ]
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
            // Short aliases (match interpreter builtins.rs)
            "eye",
            "from_data",
            "shape",
            "reshape",
            "matmul",
            "transpose",
            "flatten",
            "relu",
            "sigmoid",
            "softmax",
            "mse_loss",
            "quantize_int8",
            // Missing short aliases (found in V17 re-audit)
            "zeros",
            "ones",
            "randn",
            "tanh",
            "gelu",
            "leaky_relu",
            "cross_entropy",
            "bce_loss",
            "l1_loss",
            "xavier",
            "arange",
            "linspace",
            "concat",
            "split",
            "squeeze",
            "unsqueeze",
            "backward",
            "grad",
            "set_requires_grad",
            "Dense",
            "Conv2d",
            "BatchNorm",
            "Dropout",
            "SGD",
            "Adam",
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
            safe_blocked_builtins,
            message_structs: std::collections::HashSet::new(),
            protocol_clients: std::collections::HashSet::new(),
            message_ids: HashMap::new(),
            next_message_id: 1,
            current_device_cap: None,
            cap_port_io: [
                "port_outb",
                "port_inb",
                "port_outw",
                "port_inw",
                "port_outd",
                "port_ind",
                "port_read",
                "port_write",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect(),
            cap_irq: [
                "irq_register",
                "irq_unregister",
                "irq_enable",
                "irq_disable",
                "cli",
                "sti",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect(),
            cap_dma: [
                "dma_alloc",
                "dma_free",
                "dma_config",
                "dma_start",
                "dma_wait",
                "dma_status",
                "dma_barrier",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect(),
            cap_net: [
                "eth_init",
                "eth_send",
                "eth_recv",
                "net_socket",
                "net_bind",
                "net_listen",
                "net_accept",
                "net_connect",
                "net_send",
                "net_recv",
                "net_close",
                "http_listen",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect(),
            cap_blk: [
                "nvme_init",
                "nvme_read",
                "nvme_write",
                "sd_init",
                "sd_read_block",
                "sd_write_block",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect(),
            heap_builtins,
            tensor_builtins,
            tensor_tainted_fns: std::collections::HashSet::new(),
            os_tainted_fns: std::collections::HashSet::new(),
            heap_tainted_fns: std::collections::HashSet::new(),
            traits: HashMap::new(),
            trait_impls: std::collections::HashSet::new(),
            type_aliases: HashMap::new(),
            moves: crate::analyzer::borrow_lite::MoveTracker::new(),
            nll_info: None,
            enum_variants: HashMap::new(),
            imports: Vec::new(),
            linear_vars: HashMap::new(),
            effect_registry: crate::analyzer::effects::EffectRegistry::with_builtins(),
            fn_effects: HashMap::new(),
            in_handle_expr: false,
            current_handler_resume_type: None,
            current_fn_name: None,
            current_fn_return: None,
            symbolic_fn_shapes: HashMap::new(),
            current_fn_inferred_effects: std::collections::BTreeSet::new(),
            handled_effects_in_scope: std::collections::BTreeSet::new(),
            strict_ownership: false,
            lifetime_env: {
                let mut env = HashMap::new();
                env.insert("static".to_string(), 0);
                env
            },
            next_lifetime_id: 1,
            const_trait_registry: const_traits::ConstTraitRegistry::new(),
            dep_shape_env: HashMap::new(),
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

    /// Creates a new type checker with strict ownership enabled.
    ///
    /// In strict mode, String/Array/Struct/Tensor are Move types (not Copy).
    /// Assigning or passing them transfers ownership; use-after-move is an error.
    pub fn new_strict() -> Self {
        let mut tc = Self::new();
        tc.strict_ownership = true;
        tc
    }

    /// Returns whether strict ownership mode is enabled.
    pub fn is_strict_ownership(&self) -> bool {
        self.strict_ownership
    }

    /// Returns true if the given type is Copy in the current mode.
    ///
    /// In default mode, all types are Copy (interpreter semantics).
    /// In strict mode, only primitives and `&T` are Copy.
    fn is_copy(&self, ty: &Type) -> bool {
        if self.strict_ownership {
            crate::analyzer::borrow_lite::is_copy_type_strict(ty)
        } else {
            crate::analyzer::borrow_lite::is_copy_type(ty)
        }
    }

    /// Registers lifetime parameters from a function definition into the environment.
    ///
    /// Returns the previous environment so it can be restored after checking the function.
    fn push_lifetime_env(
        &mut self,
        lifetime_params: &[crate::parser::ast::LifetimeParam],
    ) -> (HashMap<String, u32>, u32) {
        let saved_env = self.lifetime_env.clone();
        let saved_id = self.next_lifetime_id;
        // Keep 'static from the base env
        for lp in lifetime_params {
            let id = self.next_lifetime_id;
            self.lifetime_env.insert(lp.name.clone(), id);
            self.next_lifetime_id += 1;
        }
        (saved_env, saved_id)
    }

    /// Restores a previously saved lifetime environment.
    fn pop_lifetime_env(&mut self, saved: (HashMap<String, u32>, u32)) {
        self.lifetime_env = saved.0;
        self.next_lifetime_id = saved.1;
    }

    /// Resolves a lifetime name to its ID. Returns None for undeclared lifetimes.
    /// `'_` (wildcard) returns None — it's always valid but has no specific ID.
    fn resolve_lifetime(&self, name: &str) -> Option<u32> {
        if name == "_" {
            return None;
        }
        self.lifetime_env.get(name).copied()
    }

    /// Registers built-in functions in the global scope.
    /// Analyzes a complete program.
    ///
    /// Returns `Ok(())` if no hard errors, or `Err(errors)` with all collected errors.
    /// Warnings (SE009, SE010) are included in errors but do not cause failure on their own.
    pub fn analyze(&mut self, program: &Program) -> Result<(), Vec<SemanticError>> {
        // Pre-pass: classify const generic parameters and register const trait impls.
        for item in &program.items {
            if let crate::parser::ast::Item::FnDef(fndef) = item {
                for gp in &fndef.generic_params {
                    if gp.is_comptime {
                        let kind = const_generics::classify_param(gp);
                        // Track const param in the dependent shape environment
                        if let const_generics::ParamKind::Const { .. } = kind {
                            self.dep_shape_env.insert(
                                gp.name.clone(),
                                dependent::nat::NatValue::Param(gp.name.clone()),
                            );
                        }
                    }
                }
            }
        }

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
mod tests;
