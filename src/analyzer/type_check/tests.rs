use super::*;
use crate::lexer::tokenize;
use crate::parser::parse;

fn check(source: &str) -> Result<(), Vec<SemanticError>> {
    let tokens = tokenize(source).expect("lex error");
    let program = parse(tokens).expect("parse error");
    let mut tc = TypeChecker::new();
    tc.analyze(&program)
}

fn check_errors(source: &str) -> Vec<SemanticError> {
    check(source).unwrap_err()
}

/// Returns all diagnostics (errors + warnings) from analysis.
fn check_all_diagnostics(source: &str) -> Vec<SemanticError> {
    let tokens = tokenize(source).expect("lex error");
    let program = parse(tokens).expect("parse error");
    let mut tc = TypeChecker::new();
    let _ = tc.analyze(&program);
    tc.errors
}

// ── Valid programs ──

#[test]
fn valid_int_arithmetic() {
    assert!(check("1 + 2").is_ok());
}

#[test]
fn valid_let_binding() {
    assert!(check("let x = 42").is_ok());
}

#[test]
fn valid_let_with_type() {
    assert!(check("let x: i64 = 42").is_ok());
}

#[test]
fn valid_string_concat() {
    assert!(check("\"a\" + \"b\"").is_ok());
}

#[test]
fn valid_boolean_comparison() {
    assert!(check("1 < 2").is_ok());
}

#[test]
fn valid_function_def_and_call() {
    let src = "fn add(a: i64, b: i64) -> i64 { a + b }\nadd(1, 2)";
    assert!(check(src).is_ok());
}

#[test]
fn valid_if_else() {
    assert!(check("if true { 1 } else { 2 }").is_ok());
}

#[test]
fn valid_while_loop() {
    assert!(check("let mut x = 0\nwhile x < 5 { x += 1 }").is_ok());
}

#[test]
fn valid_for_loop() {
    assert!(check("for i in [1, 2, 3] { println(i) }").is_ok());
}

#[test]
fn valid_array_literal() {
    assert!(check("[1, 2, 3]").is_ok());
}

#[test]
fn valid_closure() {
    assert!(check("let f = |x: i64| -> i64 { x * 2 }\nf(5)").is_ok());
}

#[test]
fn valid_println() {
    assert!(check("println(42)").is_ok());
}

#[test]
fn valid_pipeline() {
    let src = "fn double(x: i64) -> i64 { x * 2 }\n5 |> double";
    assert!(check(src).is_ok());
}

#[test]
fn valid_match() {
    let src = "match 1 { 1 => true, _ => false }";
    assert!(check(src).is_ok());
}

#[test]
fn valid_struct_def_and_init() {
    let src = "struct Point { x: f64, y: f64 }\nlet p = Point { x: 1.0, y: 2.0 }";
    assert!(check(src).is_ok());
}

// ── Type errors ──

#[test]
fn error_undefined_variable() {
    let errors = check_errors("x + 1");
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, SemanticError::UndefinedVariable { .. }))
    );
}

#[test]
fn error_type_mismatch_let() {
    let errors = check_errors("let x: i64 = \"hello\"");
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, SemanticError::TypeMismatch { .. }))
    );
}

#[test]
fn error_immutable_assignment() {
    let errors = check_errors("let x = 1\nx = 2");
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, SemanticError::ImmutableAssignment { .. }))
    );
}

#[test]
fn error_arity_mismatch() {
    let src = "fn f(a: i64) -> i64 { a }\nf(1, 2)";
    let errors = check_errors(src);
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, SemanticError::ArgumentCountMismatch { .. }))
    );
}

#[test]
fn error_missing_struct_field() {
    let src = "struct Point { x: f64, y: f64 }\nlet p = Point { x: 1.0 }";
    let errors = check_errors(src);
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, SemanticError::MissingField { .. }))
    );
}

#[test]
fn error_struct_field_type_mismatch() {
    let src = "struct Point { x: f64, y: f64 }\nlet p = Point { x: \"hi\", y: 2.0 }";
    let errors = check_errors(src);
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, SemanticError::TypeMismatch { .. }))
    );
}

#[test]
fn error_mixed_array_types() {
    let errors = check_errors("[1, \"hello\"]");
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, SemanticError::TypeMismatch { .. }))
    );
}

#[test]
fn error_fn_return_type_mismatch() {
    let src = "fn f() -> i64 { \"hello\" }";
    let errors = check_errors(src);
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, SemanticError::TypeMismatch { .. }))
    );
}

#[test]
fn error_argument_type_mismatch() {
    let src = "fn f(a: i64) -> i64 { a }\nf(\"hello\")";
    let errors = check_errors(src);
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, SemanticError::TypeMismatch { .. }))
    );
}

#[test]
fn valid_mutable_assignment() {
    assert!(check("let mut x = 1\nx = 2").is_ok());
}

#[test]
fn valid_nested_functions() {
    let src = r#"
        fn outer(x: i64) -> i64 {
            fn inner(y: i64) -> i64 { y * 2 }
            inner(x) + 1
        }
        outer(5)
    "#;
    assert!(check(src).is_ok());
}

#[test]
fn valid_fibonacci() {
    let src = r#"
        fn fib(n: i64) -> i64 {
            if n <= 1 { n } else { fib(n - 1) + fib(n - 2) }
        }
        fib(10)
    "#;
    assert!(check(src).is_ok());
}

#[test]
fn valid_multiple_errors_collected() {
    let src = "let x: i64 = \"hi\"\nlet y: bool = 42";
    let errors = check_errors(src);
    assert!(errors.len() >= 2, "should collect multiple errors");
}

// ── Sprint 2.6: Distinct integer/float types ──

#[test]
fn error_i32_not_assignable_to_i64() {
    let src = "fn f(x: i32) -> i32 { x }\nlet y: i64 = f(1)";
    let errors = check_errors(src);
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, SemanticError::TypeMismatch { .. }))
    );
}

#[test]
fn error_f32_not_assignable_to_f64() {
    let src = "fn f(x: f32) -> f32 { x }\nlet y: f64 = f(1.0)";
    let errors = check_errors(src);
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, SemanticError::TypeMismatch { .. }))
    );
}

#[test]
fn error_mixed_int_arithmetic() {
    let src = r#"
        fn get_i32() -> i32 { 1 }
        fn get_i64() -> i64 { 2 }
        let x = get_i32() + get_i64()
    "#;
    let errors = check_errors(src);
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, SemanticError::TypeMismatch { .. }))
    );
}

#[test]
fn valid_same_type_arithmetic() {
    let src = r#"
        fn a() -> i32 { 1 }
        fn b() -> i32 { 2 }
        let x = a() + b()
    "#;
    assert!(check(src).is_ok());
}

#[test]
fn valid_i32_binding() {
    assert!(check("let x: i32 = 42").is_ok());
}

#[test]
fn valid_f32_binding() {
    assert!(check("let x: f32 = 3.14").is_ok());
}

#[test]
fn valid_u8_binding() {
    assert!(check("let x: u8 = 255").is_ok());
}

#[test]
fn error_bitnot_on_float() {
    let src = "fn f(x: f64) -> f64 { x }\nlet x = ~f(1.0)";
    let errors = check_errors(src);
    assert!(errors.iter().any(
        |e| matches!(e, SemanticError::TypeMismatch { expected, .. } if expected == "integer")
    ));
}

#[test]
fn valid_bitnot_preserves_type() {
    let src = r#"
        fn get_u32() -> u32 { 42 }
        let x: u32 = ~get_u32()
    "#;
    assert!(check(src).is_ok());
}

#[test]
fn error_bitwise_mixed_int_types() {
    let src = r#"
        fn a() -> i32 { 1 }
        fn b() -> i64 { 2 }
        let x = a() & b()
    "#;
    let errors = check_errors(src);
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, SemanticError::TypeMismatch { .. }))
    );
}

#[test]
fn valid_all_integer_types_resolve() {
    let src = r#"
        let a: i8 = 1
        let b: i16 = 2
        let c: i32 = 3
        let d: i64 = 4
        let e: i128 = 5
        let f: u8 = 6
        let g: u16 = 7
        let h: u32 = 8
        let i: u64 = 9
        let j: u128 = 10
        let k: isize = 11
        let l: usize = 12
    "#;
    assert!(check(src).is_ok());
}

#[test]
fn valid_both_float_types_resolve() {
    let src = "let a: f32 = 1.0\nlet b: f64 = 2.0";
    assert!(check(src).is_ok());
}

// ── Sprint 2.7: SE009 UnusedVariable ──

#[test]
fn warning_unused_variable_in_function() {
    let src = r#"
        fn f() -> void {
            let unused_var = 42
        }
    "#;
    let diags = check_all_diagnostics(src);
    assert!(
        diags.iter().any(
            |e| matches!(e, SemanticError::UnusedVariable { name, .. } if name == "unused_var")
        )
    );
}

#[test]
fn no_warning_for_used_variable() {
    let src = r#"
        fn f() -> i64 {
            let x = 42
            x
        }
    "#;
    let diags = check_all_diagnostics(src);
    assert!(
        !diags
            .iter()
            .any(|e| matches!(e, SemanticError::UnusedVariable { .. })),
        "should not warn about used variables"
    );
}

#[test]
fn no_warning_for_underscore_prefix() {
    let src = r#"
        fn f() -> void {
            let _unused = 42
        }
    "#;
    let diags = check_all_diagnostics(src);
    assert!(
        !diags
            .iter()
            .any(|e| matches!(e, SemanticError::UnusedVariable { .. })),
        "_ prefix should suppress unused warning"
    );
}

#[test]
fn unused_variable_is_warning_not_error() {
    let src = r#"
        fn f() -> void {
            let unused = 42
        }
    "#;
    // Should not cause analyze() to fail
    assert!(check(src).is_ok());
}

// ── Sprint 2.7: SE010 UnreachableCode ──

#[test]
fn warning_unreachable_after_return() {
    let src = r#"
        fn f() -> i64 {
            return 1
            let x = 2
            x
        }
    "#;
    let diags = check_all_diagnostics(src);
    assert!(
        diags
            .iter()
            .any(|e| matches!(e, SemanticError::UnreachableCode { .. }))
    );
}

#[test]
fn no_warning_without_early_return() {
    let src = r#"
        fn f() -> i64 {
            let x = 1
            let y = 2
            x + y
        }
    "#;
    let diags = check_all_diagnostics(src);
    assert!(
        !diags
            .iter()
            .any(|e| matches!(e, SemanticError::UnreachableCode { .. })),
        "no unreachable code here"
    );
}

#[test]
fn unreachable_code_is_warning_not_error() {
    let src = r#"
        fn f() -> i64 {
            return 1
            2
        }
    "#;
    assert!(check(src).is_ok());
}

// ── Sprint 2.7: SE011 NonExhaustiveMatch ──

#[test]
fn error_non_exhaustive_match() {
    let src = r#"
        fn f(x: i64) -> str {
            match x {
                0 => "zero",
                1 => "one"
            }
        }
    "#;
    let errors = check_errors(src);
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, SemanticError::NonExhaustiveMatch { .. }))
    );
}

#[test]
fn valid_exhaustive_match_with_wildcard() {
    let src = r#"
        fn f(x: i64) -> str {
            match x {
                0 => "zero",
                _ => "other"
            }
        }
    "#;
    assert!(check(src).is_ok());
}

#[test]
fn valid_exhaustive_match_with_binding() {
    let src = r#"
        fn f(x: i64) -> i64 {
            match x {
                0 => 0,
                n => n * 2
            }
        }
    "#;
    assert!(check(src).is_ok());
}

#[test]
fn non_exhaustive_match_is_error() {
    let src = r#"
        fn f(x: i64) -> str {
            match x {
                0 => "zero"
            }
        }
    "#;
    assert!(check(src).is_err());
}

// ── Sprint 2.8: ScopeKind & Context Tracking ──

#[test]
fn error_break_outside_loop() {
    let src = r#"
        fn f() -> void {
            break
        }
    "#;
    let errors = check_errors(src);
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, SemanticError::BreakOutsideLoop { .. }))
    );
}

#[test]
fn error_continue_outside_loop() {
    let src = r#"
        fn f() -> void {
            continue
        }
    "#;
    let errors = check_errors(src);
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, SemanticError::BreakOutsideLoop { .. }))
    );
}

