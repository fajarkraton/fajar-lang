//! Tier 4: New tensor/scalar operation tests — 2 tests per op × 10 ops = 20 tests.
//!
//! Tests real ndarray operations via eval_source/eval_program.

/// Evaluate source code and capture all printed output.
fn eval_capture(source: &str) -> String {
    let tokens = fajar_lang::lexer::tokenize(source).expect("lex failed");
    let program = fajar_lang::parser::parse(tokens).expect("parse failed");
    let _ = fajar_lang::analyzer::analyze(&program);
    let mut interp = fajar_lang::interpreter::Interpreter::new_capturing();
    interp.eval_program(&program).expect("eval failed");
    interp.get_output().join("\n")
}

// ════════════════════════════════════════════════════════════════════════
// 1. sign(tensor) — element-wise sign
// ════════════════════════════════════════════════════════════════════════

#[test]
fn sign_tensor_basic() {
    let out = eval_capture(
        r#"
let t = from_data([-3.0, 0.0, 5.0], [3])
let s = sign(t)
println(s)
"#,
    );
    assert!(out.contains("-1"), "sign(-3) should be -1, got: {out}");
    assert!(out.contains("1"), "sign(5) should be 1, got: {out}");
}

#[test]
fn sign_scalar() {
    let out = eval_capture(
        r#"
println(sign(-42))
println(sign(0))
println(sign(7))
"#,
    );
    let lines: Vec<&str> = out.trim().lines().collect();
    assert_eq!(lines[0].trim(), "-1", "sign(-42) = -1");
    assert_eq!(lines[1].trim(), "0", "sign(0) = 0");
    assert_eq!(lines[2].trim(), "1", "sign(7) = 1");
}

// ════════════════════════════════════════════════════════════════════════
// 2. argmin(tensor) — index of minimum
// ════════════════════════════════════════════════════════════════════════

#[test]
fn argmin_tensor_basic() {
    let out = eval_capture(
        r#"
let t = from_data([3.0, 1.0, 2.0], [3])
println(argmin(t))
"#,
    );
    assert_eq!(out.trim(), "1", "argmin([3,1,2]) should be 1, got: {out}");
}

#[test]
fn argmin_first_element() {
    let out = eval_capture(
        r#"
let t = from_data([-5.0, 0.0, 10.0], [3])
println(argmin(t))
"#,
    );
    assert_eq!(out.trim(), "0", "argmin([-5,0,10]) should be 0, got: {out}");
}

// ════════════════════════════════════════════════════════════════════════
// 3. norm(tensor) — L2 norm
// ════════════════════════════════════════════════════════════════════════

#[test]
fn norm_3_4_is_5() {
    let out = eval_capture(
        r#"
let t = from_data([3.0, 4.0], [2])
println(norm(t))
"#,
    );
    let val: f64 = out.trim().parse().expect("parse norm result");
    assert!(
        (val - 5.0).abs() < 1e-10,
        "norm([3,4]) should be 5.0, got: {val}"
    );
}

#[test]
fn norm_unit_vector() {
    let out = eval_capture(
        r#"
let t = from_data([1.0, 0.0, 0.0], [3])
println(norm(t))
"#,
    );
    let val: f64 = out.trim().parse().expect("parse norm result");
    assert!(
        (val - 1.0).abs() < 1e-10,
        "norm([1,0,0]) should be 1.0, got: {val}"
    );
}

// ════════════════════════════════════════════════════════════════════════
// 4. dot(a, b) — inner product
// ════════════════════════════════════════════════════════════════════════

#[test]
fn dot_product_basic() {
    let out = eval_capture(
        r#"
let a = from_data([1.0, 2.0], [2])
let b = from_data([3.0, 4.0], [2])
println(dot(a, b))
"#,
    );
    let val: f64 = out.trim().parse().expect("parse dot result");
    assert!(
        (val - 11.0).abs() < 1e-10,
        "dot([1,2],[3,4]) should be 11.0, got: {val}"
    );
}

#[test]
fn dot_orthogonal() {
    let out = eval_capture(
        r#"
let a = from_data([1.0, 0.0], [2])
let b = from_data([0.0, 1.0], [2])
println(dot(a, b))
"#,
    );
    let val: f64 = out.trim().parse().expect("parse dot result");
    assert!(
        val.abs() < 1e-10,
        "dot of orthogonal vectors should be 0.0, got: {val}"
    );
}

// ════════════════════════════════════════════════════════════════════════
// 5. exp_tensor(t) — element-wise e^x
// ════════════════════════════════════════════════════════════════════════

#[test]
fn exp_tensor_basic() {
    let out = eval_capture(
        r#"
let t = from_data([0.0, 1.0], [2])
let r = exp_tensor(t)
println(r)
"#,
    );
    // e^0 = 1, e^1 ≈ 2.718
    assert!(
        out.contains("1.0") || out.contains("1,"),
        "e^0 should be ~1, got: {out}"
    );
    assert!(out.contains("2.71"), "e^1 should be ~2.718, got: {out}");
}

