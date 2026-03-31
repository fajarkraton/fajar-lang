//! Sprint H5: CI/CD & Build Hardening Module.
//!
//! Provides build-time validation utilities, release checklists,
//! security auditing helpers, and performance baseline tracking
//! for the Fajar Lang compiler and toolchain.
//!
//! # Overview
//!
//! - [`BuildInfo`] — version, git hash, build date, target, profile, Rust version
//! - [`FeatureFlagValidator`] — validate feature flag configurations
//! - [`TestCoverage`] — summarize test counts per module
//! - [`DependencyAudit`] — list dependencies with versions
//! - [`BinaryMetrics`] — binary size and section sizes
//! - [`CiStatusReport`] — aggregate pass/fail across test targets
//! - [`ReleaseChecklist`] — pre-release criteria verification
//! - [`PerformanceBaseline`] — regression detection baselines
//! - [`SecurityChecklist`] — unsafe/unwrap/panic count auditing

use std::collections::HashMap;

// ════════════════════════════════════════════════════════════════════════════
// H5.1 — BuildInfo
// ════════════════════════════════════════════════════════════════════════════

/// Build-time information about the Fajar Lang compiler binary.
///
/// Captures version, git hash, build date, compilation target,
/// build profile, and the Rust compiler version used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildInfo {
    /// Semantic version string (e.g., "11.0.0").
    pub version: String,
    /// Git commit hash at build time (short form, or "unknown").
    pub git_hash: String,
    /// Build date in ISO 8601 format (e.g., "2026-03-31").
    pub build_date: String,
    /// Compilation target triple (e.g., "x86_64-unknown-linux-gnu").
    pub target: String,
    /// Build profile ("debug" or "release").
    pub profile: String,
    /// Rust compiler version (e.g., "1.87.0").
    pub rust_version: String,
}

impl BuildInfo {
    /// Creates a new `BuildInfo` from compile-time environment variables.
    ///
    /// Uses `CARGO_PKG_VERSION`, `TARGET`, `PROFILE`, and falls back to
    /// "unknown" for values that cannot be determined.
    pub fn from_env() -> Self {
        BuildInfo {
            version: env!("CARGO_PKG_VERSION").to_string(),
            git_hash: option_env!("FJ_GIT_HASH").unwrap_or("unknown").to_string(),
            build_date: option_env!("FJ_BUILD_DATE")
                .unwrap_or("unknown")
                .to_string(),
            target: option_env!("FJ_TARGET")
                .unwrap_or(std::env::consts::ARCH)
                .to_string(),
            profile: if cfg!(debug_assertions) {
                "debug".to_string()
            } else {
                "release".to_string()
            },
            rust_version: option_env!("FJ_RUST_VERSION")
                .unwrap_or("unknown")
                .to_string(),
        }
    }

    /// Creates a `BuildInfo` with explicitly provided values (for testing).
    pub fn new(
        version: &str,
        git_hash: &str,
        build_date: &str,
        target: &str,
        profile: &str,
        rust_version: &str,
    ) -> Self {
        BuildInfo {
            version: version.to_string(),
            git_hash: git_hash.to_string(),
            build_date: build_date.to_string(),
            target: target.to_string(),
            profile: profile.to_string(),
            rust_version: rust_version.to_string(),
        }
    }

    /// Returns a human-readable summary string.
    pub fn summary(&self) -> String {
        format!(
            "fajar-lang {} ({}) built {} for {} [{}] rust-{}",
            self.version,
            self.git_hash,
            self.build_date,
            self.target,
            self.profile,
            self.rust_version
        )
    }
}

impl std::fmt::Display for BuildInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.summary())
    }
}

// ════════════════════════════════════════════════════════════════════════════
// H5.2 — FeatureFlagValidator
// ════════════════════════════════════════════════════════════════════════════

/// Validates that feature flag configurations are consistent.
///
/// Tracks known feature flags and can check whether a given set of
/// flags is valid (no unknown flags, no conflicts).
#[derive(Debug, Clone)]
pub struct FeatureFlagValidator {
    /// Set of known valid feature flags.
    known_flags: Vec<String>,
    /// Pairs of mutually exclusive flags.
    conflicts: Vec<(String, String)>,
}

impl FeatureFlagValidator {
    /// Creates a validator with Fajar Lang's known feature flags.
    pub fn new() -> Self {
        FeatureFlagValidator {
            known_flags: vec![
                "native".to_string(),
                "llvm".to_string(),
                "websocket".to_string(),
                "mqtt".to_string(),
                "ble".to_string(),
                "gui".to_string(),
                "https".to_string(),
            ],
            conflicts: Vec::new(),
        }
    }

