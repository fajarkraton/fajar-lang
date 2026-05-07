//! T4 dup-fn / dup-struct / dup-enum / dup-const detection tests.
//!
//! Verifies `analyze_tokens_with_spans` reports `ERR_DUPLICATE_DEF`
//! when source has multiple `fn name`/`struct N`/`enum N`/`const N`
//! declarations with the same name, AND that `let_shadow_fn` does
//! NOT fire (separate namespaces) while `let_shadow_let_in_same_scope`
//! does fire.
//!
//! Pattern: concatenate `stdlib/lexer.fj` + `stdlib/analyzer.fj` +
//! a `fn main()` driver that calls `tokenize_with_spans` →
//! `analyze_tokens_with_spans`, then `println` the error count and
//! the formatted error name. Run via in-process Interpreter and
//! assert on captured output.
//!
//! Source of truth: `docs/T4_DUP_FN_PLAN.md` §3.1 + resume protocol
//! `memory/project_resume_lanjut_protocol.md` step T4 A4.

use fajar_lang::interpreter::Interpreter;

fn load_stdlib(name: &str) -> String {
    std::fs::read_to_string(format!("stdlib/{name}"))
        .unwrap_or_else(|e| panic!("cannot read stdlib/{name}: {e}"))
}

/// Build combined fj source: lexer.fj + analyzer.fj + a driver
/// that exercises analyze_tokens_with_spans on `src` and prints
/// `error_count` then (if >0) `format_error` of the first error.
/// Returns captured `println` output lines.
fn run_driver(src: &str) -> Vec<String> {
    let lexer = load_stdlib("lexer.fj");
    let analyzer = load_stdlib("analyzer.fj");
    let escaped = src.replace('\\', "\\\\").replace('"', "\\\"");
    let driver = format!(
        r#"
fn main() {{
    let src = "{escaped}"
    let spans = tokenize_with_spans(src)
    let state = analyze_tokens_with_spans(spans, src)
    let n = error_count(state)
    println(to_string(n))
    if n >= 1 {{
        println(format_error(state.errors[0], state.error_names[0]))
    }}
}}
"#
    );
    let combined = format!("{lexer}\n{analyzer}\n{driver}");

    let mut interp = Interpreter::new_capturing();
    interp.eval_source(&combined).unwrap_or_else(|e| {
        panic!("eval_source failed: {e:?}\n--- combined source ---\n{combined}")
    });
    interp.call_main().expect("call_main");
    interp.get_output().to_vec()
}

fn parse_count(out: &[String]) -> i64 {
    out.first()
        .and_then(|s| s.trim().parse::<i64>().ok())
        .unwrap_or_else(|| panic!("first output line not an int: {out:?}"))
}

#[test]
fn dup_fn_simple_two_decls() {
    let out = run_driver("fn f(){} fn f(){}");
    let n = parse_count(&out);
    assert!(n >= 1, "expected >=1 dup error, got {n}; output: {out:?}");
    assert!(
        out.iter()
            .any(|line| line.contains("SE006: duplicate definition 'f'")),
        "expected SE006 'f' message, got: {out:?}"
    );
}

#[test]
fn dup_fn_with_pub_modifier() {
    let out = run_driver("pub fn f(){} fn f(){}");
    let n = parse_count(&out);
    assert!(n >= 1, "expected >=1 dup error, got {n}; output: {out:?}");
    assert!(
        out.iter()
            .any(|line| line.contains("SE006: duplicate definition 'f'")),
        "expected SE006 'f' message, got: {out:?}"
    );
}

#[test]
fn dup_fn_disjoint_does_not_fire() {
    let out = run_driver("fn f(){} fn g(){}");
    let n = parse_count(&out);
    assert_eq!(
        n, 0,
        "expected 0 errors for disjoint fns, got {n}; output: {out:?}"
    );
}

#[test]
fn dup_struct() {
    let out = run_driver("struct A{} struct A{}");
    let n = parse_count(&out);
    assert!(
        n >= 1,
        "expected >=1 dup error for struct, got {n}; output: {out:?}"
    );
    assert!(
        out.iter()
            .any(|line| line.contains("SE006: duplicate definition 'A'")),
        "expected SE006 'A' message, got: {out:?}"
    );
}

#[test]
fn dup_enum() {
    let out = run_driver("enum E{} enum E{}");
    let n = parse_count(&out);
    assert!(
        n >= 1,
        "expected >=1 dup error for enum, got {n}; output: {out:?}"
    );
    assert!(
        out.iter()
            .any(|line| line.contains("SE006: duplicate definition 'E'")),
        "expected SE006 'E' message, got: {out:?}"
    );
}

#[test]
fn dup_const() {
    let out = run_driver("const X: i64 = 1 const X: i64 = 2");
    let n = parse_count(&out);
    assert!(
        n >= 1,
        "expected >=1 dup error for const, got {n}; output: {out:?}"
    );
    assert!(
        out.iter()
            .any(|line| line.contains("SE006: duplicate definition 'X'")),
        "expected SE006 'X' message, got: {out:?}"
    );
}

#[test]
fn let_shadow_fn_does_not_fire() {
    // fn name lives in fn_names; let name lives in var_names — disjoint.
    let out = run_driver("fn x(){} let x = 1");
    let n = parse_count(&out);
    assert_eq!(
        n, 0,
        "expected 0 errors (separate namespaces), got {n}; output: {out:?}"
    );
}

#[test]
fn let_shadow_let_in_same_scope_fires() {
    let out = run_driver("let x = 1; let x = 2");
    let n = parse_count(&out);
    assert!(
        n >= 1,
        "expected >=1 dup error for two lets, got {n}; output: {out:?}"
    );
    assert!(
        out.iter()
            .any(|line| line.contains("SE006: duplicate definition 'x'")),
        "expected SE006 'x' message, got: {out:?}"
    );
}
