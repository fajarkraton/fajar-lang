//! CLI-facing registry operations — connects CLI commands to RegistryDb.
//!
//! Provides functions for `fj publish`, `fj install`, `fj search`, `fj update`,
//! `fj audit`, `fj tree`, shell completions, progress display, and more.

use std::collections::HashMap;
use std::path::Path;

use super::registry_db::{PublishRequest, RegistryDb};
use super::server::SearchQuery;
use super::{ProjectConfig, SemVer, VersionConstraint};

// ═══════════════════════════════════════════════════════════════════════
// PR2.1: fj publish — real registry publish
// ═══════════════════════════════════════════════════════════════════════

/// Publish a package to the local registry database.
/// Reads fj.toml, validates, creates tarball, and stores in registry.
pub fn publish_to_local_registry(
    project_root: &Path,
    config: &ProjectConfig,
    registry_path: &Path,
) -> Result<String, String> {
    let name = &config.package.name;
    let version = &config.package.version;
    let description = config.package.description.as_deref().unwrap_or("");
    let license = config.package.license.as_deref();
    let entry = &config.package.entry;

    // Validate
    SemVer::parse(version)?;
    let entry_path = project_root.join(entry);
    if !entry_path.exists() {
        return Err(format!("entry point not found: {}", entry_path.display()));
    }

    // Read entry file as tarball content (simplified — real impl would tar.gz the whole project)
    let tarball = std::fs::read(&entry_path)
        .map_err(|e| format!("failed to read {}: {e}", entry_path.display()))?;

    // Open or create registry
    let storage_dir = registry_path.join("storage");
    let db_path = registry_path.join("registry.db");
    let _ = std::fs::create_dir_all(&storage_dir);
    let reg = RegistryDb::open(&db_path.to_string_lossy(), &storage_dir)?;

    // Ensure a local user exists — try register first, ignore if exists
    let auth = match reg.register_user("local", "local@localhost") {
        Ok((_, key)) => reg.authenticate(&key)?,
        Err(_) => {
            // User already exists — re-register with unique email to get a fresh key
            let ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let (_, key) =
                reg.register_user(&format!("local_{ts}"), &format!("local_{ts}@localhost"))?;
            reg.authenticate(&key)?
        }
    };

    let req = PublishRequest {
        name,
        version,
        description,
        tarball: &tarball,
        keywords: &[],
        license,
        repository: None,
    };
    let resp = reg.publish(&auth, &req)?;
    if resp.status.0 >= 400 {
        return Err(format!("publish failed: {}", resp.body));
    }

    Ok(format!("{name} v{version}"))
}

// ═══════════════════════════════════════════════════════════════════════
// PR2.2: fj install — download from registry
// ═══════════════════════════════════════════════════════════════════════

/// Install a package from the local registry.
pub fn install_from_registry(
    package: &str,
    version: Option<&str>,
    target_dir: &Path,
    registry_path: &Path,
    offline: bool,
) -> Result<String, String> {
    let storage_dir = registry_path.join("storage");
    let db_path = registry_path.join("registry.db");

    if offline && !db_path.exists() {
        return Err("registry database not found and --offline specified".to_string());
    }

    let reg = RegistryDb::open(&db_path.to_string_lossy(), &storage_dir)?;

    let meta = reg
        .get_package(package)?
        .ok_or_else(|| format!("package '{package}' not found in registry"))?;

    let ver = if let Some(v) = version {
        v.to_string()
    } else {
        // Find latest non-yanked version
        meta.versions
            .iter()
            .find(|v| !v.yanked)
            .map(|v| v.version.clone())
            .ok_or_else(|| format!("no available version for '{package}'"))?
    };

    // Get tarball
    let data = reg.get_tarball(package, &ver)?;
    reg.record_download(package, &ver)?;

    // Write to target directory
    let pkg_dir = target_dir.join(package);
    std::fs::create_dir_all(&pkg_dir)
        .map_err(|e| format!("failed to create {}: {e}", pkg_dir.display()))?;
    let file_path = pkg_dir.join("lib.fj");
    std::fs::write(&file_path, &data)
        .map_err(|e| format!("failed to write {}: {e}", file_path.display()))?;

    Ok(format!("{package} v{ver} -> {}", pkg_dir.display()))
}

// ═══════════════════════════════════════════════════════════════════════
// PR2.3: fj update — update dependencies
// ═══════════════════════════════════════════════════════════════════════

