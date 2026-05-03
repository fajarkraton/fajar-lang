//! Error-code coverage harness — P4.C2 of FAJAR_LANG_PERFECTION_PLAN.
//!
//! Each cataloged error code in `docs/ERROR_CODES.md` MUST be triggered
//! by at least one test in this file (or in a feature-gated companion
//! file when codegen invocation is required).
//!
//! Tests are named `coverage_<code_lower>_<short_phrase>` so the audit
//! script `scripts/audit_error_codes.py` can mechanically map test
//! function → code → catalog row and flag any code missing coverage.
//!
//! Drivers:
//!   - `expect_lex_error(src, code)` — runs `lexer::tokenize`
//!   - `expect_parse_error(src, code)` — runs `lexer::tokenize` then `parser::parse`
//!   - `expect_semantic_error(src, code)` — pipeline through `analyzer::analyze`
//!   - `expect_strict_error(src, code)` — pipeline through `analyzer::analyze_strict`
//!   - `expect_runtime_error(src, code)` — full `Interpreter::eval_source`
//!
//! Each driver asserts that AT LEAST ONE error in the result formats with
//! the given code (either as `[CODE]` or `CODE:` per per-prefix convention
//! in src/).

use fajar_lang::FjError;
use fajar_lang::analyzer;
use fajar_lang::interpreter::Interpreter;
use fajar_lang::lexer;
use fajar_lang::parser;

// ════════════════════════════════════════════════════════════════════════
// Test driver helpers
// ════════════════════════════════════════════════════════════════════════

/// Assert the lexer rejects `src` with an error containing `code`.
fn expect_lex_error(src: &str, code: &str) {
    let errs = lexer::tokenize(src).expect_err(&format!(
        "expected lex to fail for {code}, but tokenize succeeded on: {src:?}"
    ));
    let any = errs.iter().any(|e| format!("{e}").contains(code));
    assert!(
        any,
        "expected lex error containing '{code}', got: {:#?}",
        errs.iter().map(|e| format!("{e}")).collect::<Vec<_>>()
    );
}

/// Assert that the parser rejects `src` (after lex) with an error containing `code`.
fn expect_parse_error(src: &str, code: &str) {
    let tokens = lexer::tokenize(src).expect("lex should succeed for parse-error test");
    let errs = parser::parse(tokens).expect_err(&format!(
        "expected parse to fail for {code}, but parse succeeded on: {src:?}"
    ));
    let any = errs.iter().any(|e| format!("{e}").contains(code));
    assert!(
        any,
        "expected parse error containing '{code}', got: {:#?}",
        errs.iter().map(|e| format!("{e}")).collect::<Vec<_>>()
    );
}

/// Assert that the analyzer rejects `src` with a default-mode error containing `code`.
#[allow(dead_code)]
fn expect_semantic_error(src: &str, code: &str) {
    let tokens = lexer::tokenize(src).expect("lex");
    let program = parser::parse(tokens).expect("parse");
    let errs = analyzer::analyze(&program).expect_err(&format!(
        "expected analyze to fail for {code}, but it succeeded on: {src:?}"
    ));
    let any = errs.iter().any(|e| format!("{e}").contains(code));
    assert!(
        any,
        "expected semantic error containing '{code}', got: {:#?}",
        errs.iter().map(|e| format!("{e}")).collect::<Vec<_>>()
    );
}

/// Assert that the strict analyzer rejects `src` with an error containing `code`.
/// Used for warning-level codes (SE009, SE010, etc.) that only fire under
/// `analyze_strict`.
#[allow(dead_code)]
fn expect_strict_error(src: &str, code: &str) {
    let tokens = lexer::tokenize(src).expect("lex");
    let program = parser::parse(tokens).expect("parse");
    let errs = analyzer::analyze_strict(&program).expect_err(&format!(
        "expected strict analyze to fail for {code}, but it succeeded on: {src:?}"
    ));
    let any = errs.iter().any(|e| format!("{e}").contains(code));
    assert!(
        any,
        "expected strict semantic error containing '{code}', got: {:#?}",
        errs.iter().map(|e| format!("{e}")).collect::<Vec<_>>()
    );
}

/// Assert the full pipeline (lex → parse → analyze → eval) emits an error
/// containing `code` (any layer).
#[allow(dead_code)]
fn expect_runtime_error(src: &str, code: &str) {
    let mut interp = Interpreter::new_capturing();
    let err = interp
        .eval_source(src)
        .expect_err(&format!("expected eval to fail for {code} on: {src:?}"));
    let formatted = match &err {
        FjError::Lex(es) => es
            .iter()
            .map(|e| format!("{e}"))
            .collect::<Vec<_>>()
            .join("\n"),
        FjError::Parse(es) => es
            .iter()
            .map(|e| format!("{e}"))
            .collect::<Vec<_>>()
            .join("\n"),
        FjError::Semantic(es) => es
            .iter()
            .map(|e| format!("{e}"))
            .collect::<Vec<_>>()
            .join("\n"),
        FjError::Runtime(re) => format!("{re}"),
    };
    assert!(
        formatted.contains(code),
        "expected pipeline error containing '{code}', got: {formatted}"
    );
}

