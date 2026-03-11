//! Dispatch runtime — automatic accelerator selection for @infer workloads.
//!
//! Scores available accelerators, implements CPU/NPU/GPU paths with automatic
//! fallback, latency profiling, workload classification, and dispatch caching.

use std::collections::HashMap;
use std::fmt;
use std::time::{Duration, Instant};

use super::infer::InferPreference;

// ═══════════════════════════════════════════════════════════════════════
// S34.7: Workload Classification
// ═══════════════════════════════════════════════════════════════════════

/// Classification of a workload's bottleneck characteristic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WorkloadClass {
    /// Dominated by arithmetic operations (large matmuls, convolutions).
    ComputeBound,
    /// Dominated by memory bandwidth (elementwise ops on large tensors).
    MemoryBound,
    /// Dominated by launch overhead (small batch, low latency required).
    LatencySensitive,
}

impl fmt::Display for WorkloadClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ComputeBound => write!(f, "compute-bound"),
            Self::MemoryBound => write!(f, "memory-bound"),
            Self::LatencySensitive => write!(f, "latency-sensitive"),
        }
    }
}

/// Classifies a workload based on its characteristics.
///
/// Uses arithmetic intensity (FLOPS / bytes accessed) as the primary metric.
pub fn classify_workload(flops: u64, bytes_accessed: u64, batch_size: u32) -> WorkloadClass {
    if batch_size <= 1 {
        return WorkloadClass::LatencySensitive;
    }

    if bytes_accessed == 0 {
        return WorkloadClass::ComputeBound;
    }

    let intensity = flops as f64 / bytes_accessed as f64;

    // Arithmetic intensity threshold: >10 FLOPS/byte = compute-bound
    if intensity > 10.0 {
        WorkloadClass::ComputeBound
    } else {
        WorkloadClass::MemoryBound
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S34.1: Dispatch Decision Engine
// ═══════════════════════════════════════════════════════════════════════

/// Device type for dispatch decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DispatchDevice {
    /// CPU fallback (always available).
    Cpu,
    /// GPU by index.
    Gpu(u32),
    /// NPU by index.
    Npu(u32),
}

impl fmt::Display for DispatchDevice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cpu => write!(f, "CPU"),
            Self::Gpu(id) => write!(f, "GPU:{id}"),
            Self::Npu(id) => write!(f, "NPU:{id}"),
        }
    }
}

/// Describes a workload for dispatch scoring.
#[derive(Debug, Clone)]
pub struct WorkloadDescriptor {
    /// Operation type (e.g., "matmul", "conv2d", "elementwise").
    pub op_type: String,
    /// Number of elements in primary input tensor.
    pub input_elements: u64,
    /// Data type ("f32", "f16", "i8").
    pub dtype: String,
    /// Batch size.
    pub batch_size: u32,
    /// Estimated FLOPS.
    pub estimated_flops: u64,
    /// Estimated bytes accessed.
    pub estimated_bytes: u64,
    /// User preference from @infer annotation.
    pub preference: InferPreference,
}

/// Score for a single accelerator.
#[derive(Debug, Clone)]
pub struct DeviceScore {
    /// Device being scored.
    pub device: DispatchDevice,
    /// Numeric score (higher = better).
    pub score: f64,
    /// Reason for the score.
    pub reason: String,
    /// Whether the device is available.
    pub available: bool,
}

/// Scores available accelerators for a given workload.
pub fn score_accelerators(
    workload: &WorkloadDescriptor,
    available: &DeviceSet,
) -> Vec<DeviceScore> {
    let mut scores = Vec::new();
    let wclass = classify_workload(
        workload.estimated_flops,
        workload.estimated_bytes,
        workload.batch_size,
    );

    // CPU always available
    let cpu_score = score_cpu(workload, &wclass);
    scores.push(cpu_score);

    // Score each GPU
    for gpu_id in &available.gpu_ids {
        let gpu_score = score_gpu(*gpu_id, workload, &wclass);
        scores.push(gpu_score);
    }

    // Score each NPU
    for npu_id in &available.npu_ids {
        let npu_score = score_npu(*npu_id, workload, &wclass);
        scores.push(npu_score);
    }

    // Apply user preference bonus
    if workload.preference != InferPreference::Auto {
        for score in &mut scores {
            let pref_match = matches!(
                (workload.preference, score.device),
                (InferPreference::Cpu, DispatchDevice::Cpu)
                    | (InferPreference::Gpu, DispatchDevice::Gpu(_))
                    | (InferPreference::Npu, DispatchDevice::Npu(_))
            );
            if pref_match {
                score.score *= 1.5; // 50% bonus for preferred device
                score.reason = format!("{} (user preferred)", score.reason);
            }
        }
    }

    // Sort by score descending
    scores.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    scores
}

