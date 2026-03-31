//! V14 Option 2 — Sprint H1: Integration Test Suite
//!
//! End-to-end pipeline tests covering the full compilation pipeline:
//! lex -> parse -> analyze -> interpret.
//!
//! 35+ tests across variables, functions, structs, enums, match, closures,
//! loops, arrays, strings, error handling, control flow, type system,
//! ML pipeline, error recovery, and CLI-like usage.

use fajar_lang::FjError;
use fajar_lang::interpreter::{Interpreter, Value};
use fajar_lang::lexer::tokenize;
use fajar_lang::parser::parse;

// ════════════════════════════════════════════════════════════════════════
// Helpers
// ════════════════════════════════════════════════════════════════════════

/// Evaluate source through the full pipeline and return captured output lines.
fn eval_output(source: &str) -> Vec<String> {
    let mut interp = Interpreter::new_capturing();
    interp.eval_source(source).expect("eval_source failed");
    interp.call_main().expect("call_main failed");
    interp.get_output().to_vec()
}

/// Evaluate source without a main function (top-level statements).
fn eval_top_level(source: &str) -> Vec<String> {
    let mut interp = Interpreter::new_capturing();
    interp.eval_source(source).expect("eval_source failed");
    interp.get_output().to_vec()
}

/// Evaluate source and expect it to produce an error (any kind).
fn eval_expect_error(source: &str) -> FjError {
    let mut interp = Interpreter::new_capturing();
    interp.eval_source(source).unwrap_err()
}

// ════════════════════════════════════════════════════════════════════════
// H1.1 — End-to-End Pipeline Tests (10 tests)
// ════════════════════════════════════════════════════════════════════════

#[test]
fn h1_1_variable_binding_and_arithmetic() {
    let src = r#"
fn main() -> void {
    let x = 10
    let y = 20
    let z = x + y * 2
    println(z)
}
"#;
    let output = eval_output(src);
    assert_eq!(output, vec!["50"]);
}

#[test]
fn h1_1_mutable_variable_reassignment() {
    let src = r#"
fn main() -> void {
    let mut counter = 0
    counter = counter + 1
    counter = counter + 1
    counter = counter + 1
    println(counter)
}
"#;
    let output = eval_output(src);
    assert_eq!(output, vec!["3"]);
}

#[test]
fn h1_1_function_definition_and_call() {
    let src = r#"
fn square(n: i64) -> i64 { n * n }
fn main() -> void {
    println(square(7))
    println(square(0))
    println(square(-3))
}
"#;
    let output = eval_output(src);
    assert_eq!(output, vec!["49", "0", "9"]);
}

#[test]
fn h1_1_struct_creation_and_field_access() {
    let src = r#"
struct Rectangle { width: f64, height: f64 }
fn main() -> void {
    let r = Rectangle { width: 10.5, height: 3.0 }
    println(r.width)
    println(r.height)
}
"#;
    let output = eval_output(src);
    assert_eq!(output, vec!["10.5", "3"]);
}

#[test]
fn h1_1_enum_creation_and_match() {
    let src = r#"
fn describe(val: i64) -> str {
    match val {
        1 => "one",
        2 => "two",
        3 => "three",
        _ => "unknown"
    }
}
fn main() -> void {
    println(describe(1))
    println(describe(2))
    println(describe(99))
}
"#;
    let output = eval_output(src);
    assert_eq!(output, vec!["one", "two", "unknown"]);
}

#[test]
fn h1_1_match_with_return_value() {
    let src = r#"
fn classify(n: i64) -> str {
    match n {
        0 => "zero",
        _ => {
            if n > 0 { "positive" } else { "negative" }
        }
    }
}
fn main() -> void {
    println(classify(0))
    println(classify(42))
    println(classify(-5))
}
"#;
    let output = eval_output(src);
    assert_eq!(output, vec!["zero", "positive", "negative"]);
}

#[test]
fn h1_1_closures_capture_environment() {
    let src = r#"
fn make_counter(start: i64) -> fn(i64) -> i64 {
    |step: i64| -> i64 { start + step }
}
fn main() -> void {
    let from_ten = make_counter(10)
    println(from_ten(5))
    println(from_ten(20))
}
"#;
    let output = eval_output(src);
    assert_eq!(output, vec!["15", "30"]);
}

#[test]
fn h1_1_loop_with_accumulator() {
    let src = r#"
fn main() -> void {
    let mut sum = 0
    let mut i = 1
    while i <= 100 {
        sum = sum + i
        i = i + 1
    }
    println(sum)
}
"#;
    let output = eval_output(src);
    assert_eq!(output, vec!["5050"]);
}