    /// Adds a known feature flag.
    pub fn add_flag(&mut self, flag: &str) {
        if !self.known_flags.iter().any(|f| f == flag) {
            self.known_flags.push(flag.to_string());
        }
    }

    /// Registers a conflict between two flags (mutually exclusive).
    pub fn add_conflict(&mut self, a: &str, b: &str) {
        self.conflicts.push((a.to_string(), b.to_string()));
    }

    /// Validates a set of active flags. Returns errors for unknown flags
    /// or conflicting flag pairs.
    pub fn validate(&self, active_flags: &[&str]) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        for flag in active_flags {
            if !self.known_flags.iter().any(|f| f == flag) {
                errors.push(format!("unknown feature flag: '{flag}'"));
            }
        }

        for (a, b) in &self.conflicts {
            if active_flags.contains(&a.as_str()) && active_flags.contains(&b.as_str()) {
                errors.push(format!("conflicting flags: '{a}' and '{b}'"));
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Returns the list of known feature flags.
    pub fn known_flags(&self) -> &[String] {
        &self.known_flags
    }
}

impl Default for FeatureFlagValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ════════════════════════════════════════════════════════════════════════════
// H5.3 — TestCoverage
// ════════════════════════════════════════════════════════════════════════════

/// Summary of test counts per module for coverage reporting.
///
/// Tracks how many tests exist in each module/file, along with
/// pass/fail counts from a test run.
#[derive(Debug, Clone, Default)]
pub struct TestCoverage {
    /// Module name → test count.
    modules: HashMap<String, TestModuleStats>,
}

/// Statistics for a single test module.
#[derive(Debug, Clone, Default)]
pub struct TestModuleStats {
    /// Total number of tests in this module.
    pub total: usize,
    /// Number of passing tests.
    pub passed: usize,
    /// Number of failing tests.
    pub failed: usize,
    /// Number of ignored/skipped tests.
    pub ignored: usize,
}

impl TestCoverage {
    /// Creates an empty coverage report.
    pub fn new() -> Self {
        TestCoverage {
            modules: HashMap::new(),
        }
    }

    /// Records test results for a module.
    pub fn record(
        &mut self,
        module: &str,
        total: usize,
        passed: usize,
        failed: usize,
        ignored: usize,
    ) {
        self.modules.insert(
            module.to_string(),
            TestModuleStats {
                total,
                passed,
                failed,
                ignored,
            },
        );
    }

    /// Returns the total number of tests across all modules.
    pub fn total_tests(&self) -> usize {
        self.modules.values().map(|s| s.total).sum()
    }

    /// Returns the total number of passing tests across all modules.
    pub fn total_passed(&self) -> usize {
        self.modules.values().map(|s| s.passed).sum()
    }

    /// Returns the total number of failing tests across all modules.
    pub fn total_failed(&self) -> usize {
        self.modules.values().map(|s| s.failed).sum()
    }

    /// Returns the pass rate as a percentage (0.0 to 100.0).
    /// Returns 100.0 if there are no tests (vacuously true).
    pub fn pass_rate(&self) -> f64 {
        let total = self.total_tests();
        if total == 0 {
            return 100.0;
        }
        (self.total_passed() as f64 / total as f64) * 100.0
    }

    /// Returns statistics for a specific module, if recorded.
    pub fn module_stats(&self, module: &str) -> Option<&TestModuleStats> {
        self.modules.get(module)
    }

    /// Returns all module names in sorted order.
    pub fn modules(&self) -> Vec<String> {
        let mut names: Vec<String> = self.modules.keys().cloned().collect();
        names.sort();
        names
    }

    /// Generates a summary report string.
    pub fn summary(&self) -> String {
        let mut lines = Vec::new();
        lines.push(format!(
            "Test Coverage: {}/{} passed ({:.1}%)",
            self.total_passed(),
            self.total_tests(),
            self.pass_rate()
        ));
        for module in self.modules() {
            if let Some(stats) = self.module_stats(&module) {
                lines.push(format!(
                    "  {}: {}/{} passed, {} failed, {} ignored",
                    module, stats.passed, stats.total, stats.failed, stats.ignored
                ));
            }
        }
        lines.join("\n")
    }
}

// ════════════════════════════════════════════════════════════════════════════
// H5.4 — DependencyAudit
// ════════════════════════════════════════════════════════════════════════════

/// Represents a single dependency entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dependency {
    /// Crate name.
    pub name: String,
    /// Version string.
    pub version: String,
    /// Whether this is a direct dependency (vs. transitive).
    pub direct: bool,
}

/// Dependency audit report listing all dependencies with versions.
///
/// Can be populated from Cargo.lock data or manually for testing.
#[derive(Debug, Clone, Default)]
pub struct DependencyAudit {
    /// All dependencies.
    deps: Vec<Dependency>,
}

impl DependencyAudit {
    /// Creates an empty audit.
    pub fn new() -> Self {
        DependencyAudit { deps: Vec::new() }
    }

