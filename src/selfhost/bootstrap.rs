//! Bootstrap chain — Stage 0/1/2 compilation, binary comparison,
//! test suite validation, performance comparison, error message parity,
//! bootstrap script, CI integration.

use std::fmt;
use std::time::Duration;

// ═══════════════════════════════════════════════════════════════════════
// S27.1-S27.3: Bootstrap Stages
// ═══════════════════════════════════════════════════════════════════════

/// A bootstrap stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Stage {
    /// Stage 0: Rust compiler → fj-stage0.
    Stage0,
    /// Stage 1: fj-stage0 compiles .fj → fj-stage1.
    Stage1,
    /// Stage 2: fj-stage1 compiles .fj → fj-stage2.
    Stage2,
}

impl fmt::Display for Stage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Stage::Stage0 => write!(f, "Stage 0 (Rust-compiled)"),
            Stage::Stage1 => write!(f, "Stage 1 (fj-compiled)"),
            Stage::Stage2 => write!(f, "Stage 2 (self-compiled)"),
        }
    }
}

/// Result of a bootstrap stage compilation.
#[derive(Debug, Clone)]
pub struct StageResult {
    /// Stage that was built.
    pub stage: Stage,
    /// Output binary path.
    pub binary_path: String,
    /// Binary size in bytes.
    pub binary_size: usize,
    /// SHA-256 hash of the binary.
    pub hash: String,
    /// Compilation time.
    pub compile_time: Duration,
    /// Whether compilation succeeded.
    pub success: bool,
}

impl fmt::Display for StageResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: {} ({} bytes, {:?}) [{}]",
            self.stage,
            self.binary_path,
            self.binary_size,
            self.compile_time,
            if self.success { "OK" } else { "FAIL" }
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S27.4: Binary Comparison
// ═══════════════════════════════════════════════════════════════════════

/// Result of comparing two binaries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinaryComparison {
    /// First binary path.
    pub binary_a: String,
    /// Second binary path.
    pub binary_b: String,
    /// Hash of first binary.
    pub hash_a: String,
    /// Hash of second binary.
    pub hash_b: String,
    /// Whether they are identical.
    pub identical: bool,
    /// First differing offset (if any).
    pub first_diff_offset: Option<usize>,
}

impl fmt::Display for BinaryComparison {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.identical {
            write!(
                f,
                "IDENTICAL: {} == {} (hash: {})",
                self.binary_a, self.binary_b, self.hash_a
            )
        } else {
            write!(
                f,
                "DIFFERENT: {} != {} (first diff at offset {})",
                self.binary_a,
                self.binary_b,
                self.first_diff_offset.unwrap_or(0)
            )
        }
    }
}

/// Compares two byte sequences for equality.
pub fn compare_bytes(a: &[u8], b: &[u8]) -> BinaryComparison {
    let identical = a == b;
    let first_diff = if !identical {
        a.iter()
            .zip(b.iter())
            .position(|(x, y)| x != y)
            .or(Some(a.len().min(b.len())))
    } else {
        None
    };

    BinaryComparison {
        binary_a: "binary_a".into(),
        binary_b: "binary_b".into(),
        hash_a: format!("{:x}", simple_hash(a)),
        hash_b: format!("{:x}", simple_hash(b)),
        identical,
        first_diff_offset: first_diff,
    }
}

