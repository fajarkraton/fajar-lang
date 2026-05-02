//! # Incremental Compilation Pipeline
//!
//! Orchestrates incremental compilation by combining the dependency graph
//! and artifact cache. On first build, performs full compilation and populates
//! the cache. On subsequent builds, detects changes via content hashing,
//! recompiles only changed files and their transitive dependents, and
//! serves unchanged files from cache.
//!
//! ## Pipeline Flow
//!
//! ```text
//! files → hash → diff against previous graph → identify changed
//!       → compute transitive dependents → compile changed set
//!       → cache new artifacts → produce CompileResult
//! ```

use std::collections::HashMap;

use super::cache::{ArtifactCache, ArtifactType, CacheKey, CacheStats, CachedArtifact};
use super::{
    DependencyGraph, IncrementalError, build_dependency_graph, compute_content_hash,
    detect_changes, topological_sort, transitive_dependents,
};

// ═══════════════════════════════════════════════════════════════════════
// CompileResult
// ═══════════════════════════════════════════════════════════════════════

/// Result of an incremental compilation run.
#[derive(Debug, Clone)]
pub struct CompileResult {
    /// Files that were actually compiled (cache miss).
    pub compiled_files: Vec<String>,
    /// Files served from cache (cache hit).
    pub cached_files: Vec<String>,
    /// Errors encountered during compilation.
    pub errors: Vec<String>,
    /// Total compilation duration in milliseconds.
    pub duration_ms: u64,
}

impl CompileResult {
    /// Returns true if the compilation completed with no errors.
    pub fn is_success(&self) -> bool {
        self.errors.is_empty()
    }

