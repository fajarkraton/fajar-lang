//! @device Safety Proofs — Sprint V4: 10 tasks.
//!
//! Proves safety properties for `@device` annotated code: no-raw-pointer usage,
//! tensor shape correctness, tensor dtype correctness, memory bound safety,
//! gradient tracking, numerical stability, shape inference, broadcast compatibility,
//! and memory layout consistency. All simulated (no real Z3).

use std::collections::HashMap;
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// V4.1: Device Violation Types
// ═══════════════════════════════════════════════════════════════════════

/// A safety violation found during @device proof checking.
#[derive(Debug, Clone, PartialEq)]
pub struct DeviceViolation {
    /// Violation kind.
    pub kind: DeviceViolationKind,
    /// Description of the violation.
    pub description: String,
    /// Source file.
    pub file: String,
    /// Source line.
    pub line: u32,
    /// Suggested fix.
    pub suggestion: String,
}

impl fmt::Display for DeviceViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{} [{}] {} (fix: {})",
            self.file, self.line, self.kind, self.description, self.suggestion
        )
    }
}

/// Kinds of @device safety violations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeviceViolationKind {
    /// Raw pointer usage in @device context (DE001).
    RawPointer,
    /// Tensor shape mismatch.
    TensorShape,
    /// Tensor dtype mismatch.
    TensorDtype,
    /// Tensor memory out-of-bounds.
    MemoryBound,
    /// Gradient tracking violation (missing or detached).
    GradientTracking,
    /// Numerical instability risk (overflow, NaN, inf).
    NumericalStability,
    /// Shape inference failure.
    ShapeInference,
    /// Broadcast incompatibility.
    BroadcastCompat,
    /// Memory layout inconsistency (stride, contiguity).
    MemoryLayout,
}

impl fmt::Display for DeviceViolationKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::RawPointer => "DE001:raw-ptr",
            Self::TensorShape => "DE-SHAPE",
            Self::TensorDtype => "DE-DTYPE",
            Self::MemoryBound => "DE-MEM",
            Self::GradientTracking => "DE-GRAD",
            Self::NumericalStability => "DE-NUMERIC",
            Self::ShapeInference => "DE-INFER",
            Self::BroadcastCompat => "DE-BROADCAST",
            Self::MemoryLayout => "DE-LAYOUT",
        };
        write!(f, "{s}")
    }
}

// ═══════════════════════════════════════════════════════════════════════
// V4.2: Device Function Representation
// ═══════════════════════════════════════════════════════════════════════

/// Represents a @device function for safety checking.
#[derive(Debug, Clone, Default)]
pub struct DeviceFunction {
    /// Function name.
    pub name: String,
    /// Source file.
    pub file: String,
    /// Source line.
    pub line: u32,
    /// Operations performed in the function body.
    pub operations: Vec<DeviceOp>,
    /// Tensor variables tracked in the function.
    pub tensors: HashMap<String, TensorInfo>,
}

/// Tensor metadata for verification.
#[derive(Debug, Clone, PartialEq)]
pub struct TensorInfo {
    /// Shape dimensions (empty = unknown).
    pub shape: Vec<TensorDim>,
    /// Data type.
    pub dtype: TensorDtype,
    /// Whether gradient tracking is enabled.
    pub requires_grad: bool,
    /// Whether the tensor is contiguous in memory.
    pub is_contiguous: bool,
    /// Memory layout (row-major or column-major).
    pub layout: MemoryLayout,
}

/// A tensor dimension (concrete or symbolic).
#[derive(Debug, Clone, PartialEq)]
pub enum TensorDim {
    /// Known concrete size.
    Concrete(usize),
    /// Symbolic (named).
    Symbolic(String),
    /// Dynamic (unknown at compile time).
    Dynamic,
}

impl fmt::Display for TensorDim {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Concrete(n) => write!(f, "{n}"),
            Self::Symbolic(name) => write!(f, "{name}"),
            Self::Dynamic => write!(f, "?"),
        }
    }
}

/// Tensor data types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TensorDtype {
    /// 32-bit float.
    F32,
    /// 64-bit float.
    F64,
    /// 8-bit integer (quantized).
    I8,
    /// 16-bit integer.
    I16,
    /// 32-bit integer.
    I32,
    /// Boolean.
    Bool,
}