#[test]
fn valid_break_inside_while() {
    let src = r#"
        fn f() -> void {
            let mut i = 0
            while i < 10 {
                if i == 5 { break }
                i = i + 1
            }
        }
    "#;
    assert!(check(src).is_ok());
}

#[test]
fn valid_break_inside_for() {
    let src = r#"
        fn f() -> void {
            for i in [1, 2, 3] {
                if i == 2 { break }
            }
        }
    "#;
    assert!(check(src).is_ok());
}

#[test]
fn valid_continue_inside_loop() {
    let src = r#"
        fn f() -> void {
            for i in [1, 2, 3] {
                if i == 2 { continue }
                println(i)
            }
        }
    "#;
    assert!(check(src).is_ok());
}

#[test]
fn error_return_outside_function() {
    let errors = check_errors("return 42");
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, SemanticError::ReturnOutsideFunction { .. }))
    );
}

#[test]
fn valid_return_inside_function() {
    let src = r#"
        fn f() -> i64 {
            return 42
        }
    "#;
    assert!(check(src).is_ok());
}

// ── Sprint 3.7: Context enforcement (@kernel/@device) ──

#[test]
fn kernel_fn_can_call_os_builtins() {
    let src = "@kernel fn init() { mem_alloc(4096, 8) }";
    assert!(check(src).is_ok());
}

#[test]
fn device_fn_cannot_call_os_builtins() {
    let src = "@device fn bad() { mem_alloc(4096, 8) }";
    let errors = check_errors(src);
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, SemanticError::RawPointerInDevice { .. }))
    );
}

#[test]
fn device_fn_cannot_call_kernel_fn() {
    let src = r#"
        @kernel fn kern_init() -> i64 { 0 }
        @device fn bad() -> i64 { kern_init() }
    "#;
    let errors = check_errors(src);
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, SemanticError::KernelCallInDevice { .. }))
    );
}

#[test]
fn kernel_fn_cannot_call_device_fn() {
    let src = r#"
        @device fn infer() -> i64 { 0 }
        @kernel fn bad() -> i64 { infer() }
    "#;
    let errors = check_errors(src);
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, SemanticError::DeviceCallInKernel { .. }))
    );
}

#[test]
fn safe_fn_can_call_both_kernel_and_device() {
    let src = r#"
        @kernel fn kern_fn() -> i64 { 0 }
        @device fn dev_fn() -> i64 { 0 }
        fn bridge() -> i64 { kern_fn() + dev_fn() }
    "#;
    assert!(check(src).is_ok());
}

#[test]
fn device_fn_cannot_call_irq_register() {
    let src = r#"@device fn bad() { irq_register(32, "handler") }"#;
    let errors = check_errors(src);
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, SemanticError::RawPointerInDevice { .. }))
    );
}

#[test]
fn device_fn_cannot_call_port_write() {
    let src = "@device fn bad() { port_write(128, 42) }";
    let errors = check_errors(src);
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, SemanticError::RawPointerInDevice { .. }))
    );
}

// ── Sprint 3.10: KE001/KE002 enforcement ──

#[test]
fn kernel_fn_cannot_call_push() {
    let src = r#"
        @kernel fn bad() {
            let mut arr = [1, 2, 3]
            push(arr, 4)
        }
    "#;
    let errors = check_errors(src);
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, SemanticError::HeapAllocInKernel { .. }))
    );
}

#[test]
fn kernel_fn_cannot_call_to_string() {
    let src = r#"@kernel fn bad() { to_string(42) }"#;
    let errors = check_errors(src);
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, SemanticError::HeapAllocInKernel { .. }))
    );
}

#[test]
fn kernel_fn_cannot_call_pop() {
    let src = r#"
        @kernel fn bad() {
            let mut arr = [1, 2, 3]
            pop(arr)
        }
    "#;
    let errors = check_errors(src);
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, SemanticError::HeapAllocInKernel { .. }))
    );
}

#[test]
fn safe_fn_can_call_push() {
    let src = r#"
        fn ok() {
            let mut arr = [1, 2, 3]
            push(arr, 4)
        }
    "#;
    assert!(check(src).is_ok());
}

#[test]
fn kernel_fn_still_allows_non_heap_builtins() {
    let src = r#"@kernel fn ok() { println(42) }"#;
    assert!(check(src).is_ok());
}

#[test]
fn kernel_fn_ke001_and_ke003_both_detected() {
    let src = r#"
        @device fn infer() -> i64 { 0 }
        @kernel fn bad() {
            push([1], 2)
            infer()
        }
    "#;
    let errors = check_errors(src);
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, SemanticError::HeapAllocInKernel { .. }))
    );
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, SemanticError::DeviceCallInKernel { .. }))
    );
}

// ── S6.1/S6.2 Trait system ──

#[test]
fn trait_def_is_registered() {
    // Trait definition should not produce errors
    let src = r#"
        trait Summary {
            fn summarize(self: str) -> str { "default" }
        }
    "#;
    assert!(check(src).is_ok());
}

#[test]
fn trait_def_duplicate_method_error() {
    let src = r#"
        trait Bad {
            fn method(self: i64) -> i64 { 0 }
            fn method(self: i64) -> i64 { 1 }
        }
    "#;
    let errors = check_errors(src);
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, SemanticError::DuplicateDefinition { .. }))
    );
}

#[test]
fn impl_trait_missing_method_error() {
    let src = r#"
        trait Greetable {
            fn greet(self: str) -> str { "hi" }
            fn farewell(self: str) -> str { "bye" }
        }
        struct Person { name: str }
        impl Greetable for Person {
            fn greet(self: str) -> str { "hello" }
        }
    "#;
    let diagnostics = check_all_diagnostics(src);
    assert!(diagnostics.iter().any(|e| matches!(
        e,
        SemanticError::MissingField {
            field,
            ..
        } if field == "farewell"
    )));
}

#[test]
fn impl_trait_complete_passes() {
    let src = r#"
        trait Greetable {
            fn greet(self: str) -> str { "hi" }
        }
        struct Person { name: str }
        impl Greetable for Person {
            fn greet(self: str) -> str { "hello" }
        }
    "#;
    // No missing method errors
    let diagnostics = check_all_diagnostics(src);
    assert!(
        !diagnostics
            .iter()
            .any(|e| matches!(e, SemanticError::MissingField { .. }))
    );
}

#[test]
fn impl_trait_wrong_param_count_se016() {
    let src = r#"
        trait Adder {
            fn add(self: i64, a: i64, b: i64) -> i64 { 0 }
        }
        struct Calc { x: i64 }
        impl Adder for Calc {
            fn add(self: i64, a: i64) -> i64 { a }
        }
    "#;
    let errors = check_all_diagnostics(src);
    assert!(errors.iter().any(|e| matches!(
        e,
        SemanticError::TraitMethodSignatureMismatch { method, .. } if method == "add"
    )));
}

#[test]
fn impl_trait_wrong_return_type_se016() {
    let src = r#"
        trait Stringify {
            fn to_s(self: i64) -> str { "x" }
        }
        struct Num { val: i64 }
        impl Stringify for Num {
            fn to_s(self: i64) -> i64 { 42 }
        }
    "#;
    let errors = check_all_diagnostics(src);
    assert!(errors.iter().any(|e| matches!(
        e,
        SemanticError::TraitMethodSignatureMismatch { method, detail, .. }
            if method == "to_s" && detail.contains("return type")
    )));
}

#[test]
fn impl_trait_matching_signature_passes() {
    let src = r#"
        trait Doubler {
            fn double(self: i64, x: i64) -> i64 { 0 }
        }
        struct MyDoubler { val: i64 }
        impl Doubler for MyDoubler {
            fn double(self: i64, x: i64) -> i64 { x * 2 }
        }
    "#;
    let errors = check_all_diagnostics(src);
    assert!(
        !errors
            .iter()
            .any(|e| matches!(e, SemanticError::TraitMethodSignatureMismatch { .. }))
    );
}

#[test]
fn generic_fn_passes_through_analyzer() {
    let src = r#"
        fn identity<T>(x: T) -> T { x }
        fn main() -> void { let y = identity(42) }
    "#;
    assert!(check(src).is_ok());
}

// ── Extern function / FFI tests (S7.1) ──────────────────────────────

#[test]
fn extern_fn_with_ffi_safe_types_passes() {
    let src = r#"
        extern fn abs(x: i32) -> i32
        fn main() -> void { let y = 1 }
    "#;
    assert!(check(src).is_ok());
}

#[test]
fn extern_fn_registered_in_symbol_table() {
    let src = r#"
        extern fn abs(x: i32) -> i32
        fn main() -> void { let y = abs(42) }
    "#;
    assert!(check(src).is_ok());
}

#[test]
fn extern_fn_rejects_string_param() {
    let src = r#"
        extern fn bad(s: str) -> i32
        fn main() -> void { let y = 1 }
    "#;
    let result = check(src);
    assert!(result.is_err());
    let errs = result.unwrap_err();
    assert!(
        errs.iter()
            .any(|e| matches!(e, SemanticError::FfiUnsafeType { .. }))
    );
}

#[test]
fn extern_fn_rejects_string_return() {
    let src = r#"
        extern fn bad(x: i32) -> str
        fn main() -> void { let y = 1 }
    "#;
    let result = check(src);
    assert!(result.is_err());
    let errs = result.unwrap_err();
    assert!(
        errs.iter()
            .any(|e| matches!(e, SemanticError::FfiUnsafeType { .. }))
    );
}

#[test]
fn extern_fn_multiple_ffi_safe_params() {
    let src = r#"
        extern fn memcpy(dst: u64, src: u64, n: u64) -> u64
        fn main() -> void { let y = 1 }
    "#;
    assert!(check(src).is_ok());
}

// ── Type inference tests (S8.1) ─────────────────────────────────────

#[test]
fn type_inference_let_int_defaults_to_i64() {
    // `let x = 42` should infer x as i64 (not {integer})
    let src = r#"
        fn takes_i64(x: i64) -> void {}
        fn main() -> void {
            let x = 42
            takes_i64(x)
        }
    "#;
    assert!(check(src).is_ok());
}

#[test]
fn type_inference_let_float_defaults_to_f64() {
    // `let x = 3.14` should infer x as f64 (not {float})
    let src = r#"
        fn takes_f64(x: f64) -> void {}
        fn main() -> void {
            let x = 3.14
            takes_f64(x)
        }
    "#;
    assert!(check(src).is_ok());
}

#[test]
fn type_inference_explicit_annotation_preserved() {
    // `let x: i32 = 42` should use i32, not default to i64
    let src = r#"
        fn takes_i32(x: i32) -> void {}
        fn main() -> void {
            let x: i32 = 42
            takes_i32(x)
        }
    "#;
    assert!(check(src).is_ok());
}

#[test]
fn default_literal_method() {
    assert_eq!(Type::IntLiteral.default_literal(), Type::I64);
    assert_eq!(Type::FloatLiteral.default_literal(), Type::F64);
    assert_eq!(Type::Bool.default_literal(), Type::Bool);
    assert_eq!(Type::Str.default_literal(), Type::Str);
}

// ── Type alias tests (S8.3) ─────────────────────────────────────────

#[test]
fn type_alias_resolves_in_function_signature() {
    let src = r#"
        type Meters = f64
        fn distance(m: Meters) -> Meters { m }
        fn main() -> void {
            let d: f64 = distance(3.14)
        }
    "#;
    assert!(check(src).is_ok());
}

#[test]
fn type_alias_of_alias_resolves() {
    let src = r#"
        type Count = i64
        type Total = Count
        fn sum(a: Total, b: Total) -> Total { a + b }
        fn main() -> void {
            let x: i64 = sum(1, 2)
        }
    "#;
    assert!(check(src).is_ok());
}

#[test]
fn type_alias_transparent_no_mismatch() {
    // Meters and f64 should be interchangeable
    let src = r#"
        type Meters = f64
        fn add_f64(a: f64, b: f64) -> f64 { a + b }
        fn main() -> void {
            let m: Meters = 1.0
            let n: f64 = 2.0
            let total = add_f64(m, n)
        }
    "#;
    assert!(check(src).is_ok());
}

