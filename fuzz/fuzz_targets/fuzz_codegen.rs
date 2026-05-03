#![no_main]
use libfuzzer_sys::fuzz_target;

// Fuzz the bytecode VM compiler (always-available codegen path). Random
// source → lex → parse → analyze → vm::Compiler::compile. The compiler
// must never panic on a parsed+analyzed program, regardless of input.
fuzz_target!(|data: &[u8]| {
    if let Ok(source) = std::str::from_utf8(data) {
        if let Ok(tokens) = fajar_lang::lexer::tokenize(source) {
            if let Ok(program) = fajar_lang::parser::parse(tokens) {
                // Run analyzer first so the program is type-checked before
                // codegen sees it (matches the production pipeline order).
                let _ = fajar_lang::analyzer::analyze(&program);
                // Compile to bytecode chunk; result is unused — we only
                // care that compile() does NOT panic.
                let _ = fajar_lang::vm::compiler::Compiler::new().compile(&program);
            }
        }
    }
});