impl fmt::Display for TensorDtype {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::F32 => "f32",
            Self::F64 => "f64",
            Self::I8 => "i8",
            Self::I16 => "i16",
            Self::I32 => "i32",
            Self::Bool => "bool",
        };
        write!(f, "{s}")
    }
}

/// Memory layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryLayout {
    /// Row-major (C-style).
    RowMajor,
    /// Column-major (Fortran-style).
    ColMajor,
}

impl fmt::Display for MemoryLayout {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RowMajor => write!(f, "row-major"),
            Self::ColMajor => write!(f, "col-major"),
        }
    }
}

/// An operation performed inside a @device function.
#[derive(Debug, Clone, PartialEq)]
pub enum DeviceOp {
    /// Raw pointer usage -- forbidden.
    RawPointer { description: String },
    /// Matrix multiplication.
    Matmul {
        lhs: String,
        rhs: String,
        result: String,
    },
    /// Element-wise operation.
    Elementwise {
        op: String,
        inputs: Vec<String>,
        output: String,
    },
    /// Reshape operation.
    Reshape {
        input: String,
        new_shape: Vec<TensorDim>,
        output: String,
    },
    /// Gradient backward pass.
    Backward { tensor: String },
    /// Division operation (numerical stability risk).
    Division {
        numerator: String,
        denominator: String,
    },
    /// Exponentiation (overflow risk).
    Exp { input: String },
    /// Log (domain error risk).
    Log { input: String },
    /// Broadcast operation.
    Broadcast {
        lhs: String,
        rhs: String,
        output: String,
    },
    /// Transpose / permute.
    Transpose { input: String, perm: Vec<usize> },
    /// Memory access (indexing).
    Access {
        tensor: String,
        indices: Vec<String>,
    },
}

// ═══════════════════════════════════════════════════════════════════════
// V4.3: Individual Proof Checkers
// ═══════════════════════════════════════════════════════════════════════

/// Checks for raw pointer usage in @device code.
pub fn check_no_raw_pointer(func: &DeviceFunction) -> Vec<DeviceViolation> {
    let mut violations = Vec::new();
    for op in &func.operations {
        if let DeviceOp::RawPointer { description } = op {
            violations.push(DeviceViolation {
                kind: DeviceViolationKind::RawPointer,
                description: format!(
                    "raw pointer usage in @device function '{}': {description}",
                    func.name
                ),
                file: func.file.clone(),
                line: func.line,
                suggestion: "use safe tensor operations instead of raw pointers".to_string(),
            });
        }
    }
    violations
}

/// Checks tensor shape compatibility for operations.
pub fn check_tensor_shapes(func: &DeviceFunction) -> Vec<DeviceViolation> {
    let mut violations = Vec::new();

    for op in &func.operations {
        match op {
            DeviceOp::Matmul { lhs, rhs, .. } => {
                if let (Some(a), Some(b)) = (func.tensors.get(lhs), func.tensors.get(rhs)) {
                    if a.shape.len() >= 2 && b.shape.len() >= 2 {
                        let a_inner = &a.shape[a.shape.len() - 1];
                        let b_outer = &b.shape[b.shape.len() - 2];
                        if let (TensorDim::Concrete(ak), TensorDim::Concrete(bk)) =
                            (a_inner, b_outer)
                        {
                            if ak != bk {
                                violations.push(DeviceViolation {
                                    kind: DeviceViolationKind::TensorShape,
                                    description: format!(
                                        "matmul shape mismatch: {lhs} inner dim {ak} != {rhs} outer dim {bk}"
                                    ),
                                    file: func.file.clone(),
                                    line: func.line,
                                    suggestion: "ensure inner dimensions match for matmul"
                                        .to_string(),
                                });
                            }
                        }
                    }
                }
            }
            DeviceOp::Reshape {
                input, new_shape, ..
            } => {
                if let Some(info) = func.tensors.get(input) {
                    let old_total = shape_total(&info.shape);
                    let new_total = shape_total(new_shape);
                    if let (Some(o), Some(n)) = (old_total, new_total) {
                        if o != n {
                            violations.push(DeviceViolation {
                                kind: DeviceViolationKind::TensorShape,
                                description: format!(
                                    "reshape element count mismatch: {input} has {o} elements, new shape has {n}"
                                ),
                                file: func.file.clone(),
                                line: func.line,
                                suggestion: "ensure total element count is preserved in reshape"
                                    .to_string(),
                            });
                        }
                    }
                }
            }
            _ => {}
        }
    }

    violations
}

