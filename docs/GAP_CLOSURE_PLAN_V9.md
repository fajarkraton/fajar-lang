# GAP_CLOSURE_PLAN_V9.md — Make Every Gap 100% Real

> **Date:** 2026-03-28
> **Author:** Claude Opus 4.6 + Fajar
> **Purpose:** Close ALL remaining gaps found in V8 Options 0-10 audit
> **Rule:** No shortcuts, no workarounds, no "framework-only" — every task must be end-to-end functional

---

## Executive Summary

Full production audit of V8 (810 tasks across Options 0-10) revealed:

| Category | Count | Description |
|----------|-------|-------------|
| Fully Real | ~610 | Working end-to-end, tested |
| Feature-Gated Real | ~40 | Code works but requires `--features` + external deps |
| Partial | ~90 | Logic exists but not wired into pipeline |
| Stub/Missing | ~70 | Empty passes, missing builtins, no integration |

This plan closes every gap. **9 Phases, 120 tasks, estimated 8,000-12,000 LOC.**

---

## Phase 1: Compiler Pipeline Wiring (20 tasks)

**Goal:** Wire existing security, profiler, and optimizer modules into the actual Cranelift codegen pipeline so they produce real effects in compiled output.

### P1.1: Security → Cranelift Integration

These modules exist in `src/codegen/security.rs` (2,919 LOC, 72 tests) but are NOT called from `src/codegen/cranelift/mod.rs`. Wire them in.

| Task | Description | File(s) | Verify |
|------|-------------|---------|--------|
| P1.1.1 | Import `security.rs` types in `cranelift/mod.rs` | `src/codegen/cranelift/mod.rs` | Compiles |
| P1.1.2 | Add `SecurityConfig` field to `CraneliftCompiler` | `src/codegen/cranelift/mod.rs`, `context.rs` | Field accessible |
| P1.1.3 | Inject stack canary prologue/epilogue in `compile_function()` | `cranelift/compile/mod.rs` | Generated code includes canary check |
| P1.1.4 | Inject bounds checks on array/slice access | `cranelift/compile/arrays.rs` | Out-of-bounds trapped |
| P1.1.5 | Inject integer overflow checks (debug mode) | `cranelift/compile/expr.rs` | Overflow trapped in debug |
| P1.1.6 | Wire `AllocationBudget` into heap allocation calls | `cranelift/runtime_fns.rs` | Budget exceeded → error |
| P1.1.7 | Wire `SecurityLinter` as pre-compilation pass | `cranelift/mod.rs` | Lint warnings emitted before codegen |
| P1.1.8 | Wire `TaintAnalysis` on function parameters from FFI | `cranelift/compile/mod.rs` | FFI inputs marked tainted |
| P1.1.9 | Tests: compile .fj with canary, trigger overflow, verify trap | `cranelift/tests.rs` | 5 new tests |
| P1.1.10 | Benchmark: measure overhead of security checks | `benches/security_bench.rs` | < 10% overhead |

### P1.2: Profiler → Compiler Integration

`src/profiler/instrument.rs` (1,336 LOC, 28 tests) has `enter_fn()`/`exit_fn()` but they're never called automatically.

| Task | Description | File(s) | Verify |
|------|-------------|---------|--------|
| P1.2.1 | Add `--profile` flag to `fj run` and `fj build` | `src/main.rs` | Flag parsed |
| P1.2.2 | In interpreter: inject `ProfileSession::enter_fn()` at function call entry | `src/interpreter/eval/mod.rs` | Profiler records function entries |
| P1.2.3 | In interpreter: inject `ProfileSession::exit_fn()` at function return | `src/interpreter/eval/mod.rs` | Profiler records function exits |
| P1.2.4 | In Cranelift: emit `fj_rt_profile_enter`/`fj_rt_profile_exit` calls | `cranelift/compile/mod.rs` | Native code emits profile events |
| P1.2.5 | Add `fj_rt_profile_enter`/`fj_rt_profile_exit` to `runtime_fns.rs` | `cranelift/runtime_fns.rs` | Runtime functions exist |
| P1.2.6 | Output profile to `fj-profile.json` (Chrome trace format) | `src/main.rs` | File written on exit |
| P1.2.7 | Tests: profile a .fj program, verify JSON output | `tests/` | 3 new tests |

