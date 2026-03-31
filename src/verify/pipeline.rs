//! Pipeline Integration — Sprint V7 (10 tasks).
//!
//! Integrates formal verification into the Fajar Lang compiler pipeline:
//! `fj verify` CLI command, `fj check --verify`, LSP verification hints,
//! CI verification step, fj.toml configuration, suppression comments,
//! REPL verification, IDE code actions, and verification diffs.

use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;

// ═══════════════════════════════════════════════════════════════════════
// V7.1: Verification Configuration (from fj.toml)
// ═══════════════════════════════════════════════════════════════════════

/// Verification configuration parsed from `[verify]` section in fj.toml.
#[derive(Debug, Clone)]
pub struct VerifyConfig {
    /// Enable verification (default: false — opt-in).
    pub enabled: bool,
    /// Solver timeout per VC in milliseconds.
    pub timeout_ms: u64,
    /// Maximum total verification time in seconds.
    pub max_total_time_s: u64,
    /// Solver backend name.
    pub solver: String,
    /// Fallback solver (if primary times out).
    pub fallback_solver: Option<String>,
    /// Enable automated property inference.
    pub auto_infer: bool,
    /// Minimum confidence for inferred properties (0.0-1.0).
    pub min_confidence: f64,
    /// Properties to verify (empty = all).
    pub properties: Vec<String>,
    /// Files/modules to exclude from verification.
    pub exclude: Vec<String>,
    /// Enable proof caching.
    pub cache_enabled: bool,
    /// Cache directory.
    pub cache_dir: PathBuf,
    /// Fail CI on verification failure.
    pub fail_on_error: bool,
    /// Fail CI on verification timeout.
    pub fail_on_timeout: bool,
    /// Number of parallel verification threads.
    pub parallel: u32,
    /// Certification standard (e.g., "DO-178C", "ISO-26262").
    pub certification: Option<String>,
}

impl Default for VerifyConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            timeout_ms: 5000,
            max_total_time_s: 300,
            solver: "z3".to_string(),
            fallback_solver: Some("cvc5".to_string()),
            auto_infer: true,
            min_confidence: 0.5,
            properties: Vec::new(),
            exclude: Vec::new(),
            cache_enabled: true,
            cache_dir: PathBuf::from(".fj-verify-cache"),
            fail_on_error: true,
            fail_on_timeout: false,
            parallel: 4,
            certification: None,
        }
    }
}

impl VerifyConfig {
    /// Parses verification config from a TOML table.
    pub fn from_toml(table: &HashMap<String, String>) -> Self {
        let mut config = Self::default();

        if let Some(v) = table.get("enabled") {
            config.enabled = v == "true";
        }
        if let Some(v) = table.get("timeout_ms") {
            if let Ok(ms) = v.parse::<u64>() {
                config.timeout_ms = ms;
            }
        }
        if let Some(v) = table.get("solver") {
            config.solver = v.clone();
        }
        if let Some(v) = table.get("auto_infer") {
            config.auto_infer = v == "true";
        }
        if let Some(v) = table.get("min_confidence") {
            if let Ok(c) = v.parse::<f64>() {
                config.min_confidence = c;
            }
        }
        if let Some(v) = table.get("fail_on_error") {
            config.fail_on_error = v == "true";
        }
        if let Some(v) = table.get("parallel") {
            if let Ok(p) = v.parse::<u32>() {
                config.parallel = p;
            }
        }
        if let Some(v) = table.get("certification") {
            config.certification = Some(v.clone());
        }

        config
    }

    /// Returns true if a file should be verified (not excluded).
    pub fn should_verify(&self, file: &str) -> bool {
        if !self.enabled {
            return false;
        }
        !self.exclude.iter().any(|pattern| file.contains(pattern))
    }
}

// ═══════════════════════════════════════════════════════════════════════
// V7.2: CLI Command (`fj verify`)
// ═══════════════════════════════════════════════════════════════════════

/// Arguments for the `fj verify` CLI command.
#[derive(Debug, Clone)]
pub struct VerifyCommand {
    /// Files to verify (empty = all project files).
    pub files: Vec<PathBuf>,
    /// Override solver timeout.
    pub timeout_ms: Option<u64>,
    /// Verbose output.
    pub verbose: bool,
    /// Output format.
    pub format: OutputFormat,
    /// Only show failures (suppress proven VCs).
    pub failures_only: bool,
    /// Enable auto-inference.
    pub auto_infer: bool,
    /// Generate certification report.
    pub certify: Option<String>,
    /// Show SMT-LIB2 output for debugging.
    pub show_smt: bool,
}