/// Checks tensor dtype compatibility for operations.
pub fn check_tensor_dtypes(func: &DeviceFunction) -> Vec<DeviceViolation> {
    let mut violations = Vec::new();

    for op in &func.operations {
        if let DeviceOp::Elementwise { inputs, .. } = op {
            let dtypes: Vec<Option<&TensorDtype>> = inputs
                .iter()
                .map(|name| func.tensors.get(name).map(|t| &t.dtype))
                .collect();
            let first = dtypes.first().copied().flatten();
            for (i, dtype) in dtypes.iter().enumerate().skip(1) {
                if let (Some(f), Some(d)) = (first, dtype) {
                    if f != *d {
                        violations.push(DeviceViolation {
                            kind: DeviceViolationKind::TensorDtype,
                            description: format!(
                                "dtype mismatch in elementwise op: input 0 is {f}, input {i} is {d}"
                            ),
                            file: func.file.clone(),
                            line: func.line,
                            suggestion: "cast tensors to the same dtype before operation"
                                .to_string(),
                        });
                    }
                }
            }
        }
    }

    violations
}

/// Checks memory bounds for tensor access operations.
pub fn check_memory_bounds(func: &DeviceFunction) -> Vec<DeviceViolation> {
    let mut violations = Vec::new();

    for op in &func.operations {
        if let DeviceOp::Access { tensor, indices } = op {
            if let Some(info) = func.tensors.get(tensor) {
                if indices.len() != info.shape.len() {
                    violations.push(DeviceViolation {
                        kind: DeviceViolationKind::MemoryBound,
                        description: format!(
                            "index rank mismatch: {tensor} has {} dims but {} indices provided",
                            info.shape.len(),
                            indices.len()
                        ),
                        file: func.file.clone(),
                        line: func.line,
                        suggestion: "ensure number of indices matches tensor rank".to_string(),
                    });
                }
            }
        }
    }

    violations
}

/// Checks gradient tracking: backward() requires requires_grad=true.
pub fn check_gradient_tracking(func: &DeviceFunction) -> Vec<DeviceViolation> {
    let mut violations = Vec::new();

    for op in &func.operations {
        if let DeviceOp::Backward { tensor } = op {
            if let Some(info) = func.tensors.get(tensor) {
                if !info.requires_grad {
                    violations.push(DeviceViolation {
                        kind: DeviceViolationKind::GradientTracking,
                        description: format!(
                            "backward() called on tensor '{tensor}' without requires_grad"
                        ),
                        file: func.file.clone(),
                        line: func.line,
                        suggestion: "set requires_grad=true before calling backward()".to_string(),
                    });
                }
            }
        }
    }

    violations
}

/// Checks numerical stability: division by zero, exp overflow, log domain.
pub fn check_numerical_stability(func: &DeviceFunction) -> Vec<DeviceViolation> {
    let mut violations = Vec::new();

    for op in &func.operations {
        match op {
            DeviceOp::Division { denominator, .. } => {
                // Flag if denominator could be zero (simulated: always warn unless constant)
                violations.push(DeviceViolation {
                    kind: DeviceViolationKind::NumericalStability,
                    description: format!(
                        "division by '{denominator}' in '{}' may produce NaN/inf",
                        func.name
                    ),
                    file: func.file.clone(),
                    line: func.line,
                    suggestion: "add epsilon guard: x / (y + 1e-8)".to_string(),
                });
            }
            DeviceOp::Exp { input } => {
                violations.push(DeviceViolation {
                    kind: DeviceViolationKind::NumericalStability,
                    description: format!(
                        "exp({input}) in '{}' may overflow for large values",
                        func.name
                    ),
                    file: func.file.clone(),
                    line: func.line,
                    suggestion: "clamp input before exp: exp(clamp(x, -88, 88))".to_string(),
                });
            }
            DeviceOp::Log { input } => {
                violations.push(DeviceViolation {
                    kind: DeviceViolationKind::NumericalStability,
                    description: format!("log({input}) in '{}' is undefined for x <= 0", func.name),
                    file: func.file.clone(),
                    line: func.line,
                    suggestion: "add guard: log(max(x, 1e-8))".to_string(),
                });
            }
            _ => {}
        }
    }

    violations
}

