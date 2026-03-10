//! End-to-end integration tests for the Fajar Lang interpreter.
//!
//! These tests run complete `.fj` programs through the full pipeline
//! (lex → parse → analyze → interpret) and verify output.

use fajar_lang::analyzer::analyze;
use fajar_lang::interpreter::Interpreter;
use fajar_lang::interpreter::Value;
use fajar_lang::lexer::tokenize;
use fajar_lang::parser::parse;

/// Helper: evaluates source code and returns captured output lines.
fn eval_output(source: &str) -> Vec<String> {
    let mut interp = Interpreter::new_capturing();
    interp.eval_source(source).expect("eval_source failed");
    interp.call_main().expect("call_main failed");
    interp.get_output().to_vec()
}

#[test]
fn e2e_hello_world() {
    let source = std::fs::read_to_string("examples/hello.fj").expect("cannot read hello.fj");
    let mut interp = Interpreter::new_capturing();
    interp.eval_source(&source).expect("eval failed");
    interp.call_main().expect("call_main failed");
    let output = interp.get_output();
    assert!(
        output.iter().any(|line| line.contains("Hello")),
        "expected 'Hello' in output, got: {output:?}"
    );
}

#[test]
fn e2e_fibonacci() {
    let source =
        std::fs::read_to_string("examples/fibonacci.fj").expect("cannot read fibonacci.fj");
    let mut interp = Interpreter::new_capturing();
    interp.eval_source(&source).expect("eval failed");
    interp.call_main().expect("call_main failed");
    let output = interp.get_output();
    // Fibonacci sequence should include 0, 1, 1, 2, 3, 5, 8
    assert!(
        output.len() >= 7,
        "expected at least 7 lines, got {}",
        output.len()
    );
    assert_eq!(output[0], "0");
    assert_eq!(output[1], "1");
    assert_eq!(output[2], "1");
    assert_eq!(output[3], "2");
    assert_eq!(output[4], "3");
    assert_eq!(output[5], "5");
    assert_eq!(output[6], "8");
}

#[test]
fn e2e_factorial() {
    let source =
        std::fs::read_to_string("examples/factorial.fj").expect("cannot read factorial.fj");
    let mut interp = Interpreter::new_capturing();
    interp.eval_source(&source).expect("eval failed");
    interp.call_main().expect("call_main failed");
    let output = interp.get_output();
    assert!(
        output.len() >= 5,
        "expected at least 5 lines, got {}",
        output.len()
    );
    // factorial(0)=1, factorial(1)=1, factorial(2)=2, factorial(3)=6, factorial(4)=24
    assert_eq!(output[0], "1");
    assert_eq!(output[1], "1");
    assert_eq!(output[2], "2");
    assert_eq!(output[3], "6");
    assert_eq!(output[4], "24");
}

#[test]
fn e2e_simple_expression() {
    let output = eval_output("fn main() -> void { println(1 + 2 * 3) }");
    assert_eq!(output, vec!["7"]);
}

#[test]
fn e2e_recursive_function() {
    let source = r#"
fn fib(n: i64) -> i64 {
    if n <= 1 { n } else { fib(n - 1) + fib(n - 2) }
}
fn main() -> void {
    println(fib(10))
}
"#;
    let output = eval_output(source);
    assert_eq!(output, vec!["55"]);
}

#[test]
fn e2e_struct_and_field_access() {
    let source = r#"
struct Point { x: i64, y: i64 }
fn main() -> void {
    let p = Point { x: 10, y: 20 }
    println(p.x + p.y)
}
"#;
    let output = eval_output(source);
    assert_eq!(output, vec!["30"]);
}

#[test]
fn e2e_closures() {
    let source = r#"
fn make_adder(n: i64) -> fn(i64) -> i64 {
    |x: i64| -> i64 { x + n }
}
fn main() -> void {
    let add5 = make_adder(5)
    println(add5(10))
}
"#;
    let output = eval_output(source);
    assert_eq!(output, vec!["15"]);
}

#[test]
fn e2e_match_expression() {
    let source = r#"
fn describe(n: i64) -> str {
    match n {
        0 => "zero",
        1 => "one",
        _ => "other"
    }
}
fn main() -> void {
    println(describe(0))
    println(describe(1))
    println(describe(42))
}
"#;
    let output = eval_output(source);
    assert_eq!(output, vec!["zero", "one", "other"]);
}

#[test]
fn e2e_for_loop_with_range() {
    let source = r#"
fn main() -> void {
    let mut sum = 0
    for i in 0..5 {
        sum = sum + i
    }
    println(sum)
}
"#;
    let output = eval_output(source);
    assert_eq!(output, vec!["10"]);
}

#[test]
fn e2e_pipeline_operator() {
    let source = r#"
fn double(n: i64) -> i64 { n * 2 }
fn add_one(n: i64) -> i64 { n + 1 }
fn main() -> void {
    let result = 5 |> double |> add_one
    println(result)
}
"#;
    let output = eval_output(source);
    assert_eq!(output, vec!["11"]);
}

#[test]
fn e2e_eval_source_convenience() {
    let mut interp = Interpreter::new_capturing();
    interp
        .eval_source("let x = 42")
        .expect("eval_source failed");
    let val = interp.eval_source("x").expect("eval_source failed");
    assert_eq!(val, Value::Int(42));
}

#[test]
fn e2e_call_fn_by_name() {
    let mut interp = Interpreter::new();
    interp
        .eval_source("fn add(a: i64, b: i64) -> i64 { a + b }")
        .expect("eval_source failed");
    let result = interp
        .call_fn("add", vec![Value::Int(3), Value::Int(4)])
        .expect("call_fn failed");
    assert_eq!(result, Value::Int(7));
}

#[test]
fn e2e_impl_block_method_dispatch() {
    let src = r#"
        struct Point { x: f64, y: f64 }
        impl Point {
            fn new(x: f64, y: f64) -> Point {
                Point { x: x, y: y }
            }
            fn distance_sq(self) -> f64 {
                self.x * self.x + self.y * self.y
            }
            fn translate(self, dx: f64, dy: f64) -> Point {
                Point { x: self.x + dx, y: self.y + dy }
            }
        }

        let p = Point::new(3.0, 4.0)
        println(p.distance_sq())
        let p2 = p.translate(1.0, 1.0)
        println(p2.x)
        println(p2.y)
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["25", "4", "5"]);
}

#[test]
fn e2e_option_result_try_operator() {
    let src = r#"
        fn divide(a: f64, b: f64) -> f64 {
            if b == 0.0 { Err("division by zero") }
            else { Ok(a / b) }
        }
        fn compute() -> f64 {
            let x = divide(10.0, 2.0)?
            let y = divide(x, 2.5)?
            Ok(x + y)
        }
        let result = compute()
        println(result.is_ok())
        println(result.unwrap())
    "#;
    let output = eval_output(src);
    assert_eq!(output[0], "true");
    assert_eq!(output[1], "7");
}

#[test]
fn e2e_option_match_and_methods() {
    let src = r#"
        let a = Some(42)
        let b = None
        println(a.is_some())
        println(b.is_none())
        println(a.unwrap())
        println(b.unwrap_or(0))
        let result = match a {
            Some(x) => x * 2,
            None => 0
        }
        println(result)
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["true", "true", "42", "0", "84"]);
}

#[test]
fn e2e_impl_multiple_structs() {
    let src = r#"
        struct Circle { radius: f64 }
        struct Square { side: f64 }
        impl Circle {
            fn area(self) -> f64 { 3.14159 * self.radius * self.radius }
        }
        impl Square {
            fn area(self) -> f64 { self.side * self.side }
        }
        let c = Circle { radius: 5.0 }
        let s = Square { side: 4.0 }
        println(s.area())
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["16"]);
}

// ── Module System Tests ──────────────────────────────────────────────

#[test]
fn e2e_inline_module_qualified_access() {
    let src = r#"
        mod math {
            fn square(x: i64) -> i64 { x * x }
            fn cube(x: i64) -> i64 { x * x * x }
        }
        fn main() -> void {
            println(math::square(5))
            println(math::cube(3))
        }
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["25", "27"]);
}

#[test]
fn e2e_use_simple_import() {
    let src = r#"
        mod math {
            fn double(x: i64) -> i64 { x * 2 }
        }
        use math::double
        fn main() -> void {
            println(double(7))
        }
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["14"]);
}

#[test]
fn e2e_use_glob_import() {
    let src = r#"
        mod utils {
            fn add(a: i64, b: i64) -> i64 { a + b }
            fn sub(a: i64, b: i64) -> i64 { a - b }
        }
        use utils::*
        fn main() -> void {
            println(add(10, 3))
            println(sub(10, 3))
        }
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["13", "7"]);
}

#[test]
fn e2e_use_group_import() {
    let src = r#"
        mod ops {
            fn inc(x: i64) -> i64 { x + 1 }
            fn dec(x: i64) -> i64 { x - 1 }
            fn negate(x: i64) -> i64 { 0 - x }
        }
        use ops::{inc, dec}
        fn main() -> void {
            println(inc(5))
            println(dec(5))
        }
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["6", "4"]);
}

#[test]
fn e2e_module_with_struct() {
    let src = r#"
        mod geo {
            struct Point { x: i64, y: i64 }
            fn origin() -> Point { Point { x: 0, y: 0 } }
        }
        use geo::*
        fn main() -> void {
            let p = origin()
            println(p.x)
            println(p.y)
        }
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["0", "0"]);
}

#[test]
fn e2e_module_const() {
    let src = r#"
        mod config {
            const MAX: i64 = 100
        }
        fn main() -> void {
            println(config::MAX)
        }
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["100"]);
}

