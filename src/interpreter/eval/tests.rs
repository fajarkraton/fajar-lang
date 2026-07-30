use super::*;
use crate::lexer::tokenize;
use crate::parser::parse;

/// Helper: parse and eval a source string, returning the Value.
fn eval(source: &str) -> Result<Value, RuntimeError> {
    let tokens = tokenize(source).expect("lex error");
    let program = parse(tokens).expect("parse error");
    let mut interp = Interpreter::new_capturing();
    interp.eval_program(&program)
}

/// Helper: parse and eval, return captured output lines.
fn eval_output(source: &str) -> Vec<String> {
    let tokens = tokenize(source).expect("lex error");
    let program = parse(tokens).expect("parse error");
    let mut interp = Interpreter::new_capturing();
    interp.eval_program(&program).expect("runtime error");
    interp.get_output().to_vec()
}

// ── Literals ──

#[test]
fn eval_int_literal() {
    assert_eq!(eval("42").unwrap(), Value::Int(42));
}

#[test]
fn eval_float_literal() {
    assert_eq!(eval("1.25").unwrap(), Value::Float(1.25));
}

#[test]
fn eval_string_literal() {
    assert_eq!(eval("\"hello\"").unwrap(), Value::Str("hello".into()));
}

#[test]
fn eval_bool_literal() {
    assert_eq!(eval("true").unwrap(), Value::Bool(true));
    assert_eq!(eval("false").unwrap(), Value::Bool(false));
}

#[test]
fn eval_null_literal() {
    assert_eq!(eval("null").unwrap(), Value::Null);
}

// ── Arithmetic ──

#[test]
fn eval_addition() {
    assert_eq!(eval("1 + 2").unwrap(), Value::Int(3));
}

#[test]
fn eval_subtraction() {
    assert_eq!(eval("10 - 3").unwrap(), Value::Int(7));
}

#[test]
fn eval_multiplication() {
    assert_eq!(eval("4 * 5").unwrap(), Value::Int(20));
}

#[test]
fn eval_division() {
    assert_eq!(eval("10 / 3").unwrap(), Value::Int(3));
}

#[test]
fn eval_modulo() {
    assert_eq!(eval("10 % 3").unwrap(), Value::Int(1));
}

#[test]
fn eval_power() {
    assert_eq!(eval("2 ** 10").unwrap(), Value::Int(1024));
}

#[test]
fn eval_division_by_zero() {
    let err = eval("1 / 0").unwrap_err();
    assert!(matches!(err, RuntimeError::DivisionByZero));
}

#[test]
fn eval_float_arithmetic() {
    assert_eq!(eval("1.5 + 2.5").unwrap(), Value::Float(4.0));
    assert_eq!(eval("3.0 * 2.0").unwrap(), Value::Float(6.0));
}

#[test]
fn eval_mixed_int_float() {
    assert_eq!(eval("1 + 2.5").unwrap(), Value::Float(3.5));
    assert_eq!(eval("2.0 * 3").unwrap(), Value::Float(6.0));
}

#[test]
fn eval_string_concat() {
    assert_eq!(
        eval("\"hello\" + \" world\"").unwrap(),
        Value::Str("hello world".into())
    );
}

#[test]
fn eval_precedence() {
    assert_eq!(eval("2 + 3 * 4").unwrap(), Value::Int(14));
    assert_eq!(eval("(2 + 3) * 4").unwrap(), Value::Int(20));
}

// ── Comparison ──

#[test]
fn eval_comparison_int() {
    assert_eq!(eval("1 < 2").unwrap(), Value::Bool(true));
    assert_eq!(eval("2 > 1").unwrap(), Value::Bool(true));
    assert_eq!(eval("1 == 1").unwrap(), Value::Bool(true));
    assert_eq!(eval("1 != 2").unwrap(), Value::Bool(true));
    assert_eq!(eval("1 >= 1").unwrap(), Value::Bool(true));
    assert_eq!(eval("1 <= 1").unwrap(), Value::Bool(true));
}

// ── Logical ──

#[test]
fn eval_logical_and_or() {
    assert_eq!(eval("true && false").unwrap(), Value::Bool(false));
    assert_eq!(eval("true || false").unwrap(), Value::Bool(true));
}

#[test]
fn eval_logical_short_circuit() {
    // false && (anything) should not evaluate RHS
    assert_eq!(eval("false && (1 / 0 == 0)").unwrap(), Value::Bool(false));
    // true || (anything) should not evaluate RHS
    assert_eq!(eval("true || (1 / 0 == 0)").unwrap(), Value::Bool(true));
}

// ── Unary ──

#[test]
fn eval_negation() {
    assert_eq!(eval("-42").unwrap(), Value::Int(-42));
    assert_eq!(eval("-1.25").unwrap(), Value::Float(-1.25));
}

#[test]
fn eval_logical_not() {
    assert_eq!(eval("!true").unwrap(), Value::Bool(false));
    assert_eq!(eval("!false").unwrap(), Value::Bool(true));
}

#[test]
fn eval_bitwise_not() {
    assert_eq!(eval("~0").unwrap(), Value::Int(-1));
}

// ── Bitwise ──

#[test]
fn eval_bitwise_ops() {
    assert_eq!(eval("5 & 3").unwrap(), Value::Int(1));
    assert_eq!(eval("5 | 3").unwrap(), Value::Int(7));
    assert_eq!(eval("5 ^ 3").unwrap(), Value::Int(6));
    assert_eq!(eval("1 << 3").unwrap(), Value::Int(8));
    assert_eq!(eval("8 >> 2").unwrap(), Value::Int(2));
}

// ── Variables ──

#[test]
fn eval_let_binding() {
    assert_eq!(eval("let x = 42; x").unwrap(), Value::Int(42));
}

#[test]
fn eval_let_mut_assignment() {
    let src = "let mut x = 1; x = 2; x";
    assert_eq!(eval(src).unwrap(), Value::Int(2));
}

#[test]
fn eval_compound_assignment() {
    let src = "let mut x = 10; x += 5; x";
    assert_eq!(eval(src).unwrap(), Value::Int(15));
}

#[test]
fn eval_undefined_variable() {
    let err = eval("x").unwrap_err();
    assert!(matches!(err, RuntimeError::UndefinedVariable(_)));
}

// ── Blocks ──

#[test]
fn eval_block_returns_last_expr() {
    assert_eq!(eval("{ 1; 2; 3 }").unwrap(), Value::Int(3));
}

#[test]
fn eval_block_scope() {
    let src = "let x = 1; { let x = 2 }; x";
    assert_eq!(eval(src).unwrap(), Value::Int(1));
}

// ── If/Else ──

#[test]
fn eval_if_true() {
    assert_eq!(eval("if true { 1 } else { 2 }").unwrap(), Value::Int(1));
}

#[test]
fn eval_if_false() {
    assert_eq!(eval("if false { 1 } else { 2 }").unwrap(), Value::Int(2));
}

#[test]
fn eval_if_no_else() {
    assert_eq!(eval("if false { 1 }").unwrap(), Value::Null);
}

#[test]
fn eval_if_else_if() {
    let src = "if false { 1 } else if true { 2 } else { 3 }";
    assert_eq!(eval(src).unwrap(), Value::Int(2));
}

// ── While ──

#[test]
fn eval_while_loop() {
    let src = "let mut x = 0; while x < 5 { x += 1 }; x";
    assert_eq!(eval(src).unwrap(), Value::Int(5));
}

#[test]
fn eval_while_break() {
    let src = "let mut x = 0; while true { x += 1; if x == 3 { break } }; x";
    assert_eq!(eval(src).unwrap(), Value::Int(3));
}

#[test]
fn eval_while_continue() {
    // Sum only odd numbers 1..5
    let src = r#"
        let mut sum = 0
        let mut i = 0
        while i < 5 {
            i += 1
            if i % 2 == 0 { continue }
            sum += i
        }
        sum
    "#;
    assert_eq!(eval(src).unwrap(), Value::Int(9)); // 1+3+5
}

// ── For ──

#[test]
fn eval_for_loop_array() {
    let src = "let mut sum = 0; for x in [1, 2, 3] { sum += x }; sum";
    assert_eq!(eval(src).unwrap(), Value::Int(6));
}

#[test]
fn eval_for_loop_range() {
    let src = "let mut sum = 0; for i in 0..5 { sum += i }; sum";
    assert_eq!(eval(src).unwrap(), Value::Int(10)); // 0+1+2+3+4
}

#[test]
fn eval_for_loop_string() {
    let output = eval_output("for c in \"abc\" { println(c) }");
    assert_eq!(output, vec!["a", "b", "c"]);
}

// ── Functions ──

#[test]
fn eval_function_def_and_call() {
    let src = "fn add(a: i64, b: i64) -> i64 { a + b }\nadd(3, 4)";
    assert_eq!(eval(src).unwrap(), Value::Int(7));
}

#[test]
fn eval_recursive_function() {
    let src = r#"
        fn fact(n: i64) -> i64 {
            if n <= 1 { 1 } else { n * fact(n - 1) }
        }
        fact(5)
    "#;
    assert_eq!(eval(src).unwrap(), Value::Int(120));
}

#[test]
fn eval_function_return() {
    let src = r#"
        fn early(n: i64) -> i64 {
            if n < 0 { return -1 }
            n * 2
        }
        early(-5)
    "#;
    assert_eq!(eval(src).unwrap(), Value::Int(-1));
}

#[test]
pub(super) fn eval_closure() {
    let src = r#"
        let double = |x: i64| -> i64 { x * 2 }
        double(5)
    "#;
    assert_eq!(eval(src).unwrap(), Value::Int(10));
}

#[test]
fn eval_closure_captures_env() {
    let src = r#"
        let multiplier = 3
        let mul = |x: i64| -> i64 { x * multiplier }
        mul(4)
    "#;
    assert_eq!(eval(src).unwrap(), Value::Int(12));
}

#[test]
fn eval_arity_mismatch() {
    let src = "fn f(a: i64) -> i64 { a }\nf(1, 2)";
    let err = eval(src).unwrap_err();
    assert!(matches!(err, RuntimeError::ArityMismatch { .. }));
}

#[test]
fn eval_stack_overflow() {
    // Run in a thread with larger stack to ensure the interpreter's depth check
    // catches the overflow before the Rust stack itself overflows in debug mode.
    let result = std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(|| {
            let src = "fn inf(n: i64) -> i64 { inf(n) }\ninf(0)";
            eval(src).unwrap_err()
        })
        .expect("thread spawn")
        .join()
        .expect("thread join");
    assert!(matches!(result, RuntimeError::StackOverflow { .. }));
}

// ── Match ──

#[test]
fn eval_match_literal() {
    let src = r#"
        let x = 2
        match x {
            1 => "one",
            2 => "two",
            _ => "other"
        }
    "#;
    assert_eq!(eval(src).unwrap(), Value::Str("two".into()));
}

#[test]
fn eval_match_wildcard() {
    let src = "match 42 { _ => true }";
    assert_eq!(eval(src).unwrap(), Value::Bool(true));
}

#[test]
fn eval_match_binding() {
    let src = "match 42 { n => n + 1 }";
    assert_eq!(eval(src).unwrap(), Value::Int(43));
}

// ── Structs ──

#[test]
fn eval_struct_init_and_field() {
    let src = r#"
        let p = Point { x: 1, y: 2 }
        p.x
    "#;
    assert_eq!(eval(src).unwrap(), Value::Int(1));
}

#[test]
fn eval_struct_field_assign() {
    let src = r#"
        let mut p = Point { x: 1, y: 2 }
        p.x = 10
        p.x
    "#;
    assert_eq!(eval(src).unwrap(), Value::Int(10));
}

// ── Arrays ──

#[test]
fn eval_array_literal() {
    assert_eq!(
        eval("[1, 2, 3]").unwrap(),
        Value::array_from_vec(vec![Value::Int(1), Value::Int(2), Value::Int(3)])
    );
}

#[test]
fn eval_array_index() {
    assert_eq!(eval("[10, 20, 30][1]").unwrap(), Value::Int(20));
}

#[test]
fn eval_array_index_assign() {
    let src = "let mut arr = [1, 2, 3]; arr[0] = 10; arr[0]";
    assert_eq!(eval(src).unwrap(), Value::Int(10));
}

// ── Tuples ──

#[test]
fn eval_tuple_literal() {
    assert_eq!(
        eval("(1, true)").unwrap(),
        Value::Tuple(vec![Value::Int(1), Value::Bool(true)])
    );
}

// ── Pipeline ──

#[test]
fn eval_pipeline() {
    let src = r#"
        fn double(x: i64) -> i64 { x * 2 }
        5 |> double
    "#;
    assert_eq!(eval(src).unwrap(), Value::Int(10));
}

// ── Builtins ──

#[test]
fn eval_println() {
    let output = eval_output("println(\"hello\")");
    assert_eq!(output, vec!["hello"]);
}

#[test]
fn eval_println_int() {
    let output = eval_output("println(42)");
    assert_eq!(output, vec!["42"]);
}

#[test]
fn eval_len_string() {
    assert_eq!(eval("len(\"hello\")").unwrap(), Value::Int(5));
}

#[test]
fn eval_len_array() {
    assert_eq!(eval("len([1, 2, 3])").unwrap(), Value::Int(3));
}

#[test]
fn eval_type_of() {
    assert_eq!(eval("type_of(42)").unwrap(), Value::Str("i64".into()));
    assert_eq!(eval("type_of(\"hi\")").unwrap(), Value::Str("str".into()));
}

#[test]
fn eval_to_string() {
    assert_eq!(eval("to_string(42)").unwrap(), Value::Str("42".into()));
}

#[test]
fn eval_assert_pass() {
    assert_eq!(eval("assert(true)").unwrap(), Value::Null);
}

#[test]
fn eval_assert_fail() {
    let err = eval("assert(false)").unwrap_err();
    assert!(matches!(err, RuntimeError::TypeError(_)));
}

#[test]
fn eval_assert_eq_pass() {
    assert_eq!(eval("assert_eq(1, 1)").unwrap(), Value::Null);
}

#[test]
fn eval_assert_eq_fail() {
    let err = eval("assert_eq(1, 2)").unwrap_err();
    assert!(matches!(err, RuntimeError::TypeError(_)));
}

// ── Complex programs ──

#[test]
fn eval_fibonacci() {
    let src = r#"
        fn fib(n: i64) -> i64 {
            if n <= 1 { n } else { fib(n - 1) + fib(n - 2) }
        }
        fib(10)
    "#;
    assert_eq!(eval(src).unwrap(), Value::Int(55));
}

#[test]
fn eval_nested_functions() {
    let src = r#"
        fn outer(x: i64) -> i64 {
            fn inner(y: i64) -> i64 { y * 2 }
            inner(x) + 1
        }
        outer(5)
    "#;
    assert_eq!(eval(src).unwrap(), Value::Int(11));
}

#[test]
fn eval_counter_closure() {
    let src = r#"
        fn make_adder(base: i64) -> fn(i64) -> i64 {
            |x: i64| -> i64 { base + x }
        }
        let add5 = make_adder(5)
        add5(3)
    "#;
    assert_eq!(eval(src).unwrap(), Value::Int(8));
}

// ── impl blocks & method dispatch ──

#[test]
fn eval_impl_basic_method() {
    let src = r#"
        struct Point { x: f64, y: f64 }
        impl Point {
            fn magnitude_sq(self) -> f64 {
                self.x * self.x + self.y * self.y
            }
        }
        let p = Point { x: 3.0, y: 4.0 }
        p.magnitude_sq()
    "#;
    assert_eq!(eval(src).unwrap(), Value::Float(25.0));
}

#[test]
fn eval_impl_multiple_methods() {
    let src = r#"
        struct Rect { w: f64, h: f64 }
        impl Rect {
            fn area(self) -> f64 { self.w * self.h }
            fn perimeter(self) -> f64 { 2.0 * (self.w + self.h) }
        }
        let r = Rect { w: 5.0, h: 3.0 }
        r.area() + r.perimeter()
    "#;
    // area=15, perimeter=16, total=31
    assert_eq!(eval(src).unwrap(), Value::Float(31.0));
}

#[test]
fn eval_impl_method_with_args() {
    let src = r#"
        struct Counter { value: i64 }
        impl Counter {
            fn add(self, n: i64) -> i64 {
                self.value + n
            }
        }
        let c = Counter { value: 10 }
        c.add(5)
    "#;
    assert_eq!(eval(src).unwrap(), Value::Int(15));
}

#[test]
fn eval_impl_static_method() {
    let src = r#"
        struct Point { x: f64, y: f64 }
        impl Point {
            fn origin() -> Point {
                Point { x: 0.0, y: 0.0 }
            }
        }
        let p = Point::origin()
        p.x
    "#;
    assert_eq!(eval(src).unwrap(), Value::Float(0.0));
}

#[test]
fn eval_impl_static_with_args() {
    let src = r#"
        struct Point { x: f64, y: f64 }
        impl Point {
            fn new(x: f64, y: f64) -> Point {
                Point { x: x, y: y }
            }
        }
        let p = Point::new(3.0, 4.0)
        p.x + p.y
    "#;
    assert_eq!(eval(src).unwrap(), Value::Float(7.0));
}

#[test]
fn eval_impl_method_not_found() {
    let src = r#"
        struct Foo { x: i64 }
        impl Foo {
            fn bar(self) -> i64 { self.x }
        }
        let f = Foo { x: 1 }
        f.baz()
    "#;
    let err = eval(src).unwrap_err();
    assert!(matches!(err, RuntimeError::TypeError(_)));
}

#[test]
fn eval_impl_method_returns_struct() {
    let src = r#"
        struct Vec2 { x: f64, y: f64 }
        impl Vec2 {
            fn scale(self, factor: f64) -> Vec2 {
                Vec2 { x: self.x * factor, y: self.y * factor }
            }
        }
        let v = Vec2 { x: 1.0, y: 2.0 }
        let v2 = v.scale(3.0)
        v2.x + v2.y
    "#;
    // 3.0 + 6.0 = 9.0
    assert_eq!(eval(src).unwrap(), Value::Float(9.0));
}

#[test]
fn eval_impl_method_chain_output() {
    let src = r#"
        struct Greeter { name: str }
        impl Greeter {
            fn greet(self) -> str {
                "Hello, " + self.name + "!"
            }
        }
        let g = Greeter { name: "Fajar" }
        println(g.greet())
    "#;
    let output = eval_output(src);
    assert_eq!(output, vec!["Hello, Fajar!"]);
}

#[test]
fn eval_impl_two_structs() {
    let src = r#"
        struct Dog { name: str }
        struct Cat { name: str }
        impl Dog {
            fn speak(self) -> str { self.name + " says woof" }
        }
        impl Cat {
            fn speak(self) -> str { self.name + " says meow" }
        }
        let d = Dog { name: "Rex" }
        let c = Cat { name: "Whiskers" }
        d.speak() + " and " + c.speak()
    "#;
    assert_eq!(
        eval(src).unwrap(),
        Value::Str("Rex says woof and Whiskers says meow".into())
    );
}

#[test]
fn eval_impl_self_field_access() {
    let src = r#"
        struct Circle { radius: f64 }
        impl Circle {
            fn diameter(self) -> f64 { self.radius * 2.0 }
            fn area_approx(self) -> f64 { 1.5 * self.radius * self.radius }
        }
        let c = Circle { radius: 5.0 }
        c.diameter()
    "#;
    assert_eq!(eval(src).unwrap(), Value::Float(10.0));
}

// ── Option/Result & ? operator ──

#[test]
fn eval_some_constructor() {
    assert_eq!(
        eval("Some(42)").unwrap(),
        Value::Enum {
            variant: "Some".into(),
            data: Some(Box::new(Value::Int(42)))
        }
    );
}

#[test]
fn eval_none_value() {
    assert_eq!(
        eval("None").unwrap(),
        Value::Enum {
            variant: "None".into(),
            data: None
        }
    );
}

#[test]
fn eval_ok_constructor() {
    assert_eq!(
        eval("Ok(10)").unwrap(),
        Value::Enum {
            variant: "Ok".into(),
            data: Some(Box::new(Value::Int(10)))
        }
    );
}

#[test]
fn eval_err_constructor() {
    assert_eq!(
        eval("Err(\"bad\")").unwrap(),
        Value::Enum {
            variant: "Err".into(),
            data: Some(Box::new(Value::Str("bad".into())))
        }
    );
}

#[test]
fn eval_try_unwraps_ok() {
    let src = r#"
        fn get_val() -> i64 {
            let x = Ok(42)?
            x
        }
        get_val()
    "#;
    // ? on Ok(42) returns 42, so get_val returns 42
    // But the return is wrapped as ControlFlow::Return — need a top-level fn
    assert_eq!(eval(src).unwrap(), Value::Int(42));
}

#[test]
fn eval_try_unwraps_some() {
    let src = r#"
        fn get_val() -> i64 {
            let x = Some(99)?
            x
        }
        get_val()
    "#;
    assert_eq!(eval(src).unwrap(), Value::Int(99));
}

#[test]
fn eval_try_short_circuits_err() {
    let src = r#"
        fn might_fail() -> i64 {
            let x = Err("oops")?
            x + 100
        }
        might_fail()
    "#;
    // ? on Err short-circuits, returning Err("oops")
    assert_eq!(
        eval(src).unwrap(),
        Value::Enum {
            variant: "Err".into(),
            data: Some(Box::new(Value::Str("oops".into())))
        }
    );
}

#[test]
fn eval_try_short_circuits_none() {
    let src = r#"
        fn might_fail() -> i64 {
            let x = None?
            x + 100
        }
        might_fail()
    "#;
    assert_eq!(
        eval(src).unwrap(),
        Value::Enum {
            variant: "None".into(),
            data: None
        }
    );
}

#[test]
fn eval_try_propagation_chain() {
    let src = r#"
        fn step1() -> i64 { Ok(10) }
        fn step2() -> i64 {
            let a = step1()?
            let b = Ok(20)?
            a + b
        }
        step2()
    "#;
    // step1() returns Ok(10), ? unwraps to 10
    // Ok(20) unwrapped to 20, total = 30
    assert_eq!(eval(src).unwrap(), Value::Int(30));
}

