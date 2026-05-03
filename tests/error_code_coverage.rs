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
use fajar_lang::analyzer::type_check::TypeChecker;
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

/// Assert that any diagnostic (error OR warning) emitted during analysis
/// contains `code`. Use for warning-level codes (SE009/SE010/SE019/SE020)
/// that `analyzer::analyze` filters out of its Result.
fn expect_diagnostic(src: &str, code: &str) {
    let tokens = lexer::tokenize(src).expect("lex");
    let program = parser::parse(tokens).expect("parse");
    let mut tc = TypeChecker::new();
    let _ = tc.analyze(&program);
    let diags = tc.diagnostics();
    let any = diags.iter().any(|e| format!("{e}").contains(code));
    assert!(
        any,
        "expected diagnostic containing '{code}', got: {:#?}",
        diags.iter().map(|e| format!("{e}")).collect::<Vec<_>>()
    );
}

/// Assert that the strict analyzer rejects `src` with an error containing `code`.
/// Strict mode (`analyze_strict`) flips Move-semantic errors on for non-Copy
/// types (ME001/ME003 etc.); it does NOT promote warnings to errors.
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

// ════════════════════════════════════════════════════════════════════════
// SE — Semantic Errors (SE001-SE023, with overloaded codes)
// ════════════════════════════════════════════════════════════════════════

#[test]
fn coverage_se001_undefined_variable() {
    expect_semantic_error("fn main() { let _ = undefined_var }", "SE001");
}

#[test]
fn coverage_se002_undefined_function() {
    // SE002 fires when calling a value whose resolved type isn't function-like.
    expect_semantic_error("fn main() { let x = 42\nx() }", "SE002");
}

#[test]
fn coverage_se003_undefined_type_format() {
    // SE003 UndefinedType: declared in SemanticError but the analyzer
    // currently silently treats unknown type names as Type::Unknown
    // rather than emitting SE003. Variant is reserved; we validate
    // its Display impl by direct construction so it cannot drift.
    use fajar_lang::analyzer::type_check::SemanticError;
    use fajar_lang::lexer::token::Span;
    let e = SemanticError::UndefinedType {
        name: "NoSuchType".into(),
        span: Span::new(0, 0),
        suggestion: None,
    };
    assert!(format!("{e}").contains("SE003"), "got: {e}");
}

#[test]
fn coverage_se004_type_mismatch() {
    expect_semantic_error("fn main() { let x: i64 = \"not int\" }", "SE004");
}

#[test]
fn coverage_se005_argument_count_mismatch() {
    expect_semantic_error(
        "fn add(a: i64, b: i64) -> i64 { a + b }\nfn main() { add(1) }",
        "SE005",
    );
}

#[test]
fn coverage_se006_duplicate_definition() {
    // SE006 DuplicateDefinition fires for duplicate trait methods.
    expect_semantic_error(
        "trait Dup { fn foo(self) -> i64\nfn foo(self) -> i64 }",
        "SE006",
    );
}

#[test]
fn coverage_se007_immutable_assignment() {
    expect_semantic_error("fn main() { let x = 1\nx = 2 }", "SE007");
}

#[test]
fn coverage_se008_missing_return_format() {
    // SE008 MissingReturn: declared but the analyzer currently uses
    // SE004 TypeMismatch when the inferred body type doesn't match
    // the declared return type. Reserved for richer diagnostic.
    use fajar_lang::analyzer::type_check::SemanticError;
    use fajar_lang::lexer::token::Span;
    let e = SemanticError::MissingReturn {
        name: "f".into(),
        expected: "i64".into(),
        span: Span::new(0, 0),
    };
    assert!(format!("{e}").contains("SE008"), "got: {e}");
}

#[test]
fn coverage_se009_unused_variable_diag() {
    // SE009 is filtered out of analyze() Result (warning-level);
    // surfaces via TypeChecker::diagnostics().
    expect_diagnostic("fn main() { let unused_local = 42 }", "SE009");
}

#[test]
fn coverage_se010_unreachable_code_diag() {
    expect_diagnostic("fn f() -> i64 { return 1\n42 }", "SE010");
}

#[test]
fn coverage_se011_non_exhaustive_match() {
    // Match on bool with only `true` branch — missing `false`.
    expect_semantic_error("fn main() { let _ = match true { true => 1 } }", "SE011");
}

#[test]
fn coverage_se012_missing_field() {
    expect_semantic_error(
        "struct P { x: i64, y: i64 }\nfn main() { let _ = P { x: 1 } }",
        "SE012",
    );
}

#[test]
fn coverage_se013_ffi_unsafe_type() {
    // SE013 fires for non-FFI-safe types in `extern fn`. `String` is
    // not FFI-safe (heap-managed; opaque ABI).
    expect_semantic_error("extern fn bad(s: String) -> i64", "SE013");
}

#[test]
fn coverage_se014_trait_bound_not_satisfied_format() {
    // SE014 TraitBoundNotSatisfied: declared but the current analyzer
    // routes generic-bound failures through SE015 UnknownTrait or
    // SE004 TypeMismatch depending on path. Variant validated via
    // direct construction.
    use fajar_lang::analyzer::type_check::SemanticError;
    use fajar_lang::lexer::token::Span;
    let e = SemanticError::TraitBoundNotSatisfied {
        concrete_type: "Bar".into(),
        trait_name: "Foo".into(),
        param_name: "T".into(),
        span: Span::new(0, 0),
    };
    assert!(format!("{e}").contains("SE014"), "got: {e}");
}

#[test]
fn coverage_se015_unknown_trait() {
    expect_semantic_error(
        "fn use_unknown<T: NoSuchTrait>(t: T) -> i64 { 0 }\nfn main() { let _ = use_unknown(1) }",
        "SE015",
    );
}

#[test]
fn coverage_se017_await_outside_async_format() {
    // SE017 AwaitOutsideAsync: declared but the parser/analyzer
    // currently allows .await syntactically and resolves the awaited
    // expression first (often emitting SE001/SE004). Variant validated
    // via direct construction.
    use fajar_lang::analyzer::type_check::SemanticError;
    use fajar_lang::lexer::token::Span;
    let e = SemanticError::AwaitOutsideAsync {
        span: Span::new(0, 0),
    };
    assert!(format!("{e}").contains("SE017"), "got: {e}");
}

#[test]
fn coverage_se019_unused_import_format() {
    // SE019 UnusedImport: declared but `use` paths are not currently
    // checked for live usage; reserved.
    use fajar_lang::analyzer::type_check::SemanticError;
    use fajar_lang::lexer::token::Span;
    let e = SemanticError::UnusedImport {
        name: "HashMap".into(),
        span: Span::new(0, 0),
    };
    assert!(format!("{e}").contains("SE019"), "got: {e}");
}

#[test]
fn coverage_se020_unreachable_pattern_diag() {
    // SE020 fires when match arms are listed AFTER a catch-all wildcard.
    expect_diagnostic(
        "fn main() { let x = 1\nlet _ = match x { _ => 0, 1 => 2 } }",
        "SE020",
    );
}

#[test]
fn coverage_se022_index_out_of_bounds_compile_time() {
    // SE022 needs both array length AND index to be const-resolvable.
    // Direct array literal indexed by integer literal triggers it.
    expect_semantic_error("fn main() { let _ = [1, 2, 3][99] }", "SE022");
}

#[test]
fn coverage_se023_quantized_not_dequantized() {
    expect_semantic_error(
        "fn main() { let t = from_data([1.0, -0.5], [2])\nlet q = quantize(t, 4)\nlet _ = matmul(q, q) }",
        "SE023",
    );
}
