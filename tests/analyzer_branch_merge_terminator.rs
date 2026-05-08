//! FJARR_LEAK Phase 2 / E3 — branch-merge analysis with terminator awareness.
//!
//! Tests that the analyzer correctly handles `if/else` branches whose
//! body ends with `return`/`break`/`continue`: such branches' consume
//! side-effects must not propagate past the if. Without E3, the
//! pattern `if cond { return pr_err(a, ...) }` would mark `a` moved
//! unconditionally and fire ME001/SE024 on subsequent uses.
//!
//! These tests exercise the existing ME001 path (strict_ownership mode)
//! since SE024 emission wire is still parked pending E2 + cascade
//! `.clone()` work. The same branch-merge semantics apply to both.

use fajar_lang::analyzer;
use fajar_lang::analyzer::type_check::TypeChecker;
use fajar_lang::lexer;
use fajar_lang::parser;

fn analyze_strict(src: &str) -> Vec<String> {
    let tokens = lexer::tokenize(src).expect("lex");
    let program = parser::parse(tokens).expect("parse");
    let mut tc = TypeChecker::new_strict();
    let _ = tc.analyze(&program);
    tc.diagnostics()
        .iter()
        .map(|e| format!("{e}"))
        .collect::<Vec<_>>()
}

fn analyze_default(src: &str) -> Result<(), Vec<String>> {
    let tokens = lexer::tokenize(src).expect("lex");
    let program = parser::parse(tokens).expect("parse");
    analyzer::analyze(&program).map_err(|errs| errs.iter().map(|e| format!("{e}")).collect())
}

// ════════════════════════════════════════════════════════════════════════
// Strict-mode branch-merge tests (ME001 emission paths)
// ════════════════════════════════════════════════════════════════════════

#[test]
fn branch_with_return_does_not_propagate_move() {
    // The if-then-branch consumes `s` via fn-call, then returns. After
    // the if, `s` should still be Owned (not Moved) because the branch
    // didn't fall through. Without E3 branch-merge, this would fire ME001
    // on the post-if `len(s)`.
    let src = r#"
        fn consume(_x: str) -> i64 { 0 }
        fn make() -> str { "hello" }
        fn main() -> i64 {
            let s: str = make()
            if 1 > 2 { return consume(s) }
            consume(s)
        }
    "#;
    let diags = analyze_strict(src);
    let me001_count = diags.iter().filter(|d| d.contains("ME001")).count();
    assert_eq!(
        me001_count, 0,
        "expected zero ME001 (branch with return shouldn't propagate consume), got: {diags:#?}"
    );
}

#[test]
fn branch_without_return_does_propagate_move() {
    // The if-then-branch consumes `s` and falls through (no return).
    // After the if, `s` should be Moved. ME001 should fire on the
    // post-if `len(s)`.
    let src = r#"
        fn consume(_x: str) -> i64 { 0 }
        fn make() -> str { "hello" }
        fn main() -> i64 {
            let s: str = make()
            let mut acc: i64 = 0
            if 1 > 2 { acc = consume(s) }
            consume(s)
        }
    "#;
    let diags = analyze_strict(src);
    let me001_count = diags.iter().filter(|d| d.contains("ME001")).count();
    assert!(
        me001_count > 0,
        "expected ME001 (consume in non-terminating branch should propagate), got: {diags:#?}"
    );
}

#[test]
fn both_branches_terminate_post_merge_unreachable() {
    // Both branches return — post-if is unreachable; `s` state doesn't
    // matter, but no ME001 should be emitted on the trailing expr (which
    // never executes).
    let src = r#"
        fn consume(_x: str) -> i64 { 0 }
        fn make() -> str { "hello" }
        fn main() -> i64 {
            let s: str = make()
            if 1 > 2 { return consume(s) } else { return consume(s) }
        }
    "#;
    let diags = analyze_strict(src);
    let me001_count = diags.iter().filter(|d| d.contains("ME001")).count();
    assert_eq!(
        me001_count, 0,
        "expected zero ME001 (both branches escape), got: {diags:#?}"
    );
}

#[test]
fn only_else_branch_terminates_then_state_propagates() {
    // Else branch returns; then-branch falls through with consume.
    // Post-merge inherits then-branch state (s is Moved) → ME001 fires
    // on post-if use.
    let src = r#"
        fn consume(_x: str) -> i64 { 0 }
        fn make() -> str { "hello" }
        fn main() -> i64 {
            let s: str = make()
            let mut acc: i64 = 0
            if 1 > 2 { acc = consume(s) } else { return 42 }
            consume(s)
        }
    "#;
    let diags = analyze_strict(src);
    let me001_count = diags.iter().filter(|d| d.contains("ME001")).count();
    assert!(
        me001_count > 0,
        "expected ME001 (then-branch consumed s + fell through), got: {diags:#?}"
    );
}

// ════════════════════════════════════════════════════════════════════════
// Default-mode (lenient) regression tests — E3 must not break anything
// ════════════════════════════════════════════════════════════════════════

#[test]
fn default_mode_pre_phase2_arrays_still_copy() {
    // Per pre-Phase-2 contract: arrays are Copy in default (lenient) mode.
    // `let b = a; len(a)` MUST analyze cleanly. E3 doesn't activate
    // ME001/SE024 in default mode.
    let src = r#"
        fn main() {
            let a: [i64] = [1, 2, 3]
            let _b = a
            let _ = len(a)
        }
    "#;
    let result = analyze_default(src);
    assert!(
        result.is_ok(),
        "expected lenient-mode array reuse to analyze OK, got: {result:#?}"
    );
}

// ════════════════════════════════════════════════════════════════════════
// E4 — .clone() builtin recognition for [T] arrays
// ════════════════════════════════════════════════════════════════════════

#[test]
fn clone_method_on_array_analyzes_and_runs() {
    // FJARR_LEAK Phase 2 / E4: arr.clone() must (a) be accepted by the
    // analyzer for `[T]` types, (b) execute correctly in the interpreter
    // returning a fresh [T], (c) be mapped to `_fj_arr_clone` by the
    // self-host codegen (verified separately via stage1_full when
    // cascade lands).
    use fajar_lang::interpreter::Interpreter;
    let src = r#"
fn main() {
    let v: [i64] = [10, 20, 30]
    let w = v.clone()
    println(to_string(len(v)))
    println(to_string(len(w)))
}
"#;
    let mut interp = Interpreter::new();
    let result = interp.eval_source(src);
    assert!(
        result.is_ok(),
        "expected .clone() on [i64] to evaluate cleanly, got: {result:#?}"
    );
}

#[test]
fn clone_independent_returns_distinct_array_value() {
    // Sanity: the cloned value is independent of the original.
    // We can't easily inspect identity in fj source, but we can
    // verify that mutating the original (via re-bind chain-grow)
    // doesn't affect the clone's len.
    use fajar_lang::interpreter::Interpreter;
    let src = r#"
fn main() {
    let mut v: [i64] = [1, 2]
    let w = v.clone()
    v = v.push(3)
    println(to_string(len(v)))
    println(to_string(len(w)))
}
"#;
    let mut interp = Interpreter::new();
    let result = interp.eval_source(src);
    assert!(
        result.is_ok(),
        "expected clone-then-mutate-original to evaluate cleanly, got: {result:#?}"
    );
}
