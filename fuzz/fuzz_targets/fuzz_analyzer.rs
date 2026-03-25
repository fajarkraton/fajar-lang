#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Fuzz the semantic analyzer: random source → lex → parse → analyze
    // The analyzer must never panic on any syntactically valid input
    if let Ok(source) = std::str::from_utf8(data) {
        if let Ok(tokens) = fajar_lang::lexer::tokenize(source) {
            if let Ok(program) = fajar_lang::parser::parse(tokens) {
                // analyze() returns Ok(()) or Err(Vec<SemanticError>)
                // Either is fine — it must NOT panic
                let _ = fajar_lang::analyzer::analyze(&program);
            }
        }
    }
});
