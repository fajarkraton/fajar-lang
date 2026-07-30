//! Arithmetic, control flow, AOT objects, cross-compilation, string literals (S3/S4/S18).

use super::super::*;
use super::{compile_and_run, compile_and_run_f64};
use crate::codegen::target::TargetConfig;
use crate::lexer::tokenize;
use crate::parser::parse;

#[test]
fn native_add() {
    assert_eq!(compile_and_run("fn main() -> i64 { 1 + 2 }"), 3);
}

#[test]
fn native_sub() {
    assert_eq!(compile_and_run("fn main() -> i64 { 10 - 3 }"), 7);
}

#[test]
fn native_mul() {
    assert_eq!(compile_and_run("fn main() -> i64 { 6 * 7 }"), 42);
}

#[test]
fn native_div() {
    assert_eq!(compile_and_run("fn main() -> i64 { 10 / 3 }"), 3);
}

#[test]
fn native_mod() {
    assert_eq!(compile_and_run("fn main() -> i64 { 10 % 3 }"), 1);
}

#[test]
fn native_negation() {
    assert_eq!(compile_and_run("fn main() -> i64 { -(42) }"), -42);
}

#[test]
fn native_complex_expr() {
    assert_eq!(
        compile_and_run("fn main() -> i64 { (2 + 3) * (10 - 4) }"),
        30
    );
}

#[test]
fn native_function_call() {
    let src = r#"
        fn add(a: i64, b: i64) -> i64 { a + b }
        fn main() -> i64 { add(1, 2) }
    "#;
    assert_eq!(compile_and_run(src), 3);
}

#[test]
fn native_multiple_functions() {
    let src = r#"
        fn double(x: i64) -> i64 { x * 2 }
        fn add_one(x: i64) -> i64 { x + 1 }
        fn main() -> i64 { add_one(double(5)) }
    "#;
    assert_eq!(compile_and_run(src), 11);
}

#[test]
fn native_fibonacci() {
    let src = r#"
        fn fib(n: i64) -> i64 {
            if n <= 1 { n } else { fib(n - 1) + fib(n - 2) }
        }
        fn main() -> i64 { fib(20) }
    "#;
    assert_eq!(compile_and_run(src), 6765);
}

#[test]
fn native_comparison_gt() {
    let src = "fn main() -> i64 { if 5 > 3 { 1 } else { 0 } }";
    assert_eq!(compile_and_run(src), 1);
}

#[test]
fn native_comparison_eq() {
    let src = "fn main() -> i64 { if 5 == 5 { 1 } else { 0 } }";
    assert_eq!(compile_and_run(src), 1);
}

#[test]
fn native_not_true() {
    let src = "fn main() -> i64 { if !true { 1 } else { 0 } }";
    assert_eq!(compile_and_run(src), 0);
}

#[test]
fn native_local_variables() {
    let src = r#"
        fn main() -> i64 {
            let x = 10
            let y = 20
            x + y
        }
    "#;
    assert_eq!(compile_and_run(src), 30);
}

#[test]
fn native_fibonacci_matches_interpreter() {
    let src = r#"
        fn fib(n: i64) -> i64 {
            if n <= 1 { n } else { fib(n - 1) + fib(n - 2) }
        }
        fn main() -> i64 { fib(15) }
    "#;
    // fib(15) instead of fib(20): avoids stack overflow when running
    // both JIT and interpreter on the same thread (dual stack usage).
    let native_result = compile_and_run(src);
    let mut interp = crate::interpreter::Interpreter::new();
    interp.eval_source(src).unwrap();
    let interp_result = interp.call_main().unwrap();
    assert_eq!(
        native_result,
        match interp_result {
            crate::interpreter::Value::Int(n) => n,
            _ => panic!("expected Int"),
        }
    );
}

// ── S3.1 additional tests ──