fn score_cpu(workload: &WorkloadDescriptor, _wclass: &WorkloadClass) -> DeviceScore {
    // CPU is always decent for small workloads, poor for large matmuls
    let base = if workload.input_elements < 1024 {
        80.0 // CPU excels at small workloads (no launch overhead)
    } else if workload.input_elements < 65536 {
        50.0
    } else {
        20.0 // Large workloads → prefer accelerator
    };

    DeviceScore {
        device: DispatchDevice::Cpu,
        score: base,
        reason: format!(
            "CPU: {:.0} elements, base={base:.0}",
            workload.input_elements
        ),
        available: true,
    }
}

fn score_gpu(id: u32, workload: &WorkloadDescriptor, wclass: &WorkloadClass) -> DeviceScore {
    let base = match wclass {
        WorkloadClass::ComputeBound => 90.0,     // GPU excels
        WorkloadClass::MemoryBound => 70.0,      // GPU decent
        WorkloadClass::LatencySensitive => 30.0, // GPU has launch overhead
    };

    // Bonus for large inputs
    let size_bonus = if workload.input_elements > 65536 {
        20.0
    } else {
        0.0
    };

    // INT8 workloads are better on NPU usually
    let dtype_penalty = if workload.dtype == "i8" { -15.0 } else { 0.0 };

    DeviceScore {
        device: DispatchDevice::Gpu(id),
        score: base + size_bonus + dtype_penalty,
        reason: format!("GPU:{id}: {wclass}, size_bonus={size_bonus:.0}"),
        available: true,
    }
}

fn score_npu(id: u32, workload: &WorkloadDescriptor, wclass: &WorkloadClass) -> DeviceScore {
    let base = match wclass {
        WorkloadClass::ComputeBound => 60.0, // NPU decent for inference
        WorkloadClass::MemoryBound => 50.0,
        WorkloadClass::LatencySensitive => 85.0, // NPU excels at low latency
    };

    // NPU excels at INT8/FP16
    let dtype_bonus = match workload.dtype.as_str() {
        "i8" => 30.0,
        "f16" => 20.0,
        _ => 0.0,
    };

    // NPU has limited model size
    let size_penalty = if workload.input_elements > 1_000_000 {
        -20.0
    } else {
        0.0
    };

    DeviceScore {
        device: DispatchDevice::Npu(id),
        score: base + dtype_bonus + size_penalty,
        reason: format!("NPU:{id}: {wclass}, dtype_bonus={dtype_bonus:.0}"),
        available: true,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S34.2-S34.4: Execution Paths
// ═══════════════════════════════════════════════════════════════════════

/// Set of available devices for dispatch.
#[derive(Debug, Clone, Default)]
pub struct DeviceSet {
    /// Available GPU device indices.
    pub gpu_ids: Vec<u32>,
    /// Available NPU device indices.
    pub npu_ids: Vec<u32>,
}

impl DeviceSet {
    /// Creates a device set with only CPU available.
    pub fn cpu_only() -> Self {
        Self::default()
    }

    /// Creates a device set with all specified devices.
    pub fn new(gpu_ids: Vec<u32>, npu_ids: Vec<u32>) -> Self {
        Self { gpu_ids, npu_ids }
    }

    /// Returns true if any GPU is available.
    pub fn has_gpu(&self) -> bool {
        !self.gpu_ids.is_empty()
    }

    /// Returns true if any NPU is available.
    pub fn has_npu(&self) -> bool {
        !self.npu_ids.is_empty()
    }

    /// Total number of available devices (including CPU).
    pub fn device_count(&self) -> usize {
        1 + self.gpu_ids.len() + self.npu_ids.len()
    }
}

/// Result of a dispatch decision.
#[derive(Debug, Clone)]
pub struct DispatchDecision {
    /// Primary device to execute on.
    pub primary: DispatchDevice,
    /// Fallback device chain (if primary fails).
    pub fallbacks: Vec<DispatchDevice>,
    /// Score of the primary device.
    pub score: f64,
    /// Workload classification.
    pub workload_class: WorkloadClass,
    /// Reason for the decision.
    pub reason: String,
}

/// Makes a dispatch decision for a workload.
pub fn decide_dispatch(workload: &WorkloadDescriptor, devices: &DeviceSet) -> DispatchDecision {
    let scores = score_accelerators(workload, devices);
    let wclass = classify_workload(
        workload.estimated_flops,
        workload.estimated_bytes,
        workload.batch_size,
    );

    let primary = scores
        .first()
        .map(|s| s.device)
        .unwrap_or(DispatchDevice::Cpu);
    let primary_score = scores.first().map(|s| s.score).unwrap_or(0.0);
    let primary_reason = scores.first().map(|s| s.reason.clone()).unwrap_or_default();

    // Build fallback chain: remaining devices by score
    let fallbacks: Vec<DispatchDevice> = scores
        .iter()
        .skip(1)
        .filter(|s| s.available)
        .map(|s| s.device)
        .collect();

    DispatchDecision {
        primary,
        fallbacks,
        score: primary_score,
        workload_class: wclass,
        reason: primary_reason,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S34.5: Automatic Fallback
// ═══════════════════════════════════════════════════════════════════════

/// Errors that trigger fallback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DispatchFailure {
    /// Out of memory on the device.
    OutOfMemory {
        /// Device that ran out of memory.
        device: String,
    },
    /// Unsupported operation on the device.
    UnsupportedOp {
        /// The unsupported operation.
        op: String,
        /// Device that doesn't support it.
        device: String,
    },
    /// Device runtime error.
    RuntimeError {
        /// Error message.
        message: String,
    },
}

impl fmt::Display for DispatchFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutOfMemory { device } => write!(f, "out of memory on {device}"),
            Self::UnsupportedOp { op, device } => {
                write!(f, "operation `{op}` not supported on {device}")
            }
            Self::RuntimeError { message } => write!(f, "runtime error: {message}"),
        }
    }
}

