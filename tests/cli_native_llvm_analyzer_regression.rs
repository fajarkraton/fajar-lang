//! v35.6.x B-δ regression: lock in that `fj run --native` and `fj run --llvm`
//! run the analyzer pre-pass before dispatching to codegen.
//!
//! Background — see `docs/V35_6_LAYER_RECONCILIATION_B0_FINDINGS.md`.
//!
//! Before B-δ, both `cmd_run_native` (src/main.rs) and `cmd_run_llvm` did
//! `lex → parse → compile_program` with no analyzer pass. The Cranelift H4
//! hook caught some violations (drifted list of ~40 names) but LLVM had no
//! context check at all — so `fj run --llvm` would silently compile @kernel
//! programs containing tensor ops.
//!
//! This test is a mechanical content-guard that fires if either of those two
//! AOT-entry-point functions ever loses its `analyze(&program)` call. The
//! analyzer itself has 149+ context_safety_tests verifying KE001/KE002/DE001
//! emission; here we lock in *that those analyzer checks actually run* on the
//! two AOT paths that historically skipped them.

use std::fs;
use std::path::PathBuf;

fn read_main_rs() -> String {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("src/main.rs");
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("read src/main.rs: {e}"))
}

/// Extracts the body of a `fn <name>(...) -> ExitCode { ... }` from main.rs.
/// Returns the substring from the function's opening `{` to the matching `}`.
fn extract_fn_body(src: &str, fn_name: &str) -> String {
    let needle = format!("fn {fn_name}(");
    let start = src
        .find(&needle)
        .unwrap_or_else(|| panic!("{fn_name} not found in main.rs"));
    let brace_start = src[start..]
        .find('{')
        .unwrap_or_else(|| panic!("no opening brace after fn {fn_name}"))
        + start;
    let mut depth = 0i32;
    let bytes = src.as_bytes();
    for (offset, &b) in bytes[brace_start..].iter().enumerate() {
        match b {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return src[brace_start..=brace_start + offset].to_string();
                }
            }
            _ => {}
        }
    }
    panic!("unbalanced braces in fn {fn_name}");
}

#[test]
fn cmd_run_native_calls_analyze() {
    let src = read_main_rs();
    let body = extract_fn_body(&src, "cmd_run_native");
    assert!(
        body.contains("analyze(&program)"),
        "fn cmd_run_native must call analyze(&program) before dispatching to \
         Cranelift codegen. Pre-v35.6.x this path bypassed the analyzer and \
         relied on Cranelift's H4 hook (a drifted partial list of ~40 names). \
         See docs/V35_6_LAYER_RECONCILIATION_B0_FINDINGS.md."
    );
}

#[test]
fn cmd_run_llvm_calls_analyze() {
    let src = read_main_rs();
    let body = extract_fn_body(&src, "cmd_run_llvm");
    assert!(
        body.contains("analyze(&program)"),
        "fn cmd_run_llvm must call analyze(&program) before dispatching to \
         LLVM codegen. LLVM codegen has no context-violation check of its own, \
         so without the analyzer pre-pass `fj run --llvm` would silently \
         compile @kernel programs containing tensor ops. \
         See docs/V35_6_LAYER_RECONCILIATION_B0_FINDINGS.md §3.2."
    );
}

#[test]
fn analyzer_rejects_kernel_with_tensor_op() {
    // Sanity-check the underlying analyzer invariant that the two cli tests
    // above depend on: a @kernel function that calls tensor_zeros should
    // produce KE002 (TensorInKernel).
    let src = r#"
        @kernel fn boot() -> i64 {
            let t = tensor_zeros(2, 3)
            0
        }
        fn main() -> i64 { boot() }
    "#;
    let tokens = fajar_lang::lexer::tokenize(src).expect("lex");
    let program = fajar_lang::parser::parse(tokens).expect("parse");
    let errs = fajar_lang::analyzer::analyze(&program)
        .expect_err("@kernel + tensor_zeros must fail analysis");
    let found_ke002 = errs.iter().any(|e| format!("{e}").contains("KE002"));
    assert!(
        found_ke002,
        "expected KE002 (TensorInKernel) error, got: {:?}",
        errs.iter().map(|e| format!("{e}")).collect::<Vec<_>>()
    );
}

#[test]
fn analyzer_rejects_device_with_raw_pointer() {
    // Companion to the above for the @device path covered by cmd_run_llvm /
    // cmd_run_native's analyzer pre-pass.
    let src = r#"
        @device fn infer() -> i64 {
            let p = mem_alloc(8, 8)
            0
        }
        fn main() -> i64 { infer() }
    "#;
    let tokens = fajar_lang::lexer::tokenize(src).expect("lex");
    let program = fajar_lang::parser::parse(tokens).expect("parse");
    let errs = fajar_lang::analyzer::analyze(&program)
        .expect_err("@device + mem_alloc must fail analysis");
    let found_de001 = errs.iter().any(|e| format!("{e}").contains("DE001"));
    assert!(
        found_de001,
        "expected DE001 (RawPointerInDevice) error, got: {:?}",
        errs.iter().map(|e| format!("{e}")).collect::<Vec<_>>()
    );
}
