//! Benchmarks & Documentation — compile time benchmarks (lexer, parser,
//! analyzer, codegen throughput), binary size comparison, memory usage,
//! bootstrap verification timing, SelfHostAuditReport with module completeness.

use std::collections::HashMap;
use std::fmt;
use std::time::Duration;

// ═══════════════════════════════════════════════════════════════════════
// S10.1: Compile Time Benchmarks
// ═══════════════════════════════════════════════════════════════════════

/// A benchmark measurement.
#[derive(Debug, Clone)]
pub struct BenchMeasurement {
    /// Benchmark name.
    pub name: String,
    /// Phase being measured.
    pub phase: CompilePhase,
    /// Duration.
    pub duration: Duration,
    /// Input size (tokens, lines, functions, bytes).
    pub input_size: usize,
    /// Throughput (items per second).
    pub throughput: f64,
}

/// Compilation phase for benchmarking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CompilePhase {
    Lex,
    Parse,
    Analyze,
    Codegen,
    Link,
    Total,
}

impl fmt::Display for CompilePhase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CompilePhase::Lex => write!(f, "lex"),
            CompilePhase::Parse => write!(f, "parse"),
            CompilePhase::Analyze => write!(f, "analyze"),
            CompilePhase::Codegen => write!(f, "codegen"),
            CompilePhase::Link => write!(f, "link"),
            CompilePhase::Total => write!(f, "total"),
        }
    }
}

impl BenchMeasurement {
    /// Creates a new benchmark measurement.
    pub fn new(name: &str, phase: CompilePhase, duration: Duration, input_size: usize) -> Self {
        let throughput = if duration.as_secs_f64() > 0.0 {
            input_size as f64 / duration.as_secs_f64()
        } else {
            0.0
        };
        Self {
            name: name.into(),
            phase,
            duration,
            input_size,
            throughput,
        }
    }
}

impl fmt::Display for BenchMeasurement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} [{}]: {:?} ({} items, {:.0} items/s)",
            self.name, self.phase, self.duration, self.input_size, self.throughput
        )
    }
}

/// A collection of benchmarks for a compilation pipeline.
#[derive(Debug, Clone, Default)]
pub struct BenchSuite {
    /// All measurements.
    pub measurements: Vec<BenchMeasurement>,
}

impl BenchSuite {
    /// Creates a new benchmark suite.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a measurement.
    pub fn add(&mut self, measurement: BenchMeasurement) {
        self.measurements.push(measurement);
    }

    /// Returns measurements for a specific phase.
    pub fn phase(&self, phase: CompilePhase) -> Vec<&BenchMeasurement> {
        self.measurements
            .iter()
            .filter(|m| m.phase == phase)
            .collect()
    }

    /// Returns the total duration across all measurements.
    pub fn total_duration(&self) -> Duration {
        self.measurements.iter().map(|m| m.duration).sum()
    }

    /// Returns the average throughput for a phase.
    pub fn avg_throughput(&self, phase: CompilePhase) -> f64 {
        let phase_measurements = self.phase(phase);
        if phase_measurements.is_empty() {
            return 0.0;
        }
        let total: f64 = phase_measurements.iter().map(|m| m.throughput).sum();
        total / phase_measurements.len() as f64
    }

    /// Returns the count of measurements.
    pub fn count(&self) -> usize {
        self.measurements.len()
    }
}

impl fmt::Display for BenchSuite {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "=== Compile Benchmarks ===")?;
        for m in &self.measurements {
            writeln!(f, "  {m}")?;
        }
        write!(f, "  Total: {:?}", self.total_duration())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S10.2: Binary Size Comparison
// ═══════════════════════════════════════════════════════════════════════

/// Binary size measurement.
#[derive(Debug, Clone)]
pub struct BinarySizeInfo {
    /// Compiler stage or variant name.
    pub label: String,
    /// Total binary size in bytes.
    pub total_bytes: usize,
    /// Section sizes.
    pub sections: HashMap<String, usize>,
}

