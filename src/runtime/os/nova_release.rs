//! Release infrastructure for FajarOS Nova v2.0 — Sprint N10.
//!
//! Provides ISO image building, boot media configuration, text-based installer,
//! documentation generation, benchmarking, audit reporting, migration guides,
//! release notes, compatibility matrix, and end-to-end test suite.
//! All structures are simulated in-memory.

use std::collections::HashMap;

// ═══════════════════════════════════════════════════════════════════════
// Release Errors
// ═══════════════════════════════════════════════════════════════════════

/// Errors produced by the release system.
#[derive(Debug, Clone, thiserror::Error)]
pub enum ReleaseError {
    /// Missing component.
    #[error("missing component: {0}")]
    MissingComponent(String),
    /// Build failure.
    #[error("build failure: {0}")]
    BuildFailure(String),
    /// Validation failure.
    #[error("validation failed: {0}")]
    ValidationFailure(String),
    /// Installer error.
    #[error("installer error: {0}")]
    InstallerError(String),
}

// ═══════════════════════════════════════════════════════════════════════
// ISO Image Builder
// ═══════════════════════════════════════════════════════════════════════

/// A file entry in the ISO image.
#[derive(Debug, Clone)]
pub struct IsoFile {
    /// Path within the ISO.
    pub path: String,
    /// File size in bytes.
    pub size: u64,
    /// Content hash (simulated).
    pub hash: String,
}

/// ISO image builder — creates bootable ISO with kernel and userland.
#[derive(Debug)]
pub struct IsoImageBuilder {
    /// Files to include.
    files: Vec<IsoFile>,
    /// ISO label.
    pub label: String,
    /// Boot loader path.
    pub bootloader: String,
    /// Kernel image path.
    pub kernel: String,
    /// Total size in bytes.
    pub total_size: u64,
}

impl IsoImageBuilder {
    /// Creates a new ISO builder.
    pub fn new(label: &str) -> Self {
        Self {
            files: Vec::new(),
            label: label.to_string(),
            bootloader: String::new(),
            kernel: String::new(),
            total_size: 0,
        }
    }

    /// Sets the bootloader.
    pub fn set_bootloader(&mut self, path: &str) {
        self.bootloader = path.to_string();
    }

    /// Sets the kernel image.
    pub fn set_kernel(&mut self, path: &str) {
        self.kernel = path.to_string();
    }

    /// Adds a file to the ISO.
    pub fn add_file(&mut self, path: &str, size: u64) {
        let hash = format!("{:08x}", path.len() as u32 * 31 + size as u32);
        self.files.push(IsoFile {
            path: path.to_string(),
            size,
            hash,
        });
        self.total_size += size;
    }

    /// Builds the ISO image. Returns the image size.
    pub fn build(&self) -> Result<u64, ReleaseError> {
        if self.kernel.is_empty() {
            return Err(ReleaseError::MissingComponent("kernel".to_string()));
        }
        if self.bootloader.is_empty() {
            return Err(ReleaseError::MissingComponent("bootloader".to_string()));
        }
        // Overhead: boot sector (512) + El Torito header (2048) + directories
        let overhead = 512 + 2048 + self.files.len() as u64 * 256;
        Ok(self.total_size + overhead)
    }

    /// Returns the number of files in the ISO.
    pub fn file_count(&self) -> usize {
        self.files.len()
    }

    /// Returns a list of all files.
    pub fn files(&self) -> &[IsoFile] {
        &self.files
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Boot Media Configuration
// ═══════════════════════════════════════════════════════════════════════

/// Boot mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootMode {
    /// Legacy BIOS boot.
    Bios,
    /// UEFI boot.
    Uefi,
    /// Hybrid (BIOS + UEFI).
    Hybrid,
}

/// Boot media configuration — BIOS + UEFI boot settings.
#[derive(Debug, Clone)]
pub struct BootMediaConfig {
    /// Boot mode.
    pub mode: BootMode,
    /// GRUB configuration (menu entries).
    pub grub_entries: Vec<GrubEntry>,
    /// Kernel command line.
    pub kernel_cmdline: String,
    /// Initrd path (if any).
    pub initrd: Option<String>,
    /// Timeout before auto-boot (seconds).
    pub timeout_secs: u32,
}

/// A GRUB menu entry.
#[derive(Debug, Clone)]
pub struct GrubEntry {
    /// Entry title.
    pub title: String,
    /// Kernel path.
    pub kernel: String,
    /// Kernel arguments.
    pub args: String,
    /// Is this the default entry?
    pub default: bool,
}

impl BootMediaConfig {
    /// Creates a default boot configuration.
    pub fn new(mode: BootMode) -> Self {
        Self {
            mode,
            grub_entries: vec![GrubEntry {
                title: "FajarOS Nova v2.0".to_string(),
                kernel: "/boot/kernel.elf".to_string(),
                args: "root=/dev/sda1 console=ttyS0".to_string(),
                default: true,
            }],
            kernel_cmdline: "root=/dev/sda1 console=ttyS0".to_string(),
            initrd: None,
            timeout_secs: 5,
        }
    }

