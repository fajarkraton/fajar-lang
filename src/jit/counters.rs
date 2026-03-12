//! Execution counters — per-function call counts, hot function detection,
//! loop back-edge counting, sampling profiler, call graph, type profiling.

use std::collections::HashMap;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

// ═══════════════════════════════════════════════════════════════════════
// S13.1 / S13.2: Per-Function Counter
// ═══════════════════════════════════════════════════════════════════════

/// Metadata for a function being profiled.
#[derive(Debug)]
pub struct FunctionProfile {
    /// Function name.
    pub name: String,
    /// Total call count.
    pub call_count: AtomicU64,
    /// Current execution tier.
    pub tier: ExecutionTier,
    /// Loop back-edge counts keyed by loop ID.
    pub loop_counts: HashMap<u32, u64>,
    /// Observed argument types per parameter index.
    pub type_profile: HashMap<usize, Vec<TypeObservation>>,
}

impl FunctionProfile {
    /// Creates a new function profile.
    pub fn new(name: &str) -> Self {
        Self {
            name: name.into(),
            call_count: AtomicU64::new(0),
            tier: ExecutionTier::Interpreter,
            loop_counts: HashMap::new(),
            type_profile: HashMap::new(),
        }
    }

    /// Increments the call counter and returns the new count.
    pub fn increment(&self) -> u64 {
        self.call_count.fetch_add(1, Ordering::Relaxed) + 1
    }

    /// Returns the current call count.
    pub fn count(&self) -> u64 {
        self.call_count.load(Ordering::Relaxed)
    }
}

/// The current execution tier of a function.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ExecutionTier {
    /// Tree-walking interpreter.
    Interpreter,
    /// Bytecode VM.
    BytecodeVM,
    /// Baseline JIT (fast compile, no optimization).
    BaselineJIT,
    /// Optimizing JIT (slow compile, full optimization).
    OptimizingJIT,
}

