# Release Notes — Fajar Lang v7.0.0 "Integrity"

> **Date:** 2026-03-28
> **Codename:** Integrity — every claim backed by verified code

---

## Highlights

- **Full production audit:** 342 files, 335,657 LOC, 5,563 tests — all verified
- **FajarOS Nova kernel passes `fj check` clean:** 21,187 lines, 0 errors
- **Type checker improvements:** 214 kernel errors eliminated
- **14 new interpreter builtins:** WebSocket, MQTT, BLE
- **Security hardening in compiled output:** bounds checks, overflow detection
- **Browser playground WASM bridge:** eval/tokenize/format/check exports
- **Benchmark suite:** 10 cross-language benchmark programs
- **FajarOS Nova CI:** automated QEMU boot testing

## Statistics

| Metric | v6.1.0 | v7.0.0 | Delta |
|--------|--------|--------|-------|
| Tests | 5,483 | 5,563 | +80 |
| Source LOC | 334,821 | 335,657 | +836 |
| Source files | 342 | 343 | +1 |
| Example .fj | 163 | 173 | +10 |
| Clippy warnings | 0 | 0 | — |
| Doc warnings | 12 | 0 | -12 |
| Kernel errors | 214 | 0 | -214 |

## Changes by Phase

### Phase 1: Type Checker (214 → 0 kernel errors)
- Parser: control flow expressions no longer consumed by Pratt infix ops
- Kernel: 16 bitwise precedence fixes with explicit parentheses
- Type checker: str_len/str_byte_at accept both str and i64
- Type checker: void branches allowed in if/else statements

### Phase 2: Interpreter Builtins (+14 builtins)
- WebSocket: ws_connect, ws_send, ws_recv, ws_close
- MQTT: mqtt_connect, mqtt_publish, mqtt_subscribe, mqtt_recv, mqtt_disconnect
- BLE: ble_scan, ble_connect, ble_read, ble_write, ble_disconnect

### Phase 3: Compiler Pipeline
- Array bounds checks emitted in Cranelift when `--security`
- Integer overflow checks (checked_add/sub/mul) when `--security`
- AST optimization pipeline (O2/Os) runs on every `fj build`

### Phase 4: External Integration
- winit 0.30 + softbuffer 0.4 for real OS windowing (`--features gui`)
- CI matrix for feature-gated code (smt, cpp-ffi, python-ffi)

### Phase 5: Playground WASM
- wasm-bindgen entry points: eval_source, tokenize_source, format_source, check_source
- build-playground.sh for wasm-pack builds
- Integrated into docs.yml deployment

### Phase 6: Benchmark Suite
- 10 benchmark .fj programs (fibonacci, quicksort, mandelbrot, etc.)
- Automated runner with warmup, timing, min/max/avg

### Phase 7: FajarOS Nova Production
- QEMU CI pipeline (check-kernel + boot test + analysis)
- Makefile for build/run/test
- Hardware verification plan (17 x86 + 7 ARM tests)

### Phase 8: Self-Hosting
- 3,076 lines of self-hosted compiler in Fajar Lang
- Bootstrap test: PASSED

### Phase 10: Documentation
- 0 cargo doc warnings (was 12)
- 156 of 173 examples pass `fj check`
- Release notes (this document)

## Known Limitations

- 17 example .fj files have `fj check` errors (ARM64/extension builtins not registered)
- Playground WASM requires `wasm-pack` build (not pre-compiled)
- GUI windowing requires `--features gui` (optional)
- Feature-gated code (Z3, libclang, pyo3) requires system libraries

## Upgrade from v6.1.0

No breaking changes. All existing .fj programs continue to work.
New CLI flags: `--security`, `--lint`, `--profile`, `--profile-output`.
