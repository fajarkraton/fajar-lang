#![no_main]
use libfuzzer_sys::fuzz_target;

// Note: Run with ASAN_OPTIONS="detect_leaks=0" because the interpreter
// uses Rc<RefCell<>> for environments which creates expected reference cycles.
// This is a known design choice (CLAUDE.md decision #6), not a real leak.

fuzz_target!(|data: &[u8]| {
    // Fuzz the full pipeline: source → lex → parse → analyze → eval
    // The interpreter must never panic/UB — it should return Ok or Err
    if let Ok(source) = std::str::from_utf8(data) {
        // Limit input size to prevent OOM from huge inputs
        if source.len() > 4096 {
            return;
        }
        let mut interp = fajar_lang::interpreter::Interpreter::new();
        // eval_source runs: lex → parse → analyze → eval
        // Any result (Ok or Err) is acceptable — no panics allowed
        let _ = interp.eval_source(source);
    }
});