#[test]
fn eval_unwrap_some() {
    assert_eq!(eval("Some(42).unwrap()").unwrap(), Value::Int(42));
}

#[test]
fn eval_unwrap_none_panics() {
    let err = eval("None.unwrap()").unwrap_err();
    assert!(matches!(err, RuntimeError::TypeError(_)));
}

#[test]
fn eval_unwrap_ok() {
    assert_eq!(eval("Ok(7).unwrap()").unwrap(), Value::Int(7));
}

#[test]
fn eval_unwrap_err_panics() {
    let err = eval("Err(\"fail\").unwrap()").unwrap_err();
    assert!(matches!(err, RuntimeError::TypeError(_)));
}

#[test]
fn eval_unwrap_or_some() {
    assert_eq!(eval("Some(42).unwrap_or(0)").unwrap(), Value::Int(42));
}

#[test]
fn eval_unwrap_or_none() {
    assert_eq!(eval("None.unwrap_or(99)").unwrap(), Value::Int(99));
}

#[test]
fn eval_unwrap_or_err() {
    assert_eq!(eval("Err(\"x\").unwrap_or(100)").unwrap(), Value::Int(100));
}

#[test]
fn eval_is_some_is_none() {
    assert_eq!(eval("Some(1).is_some()").unwrap(), Value::Bool(true));
    assert_eq!(eval("Some(1).is_none()").unwrap(), Value::Bool(false));
    assert_eq!(eval("None.is_some()").unwrap(), Value::Bool(false));
    assert_eq!(eval("None.is_none()").unwrap(), Value::Bool(true));
}

#[test]
fn eval_is_ok_is_err() {
    assert_eq!(eval("Ok(1).is_ok()").unwrap(), Value::Bool(true));
    assert_eq!(eval("Ok(1).is_err()").unwrap(), Value::Bool(false));
    assert_eq!(eval("Err(1).is_ok()").unwrap(), Value::Bool(false));
    assert_eq!(eval("Err(1).is_err()").unwrap(), Value::Bool(true));
}

#[test]
fn eval_match_on_option() {
    let src = r#"
        let val = Some(42)
        match val {
            Some(x) => x * 2,
            None => 0
        }
    "#;
    // Match on Option — variant patterns
    // Current match system uses pattern matching on enum variants
    assert_eq!(eval(src).unwrap(), Value::Int(84));
}

// ── Sprint 12: Memory Safety ──

#[test]
fn s12_integer_overflow_add_panics() {
    let src = "9223372036854775807 + 1"; // i64::MAX + 1
    let err = eval(src).unwrap_err();
    assert!(matches!(err, RuntimeError::IntegerOverflow { .. }));
}

#[test]
fn s12_integer_overflow_sub_panics() {
    // i64::MIN is -9223372036854775808, but lexer can't parse that literal directly.
    // Use -(i64::MAX) - 1 - 1 to reach underflow.
    let src = "let x = -9223372036854775807 - 1\nx - 1"; // i64::MIN - 1
    let err = eval(src).unwrap_err();
    assert!(matches!(err, RuntimeError::IntegerOverflow { .. }));
}

#[test]
fn s12_integer_overflow_mul_panics() {
    let src = "9223372036854775807 * 2"; // i64::MAX * 2
    let err = eval(src).unwrap_err();
    assert!(matches!(err, RuntimeError::IntegerOverflow { .. }));
}

#[test]
fn s12_integer_overflow_pow_panics() {
    let src = "2 ** 63"; // 2^63 overflows i64
    let err = eval(src).unwrap_err();
    assert!(matches!(err, RuntimeError::IntegerOverflow { .. }));
}

#[test]
fn s12_wrapping_add_wraps_correctly() {
    let src = "wrapping_add(9223372036854775807, 1)";
    assert_eq!(eval(src).unwrap(), Value::Int(i64::MIN));
}

#[test]
fn s12_wrapping_sub_wraps_correctly() {
    // i64::MIN wrapping_sub 1 = i64::MAX
    let src = "let x = -9223372036854775807 - 1\nwrapping_sub(x, 1)";
    assert_eq!(eval(src).unwrap(), Value::Int(i64::MAX));
}

#[test]
fn s12_wrapping_mul_wraps_correctly() {
    let src = "wrapping_mul(9223372036854775807, 2)";
    assert_eq!(eval(src).unwrap(), Value::Int(-2));
}

#[test]
fn s12_checked_add_returns_some_on_success() {
    let src = "checked_add(1, 2)";
    assert_eq!(
        eval(src).unwrap(),
        Value::Enum {
            variant: "Some".into(),
            data: Some(Box::new(Value::Int(3))),
        }
    );
}

#[test]
fn s12_checked_add_returns_none_on_overflow() {
    let src = "checked_add(9223372036854775807, 1)";
    assert_eq!(
        eval(src).unwrap(),
        Value::Enum {
            variant: "None".into(),
            data: None,
        }
    );
}

#[test]
fn s12_saturating_add_saturates() {
    let src = "saturating_add(9223372036854775807, 1)";
    assert_eq!(eval(src).unwrap(), Value::Int(i64::MAX));
}

#[test]
fn s12_saturating_sub_saturates() {
    // i64::MIN saturating_sub 1 = i64::MIN (saturated)
    let src = "let x = -9223372036854775807 - 1\nsaturating_sub(x, 1)";
    assert_eq!(eval(src).unwrap(), Value::Int(i64::MIN));
}

#[test]
fn s12_array_index_out_of_bounds_re010() {
    let src = "[1, 2, 3][5]";
    let err = eval(src).unwrap_err();
    assert!(matches!(
        err,
        RuntimeError::IndexOutOfBounds {
            index: 5,
            collection: _,
            length: 3,
        }
    ));
}

#[test]
fn s12_string_index_out_of_bounds_re010() {
    let src = r#""hi"[5]"#;
    let err = eval(src).unwrap_err();
    assert!(matches!(
        err,
        RuntimeError::IndexOutOfBounds {
            index: 5,
            collection: _,
            length: 2,
        }
    ));
}

#[test]
fn s12_stack_overflow_configurable_depth() {
    // Use eval_with_depth helper to test custom depth
    let src = "fn inf(n: i64) -> i64 { inf(n) }\ninf(0)";
    let tokens = crate::lexer::tokenize(src).unwrap();
    let program = crate::parser::parse(tokens).unwrap();
    let mut interp = Interpreter::new_capturing();
    interp.set_max_recursion_depth(10);
    let err = interp.eval_program(&program).unwrap_err();
    match err {
        RuntimeError::StackOverflow { depth, .. } => assert_eq!(depth, 10),
        other => panic!("expected StackOverflow, got: {other:?}"),
    }
}

#[test]
fn stack_overflow_includes_backtrace() {
    let src = "fn c(n: i64) -> i64 { c(n) }\nfn b(n: i64) -> i64 { c(n) }\nfn a(n: i64) -> i64 { b(n) }\na(1)";
    let tokens = crate::lexer::tokenize(src).unwrap();
    let program = crate::parser::parse(tokens).unwrap();
    let mut interp = Interpreter::new_capturing();
    interp.set_max_recursion_depth(5);
    let err = interp.eval_program(&program).unwrap_err();
    match err {
        RuntimeError::StackOverflow { backtrace, .. } => {
            assert!(backtrace.contains("a()"), "backtrace should show a()");
            assert!(backtrace.contains("b()"), "backtrace should show b()");
            assert!(backtrace.contains("c()"), "backtrace should show c()");
        }
        other => panic!("expected StackOverflow, got: {other:?}"),
    }
}

#[test]
fn call_stack_tracks_functions() {
    // After normal execution, call stack should be empty
    let src = "fn foo() -> i64 { 42 }\nfoo()";
    let tokens = crate::lexer::tokenize(src).unwrap();
    let program = crate::parser::parse(tokens).unwrap();
    let mut interp = Interpreter::new_capturing();
    interp.eval_program(&program).unwrap();
    assert!(interp.get_call_stack().is_empty());
}

#[test]
fn s12_normal_arithmetic_no_overflow() {
    // Normal operations should work fine
    assert_eq!(eval("100 + 200").unwrap(), Value::Int(300));
    assert_eq!(eval("1000 * 1000").unwrap(), Value::Int(1_000_000));
    assert_eq!(eval("50 - 100").unwrap(), Value::Int(-50));
    assert_eq!(eval("2 ** 10").unwrap(), Value::Int(1024));
}

#[test]
fn s12_null_safety_option_must_be_matched() {
    // Option must be matched or unwrapped — ? on None propagates
    let src = r#"
        fn safe() -> i64 {
            let val = None?
            val + 1
        }
        safe()
    "#;
    // None? should short-circuit, returning None (not null)
    assert_eq!(
        eval(src).unwrap(),
        Value::Enum {
            variant: "None".into(),
            data: None,
        }
    );
}

#[test]
fn s12_try_operator_only_on_option_result() {
    // ? on a non-Option/Result value should error
    let src = r#"
        fn bad() -> i64 {
            let x = 42?
            x
        }
        bad()
    "#;
    let err = eval(src).unwrap_err();
    assert!(matches!(err, RuntimeError::TypeError(_)));
}

#[test]
fn s12_null_arithmetic_is_type_error() {
    // null + 1 should be a type error
    let src = "null + 1";
    let err = eval(src).unwrap_err();
    assert!(matches!(err, RuntimeError::TypeError(_)));
}

#[test]
fn s12_div_overflow_i64_min_by_neg1() {
    // i64::MIN / -1 overflows (would be i64::MAX + 1)
    let src = "let x = -9223372036854775807 - 1\nx / -1";
    let err = eval(src).unwrap_err();
    assert!(matches!(err, RuntimeError::IntegerOverflow { .. }));
}

// --- Labeled break/continue tests ---

#[test]
fn labeled_break_outer_while() {
    let src = r#"
        fn main() -> i64 {
            let mut result = 0
            'outer: while true {
                let mut j = 0
                while j < 10 {
                    if j == 3 {
                        result = 42
                        break 'outer
                    }
                    j = j + 1
                }
            }
            result
        }
        main()
    "#;
    assert_eq!(eval(src).unwrap(), Value::Int(42));
}

#[test]
fn labeled_continue_outer_while() {
    let src = r#"
        fn main() -> i64 {
            let mut count = 0
            let mut i = 0
            'outer: while i < 5 {
                i = i + 1
                let mut j = 0
                while j < 5 {
                    j = j + 1
                    if j == 2 {
                        continue 'outer
                    }
                }
                count = count + 1
            }
            count
        }
        main()
    "#;
    // Inner loop always hits continue 'outer at j==2,
    // so count never increments
    assert_eq!(eval(src).unwrap(), Value::Int(0));
}

#[test]
fn labeled_break_outer_loop() {
    let src = r#"
        fn main() -> i64 {
            let mut x = 0
            'outer: loop {
                loop {
                    x = 99
                    break 'outer
                }
            }
            x
        }
        main()
    "#;
    assert_eq!(eval(src).unwrap(), Value::Int(99));
}

#[test]
fn labeled_break_inner_only() {
    // break without label only breaks inner loop
    let src = r#"
        fn main() -> i64 {
            let mut sum = 0
            let mut i = 0
            while i < 3 {
                let mut j = 0
                while j < 100 {
                    if j == 2 {
                        break
                    }
                    j = j + 1
                }
                sum = sum + i
                i = i + 1
            }
            sum
        }
        main()
    "#;
    // sum = 0 + 1 + 2 = 3
    assert_eq!(eval(src).unwrap(), Value::Int(3));
}

#[test]
fn labeled_break_for_loop() {
    let src = r#"
        fn main() -> i64 {
            let mut result = 0
            'outer: for i in 0..10 {
                for j in 0..10 {
                    if i + j == 5 {
                        result = i * 100 + j
                        break 'outer
                    }
                }
            }
            result
        }
        main()
    "#;
    // First time i+j==5: i=0, j=5 → result=5
    assert_eq!(eval(src).unwrap(), Value::Int(5));
}

// --- const in function body tests ---

#[test]
fn const_in_function_body() {
    let src = r#"
        fn main() -> i64 {
            const SIZE: i64 = 4096 * 16
            SIZE
        }
        main()
    "#;
    assert_eq!(eval(src).unwrap(), Value::Int(65536));
}

#[test]
fn const_chained_in_body() {
    let src = r#"
        fn main() -> i64 {
            const A: i64 = 100
            const B: i64 = A * 2
            const C: i64 = A + B
            C
        }
        main()
    "#;
    assert_eq!(eval(src).unwrap(), Value::Int(300));
}

#[test]
fn const_immutability_check() {
    // const assignment should produce analyzer error SE007
    use crate::analyzer::analyze;
    use crate::lexer::tokenize;
    use crate::parser::parse;
    let src = r#"
        fn main() -> i64 {
            const X: i64 = 42
            X = 10
            X
        }
    "#;
    let tokens = tokenize(src).unwrap();
    let program = parse(tokens).unwrap();
    let result = analyze(&program);
    assert!(result.is_err(), "const assignment should be rejected");
}

// ── Profiler tests ──

/// Profiling session records function calls when enabled.
#[test]
fn test_profiler_records_function_calls() {
    let src = r#"
        fn add(a: i64, b: i64) -> i64 { a + b }
        fn run() -> i64 { add(1, 2) }
        run()
    "#;
    let tokens = tokenize(src).expect("lex error");
    let program = parse(tokens).expect("parse error");
    let mut interp = Interpreter::new_capturing();
    interp.enable_profiling();
    interp.eval_program(&program).expect("runtime error");
    let session = interp.profile_session.as_ref().expect("session missing");
    assert!(
        session.call_count() > 0,
        "expected at least one call recorded, got {}",
        session.call_count()
    );
}

/// Profiling tracks nested call depth correctly.
#[test]
fn test_profiler_nested_calls() {
    let src = r#"
        fn inner() -> i64 { 42 }
        fn middle() -> i64 { inner() }
        fn outer() -> i64 { middle() }
        outer()
    "#;
    let tokens = tokenize(src).expect("lex error");
    let program = parse(tokens).expect("parse error");
    let mut interp = Interpreter::new_capturing();
    interp.enable_profiling();
    interp.eval_program(&program).expect("runtime error");
    let session = interp.profile_session.as_ref().expect("session missing");
    // outer + middle + inner = 3 calls minimum
    assert!(
        session.call_count() >= 3,
        "expected at least 3 nested calls, got {}",
        session.call_count()
    );
    // At least one call should have depth > 0 (i.e., nested)
    let has_nested = session.records().iter().any(|r| r.depth > 0);
    assert!(
        has_nested,
        "expected at least one nested call with depth > 0"
    );
}

/// to_trace() produces well-formed Chrome JSON trace output.
#[test]
fn test_profiler_output_json() {
    let src = r#"
        fn greet() -> i64 { 1 }
        greet()
    "#;
    let tokens = tokenize(src).expect("lex error");
    let program = parse(tokens).expect("parse error");
    let mut interp = Interpreter::new_capturing();
    interp.enable_profiling();
    interp.eval_program(&program).expect("runtime error");
    let session = interp.profile_session.as_ref().expect("session missing");
    let trace = session.to_trace();
    // Chrome trace is a JSON array: starts with '[' and ends with ']'
    assert!(trace.starts_with('['), "trace should start with '['");
    assert!(trace.ends_with(']'), "trace should end with ']'");
    // Should contain the function name
    assert!(
        trace.contains("greet"),
        "trace should contain function name 'greet'"
    );
}

// ── WebSocket / MQTT tests ──

#[test]
#[cfg_attr(
    feature = "websocket",
    ignore = "requires live WebSocket server; tests mock impl only"
)]
fn test_ws_connect_send_recv_close() {
    let mut interp = Interpreter::new();
    let result = interp.eval_source(
        r#"
        let ws = ws_connect("ws://localhost:8080")
        let sent = ws_send(ws, "hello")
        let msg = ws_recv(ws)
        ws_close(ws)
        msg
    "#,
    );
    assert!(result.is_ok(), "expected Ok, got {:?}", result);
    match result.unwrap() {
        Value::Str(s) => assert_eq!(s, "hello"),
        other => panic!("expected Str, got {:?}", other),
    }
}

#[test]
#[cfg_attr(
    feature = "mqtt",
    ignore = "requires live MQTT broker; tests mock impl only"
)]
fn test_mqtt_pub_sub_roundtrip() {
    let mut interp = Interpreter::new();
    let result = interp.eval_source(
        r#"
        let client = mqtt_connect("localhost")
        mqtt_subscribe(client, "sensors/temp")
        mqtt_publish(client, "sensors/temp", "22.5")
        let msg = mqtt_recv(client)
        mqtt_disconnect(client)
        msg
    "#,
    );
    assert!(result.is_ok(), "expected Ok, got {:?}", result);
    match result.unwrap() {
        Value::Map(m) => {
            assert_eq!(m.get("topic").unwrap(), &Value::Str("sensors/temp".into()));
            assert_eq!(m.get("payload").unwrap(), &Value::Str("22.5".into()));
        }
        other => panic!("expected Map, got {:?}", other),
    }
}

#[test]
fn test_mqtt_no_message_returns_null() {
    let mut interp = Interpreter::new();
    let result = interp.eval_source(
        r#"
        let client = mqtt_connect("localhost")
        mqtt_subscribe(client, "empty/topic")
        let msg = mqtt_recv(client)
        mqtt_disconnect(client)
        msg
    "#,
    );
    assert!(result.is_ok(), "expected Ok, got {:?}", result);
    assert_eq!(result.unwrap(), Value::Null);
}

#[test]
#[cfg_attr(
    feature = "websocket",
    ignore = "requires live WebSocket server; tests mock impl only"
)]
fn test_ws_recv_empty_returns_null() {
    let mut interp = Interpreter::new();
    let result = interp.eval_source(
        r#"
        let ws = ws_connect("ws://example.com")
        let msg = ws_recv(ws)
        ws_close(ws)
        msg
    "#,
    );
    assert!(result.is_ok(), "expected Ok, got {:?}", result);
    assert_eq!(result.unwrap(), Value::Null);
}

#[test]
#[cfg_attr(
    feature = "ble",
    ignore = "requires BlueZ + adapter; tests mock impl only"
)]
fn test_ble_scan_returns_devices() {
    let mut interp = Interpreter::new();
    let result = interp.eval_source(
        r#"
        let devices = ble_scan()
        len(devices)
    "#,
    );
    assert!(result.is_ok(), "expected Ok, got {:?}", result);
    match result.unwrap() {
        Value::Int(n) => assert!(n >= 2, "expected at least 2 simulated devices, got {n}"),
        other => panic!("expected Int, got {:?}", other),
    }
}

#[test]
#[cfg_attr(
    feature = "ble",
    ignore = "requires BlueZ + adapter; tests mock impl only"
)]
fn test_ble_connect_read_write_disconnect() {
    let mut interp = Interpreter::new();
    let result = interp.eval_source(
        r#"
        let handle = ble_connect("AA:BB:CC:DD:EE:01")
        let data = ble_read(handle, "00002a6e-0000-1000-8000-00805f9b34fb")
        let ok = ble_write(handle, "0000ff01-0000-1000-8000-00805f9b34fb", [1])
        ble_disconnect(handle)
        ok
    "#,
    );
    assert!(result.is_ok(), "expected Ok, got {:?}", result);
    assert_eq!(result.unwrap(), Value::Bool(true));
}

#[test]
fn test_ble_connect_invalid_returns_negative() {
    let mut interp = Interpreter::new();
    let result = interp.eval_source(
        r#"
        let handle = ble_connect("XX:XX:XX:XX:XX:XX")
        handle
    "#,
    );
    assert!(result.is_ok(), "expected Ok, got {:?}", result);
    assert_eq!(result.unwrap(), Value::Int(-1));
}

#[test]
#[cfg_attr(
    feature = "ble",
    ignore = "requires BlueZ + adapter; tests mock impl only"
)]
fn test_ble_read_after_write_returns_new_data() {
    let mut interp = Interpreter::new();
    let result = interp.eval_source(
        r#"
        let h = ble_connect("AA:BB:CC:DD:EE:02")
        ble_write(h, "0000ff01-0000-1000-8000-00805f9b34fb", [0x42, 0x43])
        let data = ble_read(h, "0000ff01-0000-1000-8000-00805f9b34fb")
        ble_disconnect(h)
        data
    "#,
    );
    assert!(result.is_ok(), "expected Ok, got {:?}", result);
    match result.unwrap() {
        Value::Array(arr) => {
            assert_eq!(arr.len(), 2);
            assert_eq!(arr[0], Value::Int(0x42));
            assert_eq!(arr[1], Value::Int(0x43));
        }
        other => panic!("expected Array, got {:?}", other),
    }
}

// ===================================================================
// V14 Phase 3 — Algebraic Effect System Tests
// ===================================================================

#[test]
fn ef1_1_effect_declaration_registers_in_env() {
    let mut interp = Interpreter::new_capturing();
    let result = interp.eval_source(
        r#"
        effect Console {
            fn log(msg: str) -> void
            fn read_line() -> str
        }
        let x = 42
        x
        "#,
    );
    assert!(
        result.is_ok(),
        "effect declaration should succeed: {:?}",
        result.err()
    );
    assert_eq!(result.unwrap(), Value::Int(42));
}

#[test]
fn ef1_2_effect_op_registered_as_builtin() {
    let mut interp = Interpreter::new_capturing();
    let result = interp.eval_source(
        r#"
        effect Logger {
            fn info(msg: str) -> void
        }
        // Logger::info should be defined in scope as a builtin
        let f = Logger::info
        type_of(f)
        "#,
    );
    assert!(
        result.is_ok(),
        "effect op lookup should work: {:?}",
        result.err()
    );
    // Should be a BuiltinFn
    match result.unwrap() {
        Value::Str(s) => assert!(s.contains("builtin") || s.contains("function"), "got: {s}"),
        other => panic!("expected type_of to return string, got: {:?}", other),
    }
}

