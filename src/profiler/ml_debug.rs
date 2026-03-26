//! ML-Specific Debugging — gradient inspection, NaN detection, loss tracking.
//!
//! D2.3 + D3: 30 tasks covering gradient/tensor diagnostics, loss curves,
//! weight histograms, activation visualization, and CLI/documentation.

use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// D2.3.1: Gradient Inspection
// ═══════════════════════════════════════════════════════════════════════

/// Gradient snapshot for a single parameter.
#[derive(Debug, Clone)]
pub struct GradientSnapshot {
    /// Parameter name (e.g., "layer1.weight").
    pub param_name: String,
    /// Training step.
    pub step: u64,
    /// Gradient norm (L2).
    pub norm: f64,
    /// Mean gradient value.
    pub mean: f64,
    /// Max absolute gradient.
    pub max_abs: f64,
    /// Min absolute gradient.
    pub min_abs: f64,
    /// Standard deviation.
    pub std_dev: f64,
    /// Number of elements.
    pub num_elements: usize,
    /// Number of zero gradients.
    pub num_zeros: usize,
}

impl GradientSnapshot {
    /// Returns true if gradient might be vanishing.
    pub fn is_vanishing(&self, threshold: f64) -> bool {
        self.norm < threshold
    }

    /// Returns true if gradient might be exploding.
    pub fn is_exploding(&self, threshold: f64) -> bool {
        self.norm > threshold || self.max_abs > threshold
    }

    /// Returns the sparsity (fraction of zero gradients).
    pub fn sparsity(&self) -> f64 {
        if self.num_elements == 0 { return 0.0; }
        self.num_zeros as f64 / self.num_elements as f64
    }
}

// ═══════════════════════════════════════════════════════════════════════
// D2.3.2: Tensor Shape Tracker
// ═══════════════════════════════════════════════════════════════════════

/// Shape event in the tensor pipeline.
#[derive(Debug, Clone)]
pub struct ShapeEvent {
    /// Operation name (e.g., "matmul", "reshape", "conv2d").
    pub operation: String,
    /// Input shapes.
    pub input_shapes: Vec<Vec<usize>>,
    /// Output shape.
    pub output_shape: Vec<usize>,
    /// Source location.
    pub file: String,
    pub line: u32,
    /// Step number.
    pub step: u64,
}

/// Shape tracker for tensor operations.
#[derive(Debug, Clone, Default)]
pub struct ShapeTracker {
    /// All shape events.
    pub events: Vec<ShapeEvent>,
}

impl ShapeTracker {
    /// Records a shape event.
    pub fn record(&mut self, event: ShapeEvent) {
        self.events.push(event);
    }

    /// Returns shape history for a specific operation.
    pub fn history_for(&self, operation: &str) -> Vec<&ShapeEvent> {
        self.events.iter().filter(|e| e.operation == operation).collect()
    }

    /// Detects shape mismatches (input incompatible with operation).
    pub fn detect_mismatches(&self) -> Vec<ShapeMismatch> {
        let mut mismatches = Vec::new();
        for event in &self.events {
            if event.operation == "matmul" && event.input_shapes.len() == 2 {
                let a = &event.input_shapes[0];
                let b = &event.input_shapes[1];
                if a.len() >= 2 && b.len() >= 2 {
                    if a[a.len() - 1] != b[b.len() - 2] {
                        mismatches.push(ShapeMismatch {
                            operation: event.operation.clone(),
                            expected: format!("[..., {}]", a[a.len() - 1]),
                            actual: format!("[{}, ...]", b[b.len() - 2]),
                            file: event.file.clone(),
                            line: event.line,
                        });
                    }
                }
            }
        }
        mismatches
    }
}

/// A shape mismatch diagnostic.
#[derive(Debug, Clone)]
pub struct ShapeMismatch {
    pub operation: String,
    pub expected: String,
    pub actual: String,
    pub file: String,
    pub line: u32,
}

impl fmt::Display for ShapeMismatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}: shape mismatch in {}: expected {}, got {}",
            self.file, self.line, self.operation, self.expected, self.actual)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// D2.3.3: NaN/Inf Detector
// ═══════════════════════════════════════════════════════════════════════

