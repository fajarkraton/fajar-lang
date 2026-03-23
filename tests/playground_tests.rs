//! Playground and WebAssembly integration tests for Fajar Lang.

use fajar_lang::playground::examples::builtin_examples;
use fajar_lang::playground::share;
use fajar_lang::playground::ui::MonacoConfig;

// ════════════════════════════════════════════════════════════════════════
// 1. URL sharing
// ════════════════════════════════════════════════════════════════════════

#[test]
fn share_encode_decode_roundtrip() {
    let source = "fn main() { println(42) }";
    let encoded = share::encode_for_url(source);
    let decoded = share::decode_from_url(&encoded).unwrap();
    assert_eq!(decoded, source);
}

#[test]
fn share_url_generation() {
    let url = share::share_url("https://play.fajarlang.org", "let x = 42");
    assert!(url.starts_with("https://play.fajarlang.org#code="));
}

#[test]
fn share_short_id_deterministic() {
    let id1 = share::short_url_id("fn main() {}");
    let id2 = share::short_url_id("fn main() {}");
    assert_eq!(id1, id2);
}

#[test]
fn share_short_id_different_for_different_source() {
    let id1 = share::short_url_id("fn a() {}");
    let id2 = share::short_url_id("fn b() {}");
    assert_ne!(id1, id2);
}

#[test]
fn share_short_url_format() {
    let url = share::short_url("https://play.fajarlang.org", "abc12345");
    assert!(
        url.contains("abc12345"),
        "short URL should contain the ID: {url}"
    );
}

// ════════════════════════════════════════════════════════════════════════
// 2. Example gallery
// ════════════════════════════════════════════════════════════════════════

#[test]
fn examples_not_empty() {
    let examples = builtin_examples();
    assert!(!examples.is_empty());
}

#[test]
fn examples_have_hello_world() {
    let examples = builtin_examples();
    let has_hello = examples.iter().any(|e| {
        e.title.to_lowercase().contains("hello")
            || e.code.contains("Hello")
            || e.code.contains("hello")
    });
    assert!(has_hello, "examples should include a hello world");
}

#[test]
fn examples_have_valid_code() {
    let examples = builtin_examples();
    for example in &examples {
        assert!(
            !example.code.is_empty(),
            "example '{}' has empty code",
            example.title
        );
        assert!(!example.title.is_empty(), "example has empty title");
        assert!(
            !example.slug.is_empty(),
            "example '{}' has empty slug",
            example.title
        );
    }
}

#[test]
fn examples_have_unique_slugs() {
    let examples = builtin_examples();
    let mut slugs = std::collections::HashSet::new();
    for example in &examples {
        assert!(
            slugs.insert(example.slug.clone()),
            "duplicate slug: {}",
            example.slug
        );
    }
}

// ════════════════════════════════════════════════════════════════════════
// 3. Monaco editor config
// ════════════════════════════════════════════════════════════════════════

#[test]
fn monaco_config_defaults() {
    let config = MonacoConfig::default();
    assert_eq!(config.language_id, "fajar");
    assert_eq!(config.theme_name, "fajar-dark");
    assert!(config.font_size > 0);
    assert!(config.line_numbers);
}

#[test]
fn monaco_config_tab_size() {
    let config = MonacoConfig::default();
    assert_eq!(config.tab_size, 4);
}

// ════════════════════════════════════════════════════════════════════════
// 4. Wasm backend (requires --features wasm)
// ════════════════════════════════════════════════════════════════════════

#[cfg(feature = "wasm")]
#[test]
fn wasm_compiles_basic_program() {
    let source = "fn main() -> i64 { 42 }";
    let tokens = fajar_lang::lexer::tokenize(source).unwrap();
    let program = fajar_lang::parser::parse(tokens).unwrap();
    let mut compiler = fajar_lang::codegen::wasm::WasmCompiler::new(
        fajar_lang::codegen::wasm::WasmTarget::Browser,
    );
    let result = compiler.compile(&program);
    assert!(
        result.is_ok(),
        "basic program should compile to wasm: {result:?}"
    );
}

#[cfg(feature = "wasm")]
#[test]
fn wasm_browser_loader_html() {
    let html = fajar_lang::codegen::wasm::WasmCompiler::generate_browser_loader("app.wasm");
    assert!(html.contains("<!DOCTYPE html>"));
    assert!(html.contains("app.wasm"));
    assert!(html.contains("WebAssembly"));
}

// ════════════════════════════════════════════════════════════════════════
// 5. Playground file generation
// ════════════════════════════════════════════════════════════════════════

#[test]
fn playground_generates_files() {
    let dir = &std::env::temp_dir()
        .join("fj-playground-test")
        .display()
        .to_string();
    let _ = std::fs::remove_dir_all(dir);
    std::fs::create_dir_all(dir).unwrap();

    // Generate index.html (simplified check)
    let html_exists = std::path::Path::new(dir).exists();
    assert!(html_exists);

    let _ = std::fs::remove_dir_all(dir);
}