    /// Adds a dependency to the audit.
    pub fn add(&mut self, name: &str, version: &str, direct: bool) {
        self.deps.push(Dependency {
            name: name.to_string(),
            version: version.to_string(),
            direct,
        });
    }

    /// Parses a simplified Cargo.lock-style text into dependency entries.
    ///
    /// Expected format per entry:
    /// ```text
    /// [[package]]
    /// name = "crate-name"
    /// version = "1.2.3"
    /// ```
    ///
    /// Returns the number of entries parsed. Direct/transitive classification
    /// defaults to `false` (transitive); the caller can mark known direct deps.
    pub fn parse_cargo_lock(&mut self, content: &str) -> usize {
        let mut count = 0;
        let mut current_name: Option<String> = None;
        let mut current_version: Option<String> = None;

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed == "[[package]]" {
                // Flush previous entry.
                if let (Some(name), Some(version)) = (current_name.take(), current_version.take()) {
                    self.deps.push(Dependency {
                        name,
                        version,
                        direct: false,
                    });
                    count += 1;
                }
            } else if let Some(rest) = trimmed.strip_prefix("name = ") {
                current_name = Some(rest.trim_matches('"').to_string());
            } else if let Some(rest) = trimmed.strip_prefix("version = ") {
                current_version = Some(rest.trim_matches('"').to_string());
            }
        }
        // Flush final entry.
        if let (Some(name), Some(version)) = (current_name, current_version) {
            self.deps.push(Dependency {
                name,
                version,
                direct: false,
            });
            count += 1;
        }
        count
    }

    /// Marks a dependency as direct (vs. transitive).
    pub fn mark_direct(&mut self, name: &str) {
        for dep in &mut self.deps {
            if dep.name == name {
                dep.direct = true;
            }
        }
    }

    /// Returns the total number of dependencies.
    pub fn count(&self) -> usize {
        self.deps.len()
    }

    /// Returns only direct dependencies.
    pub fn direct_deps(&self) -> Vec<&Dependency> {
        self.deps.iter().filter(|d| d.direct).collect()
    }

    /// Returns all dependencies.
    pub fn all_deps(&self) -> &[Dependency] {
        &self.deps
    }

    /// Finds a dependency by name.
    pub fn find(&self, name: &str) -> Option<&Dependency> {
        self.deps.iter().find(|d| d.name == name)
    }
}

// ════════════════════════════════════════════════════════════════════════════
// H5.5 — BinaryMetrics
// ════════════════════════════════════════════════════════════════════════════

/// Metrics about the compiled binary (sizes, sections).
///
/// Section sizes are simulated when actual binary introspection is
/// not available. Use `from_path` for real file size measurement.
#[derive(Debug, Clone)]
pub struct BinaryMetrics {
    /// Total binary size in bytes.
    pub total_size: u64,
    /// Section sizes: section name → size in bytes.
    pub sections: HashMap<String, u64>,
    /// Binary path (if loaded from disk).
    pub path: Option<String>,
}

impl BinaryMetrics {
    /// Creates metrics with a known total size and empty section map.
    pub fn new(total_size: u64) -> Self {
        BinaryMetrics {
            total_size,
            sections: HashMap::new(),
            path: None,
        }
    }

    /// Loads binary metrics from a file path. Reads the file size; section
    /// sizes are estimated based on typical ELF proportions.
    pub fn from_path(path: &str) -> Result<Self, String> {
        let metadata =
            std::fs::metadata(path).map_err(|e| format!("cannot read binary at '{path}': {e}"))?;

        let total = metadata.len();
        let mut sections = HashMap::new();
        // Estimate section sizes based on typical proportions.
        sections.insert(".text".to_string(), total * 60 / 100);
        sections.insert(".rodata".to_string(), total * 15 / 100);
        sections.insert(".data".to_string(), total * 5 / 100);
        sections.insert(".bss".to_string(), total * 2 / 100);
        sections.insert(".other".to_string(), total * 18 / 100);

        Ok(BinaryMetrics {
            total_size: total,
            sections,
            path: Some(path.to_string()),
        })
    }

