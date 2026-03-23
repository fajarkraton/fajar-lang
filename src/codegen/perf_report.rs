//! Performance report generator for Fajar Lang.
//!
//! Collects benchmark results and generates a publish-ready markdown
//! report comparing measured performance against targets.

use super::benchmarks::BenchmarkSuite;

/// Performance targets for Fajar Lang.
#[derive(Debug, Clone)]
pub struct PerformanceTargets {
    /// JSON parse 1MB target (milliseconds).
    pub json_parse_ms: f64,
    /// Compilation speed target for 10K LOC (seconds).
    pub compile_10k_loc_s: f64,
    /// Incremental rebuild target (milliseconds).
    pub incremental_rebuild_ms: f64,
    /// Hello world binary size target (bytes).
    pub binary_size_bytes: usize,
    /// Cold start target (milliseconds).
    pub cold_start_ms: f64,
    /// Matrix multiply 1000x1000 target vs BLAS factor.
    pub matrix_vs_blas_factor: f64,
}

impl PerformanceTargets {
    /// Default targets from WORLD_CLASS_PLAN.
    pub fn default_targets() -> Self {
        Self {
            json_parse_ms: 10.0,
            compile_10k_loc_s: 3.0,
            incremental_rebuild_ms: 500.0,
            binary_size_bytes: 100_000, // 100KB
            cold_start_ms: 5.0,
            matrix_vs_blas_factor: 2.0,
        }
    }
}

impl Default for PerformanceTargets {
    fn default() -> Self {
        Self::default_targets()
    }
}

/// A measured performance metric with pass/fail against target.
#[derive(Debug, Clone)]
pub struct PerfMetric {
    /// Metric name.
    pub name: String,
    /// Measured value.
    pub measured: f64,
    /// Target value.
    pub target: f64,
    /// Unit (e.g., "ms", "KB", "s").
    pub unit: String,
    /// Whether lower is better.
    pub lower_is_better: bool,
}

impl PerfMetric {
    /// Creates a new metric (lower is better).
    pub fn new(name: &str, measured: f64, target: f64, unit: &str) -> Self {
        Self {
            name: name.to_string(),
            measured,
            target,
            unit: unit.to_string(),
            lower_is_better: true,
        }
    }

    /// Whether this metric passes the target.
    pub fn passes(&self) -> bool {
        if self.lower_is_better {
            self.measured <= self.target
        } else {
            self.measured >= self.target
        }
    }

    /// Returns a pass/fail symbol.
    pub fn status_symbol(&self) -> &str {
        if self.passes() { "PASS" } else { "FAIL" }
    }

    /// Returns the ratio (measured / target).
    pub fn ratio(&self) -> f64 {
        if self.target == 0.0 {
            return 0.0;
        }
        self.measured / self.target
    }
}

/// A complete performance report.
#[derive(Debug, Clone)]
pub struct PerformanceReport {
    /// Report title.
    pub title: String,
    /// Individual metrics.
    pub metrics: Vec<PerfMetric>,
    /// Benchmark suite results (optional).
    pub suite: Option<BenchmarkSuite>,
    /// Generation timestamp.
    pub timestamp: String,
}

impl PerformanceReport {
    /// Creates a new empty report.
    pub fn new(title: &str) -> Self {
        Self {
            title: title.to_string(),
            metrics: Vec::new(),
            suite: None,
            timestamp: "2026-03-23".to_string(),
        }
    }

    /// Adds a metric to the report.
    pub fn add_metric(&mut self, metric: PerfMetric) {
        self.metrics.push(metric);
    }

    /// Attaches a benchmark suite.
    pub fn with_suite(mut self, suite: BenchmarkSuite) -> Self {
        self.suite = Some(suite);
        self
    }

    /// Returns the number of passing metrics.
    pub fn pass_count(&self) -> usize {
        self.metrics.iter().filter(|m| m.passes()).count()
    }

    /// Returns the total number of metrics.
    pub fn total_count(&self) -> usize {
        self.metrics.len()
    }

    /// Returns the overall pass rate as a percentage.
    pub fn pass_rate(&self) -> f64 {
        if self.metrics.is_empty() {
            return 100.0;
        }
        (self.pass_count() as f64 / self.total_count() as f64) * 100.0
    }

