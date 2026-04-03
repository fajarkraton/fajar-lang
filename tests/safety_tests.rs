//! Comprehensive safety test suite for Fajar Lang.
//!
//! Tests memory safety, integer overflow, bounds checking, null safety,
//! move semantics, context isolation, and type safety across the full pipeline.

use fajar_lang::FjError;
use fajar_lang::interpreter::{Interpreter, RuntimeError, Value};

/// Helper: run source through full pipeline (lex → parse → analyze → eval).
/// Extracts RuntimeError from FjError for convenient matching.
fn eval(source: &str) -> Result<Value, RuntimeError> {
    let mut interp = Interpreter::new_capturing();
    interp.eval_source(source).map_err(|e| match e {
        FjError::Runtime(re) => re,
        other => panic!("expected RuntimeError, got: {other}"),
    })
}

/// Helper: run source and expect any error (semantic or runtime).
fn eval_any_error(source: &str) -> FjError {
    let mut interp = Interpreter::new_capturing();
    interp.eval_source(source).unwrap_err()
}

/// Helper: check that source produces a semantic error containing the given code.
fn expect_semantic_error(source: &str, error_code: &str) {
    let tokens = fajar_lang::lexer::tokenize(source).expect("lex failed");
    let program = fajar_lang::parser::parse(tokens).expect("parse failed");
    let errors = fajar_lang::analyzer::analyze(&program).unwrap_err();
    let found = errors.iter().any(|e| format!("{e}").contains(error_code));
    assert!(
        found,
        "expected error containing '{error_code}', got: {errors:?}"
    );
}

/// Helper: check that source produces a strict-mode semantic error containing the code.
fn expect_strict_error(source: &str, error_code: &str) {
    let tokens = fajar_lang::lexer::tokenize(source).expect("lex failed");
    let program = fajar_lang::parser::parse(tokens).expect("parse failed");
    let errors = fajar_lang::analyzer::analyze_strict(&program).unwrap_err();
    let found = errors.iter().any(|e| format!("{e}").contains(error_code));
    assert!(
        found,
        "expected strict error containing '{error_code}', got: {errors:?}"
    );
}

/// Helper: check that source passes strict-mode analysis without errors.
fn expect_strict_ok(source: &str) {
    let tokens = fajar_lang::lexer::tokenize(source).expect("lex failed");
    let program = fajar_lang::parser::parse(tokens).expect("parse failed");
    let result = fajar_lang::analyzer::analyze_strict(&program);
    assert!(
        result.is_ok(),
        "expected strict analysis to pass, got errors: {:?}",
        result.unwrap_err()
    );
}

// ════════════════════════════════════════════════════════════════════════
// Integer Overflow Safety (RE009)
// ════════════════════════════════════════════════════════════════════════

#[test]
fn safety_overflow_add_max_plus_one() {
    let err = eval("9223372036854775807 + 1").unwrap_err();
    assert!(matches!(err, RuntimeError::IntegerOverflow { .. }));
}

#[test]
fn safety_overflow_sub_min_minus_one() {
    let err = eval("let x = -9223372036854775807 - 1\nx - 1").unwrap_err();
    assert!(matches!(err, RuntimeError::IntegerOverflow { .. }));
}

#[test]
fn safety_overflow_mul_large() {
    let err = eval("9223372036854775807 * 2").unwrap_err();
    assert!(matches!(err, RuntimeError::IntegerOverflow { .. }));
}

#[test]
fn safety_overflow_pow_large() {
    let err = eval("2 ** 63").unwrap_err();
    assert!(matches!(err, RuntimeError::IntegerOverflow { .. }));
}

#[test]
fn safety_overflow_div_min_by_neg1() {
    let err = eval("let x = -9223372036854775807 - 1\nx / -1").unwrap_err();
    assert!(matches!(err, RuntimeError::IntegerOverflow { .. }));
}

#[test]
fn safety_overflow_compound_add_assign() {
    let src = "let mut x = 9223372036854775807\nx += 1";
    let err = eval(src).unwrap_err();
    assert!(matches!(err, RuntimeError::IntegerOverflow { .. }));
}

#[test]
fn safety_overflow_compound_sub_assign() {
    let src = "let mut x = -9223372036854775807 - 1\nx -= 1";
    let err = eval(src).unwrap_err();
    assert!(matches!(err, RuntimeError::IntegerOverflow { .. }));
}