// ── Never type & exhaustiveness (S8.4) ──────────────────────────────

#[test]
fn never_type_in_return_position() {
    // `fn diverge() -> ! { loop {} }` should be accepted
    let src = r#"
        fn diverge() -> ! {
            while true { 0 }
        }
        fn main() -> void { let x = 1 }
    "#;
    assert!(check(src).is_ok());
}

#[test]
fn non_exhaustive_match_detected() {
    let src = r#"
        fn main() -> void {
            let x = 1
            match x {
                1 => 10,
                2 => 20
            }
        }
    "#;
    let result = check(src);
    assert!(result.is_err());
    let errs = result.unwrap_err();
    assert!(
        errs.iter()
            .any(|e| matches!(e, SemanticError::NonExhaustiveMatch { .. }))
    );
}

#[test]
fn exhaustive_match_with_wildcard_passes() {
    let src = r#"
        fn main() -> void {
            let x = 1
            match x {
                1 => 10,
                _ => 0
            }
        }
    "#;
    assert!(check(src).is_ok());
}

// ── v0.4 S2.4: Match exhaustiveness for generic enums ──

#[test]
fn exhaustive_option_match_all_variants() {
    // Match on Option with both Some and None → exhaustive, no error
    let src = r#"
        enum Option<T> { Some(T), None }
        fn check(x: i64) -> i64 {
            let opt = Some(x)
            match opt {
                Some(v) => v,
                None => 0
            }
        }
    "#;
    assert!(check(src).is_ok());
}

#[test]
fn non_exhaustive_option_match_missing_none() {
    // Match on Option with only Some → non-exhaustive
    let src = r#"
        enum Option<T> { Some(T), None }
        fn check(x: i64) -> i64 {
            let opt = Some(x)
            match opt {
                Some(v) => v
            }
        }
    "#;
    let errors = check_errors(src);
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, SemanticError::NonExhaustiveMatch { .. }))
    );
}

#[test]
fn exhaustive_result_match_ok_err() {
    // Match on Result with Ok and Err → exhaustive
    let src = r#"
        enum Result<T, E> { Ok(T), Err(E) }
        fn check(x: i64) -> i64 {
            let r = Ok(x)
            match r {
                Ok(v) => v,
                Err(e) => 0
            }
        }
    "#;
    assert!(check(src).is_ok());
}

#[test]
fn non_exhaustive_result_match_missing_err() {
    // Match on Result with only Ok → non-exhaustive
    let src = r#"
        enum Result<T, E> { Ok(T), Err(E) }
        fn check(x: i64) -> i64 {
            let r = Ok(x)
            match r {
                Ok(v) => v
            }
        }
    "#;
    let errors = check_errors(src);
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, SemanticError::NonExhaustiveMatch { .. }))
    );
}

#[test]
fn exhaustive_user_enum_all_variants() {
    // User-defined enum with all variants matched
    let src = r#"
        enum Color { Red, Green, Blue }
        fn name(c: i64) -> i64 {
            match c {
                Color::Red => 1,
                Color::Green => 2,
                Color::Blue => 3
            }
        }
    "#;
    assert!(check(src).is_ok());
}

#[test]
fn non_exhaustive_user_enum_missing_variant() {
    // User-defined enum missing Blue → non-exhaustive
    let src = r#"
        enum Color { Red, Green, Blue }
        fn name(c: i64) -> i64 {
            match c {
                Color::Red => 1,
                Color::Green => 2
            }
        }
    "#;
    let errors = check_errors(src);
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, SemanticError::NonExhaustiveMatch { .. }))
    );
}

// ── v0.4 S4: Future/Poll type system ──

#[test]
fn poll_enum_exhaustive_ready_pending() {
    // Poll<T> is a built-in enum — matching Ready + Pending is exhaustive
    let src = r#"
        fn check(x: i64) -> i64 {
            match x {
                Ready(v) => v,
                Pending => 0
            }
        }
    "#;
    assert!(check(src).is_ok());
}

#[test]
fn poll_enum_non_exhaustive_missing_pending() {
    // Missing Pending → non-exhaustive
    let src = r#"
        fn check(x: i64) -> i64 {
            match x {
                Ready(v) => v
            }
        }
    "#;
    let errors = check_errors(src);
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, SemanticError::NonExhaustiveMatch { .. }))
    );
}

#[test]
fn await_outside_async_is_error() {
    // v0.7: .await is now allowed in any context (cooperative eval)
    let src = r#"
        fn not_async() -> i64 {
            let x = 42
            x.await
        }
    "#;
    let result = check(src);
    // Should succeed now (no AwaitOutsideAsync error)
    assert!(
        result.is_ok()
            || !result
                .unwrap_err()
                .iter()
                .any(|e| matches!(e, SemanticError::AwaitOutsideAsync { .. }))
    );
}

#[test]
fn await_inside_async_is_ok() {
    // .await inside async fn is valid
    let src = r#"
        async fn inner() -> i64 { 42 }
        async fn outer() -> i64 {
            inner().await
        }
    "#;
    assert!(check(src).is_ok());
}

// ── Move semantics (S9.1-S9.2) ─────────────────────────────────────

#[test]
fn copy_type_not_moved() {
    // i64 is Copy, so `let y = x; println(x)` should work
    let src = r#"
        fn main() -> void {
            let x = 42
            let y = x
            println(x)
        }
    "#;
    assert!(check(src).is_ok());
}

#[test]
fn move_type_use_after_move_detected() {
    // FJARR_LEAK Phase 2 D-FULL (v35.5.0): arrays are Move (affine).
    // `let b = a` consumes a; `len(a)` fires SE024.
    let src = r#"
        fn main() -> void {
            let a: [i64] = [1, 2, 3]
            let b = a
            len(a)
        }
    "#;
    let errors = check_errors(src);
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, SemanticError::UseAfterMoveArray { .. })),
        "expected SE024 UseAfterMoveArray, got: {errors:?}"
    );
}

#[test]
fn move_type_ok_when_not_used_after() {
    // str is Copy now, so this always works. Kept for backward compat.
    let src = r#"
        fn main() -> void {
            let s: str = "hello"
            let t = s
            println(t)
        }
    "#;
    assert!(check(src).is_ok());
}

#[test]
fn fn_call_moves_move_type_arg() {
    // FJARR_LEAK Phase 2 D-FULL (v35.5.0): fn-arg consume of an array
    // marks it Moved; subsequent use fires SE024.
    let src = r#"
        fn consume(a: [i64]) -> void {
            println(len(a))
        }
        fn main() -> void {
            let a: [i64] = [1, 2, 3]
            consume(a)
            len(a)
        }
    "#;
    let errors = check_errors(src);
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, SemanticError::UseAfterMoveArray { .. })),
        "expected SE024 UseAfterMoveArray, got: {errors:?}"
    );
}

#[test]
fn fn_call_copy_type_arg_not_moved() {
    // Passing a copy-type (i64) to a function should NOT mark it moved
    let src = r#"
        fn use_val(x: i64) -> i64 {
            x + 1
        }
        fn main() -> void {
            let x = 42
            use_val(x)
            println(x)
        }
    "#;
    assert!(check(src).is_ok());
}

#[test]
fn fn_call_move_type_ok_when_not_used_after() {
    // Passing a move-type to a function is fine if not used afterward
    let src = r#"
        fn consume(s: str) -> void {
            println(s)
        }
        fn main() -> void {
            let s: str = "hello"
            consume(s)
        }
    "#;
    assert!(check(src).is_ok());
}

// ── Trait bounds ──

#[test]
fn generic_fn_with_known_trait_bound_ok() {
    let src = r#"
        fn max_val<T: PartialOrd>(a: T, b: T) -> T {
            if a > b { a } else { b }
        }
    "#;
    assert!(check(src).is_ok());
}

#[test]
fn generic_fn_with_multiple_bounds_ok() {
    let src = r#"
        fn display_and_compare<T: Display + PartialEq>(a: T, b: T) -> void {
            println(a)
        }
    "#;
    assert!(check(src).is_ok());
}

#[test]
fn generic_fn_unknown_trait_bound_error() {
    let src = r#"
        fn sort<T: NonexistentTrait>(arr: T) -> T {
            arr
        }
    "#;
    let errors = check_errors(src);
    assert!(errors.iter().any(
        |e| matches!(e, SemanticError::UnknownTrait { name, .. } if name == "NonexistentTrait")
    ));
}

#[test]
fn builtin_traits_registered() {
    let tc = TypeChecker::new();
    // Built-in traits should be registered
    assert!(tc.traits.contains_key("Display"));
    assert!(tc.traits.contains_key("Clone"));
    assert!(tc.traits.contains_key("PartialEq"));
    assert!(tc.traits.contains_key("Ord"));
    assert!(tc.traits.contains_key("Debug"));
    assert!(tc.traits.contains_key("Default"));
    assert!(tc.traits.contains_key("Hash"));
    assert!(tc.traits.contains_key("Copy"));
}

#[test]
fn primitive_types_implement_builtin_traits() {
    let tc = TypeChecker::new();
    // i64 should implement all common traits
    assert!(tc.type_satisfies_trait("i64", "Display"));
    assert!(tc.type_satisfies_trait("i64", "Clone"));
    assert!(tc.type_satisfies_trait("i64", "Copy"));
    assert!(tc.type_satisfies_trait("i64", "PartialEq"));
    assert!(tc.type_satisfies_trait("i64", "Ord"));

    // bool
    assert!(tc.type_satisfies_trait("bool", "Display"));
    assert!(tc.type_satisfies_trait("bool", "Eq"));

    // String
    assert!(tc.type_satisfies_trait("String", "Display"));
    assert!(tc.type_satisfies_trait("String", "Clone"));
}

#[test]
fn user_defined_trait_with_bound_ok() {
    let src = r#"
        trait Printable {
            fn to_str() -> str { "default" }
        }
        fn show<T: Printable>(x: T) -> void {
            println(x)
        }
    "#;
    assert!(check(src).is_ok());
}

#[test]
fn generic_fn_no_bounds_still_works() {
    let src = r#"
        fn identity<T>(x: T) -> T {
            x
        }
    "#;
    assert!(check(src).is_ok());
}

// ── S9.4 Move semantics in pattern matching ──

#[test]
fn match_destructure_moves_subject() {
    // Destructuring a move-type via pattern matching should mark it moved
    let src = r#"
        fn main() -> void {
            let x: str = "hello"
            match x {
                _ => println("matched")
            }
            println(x)
        }
    "#;
    // str is a move type. However, wildcard `_` doesn't destructure,
    // so this should NOT trigger use-after-move.
    assert!(check(src).is_ok());
}

#[test]
fn match_enum_destructure_moves_subject() {
    // FJARR_LEAK Phase 2 D-FULL (v35.5.0): match-with-destructure consumes
    // the subject; subsequent use fires SE024.
    let src = r#"
        fn main() -> void {
            let x: [i64] = [1, 2, 3]
            match x {
                Some(inner) => println("got")
                _ => println("none")
            }
            len(x)
        }
    "#;
    let errors = check_errors(src);
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, SemanticError::UseAfterMoveArray { .. })),
        "expected SE024 UseAfterMoveArray, got: {errors:?}"
    );
}

#[test]
fn match_copy_type_no_move() {
    // Copy types (i64) should not be moved by pattern matching
    let src = r#"
        fn main() -> void {
            let x: i64 = 42
            match x {
                0 => println("zero")
                _ => println("other")
            }
            println(x)
        }
    "#;
    assert!(check(src).is_ok());
}

// ── Unreachable code after diverging expression ──────────────────

#[test]
fn unreachable_after_return_tail_expr() {
    // Tail expression after return should be flagged
    let src = r#"
        fn f() -> i64 {
            return 1
            99
        }
    "#;
    let diags = check_all_diagnostics(src);
    assert!(
        diags
            .iter()
            .any(|e| matches!(e, SemanticError::UnreachableCode { .. }))
    );
}

#[test]
fn unreachable_after_return_with_multiple_stmts() {
    let src = r#"
        fn f() -> i64 {
            return 1
            let x = 2
            x
        }
    "#;
    let diags = check_all_diagnostics(src);
    assert!(
        diags
            .iter()
            .any(|e| matches!(e, SemanticError::UnreachableCode { .. }))
    );
}

