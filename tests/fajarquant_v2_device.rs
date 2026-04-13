//! Integration tests for B5.L4 — @device fn quantization kernels.
//!
//! Verifies that FajarQuant v2 operations work in @device context
//! and that context isolation rules are enforced.

use fajar_lang::interpreter::Interpreter;

fn run_ok(src: &str) -> String {
    let mut interp = Interpreter::new();
    match interp.eval_source(src) {
        Ok(val) => format!("{val}"),
        Err(e) => panic!("expected OK, got error: {e}"),
    }
}

fn run_succeeds(src: &str) {
    let mut interp = Interpreter::new();
    interp
        .eval_source(src)
        .unwrap_or_else(|e| panic!("expected OK, got error: {e}"));
}

// ═══════════════════════════════════════════════════════════════════════
// Positive: v2 operations work in @device context
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn device_fn_can_quantize() {
    run_succeeds(
        r#"
        @device
        fn quant(t: Tensor, bits: i64) -> Quantized {
            quantize(t, bits)
        }
        let t = from_data([1.0, 2.0, 3.0, 4.0], [4])
        quant(t, 4)
    "#,
    );
}

#[test]
fn device_fn_can_dequantize() {
    run_succeeds(
        r#"
        @device
        fn deq(q: Quantized) -> Tensor {
            dequantize(q)
        }
        let t = from_data([1.0, 2.0, 3.0, 4.0], [4])
        let q = quantize(t, 4)
        deq(q)
    "#,
    );
}

#[test]
fn device_fn_can_hadamard() {
    run_succeeds(
        r#"
        @device
        fn rotate(t: Tensor) -> Tensor {
            hadamard(t)
        }
        let t = from_data([1.0, 2.0, 3.0, 4.0], [4])
        rotate(t)
    "#,
    );
}

#[test]
fn device_fn_can_matmul() {
    run_succeeds(
        r#"
        @device
        fn transform(x: Tensor, rotation: Tensor) -> Tensor {
            matmul(x, transpose(rotation))
        }
        let x = from_data([1.0, 2.0, 3.0, 4.0], [1, 4])
        let r = eye(4)
        transform(x, r)
    "#,
    );
}

#[test]
fn device_fn_full_v2_pipeline() {
    let result = run_ok(
        r#"
        @device
        fn fq_v2_quantize(kv: Tensor, rotation: Tensor, bits: i64) -> Quantized {
            let rotated = matmul(kv, transpose(rotation))
            quantize(rotated, bits)
        }

        @device
        fn fq_v2_dequantize(q: Quantized, rotation: Tensor) -> Tensor {
            let deq = dequantize(q)
            matmul(deq, rotation)
        }

        let rotation = from_data([
            0.5, 0.5, 0.5, 0.5,
            0.5, -0.5, 0.5, -0.5,
            0.5, 0.5, -0.5, -0.5,
            0.5, -0.5, -0.5, 0.5
        ], [4, 4])

        let kv = from_data([10.0, 0.0, 0.0, 0.0], [1, 4])
        let q = fq_v2_quantize(kv, rotation, 4)
        let restored = fq_v2_dequantize(q, rotation)
        type_of(restored)
    "#,
    );
    assert_eq!(result, "tensor");
}

#[test]
fn device_fn_quantized_introspection() {
    let result = run_ok(
        r#"
        @device
        fn info(q: Quantized) -> i64 {
            quantized_bits(q)
        }
        let q = quantize(from_data([1.0, 2.0], [2]), 2)
        info(q)
    "#,
    );
    assert_eq!(result, "2");
}

#[test]
fn device_fn_verify_orthogonal() {
    let result = run_ok(
        r#"
        @device
        fn check_rotation(r: Tensor) -> bool {
            verify_orthogonal(r, 1e-10)
        }
        let eye = eye(4)
        check_rotation(eye)
    "#,
    );
    assert_eq!(result, "true");
}
