//! Incremental compilation integration tests for Fajar Lang.
//!
//! Tests dependency graphs, change detection, function-level tracking,
//! disk persistence, and incremental rebuild logic.

use fajar_lang::compiler::incremental::cache::*;
use fajar_lang::compiler::incremental::pipeline::*;
use fajar_lang::compiler::incremental::*;

// ════════════════════════════════════════════════════════════════════════
// 1. Content hashing
// ════════════════════════════════════════════════════════════════════════

#[test]
fn content_hash_deterministic() {
    let h1 = compute_content_hash("fn main() { 42 }");
    let h2 = compute_content_hash("fn main() { 42 }");
    assert_eq!(h1, h2);
}

#[test]
fn content_hash_different_for_different_source() {
    let h1 = compute_content_hash("fn main() { 42 }");
    let h2 = compute_content_hash("fn main() { 43 }");
    assert_ne!(h1, h2);
}

#[test]
fn content_hash_ignores_trailing_whitespace() {
    let h1 = compute_content_hash("fn main() { 42 }");
    let h2 = compute_content_hash("fn main() { 42 }   ");
    assert_eq!(h1, h2);
}

// ════════════════════════════════════════════════════════════════════════
// 2. File-level dependency graph
// ════════════════════════════════════════════════════════════════════════

#[test]
fn dep_graph_add_and_query() {
    let mut graph = DependencyGraph::new();
    graph.add_file(
        "a.fj".into(),
        "h1".into(),
        vec!["b.fj".into()],
        vec!["main".into()],
    );
    graph.add_file("b.fj".into(), "h2".into(), vec![], vec!["helper".into()]);

    assert_eq!(graph.file_count(), 2);
    assert_eq!(graph.dependencies("a.fj"), vec!["b.fj"]);
    assert_eq!(graph.dependents("b.fj"), vec!["a.fj"]);
}

#[test]
fn dep_graph_build_from_source() {
    let files = vec![
        ("main.fj".into(), "use math\nfn main() { }".into()),
        (
            "math.fj".into(),
            "pub fn add(a: i32, b: i32) -> i32 { a + b }".into(),
        ),
    ];
    let graph = build_dependency_graph(&files);
    assert_eq!(graph.file_count(), 2);
}

#[test]
fn dep_graph_transitive_dependents() {
    let mut graph = DependencyGraph::new();
    graph.add_file("a.fj".into(), "h1".into(), vec![], vec![]);
    graph.add_file("b.fj".into(), "h2".into(), vec!["a.fj".into()], vec![]);
    graph.add_file("c.fj".into(), "h3".into(), vec!["b.fj".into()], vec![]);

    let affected = transitive_dependents(&graph, &["a.fj".into()]);
    assert!(affected.contains(&"a.fj".to_string()));
    assert!(affected.contains(&"b.fj".to_string()));
    assert!(affected.contains(&"c.fj".to_string()));
}

#[test]
fn dep_graph_topological_sort() {
    let mut graph = DependencyGraph::new();
    graph.add_file("a.fj".into(), "h1".into(), vec![], vec![]);
    graph.add_file("b.fj".into(), "h2".into(), vec!["a.fj".into()], vec![]);
    graph.add_file("c.fj".into(), "h3".into(), vec!["b.fj".into()], vec![]);

    let order = topological_sort(&graph).unwrap();
    let a_idx = order.iter().position(|x| x == "a.fj").unwrap();
    let b_idx = order.iter().position(|x| x == "b.fj").unwrap();
    let c_idx = order.iter().position(|x| x == "c.fj").unwrap();
    assert!(a_idx < b_idx);
    assert!(b_idx < c_idx);
}

// ════════════════════════════════════════════════════════════════════════
// 3. Change detection
// ════════════════════════════════════════════════════════════════════════

#[test]
fn detect_changes_same_graph() {
    let files = vec![("a.fj".into(), "fn main() {}".into())];
    let g1 = build_dependency_graph(&files);
    let g2 = build_dependency_graph(&files);
    let changes = detect_changes(&g1, &g2);
    assert!(changes.is_empty());
}

#[test]
fn detect_changes_modified_file() {
    let f1 = vec![("a.fj".into(), "fn main() { 1 }".into())];
    let f2 = vec![("a.fj".into(), "fn main() { 2 }".into())];
    let g1 = build_dependency_graph(&f1);
    let g2 = build_dependency_graph(&f2);
    let changes = detect_changes(&g1, &g2);
    assert_eq!(changes, vec!["a.fj"]);
}

#[test]
fn detect_changes_new_file() {
    let f1 = vec![("a.fj".into(), "fn a() {}".into())];
    let f2 = vec![
        ("a.fj".into(), "fn a() {}".into()),
        ("b.fj".into(), "fn b() {}".into()),
    ];
    let g1 = build_dependency_graph(&f1);
    let g2 = build_dependency_graph(&f2);
    let changes = detect_changes(&g1, &g2);
    assert!(changes.contains(&"b.fj".to_string()));
}

