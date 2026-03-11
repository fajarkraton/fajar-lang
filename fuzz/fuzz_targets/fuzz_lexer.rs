#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Only fuzz valid UTF-8 input — Fajar Lang sources are always text
    if let Ok(source) = std::str::from_utf8(data) {
        // The lexer must never panic on any input
        let _ = fajar_lang::lexer::tokenize(source);
    }
});
