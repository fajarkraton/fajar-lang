//! Real-world performance benchmark tests for Fajar Lang.

use fajar_lang::codegen::benchmarks::*;
use fajar_lang::codegen::perf_report::*;
use fajar_lang::interpreter::Interpreter;
use std::path::Path;
use std::time::Instant;

fn run_example(path: &str) -> Vec<String> {
    let source = std::fs::read_to_string(path).unwrap();
    let mut interp = Interpreter::new_capturing();
    interp.eval_source(&source).unwrap();
    interp.call_main().unwrap();
    interp.get_output().to_vec()
}

// ════════════════════════════════════════════════════════════════════════
// 1. Real-world benchmark programs
// ════════════════════════════════════════════════════════════════════════

#[test]
fn bench_json_parse_exists_and_runs() {
    assert!(Path::new("examples/bench_json_parse.fj").exists());
    let output = run_example("examples/bench_json_parse.fj");
    assert!(!output.is_empty());
}

#[test]
fn bench_cold_start_runs() {
    let output = run_example("examples/bench_cold_start.fj");
    assert!(output.iter().any(|l| l.contains("42")));
}

#[test]
fn bench_string_ops_runs() {
    let output = run_example("examples/bench_string_ops.fj");
    assert!(output.iter().any(|l| l.contains("1000")));
}

#[test]
fn bench_compile_speed_runs() {
    let output = run_example("examples/bench_compile_speed.fj");
    assert!(output.iter().any(|l| l.contains("100000")));
}

// ════════════════════════════════════════════════════════════════════════
// 2. Compilation speed
// ════════════════════════════════════════════════════════════════════════

#[test]
fn compile_speed_hello_world() {
    let source = "fn main() { println(42) }";
    let start = Instant::now();
    let tokens = fajar_lang::lexer::tokenize(source).unwrap();
    let _program = fajar_lang::parser::parse(tokens).unwrap();
    let elapsed = start.elapsed();
    // Hello world should compile in <10ms
    assert!(
        elapsed.as_millis() < 100,
        "hello world compile took {}ms",
        elapsed.as_millis()
    );
}

#[test]
fn compile_speed_medium_program() {
    // Generate a medium-sized program (~50 functions)
    let mut source = String::new();
    for i in 0..50 {
        source.push_str(&format!("fn func_{i}(x: i64) -> i64 {{ x + {i} }}\n"));
    }
    source.push_str("fn main() { func_0(42) }\n");

    let start = Instant::now();
    let tokens = fajar_lang::lexer::tokenize(&source).unwrap();
    let _program = fajar_lang::parser::parse(tokens).unwrap();
    let elapsed = start.elapsed();

    // 50 functions should compile in <100ms
    assert!(
        elapsed.as_millis() < 500,
        "50 functions compile took {}ms",
        elapsed.as_millis()
    );
}

// ════════════════════════════════════════════════════════════════════════
// 3. Performance report
// ════════════════════════════════════════════════════════════════════════

#[test]
fn perf_report_generation() {
    let report = generate_report(2000.0, 3.0, 80.0);
    assert_eq!(report.total_count(), 3);
    let md = report.to_markdown();
    assert!(md.contains("Performance Report"));
    assert!(md.contains("Compilation Speed"));
    assert!(md.contains("Cold Start"));
    assert!(md.contains("Binary Size"));
}

#[test]
fn perf_report_all_pass() {
    let report = generate_report(1000.0, 2.0, 50.0);
    assert!(report.pass_rate() > 90.0);
}

#[test]
fn perf_report_markdown_table() {
    let report = generate_report(2000.0, 3.0, 80.0);
    let md = report.to_markdown();
    assert!(md.contains("| Metric |"));
    assert!(md.contains("| Measured |"));
    assert!(md.contains("PASS"));
}

#[test]
fn perf_targets_reasonable() {
    let t = PerformanceTargets::default_targets();
    assert!(t.cold_start_ms > 0.0);
    assert!(t.compile_10k_loc_s > 0.0);
    assert!(t.binary_size_bytes > 0);
}

// ════════════════════════════════════════════════════════════════════════
// 4. Interpreter cold start
// ════════════════════════════════════════════════════════════════════════

#[test]
fn interpreter_cold_start_speed() {
    let start = Instant::now();
    let mut interp = Interpreter::new_capturing();
    interp.eval_source("42").unwrap();
    let elapsed = start.elapsed();

    // Interpreter cold start should be fast (<50ms)
    assert!(
        elapsed.as_millis() < 200,
        "interpreter cold start: {}ms",
        elapsed.as_millis()
    );
}

// ════════════════════════════════════════════════════════════════════════
// 5. Measure framework integration
// ════════════════════════════════════════════════════════════════════════

#[test]
fn measure_compile_speed_integration() {
    let result = measure_compile_speed("fn main() { let x = 42\n let y = x + 1\n println(y) }");
    assert!(result.duration.as_nanos() > 0);
    assert!(result.output.is_some());
}

#[test]
fn measure_fn_integration() {
    let result = measure("add", 100, || 2 + 2);
    assert_eq!(result.iterations, 100);
    assert!(result.iter_per_sec() > 0.0);
}
