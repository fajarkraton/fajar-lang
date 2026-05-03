#![no_main]
use libfuzzer_sys::fuzz_target;

// Fuzz the async/effect machinery. The input is wrapped inside `async fn _f()`
// so any `.await` / effect-handler construct in the body is exercised in a
// proper async context. Drives lex → parse → analyze; asserts no panic.
fuzz_target!(|data: &[u8]| {
    if let Ok(body) = std::str::from_utf8(data) {
        let source = format!("async fn _fuzz_async_target() {{ {body} }}");
        if let Ok(tokens) = fajar_lang::lexer::tokenize(&source) {
            if let Ok(program) = fajar_lang::parser::parse(tokens) {
                let _ = fajar_lang::analyzer::analyze(&program);
            }
        }
    }
});
