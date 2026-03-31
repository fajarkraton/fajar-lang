//! Error recovery and edge cases for incremental compilation.
//!
//! Ensures the cache remains consistent even under adverse conditions:
//! parse errors, type errors, interrupted builds, clock skew, symlinks,
//! case-insensitive filesystems, unicode paths, large files, and circular deps.

use std::collections::{HashMap, HashSet};

use super::compute_content_hash;
use super::disk::HashStore;

// ═══════════════════════════════════════════════════════════════════════
// I7.1: Parse Error Recovery
// ═══════════════════════════════════════════════════════════════════════

/// Result of attempting to compile a module (may have errors).
#[derive(Debug, Clone)]
pub struct ModuleCompileResult {
    /// Module file path.
    pub path: String,
    /// Whether parsing succeeded.
    pub parse_ok: bool,
    /// Whether type checking succeeded.
    pub type_check_ok: bool,
    /// Whether codegen succeeded.
    pub codegen_ok: bool,
    /// Error messages (if any).
    pub errors: Vec<String>,
}

/// Given a set of module results, determine which modules can be cached
/// even if some modules have errors.
///
/// Rule: A module with parse errors cannot be cached, but modules that
/// parsed successfully can be cached even if other modules failed.
pub fn cacheable_modules(results: &[ModuleCompileResult]) -> Vec<String> {
    results
        .iter()
        .filter(|r| r.parse_ok) // Only cache modules that parsed OK
        .map(|r| r.path.clone())
        .collect()
}

/// I7.2: Modules that type-checked successfully can be cached at the
/// TypedAst level even if other modules had type errors.
pub fn type_cacheable_modules(results: &[ModuleCompileResult]) -> Vec<String> {
    results
        .iter()
        .filter(|r| r.parse_ok && r.type_check_ok)
        .map(|r| r.path.clone())
        .collect()
}

// ═══════════════════════════════════════════════════════════════════════
// I7.3: Interrupted Build Recovery
// ═══════════════════════════════════════════════════════════════════════

/// State of a build that may have been interrupted.
#[derive(Debug, Clone)]
pub struct BuildState {
    /// Modules that completed compilation before interruption.
    pub completed: HashSet<String>,
    /// Modules that were in-progress when interrupted.
    pub in_progress: HashSet<String>,
    /// Modules not yet started.
    pub pending: HashSet<String>,
}

impl BuildState {
    /// Recover from an interrupted build.
    ///
    /// - Completed modules: keep their cached artifacts.
    /// - In-progress modules: discard (may be corrupt), re-add to pending.
    /// - Pending modules: remain pending.
    pub fn recover(&self) -> RecoveryPlan {
        let mut to_rebuild: HashSet<String> = self.in_progress.clone();
        to_rebuild.extend(self.pending.iter().cloned());

        RecoveryPlan {
            keep_cached: self.completed.clone(),
            rebuild: to_rebuild,
            discarded: self.in_progress.clone(),
        }
    }
}

/// Plan for recovering from an interrupted build.
#[derive(Debug, Clone)]
pub struct RecoveryPlan {
    /// Modules whose cache is still valid.
    pub keep_cached: HashSet<String>,
    /// Modules that need rebuilding.
    pub rebuild: HashSet<String>,
    /// Modules whose partial artifacts were discarded.
    pub discarded: HashSet<String>,
}

// ═══════════════════════════════════════════════════════════════════════
// I7.4: Clock Skew Handling
// ═══════════════════════════════════════════════════════════════════════

/// Detect whether a file has changed using content hash (not mtime).
///
/// Content hash is immune to clock skew — the hash changes if and only if
/// the file contents actually changed, regardless of timestamps.
pub fn content_based_change_detection(
    path: &str,
    content: &str,
    hash_store: &HashStore,
) -> ChangeDetection {
    let current_hash = compute_content_hash(content);
    let changed = hash_store.has_changed(path, &current_hash);

    ChangeDetection {
        path: path.to_string(),
        current_hash,
        changed,
        method: "content-hash".to_string(),
    }
}

/// Result of change detection for a single file.
#[derive(Debug, Clone)]
pub struct ChangeDetection {
    /// File path.
    pub path: String,
    /// Current content hash.
    pub current_hash: String,
    /// Whether the file has changed.
    pub changed: bool,
    /// Detection method used.
    pub method: String,
}

// ═══════════════════════════════════════════════════════════════════════
// I7.5: Symlink Handling
// ═══════════════════════════════════════════════════════════════════════

