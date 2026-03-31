//! Verification Benchmarks & Documentation — Sprint V10 (10 tasks).
//!
//! Verification time benchmarks, scalability tests, false positive rate,
//! bug detection rate, cache speedup measurement, documentation generation,
//! example verified kernel/ML modules, and audit report generation.

use std::collections::HashMap;
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// V10.1: Timing Benchmark
// ═══════════════════════════════════════════════════════════════════════

/// A single verification timing measurement.
#[derive(Debug, Clone)]
pub struct TimingMeasurement {
    /// Name of the benchmark.
    pub name: String,
    /// Number of VCs checked.
    pub vc_count: u64,
    /// Total wall-clock time in milliseconds.
    pub total_ms: f64,
    /// Time per VC in milliseconds.
    pub per_vc_ms: f64,
    /// Peak memory usage in MB.
    pub peak_memory_mb: f64,
    /// Solver backend used.
    pub solver: String,
}

impl TimingMeasurement {
    /// Creates a timing measurement from total time and VC count.
    pub fn new(
        name: &str,
        vc_count: u64,
        total_ms: f64,
        peak_memory_mb: f64,
        solver: &str,
    ) -> Self {
        let per_vc_ms = if vc_count > 0 {
            total_ms / vc_count as f64
        } else {
            0.0
        };
        Self {
            name: name.to_string(),
            vc_count,
            total_ms,
            per_vc_ms,
            peak_memory_mb,
            solver: solver.to_string(),
        }
    }

    /// Returns VCs per second.
    pub fn throughput(&self) -> f64 {
        if self.total_ms <= 0.0 {
            return 0.0;
        }
        (self.vc_count as f64 / self.total_ms) * 1000.0
    }
}

impl fmt::Display for TimingMeasurement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: {} VCs in {:.1}ms ({:.2}ms/vc, {:.0} vc/s, {:.1}MB peak, {})",
            self.name,
            self.vc_count,
            self.total_ms,
            self.per_vc_ms,
            self.throughput(),
            self.peak_memory_mb,
            self.solver,
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════
// V10.2: Scalability Test
// ═══════════════════════════════════════════════════════════════════════

/// Scalability test result (how verification time grows with program size).
#[derive(Debug, Clone)]
pub struct ScalabilityResult {
    /// Data points: (program size in LOC, verification time in ms).
    pub data_points: Vec<(u64, f64)>,
    /// Estimated complexity class.
    pub complexity: ComplexityClass,
    /// Whether verification stays under budget (e.g., < 5s for 10K LOC).
    pub within_budget: bool,
    /// Budget threshold in ms.
    pub budget_ms: f64,
    /// Maximum program size tested in LOC.
    pub max_loc: u64,
}

/// Estimated algorithmic complexity of verification time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComplexityClass {
    /// O(n) — linear in program size.
    Linear,
    /// O(n log n) — near-linear.
    NearLinear,
    /// O(n^2) — quadratic.
    Quadratic,
    /// O(n^3) or worse — polynomial.
    Polynomial,
    /// Exponential or worse.
    Exponential,
}

impl fmt::Display for ComplexityClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Linear => write!(f, "O(n)"),
            Self::NearLinear => write!(f, "O(n log n)"),
            Self::Quadratic => write!(f, "O(n^2)"),
            Self::Polynomial => write!(f, "O(n^k)"),
            Self::Exponential => write!(f, "O(2^n)"),
        }
    }
}

/// Estimates complexity class from data points.
pub fn estimate_complexity(data_points: &[(u64, f64)]) -> ComplexityClass {
    if data_points.len() < 2 {
        return ComplexityClass::Linear;
    }

    // Compute growth ratio between consecutive points.
    let mut ratios = Vec::new();
    for window in data_points.windows(2) {
        let (loc1, time1) = window[0];
        let (loc2, time2) = window[1];
        if loc1 > 0 && time1 > 0.0 && loc2 > loc1 {
            let size_ratio = loc2 as f64 / loc1 as f64;
            let time_ratio = time2 / time1;
            if size_ratio > 0.0 {
                ratios.push(time_ratio / size_ratio);
            }
        }
    }

    if ratios.is_empty() {
        return ComplexityClass::Linear;
    }

    let avg_ratio: f64 = ratios.iter().sum::<f64>() / ratios.len() as f64;

    if avg_ratio < 1.2 {
        ComplexityClass::Linear
    } else if avg_ratio < 1.5 {
        ComplexityClass::NearLinear
    } else if avg_ratio < 2.5 {
        ComplexityClass::Quadratic
    } else if avg_ratio < 5.0 {
        ComplexityClass::Polynomial
    } else {
        ComplexityClass::Exponential
    }
}