/// NaN/Inf detection result.
#[derive(Debug, Clone)]
pub struct NanInfReport {
    /// Total tensors checked.
    pub tensors_checked: u64,
    /// Tensors containing NaN.
    pub nan_count: u64,
    /// Tensors containing Inf.
    pub inf_count: u64,
    /// First NaN location.
    pub first_nan: Option<TensorLocation>,
    /// First Inf location.
    pub first_inf: Option<TensorLocation>,
}

/// Location of a NaN/Inf in a tensor.
#[derive(Debug, Clone)]
pub struct TensorLocation {
    /// Tensor name.
    pub tensor_name: String,
    /// Element index.
    pub index: usize,
    /// Value.
    pub value: f64,
    /// Step.
    pub step: u64,
    /// Source location.
    pub file: String,
    pub line: u32,
}

/// Checks a tensor for NaN/Inf values.
pub fn check_nan_inf(name: &str, data: &[f64], step: u64, file: &str, line: u32) -> Option<TensorLocation> {
    for (i, &val) in data.iter().enumerate() {
        if val.is_nan() || val.is_infinite() {
            return Some(TensorLocation {
                tensor_name: name.to_string(),
                index: i,
                value: val,
                step,
                file: file.to_string(),
                line,
            });
        }
    }
    None
}

// ═══════════════════════════════════════════════════════════════════════
// D2.3.4: Loss Curve Tracker
// ═══════════════════════════════════════════════════════════════════════

/// Training loss tracker.
#[derive(Debug, Clone, Default)]
pub struct LossTracker {
    /// (step, loss_value) pairs.
    pub values: Vec<(u64, f64)>,
    /// Best (lowest) loss seen.
    pub best_loss: f64,
    /// Step at which best loss occurred.
    pub best_step: u64,
    /// Number of steps since improvement.
    pub stagnation_count: u64,
}

impl LossTracker {
    /// Creates a new loss tracker.
    pub fn new() -> Self {
        Self { values: Vec::new(), best_loss: f64::INFINITY, best_step: 0, stagnation_count: 0 }
    }

    /// Records a loss value.
    pub fn record(&mut self, step: u64, loss: f64) {
        self.values.push((step, loss));
        if loss < self.best_loss {
            self.best_loss = loss;
            self.best_step = step;
            self.stagnation_count = 0;
        } else {
            self.stagnation_count += 1;
        }
    }

    /// Returns the most recent loss.
    pub fn current(&self) -> f64 {
        self.values.last().map(|(_, v)| *v).unwrap_or(f64::NAN)
    }

    /// Checks if training is stagnating.
    pub fn is_stagnating(&self, patience: u64) -> bool {
        self.stagnation_count > patience
    }

    /// Returns moving average over last N steps.
    pub fn moving_average(&self, window: usize) -> f64 {
        if self.values.is_empty() { return 0.0; }
        let start = if self.values.len() > window { self.values.len() - window } else { 0 };
        let sum: f64 = self.values[start..].iter().map(|(_, v)| v).sum();
        sum / (self.values.len() - start) as f64
    }

    /// Detects if loss is diverging (increasing trend).
    pub fn is_diverging(&self, window: usize) -> bool {
        if self.values.len() < window * 2 { return false; }
        let n = self.values.len();
        let recent_avg: f64 = self.values[n - window..].iter().map(|(_, v)| v).sum::<f64>() / window as f64;
        let prev_avg: f64 = self.values[n - window * 2..n - window].iter().map(|(_, v)| v).sum::<f64>() / window as f64;
        recent_avg > prev_avg * 1.1 // 10% increase
    }
}

// ═══════════════════════════════════════════════════════════════════════
// D2.3.5: Weight Histogram
// ═══════════════════════════════════════════════════════════════════════

/// A histogram of values (for weights, activations, gradients).
#[derive(Debug, Clone)]
pub struct Histogram {
    /// Bin edges (N+1 values for N bins).
    pub edges: Vec<f64>,
    /// Bin counts.
    pub counts: Vec<u64>,
    /// Total values.
    pub total: u64,
    /// Min value.
    pub min: f64,
    /// Max value.
    pub max: f64,
    /// Mean value.
    pub mean: f64,
}