#[test]
fn exp_tensor_type() {
    let out = eval_capture(
        r#"
let t = from_data([0.0], [1])
let r = exp_tensor(t)
println(type_of(r))
"#,
    );
    assert!(
        out.contains("tensor") || out.contains("Tensor"),
        "exp_tensor should return tensor, got: {out}"
    );
}

// ════════════════════════════════════════════════════════════════════════
// 6. log_tensor(t) — element-wise ln(x)
// ════════════════════════════════════════════════════════════════════════

#[test]
fn log_tensor_basic() {
    let out = eval_capture(
        r#"
let t = from_data([1.0, 2.718281828], [2])
let r = log_tensor(t)
println(r)
"#,
    );
    // ln(1) = 0, ln(e) ≈ 1
    assert!(
        out.contains("0.0") || out.contains("0,"),
        "ln(1) should be ~0, got: {out}"
    );
}

#[test]
fn log_tensor_inverse_of_exp() {
    // log(exp(x)) ≈ x
    let out = eval_capture(
        r#"
let t = from_data([2.0], [1])
let e = exp_tensor(t)
let l = log_tensor(e)
println(l)
"#,
    );
    assert!(
        out.contains("2.0") || out.contains("2,"),
        "log(exp(2)) should be ~2, got: {out}"
    );
}

// ════════════════════════════════════════════════════════════════════════
// 7. sqrt_tensor(t) — element-wise sqrt
// ════════════════════════════════════════════════════════════════════════

#[test]
fn sqrt_tensor_basic() {
    let out = eval_capture(
        r#"
let t = from_data([4.0, 9.0], [2])
let r = sqrt_tensor(t)
println(r)
"#,
    );
    assert!(
        out.contains("2.0") || out.contains("2,"),
        "sqrt(4) should be 2, got: {out}"
    );
    assert!(
        out.contains("3.0") || out.contains("3,"),
        "sqrt(9) should be 3, got: {out}"
    );
}

#[test]
fn sqrt_tensor_one() {
    let out = eval_capture(
        r#"
let t = from_data([1.0], [1])
let r = sqrt_tensor(t)
println(r)
"#,
    );
    assert!(
        out.contains("1.0") || out.contains("1,"),
        "sqrt(1) should be 1, got: {out}"
    );
}

// ════════════════════════════════════════════════════════════════════════
// 8. abs_tensor(t) — element-wise absolute value
// ════════════════════════════════════════════════════════════════════════

#[test]
fn abs_tensor_basic() {
    let out = eval_capture(
        r#"
let t = from_data([-3.0, 4.0], [2])
let r = abs_tensor(t)
println(r)
"#,
    );
    assert!(
        out.contains("3.0") || out.contains("3,"),
        "abs(-3) should be 3, got: {out}"
    );
    assert!(
        out.contains("4.0") || out.contains("4,"),
        "abs(4) should be 4, got: {out}"
    );
}

#[test]
fn abs_tensor_zeros() {
    let out = eval_capture(
        r#"
let t = from_data([0.0, -0.0], [2])
let r = abs_tensor(t)
println(r)
"#,
    );
    assert!(
        out.contains("0.0") || out.contains("0,"),
        "abs(0) should be 0, got: {out}"
    );
}

// ════════════════════════════════════════════════════════════════════════
// 9. exp(x) — scalar e^x
// ════════════════════════════════════════════════════════════════════════

#[test]
fn exp_scalar_one() {
    let out = eval_capture(
        r#"
println(exp(1.0))
"#,
    );
    let val: f64 = out.trim().parse().expect("parse exp result");
    assert!(
        (val - std::f64::consts::E).abs() < 1e-10,
        "exp(1) should be e ≈ 2.718, got: {val}"
    );
}

#[test]
fn exp_scalar_zero() {
    let out = eval_capture(
        r#"
println(exp(0.0))
"#,
    );
    let val: f64 = out.trim().parse().expect("parse exp result");
    assert!(
        (val - 1.0).abs() < 1e-10,
        "exp(0) should be 1.0, got: {val}"
    );
}

// ════════════════════════════════════════════════════════════════════════
// 10. gamma(x) — Gamma function
// ════════════════════════════════════════════════════════════════════════

#[test]
fn gamma_factorial() {
    // gamma(5) = 4! = 24
    let out = eval_capture(
        r#"
println(gamma(5.0))
"#,
    );
    let val: f64 = out.trim().parse().expect("parse gamma result");
    assert!(
        (val - 24.0).abs() < 0.01,
        "gamma(5) should be 24.0, got: {val}"
    );
}

#[test]
fn gamma_half() {
    // gamma(0.5) = sqrt(pi) ≈ 1.7724
    let out = eval_capture(
        r#"
println(gamma(0.5))
"#,
    );
    let val: f64 = out.trim().parse().expect("parse gamma result");
    let expected = std::f64::consts::PI.sqrt();
    assert!(
        (val - expected).abs() < 0.01,
        "gamma(0.5) should be sqrt(pi) ≈ {expected}, got: {val}"
    );
}
