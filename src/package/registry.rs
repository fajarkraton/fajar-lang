//! Package registry — semver parsing, version constraints, and resolution.
//!
//! Provides a local file-based registry for Fajar Lang packages.

use std::cmp::Ordering;
use std::collections::HashMap;
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
}

/// A simple in-memory package registry.
#[derive(Debug, Default)]
pub struct Registry {
    /// Registered packages.
    packages: HashMap<String, PackageEntry>,
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
            });
        if !entry.versions.contains(&version) {
            entry.versions.push(version);
            entry.versions.sort();
        }
    }

    /// Looks up a package by name.
    pub fn lookup(&self, name: &str) -> Option<&PackageEntry> {
        self.packages.get(name)
    }

    /// Resolves the best matching version for a constraint.
    ///
    /// Returns the highest version that satisfies the constraint.
    pub fn resolve(&self, name: &str, constraint: &VersionConstraint) -> Option<SemVer> {
        let entry = self.packages.get(name)?;
        entry
            .versions
            .iter()
            .rev()
            .find(|v| constraint.matches(v))
            .cloned()
    }

    /// Returns the number of registered packages.
    pub fn package_count(&self) -> usize {
        self.packages.len()
    }

    /// Searches packages by name prefix or substring.
    ///
    /// Returns matching entries sorted by name.
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
}

/// Resolves all dependencies from a dependency map.
///
/// Returns a map of package name → resolved version, or an error
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
}