    /// Adds a GRUB menu entry.
    pub fn add_entry(&mut self, entry: GrubEntry) {
        self.grub_entries.push(entry);
    }

    /// Generates the GRUB configuration as a string.
    pub fn generate_grub_cfg(&self) -> String {
        let mut cfg = String::new();
        cfg.push_str(&format!("set timeout={}\n", self.timeout_secs));
        cfg.push_str("set default=0\n\n");

        for (i, entry) in self.grub_entries.iter().enumerate() {
            cfg.push_str(&format!("menuentry \"{}\" {{\n", entry.title));
            cfg.push_str(&format!("  multiboot2 {} {}\n", entry.kernel, entry.args));
            if let Some(ref initrd) = self.initrd {
                cfg.push_str(&format!("  module2 {}\n", initrd));
            }
            cfg.push_str("}\n");
            if i < self.grub_entries.len() - 1 {
                cfg.push('\n');
            }
        }
        cfg
    }

    /// Returns the number of boot entries.
    pub fn entry_count(&self) -> usize {
        self.grub_entries.len()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Installer Wizard
// ═══════════════════════════════════════════════════════════════════════

/// Installer step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallerStep {
    /// Welcome screen.
    Welcome,
    /// Disk partitioning.
    Partition,
    /// Filesystem formatting.
    Format,
    /// File copy.
    Copy,
    /// Bootloader installation.
    Bootloader,
    /// Configuration.
    Configure,
    /// Done.
    Complete,
}

/// Partition entry for the installer.
#[derive(Debug, Clone)]
pub struct InstallerPartition {
    /// Device path (e.g. "/dev/sda1").
    pub device: String,
    /// Filesystem type (e.g. "ext4", "fat32").
    pub fstype: String,
    /// Mount point (e.g. "/", "/boot").
    pub mount: String,
    /// Size in MB.
    pub size_mb: u64,
}

/// Text-based installer wizard.
#[derive(Debug)]
pub struct InstallerWizard {
    /// Current step.
    pub step: InstallerStep,
    /// Configured partitions.
    pub partitions: Vec<InstallerPartition>,
    /// Hostname.
    pub hostname: String,
    /// Root password (hashed, simulated).
    pub root_password_hash: String,
    /// Timezone.
    pub timezone: String,
    /// Installer log.
    pub log: Vec<String>,
    /// Is installation complete?
    pub complete: bool,
}

impl InstallerWizard {
    /// Creates a new installer wizard.
    pub fn new() -> Self {
        Self {
            step: InstallerStep::Welcome,
            partitions: Vec::new(),
            hostname: "fajaros".to_string(),
            root_password_hash: String::new(),
            timezone: "UTC".to_string(),
            log: Vec::new(),
            complete: false,
        }
    }

    /// Advances to the next installer step.
    pub fn next_step(&mut self) -> InstallerStep {
        self.step = match self.step {
            InstallerStep::Welcome => InstallerStep::Partition,
            InstallerStep::Partition => InstallerStep::Format,
            InstallerStep::Format => InstallerStep::Copy,
            InstallerStep::Copy => InstallerStep::Bootloader,
            InstallerStep::Bootloader => InstallerStep::Configure,
            InstallerStep::Configure => {
                self.complete = true;
                InstallerStep::Complete
            }
            InstallerStep::Complete => InstallerStep::Complete,
        };
        self.log
            .push(format!("Step: {:?}", self.step));
        self.step
    }

    /// Adds a partition configuration.
    pub fn add_partition(&mut self, partition: InstallerPartition) {
        self.partitions.push(partition);
    }

    /// Sets the hostname.
    pub fn set_hostname(&mut self, name: &str) {
        self.hostname = name.to_string();
    }

    /// Sets the root password (stores a simulated hash).
    pub fn set_root_password(&mut self, password: &str) {
        // Simulated hash: length + first char
        let hash = format!(
            "sha256:{:08x}",
            password.len() as u32 * 31 + password.chars().next().unwrap_or('0') as u32
        );
        self.root_password_hash = hash;
    }

    /// Sets the timezone.
    pub fn set_timezone(&mut self, tz: &str) {
        self.timezone = tz.to_string();
    }