impl Default for VerifyCommand {
    fn default() -> Self {
        Self {
            files: Vec::new(),
            timeout_ms: None,
            verbose: false,
            format: OutputFormat::Human,
            failures_only: false,
            auto_infer: true,
            certify: None,
            show_smt: false,
        }
    }
}

/// Output format for verification results.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    /// Human-readable terminal output.
    Human,
    /// JSON output (for CI integration).
    Json,
    /// SARIF output (for GitHub code scanning).
    Sarif,
    /// CSV output (for spreadsheet analysis).
    Csv,
}

impl fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Human => write!(f, "human"),
            Self::Json => write!(f, "json"),
            Self::Sarif => write!(f, "sarif"),
            Self::Csv => write!(f, "csv"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// V7.3: Suppression Comments
// ═══════════════════════════════════════════════════════════════════════

/// A verification suppression (user explicitly silences a warning).
#[derive(Debug, Clone, PartialEq)]
pub struct Suppression {
    /// The kind of property being suppressed.
    pub kind: String,
    /// Reason for suppression (required).
    pub reason: String,
    /// Source file.
    pub file: String,
    /// Source line.
    pub line: u32,
}

/// Parses suppression comments from source code.
/// Format: `// @verify:suppress(kind) reason`
pub fn parse_suppressions(source: &str, file: &str) -> Vec<Suppression> {
    let mut suppressions = Vec::new();

    for (line_num, line) in source.lines().enumerate() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("// @verify:suppress(") {
            if let Some(paren_end) = rest.find(')') {
                let kind = rest[..paren_end].to_string();
                let reason = rest[paren_end + 1..].trim().to_string();
                suppressions.push(Suppression {
                    kind,
                    reason,
                    file: file.to_string(),
                    line: (line_num + 1) as u32,
                });
            }
        }
    }

    suppressions
}

/// Checks if a property kind at a given line is suppressed.
pub fn is_suppressed(suppressions: &[Suppression], kind: &str, line: u32) -> bool {
    suppressions
        .iter()
        .any(|s| s.kind == kind && (s.line == line || s.line + 1 == line))
}

// ═══════════════════════════════════════════════════════════════════════
// V7.4: Pipeline Integration (`fj check --verify`)
// ═══════════════════════════════════════════════════════════════════════

/// Verification phase in the compiler pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerificationPhase {
    /// After parsing: structural checks.
    PostParse,
    /// After type checking: type safety proofs.
    PostTypeCheck,
    /// After borrow checking: ownership proofs.
    PostBorrowCheck,
    /// After optimization: preserve correctness.
    PostOptimization,
    /// Full verification (all phases).
    Full,
}

impl fmt::Display for VerificationPhase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PostParse => write!(f, "post-parse"),
            Self::PostTypeCheck => write!(f, "post-type-check"),
            Self::PostBorrowCheck => write!(f, "post-borrow-check"),
            Self::PostOptimization => write!(f, "post-optimization"),
            Self::Full => write!(f, "full"),
        }
    }
}

/// Result of running verification in the pipeline.
#[derive(Debug, Clone)]
pub struct PipelineVerificationResult {
    /// Which phase ran.
    pub phase: VerificationPhase,
    /// Total VCs checked.
    pub total_vcs: u64,
    /// VCs proven safe.
    pub proven: u64,
    /// VCs that failed (counterexample found).
    pub failed: u64,
    /// VCs that timed out.
    pub timeouts: u64,
    /// VCs suppressed by user.
    pub suppressed: u64,
    /// Wall-clock time in milliseconds.
    pub elapsed_ms: f64,
    /// Individual failures.
    pub failures: Vec<PipelineFailure>,
    /// Whether verification passed overall.
    pub passed: bool,
}

/// A single failure in the pipeline.
#[derive(Debug, Clone)]
pub struct PipelineFailure {
    /// File path.
    pub file: String,
    /// Line number.
    pub line: u32,
    /// Column (if known).
    pub column: Option<u32>,
    /// Error message.
    pub message: String,
    /// VC kind (e.g., "bounds-check", "overflow").
    pub kind: String,
    /// Severity level.
    pub severity: Severity,
    /// Suggested fix.
    pub fix: Option<String>,
}