### P1.3: Optimizer Stub Passes → Real Implementations

5 passes in `src/codegen/opt_passes.rs` line 1627 return 0. These need real AST-level implementations.

| Task | Description | File(s) | Verify |
|------|-------------|---------|--------|
| P1.3.1 | **LICM** — Loop-Invariant Code Motion: identify expressions in loop bodies that don't depend on loop variables, count them for hoisting | `opt_passes.rs` | Returns non-zero for loops with invariant exprs |
| P1.3.2 | **CSE** — Common Subexpression Elimination: find identical sub-expressions in same scope, count elimination opportunities | `opt_passes.rs` | Returns count of duplicate exprs |
| P1.3.3 | **Devirtualize** — Static dispatch: identify trait method calls where concrete type is known, count opportunities | `opt_passes.rs` | Returns count of resolvable calls |

> **Note on Vectorize and Function Merge:** These passes genuinely require IR-level representation (SIMD instruction selection, binary-identical function body comparison). At the AST level, we can implement **analysis** that counts opportunities but cannot transform code. The honest approach:

| Task | Description | File(s) | Verify |
|------|-------------|---------|--------|
| P1.3.4 | **Vectorize analysis** — Identify loops with parallel array operations that could be SIMD-ized, count opportunities | `opt_passes.rs` | Returns count of vectorizable loops |
| P1.3.5 | **Function merge analysis** — Find functions with identical AST structure (after alpha-renaming), count merge candidates | `opt_passes.rs` | Returns count of mergeable fn pairs |
| P1.3.6 | Tests for all 5 passes with sample programs | `opt_passes.rs` tests | 5+ new tests |

---

## Phase 2: Interpreter Builtins (15 tasks)

**Goal:** Implement missing builtins that .fj templates reference but don't exist.

### P2.1: WebSocket Builtins

Templates reference `ws_accept`, `ws_send`, `ws_close` but they don't exist in the interpreter.

| Task | Description | File(s) | Verify |
|------|-------------|---------|--------|
| P2.1.1 | Implement `ws_accept(socket)` — upgrade HTTP to WebSocket (simulated handshake) | `src/interpreter/eval/builtins.rs` | Returns WebSocket handle |
| P2.1.2 | Implement `ws_send(ws, message)` — send text frame | `src/interpreter/eval/builtins.rs` | Returns bytes sent |
| P2.1.3 | Implement `ws_recv(ws)` — receive text frame | `src/interpreter/eval/builtins.rs` | Returns message string |
| P2.1.4 | Implement `ws_close(ws)` — close WebSocket connection | `src/interpreter/eval/builtins.rs` | Returns success |
| P2.1.5 | Register all ws_* builtins in `builtin_fn_names()` and `eval_builtin_call()` | `src/interpreter/eval/builtins.rs` | Builtins callable from .fj |
| P2.1.6 | Tests: WebSocket echo roundtrip in .fj | `tests/eval_tests.rs` | 3 tests |

### P2.2: MQTT Builtins (simulated in-memory broker)

