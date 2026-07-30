//! f64 arithmetic, string concat, println, dynamic heap arrays, enum/match.

use super::{compile_and_run, compile_and_run_f64};

// ── Float (f64) arithmetic in native codegen ─────────────────────────

#[test]
fn native_f64_add() {
    let result = compile_and_run_f64("fn main() -> f64 { 1.5 + 2.3 }");
    assert!((result - 3.8).abs() < 1e-10);
}

#[test]
fn native_f64_sub() {
    let result = compile_and_run_f64("fn main() -> f64 { 10.0 - 3.5 }");
    assert!((result - 6.5).abs() < 1e-10);
}

#[test]
fn native_f64_mul() {
    let result = compile_and_run_f64("fn main() -> f64 { 3.0 * 4.5 }");
    assert!((result - 13.5).abs() < 1e-10);
}

#[test]
fn native_f64_div() {
    let result = compile_and_run_f64("fn main() -> f64 { 10.0 / 4.0 }");
    assert!((result - 2.5).abs() < 1e-10);
}

#[test]
fn native_f64_neg() {
    let result = compile_and_run_f64("fn main() -> f64 { -(3.14) }");
    assert!((result - (-3.14)).abs() < 1e-10);
}

#[test]
fn native_f64_variable() {
    let src = r#"
        fn main() -> f64 {
            let x: f64 = 2.5
            let y: f64 = 3.5
            x + y
        }
    "#;
    let result = compile_and_run_f64(src);
    assert!((result - 6.0).abs() < 1e-10);
}

#[test]
fn native_f64_inferred_type() {
    // Type inferred from f64 literal on RHS
    let src = r#"
        fn main() -> f64 {
            let x = 2.5
            let y = 3.5
            x * y
        }
    "#;
    let result = compile_and_run_f64(src);
    assert!((result - 8.75).abs() < 1e-10);
}

#[test]
fn native_f64_compound_expr() {
    let src = r#"
        fn main() -> f64 {
            let a = 2.0
            let b = 3.0
            let c = 4.0
            a * b + c
        }
    "#;
    let result = compile_and_run_f64(src);
    assert!((result - 10.0).abs() < 1e-10);
}

#[test]
fn native_f64_comparison_gt() {
    let src = r#"
        fn main() -> i64 {
            let a: f64 = 3.14
            let b: f64 = 2.71
            if a > b { 1 } else { 0 }
        }
    "#;
    assert_eq!(compile_and_run(src), 1);
}

#[test]
fn native_f64_comparison_lt() {
    let src = r#"
        fn main() -> i64 {
            let a: f64 = 1.0
            let b: f64 = 2.0
            if a < b { 1 } else { 0 }
        }
    "#;
    assert_eq!(compile_and_run(src), 1);
}

#[test]
fn native_f64_comparison_eq() {
    let src = r#"
        fn main() -> i64 {
            let a: f64 = 3.0
            let b: f64 = 3.0
            if a == b { 1 } else { 0 }
        }
    "#;
    assert_eq!(compile_and_run(src), 1);
}

#[test]
fn native_f64_if_expr() {
    let src = r#"
        fn main() -> f64 {
            let x: f64 = 5.0
            if x > 3.0 { 1.0 } else { 0.0 }
        }
    "#;
    let result = compile_and_run_f64(src);
    assert!((result - 1.0).abs() < 1e-10);
}

#[test]
fn native_f64_function_call() {
    let src = r#"
        fn add_f64(a: f64, b: f64) -> f64 {
            a + b
        }
        fn main() -> f64 {
            add_f64(1.5, 2.5)
        }
    "#;
    let result = compile_and_run_f64(src);
    assert!((result - 4.0).abs() < 1e-10);
}

#[test]
fn native_f64_mut_assign() {
    let src = r#"
        fn main() -> f64 {
            let mut x: f64 = 1.0
            x = x + 0.5
            x += 0.5
            x
        }
    "#;
    let result = compile_and_run_f64(src);
    assert!((result - 2.0).abs() < 1e-10);
}

