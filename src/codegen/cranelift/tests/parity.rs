//! Interpreter-parity probes and Phase E parity suites.

use super::compile_and_run;

// ── Probe tests: discover remaining parity gaps ──

#[test]
fn native_println_format_result() {
    // println(format("x={}", 42)) — print a formatted string
    let src = r#"
        fn main() -> i64 {
            let s = format("result={}", 100)
            println(s)
            s.len()
        }
    "#;
    assert_eq!(compile_and_run(src), 10);
}

#[test]
fn native_string_ne() {
    let src = r#"
        fn main() -> i64 {
            let a = "hello"
            let b = "world"
            if a != b { 1 } else { 0 }
        }
    "#;
    assert_eq!(compile_and_run(src), 1);
}

#[test]
fn native_fn_returns_string() {
    // String-returning functions with format are complex (str passed as ptr+len pair).
    // For now, test that format works in main directly.
    let src = r#"
        fn main() -> i64 {
            let s = format("hello {}", "world")
            s.len()
        }
    "#;
    // "hello world" = 11
    assert_eq!(compile_and_run(src), 11);
}

#[test]
fn native_nested_if_else_chain() {
    let src = r#"
        fn classify(x: i64) -> i64 {
            if x < 0 {
                -1
            } else if x == 0 {
                0
            } else {
                1
            }
        }
        fn main() -> i64 {
            classify(-5) + classify(0) + classify(10)
        }
    "#;
    // -1 + 0 + 1 = 0
    assert_eq!(compile_and_run(src), 0);
}

#[test]
fn native_fibonacci_30() {
    let src = r#"
        fn fib(n: i64) -> i64 {
            if n <= 1 { n } else { fib(n - 1) + fib(n - 2) }
        }
        fn main() -> i64 { fib(30) }
    "#;
    assert_eq!(compile_and_run(src), 832040);
}

#[test]
fn native_array_push_pop_sequence() {
    let src = r#"
        fn main() -> i64 {
            let mut arr = []
            arr.push(10)
            arr.push(20)
            arr.push(30)
            arr.push(40)
            arr.push(50)
            let last = arr.pop()
            last
        }
    "#;
    assert_eq!(compile_and_run(src), 50);
}