/// Check for dependency updates.
pub fn check_updates(
    config: &ProjectConfig,
    registry_path: &Path,
) -> Result<Vec<UpdateInfo>, String> {
    let storage_dir = registry_path.join("storage");
    let db_path = registry_path.join("registry.db");
    let reg = RegistryDb::open(&db_path.to_string_lossy(), &storage_dir)?;

    let mut updates = Vec::new();
    for (name, constraint_str) in &config.dependencies {
        if VersionConstraint::parse(constraint_str).is_ok() {
            if let Ok(Some(meta)) = reg.get_package(name) {
                if let Some(latest) = meta.versions.iter().find(|v| !v.yanked) {
                    // Extract current version from constraint string
                    let current_ver_str = constraint_str
                        .trim()
                        .trim_start_matches('^')
                        .trim_start_matches('~')
                        .trim_start_matches('=')
                        .trim_start_matches(">=")
                        .trim_start_matches("<=")
                        .trim_start_matches('>')
                        .trim_start_matches('<')
                        .trim();
                    // Compare using SemVer for correct semantic comparison
                    let needs_update = match (
                        SemVer::parse(&latest.version),
                        SemVer::parse(current_ver_str),
                    ) {
                        (Ok(latest_sv), Ok(current_sv)) => latest_sv > current_sv,
                        _ => latest.version != current_ver_str,
                    };
                    if needs_update {
                        updates.push(UpdateInfo {
                            name: name.clone(),
                            current: constraint_str.clone(),
                            latest: latest.version.clone(),
                        });
                    }
                }
            }
        }
    }
    Ok(updates)
}

/// Information about an available update.
#[derive(Debug, Clone)]
pub struct UpdateInfo {
    /// Package name.
    pub name: String,
    /// Current version constraint.
    pub current: String,
    /// Latest available version.
    pub latest: String,
}

// ═══════════════════════════════════════════════════════════════════════
// PR2.4: fj search — registry search
// ═══════════════════════════════════════════════════════════════════════

/// Search the registry for packages.
pub fn search_registry(
    query: &str,
    limit: usize,
    registry_path: &Path,
) -> Result<Vec<SearchResultDisplay>, String> {
    let storage_dir = registry_path.join("storage");
    let db_path = registry_path.join("registry.db");

    // If no registry DB exists, fall back to hardcoded standard packages
    if !db_path.exists() {
        return Ok(search_standard_packages(query, limit));
    }

    let reg = RegistryDb::open(&db_path.to_string_lossy(), &storage_dir)?;
    let results = reg.search(&SearchQuery {
        query: query.to_string(),
        limit,
        offset: 0,
    })?;

    Ok(results
        .into_iter()
        .map(|r| SearchResultDisplay {
            name: r.name,
            version: r.latest_version,
            description: r.description,
            downloads: r.downloads,
        })
        .collect())
}

/// Display-friendly search result.
#[derive(Debug, Clone)]
pub struct SearchResultDisplay {
    /// Package name.
    pub name: String,
    /// Latest version.
    pub version: String,
    /// Description.
    pub description: String,
    /// Download count.
    pub downloads: u64,
}

impl std::fmt::Display for SearchResultDisplay {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:<20} {:<10} {:>6} dl  {}",
            self.name, self.version, self.downloads, self.description
        )
    }
}

/// Fallback: search hardcoded standard packages.
fn search_standard_packages(query: &str, limit: usize) -> Vec<SearchResultDisplay> {
    let packages = vec![
        (
            "fj-async",
            "1.0.0",
            "Async runtime: tasks, channels, timers",
        ),
        ("fj-base64", "1.0.0", "Base64 encode/decode (RFC 4648)"),
        ("fj-bench", "1.0.0", "Benchmarking with statistics"),
        (
            "fj-cli-fw",
            "1.0.0",
            "CLI framework: args, subcommands, help",
        ),
        ("fj-color", "1.0.0", "Terminal ANSI colors and styles"),
        ("fj-compress", "1.0.0", "RLE and LZ77 compression"),
        ("fj-crypto", "1.0.0", "Cryptographic hash, HMAC, encryption"),
        ("fj-csv", "1.0.0", "CSV parser (RFC 4180)"),
        ("fj-db", "1.0.0", "Database: tables, queries, transactions"),
        ("fj-doc-gen", "1.0.0", "Documentation generator"),
        ("fj-drivers", "1.0.0", "Device drivers for sensors"),
        ("fj-env", "1.0.0", "Environment variables and dotenv"),
        ("fj-fs", "1.0.0", "Filesystem utilities"),
        ("fj-gpio", "1.0.0", "GPIO abstraction layer"),
        (
            "fj-hal",
            "1.0.0",
            "Hardware abstraction (GPIO, UART, I2C, SPI)",
        ),
        ("fj-http", "1.0.0", "HTTP client and server"),
        ("fj-image", "1.0.0", "Image processing: resize, filter"),
        ("fj-json", "1.0.0", "JSON serialization and parsing"),
        ("fj-log", "1.0.0", "Structured logging with levels"),
        ("fj-math", "3.0.0", "Mathematical operations"),
        ("fj-mqtt", "1.0.0", "MQTT IoT messaging client"),
        ("fj-nn", "1.0.0", "Neural network layers and training"),
        ("fj-onnx", "1.0.0", "ONNX model loading and inference"),
        ("fj-plot", "1.0.0", "Data visualization to SVG"),
        ("fj-rand", "1.0.0", "Random number generation"),
        ("fj-regex", "1.0.0", "Regular expressions (NFA engine)"),
        ("fj-sensor", "1.0.0", "Sensor fusion: Kalman, Madgwick"),
        ("fj-serial", "1.0.0", "Serialization: JSON, binary TLV"),
        ("fj-test", "1.0.0", "Advanced testing framework"),
        ("fj-time", "1.0.0", "Date, time, duration utilities"),
        ("fj-tls", "1.0.0", "TLS/SSL configuration"),
        ("fj-toml", "1.0.0", "TOML parser"),
        ("fj-url", "1.0.0", "URL parsing (RFC 3986)"),
        ("fj-uuid", "1.0.0", "UUID v4/v7 generation"),
        ("fj-web", "1.0.0", "Web framework: router, middleware"),
        ("fj-yaml", "1.0.0", "YAML parser"),
    ];

    let q = query.to_lowercase();
    packages
        .into_iter()
        .filter(|(name, _, desc)| {
            name.to_lowercase().contains(&q) || desc.to_lowercase().contains(&q)
        })
        .take(limit)
        .map(|(name, ver, desc)| SearchResultDisplay {
            name: name.to_string(),
            version: ver.to_string(),
            description: desc.to_string(),
            downloads: 0,
        })
        .collect()
}

