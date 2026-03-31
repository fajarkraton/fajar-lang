//! Package manager for FajarOS Nova v2.0 — Sprint N9.
//!
//! Provides a simulated package management system with package index,
//! dependency resolution, install/remove/update operations, repository
//! management, build-from-source, and checksum verification. All data
//! is in-memory — no real file system or network operations.

use std::collections::HashMap;

// ═══════════════════════════════════════════════════════════════════════
// Package Manager Errors
// ═══════════════════════════════════════════════════════════════════════

/// Errors produced by the package manager.
#[derive(Debug, Clone, thiserror::Error)]
pub enum PkgError {
    /// Package not found.
    #[error("package not found: {0}")]
    PackageNotFound(String),
    /// Version not found.
    #[error("version not found: {0} {1}")]
    VersionNotFound(String, String),
    /// Dependency conflict.
    #[error("dependency conflict: {0}")]
    DependencyConflict(String),
    /// Cannot remove — other packages depend on it.
    #[error("cannot remove {0}: required by {1}")]
    RequiredBy(String, String),
    /// Checksum mismatch.
    #[error("checksum mismatch for {0}: expected {1}, got {2}")]
    ChecksumMismatch(String, String, String),
    /// Signature verification failed.
    #[error("signature verification failed for {0}")]
    SignatureInvalid(String),
    /// Repository error.
    #[error("repository error: {0}")]
    RepoError(String),
    /// Build error.
    #[error("build error: {0}")]
    BuildError(String),
    /// Already installed.
    #[error("package already installed: {0} {1}")]
    AlreadyInstalled(String, String),
}

// ═══════════════════════════════════════════════════════════════════════
// Version
// ═══════════════════════════════════════════════════════════════════════

/// Semantic version (major.minor.patch).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Version {
    /// Major version.
    pub major: u32,
    /// Minor version.
    pub minor: u32,
    /// Patch version.
    pub patch: u32,
}

impl Version {
    /// Creates a new version.
    pub fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    /// Parses a version string (e.g. "1.2.3").
    pub fn parse(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.split('.').collect();
        if parts.len() != 3 {
            return None;
        }
        Some(Self {
            major: parts[0].parse().ok()?,
            minor: parts[1].parse().ok()?,
            patch: parts[2].parse().ok()?,
        })
    }

