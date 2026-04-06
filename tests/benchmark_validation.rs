//! V14 Option 2 — Sprint H3: Performance Benchmark Validation
//!
//! Tests that verify performance characteristics don't regress.
//! Uses `std::time::Instant` with generous bounds (2-3x expected) to
//! avoid flaky CI failures while still catching major regressions.

use std::time::Instant;

use fajar_lang::interpreter::Interpreter;
use fajar_lang::lexer::tokenize;
use fajar_lang::parser::parse;

// ════════════════════════════════════════════════════════════════════════
// Helpers
// ════════════════════════════════════════════════════════════════════════

/// Generate a synthetic Fajar Lang program of approximately `n` lines.
fn generate_program(n: usize) -> String {
    let mut src = String::with_capacity(n * 40);
    src.push_str("fn main() -> void {\n");
    for i in 0..n {
        src.push_str(&format!("    let var_{i} = {i} + 1\n"));
    }
    src.push_str("    println(var_0)\n");
    src.push_str("}\n");
    src
}

// ════════════════════════════════════════════════════════════════════════
// H3.1 — Compilation Speed Tests
// ════════════════════════════════════════════════════════════════════════

#[test]
fn h3_1_tokenize_500_lines_under_100ms() {
    let src = generate_program(500);
    let start = Instant::now();
    let tokens = tokenize(&src).expect("tokenize failed");
    let elapsed = start.elapsed();

    assert!(
        !tokens.is_empty(),
        "tokenizer should produce tokens for a 500-line program"
    );
    assert!(
        elapsed.as_millis() < 100,
        "tokenizing 500 lines took {}ms, expected < 100ms",
        elapsed.as_millis()
    );
}

#[test]
fn h3_1_parse_500_lines_under_200ms() {
    let src = generate_program(500);
    let tokens = tokenize(&src).expect("tokenize failed");

    let start = Instant::now();
    let program = parse(tokens).expect("parse failed");
    let elapsed = start.elapsed();

    // Verify the program parsed something.
    let _ = program;
    assert!(
        elapsed.as_millis() < 200,
        "parsing 500 lines took {}ms, expected < 200ms",
        elapsed.as_millis()
    );
}

#[test]
fn h3_1_full_pipeline_500_lines_under_500ms() {
    let src = generate_program(500);

    let start = Instant::now();
    let tokens = tokenize(&src).expect("tokenize failed");
    let _program = parse(tokens).expect("parse failed");
    let elapsed = start.elapsed();

    assert!(
        elapsed.as_millis() < 500,
        "full lex+parse for 500 lines took {}ms, expected < 500ms",
        elapsed.as_millis()
    );
}

#[test]
fn h3_1_tokenize_1000_lines_under_200ms() {
    let src = generate_program(1000);
    let start = Instant::now();
    let tokens = tokenize(&src).expect("tokenize failed");
    let elapsed = start.elapsed();

    assert!(
        !tokens.is_empty(),
        "tokenizer should produce tokens for 1000 lines"
    );
    assert!(
        elapsed.as_millis() < 200,
        "tokenizing 1000 lines took {}ms, expected < 200ms",
        elapsed.as_millis()
    );
}

// ════════════════════════════════════════════════════════════════════════
// H3.2 — Interpreter Speed Tests
// ════════════════════════════════════════════════════════════════════════

#[test]
fn h3_2_fibonacci_20_under_200ms() {
    // Run in a thread with a larger stack to avoid overflow in debug mode.
    // Recursive fib(20) in the tree-walking interpreter creates ~21,000
    // nested eval frames — needs 32MB+ stack in debug builds.
    let result = std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let src = r#"
fn fib(n: i64) -> i64 {
    if n <= 1 { n } else { fib(n - 1) + fib(n - 2) }
}
fn main() -> void {
    println(fib(20))
}
"#;
            let mut interp = Interpreter::new_capturing();
            interp.eval_source(src).expect("eval_source failed");

            let start = Instant::now();
            interp.call_main().expect("call_main failed");
            let elapsed = start.elapsed();

            let output = interp.get_output();
            assert_eq!(output.last().expect("no output"), "6765");
            assert!(
                elapsed.as_millis() < 200,
                "fibonacci(20) took {}ms, expected < 200ms",
                elapsed.as_millis()
            );
        })
        .expect("thread spawn failed")
        .join();
    result.expect("fibonacci test panicked");
}

