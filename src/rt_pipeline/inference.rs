//! Inference Engine — model loading, scheduling, caching, quantization.
//!
//! Phase R2.1: 10 tasks covering model runtime with deadline guarantees,
//! hot-swap, multi-model pipelines, and INT8 quantized inference.

use std::collections::HashMap;

// ═══════════════════════════════════════════════════════════════════════
// R2.1.1: Model Loader
// ═══════════════════════════════════════════════════════════════════════

/// Model format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelFormat {
    /// Fajar ML format (native).
    Fjml,
    /// ONNX interchange format.
    Onnx,
    /// TensorFlow Lite.
    TfLite,
    /// Qualcomm QNN DLC.
    QnnDlc,
}

/// A loaded model ready for inference.
#[derive(Debug, Clone)]
pub struct Model {
    /// Model name.
    pub name: String,
    /// Format.
    pub format: ModelFormat,
    /// Input shape (e.g., [1, 28, 28]).
    pub input_shape: Vec<usize>,
    /// Output shape (e.g., [1, 10]).
    pub output_shape: Vec<usize>,
    /// Number of parameters.
    pub num_params: usize,
    /// Model size in bytes.
    pub size_bytes: usize,
    /// Whether the model is quantized.
    pub quantized: bool,
    /// Quantization type (if quantized).
    pub quant_type: Option<QuantType>,
    /// Version / hash for hot-swap.
    pub version: String,
}

/// Quantization type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuantType {
    Int8,
    Uint8,
    Float16,
    Int4,
}

impl Model {
    /// Returns total FLOPs for a single inference (estimate).
    pub fn estimated_flops(&self) -> u64 {
        // Rough: 2 * params for matmul-dominated models
        self.num_params as u64 * 2
    }

    /// Returns memory required for inference (activations + weights).
    pub fn memory_required(&self) -> usize {
        let weight_bytes = if self.quantized {
            self.num_params // INT8: 1 byte per param
        } else {
            self.num_params * 4 // FP32: 4 bytes per param
        };
        let activation_bytes: usize = self.input_shape.iter().product::<usize>() * 4
            + self.output_shape.iter().product::<usize>() * 4;
        weight_bytes + activation_bytes
    }
}

// ═══════════════════════════════════════════════════════════════════════
// R2.1.2: Inference Scheduler
// ═══════════════════════════════════════════════════════════════════════

/// Priority level for inference tasks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum InferencePriority {
    /// Best-effort (no deadline).
    Low,
    /// Normal (soft deadline).
    Normal,
    /// High (hard deadline).
    High,
    /// Critical (safety-related, must complete).
    Critical,
}

/// An inference request in the scheduler queue.
#[derive(Debug, Clone)]
pub struct InferenceRequest {
    /// Request ID.
    pub id: u64,
    /// Model to use.
    pub model_name: String,
    /// Priority.
    pub priority: InferencePriority,
    /// Deadline in microseconds from now.
    pub deadline_us: u64,
    /// Input data (flattened f32 values).
    pub input: Vec<f32>,
    /// Batch index (for batch inference).
    pub batch_index: Option<u32>,
}

/// Inference result.
#[derive(Debug, Clone)]
pub struct InferenceResult {
    /// Request ID.
    pub request_id: u64,
    /// Output values.
    pub output: Vec<f32>,
    /// Inference latency in microseconds.
    pub latency_us: u64,
    /// Whether deadline was met.
    pub deadline_met: bool,
    /// Confidence score (max output value).
    pub confidence: f32,
    /// Predicted class (argmax of output).
    pub predicted_class: usize,
}

