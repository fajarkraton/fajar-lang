//! Stack-allocated tensors — fixed-size, no heap, for embedded/kernel contexts.
//!
//! `StackTensor<N>` holds up to `N` f64 elements on the stack with
//! a compile-time-known capacity and runtime shape. Suitable for
//! @kernel contexts where heap allocation is forbidden.

/// Maximum elements a stack tensor can hold.
/// Configurable per use case; 256 elements = 2KB on stack.
pub const MAX_STACK_TENSOR_ELEMENTS: usize = 256;

/// A fixed-capacity tensor stored entirely on the stack.
///
/// No heap allocation. Shape is set at runtime but capacity is fixed.
/// This is suitable for embedded inference where tensors are small
/// and heap allocation is not available.
#[derive(Clone)]
pub struct StackTensor<const N: usize> {
    /// Data buffer (stack-allocated).
    data: [f64; N],
    /// Number of elements actually in use (<= N).
    len: usize,
    /// Shape dimensions (max 4D).
    shape: [usize; 4],
    /// Number of shape dimensions (rank).
    ndim: usize,
}

impl<const N: usize> StackTensor<N> {
    /// Creates a zero-filled stack tensor with the given shape.
    ///
    /// Returns `None` if the shape requires more than `N` elements.
    pub fn zeros(shape: &[usize]) -> Option<Self> {
        if shape.len() > 4 {
            return None;
        }
        let len: usize = shape.iter().product();
        if len > N {
            return None;
        }
        let mut shape_arr = [0usize; 4];
        for (i, &d) in shape.iter().enumerate() {
            shape_arr[i] = d;
        }
        Some(Self {
            data: [0.0; N],
            len,
            shape: shape_arr,
            ndim: shape.len(),
        })
    }

    /// Creates a stack tensor from a slice of data and shape.
    ///
    /// Returns `None` if data doesn't fit or shape doesn't match.
    pub fn from_slice(data: &[f64], shape: &[usize]) -> Option<Self> {
        if shape.len() > 4 {
            return None;
        }
        let expected_len: usize = shape.iter().product();
        if expected_len != data.len() || expected_len > N {
            return None;
        }
        let mut buf = [0.0; N];
        buf[..data.len()].copy_from_slice(data);
        let mut shape_arr = [0usize; 4];
        for (i, &d) in shape.iter().enumerate() {
            shape_arr[i] = d;
        }
        Some(Self {
            data: buf,
            len: expected_len,
            shape: shape_arr,
            ndim: shape.len(),
        })
    }

    /// Returns the shape as a slice.
    pub fn shape(&self) -> &[usize] {
        &self.shape[..self.ndim]
    }

    /// Returns the number of elements.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns true if the tensor has zero elements.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns the rank (number of dimensions).
    pub fn ndim(&self) -> usize {
        self.ndim
    }

    /// Returns a reference to the data slice.
    pub fn data(&self) -> &[f64] {
        &self.data[..self.len]
    }

    /// Returns a mutable reference to the data slice.
    pub fn data_mut(&mut self) -> &mut [f64] {
        &mut self.data[..self.len]
    }

    /// Gets a single element by flat index.
    pub fn get(&self, idx: usize) -> Option<f64> {
        if idx < self.len {
            Some(self.data[idx])
        } else {
            None
        }
    }

    /// Sets a single element by flat index.
    pub fn set(&mut self, idx: usize, val: f64) -> bool {
        if idx < self.len {
            self.data[idx] = val;
            true
        } else {
            false
        }
    }

    /// Element-wise addition. Both tensors must have the same shape.
    pub fn add(&self, other: &Self) -> Option<Self> {
        if self.shape() != other.shape() {
            return None;
        }
        let mut result = self.clone();
        for i in 0..self.len {
            result.data[i] += other.data[i];
        }
        Some(result)
    }