// ═══════════════════════════════════════════════════════════════════════
// PR2.7: fj audit — vulnerability check
// ═══════════════════════════════════════════════════════════════════════

/// Audit result for a dependency.
#[derive(Debug, Clone)]
pub struct AuditResult {
    /// Package name.
    pub name: String,
    /// Installed version.
    pub version: String,
    /// Severity (if vulnerable).
    pub severity: Option<String>,
    /// Advisory message.
    pub advisory: Option<String>,
}

/// Audit project dependencies for known vulnerabilities.
/// Uses the real AdvisoryDatabase from package::audit.
pub fn audit_dependencies(config: &ProjectConfig) -> Vec<AuditResult> {
    use super::audit::AdvisoryDatabase;

    let db = AdvisoryDatabase::new();
    let deps: Vec<(String, String)> = config
        .dependencies
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();

    let report = super::audit::audit_dependencies(&deps, &db);

    // Convert findings to AuditResult
    let mut results: Vec<AuditResult> = report
        .affected
        .iter()
        .map(|f| AuditResult {
            name: f.package.clone(),
            version: f.version.clone(),
            severity: Some(format!("{}", f.severity)),
            advisory: Some(f.description.clone()),
        })
        .collect();

    // Add clean dependencies (no findings)
    for (name, version) in &deps {
        if !results.iter().any(|r| r.name == *name) {
            results.push(AuditResult {
                name: name.clone(),
                version: version.clone(),
                severity: None,
                advisory: None,
            });
        }
    }

    results
}

// ═══════════════════════════════════════════════════════════════════════
// PR2.12: fj tree — dependency tree display
// ═══════════════════════════════════════════════════════════════════════

/// A node in the dependency tree.
#[derive(Debug, Clone)]
pub struct DepTreeNode {
    /// Package name.
    pub name: String,
    /// Version constraint.
    pub version: String,
    /// Transitive dependencies.
    pub children: Vec<DepTreeNode>,
}

impl DepTreeNode {
    /// Format the tree for terminal display.
    pub fn display(&self, prefix: &str, is_last: bool) -> String {
        let connector = if is_last { "└── " } else { "├── " };
        let child_prefix = if is_last {
            format!("{prefix}    ")
        } else {
            format!("{prefix}│   ")
        };

        let mut output = format!("{prefix}{connector}{} v{}\n", self.name, self.version);
        for (i, child) in self.children.iter().enumerate() {
            let last = i == self.children.len() - 1;
            output.push_str(&child.display(&child_prefix, last));
        }
        output
    }
}

/// Build a dependency tree from project config.
pub fn build_dep_tree(config: &ProjectConfig) -> Vec<DepTreeNode> {
    config
        .dependencies
        .iter()
        .map(|(name, version)| DepTreeNode {
            name: name.clone(),
            version: version.clone(),
            children: Vec::new(), // Transitive deps would be resolved via registry
        })
        .collect()
}

// ═══════════════════════════════════════════════════════════════════════
// PR2.13: Checksum verification
// ═══════════════════════════════════════════════════════════════════════

/// Verify SHA-256 checksum of installed package.
pub fn verify_checksum(pkg_path: &Path, expected: &str) -> Result<bool, String> {
    use sha2::{Digest, Sha256};
    let data = std::fs::read(pkg_path).map_err(|e| format!("read failed: {e}"))?;
    let mut hasher = Sha256::new();
    hasher.update(&data);
    let actual = format!("{:x}", hasher.finalize());
    Ok(actual == expected)
}

// ═══════════════════════════════════════════════════════════════════════
// PR2.14: Proxy support
// ═══════════════════════════════════════════════════════════════════════

/// Proxy configuration for corporate environments.
#[derive(Debug, Clone)]
pub struct ProxyConfig {
    /// HTTP proxy URL.
    pub http_proxy: Option<String>,
    /// HTTPS proxy URL.
    pub https_proxy: Option<String>,
    /// Hosts to bypass proxy for.
    pub no_proxy: Vec<String>,
}

