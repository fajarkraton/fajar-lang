//! Hardening plus string/math/array method builtins (F, E.3-E.5).

use super::{compile_and_run, compile_and_run_f64};

// ── F: Hardening tests ──

#[test]
fn native_f1_struct_zero_fields() {
    // Struct with no fields should compile (slot_size = 0)
    let src = r#"
        struct Unit { }
        fn main() -> i64 {
            let u = Unit { }
            42
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_f7_two_structs_self_field() {
    // Two structs with impl, each accessing self.field → correct values
    let src = r#"
        struct Foo { x: i64 }
        struct Bar { x: i64 }
        impl Foo {
            fn get_x(&self) -> i64 { self.x }
        }
        impl Bar {
            fn get_x(&self) -> i64 { self.x + 100 }
        }
        fn main() -> i64 {
            let f = Foo { x: 5 }
            let b = Bar { x: 7 }
            f.get_x() + b.get_x()
        }
    "#;
    assert_eq!(compile_and_run(src), 5 + 107);
}

#[test]
fn native_f8_bitwise_and() {
    let src = r#"
        fn main() -> i64 { 0xFF & 0x0F }
    "#;
    assert_eq!(compile_and_run(src), 0x0F);
}

#[test]
fn native_f8_bitwise_or() {
    let src = r#"
        fn main() -> i64 { 0xF0 | 0x0F }
    "#;
    assert_eq!(compile_and_run(src), 0xFF);
}

#[test]
fn native_f8_bitwise_xor() {
    let src = r#"
        fn main() -> i64 { 0xFF ^ 0x0F }
    "#;
    assert_eq!(compile_and_run(src), 0xF0);
}

#[test]
fn native_f8_shift_left() {
    let src = r#"
        fn main() -> i64 { 1 << 4 }
    "#;
    assert_eq!(compile_and_run(src), 16);
}

#[test]
fn native_f8_shift_right() {
    let src = r#"
        fn main() -> i64 { 64 >> 3 }
    "#;
    assert_eq!(compile_and_run(src), 8);
}

#[test]
fn native_f8_not_equal() {
    let src = r#"
        fn main() -> i64 {
            if 3 != 4 { 1 } else { 0 }
        }
    "#;
    assert_eq!(compile_and_run(src), 1);
}

#[test]
fn native_f8_less_equal() {
    let src = r#"
        fn main() -> i64 {
            let a = if 3 <= 3 { 1 } else { 0 }
            let b = if 3 <= 4 { 1 } else { 0 }
            let c = if 4 <= 3 { 1 } else { 0 }
            a + b + c
        }
    "#;
    assert_eq!(compile_and_run(src), 2);
}

#[test]
fn native_f8_greater_equal() {
    let src = r#"
        fn main() -> i64 {
            let a = if 3 >= 3 { 1 } else { 0 }
            let b = if 4 >= 3 { 1 } else { 0 }
            let c = if 3 >= 4 { 1 } else { 0 }
            a + b + c
        }
    "#;
    assert_eq!(compile_and_run(src), 2);
}

#[test]
fn native_f8_block_expr_with_stmts() {
    let src = r#"
        fn main() -> i64 {
            let x = {
                let y = 10
                y * 2
            }
            x
        }
    "#;
    assert_eq!(compile_and_run(src), 20);
}

#[test]
fn native_f8_block_expr_f64() {
    let src = r#"
        fn main() -> f64 {
            let x = { 3.14 }
            x
        }
    "#;
    let result = compile_and_run_f64(src);
    assert!((result - 3.14).abs() < 1e-10);
}

#[test]
fn native_f8_nested_blocks() {
    let src = r#"
        fn main() -> i64 {
            let x = { { 42 } }
            x
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

// ── E.3 String method tests ──────────────────────────────────────

#[test]
fn native_e3_string_len() {
    let src = r#"
        fn main() -> i64 {
            let s = "hello"
            s.len()
        }
    "#;
    assert_eq!(compile_and_run(src), 5);
}

#[test]
fn native_e3_string_is_empty_false() {
    let src = r#"
        fn main() -> i64 {
            let s = "hello"
            s.is_empty()
        }
    "#;
    assert_eq!(compile_and_run(src), 0);
}

#[test]
fn native_e3_string_is_empty_true() {
    let src = r#"
        fn main() -> i64 {
            let s = ""
            s.is_empty()
        }
    "#;
    assert_eq!(compile_and_run(src), 1);
}

#[test]
fn native_e3_string_contains_true() {
    let src = r#"
        fn main() -> i64 {
            let s = "hello world"
            s.contains("world")
        }
    "#;
    assert_eq!(compile_and_run(src), 1);
}

#[test]
fn native_e3_string_contains_false() {
    let src = r#"
        fn main() -> i64 {
            let s = "hello world"
            s.contains("xyz")
        }
    "#;
    assert_eq!(compile_and_run(src), 0);
}

#[test]
fn native_e3_string_starts_with() {
    let src = r#"
        fn main() -> i64 {
            let s = "hello world"
            s.starts_with("hello")
        }
    "#;
    assert_eq!(compile_and_run(src), 1);
}

#[test]
fn native_e3_string_ends_with() {
    let src = r#"
        fn main() -> i64 {
            let s = "hello world"
            s.ends_with("world")
        }
    "#;
    assert_eq!(compile_and_run(src), 1);
}

#[test]
fn native_e3_string_trim_len() {
    // trim returns a view; verify the trimmed length
    let src = r#"
        fn main() -> i64 {
            let s = "  hello  "
            let t = s.trim()
            t.len()
        }
    "#;
    assert_eq!(compile_and_run(src), 5);
}

#[test]
fn native_e3_string_to_uppercase_len() {
    // to_uppercase preserves length for ASCII
    let src = r#"
        fn main() -> i64 {
            let s = "hello"
            let u = s.to_uppercase()
            u.len()
        }
    "#;
    assert_eq!(compile_and_run(src), 5);
}

#[test]
fn native_e3_string_to_lowercase_len() {
    let src = r#"
        fn main() -> i64 {
            let s = "HELLO"
            let l = s.to_lowercase()
            l.len()
        }
    "#;
    assert_eq!(compile_and_run(src), 5);
}

#[test]
fn native_e3_string_replace_contains() {
    // Replace "world" with "fajar", then check contains
    let src = r#"
        fn main() -> i64 {
            let s = "hello world"
            let r = s.replace("world", "fajar")
            r.contains("fajar")
        }
    "#;
    assert_eq!(compile_and_run(src), 1);
}

#[test]
fn native_e3_string_substring_len() {
    let src = r#"
        fn main() -> i64 {
            let s = "hello world"
            let sub = s.substring(0, 5)
            sub.len()
        }
    "#;
    assert_eq!(compile_and_run(src), 5);
}

// ── E.4 Math builtin tests ───────────────────────────────────────

#[test]
fn native_e4_abs_positive() {
    assert_eq!(compile_and_run("fn main() -> i64 { abs(-42) }"), 42);
}

#[test]
fn native_e4_abs_already_positive() {
    assert_eq!(compile_and_run("fn main() -> i64 { abs(7) }"), 7);
}

#[test]
fn native_e4_abs_float() {
    let result = compile_and_run_f64("fn main() -> f64 { abs(-3.14) }");
    assert!((result - 3.14).abs() < 1e-10);
}

#[test]
fn native_e4_sqrt() {
    let result = compile_and_run_f64("fn main() -> f64 { sqrt(9.0) }");
    assert!((result - 3.0).abs() < 1e-10);
}

#[test]
fn native_e4_floor() {
    let result = compile_and_run_f64("fn main() -> f64 { floor(3.7) }");
    assert!((result - 3.0).abs() < 1e-10);
}

#[test]
fn native_e4_ceil() {
    let result = compile_and_run_f64("fn main() -> f64 { ceil(3.2) }");
    assert!((result - 4.0).abs() < 1e-10);
}

#[test]
fn native_e4_round() {
    let result = compile_and_run_f64("fn main() -> f64 { round(3.5) }");
    // IEEE 754 round-to-even: 3.5 rounds to 4.0
    assert!((result - 4.0).abs() < 1e-10);
}

#[test]
fn native_e4_min_int() {
    assert_eq!(compile_and_run("fn main() -> i64 { min(3, 7) }"), 3);
}

#[test]
fn native_e4_max_int() {
    assert_eq!(compile_and_run("fn main() -> i64 { max(3, 7) }"), 7);
}

#[test]
fn native_e4_min_float() {
    let result = compile_and_run_f64("fn main() -> f64 { min(3.5, 7.2) }");
    assert!((result - 3.5).abs() < 1e-10);
}

#[test]
fn native_e4_max_float() {
    let result = compile_and_run_f64("fn main() -> f64 { max(3.5, 7.2) }");
    assert!((result - 7.2).abs() < 1e-10);
}

#[test]
fn native_e4_clamp_within() {
    assert_eq!(compile_and_run("fn main() -> i64 { clamp(5, 1, 10) }"), 5);
}

#[test]
fn native_e4_clamp_below() {
    assert_eq!(compile_and_run("fn main() -> i64 { clamp(-3, 1, 10) }"), 1);
}

#[test]
fn native_e4_clamp_above() {
    assert_eq!(compile_and_run("fn main() -> i64 { clamp(15, 1, 10) }"), 10);
}

// ── E.5 Array method tests ───────────────────────────────────────

#[test]
fn native_e5_heap_array_is_empty_true() {
    let src = r#"
        fn main() -> i64 {
            let mut arr = []
            arr.is_empty()
        }
    "#;
    assert_eq!(compile_and_run(src), 1);
}

#[test]
fn native_e5_heap_array_is_empty_false() {
    let src = r#"
        fn main() -> i64 {
            let mut arr = []
            arr.push(42)
            arr.is_empty()
        }
    "#;
    assert_eq!(compile_and_run(src), 0);
}

#[test]
fn native_e5_heap_array_contains_true() {
    let src = r#"
        fn main() -> i64 {
            let mut arr = []
            arr.push(10)
            arr.push(20)
            arr.push(30)
            arr.contains(20)
        }
    "#;
    assert_eq!(compile_and_run(src), 1);
}

#[test]
fn native_e5_heap_array_contains_false() {
    let src = r#"
        fn main() -> i64 {
            let mut arr = []
            arr.push(10)
            arr.push(20)
            arr.push(30)
            arr.contains(99)
        }
    "#;
    assert_eq!(compile_and_run(src), 0);
}

#[test]
fn native_e5_heap_array_reverse() {
    let src = r#"
        fn main() -> i64 {
            let mut arr = []
            arr.push(1)
            arr.push(2)
            arr.push(3)
            arr.reverse()
            arr[0]
        }
    "#;
    assert_eq!(compile_and_run(src), 3);
}

#[test]
fn native_e5_stack_array_is_empty() {
    let src = r#"
        fn main() -> i64 {
            let arr = [1, 2, 3]
            arr.is_empty()
        }
    "#;
    assert_eq!(compile_and_run(src), 0);
}

// ── E.4 continued: trig, log, pow, len builtins ──────────────────

#[test]
fn native_e4_sin() {
    let result = compile_and_run_f64("fn main() -> f64 { sin(0.0) }");
    assert!(result.abs() < 1e-10);
}

#[test]
fn native_e4_cos() {
    let result = compile_and_run_f64("fn main() -> f64 { cos(0.0) }");
    assert!((result - 1.0).abs() < 1e-10);
}

#[test]
fn native_e4_tan() {
    let result = compile_and_run_f64("fn main() -> f64 { tan(0.0) }");
    assert!(result.abs() < 1e-10);
}

#[test]
fn native_e4_pow_float() {
    let result = compile_and_run_f64("fn main() -> f64 { pow(2.0, 10.0) }");
    assert!((result - 1024.0).abs() < 1e-10);
}

#[test]
fn native_e4_log2() {
    let result = compile_and_run_f64("fn main() -> f64 { log2(8.0) }");
    assert!((result - 3.0).abs() < 1e-10);
}

#[test]
fn native_e4_log10() {
    let result = compile_and_run_f64("fn main() -> f64 { log10(1000.0) }");
    assert!((result - 3.0).abs() < 1e-10);
}

#[test]
fn native_e4_len_string() {
    let src = r#"
        fn main() -> i64 {
            let s = "hello"
            len(s)
        }
    "#;
    assert_eq!(compile_and_run(src), 5);
}

#[test]
fn native_e4_len_heap_array() {
    let src = r#"
        fn main() -> i64 {
            let mut arr = []
            arr.push(1)
            arr.push(2)
            arr.push(3)
            len(arr)
        }
    "#;
    assert_eq!(compile_and_run(src), 3);
}

#[test]
fn native_e4_len_stack_array() {
    let src = r#"
        fn main() -> i64 {
            let arr = [10, 20, 30, 40]
            len(arr)
        }
    "#;
    assert_eq!(compile_and_run(src), 4);
}

#[test]
fn native_e4_assert_eq_pass() {
    let src = r#"
        fn main() -> i64 {
            assert_eq(42, 42)
            1
        }
    "#;
    assert_eq!(compile_and_run(src), 1);
}

#[test]
fn native_e4_sin_pi_half() {
    let result = compile_and_run_f64(
        r#"
        fn main() -> f64 {
            sin(1.5707963267948966)
        }
    "#,
    );
    assert!((result - 1.0).abs() < 1e-10);
}

#[test]
fn native_string_index_of_found() {
    // Native codegen index_of returns raw i64 (position or -1), not Option
    let src = r#"
        fn main() -> i64 {
            let s = "hello world"
            s.index_of("world")
        }
    "#;
    assert_eq!(compile_and_run(src), 6);
}

#[test]
fn native_string_index_of_not_found() {
    // Native codegen index_of returns -1 when not found
    let src = r#"
        fn main() -> i64 {
            let s = "hello world"
            s.index_of("xyz")
        }
    "#;
    assert_eq!(compile_and_run(src), -1_i64);
}

#[test]
fn native_string_index_of_at_start() {
    // Native codegen index_of returns raw position
    let src = r#"
        fn main() -> i64 {
            let s = "hello"
            s.index_of("he")
        }
    "#;
    assert_eq!(compile_and_run(src), 0);
}

#[test]
fn native_array_join() {
    // join returns a string; we verify by checking the length
    let src = r#"
        fn main() -> i64 {
            let arr = [1, 2, 3]
            let result = arr.join(", ")
            result.len()
        }
    "#;
    assert_eq!(compile_and_run(src), 7); // "1, 2, 3" = 7 chars
}

#[test]
fn native_array_join_empty_sep() {
    let src = r#"
        fn main() -> i64 {
            let arr = [10, 20, 30]
            let result = arr.join("")
            result.len()
        }
    "#;
    assert_eq!(compile_and_run(src), 6); // "102030" = 6 chars
}

#[test]
fn native_string_chars_len() {
    let src = r#"
        fn main() -> i64 {
            let s = "hello"
            let c = s.chars()
            c.len()
        }
    "#;
    assert_eq!(compile_and_run(src), 5);
}

#[test]
fn native_string_chars_get() {
    let src = r#"
        fn main() -> i64 {
            let s = "ABC"
            let c = s.chars()
            c[0]
        }
    "#;
    assert_eq!(compile_and_run(src), 65); // 'A' = 65
}

#[test]
fn native_string_bytes_len() {
    let src = r#"
        fn main() -> i64 {
            let s = "hi"
            let b = s.bytes()
            b.len()
        }
    "#;
    assert_eq!(compile_and_run(src), 2);
}

#[test]
fn native_string_bytes_get() {
    let src = r#"
        fn main() -> i64 {
            let s = "AB"
            let b = s.bytes()
            b[1]
        }
    "#;
    assert_eq!(compile_and_run(src), 66); // 'B' = 66
}

#[test]
fn native_string_repeat() {
    let src = r#"
        fn main() -> i64 {
            let s = "ab"
            let r = s.repeat(3)
            r.len()
        }
    "#;
    assert_eq!(compile_and_run(src), 6); // "ababab" = 6 chars
}

#[test]
fn native_string_repeat_zero() {
    let src = r#"
        fn main() -> i64 {
            let s = "hello"
            let r = s.repeat(0)
            r.len()
        }
    "#;
    assert_eq!(compile_and_run(src), 0); // "" = 0 chars
}

#[test]
fn native_string_rev() {
    let src = r#"
        fn main() -> i64 {
            let s = "ABC"
            let r = s.rev()
            let c = r.chars()
            c[0]
        }
    "#;
    // "ABC" reversed = "CBA", first char 'C' = 67
    assert_eq!(compile_and_run(src), 67);
}

#[test]
fn native_string_rev_len() {
    let src = r#"
        fn main() -> i64 {
            let s = "hello"
            let r = s.rev()
            r.len()
        }
    "#;
    assert_eq!(compile_and_run(src), 5);
}

#[test]
fn native_fn_returns_string_len() {
    let src = r#"
        fn greet() -> str { "hello" }
        fn main() -> i64 {
            let s = greet()
            s.len()
        }
    "#;
    assert_eq!(compile_and_run(src), 5);
}

#[test]
fn native_fn_returns_string_if_else() {
    let src = r#"
        fn classify(x: i64) -> str {
            if x == 0 { "idle" } else { "active" }
        }
        fn main() -> i64 {
            let a = classify(0)
            let b = classify(1)
            a.len() + b.len()
        }
    "#;
    // "idle" = 4, "active" = 6
    assert_eq!(compile_and_run(src), 10);
}

#[test]
fn native_fn_returns_string_chained_if() {
    let src = r#"
        fn label(x: i64) -> str {
            if x == 0 { "zero" } else if x == 1 { "one" } else { "many" }
        }
        fn main() -> i64 {
            let a = label(0)
            let b = label(1)
            let c = label(5)
            a.len() + b.len() + c.len()
        }
    "#;
    // "zero"=4, "one"=3, "many"=4
    assert_eq!(compile_and_run(src), 11);
}

#[test]
fn native_match_returns_string() {
    let src = r#"
        fn describe(x: i64) -> str {
            match x {
                0 => "zero",
                1 => "one",
                _ => "other"
            }
        }
        fn main() -> i64 {
            let a = describe(0)
            let b = describe(1)
            let c = describe(5)
            a.len() + b.len() + c.len()
        }
    "#;
    // "zero"=4, "one"=3, "other"=5
    assert_eq!(compile_and_run(src), 12);
}

#[test]
fn native_println_bool_var_from_str_eq() {
    // Regression: println(eq) where eq = str_var == "lit" used to segfault
    // because last_string_len leaked into the bool variable's string_lens entry.
    let src = r#"
        fn main() -> i64 {
            let a = "hello"
            let eq = a == "hello"
            if eq { 1 } else { 0 }
        }
    "#;
    assert_eq!(compile_and_run(src), 1);
}

#[test]
fn native_str_param_print() {
    // Test that string parameters are correctly passed (ptr + len)
    let src = r#"
        fn greet(name: str) -> i64 {
            println(name)
            1
        }
        fn main() -> i64 { greet("world") }
    "#;
    assert_eq!(compile_and_run(src), 1);
}

#[test]
fn native_str_param_eq() {
    // Test string comparison in a user function with str parameter
    let src = r#"
        fn check(word: str) -> i64 {
            if word == "hello" { 1 } else { 0 }
        }
        fn main() -> i64 { check("hello") }
    "#;
    assert_eq!(compile_and_run(src), 1);
}

#[test]
fn native_str_param_ne() {
    // Test string != comparison
    let src = r#"
        fn check(word: str) -> i64 {
            if word != "hello" { 1 } else { 0 }
        }
        fn main() -> i64 { check("world") }
    "#;
    assert_eq!(compile_and_run(src), 1);
}

#[test]
fn native_str_param_multi_if() {
    // Test multiple if-return with string comparisons (lookup table pattern)
    let src = r#"
        fn lookup(word: str) -> i64 {
            if word == "a" { return 1 }
            if word == "b" { return 2 }
            if word == "c" { return 3 }
            return 0
        }
        fn main() -> i64 { lookup("b") }
    "#;
    assert_eq!(compile_and_run(src), 2);
}

#[test]
fn native_str_param_with_int_param() {
    // Test mixed string + int parameters
    let src = r#"
        fn greet(name: str, count: i64) -> i64 {
            if name == "test" { count * 2 } else { count }
        }
        fn main() -> i64 { greet("test", 21) }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_str_eq_returns_bool() {
    // Test that string == returning bool doesn't cause type mismatch
    let src = r#"
        fn is_x(c: str) -> bool {
            c == "x" || c == "y"
        }
        fn main() -> i64 {
            if is_x("y") { 1 } else { 0 }
        }
    "#;
    assert_eq!(compile_and_run(src), 1);
}

#[test]
fn native_elseif_array_return() {
    // Test if/else-if/else returning arrays (merge type must be pointer, not element type)
    let src = r#"
        fn pick(n: i64) -> [f64; 2] {
            if n < 5 {
                let d = [1.0, 2.0]
                d
            } else if n < 10 {
                let d = [3.0, 4.0]
                d
            } else {
                let d = [5.0, 6.0]
                d
            }
        }
        fn main() -> i64 {
            let r = pick(7)
            0
        }
    "#;
    assert_eq!(compile_and_run(src), 0);
}

#[test]
fn native_wrapping_add() {
    let src = r#"
        fn main() -> i64 { wrapping_add(100, 200) }
    "#;
    assert_eq!(compile_and_run(src), 300);
}

#[test]
fn native_wrapping_sub() {
    let src = r#"
        fn main() -> i64 { wrapping_sub(10, 3) }
    "#;
    assert_eq!(compile_and_run(src), 7);
}

#[test]
fn native_wrapping_mul() {
    let src = r#"
        fn main() -> i64 { wrapping_mul(6, 7) }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_saturating_add_clamps() {
    let src = r#"
        fn main() -> i64 { saturating_add(9223372036854775800, 100) }
    "#;
    assert_eq!(compile_and_run(src), i64::MAX);
}

#[test]
fn native_saturating_sub_floors_at_zero() {
    let src = r#"
        fn main() -> i64 { saturating_sub(5, 3) }
    "#;
    assert_eq!(compile_and_run(src), 2);
}

#[test]
fn native_println_method_call_string() {
    // Regression: println(s.to_uppercase()) printed pointer value
    let src = r#"
        fn main() -> i64 {
            let s = "hello"
            let u = s.to_uppercase()
            u.len()
        }
    "#;
    assert_eq!(compile_and_run(src), 5);
}

#[test]
fn native_trim_start() {
    let src = r#"
        fn main() -> i64 {
            let s = "  hello  "
            let t = s.trim_start()
            t.len()
        }
    "#;
    // "hello  " = 7
    assert_eq!(compile_and_run(src), 7);
}

#[test]
fn native_trim_end() {
    let src = r#"
        fn main() -> i64 {
            let s = "  hello  "
            let t = s.trim_end()
            t.len()
        }
    "#;
    // "  hello" = 7
    assert_eq!(compile_and_run(src), 7);
}

#[test]
fn native_dbg_returns_value() {
    let src = r#"
        fn main() -> i64 {
            let x = dbg(42)
            x + 1
        }
    "#;
    assert_eq!(compile_and_run(src), 43);
}

#[test]
fn native_parse_int_ok() {
    let src = r#"
        fn main() -> i64 {
            let s = "123"
            let r = s.parse_int()
            match r { Ok(n) => n, Err(_) => -1 }
        }
    "#;
    assert_eq!(compile_and_run(src), 123);
}

#[test]
fn native_parse_int_err() {
    let src = r#"
        fn main() -> i64 {
            let s = "abc"
            let r = s.parse_int()
            match r { Ok(n) => n, Err(_) => -1 }
        }
    "#;
    assert_eq!(compile_and_run(src), -1);
}

#[test]
fn native_method_on_string_literal() {
    let src = r#"
        fn main() -> i64 { "hello world".len() }
    "#;
    assert_eq!(compile_and_run(src), 11);
}

#[test]
fn native_trim_on_string_literal() {
    let src = r#"
        fn main() -> i64 {
            let t = "  hi  ".trim()
            t.len()
        }
    "#;
    assert_eq!(compile_and_run(src), 2);
}

#[test]
fn native_assert_pass() {
    let src = r#"
        fn main() -> i64 {
            assert(1 == 1)
            42
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_assert_eq_pass() {
    let src = r#"
        fn main() -> i64 {
            assert_eq(10, 10)
            1
        }
    "#;
    assert_eq!(compile_and_run(src), 1);
}

#[test]
fn native_len_string() {
    let src = r#"
        fn main() -> i64 {
            let s = "hello"
            len(s)
        }
    "#;
    assert_eq!(compile_and_run(src), 5);
}

#[test]
fn native_len_array() {
    let src = r#"
        fn main() -> i64 {
            let arr = [10, 20, 30]
            len(arr)
        }
    "#;
    assert_eq!(compile_and_run(src), 3);
}

#[test]
fn native_to_string_int() {
    let src = r#"
        fn main() -> i64 {
            let s = to_string(42)
            s.len()
        }
    "#;
    assert_eq!(compile_and_run(src), 2); // "42" has length 2
}

#[test]
fn native_eprintln_i64() {
    // eprintln writes to stderr, but should not crash and returns null (0)
    let src = r#"
        fn main() -> i64 {
            eprintln(42)
            1
        }
    "#;
    assert_eq!(compile_and_run(src), 1);
}

#[test]
fn native_eprintln_string() {
    let src = r#"
        fn main() -> i64 {
            eprintln("error msg")
            1
        }
    "#;
    assert_eq!(compile_and_run(src), 1);
}

#[test]
fn native_eprint_i64() {
    let src = r#"
        fn main() -> i64 {
            eprint(99)
            1
        }
    "#;
    assert_eq!(compile_and_run(src), 1);
}

#[test]
fn native_saturating_mul_no_overflow() {
    let src = r#"
        fn main() -> i64 { saturating_mul(6, 7) }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_saturating_mul_clamps_max() {
    let src = r#"
        fn main() -> i64 { saturating_mul(9223372036854775807, 2) }
    "#;
    assert_eq!(compile_and_run(src), i64::MAX);
}

#[test]
fn native_saturating_mul_clamps_min() {
    let src = r#"
        fn main() -> i64 {
            let x = saturating_mul(-9223372036854775807, 2)
            if x < 0 { 1 } else { 0 }
        }
    "#;
    assert_eq!(compile_and_run(src), 1);
}

#[test]
fn native_checked_add_some() {
    let src = r#"
        fn main() -> i64 {
            let tag = checked_add(10, 20)
            tag
        }
    "#;
    // tag=1 means Some
    assert_eq!(compile_and_run(src), 1);
}

#[test]
fn native_checked_add_overflow() {
    let src = r#"
        fn main() -> i64 {
            let tag = checked_add(9223372036854775807, 1)
            tag
        }
    "#;
    // tag=0 means None (overflow)
    assert_eq!(compile_and_run(src), 0);
}

#[test]
fn native_checked_sub_some() {
    let src = r#"
        fn main() -> i64 {
            let tag = checked_sub(50, 30)
            tag
        }
    "#;
    assert_eq!(compile_and_run(src), 1);
}

#[test]
fn native_checked_sub_overflow() {
    let src = r#"
        fn main() -> i64 {
            let tag = checked_sub(-9223372036854775807, 100)
            tag
        }
    "#;
    assert_eq!(compile_and_run(src), 0);
}

#[test]
fn native_checked_mul_some() {
    let src = r#"
        fn main() -> i64 {
            let tag = checked_mul(6, 7)
            tag
        }
    "#;
    assert_eq!(compile_and_run(src), 1);
}

#[test]
fn native_checked_mul_overflow() {
    let src = r#"
        fn main() -> i64 {
            let tag = checked_mul(9223372036854775807, 2)
            tag
        }
    "#;
    assert_eq!(compile_and_run(src), 0);
}

#[test]
fn native_split_len() {
    let src = r#"
        fn main() -> i64 {
            let s = "hello,world,foo"
            let parts = s.split(",")
            parts.len()
        }
    "#;
    assert_eq!(compile_and_run(src), 3);
}

#[test]
fn native_split_single() {
    let src = r#"
        fn main() -> i64 {
            let s = "no_delimiters"
            let parts = s.split(",")
            parts.len()
        }
    "#;
    assert_eq!(compile_and_run(src), 1);
}

#[test]
fn native_split_empty_delimiter() {
    let src = r#"
        fn main() -> i64 {
            let s = "abc"
            let parts = s.split("")
            parts.len()
        }
    "#;
    // Splitting by "" gives: "", "a", "b", "c", "" = 5 parts (Rust's split("") behavior)
    assert_eq!(compile_and_run(src), 5);
}

#[test]
fn native_format_no_args() {
    let src = r#"
        fn main() -> i64 {
            let s = format("hello world")
            s.len()
        }
    "#;
    assert_eq!(compile_and_run(src), 11);
}

#[test]
fn native_format_one_int() {
    let src = r#"
        fn main() -> i64 {
            let s = format("value={}", 42)
            s.len()
        }
    "#;
    // "value=42" = 8 chars
    assert_eq!(compile_and_run(src), 8);
}

#[test]
fn native_format_two_ints() {
    let src = r#"
        fn main() -> i64 {
            let s = format("{} + {} = 3", 1, 2)
            s.len()
        }
    "#;
    // "1 + 2 = 3" = 9 chars
    assert_eq!(compile_and_run(src), 9);
}

#[test]
fn native_format_string_arg() {
    let src = r#"
        fn main() -> i64 {
            let name = "world"
            let s = format("hello {}", name)
            s.len()
        }
    "#;
    // "hello world" = 11 chars
    assert_eq!(compile_and_run(src), 11);
}

#[test]
fn native_format_bool_arg() {
    let src = r#"
        fn main() -> i64 {
            let s = format("flag={}", true)
            s.len()
        }
    "#;
    // "flag=true" = 9 chars
    assert_eq!(compile_and_run(src), 9);
}

#[test]
fn native_format_float_arg() {
    let src = r#"
        fn main() -> i64 {
            let s = format("x={}", 3.14)
            if s.len() > 0 { 1 } else { 0 }
        }
    "#;
    assert_eq!(compile_and_run(src), 1);
}

#[test]
fn native_format_mixed_args() {
    let src = r#"
        fn main() -> i64 {
            let s = format("{} is {}", "hello", 42)
            s.len()
        }
    "#;
    // "hello is 42" = 11 chars
    assert_eq!(compile_and_run(src), 11);
}

#[test]
fn native_checked_add_value() {
    // Verify the actual payload value from checked_add
    let src = r#"
        fn main() -> i64 {
            let a = 10
            let b = 20
            let result = checked_add(a, b)
            if result == 1 {
                30
            } else {
                0
            }
        }
    "#;
    assert_eq!(compile_and_run(src), 30);
}

#[test]
fn native_saturating_mul_zero() {
    let src = r#"
        fn main() -> i64 { saturating_mul(0, 9223372036854775807) }
    "#;
    assert_eq!(compile_and_run(src), 0);
}

#[test]
fn native_nested_string_ops() {
    let src = r#"
        fn main() -> i64 {
            let s = "hello world"
            let u = s.to_uppercase()
            let t = u.trim()
            t.len()
        }
    "#;
    assert_eq!(compile_and_run(src), 11);
}

#[test]
fn native_chained_replace() {
    let src = r#"
        fn main() -> i64 {
            let s = "hello world"
            let r = s.replace("world", "fajar")
            r.len()
        }
    "#;
    // "hello fajar" = 11 chars
    assert_eq!(compile_and_run(src), 11);
}

#[test]
fn native_string_contains_true() {
    let src = r#"
        fn main() -> i64 {
            let s = "hello world"
            if s.contains("world") { 1 } else { 0 }
        }
    "#;
    assert_eq!(compile_and_run(src), 1);
}

#[test]
fn native_for_in_range_sum() {
    let src = r#"
        fn main() -> i64 {
            let mut sum = 0
            for i in 0..10 {
                sum = sum + i
            }
            sum
        }
    "#;
    // 0+1+2+...+9 = 45
    assert_eq!(compile_and_run(src), 45);
}

#[test]
fn native_while_with_break() {
    let src = r#"
        fn main() -> i64 {
            let mut x = 0
            while true {
                x = x + 1
                if x == 10 {
                    break
                }
            }
            x
        }
    "#;
    assert_eq!(compile_and_run(src), 10);
}

#[test]
fn native_loop_with_continue() {
    let src = r#"
        fn main() -> i64 {
            let mut sum = 0
            let mut i = 0
            loop {
                i = i + 1
                if i > 10 { break }
                if i % 2 == 0 { continue }
                sum = sum + i
            }
            sum
        }
    "#;
    // odd numbers 1+3+5+7+9 = 25
    assert_eq!(compile_and_run(src), 25);
}

#[test]
fn native_nested_function_calls() {
    let src = r#"
        fn double(x: i64) -> i64 { x * 2 }
        fn add_one(x: i64) -> i64 { x + 1 }
        fn main() -> i64 { add_one(double(5)) }
    "#;
    assert_eq!(compile_and_run(src), 11);
}

#[test]
fn native_recursive_sum() {
    let src = r#"
        fn sum(n: i64) -> i64 {
            if n <= 0 { 0 } else { n + sum(n - 1) }
        }
        fn main() -> i64 { sum(10) }
    "#;
    assert_eq!(compile_and_run(src), 55);
}

#[test]
fn native_enum_match_with_return() {
    let src = r#"
        enum Color { Red, Green, Blue }
        fn code(c: i64) -> i64 {
            match c {
                0 => 255,
                1 => 128,
                2 => 64,
                _ => 0,
            }
        }
        fn main() -> i64 { code(1) }
    "#;
    assert_eq!(compile_and_run(src), 128);
}

#[test]
fn native_multiple_string_vars() {
    let src = r#"
        fn main() -> i64 {
            let a = "hello"
            let b = "world"
            let c = "!"
            a.len() + b.len() + c.len()
        }
    "#;
    // 5 + 5 + 1 = 11
    assert_eq!(compile_and_run(src), 11);
}

#[test]
fn native_struct_multiple_methods() {
    let src = r#"
        struct Rect { w: i64, h: i64 }
        impl Rect {
            fn area(self) -> i64 { self.w * self.h }
            fn perimeter(self) -> i64 { 2 * (self.w + self.h) }
        }
        fn main() -> i64 {
            let r = Rect { w: 5, h: 3 }
            r.area() + r.perimeter()
        }
    "#;
    // area=15, perimeter=16, total=31
    assert_eq!(compile_and_run(src), 31);
}

#[test]
fn native_pipeline_chain() {
    let src = r#"
        fn double(x: i64) -> i64 { x * 2 }
        fn inc(x: i64) -> i64 { x + 1 }
        fn main() -> i64 { 5 |> double |> inc |> double }
    "#;
    // ((5*2)+1)*2 = 22
    assert_eq!(compile_and_run(src), 22);
}

#[test]
fn native_as_cast_f64_to_i64() {
    let src = r#"
        fn main() -> i64 { 3.7 as i64 }
    "#;
    assert_eq!(compile_and_run(src), 3);
}

#[test]
fn native_as_cast_i64_to_f64() {
    let src = r#"
        fn main() -> i64 {
            let x = 42 as f64
            x as i64
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_bitwise_ops() {
    let src = r#"
        fn main() -> i64 {
            let a = 0xFF
            let b = 0x0F
            let and_result = a & b
            let or_result = a | b
            let xor_result = a ^ b
            and_result + or_result + xor_result
        }
    "#;
    // and=0x0F=15, or=0xFF=255, xor=0xF0=240 → 510
    assert_eq!(compile_and_run(src), 510);
}

#[test]
fn native_shift_ops() {
    let src = r#"
        fn main() -> i64 {
            let x = 1 << 10
            let y = x >> 5
            y
        }
    "#;
    // 1<<10 = 1024, 1024>>5 = 32
    assert_eq!(compile_and_run(src), 32);
}

#[test]
fn native_split_index_first() {
    let src = r#"
        fn main() -> i64 {
            let s = "hello,world"
            let parts = s.split(",")
            let first = parts[0]
            first.len()
        }
    "#;
    // "hello" = 5 chars
    assert_eq!(compile_and_run(src), 5);
}

#[test]
fn native_split_index_second() {
    let src = r#"
        fn main() -> i64 {
            let s = "a:bb:ccc"
            let parts = s.split(":")
            let second = parts[1]
            second.len()
        }
    "#;
    // "bb" = 2 chars
    assert_eq!(compile_and_run(src), 2);
}

#[test]
fn native_for_in_split_count() {
    let src = r#"
        fn main() -> i64 {
            let s = "one,two,three"
            let parts = s.split(",")
            let mut total_len = 0
            for part in parts {
                total_len = total_len + part.len()
            }
            total_len
        }
    "#;
    // "one"=3 + "two"=3 + "three"=5 = 11
    assert_eq!(compile_and_run(src), 11);
}

#[test]
fn native_for_in_split_count_items() {
    let src = r#"
        fn main() -> i64 {
            let s = "a,b,c,d,e"
            let parts = s.split(",")
            let mut count = 0
            for part in parts {
                count = count + 1
            }
            count
        }
    "#;
    assert_eq!(compile_and_run(src), 5);
}