#[test]
fn e2e_nested_modules() {
    let src = r#"
        mod outer {
            mod inner {
                fn secret() -> i64 { 42 }
            }
        }
        fn main() -> void {
            println(outer::inner::secret())
        }
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["42"]);
}

// ── Cast Expression & Minor Gaps Tests ───────────────────────────────

#[test]
fn e2e_cast_int_to_float() {
    let output = eval_output("fn main() -> void { println(42 as f64) }");
    assert_eq!(output, vec!["42"]);
}

#[test]
fn e2e_cast_float_to_int() {
    let output = eval_output("fn main() -> void { println(3.7 as i64) }");
    assert_eq!(output, vec!["3"]);
}

#[test]
fn e2e_cast_int_widening() {
    let src = r#"
        fn main() -> void {
            let x: i32 = 100
            let y = x as i64
            println(y + 1)
        }
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["101"]);
}

#[test]
fn e2e_cast_float_narrowing() {
    let src = r#"
        fn main() -> void {
            let x: f64 = 2.5
            let y = x as f32
            println(y)
        }
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["2.5"]);
}

#[test]
fn e2e_cast_bool_to_int() {
    let src = r#"
        fn main() -> void {
            println(true as i64)
            println(false as i64)
        }
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["1", "0"]);
}

#[test]
fn e2e_cast_in_expression() {
    let src = r#"
        fn main() -> void {
            let result = (10 as f64) / 3.0
            println(result)
        }
    "#;
    let output = eval_output(src);
    // 10.0 / 3.0 = 3.3333...
    assert!(output[0].starts_with("3.3333"));
}

#[test]
fn e2e_named_arguments() {
    let src = r#"
        fn greet(name: str, times: i64) -> void {
            let mut i = 0
            while i < times {
                println(name)
                i = i + 1
            }
        }
        fn main() -> void {
            greet(times: 2, name: "hello")
        }
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["hello", "hello"]);
}

// ── Global Builtins & Math Tests ─────────────────────────────────────

#[test]
fn e2e_panic_terminates() {
    let mut interp = Interpreter::new_capturing();
    interp
        .eval_source(r#"fn main() -> void { panic("boom") }"#)
        .expect("eval failed");
    let result = interp.call_main();
    assert!(result.is_err());
    let err = format!("{}", result.unwrap_err());
    assert!(err.contains("boom"), "expected 'boom' in error: {err}");
}

#[test]
fn e2e_dbg_prints_and_returns() {
    let src = r#"
        fn main() -> void {
            let x = dbg(42)
            println(x)
        }
    "#;
    let output = eval_output(src);
    // dbg prints "[dbg] 42", then println prints "42"
    assert_eq!(output.len(), 2);
    assert!(output[0].contains("42"));
    assert_eq!(output[1], "42");
}

#[test]
fn e2e_eprint_and_eprintln() {
    let src = r#"
        fn main() -> void {
            eprintln("error msg")
            println("ok")
        }
    "#;
    let output = eval_output(src);
    // eprintln captured as output in test mode, then println
    assert_eq!(output.len(), 2);
    assert_eq!(output[0], "error msg");
    assert_eq!(output[1], "ok");
}

#[test]
fn e2e_math_abs() {
    let src = r#"
        fn main() -> void {
            println(abs(-5))
            println(abs(3))
            println(abs(-2.5))
        }
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["5", "3", "2.5"]);
}

#[test]
fn e2e_math_sqrt() {
    let src = r#"
        fn main() -> void {
            println(sqrt(4.0))
            println(sqrt(9.0))
        }
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["2", "3"]);
}

#[test]
fn e2e_math_pow() {
    let src = r#"
        fn main() -> void {
            println(pow(2.0, 10.0))
            println(pow(3.0, 2.0))
        }
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["1024", "9"]);
}

#[test]
fn e2e_math_floor_ceil_round() {
    let src = r#"
        fn main() -> void {
            println(floor(3.7))
            println(ceil(3.2))
            println(round(3.5))
        }
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["3", "4", "4"]);
}

#[test]
fn e2e_math_clamp() {
    let src = r#"
        fn main() -> void {
            println(clamp(15.0, 0.0, 10.0))
            println(clamp(-5.0, 0.0, 10.0))
            println(clamp(5.0, 0.0, 10.0))
        }
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["10", "0", "5"]);
}

#[test]
fn e2e_math_log() {
    let src = r#"
        fn main() -> void {
            println(log2(8.0))
            println(log10(1000.0))
        }
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["3", "3"]);
}

#[test]
fn e2e_math_trig() {
    let src = r#"
        fn main() -> void {
            println(sin(0.0))
            println(cos(0.0))
            println(tan(0.0))
        }
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["0", "1", "0"]);
}

#[test]
fn e2e_math_constants() {
    let src = r#"
        fn main() -> void {
            println(PI)
            println(E)
        }
    "#;
    let output = eval_output(src);
    assert!(output[0].starts_with("3.14159"));
    assert!(output[1].starts_with("2.71828"));
}

#[test]
fn e2e_math_min_max() {
    let src = r#"
        fn main() -> void {
            println(min(3, 7))
            println(max(3, 7))
            println(min(2.5, 1.5))
            println(max(2.5, 1.5))
        }
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["3", "7", "1.5", "2.5"]);
}

// ── Loop Expression Tests ────────────────────────────────────────────

#[test]
fn e2e_loop_with_break() {
    let src = r#"
        fn main() -> void {
            let mut i = 0
            loop {
                i = i + 1
                if i == 5 { break }
            }
            println(i)
        }
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["5"]);
}

#[test]
fn e2e_loop_with_continue() {
    let src = r#"
        fn main() -> void {
            let mut i = 0
            let mut sum = 0
            loop {
                i = i + 1
                if i > 10 { break }
                if i % 2 == 0 { continue }
                sum = sum + i
            }
            println(sum)
        }
    "#;
    let output = eval_output(src);
    // 1 + 3 + 5 + 7 + 9 = 25
    assert_eq!(output, vec!["25"]);
}

#[test]
fn e2e_loop_break_value() {
    let src = r#"
        fn find_first_gt(threshold: i64) -> i64 {
            let mut n = 0
            loop {
                n = n + 1
                if n * n > threshold { break }
            }
            n
        }
        fn main() -> void {
            println(find_first_gt(50))
        }
    "#;
    let output = eval_output(src);
    // 8*8=64 > 50
    assert_eq!(output, vec!["8"]);
}

// ═══════════════════════════════════════════════════════════════════════
// Formatter integration tests
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn e2e_formatter_idempotent_hello() {
    let source = std::fs::read_to_string("examples/hello.fj").expect("cannot read hello.fj");
    let formatted = fajar_lang::formatter::format(&source).expect("format failed");
    let second = fajar_lang::formatter::format(&formatted).expect("format second pass failed");
    assert_eq!(formatted, second, "formatter must be idempotent");
}

#[test]
fn e2e_formatter_preserves_comments() {
    let src = r#"// Top-level comment
fn add(a: i32, b: i32) -> i32 {
    // Inside function
    a + b
}
"#;
    let result = fajar_lang::formatter::format(src).expect("format failed");
    assert!(
        result.contains("// Top-level comment"),
        "top comment preserved"
    );
    assert!(
        result.contains("// Inside function"),
        "inner comment preserved"
    );
}

#[test]
fn e2e_formatter_normalizes_spacing() {
    let src = "fn   add(  a:i32 , b:i32 )->i32{   a+b   }";
    let result = fajar_lang::formatter::format(src).expect("format failed");
    assert!(result.contains("fn add(a: i32, b: i32) -> i32"));
    assert!(result.contains("a + b"));
}

#[test]
fn e2e_formatter_formatted_code_still_runs() {
    let src = r#"
        fn double(x: i32) -> i32 { x * 2 }
        fn main() -> void {
            println(double(21))
        }
    "#;
    let formatted = fajar_lang::formatter::format(src).expect("format failed");
    // Run the formatted code
    let mut interp = Interpreter::new_capturing();
    interp
        .eval_source(&formatted)
        .expect("formatted code should run");
    interp.call_main().expect("call_main failed");
    let output = interp.get_output();
    assert_eq!(output, vec!["42"]);
}

#[test]
fn e2e_formatter_check_mode() {
    let already_formatted = "fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n";
    let result = fajar_lang::formatter::format(already_formatted).expect("format failed");
    assert_eq!(
        already_formatted, result,
        "already-formatted code should be unchanged"
    );
}

// ═══════════════════════════════════════════════════════════════════════
// Bytecode VM integration tests
// ═══════════════════════════════════════════════════════════════════════

/// Helper: runs source through the bytecode VM and returns captured output.
fn vm_output(source: &str) -> Vec<String> {
    let tokens = fajar_lang::lexer::tokenize(source).expect("lex failed");
    let program = fajar_lang::parser::parse(tokens).expect("parse failed");
    let (_, output) = fajar_lang::vm::run_program_capturing(&program).expect("vm run failed");
    output
}

#[test]
fn vm_hello_world() {
    let source = std::fs::read_to_string("examples/hello.fj").expect("cannot read hello.fj");
    let tokens = fajar_lang::lexer::tokenize(&source).expect("lex failed");
    let program = fajar_lang::parser::parse(tokens).expect("parse failed");
    let (_, output) = fajar_lang::vm::run_program_capturing(&program).expect("vm run failed");
    assert!(
        output.iter().any(|line| line.contains("Hello")),
        "expected 'Hello' in output, got: {output:?}"
    );
}

