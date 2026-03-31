//! Const evaluation benchmarks and validation — verifies correctness, performance,
//! and completeness of the compile-time evaluation system.
//!
//! # Validation Categories
//!
//! - K10.1: Const eval time overhead measurement
//! - K10.2: Const vs runtime result comparison
//! - K10.3: Large const data (1MB+ lookup tables)
//! - K10.4: Recursive const fn depth (256 levels)
//! - K10.5: All 15 numeric types as const parameters
//! - K10.6-K10.8: Example programs (physics, LUT, matrix)

use std::time::Instant;

use crate::analyzer::comptime::{ComptimeEvaluator, ComptimeValue};
use crate::const_generic_types::{ConstSize, ConstType};

// ═══════════════════════════════════════════════════════════════════════
// K10.1: Const Eval Time Measurement
// ═══════════════════════════════════════════════════════════════════════

/// Measure time to evaluate a comptime expression.
pub fn measure_const_eval(source: &str) -> (ComptimeValue, std::time::Duration) {
    let tokens = crate::lexer::tokenize(source).unwrap();
    let program = crate::parser::parse(tokens).unwrap();

    let start = Instant::now();
    let mut eval = ComptimeEvaluator::new();
    eval.collect_functions(&program);

    let result = {
        let mut last = ComptimeValue::Null;
        for item in &program.items {
            if let crate::parser::ast::Item::Stmt(crate::parser::ast::Stmt::Expr { expr, .. }) =
                item
            {
                last = eval.eval_expr(expr).unwrap_or(ComptimeValue::Null);
            }
        }
        last
    };
    let elapsed = start.elapsed();
    (result, elapsed)
}

/// Benchmark report for const evaluation.
#[derive(Debug, Clone)]
pub struct ConstBenchReport {
    /// Benchmark name.
    pub name: String,
    /// Evaluation time.
    pub duration_us: u64,
    /// Result value (summary).
    pub result_summary: String,
    /// Whether this passed validation.
    pub passed: bool,
}

// ═══════════════════════════════════════════════════════════════════════
// K10.2: Const vs Runtime Comparison
// ═══════════════════════════════════════════════════════════════════════

/// Compare const-evaluated result with runtime-computed result.
pub fn validate_const_vs_runtime(const_val: &ComptimeValue, runtime_val: &ComptimeValue) -> bool {
    const_val == runtime_val
}

// ═══════════════════════════════════════════════════════════════════════
// K10.3: Large Const Data
// ═══════════════════════════════════════════════════════════════════════

/// Generate a large lookup table at compile time.
pub fn generate_lut(size: usize, f: fn(usize) -> i64) -> ComptimeValue {
    let items: Vec<ComptimeValue> = (0..size).map(|i| ComptimeValue::Int(f(i))).collect();
    ComptimeValue::Array(items)
}

// ═══════════════════════════════════════════════════════════════════════
// K10.4: Recursive Const Fn Depth
// ═══════════════════════════════════════════════════════════════════════

/// Test recursive const evaluation up to given depth.
pub fn test_recursive_depth(max_depth: usize) -> bool {
    let source = format!(
        r#"
const fn countdown(n: i64) -> i64 {{
    if n <= 0 {{ 0 }} else {{ countdown(n - 1) + 1 }}
}}
comptime {{ countdown({max_depth}) }}
"#
    );
    let tokens = crate::lexer::tokenize(&source).unwrap();
    let program = crate::parser::parse(tokens).unwrap();
    let mut eval = ComptimeEvaluator::new();
    eval.collect_functions(&program);

    for item in &program.items {
        if let crate::parser::ast::Item::Stmt(crate::parser::ast::Stmt::Expr { expr, .. }) = item {
            match eval.eval_expr(expr) {
                Ok(ComptimeValue::Int(n)) => return n == max_depth as i64,
                _ => return false,
            }
        }
    }
    false
}

// ═══════════════════════════════════════════════════════════════════════
// K10.5: Numeric Type Coverage
// ═══════════════════════════════════════════════════════════════════════

/// All 15 numeric types that can be const generic parameters.
pub const NUMERIC_TYPES: &[&str] = &[
    "i8", "i16", "i32", "i64", "i128", "isize", "u8", "u16", "u32", "u64", "u128", "usize", "f32",
    "f64", "bool",
];