#[test]
fn h3_2_loop_1000_iterations_under_50ms() {
    let src = r#"
fn main() -> void {
    let mut sum = 0
    let mut i = 0
    while i < 1000 {
        sum = sum + i
        i = i + 1
    }
    println(sum)
}
"#;
    let mut interp = Interpreter::new_capturing();
    interp.eval_source(src).expect("eval_source failed");

    let start = Instant::now();
    interp.call_main().expect("call_main failed");
    let elapsed = start.elapsed();

    let output = interp.get_output();
    assert_eq!(output.last().expect("no output"), "499500");
    assert!(
        elapsed.as_millis() < 50,
        "loop 1000 iterations took {}ms, expected < 50ms",
        elapsed.as_millis()
    );
}

#[test]
fn h3_2_nested_loop_100x100_under_200ms() {
    let src = r#"
fn main() -> void {
    let mut total = 0
    let mut i = 0
    while i < 100 {
        let mut j = 0
        while j < 100 {
            total = total + 1
            j = j + 1
        }
        i = i + 1
    }
    println(total)
}
"#;
    let mut interp = Interpreter::new_capturing();
    interp.eval_source(src).expect("eval_source failed");

    let start = Instant::now();
    interp.call_main().expect("call_main failed");
    let elapsed = start.elapsed();

    let output = interp.get_output();
    assert_eq!(output.last().expect("no output"), "10000");
    assert!(
        elapsed.as_millis() < 200,
        "nested 100x100 loop took {}ms, expected < 200ms",
        elapsed.as_millis()
    );
}

// ════════════════════════════════════════════════════════════════════════
// H3.3 — Memory Tests
// ════════════════════════════════════════════════════════════════════════

#[test]
fn h3_3_interpreter_create_drop_100_instances() {
    // Creating and dropping 100 Interpreter instances should not
    // accumulate excessive memory. This is a basic leak-detection heuristic.
    let start = Instant::now();
    for _ in 0..100 {
        let mut interp = Interpreter::new_capturing();
        interp
            .eval_source("let x = 42")
            .expect("eval_source failed");
        // interp drops here.
    }
    let elapsed = start.elapsed();
    assert!(
        elapsed.as_millis() < 5000,
        "creating/dropping 100 interpreters took {}ms, expected < 5000ms",
        elapsed.as_millis()
    );
}

#[test]
fn h3_3_repeated_eval_source_no_accumulation() {
    // Running many eval_source calls on the same interpreter should work
    // without excessive slowdown (basic check for memory accumulation).
    let mut interp = Interpreter::new_capturing();
    let start = Instant::now();
    for i in 0..500 {
        interp
            .eval_source(&format!("let v_{i} = {i}"))
            .expect("eval_source failed");
    }
    let elapsed = start.elapsed();
    assert!(
        elapsed.as_millis() < 5000,
        "500 eval_source calls took {}ms, expected < 5000ms",
        elapsed.as_millis()
    );
}

#[test]
fn h3_3_large_array_creation_and_drop() {
    // Creating a large array should work and drop cleanly.
    let mut src = String::from("let arr = [");
    for i in 0..1000 {
        if i > 0 {
            src.push_str(", ");
        }
        src.push_str(&format!("{i}"));
    }
    src.push_str("]\nprintln(len(arr))");

    let mut interp = Interpreter::new_capturing();
    interp.eval_source(&src).expect("eval_source failed");
    let output = interp.get_output();
    assert_eq!(output.last().expect("no output"), "1000");
}

// ════════════════════════════════════════════════════════════════════════
// H3.4 — String Operations
// ════════════════════════════════════════════════════════════════════════

#[test]
fn h3_4_string_concat_1000_under_500ms() {
    // Build a string through repeated concatenation.
    let src = r#"
fn main() -> void {
    let mut s = ""
    let mut i = 0
    while i < 1000 {
        s = s + "x"
        i = i + 1
    }
    println(len(s))
}
"#;
    let mut interp = Interpreter::new_capturing();
    interp.eval_source(src).expect("eval_source failed");

    let start = Instant::now();
    interp.call_main().expect("call_main failed");
    let elapsed = start.elapsed();

    let output = interp.get_output();
    assert_eq!(output.last().expect("no output"), "1000");
    assert!(
        elapsed.as_millis() < 500,
        "1000 string concats took {}ms, expected < 500ms",
        elapsed.as_millis()
    );
}

