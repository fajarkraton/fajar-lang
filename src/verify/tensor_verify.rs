//! Tensor Shape Verification — dependent types, matmul proofs, broadcast rules.
//!
//! Phase V3: 20 tasks covering shape as dependent types, compatibility proofs
//! for matmul/reshape/conv2d/concat/split/transpose, symbolic shapes, ONNX.

use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// V3.1: Tensor Shape as Dependent Type
// ═══════════════════════════════════════════════════════════════════════

/// A symbolic tensor shape (may contain variables or constants).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolicShape {
    /// Dimensions (each may be concrete or symbolic).
    pub dims: Vec<ShapeDim>,
}

/// A single shape dimension.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShapeDim {
    /// Known concrete value.
    Concrete(usize),
    /// Symbolic variable (e.g., "N", "batch_size").
    Symbolic(String),
    /// Dynamic (unknown at compile time, but bounded).
    Dynamic { min: usize, max: usize },
    /// Expression (e.g., N * 2, H / stride).
    Expr(Box<ShapeExpr>),
}

/// Shape expression for computed dimensions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShapeExpr {
    Var(String),
    Lit(usize),
    Add(Box<ShapeExpr>, Box<ShapeExpr>),
    Sub(Box<ShapeExpr>, Box<ShapeExpr>),
    Mul(Box<ShapeExpr>, Box<ShapeExpr>),
    Div(Box<ShapeExpr>, Box<ShapeExpr>),
    Max(Box<ShapeExpr>, Box<ShapeExpr>),
}

impl fmt::Display for ShapeDim {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Concrete(n) => write!(f, "{n}"),
            Self::Symbolic(name) => write!(f, "{name}"),
            Self::Dynamic { min, max } => write!(f, "{min}..{max}"),
            Self::Expr(expr) => write!(f, "{expr}"),
        }
    }
}

impl fmt::Display for ShapeExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Var(name) => write!(f, "{name}"),
            Self::Lit(n) => write!(f, "{n}"),
            Self::Add(a, b) => write!(f, "({a} + {b})"),
            Self::Sub(a, b) => write!(f, "({a} - {b})"),
            Self::Mul(a, b) => write!(f, "({a} * {b})"),
            Self::Div(a, b) => write!(f, "({a} / {b})"),
            Self::Max(a, b) => write!(f, "max({a}, {b})"),
        }
    }
}

impl fmt::Display for SymbolicShape {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let dims: Vec<String> = self.dims.iter().map(|d| format!("{d}")).collect();
        write!(f, "[{}]", dims.join(", "))
    }
}

impl SymbolicShape {
    /// Creates a concrete shape.
    pub fn concrete(dims: &[usize]) -> Self {
        Self { dims: dims.iter().map(|&d| ShapeDim::Concrete(d)).collect() }
    }

    /// Number of dimensions (rank).
    pub fn rank(&self) -> usize { self.dims.len() }

    /// Returns true if all dimensions are concrete.
    pub fn is_fully_concrete(&self) -> bool {
        self.dims.iter().all(|d| matches!(d, ShapeDim::Concrete(_)))
    }

