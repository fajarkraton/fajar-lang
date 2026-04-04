//! TensorValue — ndarray-backed tensor with shape, dtype, and gradient tracking.
//!
//! The core data structure for ML operations in Fajar Lang. Uses `ndarray::ArrayD<f64>`
//! as the backing store for dynamic-rank tensors.

use ndarray::ArrayD;
use std::fmt;
use thiserror::Error;

use super::autograd::TensorId;

// ═══════════════════════════════════════════════════════════════════════
// Tensor errors
// ═══════════════════════════════════════════════════════════════════════

/// Errors from tensor operations.
#[derive(Debug, Clone, Error, PartialEq)]
pub enum TensorError {
    /// TE001: Shape mismatch in element-wise operation.
    #[error("TE001: shape mismatch: expected {expected:?}, got {got:?}")]
    ShapeMismatch {
        /// Expected shape.
        expected: Vec<usize>,
        /// Actual shape.
        got: Vec<usize>,
    },

    /// TE002: Incompatible shapes for matrix multiplication.
    #[error(
        "TE002: matmul shape mismatch: {left:?} @ {right:?} — inner dims {left_inner} != {right_inner}"
    )]
    MatmulShapeMismatch {
        /// Left operand shape.
        left: Vec<usize>,
        /// Right operand shape.
        right: Vec<usize>,
        /// Left inner dimension.
        left_inner: usize,
        /// Right inner dimension.
        right_inner: usize,
    },

    /// TE003: Invalid reshape — element count does not match.
    #[error(
        "TE003: cannot reshape {from:?} ({from_count} elements) to {to:?} ({to_count} elements)"
    )]
    ReshapeError {
        /// Original shape.
        from: Vec<usize>,
        /// Target shape.
        to: Vec<usize>,
        /// Original element count.
        from_count: usize,
        /// Target element count.
        to_count: usize,
    },

    /// TE004: Operation requires specific rank.
    #[error("TE004: expected rank {expected}, got rank {got}")]
    RankMismatch {
        /// Expected rank.
        expected: usize,
        /// Actual rank.
        got: usize,
    },

    /// TE005: Backward called on non-scalar tensor.
    #[error("TE005: backward() requires scalar tensor (numel=1), got shape {shape:?}")]
    BackwardNonScalar {
        /// Tensor shape.
        shape: Vec<usize>,
    },

    /// TE006: No gradient available.
    #[error("TE006: no gradient for tensor (requires_grad=false or grad not computed)")]
    NoGradient,

    /// TE007: Division by zero in tensor operation.
    #[error("TE007: division by zero in tensor element")]
    DivisionByZero,

    /// TE008: Invalid tensor data.
    #[error("TE008: {reason}")]
    InvalidData {
        /// Reason for invalidity.
        reason: String,
    },
}

// ═══════════════════════════════════════════════════════════════════════
// TensorValue
// ═══════════════════════════════════════════════════════════════════════

/// A runtime tensor value backed by ndarray.
///
/// Holds the data, shape metadata, and optional gradient for autograd.
/// Invariant: `data.shape().iter().product::<usize>() == data.len()`.
#[derive(Debug, Clone)]
pub struct TensorValue {
    /// The tensor data (dynamic-rank, f64).
    data: ArrayD<f64>,
    /// Whether this tensor tracks gradients.
    requires_grad: bool,
    /// Accumulated gradient (same shape as data), if computed.
    grad: Option<ArrayD<f64>>,
    /// Unique identifier for autograd tape tracking.
    id: Option<TensorId>,
}

impl TensorValue {
    // ── Creation ──

    /// Creates a tensor from an ndarray with optional gradient tracking.
    pub fn new(data: ArrayD<f64>, requires_grad: bool) -> Self {
        Self {
            data,
            requires_grad,
            grad: None,
            id: None,
        }
    }

    /// Creates a tensor from an existing ndarray (no gradient tracking).
    pub fn from_ndarray(data: ArrayD<f64>) -> Self {
        Self::new(data, false)
    }

