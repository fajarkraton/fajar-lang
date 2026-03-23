//! Benchmark framework for Fajar Lang performance measurement.
//!
//! Provides a structured approach to measuring and comparing
//! compilation speed, execution speed, and memory usage.

use std::time::{Duration, Instant};

/// A single benchmark result.
#[derive(Debug, Clone)]
pub struct BenchmarkResult {
    /// Benchmark name.
    pub name: String,
    /// Execution time.
    pub duration: Duration,
    /// Number of iterations.
    pub iterations: u64,
    /// Optional output value for correctness checking.
    pub output: Option<String>,
    /// Memory usage in bytes (estimated).
    pub memory_bytes: usize,
}

impl BenchmarkResult {
    /// Creates a new benchmark result.
    pub fn new(name: &str, duration: Duration, iterations: u64) -> Self {
        Self {
            name: name.to_string(),
            duration,
            iterations,
            output: None,
            memory_bytes: 0,
        }
    }

    /// Returns time per iteration in microseconds.
    pub fn us_per_iter(&self) -> f64 {
        if self.iterations == 0 {
            return 0.0;
        }
        self.duration.as_micros() as f64 / self.iterations as f64
    }

    /// Returns time per iteration in milliseconds.
    pub fn ms_per_iter(&self) -> f64 {
        if self.iterations == 0 {
            return 0.0;
        }
        self.duration.as_millis() as f64 / self.iterations as f64
    }

    /// Returns iterations per second.
    pub fn iter_per_sec(&self) -> f64 {
        if self.duration.as_nanos() == 0 {
            return 0.0;
        }
        self.iterations as f64 / self.duration.as_secs_f64()
    }
}

impl std::fmt::Display for BenchmarkResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}: {:.2}ms ({} iters, {:.2} iter/s)",
            self.name,
            self.ms_per_iter(),
            self.iterations,
            self.iter_per_sec(),
        )
    }
}

/// Comparison between two benchmark runs.
#[derive(Debug, Clone)]
pub struct BenchmarkComparison {
    /// Benchmark name.
    pub name: String,
    /// Baseline result.
    pub baseline: BenchmarkResult,
    /// Current result.
    pub current: BenchmarkResult,
    /// Speedup factor (baseline_time / current_time).
    pub speedup: f64,
}

impl BenchmarkComparison {
    /// Creates a comparison between baseline and current.
    pub fn compare(baseline: BenchmarkResult, current: BenchmarkResult) -> Self {
        let speedup = if current.us_per_iter() > 0.0 {
            baseline.us_per_iter() / current.us_per_iter()
        } else {
            0.0
        };
        Self {
            name: baseline.name.clone(),
            baseline,
            current,
            speedup,
        }
    }

    /// Returns true if current is faster than baseline.
    pub fn is_improvement(&self) -> bool {
        self.speedup > 1.0
    }

    /// Returns true if current is significantly slower (>10% regression).
    pub fn is_regression(&self) -> bool {
        self.speedup < 0.9
    }
}

impl std::fmt::Display for BenchmarkComparison {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let symbol = if self.speedup > 1.1 {
            "faster"
        } else if self.speedup < 0.9 {
            "SLOWER"
        } else {
            "~same"
        };
        write!(
            f,
            "{}: {:.2}ms → {:.2}ms ({:.2}x {symbol})",
            self.name,
            self.baseline.ms_per_iter(),
            self.current.ms_per_iter(),
            self.speedup,
        )
    }
}

/// A suite of benchmarks.
#[derive(Debug, Clone)]
pub struct BenchmarkSuite {
    /// Suite name.
    pub name: String,
    /// Individual benchmark results.
    pub results: Vec<BenchmarkResult>,
    /// Total suite duration.
    pub total_duration: Duration,
}

impl BenchmarkSuite {
    /// Creates a new empty suite.
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            results: Vec::new(),
            total_duration: Duration::ZERO,
        }
    }

    /// Adds a benchmark result.
    pub fn add(&mut self, result: BenchmarkResult) {
        self.total_duration += result.duration;
        self.results.push(result);
    }

    /// Returns the number of benchmarks.
    pub fn count(&self) -> usize {
        self.results.len()
    }

    /// Returns a summary report.
    pub fn summary(&self) -> String {
        let mut report = format!(
            "Benchmark Suite: {} ({} benchmarks)\n",
            self.name,
            self.count()
        );
        report.push_str(&format!(
            "Total time: {:.2}ms\n\n",
            self.total_duration.as_millis()
        ));
        for result in &self.results {
            report.push_str(&format!("  {result}\n"));
        }
        report
    }
}

