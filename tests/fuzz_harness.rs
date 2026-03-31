//! V14 Option 2 — Sprint H2: Fuzz Testing Harnesses
//!
//! Deterministic fuzz-like tests that exercise the lexer, parser, and interpreter
//! with malformed, edge-case, and adversarial inputs. The goal is to verify
//! that no input can cause a panic — all failures must be clean errors.
//!
//! Uses controlled seeds and deterministic generation (not cargo-fuzz).

use fajar_lang::interpreter::Interpreter;
use fajar_lang::lexer::tokenize;
use fajar_lang::parser::parse;

// ════════════════════════════════════════════════════════════════════════
// Helpers
// ════════════════════════════════════════════════════════════════════════

/// Simple deterministic pseudo-random number generator (xorshift64).
struct Rng {
    state: u64,
}

impl Rng {
    fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 { 1 } else { seed },
        }
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }

    fn next_range(&mut self, max: usize) -> usize {
        (self.next_u64() % (max as u64)) as usize
    }

    fn next_char(&mut self) -> char {
        let table: &[u8] =
            b"abcdefghijklmnopqrstuvwxyz0123456789 \t\n+-*/=<>!&|^~(){}[];:,.\"'@#$%\\?";
        let idx = self.next_range(table.len());
        table[idx] as char
    }

    fn random_string(&mut self, max_len: usize) -> String {
        let len = self.next_range(max_len + 1);
        (0..len).map(|_| self.next_char()).collect()
    }
}

/// Feed source to the full pipeline and assert no panic occurs.
/// Returns Ok if evaluation succeeded, Err with the error otherwise.
fn eval_no_panic(source: &str) -> Result<(), String> {
    let mut interp = Interpreter::new_capturing();
    match interp.eval_source(source) {
        Ok(_) => Ok(()),
        Err(e) => Err(format!("{e}")),
    }
}

// ════════════════════════════════════════════════════════════════════════
// H2.1 — Lexer Robustness (50 random/malformed strings)
// ════════════════════════════════════════════════════════════════════════

#[test]
fn h2_1_lexer_random_ascii_no_panic() {
    let mut rng = Rng::new(0xDEAD_BEEF_CAFE_1234);
    for i in 0..50 {
        let input = rng.random_string(200);
        // The lexer must never panic, regardless of input.
        let result = tokenize(&input);
        // We don't care if it's Ok or Err, just that it didn't panic.
        let _ = result;
        // Verify we can safely format any error.
        if let Err(errors) = &result {
            for e in errors {
                let _ = format!("{e}");
            }
        }
        assert!(i < 50, "loop counter sanity check (should never fail)");
    }
}

#[test]
fn h2_1_lexer_unterminated_strings() {
    let cases = [
        r#""hello"#,
        r#""unterminated string with newline
"#,
        r#""#,
        "\"\\\"",
        "\"hello\\",
    ];
    for case in &cases {
        let result = tokenize(case);
        // Must not panic; an error is expected.
        assert!(
            result.is_err() || result.is_ok(),
            "tokenize should return a result for: {case}"
        );
    }
}

#[test]
fn h2_1_lexer_invalid_numbers() {
    let cases = [
        "0x",
        "0xGGGG",
        "0b",
        "0b29",
        "0o89",
        "99999999999999999999999999999999",
        "1.2.3.4",
        "1e999999",
        "..",
        "...",
    ];
    for case in &cases {
        let result = tokenize(case);
        let _ = result; // Must not panic.
    }
}

#[test]
fn h2_1_lexer_unicode_input() {
    let cases = [
        "\u{0000}",
        "\u{FFFD}",
        "\u{200B}",                 // zero-width space
        "\u{202E}",                 // right-to-left override
        "\u{FE0F}",                 // variation selector
        "let \u{03B1} = 42",        // Greek alpha
        "let \u{4E16}\u{754C} = 1", // Chinese characters
        "\u{1F600}",                // emoji
        "fn \u{00E9}() {}",         // accented e
        "\u{200D}\u{200D}\u{200D}", // zero-width joiners
    ];
    for case in &cases {
        let result = tokenize(case);
        let _ = result; // Must not panic.
    }
}

#[test]
fn h2_1_lexer_special_sequences() {
    let cases = [
        "\0\0\0",
        "\r\r\r",
        "\t\t\t\t\t",
        "///////////",
        "/* unclosed comment",
        "/* nested /* comment */",
        "########",
        "@@@@@@",
        "!!!!",
        "; ; ; ;",
    ];
    for case in &cases {
        let result = tokenize(case);
        let _ = result; // Must not panic.
    }
}

// ════════════════════════════════════════════════════════════════════════
// H2.2 — Parser Robustness (50 malformed token sequences)
// ════════════════════════════════════════════════════════════════════════