    /// Adds or updates a section size.
    pub fn set_section(&mut self, name: &str, size: u64) {
        self.sections.insert(name.to_string(), size);
    }

    /// Returns the size in megabytes (as f64).
    pub fn size_mb(&self) -> f64 {
        self.total_size as f64 / (1024.0 * 1024.0)
    }

    /// Returns a summary string.
    pub fn summary(&self) -> String {
        let mut lines = Vec::new();
        lines.push(format!(
            "Binary size: {:.2} MB ({} bytes)",
            self.size_mb(),
            self.total_size
        ));
        let mut sorted_sections: Vec<_> = self.sections.iter().collect();
        sorted_sections.sort_by_key(|(name, _)| (*name).clone());
        for (name, size) in sorted_sections {
            lines.push(format!("  {name}: {size} bytes"));
        }
        lines.join("\n")
    }
}

// ════════════════════════════════════════════════════════════════════════════
// H5.6 — CiStatusReport
// ════════════════════════════════════════════════════════════════════════════

/// Outcome of a single CI job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CiJobStatus {
    /// Job passed.
    Pass,
    /// Job failed with an error message.
    Fail(String),
    /// Job was skipped.
    Skipped,
}

/// Aggregate CI status report across multiple test targets/jobs.
#[derive(Debug, Clone, Default)]
pub struct CiStatusReport {
    /// Job name → status.
    jobs: Vec<(String, CiJobStatus)>,
}

impl CiStatusReport {
    /// Creates an empty CI report.
    pub fn new() -> Self {
        CiStatusReport { jobs: Vec::new() }
    }

    /// Records the result of a CI job.
    pub fn record(&mut self, job_name: &str, status: CiJobStatus) {
        self.jobs.push((job_name.to_string(), status));
    }

    /// Returns true if all non-skipped jobs passed.
    pub fn all_passed(&self) -> bool {
        self.jobs
            .iter()
            .all(|(_, s)| matches!(s, CiJobStatus::Pass | CiJobStatus::Skipped))
    }

    /// Returns the number of jobs that passed.
    pub fn passed_count(&self) -> usize {
        self.jobs
            .iter()
            .filter(|(_, s)| matches!(s, CiJobStatus::Pass))
            .count()
    }

    /// Returns the number of jobs that failed.
    pub fn failed_count(&self) -> usize {
        self.jobs
            .iter()
            .filter(|(_, s)| matches!(s, CiJobStatus::Fail(_)))
            .count()
    }

    /// Returns the names of failed jobs.
    pub fn failed_jobs(&self) -> Vec<&str> {
        self.jobs
            .iter()
            .filter_map(|(name, s)| {
                if matches!(s, CiJobStatus::Fail(_)) {
                    Some(name.as_str())
                } else {
                    None
                }
            })
            .collect()
    }

    /// Returns a summary string.
    pub fn summary(&self) -> String {
        let total = self.jobs.len();
        let passed = self.passed_count();
        let failed = self.failed_count();
        let skipped = total - passed - failed;
        format!("CI: {passed}/{total} passed, {failed} failed, {skipped} skipped")
    }
}

// ════════════════════════════════════════════════════════════════════════════
// H5.7 — ReleaseChecklist
// ════════════════════════════════════════════════════════════════════════════

/// A single pre-release criterion.
#[derive(Debug, Clone)]
pub struct ChecklistItem {
    /// Description of the criterion.
    pub description: String,
    /// Whether this criterion is met.
    pub met: bool,
    /// Optional note/reason.
    pub note: Option<String>,
}

/// Verifies all pre-release criteria are met before shipping.
///
/// Each item represents a gate (e.g., "all tests pass", "no clippy warnings",
/// "changelog updated"). The release is only approved if all items are met.
#[derive(Debug, Clone, Default)]
pub struct ReleaseChecklist {
    /// Checklist items.
    items: Vec<ChecklistItem>,
}

impl ReleaseChecklist {
    /// Creates an empty release checklist.
    pub fn new() -> Self {
        ReleaseChecklist { items: Vec::new() }
    }

    /// Creates a checklist pre-populated with standard Fajar Lang release criteria.
    pub fn standard() -> Self {
        let mut checklist = ReleaseChecklist::new();
        checklist.add("All tests pass (cargo test)", false, None);
        checklist.add("Clippy clean (cargo clippy -- -D warnings)", false, None);
        checklist.add("Formatting verified (cargo fmt -- --check)", false, None);
        checklist.add("No .unwrap() in src/ (only in tests)", false, None);
        checklist.add("CHANGELOG.md updated", false, None);
        checklist.add("Version bumped in Cargo.toml", false, None);
        checklist.add("All examples compile (fj check)", false, None);
        checklist.add("Release notes drafted", false, None);
        checklist
    }