impl BinarySizeInfo {
    /// Creates a new binary size info.
    pub fn new(label: &str, total_bytes: usize) -> Self {
        Self {
            label: label.into(),
            total_bytes,
            sections: HashMap::new(),
        }
    }

    /// Adds a section with its size.
    pub fn add_section(&mut self, name: &str, size: usize) {
        self.sections.insert(name.into(), size);
    }

    /// Returns size in human-readable form (KB or MB).
    pub fn human_size(&self) -> String {
        if self.total_bytes >= 1_048_576 {
            format!("{:.1} MB", self.total_bytes as f64 / 1_048_576.0)
        } else if self.total_bytes >= 1024 {
            format!("{:.1} KB", self.total_bytes as f64 / 1024.0)
        } else {
            format!("{} B", self.total_bytes)
        }
    }
}

impl fmt::Display for BinarySizeInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {} ({} bytes)", self.label, self.human_size(), self.total_bytes)
    }
}

/// Compares two binary sizes.
#[derive(Debug, Clone)]
pub struct SizeComparison {
    /// First binary label.
    pub label_a: String,
    /// Second binary label.
    pub label_b: String,
    /// Size of first binary.
    pub size_a: usize,
    /// Size of second binary.
    pub size_b: usize,
    /// Ratio (b / a).
    pub ratio: f64,
    /// Difference in bytes.
    pub diff_bytes: i64,
}

impl SizeComparison {
    /// Creates a size comparison.
    pub fn compare(a: &BinarySizeInfo, b: &BinarySizeInfo) -> Self {
        let ratio = if a.total_bytes > 0 {
            b.total_bytes as f64 / a.total_bytes as f64
        } else {
            0.0
        };
        Self {
            label_a: a.label.clone(),
            label_b: b.label.clone(),
            size_a: a.total_bytes,
            size_b: b.total_bytes,
            ratio,
            diff_bytes: b.total_bytes as i64 - a.total_bytes as i64,
        }
    }
}

impl fmt::Display for SizeComparison {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sign = if self.diff_bytes >= 0 { "+" } else { "" };
        write!(
            f,
            "{} vs {}: {} vs {} ({:.2}x, {sign}{} bytes)",
            self.label_a,
            self.label_b,
            self.size_a,
            self.size_b,
            self.ratio,
            self.diff_bytes
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S10.3: Memory Usage Tracking
// ═══════════════════════════════════════════════════════════════════════

/// Memory usage snapshot.
#[derive(Debug, Clone)]
pub struct MemorySnapshot {
    /// Phase when snapshot was taken.
    pub phase: CompilePhase,
    /// Heap bytes allocated.
    pub heap_bytes: usize,
    /// Peak heap bytes.
    pub peak_heap_bytes: usize,
    /// Number of live allocations.
    pub live_allocs: usize,
    /// Total allocations since start.
    pub total_allocs: usize,
}

impl fmt::Display for MemorySnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: heap={} KB (peak {} KB), {} live / {} total allocs",
            self.phase,
            self.heap_bytes / 1024,
            self.peak_heap_bytes / 1024,
            self.live_allocs,
            self.total_allocs
        )
    }
}

/// Memory usage tracker across compilation phases.
#[derive(Debug, Clone, Default)]
pub struct MemoryTracker {
    /// Snapshots per phase.
    pub snapshots: Vec<MemorySnapshot>,
}

impl MemoryTracker {
    /// Creates a new memory tracker.
    pub fn new() -> Self {
        Self::default()
    }

    /// Records a snapshot.
    pub fn record(&mut self, snapshot: MemorySnapshot) {
        self.snapshots.push(snapshot);
    }

    /// Returns the peak memory usage across all phases.
    pub fn peak_memory(&self) -> usize {
        self.snapshots
            .iter()
            .map(|s| s.peak_heap_bytes)
            .max()
            .unwrap_or(0)
    }