// ── self/&self validation ──────────────────────────────────────────

#[test]
fn self_in_free_fn_rejected() {
    let src = r#"
        fn bad(self) -> void {
            0
        }
    "#;
    let result = check(src);
    assert!(result.is_err());
}

#[test]
fn self_in_impl_method_ok() {
    let src = r#"
        struct Point { x: f64, y: f64 }
        impl Point {
            fn get_x(self) -> f64 {
                self.x
            }
        }
    "#;
    assert!(check(src).is_ok());
}

#[test]
fn self_must_be_first_param() {
    let src = r#"
        struct Foo { val: i64 }
        impl Foo {
            fn bad(x: i64, self) -> void {
                0
            }
        }
    "#;
    let result = check(src);
    assert!(result.is_err());
}

// ── S10.1: Immutable borrows ───────────────────────────────────────

#[test]
fn immutable_borrow_returns_ref_type() {
    let src = r#"
        fn main() -> void {
            let x = 42
            let r = &x
        }
    "#;
    assert!(check(src).is_ok());
}

#[test]
fn multiple_immutable_borrows_ok() {
    let src = r#"
        fn main() -> void {
            let x = 42
            let r1 = &x
            let r2 = &x
        }
    "#;
    assert!(check(src).is_ok());
}

#[test]
fn move_while_immutably_borrowed_me003() {
    // FJARR_LEAK Phase 2 D-FULL (v35.5.0): arrays are affine — moving
    // while a live borrow exists fires ME003 MoveWhileBorrowed.
    let src = r#"
        fn consume(a: [i64]) -> void { println(len(a)) }
        fn main() -> void {
            let a: [i64] = [1, 2, 3]
            let r = &a
            consume(a)
            println(r)
        }
    "#;
    let errors = check_errors(src);
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, SemanticError::MoveWhileBorrowed { .. })),
        "expected ME003 MoveWhileBorrowed, got: {errors:?}"
    );
}

#[test]
fn nll_move_after_borrow_last_use_ok() {
    // NLL: r is NOT used after consume(s), so borrow is dead → move OK
    let src = r#"
        fn consume(s: str) -> void { println(s) }
        fn main() -> void {
            let s: str = "hello"
            let r = &s
            println(r)
            consume(s)
        }
    "#;
    assert!(check(src).is_ok());
}

#[test]
fn move_after_borrow_scope_ends_ok() {
    let src = r#"
        fn consume(s: str) -> void { println(s) }
        fn main() -> void {
            let s: str = "hello"
            {
                let r = &s
            }
            consume(s)
        }
    "#;
    assert!(check(src).is_ok());
}

// ── S10.2: Mutable borrows ─────────────────────────────────────────

#[test]
fn exclusive_mut_borrow_rejects_second_mut_me004() {
    // r1 is used AFTER r2 creation, so r1's borrow is live → ME004
    let src = r#"
        fn main() -> void {
            let mut x = 42
            let r1 = &mut x
            let r2 = &mut x
            println(r1)
        }
    "#;
    let result = check(src);
    assert!(result.is_err());
    let errs = result.unwrap_err();
    assert!(
        errs.iter()
            .any(|e| matches!(e, SemanticError::MutBorrowConflict { name, .. } if name == "x"))
    );
}

#[test]
fn nll_mut_reborrow_after_last_use_ok() {
    // NLL: r1's last use is before r2 creation → borrow released → OK
    let src = r#"
        fn main() -> void {
            let mut x = 42
            let r1 = &mut x
            println(r1)
            let r2 = &mut x
            println(r2)
        }
    "#;
    assert!(check(src).is_ok());
}

#[test]
fn mut_borrow_rejects_imm_borrow_me005() {
    // r1 (&mut) is used AFTER r2 (&) creation → ME005
    let src = r#"
        fn main() -> void {
            let mut x = 42
            let r1 = &mut x
            let r2 = &x
            println(r1)
        }
    "#;
    let result = check(src);
    assert!(result.is_err());
    let errs = result.unwrap_err();
    assert!(
        errs.iter()
            .any(|e| matches!(e, SemanticError::ImmBorrowConflict { name, .. } if name == "x"))
    );
}

#[test]
fn imm_borrow_rejects_mut_borrow_me004() {
    // r1 (&) is used AFTER r2 (&mut) creation → ME004
    let src = r#"
        fn main() -> void {
            let mut x = 42
            let r1 = &x
            let r2 = &mut x
            println(r1)
        }
    "#;
    let result = check(src);
    assert!(result.is_err());
    let errs = result.unwrap_err();
    assert!(
        errs.iter()
            .any(|e| matches!(e, SemanticError::MutBorrowConflict { name, .. } if name == "x"))
    );
}

#[test]
fn nll_imm_then_mut_after_last_use_ok() {
    // NLL: r1 (&) last use is println(r1), before r2 (&mut) → OK
    let src = r#"
        fn main() -> void {
            let mut x = 42
            let r1 = &x
            println(r1)
            let r2 = &mut x
            println(r2)
        }
    "#;
    assert!(check(src).is_ok());
}

#[test]
fn nll_unused_borrow_immediately_dead() {
    // NLL: r is never used, so borrow is immediately dead → mut borrow OK
    let src = r#"
        fn main() -> void {
            let mut x = 42
            let r = &x
            let r2 = &mut x
            println(r2)
        }
    "#;
    assert!(check(src).is_ok());
}

// ── S10.3: Borrow scoping ──────────────────────────────────────────

#[test]
fn mut_borrow_after_imm_scope_ends_ok() {
    let src = r#"
        fn main() -> void {
            let mut x = 42
            {
                let r = &x
            }
            let r2 = &mut x
        }
    "#;
    assert!(check(src).is_ok());
}

#[test]
fn imm_borrow_after_mut_scope_ends_ok() {
    let src = r#"
        fn main() -> void {
            let mut x = 42
            {
                let r = &mut x
            }
            let r2 = &x
        }
    "#;
    assert!(check(src).is_ok());
}

// ── S10.4: NLL borrow checker ────────────────────────────────────────

#[test]
fn nll_borrow_live_across_if_branch() {
    // r is used in if branch, so it's live at x = 10 (if x=10 comes after if)
    // Actually r's last use is inside the if, so after if, r is dead → OK
    let src = r#"
        fn main() -> void {
            let mut x = 42
            let r = &x
            if true {
                println(r)
            }
            x = 10
        }
    "#;
    assert!(check(src).is_ok());
}

#[test]
fn nll_borrow_still_live_in_loop() {
    // r is used inside while loop → extended to loop end → still live
    let src = r#"
        fn main() -> void {
            let mut x = 42
            let r = &x
            let mut i = 0
            while i < 3 {
                println(r)
                i = i + 1
            }
            x = 10
        }
    "#;
    // After the loop, r's uses were extended to loop end.
    // x = 10 is after the loop, so r should be dead → OK
    assert!(check(src).is_ok());
}

#[test]
fn nll_reassign_after_last_use_same_scope() {
    // Classic NLL pattern: borrow used then reassign in same scope
    let src = r#"
        fn main() -> void {
            let mut x = 42
            let r = &x
            println(r)
            x = 100
            println(x)
        }
    "#;
    assert!(check(src).is_ok());
}

#[test]
fn nll_reassign_before_use_still_error() {
    // Reassign x BEFORE using r → error (r is live at x = 100)
    let src = r#"
        fn main() -> void {
            let mut x = 42
            let r = &x
            x = 100
            println(r)
        }
    "#;
    let result = check(src);
    assert!(result.is_err());
}

// ── B.1: Tensor shape type tests ──

#[test]
fn b1_tensor_type_display_known_dims() {
    let t = Type::Tensor {
        element: Box::new(Type::F64),
        dims: vec![Some(3), Some(4)],
    };
    assert_eq!(t.display_name(), "Tensor<f64>[3, 4]");
}

#[test]
fn b1_tensor_type_display_dynamic_dims() {
    let t = Type::Tensor {
        element: Box::new(Type::F32),
        dims: vec![None, Some(10)],
    };
    assert_eq!(t.display_name(), "Tensor<f32>[*, 10]");
}

#[test]
fn b1_tensor_is_tensor() {
    let t = Type::Tensor {
        element: Box::new(Type::F64),
        dims: vec![Some(3)],
    };
    assert!(t.is_tensor());
    assert!(!Type::I64.is_tensor());
}

#[test]
fn b1_tensor_compatible_same_shape() {
    let a = Type::Tensor {
        element: Box::new(Type::F64),
        dims: vec![Some(3), Some(4)],
    };
    let b = Type::Tensor {
        element: Box::new(Type::F64),
        dims: vec![Some(3), Some(4)],
    };
    assert!(a.is_compatible(&b));
}

#[test]
fn b1_tensor_compatible_dynamic_dim() {
    let a = Type::Tensor {
        element: Box::new(Type::F64),
        dims: vec![None, Some(4)],
    };
    let b = Type::Tensor {
        element: Box::new(Type::F64),
        dims: vec![Some(3), Some(4)],
    };
    assert!(a.is_compatible(&b));
}

#[test]
fn b1_tensor_incompatible_different_shape() {
    let a = Type::Tensor {
        element: Box::new(Type::F64),
        dims: vec![Some(3), Some(4)],
    };
    let b = Type::Tensor {
        element: Box::new(Type::F64),
        dims: vec![Some(5), Some(4)],
    };
    assert!(!a.is_compatible(&b));
}

#[test]
fn b1_tensor_incompatible_different_ndims() {
    let a = Type::Tensor {
        element: Box::new(Type::F64),
        dims: vec![Some(3)],
    };
    let b = Type::Tensor {
        element: Box::new(Type::F64),
        dims: vec![Some(3), Some(4)],
    };
    assert!(!a.is_compatible(&b));
}

#[test]
fn b1_tensor_incompatible_different_element() {
    let a = Type::Tensor {
        element: Box::new(Type::F64),
        dims: vec![Some(3)],
    };
    let b = Type::Tensor {
        element: Box::new(Type::I64),
        dims: vec![Some(3)],
    };
    assert!(!a.is_compatible(&b));
}

#[test]
fn b1_matmul_shape_valid() {
    let a = Type::Tensor {
        element: Box::new(Type::F64),
        dims: vec![Some(3), Some(4)],
    };
    let b = Type::Tensor {
        element: Box::new(Type::F64),
        dims: vec![Some(4), Some(5)],
    };
    let result = a.matmul_shape(&b).unwrap();
    assert_eq!(
        result,
        Type::Tensor {
            element: Box::new(Type::F64),
            dims: vec![Some(3), Some(5)],
        }
    );
}

#[test]
fn b1_matmul_shape_dynamic_k() {
    let a = Type::Tensor {
        element: Box::new(Type::F64),
        dims: vec![Some(3), None],
    };
    let b = Type::Tensor {
        element: Box::new(Type::F64),
        dims: vec![None, Some(5)],
    };
    let result = a.matmul_shape(&b).unwrap();
    assert_eq!(
        result,
        Type::Tensor {
            element: Box::new(Type::F64),
            dims: vec![Some(3), Some(5)],
        }
    );
}

#[test]
fn b1_matmul_shape_k_mismatch() {
    let a = Type::Tensor {
        element: Box::new(Type::F64),
        dims: vec![Some(3), Some(4)],
    };
    let b = Type::Tensor {
        element: Box::new(Type::F64),
        dims: vec![Some(7), Some(5)],
    };
    assert!(a.matmul_shape(&b).is_none());
}

#[test]
fn b1_matmul_shape_not_2d() {
    let a = Type::Tensor {
        element: Box::new(Type::F64),
        dims: vec![Some(3)],
    };
    let b = Type::Tensor {
        element: Box::new(Type::F64),
        dims: vec![Some(3), Some(5)],
    };
    assert!(a.matmul_shape(&b).is_none());
}

#[test]
fn b1_elementwise_shape_valid() {
    let a = Type::Tensor {
        element: Box::new(Type::F64),
        dims: vec![Some(3), Some(4)],
    };
    let b = Type::Tensor {
        element: Box::new(Type::F64),
        dims: vec![Some(3), Some(4)],
    };
    let result = a.elementwise_shape(&b).unwrap();
    assert_eq!(
        result,
        Type::Tensor {
            element: Box::new(Type::F64),
            dims: vec![Some(3), Some(4)],
        }
    );
}

