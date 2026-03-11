#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Only fuzz valid UTF-8 input
    if let Ok(source) = std::str::from_utf8(data) {
        // Lex first, then parse — neither should panic on any input
        if let Ok(tokens) = fajar_lang::lexer::tokenize(source) {
            let _ = fajar_lang::parser::parse(tokens);
        }
    }
});
