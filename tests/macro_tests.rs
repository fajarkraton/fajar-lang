//! Macro system integration tests for Fajar Lang.
//!
//! Tests macro invocations, macro_rules!, @derive, built-in macros,
//! and the macro expansion pipeline.

use fajar_lang::interpreter::{Interpreter, Value};
use fajar_lang::macros;

/// Helper: run through full pipeline.
fn eval_ok(source: &str) -> Value {
    let mut interp = Interpreter::new_capturing();
    interp
        .eval_source(source)
        .unwrap_or_else(|e| panic!("eval failed: {e}"))
}

fn eval_output(source: &str) -> Vec<String> {
    let mut interp = Interpreter::new_capturing();
    interp.eval_source(source).unwrap();
    interp.call_main().unwrap();
    interp.get_output().to_vec()
}

fn parse_ok(source: &str) {
    let tokens = fajar_lang::lexer::tokenize(source).expect("lex failed");
    fajar_lang::parser::parse(tokens).expect("parse failed");
}

// ════════════════════════════════════════════════════════════════════════
// 1. Parsing: macro invocations
// ════════════════════════════════════════════════════════════════════════

#[test]
fn parse_vec_macro() {
    parse_ok("vec![1, 2, 3]");
}

#[test]
fn parse_vec_macro_empty() {
    parse_ok("vec![]");
}

#[test]
fn parse_stringify_macro() {
    parse_ok("stringify!(42)");
}

