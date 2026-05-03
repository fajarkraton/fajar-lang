//! # Structured Sparsity
//!
//! 4:2 structured sparsity (NVIDIA Ampere+), CSR/CSC sparse storage,
//! pruning API, and sparse-dense matrix multiplication.
//!
//! ## 4:2 Sparsity Pattern
//!
//! Every group of 4 consecutive elements must have exactly 2 zeros.
//! The sparsity mask is stored as 2-bit indices per group (NVIDIA format),
//! enabling 2× Tensor Core throughput on Ampere and later GPUs.

use super::tensor::{TensorError, TensorValue};
use serde::Serialize;

// ═══════════════════════════════════════════════════════════════════════
// Sparse Format
// ═══════════════════════════════════════════════════════════════════════

/// Sparse storage format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum SparseFormat {
    /// 4:2 structured sparsity (2 non-zeros per 4 elements).
    Structured4x2,
    /// Compressed Sparse Row.
    CSR,
    /// Compressed Sparse Column.
    CSC,
}

impl std::fmt::Display for SparseFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SparseFormat::Structured4x2 => write!(f, "4:2"),
            SparseFormat::CSR => write!(f, "CSR"),
            SparseFormat::CSC => write!(f, "CSC"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// 4:2 Structured Sparsity
// ═══════════════════════════════════════════════════════════════════════

/// 4:2 structured sparse tensor.
///
/// Stores only the 2 non-zero values per group of 4, plus a metadata
/// byte encoding which 2 of the 4 positions hold values.
#[derive(Debug, Clone)]
pub struct Structured4x2Tensor {
    /// Non-zero values (2 per group of 4).
    values: Vec<f64>,
    /// Metadata: 2-bit index pairs per group. Each byte encodes one group:
    /// bits `[1:0]` = index of first non-zero, bits `[3:2]` = index of second.
    metadata: Vec<u8>,
    /// Original dense shape.
    shape: Vec<usize>,
    /// Total elements in the dense representation.
    num_elements: usize,
}

impl Structured4x2Tensor {
    /// Apply 4:2 pruning to a dense tensor.
    ///
    /// For each group of 4 elements, keeps the 2 with largest magnitude
    /// and zeros the other 2.
    pub fn from_dense(tensor: &TensorValue) -> Self {
        let dense = tensor.to_vec();
        let n = dense.len();
        let num_groups = n.div_ceil(4);
        let mut values = Vec::with_capacity(num_groups * 2);
        let mut metadata = Vec::with_capacity(num_groups);

        for group_start in (0..n).step_by(4) {
            let group_end = (group_start + 4).min(n);
            let group_len = group_end - group_start;

            // Get indices sorted by magnitude (descending)
            let mut indexed: Vec<(usize, f64)> = (0..group_len)
                .map(|i| (i, dense[group_start + i]))
                .collect();
            indexed.sort_by(|a, b| {
                b.1.abs()
                    .partial_cmp(&a.1.abs())
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            // Keep top 2 by magnitude
            let (idx0, idx1) = if group_len >= 2 {
                let mut pair = [indexed[0].0, indexed[1].0];
                pair.sort(); // Store in ascending order
                (pair[0], pair[1])
            } else if group_len == 1 {
                (indexed[0].0, indexed[0].0)
            } else {
                (0, 0)
            };

            values.push(dense.get(group_start + idx0).copied().unwrap_or(0.0));
            values.push(dense.get(group_start + idx1).copied().unwrap_or(0.0));
            metadata.push(((idx1 as u8) << 2) | (idx0 as u8));
        }

        Self {
            values,
            metadata,
            shape: tensor.shape().to_vec(),
            num_elements: n,
        }
    }

    /// Reconstruct the dense tensor (zeros where pruned).
    pub fn to_dense(&self) -> TensorValue {
        let mut dense = vec![0.0_f64; self.num_elements];

        for (group_idx, &meta) in self.metadata.iter().enumerate() {
            let group_start = group_idx * 4;
            let idx0 = (meta & 0x03) as usize;
            let idx1 = ((meta >> 2) & 0x03) as usize;
            let val_base = group_idx * 2;

            if group_start + idx0 < self.num_elements {
                dense[group_start + idx0] = self.values[val_base];
            }
            if group_start + idx1 < self.num_elements && idx0 != idx1 {
                dense[group_start + idx1] = self.values[val_base + 1];
            }
        }

        TensorValue::from_data(dense, &self.shape)
            .unwrap_or_else(|_| TensorValue::zeros(&self.shape))
    }

    /// Number of non-zero values stored.
    pub fn nnz(&self) -> usize {
        self.values.len()
    }

    /// Original dense element count.
    pub fn num_elements(&self) -> usize {
        self.num_elements
    }

    /// Sparsity ratio (fraction of zeros).
    pub fn sparsity(&self) -> f64 {
        if self.num_elements == 0 {
            return 0.0;
        }
        1.0 - (self.nnz() as f64 / self.num_elements as f64)
    }

    /// Memory size in bytes.
    pub fn byte_size(&self) -> usize {
        self.values.len() * 8 + self.metadata.len()
    }

    /// Shape.
    pub fn shape(&self) -> &[usize] {
        &self.shape
    }
}

// ═══════════════════════════════════════════════════════════════════════
// CSR Storage
// ═══════════════════════════════════════════════════════════════════════

/// Compressed Sparse Row format.
///
/// Standard CSR: three arrays — values, column indices, row pointers.
#[derive(Debug, Clone)]
pub struct CsrMatrix {
    /// Non-zero values.
    values: Vec<f64>,
    /// Column indices for each non-zero value.
    col_indices: Vec<usize>,
    /// Row pointer array (length = rows + 1).
    row_ptr: Vec<usize>,
    /// Number of rows.
    rows: usize,
    /// Number of columns.
    cols: usize,
}

impl CsrMatrix {
    /// Create CSR from a dense 2D tensor.
    pub fn from_dense(tensor: &TensorValue) -> Result<Self, TensorError> {
        let shape = tensor.shape();
        if shape.len() != 2 {
            return Err(TensorError::ShapeMismatch {
                expected: vec![0, 0],
                got: shape.to_vec(),
            });
        }

        let rows = shape[0];
        let cols = shape[1];
        let dense = tensor.to_vec();
        let mut values = Vec::new();
        let mut col_indices = Vec::new();
        let mut row_ptr = vec![0usize];

        for r in 0..rows {
            for c in 0..cols {
                let val = dense[r * cols + c];
                if val != 0.0 {
                    values.push(val);
                    col_indices.push(c);
                }
            }
            row_ptr.push(values.len());
        }

        Ok(Self {
            values,
            col_indices,
            row_ptr,
            rows,
            cols,
        })
    }

    /// Reconstruct dense 2D tensor.
    pub fn to_dense(&self) -> TensorValue {
        let mut dense = vec![0.0_f64; self.rows * self.cols];
        for r in 0..self.rows {
            for idx in self.row_ptr[r]..self.row_ptr[r + 1] {
                let c = self.col_indices[idx];
                dense[r * self.cols + c] = self.values[idx];
            }
        }
        TensorValue::from_data(dense, &[self.rows, self.cols])
            .unwrap_or_else(|_| TensorValue::zeros(&[self.rows, self.cols]))
    }

    /// Number of non-zeros.
    pub fn nnz(&self) -> usize {
        self.values.len()
    }

    /// Sparsity ratio.
    pub fn sparsity(&self) -> f64 {
        let total = self.rows * self.cols;
        if total == 0 {
            return 0.0;
        }
        1.0 - (self.nnz() as f64 / total as f64)
    }

    /// Shape (rows, cols).
    pub fn shape(&self) -> (usize, usize) {
        (self.rows, self.cols)
    }

    /// Sparse-dense matrix multiply: self (sparse) × dense.
    pub fn matmul_dense(&self, dense: &TensorValue) -> Result<TensorValue, TensorError> {
        let d_shape = dense.shape();
        if d_shape.len() != 2 || d_shape[0] != self.cols {
            return Err(TensorError::ShapeMismatch {
                expected: vec![self.cols, 0],
                got: d_shape.to_vec(),
            });
        }

        let d_cols = d_shape[1];
        let d_vals = dense.to_vec();
        let mut result = vec![0.0_f64; self.rows * d_cols];

        for r in 0..self.rows {
            for idx in self.row_ptr[r]..self.row_ptr[r + 1] {
                let c = self.col_indices[idx];
                let val = self.values[idx];
                for dc in 0..d_cols {
                    result[r * d_cols + dc] += val * d_vals[c * d_cols + dc];
                }
            }
        }

        TensorValue::from_data(result, &[self.rows, d_cols])
    }
}

// ═══════════════════════════════════════════════════════════════════════
// CSC Storage
// ═══════════════════════════════════════════════════════════════════════

/// Compressed Sparse Column format.
#[derive(Debug, Clone)]
pub struct CscMatrix {
    /// Non-zero values.
    values: Vec<f64>,
    /// Row indices for each non-zero value.
    row_indices: Vec<usize>,
    /// Column pointer array (length = cols + 1).
    col_ptr: Vec<usize>,
    /// Number of rows.
    rows: usize,
    /// Number of columns.
    cols: usize,
}

impl CscMatrix {
    /// Create CSC from a dense 2D tensor.
    pub fn from_dense(tensor: &TensorValue) -> Result<Self, TensorError> {
        let shape = tensor.shape();
        if shape.len() != 2 {
            return Err(TensorError::ShapeMismatch {
                expected: vec![0, 0],
                got: shape.to_vec(),
            });
        }

        let rows = shape[0];
        let cols = shape[1];
        let dense = tensor.to_vec();
        let mut values = Vec::new();
        let mut row_indices = Vec::new();
        let mut col_ptr = vec![0usize];

        for c in 0..cols {
            for r in 0..rows {
                let val = dense[r * cols + c];
                if val != 0.0 {
                    values.push(val);
                    row_indices.push(r);
                }
            }
            col_ptr.push(values.len());
        }

        Ok(Self {
            values,
            row_indices,
            col_ptr,
            rows,
            cols,
        })
    }

    /// Reconstruct dense 2D tensor.
    pub fn to_dense(&self) -> TensorValue {
        let mut dense = vec![0.0_f64; self.rows * self.cols];
        for c in 0..self.cols {
            for idx in self.col_ptr[c]..self.col_ptr[c + 1] {
                let r = self.row_indices[idx];
                dense[r * self.cols + c] = self.values[idx];
            }
        }
        TensorValue::from_data(dense, &[self.rows, self.cols])
            .unwrap_or_else(|_| TensorValue::zeros(&[self.rows, self.cols]))
    }

    /// Number of non-zeros.
    pub fn nnz(&self) -> usize {
        self.values.len()
    }

    /// Sparsity ratio.
    pub fn sparsity(&self) -> f64 {
        let total = self.rows * self.cols;
        if total == 0 {
            return 0.0;
        }
        1.0 - (self.nnz() as f64 / total as f64)
    }

    /// Shape (rows, cols).
    pub fn shape(&self) -> (usize, usize) {
        (self.rows, self.cols)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Pruning API
// ═══════════════════════════════════════════════════════════════════════

/// Pruning configuration.
#[derive(Debug, Clone)]
pub struct PruneConfig {
    /// Target sparsity ratio (0.0 to 1.0).
    pub target_sparsity: f64,
    /// Pruning pattern ("unstructured" or "4:2").
    pub pattern: String,
}

/// Apply magnitude-based pruning to a tensor.
///
/// Zeros out the smallest elements by magnitude to achieve the target sparsity.
pub fn prune_magnitude(tensor: &TensorValue, target_sparsity: f64) -> TensorValue {
    let mut values = tensor.to_vec();
    let n = values.len();
    let num_zeros = ((n as f64) * target_sparsity.clamp(0.0, 1.0)) as usize;

    if num_zeros == 0 || n == 0 {
        return tensor.clone();
    }

    // Find threshold: sort magnitudes, threshold = magnitude at num_zeros position
    let mut magnitudes: Vec<(usize, f64)> = values
        .iter()
        .enumerate()
        .map(|(i, &v)| (i, v.abs()))
        .collect();
    magnitudes.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

    for i in 0..num_zeros.min(n) {
        values[magnitudes[i].0] = 0.0;
    }

    TensorValue::from_data(values, tensor.shape()).unwrap_or_else(|_| tensor.clone())
}

/// Compute sparsity (fraction of zeros) of a tensor.
pub fn compute_sparsity(tensor: &TensorValue) -> f64 {
    let values = tensor.to_vec();
    if values.is_empty() {
        return 0.0;
    }
    let zeros = values.iter().filter(|&&v| v == 0.0).count();
    zeros as f64 / values.len() as f64
}

/// Check if a tensor satisfies 4:2 structured sparsity.
pub fn is_structured_sparse_4x2(tensor: &TensorValue) -> bool {
    let values = tensor.to_vec();
    if !values.len().is_multiple_of(4) {
        return false;
    }

    for chunk in values.chunks(4) {
        let zeros = chunk.iter().filter(|&&v| v == 0.0).count();
        if zeros != 2 {
            return false;
        }
    }
    true
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── S8.1: 4:2 Sparsity Pattern ────────────────────────────────────

    #[test]
    fn structured_4x2_basic() {
        let tensor =
            TensorValue::from_data(vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0], &[8]).unwrap();
        let sparse = Structured4x2Tensor::from_dense(&tensor);
        assert_eq!(sparse.nnz(), 4); // 2 per group × 2 groups
        assert!((sparse.sparsity() - 0.5).abs() < 1e-6);
    }

    #[test]
    fn structured_4x2_round_trip() {
        let tensor = TensorValue::from_data(vec![1.0, 0.5, 3.0, 0.1], &[4]).unwrap();
        let sparse = Structured4x2Tensor::from_dense(&tensor);
        let dense = sparse.to_dense();
        let vals = dense.to_vec();
        // Top 2 by magnitude: 3.0 and 1.0, others zeroed
        let nonzeros: Vec<f64> = vals.iter().filter(|&&v| v != 0.0).copied().collect();
        assert_eq!(nonzeros.len(), 2);
    }

    // ── S8.2: Sparse Metadata Format ──────────────────────────────────

    #[test]
    fn structured_4x2_metadata() {
        let tensor = TensorValue::from_data(vec![0.0, 10.0, 0.0, 20.0], &[4]).unwrap();
        let sparse = Structured4x2Tensor::from_dense(&tensor);
        assert_eq!(sparse.metadata.len(), 1); // 1 group of 4
    }

    // ── S8.3: CSR Storage ─────────────────────────────────────────────

    #[test]
    fn csr_basic() {
        let tensor =
            TensorValue::from_data(vec![1.0, 0.0, 0.0, 2.0, 0.0, 3.0, 0.0, 0.0, 4.0], &[3, 3])
                .unwrap();
        let csr = CsrMatrix::from_dense(&tensor).unwrap();
        assert_eq!(csr.nnz(), 4);
        assert!((csr.sparsity() - 5.0 / 9.0).abs() < 1e-6);
    }

    #[test]
    fn csr_round_trip() {
        let tensor = TensorValue::from_data(vec![1.0, 0.0, 2.0, 0.0, 3.0, 0.0], &[2, 3]).unwrap();
        let csr = CsrMatrix::from_dense(&tensor).unwrap();
        let dense = csr.to_dense();
        let orig = tensor.to_vec();
        let back = dense.to_vec();
        for (a, b) in orig.iter().zip(back.iter()) {
            assert!((a - b).abs() < 1e-10);
        }
    }

    // ── S8.4: CSC Storage ─────────────────────────────────────────────

    #[test]
    fn csc_basic() {
        let tensor =
            TensorValue::from_data(vec![1.0, 0.0, 0.0, 2.0, 0.0, 3.0, 0.0, 0.0, 4.0], &[3, 3])
                .unwrap();
        let csc = CscMatrix::from_dense(&tensor).unwrap();
        assert_eq!(csc.nnz(), 4);
    }

    #[test]
    fn csc_round_trip() {
        let tensor = TensorValue::from_data(vec![5.0, 0.0, 0.0, 6.0], &[2, 2]).unwrap();
        let csc = CscMatrix::from_dense(&tensor).unwrap();
        let dense = csc.to_dense();
        let orig = tensor.to_vec();
        let back = dense.to_vec();
        for (a, b) in orig.iter().zip(back.iter()) {
            assert!((a - b).abs() < 1e-10);
        }
    }

    // ── S8.5: Sparse-Dense MatMul ─────────────────────────────────────

    #[test]
    fn csr_matmul_dense() {
        // [[1, 0], [0, 2]] × [[3], [4]] = [[3], [8]]
        let sparse = TensorValue::from_data(vec![1.0, 0.0, 0.0, 2.0], &[2, 2]).unwrap();
        let dense = TensorValue::from_data(vec![3.0, 4.0], &[2, 1]).unwrap();
        let csr = CsrMatrix::from_dense(&sparse).unwrap();
        let result = csr.matmul_dense(&dense).unwrap();
        let vals = result.to_vec();
        assert!((vals[0] - 3.0).abs() < 1e-10);
        assert!((vals[1] - 8.0).abs() < 1e-10);
    }

    // ── S8.6: Pruning API ─────────────────────────────────────────────

    #[test]
    fn prune_magnitude_50_percent() {
        let tensor = TensorValue::from_data(vec![1.0, 2.0, 3.0, 4.0], &[4]).unwrap();
        let pruned = prune_magnitude(&tensor, 0.5);
        let vals = pruned.to_vec();
        let zeros = vals.iter().filter(|&&v| v == 0.0).count();
        assert_eq!(zeros, 2);
    }

    #[test]
    fn prune_magnitude_keeps_largest() {
        let tensor = TensorValue::from_data(vec![0.1, 10.0, 0.2, 20.0], &[4]).unwrap();
        let pruned = prune_magnitude(&tensor, 0.5);
        let vals = pruned.to_vec();
        // Should keep 10.0 and 20.0
        assert!(vals.contains(&10.0));
        assert!(vals.contains(&20.0));
    }

    // ── S8.7: Pruning Schedule (implicit via target_sparsity) ─────────

    #[test]
    fn prune_zero_sparsity_no_change() {
        let tensor = TensorValue::from_data(vec![1.0, 2.0, 3.0], &[3]).unwrap();
        let pruned = prune_magnitude(&tensor, 0.0);
        let orig = tensor.to_vec();
        let result = pruned.to_vec();
        for (a, b) in orig.iter().zip(result.iter()) {
            assert!((a - b).abs() < 1e-10);
        }
    }

    // ── S8.9: Sparsity Analysis ───────────────────────────────────────

    #[test]
    fn compute_sparsity_basic() {
        let tensor = TensorValue::from_data(vec![1.0, 0.0, 0.0, 2.0], &[4]).unwrap();
        let s = compute_sparsity(&tensor);
        assert!((s - 0.5).abs() < 1e-6);
    }

    #[test]
    fn is_structured_sparse_true() {
        let tensor =
            TensorValue::from_data(vec![1.0, 0.0, 0.0, 2.0, 3.0, 0.0, 0.0, 4.0], &[8]).unwrap();
        assert!(is_structured_sparse_4x2(&tensor));
    }

    #[test]
    fn is_structured_sparse_false() {
        let tensor = TensorValue::from_data(vec![1.0, 2.0, 3.0, 4.0], &[4]).unwrap();
        assert!(!is_structured_sparse_4x2(&tensor));
    }

    // ── S8.10: Edge Cases ─────────────────────────────────────────────

    #[test]
    fn csr_empty_matrix() {
        let tensor = TensorValue::from_data(vec![0.0; 4], &[2, 2]).unwrap();
        let csr = CsrMatrix::from_dense(&tensor).unwrap();
        assert_eq!(csr.nnz(), 0);
        assert!((csr.sparsity() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn sparse_format_display() {
        assert_eq!(format!("{}", SparseFormat::Structured4x2), "4:2");
        assert_eq!(format!("{}", SparseFormat::CSR), "CSR");
        assert_eq!(format!("{}", SparseFormat::CSC), "CSC");
    }

    #[test]
    fn structured_4x2_shape() {
        let tensor = TensorValue::from_data(vec![1.0; 8], &[2, 4]).unwrap();
        let sparse = Structured4x2Tensor::from_dense(&tensor);
        assert_eq!(sparse.shape(), &[2, 4]);
    }
}