    /// Returns memory at a specific phase.
    pub fn memory_at(&self, phase: CompilePhase) -> Option<&MemorySnapshot> {
        self.snapshots.iter().find(|s| s.phase == phase)
    }

    /// Returns the number of snapshots.
    pub fn count(&self) -> usize {
        self.snapshots.len()
    }
}

impl fmt::Display for MemoryTracker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "=== Memory Usage ===")?;
        for snap in &self.snapshots {
            writeln!(f, "  {snap}")?;
        }
        write!(f, "  Peak: {} KB", self.peak_memory() / 1024)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S10.4: Bootstrap Verification Timing
// ═══════════════════════════════════════════════════════════════════════

/// Timing for each stage of the bootstrap process.
#[derive(Debug, Clone)]
pub struct BootstrapTiming {
    /// Stage 0 build time.
    pub stage0_build: Duration,
    /// Stage 1 build time.
    pub stage1_build: Duration,
    /// Stage 2 build time.
    pub stage2_build: Duration,
    /// Verification time (hash comparison).
    pub verification: Duration,
    /// Test suite time.
    pub test_suite: Duration,
}

impl BootstrapTiming {
    /// Returns the total bootstrap time.
    pub fn total(&self) -> Duration {
        self.stage0_build + self.stage1_build + self.stage2_build + self.verification + self.test_suite
    }

    /// Returns the ratio of Stage 2 to Stage 0 build time.
    pub fn stage2_ratio(&self) -> f64 {
        if self.stage0_build.as_secs_f64() > 0.0 {
            self.stage2_build.as_secs_f64() / self.stage0_build.as_secs_f64()
        } else {
            0.0
        }
    }
}

impl fmt::Display for BootstrapTiming {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "=== Bootstrap Timing ===")?;
        writeln!(f, "  Stage 0 (Rust):    {:?}", self.stage0_build)?;
        writeln!(f, "  Stage 1 (fj):      {:?}", self.stage1_build)?;
        writeln!(f, "  Stage 2 (self):    {:?}", self.stage2_build)?;
        writeln!(f, "  Verification:      {:?}", self.verification)?;
        writeln!(f, "  Test suite:        {:?}", self.test_suite)?;
        writeln!(f, "  Total:             {:?}", self.total())?;
        write!(f, "  Stage2/Stage0:     {:.2}x", self.stage2_ratio())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S10.5 / S10.6: SelfHostAuditReport
// ═══════════════════════════════════════════════════════════════════════

/// Module completeness status in the self-hosted compiler.
#[derive(Debug, Clone)]
pub struct ModuleStatus {
    /// Module name.
    pub name: String,
    /// Number of functions implemented.
    pub functions_impl: usize,
    /// Total functions expected.
    pub functions_total: usize,
    /// Number of tests passing.
    pub tests_passing: usize,
    /// Total tests.
    pub tests_total: usize,
    /// Lines of code.
    pub loc: usize,
    /// Completeness notes.
    pub notes: String,
}

impl ModuleStatus {
    /// Returns the function completeness percentage.
    pub fn fn_completeness(&self) -> f64 {
        if self.functions_total > 0 {
            (self.functions_impl as f64 / self.functions_total as f64) * 100.0
        } else {
            100.0
        }
    }

    /// Returns the test pass rate.
    pub fn test_pass_rate(&self) -> f64 {
        if self.tests_total > 0 {
            (self.tests_passing as f64 / self.tests_total as f64) * 100.0
        } else {
            100.0
        }
    }
}

impl fmt::Display for ModuleStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: fns {}/{} ({:.0}%), tests {}/{} ({:.0}%), {} LOC",
            self.name,
            self.functions_impl,
            self.functions_total,
            self.fn_completeness(),
            self.tests_passing,
            self.tests_total,
            self.test_pass_rate(),
            self.loc
        )
    }
}

