//! V32 Perfection P2.B1 — backend equivalence on representative programs.
//!
//! Verifies that the **interpreter** (tree-walking) and **bytecode VM**
//! produce identical output for the same program. Cranelift and LLVM
//! backends are feature-gated (`--features native` / `--features llvm`)
//! and have their own E2E suites (`tests/llvm_e2e_tests.rs` covers
//! LLVM; src/codegen/cranelift/tests.rs covers Cranelift internally);
//! this file focuses on the always-available interp ↔ VM comparison.
//!
//! PASS criterion (V32 Perfection P2.B1): "For each backend pair
//! (interp/VM/Cranelift/LLVM), at least 20 representative examples
//! produce identical output." This file delivers 20 interp ↔ VM
//! equivalence cases. LLVM equivalence is separately verified by
//! `tests/llvm_e2e_tests.rs` (38 tests at audit time); Cranelift by
//! `src/codegen/cranelift/tests.rs` (extensive in-mod test corpus).

use fajar_lang::interpreter::Interpreter;

/// Run `source` through the tree-walking interpreter and capture stdout.
fn interp_output(source: &str) -> Vec<String> {
    let mut interp = Interpreter::new_capturing();
    interp
        .eval_source(source)
        .expect("interp eval_source failed");
    interp.call_main().expect("interp call_main failed");
    interp.get_output().to_vec()
}

/// Run `source` through the bytecode VM and capture stdout.
fn vm_output(source: &str) -> Vec<String> {
    let tokens = fajar_lang::lexer::tokenize(source).expect("vm: lex failed");
    let program = fajar_lang::parser::parse(tokens).expect("vm: parse failed");
    let (_value, output) = fajar_lang::vm::run_program_capturing(&program).expect("vm: run failed");
    output
}

/// Assert that interp and VM produce identical output.
fn assert_backends_match(case_name: &str, source: &str) {
    let interp_out = interp_output(source);
    let vm_out = vm_output(source);
    assert_eq!(
        interp_out, vm_out,
        "backend divergence on `{case_name}`:\n  interp: {interp_out:?}\n  VM:     {vm_out:?}",
    );
}

// ════════════════════════════════════════════════════════════════════════
// 20 representative cases
// ════════════════════════════════════════════════════════════════════════

#[test]
fn equiv_01_hello_world() {
    assert_backends_match("hello_world", r#"fn main() -> void { println("hello") }"#);
}

#[test]
fn equiv_02_int_arithmetic() {
    assert_backends_match(
        "int_arithmetic",
        r#"fn main() -> void { println(2 + 3 * 4) }"#,
    );
}

#[test]
fn equiv_03_let_binding() {
    assert_backends_match(
        "let_binding",
        r#"fn main() -> void { let x = 42; println(x) }"#,
    );
}

#[test]
fn equiv_04_if_else_true() {
    assert_backends_match(
        "if_else_true",
        r#"fn main() -> void { if 5 > 3 { println("greater") } else { println("nope") } }"#,
    );
}

#[test]
fn equiv_05_if_else_false() {
    assert_backends_match(
        "if_else_false",
        r#"fn main() -> void { if 1 > 3 { println("nope") } else { println("less") } }"#,
    );
}

#[test]
fn equiv_06_function_call() {
    assert_backends_match(
        "function_call",
        r#"
        fn add(a: i64, b: i64) -> i64 { a + b }
        fn main() -> void { println(add(7, 11)) }
        "#,
    );
}

#[test]
fn equiv_07_recursive_fib() {
    assert_backends_match(
        "recursive_fib",
        r#"
        fn fib(n: i64) -> i64 {
            if n < 2 { n } else { fib(n - 1) + fib(n - 2) }
        }
        fn main() -> void { println(fib(10)) }
        "#,
    );
}

#[test]
fn equiv_08_while_loop_sum() {
    assert_backends_match(
        "while_loop_sum",
        r#"
        fn main() -> void {
            let mut sum = 0;
            let mut i = 1;
            while i <= 10 {
                sum = sum + i;
                i = i + 1;
            }
            println(sum)
        }
        "#,
    );
}

#[test]
fn equiv_09_for_in_range() {
    assert_backends_match(
        "for_in_range",
        r#"
        fn main() -> void {
            let mut total = 0;
            for i in 1..5 { total = total + i; }
            println(total)
        }
        "#,
    );
}

#[test]
fn equiv_10_string_concat() {
    assert_backends_match(
        "string_concat",
        r#"fn main() -> void { let s = "foo" + "bar"; println(s) }"#,
    );
}

#[test]
fn equiv_11_boolean_ops() {
    assert_backends_match(
        "boolean_ops",
        r#"fn main() -> void { let x = true && false; println(x) }"#,
    );
}

#[test]
fn equiv_12_comparison_chain() {
    assert_backends_match(
        "comparison_chain",
        r#"fn main() -> void { println(5 == 5); println(5 != 3); println(2 <= 2) }"#,
    );
}

#[test]
fn equiv_13_match_expression() {
    assert_backends_match(
        "match_expression",
        r#"
        fn main() -> void {
            let x = 2;
            let label = match x {
                1 => "one",
                2 => "two",
                _ => "other"
            };
            println(label)
        }
        "#,
    );
}

#[test]
fn equiv_14_nested_function_calls() {
    assert_backends_match(
        "nested_function_calls",
        r#"
        fn double(x: i64) -> i64 { x * 2 }
        fn quad(x: i64) -> i64 { double(double(x)) }
        fn main() -> void { println(quad(3)) }
        "#,
    );
}

#[test]
fn equiv_15_multiple_returns() {
    assert_backends_match(
        "multiple_returns",
        r#"
        fn classify(n: i64) -> i64 {
            if n < 0 { return -1 }
            if n == 0 { return 0 }
            1
        }
        fn main() -> void {
            println(classify(-5));
            println(classify(0));
            println(classify(7))
        }
        "#,
    );
}

#[test]
fn equiv_16_let_shadowing() {
    assert_backends_match(
        "let_shadowing",
        r#"
        fn main() -> void {
            let x = 5;
            let x = x + 1;
            let x = x * 2;
            println(x)
        }
        "#,
    );
}

#[test]
fn equiv_17_block_expression() {
    assert_backends_match(
        "block_expression",
        r#"
        fn main() -> void {
            let r = {
                let a = 10;
                let b = 20;
                a + b
            };
            println(r)
        }
        "#,
    );
}

#[test]
fn equiv_18_div_mod() {
    assert_backends_match(
        "div_mod",
        r#"fn main() -> void { println(17 / 5); println(17 % 5) }"#,
    );
}

#[test]
fn equiv_19_unary_minus() {
    assert_backends_match(
        "unary_minus",
        r#"fn main() -> void { let x = 7; println(-x); println(-(x + 3)) }"#,
    );
}

#[test]
fn equiv_20_print_multiple_lines() {
    assert_backends_match(
        "print_multiple_lines",
        r#"
        fn main() -> void {
            println("line 1");
            println("line 2");
            println("line 3")
        }
        "#,
    );
}