/// Measures the execution time of a function.
pub fn measure<F, R>(name: &str, iterations: u64, f: F) -> BenchmarkResult
where
    F: Fn() -> R,
{
    let start = Instant::now();
    for _ in 0..iterations {
        std::hint::black_box(f());
    }
    let duration = start.elapsed();
    BenchmarkResult::new(name, duration, iterations)
}

/// Measures compilation speed of a source file.
pub fn measure_compile_speed(source: &str) -> BenchmarkResult {
    let start = Instant::now();
    let tokens = crate::lexer::tokenize(source);
    let lex_done = start.elapsed();

    if let Ok(tokens) = tokens {
        let _program = crate::parser::parse(tokens);
    }
    let total = start.elapsed();

    let mut result = BenchmarkResult::new("compile_speed", total, 1);
    result.output = Some(format!(
        "lex: {:.2}ms, parse+analyze: {:.2}ms",
        lex_done.as_millis(),
        (total - lex_done).as_millis()
    ));
    result.memory_bytes = source.len();
    result
}

/// Benchmarks Game program list.
pub const BENCHMARK_PROGRAMS: &[(&str, &str)] = &[
    ("nbody", "examples/bench_nbody.fj"),
    ("fannkuch", "examples/bench_fannkuch.fj"),
    ("spectral-norm", "examples/bench_spectral_norm.fj"),
    ("mandelbrot", "examples/bench_mandelbrot.fj"),
    ("binary-trees", "examples/bench_binary_trees.fj"),
    ("fasta", "examples/bench_fasta.fj"),
    ("matrix-multiply", "examples/bench_matrix_multiply.fj"),
    ("fibonacci", "examples/fibonacci.fj"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn benchmark_result_display() {
        let r = BenchmarkResult::new("test", Duration::from_millis(100), 10);
        let s = format!("{r}");
        assert!(s.contains("test"));
        assert!(s.contains("10.00ms"));
    }

    #[test]
    fn benchmark_result_per_iter() {
        let r = BenchmarkResult::new("test", Duration::from_millis(100), 10);
        assert!((r.ms_per_iter() - 10.0).abs() < 0.1);
    }

    #[test]
    fn benchmark_result_iter_per_sec() {
        let r = BenchmarkResult::new("test", Duration::from_secs(1), 100);
        assert!((r.iter_per_sec() - 100.0).abs() < 1.0);
    }

    #[test]
    fn benchmark_comparison_improvement() {
        let baseline = BenchmarkResult::new("test", Duration::from_millis(200), 1);
        let current = BenchmarkResult::new("test", Duration::from_millis(100), 1);
        let cmp = BenchmarkComparison::compare(baseline, current);
        assert!(cmp.is_improvement());
        assert!(!cmp.is_regression());
        assert!((cmp.speedup - 2.0).abs() < 0.1);
    }

    #[test]
    fn benchmark_comparison_regression() {
        let baseline = BenchmarkResult::new("test", Duration::from_millis(100), 1);
        let current = BenchmarkResult::new("test", Duration::from_millis(200), 1);
        let cmp = BenchmarkComparison::compare(baseline, current);
        assert!(cmp.is_regression());
    }

    #[test]
    fn benchmark_suite_summary() {
        let mut suite = BenchmarkSuite::new("test_suite");
        suite.add(BenchmarkResult::new("a", Duration::from_millis(10), 1));
        suite.add(BenchmarkResult::new("b", Duration::from_millis(20), 1));
        assert_eq!(suite.count(), 2);
        let summary = suite.summary();
        assert!(summary.contains("test_suite"));
        assert!(summary.contains("2 benchmarks"));
    }

    #[test]
    fn benchmark_programs_listed() {
        assert!(BENCHMARK_PROGRAMS.len() >= 7);
        let names: Vec<&str> = BENCHMARK_PROGRAMS.iter().map(|(n, _)| *n).collect();
        assert!(names.contains(&"nbody"));
        assert!(names.contains(&"mandelbrot"));
        assert!(names.contains(&"binary-trees"));
    }

    #[test]
    fn measure_fn() {
        let result = measure("add", 1000, || 2 + 2);
        assert_eq!(result.iterations, 1000);
        assert!(result.duration.as_nanos() > 0);
    }

    #[test]
    fn compile_speed_measurement() {
        let source = "fn main() { 42 }";
        let result = measure_compile_speed(source);
        assert!(result.duration.as_nanos() > 0);
        assert_eq!(result.memory_bytes, source.len());
    }
}