| Task | Description | File(s) | Verify |
|------|-------------|---------|--------|
| P2.2.1 | Implement `mqtt_connect(broker_addr)` → client handle | `src/interpreter/eval/builtins.rs` | Returns MQTT client |
| P2.2.2 | Implement `mqtt_publish(client, topic, payload, qos)` | `src/interpreter/eval/builtins.rs` | Message queued |
| P2.2.3 | Implement `mqtt_subscribe(client, topic)` | `src/interpreter/eval/builtins.rs` | Subscription registered |
| P2.2.4 | Implement `mqtt_receive(client)` → (topic, payload) | `src/interpreter/eval/builtins.rs` | Returns next message |
| P2.2.5 | Implement `mqtt_disconnect(client)` | `src/interpreter/eval/builtins.rs` | Connection closed |
| P2.2.6 | In-memory `MqttBroker` struct for simulation | `src/stdlib_v3/net.rs` or new file | Broker routes messages |
| P2.2.7 | Register all mqtt_* builtins | `src/interpreter/eval/builtins.rs` | Callable from .fj |
| P2.2.8 | Tests: publish-subscribe roundtrip | `tests/eval_tests.rs` | 3 tests |

### P2.3: Type Checker Registration

| Task | Description | File(s) | Verify |
|------|-------------|---------|--------|
| P2.3.1 | Register ws_*/mqtt_* signatures in `type_check.rs` builtin table | `src/analyzer/type_check.rs` | Analyzer accepts ws_/mqtt_ calls |

---

## Phase 3: Playground WASM (15 tasks)

**Goal:** Build a real, working browser-based playground that compiles and runs .fj code.

| Task | Description | File(s) | Verify |
|------|-------------|---------|--------|
| P3.1 | Add `wasm-bindgen = "0.2"` to Cargo.toml under `[target.'cfg(target_arch = "wasm32")'.dependencies]` | `Cargo.toml` | Dependency resolves |
| P3.2 | Add `wasm-pack` build script (`Makefile.playground` or `build-playground.sh`) | root | Script exists |
| P3.3 | Create `src/playground/wasm_api.rs` with `#[wasm_bindgen]` exports: `eval_source(code: &str) -> String`, `tokenize(code: &str) -> String`, `parse(code: &str) -> String`, `format(code: &str) -> String` | `src/playground/wasm_api.rs` | Compiles to wasm32 |
| P3.4 | Ensure `tokenize()` + `parse()` + `analyze()` + interpreter work in wasm32 target (no filesystem, no network) | `src/playground/wasm_api.rs` | No panics on eval |
| P3.5 | Build `playground/pkg/` with wasm-pack | `playground/pkg/` | .wasm + .js glue exist |
| P3.6 | Update `playground/src/executor.js` to import from `../pkg/` | `playground/src/executor.js` | JS imports wasm |
| P3.7 | Update `playground/src/worker.js` to use wasm eval | `playground/src/worker.js` | Worker runs .fj code |
| P3.8 | Test: playground runs `let x = 42; println(x)` in browser | manual | Output shows "42" |
| P3.9 | Test: playground shows parse errors with spans | manual | Error display works |
| P3.10 | Test: playground formats code | manual | Formatted output shown |
| P3.11 | Add wasm32 target to CI workflow | `.github/workflows/ci.yml` | CI builds wasm |
| P3.12 | Add playground build to docs.yml deploy | `.github/workflows/docs.yml` | Playground deployed |
| P3.13 | Update book tutorials "try in playground" links | `book/src/tutorials/` | Links work |
| P3.14 | Test: 5 example programs run in playground | manual | All produce correct output |
| P3.15 | Document playground architecture in `playground/README.md` | `playground/README.md` | Architecture documented |

---

## Phase 4: LSP Improvements (5 tasks)

**Goal:** Make goto-definition scope-aware instead of text-search.

| Task | Description | File(s) | Verify |
|------|-------------|---------|--------|
| P4.1 | Parse document into AST on `did_open`/`did_change`, cache AST per URI | `src/lsp/server.rs` | AST cached |
| P4.2 | Build symbol table from cached AST (function defs, struct defs, variable bindings with scopes) | `src/lsp/server.rs` or `src/lsp/symbols.rs` | Symbol table built |
| P4.3 | Replace text-search `goto_definition` with symbol table lookup (scope-aware) | `src/lsp/server.rs` | Finds correct definition in nested scopes |
| P4.4 | Update `references` to use symbol table instead of word-search | `src/lsp/server.rs` | Only finds same-scope references |
| P4.5 | Tests: goto-definition with shadowed variables resolves to correct scope | `src/lsp/` tests | 3 new tests |