// ═══════════════════════════════════════════════════════════════════════
// V10.3: False Positive Rate
// ═══════════════════════════════════════════════════════════════════════

/// False positive analysis result.
#[derive(Debug, Clone)]
pub struct FalsePositiveAnalysis {
    /// Total warnings reported.
    pub total_warnings: u64,
    /// Confirmed true positives (real bugs).
    pub true_positives: u64,
    /// Confirmed false positives (spurious warnings).
    pub false_positives: u64,
    /// Unclassified (not yet reviewed).
    pub unclassified: u64,
    /// Per-category breakdown.
    pub by_category: HashMap<String, CategoryStats>,
}

/// Per-category false positive stats.
#[derive(Debug, Clone)]
pub struct CategoryStats {
    /// Category name.
    pub name: String,
    /// True positives in this category.
    pub true_positives: u64,
    /// False positives in this category.
    pub false_positives: u64,
}

impl CategoryStats {
    /// Returns precision (TP / (TP + FP)).
    pub fn precision(&self) -> f64 {
        let total = self.true_positives + self.false_positives;
        if total == 0 {
            return 1.0;
        }
        self.true_positives as f64 / total as f64
    }
}

impl FalsePositiveAnalysis {
    /// Returns false positive rate (FP / total warnings).
    pub fn false_positive_rate(&self) -> f64 {
        if self.total_warnings == 0 {
            return 0.0;
        }
        self.false_positives as f64 / self.total_warnings as f64
    }

    /// Returns precision (TP / (TP + FP)).
    pub fn precision(&self) -> f64 {
        let total = self.true_positives + self.false_positives;
        if total == 0 {
            return 1.0;
        }
        self.true_positives as f64 / total as f64
    }

    /// Returns true if false positive rate is acceptable (< 10%).
    pub fn is_acceptable(&self) -> bool {
        self.false_positive_rate() < 0.10
    }
}

impl fmt::Display for FalsePositiveAnalysis {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "False Positive Analysis:")?;
        writeln!(f, "  Total warnings:   {}", self.total_warnings)?;
        writeln!(f, "  True positives:   {}", self.true_positives)?;
        writeln!(f, "  False positives:  {}", self.false_positives)?;
        writeln!(f, "  Unclassified:     {}", self.unclassified)?;
        writeln!(
            f,
            "  FP rate:          {:.1}%",
            self.false_positive_rate() * 100.0
        )?;
        writeln!(f, "  Precision:        {:.1}%", self.precision() * 100.0)?;
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// V10.4: Bug Detection Rate
// ═══════════════════════════════════════════════════════════════════════

/// Bug detection effectiveness analysis.
#[derive(Debug, Clone)]
pub struct BugDetectionAnalysis {
    /// Known bugs in the test suite.
    pub known_bugs: u64,
    /// Bugs detected by verification.
    pub detected_bugs: u64,
    /// Bugs missed by verification.
    pub missed_bugs: u64,
    /// Detection breakdown by category.
    pub by_category: HashMap<String, DetectionCategory>,
}

/// Detection stats for a bug category.
#[derive(Debug, Clone)]
pub struct DetectionCategory {
    /// Category name.
    pub name: String,
    /// Bugs in this category.
    pub total: u64,
    /// Bugs detected.
    pub detected: u64,
}

impl DetectionCategory {
    /// Returns detection rate.
    pub fn detection_rate(&self) -> f64 {
        if self.total == 0 {
            return 1.0;
        }
        self.detected as f64 / self.total as f64
    }
}

impl BugDetectionAnalysis {
    /// Returns overall detection rate (recall).
    pub fn detection_rate(&self) -> f64 {
        if self.known_bugs == 0 {
            return 1.0;
        }
        self.detected_bugs as f64 / self.known_bugs as f64
    }

    /// Returns true if detection rate meets target (> 90%).
    pub fn meets_target(&self) -> bool {
        self.detection_rate() >= 0.90
    }
}