/// Simple hash for comparison (not cryptographic).
fn simple_hash(data: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for &byte in data {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

// ═══════════════════════════════════════════════════════════════════════
// S27.5: Test Suite Validation
// ═══════════════════════════════════════════════════════════════════════

/// Result of running the test suite through a stage binary.
#[derive(Debug, Clone)]
pub struct TestSuiteResult {
    /// Stage that ran the tests.
    pub stage: Stage,
    /// Total tests.
    pub total: usize,
    /// Passed tests.
    pub passed: usize,
    /// Failed tests.
    pub failed: usize,
    /// Skipped tests.
    pub skipped: usize,
    /// Total run time.
    pub duration: Duration,
}

impl TestSuiteResult {
    /// Whether all tests passed.
    pub fn all_passed(&self) -> bool {
        self.failed == 0
    }

    /// Pass rate as a percentage.
    pub fn pass_rate(&self) -> f64 {
        if self.total > 0 {
            (self.passed as f64 / self.total as f64) * 100.0
        } else {
            100.0
        }
    }
}

impl fmt::Display for TestSuiteResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: {}/{} passed ({:.1}%), {} failed, {} skipped ({:?})",
            self.stage,
            self.passed,
            self.total,
            self.pass_rate(),
            self.failed,
            self.skipped,
            self.duration
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S27.6: Performance Comparison
// ═══════════════════════════════════════════════════════════════════════

/// Performance comparison between stages.
#[derive(Debug, Clone)]
pub struct PerfComparison {
    /// Benchmark name.
    pub benchmark: String,
    /// Stage 0 result.
    pub stage0_time: Duration,
    /// Stage 1 result.
    pub stage1_time: Duration,
    /// Ratio (stage1 / stage0).
    pub ratio: f64,
    /// Whether within acceptable threshold.
    pub acceptable: bool,
}

impl PerfComparison {
    /// Creates a performance comparison.
    pub fn compare(benchmark: &str, stage0: Duration, stage1: Duration, max_ratio: f64) -> Self {
        let ratio = stage1.as_secs_f64() / stage0.as_secs_f64().max(0.001);
        Self {
            benchmark: benchmark.into(),
            stage0_time: stage0,
            stage1_time: stage1,
            ratio,
            acceptable: ratio <= max_ratio,
        }
    }
}

impl fmt::Display for PerfComparison {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: stage0={:?}, stage1={:?}, ratio={:.2}x [{}]",
            self.benchmark,
            self.stage0_time,
            self.stage1_time,
            self.ratio,
            if self.acceptable { "OK" } else { "SLOW" }
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S27.7: Error Message Parity
// ═══════════════════════════════════════════════════════════════════════

/// Error message comparison between stages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorParity {
    /// Test program.
    pub program: String,
    /// Stage 0 error messages.
    pub stage0_errors: Vec<String>,
    /// Stage 1 error messages.
    pub stage1_errors: Vec<String>,
    /// Whether messages match exactly.
    pub exact_match: bool,
}

impl ErrorParity {
    /// Compares error messages from two stages.
    pub fn compare(program: &str, stage0: Vec<String>, stage1: Vec<String>) -> Self {
        let exact_match = stage0 == stage1;
        Self {
            program: program.into(),
            stage0_errors: stage0,
            stage1_errors: stage1,
            exact_match,
        }
    }
}

impl fmt::Display for ErrorParity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.exact_match {
            write!(f, "{}: errors MATCH", self.program)
        } else {
            write!(
                f,
                "{}: errors DIFFER (stage0={}, stage1={})",
                self.program,
                self.stage0_errors.len(),
                self.stage1_errors.len()
            )
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S27.8 / S27.9: Bootstrap Script & CI
// ═══════════════════════════════════════════════════════════════════════

/// Bootstrap pipeline configuration.
#[derive(Debug, Clone)]
pub struct BootstrapConfig {
    /// Source directory.
    pub source_dir: String,
    /// Output directory.
    pub output_dir: String,
    /// Performance threshold (max ratio stage1/stage0).
    pub perf_threshold: f64,
    /// Whether to run test suite validation.
    pub run_tests: bool,
    /// Whether to verify binary reproducibility.
    pub verify_reproducible: bool,
}

impl Default for BootstrapConfig {
    fn default() -> Self {
        Self {
            source_dir: "src".into(),
            output_dir: "target/bootstrap".into(),
            perf_threshold: 2.0,
            run_tests: true,
            verify_reproducible: true,
        }
    }
}

/// Full bootstrap pipeline result.
#[derive(Debug, Clone)]
pub struct BootstrapResult {
    /// Stage results.
    pub stages: Vec<StageResult>,
    /// Binary comparison (stage1 vs stage2).
    pub binary_match: Option<BinaryComparison>,
    /// Test suite results.
    pub test_results: Vec<TestSuiteResult>,
    /// Overall success.
    pub success: bool,
}

impl BootstrapResult {
    /// Creates a successful bootstrap result.
    pub fn success(stages: Vec<StageResult>) -> Self {
        Self {
            success: true,
            stages,
            binary_match: None,
            test_results: Vec::new(),
        }
    }

    /// Renders the bootstrap result as a report.
    pub fn render(&self) -> String {
        let mut lines = Vec::new();
        lines.push("=== Bootstrap Report ===".into());
        for stage in &self.stages {
            lines.push(format!("  {stage}"));
        }
        if let Some(cmp) = &self.binary_match {
            lines.push(format!("  Binary: {cmp}"));
        }
        for test in &self.test_results {
            lines.push(format!("  Tests: {test}"));
        }
        lines.push(format!(
            "  Result: {}",
            if self.success { "PASS" } else { "FAIL" }
        ));
        lines.join("\n")
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // S27.1 — Stage 0
    #[test]
    fn s27_1_stage_display() {
        assert!(Stage::Stage0.to_string().contains("Rust"));
        assert!(Stage::Stage1.to_string().contains("fj-compiled"));
        assert!(Stage::Stage2.to_string().contains("self-compiled"));
    }

    #[test]
    fn s27_1_stage_result() {
        let result = StageResult {
            stage: Stage::Stage0,
            binary_path: "target/release/fj".into(),
            binary_size: 5_000_000,
            hash: "abc123".into(),
            compile_time: Duration::from_secs(30),
            success: true,
        };
        assert!(result.to_string().contains("Stage 0"));
        assert!(result.to_string().contains("OK"));
    }

    // S27.2 / S27.3 — Stage 1 & 2
    #[test]
    fn s27_2_stage1_result() {
        let result = StageResult {
            stage: Stage::Stage1,
            binary_path: "target/bootstrap/fj-stage1".into(),
            binary_size: 6_000_000,
            hash: "def456".into(),
            compile_time: Duration::from_secs(45),
            success: true,
        };
        assert!(result.success);
    }

    // S27.4 — Binary Comparison
    #[test]
    fn s27_4_identical_binaries() {
        let data = vec![1, 2, 3, 4, 5];
        let cmp = compare_bytes(&data, &data);
        assert!(cmp.identical);
        assert_eq!(cmp.first_diff_offset, None);
    }

    #[test]
    fn s27_4_different_binaries() {
        let a = vec![1, 2, 3, 4, 5];
        let b = vec![1, 2, 9, 4, 5];
        let cmp = compare_bytes(&a, &b);
        assert!(!cmp.identical);
        assert_eq!(cmp.first_diff_offset, Some(2));
    }

    #[test]
    fn s27_4_comparison_display() {
        let cmp = BinaryComparison {
            binary_a: "stage1".into(),
            binary_b: "stage2".into(),
            hash_a: "abc".into(),
            hash_b: "abc".into(),
            identical: true,
            first_diff_offset: None,
        };
        assert!(cmp.to_string().contains("IDENTICAL"));
    }

    // S27.5 — Test Suite Validation
    #[test]
    fn s27_5_all_passed() {
        let result = TestSuiteResult {
            stage: Stage::Stage1,
            total: 3000,
            passed: 3000,
            failed: 0,
            skipped: 0,
            duration: Duration::from_secs(60),
        };
        assert!(result.all_passed());
        assert!((result.pass_rate() - 100.0).abs() < 0.01);
    }

    #[test]
    fn s27_5_some_failed() {
        let result = TestSuiteResult {
            stage: Stage::Stage1,
            total: 100,
            passed: 95,
            failed: 5,
            skipped: 0,
            duration: Duration::from_secs(10),
        };
        assert!(!result.all_passed());
        assert!((result.pass_rate() - 95.0).abs() < 0.01);
    }

    // S27.6 — Performance Comparison
    #[test]
    fn s27_6_acceptable_perf() {
        let cmp = PerfComparison::compare(
            "fibonacci",
            Duration::from_millis(100),
            Duration::from_millis(150),
            2.0,
        );
        assert!(cmp.acceptable);
        assert!(cmp.ratio < 2.0);
    }

    #[test]
    fn s27_6_slow_perf() {
        let cmp = PerfComparison::compare(
            "sort",
            Duration::from_millis(100),
            Duration::from_millis(300),
            2.0,
        );
        assert!(!cmp.acceptable);
    }

    // S27.7 — Error Message Parity
    #[test]
    fn s27_7_matching_errors() {
        let parity = ErrorParity::compare(
            "test.fj",
            vec!["SE004: type mismatch".into()],
            vec!["SE004: type mismatch".into()],
        );
        assert!(parity.exact_match);
        assert!(parity.to_string().contains("MATCH"));
    }

    #[test]
    fn s27_7_different_errors() {
        let parity = ErrorParity::compare(
            "test.fj",
            vec!["SE004: type mismatch".into()],
            vec!["SE004: type error".into()],
        );
        assert!(!parity.exact_match);
        assert!(parity.to_string().contains("DIFFER"));
    }

    // S27.8 — Bootstrap Script
    #[test]
    fn s27_8_bootstrap_config_default() {
        let config = BootstrapConfig::default();
        assert_eq!(config.perf_threshold, 2.0);
        assert!(config.run_tests);
        assert!(config.verify_reproducible);
    }

    // S27.9 — CI Bootstrap
    #[test]
    fn s27_9_bootstrap_result_render() {
        let result = BootstrapResult::success(vec![StageResult {
            stage: Stage::Stage0,
            binary_path: "fj".into(),
            binary_size: 5_000_000,
            hash: "abc".into(),
            compile_time: Duration::from_secs(30),
            success: true,
        }]);
        let report = result.render();
        assert!(report.contains("Bootstrap Report"));
        assert!(report.contains("PASS"));
    }

    // S27.10 — Additional
    #[test]
    fn s27_10_perf_comparison_display() {
        let cmp = PerfComparison::compare(
            "parse",
            Duration::from_millis(50),
            Duration::from_millis(80),
            2.0,
        );
        assert!(cmp.to_string().contains("parse"));
        assert!(cmp.to_string().contains("OK"));
    }

    #[test]
    fn s27_10_test_suite_display() {
        let result = TestSuiteResult {
            stage: Stage::Stage1,
            total: 100,
            passed: 100,
            failed: 0,
            skipped: 0,
            duration: Duration::from_secs(5),
        };
        let display = result.to_string();
        assert!(display.contains("100/100"));
        assert!(display.contains("100.0%"));
    }
}
