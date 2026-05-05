//! FajarQuant fj-lang stdlib port — bit-equivalent regression tests.
//!
//! FAJARQUANT_RUST_TO_FJ_PLAN Phase 5 (consolidated cross-validation).
//! Loads `stdlib/fajarquant.fj` content, prepends test main, runs through
//! fj-lang eval, asserts outputs match the canonical Rust reference values
//! recorded during Phase 2-4 ports.
//!
//! Each test exercises one module's port and asserts at least 3 outputs
//! are bit-exact match Rust. Total ~40 assertions across 7 modules.

use std::fs;

/// Load stdlib/fajarquant.fj and append the given test body (top-level
/// statements — no `fn main()` wrapper, since `eval_program` runs
/// top-level stmts directly).
fn run_with_stdlib(test_body: &str) -> String {
    let stdlib_src =
        fs::read_to_string("stdlib/fajarquant.fj").expect("stdlib/fajarquant.fj missing");
    let combined = format!("{stdlib_src}\n\n{test_body}");
    let tokens = fajar_lang::lexer::tokenize(&combined).expect("lex failed");
    let program = fajar_lang::parser::parse(tokens).expect("parse failed");
    let _ = fajar_lang::analyzer::analyze(&program);
    let mut interp = fajar_lang::interpreter::Interpreter::new_capturing();
    interp.eval_program(&program).expect("eval failed");
    interp.get_output().join("\n")
}

// ═══════════════════════════════════════════════════════════════════════
// Phase 2.B — hierarchical: bit-decay schedule
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn fajarquant_fj_hierarchical_bits_for_age() {
    // Canonical Rust outputs (base=8, min=2, decay=0.001):
    //   age=0   → 8
    //   age=10  → 8
    //   age=100 → 7
    //   age=1000 → 3
    //   age=5000 → 2
    let out = run_with_stdlib(
        r#"
println(bits_for_age(8, 2, 0.001, 0))
println(bits_for_age(8, 2, 0.001, 10))
println(bits_for_age(8, 2, 0.001, 100))
println(bits_for_age(8, 2, 0.001, 1000))
println(bits_for_age(8, 2, 0.001, 5000))
"#,
    );
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines[0], "8", "bits_for_age(0): {out}");
    assert_eq!(lines[1], "8", "bits_for_age(10): {out}");
    assert_eq!(lines[2], "7", "bits_for_age(100): {out}");
    assert_eq!(lines[3], "3", "bits_for_age(1000): {out}");
    assert_eq!(lines[4], "2", "bits_for_age(5000): {out}");
}

#[test]
fn fajarquant_fj_hierarchical_total_bits() {
    let out = run_with_stdlib(
        r#"
println(schedule_total_bits(10, 8, 2, 0.001))
println(schedule_total_bits(100, 8, 2, 0.001))
println(schedule_total_bits(1000, 8, 2, 0.001))
println(schedule_total_bits(10000, 8, 2, 0.001))
"#,
    );
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines[0], "80");
    assert_eq!(lines[1], "765");
    assert_eq!(lines[2], "5051");
    assert_eq!(lines[3], "23215");
}

// ═══════════════════════════════════════════════════════════════════════
// Phase 2.C — scalar_baseline: ternary BitLinear
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn fajarquant_fj_decode_ternary_byte() {
    let out = run_with_stdlib(
        r#"
let r = decode_ternary_byte(100)
println(r[0])
println(r[1])
println(r[2])
println(r[3])
"#,
    );
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines[0], "-1");
    assert_eq!(lines[1], "0");
    assert_eq!(lines[2], "1");
    assert_eq!(lines[3], "0");
}

#[test]
fn fajarquant_fj_bitlinear_packed_scalar() {
    let out = run_with_stdlib(
        r#"
let w: [i64] = [1, -1, 0]
let x: [i64] = [127, -64, 95]
let p = pack_ternary_v31(w, 3)
let y = bitlinear_packed_scalar(p, x, 1, 3)
println(y[0])
"#,
    );
    assert_eq!(out.trim(), "191");
}

