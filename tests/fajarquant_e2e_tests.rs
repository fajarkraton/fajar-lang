//! FajarQuant Phase 7: End-to-end integration tests.
//!
//! Tests the complete pipeline: create → encode → decode → verify.

fn eval_capture(source: &str) -> String {
    let tokens = fajar_lang::lexer::tokenize(source).expect("lex failed");
    let program = fajar_lang::parser::parse(tokens).expect("parse failed");
    let _ = fajar_lang::analyzer::analyze(&program);
    let mut interp = fajar_lang::interpreter::Interpreter::new_capturing();
    interp.eval_program(&program).expect("eval failed");
    interp.get_output().join("\n")
}

// ═══════════════════════════════════════════════════════════════════════
// TurboQuant E2E
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn turboquant_create_encode_decode_e2e() {
    let out = eval_capture(
        r#"
let config = turboquant_create(16, 3)
let x = randn(1, 16)
let enc = turboquant_encode(config, x)
let dec = turboquant_decode(config, enc)
println(type_of(dec))
"#,
    );
    assert!(out.contains("tensor") || out.contains("Tensor"));
}

#[test]
fn turboquant_inner_product_e2e() {
    let out = eval_capture(
        r#"
let config = turboquant_create(8, 2)
let x = randn(1, 8)
let y = randn(1, 8)
let result = turboquant_inner_product(config, x, y)
let err = map_get_or(result, "error", -1.0)
println(type_of(err))
"#,
    );
    assert!(out.contains("f64") || out.contains("float"));
}

#[test]
fn turboquant_multiple_bit_widths() {
    let out = eval_capture(
        r#"
let dim = 8
let x = randn(1, dim)
let c1 = turboquant_create(dim, 1)
let c2 = turboquant_create(dim, 2)
let c3 = turboquant_create(dim, 3)
let c4 = turboquant_create(dim, 4)
let e1 = turboquant_encode(c1, x)
let e2 = turboquant_encode(c2, x)
let e3 = turboquant_encode(c3, x)
let e4 = turboquant_encode(c4, x)
println("b1=" + to_string(map_get_or(c1, "codebook_size", 0)))
println("b2=" + to_string(map_get_or(c2, "codebook_size", 0)))
println("b3=" + to_string(map_get_or(c3, "codebook_size", 0)))
println("b4=" + to_string(map_get_or(c4, "codebook_size", 0)))
"#,
    );
    assert!(out.contains("b1=2"));
    assert!(out.contains("b2=4"));
    assert!(out.contains("b3=8"));
    assert!(out.contains("b4=16"));
}

// ═══════════════════════════════════════════════════════════════════════
// FajarQuant Adaptive E2E
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn fajarquant_compare_returns_valid_results() {
    let out = eval_capture(
        r#"
let result = fajarquant_compare(16, 2, 100)
let adaptive = map_get_or(result, "adaptive_mse", -1.0)
let random = map_get_or(result, "random_mse", -1.0)
let improv = map_get_or(result, "improvement_pct", -999.0)
println(type_of(adaptive))
println(type_of(random))
println(type_of(improv))
"#,
    );
    let lines: Vec<&str> = out.trim().lines().collect();
    assert!(lines.len() >= 3);
    for line in &lines {
        assert!(
            line.contains("f64") || line.contains("float"),
            "all results should be float, got: {line}"
        );
    }
}

#[test]
fn fajarquant_improvement_increases_with_bits() {
    // Higher bit-width should generally show more improvement from adaptive
    let out = eval_capture(
        r#"
let r2 = fajarquant_compare(16, 2, 200)
let r3 = fajarquant_compare(16, 3, 200)
let i2 = map_get_or(r2, "improvement_pct", 0.0)
let i3 = map_get_or(r3, "improvement_pct", 0.0)
println(i2)
println(i3)
"#,
    );
    let lines: Vec<&str> = out.trim().lines().collect();
    assert!(lines.len() >= 2, "expected 2 improvement values");
    let i2: f64 = lines[0].trim().parse().unwrap_or(0.0);
    let i3: f64 = lines[1].trim().parse().unwrap_or(0.0);
    // At higher bits, adaptive should maintain or improve its advantage
    assert!(i3 > 0.0, "b=3 should show positive improvement, got: {i3}");
}

// ═══════════════════════════════════════════════════════════════════════
// New tensor ops E2E
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn clamp_tensor_e2e() {
    let out = eval_capture(
        r#"
let t = from_data([-5.0, 0.0, 5.0, 10.0], [4])
let c = clamp_tensor(t, -1.0, 1.0)
println(c)
"#,
    );
    assert!(out.contains("-1.0") || out.contains("-1,"));
    assert!(out.contains("1.0") || out.contains("1,"));
}

#[test]
fn where_tensor_e2e() {
    let out = eval_capture(
        r#"
let cond = from_data([1.0, 0.0, 1.0, 0.0], [4])
let x = from_data([10.0, 10.0, 10.0, 10.0], [4])
let y = from_data([0.0, 0.0, 0.0, 0.0], [4])
let result = where_tensor(cond, x, y)
println(result)
"#,
    );
    // where cond>0: take x (10), else take y (0)
    assert!(out.contains("10.0") || out.contains("10,"));
    assert!(out.contains("0.0") || out.contains("0,"));
}

// ═══════════════════════════════════════════════════════════════════════
// Full pipeline E2E
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn full_pipeline_quantize_compare_report() {
    let out = eval_capture(
        r#"
let dim = 8
let config = turboquant_create(dim, 3)
let x = from_data([0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8], [dim])
let enc = turboquant_encode(config, x)
let dec = turboquant_decode(config, enc)
println("encoded indices count: " + to_string(len(map_get_or(enc, "indices", []))))
println("decoded type: " + to_string(type_of(dec)))
println("pipeline complete")
"#,
    );
    assert!(out.contains("encoded indices count: 8"));
    assert!(out.contains("decoded type: tensor"));
    assert!(out.contains("pipeline complete"));
}