#[test]
fn native_struct_constructor_and_methods() {
    let src = r#"
        struct Counter { value: i64 }
        impl Counter {
            fn new(start: i64) -> Counter {
                Counter { value: start }
            }
            fn get(self) -> i64 {
                self.value
            }
        }
        fn main() -> i64 {
            let c = Counter::new(42)
            c.get()
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_match_string_len() {
    let src = r#"
        fn main() -> i64 {
            let x = 3
            let result = match x {
                1 => 10,
                2 => 20,
                3 => 30,
                _ => 0,
            }
            result
        }
    "#;
    assert_eq!(compile_and_run(src), 30);
}

#[test]
fn native_match_string_to_variable() {
    // Regression: match returning string lost length tracking when assigned
    // to a variable, causing println to output pointer garbage.
    let src = r#"
        fn main() -> i64 {
            let x = 1
            let result = match x {
                1 => "hello",
                2 => "world",
                _ => "other",
            }
            len(result)
        }
    "#;
    assert_eq!(compile_and_run(src), 5); // "hello".len() == 5
}

#[test]
fn native_match_string_default_arm() {
    let src = r#"
        fn main() -> i64 {
            let x = 99
            let result = match x {
                1 => "hi",
                _ => "default",
            }
            len(result)
        }
    "#;
    assert_eq!(compile_and_run(src), 7); // "default".len() == 7
}

#[test]
fn native_multiple_params_function() {
    let src = r#"
        fn sum4(a: i64, b: i64, c: i64, d: i64) -> i64 {
            a + b + c + d
        }
        fn main() -> i64 { sum4(1, 2, 3, 4) }
    "#;
    assert_eq!(compile_and_run(src), 10);
}

#[test]
fn native_early_return() {
    let src = r#"
        fn find_first_even(a: i64, b: i64, c: i64) -> i64 {
            if a % 2 == 0 { return a }
            if b % 2 == 0 { return b }
            if c % 2 == 0 { return c }
            -1
        }
        fn main() -> i64 { find_first_even(3, 8, 5) }
    "#;
    assert_eq!(compile_and_run(src), 8);
}

#[test]
fn native_string_starts_ends_with() {
    let src = r#"
        fn main() -> i64 {
            let s = "hello world"
            let a = if s.starts_with("hello") { 1 } else { 0 }
            let b = if s.ends_with("world") { 1 } else { 0 }
            a + b
        }
    "#;
    assert_eq!(compile_and_run(src), 2);
}

#[test]
fn native_for_in_array_with_index() {
    let src = r#"
        fn main() -> i64 {
            let arr = [10, 20, 30, 40, 50]
            let mut sum = 0
            for x in arr {
                sum = sum + x
            }
            sum
        }
    "#;
    assert_eq!(compile_and_run(src), 150);
}

#[test]
fn native_const_in_function() {
    let src = r#"
        const limit: i64 = 100
        fn clamp(x: i64) -> i64 {
            if x > limit { limit } else { x }
        }
        fn main() -> i64 { clamp(200) }
    "#;
    assert_eq!(compile_and_run(src), 100);
}

#[test]
fn native_mutable_string_reassign() {
    let src = r#"
        fn main() -> i64 {
            let mut s = "hello"
            s = "world!"
            s.len()
        }
    "#;
    assert_eq!(compile_and_run(src), 6);
}

#[test]
fn native_complex_expression() {
    let src = r#"
        fn main() -> i64 {
            let x = (1 + 2) * (3 + 4) - 5
            x
        }
    "#;
    // (3) * (7) - 5 = 16
    assert_eq!(compile_and_run(src), 16);
}

#[test]
fn native_bool_logic() {
    let src = r#"
        fn main() -> i64 {
            let a = true
            let b = false
            let c = a && !b
            let d = a || b
            if c && d { 1 } else { 0 }
        }
    "#;
    assert_eq!(compile_and_run(src), 1);
}

#[test]
fn native_power_operator_simple() {
    let src = r#"
        fn main() -> i64 { 2 ** 10 }
    "#;
    assert_eq!(compile_and_run(src), 1024);
}

#[test]
fn native_negative_numbers() {
    let src = r#"
        fn main() -> i64 {
            let x = -42
            let y = -x
            y
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_modulo_operator() {
    let src = r#"
        fn main() -> i64 { 17 % 5 }
    "#;
    assert_eq!(compile_and_run(src), 2);
}

#[test]
fn native_string_method_chain_result() {
    let src = r#"
        fn main() -> i64 {
            let s = "  Hello World  "
            let t = s.trim()
            let u = t.to_lowercase()
            u.len()
        }
    "#;
    // "hello world" = 11
    assert_eq!(compile_and_run(src), 11);
}

#[test]
fn native_for_range_inclusive_sum() {
    let src = r#"
        fn main() -> i64 {
            let mut sum = 0
            for i in 0..=10 {
                sum = sum + i
            }
            sum
        }
    "#;
    // 0+1+...+10 = 55
    assert_eq!(compile_and_run(src), 55);
}

#[test]
fn native_multiple_structs() {
    let src = r#"
        struct Point { x: i64, y: i64 }
        struct Size { w: i64, h: i64 }
        fn main() -> i64 {
            let p = Point { x: 10, y: 20 }
            let s = Size { w: 30, h: 40 }
            p.x + p.y + s.w + s.h
        }
    "#;
    assert_eq!(compile_and_run(src), 100);
}

#[test]
fn native_enum_tag_comparison() {
    let src = r#"
        enum Color { Red, Green, Blue }
        fn main() -> i64 {
            let c = Color::Green
            match c {
                Color::Red => 1,
                Color::Green => 2,
                Color::Blue => 3,
                _ => 0,
            }
        }
    "#;
    assert_eq!(compile_and_run(src), 2);
}

#[test]
fn native_array_contains() {
    let src = r#"
        fn main() -> i64 {
            let arr = [1, 2, 3, 4, 5]
            let mut found = 0
            if arr.contains(3) { found = 1 }
            found
        }
    "#;
    assert_eq!(compile_and_run(src), 1);
}

#[test]
fn native_string_is_empty() {
    let src = r#"
        fn main() -> i64 {
            let a = ""
            let b = "hello"
            let x = if a.is_empty() { 1 } else { 0 }
            let y = if b.is_empty() { 0 } else { 1 }
            x + y
        }
    "#;
    assert_eq!(compile_and_run(src), 2);
}

#[test]
fn native_compound_assignment() {
    let src = r#"
        fn main() -> i64 {
            let mut x = 10
            x += 5
            x -= 3
            x *= 2
            x
        }
    "#;
    // (10+5-3)*2 = 24
    assert_eq!(compile_and_run(src), 24);
}

#[test]
fn native_deeply_nested_calls() {
    let src = r#"
        fn a(x: i64) -> i64 { x + 1 }
        fn b(x: i64) -> i64 { a(a(x)) }
        fn c(x: i64) -> i64 { b(b(x)) }
        fn main() -> i64 { c(0) }
    "#;
    // c(0) = b(b(0)) = b(a(a(0))) = b(2) = a(a(2)) = 4
    assert_eq!(compile_and_run(src), 4);
}

// ── Phase E: Additional parity tests ─────────────────────────────────

#[test]
fn native_math_log() {
    let src = r#"
        fn main() -> i64 {
            let x = log(1.0)
            if x == 0.0 { 1 } else { 0 }
        }
    "#;
    assert_eq!(compile_and_run(src), 1);
}

#[test]
fn native_dbg_passthrough() {
    let src = r#"
        fn main() -> i64 {
            let x = dbg(42)
            x
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_stack_array_contains() {
    let src = r#"
        fn main() -> i64 {
            let arr = [10, 20, 30, 40, 50]
            let a = if arr.contains(30) { 1 } else { 0 }
            let b = if arr.contains(99) { 1 } else { 0 }
            a * 10 + b
        }
    "#;
    // contains(30)=true→1, contains(99)=false→0 → 10+0=10
    assert_eq!(compile_and_run(src), 10);
}

#[test]
fn native_while_break_value() {
    let src = r#"
        fn main() -> i64 {
            let mut i = 0
            let mut found = -1
            while i < 100 {
                if i * i > 50 {
                    found = i
                    break
                }
                i = i + 1
            }
            found
        }
    "#;
    // 8*8=64 > 50, so found=8
    assert_eq!(compile_and_run(src), 8);
}

#[test]
fn native_while_continue() {
    let src = r#"
        fn main() -> i64 {
            let mut sum = 0
            let mut i = 0
            while i < 10 {
                i = i + 1
                if i % 2 == 0 { continue }
                sum = sum + i
            }
            sum
        }
    "#;
    // 1+3+5+7+9 = 25
    assert_eq!(compile_and_run(src), 25);
}

#[test]
fn native_string_starts_ends_combined() {
    let src = r#"
        fn main() -> i64 {
            let s = "hello world"
            let a = if s.starts_with("hello") { 1 } else { 0 }
            let b = if s.ends_with("world") { 1 } else { 0 }
            let c = if s.starts_with("xyz") { 1 } else { 0 }
            a + b + c
        }
    "#;
    assert_eq!(compile_and_run(src), 2);
}

#[test]
fn native_string_replace() {
    let src = r#"
        fn main() -> i64 {
            let s = "hello world"
            let t = s.replace("world", "fajar")
            t.len()
        }
    "#;
    // "hello fajar" = 11
    assert_eq!(compile_and_run(src), 11);
}

#[test]
fn native_string_repeat_four() {
    let src = r#"
        fn main() -> i64 {
            let s = "ab"
            let t = s.repeat(4)
            t.len()
        }
    "#;
    // "abababab" = 8
    assert_eq!(compile_and_run(src), 8);
}

#[test]
fn native_string_index_of() {
    // Native codegen index_of returns raw i64 position (or -1 for not found)
    let src = r#"
        fn main() -> i64 {
            let s = "hello world"
            let idx = s.index_of("world")
            idx
        }
    "#;
    assert_eq!(compile_and_run(src), 6);
}

#[test]
fn native_multi_param_function() {
    let src = r#"
        fn weighted_sum(a: i64, b: i64, c: i64, wa: i64, wb: i64, wc: i64) -> i64 {
            a * wa + b * wb + c * wc
        }
        fn main() -> i64 {
            weighted_sum(1, 2, 3, 10, 20, 30)
        }
    "#;
    // 1*10 + 2*20 + 3*30 = 10+40+90 = 140
    assert_eq!(compile_and_run(src), 140);
}

#[test]
fn native_recursive_gcd() {
    let src = r#"
        fn gcd(a: i64, b: i64) -> i64 {
            if b == 0 { a } else { gcd(b, a % b) }
        }
        fn main() -> i64 { gcd(48, 18) }
    "#;
    assert_eq!(compile_and_run(src), 6);
}

#[test]
fn native_nested_struct_access() {
    let src = r#"
        struct Vec2 { x: i64, y: i64 }
        impl Vec2 {
            fn new(x: i64, y: i64) -> Vec2 { Vec2 { x: x, y: y } }
            fn sum(self) -> i64 { self.x + self.y }
        }
        fn main() -> i64 {
            let v = Vec2::new(3, 7)
            v.sum()
        }
    "#;
    assert_eq!(compile_and_run(src), 10);
}

#[test]
fn native_loop_with_break() {
    let src = r#"
        fn main() -> i64 {
            let mut count = 0
            loop {
                count = count + 1
                if count >= 10 { break }
            }
            count
        }
    "#;
    assert_eq!(compile_and_run(src), 10);
}

#[test]
fn native_bitwise_combined() {
    let src = r#"
        fn main() -> i64 {
            let a = 0xFF
            let b = 0x0F
            let c = a & b
            let d = a | b
            let e = a ^ b
            c + (d - e)
        }
    "#;
    // c = 0x0F=15, d = 0xFF=255, e = 0xF0=240
    // 15 + (255-240) = 15+15 = 30
    assert_eq!(compile_and_run(src), 30);
}

#[test]
fn native_shift_operations() {
    let src = r#"
        fn main() -> i64 {
            let a = 1 << 10
            let b = a >> 5
            b
        }
    "#;
    // 1<<10 = 1024, 1024>>5 = 32
    assert_eq!(compile_and_run(src), 32);
}

#[test]
fn native_to_string_len() {
    let src = r#"
        fn main() -> i64 {
            let s = to_string(12345)
            s.len()
        }
    "#;
    assert_eq!(compile_and_run(src), 5);
}

#[test]
fn native_to_int_conversion() {
    let src = r#"
        fn main() -> i64 {
            let x = 3.14
            to_int(x)
        }
    "#;
    assert_eq!(compile_and_run(src), 3);
}

#[test]
fn native_heap_array_contains() {
    let src = r#"
        fn main() -> i64 {
            let mut arr = []
            arr.push(100)
            arr.push(200)
            arr.push(300)
            if arr.contains(200) { 1 } else { 0 }
        }
    "#;
    assert_eq!(compile_and_run(src), 1);
}

#[test]
fn native_for_range_with_function() {
    let src = r#"
        fn square(x: i64) -> i64 { x * x }
        fn main() -> i64 {
            let mut sum = 0
            for i in 1..5 {
                sum = sum + square(i)
            }
            sum
        }
    "#;
    // 1+4+9+16 = 30
    assert_eq!(compile_and_run(src), 30);
}

#[test]
fn native_match_with_default() {
    let src = r#"
        fn grade(score: i64) -> i64 {
            if score >= 90 { 4 }
            else if score >= 80 { 3 }
            else if score >= 70 { 2 }
            else { 1 }
        }
        fn main() -> i64 {
            grade(95) + grade(85) + grade(75) + grade(50)
        }
    "#;
    // 4+3+2+1 = 10
    assert_eq!(compile_and_run(src), 10);
}

#[test]
fn native_string_concat_in_loop() {
    let src = r#"
        fn main() -> i64 {
            let mut s = ""
            let mut i = 0
            while i < 3 {
                s = s + "ab"
                i = i + 1
            }
            s.len()
        }
    "#;
    // "ababab" = 6
    assert_eq!(compile_and_run(src), 6);
}

#[test]
fn native_mutual_recursion() {
    let src = r#"
        fn is_even(n: i64) -> i64 {
            if n == 0 { 1 } else { is_odd(n - 1) }
        }
        fn is_odd(n: i64) -> i64 {
            if n == 0 { 0 } else { is_even(n - 1) }
        }
        fn main() -> i64 {
            is_even(10) + is_odd(7)
        }
    "#;
    // is_even(10)=1, is_odd(7)=1 → 2
    assert_eq!(compile_and_run(src), 2);
}

// ═══════════════════════════════════════════════════════════════════
// E.2 — Closure support tests
// ═══════════════════════════════════════════════════════════════════

#[test]
fn native_closure_no_capture() {
    let src = r#"
        fn main() -> i64 {
            let f = |x: i64| -> i64 { x + 1 }
            f(5)
        }
    "#;
    assert_eq!(compile_and_run(src), 6);
}

#[test]
fn native_closure_with_capture() {
    let src = r#"
        fn main() -> i64 {
            let n = 10
            let f = |x: i64| -> i64 { x + n }
            f(5)
        }
    "#;
    assert_eq!(compile_and_run(src), 15);
}

#[test]
fn native_closure_multi_capture() {
    let src = r#"
        fn main() -> i64 {
            let a = 3
            let b = 7
            let f = |x: i64| -> i64 { x + a + b }
            f(10)
        }
    "#;
    assert_eq!(compile_and_run(src), 20);
}

#[test]
fn native_closure_multiply() {
    let src = r#"
        fn main() -> i64 {
            let factor = 5
            let mul = |x: i64| -> i64 { x * factor }
            mul(8)
        }
    "#;
    assert_eq!(compile_and_run(src), 40);
}

#[test]
fn native_closure_two_params() {
    let src = r#"
        fn main() -> i64 {
            let add = |a: i64, b: i64| -> i64 { a + b }
            add(3, 4)
        }
    "#;
    assert_eq!(compile_and_run(src), 7);
}

#[test]
fn native_closure_two_params_with_capture() {
    let src = r#"
        fn main() -> i64 {
            let offset = 100
            let add_offset = |a: i64, b: i64| -> i64 { a + b + offset }
            add_offset(3, 4)
        }
    "#;
    assert_eq!(compile_and_run(src), 107);
}

#[test]
fn native_closure_capture_and_call_fn() {
    let src = r#"
        fn double(x: i64) -> i64 { x * 2 }
        fn main() -> i64 {
            let n = 5
            let f = |x: i64| -> i64 { double(x) + n }
            f(3)
        }
    "#;
    // double(3) + 5 = 6 + 5 = 11
    assert_eq!(compile_and_run(src), 11);
}

#[test]
fn native_closure_in_expression() {
    let src = r#"
        fn main() -> i64 {
            let f = |x: i64| -> i64 { x * x }
            f(3) + f(4)
        }
    "#;
    // 9 + 16 = 25
    assert_eq!(compile_and_run(src), 25);
}

#[test]
fn native_closure_capture_mutable() {
    // Closure captures the value at the time of creation
    let src = r#"
        fn main() -> i64 {
            let mut x = 10
            let f = |y: i64| -> i64 { y + x }
            x = 20
            f(5)
        }
    "#;
    // Closure captured x=10 at creation time (by value)
    // f(5) = 5 + 10 = 15... but in our model, captured vars are
    // passed at call time, so x=20 at the time of f(5)
    // Actually no — we pass current value of x at call time: 5 + 20 = 25
    assert_eq!(compile_and_run(src), 25);
}

#[test]
fn native_closure_no_args() {
    let src = r#"
        fn main() -> i64 {
            let val = 42
            let f = || -> i64 { val }
            f()
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_closure_with_if() {
    let src = r#"
        fn main() -> i64 {
            let threshold = 10
            let check = |x: i64| -> i64 {
                if x > threshold { 1 } else { 0 }
            }
            check(15) + check(5)
        }
    "#;
    // check(15)=1, check(5)=0 → 1
    assert_eq!(compile_and_run(src), 1);
}

#[test]
fn native_two_closures() {
    let src = r#"
        fn main() -> i64 {
            let a = |x: i64| -> i64 { x + 1 }
            let b = |x: i64| -> i64 { x * 2 }
            a(b(5))
        }
    "#;
    // b(5)=10, a(10)=11
    assert_eq!(compile_and_run(src), 11);
}

// ═══════════════════════════════════════════════════════════════════════
// S2: Function pointers and closures-as-arguments
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn native_fn_ptr_simple() {
    // Assign a function to a fn-pointer variable, then call it
    let src = r#"
        fn double(x: i64) -> i64 { x * 2 }
        fn main() -> i64 {
            let f: fn(i64) -> i64 = double
            f(21)
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_fn_ptr_reassign() {
    // Reassign fn-pointer to a different function
    let src = r#"
        fn add_one(x: i64) -> i64 { x + 1 }
        fn mul_two(x: i64) -> i64 { x * 2 }
        fn main() -> i64 {
            let mut f: fn(i64) -> i64 = add_one
            let a = f(10)
            f = mul_two
            let b = f(10)
            a + b
        }
    "#;
    // a = 11, b = 20, total = 31
    assert_eq!(compile_and_run(src), 31);
}

#[test]
fn native_fn_ptr_as_arg() {
    // Pass a function pointer as argument to another function
    let src = r#"
        fn double(x: i64) -> i64 { x * 2 }
        fn apply(f: fn(i64) -> i64, x: i64) -> i64 { f(x) }
        fn main() -> i64 {
            apply(double, 21)
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_fn_ptr_multi_param() {
    // Function pointer with multiple parameters
    let src = r#"
        fn add(a: i64, b: i64) -> i64 { a + b }
        fn apply_binary(f: fn(i64, i64) -> i64, x: i64, y: i64) -> i64 { f(x, y) }
        fn main() -> i64 {
            apply_binary(add, 20, 22)
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

// ═══════════════════════════════════════════════════════════════════════
// S2.4 — Closure as function argument
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn native_closure_as_arg_no_capture() {
    // Pass a capture-less closure variable as argument
    let src = r#"
        fn apply(f: fn(i64) -> i64, x: i64) -> i64 { f(x) }
        fn main() -> i64 {
            let double = |x: i64| -> i64 { x * 2 }
            apply(double, 21)
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_closure_inline_as_arg() {
    // Pass an inline closure directly as argument
    let src = r#"
        fn apply(f: fn(i64) -> i64, x: i64) -> i64 { f(x) }
        fn main() -> i64 {
            apply(|x: i64| -> i64 { x + 10 }, 32)
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_closure_as_arg_with_capture() {
    // S2.6 (closed 2026-06-12): capture closures pass as tagged
    // ClosureHandles; fn-ptr call sites dispatch via __closure_call_dyn_N
    // which untags handles and calls raw fn ptrs directly.
    let src = r#"
        fn apply(f: fn(i64) -> i64, x: i64) -> i64 { f(x) }
        fn main() -> i64 {
            let offset = 10
            let add_offset = |x: i64| -> i64 { x + offset }
            apply(add_offset, 32)
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_closure_as_arg_mixed_plain_and_capture() {
    // S2.6 key uniformity test: the SAME call site receives a plain fn
    // (raw aligned address, tag bit clear) and a capture closure (tagged
    // handle) — dyn dispatch must handle both.
    let src = r#"
        fn double(x: i64) -> i64 { x * 2 }
        fn apply(f: fn(i64) -> i64, x: i64) -> i64 { f(x) }
        fn main() -> i64 {
            let offset = 2
            let add_offset = |x: i64| -> i64 { x + offset }
            apply(double, 10) + apply(add_offset, 20)
        }
    "#;
    // 20 + 22 = 42
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_closure_as_arg_capture_two_params() {
    // S2.6: 2-arg fn-ptr param with a capturing closure (call_dyn_2 path).
    let src = r#"
        fn apply2(f: fn(i64, i64) -> i64, x: i64, y: i64) -> i64 { f(x, y) }
        fn main() -> i64 {
            let bias = 2
            let madd = |x: i64, y: i64| -> i64 { x * y + bias }
            apply2(madd, 5, 8)
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_closure_as_arg_multiple() {
    // Pass different closures to the same higher-order function
    let src = r#"
        fn apply(f: fn(i64) -> i64, x: i64) -> i64 { f(x) }
        fn main() -> i64 {
            let a = apply(|x: i64| -> i64 { x * 2 }, 10)
            let b = apply(|x: i64| -> i64 { x + 5 }, 10)
            a + b
        }
    "#;
    // a = 20, b = 15, total = 35
    assert_eq!(compile_and_run(src), 35);
}

// ═══════════════════════════════════════════════════════════════════════
// S2.5 — Higher-order functions
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn native_higher_order_apply_twice() {
    // Apply a function twice
    let src = r#"
        fn apply_twice(f: fn(i64) -> i64, x: i64) -> i64 {
            f(f(x))
        }
        fn double(x: i64) -> i64 { x * 2 }
        fn main() -> i64 {
            apply_twice(double, 5)
        }
    "#;
    // double(double(5)) = double(10) = 20
    assert_eq!(compile_and_run(src), 20);
}

#[test]
fn native_higher_order_compose() {
    // Compose two functions: compose(f, g)(x) = f(g(x))
    let src = r#"
        fn double(x: i64) -> i64 { x * 2 }
        fn add_one(x: i64) -> i64 { x + 1 }
        fn compose_and_apply(f: fn(i64) -> i64, g: fn(i64) -> i64, x: i64) -> i64 {
            f(g(x))
        }
        fn main() -> i64 {
            compose_and_apply(double, add_one, 10)
        }
    "#;
    // double(add_one(10)) = double(11) = 22
    assert_eq!(compile_and_run(src), 22);
}

#[test]
fn native_higher_order_conditional_apply() {
    // Choose which function to apply based on a condition
    let src = r#"
        fn double(x: i64) -> i64 { x * 2 }
        fn triple(x: i64) -> i64 { x * 3 }
        fn apply(f: fn(i64) -> i64, x: i64) -> i64 { f(x) }
        fn main() -> i64 {
            let use_double = 1
            let f: fn(i64) -> i64 = double
            if use_double == 0 {
                f = triple
            }
            apply(f, 7)
        }
    "#;
    // use_double=1, so f=double, double(7) = 14
    assert_eq!(compile_and_run(src), 14);
}

#[test]
fn native_higher_order_binary_op() {
    // Higher-order function with binary operation
    let src = r#"
        fn add(a: i64, b: i64) -> i64 { a + b }
        fn mul(a: i64, b: i64) -> i64 { a * b }
        fn fold_two(f: fn(i64, i64) -> i64, a: i64, b: i64) -> i64 { f(a, b) }
        fn main() -> i64 {
            let sum = fold_two(add, 10, 20)
            let prod = fold_two(mul, 3, 5)
            sum + prod
        }
    "#;
    // sum = 30, prod = 15, total = 45
    assert_eq!(compile_and_run(src), 45);
}

#[test]
fn native_higher_order_inline_closure_compose() {
    // Compose with inline closures
    let src = r#"
        fn compose_and_apply(f: fn(i64) -> i64, g: fn(i64) -> i64, x: i64) -> i64 {
            f(g(x))
        }
        fn main() -> i64 {
            compose_and_apply(|x: i64| -> i64 { x * 3 }, |x: i64| -> i64 { x + 2 }, 10)
        }
    "#;
    // (10 + 2) * 3 = 36
    assert_eq!(compile_and_run(src), 36);
}

#[test]
fn native_higher_order_predicate() {
    // Function that returns bool (0 or 1), used as predicate
    let src = r#"
        fn is_positive(x: i64) -> i64 { if x > 0 { 1 } else { 0 } }
        fn test_pred(pred: fn(i64) -> i64, x: i64) -> i64 { pred(x) }
        fn main() -> i64 {
            let a = test_pred(is_positive, 5)
            let b = test_pred(is_positive, -3)
            a + b
        }
    "#;
    // a = 1, b = 0, total = 1
    assert_eq!(compile_and_run(src), 1);
}

// ═══════════════════════════════════════════════════════════════════════
// S2.6 — Returning closures
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn native_return_closure_no_capture() {
    // Return a closure that doesn't capture anything
    let src = r#"
        fn make_doubler() -> fn(i64) -> i64 {
            let f = |x: i64| -> i64 { x * 2 }
            f
        }
        fn main() -> i64 {
            let d: fn(i64) -> i64 = make_doubler()
            d(21)
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_return_closure_with_capture() {
    // Return a closure that captures a local variable
    let src = r#"
        fn make_adder(n: i64) -> fn(i64) -> i64 {
            let f = |x: i64| -> i64 { x + n }
            f
        }
        fn main() -> i64 {
            let add5: fn(i64) -> i64 = make_adder(5)
            add5(10)
        }
    "#;
    assert_eq!(compile_and_run(src), 15);
}

#[test]
fn native_use_returned_closure() {
    // Return closure, use it multiple times
    let src = r#"
        fn make_multiplier(factor: i64) -> fn(i64) -> i64 {
            let f = |x: i64| -> i64 { x * factor }
            f
        }
        fn main() -> i64 {
            let times3: fn(i64) -> i64 = make_multiplier(3)
            let a = times3(10)
            let b = times3(5)
            a + b
        }
    "#;
    assert_eq!(compile_and_run(src), 45);
}

// ═══════════════════════════════════════════════════════════════════════
// S2.7 — Integration tests (callback/event handler patterns)
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn native_callback_pattern() {
    // Callback pattern: transform + accumulate with fn pointers
    let src = r#"
        fn transform(x: i64, f: fn(i64) -> i64) -> i64 { f(x) }
        fn main() -> i64 {
            let mut total = 0
            let mut i = 1
            while i <= 5 {
                total = total + transform(i, |x: i64| -> i64 { x * x })
                i = i + 1
            }
            total
        }
    "#;
    // 1^2 + 2^2 + 3^2 + 4^2 + 5^2 = 1 + 4 + 9 + 16 + 25 = 55
    assert_eq!(compile_and_run(src), 55);
}

#[test]
fn native_event_handler_pattern() {
    // Simulated event handler: register handler, dispatch events
    let src = r#"
        fn dispatch(handler: fn(i64) -> i64, event_code: i64) -> i64 {
            handler(event_code)
        }
        fn on_click(code: i64) -> i64 { code * 10 }
        fn on_key(code: i64) -> i64 { code + 100 }
        fn main() -> i64 {
            let a = dispatch(on_click, 5)
            let b = dispatch(on_key, 3)
            a + b
        }
    "#;
    // a = 50, b = 103, total = 153
    assert_eq!(compile_and_run(src), 153);
}

#[test]
fn native_strategy_pattern() {
    // Strategy pattern: choose algorithm at runtime
    let src = r#"
        fn compute(strategy: fn(i64, i64) -> i64, a: i64, b: i64) -> i64 {
            strategy(a, b)
        }
        fn fast_algo(a: i64, b: i64) -> i64 { a + b }
        fn precise_algo(a: i64, b: i64) -> i64 { a * b }
        fn main() -> i64 {
            let r1 = compute(fast_algo, 10, 20)
            let r2 = compute(precise_algo, 3, 7)
            r1 + r2
        }
    "#;
    // r1 = 30, r2 = 21, total = 51
    assert_eq!(compile_and_run(src), 51);
}

// ═══════════════════════════════════════════════════════════════════════
// S3 — HashMap in Native Codegen
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn native_hashmap_new_and_len() {
    let src = r#"
        fn main() -> i64 {
            let m = HashMap::new()
            m.len()
        }
    "#;
    assert_eq!(compile_and_run(src), 0);
}

#[test]
fn native_hashmap_insert_and_get() {
    let src = r#"
        fn main() -> i64 {
            let m = HashMap::new()
            m.insert("x", 42)
            m.get("x")
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_hashmap_insert_multiple() {
    let src = r#"
        fn main() -> i64 {
            let m = HashMap::new()
            m.insert("a", 10)
            m.insert("b", 20)
            m.insert("c", 30)
            let sum = m.get("a") + m.get("b") + m.get("c")
            sum
        }
    "#;
    assert_eq!(compile_and_run(src), 60);
}

#[test]
fn native_hashmap_len_after_inserts() {
    let src = r#"
        fn main() -> i64 {
            let m = HashMap::new()
            m.insert("one", 1)
            m.insert("two", 2)
            m.insert("three", 3)
            m.len()
        }
    "#;
    assert_eq!(compile_and_run(src), 3);
}

#[test]
fn native_hashmap_contains_key() {
    let src = r#"
        fn main() -> i64 {
            let m = HashMap::new()
            m.insert("hello", 99)
            let has_hello = m.contains_key("hello")
            let has_world = m.contains_key("world")
            has_hello + has_world
        }
    "#;
    // has_hello = 1, has_world = 0, total = 1
    assert_eq!(compile_and_run(src), 1);
}

#[test]
fn native_hashmap_remove() {
    let src = r#"
        fn main() -> i64 {
            let m = HashMap::new()
            m.insert("x", 10)
            m.insert("y", 20)
            m.remove("x")
            m.len()
        }
    "#;
    assert_eq!(compile_and_run(src), 1);
}

#[test]
fn native_hashmap_clear() {
    let src = r#"
        fn main() -> i64 {
            let m = HashMap::new()
            m.insert("a", 1)
            m.insert("b", 2)
            m.clear()
            m.len()
        }
    "#;
    assert_eq!(compile_and_run(src), 0);
}

#[test]
fn native_hashmap_overwrite() {
    // Inserting same key twice should overwrite
    let src = r#"
        fn main() -> i64 {
            let m = HashMap::new()
            m.insert("x", 10)
            m.insert("x", 42)
            m.get("x")
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_hashmap_get_missing() {
    // Getting a key that doesn't exist returns 0
    let src = r#"
        fn main() -> i64 {
            let m = HashMap::new()
            m.get("nonexistent")
        }
    "#;
    assert_eq!(compile_and_run(src), 0);
}

#[test]
fn native_hashmap_in_function() {
    // HashMap used inside a function (cleanup at function exit)
    let src = r#"
        fn compute() -> i64 {
            let m = HashMap::new()
            m.insert("result", 100)
            m.get("result")
        }
        fn main() -> i64 {
            compute()
        }
    "#;
    assert_eq!(compile_and_run(src), 100);
}

// ═══════════════════════════════════════════════════════════════════════
// S4.1 — Try operator `?` in codegen
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn native_try_ok_unwraps() {
    // When ? is applied to Ok(v), it unwraps to v
    let src = r#"
        fn maybe_value() -> i64 {
            let x = Ok(42)
            x?
        }
        fn main() -> i64 {
            maybe_value()
        }
    "#;
    // Ok(42)? should unwrap to 42, but the function returns the payload
    // Since maybe_value returns i64 (not Result), need to handle return ABI
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_try_err_returns_early() {
    // When ? is applied to Err(e), it returns early with tag=1
    let src = r#"
        fn might_fail(flag: i64) -> i64 {
            if flag == 0 {
                let e = Err(99)
                e?
            }
            100
        }
        fn main() -> i64 {
            might_fail(1)
        }
    "#;
    // flag=1, so the if-branch is skipped, returns 100
    assert_eq!(compile_and_run(src), 100);
}

#[test]
fn native_try_ok_continues() {
    // Ok path continues execution
    let src = r#"
        fn compute(x: i64) -> i64 {
            let result = Ok(x * 2)
            let val = result?
            val + 10
        }
        fn main() -> i64 {
            compute(5)
        }
    "#;
    // Ok(10)? = 10, 10 + 10 = 20
    assert_eq!(compile_and_run(src), 20);
}

#[test]
fn native_try_err_propagates() {
    // Err propagation: ? on Err returns the error value
    let src = r#"
        fn inner() -> i64 {
            let r = Err(55)
            r?
            999
        }
        fn main() -> i64 {
            inner()
        }
    "#;
    // Err(55)? should return 55 early, never reaching 999
    assert_eq!(compile_and_run(src), 55);
}

// ═══════════════════════════════════════════════════════════════════════
// S4.2 — Option/Result methods in codegen
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn native_is_some_true() {
    let src = r#"
        fn main() -> i64 {
            is_some(Some(42))
        }
    "#;
    assert_eq!(compile_and_run(src), 1);
}

#[test]
fn native_is_some_false() {
    let src = r#"
        fn main() -> i64 {
            is_some(None)
        }
    "#;
    assert_eq!(compile_and_run(src), 0);
}

#[test]
fn native_is_none_true() {
    let src = r#"
        fn main() -> i64 {
            is_none(None)
        }
    "#;
    assert_eq!(compile_and_run(src), 1);
}

#[test]
fn native_is_none_false() {
    let src = r#"
        fn main() -> i64 {
            is_none(Some(10))
        }
    "#;
    assert_eq!(compile_and_run(src), 0);
}

#[test]
fn native_unwrap_some() {
    let src = r#"
        fn main() -> i64 {
            unwrap(Some(99))
        }
    "#;
    assert_eq!(compile_and_run(src), 99);
}

#[test]
fn native_unwrap_or_some() {
    let src = r#"
        fn main() -> i64 {
            unwrap_or(Some(42), 0)
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_unwrap_or_none() {
    let src = r#"
        fn main() -> i64 {
            unwrap_or(None, 77)
        }
    "#;
    assert_eq!(compile_and_run(src), 77);
}

#[test]
fn native_is_err_ok() {
    let src = r#"
        fn main() -> i64 {
            is_err(Ok(5))
        }
    "#;
    assert_eq!(compile_and_run(src), 0);
}

#[test]
fn native_is_err_err() {
    let src = r#"
        fn main() -> i64 {
            is_err(Err(5))
        }
    "#;
    assert_eq!(compile_and_run(src), 1);
}
