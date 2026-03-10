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