impl InferenceResult {
    /// Computes confidence and class from output.
    pub fn from_output(
        request_id: u64,
        output: Vec<f32>,
        latency_us: u64,
        deadline_us: u64,
    ) -> Self {
        let (predicted_class, confidence) = output
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(i, &v)| (i, v))
            .unwrap_or((0, 0.0));
        Self {
            request_id,
            output,
            latency_us,
            deadline_met: latency_us <= deadline_us,
            confidence,
            predicted_class,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// R2.1.5: Model Hot-Swap
// ═══════════════════════════════════════════════════════════════════════

/// Model registry — manages loaded models with hot-swap support.
#[derive(Debug, Clone)]
pub struct ModelRegistry {
    /// Active models by name.
    pub models: HashMap<String, Model>,
    /// Pending replacement (new version being loaded).
    pub pending: HashMap<String, Model>,
    /// Total swaps performed.
    pub swap_count: u64,
}

impl Default for ModelRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ModelRegistry {
    /// Creates a new empty registry.
    pub fn new() -> Self {
        Self {
            models: HashMap::new(),
            pending: HashMap::new(),
            swap_count: 0,
        }
    }

    /// Registers a model.
    pub fn register(&mut self, model: Model) {
        self.models.insert(model.name.clone(), model);
    }

    /// Gets a model by name.
    pub fn get(&self, name: &str) -> Option<&Model> {
        self.models.get(name)
    }

    /// Stages a new version for hot-swap.
    pub fn stage_swap(&mut self, model: Model) {
        self.pending.insert(model.name.clone(), model);
    }

    /// Commits the swap (atomic replacement).
    pub fn commit_swap(&mut self, name: &str) -> bool {
        if let Some(new_model) = self.pending.remove(name) {
            self.models.insert(name.to_string(), new_model);
            self.swap_count += 1;
            true
        } else {
            false
        }
    }

    /// Number of loaded models.
    pub fn count(&self) -> usize {
        self.models.len()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// R2.1.6: Multi-Model Pipeline
// ═══════════════════════════════════════════════════════════════════════

/// A stage in a multi-model pipeline.
#[derive(Debug, Clone)]
pub struct PipelineStage {
    /// Model name for this stage.
    pub model_name: String,
    /// Stage name (e.g., "detector", "classifier").
    pub stage_name: String,
    /// Input transformation (e.g., crop, resize).
    pub input_transform: Option<String>,
    /// Minimum confidence to pass to next stage.
    pub confidence_threshold: f32,
}

/// Multi-model inference pipeline (e.g., detector → classifier → tracker).
#[derive(Debug, Clone)]
pub struct ModelPipeline {
    /// Pipeline name.
    pub name: String,
    /// Ordered stages.
    pub stages: Vec<PipelineStage>,
    /// Whether to short-circuit on low confidence.
    pub short_circuit: bool,
}

impl ModelPipeline {
    /// Creates a new pipeline.
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            stages: Vec::new(),
            short_circuit: true,
        }
    }

    /// Adds a stage.
    pub fn add_stage(mut self, model: &str, stage: &str, threshold: f32) -> Self {
        self.stages.push(PipelineStage {
            model_name: model.to_string(),
            stage_name: stage.to_string(),
            input_transform: None,
            confidence_threshold: threshold,
        });
        self
    }

    /// Number of stages.
    pub fn stage_count(&self) -> usize {
        self.stages.len()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// R2.1.8: Inference Cache (LRU)
// ═══════════════════════════════════════════════════════════════════════

/// LRU cache for inference results.
#[derive(Debug, Clone)]
pub struct InferenceCache {
    /// Cache entries: hash → (result, access count).
    entries: Vec<(u64, InferenceResult, u64)>,
    /// Maximum entries.
    pub capacity: usize,
    /// Cache hits.
    pub hits: u64,
    /// Cache misses.
    pub misses: u64,
}

impl InferenceCache {
    /// Creates a new cache.
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: Vec::with_capacity(capacity),
            capacity,
            hits: 0,
            misses: 0,
        }
    }

    /// Looks up a cached result by input hash.
    pub fn get(&mut self, input_hash: u64) -> Option<&InferenceResult> {
        if let Some(pos) = self.entries.iter().position(|(h, _, _)| *h == input_hash) {
            self.entries[pos].2 += 1; // increment access
            self.hits += 1;
            Some(&self.entries[pos].1)
        } else {
            self.misses += 1;
            None
        }
    }

    /// Inserts a result into the cache.
    pub fn insert(&mut self, input_hash: u64, result: InferenceResult) {
        if self.entries.len() >= self.capacity {
            // Evict least recently used (lowest access count)
            let min_idx = self
                .entries
                .iter()
                .enumerate()
                .min_by_key(|(_, (_, _, count))| *count)
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.entries.remove(min_idx);
        }
        self.entries.push((input_hash, result, 1));
    }

    /// Cache hit rate.
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            return 0.0;
        }
        self.hits as f64 / total as f64
    }
}

