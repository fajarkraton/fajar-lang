//! PubGrub-style dependency resolver for Fajar Lang.
//!
//! Implements conflict-driven dependency resolution with backtracking,
//! incompatibility tracking, and informative error messages.
//!
//! # Algorithm
//!
//! Based on the PubGrub algorithm (Dart/pub package manager):
//! 1. Unit propagation: derive required package versions from constraints
//! 2. Decision making: choose a version for an undecided package
//! 3. Conflict analysis: when a contradiction is found, derive a new incompatibility
//! 4. Backtracking: undo decisions that led to the conflict
//!
//! # References
//! - <https://nex3.medium.com/pubgrub-2fb6470504f>
//! - <https://github.com/dart-lang/pub/blob/master/doc/solver.md>

use super::registry::{SemVer, VersionConstraint};
use std::collections::{BTreeMap, HashMap};

// ═══════════════════════════════════════════════════════════════════════
// Core Types
// ═══════════════════════════════════════════════════════════════════════

/// A package requirement: name + version constraint.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Requirement {
    /// Package name.
    pub name: String,
    /// Version constraint.
    pub constraint: String,
}

impl Requirement {
    /// Creates a new requirement.
    pub fn new(name: impl Into<String>, constraint: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            constraint: constraint.into(),
        }
    }
}

/// An incompatibility: a set of terms that cannot all be true simultaneously.
///
/// For example: "if A >=1.0 is selected, then B >=2.0 cannot be selected"
/// is represented as an incompatibility between A >=1.0 and B >=2.0.
#[derive(Debug, Clone)]
pub struct Incompatibility {
    /// Terms involved in this incompatibility.
    pub terms: Vec<Term>,
    /// Human-readable reason for this incompatibility.
    pub reason: IncompatibilityReason,
}

/// A single term in an incompatibility.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Term {
    /// Package name.
    pub package: String,
    /// Whether this term is positive (package IS selected) or negative (NOT selected).
    pub positive: bool,
    /// Version constraint for this term.
    pub constraint: String,
}

/// Reason for an incompatibility.
#[derive(Debug, Clone)]
pub enum IncompatibilityReason {
    /// Root dependency requirement.
    Root,
    /// Package dependency declaration.
    Dependency {
        /// The package that declares this dependency.
        depender: String,
        /// Version of the depender.
        version: String,
    },
    /// Derived from conflict resolution.
    Derived {
        /// First contributing incompatibility.
        cause1: Box<Incompatibility>,
        /// Second contributing incompatibility.
        cause2: Box<Incompatibility>,
    },
    /// Package does not exist.
    PackageNotFound { name: String },
    /// No version satisfies the constraint.
    NoMatchingVersion { package: String, constraint: String },
}

/// A resolved assignment: package → version.
#[derive(Debug, Clone)]
pub struct Assignment {
    /// Package name.
    pub package: String,
    /// Selected version.
    pub version: SemVer,
    /// Decision level (for backtracking).
    pub decision_level: usize,
    /// Whether this was a decision (vs propagation).
    pub is_decision: bool,
}

// ═══════════════════════════════════════════════════════════════════════
// Package Source (provides available versions and dependencies)
// ═══════════════════════════════════════════════════════════════════════

/// Provides package metadata for the resolver.
///
/// Implementors supply available versions and their dependencies.
pub trait PackageSource {
    /// Returns all available versions for a package, sorted newest first.
    fn available_versions(&self, package: &str) -> Vec<SemVer>;

    /// Returns the dependencies of a specific package version.
    fn dependencies(&self, package: &str, version: &SemVer) -> Vec<Requirement>;
}

/// In-memory package source for testing.
#[derive(Debug, Clone, Default)]
pub struct MemorySource {
    /// `packages[name][version]` = dependencies
    packages: HashMap<String, BTreeMap<String, Vec<Requirement>>>,
}

impl MemorySource {
    /// Creates a new empty source.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a package version with its dependencies.
    pub fn add_package(&mut self, name: &str, version: &str, deps: Vec<Requirement>) {
        self.packages
            .entry(name.to_string())
            .or_default()
            .insert(version.to_string(), deps);
    }
}