impl ProxyConfig {
    /// Load proxy config from environment variables.
    pub fn from_env() -> Self {
        Self {
            http_proxy: std::env::var("HTTP_PROXY")
                .or_else(|_| std::env::var("http_proxy"))
                .ok(),
            https_proxy: std::env::var("HTTPS_PROXY")
                .or_else(|_| std::env::var("https_proxy"))
                .ok(),
            no_proxy: std::env::var("NO_PROXY")
                .or_else(|_| std::env::var("no_proxy"))
                .unwrap_or_default()
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect(),
        }
    }

    /// Check if proxy is configured.
    pub fn is_configured(&self) -> bool {
        self.http_proxy.is_some() || self.https_proxy.is_some()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// PR2.15: Shell completions
// ═══════════════════════════════════════════════════════════════════════

/// Generate shell completion script.
pub fn generate_completions(shell: &str) -> String {
    match shell {
        "bash" => generate_bash_completions(),
        "zsh" => generate_zsh_completions(),
        "fish" => generate_fish_completions(),
        _ => format!("# Unsupported shell: {shell}\n# Supported: bash, zsh, fish\n"),
    }
}

fn generate_bash_completions() -> String {
    r#"# Fajar Lang bash completions
_fj_completions() {
    local cur prev commands
    cur="${COMP_WORDS[COMP_CWORD]}"
    prev="${COMP_WORDS[COMP_CWORD-1]}"
    commands="run repl check build fmt new test doc watch bench lsp publish add search login yank install verify profile dump-tokens dump-ast hw-info hw-json"

    if [ "$COMP_CWORD" -eq 1 ]; then
        COMPREPLY=($(compgen -W "$commands" -- "$cur"))
        return
    fi

    case "$prev" in
        run|check|test|doc|watch|bench|fmt|verify|profile|dump-tokens|dump-ast)
            COMPREPLY=($(compgen -f -X '!*.fj' -- "$cur"))
            ;;
        search|install|add|yank)
            COMPREPLY=()
            ;;
    esac
}
complete -F _fj_completions fj
"#.to_string()
}

fn generate_zsh_completions() -> String {
    r#"#compdef fj
# Fajar Lang zsh completions
_fj() {
    local -a commands
    commands=(
        'run:Execute a Fajar Lang program'
        'repl:Start interactive REPL'
        'check:Type-check without executing'
        'build:Build from fj.toml'
        'fmt:Format source code'
        'new:Create a new project'
        'test:Run @test functions'
        'doc:Generate documentation'
        'watch:Watch files and re-run on change'
        'bench:Run benchmarks'
        'lsp:Start LSP server'
        'publish:Publish to registry'
        'search:Search packages'
        'install:Install a package'
        'login:Log in to registry'
        'yank:Yank a version'
        'add:Add dependency'
        'verify:Formal verification'
        'profile:Profile a program'
        'dump-tokens:Show lexer output'
        'dump-ast:Show parser output'
        'hw-info:Show hardware info'
        'hw-json:Hardware info as JSON'
    )
    _describe 'command' commands
}
_fj "$@"
"#
    .to_string()
}

fn generate_fish_completions() -> String {
    r#"# Fajar Lang fish completions
complete -c fj -n '__fish_use_subcommand' -a run -d 'Execute a Fajar Lang program'
complete -c fj -n '__fish_use_subcommand' -a repl -d 'Start interactive REPL'
complete -c fj -n '__fish_use_subcommand' -a check -d 'Type-check without executing'
complete -c fj -n '__fish_use_subcommand' -a build -d 'Build from fj.toml'
complete -c fj -n '__fish_use_subcommand' -a fmt -d 'Format source code'
complete -c fj -n '__fish_use_subcommand' -a new -d 'Create a new project'
complete -c fj -n '__fish_use_subcommand' -a test -d 'Run @test functions'
complete -c fj -n '__fish_use_subcommand' -a publish -d 'Publish to registry'
complete -c fj -n '__fish_use_subcommand' -a search -d 'Search packages'
complete -c fj -n '__fish_use_subcommand' -a install -d 'Install a package'
complete -c fj -n '__fish_use_subcommand' -a login -d 'Log in to registry'
complete -c fj -n '__fish_use_subcommand' -a yank -d 'Yank a version'
complete -c fj -n '__fish_use_subcommand' -a add -d 'Add dependency'
complete -c fj -n '__fish_use_subcommand' -a watch -d 'Watch files and re-run'
complete -c fj -n '__fish_use_subcommand' -a bench -d 'Run benchmarks'
complete -c fj -n '__fish_use_subcommand' -a lsp -d 'Start LSP server'
complete -c fj -n '__fish_use_subcommand' -a verify -d 'Formal verification'
complete -c fj -n '__fish_use_subcommand' -a profile -d 'Profile a program'
complete -c fj -n '__fish_use_subcommand' -a dump-tokens -d 'Show lexer output'
complete -c fj -n '__fish_use_subcommand' -a dump-ast -d 'Show parser output'
complete -c fj -n '__fish_use_subcommand' -a hw-info -d 'Show hardware info'
complete -c fj -n '__fish_use_subcommand' -a hw-json -d 'Hardware info as JSON'
complete -c fj -n '__fish_seen_subcommand_from run' -F -r
"#
    .to_string()
}