#[test]
fn detect_changes_removed_file() {
    let f1 = vec![
        ("a.fj".into(), "fn a() {}".into()),
        ("b.fj".into(), "fn b() {}".into()),
    ];
    let f2 = vec![("a.fj".into(), "fn a() {}".into())];
    let g1 = build_dependency_graph(&f1);
    let g2 = build_dependency_graph(&f2);
    let changes = detect_changes(&g1, &g2);
    assert!(changes.contains(&"b.fj".to_string()));
}

// ════════════════════════════════════════════════════════════════════════
// 4. Function-level dependency graph
// ════════════════════════════════════════════════════════════════════════

#[test]
fn fn_graph_build_from_source() {
    let files = vec![(
        "main.fj".into(),
        "fn add(a: i64, b: i64) -> i64 { a + b }\nfn main() { add(1, 2) }".into(),
    )];
    let graph = build_function_graph(&files);
    assert!(graph.functions.contains_key("add"));
    assert!(graph.functions.contains_key("main"));
    assert_eq!(graph.function_count(), 2);
}

#[test]
fn fn_graph_tracks_calls() {
    let files = vec![(
        "math.fj".into(),
        "fn helper() -> i64 { 42 }\nfn compute() -> i64 { helper() }".into(),
    )];
    let graph = build_function_graph(&files);
    let compute = graph.functions.get("compute").unwrap();
    assert!(compute.calls.contains(&"helper".to_string()));
}

#[test]
fn fn_graph_reverse_callers() {
    let files = vec![(
        "lib.fj".into(),
        "fn helper() -> i64 { 42 }\nfn a() { helper() }\nfn b() { helper() }".into(),
    )];
    let graph = build_function_graph(&files);
    let callers = graph.callers_of("helper");
    assert!(callers.contains(&"a".to_string()));
    assert!(callers.contains(&"b".to_string()));
}

#[test]
fn fn_graph_detect_changed() {
    let f1 = vec![("a.fj".into(), "fn foo() { 1 }\nfn bar() { 2 }".into())];
    let f2 = vec![("a.fj".into(), "fn foo() { 1 }\nfn bar() { 99 }".into())];
    let g1 = build_function_graph(&f1);
    let g2 = build_function_graph(&f2);
    let changed = g2.detect_changed_functions(&g1);
    assert!(changed.contains(&"bar".to_string()));
    assert!(!changed.contains(&"foo".to_string()));
}

#[test]
fn fn_graph_transitive_callers() {
    let files = vec![(
        "chain.fj".into(),
        "fn a() { 1 }\nfn b() { a() }\nfn c() { b() }".into(),
    )];
    let graph = build_function_graph(&files);
    let affected = graph.transitive_callers(&["a".into()]);
    assert!(affected.contains(&"a".to_string()));
    assert!(affected.contains(&"b".to_string()));
    assert!(affected.contains(&"c".to_string()));
}

#[test]
fn fn_graph_const_fn_detection() {
    let files = vec![(
        "lib.fj".into(),
        "const fn factorial(n: i64) -> i64 { if n <= 1 { 1 } else { n * factorial(n - 1) } }"
            .into(),
    )];
    let graph = build_function_graph(&files);
    let f = graph.functions.get("factorial").unwrap();
    assert!(f.is_const);
}

#[test]
fn fn_graph_empty_source() {
    let files = vec![("empty.fj".into(), "// just a comment".into())];
    let graph = build_function_graph(&files);
    assert_eq!(graph.function_count(), 0);
}

fn temp_path(name: &str) -> String {
    std::env::temp_dir().join(name).display().to_string()
}

// ════════════════════════════════════════════════════════════════════════
// 5. Artifact cache
// ════════════════════════════════════════════════════════════════════════

#[test]
fn cache_store_and_lookup() {
    let mut cache = ArtifactCache::new(temp_path("fj-test-cache").into());
    let key = CacheKey::new("hash1".into(), "v1".into(), "x86".into(), "O2".into());
    let artifact = CachedArtifact::new(key.clone(), ArtifactType::Object, vec![1, 2, 3], 1);
    cache.cache_store(key.clone(), artifact).unwrap();
    let result = cache.cache_lookup(&key);
    assert!(result.is_some());
    assert_eq!(result.unwrap().data, vec![1, 2, 3]);
}

#[test]
fn cache_miss_for_unknown_key() {
    let mut cache = ArtifactCache::new(temp_path("fj-test-cache").into());
    let key = CacheKey::new("unknown".into(), "v1".into(), "x86".into(), "O2".into());
    assert!(cache.cache_lookup(&key).is_none());
}

#[test]
fn cache_invalidation() {
    let mut cache = ArtifactCache::new(temp_path("fj-test-cache").into());
    let key = CacheKey::new("h1".into(), "v1".into(), "x86".into(), "O2".into());
    let artifact = CachedArtifact::new(key.clone(), ArtifactType::Ast, vec![42], 1);
    cache.cache_store(key.clone(), artifact).unwrap();
    assert!(cache.cache_lookup(&key).is_some());
    cache.cache_invalidate(&key);
    assert!(cache.cache_lookup(&key).is_none());
}

