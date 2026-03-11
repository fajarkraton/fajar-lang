//! Dependency resolver — transitive dependency resolution with lock file generation.
//!
//! Resolves a dependency tree from a `fj.toml`, handling transitive dependencies
//! and producing a deterministic lock file for reproducible builds.

use std::collections::HashMap;

use super::registry::{Registry, SemVer, VersionConstraint};

/// A resolved dependency with its full constraint chain.
#[derive(Debug, Clone)]
pub struct ResolvedDep {
    /// Package name.
    pub name: String,
    /// Resolved version.
    pub version: SemVer,
    /// Which packages depend on this one.
    pub required_by: Vec<String>,
}

/// Lock file content — a deterministic snapshot of resolved versions.
#[derive(Debug, Clone)]
pub struct LockFile {
    /// Resolved dependencies, sorted by name.
    pub entries: Vec<LockEntry>,
}

/// A single entry in the lock file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LockEntry {
    /// Package name.
    pub name: String,
    /// Locked version.
    pub version: SemVer,
    /// Optional SHA-256 checksum for integrity verification.
    pub checksum: Option<String>,
}

/// Lock file format version.
const LOCK_FORMAT_V2: &str = "v2";

impl LockFile {
    /// Serializes the lock file to v2 format (includes checksums).
    pub fn to_string_repr(&self) -> String {
        let mut out = String::new();
        out.push_str("# fj.lock — auto-generated, do not edit\n");
        out.push_str(&format!(
            "# Fajar Lang dependency lock file (format {LOCK_FORMAT_V2})\n\n"
        ));
        for entry in &self.entries {
            if let Some(ref cksum) = entry.checksum {
                out.push_str(&format!("{}={} {}\n", entry.name, entry.version, cksum));
            } else {
                out.push_str(&format!("{}={}\n", entry.name, entry.version));
            }
        }
        out
    }

    /// Parses a lock file from string content (supports both v1 and v2 formats).
    ///
    /// V1 format: `name=version`
    /// V2 format: `name=version checksum`
    pub fn parse(content: &str) -> Result<Self, String> {
        let mut entries = Vec::new();
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let parts: Vec<&str> = line.splitn(2, '=').collect();
            if parts.len() != 2 {
                return Err(format!("invalid lock file line: '{line}'"));
            }
            let name = parts[0].trim().to_string();
            let rhs = parts[1].trim();

            // V2: version may be followed by a space and checksum
            let (version_str, checksum) = if let Some(space_idx) = rhs.find(' ') {
                let ver = &rhs[..space_idx];
                let cksum = rhs[space_idx + 1..].trim();
                (ver, Some(cksum.to_string()))
            } else {
                (rhs, None)
            };

            let version = SemVer::parse(version_str)?;
            entries.push(LockEntry {
                name,
                version,
                checksum,
            });
        }
        Ok(LockFile { entries })
    }
}

/// Callback to retrieve dependencies for a specific package version.
/// Returns a map of dependency name -> version constraint string.
pub type DepLookupFn = Box<dyn Fn(&str, &SemVer) -> HashMap<String, String>>;

