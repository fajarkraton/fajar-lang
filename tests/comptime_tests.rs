//! Compile-time evaluation integration tests for Fajar Lang.
//!
//! Tests `comptime` blocks, `comptime fn`, comptime parameters,
//! and compile-time restrictions.

#![allow(dead_code)]

use fajar_lang::FjError;
use fajar_lang::analyzer::comptime::{ComptimeError, ComptimeEvaluator, ComptimeValue};
use fajar_lang::interpreter::{Interpreter, Value};

/// Helper: run through full pipeline.
fn eval(source: &str) -> Result<Value, FjError> {
    let mut interp = Interpreter::new_capturing();
    interp.eval_source(source)
}

/// Helper: run and expect success.
fn eval_ok(source: &str) -> Value {
    eval(source).unwrap_or_else(|e| panic!("expected success, got: {e}"))
}

/// Helper: parse and check analysis passes.
fn expect_analysis_ok(source: &str) {
    let tokens = fajar_lang::lexer::tokenize(source).expect("lex failed");
    let program = fajar_lang::parser::parse(tokens).expect("parse failed");
    match fajar_lang::analyzer::analyze(&program) {
        Ok(()) => {}
        Err(errors) => {
            let hard = errors.iter().filter(|e| !e.is_warning()).count();
            assert!(hard == 0, "unexpected hard errors: {errors:?}");
        }
    }
}

/// Helper: parse successfully.
fn parse_ok(source: &str) {
    let tokens = fajar_lang::lexer::tokenize(source).expect("lex failed");
    fajar_lang::parser::parse(tokens).expect("parse failed");
}

/// Helper: evaluate comptime expression using the comptime evaluator.
fn comptime_eval(source: &str) -> Result<ComptimeValue, ComptimeError> {
    let tokens = fajar_lang::lexer::tokenize(source).unwrap();
    let program = fajar_lang::parser::parse(tokens).unwrap();
    let mut eval = ComptimeEvaluator::new();
    eval.collect_functions(&program);
    for item in &program.items {
        if let fajar_lang::parser::ast::Item::Stmt(fajar_lang::parser::ast::Stmt::Expr {
            expr,
            ..
        }) = item
        {
            return eval.eval_expr(expr);
        }
    }
    Err(ComptimeError::NotComptime {
        reason: "no expression found".into(),
    })
}

// ════════════════════════════════════════════════════════════════════════
// 1. Lexer: comptime keyword
// ════════════════════════════════════════════════════════════════════════

#[test]
fn comptime_keyword_lexes() {
    let tokens = fajar_lang::lexer::tokenize("comptime").unwrap();
    assert_eq!(format!("{}", tokens[0].kind), "comptime");
}

// ════════════════════════════════════════════════════════════════════════
// 2. Parser: comptime blocks
// ════════════════════════════════════════════════════════════════════════

#[test]
fn parse_comptime_block() {
    parse_ok("comptime { 42 }");
}

#[test]
fn parse_comptime_block_with_let() {
    parse_ok("comptime { let x = 5\n x + 3 }");
}

#[test]
fn parse_comptime_fn_alias() {
    parse_ok(
        "comptime fn factorial(n: i64) -> i64 { if n <= 1 { 1 } else { n * factorial(n - 1) } }",
    );
}

#[test]
fn parse_comptime_generic_param() {
    // comptime params use trait-bound syntax: `comptime N: Const`
    // Type constraint like `i64` is not directly supported as a bound
    // (it's a keyword), so we use a trait-style bound
    parse_ok("fn zeros<comptime N>() -> i64 { 0 }");
}

#[test]
fn parse_comptime_generic_param_with_bound() {
    parse_ok("fn make<comptime N: Copy>() -> i64 { 0 }");
}

#[test]
fn parse_comptime_in_const_decl() {
    parse_ok("const X: i64 = comptime { 2 + 3 }");
}

// ════════════════════════════════════════════════════════════════════════
// 3. Comptime evaluator: arithmetic
// ════════════════════════════════════════════════════════════════════════

#[test]
fn comptime_eval_int_add() {
    let r = comptime_eval("comptime { 3 + 4 }").unwrap();
    assert_eq!(r, ComptimeValue::Int(7));
}

#[test]
fn comptime_eval_int_sub() {
    let r = comptime_eval("comptime { 10 - 3 }").unwrap();
    assert_eq!(r, ComptimeValue::Int(7));
}

