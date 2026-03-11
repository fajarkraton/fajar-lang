//! Package registry — semver parsing, version constraints, and resolution.
//!
//! Provides a local file-based registry for Fajar Lang packages.

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// Semantic Version
// ═══════════════════════════════════════════════════════════════════════

/// A semantic version: `major.minor.patch`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SemVer {
    /// Major version (breaking changes).
    pub major: u32,
    /// Minor version (new features, backwards compatible).
    pub minor: u32,
    /// Patch version (bug fixes).
    pub patch: u32,
}

impl SemVer {
    /// Creates a new SemVer.
    pub fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    /// Parses a version string like "1.2.3".
    pub fn parse(s: &str) -> Result<Self, String> {
        let parts: Vec<&str> = s.trim().split('.').collect();
        if parts.len() != 3 {
            return Err(format!(
                "invalid semver: '{s}' (expected major.minor.patch)"
            ));
        }
        let major = parts[0]
            .parse::<u32>()
            .map_err(|_| format!("invalid major version: '{}'", parts[0]))?;
        let minor = parts[1]
            .parse::<u32>()
            .map_err(|_| format!("invalid minor version: '{}'", parts[1]))?;
        let patch = parts[2]
            .parse::<u32>()
            .map_err(|_| format!("invalid patch version: '{}'", parts[2]))?;
        Ok(Self::new(major, minor, patch))
    }
}

impl Ord for SemVer {
    fn cmp(&self, other: &Self) -> Ordering {
        self.major
            .cmp(&other.major)
            .then(self.minor.cmp(&other.minor))
            .then(self.patch.cmp(&other.patch))
    }
}

impl PartialOrd for SemVer {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for SemVer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Version Constraint
// ═══════════════════════════════════════════════════════════════════════

/// A version constraint for dependency resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VersionConstraint {
    /// Exact version: `=1.2.3` or `1.2.3`.
    Exact(SemVer),
    /// Compatible (caret): `^1.2.3` — allows minor/patch bumps.
    Caret(SemVer),
    /// Tilde: `~1.2.3` — allows patch bumps only.
    Tilde(SemVer),
    /// Greater or equal: `>=1.2.3`.
    GreaterEqual(SemVer),
    /// Less than: `<2.0.0`.
    LessThan(SemVer),
    /// Wildcard: `*` — any version.
    Any,
}

impl VersionConstraint {
    /// Parses a constraint string.
    ///
    /// Supported formats:
    /// - `"1.2.3"` or `"=1.2.3"` — exact
    /// - `"^1.2.3"` — caret (compatible)
    /// - `"~1.2.3"` — tilde (patch only)
    /// - `">=1.2.3"` — greater or equal
    /// - `"<1.2.3"` — less than
    /// - `"*"` — any version
    pub fn parse(s: &str) -> Result<Self, String> {
        let s = s.trim();
        if s == "*" {
            return Ok(VersionConstraint::Any);
        }
        if let Some(rest) = s.strip_prefix(">=") {
            return Ok(VersionConstraint::GreaterEqual(SemVer::parse(rest)?));
        }
        if let Some(rest) = s.strip_prefix('^') {
            return Ok(VersionConstraint::Caret(SemVer::parse(rest)?));
        }
        if let Some(rest) = s.strip_prefix('~') {
            return Ok(VersionConstraint::Tilde(SemVer::parse(rest)?));
        }
        if let Some(rest) = s.strip_prefix('<') {
            return Ok(VersionConstraint::LessThan(SemVer::parse(rest)?));
        }
        if let Some(rest) = s.strip_prefix('=') {
            return Ok(VersionConstraint::Exact(SemVer::parse(rest)?));
        }
        // Default: exact match
        Ok(VersionConstraint::Exact(SemVer::parse(s)?))
    }