#[test]
fn fajarquant_fj_absmax_quantize() {
    let out = run_with_stdlib(
        r#"
let x: [f64] = [-8.0, 0.0, 8.0, 4.0]
let r = absmax_quantize_i8(x, 4)
println(r[0])
println(r[1] as i64)
println(r[2] as i64)
println(r[3] as i64)
println(r[4] as i64)
"#,
    );
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines[0], "15.875");
    assert_eq!(lines[1], "-127");
    assert_eq!(lines[2], "0");
    assert_eq!(lines[3], "127");
    assert_eq!(lines[4], "64");
}

// ═══════════════════════════════════════════════════════════════════════
// Phase 3.A — fused_attention: codebook ops
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn fajarquant_fj_codebook_dot_product() {
    let out = run_with_stdlib(
        r#"
let cb: [f64] = [0.5, -0.3, 1.2, 0.0]
let q: [f64] = [1.0, 2.0, 3.0]
let k: [i64] = [0, 2, 1]
let dot = codebook_dot_product(q, k, cb, 3)
println(dot)
"#,
    );
    assert_eq!(out.trim(), "2");
}

#[test]
fn fajarquant_fj_fused_attention() {
    let out = run_with_stdlib(
        r#"
let cb: [f64] = [0.5, -0.3, 1.2, 0.0]
let q: [f64] = [1.0, 1.0]
let k: [i64] = [0, 2, 1, 3]
let v: [i64] = [2, 1, 0, 3]
let r = fused_quantized_attention(q, k, v, cb, 2, 2)
println(r[0])
println(r[1])
"#,
    );
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines[0], "1.1165579545845175");
    assert_eq!(lines[1], "-0.2642391233933647");
}

// ═══════════════════════════════════════════════════════════════════════
// Phase 3.B — turboquant: LCG
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn fajarquant_fj_lcg_sequence() {
    let out = run_with_stdlib(
        r#"
let mut s: i64 = 42
s = lcg_next_state(s)
println(lcg_to_f64(s))
s = lcg_next_state(s)
println(lcg_to_f64(s))
s = lcg_next_state(s)
println(lcg_to_f64(s))
"#,
    );
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines[0], "0.49002167176561373");
    assert_eq!(lines[1], "0.46876518464537753");
    assert_eq!(lines[2], "0.5602241947856252");
}

// ═══════════════════════════════════════════════════════════════════════
// Phase 3.C — kivi: per-channel quantization
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn fajarquant_fj_kivi_per_channel_quantize() {
    let out = run_with_stdlib(
        r#"
let keys: [f64] = [
    1.0, 0.5, -0.2,
    0.8, 0.7, -0.1,
    1.2, 0.6, -0.3,
    0.9, 0.4, -0.15,
]
let p = kivi_quantize_keys(keys, 4, 3, 4)
println(p[0])
println(p[3])
println(p[6] as i64)
println(p[7] as i64)
"#,
    );
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines[0], "0.02666666666666666");
    assert_eq!(lines[1], "0.8");
    assert_eq!(lines[2], "8");
    assert_eq!(lines[3], "5");
}

// ═══════════════════════════════════════════════════════════════════════
// Phase 4 — adaptive: PCA via power iteration
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn fajarquant_fj_pca_power_iteration() {
    let out = run_with_stdlib(
        r#"
let vecs: [f64] = [
    2.0, 1.0, 0.5,
    1.5, 0.8, 0.3,
    2.2, 1.1, 0.6,
    1.8, 0.9, 0.4,
]
let cov = compute_covariance(vecs, 4, 3)
println(cov[0])
println(cov[1])
let ev = power_iteration_eigenvectors(cov, 3, 50)
println(ev[0])
println(ev[8])
"#,
    );
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines[0], "3.5825");
    assert_eq!(lines[1], "1.8100000000000003");
    assert_eq!(lines[2], "0.8721706907444003");
    assert_eq!(lines[3], "0.2969907283617762");
}