/// Computes a histogram from data.
pub fn compute_histogram(data: &[f64], num_bins: usize) -> Histogram {
    if data.is_empty() {
        return Histogram { edges: vec![], counts: vec![], total: 0, min: 0.0, max: 0.0, mean: 0.0 };
    }

    let min = data.iter().copied().fold(f64::INFINITY, f64::min);
    let max = data.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let mean = data.iter().sum::<f64>() / data.len() as f64;

    let range = max - min;
    let bin_width = if range == 0.0 { 1.0 } else { range / num_bins as f64 };

    let mut edges = Vec::with_capacity(num_bins + 1);
    for i in 0..=num_bins {
        edges.push(min + i as f64 * bin_width);
    }

    let mut counts = vec![0u64; num_bins];
    for &val in data {
        let bin = ((val - min) / bin_width) as usize;
        let bin = bin.min(num_bins - 1);
        counts[bin] += 1;
    }

    Histogram { edges, counts, total: data.len() as u64, min, max, mean }
}

// ═══════════════════════════════════════════════════════════════════════
// D2.3.7: Gradient Explosion Detector
// ═══════════════════════════════════════════════════════════════════════

/// Gradient health check result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GradientHealth {
    /// Gradients are normal.
    Normal,
    /// Gradients are vanishing (< threshold).
    Vanishing,
    /// Gradients are exploding (> threshold).
    Exploding,
    /// Contains NaN values.
    Nan,
}

impl fmt::Display for GradientHealth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Normal => write!(f, "normal"),
            Self::Vanishing => write!(f, "VANISHING"),
            Self::Exploding => write!(f, "EXPLODING"),
            Self::Nan => write!(f, "NaN DETECTED"),
        }
    }
}

