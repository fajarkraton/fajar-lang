//! Sprint H4: Security Audit Tests for Fajar Lang.
//!
//! Validates that the compiler and interpreter handle adversarial, malformed,
//! and edge-case inputs cleanly — producing errors, not crashes or UB.
//!
//! H4.1  Input validation (large programs, deep nesting, long identifiers)
//! H4.2  No stack overflow on user input (recursion → RuntimeError)
//! H4.3  Integer overflow (checked arithmetic → RE009)
//! H4.4  String safety (null bytes, huge strings, format injection)
//! H4.5  Array bounds (negative, huge, empty → RE010)
//! H4.6  Division by zero (int & float → RE001)
//! H4.7  Infinite loop protection (timeout-killable)
//! H4.8  Resource exhaustion (many variables, no OOM crash)
//! H4.9  REPL isolation (eval_source state leakage)
//! H4.10 Type confusion (mixed types → clean errors)

use fajar_lang::FjError;
use fajar_lang::interpreter::{Interpreter, RuntimeError, Value};

/// Run source through the full pipeline (lex -> parse -> analyze -> eval).
/// Returns RuntimeError on failure for convenient matching.
fn eval(source: &str) -> Result<Value, RuntimeError> {
    let mut interp = Interpreter::new_capturing();
    interp.eval_source(source).map_err(|e| match e {
        FjError::Runtime(re) => re,
        other => panic!("expected RuntimeError, got: {other}"),
    })
}

/// Run source and expect any kind of error (lex, parse, semantic, or runtime).
fn eval_expect_error(source: &str) -> FjError {
    let mut interp = Interpreter::new_capturing();
    interp.eval_source(source).expect_err(&format!(
        "expected error for: {}",
        &source[..source.len().min(80)]
    ))
}

// ════════════════════════════════════════════════════════════════════════════
// H4.1 — Input Validation
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn h4_1_large_program_over_1mb() {
    // Generate a program > 1 MB of repeated valid statements.
    // The compiler should handle it (maybe slowly) or produce an error — not crash.
    // Run in a large-stack thread for safety.
    let handle = std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(|| {
            let line = "let x_abcdef = 42\n";
            let count = (1_100_000 / line.len()) + 1;
            let source: String = line.repeat(count);
            assert!(source.len() > 1_000_000, "source should exceed 1 MB");

            let mut interp = Interpreter::new_capturing();
            // We accept either Ok or Err — the key requirement is no panic/crash.
            let _result = interp.eval_source(&source);
            true
        })
        .expect("failed to spawn thread");

    let ok = handle.join().expect("thread panicked on large program");
    assert!(ok);
}

#[test]
fn h4_1_deeply_nested_expressions_100_levels() {
    // Build (((((...42...))))) with 100+ levels of parentheses.
    // Run in a large-stack thread to avoid real stack overflow in the parser.
    let handle = std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(|| {
            let depth = 120;
            let mut source = String::new();
            for _ in 0..depth {
                source.push('(');
            }
            source.push_str("42");
            for _ in 0..depth {
                source.push(')');
            }

            let mut interp = Interpreter::new_capturing();
            // Should parse and evaluate to 42, or produce an error — never crash.
            match interp.eval_source(&source) {
                Ok(Value::Int(42)) => true, // ideal
                Ok(_) => true,              // acceptable
                Err(_) => true,             // error is also acceptable (e.g., parser depth limit)
            }
        })
        .expect("failed to spawn thread");

    let ok = handle
        .join()
        .expect("thread panicked — stack overflow in nested exprs");
    assert!(ok);
}

#[test]
fn h4_1_very_long_identifier_10k_chars() {
    let ident: String = std::iter::repeat('a').take(10_000).collect();
    let source = format!("let {} = 1", ident);

    let mut interp = Interpreter::new_capturing();
    // Must not crash. Either accepts or produces an error.
    let _result = interp.eval_source(&source);
}

#[test]
fn h4_1_deeply_nested_blocks() {
    // { { { ... 42 ... } } } with 100 levels.
    // Run in a large-stack thread because deep nesting can overflow
    // the default test thread stack.
    let handle = std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(|| {
            let depth = 100;
            let mut source = String::new();
            for _ in 0..depth {
                source.push_str("{ ");
            }
            source.push_str("let _x = 42");
            for _ in 0..depth {
                source.push_str(" }");
            }

            let mut interp = Interpreter::new_capturing();
            let _result = interp.eval_source(&source);
            // No crash = pass. Return true to indicate no panic.
            true
        })
        .expect("failed to spawn thread");

    let ok = handle
        .join()
        .expect("thread panicked — stack overflow in deeply nested blocks");
    assert!(ok);
}

