//! AI-integrated kernel — ML-enhanced kernel subsystems.
//!
//! Sprint N4: Provides neural/ML-enhanced kernel components for
//! scheduling, anomaly detection, memory prediction, and power management.
//! All models are simple and simulated — no external ML frameworks.
//!
//! # Architecture
//!
//! ```text
//! InKernelTensorOps    — basic matmul/add in @kernel context
//! MlScheduler          — neural scheduler (linear model)
//! AnomalyDetector      — syscall pattern frequency analysis
//! PredictiveMemory     — exponential moving average allocation predictor
//! QnnBridge            — Qualcomm QNN inference bridge (simulated)
//! PowerManagementMl    — idle period prediction
//! NetworkTrafficMl     — congestion prediction
//! StoragePrefetchMl    — file access pattern prediction
//! SecurityMl           — intrusion detection via syscall sequences
//! AiKernelBenchmark    — ML-enhanced vs baseline comparison
//! ```

use std::collections::HashMap;
use thiserror::Error;

// ═══════════════════════════════════════════════════════════════════════
// Error types
// ═══════════════════════════════════════════════════════════════════════

/// Errors from AI kernel operations.
#[derive(Debug, Clone, Error, PartialEq)]
pub enum AiKernelError {
    /// Tensor dimension mismatch.
    #[error("tensor dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch {
        /// Expected dimension.
        expected: usize,
        /// Actual dimension.
        actual: usize,
    },

    /// Model not trained.
    #[error("model not trained: {name}")]
    ModelNotTrained {
        /// Model name.
        name: String,
    },

    /// QNN bridge error.
    #[error("QNN error: {reason}")]
    QnnError {
        /// Error description.
        reason: String,
    },

    /// Insufficient data for prediction.
    #[error("insufficient data: need {needed} samples, have {have}")]
    InsufficientData {
        /// Samples needed.
        needed: usize,
        /// Samples available.
        have: usize,
    },

    /// Anomaly detected.
    #[error("anomaly detected: {description}")]
    AnomalyDetected {
        /// Anomaly description.
        description: String,
    },
}

/// Result type for AI kernel operations.
pub type AiKernelResult<T> = Result<T, AiKernelError>;

// ═══════════════════════════════════════════════════════════════════════
// In-Kernel Tensor Ops
// ═══════════════════════════════════════════════════════════════════════

/// Fixed-size matrix for in-kernel tensor operations.
///
/// Uses stack-allocated arrays only — no heap allocation.
/// Suitable for @kernel context where heap is forbidden.
/// Maximum size: 128x128 (V27.5 P1.1: increased from 16 for Gemma 3 1B head_dim=256).
pub const MAX_KERNEL_TENSOR_DIM: usize = 128;

/// A small fixed-size matrix for kernel-space computation.
#[derive(Debug, Clone)]
pub struct KernelMatrix {
    /// Matrix data in row-major order.
    data: Vec<f32>,
    /// Number of rows.
    pub rows: usize,
    /// Number of columns.
    pub cols: usize,
}

impl KernelMatrix {
    /// Creates a new zero-filled kernel matrix.
    pub fn zeros(rows: usize, cols: usize) -> AiKernelResult<Self> {
        if rows > MAX_KERNEL_TENSOR_DIM || cols > MAX_KERNEL_TENSOR_DIM {
            return Err(AiKernelError::DimensionMismatch {
                expected: MAX_KERNEL_TENSOR_DIM,
                actual: rows.max(cols),
            });
        }
        Ok(Self {
            data: vec![0.0; rows * cols],
            rows,
            cols,
        })
    }

    /// Creates a kernel matrix from data.
    pub fn from_data(rows: usize, cols: usize, data: Vec<f32>) -> AiKernelResult<Self> {
        if rows > MAX_KERNEL_TENSOR_DIM || cols > MAX_KERNEL_TENSOR_DIM {
            return Err(AiKernelError::DimensionMismatch {
                expected: MAX_KERNEL_TENSOR_DIM,
                actual: rows.max(cols),
            });
        }
        if data.len() != rows * cols {
            return Err(AiKernelError::DimensionMismatch {
                expected: rows * cols,
                actual: data.len(),
            });
        }
        Ok(Self { data, rows, cols })
    }

    /// Gets a value at (row, col).
    pub fn get(&self, row: usize, col: usize) -> f32 {
        self.data[row * self.cols + col]
    }

    /// Sets a value at (row, col).
    pub fn set(&mut self, row: usize, col: usize, val: f32) {
        self.data[row * self.cols + col] = val;
    }

    /// Returns the data as a slice.
    pub fn data(&self) -> &[f32] {
        &self.data
    }
}