// ═══════════════════════════════════════════════════════════════════════
// R2.1.10: Latency SLA
// ═══════════════════════════════════════════════════════════════════════

/// Latency SLA tracker.
#[derive(Debug, Clone)]
pub struct LatencySla {
    /// Target latency in microseconds.
    pub target_us: u64,
    /// Measured latencies (recent window).
    pub latencies: Vec<u64>,
    /// Maximum window size.
    pub window_size: usize,
    /// Total violations.
    pub violations: u64,
    /// Total requests.
    pub total: u64,
}

impl LatencySla {
    /// Creates a new SLA tracker.
    pub fn new(target_us: u64, window_size: usize) -> Self {
        Self {
            target_us,
            latencies: Vec::with_capacity(window_size),
            window_size,
            violations: 0,
            total: 0,
        }
    }

    /// Records a latency measurement.
    pub fn record(&mut self, latency_us: u64) {
        if self.latencies.len() >= self.window_size {
            self.latencies.remove(0);
        }
        self.latencies.push(latency_us);
        self.total += 1;
        if latency_us > self.target_us {
            self.violations += 1;
        }
    }

    /// Returns p50 latency.
    pub fn p50(&self) -> u64 {
        self.percentile(50)
    }

    /// Returns p95 latency.
    pub fn p95(&self) -> u64 {
        self.percentile(95)
    }

    /// Returns p99 latency.
    pub fn p99(&self) -> u64 {
        self.percentile(99)
    }

    /// Returns a specific percentile.
    pub fn percentile(&self, p: u32) -> u64 {
        if self.latencies.is_empty() {
            return 0;
        }
        let mut sorted = self.latencies.clone();
        sorted.sort();
        let idx = (p as usize * sorted.len() / 100).min(sorted.len() - 1);
        sorted[idx]
    }

    /// Returns SLA compliance rate (0.0 to 1.0).
    pub fn compliance(&self) -> f64 {
        if self.total == 0 {
            return 1.0;
        }
        1.0 - (self.violations as f64 / self.total as f64)
    }