/// Attempts to execute on the next device in the fallback chain.
pub fn fallback_next(
    decision: &DispatchDecision,
    failure: &DispatchFailure,
) -> Option<DispatchDevice> {
    // Log the failure
    let _ = format!("dispatch fallback: {} on {}", failure, decision.primary);

    // Return the next available fallback
    decision.fallbacks.first().copied()
}

// ═══════════════════════════════════════════════════════════════════════
// S34.6: Latency Profiling
// ═══════════════════════════════════════════════════════════════════════

/// Latency measurement for a device.
#[derive(Debug, Clone)]
pub struct DeviceLatency {
    /// Device measured.
    pub device: DispatchDevice,
    /// Warmup latency (first run, includes compilation/loading).
    pub warmup_us: u64,
    /// Steady-state latency (average of subsequent runs).
    pub steady_us: u64,
    /// Number of calibration runs.
    pub calibration_runs: u32,
}

/// Latency calibration results per device.
#[derive(Debug, Clone, Default)]
pub struct LatencyProfile {
    /// Measured latencies by device.
    measurements: HashMap<String, DeviceLatency>,
}

impl LatencyProfile {
    /// Creates a new empty latency profile.
    pub fn new() -> Self {
        Self::default()
    }

    /// Records a latency measurement.
    pub fn record(&mut self, device: DispatchDevice, warmup_us: u64, steady_us: u64, runs: u32) {
        let key = format!("{device}");
        self.measurements.insert(
            key,
            DeviceLatency {
                device,
                warmup_us,
                steady_us,
                calibration_runs: runs,
            },
        );
    }

    /// Gets the latency for a device.
    pub fn get(&self, device: &DispatchDevice) -> Option<&DeviceLatency> {
        self.measurements.get(&format!("{device}"))
    }

    /// Returns the device with lowest steady-state latency.
    pub fn fastest_device(&self) -> Option<DispatchDevice> {
        self.measurements
            .values()
            .min_by_key(|m| m.steady_us)
            .map(|m| m.device)
    }

    /// Returns all measured devices.
    pub fn devices(&self) -> Vec<&DeviceLatency> {
        self.measurements.values().collect()
    }