/// Checks shape inference: all output shapes must be determinable.
pub fn check_shape_inference(func: &DeviceFunction) -> Vec<DeviceViolation> {
    let mut violations = Vec::new();

    for op in &func.operations {
        if let DeviceOp::Matmul { lhs, rhs, result } = op {
            let lhs_info = func.tensors.get(lhs);
            let rhs_info = func.tensors.get(rhs);
            let result_info = func.tensors.get(result);

            if lhs_info.is_none() || rhs_info.is_none() {
                violations.push(DeviceViolation {
                    kind: DeviceViolationKind::ShapeInference,
                    description: format!(
                        "cannot infer shape for matmul({lhs}, {rhs}): input shapes unknown"
                    ),
                    file: func.file.clone(),
                    line: func.line,
                    suggestion: "annotate tensor shapes or add shape assertions".to_string(),
                });
            }

            if result_info
                .map(|i| i.shape.iter().any(|d| matches!(d, TensorDim::Dynamic)))
                .unwrap_or(false)
            {
                violations.push(DeviceViolation {
                    kind: DeviceViolationKind::ShapeInference,
                    description: format!(
                        "matmul result '{result}' has dynamic dimensions that could not be resolved"
                    ),
                    file: func.file.clone(),
                    line: func.line,
                    suggestion: "provide concrete dimensions or symbolic constraints".to_string(),
                });
            }
        }
    }

    violations
}

/// Checks broadcast compatibility between tensor operands.
pub fn check_broadcast_compatibility(func: &DeviceFunction) -> Vec<DeviceViolation> {
    let mut violations = Vec::new();

    for op in &func.operations {
        if let DeviceOp::Broadcast { lhs, rhs, .. } = op {
            if let (Some(a), Some(b)) = (func.tensors.get(lhs), func.tensors.get(rhs)) {
                let max_rank = a.shape.len().max(b.shape.len());
                for i in 0..max_rank {
                    let a_dim = if i < a.shape.len() {
                        &a.shape[a.shape.len() - 1 - i]
                    } else {
                        &TensorDim::Concrete(1)
                    };
                    let b_dim = if i < b.shape.len() {
                        &b.shape[b.shape.len() - 1 - i]
                    } else {
                        &TensorDim::Concrete(1)
                    };
                    if let (TensorDim::Concrete(av), TensorDim::Concrete(bv)) = (a_dim, b_dim) {
                        if av != bv && *av != 1 && *bv != 1 {
                            violations.push(DeviceViolation {
                                kind: DeviceViolationKind::BroadcastCompat,
                                description: format!(
                                    "broadcast incompatible: {lhs} dim {av} vs {rhs} dim {bv} at axis {i} (from right)"
                                ),
                                file: func.file.clone(),
                                line: func.line,
                                suggestion: "reshape tensors to be broadcast-compatible"
                                    .to_string(),
                            });
                        }
                    }
                }
            }
        }
    }

    violations
}

/// Checks memory layout consistency (e.g., matmul requires contiguous tensors).
pub fn check_memory_layout(func: &DeviceFunction) -> Vec<DeviceViolation> {
    let mut violations = Vec::new();

    for op in &func.operations {
        if let DeviceOp::Matmul { lhs, rhs, .. } = op {
            for name in [lhs, rhs] {
                if let Some(info) = func.tensors.get(name) {
                    if !info.is_contiguous {
                        violations.push(DeviceViolation {
                            kind: DeviceViolationKind::MemoryLayout,
                            description: format!(
                                "tensor '{name}' is non-contiguous in matmul (performance/correctness risk)"
                            ),
                            file: func.file.clone(),
                            line: func.line,
                            suggestion: "call .contiguous() before matmul".to_string(),
                        });
                    }
                }
            }
        }
    }

    violations
}

// ═══════════════════════════════════════════════════════════════════════
// V4.10: Device Safety Checker (Orchestrator)
// ═══════════════════════════════════════════════════════════════════════

