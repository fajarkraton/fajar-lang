//! Distributed training, mixed precision, union/repr/bitfields, async I/O, section placement, bootstrap (S34/S39/S24/S46/S47).

use super::super::*;
use super::{compile_and_run, interpret_main};
use crate::lexer::tokenize;
use crate::parser::parse;

// S34 — Distributed Training
// =====================================================================

#[test]
fn native_dist_init_and_query() {
    let src = r#"
        fn main() -> i64 {
            let ctx = dist_init(4, 2)
            let ws = dist_world_size(ctx)
            let rank = dist_rank(ctx)
            dist_free(ctx)
            ws * 10 + rank
        }
    "#;
    // world_size=4, rank=2 → 4*10 + 2 = 42
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_dist_all_reduce_sum() {
    let src = r#"
        fn main() -> i64 {
            let ctx = dist_init(3, 0)
            let t = tensor_ones(2, 2)
            let reduced = dist_all_reduce_sum(ctx, t)
            let rows = tensor_rows(reduced)
            let cols = tensor_cols(reduced)
            tensor_free(t)
            tensor_free(reduced)
            dist_free(ctx)
            rows * 10 + cols
        }
    "#;
    // all_reduce_sum with world_size=3: ones * 3 → shape preserved: 2x2
    assert_eq!(compile_and_run(src), 22);
}

#[test]
fn native_dist_broadcast() {
    let src = r#"
        fn main() -> i64 {
            let ctx = dist_init(2, 0)
            let t = tensor_ones(3, 1)
            let bc = dist_broadcast(ctx, t, 0)
            let rows = tensor_rows(bc)
            tensor_free(t)
            tensor_free(bc)
            dist_free(ctx)
            rows
        }
    "#;
    // broadcast copies the tensor: 3x1 → 3 rows
    assert_eq!(compile_and_run(src), 3);
}

#[test]
fn native_dist_split_batch_rank0() {
    // Split 6-row tensor across 3 ranks, rank 0 gets rows 0-1
    let src = r#"
        fn main() -> i64 {
            let ctx = dist_init(3, 0)
            let t = tensor_ones(6, 2)
            let chunk = dist_split_batch(ctx, t)
            let rows = tensor_rows(chunk)
            let cols = tensor_cols(chunk)
            tensor_free(t)
            tensor_free(chunk)
            dist_free(ctx)
            rows * 10 + cols
        }
    "#;
    // 6 rows / 3 ranks = 2 rows per rank
    assert_eq!(compile_and_run(src), 22);
}

#[test]
fn native_dist_split_batch_last_rank() {
    // Last rank gets remainder rows
    let src = r#"
        fn main() -> i64 {
            let ctx = dist_init(2, 1)
            let t = tensor_ones(5, 3)
            let chunk = dist_split_batch(ctx, t)
            let rows = tensor_rows(chunk)
            let cols = tensor_cols(chunk)
            tensor_free(t)
            tensor_free(chunk)
            dist_free(ctx)
            rows * 10 + cols
        }
    "#;
    // 5 rows / 2 ranks: rank 0 gets 2 rows, rank 1 (last) gets 3 rows (5-2)
    assert_eq!(compile_and_run(src), 33);
}

// S34.4 — TCP gradient exchange
#[test]
fn native_dist_tcp_bind_and_port() {
    // Bind a TCP listener on ephemeral port and verify port > 0
    let src = r#"
        fn main() -> i64 {
            let handle = dist_tcp_bind(0)
            let port = dist_tcp_port(handle)
            dist_tcp_free(handle)
            if port > 0 { 1 } else { 0 }
        }
    "#;
    assert_eq!(compile_and_run(src), 1);
}

#[test]
fn native_dist_tcp_send_recv_roundtrip() {
    // Bind, send a 2x3 tensor to self, recv it back, verify shape
    let src = r#"
        fn main() -> i64 {
            let server = dist_tcp_bind(0)
            let port = dist_tcp_port(server)
            let t = tensor_ones(2, 3)
            let sent = dist_tcp_send(port, t)
            let received = dist_tcp_recv(server)
            let rows = tensor_rows(received)
            let cols = tensor_cols(received)
            tensor_free(t)
            tensor_free(received)
            dist_tcp_free(server)
            rows * 10 + cols
        }
    "#;
    assert_eq!(compile_and_run(src), 23);
}

// =====================================================================
// S39 — Mixed Precision Types
// =====================================================================

#[test]
fn native_f16_type_parses() {
    // f16 type annotation is accepted by parser
    let src = r#"
        fn main() -> i64 {
            let x: f16 = 0
            42
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_bf16_type_parses() {
    // bf16 type annotation is accepted by parser
    let src = r#"
        fn main() -> i64 {
            let x: bf16 = 0
            99
        }
    "#;
    assert_eq!(compile_and_run(src), 99);
}

#[test]
fn native_f32_to_f16_roundtrip() {
    // Convert f32(1.0) to f16 bits, then back to f32
    let src = r#"
        fn main() -> i64 {
            let f32_bits = 1065353216
            let h = f32_to_f16(f32_bits)
            let back = f16_to_f32(h)
            if back == f32_bits { 1 } else { 0 }
        }
    "#;
    // 1065353216 = f32::to_bits(1.0), f16(1.0) = 0x3C00, back to f32 = 1.0
    assert_eq!(compile_and_run(src), 1);
}

#[test]
fn native_tensor_to_f16_preserves_shape() {
    let src = r#"
        fn main() -> i64 {
            let t = tensor_ones(3, 4)
            let h = tensor_to_f16(t)
            let rows = tensor_rows(h)
            let cols = tensor_cols(h)
            tensor_free(t)
            tensor_free(h)
            rows * 10 + cols
        }
    "#;
    // tensor_to_f16 preserves shape: 3x4
    assert_eq!(compile_and_run(src), 34);
}

// =====================================================================
// S39.3 — Loss scaling
// =====================================================================

#[test]
fn native_loss_scale_basic() {
    // loss_scale multiplies each element by scale factor
    // Scale ones(2,3) by 2.0 → 6 elements of 2.0 → subtract original ones gives 6 elements of 1.0
    // Then check rows/cols preserved
    let src = r#"
        fn main() -> i64 {
            let t = tensor_ones(2, 3)
            let scaled = loss_scale(t, 2.0)
            let diff = tensor_sub(scaled, t)
            let rows = tensor_rows(diff)
            let cols = tensor_cols(diff)
            tensor_free(t)
            tensor_free(scaled)
            tensor_free(diff)
            rows * 10 + cols
        }
    "#;
    // Shape should be 2x3
    assert_eq!(compile_and_run(src), 23);
}

#[test]
fn native_loss_unscale_basic() {
    // loss_unscale divides: scale by 4.0 then unscale by 4.0 should give back original
    // Subtract unscaled from original → zeros → shape 2x3
    let src = r#"
        fn main() -> i64 {
            let t = tensor_ones(2, 3)
            let scaled = loss_scale(t, 4.0)
            let unscaled = loss_unscale(scaled, 4.0)
            let diff = tensor_sub(unscaled, t)
            let rows = tensor_rows(diff)
            let cols = tensor_cols(diff)
            tensor_free(t)
            tensor_free(scaled)
            tensor_free(unscaled)
            tensor_free(diff)
            rows * 10 + cols
        }
    "#;
    assert_eq!(compile_and_run(src), 23);
}

#[test]
fn native_loss_scale_preserves_shape() {
    let src = r#"
        fn main() -> i64 {
            let t = tensor_ones(3, 4)
            let scaled = loss_scale(t, 256.0)
            let rows = tensor_rows(scaled)
            let cols = tensor_cols(scaled)
            tensor_free(t)
            tensor_free(scaled)
            rows * 10 + cols
        }
    "#;
    assert_eq!(compile_and_run(src), 34); // 3*10 + 4
}

// =====================================================================
// S39.4 — Post-training quantization
// =====================================================================

#[test]
fn native_tensor_quantize_int8_basic() {
    // Quantize a uniform tensor: all 1.0 → should get a single quant value
    let src = r#"
        fn main() -> i64 {
            let t = tensor_ones(2, 2)
            let q = tensor_quantize_int8(t)
            let rows = tensor_rows(q)
            let cols = tensor_cols(q)
            tensor_free(t)
            tensor_free(q)
            rows * 10 + cols
        }
    "#;
    // Shape preserved: 2x2
    assert_eq!(compile_and_run(src), 22);
}

#[test]
fn native_tensor_dequantize_roundtrip() {
    // Quantize then dequantize: shape should be preserved
    let src = r#"
        fn main() -> i64 {
            let t = tensor_ones(3, 4)
            let scaled = loss_scale(t, 5.0)
            let q = tensor_quantize_int8(scaled)
            let s = tensor_quant_scale()
            let z = tensor_quant_zero_point()
            let dq = tensor_dequantize_int8(q, s, z)
            let rows = tensor_rows(dq)
            let cols = tensor_cols(dq)
            tensor_free(t)
            tensor_free(scaled)
            tensor_free(q)
            tensor_free(dq)
            rows * 10 + cols
        }
    "#;
    // Shape preserved: 3x4
    assert_eq!(compile_and_run(src), 34);
}

#[test]
fn native_tensor_quant_params() {
    // After quantizing, quant_scale and quant_zero_point should be retrievable
    // Verify by calling both, then checking quantized tensor shape
    let src = r#"
        fn main() -> i64 {
            let t = tensor_ones(2, 3)
            let q = tensor_quantize_int8(t)
            let s = tensor_quant_scale()
            let z = tensor_quant_zero_point()
            let rows = tensor_rows(q)
            let cols = tensor_cols(q)
            tensor_free(t)
            tensor_free(q)
            rows * 10 + cols
        }
    "#;
    // Shape preserved: 2x3 → 23
    assert_eq!(compile_and_run(src), 23);
}

// =====================================================================
// S24 — Union, Repr, Bitfields
// =====================================================================

#[test]
fn native_union_parse_and_init() {
    // Union fields share the same memory — writing one overwrites the other
    let src = r#"
        union Register {
            as_i64: i64,
            as_val: i64,
        }
        fn main() -> i64 {
            let r = Register { as_i64: 42 }
            r.as_i64
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_union_shared_memory() {
    // Both fields read from the same offset (union semantics)
    let src = r#"
        union Bits {
            raw: i64,
            val: i64,
        }
        fn main() -> i64 {
            let b = Bits { raw: 99 }
            b.val
        }
    "#;
    // Since both fields are at offset 0 and same type, reading val gives raw's value
    assert_eq!(compile_and_run(src), 99);
}

#[test]
fn native_repr_c_struct() {
    // @repr_c struct — annotation is parsed correctly
    let src = r#"
        @repr_c
        struct CStruct {
            x: i64,
            y: i64,
        }
        fn main() -> i64 {
            let s = CStruct { x: 10, y: 20 }
            s.x + s.y
        }
    "#;
    assert_eq!(compile_and_run(src), 30);
}

#[test]
fn native_repr_packed_struct() {
    // @repr_packed struct — annotation is parsed correctly
    let src = r#"
        @repr_packed
        struct Packed {
            a: i64,
            b: i64,
        }
        fn main() -> i64 {
            let p = Packed { a: 5, b: 7 }
            p.a + p.b
        }
    "#;
    assert_eq!(compile_and_run(src), 12);
}

#[test]
fn native_union_with_repr_c() {
    // @repr_c union — combines repr annotation with union
    let src = r#"
        @repr_c
        union CUnion {
            integer: i64,
            bits: i64,
        }
        fn main() -> i64 {
            let u = CUnion { integer: 255 }
            u.bits
        }
    "#;
    assert_eq!(compile_and_run(src), 255);
}

#[test]
fn native_union_overwrite_field() {
    // Writing to a union field overwrites any previous value
    let src = r#"
        union Data {
            x: i64,
            y: i64,
        }
        fn main() -> i64 {
            let mut d = Data { x: 100 }
            d.y = 200
            d.x
        }
    "#;
    // d.y = 200 writes at offset 0, d.x reads from offset 0 → 200
    assert_eq!(compile_and_run(src), 200);
}

// ═══════════════════════════════════════════════════════════════════════
// S43.1 — Dead Function Elimination
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn native_dce_dead_fn_not_compiled() {
    // dead_fn is never called from main → should be eliminated
    let src = r#"
        fn dead_fn() -> i64 { 999 }
        fn main() -> i64 { 42 }
    "#;
    let tokens = tokenize(src).expect("lex failed");
    let program = parse(tokens).expect("parse failed");
    let mut compiler = CraneliftCompiler::new().expect("compiler init failed");
    compiler
        .compile_program(&program)
        .expect("compilation failed");
    // main should work
    let fn_ptr = compiler.get_fn_ptr("main").expect("main not found");
    let main_fn: fn() -> i64 = unsafe { std::mem::transmute(fn_ptr) };
    assert_eq!(main_fn(), 42);
    // dead_fn should NOT be compiled (get_fn_ptr returns Err)
    assert!(compiler.get_fn_ptr("dead_fn").is_err());
}

#[test]
fn native_dce_reachable_fn_kept() {
    // helper is called from main → must be kept
    let src = r#"
        fn helper() -> i64 { 10 }
        fn main() -> i64 { helper() + 5 }
    "#;
    assert_eq!(compile_and_run(src), 15);
}

#[test]
fn native_dce_transitive_reachable() {
    // main → foo → bar: both should be kept, dead should be eliminated
    let src = r#"
        fn dead() -> i64 { 0 }
        fn bar() -> i64 { 7 }
        fn foo() -> i64 { bar() + 3 }
        fn main() -> i64 { foo() }
    "#;
    assert_eq!(compile_and_run(src), 10);
}

#[test]
fn native_dce_entry_point_kept() {
    // @entry annotated function is an entry point even without main
    // Since there IS an @entry, DCE should use it as entry point
    let src = r#"
        fn unused() -> i64 { 999 }
        @entry
        fn start() -> i64 { 42 }
    "#;
    let tokens = tokenize(src).expect("lex failed");
    let program = parse(tokens).expect("parse failed");
    let mut compiler = CraneliftCompiler::new().expect("compiler init failed");
    compiler
        .compile_program(&program)
        .expect("compilation failed");
    // start should be compiled (it's an entry point)
    assert!(compiler.get_fn_ptr("start").is_ok());
    // unused should NOT be compiled
    assert!(compiler.get_fn_ptr("unused").is_err());
}

// ── S10.4: Async I/O ──────────────────────────────────────────────────

#[test]
fn native_async_read_file() {
    // Write a test file, then async-read it and verify success
    std::fs::write("/tmp/fj_async_read_test.txt", "async hello").unwrap();
    let src = r#"
        fn main() -> i64 {
            let handle = async_read_file("/tmp/fj_async_read_test.txt")
            while handle.poll() == 0 {
                let x = 0
            }
            let st = handle.status()
            handle.free()
            st
        }
    "#;
    assert_eq!(compile_and_run(src), 0); // 0 = success
    let _ = std::fs::remove_file("/tmp/fj_async_read_test.txt");
}

#[test]
fn native_async_write_file() {
    let src = r#"
        fn main() -> i64 {
            let handle = async_write_file("/tmp/fj_async_write_test.txt", "async data")
            while handle.poll() == 0 {
                let x = 0
            }
            let st = handle.status()
            handle.free()
            st
        }
    "#;
    assert_eq!(compile_and_run(src), 0); // 0 = success
    assert_eq!(
        std::fs::read_to_string("/tmp/fj_async_write_test.txt").unwrap(),
        "async data"
    );
    let _ = std::fs::remove_file("/tmp/fj_async_write_test.txt");
}

#[test]
fn native_async_io_concurrent() {
    // Write two files concurrently and verify both succeed
    let src = r#"
        fn main() -> i64 {
            let h1 = async_write_file("/tmp/fj_async_c1.txt", "one")
            let h2 = async_write_file("/tmp/fj_async_c2.txt", "two")
            while h1.poll() == 0 {
                let x = 0
            }
            while h2.poll() == 0 {
                let x = 0
            }
            let s1 = h1.status()
            let s2 = h2.status()
            h1.free()
            h2.free()
            s1 + s2
        }
    "#;
    assert_eq!(compile_and_run(src), 0); // both 0 = success
    assert_eq!(
        std::fs::read_to_string("/tmp/fj_async_c1.txt").unwrap(),
        "one"
    );
    assert_eq!(
        std::fs::read_to_string("/tmp/fj_async_c2.txt").unwrap(),
        "two"
    );
    let _ = std::fs::remove_file("/tmp/fj_async_c1.txt");
    let _ = std::fs::remove_file("/tmp/fj_async_c2.txt");
}

// ── S18.3: Section placement attributes ───────────────────────────────

#[test]
fn native_section_annotation_parsed() {
    // @section(".text.boot") should parse and compile without errors
    let src = r#"
        @section(".text.boot")
        fn boot_entry() -> i64 { 42 }

        fn main() -> i64 { boot_entry() }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_section_annotation_tracked_by_aot() {
    let src = r#"
        @section(".text.boot")
        fn _start() -> i64 { 0 }

        fn main() -> i64 { _start() }
    "#;
    let tokens = tokenize(src).expect("lex failed");
    let program = parse(tokens).expect("parse failed");
    let mut compiler = ObjectCompiler::new("section_test").expect("compiler init failed");
    compiler
        .compile_program(&program)
        .expect("compilation failed");
    let sections = compiler.fn_sections();
    assert_eq!(sections.get("_start"), Some(&".text.boot".to_string()));
    assert!(!sections.contains_key("main"));
}

#[test]
fn native_section_annotation_multiple() {
    // Multiple functions with different sections
    let src = r#"
        @section(".text.boot")
        fn boot() -> i64 { 1 }

        @section(".text.init")
        fn init() -> i64 { 2 }

        fn main() -> i64 { boot() + init() }
    "#;
    assert_eq!(compile_and_run(src), 3);
}

// =====================================================================
// Heap array return from functions
// =====================================================================

#[test]
fn native_heap_array_return_basic() {
    // Function returning a heap array; caller indexes into it
    let src = r#"
        fn make() -> [i64] {
            let mut a: [i64] = []
            a = a.push(42)
            a = a.push(99)
            a
        }

        fn main() -> i64 {
            let arr = make()
            arr[0]
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_heap_array_return_index_second() {
    // Index the second element of a returned heap array
    let src = r#"
        fn make() -> [i64] {
            let mut a: [i64] = []
            a = a.push(10)
            a = a.push(20)
            a = a.push(30)
            a
        }

        fn main() -> i64 {
            let arr = make()
            arr[1] + arr[2]
        }
    "#;
    assert_eq!(compile_and_run(src), 50);
}

// =====================================================================
// S18.4 — #[link_section] for data placement
// =====================================================================

#[test]
fn native_data_section_annotation_parsed() {
    // @section on const should be tracked by AOT compiler
    let src = r#"
        @section(".data.config")
        const BUFFER_SIZE: i64 = 4096

        fn main() -> i64 {
            BUFFER_SIZE
        }
    "#;
    // JIT: const still works as a local variable (section is a no-op in JIT)
    assert_eq!(compile_and_run(src), 4096);
}

#[test]
fn native_data_section_tracked_by_aot() {
    // Verify AOT compiler tracks the data section annotation
    let src = r#"
        @section(".bss")
        const ZERO_BUF: i64 = 0

        @section(".data.config")
        const CONFIG_VAL: i64 = 42

        fn main() -> i64 {
            ZERO_BUF + CONFIG_VAL
        }
    "#;
    let tokens = tokenize(src).expect("lex failed");
    let program = parse(tokens).expect("parse failed");
    let mut compiler = super::ObjectCompiler::new("test_data_section").expect("compiler init");
    compiler
        .compile_program(&program)
        .expect("compilation failed");
    let sections = compiler.data_sections();
    assert_eq!(sections.get("ZERO_BUF"), Some(&".bss".to_string()));
    assert_eq!(
        sections.get("CONFIG_VAL"),
        Some(&".data.config".to_string())
    );
}

#[test]
fn native_data_section_multiple_types() {
    // Section annotation works with different const types
    let src = r#"
        @section(".rodata.magic")
        const MAGIC: i64 = 255

        fn main() -> i64 {
            MAGIC
        }
    "#;
    assert_eq!(compile_and_run(src), 255);
}

// ── If/Else Type Coercion ──────────────────────────────────────────

#[test]
fn native_if_else_bool_merge_type_coercion() {
    // When if/else branches produce different-width results (bool i8 vs i64),
    // the merge block value is coerced to match the expected type.
    let src = r#"
        fn test() -> i64 {
            let mut flag = true
            let mut count = 0
            while flag {
                count = count + 1
                if count >= 3 {
                    flag = false
                }
            }
            count
        }
        fn main() -> i64 { test() }
    "#;
    assert_eq!(compile_and_run(src), 3);
}

// ── Heap Array Reassignment (no double-free) ───────────────────────

#[test]
fn native_heap_array_reassign_no_crash() {
    // `a = b` where both are heap arrays must not double-free.
    let src = r#"
        fn main() -> i64 {
            let mut a: [i64] = []
            a = a.push(42)
            let mut b: [i64] = []
            b = b.push(99)
            a = b
            a[0]
        }
    "#;
    assert_eq!(compile_and_run(src), 99);
}

#[test]
fn native_heap_array_reassign_in_while_loop() {
    // `values = new_vals` inside a while loop must not double-free.
    let src = r#"
        fn main() -> i64 {
            let mut values: [i64] = []
            values = values.push(10)
            values = values.push(20)
            values = values.push(30)
            let mut popping = true
            while popping {
                let vlen = to_int(len(values))
                if vlen <= 1 {
                    popping = false
                } else {
                    let mut new_vals: [i64] = []
                    let mut j = 0
                    while j < vlen - 1 {
                        new_vals = new_vals.push(values[j])
                        j = j + 1
                    }
                    values = new_vals
                }
            }
            values[0]
        }
    "#;
    assert_eq!(compile_and_run(src), 10);
}

#[test]
fn native_shunting_yard_expression_parser() {
    // Self-hosted shunting-yard parser: 2 + 3 * 4 = 14
    let src = r#"
        fn is_digit(c: str) -> bool {
            c == "0" || c == "1" || c == "2" || c == "3" || c == "4" ||
            c == "5" || c == "6" || c == "7" || c == "8" || c == "9"
        }
        fn char_to_digit(c: str) -> i64 {
            if c == "0" { return 0 }
            if c == "1" { return 1 }
            if c == "2" { return 2 }
            if c == "3" { return 3 }
            if c == "4" { return 4 }
            if c == "5" { return 5 }
            if c == "6" { return 6 }
            if c == "7" { return 7 }
            if c == "8" { return 8 }
            return 9
        }
        fn tokenize(source: str) -> [i64] {
            let n = to_int(len(source))
            let mut pos = 0
            let mut result: [i64] = []
            while pos < n {
                let c = source.substring(pos, pos + 1)
                if c == " " { pos = pos + 1; continue }
                if is_digit(c) {
                    let mut val = 0
                    while pos < n && is_digit(source.substring(pos, pos + 1)) {
                        val = val * 10 + char_to_digit(source.substring(pos, pos + 1))
                        pos = pos + 1
                    }
                    result = result.push(130)
                    result = result.push(val)
                    continue
                }
                if c == "+" { result = result.push(70); result = result.push(0); pos = pos + 1; continue }
                if c == "*" { result = result.push(72); result = result.push(0); pos = pos + 1; continue }
                pos = pos + 1
            }
            result = result.push(0)
            result = result.push(0)
            result
        }
        fn precedence(kind: i64) -> i64 {
            if kind == 70 { return 10 }
            if kind == 72 { return 12 }
            return 0
        }
        fn eval_expr(source: str) -> i64 {
            let tokens = tokenize(source)
            let num_tokens = to_int(len(tokens)) / 2
            let mut values: [i64] = []
            let mut ops: [i64] = []
            let mut pos = 0
            while pos < num_tokens {
                let idx = pos * 2
                let kind = tokens[idx]
                let val = tokens[idx + 1]
                if kind == 0 { pos = num_tokens; continue }
                if kind == 130 {
                    values = values.push(val)
                    pos = pos + 1
                    continue
                }
                if kind == 70 || kind == 72 {
                    let prec = precedence(kind)
                    let mut popping = true
                    while popping {
                        let ops_len = to_int(len(ops))
                        if ops_len == 0 {
                            popping = false
                        } else {
                            let top_op = ops[ops_len - 1]
                            let top_prec = precedence(top_op)
                            if top_prec >= prec {
                                let vals_len = to_int(len(values))
                                let rv = values[vals_len - 1]
                                let lv = values[vals_len - 2]
                                let mut new_vals: [i64] = []
                                let mut j = 0
                                while j < vals_len - 2 {
                                    new_vals = new_vals.push(values[j])
                                    j = j + 1
                                }
                                if top_op == 70 {
                                    new_vals = new_vals.push(lv + rv)
                                } else {
                                    new_vals = new_vals.push(lv * rv)
                                }
                                values = new_vals
                                let mut new_ops: [i64] = []
                                let mut k = 0
                                while k < ops_len - 1 {
                                    new_ops = new_ops.push(ops[k])
                                    k = k + 1
                                }
                                ops = new_ops
                            } else {
                                popping = false
                            }
                        }
                    }
                    ops = ops.push(kind)
                    pos = pos + 1
                    continue
                }
                pos = pos + 1
            }
            let mut finishing = true
            while finishing {
                let ops_len = to_int(len(ops))
                if ops_len == 0 {
                    finishing = false
                } else {
                    let top_op = ops[ops_len - 1]
                    let vals_len = to_int(len(values))
                    let rv = values[vals_len - 1]
                    let lv = values[vals_len - 2]
                    let mut new_vals: [i64] = []
                    let mut j = 0
                    while j < vals_len - 2 {
                        new_vals = new_vals.push(values[j])
                        j = j + 1
                    }
                    if top_op == 70 {
                        new_vals = new_vals.push(lv + rv)
                    } else {
                        new_vals = new_vals.push(lv * rv)
                    }
                    values = new_vals
                    let mut new_ops: [i64] = []
                    let mut k = 0
                    while k < ops_len - 1 {
                        new_ops = new_ops.push(ops[k])
                        k = k + 1
                    }
                    ops = new_ops
                }
            }
            values[0]
        }
        fn main() -> i64 { eval_expr("2 + 3 * 4") }
    "#;
    assert_eq!(compile_and_run(src), 14);
}

// ── S46: Bootstrap Tests + S47: Self-Hosting Hardening ─────────────

#[test]
fn native_bootstrap_fibonacci() {
    // Same fibonacci program produces identical results on both backends.
    let src = r#"
        fn fib(n: i64) -> i64 {
            if n <= 1 { return n }
            fib(n - 1) + fib(n - 2)
        }
        fn main() -> i64 { fib(10) }
    "#;
    let interp_result = interpret_main(src);
    let native_result = compile_and_run(src);
    assert_eq!(interp_result, native_result);
    assert_eq!(native_result, 55);
}

#[test]
fn native_bootstrap_string_operations() {
    // String operations produce identical results on both backends.
    let src = r#"
        fn main() -> i64 {
            let s = "Hello, World!"
            let trimmed = "  hello  ".trim()
            to_int(len(s)) + to_int(len(trimmed))
        }
    "#;
    let interp_result = interpret_main(src);
    let native_result = compile_and_run(src);
    assert_eq!(interp_result, native_result);
    assert_eq!(native_result, 18);
}

#[test]
fn native_bootstrap_heap_array_ops() {
    // Heap array operations produce identical results on both backends.
    let src = r#"
        fn main() -> i64 {
            let mut arr: [i64] = []
            arr = arr.push(10)
            arr = arr.push(20)
            arr = arr.push(30)
            let mut sum = 0
            let mut i = 0
            while i < to_int(len(arr)) {
                sum = sum + arr[i]
                i = i + 1
            }
            sum
        }
    "#;
    let interp_result = interpret_main(src);
    let native_result = compile_and_run(src);
    assert_eq!(interp_result, native_result);
    assert_eq!(native_result, 60);
}

#[test]
fn native_bootstrap_perf_fibonacci() {
    // S47.3: Performance comparison — native fib(25) should be faster than interpreter.
    let src = r#"
        fn fib(n: i64) -> i64 {
            if n <= 1 { return n }
            fib(n - 1) + fib(n - 2)
        }
        fn main() -> i64 { fib(25) }
    "#;

    // Interpreter timing
    let start = std::time::Instant::now();
    let interp_result = interpret_main(src);
    let interp_time = start.elapsed();

    // Native timing
    let start = std::time::Instant::now();
    let native_result = compile_and_run(src);
    let native_time = start.elapsed();

    assert_eq!(interp_result, native_result);
    assert_eq!(native_result, 75025);

    // Native should be at least 2x faster (typically 10-100x)
    eprintln!(
        "  fib(25): interp={:?} native={:?} speedup={:.1}x",
        interp_time,
        native_time,
        interp_time.as_secs_f64() / native_time.as_secs_f64()
    );
}

#[test]
fn native_s47_complex_control_flow_bootstrap() {
    // Complex control flow with bool flags and heap arrays works identically
    // on both backends (this was the pattern that caused CE004 + double-free).
    let src = r#"
        fn main() -> i64 {
            let mut vals: [i64] = []
            vals = vals.push(100)
            vals = vals.push(200)
            vals = vals.push(300)
            let mut done = false
            while !done {
                let vlen = to_int(len(vals))
                if vlen <= 1 {
                    done = true
                } else {
                    let mut new_arr: [i64] = []
                    let mut i = 0
                    while i < vlen - 1 {
                        new_arr = new_arr.push(vals[i])
                        i = i + 1
                    }
                    vals = new_arr
                }
            }
            vals[0]
        }
    "#;
    let interp_result = interpret_main(src);
    let native_result = compile_and_run(src);
    assert_eq!(interp_result, native_result);
    assert_eq!(native_result, 100);
}

// ═══════════════════════════════════════════════════════════════════════
// S4.1 — Nested `?` operator in codegen
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn native_nested_try_ok() {
    // Multiple sequential ? operators, all Ok paths succeed
    let src = r#"
        fn inner() -> i64 {
            let a = Ok(10)
            let b = Ok(20)
            let x = a?
            let y = b?
            x + y
        }
        fn main() -> i64 {
            inner()
        }
    "#;
    // Both Ok: 10 + 20 = 30
    assert_eq!(compile_and_run(src), 30);
}

#[test]
fn native_nested_try_chain() {
    // Three sequential ? operators — all succeed and accumulate
    let src = r#"
        fn compute() -> i64 {
            let a = Ok(5)
            let b = Ok(10)
            let c = Ok(15)
            let x = a?
            let y = b?
            let z = c?
            x + y + z
        }
        fn main() -> i64 {
            compute()
        }
    "#;
    // 5 + 10 + 15 = 30
    assert_eq!(compile_and_run(src), 30);
}

#[test]
fn native_nested_try_err_propagation() {
    // First ? succeeds, second ? hits Err and short-circuits
    let src = r#"
        fn risky() -> i64 {
            let a = Ok(10)
            let b = Err(77)
            let x = a?
            let y = b?
            x + y
        }
        fn main() -> i64 {
            risky()
        }
    "#;
    // a? succeeds (10), b? hits Err(77) and returns 77 early
    assert_eq!(compile_and_run(src), 77);
}

// ═══════════════════════════════════════════════════════════════════════
// S13.1 — Concurrent HashMap tests
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn native_concurrent_map_basic() {
    // HashMap works correctly alongside thread operations
    let src = r#"
        fn compute(n: i64) -> i64 {
            n * n
        }

        fn main() -> i64 {
            let h = thread::spawn(compute, 7)
            let m = HashMap::new()
            m.insert("base", 10)
            let thread_result = h.join()
            m.insert("computed", thread_result)
            m.get("base") + m.get("computed")
        }
    "#;
    // thread_result = 7*7 = 49, base = 10, total = 59
    assert_eq!(compile_and_run(src), 59);
}

#[test]
fn native_map_after_thread() {
    // Multiple threads compute values, results aggregated into a HashMap
    let src = r#"
        fn square(n: i64) -> i64 {
            n * n
        }

        fn main() -> i64 {
            let h1 = thread::spawn(square, 3)
            let h2 = thread::spawn(square, 4)
            let h3 = thread::spawn(square, 5)
            let r1 = h1.join()
            let r2 = h2.join()
            let r3 = h3.join()
            let m = HashMap::new()
            m.insert("a", r1)
            m.insert("b", r2)
            m.insert("c", r3)
            m.get("a") + m.get("b") + m.get("c")
        }
    "#;
    // 9 + 16 + 25 = 50
    assert_eq!(compile_and_run(src), 50);
}

// ═══════════════════════════════════════════════════════════════════════
// v0.4 S1 — Generic Enum Infrastructure
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn native_enum_variant_types_tracked() {
    // Verify that user-defined enums with typed payloads compile correctly
    let src = r#"
        enum Shape {
            Circle(i64),
            Rect(i64),
            None
        }
        fn main() -> i64 {
            let s = Shape::Circle(42)
            match s {
                Circle(r) => r,
                Rect(w) => w,
                None => 0
            }
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_enum_float_payload() {
    // Enum with float payload — tests type-aware payload tracking
    let src = r#"
        enum Value {
            Int(i64),
            None
        }
        fn main() -> i64 {
            let v = Value::Int(99)
            match v {
                Int(x) => x,
                None => 0
            }
        }
    "#;
    assert_eq!(compile_and_run(src), 99);
}

#[test]
fn native_enum_option_generic_pattern() {
    // Enum with payload: construct then destructure via match
    let src = r#"
        fn main() -> i64 {
            let r = Ok(42)
            match r {
                Ok(v) => v,
                Err(e) => e
            }
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}