#[test]
fn comptime_eval_int_mul() {
    let r = comptime_eval("comptime { 6 * 7 }").unwrap();
    assert_eq!(r, ComptimeValue::Int(42));
}

#[test]
fn comptime_eval_int_div() {
    let r = comptime_eval("comptime { 84 / 2 }").unwrap();
    assert_eq!(r, ComptimeValue::Int(42));
}

#[test]
fn comptime_eval_int_rem() {
    let r = comptime_eval("comptime { 10 % 3 }").unwrap();
    assert_eq!(r, ComptimeValue::Int(1));
}

#[test]
fn comptime_eval_nested_arithmetic() {
    let r = comptime_eval("comptime { (2 + 3) * (4 - 1) }").unwrap();
    assert_eq!(r, ComptimeValue::Int(15));
}

#[test]
fn comptime_eval_negation() {
    let r = comptime_eval("comptime { -42 }").unwrap();
    assert_eq!(r, ComptimeValue::Int(-42));
}

#[test]
fn comptime_eval_bitwise_and() {
    let r = comptime_eval("comptime { 0xFF & 0x0F }").unwrap();
    assert_eq!(r, ComptimeValue::Int(0x0F));
}

#[test]
fn comptime_eval_bitwise_or() {
    let r = comptime_eval("comptime { 0xF0 | 0x0F }").unwrap();
    assert_eq!(r, ComptimeValue::Int(0xFF));
}

#[test]
fn comptime_eval_bitwise_xor() {
    let r = comptime_eval("comptime { 0xFF ^ 0x0F }").unwrap();
    assert_eq!(r, ComptimeValue::Int(0xF0));
}

#[test]
fn comptime_eval_shift_left() {
    let r = comptime_eval("comptime { 1 << 8 }").unwrap();
    assert_eq!(r, ComptimeValue::Int(256));
}

#[test]
fn comptime_eval_shift_right() {
    let r = comptime_eval("comptime { 256 >> 4 }").unwrap();
    assert_eq!(r, ComptimeValue::Int(16));
}

// ════════════════════════════════════════════════════════════════════════
// 4. Comptime evaluator: comparisons and booleans
// ════════════════════════════════════════════════════════════════════════

#[test]
fn comptime_eval_eq() {
    let r = comptime_eval("comptime { 5 == 5 }").unwrap();
    assert_eq!(r, ComptimeValue::Bool(true));
}

#[test]
fn comptime_eval_ne() {
    let r = comptime_eval("comptime { 5 != 3 }").unwrap();
    assert_eq!(r, ComptimeValue::Bool(true));
}

#[test]
fn comptime_eval_lt() {
    let r = comptime_eval("comptime { 3 < 5 }").unwrap();
    assert_eq!(r, ComptimeValue::Bool(true));
}

#[test]
fn comptime_eval_ge() {
    let r = comptime_eval("comptime { 5 >= 5 }").unwrap();
    assert_eq!(r, ComptimeValue::Bool(true));
}

#[test]
fn comptime_eval_not() {
    let r = comptime_eval("comptime { !false }").unwrap();
    assert_eq!(r, ComptimeValue::Bool(true));
}

#[test]
fn comptime_eval_and() {
    let r = comptime_eval("comptime { true && false }").unwrap();
    assert_eq!(r, ComptimeValue::Bool(false));
}

#[test]
fn comptime_eval_or() {
    let r = comptime_eval("comptime { false || true }").unwrap();
    assert_eq!(r, ComptimeValue::Bool(true));
}

// ════════════════════════════════════════════════════════════════════════
// 5. Comptime evaluator: control flow
// ════════════════════════════════════════════════════════════════════════

#[test]
fn comptime_eval_if_true() {
    let r = comptime_eval("comptime { if true { 10 } else { 20 } }").unwrap();
    assert_eq!(r, ComptimeValue::Int(10));
}

#[test]
fn comptime_eval_if_false() {
    let r = comptime_eval("comptime { if false { 10 } else { 20 } }").unwrap();
    assert_eq!(r, ComptimeValue::Int(20));
}

#[test]
fn comptime_eval_if_expr_condition() {
    let r = comptime_eval("comptime { if 5 > 3 { 1 } else { 0 } }").unwrap();
    assert_eq!(r, ComptimeValue::Int(1));
}

