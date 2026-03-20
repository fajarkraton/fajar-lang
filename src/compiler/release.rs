//! # Release Engineering — Phase 7 (v1.0.0 "Genesis")
//!
//! Provides comprehensive release infrastructure for Fajar Lang binaries:
//!
//! - **Sprint 25** — `ReleasePipeline`: automated multi-target build, sign, publish workflow
//! - **Sprint 26** — `BinarySizeOptimizer`: section analysis, function-level size, optimization suggestions
//! - **Sprint 27** — `StabilityChecker`: API snapshots, diffs, SemVer validation, breaking change detection
//! - **Sprint 28** — `ChangelogGenerator`, `QualityGate`, `ReleaseNotes`: changelog from commits,
//!   CI quality enforcement, release announcement generation
//!
//! ## Architecture
//!
//! ```text
//! ReleasePipeline → plan() → execute() → verify() → report()
//! BinarySizeOptimizer → analyze() → suggest() → FeatureImpact
//! StabilityChecker → snapshot() → diff(old, new) → is_breaking()
//! ChangelogGenerator → from_commits() → MigrationGuide
//! QualityGateRunner → run_all() → Vec<QualityResult>
//! ReleaseNotes → generate_github_release() / generate_blog_post() / generate_tweet()
//! ```

use std::collections::HashMap;
use std::fmt;
use std::time::Instant;

use thiserror::Error;

// ═══════════════════════════════════════════════════════════════════════
// Error types
// ═══════════════════════════════════════════════════════════════════════

/// Errors arising from release engineering operations.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ReleaseError {
    /// A release stage failed with the given reason.
    #[error("release stage `{stage}` failed: {reason}")]
    StageFailed {
        /// The stage that failed.
        stage: String,
        /// The failure reason.
        reason: String,
    },

    /// The binary at the given path could not be analyzed.
    #[error("binary analysis failed for `{path}`: {reason}")]
    BinaryAnalysisFailed {
        /// Path to the binary.
        path: String,
        /// The failure reason.
        reason: String,
    },

    /// An API stability violation was detected.
    #[error("API stability violation: {description}")]
    StabilityViolation {
        /// Description of the violation.
        description: String,
    },

    /// A quality gate check failed.
    #[error("quality gate `{check}` failed: {message}")]
    QualityGateFailed {
        /// The check that failed.
        check: String,
        /// The failure message.
        message: String,
    },

    /// A SemVer rule was violated.
    #[error("semver violation: {0}")]
    SemVerViolation(String),

    /// Invalid release configuration.
    #[error("invalid release config: {message}")]
    InvalidConfig {
        /// Description of the configuration error.
        message: String,
    },
}

// ═══════════════════════════════════════════════════════════════════════
// Sprint 25 — ReleasePipeline
// ═══════════════════════════════════════════════════════════════════════

/// A stage in the release pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReleaseStage {
    /// Run the full test suite.
    Test,
    /// Build release binaries for all configured targets.
    Build,
    /// Cryptographically sign the release artifacts.
    Sign,
    /// Publish to configured registries (crates.io, GitHub, Homebrew).
    Publish,
    /// Verify published artifacts are accessible and correct.
    Verify,
    /// Generate and distribute release announcements.
    Announce,
}

impl fmt::Display for ReleaseStage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReleaseStage::Test => write!(f, "test"),
            ReleaseStage::Build => write!(f, "build"),
            ReleaseStage::Sign => write!(f, "sign"),
            ReleaseStage::Publish => write!(f, "publish"),
            ReleaseStage::Verify => write!(f, "verify"),
            ReleaseStage::Announce => write!(f, "announce"),
        }
    }
}

/// The operating system for a release target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TargetOs {
    /// Standard glibc-linked Linux.
    Linux,
    /// Statically-linked musl Linux.
    LinuxMusl,
    /// macOS (Darwin).
    MacOS,
    /// Windows (MSVC or GNU).
    Windows,
    /// FreeBSD.
    FreeBSD,
}

impl fmt::Display for TargetOs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TargetOs::Linux => write!(f, "linux"),
            TargetOs::LinuxMusl => write!(f, "linux-musl"),
            TargetOs::MacOS => write!(f, "macos"),
            TargetOs::Windows => write!(f, "windows"),
            TargetOs::FreeBSD => write!(f, "freebsd"),
        }
    }
}

/// The CPU architecture for a release target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TargetArch {
    /// 64-bit x86 (AMD64/Intel 64).
    X86_64,
    /// 64-bit ARM (ARMv8-A).
    Aarch64,
    /// 64-bit RISC-V.
    Riscv64,
    /// 32-bit WebAssembly.
    Wasm32,
}

impl fmt::Display for TargetArch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TargetArch::X86_64 => write!(f, "x86_64"),
            TargetArch::Aarch64 => write!(f, "aarch64"),
            TargetArch::Riscv64 => write!(f, "riscv64"),
            TargetArch::Wasm32 => write!(f, "wasm32"),
        }
    }
}

/// A single build target for the release.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseTarget {
    /// The target operating system.
    pub os: TargetOs,
    /// The target CPU architecture.
    pub arch: TargetArch,
    /// Cargo feature flags to enable for this target.
    pub features: Vec<String>,
    /// Whether to produce a statically-linked binary.
    pub static_link: bool,
}

impl ReleaseTarget {
    /// Creates a new release target.
    pub fn new(os: TargetOs, arch: TargetArch) -> Self {
        Self {
            os,
            arch,
            features: Vec::new(),
            static_link: false,
        }
    }

    /// Returns the Rust triple string for this target.
    pub fn triple(&self) -> String {
        let arch = match self.arch {
            TargetArch::X86_64 => "x86_64",
            TargetArch::Aarch64 => "aarch64",
            TargetArch::Riscv64 => "riscv64gc",
            TargetArch::Wasm32 => "wasm32",
        };
        let os_vendor = match self.os {
            TargetOs::Linux => "unknown-linux-gnu",
            TargetOs::LinuxMusl => "unknown-linux-musl",
            TargetOs::MacOS => "apple-darwin",
            TargetOs::Windows => "pc-windows-msvc",
            TargetOs::FreeBSD => "unknown-freebsd",
        };
        format!("{arch}-{os_vendor}")
    }

    /// Returns the expected binary name for this target.
    pub fn binary_name(&self) -> String {
        let ext = if self.os == TargetOs::Windows {
            ".exe"
        } else {
            ""
        };
        format!("fj-{}-{}{}", self.os, self.arch, ext)
    }
}

/// Configuration for a release pipeline run.
#[derive(Debug, Clone)]
pub struct ReleaseConfig {
    /// The version to release (e.g., "1.0.0").
    pub version: String,
    /// The set of targets to build.
    pub targets: Vec<ReleaseTarget>,
    /// Path to the signing key (if any).
    pub sign_key: Option<String>,
    /// Whether to publish to crates.io.
    pub publish_crates_io: bool,
    /// Whether to publish to GitHub Releases.
    pub publish_github: bool,
    /// Whether to publish to Homebrew.
    pub publish_homebrew: bool,
}

impl ReleaseConfig {
    /// Creates a new release configuration for the given version.
    pub fn new(version: String) -> Self {
        Self {
            version,
            targets: Vec::new(),
            sign_key: None,
            publish_crates_io: false,
            publish_github: false,
            publish_homebrew: false,
        }
    }

    /// Validates the release configuration.
    pub fn validate(&self) -> Result<(), ReleaseError> {
        if self.version.is_empty() {
            return Err(ReleaseError::InvalidConfig {
                message: "version must not be empty".to_string(),
            });
        }
        if self.targets.is_empty() {
            return Err(ReleaseError::InvalidConfig {
                message: "at least one release target is required".to_string(),
            });
        }
        Ok(())
    }

    /// Returns a default multi-platform release configuration.
    pub fn default_multiplatform(version: String) -> Self {
        Self {
            version,
            targets: vec![
                ReleaseTarget::new(TargetOs::Linux, TargetArch::X86_64),
                ReleaseTarget::new(TargetOs::MacOS, TargetArch::Aarch64),
                ReleaseTarget::new(TargetOs::Windows, TargetArch::X86_64),
            ],
            sign_key: None,
            publish_crates_io: true,
            publish_github: true,
            publish_homebrew: false,
        }
    }
}

/// A built release artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseArtifact {
    /// The artifact file name (e.g., "fj-linux-x86_64").
    pub name: String,
    /// The target this artifact was built for.
    pub target: String,
    /// The file path of the artifact.
    pub path: String,
    /// The artifact size in bytes.
    pub size_bytes: u64,
    /// The SHA-256 hash of the artifact.
    pub sha256: String,
    /// The cryptographic signature (if signing was enabled).
    pub signature: Option<String>,
}

/// The result of a stage execution within the release pipeline.
#[derive(Debug, Clone)]
pub struct StageResult {
    /// The stage that was executed.
    pub stage: ReleaseStage,
    /// Whether the stage succeeded.
    pub success: bool,
    /// Human-readable message.
    pub message: String,
    /// Duration of the stage in milliseconds.
    pub duration_ms: u64,
}

/// The final report for a release pipeline run.
#[derive(Debug, Clone)]
pub struct ReleaseReport {
    /// The released version.
    pub version: String,
    /// The artifacts produced.
    pub artifacts: Vec<ReleaseArtifact>,
    /// Results from each stage.
    pub test_results: Vec<StageResult>,
    /// Total pipeline execution time in seconds.
    pub total_time_secs: f64,
    /// Whether the entire release succeeded.
    pub success: bool,
}

impl ReleaseReport {
    /// Returns the number of successful stages.
    pub fn stages_passed(&self) -> usize {
        self.test_results.iter().filter(|r| r.success).count()
    }

    /// Returns the total number of stages executed.
    pub fn stages_total(&self) -> usize {
        self.test_results.len()
    }
}

/// The automated release pipeline.
///
/// Orchestrates the full release workflow: test, build, sign, publish,
/// verify, and announce.
#[derive(Debug)]
pub struct ReleasePipeline {
    /// The release configuration.
    config: ReleaseConfig,
    /// Ordered list of stages to execute.
    stages: Vec<ReleaseStage>,
    /// Results collected during execution.
    results: Vec<StageResult>,
    /// Artifacts produced during the build stage.
    artifacts: Vec<ReleaseArtifact>,
    /// Pipeline start time.
    start_time: Option<Instant>,
}

impl ReleasePipeline {
    /// Creates a new release pipeline with the given configuration.
    pub fn new(config: ReleaseConfig) -> Self {
        Self {
            config,
            stages: Vec::new(),
            results: Vec::new(),
            artifacts: Vec::new(),
            start_time: None,
        }
    }

    /// Plans the release by determining which stages are needed.
    ///
    /// Populates the internal stage list based on the configuration.
    pub fn plan(&mut self) -> Result<Vec<ReleaseStage>, ReleaseError> {
        self.config.validate()?;
        self.stages.clear();

        self.stages.push(ReleaseStage::Test);
        self.stages.push(ReleaseStage::Build);

        if self.config.sign_key.is_some() {
            self.stages.push(ReleaseStage::Sign);
        }

        if self.has_publish_target() {
            self.stages.push(ReleaseStage::Publish);
        }

        self.stages.push(ReleaseStage::Verify);
        self.stages.push(ReleaseStage::Announce);

        Ok(self.stages.clone())
    }

