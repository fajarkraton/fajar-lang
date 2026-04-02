#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Fuzz effect system: wrap input in effect/handle constructs.
    // Tests effect declaration, handler dispatch, resume continuations.
    // Must never panic on any input.
    if let Ok(raw) = std::str::from_utf8(data) {
        if raw.len() > 2048 {
            return;
        }
        let mut interp = fajar_lang::interpreter::Interpreter::new();

        // Test 1: raw source through full pipeline (catches effect keyword issues)
        let _ = interp.eval_source(raw);

        // Test 2: wrap in effect declaration pattern
        let escaped = raw.replace('\\', "\\\\").replace('"', "\\\"");
        let effect_src = format!(
            "effect Fuzz {{ fn op(x: str) -> str }}\n\
             handle {{ let r = Fuzz::op(\"{escaped}\"); r }} with {{ Fuzz::op(v) => {{ resume(v) }} }}"
        );
        let mut interp2 = fajar_lang::interpreter::Interpreter::new();
        let _ = interp2.eval_source(&effect_src);
    }
});