impl fmt::Display for BugDetectionAnalysis {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Bug Detection Analysis:")?;
        writeln!(f, "  Known bugs:     {}", self.known_bugs)?;
        writeln!(f, "  Detected:       {}", self.detected_bugs)?;
        writeln!(f, "  Missed:         {}", self.missed_bugs)?;
        writeln!(f, "  Detection rate: {:.1}%", self.detection_rate() * 100.0)?;
        if !self.by_category.is_empty() {
            writeln!(f, "  By category:")?;
            let mut cats: Vec<_> = self.by_category.iter().collect();
            cats.sort_by_key(|(k, _)| (*k).clone());
            for (_, cat) in cats {
                writeln!(
                    f,
                    "    {}: {}/{} ({:.0}%)",
                    cat.name,
                    cat.detected,
                    cat.total,
                    cat.detection_rate() * 100.0,
                )?;
            }
        }
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// V10.5: Cache Speedup Measurement
// ═══════════════════════════════════════════════════════════════════════

/// Cache performance measurement.
#[derive(Debug, Clone)]
pub struct CachePerformance {
    /// Time without cache (cold run) in ms.
    pub cold_run_ms: f64,
    /// Time with cache (warm run) in ms.
    pub warm_run_ms: f64,
    /// Cache hit rate.
    pub hit_rate: f64,
    /// Number of cached entries.
    pub cached_entries: u64,
    /// Cache size on disk in bytes.
    pub cache_size_bytes: u64,
}

impl CachePerformance {
    /// Returns speedup factor (cold / warm).
    pub fn speedup(&self) -> f64 {
        if self.warm_run_ms <= 0.0 {
            return 1.0;
        }
        self.cold_run_ms / self.warm_run_ms
    }

    /// Returns time saved in ms.
    pub fn time_saved_ms(&self) -> f64 {
        self.cold_run_ms - self.warm_run_ms
    }

    /// Returns true if cache provides meaningful speedup (> 2x).
    pub fn is_effective(&self) -> bool {
        self.speedup() >= 2.0
    }
}

impl fmt::Display for CachePerformance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Cache Performance:")?;
        writeln!(f, "  Cold run:     {:.1}ms", self.cold_run_ms)?;
        writeln!(f, "  Warm run:     {:.1}ms", self.warm_run_ms)?;
        writeln!(f, "  Speedup:      {:.1}x", self.speedup())?;
        writeln!(f, "  Time saved:   {:.1}ms", self.time_saved_ms())?;
        writeln!(f, "  Hit rate:     {:.1}%", self.hit_rate * 100.0)?;
        writeln!(f, "  Cache entries: {}", self.cached_entries)?;
        writeln!(f, "  Cache size:   {} bytes", self.cache_size_bytes)?;
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// V10.6: Benchmark Suite
// ═══════════════════════════════════════════════════════════════════════

/// A complete verification benchmark suite.
#[derive(Debug, Clone)]
pub struct BenchmarkSuite {
    /// Suite name.
    pub name: String,
    /// Individual timing measurements.
    pub timings: Vec<TimingMeasurement>,
    /// Scalability result.
    pub scalability: Option<ScalabilityResult>,
    /// False positive analysis.
    pub false_positives: Option<FalsePositiveAnalysis>,
    /// Bug detection analysis.
    pub bug_detection: Option<BugDetectionAnalysis>,
    /// Cache performance.
    pub cache_performance: Option<CachePerformance>,
}

impl BenchmarkSuite {
    /// Creates a new empty benchmark suite.
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            timings: Vec::new(),
            scalability: None,
            false_positives: None,
            bug_detection: None,
            cache_performance: None,
        }
    }

    /// Adds a timing measurement.
    pub fn add_timing(&mut self, measurement: TimingMeasurement) {
        self.timings.push(measurement);
    }

    /// Returns the total VCs across all benchmarks.
    pub fn total_vcs(&self) -> u64 {
        self.timings.iter().map(|t| t.vc_count).sum()
    }

    /// Returns the total verification time.
    pub fn total_time_ms(&self) -> f64 {
        self.timings.iter().map(|t| t.total_ms).sum()
    }

    /// Returns average time per VC across all benchmarks.
    pub fn avg_per_vc_ms(&self) -> f64 {
        let total_vcs = self.total_vcs();
        if total_vcs == 0 {
            return 0.0;
        }
        self.total_time_ms() / total_vcs as f64
    }

    /// Returns true if all quality targets are met.
    pub fn all_targets_met(&self) -> bool {
        let fp_ok = self
            .false_positives
            .as_ref()
            .is_none_or(|fp| fp.is_acceptable());
        let bug_ok = self
            .bug_detection
            .as_ref()
            .is_none_or(|bd| bd.meets_target());
        let cache_ok = self
            .cache_performance
            .as_ref()
            .is_none_or(|cp| cp.is_effective());
        let scalability_ok = self.scalability.as_ref().is_none_or(|s| s.within_budget);
        fp_ok && bug_ok && cache_ok && scalability_ok
    }
}