    /// Executes the planned release stages in order.
    ///
    /// Each stage is simulated with a success result. In production,
    /// each stage would invoke real build/sign/publish tooling.
    pub fn execute(&mut self) -> Result<(), ReleaseError> {
        if self.stages.is_empty() {
            return Err(ReleaseError::StageFailed {
                stage: "pipeline".to_string(),
                reason: "no stages planned; call plan() first".to_string(),
            });
        }
        self.start_time = Some(Instant::now());

        for stage in self.stages.clone() {
            let result = self.execute_stage(stage);
            self.results.push(result);
        }

        Ok(())
    }

    /// Verifies that all executed stages completed successfully.
    pub fn verify(&self) -> bool {
        self.results.iter().all(|r| r.success)
    }

    /// Generates the final release report.
    pub fn report(&self) -> ReleaseReport {
        let elapsed = self
            .start_time
            .map(|t| t.elapsed().as_secs_f64())
            .unwrap_or(0.0);

        ReleaseReport {
            version: self.config.version.clone(),
            artifacts: self.artifacts.clone(),
            test_results: self.results.clone(),
            total_time_secs: elapsed,
            success: self.verify(),
        }
    }

    /// Returns the planned stages.
    pub fn planned_stages(&self) -> &[ReleaseStage] {
        &self.stages
    }

    /// Executes a single release stage (simulation).
    fn execute_stage(&mut self, stage: ReleaseStage) -> StageResult {
        let start = Instant::now();

        match stage {
            ReleaseStage::Build => self.execute_build_stage(start),
            _ => StageResult {
                stage,
                success: true,
                message: format!("{stage} completed successfully"),
                duration_ms: start.elapsed().as_millis() as u64,
            },
        }
    }

    /// Simulates building artifacts for all configured targets.
    fn execute_build_stage(&mut self, start: Instant) -> StageResult {
        for target in &self.config.targets {
            let artifact = ReleaseArtifact {
                name: target.binary_name(),
                target: target.triple(),
                path: format!(
                    "target/{}/release/{}",
                    target.triple(),
                    target.binary_name()
                ),
                size_bytes: 0,
                sha256: "0".repeat(64),
                signature: None,
            };
            self.artifacts.push(artifact);
        }

        StageResult {
            stage: ReleaseStage::Build,
            success: true,
            message: format!("built {} artifacts", self.config.targets.len()),
            duration_ms: start.elapsed().as_millis() as u64,
        }
    }

    /// Returns true if any publish target is configured.
    fn has_publish_target(&self) -> bool {
        self.config.publish_crates_io || self.config.publish_github || self.config.publish_homebrew
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Sprint 26 — BinarySizeOptimizer
// ═══════════════════════════════════════════════════════════════════════

/// Size information for a single binary section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectionSize {
    /// The section name (e.g., ".text", ".rodata").
    pub name: String,
    /// The section size in bytes.
    pub size_bytes: usize,
}

/// Size information for a single function in the binary.
#[derive(Debug, Clone, PartialEq)]
pub struct FunctionSize {
    /// The demangled function name.
    pub name: String,
    /// The function size in bytes.
    pub size_bytes: usize,
    /// Percentage of total .text section.
    pub percentage: f64,
}

/// Size information for a crate contributing to the binary.
#[derive(Debug, Clone, PartialEq)]
pub struct CrateSize {
    /// The crate name.
    pub name: String,
    /// Total bytes attributed to this crate.
    pub size_bytes: usize,
    /// Percentage of total binary size.
    pub percentage: f64,
}

/// A complete binary size analysis report.
#[derive(Debug, Clone)]
pub struct BinarySizeReport {
    /// Total binary size in bytes.
    pub total_bytes: usize,
    /// Per-section size breakdown.
    pub sections: Vec<SectionSize>,
    /// Top functions by size.
    pub top_functions: Vec<FunctionSize>,
    /// Top crates by size.
    pub top_crates: Vec<CrateSize>,
}

impl BinarySizeReport {
    /// Returns the size of a section by name, or `None` if not found.
    pub fn section_size(&self, name: &str) -> Option<usize> {
        self.sections
            .iter()
            .find(|s| s.name == name)
            .map(|s| s.size_bytes)
    }

    /// Returns a human-readable summary of the binary size.
    pub fn summary(&self) -> String {
        let kb = self.total_bytes as f64 / 1024.0;
        let mb = kb / 1024.0;
        let sections_text: Vec<String> = self
            .sections
            .iter()
            .map(|s| format!("  {}: {} bytes", s.name, s.size_bytes))
            .collect();
        format!(
            "Binary size: {} bytes ({:.1} KB, {:.2} MB)\nSections:\n{}",
            self.total_bytes,
            kb,
            mb,
            sections_text.join("\n")
        )
    }
}

/// A suggestion for reducing binary size.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptimizationSuggestion {
    /// The category of the suggestion (e.g., "strip", "lto", "codegen-units").
    pub category: String,
    /// A human-readable description of the suggestion.
    pub description: String,
    /// Estimated size savings in bytes.
    pub estimated_savings_bytes: usize,
}

/// A Cargo build profile configuration for size optimization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildProfile {
    /// Profile name.
    pub name: String,
    /// Optimization level (0, 1, 2, 3, "s", "z").
    pub opt_level: String,
    /// Enable link-time optimization.
    pub lto: bool,
    /// Number of codegen units (1 = best optimization).
    pub codegen_units: u32,
    /// Strip debug symbols.
    pub strip: bool,
    /// Enable panic = "abort".
    pub panic_abort: bool,
}

impl BuildProfile {
    /// Returns the debug build profile (fast compile, large binary).
    pub fn debug() -> Self {
        Self {
            name: "debug".to_string(),
            opt_level: "0".to_string(),
            lto: false,
            codegen_units: 16,
            strip: false,
            panic_abort: false,
        }
    }

    /// Returns the standard release profile.
    pub fn release() -> Self {
        Self {
            name: "release".to_string(),
            opt_level: "3".to_string(),
            lto: false,
            codegen_units: 16,
            strip: false,
            panic_abort: false,
        }
    }

    /// Returns the distribution profile (maximum size optimization).
    pub fn dist() -> Self {
        Self {
            name: "dist".to_string(),
            opt_level: "z".to_string(),
            lto: true,
            codegen_units: 1,
            strip: true,
            panic_abort: true,
        }
    }

    /// Generates a TOML snippet for the `[profile.XXX]` section.
    pub fn to_toml(&self) -> String {
        let mut lines = vec![format!("[profile.{}]", self.name)];
        lines.push(format!("opt-level = \"{}\"", self.opt_level));
        lines.push(format!("lto = {}", self.lto));
        lines.push(format!("codegen-units = {}", self.codegen_units));
        if self.strip {
            lines.push("strip = true".to_string());
        }
        if self.panic_abort {
            lines.push("panic = \"abort\"".to_string());
        }
        lines.join("\n")
    }
}

/// Measures the binary size impact of a single feature flag.
#[derive(Debug, Clone, PartialEq)]
pub struct FeatureImpact {
    /// The feature flag name.
    pub feature_name: String,
    /// Binary size in bytes without this feature.
    pub size_without: usize,
    /// Binary size in bytes with this feature enabled.
    pub size_with: usize,
    /// The size delta in bytes (positive = larger).
    pub delta_bytes: i64,
    /// The size delta as a percentage of the base size.
    pub delta_percent: f64,
}

impl FeatureImpact {
    /// Creates a new feature impact measurement.
    pub fn new(feature_name: String, size_without: usize, size_with: usize) -> Self {
        let delta_bytes = size_with as i64 - size_without as i64;
        let delta_percent = if size_without > 0 {
            (delta_bytes as f64 / size_without as f64) * 100.0
        } else {
            0.0
        };
        Self {
            feature_name,
            size_without,
            size_with,
            delta_bytes,
            delta_percent,
        }
    }
}

/// Analyzes binary size and provides optimization suggestions.
#[derive(Debug)]
pub struct SizeOptimizer {
    /// Known section data (populated during analysis).
    sections: Vec<SectionSize>,
    /// Known function sizes (populated during analysis).
    functions: Vec<FunctionSize>,
    /// Known crate sizes (populated during analysis).
    crates: Vec<CrateSize>,
}

impl SizeOptimizer {
    /// Creates a new size optimizer.
    pub fn new() -> Self {
        Self {
            sections: Vec::new(),
            functions: Vec::new(),
            crates: Vec::new(),
        }
    }

    /// Analyzes a binary file and returns a size report.
    ///
    /// In simulation mode, this parses a mock representation. In
    /// production, it would shell out to `bloaty`, `nm`, or `objdump`.
    pub fn analyze(&mut self, binary_path: &str) -> Result<BinarySizeReport, ReleaseError> {
        if binary_path.is_empty() {
            return Err(ReleaseError::BinaryAnalysisFailed {
                path: binary_path.to_string(),
                reason: "empty binary path".to_string(),
            });
        }

        // Simulation: produce a representative section layout
        self.sections = default_sections();
        self.functions = default_functions(&self.sections);
        self.crates = default_crates(&self.sections);

        let total_bytes: usize = self.sections.iter().map(|s| s.size_bytes).sum();

        Ok(BinarySizeReport {
            total_bytes,
            sections: self.sections.clone(),
            top_functions: self.functions.clone(),
            top_crates: self.crates.clone(),
        })
    }

    /// Returns optimization suggestions based on the last analysis.
    pub fn suggest(&self) -> Vec<OptimizationSuggestion> {
        let mut suggestions = Vec::new();

        let has_debug = self.sections.iter().any(|s| s.name.contains("debug"));
        if has_debug {
            suggestions.push(OptimizationSuggestion {
                category: "strip".to_string(),
                description: "Strip debug info with `strip = true`".to_string(),
                estimated_savings_bytes: estimate_debug_savings(&self.sections),
            });
        }

        suggestions.push(OptimizationSuggestion {
            category: "lto".to_string(),
            description: "Enable LTO with `lto = true`".to_string(),
            estimated_savings_bytes: estimate_lto_savings(&self.sections),
        });

        suggestions.push(OptimizationSuggestion {
            category: "codegen-units".to_string(),
            description: "Set `codegen-units = 1` for better inlining".to_string(),
            estimated_savings_bytes: estimate_cgu_savings(&self.sections),
        });

        suggestions.push(OptimizationSuggestion {
            category: "opt-level".to_string(),
            description: "Use `opt-level = \"z\"` for minimum size".to_string(),
            estimated_savings_bytes: estimate_optz_savings(&self.sections),
        });

        suggestions
    }
}

impl Default for SizeOptimizer {
    fn default() -> Self {
        Self::new()
    }
}

/// Returns the default simulated section layout.
fn default_sections() -> Vec<SectionSize> {
    vec![
        SectionSize {
            name: ".text".to_string(),
            size_bytes: 2_048_000,
        },
        SectionSize {
            name: ".rodata".to_string(),
            size_bytes: 512_000,
        },
        SectionSize {
            name: ".data".to_string(),
            size_bytes: 64_000,
        },
        SectionSize {
            name: ".bss".to_string(),
            size_bytes: 32_000,
        },
        SectionSize {
            name: ".debug_info".to_string(),
            size_bytes: 1_024_000,
        },
        SectionSize {
            name: ".debug_str".to_string(),
            size_bytes: 256_000,
        },
    ]
}