#[test]
fn vm_fibonacci() {
    let source =
        std::fs::read_to_string("examples/fibonacci.fj").expect("cannot read fibonacci.fj");
    let tokens = fajar_lang::lexer::tokenize(&source).expect("lex failed");
    let program = fajar_lang::parser::parse(tokens).expect("parse failed");
    let (_, output) = fajar_lang::vm::run_program_capturing(&program).expect("vm run failed");
    assert!(
        output.len() >= 7,
        "expected at least 7 lines, got {}",
        output.len()
    );
    assert_eq!(output[0], "0");
    assert_eq!(output[1], "1");
    assert_eq!(output[2], "1");
    assert_eq!(output[3], "2");
    assert_eq!(output[4], "3");
    assert_eq!(output[5], "5");
    assert_eq!(output[6], "8");
}

#[test]
fn vm_factorial() {
    let source =
        std::fs::read_to_string("examples/factorial.fj").expect("cannot read factorial.fj");
    let tokens = fajar_lang::lexer::tokenize(&source).expect("lex failed");
    let program = fajar_lang::parser::parse(tokens).expect("parse failed");
    let (_, output) = fajar_lang::vm::run_program_capturing(&program).expect("vm run failed");
    assert!(
        output.len() >= 5,
        "expected at least 5 lines, got {}",
        output.len()
    );
    assert_eq!(output[0], "1");
    assert_eq!(output[1], "1");
    assert_eq!(output[2], "2");
    assert_eq!(output[3], "6");
    assert_eq!(output[4], "24");
}

#[test]
fn vm_simple_arithmetic() {
    let output = vm_output("fn main() -> void { println(1 + 2 * 3) }");
    assert_eq!(output, vec!["7"]);
}

#[test]
fn vm_boolean_logic() {
    let output = vm_output("fn main() -> void { println(true && false) println(!false) }");
    assert_eq!(output, vec!["false", "true"]);
}

#[test]
fn vm_string_concat() {
    let output = vm_output(r#"fn main() -> void { println("hello" + " world") }"#);
    assert_eq!(output, vec!["hello world"]);
}

#[test]
fn vm_variable_binding() {
    let src = r#"
        fn main() -> void {
            let x = 10
            let y = 20
            println(x + y)
        }
    "#;
    let output = vm_output(src);
    assert_eq!(output, vec!["30"]);
}

#[test]
fn vm_if_else() {
    let src = r#"
        fn main() -> void {
            let x = 10
            if x > 5 {
                println("big")
            } else {
                println("small")
            }
        }
    "#;
    let output = vm_output(src);
    assert_eq!(output, vec!["big"]);
}

#[test]
fn vm_while_loop() {
    let src = r#"
        fn main() -> void {
            let mut i = 0
            let mut sum = 0
            while i < 5 {
                sum = sum + i
                i = i + 1
            }
            println(sum)
        }
    "#;
    let output = vm_output(src);
    assert_eq!(output, vec!["10"]);
}

#[test]
fn vm_for_range() {
    let src = r#"
        fn main() -> void {
            let mut sum = 0
            for i in 0..5 {
                sum = sum + i
            }
            println(sum)
        }
    "#;
    let output = vm_output(src);
    assert_eq!(output, vec!["10"]);
}

#[test]
fn vm_recursive_function() {
    let src = r#"
        fn fib(n: i64) -> i64 {
            if n <= 1 { n } else { fib(n - 1) + fib(n - 2) }
        }
        fn main() -> void {
            println(fib(10))
        }
    "#;
    let output = vm_output(src);
    assert_eq!(output, vec!["55"]);
}

#[test]
fn vm_builtin_len() {
    let src = r#"
        fn main() -> void {
            let arr = [1, 2, 3, 4, 5]
            println(len(arr))
        }
    "#;
    let output = vm_output(src);
    assert_eq!(output, vec!["5"]);
}

#[test]
fn vm_builtin_type_of() {
    let src = r#"
        fn main() -> void {
            println(type_of(42))
            println(type_of(3.14))
            println(type_of("hi"))
            println(type_of(true))
        }
    "#;
    let output = vm_output(src);
    assert_eq!(output, vec!["i64", "f64", "str", "bool"]);
}

#[test]
fn vm_matches_interpreter_arithmetic() {
    let cases = [
        "fn main() -> void { println(2 ** 10) }",
        "fn main() -> void { println(17 % 5) }",
        "fn main() -> void { println(100 / 3) }",
        "fn main() -> void { println(-42) }",
    ];
    for src in &cases {
        let interp_out = eval_output(src);
        let vm_out = vm_output(src);
        assert_eq!(interp_out, vm_out, "mismatch for: {src}");
    }
}

#[test]
fn vm_matches_interpreter_comparison() {
    let cases = [
        "fn main() -> void { println(3 > 2) println(3 < 2) println(3 == 3) println(3 != 4) }",
        "fn main() -> void { println(1 >= 1) println(1 <= 0) }",
    ];
    for src in &cases {
        let interp_out = eval_output(src);
        let vm_out = vm_output(src);
        assert_eq!(interp_out, vm_out, "mismatch for: {src}");
    }
}

#[test]
fn vm_short_circuit_and() {
    // false && panic("boom") must NOT panic — RHS not evaluated
    let src = r#"
        fn main() -> void {
            let result = false && panic("boom")
            println("ok")
        }
    "#;
    let output = vm_output(src);
    assert_eq!(output, vec!["ok"]);
}

#[test]
fn vm_short_circuit_or() {
    // true || panic("boom") must NOT panic — RHS not evaluated
    let src = r#"
        fn main() -> void {
            let result = true || panic("boom")
            println("ok")
        }
    "#;
    let output = vm_output(src);
    assert_eq!(output, vec!["ok"]);
}

#[test]
fn vm_short_circuit_and_evaluates_rhs_when_true() {
    let src = r#"
        fn main() -> void {
            let result = true && false
            println(result)
            let result2 = true && true
            println(result2)
        }
    "#;
    let output = vm_output(src);
    assert_eq!(output, vec!["false", "true"]);
}

#[test]
fn vm_short_circuit_or_evaluates_rhs_when_false() {
    let src = r#"
        fn main() -> void {
            let result = false || true
            println(result)
            let result2 = false || false
            println(result2)
        }
    "#;
    let output = vm_output(src);
    assert_eq!(output, vec!["true", "false"]);
}

#[test]
fn vm_struct_field_access() {
    let src = r#"
        struct Point { x: i64, y: i64 }
        fn main() -> void {
            let p = Point { x: 10, y: 20 }
            println(p.x)
            println(p.y)
        }
    "#;
    let output = vm_output(src);
    assert_eq!(output, vec!["10", "20"]);
}

#[test]
fn vm_array_index() {
    let src = r#"
        fn main() -> void {
            let arr = [10, 20, 30]
            println(arr[0])
            println(arr[1])
            println(arr[2])
        }
    "#;
    let output = vm_output(src);
    assert_eq!(output, vec!["10", "20", "30"]);
}

#[test]
fn vm_enum_construction() {
    let src = r#"
        enum Color { Red, Green, Blue }
        fn main() -> void {
            let c = Color::Red
            println(type_of(c))
        }
    "#;
    let output = vm_output(src);
    assert_eq!(output, vec!["enum"]);
}

#[test]
fn vm_pipe_operator() {
    let src = r#"
        fn double(x: i64) -> i64 { x * 2 }
        fn add_one(x: i64) -> i64 { x + 1 }
        fn main() -> void {
            let result = 5 |> double |> add_one
            println(result)
        }
    "#;
    let output = vm_output(src);
    assert_eq!(output, vec!["11"]); // (5*2)+1 = 11
}

#[test]
fn vm_closure_no_capture() {
    let src = r#"
        fn main() -> void {
            let f = |y: i64| -> i64 { y + 1 }
            println(f(5))
        }
    "#;
    let output = vm_output(src);
    assert_eq!(output, vec!["6"]);
}

#[test]
fn vm_closure_with_capture() {
    let src = r#"
        fn main() -> void {
            let x = 10
            let f = |y: i64| -> i64 { x + y }
            println(f(5))
        }
    "#;
    let output = vm_output(src);
    assert_eq!(output, vec!["15"]);
}

// ═══════════════════════════════════════════════════════════════════════
// Sprint 6.1 — std::string & std::convert
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn string_trim_start_end() {
    let src = r#"
        fn main() -> void {
            let s = "  hello  "
            println(s.trim())
            println(s.trim_start())
            println(s.trim_end())
        }
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["hello", "hello  ", "  hello"]);
}

#[test]
fn string_to_uppercase_lowercase() {
    let src = r#"
        fn main() -> void {
            let s = "Hello World"
            println(s.to_uppercase())
            println(s.to_lowercase())
        }
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["HELLO WORLD", "hello world"]);
}

#[test]
fn string_contains() {
    let src = r#"
        fn main() -> void {
            let s = "hello world"
            println(s.contains("world"))
            println(s.contains("xyz"))
        }
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["true", "false"]);
}

#[test]
fn string_starts_with_ends_with() {
    let src = r#"
        fn main() -> void {
            let s = "hello world"
            println(s.starts_with("hello"))
            println(s.starts_with("world"))
            println(s.ends_with("world"))
            println(s.ends_with("hello"))
        }
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["true", "false", "true", "false"]);
}

#[test]
fn string_replace() {
    let src = r#"
        fn main() -> void {
            let s = "hello world"
            println(s.replace("world", "fajar"))
            println(s.replace("xyz", "abc"))
        }
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["hello fajar", "hello world"]);
}