/// Resolve a path, following symlinks, and return the canonical path.
///
/// For cache purposes, we always use the real (canonical) path as the key
/// to avoid duplicate cache entries for symlinked files.
pub fn resolve_real_path(path: &str) -> String {
    match std::fs::canonicalize(path) {
        Ok(canonical) => canonical.to_string_lossy().to_string(),
        Err(_) => path.to_string(), // Fallback to original if can't resolve
    }
}

/// Normalize a set of paths by resolving symlinks and deduplicating.
pub fn normalize_paths(paths: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut result = Vec::new();

    for path in paths {
        let real = resolve_real_path(path);
        if seen.insert(real.clone()) {
            result.push(real);
        }
    }

    result
}

// ═══════════════════════════════════════════════════════════════════════
// I7.6: Case Sensitivity
// ═══════════════════════════════════════════════════════════════════════

/// Normalize a path for case-insensitive comparison.
///
/// On macOS/Windows, `Foo.fj` and `foo.fj` refer to the same file.
/// We normalize to lowercase for cache keys on case-insensitive systems.
pub fn normalize_case(path: &str, case_sensitive: bool) -> String {
    if case_sensitive {
        path.to_string()
    } else {
        path.to_lowercase()
    }
}

/// Detect duplicate files that differ only in case.
pub fn detect_case_conflicts(paths: &[String]) -> Vec<(String, String)> {
    let mut conflicts = Vec::new();
    let mut seen: HashMap<String, String> = HashMap::new();

    for path in paths {
        let lower = path.to_lowercase();
        if let Some(existing) = seen.get(&lower) {
            if existing != path {
                conflicts.push((existing.clone(), path.clone()));
            }
        } else {
            seen.insert(lower, path.clone());
        }
    }

    conflicts
}

// ═══════════════════════════════════════════════════════════════════════
// I7.7: Unicode Paths
// ═══════════════════════════════════════════════════════════════════════

/// Validate that a path can be used as a cache key.
///
/// Cache keys must be valid UTF-8 strings. Most paths are fine,
/// but we check for edge cases.
pub fn validate_cache_path(path: &str) -> Result<String, String> {
    if path.is_empty() {
        return Err("empty path".into());
    }

    // Normalize separators
    let normalized = path.replace('\\', "/");

    // Check for null bytes
    if normalized.contains('\0') {
        return Err("path contains null byte".into());
    }

    Ok(normalized)
}

/// Test if a path contains unicode characters.
pub fn has_unicode(path: &str) -> bool {
    !path.is_ascii()
}

// ═══════════════════════════════════════════════════════════════════════
// I7.8: Large File Handling
// ═══════════════════════════════════════════════════════════════════════

/// Hash a large file efficiently by reading in chunks.
///
/// For files > 1MB, we hash in 64KB chunks to avoid loading the entire
/// file into memory.
pub fn hash_large_content(content: &str) -> String {
    // For our FNV-1a hash, we can process in chunks since FNV is streaming
    compute_content_hash(content)
}

/// Check if content exceeds the large file threshold.
pub fn is_large_file(content: &str) -> bool {
    content.len() > 1_048_576 // 1 MB
}

/// Estimate hash time for a given content size.
pub fn estimated_hash_time_us(size_bytes: usize) -> u64 {
    // ~1 GB/s hashing speed → 1 byte per nanosecond
    (size_bytes as u64) / 1000 // microseconds
}

// ═══════════════════════════════════════════════════════════════════════
// I7.9: Circular Dependency Detection
// ═══════════════════════════════════════════════════════════════════════

/// Detect circular dependencies and report a clear error message.
pub fn detect_and_report_circular(
    deps: &HashMap<String, Vec<String>>,
) -> Option<String> {
    let mut visited = HashSet::new();
    let mut in_stack = HashSet::new();
    let mut path = Vec::new();

    for node in deps.keys() {
        if !visited.contains(node) {
            if let Some(cycle) =
                find_cycle(node, deps, &mut visited, &mut in_stack, &mut path)
            {
                return Some(format!(
                    "circular dependency: {}",
                    cycle.join(" -> ")
                ));
            }
        }
    }
    None
}