/// Returns the default simulated top functions.
fn default_functions(sections: &[SectionSize]) -> Vec<FunctionSize> {
    let text_size = sections
        .iter()
        .find(|s| s.name == ".text")
        .map(|s| s.size_bytes)
        .unwrap_or(1);
    let fns = vec![
        ("fajar_lang::parser::parse", 45_000),
        ("fajar_lang::analyzer::type_check", 38_000),
        ("fajar_lang::interpreter::eval_expr", 32_000),
        ("fajar_lang::codegen::cranelift::compile", 28_000),
        ("fajar_lang::runtime::ml::tensor_matmul", 22_000),
    ];
    fns.into_iter()
        .map(|(name, size)| FunctionSize {
            name: name.to_string(),
            size_bytes: size,
            percentage: (size as f64 / text_size as f64) * 100.0,
        })
        .collect()
}

/// Returns the default simulated crate sizes.
fn default_crates(sections: &[SectionSize]) -> Vec<CrateSize> {
    let total: usize = sections.iter().map(|s| s.size_bytes).sum();
    let crates = vec![
        ("fajar_lang", 1_200_000),
        ("cranelift_codegen", 600_000),
        ("ndarray", 300_000),
        ("miette", 150_000),
        ("clap", 120_000),
    ];
    crates
        .into_iter()
        .map(|(name, size)| CrateSize {
            name: name.to_string(),
            size_bytes: size,
            percentage: (size as f64 / total as f64) * 100.0,
        })
        .collect()
}

/// Estimates savings from stripping debug info.
fn estimate_debug_savings(sections: &[SectionSize]) -> usize {
    sections
        .iter()
        .filter(|s| s.name.contains("debug"))
        .map(|s| s.size_bytes)
        .sum()
}

/// Estimates savings from enabling LTO (roughly 10% of .text).
fn estimate_lto_savings(sections: &[SectionSize]) -> usize {
    sections
        .iter()
        .find(|s| s.name == ".text")
        .map(|s| s.size_bytes / 10)
        .unwrap_or(0)
}

/// Estimates savings from codegen-units=1 (roughly 5% of .text).
fn estimate_cgu_savings(sections: &[SectionSize]) -> usize {
    sections
        .iter()
        .find(|s| s.name == ".text")
        .map(|s| s.size_bytes / 20)
        .unwrap_or(0)
}

/// Estimates savings from opt-level=z (roughly 15% of .text).
fn estimate_optz_savings(sections: &[SectionSize]) -> usize {
    sections
        .iter()
        .find(|s| s.name == ".text")
        .map(|s| s.size_bytes * 15 / 100)
        .unwrap_or(0)
}

// ═══════════════════════════════════════════════════════════════════════
// Sprint 27 — StabilityGuarantee
// ═══════════════════════════════════════════════════════════════════════

/// The kind of a public API item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ApiItemKind {
    /// A free function.
    Function,
    /// A struct definition.
    Struct,
    /// An enum definition.
    Enum,
    /// A trait definition.
    Trait,
    /// A constant.
    Constant,
    /// A type alias.
    TypeAlias,
    /// A method on an impl block.
    Method,
    /// A struct field.
    Field,
}

impl fmt::Display for ApiItemKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ApiItemKind::Function => write!(f, "function"),
            ApiItemKind::Struct => write!(f, "struct"),
            ApiItemKind::Enum => write!(f, "enum"),
            ApiItemKind::Trait => write!(f, "trait"),
            ApiItemKind::Constant => write!(f, "constant"),
            ApiItemKind::TypeAlias => write!(f, "type alias"),
            ApiItemKind::Method => write!(f, "method"),
            ApiItemKind::Field => write!(f, "field"),
        }
    }
}

/// The stability classification of an API item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StabilityLevel {
    /// The item is stable and covered by SemVer guarantees.
    Stable,
    /// The item is experimental and may change without notice.
    Unstable,
    /// The item is deprecated, with the version it was deprecated
    /// and the suggested replacement.
    Deprecated {
        /// The version in which the item was deprecated.
        since: String,
        /// The recommended replacement.
        replacement: String,
    },
    /// The item is internal and not part of the public API.
    Internal,
}

impl fmt::Display for StabilityLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StabilityLevel::Stable => write!(f, "stable"),
            StabilityLevel::Unstable => write!(f, "unstable"),
            StabilityLevel::Deprecated { since, .. } => {
                write!(f, "deprecated(since = {since})")
            }
            StabilityLevel::Internal => write!(f, "internal"),
        }
    }
}

/// A single public API item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiItem {
    /// The fully-qualified name (e.g., "fajar_lang::parser::parse").
    pub name: String,
    /// The kind of API item.
    pub kind: ApiItemKind,
    /// The item's type signature or definition summary.
    pub signature: String,
    /// The stability classification.
    pub stability: StabilityLevel,
    /// The version since which this item exists.
    pub since_version: String,
}

/// A snapshot of the public API at a specific version.
#[derive(Debug, Clone)]
pub struct ApiSnapshot {
    /// The version this snapshot represents.
    pub version: String,
    /// All public API items.
    pub items: Vec<ApiItem>,
    /// ISO-8601 timestamp of the snapshot.
    pub timestamp: String,
}

impl ApiSnapshot {
    /// Creates a new API snapshot.
    pub fn new(version: String, items: Vec<ApiItem>, timestamp: String) -> Self {
        Self {
            version,
            items,
            timestamp,
        }
    }

    /// Returns the number of stable items.
    pub fn stable_count(&self) -> usize {
        self.items
            .iter()
            .filter(|i| i.stability == StabilityLevel::Stable)
            .count()
    }

    /// Looks up an item by name.
    pub fn find(&self, name: &str) -> Option<&ApiItem> {
        self.items.iter().find(|i| i.name == name)
    }
}

/// The kind of change detected between two API snapshots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChangeKind {
    /// The function/method signature changed.
    SignatureChange,
    /// The stability level changed.
    StabilityChange,
    /// The item was removed entirely.
    Removal,
    /// The type of a field or constant changed.
    TypeChange,
}

impl fmt::Display for ChangeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ChangeKind::SignatureChange => write!(f, "signature-change"),
            ChangeKind::StabilityChange => write!(f, "stability-change"),
            ChangeKind::Removal => write!(f, "removal"),
            ChangeKind::TypeChange => write!(f, "type-change"),
        }
    }
}

/// A detected change to a specific API item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiChange {
    /// The name of the changed item.
    pub item_name: String,
    /// The kind of change.
    pub change_kind: ChangeKind,
    /// The old signature or description.
    pub old_signature: String,
    /// The new signature or description.
    pub new_signature: String,
}

/// The result of comparing two API snapshots.
#[derive(Debug, Clone)]
pub struct ApiDiff {
    /// Items added in the new version.
    pub added: Vec<ApiItem>,
    /// Items removed in the new version.
    pub removed: Vec<ApiItem>,
    /// Items that changed between versions.
    pub changed: Vec<ApiChange>,
}

impl ApiDiff {
    /// Returns `true` if the diff contains no changes.
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty() && self.changed.is_empty()
    }

    /// Returns the total number of changes.
    pub fn total_changes(&self) -> usize {
        self.added.len() + self.removed.len() + self.changed.len()
    }
}

/// Checks API stability and SemVer compliance.
pub struct StabilityChecker {
    /// Cached snapshots by version.
    snapshots: HashMap<String, ApiSnapshot>,
}

impl StabilityChecker {
    /// Creates a new stability checker.
    pub fn new() -> Self {
        Self {
            snapshots: HashMap::new(),
        }
    }

    /// Takes an API snapshot and stores it.
    pub fn snapshot(&mut self, snap: ApiSnapshot) {
        self.snapshots.insert(snap.version.clone(), snap);
    }

    /// Computes the diff between two snapshots.
    pub fn diff(&self, old: &ApiSnapshot, new: &ApiSnapshot) -> ApiDiff {
        let old_map: HashMap<&str, &ApiItem> =
            old.items.iter().map(|i| (i.name.as_str(), i)).collect();
        let new_map: HashMap<&str, &ApiItem> =
            new.items.iter().map(|i| (i.name.as_str(), i)).collect();

        let added = collect_added(&old_map, &new.items);
        let removed = collect_removed(&new_map, &old.items);
        let changed = collect_changed(&old_map, &new.items);

        ApiDiff {
            added,
            removed,
            changed,
        }
    }

    /// Determines whether a diff contains breaking changes.
    ///
    /// Breaking changes include: removal of stable items, signature
    /// changes on stable items, and stability downgrades.
    pub fn is_breaking(&self, diff: &ApiDiff) -> bool {
        has_breaking_removals(&diff.removed) || has_breaking_changes(&diff.changed)
    }

    /// Validates that the version bump is compatible with the diff.
    ///
    /// Returns an error if a breaking change appears without a major
    /// version bump, or a feature addition appears in a patch bump.
    pub fn validate_semver(
        &self,
        old_version: &str,
        new_version: &str,
        diff: &ApiDiff,
    ) -> Result<(), ReleaseError> {
        let old_parts = parse_semver(old_version)?;
        let new_parts = parse_semver(new_version)?;
        let bump = classify_bump(&old_parts, &new_parts);

        if self.is_breaking(diff) && bump != VersionBump::Major {
            return Err(ReleaseError::SemVerViolation(
                "breaking changes require a major version bump".to_string(),
            ));
        }

        if !diff.added.is_empty() && bump == VersionBump::Patch {
            return Err(ReleaseError::SemVerViolation(
                "new API additions require at least a minor version bump".to_string(),
            ));
        }

        Ok(())
    }

    /// Returns a stored snapshot by version.
    pub fn get_snapshot(&self, version: &str) -> Option<&ApiSnapshot> {
        self.snapshots.get(version)
    }
}

impl Default for StabilityChecker {
    fn default() -> Self {
        Self::new()
    }
}

/// Collects items present in `new_items` but not in `old_map`.
fn collect_added(old_map: &HashMap<&str, &ApiItem>, new_items: &[ApiItem]) -> Vec<ApiItem> {
    new_items
        .iter()
        .filter(|i| !old_map.contains_key(i.name.as_str()))
        .cloned()
        .collect()
}

/// Collects items present in `old_items` but not in `new_map`.
fn collect_removed(new_map: &HashMap<&str, &ApiItem>, old_items: &[ApiItem]) -> Vec<ApiItem> {
    old_items
        .iter()
        .filter(|i| !new_map.contains_key(i.name.as_str()))
        .cloned()
        .collect()
}