    /// Element-wise subtraction.
    pub fn sub(&self, other: &Self) -> Option<Self> {
        if self.shape() != other.shape() {
            return None;
        }
        let mut result = self.clone();
        for i in 0..self.len {
            result.data[i] -= other.data[i];
        }
        Some(result)
    }

    /// Element-wise multiplication.
    pub fn mul(&self, other: &Self) -> Option<Self> {
        if self.shape() != other.shape() {
            return None;
        }
        let mut result = self.clone();
        for i in 0..self.len {
            result.data[i] *= other.data[i];
        }
        Some(result)
    }

    /// Scalar multiplication.
    pub fn scale(&self, scalar: f64) -> Self {
        let mut result = self.clone();
        for i in 0..self.len {
            result.data[i] *= scalar;
        }
        result
    }

    /// Applies ReLU activation: max(0, x).
    pub fn relu(&self) -> Self {
        let mut result = self.clone();
        for i in 0..self.len {
            if result.data[i] < 0.0 {
                result.data[i] = 0.0;
            }
        }
        result
    }

    /// Returns the sum of all elements.
    pub fn sum(&self) -> f64 {
        let mut s = 0.0;
        for i in 0..self.len {
            s += self.data[i];
        }
        s
    }

    /// Returns the index of the maximum element.
    pub fn argmax(&self) -> usize {
        let mut max_idx = 0;
        let mut max_val = f64::NEG_INFINITY;
        for i in 0..self.len {
            if self.data[i] > max_val {
                max_val = self.data[i];
                max_idx = i;
            }
        }
        max_idx
    }
}

/// Stack-allocated matrix multiplication.
///
/// `a` shape: `[M, K]`, `b` shape: `[K, P]` → result: `[M, P]`.
/// Returns `None` if shapes are incompatible or result exceeds capacity.
pub fn stack_matmul<const N: usize>(
    a: &StackTensor<N>,
    b: &StackTensor<N>,
) -> Option<StackTensor<N>> {
    if a.ndim() != 2 || b.ndim() != 2 {
        return None;
    }
    let m = a.shape()[0];
    let k = a.shape()[1];
    if b.shape()[0] != k {
        return None;
    }
    let p = b.shape()[1];

    if m * p > N {
        return None;
    }

    let mut result = StackTensor::<N>::zeros(&[m, p])?;
    for i in 0..m {
        for j in 0..p {
            let mut acc = 0.0;
            for kk in 0..k {
                acc += a.data[i * k + kk] * b.data[kk * p + j];
            }
            result.data[i * p + j] = acc;
        }
    }
    Some(result)
}

/// Stack-allocated dense layer forward pass: `y = x @ W + b`.
///
/// `x` shape: `[batch, in_features]`
/// `w` shape: `[in_features, out_features]`
/// `b` data length: `out_features` (broadcast per row)
pub fn stack_dense_forward<const N: usize>(
    x: &StackTensor<N>,
    w: &StackTensor<N>,
    b: &StackTensor<N>,
) -> Option<StackTensor<N>> {
    let mut result = stack_matmul(x, w)?;
    // Add bias (broadcast across batch dimension)
    let batch = result.shape()[0];
    let out_features = result.shape()[1];
    if b.len() != out_features {
        return None;
    }
    for i in 0..batch {
        for j in 0..out_features {
            result.data[i * out_features + j] += b.data[j];
        }
    }
    Some(result)
}