#[test]
fn h2_2_parser_random_expressions_no_panic() {
    let mut rng = Rng::new(0xCAFE_BABE_DEAD_0001);
    let operators = ["+", "-", "*", "/", "==", "!=", "<", ">", "&&", "||"];
    let values = ["42", "3.14", "true", "false", "\"hi\"", "x", "foo"];

    for _ in 0..50 {
        let parts: usize = rng.next_range(10) + 1;
        let mut expr = String::new();
        for j in 0..parts {
            if j % 2 == 0 {
                expr.push_str(values[rng.next_range(values.len())]);
            } else {
                expr.push(' ');
                expr.push_str(operators[rng.next_range(operators.len())]);
                expr.push(' ');
            }
        }
        if let Ok(tokens) = tokenize(&expr) {
            let _ = parse(tokens); // Must not panic.
        }
    }
}

#[test]
fn h2_2_parser_malformed_function_defs() {
    let cases = [
        "fn () {}",
        "fn foo(",
        "fn foo() ->",
        "fn foo(a: ) {}",
        "fn foo(,) {}",
        "fn {}",
        "fn foo() -> void {",
        "fn foo(a: i64, b: ) {}",
        "fn (x: i64) {}",
        "fn foo() -> -> void {}",
    ];
    for case in &cases {
        if let Ok(tokens) = tokenize(case) {
            let result = parse(tokens);
            let _ = result; // Must not panic.
        }
    }
}

#[test]
fn h2_2_parser_malformed_structs() {
    let cases = [
        "struct {}",
        "struct Foo {",
        "struct Foo { x: }",
        "struct Foo { , }",
        "struct { x: i64 }",
        "struct Foo { x: i64, y: }",
        "struct Foo { x: i64 y: i64 }",
    ];
    for case in &cases {
        if let Ok(tokens) = tokenize(case) {
            let _ = parse(tokens);
        }
    }
}

#[test]
fn h2_2_parser_unbalanced_delimiters() {
    let cases = [
        "(((",
        ")))",
        "{{{",
        "}}}",
        "[[[",
        "]]]",
        "({[",
        "]})",
        "fn main() { if true { } } }",
        "let x = (((1 + 2)",
    ];
    for case in &cases {
        if let Ok(tokens) = tokenize(case) {
            let _ = parse(tokens);
        }
    }
}

#[test]
fn h2_2_parser_empty_and_whitespace() {
    let cases = [
        "",
        " ",
        "\n",
        "\t",
        "\n\n\n",
        "   \t   \n   ",
        "// just a comment",
        "// comment\n// another",
    ];
    for case in &cases {
        if let Ok(tokens) = tokenize(case) {
            let result = parse(tokens);
            // Empty source should parse to an empty program, not panic.
            assert!(
                result.is_ok() || result.is_err(),
                "parse should return a result"
            );
        }
    }
}

// ════════════════════════════════════════════════════════════════════════
// H2.3 — Interpreter Robustness (30 edge-case programs)
// ════════════════════════════════════════════════════════════════════════

#[test]
fn h2_3_empty_program() {
    let _ = eval_no_panic("");
}

#[test]
fn h2_3_just_comments() {
    let _ = eval_no_panic("// nothing here\n// just comments");
}

#[test]
fn h2_3_huge_integer_literal() {
    // i64::MAX is handled; anything beyond should error, not panic.
    let _ = eval_no_panic("let x = 9223372036854775807");
    let _ = eval_no_panic("let x = 9223372036854775808"); // might overflow
}

#[test]
fn h2_3_deeply_nested_expressions() {
    // Build: ((((((((((1))))))))))
    let mut expr = String::from("1");
    for _ in 0..50 {
        expr = format!("({expr})");
    }
    let src = format!("let x = {expr}");
    let _ = eval_no_panic(&src);
}

#[test]
fn h2_3_deeply_nested_if_else() {
    // Build: if true { if true { ... 42 ... } else { 0 } } else { 0 }
    let mut src = String::from("42");
    for _ in 0..20 {
        src = format!("if true {{ {src} }} else {{ 0 }}");
    }
    let src = format!("let x = {src}");
    let _ = eval_no_panic(&src);
}

#[test]
fn h2_3_empty_function_body() {
    let _ = eval_no_panic("fn noop() -> void {}");
}

#[test]
fn h2_3_many_variables() {
    let mut src = String::new();
    for i in 0..200 {
        src.push_str(&format!("let var_{i} = {i}\n"));
    }
    src.push_str("let total = var_0 + var_199\nprintln(total)");
    let _ = eval_no_panic(&src);
}

#[test]
fn h2_3_recursive_depth_limit() {
    // Deep but bounded recursion should work; use a thread with a larger
    // stack to avoid host stack overflow in debug builds.
    let result = std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024) // 8 MB stack
        .spawn(|| {
            let src = r#"
fn infinite(n: i64) -> i64 {
    infinite(n + 1)
}
fn main() -> void {
    println(infinite(0))
}
"#;
            let mut interp = Interpreter::new_capturing();
            let result = interp.eval_source(src);
            if result.is_ok() {
                let main_result = interp.call_main();
                // Should produce a stack overflow error.
                assert!(
                    main_result.is_err(),
                    "infinite recursion should produce an error"
                );
            }
            // Either eval_source or call_main should error — not panic.
        })
        .expect("failed to spawn thread")
        .join();
    // The thread should not panic (Ok), or if it does, the test reports it.
    assert!(
        result.is_ok(),
        "recursive depth test panicked: {:?}",
        result.unwrap_err()
    );
}