---

## Phase 5: Feature-Gated Code CI (10 tasks)

**Goal:** Ensure feature-gated code (Z3, libclang, pyo3) actually compiles and passes tests when features are enabled. Add CI jobs.

| Task | Description | File(s) | Verify |
|------|-------------|---------|--------|
| P5.1 | Add CI matrix job: `cargo test --features smt` (requires Z3 dev libs) | `.github/workflows/ci.yml` | CI job passes |
| P5.2 | Add CI matrix job: `cargo test --features cpp-ffi` (requires libclang) | `.github/workflows/ci.yml` | CI job passes |
| P5.3 | Add CI matrix job: `cargo test --features python-ffi` (requires Python 3.8+) | `.github/workflows/ci.yml` | CI job passes |
| P5.4 | Add `apt-get install libz3-dev` to CI setup for smt job | `.github/workflows/ci.yml` | Z3 headers available |
| P5.5 | Add `apt-get install libclang-dev` to CI setup for cpp-ffi job | `.github/workflows/ci.yml` | libclang available |
| P5.6 | Add `apt-get install python3-dev` to CI setup for python-ffi job | `.github/workflows/ci.yml` | Python headers available |
| P5.7 | Verify `fj verify` CLI works with `--features smt` on a real .fj program | `tests/` | Z3 returns sat/unsat |
| P5.8 | Verify C++ header parsing works with `--features cpp-ffi` | `tests/` | Struct extracted from .h |
| P5.9 | Verify Python interop works with `--features python-ffi` | `tests/` | Python function called |
| P5.10 | Document feature flags in README.md and `fj --help` | `README.md`, `src/main.rs` | Flags documented |

---

## Phase 6: GUI OS Windowing (15 tasks)

**Goal:** Add real OS windowing via `winit` (cross-platform window creation library).

| Task | Description | File(s) | Verify |
|------|-------------|---------|--------|
| P6.1 | Add `winit = { version = "0.30", optional = true }` to Cargo.toml | `Cargo.toml` | Dependency resolves |
| P6.2 | Add `softbuffer = { version = "0.4", optional = true }` for pixel buffer display | `Cargo.toml` | Dependency resolves |
| P6.3 | Add `gui = ["dep:winit", "dep:softbuffer"]` feature flag | `Cargo.toml` | Feature defined |
| P6.4 | Implement `WinitBackend` in `src/gui/platform.rs` that creates a real OS window | `src/gui/platform.rs` | Window opens on screen |
| P6.5 | Implement event loop: translate winit events → GUI `Event` enum | `src/gui/platform.rs` | Mouse/keyboard events dispatched |
| P6.6 | Implement `SoftbufferRenderer`: blit `Canvas` pixels to OS window via softbuffer | `src/gui/platform.rs` | Pixels visible on screen |
| P6.7 | Implement `run_app(root_widget, config)` entry point | `src/gui/mod.rs` | App runs with event loop |
| P6.8 | Handle window resize events → relayout | `src/gui/platform.rs` | Widgets resize correctly |
| P6.9 | Handle DPI scaling via `winit::dpi` | `src/gui/platform.rs` | HiDPI renders correctly |
| P6.10 | Implement real clipboard via `arboard` crate (optional dep) | `src/gui/platform.rs` | Copy/paste works with OS clipboard |
| P6.11 | Add `fj gui` CLI command to launch GUI app from .fj source | `src/main.rs` | `fj gui examples/calculator.fj` opens window |
| P6.12 | Wire gui builtins into interpreter: `gui_window()`, `gui_button()`, etc. | `src/interpreter/eval/builtins.rs` | .fj code can create windows |
| P6.13 | Example: `examples/gui_hello.fj` — window with button and label | `examples/gui_hello.fj` | Window opens, button clickable |
| P6.14 | Example: `examples/gui_calculator.fj` — calculator app | `examples/gui_calculator.fj` | Calculator functional |
| P6.15 | Tests: create window, render widget, close (headless via `DISPLAY=:99` or feature gate) | `src/gui/` tests | Tests pass in CI |

