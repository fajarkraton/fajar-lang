//! Model Serving — inference server, dynamic batching, model registry,
//! validation, postprocessing, warmup, health checks, metrics, versioning.

use std::collections::HashMap;
use std::fmt;
use std::time::Duration;

// ═══════════════════════════════════════════════════════════════════════
// S16.1: Inference Server
// ═══════════════════════════════════════════════════════════════════════

/// Server protocol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServerProtocol {
    /// HTTP REST API.
    Http,
    /// gRPC.
    Grpc,
}

impl fmt::Display for ServerProtocol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ServerProtocol::Http => write!(f, "HTTP"),
            ServerProtocol::Grpc => write!(f, "gRPC"),
        }
    }
}

/// Inference server configuration.
#[derive(Debug, Clone)]
pub struct InferenceServerConfig {
    /// Server protocol.
    pub protocol: ServerProtocol,
    /// Listening port.
    pub port: u16,
    /// Number of worker threads.
    pub workers: usize,
    /// Maximum request body size in bytes.
    pub max_body_size: usize,
}

impl Default for InferenceServerConfig {
    fn default() -> Self {
        InferenceServerConfig {
            protocol: ServerProtocol::Http,
            port: 8080,
            workers: 4,
            max_body_size: 10 * 1024 * 1024, // 10MB
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S16.2: Dynamic Batching
// ═══════════════════════════════════════════════════════════════════════

/// A pending inference request.
#[derive(Debug, Clone)]
pub struct InferenceRequest {
    /// Request ID.
    pub id: u64,
    /// Input data.
    pub input: Vec<f64>,
    /// Input shape.
    pub shape: Vec<usize>,
}

/// Dynamic batching configuration.
#[derive(Debug, Clone)]
pub struct BatchingConfig {
    /// Maximum batch size.
    pub max_batch_size: usize,
    /// Maximum wait time before dispatching an incomplete batch.
    pub max_wait: Duration,
}

impl Default for BatchingConfig {
    fn default() -> Self {
        BatchingConfig {
            max_batch_size: 32,
            max_wait: Duration::from_millis(10),
        }
    }
}

/// A batcher that accumulates requests.
#[derive(Debug)]
pub struct DynamicBatcher {
    /// Configuration.
    pub config: BatchingConfig,
    /// Pending requests.
    pub pending: Vec<InferenceRequest>,
}

impl DynamicBatcher {
    /// Creates a new batcher.
    pub fn new(config: BatchingConfig) -> Self {
        DynamicBatcher {
            config,
            pending: Vec::new(),
        }
    }

    /// Adds a request to the pending queue.
    pub fn add(&mut self, request: InferenceRequest) {
        self.pending.push(request);
    }

    /// Checks if a batch should be dispatched.
    pub fn should_dispatch(&self) -> bool {
        self.pending.len() >= self.config.max_batch_size
    }

    /// Takes the current batch for processing.
    pub fn take_batch(&mut self) -> Vec<InferenceRequest> {
        let batch_size = self.pending.len().min(self.config.max_batch_size);
        self.pending.drain(..batch_size).collect()
    }

    /// Returns the number of pending requests.
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S16.3: Model Registry
// ═══════════════════════════════════════════════════════════════════════

/// A registered model.
#[derive(Debug, Clone)]
pub struct RegisteredModel {
    /// Model name.
    pub name: String,
    /// Model version.
    pub version: String,
    /// Input shape.
    pub input_shape: Vec<usize>,
    /// Output shape.
    pub output_shape: Vec<usize>,
    /// Whether the model is loaded and ready.
    pub loaded: bool,
}

/// Model registry for managing model lifecycle.
#[derive(Debug, Default)]
pub struct ModelRegistry {
    /// Models indexed by "name:version".
    models: HashMap<String, RegisteredModel>,
    /// Active model per name (for A/B testing).
    active: HashMap<String, String>,
}

impl ModelRegistry {
    /// Creates a new empty registry.
    pub fn new() -> Self {
        ModelRegistry::default()
    }

    /// Registers a model.
    pub fn register(&mut self, model: RegisteredModel) {
        let key = format!("{}:{}", model.name, model.version);
        if !self.active.contains_key(&model.name) {
            self.active
                .insert(model.name.clone(), model.version.clone());
        }
        self.models.insert(key, model);
    }

    /// Gets the active model for a name.
    pub fn get_active(&self, name: &str) -> Option<&RegisteredModel> {
        let version = self.active.get(name)?;
        self.models.get(&format!("{name}:{version}"))
    }

    /// Sets the active version for a model name.
    pub fn set_active(&mut self, name: &str, version: &str) -> bool {
        let key = format!("{name}:{version}");
        if self.models.contains_key(&key) {
            self.active.insert(name.to_string(), version.to_string());
            true
        } else {
            false
        }
    }

    /// Unloads a model.
    pub fn unload(&mut self, name: &str, version: &str) {
        let key = format!("{name}:{version}");
        if let Some(model) = self.models.get_mut(&key) {
            model.loaded = false;
        }
    }

    /// Returns the number of registered models.
    pub fn len(&self) -> usize {
        self.models.len()
    }

    /// Returns true if no models are registered.
    pub fn is_empty(&self) -> bool {
        self.models.is_empty()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S16.4: Input Validation
// ═══════════════════════════════════════════════════════════════════════

/// Validation error for inference requests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationError {
    /// Shape mismatch.
    ShapeMismatch {
        expected: Vec<usize>,
        got: Vec<usize>,
    },
    /// Empty input.
    EmptyInput,
    /// Unknown model.
    UnknownModel(String),
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ValidationError::ShapeMismatch { expected, got } => {
                write!(f, "shape mismatch: expected {expected:?}, got {got:?}")
            }
            ValidationError::EmptyInput => write!(f, "empty input"),
            ValidationError::UnknownModel(name) => write!(f, "unknown model: {name}"),
        }
    }
}

/// Validates an inference request against the model's expected shape.
pub fn validate_request(
    request: &InferenceRequest,
    expected_shape: &[usize],
) -> Result<(), ValidationError> {
    if request.input.is_empty() {
        return Err(ValidationError::EmptyInput);
    }
    if request.shape != expected_shape {
        return Err(ValidationError::ShapeMismatch {
            expected: expected_shape.to_vec(),
            got: request.shape.clone(),
        });
    }
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// S16.5: Output Postprocessing
// ═══════════════════════════════════════════════════════════════════════

/// Applies softmax to logits.
pub fn softmax(logits: &[f64]) -> Vec<f64> {
    let max_val = logits.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let exps: Vec<f64> = logits.iter().map(|&l| (l - max_val).exp()).collect();
    let sum: f64 = exps.iter().sum();
    exps.iter().map(|&e| e / sum).collect()
}

/// Returns the index of the maximum value.
pub fn serving_argmax(values: &[f64]) -> usize {
    values
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(i, _)| i)
        .unwrap_or(0)
}

/// Maps an index to a label.
pub fn map_label(index: usize, labels: &[&str]) -> String {
    labels
        .get(index)
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("class_{index}"))
}

// ═══════════════════════════════════════════════════════════════════════
// S16.6: Model Warmup
// ═══════════════════════════════════════════════════════════════════════

/// Warmup configuration.
#[derive(Debug, Clone)]
pub struct WarmupConfig {
    /// Number of warmup iterations.
    pub iterations: usize,
    /// Input shape for dummy data.
    pub input_shape: Vec<usize>,
}

/// Generates dummy warmup data.
pub fn generate_warmup_data(shape: &[usize]) -> Vec<f64> {
    let total: usize = shape.iter().product();
    vec![0.0; total]
}

// ═══════════════════════════════════════════════════════════════════════
// S16.7: Health & Readiness
// ═══════════════════════════════════════════════════════════════════════

/// Health check status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HealthStatus {
    /// Server is healthy.
    Healthy,
    /// Server is starting up.
    Starting,
    /// Server is unhealthy.
    Unhealthy(String),
}

impl fmt::Display for HealthStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HealthStatus::Healthy => write!(f, "Healthy"),
            HealthStatus::Starting => write!(f, "Starting"),
            HealthStatus::Unhealthy(reason) => write!(f, "Unhealthy({reason})"),
        }
    }
}

