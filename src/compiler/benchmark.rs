//! # Benchmark Suite
//!
//! Provides benchmark infrastructure for measuring feature overhead,
//! detecting performance regressions, and generating benchmark reports
//! for the Fajar Lang compiler and runtime.
//!
//! ## Architecture
//!
//! ```text
//! BenchmarkSuite → run_benchmark() → BenchmarkResult
//!                → measure_overhead() → OverheadResult
//!                → detect_regression() → Option<Regression>
//!                → generate_benchmark_report()
//! ```

use std::time::Instant;

use thiserror::Error;

// ═══════════════════════════════════════════════════════════════════════
// Error types
// ═══════════════════════════════════════════════════════════════════════

/// Errors arising from benchmark operations.
#[derive(Debug, Error)]
pub enum BenchmarkError {
    /// A benchmark failed to produce valid results.
    #[error("benchmark `{name}` failed: {reason}")]
    RunFailed {
        /// The benchmark name.
        name: String,
        /// The failure reason.
        reason: String,
    },

    /// The benchmark suite configuration is invalid.
    #[error("invalid benchmark configuration: {message}")]
    InvalidConfig {
        /// Description of the configuration error.
        message: String,
    },
}

// ═══════════════════════════════════════════════════════════════════════
// BenchmarkCategory
// ═══════════════════════════════════════════════════════════════════════

/// The category of a benchmark, corresponding to a compiler or runtime feature area.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BenchmarkCategory {
    /// Effect system benchmarks.
    Effects,
    /// Compile-time evaluation benchmarks.
    Comptime,
    /// SIMD / vectorization benchmarks.
    Simd,
    /// Security feature overhead benchmarks.
    Security,
    /// Async I/O benchmarks.
    AsyncIo,
    /// Macro expansion benchmarks.
    Macros,
    /// Full compilation pipeline benchmarks.
    Compilation,
}

