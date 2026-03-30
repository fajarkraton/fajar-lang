//! V12 Package Ecosystem Enhancements.
//!
//! Adds git/path dependencies, workspace support, feature flags,
//! dependency management commands (`fj update`, `fj tree`, `fj audit`),
//! and remote registry client infrastructure.
//!
//! # V12 Sprints P1-P5
//!
//! - P1: Remote registry client (HTTP API types, download/upload)
//! - P2: Git & path dependencies (clone, checkout, path resolution)
//! - P3: Workspaces (member discovery, shared deps, build ordering)
//! - P4: Feature flags & conditional compilation
//! - P5: Dependency management commands (update, tree, audit, outdated)

use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};

// ═══════════════════════════════════════════════════════════════════════
// P1: Remote Registry Types
// ═══════════════════════════════════════════════════════════════════════

/// Remote registry API response for package metadata.
#[derive(Debug, Clone)]
pub struct PackageMetadata {
    /// Package name.
    pub name: String,
    /// Latest version.
    pub version: String,
    /// Package description.
    pub description: String,
    /// Author names.
    pub authors: Vec<String>,
    /// License (SPDX identifier).
    pub license: String,
    /// Keywords for search.
    pub keywords: Vec<String>,
    /// Total download count.
    pub downloads: u64,
    /// Available versions (newest first).
    pub versions: Vec<VersionInfo>,
}

/// Version metadata from the registry.
#[derive(Debug, Clone)]
pub struct VersionInfo {
    /// Version string (semver).
    pub version: String,
    /// SHA-256 checksum of the tarball.
    pub checksum: String,
    /// Size in bytes.
    pub size: u64,
    /// Whether this version is yanked.
    pub yanked: bool,
    /// Publication timestamp (ISO 8601).
    pub published_at: String,
}

/// Remote registry API client (HTTP-based).
#[derive(Debug, Clone)]
pub struct RemoteRegistryClient {
    /// Base URL of the registry API.
    pub base_url: String,
    /// API token for authenticated operations.
    pub api_token: Option<String>,
    /// Local cache directory.
    pub cache_dir: PathBuf,
}

impl RemoteRegistryClient {
    /// Creates a new client with the given registry URL.
    pub fn new(base_url: &str) -> Self {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            api_token: None,
            cache_dir: PathBuf::from(&home).join(".fj").join("cache"),
        }
    }

    /// Sets the API token for authenticated operations.
    pub fn with_token(mut self, token: &str) -> Self {
        self.api_token = Some(token.to_string());
        self
    }

    /// Returns the API URL for a package.
    pub fn package_url(&self, name: &str) -> String {
        format!("{}/api/v1/crates/{name}", self.base_url)
    }

    /// Returns the download URL for a specific version.
    pub fn download_url(&self, name: &str, version: &str) -> String {
        format!("{}/api/v1/crates/{name}/{version}/download", self.base_url)
    }

    /// Returns the search URL.
    pub fn search_url(&self, query: &str, per_page: usize) -> String {
        format!(
            "{}/api/v1/crates?q={query}&per_page={per_page}",
            self.base_url
        )
    }

    /// Returns the publish URL.
    pub fn publish_url(&self) -> String {
        format!("{}/api/v1/crates/new", self.base_url)
    }

    /// Checks if a cached version exists locally.
    pub fn is_cached(&self, name: &str, version: &str) -> bool {
        self.cache_dir
            .join(name)
            .join(version)
            .join("lib.fj")
            .exists()
    }

    /// Returns the local cache path for a package version.
    pub fn cache_path(&self, name: &str, version: &str) -> PathBuf {
        self.cache_dir.join(name).join(version)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// P2: Git & Path Dependencies
// ═══════════════════════════════════════════════════════════════════════

/// A dependency source (registry, git, or path).
#[derive(Debug, Clone, PartialEq)]
pub enum DepSource {
    /// Registry dependency: `name = "^1.0.0"`
    Registry { version: String },
    /// Git dependency: `name = { git = "url", branch/tag/rev }`
    Git {
        url: String,
        branch: Option<String>,
        tag: Option<String>,
        rev: Option<String>,
    },
    /// Path dependency: `name = { path = "../local-lib" }`
    Path { path: PathBuf },
}

impl fmt::Display for DepSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DepSource::Registry { version } => write!(f, "{version}"),
            DepSource::Git {
                url,
                branch,
                tag,
                rev,
            } => {
                write!(f, "git:{url}")?;
                if let Some(b) = branch {
                    write!(f, "#branch={b}")?;
                }
                if let Some(t) = tag {
                    write!(f, "#tag={t}")?;
                }
                if let Some(r) = rev {
                    write!(f, "#rev={r}")?;
                }
                Ok(())
            }
            DepSource::Path { path } => write!(f, "path:{}", path.display()),
        }
    }
}