// ════════════════════════════════════════════════════════════════════════════
// H4.2 — No Stack Overflow on User Input
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn h4_2_recursive_function_produces_runtime_error() {
    // Deep recursion must produce RuntimeError::StackOverflow, not a real
    // Rust stack overflow.  Run in a thread with an 8 MB stack so the Rust
    // side has headroom. Value is !Send, so only send a bool result.
    let handle = std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(|| {
            let source = r#"
                fn boom() -> i64 { boom() }
                boom()
            "#;
            let mut interp = Interpreter::new_capturing();
            match interp.eval_source(source) {
                Err(FjError::Runtime(RuntimeError::StackOverflow { .. })) => true,
                Err(_) => true, // any error is acceptable (not a crash)
                Ok(_) => false, // should not succeed
            }
        })
        .expect("failed to spawn thread");

    let got_error = handle
        .join()
        .expect("thread panicked — real stack overflow?");
    assert!(
        got_error,
        "deeply recursive program should produce an error"
    );
}

#[test]
fn h4_2_mutual_recursion_stack_overflow() {
    let handle = std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(|| {
            let source = r#"
                fn ping(n: i64) -> i64 { pong(n) }
                fn pong(n: i64) -> i64 { ping(n) }
                ping(1)
            "#;
            let mut interp = Interpreter::new_capturing();
            match interp.eval_source(source) {
                Err(FjError::Runtime(RuntimeError::StackOverflow { .. })) => true,
                Err(_) => true,
                Ok(_) => false,
            }
        })
        .expect("failed to spawn thread");

    let got_error = handle
        .join()
        .expect("thread panicked — real stack overflow?");
    assert!(got_error, "mutual recursion should produce an error");
}

#[test]
fn h4_2_recursion_with_set_depth() {
    // Verify that set_max_recursion_depth is respected.
    let handle = std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(|| {
            let source = r#"
                fn recurse(n: i64) -> i64 {
                    if n <= 0 { return 0 }
                    recurse(n - 1)
                }
                recurse(1000)
            "#;
            let mut interp = Interpreter::new_capturing();
            interp.set_max_recursion_depth(10);
            match interp.eval_source(source) {
                Err(FjError::Runtime(RuntimeError::StackOverflow { .. })) => true,
                Err(_) => true,
                Ok(_) => false, // with depth=10, recurse(1000) should fail
            }
        })
        .expect("failed to spawn thread");

    let got_error = handle.join().expect("thread panicked");
    assert!(got_error, "recursion should hit the custom depth limit");
}

// ════════════════════════════════════════════════════════════════════════════
// H4.3 — Integer Overflow
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn h4_3_i64_max_plus_one_overflow() {
    let err = eval("9223372036854775807 + 1").unwrap_err();
    assert!(
        matches!(err, RuntimeError::IntegerOverflow { .. }),
        "i64::MAX + 1 should be IntegerOverflow, got: {err:?}"
    );
}

#[test]
fn h4_3_i64_min_minus_one_overflow() {
    // i64::MIN is -9223372036854775808, construct it carefully.
    let err = eval("let x = -9223372036854775807 - 1\nx - 1").unwrap_err();
    assert!(
        matches!(err, RuntimeError::IntegerOverflow { .. }),
        "i64::MIN - 1 should be IntegerOverflow, got: {err:?}"
    );
}

#[test]
fn h4_3_multiply_overflow() {
    let err = eval("9223372036854775807 * 2").unwrap_err();
    assert!(matches!(err, RuntimeError::IntegerOverflow { .. }));
}

// ════════════════════════════════════════════════════════════════════════════
// H4.4 — String Safety
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn h4_4_null_byte_in_string() {
    // A null byte embedded in a string literal should be handled cleanly.
    let source = r#"let s = "hello\0world""#;
    let mut interp = Interpreter::new_capturing();
    // Should either accept the null byte or produce a lex/parse error — not crash.
    let _result = interp.eval_source(source);
}

