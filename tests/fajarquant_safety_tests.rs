//! FajarQuant Phase 5: Compiler safety tests.
//!
//! Verifies that Fajar Lang's @kernel/@device context enforcement catches
//! context violations in quantization code at analysis time.

/// Evaluate source and expect a semantic error (context violation).
fn expect_semantic_error(source: &str, expected_code: &str) {
    let tokens = fajar_lang::lexer::tokenize(source).expect("lex failed");
    let program = fajar_lang::parser::parse(tokens).expect("parse failed");
    let result = fajar_lang::analyzer::analyze(&program);
    match result {
        Err(errors) => {
            let real_errors: Vec<_> = errors.iter().filter(|e| !e.is_warning()).collect();
            assert!(
                !real_errors.is_empty(),
                "expected semantic error {expected_code}, got no errors"
            );
            let all_msgs: Vec<String> = real_errors.iter().map(|e| format!("{e}")).collect();
            assert!(
                all_msgs.iter().any(|m| m.contains(expected_code)),
                "expected error containing '{expected_code}', got: {all_msgs:?}"
            );
        }
        Ok(()) => {
            // Some errors are only warnings — check if we got the right one
            // In this case, the analyzer may have accepted it
            panic!("expected semantic error {expected_code}, but analyzer accepted the code");
        }
    }
}

/// Evaluate source and expect it to pass analysis (no errors).
fn expect_analysis_ok(source: &str) {
    let tokens = fajar_lang::lexer::tokenize(source).expect("lex failed");
    let program = fajar_lang::parser::parse(tokens).expect("parse failed");
    let result = fajar_lang::analyzer::analyze(&program);
    if let Err(errors) = result {
        let real_errors: Vec<_> = errors.iter().filter(|e| !e.is_warning()).collect();
        if !real_errors.is_empty() {
            let msgs: Vec<String> = real_errors.iter().map(|e| format!("{e}")).collect();
            panic!("expected OK, got errors: {msgs:?}");
        }
    }
}

/// Evaluate and capture output.
fn eval_capture(source: &str) -> String {
    let tokens = fajar_lang::lexer::tokenize(source).expect("lex failed");
    let program = fajar_lang::parser::parse(tokens).expect("parse failed");
    let _ = fajar_lang::analyzer::analyze(&program);
    let mut interp = fajar_lang::interpreter::Interpreter::new_capturing();
    interp.eval_program(&program).expect("eval failed");
    interp.get_output().join("\n")
}

// ════════════════════════════════════════════════════════════════════════
// 5.1: @kernel rejects tensor operations
// ════════════════════════════════════════════════════════════════════════

#[test]
fn kernel_rejects_tensor_in_quantize() {
    // @kernel context should reject tensor creation (KE002)
    expect_semantic_error(
        r#"
@kernel fn bad_quantize() -> void {
    let t = zeros(2, 3)
}
"#,
        "KE002",
    );
}

#[test]
fn kernel_rejects_randn_in_quantize() {
    expect_semantic_error(
        r#"
@kernel fn bad_rotate() -> void {
    let r = randn(4, 4)
}
"#,
        "KE002",
    );
}

// ════════════════════════════════════════════════════════════════════════
// 5.2: @device rejects raw pointer operations
// ════════════════════════════════════════════════════════════════════════

#[test]
fn device_rejects_raw_pointer() {
    // @device context should reject raw pointer dereference (DE001)
    expect_semantic_error(
        r#"
@device fn bad_attention() -> void {
    let p = mem_alloc(1024)
}
"#,
        "DE",
    );
}

// ════════════════════════════════════════════════════════════════════════
// 5.3: @safe context blocks OS primitives
// ════════════════════════════════════════════════════════════════════════

#[test]
fn safe_blocks_os_primitives() {
    expect_semantic_error(
        r#"
@safe fn user_code() -> void {
    mem_alloc(1024)
}
"#,
        "SE",
    );
}

// ════════════════════════════════════════════════════════════════════════
// 5.4: Valid cross-context bridge pattern
// ════════════════════════════════════════════════════════════════════════

#[test]
fn valid_device_quantize_function() {
    // @device allows tensor ops — this should pass analysis
    expect_analysis_ok(
        r#"
@device fn quantize_vector() -> void {
    let x = randn(1, 8)
    let config = turboquant_create(8, 3)
    let encoded = turboquant_encode(config, x)
    println("quantized OK")
}
"#,
    );
}

#[test]
fn valid_safe_function_calls_turboquant() {
    // @safe with standard builtins should work
    let out = eval_capture(
        r#"
fn test_quant() -> void {
    let config = turboquant_create(8, 2)
    println("config created")
}
test_quant()
"#,
    );
    assert!(out.contains("config created"));
}

// ════════════════════════════════════════════════════════════════════════
// 5.5: FajarQuant E2E in valid context
// ════════════════════════════════════════════════════════════════════════

#[test]
fn fajarquant_compare_in_safe_context() {
    let out = eval_capture(
        r#"
let result = fajarquant_compare(8, 2, 50)
let improvement = map_get_or(result, "improvement_pct", -999.0)
println(type_of(improvement))
"#,
    );
    assert!(
        out.contains("f64") || out.contains("float"),
        "should return float improvement, got: {out}"
    );
}

#[test]
fn turboquant_full_pipeline_works() {
    let out = eval_capture(
        r#"
let dim = 8
let bits = 2
let config = turboquant_create(dim, bits)
let x = randn(1, dim)
let enc = turboquant_encode(config, x)
let dec = turboquant_decode(config, enc)
println(type_of(dec))
println("pipeline OK")
"#,
    );
    assert!(out.contains("tensor") || out.contains("Tensor"));
    assert!(out.contains("pipeline OK"));
}