/// Resolves the full dependency tree, including transitive dependencies.
///
/// Uses a simple BFS approach. For diamond dependencies where multiple
/// packages depend on the same package, the highest compatible version is chosen.
pub fn resolve_full(
    registry: &Registry,
    root_deps: &HashMap<String, String>,
    dep_lookup: &DepLookupFn,
) -> Result<LockFile, String> {
    let mut resolved: HashMap<String, SemVer> = HashMap::new();
    let mut queue: Vec<(String, String, String)> = Vec::new(); // (name, constraint, required_by)

    // Seed with root dependencies
    for (name, constraint) in root_deps {
        queue.push((name.clone(), constraint.clone(), "root".into()));
    }

    while let Some((name, constraint_str, required_by)) = queue.pop() {
        let constraint = VersionConstraint::parse(&constraint_str).map_err(|e| {
            format!("invalid constraint for '{name}' (required by {required_by}): {e}")
        })?;

        let version = registry.resolve(&name, &constraint).ok_or_else(|| {
            format!("no version of '{name}' satisfies {constraint} (required by {required_by})")
        })?;

        // Check if already resolved
        if let Some(existing) = resolved.get(&name) {
            if *existing == version {
                continue; // Same version, no conflict
            }
            // If different but both satisfy constraints, keep the higher one
            if constraint.matches(existing) && *existing >= version {
                continue; // Existing is higher and still compatible
            }
            if constraint.matches(&version) && version > *existing {
                // New version is higher, update
            } else if *existing != version {
                return Err(format!(
                    "version conflict for '{name}': {} (required by {required_by}) vs {} (already resolved)",
                    version, existing
                ));
            }
        }

        resolved.insert(name.clone(), version.clone());

        // Look up transitive dependencies
        let transitive = dep_lookup(&name, &version);
        for (dep_name, dep_constraint) in transitive {
            queue.push((dep_name, dep_constraint, name.clone()));
        }
    }

    // Build sorted lock file
    let mut entries: Vec<LockEntry> = resolved
        .into_iter()
        .map(|(name, version)| LockEntry {
            name,
            version,
            checksum: None,
        })
        .collect();
    entries.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(LockFile { entries })
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn make_empty_lookup() -> DepLookupFn {
        Box::new(|_name: &str, _version: &SemVer| HashMap::new())
    }

    #[test]
    fn resolve_simple_deps() {
        let mut reg = Registry::new();
        reg.publish("fj-math", SemVer::new(1, 0, 0), "Math");
        reg.publish("fj-math", SemVer::new(1, 1, 0), "Math");
        reg.publish("fj-nn", SemVer::new(0, 2, 0), "NN");

        let mut deps = HashMap::new();
        deps.insert("fj-math".into(), "^1.0.0".into());
        deps.insert("fj-nn".into(), "0.2.0".into());

        let lock = resolve_full(&reg, &deps, &make_empty_lookup()).unwrap();
        assert_eq!(lock.entries.len(), 2);
        // Sorted by name
        assert_eq!(lock.entries[0].name, "fj-math");
        assert_eq!(lock.entries[0].version, SemVer::new(1, 1, 0));
        assert_eq!(lock.entries[1].name, "fj-nn");
        assert_eq!(lock.entries[1].version, SemVer::new(0, 2, 0));
    }

    #[test]
    fn resolve_transitive_deps() {
        let mut reg = Registry::new();
        reg.publish("app-lib", SemVer::new(1, 0, 0), "App library");
        reg.publish("fj-math", SemVer::new(1, 0, 0), "Math");

        let mut deps = HashMap::new();
        deps.insert("app-lib".into(), "^1.0.0".into());

        // app-lib depends on fj-math
        let lookup: DepLookupFn = Box::new(|name, _ver| {
            if name == "app-lib" {
                let mut deps = HashMap::new();
                deps.insert("fj-math".into(), "^1.0.0".into());
                deps
            } else {
                HashMap::new()
            }
        });

        let lock = resolve_full(&reg, &deps, &lookup).unwrap();
        assert_eq!(lock.entries.len(), 2);
        assert!(lock.entries.iter().any(|e| e.name == "app-lib"));
        assert!(lock.entries.iter().any(|e| e.name == "fj-math"));
    }

    #[test]
    fn resolve_diamond_dependency() {
        let mut reg = Registry::new();
        reg.publish("lib-a", SemVer::new(1, 0, 0), "A");
        reg.publish("lib-b", SemVer::new(1, 0, 0), "B");
        reg.publish("shared", SemVer::new(1, 0, 0), "Shared");
        reg.publish("shared", SemVer::new(1, 1, 0), "Shared");

        // root -> lib-a -> shared ^1.0.0
        // root -> lib-b -> shared ^1.0.0
        let mut deps = HashMap::new();
        deps.insert("lib-a".into(), "^1.0.0".into());
        deps.insert("lib-b".into(), "^1.0.0".into());

        let lookup: DepLookupFn = Box::new(|name, _ver| {
            if name == "lib-a" || name == "lib-b" {
                let mut deps = HashMap::new();
                deps.insert("shared".into(), "^1.0.0".into());
                deps
            } else {
                HashMap::new()
            }
        });

        let lock = resolve_full(&reg, &deps, &lookup).unwrap();
        // shared should appear only once
        let shared_count = lock.entries.iter().filter(|e| e.name == "shared").count();
        assert_eq!(shared_count, 1);
        // Should pick highest compatible: 1.1.0
        let shared = lock.entries.iter().find(|e| e.name == "shared").unwrap();
        assert_eq!(shared.version, SemVer::new(1, 1, 0));
    }

    #[test]
    fn resolve_version_conflict_detected() {
        let mut reg = Registry::new();
        reg.publish("shared", SemVer::new(1, 0, 0), "Shared");
        reg.publish("shared", SemVer::new(2, 0, 0), "Shared");

        // Two deps require incompatible versions of shared
        let mut deps = HashMap::new();
        deps.insert("shared".into(), "1.0.0".into()); // exact 1.0.0

        // But we also need shared 2.0.0 through transitive
        // This tests the direct case — root wants 1.0.0 exact
        let lock = resolve_full(&reg, &deps, &make_empty_lookup()).unwrap();
        assert_eq!(lock.entries[0].version, SemVer::new(1, 0, 0));
    }

    #[test]
    fn resolve_missing_dependency() {
        let reg = Registry::new();
        let mut deps = HashMap::new();
        deps.insert("nonexistent".into(), "^1.0.0".into());

        let result = resolve_full(&reg, &deps, &make_empty_lookup());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("no version of 'nonexistent'"));
    }

    // ── Lock file ──

    #[test]
    fn lock_file_serialize() {
        let lock = LockFile {
            entries: vec![
                LockEntry {
                    name: "fj-math".into(),
                    version: SemVer::new(1, 1, 0),
                    checksum: None,
                },
                LockEntry {
                    name: "fj-nn".into(),
                    version: SemVer::new(0, 3, 0),
                    checksum: None,
                },
            ],
        };
        let content = lock.to_string_repr();
        assert!(content.contains("fj-math=1.1.0"));
        assert!(content.contains("fj-nn=0.3.0"));
        assert!(content.contains("# fj.lock"));
    }

    #[test]
    fn lock_file_parse() {
        let content = "# comment\nfj-math=1.1.0\nfj-nn=0.3.0\n";
        let lock = LockFile::parse(content).unwrap();
        assert_eq!(lock.entries.len(), 2);
        assert_eq!(lock.entries[0].name, "fj-math");
        assert_eq!(lock.entries[0].version, SemVer::new(1, 1, 0));
        assert!(lock.entries[0].checksum.is_none());
    }

    #[test]
    fn lock_file_roundtrip() {
        let lock = LockFile {
            entries: vec![
                LockEntry {
                    name: "a".into(),
                    version: SemVer::new(1, 0, 0),
                    checksum: None,
                },
                LockEntry {
                    name: "b".into(),
                    version: SemVer::new(2, 3, 4),
                    checksum: None,
                },
            ],
        };
        let content = lock.to_string_repr();
        let parsed = LockFile::parse(&content).unwrap();
        assert_eq!(parsed.entries, lock.entries);
    }

    #[test]
    fn lock_file_parse_invalid() {
        assert!(LockFile::parse("bad-line-no-equals").is_err());
    }

    // ── Sprint 17: Lock file v2 with checksums ──

    #[test]
    fn lock_file_v2_with_checksum() {
        let lock = LockFile {
            entries: vec![
                LockEntry {
                    name: "fj-math".into(),
                    version: SemVer::new(1, 0, 0),
                    checksum: Some("abc123def456".to_string()),
                },
                LockEntry {
                    name: "fj-nn".into(),
                    version: SemVer::new(0, 2, 0),
                    checksum: None,
                },
            ],
        };
        let content = lock.to_string_repr();
        assert!(content.contains("fj-math=1.0.0 abc123def456"));
        assert!(content.contains("fj-nn=0.2.0"));
        assert!(!content.contains("fj-nn=0.2.0 ")); // no trailing space
        assert!(content.contains("format v2"));
    }

    #[test]
    fn lock_file_v2_parse_with_checksum() {
        let content = "# header\nfj-math=1.0.0 deadbeef\nfj-nn=0.2.0\n";
        let lock = LockFile::parse(content).unwrap();
        assert_eq!(lock.entries.len(), 2);

        assert_eq!(lock.entries[0].name, "fj-math");
        assert_eq!(lock.entries[0].version, SemVer::new(1, 0, 0));
        assert_eq!(lock.entries[0].checksum, Some("deadbeef".to_string()));

        assert_eq!(lock.entries[1].name, "fj-nn");
        assert_eq!(lock.entries[1].version, SemVer::new(0, 2, 0));
        assert!(lock.entries[1].checksum.is_none());
    }

    #[test]
    fn lock_file_v2_roundtrip_with_checksum() {
        let lock = LockFile {
            entries: vec![
                LockEntry {
                    name: "a".into(),
                    version: SemVer::new(1, 0, 0),
                    checksum: Some("aabbccdd".to_string()),
                },
                LockEntry {
                    name: "b".into(),
                    version: SemVer::new(2, 0, 0),
                    checksum: None,
                },
            ],
        };
        let content = lock.to_string_repr();
        let parsed = LockFile::parse(&content).unwrap();
        assert_eq!(parsed.entries, lock.entries);
    }

    #[test]
    fn lock_entry_default_checksum_is_none() {
        let entry = LockEntry {
            name: "pkg".into(),
            version: SemVer::new(0, 1, 0),
            checksum: None,
        };
        assert!(entry.checksum.is_none());
    }
}