/// Readiness check: is the server ready to serve requests?
pub fn check_readiness(registry: &ModelRegistry, required_model: &str) -> HealthStatus {
    match registry.get_active(required_model) {
        Some(model) if model.loaded => HealthStatus::Healthy,
        Some(_) => HealthStatus::Starting,
        None => HealthStatus::Unhealthy(format!("model `{required_model}` not found")),
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S16.8: Inference Metrics
// ═══════════════════════════════════════════════════════════════════════

/// Inference performance metrics.
#[derive(Debug, Clone, Default)]
pub struct InferenceMetrics {
    /// Total requests processed.
    pub total_requests: u64,
    /// Total latency in milliseconds.
    pub total_latency_ms: f64,
    /// Maximum latency in milliseconds.
    pub max_latency_ms: f64,
    /// Batch size distribution: size → count.
    pub batch_sizes: HashMap<usize, u64>,
}

impl InferenceMetrics {
    /// Records a completed inference.
    pub fn record(&mut self, latency_ms: f64, batch_size: usize) {
        self.total_requests += 1;
        self.total_latency_ms += latency_ms;
        if latency_ms > self.max_latency_ms {
            self.max_latency_ms = latency_ms;
        }
        *self.batch_sizes.entry(batch_size).or_insert(0) += 1;
    }

    /// Returns the average latency.
    pub fn avg_latency_ms(&self) -> f64 {
        if self.total_requests == 0 {
            0.0
        } else {
            self.total_latency_ms / self.total_requests as f64
        }
    }

    /// Returns throughput (requests per second), given total elapsed seconds.
    pub fn throughput(&self, elapsed_secs: f64) -> f64 {
        if elapsed_secs <= 0.0 {
            0.0
        } else {
            self.total_requests as f64 / elapsed_secs
        }
    }
}

impl fmt::Display for InferenceMetrics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "requests={}, avg_latency={:.2}ms, max_latency={:.2}ms",
            self.total_requests,
            self.avg_latency_ms(),
            self.max_latency_ms
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S16.9: Model Versioning
// ═══════════════════════════════════════════════════════════════════════

/// Traffic split configuration for blue-green deployment.
#[derive(Debug, Clone)]
pub struct TrafficSplit {
    /// Version A (blue).
    pub version_a: String,
    /// Version B (green).
    pub version_b: String,
    /// Percentage of traffic to version B (0-100).
    pub percent_b: u32,
}

impl TrafficSplit {
    /// Determines which version should handle a request based on a random value (0-99).
    pub fn route(&self, random_percent: u32) -> &str {
        if random_percent < self.percent_b {
            &self.version_b
        } else {
            &self.version_a
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // S16.1 — Inference Server
    #[test]
    fn s16_1_server_config() {
        let cfg = InferenceServerConfig::default();
        assert_eq!(cfg.port, 8080);
        assert_eq!(cfg.protocol, ServerProtocol::Http);
    }

    // S16.2 — Dynamic Batching
    #[test]
    fn s16_2_dynamic_batching() {
        let config = BatchingConfig {
            max_batch_size: 4,
            max_wait: Duration::from_millis(10),
        };
        let mut batcher = DynamicBatcher::new(config);

        for i in 0..3 {
            batcher.add(InferenceRequest {
                id: i,
                input: vec![1.0],
                shape: vec![1],
            });
        }
        assert!(!batcher.should_dispatch());

        batcher.add(InferenceRequest {
            id: 3,
            input: vec![1.0],
            shape: vec![1],
        });
        assert!(batcher.should_dispatch());

        let batch = batcher.take_batch();
        assert_eq!(batch.len(), 4);
        assert_eq!(batcher.pending_count(), 0);
    }

    // S16.3 — Model Registry
    #[test]
    fn s16_3_model_registry() {
        let mut registry = ModelRegistry::new();
        registry.register(RegisteredModel {
            name: "resnet".into(),
            version: "1.0".into(),
            input_shape: vec![1, 3, 224, 224],
            output_shape: vec![1, 1000],
            loaded: true,
        });
        registry.register(RegisteredModel {
            name: "resnet".into(),
            version: "2.0".into(),
            input_shape: vec![1, 3, 224, 224],
            output_shape: vec![1, 1000],
            loaded: true,
        });

        let active = registry.get_active("resnet").unwrap();
        assert_eq!(active.version, "1.0"); // First registered is default

        registry.set_active("resnet", "2.0");
        let active = registry.get_active("resnet").unwrap();
        assert_eq!(active.version, "2.0");
    }

    // S16.4 — Input Validation
    #[test]
    fn s16_4_validation() {
        let req = InferenceRequest {
            id: 1,
            input: vec![1.0, 2.0, 3.0],
            shape: vec![1, 3],
        };
        assert!(validate_request(&req, &[1, 3]).is_ok());
        assert!(validate_request(&req, &[1, 4]).is_err());
    }

    #[test]
    fn s16_4_empty_input() {
        let req = InferenceRequest {
            id: 1,
            input: vec![],
            shape: vec![0],
        };
        assert_eq!(
            validate_request(&req, &[0]),
            Err(ValidationError::EmptyInput)
        );
    }

    // S16.5 — Output Postprocessing
    #[test]
    fn s16_5_softmax() {
        let probs = softmax(&[1.0, 2.0, 3.0]);
        let sum: f64 = probs.iter().sum();
        assert!((sum - 1.0).abs() < 1e-10);
        assert!(probs[2] > probs[1] && probs[1] > probs[0]);
    }

    #[test]
    fn s16_5_label_mapping() {
        let labels = vec!["cat", "dog", "bird"];
        assert_eq!(map_label(1, &labels), "dog");
        assert_eq!(map_label(5, &labels), "class_5");
    }

    // S16.6 — Model Warmup
    #[test]
    fn s16_6_warmup_data() {
        let data = generate_warmup_data(&[1, 3, 224, 224]);
        assert_eq!(data.len(), 3 * 224 * 224);
    }

    // S16.7 — Health & Readiness
    #[test]
    fn s16_7_health_check() {
        let mut registry = ModelRegistry::new();
        assert_eq!(
            check_readiness(&registry, "model"),
            HealthStatus::Unhealthy("model `model` not found".into())
        );

        registry.register(RegisteredModel {
            name: "model".into(),
            version: "1.0".into(),
            input_shape: vec![1],
            output_shape: vec![1],
            loaded: true,
        });
        assert_eq!(check_readiness(&registry, "model"), HealthStatus::Healthy);
    }

    // S16.8 — Inference Metrics
    #[test]
    fn s16_8_metrics() {
        let mut metrics = InferenceMetrics::default();
        metrics.record(5.0, 8);
        metrics.record(15.0, 16);
        assert_eq!(metrics.total_requests, 2);
        assert!((metrics.avg_latency_ms() - 10.0).abs() < 1e-10);
        assert_eq!(metrics.max_latency_ms, 15.0);
        assert!((metrics.throughput(1.0) - 2.0).abs() < 1e-10);
    }

    #[test]
    fn s16_8_metrics_display() {
        let mut metrics = InferenceMetrics::default();
        metrics.record(10.0, 4);
        let s = metrics.to_string();
        assert!(s.contains("requests=1"));
    }

    // S16.9 — Model Versioning
    #[test]
    fn s16_9_traffic_split() {
        let split = TrafficSplit {
            version_a: "1.0".into(),
            version_b: "2.0".into(),
            percent_b: 20,
        };
        assert_eq!(split.route(10), "2.0"); // 10 < 20 → version B
        assert_eq!(split.route(50), "1.0"); // 50 >= 20 → version A
    }

    // S16.10 — Integration
    #[test]
    fn s16_10_server_protocol_display() {
        assert_eq!(ServerProtocol::Http.to_string(), "HTTP");
        assert_eq!(ServerProtocol::Grpc.to_string(), "gRPC");
    }

    #[test]
    fn s16_10_health_status_display() {
        assert_eq!(HealthStatus::Healthy.to_string(), "Healthy");
        assert_eq!(HealthStatus::Starting.to_string(), "Starting");
    }

    #[test]
    fn s16_10_validation_error_display() {
        let err = ValidationError::EmptyInput;
        assert_eq!(err.to_string(), "empty input");
    }
}
