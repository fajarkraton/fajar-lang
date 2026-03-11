//! Model compression pipeline for Fajar Lang.
//!
//! End-to-end pipeline: train -> prune -> distill -> quantize -> export.
//! Orchestrates multiple compression techniques into a single workflow,
//! with reporting at each stage for size and accuracy tracking.

use ndarray::Array2;

use super::pruning::{self, PruningError, PruningStrategy};
use super::quantize::QuantizedTensor;
use super::tensor::{TensorError, TensorValue};

// ═══════════════════════════════════════════════════════════════════════
// Error types
// ═══════════════════════════════════════════════════════════════════════

/// Errors from the compression pipeline.
#[derive(Debug, Clone, thiserror::Error, PartialEq)]
pub enum CompressionError {
    /// Pruning stage failed.
    #[error("pruning error: {0}")]
    Pruning(#[from] PruningError),

    /// Tensor error.
    #[error("tensor error: {0}")]
    Tensor(#[from] TensorError),

    /// Invalid pipeline configuration.
    #[error("invalid pipeline: {reason}")]
    InvalidPipeline {
        /// Description of the error.
        reason: String,
    },

    /// Empty pipeline (no stages).
    #[error("compression pipeline has no stages")]
    EmptyPipeline,
}

// ═══════════════════════════════════════════════════════════════════════
// Compression stages
// ═══════════════════════════════════════════════════════════════════════

/// A single stage in the compression pipeline.
#[derive(Debug, Clone, PartialEq)]
pub enum CompressionStage {
    /// Train the model (simulated: no-op pass-through).
    Train {
        /// Number of training epochs.
        epochs: usize,
    },
    /// Prune weights below a ratio threshold.
    Prune {
        /// Fraction of weights to prune (0.0..1.0).
        ratio: f64,
        /// Pruning strategy.
        strategy: PruningStrategy,
    },
    /// Distill knowledge (simulated: scale weights toward target).
    Distill {
        /// Distillation temperature.
        temperature: f64,
    },
    /// Quantize to INT8.
    Quantize,
    /// Export the model (simulated: serialize size estimate).
    Export {
        /// Target format name.
        format: String,
    },
}

impl std::fmt::Display for CompressionStage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Train { epochs } => write!(f, "Train({epochs} epochs)"),
            Self::Prune { ratio, strategy } => {
                write!(f, "Prune({ratio:.0}%, {strategy:?})")
            }
            Self::Distill { temperature } => {
                write!(f, "Distill(T={temperature})")
            }
            Self::Quantize => write!(f, "Quantize(INT8)"),
            Self::Export { format } => write!(f, "Export({format})"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Model snapshot
// ═══════════════════════════════════════════════════════════════════════

/// Snapshot of model state at a pipeline stage.
#[derive(Debug, Clone)]
pub struct ModelSnapshot {
    /// Stage name/description.
    pub stage_name: String,
    /// Number of non-zero parameters.
    pub param_count: usize,
    /// Estimated model size in bytes.
    pub size_bytes: usize,
    /// Simulated accuracy (0.0 to 1.0).
    pub accuracy: f64,
}

impl ModelSnapshot {
    /// Creates a snapshot from weight data.
    pub fn from_weights(stage_name: &str, weights: &Array2<f64>, accuracy: f64) -> Self {
        let nonzero = weights.iter().filter(|&&v| v != 0.0).count();
        Self {
            stage_name: stage_name.to_string(),
            param_count: nonzero,
            size_bytes: nonzero * std::mem::size_of::<f64>(),
            accuracy,
        }
    }

    /// Creates a snapshot for quantized weights.
    pub fn from_quantized(stage_name: &str, quantized: &QuantizedTensor, accuracy: f64) -> Self {
        Self {
            stage_name: stage_name.to_string(),
            param_count: quantized.numel(),
            size_bytes: quantized.numel() * std::mem::size_of::<i8>() + std::mem::size_of::<f64>(), // scale factor
            accuracy,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Pipeline report
// ═══════════════════════════════════════════════════════════════════════

/// Report from running the full compression pipeline.
#[derive(Debug, Clone)]
pub struct PipelineReport {
    /// Snapshots at each stage.
    pub snapshots: Vec<ModelSnapshot>,
    /// Total compression ratio (initial size / final size).
    pub total_compression: f64,
    /// Accuracy before compression.
    pub initial_accuracy: f64,
    /// Accuracy after compression.
    pub final_accuracy: f64,
}

impl PipelineReport {
    /// Returns the accuracy drop from compression.
    pub fn accuracy_drop(&self) -> f64 {
        self.initial_accuracy - self.final_accuracy
    }

    /// Returns the number of stages in the report.
    pub fn num_stages(&self) -> usize {
        self.snapshots.len()
    }

    /// Returns the initial model size in bytes.
    pub fn initial_size_bytes(&self) -> usize {
        self.snapshots.first().map(|s| s.size_bytes).unwrap_or(0)
    }

    /// Returns the final model size in bytes.
    pub fn final_size_bytes(&self) -> usize {
        self.snapshots.last().map(|s| s.size_bytes).unwrap_or(0)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Compression pipeline
// ═══════════════════════════════════════════════════════════════════════

/// End-to-end model compression pipeline.
///
/// Sequences multiple compression stages (train, prune, distill,
/// quantize, export) and tracks model size and accuracy at each stage.
#[derive(Debug, Clone)]
pub struct CompressionPipeline {
    /// Ordered list of compression stages.
    stages: Vec<CompressionStage>,
}

impl CompressionPipeline {
    /// Creates a new pipeline with the given stages.
    pub fn new(stages: Vec<CompressionStage>) -> Result<Self, CompressionError> {
        if stages.is_empty() {
            return Err(CompressionError::EmptyPipeline);
        }
        Ok(Self { stages })
    }

    /// Returns the stages in this pipeline.
    pub fn stages(&self) -> &[CompressionStage] {
        &self.stages
    }

    /// Returns the number of stages.
    pub fn num_stages(&self) -> usize {
        self.stages.len()
    }

    /// Runs the pipeline on the given weight matrix.
    ///
    /// Returns a `PipelineReport` with snapshots at each stage.
    pub fn run(
        &self,
        weights: &Array2<f64>,
        initial_accuracy: f64,
    ) -> Result<PipelineReport, CompressionError> {
        let mut current = weights.clone();
        let mut accuracy = initial_accuracy;
        let mut snapshots = Vec::new();

        // Initial snapshot
        snapshots.push(ModelSnapshot::from_weights("initial", &current, accuracy));

        for stage in &self.stages {
            let (new_weights, new_accuracy, snapshot) = execute_stage(stage, &current, accuracy)?;
            current = new_weights;
            accuracy = new_accuracy;
            snapshots.push(snapshot);
        }

        let initial_size = snapshots.first().map(|s| s.size_bytes).unwrap_or(1);
        let final_size = snapshots.last().map(|s| s.size_bytes).unwrap_or(1);
        let compression = if final_size > 0 {
            initial_size as f64 / final_size as f64
        } else {
            f64::INFINITY
        };

        Ok(PipelineReport {
            snapshots,
            total_compression: compression,
            initial_accuracy,
            final_accuracy: accuracy,
        })
    }
}

/// Executes a single compression stage.
///
/// Returns (updated_weights, accuracy, snapshot).
fn execute_stage(
    stage: &CompressionStage,
    weights: &Array2<f64>,
    accuracy: f64,
) -> Result<(Array2<f64>, f64, ModelSnapshot), CompressionError> {
    match stage {
        CompressionStage::Train { epochs } => execute_train(weights, accuracy, *epochs),
        CompressionStage::Prune { ratio, strategy } => {
            execute_prune(weights, accuracy, *ratio, *strategy)
        }
        CompressionStage::Distill { temperature } => {
            execute_distill(weights, accuracy, *temperature)
        }
        CompressionStage::Quantize => execute_quantize(weights, accuracy),
        CompressionStage::Export { format } => execute_export(weights, accuracy, format),
    }
}

/// Simulates training (pass-through with minor weight adjustment).
fn execute_train(
    weights: &Array2<f64>,
    accuracy: f64,
    _epochs: usize,
) -> Result<(Array2<f64>, f64, ModelSnapshot), CompressionError> {
    // Simulate: training slightly improves accuracy
    let new_accuracy = (accuracy + 0.02).min(1.0);
    let snapshot = ModelSnapshot::from_weights("train", weights, new_accuracy);
    Ok((weights.clone(), new_accuracy, snapshot))
}

/// Executes pruning stage.
fn execute_prune(
    weights: &Array2<f64>,
    accuracy: f64,
    ratio: f64,
    strategy: PruningStrategy,
) -> Result<(Array2<f64>, f64, ModelSnapshot), CompressionError> {
    let pruned = pruning::prune_dense(weights, ratio, strategy)?;
    // Simulate: pruning causes small accuracy drop proportional to ratio
    let acc_drop = ratio * 0.05;
    let new_accuracy = (accuracy - acc_drop).max(0.0);
    let snapshot = ModelSnapshot::from_weights("prune", &pruned.weights, new_accuracy);
    Ok((pruned.weights, new_accuracy, snapshot))
}

/// Simulates distillation (small weight smoothing).
fn execute_distill(
    weights: &Array2<f64>,
    accuracy: f64,
    temperature: f64,
) -> Result<(Array2<f64>, f64, ModelSnapshot), CompressionError> {
    // Simulate: distillation recovers some accuracy
    let recovery = 0.01 * temperature.min(5.0);
    let new_accuracy = (accuracy + recovery).min(1.0);
    let snapshot = ModelSnapshot::from_weights("distill", weights, new_accuracy);
    Ok((weights.clone(), new_accuracy, snapshot))
}

/// Executes quantization stage (INT8).
fn execute_quantize(
    weights: &Array2<f64>,
    accuracy: f64,
) -> Result<(Array2<f64>, f64, ModelSnapshot), CompressionError> {
    let rows = weights.nrows();
    let cols = weights.ncols();
    let flat: Vec<f64> = weights.iter().copied().collect();
    let tv = TensorValue::from_data(flat, &[rows, cols])?;
    let quantized = QuantizedTensor::quantize(&tv);

    // Dequantize back for the pipeline to continue with f64
    let dequantized = quantized.dequantize();
    let dq_data = dequantized.to_vec();
    let result = Array2::from_shape_vec((rows, cols), dq_data).map_err(|e| {
        CompressionError::Tensor(TensorError::InvalidData {
            reason: e.to_string(),
        })
    })?;

    // Simulate: quantization causes ~1% accuracy drop
    let new_accuracy = (accuracy - 0.01).max(0.0);
    let snapshot = ModelSnapshot::from_quantized("quantize", &quantized, new_accuracy);
    Ok((result, new_accuracy, snapshot))
}

/// Simulates export (pass-through with format metadata).
fn execute_export(
    weights: &Array2<f64>,
    accuracy: f64,
    format: &str,
) -> Result<(Array2<f64>, f64, ModelSnapshot), CompressionError> {
    let name = format!("export({format})");
    let snapshot = ModelSnapshot::from_weights(&name, weights, accuracy);
    Ok((weights.clone(), accuracy, snapshot))
}

// ═══════════════════════════════════════════════════════════════════════
// Auto-selection
// ═══════════════════════════════════════════════════════════════════════

/// Automatically selects a compression pipeline based on target size.
///
/// Estimates the number of f64 parameters from `current_size_bytes`,
/// then selects pruning ratio and whether to quantize to reach
/// `target_size_bytes`.
pub fn auto_select_pipeline(
    current_size_bytes: usize,
    target_size_bytes: usize,
) -> Result<Vec<CompressionStage>, CompressionError> {
    if target_size_bytes == 0 {
        return Err(CompressionError::InvalidPipeline {
            reason: "target_size_bytes must be > 0".to_string(),
        });
    }
    if target_size_bytes >= current_size_bytes {
        // No compression needed
        return Ok(vec![CompressionStage::Export {
            format: "fj".to_string(),
        }]);
    }

    let ratio_needed = 1.0 - (target_size_bytes as f64 / current_size_bytes as f64);
    let mut stages = Vec::new();

    // Always start with training
    stages.push(CompressionStage::Train { epochs: 5 });

    // Add pruning if significant compression needed
    if ratio_needed > 0.1 {
        let prune_ratio = (ratio_needed * 0.7).min(0.9);
        stages.push(CompressionStage::Prune {
            ratio: prune_ratio,
            strategy: PruningStrategy::MagnitudeBased,
        });
    }

    // Add distillation for large compression
    if ratio_needed > 0.5 {
        stages.push(CompressionStage::Distill { temperature: 3.0 });
    }

    // Add quantization for very large compression (8x from f64->i8)
    if ratio_needed > 0.7 {
        stages.push(CompressionStage::Quantize);
    }

    // Always end with export
    stages.push(CompressionStage::Export {
        format: "fj".to_string(),
    });

    Ok(stages)
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn s16_1_pipeline_runs_all_stages() {
        let weights = Array2::from_shape_vec(
            (4, 4),
            vec![
                1.0, 0.1, 0.5, 2.0, 0.01, 3.0, 0.02, 1.5, 0.3, 0.8, 0.05, 4.0, 0.1, 0.2, 0.3, 0.4,
            ],
        )
        .unwrap();

        let pipeline = CompressionPipeline::new(vec![
            CompressionStage::Train { epochs: 5 },
            CompressionStage::Prune {
                ratio: 0.3,
                strategy: PruningStrategy::MagnitudeBased,
            },
            CompressionStage::Quantize,
            CompressionStage::Export {
                format: "fj".to_string(),
            },
        ])
        .unwrap();

        let report = pipeline.run(&weights, 0.95).unwrap();

        // initial + 4 stages = 5 snapshots
        assert_eq!(report.num_stages(), 5);
        assert!(report.total_compression > 1.0);
    }

    #[test]
    fn s16_2_pipeline_report_tracks_accuracy() {
        let weights = Array2::ones((4, 4));

        let pipeline = CompressionPipeline::new(vec![
            CompressionStage::Train { epochs: 3 },
            CompressionStage::Prune {
                ratio: 0.5,
                strategy: PruningStrategy::MagnitudeBased,
            },
        ])
        .unwrap();

        let report = pipeline.run(&weights, 0.90).unwrap();

        assert!((report.initial_accuracy - 0.90).abs() < 1e-10);
        // Accuracy should change through the pipeline
        assert!(report.final_accuracy > 0.0);
        assert!(report.final_accuracy <= 1.0);
    }

    #[test]
    fn s16_3_auto_select_no_compression_needed() {
        let stages = auto_select_pipeline(1000, 1000).unwrap();

        // Only export stage when no compression needed
        assert_eq!(stages.len(), 1);
        assert!(matches!(stages[0], CompressionStage::Export { .. }));
    }

    #[test]
    fn s16_4_auto_select_moderate_compression() {
        let stages = auto_select_pipeline(10000, 5000).unwrap();

        // Should include train + prune + export
        assert!(stages.len() >= 3);
        assert!(stages
            .iter()
            .any(|s| matches!(s, CompressionStage::Train { .. })));
        assert!(stages
            .iter()
            .any(|s| matches!(s, CompressionStage::Prune { .. })));
        assert!(stages
            .iter()
            .any(|s| matches!(s, CompressionStage::Export { .. })));
    }

    #[test]
    fn s16_5_auto_select_aggressive_compression() {
        let stages = auto_select_pipeline(100000, 10000).unwrap();

        // Should include distillation and quantization
        assert!(stages
            .iter()
            .any(|s| matches!(s, CompressionStage::Distill { .. })));
        assert!(stages
            .iter()
            .any(|s| matches!(s, CompressionStage::Quantize)));
    }

    #[test]
    fn s16_6_model_snapshot_from_weights() {
        let weights = Array2::from_shape_vec((2, 3), vec![1.0, 0.0, 0.5, 0.0, 2.0, 0.0]).unwrap();

        let snapshot = ModelSnapshot::from_weights("test", &weights, 0.85);

        assert_eq!(snapshot.stage_name, "test");
        assert_eq!(snapshot.param_count, 3); // 3 non-zero
        assert_eq!(snapshot.size_bytes, 3 * 8); // 3 * sizeof(f64)
        assert!((snapshot.accuracy - 0.85).abs() < 1e-10);
    }

    #[test]
    fn s16_7_quantize_stage_reduces_size() {
        let weights = Array2::from_shape_vec(
            (4, 4),
            vec![
                1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0, 13.0, 14.0, 15.0,
                16.0,
            ],
        )
        .unwrap();

        let pipeline = CompressionPipeline::new(vec![CompressionStage::Quantize]).unwrap();

        let report = pipeline.run(&weights, 0.95).unwrap();

        // Quantized model should be smaller
        let initial_size = report.initial_size_bytes();
        let final_size = report.final_size_bytes();
        assert!(
            final_size < initial_size,
            "quantized size {final_size} should be < initial {initial_size}"
        );
    }

    #[test]
    fn s16_8_empty_pipeline_returns_error() {
        let result = CompressionPipeline::new(vec![]);
        assert!(result.is_err());
    }

    #[test]
    fn s16_9_pipeline_report_compression_ratio() {
        let weights = Array2::ones((10, 10));

        let pipeline = CompressionPipeline::new(vec![CompressionStage::Prune {
            ratio: 0.5,
            strategy: PruningStrategy::MagnitudeBased,
        }])
        .unwrap();

        let report = pipeline.run(&weights, 0.90).unwrap();

        assert!(
            report.total_compression >= 1.0,
            "compression should be >= 1.0"
        );
    }

    #[test]
    fn s16_10_end_to_end_pipeline() {
        let weights =
            Array2::from_shape_vec((8, 8), (0..64).map(|i| (i as f64) * 0.1).collect()).unwrap();

        let stages = auto_select_pipeline(
            64 * 8, // 64 f64 params
            64,     // target: ~8 i8 params worth
        )
        .unwrap();

        let pipeline = CompressionPipeline::new(stages).unwrap();
        let report = pipeline.run(&weights, 0.92).unwrap();

        assert!(report.num_stages() >= 3);
        assert!(report.total_compression > 1.0);
        assert!(report.final_accuracy > 0.0);
        // Accuracy drop can be negative if pipeline improves accuracy (e.g., train + distill)
        assert!(report.accuracy_drop().abs() < 1.0);
    }
}