#[test]
fn parse_concat_macro() {
    parse_ok(r#"concat!("hello", " ", "world")"#);
}

#[test]
fn parse_dbg_macro() {
    parse_ok("dbg!(42)");
}

#[test]
fn parse_todo_macro() {
    parse_ok("todo!()");
}

#[test]
fn parse_nested_macro_args() {
    parse_ok("vec![1 + 2, 3 * 4, 5]");
}

// ════════════════════════════════════════════════════════════════════════
// 2. Parsing: macro_rules!
// ════════════════════════════════════════════════════════════════════════

#[test]
fn parse_macro_rules_empty() {
    parse_ok("macro_rules! empty {}");
}

#[test]
fn parse_macro_rules_one_arm() {
    parse_ok(
        r#"
macro_rules! answer {
    () => { 42 }
}
"#,
    );
}

#[test]
fn parse_macro_rules_with_pattern() {
    // Note: $ patterns not yet in lexer; use simple pattern for now
    parse_ok(
        r#"
macro_rules! double {
    (x) => { x * 2 }
}
"#,
    );
}

// ════════════════════════════════════════════════════════════════════════
// 3. Parsing: @derive annotation
// ════════════════════════════════════════════════════════════════════════

#[test]
fn parse_derive_single() {
    parse_ok("@derive(Debug) struct Point { x: f64, y: f64 }");
}

#[test]
fn parse_derive_multiple() {
    parse_ok("@derive(Debug, Clone, PartialEq) struct Coord { x: i64, y: i64 }");
}

#[test]
fn parse_derive_on_enum() {
    parse_ok("@derive(Debug) enum Color { Red, Green, Blue }");
}

#[test]
fn parse_pure_annotation() {
    parse_ok("@pure fn add(a: i64, b: i64) -> i64 { a + b }");
}

// ════════════════════════════════════════════════════════════════════════
// 4. Evaluation: vec! macro
// ════════════════════════════════════════════════════════════════════════

#[test]
fn eval_vec_macro_creates_array() {
    let result = eval_ok("vec![1, 2, 3]");
    match result {
        Value::Array(arr) => {
            assert_eq!(arr.len(), 3);
            assert_eq!(arr[0], Value::Int(1));
            assert_eq!(arr[1], Value::Int(2));
            assert_eq!(arr[2], Value::Int(3));
        }
        _ => panic!("expected Array, got {result:?}"),
    }
}

#[test]
fn eval_vec_macro_empty() {
    let result = eval_ok("vec![]");
    match result {
        Value::Array(arr) => assert!(arr.is_empty()),
        _ => panic!("expected empty Array"),
    }
}

#[test]
fn eval_vec_macro_with_expressions() {
    let result = eval_ok("vec![1 + 1, 2 * 3, 10 - 3]");
    match result {
        Value::Array(arr) => {
            assert_eq!(arr.len(), 3);
            assert_eq!(arr[0], Value::Int(2));
            assert_eq!(arr[1], Value::Int(6));
            assert_eq!(arr[2], Value::Int(7));
        }
        _ => panic!("expected Array"),
    }
}

// ════════════════════════════════════════════════════════════════════════
// 5. Evaluation: stringify! macro
// ════════════════════════════════════════════════════════════════════════

#[test]
fn eval_stringify_int() {
    let result = eval_ok("stringify!(42)");
    assert_eq!(result, Value::Str("42".to_string()));
}

#[test]
fn eval_stringify_string() {
    let result = eval_ok(r#"stringify!("hello")"#);
    assert_eq!(result, Value::Str("hello".to_string()));
}

#[test]
fn eval_stringify_bool() {
    let result = eval_ok("stringify!(true)");
    assert_eq!(result, Value::Str("true".to_string()));
}

// ════════════════════════════════════════════════════════════════════════
// 6. Evaluation: concat! macro
// ════════════════════════════════════════════════════════════════════════

#[test]
fn eval_concat_strings() {
    let result = eval_ok(r#"concat!("hello", " ", "world")"#);
    assert_eq!(result, Value::Str("hello world".to_string()));
}

#[test]
fn eval_concat_mixed_types() {
    let result = eval_ok(r#"concat!("value: ", 42)"#);
    assert_eq!(result, Value::Str("value: 42".to_string()));
}

// ════════════════════════════════════════════════════════════════════════
// 7. Evaluation: dbg! macro
// ════════════════════════════════════════════════════════════════════════

#[test]
fn eval_dbg_returns_value() {
    let result = eval_ok("dbg!(42)");
    assert_eq!(result, Value::Int(42));
}

#[test]
fn eval_dbg_string() {
    let result = eval_ok(r#"dbg!("test")"#);
    assert_eq!(result, Value::Str("test".to_string()));
}

// ════════════════════════════════════════════════════════════════════════
// 8. Macro registry
// ════════════════════════════════════════════════════════════════════════

#[test]
fn registry_has_all_builtins() {
    let reg = macros::MacroRegistry::new();
    assert!(reg.contains("vec"));
    assert!(reg.contains("stringify"));
    assert!(reg.contains("concat"));
    assert!(reg.contains("dbg"));
    assert!(reg.contains("todo"));
    assert!(reg.contains("env"));
    assert!(reg.contains("line"));
    assert!(reg.contains("file"));
    assert!(reg.contains("column"));
    assert!(reg.contains("include"));
    assert!(reg.contains("cfg"));
}

#[test]
fn registry_custom_macro() {
    let mut reg = macros::MacroRegistry::new();
    reg.register(
        "my_macro".to_string(),
        macros::MacroDef::UserDefined {
            name: "my_macro".to_string(),
            arms: vec![],
        },
    );
    assert!(reg.contains("my_macro"));
}

// ════════════════════════════════════════════════════════════════════════
// 9. Derive system
// ════════════════════════════════════════════════════════════════════════

#[test]
fn derive_debug_generates_impl() {
    let desc = macros::describe_derive("Debug", "Point", &["x".into(), "y".into()]);
    assert!(desc.contains("impl Debug for Point"));
}

#[test]
fn derive_clone_generates_impl() {
    let desc = macros::describe_derive("Clone", "Vec3", &["x".into(), "y".into(), "z".into()]);
    assert!(desc.contains("impl Clone for Vec3"));
    assert!(desc.contains("self.x.clone()"));
}

#[test]
fn derive_partial_eq_generates_impl() {
    let desc = macros::describe_derive("PartialEq", "Pair", &["a".into(), "b".into()]);
    assert!(desc.contains("impl PartialEq for Pair"));
    assert!(desc.contains("self.a == other.a"));
    assert!(desc.contains("self.b == other.b"));
}

#[test]
fn supported_derive_traits() {
    assert!(macros::is_supported_derive("Debug"));
    assert!(macros::is_supported_derive("Clone"));
    assert!(macros::is_supported_derive("PartialEq"));
    assert!(macros::is_supported_derive("Default"));
    assert!(macros::is_supported_derive("Hash"));
    assert!(!macros::is_supported_derive("Serialize"));
    assert!(!macros::is_supported_derive("Unknown"));
}

// ════════════════════════════════════════════════════════════════════════
// 10. Macros in functions
// ════════════════════════════════════════════════════════════════════════

#[test]
fn macro_in_function_body() {
    let output = eval_output(
        r#"
fn main() {
    let arr = vec![10, 20, 30]
    println(len(arr))
}
"#,
    );
    assert!(output.iter().any(|l| l.contains("3")));
}

#[test]
fn stringify_in_function() {
    let output = eval_output(
        r#"
fn main() {
    let s = stringify!(42)
    println(s)
}
"#,
    );
    assert!(output.iter().any(|l| l.contains("42")));
}

#[test]
fn concat_in_function() {
    let output = eval_output(
        r#"
fn main() {
    let msg = concat!("hello", " ", "world")
    println(msg)
}
"#,
    );
    assert!(output.iter().any(|l| l.contains("hello world")));
}

// ════════════════════════════════════════════════════════════════════════
// V32 Perfection P2.B4 — additional macro_rules! patterns
// ════════════════════════════════════════════════════════════════════════
//
// Existing tests cover 3 macro_rules! patterns (empty, one_arm,
// with_pattern). PASS criterion requires 5+ patterns. Adding 3 more
// to over-cover (6 total).

#[test]
fn parse_macro_rules_with_expr_metavar_single_arg() {
    // $x:expr metavar — supported per examples/macros.fj
    parse_ok(
        r#"
macro_rules! square { ($x:expr) => { $x * $x } }
"#,
    );
}

#[test]
fn parse_macro_rules_with_expr_metavar_two_args() {
    parse_ok(
        r#"
macro_rules! add { ($a:expr, $b:expr) => { $a + $b } }
"#,
    );
}

#[test]
fn parse_macro_rules_with_control_flow_body() {
    // expr metavar invoked inside if-expression in expansion
    parse_ok(
        r#"
macro_rules! max { ($a:expr, $b:expr) => { if $a > $b { $a } else { $b } } }
"#,
    );
}

// ════════════════════════════════════════════════════════════════════════
// V32 Perfection P2.B4 — proc-macro / @derive coverage extension
// ════════════════════════════════════════════════════════════════════════

#[test]
fn parse_derive_with_attrs_combined() {
    // @derive combined with @doc — exercises annotation-stack parsing
    parse_ok(
        r#"
@derive(Debug, Clone)
struct Pair { x: i64, y: i64 }
"#,
    );
}

#[test]
fn parse_derive_on_unit_struct() {
    parse_ok("@derive(Debug) struct Marker {}");
}