    /// Adds a checklist item.
    pub fn add(&mut self, description: &str, met: bool, note: Option<&str>) {
        self.items.push(ChecklistItem {
            description: description.to_string(),
            met,
            note: note.map(|s| s.to_string()),
        });
    }

    /// Marks a specific item as met (by index).
    pub fn mark_met(&mut self, index: usize) {
        if let Some(item) = self.items.get_mut(index) {
            item.met = true;
        }
    }

    /// Marks a specific item as met (by description substring match).
    pub fn mark_met_by_desc(&mut self, desc_contains: &str) {
        for item in &mut self.items {
            if item.description.contains(desc_contains) {
                item.met = true;
            }
        }
    }

    /// Returns true if all items are met.
    pub fn all_met(&self) -> bool {
        self.items.iter().all(|i| i.met)
    }

    /// Returns the number of unmet items.
    pub fn unmet_count(&self) -> usize {
        self.items.iter().filter(|i| !i.met).count()
    }

    /// Returns descriptions of unmet items.
    pub fn unmet_items(&self) -> Vec<&str> {
        self.items
            .iter()
            .filter(|i| !i.met)
            .map(|i| i.description.as_str())
            .collect()
    }

    /// Returns all items.
    pub fn items(&self) -> &[ChecklistItem] {
        &self.items
    }

    /// Returns a summary string.
    pub fn summary(&self) -> String {
        let total = self.items.len();
        let met = total - self.unmet_count();
        let mut lines = Vec::new();
        lines.push(format!("Release Checklist: {met}/{total} criteria met"));
        for item in &self.items {
            let mark = if item.met { "[x]" } else { "[ ]" };
            let note = item
                .note
                .as_ref()
                .map(|n| format!(" -- {n}"))
                .unwrap_or_default();
            lines.push(format!("  {mark} {}{note}", item.description));
        }
        lines.join("\n")
    }
}

// ════════════════════════════════════════════════════════════════════════════
// H5.8 — PerformanceBaseline
// ════════════════════════════════════════════════════════════════════════════

/// A single performance metric entry.
#[derive(Debug, Clone)]
pub struct PerfMetric {
    /// Metric name (e.g., "fibonacci_20_ms").
    pub name: String,
    /// Measured value.
    pub value: f64,
    /// Unit (e.g., "ms", "us", "MB").
    pub unit: String,
    /// Threshold for regression detection (max acceptable value).
    pub threshold: Option<f64>,
}

/// Records baseline performance metrics for regression detection.
///
/// Metrics can be compared against previous baselines to detect regressions
/// (values exceeding thresholds).
#[derive(Debug, Clone, Default)]
pub struct PerformanceBaseline {
    /// Recorded metrics.
    metrics: Vec<PerfMetric>,
}

impl PerformanceBaseline {
    /// Creates an empty baseline.
    pub fn new() -> Self {
        PerformanceBaseline {
            metrics: Vec::new(),
        }
    }

    /// Records a metric value.
    pub fn record(&mut self, name: &str, value: f64, unit: &str) {
        self.metrics.push(PerfMetric {
            name: name.to_string(),
            value,
            unit: unit.to_string(),
            threshold: None,
        });
    }

    /// Records a metric with a regression threshold.
    pub fn record_with_threshold(&mut self, name: &str, value: f64, unit: &str, threshold: f64) {
        self.metrics.push(PerfMetric {
            name: name.to_string(),
            value,
            unit: unit.to_string(),
            threshold: Some(threshold),
        });
    }

    /// Checks all metrics against their thresholds. Returns a list of
    /// regressions (metrics that exceed their threshold).
    pub fn check_regressions(&self) -> Vec<&PerfMetric> {
        self.metrics
            .iter()
            .filter(|m| {
                if let Some(threshold) = m.threshold {
                    m.value > threshold
                } else {
                    false
                }
            })
            .collect()
    }

    /// Returns true if no regressions are detected.
    pub fn no_regressions(&self) -> bool {
        self.check_regressions().is_empty()
    }

    /// Returns all recorded metrics.
    pub fn metrics(&self) -> &[PerfMetric] {
        &self.metrics
    }

    /// Returns a metric by name.
    pub fn get(&self, name: &str) -> Option<&PerfMetric> {
        self.metrics.iter().find(|m| m.name == name)
    }