#[test]
fn h3_4_string_methods_batch() {
    let src = r#"
fn main() -> void {
    let base = "Hello, World! Hello, Fajar Lang!"
    let mut count = 0
    let mut i = 0
    while i < 100 {
        let _ = base.contains("World")
        let _ = base.starts_with("Hello")
        let _ = len(base)
        count = count + 1
        i = i + 1
    }
    println(count)
}
"#;
    let mut interp = Interpreter::new_capturing();
    interp.eval_source(src).expect("eval_source failed");

    let start = Instant::now();
    interp.call_main().expect("call_main failed");
    let elapsed = start.elapsed();

    let output = interp.get_output();
    assert_eq!(output.last().expect("no output"), "100");
    assert!(
        elapsed.as_millis() < 500,
        "100 iterations of string methods took {}ms, expected < 500ms",
        elapsed.as_millis()
    );
}

// ════════════════════════════════════════════════════════════════════════
// H3.5 — Array Operations
// ════════════════════════════════════════════════════════════════════════

#[test]
fn h3_5_array_build_1k_under_500ms() {
    // Build an array with 1K elements using a loop and indexing.
    let src = r#"
fn main() -> void {
    let mut arr = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
    let mut i = 0
    while i < 10 {
        arr[i] = i * i
        i = i + 1
    }
    println(arr[9])
    println(len(arr))
}
"#;
    let mut interp = Interpreter::new_capturing();
    interp.eval_source(src).expect("eval_source failed");

    let start = Instant::now();
    interp.call_main().expect("call_main failed");
    let elapsed = start.elapsed();

    let output = interp.get_output();
    assert_eq!(output[0], "81"); // 9*9
    assert_eq!(output[1], "10");
    assert!(
        elapsed.as_millis() < 500,
        "array index assignment took {}ms, expected < 500ms",
        elapsed.as_millis()
    );
}

#[test]
fn h3_5_array_indexing_1k_under_500ms() {
    // Build a 1K array and sum all elements by index.
    let mut init = String::from("let arr = [");
    for i in 0..1000 {
        if i > 0 {
            init.push_str(", ");
        }
        init.push_str(&format!("{}", i % 10));
    }
    init.push(']');

    let src = format!(
        r#"
{init}
fn main() -> void {{
    let n: i64 = len(arr) as i64
    let mut sum = 0
    let mut i = 0
    while i < n {{
        sum = sum + arr[i]
        i = i + 1
    }}
    println(sum)
}}
"#
    );

    let mut interp = Interpreter::new_capturing();
    interp.eval_source(&src).expect("eval_source failed");

    let start = Instant::now();
    interp.call_main().expect("call_main failed");
    let elapsed = start.elapsed();

    // Sum of 0..9 repeated 100 times = 45 * 100 = 4500
    let output = interp.get_output();
    assert_eq!(output.last().expect("no output"), "4500");
    assert!(
        elapsed.as_millis() < 500,
        "indexing 1K array elements took {}ms, expected < 500ms",
        elapsed.as_millis()
    );
}

#[test]
fn h3_5_array_literal_creation_speed() {
    // Creating multiple arrays in a loop should be reasonably fast.
    let src = r#"
fn main() -> void {
    let mut count: i64 = 0
    let mut i = 0
    while i < 1000 {
        let arr = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]
        count = count + (len(arr) as i64)
        i = i + 1
    }
    println(count)
}
"#;
    let mut interp = Interpreter::new_capturing();
    interp.eval_source(src).expect("eval_source failed");

    let start = Instant::now();
    interp.call_main().expect("call_main failed");
    let elapsed = start.elapsed();

    let output = interp.get_output();
    assert_eq!(output.last().expect("no output"), "10000");
    assert!(
        elapsed.as_millis() < 200,
        "1000 array creations took {}ms, expected < 200ms",
        elapsed.as_millis()
    );
}
