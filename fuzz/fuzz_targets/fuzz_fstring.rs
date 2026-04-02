#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Fuzz f-string interpolation: wrap input in f"..." and lex/parse/eval.
    // Must never panic on any input — even malformed { } nesting.
    if let Ok(raw) = std::str::from_utf8(data) {
        if raw.len() > 2048 {
            return;
        }
        // Test raw string through lexer (catches f-string tokenization bugs)
        let source = format!("let x = f\"{}\"", raw);
        let _ = fajar_lang::lexer::tokenize(&source);

        // Also test through full pipeline
        let mut interp = fajar_lang::interpreter::Interpreter::new();
        let _ = interp.eval_source(&source);
    }
});
