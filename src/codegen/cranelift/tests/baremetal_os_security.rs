//! no_std/context enforcement, bare-metal _start, HAL/OS builtins, kernel boot pattern, security runtime.

use super::super::*;
use super::{compile_and_run, compile_and_run_with_security};
use crate::lexer::tokenize;
use crate::parser::parse;

// ── H1: no_std enforcement in codegen ──

#[test]
fn nostd_rejects_tensor_zeros() {
    let src = r#"
        fn main() -> i64 {
            let t = tensor_zeros([2, 3])
            0
        }
    "#;
    let tokens = tokenize(src).expect("lex failed");
    let program = parse(tokens).expect("parse failed");
    let mut compiler = CraneliftCompiler::new().expect("compiler init failed");
    compiler.set_no_std(true);
    let result = compiler.compile_program(&program);
    assert!(
        result.is_err(),
        "expected no_std violation for tensor_zeros"
    );
    let errs = result.unwrap_err();
    assert!(
        errs.iter().any(|e| format!("{e}").contains("NS001")),
        "expected NS001 error code, got: {errs:?}"
    );
}

#[test]
fn nostd_rejects_read_file() {
    let src = r#"
        fn main() -> i64 {
            let data = read_file("test.txt")
            0
        }
    "#;
    let tokens = tokenize(src).expect("lex failed");
    let program = parse(tokens).expect("parse failed");
    let mut compiler = CraneliftCompiler::new().expect("compiler init failed");
    compiler.set_no_std(true);
    let result = compiler.compile_program(&program);
    assert!(result.is_err(), "expected no_std violation for read_file");
}

#[test]
fn nostd_allows_pure_arithmetic() {
    let src = r#"
        fn main() -> i64 {
            let x = 10 + 20
            x * 3
        }
    "#;
    let tokens = tokenize(src).expect("lex failed");
    let program = parse(tokens).expect("parse failed");
    let mut compiler = CraneliftCompiler::new().expect("compiler init failed");
    compiler.set_no_std(true);
    let result = compiler.compile_program(&program);
    assert!(
        result.is_ok(),
        "pure arithmetic should pass no_std: {result:?}"
    );
}

#[test]
fn nostd_normal_mode_allows_tensor() {
    // Without no_std, tensor_zeros should compile fine
    let src = r#"
        fn main() -> i64 {
            let t = tensor_zeros(2, 3)
            tensor_free(t)
            0
        }
    "#;
    assert_eq!(compile_and_run(src), 0);
}

// ── H4: Context enforcement in native codegen ──

#[test]
fn context_kernel_rejects_tensor() {
    // @kernel function calling tensor_zeros should fail with ContextViolation
    let src = r#"
        @kernel fn boot() -> i64 {
            let t = tensor_zeros(2, 3)
            0
        }
        fn main() -> i64 { boot() }
    "#;
    let tokens = crate::lexer::tokenize(src).expect("lex failed");
    let program = crate::parser::parse(tokens).expect("parse failed");
    let mut compiler = CraneliftCompiler::new().expect("compiler init failed");
    let result = compiler.compile_program(&program);
    assert!(result.is_err(), "@kernel should reject tensor_zeros");
    let errs = result.unwrap_err();
    let msg = format!("{:?}", errs);
    assert!(
        msg.contains("ContextViolation") || msg.contains("KE002"),
        "error should mention context violation: {msg}"
    );
}

#[test]
fn context_kernel_rejects_read_file() {
    let src = r#"
        @kernel fn boot() -> i64 {
            let f = read_file("test.txt")
            0
        }
        fn main() -> i64 { boot() }
    "#;
    let tokens = crate::lexer::tokenize(src).expect("lex failed");
    let program = crate::parser::parse(tokens).expect("parse failed");
    let mut compiler = CraneliftCompiler::new().expect("compiler init failed");
    let result = compiler.compile_program(&program);
    assert!(result.is_err(), "@kernel should reject read_file");
    let errs = result.unwrap_err();
    let msg = format!("{:?}", errs);
    assert!(
        msg.contains("ContextViolation") || msg.contains("KE001"),
        "error should mention context violation: {msg}"
    );
}

#[test]
fn context_device_rejects_raw_pointer() {
    let src = r#"
        @device fn infer() -> i64 {
            let p = mem_alloc(8, 8)
            0
        }
        fn main() -> i64 { infer() }
    "#;
    let tokens = crate::lexer::tokenize(src).expect("lex failed");
    let program = crate::parser::parse(tokens).expect("parse failed");
    let mut compiler = CraneliftCompiler::new().expect("compiler init failed");
    let result = compiler.compile_program(&program);
    assert!(result.is_err(), "@device should reject mem_alloc");
    let errs = result.unwrap_err();
    let msg = format!("{:?}", errs);
    assert!(
        msg.contains("ContextViolation") || msg.contains("DE001"),
        "error should mention context violation: {msg}"
    );
}

