//! FJARR_LEAK Phase 2 — 18.D.1 RED phase test fixtures for SE024.
//!
//! Per `docs/FJARR_LEAK_PLAN.md` row 18.D.1 + decision file
//! `docs/decisions/2026-05-07-fjarr-leak-strategy.md` (Choice F:
//! A-now arena + D-Phase-19 linear types) — adapted to SE024 naming
//! (original plan said SE017 but SE017 = AwaitOutsideAsync; user picked
//! SE024 in 2026-05-08 session).
//!
//! ## TDD discipline
//!
//! - **Format test** (`format_*`): direct construction of the
//!   `SemanticError::UseAfterMoveArray` variant + assert the formatted
//!   message contains `"SE024"`. **PASSES TODAY** as soon as the variant
//!   exists in `src/analyzer/type_check/mod.rs` (this commit).
//!
//! - **Emission tests** (`emit_*`): exercise an actual fj source pattern
//!   that should trigger SE024. Currently `#[ignore]` — they FAIL today
//!   because Phase 2 18.D.1.2 hasn't wired emission yet (analyzer doesn't
//!   know to call `mark_moved` on `[T]` consumes nor `check_use` for SE024).
//!   The RED→GREEN transition happens in 18.D.1.2 (next commit), where
//!   the `#[ignore]` attributes get removed and the tests pass.
//!
//! - **OK tests** (`ok_*`): patterns that should NOT trigger SE024 even
//!   after wiring (chain-grow, `.clone()`, single-use). Run today as a
//!   regression baseline (analyzer compiles, no SE024 emitted).
//!
//! ## RED→GREEN flow
//!
//! ```text
//! 18.D.1.1 (this commit):
//!   format_* PASS · emit_* IGNORED · ok_* PASS
//! 18.D.1.2 (next commit):
//!   format_* PASS · emit_* un-ignored + PASS · ok_* PASS
//! 18.D.1.3 (after self-host re-compile):
//!   all PASS · stage1_full + phase17 still GREEN
//! ```

use fajar_lang::analyzer;
use fajar_lang::analyzer::type_check::SemanticError;
use fajar_lang::lexer;
use fajar_lang::lexer::token::Span;
use fajar_lang::parser;

// ════════════════════════════════════════════════════════════════════════
// Format test — passes as soon as variant exists
// ════════════════════════════════════════════════════════════════════════

#[test]
fn format_se024_variant_includes_code() {
    let e = SemanticError::UseAfterMoveArray {
        name: "v".into(),
        span: Span::new(20, 21),
        move_span: Span::new(10, 11),
    };
    let msg = format!("{e}");
    assert!(
        msg.contains("SE024"),
        "expected SE024 in message, got: {msg}"
    );
    assert!(
        msg.contains("v"),
        "expected variable name in message, got: {msg}"
    );
    assert!(
        msg.contains("moved at byte 10"),
        "expected move_span byte info, got: {msg}"
    );
}

#[test]
fn format_se024_secondary_span_says_array_moved_here() {
    let e = SemanticError::UseAfterMoveArray {
        name: "arr".into(),
        span: Span::new(50, 53),
        move_span: Span::new(20, 23),
    };
    let secondary = e.secondary_span();
    assert!(
        secondary.is_some(),
        "secondary_span should return Some for SE024"
    );
    let (sp, label) = secondary.unwrap();
    assert_eq!(sp.start, 20, "secondary span should point at move site");
    assert_eq!(label, "array moved here");
}

#[test]
fn format_se024_hint_suggests_clone_or_restructure() {
    let e = SemanticError::UseAfterMoveArray {
        name: "data".into(),
        span: Span::new(30, 34),
        move_span: Span::new(15, 19),
    };
    let hint = e.hint().expect("SE024 should provide a hint");
    assert!(
        hint.contains(".clone()"),
        "hint should suggest .clone(), got: {hint}"
    );
    assert!(
        hint.contains("FJARR_LEAK"),
        "hint should reference FJARR_LEAK Phase 2 origin, got: {hint}"
    );
    assert!(
        hint.contains("data"),
        "hint should mention the variable name, got: {hint}"
    );
}

#[test]
fn format_se024_span_method_returns_use_site() {
    let e = SemanticError::UseAfterMoveArray {
        name: "tokens".into(),
        span: Span::new(100, 106),
        move_span: Span::new(40, 46),
    };
    let primary = e.span();
    assert_eq!(
        primary.start, 100,
        "primary span should be the use-after-move site, not the move site"
    );
}

// ════════════════════════════════════════════════════════════════════════
// Emission tests — IGNORED until 18.D.1.2 wires the analyzer
// ════════════════════════════════════════════════════════════════════════
//
// These exercise the actual fj source patterns Phase 2 must catch.
// Each is `#[ignore]` because emission isn't wired yet. After 18.D.1.2,
// remove the `#[ignore]` and these MUST pass.