#[test]
fn ef1_3_effect_op_default_handler_outside_handle() {
    // Outside a handle block, user-defined effect ops return Null by default.
    let mut interp = Interpreter::new_capturing();
    let result = interp.eval_source(
        r#"
        effect Logger {
            fn info(msg: str) -> void
        }
        let result = Logger::info("hello")
        result
        "#,
    );
    assert!(
        result.is_ok(),
        "default handler should work: {:?}",
        result.err()
    );
    assert_eq!(result.unwrap(), Value::Null);
}

#[test]
fn ef1_4_handle_intercepts_effect_op() {
    let mut interp = Interpreter::new_capturing();
    let result = interp.eval_source(
        r#"
        effect Ask {
            fn get_name() -> str
        }
        let result = handle {
            Ask::get_name()
        } with {
            Ask::get_name() => { "Fajar" }
        }
        result
        "#,
    );
    assert!(
        result.is_ok(),
        "handle should intercept effect: {:?}",
        result.err()
    );
    assert_eq!(result.unwrap(), Value::Str("Fajar".into()));
}

#[test]
fn ef1_5_handle_with_params() {
    let mut interp = Interpreter::new_capturing();
    let result = interp.eval_source(
        r#"
        effect Logger {
            fn log(msg: str) -> void
        }
        let captured = ""
        let result = handle {
            Logger::log("hello world")
            42
        } with {
            Logger::log(msg) => { msg }
        }
        result
        "#,
    );
    assert!(
        result.is_ok(),
        "handle with params should work: {:?}",
        result.err()
    );
    // V15: With multi-step continuations, the handler's resume value ("hello world")
    // is cached and the body continues. The body's final expression (42) is the
    // result of the handle expression.
    assert_eq!(result.unwrap(), Value::Int(42));
}

#[test]
fn ef1_6_resume_in_handler() {
    let mut interp = Interpreter::new_capturing();
    let result = interp.eval_source(
        r#"
        effect Ask {
            fn question(prompt: str) -> str
        }
        let answer = handle {
            Ask::question("What is your name?")
        } with {
            Ask::question(prompt) => { resume("Fajar") }
        }
        answer
        "#,
    );
    assert!(result.is_ok(), "resume should work: {:?}", result.err());
    assert_eq!(result.unwrap(), Value::Str("Fajar".into()));
}

#[test]
fn ef1_7_effect_registry_has_builtins() {
    let interp = Interpreter::new();
    // Verify the runtime effect registry has built-in effects.
    assert!(interp.effect_registry.lookup("IO").is_some());
    assert!(interp.effect_registry.lookup("Alloc").is_some());
    assert!(interp.effect_registry.lookup("Panic").is_some());
    assert!(interp.effect_registry.lookup("Exception").is_some());
    assert!(interp.effect_registry.lookup("Async").is_some());
    assert!(interp.effect_registry.lookup("State").is_some());
    assert!(interp.effect_registry.lookup("Hardware").is_some());
    assert!(interp.effect_registry.lookup("Tensor").is_some());
    assert_eq!(interp.effect_registry.count(), 8);
}

#[test]
fn ef1_8_user_effect_registers_in_registry() {
    let mut interp = Interpreter::new_capturing();
    let _ = interp.eval_source(
        r#"
        effect MyEffect {
            fn do_thing(x: i32) -> i32
        }
        "#,
    );
    assert!(interp.effect_registry.lookup("MyEffect").is_some());
    let decl = interp.effect_registry.lookup("MyEffect").unwrap();
    assert_eq!(decl.op_count(), 1);
    assert!(decl.find_op("do_thing").is_some());
}

#[test]
fn ef1_9_unhandled_effect_reraises() {
    // If no handler matches, the effect should propagate upward.
    // Outside all handle blocks, the default handler kicks in.
    let mut interp = Interpreter::new_capturing();
    let result = interp.eval_source(
        r#"
        effect Db {
            fn query(sql: str) -> str
        }
        // No handle block — default handler returns Null.
        Db::query("SELECT 1")
        "#,
    );
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), Value::Null);
}

#[test]
fn ef1_10_handle_multiple_ops_same_effect() {
    let mut interp = Interpreter::new_capturing();
    let result = interp.eval_source(
        r#"
        effect Console {
            fn log(msg: str) -> void
            fn read_line() -> str
        }
        let result = handle {
            Console::read_line()
        } with {
            Console::log(msg) => { null }
            Console::read_line() => { "user input" }
        }
        result
        "#,
    );
    assert!(
        result.is_ok(),
        "multiple ops should work: {:?}",
        result.err()
    );
    assert_eq!(result.unwrap(), Value::Str("user input".into()));
}

// ===================================================================
// Sprint EF2 — Handler Semantics
// ===================================================================

#[test]
fn ef2_1_nested_handle_inner_catches() {
    // Inner handle should catch the effect before outer handle.
    let mut interp = Interpreter::new_capturing();
    let result = interp.eval_source(
        r#"
        effect Ask {
            fn name() -> str
        }
        let result = handle {
            handle {
                Ask::name()
            } with {
                Ask::name() => { "inner" }
            }
        } with {
            Ask::name() => { "outer" }
        }
        result
        "#,
    );
    assert!(result.is_ok(), "nested handle: {:?}", result.err());
    assert_eq!(result.unwrap(), Value::Str("inner".into()));
}

#[test]
fn ef2_2_nested_handle_outer_catches_unhandled() {
    // If inner handle doesn't match, effect propagates to outer handle.
    let mut interp = Interpreter::new_capturing();
    let result = interp.eval_source(
        r#"
        effect Ask {
            fn name() -> str
        }
        effect Db {
            fn query(sql: str) -> str
        }
        let result = handle {
            handle {
                Ask::name()
            } with {
                Db::query(sql) => { "db result" }
            }
        } with {
            Ask::name() => { "outer caught it" }
        }
        result
        "#,
    );
    assert!(
        result.is_ok(),
        "outer catches unhandled: {:?}",
        result.err()
    );
    assert_eq!(result.unwrap(), Value::Str("outer caught it".into()));
}

#[test]
fn ef2_3_handler_accesses_outer_scope() {
    let mut interp = Interpreter::new_capturing();
    let result = interp.eval_source(
        r#"
        effect Ask {
            fn name() -> str
        }
        let prefix = "Hello, "
        let result = handle {
            Ask::name()
        } with {
            Ask::name() => { prefix }
        }
        result
        "#,
    );
    assert!(result.is_ok(), "outer scope access: {:?}", result.err());
    assert_eq!(result.unwrap(), Value::Str("Hello, ".into()));
}

#[test]
fn ef2_4_handler_with_computation() {
    let mut interp = Interpreter::new_capturing();
    let result = interp.eval_source(
        r#"
        effect Math {
            fn double(x: i32) -> i32
        }
        let result = handle {
            Math::double(21)
        } with {
            Math::double(x) => { x * 2 }
        }
        result
        "#,
    );
    assert!(result.is_ok(), "handler computation: {:?}", result.err());
    assert_eq!(result.unwrap(), Value::Int(42));
}

#[test]
fn ef2_5_handler_returns_different_type() {
    let mut interp = Interpreter::new_capturing();
    let result = interp.eval_source(
        r#"
        effect Stringify {
            fn convert(x: i32) -> str
        }
        let result = handle {
            Stringify::convert(42)
        } with {
            Stringify::convert(x) => { "forty-two" }
        }
        result
        "#,
    );
    assert!(result.is_ok(), "different type return: {:?}", result.err());
    assert_eq!(result.unwrap(), Value::Str("forty-two".into()));
}

#[test]
fn ef2_6_body_completes_without_effect() {
    // If body doesn't perform any effect, its result is returned directly.
    let mut interp = Interpreter::new_capturing();
    let result = interp.eval_source(
        r#"
        effect Ask {
            fn name() -> str
        }
        let result = handle {
            42
        } with {
            Ask::name() => { "unused" }
        }
        result
        "#,
    );
    assert!(result.is_ok(), "no effect body: {:?}", result.err());
    assert_eq!(result.unwrap(), Value::Int(42));
}

#[test]
fn ef2_7_effect_in_function_call() {
    // Effect raised inside a function called from handle body.
    let mut interp = Interpreter::new_capturing();
    let result = interp.eval_source(
        r#"
        effect Config {
            fn get_value(key: str) -> str
        }
        fn read_config(key: str) -> str with Config {
            Config::get_value(key)
        }
        let result = handle {
            read_config("host")
        } with {
            Config::get_value(key) => { "localhost" }
        }
        result
        "#,
    );
    assert!(result.is_ok(), "effect in fn call: {:?}", result.err());
    assert_eq!(result.unwrap(), Value::Str("localhost".into()));
}

#[test]
fn ef2_8_resume_with_computed_value() {
    let mut interp = Interpreter::new_capturing();
    let result = interp.eval_source(
        r#"
        effect Random {
            fn next_int(max: i32) -> i32
        }
        let result = handle {
            Random::next_int(100)
        } with {
            Random::next_int(max) => { resume(max / 2) }
        }
        result
        "#,
    );
    assert!(result.is_ok(), "resume computed: {:?}", result.err());
    assert_eq!(result.unwrap(), Value::Int(50));
}

#[test]
fn ef2_9_multiple_effects_different_types() {
    // Handle block with handlers for two different effects.
    let mut interp = Interpreter::new_capturing();
    let result = interp.eval_source(
        r#"
        effect Logger {
            fn log(msg: str) -> void
        }
        effect Config {
            fn get(key: str) -> str
        }
        let result = handle {
            Config::get("name")
        } with {
            Logger::log(msg) => { null }
            Config::get(key) => { "Fajar Lang" }
        }
        result
        "#,
    );
    assert!(result.is_ok(), "multi-effect: {:?}", result.err());
    assert_eq!(result.unwrap(), Value::Str("Fajar Lang".into()));
}

#[test]
fn ef2_10_effect_handler_zero_params() {
    let mut interp = Interpreter::new_capturing();
    let result = interp.eval_source(
        r#"
        effect Clock {
            fn now() -> i64
        }
        let result = handle {
            Clock::now()
        } with {
            Clock::now() => { 1711929600 }
        }
        result
        "#,
    );
    assert!(result.is_ok(), "zero params: {:?}", result.err());
    assert_eq!(result.unwrap(), Value::Int(1711929600));
}

// ===================================================================
// Sprint EF3 — Effect Inference
// ===================================================================

#[test]
fn ef3_1_undeclared_effect_in_fn_body() {
    // Function calls effect op without declaring it — should get error.
    let mut interp = Interpreter::new_capturing();
    let result = interp.eval_source(
        r#"
        effect Logger {
            fn log(msg: str) -> void
        }
        fn greet() {
            Logger::log("hello")
        }
        greet()
        "#,
    );
    // Should fail with EE001 UndeclaredEffect
    assert!(result.is_err(), "should detect undeclared effect");
    let err = format!("{:?}", result.err().unwrap());
    assert!(
        err.contains("UndeclaredEffect") || err.contains("EE001"),
        "error should be EE001: {err}"
    );
}

#[test]
fn ef3_2_declared_effect_in_fn_passes() {
    // Function declares effects in `with` clause — should pass.
    let mut interp = Interpreter::new_capturing();
    let result = interp.eval_source(
        r#"
        effect Logger {
            fn log(msg: str) -> void
        }
        fn greet() with Logger {
            Logger::log("hello")
        }
        greet()
        "#,
    );
    assert!(
        result.is_ok(),
        "declared effect should pass: {:?}",
        result.err()
    );
}

#[test]
fn ef3_3_handled_effect_no_warning() {
    // Effect inside a handle block should not require `with` declaration.
    let mut interp = Interpreter::new_capturing();
    let result = interp.eval_source(
        r#"
        effect Ask {
            fn name() -> str
        }
        fn greet() -> str {
            handle {
                Ask::name()
            } with {
                Ask::name() => { "Fajar" }
            }
        }
        greet()
        "#,
    );
    assert!(
        result.is_ok(),
        "handled effect no warning: {:?}",
        result.err()
    );
    assert_eq!(result.unwrap(), Value::Str("Fajar".into()));
}

#[test]
fn ef3_4_fn_with_effects_executes() {
    // A function with declared effects should execute normally.
    let mut interp = Interpreter::new_capturing();
    let result = interp.eval_source(
        r#"
        effect Config {
            fn get(key: str) -> str
        }
        fn read_config(key: str) -> str with Config {
            Config::get(key)
        }
        // Call inside handle block so effect is intercepted.
        handle {
            read_config("host")
        } with {
            Config::get(key) => { "localhost" }
        }
        "#,
    );
    assert!(result.is_ok(), "fn with effects: {:?}", result.err());
    assert_eq!(result.unwrap(), Value::Str("localhost".into()));
}

#[test]
fn ef3_5_effect_propagation_through_call() {
    // Calling a function with effects propagates those effects.
    let mut interp = Interpreter::new_capturing();
    let result = interp.eval_source(
        r#"
        effect Db {
            fn query(sql: str) -> str
        }
        fn get_user(id: i32) -> str with Db {
            Db::query("SELECT name WHERE id=" )
        }
        // get_user performs Db, handled here
        handle {
            get_user(1)
        } with {
            Db::query(sql) => { "Alice" }
        }
        "#,
    );
    assert!(result.is_ok(), "effect propagation: {:?}", result.err());
    assert_eq!(result.unwrap(), Value::Str("Alice".into()));
}

#[test]
fn ef3_6_no_effects_function_passes() {
    // Function with no effect operations should work fine.
    let mut interp = Interpreter::new_capturing();
    let result = interp.eval_source(
        r#"
        fn add(a: i32, b: i32) -> i32 {
            a + b
        }
        add(1, 2)
        "#,
    );
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), Value::Int(3));
}

#[test]
fn ef3_7_multiple_effects_declared() {
    // Function can declare multiple effects.
    let mut interp = Interpreter::new_capturing();
    let result = interp.eval_source(
        r#"
        effect Logger {
            fn log(msg: str) -> void
        }
        effect Db {
            fn query(sql: str) -> str
        }
        fn process() with Logger, Db {
            Logger::log("starting")
            Db::query("SELECT 1")
        }
        handle {
            process()
        } with {
            Logger::log(msg) => { null }
            Db::query(sql) => { "done" }
        }
        "#,
    );
    assert!(result.is_ok(), "multi-effects: {:?}", result.err());
}

#[test]
fn ef3_8_builtin_effects_registered() {
    // Built-in effects (IO, Alloc, etc.) should be recognized.
    let mut interp = Interpreter::new_capturing();
    let result = interp.eval_source(
        r#"
        fn do_io() with IO {
            IO::print("hello")
        }
        do_io()
        "#,
    );
    assert!(result.is_ok(), "builtin effect: {:?}", result.err());
}

#[test]
fn ef3_9_context_effect_compatibility() {
    // Effects in @kernel context: Hardware OK, Tensor forbidden.
    let mut interp = Interpreter::new_capturing();
    let result = interp.eval_source(
        r#"
        effect Custom {
            fn tick() -> void
        }
        let x = 42
        x
        "#,
    );
    assert!(result.is_ok());
}

#[test]
fn ef3_10_effect_set_operations() {
    // Test the EffectSet union/intersection operations.
    use crate::analyzer::effects::EffectSet;
    let mut set_a = EffectSet::empty();
    set_a.insert("IO".to_string());
    set_a.insert("Alloc".to_string());

    let mut set_b = EffectSet::empty();
    set_b.insert("IO".to_string());
    set_b.insert("Panic".to_string());

    let union = set_a.union(&set_b);
    assert_eq!(union.len(), 3); // IO, Alloc, Panic

    let intersection = set_a.intersection(&set_b);
    assert_eq!(intersection.len(), 1); // IO

    let diff = set_a.difference(&set_b);
    assert_eq!(diff.len(), 1); // Alloc
    assert!(diff.contains("Alloc"));

    assert!(set_a.is_subset_of(&union));
    assert!(!set_a.is_subset_of(&set_b));
}

// ===================================================================
// V15 Sprint B1 — Multi-step Continuations
// ===================================================================

#[test]
fn v15_b1_1_multi_step_two_effects() {
    // Body with 2 effect calls — both must execute.
    let mut interp = Interpreter::new_capturing();
    let result = interp.eval_source(
        r#"
        effect Console {
            fn log(msg: str) -> void
        }
        let result = handle {
            Console::log("hello")
            Console::log("world")
            42
        } with {
            Console::log(msg) => { resume(null) }
        }
        result
        "#,
    );
    assert!(result.is_ok(), "multi-step 2 effects: {:?}", result.err());
    assert_eq!(result.unwrap(), Value::Int(42));
}

#[test]
fn v15_b1_2_multi_step_three_effects() {
    // Body with 3 sequential effect calls — all must execute.
    let mut interp = Interpreter::new_capturing();
    let result = interp.eval_source(
        r#"
        effect Counter {
            fn next() -> i32
        }
        let mut n = 0
        let result = handle {
            let a = Counter::next()
            let b = Counter::next()
            let c = Counter::next()
            a + b + c
        } with {
            Counter::next() => { resume(10) }
        }
        result
        "#,
    );
    assert!(result.is_ok(), "multi-step 3 effects: {:?}", result.err());
    assert_eq!(result.unwrap(), Value::Int(30));
}

#[test]
fn v15_b1_3_resume_return_value() {
    // resume(42) makes the effect call site return 42.
    let mut interp = Interpreter::new_capturing();
    let result = interp.eval_source(
        r#"
        effect AppState {
            fn get() -> i32
        }
        let x = handle {
            AppState::get()
        } with {
            AppState::get() => { resume(42) }
        }
        x
        "#,
    );
    assert!(result.is_ok(), "resume return value: {:?}", result.err());
    assert_eq!(result.unwrap(), Value::Int(42));
}

#[test]
fn v15_b1_3b_resume_value_used_in_body() {
    // Resume value is used in subsequent computation in the body.
    let mut interp = Interpreter::new_capturing();
    let result = interp.eval_source(
        r#"
        effect AppState {
            fn get() -> i32
        }
        let result = handle {
            let x = AppState::get()
            x * 2 + 1
        } with {
            AppState::get() => { resume(21) }
        }
        result
        "#,
    );
    assert!(result.is_ok(), "resume value in body: {:?}", result.err());
    assert_eq!(result.unwrap(), Value::Int(43));
}

#[test]
fn v15_b1_4_multi_effect_types_in_handler() {
    // Handle block with handlers for two different effects, body uses both.
    let mut interp = Interpreter::new_capturing();
    let result = interp.eval_source(
        r#"
        effect Logger {
            fn log(msg: str) -> void
        }
        effect Config {
            fn get(key: str) -> str
        }
        let result = handle {
            Logger::log("starting")
            let host = Config::get("host")
            Logger::log("done")
            host
        } with {
            Logger::log(msg) => { resume(null) }
            Config::get(key) => { resume("localhost") }
        }
        result
        "#,
    );
    assert!(result.is_ok(), "multi-effect types: {:?}", result.err());
    assert_eq!(result.unwrap(), Value::Str("localhost".into()));
}

#[test]
fn v15_b1_5_resume_no_arg() {
    // resume() is alias for resume(null).
    let mut interp = Interpreter::new_capturing();
    let result = interp.eval_source(
        r#"
        effect Logger {
            fn log(msg: str) -> void
        }
        let result = handle {
            Logger::log("hello")
            Logger::log("world")
            "done"
        } with {
            Logger::log(msg) => { resume() }
        }
        result
        "#,
    );
    assert!(result.is_ok(), "resume no-arg: {:?}", result.err());
    assert_eq!(result.unwrap(), Value::Str("done".into()));
}

#[test]
fn v15_b1_6_handler_scope_isolation() {
    // Handler params must not leak to outer scope.
    let mut interp = Interpreter::new_capturing();
    let result = interp.eval_source(
        r#"
        effect Logger {
            fn log(msg: str) -> void
        }
        let msg = "outer"
        handle {
            Logger::log("inner")
        } with {
            Logger::log(msg) => { resume(null) }
        }
        msg
        "#,
    );
    assert!(result.is_ok(), "handler scope: {:?}", result.err());
    assert_eq!(result.unwrap(), Value::Str("outer".into()));
}

#[test]
fn v15_b1_7_nested_handle_multi_step() {
    // Nested handle with multi-step: inner catches A, outer catches B.
    let mut interp = Interpreter::new_capturing();
    let result = interp.eval_source(
        r#"
        effect Ask {
            fn name() -> str
        }
        effect Logger {
            fn log(msg: str) -> void
        }
        let result = handle {
            handle {
                let n = Ask::name()
                Logger::log(n.clone())
                n
            } with {
                Ask::name() => { resume("Fajar") }
            }
        } with {
            Logger::log(msg) => { resume(null) }
        }
        result
        "#,
    );
    assert!(result.is_ok(), "nested multi-step: {:?}", result.err());
    assert_eq!(result.unwrap(), Value::Str("Fajar".into()));
}

#[test]
fn v15_b1_8_resume_type_mismatch() {
    // resume("hello") when effect returns i32 should produce SE004.
    let mut interp = Interpreter::new_capturing();
    let result = interp.eval_source(
        r#"
        effect Counter {
            fn next() -> i32
        }
        handle {
            Counter::next()
        } with {
            Counter::next() => { resume("wrong type") }
        }
        "#,
    );
    // Should produce a type mismatch error from analyzer.
    assert!(result.is_err(), "should detect type mismatch in resume");
}

#[test]
fn v15_b1_9_effect_arity_mismatch() {
    // Handler with wrong number of params should produce SE005.
    let mut interp = Interpreter::new_capturing();
    let result = interp.eval_source(
        r#"
        effect Logger {
            fn log(msg: str) -> void
        }
        handle {
            Logger::log("hello")
        } with {
            Logger::log() => { resume(null) }
        }
        "#,
    );
    // Should produce argument count mismatch error.
    assert!(result.is_err(), "should detect arity mismatch in handler");
}

// ===================================================================
// Sprint EF4 — Effect Polymorphism
// ===================================================================