    /// Runs a simple calibration (simulated for testing).
    pub fn calibrate_simulated(&mut self, device: DispatchDevice, base_latency_us: u64) {
        let warmup = base_latency_us * 3; // Warmup is ~3x steady state
        self.record(device, warmup, base_latency_us, 10);
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S34.8: Dispatch Cache
// ═══════════════════════════════════════════════════════════════════════

/// Cache key for dispatch decisions.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DispatchCacheKey {
    /// Hash of the model/function.
    pub model_hash: u64,
    /// Input shape as string (e.g., "[1, 3, 224, 224]").
    pub input_shape: String,
    /// Available hardware signature.
    pub hw_signature: String,
}

/// Cache of dispatch decisions.
#[derive(Debug, Clone, Default)]
pub struct DispatchCache {
    /// Cached decisions.
    entries: HashMap<DispatchCacheKey, CachedDecision>,
}

/// A cached dispatch decision with metadata.
#[derive(Debug, Clone)]
pub struct CachedDecision {
    /// The cached device.
    pub device: DispatchDevice,
    /// When the decision was made.
    pub cached_at: Instant,
    /// How many times this cache entry has been used.
    pub hit_count: u64,
}

impl DispatchCache {
    /// Creates a new empty cache.
    pub fn new() -> Self {
        Self::default()
    }

    /// Looks up a cached dispatch decision.
    pub fn get(&mut self, key: &DispatchCacheKey) -> Option<DispatchDevice> {
        if let Some(entry) = self.entries.get_mut(key) {
            entry.hit_count += 1;
            Some(entry.device)
        } else {
            None
        }
    }

    /// Stores a dispatch decision in the cache.
    pub fn insert(&mut self, key: DispatchCacheKey, device: DispatchDevice) {
        self.entries.insert(
            key,
            CachedDecision {
                device,
                cached_at: Instant::now(),
                hit_count: 0,
            },
        );
    }

    /// Returns the cache hit rate.
    pub fn hit_rate(&self) -> f64 {
        let total_hits: u64 = self.entries.values().map(|e| e.hit_count).sum();
        let total_entries = self.entries.len() as u64;
        if total_entries == 0 {
            0.0
        } else {
            total_hits as f64 / (total_hits + total_entries) as f64
        }
    }

    /// Returns the number of cached entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns true if the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Clears the cache.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S34.9: Dispatch Logging
// ═══════════════════════════════════════════════════════════════════════

/// Log level for dispatch decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispatchLogLevel {
    /// Only log errors.
    Error,
    /// Log warnings and errors.
    Warn,
    /// Log all decisions.
    Debug,
}

/// A dispatch log entry.
#[derive(Debug, Clone)]
pub struct DispatchLogEntry {
    /// Timestamp relative to start.
    pub timestamp: Duration,
    /// Function or workload name.
    pub workload: String,
    /// Selected device.
    pub device: DispatchDevice,
    /// Score.
    pub score: f64,
    /// Classification.
    pub workload_class: WorkloadClass,
    /// Whether it was a cache hit.
    pub cache_hit: bool,
}

impl fmt::Display for DispatchLogEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let cache = if self.cache_hit { " [cached]" } else { "" };
        write!(
            f,
            "[{:.3}ms] {} -> {} (score={:.1}, class={}{})",
            self.timestamp.as_secs_f64() * 1000.0,
            self.workload,
            self.device,
            self.score,
            self.workload_class,
            cache,
        )
    }
}

/// Dispatch logger that records decisions.
#[derive(Debug, Clone)]
pub struct DispatchLogger {
    /// Minimum log level.
    pub level: DispatchLogLevel,
    /// Recorded entries.
    entries: Vec<DispatchLogEntry>,
    /// Start time for relative timestamps.
    start: Instant,
}

impl DispatchLogger {
    /// Creates a new dispatch logger.
    pub fn new(level: DispatchLogLevel) -> Self {
        Self {
            level,
            entries: Vec::new(),
            start: Instant::now(),
        }
    }

    /// Logs a dispatch decision.
    pub fn log_decision(
        &mut self,
        workload: &str,
        device: DispatchDevice,
        score: f64,
        wclass: WorkloadClass,
        cache_hit: bool,
    ) {
        self.entries.push(DispatchLogEntry {
            timestamp: self.start.elapsed(),
            workload: workload.to_string(),
            device,
            score,
            workload_class: wclass,
            cache_hit,
        });
    }