/// Severity levels for verification diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// Must fix: counterexample proves the bug.
    Error,
    /// Should fix: solver could not prove safety.
    Warning,
    /// Informational: property inferred automatically.
    Info,
    /// Hint: suggestion to add annotation for better coverage.
    Hint,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Error => write!(f, "error"),
            Self::Warning => write!(f, "warning"),
            Self::Info => write!(f, "info"),
            Self::Hint => write!(f, "hint"),
        }
    }
}

impl PipelineVerificationResult {
    /// Creates a passing result with no VCs.
    pub fn empty(phase: VerificationPhase) -> Self {
        Self {
            phase,
            total_vcs: 0,
            proven: 0,
            failed: 0,
            timeouts: 0,
            suppressed: 0,
            elapsed_ms: 0.0,
            failures: Vec::new(),
            passed: true,
        }
    }

    /// Returns coverage ratio (proven / total, excluding suppressed).
    pub fn coverage(&self) -> f64 {
        let effective = self.total_vcs.saturating_sub(self.suppressed);
        if effective == 0 {
            return 1.0;
        }
        self.proven as f64 / effective as f64
    }
}

impl fmt::Display for PipelineVerificationResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let icon = if self.passed { "PASS" } else { "FAIL" };
        writeln!(
            f,
            "[{icon}] Verification ({}) — {:.1}ms",
            self.phase, self.elapsed_ms
        )?;
        writeln!(
            f,
            "  VCs: {} total, {} proven, {} failed, {} timeout, {} suppressed",
            self.total_vcs, self.proven, self.failed, self.timeouts, self.suppressed
        )?;
        writeln!(f, "  Coverage: {:.1}%", self.coverage() * 100.0)?;
        for failure in &self.failures {
            writeln!(
                f,
                "  [{severity}] {file}:{line}: {msg} ({kind})",
                severity = failure.severity,
                file = failure.file,
                line = failure.line,
                msg = failure.message,
                kind = failure.kind,
            )?;
            if let Some(ref fix) = failure.fix {
                writeln!(f, "    fix: {fix}")?;
            }
        }
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// V7.5: LSP Verification Hints
// ═══════════════════════════════════════════════════════════════════════

/// A verification diagnostic for the LSP to display.
#[derive(Debug, Clone, PartialEq)]
pub struct LspVerifyDiagnostic {
    /// File path.
    pub file: String,
    /// Start line (0-based for LSP).
    pub start_line: u32,
    /// Start column.
    pub start_col: u32,
    /// End line.
    pub end_line: u32,
    /// End column.
    pub end_col: u32,
    /// Severity.
    pub severity: Severity,
    /// Diagnostic message.
    pub message: String,
    /// Diagnostic code (e.g., "FV001").
    pub code: String,
    /// Source identifier.
    pub source: String,
}

impl Default for LspVerifyDiagnostic {
    fn default() -> Self {
        Self {
            file: String::new(),
            start_line: 0,
            start_col: 0,
            end_line: 0,
            end_col: 0,
            severity: Severity::Info,
            message: String::new(),
            code: String::new(),
            source: "fj-verify".to_string(),
        }
    }
}

/// An LSP code action for verification.
#[derive(Debug, Clone, PartialEq)]
pub struct VerifyCodeAction {
    /// Title shown in IDE.
    pub title: String,
    /// Kind of action.
    pub kind: CodeActionKind,
    /// The text to insert/replace.
    pub edit_text: String,
    /// File to edit.
    pub file: String,
    /// Line to insert at.
    pub line: u32,
}

/// Kinds of verification-related code actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodeActionKind {
    /// Add @requires annotation.
    AddRequires,
    /// Add @ensures annotation.
    AddEnsures,
    /// Add @invariant annotation.
    AddInvariant,
    /// Add bounds check guard.
    AddBoundsGuard,
    /// Suppress verification warning.
    SuppressWarning,
}

impl fmt::Display for CodeActionKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AddRequires => write!(f, "add-requires"),
            Self::AddEnsures => write!(f, "add-ensures"),
            Self::AddInvariant => write!(f, "add-invariant"),
            Self::AddBoundsGuard => write!(f, "add-bounds-guard"),
            Self::SuppressWarning => write!(f, "suppress-warning"),
        }
    }
}

