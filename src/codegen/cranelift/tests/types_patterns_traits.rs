//! Type propagation, pattern matching, memory, struct/tuple fields, trait dispatch (A.8-A.12, B.3-B.6).

use super::super::*;
use super::{compile_and_run, compile_and_run_f64};
use crate::lexer::tokenize;
use crate::parser::parse;

// ===== A.8 — Type Propagation Completeness Tests =====

#[test]
fn native_a8_unary_neg_preserves_float_type() {
    let src = r#"
        fn main() -> f64 {
            let x: f64 = 3.14
            let y = -x
            y
        }
    "#;
    let result = compile_and_run_f64(src);
    assert!((result - (-3.14)).abs() < 1e-10);
}

#[test]
fn native_a8_if_else_with_neg_float() {
    let src = r#"
        fn main() -> f64 {
            let x: f64 = 3.14
            if true { -x } else { 0.0 }
        }
    "#;
    let result = compile_and_run_f64(src);
    assert!((result - (-3.14)).abs() < 1e-10);
}

#[test]
fn native_a8_match_returns_float() {
    let src = r#"
        fn main() -> f64 {
            let x = 1
            match x {
                1 => 3.14,
                _ => 0.0
            }
        }
    "#;
    let result = compile_and_run_f64(src);
    assert!((result - 3.14).abs() < 1e-10);
}

#[test]
fn native_a8_block_tail_preserves_float() {
    let src = r#"
        fn main() -> f64 {
            let x: f64 = {
                let a = 1
                2.5
            }
            x
        }
    "#;
    let result = compile_and_run_f64(src);
    assert!((result - 2.5).abs() < 1e-10);
}

#[test]
fn native_a8_pipe_preserves_return_type() {
    let src = r#"
        fn double(x: i64) -> i64 { x * 2 }
        fn main() -> i64 {
            let result = 5 |> double
            result
        }
    "#;
    assert_eq!(compile_and_run(src), 10);
}

#[test]
fn native_a8_method_call_type_propagation() {
    let src = r#"
        fn main() -> i64 {
            let mut arr = []
            arr.push(10)
            arr.push(20)
            arr.len()
        }
    "#;
    assert_eq!(compile_and_run(src), 2);
}

#[test]
fn native_a8_while_type_is_int() {
    let src = r#"
        fn main() -> i64 {
            let mut i = 0
            while i < 5 {
                i = i + 1
            }
            i
        }
    "#;
    assert_eq!(compile_and_run(src), 5);
}

#[test]
fn native_a8_for_type_is_int() {
    let src = r#"
        fn main() -> i64 {
            let mut sum = 0
            for i in 0..5 {
                sum = sum + i
            }
            sum
        }
    "#;
    assert_eq!(compile_and_run(src), 10);
}

#[test]
fn native_a8_index_type_is_int() {
    let src = r#"
        fn main() -> i64 {
            let arr = [10, 20, 30]
            let x = arr[1]
            x + 5
        }
    "#;
    assert_eq!(compile_and_run(src), 25);
}

#[test]
fn native_a8_match_wildcard_float() {
    let src = r#"
        fn main() -> f64 {
            let x = 99
            match x {
                0 => 1.0,
                _ => 2.5
            }
        }
    "#;
    let result = compile_and_run_f64(src);
    assert!((result - 2.5).abs() < 1e-10);
}

// ===== A.9 — Pattern Matching Completeness Tests =====

#[test]
fn native_a9_tuple_pattern_destructure() {
    let src = r#"
        fn main() -> i64 {
            let t = (10, 20)
            match t {
                (a, b) => a + b
            }
        }
    "#;
    assert_eq!(compile_and_run(src), 30);
}

#[test]
fn native_a9_tuple_pattern_with_wildcard() {
    let src = r#"
        fn main() -> i64 {
            let t = (5, 99)
            match t {
                (x, _) => x * 3
            }
        }
    "#;
    assert_eq!(compile_and_run(src), 15);
}