/// Configuration for device safety checking.
#[derive(Debug, Clone)]
pub struct DeviceCheckConfig {
    /// Whether to check numerical stability.
    pub check_numerical: bool,
    /// Whether to check gradient tracking.
    pub check_gradients: bool,
    /// Whether to check memory layout.
    pub check_layout: bool,
    /// Whether to check shape inference.
    pub check_inference: bool,
}

impl Default for DeviceCheckConfig {
    fn default() -> Self {
        Self {
            check_numerical: true,
            check_gradients: true,
            check_layout: true,
            check_inference: true,
        }
    }
}

/// Device safety checker: runs all proof checks on @device functions.
#[derive(Debug)]
pub struct DeviceSafetyChecker {
    /// Configuration.
    pub config: DeviceCheckConfig,
    /// All violations found.
    pub violations: Vec<DeviceViolation>,
    /// Per-function violation counts.
    pub per_function: HashMap<String, Vec<DeviceViolation>>,
    /// Statistics.
    pub stats: DeviceCheckStats,
}

/// Statistics for device safety checking.
#[derive(Debug, Clone, Default)]
pub struct DeviceCheckStats {
    /// Total functions checked.
    pub functions_checked: u64,
    /// Total violations found.
    pub total_violations: u64,
    /// Violations by kind.
    pub by_kind: HashMap<DeviceViolationKind, u64>,
    /// Functions that passed all checks.
    pub functions_clean: u64,
}

impl DeviceSafetyChecker {
    /// Creates a new checker with the given configuration.
    pub fn new(config: DeviceCheckConfig) -> Self {
        Self {
            config,
            violations: Vec::new(),
            per_function: HashMap::new(),
            stats: DeviceCheckStats::default(),
        }
    }

    /// Creates a new checker with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(DeviceCheckConfig::default())
    }

    /// Runs all safety checks on a single @device function.
    pub fn check_function(&mut self, func: &DeviceFunction) -> Vec<DeviceViolation> {
        let mut func_violations = Vec::new();

        // Mandatory checks
        func_violations.extend(check_no_raw_pointer(func));
        func_violations.extend(check_tensor_shapes(func));
        func_violations.extend(check_tensor_dtypes(func));
        func_violations.extend(check_memory_bounds(func));
        func_violations.extend(check_broadcast_compatibility(func));

        // Optional checks
        if self.config.check_numerical {
            func_violations.extend(check_numerical_stability(func));
        }
        if self.config.check_gradients {
            func_violations.extend(check_gradient_tracking(func));
        }
        if self.config.check_layout {
            func_violations.extend(check_memory_layout(func));
        }
        if self.config.check_inference {
            func_violations.extend(check_shape_inference(func));
        }

        // Update stats
        self.stats.functions_checked += 1;
        if func_violations.is_empty() {
            self.stats.functions_clean += 1;
        }
        for v in &func_violations {
            *self.stats.by_kind.entry(v.kind).or_insert(0) += 1;
        }
        self.stats.total_violations += func_violations.len() as u64;

        self.per_function
            .insert(func.name.clone(), func_violations.clone());
        self.violations.extend(func_violations.clone());

        func_violations
    }

    /// Runs all safety checks on multiple @device functions.
    pub fn check_all(&mut self, functions: &[DeviceFunction]) -> Vec<DeviceViolation> {
        for func in functions {
            self.check_function(func);
        }
        self.violations.clone()
    }

    /// Returns true if no violations were found.
    pub fn is_clean(&self) -> bool {
        self.violations.is_empty()
    }

    /// Returns the number of violations of a specific kind.
    pub fn count_kind(&self, kind: DeviceViolationKind) -> u64 {
        self.stats.by_kind.get(&kind).copied().unwrap_or(0)
    }

    /// Generates a summary report.
    pub fn report(&self) -> String {
        let mut report = String::new();
        report.push_str(&format!(
            "Device Safety Report: {} functions checked\n",
            self.stats.functions_checked
        ));
        report.push_str(&format!(
            "  Clean: {}, With violations: {}\n",
            self.stats.functions_clean,
            self.stats.functions_checked - self.stats.functions_clean
        ));
        report.push_str(&format!(
            "  Total violations: {}\n",
            self.stats.total_violations
        ));
        if !self.stats.by_kind.is_empty() {
            report.push_str("  By kind:\n");
            let mut kinds: Vec<_> = self.stats.by_kind.iter().collect();
            kinds.sort_by_key(|(k, _)| format!("{k}"));
            for (kind, count) in kinds {
                report.push_str(&format!("    {kind}: {count}\n"));
            }
        }
        report
    }
}