#[test]
fn comptime_eval_nested_if() {
    let r = comptime_eval("comptime { if true { if false { 1 } else { 2 } } else { 3 } }").unwrap();
    assert_eq!(r, ComptimeValue::Int(2));
}

// ════════════════════════════════════════════════════════════════════════
// 6. Comptime evaluator: blocks and variables
// ════════════════════════════════════════════════════════════════════════

#[test]
fn comptime_eval_let_binding() {
    let r = comptime_eval("comptime { let x = 42\n x }").unwrap();
    assert_eq!(r, ComptimeValue::Int(42));
}

#[test]
fn comptime_eval_multiple_lets() {
    let r = comptime_eval("comptime { let a = 10\n let b = 20\n a + b }").unwrap();
    assert_eq!(r, ComptimeValue::Int(30));
}

#[test]
fn comptime_eval_let_chain() {
    let r = comptime_eval("comptime { let x = 5\n let y = x * 2\n y + 1 }").unwrap();
    assert_eq!(r, ComptimeValue::Int(11));
}

// ════════════════════════════════════════════════════════════════════════
// 7. Comptime evaluator: function calls
// ════════════════════════════════════════════════════════════════════════

#[test]
fn comptime_eval_const_fn_call() {
    let r = comptime_eval(
        r#"
const fn double(x: i64) -> i64 { x * 2 }
comptime { double(21) }
"#,
    )
    .unwrap();
    assert_eq!(r, ComptimeValue::Int(42));
}

#[test]
fn comptime_eval_recursive_factorial() {
    let r = comptime_eval(
        r#"
const fn factorial(n: i64) -> i64 {
    if n <= 1 { 1 } else { n * factorial(n - 1) }
}
comptime { factorial(10) }
"#,
    )
    .unwrap();
    assert_eq!(r, ComptimeValue::Int(3628800));
}

#[test]
fn comptime_eval_fibonacci() {
    let r = comptime_eval(
        r#"
const fn fib(n: i64) -> i64 {
    if n <= 1 { n } else { fib(n - 1) + fib(n - 2) }
}
comptime { fib(20) }
"#,
    )
    .unwrap();
    assert_eq!(r, ComptimeValue::Int(6765));
}

#[test]
fn comptime_eval_multi_param_fn() {
    let r = comptime_eval(
        r#"
const fn add(a: i64, b: i64) -> i64 { a + b }
comptime { add(17, 25) }
"#,
    )
    .unwrap();
    assert_eq!(r, ComptimeValue::Int(42));
}

#[test]
fn comptime_eval_chained_fn_calls() {
    let r = comptime_eval(
        r#"
const fn double(x: i64) -> i64 { x * 2 }
const fn inc(x: i64) -> i64 { x + 1 }
comptime { inc(double(20)) }
"#,
    )
    .unwrap();
    assert_eq!(r, ComptimeValue::Int(41));
}

// ════════════════════════════════════════════════════════════════════════
// 8. Comptime evaluator: arrays
// ════════════════════════════════════════════════════════════════════════

#[test]
fn comptime_eval_array_literal() {
    let r = comptime_eval("comptime { [1, 2, 3] }").unwrap();
    assert_eq!(
        r,
        ComptimeValue::Array(vec![
            ComptimeValue::Int(1),
            ComptimeValue::Int(2),
            ComptimeValue::Int(3),
        ])
    );
}

#[test]
fn comptime_eval_array_index() {
    let r = comptime_eval("comptime { let arr = [10, 20, 30]\n arr[1] }").unwrap();
    assert_eq!(r, ComptimeValue::Int(20));
}

// ════════════════════════════════════════════════════════════════════════
// 9. Comptime evaluator: floats
// ════════════════════════════════════════════════════════════════════════

#[test]
fn comptime_eval_float_add() {
    let r = comptime_eval("comptime { 1.5 + 2.5 }").unwrap();
    assert_eq!(r, ComptimeValue::Float(4.0));
}

#[test]
fn comptime_eval_float_mul() {
    let r = comptime_eval("comptime { 3.0 * 2.5 }").unwrap();
    assert_eq!(r, ComptimeValue::Float(7.5));
}

#[test]
fn comptime_eval_float_comparison() {
    let r = comptime_eval("comptime { 3.5 > 2.72 }").unwrap();
    assert_eq!(r, ComptimeValue::Bool(true));
}

// ════════════════════════════════════════════════════════════════════════
// 10. Comptime evaluator: strings
// ════════════════════════════════════════════════════════════════════════