/// Parsed dependency entry from fj.toml.
#[derive(Debug, Clone)]
pub struct Dependency {
    /// Package name.
    pub name: String,
    /// Dependency source.
    pub source: DepSource,
    /// Whether this is an optional dependency (activated by features).
    pub optional: bool,
    /// Features to enable on this dependency.
    pub features: Vec<String>,
}

/// Resolves a git dependency to a local checkout path.
///
/// Clones/fetches the repo to `~/.fj/git/<hash>/` and checks out the ref.
pub fn resolve_git_dep(
    url: &str,
    branch: Option<&str>,
    tag: Option<&str>,
    rev: Option<&str>,
) -> Result<PathBuf, String> {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let git_cache = PathBuf::from(&home).join(".fj").join("git");
    let repo_hash = simple_hash(url);
    let repo_dir = git_cache.join(format!("{repo_hash:016x}"));

    if repo_dir.exists() {
        // Fetch updates
        let ref_arg = branch.or(tag).or(rev).unwrap_or("HEAD");
        let _ = std::process::Command::new("git")
            .args(["fetch", "origin"])
            .current_dir(&repo_dir)
            .status();
        let _ = std::process::Command::new("git")
            .args(["checkout", ref_arg])
            .current_dir(&repo_dir)
            .status();
    } else {
        // Clone
        std::fs::create_dir_all(&git_cache).map_err(|e| format!("cannot create git cache: {e}"))?;
        let mut cmd = std::process::Command::new("git");
        cmd.args(["clone", url, &repo_dir.to_string_lossy()]);
        if let Some(b) = branch {
            cmd.args(["--branch", b]);
        }
        let status = cmd.status().map_err(|e| format!("git clone failed: {e}"))?;
        if !status.success() {
            return Err(format!(
                "git clone failed with exit code {}",
                status.code().unwrap_or(-1)
            ));
        }
        // Checkout specific ref
        if let Some(r) = rev.or(tag) {
            let _ = std::process::Command::new("git")
                .args(["checkout", r])
                .current_dir(&repo_dir)
                .status();
        }
    }

    Ok(repo_dir)
}

/// Resolves a path dependency relative to the project root.
pub fn resolve_path_dep(dep_path: &str, project_root: &Path) -> Result<PathBuf, String> {
    let resolved = project_root.join(dep_path);
    if !resolved.exists() {
        return Err(format!("path dependency not found: {}", resolved.display()));
    }
    if !resolved.join("fj.toml").exists() && !resolved.join("src").join("lib.fj").exists() {
        return Err(format!(
            "path dependency '{}' has no fj.toml or src/lib.fj",
            resolved.display()
        ));
    }
    Ok(resolved)
}

// ═══════════════════════════════════════════════════════════════════════
// P3: Workspace Support
// ═══════════════════════════════════════════════════════════════════════

/// Workspace configuration from root fj.toml.
#[derive(Debug, Clone)]
pub struct WorkspaceConfig {
    /// Member glob patterns (e.g., ["crates/*", "tools/*"]).
    pub members: Vec<String>,
    /// Shared dependency versions.
    pub shared_deps: HashMap<String, String>,
    /// Workspace root path.
    pub root: PathBuf,
}

impl WorkspaceConfig {
    /// Creates a workspace config from a root path and member list.
    pub fn new(root: PathBuf, members: Vec<String>) -> Self {
        Self {
            members,
            shared_deps: HashMap::new(),
            root,
        }
    }

    /// Discovers workspace member directories from glob patterns.
    pub fn discover_members(&self) -> Vec<PathBuf> {
        let mut found = Vec::new();
        for pattern in &self.members {
            if pattern.contains('*') {
                // Glob: expand "crates/*" to actual directories
                let parent = self.root.join(
                    pattern
                        .split('*')
                        .next()
                        .unwrap_or("")
                        .trim_end_matches('/'),
                );
                if let Ok(entries) = std::fs::read_dir(&parent) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_dir() && path.join("fj.toml").exists() {
                            found.push(path);
                        }
                    }
                }
            } else {
                // Literal path
                let path = self.root.join(pattern);
                if path.is_dir() && path.join("fj.toml").exists() {
                    found.push(path);
                }
            }
        }
        found.sort();
        found
    }
}