---

## Phase 7: Template External Integration (15 tasks)

**Goal:** Make app template external integrations real (Docker, OpenAPI, BLE).

| Task | Description | File(s) | Verify |
|------|-------------|---------|--------|
| P7.1 | `fj docker` command: generate real Dockerfile from `fj.toml` | `src/main.rs`, `src/package/manifest.rs` | `fj docker` produces valid Dockerfile |
| P7.2 | `fj openapi` command: generate OpenAPI 3.0 YAML from route annotations | `src/main.rs`, new `src/tools/openapi.rs` | Valid OpenAPI YAML generated |
| P7.3 | BLE builtin: `ble_scan()` → list of simulated BLE devices | `src/interpreter/eval/builtins.rs` | Returns device list |
| P7.4 | BLE builtin: `ble_connect(addr)` → handle | `src/interpreter/eval/builtins.rs` | Returns connection |
| P7.5 | BLE builtin: `ble_read(handle, char_uuid)` → bytes | `src/interpreter/eval/builtins.rs` | Returns data |
| P7.6 | BLE builtin: `ble_write(handle, char_uuid, data)` | `src/interpreter/eval/builtins.rs` | Returns success |
| P7.7 | BLE builtin: `ble_disconnect(handle)` | `src/interpreter/eval/builtins.rs` | Connection closed |
| P7.8 | TensorBoard builtin: `tb_log_scalar(tag, value, step)` | `src/interpreter/eval/builtins.rs` | Writes TF event file |
| P7.9 | TensorBoard builtin: `tb_log_histogram(tag, values, step)` | `src/interpreter/eval/builtins.rs` | Writes histogram event |
| P7.10 | Register all new builtins in type checker | `src/analyzer/type_check.rs` | Analyzer accepts calls |
| P7.11 | Template `template_web_service.fj` — verify all builtins work | `examples/template_web_service.fj` | Template runs end-to-end |
| P7.12 | Template `template_iot_edge.fj` — verify BLE + MQTT builtins | `examples/template_iot_edge.fj` | Template runs end-to-end |
| P7.13 | Example `examples/mnist_99.fj` — real MNIST training achieving 90%+ | `examples/mnist_99.fj` | Prints accuracy > 90% |
| P7.14 | Example `examples/cifar_demo.fj` — CIFAR-10 inference demo | `examples/cifar_demo.fj` | Runs without error |
| P7.15 | Tests for all new builtins | `tests/eval_tests.rs` | 8 new tests |

---

## Phase 8: Self-Hosting Parser Upgrade (5 tasks)

**Goal:** Upgrade `stdlib/parser.fj` from position-tracking to AST-building.

| Task | Description | File(s) | Verify |
|------|-------------|---------|--------|
| P8.1 | Define AST node types in `stdlib/ast.fj` using Fajar Lang enums | `stdlib/ast.fj` | Types defined |
| P8.2 | Modify `stdlib/parser.fj` to return AST nodes instead of positions | `stdlib/parser.fj` | Parser returns AST |
| P8.3 | Add `stdlib/analyzer.fj` — basic type checking in Fajar Lang | `stdlib/analyzer.fj` | Catches type errors |
| P8.4 | Bootstrap test: self-hosted pipeline lexes+parses a .fj file | test script | Output matches Rust pipeline |
| P8.5 | Update `examples/selfhost_test.fj` to use new AST parser | `examples/selfhost_test.fj` | Runs successfully |

---

## Phase 9: Documentation & Verification (10 tasks)

**Goal:** Update all documentation to reflect real state, verify everything compiles and passes.