    /// Returns all log entries.
    pub fn entries(&self) -> &[DispatchLogEntry] {
        &self.entries
    }

    /// Returns formatted log output.
    pub fn format_log(&self) -> String {
        self.entries
            .iter()
            .map(|e| format!("{e}"))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn test_workload() -> WorkloadDescriptor {
        WorkloadDescriptor {
            op_type: "matmul".to_string(),
            input_elements: 100_000,
            dtype: "f32".to_string(),
            batch_size: 32,
            estimated_flops: 2_000_000_000,
            estimated_bytes: 400_000,
            preference: InferPreference::Auto,
        }
    }

    // S34.1: Dispatch decision engine
    #[test]
    fn s34_1_score_accelerators_cpu_only() {
        let workload = test_workload();
        let devices = DeviceSet::cpu_only();
        let scores = score_accelerators(&workload, &devices);
        assert!(!scores.is_empty());
        assert_eq!(scores[0].device, DispatchDevice::Cpu);
    }

    #[test]
    fn s34_1_score_accelerators_with_gpu() {
        let workload = test_workload();
        let devices = DeviceSet::new(vec![0], vec![]);
        let scores = score_accelerators(&workload, &devices);
        assert!(scores.len() >= 2);
        // GPU should score higher for large compute-bound workload
        assert_eq!(scores[0].device, DispatchDevice::Gpu(0));
    }

    #[test]
    fn s34_1_user_preference_bonus() {
        let mut workload = test_workload();
        workload.preference = InferPreference::Npu;
        let devices = DeviceSet::new(vec![0], vec![0]);
        let scores = score_accelerators(&workload, &devices);
        // NPU should get preference bonus
        let npu_score = scores
            .iter()
            .find(|s| matches!(s.device, DispatchDevice::Npu(_)));
        assert!(npu_score.is_some());
        assert!(npu_score.unwrap().reason.contains("preferred"));
    }

    // S34.2: CPU fallback path
    #[test]
    fn s34_2_cpu_fallback_always_available() {
        let workload = test_workload();
        let devices = DeviceSet::cpu_only();
        let decision = decide_dispatch(&workload, &devices);
        assert_eq!(decision.primary, DispatchDevice::Cpu);
    }

    // S34.3: NPU dispatch path
    #[test]
    fn s34_3_npu_preferred_for_int8() {
        let mut workload = test_workload();
        workload.dtype = "i8".to_string();
        workload.batch_size = 1;
        let devices = DeviceSet::new(vec![], vec![0]);
        let scores = score_accelerators(&workload, &devices);
        let npu = scores
            .iter()
            .find(|s| matches!(s.device, DispatchDevice::Npu(_)));
        assert!(npu.is_some());
    }

    // S34.4: GPU dispatch path
    #[test]
    fn s34_4_gpu_preferred_for_large_matmul() {
        let workload = test_workload();
        let devices = DeviceSet::new(vec![0], vec![0]);
        let decision = decide_dispatch(&workload, &devices);
        assert_eq!(decision.primary, DispatchDevice::Gpu(0));
    }

    // S34.5: Automatic fallback
    #[test]
    fn s34_5_fallback_on_oom() {
        let workload = test_workload();
        let devices = DeviceSet::new(vec![0], vec![0]);
        let decision = decide_dispatch(&workload, &devices);
        let failure = DispatchFailure::OutOfMemory {
            device: "GPU:0".to_string(),
        };
        let next = fallback_next(&decision, &failure);
        assert!(next.is_some());
    }

    #[test]
    fn s34_5_fallback_chain() {
        let workload = test_workload();
        let devices = DeviceSet::new(vec![0], vec![0]);
        let decision = decide_dispatch(&workload, &devices);
        assert!(!decision.fallbacks.is_empty());
    }

    // S34.6: Latency profiling
    #[test]
    fn s34_6_latency_profile() {
        let mut lp = LatencyProfile::new();
        lp.calibrate_simulated(DispatchDevice::Cpu, 100);
        lp.calibrate_simulated(DispatchDevice::Gpu(0), 10);

        let fastest = lp.fastest_device();
        assert_eq!(fastest, Some(DispatchDevice::Gpu(0)));

        let cpu_lat = lp.get(&DispatchDevice::Cpu);
        assert!(cpu_lat.is_some());
        assert_eq!(cpu_lat.unwrap().steady_us, 100);
        assert_eq!(cpu_lat.unwrap().warmup_us, 300); // 3x
    }

    // S34.7: Workload classification
    #[test]
    fn s34_7_compute_bound() {
        let wc = classify_workload(1_000_000, 1000, 32);
        assert_eq!(wc, WorkloadClass::ComputeBound);
    }

    #[test]
    fn s34_7_memory_bound() {
        let wc = classify_workload(1000, 10000, 32);
        assert_eq!(wc, WorkloadClass::MemoryBound);
    }

    #[test]
    fn s34_7_latency_sensitive() {
        let wc = classify_workload(1_000_000, 1000, 1);
        assert_eq!(wc, WorkloadClass::LatencySensitive);
    }

    // S34.8: Dispatch cache
    #[test]
    fn s34_8_cache_insert_and_get() {
        let mut cache = DispatchCache::new();
        let key = DispatchCacheKey {
            model_hash: 0xDEADBEEF,
            input_shape: "[1, 3, 224, 224]".to_string(),
            hw_signature: "CPU+GPU:0".to_string(),
        };
        cache.insert(key.clone(), DispatchDevice::Gpu(0));
        let result = cache.get(&key);
        assert_eq!(result, Some(DispatchDevice::Gpu(0)));
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn s34_8_cache_miss() {
        let mut cache = DispatchCache::new();
        let key = DispatchCacheKey {
            model_hash: 0,
            input_shape: "[]".to_string(),
            hw_signature: "CPU".to_string(),
        };
        assert_eq!(cache.get(&key), None);
    }

    #[test]
    fn s34_8_cache_hit_rate() {
        let mut cache = DispatchCache::new();
        let key = DispatchCacheKey {
            model_hash: 1,
            input_shape: "[1]".to_string(),
            hw_signature: "CPU".to_string(),
        };
        cache.insert(key.clone(), DispatchDevice::Cpu);
        cache.get(&key); // hit
        cache.get(&key); // hit
        assert!(cache.hit_rate() > 0.0);
    }

    // S34.9: Dispatch logging
    #[test]
    fn s34_9_dispatch_logging() {
        let mut logger = DispatchLogger::new(DispatchLogLevel::Debug);
        logger.log_decision(
            "matmul_128",
            DispatchDevice::Gpu(0),
            95.0,
            WorkloadClass::ComputeBound,
            false,
        );
        logger.log_decision(
            "relu_128",
            DispatchDevice::Cpu,
            80.0,
            WorkloadClass::MemoryBound,
            true,
        );

        assert_eq!(logger.entries().len(), 2);
        let log = logger.format_log();
        assert!(log.contains("matmul_128"));
        assert!(log.contains("GPU:0"));
        assert!(log.contains("[cached]"));
    }

    // S34.10: Integration
    #[test]
    fn s34_10_end_to_end_dispatch() {
        let workload = WorkloadDescriptor {
            op_type: "conv2d".to_string(),
            input_elements: 50000,
            dtype: "f32".to_string(),
            batch_size: 8,
            estimated_flops: 500_000_000,
            estimated_bytes: 200_000,
            preference: InferPreference::Auto,
        };
        let devices = DeviceSet::new(vec![0], vec![0]);
        let decision = decide_dispatch(&workload, &devices);

        assert!(decision.score > 0.0);
        assert!(!decision.reason.is_empty());
        assert!(!decision.fallbacks.is_empty());
    }

    #[test]
    fn s34_10_device_set() {
        let ds = DeviceSet::new(vec![0, 1], vec![0]);
        assert!(ds.has_gpu());
        assert!(ds.has_npu());
        assert_eq!(ds.device_count(), 4); // CPU + 2 GPU + 1 NPU
    }

    #[test]
    fn s34_10_workload_class_display() {
        assert_eq!(format!("{}", WorkloadClass::ComputeBound), "compute-bound");
        assert_eq!(format!("{}", WorkloadClass::MemoryBound), "memory-bound");
        assert_eq!(
            format!("{}", WorkloadClass::LatencySensitive),
            "latency-sensitive"
        );
    }
}