/// In-kernel tensor operations (small fixed-size matrices only).
pub struct InKernelTensorOps;

impl InKernelTensorOps {
    /// Matrix multiply: C = A * B.
    pub fn matmul(a: &KernelMatrix, b: &KernelMatrix) -> AiKernelResult<KernelMatrix> {
        if a.cols != b.rows {
            return Err(AiKernelError::DimensionMismatch {
                expected: a.cols,
                actual: b.rows,
            });
        }
        let mut result = KernelMatrix::zeros(a.rows, b.cols)?;
        for i in 0..a.rows {
            for j in 0..b.cols {
                let mut sum = 0.0f32;
                for k in 0..a.cols {
                    sum += a.get(i, k) * b.get(k, j);
                }
                result.set(i, j, sum);
            }
        }
        Ok(result)
    }

    /// Element-wise add: C = A + B.
    pub fn add(a: &KernelMatrix, b: &KernelMatrix) -> AiKernelResult<KernelMatrix> {
        if a.rows != b.rows || a.cols != b.cols {
            return Err(AiKernelError::DimensionMismatch {
                expected: a.rows * a.cols,
                actual: b.rows * b.cols,
            });
        }
        let data: Vec<f32> = a
            .data()
            .iter()
            .zip(b.data().iter())
            .map(|(x, y)| x + y)
            .collect();
        KernelMatrix::from_data(a.rows, a.cols, data)
    }