#[test]
fn b1_elementwise_shape_mismatch() {
    let a = Type::Tensor {
        element: Box::new(Type::F64),
        dims: vec![Some(3), Some(4)],
    };
    let b = Type::Tensor {
        element: Box::new(Type::F64),
        dims: vec![Some(3), Some(5)],
    };
    assert!(a.elementwise_shape(&b).is_none());
}

#[test]
fn b1_resolve_tensor_type_annotation() {
    // Tensor<f64>[3, 4] type annotation resolves to Type::Tensor
    let src = r#"
        fn process(t: Tensor<f64>[3, 4]) -> void {
            println(t)
        }
    "#;
    assert!(check(src).is_ok());
}

#[test]
fn b1_tensor_zeros_shape_inferred() {
    // tensor_zeros(3, 4) should type-check as Tensor with dynamic shape
    let src = "let t = tensor_zeros(3, 4)";
    assert!(check(src).is_ok());
}

#[test]
fn b1_tensor_matmul_shape_check() {
    // tensor_matmul should accept two tensor arguments
    let src = r#"
        let a = tensor_zeros(3, 4)
        let b = tensor_zeros(4, 5)
        let c = tensor_matmul(a, b)
    "#;
    assert!(check(src).is_ok());
}

#[test]
fn b1_tensor_type_annotation_tensor_param() {
    // Function with Tensor type param accepts tensor values
    let src = r#"
        fn transform(t: Tensor<f64>[*, *]) -> void {
            println(t)
        }
        let a = tensor_zeros(3, 4)
        transform(a)
    "#;
    assert!(check(src).is_ok());
}

// ── B.4: Tensor shape hardening tests ──

#[test]
fn b4_matmul_operator_annotated_valid() {
    // @ operator with compatible annotated tensor params
    let src = r#"
        fn f(a: Tensor<f64>[3, 4], b: Tensor<f64>[4, 5]) -> void {
            let c = a @ b
            println(c)
        }
    "#;
    assert!(check(src).is_ok());
}

#[test]
fn b4_matmul_operator_annotated_k_mismatch() {
    // @ operator with incompatible K dims → TE001
    let src = r#"
        fn f(a: Tensor<f64>[3, 4], b: Tensor<f64>[7, 5]) -> void {
            let c = a @ b
            println(c)
        }
    "#;
    let errors = check_errors(src);
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, SemanticError::TensorShapeMismatch { .. }))
    );
}

#[test]
fn b4_matmul_operator_dynamic_no_error() {
    // @ with dynamic tensors (from builtins) → no shape error
    let src = r#"
        let a = tensor_zeros(3, 4)
        let b = tensor_zeros(4, 5)
        let c = a @ b
    "#;
    assert!(check(src).is_ok());
}

#[test]
fn b4_matmul_operator_1d_error() {
    // @ with 1D tensor → TE001 (matmul requires 2D)
    let src = r#"
        fn f(a: Tensor<f64>[3], b: Tensor<f64>[3, 5]) -> void {
            let c = a @ b
            println(c)
        }
    "#;
    let errors = check_errors(src);
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, SemanticError::TensorShapeMismatch { .. }))
    );
}

#[test]
fn b4_elementwise_annotated_valid() {
    // tensor + tensor with same annotated shapes → OK
    let src = r#"
        fn f(a: Tensor<f64>[3, 4], b: Tensor<f64>[3, 4]) -> void {
            let c = a + b
            println(c)
        }
    "#;
    assert!(check(src).is_ok());
}

#[test]
fn b4_elementwise_annotated_mismatch() {
    // tensor + tensor with different annotated shapes → TE001
    let src = r#"
        fn f(a: Tensor<f64>[3, 4], b: Tensor<f64>[5, 6]) -> void {
            let c = a + b
            println(c)
        }
    "#;
    let errors = check_errors(src);
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, SemanticError::TensorShapeMismatch { .. }))
    );
}

#[test]
fn b4_elementwise_dynamic_bypass() {
    // P1 (Compass §6.3) update: constructors with literal args are now
    // concrete, so [3,4] + [5,6] errors at check time (was the dynamic
    // bypass pre-P1). The bypass still holds for non-literal dims.
    let src = r#"
        let a = tensor_zeros(3, 4)
        let b = tensor_zeros(5, 6)
        let c = a + b
    "#;
    assert!(has_shape_mismatch(&check_errors(src)));

    let dynamic = r#"
        fn f(n: i64) -> void {
            let a = tensor_zeros(n, 4)
            let b = tensor_zeros(5, 6)
            let c = a + b
            println(c)
        }
    "#;
    assert!(check(dynamic).is_ok());
}

#[test]
fn b4_nested_tensor_type_rejected() {
    // Tensor<Tensor<f64>[3]>[2] → error
    let src = r#"
        fn f(t: Tensor<Tensor<f64>[3]>[2]) -> void {
            println(t)
        }
    "#;
    let errors = check_errors(src);
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, SemanticError::TypeMismatch { .. }))
    );
}

#[test]
fn b4_elementwise_sub_annotated_mismatch() {
    // tensor - tensor with different shapes → TE001
    let src = r#"
        fn f(a: Tensor<f64>[2, 3], b: Tensor<f64>[4, 3]) -> void {
            let c = a - b
            println(c)
        }
    "#;
    let errors = check_errors(src);
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, SemanticError::TensorShapeMismatch { .. }))
    );
}

#[test]
fn b4_matmul_result_shape_propagated() {
    // @ result shape: [3,4] @ [4,5] → [3,5]
    let a = Type::Tensor {
        element: Box::new(Type::F64),
        dims: vec![Some(3), Some(4)],
    };
    let b = Type::Tensor {
        element: Box::new(Type::F64),
        dims: vec![Some(4), Some(5)],
    };
    let result = a.matmul_shape(&b).unwrap();
    if let Type::Tensor { dims, .. } = result {
        assert_eq!(dims, vec![Some(3), Some(5)]);
    } else {
        panic!("expected Tensor type");
    }
}

// ── P1 (Compass §6.3): shape-aware tensor builtins ──
// See docs/TENSOR_SHAPE_CT_PLAN.md §1 + TENSOR_SHAPE_CT_B0_FINDINGS.md.
// Constructors with literal args produce concrete dims; matmul/reshape/
// elementwise call forms check + propagate shapes at compile time.

fn has_shape_mismatch(errors: &[SemanticError]) -> bool {
    errors
        .iter()
        .any(|e| matches!(e, SemanticError::TensorShapeMismatch { .. }))
}

#[test]
fn p1_matmul_call_mismatch_errors_at_check() {
    // B0 probe P1: previously passed `fj check`, died at runtime.
    let src = r#"
        let a = zeros(2, 3)
        let b = zeros(4, 5)
        let c = matmul(a, b)
    "#;
    assert!(has_shape_mismatch(&check_errors(src)));
}

#[test]
fn p1_matmul_call_matching_ok() {
    let src = r#"
        let a = zeros(2, 3)
        let b = zeros(3, 5)
        let c = matmul(a, b)
    "#;
    assert!(check(src).is_ok());
}

#[test]
fn p1_matmul_result_propagates_through_chain() {
    // matmul result [2,4]; second matmul inner 4 != 5 must error.
    let src = r#"
        let c = matmul(zeros(2, 3), zeros(3, 4))
        let d = matmul(c, zeros(5, 6))
    "#;
    assert!(has_shape_mismatch(&check_errors(src)));
}

#[test]
fn p1_at_operator_unannotated_mismatch_errors() {
    // B0 P5 required manual annotations; constructors now feed the @ path.
    let src = r#"
        let c = zeros(2, 3) @ zeros(4, 5)
    "#;
    assert!(has_shape_mismatch(&check_errors(src)));
}

#[test]
fn p1_eye_is_square() {
    let src = r#"
        let c = matmul(eye(3), zeros(4, 5))
    "#;
    assert!(has_shape_mismatch(&check_errors(src)));
}

#[test]
fn p1_transpose_swaps_dims_ok_and_err() {
    let ok = r#"
        let c = matmul(transpose(zeros(3, 2)), zeros(3, 7))
    "#;
    assert!(check(ok).is_ok());
    let err = r#"
        let c = matmul(transpose(zeros(3, 2)), zeros(2, 7))
    "#;
    assert!(has_shape_mismatch(&check_errors(err)));
}

#[test]
fn p1_reshape_element_count_mismatch_errors() {
    let src = r#"
        let r = reshape(zeros(2, 3), [4, 2])
    "#;
    assert!(has_shape_mismatch(&check_errors(src)));
}

#[test]
fn p1_reshape_result_shape_is_concrete() {
    // reshape → [3,2]; matmul inner 2 != 5 proves concreteness.
    let src = r#"
        let r = reshape(zeros(2, 3), [3, 2])
        let c = matmul(r, zeros(5, 5))
    "#;
    assert!(has_shape_mismatch(&check_errors(src)));
}

#[test]
fn p1_elementwise_mismatch_errors() {
    let src = r#"
        let c = tensor_add(zeros(2, 3), zeros(2, 4))
    "#;
    assert!(has_shape_mismatch(&check_errors(src)));
}

#[test]
fn p1_activation_preserves_shape() {
    // relu output keeps [2,3]; matmul inner 3 != 4 must error.
    let src = r#"
        let c = matmul(relu(zeros(2, 3)), zeros(4, 5))
    "#;
    assert!(has_shape_mismatch(&check_errors(src)));
}

#[test]
fn p1_dynamic_args_stay_gradual_ok() {
    // Non-literal dims → dynamic tensor → no false positives.
    let src = r#"
        fn f(n: i64) -> void {
            let a = zeros(n, 3)
            let c = matmul(a, zeros(9, 9))
            println(c)
        }
    "#;
    assert!(check(src).is_ok());
}

#[test]
fn p1_param_boundary_concrete_mismatch_errors() {
    // B0 probe P4: zeros(2,2) into Tensor<f64>[3,3] param must fail.
    let src = r#"
        fn take(t: Tensor<f64>[3, 3]) -> void {
            println(t)
        }
        take(zeros(2, 2))
    "#;
    let errors = check_errors(src);
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, SemanticError::TypeMismatch { .. }))
    );
}

// ── P2 (Compass §6.3): boundary + return-position enforcement ──
// See docs/TENSOR_SHAPE_CT_PLAN.md §2.

#[test]
fn p2_tensor_compat_matrix() {
    // P2.1 — table-driven codification of the Tensor is_compatible
    // matrix: known×known equal, known×dyn gradual, rank mismatch.
    let t = |dims: Vec<Option<u64>>| Type::Tensor {
        element: Box::new(Type::F64),
        dims,
    };
    let known = t(vec![Some(2), Some(3)]);
    let known_other = t(vec![Some(4), Some(5)]);
    let rank3 = t(vec![Some(2), Some(3), Some(4)]);
    let dyn_rank = t(vec![]);
    let partial = t(vec![None, Some(3)]);
    let partial_conflict = t(vec![None, Some(4)]);
    let f32_known = Type::Tensor {
        element: Box::new(Type::F32),
        dims: vec![Some(2), Some(3)],
    };

    assert!(known.is_compatible(&known)); // 1: same known
    assert!(!known.is_compatible(&known_other)); // 2: dim conflict
    assert!(!known.is_compatible(&rank3)); // 3: rank mismatch
    assert!(known.is_compatible(&dyn_rank)); // 4a: known×dyn gradual
    assert!(dyn_rank.is_compatible(&known)); // 4b: dyn×known gradual
    assert!(partial.is_compatible(&known)); // 5: wildcard dim matches
    assert!(!partial.is_compatible(&partial_conflict)); // 6: Some(3)×Some(4) at dim 1 conflicts
    assert!(dyn_rank.is_compatible(&dyn_rank)); // 7: dyn×dyn
    assert!(!known.is_compatible(&f32_known)); // 8: element mismatch
    assert!(!partial.is_compatible(&rank3)); // 9: partial×rank mismatch
}