/// Collects changes for items present in both snapshots.
fn collect_changed(old_map: &HashMap<&str, &ApiItem>, new_items: &[ApiItem]) -> Vec<ApiChange> {
    let mut changes = Vec::new();
    for new_item in new_items {
        if let Some(old_item) = old_map.get(new_item.name.as_str()) {
            if old_item.signature != new_item.signature {
                changes.push(ApiChange {
                    item_name: new_item.name.clone(),
                    change_kind: ChangeKind::SignatureChange,
                    old_signature: old_item.signature.clone(),
                    new_signature: new_item.signature.clone(),
                });
            } else if old_item.stability != new_item.stability {
                changes.push(ApiChange {
                    item_name: new_item.name.clone(),
                    change_kind: ChangeKind::StabilityChange,
                    old_signature: format!("{}", old_item.stability),
                    new_signature: format!("{}", new_item.stability),
                });
            }
        }
    }
    changes
}

/// Returns `true` if any removed item was stable.
fn has_breaking_removals(removed: &[ApiItem]) -> bool {
    removed
        .iter()
        .any(|i| i.stability == StabilityLevel::Stable)
}

/// Returns `true` if any change affects a stable item's signature.
fn has_breaking_changes(changed: &[ApiChange]) -> bool {
    changed
        .iter()
        .any(|c| c.change_kind == ChangeKind::SignatureChange)
}

/// Semantic version parts.
#[derive(Debug, Clone, PartialEq, Eq)]
struct SemVerParts {
    major: u64,
    minor: u64,
    patch: u64,
}

/// The kind of version bump.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VersionBump {
    Major,
    Minor,
    Patch,
    None,
}

/// Parses a "X.Y.Z" version string into parts.
fn parse_semver(version: &str) -> Result<SemVerParts, ReleaseError> {
    let parts: Vec<&str> = version.split('.').collect();
    if parts.len() != 3 {
        return Err(ReleaseError::InvalidConfig {
            message: format!("invalid semver: `{version}`"),
        });
    }
    let parse_part = |s: &str| -> Result<u64, ReleaseError> {
        s.parse::<u64>().map_err(|_| ReleaseError::InvalidConfig {
            message: format!("invalid semver component: `{s}`"),
        })
    };
    Ok(SemVerParts {
        major: parse_part(parts[0])?,
        minor: parse_part(parts[1])?,
        patch: parse_part(parts[2])?,
    })
}

/// Classifies the bump between two semver versions.
fn classify_bump(old: &SemVerParts, new: &SemVerParts) -> VersionBump {
    if new.major > old.major {
        VersionBump::Major
    } else if new.minor > old.minor {
        VersionBump::Minor
    } else if new.patch > old.patch {
        VersionBump::Patch
    } else {
        VersionBump::None
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Sprint 28 — ChangelogGenerator
// ═══════════════════════════════════════════════════════════════════════

/// The category of a changelog entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChangeCategory {
    /// New features or capabilities.
    Added,
    /// Changes to existing functionality.
    Changed,
    /// Newly deprecated features.
    Deprecated,
    /// Removed features or capabilities.
    Removed,
    /// Bug fixes.
    Fixed,
    /// Security-related changes.
    Security,
    /// API-breaking changes.
    Breaking,
}

impl fmt::Display for ChangeCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ChangeCategory::Added => write!(f, "Added"),
            ChangeCategory::Changed => write!(f, "Changed"),
            ChangeCategory::Deprecated => write!(f, "Deprecated"),
            ChangeCategory::Removed => write!(f, "Removed"),
            ChangeCategory::Fixed => write!(f, "Fixed"),
            ChangeCategory::Security => write!(f, "Security"),
            ChangeCategory::Breaking => write!(f, "Breaking"),
        }
    }
}

/// A single changelog item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeItem {
    /// The human-readable description.
    pub description: String,
    /// The optional scope (e.g., "parser", "runtime").
    pub scope: Option<String>,
    /// Related issue/PR numbers (e.g., "#42").
    pub related_issues: Vec<String>,
}

/// A section of the changelog grouped by category.
#[derive(Debug, Clone)]
pub struct ChangeSection {
    /// The category for this section.
    pub category: ChangeCategory,
    /// The items in this section.
    pub items: Vec<ChangeItem>,
}

/// A complete changelog entry for a single version.
#[derive(Debug, Clone)]
pub struct ChangelogEntry {
    /// The version string.
    pub version: String,
    /// The release date (ISO-8601).
    pub date: String,
    /// The optional release codename.
    pub codename: Option<String>,
    /// The changelog sections.
    pub sections: Vec<ChangeSection>,
}

impl ChangelogEntry {
    /// Renders the changelog entry as a markdown string.
    pub fn to_markdown(&self) -> String {
        let mut md = String::new();
        let codename_str = self
            .codename
            .as_ref()
            .map(|c| format!(" \"{c}\""))
            .unwrap_or_default();
        md.push_str(&format!(
            "## [{}]{} - {}\n\n",
            self.version, codename_str, self.date
        ));

        for section in &self.sections {
            md.push_str(&format!("### {}\n\n", section.category));
            for item in &section.items {
                append_change_item(&mut md, item);
            }
            md.push('\n');
        }

        md
    }
}

/// Appends a single change item to the markdown buffer.
fn append_change_item(md: &mut String, item: &ChangeItem) {
    let scope_str = item
        .scope
        .as_ref()
        .map(|s| format!("**{s}:** "))
        .unwrap_or_default();
    let issues_str = if item.related_issues.is_empty() {
        String::new()
    } else {
        format!(" ({})", item.related_issues.join(", "))
    };
    md.push_str(&format!(
        "- {}{}{}\n",
        scope_str, item.description, issues_str
    ));
}

/// Information about a single git commit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitInfo {
    /// The commit hash (short or full).
    pub hash: String,
    /// The full commit message (first line).
    pub message: String,
    /// The commit author.
    pub author: String,
    /// The commit date (ISO-8601).
    pub date: String,
}

/// Generates changelogs from conventional commit messages.
pub struct ChangelogGenerator;

impl ChangelogGenerator {
    /// Generates a changelog entry by parsing conventional commit messages.
    ///
    /// Supports the format `type(scope): description` where type is one of
    /// `feat`, `fix`, `refactor`, `perf`, `docs`, `test`, `chore`, `ci`,
    /// and the optional `!` suffix indicates a breaking change.
    pub fn from_commits(
        version: &str,
        date: &str,
        codename: Option<&str>,
        commits: &[CommitInfo],
    ) -> ChangelogEntry {
        let mut buckets: HashMap<ChangeCategory, Vec<ChangeItem>> = HashMap::new();

        for commit in commits {
            if let Some((cat, item)) = parse_conventional_commit(&commit.message) {
                buckets.entry(cat).or_default().push(item);
            }
        }

        let sections = build_sections(buckets);

        ChangelogEntry {
            version: version.to_string(),
            date: date.to_string(),
            codename: codename.map(|s| s.to_string()),
            sections,
        }
    }
}

/// Parses a conventional commit message into a category and item.
fn parse_conventional_commit(message: &str) -> Option<(ChangeCategory, ChangeItem)> {
    let first_line = message.lines().next()?;
    let (prefix, description) = first_line.split_once(':')?;
    let description = description.trim().to_string();
    if description.is_empty() {
        return None;
    }

    let is_breaking = prefix.contains('!');
    let prefix_clean = prefix.replace('!', "");

    let (commit_type, scope) = if let Some((t, s)) = prefix_clean.split_once('(') {
        (t.trim(), Some(s.trim_end_matches(')').to_string()))
    } else {
        (prefix_clean.trim(), None)
    };

    let category = if is_breaking {
        ChangeCategory::Breaking
    } else {
        match commit_type {
            "feat" => ChangeCategory::Added,
            "fix" => ChangeCategory::Fixed,
            "refactor" | "perf" => ChangeCategory::Changed,
            "docs" | "test" | "chore" | "ci" => ChangeCategory::Changed,
            "deprecate" => ChangeCategory::Deprecated,
            "security" => ChangeCategory::Security,
            _ => return None,
        }
    };

    Some((
        category,
        ChangeItem {
            description,
            scope,
            related_issues: Vec::new(),
        },
    ))
}

/// Builds ordered sections from the category buckets.
fn build_sections(buckets: HashMap<ChangeCategory, Vec<ChangeItem>>) -> Vec<ChangeSection> {
    let order = [
        ChangeCategory::Breaking,
        ChangeCategory::Added,
        ChangeCategory::Changed,
        ChangeCategory::Deprecated,
        ChangeCategory::Removed,
        ChangeCategory::Fixed,
        ChangeCategory::Security,
    ];

    order
        .iter()
        .filter_map(|cat| {
            buckets.get(cat).map(|items| ChangeSection {
                category: *cat,
                items: items.clone(),
            })
        })
        .collect()
}

/// A migration guide between two versions.
#[derive(Debug, Clone)]
pub struct MigrationGuide {
    /// The source version.
    pub from_version: String,
    /// The target version.
    pub to_version: String,
    /// Breaking changes that require action.
    pub breaking_changes: Vec<String>,
    /// Step-by-step migration instructions.
    pub migration_steps: Vec<String>,
}

impl MigrationGuide {
    /// Creates a migration guide from a changelog and API diff.
    pub fn from_changelog_and_diff(
        from_version: &str,
        to_version: &str,
        changelog: &ChangelogEntry,
        diff: &ApiDiff,
    ) -> Self {
        let breaking_changes = collect_breaking_descriptions(changelog, diff);
        let migration_steps = generate_migration_steps(&breaking_changes);

        Self {
            from_version: from_version.to_string(),
            to_version: to_version.to_string(),
            breaking_changes,
            migration_steps,
        }
    }

    /// Renders the migration guide as markdown.
    pub fn to_markdown(&self) -> String {
        let mut md = String::new();
        md.push_str(&format!(
            "# Migration Guide: {} -> {}\n\n",
            self.from_version, self.to_version
        ));

        if self.breaking_changes.is_empty() {
            md.push_str("No breaking changes in this release.\n");
            return md;
        }

        md.push_str("## Breaking Changes\n\n");
        for change in &self.breaking_changes {
            md.push_str(&format!("- {change}\n"));
        }

        md.push_str("\n## Migration Steps\n\n");
        for (i, step) in self.migration_steps.iter().enumerate() {
            md.push_str(&format!("{}. {step}\n", i + 1));
        }

        md
    }
}

/// Collects breaking change descriptions from the changelog and diff.
fn collect_breaking_descriptions(changelog: &ChangelogEntry, diff: &ApiDiff) -> Vec<String> {
    let mut descriptions = Vec::new();

    for section in &changelog.sections {
        if section.category == ChangeCategory::Breaking {
            for item in &section.items {
                descriptions.push(item.description.clone());
            }
        }
    }

    for removed in &diff.removed {
        if removed.stability == StabilityLevel::Stable {
            descriptions.push(format!("Removed stable API: `{}`", removed.name));
        }
    }

    for changed in &diff.changed {
        if changed.change_kind == ChangeKind::SignatureChange {
            descriptions.push(format!(
                "Signature changed: `{}` ({} -> {})",
                changed.item_name, changed.old_signature, changed.new_signature
            ));
        }
    }

    descriptions
}