#[test]
fn string_split() {
    let src = r#"
        fn main() -> void {
            let s = "a,b,c,d"
            let parts = s.split(",")
            println(len(parts))
            println(parts[0])
            println(parts[1])
            println(parts[2])
            println(parts[3])
        }
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["4", "a", "b", "c", "d"]);
}

#[test]
fn string_repeat() {
    let src = r#"
        fn main() -> void {
            let s = "ab"
            println(s.repeat(3))
            println(s.repeat(0))
        }
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["ababab", ""]);
}

#[test]
fn string_chars() {
    let src = r#"
        fn main() -> void {
            let s = "abc"
            let cs = s.chars()
            println(len(cs))
            println(cs[0])
            println(cs[1])
            println(cs[2])
        }
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["3", "a", "b", "c"]);
}

#[test]
fn string_substring() {
    let src = r#"
        fn main() -> void {
            let s = "hello world"
            println(s.substring(0, 5))
            println(s.substring(6, 11))
        }
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["hello", "world"]);
}

#[test]
fn string_is_empty() {
    let src = r#"
        fn main() -> void {
            let s = ""
            let t = "hello"
            println(s.is_empty())
            println(t.is_empty())
        }
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["true", "false"]);
}

#[test]
fn string_parse_int() {
    let src = r#"
        fn main() -> void {
            let s = "42"
            let result = s.parse_int()
            match result {
                Ok(n) => println(n),
                Err(e) => println(e),
            }
        }
    "#;
    let output = eval_output(src);
    assert_eq!(output[0], "42");
}

#[test]
fn string_parse_int_error() {
    let src = r#"
        fn main() -> void {
            let bad = "abc"
            let result = bad.parse_int()
            match result {
                Ok(n) => println(n),
                Err(e) => println(e),
            }
        }
    "#;
    let output = eval_output(src);
    assert!(output[0].contains("parse error"));
}

#[test]
fn string_parse_float() {
    let src = r#"
        fn main() -> void {
            let s = "3.14"
            let result = s.parse_float()
            match result {
                Ok(n) => println(n),
                Err(e) => println(e),
            }
        }
    "#;
    let output = eval_output(src);
    assert_eq!(output[0], "3.14");
}

#[test]
fn string_len() {
    let src = r#"
        fn main() -> void {
            let s = "hello"
            println(s.len())
            println(len(s))
        }
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["5", "5"]);
}

#[test]
fn convert_int_to_float_cast() {
    let src = r#"
        fn main() -> void {
            let x: i64 = 42
            let y = x as f64
            println(y)
        }
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["42"]);
}

#[test]
fn convert_float_to_int_cast() {
    let src = r#"
        fn main() -> void {
            let x: f64 = 3.7
            let y = x as i64
            println(y)
        }
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["3"]);
}

#[test]
fn convert_bool_to_int_cast() {
    let src = r#"
        fn main() -> void {
            let t = true as i64
            let f = false as i64
            println(t)
            println(f)
        }
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["1", "0"]);
}

#[test]
fn convert_to_string_builtin() {
    let src = r#"
        fn main() -> void {
            println(to_string(42))
            println(to_string(3.14))
            println(to_string(true))
        }
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["42", "3.14", "true"]);
}

#[test]
fn array_join() {
    let src = r#"
        fn main() -> void {
            let parts = ["hello", "world", "fajar"]
            println(parts.join(", "))
            println(parts.join("-"))
        }
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["hello, world, fajar", "hello-world-fajar"]);
}

#[test]
fn array_reverse() {
    let src = r#"
        fn main() -> void {
            let arr = [1, 2, 3]
            let rev = arr.reverse()
            println(rev[0])
            println(rev[1])
            println(rev[2])
        }
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["3", "2", "1"]);
}

#[test]
fn array_contains() {
    let src = r#"
        fn main() -> void {
            let arr = [1, 2, 3, 4, 5]
            println(arr.contains(3))
            println(arr.contains(10))
        }
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["true", "false"]);
}

#[test]
fn array_is_empty() {
    let src = r#"
        fn main() -> void {
            let empty: [i64] = []
            let full = [1, 2]
            println(empty.is_empty())
            println(full.is_empty())
        }
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["true", "false"]);
}

// ═══════════════════════════════════════════════════════════════════════
// Sprint 6.2 — std::collections (HashMap)
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn hashmap_create_insert_get() {
    let src = r#"
        fn main() -> void {
            let mut m = map_new()
            m = map_insert(m, "name", "fajar")
            m = map_insert(m, "lang", "fj")
            let result = map_get(m, "name")
            match result {
                Some(v) => println(v),
                None => println("not found"),
            }
            let missing = map_get(m, "xyz")
            match missing {
                Some(v) => println(v),
                None => println("not found"),
            }
        }
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["fajar", "not found"]);
}

#[test]
fn hashmap_contains_key() {
    let src = r#"
        fn main() -> void {
            let mut m = map_new()
            m = map_insert(m, "x", 42)
            println(map_contains_key(m, "x"))
            println(map_contains_key(m, "y"))
        }
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["true", "false"]);
}

#[test]
fn hashmap_remove() {
    let src = r#"
        fn main() -> void {
            let mut m = map_new()
            m = map_insert(m, "a", 1)
            m = map_insert(m, "b", 2)
            println(map_len(m))
            m = map_remove(m, "a")
            println(map_len(m))
            println(map_contains_key(m, "a"))
        }
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["2", "1", "false"]);
}

#[test]
fn hashmap_keys_values() {
    let src = r#"
        fn main() -> void {
            let mut m = map_new()
            m = map_insert(m, "x", 10)
            let ks = map_keys(m)
            let vs = map_values(m)
            println(len(ks))
            println(len(vs))
            println(ks[0])
            println(vs[0])
        }
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["1", "1", "x", "10"]);
}

#[test]
fn hashmap_method_style() {
    let src = r#"
        fn main() -> void {
            let mut m = map_new()
            m = m.insert("key", "value")
            println(m.len())
            println(m.contains_key("key"))
            println(m.is_empty())
            let result = m.get("key")
            match result {
                Some(v) => println(v),
                None => println("none"),
            }
        }
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["1", "true", "false", "value"]);
}

#[test]
fn hashmap_for_in_iteration() {
    let src = r#"
        fn main() -> void {
            let mut m = map_new()
            m = map_insert(m, "key", "value")
            let mut count = 0
            for entry in m {
                count = count + 1
            }
            println(count)
        }
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["1"]);
}

#[test]
fn hashmap_method_remove_keys_values() {
    let src = r#"
        fn main() -> void {
            let mut m = map_new()
            m = m.insert("a", 1)
            m = m.insert("b", 2)
            let ks = m.keys()
            println(len(ks))
            m = m.remove("a")
            println(m.len())
        }
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["2", "1"]);
}

// ═══════════════════════════════════════════════════════════════════════
// Sprint 6.3 — std::io & File I/O
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn file_write_and_read() {
    let path = "/tmp/fajar_test_rw.txt";
    let src = format!(
        r#"
        fn main() -> void {{
            let res = write_file("{path}", "hello fajar")
            match res {{
                Ok(_) => println("written"),
                Err(e) => println(e),
            }}
            let content = read_file("{path}")
            match content {{
                Ok(s) => println(s),
                Err(e) => println(e),
            }}
        }}
    "#
    );
    let output = eval_output(&src);
    assert_eq!(output, vec!["written", "hello fajar"]);
    let _ = std::fs::remove_file(path);
}

#[test]
fn file_append() {
    let path = "/tmp/fajar_test_append.txt";
    let _ = std::fs::remove_file(path);
    let src = format!(
        r#"
        fn main() -> void {{
            write_file("{path}", "hello")
            append_file("{path}", " world")
            let content = read_file("{path}")
            match content {{
                Ok(s) => println(s),
                Err(e) => println(e),
            }}
        }}
    "#
    );
    let output = eval_output(&src);
    assert_eq!(output, vec!["hello world"]);
    let _ = std::fs::remove_file(path);
}

#[test]
fn file_exists_check() {
    let path = "/tmp/fajar_test_exists.txt";
    let _ = std::fs::remove_file(path);
    let src = format!(
        r#"
        fn main() -> void {{
            println(file_exists("{path}"))
            write_file("{path}", "test")
            println(file_exists("{path}"))
        }}
    "#
    );
    let output = eval_output(&src);
    assert_eq!(output, vec!["false", "true"]);
    let _ = std::fs::remove_file(path);
}

// ═══════════════════════════════════════════════════════════════════════
// Sprint 6.4 — OS & NN Stdlib Completion
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn metric_accuracy() {
    let src = r#"
        fn main() -> void {
            let preds = [0, 1, 2, 0, 1]
            let labels = [0, 1, 2, 0, 1]
            println(metric_accuracy(preds, labels))
        }
    "#;
    let output = eval_output(src);
    assert_eq!(output[0], "1");
}

#[test]
fn metric_accuracy_partial() {
    let src = r#"
        fn main() -> void {
            let preds = [0, 1, 0, 1]
            let labels = [0, 0, 0, 0]
            println(metric_accuracy(preds, labels))
        }
    "#;
    let output = eval_output(src);
    assert_eq!(output[0], "0.5");
}

#[test]
fn metric_precision_recall_f1() {
    let src = r#"
        fn main() -> void {
            let p1 = [1, 1, 0, 0]
            let l1 = [1, 0, 0, 1]
            println(metric_precision(p1, l1, 1))
            let p2 = [1, 1, 0, 0]
            let l2 = [1, 0, 0, 1]
            println(metric_recall(p2, l2, 1))
            let p3 = [1, 1, 0, 0]
            let l3 = [1, 0, 0, 1]
            println(metric_f1_score(p3, l3, 1))
        }
    "#;
    let output = eval_output(src);
    assert_eq!(output[0], "0.5");
    assert_eq!(output[1], "0.5");
    assert_eq!(output[2], "0.5");
}

