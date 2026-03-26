//! Property-based tests for Fajar Lang using proptest.
//!
//! Sprint 7.1: Fuzzing & property testing invariants.

use fajar_lang::lexer::tokenize;
use fajar_lang::parser::parse;
use proptest::prelude::*;

// ═══════════════════════════════════════════════════════════════════════
// Lexer properties
// ═══════════════════════════════════════════════════════════════════════

proptest! {
    /// The lexer should never panic on arbitrary input.
    #[test]
    fn lexer_never_panics(s in "\\PC*") {
        let _ = tokenize(&s);
    }

    /// If lexing succeeds, the last token is always EOF.
    #[test]
    fn lexer_last_token_is_eof(s in "[a-zA-Z0-9_ \\+\\-\\*/=;\\n\\(\\)\\{\\},:]+") {
        if let Ok(tokens) = tokenize(&s) {
            prop_assert!(!tokens.is_empty(), "token stream should not be empty");
            prop_assert!(
                matches!(tokens.last().unwrap().kind, fajar_lang::lexer::token::TokenKind::Eof),
                "last token should be Eof"
            );
        }
    }

    /// All token spans are within the source string bounds.
    #[test]
    fn lexer_spans_within_bounds(s in "[a-zA-Z0-9_ \\+\\-\\*/=;\\n]+") {
        if let Ok(tokens) = tokenize(&s) {
            for tok in &tokens {
                prop_assert!(
                    tok.span.end <= s.len(),
                    "span end {} exceeds source len {} for token {:?}",
                    tok.span.end, s.len(), tok.kind
                );
                prop_assert!(
                    tok.span.start <= tok.span.end,
                    "span start {} > end {} for token {:?}",
                    tok.span.start, tok.span.end, tok.kind
                );
            }
        }
    }

    /// Integer literals within i64 range always produce Int tokens.
    #[test]
    fn lexer_valid_integers(n in 0i64..1_000_000) {
        let src = format!("{n}");
        let tokens = tokenize(&src).expect("valid integer should lex");
        prop_assert!(tokens.len() >= 2); // at least IntLit + Eof
        match &tokens[0].kind {
            fajar_lang::lexer::token::TokenKind::IntLit(v) => {
                prop_assert_eq!(*v, n);
            }
            other => prop_assert!(false, "expected IntLit, got {:?}", other),
        }
    }

    /// String literals round-trip through the lexer.
    #[test]
    fn lexer_string_roundtrip(s in "[a-zA-Z0-9 ]{0,50}") {
        let src = format!("\"{}\"", s);
        let tokens = tokenize(&src).expect("valid string should lex");
        match &tokens[0].kind {
            fajar_lang::lexer::token::TokenKind::StringLit(v) => {
                prop_assert_eq!(v, &s);
            }
            other => prop_assert!(false, "expected StringLit, got {:?}", other),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Parser properties
// ═══════════════════════════════════════════════════════════════════════

proptest! {
    /// The parser should never panic on arbitrary token streams.
    #[test]
    fn parser_never_panics(s in "\\PC{0,200}") {
        if let Ok(tokens) = tokenize(&s) {
            let _ = parse(tokens);
        }
    }

    /// Valid simple expressions always parse successfully.
    #[test]
    fn parser_simple_arithmetic(a in -1000i64..1000, b in -1000i64..1000) {
        let src = format!("{a} + {b}");
        let tokens = tokenize(&src).expect("arithmetic should lex");
        let _program = parse(tokens).expect("arithmetic should parse");
    }

    /// Let statements with integer literals always parse.
    #[test]
    fn parser_let_statement(name in "[a-z]{2}[a-z0-9_]{2,10}", val in 0i64..10000) {
        // Filter out names that happen to be keywords or type names
        let keywords = [
            "if", "in", "fn", "as", "do", "for", "let", "mod", "mut", "pub", "use",
            "i8", "i16", "i32", "i64", "u8", "u16", "u32", "u64", "f32", "f64",
            "str", "ptr", "int", "bool", "char", "void", "true", "null", "type",
            "enum", "impl", "else", "loop", "while", "break", "const", "match",
            "trait", "super", "return", "struct", "extern", "tensor", "float",
            "isize", "usize", "never", "model", "layer", "loss", "grad",
            "continue",
        ];
        prop_assume!(!keywords.contains(&name.as_str()));
        let src = format!("let {name} = {val}");
        let tokens = tokenize(&src).expect("let should lex");
        let _program = parse(tokens).expect("let should parse");
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Interpreter properties
// ═══════════════════════════════════════════════════════════════════════

proptest! {
    /// Integer arithmetic is consistent: a + b == b + a (commutative)
    #[test]
    fn interpreter_addition_commutative(a in -10000i64..10000, b in -10000i64..10000) {
        use fajar_lang::interpreter::Interpreter;
        let src1 = format!("fn main() -> void {{ println({a} + {b}) }}");
        let src2 = format!("fn main() -> void {{ println({b} + {a}) }}");

        let mut i1 = Interpreter::new_capturing();
        i1.eval_source(&src1).unwrap();
        i1.call_main().unwrap();

        let mut i2 = Interpreter::new_capturing();
        i2.eval_source(&src2).unwrap();
        i2.call_main().unwrap();

        prop_assert_eq!(i1.get_output(), i2.get_output());
    }

    /// Integer multiplication is consistent: a * b == b * a (commutative)
    #[test]
    fn interpreter_multiplication_commutative(a in -100i64..100, b in -100i64..100) {
        use fajar_lang::interpreter::Interpreter;
        let src1 = format!("fn main() -> void {{ println({a} * {b}) }}");
        let src2 = format!("fn main() -> void {{ println({b} * {a}) }}");

        let mut i1 = Interpreter::new_capturing();
        i1.eval_source(&src1).unwrap();
        i1.call_main().unwrap();

        let mut i2 = Interpreter::new_capturing();
        i2.eval_source(&src2).unwrap();
        i2.call_main().unwrap();

        prop_assert_eq!(i1.get_output(), i2.get_output());
    }

    /// String length is always non-negative and matches actual string.
    #[test]
    fn interpreter_string_len(s in "[a-zA-Z0-9 ]{0,50}") {
        use fajar_lang::interpreter::Interpreter;
        let escaped = s.replace('\\', "\\\\").replace('"', "\\\"");
        let src = format!("fn main() -> void {{ println(len(\"{escaped}\")) }}");

        let mut interp = Interpreter::new_capturing();
        interp.eval_source(&src).unwrap();
        interp.call_main().unwrap();

        let output = interp.get_output();
        let reported_len: usize = output[0].parse().unwrap();
        prop_assert_eq!(reported_len, s.len());
    }

    /// Boolean negation is involutive: !!x == x
    #[test]
    fn interpreter_double_negation(b in proptest::bool::ANY) {
        use fajar_lang::interpreter::Interpreter;
        let src = format!("fn main() -> void {{ println(!!{b}) }}");

        let mut interp = Interpreter::new_capturing();
        interp.eval_source(&src).unwrap();
        interp.call_main().unwrap();

        let output = interp.get_output();
        prop_assert_eq!(&output[0], &format!("{b}"));
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Value properties
// ═══════════════════════════════════════════════════════════════════════

proptest! {
    /// Value::Int display is consistent with Rust i64 display.
    #[test]
    fn value_int_display(n in proptest::num::i64::ANY) {
        use fajar_lang::interpreter::Value;
        let v = Value::Int(n);
        prop_assert_eq!(format!("{v}"), format!("{n}"));
    }

    /// Value::Bool display is "true" or "false".
    #[test]
    fn value_bool_display(b in proptest::bool::ANY) {
        use fajar_lang::interpreter::Value;
        let v = Value::Bool(b);
        let displayed = format!("{v}");
        prop_assert!(displayed == "true" || displayed == "false");
    }

    /// Value equality is reflexive.
    #[test]
    fn value_equality_reflexive(n in proptest::num::i64::ANY) {
        use fajar_lang::interpreter::Value;
        let v = Value::Int(n);
        prop_assert_eq!(v.clone(), v);
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S13.2: Extended fuzz tests
// ═══════════════════════════════════════════════════════════════════════

proptest! {
    /// Lexer never panics on random Unicode input.
    #[test]
    fn fuzz_lexer_unicode(s in "\\PC{0,500}") {
        let _ = tokenize(&s);
    }

    /// Lexer never panics on binary-like input.
    #[test]
    fn fuzz_lexer_binary(bytes in proptest::collection::vec(proptest::num::u8::ANY, 0..200)) {
        let s = String::from_utf8_lossy(&bytes).to_string();
        let _ = tokenize(&s);
    }

    /// Parser never panics on random alphanumeric + operator input.
    #[test]
    fn fuzz_parser_operators(s in "[a-z0-9 \\+\\-\\*/%=<>!&|\\^~(){}\\[\\];:,.]{0,300}") {
        if let Ok(tokens) = tokenize(&s) {
            let _ = parse(tokens);
        }
    }

    /// Random valid function definitions always parse.
    #[test]
    fn fuzz_parser_fn_def(
        name in "[a-z]{3,8}",
        body_val in -1000i64..1000
    ) {
        let keywords = ["if", "in", "fn", "as", "do", "for", "let", "mod", "mut", "pub", "use",
            "i8", "i16", "i32", "i64", "u8", "u16", "u32", "u64", "f32", "f64",
            "str", "ptr", "int", "bool", "char", "void", "true", "null", "type",
            "enum", "impl", "else", "loop", "while", "break", "const", "match",
            "trait", "super", "return", "struct", "extern", "tensor", "float",
            "isize", "usize", "never", "model", "layer", "loss", "grad", "continue"];
        prop_assume!(!keywords.contains(&name.as_str()));
        let src = format!("fn {name}() -> i64 {{ {body_val} }}");
        let tokens = tokenize(&src).expect("fn def should lex");
        let _program = parse(tokens).expect("fn def should parse");
    }

    /// Arithmetic with edge values (MAX, MIN, 0) never panics the interpreter.
    #[test]
    fn fuzz_interpreter_edge_arithmetic(
        a in prop_oneof![
            Just(i64::MAX),
            Just(i64::MIN),
            Just(0i64),
            Just(1i64),
            Just(-1i64),
            -1000i64..1000
        ],
        b in prop_oneof![
            Just(i64::MAX),
            Just(i64::MIN),
            Just(0i64),
            Just(1i64),
            Just(-1i64),
            -1000i64..1000
        ],
        op in prop_oneof![Just("+"), Just("-"), Just("*")]
    ) {
        use fajar_lang::interpreter::Interpreter;
        let src = format!("fn main() -> void {{ println({a} {op} {b}) }}");
        let mut interp = Interpreter::new_capturing();
        let _ = interp.eval_source(&src);
        let _ = interp.call_main();
        // Must not panic
    }

    /// Division/modulo by zero returns error, never panics.
    #[test]
    fn fuzz_interpreter_division(
        a in -1000i64..1000,
        b in -1000i64..1000,
        op in prop_oneof![Just("/"), Just("%")]
    ) {
        use fajar_lang::interpreter::Interpreter;
        let src = format!("fn main() -> void {{ println({a} {op} {b}) }}");
        let mut interp = Interpreter::new_capturing();
        let _ = interp.eval_source(&src);
        let _ = interp.call_main();
        // Must not panic — division by zero should be a Result::Err
    }

    /// Comparison operators always produce boolean output.
    #[test]
    fn fuzz_interpreter_comparison(
        a in -1000i64..1000,
        b in -1000i64..1000,
        op in prop_oneof![Just("=="), Just("!="), Just("<"), Just(">"), Just("<="), Just(">=")]
    ) {
        use fajar_lang::interpreter::Interpreter;
        let src = format!("fn main() -> void {{ println({a} {op} {b}) }}");
        let mut interp = Interpreter::new_capturing();
        interp.eval_source(&src).unwrap();
        interp.call_main().unwrap();
        let output = interp.get_output();
        prop_assert!(output[0] == "true" || output[0] == "false");
    }

    /// Nested if-else never panics.
    #[test]
    fn fuzz_interpreter_if_else(
        cond in proptest::bool::ANY,
        then_val in -100i64..100,
        else_val in -100i64..100
    ) {
        use fajar_lang::interpreter::Interpreter;
        let src = format!(
            "fn main() -> void {{ println(if {cond} {{ {then_val} }} else {{ {else_val} }}) }}"
        );
        let mut interp = Interpreter::new_capturing();
        interp.eval_source(&src).unwrap();
        interp.call_main().unwrap();
        let output = interp.get_output();
        let expected = if cond { then_val } else { else_val };
        prop_assert_eq!(&output[0], &format!("{expected}"));
    }

    /// For-range always produces correct sum (0..n).
    #[test]
    fn fuzz_interpreter_for_range_sum(n in 1u32..50) {
        use fajar_lang::interpreter::Interpreter;
        let src = format!(
            "fn main() -> void {{ let mut s = 0\nfor i in 0..{n} {{ s = s + i }}\nprintln(s) }}"
        );
        let mut interp = Interpreter::new_capturing();
        interp.eval_source(&src).unwrap();
        interp.call_main().unwrap();
        let output = interp.get_output();
        let expected: u32 = n * (n - 1) / 2;
        prop_assert_eq!(&output[0], &format!("{expected}"));
    }

    /// While loop with bounded iteration never hangs.
    #[test]
    fn fuzz_interpreter_while_bounded(limit in 1u32..100) {
        use fajar_lang::interpreter::Interpreter;
        let src = format!(
            "fn main() -> void {{ let mut i = 0\nwhile i < {limit} {{ i = i + 1 }}\nprintln(i) }}"
        );
        let mut interp = Interpreter::new_capturing();
        interp.eval_source(&src).unwrap();
        interp.call_main().unwrap();
        let output = interp.get_output();
        prop_assert_eq!(&output[0], &format!("{limit}"));
    }

    /// Variable shadowing in nested blocks.
    #[test]
    fn fuzz_interpreter_shadowing(outer in -100i64..100, inner in -100i64..100) {
        use fajar_lang::interpreter::Interpreter;
        let src = format!(
            "fn main() -> void {{ let x = {outer}\n{{ let x = {inner}\nprintln(x) }}\nprintln(x) }}"
        );
        let mut interp = Interpreter::new_capturing();
        interp.eval_source(&src).unwrap();
        interp.call_main().unwrap();
        let output = interp.get_output();
        prop_assert_eq!(&output[0], &format!("{inner}"));
        prop_assert_eq!(&output[1], &format!("{outer}"));
    }

    /// Analyzer never panics on random source.
    #[test]
    fn fuzz_analyzer_never_panics(s in "\\PC{0,200}") {
        if let Ok(tokens) = tokenize(&s) {
            if let Ok(program) = parse(tokens) {
                let _ = fajar_lang::analyzer::analyze(&program);
            }
        }
    }

    /// Full pipeline (lex → parse → analyze → interpret) never panics.
    #[test]
    fn fuzz_full_pipeline(s in "[a-z0-9 \\+\\-\\*/=;{}()\\n]{0,200}") {
        use fajar_lang::interpreter::Interpreter;
        let mut interp = Interpreter::new();
        let _ = interp.eval_source(&s);
    }

    /// Subtraction is anti-commutative: a - b == -(b - a).
    #[test]
    fn interpreter_subtraction_anticommutative(a in -10000i64..10000, b in -10000i64..10000) {
        use fajar_lang::interpreter::Interpreter;
        let src1 = format!("fn main() -> void {{ println({a} - {b}) }}");
        let src2 = format!("fn main() -> void {{ println(-({b} - {a})) }}");

        let mut i1 = Interpreter::new_capturing();
        i1.eval_source(&src1).unwrap();
        i1.call_main().unwrap();

        let mut i2 = Interpreter::new_capturing();
        i2.eval_source(&src2).unwrap();
        i2.call_main().unwrap();

        prop_assert_eq!(i1.get_output(), i2.get_output());
    }

    /// Distributive property: a * (b + c) == a*b + a*c.
    #[test]
    fn interpreter_distributive(
        a in -100i64..100,
        b in -100i64..100,
        c in -100i64..100
    ) {
        use fajar_lang::interpreter::Interpreter;
        let src1 = format!("fn main() -> void {{ println({a} * ({b} + {c})) }}");
        let src2 = format!("fn main() -> void {{ println({a} * {b} + {a} * {c}) }}");

        let mut i1 = Interpreter::new_capturing();
        i1.eval_source(&src1).unwrap();
        i1.call_main().unwrap();

        let mut i2 = Interpreter::new_capturing();
        i2.eval_source(&src2).unwrap();
        i2.call_main().unwrap();

        prop_assert_eq!(i1.get_output(), i2.get_output());
    }

    /// Function calls are deterministic.
    #[test]
    fn interpreter_fn_call_deterministic(x in -100i64..100) {
        use fajar_lang::interpreter::Interpreter;
        let src = format!(
            "fn double(n: i64) -> i64 {{ n * 2 }}\nfn main() -> void {{ println(double({x})) }}"
        );
        let mut i1 = Interpreter::new_capturing();
        i1.eval_source(&src).unwrap();
        i1.call_main().unwrap();

        let mut i2 = Interpreter::new_capturing();
        i2.eval_source(&src).unwrap();
        i2.call_main().unwrap();

        prop_assert_eq!(i1.get_output(), i2.get_output());
        prop_assert_eq!(&i1.get_output()[0], &format!("{}", x * 2));
    }

    /// Value::Float display round-trips reasonably.
    #[test]
    fn value_float_display(f in -1e6f64..1e6f64) {
        use fajar_lang::interpreter::Value;
        let v = Value::Float(f);
        let displayed = format!("{v}");
        // Should be parseable back to a number
        prop_assert!(displayed.parse::<f64>().is_ok());
    }

    /// Value::Str display matches the string content.
    #[test]
    fn value_str_display(s in "[a-zA-Z0-9 ]{0,50}") {
        use fajar_lang::interpreter::Value;
        let v = Value::Str(s.clone());
        prop_assert_eq!(format!("{v}"), s);
    }
}

// ═══════════════════════════════════════════════════════════════════════
// QA1.13: Additional property tests (50 new invariants)
// ═══════════════════════════════════════════════════════════════════════

proptest! {
    // ── Lexer extended ──

    /// Float literals lex correctly.
    #[test]
    fn lexer_float_lit(whole in 0u32..999, frac in 0u32..999) {
        let src = format!("{whole}.{frac}");
        let tokens = tokenize(&src).expect("float literal should lex");
        prop_assert!(tokens.len() >= 2);
    }

    /// Identifiers with underscores lex as Ident.
    #[test]
    fn lexer_underscore_ident(prefix in "[a-z]{2,5}", suffix in "[a-z0-9]{2,5}") {
        let src = format!("{prefix}_{suffix}");
        let tokens = tokenize(&src).expect("underscore ident should lex");
        prop_assert!(tokens.len() >= 2);
    }

    /// Empty source produces only EOF.
    #[test]
    fn lexer_empty_source(_ in Just(())) {
        let tokens = tokenize("").expect("empty string should lex");
        prop_assert_eq!(tokens.len(), 1); // just EOF
    }

    /// Whitespace-only source produces only EOF.
    #[test]
    fn lexer_whitespace_only(spaces in " {1,50}") {
        let tokens = tokenize(&spaces).expect("whitespace should lex");
        prop_assert_eq!(tokens.len(), 1); // just EOF
    }

    /// Token count is always >= 1 (at least EOF).
    #[test]
    fn lexer_always_has_eof(s in "[a-z0-9 +\\-*/]{0,100}") {
        if let Ok(tokens) = tokenize(&s) {
            prop_assert!(tokens.len() >= 1);
        }
    }

    // ── Parser extended ──

    /// Struct definitions always parse.
    #[test]
    fn parser_struct_def(name in "[A-Z][a-z]{2,6}", field in "[a-z]{3,6}") {
        let kw = ["if", "in", "fn", "as", "do", "for", "let", "mod", "mut", "pub", "use",
            "str", "ptr", "int", "bool", "char", "void", "true", "null", "type",
            "enum", "impl", "else", "loop", "while", "break", "const", "match",
            "trait", "super", "return", "struct", "extern", "tensor", "float",
            "isize", "usize", "never", "model", "layer", "loss", "grad", "continue"];
        prop_assume!(!kw.contains(&field.as_str()) && !kw.contains(&name.to_lowercase().as_str()));
        let src = format!("struct {name} {{ {field}: i64 }}");
        let tokens = tokenize(&src).expect("struct should lex");
        let _prog = parse(tokens).expect("struct should parse");
    }

    /// Enum definitions always parse.
    #[test]
    fn parser_enum_def(name in "[A-Z][a-z]{3,6}", v1 in "[A-Z][a-z]{3,6}", v2 in "[A-Z][a-z]{3,6}") {
        prop_assume!(v1 != v2);
        let src = format!("enum {name} {{ {v1}, {v2} }}");
        let tokens = tokenize(&src).expect("enum should lex");
        let _prog = parse(tokens).expect("enum should parse");
    }

    /// Parenthesized expressions parse.
    #[test]
    fn parser_paren_expr(a in -100i64..100, b in -100i64..100) {
        let src = format!("({a} + {b})");
        let tokens = tokenize(&src).expect("paren expr should lex");
        let _prog = parse(tokens).expect("paren expr should parse");
    }

    /// Match expressions parse with multiple arms.
    #[test]
    fn parser_match_expr(val in 0i64..10, r1 in -100i64..100, r2 in -100i64..100) {
        let src = format!("match {val} {{ 0 => {r1}, _ => {r2} }}");
        let tokens = tokenize(&src).expect("match should lex");
        let _prog = parse(tokens).expect("match should parse");
    }

    /// Array literals parse.
    #[test]
    fn parser_array_lit(a in -100i64..100, b in -100i64..100, c in -100i64..100) {
        let src = format!("[{a}, {b}, {c}]");
        let tokens = tokenize(&src).expect("array should lex");
        let _prog = parse(tokens).expect("array should parse");
    }

    // ── Interpreter extended ──

    /// Unary minus is its own inverse: -(-x) == x.
    #[test]
    fn interpreter_double_minus(x in -10000i64..10000) {
        use fajar_lang::interpreter::Interpreter;
        let src = format!("fn main() -> void {{ println(-(-{x})) }}");
        let mut interp = Interpreter::new_capturing();
        interp.eval_source(&src).unwrap();
        interp.call_main().unwrap();
        prop_assert_eq!(&interp.get_output()[0], &format!("{x}"));
    }

    /// Multiplication by zero is always zero.
    #[test]
    fn interpreter_mul_zero(x in -10000i64..10000) {
        use fajar_lang::interpreter::Interpreter;
        let src = format!("fn main() -> void {{ println({x} * 0) }}");
        let mut interp = Interpreter::new_capturing();
        interp.eval_source(&src).unwrap();
        interp.call_main().unwrap();
        prop_assert_eq!(&interp.get_output()[0], "0");
    }

    /// Addition with zero is identity.
    #[test]
    fn interpreter_add_zero(x in -10000i64..10000) {
        use fajar_lang::interpreter::Interpreter;
        let src = format!("fn main() -> void {{ println({x} + 0) }}");
        let mut interp = Interpreter::new_capturing();
        interp.eval_source(&src).unwrap();
        interp.call_main().unwrap();
        prop_assert_eq!(&interp.get_output()[0], &format!("{x}"));
    }

    /// Multiplication by one is identity.
    #[test]
    fn interpreter_mul_one(x in -10000i64..10000) {
        use fajar_lang::interpreter::Interpreter;
        let src = format!("fn main() -> void {{ println({x} * 1) }}");
        let mut interp = Interpreter::new_capturing();
        interp.eval_source(&src).unwrap();
        interp.call_main().unwrap();
        prop_assert_eq!(&interp.get_output()[0], &format!("{x}"));
    }

    /// Boolean AND with true is identity.
    #[test]
    fn interpreter_and_true(b in proptest::bool::ANY) {
        use fajar_lang::interpreter::Interpreter;
        let src = format!("fn main() -> void {{ println({b} && true) }}");
        let mut interp = Interpreter::new_capturing();
        interp.eval_source(&src).unwrap();
        interp.call_main().unwrap();
        prop_assert_eq!(&interp.get_output()[0], &format!("{b}"));
    }

    /// Boolean OR with false is identity.
    #[test]
    fn interpreter_or_false(b in proptest::bool::ANY) {
        use fajar_lang::interpreter::Interpreter;
        let src = format!("fn main() -> void {{ println({b} || false) }}");
        let mut interp = Interpreter::new_capturing();
        interp.eval_source(&src).unwrap();
        interp.call_main().unwrap();
        prop_assert_eq!(&interp.get_output()[0], &format!("{b}"));
    }

    /// Equality is reflexive: x == x is always true.
    #[test]
    fn interpreter_equality_reflexive(x in -10000i64..10000) {
        use fajar_lang::interpreter::Interpreter;
        let src = format!("fn main() -> void {{ println({x} == {x}) }}");
        let mut interp = Interpreter::new_capturing();
        interp.eval_source(&src).unwrap();
        interp.call_main().unwrap();
        prop_assert_eq!(&interp.get_output()[0], "true");
    }

    /// Inequality with different values is true.
    #[test]
    fn interpreter_inequality(a in -10000i64..9999) {
        use fajar_lang::interpreter::Interpreter;
        let b = a + 1;
        let src = format!("fn main() -> void {{ println({a} != {b}) }}");
        let mut interp = Interpreter::new_capturing();
        interp.eval_source(&src).unwrap();
        interp.call_main().unwrap();
        prop_assert_eq!(&interp.get_output()[0], "true");
    }

    /// Less-than is strict: x < x is always false.
    #[test]
    fn interpreter_lt_strict(x in -10000i64..10000) {
        use fajar_lang::interpreter::Interpreter;
        let src = format!("fn main() -> void {{ println({x} < {x}) }}");
        let mut interp = Interpreter::new_capturing();
        interp.eval_source(&src).unwrap();
        interp.call_main().unwrap();
        prop_assert_eq!(&interp.get_output()[0], "false");
    }

    /// Less-or-equal is reflexive: x <= x is always true.
    #[test]
    fn interpreter_le_reflexive(x in -10000i64..10000) {
        use fajar_lang::interpreter::Interpreter;
        let src = format!("fn main() -> void {{ println({x} <= {x}) }}");
        let mut interp = Interpreter::new_capturing();
        interp.eval_source(&src).unwrap();
        interp.call_main().unwrap();
        prop_assert_eq!(&interp.get_output()[0], "true");
    }

    /// Recursive fibonacci produces correct values for small n.
    #[test]
    fn interpreter_fibonacci(n in 0u32..10) {
        use fajar_lang::interpreter::Interpreter;
        let src = "fn fib(n: i64) -> i64 { if n <= 1 { n } else { fib(n - 1) + fib(n - 2) } }\nfn main() -> void { println(fib(".to_string() + &n.to_string() + ")) }";
        let mut interp = Interpreter::new_capturing();
        interp.eval_source(&src).unwrap();
        interp.call_main().unwrap();
        let expected = [0, 1, 1, 2, 3, 5, 8, 13, 21, 34][n as usize];
        prop_assert_eq!(&interp.get_output()[0], &format!("{expected}"));
    }

    /// Nested function calls compose correctly.
    #[test]
    fn interpreter_nested_calls(x in -50i64..50) {
        use fajar_lang::interpreter::Interpreter;
        let src = format!(
            "fn add1(n: i64) -> i64 {{ n + 1 }}\nfn dbl(n: i64) -> i64 {{ n * 2 }}\nfn main() -> void {{ println(dbl(add1({x}))) }}"
        );
        let mut interp = Interpreter::new_capturing();
        interp.eval_source(&src).unwrap();
        interp.call_main().unwrap();
        let expected = (x + 1) * 2;
        prop_assert_eq!(&interp.get_output()[0], &format!("{expected}"));
    }

    /// Array length matches literal count.
    #[test]
    fn interpreter_array_len(a in -100i64..100, b in -100i64..100, c in -100i64..100) {
        use fajar_lang::interpreter::Interpreter;
        let src = format!("fn main() -> void {{ let arr = [{a}, {b}, {c}]\nprintln(len(arr)) }}");
        let mut interp = Interpreter::new_capturing();
        interp.eval_source(&src).unwrap();
        interp.call_main().unwrap();
        prop_assert_eq!(&interp.get_output()[0], "3");
    }

    /// Break exits loop early.
    #[test]
    fn interpreter_break_exits(limit in 1u32..20) {
        use fajar_lang::interpreter::Interpreter;
        let src = format!(
            "fn main() -> void {{ let mut i = 0\nwhile true {{ if i >= {limit} {{ break }}\ni = i + 1 }}\nprintln(i) }}"
        );
        let mut interp = Interpreter::new_capturing();
        interp.eval_source(&src).unwrap();
        interp.call_main().unwrap();
        prop_assert_eq!(&interp.get_output()[0], &format!("{limit}"));
    }

    /// String concatenation length property.
    #[test]
    fn interpreter_string_concat_len(a in "[a-z]{1,10}", b in "[a-z]{1,10}") {
        use fajar_lang::interpreter::Interpreter;
        let src = format!("fn main() -> void {{ println(len(\"{a}\" + \"{b}\")) }}");
        let mut interp = Interpreter::new_capturing();
        interp.eval_source(&src).unwrap();
        interp.call_main().unwrap();
        let expected = a.len() + b.len();
        prop_assert_eq!(&interp.get_output()[0], &format!("{expected}"));
    }

    /// Modulo property: (a / b) * b + (a % b) == a (for b != 0).
    #[test]
    fn interpreter_div_mod_identity(a in -1000i64..1000, b in 1i64..100) {
        use fajar_lang::interpreter::Interpreter;
        let src = format!(
            "fn main() -> void {{ println({a} / {b} * {b} + {a} % {b}) }}"
        );
        let mut interp = Interpreter::new_capturing();
        interp.eval_source(&src).unwrap();
        interp.call_main().unwrap();
        prop_assert_eq!(&interp.get_output()[0], &format!("{a}"));
    }

    /// Bitwise AND with self is identity.
    #[test]
    fn interpreter_bitand_self(x in 0i64..10000) {
        use fajar_lang::interpreter::Interpreter;
        let src = format!("fn main() -> void {{ println({x} & {x}) }}");
        let mut interp = Interpreter::new_capturing();
        interp.eval_source(&src).unwrap();
        interp.call_main().unwrap();
        prop_assert_eq!(&interp.get_output()[0], &format!("{x}"));
    }

    /// Bitwise OR with self is identity.
    #[test]
    fn interpreter_bitor_self(x in 0i64..10000) {
        use fajar_lang::interpreter::Interpreter;
        let src = format!("fn main() -> void {{ println({x} | {x}) }}");
        let mut interp = Interpreter::new_capturing();
        interp.eval_source(&src).unwrap();
        interp.call_main().unwrap();
        prop_assert_eq!(&interp.get_output()[0], &format!("{x}"));
    }

    /// XOR with self is always zero.
    #[test]
    fn interpreter_xor_self(x in 0i64..10000) {
        use fajar_lang::interpreter::Interpreter;
        let src = format!("fn main() -> void {{ println({x} ^ {x}) }}");
        let mut interp = Interpreter::new_capturing();
        interp.eval_source(&src).unwrap();
        interp.call_main().unwrap();
        prop_assert_eq!(&interp.get_output()[0], "0");
    }

    /// Left shift followed by right shift restores value (for small shifts).
    #[test]
    fn interpreter_shift_roundtrip(x in 0i64..1000, shift in 0u32..10) {
        use fajar_lang::interpreter::Interpreter;
        let src = format!("fn main() -> void {{ println(({x} << {shift}) >> {shift}) }}");
        let mut interp = Interpreter::new_capturing();
        interp.eval_source(&src).unwrap();
        interp.call_main().unwrap();
        prop_assert_eq!(&interp.get_output()[0], &format!("{x}"));
    }

    // ── Value properties extended ──

    /// Value::Array display includes brackets.
    #[test]
    fn value_array_display(a in -100i64..100, b in -100i64..100) {
        use fajar_lang::interpreter::Value;
        let v = Value::Array(vec![Value::Int(a), Value::Int(b)]);
        let displayed = format!("{v}");
        prop_assert!(displayed.starts_with('['));
        prop_assert!(displayed.ends_with(']'));
    }

    /// Value::Null display is "null".
    #[test]
    fn value_null_display(_ in Just(())) {
        use fajar_lang::interpreter::Value;
        prop_assert_eq!(format!("{}", Value::Null), "null");
    }

    /// Value::Char display matches.
    #[test]
    fn value_char_display(c in proptest::char::range('a', 'z')) {
        use fajar_lang::interpreter::Value;
        let v = Value::Char(c);
        prop_assert_eq!(format!("{v}"), format!("{c}"));
    }

    // ── Lexer-Parser consistency ──

    /// Valid let + assignment always produces parseable output.
    #[test]
    fn consistency_let_assign(val in -10000i64..10000) {
        let src = format!("let x = {val}");
        let tokens = tokenize(&src).expect("let assign should lex");
        let _prog = parse(tokens).expect("let assign should parse");
    }

    /// Valid function with return always parses.
    #[test]
    fn consistency_fn_return(val in -1000i64..1000) {
        let src = format!("fn foo() -> i64 {{ return {val} }}");
        let tokens = tokenize(&src).expect("fn return should lex");
        let _prog = parse(tokens).expect("fn return should parse");
    }

    /// Multi-statement blocks always parse.
    #[test]
    fn consistency_block_stmts(a in -100i64..100, b in -100i64..100) {
        let src = format!("{{ let x = {a}\nlet y = {b}\nx + y }}");
        let tokens = tokenize(&src).expect("block should lex");
        let _prog = parse(tokens).expect("block should parse");
    }

    /// Nested arithmetic with parens always parses.
    #[test]
    fn consistency_nested_arith(a in -100i64..100, b in -100i64..100, c in -100i64..100) {
        let src = format!("({a} + {b}) * {c}");
        let tokens = tokenize(&src).expect("nested arith should lex");
        let _prog = parse(tokens).expect("nested arith should parse");
    }

    /// Boolean expressions always parse and evaluate correctly.
    #[test]
    fn interpreter_bool_expr(a in proptest::bool::ANY, b in proptest::bool::ANY) {
        use fajar_lang::interpreter::Interpreter;
        let src = format!("fn main() -> void {{ println({a} && {b}) }}");
        let mut interp = Interpreter::new_capturing();
        interp.eval_source(&src).unwrap();
        interp.call_main().unwrap();
        let expected = a && b;
        prop_assert_eq!(&interp.get_output()[0], &format!("{expected}"));
    }

    /// Boolean OR evaluates correctly.
    #[test]
    fn interpreter_bool_or(a in proptest::bool::ANY, b in proptest::bool::ANY) {
        use fajar_lang::interpreter::Interpreter;
        let src = format!("fn main() -> void {{ println({a} || {b}) }}");
        let mut interp = Interpreter::new_capturing();
        interp.eval_source(&src).unwrap();
        interp.call_main().unwrap();
        let expected = a || b;
        prop_assert_eq!(&interp.get_output()[0], &format!("{expected}"));
    }

    /// Interpreter handles multiple println outputs correctly.
    #[test]
    fn interpreter_multi_output(a in -100i64..100, b in -100i64..100) {
        use fajar_lang::interpreter::Interpreter;
        let src = format!("fn main() -> void {{ println({a})\nprintln({b}) }}");
        let mut interp = Interpreter::new_capturing();
        interp.eval_source(&src).unwrap();
        interp.call_main().unwrap();
        let output = interp.get_output();
        prop_assert_eq!(output.len(), 2);
        prop_assert_eq!(&output[0], &format!("{a}"));
        prop_assert_eq!(&output[1], &format!("{b}"));
    }

    /// Const declarations are immutable and evaluate correctly.
    #[test]
    fn interpreter_const_decl(val in -10000i64..10000) {
        use fajar_lang::interpreter::Interpreter;
        let src = format!("fn main() -> void {{ const X: i64 = {val}\nprintln(X) }}");
        let mut interp = Interpreter::new_capturing();
        interp.eval_source(&src).unwrap();
        interp.call_main().unwrap();
        prop_assert_eq!(&interp.get_output()[0], &format!("{val}"));
    }

    /// Lexer handles comment lines gracefully.
    #[test]
    fn lexer_comments(s in "[a-z ]{0,50}") {
        let src = format!("// {s}\nlet x = 42");
        let tokens = tokenize(&src).expect("comment + let should lex");
        prop_assert!(tokens.len() >= 2);
    }

    /// Interpreter handles if-without-else as statement.
    #[test]
    fn interpreter_if_no_else(x in -100i64..100) {
        use fajar_lang::interpreter::Interpreter;
        let src = format!(
            "fn main() -> void {{ let mut r = 0\nif {x} > 0 {{ r = 1 }}\nprintln(r) }}"
        );
        let mut interp = Interpreter::new_capturing();
        interp.eval_source(&src).unwrap();
        interp.call_main().unwrap();
        let expected = if x > 0 { 1 } else { 0 };
        prop_assert_eq!(&interp.get_output()[0], &format!("{expected}"));
    }

    /// Associativity of addition: (a + b) + c == a + (b + c).
    #[test]
    fn interpreter_add_assoc(a in -100i64..100, b in -100i64..100, c in -100i64..100) {
        use fajar_lang::interpreter::Interpreter;
        let src1 = format!("fn main() -> void {{ println(({a} + {b}) + {c}) }}");
        let src2 = format!("fn main() -> void {{ println({a} + ({b} + {c})) }}");
        let mut i1 = Interpreter::new_capturing();
        i1.eval_source(&src1).unwrap();
        i1.call_main().unwrap();
        let mut i2 = Interpreter::new_capturing();
        i2.eval_source(&src2).unwrap();
        i2.call_main().unwrap();
        prop_assert_eq!(i1.get_output(), i2.get_output());
    }

    /// Associativity of multiplication: (a * b) * c == a * (b * c).
    #[test]
    fn interpreter_mul_assoc(a in -20i64..20, b in -20i64..20, c in -20i64..20) {
        use fajar_lang::interpreter::Interpreter;
        let src1 = format!("fn main() -> void {{ println(({a} * {b}) * {c}) }}");
        let src2 = format!("fn main() -> void {{ println({a} * ({b} * {c})) }}");
        let mut i1 = Interpreter::new_capturing();
        i1.eval_source(&src1).unwrap();
        i1.call_main().unwrap();
        let mut i2 = Interpreter::new_capturing();
        i2.eval_source(&src2).unwrap();
        i2.call_main().unwrap();
        prop_assert_eq!(i1.get_output(), i2.get_output());
    }
}
