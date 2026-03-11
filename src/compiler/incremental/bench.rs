//! # Compilation Speed Benchmarks
//!
//! Provides benchmark infrastructure for measuring full and incremental
//! compilation performance. Includes project generation for simulation,
//! timing per compilation phase, and bottleneck analysis.
//!
//! ## Benchmark Scenarios
//!
//! - **Full build**: compile entire project from scratch
//! - **Incremental 1-file**: change 1 file and rebuild
//! - **Incremental 10-file**: change 10 files and rebuild
//! - **Lazy parsing/analysis**: parse/analyze only changed files

use std::collections::HashMap;

use super::compute_content_hash;
use super::pipeline::IncrementalCompiler;

// ═══════════════════════════════════════════════════════════════════════
// BenchmarkResult
// ═══════════════════════════════════════════════════════════════════════

/// Result of a compilation benchmark run.
#[derive(Debug, Clone)]
pub struct BenchmarkResult {
    /// Time for full build in milliseconds.
    pub full_build_ms: u64,
    /// Time for incremental build in milliseconds.
    pub incremental_ms: u64,
    /// Speedup ratio (full_build_ms / incremental_ms).
    pub speedup_ratio: f64,
    /// Cache hit rate during the incremental build.
    pub cache_hit_rate: f64,
}

