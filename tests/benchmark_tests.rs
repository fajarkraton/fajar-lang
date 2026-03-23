//! Benchmarks Game integration tests for Fajar Lang.
//!
//! Verifies that all benchmark programs parse, analyze, and run correctly.

use fajar_lang::codegen::benchmarks::*;
use fajar_lang::interpreter::Interpreter;
use std::path::Path;

/// Helper: run a .fj example through the interpreter.
fn run_example(path: &str) -> Vec<String> {
    let source =
        std::fs::read_to_string(path).unwrap_or_else(|e| panic!("cannot read {path}: {e}"));
    let mut interp = Interpreter::new_capturing();
    interp
        .eval_source(&source)
        .unwrap_or_else(|e| panic!("{path} eval failed: {e}"));
    interp
        .call_main()
        .unwrap_or_else(|e| panic!("{path} main() failed: {e}"));
    interp.get_output().to_vec()
}

/// Helper: verify a .fj file parses cleanly.
fn parse_example(path: &str) {
    let source = std::fs::read_to_string(path).unwrap();
    let tokens = fajar_lang::lexer::tokenize(&source).unwrap();
    let _program = fajar_lang::parser::parse(tokens).unwrap();
}

// ════════════════════════════════════════════════════════════════════════
// 1. All benchmark programs exist
// ════════════════════════════════════════════════════════════════════════

#[test]
fn bench_nbody_exists() {
    assert!(Path::new("examples/bench_nbody.fj").exists());
}

#[test]
fn bench_fannkuch_exists() {
    assert!(Path::new("examples/bench_fannkuch.fj").exists());
}

#[test]
fn bench_spectral_norm_exists() {
    assert!(Path::new("examples/bench_spectral_norm.fj").exists());
}

#[test]
fn bench_mandelbrot_exists() {
    assert!(Path::new("examples/bench_mandelbrot.fj").exists());
}

#[test]
fn bench_binary_trees_exists() {
    assert!(Path::new("examples/bench_binary_trees.fj").exists());
}

#[test]
fn bench_fasta_exists() {
    assert!(Path::new("examples/bench_fasta.fj").exists());
}

#[test]
fn bench_matrix_multiply_exists() {
    assert!(Path::new("examples/bench_matrix_multiply.fj").exists());
}

// ════════════════════════════════════════════════════════════════════════
// 2. All benchmark programs parse
// ════════════════════════════════════════════════════════════════════════

#[test]
fn bench_nbody_parses() {
    parse_example("examples/bench_nbody.fj");
}

#[test]
fn bench_fannkuch_parses() {
    parse_example("examples/bench_fannkuch.fj");
}

#[test]
fn bench_spectral_norm_parses() {
    parse_example("examples/bench_spectral_norm.fj");
}

#[test]
fn bench_mandelbrot_parses() {
    parse_example("examples/bench_mandelbrot.fj");
}

#[test]
fn bench_binary_trees_parses() {
    parse_example("examples/bench_binary_trees.fj");
}

#[test]
fn bench_fasta_parses() {
    parse_example("examples/bench_fasta.fj");
}

#[test]
fn bench_matrix_multiply_parses() {
    parse_example("examples/bench_matrix_multiply.fj");
}

// ════════════════════════════════════════════════════════════════════════
// 3. Benchmark programs produce output
// ════════════════════════════════════════════════════════════════════════

#[test]
fn bench_fasta_runs() {
    let output = run_example("examples/bench_fasta.fj");
    assert!(!output.is_empty(), "fasta should produce output");
    // fasta(100000) should output 100000
    assert!(output[0].contains("100000"));
}

#[test]
fn bench_binary_trees_runs() {
    let output = run_example("examples/bench_binary_trees.fj");
    assert!(!output.is_empty(), "binary-trees should produce output");
}

#[test]
fn bench_mandelbrot_runs() {
    let output = run_example("examples/bench_mandelbrot.fj");
    assert!(!output.is_empty(), "mandelbrot should produce output");
    // 200x200 mandelbrot should have >5000 pixels in set
    let count: i64 = output[0].trim().parse().unwrap_or(0);
    assert!(count > 5000, "expected >5000 pixels in set, got {count}");
}

#[test]
fn bench_matrix_multiply_runs() {
    let output = run_example("examples/bench_matrix_multiply.fj");
    assert!(!output.is_empty(), "matrix-multiply should produce output");
}

// ════════════════════════════════════════════════════════════════════════
// 4. Benchmark framework
// ════════════════════════════════════════════════════════════════════════

#[test]
fn benchmark_result_format() {
    let r = BenchmarkResult::new("test", std::time::Duration::from_millis(50), 5);
    assert_eq!(r.iterations, 5);
    assert!((r.ms_per_iter() - 10.0).abs() < 0.1);
}

#[test]
fn benchmark_comparison_speedup() {
    let b = BenchmarkResult::new("x", std::time::Duration::from_millis(100), 1);
    let c = BenchmarkResult::new("x", std::time::Duration::from_millis(50), 1);
    let cmp = BenchmarkComparison::compare(b, c);
    assert!(cmp.is_improvement());
    assert!((cmp.speedup - 2.0).abs() < 0.1);
}

#[test]
fn benchmark_suite_add() {
    let mut suite = BenchmarkSuite::new("game");
    suite.add(BenchmarkResult::new(
        "a",
        std::time::Duration::from_millis(1),
        1,
    ));
    suite.add(BenchmarkResult::new(
        "b",
        std::time::Duration::from_millis(2),
        1,
    ));
    assert_eq!(suite.count(), 2);
}

#[test]
fn benchmark_programs_list_complete() {
    assert!(BENCHMARK_PROGRAMS.len() >= 7);
    for (name, path) in BENCHMARK_PROGRAMS {
        assert!(
            Path::new(path).exists(),
            "benchmark program '{name}' missing at {path}"
        );
    }
}

#[test]
fn compile_speed_smoke() {
    let r = measure_compile_speed("fn main() { let x = 42\n println(x) }");
    assert!(r.duration.as_nanos() > 0);
}

// ════════════════════════════════════════════════════════════════════════
// 5. Benchmark program list
// ════════════════════════════════════════════════════════════════════════

#[test]
fn all_benchmark_programs_parse() {
    for (name, path) in BENCHMARK_PROGRAMS {
        let source =
            std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{name} ({path}): {e}"));
        let tokens = fajar_lang::lexer::tokenize(&source)
            .unwrap_or_else(|e| panic!("{name} lex failed: {e:?}"));
        let _program = fajar_lang::parser::parse(tokens)
            .unwrap_or_else(|e| panic!("{name} parse failed: {e:?}"));
    }
}