impl<const N: usize> std::fmt::Debug for StackTensor<N> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "StackTensor<{N}>(shape={:?}, data={:?})",
            self.shape(),
            self.data()
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    type T64 = StackTensor<64>;

    #[test]
    fn zeros_creates_correct_shape() {
        let t = T64::zeros(&[2, 3]).unwrap();
        assert_eq!(t.shape(), &[2, 3]);
        assert_eq!(t.len(), 6);
        assert_eq!(t.ndim(), 2);
        assert!(t.data().iter().all(|&v| v == 0.0));
    }

    #[test]
    fn zeros_rejects_too_large() {
        let result = StackTensor::<4>::zeros(&[3, 3]); // 9 > 4
        assert!(result.is_none());
    }

    #[test]
    fn from_slice_basic() {
        let t = T64::from_slice(&[1.0, 2.0, 3.0, 4.0], &[2, 2]).unwrap();
        assert_eq!(t.shape(), &[2, 2]);
        assert_eq!(t.data(), &[1.0, 2.0, 3.0, 4.0]);
    }

    #[test]
    fn from_slice_mismatched_length() {
        let result = T64::from_slice(&[1.0, 2.0], &[3]);
        assert!(result.is_none());
    }

    #[test]
    fn get_set() {
        let mut t = T64::from_slice(&[10.0, 20.0, 30.0], &[3]).unwrap();
        assert_eq!(t.get(1), Some(20.0));
        assert_eq!(t.get(5), None);
        assert!(t.set(1, 99.0));
        assert_eq!(t.get(1), Some(99.0));
        assert!(!t.set(5, 0.0));
    }

    #[test]
    fn element_wise_add() {
        let a = T64::from_slice(&[1.0, 2.0, 3.0], &[3]).unwrap();
        let b = T64::from_slice(&[10.0, 20.0, 30.0], &[3]).unwrap();
        let c = a.add(&b).unwrap();
        assert_eq!(c.data(), &[11.0, 22.0, 33.0]);
    }

    #[test]
    fn element_wise_sub() {
        let a = T64::from_slice(&[10.0, 20.0], &[2]).unwrap();
        let b = T64::from_slice(&[3.0, 7.0], &[2]).unwrap();
        let c = a.sub(&b).unwrap();
        assert_eq!(c.data(), &[7.0, 13.0]);
    }

    #[test]
    fn element_wise_mul() {
        let a = T64::from_slice(&[2.0, 3.0], &[2]).unwrap();
        let b = T64::from_slice(&[4.0, 5.0], &[2]).unwrap();
        let c = a.mul(&b).unwrap();
        assert_eq!(c.data(), &[8.0, 15.0]);
    }

    #[test]
    fn shape_mismatch_returns_none() {
        let a = T64::from_slice(&[1.0, 2.0], &[2]).unwrap();
        let b = T64::from_slice(&[1.0, 2.0, 3.0], &[3]).unwrap();
        assert!(a.add(&b).is_none());
    }

    #[test]
    fn scalar_scale() {
        let t = T64::from_slice(&[1.0, 2.0, 3.0], &[3]).unwrap();
        let s = t.scale(2.0);
        assert_eq!(s.data(), &[2.0, 4.0, 6.0]);
    }

    #[test]
    fn relu_activation() {
        let t = T64::from_slice(&[-1.0, 0.0, 1.0, -5.0, 3.0], &[5]).unwrap();
        let r = t.relu();
        assert_eq!(r.data(), &[0.0, 0.0, 1.0, 0.0, 3.0]);
    }

    #[test]
    fn sum_all_elements() {
        let t = T64::from_slice(&[1.0, 2.0, 3.0, 4.0], &[4]).unwrap();
        assert_eq!(t.sum(), 10.0);
    }

    #[test]
    fn argmax_finds_max() {
        let t = T64::from_slice(&[1.0, 5.0, 3.0, 2.0], &[4]).unwrap();
        assert_eq!(t.argmax(), 1);
    }

    #[test]
    fn stack_matmul_2x2() {
        // [[1, 2], [3, 4]] @ [[5, 6], [7, 8]] = [[19, 22], [43, 50]]
        let a = T64::from_slice(&[1.0, 2.0, 3.0, 4.0], &[2, 2]).unwrap();
        let b = T64::from_slice(&[5.0, 6.0, 7.0, 8.0], &[2, 2]).unwrap();
        let c = stack_matmul(&a, &b).unwrap();
        assert_eq!(c.shape(), &[2, 2]);
        assert_eq!(c.data(), &[19.0, 22.0, 43.0, 50.0]);
    }

    #[test]
    fn stack_matmul_non_square() {
        // [1, 2, 3] @ [[1], [2], [3]] = [14]
        let a = T64::from_slice(&[1.0, 2.0, 3.0], &[1, 3]).unwrap();
        let b = T64::from_slice(&[1.0, 2.0, 3.0], &[3, 1]).unwrap();
        let c = stack_matmul(&a, &b).unwrap();
        assert_eq!(c.shape(), &[1, 1]);
        assert_eq!(c.data(), &[14.0]);
    }

    #[test]
    fn stack_matmul_shape_mismatch() {
        let a = T64::from_slice(&[1.0, 2.0, 3.0], &[1, 3]).unwrap();
        let b = T64::from_slice(&[1.0, 2.0], &[1, 2]).unwrap();
        assert!(stack_matmul(&a, &b).is_none());
    }

    #[test]
    fn stack_matmul_too_large() {
        // Result would be 4x4=16, but capacity is only 8
        let a = StackTensor::<8>::from_slice(&[1.0; 8], &[4, 2]).unwrap();
        let b = StackTensor::<8>::from_slice(&[1.0; 8], &[2, 4]).unwrap();
        assert!(stack_matmul(&a, &b).is_none()); // 4*4=16 > 8
    }

    #[test]
    fn stack_dense_forward_basic() {
        // x: [1, 2], w: [[1, 0], [0, 1]], b: [0.5, 0.5]
        // result = [1, 2] @ I + [0.5, 0.5] = [1.5, 2.5]
        let x = T64::from_slice(&[1.0, 2.0], &[1, 2]).unwrap();
        let w = T64::from_slice(&[1.0, 0.0, 0.0, 1.0], &[2, 2]).unwrap();
        let b = T64::from_slice(&[0.5, 0.5], &[2]).unwrap();
        let y = stack_dense_forward(&x, &w, &b).unwrap();
        assert_eq!(y.shape(), &[1, 2]);
        assert_eq!(y.data(), &[1.5, 2.5]);
    }

    #[test]
    fn stack_dense_with_relu() {
        let x = T64::from_slice(&[-1.0, 2.0], &[1, 2]).unwrap();
        let w = T64::from_slice(&[1.0, 0.0, 0.0, 1.0], &[2, 2]).unwrap();
        let b = T64::from_slice(&[0.0, 0.0], &[2]).unwrap();
        let y = stack_dense_forward(&x, &w, &b).unwrap().relu();
        assert_eq!(y.data(), &[0.0, 2.0]);
    }

    #[test]
    fn stack_tensor_fits_in_specified_limit() {
        // Verify that a StackTensor<64> with a [4, 4] shape uses exactly 16 elements
        let t = StackTensor::<64>::zeros(&[4, 4]).unwrap();
        assert_eq!(t.len(), 16);
        // The struct itself should be fully on the stack (no heap pointers)
        assert!(std::mem::size_of::<StackTensor<64>>() > 0);
        // Size should be approximately 64*8 (data) + metadata
        let size = std::mem::size_of::<StackTensor<64>>();
        assert!(size <= 64 * 8 + 64, "size {size} should be ~512 + overhead");
    }

    #[test]
    fn is_empty() {
        let t = T64::zeros(&[0]).unwrap();
        assert!(t.is_empty());
        let t2 = T64::zeros(&[1]).unwrap();
        assert!(!t2.is_empty());
    }

    #[test]
    fn max_4d_shape() {
        let t = T64::zeros(&[2, 2, 2, 2]).unwrap();
        assert_eq!(t.ndim(), 4);
        assert_eq!(t.len(), 16);
    }

    #[test]
    fn rejects_5d_shape() {
        let result = T64::zeros(&[1, 1, 1, 1, 1]);
        assert!(result.is_none());
    }
}