impl std::fmt::Display for BenchmarkCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BenchmarkCategory::Effects => write!(f, "effects"),
            BenchmarkCategory::Comptime => write!(f, "comptime"),
            BenchmarkCategory::Simd => write!(f, "simd"),
            BenchmarkCategory::Security => write!(f, "security"),
            BenchmarkCategory::AsyncIo => write!(f, "async-io"),
            BenchmarkCategory::Macros => write!(f, "macros"),
            BenchmarkCategory::Compilation => write!(f, "compilation"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Benchmark / BenchmarkSuite
// ═══════════════════════════════════════════════════════════════════════

/// A single benchmark definition.
#[derive(Debug, Clone)]
pub struct Benchmark {
    /// The benchmark name.
    pub name: String,
    /// The feature area this benchmark measures.
    pub category: BenchmarkCategory,
    /// Number of iterations to run.
    pub iterations: u64,
}

impl Benchmark {
    /// Creates a new benchmark definition.
    pub fn new(name: String, category: BenchmarkCategory, iterations: u64) -> Self {
        Self {
            name,
            category,
            iterations,
        }
    }
}

/// A collection of related benchmarks.
#[derive(Debug, Clone)]
pub struct BenchmarkSuite {
    /// The suite name.
    pub name: String,
    /// The benchmarks in this suite.
    pub benchmarks: Vec<Benchmark>,
}

impl BenchmarkSuite {
    /// Creates a new benchmark suite.
    pub fn new(name: String) -> Self {
        Self {
            name,
            benchmarks: Vec::new(),
        }
    }

    /// Adds a benchmark to the suite.
    pub fn add(&mut self, bench: Benchmark) {
        self.benchmarks.push(bench);
    }

    /// Returns the number of benchmarks in the suite.
    pub fn len(&self) -> usize {
        self.benchmarks.len()
    }

    /// Returns `true` if the suite has no benchmarks.
    pub fn is_empty(&self) -> bool {
        self.benchmarks.is_empty()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// BenchmarkResult
// ═══════════════════════════════════════════════════════════════════════

/// The result of running a single benchmark.
#[derive(Debug, Clone)]
pub struct BenchmarkResult {
    /// The benchmark name.
    pub name: String,
    /// Total iterations executed.
    pub iterations: u64,
    /// Total elapsed time in nanoseconds.
    pub total_ns: u64,
    /// Mean time per iteration in nanoseconds.
    pub mean_ns: u64,
    /// Minimum iteration time in nanoseconds.
    pub min_ns: u64,
    /// Maximum iteration time in nanoseconds.
    pub max_ns: u64,
    /// Standard deviation of iteration times in nanoseconds.
    pub std_dev_ns: u64,
}

impl BenchmarkResult {
    /// Creates a benchmark result from raw timing data.
    pub fn from_samples(name: String, samples: &[u64]) -> Self {
        let iterations = samples.len() as u64;
        let total_ns: u64 = samples.iter().sum();
        let mean_ns = if iterations > 0 {
            total_ns / iterations
        } else {
            0
        };
        let min_ns = samples.iter().copied().min().unwrap_or(0);
        let max_ns = samples.iter().copied().max().unwrap_or(0);
        let std_dev_ns = compute_std_dev(samples, mean_ns);

        Self {
            name,
            iterations,
            total_ns,
            mean_ns,
            min_ns,
            max_ns,
            std_dev_ns,
        }
    }
}

/// Computes the population standard deviation for a set of samples.
fn compute_std_dev(samples: &[u64], mean: u64) -> u64 {
    if samples.is_empty() {
        return 0;
    }
    let variance: f64 = samples
        .iter()
        .map(|&s| {
            let diff = s as f64 - mean as f64;
            diff * diff
        })
        .sum::<f64>()
        / samples.len() as f64;
    variance.sqrt() as u64
}

// ═══════════════════════════════════════════════════════════════════════
// run_benchmark
// ═══════════════════════════════════════════════════════════════════════

/// Runs a benchmark by executing the provided function `iterations` times,
/// collecting per-iteration timing samples.
///
/// Returns a [`BenchmarkResult`] with statistical summaries.
pub fn run_benchmark(bench: &Benchmark, f: impl Fn()) -> BenchmarkResult {
    let mut samples = Vec::with_capacity(bench.iterations as usize);

    for _ in 0..bench.iterations {
        let start = Instant::now();
        f();
        let elapsed = start.elapsed().as_nanos() as u64;
        samples.push(elapsed);
    }

    BenchmarkResult::from_samples(bench.name.clone(), &samples)
}

// ═══════════════════════════════════════════════════════════════════════
// Overhead measurement
// ═══════════════════════════════════════════════════════════════════════

/// The default overhead threshold (10%).
const DEFAULT_OVERHEAD_THRESHOLD_PCT: f64 = 10.0;

/// The result of measuring feature overhead.
#[derive(Debug, Clone)]
pub struct OverheadResult {
    /// Absolute overhead in nanoseconds.
    pub absolute_ns: u64,
    /// Relative overhead as a percentage of the baseline.
    pub relative_percent: f64,
    /// Whether the overhead is within the acceptable threshold (< 10%).
    pub acceptable: bool,
}

/// Measures the overhead of a feature by comparing baseline and with-feature timings.
///
/// Returns an [`OverheadResult`] with the absolute and relative overhead.
/// Overhead below 10% is considered acceptable.
pub fn measure_overhead(baseline_ns: u64, with_feature_ns: u64) -> OverheadResult {
    let absolute_ns = with_feature_ns.saturating_sub(baseline_ns);
    let relative_percent = if baseline_ns > 0 {
        (absolute_ns as f64 / baseline_ns as f64) * 100.0
    } else if with_feature_ns > 0 {
        f64::INFINITY
    } else {
        0.0
    };
    let acceptable = relative_percent < DEFAULT_OVERHEAD_THRESHOLD_PCT;

    OverheadResult {
        absolute_ns,
        relative_percent,
        acceptable,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Regression detection
// ═══════════════════════════════════════════════════════════════════════

/// A detected performance regression.
#[derive(Debug, Clone)]
pub struct Regression {
    /// The benchmark name.
    pub benchmark_name: String,
    /// The baseline mean time in nanoseconds.
    pub baseline_ns: u64,
    /// The current mean time in nanoseconds.
    pub current_ns: u64,
    /// The regression percentage above the threshold.
    pub regression_pct: f64,
    /// The threshold percentage that was exceeded.
    pub threshold_pct: f64,
}

impl std::fmt::Display for Regression {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "REGRESSION in `{}`: {:.1}% slower ({}ns -> {}ns, threshold: {:.1}%)",
            self.benchmark_name,
            self.regression_pct,
            self.baseline_ns,
            self.current_ns,
            self.threshold_pct,
        )
    }
}

/// Detects whether the current benchmark result is a regression
/// compared to the baseline, using the given percentage threshold.
///
/// Returns `Some(Regression)` if the current mean exceeds the baseline
/// by more than `threshold_pct` percent.
pub fn detect_regression(
    current: &BenchmarkResult,
    baseline: &BenchmarkResult,
    threshold_pct: f64,
) -> Option<Regression> {
    if baseline.mean_ns == 0 {
        return None;
    }
    let diff = current.mean_ns as f64 - baseline.mean_ns as f64;
    let regression_pct = (diff / baseline.mean_ns as f64) * 100.0;

    if regression_pct > threshold_pct {
        Some(Regression {
            benchmark_name: current.name.clone(),
            baseline_ns: baseline.mean_ns,
            current_ns: current.mean_ns,
            regression_pct,
            threshold_pct,
        })
    } else {
        None
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Benchmark report generation
// ═══════════════════════════════════════════════════════════════════════

/// Generates a human-readable benchmark report from a set of results.
///
/// Includes per-benchmark statistics and overall pass/fail summary.
pub fn generate_benchmark_report(results: &[BenchmarkResult]) -> String {
    let mut report = String::new();
    report.push_str("=== Benchmark Report ===\n\n");

    for result in results {
        append_result_line(&mut report, result);
    }

    append_report_summary(&mut report, results);
    report
}

/// Appends a single benchmark result line to the report.
fn append_result_line(report: &mut String, result: &BenchmarkResult) {
    report.push_str(&format!(
        "{}: mean={}ns, min={}ns, max={}ns, std_dev={}ns ({} iters)\n",
        result.name,
        result.mean_ns,
        result.min_ns,
        result.max_ns,
        result.std_dev_ns,
        result.iterations,
    ));
}

/// Appends the summary section to the benchmark report.
fn append_report_summary(report: &mut String, results: &[BenchmarkResult]) {
    report.push('\n');
    let total_iters: u64 = results.iter().map(|r| r.iterations).sum();
    let total_ns: u64 = results.iter().map(|r| r.total_ns).sum();
    let total_ms = total_ns / 1_000_000;
    report.push_str(&format!(
        "Total: {} benchmarks, {} iterations, {}ms elapsed\n",
        results.len(),
        total_iters,
        total_ms,
    ));

    let status = if results.is_empty() { "SKIP" } else { "PASS" };
    report.push_str(&format!("Status: {status}\n"));
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn s27_1_benchmark_suite_creation() {
        let mut suite = BenchmarkSuite::new("compiler".to_string());
        assert!(suite.is_empty());
        assert_eq!(suite.len(), 0);

        suite.add(Benchmark::new(
            "lex_speed".to_string(),
            BenchmarkCategory::Compilation,
            100,
        ));
        assert_eq!(suite.len(), 1);
        assert!(!suite.is_empty());
        assert_eq!(suite.name, "compiler");
        assert_eq!(suite.benchmarks[0].name, "lex_speed");
        assert_eq!(suite.benchmarks[0].category, BenchmarkCategory::Compilation);
        assert_eq!(suite.benchmarks[0].iterations, 100);
    }

    #[test]
    fn s27_2_benchmark_result_from_samples() {
        let samples = vec![100, 200, 300, 400, 500];
        let result = BenchmarkResult::from_samples("test_bench".to_string(), &samples);

        assert_eq!(result.name, "test_bench");
        assert_eq!(result.iterations, 5);
        assert_eq!(result.total_ns, 1500);
        assert_eq!(result.mean_ns, 300);
        assert_eq!(result.min_ns, 100);
        assert_eq!(result.max_ns, 500);
        // std_dev of [100,200,300,400,500] around mean 300 = sqrt(20000) ~ 141
        assert!(result.std_dev_ns > 100 && result.std_dev_ns < 200);
    }

    #[test]
    fn s27_3_benchmark_result_empty_samples() {
        let result = BenchmarkResult::from_samples("empty".to_string(), &[]);
        assert_eq!(result.iterations, 0);
        assert_eq!(result.total_ns, 0);
        assert_eq!(result.mean_ns, 0);
        assert_eq!(result.min_ns, 0);
        assert_eq!(result.max_ns, 0);
        assert_eq!(result.std_dev_ns, 0);
    }

    #[test]
    fn s27_4_run_benchmark_collects_timing() {
        let bench = Benchmark::new("noop_bench".to_string(), BenchmarkCategory::Effects, 10);
        let counter = std::cell::Cell::new(0u64);
        let result = run_benchmark(&bench, || {
            counter.set(counter.get() + 1);
        });

        assert_eq!(result.name, "noop_bench");
        assert_eq!(result.iterations, 10);
        assert_eq!(counter.get(), 10);
        // Each iteration should take some positive amount of time
        assert!(result.total_ns > 0);
        assert!(result.mean_ns > 0 || result.total_ns > 0);
    }

    #[test]
    fn s27_5_measure_overhead_acceptable() {
        // 5% overhead: 100ns baseline, 105ns with feature
        let result = measure_overhead(100, 105);
        assert_eq!(result.absolute_ns, 5);
        assert!((result.relative_percent - 5.0).abs() < 0.01);
        assert!(result.acceptable);
    }

    #[test]
    fn s27_6_measure_overhead_unacceptable() {
        // 20% overhead: 100ns baseline, 120ns with feature
        let result = measure_overhead(100, 120);
        assert_eq!(result.absolute_ns, 20);
        assert!((result.relative_percent - 20.0).abs() < 0.01);
        assert!(!result.acceptable);
    }

    #[test]
    fn s27_7_measure_overhead_zero_baseline() {
        // Zero baseline with nonzero feature
        let result = measure_overhead(0, 50);
        assert_eq!(result.absolute_ns, 50);
        assert!(result.relative_percent.is_infinite());
        assert!(!result.acceptable);

        // Both zero
        let result_zero = measure_overhead(0, 0);
        assert_eq!(result_zero.absolute_ns, 0);
        assert!((result_zero.relative_percent - 0.0).abs() < 0.01);
        assert!(result_zero.acceptable);
    }

    #[test]
    fn s27_8_detect_regression_found() {
        let baseline = BenchmarkResult::from_samples("bench".to_string(), &[100, 100, 100]);
        let current = BenchmarkResult::from_samples("bench".to_string(), &[130, 130, 130]);

        let regression = detect_regression(&current, &baseline, 10.0);
        assert!(regression.is_some());
        let reg = regression.unwrap();
        assert_eq!(reg.benchmark_name, "bench");
        assert_eq!(reg.baseline_ns, 100);
        assert_eq!(reg.current_ns, 130);
        assert!((reg.regression_pct - 30.0).abs() < 0.01);
        assert!((reg.threshold_pct - 10.0).abs() < 0.01);
        // Display works
        let display = format!("{}", reg);
        assert!(display.contains("REGRESSION"));
        assert!(display.contains("bench"));
    }

    #[test]
    fn s27_9_detect_regression_not_found() {
        let baseline = BenchmarkResult::from_samples("bench".to_string(), &[100, 100, 100]);
        let current = BenchmarkResult::from_samples("bench".to_string(), &[105, 105, 105]);

        // 5% increase, threshold is 10% -> no regression
        let regression = detect_regression(&current, &baseline, 10.0);
        assert!(regression.is_none());

        // Faster than baseline -> no regression
        let faster = BenchmarkResult::from_samples("bench".to_string(), &[80, 80, 80]);
        assert!(detect_regression(&faster, &baseline, 10.0).is_none());
    }

    #[test]
    fn s27_10_generate_benchmark_report() {
        let results = vec![
            BenchmarkResult::from_samples("lex_bench".to_string(), &[100, 200, 300]),
            BenchmarkResult::from_samples("parse_bench".to_string(), &[500, 600, 700]),
        ];
        let report = generate_benchmark_report(&results);

        assert!(report.contains("Benchmark Report"));
        assert!(report.contains("lex_bench"));
        assert!(report.contains("parse_bench"));
        assert!(report.contains("mean="));
        assert!(report.contains("min="));
        assert!(report.contains("max="));
        assert!(report.contains("std_dev="));
        assert!(report.contains("Total: 2 benchmarks"));
        assert!(report.contains("6 iterations"));
        assert!(report.contains("Status: PASS"));
    }
}
