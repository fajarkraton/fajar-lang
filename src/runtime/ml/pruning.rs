//! Model pruning for Fajar Lang.
//!
//! Provides structured pruning to reduce model size by removing
//! unimportant channels, filters, and weights. Supports magnitude-based,
//! gradient-based, and random pruning strategies with configurable
//! schedules for gradual sparsity increases.

use ndarray::Array2;

use super::tensor::{TensorError, TensorValue};

// ═══════════════════════════════════════════════════════════════════════
// Error types
// ═══════════════════════════════════════════════════════════════════════

/// Errors from pruning operations.
#[derive(Debug, Clone, thiserror::Error, PartialEq)]
pub enum PruningError {
    /// Invalid pruning ratio (must be in 0.0..1.0).
    #[error("invalid pruning ratio {ratio}: must be in [0.0, 1.0)")]
    InvalidRatio {
        /// The invalid ratio.
        ratio: f64,
    },

    /// Empty weight tensor cannot be pruned.
    #[error("cannot prune empty weight tensor")]
    EmptyWeights,

    /// Schedule has invalid parameters.
    #[error("invalid schedule: {reason}")]
    InvalidSchedule {
        /// Description of what went wrong.
        reason: String,
    },

    /// Underlying tensor error.
    #[error("tensor error: {0}")]
    Tensor(#[from] TensorError),
}

// ═══════════════════════════════════════════════════════════════════════
// Pruning strategy
// ═══════════════════════════════════════════════════════════════════════

/// Strategy for selecting which weights to prune.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PruningStrategy {
    /// Prune weights with smallest absolute magnitude.
    MagnitudeBased,
    /// Prune weights with smallest gradient magnitude.
    GradientBased,
    /// Prune weights randomly (uniform selection).
    RandomBased,
}

// ═══════════════════════════════════════════════════════════════════════
// Pruning mask
// ═══════════════════════════════════════════════════════════════════════

/// Binary mask tracking which weights are kept (true) or pruned (false).
#[derive(Debug, Clone, PartialEq)]
pub struct PruningMask {
    /// Mask data: true = keep, false = pruned.
    mask: Vec<bool>,
    /// Shape of the mask (matches weight shape).
    shape: [usize; 2],
}

impl PruningMask {
    /// Creates a new mask with all weights kept.
    pub fn all_kept(rows: usize, cols: usize) -> Self {
        Self {
            mask: vec![true; rows * cols],
            shape: [rows, cols],
        }
    }

    /// Returns the mask data as a slice.
    pub fn data(&self) -> &[bool] {
        &self.mask
    }

    /// Returns the shape of the mask.
    pub fn shape(&self) -> [usize; 2] {
        self.shape
    }

    /// Returns the number of kept (non-pruned) weights.
    pub fn kept_count(&self) -> usize {
        self.mask.iter().filter(|&&v| v).count()
    }

    /// Returns the number of pruned weights.
    pub fn pruned_count(&self) -> usize {
        self.mask.iter().filter(|&&v| !v).count()
    }

    /// Returns the sparsity ratio (fraction of pruned weights).
    pub fn sparsity(&self) -> f64 {
        let total = self.mask.len();
        if total == 0 {
            return 0.0;
        }
        self.pruned_count() as f64 / total as f64
    }