    /// Returns the total number of files processed.
    pub fn total_files(&self) -> usize {
        self.compiled_files.len() + self.cached_files.len()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// BuildReport
// ═══════════════════════════════════════════════════════════════════════

/// Human-readable summary of a build operation.
#[derive(Debug, Clone)]
pub struct BuildReport {
    /// Total number of files in the project.
    pub total_files: usize,
    /// Number of cache hits (files served from cache).
    pub cache_hits: usize,
    /// Number of cache misses (files actually compiled).
    pub cache_misses: usize,
    /// Build duration in milliseconds.
    pub duration_ms: u64,
}

impl BuildReport {
    /// Returns the cache hit rate as a percentage.
    pub fn hit_rate_pct(&self) -> f64 {
        if self.total_files == 0 {
            return 0.0;
        }
        (self.cache_hits as f64 / self.total_files as f64) * 100.0
    }

    /// Formats a human-readable summary string.
    pub fn summary(&self) -> String {
        format!(
            "Compiled {}/{} files ({:.0}% cache hit rate) in {}ms",
            self.cache_misses,
            self.total_files,
            self.hit_rate_pct(),
            self.duration_ms
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════
// CacheStatsReport
// ═══════════════════════════════════════════════════════════════════════

/// Summary report of cache statistics.
#[derive(Debug, Clone)]
pub struct CacheStatsReport {
    /// Cache hit rate percentage.
    pub hit_rate: f64,
    /// Total cache size in bytes.
    pub total_size: usize,
    /// Number of entries in the cache.
    pub entry_count: usize,
}

impl CacheStatsReport {
    /// Creates a report from raw cache statistics.
    pub fn from_stats(stats: &CacheStats) -> Self {
        Self {
            hit_rate: stats.hit_rate_pct(),
            total_size: stats.total_size,
            entry_count: stats.entry_count,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// FileWatcher
// ═══════════════════════════════════════════════════════════════════════

/// Simulated file watcher for detecting modified files.
///
/// In a real implementation this would use OS-level file system notifications.
/// This version uses timestamp comparison for simulation purposes.
#[derive(Debug)]
pub struct FileWatcher {
    /// Map from file path to last known modification timestamp.
    file_timestamps: HashMap<String, u64>,
}

impl FileWatcher {
    /// Creates a new file watcher with no tracked files.
    pub fn new() -> Self {
        Self {
            file_timestamps: HashMap::new(),
        }
    }

    /// Registers a file with its current modification timestamp.
    pub fn track_file(&mut self, path: String, timestamp: u64) {
        self.file_timestamps.insert(path, timestamp);
    }

    /// Detects files modified since the given timestamp.
    ///
    /// Returns paths of files whose tracked timestamp is greater than `since`.
    pub fn detect_modified(&self, since: u64) -> Vec<String> {
        let mut modified: Vec<String> = self
            .file_timestamps
            .iter()
            .filter(|(_, ts)| **ts > since)
            .map(|(path, _)| path.clone())
            .collect();
        modified.sort();
        modified
    }

    /// Updates the timestamp for a file (simulating a file save).
    pub fn update_timestamp(&mut self, path: &str, timestamp: u64) {
        if let Some(ts) = self.file_timestamps.get_mut(path) {
            *ts = timestamp;
        }
    }

    /// Returns the number of tracked files.
    pub fn tracked_count(&self) -> usize {
        self.file_timestamps.len()
    }
}

impl Default for FileWatcher {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// IncrementalCompiler
// ═══════════════════════════════════════════════════════════════════════

/// The incremental compiler that orchestrates smart recompilation.
///
/// Combines the dependency graph and artifact cache to minimize work
/// on rebuild. Tracks the previous build's dependency graph to detect
/// changes between builds.
#[derive(Debug)]
pub struct IncrementalCompiler {
    /// Current dependency graph.
    pub graph: DependencyGraph,
    /// Artifact cache for storing compiled results.
    pub cache: ArtifactCache,
    /// Previous build's dependency graph for change detection.
    pub previous_graph: Option<DependencyGraph>,
    /// Compiler version string for cache key generation.
    compiler_version: String,
    /// Target architecture string.
    target: String,
    /// Optimization level string.
    opt_level: String,
    /// Simulated timestamp counter for builds.
    build_counter: u64,
}

impl IncrementalCompiler {
    /// Creates a new incremental compiler with default settings.
    pub fn new() -> Self {
        Self {
            graph: DependencyGraph::new(),
            cache: ArtifactCache::new(".fj-cache".to_string()),
            previous_graph: None,
            compiler_version: "0.8.0".to_string(),
            target: "x86_64".to_string(),
            opt_level: "O2".to_string(),
            build_counter: 0,
        }
    }

    /// Creates an incremental compiler with custom settings.
    pub fn with_config(
        compiler_version: String,
        target: String,
        opt_level: String,
        cache_dir: String,
    ) -> Self {
        Self {
            graph: DependencyGraph::new(),
            cache: ArtifactCache::new(cache_dir),
            previous_graph: None,
            compiler_version,
            target,
            opt_level,
            build_counter: 0,
        }
    }

    /// Performs incremental compilation of the given source files.
    ///
    /// On first build, compiles all files. On subsequent builds, detects
    /// changes and only recompiles affected files, serving the rest from cache.
    pub fn compile_incremental(&mut self, files: &[(String, String)]) -> CompileResult {
        self.build_counter += 1;
        let new_graph = build_dependency_graph(files);

        let (to_compile, to_cache) = self.partition_files(files, &new_graph);

        let mut compiled_files = Vec::new();
        let mut cached_files = Vec::new();
        let mut errors = Vec::new();

        // Process cached files
        for path in &to_cache {
            cached_files.push(path.clone());
        }

        // Compile changed files in topological order
        let compile_order = determine_compile_order(&new_graph, &to_compile);
        for path in &compile_order {
            match self.compile_single_file(path, files) {
                Ok(()) => compiled_files.push(path.clone()),
                Err(e) => errors.push(format!("{path}: {e}")),
            }
        }

        // Update state for next build
        self.previous_graph = Some(new_graph.clone());
        self.graph = new_graph;

        CompileResult {
            compiled_files,
            cached_files,
            errors,
            duration_ms: self.build_counter, // simulated
        }
    }

    /// Partitions files into those needing compilation and those cached.
    fn partition_files(
        &mut self,
        files: &[(String, String)],
        new_graph: &DependencyGraph,
    ) -> (Vec<String>, Vec<String>) {
        let all_paths: Vec<String> = files.iter().map(|(p, _)| p.clone()).collect();

        match &self.previous_graph {
            None => {
                // First build: compile everything
                (all_paths, Vec::new())
            }
            Some(prev) => {
                let changed = detect_changes(prev, new_graph);
                let affected = transitive_dependents(new_graph, &changed);

                let mut to_compile = Vec::new();
                let mut to_cache = Vec::new();

                for path in &all_paths {
                    if affected.contains(path) {
                        to_compile.push(path.clone());
                    } else {
                        to_cache.push(path.clone());
                    }
                }

                (to_compile, to_cache)
            }
        }
    }

    /// Compiles a single file and stores the result in the cache.
    fn compile_single_file(
        &mut self,
        path: &str,
        files: &[(String, String)],
    ) -> Result<(), IncrementalError> {
        let source = files
            .iter()
            .find(|(p, _)| p == path)
            .map(|(_, s)| s.as_str())
            .ok_or_else(|| IncrementalError::FileNotFound {
                path: path.to_string(),
            })?;

        let hash = compute_content_hash(source);
        let key = CacheKey::new(
            hash,
            self.compiler_version.clone(),
            self.target.clone(),
            self.opt_level.clone(),
        );

        // Simulate compilation: produce artifact data from source
        let artifact_data = simulate_compile(source);
        let artifact = CachedArtifact::new(
            key.clone(),
            ArtifactType::Object,
            artifact_data,
            self.build_counter,
        );

        self.cache.cache_store(key, artifact)
    }

    /// Generates a build report from a compilation result.
    pub fn build_report(&self, result: &CompileResult) -> BuildReport {
        BuildReport {
            total_files: result.total_files(),
            cache_hits: result.cached_files.len(),
            cache_misses: result.compiled_files.len(),
            duration_ms: result.duration_ms,
        }
    }

    /// Clears all cached artifacts, forcing a full recompile on next build.
    pub fn clean_cache(&mut self) {
        self.cache.clear();
        self.previous_graph = None;
    }

    /// Returns a statistics report for the artifact cache.
    pub fn cache_stats_report(&self) -> CacheStatsReport {
        CacheStatsReport::from_stats(&self.cache.stats())
    }
}

impl Default for IncrementalCompiler {
    fn default() -> Self {
        Self::new()
    }
}

/// Determines compilation order for the given subset of files.
fn determine_compile_order(graph: &DependencyGraph, files: &[String]) -> Vec<String> {
    match topological_sort(graph) {
        Ok(order) => {
            let file_set: std::collections::HashSet<&String> = files.iter().collect();
            order.into_iter().filter(|f| file_set.contains(f)).collect()
        }
        Err(_) => {
            // Fallback: compile in given order if cycle detected
            files.to_vec()
        }
    }
}

/// Simulates compilation by producing deterministic output from source.
fn simulate_compile(source: &str) -> Vec<u8> {
    // Run real compiler front-end: tokenize → parse → analyze.
    // Returns serialized AST on success, or error description on failure.
    match crate::lexer::tokenize(source) {
        Ok(tokens) => match crate::parser::parse(tokens) {
            Ok(program) => {
                // Run analyzer (ignore warnings, collect errors).
                let _analysis = crate::analyzer::analyze(&program);
                // Serialize a digest of the program (item count + hash).
                let item_count = program.items.len();
                let hash = compute_content_hash(source);
                format!("compiled:{hash}:items={item_count}").into_bytes()
            }
            Err(_) => {
                let hash = compute_content_hash(source);
                format!("parse_error:{hash}").into_bytes()
            }
        },
        Err(_) => {
            let hash = compute_content_hash(source);
            format!("lex_error:{hash}").into_bytes()
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Parallel compilation simulation
// ═══════════════════════════════════════════════════════════════════════

/// Simulates parallel compilation of files across multiple workers.
///
/// Divides the file list into chunks and "compiles" each chunk independently.
/// In a real implementation, this would use thread pools or async tasks.
pub fn compile_parallel(files: &[(String, String)], max_workers: usize) -> Vec<CompileResult> {
    let worker_count = max_workers.max(1).min(files.len().max(1));
    let chunk_size = (files.len() + worker_count - 1) / worker_count.max(1);

    let mut results = Vec::new();

    for (worker_id, chunk) in files.chunks(chunk_size.max(1)).enumerate() {
        let mut compiled = Vec::new();
        let mut errors = Vec::new();

        for (path, source) in chunk {
            // Simulate compilation
            if source.is_empty() {
                errors.push(format!("{path}: empty source file"));
            } else {
                compiled.push(path.clone());
            }
        }

        results.push(CompileResult {
            compiled_files: compiled,
            cached_files: Vec::new(),
            errors,
            duration_ms: (worker_id as u64 + 1) * 10, // simulated timing
        });
    }

    results
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_files() -> Vec<(String, String)> {
        vec![
            (
                "math.fj".to_string(),
                "pub fn add(a: i32, b: i32) -> i32 { a + b }".to_string(),
            ),
            (
                "utils.fj".to_string(),
                "use math\npub fn double(x: i32) -> i32 { add(x, x) }".to_string(),
            ),
            (
                "main.fj".to_string(),
                "use utils\nfn main() { double(21) }".to_string(),
            ),
        ]
    }

    #[test]
    fn s11_1_first_build_compiles_all() {
        let mut compiler = IncrementalCompiler::new();
        let files = sample_files();

        let result = compiler.compile_incremental(&files);
        assert!(result.is_success());
        assert_eq!(result.compiled_files.len(), 3);
        assert_eq!(result.cached_files.len(), 0);
        assert_eq!(result.total_files(), 3);
    }

    #[test]
    fn s11_2_unchanged_rebuild_uses_cache() {
        let mut compiler = IncrementalCompiler::new();
        let files = sample_files();

        // First build
        compiler.compile_incremental(&files);

        // Second build with same files
        let result = compiler.compile_incremental(&files);
        assert!(result.is_success());
        assert_eq!(result.compiled_files.len(), 0);
        assert_eq!(result.cached_files.len(), 3);
    }

    #[test]
    fn s11_3_single_file_change_recompiles_dependents() {
        let mut compiler = IncrementalCompiler::new();
        let files = sample_files();

        // First build
        compiler.compile_incremental(&files);

        // Change math.fj only
        let files_v2 = vec![
            (
                "math.fj".to_string(),
                "pub fn add(a: i32, b: i32) -> i32 { a + b + 0 }".to_string(),
            ),
            (
                "utils.fj".to_string(),
                "use math\npub fn double(x: i32) -> i32 { add(x, x) }".to_string(),
            ),
            (
                "main.fj".to_string(),
                "use utils\nfn main() { double(21) }".to_string(),
            ),
        ];

        let result = compiler.compile_incremental(&files_v2);
        assert!(result.is_success());
        // math.fj changed, utils.fj and main.fj depend on it
        assert!(result.compiled_files.contains(&"math.fj".to_string()));
        assert!(!result.compiled_files.is_empty());
    }

    #[test]
    fn s11_4_build_report_summary() {
        let mut compiler = IncrementalCompiler::new();
        let files = sample_files();

        let result = compiler.compile_incremental(&files);
        let report = compiler.build_report(&result);

        assert_eq!(report.total_files, 3);
        assert_eq!(report.cache_misses, 3);
        assert_eq!(report.cache_hits, 0);
        assert!(report.summary().contains("3/3"));
    }

    #[test]
    fn s11_5_clean_cache_forces_full_recompile() {
        let mut compiler = IncrementalCompiler::new();
        let files = sample_files();

        compiler.compile_incremental(&files);
        compiler.clean_cache();

        let result = compiler.compile_incremental(&files);
        // After cache clear, it is a "first build" again
        assert_eq!(result.compiled_files.len(), 3);
        assert_eq!(result.cached_files.len(), 0);
    }

    #[test]
    fn s11_6_file_watcher_detects_modifications() {
        let mut watcher = FileWatcher::new();
        watcher.track_file("a.fj".to_string(), 100);
        watcher.track_file("b.fj".to_string(), 200);
        watcher.track_file("c.fj".to_string(), 300);

        let modified = watcher.detect_modified(150);
        assert_eq!(modified, vec!["b.fj", "c.fj"]);

        watcher.update_timestamp("a.fj", 500);
        let modified2 = watcher.detect_modified(250);
        assert!(modified2.contains(&"a.fj".to_string()));
        assert!(modified2.contains(&"c.fj".to_string()));
    }

    #[test]
    fn s11_7_parallel_compilation_distributes_work() {
        let files: Vec<(String, String)> = (0..10)
            .map(|i| (format!("file{i}.fj"), format!("fn f{i}() {{ }}")))
            .collect();

        let results = compile_parallel(&files, 3);
        assert!(results.len() >= 2); // at least 2 workers

        let total_compiled: usize = results.iter().map(|r| r.compiled_files.len()).sum();
        assert_eq!(total_compiled, 10);
    }

    #[test]
    fn s11_8_error_recovery_on_empty_source() {
        let files = vec![
            ("good.fj".to_string(), "fn main() { }".to_string()),
            ("empty.fj".to_string(), String::new()),
        ];

        let results = compile_parallel(&files, 2);
        let all_errors: Vec<String> = results.iter().flat_map(|r| r.errors.clone()).collect();
        assert!(all_errors.iter().any(|e| e.contains("empty")));
    }

    #[test]
    fn s11_9_cache_stats_report() {
        let mut compiler = IncrementalCompiler::new();
        let files = sample_files();

        compiler.compile_incremental(&files);
        let report = compiler.cache_stats_report();

        assert_eq!(report.entry_count, 3);
        assert!(report.total_size > 0);
    }

    #[test]
    fn s11_10_compile_result_success_flag() {
        let result_ok = CompileResult {
            compiled_files: vec!["a.fj".into()],
            cached_files: vec![],
            errors: vec![],
            duration_ms: 10,
        };
        assert!(result_ok.is_success());

        let result_err = CompileResult {
            compiled_files: vec![],
            cached_files: vec![],
            errors: vec!["syntax error".into()],
            duration_ms: 5,
        };
        assert!(!result_err.is_success());
    }
}