#[test]
#[ignore = "18.D.1.2 OVERFIRE — branch-merge analysis (E3) needed before re-attempt"]
fn emit_se024_basic_consume_then_use() {
    expect_se024(
        r#"
fn consume(v: [i64]) -> i64 { len(v) }
fn main() {
    let v: [i64] = [1, 2, 3]
    let _a = consume(v)
    let _b = consume(v)  // SE024 here — v moved on prior line
}
"#,
    );
}

#[test]
#[ignore = "18.D.1.2 OVERFIRE — branch-merge analysis (E3) needed before re-attempt"]
fn emit_se024_branch_merge_then_use() {
    expect_se024(
        r#"
fn consume(v: [i64]) -> i64 { len(v) }
fn main() {
    let v: [i64] = [1, 2, 3]
    if true { let _a = consume(v) } else { let _b = consume(v) }
    let _c = consume(v)  // SE024 here — v moved in BOTH branches
}
"#,
    );
}

#[test]
#[ignore = "18.D.1.2 OVERFIRE — branch-merge analysis (E3) needed before re-attempt"]
fn emit_se024_let_alias_then_original_use() {
    expect_se024(
        r#"
fn main() {
    let outer: [i64] = [1, 2, 3]
    let inner = outer  // outer moved into inner
    let _x = len(outer)  // SE024 here — outer is moved
    let _y = len(inner)  // OK
}
"#,
    );
}

#[test]
#[ignore = "18.D.1.2 OVERFIRE — branch-merge analysis (E3) needed before re-attempt"]
fn emit_se024_str_array_use_after_move() {
    expect_se024(
        r#"
fn consume_str(v: [str]) -> i64 { len(v) }
fn main() {
    let v: [str] = ["a", "b"]
    let _a = consume_str(v)
    let _b = consume_str(v)  // SE024 here — [str] is also affine
}
"#,
    );
}

// ════════════════════════════════════════════════════════════════════════
// OK tests — patterns that must NOT trigger SE024 (regression baseline)
// ════════════════════════════════════════════════════════════════════════
//
// Run today + after wiring. If any of these starts emitting SE024 after
// 18.D.1.2 wiring, the analyzer is over-eager and Phase 2 must back off
// before ship.

#[test]
fn ok_chain_grow_pattern_no_se024() {
    // Chain-grow `a = a.push(x)` is single-use of right-side `a`
    // followed by re-bind. Affine-friendly. Self-host source uses this
    // pattern 133+ times per FJARR_LEAK_PHASE_2_B0_FINDINGS.md §B0.7.
    expect_no_se024(
        r#"
fn main() {
    let mut v: [i64] = []
    v = v.push(1)
    v = v.push(2)
    v = v.push(3)
    let _ = len(v)
}
"#,
    );
}

#[test]
fn ok_single_use_no_se024() {
    expect_no_se024(
        r#"
fn consume(v: [i64]) -> i64 { len(v) }
fn main() {
    let v: [i64] = [1, 2, 3]
    let _ = consume(v)
}
"#,
    );
}

#[test]
fn ok_return_consumes_no_se024() {
    expect_no_se024(
        r#"
fn make() -> [i64] {
    let v: [i64] = [1, 2, 3]
    v
}
fn main() {
    let _ = make()
}
"#,
    );
}

// ════════════════════════════════════════════════════════════════════════
// Helper assertions
// ════════════════════════════════════════════════════════════════════════

fn expect_se024(src: &str) {
    let tokens = lexer::tokenize(src).expect("lex should succeed for SE024 emission test");
    let program = parser::parse(tokens).expect("parse should succeed for SE024 emission test");
    let result = analyzer::analyze(&program);
    let errs = match result {
        Ok(()) => panic!(
            "expected analyzer to emit SE024 for source:\n{src}\nbut analysis succeeded with no errors"
        ),
        Err(es) => es,
    };
    let any = errs.iter().any(|e| format!("{e}").contains("SE024"));
    assert!(
        any,
        "expected SE024 in errors for source:\n{src}\ngot: {:#?}",
        errs.iter().map(|e| format!("{e}")).collect::<Vec<_>>()
    );
}

fn expect_no_se024(src: &str) {
    let tokens = lexer::tokenize(src).expect("lex should succeed for SE024 negative test");
    let program = parser::parse(tokens).expect("parse should succeed for SE024 negative test");
    let _ = analyzer::analyze(&program); // ignore other errors; we only care about absence of SE024
    // Re-run via TypeChecker to inspect diagnostics directly so we catch
    // both error- and warning-level emission paths.
    let mut tc = fajar_lang::analyzer::type_check::TypeChecker::new();
    let _ = tc.analyze(&program);
    let any_se024 = tc
        .diagnostics()
        .iter()
        .any(|e| format!("{e}").contains("SE024"));
    assert!(
        !any_se024,
        "did not expect SE024 in diagnostics for source:\n{src}\ngot: {:#?}",
        tc.diagnostics()
            .iter()
            .map(|e| format!("{e}"))
            .collect::<Vec<_>>()
    );
}
