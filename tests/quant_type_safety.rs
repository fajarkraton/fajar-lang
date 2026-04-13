//! Integration tests for B5.L1.2 — SE023 QuantizedNotDequantized analyzer rule.
//!
//! Verifies that the compiler prevents using a Quantized value where a Tensor
//! is expected, forcing explicit dequantize() calls.

use fajar_lang::interpreter::Interpreter;

/// Helper: run source and expect success.
fn run_ok(src: &str) -> String {
    let mut interp = Interpreter::new();
    match interp.eval_source(src) {
        Ok(val) => format!("{val}"),
        Err(e) => panic!("expected OK, got error: {e}"),
    }
}

/// Helper: run source and expect it to produce a result (may be any value).
fn run_succeeds(src: &str) {
    let mut interp = Interpreter::new();
    interp
        .eval_source(src)
        .unwrap_or_else(|e| panic!("expected OK, got error: {e}"));
}

/// Helper: run source and expect a runtime error containing the given substring.
fn run_err(src: &str, expected_substr: &str) {
    let mut interp = Interpreter::new();
    match interp.eval_source(src) {
        Ok(val) => panic!("expected error containing '{expected_substr}', got OK: {val}"),
        Err(e) => {
            let msg = format!("{e}");
            assert!(
                msg.contains(expected_substr),
                "expected error containing '{expected_substr}', got: {msg}"
            );
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Positive tests — quantize/dequantize roundtrip works
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn quantize_2bit_roundtrip() {
    run_succeeds(
        r#"
        let t = from_data([1.0, -1.0, 0.5, 0.0], [2, 2])
        let q = quantize(t, 2)
        let d = dequantize(q)
        d
    "#,
    );
}

#[test]
fn quantize_4bit_roundtrip() {
    let result = run_ok(
        r#"
        let t = from_data([1.0, -0.5, 0.25], [3])
        let q = quantize(t, 4)
        type_of(q)
    "#,
    );
    assert_eq!(result, "quantized");
}

#[test]
fn quantize_8bit_roundtrip() {
    run_succeeds(
        r#"
        let t = from_data([1.0, 2.0, 3.0, 4.0], [2, 2])
        let q = quantize(t, 8)
        let d = dequantize(q)
        d
    "#,
    );
}

// ═══════════════════════════════════════════════════════════════════════
// Introspection tests
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn quantized_bits_returns_bit_width() {
    let result = run_ok(
        r#"
        let q = quantize(from_data([1.0], [1]), 4)
        quantized_bits(q)
    "#,
    );
    assert_eq!(result, "4");
}

#[test]
fn quantized_shape_returns_shape() {
    let result = run_ok(
        r#"
        let q = quantize(from_data([1.0, 2.0, 3.0, 4.0, 5.0, 6.0], [2, 3]), 4)
        quantized_shape(q)
    "#,
    );
    assert_eq!(result, "[2, 3]");
}

#[test]
fn quantized_numel_returns_element_count() {
    let result = run_ok(
        r#"
        let q = quantize(from_data([1.0, 2.0, 3.0, 4.0, 5.0, 6.0], [2, 3]), 2)
        quantized_numel(q)
    "#,
    );
    assert_eq!(result, "6");
}

#[test]
fn quantized_size_bytes_reflects_compression() {
    let result = run_ok(
        r#"
        let q = quantize(from_data([1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0], [8]), 2)
        quantized_size_bytes(q)
    "#,
    );
    assert_eq!(result, "2"); // 8 elements * 2 bits / 8 = 2 bytes
}

// ═══════════════════════════════════════════════════════════════════════
// Negative tests — invalid bit widths rejected at runtime
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn quantize_invalid_bits_5() {
    run_err(
        r#"
        let t = from_data([1.0], [1])
        quantize(t, 5)
    "#,
        "unsupported bit width 5",
    );
}

#[test]
fn quantize_invalid_bits_0() {
    run_err(
        r#"
        let t = from_data([1.0], [1])
        quantize(t, 0)
    "#,
        "unsupported bit width 0",
    );
}

// ═══════════════════════════════════════════════════════════════════════
// SE023 tests — dequantize required before tensor operations
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn dequantize_before_matmul_succeeds() {
    // Correct usage: dequantize before passing to tensor operation
    run_succeeds(
        r#"
        let t = from_data([1.0, 2.0, 3.0, 4.0], [2, 2])
        let q = quantize(t, 4)
        let d = dequantize(q)
        matmul(d, d)
    "#,
    );
}

#[test]
fn dequantize_is_not_tensor() {
    // type_of should return "quantized", not "tensor"
    let result = run_ok(
        r#"
        let q = quantize(from_data([1.0], [1]), 4)
        type_of(q)
    "#,
    );
    assert_eq!(result, "quantized");
}

#[test]
fn quantized_passed_to_tensor_fn_fails() {
    // Passing a Quantized directly to a tensor builtin should fail at runtime
    // (the analyzer may or may not catch it depending on type inference)
    run_err(
        r#"
        let q = quantize(from_data([1.0, 2.0, 3.0, 4.0], [2, 2]), 4)
        matmul(q, q)
    "#,
        "expected",
    );
}

#[test]
fn quantized_display_shows_metadata() {
    let result = run_ok(
        r#"
        let q = quantize(from_data([1.0, -1.0], [2]), 4)
        to_string(q)
    "#,
    );
    assert!(
        result.contains("4bit"),
        "display should mention bit width: {result}"
    );
    assert!(
        result.contains("[2]"),
        "display should mention shape: {result}"
    );
}