#[test]
fn native_a9_struct_pattern_destructure() {
    let src = r#"
        struct Point { x: i64, y: i64 }
        fn main() -> i64 {
            let p = Point { x: 3, y: 4 }
            match p {
                Point { x, y } => x + y
            }
        }
    "#;
    assert_eq!(compile_and_run(src), 7);
}

#[test]
fn native_a9_range_pattern_match() {
    let src = r#"
        fn main() -> i64 {
            let x = 5
            match x {
                1..10 => 1,
                _ => 0
            }
        }
    "#;
    assert_eq!(compile_and_run(src), 1);
}

#[test]
fn native_a9_range_pattern_no_match() {
    let src = r#"
        fn main() -> i64 {
            let x = 15
            match x {
                1..10 => 1,
                _ => 0
            }
        }
    "#;
    assert_eq!(compile_and_run(src), 0);
}

#[test]
fn native_a9_range_pattern_inclusive() {
    let src = r#"
        fn main() -> i64 {
            let x = 10
            match x {
                1..=10 => 1,
                _ => 0
            }
        }
    "#;
    assert_eq!(compile_and_run(src), 1);
}

#[test]
fn native_a9_enum_match_still_works() {
    let src = r#"
        fn main() -> i64 {
            let x = Some(42)
            match x {
                Some(v) => v,
                None => -1
            }
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_a9_struct_pattern_single_field() {
    let src = r#"
        struct Wrapper { val: i64 }
        fn main() -> i64 {
            let w = Wrapper { val: 77 }
            match w {
                Wrapper { val } => val
            }
        }
    "#;
    assert_eq!(compile_and_run(src), 77);
}

// ── A.10: Memory Management ─────────────────────────────────────────

#[test]
fn native_a10_heap_array_cleanup_on_return() {
    // Heap array is allocated and freed at function exit.
    // If cleanup were missing, this would leak (no crash, but tests
    // confirm the codegen path works without errors).
    let src = r#"
        fn main() -> i64 {
            let arr: [i64] = []
            42
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_a10_heap_array_with_push_cleanup() {
    // Heap array with pushed elements is cleaned up.
    let src = r#"
        fn compute() -> i64 {
            let arr: [i64] = []
            arr.push(10)
            arr.push(20)
            arr.push(30)
            arr[1]
        }
        fn main() -> i64 { compute() }
    "#;
    assert_eq!(compile_and_run(src), 20);
}

#[test]
fn native_a10_string_concat_cleanup() {
    // Concat produces a heap-allocated string that should be freed.
    // The function returns an integer, so the string must be freed.
    let src = r#"
        fn compute() -> i64 {
            let a = "hello"
            let b = " world"
            let c = a + b
            42
        }
        fn main() -> i64 { compute() }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_a10_multiple_owned_cleanup() {
    // Multiple heap resources (array + string concat) in one function.
    let src = r#"
        fn compute() -> i64 {
            let arr: [i64] = []
            arr.push(1)
            let a = "foo"
            let b = "bar"
            let c = a + b
            arr[0]
        }
        fn main() -> i64 { compute() }
    "#;
    assert_eq!(compile_and_run(src), 1);
}

#[test]
fn native_a10_early_return_cleanup() {
    // Early return should still emit cleanup for owned resources.
    let src = r#"
        fn compute() -> i64 {
            let arr: [i64] = []
            arr.push(99)
            return arr[0]
        }
        fn main() -> i64 { compute() }
    "#;
    assert_eq!(compile_and_run(src), 99);
}

#[test]
fn native_a10_no_cleanup_for_static_strings() {
    // String literals are static data — should NOT be freed.
    let src = r#"
        fn main() -> i64 {
            let s = "hello"
            5
        }
    "#;
    assert_eq!(compile_and_run(src), 5);
}

#[test]
fn native_a10_owned_ptrs_tracked_correctly() {
    // Verify the codegen produces valid IR even with multiple
    // owned resources and a non-trivial control flow.
    let src = r#"
        fn compute(x: i64) -> i64 {
            let arr: [i64] = []
            arr.push(x)
            arr.push(x + 1)
            if x > 0 {
                arr[0]
            } else {
                arr[1]
            }
        }
        fn main() -> i64 { compute(10) }
    "#;
    assert_eq!(compile_and_run(src), 10);
}

// ── A.11: Type-Aware Struct & Tuple Fields ──────────────────────────

#[test]
fn native_a11_struct_f64_field() {
    let src = r#"
        struct Circle { radius: f64 }
        fn main() -> i64 {
            let c = Circle { radius: 3.14 }
            let r = c.radius
            if r > 3.0 { 1 } else { 0 }
        }
    "#;
    assert_eq!(compile_and_run(src), 1);
}

#[test]
fn native_a11_struct_f64_field_roundtrip() {
    // Store f64, load f64, use in float comparison
    let src = r#"
        struct Point { x: f64, y: f64 }
        fn main() -> i64 {
            let p = Point { x: 1.5, y: 2.5 }
            let sum = p.x + p.y
            if sum > 3.9 { 1 } else { 0 }
        }
    "#;
    assert_eq!(compile_and_run(src), 1);
}

#[test]
fn native_a11_struct_mixed_fields() {
    // i64 and f64 fields in the same struct
    let src = r#"
        struct Rect { width: f64, height: f64, count: i64 }
        fn main() -> i64 {
            let r = Rect { width: 5.0, height: 3.0, count: 7 }
            r.count
        }
    "#;
    assert_eq!(compile_and_run(src), 7);
}

#[test]
fn native_a11_struct_f64_field_assign() {
    let src = r#"
        struct Acc { total: f64 }
        fn main() -> i64 {
            let mut a = Acc { total: 1.0 }
            a.total += 2.5
            if a.total > 3.4 { 1 } else { 0 }
        }
    "#;
    assert_eq!(compile_and_run(src), 1);
}

#[test]
fn native_a11_tuple_mixed_types() {
    // Tuple with i64 and f64 elements
    let src = r#"
        fn main() -> i64 {
            let t = (42, 3.14)
            t.0
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_a11_tuple_f64_element() {
    // Access f64 element from tuple, use in float comparison
    let src = r#"
        fn main() -> i64 {
            let t = (10, 2.5)
            let v = t.1
            if v > 2.0 { 1 } else { 0 }
        }
    "#;
    assert_eq!(compile_and_run(src), 1);
}

#[test]
fn native_a11_struct_pattern_f64_field() {
    // Pattern match destructuring with f64 fields
    let src = r#"
        struct Vec2 { x: f64, y: f64 }
        fn main() -> i64 {
            let v = Vec2 { x: 1.5, y: 2.5 }
            match v {
                Vec2 { x, y } => {
                    if x + y > 3.0 { 1 } else { 0 }
                }
            }
        }
    "#;
    assert_eq!(compile_and_run(src), 1);
}

#[test]
fn native_a11_struct_bool_field() {
    // Bool field stored and loaded correctly
    let src = r#"
        struct Config { enabled: i64, count: i64 }
        fn main() -> i64 {
            let c = Config { enabled: 1, count: 42 }
            if c.enabled > 0 { c.count } else { 0 }
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

// ── A.12: Codegen Completeness Polish ───────────────────────────────

#[test]
fn native_a12_field_div_assign() {
    let src = r#"
        struct Counter { val: i64 }
        fn main() -> i64 {
            let mut c = Counter { val: 100 }
            c.val /= 5
            c.val
        }
    "#;
    assert_eq!(compile_and_run(src), 20);
}

#[test]
fn native_a12_field_rem_assign() {
    let src = r#"
        struct Counter { val: i64 }
        fn main() -> i64 {
            let mut c = Counter { val: 17 }
            c.val %= 5
            c.val
        }
    "#;
    assert_eq!(compile_and_run(src), 2);
}

#[test]
fn native_a12_field_bitand_assign() {
    let src = r#"
        struct Mask { bits: i64 }
        fn main() -> i64 {
            let mut m = Mask { bits: 15 }
            m.bits &= 6
            m.bits
        }
    "#;
    assert_eq!(compile_and_run(src), 6);
}

#[test]
fn native_a12_field_bitor_assign() {
    let src = r#"
        struct Mask { bits: i64 }
        fn main() -> i64 {
            let mut m = Mask { bits: 3 }
            m.bits |= 12
            m.bits
        }
    "#;
    assert_eq!(compile_and_run(src), 15);
}

#[test]
fn native_a12_field_bitxor_assign() {
    let src = r#"
        struct Mask { bits: i64 }
        fn main() -> i64 {
            let mut m = Mask { bits: 15 }
            m.bits ^= 9
            m.bits
        }
    "#;
    // 15 = 0b1111, 9 = 0b1001, xor = 0b0110 = 6
    assert_eq!(compile_and_run(src), 6);
}

#[test]
fn native_a12_field_shl_assign() {
    let src = r#"
        struct Reg { val: i64 }
        fn main() -> i64 {
            let mut r = Reg { val: 1 }
            r.val <<= 4
            r.val
        }
    "#;
    assert_eq!(compile_and_run(src), 16);
}

#[test]
fn native_a12_field_shr_assign() {
    let src = r#"
        struct Reg { val: i64 }
        fn main() -> i64 {
            let mut r = Reg { val: 64 }
            r.val >>= 3
            r.val
        }
    "#;
    assert_eq!(compile_and_run(src), 8);
}

#[test]
fn native_a12_field_f64_div_assign() {
    let src = r#"
        struct Acc { total: f64 }
        fn main() -> i64 {
            let mut a = Acc { total: 10.0 }
            a.total /= 4.0
            if a.total > 2.4 { 1 } else { 0 }
        }
    "#;
    assert_eq!(compile_and_run(src), 1);
}

#[test]
fn native_a12_pipe_to_call_with_args() {
    // `x |> f(y)` desugars to `f(x, y)`
    let src = r#"
        fn add(a: i64, b: i64) -> i64 { a + b }
        fn main() -> i64 {
            5 |> add(10)
        }
    "#;
    assert_eq!(compile_and_run(src), 15);
}

#[test]
fn native_a12_pipe_to_call_chain() {
    // Chained pipes with call syntax
    let src = r#"
        fn add(a: i64, b: i64) -> i64 { a + b }
        fn mul(a: i64, b: i64) -> i64 { a * b }
        fn main() -> i64 {
            2 |> add(3) |> mul(4)
        }
    "#;
    // (2 + 3) * 4 = 20
    assert_eq!(compile_and_run(src), 20);
}

#[test]
fn native_a12_pipe_to_ident_still_works() {
    // Existing `x |> f` syntax preserved
    let src = r#"
        fn double(x: i64) -> i64 { x * 2 }
        fn main() -> i64 {
            7 |> double
        }
    "#;
    assert_eq!(compile_and_run(src), 14);
}

#[test]
fn native_a12_cast_float_to_bool_true() {
    let src = r#"
        fn main() -> i64 {
            let x = 1.5 as bool
            x
        }
    "#;
    assert_eq!(compile_and_run(src), 1);
}

#[test]
fn native_a12_cast_float_to_bool_false() {
    let src = r#"
        fn main() -> i64 {
            let x = 0.0 as bool
            x
        }
    "#;
    assert_eq!(compile_and_run(src), 0);
}

#[test]
fn native_a12_cast_bool_to_f64() {
    let src = r#"
        fn main() -> i64 {
            let x = 1 as f64
            if x > 0.5 { 1 } else { 0 }
        }
    "#;
    assert_eq!(compile_and_run(src), 1);
}

#[test]
fn native_a12_cast_unsupported_returns_error() {
    let src = r#"
        fn main() -> i64 {
            let x = 42 as Tensor
            x
        }
    "#;
    // Should return a compile error, not silently pass through
    let result = std::panic::catch_unwind(|| compile_and_run(src));
    assert!(result.is_err());
}

// ── B.3: Static Trait Dispatch in Codegen ───────────────────────────

#[test]
fn native_b3_trait_impl_method_dispatch() {
    // Trait defined, impl for struct, call via struct method syntax
    let src = r#"
        trait Computable {
            fn compute(&self) -> i64 { }
        }
        struct Data { val: i64 }
        impl Computable for Data {
            fn compute(&self) -> i64 { self.val * 2 }
        }
        fn main() -> i64 {
            let d = Data { val: 21 }
            d.compute()
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_b3_trait_qualified_call() {
    // Call via Trait::method(obj) syntax
    let src = r#"
        trait Describable {
            fn describe(&self) -> i64 { }
        }
        struct Item { id: i64 }
        impl Describable for Item {
            fn describe(&self) -> i64 { self.id + 100 }
        }
        fn main() -> i64 {
            let item = Item { id: 5 }
            Describable::describe(item)
        }
    "#;
    assert_eq!(compile_and_run(src), 105);
}

#[test]
fn native_b3_multiple_trait_methods() {
    let src = r#"
        trait Shape {
            fn area(&self) -> i64 { }
            fn perimeter(&self) -> i64 { }
        }
        struct Rect { w: i64, h: i64 }
        impl Shape for Rect {
            fn area(&self) -> i64 { self.w * self.h }
            fn perimeter(&self) -> i64 { 2 * (self.w + self.h) }
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
fn native_b3_trait_defs_collected() {
    // Verifies trait definitions are collected without error
    let src = r#"
        trait Printable {
            fn display(&self) -> i64 { }
        }
        struct Num { val: i64 }
        impl Printable for Num {
            fn display(&self) -> i64 { self.val }
        }
        fn main() -> i64 {
            let n = Num { val: 77 }
            n.display()
        }
    "#;
    assert_eq!(compile_and_run(src), 77);
}

#[test]
fn native_b3_inherent_and_trait_impl_coexist() {
    // Struct has both inherent methods and trait impls
    let src = r#"
        trait Valuable {
            fn value(&self) -> i64 { }
        }
        struct Coin { amount: i64 }
        impl Coin {
            fn double_amount(&self) -> i64 { self.amount * 2 }
        }
        impl Valuable for Coin {
            fn value(&self) -> i64 { self.amount }
        }
        fn main() -> i64 {
            let c = Coin { amount: 50 }
            c.value() + c.double_amount()
        }
    "#;
    assert_eq!(compile_and_run(src), 150);
}

#[test]
fn native_b3_trait_method_with_args() {
    let src = r#"
        trait Addable {
            fn add_to(&self, x: i64) -> i64 { }
        }
        struct Counter { count: i64 }
        impl Addable for Counter {
            fn add_to(&self, x: i64) -> i64 { self.count + x }
        }
        fn main() -> i64 {
            let c = Counter { count: 10 }
            c.add_to(32)
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

// ═══════════════════════════════════════════════════════════════
// B.2 — Type-checked destructuring
// ═══════════════════════════════════════════════════════════════

#[test]
fn native_b2_enum_f64_payload_destructure() {
    // Enum with f64 payload: destructure and use as f64
    let src = r#"
        enum Value {
            Int(i64),
            Float(f64),
        }
        fn main() -> f64 {
            let v = Float(3.14)
            match v {
                Float(x) => x,
                Int(n) => 0.0,
            }
        }
    "#;
    let result = compile_and_run_f64(src);
    assert!((result - 3.14).abs() < 1e-10);
}

#[test]
fn native_b2_enum_variant_type_tracking() {
    // Enum payload type preserved through match destructuring
    let src = r#"
        enum Wrapper { Val(i64) }
        fn main() -> i64 {
            let w = Val(42)
            match w {
                Val(x) => x + 1,
            }
        }
    "#;
    assert_eq!(compile_and_run(src), 43);
}

#[test]
fn native_b2_some_payload_type_preserved() {
    // Some() preserves the payload type through match
    let src = r#"
        fn main() -> i64 {
            let x = Some(99)
            match x {
                Some(v) => v,
                None => 0,
            }
        }
    "#;
    assert_eq!(compile_and_run(src), 99);
}

#[test]
fn native_b2_tuple_pattern_type_aware() {
    // Tuple pattern loads elements with correct types
    let src = r#"
        fn main() -> i64 {
            let t = (10, 20, 30)
            match t {
                (a, b, c) => a + b + c,
            }
        }
    "#;
    assert_eq!(compile_and_run(src), 60);
}

#[test]
fn native_b2_struct_pattern_type_aware_f64() {
    // Struct pattern binds fields with correct Cranelift types
    let src = r#"
        struct Measurement { value: f64, count: i64 }
        fn main() -> i64 {
            let m = Measurement { value: 2.5, count: 4 }
            match m {
                Measurement { value, count } => count,
            }
        }
    "#;
    assert_eq!(compile_and_run(src), 4);
}

#[test]
fn native_b2_match_ident_binding_type() {
    // Catch-all ident pattern uses the subject's type
    let src = r#"
        fn main() -> i64 {
            let x = 42
            match x {
                n => n + 1,
            }
        }
    "#;
    assert_eq!(compile_and_run(src), 43);
}

#[test]
fn native_b2_enum_payload_type_in_variant_types() {
    // Verify enum payload types work for user-defined enums with multiple variants
    let src = r#"
        enum Shape {
            Circle(f64),
            Square(i64),
        }
        fn main() -> i64 {
            let s = Square(7)
            match s {
                Square(side) => side * side,
                Circle(r) => 0,
            }
        }
    "#;
    assert_eq!(compile_and_run(src), 49);
}

#[test]
fn native_b2_define_function_error_recovery() {
    // Verify that define_function error doesn't poison builder_ctx
    // (func_ctx.is_empty() fix). Compile a program where a called function
    // fails but the overall compile returns a proper error, not a panic.
    // Note: broken() must be called from main, otherwise DCE eliminates it.
    let tokens =
        tokenize("fn broken() -> i64 { unknown_var } fn main() -> i64 { broken() }").expect("lex");
    let program = parse(tokens).expect("parse");
    let mut compiler = CraneliftCompiler::new().expect("init");
    // Should return Err, not panic
    let result = compiler.compile_program(&program);
    assert!(result.is_err());
}

// ── B.5: Trait dispatch correctness tests ──

#[test]
fn native_b5_two_impls_qualified_call_a() {
    // Two types implement same trait — Trait::method(type_a) calls A's impl
    let src = r#"
        trait Compute {
            fn calc(&self) -> i64 { }
        }
        struct Alpha { v: i64 }
        struct Beta { v: i64 }
        impl Compute for Alpha {
            fn calc(&self) -> i64 { self.v + 100 }
        }
        impl Compute for Beta {
            fn calc(&self) -> i64 { self.v + 200 }
        }
        fn main() -> i64 {
            let a = Alpha { v: 5 }
            Compute::calc(a)
        }
    "#;
    assert_eq!(compile_and_run(src), 105);
}

#[test]
fn native_b5_two_impls_qualified_call_b() {
    // Two types implement same trait — Trait::method(type_b) calls B's impl
    let src = r#"
        trait Compute {
            fn calc(&self) -> i64 { }
        }
        struct Alpha { v: i64 }
        struct Beta { v: i64 }
        impl Compute for Alpha {
            fn calc(&self) -> i64 { self.v + 100 }
        }
        impl Compute for Beta {
            fn calc(&self) -> i64 { self.v + 200 }
        }
        fn main() -> i64 {
            let b = Beta { v: 5 }
            Compute::calc(b)
        }
    "#;
    assert_eq!(compile_and_run(src), 205);
}

#[test]
fn native_b5_two_impls_method_dispatch() {
    // obj.method() dispatch with multiple impls
    let src = r#"
        trait Compute {
            fn calc(&self) -> i64 { }
        }
        struct Alpha { v: i64 }
        struct Beta { v: i64 }
        impl Compute for Alpha {
            fn calc(&self) -> i64 { self.v + 100 }
        }
        impl Compute for Beta {
            fn calc(&self) -> i64 { self.v + 200 }
        }
        fn main() -> i64 {
            let a = Alpha { v: 5 }
            let b = Beta { v: 5 }
            a.calc() + b.calc()
        }
    "#;
    assert_eq!(compile_and_run(src), 310);
}

#[test]
fn native_b5_trait_method_not_in_def_error() {
    // Trait::non_existent_method(obj) → error
    let src = r#"
        trait Compute {
            fn calc(&self) -> i64 { }
        }
        struct Alpha { v: i64 }
        impl Compute for Alpha {
            fn calc(&self) -> i64 { self.v }
        }
        fn main() -> i64 {
            let a = Alpha { v: 5 }
            Compute::bogus(a)
        }
    "#;
    let tokens = tokenize(src).expect("lex");
    let program = parse(tokens).expect("parse");
    let mut compiler = CraneliftCompiler::new().expect("init");
    let result = compiler.compile_program(&program);
    assert!(result.is_err());
}

#[test]
fn native_b5_trait_no_impl_error() {
    // Trait::method on type with no impl → error
    let src = r#"
        trait Compute {
            fn calc(&self) -> i64 { }
        }
        struct Alpha { v: i64 }
        struct Beta { v: i64 }
        impl Compute for Alpha {
            fn calc(&self) -> i64 { self.v }
        }
        fn main() -> i64 {
            let b = Beta { v: 5 }
            Compute::calc(b)
        }
    "#;
    let tokens = tokenize(src).expect("lex");
    let program = parse(tokens).expect("parse");
    let mut compiler = CraneliftCompiler::new().expect("init");
    let result = compiler.compile_program(&program);
    assert!(result.is_err());
}

// ── B.6: Destructuring robustness tests ──

#[test]
fn native_b6_enum_no_payload_variant() {
    // No-payload variant (None) works without binding
    let src = r#"
        fn main() -> i64 {
            let x = None
            match x {
                Some(v) => v,
                None => 42,
            }
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_b6_match_wildcard_fallback() {
    // Wildcard arm catches unmatched values
    let src = r#"
        fn main() -> i64 {
            let x = 99
            match x {
                1 => 10,
                2 => 20,
                _ => 99,
            }
        }
    "#;
    assert_eq!(compile_and_run(src), 99);
}

#[test]
fn native_b6_match_enum_multiple_variants() {
    // Match with multiple enum variants, only one matches
    let src = r#"
        enum Color { Red, Green, Blue }
        fn main() -> i64 {
            let c = Green
            match c {
                Red => 1,
                Green => 2,
                Blue => 3,
            }
        }
    "#;
    assert_eq!(compile_and_run(src), 2);
}

#[test]
fn native_b6_match_ident_binding() {
    // Ident pattern binds the full subject value
    let src = r#"
        fn main() -> i64 {
            let x = 7
            match x {
                n => n * n,
            }
        }
    "#;
    assert_eq!(compile_and_run(src), 49);
}

#[test]
fn native_b6_match_merge_type_f64() {
    // Match arms returning f64 should use f64 merge type
    let src = r#"
        fn main() -> f64 {
            let x = 1
            match x {
                1 => 3.14,
                _ => 2.72,
            }
        }
    "#;
    let result = compile_and_run_f64(src);
    assert!((result - 3.14).abs() < 1e-10);
}

#[test]
fn native_b6_enum_single_field_doc() {
    // Single-field enum payloads work correctly (multi-field deferred to v0.2)
    let src = r#"
        enum Msg { Hello(i64), Bye }
        fn main() -> i64 {
            let m = Hello(100)
            match m {
                Hello(val) => val + 1,
                Bye => 0,
            }
        }
    "#;
    assert_eq!(compile_and_run(src), 101);
}
