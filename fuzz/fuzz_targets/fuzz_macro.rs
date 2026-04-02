#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Fuzz macro expansion: test format!, println!, assert_eq! etc.
    // Must never panic or infinite-loop on any input.
    if let Ok(raw) = std::str::from_utf8(data) {
        if raw.len() > 2048 {
            return;
        }
        let mut interp = fajar_lang::interpreter::Interpreter::new();

        // Test 1: raw source (catches any macro-related parse/eval bugs)
        let _ = interp.eval_source(raw);

        // Test 2: wrap in f-string expression (exercises interpolation expansion)
        let escaped = raw.replace('\\', "\\\\").replace('"', "\\\"");
        let fmt_src = format!("let _r = f\"{{{escaped}}}\"");
        let mut interp2 = fajar_lang::interpreter::Interpreter::new();
        let _ = interp2.eval_source(&fmt_src);
    }
});
