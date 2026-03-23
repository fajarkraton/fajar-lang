//! Documentation generation integration tests for Fajar Lang.
//!
//! Tests doc generation for new features (effects, comptime, macros),
//! migration guide existence, and book chapter completeness.

use std::path::Path;

// ════════════════════════════════════════════════════════════════════════
// 1. Doc generation for new syntax
// ════════════════════════════════════════════════════════════════════════

#[test]
fn docgen_effect_syntax() {
    let source = r#"
/// An effect declaration for logging.
effect Logger {
    fn log(msg: str) -> void
}

/// A function with effect annotations.
fn greet() with IO {
    println("hello")
}
"#;
    let tokens = fajar_lang::lexer::tokenize(source).unwrap();
    let program = fajar_lang::parser::parse(tokens).unwrap();
    let html = fajar_lang::docgen::generate_docs("effects_demo", &program);
    assert!(html.contains("greet"));
}

#[test]
fn docgen_comptime_syntax() {
    let source = r#"
/// Comptime factorial function.
comptime fn factorial(n: i64) -> i64 {
    if n <= 1 { 1 } else { n * factorial(n - 1) }
}
"#;
    let tokens = fajar_lang::lexer::tokenize(source).unwrap();
    let program = fajar_lang::parser::parse(tokens).unwrap();
    let html = fajar_lang::docgen::generate_docs("comptime_demo", &program);
    assert!(html.contains("factorial"));
}

#[test]
fn docgen_derive_struct() {
    let source = r#"
/// A point in 2D space.
@derive(Debug, Clone, PartialEq)
struct Point {
    x: f64,
    y: f64
}
"#;
    let tokens = fajar_lang::lexer::tokenize(source).unwrap();
    let program = fajar_lang::parser::parse(tokens).unwrap();
    let html = fajar_lang::docgen::generate_docs("derive_demo", &program);
    assert!(html.contains("Point"));
}

#[test]
fn docgen_empty_program() {
    let tokens = fajar_lang::lexer::tokenize("").unwrap();
    let program = fajar_lang::parser::parse(tokens).unwrap();
    let html = fajar_lang::docgen::generate_docs("empty", &program);
    // Empty program may produce empty or minimal output — just verify no crash
    let _ = html;
}

#[test]
fn docgen_with_doc_comments() {
    let source = r#"
/// Adds two numbers together.
///
/// # Examples
/// ```
/// let result = add(2, 3)
/// assert_eq(result, 5)
/// ```
fn add(a: i64, b: i64) -> i64 { a + b }
"#;
    let tokens = fajar_lang::lexer::tokenize(source).unwrap();
    let program = fajar_lang::parser::parse(tokens).unwrap();
    let html = fajar_lang::docgen::generate_docs("add_module", &program);
    assert!(html.contains("add"));
    assert!(html.contains("Adds two numbers"));
}

// ════════════════════════════════════════════════════════════════════════
// 2. Migration guides exist
// ════════════════════════════════════════════════════════════════════════

#[test]
fn migration_guide_rust_exists() {
    assert!(Path::new("book/src/migration/from-rust.md").exists());
}

#[test]
fn migration_guide_cpp_exists() {
    assert!(Path::new("book/src/migration/from-cpp.md").exists());
}

#[test]
fn migration_guide_python_exists() {
    assert!(Path::new("book/src/migration/from-python.md").exists());
}

#[test]
fn migration_guide_rust_content() {
    let content = std::fs::read_to_string("book/src/migration/from-rust.md").unwrap();
    assert!(content.contains("Effect System"));
    assert!(content.contains("Linear Types"));
    assert!(content.contains("Comptime"));
    assert!(content.contains("Context Annotations"));
}

#[test]
fn migration_guide_cpp_content() {
    let content = std::fs::read_to_string("book/src/migration/from-cpp.md").unwrap();
    assert!(content.contains("Memory Safety"));
    assert!(content.contains("No Undefined Behavior"));
}

#[test]
fn migration_guide_python_content() {
    let content = std::fs::read_to_string("book/src/migration/from-python.md").unwrap();
    assert!(content.contains("Tensor"));
    assert!(content.contains("Performance"));
}

// ════════════════════════════════════════════════════════════════════════
// 3. Book chapters exist
// ════════════════════════════════════════════════════════════════════════

#[test]
fn book_effects_chapter() {
    let content = std::fs::read_to_string("book/src/advanced/effects.md").unwrap();
    assert!(content.contains("with"));
    assert!(content.contains("handle"));
    assert!(content.contains("@kernel"));
    assert!(content.contains("EE006"));
}

#[test]
fn book_comptime_chapter() {
    let content = std::fs::read_to_string("book/src/advanced/comptime.md").unwrap();
    assert!(content.contains("comptime"));
    assert!(content.contains("factorial"));
    assert!(content.contains("CT007"));
}

#[test]
fn book_macros_chapter() {
    let content = std::fs::read_to_string("book/src/advanced/macros.md").unwrap();
    assert!(content.contains("vec!"));
    assert!(content.contains("stringify!"));
    assert!(content.contains("@derive"));
}

#[test]
fn book_summary_has_migration() {
    let content = std::fs::read_to_string("book/src/SUMMARY.md").unwrap();
    assert!(content.contains("Migration"));
    assert!(content.contains("from-rust"));
    assert!(content.contains("from-cpp"));
    assert!(content.contains("from-python"));
}

// ════════════════════════════════════════════════════════════════════════
// 4. Core documentation exists
// ════════════════════════════════════════════════════════════════════════

#[test]
fn docs_error_codes_exists() {
    assert!(Path::new("docs/ERROR_CODES.md").exists());
}

#[test]
fn docs_fajar_lang_spec_exists() {
    assert!(Path::new("docs/FAJAR_LANG_SPEC.md").exists());
}

#[test]
fn docs_stdlib_spec_exists() {
    assert!(Path::new("docs/STDLIB_SPEC.md").exists());
}

#[test]
fn docs_architecture_exists() {
    assert!(Path::new("docs/ARCHITECTURE.md").exists());
}
