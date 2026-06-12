//! Tensor shape compile-time probe corpus — P4.2 of TENSOR_SHAPE_CT_PLAN.
//!
//! Permanent regression harness for the Compass §6.3 track (P1-P3): every
//! probe from `docs/TENSOR_SHAPE_CT_B0_FINDINGS.md` plus the P2/P3 session
//! probes, exercised end-to-end through lexer → parser → analyzer exactly
//! as `fj check` does. Runs in the ci.yml main job via `cargo test --tests`
//! (P4.4 prevention layer — §6.8 R3).
//!
//! History: before P1 (commit e02ca768), probes B0-P1 and B0-P4 PASSED
//! `fj check` and only failed at runtime; before P2 (12d4effd), explicit
//! `return` statements were never type-checked at all; P3 (35e6fa96) added
//! symbolic dims + TE011.

use fajar_lang::analyzer;
use fajar_lang::lexer;
use fajar_lang::parser;

/// Run the `fj check` pipeline; return Err(error strings) when analysis fails.
fn check(src: &str) -> Result<(), Vec<String>> {
    let tokens = lexer::tokenize(src).expect("lex");
    let program = parser::parse(tokens).expect("parse");
    analyzer::analyze(&program).map_err(|es| es.iter().map(|e| format!("{e}")).collect())
}

fn expect_error_containing(src: &str, needle: &str) {
    let errs = check(src).expect_err(&format!("expected '{needle}' error, got clean: {src:?}"));
    assert!(
        errs.iter().any(|e| e.contains(needle)),
        "expected an error containing '{needle}', got: {errs:#?}"
    );
}

// ════════════════════════════════════════════════════════════════════════
// B0 probes (docs/TENSOR_SHAPE_CT_B0_FINDINGS.md §2)
// ════════════════════════════════════════════════════════════════════════

#[test]
fn b0_p1_unannotated_matmul_mismatch_red_at_check() {
    // THE original gap: passed `fj check`, died at runtime pre-P1.
    expect_error_containing(
        "@device fn bad() -> void {\n    let a = zeros(2, 3)\n    let b = zeros(4, 5)\n    let c = matmul(a, b)\n    println(c)\n}",
        "TE001",
    );
}

#[test]
fn b0_p3_element_type_at_param_boundary() {
    // f32 param vs f64 constructor default.
    expect_error_containing(
        "fn take(t: Tensor<f32>) -> void { println(t) }\ntake(zeros(2, 2))",
        "SE004",
    );
}

#[test]
fn b0_p4_shape_at_param_boundary_red_since_p1() {
    expect_error_containing(
        "fn take(t: Tensor<f64>[3, 3]) -> void { println(t) }\ntake(zeros(2, 2))",
        "SE004",
    );
}

#[test]
fn b0_p5_annotated_at_operator_mismatch() {
    expect_error_containing(
        "let a: Tensor<f64>[2, 3] = zeros(2, 3)\nlet b: Tensor<f64>[4, 5] = zeros(4, 5)\nlet c = a @ b",
        "TE001",
    );
}

#[test]
fn b0_p6_known_vs_known_assignment() {
    expect_error_containing(
        "let a: Tensor<f64>[2, 3] = zeros(2, 3)\nlet b: Tensor<f64>[9, 9] = a",
        "SE004",
    );
}

// ════════════════════════════════════════════════════════════════════════
// P1 — shape-aware builtins
// ════════════════════════════════════════════════════════════════════════

#[test]
fn p1_chain_propagation() {
    expect_error_containing(
        "let c = matmul(zeros(2, 3), zeros(3, 4))\nlet d = matmul(c, zeros(5, 6))",
        "TE001",
    );
}

#[test]
fn p1_reshape_element_count() {
    expect_error_containing("let r = reshape(zeros(2, 3), [4, 2])", "TE001");
}

#[test]
fn p1_transpose_and_activation_propagate() {
    expect_error_containing(
        "let c = matmul(transpose(zeros(3, 2)), zeros(2, 7))",
        "TE001",
    );
    expect_error_containing("let c = matmul(relu(zeros(2, 3)), zeros(4, 5))", "TE001");
}