| Task | Description | File(s) | Verify |
|------|-------------|---------|--------|
| P9.1 | Update `docs/GAP_ANALYSIS_V2.md` with new audit results | `docs/GAP_ANALYSIS_V2.md` | Reflects current state |
| P9.2 | Update `CLAUDE.md` test counts and LOC | `CLAUDE.md` | Accurate numbers |
| P9.3 | Update `community/ECOSYSTEM_HEALTH.md` metrics | `community/ECOSYSTEM_HEALTH.md` | Current metrics |
| P9.4 | Run `cargo test --lib` — ALL must pass | — | 0 failures |
| P9.5 | Run `cargo test --features native` — ALL must pass | — | 0 failures |
| P9.6 | Run `cargo clippy -- -D warnings` — 0 warnings | — | Clean |
| P9.7 | Run `cargo fmt -- --check` — clean | — | Clean |
| P9.8 | Run `cargo doc` — no warnings | — | Docs build |
| P9.9 | Verify all 126+ examples lex and parse without errors | script | 0 failures |
| P9.10 | Tag release `v7.0.0 "Integrity"` | git tag | Tag created |

---

## Dependency Graph

```
Phase 1 (Wiring)──────────────┐
Phase 2 (Builtins)─────────┐  │
Phase 3 (Playground WASM)──┤  │
Phase 4 (LSP)──────────────┤  ├──→ Phase 9 (Verify + Release)
Phase 5 (CI Features)───────┤  │
Phase 6 (GUI Windowing)─────┤  │
Phase 7 (Template Integr.)──┤  │
Phase 8 (Self-Host Upgrade)─┘  │
                                │
Phases 1-8 can run in parallel──┘
```

Phases 1-8 are independent and can be executed in any order.
Phase 9 must run last to verify everything.

---

## Priority Order (Recommended)

| Priority | Phase | Reason |
|----------|-------|--------|
| 1 | **Phase 1** — Compiler Pipeline Wiring | Highest impact: makes security/profiler/optimizer real in compiled output |
| 2 | **Phase 2** — Interpreter Builtins | Unblocks Phase 7 templates |
| 3 | **Phase 6** — GUI Windowing | Most visible: users can see windows on screen |
| 4 | **Phase 3** — Playground WASM | Most impactful for adoption: try Fajar Lang in browser |
| 5 | **Phase 5** — CI Feature Gates | Ensures feature-gated code stays working |
| 6 | **Phase 7** — Template Integration | Makes app templates fully runnable |
| 7 | **Phase 4** — LSP Improvements | Quality of life for developers |
| 8 | **Phase 8** — Self-Host Upgrade | Long-term goal, not urgent |
| 9 | **Phase 9** — Verification | Must be last |

---

## Summary

| Phase | Tasks | Est. LOC | Effort |
|-------|-------|----------|--------|
| 1. Compiler Wiring | 20 | ~2,000 | High — touches Cranelift pipeline |
| 2. Interpreter Builtins | 15 | ~1,200 | Medium — well-defined interface |
| 3. Playground WASM | 15 | ~800 | Medium — wasm-bindgen + JS wiring |
| 4. LSP Improvements | 5 | ~500 | Medium — AST-based symbol table |
| 5. CI Feature Gates | 10 | ~200 | Low — CI config + apt-get |
| 6. GUI Windowing | 15 | ~2,500 | High — winit integration |
| 7. Template Integration | 15 | ~1,500 | Medium — builtins + examples |
| 8. Self-Host Upgrade | 5 | ~1,000 | High — parser rewrite in .fj |
| 9. Verification | 10 | ~200 | Low — testing + docs |
| **TOTAL** | **110** | **~10,000** | |

**When all 110 tasks are complete, every single one of the 810 V8 tasks will be genuinely, provably, end-to-end real.**

---

*GAP_CLOSURE_PLAN_V9.md — Version 1.0 — 2026-03-28*