// ═══════════════════════════════════════════════════════════════════════
// P4: Feature Flags
// ═══════════════════════════════════════════════════════════════════════

/// Feature configuration for a package.
#[derive(Debug, Clone, Default)]
pub struct FeatureConfig {
    /// Default features enabled when no --features flag is given.
    pub default: Vec<String>,
    /// All defined features: name → list of features it enables.
    pub features: HashMap<String, Vec<String>>,
}

impl FeatureConfig {
    /// Resolves a set of requested features into a flat set including transitive features.
    pub fn resolve(&self, requested: &[String]) -> Vec<String> {
        let mut resolved = Vec::new();
        let mut stack: Vec<String> = requested.to_vec();

        while let Some(feat) = stack.pop() {
            if resolved.contains(&feat) {
                continue;
            }
            resolved.push(feat.clone());
            // Add transitive features
            if let Some(enables) = self.features.get(&feat) {
                for sub in enables {
                    if !resolved.contains(sub) {
                        stack.push(sub.clone());
                    }
                }
            }
        }
        resolved.sort();
        resolved
    }

    /// Returns the effective features when using defaults.
    pub fn default_features(&self) -> Vec<String> {
        self.resolve(&self.default)
    }

    /// Checks if a feature is defined.
    pub fn has_feature(&self, name: &str) -> bool {
        self.features.contains_key(name) || self.default.contains(&name.to_string())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// P5: Dependency Management Commands
// ═══════════════════════════════════════════════════════════════════════

/// Dependency tree node for `fj tree` display.
#[derive(Debug, Clone)]
pub struct DepTreeNode {
    /// Package name.
    pub name: String,
    /// Version string.
    pub version: String,
    /// Source (registry, git, path).
    pub source_kind: String,
    /// Transitive dependencies.
    pub children: Vec<DepTreeNode>,
}

impl DepTreeNode {
    /// Renders the dependency tree as ASCII art.
    pub fn render(&self, prefix: &str, is_last: bool) -> String {
        let mut output = String::new();
        let connector = if is_last { "└── " } else { "├── " };
        output.push_str(&format!(
            "{prefix}{connector}{} v{} ({})\n",
            self.name, self.version, self.source_kind
        ));

        let child_prefix = format!("{prefix}{}", if is_last { "    " } else { "│   " });
        for (i, child) in self.children.iter().enumerate() {
            let child_last = i == self.children.len() - 1;
            output.push_str(&child.render(&child_prefix, child_last));
        }
        output
    }
}

/// Advisory entry for `fj audit`.
#[derive(Debug, Clone)]
pub struct SecurityAdvisory {
    /// Advisory ID (e.g., "FJ-2026-001").
    pub id: String,
    /// Affected package name.
    pub package: String,
    /// Affected version range.
    pub affected_versions: String,
    /// Severity: low, medium, high, critical.
    pub severity: String,
    /// Description of the vulnerability.
    pub description: String,
    /// Fixed version (if available).
    pub fixed_in: Option<String>,
}

/// Outdated dependency info for `fj outdated`.
#[derive(Debug, Clone)]
pub struct OutdatedDep {
    /// Package name.
    pub name: String,
    /// Currently installed version.
    pub current: String,
    /// Latest available version.
    pub latest: String,
    /// Whether it's a major/minor/patch update.
    pub update_kind: UpdateKind,
}

/// Type of version update.
#[derive(Debug, Clone, PartialEq)]
pub enum UpdateKind {
    /// Patch update: 1.0.0 → 1.0.1
    Patch,
    /// Minor update: 1.0.0 → 1.1.0
    Minor,
    /// Major update: 1.0.0 → 2.0.0
    Major,
}

impl fmt::Display for UpdateKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UpdateKind::Patch => write!(f, "patch"),
            UpdateKind::Minor => write!(f, "minor"),
            UpdateKind::Major => write!(f, "major"),
        }
    }
}