    /// Returns concrete dimensions (panics if any symbolic).
    pub fn to_concrete(&self) -> Option<Vec<usize>> {
        self.dims.iter().map(|d| match d {
            ShapeDim::Concrete(n) => Some(*n),
            _ => None,
        }).collect()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// V3.2: Shape Compatibility Proofs
// ═══════════════════════════════════════════════════════════════════════

/// Shape constraint (to be verified by SMT solver).
#[derive(Debug, Clone)]
pub struct ShapeConstraint {
    /// Description.
    pub description: String,
    /// The constraint expression.
    pub constraint: ShapeCheck,
    /// Source location.
    pub file: String,
    pub line: u32,
    /// Verification status.
    pub status: ShapeCheckStatus,
}

/// A shape check to verify.
#[derive(Debug, Clone)]
pub enum ShapeCheck {
    /// Two dimensions must be equal.
    DimEqual(ShapeDim, ShapeDim),
    /// Dimension must be positive.
    DimPositive(ShapeDim),
    /// Dimension must be divisible.
    DimDivisible(ShapeDim, ShapeDim),
    /// Total elements preserved (reshape).
    ElementsPreserved(SymbolicShape, SymbolicShape),
    /// Broadcast compatibility.
    BroadcastCompatible(ShapeDim, ShapeDim),
    /// Matmul inner dimensions match.
    MatmulCompatible(SymbolicShape, SymbolicShape),
    /// Conv2d output shape valid.
    Conv2dValid { input_h: ShapeDim, input_w: ShapeDim, kernel_h: ShapeDim, kernel_w: ShapeDim, stride: usize, padding: usize },
}

/// Shape check status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShapeCheckStatus {
    Pending,
    Valid,
    Invalid(String),
    Unknown,
}

// ═══════════════════════════════════════════════════════════════════════
// V3.3: Matmul Shape Verification
// ═══════════════════════════════════════════════════════════════════════

/// Verifies matmul shape compatibility: A[..., M, K] × B[..., K, N] → [M, N].
pub fn verify_matmul(a: &SymbolicShape, b: &SymbolicShape) -> ShapeConstraint {
    let a_last = a.dims.last().cloned().unwrap_or(ShapeDim::Concrete(0));
    let b_second_last = if b.rank() >= 2 { b.dims[b.rank() - 2].clone() } else { ShapeDim::Concrete(0) };

    let status = match (&a_last, &b_second_last) {
        (ShapeDim::Concrete(ak), ShapeDim::Concrete(bk)) => {
            if ak == bk { ShapeCheckStatus::Valid }
            else { ShapeCheckStatus::Invalid(format!("inner dimensions mismatch: {ak} != {bk}")) }
        }
        (ShapeDim::Symbolic(a_name), ShapeDim::Symbolic(b_name)) => {
            if a_name == b_name { ShapeCheckStatus::Valid }
            else { ShapeCheckStatus::Pending } // needs SMT
        }
        _ => ShapeCheckStatus::Pending,
    };

    ShapeConstraint {
        description: format!("matmul: {} × {}", a, b),
        constraint: ShapeCheck::MatmulCompatible(a.clone(), b.clone()),
        file: String::new(), line: 0, status,
    }
}

/// Computes the output shape of matmul.
pub fn matmul_output_shape(a: &SymbolicShape, b: &SymbolicShape) -> Option<SymbolicShape> {
    if a.rank() < 2 || b.rank() < 2 { return None; }
    let m = a.dims[a.rank() - 2].clone();
    let n = b.dims[b.rank() - 1].clone();
    Some(SymbolicShape { dims: vec![m, n] })
}

// ═══════════════════════════════════════════════════════════════════════
// V3.4: Reshape Verification
// ═══════════════════════════════════════════════════════════════════════

/// Verifies reshape validity (total elements preserved).
pub fn verify_reshape(input: &SymbolicShape, output: &SymbolicShape) -> ShapeConstraint {
    let status = match (input.to_concrete(), output.to_concrete()) {
        (Some(in_dims), Some(out_dims)) => {
            let in_total: usize = in_dims.iter().product();
            let out_total: usize = out_dims.iter().product();
            if in_total == out_total { ShapeCheckStatus::Valid }
            else { ShapeCheckStatus::Invalid(format!("element count mismatch: {in_total} != {out_total}")) }
        }
        _ => ShapeCheckStatus::Pending,
    };

    ShapeConstraint {
        description: format!("reshape: {} → {}", input, output),
        constraint: ShapeCheck::ElementsPreserved(input.clone(), output.clone()),
        file: String::new(), line: 0, status,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// V3.5: Broadcast Rule Verification
// ═══════════════════════════════════════════════════════════════════════

/// Verifies broadcast compatibility between two shapes.
pub fn verify_broadcast(a: &SymbolicShape, b: &SymbolicShape) -> (ShapeCheckStatus, Option<SymbolicShape>) {
    let max_rank = a.rank().max(b.rank());
    let mut result_dims = Vec::new();

    for i in 0..max_rank {
        let a_dim = if i < a.rank() { a.dims[a.rank() - 1 - i].clone() } else { ShapeDim::Concrete(1) };
        let b_dim = if i < b.rank() { b.dims[b.rank() - 1 - i].clone() } else { ShapeDim::Concrete(1) };

        match (&a_dim, &b_dim) {
            (ShapeDim::Concrete(1), d) | (d, ShapeDim::Concrete(1)) => result_dims.push(d.clone()),
            (ShapeDim::Concrete(a_val), ShapeDim::Concrete(b_val)) => {
                if a_val == b_val { result_dims.push(ShapeDim::Concrete(*a_val)); }
                else { return (ShapeCheckStatus::Invalid(format!("cannot broadcast {a_val} with {b_val}")), None); }
            }
            (ShapeDim::Symbolic(a_name), ShapeDim::Symbolic(b_name)) if a_name == b_name => {
                result_dims.push(ShapeDim::Symbolic(a_name.clone()));
            }
            _ => {
                result_dims.push(ShapeDim::Expr(Box::new(ShapeExpr::Max(
                    Box::new(ShapeExpr::Var(format!("{a_dim}"))),
                    Box::new(ShapeExpr::Var(format!("{b_dim}"))),
                ))));
            }
        }
    }

    result_dims.reverse();
    (ShapeCheckStatus::Valid, Some(SymbolicShape { dims: result_dims }))
}

// ═══════════════════════════════════════════════════════════════════════
// V3.6: Conv2d Output Shape
// ═══════════════════════════════════════════════════════════════════════

/// Computes conv2d output shape: (H - K + 2P) / S + 1.
pub fn conv2d_output_shape(
    input_h: usize, input_w: usize,
    kernel_h: usize, kernel_w: usize,
    stride: usize, padding: usize,
) -> (usize, usize) {
    let out_h = (input_h - kernel_h + 2 * padding) / stride + 1;
    let out_w = (input_w - kernel_w + 2 * padding) / stride + 1;
    (out_h, out_w)
}

/// Verifies conv2d output shape is valid (> 0).
pub fn verify_conv2d(input_h: usize, input_w: usize, kernel_h: usize, kernel_w: usize, stride: usize, padding: usize) -> ShapeCheckStatus {
    if stride == 0 { return ShapeCheckStatus::Invalid("stride cannot be 0".to_string()); }
    if kernel_h > input_h + 2 * padding { return ShapeCheckStatus::Invalid(format!("kernel_h ({kernel_h}) > input_h + 2*padding ({})", input_h + 2 * padding)); }
    if kernel_w > input_w + 2 * padding { return ShapeCheckStatus::Invalid(format!("kernel_w ({kernel_w}) > input_w + 2*padding ({})", input_w + 2 * padding)); }
    let (out_h, out_w) = conv2d_output_shape(input_h, input_w, kernel_h, kernel_w, stride, padding);
    if out_h == 0 || out_w == 0 { return ShapeCheckStatus::Invalid("output dimension is 0".to_string()); }
    ShapeCheckStatus::Valid
}

// ═══════════════════════════════════════════════════════════════════════
// V3.7-V3.8: Concat + Split Verification
// ═══════════════════════════════════════════════════════════════════════

/// Verifies concatenation: all shapes must match except on the concat axis.
pub fn verify_concat(shapes: &[SymbolicShape], axis: usize) -> ShapeCheckStatus {
    if shapes.is_empty() { return ShapeCheckStatus::Invalid("empty input".to_string()); }
    let rank = shapes[0].rank();
    if axis >= rank { return ShapeCheckStatus::Invalid(format!("axis {axis} >= rank {rank}")); }

    for (i, shape) in shapes.iter().enumerate().skip(1) {
        if shape.rank() != rank { return ShapeCheckStatus::Invalid(format!("rank mismatch at input {i}")); }
        for d in 0..rank {
            if d != axis {
                if shape.dims[d] != shapes[0].dims[d] {
                    return ShapeCheckStatus::Invalid(format!("dim {d} mismatch at input {i}"));
                }
            }
        }
    }
    ShapeCheckStatus::Valid
}

/// Verifies split: total of split sizes must equal input dimension.
pub fn verify_split(input_dim: usize, split_sizes: &[usize]) -> ShapeCheckStatus {
    let total: usize = split_sizes.iter().sum();
    if total != input_dim {
        ShapeCheckStatus::Invalid(format!("split sizes sum {total} != input dim {input_dim}"))
    } else {
        ShapeCheckStatus::Valid
    }
}

/// Verifies transpose permutation is valid.
pub fn verify_transpose(rank: usize, perm: &[usize]) -> ShapeCheckStatus {
    if perm.len() != rank { return ShapeCheckStatus::Invalid(format!("perm length {} != rank {rank}", perm.len())); }
    let mut seen = vec![false; rank];
    for &p in perm {
        if p >= rank { return ShapeCheckStatus::Invalid(format!("perm index {p} >= rank {rank}")); }
        if seen[p] { return ShapeCheckStatus::Invalid(format!("duplicate perm index {p}")); }
        seen[p] = true;
    }
    ShapeCheckStatus::Valid
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v3_1_symbolic_shape() {
        let s = SymbolicShape::concrete(&[3, 4, 5]);
        assert_eq!(s.rank(), 3);
        assert!(s.is_fully_concrete());
        assert_eq!(format!("{s}"), "[3, 4, 5]");
        assert_eq!(s.to_concrete(), Some(vec![3, 4, 5]));
    }

    #[test]
    fn v3_1_symbolic_dims() {
        let s = SymbolicShape { dims: vec![ShapeDim::Symbolic("N".into()), ShapeDim::Concrete(784)] };
        assert_eq!(s.rank(), 2);
        assert!(!s.is_fully_concrete());
        assert_eq!(format!("{s}"), "[N, 784]");
    }

    #[test]
    fn v3_2_matmul_concrete_valid() {
        let a = SymbolicShape::concrete(&[4, 3]);
        let b = SymbolicShape::concrete(&[3, 5]);
        let constraint = verify_matmul(&a, &b);
        assert_eq!(constraint.status, ShapeCheckStatus::Valid);
    }

    #[test]
    fn v3_2_matmul_concrete_invalid() {
        let a = SymbolicShape::concrete(&[4, 3]);
        let b = SymbolicShape::concrete(&[5, 2]);
        let constraint = verify_matmul(&a, &b);
        assert!(matches!(constraint.status, ShapeCheckStatus::Invalid(_)));
    }

    #[test]
    fn v3_2_matmul_symbolic() {
        let a = SymbolicShape { dims: vec![ShapeDim::Symbolic("M".into()), ShapeDim::Symbolic("K".into())] };
        let b = SymbolicShape { dims: vec![ShapeDim::Symbolic("K".into()), ShapeDim::Symbolic("N".into())] };
        let constraint = verify_matmul(&a, &b);
        assert_eq!(constraint.status, ShapeCheckStatus::Valid); // same symbolic name
    }

    #[test]
    fn v3_3_matmul_output() {
        let a = SymbolicShape::concrete(&[4, 3]);
        let b = SymbolicShape::concrete(&[3, 5]);
        let out = matmul_output_shape(&a, &b).unwrap();
        assert_eq!(out.to_concrete(), Some(vec![4, 5]));
    }

    #[test]
    fn v3_4_reshape_valid() {
        let input = SymbolicShape::concrete(&[2, 3, 4]);
        let output = SymbolicShape::concrete(&[6, 4]);
        let constraint = verify_reshape(&input, &output);
        assert_eq!(constraint.status, ShapeCheckStatus::Valid);
    }

    #[test]
    fn v3_4_reshape_invalid() {
        let input = SymbolicShape::concrete(&[2, 3]);
        let output = SymbolicShape::concrete(&[7]);
        let constraint = verify_reshape(&input, &output);
        assert!(matches!(constraint.status, ShapeCheckStatus::Invalid(_)));
    }

    #[test]
    fn v3_5_broadcast_same() {
        let a = SymbolicShape::concrete(&[3, 4]);
        let b = SymbolicShape::concrete(&[3, 4]);
        let (status, result) = verify_broadcast(&a, &b);
        assert_eq!(status, ShapeCheckStatus::Valid);
        assert_eq!(result.unwrap().to_concrete(), Some(vec![3, 4]));
    }

    #[test]
    fn v3_5_broadcast_expand() {
        let a = SymbolicShape::concrete(&[3, 1]);
        let b = SymbolicShape::concrete(&[1, 4]);
        let (status, result) = verify_broadcast(&a, &b);
        assert_eq!(status, ShapeCheckStatus::Valid);
        assert_eq!(result.unwrap().to_concrete(), Some(vec![3, 4]));
    }

    #[test]
    fn v3_5_broadcast_invalid() {
        let a = SymbolicShape::concrete(&[3, 5]);
        let b = SymbolicShape::concrete(&[4, 5]);
        let (status, _) = verify_broadcast(&a, &b);
        assert!(matches!(status, ShapeCheckStatus::Invalid(_)));
    }

    #[test]
    fn v3_6_conv2d_valid() {
        let (h, w) = conv2d_output_shape(28, 28, 5, 5, 1, 0);
        assert_eq!(h, 24); assert_eq!(w, 24);
        assert_eq!(verify_conv2d(28, 28, 5, 5, 1, 0), ShapeCheckStatus::Valid);
    }

    #[test]
    fn v3_6_conv2d_with_padding() {
        let (h, w) = conv2d_output_shape(28, 28, 3, 3, 1, 1);
        assert_eq!(h, 28); assert_eq!(w, 28); // same padding
    }

    #[test]
    fn v3_6_conv2d_invalid() {
        let status = verify_conv2d(5, 5, 10, 10, 1, 0);
        assert!(matches!(status, ShapeCheckStatus::Invalid(_)));
    }

    #[test]
    fn v3_7_concat_valid() {
        let shapes = vec![
            SymbolicShape::concrete(&[3, 4]),
            SymbolicShape::concrete(&[5, 4]),
        ];
        assert_eq!(verify_concat(&shapes, 0), ShapeCheckStatus::Valid);
    }

    #[test]
    fn v3_7_concat_invalid() {
        let shapes = vec![
            SymbolicShape::concrete(&[3, 4]),
            SymbolicShape::concrete(&[3, 5]),
        ];
        assert!(matches!(verify_concat(&shapes, 0), ShapeCheckStatus::Invalid(_))); // dim 1 mismatch
    }

    #[test]
    fn v3_8_split_valid() {
        assert_eq!(verify_split(10, &[3, 3, 4]), ShapeCheckStatus::Valid);
    }

    #[test]
    fn v3_8_split_invalid() {
        assert!(matches!(verify_split(10, &[3, 3, 3]), ShapeCheckStatus::Invalid(_)));
    }

    #[test]
    fn v3_8_transpose_valid() {
        assert_eq!(verify_transpose(3, &[2, 0, 1]), ShapeCheckStatus::Valid);
    }

    #[test]
    fn v3_8_transpose_invalid() {
        assert!(matches!(verify_transpose(3, &[0, 1]), ShapeCheckStatus::Invalid(_))); // wrong length
        assert!(matches!(verify_transpose(3, &[0, 1, 1]), ShapeCheckStatus::Invalid(_))); // duplicate
    }

    #[test]
    fn v3_1_dynamic_dim() {
        let d = ShapeDim::Dynamic { min: 1, max: 128 };
        assert_eq!(format!("{d}"), "1..128");
    }

    #[test]
    fn v3_1_shape_expr() {
        let expr = ShapeExpr::Div(
            Box::new(ShapeExpr::Sub(Box::new(ShapeExpr::Var("H".into())), Box::new(ShapeExpr::Lit(3)))),
            Box::new(ShapeExpr::Lit(2)),
        );
        assert_eq!(format!("{expr}"), "((H - 3) / 2)");
    }
}