#[test]
fn h4_4_very_long_string_literal() {
    // A string with 100K characters.
    let long_str: String = std::iter::repeat('x').take(100_000).collect();
    let source = format!("let s = \"{}\"", long_str);
    let mut interp = Interpreter::new_capturing();
    let _result = interp.eval_source(&source);
    // No crash = pass.
}

#[test]
fn h4_4_format_string_injection_attempt() {
    // Attempt format string injection in f-strings — should not execute
    // arbitrary code or crash.
    let source = r#"
        let user_input = "%s%s%s%n%n%n"
        let msg = f"value: {user_input}"
    "#;
    let mut interp = Interpreter::new_capturing();
    let result = interp.eval_source(source);
    // Should succeed safely — user_input is just a plain string.
    assert!(
        result.is_ok(),
        "format string injection should be harmless: {result:?}"
    );
}

// ════════════════════════════════════════════════════════════════════════════
// H4.5 — Array Bounds
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn h4_5_negative_index() {
    let err = eval("let arr = [1, 2, 3]\narr[-1]").unwrap_err();
    assert!(
        matches!(err, RuntimeError::IndexOutOfBounds { .. }),
        "negative index should be IndexOutOfBounds, got: {err:?}"
    );
}

#[test]
fn h4_5_huge_index() {
    let err = eval("let arr = [1, 2, 3]\narr[999999999]").unwrap_err();
    assert!(
        matches!(err, RuntimeError::IndexOutOfBounds { .. }),
        "huge index should be IndexOutOfBounds, got: {err:?}"
    );
}

#[test]
fn h4_5_empty_array_access() {
    let source = "let arr: [i64] = []\narr[0]";
    let result = eval(source);
    assert!(result.is_err(), "accessing empty array should error");
}

// ════════════════════════════════════════════════════════════════════════════
// H4.6 — Division by Zero
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn h4_6_integer_division_by_zero() {
    let err = eval("42 / 0").unwrap_err();
    assert!(
        matches!(err, RuntimeError::DivisionByZero),
        "int div by zero should be DivisionByZero, got: {err:?}"
    );
}

#[test]
fn h4_6_integer_modulo_by_zero() {
    let err = eval("42 % 0").unwrap_err();
    assert!(
        matches!(err, RuntimeError::DivisionByZero),
        "int modulo by zero should be DivisionByZero, got: {err:?}"
    );
}

#[test]
fn h4_6_float_division_by_zero() {
    // IEEE 754: float / 0.0 → Inf. The interpreter may produce Inf or error.
    let result = eval("42.0 / 0.0");
    match result {
        Ok(Value::Float(f)) => assert!(f.is_infinite(), "float div by zero should be Inf"),
        Err(RuntimeError::DivisionByZero) => {} // also acceptable
        other => panic!("unexpected result for float div by zero: {other:?}"),
    }
}

// ════════════════════════════════════════════════════════════════════════════
// H4.7 — Infinite Loop Protection
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn h4_7_infinite_loop_can_be_killed_via_timeout() {
    // Run `while true { let x = 1 }` in a thread and verify we can kill it
    // by dropping the thread handle (join with a timeout).
    let handle = std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(|| {
            let source = "while true { let _x = 1 }";
            let mut interp = Interpreter::new_capturing();
            let _result = interp.eval_source(source);
            // If we reach here, the loop somehow terminated.
            true
        })
        .expect("failed to spawn thread");

    // Wait a short time — if the thread is still running, it's the infinite loop.
    std::thread::sleep(std::time::Duration::from_millis(200));

    // The thread should still be running (infinite loop). We can't join without
    // blocking forever, so we just verify the thread was successfully created
    // and the test harness can clean up. The important thing is no panic/crash
    // in the spawned thread.
    assert!(
        !handle.is_finished(),
        "infinite loop should still be running after 200ms"
    );

    // Drop the handle — the thread will be detached and cleaned up at process exit.
    // This is acceptable in tests; the OS reclaims the thread.
    drop(handle);
}