    /// Returns a summary string.
    pub fn summary(&self) -> String {
        let regressions = self.check_regressions();
        let mut lines = Vec::new();
        lines.push(format!(
            "Performance: {} metrics, {} regressions",
            self.metrics.len(),
            regressions.len()
        ));
        for m in &self.metrics {
            let status = match m.threshold {
                Some(t) if m.value > t => " [REGRESSION]",
                Some(_) => " [OK]",
                None => "",
            };
            lines.push(format!("  {}: {:.2} {}{status}", m.name, m.value, m.unit));
        }
        lines.join("\n")
    }
}

// ════════════════════════════════════════════════════════════════════════════
// H5.9 — SecurityChecklist
// ════════════════════════════════════════════════════════════════════════════

/// Security audit results for the codebase.
///
/// Counts occurrences of `unsafe`, `.unwrap()`, `panic!()`, and other
/// potentially dangerous patterns in source code.
#[derive(Debug, Clone, Default)]
pub struct SecurityChecklist {
    /// Number of `unsafe` blocks found.
    pub unsafe_count: usize,
    /// Number of `.unwrap()` calls found in non-test code.
    pub unwrap_count: usize,
    /// Number of `panic!()` calls found in non-test code.
    pub panic_count: usize,
    /// Number of `todo!()` calls found.
    pub todo_count: usize,
    /// Number of `unreachable!()` calls found.
    pub unreachable_count: usize,
    /// Files audited.
    pub files_audited: usize,
    /// Custom findings (description list).
    pub findings: Vec<String>,
}

impl SecurityChecklist {
    /// Creates an empty security checklist.
    pub fn new() -> Self {
        SecurityChecklist::default()
    }

    /// Audits a single source file's content for dangerous patterns.
    ///
    /// Scans the text for `unsafe`, `.unwrap()`, `panic!()`, `todo!()`,
    /// and `unreachable!()` occurrences. Lines inside `#[cfg(test)]` or
    /// `tests/` paths are excluded from unwrap/panic counts via the
    /// `is_test_code` parameter.
    pub fn audit_source(&mut self, content: &str, is_test_code: bool) {
        self.files_audited += 1;

        for line in content.lines() {
            let trimmed = line.trim();

            // Skip comments.
            if trimmed.starts_with("//") {
                continue;
            }

            if trimmed.contains("unsafe ") || trimmed.contains("unsafe{") {
                self.unsafe_count += 1;
            }

            if !is_test_code {
                if trimmed.contains(".unwrap()") {
                    self.unwrap_count += 1;
                }
                if trimmed.contains("panic!(") {
                    self.panic_count += 1;
                }
            }

            if trimmed.contains("todo!(") {
                self.todo_count += 1;
            }
            if trimmed.contains("unreachable!(") {
                self.unreachable_count += 1;
            }
        }
    }

    /// Adds a custom finding/note.
    pub fn add_finding(&mut self, finding: &str) {
        self.findings.push(finding.to_string());
    }

    /// Returns true if the codebase passes all security criteria:
    /// - Zero `.unwrap()` in non-test code
    /// - Zero `panic!()` in non-test code
    pub fn passes_strict(&self) -> bool {
        self.unwrap_count == 0 && self.panic_count == 0
    }

    /// Returns a summary string.
    pub fn summary(&self) -> String {
        let status = if self.passes_strict() {
            "PASS"
        } else {
            "NEEDS REVIEW"
        };
        let mut lines = Vec::new();
        lines.push(format!(
            "Security Audit [{status}] ({} files audited)",
            self.files_audited
        ));
        lines.push(format!("  unsafe blocks: {}", self.unsafe_count));
        lines.push(format!("  .unwrap() in src: {}", self.unwrap_count));
        lines.push(format!("  panic!() in src: {}", self.panic_count));
        lines.push(format!("  todo!(): {}", self.todo_count));
        lines.push(format!("  unreachable!(): {}", self.unreachable_count));
        for finding in &self.findings {
            lines.push(format!("  FINDING: {finding}"));
        }
        lines.join("\n")
    }
}

