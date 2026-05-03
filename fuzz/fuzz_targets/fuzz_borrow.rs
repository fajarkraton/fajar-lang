#![no_main]
use libfuzzer_sys::fuzz_target;

// Fuzz the polonius borrow checker. Random source → lex → parse →
// FactGenerator::generate → PoloniusSolver::solve. Both fact generation
// and the solver must never panic. The solver is constrained to a
// modest iteration cap so adversarial CFGs cannot stall the fuzzer.
fuzz_target!(|data: &[u8]| {
    if let Ok(source) = std::str::from_utf8(data) {
        if let Ok(tokens) = fajar_lang::lexer::tokenize(source) {
            if let Ok(program) = fajar_lang::parser::parse(tokens) {
                let facts = fajar_lang::analyzer::polonius::FactGenerator::new().generate(&program);
                let _ = fajar_lang::analyzer::polonius::PoloniusSolver::with_max_iterations(200)
                    .solve(&facts);
            }
        }
    }
});