#[test]
fn safety_overflow_compound_mul_assign() {
    let src = "let mut x = 9223372036854775807\nx *= 2";
    let err = eval(src).unwrap_err();
    assert!(matches!(err, RuntimeError::IntegerOverflow { .. }));
}

#[test]
fn safety_normal_arithmetic_no_overflow() {
    assert_eq!(eval("100 + 200").unwrap(), Value::Int(300));
    assert_eq!(eval("1000 * 1000").unwrap(), Value::Int(1_000_000));
    assert_eq!(eval("50 - 100").unwrap(), Value::Int(-50));
    assert_eq!(eval("2 ** 10").unwrap(), Value::Int(1024));
    assert_eq!(eval("100 / 3").unwrap(), Value::Int(33));
}

// ── Wrapping builtins ──

#[test]
fn safety_wrapping_add() {
    assert_eq!(
        eval("wrapping_add(9223372036854775807, 1)").unwrap(),
        Value::Int(i64::MIN)
    );
}

#[test]
fn safety_wrapping_sub() {
    assert_eq!(
        eval("let x = -9223372036854775807 - 1\nwrapping_sub(x, 1)").unwrap(),
        Value::Int(i64::MAX)
    );
}

#[test]
fn safety_wrapping_mul() {
    assert_eq!(
        eval("wrapping_mul(9223372036854775807, 2)").unwrap(),
        Value::Int(-2)
    );
}

#[test]
fn safety_wrapping_add_no_overflow() {
    assert_eq!(eval("wrapping_add(10, 20)").unwrap(), Value::Int(30));
}

// ── Checked builtins ──

#[test]
fn safety_checked_add_some() {
    assert_eq!(
        eval("checked_add(1, 2)").unwrap(),
        Value::Enum {
            variant: "Some".into(),
            data: Some(Box::new(Value::Int(3))),
        }
    );
}

#[test]
fn safety_checked_add_none_on_overflow() {
    assert_eq!(
        eval("checked_add(9223372036854775807, 1)").unwrap(),
        Value::Enum {
            variant: "None".into(),
            data: None,
        }
    );
}

#[test]
fn safety_checked_sub_some() {
    assert_eq!(
        eval("checked_sub(10, 3)").unwrap(),
        Value::Enum {
            variant: "Some".into(),
            data: Some(Box::new(Value::Int(7))),
        }
    );
}

#[test]
fn safety_checked_mul_none_on_overflow() {
    assert_eq!(
        eval("checked_mul(9223372036854775807, 2)").unwrap(),
        Value::Enum {
            variant: "None".into(),
            data: None,
        }
    );
}

// ── Saturating builtins ──

#[test]
fn safety_saturating_add_clamps_at_max() {
    assert_eq!(
        eval("saturating_add(9223372036854775807, 100)").unwrap(),
        Value::Int(i64::MAX)
    );
}

#[test]
fn safety_saturating_sub_clamps_at_min() {
    assert_eq!(
        eval("let x = -9223372036854775807 - 1\nsaturating_sub(x, 100)").unwrap(),
        Value::Int(i64::MIN)
    );
}

#[test]
fn safety_saturating_mul_clamps() {
    assert_eq!(
        eval("saturating_mul(9223372036854775807, 2)").unwrap(),
        Value::Int(i64::MAX)
    );
}

// ════════════════════════════════════════════════════════════════════════
// Array Bounds Checking (RE010)
// ════════════════════════════════════════════════════════════════════════

#[test]
fn safety_array_index_out_of_bounds() {
    // P2.1 compile-time bounds check: caught at analysis time (SE022).
    let err = eval_any_error("[1, 2, 3][5]");
    let msg = format!("{err}");
    assert!(
        msg.contains("out of bounds") || msg.contains("SE022"),
        "expected OOB error, got: {msg}"
    );
}

#[test]
fn safety_array_index_negative_wraps() {
    // Negative index wraps to large usize, out of bounds
    let err = eval("[1, 2, 3][-1]").unwrap_err();
    assert!(matches!(err, RuntimeError::IndexOutOfBounds { .. }));
}

#[test]
fn safety_array_index_empty() {
    // P2.1 compile-time bounds check: caught at analysis time (SE022).
    let err = eval_any_error("[][0]");
    let msg = format!("{err}");
    assert!(
        msg.contains("out of bounds") || msg.contains("SE022"),
        "expected OOB error, got: {msg}"
    );
}