    /// Returns true if the given version satisfies this constraint.
    pub fn matches(&self, version: &SemVer) -> bool {
        match self {
            VersionConstraint::Exact(v) => version == v,
            VersionConstraint::Any => true,
            VersionConstraint::GreaterEqual(v) => version >= v,
            VersionConstraint::LessThan(v) => version < v,
            VersionConstraint::Caret(v) => {
                // ^X.Y.Z: >=X.Y.Z, <(X+1).0.0 (when X > 0)
                //          >=0.Y.Z, <0.(Y+1).0 (when X == 0, Y > 0)
                //          >=0.0.Z, <0.0.(Z+1) (when X == 0, Y == 0)
                if version < v {
                    return false;
                }
                if v.major > 0 {
                    version.major == v.major
                } else if v.minor > 0 {
                    version.major == 0 && version.minor == v.minor
                } else {
                    version.major == 0 && version.minor == 0 && version.patch == v.patch
                }
            }
            VersionConstraint::Tilde(v) => {
                // ~X.Y.Z: >=X.Y.Z, <X.(Y+1).0
                if version < v {
                    return false;
                }
                version.major == v.major && version.minor == v.minor
            }
        }
    }
}

impl fmt::Display for VersionConstraint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VersionConstraint::Exact(v) => write!(f, "={v}"),
            VersionConstraint::Caret(v) => write!(f, "^{v}"),
            VersionConstraint::Tilde(v) => write!(f, "~{v}"),
            VersionConstraint::GreaterEqual(v) => write!(f, ">={v}"),
            VersionConstraint::LessThan(v) => write!(f, "<{v}"),
            VersionConstraint::Any => write!(f, "*"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Authentication
// ═══════════════════════════════════════════════════════════════════════

/// An authentication token for registry operations.
///
/// Tokens can optionally be scoped to a specific package, granting
/// publish/yank permissions only for that package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthToken {
    /// The token string (opaque bearer token).
    pub token: String,
    /// Optional scope — if `Some`, this token is only valid for the named package.
    pub scope: Option<String>,
}

impl AuthToken {
    /// Creates a new unscoped authentication token.
    pub fn new(token: &str) -> Self {
        Self {
            token: token.to_string(),
            scope: None,
        }
    }