impl fmt::Display for ExecutionTier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExecutionTier::Interpreter => write!(f, "interpreter"),
            ExecutionTier::BytecodeVM => write!(f, "bytecode-vm"),
            ExecutionTier::BaselineJIT => write!(f, "baseline-jit"),
            ExecutionTier::OptimizingJIT => write!(f, "optimizing-jit"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S13.3 / S13.4: Thresholds
// ═══════════════════════════════════════════════════════════════════════

/// Configurable thresholds for tier promotion.
#[derive(Debug, Clone)]
pub struct TierThresholds {
    /// Call count to trigger baseline JIT compilation.
    pub baseline_threshold: u64,
    /// Call count to trigger optimizing JIT compilation.
    pub optimizing_threshold: u64,
    /// Loop back-edge count for hot loop detection.
    pub loop_threshold: u64,
    /// OSR threshold (back-edges within single invocation).
    pub osr_threshold: u64,
}

impl Default for TierThresholds {
    fn default() -> Self {
        Self {
            baseline_threshold: 100,
            optimizing_threshold: 10_000,
            loop_threshold: 1_000,
            osr_threshold: 500,
        }
    }
}

/// Determines whether a function should be promoted to a higher tier.
pub fn check_promotion(
    profile: &FunctionProfile,
    thresholds: &TierThresholds,
) -> Option<ExecutionTier> {
    let count = profile.count();
    match profile.tier {
        ExecutionTier::Interpreter | ExecutionTier::BytecodeVM => {
            if count >= thresholds.optimizing_threshold {
                Some(ExecutionTier::OptimizingJIT)
            } else if count >= thresholds.baseline_threshold {
                Some(ExecutionTier::BaselineJIT)
            } else {
                None
            }
        }
        ExecutionTier::BaselineJIT => {
            if count >= thresholds.optimizing_threshold {
                Some(ExecutionTier::OptimizingJIT)
            } else {
                None
            }
        }
        ExecutionTier::OptimizingJIT => None,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S13.5: Loop Back-Edge Counter
// ═══════════════════════════════════════════════════════════════════════

/// Records a loop back-edge execution.
pub fn record_back_edge(profile: &mut FunctionProfile, loop_id: u32) -> u64 {
    let count = profile.loop_counts.entry(loop_id).or_insert(0);
    *count += 1;
    *count
}

/// Checks whether a loop is hot.
pub fn is_hot_loop(profile: &FunctionProfile, loop_id: u32, threshold: u64) -> bool {
    profile
        .loop_counts
        .get(&loop_id)
        .is_some_and(|c| *c >= threshold)
}

// ═══════════════════════════════════════════════════════════════════════
// S13.6: Sampling Profiler
// ═══════════════════════════════════════════════════════════════════════

/// A sampling profiler that records function activity.
#[derive(Debug, Clone, Default)]
pub struct SamplingProfiler {
    /// Function name -> sample count.
    pub samples: HashMap<String, u64>,
    /// Total samples taken.
    pub total_samples: u64,
    /// Sampling interval in microseconds.
    pub interval_us: u64,
}

impl SamplingProfiler {
    /// Creates a new profiler with the given interval.
    pub fn new(interval_us: u64) -> Self {
        Self {
            samples: HashMap::new(),
            total_samples: 0,
            interval_us,
        }
    }

    /// Records a sample for a function.
    pub fn record_sample(&mut self, function: &str) {
        *self.samples.entry(function.into()).or_insert(0) += 1;
        self.total_samples += 1;
    }

    /// Returns the hottest functions sorted by sample count.
    pub fn hottest(&self, top_n: usize) -> Vec<(&str, u64)> {
        let mut sorted: Vec<(&str, u64)> =
            self.samples.iter().map(|(k, v)| (k.as_str(), *v)).collect();
        sorted.sort_by_key(|x| std::cmp::Reverse(x.1));
        sorted.truncate(top_n);
        sorted
    }

    /// Returns the percentage of time spent in a function.
    pub fn percentage(&self, function: &str) -> f64 {
        if self.total_samples == 0 {
            return 0.0;
        }
        let count = self.samples.get(function).copied().unwrap_or(0);
        (count as f64 / self.total_samples as f64) * 100.0
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S13.7: Call Graph Recording
// ═══════════════════════════════════════════════════════════════════════

/// A call graph edge.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CallEdge {
    /// Caller function name.
    pub caller: String,
    /// Callee function name.
    pub callee: String,
}

/// Records caller-callee relationships for inlining decisions.
#[derive(Debug, Clone, Default)]
pub struct CallGraph {
    /// Edges with call counts.
    pub edges: HashMap<CallEdge, u64>,
}

impl CallGraph {
    /// Creates an empty call graph.
    pub fn new() -> Self {
        Self::default()
    }

    /// Records a call from caller to callee.
    pub fn record_call(&mut self, caller: &str, callee: &str) {
        let edge = CallEdge {
            caller: caller.into(),
            callee: callee.into(),
        };
        *self.edges.entry(edge).or_insert(0) += 1;
    }

    /// Returns callees of a function sorted by frequency.
    pub fn callees_of(&self, caller: &str) -> Vec<(&str, u64)> {
        let mut result: Vec<(&str, u64)> = self
            .edges
            .iter()
            .filter(|(e, _)| e.caller == caller)
            .map(|(e, c)| (e.callee.as_str(), *c))
            .collect();
        result.sort_by_key(|x| std::cmp::Reverse(x.1));
        result
    }

    /// Returns callers of a function sorted by frequency.
    pub fn callers_of(&self, callee: &str) -> Vec<(&str, u64)> {
        let mut result: Vec<(&str, u64)> = self
            .edges
            .iter()
            .filter(|(e, _)| e.callee == callee)
            .map(|(e, c)| (e.caller.as_str(), *c))
            .collect();
        result.sort_by_key(|x| std::cmp::Reverse(x.1));
        result
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S13.8: Type Profiling
// ═══════════════════════════════════════════════════════════════════════

/// An observed type at a call site.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TypeObservation {
    /// Integer type.
    Int,
    /// Float type.
    Float,
    /// Boolean type.
    Bool,
    /// String type.
    Str,
    /// Struct with name.
    Struct(String),
    /// Array type.
    Array,
    /// Other/unknown type.
    Other(String),
}

impl fmt::Display for TypeObservation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TypeObservation::Int => write!(f, "i64"),
            TypeObservation::Float => write!(f, "f64"),
            TypeObservation::Bool => write!(f, "bool"),
            TypeObservation::Str => write!(f, "str"),
            TypeObservation::Struct(name) => write!(f, "{name}"),
            TypeObservation::Array => write!(f, "array"),
            TypeObservation::Other(name) => write!(f, "{name}"),
        }
    }
}

/// Records a type observation for a function parameter.
pub fn record_type(profile: &mut FunctionProfile, param_index: usize, observed: TypeObservation) {
    profile
        .type_profile
        .entry(param_index)
        .or_default()
        .push(observed);
}

/// Returns whether a parameter is monomorphic (always the same type).
pub fn is_monomorphic(profile: &FunctionProfile, param_index: usize) -> bool {
    profile.type_profile.get(&param_index).is_none_or(|obs| {
        if obs.is_empty() {
            return true;
        }
        let first = &obs[0];
        obs.iter().all(|o| o == first)
    })
}

// ═══════════════════════════════════════════════════════════════════════
// S13.9: CLI Configuration
// ═══════════════════════════════════════════════════════════════════════

/// JIT configuration from CLI arguments.
#[derive(Debug, Clone)]
pub struct JitConfig {
    /// Thresholds.
    pub thresholds: TierThresholds,
    /// Whether JIT is enabled.
    pub enabled: bool,
    /// Whether to print compilation events.
    pub verbose: bool,
}

impl Default for JitConfig {
    fn default() -> Self {
        Self {
            thresholds: TierThresholds::default(),
            enabled: true,
            verbose: false,
        }
    }
}

impl JitConfig {
    /// Creates config from CLI-style arguments.
    pub fn from_args(
        jit_threshold: Option<u64>,
        opt_threshold: Option<u64>,
        enabled: bool,
    ) -> Self {
        let mut thresholds = TierThresholds::default();
        if let Some(t) = jit_threshold {
            thresholds.baseline_threshold = t;
        }
        if let Some(t) = opt_threshold {
            thresholds.optimizing_threshold = t;
        }
        Self {
            thresholds,
            enabled,
            verbose: false,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // S13.1 — Per-Function Counter
    #[test]
    fn s13_1_function_profile_creation() {
        let profile = FunctionProfile::new("factorial");
        assert_eq!(profile.name, "factorial");
        assert_eq!(profile.count(), 0);
        assert_eq!(profile.tier, ExecutionTier::Interpreter);
    }

    // S13.2 — Counter Increment
    #[test]
    fn s13_2_counter_increment() {
        let profile = FunctionProfile::new("foo");
        assert_eq!(profile.increment(), 1);
        assert_eq!(profile.increment(), 2);
        assert_eq!(profile.increment(), 3);
        assert_eq!(profile.count(), 3);
    }

    // S13.3 — Hot Function Threshold
    #[test]
    fn s13_3_baseline_promotion() {
        let profile = FunctionProfile::new("hot");
        for _ in 0..100 {
            profile.increment();
        }
        let thresholds = TierThresholds::default();
        let promotion = check_promotion(&profile, &thresholds);
        assert_eq!(promotion, Some(ExecutionTier::BaselineJIT));
    }

    #[test]
    fn s13_3_no_promotion_below_threshold() {
        let profile = FunctionProfile::new("cold");
        for _ in 0..50 {
            profile.increment();
        }
        let thresholds = TierThresholds::default();
        assert!(check_promotion(&profile, &thresholds).is_none());
    }

    // S13.4 — Super-Hot Threshold
    #[test]
    fn s13_4_optimizing_promotion() {
        let profile = FunctionProfile::new("super_hot");
        for _ in 0..10_000 {
            profile.increment();
        }
        let thresholds = TierThresholds::default();
        let promotion = check_promotion(&profile, &thresholds);
        assert_eq!(promotion, Some(ExecutionTier::OptimizingJIT));
    }

    #[test]
    fn s13_4_already_optimized() {
        let mut profile = FunctionProfile::new("done");
        profile.tier = ExecutionTier::OptimizingJIT;
        for _ in 0..100_000 {
            profile.increment();
        }
        assert!(check_promotion(&profile, &TierThresholds::default()).is_none());
    }

    // S13.5 — Loop Back-Edge Counter
    #[test]
    fn s13_5_back_edge_counting() {
        let mut profile = FunctionProfile::new("loop_fn");
        for _ in 0..500 {
            record_back_edge(&mut profile, 0);
        }
        assert!(is_hot_loop(&profile, 0, 500));
        assert!(!is_hot_loop(&profile, 0, 501));
        assert!(!is_hot_loop(&profile, 1, 1));
    }

    // S13.6 — Sampling Profiler
    #[test]
    fn s13_6_sampling_profiler() {
        let mut profiler = SamplingProfiler::new(1000);
        for _ in 0..80 {
            profiler.record_sample("hot_fn");
        }
        for _ in 0..20 {
            profiler.record_sample("cold_fn");
        }
        assert_eq!(profiler.total_samples, 100);
        assert!((profiler.percentage("hot_fn") - 80.0).abs() < 0.01);
        let top = profiler.hottest(1);
        assert_eq!(top[0].0, "hot_fn");
    }

    // S13.7 — Call Graph
    #[test]
    fn s13_7_call_graph() {
        let mut graph = CallGraph::new();
        graph.record_call("main", "foo");
        graph.record_call("main", "foo");
        graph.record_call("main", "bar");
        let callees = graph.callees_of("main");
        assert_eq!(callees[0].0, "foo");
        assert_eq!(callees[0].1, 2);
        let callers = graph.callers_of("foo");
        assert_eq!(callers[0].0, "main");
    }

    // S13.8 — Type Profiling
    #[test]
    fn s13_8_type_profiling() {
        let mut profile = FunctionProfile::new("generic");
        record_type(&mut profile, 0, TypeObservation::Int);
        record_type(&mut profile, 0, TypeObservation::Int);
        assert!(is_monomorphic(&profile, 0));
        record_type(&mut profile, 0, TypeObservation::Str);
        assert!(!is_monomorphic(&profile, 0));
    }

    #[test]
    fn s13_8_type_observation_display() {
        assert_eq!(TypeObservation::Int.to_string(), "i64");
        assert_eq!(TypeObservation::Float.to_string(), "f64");
        assert_eq!(TypeObservation::Struct("Point".into()).to_string(), "Point");
    }

    // S13.9 — CLI Config
    #[test]
    fn s13_9_config_from_args() {
        let config = JitConfig::from_args(Some(200), Some(5000), true);
        assert_eq!(config.thresholds.baseline_threshold, 200);
        assert_eq!(config.thresholds.optimizing_threshold, 5000);
        assert!(config.enabled);
    }

    #[test]
    fn s13_9_default_config() {
        let config = JitConfig::default();
        assert_eq!(config.thresholds.baseline_threshold, 100);
        assert_eq!(config.thresholds.optimizing_threshold, 10_000);
    }

    // S13.10 — Additional
    #[test]
    fn s13_10_tier_ordering() {
        assert!(ExecutionTier::Interpreter < ExecutionTier::BaselineJIT);
        assert!(ExecutionTier::BaselineJIT < ExecutionTier::OptimizingJIT);
    }

    #[test]
    fn s13_10_tier_display() {
        assert_eq!(ExecutionTier::Interpreter.to_string(), "interpreter");
        assert_eq!(ExecutionTier::BaselineJIT.to_string(), "baseline-jit");
        assert_eq!(ExecutionTier::OptimizingJIT.to_string(), "optimizing-jit");
    }
}