    /// Applies the mask to a weight matrix, zeroing pruned entries.
    pub fn apply(&self, weights: &Array2<f64>) -> Array2<f64> {
        let mut result = weights.clone();
        for (i, &keep) in self.mask.iter().enumerate() {
            if !keep {
                let row = i / self.shape[1];
                let col = i % self.shape[1];
                result[[row, col]] = 0.0;
            }
        }
        result
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Pruned layer
// ═══════════════════════════════════════════════════════════════════════

/// Result of pruning a layer: masked weights plus metadata.
#[derive(Debug, Clone)]
pub struct PrunedLayer {
    /// Weights after pruning (zeroed entries for pruned weights).
    pub weights: Array2<f64>,
    /// Binary mask indicating which weights are kept.
    pub mask: PruningMask,
    /// Original parameter count before pruning.
    pub original_size: usize,
    /// Remaining non-zero parameter count after pruning.
    pub pruned_size: usize,
}

impl PrunedLayer {
    /// Returns the compression ratio (original / pruned).
    pub fn compression_ratio(&self) -> f64 {
        if self.pruned_size == 0 {
            return f64::INFINITY;
        }
        self.original_size as f64 / self.pruned_size as f64
    }

    /// Returns the sparsity ratio (fraction of zeroed weights).
    pub fn sparsity(&self) -> f64 {
        self.mask.sparsity()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Pruning configuration
// ═══════════════════════════════════════════════════════════════════════

/// Configuration for a pruning operation.
#[derive(Debug, Clone)]
pub struct PruningConfig {
    /// Target pruning ratio (fraction of weights to remove).
    pub target_ratio: f64,
    /// Strategy for selecting weights to prune.
    pub strategy: PruningStrategy,
    /// Optional schedule for gradual pruning.
    pub schedule: Option<PruningSchedule>,
}

impl PruningConfig {
    /// Creates a new pruning configuration.
    pub fn new(target_ratio: f64, strategy: PruningStrategy) -> Result<Self, PruningError> {
        validate_ratio(target_ratio)?;
        Ok(Self {
            target_ratio,
            strategy,
            schedule: None,
        })
    }

    /// Creates a configuration with a gradual pruning schedule.
    pub fn with_schedule(
        target_ratio: f64,
        strategy: PruningStrategy,
        schedule: PruningSchedule,
    ) -> Result<Self, PruningError> {
        validate_ratio(target_ratio)?;
        Ok(Self {
            target_ratio,
            strategy,
            schedule: Some(schedule),
        })
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Pruning schedule
// ═══════════════════════════════════════════════════════════════════════

/// Schedule for gradual pruning over multiple epochs.
///
/// Linearly interpolates the pruning ratio from `start_ratio` to
/// `target_ratio` over `num_epochs` epochs.
#[derive(Debug, Clone)]
pub struct PruningSchedule {
    /// Initial pruning ratio (start of training).
    pub start_ratio: f64,
    /// Final pruning ratio (end of schedule).
    pub target_ratio: f64,
    /// Number of epochs over which to ramp up pruning.
    pub num_epochs: usize,
}

impl PruningSchedule {
    /// Creates a new pruning schedule.
    pub fn new(
        start_ratio: f64,
        target_ratio: f64,
        num_epochs: usize,
    ) -> Result<Self, PruningError> {
        validate_ratio(start_ratio)?;
        validate_ratio(target_ratio)?;
        if num_epochs == 0 {
            return Err(PruningError::InvalidSchedule {
                reason: "num_epochs must be > 0".to_string(),
            });
        }
        if start_ratio > target_ratio {
            return Err(PruningError::InvalidSchedule {
                reason: format!(
                    "start_ratio ({start_ratio}) must be <= target_ratio ({target_ratio})"
                ),
            });
        }
        Ok(Self {
            start_ratio,
            target_ratio,
            num_epochs,
        })
    }

    /// Returns the pruning ratio at a given epoch (linear interpolation).
    pub fn ratio_at_epoch(&self, epoch: usize) -> f64 {
        if epoch >= self.num_epochs {
            return self.target_ratio;
        }
        let progress = epoch as f64 / self.num_epochs as f64;
        self.start_ratio + (self.target_ratio - self.start_ratio) * progress
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Channel importance
// ═══════════════════════════════════════════════════════════════════════

/// Computes per-channel (column) importance scores via L1 norm.
///
/// For a weight matrix of shape `[in, out]`, returns a vector of length
/// `out` where each entry is the sum of absolute values in that column.
pub fn channel_importance_l1(weights: &Array2<f64>) -> Vec<f64> {
    let cols = weights.ncols();
    let mut importance = vec![0.0; cols];
    for (col, imp) in importance.iter_mut().enumerate() {
        *imp = weights.column(col).iter().map(|v| v.abs()).sum();
    }
    importance
}

/// Computes per-channel (column) importance scores via L2 norm.
///
/// For a weight matrix of shape `[in, out]`, returns a vector of length
/// `out` where each entry is the Euclidean norm of that column.
pub fn channel_importance_l2(weights: &Array2<f64>) -> Vec<f64> {
    let cols = weights.ncols();
    let mut importance = vec![0.0; cols];
    for (col, imp) in importance.iter_mut().enumerate() {
        let sum_sq: f64 = weights.column(col).iter().map(|v| v * v).sum();
        *imp = sum_sq.sqrt();
    }
    importance
}

// ═══════════════════════════════════════════════════════════════════════
// Core pruning functions
// ═══════════════════════════════════════════════════════════════════════

/// Validates that a pruning ratio is in [0.0, 1.0).
fn validate_ratio(ratio: f64) -> Result<(), PruningError> {
    if !(0.0..1.0).contains(&ratio) {
        return Err(PruningError::InvalidRatio { ratio });
    }
    Ok(())
}

/// Computes a magnitude-based pruning mask for a weight matrix.
///
/// Selects the `ratio` fraction of weights with smallest absolute value
/// and marks them for pruning.
fn magnitude_mask(weights: &Array2<f64>, ratio: f64) -> PruningMask {
    let rows = weights.nrows();
    let cols = weights.ncols();
    let total = rows * cols;
    let num_prune = (total as f64 * ratio).round() as usize;

    // Collect (index, abs_value) pairs and sort by magnitude
    let mut indexed: Vec<(usize, f64)> = weights
        .iter()
        .enumerate()
        .map(|(i, &v)| (i, v.abs()))
        .collect();
    indexed.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

    let mut mask = vec![true; total];
    for &(idx, _) in indexed.iter().take(num_prune) {
        mask[idx] = false;
    }

    PruningMask {
        mask,
        shape: [rows, cols],
    }
}

/// Computes a gradient-based pruning mask.
///
/// Uses gradient magnitudes to identify least important weights.
/// Weights with the smallest gradient magnitudes are pruned.
fn gradient_mask(weights: &Array2<f64>, gradients: &Array2<f64>, ratio: f64) -> PruningMask {
    let rows = weights.nrows();
    let cols = weights.ncols();
    let total = rows * cols;
    let num_prune = (total as f64 * ratio).round() as usize;

    // Sort by gradient magnitude (importance = |weight| * |gradient|)
    let mut indexed: Vec<(usize, f64)> = weights
        .iter()
        .zip(gradients.iter())
        .enumerate()
        .map(|(i, (&w, &g))| (i, w.abs() * g.abs()))
        .collect();
    indexed.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

    let mut mask = vec![true; total];
    for &(idx, _) in indexed.iter().take(num_prune) {
        mask[idx] = false;
    }

    PruningMask {
        mask,
        shape: [rows, cols],
    }
}

/// Computes a random pruning mask.
///
/// Selects `ratio` fraction of weights uniformly at random to prune.
/// Uses a deterministic seed for reproducibility.
fn random_mask(rows: usize, cols: usize, ratio: f64, seed: u64) -> PruningMask {
    let total = rows * cols;
    let num_prune = (total as f64 * ratio).round() as usize;

    // Simple deterministic PRNG (xorshift64) for reproducibility
    let mut state = seed.wrapping_add(1);
    let mut indices: Vec<usize> = (0..total).collect();
    // Fisher-Yates shuffle with xorshift
    for i in (1..total).rev() {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        let j = (state as usize) % (i + 1);
        indices.swap(i, j);
    }

    let mut mask = vec![true; total];
    for &idx in indices.iter().take(num_prune) {
        mask[idx] = false;
    }

    PruningMask {
        mask,
        shape: [rows, cols],
    }
}

/// Prunes a dense (fully-connected) layer's weight matrix.
///
/// Removes the specified fraction of weights according to the given
/// strategy. Returns a `PrunedLayer` with zeroed pruned entries and
/// the corresponding binary mask.
pub fn prune_dense(
    weights: &Array2<f64>,
    ratio: f64,
    strategy: PruningStrategy,
) -> Result<PrunedLayer, PruningError> {
    validate_ratio(ratio)?;
    let rows = weights.nrows();
    let cols = weights.ncols();
    if rows == 0 || cols == 0 {
        return Err(PruningError::EmptyWeights);
    }

    let mask = match strategy {
        PruningStrategy::MagnitudeBased => magnitude_mask(weights, ratio),
        PruningStrategy::GradientBased => {
            // Without gradients, fall back to magnitude
            magnitude_mask(weights, ratio)
        }
        PruningStrategy::RandomBased => random_mask(rows, cols, ratio, 42),
    };

    let pruned_weights = mask.apply(weights);
    let kept = mask.kept_count();

    Ok(PrunedLayer {
        weights: pruned_weights,
        mask,
        original_size: rows * cols,
        pruned_size: kept,
    })
}

/// Prunes a dense layer with gradient information.
///
/// Uses the gradient magnitudes (combined with weight magnitudes) to
/// determine which weights are least important.
pub fn prune_dense_with_gradients(
    weights: &Array2<f64>,
    gradients: &Array2<f64>,
    ratio: f64,
) -> Result<PrunedLayer, PruningError> {
    validate_ratio(ratio)?;
    let rows = weights.nrows();
    let cols = weights.ncols();
    if rows == 0 || cols == 0 {
        return Err(PruningError::EmptyWeights);
    }

    let mask = gradient_mask(weights, gradients, ratio);
    let pruned_weights = mask.apply(weights);
    let kept = mask.kept_count();

    Ok(PrunedLayer {
        weights: pruned_weights,
        mask,
        original_size: rows * cols,
        pruned_size: kept,
    })
}

/// Prunes a Conv2d layer's weight matrix (simulated as 2D).
///
/// In the simulation, Conv2d weights are flattened to a 2D matrix
/// where rows represent output filters. Channel pruning removes
/// entire rows (filters) based on their L1 norm importance.
pub fn prune_conv2d(weights: &Array2<f64>, ratio: f64) -> Result<PrunedLayer, PruningError> {
    validate_ratio(ratio)?;
    let rows = weights.nrows();
    let cols = weights.ncols();
    if rows == 0 || cols == 0 {
        return Err(PruningError::EmptyWeights);
    }

    let num_prune = (rows as f64 * ratio).round() as usize;

    // Compute per-filter (row) importance via L1 norm
    let mut filter_importance: Vec<(usize, f64)> = (0..rows)
        .map(|r| {
            let norm: f64 = weights.row(r).iter().map(|v| v.abs()).sum();
            (r, norm)
        })
        .collect();
    filter_importance.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

    // Build mask: prune entire rows for least important filters
    let mut mask_data = vec![true; rows * cols];
    for &(row_idx, _) in filter_importance.iter().take(num_prune) {
        for col in 0..cols {
            mask_data[row_idx * cols + col] = false;
        }
    }

    let mask = PruningMask {
        mask: mask_data,
        shape: [rows, cols],
    };
    let pruned_weights = mask.apply(weights);
    let kept = mask.kept_count();

    Ok(PrunedLayer {
        weights: pruned_weights,
        mask,
        original_size: rows * cols,
        pruned_size: kept,
    })
}

// ═══════════════════════════════════════════════════════════════════════
// Fine-tuning simulation
// ═══════════════════════════════════════════════════════════════════════

/// Simulates fine-tuning after pruning.
///
/// Performs `epochs` steps of simulated gradient updates, respecting
/// the pruning mask (pruned weights remain zero). Returns the
/// fine-tuned weights.
pub fn fine_tune_after_pruning(
    pruned: &PrunedLayer,
    learning_rate: f64,
    epochs: usize,
) -> Array2<f64> {
    let mut weights = pruned.weights.clone();
    let [rows, cols] = pruned.mask.shape();

    for _ in 0..epochs {
        // Simulate gradient as small perturbation
        for r in 0..rows {
            for c in 0..cols {
                let idx = r * cols + c;
                if pruned.mask.data()[idx] {
                    // Small simulated gradient update
                    let grad = weights[[r, c]] * 0.01;
                    weights[[r, c]] -= learning_rate * grad;
                }
            }
        }
    }

    weights
}

// ═══════════════════════════════════════════════════════════════════════
// Pruning report
// ═══════════════════════════════════════════════════════════════════════

/// Summary report of a pruning operation.
#[derive(Debug, Clone)]
pub struct PruningReport {
    /// Original number of parameters.
    pub original_params: usize,
    /// Number of remaining (non-zero) parameters.
    pub pruned_params: usize,
    /// Fraction of parameters removed.
    pub sparsity: f64,
    /// Compression ratio (original / remaining).
    pub compression_ratio: f64,
    /// Estimated memory savings in bytes (assuming f64).
    pub memory_saved_bytes: usize,
}

/// Generates a pruning report from original and pruned parameter counts.
pub fn pruning_report(original_params: usize, pruned_params: usize) -> PruningReport {
    let sparsity = if original_params == 0 {
        0.0
    } else {
        1.0 - (pruned_params as f64 / original_params as f64)
    };

    let compression_ratio = if pruned_params == 0 {
        f64::INFINITY
    } else {
        original_params as f64 / pruned_params as f64
    };

    let removed = original_params.saturating_sub(pruned_params);
    let memory_saved_bytes = removed * std::mem::size_of::<f64>();

    PruningReport {
        original_params,
        pruned_params,
        sparsity,
        compression_ratio,
        memory_saved_bytes,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// TensorValue convenience wrappers
// ═══════════════════════════════════════════════════════════════════════

/// Prunes a TensorValue (must be 2D) using magnitude-based pruning.
///
/// Returns the pruned TensorValue and metadata.
pub fn prune_tensor(
    tensor: &TensorValue,
    ratio: f64,
    strategy: PruningStrategy,
) -> Result<(TensorValue, PrunedLayer), PruningError> {
    let shape = tensor.shape();
    if shape.len() != 2 {
        return Err(PruningError::Tensor(TensorError::RankMismatch {
            expected: 2,
            got: shape.len(),
        }));
    }

    let rows = shape[0];
    let cols = shape[1];
    let data = tensor.to_vec();
    let array = Array2::from_shape_vec((rows, cols), data).map_err(|e| {
        PruningError::Tensor(TensorError::InvalidData {
            reason: e.to_string(),
        })
    })?;

    let pruned = prune_dense(&array, ratio, strategy)?;
    let flat: Vec<f64> = pruned.weights.iter().copied().collect();
    let tv = TensorValue::from_data(flat, &[rows, cols])?;

    Ok((tv, pruned))
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn s13_1_magnitude_pruning_zeros_smallest_weights() {
        let weights = Array2::from_shape_vec(
            (3, 4),
            vec![
                1.0, 0.1, 0.5, 2.0, 0.01, 3.0, 0.02, 1.5, 0.3, 0.8, 0.05, 4.0,
            ],
        )
        .unwrap();

        let result = prune_dense(&weights, 0.5, PruningStrategy::MagnitudeBased).unwrap();

        // 12 weights, prune 50% = 6 pruned, 6 kept
        assert_eq!(result.original_size, 12);
        assert_eq!(result.pruned_size, 6);
        assert!((result.sparsity() - 0.5).abs() < 1e-10);

        // Verify pruned entries are zero
        let mut zero_count = 0;
        for &v in result.weights.iter() {
            if v == 0.0 {
                zero_count += 1;
            }
        }
        assert_eq!(zero_count, 6);

        // The largest weights should be preserved
        assert!(result.weights[[1, 1]].abs() > 0.0); // 3.0 kept
        assert!(result.weights[[2, 3]].abs() > 0.0); // 4.0 kept
    }

    #[test]
    fn s13_2_channel_importance_l1_computes_column_norms() {
        let weights = Array2::from_shape_vec((3, 2), vec![1.0, -2.0, 3.0, 0.5, -0.5, 1.0]).unwrap();

        let importance = channel_importance_l1(&weights);
        assert_eq!(importance.len(), 2);
        // Column 0: |1.0| + |3.0| + |-0.5| = 4.5
        assert!((importance[0] - 4.5).abs() < 1e-10);
        // Column 1: |-2.0| + |0.5| + |1.0| = 3.5
        assert!((importance[1] - 3.5).abs() < 1e-10);
    }

    #[test]
    fn s13_3_pruning_schedule_linear_interpolation() {
        let schedule = PruningSchedule::new(0.1, 0.9, 10).unwrap();

        assert!((schedule.ratio_at_epoch(0) - 0.1).abs() < 1e-10);
        assert!((schedule.ratio_at_epoch(5) - 0.5).abs() < 1e-10);
        assert!((schedule.ratio_at_epoch(10) - 0.9).abs() < 1e-10);
        // Beyond schedule, clamp to target
        assert!((schedule.ratio_at_epoch(20) - 0.9).abs() < 1e-10);
    }

    #[test]
    fn s13_4_fine_tune_preserves_mask() {
        let weights = Array2::from_shape_vec((2, 3), vec![1.0, 0.01, 0.5, 0.02, 2.0, 0.1]).unwrap();

        let pruned = prune_dense(&weights, 0.5, PruningStrategy::MagnitudeBased).unwrap();
        let fine_tuned = fine_tune_after_pruning(&pruned, 0.01, 5);

        // Pruned weights must remain zero
        for (i, &keep) in pruned.mask.data().iter().enumerate() {
            let r = i / 3;
            let c = i % 3;
            if !keep {
                assert_eq!(
                    fine_tuned[[r, c]],
                    0.0,
                    "pruned weight at [{r},{c}] should remain zero"
                );
            }
        }
    }

    #[test]
    fn s13_5_pruning_report_calculates_metrics() {
        let report = pruning_report(1000, 400);

        assert_eq!(report.original_params, 1000);
        assert_eq!(report.pruned_params, 400);
        assert!((report.sparsity - 0.6).abs() < 1e-10);
        assert!((report.compression_ratio - 2.5).abs() < 1e-10);
        assert_eq!(report.memory_saved_bytes, 600 * 8); // 600 f64s
    }

    #[test]
    fn s13_6_pruning_mask_apply_zeros_entries() {
        let weights = Array2::from_shape_vec((2, 2), vec![1.0, 2.0, 3.0, 4.0]).unwrap();

        let mask = PruningMask {
            mask: vec![true, false, false, true],
            shape: [2, 2],
        };

        let result = mask.apply(&weights);
        assert_eq!(result[[0, 0]], 1.0);
        assert_eq!(result[[0, 1]], 0.0);
        assert_eq!(result[[1, 0]], 0.0);
        assert_eq!(result[[1, 1]], 4.0);
    }

    #[test]
    fn s13_7_conv2d_pruning_removes_entire_filters() {
        let weights = Array2::from_shape_vec(
            (4, 3),
            vec![
                0.1, 0.1, 0.1, // filter 0: L1 = 0.3
                1.0, 1.0, 1.0, // filter 1: L1 = 3.0
                0.2, 0.2, 0.2, // filter 2: L1 = 0.6
                2.0, 2.0, 2.0, // filter 3: L1 = 6.0
            ],
        )
        .unwrap();

        let result = prune_conv2d(&weights, 0.5).unwrap();

        // Should prune 2 out of 4 filters (least important)
        // Filters 0 and 2 have lowest L1 norms
        assert_eq!(result.weights[[0, 0]], 0.0); // filter 0 pruned
        assert_eq!(result.weights[[0, 1]], 0.0);
        assert_eq!(result.weights[[0, 2]], 0.0);
        assert!(result.weights[[1, 0]] > 0.0); // filter 1 kept
        assert_eq!(result.weights[[2, 0]], 0.0); // filter 2 pruned
        assert!(result.weights[[3, 0]] > 0.0); // filter 3 kept
    }

    #[test]
    fn s13_8_random_pruning_achieves_target_sparsity() {
        let weights = Array2::ones((10, 10));

        let result = prune_dense(&weights, 0.3, PruningStrategy::RandomBased).unwrap();

        // Should prune ~30% of 100 weights = 30
        let zero_count = result.weights.iter().filter(|&&v| v == 0.0).count();
        assert_eq!(zero_count, 30);
        assert_eq!(result.pruned_size, 70);
    }

    #[test]
    fn s13_9_invalid_ratio_returns_error() {
        let weights = Array2::ones((3, 3));

        assert!(prune_dense(&weights, -0.1, PruningStrategy::MagnitudeBased).is_err());
        assert!(prune_dense(&weights, 1.0, PruningStrategy::MagnitudeBased).is_err());
        assert!(prune_dense(&weights, 1.5, PruningStrategy::MagnitudeBased).is_err());
        assert!(prune_dense(&weights, 0.0, PruningStrategy::MagnitudeBased).is_ok());
    }

    #[test]
    fn s13_10_prune_tensor_convenience_wrapper() {
        let tv = TensorValue::from_data(vec![1.0, 0.01, 0.5, 2.0, 0.02, 3.0, 0.1, 1.5], &[2, 4])
            .unwrap();

        let (pruned_tv, metadata) =
            prune_tensor(&tv, 0.5, PruningStrategy::MagnitudeBased).unwrap();

        assert_eq!(pruned_tv.shape(), &[2, 4]);
        assert_eq!(metadata.original_size, 8);
        assert_eq!(metadata.pruned_size, 4);

        // Pruned tensor should have 4 zeros
        let zeros = pruned_tv.to_vec().iter().filter(|&&v| v == 0.0).count();
        assert_eq!(zeros, 4);
    }
}