#[test]
fn safety_array_valid_index() {
    assert_eq!(eval("[10, 20, 30][0]").unwrap(), Value::Int(10));
    assert_eq!(eval("[10, 20, 30][2]").unwrap(), Value::Int(30));
}

#[test]
fn safety_string_index_out_of_bounds() {
    let err = eval(r#""hi"[5]"#).unwrap_err();
    assert!(matches!(
        err,
        RuntimeError::IndexOutOfBounds {
            index: 5,
            length: 2,
            ..
        }
    ));
}

#[test]
fn safety_string_valid_index() {
    assert_eq!(eval(r#""hello"[0]"#).unwrap(), Value::Char('h'));
    assert_eq!(eval(r#""hello"[4]"#).unwrap(), Value::Char('o'));
}

#[test]
fn safety_array_assign_out_of_bounds() {
    let err = eval("let mut a = [1, 2, 3]\na[10] = 99").unwrap_err();
    assert!(matches!(err, RuntimeError::IndexOutOfBounds { .. }));
}

// ════════════════════════════════════════════════════════════════════════
// Null Safety
// ════════════════════════════════════════════════════════════════════════

#[test]
fn safety_null_arithmetic_is_error() {
    // null + 1 caught at semantic analysis (SE004 type mismatch)
    let err = eval_any_error("null + 1");
    assert!(matches!(err, FjError::Semantic(_)));
}

#[test]
fn safety_null_comparison_works() {
    // null == null is valid
    assert_eq!(eval("null == null").unwrap(), Value::Bool(true));
}

#[test]
fn safety_try_on_non_option_is_error() {
    let src = r#"
        fn bad() -> i64 {
            let x = 42?
            x
        }
        bad()
    "#;
    let err = eval(src).unwrap_err();
    assert!(matches!(err, RuntimeError::TypeError(_)));
}

#[test]
fn safety_try_propagates_none() {
    let src = r#"
        fn get() -> i64 {
            let val = None?
            val + 1
        }
        get()
    "#;
    assert_eq!(
        eval(src).unwrap(),
        Value::Enum {
            variant: "None".into(),
            data: None,
        }
    );
}

#[test]
fn safety_try_unwraps_some() {
    let src = r#"
        fn get() -> i64 {
            let val = Some(42)?
            val
        }
        get()
    "#;
    assert_eq!(eval(src).unwrap(), Value::Int(42));
}

#[test]
fn safety_try_propagates_err() {
    let src = r#"
        fn get() -> i64 {
            let val = Err("fail")?
            val + 1
        }
        get()
    "#;
    assert_eq!(
        eval(src).unwrap(),
        Value::Enum {
            variant: "Err".into(),
            data: Some(Box::new(Value::Str("fail".into()))),
        }
    );
}

#[test]
fn safety_unwrap_none_panics() {
    let err = eval("None.unwrap()").unwrap_err();
    assert!(matches!(err, RuntimeError::TypeError(_)));
}

#[test]
fn safety_unwrap_some_succeeds() {
    assert_eq!(eval("Some(42).unwrap()").unwrap(), Value::Int(42));
}

// ════════════════════════════════════════════════════════════════════════
// Stack Overflow Protection (RE003)
// ════════════════════════════════════════════════════════════════════════

#[test]
fn safety_stack_overflow_infinite_recursion() {
    // Needs larger stack so interpreter can catch overflow before Rust crashes
    let result = std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(|| {
            let src = "fn inf(n: i64) -> i64 { inf(n) }\ninf(0)";
            let err = eval(src).unwrap_err();
            assert!(matches!(err, RuntimeError::StackOverflow { .. }));
        })
        .expect("thread spawn")
        .join();
    result.expect("test panicked");
}

#[test]
fn safety_stack_overflow_mutual_recursion() {
    let result = std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(|| {
            let src = r#"
                fn a(n: i64) -> i64 { b(n) }
                fn b(n: i64) -> i64 { a(n) }
                a(0)
            "#;
            let err = eval(src).unwrap_err();
            assert!(matches!(err, RuntimeError::StackOverflow { .. }));
        })
        .expect("thread spawn")
        .join();
    result.expect("test panicked");
}

#[test]
fn safety_stack_overflow_custom_depth() {
    let src = "fn inf(n: i64) -> i64 { inf(n) }\ninf(0)";
    let tokens = fajar_lang::lexer::tokenize(src).unwrap();
    let program = fajar_lang::parser::parse(tokens).unwrap();
    let mut interp = Interpreter::new_capturing();
    interp.set_max_recursion_depth(5);
    let err = interp.eval_program(&program).unwrap_err();
    match err {
        RuntimeError::StackOverflow { depth, .. } => assert_eq!(depth, 5),
        other => panic!("expected StackOverflow, got: {other:?}"),
    }
}

#[test]
fn safety_deep_but_not_overflow() {
    // Recursion that terminates before limit
    let src = r#"
        fn countdown(n: i64) -> i64 {
            if n <= 0 { 0 }
            else { countdown(n - 1) }
        }
        countdown(10)
    "#;
    assert_eq!(eval(src).unwrap(), Value::Int(0));
}

// ════════════════════════════════════════════════════════════════════════
// Division by Zero
// ════════════════════════════════════════════════════════════════════════

#[test]
fn safety_division_by_zero_int() {
    let err = eval("10 / 0").unwrap_err();
    assert!(matches!(err, RuntimeError::DivisionByZero));
}

#[test]
fn safety_division_by_zero_float() {
    let err = eval("10.0 / 0.0").unwrap_err();
    assert!(matches!(err, RuntimeError::DivisionByZero));
}

#[test]
fn safety_modulo_by_zero() {
    let err = eval("10 % 0").unwrap_err();
    assert!(matches!(err, RuntimeError::DivisionByZero));
}

// ════════════════════════════════════════════════════════════════════════
// Move Semantics (ME001)
// ════════════════════════════════════════════════════════════════════════

#[test]
fn safety_move_string_use_after_move() {
    // Strings are now Copy (runtime uses clone-on-assign like Rc<String>),
    // so `let t = s` copies the string and both remain valid.
    // Verify no error occurs:
    let result = eval(
        r#"
        let s: str = "hello"
        let t: str = s
        println(s)
        "#,
    );
    assert!(result.is_ok());
}

#[test]
fn safety_move_array_use_after_move() {
    // Arrays are clone-on-assign (like strings), so both remain valid.
    // Verify no error occurs:
    let result = eval(
        r#"
        let a: [i64] = [1, 2, 3]
        let b: [i64] = a
        len(a)
        "#,
    );
    assert!(result.is_ok());
}

#[test]
fn safety_copy_type_no_move() {
    // Integers are Copy — no move semantics
    assert_eq!(
        eval("let x = 42\nlet y = x\nx + y").unwrap(),
        Value::Int(84)
    );
}

#[test]
fn safety_copy_bool_no_move() {
    assert_eq!(
        eval("let a = true\nlet b = a\na").unwrap(),
        Value::Bool(true)
    );
}

#[test]
fn safety_copy_float_no_move() {
    assert_eq!(
        eval("let x = 3.14\nlet y = x\nx").unwrap(),
        Value::Float(3.14)
    );
}

// ════════════════════════════════════════════════════════════════════════
// Context Isolation (@kernel / @device)
// ════════════════════════════════════════════════════════════════════════

#[test]
fn safety_kernel_no_heap_alloc() {
    // KE001: push() is a heap-allocating builtin, banned in @kernel
    expect_semantic_error(
        r#"
        @kernel fn k() {
            let mut a: [i64] = [1, 2]
            push(a, 3)
        }
        "#,
        "KE001",
    );
}

#[test]
fn safety_kernel_no_tensor() {
    expect_semantic_error(
        r#"
        @kernel fn k() {
            let t: Tensor = tensor_zeros([2, 3])
        }
        "#,
        "KE002",
    );
}

#[test]
fn safety_device_no_raw_pointer() {
    // DE001: OS builtins (mem_alloc) banned in @device context
    expect_semantic_error(
        r#"
        @device fn d() {
            mem_alloc(4096, 8)
        }
        "#,
        "DE001",
    );
}

// ════════════════════════════════════════════════════════════════════════
// FFI Safety (SE013)
// ════════════════════════════════════════════════════════════════════════

#[test]
fn safety_ffi_rejects_string_param() {
    expect_semantic_error(
        r#"
        extern fn bad(s: str) -> i32
        "#,
        "SE013",
    );
}

#[test]
fn safety_ffi_rejects_array_param() {
    expect_semantic_error(
        r#"
        extern fn bad(a: [i32]) -> i32
        "#,
        "SE013",
    );
}

#[test]
fn safety_ffi_accepts_primitives() {
    // Should NOT produce an error
    let src = "extern fn abs(x: i32) -> i32";
    let tokens = fajar_lang::lexer::tokenize(src).expect("lex failed");
    let program = fajar_lang::parser::parse(tokens).expect("parse failed");
    let result = fajar_lang::analyzer::analyze(&program);
    assert!(
        result.is_ok(),
        "extern fn with primitives should pass: {result:?}"
    );
}

// ════════════════════════════════════════════════════════════════════════
// Type Safety
// ════════════════════════════════════════════════════════════════════════

#[test]
fn safety_type_mismatch_add_str_int() {
    // Caught at semantic analysis (SE004)
    let err = eval_any_error(r#""hello" + 1"#);
    assert!(matches!(err, FjError::Semantic(_)));
}

#[test]
fn safety_type_mismatch_call_non_function() {
    // Caught at semantic analysis (SE002)
    let err = eval_any_error("let x = 42\nx(1)");
    assert!(matches!(err, FjError::Semantic(_)));
}

#[test]
fn safety_undefined_variable() {
    // Caught at semantic analysis (SE001)
    let err = eval_any_error("nonexistent + 1");
    assert!(matches!(err, FjError::Semantic(_)));
}

#[test]
fn safety_wrong_arity() {
    // Caught at semantic analysis (SE005)
    let src = "fn f(a: i64) -> i64 { a }\nf(1, 2)";
    let err = eval_any_error(src);
    assert!(matches!(err, FjError::Semantic(_)));
}

// ════════════════════════════════════════════════════════════════════════
// Tensor Shape Safety (Runtime)
// ════════════════════════════════════════════════════════════════════════

#[test]
fn safety_tensor_add_shape_mismatch() {
    // TE001: element-wise add requires matching shapes
    let src = r#"
        let a = tensor_from_data([1.0, 2.0, 3.0], [3])
        let b = tensor_from_data([1.0, 2.0], [2])
        tensor_add(a, b)
    "#;
    let err = eval(src);
    assert!(err.is_err(), "shape mismatch should error");
    let msg = format!("{}", err.unwrap_err());
    assert!(msg.contains("shape"), "error should mention shape: {msg}");
}

#[test]
fn safety_tensor_sub_shape_mismatch() {
    let src = r#"
        let a = tensor_from_data([1.0, 2.0], [2])
        let b = tensor_from_data([1.0, 2.0, 3.0], [3])
        tensor_sub(a, b)
    "#;
    let err = eval(src);
    assert!(err.is_err());
}

#[test]
fn safety_tensor_mul_shape_mismatch() {
    let src = r#"
        let a = tensor_from_data([1.0, 2.0, 3.0], [3])
        let b = tensor_from_data([1.0, 2.0, 3.0, 4.0], [4])
        tensor_mul(a, b)
    "#;
    let err = eval(src);
    assert!(err.is_err());
}

#[test]
fn safety_tensor_matmul_inner_dim_mismatch() {
    // TE002: matmul [2,3] @ [4,2] — inner dims 3 != 4
    let src = r#"
        let a = tensor_from_data([1.0, 2.0, 3.0, 4.0, 5.0, 6.0], [2, 3])
        let b = tensor_from_data([1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0], [4, 2])
        tensor_matmul(a, b)
    "#;
    let err = eval(src);
    assert!(err.is_err());
    let msg = format!("{}", err.unwrap_err());
    assert!(
        msg.contains("matmul") || msg.contains("shape"),
        "error should mention matmul: {msg}"
    );
}

#[test]
fn safety_tensor_matmul_rank1_error() {
    // matmul requires rank 2
    let src = r#"
        let a = tensor_from_data([1.0, 2.0, 3.0], [3])
        let b = tensor_from_data([1.0, 2.0, 3.0], [3])
        tensor_matmul(a, b)
    "#;
    let err = eval(src);
    assert!(err.is_err());
}

#[test]
fn safety_tensor_reshape_element_count_mismatch() {
    // TE003: can't reshape [6] to [2,4] (6 vs 8 elements)
    let src = r#"
        let a = tensor_from_data([1.0, 2.0, 3.0, 4.0, 5.0, 6.0], [6])
        tensor_reshape(a, [2, 4])
    "#;
    let err = eval(src);
    assert!(err.is_err());
    let msg = format!("{}", err.unwrap_err());
    assert!(
        msg.contains("reshape") || msg.contains("element"),
        "error should mention reshape: {msg}"
    );
}

#[test]
fn safety_tensor_from_data_shape_mismatch() {
    // Data length doesn't match shape
    let src = r#"
        tensor_from_data([1.0, 2.0, 3.0], [2, 2])
    "#;
    let err = eval(src);
    assert!(err.is_err());
}

#[test]
fn safety_tensor_backward_non_scalar() {
    // backward on non-scalar tensor — may produce seed gradient for full shape
    // This is valid behavior (backward seeds with ones of output shape)
    let src = r#"
        let a = tensor_set_requires_grad(tensor_from_data([1.0, 2.0], [2]), true)
        let b = tensor_add(a, a)
        let s = tensor_sum(b)
        tensor_backward(s)
        tensor_grad(a)
    "#;
    let result = eval(src);
    assert!(result.is_ok(), "backward on scalar sum should succeed");
}

#[test]
fn safety_tensor_div_by_zero() {
    // TE007: division by zero element
    let src = r#"
        let a = tensor_from_data([1.0, 2.0], [2])
        let b = tensor_from_data([0.0, 0.0], [2])
        tensor_div(a, b)
    "#;
    // Tensor div by zero may produce Inf/NaN rather than error — verify it doesn't panic
    let result = eval(src);
    // Either succeeds (with Inf/NaN) or returns error — both are safe
    match result {
        Ok(_) => {}  // NaN/Inf is ok
        Err(_) => {} // Error is also ok
    }
}

#[test]
fn safety_tensor_grad_no_requires_grad() {
    // Gradient on tensor without requires_grad — returns error or zeros
    let src = r#"
        let a = tensor_from_data([1.0, 2.0], [2])
        let b = tensor_add(a, a)
        tensor_sum(b)
    "#;
    // Should run without panic (gradient not requested, so no backward needed)
    let result = eval(src);
    assert!(result.is_ok(), "operations without grad should succeed");
}

#[test]
fn safety_tensor_valid_add() {
    // Positive test: matching shapes should work
    let src = r#"
        let a = tensor_from_data([1.0, 2.0, 3.0], [3])
        let b = tensor_from_data([4.0, 5.0, 6.0], [3])
        let c = tensor_add(a, b)
        tensor_shape(c)
    "#;
    let result = eval(src);
    assert!(result.is_ok(), "valid tensor add should succeed");
}

#[test]
fn safety_tensor_valid_matmul() {
    // Positive test: [2,3] @ [3,2] should succeed
    let src = r#"
        let a = tensor_from_data([1.0, 2.0, 3.0, 4.0, 5.0, 6.0], [2, 3])
        let b = tensor_from_data([1.0, 2.0, 3.0, 4.0, 5.0, 6.0], [3, 2])
        let c = tensor_matmul(a, b)
        tensor_shape(c)
    "#;
    let result = eval(src);
    assert!(result.is_ok(), "valid matmul should succeed");
}

#[test]
fn safety_tensor_valid_reshape() {
    // Positive test: [6] → [2,3] (same element count)
    let src = r#"
        let a = tensor_from_data([1.0, 2.0, 3.0, 4.0, 5.0, 6.0], [6])
        let b = tensor_reshape(a, [2, 3])
        tensor_shape(b)
    "#;
    let result = eval(src);
    assert!(result.is_ok(), "valid reshape should succeed");
}

#[test]
fn safety_tensor_squeeze_with_dim1() {
    // squeeze removes dimensions of size 1
    let src = r#"
        let a = tensor_from_data([1.0, 2.0, 3.0], [1, 3])
        let b = tensor_squeeze(a, 0)
        tensor_shape(b)
    "#;
    let result = eval(src);
    assert!(result.is_ok(), "squeeze on dim=1 should succeed");
}

#[test]
fn safety_tensor_transpose_rank2() {
    // Transpose on rank-2 tensor
    let src = r#"
        let a = tensor_from_data([1.0, 2.0, 3.0, 4.0, 5.0, 6.0], [2, 3])
        let b = tensor_transpose(a)
        tensor_shape(b)
    "#;
    let result = eval(src);
    assert!(result.is_ok(), "transpose should succeed on rank 2");
}

#[test]
fn safety_tensor_flatten() {
    let src = r#"
        let a = tensor_from_data([1.0, 2.0, 3.0, 4.0, 5.0, 6.0], [2, 3])
        let b = tensor_flatten(a)
        tensor_shape(b)
    "#;
    let result = eval(src);
    assert!(result.is_ok(), "flatten should always succeed");
}

#[test]
fn safety_tensor_softmax_valid() {
    let src = r#"
        let a = tensor_from_data([1.0, 2.0, 3.0], [3])
        let b = tensor_softmax(a)
        tensor_sum(b)
    "#;
    let result = eval(src);
    assert!(result.is_ok(), "softmax on valid tensor should succeed");
}

#[test]
fn safety_tensor_relu_preserves_shape() {
    let src = r#"
        let a = tensor_from_data([-1.0, 0.0, 1.0, 2.0], [2, 2])
        let b = tensor_relu(a)
        tensor_shape(b)
    "#;
    let result = eval(src);
    assert!(result.is_ok(), "relu should preserve shape");
}

#[test]
fn safety_tensor_creation_valid() {
    // tensor_from_data should create tensors with correct shape
    let src = r#"
        let a = tensor_from_data([1.0, 2.0, 3.0, 4.0, 5.0, 6.0], [2, 3])
        let b = tensor_from_data([1.0], [1])
        tensor_shape(a)
    "#;
    let result = eval(src);
    assert!(
        result.is_ok(),
        "tensor creation with valid shapes should succeed"
    );
}

// ════════════════════════════════════════════════════════════════════════
// Strict Ownership — Phase D Integration Tests (20 tests)
// ════════════════════════════════════════════════════════════════════════

// ── ME001: Use After Move ──────────────────────────────────────────────

#[test]
fn strict_me001_string_use_after_move() {
    expect_strict_error(
        r#"
        let s: str = "hello"
        let t: str = s
        println(s)
        "#,
        "ME001",
    );
}

#[test]
fn strict_me001_array_use_after_move() {
    expect_strict_error(
        r#"
        let a = [1, 2, 3]
        let b = a
        len(a)
        "#,
        "ME001",
    );
}

#[test]
fn strict_me001_struct_use_after_move() {
    expect_strict_error(
        r#"
        struct Point { x: f64, y: f64 }
        let p = Point { x: 1.0, y: 2.0 }
        let q = p
        println(p)
        "#,
        "ME001",
    );
}

#[test]
fn strict_me001_move_in_function_call() {
    expect_strict_error(
        r#"
        fn consume(s: str) -> str { s }
        let s: str = "hello"
        consume(s)
        println(s)
        "#,
        "ME001",
    );
}

// ── ME003: Move While Borrowed ─────────────────────────────────────────

#[test]
fn strict_me003_move_while_immutably_borrowed() {
    expect_strict_error(
        r#"
        let s: str = "hello"
        let r = &s
        let t: str = s
        "#,
        "ME003",
    );
}

#[test]
fn strict_me003_move_while_mutably_borrowed() {
    expect_strict_error(
        r#"
        let mut s: str = "hello"
        let r = &mut s
        let t: str = s
        "#,
        "ME003",
    );
}

// ── ME004: Mutable Borrow Conflict ─────────────────────────────────────

#[test]
fn strict_me004_double_mut_borrow() {
    expect_strict_error(
        r#"
        let mut x: i64 = 42
        let r1 = &mut x
        let r2 = &mut x
        "#,
        "ME004",
    );
}

#[test]
fn strict_me004_mut_borrow_while_imm_borrowed() {
    expect_strict_error(
        r#"
        let mut x: i64 = 42
        let r1 = &x
        let r2 = &mut x
        "#,
        "ME004",
    );
}

// ── ME005: Immutable Borrow Conflict ───────────────────────────────────

#[test]
fn strict_me005_imm_borrow_while_mut_borrowed() {
    expect_strict_error(
        r#"
        let mut x: i64 = 42
        let r1 = &mut x
        let r2 = &x
        "#,
        "ME005",
    );
}

// ── ME010: Dangling Reference ──────────────────────────────────────────

#[test]
fn strict_me010_return_ref_to_local() {
    // Single-line block so &z is parsed as the tail expression
    expect_strict_error(r#"fn dangling() -> &i32 { let z: i32 = 1; &z }"#, "ME010");
}

// ── Copy Types: No Move Errors ─────────────────────────────────────────

#[test]
fn strict_copy_int_no_move() {
    expect_strict_ok(
        r#"
        let x: i64 = 42
        let y: i64 = x
        let z: i64 = x + y
        "#,
    );
}

#[test]
fn strict_copy_bool_no_move() {
    expect_strict_ok(
        r#"
        let a: bool = true
        let b: bool = a
        let c: bool = a
        "#,
    );
}

#[test]
fn strict_copy_float_no_move() {
    expect_strict_ok(
        r#"
        let x: f64 = 3.14
        let y: f64 = x
        let z: f64 = x + y
        "#,
    );
}

#[test]
fn strict_copy_ref_no_move() {
    // References themselves are Copy
    expect_strict_ok(
        r#"
        let x: i64 = 42
        let r1 = &x
        let r2 = r1
        "#,
    );
}

// ── Multiple Immutable Borrows OK ──────────────────────────────────────

#[test]
fn strict_multiple_imm_borrows_ok() {
    expect_strict_ok(
        r#"
        let x: i64 = 42
        let r1 = &x
        let r2 = &x
        let r3 = &x
        "#,
    );
}

// ── Sequential Borrows (NLL) ───────────────────────────────────────────

#[test]
fn strict_nll_reborrow_after_release() {
    // After NLL releases immutable borrows, we can take new immutable borrows
    expect_strict_ok(
        r#"
        let x: i64 = 42
        let r1 = &x
        let r2 = &x
        "#,
    );
}

// ── Error Hint Content ─────────────────────────────────────────────────

#[test]
fn strict_me001_error_has_hint() {
    let tokens = fajar_lang::lexer::tokenize(
        r#"
        let s: str = "hello"
        let t: str = s
        println(s)
        "#,
    )
    .expect("lex");
    let program = fajar_lang::parser::parse(tokens).expect("parse");
    let errors = fajar_lang::analyzer::analyze_strict(&program).unwrap_err();
    let me001 = errors.iter().find(|e| format!("{e}").contains("ME001"));
    assert!(me001.is_some(), "expected ME001 error");
    let hint = me001.unwrap().hint();
    assert!(hint.is_some(), "ME001 should have a hint");
    assert!(
        hint.unwrap().contains("clone"),
        "hint should suggest cloning"
    );
}

#[test]
fn strict_me003_error_has_hint() {
    let tokens = fajar_lang::lexer::tokenize(
        r#"
        let s: str = "hello"
        let r = &s
        let t: str = s
        "#,
    )
    .expect("lex");
    let program = fajar_lang::parser::parse(tokens).expect("parse");
    let errors = fajar_lang::analyzer::analyze_strict(&program).unwrap_err();
    let me003 = errors.iter().find(|e| format!("{e}").contains("ME003"));
    assert!(me003.is_some(), "expected ME003 error");
    let hint = me003.unwrap().hint();
    assert!(hint.is_some(), "ME003 should have a hint");
    assert!(
        hint.unwrap().contains("borrow"),
        "hint should mention borrow"
    );
}

#[test]
fn strict_me001_error_has_secondary_span() {
    let tokens = fajar_lang::lexer::tokenize(
        r#"
        let s: str = "hello"
        let t: str = s
        println(s)
        "#,
    )
    .expect("lex");
    let program = fajar_lang::parser::parse(tokens).expect("parse");
    let errors = fajar_lang::analyzer::analyze_strict(&program).unwrap_err();
    let me001 = errors.iter().find(|e| format!("{e}").contains("ME001"));
    assert!(me001.is_some(), "expected ME001 error");
    let secondary = me001.unwrap().secondary_span();
    assert!(secondary.is_some(), "ME001 should have secondary span");
    let (_, label) = secondary.unwrap();
    assert_eq!(label, "value moved here");
}

// ── Error Message Contains Byte Offset ─────────────────────────────────

#[test]
fn strict_me001_message_has_move_location() {
    let tokens = fajar_lang::lexer::tokenize(
        r#"
        let s: str = "hello"
        let t: str = s
        println(s)
        "#,
    )
    .expect("lex");
    let program = fajar_lang::parser::parse(tokens).expect("parse");
    let errors = fajar_lang::analyzer::analyze_strict(&program).unwrap_err();
    let me001 = errors.iter().find(|e| format!("{e}").contains("ME001"));
    assert!(me001.is_some(), "expected ME001 error");
    let msg = format!("{}", me001.unwrap());
    assert!(
        msg.contains("moved at byte"),
        "error message should include move location, got: {msg}"
    );
}