    /// Creates a tensor filled with zeros.
    ///
    /// # Arguments
    /// * `shape` — Dimensions of the tensor (e.g., `[3, 4]` for 3x4 matrix).
    pub fn zeros(shape: &[usize]) -> Self {
        Self::new(ArrayD::zeros(shape), false)
    }

    /// Creates a tensor filled with ones.
    pub fn ones(shape: &[usize]) -> Self {
        Self::new(ArrayD::ones(shape), false)
    }

    /// Creates a tensor filled with a constant value.
    pub fn full(shape: &[usize], value: f64) -> Self {
        Self::new(ArrayD::from_elem(shape, value), false)
    }

    /// Creates a tensor from a flat data vector and shape.
    ///
    /// Returns `Err` if `data.len() != shape.iter().product()`.
    pub fn from_data(data: Vec<f64>, shape: &[usize]) -> Result<Self, TensorError> {
        let expected_len: usize = shape.iter().product();
        if data.len() != expected_len {
            return Err(TensorError::InvalidData {
                reason: format!(
                    "data length {} does not match shape {:?} (expected {})",
                    data.len(),
                    shape,
                    expected_len
                ),
            });
        }
        let arr =
            ArrayD::from_shape_vec(shape.to_vec(), data).map_err(|e| TensorError::InvalidData {
                reason: e.to_string(),
            })?;
        Ok(Self::new(arr, false))
    }

    /// Creates a tensor with random values from standard normal distribution.
    pub fn randn(shape: &[usize]) -> Self {
        use ndarray_rand::RandomExt;
        use ndarray_rand::rand_distr::StandardNormal;
        let arr = ArrayD::random(shape, StandardNormal);
        Self::new(arr, false)
    }

    /// Creates an identity matrix of size n x n.
    pub fn eye(n: usize) -> Self {
        let mut data = ArrayD::zeros(vec![n, n]);
        for i in 0..n {
            data[[i, i]] = 1.0;
        }
        Self::new(data, false)
    }

    // ── Accessors ──

    /// Returns the shape of this tensor as a slice.
    pub fn shape(&self) -> &[usize] {
        self.data.shape()
    }

    /// Returns the number of dimensions (rank).
    pub fn ndim(&self) -> usize {
        self.data.ndim()
    }

    /// Returns the total number of elements.
    pub fn numel(&self) -> usize {
        self.data.len()
    }

    /// Returns whether this tensor tracks gradients.
    pub fn requires_grad(&self) -> bool {
        self.requires_grad
    }

    /// Sets the requires_grad flag.
    pub fn set_requires_grad(&mut self, val: bool) {
        self.requires_grad = val;
    }

    /// Returns a reference to the underlying ndarray data.
    pub fn data(&self) -> &ArrayD<f64> {
        &self.data
    }

    /// Returns a mutable reference to the underlying data.
    pub fn data_mut(&mut self) -> &mut ArrayD<f64> {
        &mut self.data
    }

    /// Returns the gradient if available.
    pub fn grad(&self) -> Option<&ArrayD<f64>> {
        self.grad.as_ref()
    }

    /// Sets the gradient.
    pub fn set_grad(&mut self, grad: ArrayD<f64>) {
        self.grad = Some(grad);
    }

    /// Accumulates gradient (adds to existing gradient).
    pub fn accumulate_grad(&mut self, grad: &ArrayD<f64>) {
        match &mut self.grad {
            Some(existing) => *existing += grad,
            None => self.grad = Some(grad.clone()),
        }
    }

    /// Resets the gradient to None.
    pub fn zero_grad(&mut self) {
        self.grad = None;
    }

    /// Returns the tensor's autograd id, if assigned.
    pub fn id(&self) -> Option<TensorId> {
        self.id
    }

    /// Sets the tensor's autograd id.
    pub fn set_id(&mut self, id: TensorId) {
        self.id = Some(id);
    }

    /// Returns the data as a flat `Vec<f64>`.
    pub fn to_vec(&self) -> Vec<f64> {
        self.data.iter().copied().collect()
    }