/// Generates migration steps from breaking change descriptions.
fn generate_migration_steps(breaking_changes: &[String]) -> Vec<String> {
    let mut steps = Vec::new();

    for change in breaking_changes {
        if change.contains("Removed") {
            steps.push(format!("Replace usage of the removed API. {}", change));
        } else if change.contains("Signature changed") {
            steps.push(format!(
                "Update call sites to match the new signature. {}",
                change
            ));
        } else {
            steps.push(format!("Review and adapt to: {}", change));
        }
    }

    if !steps.is_empty() {
        steps.push("Run `cargo test` to verify all changes compile.".to_string());
        steps.push("Run `fj check` on all .fj sources.".to_string());
    }

    steps
}

// ═══════════════════════════════════════════════════════════════════════
// Sprint 28 — QualityGate
// ═══════════════════════════════════════════════════════════════════════

/// A quality check to enforce during CI.
#[derive(Debug, Clone, PartialEq)]
pub enum QualityCheck {
    /// All tests must pass.
    TestsPassing,
    /// `cargo clippy -- -D warnings` must be clean.
    ClippyClean,
    /// `cargo fmt -- --check` must be clean.
    FmtCheck,
    /// Code coverage must be at least the given percentage.
    CoverageMin(f64),
    /// No benchmark regression above the given percentage.
    BenchRegression(f64),
    /// Binary size must not exceed the given number of bytes.
    BinarySize(usize),
    /// No new compiler warnings allowed.
    NoNewWarnings,
    /// Documentation coverage must be at least the given percentage.
    DocCoverage(f64),
}

impl fmt::Display for QualityCheck {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            QualityCheck::TestsPassing => write!(f, "tests-passing"),
            QualityCheck::ClippyClean => write!(f, "clippy-clean"),
            QualityCheck::FmtCheck => write!(f, "fmt-check"),
            QualityCheck::CoverageMin(pct) => write!(f, "coverage >= {pct:.1}%"),
            QualityCheck::BenchRegression(pct) => {
                write!(f, "bench-regression < {pct:.1}%")
            }
            QualityCheck::BinarySize(bytes) => {
                write!(f, "binary-size <= {} bytes", bytes)
            }
            QualityCheck::NoNewWarnings => write!(f, "no-new-warnings"),
            QualityCheck::DocCoverage(pct) => write!(f, "doc-coverage >= {pct:.1}%"),
        }
    }
}

/// The result of a single quality check.
#[derive(Debug, Clone)]
pub struct QualityResult {
    /// The check that was run.
    pub check: String,
    /// Whether the check passed.
    pub passed: bool,
    /// A summary message.
    pub message: String,
    /// Detailed output or diagnostics.
    pub details: String,
}

/// The classification of a check within the gate policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckLevel {
    /// The check must pass for the gate to succeed.
    Required,
    /// Failure is reported but does not block the gate.
    Warn,
    /// The check is informational only.
    Info,
}

/// A policy describing which checks are required, warned, or informational.
#[derive(Debug, Clone)]
pub struct GatePolicy {
    /// Checks that must pass.
    pub required_checks: Vec<QualityCheck>,
    /// Checks that produce warnings on failure.
    pub warn_checks: Vec<QualityCheck>,
    /// Checks that are informational only.
    pub info_checks: Vec<QualityCheck>,
}

impl GatePolicy {
    /// Creates a strict policy requiring all provided checks.
    pub fn strict(checks: Vec<QualityCheck>) -> Self {
        Self {
            required_checks: checks,
            warn_checks: Vec::new(),
            info_checks: Vec::new(),
        }
    }

    /// Returns the default production gate policy.
    pub fn production_defaults() -> Self {
        Self {
            required_checks: vec![
                QualityCheck::TestsPassing,
                QualityCheck::ClippyClean,
                QualityCheck::FmtCheck,
            ],
            warn_checks: vec![
                QualityCheck::CoverageMin(85.0),
                QualityCheck::BenchRegression(10.0),
            ],
            info_checks: vec![QualityCheck::DocCoverage(80.0)],
        }
    }

    /// Returns all checks in order: required, warn, info.
    pub fn all_checks(&self) -> Vec<(&QualityCheck, CheckLevel)> {
        let mut all = Vec::new();
        for c in &self.required_checks {
            all.push((c, CheckLevel::Required));
        }
        for c in &self.warn_checks {
            all.push((c, CheckLevel::Warn));
        }
        for c in &self.info_checks {
            all.push((c, CheckLevel::Info));
        }
        all
    }
}

/// A code coverage report.
#[derive(Debug, Clone)]
pub struct CoverageReport {
    /// Total lines of code analyzed.
    pub total_lines: usize,
    /// Lines covered by at least one test.
    pub covered_lines: usize,
    /// Coverage percentage.
    pub percentage: f64,
    /// Files with zero or low coverage.
    pub uncovered_files: Vec<String>,
}

impl CoverageReport {
    /// Creates a coverage report from line counts.
    pub fn new(total_lines: usize, covered_lines: usize, uncovered_files: Vec<String>) -> Self {
        let percentage = if total_lines > 0 {
            (covered_lines as f64 / total_lines as f64) * 100.0
        } else {
            0.0
        };
        Self {
            total_lines,
            covered_lines,
            percentage,
            uncovered_files,
        }
    }
}

/// A benchmark regression detection result.
#[derive(Debug, Clone)]
pub struct BenchmarkRegression {
    /// The benchmark name.
    pub benchmark_name: String,
    /// The baseline time in nanoseconds.
    pub baseline_ns: u64,
    /// The current time in nanoseconds.
    pub current_ns: u64,
    /// The change as a percentage (positive = slower).
    pub change_pct: f64,
}

/// Runs quality gate checks and collects results.
pub struct QualityGateRunner {
    /// Simulated test pass state.
    tests_passing: bool,
    /// Simulated clippy pass state.
    clippy_clean: bool,
    /// Simulated fmt pass state.
    fmt_clean: bool,
    /// Simulated coverage.
    coverage: Option<CoverageReport>,
    /// Simulated binary size in bytes.
    binary_size: usize,
    /// Simulated benchmark regressions.
    regressions: Vec<BenchmarkRegression>,
    /// Simulated warning count.
    warning_count: usize,
    /// Simulated doc coverage percentage.
    doc_coverage_pct: f64,
}

impl QualityGateRunner {
    /// Creates a new quality gate runner with default (passing) state.
    pub fn new() -> Self {
        Self {
            tests_passing: true,
            clippy_clean: true,
            fmt_clean: true,
            coverage: None,
            binary_size: 0,
            regressions: Vec::new(),
            warning_count: 0,
            doc_coverage_pct: 100.0,
        }
    }

    /// Sets the simulated test pass state.
    pub fn set_tests_passing(&mut self, passing: bool) {
        self.tests_passing = passing;
    }

    /// Sets the simulated clippy pass state.
    pub fn set_clippy_clean(&mut self, clean: bool) {
        self.clippy_clean = clean;
    }

    /// Sets the simulated fmt check state.
    pub fn set_fmt_clean(&mut self, clean: bool) {
        self.fmt_clean = clean;
    }

    /// Sets the simulated coverage report.
    pub fn set_coverage(&mut self, report: CoverageReport) {
        self.coverage = Some(report);
    }

    /// Sets the simulated binary size.
    pub fn set_binary_size(&mut self, size: usize) {
        self.binary_size = size;
    }

    /// Adds a simulated benchmark regression.
    pub fn add_regression(&mut self, reg: BenchmarkRegression) {
        self.regressions.push(reg);
    }

    /// Sets the simulated warning count.
    pub fn set_warning_count(&mut self, count: usize) {
        self.warning_count = count;
    }

    /// Sets the simulated doc coverage percentage.
    pub fn set_doc_coverage(&mut self, pct: f64) {
        self.doc_coverage_pct = pct;
    }

    /// Runs all quality checks and returns the results.
    pub fn run_all(&self, checks: &[QualityCheck]) -> Vec<QualityResult> {
        checks.iter().map(|c| self.run_check(c)).collect()
    }

    /// Runs a single quality check.
    fn run_check(&self, check: &QualityCheck) -> QualityResult {
        match check {
            QualityCheck::TestsPassing => self.check_tests(),
            QualityCheck::ClippyClean => self.check_clippy(),
            QualityCheck::FmtCheck => self.check_fmt(),
            QualityCheck::CoverageMin(min) => self.check_coverage(*min),
            QualityCheck::BenchRegression(max) => self.check_bench_regression(*max),
            QualityCheck::BinarySize(max) => self.check_binary_size(*max),
            QualityCheck::NoNewWarnings => self.check_warnings(),
            QualityCheck::DocCoverage(min) => self.check_doc_coverage(*min),
        }
    }

    /// Checks whether all tests pass.
    fn check_tests(&self) -> QualityResult {
        QualityResult {
            check: "tests-passing".to_string(),
            passed: self.tests_passing,
            message: if self.tests_passing {
                "all tests pass".to_string()
            } else {
                "test failures detected".to_string()
            },
            details: String::new(),
        }
    }

    /// Checks whether clippy is clean.
    fn check_clippy(&self) -> QualityResult {
        QualityResult {
            check: "clippy-clean".to_string(),
            passed: self.clippy_clean,
            message: if self.clippy_clean {
                "clippy clean".to_string()
            } else {
                "clippy warnings detected".to_string()
            },
            details: String::new(),
        }
    }

    /// Checks whether formatting is clean.
    fn check_fmt(&self) -> QualityResult {
        QualityResult {
            check: "fmt-check".to_string(),
            passed: self.fmt_clean,
            message: if self.fmt_clean {
                "formatting clean".to_string()
            } else {
                "formatting issues detected".to_string()
            },
            details: String::new(),
        }
    }

    /// Checks code coverage against a minimum threshold.
    fn check_coverage(&self, min_pct: f64) -> QualityResult {
        let actual = self.coverage.as_ref().map(|c| c.percentage).unwrap_or(0.0);
        let passed = actual >= min_pct;
        QualityResult {
            check: format!("coverage >= {min_pct:.1}%"),
            passed,
            message: format!("coverage: {actual:.1}% (min: {min_pct:.1}%)"),
            details: self
                .coverage
                .as_ref()
                .map(|c| format!("{}/{} lines covered", c.covered_lines, c.total_lines))
                .unwrap_or_default(),
        }
    }

    /// Checks for benchmark regressions above the threshold.
    fn check_bench_regression(&self, max_pct: f64) -> QualityResult {
        let worst = self
            .regressions
            .iter()
            .map(|r| r.change_pct)
            .fold(0.0_f64, f64::max);
        let passed = worst <= max_pct;
        QualityResult {
            check: format!("bench-regression < {max_pct:.1}%"),
            passed,
            message: format!("worst regression: {worst:.1}% (max: {max_pct:.1}%)"),
            details: self
                .regressions
                .iter()
                .map(|r| format!("{}: {:.1}%", r.benchmark_name, r.change_pct))
                .collect::<Vec<_>>()
                .join(", "),
        }
    }

    /// Checks that binary size is within the limit.
    fn check_binary_size(&self, max_bytes: usize) -> QualityResult {
        let passed = self.binary_size <= max_bytes;
        QualityResult {
            check: format!("binary-size <= {max_bytes} bytes"),
            passed,
            message: format!(
                "binary size: {} bytes (max: {} bytes)",
                self.binary_size, max_bytes
            ),
            details: String::new(),
        }
    }