/// Generates code actions for a verification diagnostic.
pub fn suggest_code_actions(
    file: &str,
    line: u32,
    kind: &str,
    message: &str,
) -> Vec<VerifyCodeAction> {
    let mut actions = Vec::new();

    match kind {
        "bounds-check" => {
            actions.push(VerifyCodeAction {
                title: "Add bounds check guard".to_string(),
                kind: CodeActionKind::AddBoundsGuard,
                edit_text: "if index < len(arr) { /* ... */ }".to_string(),
                file: file.to_string(),
                line,
            });
            actions.push(VerifyCodeAction {
                title: format!("Suppress: {message}"),
                kind: CodeActionKind::SuppressWarning,
                edit_text: format!("// @verify:suppress(bounds-check) {message}"),
                file: file.to_string(),
                line,
            });
        }
        "overflow" | "no-overflow" => {
            actions.push(VerifyCodeAction {
                title: "Add @requires for operand bounds".to_string(),
                kind: CodeActionKind::AddRequires,
                edit_text: "@requires(a >= 0 && a <= 1000)".to_string(),
                file: file.to_string(),
                line,
            });
        }
        "null-safety" => {
            actions.push(VerifyCodeAction {
                title: "Add null check guard".to_string(),
                kind: CodeActionKind::AddBoundsGuard,
                edit_text: "if let Some(val) = opt { /* ... */ }".to_string(),
                file: file.to_string(),
                line,
            });
        }
        _ => {
            actions.push(VerifyCodeAction {
                title: format!("Suppress: {message}"),
                kind: CodeActionKind::SuppressWarning,
                edit_text: format!("// @verify:suppress({kind}) {message}"),
                file: file.to_string(),
                line,
            });
        }
    }

    actions
}

// ═══════════════════════════════════════════════════════════════════════
// V7.6: CI Integration
// ═══════════════════════════════════════════════════════════════════════

/// CI verification step result.
#[derive(Debug, Clone)]
pub struct CiVerificationResult {
    /// Overall pass/fail.
    pub passed: bool,
    /// Exit code (0 = pass, 1 = fail, 2 = timeout).
    pub exit_code: i32,
    /// Results per file.
    pub file_results: HashMap<String, PipelineVerificationResult>,
    /// Total time in seconds.
    pub total_time_s: f64,
    /// Summary message for CI log.
    pub summary: String,
}

impl CiVerificationResult {
    /// Creates a CI result from individual pipeline results.
    pub fn from_results(
        results: HashMap<String, PipelineVerificationResult>,
        fail_on_timeout: bool,
    ) -> Self {
        let total_time_s: f64 = results.values().map(|r| r.elapsed_ms / 1000.0).sum();
        let any_failed = results.values().any(|r| r.failed > 0);
        let any_timeout = results.values().any(|r| r.timeouts > 0);

        let passed = !any_failed && (!fail_on_timeout || !any_timeout);
        let exit_code = if any_failed {
            1
        } else if any_timeout && fail_on_timeout {
            2
        } else {
            0
        };

        let total_vcs: u64 = results.values().map(|r| r.total_vcs).sum();
        let total_proven: u64 = results.values().map(|r| r.proven).sum();
        let total_failed: u64 = results.values().map(|r| r.failed).sum();

        let summary = format!(
            "Verification: {total_vcs} VCs, {total_proven} proven, {total_failed} failed ({:.1}s)",
            total_time_s
        );

        Self {
            passed,
            exit_code,
            file_results: results,
            total_time_s,
            summary,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// V7.7: REPL Verification
// ═══════════════════════════════════════════════════════════════════════

/// State for incremental REPL verification.
#[derive(Debug, Clone, Default)]
pub struct ReplVerifyState {
    /// Accumulated variable bounds from previous expressions.
    pub known_bounds: HashMap<String, (i64, i64)>,
    /// Functions defined so far with their specs.
    pub function_specs: HashMap<String, Vec<String>>,
    /// Number of VCs checked in this session.
    pub total_vcs_checked: u64,
    /// Number of VCs proven in this session.
    pub total_vcs_proven: u64,
    /// Whether REPL verification is enabled.
    pub enabled: bool,
}

impl ReplVerifyState {
    /// Creates a new REPL verify state.
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            ..Default::default()
        }
    }

    /// Records bounds for a variable (from let binding or assignment).
    pub fn record_bounds(&mut self, var: &str, min: i64, max: i64) {
        self.known_bounds.insert(var.to_string(), (min, max));
    }

    /// Looks up known bounds.
    pub fn get_bounds(&self, var: &str) -> Option<(i64, i64)> {
        self.known_bounds.get(var).copied()
    }

    /// Records a function spec.
    pub fn record_spec(&mut self, function: &str, spec: &str) {
        self.function_specs
            .entry(function.to_string())
            .or_default()
            .push(spec.to_string());
    }

    /// Returns verification hit rate for the session.
    pub fn session_coverage(&self) -> f64 {
        if self.total_vcs_checked == 0 {
            return 1.0;
        }
        self.total_vcs_proven as f64 / self.total_vcs_checked as f64
    }
}

// ═══════════════════════════════════════════════════════════════════════
// V7.8: Verification Diff
// ═══════════════════════════════════════════════════════════════════════

/// Comparison between two verification runs (for CI diffs).
#[derive(Debug, Clone)]
pub struct VerificationDiff {
    /// VCs that were proven before but now fail.
    pub regressions: Vec<String>,
    /// VCs that were failing but now proven.
    pub improvements: Vec<String>,
    /// New VCs not in previous run.
    pub new_vcs: Vec<String>,
    /// VCs removed since previous run.
    pub removed_vcs: Vec<String>,
}

impl VerificationDiff {
    /// Computes the diff between two sets of VC results.
    /// Each map: VC name -> was it proven (true) or failed (false).
    pub fn compute(before: &HashMap<String, bool>, after: &HashMap<String, bool>) -> Self {
        let mut regressions = Vec::new();
        let mut improvements = Vec::new();
        let mut new_vcs = Vec::new();
        let mut removed_vcs = Vec::new();

        for (name, &after_proven) in after {
            match before.get(name) {
                Some(&before_proven) => {
                    if before_proven && !after_proven {
                        regressions.push(name.clone());
                    } else if !before_proven && after_proven {
                        improvements.push(name.clone());
                    }
                }
                None => new_vcs.push(name.clone()),
            }
        }

        for name in before.keys() {
            if !after.contains_key(name) {
                removed_vcs.push(name.clone());
            }
        }

        regressions.sort();
        improvements.sort();
        new_vcs.sort();
        removed_vcs.sort();

        Self {
            regressions,
            improvements,
            new_vcs,
            removed_vcs,
        }
    }