// ════════════════════════════════════════════════════════════════════════
// LE — Lex Errors (8 codes: LE001-LE008)
// ════════════════════════════════════════════════════════════════════════

#[test]
fn coverage_le001_unexpected_char() {
    // `#` is not a recognized character outside string/comment context.
    expect_lex_error("let x = #", "LE001");
}

#[test]
fn coverage_le002_unterminated_string() {
    expect_lex_error("let s = \"hello\n", "LE002");
}

#[test]
fn coverage_le003_unterminated_block_comment() {
    expect_lex_error("let x = 1\n/* unclosed", "LE003");
}

#[test]
fn coverage_le004_invalid_number_literal() {
    expect_lex_error("let x = 0xZZ", "LE004");
}

#[test]
fn coverage_le005_invalid_escape_sequence() {
    expect_lex_error("let s = \"\\q\"", "LE005");
}

#[test]
fn coverage_le006_number_overflow() {
    expect_lex_error("let x = 99999999999999999999999999", "LE006");
}

#[test]
fn coverage_le007_empty_char_literal() {
    expect_lex_error("let c = ''", "LE007");
}

#[test]
fn coverage_le008_multi_char_literal() {
    expect_lex_error("let c = 'ab'", "LE008");
}

// ════════════════════════════════════════════════════════════════════════
// PE — Parse Errors (11 codes: PE001-PE011)
// ════════════════════════════════════════════════════════════════════════

#[test]
fn coverage_pe001_unexpected_token() {
    // Incomplete function body — parser expects `}` and hits EOF, fires
    // PE001 with `expected }, found EOF`.
    expect_parse_error("fn f() {", "PE001");
}

#[test]
fn coverage_pe002_expected_expression() {
    // Trailing operator with no rhs forces parser into expr context with no token.
    expect_parse_error("let x = 1 +", "PE002");
}

#[test]
fn coverage_pe003_expected_type() {
    expect_parse_error("fn f(x: ) { x }", "PE003");
}

#[test]
fn coverage_pe004_expected_pattern() {
    // Missing pattern in match arm.
    expect_parse_error("fn f() { match 1 { => 0 } }", "PE004");
}

#[test]
fn coverage_pe005_expected_identifier() {
    // `let = 42` — identifier expected after `let`.
    expect_parse_error("let = 42", "PE005");
}

// PE006-PE011: declared in `ParseError` enum but the current parser routes
// these conditions through PE001 UnexpectedToken (EOF case), PE004
// ExpectedPattern (bad pattern case), or no error at all (trailing separator,
// stray `@`, missing module file). The variants are reserved for future
// grammar additions where finer-grained diagnostics matter (e.g. dedicated
// PE011 ModuleFileNotFound when `mod foo` resolution is wired through the
// compiler driver). To prove the variant formats correctly today, we
// construct each one directly and assert the catalog code appears in the
// formatted output. When the parser starts emitting them naturally, replace
// the construction test with a parse-error trigger.
//
// See docs/ERROR_CODES.md §3 PE table for the framework-status annotation.

use fajar_lang::lexer::token::Span;
use fajar_lang::parser::ParseError;

#[test]
fn coverage_pe006_unexpected_eof_format() {
    let e = ParseError::UnexpectedEof {
        span: Span::new(0, 0),
    };
    assert!(format!("{e}").contains("PE006"), "got: {e}");
}

#[test]
fn coverage_pe007_invalid_pattern_format() {
    let e = ParseError::InvalidPattern {
        line: 1,
        col: 1,
        span: Span::new(0, 0),
    };
    assert!(format!("{e}").contains("PE007"), "got: {e}");
}

#[test]
fn coverage_pe008_duplicate_field() {
    expect_parse_error(
        "struct P { x: i64, y: i64 }\nfn f() { let _ = P { x: 1, x: 2, y: 3 } }",
        "PE008",
    );
}

#[test]
fn coverage_pe009_trailing_separator_format() {
    let e = ParseError::TrailingSeparator {
        line: 1,
        col: 1,
        span: Span::new(0, 0),
    };
    assert!(format!("{e}").contains("PE009"), "got: {e}");
}

#[test]
fn coverage_pe010_invalid_annotation_format() {
    let e = ParseError::InvalidAnnotation {
        line: 1,
        col: 1,
        span: Span::new(0, 0),
    };
    assert!(format!("{e}").contains("PE010"), "got: {e}");
}

#[test]
fn coverage_pe011_module_file_not_found_format() {
    let e = ParseError::ModuleFileNotFound {
        path: "nonexistent.fj".into(),
        span: Span::new(0, 0),
    };
    assert!(format!("{e}").contains("PE011"), "got: {e}");
}