#[test]
fn comptime_eval_string_literal() {
    let r = comptime_eval(r#"comptime { "hello" }"#).unwrap();
    assert_eq!(r, ComptimeValue::Str("hello".into()));
}

#[test]
fn comptime_eval_string_concat() {
    let r = comptime_eval(r#"comptime { "hello" + " world" }"#).unwrap();
    assert_eq!(r, ComptimeValue::Str("hello world".into()));
}

// ════════════════════════════════════════════════════════════════════════
// 11. Comptime evaluator: errors and restrictions
// ════════════════════════════════════════════════════════════════════════

#[test]
fn comptime_eval_division_by_zero() {
    let r = comptime_eval("comptime { 1 / 0 }");
    assert!(matches!(r, Err(ComptimeError::DivisionByZero)));
}

#[test]
fn comptime_eval_undefined_var() {
    let r = comptime_eval("comptime { unknown_var }");
    assert!(matches!(r, Err(ComptimeError::UndefinedVariable { .. })));
}

#[test]
fn comptime_eval_undefined_fn() {
    let r = comptime_eval("comptime { unknown_fn(1) }");
    assert!(matches!(r, Err(ComptimeError::UndefinedFunction { .. })));
}

#[test]
fn comptime_eval_io_forbidden_print() {
    let source = r#"
const fn bad() -> i64 { println("oops") }
comptime { bad() }
"#;
    let r = comptime_eval(source);
    assert!(matches!(r, Err(ComptimeError::IoForbidden)));
}

#[test]
fn comptime_eval_io_forbidden_read_file() {
    let source = r#"
const fn bad() -> i64 { read_file("x.txt") }
comptime { bad() }
"#;
    let r = comptime_eval(source);
    assert!(matches!(r, Err(ComptimeError::IoForbidden)));
}

// ════════════════════════════════════════════════════════════════════════
// 12. Interpreter: comptime blocks run at runtime too
// ════════════════════════════════════════════════════════════════════════

#[test]
fn interpreter_comptime_block_returns_value() {
    let mut interp = Interpreter::new_capturing();
    let r = interp.eval_source("comptime { 42 }");
    assert!(r.is_ok());
}

#[test]
fn interpreter_comptime_fn_runs() {
    let mut interp = Interpreter::new_capturing();
    interp
        .eval_source(
            r#"
comptime fn triple(x: i64) -> i64 { x * 3 }
fn main() { println(triple(14)) }
"#,
        )
        .unwrap();
    interp.call_main().unwrap();
    let output = interp.get_output();
    assert!(output.iter().any(|l| l.contains("42")));
}

// ════════════════════════════════════════════════════════════════════════
// 13. Comptime evaluator: ComptimeValue methods
// ════════════════════════════════════════════════════════════════════════

#[test]
fn comptime_value_as_int() {
    assert_eq!(ComptimeValue::Int(42).as_int(), Some(42));
    assert_eq!(ComptimeValue::Bool(true).as_int(), Some(1));
    assert_eq!(ComptimeValue::Bool(false).as_int(), Some(0));
    assert_eq!(ComptimeValue::Float(1.25).as_int(), None);
}

#[test]
fn comptime_value_as_float() {
    assert_eq!(ComptimeValue::Float(1.25).as_float(), Some(1.25));
    assert_eq!(ComptimeValue::Int(42).as_float(), Some(42.0));
    assert_eq!(ComptimeValue::Bool(true).as_float(), None);
}

#[test]
fn comptime_value_as_bool() {
    assert_eq!(ComptimeValue::Bool(true).as_bool(), Some(true));
    assert_eq!(ComptimeValue::Int(0).as_bool(), Some(false));
    assert_eq!(ComptimeValue::Int(1).as_bool(), Some(true));
}

#[test]
fn comptime_value_display() {
    assert_eq!(format!("{}", ComptimeValue::Int(42)), "42");
    assert_eq!(format!("{}", ComptimeValue::Float(1.25)), "1.25");
    assert_eq!(format!("{}", ComptimeValue::Bool(true)), "true");
    assert_eq!(format!("{}", ComptimeValue::Str("hi".into())), "hi");
    assert_eq!(format!("{}", ComptimeValue::Null), "null");
    assert_eq!(
        format!(
            "{}",
            ComptimeValue::Array(vec![ComptimeValue::Int(1), ComptimeValue::Int(2)])
        ),
        "[1, 2]"
    );
}