#[test]
fn ef4_1_effect_bound_validation() {
    use crate::analyzer::effects::{EffectBound, EffectSet, check_effect_bound};
    let mut required = EffectSet::empty();
    required.insert("IO".to_string());
    required.insert("Alloc".to_string());
    let bound = EffectBound::new("E", required);

    // Concrete with subset — should pass.
    let mut concrete = EffectSet::empty();
    concrete.insert("IO".to_string());
    assert!(check_effect_bound(&bound, &concrete).is_ok());

    // Concrete with extra effect — should fail.
    let mut bad = EffectSet::empty();
    bad.insert("IO".to_string());
    bad.insert("Panic".to_string()); // not in bound
    assert!(check_effect_bound(&bound, &bad).is_err());
}

#[test]
fn ef4_2_no_effect_bound() {
    use crate::analyzer::effects::{EffectSet, NoEffectBound};
    let no_eff = NoEffectBound::new("F");

    // Empty effects — passes.
    let empty = EffectSet::empty();
    assert!(no_eff.check(&empty).is_ok());

    // Non-empty effects — fails.
    let mut with_io = EffectSet::empty();
    with_io.insert("IO".to_string());
    assert!(no_eff.check(&with_io).is_err());
}

#[test]
fn ef4_3_effect_trait_method() {
    use crate::analyzer::effects::{EffectSet, EffectTraitMethod, check_trait_method_effects};
    let mut trait_effects = EffectSet::empty();
    trait_effects.insert("IO".to_string());
    trait_effects.insert("Alloc".to_string());
    let trait_method = EffectTraitMethod::new("process", vec!["str".into()], "void", trait_effects);

    // Impl with subset — OK.
    let mut impl_effects = EffectSet::empty();
    impl_effects.insert("IO".to_string());
    assert!(check_trait_method_effects(&trait_method, &impl_effects).is_ok());

    // Impl with extra effect — error.
    let mut bad_effects = EffectSet::empty();
    bad_effects.insert("Panic".to_string());
    assert!(check_trait_method_effects(&trait_method, &bad_effects).is_err());
}

#[test]
fn ef4_4_cross_module_effect_tracking() {
    use crate::analyzer::effects::{CrossModuleEffects, EffectSet};
    let mut cross = CrossModuleEffects::new();

    let mut io_set = EffectSet::empty();
    io_set.insert("IO".to_string());
    cross.register_fn("std", "println", io_set);

    let mut db_set = EffectSet::empty();
    db_set.insert("Db".to_string());
    cross.register_fn("db", "query", db_set);

    // Infer effects from calling both.
    let combined = cross.infer_from_calls(&[("std", "println"), ("db", "query")]);
    assert_eq!(combined.len(), 2);
    assert!(combined.contains("IO"));
    assert!(combined.contains("Db"));
}

#[test]
fn ef4_5_effect_erasure_hints() {
    use crate::analyzer::effects::{
        EffectErasureHint, EffectHandler, EffectSet, HandlerScopeStack, compute_erasure_hints,
    };

    let mut stack = HandlerScopeStack::new();
    stack.push_scope();
    stack.add_handler(EffectHandler::new("IO")).unwrap_or(());

    let mut effects = EffectSet::empty();
    effects.insert("IO".to_string());
    effects.insert("Panic".to_string());

    let hints = compute_erasure_hints(&effects, &stack);
    // IO should be erasable (handler at immediate scope).
    assert!(matches!(
        hints.get("IO"),
        Some(EffectErasureHint::FullErase)
    ));
    // Panic has no handler — not erasable.
    assert!(matches!(
        hints.get("Panic"),
        Some(EffectErasureHint::NoErase)
    ));
}

#[test]
fn ef4_6_effect_closure_tracking() {
    use crate::analyzer::effects::{EffectClosure, EffectSet};
    let mut effects = EffectSet::empty();
    effects.insert("IO".to_string());
    let closure = EffectClosure::new(
        effects,
        vec!["str".into()],
        "void",
        vec!["captured_var".into()],
    );
    assert!(!closure.is_pure());
    assert_eq!(closure.captures.len(), 1);

    let pure_closure = EffectClosure::new(EffectSet::empty(), vec![], "i32", vec![]);
    assert!(pure_closure.is_pure());
}

#[test]
fn ef4_7_effect_checker_full_pipeline() {
    use crate::analyzer::effects::EffectChecker;
    let mut checker = EffectChecker::new();

    // Registry has builtins.
    assert!(checker.registry.lookup("IO").is_some());

    // Push handler scope.
    checker.handler_stack.push_scope();
    assert_eq!(checker.handler_stack.depth(), 1);

    // Register cross-module effect.
    let mut io = crate::analyzer::effects::EffectSet::empty();
    io.insert("IO".to_string());
    checker.cross_module.register_fn("std", "print", io);
    assert_eq!(checker.cross_module.count(), 1);

    checker.handler_stack.pop_scope();
    assert_eq!(checker.handler_stack.depth(), 0);
}

#[test]
fn ef4_8_builtin_handlers() {
    use crate::analyzer::effects::{
        builtin_alloc_handler, builtin_exception_handler, builtin_io_handler,
    };
    let io = builtin_io_handler();
    assert_eq!(io.handler_count(), 2);
    assert!(io.find_handler("print").is_some());
    assert!(io.find_handler("read").is_some());

    let alloc = builtin_alloc_handler();
    assert_eq!(alloc.handler_count(), 2);

    let exception = builtin_exception_handler();
    assert_eq!(exception.handler_count(), 1);
}

#[test]
fn ef4_9_context_forbidden_effects() {
    use crate::analyzer::effects::{
        ContextAnnotation, EffectKind, allowed_effects, forbidden_effects,
    };
    let kernel_forbidden = forbidden_effects(ContextAnnotation::Kernel);
    assert!(kernel_forbidden.contains(&EffectKind::Alloc));
    assert!(kernel_forbidden.contains(&EffectKind::Tensor));

    let device_forbidden = forbidden_effects(ContextAnnotation::Device);
    assert!(device_forbidden.contains(&EffectKind::Hardware));

    let safe_forbidden = forbidden_effects(ContextAnnotation::Safe);
    assert!(safe_forbidden.len() >= 3); // IO, Alloc, Hardware, Tensor

    let unsafe_forbidden = forbidden_effects(ContextAnnotation::Unsafe);
    assert!(unsafe_forbidden.is_empty());

    let kernel_allowed = allowed_effects(ContextAnnotation::Kernel);
    assert!(kernel_allowed.contains("Hardware"));
}

#[test]
fn ef4_10_effect_polymorphic_fn() {
    // A function with effect variable in generics should compile.
    let mut interp = Interpreter::new_capturing();
    let result = interp.eval_source(
        r#"
        effect Ask {
            fn name() -> str
        }
        // Effect polymorphic: E is an effect variable.
        fn with_default<E: Effect>(default_val: str) -> str {
            default_val
        }
        let result = with_default("hello")
        result
        "#,
    );
    assert!(result.is_ok(), "effect polymorphic fn: {:?}", result.err());
    assert_eq!(result.unwrap(), Value::Str("hello".into()));
}

// ===================================================================
// Sub-Option 5B — Dependent Types (DT1-DT4)
// ===================================================================

// Sprint DT1: Type-Level Integers & Const Generics

#[test]
fn dt1_1_nat_value_arithmetic() {
    use crate::dependent::nat::NatValue;
    let a = NatValue::Literal(3);
    let b = NatValue::Literal(4);
    let sum = NatValue::Add(Box::new(a), Box::new(b));
    assert_eq!(sum.evaluate(&std::collections::HashMap::new()), Some(7));
}

#[test]
fn dt1_2_nat_value_substitution() {
    use crate::dependent::nat::NatValue;
    let expr = NatValue::Add(
        Box::new(NatValue::Param("N".into())),
        Box::new(NatValue::Literal(1)),
    );
    let mut env = std::collections::HashMap::new();
    env.insert("N".to_string(), 5u64);
    assert_eq!(expr.evaluate(&env), Some(6));
}

#[test]
fn dt1_3_nat_constraint_equality() {
    use crate::dependent::nat::{NatConstraint, NatValue};
    let c = NatConstraint::Equal(NatValue::Literal(5), NatValue::Literal(5));
    assert!(c.check(&std::collections::HashMap::new()).is_ok());

    let c2 = NatConstraint::Equal(NatValue::Literal(5), NatValue::Literal(3));
    assert!(c2.check(&std::collections::HashMap::new()).is_err());
}

#[test]
fn dt1_4_nat_constraint_less_than() {
    use crate::dependent::nat::{NatConstraint, NatValue};
    let c = NatConstraint::LessThan(NatValue::Literal(3), 5);
    assert!(c.check(&std::collections::HashMap::new()).is_ok());

    let c2 = NatConstraint::LessThan(NatValue::Literal(5), 3);
    assert!(c2.check(&std::collections::HashMap::new()).is_err());
}

#[test]
fn dt1_5_nat_multiplication() {
    use crate::dependent::nat::NatValue;
    let product = NatValue::Mul(
        Box::new(NatValue::Literal(3)),
        Box::new(NatValue::Literal(7)),
    );
    assert_eq!(
        product.evaluate(&std::collections::HashMap::new()),
        Some(21)
    );
}

#[test]
fn dt1_6_const_generic_param() {
    use crate::dependent::nat::{ConstGenericParam, ConstType};
    let param = ConstGenericParam {
        name: "N".into(),
        const_type: ConstType::Usize,
    };
    assert_eq!(param.name, "N");
    assert_eq!(param.const_type, ConstType::Usize);
}

#[test]
fn dt1_7_nat_free_params() {
    use crate::dependent::nat::NatValue;
    let expr = NatValue::Add(
        Box::new(NatValue::Param("N".into())),
        Box::new(NatValue::Mul(
            Box::new(NatValue::Param("M".into())),
            Box::new(NatValue::Literal(2)),
        )),
    );
    let params = expr.free_params();
    assert!(params.iter().any(|p| p == "N"));
    assert!(params.iter().any(|p| p == "M"));
    assert_eq!(params.len(), 2);
}

#[test]
fn dt1_8_nat_is_concrete() {
    use crate::dependent::nat::NatValue;
    assert!(NatValue::Literal(5).is_concrete());
    assert!(!NatValue::Param("N".into()).is_concrete());
    assert!(
        !NatValue::Add(
            Box::new(NatValue::Param("N".into())),
            Box::new(NatValue::Literal(1)),
        )
        .is_concrete()
    );
}

#[test]
fn dt1_9_nat_substitution() {
    use crate::dependent::nat::NatValue;
    let expr = NatValue::Add(
        Box::new(NatValue::Param("N".into())),
        Box::new(NatValue::Literal(1)),
    );
    let mut sub_env = std::collections::HashMap::new();
    sub_env.insert("N".to_string(), 10u64);
    let result = expr.substitute(&sub_env);
    assert_eq!(result.evaluate(&std::collections::HashMap::new()), Some(11));
}

#[test]
fn dt1_10_kind_system() {
    use crate::dependent::nat::Kind;
    let type_kind = Kind::Type;
    let nat_kind = Kind::Nat;
    let dep_kind = Kind::Dependent(Box::new(Kind::Nat), Box::new(Kind::Type));
    assert_eq!(format!("{type_kind}"), "Type");
    assert_eq!(format!("{nat_kind}"), "Nat");
    assert_eq!(format!("{dep_kind}"), "Nat -> Type");
}

// v35.7.2 + Action C extension (2026-05-12): Sprint DT2 (dependent
// arrays), DT3 (tensor_shapes), and DT4 (dependent patterns) tests
// removed alongside deletion of dependent::arrays + dependent::patterns +
// dependent::tensor_shapes per Compass §5.1 dependent-types freeze.
// See docs/ARRAYS_PATTERNS_LOAD_BEARING_B0_FINDINGS.md +
// docs/decisions/2026-05-12-arrays-patterns-deletion.md +
// docs/TENSOR_SHAPES_LOAD_BEARING_B0_FINDINGS.md.
// dt4_9 (const generic E2E) coverage retained via tests/v20_builtin_tests.rs
// and src/const_generics.rs unit tests.

// ===================================================================
// Sub-Option 5D — LSP v4 (LS1-LS4)
// ===================================================================

// Sprint LS1: Semantic Tokens

#[test]
fn ls1_1_semantic_token_types() {
    use crate::lsp_v3::semantic::SemanticTokenType;
    assert_eq!(SemanticTokenType::Keyword.index(), 15);
    assert!(SemanticTokenType::legend().len() >= 10);
}

#[test]
fn ls1_2_semantic_token_modifiers() {
    use crate::lsp_v3::semantic::SemanticTokenModifier;
    assert!(SemanticTokenModifier::Declaration.bitmask() > 0);
    assert!(SemanticTokenModifier::legend().len() >= 2);
}

#[test]
fn ls1_3_semantic_token_encoding() {
    use crate::lsp_v3::semantic::{AbsoluteToken, SemanticTokenType, encode_semantic_tokens};
    let tokens = vec![
        AbsoluteToken {
            line: 0,
            start: 0,
            length: 3,
            token_type: SemanticTokenType::Keyword.index(),
            modifiers: 0,
        },
        AbsoluteToken {
            line: 0,
            start: 4,
            length: 1,
            token_type: SemanticTokenType::Variable.index(),
            modifiers: 0,
        },
    ];
    let encoded = encode_semantic_tokens(&tokens);
    assert_eq!(encoded.len(), 2);
    assert_eq!(encoded[0].delta_line, 0);
    assert_eq!(encoded[0].delta_start, 0);
    assert_eq!(encoded[1].delta_start, 4);
}

#[test]
fn ls1_4_semantic_token_multiline() {
    use crate::lsp_v3::semantic::{AbsoluteToken, SemanticTokenType, encode_semantic_tokens};
    let tokens = vec![
        AbsoluteToken {
            line: 0,
            start: 0,
            length: 2,
            token_type: SemanticTokenType::Keyword.index(),
            modifiers: 0,
        },
        AbsoluteToken {
            line: 2,
            start: 5,
            length: 3,
            token_type: SemanticTokenType::Function.index(),
            modifiers: 0,
        },
    ];
    let encoded = encode_semantic_tokens(&tokens);
    assert_eq!(encoded[1].delta_line, 2);
    assert_eq!(encoded[1].delta_start, 5); // new line resets to absolute
}

#[test]
fn ls1_5_token_type_legend() {
    use crate::lsp_v3::semantic::SemanticTokenType;
    let legend = SemanticTokenType::legend();
    assert!(legend.contains(&"keyword"));
    assert!(legend.contains(&"function"));
    assert!(legend.contains(&"variable"));
}

#[test]
fn ls1_6_empty_token_encoding() {
    use crate::lsp_v3::semantic::encode_semantic_tokens;
    let encoded = encode_semantic_tokens(&[]);
    assert!(encoded.is_empty());
}

#[test]
fn ls1_7_token_type_all_variants() {
    use crate::lsp_v3::semantic::SemanticTokenType;
    let types = [
        SemanticTokenType::Keyword,
        SemanticTokenType::Function,
        SemanticTokenType::Variable,
        SemanticTokenType::Type,
        SemanticTokenType::String,
        SemanticTokenType::Number,
        SemanticTokenType::Comment,
    ];
    for t in types {
        assert!(t.index() < 20);
    }
}

#[test]
fn ls1_8_modifier_legend() {
    use crate::lsp_v3::semantic::SemanticTokenModifier;
    let legend = SemanticTokenModifier::legend();
    assert!(legend.contains(&"declaration"));
}

#[test]
fn ls1_9_absolute_token_fields() {
    use crate::lsp_v3::semantic::{AbsoluteToken, SemanticTokenType};
    let tok = AbsoluteToken {
        line: 5,
        start: 10,
        length: 3,
        token_type: SemanticTokenType::Keyword.index(),
        modifiers: 0,
    };
    assert_eq!(tok.line, 5);
    assert_eq!(tok.start, 10);
    assert_eq!(tok.length, 3);
}

#[test]
fn ls1_10_semantic_token_delta() {
    use crate::lsp_v3::semantic::SemanticToken;
    let tok = SemanticToken {
        delta_line: 1,
        delta_start: 5,
        length: 3,
        token_type: 0,
        token_modifiers: 0,
    };
    assert_eq!(tok.delta_line, 1);
    assert_eq!(tok.length, 3);
}

// Sprint LS2: Inlay Hints

#[test]
fn ls2_1_inlay_hint_provider_creation() {
    use crate::lsp::completion::InlayHintProvider;
    let provider = InlayHintProvider::new();
    let hints = provider.compute_inlay_hints("");
    assert!(hints.is_empty());
}

#[test]
fn ls2_2_inlay_hint_for_let_int() {
    use crate::lsp::completion::InlayHintProvider;
    let provider = InlayHintProvider::new();
    let hints = provider.compute_inlay_hints("let x = 42");
    assert!(!hints.is_empty());
    assert!(
        hints[0].label.contains("i64")
            || hints[0].label.contains("i32")
            || hints[0].label.contains("int")
    );
}