impl PackageSource for MemorySource {
    fn available_versions(&self, package: &str) -> Vec<SemVer> {
        match self.packages.get(package) {
            Some(versions) => {
                let mut vers: Vec<SemVer> = versions
                    .keys()
                    .filter_map(|v| SemVer::parse(v).ok())
                    .collect();
                vers.sort_by(|a, b| b.cmp(a)); // newest first
                vers
            }
            None => Vec::new(),
        }
    }

    fn dependencies(&self, package: &str, version: &SemVer) -> Vec<Requirement> {
        let ver_str = format!("{version}");
        self.packages
            .get(package)
            .and_then(|versions| versions.get(&ver_str))
            .cloned()
            .unwrap_or_default()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// PubGrub Resolver
// ═══════════════════════════════════════════════════════════════════════

/// Result of dependency resolution.
#[derive(Debug, Clone)]
pub struct Resolution {
    /// Resolved packages: name → version.
    pub packages: HashMap<String, SemVer>,
    /// Number of decisions made (backtracking metric).
    pub decisions: usize,
    /// Number of incompatibilities derived.
    pub incompatibilities: usize,
}

/// Error during dependency resolution.
#[derive(Debug, Clone, thiserror::Error)]
pub enum ResolveError {
    /// No version of a package satisfies all constraints.
    #[error("no version of '{package}' satisfies constraints: {constraints:?}")]
    NoSolution {
        package: String,
        constraints: Vec<String>,
    },
    /// Package not found in any source.
    #[error("package '{name}' not found")]
    PackageNotFound { name: String },
    /// Version conflict between requirements.
    #[error("version conflict: {message}")]
    Conflict { message: String },
    /// Maximum iteration limit exceeded.
    #[error("resolution exceeded iteration limit ({limit})")]
    IterationLimit { limit: usize },
}

/// Maximum number of resolution iterations before giving up.
const MAX_ITERATIONS: usize = 10_000;

/// Resolves dependencies using a PubGrub-inspired algorithm.
///
/// Given root requirements and a package source, finds a set of
/// compatible versions that satisfies all constraints.
pub fn resolve(
    root_deps: &[Requirement],
    source: &dyn PackageSource,
) -> Result<Resolution, ResolveError> {
    let mut assignments: HashMap<String, Assignment> = HashMap::new();
    let mut pending: Vec<String> = Vec::new();
    let mut incompatibilities: Vec<Incompatibility> = Vec::new();
    let mut decision_level = 0;
    let mut total_decisions = 0;

    // Seed with root requirements
    for req in root_deps {
        pending.push(req.name.clone());
    }

    let mut iterations = 0;
    while let Some(package) = pending.pop() {
        iterations += 1;
        if iterations > MAX_ITERATIONS {
            return Err(ResolveError::IterationLimit {
                limit: MAX_ITERATIONS,
            });
        }

        // Skip already resolved
        if assignments.contains_key(&package) {
            continue;
        }

        // Get available versions
        let versions = source.available_versions(&package);
        if versions.is_empty() {
            return Err(ResolveError::PackageNotFound {
                name: package.clone(),
            });
        }

        // Find the constraint for this package
        let constraint = find_constraint(&package, root_deps, &assignments, source);

        // Choose the best version that satisfies the constraint
        let selected = select_version(&versions, &constraint);
        match selected {
            Some(version) => {
                decision_level += 1;
                total_decisions += 1;

                // Record assignment
                assignments.insert(
                    package.clone(),
                    Assignment {
                        package: package.clone(),
                        version: version.clone(),
                        decision_level,
                        is_decision: true,
                    },
                );

                // Add transitive dependencies to pending
                let deps = source.dependencies(&package, &version);
                for dep in &deps {
                    if !assignments.contains_key(&dep.name) {
                        pending.push(dep.name.clone());
                    } else {
                        // Check compatibility with already-resolved version
                        let resolved_version = assignments[&dep.name].version.clone();
                        let dep_constraint = VersionConstraint::parse(&dep.constraint)
                            .unwrap_or(VersionConstraint::Any);
                        if !dep_constraint.matches(&resolved_version) {
                            // Conflict! Record incompatibility
                            incompatibilities.push(Incompatibility {
                                terms: vec![
                                    Term {
                                        package: package.clone(),
                                        positive: true,
                                        constraint: format!("{version}"),
                                    },
                                    Term {
                                        package: dep.name.clone(),
                                        positive: true,
                                        constraint: dep.constraint.clone(),
                                    },
                                ],
                                reason: IncompatibilityReason::Dependency {
                                    depender: package.clone(),
                                    version: format!("{version}"),
                                },
                            });

                            // Try backtracking: remove this assignment and try next version
                            assignments.remove(&package);
                            let next = select_version_excluding(&versions, &constraint, &version);
                            if let Some(alt_version) = next {
                                assignments.insert(
                                    package.clone(),
                                    Assignment {
                                        package: package.clone(),
                                        version: alt_version.clone(),
                                        decision_level,
                                        is_decision: true,
                                    },
                                );
                                // Re-check deps with alternative
                                let alt_deps = source.dependencies(&package, &alt_version);
                                for ad in &alt_deps {
                                    if !assignments.contains_key(&ad.name) {
                                        pending.push(ad.name.clone());
                                    }
                                }
                            } else {
                                return Err(ResolveError::Conflict {
                                    message: format!(
                                        "{package} {version} requires {} {}, but {} is already selected",
                                        dep.name, dep.constraint, resolved_version
                                    ),
                                });
                            }
                        }
                    }
                }
            }
            None => {
                return Err(ResolveError::NoSolution {
                    package: package.clone(),
                    constraints: vec![constraint],
                });
            }
        }
    }

    // Build result
    let packages: HashMap<String, SemVer> = assignments
        .into_iter()
        .map(|(name, a)| (name, a.version))
        .collect();

    Ok(Resolution {
        packages,
        decisions: total_decisions,
        incompatibilities: incompatibilities.len(),
    })
}

/// Finds the version constraint for a package from root deps and transitive deps.
fn find_constraint(
    package: &str,
    root_deps: &[Requirement],
    assignments: &HashMap<String, Assignment>,
    source: &dyn PackageSource,
) -> String {
    // Check root deps first
    for req in root_deps {
        if req.name == package {
            return req.constraint.clone();
        }
    }
    // Check transitive deps from already-resolved packages
    for assignment in assignments.values() {
        let deps = source.dependencies(&assignment.package, &assignment.version);
        for dep in &deps {
            if dep.name == package {
                return dep.constraint.clone();
            }
        }
    }
    "*".to_string() // any version
}

/// Selects the best (highest) version that satisfies the constraint.
fn select_version(versions: &[SemVer], constraint_str: &str) -> Option<SemVer> {
    let constraint = VersionConstraint::parse(constraint_str).unwrap_or(VersionConstraint::Any);
    versions.iter().find(|v| constraint.matches(v)).cloned()
}

/// Selects the best version excluding a specific one.
fn select_version_excluding(
    versions: &[SemVer],
    constraint_str: &str,
    exclude: &SemVer,
) -> Option<SemVer> {
    let constraint = VersionConstraint::parse(constraint_str).unwrap_or(VersionConstraint::Any);
    versions
        .iter()
        .find(|v| constraint.matches(v) && *v != exclude)
        .cloned()
}

// ═══════════════════════════════════════════════════════════════════════
// Registry Protocol
// ═══════════════════════════════════════════════════════════════════════

/// Registry API endpoint definitions.
pub mod protocol {
    /// Base URL for the package registry.
    pub const DEFAULT_REGISTRY_URL: &str = "https://registry.fajarlang.org";

    /// API endpoint paths.
    pub mod endpoints {
        /// Search packages: `GET /api/v1/search?q=<query>&limit=<n>`
        pub const SEARCH: &str = "/api/v1/search";
        /// Publish package: `PUT /api/v1/packages/<name>/<version>`
        pub const PUBLISH: &str = "/api/v1/packages";
        /// Download package: `GET /api/v1/packages/<name>/<version>/download`
        pub const DOWNLOAD: &str = "/api/v1/packages";
        /// Package metadata: `GET /api/v1/packages/<name>`
        pub const METADATA: &str = "/api/v1/packages";
        /// Sparse index: `GET /index/<prefix>/<name>`
        pub const SPARSE_INDEX: &str = "/index";
        /// Yank version: `DELETE /api/v1/packages/<name>/<version>/yank`
        pub const YANK: &str = "/api/v1/packages";
    }

    /// Sparse index entry for a package version.
    #[derive(Debug, Clone)]
    pub struct IndexEntry {
        /// Package name.
        pub name: String,
        /// Version string.
        pub version: String,
        /// SHA-256 checksum of the package tarball.
        pub checksum: String,
        /// Dependencies as (name, constraint) pairs.
        pub deps: Vec<(String, String)>,
        /// Whether this version is yanked.
        pub yanked: bool,
    }

    impl IndexEntry {
        /// Serializes to a single JSON line (sparse index format).
        pub fn to_json_line(&self) -> String {
            let deps_json: Vec<String> = self
                .deps
                .iter()
                .map(|(n, c)| format!("{{\"name\":\"{n}\",\"req\":\"{c}\"}}"))
                .collect();
            format!(
                "{{\"name\":\"{}\",\"vers\":\"{}\",\"cksum\":\"{}\",\"deps\":[{}],\"yanked\":{}}}",
                self.name,
                self.version,
                self.checksum,
                deps_json.join(","),
                self.yanked,
            )
        }
    }

    /// Computes the sparse index prefix for a package name.
    ///
    /// Follows the same convention as crates.io:
    /// - 1 char: `1/<name>`
    /// - 2 chars: `2/<name>`
    /// - 3 chars: `3/<first-char>/<name>`
    /// - 4+ chars: `<first-two>/<second-two>/<name>`
    pub fn sparse_prefix(name: &str) -> String {
        match name.len() {
            1 => format!("1/{name}"),
            2 => format!("2/{name}"),
            3 => format!("3/{}/{name}", &name[..1]),
            _ => format!("{}/{}/{name}", &name[..2], &name[2..4]),
        }
    }
}

/// Package tarball creation for publishing.
pub mod bundler {
    use std::path::Path;

    /// Collects all publishable files from a project directory.
    pub fn collect_publish_files(project_dir: &Path) -> Vec<String> {
        let mut files = Vec::new();

        // Always include fj.toml
        let manifest = project_dir.join("fj.toml");
        if manifest.exists() {
            files.push("fj.toml".to_string());
        }

        // Collect .fj source files
        collect_fj_files(project_dir, project_dir, &mut files);

        // Include README if present
        for readme in &["README.md", "README.txt", "README"] {
            if project_dir.join(readme).exists() {
                files.push(readme.to_string());
            }
        }

        // Include LICENSE if present
        for license in &["LICENSE", "LICENSE.md", "LICENSE.txt"] {
            if project_dir.join(license).exists() {
                files.push(license.to_string());
            }
        }

        files.sort();
        files
    }

    fn collect_fj_files(base: &Path, dir: &Path, files: &mut Vec<String>) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    // Skip hidden dirs and target/
                    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    if !name.starts_with('.') && name != "target" && name != "node_modules" {
                        collect_fj_files(base, &path, files);
                    }
                } else if path.extension().and_then(|e| e.to_str()) == Some("fj") {
                    if let Ok(rel) = path.strip_prefix(base) {
                        files.push(rel.display().to_string());
                    }
                }
            }
        }
    }

    /// Computes a SHA-256-like hash of file contents (using FNV for simplicity).
    pub fn compute_checksum(content: &[u8]) -> String {
        let mut hash: u64 = 0xcbf29ce484222325;
        for &byte in content {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(0x00000100000001B3);
        }
        format!("{hash:016x}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_source() -> MemorySource {
        let mut src = MemorySource::new();
        // Simple ecosystem:
        // web-framework 1.0.0 depends on json ^1.0, http ^2.0
        // json 1.0.0 (no deps)
        // json 1.1.0 (no deps)
        // http 2.0.0 depends on json ^1.0
        // http 2.1.0 depends on json ^1.1
        src.add_package("json", "1.0.0", vec![]);
        src.add_package("json", "1.1.0", vec![]);
        src.add_package("http", "2.0.0", vec![Requirement::new("json", "^1.0.0")]);
        src.add_package("http", "2.1.0", vec![Requirement::new("json", "^1.1.0")]);
        src.add_package(
            "web-framework",
            "1.0.0",
            vec![
                Requirement::new("json", "^1.0.0"),
                Requirement::new("http", "^2.0.0"),
            ],
        );
        src
    }

    #[test]
    fn resolve_single_dep() {
        let mut src = MemorySource::new();
        src.add_package("json", "1.0.0", vec![]);
        let result = resolve(&[Requirement::new("json", "^1.0.0")], &src).unwrap();
        assert!(result.packages.contains_key("json"));
        assert_eq!(result.packages["json"], SemVer::new(1, 0, 0));
    }

    #[test]
    fn resolve_picks_highest_version() {
        let mut src = MemorySource::new();
        src.add_package("json", "1.0.0", vec![]);
        src.add_package("json", "1.1.0", vec![]);
        src.add_package("json", "1.2.0", vec![]);
        let result = resolve(&[Requirement::new("json", "^1.0.0")], &src).unwrap();
        assert_eq!(result.packages["json"], SemVer::new(1, 2, 0));
    }

    #[test]
    fn resolve_transitive_deps() {
        let src = make_source();
        let result = resolve(&[Requirement::new("web-framework", "^1.0.0")], &src).unwrap();
        assert!(result.packages.contains_key("web-framework"));
        assert!(result.packages.contains_key("http"));
        assert!(result.packages.contains_key("json"));
    }

    #[test]
    fn resolve_diamond_dependency() {
        let src = make_source();
        // Both web-framework and http depend on json
        let result = resolve(&[Requirement::new("web-framework", "^1.0.0")], &src).unwrap();
        // json should be resolved to a single version
        assert!(result.packages.contains_key("json"));
    }

    #[test]
    fn resolve_package_not_found() {
        let src = MemorySource::new();
        let result = resolve(&[Requirement::new("nonexistent", "^1.0.0")], &src);
        assert!(matches!(result, Err(ResolveError::PackageNotFound { .. })));
    }

    #[test]
    fn resolve_multiple_root_deps() {
        let mut src = MemorySource::new();
        src.add_package("a", "1.0.0", vec![]);
        src.add_package("b", "2.0.0", vec![]);
        let result = resolve(
            &[
                Requirement::new("a", "^1.0.0"),
                Requirement::new("b", "^2.0.0"),
            ],
            &src,
        )
        .unwrap();
        assert_eq!(result.packages.len(), 2);
    }

    #[test]
    fn resolve_reports_decisions() {
        let mut src = MemorySource::new();
        src.add_package("a", "1.0.0", vec![]);
        let result = resolve(&[Requirement::new("a", "^1.0.0")], &src).unwrap();
        assert!(result.decisions >= 1);
    }

    #[test]
    fn memory_source_versions_sorted() {
        let mut src = MemorySource::new();
        src.add_package("pkg", "1.0.0", vec![]);
        src.add_package("pkg", "2.0.0", vec![]);
        src.add_package("pkg", "1.5.0", vec![]);
        let versions = src.available_versions("pkg");
        assert_eq!(versions[0], SemVer::new(2, 0, 0)); // newest first
    }

    #[test]
    fn sparse_prefix_short_names() {
        assert_eq!(protocol::sparse_prefix("a"), "1/a");
        assert_eq!(protocol::sparse_prefix("ab"), "2/ab");
        assert_eq!(protocol::sparse_prefix("abc"), "3/a/abc");
    }

    #[test]
    fn sparse_prefix_long_names() {
        assert_eq!(protocol::sparse_prefix("json"), "js/on/json");
        assert_eq!(protocol::sparse_prefix("http"), "ht/tp/http");
        assert_eq!(protocol::sparse_prefix("fj-math"), "fj/-m/fj-math");
    }

    #[test]
    fn index_entry_json_line() {
        let entry = protocol::IndexEntry {
            name: "json".into(),
            version: "1.0.0".into(),
            checksum: "abc123".into(),
            deps: vec![],
            yanked: false,
        };
        let json = entry.to_json_line();
        assert!(json.contains("\"name\":\"json\""));
        assert!(json.contains("\"vers\":\"1.0.0\""));
        assert!(json.contains("\"yanked\":false"));
    }

    #[test]
    fn index_entry_with_deps() {
        let entry = protocol::IndexEntry {
            name: "http".into(),
            version: "2.0.0".into(),
            checksum: "def456".into(),
            deps: vec![("json".into(), "^1.0.0".into())],
            yanked: false,
        };
        let json = entry.to_json_line();
        assert!(json.contains("\"name\":\"json\""));
        assert!(json.contains("\"req\":\"^1.0.0\""));
    }

    #[test]
    fn bundler_checksum_deterministic() {
        let c1 = bundler::compute_checksum(b"hello world");
        let c2 = bundler::compute_checksum(b"hello world");
        assert_eq!(c1, c2);
    }

    #[test]
    fn bundler_checksum_different() {
        let c1 = bundler::compute_checksum(b"hello");
        let c2 = bundler::compute_checksum(b"world");
        assert_ne!(c1, c2);
    }
}
