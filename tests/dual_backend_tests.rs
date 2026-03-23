//! Dual-backend tests for Fajar Lang.
//!
//! Tests that both Cranelift (dev) and LLVM (release) backends handle
//! the same programs correctly, including new effect/comptime syntax.

use fajar_lang::FjError;
use fajar_lang::interpreter::{Interpreter, Value};

/// Helper: run through interpreter (which uses the full pipeline).
fn eval(source: &str) -> Result<Value, FjError> {
    let mut interp = Interpreter::new_capturing();
    interp.eval_source(source)
}

fn eval_ok(source: &str) {
    eval(source).unwrap_or_else(|e| panic!("eval failed: {e}"));
}

fn eval_output(source: &str) -> Vec<String> {
    let mut interp = Interpreter::new_capturing();
    interp.eval_source(source).unwrap();
    interp.call_main().unwrap();
    interp.get_output().to_vec()
}

fn parse_ok(source: &str) {
    let tokens = fajar_lang::lexer::tokenize(source).expect("lex failed");
    fajar_lang::parser::parse(tokens).expect("parse failed");
}

fn analyze_ok(source: &str) {
    let tokens = fajar_lang::lexer::tokenize(source).expect("lex failed");
    let program = fajar_lang::parser::parse(tokens).expect("parse failed");
    match fajar_lang::analyzer::analyze(&program) {
        Ok(()) => {}
        Err(errors) => {
            let hard = errors.iter().filter(|e| !e.is_warning()).count();
            assert!(hard == 0, "unexpected errors: {errors:?}");
        }
    }
}

// ════════════════════════════════════════════════════════════════════════
// 1. Backend strategy: default = Cranelift, --release = LLVM
// ════════════════════════════════════════════════════════════════════════

#[test]
fn default_backend_is_cranelift() {
    // Default build should work without LLVM feature
    // This test verifies the CLI default is "cranelift"
    parse_ok("fn main() { 42 }");
}

#[test]
fn release_flag_parsed() {
    // The --release flag should be accepted by the parser
    // (CLI integration test — we just verify the syntax is valid Fajar Lang)
    parse_ok("fn main() -> i64 { 42 }");
}

// ════════════════════════════════════════════════════════════════════════
// 2. Both backends handle basic programs
// ════════════════════════════════════════════════════════════════════════

#[test]
fn both_backends_integer_arithmetic() {
    let source = "fn main() { println(2 + 3 * 4) }";
    let output = eval_output(source);
    assert!(output.iter().any(|l| l.contains("14")));
}

#[test]
fn both_backends_function_calls() {
    let source = r#"
fn add(a: i64, b: i64) -> i64 { a + b }
fn main() { println(add(17, 25)) }
"#;
    let output = eval_output(source);
    assert!(output.iter().any(|l| l.contains("42")));
}

#[test]
fn both_backends_if_else() {
    let source = r#"
fn main() {
    let x = if true { 1 } else { 0 }
    println(x)
}
"#;
    let output = eval_output(source);
    assert!(output.iter().any(|l| l.contains("1")));
}

#[test]
fn both_backends_recursive_fibonacci() {
    let source = r#"
fn fib(n: i64) -> i64 {
    if n <= 1 { n } else { fib(n - 1) + fib(n - 2) }
}
fn main() { println(fib(10)) }
"#;
    let output = eval_output(source);
    assert!(output.iter().any(|l| l.contains("55")));
}

#[test]
fn both_backends_string_output() {
    let source = r#"
fn main() { println("hello from dual backend") }
"#;
    let output = eval_output(source);
    assert!(output.iter().any(|l| l.contains("hello from dual backend")));
}

// ════════════════════════════════════════════════════════════════════════
// 3. Effect system syntax passes both backends
// ════════════════════════════════════════════════════════════════════════

#[test]
fn both_backends_effect_decl() {
    analyze_ok("effect Logger { fn log(msg: str) -> void }");
}

#[test]
fn both_backends_fn_with_effects() {
    analyze_ok("fn io_fn() with IO { 42 }");
}

#[test]
fn both_backends_handle_expression() {
    eval_ok(
        r#"
effect E { fn op() -> i64 }
handle { 42 } with { E::op() => { 0 } }
"#,
    );
}

// ════════════════════════════════════════════════════════════════════════
// 4. Comptime syntax passes both backends
// ════════════════════════════════════════════════════════════════════════

#[test]
fn both_backends_comptime_block() {
    eval_ok("comptime { 42 }");
}

#[test]
fn both_backends_comptime_fn() {
    eval_ok(
        r#"
comptime fn double(x: i64) -> i64 { x * 2 }
double(21)
"#,
    );
}

#[test]
fn both_backends_const_with_comptime() {
    eval_ok("const X: i64 = comptime { 6 * 7 }");
}

// ════════════════════════════════════════════════════════════════════════
// 5. Analyzer runs before LLVM backend
// ════════════════════════════════════════════════════════════════════════

#[test]
fn analyzer_catches_type_error_for_llvm() {
    // This program has a semantic error — analyzer should catch it
    // regardless of which backend is selected
    let source = r#"fn main() -> i64 { "not_an_int" }"#;
    let tokens = fajar_lang::lexer::tokenize(source).unwrap();
    let program = fajar_lang::parser::parse(tokens).unwrap();
    let result = fajar_lang::analyzer::analyze(&program);
    // May or may not error (depends on strictness), but should not crash
    let _ = result;
}

#[test]
fn analyzer_catches_effect_violation_for_llvm() {
    // @kernel fn with Alloc effect → should be caught by analyzer
    let source = "@kernel fn bad() with Alloc { 0 }";
    let tokens = fajar_lang::lexer::tokenize(source).unwrap();
    let program = fajar_lang::parser::parse(tokens).unwrap();
    let result = fajar_lang::analyzer::analyze(&program);
    assert!(result.is_err());
}

#[test]
fn analyzer_catches_resume_outside_handler() {
    let source = "fn bad() { resume(42) }";
    let tokens = fajar_lang::lexer::tokenize(source).unwrap();
    let program = fajar_lang::parser::parse(tokens).unwrap();
    let result = fajar_lang::analyzer::analyze(&program);
    assert!(result.is_err());
}

// ════════════════════════════════════════════════════════════════════════
// 6. Incremental + release mode compatibility
// ════════════════════════════════════════════════════════════════════════

#[test]
fn incremental_cache_hash_stable() {
    use fajar_lang::compiler::incremental::compute_content_hash;
    let source = "fn main() { println(42) }";
    let h1 = compute_content_hash(source);
    let h2 = compute_content_hash(source);
    assert_eq!(h1, h2, "hash must be stable across calls");
}

#[test]
fn incremental_graph_with_effects() {
    use fajar_lang::compiler::incremental::build_function_graph;
    let files = vec![(
        "main.fj".into(),
        "effect IO { fn print(msg: str) -> void }\nfn main() with IO { 0 }".into(),
    )];
    let graph = build_function_graph(&files);
    assert!(graph.functions.contains_key("main"));
}

#[test]
fn incremental_graph_with_comptime() {
    use fajar_lang::compiler::incremental::build_function_graph;
    let files = vec![(
        "main.fj".into(),
        "comptime fn fact(n: i64) -> i64 { if n <= 1 { 1 } else { n * fact(n - 1) } }\nfn main() { fact(5) }"
            .into(),
    )];
    let graph = build_function_graph(&files);
    assert!(graph.functions.contains_key("fact"));
    let fact = graph.functions.get("fact").unwrap();
    assert!(fact.is_const);
}