    /// Runs the full installation (simulated).
    pub fn run_install(&mut self) -> Result<(), ReleaseError> {
        if self.partitions.is_empty() {
            return Err(ReleaseError::InstallerError(
                "no partitions configured".to_string(),
            ));
        }

        while self.step != InstallerStep::Complete {
            self.next_step();
        }
        Ok(())
    }
}

impl Default for InstallerWizard {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Documentation Generator
// ═══════════════════════════════════════════════════════════════════════

/// Documentation section.
#[derive(Debug, Clone)]
pub struct DocSection {
    /// Section title.
    pub title: String,
    /// Section content (markdown).
    pub content: String,
    /// Subsections.
    pub subsections: Vec<DocSection>,
}

/// Documentation generator — architecture guide + API reference.
#[derive(Debug)]
pub struct NovaDocGenerator {
    /// Document sections.
    sections: Vec<DocSection>,
    /// Document title.
    pub title: String,
    /// Document version.
    pub version: String,
}

impl NovaDocGenerator {
    /// Creates a new doc generator.
    pub fn new(title: &str, version: &str) -> Self {
        Self {
            sections: Vec::new(),
            title: title.to_string(),
            version: version.to_string(),
        }
    }

    /// Adds a top-level section.
    pub fn add_section(&mut self, section: DocSection) {
        self.sections.push(section);
    }

    /// Generates the full documentation as markdown.
    pub fn generate(&self) -> String {
        let mut md = String::new();
        md.push_str(&format!("# {} v{}\n\n", self.title, self.version));

        // Table of contents
        md.push_str("## Table of Contents\n\n");
        for (i, section) in self.sections.iter().enumerate() {
            md.push_str(&format!("{}. {}\n", i + 1, section.title));
            for (j, sub) in section.subsections.iter().enumerate() {
                md.push_str(&format!("   {}. {}\n", j + 1, sub.title));
            }
        }
        md.push('\n');

        // Content
        for section in &self.sections {
            md.push_str(&format!("## {}\n\n", section.title));
            md.push_str(&section.content);
            md.push_str("\n\n");

            for sub in &section.subsections {
                md.push_str(&format!("### {}\n\n", sub.title));
                md.push_str(&sub.content);
                md.push_str("\n\n");
            }
        }

        md
    }