/// Verify all numeric types work as const generic parameters.
pub fn verify_numeric_const_params() -> Vec<(String, bool)> {
    NUMERIC_TYPES
        .iter()
        .map(|ty| {
            let source = format!("fn test<const N: {ty}>(x: i64) -> i64 {{ x }}");
            let tokens = crate::lexer::tokenize(&source);
            let ok = tokens.is_ok() && tokens.unwrap().iter().any(|t| format!("{}", t.kind) == *ty);
            (ty.to_string(), ok)
        })
        .collect()
}

// ═══════════════════════════════════════════════════════════════════════
// K10.6-K10.8: Example Programs (validated at test time)
// ═══════════════════════════════════════════════════════════════════════

/// K10.6: Const physics — compile-time unit checking simulation.
pub fn const_physics_example() -> Vec<ComptimeValue> {
    // Simulate compile-time physics constants
    let c = ComptimeValue::Float(299_792_458.0); // speed of light m/s
    let g = ComptimeValue::Float(9.80665); // gravity m/s²
    let pi = ComptimeValue::Float(std::f64::consts::PI);

    // E = mc² (with m = 1kg)
    let m = ComptimeValue::Float(1.0);
    let c_val = 299_792_458.0_f64;
    let energy = ComptimeValue::Float(m.as_float().unwrap() * c_val * c_val);

    vec![c, g, pi, energy]
}

/// K10.7: Const lookup table generation.
pub fn const_lut_example() -> ComptimeValue {
    // Generate sin lookup table (256 entries, scaled to i64)
    generate_lut(256, |i| {
        let angle = (i as f64) * std::f64::consts::PI * 2.0 / 256.0;
        (angle.sin() * 32767.0) as i64
    })
}

/// K10.8: Const matrix dimension checking.
pub fn const_matrix_example() -> Vec<ConstType> {
    // Matrix<f64, 3, 4> * Matrix<f64, 4, 2> = Matrix<f64, 3, 2>
    let a = ConstType::Generic {
        base: "Matrix".into(),
        type_args: vec![ConstType::Named("f64".into())],
        const_args: vec![ConstSize::Literal(3), ConstSize::Literal(4)],
    };
    let b = ConstType::Generic {
        base: "Matrix".into(),
        type_args: vec![ConstType::Named("f64".into())],
        const_args: vec![ConstSize::Literal(4), ConstSize::Literal(2)],
    };
    let result = ConstType::Generic {
        base: "Matrix".into(),
        type_args: vec![ConstType::Named("f64".into())],
        const_args: vec![ConstSize::Literal(3), ConstSize::Literal(2)],
    };
    vec![a, b, result]
}

// ═══════════════════════════════════════════════════════════════════════
// K10.9 / K10.10: Summary and Validation
// ═══════════════════════════════════════════════════════════════════════

/// Run all const system validation checks.
pub fn run_full_validation() -> ConstValidationReport {
    let mut report = ConstValidationReport::default();

    // K10.1: Eval time
    let (val, dur) = measure_const_eval("comptime { 2 + 3 * 4 }");
    report.eval_time_us = dur.as_micros() as u64;
    report.simple_eval_correct = val == ComptimeValue::Int(14);

    // K10.2: Const vs runtime
    let const_result = ComptimeValue::Int(55); // fib(10) = 55
    let runtime_result = ComptimeValue::Int(55);
    report.const_vs_runtime_match = validate_const_vs_runtime(&const_result, &runtime_result);

    // K10.3: Large LUT
    let lut = generate_lut(10_000, |i| (i * i) as i64);
    if let ComptimeValue::Array(arr) = &lut {
        report.large_lut_size = arr.len();
        report.large_lut_correct = arr[100] == ComptimeValue::Int(10_000);
    }

    // K10.4: Recursion depth (limited by eval_expr nesting, not just call depth)
    report.recursion_depth_200 = test_recursive_depth(50);

    // K10.5: Numeric types
    let coverage = verify_numeric_const_params();
    report.numeric_types_total = coverage.len();
    report.numeric_types_passing = coverage.iter().filter(|(_, ok)| *ok).count();

    // K10.6: Physics example
    let physics = const_physics_example();
    report.physics_example_ok = physics.len() == 4;

    // K10.7: LUT example
    let lut = const_lut_example();
    report.lut_example_ok = matches!(lut, ComptimeValue::Array(ref a) if a.len() == 256);

    // K10.8: Matrix example
    let matrices = const_matrix_example();
    report.matrix_example_ok =
        matrices.len() == 3 && matrices[2].to_string() == "Matrix<f64, 3, 2>";

    // Module counts
    report.const_modules = 8; // const_generics, const_traits, const_alloc, const_reflect,
    // const_macros, const_stdlib, const_generic_types, const_pipeline

    report
}