#[test]
fn native_f64_while_loop() {
    let src = r#"
        fn main() -> f64 {
            let mut sum: f64 = 0.0
            let mut i = 0
            while i < 5 {
                sum += 1.5
                i = i + 1
            }
            sum
        }
    "#;
    let result = compile_and_run_f64(src);
    assert!((result - 7.5).abs() < 1e-10);
}

#[test]
fn native_f64_recursive_fn() {
    let src = r#"
        fn sum_f64(n: i64) -> f64 {
            if n == 0 { 0.0 } else { 1.5 + sum_f64(n - 1) }
        }
        fn main() -> f64 {
            sum_f64(4)
        }
    "#;
    let result = compile_and_run_f64(src);
    assert!((result - 6.0).abs() < 1e-10);
}

// ── String variable + concatenation in native codegen ────────────────

#[test]
fn native_string_variable_println() {
    // println with a string variable (not literal) — dispatches to __println_str
    let src = r#"
        fn main() -> i64 {
            let msg = "hello native"
            println(msg)
            0
        }
    "#;
    assert_eq!(compile_and_run(src), 0);
}

#[test]
fn native_string_concat_variables() {
    // Runtime string concat: variable + variable
    let src = r#"
        fn main() -> i64 {
            let a = "hello"
            let b = " world"
            let c = a + b
            println(c)
            0
        }
    "#;
    assert_eq!(compile_and_run(src), 0);
}

#[test]
fn native_string_concat_literal_and_variable() {
    // Mixed: literal + variable
    let src = r#"
        fn main() -> i64 {
            let name = "Fajar"
            let greeting = "Hello, " + name
            println(greeting)
            0
        }
    "#;
    assert_eq!(compile_and_run(src), 0);
}

#[test]
fn native_string_concat_chain() {
    // Chain: a + b + c (multiple concats)
    let src = r#"
        fn main() -> i64 {
            let a = "one"
            let b = " two"
            let c = " three"
            let result = a + b + c
            println(result)
            0
        }
    "#;
    assert_eq!(compile_and_run(src), 0);
}

// ── println/print for f64 values ─────────────────────────────────────

#[test]
fn native_println_f64_literal() {
    let src = r#"
        fn main() -> i64 {
            println(3.14)
            0
        }
    "#;
    assert_eq!(compile_and_run(src), 0);
}

#[test]
fn native_println_f64_variable() {
    let src = r#"
        fn main() -> i64 {
            let pi: f64 = 3.14159
            println(pi)
            0
        }
    "#;
    assert_eq!(compile_and_run(src), 0);
}

#[test]
fn native_println_f64_expr() {
    // Print a computed f64 value
    let src = r#"
        fn main() -> i64 {
            let a: f64 = 2.5
            let b: f64 = 3.5
            println(a + b)
            0
        }
    "#;
    assert_eq!(compile_and_run(src), 0);
}

#[test]
fn native_print_bool_as_int() {
    // Booleans are printed as 0/1 via the integer printer
    let src = r#"
        fn main() -> i64 {
            let x = 5 > 3
            println(x)
            0
        }
    "#;
    assert_eq!(compile_and_run(src), 0);
}

// ── Dynamic heap arrays ──────────────────────────────────────────────

#[test]
fn native_heap_array_push_and_len() {
    let src = r#"
        fn main() -> i64 {
            let mut arr = []
            arr.push(10)
            arr.push(20)
            arr.push(30)
            arr.len()
        }
    "#;
    assert_eq!(compile_and_run(src), 3);
}

#[test]
fn native_heap_array_push_and_index() {
    let src = r#"
        fn main() -> i64 {
            let mut arr = []
            arr.push(100)
            arr.push(200)
            arr.push(300)
            arr[1]
        }
    "#;
    assert_eq!(compile_and_run(src), 200);
}