// ═══════════════════════════════════════════════════════════════════════
// Sprint 7.3 — Security & Context Isolation Tests
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn security_kernel_cannot_use_heap() {
    // KE001: @kernel should reject heap operations like push
    let src = r#"
        @kernel fn bad() -> void {
            let arr = [1, 2, 3]
            push(arr, 4)
        }
    "#;
    let tokens = tokenize(src).unwrap();
    let program = parse(tokens).unwrap();
    let result = analyze(&program);
    assert!(result.is_err(), "heap alloc in kernel should be rejected");
}

#[test]
fn security_device_cannot_use_raw_pointer() {
    // DE001: @device should reject raw pointer operations
    let src = r#"
        @device fn bad() -> void {
            let p = mem_alloc(4096, 8)
        }
    "#;
    let tokens = tokenize(src).unwrap();
    let program = parse(tokens).unwrap();
    let result = analyze(&program);
    assert!(result.is_err(), "raw pointer in device should be rejected");
}

#[test]
fn security_immutable_reassignment() {
    // SE007: Cannot reassign immutable variable
    let src = r#"
        fn main() -> void {
            let x = 42
            x = 10
        }
    "#;
    let tokens = tokenize(src).unwrap();
    let program = parse(tokens).unwrap();
    let result = analyze(&program);
    assert!(result.is_err(), "immutable reassignment should be rejected");
}

#[test]
fn security_division_by_zero() {
    let src = r#"
        fn main() -> void {
            let x = 42 / 0
        }
    "#;
    let mut interp = Interpreter::new_capturing();
    interp.eval_source(src).unwrap();
    let result = interp.call_main();
    assert!(
        result.is_err(),
        "division by zero should produce runtime error"
    );
}

#[test]
fn security_stack_overflow() {
    let src = r#"
        fn infinite() -> i64 {
            infinite()
        }
        fn main() -> void {
            println(infinite())
        }
    "#;
    let mut interp = Interpreter::new_capturing();
    interp.eval_source(src).unwrap();
    let result = interp.call_main();
    assert!(
        result.is_err(),
        "infinite recursion should produce stack overflow"
    );
}

#[test]
fn security_array_out_of_bounds() {
    let src = r#"
        fn main() -> void {
            let arr = [1, 2, 3]
            println(arr[10])
        }
    "#;
    let mut interp = Interpreter::new_capturing();
    interp.eval_source(src).unwrap();
    let result = interp.call_main();
    assert!(
        result.is_err(),
        "array out of bounds should produce runtime error"
    );
}

#[test]
fn file_read_nonexistent() {
    let src = r#"
        fn main() -> void {
            let result = read_file("/tmp/fajar_nonexistent_file_xyz.txt")
            match result {
                Ok(s) => println(s),
                Err(e) => println("error"),
            }
        }
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["error"]);
}

// ═══════════════════════════════════════════════════════════════════════
// Sprint S1.1 — Analyzer Integration in eval_source()
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn s1_1_eval_source_runs_analyzer() {
    // eval_source should catch semantic errors before execution
    let mut interp = Interpreter::new_capturing();
    let result = interp.eval_source(
        r#"
        fn main() -> void {
            let x = 42
            x = 10
        }
    "#,
    );
    assert!(
        result.is_err(),
        "immutable reassignment should be caught by analyzer"
    );
    match result {
        Err(fajar_lang::FjError::Semantic(errors)) => {
            assert!(!errors.is_empty());
        }
        other => panic!("expected Semantic error, got: {other:?}"),
    }
}

#[test]
fn s1_1_valid_program_passes_analyzer_and_runs() {
    // A valid program should pass analyzer and execute correctly
    let src = r#"
        fn add(a: i64, b: i64) -> i64 { a + b }
        fn main() -> void {
            println(add(1, 2))
        }
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["3"]);
}

#[test]
fn s1_1_context_violation_caught_by_eval_source() {
    // @kernel context violation should be caught before execution
    let mut interp = Interpreter::new_capturing();
    let result = interp.eval_source(
        r#"
        @kernel fn bad() -> void {
            let s = to_string(42)
        }
    "#,
    );
    assert!(result.is_err(), "@kernel heap alloc should be caught");
}

#[test]
fn s1_1_repl_style_multi_call_works() {
    // Variables defined in first call should be visible in second call
    let mut interp = Interpreter::new_capturing();
    interp.eval_source("let x = 42").expect("first eval failed");
    let val = interp.eval_source("x").expect("second eval failed");
    assert_eq!(val, Value::Int(42));
}

#[test]
fn s1_1_warnings_do_not_block_execution() {
    // Warnings (like unused variables) should not prevent execution
    let src = r#"
        fn main() -> void {
            let _unused = 42
            println("hello")
        }
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["hello"]);
}

// ═══════════════════════════════════════════════════════════════════════
// S1.4 — File-based modules
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn s1_4_file_based_module_compiles_and_runs() {
    // Create a temporary directory with a module file
    let dir = std::env::temp_dir().join("fj_test_s1_4_basic");
    let _ = std::fs::create_dir_all(&dir);

    // Write the module file
    std::fs::write(
        dir.join("mathlib.fj"),
        "fn double(x: i64) -> i64 { x * 2 }\nfn triple(x: i64) -> i64 { x * 3 }\n",
    )
    .unwrap();

    // Main program that uses mod mathlib;
    let main_src = r#"
        mod mathlib;
        use mathlib::double

        fn main() -> void {
            println(double(5))
        }
    "#;

    let tokens = tokenize(main_src).unwrap();
    let program = parse(tokens).unwrap();
    let mut interp = Interpreter::new_capturing();
    interp.set_source_dir(dir.clone());
    interp.eval_program(&program).unwrap();
    interp.call_main().unwrap();
    assert_eq!(interp.get_output(), vec!["10"]);

    // Cleanup
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn s1_4_file_module_not_found_error() {
    let dir = std::env::temp_dir().join("fj_test_s1_4_not_found");
    let _ = std::fs::create_dir_all(&dir);

    let main_src = "mod nonexistent;";
    let tokens = tokenize(main_src).unwrap();
    let program = parse(tokens).unwrap();
    let mut interp = Interpreter::new_capturing();
    interp.set_source_dir(dir.clone());
    let result = interp.eval_program(&program);
    assert!(result.is_err(), "should error on missing module file");
    let err_msg = format!("{:?}", result.unwrap_err());
    assert!(
        err_msg.contains("PE011") || err_msg.contains("module file not found"),
        "expected PE011 error, got: {err_msg}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn s1_4_circular_module_dependency_detected() {
    let dir = std::env::temp_dir().join("fj_test_s1_4_circular");
    let _ = std::fs::create_dir_all(&dir);

    // a.fj imports b, b.fj imports a → circular
    std::fs::write(dir.join("a.fj"), "mod b;\n").unwrap();
    std::fs::write(dir.join("b.fj"), "mod a;\n").unwrap();

    let main_src = "mod a;";
    let tokens = tokenize(main_src).unwrap();
    let program = parse(tokens).unwrap();
    let mut interp = Interpreter::new_capturing();
    interp.set_source_dir(dir.clone());
    let result = interp.eval_program(&program);
    assert!(result.is_err(), "should detect circular dependency");
    let err_msg = format!("{:?}", result.unwrap_err());
    assert!(
        err_msg.contains("circular"),
        "expected circular dependency error, got: {err_msg}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn s1_4_file_module_glob_import() {
    let dir = std::env::temp_dir().join("fj_test_s1_4_glob");
    let _ = std::fs::create_dir_all(&dir);

    std::fs::write(
        dir.join("utils.fj"),
        "fn add_one(x: i64) -> i64 { x + 1 }\nfn square(x: i64) -> i64 { x * x }\n",
    )
    .unwrap();

    let main_src = r#"
        mod utils;
        use utils::*

        fn main() -> void {
            println(add_one(4))
            println(square(3))
        }
    "#;

    let tokens = tokenize(main_src).unwrap();
    let program = parse(tokens).unwrap();
    let mut interp = Interpreter::new_capturing();
    interp.set_source_dir(dir.clone());
    interp.eval_program(&program).unwrap();
    interp.call_main().unwrap();
    assert_eq!(interp.get_output(), vec!["5", "9"]);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn s1_4_file_module_qualified_access() {
    let dir = std::env::temp_dir().join("fj_test_s1_4_qualified");
    let _ = std::fs::create_dir_all(&dir);

    std::fs::write(
        dir.join("mymod.fj"),
        "fn greet() -> str { \"hello from mymod\" }\n",
    )
    .unwrap();

    let main_src = r#"
        mod mymod;

        fn main() -> void {
            println(mymod::greet())
        }
    "#;

    let tokens = tokenize(main_src).unwrap();
    let program = parse(tokens).unwrap();
    let mut interp = Interpreter::new_capturing();
    interp.set_source_dir(dir.clone());
    interp.eval_program(&program).unwrap();
    interp.call_main().unwrap();
    assert_eq!(interp.get_output(), vec!["hello from mymod"]);

    let _ = std::fs::remove_dir_all(&dir);
}

// ═══════════════════════════════════════════════════════════════════════
// S5.2/S5.3 — Generic functions (monomorphization in interpreter)
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn s5_generic_identity_int() {
    let src = r#"
        fn identity<T>(x: T) -> T { x }
        fn main() -> void { println(identity(42)) }
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["42"]);
}

#[test]
fn s5_generic_identity_float() {
    let src = r#"
        fn identity<T>(x: T) -> T { x }
        fn main() -> void { println(identity(3.14)) }
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["3.14"]);
}

#[test]
fn s5_generic_max_int() {
    let src = r#"
        fn max<T>(a: T, b: T) -> T {
            if a > b { a } else { b }
        }
        fn main() -> void { println(max(3, 7)) }
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["7"]);
}

#[test]
fn s5_generic_max_float() {
    let src = r#"
        fn max<T>(a: T, b: T) -> T {
            if a > b { a } else { b }
        }
        fn main() -> void { println(max(1.5, 2.5)) }
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["2.5"]);
}