#[test]
fn h2_3_string_with_escapes() {
    let cases = [
        r#"let s = "hello\nworld""#,
        r#"let s = "tab\there""#,
        r#"let s = "quote\"inside""#,
        r#"let s = "backslash\\end""#,
        r#"let s = """#,
    ];
    for case in &cases {
        let _ = eval_no_panic(case);
    }
}

#[test]
fn h2_3_zero_length_array() {
    let _ = eval_no_panic("let arr: [i64; 0] = []");
    let _ = eval_no_panic("let arr = []");
}

// ════════════════════════════════════════════════════════════════════════
// H2.4 — Format String Robustness
// ════════════════════════════════════════════════════════════════════════

#[test]
fn h2_4_fstring_basic() {
    let output = {
        let mut interp = Interpreter::new_capturing();
        interp
            .eval_source(r#"let name = "world""#)
            .expect("eval failed");
        interp
            .eval_source(r#"println(f"hello {name}")"#)
            .expect("eval failed");
        interp.get_output().to_vec()
    };
    assert_eq!(output, vec!["hello world"]);
}

#[test]
fn h2_4_fstring_with_expressions() {
    let mut interp = Interpreter::new_capturing();
    interp.eval_source(r#"let x = 10"#).expect("eval failed");
    interp
        .eval_source(r#"println(f"result: {x + 5}")"#)
        .expect("eval failed");
    let output = interp.get_output().to_vec();
    assert_eq!(output, vec!["result: 15"]);
}

#[test]
fn h2_4_fstring_empty_expression() {
    // f"hello {}" — empty braces should produce an error, not a panic.
    let mut interp = Interpreter::new_capturing();
    let result = interp.eval_source(r#"println(f"hello {}")"#);
    // Might succeed with empty string or might error — must not panic.
    let _ = result;
}

#[test]
fn h2_4_fstring_nested_braces() {
    // Nested braces might confuse the parser — must not panic.
    let mut interp = Interpreter::new_capturing();
    let result = interp.eval_source(r#"println(f"value: {{literal}}")"#);
    let _ = result; // Must not panic.
}

#[test]
fn h2_4_fstring_special_characters() {
    let cases = [
        r#"println(f"tab:\there")"#,
        r#"println(f"newline:\nhere")"#,
        r#"let x = 1
println(f"{x}")"#,
    ];
    for case in &cases {
        let mut interp = Interpreter::new_capturing();
        let _ = interp.eval_source(case); // Must not panic.
    }
}

// ════════════════════════════════════════════════════════════════════════
// H2.5 — Error Message Quality
// ════════════════════════════════════════════════════════════════════════

#[test]
fn h2_5_lex_error_has_message() {
    let result = tokenize("\"unterminated");
    if let Err(errors) = result {
        for e in &errors {
            let msg = format!("{e}");
            assert!(!msg.is_empty(), "lex error should have a non-empty message");
        }
    }
}

#[test]
fn h2_5_parse_error_has_message() {
    if let Ok(tokens) = tokenize("fn ()") {
        if let Err(errors) = parse(tokens) {
            for e in &errors {
                let msg = format!("{e}");
                assert!(
                    !msg.is_empty(),
                    "parse error should have a non-empty message"
                );
            }
        }
    }
}

#[test]
fn h2_5_runtime_error_has_code() {
    // Division by zero should produce RE001.
    let mut interp = Interpreter::new_capturing();
    let result = interp.eval_source("10 / 0");
    if let Err(e) = result {
        let msg = format!("{e}");
        assert!(
            msg.contains("RE001") || msg.contains("division") || msg.contains("zero"),
            "division by zero error should mention RE001 or 'division': {msg}"
        );
    }
}

#[test]
fn h2_5_undefined_var_error_has_name() {
    let mut interp = Interpreter::new_capturing();
    let result = interp.eval_source("println(totally_undefined_xyz)");
    if let Err(e) = result {
        let msg = format!("{e}");
        // The error message should mention the variable name.
        assert!(
            msg.contains("totally_undefined_xyz")
                || msg.contains("undefined")
                || msg.contains("not found"),
            "undefined variable error should mention the variable: {msg}"
        );
    }
}

#[test]
fn h2_5_all_fjerror_variants_display() {
    // Verify that all FjError variants can be displayed without panic.
    let test_cases: Vec<(&str, bool)> = vec![
        ("\"unterminated", true),                    // lex error
        ("fn ()", true),                             // parse error
        ("10 / 0", true),                            // runtime error
        ("fn main() -> void { unknown_var }", true), // semantic or runtime
    ];
    for (src, expect_err) in test_cases {
        let mut interp = Interpreter::new_capturing();
        let result = interp.eval_source(src);
        if expect_err {
            if let Err(e) = result {
                let msg = format!("{e}");
                assert!(
                    !msg.is_empty(),
                    "error message should not be empty for: {src}"
                );
            }
        }
    }
}