    /// Creates a new scoped authentication token for a specific package.
    pub fn scoped(token: &str, package_name: &str) -> Self {
        Self {
            token: token.to_string(),
            scope: Some(package_name.to_string()),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Registry Configuration
// ═══════════════════════════════════════════════════════════════════════

/// Configuration for connecting to a package registry.
#[derive(Debug, Clone)]
pub struct RegistryConfig {
    /// Base URL of the registry (e.g., `https://registry.fajarlang.dev`).
    pub registry_url: String,
    /// API endpoint URL (e.g., `https://registry.fajarlang.dev/api/v1`).
    pub api_url: String,
    /// Whether authentication is required for publish/yank operations.
    pub auth_required: bool,
}

impl Default for RegistryConfig {
    fn default() -> Self {
        Self {
            registry_url: "https://registry.fajarlang.dev".to_string(),
            api_url: "https://registry.fajarlang.dev/api/v1".to_string(),
            auth_required: true,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Sparse Index
// ═══════════════════════════════════════════════════════════════════════

/// A dependency entry within a sparse index version record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SparseIndexDep {
    /// Dependency package name.
    pub name: String,
    /// Version constraint string (e.g., `"^1.0.0"`).
    pub req: String,
}

/// A single version record within a sparse index entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SparseIndexVersion {
    /// The version string.
    pub vers: String,
    /// Dependencies for this version.
    pub deps: Vec<SparseIndexDep>,
    /// SHA-256 checksum of the published archive.
    pub cksum: String,
    /// Whether this version has been yanked.
    pub yanked: bool,
}

/// A sparse index entry for a single package.
///
/// The sparse index is a per-package JSON document containing all
/// versions, their dependencies, checksums, and yank status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SparseIndexEntry {
    /// Package name.
    pub name: String,
    /// All published versions for this package.
    pub versions: Vec<SparseIndexVersion>,
}

// ═══════════════════════════════════════════════════════════════════════
// Package Registry
// ═══════════════════════════════════════════════════════════════════════

/// Metadata about a registered package.
#[derive(Debug, Clone)]
pub struct PackageEntry {
    /// Package name.
    pub name: String,
    /// Available versions (sorted ascending).
    pub versions: Vec<SemVer>,
    /// Description.
    pub description: String,
    /// Total download count across all versions.
    pub download_count: u64,
}

/// A simple in-memory package registry.
#[derive(Debug, Default)]
pub struct Registry {
    /// Registered packages.
    packages: HashMap<String, PackageEntry>,
    /// Set of yanked (package_name, version) pairs.
    yanked: HashSet<(String, SemVer)>,
    /// Stored authentication tokens.
    tokens: Vec<AuthToken>,
    /// Per-version dependency metadata: `(name, version)` -> deps map.
    version_deps: HashMap<(String, SemVer), HashMap<String, String>>,
    /// Per-version checksum: `(name, version)` -> SHA-256 hex string.
    version_checksums: HashMap<(String, SemVer), String>,
}

impl Registry {
    /// Creates an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a package version.
    pub fn publish(&mut self, name: &str, version: SemVer, description: &str) {
        let entry = self
            .packages
            .entry(name.to_string())
            .or_insert_with(|| PackageEntry {
                name: name.to_string(),
                versions: Vec::new(),
                description: description.to_string(),
                download_count: 0,
            });
        if !entry.versions.contains(&version) {
            entry.versions.push(version);
            entry.versions.sort();
        }
    }

    /// Registers a package version with dependency and checksum metadata.
    pub fn publish_with_meta(
        &mut self,
        name: &str,
        version: SemVer,
        description: &str,
        deps: HashMap<String, String>,
        checksum: &str,
    ) {
        self.publish(name, version.clone(), description);
        self.version_deps
            .insert((name.to_string(), version.clone()), deps);
        self.version_checksums
            .insert((name.to_string(), version), checksum.to_string());
    }

    /// Looks up a package by name.
    pub fn lookup(&self, name: &str) -> Option<&PackageEntry> {
        self.packages.get(name)
    }

    /// Resolves the best matching version for a constraint.
    ///
    /// Returns the highest non-yanked version that satisfies the constraint.
    pub fn resolve(&self, name: &str, constraint: &VersionConstraint) -> Option<SemVer> {
        let entry = self.packages.get(name)?;
        entry
            .versions
            .iter()
            .rev()
            .find(|v| constraint.matches(v) && !self.is_yanked(name, v))
            .cloned()
    }

    /// Resolves a version and increments the download counter.
    ///
    /// Like `resolve`, but requires `&mut self` so it can update the
    /// download count for the matched package.
    pub fn resolve_and_count(
        &mut self,
        name: &str,
        constraint: &VersionConstraint,
    ) -> Option<SemVer> {
        let entry = self.packages.get(name)?;
        let resolved = entry
            .versions
            .iter()
            .rev()
            .find(|v| constraint.matches(v) && !self.is_yanked(name, v))
            .cloned();

        if resolved.is_some() {
            if let Some(entry) = self.packages.get_mut(name) {
                entry.download_count = entry.download_count.saturating_add(1);
            }
        }
        resolved
    }

    /// Returns the number of registered packages.
    pub fn package_count(&self) -> usize {
        self.packages.len()
    }

    /// Searches packages by name prefix or substring.
    ///
    /// Returns matching entries sorted by name, with download counts visible.
    pub fn search(&self, query: &str) -> Vec<&PackageEntry> {
        let q = query.to_lowercase();
        let mut results: Vec<&PackageEntry> = self
            .packages
            .values()
            .filter(|entry| entry.name.contains(&q))
            .collect();
        results.sort_by(|a, b| {
            // Prefix matches first, then alphabetical
            let a_prefix = a.name.starts_with(&q);
            let b_prefix = b.name.starts_with(&q);
            b_prefix.cmp(&a_prefix).then(a.name.cmp(&b.name))
        });
        results
    }

    /// Lists all packages in the registry, sorted by name.
    pub fn list_all(&self) -> Vec<&PackageEntry> {
        let mut entries: Vec<&PackageEntry> = self.packages.values().collect();
        entries.sort_by(|a, b| a.name.cmp(&b.name));
        entries
    }

    /// Returns the latest version of a package, if it exists.
    pub fn latest_version(&self, name: &str) -> Option<&SemVer> {
        self.packages.get(name)?.versions.last()
    }

    // ── Yank management ──

    /// Marks a specific version of a package as yanked.
    ///
    /// Yanked versions will be skipped during resolution but remain
    /// visible in package metadata for audit purposes.
    pub fn yank(&mut self, name: &str, version: &SemVer) -> Result<(), String> {
        let entry = self
            .packages
            .get(name)
            .ok_or_else(|| format!("package '{name}' not found"))?;
        if !entry.versions.contains(version) {
            return Err(format!("version {version} of '{name}' not found"));
        }
        self.yanked.insert((name.to_string(), version.clone()));
        Ok(())
    }

    /// Returns `true` if the given package version has been yanked.
    pub fn is_yanked(&self, name: &str, version: &SemVer) -> bool {
        self.yanked.contains(&(name.to_string(), version.clone()))
    }

    // ── Token management ──

    /// Adds an authentication token to the registry.
    pub fn add_token(&mut self, token: AuthToken) {
        self.tokens.push(token);
    }

    /// Validates a token string, returning `true` if a matching token exists.
    ///
    /// If `package_name` is `Some`, also checks that the token has either
    /// no scope (global) or a scope matching the given package.
    pub fn validate_token(&self, token_str: &str, package_name: Option<&str>) -> bool {
        self.tokens.iter().any(|t| {
            if t.token != token_str {
                return false;
            }
            match (&t.scope, package_name) {
                // Unscoped token: valid for any package
                (None, _) => true,
                // Scoped token matches the requested package
                (Some(scope), Some(pkg)) => scope == pkg,
                // Scoped token but no package requested: valid (general auth check)
                (Some(_), None) => true,
            }
        })
    }

    // ── Sparse index ──

    /// Generates a sparse index entry for a package.
    ///
    /// The sparse index contains all versions, their dependency metadata,
    /// checksums, and yank status. Returns `None` if the package does not exist.
    pub fn to_sparse_index(&self, name: &str) -> Option<SparseIndexEntry> {
        let entry = self.packages.get(name)?;
        let versions = entry
            .versions
            .iter()
            .map(|v| {
                let deps = self
                    .version_deps
                    .get(&(name.to_string(), v.clone()))
                    .cloned()
                    .unwrap_or_default()
                    .into_iter()
                    .map(|(dep_name, req)| SparseIndexDep {
                        name: dep_name,
                        req,
                    })
                    .collect();
                let cksum = self
                    .version_checksums
                    .get(&(name.to_string(), v.clone()))
                    .cloned()
                    .unwrap_or_default();
                SparseIndexVersion {
                    vers: v.to_string(),
                    deps,
                    cksum,
                    yanked: self.is_yanked(name, v),
                }
            })
            .collect();
        Some(SparseIndexEntry {
            name: name.to_string(),
            versions,
        })
    }

    /// Serializes a sparse index entry to JSON.
    ///
    /// Returns `None` if the package does not exist.
    pub fn sparse_index_json(&self, name: &str) -> Option<String> {
        let entry = self.to_sparse_index(name)?;
        Some(sparse_index_to_json(&entry))
    }

    // ── Download count ──

    /// Returns the download count for a package, or `None` if it does not exist.
    pub fn download_count(&self, name: &str) -> Option<u64> {
        self.packages.get(name).map(|e| e.download_count)
    }

    /// Returns all registered package names.
    pub fn package_names(&self) -> Vec<&str> {
        self.packages.keys().map(|s| s.as_str()).collect()
    }
}

/// Escapes a string for JSON output (handles `"` and `\`).
fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(ch),
        }
    }
    out
}