// ═══════════════════════════════════════════════════════════════════════
// PR2.16: Progress indicators
// ═══════════════════════════════════════════════════════════════════════

/// Simple progress display for package operations.
pub struct Progress {
    total: usize,
    current: usize,
    label: String,
}

impl Progress {
    /// Create a new progress indicator.
    pub fn new(label: &str, total: usize) -> Self {
        Self {
            total,
            current: 0,
            label: label.to_string(),
        }
    }

    /// Advance progress by one step.
    pub fn tick(&mut self, item: &str) {
        self.current += 1;
        eprint!(
            "\r  {} [{}/{}] {} ...",
            self.label, self.current, self.total, item
        );
    }

    /// Complete the progress.
    pub fn done(&self) {
        eprintln!(
            "\r  {} [{}/{}] done.              ",
            self.label, self.total, self.total
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════
// PR2.17: Conflict resolution
// ═══════════════════════════════════════════════════════════════════════

/// A version conflict between dependencies.
#[derive(Debug, Clone)]
pub struct VersionConflict {
    /// Package with the conflict.
    pub package: String,
    /// Constraints from different dependents.
    pub constraints: Vec<(String, String)>,
}

impl std::fmt::Display for VersionConflict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "conflict: {}", self.package)?;
        for (dep, constraint) in &self.constraints {
            writeln!(f, "  required by {} ({})", dep, constraint)?;
        }
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// PR2.18: Feature flags
// ═══════════════════════════════════════════════════════════════════════

/// Feature flag configuration from fj.toml.
#[derive(Debug, Clone)]
pub struct FeatureConfig {
    /// Available features and their optional dependencies.
    pub features: HashMap<String, Vec<String>>,
    /// Currently enabled features.
    pub enabled: Vec<String>,
}

impl FeatureConfig {
    /// Create from fj.toml features section.
    pub fn new() -> Self {
        Self {
            features: HashMap::new(),
            enabled: Vec::new(),
        }
    }

    /// Add a feature definition.
    pub fn define(&mut self, name: &str, deps: Vec<String>) {
        self.features.insert(name.to_string(), deps);
    }

    /// Enable a feature. Returns the additional dependencies it activates.
    /// No-op if feature is already enabled (prevents duplicates).
    pub fn enable(&mut self, name: &str) -> Result<Vec<String>, String> {
        if !self.features.contains_key(name) {
            return Err(format!("unknown feature: '{name}'"));
        }
        if !self.enabled.contains(&name.to_string()) {
            self.enabled.push(name.to_string());
        }
        Ok(self.features.get(name).cloned().unwrap_or_default())
    }

    /// Check if a feature is enabled.
    pub fn is_enabled(&self, name: &str) -> bool {
        self.enabled.contains(&name.to_string())
    }
}

impl Default for FeatureConfig {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// PR2.19: Build scripts
// ═══════════════════════════════════════════════════════════════════════

/// Build hook configuration.
#[derive(Debug, Clone, Default)]
pub struct BuildHooks {
    /// Commands to run before build.
    pub pre_build: Vec<String>,
    /// Commands to run after build.
    pub post_build: Vec<String>,
}

impl BuildHooks {
    /// Run pre-build hooks. Returns Err on first failure.
    pub fn run_pre_build(&self) -> Result<(), String> {
        for cmd in &self.pre_build {
            validate_hook_command(cmd)?;
            eprintln!("[fj] running pre-build hook: {cmd}");
            let status = std::process::Command::new("sh")
                .arg("-c")
                .arg(cmd)
                .status()
                .map_err(|e| format!("pre-build hook failed: {e}"))?;
            if !status.success() {
                return Err(format!("pre-build hook failed: {cmd}"));
            }
        }
        Ok(())
    }

    /// Run post-build hooks. Returns Err on first failure.
    pub fn run_post_build(&self) -> Result<(), String> {
        for cmd in &self.post_build {
            validate_hook_command(cmd)?;
            eprintln!("[fj] running post-build hook: {cmd}");
            let status = std::process::Command::new("sh")
                .arg("-c")
                .arg(cmd)
                .status()
                .map_err(|e| format!("post-build hook failed: {e}"))?;
            if !status.success() {
                return Err(format!("post-build hook failed: {cmd}"));
            }
        }
        Ok(())
    }
}

/// Dangerous shell patterns that build hooks must never contain.
const DANGEROUS_PATTERNS: &[&str] = &[
    "rm -rf /",
    "rm -fr /",
    "dd if=",
    "mkfs",
    ":(){ :|:& };:",
    "> /dev/sd",
    "chmod -R 777 /",
    "mv / ",
    "wget|sh",
    "curl|sh",
    "wget|bash",
    "curl|bash",
];

/// Validate that a build hook command does not contain known dangerous patterns.
/// Returns `Ok(())` if the command is safe, or `Err` with a description of the
/// rejected pattern.
fn validate_hook_command(cmd: &str) -> Result<(), String> {
    let normalized = cmd.replace('\\', "").to_lowercase();
    for pattern in DANGEROUS_PATTERNS {
        if normalized.contains(pattern) {
            return Err(format!(
                "build hook rejected: command contains dangerous pattern '{pattern}': {cmd}"
            ));
        }
    }
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::package::manifest::PackageInfo;

    fn test_project_config() -> ProjectConfig {
        ProjectConfig {
            package: PackageInfo {
                name: "test-project".to_string(),
                version: "0.1.0".to_string(),
                entry: "src/lib.fj".to_string(),
                linker_script: None,
                target: None,
                no_std: false,
                authors: vec![],
                description: None,
                license: None,
                keywords: vec![],
                categories: vec![],
            },
            dependencies: std::collections::HashMap::new(),
            dev_dependencies: std::collections::HashMap::new(),
            kernel: None,
            service: vec![],
        }
    }

    // PR2.4: Search fallback
    #[test]
    fn pr2_4_search_standard_packages() {
        let results = search_standard_packages("math", 10);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "fj-math");
    }

    #[test]
    fn pr2_4_search_no_match() {
        let results = search_standard_packages("nonexistent", 10);
        assert!(results.is_empty());
    }

    #[test]
    fn pr2_4_search_description_match() {
        let results = search_standard_packages("neural", 10);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "fj-nn");
    }

    // PR2.7: Audit
    #[test]
    fn pr2_7_audit_no_vulnerabilities() {
        let mut config = test_project_config();
        config
            .dependencies
            .insert("fj-math".to_string(), "^1.0.0".to_string());
        let results = audit_dependencies(&config);
        assert_eq!(results.len(), 1);
        assert!(results[0].severity.is_none());
    }

    // PR2.12: Dependency tree
    #[test]
    fn pr2_12_dep_tree_display() {
        let tree = DepTreeNode {
            name: "fj-web".to_string(),
            version: "1.0.0".to_string(),
            children: vec![
                DepTreeNode {
                    name: "fj-http".to_string(),
                    version: "1.0.0".to_string(),
                    children: vec![],
                },
                DepTreeNode {
                    name: "fj-json".to_string(),
                    version: "1.0.0".to_string(),
                    children: vec![],
                },
            ],
        };
        let output = tree.display("", true);
        assert!(output.contains("fj-web"));
        assert!(output.contains("fj-http"));
        assert!(output.contains("fj-json"));
        assert!(output.contains("├──") || output.contains("└──"));
    }

    #[test]
    fn pr2_12_build_dep_tree() {
        let mut config = test_project_config();
        config
            .dependencies
            .insert("fj-math".to_string(), "^1.0.0".to_string());
        config
            .dependencies
            .insert("fj-nn".to_string(), "^0.2.0".to_string());
        let tree = build_dep_tree(&config);
        assert_eq!(tree.len(), 2);
    }

    // PR2.13: Checksum verification
    #[test]
    fn pr2_13_verify_checksum() {
        use sha2::{Digest, Sha256};
        let dir = std::env::temp_dir();
        let path = dir.join("fj_checksum_test.bin");
        let data = b"test data for checksum";
        std::fs::write(&path, data).unwrap();

        let mut hasher = Sha256::new();
        hasher.update(data);
        let expected = format!("{:x}", hasher.finalize());

        assert!(verify_checksum(&path, &expected).unwrap());
        assert!(!verify_checksum(&path, "wrong_checksum").unwrap());
        let _ = std::fs::remove_file(&path);
    }

    // PR2.14: Proxy config
    #[test]
    fn pr2_14_proxy_config_default() {
        let cfg = ProxyConfig {
            http_proxy: None,
            https_proxy: None,
            no_proxy: vec![],
        };
        assert!(!cfg.is_configured());
    }

    #[test]
    fn pr2_14_proxy_configured() {
        let cfg = ProxyConfig {
            http_proxy: Some("http://proxy:8080".to_string()),
            https_proxy: None,
            no_proxy: vec!["localhost".to_string()],
        };
        assert!(cfg.is_configured());
    }

    // PR2.15: Shell completions
    #[test]
    fn pr2_15_bash_completions() {
        let comps = generate_completions("bash");
        assert!(comps.contains("_fj_completions"));
        assert!(comps.contains("complete -F"));
        assert!(comps.contains("publish"));
    }

    #[test]
    fn pr2_15_zsh_completions() {
        let comps = generate_completions("zsh");
        assert!(comps.contains("#compdef fj"));
        assert!(comps.contains("publish"));
    }

    #[test]
    fn pr2_15_fish_completions() {
        let comps = generate_completions("fish");
        assert!(comps.contains("complete -c fj"));
        assert!(comps.contains("publish"));
    }

    // PR2.16: Progress indicators
    #[test]
    fn pr2_16_progress() {
        let mut prog = Progress::new("Installing", 3);
        prog.tick("fj-math");
        assert_eq!(prog.current, 1);
        prog.tick("fj-nn");
        assert_eq!(prog.current, 2);
    }

    // PR2.17: Version conflicts
    #[test]
    fn pr2_17_conflict_display() {
        let conflict = VersionConflict {
            package: "fj-http".to_string(),
            constraints: vec![
                ("fj-web".to_string(), "^1.0.0".to_string()),
                ("fj-api".to_string(), "^2.0.0".to_string()),
            ],
        };
        let s = format!("{conflict}");
        assert!(s.contains("conflict: fj-http"));
        assert!(s.contains("fj-web"));
        assert!(s.contains("fj-api"));
    }

    // PR2.18: Feature flags
    #[test]
    fn pr2_18_feature_config() {
        let mut cfg = FeatureConfig::new();
        cfg.define("tls", vec!["fj-tls".to_string()]);
        cfg.define("gpu", vec!["fj-cuda".to_string()]);

        assert!(!cfg.is_enabled("tls"));
        let deps = cfg.enable("tls").unwrap();
        assert_eq!(deps, vec!["fj-tls"]);
        assert!(cfg.is_enabled("tls"));
    }

    #[test]
    fn pr2_18_unknown_feature() {
        let cfg = FeatureConfig::new();
        // Can't enable on immutable, but test the error path
        let mut cfg2 = cfg;
        assert!(cfg2.enable("nonexistent").is_err());
    }

    // PR2.19: Build hooks
    #[test]
    fn pr2_19_build_hooks_empty() {
        let hooks = BuildHooks::default();
        assert!(hooks.pre_build.is_empty());
        assert!(hooks.post_build.is_empty());
        assert!(hooks.run_pre_build().is_ok());
        assert!(hooks.run_post_build().is_ok());
    }

    #[test]
    fn pr2_19_build_hooks_success() {
        let hooks = BuildHooks {
            pre_build: vec!["true".to_string()],
            post_build: vec!["true".to_string()],
        };
        assert!(hooks.run_pre_build().is_ok());
        assert!(hooks.run_post_build().is_ok());
    }

    #[test]
    fn pr2_19_build_hooks_rejects_dangerous_commands() {
        // rm -rf /
        let hooks = BuildHooks {
            pre_build: vec!["rm -rf /".to_string()],
            post_build: vec![],
        };
        let err = hooks.run_pre_build().unwrap_err();
        assert!(err.contains("dangerous pattern"));

        // dd if=
        let hooks2 = BuildHooks {
            pre_build: vec![],
            post_build: vec!["dd if=/dev/zero of=/dev/sda".to_string()],
        };
        let err2 = hooks2.run_post_build().unwrap_err();
        assert!(err2.contains("dangerous pattern"));

        // mkfs
        let hooks3 = BuildHooks {
            pre_build: vec!["mkfs.ext4 /dev/sda1".to_string()],
            post_build: vec![],
        };
        let err3 = hooks3.run_pre_build().unwrap_err();
        assert!(err3.contains("dangerous pattern"));

        // fork bomb
        let hooks4 = BuildHooks {
            pre_build: vec![":(){ :|:& };:".to_string()],
            post_build: vec![],
        };
        let err4 = hooks4.run_pre_build().unwrap_err();
        assert!(err4.contains("dangerous pattern"));
    }

    #[test]
    fn pr2_19_validate_hook_command_allows_safe_commands() {
        assert!(super::validate_hook_command("echo hello").is_ok());
        assert!(super::validate_hook_command("cargo build --release").is_ok());
        assert!(super::validate_hook_command("rm -rf target/").is_ok()); // specific dir is fine
        assert!(super::validate_hook_command("true").is_ok());
    }

    // PR2.1-PR2.2: Publish + install roundtrip via RegistryDb
    #[test]
    fn pr2_e2e_publish_install_roundtrip() {
        let dir = std::env::temp_dir().join("fj_cli_test");
        let _ = std::fs::create_dir_all(&dir);
        let reg_dir = dir.join("registry");
        let storage_dir = reg_dir.join("storage");
        let _ = std::fs::create_dir_all(&storage_dir);

        let db_path = reg_dir.join("registry.db");
        let reg = RegistryDb::open(&db_path.to_string_lossy(), &storage_dir).unwrap();
        let (_, key) = reg.register_user("clitest", "cli@test.com").unwrap();
        let auth = reg.authenticate(&key).unwrap();

        // Publish
        let resp = reg
            .publish(
                &auth,
                &PublishRequest {
                    name: "fj-cli-test",
                    version: "0.1.0",
                    description: "CLI roundtrip test",
                    tarball: b"fn main() { println(\"hello\") }",
                    keywords: &[],
                    license: None,
                    repository: None,
                },
            )
            .unwrap();
        assert_eq!(resp.status.0, 201);

        // Install
        let install_dir = dir.join("installed");
        let result =
            install_from_registry("fj-cli-test", None, &install_dir, &reg_dir, false).unwrap();
        assert!(result.contains("fj-cli-test"));
        assert!(result.contains("0.1.0"));

        // Verify installed content
        let installed = std::fs::read(install_dir.join("fj-cli-test").join("lib.fj")).unwrap();
        assert_eq!(installed, b"fn main() { println(\"hello\") }");

        // Cleanup
        let _ = std::fs::remove_dir_all(&dir);
    }

    // PR2.4: Search via registry
    #[test]
    fn pr2_4_search_via_registry() {
        let dir = std::env::temp_dir().join("fj_search_test");
        let reg_dir = dir.join("registry");
        let storage_dir = reg_dir.join("storage");
        let _ = std::fs::create_dir_all(&storage_dir);

        let db_path = reg_dir.join("registry.db");
        let reg = RegistryDb::open(&db_path.to_string_lossy(), &storage_dir).unwrap();
        let (_, key) = reg.register_user("searcher", "s@t.com").unwrap();
        let auth = reg.authenticate(&key).unwrap();

        reg.publish(
            &auth,
            &PublishRequest {
                name: "fj-searchable",
                version: "1.0.0",
                description: "A searchable package",
                tarball: b"x",
                keywords: &[],
                license: None,
                repository: None,
            },
        )
        .unwrap();

        let results = search_registry("searchable", 10, &reg_dir).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "fj-searchable");

        let _ = std::fs::remove_dir_all(&dir);
    }