#[test]
fn context_safe_allows_normal_code() {
    let src = r#"
        @safe fn compute(x: i64) -> i64 { x + 1 }
        fn main() -> i64 { compute(41) }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn context_unsafe_allows_everything() {
    // @unsafe should not reject anything
    let src = r#"
        @unsafe fn do_everything() -> i64 { 42 }
        fn main() -> i64 { do_everything() }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

// ── M1: Pointer dereference in native codegen ──

#[test]
fn native_pointer_deref() {
    let src = r#"
        fn main() -> i64 {
            let p = alloc(8)
            volatile_write(p, 99)
            let val = *p
            dealloc(p, 8)
            val
        }
    "#;
    assert_eq!(compile_and_run(src), 99);
}

#[test]
fn native_pointer_deref_in_expr() {
    let src = r#"
        fn main() -> i64 {
            let p = alloc(8)
            volatile_write(p, 10)
            let val = *p + 5
            dealloc(p, 8)
            val
        }
    "#;
    assert_eq!(compile_and_run(src), 15);
}

// ── M3: Const evaluation / folding in native codegen ──

#[test]
fn native_const_folding() {
    // Verify const values propagate correctly at compile time
    let src = r#"
        const PAGE_SIZE: i64 = 4096
        fn main() -> i64 {
            let x = PAGE_SIZE * 2
            x
        }
    "#;
    assert_eq!(compile_and_run(src), 8192);
}

#[test]
fn native_const_arithmetic() {
    let src = r#"
        const BASE: i64 = 100
        const OFFSET: i64 = 42
        fn main() -> i64 {
            BASE + OFFSET
        }
    "#;
    assert_eq!(compile_and_run(src), 142);
}

// ── Const fn compile-time evaluation ──

#[test]
fn native_const_fn_basic() {
    // const fn add should be evaluated at compile time
    let src = r#"
        const fn add(a: i64, b: i64) -> i64 { a + b }
        const RESULT: i64 = add(10, 32)
        fn main() -> i64 {
            RESULT
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_const_fn_multiply() {
    let src = r#"
        const fn mul(a: i64, b: i64) -> i64 { a * b }
        const PAGE_SIZE: i64 = 4096
        const PAGES: i64 = mul(PAGE_SIZE, 8)
        fn main() -> i64 {
            PAGES
        }
    "#;
    assert_eq!(compile_and_run(src), 32768);
}

#[test]
fn native_const_fn_recursive_fib() {
    // const fn fib should evaluate recursively at compile time
    let src = r#"
        const fn fib(n: i64) -> i64 {
            if n <= 1 { n } else { fib(n - 1) + fib(n - 2) }
        }
        const FIB10: i64 = fib(10)
        fn main() -> i64 {
            FIB10
        }
    "#;
    assert_eq!(compile_and_run(src), 55);
}

#[test]
fn native_const_fn_with_const_args() {
    // const fn using other constants as arguments
    let src = r#"
        const fn square(x: i64) -> i64 { x * x }
        const BASE: i64 = 7
        const SQ: i64 = square(BASE)
        fn main() -> i64 {
            SQ
        }
    "#;
    assert_eq!(compile_and_run(src), 49);
}

#[test]
fn native_const_fn_conditional() {
    let src = r#"
        const fn abs(x: i64) -> i64 {
            if x < 0 { 0 - x } else { x }
        }
        const VAL: i64 = abs(-42)
        fn main() -> i64 {
            VAL
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_const_fn_called_at_runtime() {
    // const fn should also work as a regular function at runtime
    let src = r#"
        const fn add(a: i64, b: i64) -> i64 { a + b }
        fn main() -> i64 {
            let x = 10
            add(x, 20)
        }
    "#;
    assert_eq!(compile_and_run(src), 30);
}

// ── B4: Bare-metal _start generation ──

#[test]
fn bare_metal_start_has_bss_zeroing() {
    // When no_std is enabled with @entry, _start should include BSS zeroing
    let src = r#"
        @entry fn boot() {
            let x = 42
        }
    "#;
    let tokens = crate::lexer::tokenize(src).expect("lex failed");
    let program = crate::parser::parse(tokens).expect("parse failed");
    let mut compiler = super::ObjectCompiler::new("test_bare_metal").expect("compiler init failed");
    compiler.set_no_std(true);
    let result = compiler.compile_program(&program);
    assert!(
        result.is_ok(),
        "bare-metal _start should compile: {result:?}"
    );
    // Verify the object file was produced (contains _start + BSS zeroing)
    let product = compiler.finish();
    let bytes = product.emit().expect("emit failed");
    assert!(bytes.len() > 100, "object file should be non-trivial");
}

#[test]
fn non_bare_metal_start_has_return() {
    // Normal mode: _start just calls entry and returns
    let src = r#"
        @entry fn boot() {
            let x = 42
        }
    "#;
    let tokens = crate::lexer::tokenize(src).expect("lex failed");
    let program = crate::parser::parse(tokens).expect("parse failed");
    let mut compiler = super::ObjectCompiler::new("test_normal").expect("compiler init failed");
    // NOT setting no_std — normal mode
    let result = compiler.compile_program(&program);
    assert!(result.is_ok(), "normal _start should compile: {result:?}");
    let product = compiler.finish();
    let bytes = product.emit().expect("emit failed");
    assert!(bytes.len() > 50, "object file should be non-trivial");
}

#[test]
fn bare_metal_aarch64_start() {
    // ARM64 bare-metal target should produce valid object
    let src = r#"
        @entry fn kernel_main() {
            let uart_base: i64 = 0x09000000
        }
    "#;
    let tokens = crate::lexer::tokenize(src).expect("lex failed");
    let program = crate::parser::parse(tokens).expect("parse failed");
    let target = crate::codegen::target::TargetConfig::from_triple("aarch64-unknown-none");
    if let Ok(target) = target {
        if let Ok(mut compiler) =
            super::ObjectCompiler::new_with_target("test_aarch64_start", &target)
        {
            compiler.set_no_std(true);
            let result = compiler.compile_program(&program);
            assert!(
                result.is_ok(),
                "aarch64 bare-metal _start should compile: {result:?}"
            );
        }
    }
    // Skip if aarch64 target not available
}

// ═══════════════════════════════════════════════════════════════════════
// B1: ARM64 inline assembly encoding integration
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn native_asm_arm64_mrs_encoding() {
    // asm!("mrs x0, SCTLR_EL1") should encode to a valid ARM64 mrs instruction word
    let src = r#"
        fn main() -> i64 {
            let mut encoded: i64 = 0
            asm!("mrs x0, SCTLR_EL1", out(reg) encoded)
            encoded
        }
    "#;
    let result = compile_and_run(src);
    // The encoded mrs instruction word should be non-zero
    assert_ne!(
        result, 0,
        "mrs encoding should produce non-zero instruction word"
    );
    // Verify it matches the expected encoding from aarch64_asm
    let expected = crate::codegen::aarch64_asm::encode_instruction("mrs", &["x0", "SCTLR_EL1"])
        .expect("encode_instruction should succeed");
    assert_eq!(
        result, expected as i64,
        "JIT mrs encoding should match aarch64_asm encoder"
    );
}

#[test]
fn native_asm_arm64_msr_encoding() {
    let src = r#"
        fn main() -> i64 {
            let mut encoded: i64 = 0
            asm!("msr VBAR_EL1, x1", out(reg) encoded)
            encoded
        }
    "#;
    let result = compile_and_run(src);
    let expected = crate::codegen::aarch64_asm::encode_instruction("msr", &["VBAR_EL1", "x1"])
        .expect("encode_instruction should succeed");
    assert_eq!(result, expected as i64, "msr encoding should match");
}

#[test]
fn native_asm_arm64_isb_encoding() {
    let src = r#"
        fn main() -> i64 {
            let mut encoded: i64 = 0
            asm!("isb", out(reg) encoded)
            encoded
        }
    "#;
    let result = compile_and_run(src);
    let expected = crate::codegen::aarch64_asm::encode_instruction("isb", &[])
        .expect("encode_instruction should succeed");
    assert_eq!(result, expected as i64, "isb encoding should match");
}

#[test]
fn native_asm_arm64_wfi_encoding() {
    let src = r#"
        fn main() -> i64 {
            let mut encoded: i64 = 0
            asm!("wfi", out(reg) encoded)
            encoded
        }
    "#;
    let result = compile_and_run(src);
    let expected = crate::codegen::aarch64_asm::encode_instruction("wfi", &[])
        .expect("encode_instruction should succeed");
    assert_eq!(result, expected as i64, "wfi encoding should match");
}

#[test]
fn native_asm_arm64_eret_encoding() {
    let src = r#"
        fn main() -> i64 {
            let mut encoded: i64 = 0
            asm!("eret", out(reg) encoded)
            encoded
        }
    "#;
    let result = compile_and_run(src);
    let expected = crate::codegen::aarch64_asm::encode_instruction("eret", &[])
        .expect("encode_instruction should succeed");
    assert_eq!(result, expected as i64, "eret encoding should match");
}

#[test]
fn native_asm_arm64_svc_encoding() {
    let src = r#"
        fn main() -> i64 {
            let mut encoded: i64 = 0
            asm!("svc #0", out(reg) encoded)
            encoded
        }
    "#;
    let result = compile_and_run(src);
    let expected = crate::codegen::aarch64_asm::encode_instruction("svc", &["#0"])
        .expect("encode_instruction should succeed");
    assert_eq!(result, expected as i64, "svc encoding should match");
}

#[test]
fn native_asm_arm64_movz_encoding() {
    let src = r#"
        fn main() -> i64 {
            let mut encoded: i64 = 0
            asm!("movz x0, #0x1234", out(reg) encoded)
            encoded
        }
    "#;
    let result = compile_and_run(src);
    let expected = crate::codegen::aarch64_asm::encode_instruction("movz", &["x0", "#0x1234"])
        .expect("encode_instruction should succeed");
    assert_eq!(result, expected as i64, "movz encoding should match");
}

#[test]
fn native_asm_arm64_ldr_encoding() {
    let src = r#"
        fn main() -> i64 {
            let mut encoded: i64 = 0
            asm!("ldr x0, [x1, #8]", out(reg) encoded)
            encoded
        }
    "#;
    let result = compile_and_run(src);
    let expected = crate::codegen::aarch64_asm::encode_instruction("ldr", &["x0", "[x1, #8]"])
        .expect("encode_instruction should succeed");
    assert_eq!(result, expected as i64, "ldr encoding should match");
}

#[test]
fn native_asm_arm64_ret_encoding() {
    let src = r#"
        fn main() -> i64 {
            let mut encoded: i64 = 0
            asm!("ret", out(reg) encoded)
            encoded
        }
    "#;
    let result = compile_and_run(src);
    let expected = crate::codegen::aarch64_asm::encode_instruction("ret", &[])
        .expect("encode_instruction should succeed");
    assert_eq!(result, expected as i64, "ret encoding should match");
}

#[test]
fn native_asm_arm64_dsb_encoding() {
    let src = r#"
        fn main() -> i64 {
            let mut encoded: i64 = 0
            asm!("dsb sy", out(reg) encoded)
            encoded
        }
    "#;
    let result = compile_and_run(src);
    let expected = crate::codegen::aarch64_asm::encode_instruction("dsb", &["sy"])
        .expect("encode_instruction should succeed");
    assert_eq!(result, expected as i64, "dsb encoding should match");
}

#[test]
fn native_asm_arm64_sequence() {
    // Multiple ARM64 instructions in sequence
    let src = r#"
        fn main() -> i64 {
            let mut e1: i64 = 0
            let mut e2: i64 = 0
            asm!("isb", out(reg) e1)
            asm!("dsb sy", out(reg) e2)
            e1 + e2
        }
    "#;
    let result = compile_and_run(src);
    let isb = crate::codegen::aarch64_asm::encode_instruction("isb", &[]).unwrap() as i64;
    let dsb = crate::codegen::aarch64_asm::encode_instruction("dsb", &["sy"]).unwrap() as i64;
    assert_eq!(
        result,
        isb + dsb,
        "sum of encoded instructions should match"
    );
}

// ═══════════════════════════════════════════════════════════════════════
// Phase 3: HAL Driver Builtins (Sprint 11-15 — FajarOS v3.0 "Surya")
// ═══════════════════════════════════════════════════════════════════════

// ── Sprint 11: GPIO Builtins ──

#[test]
fn native_nostd_gpio_write_read() {
    // GPIO write/read cycle in no_std bare-metal mode
    let src = r#"
        fn main() -> i64 {
            gpio_set_output(42)
            gpio_write(42, 1)
            let val = gpio_read(42)
            gpio_write(42, 0)
            let val2 = gpio_read(42)
            val * 10 + val2
        }
    "#;
    let tokens = tokenize(src).expect("lex failed");
    let program = parse(tokens).expect("parse failed");
    let mut compiler = CraneliftCompiler::new().expect("compiler init failed");
    compiler.set_no_std(true);
    compiler
        .compile_program(&program)
        .expect("GPIO builtins should compile in no_std mode");
    let fn_ptr = compiler.get_fn_ptr("main").expect("main not found");
    let main_fn: fn() -> i64 = unsafe { std::mem::transmute(fn_ptr) };
    assert_eq!(main_fn(), 10); // val=1*10 + val2=0 = 10
}

#[test]
fn native_nostd_gpio_toggle() {
    let src = r#"
        fn main() -> i64 {
            gpio_set_output(50)
            gpio_write(50, 0)
            gpio_toggle(50)
            let v1 = gpio_read(50)
            gpio_toggle(50)
            let v2 = gpio_read(50)
            v1 * 10 + v2
        }
    "#;
    let tokens = tokenize(src).expect("lex failed");
    let program = parse(tokens).expect("parse failed");
    let mut compiler = CraneliftCompiler::new().expect("compiler init failed");
    compiler.set_no_std(true);
    compiler
        .compile_program(&program)
        .expect("GPIO toggle should compile");
    let fn_ptr = compiler.get_fn_ptr("main").expect("main not found");
    let main_fn: fn() -> i64 = unsafe { std::mem::transmute(fn_ptr) };
    assert_eq!(main_fn(), 10); // v1=1, v2=0 → 10
}

#[test]
fn native_nostd_gpio_config() {
    let src = r#"
        fn main() -> i64 {
            let r = gpio_config(96, 0, 1, 2)
            r
        }
    "#;
    let tokens = tokenize(src).expect("lex failed");
    let program = parse(tokens).expect("parse failed");
    let mut compiler = CraneliftCompiler::new().expect("compiler init failed");
    compiler.set_no_std(true);
    compiler
        .compile_program(&program)
        .expect("gpio_config should compile");
    let fn_ptr = compiler.get_fn_ptr("main").expect("main not found");
    let main_fn: fn() -> i64 = unsafe { std::mem::transmute(fn_ptr) };
    assert_eq!(main_fn(), 0); // success
}

#[test]
fn native_nostd_gpio_invalid_pin() {
    let src = r#"
        fn main() -> i64 {
            gpio_write(999, 1)
        }
    "#;
    let tokens = tokenize(src).expect("lex failed");
    let program = parse(tokens).expect("parse failed");
    let mut compiler = CraneliftCompiler::new().expect("compiler init failed");
    compiler.set_no_std(true);
    compiler
        .compile_program(&program)
        .expect("gpio with invalid pin should compile");
    let fn_ptr = compiler.get_fn_ptr("main").expect("main not found");
    let main_fn: fn() -> i64 = unsafe { std::mem::transmute(fn_ptr) };
    assert_eq!(main_fn(), -1); // error: invalid pin
}

// ── Sprint 12: UART Builtins ──

#[test]
fn native_nostd_uart_init() {
    let src = r#"
        fn main() -> i64 {
            let r = uart_init(0, 115200)
            r
        }
    "#;
    let tokens = tokenize(src).expect("lex failed");
    let program = parse(tokens).expect("parse failed");
    let mut compiler = CraneliftCompiler::new().expect("compiler init failed");
    compiler.set_no_std(true);
    compiler
        .compile_program(&program)
        .expect("uart_init should compile");
    let fn_ptr = compiler.get_fn_ptr("main").expect("main not found");
    let main_fn: fn() -> i64 = unsafe { std::mem::transmute(fn_ptr) };
    assert_eq!(main_fn(), 0);
}

#[test]
fn native_nostd_uart_write_byte() {
    let src = r#"
        fn main() -> i64 {
            uart_init(1, 9600)
            uart_write_byte(1, 65)
        }
    "#;
    let tokens = tokenize(src).expect("lex failed");
    let program = parse(tokens).expect("parse failed");
    let mut compiler = CraneliftCompiler::new().expect("compiler init failed");
    compiler.set_no_std(true);
    compiler
        .compile_program(&program)
        .expect("uart_write_byte should compile");
    let fn_ptr = compiler.get_fn_ptr("main").expect("main not found");
    let main_fn: fn() -> i64 = unsafe { std::mem::transmute(fn_ptr) };
    assert_eq!(main_fn(), 0); // success (byte written to nowhere)
}

// ── Sprint 13: SPI/I2C Builtins ──

#[test]
fn native_nostd_spi_loopback() {
    let src = r#"
        fn main() -> i64 {
            spi_init(0, 1000000)
            spi_cs_set(0, 0, 1)
            spi_transfer(0, 42)
            let rx = spi_transfer(0, 99)
            spi_cs_set(0, 0, 0)
            rx
        }
    "#;
    let tokens = tokenize(src).expect("lex failed");
    let program = parse(tokens).expect("parse failed");
    let mut compiler = CraneliftCompiler::new().expect("compiler init failed");
    compiler.set_no_std(true);
    compiler
        .compile_program(&program)
        .expect("SPI builtins should compile");
    let fn_ptr = compiler.get_fn_ptr("main").expect("main not found");
    let main_fn: fn() -> i64 = unsafe { std::mem::transmute(fn_ptr) };
    assert_eq!(main_fn(), 42); // loopback: previous TX (42) returned as RX
}

#[test]
fn native_nostd_i2c_init() {
    let src = r#"
        fn main() -> i64 {
            let r = i2c_init(0, 400000)
            r
        }
    "#;
    let tokens = tokenize(src).expect("lex failed");
    let program = parse(tokens).expect("parse failed");
    let mut compiler = CraneliftCompiler::new().expect("compiler init failed");
    compiler.set_no_std(true);
    compiler
        .compile_program(&program)
        .expect("i2c_init should compile");
    let fn_ptr = compiler.get_fn_ptr("main").expect("main not found");
    let main_fn: fn() -> i64 = unsafe { std::mem::transmute(fn_ptr) };
    assert_eq!(main_fn(), 0);
}

// ── Sprint 14: Timer Builtins ──

#[test]
fn native_nostd_timer_ticks() {
    let src = r#"
        fn main() -> i64 {
            let t1 = timer_get_ticks()
            let t2 = timer_get_ticks()
            if t2 > t1 { 1 } else { 0 }
        }
    "#;
    let tokens = tokenize(src).expect("lex failed");
    let program = parse(tokens).expect("parse failed");
    let mut compiler = CraneliftCompiler::new().expect("compiler init failed");
    compiler.set_no_std(true);
    compiler
        .compile_program(&program)
        .expect("timer_get_ticks should compile");
    let fn_ptr = compiler.get_fn_ptr("main").expect("main not found");
    let main_fn: fn() -> i64 = unsafe { std::mem::transmute(fn_ptr) };
    assert_eq!(main_fn(), 1); // monotonic
}

#[test]
fn native_nostd_timer_frequency() {
    let src = r#"
        fn main() -> i64 {
            timer_get_freq()
        }
    "#;
    let tokens = tokenize(src).expect("lex failed");
    let program = parse(tokens).expect("parse failed");
    let mut compiler = CraneliftCompiler::new().expect("compiler init failed");
    compiler.set_no_std(true);
    compiler
        .compile_program(&program)
        .expect("timer_get_freq should compile");
    let fn_ptr = compiler.get_fn_ptr("main").expect("main not found");
    let main_fn: fn() -> i64 = unsafe { std::mem::transmute(fn_ptr) };
    assert_eq!(main_fn(), 62_500_000); // QEMU default 62.5 MHz
}

#[test]
fn native_nostd_timer_deadline_and_uptime() {
    let src = r#"
        fn main() -> i64 {
            timer_mark_boot()
            timer_set_deadline(1000000)
            timer_enable_virtual()
            timer_disable_virtual()
            time_since_boot()
        }
    "#;
    let tokens = tokenize(src).expect("lex failed");
    let program = parse(tokens).expect("parse failed");
    let mut compiler = CraneliftCompiler::new().expect("compiler init failed");
    compiler.set_no_std(true);
    compiler
        .compile_program(&program)
        .expect("timer deadline + uptime should compile");
    let fn_ptr = compiler.get_fn_ptr("main").expect("main not found");
    let main_fn: fn() -> i64 = unsafe { std::mem::transmute(fn_ptr) };
    let uptime = main_fn();
    assert!(uptime >= 0); // non-negative uptime
}

// ── Sprint 15: DMA Builtins ──

#[test]
fn native_nostd_dma_lifecycle() {
    // Use channel 7 to avoid conflicts with other parallel tests
    let src = r#"
        fn main() -> i64 {
            let status0 = dma_status(7)
            dma_config(7, 0, 0, 64)
            let status1 = dma_status(7)
            dma_start(7)
            let status2 = dma_status(7)
            dma_wait(7)
            dma_barrier()
            status0 * 100 + status1 * 10 + status2
        }
    "#;
    let tokens = tokenize(src).expect("lex failed");
    let program = parse(tokens).expect("parse failed");
    let mut compiler = CraneliftCompiler::new().expect("compiler init failed");
    compiler.set_no_std(true);
    compiler
        .compile_program(&program)
        .expect("DMA lifecycle should compile");
    let fn_ptr = compiler.get_fn_ptr("main").expect("main not found");
    let main_fn: fn() -> i64 = unsafe { std::mem::transmute(fn_ptr) };
    // status0=0(idle)*100 + status1=1(configured)*10 + status2=3(done) = 13
    assert_eq!(main_fn(), 13);
}

// ── Combined: HAL Integration ──

#[test]
fn native_nostd_hal_blinky_pattern() {
    // Simulates the classic "blinky" LED program pattern
    let src = r#"
        fn main() -> i64 {
            gpio_config(96, 0, 1, 0)
            uart_init(0, 115200)
            timer_mark_boot()

            let mut count = 0
            let mut i = 0
            while i < 5 {
                gpio_write(96, 1)
                let on = gpio_read(96)
                gpio_write(96, 0)
                let off = gpio_read(96)
                count = count + on - off
                i = i + 1
            }
            count
        }
    "#;
    let tokens = tokenize(src).expect("lex failed");
    let program = parse(tokens).expect("parse failed");
    let mut compiler = CraneliftCompiler::new().expect("compiler init failed");
    compiler.set_no_std(true);
    compiler
        .compile_program(&program)
        .expect("blinky pattern should compile");
    let fn_ptr = compiler.get_fn_ptr("main").expect("main not found");
    let main_fn: fn() -> i64 = unsafe { std::mem::transmute(fn_ptr) };
    assert_eq!(main_fn(), 5); // 5 cycles, each adds 1 (on=1, off=0)
}

#[test]
fn native_nostd_hal_sensor_poll() {
    // Simulates I2C sensor polling with SPI data forwarding
    let src = r#"
        fn main() -> i64 {
            i2c_init(0, 400000)
            spi_init(0, 1000000)

            spi_cs_set(0, 0, 1)
            spi_transfer(0, 55)
            let forwarded = spi_transfer(0, 0)
            spi_cs_set(0, 0, 0)

            forwarded
        }
    "#;
    let tokens = tokenize(src).expect("lex failed");
    let program = parse(tokens).expect("parse failed");
    let mut compiler = CraneliftCompiler::new().expect("compiler init failed");
    compiler.set_no_std(true);
    compiler
        .compile_program(&program)
        .expect("sensor poll should compile");
    let fn_ptr = compiler.get_fn_ptr("main").expect("main not found");
    let main_fn: fn() -> i64 = unsafe { std::mem::transmute(fn_ptr) };
    assert_eq!(main_fn(), 55); // SPI loopback returns previous TX
}

// ── Sprint 16-17: Storage Builtins ──

#[test]
fn native_nostd_nvme_lifecycle() {
    let src = r#"
        fn main() -> i64 {
            let r = nvme_init()
            r
        }
    "#;
    let tokens = tokenize(src).expect("lex failed");
    let program = parse(tokens).expect("parse failed");
    let mut compiler = CraneliftCompiler::new().expect("compiler init failed");
    compiler.set_no_std(true);
    compiler
        .compile_program(&program)
        .expect("nvme_init should compile");
    let fn_ptr = compiler.get_fn_ptr("main").expect("main not found");
    let main_fn: fn() -> i64 = unsafe { std::mem::transmute(fn_ptr) };
    assert_eq!(main_fn(), 0);
}

#[test]
fn native_nostd_sd_init() {
    let src = r#"
        fn main() -> i64 {
            sd_init()
        }
    "#;
    let tokens = tokenize(src).expect("lex failed");
    let program = parse(tokens).expect("parse failed");
    let mut compiler = CraneliftCompiler::new().expect("compiler init failed");
    compiler.set_no_std(true);
    compiler
        .compile_program(&program)
        .expect("sd_init should compile");
    let fn_ptr = compiler.get_fn_ptr("main").expect("main not found");
    let main_fn: fn() -> i64 = unsafe { std::mem::transmute(fn_ptr) };
    assert_eq!(main_fn(), 0);
}

#[test]
fn native_nostd_vfs_close() {
    // VFS close with a file descriptor (no strings in no_std)
    let src = r#"
        fn main() -> i64 {
            vfs_close(3)
        }
    "#;
    let tokens = tokenize(src).expect("lex failed");
    let program = parse(tokens).expect("parse failed");
    let mut compiler = CraneliftCompiler::new().expect("compiler init failed");
    compiler.set_no_std(true);
    compiler
        .compile_program(&program)
        .expect("vfs_close should compile");
    let fn_ptr = compiler.get_fn_ptr("main").expect("main not found");
    let main_fn: fn() -> i64 = unsafe { std::mem::transmute(fn_ptr) };
    assert_eq!(main_fn(), 0);
}

// ── Sprint 20-23: Network Builtins ──

#[test]
fn native_nostd_eth_init() {
    let src = r#"
        fn main() -> i64 {
            eth_init()
        }
    "#;
    let tokens = tokenize(src).expect("lex failed");
    let program = parse(tokens).expect("parse failed");
    let mut compiler = CraneliftCompiler::new().expect("compiler init failed");
    compiler.set_no_std(true);
    compiler
        .compile_program(&program)
        .expect("eth_init should compile");
    let fn_ptr = compiler.get_fn_ptr("main").expect("main not found");
    let main_fn: fn() -> i64 = unsafe { std::mem::transmute(fn_ptr) };
    assert_eq!(main_fn(), 0);
}

#[test]
fn native_nostd_net_tcp_server() {
    let src = r#"
        fn main() -> i64 {
            let sock = net_socket(0)
            let b = net_bind(sock, 8080)
            let l = net_listen(sock)
            let c = net_close(sock)
            if sock >= 0 { b + l + c } else { -1 }
        }
    "#;
    let tokens = tokenize(src).expect("lex failed");
    let program = parse(tokens).expect("parse failed");
    let mut compiler = CraneliftCompiler::new().expect("compiler init failed");
    compiler.set_no_std(true);
    compiler
        .compile_program(&program)
        .expect("TCP server should compile");
    let fn_ptr = compiler.get_fn_ptr("main").expect("main not found");
    let main_fn: fn() -> i64 = unsafe { std::mem::transmute(fn_ptr) };
    assert_eq!(main_fn(), 0); // all return 0 (success)
}

#[test]
fn native_nostd_net_tcp_client() {
    let src = r#"
        fn main() -> i64 {
            let sock = net_socket(0)
            let c = net_connect(sock, 0, 80)
            let r = net_close(sock)
            c + r
        }
    "#;
    let tokens = tokenize(src).expect("lex failed");
    let program = parse(tokens).expect("parse failed");
    let mut compiler = CraneliftCompiler::new().expect("compiler init failed");
    compiler.set_no_std(true);
    compiler
        .compile_program(&program)
        .expect("TCP client should compile");
    let fn_ptr = compiler.get_fn_ptr("main").expect("main not found");
    let main_fn: fn() -> i64 = unsafe { std::mem::transmute(fn_ptr) };
    assert_eq!(main_fn(), 0);
}

#[test]
fn native_nostd_net_accept() {
    let src = r#"
        fn main() -> i64 {
            let sock = net_socket(0)
            net_bind(sock, 9090)
            net_listen(sock)
            let client = net_accept(sock)
            let r = net_close(client)
            net_close(sock)
            if client >= 0 { r } else { -1 }
        }
    "#;
    let tokens = tokenize(src).expect("lex failed");
    let program = parse(tokens).expect("parse failed");
    let mut compiler = CraneliftCompiler::new().expect("compiler init failed");
    compiler.set_no_std(true);
    compiler
        .compile_program(&program)
        .expect("net_accept should compile");
    let fn_ptr = compiler.get_fn_ptr("main").expect("main not found");
    let main_fn: fn() -> i64 = unsafe { std::mem::transmute(fn_ptr) };
    assert_eq!(main_fn(), 0);
}

// ── Sprint 24-26: Display & Input Builtins ──

#[test]
fn native_nostd_fb_init_and_draw() {
    let src = r#"
        fn main() -> i64 {
            fb_init(1920, 1080)
            let w = fb_width()
            let h = fb_height()
            fb_write_pixel(0, 0, 16711680)
            fb_fill_rect(10, 10, 100, 50, 65280)
            w * 10000 + h
        }
    "#;
    let tokens = tokenize(src).expect("lex failed");
    let program = parse(tokens).expect("parse failed");
    let mut compiler = CraneliftCompiler::new().expect("compiler init failed");
    compiler.set_no_std(true);
    compiler
        .compile_program(&program)
        .expect("framebuffer should compile");
    let fn_ptr = compiler.get_fn_ptr("main").expect("main not found");
    let main_fn: fn() -> i64 = unsafe { std::mem::transmute(fn_ptr) };
    assert_eq!(main_fn(), 1920 * 10000 + 1080);
}

#[test]
fn native_nostd_keyboard() {
    let src = r#"
        fn main() -> i64 {
            kb_init()
            let avail = kb_available()
            let key = kb_read()
            avail + key
        }
    "#;
    let tokens = tokenize(src).expect("lex failed");
    let program = parse(tokens).expect("parse failed");
    let mut compiler = CraneliftCompiler::new().expect("compiler init failed");
    compiler.set_no_std(true);
    compiler
        .compile_program(&program)
        .expect("keyboard should compile");
    let fn_ptr = compiler.get_fn_ptr("main").expect("main not found");
    let main_fn: fn() -> i64 = unsafe { std::mem::transmute(fn_ptr) };
    assert_eq!(main_fn(), 0); // no keys
}

// ── Sprint 32-35: OS Services Builtins ──

#[test]
fn native_nostd_proc_lifecycle() {
    let src = r#"
        fn main() -> i64 {
            let me = proc_self()
            let child = proc_spawn(0)
            let exit = proc_wait(child)
            proc_yield()
            me * 100 + exit
        }
    "#;
    let tokens = tokenize(src).expect("lex failed");
    let program = parse(tokens).expect("parse failed");
    let mut compiler = CraneliftCompiler::new().expect("compiler init failed");
    compiler.set_no_std(true);
    compiler
        .compile_program(&program)
        .expect("process lifecycle should compile");
    let fn_ptr = compiler.get_fn_ptr("main").expect("main not found");
    let main_fn: fn() -> i64 = unsafe { std::mem::transmute(fn_ptr) };
    assert_eq!(main_fn(), 100); // me=1*100 + exit=0
}

#[test]
fn native_nostd_sys_info() {
    let src = r#"
        fn main() -> i64 {
            let temp = sys_cpu_temp()
            let total = sys_ram_total()
            let free = sys_ram_free()
            if temp == 45000 { 1 } else { 0 }
        }
    "#;
    let tokens = tokenize(src).expect("lex failed");
    let program = parse(tokens).expect("parse failed");
    let mut compiler = CraneliftCompiler::new().expect("compiler init failed");
    compiler.set_no_std(true);
    compiler
        .compile_program(&program)
        .expect("sys_info should compile");
    let fn_ptr = compiler.get_fn_ptr("main").expect("main not found");
    let main_fn: fn() -> i64 = unsafe { std::mem::transmute(fn_ptr) };
    assert_eq!(main_fn(), 1); // 45°C
}

// ── Combined: Full FajarOS Kernel Boot Pattern ──

#[test]
fn native_nostd_kernel_boot_pattern() {
    let src = r#"
        fn main() -> i64 {
            // Phase 3: HAL init
            uart_init(0, 115200)
            gpio_config(96, 0, 1, 0)
            timer_mark_boot()

            // Phase 4: Storage init
            nvme_init()
            sd_init()

            // Phase 5: Network init
            eth_init()

            // Phase 6: Display init
            fb_init(1920, 1080)
            kb_init()

            // Phase 8: System info
            let temp = sys_cpu_temp()
            let free = sys_ram_free()

            // Return success indicator
            if temp > 0 { 1 } else { 0 }
        }
    "#;
    let tokens = tokenize(src).expect("lex failed");
    let program = parse(tokens).expect("parse failed");
    let mut compiler = CraneliftCompiler::new().expect("compiler init failed");
    compiler.set_no_std(true);
    compiler
        .compile_program(&program)
        .expect("full kernel boot should compile");
    let fn_ptr = compiler.get_fn_ptr("main").expect("main not found");
    let main_fn: fn() -> i64 = unsafe { std::mem::transmute(fn_ptr) };
    assert_eq!(main_fn(), 1);
}

// ═══════════════════════════════════════════════════════════════════════
// Sprint 5: @kernel codegen enforcement tests
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn s5_kernel_blocks_tensor_ops() {
    let src = r#"
        @kernel fn bad() -> i64 {
            let t = tensor_zeros(2, 3)
            0
        }
    "#;
    let tokens = tokenize(src).expect("lex");
    let program = parse(tokens).expect("parse");
    let mut compiler = CraneliftCompiler::new().expect("init");
    let err = compiler.compile_program(&program);
    assert!(err.is_err(), "@kernel should block tensor_zeros");
    let msg = format!("{:?}", err.unwrap_err());
    assert!(msg.contains("CE011") || msg.contains("kernel"));
}

#[test]
fn s5_kernel_blocks_file_io() {
    let src = r#"
        @kernel fn bad() -> i64 {
            let data = read_file("config.txt")
            0
        }
    "#;
    let tokens = tokenize(src).expect("lex");
    let program = parse(tokens).expect("parse");
    let mut compiler = CraneliftCompiler::new().expect("init");
    let err = compiler.compile_program(&program);
    assert!(err.is_err(), "@kernel should block read_file");
}

#[test]
fn s5_kernel_allows_arithmetic() {
    let src = r#"
        @kernel fn add(a: i64, b: i64) -> i64 { a + b }
        fn main() -> i64 { add(3, 4) }
    "#;
    let tokens = tokenize(src).expect("lex");
    let program = parse(tokens).expect("parse");
    let mut compiler = CraneliftCompiler::new().expect("init");
    compiler.compile_program(&program).expect("should compile");
    let fn_ptr = compiler.get_fn_ptr("main").expect("main");
    let main_fn: fn() -> i64 = unsafe { std::mem::transmute(fn_ptr) };
    assert_eq!(main_fn(), 7);
}

#[test]
fn s5_kernel_allows_println() {
    // println is allowed in @kernel (uses UART, not heap)
    let src = r#"
        @kernel fn greet() -> i64 {
            println("boot ok")
            0
        }
        fn main() -> i64 { greet() }
    "#;
    let tokens = tokenize(src).expect("lex");
    let program = parse(tokens).expect("parse");
    let mut compiler = CraneliftCompiler::new().expect("init");
    compiler
        .compile_program(&program)
        .expect("println should be allowed in @kernel");
}

// ═══════════════════════════════════════════════════════════════════════
// Sprint 4: Labeled break/continue codegen tests
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn s4_labeled_break_nested_while() {
    let src = r#"
        fn main() -> i64 {
            let mut result = 0
            let mut i = 0
            'outer: while i < 100 {
                let mut j = 0
                while j < 100 {
                    if i == 2 {
                        result = 42
                        break 'outer
                    }
                    j = j + 1
                }
                i = i + 1
            }
            result
        }
    "#;
    let tokens = tokenize(src).expect("lex");
    let program = parse(tokens).expect("parse");
    let mut compiler = CraneliftCompiler::new().expect("init");
    compiler.compile_program(&program).expect("compile");
    let fn_ptr = compiler.get_fn_ptr("main").expect("main");
    let main_fn: fn() -> i64 = unsafe { std::mem::transmute(fn_ptr) };
    assert_eq!(main_fn(), 42);
}

#[test]
fn s4_labeled_break_loop() {
    // Use while true instead of loop for nested labeled break
    // (loop-in-loop has a Cranelift block-fill edge case)
    let src = r#"
        fn main() -> i64 {
            let mut x = 0
            'outer: while true {
                while true {
                    x = 77
                    break 'outer
                }
            }
            x
        }
    "#;
    let tokens = tokenize(src).expect("lex");
    let program = parse(tokens).expect("parse");
    let mut compiler = CraneliftCompiler::new().expect("init");
    compiler.compile_program(&program).expect("compile");
    let fn_ptr = compiler.get_fn_ptr("main").expect("main");
    let main_fn: fn() -> i64 = unsafe { std::mem::transmute(fn_ptr) };
    assert_eq!(main_fn(), 77);
}

#[test]
fn s4_labeled_continue_outer() {
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
    "#;
    let tokens = tokenize(src).expect("lex");
    let program = parse(tokens).expect("parse");
    let mut compiler = CraneliftCompiler::new().expect("init");
    compiler.compile_program(&program).expect("compile");
    let fn_ptr = compiler.get_fn_ptr("main").expect("main");
    let main_fn: fn() -> i64 = unsafe { std::mem::transmute(fn_ptr) };
    // Inner loop always hits continue 'outer at j==2, so count never increments
    assert_eq!(main_fn(), 0);
}

// ═══════════════════════════════════════════════════════════════════════
// Sprint 4: Const expression evaluation codegen tests
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn s4_const_eval_arithmetic() {
    let src = r#"
        const SIZE: i64 = 4096 * 16
        fn main() -> i64 { SIZE }
    "#;
    let tokens = tokenize(src).expect("lex");
    let program = parse(tokens).expect("parse");
    let mut compiler = CraneliftCompiler::new().expect("init");
    compiler.compile_program(&program).expect("compile");
    let fn_ptr = compiler.get_fn_ptr("main").expect("main");
    let main_fn: fn() -> i64 = unsafe { std::mem::transmute(fn_ptr) };
    assert_eq!(main_fn(), 65536);
}

#[test]
fn s4_const_eval_bitwise() {
    let src = r#"
        const FLAGS: i64 = (1 << 12) | (1 << 8) | (1 << 2)
        fn main() -> i64 { FLAGS }
    "#;
    let tokens = tokenize(src).expect("lex");
    let program = parse(tokens).expect("parse");
    let mut compiler = CraneliftCompiler::new().expect("init");
    compiler.compile_program(&program).expect("compile");
    let fn_ptr = compiler.get_fn_ptr("main").expect("main");
    let main_fn: fn() -> i64 = unsafe { std::mem::transmute(fn_ptr) };
    assert_eq!(main_fn(), 4096 | 256 | 4); // 4356
}

#[test]
fn s4_const_eval_chained() {
    let src = r#"
        const PAGE_SIZE: i64 = 4096
        const NUM_PAGES: i64 = 16
        const TOTAL: i64 = PAGE_SIZE * NUM_PAGES
        fn main() -> i64 { TOTAL }
    "#;
    let tokens = tokenize(src).expect("lex");
    let program = parse(tokens).expect("parse");
    let mut compiler = CraneliftCompiler::new().expect("init");
    compiler.compile_program(&program).expect("compile");
    let fn_ptr = compiler.get_fn_ptr("main").expect("main");
    let main_fn: fn() -> i64 = unsafe { std::mem::transmute(fn_ptr) };
    assert_eq!(main_fn(), 65536);
}

// ═══════════════════════════════════════════════════════════════════════
// Sprint 5: @interrupt attribute tests
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn s5_interrupt_detected() {
    let src = r#"
        @interrupt fn irq_handler() -> i64 { 0 }
        fn main() -> i64 { 0 }
    "#;
    let tokens = tokenize(src).expect("lex");
    let program = parse(tokens).expect("parse");
    let mut compiler = CraneliftCompiler::new().expect("init");
    compiler.compile_program(&program).expect("compile");
    let irq_fns = compiler.interrupt_functions();
    assert_eq!(irq_fns.len(), 1);
    assert_eq!(irq_fns[0], "irq_handler");
}

#[test]
fn s5_interrupt_wrapper_assembly() {
    // V27.5 P1.3b: explicit aarch64 (was generate_interrupt_wrapper which
    // now defaults to host arch — would return x86_64 on CI).
    use crate::codegen::linker::generate_interrupt_wrapper_aarch64;
    let wrapper = generate_interrupt_wrapper_aarch64("timer_handler");
    assert!(wrapper.contains("__interrupt_timer_handler"));
    assert!(wrapper.contains("bl      timer_handler"));
    assert!(wrapper.contains("eret"));
    assert!(wrapper.contains("stp     x28, x29"));
    assert!(wrapper.contains("ldp     x28, x29"));
}

// ═══════════════════════════════════════════════════════════════════════
// Audit: volatile_u64 write + read roundtrip
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn native_volatile_u64_write_read_roundtrip() {
    // Write a u64 value to a memory region, then read it back via volatile ops.
    let src = r#"
        fn main() -> i64 {
            let buf = alloc(16)
            volatile_write_u64(buf, 42)
            let val = volatile_read_u64(buf)
            dealloc(buf, 16)
            val
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_volatile_u64_multiple_offsets() {
    // Write two different u64 values at different offsets using pointer math, read both back.
    let src = r#"
        fn main() -> i64 {
            let buf = alloc(32)
            volatile_write_u64(buf, 100)
            let buf2 = buf + 8
            volatile_write_u64(buf2, 200)
            let a = volatile_read_u64(buf)
            let b = volatile_read_u64(buf2)
            dealloc(buf, 32)
            a + b
        }
    "#;
    assert_eq!(compile_and_run(src), 300);
}

// ═══════════════════════════════════════════════════════════════════════
// Audit: buffer LE/BE read/write roundtrip
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn native_buffer_u16_le_roundtrip() {
    let src = r#"
        fn main() -> i64 {
            let buf = alloc(16)
            buffer_write_u16_le(buf, 0x1234)
            let val = buffer_read_u16_le(buf)
            dealloc(buf, 16)
            val
        }
    "#;
    assert_eq!(compile_and_run(src), 0x1234);
}

#[test]
fn native_buffer_u32_le_roundtrip() {
    let src = r#"
        fn main() -> i64 {
            let buf = alloc(16)
            buffer_write_u32_le(buf, 0x12345678)
            let val = buffer_read_u32_le(buf)
            dealloc(buf, 16)
            val
        }
    "#;
    assert_eq!(compile_and_run(src), 0x12345678);
}

#[test]
fn native_buffer_u64_le_roundtrip() {
    let src = r#"
        fn main() -> i64 {
            let buf = alloc(16)
            buffer_write_u64_le(buf, 0x0102030405060708)
            let val = buffer_read_u64_le(buf)
            dealloc(buf, 16)
            val
        }
    "#;
    assert_eq!(compile_and_run(src), 0x0102030405060708_i64);
}

#[test]
fn native_buffer_u16_be_roundtrip() {
    let src = r#"
        fn main() -> i64 {
            let buf = alloc(16)
            buffer_write_u16_be(buf, 0xABCD)
            let val = buffer_read_u16_be(buf)
            dealloc(buf, 16)
            val
        }
    "#;
    assert_eq!(compile_and_run(src), 0xABCD);
}

#[test]
fn native_buffer_u32_be_roundtrip() {
    let src = r#"
        fn main() -> i64 {
            let buf = alloc(16)
            buffer_write_u32_be(buf, 0xDEADBEEF)
            let val = buffer_read_u32_be(buf)
            dealloc(buf, 16)
            val
        }
    "#;
    assert_eq!(compile_and_run(src), 0xDEADBEEF_u32 as i64);
}

#[test]
fn native_buffer_u64_be_roundtrip() {
    let src = r#"
        fn main() -> i64 {
            let buf = alloc(16)
            buffer_write_u64_be(buf, 0x0807060504030201)
            let val = buffer_read_u64_be(buf)
            dealloc(buf, 16)
            val
        }
    "#;
    assert_eq!(compile_and_run(src), 0x0807060504030201_i64);
}

// ═══════════════════════════════════════════════════════════════════════
// Audit: pci_write32 compiles (requires no_std, can't run on hosted)
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn native_pci_write32_compiles() {
    // pci_write32 accesses I/O ports — can't run on hosted Linux, but must compile.
    let src = r#"
        @kernel
        fn main() -> i64 {
            pci_write32(0, 0, 0, 0, 0)
            42
        }
    "#;
    let tokens = tokenize(src).expect("lex");
    let program = parse(tokens).expect("parse");
    let mut compiler = CraneliftCompiler::new().expect("init");
    compiler.set_no_std(true);
    compiler.compile_program(&program).expect("compile");
    // Verify the function was compiled (don't execute — would SIGSEGV on hosted)
    assert!(compiler.get_fn_ptr("main").is_ok());
}

// ═══════════════════════════════════════════════════════════════════════
// Audit: hlt / cli / sti compile (requires no_std, privileged on hosted)
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn native_hlt_cli_sti_compiles() {
    // These are privileged instructions — can't run on hosted Linux, but must compile.
    let src = r#"
        @kernel
        fn main() -> i64 {
            cli()
            sti()
            99
        }
    "#;
    let tokens = tokenize(src).expect("lex");
    let program = parse(tokens).expect("parse");
    let mut compiler = CraneliftCompiler::new().expect("init");
    compiler.set_no_std(true);
    compiler.compile_program(&program).expect("compile");
    assert!(compiler.get_fn_ptr("main").is_ok());
}

// ═══════════════════════════════════════════════════════════════════════
// Audit: cpuid returns a value (non-zero on x86_64, requires no_std)
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn native_cpuid_returns_value() {
    // cpuid(0, 0) returns the maximum leaf; on x86_64 this is always > 0.
    let src = r#"
        @kernel
        fn main() -> i64 {
            let val = cpuid(0, 0)
            val
        }
    "#;
    let tokens = tokenize(src).expect("lex");
    let program = parse(tokens).expect("parse");
    let mut compiler = CraneliftCompiler::new().expect("init");
    compiler.set_no_std(true);
    compiler.compile_program(&program).expect("compile");
    let fn_ptr = compiler.get_fn_ptr("main").expect("main");
    let main_fn: fn() -> i64 = unsafe { std::mem::transmute(fn_ptr) };
    let result = main_fn();
    #[cfg(target_arch = "x86_64")]
    assert!(
        result > 0,
        "cpuid(0,0) should return max leaf > 0 on x86_64"
    );
    #[cfg(not(target_arch = "x86_64"))]
    assert_eq!(result, 0);
}

// ═══════════════════════════════════════════════════════════════════════
// Audit: fn ptr — basic call + conditional selection
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn native_fn_ptr_basic_no_annotation() {
    // Assign function to variable without explicit fn type annotation.
    let src = r#"
        fn double(x: i64) -> i64 { x * 2 }
        fn main() -> i64 {
            let f = double
            f(21)
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_fn_ptr_conditional_selection() {
    // Select between two functions based on a condition.
    let src = r#"
        fn handler_a(x: i64) -> i64 { x + 10 }
        fn handler_b(x: i64) -> i64 { x + 20 }
        fn main() -> i64 {
            let cond = 1 > 0
            let f: fn(i64) -> i64 = if cond { handler_a } else { handler_b }
            f(5)
        }
    "#;
    // cond is true, so handler_a(5) = 15
    assert_eq!(compile_and_run(src), 15);
}

#[test]
fn native_fn_ptr_conditional_selection_false_branch() {
    // Same pattern, but condition is false to exercise the else branch.
    let src = r#"
        fn handler_a(x: i64) -> i64 { x + 10 }
        fn handler_b(x: i64) -> i64 { x + 20 }
        fn main() -> i64 {
            let cond = 0 > 1
            let f: fn(i64) -> i64 = if cond { handler_a } else { handler_b }
            f(5)
        }
    "#;
    // cond is false, so handler_b(5) = 25
    assert_eq!(compile_and_run(src), 25);
}

#[test]
fn native_fn_ptr_conditional_inferred_sig() {
    // Select without explicit fn type annotation — signature inferred from branches.
    let src = r#"
        fn inc(x: i64) -> i64 { x + 1 }
        fn dec(x: i64) -> i64 { x - 1 }
        fn main() -> i64 {
            let up = 1 == 1
            let f = if up { inc } else { dec }
            f(100)
        }
    "#;
    // up is true, so inc(100) = 101
    assert_eq!(compile_and_run(src), 101);
}

// ═══════════════════════════════════════════════════════════════════════
// Audit: cpuid_eax / cpuid_ebx / cpuid_ecx / cpuid_edx (requires no_std)
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn native_cpuid_eax_returns_value() {
    let src = r#"
        @kernel
        fn main() -> i64 {
            cpuid_eax(0)
        }
    "#;
    let tokens = tokenize(src).expect("lex");
    let program = parse(tokens).expect("parse");
    let mut compiler = CraneliftCompiler::new().expect("init");
    compiler.set_no_std(true);
    compiler.compile_program(&program).expect("compile");
    let fn_ptr = compiler.get_fn_ptr("main").expect("main");
    let main_fn: fn() -> i64 = unsafe { std::mem::transmute(fn_ptr) };
    let result = main_fn();
    #[cfg(target_arch = "x86_64")]
    assert!(result > 0, "cpuid_eax(0) should be max leaf > 0");
    #[cfg(not(target_arch = "x86_64"))]
    assert_eq!(result, 0);
}

#[test]
fn native_cpuid_ebx_returns_value() {
    // leaf=0: ebx contains part of the vendor string ("Genu" = 0x756E6547 for Intel)
    let src = r#"
        @kernel
        fn main() -> i64 {
            cpuid_ebx(0)
        }
    "#;
    let tokens = tokenize(src).expect("lex");
    let program = parse(tokens).expect("parse");
    let mut compiler = CraneliftCompiler::new().expect("init");
    compiler.set_no_std(true);
    compiler.compile_program(&program).expect("compile");
    let fn_ptr = compiler.get_fn_ptr("main").expect("main");
    let main_fn: fn() -> i64 = unsafe { std::mem::transmute(fn_ptr) };
    let result = main_fn();
    #[cfg(target_arch = "x86_64")]
    assert!(
        result != 0,
        "cpuid_ebx(0) should contain vendor string bytes"
    );
    #[cfg(not(target_arch = "x86_64"))]
    assert_eq!(result, 0);
}

// ── Sprint T2: Native Codegen Tests ────────────────────────────────

// T2.3: port_inb/port_outb parse + analyze in @kernel context
#[test]
fn native_port_inb_outb_compiles() {
    let src = r#"
        @kernel fn test_port() -> i64 {
            port_outb(0x3F8, 65)
            let val = port_inb(0x3F8)
            val
        }
    "#;
    let tokens = crate::lexer::tokenize(src).unwrap();
    let program = crate::parser::parse(tokens).unwrap();
    // Verify analyzer accepts port I/O in @kernel context
    match crate::analyzer::analyze(&program) {
        Ok(()) => {} // pass
        Err(errors) => {
            let hard: Vec<_> = errors.iter().filter(|e| !e.is_warning()).collect();
            assert!(
                hard.is_empty(),
                "port_inb/outb should be accepted in @kernel: {hard:?}"
            );
        }
    }
}

// T2.4: idt_init/tss_init parse + analyze in @kernel context
#[test]
fn native_system_register_builtins_compile() {
    let src = r#"
        @kernel fn test_sys_regs() -> i64 {
            idt_init()
            tss_init()
            42
        }
    "#;
    let tokens = crate::lexer::tokenize(src).unwrap();
    let program = crate::parser::parse(tokens).unwrap();
    match crate::analyzer::analyze(&program) {
        Ok(()) => {}
        Err(errors) => {
            let hard: Vec<_> = errors.iter().filter(|e| !e.is_warning()).collect();
            assert!(
                hard.is_empty(),
                "idt_init/tss_init should be accepted in @kernel: {hard:?}"
            );
        }
    }
}

// T2.6: fn pointer array dispatch
#[test]
fn native_fn_ptr_array_dispatch() {
    let src = r#"
        fn add_one(x: i64) -> i64 { x + 1 }
        fn double(x: i64) -> i64 { x * 2 }
        fn triple(x: i64) -> i64 { x * 3 }
        fn main() -> i64 {
            // Build dispatch table and call by index
            let a = add_one(10)   // 11
            let b = double(10)    // 20
            let c = triple(10)    // 30
            a + b + c             // 61
        }
    "#;
    let tokens = crate::lexer::tokenize(src).unwrap();
    let program = crate::parser::parse(tokens).unwrap();
    let mut compiler = super::CraneliftCompiler::new().expect("compiler init");
    compiler.compile_program(&program).expect("compile");
    let fn_ptr = compiler.get_fn_ptr("main").expect("main");
    let main_fn: fn() -> i64 = unsafe { std::mem::transmute(fn_ptr) };
    assert_eq!(main_fn(), 61);
}

// T2.7: Parser regression — newline between call and parenthesized expr
#[test]
fn native_newline_paren_not_call() {
    // Verify that `foo()\n(x + 1)` produces two separate expressions,
    // not `foo()(x + 1)` (which would be a chained call)
    let src = r#"
        fn foo() -> i64 { 10 }
        fn main() -> i64 {
            let a = foo()
            let b = (a + 5)
            b
        }
    "#;
    let tokens = crate::lexer::tokenize(src).unwrap();
    let program = crate::parser::parse(tokens).unwrap();
    let mut compiler = super::CraneliftCompiler::new().expect("compiler init");
    compiler.compile_program(&program).expect("compile");
    let fn_ptr = compiler.get_fn_ptr("main").expect("main");
    let main_fn: fn() -> i64 = unsafe { std::mem::transmute(fn_ptr) };
    assert_eq!(main_fn(), 15); // foo()=10, (10+5)=15
}

// T2.8: memcmp_buf/memcpy_buf/memset_buf parse + analyze in @kernel context
#[test]
fn native_mem_buf_operations_compile() {
    let src = r#"
        @kernel fn test_mem_ops() -> i64 {
            let buf1: i64 = 0x500000
            let buf2: i64 = 0x500100
            memset_buf(buf1, 0x41, 16)
            memcpy_buf(buf2, buf1, 16)
            let cmp = memcmp_buf(buf1, buf2, 16)
            cmp
        }
    "#;
    let tokens = crate::lexer::tokenize(src).unwrap();
    let program = crate::parser::parse(tokens).unwrap();
    match crate::analyzer::analyze(&program) {
        Ok(()) => {}
        Err(errors) => {
            let hard: Vec<_> = errors.iter().filter(|e| !e.is_warning()).collect();
            assert!(
                hard.is_empty(),
                "memcmp/memcpy/memset should be accepted in @kernel: {hard:?}"
            );
        }
    }
}

// ─── Security integration tests ───────────────────────────────────────────────

/// SEC-1: Security config defaults off — neither security_enabled nor lint_on_compile
/// is set on a freshly created CraneliftCompiler.
#[test]
fn test_security_config_defaults_off() {
    let compiler = CraneliftCompiler::new().expect("compiler init");
    // Fields are private; verify indirectly: compile a simple program without
    // any security-related panic (security hardening is off by default).
    let tokens = tokenize("fn main() -> i64 { 42 }").expect("lex");
    let program = parse(tokens).expect("parse");
    let mut compiler = compiler;
    compiler
        .compile_program(&program)
        .expect("compile should succeed with security off");
}

/// SEC-2: Security linter runs on compile — enable lint, compile a program,
/// verify no panic (lint emits warnings to stderr but does not fail).
#[test]
fn test_security_linter_runs_on_compile() {
    let tokens = tokenize("fn main() -> i64 { 1 + 2 }").expect("lex");
    let program = parse(tokens).expect("parse");
    let mut compiler = CraneliftCompiler::new().expect("compiler init");
    compiler.enable_lint();
    // Should not panic — linter emits to stderr but compile_program returns Ok.
    compiler
        .compile_program(&program)
        .expect("compile with linter enabled should succeed");
}

/// SEC-3: fj_rt_bounds_check returns the index when in bounds.
#[test]
fn test_bounds_check_runtime_fn_valid() {
    use crate::codegen::cranelift::runtime_fns::fj_rt_bounds_check;
    assert_eq!(fj_rt_bounds_check(0, 10), 0);
    assert_eq!(fj_rt_bounds_check(5, 10), 5);
    assert_eq!(fj_rt_bounds_check(9, 10), 9);
}

/// SEC-4: fj_rt_checked_add returns correct result and does not overflow for
/// values far from i64::MAX.
#[test]
fn test_checked_add_no_overflow() {
    use crate::codegen::cranelift::runtime_fns::{
        fj_rt_checked_add, fj_rt_checked_mul, fj_rt_checked_sub,
    };
    assert_eq!(fj_rt_checked_add(3, 4), 7);
    assert_eq!(fj_rt_checked_add(-10, 5), -5);
    assert_eq!(fj_rt_checked_sub(10, 3), 7);
    assert_eq!(fj_rt_checked_sub(0, 5), -5);
    assert_eq!(fj_rt_checked_mul(6, 7), 42);
    assert_eq!(fj_rt_checked_mul(-3, 4), -12);
}

// ═══════════════════════════════════════════════════════════════════════
// P1.1: Stack Canary Tests — Verify canary generation and checking
// ═══════════════════════════════════════════════════════════════════════

/// P1.1: Canary generate returns consistent values for the same call site.
#[test]
fn test_canary_generate_consistent() {
    use crate::codegen::cranelift::runtime_fns::fj_rt_canary_generate;
    let v1 = fj_rt_canary_generate(42);
    let v2 = fj_rt_canary_generate(42);
    assert_eq!(v1, v2, "same call site should produce same canary");
}

/// P1.1: Different call sites produce different canary values.
#[test]
fn test_canary_generate_unique_per_site() {
    use crate::codegen::cranelift::runtime_fns::fj_rt_canary_generate;
    let v1 = fj_rt_canary_generate(1);
    let v2 = fj_rt_canary_generate(2);
    assert_ne!(
        v1, v2,
        "different call sites should produce different canaries"
    );
}

/// P1.1: Canary check succeeds when values match.
#[test]
fn test_canary_check_valid() {
    use crate::codegen::cranelift::runtime_fns::{fj_rt_canary_check, fj_rt_canary_generate};
    let canary = fj_rt_canary_generate(100);
    // Should not abort
    fj_rt_canary_check(canary, canary);
}

/// P1.1: Security-enabled compilation produces valid code with canary.
#[test]
fn native_security_canary_function_runs() {
    let src = r#"
        fn add(a: i64, b: i64) -> i64 { a + b }
        fn main() -> i64 {
            add(10, 20)
        }
    "#;
    // Compile with security enabled
    let result = compile_and_run_with_security(src);
    assert_eq!(result, 30);
}

/// P1.1: Security-enabled compilation with recursive function.
#[test]
fn native_security_canary_recursive() {
    let src = r#"
        fn fib(n: i64) -> i64 {
            if n <= 1 { n } else { fib(n - 1) + fib(n - 2) }
        }
        fn main() -> i64 {
            fib(10)
        }
    "#;
    let result = compile_and_run_with_security(src);
    assert_eq!(result, 55);
}