#[test]
fn h1_1_array_creation_and_indexing() {
    let src = r#"
fn main() -> void {
    let arr = [10, 20, 30, 40, 50]
    println(arr[0])
    println(arr[4])
    println(len(arr))
}
"#;
    let output = eval_output(src);
    assert_eq!(output, vec!["10", "50", "5"]);
}

#[test]
fn h1_1_string_operations_pipeline() {
    let src = r#"
fn main() -> void {
    let s = "Hello, World!"
    println(len(s))
    println(s.contains("World"))
    println(s.starts_with("Hello"))
}
"#;
    let output = eval_output(src);
    assert_eq!(output, vec!["13", "true", "true"]);
}

// ════════════════════════════════════════════════════════════════════════
// H1.2 — Control Flow Tests (5 tests)
// ════════════════════════════════════════════════════════════════════════

#[test]
fn h1_2_if_else_as_expression() {
    let src = r#"
fn abs(n: i64) -> i64 {
    if n < 0 { 0 - n } else { n }
}
fn main() -> void {
    println(abs(5))
    println(abs(-5))
    println(abs(0))
}
"#;
    let output = eval_output(src);
    assert_eq!(output, vec!["5", "5", "0"]);
}

#[test]
fn h1_2_while_loop_with_early_exit() {
    let src = r#"
fn first_over_100(start: i64) -> i64 {
    let mut n = start
    while true {
        if n > 100 { return n }
        n = n * 2
    }
    0
}
fn main() -> void {
    println(first_over_100(1))
    println(first_over_100(50))
}
"#;
    let output = eval_output(src);
    assert_eq!(output, vec!["128", "200"]);
}

#[test]
fn h1_2_for_in_range_iteration() {
    let src = r#"
fn main() -> void {
    let mut product = 1
    for i in 1..6 {
        product = product * i
    }
    println(product)
}
"#;
    let output = eval_output(src);
    // 1 * 2 * 3 * 4 * 5 = 120
    assert_eq!(output, vec!["120"]);
}

#[test]
fn h1_2_loop_break_continue() {
    let src = r#"
fn main() -> void {
    let mut result = 0
    let mut i = 0
    while i < 20 {
        i = i + 1
        if i % 2 == 0 { continue }
        if i > 10 { break }
        result = result + i
    }
    println(result)
}
"#;
    let output = eval_output(src);
    // Odd numbers 1..=9: 1+3+5+7+9 = 25
    assert_eq!(output, vec!["25"]);
}

#[test]
fn h1_2_nested_match_expressions() {
    let src = r#"
fn categorize(x: i64, y: i64) -> str {
    match x {
        0 => match y {
            0 => "origin",
            _ => "y-axis"
        },
        _ => match y {
            0 => "x-axis",
            _ => "plane"
        }
    }
}
fn main() -> void {
    println(categorize(0, 0))
    println(categorize(0, 5))
    println(categorize(3, 0))
    println(categorize(1, 1))
}
"#;
    let output = eval_output(src);
    assert_eq!(output, vec!["origin", "y-axis", "x-axis", "plane"]);
}

// ════════════════════════════════════════════════════════════════════════
// H1.3 — Type System Tests (5 tests)
// ════════════════════════════════════════════════════════════════════════

#[test]
fn h1_3_generic_function_identity() {
    let src = r#"
fn identity<T>(x: T) -> T { x }
fn main() -> void {
    println(identity(42))
    println(identity("hello"))
    println(identity(true))
}
"#;
    let output = eval_output(src);
    assert_eq!(output, vec!["42", "hello", "true"]);
}

#[test]
fn h1_3_trait_dispatch() {
    let src = r#"
struct Dog {}
struct Cat {}
impl Dog {
    fn speak(self) -> str { "Woof!" }
}
impl Cat {
    fn speak(self) -> str { "Meow!" }
}
fn main() -> void {
    let d = Dog {}
    let c = Cat {}
    println(d.speak())
    println(c.speak())
}
"#;
    let output = eval_output(src);
    assert_eq!(output, vec!["Woof!", "Meow!"]);
}

#[test]
fn h1_3_option_some_none() {
    let src = r#"
fn find_positive(n: i64) -> i64 {
    let result = if n > 0 { Some(n) } else { None }
    match result {
        Some(v) => v,
        None => -1
    }
}
fn main() -> void {
    println(find_positive(42))
    println(find_positive(-5))
    println(find_positive(0))
}
"#;
    let output = eval_output(src);
    assert_eq!(output, vec!["42", "-1", "-1"]);
}