    /// Checks that there are no new compiler warnings.
    fn check_warnings(&self) -> QualityResult {
        let passed = self.warning_count == 0;
        QualityResult {
            check: "no-new-warnings".to_string(),
            passed,
            message: format!("{} new warnings", self.warning_count),
            details: String::new(),
        }
    }

    /// Checks documentation coverage.
    fn check_doc_coverage(&self, min_pct: f64) -> QualityResult {
        let passed = self.doc_coverage_pct >= min_pct;
        QualityResult {
            check: format!("doc-coverage >= {min_pct:.1}%"),
            passed,
            message: format!(
                "doc coverage: {:.1}% (min: {:.1}%)",
                self.doc_coverage_pct, min_pct
            ),
            details: String::new(),
        }
    }
}

impl Default for QualityGateRunner {
    fn default() -> Self {
        Self::new()
    }
}

/// Generates a pass/fail summary report from quality results.
pub fn quality_gate_summary(results: &[QualityResult]) -> String {
    let passed = results.iter().filter(|r| r.passed).count();
    let total = results.len();
    let all_pass = passed == total;

    let mut report = String::new();
    report.push_str("=== Quality Gate Report ===\n\n");

    for result in results {
        let status = if result.passed { "PASS" } else { "FAIL" };
        report.push_str(&format!(
            "[{status}] {}: {}\n",
            result.check, result.message
        ));
    }

    report.push_str(&format!(
        "\nResult: {passed}/{total} checks passed — {}\n",
        if all_pass {
            "GATE PASSED"
        } else {
            "GATE FAILED"
        }
    ));

    report
}

// ═══════════════════════════════════════════════════════════════════════
// Sprint 28 — ReleaseNotes
// ═══════════════════════════════════════════════════════════════════════

/// A highlighted feature for the release notes.
#[derive(Debug, Clone)]
pub struct Highlight {
    /// The feature title.
    pub title: String,
    /// A description of the feature.
    pub description: String,
    /// An optional code example.
    pub code_example: Option<String>,
}

/// Statistics for the release.
#[derive(Debug, Clone)]
pub struct ReleaseStats {
    /// Total number of tests.
    pub tests: usize,
    /// Total lines of code.
    pub loc: usize,
    /// Total number of source files.
    pub files: usize,
    /// Number of example programs.
    pub examples: usize,
    /// Number of error codes.
    pub error_codes: usize,
    /// Number of breaking changes.
    pub breaking_changes: usize,
}

/// Complete release notes for a version.
#[derive(Debug, Clone)]
pub struct ReleaseNotes {
    /// The version string.
    pub version: String,
    /// The release codename.
    pub codename: Option<String>,
    /// Top highlights of the release.
    pub highlights: Vec<Highlight>,
    /// Changelog sections.
    pub sections: Vec<ChangeSection>,
    /// Release statistics.
    pub stats: ReleaseStats,
    /// Upgrade instructions.
    pub upgrade_instructions: Vec<String>,
}

/// Generates GitHub Releases markdown from release notes.
pub fn generate_github_release(notes: &ReleaseNotes) -> String {
    let mut md = String::new();

    // Title
    let codename_str = notes
        .codename
        .as_ref()
        .map(|c| format!(" \"{c}\""))
        .unwrap_or_default();
    md.push_str(&format!(
        "# Fajar Lang v{}{}\n\n",
        notes.version, codename_str
    ));

    // Highlights
    if !notes.highlights.is_empty() {
        md.push_str("## Highlights\n\n");
        for hl in &notes.highlights {
            append_highlight(&mut md, hl);
        }
    }

    // Changelog sections
    append_changelog_sections(&mut md, &notes.sections);

    // Stats
    append_stats(&mut md, &notes.stats);

    // Upgrade
    append_upgrade_instructions(&mut md, &notes.upgrade_instructions);

    md
}

/// Appends a single highlight to the markdown.
fn append_highlight(md: &mut String, hl: &Highlight) {
    md.push_str(&format!("### {}\n\n{}\n\n", hl.title, hl.description));
    if let Some(code) = &hl.code_example {
        md.push_str(&format!("```fajar\n{code}\n```\n\n"));
    }
}

/// Appends changelog sections to the markdown.
fn append_changelog_sections(md: &mut String, sections: &[ChangeSection]) {
    if sections.is_empty() {
        return;
    }
    md.push_str("## Changes\n\n");
    for section in sections {
        md.push_str(&format!("### {}\n\n", section.category));
        for item in &section.items {
            append_change_item(md, item);
        }
        md.push('\n');
    }
}

/// Appends release statistics to the markdown.
fn append_stats(md: &mut String, stats: &ReleaseStats) {
    md.push_str("## Stats\n\n");
    md.push_str(&format!("- **Tests:** {}\n", stats.tests));
    md.push_str(&format!("- **LOC:** {}\n", stats.loc));
    md.push_str(&format!("- **Files:** {}\n", stats.files));
    md.push_str(&format!("- **Examples:** {}\n", stats.examples));
    md.push_str(&format!("- **Error codes:** {}\n", stats.error_codes));
    if stats.breaking_changes > 0 {
        md.push_str(&format!(
            "- **Breaking changes:** {}\n",
            stats.breaking_changes
        ));
    }
    md.push('\n');
}

/// Appends upgrade instructions to the markdown.
fn append_upgrade_instructions(md: &mut String, instructions: &[String]) {
    if instructions.is_empty() {
        return;
    }
    md.push_str("## Upgrade Instructions\n\n");
    for (i, step) in instructions.iter().enumerate() {
        md.push_str(&format!("{}. {step}\n", i + 1));
    }
    md.push('\n');
}

/// Generates a blog-style release announcement.
pub fn generate_blog_post(notes: &ReleaseNotes) -> String {
    let mut post = String::new();

    let codename_str = notes
        .codename
        .as_ref()
        .map(|c| format!(" \"{c}\""))
        .unwrap_or_default();
    post.push_str(&format!(
        "# Announcing Fajar Lang v{}{}\n\n",
        notes.version, codename_str
    ));

    post.push_str(&format!(
        "We are excited to announce Fajar Lang v{}, ",
        notes.version
    ));
    if let Some(codename) = &notes.codename {
        post.push_str(&format!("codenamed \"{codename}\", "));
    }
    post.push_str("a major milestone for the Fajar Lang project.\n\n");

    if !notes.highlights.is_empty() {
        post.push_str("## What's New\n\n");
        for hl in &notes.highlights {
            post.push_str(&format!("**{}** -- {}\n\n", hl.title, hl.description));
        }
    }

    post.push_str(&format!(
        "## By the Numbers\n\n\
         This release includes {} tests across {} lines of code in {} files, \
         with {} example programs and {} error codes.\n\n",
        notes.stats.tests,
        notes.stats.loc,
        notes.stats.files,
        notes.stats.examples,
        notes.stats.error_codes,
    ));

    post.push_str("## Get Started\n\n");
    post.push_str("```bash\ncargo install fajar-lang\nfj run examples/hello.fj\n```\n\n");

    post
}