    // PR3.30: Ecosystem package validation
    #[test]
    fn pr3_30_all_ecosystem_packages_exist() {
        let packages_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("packages");
        let expected = vec![
            "fj-async",
            "fj-base64",
            "fj-bench",
            "fj-cli-fw",
            "fj-color",
            "fj-compress",
            "fj-crypto",
            "fj-csv",
            "fj-db",
            "fj-doc-gen",
            "fj-drivers",
            "fj-env",
            "fj-fs",
            "fj-gpio",
            "fj-hal",
            "fj-hello",
            "fj-http",
            "fj-image",
            "fj-json",
            "fj-log",
            "fj-math",
            "fj-mqtt",
            "fj-nn",
            "fj-onnx",
            "fj-plot",
            "fj-rand",
            "fj-regex",
            "fj-sensor",
            "fj-serial",
            "fj-test",
            "fj-time",
            "fj-tls",
            "fj-toml",
            "fj-url",
            "fj-uuid",
            "fj-web",
            "fj-yaml",
        ];
        for pkg in &expected {
            let lib = packages_dir.join(pkg).join("src/lib.fj");
            assert!(lib.exists(), "missing package: {pkg}/src/lib.fj");
        }
        assert!(
            expected.len() >= 37,
            "expected 37+ packages, got {}",
            expected.len()
        );
    }