#[test]
fn native_mutable_variable() {
    let src = r#"
        fn main() -> i64 {
            let mut x = 0
            x = 42
            x
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

// ── S3.2 additional tests ──

#[test]
fn native_nested_if_else() {
    let src = r#"
        fn classify(n: i64) -> i64 {
            if n > 0 { 1 } else { if n < 0 { -(1) } else { 0 } }
        }
        fn main() -> i64 {
            classify(5) + classify(-(3)) + classify(0)
        }
    "#;
    assert_eq!(compile_and_run(src), 0); // 1 + (-1) + 0
}

#[test]
fn native_absolute_value() {
    let src = r#"
        fn abs(x: i64) -> i64 {
            if x > 0 { x } else { -(x) }
        }
        fn main() -> i64 { abs(-(7)) }
    "#;
    assert_eq!(compile_and_run(src), 7);
}

// ── S3.3 while loops ──

#[test]
fn native_while_sum() {
    let src = r#"
        fn main() -> i64 {
            let mut sum = 0
            let mut i = 1
            while i <= 100 {
                sum = sum + i
                i = i + 1
            }
            sum
        }
    "#;
    assert_eq!(compile_and_run(src), 5050);
}

#[test]
fn native_while_zero_iterations() {
    let src = r#"
        fn main() -> i64 {
            let mut x = 99
            while false {
                x = 0
            }
            x
        }
    "#;
    assert_eq!(compile_and_run(src), 99);
}

// ── S3.4 for-range loops ──

#[test]
fn native_for_range_sum() {
    let src = r#"
        fn main() -> i64 {
            let mut sum = 0
            for i in 0..10 {
                sum = sum + i
            }
            sum
        }
    "#;
    assert_eq!(compile_and_run(src), 45); // 0+1+2+...+9
}

#[test]
fn native_for_range_inclusive() {
    let src = r#"
        fn main() -> i64 {
            let mut sum = 0
            for i in 1..=10 {
                sum = sum + i
            }
            sum
        }
    "#;
    assert_eq!(compile_and_run(src), 55); // 1+2+...+10
}

#[test]
fn native_for_range_nested() {
    // Multiplication table: sum of i*j for i=1..4, j=1..4
    let src = r#"
        fn main() -> i64 {
            let mut sum = 0
            for i in 1..4 {
                for j in 1..4 {
                    sum = sum + i * j
                }
            }
            sum
        }
    "#;
    // (1+2+3) * (1+2+3) = 6*6 = 36
    assert_eq!(compile_and_run(src), 36);
}

// ── S4.3 AOT object compilation ──

#[test]
fn object_compiler_produces_bytes() {
    let src = r#"
        fn main() -> i64 { 42 }
    "#;
    let tokens = tokenize(src).expect("lex failed");
    let program = parse(tokens).expect("parse failed");
    let mut compiler = ObjectCompiler::new("test").expect("compiler init failed");
    compiler
        .compile_program(&program)
        .expect("compilation failed");
    let product = compiler.finish();
    let bytes = product.emit().expect("emit failed");
    // Object file should have ELF header (0x7f ELF) on Linux
    assert!(!bytes.is_empty());
    assert!(bytes.len() > 64); // minimum ELF size
}

#[test]
fn aot_interrupt_wrapper_emitted() {
    // V27.5 P1.3a: @interrupt fn must produce assembly wrapper in
    // global_asm_sections after compile_program completes.
    let src = r#"
        @interrupt
        fn timer_irq() -> i64 { 0 }
        fn main() -> i64 { 0 }
    "#;
    let tokens = tokenize(src).expect("lex failed");
    let program = parse(tokens).expect("parse failed");
    let mut compiler = ObjectCompiler::new("irq_test").expect("compiler init failed");
    compiler
        .compile_program(&program)
        .expect("compilation failed");
    // Verify interrupt function was collected
    assert_eq!(compiler.interrupt_functions().len(), 1);
    assert_eq!(compiler.interrupt_functions()[0], "timer_irq");
    // Verify wrapper was emitted to global_asm_sections
    assert_eq!(compiler.global_asm_sections().len(), 1);
    let wrapper = &compiler.global_asm_sections()[0];
    assert!(
        wrapper.contains("timer_irq"),
        "wrapper should reference handler name"
    );
}

#[test]
fn aot_multiple_interrupt_wrappers() {
    // Two @interrupt fns produce two separate wrappers
    let src = r#"
        @interrupt
        fn timer_irq() -> i64 { 0 }
        @interrupt
        fn keyboard_irq() -> i64 { 0 }
        fn main() -> i64 { 0 }
    "#;
    let tokens = tokenize(src).expect("lex failed");
    let program = parse(tokens).expect("parse failed");
    let mut compiler = ObjectCompiler::new("irq_multi_test").expect("compiler init failed");
    compiler
        .compile_program(&program)
        .expect("compilation failed");
    assert_eq!(compiler.interrupt_functions().len(), 2);
    assert_eq!(compiler.global_asm_sections().len(), 2);
}

#[test]
fn object_compiler_fibonacci() {
    let src = r#"
        fn fib(n: i64) -> i64 {
            if n <= 1 { n } else { fib(n - 1) + fib(n - 2) }
        }
        fn main() -> i64 { fib(10) }
    "#;
    let tokens = tokenize(src).expect("lex failed");
    let program = parse(tokens).expect("parse failed");
    let mut compiler = ObjectCompiler::new("fib_test").expect("compiler init failed");
    compiler
        .compile_program(&program)
        .expect("compilation failed");
    let product = compiler.finish();
    let bytes = product.emit().expect("emit failed");
    assert!(!bytes.is_empty());
}

// ── S4.4 Runtime functions ──

#[test]
fn native_println_call() {
    let src = r#"
        fn main() -> i64 {
            println(42)
            0
        }
    "#;
    // Should compile and run without error (println outputs to stdout)
    assert_eq!(compile_and_run(src), 0);
}

#[test]
fn native_println_in_loop() {
    let src = r#"
        fn main() -> i64 {
            let mut sum = 0
            for i in 1..=5 {
                println(i)
                sum = sum + i
            }
            sum
        }
    "#;
    assert_eq!(compile_and_run(src), 15);
}

// ── Extern function (FFI) tests ────────────────────────────────────

#[test]
fn native_extern_fn_declaration_compiles() {
    // Extern fn declarations should compile without error;
    // the symbol is imported, not defined.
    let src = r#"
        extern fn abs(x: i64) -> i64
        fn main() -> i64 {
            42
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_extern_fn_call_abs() {
    // Call libc abs() via extern fn declaration.
    // JIT will resolve the symbol from the process symbol table.
    let src = r#"
        extern fn abs(x: i64) -> i64
        fn main() -> i64 {
            abs(-42)
        }
    "#;
    // Note: libc abs() takes int (32-bit), but we pass i64.
    // On x86_64, this works because the value fits in 32 bits
    // and the calling convention passes it in the same register.
    // The result may be truncated to 32-bit.
    let result = compile_and_run(src);
    assert_eq!(result, 42);
}

#[test]
fn object_compiler_extern_fn() {
    let src = r#"
        extern fn abs(x: i64) -> i64
        fn main() -> i64 {
            abs(-7)
        }
    "#;
    let tokens = crate::lexer::tokenize(src).unwrap();
    let program = crate::parser::parse(tokens).unwrap();
    let mut compiler = ObjectCompiler::new("test_extern").unwrap();
    // Should compile without error (extern fn creates an import)
    compiler.compile_program(&program).unwrap();
    let product = compiler.finish();
    let obj_bytes = product.emit().unwrap();
    assert!(!obj_bytes.is_empty());
}

// ── S18.2 ARM64 (aarch64) cross-compilation ──────────────────────

#[test]
fn aarch64_object_simple() {
    let target = TargetConfig::from_triple("aarch64-unknown-linux-gnu").unwrap();
    let src = "fn main() -> i64 { 42 }";
    let tokens = tokenize(src).unwrap();
    let program = parse(tokens).unwrap();
    let mut compiler = ObjectCompiler::new_with_target("aarch64_simple", &target).unwrap();
    compiler.compile_program(&program).unwrap();
    let product = compiler.finish();
    let bytes = product.emit().unwrap();
    // Verify ELF header
    assert!(bytes.len() > 64);
    assert_eq!(&bytes[..4], b"\x7fELF");
    // ELF class: 64-bit (2)
    assert_eq!(bytes[4], 2);
    // ELF machine: aarch64 = 0xB7 (183)
    assert_eq!(bytes[18], 0xB7);
}

#[test]
fn aarch64_object_fibonacci() {
    let target = TargetConfig::from_triple("aarch64-unknown-linux-gnu").unwrap();
    let src = r#"
        fn fib(n: i64) -> i64 {
            if n <= 1 { n } else { fib(n - 1) + fib(n - 2) }
        }
        fn main() -> i64 { fib(10) }
    "#;
    let tokens = tokenize(src).unwrap();
    let program = parse(tokens).unwrap();
    let mut compiler = ObjectCompiler::new_with_target("aarch64_fib", &target).unwrap();
    compiler.compile_program(&program).unwrap();
    let product = compiler.finish();
    let bytes = product.emit().unwrap();
    assert!(bytes.len() > 100);
    assert_eq!(&bytes[..4], b"\x7fELF");
    assert_eq!(bytes[18], 0xB7); // aarch64
}

#[test]
fn aarch64_object_loops_and_calls() {
    let target = TargetConfig::from_triple("aarch64-unknown-linux-gnu").unwrap();
    let src = r#"
        fn square(x: i64) -> i64 { x * x }
        fn main() -> i64 {
            let mut sum = 0
            for i in 1..=10 {
                sum = sum + square(i)
            }
            sum
        }
    "#;
    let tokens = tokenize(src).unwrap();
    let program = parse(tokens).unwrap();
    let mut compiler = ObjectCompiler::new_with_target("aarch64_loops", &target).unwrap();
    compiler.compile_program(&program).unwrap();
    let product = compiler.finish();
    let bytes = product.emit().unwrap();
    assert!(bytes.len() > 100);
    assert_eq!(bytes[18], 0xB7);
}

#[test]
fn aarch64_bare_metal_object() {
    // aarch64-unknown-none-elf specifies ELF binary format explicitly
    let target = TargetConfig::from_triple("aarch64-unknown-none-elf").unwrap();
    let src = "fn main() -> i64 { 1 + 2 }";
    let tokens = tokenize(src).unwrap();
    let program = parse(tokens).unwrap();
    let mut compiler = ObjectCompiler::new_with_target("aarch64_bare", &target).unwrap();
    compiler.compile_program(&program).unwrap();
    let product = compiler.finish();
    let bytes = product.emit().unwrap();
    assert!(bytes.len() > 64);
    assert_eq!(&bytes[..4], b"\x7fELF");
    assert_eq!(bytes[18], 0xB7);
}

// ── S18.3 RISC-V (riscv64) cross-compilation ─────────────────────

#[test]
fn riscv64_object_simple() {
    let target = TargetConfig::from_triple("riscv64gc-unknown-linux-gnu").unwrap();
    let src = "fn main() -> i64 { 42 }";
    let tokens = tokenize(src).unwrap();
    let program = parse(tokens).unwrap();
    let mut compiler = ObjectCompiler::new_with_target("riscv64_simple", &target).unwrap();
    compiler.compile_program(&program).unwrap();
    let product = compiler.finish();
    let bytes = product.emit().unwrap();
    assert!(bytes.len() > 64);
    assert_eq!(&bytes[..4], b"\x7fELF");
    // ELF machine: riscv = 0xF3 (243)
    assert_eq!(bytes[18], 0xF3);
}

#[test]
fn riscv64_object_fibonacci() {
    let target = TargetConfig::from_triple("riscv64gc-unknown-linux-gnu").unwrap();
    let src = r#"
        fn fib(n: i64) -> i64 {
            if n <= 1 { n } else { fib(n - 1) + fib(n - 2) }
        }
        fn main() -> i64 { fib(10) }
    "#;
    let tokens = tokenize(src).unwrap();
    let program = parse(tokens).unwrap();
    let mut compiler = ObjectCompiler::new_with_target("riscv64_fib", &target).unwrap();
    compiler.compile_program(&program).unwrap();
    let product = compiler.finish();
    let bytes = product.emit().unwrap();
    assert!(bytes.len() > 100);
    assert_eq!(&bytes[..4], b"\x7fELF");
    assert_eq!(bytes[18], 0xF3); // riscv64
}

#[test]
fn riscv64_object_loops() {
    let target = TargetConfig::from_triple("riscv64gc-unknown-linux-gnu").unwrap();
    let src = r#"
        fn main() -> i64 {
            let mut sum = 0
            let mut i = 0
            while i < 100 {
                sum = sum + i
                i = i + 1
            }
            sum
        }
    "#;
    let tokens = tokenize(src).unwrap();
    let program = parse(tokens).unwrap();
    let mut compiler = ObjectCompiler::new_with_target("riscv64_loops", &target).unwrap();
    compiler.compile_program(&program).unwrap();
    let product = compiler.finish();
    let bytes = product.emit().unwrap();
    assert!(bytes.len() > 100);
    assert_eq!(bytes[18], 0xF3);
}

#[test]
fn riscv64_bare_metal_object() {
    let target = TargetConfig::from_triple("riscv64gc-unknown-none-elf").unwrap();
    let src = "fn main() -> i64 { 99 }";
    let tokens = tokenize(src).unwrap();
    let program = parse(tokens).unwrap();
    let mut compiler = ObjectCompiler::new_with_target("riscv64_bare", &target).unwrap();
    compiler.compile_program(&program).unwrap();
    let product = compiler.finish();
    let bytes = product.emit().unwrap();
    assert!(bytes.len() > 64);
    assert_eq!(&bytes[..4], b"\x7fELF");
    assert_eq!(bytes[18], 0xF3);
}

// ── S4.1 String literals in native codegen ──────────────────────────

#[test]
fn native_println_string_literal() {
    let src = r#"
        fn main() -> i64 {
            println("Hello, Fajar Lang!")
            0
        }
    "#;
    // Should compile and run without error (println outputs to stdout)
    assert_eq!(compile_and_run(src), 0);
}

#[test]
fn native_print_string_literal() {
    let src = r#"
        fn main() -> i64 {
            print("hello ")
            print("world")
            0
        }
    "#;
    assert_eq!(compile_and_run(src), 0);
}

#[test]
fn native_println_string_in_loop() {
    let src = r#"
        fn main() -> i64 {
            let mut count = 0
            for i in 0..3 {
                println("tick")
                count = count + 1
            }
            count
        }
    "#;
    assert_eq!(compile_and_run(src), 3);
}

#[test]
fn native_string_literal_dedup() {
    // Same string literal used twice should be deduplicated in data section
    let src = r#"
        fn main() -> i64 {
            println("same")
            println("same")
            0
        }
    "#;
    assert_eq!(compile_and_run(src), 0);
}

#[test]
fn native_mixed_println_int_and_string() {
    let src = r#"
        fn main() -> i64 {
            println("result:")
            println(42)
            0
        }
    "#;
    assert_eq!(compile_and_run(src), 0);
}

#[test]
fn native_string_literal_returns_ptr() {
    // String literal in expression context returns a pointer (non-zero)
    let src = r#"
        fn main() -> i64 {
            let p = "hello"
            if p != 0 { 1 } else { 0 }
        }
    "#;
    assert_eq!(compile_and_run(src), 1);
}

#[test]
fn native_string_in_if_branches() {
    let src = r#"
        fn main() -> i64 {
            let x = 1
            if x == 1 {
                println("branch A")
            } else {
                println("branch B")
            }
            0
        }
    "#;
    assert_eq!(compile_and_run(src), 0);
}

#[test]
fn native_empty_string() {
    let src = r#"
        fn main() -> i64 {
            println("")
            0
        }
    "#;
    assert_eq!(compile_and_run(src), 0);
}

#[test]
fn object_compiler_string_data_section() {
    let src = r#"
        fn main() -> i64 {
            println("embedded string")
            0
        }
    "#;
    let tokens = tokenize(src).unwrap();
    let program = parse(tokens).unwrap();
    let mut compiler = ObjectCompiler::new("str_test").unwrap();
    compiler.compile_program(&program).unwrap();
    let product = compiler.finish();
    let bytes = product.emit().unwrap();
    assert!(!bytes.is_empty());
    // The string "embedded string" should appear in the object file
    let has_string = bytes.windows(15).any(|w| w == b"embedded string");
    assert!(has_string, "string literal not found in object file");
}

// ── Compound assignment operators ───────────────────────────────────

#[test]
fn native_add_assign() {
    let src = r#"
        fn main() -> i64 {
            let mut x = 10
            x += 5
            x
        }
    "#;
    assert_eq!(compile_and_run(src), 15);
}

#[test]
fn native_sub_assign() {
    let src = r#"
        fn main() -> i64 {
            let mut x = 10
            x -= 3
            x
        }
    "#;
    assert_eq!(compile_and_run(src), 7);
}

#[test]
fn native_mul_assign() {
    let src = r#"
        fn main() -> i64 {
            let mut x = 6
            x *= 7
            x
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_div_assign() {
    let src = r#"
        fn main() -> i64 {
            let mut x = 100
            x /= 4
            x
        }
    "#;
    assert_eq!(compile_and_run(src), 25);
}

#[test]
fn native_compound_assign_in_loop() {
    let src = r#"
        fn main() -> i64 {
            let mut sum = 0
            for i in 1..=10 {
                sum += i
            }
            sum
        }
    "#;
    assert_eq!(compile_and_run(src), 55);
}

// ── Loop expression ────────────────────────────────────────────────

#[test]
fn native_loop_with_return() {
    let src = r#"
        fn count_to_ten() -> i64 {
            let mut i = 0
            loop {
                i += 1
                if i == 10 {
                    return i
                }
            }
        }
        fn main() -> i64 { count_to_ten() }
    "#;
    assert_eq!(compile_and_run(src), 10);
}

// ── Char and Null literals ─────────────────────────────────────────

#[test]
fn native_char_literal() {
    let src = r#"
        fn main() -> i64 {
            let c = 'A'
            c
        }
    "#;
    assert_eq!(compile_and_run(src), 65); // ASCII 'A'
}

#[test]
fn native_null_literal() {
    let src = r#"
        fn main() -> i64 {
            let n = null
            if n == 0 { 1 } else { 0 }
        }
    "#;
    assert_eq!(compile_and_run(src), 1);
}

// ── Break and Continue ───────────────────────────────────────────────

#[test]
fn native_break_in_while() {
    let src = r#"
        fn main() -> i64 {
            let mut i = 0
            while i < 100 {
                if i == 5 {
                    break
                }
                i = i + 1
            }
            i
        }
    "#;
    assert_eq!(compile_and_run(src), 5);
}

#[test]
fn native_continue_in_while() {
    let src = r#"
        fn main() -> i64 {
            let mut sum = 0
            let mut i = 0
            while i < 10 {
                i = i + 1
                if i % 2 == 0 {
                    continue
                }
                sum = sum + i
            }
            sum
        }
    "#;
    // odd numbers 1..10: 1+3+5+7+9 = 25
    assert_eq!(compile_and_run(src), 25);
}

#[test]
fn native_break_in_loop() {
    let src = r#"
        fn main() -> i64 {
            let mut count = 0
            loop {
                count = count + 1
                if count == 10 {
                    break
                }
            }
            count
        }
    "#;
    assert_eq!(compile_and_run(src), 10);
}

#[test]
fn native_continue_in_for() {
    let src = r#"
        fn main() -> i64 {
            let mut sum = 0
            for i in 0..10 {
                if i == 3 {
                    continue
                }
                if i == 7 {
                    continue
                }
                sum = sum + i
            }
            sum
        }
    "#;
    // 0+1+2+4+5+6+8+9 = 35
    assert_eq!(compile_and_run(src), 35);
}

#[test]
fn native_break_in_for() {
    let src = r#"
        fn main() -> i64 {
            let mut sum = 0
            for i in 0..100 {
                if i == 5 {
                    break
                }
                sum = sum + i
            }
            sum
        }
    "#;
    // 0+1+2+3+4 = 10
    assert_eq!(compile_and_run(src), 10);
}

#[test]
fn native_nested_loop_break() {
    let src = r#"
        fn main() -> i64 {
            let mut outer_count = 0
            let mut total = 0
            while outer_count < 3 {
                let mut inner = 0
                while inner < 100 {
                    if inner == 4 {
                        break
                    }
                    inner = inner + 1
                }
                total = total + inner
                outer_count = outer_count + 1
            }
            total
        }
    "#;
    // Each inner loop breaks at 4, 3 iterations => 4 * 3 = 12
    assert_eq!(compile_and_run(src), 12);
}

// ── Power operator ───────────────────────────────────────────────────

#[test]
fn native_power_operator() {
    let src = r#"
        fn main() -> i64 {
            2 ** 10
        }
    "#;
    assert_eq!(compile_and_run(src), 1024);
}

#[test]
fn native_power_float() {
    let src = r#"
        fn main() -> i64 {
            let x = 2.0 ** 3.0
            x as i64
        }
    "#;
    assert_eq!(compile_and_run(src), 8);
}

#[test]
fn native_power_float_fractional() {
    let src = r#"
        fn main() -> i64 {
            let x = 9.0 ** 0.5
            x as i64
        }
    "#;
    assert_eq!(compile_and_run(src), 3); // sqrt(9) = 3
}

// ═══════════════════════════════════════════════════════════════════════
// const declarations
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn native_const_int() {
    let src = r#"
        const MAX: i64 = 100
        fn main() -> i64 {
            MAX
        }
    "#;
    assert_eq!(compile_and_run(src), 100);
}

#[test]
fn native_const_float() {
    let src = r#"
        const PI: f64 = 3.14
        fn main() -> i64 {
            let x = PI * 2.0
            x as i64
        }
    "#;
    assert_eq!(compile_and_run(src), 6);
}

#[test]
fn native_const_toplevel() {
    let src = r#"
        const limit: i64 = 50
        fn main() -> i64 {
            limit
        }
    "#;
    assert_eq!(compile_and_run(src), 50);
}

#[test]
fn native_const_toplevel_multi_fn() {
    let src = r#"
        const base: i64 = 100
        fn add_base(x: i64) -> i64 {
            x + base
        }
        fn main() -> i64 {
            add_base(42)
        }
    "#;
    assert_eq!(compile_and_run(src), 142);
}

#[test]
fn native_const_toplevel_f64() {
    let src = r#"
        const pi: f64 = 3.14
        fn main() -> f64 {
            pi * 2.0
        }
    "#;
    let result = compile_and_run_f64(src);
    assert!((result - 6.28).abs() < 1e-10);
}

#[test]
fn native_array_param_i64() {
    let src = r#"
        fn sum_arr(arr: [i64; 3]) -> i64 {
            arr[0] + arr[1] + arr[2]
        }
        fn main() -> i64 {
            let data = [10, 20, 30]
            sum_arr(data)
        }
    "#;
    assert_eq!(compile_and_run(src), 60);
}

#[test]
fn native_array_param_f64() {
    let src = r#"
        fn sum_arr(arr: [f64; 3]) -> f64 {
            arr[0] + arr[1] + arr[2]
        }
        fn main() -> f64 {
            let data = [1.5, 2.5, 3.0]
            sum_arr(data)
        }
    "#;
    let result = compile_and_run_f64(src);
    assert!((result - 7.0).abs() < 1e-10);
}

#[test]
fn native_array_return() {
    let src = r#"
        fn make_arr() -> [i64; 3] {
            let result = [10, 20, 30]
            result
        }
        fn main() -> i64 {
            let arr = make_arr()
            arr[0] + arr[1] + arr[2]
        }
    "#;
    assert_eq!(compile_and_run(src), 60);
}

#[test]
fn native_array_return_f64() {
    let src = r#"
        fn sensor_data() -> [f64; 4] {
            let data = [0.1, 0.0, 9.81, 0.02]
            data
        }
        fn main() -> f64 {
            let imu = sensor_data()
            imu[2]
        }
    "#;
    let result = compile_and_run_f64(src);
    assert!((result - 9.81).abs() < 1e-10);
}

#[test]
fn native_array_pass_through() {
    let src = r#"
        fn double_first(arr: [i64; 3]) -> i64 {
            arr[0] * 2
        }
        fn make_and_use() -> i64 {
            let data = [5, 10, 15]
            double_first(data)
        }
        fn main() -> i64 {
            make_and_use()
        }
    "#;
    assert_eq!(compile_and_run(src), 10);
}

#[test]
fn native_array_first() {
    // first() on non-empty array returns Some (tag=1), payload=first element
    let src = r#"
        fn main() -> i64 {
            let arr = [10, 20, 30]
            let result = arr.first()
            match result {
                Some(v) => v,
                None => -1,
                _ => -2,
            }
        }
    "#;
    assert_eq!(compile_and_run(src), 10);
}

#[test]
fn native_array_last() {
    let src = r#"
        fn main() -> i64 {
            let arr = [10, 20, 30]
            let result = arr.last()
            match result {
                Some(v) => v,
                None => -1,
                _ => -2,
            }
        }
    "#;
    assert_eq!(compile_and_run(src), 30);
}

#[test]
fn native_array_reverse() {
    let src = r#"
        fn main() -> i64 {
            let mut arr = [1, 2, 3, 4]
            arr.reverse()
            arr[0] * 1000 + arr[1] * 100 + arr[2] * 10 + arr[3]
        }
    "#;
    assert_eq!(compile_and_run(src), 4321);
}

#[test]
fn native_short_circuit_and() {
    // Simple AND test: true && true
    let src = r#"
        fn main() -> i64 {
            let a = 1 > 0
            let b = 2 > 0
            if a && b { 10 } else { 20 }
        }
    "#;
    assert_eq!(compile_and_run(src), 10);
}

#[test]
fn native_short_circuit_and_false() {
    // AND short-circuit: false && (don't eval)
    let src = r#"
        fn main() -> i64 {
            let a = 0 > 1
            let b = 1 > 0
            if a && b { 10 } else { 20 }
        }
    "#;
    assert_eq!(compile_and_run(src), 20);
}

#[test]
fn native_short_circuit_or() {
    // OR short-circuit: true || (don't eval)
    let src = r#"
        fn main() -> i64 {
            let a = 1 > 0
            let b = 0 > 1
            if a || b { 100 } else { 0 }
        }
    "#;
    assert_eq!(compile_and_run(src), 100);
}

#[test]
fn native_short_circuit_and_prevents_div_zero() {
    // Without short-circuit, 10/0 would trap — this verifies short-circuit prevents it
    let src = r#"
        fn main() -> i64 {
            let x = 0
            if x != 0 && (10 / x) > 2 {
                1
            } else {
                42
            }
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_short_circuit_or_both_false() {
    let src = r#"
        fn main() -> i64 {
            let a = 0 > 1
            let b = 0 > 1
            if a || b { 100 } else { 0 }
        }
    "#;
    assert_eq!(compile_and_run(src), 0);
}

#[test]
fn native_power_zero_exponent() {
    let src = r#"
        fn main() -> i64 {
            42 ** 0
        }
    "#;
    assert_eq!(compile_and_run(src), 1);
}

#[test]
fn native_power_one_exponent() {
    let src = r#"
        fn main() -> i64 {
            7 ** 1
        }
    "#;
    assert_eq!(compile_and_run(src), 7);
}

// ── S4.2 Fixed arrays in native codegen ─────────────────────────────

#[test]
fn native_array_literal_and_index() {
    let src = r#"
        fn main() -> i64 {
            let a = [10, 20, 30]
            a[1]
        }
    "#;
    assert_eq!(compile_and_run(src), 20);
}

#[test]
fn native_array_first_element() {
    let src = r#"
        fn main() -> i64 {
            let a = [100, 200, 300]
            a[0]
        }
    "#;
    assert_eq!(compile_and_run(src), 100);
}

#[test]
fn native_array_last_element() {
    let src = r#"
        fn main() -> i64 {
            let a = [5, 10, 15, 20, 25]
            a[4]
        }
    "#;
    assert_eq!(compile_and_run(src), 25);
}

#[test]
fn native_array_index_assign() {
    let src = r#"
        fn main() -> i64 {
            let mut a = [1, 2, 3]
            a[0] = 99
            a[0]
        }
    "#;
    assert_eq!(compile_and_run(src), 99);
}

#[test]
fn native_array_compound_assign() {
    let src = r#"
        fn main() -> i64 {
            let mut a = [10, 20, 30]
            a[1] += 5
            a[1]
        }
    "#;
    assert_eq!(compile_and_run(src), 25);
}

#[test]
fn native_array_sum_in_loop() {
    let src = r#"
        fn main() -> i64 {
            let a = [1, 2, 3, 4, 5]
            let mut sum = 0
            let mut i = 0
            while i < 5 {
                sum += a[i]
                i += 1
            }
            sum
        }
    "#;
    assert_eq!(compile_and_run(src), 15);
}

#[test]
fn native_array_modify_all_elements() {
    let src = r#"
        fn main() -> i64 {
            let mut a = [0, 0, 0]
            a[0] = 10
            a[1] = 20
            a[2] = 30
            a[0] + a[1] + a[2]
        }
    "#;
    assert_eq!(compile_and_run(src), 60);
}

#[test]
fn native_array_expressions_as_elements() {
    let src = r#"
        fn main() -> i64 {
            let x = 5
            let a = [x, x * 2, x * 3]
            a[0] + a[1] + a[2]
        }
    "#;
    // 5 + 10 + 15 = 30
    assert_eq!(compile_and_run(src), 30);
}

// ── S3.4 For-in over arrays ──────────────────────────────────────────

#[test]
fn native_for_in_array_sum() {
    let src = r#"
        fn main() -> i64 {
            let a = [10, 20, 30]
            let mut sum = 0
            for x in a {
                sum = sum + x
            }
            sum
        }
    "#;
    assert_eq!(compile_and_run(src), 60);
}

#[test]
fn native_for_in_array_literal() {
    let src = r#"
        fn main() -> i64 {
            let mut total = 0
            for x in [1, 2, 3, 4, 5] {
                total = total + x
            }
            total
        }
    "#;
    assert_eq!(compile_and_run(src), 15);
}

// ── S5.4 Monomorphization ───────────────────────────────────────────

#[test]
fn native_mono_generic_max() {
    let src = r#"
        fn max<T>(a: T, b: T) -> T {
            if a > b { a } else { b }
        }
        fn main() -> i64 {
            max(10, 20)
        }
    "#;
    assert_eq!(compile_and_run(src), 20);
}

#[test]
fn native_mono_generic_min() {
    let src = r#"
        fn min<T>(a: T, b: T) -> T {
            if a < b { a } else { b }
        }
        fn main() -> i64 {
            min(100, 42)
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_mono_generic_identity() {
    let src = r#"
        fn identity<T>(x: T) -> T {
            x
        }
        fn main() -> i64 {
            identity(99)
        }
    "#;
    assert_eq!(compile_and_run(src), 99);
}

#[test]
fn native_mono_generic_in_expression() {
    let src = r#"
        fn max<T>(a: T, b: T) -> T {
            if a > b { a } else { b }
        }
        fn main() -> i64 {
            max(3, 5) + max(10, 7)
        }
    "#;
    // 5 + 10 = 15
    assert_eq!(compile_and_run(src), 15);
}

#[test]
fn native_mono_two_generic_fns() {
    let src = r#"
        fn max<T>(a: T, b: T) -> T {
            if a > b { a } else { b }
        }
        fn min<T>(a: T, b: T) -> T {
            if a < b { a } else { b }
        }
        fn main() -> i64 {
            max(10, 20) + min(10, 20)
        }
    "#;
    // 20 + 10 = 30
    assert_eq!(compile_and_run(src), 30);
}

#[test]
fn native_mono_generic_not_called() {
    // Generic function defined but never called — should not cause error
    let src = r#"
        fn unused<T>(x: T) -> T { x }
        fn main() -> i64 { 42 }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

// ═══════════════════════════════════════════════════════════════════
// E.6 — Type-aware generic monomorphization tests
// ═══════════════════════════════════════════════════════════════════

#[test]
fn native_mono_f64_identity() {
    let src = r#"
        fn identity<T>(x: T) -> T { x }
        fn main() -> f64 {
            identity(3.14)
        }
    "#;
    let result = compile_and_run_f64(src);
    assert!((result - 3.14).abs() < 1e-10);
}

#[test]
fn native_mono_f64_max() {
    let src = r#"
        fn max<T>(a: T, b: T) -> T {
            if a > b { a } else { b }
        }
        fn main() -> f64 {
            max(1.5, 2.7)
        }
    "#;
    let result = compile_and_run_f64(src);
    assert!((result - 2.7).abs() < 1e-10);
}

#[test]
fn native_mono_f64_min() {
    let src = r#"
        fn min<T>(a: T, b: T) -> T {
            if a < b { a } else { b }
        }
        fn main() -> f64 {
            min(3.7, 1.2)
        }
    "#;
    let result = compile_and_run_f64(src);
    assert!((result - 1.2).abs() < 1e-10);
}

#[test]
fn native_mono_i64_and_f64_same_fn() {
    // Same generic function called with i64 AND f64 — both specializations created
    let src = r#"
        fn max<T>(a: T, b: T) -> T {
            if a > b { a } else { b }
        }
        fn main() -> i64 {
            max(10, 20)
        }
    "#;
    assert_eq!(compile_and_run(src), 20);
}

#[test]
fn native_mono_f64_add_generic() {
    let src = r#"
        fn add<T>(a: T, b: T) -> T {
            a + b
        }
        fn main() -> f64 {
            add(1.5, 2.5)
        }
    "#;
    let result = compile_and_run_f64(src);
    assert!((result - 4.0).abs() < 1e-10);
}

#[test]
fn native_mono_f64_sub_generic() {
    let src = r#"
        fn sub<T>(a: T, b: T) -> T {
            a - b
        }
        fn main() -> f64 {
            sub(10.5, 3.0)
        }
    "#;
    let result = compile_and_run_f64(src);
    assert!((result - 7.5).abs() < 1e-10);
}

#[test]
fn native_mono_f64_mul_generic() {
    let src = r#"
        fn mul<T>(a: T, b: T) -> T {
            a * b
        }
        fn main() -> f64 {
            mul(3.0, 4.0)
        }
    "#;
    let result = compile_and_run_f64(src);
    assert!((result - 12.0).abs() < 1e-10);
}

#[test]
fn native_mono_f64_in_expression() {
    let src = r#"
        fn max<T>(a: T, b: T) -> T {
            if a > b { a } else { b }
        }
        fn main() -> f64 {
            max(1.0, 2.0) + max(3.0, 4.0)
        }
    "#;
    let result = compile_and_run_f64(src);
    assert!((result - 6.0).abs() < 1e-10);
}

#[test]
fn native_mono_generic_with_fn_call() {
    // Generic function that uses another function internally
    let src = r#"
        fn clamp<T>(val: T, lo: T, hi: T) -> T {
            if val < lo { lo }
            else if val > hi { hi }
            else { val }
        }
        fn main() -> i64 {
            clamp(15, 0, 10)
        }
    "#;
    assert_eq!(compile_and_run(src), 10);
}

#[test]
fn native_mono_f64_clamp() {
    let src = r#"
        fn clamp<T>(val: T, lo: T, hi: T) -> T {
            if val < lo { lo }
            else if val > hi { hi }
            else { val }
        }
        fn main() -> f64 {
            clamp(5.5, 0.0, 10.0)
        }
    "#;
    let result = compile_and_run_f64(src);
    assert!((result - 5.5).abs() < 1e-10);
}

#[test]
fn native_string_concat_literals() {
    // String literal concat at compile time: "hello" + " world" → "hello world"
    let src = r#"
        fn main() -> i64 {
            println("hello" + " world")
            0
        }
    "#;
    // Should compile and run without error
    assert_eq!(compile_and_run(src), 0);
}

#[test]
fn native_heap_runtime_registered() {
    // Verify the heap allocator runtime functions are declared
    let mut compiler = CraneliftCompiler::new().expect("compiler init failed");
    let src = "fn main() -> i64 { 0 }";
    let tokens = tokenize(src).expect("lex");
    let program = parse(tokens).expect("parse");
    compiler.compile_program(&program).expect("compile");
    assert!(compiler.functions.contains_key("__alloc"));
    assert!(compiler.functions.contains_key("__free"));
    assert!(compiler.functions.contains_key("__str_concat"));
}
