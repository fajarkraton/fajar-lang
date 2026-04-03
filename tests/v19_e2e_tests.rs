//! V19 "Precision" end-to-end tests.
//!
//! Each test exercises a complete feature via eval_source → captured output.
//! Covers: macros, pattern match, generators, @requires, channels, async.

/// Evaluate source code and capture all printed output.
fn eval_capture(source: &str) -> String {
    let tokens = fajar_lang::lexer::tokenize(source).expect("lex failed");
    let program = fajar_lang::parser::parse(tokens).expect("parse failed");
    let _ = fajar_lang::analyzer::analyze(&program); // ignore warnings
    let mut interp = fajar_lang::interpreter::Interpreter::new_capturing();
    interp.eval_program(&program).expect("eval failed");
    interp.get_output().join("\n")
}

// ════════════════════════════════════════════════════════════════════════
// Macros (V19 Phase 1)
// ════════════════════════════════════════════════════════════════════════

#[test]
fn v19_macro_double() {
    let out = eval_capture(
        "macro_rules! double { ($x:expr) => { $x * 2 } }
         println(double!(21))",
    );
    assert_eq!(out.trim(), "42");
}

#[test]
fn v19_macro_multi_arg() {
    let out = eval_capture(
        "macro_rules! add { ($a:expr, $b:expr) => { $a + $b } }
         println(add!(100, 200))",
    );
    assert_eq!(out.trim(), "300");
}

#[test]
fn v19_macro_nested() {
    let out = eval_capture(
        "macro_rules! double { ($x:expr) => { $x * 2 } }
         macro_rules! inc { ($x:expr) => { $x + 1 } }
         println(double!(inc!(4)))",
    );
    assert_eq!(out.trim(), "10");
}

#[test]
fn v19_macro_with_if() {
    let out = eval_capture(
        "macro_rules! max { ($a:expr, $b:expr) => { if $a > $b { $a } else { $b } } }
         println(max!(3, 7))
         println(max!(10, 5))",
    );
    assert_eq!(out.trim(), "7\n10");
}

// ════════════════════════════════════════════════════════════════════════
// Pattern match destructuring (V19 Phase 2)
// ════════════════════════════════════════════════════════════════════════

#[test]
fn v19_pattern_ok() {
    let out = eval_capture("match Ok(42) { Ok(v) => println(v), Err(e) => println(e) }");
    assert_eq!(out.trim(), "42");
}

#[test]
fn v19_pattern_err() {
    let out = eval_capture(r#"match Err("fail") { Ok(v) => println(v), Err(e) => println(e) }"#);
    assert_eq!(out.trim(), "fail");
}

#[test]
fn v19_pattern_some_none() {
    let out = eval_capture(
        r#"match Some(99) { Some(x) => println(x), None => println("none") }
         match None { Some(x) => println(x), None => println("none") }"#,
    );
    assert_eq!(out.trim(), "99\nnone");
}

// ════════════════════════════════════════════════════════════════════════
// Generators (V18, verified in V19)
// ════════════════════════════════════════════════════════════════════════

#[test]
fn v19_generator_yield() {
    let out = eval_capture(
        "gen fn countdown() {
            yield 3
            yield 2
            yield 1
        }
        for x in countdown() { println(x) }",
    );
    assert_eq!(out.trim(), "3\n2\n1");
}

#[test]
fn v19_generator_range() {
    let out = eval_capture(
        "gen fn range(n: i64) -> i64 {
            let mut i = 0
            while i < n { yield i\n i = i + 1 }
        }
        let items = range(4)
        println(items)",
    );
    assert!(out.contains("[0, 1, 2, 3]"), "got: {out}");
}

// ════════════════════════════════════════════════════════════════════════
// @requires preconditions (V18, verified in V19)
// ════════════════════════════════════════════════════════════════════════

#[test]
fn v19_requires_passes() {
    let out = eval_capture(
        "fn positive(n: i64) -> i64 @requires(n > 0) { n }
         println(positive(5))",
    );
    assert_eq!(out.trim(), "5");
}

#[test]
fn v19_requires_fails() {
    let source = "fn positive(n: i64) -> i64 @requires(n > 0) { n }
         positive(-1)";
    let tokens = fajar_lang::lexer::tokenize(source).expect("lex failed");
    let program = fajar_lang::parser::parse(tokens).expect("parse failed");
    let _ = fajar_lang::analyzer::analyze(&program);
    let mut interp = fajar_lang::interpreter::Interpreter::new_capturing();
    let result = interp.eval_program(&program);
    assert!(result.is_err(), "@requires should reject -1");
}

// ════════════════════════════════════════════════════════════════════════
// Channels (V18, verified in V19)
// ════════════════════════════════════════════════════════════════════════

#[test]
fn v19_channel_send_recv() {
    let out = eval_capture(
        r#"let ch = channel_create()
         channel_send(ch, "hello")
         let msg = channel_recv(ch)
         match msg { Some(v) => println(v), None => println("none") }"#,
    );
    assert_eq!(out.trim(), "hello");
}

#[test]
fn v19_channel_multiple_messages() {
    let out = eval_capture(
        "let ch = channel_create()
         channel_send(ch, 1)
         channel_send(ch, 2)
         channel_send(ch, 3)
         match channel_recv(ch) { Some(v) => println(v), None => {} }
         match channel_recv(ch) { Some(v) => println(v), None => {} }
         match channel_recv(ch) { Some(v) => println(v), None => {} }",
    );
    assert_eq!(out.trim(), "1\n2\n3");
}

// ════════════════════════════════════════════════════════════════════════
// Async I/O (V19 Phase 3)
// ════════════════════════════════════════════════════════════════════════

#[test]
fn v19_async_sleep() {
    let out = eval_capture(
        r#"async_sleep(30)
         println("awake")"#,
    );
    assert_eq!(out.trim(), "awake");
}

#[test]
fn v19_async_spawn_join() {
    let out = eval_capture(
        "fn work() -> i64 { 99 }
         let t = async_spawn(\"work\")
         println(async_join(t))",
    );
    assert_eq!(out.trim(), "99");
}
