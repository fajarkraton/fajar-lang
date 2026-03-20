//! @infer context annotation — automatic inference dispatch across CPU, NPU, and GPU.
//!
//! The `@infer` annotation marks functions or blocks for automatic hardware dispatch.
//! It allows tensor operations and scalar compute, but disallows raw pointers and OS
//! primitives. The compiler selects the best available accelerator at compile time
//! (with optional runtime fallback).

use std::collections::HashMap;
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// S33.7: Error Codes IE001-IE004
// ═══════════════════════════════════════════════════════════════════════

/// Error codes for @infer context violations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InferContextError {
    /// IE001: Raw pointer operation in @infer context.
    RawPointerInInfer {
        /// Description of the offending operation.
        operation: String,
        /// Source location line.
        line: usize,
    },
    /// IE002: OS primitive (irq, syscall, port_io) in @infer context.
    OsPrimitiveInInfer {
        /// The OS primitive used.
        primitive: String,
        /// Source location line.
        line: usize,
    },
    /// IE003: @kernel function called from @infer context.
    KernelCallInInfer {
        /// Name of the @kernel function.
        function: String,
        /// Source location line.
        line: usize,
    },
    /// IE004: @infer function has non-dispatchable return type.
    InvalidInferReturnType {
        /// The actual return type.
        actual_type: String,
        /// Source location line.
        line: usize,
    },
}

impl fmt::Display for InferContextError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RawPointerInInfer { operation, line } => {
                write!(
                    f,
                    "IE001: raw pointer operation `{operation}` not allowed in @infer context (line {line})"
                )
            }
            Self::OsPrimitiveInInfer { primitive, line } => {
                write!(
                    f,
                    "IE002: OS primitive `{primitive}` not allowed in @infer context (line {line})"
                )
            }
            Self::KernelCallInInfer { function, line } => {
                write!(
                    f,
                    "IE003: cannot call @kernel function `{function}` from @infer context (line {line})"
                )
            }
            Self::InvalidInferReturnType { actual_type, line } => {
                write!(
                    f,
                    "IE004: @infer function must return tensor or scalar type, got `{actual_type}` (line {line})"
                )
            }
        }
    }
}

impl std::error::Error for InferContextError {}

impl InferContextError {
    /// Returns the error code string.
    pub fn code(&self) -> &'static str {
        match self {
            Self::RawPointerInInfer { .. } => "IE001",
            Self::OsPrimitiveInInfer { .. } => "IE002",
            Self::KernelCallInInfer { .. } => "IE003",
            Self::InvalidInferReturnType { .. } => "IE004",
        }
    }

    /// Returns a suggested fix for the error.
    pub fn suggestion(&self) -> String {
        match self {
            Self::RawPointerInInfer { .. } => {
                "move raw pointer operations to a @kernel or @unsafe function".to_string()
            }
            Self::OsPrimitiveInInfer { .. } => {
                "move OS primitive calls to a @kernel function".to_string()
            }
            Self::KernelCallInInfer { .. } => {
                "use @device or @safe bridge functions instead".to_string()
            }
            Self::InvalidInferReturnType { .. } => {
                "@infer functions should return Tensor, f32, f64, i32, i64, or bool".to_string()
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S33.5: Compile-Time Hints
// ═══════════════════════════════════════════════════════════════════════

/// Hardware preference hint for @infer annotation.
///
/// Parsed from `@infer(prefer=gpu)` or `@infer(prefer=npu)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InferPreference {
    /// No preference — auto-select best available.
    Auto,
    /// Prefer CPU execution.
    Cpu,
    /// Prefer GPU execution.
    Gpu,
    /// Prefer NPU execution.
    Npu,
}

impl fmt::Display for InferPreference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Auto => write!(f, "auto"),
            Self::Cpu => write!(f, "cpu"),
            Self::Gpu => write!(f, "gpu"),
            Self::Npu => write!(f, "npu"),
        }
    }
}