#[test]
fn native_heap_array_index_assign() {
    let src = r#"
        fn main() -> i64 {
            let mut arr = []
            arr.push(1)
            arr.push(2)
            arr.push(3)
            arr[1] = 99
            arr[1]
        }
    "#;
    assert_eq!(compile_and_run(src), 99);
}

#[test]
fn native_heap_array_pop() {
    let src = r#"
        fn main() -> i64 {
            let mut arr = []
            arr.push(10)
            arr.push(20)
            arr.push(30)
            let last = arr.pop()
            last
        }
    "#;
    assert_eq!(compile_and_run(src), 30);
}

#[test]
fn native_heap_array_sum_loop() {
    let src = r#"
        fn main() -> i64 {
            let mut arr = []
            arr.push(1)
            arr.push(2)
            arr.push(3)
            arr.push(4)
            arr.push(5)
            let mut sum = 0
            let mut i = 0
            while i < arr.len() {
                sum = sum + arr[i]
                i = i + 1
            }
            sum
        }
    "#;
    assert_eq!(compile_and_run(src), 15);
}

#[test]
fn native_heap_array_for_in() {
    let src = r#"
        fn main() -> i64 {
            let mut arr = []
            arr.push(10)
            arr.push(20)
            arr.push(30)
            let mut total = 0
            for x in arr {
                total = total + x
            }
            total
        }
    "#;
    assert_eq!(compile_and_run(src), 60);
}

#[test]
fn native_heap_array_compound_index_assign() {
    let src = r#"
        fn main() -> i64 {
            let mut arr = []
            arr.push(10)
            arr.push(20)
            arr[0] += 5
            arr[0]
        }
    "#;
    assert_eq!(compile_and_run(src), 15);
}

#[test]
fn native_heap_array_pop_reduces_len() {
    let src = r#"
        fn main() -> i64 {
            let mut arr = []
            arr.push(1)
            arr.push(2)
            arr.push(3)
            arr.pop()
            arr.len()
        }
    "#;
    assert_eq!(compile_and_run(src), 2);
}

#[test]
fn native_stack_array_len_method() {
    let src = r#"
        fn main() -> i64 {
            let arr = [10, 20, 30, 40]
            arr.len()
        }
    "#;
    assert_eq!(compile_and_run(src), 4);
}

// ── Enum / match ─────────────────────────────────────────────────────

#[test]
fn native_match_int_literal() {
    let src = r#"
        fn main() -> i64 {
            let x = 2
            match x {
                1 => 10,
                2 => 20,
                3 => 30,
                _ => 0,
            }
        }
    "#;
    assert_eq!(compile_and_run(src), 20);
}