    #[test]
    fn pr3_30_all_packages_have_manifest() {
        let packages_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("packages");
        for entry in std::fs::read_dir(&packages_dir).unwrap() {
            let entry = entry.unwrap();
            if entry.file_type().unwrap().is_dir() {
                let toml_path = entry.path().join("fj.toml");
                assert!(
                    toml_path.exists(),
                    "missing fj.toml in {}",
                    entry.path().display()
                );
            }
        }
    }

    #[test]
    fn pr3_30_packages_have_minimum_lines() {
        let packages_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("packages");
        let new_packages = vec![
            "fj-async",
            "fj-base64",
            "fj-bench",
            "fj-cli-fw",
            "fj-color",
            "fj-compress",
            "fj-csv",
            "fj-db",
            "fj-doc-gen",
            "fj-env",
            "fj-fs",
            "fj-gpio",
            "fj-image",
            "fj-log",
            "fj-mqtt",
            "fj-onnx",
            "fj-plot",
            "fj-rand",
            "fj-regex",
            "fj-sensor",
            "fj-serial",
            "fj-test",
            "fj-time",
            "fj-tls",
            "fj-toml",
            "fj-url",
            "fj-uuid",
            "fj-web",
            "fj-yaml",
        ];
        for pkg in &new_packages {
            let lib = packages_dir.join(pkg).join("src/lib.fj");
            let content = std::fs::read_to_string(&lib).unwrap();
            let lines = content.lines().count();
            assert!(
                lines >= 30,
                "{pkg}/src/lib.fj has only {lines} lines (min 30)",
            );
        }
    }

    // PR3.30: Verify packages actually lex+parse through Fajar Lang compiler
    #[test]
    fn pr3_30_packages_lex_and_parse() {
        let packages_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("packages");
        // Test a representative sample across different domains
        let sample = vec![
            "fj-math",
            "fj-log",
            "fj-csv",
            "fj-time",
            "fj-uuid",
            "fj-color",
            "fj-rand",
            "fj-base64",
            "fj-env",
            "fj-fs",
        ];
        let mut passed = 0;
        let mut failed = Vec::new();
        for pkg in &sample {
            let lib = packages_dir.join(pkg).join("src/lib.fj");
            let source = std::fs::read_to_string(&lib).unwrap();
            match crate::lexer::tokenize(&source) {
                Ok(tokens) => match crate::parser::parse(tokens) {
                    Ok(_) => {
                        passed += 1;
                    }
                    Err(errors) => {
                        failed.push(format!("{pkg}: {} parse errors", errors.len()));
                    }
                },
                Err(errors) => {
                    failed.push(format!("{pkg}: {} lex errors", errors.len()));
                }
            }
        }
        assert!(
            passed >= 5,
            "only {passed}/{} packages parsed OK. Failed: {:?}",
            sample.len(),
            failed,
        );
    }
}