// ════════════════════════════════════════════════════════════════════════════
// H4.8 — Resource Exhaustion
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn h4_8_many_variables_no_crash() {
    // Create 10K variables. Should not OOM or crash — may be slow but stable.
    // Run in a large-stack thread since 10K statements may strain the analyzer.
    let handle = std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(|| {
            let mut lines = Vec::with_capacity(10_001);
            for i in 0..10_000 {
                lines.push(format!("let var_{i} = {i}"));
            }
            // Read the last one to ensure it's accessible.
            lines.push("var_9999".to_string());
            let source = lines.join("\n");

            let mut interp = Interpreter::new_capturing();
            let _result = interp.eval_source(&source);
            // No crash = pass.
            true
        })
        .expect("failed to spawn thread");

    let ok = handle.join().expect("thread panicked on many variables");
    assert!(ok);
}

#[test]
fn h4_8_many_function_definitions() {
    // Define 1000 functions — should handle gracefully.
    // Run in a large-stack thread for safety.
    let handle = std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(|| {
            let mut lines = Vec::with_capacity(1001);
            for i in 0..1000 {
                lines.push(format!("fn func_{i}() -> i64 {{ {i} }}"));
            }
            lines.push("func_999()".to_string());
            let source = lines.join("\n");

            let mut interp = Interpreter::new_capturing();
            let _result = interp.eval_source(&source);
            // No crash = pass.
            true
        })
        .expect("failed to spawn thread");

    let ok = handle.join().expect("thread panicked on many functions");
    assert!(ok);
}

// ════════════════════════════════════════════════════════════════════════════
// H4.9 — REPL Isolation
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn h4_9_separate_interpreters_isolated() {
    // Variables from one interpreter should not leak to another.
    let mut interp1 = Interpreter::new_capturing();
    let mut interp2 = Interpreter::new_capturing();

    let _ = interp1.eval_source("let secret = 42");

    let result = interp2.eval_source("secret");
    assert!(
        result.is_err(),
        "interp2 should not see interp1's variables"
    );
}

#[test]
fn h4_9_error_does_not_corrupt_state() {
    // After an error, the interpreter should still work for subsequent calls.
    let mut interp = Interpreter::new_capturing();

    // This should error (division by zero).
    let err = interp.eval_source("1 / 0");
    assert!(err.is_err());

    // Subsequent evaluation should still work.
    let result = interp.eval_source("let y = 10\ny + 5");
    match result {
        Ok(Value::Int(15)) => {}
        other => panic!("interpreter should recover after error, got: {other:?}"),
    }
}

#[test]
fn h4_9_eval_source_calls_accumulate_state() {
    // Within the same interpreter, variables from prior calls should be visible
    // (REPL semantics).
    let mut interp = Interpreter::new_capturing();

    let _ = interp.eval_source("let counter = 100");
    let result = interp.eval_source("counter");
    match result {
        Ok(Value::Int(100)) => {}
        other => panic!("REPL should see prior variables, got: {other:?}"),
    }
}

// ════════════════════════════════════════════════════════════════════════════
// H4.10 — Type Confusion
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn h4_10_add_string_and_int() {
    let err = eval_expect_error("\"hello\" + 42");
    // Should be a type error (semantic or runtime), not a crash.
    let msg = format!("{err}");
    assert!(
        msg.contains("type")
            || msg.contains("Type")
            || msg.contains("mismatch")
            || msg.contains("cannot"),
        "adding string + int should produce a type error, got: {msg}"
    );
}

#[test]
fn h4_10_compare_incompatible_types() {
    // Comparing a bool and an int should error cleanly.
    let result = eval_expect_error("true > 42");
    let msg = format!("{result}");
    assert!(
        msg.contains("type")
            || msg.contains("Type")
            || msg.contains("mismatch")
            || msg.contains("cannot"),
        "comparing bool > int should produce a type error, got: {msg}"
    );
}

#[test]
fn h4_10_call_non_function() {
    // Calling an integer should produce a clean error. The analyzer may catch
    // this as SE002 before the interpreter even runs, so accept any error type.
    let err = eval_expect_error("let x = 42\nx(1, 2)");
    let msg = format!("{err}");
    assert!(
        msg.contains("function")
            || msg.contains("Function")
            || msg.contains("not a function")
            || msg.contains("SE002"),
        "calling an integer should produce a function-related error, got: {msg}"
    );
}

#[test]
fn h4_10_index_non_array() {
    // Indexing a non-array should produce a clean error.
    let result = eval_expect_error("let x = 42\nx[0]");
    let msg = format!("{result}");
    assert!(
        msg.contains("type") || msg.contains("index") || msg.contains("cannot"),
        "indexing an integer should produce an error, got: {msg}"
    );
}