impl Default for DeviceSafetyChecker {
    fn default() -> Self {
        Self::with_defaults()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════════════════════════════

/// Computes total element count from a shape, if all dims are concrete.
fn shape_total(dims: &[TensorDim]) -> Option<usize> {
    let mut total = 1usize;
    for d in dims {
        match d {
            TensorDim::Concrete(n) => total *= n,
            _ => return None,
        }
    }
    Some(total)
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn make_tensor(shape: &[usize], dtype: TensorDtype) -> TensorInfo {
        TensorInfo {
            shape: shape.iter().map(|&d| TensorDim::Concrete(d)).collect(),
            dtype,
            requires_grad: true,
            is_contiguous: true,
            layout: MemoryLayout::RowMajor,
        }
    }

    fn make_clean_func(name: &str) -> DeviceFunction {
        let mut tensors = HashMap::new();
        tensors.insert("a".to_string(), make_tensor(&[4, 3], TensorDtype::F32));
        tensors.insert("b".to_string(), make_tensor(&[3, 5], TensorDtype::F32));
        tensors.insert("c".to_string(), make_tensor(&[4, 5], TensorDtype::F32));
        DeviceFunction {
            name: name.to_string(),
            file: "model.fj".to_string(),
            line: 1,
            operations: vec![DeviceOp::Matmul {
                lhs: "a".to_string(),
                rhs: "b".to_string(),
                result: "c".to_string(),
            }],
            tensors,
        }
    }

    // --- V4.1: No Raw Pointer ---

    #[test]
    fn v4_1_no_raw_pointer_clean() {
        let func = make_clean_func("forward");
        let violations = check_no_raw_pointer(&func);
        assert!(violations.is_empty());
    }

    #[test]
    fn v4_1_no_raw_pointer_violation() {
        let mut func = make_clean_func("bad_fn");
        func.operations.push(DeviceOp::RawPointer {
            description: "*mut f32 dereference".to_string(),
        });
        let violations = check_no_raw_pointer(&func);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].kind, DeviceViolationKind::RawPointer);
    }

    // --- V4.2: Tensor Shape ---

    #[test]
    fn v4_2_shape_valid_matmul() {
        let func = make_clean_func("matmul_ok");
        let violations = check_tensor_shapes(&func);
        assert!(violations.is_empty());
    }

    #[test]
    fn v4_2_shape_invalid_matmul() {
        let mut func = make_clean_func("matmul_bad");
        func.tensors.insert(
            "b".to_string(),
            make_tensor(&[7, 5], TensorDtype::F32), // inner dim 7 != 3
        );
        let violations = check_tensor_shapes(&func);
        assert!(!violations.is_empty());
        assert!(
            violations
                .iter()
                .any(|v| v.kind == DeviceViolationKind::TensorShape)
        );
    }

    #[test]
    fn v4_2_reshape_element_mismatch() {
        let mut func = make_clean_func("reshape_bad");
        func.operations.push(DeviceOp::Reshape {
            input: "a".to_string(), // 4x3 = 12 elements
            new_shape: vec![TensorDim::Concrete(5), TensorDim::Concrete(5)], // 25 elements
            output: "d".to_string(),
        });
        let violations = check_tensor_shapes(&func);
        assert!(!violations.is_empty());
    }

    // --- V4.3: Tensor Dtype ---

    #[test]
    fn v4_3_dtype_match() {
        let mut func = make_clean_func("ewise");
        func.operations = vec![DeviceOp::Elementwise {
            op: "add".to_string(),
            inputs: vec!["a".to_string(), "b".to_string()],
            output: "c".to_string(),
        }];
        let violations = check_tensor_dtypes(&func);
        assert!(violations.is_empty()); // both f32
    }