    /// Returns `true` if this version satisfies the given range.
    ///
    /// Supports: exact ("1.2.3"), caret ("^1.2.3" = >=1.2.3 <2.0.0),
    /// tilde ("~1.2.3" = >=1.2.3 <1.3.0), wildcard ("*" = any).
    pub fn satisfies(&self, range: &str) -> bool {
        if range == "*" {
            return true;
        }
        if let Some(caret) = range.strip_prefix('^') {
            if let Some(base) = Version::parse(caret) {
                return self.major == base.major
                    && (self.major > 0 && self >= &base
                        || self.major == 0 && self.minor == base.minor && self.patch >= base.patch);
            }
        }
        if let Some(tilde) = range.strip_prefix('~') {
            if let Some(base) = Version::parse(tilde) {
                return self.major == base.major
                    && self.minor == base.minor
                    && self.patch >= base.patch;
            }
        }
        // Exact match
        if let Some(exact) = Version::parse(range) {
            return *self == exact;
        }
        false
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.major
            .cmp(&other.major)
            .then(self.minor.cmp(&other.minor))
            .then(self.patch.cmp(&other.patch))
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Package Metadata
// ═══════════════════════════════════════════════════════════════════════

/// A dependency declaration.
#[derive(Debug, Clone)]
pub struct Dependency {
    /// Package name.
    pub name: String,
    /// Version range (e.g. "^1.0.0", "~2.1.0", "*").
    pub version_range: String,
}

/// Package metadata.
#[derive(Debug, Clone)]
pub struct PackageMeta {
    /// Package name.
    pub name: String,
    /// Version.
    pub version: Version,
    /// Short description.
    pub description: String,
    /// Author.
    pub author: String,
    /// Dependencies.
    pub dependencies: Vec<Dependency>,
    /// Tags for search.
    pub tags: Vec<String>,
    /// SHA-256 checksum (hex string).
    pub checksum: String,
    /// Signature (hex string, empty if unsigned).
    pub signature: String,
    /// Installed size in bytes.
    pub size: u64,
}

// ═══════════════════════════════════════════════════════════════════════
// Package Index
// ═══════════════════════════════════════════════════════════════════════

/// Package registry — holds all known packages with their versions.
#[derive(Debug)]
pub struct PackageIndex {
    /// Packages keyed by name, each with a list of versions.
    packages: HashMap<String, Vec<PackageMeta>>,
}

impl PackageIndex {
    /// Creates a new empty index.
    pub fn new() -> Self {
        Self {
            packages: HashMap::new(),
        }
    }

    /// Creates an index with pre-populated standard packages.
    pub fn with_defaults() -> Self {
        let mut index = Self::new();
        index.add(PackageMeta {
            name: "fj-core".to_string(),
            version: Version::new(2, 0, 0),
            description: "FajarOS core libraries".to_string(),
            author: "FajarOS Team".to_string(),
            dependencies: vec![],
            tags: vec!["core".to_string(), "system".to_string()],
            checksum: "a1b2c3d4".to_string(),
            signature: "sig-a1b2".to_string(),
            size: 1_048_576,
        });
        index.add(PackageMeta {
            name: "fj-net".to_string(),
            version: Version::new(1, 2, 0),
            description: "FajarOS networking library".to_string(),
            author: "FajarOS Team".to_string(),
            dependencies: vec![Dependency {
                name: "fj-core".to_string(),
                version_range: "^2.0.0".to_string(),
            }],
            tags: vec!["network".to_string(), "tcp".to_string(), "udp".to_string()],
            checksum: "e5f6a7b8".to_string(),
            signature: "sig-e5f6".to_string(),
            size: 524_288,
        });
        index.add(PackageMeta {
            name: "fj-gui".to_string(),
            version: Version::new(1, 0, 0),
            description: "FajarOS GUI toolkit".to_string(),
            author: "FajarOS Team".to_string(),
            dependencies: vec![Dependency {
                name: "fj-core".to_string(),
                version_range: "^2.0.0".to_string(),
            }],
            tags: vec!["gui".to_string(), "window".to_string(), "widget".to_string()],
            checksum: "c9d0e1f2".to_string(),
            signature: "sig-c9d0".to_string(),
            size: 786_432,
        });
        index.add(PackageMeta {
            name: "fj-ml".to_string(),
            version: Version::new(0, 5, 0),
            description: "Machine learning for FajarOS".to_string(),
            author: "FajarOS Team".to_string(),
            dependencies: vec![
                Dependency {
                    name: "fj-core".to_string(),
                    version_range: "^2.0.0".to_string(),
                },
            ],
            tags: vec!["ml".to_string(), "tensor".to_string(), "ai".to_string()],
            checksum: "abcdef12".to_string(),
            signature: "sig-abcd".to_string(),
            size: 2_097_152,
        });
        index
    }

    /// Adds a package to the index.
    pub fn add(&mut self, pkg: PackageMeta) {
        self.packages
            .entry(pkg.name.clone())
            .or_default()
            .push(pkg);
    }

    /// Looks up all versions of a package.
    pub fn lookup(&self, name: &str) -> Option<&Vec<PackageMeta>> {
        self.packages.get(name)
    }

    /// Looks up the latest version of a package.
    pub fn latest(&self, name: &str) -> Option<&PackageMeta> {
        self.packages
            .get(name)?
            .iter()
            .max_by(|a, b| a.version.cmp(&b.version))
    }

    /// Finds a specific version.
    pub fn find(&self, name: &str, version: &Version) -> Option<&PackageMeta> {
        self.packages
            .get(name)?
            .iter()
            .find(|p| p.version == *version)
    }

    /// Returns the total number of packages (unique names).
    pub fn package_count(&self) -> usize {
        self.packages.len()
    }
}

impl Default for PackageIndex {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Package Resolver (Dependency Resolution)
// ═══════════════════════════════════════════════════════════════════════

/// Resolved package with its selected version.
#[derive(Debug, Clone)]
pub struct ResolvedPackage {
    /// Package name.
    pub name: String,
    /// Selected version.
    pub version: Version,
}

/// Dependency resolver — resolves a package and all its transitive dependencies.
#[derive(Debug)]
pub struct PackageResolver {
    /// Maximum resolution depth (to prevent infinite loops).
    pub max_depth: u32,
}

impl PackageResolver {
    /// Creates a new resolver.
    pub fn new() -> Self {
        Self { max_depth: 50 }
    }

    /// Resolves all dependencies for a package.
    ///
    /// Returns a flat list of packages in install order (dependencies first).
    pub fn resolve(
        &self,
        name: &str,
        index: &PackageIndex,
    ) -> Result<Vec<ResolvedPackage>, PkgError> {
        let mut resolved: Vec<ResolvedPackage> = Vec::new();
        let mut visited: Vec<String> = Vec::new();
        self.resolve_recursive(name, index, &mut resolved, &mut visited, 0)?;
        Ok(resolved)
    }

    /// Recursive dependency resolution.
    fn resolve_recursive(
        &self,
        name: &str,
        index: &PackageIndex,
        resolved: &mut Vec<ResolvedPackage>,
        visited: &mut Vec<String>,
        depth: u32,
    ) -> Result<(), PkgError> {
        if depth > self.max_depth {
            return Err(PkgError::DependencyConflict(format!(
                "resolution depth exceeded for {}",
                name
            )));
        }

        if visited.contains(&name.to_string()) {
            return Ok(());
        }
        visited.push(name.to_string());

        let pkg = index
            .latest(name)
            .ok_or_else(|| PkgError::PackageNotFound(name.to_string()))?;

        // Resolve dependencies first
        for dep in &pkg.dependencies {
            self.resolve_recursive(&dep.name, index, resolved, visited, depth + 1)?;
        }

        // Add this package
        if !resolved.iter().any(|r| r.name == name) {
            resolved.push(ResolvedPackage {
                name: name.to_string(),
                version: pkg.version.clone(),
            });
        }

        Ok(())
    }
}

impl Default for PackageResolver {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Package Installer
// ═══════════════════════════════════════════════════════════════════════

/// An installed package record.
#[derive(Debug, Clone)]
pub struct InstalledPackage {
    /// Package metadata.
    pub meta: PackageMeta,
    /// Install timestamp.
    pub installed_at: u64,
    /// Install path.
    pub install_path: String,
}

/// Package installer — download (simulated), extract, install.
#[derive(Debug)]
pub struct PackageInstaller {
    /// Installed packages.
    installed: HashMap<String, InstalledPackage>,
    /// Install base directory.
    pub install_dir: String,
}

impl PackageInstaller {
    /// Creates a new installer.
    pub fn new(install_dir: &str) -> Self {
        Self {
            installed: HashMap::new(),
            install_dir: install_dir.to_string(),
        }
    }

    /// Installs a package.
    pub fn install(&mut self, meta: PackageMeta, now: u64) -> Result<(), PkgError> {
        if let Some(existing) = self.installed.get(&meta.name) {
            if existing.meta.version == meta.version {
                return Err(PkgError::AlreadyInstalled(
                    meta.name.clone(),
                    meta.version.to_string(),
                ));
            }
        }

        let path = format!("{}/{}-{}", self.install_dir, meta.name, meta.version);
        self.installed.insert(
            meta.name.clone(),
            InstalledPackage {
                meta,
                installed_at: now,
                install_path: path,
            },
        );
        Ok(())
    }

    /// Returns `true` if a package is installed.
    pub fn is_installed(&self, name: &str) -> bool {
        self.installed.contains_key(name)
    }

    /// Gets installed package info.
    pub fn get_installed(&self, name: &str) -> Option<&InstalledPackage> {
        self.installed.get(name)
    }

    /// Returns all installed packages.
    pub fn list_installed(&self) -> Vec<&InstalledPackage> {
        self.installed.values().collect()
    }

    /// Returns the count of installed packages.
    pub fn installed_count(&self) -> usize {
        self.installed.len()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Package Remover
// ═══════════════════════════════════════════════════════════════════════

/// Package remover — uninstalls with dependency checking.
#[derive(Debug)]
pub struct PackageRemover;

impl PackageRemover {
    /// Creates a new remover.
    pub fn new() -> Self {
        Self
    }

    /// Removes a package, checking that no other installed packages depend on it.
    pub fn remove(
        &self,
        name: &str,
        installer: &mut PackageInstaller,
    ) -> Result<(), PkgError> {
        if !installer.is_installed(name) {
            return Err(PkgError::PackageNotFound(name.to_string()));
        }

        // Check reverse dependencies
        for pkg in installer.list_installed() {
            if pkg.meta.name == name {
                continue;
            }
            for dep in &pkg.meta.dependencies {
                if dep.name == name {
                    return Err(PkgError::RequiredBy(
                        name.to_string(),
                        pkg.meta.name.clone(),
                    ));
                }
            }
        }

        installer.installed.remove(name);
        Ok(())
    }
}

impl Default for PackageRemover {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Package Search
// ═══════════════════════════════════════════════════════════════════════

/// Search result.
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// Package name.
    pub name: String,
    /// Latest version.
    pub version: Version,
    /// Description.
    pub description: String,
    /// Match score (higher = better match).
    pub score: u32,
}

/// Fuzzy package search.
#[derive(Debug)]
pub struct PackageSearch;

impl PackageSearch {
    /// Creates a new search engine.
    pub fn new() -> Self {
        Self
    }

    /// Searches for packages matching the query.
    ///
    /// Matches against name, description, and tags using a simple
    /// substring-based scoring algorithm.
    pub fn search(&self, query: &str, index: &PackageIndex) -> Vec<SearchResult> {
        let query_lower = query.to_lowercase();
        let mut results: Vec<SearchResult> = Vec::new();

        for (name, versions) in &index.packages {
            if let Some(latest) = versions.iter().max_by(|a, b| a.version.cmp(&b.version)) {
                let mut score = 0u32;
                let name_lower = name.to_lowercase();
                let desc_lower = latest.description.to_lowercase();

                // Exact name match
                if name_lower == query_lower {
                    score += 100;
                }
                // Name contains query
                if name_lower.contains(&query_lower) {
                    score += 50;
                }
                // Description contains query
                if desc_lower.contains(&query_lower) {
                    score += 20;
                }
                // Tag match
                for tag in &latest.tags {
                    if tag.to_lowercase().contains(&query_lower) {
                        score += 30;
                    }
                }

                if score > 0 {
                    results.push(SearchResult {
                        name: name.clone(),
                        version: latest.version.clone(),
                        description: latest.description.clone(),
                        score,
                    });
                }
            }
        }

        results.sort_by(|a, b| b.score.cmp(&a.score));
        results
    }
}

impl Default for PackageSearch {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Package Update
// ═══════════════════════════════════════════════════════════════════════

/// Available update information.
#[derive(Debug, Clone)]
pub struct UpdateInfo {
    /// Package name.
    pub name: String,
    /// Currently installed version.
    pub current: Version,
    /// Available version.
    pub available: Version,
}

/// Package updater — checks for updates and upgrades packages.
#[derive(Debug)]
pub struct PackageUpdate;

impl PackageUpdate {
    /// Creates a new updater.
    pub fn new() -> Self {
        Self
    }

    /// Checks for available updates.
    pub fn check_updates(
        &self,
        installer: &PackageInstaller,
        index: &PackageIndex,
    ) -> Vec<UpdateInfo> {
        let mut updates = Vec::new();
        for pkg in installer.list_installed() {
            if let Some(latest) = index.latest(&pkg.meta.name) {
                if latest.version > pkg.meta.version {
                    updates.push(UpdateInfo {
                        name: pkg.meta.name.clone(),
                        current: pkg.meta.version.clone(),
                        available: latest.version.clone(),
                    });
                }
            }
        }
        updates
    }

    /// Upgrades a specific package to the latest version.
    pub fn upgrade(
        &self,
        name: &str,
        installer: &mut PackageInstaller,
        index: &PackageIndex,
        now: u64,
    ) -> Result<Version, PkgError> {
        let latest = index
            .latest(name)
            .ok_or_else(|| PkgError::PackageNotFound(name.to_string()))?
            .clone();

        // Remove old version
        installer.installed.remove(name);

        // Install new version
        let version = latest.version.clone();
        installer.install(latest, now)?;
        Ok(version)
    }
}

impl Default for PackageUpdate {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Repository
// ═══════════════════════════════════════════════════════════════════════

/// A package repository source.
#[derive(Debug, Clone)]
pub struct RepoSource {
    /// Repository name.
    pub name: String,
    /// URL (simulated).
    pub url: String,
    /// Is this repo enabled?
    pub enabled: bool,
    /// Priority (lower = preferred).
    pub priority: u32,
}

/// Repository manager.
#[derive(Debug)]
pub struct Repository {
    /// Configured sources.
    sources: Vec<RepoSource>,
}

impl Repository {
    /// Creates a new repository manager.
    pub fn new() -> Self {
        Self {
            sources: Vec::new(),
        }
    }

    /// Creates with default FajarOS repos.
    pub fn with_defaults() -> Self {
        let mut repo = Self::new();
        repo.add_source(RepoSource {
            name: "fajaros-main".to_string(),
            url: "https://pkg.fajaros.dev/main".to_string(),
            enabled: true,
            priority: 100,
        });
        repo.add_source(RepoSource {
            name: "fajaros-community".to_string(),
            url: "https://pkg.fajaros.dev/community".to_string(),
            enabled: true,
            priority: 200,
        });
        repo
    }

    /// Adds a repository source.
    pub fn add_source(&mut self, source: RepoSource) {
        self.sources.push(source);
        self.sources.sort_by_key(|s| s.priority);
    }

    /// Removes a repository by name.
    pub fn remove_source(&mut self, name: &str) -> bool {
        let len = self.sources.len();
        self.sources.retain(|s| s.name != name);
        self.sources.len() < len
    }

    /// Enables or disables a repository.
    pub fn set_enabled(&mut self, name: &str, enabled: bool) -> bool {
        for source in &mut self.sources {
            if source.name == name {
                source.enabled = enabled;
                return true;
            }
        }
        false
    }

    /// Returns all enabled repositories.
    pub fn enabled_sources(&self) -> Vec<&RepoSource> {
        self.sources.iter().filter(|s| s.enabled).collect()
    }

    /// Returns the total number of sources.
    pub fn source_count(&self) -> usize {
        self.sources.len()
    }
}

impl Default for Repository {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Package Build
// ═══════════════════════════════════════════════════════════════════════

/// Build configuration.
#[derive(Debug, Clone)]
pub struct BuildConfig {
    /// Source directory.
    pub source_dir: String,
    /// Output package name.
    pub name: String,
    /// Version to build.
    pub version: Version,
    /// Build flags.
    pub flags: Vec<String>,
    /// Target architecture.
    pub target: String,
}

/// Build result.
#[derive(Debug, Clone)]
pub struct BuildResult {
    /// Package metadata.
    pub meta: PackageMeta,
    /// Build log lines.
    pub log: Vec<String>,
    /// Build duration (simulated ms).
    pub duration_ms: u64,
    /// Whether the build succeeded.
    pub success: bool,
}

/// Package builder — builds packages from source.
#[derive(Debug)]
pub struct PackageBuild;

impl PackageBuild {
    /// Creates a new package builder.
    pub fn new() -> Self {
        Self
    }

    /// Builds a package from the given configuration.
    pub fn build(&self, config: &BuildConfig) -> Result<BuildResult, PkgError> {
        // Validate config
        if config.name.is_empty() {
            return Err(PkgError::BuildError("package name is empty".to_string()));
        }
        if config.source_dir.is_empty() {
            return Err(PkgError::BuildError("source dir is empty".to_string()));
        }

        let mut log = Vec::new();
        log.push(format!("Building {} v{}", config.name, config.version));
        log.push(format!("Source: {}", config.source_dir));
        log.push(format!("Target: {}", config.target));
        log.push("Compiling...".to_string());
        log.push("Linking...".to_string());
        log.push("Packaging...".to_string());

        // Compute a simple checksum (simulated)
        let checksum = format!(
            "{:08x}",
            config.name.len() as u32 * 31 + config.version.major * 1000
        );

        let meta = PackageMeta {
            name: config.name.clone(),
            version: config.version.clone(),
            description: format!("Built from {}", config.source_dir),
            author: "local".to_string(),
            dependencies: vec![],
            tags: vec!["local".to_string()],
            checksum,
            signature: String::new(),
            size: 65536,
        };

        log.push(format!("Build complete: {}", meta.name));

        Ok(BuildResult {
            meta,
            log,
            duration_ms: 1500,
            success: true,
        })
    }
}

impl Default for PackageBuild {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Package Verify
// ═══════════════════════════════════════════════════════════════════════

/// Package verifier — checksum and signature verification.
#[derive(Debug)]
pub struct PackageVerify {
    /// Trusted signing keys (key_id -> public key hex).
    trusted_keys: HashMap<String, String>,
}

impl PackageVerify {
    /// Creates a new verifier.
    pub fn new() -> Self {
        let mut keys = HashMap::new();
        keys.insert("fajaros-team".to_string(), "pubkey-fajaros-2026".to_string());
        Self {
            trusted_keys: keys,
        }
    }

    /// Verifies the checksum of a package.
    pub fn verify_checksum(
        &self,
        pkg: &PackageMeta,
        actual_checksum: &str,
    ) -> Result<(), PkgError> {
        if pkg.checksum != actual_checksum {
            return Err(PkgError::ChecksumMismatch(
                pkg.name.clone(),
                pkg.checksum.clone(),
                actual_checksum.to_string(),
            ));
        }
        Ok(())
    }

    /// Verifies the signature of a package.
    pub fn verify_signature(&self, pkg: &PackageMeta) -> Result<(), PkgError> {
        if pkg.signature.is_empty() {
            return Err(PkgError::SignatureInvalid(pkg.name.clone()));
        }
        // Simulated verification: signature must start with "sig-"
        if !pkg.signature.starts_with("sig-") {
            return Err(PkgError::SignatureInvalid(pkg.name.clone()));
        }
        Ok(())
    }

    /// Adds a trusted signing key.
    pub fn add_trusted_key(&mut self, key_id: &str, public_key: &str) {
        self.trusted_keys
            .insert(key_id.to_string(), public_key.to_string());
    }

    /// Returns the number of trusted keys.
    pub fn trusted_key_count(&self) -> usize {
        self.trusted_keys.len()
    }
}

impl Default for PackageVerify {
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

    // --- Version tests ---

    #[test]
    fn version_parse_and_display() {
        let v = Version::parse("1.2.3").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);
        assert_eq!(v.to_string(), "1.2.3");
    }

    #[test]
    fn version_ordering() {
        let v1 = Version::new(1, 0, 0);
        let v2 = Version::new(1, 1, 0);
        let v3 = Version::new(2, 0, 0);
        assert!(v1 < v2);
        assert!(v2 < v3);
    }

    #[test]
    fn version_satisfies_ranges() {
        let v = Version::new(1, 2, 3);
        assert!(v.satisfies("1.2.3"));      // exact
        assert!(v.satisfies("^1.0.0"));     // caret
        assert!(v.satisfies("~1.2.0"));     // tilde
        assert!(v.satisfies("*"));           // wildcard
        assert!(!v.satisfies("^2.0.0"));    // different major
        assert!(!v.satisfies("~1.3.0"));    // different minor
    }

    // --- Package Index tests ---

    #[test]
    fn index_add_and_lookup() {
        let index = PackageIndex::with_defaults();
        assert!(index.package_count() >= 4);
        let latest = index.latest("fj-core").unwrap();
        assert_eq!(latest.version, Version::new(2, 0, 0));
    }

    #[test]
    fn index_find_specific_version() {
        let index = PackageIndex::with_defaults();
        let pkg = index.find("fj-net", &Version::new(1, 2, 0));
        assert!(pkg.is_some());
    }

    // --- Resolver tests ---

    #[test]
    fn resolve_dependencies() {
        let index = PackageIndex::with_defaults();
        let resolver = PackageResolver::new();
        let resolved = resolver.resolve("fj-net", &index).unwrap();

        // fj-net depends on fj-core, so fj-core should be first
        assert!(resolved.len() >= 2);
        assert_eq!(resolved[0].name, "fj-core");
        assert_eq!(resolved[1].name, "fj-net");
    }

    #[test]
    fn resolve_unknown_package_fails() {
        let index = PackageIndex::with_defaults();
        let resolver = PackageResolver::new();
        let result = resolver.resolve("nonexistent", &index);
        assert!(result.is_err());
    }

    // --- Installer tests ---

    #[test]
    fn install_and_check() {
        let mut installer = PackageInstaller::new("/usr/local");
        let meta = PackageMeta {
            name: "test-pkg".to_string(),
            version: Version::new(1, 0, 0),
            description: "test".to_string(),
            author: "test".to_string(),
            dependencies: vec![],
            tags: vec![],
            checksum: "abc".to_string(),
            signature: "sig-abc".to_string(),
            size: 1024,
        };
        installer.install(meta, 100).unwrap();
        assert!(installer.is_installed("test-pkg"));
        assert_eq!(installer.installed_count(), 1);
    }

    #[test]
    fn install_duplicate_fails() {
        let mut installer = PackageInstaller::new("/usr/local");
        let meta = PackageMeta {
            name: "dup".to_string(),
            version: Version::new(1, 0, 0),
            description: "".to_string(),
            author: "".to_string(),
            dependencies: vec![],
            tags: vec![],
            checksum: "".to_string(),
            signature: "".to_string(),
            size: 0,
        };
        installer.install(meta.clone(), 0).unwrap();
        assert!(installer.install(meta, 0).is_err());
    }

    // --- Remover tests ---

    #[test]
    fn remove_package() {
        let mut installer = PackageInstaller::new("/usr/local");
        let meta = PackageMeta {
            name: "removeme".to_string(),
            version: Version::new(1, 0, 0),
            description: "".to_string(),
            author: "".to_string(),
            dependencies: vec![],
            tags: vec![],
            checksum: "".to_string(),
            signature: "".to_string(),
            size: 0,
        };
        installer.install(meta, 0).unwrap();
        let remover = PackageRemover::new();
        remover.remove("removeme", &mut installer).unwrap();
        assert!(!installer.is_installed("removeme"));
    }

    #[test]
    fn remove_with_dependents_fails() {
        let mut installer = PackageInstaller::new("/usr/local");
        installer
            .install(
                PackageMeta {
                    name: "base".to_string(),
                    version: Version::new(1, 0, 0),
                    description: "".to_string(),
                    author: "".to_string(),
                    dependencies: vec![],
                    tags: vec![],
                    checksum: "".to_string(),
                    signature: "".to_string(),
                    size: 0,
                },
                0,
            )
            .unwrap();
        installer
            .install(
                PackageMeta {
                    name: "child".to_string(),
                    version: Version::new(1, 0, 0),
                    description: "".to_string(),
                    author: "".to_string(),
                    dependencies: vec![Dependency {
                        name: "base".to_string(),
                        version_range: "^1.0.0".to_string(),
                    }],
                    tags: vec![],
                    checksum: "".to_string(),
                    signature: "".to_string(),
                    size: 0,
                },
                0,
            )
            .unwrap();
        let remover = PackageRemover::new();
        assert!(remover.remove("base", &mut installer).is_err());
    }

    // --- Search tests ---

    #[test]
    fn search_by_name() {
        let index = PackageIndex::with_defaults();
        let search = PackageSearch::new();
        let results = search.search("net", &index);
        assert!(!results.is_empty());
        assert!(results.iter().any(|r| r.name == "fj-net"));
    }

    #[test]
    fn search_by_tag() {
        let index = PackageIndex::with_defaults();
        let search = PackageSearch::new();
        let results = search.search("tensor", &index);
        assert!(!results.is_empty());
        assert!(results.iter().any(|r| r.name == "fj-ml"));
    }

    // --- Update tests ---

    #[test]
    fn check_updates() {
        let mut installer = PackageInstaller::new("/usr/local");
        installer
            .install(
                PackageMeta {
                    name: "fj-core".to_string(),
                    version: Version::new(1, 0, 0), // old version
                    description: "old".to_string(),
                    author: "".to_string(),
                    dependencies: vec![],
                    tags: vec![],
                    checksum: "".to_string(),
                    signature: "".to_string(),
                    size: 0,
                },
                0,
            )
            .unwrap();
        let index = PackageIndex::with_defaults(); // has fj-core 2.0.0
        let updater = PackageUpdate::new();
        let updates = updater.check_updates(&installer, &index);
        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0].name, "fj-core");
        assert_eq!(updates[0].available, Version::new(2, 0, 0));
    }

    #[test]
    fn upgrade_package() {
        let mut installer = PackageInstaller::new("/usr/local");
        installer
            .install(
                PackageMeta {
                    name: "fj-core".to_string(),
                    version: Version::new(1, 0, 0),
                    description: "old".to_string(),
                    author: "".to_string(),
                    dependencies: vec![],
                    tags: vec![],
                    checksum: "".to_string(),
                    signature: "".to_string(),
                    size: 0,
                },
                0,
            )
            .unwrap();
        let index = PackageIndex::with_defaults();
        let updater = PackageUpdate::new();
        let new_ver = updater.upgrade("fj-core", &mut installer, &index, 100).unwrap();
        assert_eq!(new_ver, Version::new(2, 0, 0));
    }

    // --- Repository tests ---

    #[test]
    fn repo_add_and_remove() {
        let mut repo = Repository::with_defaults();
        assert_eq!(repo.source_count(), 2);
        repo.remove_source("fajaros-community");
        assert_eq!(repo.source_count(), 1);
    }

    #[test]
    fn repo_enable_disable() {
        let mut repo = Repository::with_defaults();
        repo.set_enabled("fajaros-community", false);
        assert_eq!(repo.enabled_sources().len(), 1);
        repo.set_enabled("fajaros-community", true);
        assert_eq!(repo.enabled_sources().len(), 2);
    }

    // --- Build tests ---

    #[test]
    fn build_package() {
        let builder = PackageBuild::new();
        let config = BuildConfig {
            source_dir: "/home/fajar/my-pkg".to_string(),
            name: "my-pkg".to_string(),
            version: Version::new(0, 1, 0),
            flags: vec![],
            target: "x86_64".to_string(),
        };
        let result = builder.build(&config).unwrap();
        assert!(result.success);
        assert_eq!(result.meta.name, "my-pkg");
        assert!(!result.log.is_empty());
    }

    #[test]
    fn build_empty_name_fails() {
        let builder = PackageBuild::new();
        let config = BuildConfig {
            source_dir: "/src".to_string(),
            name: "".to_string(),
            version: Version::new(0, 1, 0),
            flags: vec![],
            target: "x86_64".to_string(),
        };
        assert!(builder.build(&config).is_err());
    }

    // --- Verify tests ---

    #[test]
    fn verify_checksum_ok() {
        let verifier = PackageVerify::new();
        let pkg = PackageMeta {
            name: "test".to_string(),
            version: Version::new(1, 0, 0),
            description: "".to_string(),
            author: "".to_string(),
            dependencies: vec![],
            tags: vec![],
            checksum: "abc123".to_string(),
            signature: "sig-ok".to_string(),
            size: 0,
        };
        assert!(verifier.verify_checksum(&pkg, "abc123").is_ok());
        assert!(verifier.verify_checksum(&pkg, "wrong").is_err());
    }

    #[test]
    fn verify_signature() {
        let verifier = PackageVerify::new();
        let valid = PackageMeta {
            name: "test".to_string(),
            version: Version::new(1, 0, 0),
            description: "".to_string(),
            author: "".to_string(),
            dependencies: vec![],
            tags: vec![],
            checksum: "".to_string(),
            signature: "sig-valid".to_string(),
            size: 0,
        };
        assert!(verifier.verify_signature(&valid).is_ok());

        let invalid = PackageMeta {
            name: "bad".to_string(),
            version: Version::new(1, 0, 0),
            description: "".to_string(),
            author: "".to_string(),
            dependencies: vec![],
            tags: vec![],
            checksum: "".to_string(),
            signature: "".to_string(),
            size: 0,
        };
        assert!(verifier.verify_signature(&invalid).is_err());
    }
}