#[test]
fn s5_generic_multiple_type_params() {
    let src = r#"
        fn first<T, U>(a: T, b: U) -> T { a }
        fn main() -> void { println(first(42, 3.14)) }
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["42"]);
}

#[test]
fn s5_generic_nested_call() {
    let src = r#"
        fn identity<T>(x: T) -> T { x }
        fn double<T>(x: T) -> T { x + x }
        fn main() -> void { println(identity(double(21))) }
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["42"]);
}

#[test]
fn s5_generic_with_bound_syntax() {
    // Bounds are parsed but not enforced yet — just verify it runs
    let src = r#"
        fn max<T: Ord>(a: T, b: T) -> T {
            if a > b { a } else { b }
        }
        fn main() -> void { println(max(10, 20)) }
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["20"]);
}

#[test]
fn s5_generic_with_where_clause() {
    // where clause is parsed and runs — equivalent to inline bounds
    let src = r#"
        fn max<T>(a: T, b: T) -> T where T: Ord {
            if a > b { a } else { b }
        }
        fn main() -> void { println(max(10, 20)) }
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["20"]);
}

#[test]
fn s5_generic_with_where_clause_multiple() {
    let src = r#"
        fn first<T, U>(a: T, b: U) -> T where T: Ord, U: Display {
            a
        }
        fn main() -> void { println(first(42, 99)) }
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["42"]);
}

// ── Sprint 8 — Type System Polish ─────────────────────────────────────

#[test]
fn s8_generic_enum_option_some_match() {
    let src = r#"
        enum Option<T> { Some(T), None }
        fn main() -> void {
            let x = Some(42)
            match x {
                Some(val) => println(val),
                None => println(0)
            }
        }
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["42"]);
}

#[test]
fn s8_generic_enum_option_none_match() {
    let src = r#"
        enum Option<T> { Some(T), None }
        fn main() -> void {
            let x = None
            match x {
                Some(val) => println(val),
                None => println(-1)
            }
        }
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["-1"]);
}

#[test]
fn s8_generic_enum_result_ok() {
    let src = r#"
        enum Result<T, E> { Ok(T), Err(E) }
        fn main() -> void {
            let r = Ok(100)
            match r {
                Ok(v) => println(v),
                Err(e) => println(e)
            }
        }
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["100"]);
}

#[test]
fn s8_generic_enum_result_err() {
    let src = r#"
        enum Result<T, E> { Ok(T), Err(E) }
        fn main() -> void {
            let r = Err(-1)
            match r {
                Ok(v) => println(v),
                Err(e) => println(e)
            }
        }
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["-1"]);
}

#[test]
fn s8_extern_fn_registered_as_builtin() {
    // Extern fn declarations should be accessible in the interpreter
    let src = r#"
        extern fn abs(x: i32) -> i32
        fn main() -> void {
            println(42)
        }
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["42"]);
}

// ── S9.3 Drop insertion ──

#[test]
fn s9_3_block_scope_drops_locals() {
    // Variables inside a block should be dropped at scope exit
    // This test verifies the block scope works correctly with drop_locals
    let src = r#"
        fn main() -> void {
            let x = 10
            {
                let y = 20
                println(y)
            }
            // y is no longer accessible here (scoped out + dropped)
            println(x)
        }
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["20", "10"]);
}

#[test]
fn s9_3_nested_block_drops() {
    // Nested blocks should each drop their locals independently
    let src = r#"
        fn main() -> void {
            let a = 1
            {
                let b = 2
                {
                    let c = 3
                    println(c)
                }
                // c dropped here
                println(b)
            }
            // b dropped here
            println(a)
        }
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["3", "2", "1"]);
}

#[test]
fn s9_3_loop_scope_drops_each_iteration() {
    // Loop body variables are created/dropped each iteration
    let src = r#"
        fn main() -> void {
            let mut sum = 0
            for i in 0..3 {
                let temp = i * 10
                sum = sum + temp
            }
            println(sum)
        }
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["30"]); // 0 + 10 + 20
}

#[test]
fn s5_generic_fn_with_struct_type() {
    let src = r#"
        struct Point { x: i64, y: i64 }
        fn identity<T>(val: T) -> T { val }
        fn main() -> void {
            let p = Point { x: 10, y: 20 }
            let q = identity(p)
            println(q.x)
            println(q.y)
        }
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["10", "20"]);
}

#[test]
fn s8_3_generic_type_alias_array() {
    let src = r#"
        type IntList = Array<i64>
        let xs: IntList = [10, 20, 30]
        println(len(xs))
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["3"]);
}

#[test]
fn s8_3_generic_type_alias_nested() {
    // Alias with generic resolves correctly through type checker
    let src = r#"
        type Floats = Array<f64>
        let f: Floats = [1.5, 2.5]
        println(len(f))
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["2"]);
}

// --- Format builtin ---

#[test]
fn format_builtin_basic() {
    let src = r#"
        let name = "Alice"
        let age = 30
        let msg = format("Hello {}, age {}", name, age)
        println(msg)
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["Hello Alice, age 30"]);
}

#[test]
fn format_builtin_no_placeholders() {
    let src = r#"
        let msg = format("no placeholders")
        println(msg)
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["no placeholders"]);
}

// --- String methods: index_of, rev, bytes ---

#[test]
fn string_index_of_found() {
    let src = r#"
        let s = "hello world"
        let idx = s.index_of("world")
        match idx {
            Some(i) => println(i),
            None => println("not found"),
        }
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["6"]);
}

#[test]
fn string_index_of_not_found() {
    let src = r#"
        let s = "hello"
        let idx = s.index_of("xyz")
        match idx {
            Some(i) => println(i),
            None => println("not found"),
        }
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["not found"]);
}

#[test]
fn string_rev() {
    let src = r#"
        let s = "abcde"
        println(s.rev())
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["edcba"]);
}

#[test]
fn string_bytes() {
    let src = r#"
        let s = "AB"
        let bs = s.bytes()
        println(len(bs))
        println(bs[0])
        println(bs[1])
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["2", "65", "66"]);
}

// --- Array methods: first, last ---

#[test]
fn array_first_some() {
    let src = r#"
        let xs = [10, 20, 30]
        match xs.first() {
            Some(v) => println(v),
            None => println("empty"),
        }
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["10"]);
}

#[test]
fn array_first_empty() {
    let src = r#"
        let xs: Array<i64> = []
        match xs.first() {
            Some(v) => println(v),
            None => println("empty"),
        }
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["empty"]);
}

#[test]
fn array_last_some() {
    let src = r#"
        let xs = [10, 20, 30]
        match xs.last() {
            Some(v) => println(v),
            None => println("empty"),
        }
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["30"]);
}

#[test]
fn s5_generic_mixed_int_float_type_error() {
    // max(1, 2.0) should fail: T binds to IntLiteral from first arg,
    // then FloatLiteral from second arg conflicts
    let src = r#"
        fn max<T>(a: T, b: T) -> T { if a > b { a } else { b } }
        fn main() -> void { println(max(1, 2.0)) }
    "#;
    let mut interp = fajar_lang::interpreter::Interpreter::new();
    let result = interp.eval_source(src);
    assert!(
        matches!(result, Err(fajar_lang::FjError::Semantic(_))),
        "max(1, 2.0) should be a type error, got: {result:?}"
    );
}

#[test]
fn s5_generic_inference_identity_i64() {
    // identity<T> called with i64 → T = i64, returns i64
    let src = r#"
        fn identity<T>(x: T) -> T { x }
        fn main() -> void { println(identity(42)) }
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["42"]);
}

#[test]
fn s5_generic_inference_identity_str() {
    // identity<T> called with str → T = str
    let src = r#"
        fn identity<T>(x: T) -> T { x }
        fn main() -> void { println(identity("hello")) }
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["hello"]);
}

#[test]
fn s5_generic_inference_two_params() {
    // fn pair<T, U>(a: T, b: U) → infers both T and U
    let src = r#"
        fn first<T, U>(a: T, b: U) -> T { a }
        fn main() -> void { println(first(42, "hello")) }
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["42"]);
}

#[test]
fn s8_bidirectional_int_literal_to_f64() {
    // Bidirectional: let x: f64 = 1 should work (IntLiteral coerces to f64)
    let src = r#"
        let x: f64 = 1
        let y: f32 = 2
        println(x)
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["1"]);
}

#[test]
fn s8_bidirectional_float_literal_to_f32() {
    // FloatLiteral coerces to f32 when annotated
    let src = r#"
        let x: f32 = 3.14
        println(x)
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["3.14"]);
}

#[test]
fn s8_closure_param_inference_from_usage() {
    // Closure without type annotation — params inferred at runtime
    let src = r#"
        let double = |x| x * 2
        println(double(21))
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["42"]);
}

// ── S20 — HAL trait parsing ──────────────────────────────────────────

#[test]
fn s20_trait_method_without_body() {
    // Trait methods with signatures only (no body) should parse and type-check
    let src = r#"
        trait Sensor {
            fn read_data(&mut self) -> i64
            fn ready(&self) -> bool
        }
        println("trait parsed")
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["trait parsed"]);
}

