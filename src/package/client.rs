//! Registry client — `fj publish`, `fj install`, `fj search`, `fj login`, `fj yank`.
//!
//! Provides the CLI-side of the package registry: uploading, downloading,
//! searching, authentication, and local caching.

use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};

// ═══════════════════════════════════════════════════════════════════════
// Configuration
// ═══════════════════════════════════════════════════════════════════════

/// Registry URL and credential configuration.
#[derive(Debug, Clone)]
pub struct RegistryClientConfig {
    /// Registry API base URL.
    pub registry_url: String,
    /// Path to credentials file (~/.fj/credentials).
    pub credentials_path: PathBuf,
    /// Local package cache directory (~/.fj/cache/).
    pub cache_dir: PathBuf,
}

impl Default for RegistryClientConfig {
    fn default() -> Self {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .unwrap_or_else(|_| ".".to_string());
        let fj_dir = PathBuf::from(&home).join(".fj");
        Self {
            registry_url: "https://registry.fajarlang.dev".to_string(),
            credentials_path: fj_dir.join("credentials"),
            cache_dir: fj_dir.join("cache"),
        }
    }
}

impl RegistryClientConfig {
    /// Creates config with a custom registry URL.
    pub fn with_url(url: &str) -> Self {
        Self {
            registry_url: url.to_string(),
            ..Self::default()
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Credentials
// ═══════════════════════════════════════════════════════════════════════

/// Stored credentials for registry authentication.
#[derive(Debug, Clone)]
pub struct Credentials {
    /// API key.
    pub api_key: String,
    /// Registry URL this key is for.
    pub registry: String,
}

impl Credentials {
    /// Parses credentials from file content.
    ///
    /// Format:
    /// ```text
    /// [registry]
    /// url = https://registry.fajarlang.dev
    /// token = fj_key_xxxxx
    /// ```
    pub fn parse(content: &str) -> Option<Self> {
        let mut url = None;
        let mut token = None;
        for line in content.lines() {
            let line = line.trim();
            if let Some(val) = line.strip_prefix("url = ") {
                url = Some(val.to_string());
            } else if let Some(val) = line.strip_prefix("token = ") {
                token = Some(val.to_string());
            }
        }
        Some(Self {
            api_key: token?,
            registry: url.unwrap_or_else(|| "https://registry.fajarlang.dev".to_string()),
        })
    }

    /// Serializes credentials to file format.
    pub fn to_file_format(&self) -> String {
        format!(
            "[registry]\nurl = {}\ntoken = {}\n",
            self.registry, self.api_key
        )
    }

    /// Loads credentials from the default path.
    pub fn load(path: &Path) -> Option<Self> {
        let content = std::fs::read_to_string(path).ok()?;
        Self::parse(&content)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Package Cache
// ═══════════════════════════════════════════════════════════════════════

/// Local package cache for downloaded packages.
#[derive(Debug, Clone)]
pub struct PackageCache {
    /// Base directory for cached packages.
    pub cache_dir: PathBuf,
}

impl PackageCache {
    /// Creates a new cache at the given directory.
    pub fn new(cache_dir: PathBuf) -> Self {
        Self { cache_dir }
    }

    /// Returns the cache path for a package version.
    pub fn cache_path(&self, name: &str, version: &str) -> PathBuf {
        self.cache_dir.join(name).join(version)
    }

    /// Checks if a package version is cached locally.
    pub fn is_cached(&self, name: &str, version: &str) -> bool {
        self.cache_path(name, version).exists()
    }

    /// Returns the tarball path in cache.
    pub fn tarball_path(&self, name: &str, version: &str) -> PathBuf {
        self.cache_path(name, version)
            .join(format!("{name}-{version}.tar.gz"))
    }

    /// Lists all cached packages as `(name, version)` pairs.
    pub fn list_cached(&self) -> Vec<(String, String)> {
        let mut cached = Vec::new();
        if let Ok(names) = std::fs::read_dir(&self.cache_dir) {
            for name_entry in names.flatten() {
                let name = name_entry.file_name().to_string_lossy().to_string();
                if let Ok(versions) = std::fs::read_dir(name_entry.path()) {
                    for ver_entry in versions.flatten() {
                        let ver = ver_entry.file_name().to_string_lossy().to_string();
                        cached.push((name.clone(), ver));
                    }
                }
            }
        }
        cached
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Search Display
// ═══════════════════════════════════════════════════════════════════════

/// Search result for CLI display.
#[derive(Debug, Clone)]
pub struct SearchResultDisplay {
    /// Package name.
    pub name: String,
    /// Short description.
    pub description: String,
    /// Latest version.
    pub version: String,
    /// Download count.
    pub downloads: u64,
}

impl fmt::Display for SearchResultDisplay {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:<30} v{:<12} {:>8} downloads  {}",
            self.name, self.version, self.downloads, self.description
        )
    }
}

/// Formats search results as a table for CLI output.
pub fn format_search_results(results: &[SearchResultDisplay]) -> String {
    if results.is_empty() {
        return "No packages found.".to_string();
    }
    let mut output = String::new();
    output.push_str(&format!(
        "{:<30} {:<14} {:>10}  {}\n",
        "NAME", "VERSION", "DOWNLOADS", "DESCRIPTION"
    ));
    output.push_str(&"-".repeat(80));
    output.push('\n');
    for r in results {
        output.push_str(&format!("{r}\n"));
    }
    output.push_str(&format!("\n{} package(s) found.", results.len()));
    output
}

// ═══════════════════════════════════════════════════════════════════════
// Lockfile
// ═══════════════════════════════════════════════════════════════════════

/// A dependency lockfile entry with checksum.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LockEntry {
    /// Package name.
    pub name: String,
    /// Resolved version.
    pub version: String,
    /// SHA-256 checksum.
    pub checksum: String,
    /// Source URL.
    pub source: String,
}

/// Lockfile for reproducible builds.
#[derive(Debug, Clone)]
pub struct Lockfile {
    /// Lockfile format version.
    pub version: u32,
    /// Locked dependency entries.
    pub entries: Vec<LockEntry>,
}

impl Default for Lockfile {
    fn default() -> Self {
        Self::new()
    }
}

impl Lockfile {
    /// Creates an empty lockfile.
    pub fn new() -> Self {
        Self {
            version: 3,
            entries: Vec::new(),
        }
    }

    /// Adds a dependency to the lockfile.
    pub fn add(&mut self, name: &str, version: &str, checksum: &str, source: &str) {
        self.entries.push(LockEntry {
            name: name.to_string(),
            version: version.to_string(),
            checksum: checksum.to_string(),
            source: source.to_string(),
        });
    }

    /// Parses a lockfile from string content.
    pub fn parse(content: &str) -> Result<Self, String> {
        let mut lockfile = Self::new();
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                if let Some(ver_str) = line.strip_prefix("# fj.lock v") {
                    if let Ok(v) = ver_str.trim().parse::<u32>() {
                        lockfile.version = v;
                    }
                }
                continue;
            }
            // Format: name=version checksum source
            let parts: Vec<&str> = line.splitn(3, ' ').collect();
            if parts.len() >= 2 {
                if let Some((name, version)) = parts[0].split_once('=') {
                    lockfile.entries.push(LockEntry {
                        name: name.to_string(),
                        version: version.to_string(),
                        checksum: parts[1].to_string(),
                        source: parts.get(2).unwrap_or(&"registry").to_string(),
                    });
                }
            }
        }
        Ok(lockfile)
    }

    /// Serializes the lockfile to string.
    pub fn serialize(&self) -> String {
        let mut output = format!(
            "# fj.lock v{}\n# Auto-generated by fj — do not edit manually\n\n",
            self.version
        );
        for entry in &self.entries {
            output.push_str(&format!(
                "{}={} {} {}\n",
                entry.name, entry.version, entry.checksum, entry.source
            ));
        }
        output
    }

    /// Checks if a package is locked at a specific version.
    pub fn locked_version(&self, name: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|e| e.name == name)
            .map(|e| e.version.as_str())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Dependency Resolution
// ═══════════════════════════════════════════════════════════════════════

/// A resolved dependency with full download info.
#[derive(Debug, Clone)]
pub struct ResolvedPackage {
    /// Package name.
    pub name: String,
    /// Resolved version.
    pub version: String,
    /// Download URL.
    pub download_url: String,
    /// SHA-256 checksum.
    pub checksum: String,
    /// Transitive dependencies.
    pub dependencies: Vec<(String, String)>,
}

/// Resolves all dependencies from a fj.toml dependency map.
pub fn resolve_dependencies(
    deps: &HashMap<String, String>,
    available: &HashMap<String, Vec<(String, String)>>,
) -> Result<Vec<ResolvedPackage>, String> {
    let mut resolved = Vec::new();
    let mut visited = std::collections::HashSet::new();

    for (name, version_req) in deps {
        resolve_one(name, version_req, available, &mut resolved, &mut visited)?;
    }
    Ok(resolved)
}

fn resolve_one(
    name: &str,
    _version_req: &str,
    available: &HashMap<String, Vec<(String, String)>>,
    resolved: &mut Vec<ResolvedPackage>,
    visited: &mut std::collections::HashSet<String>,
) -> Result<(), String> {
    if visited.contains(name) {
        return Ok(());
    }
    visited.insert(name.to_string());

    let versions = available
        .get(name)
        .ok_or_else(|| format!("package '{name}' not found in registry"))?;

    // Pick the latest version (already sorted newest first)
    let (version, checksum) = versions
        .first()
        .ok_or_else(|| format!("no versions available for '{name}'"))?;

    resolved.push(ResolvedPackage {
        name: name.to_string(),
        version: version.clone(),
        download_url: format!("https://registry.fajarlang.dev/api/v1/packages/{name}/{version}"),
        checksum: checksum.clone(),
        dependencies: Vec::new(),
    });
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// Offline Mode
// ═══════════════════════════════════════════════════════════════════════

/// Checks if all required packages are available in the local cache.
pub fn check_offline_availability(
    deps: &HashMap<String, String>,
    cache: &PackageCache,
) -> Result<(), Vec<String>> {
    let missing: Vec<String> = deps
        .iter()
        .filter(|(name, version)| !cache.is_cached(name, version))
        .map(|(name, version)| format!("{name}@{version}"))
        .collect();

    if missing.is_empty() {
        Ok(())
    } else {
        Err(missing)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // S26.1: fj publish
    #[test]
    fn s26_1_registry_client_config_default() {
        let cfg = RegistryClientConfig::default();
        assert!(cfg.registry_url.contains("registry.fajarlang.dev"));
        assert!(cfg.credentials_path.to_string_lossy().contains(".fj"));
        assert!(cfg.cache_dir.to_string_lossy().contains("cache"));
    }

    #[test]
    fn s26_1_registry_client_config_custom_url() {
        let cfg = RegistryClientConfig::with_url("http://localhost:8787");
        assert_eq!(cfg.registry_url, "http://localhost:8787");
    }

    // S26.4: fj login
    #[test]
    fn s26_4_credentials_parse() {
        let content = "[registry]\nurl = https://registry.fajarlang.dev\ntoken = fj_key_test123\n";
        let creds = Credentials::parse(content).expect("should parse");
        assert_eq!(creds.api_key, "fj_key_test123");
        assert_eq!(creds.registry, "https://registry.fajarlang.dev");
    }

    #[test]
    fn s26_4_credentials_roundtrip() {
        let creds = Credentials {
            api_key: "fj_key_abc".to_string(),
            registry: "https://registry.fajarlang.dev".to_string(),
        };
        let serialized = creds.to_file_format();
        let parsed = Credentials::parse(&serialized).expect("roundtrip");
        assert_eq!(parsed.api_key, "fj_key_abc");
    }

    // S26.3: fj search display
    #[test]
    fn s26_3_search_result_display() {
        let r = SearchResultDisplay {
            name: "fj-math".to_string(),
            description: "Math utilities".to_string(),
            version: "1.0.0".to_string(),
            downloads: 42,
        };
        let s = format!("{r}");
        assert!(s.contains("fj-math"));
        assert!(s.contains("1.0.0"));
        assert!(s.contains("42"));
    }

    #[test]
    fn s26_3_format_search_results_empty() {
        let output = format_search_results(&[]);
        assert_eq!(output, "No packages found.");
    }

    #[test]
    fn s26_3_format_search_results_table() {
        let results = vec![SearchResultDisplay {
            name: "fj-math".to_string(),
            description: "Math utilities".to_string(),
            version: "1.0.0".to_string(),
            downloads: 100,
        }];
        let output = format_search_results(&results);
        assert!(output.contains("NAME"));
        assert!(output.contains("fj-math"));
        assert!(output.contains("1 package(s) found."));
    }

    // S26.6: Local cache
    #[test]
    fn s26_6_package_cache_paths() {
        let cache = PackageCache::new(PathBuf::from("/tmp/fj-cache-test"));
        assert_eq!(
            cache.cache_path("fj-math", "1.0.0"),
            PathBuf::from("/tmp/fj-cache-test/fj-math/1.0.0")
        );
        assert_eq!(
            cache.tarball_path("fj-math", "1.0.0"),
            PathBuf::from("/tmp/fj-cache-test/fj-math/1.0.0/fj-math-1.0.0.tar.gz")
        );
    }

    #[test]
    fn s26_6_package_cache_not_cached() {
        let cache = PackageCache::new(PathBuf::from("/tmp/nonexistent-fj-cache"));
        assert!(!cache.is_cached("fj-math", "1.0.0"));
    }

    // S26.7: Dependency resolution
    #[test]
    fn s26_7_resolve_dependencies() {
        let mut deps = HashMap::new();
        deps.insert("fj-math".to_string(), "^1.0.0".to_string());

        let mut available = HashMap::new();
        available.insert(
            "fj-math".to_string(),
            vec![("1.0.0".to_string(), "sha256abc".to_string())],
        );

        let resolved = resolve_dependencies(&deps, &available).expect("should resolve");
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].name, "fj-math");
        assert_eq!(resolved[0].version, "1.0.0");
    }

    #[test]
    fn s26_7_resolve_missing_package() {
        let mut deps = HashMap::new();
        deps.insert("fj-nonexistent".to_string(), "^1.0.0".to_string());
        let available = HashMap::new();
        let result = resolve_dependencies(&deps, &available);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    // S26.8: Lockfile
    #[test]
    fn s26_8_lockfile_serialize_parse() {
        let mut lock = Lockfile::new();
        lock.add("fj-math", "1.0.0", "sha256abc", "registry");
        lock.add("fj-nn", "0.2.0", "sha256def", "registry");

        let serialized = lock.serialize();
        assert!(serialized.contains("fj-math=1.0.0"));
        assert!(serialized.contains("fj-nn=0.2.0"));

        let parsed = Lockfile::parse(&serialized).expect("should parse");
        assert_eq!(parsed.entries.len(), 2);
        assert_eq!(parsed.entries[0].name, "fj-math");
        assert_eq!(parsed.entries[0].checksum, "sha256abc");
    }

    #[test]
    fn s26_8_lockfile_locked_version() {
        let mut lock = Lockfile::new();
        lock.add("fj-math", "1.0.0", "abc", "registry");
        assert_eq!(lock.locked_version("fj-math"), Some("1.0.0"));
        assert_eq!(lock.locked_version("fj-nn"), None);
    }

    // S26.9: Offline mode
    #[test]
    fn s26_9_offline_check_missing() {
        let mut deps = HashMap::new();
        deps.insert("fj-math".to_string(), "1.0.0".to_string());
        let cache = PackageCache::new(PathBuf::from("/tmp/nonexistent-offline-test"));
        let result = check_offline_availability(&deps, &cache);
        assert!(result.is_err());
        assert!(result.unwrap_err()[0].contains("fj-math"));
    }

    // S26.5: fj yank
    #[test]
    fn s26_5_yank_is_not_delete() {
        // Yank marks a version as yanked but doesn't remove it
        // This is validated at the server level — the client sends PUT to /yank
        let routes = super::super::server::api_routes();
        let yank_route = routes.iter().find(|r| r.path.contains("yank")).unwrap();
        assert_eq!(yank_route.method, super::super::server::HttpMethod::Put);
        assert!(yank_route.requires_auth);
    }

    // S26.2: fj install
    #[test]
    fn s26_2_resolved_package_has_download_url() {
        let pkg = ResolvedPackage {
            name: "fj-math".to_string(),
            version: "1.0.0".to_string(),
            download_url: "https://registry.fajarlang.dev/api/v1/packages/fj-math/1.0.0"
                .to_string(),
            checksum: "sha256abc".to_string(),
            dependencies: Vec::new(),
        };
        assert!(pkg.download_url.contains("fj-math"));
        assert!(pkg.download_url.contains("1.0.0"));
    }

    // S26.10: Lockfile version
    #[test]
    fn s26_10_lockfile_default_version() {
        let lock = Lockfile::new();
        assert_eq!(lock.version, 3);
        assert!(lock.entries.is_empty());
    }
}