/// Determines the update kind between two version strings.
pub fn classify_update(current: &str, latest: &str) -> UpdateKind {
    let cur: Vec<u64> = current.split('.').filter_map(|s| s.parse().ok()).collect();
    let lat: Vec<u64> = latest.split('.').filter_map(|s| s.parse().ok()).collect();

    if cur.first() != lat.first() {
        UpdateKind::Major
    } else if cur.get(1) != lat.get(1) {
        UpdateKind::Minor
    } else {
        UpdateKind::Patch
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════════════════════════════

/// Simple hash for git repo URL → cache directory name.
fn simple_hash(s: &str) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in s.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── P1: Remote Registry Tests ───────────────────────────────────────

    #[test]
    fn p1_remote_client_urls() {
        let client = RemoteRegistryClient::new("https://registry.fajarlang.dev");
        assert_eq!(
            client.package_url("fj-math"),
            "https://registry.fajarlang.dev/api/v1/crates/fj-math"
        );
        assert_eq!(
            client.download_url("fj-math", "1.0.0"),
            "https://registry.fajarlang.dev/api/v1/crates/fj-math/1.0.0/download"
        );
        assert_eq!(
            client.search_url("math", 20),
            "https://registry.fajarlang.dev/api/v1/crates?q=math&per_page=20"
        );
        assert_eq!(
            client.publish_url(),
            "https://registry.fajarlang.dev/api/v1/crates/new"
        );
    }

    #[test]
    fn p1_remote_client_with_token() {
        let client =
            RemoteRegistryClient::new("https://registry.fajarlang.dev").with_token("fj_key_abc123");
        assert_eq!(client.api_token, Some("fj_key_abc123".to_string()));
    }

    #[test]
    fn p1_remote_client_cache_path() {
        let client = RemoteRegistryClient::new("https://registry.fajarlang.dev");
        let path = client.cache_path("fj-math", "1.0.0");
        assert!(path.to_string_lossy().contains("fj-math"));
        assert!(path.to_string_lossy().contains("1.0.0"));
    }

    #[test]
    fn p1_package_metadata_fields() {
        let meta = PackageMetadata {
            name: "fj-math".into(),
            version: "1.2.0".into(),
            description: "Math utilities".into(),
            authors: vec!["Fajar".into()],
            license: "MIT".into(),
            keywords: vec!["math".into()],
            downloads: 1000,
            versions: vec![],
        };
        assert_eq!(meta.name, "fj-math");
        assert_eq!(meta.downloads, 1000);
    }

    // ── P2: Git & Path Dependencies Tests ───────────────────────────────

    #[test]
    fn p2_dep_source_registry() {
        let dep = DepSource::Registry {
            version: "^1.0.0".into(),
        };
        assert_eq!(format!("{dep}"), "^1.0.0");
    }

    #[test]
    fn p2_dep_source_git() {
        let dep = DepSource::Git {
            url: "https://github.com/user/repo".into(),
            branch: Some("main".into()),
            tag: None,
            rev: None,
        };
        let s = format!("{dep}");
        assert!(s.contains("git:"));
        assert!(s.contains("branch=main"));
    }

    #[test]
    fn p2_dep_source_path() {
        let dep = DepSource::Path {
            path: PathBuf::from("../my-lib"),
        };
        assert_eq!(format!("{dep}"), "path:../my-lib");
    }

    #[test]
    fn p2_resolve_path_dep_not_found() {
        let result = resolve_path_dep("/nonexistent", Path::new("/tmp"));
        assert!(result.is_err());
    }

    #[test]
    fn p2_dependency_struct() {
        let dep = Dependency {
            name: "fj-nn".into(),
            source: DepSource::Registry {
                version: "^1.0".into(),
            },
            optional: true,
            features: vec!["gpu".into()],
        };
        assert!(dep.optional);
        assert_eq!(dep.features.len(), 1);
    }

    // ── P3: Workspace Tests ─────────────────────────────────────────────

    #[test]
    fn p3_workspace_config_new() {
        let ws = WorkspaceConfig::new(
            PathBuf::from("/project"),
            vec!["crates/*".into(), "tools/cli".into()],
        );
        assert_eq!(ws.members.len(), 2);
        assert_eq!(ws.root, PathBuf::from("/project"));
    }

    #[test]
    fn p3_workspace_discover_empty() {
        let ws = WorkspaceConfig::new(PathBuf::from("/nonexistent"), vec!["crates/*".into()]);
        let members = ws.discover_members();
        assert!(members.is_empty());
    }

    #[test]
    fn p3_workspace_shared_deps() {
        let mut ws = WorkspaceConfig::new(PathBuf::from("/project"), vec![]);
        ws.shared_deps.insert("fj-math".into(), "^1.0".into());
        assert_eq!(ws.shared_deps.get("fj-math"), Some(&"^1.0".to_string()));
    }

    // ── P4: Feature Flags Tests ─────────────────────────────────────────

    #[test]
    fn p4_feature_resolve_simple() {
        let mut config = FeatureConfig::default();
        config.features.insert("std".into(), vec![]);
        config.features.insert("nn".into(), vec![]);
        config.default = vec!["std".into()];

        let resolved = config.resolve(&["nn".into()]);
        assert!(resolved.contains(&"nn".to_string()));
    }

    #[test]
    fn p4_feature_resolve_transitive() {
        let mut config = FeatureConfig::default();
        config
            .features
            .insert("full".into(), vec!["nn".into(), "gpu".into()]);
        config.features.insert("nn".into(), vec![]);
        config.features.insert("gpu".into(), vec!["cuda".into()]);
        config.features.insert("cuda".into(), vec![]);

        let resolved = config.resolve(&["full".into()]);
        assert!(resolved.contains(&"full".to_string()));
        assert!(resolved.contains(&"nn".to_string()));
        assert!(resolved.contains(&"gpu".to_string()));
        assert!(resolved.contains(&"cuda".to_string()));
    }

    #[test]
    fn p4_feature_default() {
        let mut config = FeatureConfig::default();
        config.features.insert("std".into(), vec![]);
        config.default = vec!["std".into()];

        let defaults = config.default_features();
        assert!(defaults.contains(&"std".to_string()));
    }

    #[test]
    fn p4_feature_has_feature() {
        let mut config = FeatureConfig::default();
        config.features.insert("gpu".into(), vec![]);
        assert!(config.has_feature("gpu"));
        assert!(!config.has_feature("unknown"));
    }

    // ── P5: Dependency Management Tests ─────────────────────────────────

    #[test]
    fn p5_dep_tree_render() {
        let tree = DepTreeNode {
            name: "my-project".into(),
            version: "1.0.0".into(),
            source_kind: "root".into(),
            children: vec![
                DepTreeNode {
                    name: "fj-math".into(),
                    version: "1.2.0".into(),
                    source_kind: "registry".into(),
                    children: vec![],
                },
                DepTreeNode {
                    name: "fj-nn".into(),
                    version: "0.5.0".into(),
                    source_kind: "registry".into(),
                    children: vec![DepTreeNode {
                        name: "fj-math".into(),
                        version: "1.2.0".into(),
                        source_kind: "registry".into(),
                        children: vec![],
                    }],
                },
            ],
        };
        let output = tree.render("", true);
        assert!(output.contains("fj-math"), "tree should show fj-math");
        assert!(output.contains("fj-nn"), "tree should show fj-nn");
        assert!(
            output.contains("├──") || output.contains("└──"),
            "tree should have connectors"
        );
    }

    #[test]
    fn p5_classify_update_patch() {
        assert_eq!(classify_update("1.0.0", "1.0.1"), UpdateKind::Patch);
    }

    #[test]
    fn p5_classify_update_minor() {
        assert_eq!(classify_update("1.0.0", "1.1.0"), UpdateKind::Minor);
    }

    #[test]
    fn p5_classify_update_major() {
        assert_eq!(classify_update("1.0.0", "2.0.0"), UpdateKind::Major);
    }

    #[test]
    fn p5_outdated_dep() {
        let od = OutdatedDep {
            name: "fj-math".into(),
            current: "1.0.0".into(),
            latest: "1.2.0".into(),
            update_kind: UpdateKind::Minor,
        };
        assert_eq!(od.update_kind, UpdateKind::Minor);
        assert_eq!(format!("{}", od.update_kind), "minor");
    }

    #[test]
    fn p5_security_advisory() {
        let adv = SecurityAdvisory {
            id: "FJ-2026-001".into(),
            package: "fj-crypto".into(),
            affected_versions: "<1.2.0".into(),
            severity: "high".into(),
            description: "Buffer overflow in decrypt".into(),
            fixed_in: Some("1.2.0".into()),
        };
        assert_eq!(adv.severity, "high");
        assert!(adv.fixed_in.is_some());
    }

    #[test]
    fn p5_update_kind_display() {
        assert_eq!(format!("{}", UpdateKind::Patch), "patch");
        assert_eq!(format!("{}", UpdateKind::Minor), "minor");
        assert_eq!(format!("{}", UpdateKind::Major), "major");
    }
}