#[test]
fn s20_trait_mixed_methods() {
    // Trait with both abstract (no body) and default (with body) methods
    let src = r#"
        trait Timer {
            fn start(&mut self) -> void
            fn elapsed(&self) -> bool
            fn default_timeout(&self) -> i64 { 1000 }
        }
        println("mixed trait ok")
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["mixed trait ok"]);
}

#[test]
fn s20_contextual_keyword_as_param() {
    // Domain keywords like `addr` usable as parameter names
    let src = r#"
        fn send(addr: i64, data: i64) -> i64 {
            addr + data
        }
        println(send(10, 5))
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["15"]);
}

#[test]
fn s20_hal_trait_with_contextual_keywords() {
    // HAL-style trait with `addr` parameter and body-less methods
    let src = r#"
        trait I2c {
            fn write(addr: i64, data: i64) -> void
            fn read(addr: i64) -> i64
        }
        println("i2c trait ok")
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["i2c trait ok"]);
}

// ── S1.4 — pub visibility ────────────────────────────────────────────

#[test]
fn s1_4_pub_items_importable() {
    // pub functions can be imported via use
    let src = r#"
        mod math {
            pub fn square(x: i64) -> i64 { x * x }
            fn helper() -> i64 { 42 }
        }
        use math::square
        println(square(5))
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["25"]);
}

#[test]
fn s1_4_private_item_blocked() {
    // non-pub functions cannot be imported via use
    let src = r#"
        mod math {
            pub fn square(x: i64) -> i64 { x * x }
            fn helper() -> i64 { 42 }
        }
        use math::helper
    "#;
    let mut interp = Interpreter::new_capturing();
    let result = interp.eval_source(src);
    assert!(result.is_err(), "importing private item should fail");
    let err_str = format!("{:?}", result.unwrap_err());
    assert!(
        err_str.contains("private"),
        "error should mention 'private': {err_str}"
    );
}

#[test]
fn s1_4_glob_only_imports_pub() {
    // use math::* only imports pub items
    let src = r#"
        mod math {
            pub fn square(x: i64) -> i64 { x * x }
            fn internal() -> i64 { 99 }
        }
        use math::*
        println(square(3))
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["9"]);
}

#[test]
fn s1_4_legacy_no_pub_all_visible() {
    // modules with NO pub items → all items accessible (backward compat)
    let src = r#"
        mod math {
            fn square(x: i64) -> i64 { x * x }
            fn cube(x: i64) -> i64 { x * x * x }
        }
        use math::square
        use math::cube
        println(square(4))
        println(cube(3))
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["16", "27"]);
}

// ── S44: Self-Hosted Lexer Tests ──

#[test]
fn s44_self_hosted_lexer_runs_all_tests() {
    let source = std::fs::read_to_string("examples/self_lexer_test.fj")
        .expect("cannot read self_lexer_test.fj");
    let output = eval_output(&source);
    assert!(
        output.last().map(|s| s.as_str()) == Some("All 10 self-hosted lexer tests passed!"),
        "expected all tests to pass, got: {output:?}"
    );
}

#[test]
fn s44_self_hosted_lexer_keywords_match_rust_lexer() {
    // Compare Rust lexer and self-hosted lexer on keyword tokenization
    use fajar_lang::lexer::token::TokenKind;
    let source = "if else while for fn let mut return struct enum";
    let rust_tokens = tokenize(source).unwrap();
    let rust_kinds: Vec<&str> = rust_tokens
        .iter()
        .filter(|t| t.kind != TokenKind::Eof)
        .map(|t| match &t.kind {
            TokenKind::If => "If",
            TokenKind::Else => "Else",
            TokenKind::While => "While",
            TokenKind::For => "For",
            TokenKind::Fn => "Fn",
            TokenKind::Let => "Let",
            TokenKind::Mut => "Mut",
            TokenKind::Return => "Return",
            TokenKind::Struct => "Struct",
            TokenKind::Enum => "Enum",
            _ => "Other",
        })
        .collect();
    assert_eq!(
        rust_kinds,
        vec!["If", "Else", "While", "For", "Fn", "Let", "Mut", "Return", "Struct", "Enum"]
    );
}

#[test]
fn s44_self_hosted_lexer_operators_match_rust_lexer() {
    // Verify operator token count matches between Rust and self-hosted lexer
    let source = "a == b && c != d || e <= f";
    let rust_tokens = tokenize(source).unwrap();
    let rust_count = rust_tokens
        .iter()
        .filter(|t| t.kind != fajar_lang::lexer::token::TokenKind::Eof)
        .count();
    // Self-hosted lexer: a(Ident) ==(EqEq) b(Ident) &&(AmpAmp) c(Ident) !=(BangEq) d(Ident) ||(PipePipe) e(Ident) <=(LtEq) f(Ident)
    assert_eq!(rust_count, 11);
}

#[test]
fn s44_self_hosted_lexer_comments_skipped() {
    // Verify both lexers skip comments identically
    let source = "// comment\nlet x = 1 /* block */ + 2";
    let rust_tokens = tokenize(source).unwrap();
    let rust_count = rust_tokens
        .iter()
        .filter(|t| t.kind != fajar_lang::lexer::token::TokenKind::Eof)
        .count();
    // let x = 1 + 2 → 6 tokens
    assert_eq!(rust_count, 6);
}

#[test]
fn s44_self_hosted_lexer_string_literal() {
    let source = r#"let s = "hello world""#;
    let rust_tokens = tokenize(source).unwrap();
    assert!(matches!(
        rust_tokens[3].kind,
        fajar_lang::lexer::token::TokenKind::StringLit(_)
    ));
}

#[test]
fn s44_self_hosted_lexer_float_literal() {
    let source = "3.14";
    let rust_tokens = tokenize(source).unwrap();
    assert!(matches!(
        rust_tokens[0].kind,
        fajar_lang::lexer::token::TokenKind::FloatLit(_)
    ));
}

#[test]
fn s44_self_hosted_lexer_pipeline_and_fat_arrow() {
    let source = "x |> f => y";
    let rust_tokens = tokenize(source).unwrap();
    assert_eq!(
        rust_tokens[1].kind,
        fajar_lang::lexer::token::TokenKind::PipeGt
    );
    assert_eq!(
        rust_tokens[3].kind,
        fajar_lang::lexer::token::TokenKind::FatArrow
    );
}

#[test]
fn s44_self_hosted_lexer_fn_definition_token_count() {
    let source = "fn add(a: i32, b: i32) -> i32 { a + b }";
    let rust_tokens = tokenize(source).unwrap();
    let rust_count = rust_tokens
        .iter()
        .filter(|t| t.kind != fajar_lang::lexer::token::TokenKind::Eof)
        .count();
    // fn add ( a : i32 , b : i32 ) -> i32 { a + b } = 18
    assert_eq!(rust_count, 18);
}

#[test]
fn s44_string_copy_semantics_no_move_error() {
    // Verify strings are Copy (no ME001 when reusing a string variable)
    let src = r#"
        fn check(s: str) -> str { s }
        fn main() -> void {
            let x = "hello"
            let a = check(x)
            let b = check(x)
            println(a)
            println(b)
        }
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["hello", "hello"]);
}

#[test]
fn s44_string_copy_in_loop() {
    // Verify strings can be used in loops without move errors
    let src = r#"
        fn main() -> void {
            let greeting = "hi"
            let mut i = 0
            while i < 3 {
                println(greeting)
                i = i + 1
            }
        }
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["hi", "hi", "hi"]);
}

// ═══════════════════════════════════════════════════════════════════════
// v0.5 S1: Test Framework (@test, @should_panic, @ignore)
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn s1_test_annotation_parsed() {
    let src = r#"
        @test
        fn test_one() {
            assert_eq(1 + 1, 2)
        }
        fn main() {}
    "#;
    let tokens = tokenize(src).unwrap();
    let program = parse(tokens).unwrap();
    // Find the test function
    let test_fn = program.items.iter().find_map(|item| {
        if let fajar_lang::parser::ast::Item::FnDef(fndef) = item {
            if fndef.name == "test_one" {
                return Some(fndef);
            }
        }
        None
    });
    assert!(test_fn.is_some(), "should find @test fn");
    let fndef = test_fn.unwrap();
    assert!(fndef.is_test, "is_test should be true");
    assert!(!fndef.should_panic, "should_panic should be false");
    assert!(!fndef.is_ignored, "is_ignored should be false");
}

#[test]
fn s1_should_panic_annotation_parsed() {
    let src = r#"
        @test @should_panic
        fn test_panic() {
            let x = 1 / 0
        }
        fn main() {}
    "#;
    let tokens = tokenize(src).unwrap();
    let program = parse(tokens).unwrap();
    let fndef = program
        .items
        .iter()
        .find_map(|item| {
            if let fajar_lang::parser::ast::Item::FnDef(f) = item {
                if f.name == "test_panic" {
                    return Some(f);
                }
            }
            None
        })
        .unwrap();
    assert!(fndef.is_test);
    assert!(fndef.should_panic);
}

#[test]
fn s1_ignore_annotation_parsed() {
    let src = r#"
        @test @ignore
        fn test_slow() {
            assert_eq(1, 1)
        }
        fn main() {}
    "#;
    let tokens = tokenize(src).unwrap();
    let program = parse(tokens).unwrap();
    let fndef = program
        .items
        .iter()
        .find_map(|item| {
            if let fajar_lang::parser::ast::Item::FnDef(f) = item {
                if f.name == "test_slow" {
                    return Some(f);
                }
            }
            None
        })
        .unwrap();
    assert!(fndef.is_test);
    assert!(fndef.is_ignored);
    assert!(!fndef.should_panic);
}