    /// ReLU activation (in-place style, returns new matrix).
    pub fn relu(a: &KernelMatrix) -> AiKernelResult<KernelMatrix> {
        let data: Vec<f32> = a
            .data()
            .iter()
            .map(|&x| if x > 0.0 { x } else { 0.0 })
            .collect();
        KernelMatrix::from_data(a.rows, a.cols, data)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// ML Scheduler
// ═══════════════════════════════════════════════════════════════════════

/// A simple linear model for predicting task duration.
///
/// Uses: predicted_duration = dot(weights, features) + bias
#[derive(Debug, Clone)]
pub struct MlScheduler {
    /// Weight vector (one per feature).
    weights: Vec<f32>,
    /// Bias term.
    bias: f32,
    /// Whether the model has been trained.
    trained: bool,
    /// Feature names for documentation.
    pub feature_names: Vec<String>,
}

impl MlScheduler {
    /// Creates a new ML scheduler with the given feature count.
    pub fn new(feature_names: Vec<String>) -> Self {
        let n = feature_names.len();
        Self {
            weights: vec![0.0; n],
            bias: 0.0,
            trained: false,
            feature_names,
        }
    }

    /// Trains the model with a simple least-squares fit on one sample.
    ///
    /// For simplicity, this does online gradient descent with a fixed
    /// learning rate. Call multiple times to train on more samples.
    pub fn train(
        &mut self,
        features: &[f32],
        actual_duration: f32,
        learning_rate: f32,
    ) -> AiKernelResult<()> {
        if features.len() != self.weights.len() {
            return Err(AiKernelError::DimensionMismatch {
                expected: self.weights.len(),
                actual: features.len(),
            });
        }
        let predicted: f32 = features
            .iter()
            .zip(self.weights.iter())
            .map(|(f, w)| f * w)
            .sum::<f32>()
            + self.bias;
        let error = predicted - actual_duration;
        for (w, &f) in self.weights.iter_mut().zip(features.iter()) {
            *w -= learning_rate * error * f;
        }
        self.bias -= learning_rate * error;
        self.trained = true;
        Ok(())
    }

    /// Predicts the duration for a task given its features.
    pub fn predict(&self, features: &[f32]) -> AiKernelResult<f32> {
        if !self.trained {
            return Err(AiKernelError::ModelNotTrained {
                name: "MlScheduler".to_string(),
            });
        }
        if features.len() != self.weights.len() {
            return Err(AiKernelError::DimensionMismatch {
                expected: self.weights.len(),
                actual: features.len(),
            });
        }
        let result: f32 = features
            .iter()
            .zip(self.weights.iter())
            .map(|(f, w)| f * w)
            .sum::<f32>()
            + self.bias;
        Ok(result)
    }

    /// Returns the feature count.
    pub fn feature_count(&self) -> usize {
        self.weights.len()
    }

    /// Returns whether the model has been trained.
    pub fn is_trained(&self) -> bool {
        self.trained
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Anomaly Detector
// ═══════════════════════════════════════════════════════════════════════

/// Detects unusual syscall patterns using frequency analysis.
///
/// Maintains a baseline frequency distribution and flags deviations
/// beyond a configurable threshold.
#[derive(Debug, Clone)]
pub struct AnomalyDetector {
    /// Baseline syscall frequencies: syscall_num -> count.
    baseline: HashMap<u64, u64>,
    /// Current window frequencies.
    current: HashMap<u64, u64>,
    /// Total baseline samples.
    baseline_total: u64,
    /// Total current samples.
    current_total: u64,
    /// Deviation threshold (as a multiplier of baseline frequency).
    threshold: f64,
    /// Whether baseline is established.
    baseline_set: bool,
}

impl AnomalyDetector {
    /// Creates a new anomaly detector with the given threshold.
    ///
    /// `threshold` is the maximum allowed ratio between current and baseline
    /// frequency for any syscall. A threshold of 3.0 means a syscall can
    /// appear up to 3x its baseline frequency before being flagged.
    pub fn new(threshold: f64) -> Self {
        Self {
            baseline: HashMap::new(),
            current: HashMap::new(),
            baseline_total: 0,
            current_total: 0,
            threshold,
            baseline_set: false,
        }
    }

    /// Records a syscall in the baseline window.
    pub fn record_baseline(&mut self, syscall_num: u64) {
        *self.baseline.entry(syscall_num).or_insert(0) += 1;
        self.baseline_total += 1;
    }

    /// Finalizes the baseline (call after recording all baseline data).
    pub fn finalize_baseline(&mut self) {
        self.baseline_set = true;
    }

    /// Records a syscall in the current monitoring window.
    pub fn record_current(&mut self, syscall_num: u64) {
        *self.current.entry(syscall_num).or_insert(0) += 1;
        self.current_total += 1;
    }

    /// Checks for anomalies by comparing current vs baseline frequencies.
    pub fn check(&self) -> Vec<AiKernelError> {
        if !self.baseline_set || self.baseline_total == 0 || self.current_total == 0 {
            return Vec::new();
        }

        let mut anomalies = Vec::new();
        for (&syscall_num, &current_count) in &self.current {
            let baseline_count = self.baseline.get(&syscall_num).copied().unwrap_or(0);
            let baseline_rate = if self.baseline_total > 0 {
                baseline_count as f64 / self.baseline_total as f64
            } else {
                0.0
            };
            let current_rate = current_count as f64 / self.current_total as f64;

            if baseline_rate > 0.0 && current_rate / baseline_rate > self.threshold {
                anomalies.push(AiKernelError::AnomalyDetected {
                    description: format!(
                        "syscall {} frequency {:.2}x above baseline",
                        syscall_num,
                        current_rate / baseline_rate
                    ),
                });
            } else if baseline_count == 0 && current_count > 0 {
                anomalies.push(AiKernelError::AnomalyDetected {
                    description: format!(
                        "syscall {} never seen in baseline ({} occurrences now)",
                        syscall_num, current_count
                    ),
                });
            }
        }
        anomalies
    }

    /// Resets the current monitoring window.
    pub fn reset_current(&mut self) {
        self.current.clear();
        self.current_total = 0;
    }

    /// Returns the number of unique syscalls in the baseline.
    pub fn baseline_syscall_count(&self) -> usize {
        self.baseline.len()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Predictive Memory
// ═══════════════════════════════════════════════════════════════════════

/// Predicts memory allocation needs using exponential moving average.
///
/// Tracks allocation sizes over time and predicts the next allocation
/// size, allowing pre-allocation to reduce latency.
#[derive(Debug, Clone)]
pub struct PredictiveMemory {
    /// Exponential moving average of allocation sizes.
    ema: f64,
    /// Smoothing factor (0 < alpha <= 1). Higher = more weight on recent.
    alpha: f64,
    /// Number of samples recorded.
    sample_count: u64,
    /// Last raw allocation size.
    last_size: u64,
    /// Pre-allocated pool size.
    pool_size: u64,
}

impl PredictiveMemory {
    /// Creates a new predictive memory with the given smoothing factor.
    pub fn new(alpha: f64) -> Self {
        Self {
            ema: 0.0,
            alpha,
            sample_count: 0,
            last_size: 0,
            pool_size: 0,
        }
    }

    /// Records an allocation of the given size.
    pub fn record_allocation(&mut self, size: u64) {
        if self.sample_count == 0 {
            self.ema = size as f64;
        } else {
            self.ema = self.alpha * size as f64 + (1.0 - self.alpha) * self.ema;
        }
        self.last_size = size;
        self.sample_count += 1;
    }

    /// Predicts the next allocation size.
    pub fn predict_next(&self) -> AiKernelResult<u64> {
        if self.sample_count < 2 {
            return Err(AiKernelError::InsufficientData {
                needed: 2,
                have: self.sample_count as usize,
            });
        }
        // Round up to next page boundary (4096).
        let predicted = self.ema.ceil() as u64;
        let aligned = predicted.div_ceil(4096) * 4096;
        Ok(aligned)
    }

    /// Pre-allocates a pool based on prediction.
    pub fn pre_allocate(&mut self) -> AiKernelResult<u64> {
        let predicted = self.predict_next()?;
        self.pool_size = predicted;
        Ok(predicted)
    }

    /// Returns the current EMA value.
    pub fn current_ema(&self) -> f64 {
        self.ema
    }

    /// Returns the current pool size.
    pub fn pool_size(&self) -> u64 {
        self.pool_size
    }

    /// Returns the number of samples.
    pub fn sample_count(&self) -> u64 {
        self.sample_count
    }
}

// ═══════════════════════════════════════════════════════════════════════
// QNN Bridge
// ═══════════════════════════════════════════════════════════════════════

/// Simulated Qualcomm QNN inference context.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QnnBackend {
    /// CPU inference (always available).
    Cpu,
    /// GPU inference (simulated).
    Gpu,
    /// HTP/DSP inference (simulated Hexagon).
    Htp,
}

/// A loaded QNN model.
#[derive(Debug, Clone)]
pub struct QnnModel {
    /// Model name.
    pub name: String,
    /// Backend.
    pub backend: QnnBackend,
    /// Input dimensions.
    pub input_dim: Vec<usize>,
    /// Output dimensions.
    pub output_dim: Vec<usize>,
    /// Whether the model is loaded.
    pub loaded: bool,
}

/// Simulated Qualcomm QNN inference bridge.
///
/// In production on QCS6490 hardware, this would use the real QNN SDK.
/// Here we simulate the API for correctness testing.
#[derive(Debug, Clone)]
pub struct QnnBridge {
    /// Loaded models by name.
    models: HashMap<String, QnnModel>,
    /// Active backend.
    backend: QnnBackend,
    /// Inference count.
    inference_count: u64,
}

impl QnnBridge {
    /// Creates a new QNN bridge with the given backend.
    pub fn new(backend: QnnBackend) -> Self {
        Self {
            models: HashMap::new(),
            backend,
            inference_count: 0,
        }
    }

    /// Loads a model.
    pub fn load_model(
        &mut self,
        name: &str,
        input_dim: Vec<usize>,
        output_dim: Vec<usize>,
    ) -> AiKernelResult<()> {
        self.models.insert(
            name.to_string(),
            QnnModel {
                name: name.to_string(),
                backend: self.backend,
                input_dim,
                output_dim,
                loaded: true,
            },
        );
        Ok(())
    }

    /// Runs inference on a loaded model (simulated — returns zeros).
    pub fn infer(&mut self, name: &str, input: &[f32]) -> AiKernelResult<Vec<f32>> {
        let model = self.models.get(name).ok_or(AiKernelError::QnnError {
            reason: format!("model '{}' not loaded", name),
        })?;
        let expected_input: usize = model.input_dim.iter().product();
        if input.len() != expected_input {
            return Err(AiKernelError::DimensionMismatch {
                expected: expected_input,
                actual: input.len(),
            });
        }
        let output_size: usize = model.output_dim.iter().product();
        self.inference_count += 1;
        // Simulated output: all zeros (real QNN would compute).
        Ok(vec![0.0; output_size])
    }

    /// Returns the number of inferences performed.
    pub fn inference_count(&self) -> u64 {
        self.inference_count
    }

    /// Returns the number of loaded models.
    pub fn model_count(&self) -> usize {
        self.models.len()
    }

    /// Returns the active backend.
    pub fn backend(&self) -> QnnBackend {
        self.backend
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Power Management ML
// ═══════════════════════════════════════════════════════════════════════

/// Power state for the system.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerState {
    /// Full performance.
    Active,
    /// Reduced frequency.
    LowPower,
    /// Deep sleep.
    DeepSleep,
    /// Hibernate.
    Hibernate,
}

/// Predicts idle periods to optimize power states.
///
/// Uses a sliding window average of active/idle durations.
#[derive(Debug, Clone)]
pub struct PowerManagementMl {
    /// Recent idle durations (sliding window).
    idle_history: Vec<u64>,
    /// Recent active durations (sliding window).
    active_history: Vec<u64>,
    /// Maximum history size.
    window_size: usize,
    /// Current recommended power state.
    current_state: PowerState,
}

impl PowerManagementMl {
    /// Creates a new power management predictor.
    pub fn new(window_size: usize) -> Self {
        Self {
            idle_history: Vec::new(),
            active_history: Vec::new(),
            window_size,
            current_state: PowerState::Active,
        }
    }

    /// Records an idle period duration (in microseconds).
    pub fn record_idle(&mut self, duration_us: u64) {
        if self.idle_history.len() >= self.window_size {
            self.idle_history.remove(0);
        }
        self.idle_history.push(duration_us);
    }

    /// Records an active period duration (in microseconds).
    pub fn record_active(&mut self, duration_us: u64) {
        if self.active_history.len() >= self.window_size {
            self.active_history.remove(0);
        }
        self.active_history.push(duration_us);
    }

    /// Predicts the next idle duration and recommends a power state.
    pub fn predict_state(&mut self) -> PowerState {
        if self.idle_history.is_empty() {
            self.current_state = PowerState::Active;
            return self.current_state;
        }
        let avg_idle: u64 = self.idle_history.iter().sum::<u64>() / self.idle_history.len() as u64;

        self.current_state = if avg_idle > 1_000_000 {
            // > 1 second average idle.
            PowerState::DeepSleep
        } else if avg_idle > 100_000 {
            // > 100ms average idle.
            PowerState::LowPower
        } else {
            PowerState::Active
        };
        self.current_state
    }

    /// Returns the current recommended power state.
    pub fn current_state(&self) -> PowerState {
        self.current_state
    }

    /// Returns the number of idle samples.
    pub fn idle_sample_count(&self) -> usize {
        self.idle_history.len()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Network Traffic ML
// ═══════════════════════════════════════════════════════════════════════

/// Predicts network congestion and adjusts buffer sizes.
#[derive(Debug, Clone)]
pub struct NetworkTrafficMl {
    /// Recent packet rates (packets per second).
    rate_history: Vec<u64>,
    /// Window size for averaging.
    window_size: usize,
    /// Current recommended buffer size in bytes.
    recommended_buffer: u64,
    /// Base buffer size.
    base_buffer: u64,
}

impl NetworkTrafficMl {
    /// Creates a new network traffic predictor.
    pub fn new(window_size: usize, base_buffer: u64) -> Self {
        Self {
            rate_history: Vec::new(),
            window_size,
            recommended_buffer: base_buffer,
            base_buffer,
        }
    }

    /// Records a packet rate measurement.
    pub fn record_rate(&mut self, packets_per_sec: u64) {
        if self.rate_history.len() >= self.window_size {
            self.rate_history.remove(0);
        }
        self.rate_history.push(packets_per_sec);
    }

    /// Predicts congestion and returns recommended buffer size.
    pub fn predict_buffer_size(&mut self) -> u64 {
        if self.rate_history.is_empty() {
            return self.base_buffer;
        }
        let avg_rate: u64 = self.rate_history.iter().sum::<u64>() / self.rate_history.len() as u64;

        // Scale buffer proportionally to traffic rate.
        self.recommended_buffer = if avg_rate > 10000 {
            self.base_buffer * 4 // High traffic: 4x buffer
        } else if avg_rate > 1000 {
            self.base_buffer * 2 // Medium traffic: 2x buffer
        } else {
            self.base_buffer // Low traffic: base buffer
        };
        self.recommended_buffer
    }

    /// Returns the current recommended buffer size.
    pub fn current_buffer(&self) -> u64 {
        self.recommended_buffer
    }

    /// Returns the rate sample count.
    pub fn sample_count(&self) -> usize {
        self.rate_history.len()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Storage Prefetch ML
// ═══════════════════════════════════════════════════════════════════════

/// Predicts file access patterns and prefetches blocks.
#[derive(Debug, Clone)]
pub struct StoragePrefetchMl {
    /// Recent block access sequence.
    access_history: Vec<u64>,
    /// Prefetch queue: blocks predicted to be accessed next.
    prefetch_queue: Vec<u64>,
    /// Maximum history length.
    max_history: usize,
    /// Prefetch lookahead (how many blocks ahead to prefetch).
    lookahead: u64,
}

impl StoragePrefetchMl {
    /// Creates a new storage prefetch predictor.
    pub fn new(max_history: usize, lookahead: u64) -> Self {
        Self {
            access_history: Vec::new(),
            prefetch_queue: Vec::new(),
            max_history,
            lookahead,
        }
    }

    /// Records a block access.
    pub fn record_access(&mut self, block: u64) {
        if self.access_history.len() >= self.max_history {
            self.access_history.remove(0);
        }
        self.access_history.push(block);
    }

    /// Predicts next blocks to prefetch based on sequential pattern detection.
    pub fn predict_prefetch(&mut self) -> &[u64] {
        self.prefetch_queue.clear();
        if self.access_history.len() < 2 {
            return &self.prefetch_queue;
        }

        // Detect sequential access pattern.
        let last = self.access_history[self.access_history.len() - 1];
        let prev = self.access_history[self.access_history.len() - 2];

        if last > prev {
            let stride = last - prev;
            // Prefetch ahead by stride.
            for i in 1..=self.lookahead {
                self.prefetch_queue.push(last + stride * i);
            }
        }

        &self.prefetch_queue
    }

    /// Returns the prefetch queue.
    pub fn prefetch_queue(&self) -> &[u64] {
        &self.prefetch_queue
    }

    /// Returns the number of recorded accesses.
    pub fn access_count(&self) -> usize {
        self.access_history.len()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Security ML
// ═══════════════════════════════════════════════════════════════════════

/// Detects intrusion via syscall sequence analysis.
///
/// Maintains a set of known-good syscall sequences (n-grams) and
/// flags unknown sequences as potential intrusions.
#[derive(Debug, Clone)]
pub struct SecurityMl {
    /// Known-good syscall n-grams (sequence -> count).
    known_sequences: HashMap<Vec<u64>, u64>,
    /// Recent syscall sequence.
    recent: Vec<u64>,
    /// N-gram size.
    ngram_size: usize,
    /// Whether training is complete.
    trained: bool,
    /// Alert log.
    alerts: Vec<String>,
}

impl SecurityMl {
    /// Creates a new security ML detector with the given n-gram size.
    pub fn new(ngram_size: usize) -> Self {
        Self {
            known_sequences: HashMap::new(),
            recent: Vec::new(),
            ngram_size,
            trained: false,
            alerts: Vec::new(),
        }
    }

    /// Records a syscall during training phase.
    pub fn train_syscall(&mut self, syscall_num: u64) {
        self.recent.push(syscall_num);
        if self.recent.len() >= self.ngram_size {
            let start = self.recent.len() - self.ngram_size;
            let ngram = self.recent[start..].to_vec();
            *self.known_sequences.entry(ngram).or_insert(0) += 1;
        }
    }

    /// Finalizes training.
    pub fn finalize_training(&mut self) {
        self.trained = true;
        self.recent.clear();
    }

    /// Monitors a syscall during detection phase.
    pub fn monitor_syscall(&mut self, syscall_num: u64) -> bool {
        self.recent.push(syscall_num);
        if self.recent.len() < self.ngram_size || !self.trained {
            return false; // No alert.
        }
        let start = self.recent.len() - self.ngram_size;
        let ngram = self.recent[start..].to_vec();
        if !self.known_sequences.contains_key(&ngram) {
            let alert = format!("unknown syscall sequence: {:?}", ngram);
            self.alerts.push(alert);
            return true; // Alert!
        }
        false
    }

    /// Returns the alert log.
    pub fn alerts(&self) -> &[String] {
        &self.alerts
    }

    /// Returns the number of known n-grams.
    pub fn known_sequence_count(&self) -> usize {
        self.known_sequences.len()
    }

    /// Returns whether training is complete.
    pub fn is_trained(&self) -> bool {
        self.trained
    }
}

// ═══════════════════════════════════════════════════════════════════════
// AI Kernel Benchmark
// ═══════════════════════════════════════════════════════════════════════

/// A benchmark comparison between ML-enhanced and baseline performance.
#[derive(Debug, Clone)]
pub struct BenchComparison {
    /// Metric name.
    pub metric: String,
    /// Baseline value (without ML).
    pub baseline: f64,
    /// ML-enhanced value.
    pub ml_enhanced: f64,
    /// Improvement ratio (ml / baseline). > 1.0 means ML is better.
    pub improvement: f64,
}

/// Benchmarks comparing ML-enhanced vs baseline kernel performance.
#[derive(Debug, Clone)]
pub struct AiKernelBenchmark {
    /// Benchmark results.
    results: Vec<BenchComparison>,
}

impl AiKernelBenchmark {
    /// Creates a new benchmark suite.
    pub fn new() -> Self {
        Self {
            results: Vec::new(),
        }
    }

    /// Records a benchmark comparison.
    pub fn record(&mut self, metric: &str, baseline: f64, ml_enhanced: f64) {
        let improvement = if baseline > 0.0 {
            baseline / ml_enhanced
        } else {
            1.0
        };
        self.results.push(BenchComparison {
            metric: metric.to_string(),
            baseline,
            ml_enhanced,
            improvement,
        });
    }

    /// Returns all results.
    pub fn results(&self) -> &[BenchComparison] {
        &self.results
    }

    /// Returns the average improvement ratio across all benchmarks.
    pub fn average_improvement(&self) -> f64 {
        if self.results.is_empty() {
            return 1.0;
        }
        let total: f64 = self.results.iter().map(|r| r.improvement).sum();
        total / self.results.len() as f64
    }

    /// Returns the number of recorded benchmarks.
    pub fn count(&self) -> usize {
        self.results.len()
    }

    /// Generates a summary report.
    pub fn summary(&self) -> String {
        let mut s = String::from("=== AI Kernel Benchmarks ===\n");
        for r in &self.results {
            s.push_str(&format!(
                "  {}: baseline={:.1}, ml={:.1}, improvement={:.2}x\n",
                r.metric, r.baseline, r.ml_enhanced, r.improvement
            ));
        }
        s.push_str(&format!(
            "  Average improvement: {:.2}x\n",
            self.average_improvement()
        ));
        s
    }
}

impl Default for AiKernelBenchmark {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── InKernelTensorOps ──

    #[test]
    fn kernel_tensor_matmul_2x2() {
        let a = KernelMatrix::from_data(2, 2, vec![1.0, 2.0, 3.0, 4.0]).unwrap();
        let b = KernelMatrix::from_data(2, 2, vec![5.0, 6.0, 7.0, 8.0]).unwrap();
        let c = InKernelTensorOps::matmul(&a, &b).unwrap();
        assert_eq!(c.get(0, 0), 19.0); // 1*5 + 2*7
        assert_eq!(c.get(0, 1), 22.0); // 1*6 + 2*8
        assert_eq!(c.get(1, 0), 43.0); // 3*5 + 4*7
        assert_eq!(c.get(1, 1), 50.0); // 3*6 + 4*8
    }

    #[test]
    fn kernel_tensor_add() {
        let a = KernelMatrix::from_data(1, 3, vec![1.0, 2.0, 3.0]).unwrap();
        let b = KernelMatrix::from_data(1, 3, vec![4.0, 5.0, 6.0]).unwrap();
        let c = InKernelTensorOps::add(&a, &b).unwrap();
        assert_eq!(c.data(), &[5.0, 7.0, 9.0]);
    }

    #[test]
    fn kernel_tensor_relu() {
        let a = KernelMatrix::from_data(1, 4, vec![-1.0, 0.0, 2.0, -3.0]).unwrap();
        let r = InKernelTensorOps::relu(&a).unwrap();
        assert_eq!(r.data(), &[0.0, 0.0, 2.0, 0.0]);
    }

    #[test]
    fn kernel_tensor_dimension_mismatch() {
        let a = KernelMatrix::from_data(2, 3, vec![0.0; 6]).unwrap();
        let b = KernelMatrix::from_data(2, 2, vec![0.0; 4]).unwrap();
        assert!(InKernelTensorOps::matmul(&a, &b).is_err());
    }

    #[test]
    fn kernel_tensor_exceeds_max_dim() {
        // V27.5 P1.1: max increased 16→128, so 20x20 now passes
        assert!(KernelMatrix::zeros(20, 20).is_ok());
        assert!(KernelMatrix::zeros(128, 128).is_ok());
        assert!(KernelMatrix::zeros(129, 129).is_err());
    }

    // ── MlScheduler ──

    #[test]
    fn ml_scheduler_train_and_predict() {
        let mut sched = MlScheduler::new(vec!["cpu_time".into(), "memory".into()]);
        // Train with some samples.
        for _ in 0..10 {
            assert!(sched.train(&[1.0, 2.0], 5.0, 0.01).is_ok());
        }
        assert!(sched.is_trained());
        let pred = sched.predict(&[1.0, 2.0]).unwrap();
        // Should converge toward 5.0.
        assert!(pred > 0.0);
    }

    #[test]
    fn ml_scheduler_untrained_fails() {
        let sched = MlScheduler::new(vec!["x".into()]);
        assert!(sched.predict(&[1.0]).is_err());
    }

    // ── AnomalyDetector ──

    #[test]
    fn anomaly_detector_normal_traffic() {
        let mut det = AnomalyDetector::new(3.0);
        for _ in 0..100 {
            det.record_baseline(1);
            det.record_baseline(2);
        }
        det.finalize_baseline();
        for _ in 0..50 {
            det.record_current(1);
            det.record_current(2);
        }
        assert!(det.check().is_empty());
    }

    #[test]
    fn anomaly_detector_unusual_syscall() {
        let mut det = AnomalyDetector::new(3.0);
        for _ in 0..100 {
            det.record_baseline(1);
        }
        det.finalize_baseline();
        // Syscall 99 never seen in baseline.
        for _ in 0..10 {
            det.record_current(99);
        }
        let anomalies = det.check();
        assert!(!anomalies.is_empty());
    }

    // ── PredictiveMemory ──

    #[test]
    fn predictive_memory_ema() {
        let mut pred = PredictiveMemory::new(0.3);
        pred.record_allocation(4096);
        pred.record_allocation(4096);
        pred.record_allocation(4096);
        let next = pred.predict_next().unwrap();
        assert_eq!(next, 4096); // Stable pattern.
    }

    #[test]
    fn predictive_memory_insufficient_data() {
        let pred = PredictiveMemory::new(0.3);
        assert!(pred.predict_next().is_err());
    }

    // ── QnnBridge ──

    #[test]
    fn qnn_bridge_load_and_infer() {
        let mut qnn = QnnBridge::new(QnnBackend::Cpu);
        assert!(qnn.load_model("classifier", vec![4], vec![2]).is_ok());
        let output = qnn.infer("classifier", &[1.0, 2.0, 3.0, 4.0]).unwrap();
        assert_eq!(output.len(), 2);
        assert_eq!(qnn.inference_count(), 1);
    }

    #[test]
    fn qnn_bridge_wrong_input_size() {
        let mut qnn = QnnBridge::new(QnnBackend::Gpu);
        assert!(qnn.load_model("net", vec![3], vec![1]).is_ok());
        assert!(qnn.infer("net", &[1.0, 2.0]).is_err());
    }

    #[test]
    fn qnn_bridge_model_not_found() {
        let mut qnn = QnnBridge::new(QnnBackend::Htp);
        assert!(qnn.infer("nonexistent", &[1.0]).is_err());
    }

    // ── PowerManagementMl ──

    #[test]
    fn power_ml_active_when_busy() {
        let mut power = PowerManagementMl::new(5);
        for _ in 0..5 {
            power.record_idle(1000); // 1ms idle — very busy.
        }
        assert_eq!(power.predict_state(), PowerState::Active);
    }

    #[test]
    fn power_ml_deep_sleep_when_idle() {
        let mut power = PowerManagementMl::new(5);
        for _ in 0..5 {
            power.record_idle(5_000_000); // 5 seconds idle.
        }
        assert_eq!(power.predict_state(), PowerState::DeepSleep);
    }

    // ── NetworkTrafficMl ──

    #[test]
    fn network_ml_scales_buffer() {
        let mut net = NetworkTrafficMl::new(5, 4096);
        for _ in 0..5 {
            net.record_rate(20000);
        }
        let buf = net.predict_buffer_size();
        assert_eq!(buf, 4096 * 4); // High traffic.
    }

    // ── StoragePrefetchMl ──

    #[test]
    fn storage_prefetch_sequential() {
        let mut prefetch = StoragePrefetchMl::new(100, 4);
        prefetch.record_access(10);
        prefetch.record_access(11);
        let blocks = prefetch.predict_prefetch().to_vec();
        assert_eq!(blocks, vec![12, 13, 14, 15]);
    }

    #[test]
    fn storage_prefetch_no_pattern() {
        let mut prefetch = StoragePrefetchMl::new(100, 4);
        prefetch.record_access(100);
        let blocks = prefetch.predict_prefetch();
        assert!(blocks.is_empty()); // Not enough data.
    }

    // ── SecurityMl ──

    #[test]
    fn security_ml_known_sequence_ok() {
        let mut sec = SecurityMl::new(3);
        // Training: sequence [1, 2, 3] is normal.
        sec.train_syscall(1);
        sec.train_syscall(2);
        sec.train_syscall(3);
        sec.finalize_training();

        // Monitoring: same sequence.
        assert!(!sec.monitor_syscall(1));
        assert!(!sec.monitor_syscall(2));
        assert!(!sec.monitor_syscall(3));
        assert!(sec.alerts().is_empty());
    }

    #[test]
    fn security_ml_unknown_sequence_alert() {
        let mut sec = SecurityMl::new(2);
        sec.train_syscall(1);
        sec.train_syscall(2);
        sec.train_syscall(1);
        sec.finalize_training();

        // Unknown sequence [3, 4].
        sec.monitor_syscall(3);
        let alerted = sec.monitor_syscall(4);
        assert!(alerted);
        assert!(!sec.alerts().is_empty());
    }

    // ── AiKernelBenchmark ──

    #[test]
    fn benchmark_comparison() {
        let mut bench = AiKernelBenchmark::new();
        bench.record("context_switch", 1000.0, 800.0); // ML is 1.25x better
        bench.record("syscall_latency", 500.0, 250.0); // ML is 2.0x better
        assert_eq!(bench.count(), 2);
        let avg = bench.average_improvement();
        assert!(avg > 1.0);
        let summary = bench.summary();
        assert!(summary.contains("context_switch"));
        assert!(summary.contains("syscall_latency"));
    }
}
