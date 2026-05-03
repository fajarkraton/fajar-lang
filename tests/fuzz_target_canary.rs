//! Canary tests that exercise the same public API as each fuzz target
//! in `fuzz/fuzz_targets/`. These run on stable Rust in normal CI so we
//! catch API drift without needing `cargo +nightly fuzz build`.
//!
//! If a fuzz target's body code is wrong, the corresponding canary will
//! fail to compile or panic — surfacing the bug 60+ seconds before the
//! actual fuzz run would.
//!
//! P4.C3 of FAJAR_LANG_PERFECTION_PLAN.

use fajar_lang::analyzer;
use fajar_lang::analyzer::polonius::{FactGenerator, PoloniusSolver};
use fajar_lang::lexer;
use fajar_lang::parser;
use fajar_lang::vm::compiler::Compiler;

const SAMPLE_SOURCE: &str = "fn main() -> i64 { let x = 1\nlet y = 2\nx + y }";

#[test]
fn canary_fuzz_codegen_api() {
    // Mirror fuzz/fuzz_targets/fuzz_codegen.rs body.
    let tokens = lexer::tokenize(SAMPLE_SOURCE).expect("lex");
    let program = parser::parse(tokens).expect("parse");
    let _ = analyzer::analyze(&program);
    let _chunk = Compiler::new().compile(&program);
}

#[test]
fn canary_fuzz_borrow_api() {
    // Mirror fuzz/fuzz_targets/fuzz_borrow.rs body.
    let tokens = lexer::tokenize(SAMPLE_SOURCE).expect("lex");
    let program = parser::parse(tokens).expect("parse");
    let facts = FactGenerator::new().generate(&program);
    let _result = PoloniusSolver::with_max_iterations(200).solve(&facts);
}

#[test]
fn canary_fuzz_async_api() {
    // Mirror fuzz/fuzz_targets/fuzz_async.rs body. Wraps body in async fn.
    let body = "let _ = 1";
    let source = format!("async fn _fuzz_async_target() {{ {body} }}");
    let tokens = lexer::tokenize(&source).expect("lex");
    let program = parser::parse(tokens).expect("parse");
    let _ = analyzer::analyze(&program);
}

#[test]
fn canary_fuzz_codegen_does_not_panic_on_garbage() {
    // Garbage input must NOT panic at any pipeline stage.
    let inputs = ["", "@@@@", "fn", "fn f() { let x = }", "0xZZ"];
    for src in &inputs {
        if let Ok(tokens) = lexer::tokenize(src) {
            if let Ok(program) = parser::parse(tokens) {
                let _ = analyzer::analyze(&program);
                let _ = Compiler::new().compile(&program);
            }
        }
    }
}

#[test]
fn canary_fuzz_borrow_does_not_panic_on_garbage() {
    let inputs = ["", "fn f() {}", "fn f() { let x = 1\nlet r = &x\nx }"];
    for src in &inputs {
        if let Ok(tokens) = lexer::tokenize(src) {
            if let Ok(program) = parser::parse(tokens) {
                let facts = FactGenerator::new().generate(&program);
                let _ = PoloniusSolver::with_max_iterations(200).solve(&facts);
            }
        }
    }
}

#[test]
fn canary_fuzz_async_does_not_panic_on_garbage() {
    let bodies = ["", "@@@", "let x = 1", "let _ = something().await"];
    for body in &bodies {
        let source = format!("async fn _t() {{ {body} }}");
        if let Ok(tokens) = lexer::tokenize(&source) {
            if let Ok(program) = parser::parse(tokens) {
                let _ = analyzer::analyze(&program);
            }
        }
    }
}