    /// Returns true if there are regressions.
    pub fn has_regressions(&self) -> bool {
        !self.regressions.is_empty()
    }

    /// Returns a human-readable summary.
    pub fn summary(&self) -> String {
        let mut lines = Vec::new();
        if !self.regressions.is_empty() {
            lines.push(format!(
                "Regressions ({}): {}",
                self.regressions.len(),
                self.regressions.join(", ")
            ));
        }
        if !self.improvements.is_empty() {
            lines.push(format!(
                "Improvements ({}): {}",
                self.improvements.len(),
                self.improvements.join(", ")
            ));
        }
        if !self.new_vcs.is_empty() {
            lines.push(format!("New VCs ({})", self.new_vcs.len()));
        }
        if !self.removed_vcs.is_empty() {
            lines.push(format!("Removed VCs ({})", self.removed_vcs.len()));
        }
        if lines.is_empty() {
            "No changes in verification results.".to_string()
        } else {
            lines.join("\n")
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// V7.9-V7.10: Pipeline Runner
// ═══════════════════════════════════════════════════════════════════════

/// The main verification pipeline runner.
#[derive(Debug)]
pub struct VerificationPipeline {
    /// Configuration.
    pub config: VerifyConfig,
    /// Suppressions from source comments.
    pub suppressions: Vec<Suppression>,
    /// REPL state (if in REPL mode).
    pub repl_state: Option<ReplVerifyState>,
    /// Accumulated results.
    pub results: Vec<PipelineVerificationResult>,
}

impl VerificationPipeline {
    /// Creates a new pipeline with default config.
    pub fn new() -> Self {
        Self {
            config: VerifyConfig::default(),
            suppressions: Vec::new(),
            repl_state: None,
            results: Vec::new(),
        }
    }

    /// Creates a pipeline with custom config.
    pub fn with_config(config: VerifyConfig) -> Self {
        Self {
            config,
            suppressions: Vec::new(),
            repl_state: None,
            results: Vec::new(),
        }
    }

    /// Loads suppressions from source code.
    pub fn load_suppressions(&mut self, source: &str, file: &str) {
        let new_suppressions = parse_suppressions(source, file);
        self.suppressions.extend(new_suppressions);
    }

    /// Enables REPL mode.
    pub fn enable_repl(&mut self) {
        self.repl_state = Some(ReplVerifyState::new(true));
    }

    /// Returns the total number of VCs across all results.
    pub fn total_vcs(&self) -> u64 {
        self.results.iter().map(|r| r.total_vcs).sum()
    }

    /// Returns the total proven count.
    pub fn total_proven(&self) -> u64 {
        self.results.iter().map(|r| r.proven).sum()
    }

    /// Returns true if all phases passed.
    pub fn all_passed(&self) -> bool {
        self.results.iter().all(|r| r.passed)
    }

    /// Adds a result from a verification phase.
    pub fn add_result(&mut self, result: PipelineVerificationResult) {
        self.results.push(result);
    }
}

impl Default for VerificationPipeline {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v7_1_verify_config_default() {
        let config = VerifyConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.timeout_ms, 5000);
        assert_eq!(config.solver, "z3");
        assert!(config.auto_infer);
        assert!(config.cache_enabled);
        assert_eq!(config.parallel, 4);
    }

    #[test]
    fn v7_1_verify_config_from_toml() {
        let mut table = HashMap::new();
        table.insert("enabled".to_string(), "true".to_string());
        table.insert("timeout_ms".to_string(), "10000".to_string());
        table.insert("solver".to_string(), "cvc5".to_string());
        table.insert("parallel".to_string(), "8".to_string());
        table.insert("certification".to_string(), "DO-178C".to_string());

        let config = VerifyConfig::from_toml(&table);
        assert!(config.enabled);
        assert_eq!(config.timeout_ms, 10000);
        assert_eq!(config.solver, "cvc5");
        assert_eq!(config.parallel, 8);
        assert_eq!(config.certification, Some("DO-178C".to_string()));
    }

    #[test]
    fn v7_1_should_verify() {
        let mut config = VerifyConfig::default();
        config.enabled = true;
        config.exclude = vec!["test_".to_string(), "vendor/".to_string()];

        assert!(config.should_verify("src/main.fj"));
        assert!(!config.should_verify("test_helper.fj"));
        assert!(!config.should_verify("vendor/lib.fj"));
    }

    #[test]
    fn v7_1_should_verify_disabled() {
        let config = VerifyConfig::default();
        assert!(!config.should_verify("src/main.fj"));
    }

    #[test]
    fn v7_2_verify_command_default() {
        let cmd = VerifyCommand::default();
        assert!(cmd.files.is_empty());
        assert!(!cmd.verbose);
        assert_eq!(cmd.format, OutputFormat::Human);
        assert!(cmd.auto_infer);
    }

    #[test]
    fn v7_2_output_format_display() {
        assert_eq!(format!("{}", OutputFormat::Human), "human");
        assert_eq!(format!("{}", OutputFormat::Json), "json");
        assert_eq!(format!("{}", OutputFormat::Sarif), "sarif");
        assert_eq!(format!("{}", OutputFormat::Csv), "csv");
    }

    #[test]
    fn v7_3_parse_suppressions() {
        let source = r#"
let x = arr[i]
// @verify:suppress(bounds-check) known safe: i from for loop
let y = x / d
// @verify:suppress(division-safety) d is always > 0 from precondition
"#;
        let suppressions = parse_suppressions(source, "main.fj");
        assert_eq!(suppressions.len(), 2);
        assert_eq!(suppressions[0].kind, "bounds-check");
        assert!(suppressions[0].reason.contains("known safe"));
        assert_eq!(suppressions[1].kind, "division-safety");
    }

    #[test]
    fn v7_3_parse_no_suppressions() {
        let source = "let x = 42\nlet y = x + 1\n";
        let suppressions = parse_suppressions(source, "clean.fj");
        assert!(suppressions.is_empty());
    }

    #[test]
    fn v7_3_is_suppressed() {
        let suppressions = vec![Suppression {
            kind: "bounds-check".to_string(),
            reason: "safe".to_string(),
            file: "a.fj".to_string(),
            line: 5,
        }];
        assert!(is_suppressed(&suppressions, "bounds-check", 5));
        assert!(is_suppressed(&suppressions, "bounds-check", 6)); // line after comment
        assert!(!is_suppressed(&suppressions, "bounds-check", 7));
        assert!(!is_suppressed(&suppressions, "overflow", 5));
    }

    #[test]
    fn v7_4_verification_phase_display() {
        assert_eq!(format!("{}", VerificationPhase::PostParse), "post-parse");
        assert_eq!(
            format!("{}", VerificationPhase::PostTypeCheck),
            "post-type-check"
        );
        assert_eq!(format!("{}", VerificationPhase::Full), "full");
    }

    #[test]
    fn v7_4_pipeline_result_empty() {
        let result = PipelineVerificationResult::empty(VerificationPhase::Full);
        assert!(result.passed);
        assert_eq!(result.total_vcs, 0);
        assert!((result.coverage() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn v7_4_pipeline_result_coverage() {
        let result = PipelineVerificationResult {
            phase: VerificationPhase::PostTypeCheck,
            total_vcs: 20,
            proven: 15,
            failed: 3,
            timeouts: 2,
            suppressed: 5,
            elapsed_ms: 500.0,
            failures: vec![],
            passed: false,
        };
        // Coverage: proven / (total - suppressed) = 15 / 15 = 1.0
        assert!((result.coverage() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn v7_4_pipeline_result_display() {
        let result = PipelineVerificationResult {
            phase: VerificationPhase::Full,
            total_vcs: 10,
            proven: 8,
            failed: 1,
            timeouts: 1,
            suppressed: 0,
            elapsed_ms: 250.0,
            failures: vec![PipelineFailure {
                file: "main.fj".to_string(),
                line: 42,
                column: Some(10),
                message: "array index out of bounds".to_string(),
                kind: "bounds-check".to_string(),
                severity: Severity::Error,
                fix: Some("add bounds guard".to_string()),
            }],
            passed: false,
        };
        let s = format!("{result}");
        assert!(s.contains("FAIL"));
        assert!(s.contains("main.fj:42"));
        assert!(s.contains("bounds-check"));
    }

    #[test]
    fn v7_4_severity_display() {
        assert_eq!(format!("{}", Severity::Error), "error");
        assert_eq!(format!("{}", Severity::Warning), "warning");
        assert_eq!(format!("{}", Severity::Info), "info");
        assert_eq!(format!("{}", Severity::Hint), "hint");
    }

    #[test]
    fn v7_5_lsp_diagnostic_default() {
        let diag = LspVerifyDiagnostic::default();
        assert_eq!(diag.source, "fj-verify");
        assert_eq!(diag.severity, Severity::Info);
    }

    #[test]
    fn v7_5_code_action_kind_display() {
        assert_eq!(format!("{}", CodeActionKind::AddRequires), "add-requires");
        assert_eq!(
            format!("{}", CodeActionKind::SuppressWarning),
            "suppress-warning"
        );
    }

    #[test]
    fn v7_5_suggest_code_actions_bounds() {
        let actions = suggest_code_actions("main.fj", 42, "bounds-check", "i may be out of bounds");
        assert_eq!(actions.len(), 2);
        assert_eq!(actions[0].kind, CodeActionKind::AddBoundsGuard);
        assert_eq!(actions[1].kind, CodeActionKind::SuppressWarning);
    }

    #[test]
    fn v7_5_suggest_code_actions_overflow() {
        let actions = suggest_code_actions("math.fj", 10, "overflow", "may overflow");
        assert!(!actions.is_empty());
        assert_eq!(actions[0].kind, CodeActionKind::AddRequires);
    }

    #[test]
    fn v7_6_ci_result_all_pass() {
        let mut file_results = HashMap::new();
        file_results.insert(
            "a.fj".to_string(),
            PipelineVerificationResult {
                phase: VerificationPhase::Full,
                total_vcs: 5,
                proven: 5,
                failed: 0,
                timeouts: 0,
                suppressed: 0,
                elapsed_ms: 100.0,
                failures: vec![],
                passed: true,
            },
        );
        let ci = CiVerificationResult::from_results(file_results, false);
        assert!(ci.passed);
        assert_eq!(ci.exit_code, 0);
        assert!(ci.summary.contains("5 VCs"));
    }

    #[test]
    fn v7_6_ci_result_with_failures() {
        let mut file_results = HashMap::new();
        file_results.insert(
            "b.fj".to_string(),
            PipelineVerificationResult {
                phase: VerificationPhase::Full,
                total_vcs: 10,
                proven: 8,
                failed: 2,
                timeouts: 0,
                suppressed: 0,
                elapsed_ms: 200.0,
                failures: vec![],
                passed: false,
            },
        );
        let ci = CiVerificationResult::from_results(file_results, false);
        assert!(!ci.passed);
        assert_eq!(ci.exit_code, 1);
    }

    #[test]
    fn v7_7_repl_verify_state() {
        let mut state = ReplVerifyState::new(true);
        assert!(state.enabled);

        state.record_bounds("x", 0, 100);
        assert_eq!(state.get_bounds("x"), Some((0, 100)));
        assert_eq!(state.get_bounds("y"), None);

        state.record_spec("foo", "@requires(x > 0)");
        assert_eq!(state.function_specs.get("foo").map(|v| v.len()), Some(1));

        state.total_vcs_checked = 10;
        state.total_vcs_proven = 8;
        assert!((state.session_coverage() - 0.8).abs() < f64::EPSILON);
    }

    #[test]
    fn v7_7_repl_verify_empty_coverage() {
        let state = ReplVerifyState::new(false);
        assert!(!state.enabled);
        assert!((state.session_coverage() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn v7_8_verification_diff_no_changes() {
        let before = HashMap::from([("vc1".to_string(), true), ("vc2".to_string(), false)]);
        let after = HashMap::from([("vc1".to_string(), true), ("vc2".to_string(), false)]);
        let diff = VerificationDiff::compute(&before, &after);
        assert!(!diff.has_regressions());
        assert!(diff.improvements.is_empty());
        assert!(diff.new_vcs.is_empty());
        assert!(diff.removed_vcs.is_empty());
        assert!(diff.summary().contains("No changes"));
    }

    #[test]
    fn v7_8_verification_diff_regression() {
        let before = HashMap::from([("vc1".to_string(), true)]);
        let after = HashMap::from([("vc1".to_string(), false)]);
        let diff = VerificationDiff::compute(&before, &after);
        assert!(diff.has_regressions());
        assert_eq!(diff.regressions, vec!["vc1"]);
    }

    #[test]
    fn v7_8_verification_diff_improvement() {
        let before = HashMap::from([("vc1".to_string(), false)]);
        let after = HashMap::from([("vc1".to_string(), true)]);
        let diff = VerificationDiff::compute(&before, &after);
        assert!(!diff.has_regressions());
        assert_eq!(diff.improvements, vec!["vc1"]);
    }

    #[test]
    fn v7_8_verification_diff_new_and_removed() {
        let before = HashMap::from([("old_vc".to_string(), true)]);
        let after = HashMap::from([("new_vc".to_string(), true)]);
        let diff = VerificationDiff::compute(&before, &after);
        assert_eq!(diff.new_vcs, vec!["new_vc"]);
        assert_eq!(diff.removed_vcs, vec!["old_vc"]);
        let summary = diff.summary();
        assert!(summary.contains("New VCs"));
        assert!(summary.contains("Removed VCs"));
    }

    #[test]
    fn v7_9_pipeline_new() {
        let pipeline = VerificationPipeline::new();
        assert!(!pipeline.config.enabled);
        assert!(pipeline.suppressions.is_empty());
        assert!(pipeline.repl_state.is_none());
        assert_eq!(pipeline.total_vcs(), 0);
        assert!(pipeline.all_passed());
    }

    #[test]
    fn v7_9_pipeline_with_config() {
        let config = VerifyConfig {
            enabled: true,
            ..VerifyConfig::default()
        };
        let pipeline = VerificationPipeline::with_config(config);
        assert!(pipeline.config.enabled);
    }

    #[test]
    fn v7_9_pipeline_load_suppressions() {
        let mut pipeline = VerificationPipeline::new();
        let source = "// @verify:suppress(overflow) safe range\nlet x = a + b\n";
        pipeline.load_suppressions(source, "calc.fj");
        assert_eq!(pipeline.suppressions.len(), 1);
    }

    #[test]
    fn v7_10_pipeline_add_result() {
        let mut pipeline = VerificationPipeline::new();
        pipeline.add_result(PipelineVerificationResult {
            phase: VerificationPhase::PostTypeCheck,
            total_vcs: 10,
            proven: 8,
            failed: 2,
            timeouts: 0,
            suppressed: 0,
            elapsed_ms: 100.0,
            failures: vec![],
            passed: false,
        });
        assert_eq!(pipeline.total_vcs(), 10);
        assert_eq!(pipeline.total_proven(), 8);
        assert!(!pipeline.all_passed());
    }

    #[test]
    fn v7_10_pipeline_enable_repl() {
        let mut pipeline = VerificationPipeline::new();
        assert!(pipeline.repl_state.is_none());
        pipeline.enable_repl();
        assert!(pipeline.repl_state.is_some());
    }

    #[test]
    fn v7_pipeline_default_trait() {
        let pipeline = VerificationPipeline::default();
        assert_eq!(pipeline.total_vcs(), 0);
    }
}