impl fmt::Display for BenchmarkSuite {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Benchmark Suite: {}", self.name)?;
        writeln!(f, "  Total VCs:         {}", self.total_vcs())?;
        writeln!(f, "  Total time:        {:.1}ms", self.total_time_ms())?;
        writeln!(f, "  Avg per VC:        {:.2}ms", self.avg_per_vc_ms())?;
        writeln!(f)?;

        for timing in &self.timings {
            writeln!(f, "  {timing}")?;
        }

        if let Some(ref sc) = self.scalability {
            writeln!(f)?;
            writeln!(
                f,
                "  Scalability: {} (max {} LOC, budget {:.0}ms — {})",
                sc.complexity,
                sc.max_loc,
                sc.budget_ms,
                if sc.within_budget { "OK" } else { "EXCEEDED" },
            )?;
        }

        if let Some(ref fp) = self.false_positives {
            writeln!(f)?;
            write!(f, "  {fp}")?;
        }

        if let Some(ref bd) = self.bug_detection {
            writeln!(f)?;
            write!(f, "  {bd}")?;
        }

        if let Some(ref cp) = self.cache_performance {
            writeln!(f)?;
            write!(f, "  {cp}")?;
        }

        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// V10.7-V10.8: Verification Audit Report
// ═══════════════════════════════════════════════════════════════════════

/// Complete verification audit report for a project.
#[derive(Debug, Clone)]
pub struct VerificationAuditReport {
    /// Project name.
    pub project: String,
    /// Version.
    pub version: String,
    /// Report date.
    pub date: String,
    /// Module-level completeness.
    pub modules: Vec<ModuleCompleteness>,
    /// Overall verification coverage.
    pub overall_coverage: f64,
    /// Total VCs.
    pub total_vcs: u64,
    /// Total proven.
    pub total_proven: u64,
    /// Total failed.
    pub total_failed: u64,
    /// Total timeout.
    pub total_timeout: u64,
    /// Benchmark results.
    pub benchmarks: Option<BenchmarkSuite>,
    /// Recommendations.
    pub recommendations: Vec<String>,
}

/// Per-module verification completeness.
#[derive(Debug, Clone)]
pub struct ModuleCompleteness {
    /// Module path.
    pub module: String,
    /// Number of VCs.
    pub vc_count: u64,
    /// Number proven.
    pub proven: u64,
    /// Number failed.
    pub failed: u64,
    /// Number timeout.
    pub timeout: u64,
    /// Number of annotated functions (with @requires/@ensures).
    pub annotated_functions: u32,
    /// Total functions in module.
    pub total_functions: u32,
}

impl ModuleCompleteness {
    /// Returns VC coverage.
    pub fn vc_coverage(&self) -> f64 {
        if self.vc_count == 0 {
            return 1.0;
        }
        self.proven as f64 / self.vc_count as f64
    }

    /// Returns annotation coverage (fraction of functions with specs).
    pub fn annotation_coverage(&self) -> f64 {
        if self.total_functions == 0 {
            return 1.0;
        }
        self.annotated_functions as f64 / self.total_functions as f64
    }
}

impl VerificationAuditReport {
    /// Returns overall VC coverage.
    pub fn vc_coverage(&self) -> f64 {
        if self.total_vcs == 0 {
            return 1.0;
        }
        self.total_proven as f64 / self.total_vcs as f64
    }

    /// Returns the number of modules with 100% coverage.
    pub fn fully_verified_modules(&self) -> usize {
        self.modules
            .iter()
            .filter(|m| (m.vc_coverage() - 1.0).abs() < f64::EPSILON)
            .count()
    }

    /// Returns the number of modules with failures.
    pub fn modules_with_failures(&self) -> usize {
        self.modules.iter().filter(|m| m.failed > 0).count()
    }