#[test]
fn cache_stats_tracking() {
    let mut cache = ArtifactCache::new(temp_path("fj-test-cache").into());
    let key = CacheKey::new("h1".into(), "v1".into(), "x86".into(), "O2".into());
    let artifact = CachedArtifact::new(key.clone(), ArtifactType::Object, vec![1], 1);
    cache.cache_store(key.clone(), artifact).unwrap();

    let _ = cache.cache_lookup(&key); // hit
    let stats = cache.stats();
    assert_eq!(stats.hit_count, 1);
}

// ════════════════════════════════════════════════════════════════════════
// 6. Incremental compiler pipeline
// ════════════════════════════════════════════════════════════════════════

#[test]
fn incremental_first_build_compiles_all() {
    let mut compiler = IncrementalCompiler::new();
    let files = vec![
        ("a.fj".into(), "fn a() {}".into()),
        ("b.fj".into(), "fn b() {}".into()),
    ];
    let result = compiler.compile_incremental(&files);
    assert_eq!(result.compiled_files.len(), 2);
    assert_eq!(result.cached_files.len(), 0);
}

#[test]
fn incremental_no_change_uses_cache() {
    let mut compiler = IncrementalCompiler::new();
    let files = vec![
        ("a.fj".into(), "fn a() {}".into()),
        ("b.fj".into(), "fn b() {}".into()),
    ];

    // First build
    let r1 = compiler.compile_incremental(&files);
    assert_eq!(r1.compiled_files.len(), 2);

    // Second build (same files)
    let r2 = compiler.compile_incremental(&files);
    assert_eq!(r2.compiled_files.len(), 0);
    assert_eq!(r2.cached_files.len(), 2);
}

#[test]
fn incremental_one_file_changed() {
    let mut compiler = IncrementalCompiler::new();
    let files1 = vec![
        ("a.fj".into(), "fn a() { 1 }".into()),
        ("b.fj".into(), "fn b() { 2 }".into()),
    ];
    let files2 = vec![
        ("a.fj".into(), "fn a() { 99 }".into()), // changed
        ("b.fj".into(), "fn b() { 2 }".into()),  // unchanged
    ];

    compiler.compile_incremental(&files1);
    let r2 = compiler.compile_incremental(&files2);

    assert!(r2.compiled_files.contains(&"a.fj".to_string()));
    assert!(r2.cached_files.contains(&"b.fj".to_string()));
}

#[test]
fn incremental_build_report() {
    let mut compiler = IncrementalCompiler::new();
    let files = vec![("a.fj".into(), "fn a() {}".into())];
    let result = compiler.compile_incremental(&files);
    let report = compiler.build_report(&result);
    assert_eq!(report.total_files, 1);
}

#[test]
fn incremental_clean_cache() {
    let mut compiler = IncrementalCompiler::new();
    let files = vec![("a.fj".into(), "fn a() {}".into())];
    compiler.compile_incremental(&files);
    compiler.clean_cache();

    // After clean, everything should be compiled again
    let r2 = compiler.compile_incremental(&files);
    assert_eq!(r2.compiled_files.len(), 1);
    assert_eq!(r2.cached_files.len(), 0);
}

// ════════════════════════════════════════════════════════════════════════
// 7. Disk persistence (save/load graph)
// ════════════════════════════════════════════════════════════════════════

#[test]
fn disk_save_and_load_graph() {
    let cache_dir = temp_path("fj-inc-test");
    let _ = std::fs::remove_dir_all(&cache_dir);

    let mut graph = DependencyGraph::new();
    graph.add_file("a.fj".into(), "hash_a".into(), vec![], vec!["fn_a".into()]);
    graph.add_file(
        "b.fj".into(),
        "hash_b".into(),
        vec!["a.fj".into()],
        vec!["fn_b".into()],
    );

    save_graph_snapshot(&graph, &cache_dir).unwrap();
    let loaded = load_graph_snapshot(&cache_dir).unwrap();

    assert_eq!(loaded.file_count(), 2);
    assert!(loaded.nodes.contains_key("a.fj"));
    assert!(loaded.nodes.contains_key("b.fj"));
    assert_eq!(loaded.nodes.get("a.fj").unwrap().content_hash, "hash_a");

    let _ = std::fs::remove_dir_all(&cache_dir);
}

#[test]
fn disk_load_nonexistent_returns_empty() {
    let graph = load_graph_snapshot(&temp_path("nonexistent-fj-cache-12345")).unwrap();
    assert_eq!(graph.file_count(), 0);
}

// ════════════════════════════════════════════════════════════════════════
// 8. Compile result reporting
// ════════════════════════════════════════════════════════════════════════

#[test]
fn compile_result_total_files() {
    let result = CompileResult {
        compiled_files: vec!["a.fj".into()],
        cached_files: vec!["b.fj".into(), "c.fj".into()],
        errors: vec![],
        duration_ms: 100,
    };
    assert_eq!(result.total_files(), 3);
}

#[test]
fn compile_result_has_errors() {
    let result = CompileResult {
        compiled_files: vec![],
        cached_files: vec![],
        errors: vec!["some error".into()],
        duration_ms: 0,
    };
    assert!(!result.errors.is_empty());
}