/// Parses `@infer` annotation parameter to extract preference hint.
///
/// Accepts formats: `@infer`, `@infer(prefer=gpu)`, `@infer(prefer=npu)`, `@infer(prefer=cpu)`.
pub fn parse_infer_hint(param: Option<&str>) -> InferPreference {
    match param {
        None => InferPreference::Auto,
        Some(p) => {
            let cleaned = p.trim();
            if let Some(value) = cleaned.strip_prefix("prefer=") {
                match value.trim().to_lowercase().as_str() {
                    "cpu" => InferPreference::Cpu,
                    "gpu" => InferPreference::Gpu,
                    "npu" => InferPreference::Npu,
                    _ => InferPreference::Auto,
                }
            } else {
                InferPreference::Auto
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S33.3: Analyzer Rules — Allowed/Disallowed Operations
// ═══════════════════════════════════════════════════════════════════════

/// Operations allowed in @infer context.
static ALLOWED_OPS: &[&str] = &[
    // Tensor operations
    "zeros",
    "ones",
    "randn",
    "matmul",
    "transpose",
    "reshape",
    "flatten",
    "concat",
    "split",
    "squeeze",
    // Activation functions
    "relu",
    "sigmoid",
    "tanh",
    "softmax",
    "gelu",
    "leaky_relu",
    // Loss functions
    "mse_loss",
    "cross_entropy",
    "bce_loss",
    "l1_loss",
    // Autograd
    "backward",
    "grad",
    "requires_grad",
    // Math
    "abs",
    "sqrt",
    "pow",
    "sin",
    "cos",
    "tan",
    "exp",
    "log",
    "clamp",
    "min",
    "max",
    // Scalar arithmetic (always allowed)
    "add",
    "sub",
    "mul",
    "div",
];

/// Operations disallowed in @infer context.
static DISALLOWED_OPS: &[&str] = &[
    // OS primitives
    "irq_register",
    "irq_unregister",
    "irq_enable",
    "irq_disable",
    "syscall_define",
    "syscall_dispatch",
    "port_read",
    "port_write",
    // Memory operations
    "mem_alloc",
    "mem_free",
    "mem_read",
    "mem_write",
    "page_map",
    "page_unmap",
    // Raw pointer operations
    "ptr_deref",
    "ptr_write",
    "ptr_cast",
];

/// Checks if an operation is allowed in @infer context.
pub fn is_allowed_in_infer(op: &str) -> bool {
    ALLOWED_OPS.contains(&op) || !DISALLOWED_OPS.contains(&op)
}

/// Checks if an operation is explicitly disallowed in @infer context.
pub fn is_disallowed_in_infer(op: &str) -> bool {
    DISALLOWED_OPS.contains(&op)
}

// ═══════════════════════════════════════════════════════════════════════
// S33.4: Type Checking — Dispatchable Return Types
// ═══════════════════════════════════════════════════════════════════════

/// Types that are valid return types for @infer functions.
static DISPATCHABLE_TYPES: &[&str] = &[
    "Tensor", "f32", "f64", "i8", "i16", "i32", "i64", "u8", "u16", "u32", "u64", "bool", "void",
];

/// Checks if a type name is dispatchable (valid for @infer return).
pub fn is_dispatchable_type(type_name: &str) -> bool {
    DISPATCHABLE_TYPES.contains(&type_name)
        || type_name.starts_with("Tensor")
        || type_name.starts_with("Array")
        || type_name.starts_with("Vec")
}

// ═══════════════════════════════════════════════════════════════════════
// S33.6: Context Compatibility
// ═══════════════════════════════════════════════════════════════════════

/// Annotation contexts that @infer can call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextCompat {
    /// Allowed to call.
    Allowed,
    /// Not allowed to call.
    Forbidden,
}

/// Checks whether @infer can call a function with the given annotation.
pub fn infer_can_call(target_annotation: &str) -> ContextCompat {
    match target_annotation {
        "device" | "npu" | "safe" | "infer" => ContextCompat::Allowed,
        "kernel" | "unsafe" => ContextCompat::Forbidden,
        _ => ContextCompat::Allowed, // unannotated functions are allowed
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S33.8: Diagnostic Messages
// ═══════════════════════════════════════════════════════════════════════

/// Formats a miette-compatible diagnostic message for an @infer error.
pub fn format_infer_diagnostic(error: &InferContextError) -> String {
    let code = error.code();
    let msg = error.to_string();
    let suggestion = error.suggestion();
    format!("[{code}] {msg}\n  help: {suggestion}")
}

// ═══════════════════════════════════════════════════════════════════════
// S33.9: Documentation — @infer context table
// ═══════════════════════════════════════════════════════════════════════

/// Returns the @infer context documentation table as markdown.
pub fn infer_context_table() -> String {
    let mut table = String::new();
    table.push_str("| Operation | @infer |\n");
    table.push_str("|-----------|--------|\n");
    table.push_str("| Tensor ops (zeros, matmul, relu) | OK |\n");
    table.push_str("| Scalar compute (i32, f64, bool) | OK |\n");
    table.push_str("| Call @device function | OK |\n");
    table.push_str("| Call @npu function | OK |\n");
    table.push_str("| Call @safe function | OK |\n");
    table.push_str("| Call @kernel function | ERROR IE003 |\n");
    table.push_str("| Raw pointer dereference | ERROR IE001 |\n");
    table.push_str("| OS primitives (irq, syscall) | ERROR IE002 |\n");
    table.push_str("| Non-dispatchable return type | ERROR IE004 |\n");
    table
}

// ═══════════════════════════════════════════════════════════════════════
// S33.2: InferContext — parsed @infer block
// ═══════════════════════════════════════════════════════════════════════

/// Parsed @infer context with compile-time hints and validation results.
#[derive(Debug, Clone)]
pub struct InferContext {
    /// Hardware preference hint.
    pub preference: InferPreference,
    /// Function name (if annotating a function).
    pub function_name: Option<String>,
    /// Collected validation errors.
    pub errors: Vec<InferContextError>,
    /// Operations used in the @infer block (for dispatch analysis).
    pub operations: Vec<String>,
    /// Estimated compute intensity (FLOPS).
    pub estimated_flops: u64,
}

impl InferContext {
    /// Creates a new @infer context with the given preference.
    pub fn new(preference: InferPreference) -> Self {
        Self {
            preference,
            function_name: None,
            errors: Vec::new(),
            operations: Vec::new(),
            estimated_flops: 0,
        }
    }

    /// Creates an @infer context for a named function.
    pub fn for_function(name: &str, preference: InferPreference) -> Self {
        Self {
            preference,
            function_name: Some(name.to_string()),
            errors: Vec::new(),
            operations: Vec::new(),
            estimated_flops: 0,
        }
    }

    /// Records an operation in the @infer block.
    pub fn record_op(&mut self, op: &str) {
        self.operations.push(op.to_string());
    }

    /// Validates an operation in @infer context, returning an error if disallowed.
    pub fn validate_op(&mut self, op: &str, line: usize) -> Result<(), InferContextError> {
        if is_disallowed_in_infer(op) {
            let err = if op.starts_with("ptr_") {
                InferContextError::RawPointerInInfer {
                    operation: op.to_string(),
                    line,
                }
            } else {
                InferContextError::OsPrimitiveInInfer {
                    primitive: op.to_string(),
                    line,
                }
            };
            self.errors.push(err.clone());
            Err(err)
        } else {
            self.record_op(op);
            Ok(())
        }
    }

    /// Validates a cross-context function call.
    pub fn validate_call(
        &mut self,
        callee: &str,
        callee_annotation: &str,
        line: usize,
    ) -> Result<(), InferContextError> {
        if infer_can_call(callee_annotation) == ContextCompat::Forbidden {
            let err = InferContextError::KernelCallInInfer {
                function: callee.to_string(),
                line,
            };
            self.errors.push(err.clone());
            Err(err)
        } else {
            Ok(())
        }
    }

    /// Validates the return type of an @infer function.
    pub fn validate_return_type(
        &mut self,
        type_name: &str,
        line: usize,
    ) -> Result<(), InferContextError> {
        if !is_dispatchable_type(type_name) {
            let err = InferContextError::InvalidInferReturnType {
                actual_type: type_name.to_string(),
                line,
            };
            self.errors.push(err.clone());
            Err(err)
        } else {
            Ok(())
        }
    }

    /// Returns true if the context has no validation errors.
    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }

    /// Estimates FLOPS from recorded operations.
    pub fn estimate_flops(&mut self, input_elements: u64) {
        let mut flops = 0u64;
        for op in &self.operations {
            flops += match op.as_str() {
                "matmul" => input_elements * input_elements, // O(n^2) for matmul
                "relu" | "sigmoid" | "tanh" | "softmax" | "gelu" => input_elements,
                "add" | "sub" | "mul" | "div" => input_elements,
                "backward" => input_elements * 3, // ~3x forward
                _ => input_elements,
            };
        }
        self.estimated_flops = flops;
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S33.10: Annotation Registry
// ═══════════════════════════════════════════════════════════════════════

/// Registry of @infer-annotated functions for dispatch planning.
#[derive(Debug, Clone, Default)]
pub struct InferRegistry {
    /// Map of function name -> InferContext.
    entries: HashMap<String, InferContext>,
}

impl InferRegistry {
    /// Creates a new empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers an @infer function.
    pub fn register(&mut self, name: String, ctx: InferContext) {
        self.entries.insert(name, ctx);
    }

    /// Looks up an @infer function by name.
    pub fn get(&self, name: &str) -> Option<&InferContext> {
        self.entries.get(name)
    }

    /// Returns all registered @infer function names.
    pub fn function_names(&self) -> Vec<&str> {
        self.entries.keys().map(|s| s.as_str()).collect()
    }

    /// Returns the total number of registered @infer functions.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns true if no @infer functions are registered.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // S33.1: Lexer token
    #[test]
    fn s33_1_infer_token_recognized() {
        use crate::lexer::token::{ANNOTATIONS, TokenKind};
        assert_eq!(ANNOTATIONS.get("infer"), Some(&TokenKind::AtInfer));
    }

    // S33.2: Parser support
    #[test]
    fn s33_2_infer_context_creation() {
        let ctx = InferContext::new(InferPreference::Auto);
        assert_eq!(ctx.preference, InferPreference::Auto);
        assert!(ctx.is_valid());
    }

    #[test]
    fn s33_2_infer_context_for_function() {
        let ctx = InferContext::for_function("classify", InferPreference::Gpu);
        assert_eq!(ctx.function_name, Some("classify".to_string()));
        assert_eq!(ctx.preference, InferPreference::Gpu);
    }

    // S33.3: Analyzer rules
    #[test]
    fn s33_3_tensor_ops_allowed() {
        assert!(is_allowed_in_infer("matmul"));
        assert!(is_allowed_in_infer("relu"));
        assert!(is_allowed_in_infer("softmax"));
        assert!(is_allowed_in_infer("zeros"));
    }

    #[test]
    fn s33_3_raw_pointer_disallowed() {
        assert!(is_disallowed_in_infer("ptr_deref"));
        assert!(is_disallowed_in_infer("ptr_write"));
        assert!(is_disallowed_in_infer("mem_alloc"));
    }

    #[test]
    fn s33_3_os_primitives_disallowed() {
        assert!(is_disallowed_in_infer("irq_register"));
        assert!(is_disallowed_in_infer("syscall_dispatch"));
        assert!(is_disallowed_in_infer("port_read"));
    }

    // S33.4: Type checking
    #[test]
    fn s33_4_dispatchable_types() {
        assert!(is_dispatchable_type("Tensor"));
        assert!(is_dispatchable_type("f32"));
        assert!(is_dispatchable_type("f64"));
        assert!(is_dispatchable_type("i32"));
        assert!(is_dispatchable_type("bool"));
        assert!(is_dispatchable_type("void"));
    }

    #[test]
    fn s33_4_non_dispatchable_types() {
        assert!(!is_dispatchable_type("String"));
        assert!(!is_dispatchable_type("HashMap"));
        assert!(!is_dispatchable_type("File"));
    }

    #[test]
    fn s33_4_validate_return_type() {
        let mut ctx = InferContext::new(InferPreference::Auto);
        assert!(ctx.validate_return_type("Tensor", 1).is_ok());
        assert!(ctx.validate_return_type("f64", 2).is_ok());
        assert!(ctx.validate_return_type("String", 3).is_err());
        assert!(!ctx.is_valid());
    }

    // S33.5: Compile-time hints
    #[test]
    fn s33_5_parse_prefer_gpu() {
        assert_eq!(parse_infer_hint(Some("prefer=gpu")), InferPreference::Gpu);
    }

    #[test]
    fn s33_5_parse_prefer_npu() {
        assert_eq!(parse_infer_hint(Some("prefer=npu")), InferPreference::Npu);
    }

    #[test]
    fn s33_5_parse_no_hint() {
        assert_eq!(parse_infer_hint(None), InferPreference::Auto);
    }

    #[test]
    fn s33_5_parse_invalid_hint() {
        assert_eq!(parse_infer_hint(Some("prefer=tpu")), InferPreference::Auto);
    }

    // S33.6: Context compatibility
    #[test]
    fn s33_6_infer_can_call_device() {
        assert_eq!(infer_can_call("device"), ContextCompat::Allowed);
    }

    #[test]
    fn s33_6_infer_can_call_npu() {
        assert_eq!(infer_can_call("npu"), ContextCompat::Allowed);
    }

    #[test]
    fn s33_6_infer_cannot_call_kernel() {
        assert_eq!(infer_can_call("kernel"), ContextCompat::Forbidden);
    }

    #[test]
    fn s33_6_validate_cross_context_call() {
        let mut ctx = InferContext::new(InferPreference::Auto);
        assert!(ctx.validate_call("read_sensor", "kernel", 10).is_err());
        assert!(ctx.validate_call("classify", "device", 11).is_ok());
    }

    // S33.7: Error codes
    #[test]
    fn s33_7_error_codes() {
        let e1 = InferContextError::RawPointerInInfer {
            operation: "ptr_deref".into(),
            line: 5,
        };
        assert_eq!(e1.code(), "IE001");
        assert!(e1.to_string().contains("IE001"));

        let e2 = InferContextError::OsPrimitiveInInfer {
            primitive: "irq_register".into(),
            line: 10,
        };
        assert_eq!(e2.code(), "IE002");

        let e3 = InferContextError::KernelCallInInfer {
            function: "read_sensor".into(),
            line: 15,
        };
        assert_eq!(e3.code(), "IE003");

        let e4 = InferContextError::InvalidInferReturnType {
            actual_type: "String".into(),
            line: 20,
        };
        assert_eq!(e4.code(), "IE004");
    }

    // S33.8: Diagnostic messages
    #[test]
    fn s33_8_diagnostic_formatting() {
        let err = InferContextError::RawPointerInInfer {
            operation: "ptr_deref".into(),
            line: 5,
        };
        let diag = format_infer_diagnostic(&err);
        assert!(diag.contains("[IE001]"));
        assert!(diag.contains("help:"));
        assert!(diag.contains("@kernel or @unsafe"));
    }

    // S33.9: Documentation
    #[test]
    fn s33_9_context_table() {
        let table = infer_context_table();
        assert!(table.contains("@infer"));
        assert!(table.contains("IE001"));
        assert!(table.contains("IE002"));
        assert!(table.contains("IE003"));
        assert!(table.contains("IE004"));
    }

    // S33.10: Validation integration
    #[test]
    fn s33_10_validate_op_allowed() {
        let mut ctx = InferContext::new(InferPreference::Auto);
        assert!(ctx.validate_op("matmul", 1).is_ok());
        assert!(ctx.validate_op("relu", 2).is_ok());
        assert_eq!(ctx.operations.len(), 2);
    }

    #[test]
    fn s33_10_validate_op_disallowed() {
        let mut ctx = InferContext::new(InferPreference::Auto);
        assert!(ctx.validate_op("ptr_deref", 1).is_err());
        assert!(ctx.validate_op("irq_register", 2).is_err());
        assert_eq!(ctx.errors.len(), 2);
    }

    #[test]
    fn s33_10_registry() {
        let mut reg = InferRegistry::new();
        assert!(reg.is_empty());

        let ctx = InferContext::for_function("classify", InferPreference::Npu);
        reg.register("classify".to_string(), ctx);

        assert_eq!(reg.len(), 1);
        assert!(reg.get("classify").is_some());
        assert!(reg.function_names().contains(&"classify"));
    }

    #[test]
    fn s33_10_flops_estimation() {
        let mut ctx = InferContext::new(InferPreference::Auto);
        ctx.record_op("matmul");
        ctx.record_op("relu");
        ctx.estimate_flops(1000);
        assert!(ctx.estimated_flops > 0);
    }

    #[test]
    fn s33_10_infer_preference_display() {
        assert_eq!(format!("{}", InferPreference::Auto), "auto");
        assert_eq!(format!("{}", InferPreference::Gpu), "gpu");
        assert_eq!(format!("{}", InferPreference::Npu), "npu");
        assert_eq!(format!("{}", InferPreference::Cpu), "cpu");
    }
}