#[test]
fn s1_test_fn_callable() {
    let src = r#"
        @test
        fn test_add() {
            assert_eq(2 + 3, 5)
        }
        fn main() {}
    "#;
    let mut interp = Interpreter::new();
    interp.eval_source(src).unwrap();
    // Test function should be callable
    let result = interp.call_fn("test_add", vec![]);
    assert!(result.is_ok(), "test fn should succeed");
}

#[test]
fn s1_test_fn_detects_failure() {
    let src = r#"
        @test
        fn test_bad() {
            assert_eq(1, 2)
        }
        fn main() {}
    "#;
    let mut interp = Interpreter::new();
    interp.eval_source(src).unwrap();
    let result = interp.call_fn("test_bad", vec![]);
    assert!(result.is_err(), "failing assert_eq should error");
}

#[test]
fn s1_multiple_annotations_order() {
    // @should_panic @test @ignore — all three in any order
    let src = r#"
        @should_panic @test @ignore
        fn test_combo() {
            let x = 1 / 0
        }
        fn main() {}
    "#;
    let tokens = tokenize(src).unwrap();
    let program = parse(tokens).unwrap();
    let fndef = program
        .items
        .iter()
        .find_map(|item| {
            if let fajar_lang::parser::ast::Item::FnDef(f) = item {
                if f.name == "test_combo" {
                    return Some(f);
                }
            }
            None
        })
        .unwrap();
    assert!(fndef.is_test);
    assert!(fndef.should_panic);
    assert!(fndef.is_ignored);
}

#[test]
fn s1_non_test_fn_has_default_flags() {
    let src = r#"
        fn regular() {
            let x = 1
        }
        fn main() {}
    "#;
    let tokens = tokenize(src).unwrap();
    let program = parse(tokens).unwrap();
    let fndef = program
        .items
        .iter()
        .find_map(|item| {
            if let fajar_lang::parser::ast::Item::FnDef(f) = item {
                if f.name == "regular" {
                    return Some(f);
                }
            }
            None
        })
        .unwrap();
    assert!(!fndef.is_test);
    assert!(!fndef.should_panic);
    assert!(!fndef.is_ignored);
}

#[test]
fn s1_test_with_kernel_annotation() {
    // @test with @kernel should both parse
    let src = r#"
        @test @kernel
        fn test_kernel_fn() {
            let x = 42
        }
        fn main() {}
    "#;
    let tokens = tokenize(src).unwrap();
    let program = parse(tokens).unwrap();
    let fndef = program
        .items
        .iter()
        .find_map(|item| {
            if let fajar_lang::parser::ast::Item::FnDef(f) = item {
                if f.name == "test_kernel_fn" {
                    return Some(f);
                }
            }
            None
        })
        .unwrap();
    assert!(fndef.is_test);
    assert!(fndef.annotation.is_some());
    assert_eq!(fndef.annotation.as_ref().unwrap().name, "kernel");
}

#[test]
fn s1_test_discovery_count() {
    let src = r#"
        fn helper() -> i64 { 42 }
        @test fn test_a() { assert_eq(helper(), 42) }
        @test fn test_b() { assert_eq(1, 1) }
        @test @ignore fn test_c() { assert_eq(2, 2) }
        fn not_a_test() { let x = 1 }
        fn main() {}
    "#;
    let tokens = tokenize(src).unwrap();
    let program = parse(tokens).unwrap();
    let test_count = program
        .items
        .iter()
        .filter(|item| matches!(item, fajar_lang::parser::ast::Item::FnDef(f) if f.is_test))
        .count();
    assert_eq!(test_count, 3, "should discover 3 @test functions");
}

#[test]
fn s1_lexer_tokenizes_test_annotations() {
    use fajar_lang::lexer::token::TokenKind;
    let src = "@test @should_panic @ignore fn foo() { 1 }";
    let tokens = tokenize(src).unwrap();
    assert_eq!(tokens[0].kind, TokenKind::AtTest);
    assert_eq!(tokens[1].kind, TokenKind::AtShouldPanic);
    assert_eq!(tokens[2].kind, TokenKind::AtIgnore);
    assert_eq!(tokens[3].kind, TokenKind::Fn);
}

// ═══════════════════════════════════════════════════════════════════════
// Sprint 2: Doc Comments & Generation
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn s2_lexer_emits_doc_comment_token() {
    use fajar_lang::lexer::token::TokenKind;
    let src = "/// This is a doc comment\nfn foo() { 1 }";
    let tokens = tokenize(src).unwrap();
    assert_eq!(
        tokens[0].kind,
        TokenKind::DocComment("This is a doc comment".into())
    );
    assert_eq!(tokens[1].kind, TokenKind::Fn);
}

#[test]
fn s2_lexer_consecutive_doc_comments() {
    use fajar_lang::lexer::token::TokenKind;
    let src = "/// Line 1\n/// Line 2\nfn bar() { 1 }";
    let tokens = tokenize(src).unwrap();
    assert_eq!(tokens[0].kind, TokenKind::DocComment("Line 1".into()));
    assert_eq!(tokens[1].kind, TokenKind::DocComment("Line 2".into()));
    assert_eq!(tokens[2].kind, TokenKind::Fn);
}

#[test]
fn s2_quad_slash_not_doc_comment() {
    use fajar_lang::lexer::token::TokenKind;
    let src = "//// Not a doc comment\nfn foo() { 1 }";
    let tokens = tokenize(src).unwrap();
    // //// is a regular comment, so first token should be Fn
    assert_eq!(tokens[0].kind, TokenKind::Fn);
}

#[test]
fn s2_parser_attaches_doc_to_fn() {
    let src = "/// Adds two numbers\nfn add(a: i64, b: i64) -> i64 { a + b }";
    let tokens = tokenize(src).unwrap();
    let program = parse(tokens).unwrap();
    if let fajar_lang::parser::ast::Item::FnDef(f) = &program.items[0] {
        assert_eq!(f.doc_comment.as_deref(), Some("Adds two numbers"));
        assert_eq!(f.name, "add");
    } else {
        panic!("expected FnDef");
    }
}

#[test]
fn s2_parser_attaches_multiline_doc_to_fn() {
    let src = "/// Line 1\n/// Line 2\nfn foo() { 1 }";
    let tokens = tokenize(src).unwrap();
    let program = parse(tokens).unwrap();
    if let fajar_lang::parser::ast::Item::FnDef(f) = &program.items[0] {
        assert_eq!(f.doc_comment.as_deref(), Some("Line 1\nLine 2"));
    } else {
        panic!("expected FnDef");
    }
}

#[test]
fn s2_parser_attaches_doc_to_struct() {
    let src = "/// A 2D point\nstruct Point { x: f64, y: f64 }";
    let tokens = tokenize(src).unwrap();
    let program = parse(tokens).unwrap();
    if let fajar_lang::parser::ast::Item::StructDef(s) = &program.items[0] {
        assert_eq!(s.doc_comment.as_deref(), Some("A 2D point"));
        assert_eq!(s.name, "Point");
    } else {
        panic!("expected StructDef");
    }
}

#[test]
fn s2_parser_attaches_doc_to_enum() {
    let src = "/// Shape variants\nenum Shape { Circle(f64), Rect(f64, f64) }";
    let tokens = tokenize(src).unwrap();
    let program = parse(tokens).unwrap();
    if let fajar_lang::parser::ast::Item::EnumDef(e) = &program.items[0] {
        assert_eq!(e.doc_comment.as_deref(), Some("Shape variants"));
        assert_eq!(e.name, "Shape");
    } else {
        panic!("expected EnumDef");
    }
}

#[test]
fn s2_fn_without_doc_has_none() {
    let src = "fn foo() { 1 }";
    let tokens = tokenize(src).unwrap();
    let program = parse(tokens).unwrap();
    if let fajar_lang::parser::ast::Item::FnDef(f) = &program.items[0] {
        assert!(f.doc_comment.is_none());
    } else {
        panic!("expected FnDef");
    }
}

#[test]
fn s2_doc_generation_extracts_items() {
    let src = r#"
/// Adds two numbers
fn add(a: i64, b: i64) -> i64 { a + b }

/// A point in 2D space
struct Point { x: f64, y: f64 }

fn no_doc() { 1 }
"#;
    let tokens = tokenize(src).unwrap();
    let program = parse(tokens).unwrap();
    let items = fajar_lang::docgen::extract_doc_items(&program);
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].name, "add");
    assert_eq!(items[1].name, "Point");
}

#[test]
fn s2_doc_html_generation() {
    let src = "/// Adds two numbers\nfn add(a: i64, b: i64) -> i64 { a + b }";
    let tokens = tokenize(src).unwrap();
    let program = parse(tokens).unwrap();
    let html = fajar_lang::docgen::generate_docs("test", &program);
    assert!(html.contains("<!DOCTYPE html>"));
    assert!(html.contains("add"));
    assert!(html.contains("Adds two numbers"));
    assert!(html.contains("fn add(a: i64, b: i64) -&gt; i64"));
}

#[test]
fn s2_doc_test_extraction() {
    let src = "/// Example:\n/// ```\n/// let x = 42\n/// ```\nfn foo() { 1 }";
    let tokens = tokenize(src).unwrap();
    let program = parse(tokens).unwrap();
    let doc_tests = fajar_lang::docgen::extract_doc_tests(&program);
    assert_eq!(doc_tests.len(), 1);
    assert_eq!(doc_tests[0].0, "foo");
    assert_eq!(doc_tests[0].1, "let x = 42");
}