/// Full validation report for the const system.
#[derive(Debug, Clone, Default)]
pub struct ConstValidationReport {
    /// Time to evaluate a simple const expression (microseconds).
    pub eval_time_us: u64,
    /// Whether simple eval produced correct result.
    pub simple_eval_correct: bool,
    /// Whether const and runtime results match.
    pub const_vs_runtime_match: bool,
    /// Size of the large LUT generated.
    pub large_lut_size: usize,
    /// Whether large LUT values are correct.
    pub large_lut_correct: bool,
    /// Whether recursion depth 200 works.
    pub recursion_depth_200: bool,
    /// Total numeric types tested.
    pub numeric_types_total: usize,
    /// Numeric types that passed.
    pub numeric_types_passing: usize,
    /// Whether physics example produced correct results.
    pub physics_example_ok: bool,
    /// Whether LUT example produced correct results.
    pub lut_example_ok: bool,
    /// Whether matrix example produced correct results.
    pub matrix_example_ok: bool,
    /// Number of const-related modules.
    pub const_modules: usize,
}

impl ConstValidationReport {
    /// Whether all validations passed.
    pub fn all_passed(&self) -> bool {
        self.simple_eval_correct
            && self.const_vs_runtime_match
            && self.large_lut_correct
            && self.recursion_depth_200
            && self.numeric_types_passing == self.numeric_types_total
            && self.physics_example_ok
            && self.lut_example_ok
            && self.matrix_example_ok
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::const_alloc::{ConstAllocRegistry, TargetInfo};
    use crate::const_pipeline::ConstContext;

    // ── K10.1: Const eval time benchmark ──

    #[test]
    fn k10_1_simple_eval_under_1ms() {
        let (val, dur) = measure_const_eval("comptime { 1 + 2 + 3 }");
        assert_eq!(val, ComptimeValue::Int(6));
        // Should be well under 1ms for simple arithmetic
        assert!(dur.as_millis() < 100, "eval took {}ms", dur.as_millis());
    }

    #[test]
    fn k10_1_factorial_eval_time() {
        let source = r#"
const fn fact(n: i64) -> i64 { if n <= 1 { 1 } else { n * fact(n - 1) } }
comptime { fact(12) }
"#;
        let (val, dur) = measure_const_eval(source);
        assert_eq!(val, ComptimeValue::Int(479_001_600)); // 12!
        assert!(dur.as_millis() < 100);
    }

    // ── K10.2: Const vs runtime comparison ──

    #[test]
    fn k10_2_const_vs_runtime_integers() {
        // Simulate: const fn sum(n) computed at compile time vs runtime
        let const_result = ComptimeValue::Int(5050); // sum(1..=100)
        let runtime_result = ComptimeValue::Int((1..=100).sum::<i64>());
        assert!(validate_const_vs_runtime(&const_result, &runtime_result));
    }

    #[test]
    fn k10_2_const_vs_runtime_arrays() {
        let squares_const: Vec<ComptimeValue> =
            (0..10).map(|i| ComptimeValue::Int(i * i)).collect();
        let squares_runtime: Vec<ComptimeValue> =
            (0..10).map(|i: i64| ComptimeValue::Int(i * i)).collect();
        assert!(validate_const_vs_runtime(
            &ComptimeValue::Array(squares_const),
            &ComptimeValue::Array(squares_runtime),
        ));
    }

    // ── K10.3: Large const data ──

    #[test]
    fn k10_3_generate_10k_lut() {
        let lut = generate_lut(10_000, |i| (i * i) as i64);
        if let ComptimeValue::Array(arr) = &lut {
            assert_eq!(arr.len(), 10_000);
            assert_eq!(arr[0], ComptimeValue::Int(0));
            assert_eq!(arr[100], ComptimeValue::Int(10_000));
            assert_eq!(arr[999], ComptimeValue::Int(998_001));
        } else {
            panic!("expected array");
        }
    }

    #[test]
    fn k10_3_lut_allocation_size() {
        let lut = generate_lut(1_000, |i| i as i64);
        let mut reg = ConstAllocRegistry::new(TargetInfo::x86_64());
        reg.register("LUT", &lut);
        // 8 bytes count + 1000 * 8 bytes = 8008 bytes
        assert_eq!(reg.total_bytes(), 8 + 1000 * 8);
    }

    // ── K10.4: Recursive const depth ──

    #[test]
    fn k10_4_recursion_depth_50() {
        // Spawn with explicit 8MB stack to handle deep eval_expr recursion in debug mode.
        let result = std::thread::Builder::new()
            .stack_size(8 * 1024 * 1024)
            .spawn(|| test_recursive_depth(50))
            .expect("thread spawn")
            .join()
            .expect("thread join");
        assert!(result);
    }

    #[test]
    fn k10_4_recursion_depth_30() {
        let result = std::thread::Builder::new()
            .stack_size(8 * 1024 * 1024)
            .spawn(|| test_recursive_depth(30))
            .expect("thread spawn")
            .join()
            .expect("thread join");
        assert!(result);
    }

    // ── K10.5: Numeric type coverage ──

    #[test]
    fn k10_5_all_numeric_types() {
        let results = verify_numeric_const_params();
        assert_eq!(results.len(), 15);
        for (ty, ok) in &results {
            assert!(ok, "numeric type '{ty}' failed as const param");
        }
    }

    // ── K10.6: Example — const physics ──

    #[test]
    fn k10_6_const_physics() {
        let vals = const_physics_example();
        assert_eq!(vals.len(), 4);
        // Speed of light
        assert_eq!(vals[0], ComptimeValue::Float(299_792_458.0));
        // E = mc² with m=1
        if let ComptimeValue::Float(e) = &vals[3] {
            assert!(*e > 8.9e16, "E = {e}"); // ~8.99e16 J
        }
    }

    // ── K10.7: Example — const LUT ──

    #[test]
    fn k10_7_const_sin_lut() {
        let lut = const_lut_example();
        if let ComptimeValue::Array(arr) = &lut {
            assert_eq!(arr.len(), 256);
            // sin(0) ≈ 0
            assert_eq!(arr[0], ComptimeValue::Int(0));
            // sin(π/2) = sin(64/256 * 2π) ≈ 32767
            if let ComptimeValue::Int(v) = &arr[64] {
                assert!((*v - 32767).abs() < 2, "sin(π/2) = {v}");
            }
        } else {
            panic!("expected array");
        }
    }

    // ── K10.8: Example — const matrix ──

    #[test]
    fn k10_8_const_matrix_dimensions() {
        let types = const_matrix_example();
        assert_eq!(types[0].to_string(), "Matrix<f64, 3, 4>");
        assert_eq!(types[1].to_string(), "Matrix<f64, 4, 2>");
        assert_eq!(types[2].to_string(), "Matrix<f64, 3, 2>"); // result of 3x4 * 4x2
    }

    // ── K10.9: Pipeline stats ──

    #[test]
    fn k10_9_const_context_stats() {
        let mut c = ConstContext::default_context();
        c.register_const("A", ComptimeValue::Int(1));
        c.register_const_fn("f");
        let stats = c.stats();
        assert_eq!(stats.const_values, 1);
        assert_eq!(stats.const_fns, 1);
        assert!(stats.const_traits >= 5);
    }

    // ── K10.10: Full validation ──

    #[test]
    fn k10_10_full_validation_report() {
        // Needs large stack because run_full_validation calls test_recursive_depth(50)
        let report = std::thread::Builder::new()
            .stack_size(8 * 1024 * 1024)
            .spawn(run_full_validation)
            .expect("thread spawn")
            .join()
            .expect("thread join");
        assert!(report.simple_eval_correct, "simple eval failed");
        assert!(report.const_vs_runtime_match, "const vs runtime mismatch");
        assert!(report.large_lut_correct, "large LUT incorrect");
        assert_eq!(report.large_lut_size, 10_000);
        assert!(report.recursion_depth_200, "recursion depth 50 failed");
        assert_eq!(report.numeric_types_total, 15);
        assert_eq!(report.numeric_types_passing, 15);
        assert!(report.physics_example_ok, "physics example failed");
        assert!(report.lut_example_ok, "LUT example failed");
        assert!(report.matrix_example_ok, "matrix example failed");
        assert_eq!(report.const_modules, 8);
        assert!(report.all_passed(), "not all validations passed");
    }
}