#[test]
fn p2_return_tail_shape_mismatch_errors() {
    // Already enforced pre-P2 (body-type path) — kept as regression.
    let src = r#"
        fn f() -> Tensor<f64>[2, 3] {
            zeros(3, 2)
        }
    "#;
    let errors = check_errors(src);
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, SemanticError::TypeMismatch { .. }))
    );
}

#[test]
fn p2_return_stmt_shape_mismatch_errors() {
    // B0-class gap: explicit `return` was never checked against the
    // declared return type.
    let src = r#"
        fn f() -> Tensor<f64>[2, 3] {
            return zeros(9, 9)
        }
    "#;
    let errors = check_errors(src);
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, SemanticError::TypeMismatch { .. }))
    );
}

#[test]
fn p2_return_stmt_scalar_mismatch_errors() {
    // The gap was general, not tensor-specific: `return "hello"` from
    // a -> i64 fn passed `fj check` before P2.
    let src = r#"
        fn f() -> i64 {
            return "hello"
        }
    "#;
    let errors = check_errors(src);
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, SemanticError::TypeMismatch { .. }))
    );
}

#[test]
fn p2_return_stmt_branch_mismatch_errors() {
    let src = r#"
        fn f(x: i64) -> Tensor<f64>[2, 3] {
            if x > 0 {
                return zeros(9, 9)
            }
            zeros(2, 3)
        }
    "#;
    let errors = check_errors(src);
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, SemanticError::TypeMismatch { .. }))
    );
}

#[test]
fn p2_return_stmt_matching_ok() {
    let src = r#"
        fn f(x: i64) -> Tensor<f64>[2, 3] {
            if x > 0 {
                return zeros(2, 3)
            }
            zeros(2, 3)
        }
    "#;
    assert!(check(src).is_ok());
}

#[test]
fn p2_return_stmt_dynamic_shape_gradual_ok() {
    // Non-literal dims stay dynamic → compatible with declared shape.
    let src = r#"
        fn f(n: i64) -> Tensor<f64>[2, 3] {
            return zeros(n, 3)
        }
    "#;
    assert!(check(src).is_ok());
}

#[test]
fn p2_return_in_closure_isolated_from_outer_fn() {
    // A `return` inside a closure must not be compared against the
    // enclosing fn's declared return type.
    let src = r#"
        fn f() -> i64 {
            let g = |x: i64| {
                return "s"
            }
            println(g(1))
            return 42
        }
    "#;
    let result = check(src);
    if let Err(errors) = result {
        assert!(
            !errors
                .iter()
                .any(|e| matches!(e, SemanticError::TypeMismatch { .. })),
            "closure return wrongly checked against outer fn: {errors:?}"
        );
    }
}

// ── P3 (Compass §6.3 / D3a): symbolic dims at fn boundaries ──
// `Tensor<f64>[B, I]` — uppercase ident in a dim slot is a symbolic
// dimension scoped to the fn signature, unified per call site (TE011).
// Decision: docs/decisions/2026-06-12-tensor-shape-ct.md.

const DENSE_FN: &str = r#"
    fn dense(x: Tensor<f64>[B, I], w: Tensor<f64>[I, O]) -> Tensor<f64>[B, O] {
        matmul(x, w)
    }
"#;

#[test]
fn p3_symbolic_conflict_errors_te011() {
    // I binds to 10 from x, then 11 from w → TE011 naming the symbol.
    let src = format!(
        "{DENSE_FN}
        let r = dense(zeros(4, 10), zeros(11, 2))"
    );
    let errors = check_errors(&src);
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, SemanticError::SymbolicDimMismatch { .. })),
        "expected SymbolicDimMismatch, got: {errors:?}"
    );
}

#[test]
fn p3_symbolic_match_ok_and_return_substitutes() {
    // B=4, I=10, O=2 → r: Tensor<f64>[4, 2]; proving substitution:
    // r @ [9,9] must shape-error (inner 2 != 9).
    let src = format!(
        "{DENSE_FN}
        let r = dense(zeros(4, 10), zeros(10, 2))
        let bad = r @ zeros(9, 9)"
    );
    let errors = check_errors(&src);
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, SemanticError::TensorShapeMismatch { .. })),
        "return substitution did not propagate concrete dims: {errors:?}"
    );
    // And the matching call alone is clean.
    let clean = format!(
        "{DENSE_FN}
        let r = dense(zeros(4, 10), zeros(10, 2))
        println(r)"
    );
    assert!(check(&clean).is_ok());
}

#[test]
fn p3_symbolic_dynamic_args_stay_gradual() {
    // Dynamic dims bind nothing — no false TE011.
    let src = format!(
        "{DENSE_FN}
        fn caller(n: i64) -> void {{
            let r = dense(zeros(n, 10), zeros(10, 2))
            println(r)
        }}"
    );
    assert!(check(&src).is_ok());
}

#[test]
fn p3_symbolic_bindings_are_per_call_site() {
    // Different B/O across two calls must not cross-contaminate.
    let src = format!(
        "{DENSE_FN}
        let r1 = dense(zeros(4, 10), zeros(10, 2))
        let r2 = dense(zeros(7, 10), zeros(10, 3))
        println(r1)
        println(r2)"
    );
    assert!(check(&src).is_ok());
}

#[test]
fn p3_mixed_symbolic_and_known_dims() {
    // Known dim in the same annotation still enforced by SE004 path.
    let src = r#"
        fn g(x: Tensor<f64>[B, 8]) -> void {
            println(x)
        }
        g(zeros(2, 9))
    "#;
    let errors = check_errors(src);
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, SemanticError::TypeMismatch { .. })),
        "known dim next to symbolic must still enforce: {errors:?}"
    );
}

// ── F.4: Missing builtin registration tests ──

#[test]
fn f4_tensor_detach_registered() {
    let src = r#"
        let t = tensor_zeros(2, 3)
        let d = tensor_detach(t)
    "#;
    assert!(check(src).is_ok(), "tensor_detach should be registered");
}

#[test]
fn f4_tensor_clear_tape_registered() {
    let src = r#"
        tensor_clear_tape()
    "#;
    assert!(check(src).is_ok(), "tensor_clear_tape should be registered");
}

#[test]
fn f4_tensor_no_grad_registered() {
    let src = r#"
        tensor_no_grad_begin()
        tensor_no_grad_end()
    "#;
    assert!(check(src).is_ok(), "tensor_no_grad should be registered");
}

// ── F.5: Cast expression type validation tests ──

#[test]
fn f5_cast_int_to_float() {
    let src = r#"
        let x: f64 = 42 as f64
    "#;
    assert!(check(src).is_ok(), "int as f64 should be valid");
}

#[test]
fn f5_cast_float_to_int() {
    let src = r#"
        let x: i64 = 3.14 as i64
    "#;
    assert!(check(src).is_ok(), "float as i64 should be valid");
}

#[test]
fn f5_cast_bool_to_int() {
    let src = r#"
        let x: i64 = true as i64
    "#;
    assert!(check(src).is_ok(), "bool as i64 should be valid");
}

#[test]
fn f5_cast_int_to_bool() {
    let src = r#"
        let x: bool = 1 as bool
    "#;
    assert!(check(src).is_ok(), "int as bool should be valid");
}

// ── F.6: Missing method registration tests ──

#[test]
fn f6_string_trim_start() {
    let src = r#"
        let s = "  hello  "
        let t = s.trim_start()
    "#;
    assert!(check(src).is_ok(), "trim_start should be registered");
}

#[test]
fn f6_string_trim_end() {
    let src = r#"
        let s = "  hello  "
        let t = s.trim_end()
    "#;
    assert!(check(src).is_ok(), "trim_end should be registered");
}

#[test]
fn f6_string_chars() {
    let src = r#"
        let s = "hello"
        let c = s.chars()
    "#;
    assert!(check(src).is_ok(), "chars should be registered");
}

#[test]
fn f6_string_repeat() {
    let src = r#"
        let s = "abc"
        let r = s.repeat(3)
    "#;
    assert!(check(src).is_ok(), "repeat should be registered");
}

// ── S14.3: Inline assembly context checks ──

#[test]
fn asm_rejected_in_safe_context() {
    let src = r#"
        fn main() {
            asm!("nop")
        }
    "#;
    let errors = check_errors(src);
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, SemanticError::AsmInSafeContext { .. })),
        "asm! should be rejected in @safe context"
    );
}

#[test]
fn asm_rejected_in_device_context() {
    let src = r#"
        @device
        fn compute() {
            asm!("nop")
        }
    "#;
    let errors = check_errors(src);
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, SemanticError::AsmInDeviceContext { .. })),
        "asm! should be rejected in @device context"
    );
}

#[test]
fn asm_allowed_in_kernel_context() {
    let src = r#"
        @kernel
        fn handler() {
            asm!("nop")
        }
    "#;
    let diagnostics = check_all_diagnostics(src);
    assert!(
        !diagnostics.iter().any(|e| matches!(
            e,
            SemanticError::AsmInSafeContext { .. } | SemanticError::AsmInDeviceContext { .. }
        )),
        "asm! should be allowed in @kernel context"
    );
}

#[test]
fn asm_allowed_in_unsafe_context() {
    let src = r#"
        @unsafe
        fn raw_stuff() {
            asm!("nop")
        }
    "#;
    let diagnostics = check_all_diagnostics(src);
    assert!(
        !diagnostics.iter().any(|e| matches!(
            e,
            SemanticError::AsmInSafeContext { .. } | SemanticError::AsmInDeviceContext { .. }
        )),
        "asm! should be allowed in @unsafe context"
    );
}

// ── S9.5: Async type checking ──

#[test]
fn await_rejected_outside_async() {
    // v0.7: .await now allowed in any context
    let src = r#"
        fn main() {
            let x = foo().await
        }
        fn foo() -> i64 { 42 }
    "#;
    let result = check(src);
    assert!(
        result.is_ok()
            || !result
                .unwrap_err()
                .iter()
                .any(|e| matches!(e, SemanticError::AwaitOutsideAsync { .. })),
        ".await should now be allowed outside async fn (v0.7)"
    );
}

#[test]
fn await_allowed_in_async_fn() {
    let src = r#"
        async fn compute() {
            let x = foo().await
        }
        fn foo() -> i64 { 42 }
    "#;
    let diagnostics = check_all_diagnostics(src);
    assert!(
        !diagnostics
            .iter()
            .any(|e| matches!(e, SemanticError::AwaitOutsideAsync { .. })),
        ".await should be allowed in async fn"
    );
}

#[test]
fn await_rejected_in_regular_fn_nested() {
    // v0.7: .await now allowed everywhere
    let src = r#"
        fn outer() {
            let val = something().await
        }
    "#;
    let errors = check_errors(src);
    assert!(
        !errors
            .iter()
            .any(|e| matches!(e, SemanticError::AwaitOutsideAsync { .. })),
        ".await now allowed in regular fn (v0.7)"
    );
}

// ── S6.5: Mutex/sync allowed in all contexts ──

#[test]
fn mutex_allowed_in_kernel_context() {
    // Mutex::new() call is a path call → analyzer allows it in @kernel
    let src = r#"
        @kernel
        fn smp_handler() {
            let m = Mutex::new(0)
        }
    "#;
    let diagnostics = check_all_diagnostics(src);
    // No KE001/KE002 errors for Mutex in kernel context
    assert!(
        !diagnostics.iter().any(|e| matches!(
            e,
            SemanticError::HeapAllocInKernel { .. } | SemanticError::TensorInKernel { .. }
        )),
        "Mutex should be allowed in @kernel context"
    );
}

#[test]
fn mutex_allowed_in_device_context() {
    let src = r#"
        @device
        fn gpu_sync() {
            let m = Mutex::new(0)
        }
    "#;
    let diagnostics = check_all_diagnostics(src);
    assert!(
        !diagnostics
            .iter()
            .any(|e| matches!(e, SemanticError::RawPointerInDevice { .. })),
        "Mutex should be allowed in @device context"
    );
}

// ── S9.2: Future and Poll types ──

#[test]
fn future_type_display() {
    let t = Type::Future {
        inner: Box::new(Type::I64),
    };
    assert_eq!(t.display_name(), "Future<i64>");
}

#[test]
fn future_type_compatible_with_self() {
    let a = Type::Future {
        inner: Box::new(Type::I64),
    };
    let b = Type::Future {
        inner: Box::new(Type::I64),
    };
    assert!(a.is_compatible(&b));
}

