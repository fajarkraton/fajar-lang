//! Integration tests for v3 Phase A — 12 new tensor operations.

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
// A1: var_axis
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn var_axis_from_fj() {
    let result =
        run_ok("let t = from_data([1.0, 2.0, 3.0, 4.0, 5.0, 6.0], [2, 3])\nvar_axis(t, 1)");
    assert!(
        result.contains("1.0"),
        "var of [1,2,3] should be 1.0: {result}"
    );
}

#[test]
fn var_axis_invalid_from_fj() {
    run_err("var_axis(from_data([1.0], [1]), 99)", "axis");
}

// ═══════════════════════════════════════════════════════════════════════
// A2: std_axis
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn std_axis_from_fj() {
    let result =
        run_ok("let t = from_data([1.0, 2.0, 3.0, 4.0, 5.0, 6.0], [2, 3])\nstd_axis(t, 1)");
    assert!(
        result.contains("1.0"),
        "std of [1,2,3] should be 1.0: {result}"
    );
}

// ═══════════════════════════════════════════════════════════════════════
// A3: kurtosis_axis
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn kurtosis_axis_from_fj() {
    let result = run_ok("kurtosis_axis(from_data([1.0, 2.0, 3.0, 4.0, 5.0], [5, 1]), 0)");
    // 5 uniform values: kurtosis < 0 (platykurtic)
    assert!(
        result.contains("-"),
        "uniform should have negative kurtosis: {result}"
    );
}

// ═══════════════════════════════════════════════════════════════════════
// A4: svd_ratio
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn svd_ratio_from_fj() {
    let result = run_ok("svd_ratio(eye(3))");
    let val: f64 = result.parse().unwrap();
    assert!((val - 1.0).abs() < 0.2, "identity ratio ≈ 1: {val}");
}

#[test]
fn svd_ratio_rank1_from_fj() {
    let result = run_ok("svd_ratio(from_data([1.0, 2.0, 1.0, 2.0], [2, 2]))");
    let val: f64 = result.parse().unwrap();
    assert!(val > 50.0, "rank-1 ratio should be very high: {val}");
}

// ═══════════════════════════════════════════════════════════════════════
// A5: select_dim
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn select_dim_from_fj() {
    let result = run_ok(
        "let t = from_data([10.0, 20.0, 30.0, 40.0, 50.0, 60.0], [2, 3])\nselect_dim(t, 0, 1)",
    );
    assert!(result.contains("40.0"), "row 1 starts with 40: {result}");
}

#[test]
fn select_dim_invalid_from_fj() {
    run_err("select_dim(from_data([1.0], [1]), 0, 99)", "index");
}

// ═══════════════════════════════════════════════════════════════════════
// A8: abs_max_axis
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn abs_max_axis_from_fj() {
    let result = run_ok("abs_max_axis(from_data([1.0, -5.0, 3.0, -2.0, 4.0, -1.0], [2, 3]), 0)");
    assert!(
        result.contains("5.0"),
        "abs max of col 1 should be 5: {result}"
    );
}

// ═══════════════════════════════════════════════════════════════════════
// A9: topk_indices
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn topk_indices_from_fj() {
    let result = run_ok("topk_indices(from_data([1.0, 9.0, 3.0, 7.0], [4]), 2)");
    assert!(
        result.contains("1.0"),
        "index of 9 should be in top-2: {result}"
    );
    assert!(
        result.contains("3.0"),
        "index of 7 should be in top-2: {result}"
    );
}

// ═══════════════════════════════════════════════════════════════════════
// A9.5a: skewness_axis
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn skewness_axis_from_fj() {
    // Right-skewed data
    let result = run_ok("skewness_axis(from_data([1.0, 1.0, 1.0, 1.0, 100.0], [5, 1]), 0)");
    assert!(
        !result.contains("-"),
        "right-skewed should be positive: {result}"
    );
}

// ═══════════════════════════════════════════════════════════════════════
// A9.5b: channel_cv
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn channel_cv_from_fj() {
    let result = run_ok("channel_cv(from_data([1.0, 2.0, 3.0, 4.0, 5.0, 6.0], [2, 3]), 0)");
    let val: f64 = result.parse().unwrap();
    assert!((val - 0.0).abs() < 0.01, "uniform variance CV ≈ 0: {val}");
}

// ═══════════════════════════════════════════════════════════════════════
// A6: quantize_per_channel
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn quantize_per_channel_from_fj() {
    let result = run_ok(
        r#"
        let t = from_data([100.0, 0.01, 50.0, 0.005], [2, 2])
        let q = quantize_per_channel(t, 4, 1)
        type_of(q)
    "#,
    );
    assert_eq!(result, "quantized");
}

#[test]
fn quantize_per_channel_bits_from_fj() {
    let result = run_ok(
        r#"
        let q = quantize_per_channel(from_data([1.0, 2.0, 3.0, 4.0], [2, 2]), 8, 0)
        quantized_bits(q)
    "#,
    );
    assert_eq!(result, "8");
}

// ═══════════════════════════════════════════════════════════════════════
// A7: quantize_residual
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn quantize_residual_from_fj() {
    let result = run_ok(
        r#"
        let t = from_data([1.0, -0.5, 0.25, 0.0], [2, 2])
        let r = quantize_residual(t, 2, 2)
        shape(r)
    "#,
    );
    assert_eq!(result, "[2, 2]");
}

// ═══════════════════════════════════════════════════════════════════════
// A7.5: quantize_asymmetric
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn quantize_asymmetric_from_fj() {
    let result = run_ok(
        r#"
        let t = from_data([10.0, 11.0, 12.0, 13.0], [2, 2])
        let a = quantize_asymmetric(t, 8, 0)
        shape(a)
    "#,
    );
    assert_eq!(result, "[2, 2]");
}

// ═══════════════════════════════════════════════════════════════════════
// Full pipeline: profile → select strategy (simulated)
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn v3_profiler_pipeline() {
    let result = run_ok(
        r#"
        let kv = from_data([1.0, 100.0, 0.5, 0.1, 2.0, 0.01, 0.3, 50.0], [2, 4])
        let cv = channel_cv(kv, 0)
        let sr = svd_ratio(kv)
        let kurt = kurtosis_axis(kv, 0)
        let skew = skewness_axis(kv, 0)
        // Simulate strategy selection
        let strategy = if cv > 2.0 { "KIVI" } else { if sr > 5.0 { "PCA" } else { "Hadamard" } }
        strategy
    "#,
    );
    // With these values, CV ≈ 1.3 and SVD ratio ≈ 2.0, so strategy = "Hadamard"
    assert_eq!(result, "Hadamard");
}