    #[test]
    fn v4_3_dtype_mismatch() {
        let mut func = make_clean_func("ewise_bad");
        func.tensors.insert(
            "b".to_string(),
            make_tensor(&[3, 5], TensorDtype::I32), // i32 vs f32
        );
        func.operations = vec![DeviceOp::Elementwise {
            op: "add".to_string(),
            inputs: vec!["a".to_string(), "b".to_string()],
            output: "c".to_string(),
        }];
        let violations = check_tensor_dtypes(&func);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].kind, DeviceViolationKind::TensorDtype);
    }

    // --- V4.4: Memory Bounds ---

    #[test]
    fn v4_4_access_correct_rank() {
        let mut func = make_clean_func("access_ok");
        func.operations = vec![DeviceOp::Access {
            tensor: "a".to_string(),
            indices: vec!["0".to_string(), "1".to_string()], // 2D tensor, 2 indices
        }];
        let violations = check_memory_bounds(&func);
        assert!(violations.is_empty());
    }

    #[test]
    fn v4_4_access_wrong_rank() {
        let mut func = make_clean_func("access_bad");
        func.operations = vec![DeviceOp::Access {
            tensor: "a".to_string(),
            indices: vec!["0".to_string()], // 2D tensor, only 1 index
        }];
        let violations = check_memory_bounds(&func);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].kind, DeviceViolationKind::MemoryBound);
    }

    // --- V4.5: Gradient Tracking ---

    #[test]
    fn v4_5_gradient_ok() {
        let mut func = make_clean_func("backward_ok");
        func.operations = vec![DeviceOp::Backward {
            tensor: "a".to_string(),
        }];
        let violations = check_gradient_tracking(&func);
        assert!(violations.is_empty()); // a.requires_grad = true
    }

    #[test]
    fn v4_5_gradient_missing() {
        let mut func = make_clean_func("backward_bad");
        func.tensors.insert(
            "x".to_string(),
            TensorInfo {
                shape: vec![TensorDim::Concrete(4)],
                dtype: TensorDtype::F32,
                requires_grad: false,
                is_contiguous: true,
                layout: MemoryLayout::RowMajor,
            },
        );
        func.operations = vec![DeviceOp::Backward {
            tensor: "x".to_string(),
        }];
        let violations = check_gradient_tracking(&func);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].kind, DeviceViolationKind::GradientTracking);
    }

    // --- V4.6: Numerical Stability ---

    #[test]
    fn v4_6_division_warning() {
        let mut func = make_clean_func("div_fn");
        func.operations = vec![DeviceOp::Division {
            numerator: "a".to_string(),
            denominator: "b".to_string(),
        }];
        let violations = check_numerical_stability(&func);
        assert!(!violations.is_empty());
        assert!(
            violations
                .iter()
                .any(|v| v.kind == DeviceViolationKind::NumericalStability)
        );
    }

    #[test]
    fn v4_6_exp_and_log_warnings() {
        let mut func = make_clean_func("math_fn");
        func.operations = vec![
            DeviceOp::Exp {
                input: "a".to_string(),
            },
            DeviceOp::Log {
                input: "b".to_string(),
            },
        ];
        let violations = check_numerical_stability(&func);
        assert_eq!(violations.len(), 2);
    }

    // --- V4.7: Broadcast Compatibility ---

    #[test]
    fn v4_7_broadcast_compatible() {
        let func = DeviceFunction {
            name: "bcast".to_string(),
            file: "ml.fj".to_string(),
            line: 1,
            operations: vec![DeviceOp::Broadcast {
                lhs: "x".to_string(),
                rhs: "y".to_string(),
                output: "z".to_string(),
            }],
            tensors: HashMap::from([
                ("x".to_string(), make_tensor(&[3, 1], TensorDtype::F32)),
                ("y".to_string(), make_tensor(&[1, 5], TensorDtype::F32)),
            ]),
        };
        let violations = check_broadcast_compatibility(&func);
        assert!(violations.is_empty());
    }

    #[test]
    fn v4_7_broadcast_incompatible() {
        let func = DeviceFunction {
            name: "bcast_bad".to_string(),
            file: "ml.fj".to_string(),
            line: 1,
            operations: vec![DeviceOp::Broadcast {
                lhs: "x".to_string(),
                rhs: "y".to_string(),
                output: "z".to_string(),
            }],
            tensors: HashMap::from([
                ("x".to_string(), make_tensor(&[3, 4], TensorDtype::F32)),
                ("y".to_string(), make_tensor(&[5, 4], TensorDtype::F32)),
            ]),
        };
        let violations = check_broadcast_compatibility(&func);
        assert!(!violations.is_empty());
        assert!(
            violations
                .iter()
                .any(|v| v.kind == DeviceViolationKind::BroadcastCompat)
        );
    }

    // --- V4.8: Memory Layout ---

    #[test]
    fn v4_8_layout_contiguous() {
        let func = make_clean_func("matmul_contig");
        let violations = check_memory_layout(&func);
        assert!(violations.is_empty());
    }

    #[test]
    fn v4_8_layout_non_contiguous() {
        let mut func = make_clean_func("matmul_noncontig");
        func.tensors.insert(
            "a".to_string(),
            TensorInfo {
                shape: vec![TensorDim::Concrete(4), TensorDim::Concrete(3)],
                dtype: TensorDtype::F32,
                requires_grad: true,
                is_contiguous: false, // non-contiguous
                layout: MemoryLayout::RowMajor,
            },
        );
        let violations = check_memory_layout(&func);
        assert!(!violations.is_empty());
        assert!(
            violations
                .iter()
                .any(|v| v.kind == DeviceViolationKind::MemoryLayout)
        );
    }

    // --- V4.9-V4.10: Full Checker ---

    #[test]
    fn v4_9_checker_clean() {
        let mut checker = DeviceSafetyChecker::with_defaults();
        let func = make_clean_func("good");
        let violations = checker.check_function(&func);
        // The clean func may get numerical stability warnings from optional checks,
        // but no mandatory violations. Filter mandatory:
        let mandatory: Vec<_> = violations
            .iter()
            .filter(|v| !matches!(v.kind, DeviceViolationKind::NumericalStability))
            .collect();
        assert!(mandatory.is_empty());
    }

    #[test]
    fn v4_9_checker_with_violations() {
        let mut checker = DeviceSafetyChecker::new(DeviceCheckConfig {
            check_numerical: false,
            check_gradients: false,
            check_layout: false,
            check_inference: false,
        });
        let mut func = make_clean_func("bad");
        func.operations.push(DeviceOp::RawPointer {
            description: "ptr::read".to_string(),
        });
        let violations = checker.check_function(&func);
        assert!(!violations.is_empty());
        assert!(checker.count_kind(DeviceViolationKind::RawPointer) >= 1);
    }

    #[test]
    fn v4_10_check_all_multiple() {
        let mut checker = DeviceSafetyChecker::new(DeviceCheckConfig {
            check_numerical: false,
            check_gradients: false,
            check_layout: false,
            check_inference: false,
        });
        let clean = make_clean_func("good");
        let mut bad = make_clean_func("bad");
        bad.operations.push(DeviceOp::RawPointer {
            description: "deref".to_string(),
        });
        checker.check_all(&[clean, bad]);
        assert_eq!(checker.stats.functions_checked, 2);
    }

    #[test]
    fn v4_10_report() {
        let mut checker = DeviceSafetyChecker::new(DeviceCheckConfig {
            check_numerical: false,
            check_gradients: false,
            check_layout: false,
            check_inference: false,
        });
        let func = make_clean_func("ok");
        checker.check_function(&func);
        let report = checker.report();
        assert!(report.contains("1 functions checked"));
    }

    #[test]
    fn v4_10_violation_display() {
        let v = DeviceViolation {
            kind: DeviceViolationKind::RawPointer,
            description: "raw ptr in device".to_string(),
            file: "nn.fj".to_string(),
            line: 99,
            suggestion: "use safe tensor API".to_string(),
        };
        let s = format!("{v}");
        assert!(s.contains("nn.fj:99"));
        assert!(s.contains("DE001"));
    }

    #[test]
    fn v4_10_tensor_dim_display() {
        assert_eq!(format!("{}", TensorDim::Concrete(42)), "42");
        assert_eq!(format!("{}", TensorDim::Symbolic("N".to_string())), "N");
        assert_eq!(format!("{}", TensorDim::Dynamic), "?");
    }

    #[test]
    fn v4_10_dtype_display() {
        assert_eq!(format!("{}", TensorDtype::F32), "f32");
        assert_eq!(format!("{}", TensorDtype::I8), "i8");
    }
}