#[test]
fn native_match_wildcard() {
    let src = r#"
        fn main() -> i64 {
            let x = 99
            match x {
                1 => 10,
                _ => 42,
            }
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_match_option_some() {
    let src = r#"
        fn main() -> i64 {
            let val = Some(42)
            match val {
                Some(x) => x,
                None => 0,
            }
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_match_option_none() {
    let src = r#"
        fn main() -> i64 {
            let val = None
            match val {
                Some(x) => x,
                None => -1,
            }
        }
    "#;
    assert_eq!(compile_and_run(src), -1);
}

#[test]
fn native_match_option_computed() {
    let src = r#"
        fn main() -> i64 {
            let val = Some(10 + 20)
            match val {
                Some(x) => x + 1,
                None => 0,
            }
        }
    "#;
    assert_eq!(compile_and_run(src), 31);
}

#[test]
fn native_user_enum_match() {
    let src = r#"
        enum Color { Red, Green, Blue }
        fn main() -> i64 {
            let c = 1
            match c {
                0 => 10,
                1 => 20,
                2 => 30,
                _ => 0,
            }
        }
    "#;
    assert_eq!(compile_and_run(src), 20);
}

#[test]
fn native_match_with_function() {
    let src = r#"
        fn maybe_value(flag: i64) -> i64 {
            if flag > 0 { 1 } else { 0 }
        }
        fn main() -> i64 {
            let tag = maybe_value(1)
            match tag {
                0 => 100,
                1 => 200,
                _ => 0,
            }
        }
    "#;
    assert_eq!(compile_and_run(src), 200);
}

// ═══════════════════════════════════════════════════════════════════════
// Enum path construction + match tests
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn native_enum_path_unit_variant() {
    let src = r#"
        enum Color { Red, Green, Blue }
        fn main() -> i64 {
            let c = Color::Green
            match c {
                Color::Red => 10,
                Color::Green => 20,
                Color::Blue => 30,
                _ => 0,
            }
        }
    "#;
    assert_eq!(compile_and_run(src), 20);
}

#[test]
fn native_enum_path_with_payload() {
    let src = r#"
        enum Shape { Circle, Rect }
        fn main() -> i64 {
            let s = Shape::Circle
            match s {
                Shape::Circle => 100,
                Shape::Rect => 200,
                _ => 0,
            }
        }
    "#;
    assert_eq!(compile_and_run(src), 100);
}

#[test]
fn native_enum_path_data_variant() {
    let src = r#"
        enum Result { Ok, Err }
        fn main() -> i64 {
            let val = Some(99)
            match val {
                Some(x) => x,
                None => 0,
            }
        }
    "#;
    assert_eq!(compile_and_run(src), 99);
}

#[test]
fn native_enum_bare_variant_ident() {
    let src = r#"
        enum Color { Red, Green, Blue }
        fn main() -> i64 {
            let c = Green
            match c {
                Color::Red => 10,
                Color::Green => 20,
                Color::Blue => 30,
                _ => 0,
            }
        }
    "#;
    assert_eq!(compile_and_run(src), 20);
}

#[test]
fn native_enum_user_variant_with_data() {
    let src = r#"
        enum Wrapper { Val, Empty }
        fn main() -> i64 {
            let w = Val(42)
            match w {
                Wrapper::Val(x) => x,
                Wrapper::Empty => 0,
                _ => -1,
            }
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_enum_path_constructor_call() {
    let src = r#"
        enum Wrapper { Val, Empty }
        fn main() -> i64 {
            let w = Wrapper::Val(55)
            match w {
                Wrapper::Val(x) => x + 1,
                Wrapper::Empty => 0,
                _ => -1,
            }
        }
    "#;
    assert_eq!(compile_and_run(src), 56);
}

#[test]
fn native_enum_option_path_some() {
    let src = r#"
        fn main() -> i64 {
            let val = Option::Some(77)
            match val {
                Some(x) => x,
                None => 0,
            }
        }
    "#;
    assert_eq!(compile_and_run(src), 77);
}

#[test]
fn native_enum_option_path_none() {
    let src = r#"
        fn main() -> i64 {
            let val = Option::None
            match val {
                Some(x) => x,
                None => -1,
            }
        }
    "#;
    assert_eq!(compile_and_run(src), -1);
}

#[test]
fn native_enum_fn_param_and_return() {
    let src = r#"
        enum Color { Red, Green, Blue }
        fn color_value(c: i64) -> i64 {
            match c {
                Color::Red => 1,
                Color::Green => 2,
                Color::Blue => 3,
                _ => 0,
            }
        }
        fn main() -> i64 {
            let c = Color::Blue
            color_value(c)
        }
    "#;
    assert_eq!(compile_and_run(src), 3);
}

#[test]
fn native_enum_match_multiple_arms() {
    let src = r#"
        enum Dir { North, South, East, West }
        fn main() -> i64 {
            let d = Dir::East
            match d {
                Dir::North => 1,
                Dir::South => 2,
                Dir::East => 3,
                Dir::West => 4,
                _ => 0,
            }
        }
    "#;
    assert_eq!(compile_and_run(src), 3);
}

// ═══════════════════════════════════════════════════════════════════════
// Struct init + field access tests
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn native_struct_init_field_access() {
    let src = r#"
        struct Point { x: i64, y: i64 }
        fn main() -> i64 {
            let p = Point { x: 10, y: 20 }
            p.x + p.y
        }
    "#;
    assert_eq!(compile_and_run(src), 30);
}

#[test]
fn native_struct_field_access_second() {
    let src = r#"
        struct Point { x: i64, y: i64 }
        fn main() -> i64 {
            let p = Point { x: 3, y: 7 }
            p.y
        }
    "#;
    assert_eq!(compile_and_run(src), 7);
}

#[test]
fn native_struct_field_assign() {
    let src = r#"
        struct Point { x: i64, y: i64 }
        fn main() -> i64 {
            let mut p = Point { x: 1, y: 2 }
            p.x = 99
            p.x
        }
    "#;
    assert_eq!(compile_and_run(src), 99);
}

#[test]
fn native_struct_field_compound_assign() {
    let src = r#"
        struct Point { x: i64, y: i64 }
        fn main() -> i64 {
            let mut p = Point { x: 10, y: 5 }
            p.x += 5
            p.y -= 2
            p.x + p.y
        }
    "#;
    assert_eq!(compile_and_run(src), 18);
}

#[test]
fn native_struct_three_fields() {
    let src = r#"
        struct Vec3 { x: i64, y: i64, z: i64 }
        fn main() -> i64 {
            let v = Vec3 { x: 1, y: 2, z: 3 }
            v.x + v.y + v.z
        }
    "#;
    assert_eq!(compile_and_run(src), 6);
}

#[test]
fn native_struct_multiple_instances() {
    let src = r#"
        struct Point { x: i64, y: i64 }
        fn main() -> i64 {
            let a = Point { x: 1, y: 2 }
            let b = Point { x: 3, y: 4 }
            a.x + b.y
        }
    "#;
    assert_eq!(compile_and_run(src), 5);
}

#[test]
fn native_struct_in_expression() {
    let src = r#"
        struct Rect { w: i64, h: i64 }
        fn main() -> i64 {
            let r = Rect { w: 5, h: 8 }
            r.w * r.h
        }
    "#;
    assert_eq!(compile_and_run(src), 40);
}

#[test]
fn native_struct_with_computed_fields() {
    let src = r#"
        struct Point { x: i64, y: i64 }
        fn main() -> i64 {
            let a = 10
            let b = 20
            let p = Point { x: a + 1, y: b * 2 }
            p.x + p.y
        }
    "#;
    assert_eq!(compile_and_run(src), 51);
}

// ═══════════════════════════════════════════════════════════════════════
// Bitfield struct tests
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn native_bitfield_init_and_read() {
    // u3 field (3 bits: 0-7), u4 field (4 bits: 0-15)
    // Pack a=5 into bits [0..3], b=9 into bits [3..7]
    let src = r#"
        struct Flags { a: u3, b: u4 }
        fn main() -> i64 {
            let f = Flags { a: 5, b: 9 }
            f.a + f.b
        }
    "#;
    assert_eq!(compile_and_run(src), 14); // 5 + 9
}

#[test]
fn native_bitfield_individual_read() {
    let src = r#"
        struct Bits { x: u3, y: u4 }
        fn main() -> i64 {
            let b = Bits { x: 7, y: 12 }
            b.y
        }
    "#;
    assert_eq!(compile_and_run(src), 12);
}

#[test]
fn native_bitfield_write() {
    let src = r#"
        struct Flags { a: u3, b: u4 }
        fn main() -> i64 {
            let mut f = Flags { a: 5, b: 9 }
            f.a = 3
            f.a + f.b
        }
    "#;
    assert_eq!(compile_and_run(src), 12); // 3 + 9
}

#[test]
fn native_bitfield_single_bit() {
    // u1 is a single-bit field (0 or 1)
    let src = r#"
        struct Toggle { on: u1, off: u1 }
        fn main() -> i64 {
            let t = Toggle { on: 1, off: 0 }
            t.on + t.off
        }
    "#;
    assert_eq!(compile_and_run(src), 1);
}

#[test]
fn native_bitfield_compound_assign() {
    let src = r#"
        struct Bits { x: u4, y: u4 }
        fn main() -> i64 {
            let mut b = Bits { x: 3, y: 5 }
            b.x += 2
            b.x + b.y
        }
    "#;
    assert_eq!(compile_and_run(src), 10); // (3+2) + 5
}

// ═══════════════════════════════════════════════════════════════════════
// Impl block / method tests
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn native_impl_static_method() {
    let src = r#"
        struct Calc {}
        impl Calc {
            fn add(a: i64, b: i64) -> i64 { a + b }
        }
        fn main() -> i64 {
            Calc::add(3, 4)
        }
    "#;
    assert_eq!(compile_and_run(src), 7);
}

#[test]
fn native_impl_static_constructor() {
    let src = r#"
        struct Point { x: i64, y: i64 }
        impl Point {
            fn new(x: i64, y: i64) -> i64 { x + y }
        }
        fn main() -> i64 {
            Point::new(10, 20)
        }
    "#;
    assert_eq!(compile_and_run(src), 30);
}

#[test]
fn native_impl_instance_method_self() {
    let src = r#"
        struct Point { x: i64, y: i64 }
        impl Point {
            fn sum(self: Point) -> i64 { self.x + self.y }
        }
        fn main() -> i64 {
            let p = Point { x: 10, y: 20 }
            p.sum()
        }
    "#;
    assert_eq!(compile_and_run(src), 30);
}

#[test]
fn native_impl_method_with_args() {
    let src = r#"
        struct Rect { w: i64, h: i64 }
        impl Rect {
            fn scale(self: Rect, factor: i64) -> i64 {
                self.w * self.h * factor
            }
        }
        fn main() -> i64 {
            let r = Rect { w: 3, h: 4 }
            r.scale(2)
        }
    "#;
    assert_eq!(compile_and_run(src), 24);
}

#[test]
fn native_impl_multiple_methods() {
    let src = r#"
        struct Counter { val: i64 }
        impl Counter {
            fn get(self: Counter) -> i64 { self.val }
            fn doubled(self: Counter) -> i64 { self.val * 2 }
        }
        fn main() -> i64 {
            let c = Counter { val: 21 }
            c.get() + c.doubled()
        }
    "#;
    assert_eq!(compile_and_run(src), 63);
}

#[test]
fn native_impl_constructor_returns_struct() {
    let src = r#"
        struct Point { x: i64, y: i64 }
        impl Point {
            fn new(x: i64, y: i64) -> Point { Point { x: x, y: y } }
            fn sum(self: Point) -> i64 { self.x + self.y }
        }
        fn main() -> i64 {
            let p = Point::new(10, 20)
            p.sum()
        }
    "#;
    assert_eq!(compile_and_run(src), 30);
}

#[test]
fn native_impl_constructor_bare_self() {
    // Bare `self` (no type annotation) in impl method
    let src = r#"
        struct Vec2 { a: i64, b: i64 }
        impl Vec2 {
            fn create(a: i64, b: i64) -> Vec2 { Vec2 { a: a, b: b } }
            fn dot(self) -> i64 { self.a * self.b }
        }
        fn main() -> i64 {
            let v = Vec2::create(3, 7)
            v.dot()
        }
    "#;
    assert_eq!(compile_and_run(src), 21);
}

#[test]
fn native_fn_returns_struct() {
    // Non-impl function returning a struct
    let src = r#"
        struct Pair { x: i64, y: i64 }
        fn make_pair(a: i64, b: i64) -> Pair {
            Pair { x: a, y: b }
        }
        fn main() -> i64 {
            let p = make_pair(5, 8)
            p.x + p.y
        }
    "#;
    assert_eq!(compile_and_run(src), 13);
}

// ═══════════════════════════════════════════════════════════════════════
// Tuple tests
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn native_tuple_create_and_index() {
    let src = r#"
        fn main() -> i64 {
            let t = (10, 20, 30)
            t.0 + t.2
        }
    "#;
    assert_eq!(compile_and_run(src), 40);
}

#[test]
fn native_tuple_second_element() {
    let src = r#"
        fn main() -> i64 {
            let t = (5, 15)
            t.1
        }
    "#;
    assert_eq!(compile_and_run(src), 15);
}

#[test]
fn native_tuple_computed_elements() {
    let src = r#"
        fn main() -> i64 {
            let a = 3
            let b = 7
            let t = (a * 2, b + 1)
            t.0 + t.1
        }
    "#;
    assert_eq!(compile_and_run(src), 14);
}

// ═══════════════════════════════════════════════════════════════════════
// Cast (as) tests
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn native_cast_int_to_float() {
    let src = r#"
        fn main() -> i64 {
            let x = 42 as f64
            x as i64
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_cast_float_to_int() {
    let src = r#"
        fn main() -> i64 {
            let x: f64 = 3.7
            x as i64
        }
    "#;
    assert_eq!(compile_and_run(src), 3);
}

#[test]
fn native_cast_int_to_bool() {
    let src = r#"
        fn main() -> i64 {
            let a = 5 as bool
            let b = 0 as bool
            a + b
        }
    "#;
    // 5 != 0 → 1, 0 == 0 → 0, sum = 1
    assert_eq!(compile_and_run(src), 1);
}

// ═══════════════════════════════════════════════════════════════════════
// to_float / to_int builtin tests
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn native_to_float_from_int() {
    let src = r#"
        fn main() -> i64 {
            let x = to_float(42)
            let y = x + 0.5
            y as i64
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_to_float_from_float() {
    let src = r#"
        fn main() -> i64 {
            let x = to_float(3.14)
            x as i64
        }
    "#;
    assert_eq!(compile_and_run(src), 3);
}

#[test]
fn native_to_int_from_float() {
    let src = r#"
        fn main() -> i64 {
            to_int(7.9)
        }
    "#;
    assert_eq!(compile_and_run(src), 7);
}

#[test]
fn native_to_int_from_int() {
    let src = r#"
        fn main() -> i64 {
            to_int(42)
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_to_float_in_expr() {
    let src = r#"
        fn main() -> i64 {
            let a = 10
            let b = 3
            let ratio = to_float(a) / to_float(b)
            let result = ratio * 3.0
            result as i64
        }
    "#;
    assert_eq!(compile_and_run(src), 10);
}

// ═══════════════════════════════════════════════════════════════════════
// to_string builtin tests
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn native_to_string_int_len() {
    let src = r#"
        fn main() -> i64 {
            let s = to_string(42)
            s.len()
        }
    "#;
    assert_eq!(compile_and_run(src), 2); // "42" has length 2
}

#[test]
fn native_to_string_negative_len() {
    let src = r#"
        fn main() -> i64 {
            let s = to_string(-123)
            s.len()
        }
    "#;
    assert_eq!(compile_and_run(src), 4); // "-123" has length 4
}

#[test]
fn native_to_string_print() {
    let src = r#"
        fn main() -> i64 {
            let s = to_string(99)
            println(s)
            0
        }
    "#;
    assert_eq!(compile_and_run(src), 0);
}

// ═══════════════════════════════════════════════════════════════════════
// println(bool) tests
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn native_println_bool_true() {
    let src = r#"
        fn main() -> i64 {
            println(true)
            0
        }
    "#;
    assert_eq!(compile_and_run(src), 0);
}

#[test]
fn native_println_bool_comparison() {
    let src = r#"
        fn main() -> i64 {
            let x = 5
            println(x > 3)
            0
        }
    "#;
    assert_eq!(compile_and_run(src), 0);
}

#[test]
fn native_println_no_args() {
    let src = r#"
        fn main() -> i64 {
            println("before")
            println()
            println("after")
            0
        }
    "#;
    assert_eq!(compile_and_run(src), 0);
}

// ═══════════════════════════════════════════════════════════════════════
// type_of builtin tests
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn native_type_of_int() {
    let src = r#"
        fn main() -> i64 {
            let t = type_of(42)
            t.len()
        }
    "#;
    assert_eq!(compile_and_run(src), 3); // "i64" has length 3
}

#[test]
fn native_type_of_float() {
    let src = r#"
        fn main() -> i64 {
            let t = type_of(3.14)
            t.len()
        }
    "#;
    assert_eq!(compile_and_run(src), 3); // "f64" has length 3
}

#[test]
fn native_type_of_string() {
    let src = r#"
        fn main() -> i64 {
            let t = type_of("hello")
            t.len()
        }
    "#;
    assert_eq!(compile_and_run(src), 3); // "str" has length 3
}

// ═══════════════════════════════════════════════════════════════════════
// assert builtin tests
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn native_assert_true() {
    let src = r#"
        fn main() -> i64 {
            assert(true)
            assert(1 == 1)
            assert(5 > 3)
            42
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_assert_int_nonzero() {
    let src = r#"
        fn main() -> i64 {
            assert(1)
            assert(42)
            0
        }
    "#;
    assert_eq!(compile_and_run(src), 0);
}

// ═══════════════════════════════════════════════════════════════════════
// File I/O builtin tests
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn native_write_file_returns_ok() {
    let src = r#"
        fn main() -> i64 {
            let result = write_file("/tmp/fj_native_test.txt", "hello native")
            result
        }
    "#;
    assert_eq!(compile_and_run(src), 0); // 0 = Ok tag
    // Cleanup
    let _ = std::fs::remove_file("/tmp/fj_native_test.txt");
}

#[test]
fn native_file_exists_true() {
    // Create file first
    std::fs::write("/tmp/fj_native_exists.txt", "test").unwrap();
    let src = r#"
        fn main() -> i64 {
            file_exists("/tmp/fj_native_exists.txt")
        }
    "#;
    assert_eq!(compile_and_run(src), 1); // 1 = true
    let _ = std::fs::remove_file("/tmp/fj_native_exists.txt");
}

#[test]
fn native_file_exists_false() {
    let src = r#"
        fn main() -> i64 {
            file_exists("/tmp/fj_native_no_such_file_99999.txt")
        }
    "#;
    assert_eq!(compile_and_run(src), 0); // 0 = false
}

#[test]
fn native_write_and_file_exists() {
    let src = r#"
        fn main() -> i64 {
            write_file("/tmp/fj_native_wfe.txt", "data")
            let exists = file_exists("/tmp/fj_native_wfe.txt")
            exists
        }
    "#;
    assert_eq!(compile_and_run(src), 1);
    let _ = std::fs::remove_file("/tmp/fj_native_wfe.txt");
}

// ═══════════════════════════════════════════════════════════════════════
// Pipeline operator (|>) tests
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn native_pipe_simple() {
    let src = r#"
        fn double(x: i64) -> i64 { x * 2 }
        fn main() -> i64 {
            5 |> double
        }
    "#;
    assert_eq!(compile_and_run(src), 10);
}

#[test]
fn native_pipe_chain() {
    let src = r#"
        fn double(x: i64) -> i64 { x * 2 }
        fn add_one(x: i64) -> i64 { x + 1 }
        fn main() -> i64 {
            5 |> double |> add_one
        }
    "#;
    assert_eq!(compile_and_run(src), 11);
}