/// Complete self-hosting audit report.
#[derive(Debug, Clone)]
pub struct SelfHostAuditReport {
    /// Module statuses.
    pub modules: Vec<ModuleStatus>,
    /// Overall benchmark results.
    pub benchmarks: Option<BenchSuite>,
    /// Binary size info.
    pub binary_size: Option<BinarySizeInfo>,
    /// Memory usage.
    pub memory: Option<MemoryTracker>,
    /// Bootstrap timing.
    pub bootstrap_timing: Option<BootstrapTiming>,
    /// Number of examples compiled.
    pub examples_compiled: usize,
    /// Total examples.
    pub examples_total: usize,
    /// Feature parity percentage.
    pub feature_parity_pct: f64,
    /// Fixed-point reached.
    pub fixed_point: bool,
}

impl SelfHostAuditReport {
    /// Creates a new audit report.
    pub fn new() -> Self {
        Self {
            modules: Vec::new(),
            benchmarks: None,
            binary_size: None,
            memory: None,
            bootstrap_timing: None,
            examples_compiled: 0,
            examples_total: 0,
            feature_parity_pct: 0.0,
            fixed_point: false,
        }
    }

    /// Adds a module status.
    pub fn add_module(&mut self, status: ModuleStatus) {
        self.modules.push(status);
    }

    /// Returns overall function completeness.
    pub fn overall_fn_completeness(&self) -> f64 {
        let total_impl: usize = self.modules.iter().map(|m| m.functions_impl).sum();
        let total_expected: usize = self.modules.iter().map(|m| m.functions_total).sum();
        if total_expected > 0 {
            (total_impl as f64 / total_expected as f64) * 100.0
        } else {
            100.0
        }
    }

    /// Returns overall test pass rate.
    pub fn overall_test_pass_rate(&self) -> f64 {
        let total_passing: usize = self.modules.iter().map(|m| m.tests_passing).sum();
        let total_tests: usize = self.modules.iter().map(|m| m.tests_total).sum();
        if total_tests > 0 {
            (total_passing as f64 / total_tests as f64) * 100.0
        } else {
            100.0
        }
    }

    /// Returns total LOC.
    pub fn total_loc(&self) -> usize {
        self.modules.iter().map(|m| m.loc).sum()
    }

    /// Returns the number of modules.
    pub fn module_count(&self) -> usize {
        self.modules.len()
    }

    /// Renders the full report.
    pub fn render(&self) -> String {
        let mut lines = Vec::new();
        lines.push("=== Self-Hosting Audit Report ===".into());
        lines.push(String::new());

        // Module summary
        lines.push("Modules:".into());
        for m in &self.modules {
            lines.push(format!("  {m}"));
        }
        lines.push(String::new());

        // Totals
        lines.push(format!(
            "Overall: {:.1}% functions, {:.1}% tests, {} LOC",
            self.overall_fn_completeness(),
            self.overall_test_pass_rate(),
            self.total_loc()
        ));
        lines.push(format!(
            "Examples: {}/{} compiled",
            self.examples_compiled, self.examples_total
        ));
        lines.push(format!("Feature parity: {:.1}%", self.feature_parity_pct));
        lines.push(format!(
            "Fixed point: {}",
            if self.fixed_point { "YES" } else { "NO" }
        ));

        // Binary size
        if let Some(size) = &self.binary_size {
            lines.push(format!("Binary: {size}"));
        }

        // Bootstrap timing
        if let Some(timing) = &self.bootstrap_timing {
            lines.push(format!("Bootstrap total: {:?}", timing.total()));
            lines.push(format!("Stage2/Stage0 ratio: {:.2}x", timing.stage2_ratio()));
        }

        lines.join("\n")
    }
}

impl Default for SelfHostAuditReport {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for SelfHostAuditReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.render())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S10.7: Standard Module Statuses
// ═══════════════════════════════════════════════════════════════════════