    /// Generates a text summary.
    pub fn generate_summary(&self) -> String {
        let mut summary = String::new();
        summary.push_str(&format!(
            "Verification Audit Report: {} v{}\n",
            self.project, self.version
        ));
        summary.push_str(&format!("Date: {}\n\n", self.date));
        summary.push_str(&format!(
            "Overall: {}/{} VCs proven ({:.1}%)\n",
            self.total_proven,
            self.total_vcs,
            self.vc_coverage() * 100.0,
        ));
        summary.push_str(&format!(
            "Failed: {}, Timeout: {}\n",
            self.total_failed, self.total_timeout,
        ));
        summary.push_str(&format!(
            "Modules: {} total, {} fully verified, {} with failures\n\n",
            self.modules.len(),
            self.fully_verified_modules(),
            self.modules_with_failures(),
        ));

        for module in &self.modules {
            summary.push_str(&format!(
                "  {}: {}/{} VCs ({:.1}%), {}/{} annotated\n",
                module.module,
                module.proven,
                module.vc_count,
                module.vc_coverage() * 100.0,
                module.annotated_functions,
                module.total_functions,
            ));
        }

        if !self.recommendations.is_empty() {
            summary.push_str("\nRecommendations:\n");
            for (i, rec) in self.recommendations.iter().enumerate() {
                summary.push_str(&format!("  {}. {rec}\n", i + 1));
            }
        }

        summary
    }
}

// ═══════════════════════════════════════════════════════════════════════
// V10.9-V10.10: Documentation Generator
// ═══════════════════════════════════════════════════════════════════════

/// Generates verification documentation section for a module.
pub fn generate_module_doc(
    module_name: &str,
    vc_count: u64,
    proven: u64,
    annotations: &[String],
) -> String {
    let mut doc = String::new();
    doc.push_str(&format!("## Verification: `{module_name}`\n\n"));
    doc.push_str(&format!(
        "**Coverage:** {proven}/{vc_count} VCs proven ({:.1}%)\n\n",
        if vc_count > 0 {
            proven as f64 / vc_count as f64 * 100.0
        } else {
            100.0
        }
    ));

    if !annotations.is_empty() {
        doc.push_str("**Specifications:**\n\n");
        for ann in annotations {
            doc.push_str(&format!("- `{ann}`\n"));
        }
        doc.push('\n');
    }

    doc
}

/// Generates an example verified kernel module documentation.
pub fn generate_example_kernel_doc() -> String {
    let mut doc = String::new();
    doc.push_str("## Example: Verified Kernel Module\n\n");
    doc.push_str("```fajar\n");
    doc.push_str("@kernel\n");
    doc.push_str("@requires(addr != 0 && size > 0 && size <= 4096)\n");
    doc.push_str("@ensures(result != null)\n");
    doc.push_str("fn page_alloc(addr: PhysAddr, size: usize) -> *mut u8 {\n");
    doc.push_str("    // SMT proves: no null return, no overflow in addr+size,\n");
    doc.push_str("    // no allocation > page size.\n");
    doc.push_str("    let page = mem_alloc(size)\n");
    doc.push_str("    page\n");
    doc.push_str("}\n");
    doc.push_str("```\n\n");
    doc.push_str("Verification conditions:\n");
    doc.push_str("1. Precondition: `addr != 0 && size > 0 && size <= 4096`\n");
    doc.push_str("2. Postcondition: `result != null`\n");
    doc.push_str("3. No integer overflow in `addr + size`\n\n");
    doc
}

/// Generates an example verified ML module documentation.
pub fn generate_example_ml_doc() -> String {
    let mut doc = String::new();
    doc.push_str("## Example: Verified ML Module\n\n");
    doc.push_str("```fajar\n");
    doc.push_str("@device\n");
    doc.push_str("@requires(input.shape == [N, 784] && N > 0)\n");
    doc.push_str("@ensures(result.shape == [N, 10])\n");
    doc.push_str("fn classify(input: Tensor) -> Tensor {\n");
    doc.push_str("    let h = relu(matmul(input, weights1))  // [N, 128]\n");
    doc.push_str("    let out = softmax(matmul(h, weights2)) // [N, 10]\n");
    doc.push_str("    out\n");
    doc.push_str("}\n");
    doc.push_str("```\n\n");
    doc.push_str("Verification conditions:\n");
    doc.push_str("1. matmul shape: [N,784] x [784,128] -> [N,128]\n");
    doc.push_str("2. matmul shape: [N,128] x [128,10] -> [N,10]\n");
    doc.push_str("3. softmax preserves shape\n");
    doc.push_str("4. No NaN in output (relu/softmax properties)\n\n");
    doc
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v10_1_timing_measurement() {
        let m = TimingMeasurement::new("bounds_check", 100, 500.0, 64.0, "Z3");
        assert_eq!(m.vc_count, 100);
        assert!((m.per_vc_ms - 5.0).abs() < f64::EPSILON);
        assert!((m.throughput() - 200.0).abs() < f64::EPSILON);
    }

    #[test]
    fn v10_1_timing_display() {
        let m = TimingMeasurement::new("overflow", 50, 250.0, 32.0, "CVC5");
        let s = format!("{m}");
        assert!(s.contains("overflow"));
        assert!(s.contains("50 VCs"));
        assert!(s.contains("CVC5"));
    }

    #[test]
    fn v10_1_timing_zero_vcs() {
        let m = TimingMeasurement::new("empty", 0, 0.0, 0.0, "Z3");
        assert!((m.per_vc_ms - 0.0).abs() < f64::EPSILON);
        assert!((m.throughput() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn v10_2_estimate_complexity_linear() {
        let points = vec![(100, 10.0), (200, 20.0), (400, 40.0)];
        assert_eq!(estimate_complexity(&points), ComplexityClass::Linear);
    }

    #[test]
    fn v10_2_estimate_complexity_quadratic() {
        let points = vec![(100, 10.0), (200, 40.0), (400, 160.0)];
        assert_eq!(estimate_complexity(&points), ComplexityClass::Quadratic);
    }

    #[test]
    fn v10_2_estimate_complexity_few_points() {
        let points = vec![(100, 10.0)];
        assert_eq!(estimate_complexity(&points), ComplexityClass::Linear);
    }

    #[test]
    fn v10_2_complexity_display() {
        assert_eq!(format!("{}", ComplexityClass::Linear), "O(n)");
        assert_eq!(format!("{}", ComplexityClass::Quadratic), "O(n^2)");
        assert_eq!(format!("{}", ComplexityClass::Exponential), "O(2^n)");
    }

    #[test]
    fn v10_3_false_positive_acceptable() {
        let fp = FalsePositiveAnalysis {
            total_warnings: 100,
            true_positives: 95,
            false_positives: 5,
            unclassified: 0,
            by_category: HashMap::new(),
        };
        assert!(fp.is_acceptable());
        assert!((fp.false_positive_rate() - 0.05).abs() < f64::EPSILON);
        assert!((fp.precision() - 0.95).abs() < f64::EPSILON);
    }

    #[test]
    fn v10_3_false_positive_unacceptable() {
        let fp = FalsePositiveAnalysis {
            total_warnings: 100,
            true_positives: 80,
            false_positives: 20,
            unclassified: 0,
            by_category: HashMap::new(),
        };
        assert!(!fp.is_acceptable());
        assert!((fp.false_positive_rate() - 0.20).abs() < f64::EPSILON);
    }

    #[test]
    fn v10_3_false_positive_display() {
        let fp = FalsePositiveAnalysis {
            total_warnings: 50,
            true_positives: 48,
            false_positives: 2,
            unclassified: 0,
            by_category: HashMap::new(),
        };
        let s = format!("{fp}");
        assert!(s.contains("50"));
        assert!(s.contains("Precision"));
    }

    #[test]
    fn v10_3_category_stats_precision() {
        let stats = CategoryStats {
            name: "bounds".to_string(),
            true_positives: 9,
            false_positives: 1,
        };
        assert!((stats.precision() - 0.9).abs() < f64::EPSILON);
    }

    #[test]
    fn v10_4_bug_detection_meets_target() {
        let bd = BugDetectionAnalysis {
            known_bugs: 100,
            detected_bugs: 95,
            missed_bugs: 5,
            by_category: HashMap::new(),
        };
        assert!(bd.meets_target());
        assert!((bd.detection_rate() - 0.95).abs() < f64::EPSILON);
    }

    #[test]
    fn v10_4_bug_detection_misses() {
        let bd = BugDetectionAnalysis {
            known_bugs: 100,
            detected_bugs: 80,
            missed_bugs: 20,
            by_category: HashMap::new(),
        };
        assert!(!bd.meets_target());
    }

    #[test]
    fn v10_4_bug_detection_display() {
        let mut cats = HashMap::new();
        cats.insert(
            "overflow".to_string(),
            DetectionCategory {
                name: "overflow".to_string(),
                total: 10,
                detected: 9,
            },
        );
        let bd = BugDetectionAnalysis {
            known_bugs: 10,
            detected_bugs: 9,
            missed_bugs: 1,
            by_category: cats,
        };
        let s = format!("{bd}");
        assert!(s.contains("overflow"));
        assert!(s.contains("9/10"));
    }

    #[test]
    fn v10_4_detection_category_rate() {
        let cat = DetectionCategory {
            name: "null".to_string(),
            total: 20,
            detected: 18,
        };
        assert!((cat.detection_rate() - 0.9).abs() < f64::EPSILON);
    }

    #[test]
    fn v10_5_cache_performance() {
        let cp = CachePerformance {
            cold_run_ms: 1000.0,
            warm_run_ms: 200.0,
            hit_rate: 0.85,
            cached_entries: 500,
            cache_size_bytes: 1024 * 100,
        };
        assert!((cp.speedup() - 5.0).abs() < f64::EPSILON);
        assert!((cp.time_saved_ms() - 800.0).abs() < f64::EPSILON);
        assert!(cp.is_effective());
    }

    #[test]
    fn v10_5_cache_not_effective() {
        let cp = CachePerformance {
            cold_run_ms: 100.0,
            warm_run_ms: 80.0,
            hit_rate: 0.2,
            cached_entries: 10,
            cache_size_bytes: 512,
        };
        assert!(!cp.is_effective());
    }

    #[test]
    fn v10_5_cache_display() {
        let cp = CachePerformance {
            cold_run_ms: 500.0,
            warm_run_ms: 100.0,
            hit_rate: 0.9,
            cached_entries: 200,
            cache_size_bytes: 50000,
        };
        let s = format!("{cp}");
        assert!(s.contains("5.0x"));
        assert!(s.contains("90.0%"));
    }

    #[test]
    fn v10_6_benchmark_suite_new() {
        let suite = BenchmarkSuite::new("test-suite");
        assert_eq!(suite.name, "test-suite");
        assert_eq!(suite.total_vcs(), 0);
        assert!((suite.total_time_ms() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn v10_6_benchmark_suite_add_timings() {
        let mut suite = BenchmarkSuite::new("regression");
        suite.add_timing(TimingMeasurement::new("a", 50, 100.0, 32.0, "Z3"));
        suite.add_timing(TimingMeasurement::new("b", 30, 60.0, 16.0, "Z3"));
        assert_eq!(suite.total_vcs(), 80);
        assert!((suite.total_time_ms() - 160.0).abs() < f64::EPSILON);
        assert!((suite.avg_per_vc_ms() - 2.0).abs() < f64::EPSILON);
    }

    #[test]
    fn v10_6_benchmark_suite_targets() {
        let mut suite = BenchmarkSuite::new("targets");
        suite.false_positives = Some(FalsePositiveAnalysis {
            total_warnings: 100,
            true_positives: 95,
            false_positives: 5,
            unclassified: 0,
            by_category: HashMap::new(),
        });
        suite.bug_detection = Some(BugDetectionAnalysis {
            known_bugs: 50,
            detected_bugs: 48,
            missed_bugs: 2,
            by_category: HashMap::new(),
        });
        suite.cache_performance = Some(CachePerformance {
            cold_run_ms: 1000.0,
            warm_run_ms: 200.0,
            hit_rate: 0.9,
            cached_entries: 100,
            cache_size_bytes: 5000,
        });
        suite.scalability = Some(ScalabilityResult {
            data_points: vec![(100, 10.0)],
            complexity: ComplexityClass::Linear,
            within_budget: true,
            budget_ms: 5000.0,
            max_loc: 10000,
        });
        assert!(suite.all_targets_met());
    }

    #[test]
    fn v10_6_benchmark_suite_display() {
        let mut suite = BenchmarkSuite::new("display-test");
        suite.add_timing(TimingMeasurement::new("t1", 10, 50.0, 8.0, "Z3"));
        let s = format!("{suite}");
        assert!(s.contains("display-test"));
        assert!(s.contains("10"));
    }

    #[test]
    fn v10_7_module_completeness() {
        let m = ModuleCompleteness {
            module: "kernel::memory".to_string(),
            vc_count: 20,
            proven: 18,
            failed: 1,
            timeout: 1,
            annotated_functions: 8,
            total_functions: 10,
        };
        assert!((m.vc_coverage() - 0.9).abs() < f64::EPSILON);
        assert!((m.annotation_coverage() - 0.8).abs() < f64::EPSILON);
    }

    #[test]
    fn v10_7_audit_report() {
        let report = VerificationAuditReport {
            project: "drone-firmware".to_string(),
            version: "v1.0".to_string(),
            date: "2026-03-31".to_string(),
            modules: vec![
                ModuleCompleteness {
                    module: "sensors".to_string(),
                    vc_count: 10,
                    proven: 10,
                    failed: 0,
                    timeout: 0,
                    annotated_functions: 5,
                    total_functions: 5,
                },
                ModuleCompleteness {
                    module: "control".to_string(),
                    vc_count: 20,
                    proven: 18,
                    failed: 2,
                    timeout: 0,
                    annotated_functions: 8,
                    total_functions: 10,
                },
            ],
            overall_coverage: 0.93,
            total_vcs: 30,
            total_proven: 28,
            total_failed: 2,
            total_timeout: 0,
            benchmarks: None,
            recommendations: vec!["Add specs to control module".to_string()],
        };

        assert_eq!(report.fully_verified_modules(), 1);
        assert_eq!(report.modules_with_failures(), 1);
        assert!((report.vc_coverage() - 28.0 / 30.0).abs() < f64::EPSILON);

        let summary = report.generate_summary();
        assert!(summary.contains("drone-firmware"));
        assert!(summary.contains("28/30"));
        assert!(summary.contains("sensors"));
        assert!(summary.contains("control"));
        assert!(summary.contains("Recommendations"));
    }

    #[test]
    fn v10_8_audit_report_empty() {
        let report = VerificationAuditReport {
            project: "empty".to_string(),
            version: "v0".to_string(),
            date: "2026-03-31".to_string(),
            modules: vec![],
            overall_coverage: 1.0,
            total_vcs: 0,
            total_proven: 0,
            total_failed: 0,
            total_timeout: 0,
            benchmarks: None,
            recommendations: vec![],
        };
        assert_eq!(report.fully_verified_modules(), 0);
        assert!((report.vc_coverage() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn v10_9_generate_module_doc() {
        let doc = generate_module_doc(
            "kernel::memory",
            20,
            18,
            &[
                "@requires(addr != 0)".to_string(),
                "@ensures(result >= 0)".to_string(),
            ],
        );
        assert!(doc.contains("kernel::memory"));
        assert!(doc.contains("18/20"));
        assert!(doc.contains("@requires"));
        assert!(doc.contains("@ensures"));
    }

    #[test]
    fn v10_9_generate_module_doc_no_annotations() {
        let doc = generate_module_doc("utils", 5, 5, &[]);
        assert!(doc.contains("utils"));
        assert!(doc.contains("100.0%"));
        assert!(!doc.contains("Specifications"));
    }

    #[test]
    fn v10_10_example_kernel_doc() {
        let doc = generate_example_kernel_doc();
        assert!(doc.contains("@kernel"));
        assert!(doc.contains("@requires"));
        assert!(doc.contains("@ensures"));
        assert!(doc.contains("page_alloc"));
    }

    #[test]
    fn v10_10_example_ml_doc() {
        let doc = generate_example_ml_doc();
        assert!(doc.contains("@device"));
        assert!(doc.contains("classify"));
        assert!(doc.contains("matmul"));
        assert!(doc.contains("softmax"));
    }

    #[test]
    fn v10_3_false_positive_zero_warnings() {
        let fp = FalsePositiveAnalysis {
            total_warnings: 0,
            true_positives: 0,
            false_positives: 0,
            unclassified: 0,
            by_category: HashMap::new(),
        };
        assert!((fp.false_positive_rate() - 0.0).abs() < f64::EPSILON);
        assert!((fp.precision() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn v10_4_zero_known_bugs() {
        let bd = BugDetectionAnalysis {
            known_bugs: 0,
            detected_bugs: 0,
            missed_bugs: 0,
            by_category: HashMap::new(),
        };
        assert!((bd.detection_rate() - 1.0).abs() < f64::EPSILON);
        assert!(bd.meets_target());
    }
}