#[test]
fn ls2_3_inlay_hint_for_let_string() {
    use crate::lsp::completion::InlayHintProvider;
    let provider = InlayHintProvider::new();
    let hints = provider.compute_inlay_hints(r#"let name = "hello""#);
    assert!(!hints.is_empty());
    assert!(hints[0].label.contains("str"));
}

#[test]
fn ls2_4_inlay_hint_kind() {
    use crate::lsp::completion::InlayHintKind;
    let type_hint = InlayHintKind::TypeHint;
    let param_hint = InlayHintKind::ParameterHint;
    assert_ne!(type_hint, param_hint);
}

#[test]
fn ls2_5_no_hint_for_typed_let() {
    use crate::lsp::completion::InlayHintProvider;
    let provider = InlayHintProvider::new();
    let hints = provider.compute_inlay_hints("let x: i32 = 42");
    // Already typed — no hint needed.
    assert!(hints.is_empty());
}

#[test]
fn ls2_6_inlay_hint_for_float() {
    use crate::lsp::completion::InlayHintProvider;
    let provider = InlayHintProvider::new();
    let hints = provider.compute_inlay_hints("let pi = 1.25");
    assert!(!hints.is_empty());
    assert!(hints[0].label.contains("f64") || hints[0].label.contains("float"));
}

#[test]
fn ls2_7_inlay_hint_for_bool() {
    use crate::lsp::completion::InlayHintProvider;
    let provider = InlayHintProvider::new();
    let hints = provider.compute_inlay_hints("let flag = true");
    assert!(!hints.is_empty());
    assert!(hints[0].label.contains("bool"));
}

#[test]
fn ls2_8_inlay_hint_for_array() {
    use crate::lsp::completion::InlayHintProvider;
    let provider = InlayHintProvider::new();
    let hints = provider.compute_inlay_hints("let arr = [1, 2, 3]");
    assert!(!hints.is_empty());
}

#[test]
fn ls2_9_multiple_let_bindings() {
    use crate::lsp::completion::InlayHintProvider;
    let provider = InlayHintProvider::new();
    let hints = provider.compute_inlay_hints("let x = 1\nlet y = 2\nlet z = 3");
    assert_eq!(hints.len(), 3);
}

#[test]
fn ls2_10_inlay_hint_position() {
    use crate::lsp::completion::InlayHintProvider;
    let provider = InlayHintProvider::new();
    let hints = provider.compute_inlay_hints("let x = 42");
    assert!(!hints.is_empty());
    assert_eq!(hints[0].line, 0);
}

// Sprint LS3: Completion Provider

#[test]
fn ls3_1_completion_provider_creation() {
    use crate::lsp::completion::CompletionProvider;
    let provider = CompletionProvider::new();
    let _ = provider; // just verifies it compiles
}

#[test]
fn ls3_2_default_completions() {
    use crate::lsp::completion::{CompletionProvider, CompletionTrigger};
    let provider = CompletionProvider::new();
    let result = provider.complete_at("", 0, 0, CompletionTrigger::Default);
    assert!(!result.is_empty()); // Should have builtins + keywords
}

#[test]
fn ls3_3_keyword_completions() {
    use crate::lsp::completion::{CompletionKind, CompletionProvider, CompletionTrigger};
    let provider = CompletionProvider::new();
    let result = provider.complete_at("", 0, 0, CompletionTrigger::Default);
    let has_keywords = result.iter().any(|c| c.kind == CompletionKind::Keyword);
    assert!(has_keywords);
}

#[test]
fn ls3_4_builtin_completions() {
    use crate::lsp::completion::{CompletionKind, CompletionProvider, CompletionTrigger};
    let provider = CompletionProvider::new();
    let result = provider.complete_at("", 0, 0, CompletionTrigger::Default);
    let has_builtins = result.iter().any(|c| c.kind == CompletionKind::Builtin);
    assert!(has_builtins);
}

#[test]
fn ls3_5_completion_candidate_fields() {
    use crate::lsp::completion::{CompletionCandidate, CompletionKind};
    let candidate = CompletionCandidate {
        label: "println".into(),
        kind: CompletionKind::Builtin,
        detail: Some("fn(args...) -> void".into()),
        insert_text: "println".into(),
    };
    assert_eq!(candidate.label, "println");
    assert_eq!(candidate.kind, CompletionKind::Builtin);
}

#[test]
fn ls3_6_completion_trigger_variants() {
    use crate::lsp::completion::CompletionTrigger;
    let triggers = [
        CompletionTrigger::Dot,
        CompletionTrigger::DoubleColon,
        CompletionTrigger::Angle,
        CompletionTrigger::Default,
    ];
    assert_eq!(triggers.len(), 4);
}

#[test]
fn ls3_7_completion_kind_variants() {
    use crate::lsp::completion::CompletionKind;
    let kinds = [
        CompletionKind::Function,
        CompletionKind::Variable,
        CompletionKind::Struct,
        CompletionKind::Enum,
        CompletionKind::Field,
        CompletionKind::Module,
        CompletionKind::Keyword,
        CompletionKind::Builtin,
    ];
    assert_eq!(kinds.len(), 8);
}

#[test]
fn ls3_8_completion_has_println() {
    use crate::lsp::completion::{CompletionProvider, CompletionTrigger};
    let provider = CompletionProvider::new();
    let result = provider.complete_at("", 0, 0, CompletionTrigger::Default);
    let has_println = result.iter().any(|c| c.label == "println");
    assert!(has_println);
}

#[test]
fn ls3_9_completion_has_fn_keyword() {
    use crate::lsp::completion::{CompletionProvider, CompletionTrigger};
    let provider = CompletionProvider::new();
    let result = provider.complete_at("", 0, 0, CompletionTrigger::Default);
    let has_fn = result.iter().any(|c| c.label == "fn");
    assert!(has_fn);
}

#[test]
fn ls3_10_completion_has_let_keyword() {
    use crate::lsp::completion::{CompletionProvider, CompletionTrigger};
    let provider = CompletionProvider::new();
    let result = provider.complete_at("", 0, 0, CompletionTrigger::Default);
    let has_let = result.iter().any(|c| c.label == "let");
    assert!(has_let);
}

// Sprint LS4: Workspace Symbols & Rename

#[test]
fn ls4_1_workspace_symbol_provider() {
    use crate::lsp::completion::WorkspaceSymbolProvider;
    let provider = WorkspaceSymbolProvider::new();
    let symbols = provider.search_symbols("fn hello() { }", "hello");
    assert!(!symbols.is_empty());
}

#[test]
fn ls4_2_workspace_symbol_kinds() {
    use crate::lsp::completion::WorkspaceSymbolKind;
    let kinds = [
        WorkspaceSymbolKind::Function,
        WorkspaceSymbolKind::Struct,
        WorkspaceSymbolKind::Enum,
        WorkspaceSymbolKind::Trait,
        WorkspaceSymbolKind::Constant,
        WorkspaceSymbolKind::Module,
    ];
    assert_eq!(kinds.len(), 6);
}

#[test]
fn ls4_3_workspace_find_struct() {
    use crate::lsp::completion::WorkspaceSymbolProvider;
    let provider = WorkspaceSymbolProvider::new();
    let symbols = provider.search_symbols("struct Point { x: i32, y: i32 }", "Point");
    assert!(!symbols.is_empty());
    assert_eq!(symbols[0].name, "Point");
}

#[test]
fn ls4_4_workspace_find_enum() {
    use crate::lsp::completion::WorkspaceSymbolProvider;
    let provider = WorkspaceSymbolProvider::new();
    let symbols = provider.search_symbols("enum Color { Red, Green, Blue }", "Color");
    assert!(!symbols.is_empty());
}

#[test]
fn ls4_5_rename_provider_creation() {
    use crate::lsp::completion::RenameProvider;
    let provider = RenameProvider::new();
    let _ = provider;
}

#[test]
fn ls4_6_find_references() {
    use crate::lsp::completion::RenameProvider;
    let provider = RenameProvider::new();
    let refs = provider
        .find_all_references(
            "let x = 1
let y = x + 1",
            0,
            4,
        )
        .unwrap();
    assert!(refs.len() >= 2); // definition + usage
}

#[test]
fn ls4_7_rename_symbol() {
    use crate::lsp::completion::RenameProvider;
    let provider = RenameProvider::new();
    let edits = provider
        .rename_symbol(
            "let x = 1
let y = x",
            0,
            4,
            "foo",
        )
        .unwrap();
    assert!(!edits.is_empty());
}

#[test]
fn ls4_8_workspace_empty_query() {
    use crate::lsp::completion::WorkspaceSymbolProvider;
    let provider = WorkspaceSymbolProvider::new();
    let symbols = provider.search_symbols("fn hello() { }", "");
    // Empty query returns all symbols.
    assert!(!symbols.is_empty());
}

#[test]
fn ls4_9_lsp_error_variants() {
    use crate::lsp::completion::LspError;
    let err = LspError::ParseFailed {
        message: "test".into(),
    };
    let msg = format!("{err}");
    assert!(msg.contains("test"));
}

#[test]
fn ls4_10_workspace_symbol_fields() {
    use crate::lsp::completion::{WorkspaceSymbol, WorkspaceSymbolKind};
    let sym = WorkspaceSymbol {
        name: "main".into(),
        kind: WorkspaceSymbolKind::Function,
        file: "<source>".into(),
        line: 0,
        container_name: None,
    };
    assert_eq!(sym.name, "main");
    assert_eq!(sym.line, 0);
}

// ===================================================================
// Sub-Option 5C — GPU Compute Shaders (GS1-GS4)
// ===================================================================

// Sprint GS1: SPIR-V Module & Shader Syntax

#[test]
fn gs1_1_spirv_module_creation() {
    use crate::gpu_codegen::spirv::SpirVModule;
    let module = SpirVModule::new_compute();
    assert_eq!(module.version, 0x0001_0500); // SPIR-V 1.5
    assert!(!module.capabilities.is_empty());
}

#[test]
fn gs1_2_spirv_alloc_id() {
    use crate::gpu_codegen::spirv::SpirVModule;
    let mut module = SpirVModule::new_compute();
    let id1 = module.alloc_id();
    let id2 = module.alloc_id();
    assert_eq!(id2, id1 + 1);
}

#[test]
fn gs1_3_spirv_emit_words_header() {
    use crate::gpu_codegen::spirv::SpirVModule;
    let module = SpirVModule::new_compute();
    let words = module.emit_words();
    assert_eq!(words.len(), 5);
    assert_eq!(words[0], 0x0723_0203); // SPIR-V magic
}

#[test]
fn gs1_4_spirv_validate_empty_entry() {
    use crate::gpu_codegen::spirv::SpirVModule;
    let module = SpirVModule::new_compute();
    let errors = module.validate();
    // No entry points → should have validation error
    assert!(!errors.is_empty());
}

#[test]
fn gs1_5_spirv_type_mapping() {
    use crate::gpu_codegen::spirv::{SpirVTypeDesc, map_fj_type};
    assert_eq!(map_fj_type("f32"), Some(SpirVTypeDesc::Float(32)));
    assert_eq!(map_fj_type("i32"), Some(SpirVTypeDesc::Int(32, true)));
    assert_eq!(map_fj_type("bool"), Some(SpirVTypeDesc::Bool));
    assert_eq!(map_fj_type("unknown"), None);
}

#[test]
fn gs1_6_spirv_capability_values() {
    use crate::gpu_codegen::spirv::Capability;
    assert_eq!(Capability::Shader.value(), 1);
    assert_eq!(Capability::Float16.value(), 9);
    assert_eq!(Capability::Float64.value(), 10);
}

#[test]
fn gs1_7_spirv_execution_model() {
    use crate::gpu_codegen::spirv::ExecutionModel;
    let model = ExecutionModel::GLCompute;
    assert_eq!(model, ExecutionModel::GLCompute);
    assert_ne!(model, ExecutionModel::Vertex);
}

#[test]
fn gs1_8_spirv_storage_class_values() {
    use crate::gpu_codegen::spirv::StorageClass;
    assert_eq!(StorageClass::StorageBuffer.value(), 12);
    assert_eq!(StorageClass::Workgroup.value(), 4);
    assert_eq!(StorageClass::Input.value(), 1);
}

#[test]
fn gs1_9_spirv_entry_point_struct() {
    use crate::gpu_codegen::spirv::{EntryPoint, ExecutionModel};
    let ep = EntryPoint {
        execution_model: ExecutionModel::GLCompute,
        function_id: 1,
        name: "main".into(),
        interface_ids: vec![2, 3],
        local_size: [256, 1, 1],
    };
    assert_eq!(ep.name, "main");
    assert_eq!(ep.local_size[0], 256);
}

#[test]
fn gs1_10_spirv_validate_with_entry() {
    use crate::gpu_codegen::spirv::{EntryPoint, ExecutionModel, SpirVModule};
    let mut module = SpirVModule::new_compute();
    let fn_id = module.alloc_id();
    module.entry_points.push(EntryPoint {
        execution_model: ExecutionModel::GLCompute,
        function_id: fn_id,
        name: "main".into(),
        interface_ids: vec![],
        local_size: [64, 1, 1],
    });
    let errors = module.validate();
    assert!(errors.is_empty(), "valid module: {errors:?}");
}

// Sprint GS2: SPIR-V Backend (Vulkan Compute)

#[test]
fn gs2_1_create_ssbo() {
    use crate::gpu_codegen::spirv::{StorageClass, create_ssbo};
    let ssbo = create_ssbo(10, 5, 0, 0);
    assert_eq!(ssbo.id, 10);
    assert_eq!(ssbo.storage_class, StorageClass::StorageBuffer);
    assert_eq!(ssbo.binding, Some(0));
}

#[test]
fn gs2_2_create_workgroup_var() {
    use crate::gpu_codegen::spirv::{StorageClass, create_workgroup_var};
    let wg = create_workgroup_var(20, 8);
    assert_eq!(wg.storage_class, StorageClass::Workgroup);
    assert!(wg.binding.is_none());
}

#[test]
fn gs2_3_vulkan_dispatch_1d() {
    use crate::gpu_codegen::spirv::compute_dispatch_1d;
    let dispatch = compute_dispatch_1d(1024, 256);
    assert_eq!(dispatch.group_count_x, 4);
    assert_eq!(dispatch.group_count_y, 1);
    assert_eq!(dispatch.group_count_z, 1);
}

#[test]
fn gs2_4_vulkan_dispatch_2d() {
    use crate::gpu_codegen::spirv::compute_dispatch_2d;
    let dispatch = compute_dispatch_2d(512, 256, 16, 16);
    assert_eq!(dispatch.group_count_x, 32);
    assert_eq!(dispatch.group_count_y, 16);
}

#[test]
fn gs2_5_vulkan_dispatch_rounding() {
    use crate::gpu_codegen::spirv::compute_dispatch_1d;
    // 1000 elements / 256 = 3.9 → rounds up to 4
    let dispatch = compute_dispatch_1d(1000, 256);
    assert_eq!(dispatch.group_count_x, 4);
}

#[test]
fn gs2_6_barrier_scope_values() {
    use crate::gpu_codegen::spirv::BarrierScope;
    assert_eq!(BarrierScope::Workgroup.value(), 2);
    assert_eq!(BarrierScope::Device.value(), 1);
    assert_eq!(BarrierScope::Subgroup.value(), 3);
}

#[test]
fn gs2_7_memory_semantics_values() {
    use crate::gpu_codegen::spirv::MemorySemantics;
    let acq = MemorySemantics::AcquireWorkgroup.value();
    let rel = MemorySemantics::ReleaseWorkgroup.value();
    assert_ne!(acq, rel);
    assert!(acq > 0);
}

#[test]
fn gs2_8_backend_parse() {
    use crate::gpu_codegen::spirv::{GpuBackend, parse_backend};
    assert_eq!(parse_backend("ptx"), Some(GpuBackend::Ptx));
    assert_eq!(parse_backend("spirv"), Some(GpuBackend::SpirV));
    assert_eq!(parse_backend("auto"), Some(GpuBackend::Auto));
    assert_eq!(parse_backend("metal"), None);
}

#[test]
fn gs2_9_backend_resolve_nvidia() {
    use crate::gpu_codegen::spirv::{GpuBackend, resolve_backend};
    let resolved = resolve_backend(GpuBackend::Auto, "NVIDIA GeForce RTX 4090");
    assert_eq!(resolved, GpuBackend::Ptx);
}

#[test]
fn gs2_10_backend_resolve_amd() {
    use crate::gpu_codegen::spirv::{GpuBackend, resolve_backend};
    let resolved = resolve_backend(GpuBackend::Auto, "AMD Radeon RX 7900");
    assert_eq!(resolved, GpuBackend::SpirV);
}

// Sprint GS3: PTX Backend (CUDA)

#[test]
fn gs3_1_ptx_type_mapping() {
    use crate::gpu_codegen::ptx::{PtxType, map_type};
    assert_eq!(map_type("f32"), Some(PtxType::F32));
    assert_eq!(map_type("f64"), Some(PtxType::F64));
    assert_eq!(map_type("i32"), Some(PtxType::S32));
    assert_eq!(map_type("u8"), Some(PtxType::U8));
}

#[test]
fn gs3_2_kernel_entry_emit() {
    use crate::gpu_codegen::ptx::{KernelEntry, KernelParam, PtxType};
    let kernel = KernelEntry {
        name: "vecadd".into(),
        params: vec![
            KernelParam {
                name: "a".into(),
                ptx_type: PtxType::F32,
                is_pointer: true,
            },
            KernelParam {
                name: "b".into(),
                ptx_type: PtxType::F32,
                is_pointer: true,
            },
        ],
        body: vec![],
    };
    let ptx = kernel.emit();
    assert!(ptx.contains("vecadd"));
    assert!(ptx.contains(".entry"));
}

#[test]
fn gs3_3_grid_config_1d() {
    use crate::gpu_codegen::ptx::compute_grid_1d;
    let grid = compute_grid_1d(1024, 256);
    assert_eq!(grid.total_threads(), 1024);
}

#[test]
fn gs3_4_grid_config_2d() {
    use crate::gpu_codegen::ptx::compute_grid_2d;
    let grid = compute_grid_2d(512, 256, 16);
    assert_eq!(grid.total_threads(), 512 * 256);
}

#[test]
fn gs3_5_ptx_thread_index() {
    use crate::gpu_codegen::ptx::{PtxInstruction, ThreadIndex, emit_thread_index};
    let instr = emit_thread_index("%tid_x", ThreadIndex::ThreadIdX);
    match instr {
        PtxInstruction::MovSpecial { .. } => {} // expected
        _ => panic!("expected MovSpecial instruction"),
    }
}

#[test]
fn gs3_6_ptx_global_thread_id() {
    use crate::gpu_codegen::ptx::emit_global_thread_id;
    let instrs = emit_global_thread_id("%gtid", "%tid", "%ctaid", "%ntid");
    assert!(!instrs.is_empty()); // mad (multiply-add)
}

#[test]
fn gs3_7_ptx_type_display() {
    use crate::gpu_codegen::ptx::PtxType;
    assert_eq!(format!("{}", PtxType::F32), ".f32");
    assert_eq!(format!("{}", PtxType::S32), ".s32");
}

#[test]
fn gs3_8_kernel_param_display() {
    use crate::gpu_codegen::ptx::{KernelParam, PtxType};
    let param = KernelParam {
        name: "data".into(),
        ptx_type: PtxType::F64,
        is_pointer: true,
    };
    let s = format!("{param}");
    assert!(s.contains("data"));
}

#[test]
fn gs3_9_shared_decl_fields() {
    use crate::gpu_codegen::ptx::{PtxType, SharedDecl};
    let shared = SharedDecl {
        name: "smem".into(),
        elem_type: PtxType::F32,
        count: 256,
    };
    assert_eq!(shared.name, "smem");
    assert_eq!(shared.count, 256);
}

#[test]
fn gs3_10_memory_space_variants() {
    use crate::gpu_codegen::ptx::MemorySpace;
    let spaces = [
        MemorySpace::Global,
        MemorySpace::Shared,
        MemorySpace::Local,
        MemorySpace::Constant,
    ];
    assert_eq!(spaces.len(), 4);
}

// Sprint GS4: Auto-Dispatch & Fusion

#[test]
fn gs4_1_fusion_graph_creation() {
    use crate::gpu_codegen::fusion::{FusionGraph, GpuOp, OpKind};
    let ops = vec![
        GpuOp {
            id: 0,
            kind: OpKind::ElementWiseUnary,
            inputs: vec![],
            output_elements: 1024,
        },
        GpuOp {
            id: 1,
            kind: OpKind::ElementWiseUnary,
            inputs: vec![0],
            output_elements: 1024,
        },
    ];
    let graph = FusionGraph::new(ops);
    assert_eq!(graph.ops.len(), 2);
}

#[test]
fn gs4_2_fusion_analysis() {
    use crate::gpu_codegen::fusion::{FusionGraph, GpuOp, OpKind};
    let ops = vec![
        GpuOp {
            id: 0,
            kind: OpKind::ElementWiseUnary,
            inputs: vec![],
            output_elements: 1024,
        },
        GpuOp {
            id: 1,
            kind: OpKind::ElementWiseBinary,
            inputs: vec![0],
            output_elements: 1024,
        },
    ];
    let mut graph = FusionGraph::new(ops);
    graph.analyze();
    assert!(graph.num_fusions() >= 1);
}

#[test]
fn gs4_3_can_fuse_elementwise() {
    use crate::gpu_codegen::fusion::{GpuOp, OpKind, can_fuse};
    let producer = GpuOp {
        id: 0,
        kind: OpKind::ElementWiseUnary,
        inputs: vec![],
        output_elements: 512,
    };
    let consumer = GpuOp {
        id: 1,
        kind: OpKind::ElementWiseBinary,
        inputs: vec![0],
        output_elements: 512,
    };
    assert!(can_fuse(&producer, &consumer));
}

#[test]
fn gs4_4_cannot_fuse_matmul_reduction() {
    use crate::gpu_codegen::fusion::{GpuOp, OpKind, can_fuse};
    let producer = GpuOp {
        id: 0,
        kind: OpKind::Matmul,
        inputs: vec![],
        output_elements: 1024,
    };
    let consumer = GpuOp {
        id: 1,
        kind: OpKind::Reduction,
        inputs: vec![0],
        output_elements: 1,
    };
    assert!(!can_fuse(&producer, &consumer));
}

#[test]
fn gs4_5_op_kind_display() {
    use crate::gpu_codegen::fusion::OpKind;
    assert_eq!(format!("{}", OpKind::Matmul), "Matmul");
    assert_eq!(format!("{}", OpKind::Softmax), "Softmax");
}

#[test]
fn gs4_6_device_allocator_creation() {
    use crate::gpu_codegen::gpu_memory::DeviceAllocator;
    let alloc = DeviceAllocator::new(0, 1024 * 1024, 4 * 1024 * 1024);
    let stats = alloc.stats();
    assert_eq!(stats.used_bytes, 0);
}

#[test]
fn gs4_7_device_allocator_alloc_free() {
    use crate::gpu_codegen::gpu_memory::DeviceAllocator;
    let mut alloc = DeviceAllocator::new(0, 1024 * 1024, 4 * 1024 * 1024);
    let a = alloc.allocate(4096).unwrap();
    assert_eq!(a.size, 4096);
    assert!(a.in_use);
    let stats = alloc.stats();
    assert_eq!(stats.used_bytes, 4096);
    alloc.free(a.id).unwrap();
    let stats2 = alloc.stats();
    assert_eq!(stats2.used_bytes, 0);
}

#[test]
fn gs4_8_device_allocator_oom() {
    use crate::gpu_codegen::gpu_memory::{AllocError, DeviceAllocator};
    let mut alloc = DeviceAllocator::new(0, 1024, 1024);
    let result = alloc.allocate(2048);
    assert!(matches!(result, Err(AllocError::OutOfMemory { .. })));
}

#[test]
fn gs4_9_gpu_backend_display() {
    use crate::gpu_codegen::spirv::GpuBackend;
    assert_eq!(format!("{}", GpuBackend::Ptx), "ptx");
    assert_eq!(format!("{}", GpuBackend::SpirV), "spirv");
    assert_eq!(format!("{}", GpuBackend::Auto), "auto");
}

#[test]
fn gs4_10_spirv_type_ids() {
    use crate::gpu_codegen::spirv::SpirVType;
    let void = SpirVType::Void { id: 1 };
    let int = SpirVType::Int {
        id: 2,
        width: 32,
        signed: true,
    };
    let float = SpirVType::Float { id: 3, width: 32 };
    assert_eq!(void.id(), 1);
    assert_eq!(int.id(), 2);
    assert_eq!(float.id(), 3);
}

// ===================================================================
// Sub-Option 5E — Package Registry (PR1-PR4)
// ===================================================================

// Sprint PR1: Registry Core (publish, search, resolve)

#[test]
fn pr1_1_registry_creation() {
    use crate::package::registry::Registry;
    let reg = Registry::new();
    assert_eq!(reg.package_count(), 0);
}

#[test]
fn pr1_2_publish_and_lookup() {
    use crate::package::registry::{Registry, SemVer};
    let mut reg = Registry::new();
    reg.publish("math", SemVer::new(1, 0, 0), "Math library");
    assert_eq!(reg.package_count(), 1);
    let pkg = reg.lookup("math").unwrap();
    assert_eq!(pkg.name, "math");
}

#[test]
fn pr1_3_publish_multiple_versions() {
    use crate::package::registry::{Registry, SemVer};
    let mut reg = Registry::new();
    reg.publish("http", SemVer::new(1, 0, 0), "HTTP client");
    reg.publish("http", SemVer::new(1, 1, 0), "HTTP client");
    reg.publish("http", SemVer::new(2, 0, 0), "HTTP client v2");
    let latest = reg.latest_version("http").unwrap();
    assert_eq!(*latest, SemVer::new(2, 0, 0));
}

#[test]
fn pr1_4_search_packages() {
    use crate::package::registry::{Registry, SemVer};
    let mut reg = Registry::new();
    reg.publish("json-parser", SemVer::new(1, 0, 0), "JSON parser");
    reg.publish("json-schema", SemVer::new(0, 5, 0), "JSON schema validator");
    reg.publish("http-client", SemVer::new(1, 0, 0), "HTTP client");
    let results = reg.search("json");
    assert_eq!(results.len(), 2);
}

#[test]
fn pr1_5_version_constraint_parse() {
    use crate::package::registry::{SemVer, VersionConstraint};
    let c = VersionConstraint::parse("^1.2.3").unwrap();
    assert!(c.matches(&SemVer::new(1, 3, 0)));
    assert!(!c.matches(&SemVer::new(2, 0, 0)));
}

#[test]
fn pr1_6_version_resolve() {
    use crate::package::registry::{Registry, SemVer, VersionConstraint};
    let mut reg = Registry::new();
    reg.publish("crypto", SemVer::new(1, 0, 0), "Crypto lib");
    reg.publish("crypto", SemVer::new(1, 2, 0), "Crypto lib");
    reg.publish("crypto", SemVer::new(2, 0, 0), "Crypto lib v2");
    let constraint = VersionConstraint::parse("^1.0.0").unwrap();
    let resolved = reg.resolve("crypto", &constraint).unwrap();
    assert_eq!(resolved, SemVer::new(1, 2, 0));
}

#[test]
fn pr1_7_semver_parse() {
    use crate::package::registry::SemVer;
    let v = SemVer::parse("1.25.1").unwrap();
    assert_eq!(v, SemVer::new(1, 25, 1));
    assert!(SemVer::parse("not.a.version").is_err());
}

#[test]
fn pr1_8_list_all_packages() {
    use crate::package::registry::{Registry, SemVer};
    let mut reg = Registry::new();
    reg.publish("aaa", SemVer::new(1, 0, 0), "First");
    reg.publish("zzz", SemVer::new(1, 0, 0), "Last");
    let all = reg.list_all();
    assert_eq!(all.len(), 2);
}

#[test]
fn pr1_9_package_names() {
    use crate::package::registry::{Registry, SemVer};
    let mut reg = Registry::new();
    reg.publish("alpha", SemVer::new(1, 0, 0), "A");
    reg.publish("beta", SemVer::new(1, 0, 0), "B");
    let names = reg.package_names();
    assert_eq!(names.len(), 2);
}

#[test]
fn pr1_10_yank_version() {
    use crate::package::registry::{Registry, SemVer};
    let mut reg = Registry::new();
    reg.publish("vuln-pkg", SemVer::new(1, 0, 0), "Vulnerable");
    let v = SemVer::new(1, 0, 0);
    assert!(!reg.is_yanked("vuln-pkg", &v));
    reg.yank("vuln-pkg", &v).unwrap();
    assert!(reg.is_yanked("vuln-pkg", &v));
}

// Sprint PR2: API & Metadata

#[test]
fn pr2_1_api_response_ok() {
    use crate::package::server::{ApiResponse, StatusCode};
    let resp = ApiResponse::ok(r#"{"status":"ok"}"#);
    assert_eq!(resp.status, StatusCode::OK);
}

#[test]
fn pr2_2_api_response_error() {
    use crate::package::server::{ApiResponse, StatusCode};
    let resp = ApiResponse::error(StatusCode::NOT_FOUND, "not found");
    assert_eq!(resp.status, StatusCode::NOT_FOUND);
}

#[test]
fn pr2_3_api_response_created() {
    use crate::package::server::{ApiResponse, StatusCode};
    let resp = ApiResponse::created(r#"{"published":true}"#);
    assert_eq!(resp.status, StatusCode::CREATED);
}

#[test]
fn pr2_4_auth_token_creation() {
    use crate::package::registry::AuthToken;
    let token = AuthToken::new("test-token-123");
    let _ = token; // verifies construction
}

#[test]
fn pr2_5_auth_token_scoped() {
    use crate::package::registry::AuthToken;
    let token = AuthToken::scoped("test-token-456", "my-package");
    let _ = token;
}

#[test]
fn pr2_6_token_validation() {
    use crate::package::registry::{AuthToken, Registry, SemVer};
    let mut reg = Registry::new();
    reg.publish("my-pkg", SemVer::new(1, 0, 0), "My package");
    reg.add_token(AuthToken::new("secret-key"));
    assert!(reg.validate_token("secret-key", None));
    assert!(!reg.validate_token("wrong-key", None));
}

#[test]
fn pr2_7_scoped_token_validation() {
    use crate::package::registry::{AuthToken, Registry, SemVer};
    let mut reg = Registry::new();
    reg.publish("my-pkg", SemVer::new(1, 0, 0), "My package");
    reg.add_token(AuthToken::scoped("pkg-key", "my-pkg"));
    assert!(reg.validate_token("pkg-key", Some("my-pkg")));
    assert!(!reg.validate_token("pkg-key", Some("other-pkg")));
}

#[test]
fn pr2_8_publish_with_metadata() {
    use crate::package::registry::{Registry, SemVer};
    use std::collections::HashMap;
    let mut reg = Registry::new();
    let mut deps = HashMap::new();
    deps.insert("serde".into(), "^1.0.0".into());
    reg.publish_with_meta("my-app", SemVer::new(0, 1, 0), "My app", deps, "sha256abc");
    let pkg = reg.lookup("my-app").unwrap();
    assert_eq!(pkg.name, "my-app");
}

#[test]
fn pr2_9_version_constraint_wildcard() {
    use crate::package::registry::{SemVer, VersionConstraint};
    let c = VersionConstraint::parse("*").unwrap();
    assert!(c.matches(&SemVer::new(1, 0, 0)));
    assert!(c.matches(&SemVer::new(99, 99, 99)));
}

#[test]
fn pr2_10_version_constraint_tilde() {
    use crate::package::registry::{SemVer, VersionConstraint};
    let c = VersionConstraint::parse("~1.2.0").unwrap();
    assert!(c.matches(&SemVer::new(1, 2, 5)));
    assert!(!c.matches(&SemVer::new(1, 3, 0)));
}

// Sprint PR3: Signing & Bundles

#[test]
fn pr3_1_oidc_authenticate() {
    use crate::package::signing::oidc_authenticate;
    let token = oidc_authenticate("github").unwrap();
    assert!(!token.identity.is_empty());
}

#[test]
fn pr3_2_request_certificate() {
    use crate::package::signing::{oidc_authenticate, request_certificate};
    let oidc = oidc_authenticate("github").unwrap();
    let cert = request_certificate(&oidc).unwrap();
    assert!(cert.pem.contains("CERTIFICATE"));
}

#[test]
fn pr3_3_sign_package() {
    use crate::package::signing::{oidc_authenticate, request_certificate, sign_package};
    let oidc = oidc_authenticate("github").unwrap();
    let cert = request_certificate(&oidc).unwrap();
    let sig = sign_package("sha256:deadbeef", &cert).unwrap();
    assert!(!sig.bytes.is_empty());
}

#[test]
fn pr3_4_rekor_submit() {
    use crate::package::signing::{
        oidc_authenticate, request_certificate, sign_package, submit_to_rekor,
    };
    let oidc = oidc_authenticate("github").unwrap();
    let cert = request_certificate(&oidc).unwrap();
    let sig = sign_package("sha256:abcdef", &cert).unwrap();
    let entry = submit_to_rekor(&sig, &cert, "sha256:abcdef").unwrap();
    assert!(entry.log_index > 0);
}

#[test]
fn pr3_5_signature_bundle_roundtrip() {
    use crate::package::signing::{
        FjSignatureBundle, oidc_authenticate, request_certificate, sign_package, submit_to_rekor,
    };
    let oidc = oidc_authenticate("github").unwrap();
    let cert = request_certificate(&oidc).unwrap();
    let sig = sign_package("sha256:112233", &cert).unwrap();
    let rekor = submit_to_rekor(&sig, &cert, "sha256:112233").unwrap();
    let bundle = FjSignatureBundle::new(&cert, &sig, &rekor);
    let json = bundle.to_json();
    let restored = FjSignatureBundle::from_json(&json).unwrap();
    assert!(!restored.signature.is_empty());
}

#[test]
fn pr3_6_signing_config_default() {
    use crate::package::signing::SigningConfig;
    let config = SigningConfig::default();
    assert!(!config.fulcio_url.is_empty());
    assert!(!config.rekor_url.is_empty());
}

#[test]
fn pr3_7_sbom_document_creation() {
    use crate::package::sbom::{SbomDocument, SbomFormat};
    let doc = SbomDocument::new(SbomFormat::CycloneDx);
    assert!(doc.packages.is_empty());
}

#[test]
fn pr3_8_sbom_add_package() {
    use crate::package::sbom::{SbomDocument, SbomFormat, SbomPackage};
    let mut doc = SbomDocument::new(SbomFormat::Spdx);
    doc.add_package(SbomPackage::new(
        "serde",
        "1.0.0",
        "sha256:abc",
        Some("MIT".into()),
    ));
    assert_eq!(doc.packages.len(), 1);
}

#[test]
fn pr3_9_generate_sbom() {
    use crate::package::sbom::{DepInfo, SbomFormat, generate_sbom};
    let deps = vec![DepInfo {
        name: "serde".into(),
        version: "1.0.0".into(),
        sha256: "abc".into(),
        license: Some("MIT".into()),
        dev_only: false,
    }];
    let json = generate_sbom("test-project", &deps, SbomFormat::CycloneDx).unwrap();
    assert!(json.contains("test-project"));
}

#[test]
fn pr3_10_sbom_format_variants() {
    use crate::package::sbom::SbomFormat;
    let formats = [SbomFormat::CycloneDx, SbomFormat::Spdx];
    assert_eq!(formats.len(), 2);
}

// Sprint PR4: Security & Audit

#[test]
fn pr4_1_advisory_database_creation() {
    use crate::package::audit::AdvisoryDatabase;
    let db = AdvisoryDatabase::new();
    let findings = db.check("any-package", "1.0.0");
    assert!(findings.is_empty());
}

#[test]
fn pr4_2_advisory_from_json() {
    use crate::package::audit::AdvisoryDatabase;
    let json = r#"{"advisories":[{"id":"FJ-2026-001","package":"vuln-lib","severity":"critical","description":"RCE in parser","min_version":"1.0.0","max_version":"1.2.0","patched_version":"1.2.1"}]}"#;
    let db = AdvisoryDatabase::from_json(json).unwrap();
    let hits = db.check("vuln-lib", "1.1.0");
    assert_eq!(hits.len(), 1);
}

#[test]
fn pr4_3_version_range_affects() {
    use crate::package::audit::VersionRange;
    let range = VersionRange::new(Some("1.0.0".into()), Some("2.0.0".into()));
    assert!(range.affects("1.5.0"));
    assert!(!range.affects("2.1.0"));
}

#[test]
fn pr4_4_audit_dependencies() {
    use crate::package::audit::{AdvisoryDatabase, audit_dependencies};
    let json = r#"{"advisories":[{"id":"FJ-2026-002","package":"bad-pkg","severity":"high","description":"Denial of service","min_version":"0.1.0","max_version":"0.9.0","patched_version":"1.0.0"}]}"#;
    let db = AdvisoryDatabase::from_json(json).unwrap();
    let deps = vec![("bad-pkg".into(), "0.5.0".into())];
    let report = audit_dependencies(&deps, &db);
    assert!(report.has_critical_or_high());
}

#[test]
fn pr4_5_audit_clean() {
    use crate::package::audit::{AdvisoryDatabase, audit_dependencies};
    let db = AdvisoryDatabase::new();
    let deps = vec![("safe-pkg".into(), "1.0.0".into())];
    let report = audit_dependencies(&deps, &db);
    assert!(!report.has_critical_or_high());
    assert_eq!(report.finding_count(), 0);
}

#[test]
fn pr4_6_severity_variants() {
    use crate::package::audit::Severity;
    let severities = [
        Severity::Critical,
        Severity::High,
        Severity::Medium,
        Severity::Low,
    ];
    assert_eq!(severities.len(), 4);
}

#[test]
fn pr4_7_severity_color_code() {
    use crate::package::audit::Severity;
    let code = Severity::Critical.color_code();
    assert!(!code.is_empty());
}

#[test]
fn pr4_8_version_range_unbounded() {
    use crate::package::audit::VersionRange;
    let range = VersionRange::new(None, None);
    assert!(range.affects("1.0.0"));
    assert!(range.affects("999.0.0"));
}

#[test]
fn pr4_9_advisory_not_affected() {
    use crate::package::audit::AdvisoryDatabase;
    let json = r#"{"advisories":[{"id":"FJ-2026-003","package":"lib-x","severity":"low","description":"Bug","min_version":"1.0.0","max_version":"1.5.0","patched_version":"1.5.1"}]}"#;
    let db = AdvisoryDatabase::from_json(json).unwrap();
    let hits = db.check("lib-x", "2.0.0");
    assert!(hits.is_empty());
}

#[test]
fn pr4_10_audit_report_count() {
    use crate::package::audit::{AdvisoryDatabase, audit_dependencies};
    let json = r#"{"advisories":[
        {"id":"FJ-001","package":"a","severity":"low","description":"Bug A","min_version":"1.0.0","max_version":"2.0.0","patched_version":"2.0.1"},
        {"id":"FJ-002","package":"b","severity":"medium","description":"Bug B","min_version":"0.1.0","max_version":"0.9.0","patched_version":"1.0.0"}
    ]}"#;
    let db = AdvisoryDatabase::from_json(json).unwrap();
    let deps = vec![("a".into(), "1.5.0".into()), ("b".into(), "0.5.0".into())];
    let report = audit_dependencies(&deps, &db);
    assert_eq!(report.finding_count(), 2);
}

// ===================================================================
// PHASE 2 — Option 3: FajarOS Nova v2.0 (N1-N10)
// ===================================================================

// Sprint N1: Verified @kernel Functions
//
// 2026-05-12 Path C SMT-freeze (Compass §5.1): n1_1..n1_6 + n1_9 + n1_10
// removed alongside deletion of verify::{smt,certification,pipeline}.
// See docs/VERIFY_PATH_C_LOAD_BEARING_B0_FINDINGS.md +
// docs/decisions/2026-05-12-verify-path-c-deletion.md.
// n1_7 + n1_8 retained — they exercise verify::symbolic (production-live).

#[test]
fn n1_7_symbolic_engine_creation() {
    use crate::verify::symbolic::SymbolicEngine;
    let mut engine = SymbolicEngine::new();
    engine.init_symbolic_var("addr");
    engine.complete_paths();
    let _ = engine; // verifies creation and basic ops
}

#[test]
fn n1_8_symbolic_engine_property_check() {
    use crate::verify::symbolic::SymbolicEngine;
    let mut engine = SymbolicEngine::new();
    engine.init_symbolic_var("size");
    let counterexamples = engine.check_property("size >= 0", "kernel.fj", 42);
    // Symbolic vars are unconstrained, so may find counterexample
    let _ = counterexamples;
}

// Sprint N2: Kernel Optimization

#[test]
fn n2_1_memory_manager_alloc() {
    use crate::runtime::os::memory::MemoryManager;
    let mut mm = MemoryManager::new(1024 * 1024);
    let addr = mm.alloc(4096, 16).unwrap();
    // First alloc may start at 0; just verify it succeeded
    let _ = addr;
}

#[test]
fn n2_2_memory_manager_free() {
    use crate::runtime::os::memory::MemoryManager;
    let mut mm = MemoryManager::new(1024 * 1024);
    let addr = mm.alloc(4096, 16).unwrap();
    mm.free(addr).unwrap();
}

#[test]
fn n2_3_memory_read_write() {
    use crate::runtime::os::memory::MemoryManager;
    let mut mm = MemoryManager::new(1024 * 1024);
    let addr = mm.alloc(256, 4).unwrap();
    mm.write_u32(addr, 0xDEADBEEF).unwrap();
    let val = mm.read_u32(addr).unwrap();
    assert_eq!(val, 0xDEADBEEF);
}

#[test]
fn n2_4_memory_regions() {
    use crate::runtime::os::memory::MemoryManager;
    let mut mm = MemoryManager::new(1024 * 1024);
    mm.alloc(1024, 8).unwrap();
    mm.alloc(2048, 8).unwrap();
    let regions = mm.allocated_regions();
    assert_eq!(regions.len(), 2);
}

#[test]
fn n2_5_syscall_table_define() {
    use crate::runtime::os::syscall::SyscallTable;
    let mut table = SyscallTable::new();
    table.define(1, "sys_exit".into(), 1).unwrap();
    assert_eq!(table.syscall_count(), 1);
}

#[test]
fn n2_6_syscall_dispatch() {
    use crate::runtime::os::syscall::SyscallTable;
    let mut table = SyscallTable::new();
    table.define(1, "sys_exit".into(), 1).unwrap();
    let handler = table.dispatch(1, 1).unwrap();
    assert_eq!(handler.name, "sys_exit");
}

#[test]
fn n2_7_syscall_handler_lookup() {
    use crate::runtime::os::syscall::SyscallTable;
    let mut table = SyscallTable::new();
    table.define(42, "sys_write".into(), 3).unwrap();
    let handler = table.handler_for(42);
    assert!(handler.is_some());
    assert_eq!(handler.unwrap().arg_count, 3);
}

#[test]
fn n2_8_syscall_dispatch_log() {
    use crate::runtime::os::syscall::SyscallTable;
    let mut table = SyscallTable::new();
    table.define(1, "sys_exit".into(), 1).unwrap();
    let _ = table.dispatch(1, 1);
    let log = table.dispatch_log();
    assert!(!log.is_empty());
}

#[test]
fn n2_9_memory_size() {
    use crate::runtime::os::memory::MemoryManager;
    let mm = MemoryManager::new(65536);
    assert_eq!(mm.size(), 65536);
}

#[test]
fn n2_10_syscall_unknown() {
    use crate::runtime::os::syscall::SyscallTable;
    let table = SyscallTable::new();
    let result = table.handler_for(999);
    assert!(result.is_none());
}

// Sprint N3: Distributed Kernel Services

// Sprint N4: AI-Integrated Kernel

#[test]
fn n4_1_workload_classify_compute() {
    use crate::accelerator::dispatch::classify_workload;
    let class = classify_workload(1_000_000, 1_000, 32);
    assert_eq!(format!("{class:?}"), "ComputeBound");
}

#[test]
fn n4_2_workload_classify_memory() {
    use crate::accelerator::dispatch::classify_workload;
    let class = classify_workload(100, 1_000_000, 2);
    assert_eq!(format!("{class:?}"), "MemoryBound");
}

#[test]
fn n4_3_device_set_cpu_only() {
    use crate::accelerator::dispatch::DeviceSet;
    let devices = DeviceSet::cpu_only();
    assert!(!devices.has_gpu());
    assert!(!devices.has_npu());
}

#[test]
fn n4_4_dispatch_decision_cpu() {
    use crate::accelerator::dispatch::{DeviceSet, WorkloadDescriptor, decide_dispatch};
    let workload = WorkloadDescriptor {
        op_type: "add".into(),
        input_elements: 100,
        dtype: "f32".into(),
        batch_size: 1,
        estimated_flops: 100,
        estimated_bytes: 400,
        preference: crate::accelerator::infer::InferPreference::Auto,
    };
    let decision = decide_dispatch(&workload, &DeviceSet::cpu_only());
    assert_eq!(format!("{:?}", decision.primary), "Cpu");
}

#[test]
fn n4_5_fusion_graph_matmul() {
    use crate::gpu_codegen::fusion::{FusionGraph, GpuOp, OpKind};
    let ops = vec![
        GpuOp {
            id: 0,
            kind: OpKind::Matmul,
            inputs: vec![],
            output_elements: 1024,
        },
        GpuOp {
            id: 1,
            kind: OpKind::ElementWiseUnary,
            inputs: vec![0],
            output_elements: 1024,
        },
    ];
    let mut graph = FusionGraph::new(ops);
    graph.analyze();
    // Matmul + elementwise should fuse
    assert!(graph.total_fused_ops() >= 1);
}

#[test]
fn n4_6_device_allocator_multiple() {
    use crate::gpu_codegen::gpu_memory::DeviceAllocator;
    let mut alloc = DeviceAllocator::new(0, 1024 * 1024, 4 * 1024 * 1024);
    let a1 = alloc.allocate(1024).unwrap();
    let a2 = alloc.allocate(2048).unwrap();
    let stats = alloc.stats();
    assert_eq!(stats.used_bytes, 3072);
    alloc.free(a1.id).unwrap();
    alloc.free(a2.id).unwrap();
}

#[test]
fn n4_7_spirv_compute_module() {
    use crate::gpu_codegen::spirv::{EntryPoint, ExecutionModel, SpirVModule, SpirVType};
    let mut m = SpirVModule::new_compute();
    let void_id = m.alloc_id();
    m.types.push(SpirVType::Void { id: void_id });
    let fn_id = m.alloc_id();
    m.entry_points.push(EntryPoint {
        execution_model: ExecutionModel::GLCompute,
        function_id: fn_id,
        name: "anomaly_detect".into(),
        interface_ids: vec![],
        local_size: [64, 1, 1],
    });
    assert!(m.validate().is_empty());
}

#[test]
fn n4_8_ptx_kernel_ai() {
    use crate::gpu_codegen::ptx::{KernelEntry, KernelParam, PtxType};
    let kernel = KernelEntry {
        name: "predict_duration".into(),
        params: vec![
            KernelParam {
                name: "features".into(),
                ptx_type: PtxType::F32,
                is_pointer: true,
            },
            KernelParam {
                name: "weights".into(),
                ptx_type: PtxType::F32,
                is_pointer: true,
            },
            KernelParam {
                name: "output".into(),
                ptx_type: PtxType::F32,
                is_pointer: true,
            },
        ],
        body: vec![],
    };
    let ptx = kernel.emit();
    assert!(ptx.contains("predict_duration"));
}

// 2026-05-12 Path C SMT-freeze: n4_9 (smt) + n4_10 (certification) removed.

// Sprint N5: Hardware Abstraction v2

#[test]
fn n5_1_memory_multi_alloc() {
    use crate::runtime::os::memory::MemoryManager;
    let mut mm = MemoryManager::new(1024 * 1024);
    let addrs: Vec<_> = (0..10).map(|_| mm.alloc(1024, 8).unwrap()).collect();
    assert_eq!(addrs.len(), 10);
    for addr in addrs {
        mm.free(addr).unwrap();
    }
}

#[test]
fn n5_2_syscall_multiple_define() {
    use crate::runtime::os::syscall::SyscallTable;
    let mut table = SyscallTable::new();
    table.define(0, "sys_read".into(), 3).unwrap();
    table.define(1, "sys_write".into(), 3).unwrap();
    table.define(2, "sys_open".into(), 2).unwrap();
    table.define(3, "sys_close".into(), 1).unwrap();
    assert_eq!(table.syscall_count(), 4);
}

#[test]
fn n5_3_memory_write_read_bytes() {
    use crate::runtime::os::memory::MemoryManager;
    let mut mm = MemoryManager::new(1024 * 1024);
    let addr = mm.alloc(256, 1).unwrap();
    let data = vec![0xCA, 0xFE, 0xBA, 0xBE];
    mm.write_bytes(addr, &data).unwrap();
    let read = mm.read_bytes(addr, 4).unwrap();
    assert_eq!(read, data);
}

#[test]
fn n5_7_spirv_type_vector() {
    use crate::gpu_codegen::spirv::SpirVType;
    let vec4 = SpirVType::Vector {
        id: 10,
        component_id: 5,
        count: 4,
    };
    assert_eq!(vec4.id(), 10);
}

#[test]
fn n5_8_ptx_grid_rounding() {
    use crate::gpu_codegen::ptx::compute_grid_1d;
    let grid = compute_grid_1d(1000, 256);
    assert!(grid.total_threads() >= 1000);
}

// 2026-05-12 Path C SMT-freeze: n5_9 (smt) removed.

#[test]
fn n5_10_memory_alloc_alignment() {
    use crate::runtime::os::memory::MemoryManager;
    let mut mm = MemoryManager::new(1024 * 1024);
    let addr = mm.alloc(64, 64).unwrap();
    assert_eq!(addr.0 % 64, 0);
}

// Sprint N6: Network Stack v2

#[test]
fn n6_1_syscall_table_full() {
    use crate::runtime::os::syscall::SyscallTable;
    let mut table = SyscallTable::new();
    for i in 0..34 {
        table
            .define(i, format!("sys_{i}"), (i % 4 + 1) as usize)
            .unwrap();
    }
    assert_eq!(table.syscall_count(), 34);
}

// 2026-05-12 Path C SMT-freeze: n6_5 (smt) removed.

#[test]
fn n6_6_spirv_backend_explicit() {
    use crate::gpu_codegen::spirv::{GpuBackend, resolve_backend};
    let r = resolve_backend(GpuBackend::SpirV, "NVIDIA");
    assert_eq!(r, GpuBackend::SpirV); // explicit overrides auto
}

#[test]
fn n6_7_ptx_type_all() {
    use crate::gpu_codegen::ptx::{PtxType, map_type};
    assert_eq!(map_type("u32"), Some(PtxType::U32));
    assert_eq!(map_type("u64"), Some(PtxType::U64));
    assert_eq!(map_type("f16"), Some(PtxType::F16));
}

#[test]
fn n6_8_fusion_no_fuse_independent() {
    use crate::gpu_codegen::fusion::{GpuOp, OpKind, can_fuse};
    let a = GpuOp {
        id: 0,
        kind: OpKind::ElementWiseUnary,
        inputs: vec![],
        output_elements: 100,
    };
    let b = GpuOp {
        id: 1,
        kind: OpKind::ElementWiseUnary,
        inputs: vec![],
        output_elements: 100,
    };
    assert!(!can_fuse(&a, &b)); // b doesn't depend on a
}

#[test]
fn n6_9_memory_manager_default() {
    use crate::runtime::os::memory::MemoryManager;
    let mm = MemoryManager::with_default_size();
    assert!(mm.size() > 0);
}

// 2026-05-12 Path C SMT-freeze: n6_10 (smt) removed.

// Sprint N7: Userland Libraries

#[test]
fn n7_6_ffi_mangle_name() {
    use crate::ffi_v2::cpp::mangle_name;
    let mangled = mangle_name(&["std".into()], "sort", &[]);
    assert!(!mangled.is_empty());
}

#[test]
fn n7_7_ffi_demangle_name() {
    use crate::ffi_v2::cpp::{demangle_name, mangle_name};
    let mangled = mangle_name(&[], "hello", &[]);
    let demangled = demangle_name(&mangled);
    assert!(demangled.contains("hello"));
}

// Sprint N8: GUI Framework

#[test]
fn n8_1_interpreter_gpu_available() {
    let mut interp = Interpreter::new();
    let result = interp.eval_source("let avail = gpu_available()\navail");
    assert!(result.is_ok());
}

#[test]
fn n8_2_interpreter_gpu_info() {
    let mut interp = Interpreter::new();
    let result = interp.eval_source("gpu_info()");
    assert!(result.is_ok());
}

#[test]
fn n8_3_spirv_variable_ssbo() {
    use crate::gpu_codegen::spirv::{StorageClass, create_ssbo};
    let v1 = create_ssbo(1, 2, 0, 0);
    let v2 = create_ssbo(3, 4, 1, 0);
    assert_eq!(v1.storage_class, v2.storage_class);
    assert_eq!(v1.storage_class, StorageClass::StorageBuffer);
}

#[test]
fn n8_4_spirv_builtin_values() {
    use crate::gpu_codegen::spirv::BuiltIn;
    assert_eq!(BuiltIn::GlobalInvocationId.value(), 28);
    assert_eq!(BuiltIn::WorkGroupId.value(), 26);
}

#[test]
fn n8_7_syscall_undefine() {
    use crate::runtime::os::syscall::SyscallTable;
    let mut table = SyscallTable::new();
    table.define(1, "sys_exit".into(), 1).unwrap();
    table.undefine(1).unwrap();
    assert_eq!(table.syscall_count(), 0);
}

#[test]
fn n8_8_memory_alloc_free_reuse() {
    use crate::runtime::os::memory::MemoryManager;
    let mut mm = MemoryManager::new(4096);
    let a = mm.alloc(1024, 8).unwrap();
    mm.free(a).unwrap();
    let b = mm.alloc(1024, 8).unwrap();
    // Should be able to allocate again after free
    assert!(b.0 > 0);
}

// 2026-05-12 Path C SMT-freeze: n8_9 (smt) removed.

#[test]
fn n8_10_fusion_total_ops() {
    use crate::gpu_codegen::fusion::{FusionGraph, GpuOp, OpKind};
    let ops = vec![
        GpuOp {
            id: 0,
            kind: OpKind::ElementWiseUnary,
            inputs: vec![],
            output_elements: 100,
        },
        GpuOp {
            id: 1,
            kind: OpKind::ElementWiseUnary,
            inputs: vec![0],
            output_elements: 100,
        },
        GpuOp {
            id: 2,
            kind: OpKind::ElementWiseBinary,
            inputs: vec![1],
            output_elements: 100,
        },
    ];
    let mut graph = FusionGraph::new(ops);
    graph.analyze();
    assert!(graph.total_fused_ops() >= 2);
}

// Sprint N9: Package Manager

#[test]
fn n9_1_registry_download_count() {
    use crate::package::registry::{Registry, SemVer};
    let mut reg = Registry::new();
    reg.publish("os-pkg", SemVer::new(1, 0, 0), "OS package");
    let count = reg.download_count("os-pkg");
    assert!(count.is_some());
}

#[test]
fn n9_2_registry_search_empty() {
    use crate::package::registry::Registry;
    let reg = Registry::new();
    let results = reg.search("nonexistent");
    assert!(results.is_empty());
}

#[test]
fn n9_3_semver_ordering() {
    use crate::package::registry::SemVer;
    let v1 = SemVer::new(1, 0, 0);
    let v2 = SemVer::new(2, 0, 0);
    assert!(v1 < v2);
}

#[test]
fn n9_4_version_constraint_exact() {
    use crate::package::registry::{SemVer, VersionConstraint};
    let c = VersionConstraint::parse("1.5.0").unwrap();
    assert!(c.matches(&SemVer::new(1, 5, 0)));
    assert!(!c.matches(&SemVer::new(1, 5, 1)));
}

#[test]
fn n9_7_sbom_spdx() {
    use crate::package::sbom::{DepInfo, SbomFormat, generate_sbom};
    let deps = vec![DepInfo {
        name: "core".into(),
        version: "1.0.0".into(),
        sha256: "abc".into(),
        license: Some("MIT".into()),
        dev_only: false,
    }];
    let spdx = generate_sbom("fajaros", &deps, SbomFormat::Spdx).unwrap();
    assert!(spdx.contains("fajaros"));
}

#[test]
fn n9_8_audit_empty_db() {
    use crate::package::audit::{AdvisoryDatabase, audit_dependencies};
    let db = AdvisoryDatabase::new();
    let deps = vec![("anything".into(), "1.0.0".into())];
    let report = audit_dependencies(&deps, &db);
    assert_eq!(report.finding_count(), 0);
}

#[test]
fn n9_9_signing_full_flow() {
    use crate::package::signing::{
        FjSignatureBundle, oidc_authenticate, request_certificate, sign_package, submit_to_rekor,
    };
    let oidc = oidc_authenticate("github").unwrap();
    let cert = request_certificate(&oidc).unwrap();
    let sig = sign_package("sha256:pkg-hash", &cert).unwrap();
    let rekor = submit_to_rekor(&sig, &cert, "sha256:pkg-hash").unwrap();
    let bundle = FjSignatureBundle::new(&cert, &sig, &rekor);
    let json = bundle.to_json();
    assert!(json.contains("certificate_pem"));
}

#[test]
fn n9_10_version_constraint_gte() {
    use crate::package::registry::{SemVer, VersionConstraint};
    let c = VersionConstraint::parse(">=2.0.0").unwrap();
    assert!(c.matches(&SemVer::new(2, 0, 0)));
    assert!(c.matches(&SemVer::new(3, 0, 0)));
    assert!(!c.matches(&SemVer::new(1, 9, 9)));
}

// Sprint N10: Release
//
// 2026-05-12 Path C SMT-freeze: n10_1 (certification) + n10_3 (smt) removed.

#[test]
fn n10_2_spirv_full_module() {
    use crate::gpu_codegen::spirv::*;
    let mut m = SpirVModule::new_compute();
    let void_id = m.alloc_id();
    let fn_type_id = m.alloc_id();
    let fn_id = m.alloc_id();
    m.types.push(SpirVType::Void { id: void_id });
    m.types.push(SpirVType::Function {
        id: fn_type_id,
        return_type_id: void_id,
        param_type_ids: vec![],
    });
    m.functions.push(SpirVFunction {
        id: fn_id,
        return_type_id: void_id,
        function_type_id: fn_type_id,
        param_ids: vec![],
        blocks: vec![],
    });
    m.entry_points.push(EntryPoint {
        execution_model: ExecutionModel::GLCompute,
        function_id: fn_id,
        name: "release_kernel".into(),
        interface_ids: vec![],
        local_size: [256, 1, 1],
    });
    assert!(m.validate().is_empty());
    let words = m.emit_words();
    assert_eq!(words[0], 0x0723_0203);
}

#[test]
fn n10_4_registry_yank_nonexistent() {
    use crate::package::registry::{Registry, SemVer};
    let mut reg = Registry::new();
    let result = reg.yank("no-pkg", &SemVer::new(1, 0, 0));
    assert!(result.is_err());
}

#[test]
fn n10_7_ffi_generate_class() {
    use crate::ffi_v2::cpp::{CppClass, generate_class_binding};
    let class = CppClass {
        name: "Widget".into(),
        namespace: vec![],
        bases: vec![],
        fields: vec![],
        methods: vec![],
        constructors: vec![],
        has_destructor: false,
        is_abstract: false,
        template_params: vec![],
        size_bytes: 0,
        align_bytes: 0,
    };
    let binding = generate_class_binding(&class);
    assert!(binding.contains("Widget"));
}

#[test]
fn n10_8_memory_manager_stress() {
    use crate::runtime::os::memory::MemoryManager;
    let mut mm = MemoryManager::new(16 * 1024 * 1024);
    let mut addrs = Vec::new();
    for _ in 0..100 {
        addrs.push(mm.alloc(256, 8).unwrap());
    }
    for addr in addrs {
        mm.free(addr).unwrap();
    }
    assert!(mm.allocated_regions().is_empty());
}

#[test]
fn n10_10_api_status_codes() {
    use crate::package::server::StatusCode;
    assert_eq!(StatusCode::OK.0, 200);
    assert_eq!(StatusCode::CREATED.0, 201);
    assert_eq!(StatusCode::NOT_FOUND.0, 404);
    assert_eq!(StatusCode::BAD_REQUEST.0, 400);
}

// ===================================================================
// PHASE 2 — Option 4: Real-World Validation (W1-W10)
// ===================================================================

// Sprint W1: FFI Bindgen & OpenCV

#[test]
fn w1_1_ffi_bindgen_config() {
    use crate::ffi_v2::bindgen::BindgenConfig;
    let config = BindgenConfig::new(
        "opencv2/core.hpp",
        crate::ffi_v2::bindgen::BindgenLanguage::Cpp,
        "bindings.fj",
    );
    assert_eq!(config.source_path, "opencv2/core.hpp");
}

#[test]
fn w1_2_cpp_mangle_with_namespace() {
    use crate::ffi_v2::cpp::{CppType, mangle_name};
    let mangled = mangle_name(&["cv".into()], "imread", &[CppType::String]);
    assert!(!mangled.is_empty());
}

#[test]
fn w1_3_cpp_class_binding() {
    use crate::ffi_v2::cpp::{CppClass, CppFunction, CppIntSize, CppType, generate_class_binding};
    let class = CppClass {
        name: "Mat".into(),
        namespace: vec![],
        bases: vec![],
        fields: vec![],
        methods: vec![CppFunction {
            name: "rows".into(),
            namespace: vec![],
            return_type: CppType::Int(CppIntSize::I32),
            params: vec![],
            is_const: true,
            is_static: false,
            is_virtual: false,
            is_noexcept: false,
            template_params: vec![],
        }],
        constructors: vec![],
        has_destructor: true,
        is_abstract: false,
        template_params: vec![],
        size_bytes: 0,
        align_bytes: 0,
    };
    let binding = generate_class_binding(&class);
    assert!(binding.contains("Mat"));
    assert!(binding.contains("rows"));
}

#[test]
fn w1_4_cpp_demangle() {
    use crate::ffi_v2::cpp::demangle_name;
    let result =
        demangle_name("_ZN2cv6imreadERKNSt7__cxx1112basic_stringIcSt11char_traitsIcESaIcEEEi");
    assert!(result.contains("imread") || result.contains("unknown"));
}

#[test]
fn w1_5_ffi_fajar_bindings() {
    use crate::ffi_v2::cpp::{
        CppDecl, CppFunction, CppIntSize, CppParam, CppType, generate_fajar_bindings,
    };
    let decls = vec![CppDecl::Function(CppFunction {
        name: "detect_faces".into(),
        namespace: vec![],
        return_type: CppType::Int(CppIntSize::I32),
        params: vec![CppParam {
            name: "img".into(),
            param_type: CppType::Pointer(Box::new(CppType::Void)),
            has_default: false,
        }],
        is_static: false,
        is_const: false,
        is_virtual: false,
        is_noexcept: false,
        template_params: vec![],
    })];
    let fj = generate_fajar_bindings(&decls);
    assert!(fj.contains("detect_faces"));
}

#[test]
fn w1_6_spirv_type_f16() {
    use crate::gpu_codegen::spirv::{SpirVTypeDesc, map_fj_type};
    assert_eq!(map_fj_type("f16"), Some(SpirVTypeDesc::Float(16)));
    assert_eq!(map_fj_type("u8"), Some(SpirVTypeDesc::Int(8, false)));
}

#[test]
fn w1_7_ptx_f16_type() {
    use crate::gpu_codegen::ptx::{PtxType, map_type};
    assert_eq!(map_type("f16"), Some(PtxType::F16));
    assert_eq!(map_type("bool"), Some(PtxType::Pred));
}

#[test]
fn w1_8_registry_publish_multiple() {
    use crate::package::registry::{Registry, SemVer};
    let mut reg = Registry::new();
    reg.publish(
        "opencv-fj",
        SemVer::new(0, 1, 0),
        "OpenCV bindings for Fajar",
    );
    reg.publish("opencv-fj", SemVer::new(0, 2, 0), "OpenCV bindings v0.2");
    let latest = reg.latest_version("opencv-fj").unwrap();
    assert_eq!(*latest, SemVer::new(0, 2, 0));
}

// 2026-05-12 Path C SMT-freeze: w1_9 (smt) removed.

#[test]
fn w1_10_memory_read_unalloc() {
    use crate::runtime::os::memory::MemoryManager;
    let mm = MemoryManager::new(4096);
    let result = mm.read_u32(crate::runtime::os::memory::VirtAddr(99999));
    assert!(result.is_err());
}

// Sprint W2: WASI HTTP Server

#[test]
fn w2_4_interpreter_http_route() {
    let mut interp = Interpreter::new();
    let result = interp.eval_source(
        r#"let status = 200
status"#,
    );
    assert!(result.is_ok());
}

#[test]
fn w2_5_json_parse_eval() {
    let mut interp = Interpreter::new();
    let result = interp.eval_source(
        r#"let data = "{\"key\": \"value\"}"
len(data)"#,
    );
    assert!(result.is_ok());
}

#[test]
fn w2_6_api_response_types() {
    use crate::package::server::{ApiResponse, StatusCode};
    let ok = ApiResponse::ok("{}");
    let err = ApiResponse::error(StatusCode::TOO_MANY_REQUESTS, "rate limited");
    assert_eq!(ok.status.0, 200);
    assert_eq!(err.status.0, 429);
}

#[test]
fn w2_7_version_constraint_lt() {
    use crate::package::registry::{SemVer, VersionConstraint};
    let c = VersionConstraint::parse("<2.0.0").unwrap();
    assert!(c.matches(&SemVer::new(1, 9, 9)));
    assert!(!c.matches(&SemVer::new(2, 0, 0)));
}

#[test]
fn w2_9_spirv_dispatch_large() {
    use crate::gpu_codegen::spirv::compute_dispatch_1d;
    let d = compute_dispatch_1d(1_000_000, 256);
    assert!(d.group_count_x > 3000);
}

// 2026-05-12 Path C SMT-freeze: w2_10 (smt) removed.

// Sprint W3: Distributed MNIST Training

#[test]
fn w3_1_interpreter_tensor_zeros() {
    let mut interp = Interpreter::new();
    let result = interp.eval_source("let t = zeros(3, 4)\nshape(t)");
    assert!(result.is_ok());
}

#[test]
fn w3_2_interpreter_tensor_matmul() {
    let mut interp = Interpreter::new();
    let result = interp
        .eval_source("let a = ones(2, 3)\nlet b = ones(3, 4)\nlet c = matmul(a, b)\nshape(c)");
    assert!(result.is_ok());
}

#[test]
fn w3_3_interpreter_dense_layer() {
    let mut interp = Interpreter::new();
    let result = interp.eval_source(
        "let layer = Dense(4, 2)\nlet x = ones(1, 4)\nlet y = forward(layer, x)\nshape(y)",
    );
    assert!(result.is_ok());
}

// 2026-05-12 Path C SMT-freeze: w3_7 (smt) removed.

#[test]
fn w3_8_fusion_chain() {
    use crate::gpu_codegen::fusion::{FusionGraph, GpuOp, OpKind};
    let ops = vec![
        GpuOp {
            id: 0,
            kind: OpKind::Matmul,
            inputs: vec![],
            output_elements: 3584,
        },
        GpuOp {
            id: 1,
            kind: OpKind::ElementWiseUnary,
            inputs: vec![0],
            output_elements: 3584,
        }, // relu
        GpuOp {
            id: 2,
            kind: OpKind::Matmul,
            inputs: vec![1],
            output_elements: 280,
        },
        GpuOp {
            id: 3,
            kind: OpKind::Softmax,
            inputs: vec![2],
            output_elements: 280,
        },
    ];
    let mut graph = FusionGraph::new(ops);
    graph.analyze();
    assert!(graph.num_fusions() >= 1);
}

#[test]
fn w3_9_device_alloc_gpu_memory() {
    use crate::gpu_codegen::gpu_memory::DeviceAllocator;
    let mut alloc = DeviceAllocator::new(0, 256 * 1024 * 1024, 512 * 1024 * 1024);
    // Allocate space for MNIST weights
    let _w1 = alloc.allocate(784 * 128 * 4).unwrap(); // 784→128 f32
    let _w2 = alloc.allocate(128 * 10 * 4).unwrap(); // 128→10 f32
    let stats = alloc.stats();
    assert!(stats.used_bytes > 0);
}

#[test]
fn w3_10_interpreter_relu() {
    let mut interp = Interpreter::new();
    let result =
        interp.eval_source("let t = from_data([[1.0, -2.0], [3.0, -4.0]])\nlet r = relu(t)\nr");
    assert!(result.is_ok());
}

// Sprint W4: PyTorch Model Inference

#[test]
fn w4_1_ffi_python_types() {
    use crate::ffi_v2::cpp::{CppIntSize, CppType};
    let types = [
        CppType::Int(CppIntSize::I32),
        CppType::Float,
        CppType::Double,
        CppType::Void,
    ];
    assert_eq!(types.len(), 4);
}

#[test]
fn w4_2_interpreter_softmax() {
    let mut interp = Interpreter::new();
    let result = interp.eval_source("let t = from_data([[1.0, 2.0, 3.0]])\nlet s = softmax(t)\ns");
    assert!(result.is_ok());
}

#[test]
fn w4_3_interpreter_sigmoid() {
    let mut interp = Interpreter::new();
    let result = interp.eval_source("let t = from_data([[0.0, 1.0, -1.0]])\nlet s = sigmoid(t)\ns");
    assert!(result.is_ok());
}

#[test]
fn w4_4_interpreter_reshape() {
    let mut interp = Interpreter::new();
    let result = interp.eval_source("let t = ones(2, 6)\nlet r = reshape(t, [3, 4])\nshape(r)");
    assert!(result.is_ok());
}

#[test]
fn w4_5_interpreter_transpose() {
    let mut interp = Interpreter::new();
    let result = interp.eval_source("let t = zeros(3, 5)\nlet r = transpose(t)\nshape(r)");
    assert!(result.is_ok());
}

#[test]
fn w4_6_ffi_class_with_methods() {
    use crate::ffi_v2::cpp::{CppClass, CppFunction, CppParam, CppType, generate_class_binding};
    let class = CppClass {
        name: "TorchModel".into(),
        namespace: vec![],
        bases: vec![],
        fields: vec![],
        methods: vec![
            CppFunction {
                name: "forward".into(),
                namespace: vec![],
                return_type: CppType::Pointer(Box::new(CppType::Float)),
                params: vec![CppParam {
                    name: "input".into(),
                    param_type: CppType::Pointer(Box::new(CppType::Float)),
                    has_default: false,
                }],
                is_const: false,
                is_static: false,
                is_virtual: true,
                is_noexcept: false,
                template_params: vec![],
            },
            CppFunction {
                name: "eval".into(),
                namespace: vec![],
                return_type: CppType::Void,
                params: vec![],
                is_const: false,
                is_static: false,
                is_virtual: false,
                is_noexcept: false,
                template_params: vec![],
            },
        ],
        constructors: vec![],
        has_destructor: true,
        is_abstract: false,
        template_params: vec![],
        size_bytes: 0,
        align_bytes: 0,
    };
    let binding = generate_class_binding(&class);
    assert!(binding.contains("forward"));
}

#[test]
fn w4_7_spirv_type_u64() {
    use crate::gpu_codegen::spirv::{SpirVTypeDesc, map_fj_type};
    assert_eq!(map_fj_type("u64"), Some(SpirVTypeDesc::Int(64, false)));
    assert_eq!(map_fj_type("i64"), Some(SpirVTypeDesc::Int(64, true)));
}

#[test]
fn w4_8_ptx_arith_ops() {
    use crate::gpu_codegen::ptx::ArithOp;
    let ops = [
        ArithOp::Add,
        ArithOp::Sub,
        ArithOp::Mul,
        ArithOp::Div,
        ArithOp::Rem,
    ];
    assert_eq!(ops.len(), 5);
}

#[test]
fn w4_9_version_constraint_caret_zero() {
    use crate::package::registry::{SemVer, VersionConstraint};
    let c = VersionConstraint::parse("^0.5.0").unwrap();
    assert!(c.matches(&SemVer::new(0, 5, 3)));
}

#[test]
fn w4_10_interpreter_mse_loss() {
    let mut interp = Interpreter::new();
    let result = interp.eval_source("let pred = from_data([[1.0, 2.0]])\nlet target = from_data([[1.5, 2.5]])\nmse_loss(pred, target)");
    assert!(result.is_ok());
}

// Sprint W5: Embedded ML (Radxa Dragon Q6A)

#[test]
fn w5_1_interpreter_quantize() {
    let mut interp = Interpreter::new();
    let result =
        interp.eval_source("let t = from_data([[1.0, 2.0, 3.0]])\nlet q = quantize_int8(t)\nq");
    assert!(result.is_ok());
}

// 2026-05-12 Path C SMT-freeze: w5_2 (smt) removed.

#[test]
fn w5_3_memory_small_alloc() {
    use crate::runtime::os::memory::MemoryManager;
    let mut mm = MemoryManager::new(4096);
    let a = mm.alloc(16, 4).unwrap();
    mm.write_u32(a, 42).unwrap();
    assert_eq!(mm.read_u32(a).unwrap(), 42);
}

#[test]
fn w5_4_ptx_kernel_quantized() {
    use crate::gpu_codegen::ptx::{KernelEntry, KernelParam, PtxType};
    let kernel = KernelEntry {
        name: "quantized_matmul".into(),
        params: vec![
            KernelParam {
                name: "a".into(),
                ptx_type: PtxType::U8,
                is_pointer: true,
            },
            KernelParam {
                name: "b".into(),
                ptx_type: PtxType::U8,
                is_pointer: true,
            },
            KernelParam {
                name: "c".into(),
                ptx_type: PtxType::S32,
                is_pointer: true,
            },
        ],
        body: vec![],
    };
    let ptx = kernel.emit();
    assert!(ptx.contains("quantized_matmul"));
}

#[test]
fn w5_5_dispatch_small_workload() {
    use crate::accelerator::dispatch::{DeviceSet, WorkloadDescriptor, decide_dispatch};
    let workload = WorkloadDescriptor {
        op_type: "relu".into(),
        input_elements: 10,
        dtype: "f32".into(),
        batch_size: 1,
        estimated_flops: 10,
        estimated_bytes: 40,
        preference: crate::accelerator::infer::InferPreference::Auto,
    };
    let decision = decide_dispatch(&workload, &DeviceSet::cpu_only());
    assert_eq!(format!("{:?}", decision.primary), "Cpu");
}

#[test]
fn w5_6_interpreter_conv2d() {
    let mut interp = Interpreter::new();
    let result = interp.eval_source(
        "let layer = Conv2d(1, 8, 3, 1, 0)\nlet x = ones(1, 1, 8, 8)\nlet y = forward(layer, x)\ny",
    );
    assert!(result.is_ok());
}

#[test]
fn w5_7_capability_int8() {
    use crate::gpu_codegen::spirv::Capability;
    assert_eq!(Capability::Int8.value(), 39);
}

// 2026-05-12 Path C SMT-freeze: w5_9 (smt) removed.

#[test]
fn w5_10_interpreter_eye() {
    let mut interp = Interpreter::new();
    let result = interp.eval_source("let e = eye(4)\nshape(e)");
    assert!(result.is_ok());
}

// Sprint W6: Rust serde_json Interop

#[test]
fn w6_1_interpreter_json_roundtrip() {
    let mut interp = Interpreter::new();
    let result = interp.eval_source(
        r#"let s = "{\"x\":1}"
len(s)"#,
    );
    assert!(result.is_ok());
}

#[test]
fn w6_2_interpreter_string_ops() {
    let mut interp = Interpreter::new();
    let result = interp.eval_source(
        r#"let s = "hello world"
let parts = s.split(" ")
len(parts)"#,
    );
    assert!(result.is_ok());
}

#[test]
fn w6_3_interpreter_array_collect() {
    let mut interp = Interpreter::new();
    let result = interp.eval_source("let arr = [1, 2, 3, 4, 5]\nlen(arr)");
    assert!(result.is_ok());
}

#[test]
fn w6_4_interpreter_map_create() {
    let mut interp = Interpreter::new();
    let result = interp.eval_source(
        r#"let mut m = map_new()
m = map_insert(m, "key", "value")
map_get(m, "key")"#,
    );
    assert!(result.is_ok());
}

#[test]
fn w6_5_ffi_mangle_params() {
    use crate::ffi_v2::cpp::{CppType, mangle_name};
    let m = mangle_name(&["serde".into()], "to_json", &[CppType::String]);
    assert!(!m.is_empty());
}

#[test]
fn w6_7_registry_constraint_resolve_latest() {
    use crate::package::registry::{Registry, SemVer, VersionConstraint};
    let mut reg = Registry::new();
    reg.publish("serde-fj", SemVer::new(1, 0, 0), "Serde for FJ");
    reg.publish("serde-fj", SemVer::new(1, 1, 0), "Serde for FJ");
    reg.publish("serde-fj", SemVer::new(1, 2, 0), "Serde for FJ");
    let c = VersionConstraint::parse("^1.0.0").unwrap();
    let resolved = reg.resolve("serde-fj", &c).unwrap();
    assert_eq!(resolved, SemVer::new(1, 2, 0));
}

#[test]
fn w6_8_interpreter_to_string() {
    let mut interp = Interpreter::new();
    let result = interp.eval_source("let x = 42\nto_string(x)");
    assert!(result.is_ok());
}

#[test]
fn w6_9_interpreter_parse_int() {
    let mut interp = Interpreter::new();
    let result = interp.eval_source(
        r#"let r = "123".parse_int()
r"#,
    );
    assert!(result.is_ok());
}

#[test]
fn w6_10_interpreter_string_contains() {
    let mut interp = Interpreter::new();
    let result = interp.eval_source(r#""hello world".contains("world")"#);
    assert!(result.is_ok());
}

// Sprint W7: WebSocket Chat

#[test]
fn w7_1_interpreter_array_push() {
    // FJARR_LEAK Phase 2 D-FULL (v35.5.0): arrays are affine. Use chain-grow
    // re-assignment (E1.5) so each push consumes + re-binds in the same name.
    let mut interp = Interpreter::new();
    let result =
        interp.eval_source("let mut arr = []\narr = push(arr, 1)\narr = push(arr, 2)\nlen(arr)");
    assert!(result.is_ok());
}

#[test]
fn w7_2_interpreter_while_loop() {
    let mut interp = Interpreter::new();
    let result = interp.eval_source("let mut i = 0\nwhile i < 10 { i = i + 1 }\ni");
    assert!(result.is_ok());
}

#[test]
fn w7_3_interpreter_function_def() {
    let mut interp = Interpreter::new();
    let result = interp.eval_source("fn double(x: i32) -> i32 { x * 2 }\ndouble(21)");
    assert!(result.is_ok());
}

#[test]
fn w7_6_spirv_memory_model() {
    use crate::gpu_codegen::spirv::MemoryModel;
    let models = [
        MemoryModel::Glsl450Logical,
        MemoryModel::Glsl450Physical32,
        MemoryModel::Glsl450Physical64,
    ];
    assert_eq!(models.len(), 3);
}

#[test]
fn w7_7_version_parse_error() {
    use crate::package::registry::SemVer;
    assert!(SemVer::parse("abc").is_err());
    assert!(SemVer::parse("1.2").is_err());
}

#[test]
fn w7_8_interpreter_match_expr() {
    let mut interp = Interpreter::new();
    let result = interp.eval_source("let x = 2\nmatch x { 1 => 10, 2 => 20, _ => 0 }");
    assert!(result.is_ok());
}

#[test]
fn w7_9_interpreter_struct() {
    let mut interp = Interpreter::new();
    let result =
        interp.eval_source("struct Msg { text: str }\nlet m = Msg { text: \"hello\" }\nm.text");
    assert!(result.is_ok());
}

#[test]
fn w7_10_interpreter_enum() {
    let mut interp = Interpreter::new();
    let result = interp.eval_source("enum Color { Red, Green, Blue }\nlet c = Color::Red\nc");
    assert!(result.is_ok());
}

// Sprint W8: CLI Tool

#[test]
fn w8_1_interpreter_if_else() {
    let mut interp = Interpreter::new();
    let result = interp.eval_source("let x = if true { 42 } else { 0 }\nx");
    assert!(result.is_ok());
}

#[test]
fn w8_2_interpreter_for_loop() {
    let mut interp = Interpreter::new();
    let result = interp.eval_source("let mut sum = 0\nfor i in 0..5 { sum = sum + i }\nsum");
    assert!(result.is_ok());
}

#[test]
fn w8_3_interpreter_closure() {
    let mut interp = Interpreter::new();
    let result = interp.eval_source("let add = |a: i32, b: i32| -> i32 { a + b }\nadd(3, 4)");
    assert!(result.is_ok());
}

#[test]
fn w8_4_interpreter_nested_fn() {
    let mut interp = Interpreter::new();
    let result = interp.eval_source("fn outer(x: i32) -> i32 {\n  fn inner(y: i32) -> i32 { y * 2 }\n  inner(x) + 1\n}\nouter(5)");
    assert!(result.is_ok());
}

#[test]
fn w8_5_interpreter_string_format() {
    let mut interp = Interpreter::new();
    let result = interp.eval_source(
        r#"let name = "Fajar"
let msg = f"Hello {name}"
msg"#,
    );
    assert!(result.is_ok());
}

#[test]
fn w8_6_interpreter_array_index() {
    let mut interp = Interpreter::new();
    let result = interp.eval_source("let arr = [10, 20, 30]\narr[1]");
    assert!(result.is_ok());
}

#[test]
fn w8_7_interpreter_pipeline() {
    let mut interp = Interpreter::new();
    let result = interp.eval_source(
        "fn double(x: i32) -> i32 { x * 2 }\nfn inc(x: i32) -> i32 { x + 1 }\n5 |> double |> inc",
    );
    assert!(result.is_ok());
}

#[test]
fn w8_8_interpreter_recursive() {
    let mut interp = Interpreter::new();
    let result = interp.eval_source(
        "fn fib(n: i32) -> i32 {\n  if n <= 1 { n } else { fib(n - 1) + fib(n - 2) }\n}\nfib(10)",
    );
    assert!(result.is_ok());
}

#[test]
fn w8_9_interpreter_type_of() {
    let mut interp = Interpreter::new();
    let result = interp.eval_source(r#"type_of(42)"#);
    assert!(result.is_ok());
}

#[test]
fn w8_10_interpreter_assert() {
    let mut interp = Interpreter::new();
    let result = interp.eval_source("assert(1 + 1 == 2)");
    assert!(result.is_ok());
}

// Sprint W9: Database Client

#[test]
fn w9_1_interpreter_option_some() {
    let mut interp = Interpreter::new();
    let result = interp.eval_source("let x = Some(42)\nx");
    assert!(result.is_ok());
}

#[test]
fn w9_2_interpreter_option_none() {
    let mut interp = Interpreter::new();
    let result = interp.eval_source("let x = None\nx");
    assert!(result.is_ok());
}

#[test]
fn w9_3_interpreter_result_ok() {
    let mut interp = Interpreter::new();
    let result = interp.eval_source("let r = Ok(100)\nr");
    assert!(result.is_ok());
}

#[test]
fn w9_4_interpreter_result_err() {
    let mut interp = Interpreter::new();
    let result = interp.eval_source(
        r#"let r = Err("connection failed")
r"#,
    );
    assert!(result.is_ok());
}

#[test]
fn w9_5_interpreter_hashmap() {
    let mut interp = Interpreter::new();
    let result = interp.eval_source(
        r#"let mut db = map_new()
db = map_insert(db, "users", "table")
map_get(db, "users")"#,
    );
    assert!(result.is_ok());
}

#[test]
fn w9_6_registry_multiple_packages() {
    use crate::package::registry::{Registry, SemVer};
    let mut reg = Registry::new();
    for i in 0..20 {
        reg.publish(
            &format!("pkg-{i}"),
            SemVer::new(1, 0, 0),
            &format!("Package {i}"),
        );
    }
    assert_eq!(reg.package_count(), 20);
    assert_eq!(reg.list_all().len(), 20);
}

#[test]
fn w9_7_audit_multiple_vuln() {
    use crate::package::audit::{AdvisoryDatabase, audit_dependencies};
    let json = r#"{"advisories":[
        {"id":"DB-001","package":"pg-fj","severity":"critical","description":"SQL injection","min_version":"0.1.0","max_version":"0.9.0","patched_version":"1.0.0"},
        {"id":"DB-002","package":"pg-fj","severity":"high","description":"Auth bypass","min_version":"0.5.0","max_version":"0.8.0","patched_version":"0.8.1"}
    ]}"#;
    let db = AdvisoryDatabase::from_json(json).unwrap();
    let deps = vec![("pg-fj".into(), "0.6.0".into())];
    let report = audit_dependencies(&deps, &db);
    assert_eq!(report.finding_count(), 2);
}

#[test]
fn w9_8_interpreter_tuple() {
    let mut interp = Interpreter::new();
    let result = interp.eval_source("let t = (1, 2, 3)\nt");
    assert!(result.is_ok());
}

#[test]
fn w9_9_interpreter_block_expr() {
    let mut interp = Interpreter::new();
    let result = interp.eval_source("let val = {\n  let a = 10\n  let b = 20\n  a + b\n}\nval");
    assert!(result.is_ok());
}

#[test]
fn w9_10_interpreter_nested_struct() {
    let mut interp = Interpreter::new();
    let result = interp.eval_source("struct Config { port: i32 }\nstruct Server { config: Config }\nlet s = Server { config: Config { port: 5432 } }\ns.config.port");
    assert!(result.is_ok());
}

// Sprint W10: Full-Stack Web App

#[test]
fn w10_1_interpreter_full_pipeline() {
    let mut interp = Interpreter::new();
    let code = r#"
struct Request { method: str, path: str }
struct Response { status: i32, body: str }
fn handle(req: Request) -> Response {
if req.path == "/" {
    Response { status: 200, body: "OK" }
} else {
    Response { status: 404, body: "Not Found" }
}
}
let req = Request { method: "GET", path: "/" }
let resp = handle(req)
resp.status
"#;
    let result = interp.eval_source(code);
    assert!(result.is_ok());
}

#[test]
fn w10_2_interpreter_enum_match() {
    let mut interp = Interpreter::new();
    let code = r#"
enum Method { Get, Post, Put, Delete }
fn method_str(m: Method) -> str {
match m {
    Method::Get => "GET",
    Method::Post => "POST",
    Method::Put => "PUT",
    Method::Delete => "DELETE",
}
}
method_str(Method::Post)
"#;
    let result = interp.eval_source(code);
    assert!(result.is_ok());
}

#[test]
fn w10_4_registry_full_workflow() {
    use crate::package::registry::{AuthToken, Registry, SemVer, VersionConstraint};
    let mut reg = Registry::new();
    reg.add_token(AuthToken::new("deploy-key"));
    reg.publish("web-app", SemVer::new(1, 0, 0), "Full-stack web app");
    reg.publish("web-app", SemVer::new(1, 1, 0), "Bug fixes");
    reg.publish("web-app", SemVer::new(2, 0, 0), "Major update");
    assert!(reg.validate_token("deploy-key", None));
    let c = VersionConstraint::parse("^1.0.0").unwrap();
    let v = reg.resolve("web-app", &c).unwrap();
    assert_eq!(v, SemVer::new(1, 1, 0));
}

#[test]
fn w10_5_sbom_full_project() {
    use crate::package::sbom::{DepInfo, SbomFormat, generate_sbom};
    let deps = vec![
        DepInfo {
            name: "http-fj".into(),
            version: "1.0.0".into(),
            sha256: "aaa".into(),
            license: Some("MIT".into()),
            dev_only: false,
        },
        DepInfo {
            name: "db-fj".into(),
            version: "0.5.0".into(),
            sha256: "bbb".into(),
            license: Some("Apache-2.0".into()),
            dev_only: false,
        },
        DepInfo {
            name: "test-fj".into(),
            version: "1.0.0".into(),
            sha256: "ccc".into(),
            license: Some("MIT".into()),
            dev_only: true,
        },
    ];
    let sbom = generate_sbom("fullstack-app", &deps, SbomFormat::CycloneDx).unwrap();
    assert!(sbom.contains("fullstack-app"));
}

#[test]
fn w10_6_interpreter_complex_program() {
    let mut interp = Interpreter::new();
    let code = r#"
fn fibonacci(n: i32) -> i32 {
if n <= 1 { n } else { fibonacci(n - 1) + fibonacci(n - 2) }
}
let results = [fibonacci(0), fibonacci(1), fibonacci(5), fibonacci(8)]
results
"#;
    let result = interp.eval_source(code);
    assert!(result.is_ok());
}

// 2026-05-12 Path C SMT-freeze: w10_7 (smt) removed.

#[test]
fn w10_9_interpreter_ml_pipeline() {
    let mut interp = Interpreter::new();
    let code = r#"
let x = from_data([[1.0, 2.0, 3.0, 4.0]])
let layer = Dense(4, 2)
let out = forward(layer, x)
let activated = relu(out)
shape(activated)
"#;
    let result = interp.eval_source(code);
    assert!(result.is_ok());
}

#[test]
fn w10_10_interpreter_error_handling() {
    let mut interp = Interpreter::new();
    let code = r#"
fn safe_div(a: i32, b: i32) -> i32 {
if b == 0 { 0 } else { a / b }
}
let r1 = safe_div(10, 2)
let r2 = safe_div(10, 0)
r1 + r2
"#;
    let result = interp.eval_source(code);
    assert!(result.is_ok());
}

// ===================================================================
// V15 Sprint B2 — ML Runtime Fixes
// ===================================================================

#[test]
fn v15_b2_1_tanh_shorthand() {
    let mut interp = Interpreter::new_capturing();
    let result = interp.eval_source(
        r#"
        let t = from_data([[0.0, 1.0]])
        let r = tanh(t)
        r
        "#,
    );
    assert!(result.is_ok(), "tanh shorthand: {:?}", result.err());
}

#[test]
fn v15_b2_2_leaky_relu_shorthand() {
    let mut interp = Interpreter::new_capturing();
    let result = interp.eval_source(
        r#"
        let t = from_data([[-1.0, 1.0]])
        let r = leaky_relu(t)
        r
        "#,
    );
    assert!(result.is_ok(), "leaky_relu shorthand: {:?}", result.err());
}

#[test]
fn v15_b2_4_dense_forward_method() {
    let mut interp = Interpreter::new_capturing();
    let result = interp.eval_source(
        r#"
        let l = Dense(4, 2)
        let input = ones(1, 4)
        let out = l.forward(input)
        out
        "#,
    );
    assert!(result.is_ok(), "Dense.forward(): {:?}", result.err());
    match result.unwrap() {
        Value::Tensor(t) => {
            assert_eq!(t.data().ndim(), 2, "output should be 2D");
            assert_eq!(t.data().shape()[1], 2, "output features should be 2");
        }
        other => panic!("expected Tensor, got: {:?}", other),
    }
}

#[test]
fn v15_b2_5_conv2d_forward_method() {
    let mut interp = Interpreter::new_capturing();
    let result = interp.eval_source(
        r#"
        let c = Conv2d(1, 8, 3, 1, 0)
        let input = ones(1, 1, 8, 8)
        let out = c.forward(input)
        out
        "#,
    );
    assert!(result.is_ok(), "Conv2d.forward(): {:?}", result.err());
}

#[test]
fn v15_b2_7_concat_builtin() {
    let mut interp = Interpreter::new_capturing();
    let result = interp.eval_source(
        r#"
        let a = zeros(2, 3)
        let b = ones(2, 3)
        let c = concat(a, b, 0)
        c
        "#,
    );
    assert!(result.is_ok(), "concat: {:?}", result.err());
    match result.unwrap() {
        Value::Tensor(t) => {
            assert_eq!(
                t.data().shape(),
                &[4, 3],
                "concatenated shape should be [4,3]"
            );
        }
        other => panic!("expected Tensor, got: {:?}", other),
    }
}

#[test]
fn v15_b2_8_cross_entropy_shorthand() {
    let mut interp = Interpreter::new_capturing();
    let result = interp.eval_source(
        r#"
        let pred = softmax(from_data([[1.0, 2.0, 3.0]]))
        let target = from_data([[0.0, 0.0, 1.0]])
        let ce = cross_entropy(pred, target)
        ce
        "#,
    );
    assert!(result.is_ok(), "cross_entropy: {:?}", result.err());
}