#[test]
fn future_type_incompatible_different_inner() {
    let a = Type::Future {
        inner: Box::new(Type::I64),
    };
    let b = Type::Future {
        inner: Box::new(Type::Str),
    };
    assert!(!a.is_compatible(&b));
}

#[test]
fn async_fn_has_future_return_type() {
    // async fn foo() -> i64 should have type fn() -> Future<i64>
    let src = r#"
        async fn compute() -> i64 {
            42
        }
    "#;
    let diagnostics = check_all_diagnostics(src);
    // Should compile without type errors (Future<i64> wraps return)
    assert!(
        !diagnostics
            .iter()
            .any(|e| matches!(e, SemanticError::TypeMismatch { .. })),
        "async fn should not produce type mismatch"
    );
}

// ── Function pointer types ──

#[test]
fn fn_pointer_assignment_valid() {
    let src = r#"
        fn add(a: i64, b: i64) -> i64 { a + b }
        fn main() {
            let f: fn(i64, i64) -> i64 = add
            let result = f(3, 4)
        }
    "#;
    assert!(check(src).is_ok());
}

#[test]
fn fn_pointer_as_parameter() {
    let src = r#"
        fn apply(f: fn(i64) -> i64, x: i64) -> i64 {
            f(x)
        }
        fn double(x: i64) -> i64 { x * 2 }
        fn main() {
            let result = apply(double, 5)
        }
    "#;
    assert!(check(src).is_ok());
}

#[test]
fn fn_pointer_type_mismatch() {
    let src = r#"
        fn add(a: i64, b: i64) -> i64 { a + b }
        fn main() {
            let f: fn(i64) -> i64 = add
        }
    "#;
    let errs = check_errors(src);
    assert!(
        errs.iter()
            .any(|e| matches!(e, SemanticError::TypeMismatch { .. })),
        "should report type mismatch for wrong fn pointer arity"
    );
}

#[test]
fn fn_pointer_call_type_check() {
    let src = r#"
        fn apply(f: fn(i64) -> i64, x: i64) -> i64 {
            f(x)
        }
        fn main() {
            let result: i64 = apply(|x| x + 1, 5)
        }
    "#;
    assert!(check(src).is_ok());
}

// ── S5.6: Send/Sync Thread Safety ──

#[test]
fn send_check_pass() {
    // i64 is Send — no SE018 error should be emitted
    let src = r#"
        fn worker(x: i64) -> i64 { x * 2 }
        fn main() {
            let h = thread::spawn(worker, 42)
        }
    "#;
    let diagnostics = check_all_diagnostics(src);
    assert!(
        !diagnostics
            .iter()
            .any(|e| matches!(e, SemanticError::NotSendType { .. })),
        "i64 is Send — should not produce SE018"
    );
}

#[test]
fn send_check_fn_only() {
    // Function-only spawn (no data arg) — no Send issue
    let src = r#"
        fn worker() -> i64 { 42 }
        fn main() {
            let h = thread::spawn(worker)
        }
    "#;
    let diagnostics = check_all_diagnostics(src);
    assert!(
        !diagnostics
            .iter()
            .any(|e| matches!(e, SemanticError::NotSendType { .. })),
        "No data arg — should not produce SE018"
    );
}

#[test]
fn sync_check_pass() {
    // f64 is Send+Sync — no SE018 error
    let src = r#"
        fn compute(x: f64) -> i64 { 1 }
        fn main() {
            let h = thread::spawn(compute, 3.14)
        }
    "#;
    let diagnostics = check_all_diagnostics(src);
    assert!(
        !diagnostics
            .iter()
            .any(|e| matches!(e, SemanticError::NotSendType { .. })),
        "f64 is Send — should not produce SE018"
    );
}

#[test]
fn is_send_returns_true_for_primitives() {
    assert!(Type::I64.is_send());
    assert!(Type::F64.is_send());
    assert!(Type::Bool.is_send());
    assert!(Type::Str.is_send());
    assert!(Type::Char.is_send());
    assert!(Type::U8.is_send());
    assert!(Type::I128.is_send());
}

#[test]
fn is_send_returns_true_for_composites() {
    assert!(Type::Array(Box::new(Type::I64)).is_send());
    assert!(Type::Tuple(vec![Type::I64, Type::F64]).is_send());
    assert!(
        Type::Enum {
            name: "Option".into()
        }
        .is_send()
    );
    assert!(
        Type::Function {
            params: vec![Type::I64],
            ret: Box::new(Type::I64),
        }
        .is_send()
    );
}

#[test]
fn is_sync_matches_send() {
    assert!(Type::I64.is_sync());
    assert!(Type::Str.is_sync());
    assert!(Type::Array(Box::new(Type::I64)).is_sync());
}

// ── S13.3: Borrow checker + concurrency ──

#[test]
fn reject_mut_ref_is_not_send() {
    // &mut T is NOT Send — mutable references cannot be shared across threads
    assert!(!Type::RefMut(Box::new(Type::I64), None).is_send());
    assert!(!Type::RefMut(Box::new(Type::Str), None).is_send());
    assert!(!Type::RefMut(Box::new(Type::F64), None).is_send());
}

#[test]
fn immutable_ref_is_send() {
    // &T is Send if T is Send
    assert!(Type::Ref(Box::new(Type::I64), None).is_send());
    assert!(Type::Ref(Box::new(Type::Str), None).is_send());
}

#[test]
fn allow_move_capture_in_spawn() {
    // Moving a value (i64) to thread::spawn is fine — i64 is Send
    let src = r#"
        fn worker(x: i64) -> i64 { x + 1 }
        fn main() {
            let val = 42
            let h = thread::spawn(worker, val)
        }
    "#;
    let diagnostics = check_all_diagnostics(src);
    assert!(
        !diagnostics
            .iter()
            .any(|e| matches!(e, SemanticError::NotSendType { .. })),
        "Move capture of i64 should be allowed"
    );
}

// ── Lifetime annotation tests ───────────────────────────────────────

#[test]
fn lifetime_valid_single_input_output() {
    // Single input lifetime, output uses same — valid via elision rule 2
    let src = "fn first<'a>(x: &'a i32) -> &'a i32 { x }";
    let diagnostics = check_all_diagnostics(src);
    assert!(
        !diagnostics
            .iter()
            .any(|e| matches!(e, SemanticError::LifetimeMismatch { .. })),
        "Valid single-lifetime function should pass: {:?}",
        diagnostics
    );
}

#[test]
fn lifetime_undeclared_in_param() {
    // 'b used in param but not declared — should report LifetimeMismatch
    let src = "fn foo(x: &'b i32) -> i32 { 0 }";
    let diagnostics = check_all_diagnostics(src);
    assert!(
        diagnostics
            .iter()
            .any(|e| matches!(e, SemanticError::LifetimeMismatch { .. })),
        "Undeclared lifetime 'b should produce LifetimeMismatch: {:?}",
        diagnostics
    );
}

#[test]
fn lifetime_undeclared_in_return() {
    // 'a used in return type but not declared
    let src = "fn foo(x: i32) -> &'a i32 { x }";
    let diagnostics = check_all_diagnostics(src);
    assert!(
        diagnostics
            .iter()
            .any(|e| matches!(e, SemanticError::LifetimeMismatch { .. })),
        "Undeclared lifetime in return should produce LifetimeMismatch: {:?}",
        diagnostics
    );
}

#[test]
fn lifetime_static_is_always_valid() {
    // 'static is a special built-in lifetime, never requires declaration
    let src = "fn foo(x: &'static i32) -> i32 { 0 }";
    let diagnostics = check_all_diagnostics(src);
    assert!(
        !diagnostics
            .iter()
            .any(|e| matches!(e, SemanticError::LifetimeMismatch { .. })),
        "'static should be valid without declaration: {:?}",
        diagnostics
    );
}

#[test]
fn lifetime_wildcard_is_always_valid() {
    // '_ is a wildcard lifetime, never requires declaration
    let src = "fn foo(x: &'_ i32) -> i32 { 0 }";
    let diagnostics = check_all_diagnostics(src);
    assert!(
        !diagnostics
            .iter()
            .any(|e| matches!(e, SemanticError::LifetimeMismatch { .. })),
        "'_ should be valid without declaration: {:?}",
        diagnostics
    );
}

#[test]
fn lifetime_duplicate_declaration_reports_conflict() {
    // Declaring 'a twice should produce LifetimeConflict
    let src = "fn foo<'a, 'a>(x: &'a i32) -> &'a i32 { x }";
    let diagnostics = check_all_diagnostics(src);
    assert!(
        diagnostics
            .iter()
            .any(|e| matches!(e, SemanticError::LifetimeConflict { .. })),
        "Duplicate lifetime 'a should produce LifetimeConflict: {:?}",
        diagnostics
    );
}

#[test]
fn lifetime_no_annotations_passes() {
    // Functions without any lifetime annotations should pass fine
    let src = "fn add(a: i32, b: i32) -> i32 { a + b }";
    let diagnostics = check_all_diagnostics(src);
    assert!(
        !diagnostics.iter().any(|e| matches!(
            e,
            SemanticError::LifetimeMismatch { .. }
                | SemanticError::LifetimeConflict { .. }
                | SemanticError::DanglingReference { .. }
        )),
        "No-lifetime function should have no lifetime errors: {:?}",
        diagnostics
    );
}

// ═══════════════════════════════════════════════════════════════════════
// Strict ownership mode tests (--strict-ownership)
// ═══════════════════════════════════════════════════════════════════════

/// Runs analysis in strict ownership mode, returns errors.
fn check_strict(source: &str) -> Result<(), Vec<SemanticError>> {
    let tokens = tokenize(source).expect("lex error");
    let program = parse(tokens).expect("parse error");
    let mut tc = TypeChecker::new_strict();
    tc.analyze(&program)
}

fn check_strict_errors(source: &str) -> Vec<SemanticError> {
    check_strict(source).unwrap_err()
}

#[test]
fn strict_move_string_use_after_move() {
    // ME001: use of moved String variable
    let src = r#"
        fn test() {
            let s: str = "hello"
            let t = s
            println(s)
        }
    "#;
    let errors = check_strict_errors(src);
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, SemanticError::UseAfterMove { name, .. } if name == "s")),
        "Expected ME001 UseAfterMove for 's', got: {:?}",
        errors
    );
}

#[test]
fn strict_copy_int_no_error() {
    // Primitives are Copy — no move error
    let src = r#"
        fn test() {
            let x: i32 = 42
            let y = x
            let z = x
        }
    "#;
    assert!(check_strict(src).is_ok());
}

#[test]
fn strict_move_while_borrowed() {
    // ME003: cannot move 's' because it is borrowed
    // r is used AFTER the move, so NLL keeps the borrow alive
    let src = r#"
        fn test() {
            let s: str = "hello"
            let r = &s
            let t = s
            println(r)
        }
    "#;
    let errors = check_strict_errors(src);
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, SemanticError::MoveWhileBorrowed { name, .. } if name == "s")),
        "Expected ME003 MoveWhileBorrowed for 's', got: {:?}",
        errors
    );
}

#[test]
fn strict_ref_is_copy() {
    // &T is Copy — can use reference after "move" (it's a copy)
    let src = r#"
        fn test() {
            let x: i32 = 42
            let r = &x
            let r2 = r
            let r3 = r
        }
    "#;
    assert!(check_strict(src).is_ok());
}

#[test]
fn strict_fn_arg_moves_string() {
    // Passing String to non-consuming function moves it
    let src = r#"
        fn consume(s: str) -> str { s }
        fn test() {
            let s: str = "hello"
            consume(s)
            consume(s)
        }
    "#;
    let errors = check_strict_errors(src);
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, SemanticError::UseAfterMove { name, .. } if name == "s")),
        "Expected ME001 after passing String to function, got: {:?}",
        errors
    );
}

#[test]
fn strict_fn_arg_copy_int() {
    // Passing i32 to function is a copy — no error
    let src = r#"
        fn double(x: i32) -> i32 { x * 2 }
        fn test() {
            let n: i32 = 5
            double(n)
            double(n)
        }
    "#;
    assert!(check_strict(src).is_ok());
}