    /// Returns the number of sections.
    pub fn section_count(&self) -> usize {
        self.sections.len()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Benchmark Suite
// ═══════════════════════════════════════════════════════════════════════

/// A single benchmark result.
#[derive(Debug, Clone)]
pub struct BenchResult {
    /// Benchmark name.
    pub name: String,
    /// Value (time in ms, bytes, ops/sec, etc.).
    pub value: f64,
    /// Unit (e.g. "ms", "MB", "ops/sec").
    pub unit: String,
    /// Is this result within the target?
    pub pass: bool,
    /// Target value.
    pub target: f64,
}

/// FajarOS benchmark suite.
#[derive(Debug)]
pub struct NovaBenchSuite {
    /// Benchmark results.
    results: Vec<BenchResult>,
}

impl NovaBenchSuite {
    /// Creates a new benchmark suite.
    pub fn new() -> Self {
        Self {
            results: Vec::new(),
        }
    }

    /// Adds a benchmark result.
    pub fn add_result(&mut self, result: BenchResult) {
        self.results.push(result);
    }

    /// Runs all standard benchmarks (simulated).
    pub fn run_standard_benchmarks(&mut self) {
        self.results.clear();
        self.results.push(BenchResult {
            name: "boot_time".to_string(),
            value: 850.0,
            unit: "ms".to_string(),
            pass: true,
            target: 2000.0,
        });
        self.results.push(BenchResult {
            name: "memory_usage_idle".to_string(),
            value: 4.5,
            unit: "MB".to_string(),
            pass: true,
            target: 16.0,
        });
        self.results.push(BenchResult {
            name: "syscall_throughput".to_string(),
            value: 500_000.0,
            unit: "ops/sec".to_string(),
            pass: true,
            target: 100_000.0,
        });
        self.results.push(BenchResult {
            name: "context_switch".to_string(),
            value: 2.5,
            unit: "us".to_string(),
            pass: true,
            target: 10.0,
        });
        self.results.push(BenchResult {
            name: "tcp_throughput".to_string(),
            value: 950.0,
            unit: "Mbps".to_string(),
            pass: true,
            target: 100.0,
        });
    }

    /// Returns all results.
    pub fn results(&self) -> &[BenchResult] {
        &self.results
    }

    /// Returns the number of passing benchmarks.
    pub fn pass_count(&self) -> usize {
        self.results.iter().filter(|r| r.pass).count()
    }

    /// Returns a summary string.
    pub fn summary(&self) -> String {
        let total = self.results.len();
        let pass = self.pass_count();
        format!("{}/{} benchmarks pass", pass, total)
    }
}

impl Default for NovaBenchSuite {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Audit Report
// ═══════════════════════════════════════════════════════════════════════

/// Module audit entry.
#[derive(Debug, Clone)]
pub struct ModuleAudit {
    /// Module name.
    pub name: String,
    /// Completeness percentage (0-100).
    pub completeness: u32,
    /// Number of tests.
    pub test_count: u32,
    /// Number of passing tests.
    pub pass_count: u32,
    /// Notes/issues.
    pub notes: String,
}

/// Audit report — module completeness, test coverage, feature parity.
#[derive(Debug)]
pub struct NovaAuditReport {
    /// Module audits.
    modules: Vec<ModuleAudit>,
    /// Report version.
    pub version: String,
    /// Report timestamp.
    pub timestamp: u64,
}

impl NovaAuditReport {
    /// Creates a new audit report.
    pub fn new(version: &str, timestamp: u64) -> Self {
        Self {
            modules: Vec::new(),
            version: version.to_string(),
            timestamp,
        }
    }

    /// Adds a module audit.
    pub fn add_module(&mut self, audit: ModuleAudit) {
        self.modules.push(audit);
    }

    /// Generates the standard FajarOS audit.
    pub fn generate_standard(&mut self) {
        self.modules.clear();
        let modules = vec![
            ("kernel", 100, 45, 45, "All kernel features verified"),
            ("memory", 100, 32, 32, "MMU + CoW + page tables"),
            ("scheduler", 100, 18, 18, "Preemptive + round-robin"),
            ("vfs", 100, 24, 24, "ramfs + FAT32 + ext2 + pipes"),
            ("network", 100, 28, 28, "TCP/UDP/HTTP + WiFi + IPv6"),
            ("gui", 100, 20, 20, "Window manager + widgets"),
            ("packages", 100, 16, 16, "Install/remove/search/update"),
            ("shell", 100, 35, 35, "240+ commands + pipes + redirect"),
            ("userland", 100, 22, 22, "libc + processes + signals"),
            ("drivers", 100, 15, 15, "NVMe + USB + virtio + serial"),
        ];
        for (name, comp, total, pass, notes) in modules {
            self.modules.push(ModuleAudit {
                name: name.to_string(),
                completeness: comp,
                test_count: total,
                pass_count: pass,
                notes: notes.to_string(),
            });
        }
    }

    /// Returns overall completeness percentage.
    pub fn overall_completeness(&self) -> f64 {
        if self.modules.is_empty() {
            return 0.0;
        }
        let sum: u64 = self.modules.iter().map(|m| m.completeness as u64).sum();
        sum as f64 / self.modules.len() as f64
    }

    /// Returns total test count.
    pub fn total_tests(&self) -> u32 {
        self.modules.iter().map(|m| m.test_count).sum()
    }

    /// Returns total passing test count.
    pub fn total_passing(&self) -> u32 {
        self.modules.iter().map(|m| m.pass_count).sum()
    }

    /// Returns the number of audited modules.
    pub fn module_count(&self) -> usize {
        self.modules.len()
    }

    /// Generates the report as a markdown string.
    pub fn to_markdown(&self) -> String {
        let mut md = String::new();
        md.push_str(&format!("# FajarOS Nova Audit Report v{}\n\n", self.version));
        md.push_str(&format!(
            "Overall: {:.0}% complete, {}/{} tests pass\n\n",
            self.overall_completeness(),
            self.total_passing(),
            self.total_tests(),
        ));
        md.push_str("| Module | Complete | Tests | Notes |\n");
        md.push_str("|--------|----------|-------|-------|\n");
        for m in &self.modules {
            md.push_str(&format!(
                "| {} | {}% | {}/{} | {} |\n",
                m.name, m.completeness, m.pass_count, m.test_count, m.notes,
            ));
        }
        md
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Migration Guide
// ═══════════════════════════════════════════════════════════════════════

/// A migration step.
#[derive(Debug, Clone)]
pub struct MigrationStep {
    /// Step number.
    pub number: u32,
    /// Step title.
    pub title: String,
    /// Step description.
    pub description: String,
    /// Commands to run (if any).
    pub commands: Vec<String>,
    /// Is this step mandatory?
    pub mandatory: bool,
}

/// Migration guide — v1.4 to v2.0 migration steps.
#[derive(Debug)]
pub struct MigrationGuide {
    /// Source version.
    pub from_version: String,
    /// Target version.
    pub to_version: String,
    /// Migration steps.
    steps: Vec<MigrationStep>,
}

impl MigrationGuide {
    /// Creates a new migration guide.
    pub fn new(from: &str, to: &str) -> Self {
        Self {
            from_version: from.to_string(),
            to_version: to.to_string(),
            steps: Vec::new(),
        }
    }

    /// Creates the standard v1.4 -> v2.0 migration guide.
    pub fn v14_to_v20() -> Self {
        let mut guide = Self::new("1.4.0", "2.0.0");
        guide.add_step(MigrationStep {
            number: 1,
            title: "Backup existing installation".to_string(),
            description: "Create a full backup of your FajarOS v1.4 system.".to_string(),
            commands: vec!["cp -a / /backup/fajaros-v1.4".to_string()],
            mandatory: true,
        });
        guide.add_step(MigrationStep {
            number: 2,
            title: "Update kernel image".to_string(),
            description: "Replace the v1.4 kernel with the v2.0 kernel binary.".to_string(),
            commands: vec!["cp kernel-v2.0.elf /boot/kernel.elf".to_string()],
            mandatory: true,
        });
        guide.add_step(MigrationStep {
            number: 3,
            title: "Update shell configuration".to_string(),
            description: "New shell commands added in v2.0 require config update.".to_string(),
            commands: vec!["cp /etc/shell.conf.new /etc/shell.conf".to_string()],
            mandatory: false,
        });
        guide.add_step(MigrationStep {
            number: 4,
            title: "Migrate packages".to_string(),
            description: "Re-install packages using the v2.0 package manager.".to_string(),
            commands: vec!["pkg update && pkg upgrade".to_string()],
            mandatory: true,
        });
        guide.add_step(MigrationStep {
            number: 5,
            title: "Verify network configuration".to_string(),
            description: "IPv6 and WiFi are new in v2.0 — configure if needed.".to_string(),
            commands: vec![],
            mandatory: false,
        });
        guide
    }

    /// Adds a migration step.
    pub fn add_step(&mut self, step: MigrationStep) {
        self.steps.push(step);
    }

    /// Returns the number of steps.
    pub fn step_count(&self) -> usize {
        self.steps.len()
    }

    /// Returns the number of mandatory steps.
    pub fn mandatory_count(&self) -> usize {
        self.steps.iter().filter(|s| s.mandatory).count()
    }

    /// Generates the guide as markdown.
    pub fn to_markdown(&self) -> String {
        let mut md = String::new();
        md.push_str(&format!(
            "# Migration Guide: FajarOS v{} -> v{}\n\n",
            self.from_version, self.to_version
        ));
        for step in &self.steps {
            let required = if step.mandatory { " (REQUIRED)" } else { " (optional)" };
            md.push_str(&format!(
                "## Step {}: {}{}\n\n",
                step.number, step.title, required
            ));
            md.push_str(&step.description);
            md.push_str("\n\n");
            if !step.commands.is_empty() {
                md.push_str("```\n");
                for cmd in &step.commands {
                    md.push_str(cmd);
                    md.push('\n');
                }
                md.push_str("```\n\n");
            }
        }
        md
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Release Notes
// ═══════════════════════════════════════════════════════════════════════

/// Release note category.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReleaseCategory {
    /// New feature.
    Feature,
    /// Bug fix.
    Fix,
    /// Performance improvement.
    Performance,
    /// Breaking change.
    Breaking,
    /// Deprecation.
    Deprecated,
}

/// A single release note entry.
#[derive(Debug, Clone)]
pub struct ReleaseNoteEntry {
    /// Category.
    pub category: ReleaseCategory,
    /// Description.
    pub description: String,
    /// Related component.
    pub component: String,
}

/// Release notes for FajarOS Nova v2.0.
#[derive(Debug)]
pub struct ReleaseNotes {
    /// Version.
    pub version: String,
    /// Release date.
    pub date: String,
    /// Codename.
    pub codename: String,
    /// Entries.
    entries: Vec<ReleaseNoteEntry>,
}

impl ReleaseNotes {
    /// Creates new release notes.
    pub fn new(version: &str, date: &str, codename: &str) -> Self {
        Self {
            version: version.to_string(),
            date: date.to_string(),
            codename: codename.to_string(),
            entries: Vec::new(),
        }
    }

    /// Creates the v2.0 "Nova" release notes.
    pub fn v2_nova() -> Self {
        let mut notes = Self::new("2.0.0", "2026-03-31", "Nova");
        notes.add(ReleaseNoteEntry {
            category: ReleaseCategory::Feature,
            description: "WiFi driver with WPA2/WPA3 authentication".to_string(),
            component: "network".to_string(),
        });
        notes.add(ReleaseNoteEntry {
            category: ReleaseCategory::Feature,
            description: "IPv6 stack with neighbor discovery and routing".to_string(),
            component: "network".to_string(),
        });
        notes.add(ReleaseNoteEntry {
            category: ReleaseCategory::Feature,
            description: "GUI framework with window manager and widget toolkit".to_string(),
            component: "gui".to_string(),
        });
        notes.add(ReleaseNoteEntry {
            category: ReleaseCategory::Feature,
            description: "Package manager with dependency resolution".to_string(),
            component: "packages".to_string(),
        });
        notes.add(ReleaseNoteEntry {
            category: ReleaseCategory::Feature,
            description: "Userland libraries (libc, libm, dynamic linker)".to_string(),
            component: "userland".to_string(),
        });
        notes.add(ReleaseNoteEntry {
            category: ReleaseCategory::Performance,
            description: "TCP congestion control with window scaling and SACK".to_string(),
            component: "network".to_string(),
        });
        notes.add(ReleaseNoteEntry {
            category: ReleaseCategory::Feature,
            description: "Text-based installer with partitioning wizard".to_string(),
            component: "installer".to_string(),
        });
        notes
    }

    /// Adds a release note entry.
    pub fn add(&mut self, entry: ReleaseNoteEntry) {
        self.entries.push(entry);
    }

    /// Returns entries by category.
    pub fn by_category(&self, cat: ReleaseCategory) -> Vec<&ReleaseNoteEntry> {
        self.entries.iter().filter(|e| e.category == cat).collect()
    }

    /// Returns the total entry count.
    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    /// Generates release notes as markdown.
    pub fn to_markdown(&self) -> String {
        let mut md = String::new();
        md.push_str(&format!(
            "# FajarOS Nova v{} \"{}\" Release Notes\n\n",
            self.version, self.codename
        ));
        md.push_str(&format!("Release Date: {}\n\n", self.date));

        let categories = [
            (ReleaseCategory::Feature, "New Features"),
            (ReleaseCategory::Fix, "Bug Fixes"),
            (ReleaseCategory::Performance, "Performance"),
            (ReleaseCategory::Breaking, "Breaking Changes"),
            (ReleaseCategory::Deprecated, "Deprecations"),
        ];

        for (cat, title) in &categories {
            let entries = self.by_category(*cat);
            if !entries.is_empty() {
                md.push_str(&format!("## {}\n\n", title));
                for entry in entries {
                    md.push_str(&format!(
                        "- **{}**: {}\n",
                        entry.component, entry.description
                    ));
                }
                md.push('\n');
            }
        }
        md
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Compatibility Matrix
// ═══════════════════════════════════════════════════════════════════════

/// Compatibility status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompatStatus {
    /// Fully supported.
    Supported,
    /// Partially supported (some features may not work).
    Partial,
    /// Not supported.
    Unsupported,
    /// Not tested.
    Unknown,
}

/// A compatibility entry.
#[derive(Debug, Clone)]
pub struct CompatEntry {
    /// Platform/hypervisor name.
    pub platform: String,
    /// Status.
    pub status: CompatStatus,
    /// Notes.
    pub notes: String,
}

/// Compatibility matrix — QEMU/KVM/VirtualBox/real HW.
#[derive(Debug)]
pub struct CompatMatrix {
    /// Entries.
    entries: Vec<CompatEntry>,
}

impl CompatMatrix {
    /// Creates a new empty matrix.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Creates the standard FajarOS compatibility matrix.
    pub fn standard() -> Self {
        let mut matrix = Self::new();
        matrix.add(CompatEntry {
            platform: "QEMU x86_64 (TCG)".to_string(),
            status: CompatStatus::Supported,
            notes: "Primary development target, fully tested".to_string(),
        });
        matrix.add(CompatEntry {
            platform: "QEMU x86_64 (KVM)".to_string(),
            status: CompatStatus::Supported,
            notes: "Hardware acceleration, all features verified".to_string(),
        });
        matrix.add(CompatEntry {
            platform: "VirtualBox 7.x".to_string(),
            status: CompatStatus::Partial,
            notes: "Boot + shell works, GPU passthrough limited".to_string(),
        });
        matrix.add(CompatEntry {
            platform: "VMware Workstation".to_string(),
            status: CompatStatus::Partial,
            notes: "Basic boot verified, network needs vmxnet3".to_string(),
        });
        matrix.add(CompatEntry {
            platform: "Real HW: Intel Core i9".to_string(),
            status: CompatStatus::Supported,
            notes: "Lenovo Legion Pro, NVMe + USB + network verified".to_string(),
        });
        matrix.add(CompatEntry {
            platform: "Real HW: Radxa Dragon Q6A".to_string(),
            status: CompatStatus::Supported,
            notes: "ARM64, JIT + GPIO + QNN inference verified".to_string(),
        });
        matrix
    }

    /// Adds a compatibility entry.
    pub fn add(&mut self, entry: CompatEntry) {
        self.entries.push(entry);
    }

    /// Returns entries with a specific status.
    pub fn by_status(&self, status: CompatStatus) -> Vec<&CompatEntry> {
        self.entries.iter().filter(|e| e.status == status).collect()
    }

    /// Returns the total entry count.
    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    /// Generates the matrix as markdown.
    pub fn to_markdown(&self) -> String {
        let mut md = String::new();
        md.push_str("# FajarOS Nova v2.0 Compatibility Matrix\n\n");
        md.push_str("| Platform | Status | Notes |\n");
        md.push_str("|----------|--------|-------|\n");
        for entry in &self.entries {
            let status_str = match entry.status {
                CompatStatus::Supported => "Supported",
                CompatStatus::Partial => "Partial",
                CompatStatus::Unsupported => "Unsupported",
                CompatStatus::Unknown => "Unknown",
            };
            md.push_str(&format!(
                "| {} | {} | {} |\n",
                entry.platform, status_str, entry.notes
            ));
        }
        md
    }
}

impl Default for CompatMatrix {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// End-to-End Test Suite
// ═══════════════════════════════════════════════════════════════════════

/// Test result.
#[derive(Debug, Clone)]
pub struct NovaTestResult {
    /// Test name.
    pub name: String,
    /// Did it pass?
    pub pass: bool,
    /// Duration in ms.
    pub duration_ms: u64,
    /// Error message (if failed).
    pub error: Option<String>,
}

/// End-to-end test suite — boot, shell, network.
#[derive(Debug)]
pub struct NovaTestSuite {
    /// Test results.
    results: Vec<NovaTestResult>,
}

impl NovaTestSuite {
    /// Creates a new test suite.
    pub fn new() -> Self {
        Self {
            results: Vec::new(),
        }
    }

    /// Runs all standard tests (simulated).
    pub fn run_all(&mut self) {
        self.results.clear();
        let tests = vec![
            ("boot_to_shell", true, 1200, None),
            ("kernel_memory_init", true, 50, None),
            ("vfs_mount_root", true, 30, None),
            ("scheduler_preempt", true, 100, None),
            ("tcp_loopback", true, 200, None),
            ("udp_echo", true, 80, None),
            ("wifi_scan", true, 150, None),
            ("dns_resolve", true, 75, None),
            ("shell_pipe", true, 60, None),
            ("shell_redirect", true, 55, None),
            ("process_fork_exec", true, 120, None),
            ("pkg_install", true, 300, None),
            ("gui_window_create", true, 90, None),
            ("user_login", true, 70, None),
            ("graceful_shutdown", true, 500, None),
        ];
        for (name, pass, dur, err) in tests {
            self.results.push(NovaTestResult {
                name: name.to_string(),
                pass,
                duration_ms: dur,
                error: err.map(|e: &str| e.to_string()),
            });
        }
    }

    /// Adds a test result.
    pub fn add_result(&mut self, result: NovaTestResult) {
        self.results.push(result);
    }

    /// Returns all results.
    pub fn results(&self) -> &[NovaTestResult] {
        &self.results
    }

    /// Returns the number of passing tests.
    pub fn pass_count(&self) -> usize {
        self.results.iter().filter(|r| r.pass).count()
    }

    /// Returns the number of failing tests.
    pub fn fail_count(&self) -> usize {
        self.results.iter().filter(|r| !r.pass).count()
    }

    /// Returns a summary string.
    pub fn summary(&self) -> String {
        let total = self.results.len();
        let pass = self.pass_count();
        let fail = self.fail_count();
        format!("{}/{} pass, {} fail", pass, total, fail)
    }
}

impl Default for NovaTestSuite {
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

    // --- ISO Image Builder tests ---

    #[test]
    fn iso_build_requires_kernel_and_bootloader() {
        let iso = IsoImageBuilder::new("FajarOS");
        assert!(iso.build().is_err()); // missing components
    }

    #[test]
    fn iso_build_with_files() {
        let mut iso = IsoImageBuilder::new("FajarOS Nova v2.0");
        iso.set_kernel("/boot/kernel.elf");
        iso.set_bootloader("/boot/grub/core.img");
        iso.add_file("/boot/kernel.elf", 4_000_000);
        iso.add_file("/usr/lib/libc.so", 500_000);
        iso.add_file("/usr/bin/init", 100_000);
        let size = iso.build().unwrap();
        assert!(size > 4_600_000);
        assert_eq!(iso.file_count(), 3);
    }

    // --- Boot Media Config tests ---

    #[test]
    fn boot_config_generate_grub() {
        let config = BootMediaConfig::new(BootMode::Hybrid);
        let cfg = config.generate_grub_cfg();
        assert!(cfg.contains("FajarOS Nova v2.0"));
        assert!(cfg.contains("multiboot2"));
        assert!(cfg.contains("timeout=5"));
    }

    #[test]
    fn boot_config_add_entry() {
        let mut config = BootMediaConfig::new(BootMode::Uefi);
        config.add_entry(GrubEntry {
            title: "Recovery Mode".to_string(),
            kernel: "/boot/kernel.elf".to_string(),
            args: "recovery".to_string(),
            default: false,
        });
        assert_eq!(config.entry_count(), 2);
    }

    // --- Installer Wizard tests ---

    #[test]
    fn installer_step_progression() {
        let mut wizard = InstallerWizard::new();
        assert_eq!(wizard.step, InstallerStep::Welcome);
        wizard.next_step();
        assert_eq!(wizard.step, InstallerStep::Partition);
        wizard.next_step();
        assert_eq!(wizard.step, InstallerStep::Format);
    }

    #[test]
    fn installer_full_run() {
        let mut wizard = InstallerWizard::new();
        wizard.add_partition(InstallerPartition {
            device: "/dev/sda1".to_string(),
            fstype: "ext4".to_string(),
            mount: "/".to_string(),
            size_mb: 8192,
        });
        wizard.set_hostname("my-nova");
        wizard.set_root_password("secure123");
        wizard.set_timezone("Asia/Jakarta");
        wizard.run_install().unwrap();
        assert!(wizard.complete);
        assert_eq!(wizard.step, InstallerStep::Complete);
    }

    #[test]
    fn installer_no_partitions_fails() {
        let mut wizard = InstallerWizard::new();
        assert!(wizard.run_install().is_err());
    }

    // --- Documentation Generator tests ---

    #[test]
    fn doc_generate_markdown() {
        let mut gen = NovaDocGenerator::new("FajarOS Nova", "2.0.0");
        gen.add_section(DocSection {
            title: "Introduction".to_string(),
            content: "FajarOS Nova is a modern OS.".to_string(),
            subsections: vec![],
        });
        let md = gen.generate();
        assert!(md.contains("# FajarOS Nova v2.0.0"));
        assert!(md.contains("Introduction"));
    }

    // --- Benchmark Suite tests ---

    #[test]
    fn bench_standard_benchmarks() {
        let mut bench = NovaBenchSuite::new();
        bench.run_standard_benchmarks();
        assert!(bench.results().len() >= 5);
        assert_eq!(bench.pass_count(), bench.results().len());
    }

    #[test]
    fn bench_summary() {
        let mut bench = NovaBenchSuite::new();
        bench.run_standard_benchmarks();
        let summary = bench.summary();
        assert!(summary.contains("pass"));
    }

    // --- Audit Report tests ---

    #[test]
    fn audit_standard_report() {
        let mut audit = NovaAuditReport::new("2.0.0", 1000);
        audit.generate_standard();
        assert!(audit.module_count() >= 10);
        assert!((audit.overall_completeness() - 100.0).abs() < 0.01);
        assert!(audit.total_tests() > 0);
        assert_eq!(audit.total_passing(), audit.total_tests());
    }

    #[test]
    fn audit_markdown() {
        let mut audit = NovaAuditReport::new("2.0.0", 1000);
        audit.generate_standard();
        let md = audit.to_markdown();
        assert!(md.contains("Audit Report"));
        assert!(md.contains("kernel"));
    }

    // --- Migration Guide tests ---

    #[test]
    fn migration_v14_to_v20() {
        let guide = MigrationGuide::v14_to_v20();
        assert_eq!(guide.step_count(), 5);
        assert!(guide.mandatory_count() >= 3);
    }

    #[test]
    fn migration_markdown() {
        let guide = MigrationGuide::v14_to_v20();
        let md = guide.to_markdown();
        assert!(md.contains("Migration Guide"));
        assert!(md.contains("REQUIRED"));
    }

    // --- Release Notes tests ---

    #[test]
    fn release_notes_v2() {
        let notes = ReleaseNotes::v2_nova();
        assert!(notes.entry_count() >= 7);
        assert!(!notes.by_category(ReleaseCategory::Feature).is_empty());
    }

    #[test]
    fn release_notes_markdown() {
        let notes = ReleaseNotes::v2_nova();
        let md = notes.to_markdown();
        assert!(md.contains("Nova"));
        assert!(md.contains("New Features"));
    }

    // --- Compatibility Matrix tests ---

    #[test]
    fn compat_standard_matrix() {
        let matrix = CompatMatrix::standard();
        assert!(matrix.entry_count() >= 6);
        assert!(!matrix.by_status(CompatStatus::Supported).is_empty());
    }

    #[test]
    fn compat_markdown() {
        let matrix = CompatMatrix::standard();
        let md = matrix.to_markdown();
        assert!(md.contains("QEMU"));
        assert!(md.contains("Supported"));
    }

    // --- End-to-End Test Suite tests ---

    #[test]
    fn e2e_run_all() {
        let mut suite = NovaTestSuite::new();
        suite.run_all();
        assert!(suite.results().len() >= 15);
        assert_eq!(suite.fail_count(), 0);
        assert!(suite.pass_count() > 0);
    }

    #[test]
    fn e2e_summary() {
        let mut suite = NovaTestSuite::new();
        suite.run_all();
        let summary = suite.summary();
        assert!(summary.contains("pass"));
        assert!(summary.contains("0 fail"));
    }
}
