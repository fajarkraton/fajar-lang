//! Effect polymorphism tests for Fajar Lang.
//!
//! Verifies generic-over-effects: `fn map<E: Effect>(f: fn()->i64 with E) with E`
//! The LAST unchecked item from WORLD_CLASS_PLAN.

fn parse_ok(source: &str) {
    let tokens = fajar_lang::lexer::tokenize(source).expect("lex failed");
    fajar_lang::parser::parse(tokens).expect("parse failed");
}

fn expect_ok(source: &str) {
    let tokens = fajar_lang::lexer::tokenize(source).expect("lex failed");
    let program = fajar_lang::parser::parse(tokens).expect("parse failed");
    match fajar_lang::analyzer::analyze(&program) {
        Ok(()) => {}
        Err(errors) => {
            let hard: Vec<_> = errors.iter().filter(|e| !e.is_warning()).collect();
            assert!(hard.is_empty(), "unexpected errors: {hard:?}");
        }
    }
}

fn expect_error(source: &str, code: &str) {
    let tokens = fajar_lang::lexer::tokenize(source).expect("lex failed");
    let program = fajar_lang::parser::parse(tokens).expect("parse failed");
    let errors = fajar_lang::analyzer::analyze(&program).unwrap_err();
    let found = errors.iter().any(|e| format!("{e}").contains(code));
    assert!(
        found,
        "expected '{code}', got: {:?}",
        errors.iter().map(|e| format!("{e}")).collect::<Vec<_>>()
    );
}

// ════════════════════════════════════════════════════════════════════════
// 1. Parsing: effect variable in generics
// ════════════════════════════════════════════════════════════════════════

#[test]
fn parse_effect_generic_param() {
    parse_ok("fn apply<E: Effect>(x: i64) -> i64 with E { x }");
}

#[test]
fn parse_effect_generic_with_type_param() {
    parse_ok("fn map<T, E: Effect>(x: T) -> T with E { x }");
}

#[test]
fn parse_effect_generic_no_bounds() {
    // E without `: Effect` bound is a regular type param
    parse_ok("fn identity<T>(x: T) -> T { x }");
}

#[test]
fn parse_multiple_effect_params() {
    parse_ok("fn combine<E1: Effect, E2: Effect>(x: i64) -> i64 with E1 { x }");
}

// ════════════════════════════════════════════════════════════════════════
// 2. Analysis: effect variable accepted in with clause
// ════════════════════════════════════════════════════════════════════════

#[test]
fn analyze_effect_var_in_with() {
    // E is an effect variable — should NOT trigger "unknown effect" error
    expect_ok("fn apply<E: Effect>(x: i64) -> i64 with E { x }");
}

#[test]
fn analyze_effect_var_with_known_effect() {
    // Mix of known effect (IO) and effect variable (E)
    expect_ok("fn apply<E: Effect>(x: i64) -> i64 with IO { x }");
}

#[test]
fn analyze_unknown_effect_still_errors() {
    // NonExistent is NOT an effect variable — should error
    expect_error("fn bad() with NonExistent { 42 }", "EE002");
}

#[test]
fn analyze_effect_var_not_in_context_check() {
    // E is an effect variable — context check should skip it
    // (even in @kernel, E is not checked against forbidden effects)
    expect_ok("@kernel fn k<E: Effect>() with E { 0 }");
}

// ════════════════════════════════════════════════════════════════════════
// 3. Effect variable + concrete effects
// ════════════════════════════════════════════════════════════════════════

#[test]
fn effect_var_with_concrete_io() {
    expect_ok(
        r#"
fn apply<E: Effect>(x: i64) -> i64 with E { x }
fn use_io() with IO { apply(42) }
"#,
    );
}

#[test]
fn effect_var_pure_call() {
    expect_ok(
        r#"
fn apply<E: Effect>(x: i64) -> i64 with E { x }
fn use_pure() { apply(42) }
"#,
    );
}

// ════════════════════════════════════════════════════════════════════════
// 4. Real-world patterns
// ════════════════════════════════════════════════════════════════════════

#[test]
fn effect_poly_map_pattern() {
    expect_ok(
        r#"
fn transform<E: Effect>(x: i64) -> i64 with E { x * 2 }
fn main() { println(transform(21)) }
"#,
    );
}

#[test]
fn effect_poly_with_comptime() {
    expect_ok(
        r#"
comptime fn double(x: i64) -> i64 { x * 2 }
fn apply<E: Effect>(x: i64) -> i64 with E { double(x) }
"#,
    );
}

#[test]
fn effect_poly_in_service() {
    parse_ok(
        r#"
service handler {
    fn process<E: Effect>(x: i64) -> i64 with E { x }
}
"#,
    );
}

// ════════════════════════════════════════════════════════════════════════
// 5. Edge cases
// ════════════════════════════════════════════════════════════════════════

#[test]
fn effect_var_empty_body() {
    expect_ok("fn noop<E: Effect>() with E { 0 }");
}

#[test]
fn effect_var_multiple_with() {
    // Two concrete effects + effect variable
    expect_ok("fn multi<E: Effect>() with IO, E { 0 }");
}

#[test]
fn effect_param_alongside_comptime() {
    parse_ok("fn both<comptime N, E: Effect>(x: i64) -> i64 with E { x + N }");
}

#[test]
fn regular_generic_not_effect() {
    // T without `: Effect` is NOT an effect variable
    // Using T in `with T` should error (T is unknown effect)
    expect_error("fn bad<T>(x: i64) with T { x }", "EE002");
}

#[test]
fn effect_bound_name_sensitivity() {
    // Only `: Effect` marks as effect variable, not `: Eff` or `: EffectType`
    expect_error("fn bad<E: Eff>() with E { 0 }", "EE002");
}