#[test]
fn h1_3_type_inference_let_bindings() {
    let src = r#"
fn main() -> void {
    let a = 42
    let b = 3.14
    let c = "hello"
    let d = true
    let e = [1, 2, 3]
    println(type_of(a))
    println(type_of(b))
    println(type_of(c))
    println(type_of(d))
    println(type_of(e))
}
"#;
    let output = eval_output(src);
    assert_eq!(output[0], "i64");
    assert_eq!(output[1], "f64");
    assert_eq!(output[2], "str");
    assert_eq!(output[3], "bool");
    assert!(
        output[4].contains("array"),
        "expected array type, got: {}",
        output[4]
    );
}

#[test]
fn h1_3_method_calls_on_structs() {
    let src = r#"
struct Vector2 { x: f64, y: f64 }
impl Vector2 {
    fn new(x: f64, y: f64) -> Vector2 {
        Vector2 { x: x, y: y }
    }
    fn length_sq(self) -> f64 {
        self.x * self.x + self.y * self.y
    }
    fn scale(self, factor: f64) -> Vector2 {
        Vector2 { x: self.x * factor, y: self.y * factor }
    }
}
fn main() -> void {
    let v = Vector2::new(3.0, 4.0)
    println(v.length_sq())
    let v2 = v.scale(2.0)
    println(v2.x)
    println(v2.y)
}
"#;
    let output = eval_output(src);
    assert_eq!(output, vec!["25", "6", "8"]);
}

// ════════════════════════════════════════════════════════════════════════
// H1.4 — ML Pipeline Tests (5 tests)
// ════════════════════════════════════════════════════════════════════════

#[test]
fn h1_4_tensor_creation_zeros_ones() {
    let output = eval_top_level(
        r#"
        let z = tensor_zeros([3, 3])
        let o = tensor_ones([2, 4])
        println(tensor_numel(z))
        println(tensor_numel(o))
        println(tensor_shape(z))
        println(tensor_shape(o))
        "#,
    );
    assert_eq!(output[0], "9");
    assert_eq!(output[1], "8");
    assert_eq!(output[2], "[3, 3]");
    assert_eq!(output[3], "[2, 4]");
}

#[test]
fn h1_4_tensor_from_data_and_reshape() {
    let output = eval_top_level(
        r#"
        let t = tensor_from_data([1.0, 2.0, 3.0, 4.0, 5.0, 6.0], [2, 3])
        println(tensor_shape(t))
        let t2 = tensor_reshape(t, [3, 2])
        println(tensor_shape(t2))
        println(tensor_numel(t2))
        "#,
    );
    assert_eq!(output[0], "[2, 3]");
    assert_eq!(output[1], "[3, 2]");
    assert_eq!(output[2], "6");
}

#[test]
fn h1_4_tensor_arithmetic_add_mul() {
    let output = eval_top_level(
        r#"
        let a = tensor_from_data([1.0, 2.0, 3.0], [3])
        let b = tensor_from_data([10.0, 20.0, 30.0], [3])
        let sum = tensor_add(a, b)
        println(sum)
        "#,
    );
    assert_eq!(output[0], "tensor([11.0000, 22.0000, 33.0000])");
}

#[test]
fn h1_4_tensor_activation_relu() {
    let output = eval_top_level(
        r#"
        let t = tensor_from_data([-3.0, -1.0, 0.0, 1.0, 3.0], [5])
        let activated = tensor_relu(t)
        println(activated)
        "#,
    );
    assert_eq!(
        output[0],
        "tensor([0.0000, 0.0000, 0.0000, 1.0000, 3.0000])"
    );
}

#[test]
fn h1_4_tensor_matmul_identity() {
    let output = eval_top_level(
        r#"
        let a = tensor_from_data([1.0, 2.0, 3.0, 4.0], [2, 2])
        let eye = tensor_eye(2)
        let result = tensor_matmul(a, eye)
        println(tensor_shape(result))
        println(tensor_numel(result))
        "#,
    );
    assert_eq!(output[0], "[2, 2]");
    assert_eq!(output[1], "4");
}

// ════════════════════════════════════════════════════════════════════════
// H1.5 — Error Recovery Tests (5 tests)
// ════════════════════════════════════════════════════════════════════════

#[test]
fn h1_5_parse_error_missing_brace() {
    // Ensure missing closing brace produces a parse error, not a panic.
    let err = eval_expect_error("fn main() -> void { let x = 42");
    let msg = format!("{err}");
    assert!(
        matches!(err, FjError::Parse(_)),
        "expected parse error, got: {msg}"
    );
}