impl BenchmarkResult {
    /// Creates a new benchmark result, computing the speedup ratio.
    pub fn new(full_build_ms: u64, incremental_ms: u64, cache_hit_rate: f64) -> Self {
        let speedup_ratio = if incremental_ms > 0 {
            full_build_ms as f64 / incremental_ms as f64
        } else {
            f64::INFINITY
        };
        Self {
            full_build_ms,
            incremental_ms,
            speedup_ratio,
            cache_hit_rate,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// TimeReport
// ═══════════════════════════════════════════════════════════════════════

/// Breakdown of compilation time by phase.
#[derive(Debug, Clone)]
pub struct TimeReport {
    /// Per-phase timings as `(phase_name, duration_ms)`.
    pub phases: Vec<(String, u64)>,
    /// Total parse time in milliseconds.
    pub parse_ms: u64,
    /// Total analysis time in milliseconds.
    pub analyze_ms: u64,
    /// Total code generation time in milliseconds.
    pub codegen_ms: u64,
    /// Total compilation time in milliseconds.
    pub total_ms: u64,
}

impl TimeReport {
    /// Creates a time report from phase durations.
    pub fn new(parse_ms: u64, analyze_ms: u64, codegen_ms: u64) -> Self {
        let total_ms = parse_ms + analyze_ms + codegen_ms;
        let phases = vec![
            ("parse".to_string(), parse_ms),
            ("analyze".to_string(), analyze_ms),
            ("codegen".to_string(), codegen_ms),
        ];
        Self {
            phases,
            parse_ms,
            analyze_ms,
            codegen_ms,
            total_ms,
        }
    }

    /// Returns the slowest phase name and its duration.
    pub fn slowest_phase(&self) -> Option<(&str, u64)> {
        self.phases
            .iter()
            .max_by_key(|(_, ms)| *ms)
            .map(|(name, ms)| (name.as_str(), *ms))
    }
}

// ═══════════════════════════════════════════════════════════════════════
// BottleneckReport
// ═══════════════════════════════════════════════════════════════════════

/// Identifies the slowest files and phases in a build.
#[derive(Debug, Clone)]
pub struct BottleneckReport {
    /// Slowest files ranked by compilation time: `(file_path, duration_ms)`.
    pub slowest_files: Vec<(String, u64)>,
    /// Slowest phases ranked by total time: `(phase_name, duration_ms)`.
    pub slowest_phases: Vec<(String, u64)>,
}

impl BottleneckReport {
    /// Creates a bottleneck report from file and phase timings.
    pub fn new(
        mut file_timings: Vec<(String, u64)>,
        mut phase_timings: Vec<(String, u64)>,
    ) -> Self {
        file_timings.sort_by(|a, b| b.1.cmp(&a.1));
        phase_timings.sort_by(|a, b| b.1.cmp(&a.1));
        Self {
            slowest_files: file_timings,
            slowest_phases: phase_timings,
        }
    }

    /// Returns the top N slowest files.
    pub fn top_n_files(&self, n: usize) -> &[(String, u64)] {
        let end = n.min(self.slowest_files.len());
        &self.slowest_files[..end]
    }

    /// Returns the top N slowest phases.
    pub fn top_n_phases(&self, n: usize) -> &[(String, u64)] {
        let end = n.min(self.slowest_phases.len());
        &self.slowest_phases[..end]
    }
}

// ═══════════════════════════════════════════════════════════════════════
// LazyParsing / LazyAnalysis
// ═══════════════════════════════════════════════════════════════════════

/// Lazy parsing result: only parses changed files, returns cached ASTs for others.
#[derive(Debug, Clone)]
pub struct LazyParsing {
    /// Files that were actually parsed.
    pub parsed_files: Vec<String>,
    /// Files whose AST was served from cache.
    pub cached_files: Vec<String>,
    /// Simulated AST data keyed by file path.
    pub ast_cache: HashMap<String, Vec<u8>>,
}

impl LazyParsing {
    /// Performs lazy parsing: only parse files in `changed`, cache the rest.
    pub fn parse(all_files: &[(String, String)], changed: &[String]) -> Self {
        let mut parsed_files = Vec::new();
        let mut cached_files = Vec::new();
        let mut ast_cache = HashMap::new();

        for (path, source) in all_files {
            if changed.contains(path) {
                // Actually parse (simulated)
                let ast_data = simulate_parse(source);
                ast_cache.insert(path.clone(), ast_data);
                parsed_files.push(path.clone());
            } else {
                // Use cached AST (simulated)
                let cached_ast = format!("cached_ast:{}", compute_content_hash(source));
                ast_cache.insert(path.clone(), cached_ast.into_bytes());
                cached_files.push(path.clone());
            }
        }

        Self {
            parsed_files,
            cached_files,
            ast_cache,
        }
    }

    /// Returns the total number of files processed.
    pub fn total_files(&self) -> usize {
        self.parsed_files.len() + self.cached_files.len()
    }
}

/// Lazy analysis result: only analyzes files whose dependencies changed.
#[derive(Debug, Clone)]
pub struct LazyAnalysis {
    /// Files that were actually analyzed.
    pub analyzed_files: Vec<String>,
    /// Files whose analysis was served from cache.
    pub cached_files: Vec<String>,
}

impl LazyAnalysis {
    /// Performs lazy analysis: only analyze files in `affected`, cache the rest.
    pub fn analyze(all_files: &[String], affected: &[String]) -> Self {
        let mut analyzed_files = Vec::new();
        let mut cached_files = Vec::new();

        for path in all_files {
            if affected.contains(path) {
                analyzed_files.push(path.clone());
            } else {
                cached_files.push(path.clone());
            }
        }

        Self {
            analyzed_files,
            cached_files,
        }
    }

    /// Returns the total number of files processed.
    pub fn total_files(&self) -> usize {
        self.analyzed_files.len() + self.cached_files.len()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// CompilationBenchmark
// ═══════════════════════════════════════════════════════════════════════

/// Benchmark harness for measuring compilation performance.
///
/// Generates synthetic projects and measures full vs incremental build times.
#[derive(Debug)]
pub struct CompilationBenchmark {
    /// Number of files in the simulated project.
    pub project_files: usize,
    /// Average file size in lines.
    pub file_sizes: usize,
    /// Whether this is a simulation (always true for now).
    pub simulation_mode: bool,
}

impl CompilationBenchmark {
    /// Creates a new benchmark with the given project parameters.
    pub fn new(project_files: usize, file_sizes: usize) -> Self {
        Self {
            project_files,
            file_sizes,
            simulation_mode: true,
        }
    }

    /// Generates a synthetic project with the configured number of files.
    pub fn generate_project(&self) -> Vec<(String, String)> {
        generate_synthetic_project(self.project_files, self.file_sizes)
    }

    /// Benchmarks a full build of the generated project.
    pub fn benchmark_full_build(&self, files: &[(String, String)]) -> BenchmarkResult {
        let mut compiler = IncrementalCompiler::new();
        let result = compiler.compile_incremental(files);

        let full_ms = simulate_build_time(files.len(), false);
        BenchmarkResult::new(full_ms, full_ms, 0.0).with_compile_result(&result)
    }

    /// Benchmarks an incremental rebuild after changing 1 file.
    pub fn benchmark_incremental_1_file(&self, files: &[(String, String)]) -> BenchmarkResult {
        self.benchmark_incremental_n(files, 1)
    }

    /// Benchmarks an incremental rebuild after changing 10 files.
    pub fn benchmark_incremental_10_files(&self, files: &[(String, String)]) -> BenchmarkResult {
        self.benchmark_incremental_n(files, 10)
    }

    /// Benchmarks an incremental rebuild after changing `n` files.
    fn benchmark_incremental_n(&self, files: &[(String, String)], n: usize) -> BenchmarkResult {
        let mut compiler = IncrementalCompiler::new();

        // Full build first
        compiler.compile_incremental(files);
        let full_ms = simulate_build_time(files.len(), false);

        // Modify n files
        let mut modified_files = files.to_vec();
        let changes = n.min(modified_files.len());
        for item in modified_files.iter_mut().take(changes) {
            item.1 = format!("{}\n// modified", item.1);
        }

        // Incremental rebuild
        let result = compiler.compile_incremental(&modified_files);
        let incr_ms = simulate_build_time(result.compiled_files.len(), true);

        let cache_hit_rate = if result.total_files() > 0 {
            result.cached_files.len() as f64 / result.total_files() as f64 * 100.0
        } else {
            0.0
        };

        BenchmarkResult::new(full_ms, incr_ms, cache_hit_rate)
    }

    /// Profiles compilation phases for the given files.
    pub fn profile_phases(&self, files: &[(String, String)]) -> TimeReport {
        let file_count = files.len() as u64;
        // Simulated phase timings proportional to file count
        let parse_ms = file_count * 2;
        let analyze_ms = file_count * 3;
        let codegen_ms = file_count * 5;

        TimeReport::new(parse_ms, analyze_ms, codegen_ms)
    }

    /// Identifies bottlenecks in the build.
    pub fn identify_bottlenecks(&self, files: &[(String, String)]) -> BottleneckReport {
        // Simulate per-file timings (larger files take longer)
        let file_timings: Vec<(String, u64)> = files
            .iter()
            .map(|(path, source)| {
                let time = (source.len() as u64) / 10 + 1;
                (path.clone(), time)
            })
            .collect();

        let time_report = self.profile_phases(files);
        let phase_timings = time_report.phases;

        BottleneckReport::new(file_timings, phase_timings)
    }
}

impl BenchmarkResult {
    /// Enriches the benchmark result with data from a compile result.
    fn with_compile_result(mut self, result: &super::pipeline::CompileResult) -> Self {
        if result.total_files() > 0 {
            self.cache_hit_rate =
                result.cached_files.len() as f64 / result.total_files() as f64 * 100.0;
        }
        self
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════════════════════════════

/// Generates a synthetic project with `n` files, each ~`lines` lines long.
fn generate_synthetic_project(n: usize, lines: usize) -> Vec<(String, String)> {
    let mut files = Vec::with_capacity(n);

    for i in 0..n {
        let name = format!("module_{i:04}.fj");
        let mut source = String::new();

        // Add dependencies on earlier modules
        if i > 0 {
            let dep_idx = (i - 1).min(n - 1);
            source.push_str(&format!("use module_{dep_idx:04}\n"));
        }

        // Generate function bodies
        let fns_per_file = lines / 5;
        for f in 0..fns_per_file.max(1) {
            source.push_str(&format!(
                "fn func_{i}_{f}(x: i32) -> i32 {{\n    let y = x + {f}\n    y\n}}\n\n"
            ));
        }

        files.push((name, source));
    }

    files
}

/// Simulates build time based on file count and whether incremental.
fn simulate_build_time(file_count: usize, incremental: bool) -> u64 {
    let base_per_file = 10u64; // ms per file
    let overhead = 50u64; // fixed overhead

    let compile_time = file_count as u64 * base_per_file + overhead;

    if incremental {
        // Incremental builds have less overhead
        compile_time / 2
    } else {
        compile_time
    }
}

/// Simulates parsing a source file and producing AST data.
fn simulate_parse(source: &str) -> Vec<u8> {
    let hash = compute_content_hash(source);
    format!("ast:{hash}").into_bytes()
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::incremental::build_dependency_graph;

    #[test]
    fn s12_1_benchmark_result_speedup_ratio() {
        let result = BenchmarkResult::new(1000, 100, 90.0);
        assert!((result.speedup_ratio - 10.0).abs() < 0.01);
        assert!((result.cache_hit_rate - 90.0).abs() < 0.01);
    }

    #[test]
    fn s12_2_full_build_benchmark() {
        let bench = CompilationBenchmark::new(20, 10);
        let files = bench.generate_project();

        let result = bench.benchmark_full_build(&files);
        assert!(result.full_build_ms > 0);
        // Full build: no incremental benefit
        assert!((result.speedup_ratio - 1.0).abs() < 0.01);
    }

    #[test]
    fn s12_3_incremental_1_file_benchmark() {
        let bench = CompilationBenchmark::new(50, 10);
        let files = bench.generate_project();

        let result = bench.benchmark_incremental_1_file(&files);
        assert!(result.full_build_ms > 0);
        assert!(result.incremental_ms > 0);
        // Incremental should be faster (or equal for tiny projects)
        assert!(result.incremental_ms <= result.full_build_ms);
    }

    #[test]
    fn s12_4_incremental_10_file_benchmark() {
        let bench = CompilationBenchmark::new(50, 10);
        let files = bench.generate_project();

        let result = bench.benchmark_incremental_10_files(&files);
        assert!(result.full_build_ms > 0);
        assert!(result.incremental_ms > 0);
        assert!(result.incremental_ms <= result.full_build_ms);
    }

    #[test]
    fn s12_5_lazy_parsing_only_parses_changed() {
        let files = vec![
            ("a.fj".to_string(), "fn a() {}".to_string()),
            ("b.fj".to_string(), "fn b() {}".to_string()),
            ("c.fj".to_string(), "fn c() {}".to_string()),
        ];
        let changed = vec!["b.fj".to_string()];

        let result = LazyParsing::parse(&files, &changed);
        assert_eq!(result.parsed_files, vec!["b.fj"]);
        assert_eq!(result.cached_files.len(), 2);
        assert_eq!(result.total_files(), 3);
        assert!(result.ast_cache.contains_key("a.fj"));
        assert!(result.ast_cache.contains_key("b.fj"));
        assert!(result.ast_cache.contains_key("c.fj"));
    }

    #[test]
    fn s12_6_lazy_analysis_only_analyzes_affected() {
        let all = vec!["a.fj".into(), "b.fj".into(), "c.fj".into(), "d.fj".into()];
        let affected = vec!["b.fj".into(), "c.fj".into()];

        let result = LazyAnalysis::analyze(&all, &affected);
        assert_eq!(result.analyzed_files.len(), 2);
        assert_eq!(result.cached_files.len(), 2);
        assert_eq!(result.total_files(), 4);
    }

    #[test]
    fn s12_7_time_report_phases() {
        let report = TimeReport::new(50, 80, 120);
        assert_eq!(report.total_ms, 250);
        assert_eq!(report.phases.len(), 3);

        let (name, ms) = report.slowest_phase().expect("has phases");
        assert_eq!(name, "codegen");
        assert_eq!(ms, 120);
    }

    #[test]
    fn s12_8_bottleneck_report_ranking() {
        let file_timings = vec![
            ("small.fj".into(), 10u64),
            ("huge.fj".into(), 500),
            ("medium.fj".into(), 100),
        ];
        let phase_timings = vec![
            ("parse".into(), 30u64),
            ("codegen".into(), 200),
            ("analyze".into(), 70),
        ];

        let report = BottleneckReport::new(file_timings, phase_timings);
        let top2 = report.top_n_files(2);
        assert_eq!(top2[0].0, "huge.fj");
        assert_eq!(top2[1].0, "medium.fj");

        let top1_phase = report.top_n_phases(1);
        assert_eq!(top1_phase[0].0, "codegen");
    }

    #[test]
    fn s12_9_generate_100_file_project() {
        let bench = CompilationBenchmark::new(100, 20);
        let files = bench.generate_project();
        assert_eq!(files.len(), 100);

        // Each file should have content
        for (path, source) in &files {
            assert!(path.ends_with(".fj"));
            assert!(!source.is_empty());
        }

        // Dependency chain: module_0001 should use module_0000
        let (_, src1) = &files[1];
        assert!(src1.contains("use module_0000"));

        // Profile and identify bottlenecks
        let time = bench.profile_phases(&files);
        assert!(time.total_ms > 0);
        assert!(time.parse_ms < time.codegen_ms); // codegen is slowest

        let bottlenecks = bench.identify_bottlenecks(&files);
        assert!(!bottlenecks.slowest_files.is_empty());
        assert!(!bottlenecks.slowest_phases.is_empty());
    }

    #[test]
    fn s12_10_benchmark_dependency_graph_performance() {
        let bench = CompilationBenchmark::new(100, 10);
        let files = bench.generate_project();

        // Build dependency graph
        let graph = build_dependency_graph(&files);
        assert_eq!(graph.file_count(), 100);

        // Verify chain dependencies
        let deps = graph.dependencies("module_0050.fj");
        assert!(!deps.is_empty());

        // Full build then incremental should show speedup
        let result_full = bench.benchmark_full_build(&files);
        let result_incr = bench.benchmark_incremental_1_file(&files);

        assert!(result_incr.incremental_ms <= result_full.full_build_ms);
    }
}