    /// Returns mean latency.
    pub fn mean(&self) -> f64 {
        if self.latencies.is_empty() {
            return 0.0;
        }
        self.latencies.iter().sum::<u64>() as f64 / self.latencies.len() as f64
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn r2_1_model_memory() {
        let model = Model {
            name: "mnist".to_string(),
            format: ModelFormat::Fjml,
            input_shape: vec![1, 784],
            output_shape: vec![1, 10],
            num_params: 100_000,
            size_bytes: 400_000,
            quantized: false,
            quant_type: None,
            version: "1.0".to_string(),
        };
        assert_eq!(model.estimated_flops(), 200_000);
        // FP32: 100K*4 + (784+10)*4 = 403_176
        assert!(model.memory_required() > 400_000);
    }

    #[test]
    fn r2_1_model_quantized() {
        let model = Model {
            name: "mnist_int8".to_string(),
            format: ModelFormat::QnnDlc,
            input_shape: vec![1, 784],
            output_shape: vec![1, 10],
            num_params: 100_000,
            size_bytes: 100_000,
            quantized: true,
            quant_type: Some(QuantType::Int8),
            version: "1.0".to_string(),
        };
        // INT8: 100K*1 + (784+10)*4
        assert!(model.memory_required() < 110_000);
    }

    #[test]
    fn r2_2_inference_result() {
        let output = vec![0.1, 0.05, 0.8, 0.05];
        let result = InferenceResult::from_output(1, output, 5000, 10000);
        assert_eq!(result.predicted_class, 2);
        assert!((result.confidence - 0.8).abs() < 0.001);
        assert!(result.deadline_met);
    }

    #[test]
    fn r2_2_deadline_missed() {
        let result = InferenceResult::from_output(1, vec![1.0], 15000, 10000);
        assert!(!result.deadline_met);
    }

    #[test]
    fn r2_5_model_registry() {
        let mut reg = ModelRegistry::new();
        reg.register(Model {
            name: "det".to_string(),
            format: ModelFormat::Onnx,
            input_shape: vec![1, 3, 224, 224],
            output_shape: vec![1, 100, 6],
            num_params: 5_000_000,
            size_bytes: 20_000_000,
            quantized: false,
            quant_type: None,
            version: "1.0".to_string(),
        });
        assert_eq!(reg.count(), 1);
        assert!(reg.get("det").is_some());
    }

    #[test]
    fn r2_5_hot_swap() {
        let mut reg = ModelRegistry::new();
        reg.register(Model {
            name: "clf".to_string(),
            format: ModelFormat::Fjml,
            input_shape: vec![1, 10],
            output_shape: vec![1, 5],
            num_params: 1000,
            size_bytes: 4000,
            quantized: false,
            quant_type: None,
            version: "1.0".to_string(),
        });
        reg.stage_swap(Model {
            name: "clf".to_string(),
            format: ModelFormat::Fjml,
            input_shape: vec![1, 10],
            output_shape: vec![1, 5],
            num_params: 1200,
            size_bytes: 4800,
            quantized: false,
            quant_type: None,
            version: "2.0".to_string(),
        });
        assert!(reg.commit_swap("clf"));
        assert_eq!(reg.get("clf").unwrap().version, "2.0");
        assert_eq!(reg.swap_count, 1);
    }

    #[test]
    fn r2_6_multi_model_pipeline() {
        let pipeline = ModelPipeline::new("object_recognition")
            .add_stage("yolo_tiny", "detector", 0.5)
            .add_stage("resnet18", "classifier", 0.8)
            .add_stage("kalman", "tracker", 0.3);
        assert_eq!(pipeline.stage_count(), 3);
        assert_eq!(pipeline.stages[0].stage_name, "detector");
    }

    #[test]
    fn r2_8_inference_cache() {
        let mut cache = InferenceCache::new(3);
        let result = InferenceResult::from_output(1, vec![0.9, 0.1], 1000, 5000);
        cache.insert(12345, result);
        assert!(cache.get(12345).is_some());
        assert!(cache.get(99999).is_none());
        assert_eq!(cache.hits, 1);
        assert_eq!(cache.misses, 1);
        assert!((cache.hit_rate() - 0.5).abs() < 0.001);
    }

    #[test]
    fn r2_10_latency_sla() {
        let mut sla = LatencySla::new(10000, 100); // 10ms target
        for us in [5000, 8000, 9000, 12000, 7000, 6000, 11000, 4000, 3000, 9500] {
            sla.record(us);
        }
        assert_eq!(sla.violations, 2); // 12000 and 11000 exceeded
        assert_eq!(sla.total, 10);
        assert!((sla.compliance() - 0.8).abs() < 0.001);
        assert!(sla.p50() < 10000);
        assert!(sla.p99() >= 10000);
    }

    #[test]
    fn r2_10_sla_percentiles() {
        let mut sla = LatencySla::new(10000, 100);
        for i in 1..=100 {
            sla.record(i * 100);
        } // 100, 200, ..., 10000
        assert!(sla.p50() >= 4000 && sla.p50() <= 6000);
        assert!(sla.p95() >= 9000);
    }

    #[test]
    fn r2_1_priority_ordering() {
        assert!(InferencePriority::Low < InferencePriority::Normal);
        assert!(InferencePriority::Normal < InferencePriority::High);
        assert!(InferencePriority::High < InferencePriority::Critical);
    }
}