    /// Generates a markdown report.
    pub fn to_markdown(&self) -> String {
        let mut md = String::new();

        md.push_str(&format!("# {}\n\n", self.title));
        md.push_str(&format!(
            "Generated: {} | Pass rate: {}/{} ({:.0}%)\n\n",
            self.timestamp,
            self.pass_count(),
            self.total_count(),
            self.pass_rate(),
        ));

        // Metrics table
        md.push_str("## Performance Metrics\n\n");
        md.push_str("| Metric | Measured | Target | Status | Ratio |\n");
        md.push_str("|--------|----------|--------|--------|-------|\n");
        for m in &self.metrics {
            md.push_str(&format!(
                "| {} | {:.2} {} | {:.2} {} | {} | {:.2}x |\n",
                m.name,
                m.measured,
                m.unit,
                m.target,
                m.unit,
                m.status_symbol(),
                m.ratio(),
            ));
        }
        md.push('\n');

        // Benchmark suite results
        if let Some(ref suite) = self.suite {
            md.push_str("## Benchmark Suite\n\n");
            md.push_str(&format!(
                "Suite: {} ({} benchmarks, {:.2}ms total)\n\n",
                suite.name,
                suite.count(),
                suite.total_duration.as_millis(),
            ));
            md.push_str("| Benchmark | Time (ms) | Iterations | Iter/s |\n");
            md.push_str("|-----------|-----------|------------|--------|\n");
            for r in &suite.results {
                md.push_str(&format!(
                    "| {} | {:.2} | {} | {:.0} |\n",
                    r.name,
                    r.ms_per_iter(),
                    r.iterations,
                    r.iter_per_sec(),
                ));
            }
            md.push('\n');
        }

        // Summary
        md.push_str("## Summary\n\n");
        if self.pass_rate() >= 100.0 {
            md.push_str("All performance targets met.\n");
        } else {
            let failing: Vec<&PerfMetric> = self.metrics.iter().filter(|m| !m.passes()).collect();
            md.push_str(&format!("{} metric(s) below target:\n\n", failing.len()));
            for m in failing {
                md.push_str(&format!(
                    "- **{}**: {:.2} {} (target: {:.2} {})\n",
                    m.name, m.measured, m.unit, m.target, m.unit,
                ));
            }
        }

        md
    }
}

/// Generates a performance report from measured data.
pub fn generate_report(
    compile_speed_ms: f64,
    cold_start_ms: f64,
    binary_size_kb: f64,
) -> PerformanceReport {
    let targets = PerformanceTargets::default_targets();
    let mut report = PerformanceReport::new("Fajar Lang Performance Report");

    report.add_metric(PerfMetric::new(
        "Compilation Speed (10K LOC)",
        compile_speed_ms / 1000.0,
        targets.compile_10k_loc_s,
        "s",
    ));
    report.add_metric(PerfMetric::new(
        "Cold Start",
        cold_start_ms,
        targets.cold_start_ms,
        "ms",
    ));
    report.add_metric(PerfMetric::new(
        "Binary Size (hello world)",
        binary_size_kb,
        targets.binary_size_bytes as f64 / 1024.0,
        "KB",
    ));

    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codegen::benchmarks::BenchmarkResult;
    use std::time::Duration;

    #[test]
    fn perf_metric_pass() {
        let m = PerfMetric::new("test", 5.0, 10.0, "ms");
        assert!(m.passes());
        assert_eq!(m.status_symbol(), "PASS");
    }

    #[test]
    fn perf_metric_fail() {
        let m = PerfMetric::new("test", 15.0, 10.0, "ms");
        assert!(!m.passes());
        assert_eq!(m.status_symbol(), "FAIL");
    }

    #[test]
    fn perf_metric_ratio() {
        let m = PerfMetric::new("test", 5.0, 10.0, "ms");
        assert!((m.ratio() - 0.5).abs() < 0.01);
    }

    #[test]
    fn report_pass_rate() {
        let mut report = PerformanceReport::new("test");
        report.add_metric(PerfMetric::new("a", 5.0, 10.0, "ms"));
        report.add_metric(PerfMetric::new("b", 15.0, 10.0, "ms"));
        assert_eq!(report.pass_count(), 1);
        assert_eq!(report.total_count(), 2);
        assert!((report.pass_rate() - 50.0).abs() < 0.1);
    }

    #[test]
    fn report_all_pass() {
        let mut report = PerformanceReport::new("test");
        report.add_metric(PerfMetric::new("a", 1.0, 10.0, "ms"));
        report.add_metric(PerfMetric::new("b", 2.0, 10.0, "ms"));
        assert_eq!(report.pass_rate(), 100.0);
    }

    #[test]
    fn report_markdown_format() {
        let mut report = PerformanceReport::new("Fajar Perf");
        report.add_metric(PerfMetric::new("Cold Start", 3.0, 5.0, "ms"));
        let md = report.to_markdown();
        assert!(md.contains("# Fajar Perf"));
        assert!(md.contains("Cold Start"));
        assert!(md.contains("PASS"));
        assert!(md.contains("Performance Metrics"));
    }

    #[test]
    fn report_markdown_failing() {
        let mut report = PerformanceReport::new("test");
        report.add_metric(PerfMetric::new("Slow Thing", 20.0, 5.0, "ms"));
        let md = report.to_markdown();
        assert!(md.contains("FAIL"));
        assert!(md.contains("below target"));
        assert!(md.contains("Slow Thing"));
    }

    #[test]
    fn generate_report_fn() {
        let report = generate_report(2000.0, 3.0, 80.0);
        assert_eq!(report.total_count(), 3);
        // compile_speed: 2.0s < 3.0s target → pass
        // cold_start: 3.0ms < 5.0ms target → pass
        // binary_size: 80KB < ~97KB target → pass
        assert!(report.pass_rate() > 50.0);
    }

    #[test]
    fn default_targets() {
        let t = PerformanceTargets::default_targets();
        assert_eq!(t.cold_start_ms, 5.0);
        assert_eq!(t.binary_size_bytes, 100_000);
        assert_eq!(t.compile_10k_loc_s, 3.0);
    }

    #[test]
    fn report_with_suite() {
        let mut suite = BenchmarkSuite::new("game");
        suite.add(BenchmarkResult::new("fib", Duration::from_millis(10), 100));
        let report = PerformanceReport::new("test").with_suite(suite);
        let md = report.to_markdown();
        assert!(md.contains("Benchmark Suite"));
        assert!(md.contains("fib"));
    }
}