#[test]
fn p1_elementwise_call_mismatch() {
    expect_error_containing("let c = tensor_add(zeros(2, 3), zeros(2, 4))", "TE001");
}

#[test]
fn p1_dynamic_dims_stay_gradual() {
    assert!(
        check(
            "fn f(n: i64) -> void {\n    let a = zeros(n, 3)\n    let c = matmul(a, zeros(9, 9))\n    println(c)\n}"
        )
        .is_ok()
    );
}

// ════════════════════════════════════════════════════════════════════════
// P2 — return-statement enforcement (general soundness fix)
// ════════════════════════════════════════════════════════════════════════

#[test]
fn p2_return_stmt_scalar_mismatch() {
    expect_error_containing("fn f() -> i64 {\n    return \"hello\"\n}", "SE004");
}

#[test]
fn p2_return_stmt_shape_mismatch_through_branch() {
    expect_error_containing(
        "fn f(x: i64) -> Tensor<f64>[2, 3] {\n    if x > 0 {\n        return zeros(9, 9)\n    }\n    zeros(2, 3)\n}",
        "SE004",
    );
}

#[test]
fn p2_matching_returns_clean() {
    assert!(
        check(
            "fn f(x: i64) -> Tensor<f64>[2, 3] {\n    if x > 0 {\n        return zeros(2, 3)\n    }\n    zeros(2, 3)\n}"
        )
        .is_ok()
    );
}

// ════════════════════════════════════════════════════════════════════════
// P3 — symbolic dims (TE011)
// ════════════════════════════════════════════════════════════════════════

const DENSE: &str = "fn dense(x: Tensor<f64>[B, I], w: Tensor<f64>[I, O]) -> Tensor<f64>[B, O] {\n    matmul(x, w)\n}\n";

#[test]
fn p3_symbolic_conflict_te011() {
    expect_error_containing(
        &format!("{DENSE}let r = dense(zeros(4, 10), zeros(11, 2))"),
        "TE011",
    );
}

#[test]
fn p3_return_substitution_propagates() {
    // dense → [4, 2]; @ [9, 9] must fail downstream.
    expect_error_containing(
        &format!("{DENSE}let r = dense(zeros(4, 10), zeros(10, 2))\nlet bad = r @ zeros(9, 9)"),
        "TE001",
    );
}

#[test]
fn p3_symbolic_clean_and_gradual() {
    assert!(
        check(&format!(
            "{DENSE}let r = dense(zeros(4, 10), zeros(10, 2))\nprintln(r)"
        ))
        .is_ok()
    );
    assert!(
        check(&format!(
            "{DENSE}fn caller(n: i64) -> void {{\n    let r = dense(zeros(n, 10), zeros(10, 2))\n    println(r)\n}}"
        ))
        .is_ok()
    );
}

// ════════════════════════════════════════════════════════════════════════
// P4 — domain hints (golden-lite: hint text present on shape errors)
// ════════════════════════════════════════════════════════════════════════

#[test]
fn p4_te011_hint_names_the_symbol() {
    let tokens =
        lexer::tokenize(&format!("{DENSE}let r = dense(zeros(4, 10), zeros(11, 2))")).expect("lex");
    let program = parser::parse(tokens).expect("parse");
    let errs = analyzer::analyze(&program).expect_err("expected TE011");
    let hint = errs
        .iter()
        .find_map(|e| e.hint().filter(|h| h.contains("`I`")))
        .expect("TE011 hint should name the symbol");
    assert!(hint.contains("same size"), "got hint: {hint}");
}

#[test]
fn p4_te001_hint_mentions_constructors() {
    let tokens = lexer::tokenize("let c = matmul(zeros(2, 3), zeros(4, 5))").expect("lex");
    let program = parser::parse(tokens).expect("parse");
    let errs = analyzer::analyze(&program).expect_err("expected TE001");
    let hint = errs
        .iter()
        .find_map(|e| e.hint())
        .expect("TE001 should carry a domain hint");
    assert!(hint.contains("zeros(2, 3)"), "got hint: {hint}");
}
