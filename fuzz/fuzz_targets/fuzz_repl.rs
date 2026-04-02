#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Fuzz REPL mode: sequential eval_source calls on a shared interpreter.
    // Tests cross-statement state (variables, functions persisting).
    // Must never panic — eval errors are expected and acceptable.
    if let Ok(source) = std::str::from_utf8(data) {
        if source.len() > 2048 {
            return;
        }
        let mut interp = fajar_lang::interpreter::Interpreter::new();
        // Simulate REPL: split on newlines and eval each line separately
        for line in source.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let _ = interp.eval_source(trimmed);
        }
    }
});