    /// Creates a detached copy (no gradient tracking, no autograd id).
    pub fn detach(&self) -> Self {
        Self {
            data: self.data.clone(),
            requires_grad: false,
            grad: None,
            id: None,
        }
    }

    /// Returns a scalar value if this tensor has exactly one element.
    pub fn to_scalar(&self) -> Result<f64, TensorError> {
        if self.numel() != 1 {
            return Err(TensorError::BackwardNonScalar {
                shape: self.shape().to_vec(),
            });
        }
        Ok(self.data.iter().next().copied().unwrap_or(0.0))
    }
}

impl fmt::Display for TensorValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let n = self.numel();
        if self.ndim() == 0 {
            // Scalar tensor
            write!(f, "tensor({})", self.data.iter().next().unwrap_or(&0.0))
        } else if self.ndim() == 1 && n <= 10 {
            // Small 1D: show all elements
            let vals: Vec<String> = self.data.iter().map(|v| format!("{v:.4}")).collect();
            write!(f, "tensor([{}])", vals.join(", "))
        } else if self.ndim() == 1 {
            // Large 1D: show first 4 + ... + last 2
            let data: Vec<f64> = self.data.iter().copied().collect();
            let head: Vec<String> = data[..4].iter().map(|v| format!("{v:.4}")).collect();
            let tail: Vec<String> = data[n - 2..].iter().map(|v| format!("{v:.4}")).collect();
            write!(
                f,
                "tensor([{}, ..., {}], shape=[{n}])",
                head.join(", "),
                tail.join(", ")
            )
        } else if self.ndim() == 2 && n <= 50 {
            // Small 2D: show as matrix
            let shape = self.shape();
            let rows = shape[0];
            let cols = shape[1];
            let data: Vec<f64> = self.data.iter().copied().collect();
            write!(f, "tensor([")?;
            for r in 0..rows {
                if r > 0 {
                    write!(f, "        ")?;
                }
                write!(f, "[")?;
                for c in 0..cols {
                    if c > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{:.4}", data[r * cols + c])?;
                }
                write!(f, "]")?;
                if r < rows - 1 {
                    writeln!(f, ",")?;
                }
            }
            write!(f, "])")
        } else {
            // Large multi-dimensional: show first 6 values + shape
            let data: Vec<f64> = self.data.iter().copied().collect();
            let preview: Vec<String> = data.iter().take(6).map(|v| format!("{v:.4}")).collect();
            write!(
                f,
                "tensor([{}, ...], shape={:?})",
                preview.join(", "),
                self.shape()
            )
        }
    }
}