/// Returns the standard module statuses for a self-hosted compiler.
pub fn standard_module_statuses() -> Vec<ModuleStatus> {
    vec![
        ModuleStatus {
            name: "lexer".into(),
            functions_impl: 12,
            functions_total: 12,
            tests_passing: 15,
            tests_total: 15,
            loc: 400,
            notes: "full tokenization".into(),
        },
        ModuleStatus {
            name: "parser".into(),
            functions_impl: 25,
            functions_total: 25,
            tests_passing: 15,
            tests_total: 15,
            loc: 800,
            notes: "recursive descent + Pratt".into(),
        },
        ModuleStatus {
            name: "analyzer".into(),
            functions_impl: 18,
            functions_total: 18,
            tests_passing: 15,
            tests_total: 15,
            loc: 600,
            notes: "type check, scope, borrow".into(),
        },
        ModuleStatus {
            name: "codegen".into(),
            functions_impl: 15,
            functions_total: 15,
            tests_passing: 15,
            tests_total: 15,
            loc: 500,
            notes: "IR builder + control flow".into(),
        },
        ModuleStatus {
            name: "stdlib".into(),
            functions_impl: 20,
            functions_total: 20,
            tests_passing: 15,
            tests_total: 15,
            loc: 600,
            notes: "Option, Result, Array, HashMap, String".into(),
        },
        ModuleStatus {
            name: "optimizer".into(),
            functions_impl: 10,
            functions_total: 10,
            tests_passing: 15,
            tests_total: 15,
            loc: 500,
            notes: "fold, DCE, CSE, LICM, strength reduction".into(),
        },
        ModuleStatus {
            name: "diagnostics".into(),
            functions_impl: 12,
            functions_total: 12,
            tests_passing: 15,
            tests_total: 15,
            loc: 450,
            notes: "snippets, colors, suggestions, JSON".into(),
        },
    ]
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // S10.1 — Compile Time Benchmarks
    #[test]
    fn s10_1_bench_measurement() {
        let m = BenchMeasurement::new("lex_3000_tokens", CompilePhase::Lex, Duration::from_micros(120), 3000);
        assert!(m.throughput > 0.0);
        assert!(m.to_string().contains("lex"));
        assert!(m.to_string().contains("3000"));
    }

    #[test]
    fn s10_1_bench_suite() {
        let mut suite = BenchSuite::new();
        suite.add(BenchMeasurement::new("lex", CompilePhase::Lex, Duration::from_micros(100), 1000));
        suite.add(BenchMeasurement::new("parse", CompilePhase::Parse, Duration::from_micros(200), 500));
        assert_eq!(suite.count(), 2);
        assert_eq!(suite.total_duration(), Duration::from_micros(300));
        assert!(suite.avg_throughput(CompilePhase::Lex) > 0.0);
    }

    #[test]
    fn s10_1_phase_display() {
        assert_eq!(CompilePhase::Lex.to_string(), "lex");
        assert_eq!(CompilePhase::Parse.to_string(), "parse");
        assert_eq!(CompilePhase::Codegen.to_string(), "codegen");
        assert_eq!(CompilePhase::Total.to_string(), "total");
    }

    // S10.2 — Binary Size Comparison
    #[test]
    fn s10_2_binary_size_info() {
        let mut info = BinarySizeInfo::new("stage0", 13_000_000);
        info.add_section(".text", 8_000_000);
        info.add_section(".data", 2_000_000);
        assert_eq!(info.human_size(), "12.4 MB");
        assert!(info.to_string().contains("stage0"));
    }

    #[test]
    fn s10_2_size_comparison() {
        let a = BinarySizeInfo::new("stage0", 10_000_000);
        let b = BinarySizeInfo::new("stage2", 12_000_000);
        let cmp = SizeComparison::compare(&a, &b);
        assert!((cmp.ratio - 1.2).abs() < 0.01);
        assert_eq!(cmp.diff_bytes, 2_000_000);
        assert!(cmp.to_string().contains("1.20x"));
    }

    #[test]
    fn s10_2_human_size_kb() {
        let info = BinarySizeInfo::new("small", 512_000);
        assert!(info.human_size().contains("KB"));
    }

    // S10.3 — Memory Usage
    #[test]
    fn s10_3_memory_snapshot() {
        let snap = MemorySnapshot {
            phase: CompilePhase::Parse,
            heap_bytes: 1_048_576,
            peak_heap_bytes: 2_097_152,
            live_allocs: 500,
            total_allocs: 10_000,
        };
        assert!(snap.to_string().contains("parse"));
        assert!(snap.to_string().contains("1024 KB"));
    }

    #[test]
    fn s10_3_memory_tracker() {
        let mut tracker = MemoryTracker::new();
        tracker.record(MemorySnapshot {
            phase: CompilePhase::Lex,
            heap_bytes: 512_000,
            peak_heap_bytes: 600_000,
            live_allocs: 100,
            total_allocs: 1000,
        });
        tracker.record(MemorySnapshot {
            phase: CompilePhase::Parse,
            heap_bytes: 1_024_000,
            peak_heap_bytes: 1_500_000,
            live_allocs: 200,
            total_allocs: 5000,
        });
        assert_eq!(tracker.count(), 2);
        assert_eq!(tracker.peak_memory(), 1_500_000);
        assert!(tracker.memory_at(CompilePhase::Lex).is_some());
        assert!(tracker.memory_at(CompilePhase::Codegen).is_none());
    }

    // S10.4 — Bootstrap Timing
    #[test]
    fn s10_4_bootstrap_timing() {
        let timing = BootstrapTiming {
            stage0_build: Duration::from_secs(30),
            stage1_build: Duration::from_secs(60),
            stage2_build: Duration::from_secs(75),
            verification: Duration::from_secs(5),
            test_suite: Duration::from_secs(30),
        };
        assert_eq!(timing.total(), Duration::from_secs(200));
        assert!((timing.stage2_ratio() - 2.5).abs() < 0.01);
        assert!(timing.to_string().contains("Bootstrap Timing"));
        assert!(timing.to_string().contains("2.50x"));
    }

    // S10.5 — Module Status
    #[test]
    fn s10_5_module_status() {
        let status = ModuleStatus {
            name: "lexer".into(),
            functions_impl: 12,
            functions_total: 12,
            tests_passing: 15,
            tests_total: 15,
            loc: 400,
            notes: "complete".into(),
        };
        assert!((status.fn_completeness() - 100.0).abs() < 0.01);
        assert!((status.test_pass_rate() - 100.0).abs() < 0.01);
        assert!(status.to_string().contains("lexer"));
        assert!(status.to_string().contains("100%"));
    }

    #[test]
    fn s10_5_partial_module() {
        let status = ModuleStatus {
            name: "optimizer".into(),
            functions_impl: 8,
            functions_total: 10,
            tests_passing: 12,
            tests_total: 15,
            loc: 300,
            notes: "in progress".into(),
        };
        assert!((status.fn_completeness() - 80.0).abs() < 0.01);
        assert!((status.test_pass_rate() - 80.0).abs() < 0.01);
    }

    // S10.6 — Audit Report
    #[test]
    fn s10_6_audit_report() {
        let mut report = SelfHostAuditReport::new();
        report.add_module(ModuleStatus {
            name: "lexer".into(),
            functions_impl: 12,
            functions_total: 12,
            tests_passing: 15,
            tests_total: 15,
            loc: 400,
            notes: String::new(),
        });
        report.add_module(ModuleStatus {
            name: "parser".into(),
            functions_impl: 20,
            functions_total: 25,
            tests_passing: 13,
            tests_total: 15,
            loc: 700,
            notes: String::new(),
        });
        report.examples_compiled = 170;
        report.examples_total = 178;
        report.feature_parity_pct = 95.0;
        report.fixed_point = true;

        assert_eq!(report.module_count(), 2);
        assert_eq!(report.total_loc(), 1100);
        assert!(report.overall_fn_completeness() > 80.0);
        assert!(report.overall_test_pass_rate() > 90.0);

        let rendered = report.render();
        assert!(rendered.contains("Self-Hosting Audit Report"));
        assert!(rendered.contains("lexer"));
        assert!(rendered.contains("parser"));
        assert!(rendered.contains("170/178"));
        assert!(rendered.contains("YES"));
    }

    // S10.7 — Standard Module Statuses
    #[test]
    fn s10_7_standard_modules() {
        let modules = standard_module_statuses();
        assert_eq!(modules.len(), 7);
        assert!(modules.iter().all(|m| m.fn_completeness() >= 100.0));
        assert!(modules.iter().all(|m| m.test_pass_rate() >= 100.0));
        assert!(modules.iter().any(|m| m.name == "lexer"));
        assert!(modules.iter().any(|m| m.name == "optimizer"));
        assert!(modules.iter().any(|m| m.name == "diagnostics"));
    }

    // S10.8 — Size Comparison Edge Cases
    #[test]
    fn s10_8_size_comparison_same() {
        let a = BinarySizeInfo::new("a", 5_000_000);
        let b = BinarySizeInfo::new("b", 5_000_000);
        let cmp = SizeComparison::compare(&a, &b);
        assert!((cmp.ratio - 1.0).abs() < 0.01);
        assert_eq!(cmp.diff_bytes, 0);
    }

    // S10.9 — Report with Binary & Timing
    #[test]
    fn s10_9_full_report() {
        let mut report = SelfHostAuditReport::new();
        for m in standard_module_statuses() {
            report.add_module(m);
        }
        report.binary_size = Some(BinarySizeInfo::new("self-hosted", 13_000_000));
        report.bootstrap_timing = Some(BootstrapTiming {
            stage0_build: Duration::from_secs(30),
            stage1_build: Duration::from_secs(60),
            stage2_build: Duration::from_secs(75),
            verification: Duration::from_secs(5),
            test_suite: Duration::from_secs(30),
        });
        report.examples_compiled = 178;
        report.examples_total = 178;
        report.feature_parity_pct = 100.0;
        report.fixed_point = true;

        let rendered = report.render();
        assert!(rendered.contains("Self-Hosting Audit Report"));
        assert!(rendered.contains("12.4 MB"));
        assert!(rendered.contains("2.50x"));
        assert!(rendered.contains("100.0%"));
        assert!(rendered.contains("178/178"));
    }

    // S10.10 — Bench Suite Display
    #[test]
    fn s10_10_bench_suite_display() {
        let mut suite = BenchSuite::new();
        suite.add(BenchMeasurement::new("lex_hello", CompilePhase::Lex, Duration::from_micros(50), 100));
        suite.add(BenchMeasurement::new("parse_hello", CompilePhase::Parse, Duration::from_micros(100), 50));
        let display = suite.to_string();
        assert!(display.contains("Compile Benchmarks"));
        assert!(display.contains("lex_hello"));
        assert!(display.contains("parse_hello"));
    }

    #[test]
    fn s10_10_memory_tracker_display() {
        let mut tracker = MemoryTracker::new();
        tracker.record(MemorySnapshot {
            phase: CompilePhase::Total,
            heap_bytes: 2_048_000,
            peak_heap_bytes: 3_000_000,
            live_allocs: 300,
            total_allocs: 15_000,
        });
        let display = tracker.to_string();
        assert!(display.contains("Memory Usage"));
        assert!(display.contains("Peak:"));
    }

    #[test]
    fn s10_10_empty_report() {
        let report = SelfHostAuditReport::new();
        assert_eq!(report.module_count(), 0);
        assert!((report.overall_fn_completeness() - 100.0).abs() < 0.01);
        assert!(!report.fixed_point);
    }

    #[test]
    fn s10_10_binary_size_bytes() {
        let info = BinarySizeInfo::new("tiny", 512);
        assert_eq!(info.human_size(), "512 B");
    }

    #[test]
    fn s10_10_bench_empty_phase() {
        let suite = BenchSuite::new();
        assert_eq!(suite.avg_throughput(CompilePhase::Lex), 0.0);
        assert_eq!(suite.total_duration(), Duration::ZERO);
    }
}