#[test]
fn strict_nll_sequential_borrows() {
    // NLL: after borrow is no longer used, variable can be moved
    let src = r#"
        fn test() {
            let s: str = "hello"
            let r = &s
            println(r)
            let t = s
        }
    "#;
    // This should pass: r is dead after println(r), so s can be moved
    assert!(check_strict(src).is_ok());
}

#[test]
fn strict_default_mode_no_move_errors() {
    // FJARR_LEAK Phase 2 D-FULL (v35.5.0): default mode is now strict.
    // `let t = s` consumes s (str is affine); `println(s)` fires ME001.
    let src = r#"
        fn test() {
            let s: str = "hello"
            let t = s
            println(s)
        }
    "#;
    let errors = check_errors(src);
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, SemanticError::UseAfterMove { .. })),
        "expected ME001 UseAfterMove, got: {errors:?}"
    );
}

#[test]
fn strict_new_strict_has_flag() {
    let tc = TypeChecker::new_strict();
    assert!(tc.is_strict_ownership());
}

#[test]
fn strict_new_default_no_flag() {
    let tc = TypeChecker::new();
    assert!(!tc.is_strict_ownership());
}

// ═══════════════════════════════════════════════════════════════════════
// Phase B: Lifetime validation tests
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn lifetime_env_has_static() {
    let tc = TypeChecker::new();
    assert_eq!(tc.resolve_lifetime("static"), Some(0));
}

#[test]
fn lifetime_env_wildcard_returns_none() {
    let tc = TypeChecker::new();
    assert_eq!(tc.resolve_lifetime("_"), None);
}

#[test]
fn lifetime_env_registers_fn_params() {
    let mut tc = TypeChecker::new();
    let params = vec![
        crate::parser::ast::LifetimeParam {
            name: "a".into(),
            span: crate::lexer::token::Span::new(0, 1),
        },
        crate::parser::ast::LifetimeParam {
            name: "b".into(),
            span: crate::lexer::token::Span::new(2, 3),
        },
    ];
    let saved = tc.push_lifetime_env(&params);
    assert!(tc.resolve_lifetime("a").is_some());
    assert!(tc.resolve_lifetime("b").is_some());
    assert_ne!(tc.resolve_lifetime("a"), tc.resolve_lifetime("b"));
    // static is still there
    assert_eq!(tc.resolve_lifetime("static"), Some(0));
    // pop restores
    tc.pop_lifetime_env(saved);
    assert!(tc.resolve_lifetime("a").is_none());
    assert!(tc.resolve_lifetime("b").is_none());
}

#[test]
fn lifetime_ref_type_carries_id() {
    // &'a i32 → Ref(I32, Some(id)) when 'a is declared
    let src = "fn foo<'a>(x: &'a i32) -> &'a i32 { x }";
    assert!(check(src).is_ok());
}

#[test]
fn lifetime_dangling_ref_to_local() {
    // ME010: returning reference to local variable
    // Use a single-expression function that creates and references a local
    let src = "fn bad(y: i32) -> &i32 { &y }";
    let diagnostics = check_all_diagnostics(src);
    // y is a param, so this should NOT be dangling. Test the positive case.
    assert!(
        !diagnostics
            .iter()
            .any(|e| matches!(e, SemanticError::DanglingReference { .. })),
        "Ref to param should not be dangling: {:?}",
        diagnostics
    );
    // Now test returning ref to a non-param local via a function
    // that creates a binding in a block
    let src2 = r#"fn bad2() -> &i32 { let z: i32 = 1; &z }"#;
    let diag2 = check_all_diagnostics(src2);
    assert!(
        diag2
            .iter()
            .any(|e| matches!(e, SemanticError::DanglingReference { .. })),
        "Expected ME010 DanglingReference for &local, got: {:?}",
        diag2
    );
}

#[test]
fn lifetime_return_ref_to_param_ok() {
    // Returning &param is not dangling
    let src = "fn identity(x: &i32) -> &i32 { x }";
    let diagnostics = check_all_diagnostics(src);
    assert!(
        !diagnostics
            .iter()
            .any(|e| matches!(e, SemanticError::DanglingReference { .. })),
        "Returning &param should not be dangling: {:?}",
        diagnostics
    );
}

#[test]
fn lifetime_struct_ref_field_warns() {
    // Struct with &T field but no lifetime params
    let src = r#"
        struct Holder {
            data: &i32,
        }
    "#;
    let diagnostics = check_all_diagnostics(src);
    assert!(
        diagnostics
            .iter()
            .any(|e| matches!(e, SemanticError::DanglingReference { .. })),
        "Struct with &T field and no lifetime should warn: {:?}",
        diagnostics
    );
}

#[test]
fn lifetime_elision_single_input_ok() {
    // Single input lifetime elision — unambiguous
    let src = "fn first(x: &i32) -> &i32 { x }";
    let diagnostics = check_all_diagnostics(src);
    assert!(
        !diagnostics
            .iter()
            .any(|e| matches!(e, SemanticError::LifetimeMismatch { .. })),
        "Single-input elision should pass: {:?}",
        diagnostics
    );
}

#[test]
fn lifetime_elision_ambiguous_error() {
    // Multiple input lifetimes, no &self — elision is ambiguous for output &
    let src = "fn pick(x: &i32, y: &i32) -> &i32 { x }";
    let diagnostics = check_all_diagnostics(src);
    assert!(
        diagnostics
            .iter()
            .any(|e| matches!(e, SemanticError::LifetimeMismatch { .. })),
        "Ambiguous elision should produce LifetimeMismatch: {:?}",
        diagnostics
    );
}

#[test]
fn lifetime_explicit_resolves_ambiguity() {
    // Explicit lifetime annotations resolve the ambiguity
    let src = "fn pick<'a>(x: &'a i32, y: &i32) -> &'a i32 { x }";
    let diagnostics = check_all_diagnostics(src);
    assert!(
        !diagnostics
            .iter()
            .any(|e| matches!(e, SemanticError::LifetimeMismatch { .. })),
        "Explicit lifetimes should resolve ambiguity: {:?}",
        diagnostics
    );
}

// ═══════════════════════════════════════════════════════════════════════
// Phase C: Advanced borrow tests (strict mode)
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn strict_fn_ref_param_borrows_not_moves() {
    // Passing a move-type to a fn that takes by value moves it once;
    // passing to a fn that takes &T would require &s syntax.
    // Test: calling with value on a by-value fn moves, second call fails
    let src = r#"
        fn consume(name: str) -> i32 { 0 }
        fn test() {
            let s: str = "world"
            consume(s)
            consume(s)
        }
    "#;
    let errors = check_strict_errors(src);
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, SemanticError::UseAfterMove { name, .. } if name == "s")),
        "Expected ME001 on second consume(s), got: {:?}",
        errors
    );
}

#[test]
fn strict_drop_order_ref_after_target_ok() {
    // Ref declared after target — correct drop order
    let src = r#"
        fn test() {
            let s: str = "hello"
            let r = &s
            println(r)
        }
    "#;
    assert!(check_strict(src).is_ok());
}

// ── Phase D: Edge-case unit tests (D4) ──────────────────────────────

#[test]
fn strict_move_in_if_branch_both_sides() {
    // Moving in both branches — use after if should error
    let errors = check_strict_errors(
        r#"
        fn test() {
            let s: str = "hello"
            if true {
                let a = s
            } else {
                let b = s
            }
            println(s)
        }
        "#,
    );
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, SemanticError::UseAfterMove { name, .. } if name == "s")),
        "Expected ME001 after conditional move, got: {:?}",
        errors
    );
}

#[test]
fn strict_move_then_use_in_match() {
    // Move a non-Copy value then use it in a match — ME001
    let errors = check_strict_errors(
        r#"
        fn test() {
            let s: str = "hello"
            let t = s
            match s {
                _ => println("done")
            }
        }
        "#,
    );
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, SemanticError::UseAfterMove { name, .. } if name == "s")),
        "Expected ME001 for use-after-move in match, got: {:?}",
        errors
    );
}

#[test]
fn strict_borrow_across_reassignment() {
    // Immutable borrow then reassign the variable — borrow should conflict
    let errors = check_strict_errors(
        r#"
        fn test() {
            let mut x: i64 = 1
            let r = &x
            x = 2
            println(r)
        }
        "#,
    );
    // Reassigning x while r holds a borrow should produce an error
    assert!(
        !errors.is_empty(),
        "Expected borrow-related error on reassign while borrowed, got none"
    );
}

#[test]
fn strict_copy_type_in_loop_ok() {
    // Copy types in loops should NOT produce move errors
    assert!(
        check_strict(
            r#"
        fn test() {
            let x: i64 = 42
            let mut sum: i64 = 0
            let mut i: i64 = 0
            while i < 3 {
                sum = sum + x
                i = i + 1
            }
        }
        "#
        )
        .is_ok()
    );
}

#[test]
fn strict_multiple_moves_same_var() {
    // Moving same variable twice should produce ME001 on second move
    let errors = check_strict_errors(
        r#"
        fn test() {
            let s: str = "hello"
            let a = s
            let b = s
        }
        "#,
    );
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, SemanticError::UseAfterMove { name, .. } if name == "s")),
        "Expected ME001 on second move, got: {:?}",
        errors
    );
}

#[test]
fn strict_nested_function_isolation() {
    // Inner function's local moves shouldn't affect outer scope
    assert!(
        check_strict(
            r#"
        fn outer() {
            let x: i64 = 10
            fn inner() {
                let s: str = "hi"
                let t = s
            }
            println(x)
        }
        "#
        )
        .is_ok()
    );
}

#[test]
fn strict_hint_on_me001() {
    // Verify hint() returns non-None for UseAfterMove
    let errors = check_strict_errors(
        r#"
        fn test() {
            let s: str = "hello"
            let t = s
            println(s)
        }
        "#,
    );
    let me001 = errors
        .iter()
        .find(|e| matches!(e, SemanticError::UseAfterMove { .. }));
    assert!(me001.is_some(), "Expected ME001");
    let hint = me001.unwrap().hint();
    assert!(hint.is_some(), "ME001 should have a hint");
    assert!(
        hint.unwrap().contains("clone"),
        "Hint should suggest cloning"
    );
}

#[test]
fn strict_hint_on_me004() {
    // Verify hint() returns non-None for MutBorrowConflict
    // r1 is used AFTER r2, so NLL keeps the borrow live → ME004
    let errors = check_strict_errors(
        r#"
        fn test() {
            let mut x: i64 = 42
            let r1 = &x
            let r2 = &mut x
            println(r1)
        }
        "#,
    );
    let me004 = errors
        .iter()
        .find(|e| matches!(e, SemanticError::MutBorrowConflict { .. }));
    assert!(me004.is_some(), "Expected ME004");
    let hint = me004.unwrap().hint();
    assert!(hint.is_some(), "ME004 should have a hint");
    assert!(
        hint.unwrap().contains("borrow"),
        "Hint should mention borrow scope"
    );
}

#[test]
fn strict_secondary_span_on_me003() {
    // Verify secondary_span() returns borrow location for MoveWhileBorrowed
    // r is used AFTER the move, so NLL keeps the borrow live → ME003
    let errors = check_strict_errors(
        r#"
        fn test() {
            let s: str = "hello"
            let r = &s
            let t = s
            println(r)
        }
        "#,
    );
    let me003 = errors
        .iter()
        .find(|e| matches!(e, SemanticError::MoveWhileBorrowed { .. }));
    assert!(me003.is_some(), "Expected ME003");
    let secondary = me003.unwrap().secondary_span();
    assert!(secondary.is_some(), "ME003 should have secondary span");
    let (_, label) = secondary.unwrap();
    assert_eq!(label, "borrow created here");
}

#[test]
fn strict_error_message_contains_byte_offset() {
    // ME001 error message should include "moved at byte" info
    let errors = check_strict_errors(
        r#"
        fn test() {
            let s: str = "hello"
            let t = s
            println(s)
        }
        "#,
    );
    let me001 = errors
        .iter()
        .find(|e| matches!(e, SemanticError::UseAfterMove { .. }));
    assert!(me001.is_some(), "Expected ME001");
    let msg = format!("{}", me001.unwrap());
    assert!(
        msg.contains("moved at byte"),
        "Error message should contain 'moved at byte', got: {msg}"
    );
}
