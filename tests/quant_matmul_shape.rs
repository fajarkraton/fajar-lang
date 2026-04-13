//! Integration tests for B5.L6 — matmul_quantized with shape verification.

use fajar_lang::interpreter::Interpreter;

fn run_ok(src: &str) -> String {
    let mut interp = Interpreter::new();
    match interp.eval_source(src) {
        Ok(val) => format!("{val}"),
        Err(e) => panic!("expected OK, got error: {e}"),
    }
}

fn run_err(src: &str, substr: &str) {
    let mut interp = Interpreter::new();
    match interp.eval_source(src) {
        Ok(val) => panic!("expected error containing '{substr}', got OK: {val}"),
        Err(e) => {
            let msg = format!("{e}");
            assert!(msg.contains(substr), "expected '{substr}' in: {msg}");
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Positive: shape-compatible matmul succeeds
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn matmul_quantized_nk_layout() {
    // query [1, 4] × KV^T [4, 3]^T = [1, 3]
    // KV quantized shape [3, 4] → dequantize → transpose → [4, 3]
    run_ok(
        r#"
        let query = from_data([1.0, 0.0, 0.0, 0.0], [1, 4])
        let kv = from_data([1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0], [3, 4])
        let kv_q = quantize(kv, 8)
        matmul_quantized(query, kv_q)
    "#,
    );
}

#[test]
fn matmul_quantized_kn_layout() {
    // query [1, 4] × KV [4, 3] = [1, 3]
    run_ok(
        r#"
        let query = from_data([1.0, 0.0, 0.0, 0.0], [1, 4])
        let kv = from_data([1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0], [4, 3])
        let kv_q = quantize(kv, 8)
        matmul_quantized(query, kv_q)
    "#,
    );
}

#[test]
fn matmul_quantized_output_shape() {
    // query [2, 4] × KV [3, 4]^T → [2, 3]
    let result = run_ok(
        r#"
        let query = from_data([1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0], [2, 4])
        let kv = from_data([1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0], [3, 4])
        let kv_q = quantize(kv, 4)
        let out = matmul_quantized(query, kv_q)
        shape(out)
    "#,
    );
    assert_eq!(result, "[2, 3]");
}

#[test]
fn matmul_quantized_identity() {
    // query [2, 2] × eye [2, 2]^T = query
    let result = run_ok(
        r#"
        let query = from_data([3.0, 7.0, 1.0, 5.0], [2, 2])
        let eye = from_data([1.0, 0.0, 0.0, 1.0], [2, 2])
        let eye_q = quantize(eye, 8)
        let out = matmul_quantized(query, eye_q)
        out
    "#,
    );
    // Should approximately recover query (8-bit quantization error small)
    assert!(result.contains("3.0"), "should contain ~3.0: {result}");
    assert!(result.contains("7.0"), "should contain ~7.0: {result}");
}

#[test]
fn matmul_quantized_with_rotation() {
    // Full pipeline: hadamard_quantize KV → matmul_quantized with query
    run_ok(
        r#"
        let query = from_data([1.0, 0.0, 0.0, 0.0], [1, 4])
        let kv = from_data([1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0], [2, 4])
        let kv_q = hadamard_quantize(kv, 4)
        matmul_quantized(query, kv_q)
    "#,
    );
}

// ═══════════════════════════════════════════════════════════════════════
// Negative: shape mismatches caught
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn matmul_quantized_shape_mismatch() {
    // query [1, 4] vs KV [3, 5] — inner dims 4 ≠ 5 and 4 ≠ 3
    run_err(
        r#"
        let query = from_data([1.0, 0.0, 0.0, 0.0], [1, 4])
        let kv = from_data([1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0, 13.0, 14.0, 15.0], [3, 5])
        let kv_q = quantize(kv, 4)
        matmul_quantized(query, kv_q)
    "#,
        "shape mismatch",
    );
}

#[test]
fn matmul_quantized_query_not_2d() {
    run_err(
        r#"
        let query = from_data([1.0, 2.0, 3.0, 4.0], [4])
        let kv = from_data([1.0, 2.0, 3.0, 4.0], [2, 2])
        let kv_q = quantize(kv, 4)
        matmul_quantized(query, kv_q)
    "#,
        "query must be 2D",
    );
}

#[test]
fn matmul_quantized_wrong_types() {
    // Two tensors (not Quantized) should fail (analyzer catches as SE004)
    run_err(
        r#"
        let a = from_data([1.0, 2.0, 3.0, 4.0], [2, 2])
        let b = from_data([1.0, 2.0, 3.0, 4.0], [2, 2])
        matmul_quantized(a, b)
    "#,
        "type mismatch",
    );
}