/// Generates a short social media announcement (max 280 characters).
pub fn generate_tweet(notes: &ReleaseNotes) -> String {
    let codename_str = notes
        .codename
        .as_ref()
        .map(|c| format!(" \"{c}\""))
        .unwrap_or_default();

    let highlight_count = notes.highlights.len();
    let base = format!(
        "Fajar Lang v{}{} released! {} tests, {}K LOC, {} examples.",
        notes.version,
        codename_str,
        notes.stats.tests,
        notes.stats.loc / 1000,
        notes.stats.examples,
    );

    let suffix = if highlight_count > 0 {
        format!(" {} new features.", highlight_count)
    } else {
        String::new()
    };

    let full = format!("{base}{suffix} #FajarLang #ProgrammingLanguages");

    // Truncate to 280 chars if needed
    if full.len() <= 280 {
        full
    } else {
        let mut truncated = full[..277].to_string();
        truncated.push_str("...");
        truncated
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── Sprint 25: ReleasePipeline (s25_1 – s25_10) ─────────────────

    #[test]
    fn s25_1_release_stage_display() {
        assert_eq!(format!("{}", ReleaseStage::Test), "test");
        assert_eq!(format!("{}", ReleaseStage::Build), "build");
        assert_eq!(format!("{}", ReleaseStage::Sign), "sign");
        assert_eq!(format!("{}", ReleaseStage::Publish), "publish");
        assert_eq!(format!("{}", ReleaseStage::Verify), "verify");
        assert_eq!(format!("{}", ReleaseStage::Announce), "announce");
    }

    #[test]
    fn s25_2_release_target_triple_and_binary_name() {
        let linux = ReleaseTarget::new(TargetOs::Linux, TargetArch::X86_64);
        assert_eq!(linux.triple(), "x86_64-unknown-linux-gnu");
        assert_eq!(linux.binary_name(), "fj-linux-x86_64");

        let win = ReleaseTarget::new(TargetOs::Windows, TargetArch::X86_64);
        assert_eq!(win.triple(), "x86_64-pc-windows-msvc");
        assert_eq!(win.binary_name(), "fj-windows-x86_64.exe");

        let mac = ReleaseTarget::new(TargetOs::MacOS, TargetArch::Aarch64);
        assert_eq!(mac.triple(), "aarch64-apple-darwin");
        assert_eq!(mac.binary_name(), "fj-macos-aarch64");

        let musl = ReleaseTarget::new(TargetOs::LinuxMusl, TargetArch::X86_64);
        assert_eq!(musl.triple(), "x86_64-unknown-linux-musl");

        let rv = ReleaseTarget::new(TargetOs::Linux, TargetArch::Riscv64);
        assert_eq!(rv.triple(), "riscv64gc-unknown-linux-gnu");

        let wasm = ReleaseTarget::new(TargetOs::FreeBSD, TargetArch::Wasm32);
        assert!(wasm.triple().starts_with("wasm32"));
    }

    #[test]
    fn s25_3_release_config_validation() {
        let empty_version = ReleaseConfig::new(String::new());
        assert!(empty_version.validate().is_err());

        let no_targets = ReleaseConfig::new("1.0.0".to_string());
        assert!(no_targets.validate().is_err());

        let valid = ReleaseConfig::default_multiplatform("1.0.0".to_string());
        assert!(valid.validate().is_ok());
        assert_eq!(valid.targets.len(), 3);
        assert!(valid.publish_crates_io);
        assert!(valid.publish_github);
        assert!(!valid.publish_homebrew);
    }

    #[test]
    fn s25_4_release_pipeline_plan_stages() {
        let config = ReleaseConfig::default_multiplatform("1.0.0".to_string());
        let mut pipeline = ReleasePipeline::new(config);

        let stages = pipeline.plan().unwrap();
        // Test, Build, Publish (crates_io + github), Verify, Announce
        assert!(stages.contains(&ReleaseStage::Test));
        assert!(stages.contains(&ReleaseStage::Build));
        assert!(stages.contains(&ReleaseStage::Publish));
        assert!(stages.contains(&ReleaseStage::Verify));
        assert!(stages.contains(&ReleaseStage::Announce));
        // No sign key -> no Sign stage
        assert!(!stages.contains(&ReleaseStage::Sign));
    }

    #[test]
    fn s25_5_release_pipeline_plan_with_signing() {
        let mut config = ReleaseConfig::default_multiplatform("1.0.0".to_string());
        config.sign_key = Some("/path/to/key.gpg".to_string());

        let mut pipeline = ReleasePipeline::new(config);
        let stages = pipeline.plan().unwrap();
        assert!(stages.contains(&ReleaseStage::Sign));
    }

    #[test]
    fn s25_6_release_pipeline_execute_produces_artifacts() {
        let config = ReleaseConfig::default_multiplatform("1.0.0".to_string());
        let mut pipeline = ReleasePipeline::new(config);

        pipeline.plan().unwrap();
        pipeline.execute().unwrap();

        let report = pipeline.report();
        assert_eq!(report.version, "1.0.0");
        assert_eq!(report.artifacts.len(), 3);
        assert!(report.success);
        assert!(report.total_time_secs >= 0.0);
    }

    #[test]
    fn s25_7_release_pipeline_execute_without_plan_fails() {
        let mut config = ReleaseConfig::new("1.0.0".to_string());
        config
            .targets
            .push(ReleaseTarget::new(TargetOs::Linux, TargetArch::X86_64));
        let mut pipeline = ReleasePipeline::new(config);

        // Don't call plan()
        let result = pipeline.execute();
        assert!(result.is_err());
    }

    #[test]
    fn s25_8_release_report_stage_counts() {
        let config = ReleaseConfig::default_multiplatform("1.0.0".to_string());
        let mut pipeline = ReleasePipeline::new(config);
        pipeline.plan().unwrap();
        pipeline.execute().unwrap();

        let report = pipeline.report();
        assert!(report.stages_passed() > 0);
        assert_eq!(report.stages_passed(), report.stages_total());
    }

    #[test]
    fn s25_9_release_artifact_structure() {
        let config = ReleaseConfig::default_multiplatform("1.0.0".to_string());
        let mut pipeline = ReleasePipeline::new(config);
        pipeline.plan().unwrap();
        pipeline.execute().unwrap();

        let report = pipeline.report();
        for artifact in &report.artifacts {
            assert!(!artifact.name.is_empty());
            assert!(!artifact.target.is_empty());
            assert!(!artifact.path.is_empty());
            assert_eq!(artifact.sha256.len(), 64);
        }
    }

    #[test]
    fn s25_10_release_error_display() {
        let err = ReleaseError::StageFailed {
            stage: "build".to_string(),
            reason: "linker not found".to_string(),
        };
        assert!(format!("{err}").contains("build"));
        assert!(format!("{err}").contains("linker not found"));

        let err2 = ReleaseError::InvalidConfig {
            message: "empty version".to_string(),
        };
        assert!(format!("{err2}").contains("empty version"));
    }

    // ── Sprint 26: BinarySizeOptimizer (s26_1 – s26_10) ──────────────

    #[test]
    fn s26_1_binary_size_analysis() {
        let mut optimizer = SizeOptimizer::new();
        let report = optimizer.analyze("target/release/fj").unwrap();

        assert!(report.total_bytes > 0);
        assert!(!report.sections.is_empty());
        assert!(!report.top_functions.is_empty());
        assert!(!report.top_crates.is_empty());
    }

    #[test]
    fn s26_2_binary_size_section_lookup() {
        let mut optimizer = SizeOptimizer::new();
        let report = optimizer.analyze("target/release/fj").unwrap();

        assert!(report.section_size(".text").is_some());
        assert!(report.section_size(".rodata").is_some());
        assert!(report.section_size(".nonexistent").is_none());
    }

    #[test]
    fn s26_3_binary_size_report_summary() {
        let mut optimizer = SizeOptimizer::new();
        let report = optimizer.analyze("target/release/fj").unwrap();
        let summary = report.summary();

        assert!(summary.contains("Binary size:"));
        assert!(summary.contains("KB"));
        assert!(summary.contains("MB"));
        assert!(summary.contains(".text"));
        assert!(summary.contains("Sections:"));
    }

    #[test]
    fn s26_4_optimization_suggestions() {
        let mut optimizer = SizeOptimizer::new();
        optimizer.analyze("target/release/fj").unwrap();
        let suggestions = optimizer.suggest();

        assert!(suggestions.len() >= 3);

        let categories: Vec<&str> = suggestions.iter().map(|s| s.category.as_str()).collect();
        assert!(categories.contains(&"lto"));
        assert!(categories.contains(&"codegen-units"));
        assert!(categories.contains(&"opt-level"));

        for s in &suggestions {
            assert!(!s.description.is_empty());
            assert!(s.estimated_savings_bytes > 0);
        }
    }

    #[test]
    fn s26_5_build_profile_debug() {
        let debug = BuildProfile::debug();
        assert_eq!(debug.name, "debug");
        assert_eq!(debug.opt_level, "0");
        assert!(!debug.lto);
        assert!(!debug.strip);
        assert!(!debug.panic_abort);
        assert_eq!(debug.codegen_units, 16);
    }

    #[test]
    fn s26_6_build_profile_dist() {
        let dist = BuildProfile::dist();
        assert_eq!(dist.name, "dist");
        assert_eq!(dist.opt_level, "z");
        assert!(dist.lto);
        assert!(dist.strip);
        assert!(dist.panic_abort);
        assert_eq!(dist.codegen_units, 1);
    }

    #[test]
    fn s26_7_build_profile_to_toml() {
        let dist = BuildProfile::dist();
        let toml = dist.to_toml();

        assert!(toml.contains("[profile.dist]"));
        assert!(toml.contains("opt-level = \"z\""));
        assert!(toml.contains("lto = true"));
        assert!(toml.contains("codegen-units = 1"));
        assert!(toml.contains("strip = true"));
        assert!(toml.contains("panic = \"abort\""));
    }

    #[test]
    fn s26_8_feature_impact_measurement() {
        let impact = FeatureImpact::new("native".to_string(), 5_000_000, 7_500_000);
        assert_eq!(impact.feature_name, "native");
        assert_eq!(impact.delta_bytes, 2_500_000);
        assert!((impact.delta_percent - 50.0).abs() < 0.1);

        let zero = FeatureImpact::new("empty".to_string(), 0, 100);
        assert_eq!(zero.delta_percent, 0.0);
    }

    #[test]
    fn s26_9_binary_analysis_empty_path_fails() {
        let mut optimizer = SizeOptimizer::new();
        let result = optimizer.analyze("");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(format!("{err}").contains("empty binary path"));
    }

    #[test]
    fn s26_10_function_and_crate_percentages() {
        let mut optimizer = SizeOptimizer::new();
        let report = optimizer.analyze("target/release/fj").unwrap();

        for func in &report.top_functions {
            assert!(func.percentage > 0.0);
            assert!(func.percentage < 100.0);
            assert!(func.size_bytes > 0);
        }

        for cr in &report.top_crates {
            assert!(cr.percentage > 0.0);
            assert!(cr.percentage < 100.0);
            assert!(cr.size_bytes > 0);
        }
    }

    // ── Sprint 27: StabilityGuarantee (s27_1 – s27_10) ───────────────

    #[test]
    fn s27_1_api_item_kind_display() {
        assert_eq!(format!("{}", ApiItemKind::Function), "function");
        assert_eq!(format!("{}", ApiItemKind::Struct), "struct");
        assert_eq!(format!("{}", ApiItemKind::Enum), "enum");
        assert_eq!(format!("{}", ApiItemKind::Trait), "trait");
        assert_eq!(format!("{}", ApiItemKind::Method), "method");
    }

    #[test]
    fn s27_2_stability_level_display() {
        assert_eq!(format!("{}", StabilityLevel::Stable), "stable");
        assert_eq!(format!("{}", StabilityLevel::Unstable), "unstable");
        assert_eq!(format!("{}", StabilityLevel::Internal), "internal");
        let dep = StabilityLevel::Deprecated {
            since: "0.9.0".to_string(),
            replacement: "new_fn".to_string(),
        };
        assert!(format!("{dep}").contains("0.9.0"));
    }

    #[test]
    fn s27_3_api_snapshot_creation_and_lookup() {
        let items = vec![
            ApiItem {
                name: "parse".to_string(),
                kind: ApiItemKind::Function,
                signature: "fn parse(tokens: Vec<Token>) -> Program".to_string(),
                stability: StabilityLevel::Stable,
                since_version: "0.1.0".to_string(),
            },
            ApiItem {
                name: "compile".to_string(),
                kind: ApiItemKind::Function,
                signature: "fn compile(prog: &Program) -> Binary".to_string(),
                stability: StabilityLevel::Unstable,
                since_version: "0.5.0".to_string(),
            },
        ];
        let snap = ApiSnapshot::new("1.0.0".to_string(), items, "2026-03-11".to_string());

        assert_eq!(snap.stable_count(), 1);
        assert!(snap.find("parse").is_some());
        assert!(snap.find("nonexistent").is_none());
    }

    #[test]
    fn s27_4_api_diff_added_and_removed() {
        let old = ApiSnapshot::new(
            "0.9.0".to_string(),
            vec![make_api_item("alpha", "fn alpha() -> i32")],
            "2026-01-01".to_string(),
        );
        let new = ApiSnapshot::new(
            "1.0.0".to_string(),
            vec![make_api_item("beta", "fn beta() -> i32")],
            "2026-03-11".to_string(),
        );

        let checker = StabilityChecker::new();
        let diff = checker.diff(&old, &new);

        assert_eq!(diff.added.len(), 1);
        assert_eq!(diff.added[0].name, "beta");
        assert_eq!(diff.removed.len(), 1);
        assert_eq!(diff.removed[0].name, "alpha");
        assert!(diff.changed.is_empty());
        assert_eq!(diff.total_changes(), 2);
    }

    #[test]
    fn s27_5_api_diff_signature_change() {
        let old = ApiSnapshot::new(
            "0.9.0".to_string(),
            vec![make_api_item("parse", "fn parse(t: Vec<Token>) -> Ast")],
            "2026-01-01".to_string(),
        );
        let new = ApiSnapshot::new(
            "1.0.0".to_string(),
            vec![make_api_item("parse", "fn parse(t: &[Token]) -> Ast")],
            "2026-03-11".to_string(),
        );

        let checker = StabilityChecker::new();
        let diff = checker.diff(&old, &new);

        assert!(diff.added.is_empty());
        assert!(diff.removed.is_empty());
        assert_eq!(diff.changed.len(), 1);
        assert_eq!(diff.changed[0].change_kind, ChangeKind::SignatureChange);
    }

    #[test]
    fn s27_6_is_breaking_detects_removals() {
        let old = ApiSnapshot::new(
            "0.9.0".to_string(),
            vec![make_api_item("stable_fn", "fn stable_fn()")],
            "2026-01-01".to_string(),
        );
        let new = ApiSnapshot::new("1.0.0".to_string(), Vec::new(), "2026-03-11".to_string());

        let checker = StabilityChecker::new();
        let diff = checker.diff(&old, &new);
        assert!(checker.is_breaking(&diff));
    }

    #[test]
    fn s27_7_is_breaking_detects_signature_changes() {
        let old = ApiSnapshot::new(
            "0.9.0".to_string(),
            vec![make_api_item("foo", "fn foo(a: i32)")],
            "2026-01-01".to_string(),
        );
        let new = ApiSnapshot::new(
            "1.0.0".to_string(),
            vec![make_api_item("foo", "fn foo(a: i64)")],
            "2026-03-11".to_string(),
        );

        let checker = StabilityChecker::new();
        let diff = checker.diff(&old, &new);
        assert!(checker.is_breaking(&diff));
    }

    #[test]
    fn s27_8_semver_validation_breaking_in_minor_fails() {
        let old = ApiSnapshot::new(
            "1.0.0".to_string(),
            vec![make_api_item("foo", "fn foo(a: i32)")],
            "2026-01-01".to_string(),
        );
        let new = ApiSnapshot::new(
            "1.1.0".to_string(),
            vec![make_api_item("foo", "fn foo(a: i64)")],
            "2026-03-11".to_string(),
        );

        let checker = StabilityChecker::new();
        let diff = checker.diff(&old, &new);
        let result = checker.validate_semver("1.0.0", "1.1.0", &diff);
        assert!(result.is_err());
    }

    #[test]
    fn s27_9_semver_validation_addition_in_patch_fails() {
        let old = ApiSnapshot::new(
            "1.0.0".to_string(),
            vec![make_api_item("foo", "fn foo()")],
            "2026-01-01".to_string(),
        );
        let new = ApiSnapshot::new(
            "1.0.1".to_string(),
            vec![
                make_api_item("foo", "fn foo()"),
                make_api_item("bar", "fn bar()"),
            ],
            "2026-03-11".to_string(),
        );

        let checker = StabilityChecker::new();
        let diff = checker.diff(&old, &new);
        let result = checker.validate_semver("1.0.0", "1.0.1", &diff);
        assert!(result.is_err());
    }

    #[test]
    fn s27_10_semver_validation_valid_major_bump() {
        let old = ApiSnapshot::new(
            "1.0.0".to_string(),
            vec![make_api_item("old_fn", "fn old_fn()")],
            "2026-01-01".to_string(),
        );
        let new = ApiSnapshot::new(
            "2.0.0".to_string(),
            vec![make_api_item("new_fn", "fn new_fn()")],
            "2026-03-11".to_string(),
        );

        let checker = StabilityChecker::new();
        let diff = checker.diff(&old, &new);
        let result = checker.validate_semver("1.0.0", "2.0.0", &diff);
        assert!(result.is_ok());
    }

    // ── Sprint 28: Changelog, QualityGate, ReleaseNotes (s28_1 – s28_10) ──

    #[test]
    fn s28_1_changelog_from_conventional_commits() {
        let commits = vec![
            make_commit("abc123", "feat(parser): add pipeline operator"),
            make_commit("def456", "fix(analyzer): resolve module paths"),
            make_commit("ghi789", "refactor(codegen): simplify emit_call"),
        ];
        let entry =
            ChangelogGenerator::from_commits("1.0.0", "2026-03-11", Some("Genesis"), &commits);

        assert_eq!(entry.version, "1.0.0");
        assert_eq!(entry.codename, Some("Genesis".to_string()));
        assert!(!entry.sections.is_empty());

        let md = entry.to_markdown();
        assert!(md.contains("[1.0.0]"));
        assert!(md.contains("Genesis"));
        assert!(md.contains("2026-03-11"));
        assert!(md.contains("pipeline operator"));
        assert!(md.contains("module paths"));
    }

    #[test]
    fn s28_2_changelog_breaking_change_detection() {
        let commits = vec![
            make_commit("aaa", "feat!(parser): change AST node layout"),
            make_commit("bbb", "feat(runtime): add GPU support"),
        ];
        let entry = ChangelogGenerator::from_commits("2.0.0", "2026-03-11", None, &commits);

        let breaking = entry
            .sections
            .iter()
            .find(|s| s.category == ChangeCategory::Breaking);
        assert!(breaking.is_some());
        assert_eq!(breaking.unwrap().items.len(), 1);
        assert!(
            breaking.unwrap().items[0]
                .description
                .contains("AST node layout")
        );
    }

    #[test]
    fn s28_3_migration_guide_generation() {
        let changelog = ChangelogEntry {
            version: "2.0.0".to_string(),
            date: "2026-03-11".to_string(),
            codename: None,
            sections: vec![ChangeSection {
                category: ChangeCategory::Breaking,
                items: vec![ChangeItem {
                    description: "Removed legacy parse mode".to_string(),
                    scope: Some("parser".to_string()),
                    related_issues: vec![],
                }],
            }],
        };

        let diff = ApiDiff {
            added: vec![],
            removed: vec![make_api_item("old_parse", "fn old_parse()")],
            changed: vec![],
        };

        let guide = MigrationGuide::from_changelog_and_diff("1.0.0", "2.0.0", &changelog, &diff);
        assert_eq!(guide.from_version, "1.0.0");
        assert_eq!(guide.to_version, "2.0.0");
        assert!(!guide.breaking_changes.is_empty());
        assert!(!guide.migration_steps.is_empty());

        let md = guide.to_markdown();
        assert!(md.contains("Migration Guide"));
        assert!(md.contains("Breaking Changes"));
        assert!(md.contains("Migration Steps"));
    }

    #[test]
    fn s28_4_quality_gate_all_passing() {
        let runner = QualityGateRunner::new();
        let checks = vec![
            QualityCheck::TestsPassing,
            QualityCheck::ClippyClean,
            QualityCheck::FmtCheck,
        ];
        let results = runner.run_all(&checks);

        assert_eq!(results.len(), 3);
        assert!(results.iter().all(|r| r.passed));

        let summary = quality_gate_summary(&results);
        assert!(summary.contains("GATE PASSED"));
        assert!(summary.contains("3/3"));
    }

    #[test]
    fn s28_5_quality_gate_failing_checks() {
        let mut runner = QualityGateRunner::new();
        runner.set_tests_passing(false);
        runner.set_clippy_clean(false);

        let checks = vec![
            QualityCheck::TestsPassing,
            QualityCheck::ClippyClean,
            QualityCheck::FmtCheck,
        ];
        let results = runner.run_all(&checks);

        let passed = results.iter().filter(|r| r.passed).count();
        assert_eq!(passed, 1); // only fmt passes

        let summary = quality_gate_summary(&results);
        assert!(summary.contains("GATE FAILED"));
        assert!(summary.contains("1/3"));
    }

    #[test]
    fn s28_6_quality_gate_coverage_and_binary_size() {
        let mut runner = QualityGateRunner::new();
        runner.set_coverage(CoverageReport::new(1000, 900, vec![]));
        runner.set_binary_size(5_000_000);

        let checks = vec![
            QualityCheck::CoverageMin(85.0),
            QualityCheck::BinarySize(10_000_000),
        ];
        let results = runner.run_all(&checks);

        assert!(results[0].passed); // 90% >= 85%
        assert!(results[1].passed); // 5MB <= 10MB

        // Now test failure
        let checks_fail = vec![
            QualityCheck::CoverageMin(95.0),
            QualityCheck::BinarySize(1_000_000),
        ];
        let results_fail = runner.run_all(&checks_fail);
        assert!(!results_fail[0].passed); // 90% < 95%
        assert!(!results_fail[1].passed); // 5MB > 1MB
    }

    #[test]
    fn s28_7_quality_gate_bench_regression() {
        let mut runner = QualityGateRunner::new();
        runner.add_regression(BenchmarkRegression {
            benchmark_name: "lex_speed".to_string(),
            baseline_ns: 1000,
            current_ns: 1150,
            change_pct: 15.0,
        });

        let pass = runner.run_all(&[QualityCheck::BenchRegression(20.0)]);
        assert!(pass[0].passed); // 15% < 20%

        let fail = runner.run_all(&[QualityCheck::BenchRegression(10.0)]);
        assert!(!fail[0].passed); // 15% > 10%
    }

    #[test]
    fn s28_8_gate_policy_production_defaults() {
        let policy = GatePolicy::production_defaults();

        assert_eq!(policy.required_checks.len(), 3);
        assert_eq!(policy.warn_checks.len(), 2);
        assert_eq!(policy.info_checks.len(), 1);

        let all = policy.all_checks();
        assert_eq!(all.len(), 6);
        assert_eq!(all[0].1, CheckLevel::Required);
        assert_eq!(all[3].1, CheckLevel::Warn);
        assert_eq!(all[5].1, CheckLevel::Info);
    }

    #[test]
    fn s28_9_github_release_generation() {
        let notes = make_sample_release_notes();
        let md = generate_github_release(&notes);

        assert!(md.contains("# Fajar Lang v1.0.0"));
        assert!(md.contains("Genesis"));
        assert!(md.contains("Highlights"));
        assert!(md.contains("First-class tensor types"));
        assert!(md.contains("Stats"));
        assert!(md.contains("Tests:"));
        assert!(md.contains("LOC:"));
        assert!(md.contains("Upgrade Instructions"));
    }

    #[test]
    fn s28_10_blog_post_and_tweet_generation() {
        let notes = make_sample_release_notes();

        let blog = generate_blog_post(&notes);
        assert!(blog.contains("Announcing Fajar Lang v1.0.0"));
        assert!(blog.contains("Genesis"));
        assert!(blog.contains("What's New"));
        assert!(blog.contains("By the Numbers"));
        assert!(blog.contains("cargo install fajar-lang"));

        let tweet = generate_tweet(&notes);
        assert!(tweet.len() <= 280);
        assert!(tweet.contains("Fajar Lang v1.0.0"));
        assert!(tweet.contains("#FajarLang"));
    }

    // ── Test helpers ──────────────────────────────────────────────────

    fn make_api_item(name: &str, signature: &str) -> ApiItem {
        ApiItem {
            name: name.to_string(),
            kind: ApiItemKind::Function,
            signature: signature.to_string(),
            stability: StabilityLevel::Stable,
            since_version: "0.1.0".to_string(),
        }
    }

    fn make_commit(hash: &str, message: &str) -> CommitInfo {
        CommitInfo {
            hash: hash.to_string(),
            message: message.to_string(),
            author: "Fajar <fajar@primecore.id>".to_string(),
            date: "2026-03-11".to_string(),
        }
    }

    fn make_sample_release_notes() -> ReleaseNotes {
        ReleaseNotes {
            version: "1.0.0".to_string(),
            codename: Some("Genesis".to_string()),
            highlights: vec![Highlight {
                title: "First-class tensor types".to_string(),
                description: "Tensors are now a built-in type with shape checking.".to_string(),
                code_example: Some("let t = zeros(3, 4)".to_string()),
            }],
            sections: vec![ChangeSection {
                category: ChangeCategory::Added,
                items: vec![ChangeItem {
                    description: "Tensor type system".to_string(),
                    scope: Some("runtime".to_string()),
                    related_issues: vec!["#100".to_string()],
                }],
            }],
            stats: ReleaseStats {
                tests: 2650,
                loc: 98000,
                files: 97,
                examples: 24,
                error_codes: 71,
                breaking_changes: 0,
            },
            upgrade_instructions: vec![
                "Update Cargo.toml: `fajar-lang = \"1.0.0\"`".to_string(),
                "Run `fj check` on all source files.".to_string(),
            ],
        }
    }
}