/// Serializes a `SparseIndexEntry` to a pretty-printed JSON string.
fn sparse_index_to_json(entry: &SparseIndexEntry) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!("  \"name\": \"{}\",\n", json_escape(&entry.name)));
    out.push_str("  \"versions\": [\n");
    for (i, ver) in entry.versions.iter().enumerate() {
        out.push_str("    {\n");
        out.push_str(&format!(
            "      \"vers\": \"{}\",\n",
            json_escape(&ver.vers)
        ));
        out.push_str("      \"deps\": [");
        if ver.deps.is_empty() {
            out.push(']');
        } else {
            out.push('\n');
            for (j, dep) in ver.deps.iter().enumerate() {
                out.push_str(&format!(
                    "        {{ \"name\": \"{}\", \"req\": \"{}\" }}",
                    json_escape(&dep.name),
                    json_escape(&dep.req),
                ));
                if j + 1 < ver.deps.len() {
                    out.push(',');
                }
                out.push('\n');
            }
            out.push_str("      ]");
        }
        out.push_str(",\n");
        out.push_str(&format!(
            "      \"cksum\": \"{}\",\n",
            json_escape(&ver.cksum)
        ));
        out.push_str(&format!("      \"yanked\": {}\n", ver.yanked));
        out.push_str("    }");
        if i + 1 < entry.versions.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str("  ]\n");
    out.push('}');
    out
}