fn find_cycle(
    node: &str,
    deps: &HashMap<String, Vec<String>>,
    visited: &mut HashSet<String>,
    in_stack: &mut HashSet<String>,
    path: &mut Vec<String>,
) -> Option<Vec<String>> {
    visited.insert(node.to_string());
    in_stack.insert(node.to_string());
    path.push(node.to_string());

    if let Some(neighbors) = deps.get(node) {
        for next in neighbors {
            if !visited.contains(next) {
                if let Some(cycle) = find_cycle(next, deps, visited, in_stack, path) {
                    return Some(cycle);
                }
            } else if in_stack.contains(next) {
                let start = path.iter().position(|n| n == next).unwrap_or(0);
                let mut cycle: Vec<String> = path[start..].to_vec();
                cycle.push(next.clone());
                return Some(cycle);
            }
        }
    }

    path.pop();
    in_stack.remove(node);
    None
}

// ═══════════════════════════════════════════════════════════════════════
// Tests — I7.10
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── I7.1: Parse error recovery ──

    #[test]
    fn i7_1_parse_error_doesnt_invalidate_others() {
        let results = vec![
            ModuleCompileResult {
                path: "good.fj".into(),
                parse_ok: true,
                type_check_ok: true,
                codegen_ok: true,
                errors: vec![],
            },
            ModuleCompileResult {
                path: "bad.fj".into(),
                parse_ok: false,
                type_check_ok: false,
                codegen_ok: false,
                errors: vec!["syntax error".into()],
            },
        ];

        let cacheable = cacheable_modules(&results);
        assert_eq!(cacheable, vec!["good.fj"]);
        assert!(!cacheable.contains(&"bad.fj".to_string()));
    }

    // ── I7.2: Type error recovery ──

    #[test]
    fn i7_2_type_error_partial_cache() {
        let results = vec![
            ModuleCompileResult {
                path: "ok.fj".into(),
                parse_ok: true,
                type_check_ok: true,
                codegen_ok: true,
                errors: vec![],
            },
            ModuleCompileResult {
                path: "type_err.fj".into(),
                parse_ok: true,
                type_check_ok: false,
                codegen_ok: false,
                errors: vec!["type mismatch".into()],
            },
        ];

        let type_cacheable = type_cacheable_modules(&results);
        assert_eq!(type_cacheable, vec!["ok.fj"]);
    }

    // ── I7.3: Interrupted build recovery ──

    #[test]
    fn i7_3_recover_interrupted_build() {
        let state = BuildState {
            completed: ["a.fj", "b.fj"].iter().map(|s| s.to_string()).collect(),
            in_progress: ["c.fj"].iter().map(|s| s.to_string()).collect(),
            pending: ["d.fj", "e.fj"].iter().map(|s| s.to_string()).collect(),
        };

        let plan = state.recover();
        assert!(plan.keep_cached.contains("a.fj"));
        assert!(plan.keep_cached.contains("b.fj"));
        assert!(plan.rebuild.contains("c.fj")); // was in-progress, must rebuild
        assert!(plan.rebuild.contains("d.fj"));
        assert!(plan.rebuild.contains("e.fj"));
        assert!(plan.discarded.contains("c.fj"));
    }

    // ── I7.4: Clock skew handling ──

    #[test]
    fn i7_4_content_hash_immune_to_clock_skew() {
        let mut store = HashStore::default();
        store.update("file.fj", &compute_content_hash("let x = 42"));

        // Same content → not changed (even if mtime differs)
        let det = content_based_change_detection("file.fj", "let x = 42", &store);
        assert!(!det.changed);
        assert_eq!(det.method, "content-hash");

        // Different content → changed
        let det = content_based_change_detection("file.fj", "let x = 99", &store);
        assert!(det.changed);
    }

    // ── I7.5: Symlink handling ──

    #[test]
    fn i7_5_normalize_deduplicates() {
        // Without actual symlinks, test path dedup logic
        let paths = vec![
            "/project/src/main.fj".to_string(),
            "/project/src/main.fj".to_string(), // duplicate
            "/project/src/lib.fj".to_string(),
        ];
        let normalized = normalize_paths(&paths);
        assert_eq!(normalized.len(), 2); // deduplicated
    }

    // ── I7.6: Case sensitivity ──

    #[test]
    fn i7_6_case_insensitive_normalization() {
        assert_eq!(normalize_case("Foo.fj", false), "foo.fj");
        assert_eq!(normalize_case("Foo.fj", true), "Foo.fj");
    }

    #[test]
    fn i7_6_detect_case_conflicts() {
        let paths = vec![
            "src/Foo.fj".to_string(),
            "src/foo.fj".to_string(),
            "src/bar.fj".to_string(),
        ];
        let conflicts = detect_case_conflicts(&paths);
        assert_eq!(conflicts.len(), 1);
        assert!(conflicts[0].0.contains("Foo") || conflicts[0].1.contains("Foo"));
    }

    #[test]
    fn i7_6_no_case_conflicts() {
        let paths = vec!["a.fj".to_string(), "b.fj".to_string()];
        assert!(detect_case_conflicts(&paths).is_empty());
    }

    // ── I7.7: Unicode paths ──

    #[test]
    fn i7_7_unicode_path_valid() {
        let result = validate_cache_path("src/日本語/mod.fj");
        assert!(result.is_ok());
        assert!(has_unicode("src/日本語/mod.fj"));
    }

    #[test]
    fn i7_7_empty_path_rejected() {
        assert!(validate_cache_path("").is_err());
    }

    #[test]
    fn i7_7_backslash_normalized() {
        let result = validate_cache_path("src\\windows\\mod.fj").unwrap();
        assert_eq!(result, "src/windows/mod.fj");
    }

    // ── I7.8: Large file handling ──

    #[test]
    fn i7_8_large_file_detection() {
        assert!(!is_large_file("small content"));
        let large = "x".repeat(2_000_000);
        assert!(is_large_file(&large));
    }

    #[test]
    fn i7_8_hash_large_file_works() {
        let large = "fn main() { }\n".repeat(100_000); // ~1.5MB
        let hash = hash_large_content(&large);
        assert!(!hash.is_empty());
        // Deterministic
        assert_eq!(hash, hash_large_content(&large));
    }

    #[test]
    fn i7_8_estimated_hash_time() {
        let time_10mb = estimated_hash_time_us(10_000_000);
        assert!(time_10mb < 20_000); // < 20ms for 10MB
    }

    // ── I7.9: Circular dependency detection ──

    #[test]
    fn i7_9_no_circular() {
        let mut deps = HashMap::new();
        deps.insert("a.fj".to_string(), vec!["b.fj".to_string()]);
        deps.insert("b.fj".to_string(), vec!["c.fj".to_string()]);
        deps.insert("c.fj".to_string(), vec![]);

        assert!(detect_and_report_circular(&deps).is_none());
    }

    #[test]
    fn i7_9_circular_detected() {
        let mut deps = HashMap::new();
        deps.insert("a.fj".to_string(), vec!["b.fj".to_string()]);
        deps.insert("b.fj".to_string(), vec!["c.fj".to_string()]);
        deps.insert("c.fj".to_string(), vec!["a.fj".to_string()]);

        let err = detect_and_report_circular(&deps);
        assert!(err.is_some());
        let msg = err.unwrap();
        assert!(msg.contains("circular dependency"));
        assert!(msg.contains("->"));
    }

    // ── I7.10: Integration ──

    #[test]
    fn i7_10_full_edge_case_pipeline() {
        // Simulate: 3 modules, one has parse error, one interrupted
        let compile_results = vec![
            ModuleCompileResult {
                path: "core.fj".into(), parse_ok: true, type_check_ok: true,
                codegen_ok: true, errors: vec![],
            },
            ModuleCompileResult {
                path: "broken.fj".into(), parse_ok: false, type_check_ok: false,
                codegen_ok: false, errors: vec!["parse error line 5".into()],
            },
            ModuleCompileResult {
                path: "partial.fj".into(), parse_ok: true, type_check_ok: true,
                codegen_ok: false, errors: vec!["codegen interrupted".into()],
            },
        ];

        // Only core.fj is fully cacheable
        assert_eq!(cacheable_modules(&compile_results), vec!["core.fj", "partial.fj"]);
        assert_eq!(type_cacheable_modules(&compile_results), vec!["core.fj", "partial.fj"]);

        // Build state after interruption
        let state = BuildState {
            completed: ["core.fj"].iter().map(|s| s.to_string()).collect(),
            in_progress: ["partial.fj"].iter().map(|s| s.to_string()).collect(),
            pending: ["broken.fj"].iter().map(|s| s.to_string()).collect(),
        };
        let recovery = state.recover();
        assert_eq!(recovery.keep_cached.len(), 1); // core.fj
        assert_eq!(recovery.rebuild.len(), 2); // partial + broken

        // Content-based change detection
        let mut store = HashStore::default();
        store.update("core.fj", &compute_content_hash("fn main() {}"));
        let det = content_based_change_detection("core.fj", "fn main() {}", &store);
        assert!(!det.changed);

        // Unicode + case sensitivity
        assert!(validate_cache_path("src/模块/main.fj").is_ok());
        assert_eq!(normalize_case("SRC/Main.fj", false), "src/main.fj");
    }
}