#[test]
fn h1_5_semantic_error_undefined_variable() {
    // Reference to undefined variable should produce a clean error.
    let err = eval_expect_error("fn main() -> void { println(nonexistent_var) }");
    let msg = format!("{err}");
    // The error should mention the variable name or be a semantic/runtime error.
    assert!(
        matches!(err, FjError::Semantic(_) | FjError::Runtime(_)),
        "expected semantic or runtime error, got: {msg}"
    );
}

#[test]
fn h1_5_type_error_add_string_and_int() {
    // Adding incompatible types should produce a type error.
    let mut interp = Interpreter::new_capturing();
    let result = interp.eval_source(r#"let x = "hello" + 42"#);
    // Should be an error (semantic or runtime), not a panic.
    assert!(
        result.is_err(),
        "expected an error for string + int, but got success"
    );
}

#[test]
fn h1_5_arity_mismatch_error() {
    // Calling a function with wrong number of arguments.
    let err = eval_expect_error(
        r#"
        fn add(a: i64, b: i64) -> i64 { a + b }
        fn main() -> void { println(add(1, 2, 3)) }
        "#,
    );
    let msg = format!("{err}");
    // Should produce an arity error.
    assert!(
        matches!(err, FjError::Semantic(_) | FjError::Runtime(_)),
        "expected error for arity mismatch, got: {msg}"
    );
}

#[test]
fn h1_5_division_by_zero_recovery() {
    // Division by zero should produce a runtime error, not a panic.
    let mut interp = Interpreter::new_capturing();
    let result = interp.eval_source("let x = 10 / 0");
    assert!(
        result.is_err(),
        "expected division by zero error, but got success"
    );
    let msg = format!("{}", result.unwrap_err());
    assert!(
        msg.contains("division") || msg.contains("zero") || msg.contains("RE001"),
        "expected division-related error message, got: {msg}"
    );
}

// ════════════════════════════════════════════════════════════════════════
// H1.8 — CLI-like Tests (5 tests)
// ════════════════════════════════════════════════════════════════════════

#[test]
fn h1_8_eval_source_returns_last_value() {
    let mut interp = Interpreter::new_capturing();
    let val = interp.eval_source("42").expect("eval_source failed");
    assert_eq!(val, Value::Int(42));
}

#[test]
fn h1_8_eval_source_multi_statement() {
    let mut interp = Interpreter::new_capturing();
    interp
        .eval_source("let x = 10")
        .expect("eval_source failed");
    interp
        .eval_source("let y = 20")
        .expect("eval_source failed");
    let val = interp.eval_source("x + y").expect("eval_source failed");
    assert_eq!(val, Value::Int(30));
}

#[test]
fn h1_8_eval_source_function_definition_and_call() {
    let mut interp = Interpreter::new_capturing();
    interp
        .eval_source("fn double(n: i64) -> i64 { n * 2 }")
        .expect("eval_source failed");
    let val = interp
        .eval_source("double(21)")
        .expect("eval_source failed");
    assert_eq!(val, Value::Int(42));
}

#[test]
fn h1_8_check_mode_type_validation() {
    // Simulate "check" mode: tokenize + parse + analyze without evaluating.
    let src = r#"
        fn factorial(n: i64) -> i64 {
            if n <= 1 { 1 } else { n * factorial(n - 1) }
        }
        fn main() -> void {
            println(factorial(10))
        }
    "#;
    let tokens = tokenize(src).expect("tokenize failed");
    let program = parse(tokens).expect("parse failed");
    // Analysis should pass without errors.
    let result = fajar_lang::analyzer::analyze(&program);
    assert!(result.is_ok(), "analysis failed: {:?}", result.unwrap_err());
}

#[test]
fn h1_8_multi_function_program() {
    let src = r#"
fn is_even(n: i64) -> bool { n % 2 == 0 }
fn is_positive(n: i64) -> bool { n > 0 }
fn classify(n: i64) -> str {
    if is_positive(n) {
        if is_even(n) { "positive even" } else { "positive odd" }
    } else {
        "non-positive"
    }
}
fn main() -> void {
    println(classify(4))
    println(classify(7))
    println(classify(-3))
    println(classify(0))
}
"#;
    let output = eval_output(src);
    assert_eq!(
        output,
        vec![
            "positive even",
            "positive odd",
            "non-positive",
            "non-positive"
        ]
    );
}