/// Resolves all dependencies from a dependency map.
///
/// Returns a map of package name -> resolved version, or an error
/// if any dependency cannot be resolved.
pub fn resolve_dependencies(
    registry: &Registry,
    deps: &HashMap<String, String>,
) -> Result<HashMap<String, SemVer>, String> {
    let mut resolved = HashMap::new();
    for (name, constraint_str) in deps {
        let constraint = VersionConstraint::parse(constraint_str)
            .map_err(|e| format!("invalid constraint for '{name}': {e}"))?;
        let version = registry
            .resolve(name, &constraint)
            .ok_or_else(|| format!("no version of '{name}' satisfies {constraint}"))?;
        resolved.insert(name.clone(), version);
    }
    Ok(resolved)
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── SemVer ──

    #[test]
    fn semver_parse() {
        let v = SemVer::parse("1.2.3").unwrap();
        assert_eq!(v, SemVer::new(1, 2, 3));
    }

    #[test]
    fn semver_parse_invalid() {
        assert!(SemVer::parse("1.2").is_err());
        assert!(SemVer::parse("abc").is_err());
        assert!(SemVer::parse("1.2.3.4").is_err());
    }

    #[test]
    fn semver_ordering() {
        let v1 = SemVer::new(1, 0, 0);
        let v2 = SemVer::new(1, 1, 0);
        let v3 = SemVer::new(2, 0, 0);
        assert!(v1 < v2);
        assert!(v2 < v3);
        assert!(v1 < v3);
    }

    #[test]
    fn semver_display() {
        assert_eq!(format!("{}", SemVer::new(1, 2, 3)), "1.2.3");
    }

    // ── VersionConstraint ──

    #[test]
    fn constraint_exact() {
        let c = VersionConstraint::parse("1.0.0").unwrap();
        assert!(c.matches(&SemVer::new(1, 0, 0)));
        assert!(!c.matches(&SemVer::new(1, 0, 1)));
    }

    #[test]
    fn constraint_exact_with_equals() {
        let c = VersionConstraint::parse("=1.0.0").unwrap();
        assert!(c.matches(&SemVer::new(1, 0, 0)));
        assert!(!c.matches(&SemVer::new(1, 1, 0)));
    }

    #[test]
    fn constraint_caret_major() {
        let c = VersionConstraint::parse("^1.2.3").unwrap();
        assert!(c.matches(&SemVer::new(1, 2, 3)));
        assert!(c.matches(&SemVer::new(1, 3, 0)));
        assert!(c.matches(&SemVer::new(1, 99, 99)));
        assert!(!c.matches(&SemVer::new(2, 0, 0)));
        assert!(!c.matches(&SemVer::new(1, 2, 2))); // below
    }

    #[test]
    fn constraint_caret_zero_major() {
        let c = VersionConstraint::parse("^0.2.3").unwrap();
        assert!(c.matches(&SemVer::new(0, 2, 3)));
        assert!(c.matches(&SemVer::new(0, 2, 9)));
        assert!(!c.matches(&SemVer::new(0, 3, 0)));
    }

    #[test]
    fn constraint_caret_zero_zero() {
        let c = VersionConstraint::parse("^0.0.5").unwrap();
        assert!(c.matches(&SemVer::new(0, 0, 5)));
        assert!(!c.matches(&SemVer::new(0, 0, 6)));
    }

    #[test]
    fn constraint_tilde() {
        let c = VersionConstraint::parse("~1.2.3").unwrap();
        assert!(c.matches(&SemVer::new(1, 2, 3)));
        assert!(c.matches(&SemVer::new(1, 2, 9)));
        assert!(!c.matches(&SemVer::new(1, 3, 0)));
        assert!(!c.matches(&SemVer::new(1, 2, 2)));
    }

    #[test]
    fn constraint_gte() {
        let c = VersionConstraint::parse(">=1.5.0").unwrap();
        assert!(c.matches(&SemVer::new(1, 5, 0)));
        assert!(c.matches(&SemVer::new(2, 0, 0)));
        assert!(!c.matches(&SemVer::new(1, 4, 9)));
    }

    #[test]
    fn constraint_lt() {
        let c = VersionConstraint::parse("<2.0.0").unwrap();
        assert!(c.matches(&SemVer::new(1, 99, 99)));
        assert!(!c.matches(&SemVer::new(2, 0, 0)));
    }

    #[test]
    fn constraint_any() {
        let c = VersionConstraint::parse("*").unwrap();
        assert!(c.matches(&SemVer::new(0, 0, 0)));
        assert!(c.matches(&SemVer::new(99, 99, 99)));
    }

    #[test]
    fn constraint_parse_invalid() {
        assert!(VersionConstraint::parse(">>1.0.0").is_err());
    }

    // ── Registry ──

    #[test]
    fn registry_publish_and_lookup() {
        let mut reg = Registry::new();
        reg.publish("fj-math", SemVer::new(1, 0, 0), "Math library");
        reg.publish("fj-math", SemVer::new(1, 1, 0), "Math library");

        let entry = reg.lookup("fj-math").unwrap();
        assert_eq!(entry.versions.len(), 2);
        assert_eq!(entry.versions[0], SemVer::new(1, 0, 0));
        assert_eq!(entry.versions[1], SemVer::new(1, 1, 0));
    }

    #[test]
    fn registry_resolve_caret() {
        let mut reg = Registry::new();
        reg.publish("fj-nn", SemVer::new(0, 1, 0), "Neural nets");
        reg.publish("fj-nn", SemVer::new(0, 2, 0), "Neural nets");
        reg.publish("fj-nn", SemVer::new(1, 0, 0), "Neural nets");

        let constraint = VersionConstraint::parse("^0.2.0").unwrap();
        let resolved = reg.resolve("fj-nn", &constraint).unwrap();
        assert_eq!(resolved, SemVer::new(0, 2, 0));
    }

    #[test]
    fn registry_resolve_highest_match() {
        let mut reg = Registry::new();
        reg.publish("fj-hal", SemVer::new(1, 0, 0), "HAL");
        reg.publish("fj-hal", SemVer::new(1, 1, 0), "HAL");
        reg.publish("fj-hal", SemVer::new(1, 2, 0), "HAL");
        reg.publish("fj-hal", SemVer::new(2, 0, 0), "HAL");

        let constraint = VersionConstraint::parse("^1.0.0").unwrap();
        let resolved = reg.resolve("fj-hal", &constraint).unwrap();
        assert_eq!(resolved, SemVer::new(1, 2, 0)); // highest 1.x
    }

    #[test]
    fn registry_resolve_not_found() {
        let reg = Registry::new();
        let constraint = VersionConstraint::parse("^1.0.0").unwrap();
        assert!(reg.resolve("nonexistent", &constraint).is_none());
    }

    #[test]
    fn registry_no_matching_version() {
        let mut reg = Registry::new();
        reg.publish("lib", SemVer::new(0, 1, 0), "A lib");

        let constraint = VersionConstraint::parse("^1.0.0").unwrap();
        assert!(reg.resolve("lib", &constraint).is_none());
    }

    // ── Dependency resolution ──

    #[test]
    fn resolve_dependencies_basic() {
        let mut reg = Registry::new();
        reg.publish("fj-math", SemVer::new(1, 0, 0), "Math");
        reg.publish("fj-math", SemVer::new(1, 1, 0), "Math");
        reg.publish("fj-nn", SemVer::new(0, 3, 0), "Neural nets");

        let mut deps = HashMap::new();
        deps.insert("fj-math".into(), "^1.0.0".into());
        deps.insert("fj-nn".into(), "0.3.0".into());

        let resolved = resolve_dependencies(&reg, &deps).unwrap();
        assert_eq!(resolved["fj-math"], SemVer::new(1, 1, 0));
        assert_eq!(resolved["fj-nn"], SemVer::new(0, 3, 0));
    }

    #[test]
    fn resolve_dependencies_missing() {
        let reg = Registry::new();
        let mut deps = HashMap::new();
        deps.insert("missing".into(), "^1.0.0".into());

        let result = resolve_dependencies(&reg, &deps);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("no version of 'missing'"));
    }

    #[test]
    fn resolve_dependencies_invalid_constraint() {
        let reg = Registry::new();
        let mut deps = HashMap::new();
        deps.insert("pkg".into(), "not-a-version".into());

        let result = resolve_dependencies(&reg, &deps);
        assert!(result.is_err());
    }

    // ── fj.toml with dependencies ──

    #[test]
    fn parse_manifest_with_dependencies() {
        let toml = r#"
[package]
name = "my-project"
version = "0.1.0"

[dependencies]
fj-math = "^1.0.0"
fj-nn = "0.3.0"
"#;
        let config = super::super::manifest::ProjectConfig::parse(toml).unwrap();
        assert_eq!(config.dependencies.len(), 2);
        assert_eq!(config.dependencies["fj-math"], "^1.0.0");
        assert_eq!(config.dependencies["fj-nn"], "0.3.0");
    }

    // ── Search ──

    #[test]
    fn registry_search_by_prefix() {
        let mut reg = Registry::new();
        reg.publish("fj-math", SemVer::new(1, 0, 0), "Math library");
        reg.publish("fj-nn", SemVer::new(0, 1, 0), "Neural nets");
        reg.publish("fj-math-extra", SemVer::new(0, 1, 0), "Math extras");
        reg.publish("other-lib", SemVer::new(1, 0, 0), "Other");

        let results = reg.search("fj-math");
        assert_eq!(results.len(), 2);
        // Prefix matches first
        assert_eq!(results[0].name, "fj-math");
        assert_eq!(results[1].name, "fj-math-extra");
    }

    #[test]
    fn registry_search_substring() {
        let mut reg = Registry::new();
        reg.publish("fj-nn", SemVer::new(0, 1, 0), "Neural nets");
        reg.publish("my-nn-lib", SemVer::new(1, 0, 0), "Another NN");

        let results = reg.search("nn");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn registry_search_no_match() {
        let mut reg = Registry::new();
        reg.publish("fj-math", SemVer::new(1, 0, 0), "Math");

        let results = reg.search("nonexistent");
        assert!(results.is_empty());
    }

    #[test]
    fn registry_list_all() {
        let mut reg = Registry::new();
        reg.publish("fj-nn", SemVer::new(0, 1, 0), "NN");
        reg.publish("fj-hal", SemVer::new(0, 1, 0), "HAL");
        reg.publish("fj-math", SemVer::new(1, 0, 0), "Math");

        let all = reg.list_all();
        assert_eq!(all.len(), 3);
        assert_eq!(all[0].name, "fj-hal");
        assert_eq!(all[1].name, "fj-math");
        assert_eq!(all[2].name, "fj-nn");
    }

    #[test]
    fn registry_latest_version() {
        let mut reg = Registry::new();
        reg.publish("fj-math", SemVer::new(1, 0, 0), "Math");
        reg.publish("fj-math", SemVer::new(1, 1, 0), "Math");
        reg.publish("fj-math", SemVer::new(2, 0, 0), "Math");

        let latest = reg.latest_version("fj-math").unwrap();
        assert_eq!(*latest, SemVer::new(2, 0, 0));
        assert!(reg.latest_version("nonexistent").is_none());
    }

    #[test]
    fn parse_manifest_no_dependencies() {
        let toml = r#"
[package]
name = "simple"
"#;
        let config = super::super::manifest::ProjectConfig::parse(toml).unwrap();
        assert!(config.dependencies.is_empty());
    }

    // ── Sprint 15: Yank ──

    #[test]
    fn yank_version_skipped_in_resolve() {
        let mut reg = Registry::new();
        reg.publish("fj-math", SemVer::new(1, 0, 0), "Math");
        reg.publish("fj-math", SemVer::new(1, 1, 0), "Math");
        reg.publish("fj-math", SemVer::new(1, 2, 0), "Math");

        // Yank the highest version
        reg.yank("fj-math", &SemVer::new(1, 2, 0)).unwrap();

        let constraint = VersionConstraint::parse("^1.0.0").unwrap();
        let resolved = reg.resolve("fj-math", &constraint).unwrap();
        // Should skip 1.2.0 (yanked) and return 1.1.0
        assert_eq!(resolved, SemVer::new(1, 1, 0));
    }

    #[test]
    fn yank_nonexistent_package_errors() {
        let mut reg = Registry::new();
        let result = reg.yank("nonexistent", &SemVer::new(1, 0, 0));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[test]
    fn yank_nonexistent_version_errors() {
        let mut reg = Registry::new();
        reg.publish("fj-math", SemVer::new(1, 0, 0), "Math");
        let result = reg.yank("fj-math", &SemVer::new(9, 9, 9));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[test]
    fn is_yanked_check() {
        let mut reg = Registry::new();
        reg.publish("fj-nn", SemVer::new(0, 1, 0), "NN");
        assert!(!reg.is_yanked("fj-nn", &SemVer::new(0, 1, 0)));

        reg.yank("fj-nn", &SemVer::new(0, 1, 0)).unwrap();
        assert!(reg.is_yanked("fj-nn", &SemVer::new(0, 1, 0)));
    }

    // ── Sprint 15: Auth tokens ──

    #[test]
    fn validate_unscoped_token() {
        let mut reg = Registry::new();
        reg.add_token(AuthToken::new("secret-token-123"));

        assert!(reg.validate_token("secret-token-123", None));
        assert!(reg.validate_token("secret-token-123", Some("fj-math")));
        assert!(!reg.validate_token("wrong-token", None));
    }

    #[test]
    fn validate_scoped_token() {
        let mut reg = Registry::new();
        reg.add_token(AuthToken::scoped("scoped-token", "fj-math"));

        // Valid for the scoped package
        assert!(reg.validate_token("scoped-token", Some("fj-math")));
        // Not valid for a different package
        assert!(!reg.validate_token("scoped-token", Some("fj-nn")));
        // Valid for general auth check (no package specified)
        assert!(reg.validate_token("scoped-token", None));
    }

    // ── Sprint 15: Registry config ──

    #[test]
    fn registry_config_default() {
        let config = RegistryConfig::default();
        assert!(config.registry_url.contains("fajarlang"));
        assert!(config.api_url.contains("api/v1"));
        assert!(config.auth_required);
    }

    // ── Sprint 15: Sparse index ──

    #[test]
    fn sparse_index_generation() {
        let mut reg = Registry::new();
        let mut deps = HashMap::new();
        deps.insert("fj-math".to_string(), "^1.0.0".to_string());
        reg.publish_with_meta(
            "fj-nn",
            SemVer::new(0, 1, 0),
            "Neural nets",
            deps,
            "abc123def456",
        );
        reg.publish("fj-nn", SemVer::new(0, 2, 0), "Neural nets");
        reg.yank("fj-nn", &SemVer::new(0, 1, 0)).unwrap();

        let index = reg.to_sparse_index("fj-nn").unwrap();
        assert_eq!(index.name, "fj-nn");
        assert_eq!(index.versions.len(), 2);

        // First version: yanked, has deps and checksum
        assert_eq!(index.versions[0].vers, "0.1.0");
        assert!(index.versions[0].yanked);
        assert_eq!(index.versions[0].deps.len(), 1);
        assert_eq!(index.versions[0].deps[0].name, "fj-math");
        assert_eq!(index.versions[0].cksum, "abc123def456");

        // Second version: not yanked, no deps or checksum
        assert_eq!(index.versions[1].vers, "0.2.0");
        assert!(!index.versions[1].yanked);
        assert!(index.versions[1].deps.is_empty());
    }

    #[test]
    fn sparse_index_json_output() {
        let mut reg = Registry::new();
        reg.publish("fj-math", SemVer::new(1, 0, 0), "Math");

        let json = reg.sparse_index_json("fj-math").unwrap();
        assert!(json.contains("\"name\": \"fj-math\""));
        assert!(json.contains("\"vers\": \"1.0.0\""));
        assert!(reg.sparse_index_json("nonexistent").is_none());
    }

    // ── Sprint 15: Download count ──

    #[test]
    fn download_count_increments_on_resolve_and_count() {
        let mut reg = Registry::new();
        reg.publish("fj-math", SemVer::new(1, 0, 0), "Math");

        assert_eq!(reg.download_count("fj-math"), Some(0));

        let constraint = VersionConstraint::parse("^1.0.0").unwrap();
        reg.resolve_and_count("fj-math", &constraint);
        assert_eq!(reg.download_count("fj-math"), Some(1));

        reg.resolve_and_count("fj-math", &constraint);
        reg.resolve_and_count("fj-math", &constraint);
        assert_eq!(reg.download_count("fj-math"), Some(3));
    }

    #[test]
    fn search_shows_download_count() {
        let mut reg = Registry::new();
        reg.publish("fj-math", SemVer::new(1, 0, 0), "Math");

        let constraint = VersionConstraint::parse("^1.0.0").unwrap();
        reg.resolve_and_count("fj-math", &constraint);
        reg.resolve_and_count("fj-math", &constraint);

        let results = reg.search("fj-math");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].download_count, 2);
    }
}