impl PartialEq for TensorValue {
    fn eq(&self, other: &Self) -> bool {
        self.shape() == other.shape() && self.data == other.data
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zeros_creates_correct_shape() {
        let t = TensorValue::zeros(&[3, 4]);
        assert_eq!(t.shape(), &[3, 4]);
        assert_eq!(t.numel(), 12);
        assert_eq!(t.ndim(), 2);
        assert!(t.to_vec().iter().all(|&v| v == 0.0));
    }

    #[test]
    fn ones_creates_correct_values() {
        let t = TensorValue::ones(&[2, 3]);
        assert_eq!(t.shape(), &[2, 3]);
        assert!(t.to_vec().iter().all(|&v| v == 1.0));
    }

    #[test]
    fn full_creates_constant_tensor() {
        let t = TensorValue::full(&[2, 2], 42.0);
        assert!(t.to_vec().iter().all(|&v| v == 42.0));
    }

    #[test]
    fn from_data_creates_tensor() {
        let t = TensorValue::from_data(vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0], &[2, 3]).unwrap();
        assert_eq!(t.shape(), &[2, 3]);
        assert_eq!(t.numel(), 6);
        assert_eq!(t.to_vec(), vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
    }

    #[test]
    fn from_data_rejects_mismatched_length() {
        let result = TensorValue::from_data(vec![1.0, 2.0, 3.0], &[2, 3]);
        assert!(matches!(result, Err(TensorError::InvalidData { .. })));
    }

    #[test]
    fn randn_creates_correct_shape() {
        let t = TensorValue::randn(&[10, 5]);
        assert_eq!(t.shape(), &[10, 5]);
        assert_eq!(t.numel(), 50);
        // Check values are not all zero (extremely unlikely for randn)
        assert!(t.to_vec().iter().any(|&v| v != 0.0));
    }

    #[test]
    fn eye_creates_identity() {
        let t = TensorValue::eye(3);
        assert_eq!(t.shape(), &[3, 3]);
        let data = t.to_vec();
        // Diagonal should be 1.0, off-diagonal 0.0
        assert_eq!(data, vec![1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn requires_grad_default_false() {
        let t = TensorValue::zeros(&[2]);
        assert!(!t.requires_grad());
    }

    #[test]
    fn set_requires_grad() {
        let mut t = TensorValue::zeros(&[2]);
        t.set_requires_grad(true);
        assert!(t.requires_grad());
    }

    #[test]
    fn grad_operations() {
        let mut t = TensorValue::zeros(&[2, 3]);
        assert!(t.grad().is_none());

        let grad = ArrayD::ones(vec![2, 3]);
        t.set_grad(grad);
        assert!(t.grad().is_some());
        assert_eq!(t.grad().unwrap().shape(), &[2, 3]);

        // Accumulate
        let grad2 = ArrayD::ones(vec![2, 3]);
        t.accumulate_grad(&grad2);
        assert!(t.grad().unwrap().iter().all(|&v| v == 2.0));

        // Zero grad
        t.zero_grad();
        assert!(t.grad().is_none());
    }

    #[test]
    fn to_scalar_success() {
        let t = TensorValue::from_data(vec![42.0], &[1]).unwrap();
        assert_eq!(t.to_scalar().unwrap(), 42.0);
    }

    #[test]
    fn to_scalar_fails_for_multi_element() {
        let t = TensorValue::zeros(&[2, 3]);
        assert!(matches!(
            t.to_scalar(),
            Err(TensorError::BackwardNonScalar { .. })
        ));
    }

    #[test]
    fn display_scalar() {
        let t = TensorValue::from_data(vec![3.14], &[]).unwrap();
        assert_eq!(format!("{t}"), "tensor(3.14)");
    }

    #[test]
    fn display_small_1d() {
        let t = TensorValue::from_data(vec![1.0, 2.0, 3.0], &[3]).unwrap();
        assert_eq!(format!("{t}"), "tensor([1.0000, 2.0000, 3.0000])");
    }

    #[test]
    fn display_large_tensor() {
        let t = TensorValue::zeros(&[10, 20]);
        // Large 2D (200 elements > 50): shows first 6 values + shape
        let display = format!("{t}");
        assert!(
            display.contains("shape=[10, 20]"),
            "should show shape: {display}"
        );
        assert!(display.contains("0.0000"), "should show values: {display}");
    }

    #[test]
    fn equality() {
        let a = TensorValue::from_data(vec![1.0, 2.0], &[2]).unwrap();
        let b = TensorValue::from_data(vec![1.0, 2.0], &[2]).unwrap();
        let c = TensorValue::from_data(vec![1.0, 3.0], &[2]).unwrap();
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn equality_different_shape() {
        let a = TensorValue::from_data(vec![1.0, 2.0, 3.0, 4.0], &[2, 2]).unwrap();
        let b = TensorValue::from_data(vec![1.0, 2.0, 3.0, 4.0], &[4]).unwrap();
        assert_ne!(a, b); // Same data, different shape
    }

    #[test]
    fn one_dimensional_tensor() {
        let t = TensorValue::zeros(&[5]);
        assert_eq!(t.ndim(), 1);
        assert_eq!(t.numel(), 5);
    }

    #[test]
    fn three_dimensional_tensor() {
        let t = TensorValue::zeros(&[2, 3, 4]);
        assert_eq!(t.ndim(), 3);
        assert_eq!(t.numel(), 24);
    }
}