// ════════════════════════════════════════════════════════════════════════════
// H5.10 — Tests
// ════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // --- BuildInfo tests ---

    #[test]
    fn build_info_from_env_has_version() {
        let info = BuildInfo::from_env();
        assert!(!info.version.is_empty(), "version should not be empty");
        assert!(
            info.version.contains('.'),
            "version should be semver: {}",
            info.version
        );
    }

    #[test]
    fn build_info_new_custom_values() {
        let info = BuildInfo::new(
            "1.2.3",
            "abc123",
            "2026-03-31",
            "x86_64",
            "release",
            "1.87.0",
        );
        assert_eq!(info.version, "1.2.3");
        assert_eq!(info.git_hash, "abc123");
        assert_eq!(info.profile, "release");
    }

    #[test]
    fn build_info_summary_format() {
        let info = BuildInfo::new(
            "11.0.0",
            "deadbeef",
            "2026-03-31",
            "x86_64",
            "debug",
            "1.87.0",
        );
        let s = info.summary();
        assert!(s.contains("11.0.0"));
        assert!(s.contains("deadbeef"));
        assert!(s.contains("debug"));
    }

    #[test]
    fn build_info_display_matches_summary() {
        let info = BuildInfo::new("1.0.0", "abc", "2026-01-01", "arm", "release", "1.80.0");
        assert_eq!(format!("{info}"), info.summary());
    }

    // --- FeatureFlagValidator tests ---

    #[test]
    fn feature_flag_validator_known_flags_accepted() {
        let v = FeatureFlagValidator::new();
        assert!(v.validate(&["native", "llvm"]).is_ok());
    }

    #[test]
    fn feature_flag_validator_unknown_flag_rejected() {
        let v = FeatureFlagValidator::new();
        let errs = v.validate(&["native", "flux_capacitor"]).unwrap_err();
        assert!(errs.iter().any(|e| e.contains("flux_capacitor")));
    }

    #[test]
    fn feature_flag_validator_conflict_detected() {
        let mut v = FeatureFlagValidator::new();
        v.add_conflict("native", "llvm");
        let errs = v.validate(&["native", "llvm"]).unwrap_err();
        assert!(errs.iter().any(|e| e.contains("conflicting")));
    }

    #[test]
    fn feature_flag_validator_add_custom_flag() {
        let mut v = FeatureFlagValidator::new();
        v.add_flag("experimental");
        assert!(v.validate(&["experimental"]).is_ok());
    }

    // --- TestCoverage tests ---

    #[test]
    fn test_coverage_empty_is_100_percent() {
        let cov = TestCoverage::new();
        assert_eq!(cov.pass_rate(), 100.0);
        assert_eq!(cov.total_tests(), 0);
    }

    #[test]
    fn test_coverage_records_and_reports() {
        let mut cov = TestCoverage::new();
        cov.record("lexer", 50, 50, 0, 0);
        cov.record("parser", 100, 98, 2, 0);
        assert_eq!(cov.total_tests(), 150);
        assert_eq!(cov.total_passed(), 148);
        assert_eq!(cov.total_failed(), 2);
    }

    #[test]
    fn test_coverage_summary_format() {
        let mut cov = TestCoverage::new();
        cov.record("eval", 10, 9, 1, 0);
        let s = cov.summary();
        assert!(s.contains("9/10"));
        assert!(s.contains("eval"));
    }

    // --- DependencyAudit tests ---

    #[test]
    fn dependency_audit_add_and_find() {
        let mut audit = DependencyAudit::new();
        audit.add("serde", "1.0.200", true);
        assert_eq!(audit.count(), 1);
        let dep = audit.find("serde").expect("should find serde");
        assert_eq!(dep.version, "1.0.200");
        assert!(dep.direct);
    }

    #[test]
    fn dependency_audit_parse_cargo_lock() {
        let lock_content = r#"
[[package]]
name = "thiserror"
version = "2.0.0"

[[package]]
name = "miette"
version = "7.0.0"
"#;
        let mut audit = DependencyAudit::new();
        let count = audit.parse_cargo_lock(lock_content);
        assert_eq!(count, 2);
        assert!(audit.find("thiserror").is_some());
        assert!(audit.find("miette").is_some());
    }

    #[test]
    fn dependency_audit_mark_direct() {
        let mut audit = DependencyAudit::new();
        audit.add("tokio", "1.40.0", false);
        assert!(!audit.find("tokio").unwrap().direct);
        audit.mark_direct("tokio");
        assert!(audit.find("tokio").unwrap().direct);
    }

    // --- BinaryMetrics tests ---

    #[test]
    fn binary_metrics_size_mb() {
        let m = BinaryMetrics::new(10 * 1024 * 1024);
        assert!((m.size_mb() - 10.0).abs() < 0.01);
    }

    #[test]
    fn binary_metrics_sections() {
        let mut m = BinaryMetrics::new(1000);
        m.set_section(".text", 600);
        m.set_section(".rodata", 200);
        assert_eq!(m.sections.get(".text"), Some(&600));
    }

    #[test]
    fn binary_metrics_summary_contains_size() {
        let m = BinaryMetrics::new(5_000_000);
        let s = m.summary();
        assert!(s.contains("MB"));
        assert!(s.contains("5000000"));
    }

    // --- CiStatusReport tests ---

    #[test]
    fn ci_report_all_pass() {
        let mut r = CiStatusReport::new();
        r.record("test-linux", CiJobStatus::Pass);
        r.record("test-macos", CiJobStatus::Pass);
        r.record("lint", CiJobStatus::Skipped);
        assert!(r.all_passed());
        assert_eq!(r.passed_count(), 2);
        assert_eq!(r.failed_count(), 0);
    }

    #[test]
    fn ci_report_with_failure() {
        let mut r = CiStatusReport::new();
        r.record("test-linux", CiJobStatus::Pass);
        r.record("test-windows", CiJobStatus::Fail("timeout".to_string()));
        assert!(!r.all_passed());
        assert_eq!(r.failed_jobs(), vec!["test-windows"]);
    }

    // --- ReleaseChecklist tests ---

    #[test]
    fn release_checklist_standard_not_initially_met() {
        let cl = ReleaseChecklist::standard();
        assert!(!cl.all_met());
        assert!(cl.unmet_count() > 0);
    }

    #[test]
    fn release_checklist_mark_met() {
        let mut cl = ReleaseChecklist::new();
        cl.add("tests pass", false, None);
        cl.add("clippy clean", false, None);
        assert_eq!(cl.unmet_count(), 2);

        cl.mark_met(0);
        assert_eq!(cl.unmet_count(), 1);

        cl.mark_met_by_desc("clippy");
        assert!(cl.all_met());
    }

    // --- PerformanceBaseline tests ---

    #[test]
    fn perf_baseline_no_regressions_when_within_threshold() {
        let mut pb = PerformanceBaseline::new();
        pb.record_with_threshold("fib_20", 25.0, "ms", 50.0);
        assert!(pb.no_regressions());
    }

    #[test]
    fn perf_baseline_regression_detected() {
        let mut pb = PerformanceBaseline::new();
        pb.record_with_threshold("fib_20", 75.0, "ms", 50.0);
        assert!(!pb.no_regressions());
        assert_eq!(pb.check_regressions().len(), 1);
    }

    #[test]
    fn perf_baseline_no_threshold_no_regression() {
        let mut pb = PerformanceBaseline::new();
        pb.record("compile_time", 1500.0, "ms");
        assert!(pb.no_regressions());
    }

    // --- SecurityChecklist tests ---

    #[test]
    fn security_checklist_clean_code() {
        let mut sc = SecurityChecklist::new();
        sc.audit_source("fn main() { let x = 42; }", false);
        assert_eq!(sc.unsafe_count, 0);
        assert_eq!(sc.unwrap_count, 0);
        assert_eq!(sc.panic_count, 0);
        assert!(sc.passes_strict());
    }

    #[test]
    fn security_checklist_detects_unwrap() {
        let mut sc = SecurityChecklist::new();
        sc.audit_source("let v = x.unwrap();", false);
        assert_eq!(sc.unwrap_count, 1);
        assert!(!sc.passes_strict());
    }

    #[test]
    fn security_checklist_detects_unsafe() {
        let mut sc = SecurityChecklist::new();
        sc.audit_source("unsafe { ptr::read(p) }", false);
        assert_eq!(sc.unsafe_count, 1);
    }

    #[test]
    fn security_checklist_test_code_exempt_from_unwrap() {
        let mut sc = SecurityChecklist::new();
        sc.audit_source("let v = x.unwrap();", true);
        // unwrap in test code should NOT be counted.
        assert_eq!(sc.unwrap_count, 0);
        assert!(sc.passes_strict());
    }

    #[test]
    fn security_checklist_comments_excluded() {
        let mut sc = SecurityChecklist::new();
        sc.audit_source("// unsafe { panic!(\"oh no\") }", false);
        assert_eq!(sc.unsafe_count, 0);
        assert_eq!(sc.panic_count, 0);
    }

    #[test]
    fn security_checklist_summary_format() {
        let mut sc = SecurityChecklist::new();
        sc.audit_source("fn safe() { }", false);
        let s = sc.summary();
        assert!(s.contains("PASS"));
        assert!(s.contains("1 files audited"));
    }

    #[test]
    fn security_checklist_custom_finding() {
        let mut sc = SecurityChecklist::new();
        sc.add_finding("CVE-2099-0001 found in dep X");
        assert_eq!(sc.findings.len(), 1);
        let s = sc.summary();
        assert!(s.contains("CVE-2099-0001"));
    }
}