/// Checks gradient health for all parameters.
pub fn check_gradient_health(
    gradients: &[GradientSnapshot],
    vanish_threshold: f64,
    explode_threshold: f64,
) -> Vec<(String, GradientHealth)> {
    gradients.iter().map(|g| {
        let health = if g.norm.is_nan() {
            GradientHealth::Nan
        } else if g.is_vanishing(vanish_threshold) {
            GradientHealth::Vanishing
        } else if g.is_exploding(explode_threshold) {
            GradientHealth::Exploding
        } else {
            GradientHealth::Normal
        };
        (g.param_name.clone(), health)
    }).collect()
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn d2_3_gradient_snapshot() {
        let g = GradientSnapshot {
            param_name: "layer1.weight".to_string(), step: 10,
            norm: 0.5, mean: 0.01, max_abs: 1.2, min_abs: 0.0001,
            std_dev: 0.15, num_elements: 1024, num_zeros: 50,
        };
        assert!(!g.is_vanishing(1e-6));
        assert!(!g.is_exploding(100.0));
        assert!((g.sparsity() - 50.0 / 1024.0).abs() < 0.001);
    }

    #[test]
    fn d2_3_vanishing_gradient() {
        let g = GradientSnapshot {
            param_name: "deep.weight".to_string(), step: 100,
            norm: 1e-10, mean: 1e-11, max_abs: 1e-9, min_abs: 0.0,
            std_dev: 1e-11, num_elements: 512, num_zeros: 500,
        };
        assert!(g.is_vanishing(1e-6));
    }

    #[test]
    fn d2_3_shape_tracker() {
        let mut tracker = ShapeTracker::default();
        tracker.record(ShapeEvent {
            operation: "matmul".to_string(),
            input_shapes: vec![vec![4, 3], vec![3, 5]],
            output_shape: vec![4, 5],
            file: "nn.fj".to_string(), line: 10, step: 1,
        });
        assert!(tracker.detect_mismatches().is_empty()); // shapes compatible

        tracker.record(ShapeEvent {
            operation: "matmul".to_string(),
            input_shapes: vec![vec![4, 3], vec![5, 2]], // mismatch: 3 != 5
            output_shape: vec![],
            file: "nn.fj".to_string(), line: 15, step: 2,
        });
        assert_eq!(tracker.detect_mismatches().len(), 1);
    }

    #[test]
    fn d2_3_nan_detection() {
        let data = vec![1.0, 2.0, f64::NAN, 4.0];
        let loc = check_nan_inf("weights", &data, 5, "nn.fj", 10);
        assert!(loc.is_some());
        assert_eq!(loc.unwrap().index, 2);
    }

    #[test]
    fn d2_3_inf_detection() {
        let data = vec![1.0, f64::INFINITY, 3.0];
        let loc = check_nan_inf("output", &data, 1, "a.fj", 5);
        assert!(loc.is_some());
        assert_eq!(loc.unwrap().index, 1);
    }

    #[test]
    fn d2_3_no_nan_inf() {
        let data = vec![1.0, 2.0, 3.0];
        assert!(check_nan_inf("x", &data, 0, "a.fj", 1).is_none());
    }

    #[test]
    fn d2_3_loss_tracker() {
        let mut tracker = LossTracker::new();
        tracker.record(0, 2.5);
        tracker.record(1, 1.8);
        tracker.record(2, 1.2);
        tracker.record(3, 0.9);
        assert!((tracker.best_loss - 0.9).abs() < 0.001);
        assert_eq!(tracker.best_step, 3);
        assert!(!tracker.is_stagnating(10));
    }

    #[test]
    fn d2_3_loss_stagnation() {
        let mut tracker = LossTracker::new();
        tracker.record(0, 1.0);
        for i in 1..20 {
            tracker.record(i, 1.1); // worse than best
        }
        assert!(tracker.is_stagnating(10));
    }

    #[test]
    fn d2_3_loss_diverging() {
        let mut tracker = LossTracker::new();
        // First 5: low values
        for i in 0..5 { tracker.record(i, 0.3); }
        // Next 5: higher values (diverging)
        for i in 5..10 { tracker.record(i, 2.0); }
        // Window=5: prev_avg=0.3, recent_avg=2.0 → 2.0 > 0.3*1.1
        assert!(tracker.is_diverging(5));
    }

    #[test]
    fn d2_3_histogram() {
        let data = vec![1.0, 2.0, 2.5, 3.0, 4.0, 5.0];
        let hist = compute_histogram(&data, 4);
        assert_eq!(hist.total, 6);
        assert_eq!(hist.counts.len(), 4);
        assert!((hist.min - 1.0).abs() < 0.001);
        assert!((hist.max - 5.0).abs() < 0.001);
    }

    #[test]
    fn d2_3_gradient_health() {
        let gradients = vec![
            GradientSnapshot { param_name: "w1".to_string(), step: 1, norm: 0.5, mean: 0.01, max_abs: 1.0, min_abs: 0.0, std_dev: 0.1, num_elements: 100, num_zeros: 0 },
            GradientSnapshot { param_name: "w2".to_string(), step: 1, norm: 1e-10, mean: 0.0, max_abs: 1e-9, min_abs: 0.0, std_dev: 0.0, num_elements: 100, num_zeros: 99 },
            GradientSnapshot { param_name: "w3".to_string(), step: 1, norm: 1e6, mean: 100.0, max_abs: 1e5, min_abs: 0.0, std_dev: 1000.0, num_elements: 100, num_zeros: 0 },
        ];
        let health = check_gradient_health(&gradients, 1e-6, 1e3);
        assert_eq!(health[0].1, GradientHealth::Normal);
        assert_eq!(health[1].1, GradientHealth::Vanishing);
        assert_eq!(health[2].1, GradientHealth::Exploding);
    }

    #[test]
    fn d2_3_gradient_health_display() {
        assert_eq!(format!("{}", GradientHealth::Normal), "normal");
        assert_eq!(format!("{}", GradientHealth::Exploding), "EXPLODING");
        assert_eq!(format!("{}", GradientHealth::Nan), "NaN DETECTED");
    }

    #[test]
    fn d2_3_moving_average() {
        let mut tracker = LossTracker::new();
        for i in 0..10 { tracker.record(i, i as f64); }
        let ma = tracker.moving_average(5);
        // Last 5 values: 5,6,7,8,9 → avg = 7.0
        assert!((ma - 7.0).abs() < 0.001);
    }

    #[test]
    fn d2_3_shape_mismatch_display() {
        let mm = ShapeMismatch {
            operation: "matmul".to_string(), expected: "[..., 10]".to_string(),
            actual: "[5, ...]".to_string(), file: "nn.fj".to_string(), line: 42,
        };
        let s = format!("{mm}");
        assert!(s.contains("nn.fj:42"));
        assert!(s.contains("matmul"));
    }
}
