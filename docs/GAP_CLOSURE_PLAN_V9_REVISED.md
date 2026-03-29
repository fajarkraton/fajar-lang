# GAP_CLOSURE_PLAN_V9_REVISED.md — Complete Gap Closure

> **Date:** 2026-03-29
> **Author:** Claude Opus 4.6 + Fajar
> **Method:** Per-module code audit (reading every function body, not just headers)
> **Rule:** Every task verified by integration test, not unit test on types

---

## Executive Summary

Full per-module audit (35,341 LOC across 19 modules, 815 tests) reveals a different picture than GAP_ANALYSIS_V2:

| Category | V2 Assessment | Actual (Audit 2026-03-29) |
|----------|--------------|--------------------------|
| `stdlib_v3/formats.rs` | Stub — needs parsing | **100% real** — hand-written JSON/TOML/CSV parsers |
| `stdlib_v3/system.rs` | Stub — needs process spawn | **100% real** — real std::process, walk_dir, etc. |
| `stdlib_v3/crypto.rs` | Stub — needs RustCrypto | **95% real** — SHA/AES/Ed25519/Argon2 via real crates |
| `tensor_verify.rs` | Stub — needs SMT | **100% real** — standalone shape checker |
| `codegen/wasm/` | Basic — needs WASI P2 | **95% real** — hand-written binary emitter |
| `codegen/ptx.rs` | Stub — needs CUDA | **100% real** — complete PTX text emitter |
| Plugin system | Does not exist | **Exists** — 940 LOC, `libloading`, 5 built-in plugins |

**The real problem is not missing implementations — it's missing WIRING.**

18 modules totaling 35K LOC sit disconnected from the main compiler pipeline. The fix is integration, not rewrite.

---

## Revised Gap Classification

### Already Real — No Work Needed (remove from plan)

| Module | LOC | Tests | Verdict |
|--------|-----|-------|---------|
| `stdlib_v3/formats.rs` | 1,458 | 57 | 100% real — JSON/TOML/CSV hand-written parsers |
| `stdlib_v3/system.rs` | 1,340 | 38 | 100% real — real std::process/fs/env |
| `stdlib_v3/crypto.rs` | 1,536 | 51 | 95% real — SHA/AES/Ed25519/Argon2 all real |
| `verify/tensor_verify.rs` | 600 | 22 | 100% real — shape verification algorithms |
| `codegen/ptx.rs` | 711 | 19 | 100% real — PTX text emitter |
| `codegen/wasm/` | 2,921 | 40 | 95% real — binary emitter + compiler |

**These 6 modules (8,566 LOC) need ZERO implementation work.**

### Needs Integration Only (wiring into pipeline)

| Module | LOC | Tests | What's Missing |
|--------|-----|-------|---------------|
| `codegen/security.rs` | 2,919 | 72 | Not called from Cranelift `compile_function()` |
| `codegen/opt_passes.rs` | 3,668 | 56 | Not called from Cranelift pipeline |
| `profiler/` | 3,340 | 66 | Not called from interpreter or Cranelift |
| `dependent/` | 2,529 | 102 | Not integrated into `analyzer/type_check.rs` |
| `lsp_v3/` | 2,368 | 42 | Not connected to `tower-lsp` server |
| `compiler/incremental/` | 2,780 | 30 | Uses simulated compile, not real pipeline |
| `plugin/` | 940 | 19 | Not invoked from `main.rs` |

### Needs Completion (real logic exists, some parts simulated)

| Module | LOC | Tests | What's Missing |
|--------|-----|-------|---------------|
| `concurrency_v2/` | 2,718 | 77 | Single-threaded — needs tokio threads |
| `distributed/` | 4,041 | 85 | Transport real TCP, RPC/tensors simulated |
| `rt_pipeline/` | 2,554 | 47 | Simulated sensors — needs real pipeline wiring |

### Feature-Gated (already work when feature enabled)

| Module | LOC | Tests | Feature Flag | External Dep |
|--------|-----|-------|-------------|-------------|
| `ffi_v2/cpp.rs` | 1,499 | 19 | `cpp-ffi` | `clang-sys` + libclang |
| `ffi_v2/python.rs` | 1,268 | 43 | `python-ffi` | `pyo3` + Python 3.8+ |
| `verify/smt.rs` | 1,101 | 30 | `smt` | `z3` crate + libz3 |

**These already work in CI (Phase 5 of old V9 = DONE).**

---

## Phase 1: Compiler Pipeline Wiring (18 tasks)

**Goal:** Wire `security.rs`, `opt_passes.rs`, `profiler/` into the Cranelift codegen so they produce real effects in compiled output.

**Priority:** HIGHEST — transforms 10K LOC of isolated code into working compiler features.

### P1.1: Security → Cranelift (8 tasks)

| # | Task | File(s) | Verify |
|---|------|---------|--------|
| P1.1.1 | Import `SecurityConfig` and add as field on `CraneliftCompiler` | `cranelift/mod.rs`, `cranelift/context.rs` | Field accessible in compile methods |
| P1.1.2 | Call `StackCanaryConfig::generate()` at function prologue in `compile_function()` — emit canary write before first instruction | `cranelift/compile/mod.rs` | Disasm shows canary store at function entry |
| P1.1.3 | Emit canary check at every function return — compare canary, call `fj_rt_stack_smash_detected` on mismatch | `cranelift/compile/mod.rs` | Disasm shows canary verify before ret |
| P1.1.4 | Add `fj_rt_stack_smash_detected` and `fj_rt_bounds_check_failed` to `runtime_fns.rs` | `cranelift/runtime_fns.rs` | Functions exist, abort with message |
| P1.1.5 | In `compile_array_access()`: when `BoundsCheckMode::Always`, emit index-vs-length comparison + trap | `cranelift/compile/arrays.rs` | OOB access → trap in compiled code |
| P1.1.6 | In debug builds (`-g`): wrap `iadd`/`isub`/`imul` with overflow-checking variants via `BoundsCheckMode` config | `cranelift/compile/expr.rs` | Debug overflow → trap |
| P1.1.7 | Run `SecurityLinter::lint()` before codegen in `compile_program()`, emit warnings | `cranelift/mod.rs` | Lint warnings appear in compiler output |
| P1.1.8 | Tests: compile .fj with canary enabled, corrupt stack, verify abort; OOB array → trap; debug overflow → trap | `cranelift/tests.rs` | 5 new native tests pass |

### P1.2: Optimizer → Cranelift (5 tasks)

| # | Task | File(s) | Verify |
|---|------|---------|--------|
| P1.2.1 | Add `OptimizationLevel` field to `CraneliftCompiler` (O0/O1/O2/O3 from CLI `--opt-level`) | `cranelift/mod.rs`, `src/main.rs` | `fj build --opt-level 2` parsed |
| P1.2.2 | Before Cranelift codegen, run `OptimizationPipeline::run()` on the AST per optimization level | `cranelift/mod.rs` | Passes executed, metrics collected |
| P1.2.3 | Wire `ConstantFolder::fold()` into expression compilation — constant exprs resolved at compile time | `cranelift/compile/expr.rs` | `2 + 3` compiles to `iconst 5` |
| P1.2.4 | Wire `DeadFunctionEliminator::eliminate()` — skip codegen for unreachable functions | `cranelift/mod.rs` | Dead functions not emitted in object file |
| P1.2.5 | Tests: verify constant folding in disasm, dead function not in object, optimization report generated | `cranelift/tests.rs` | 3 new tests |

### P1.3: Profiler → Pipeline (5 tasks)

| # | Task | File(s) | Verify |
|---|------|---------|--------|
| P1.3.1 | Add `--profile` flag to `fj run` CLI | `src/main.rs` | Flag parsed, `ProfileSession` created |
| P1.3.2 | In interpreter `eval_call()`: if profiling, call `session.enter_fn(name)` / `session.exit_fn()` | `src/interpreter/eval.rs` | Profiler records function timings |
| P1.3.3 | Add `fj_rt_profile_enter(name_ptr, name_len)` and `fj_rt_profile_exit()` to runtime_fns | `cranelift/runtime_fns.rs` | Runtime functions exist |
| P1.3.4 | In Cranelift `compile_function()`: if profiling, emit calls to `fj_rt_profile_enter`/`exit` | `cranelift/compile/mod.rs` | Native code has profile instrumentation |
| P1.3.5 | On program exit with `--profile`, write `fj-profile.json` in Chrome trace format; test with sample program | `src/main.rs`, tests | JSON file written, loadable in chrome://tracing |

---

## Phase 2: Analyzer Integration (8 tasks)

**Goal:** Wire `dependent/` types and `tensor_verify.rs` into the semantic analyzer so compile-time shape/bound checking actually works.

### P2.1: Dependent Types → Analyzer (5 tasks)

| # | Task | File(s) | Verify |
|---|------|---------|--------|
| P2.1.1 | Import `NatValue`, `NatConstraint`, `DepArray`, `DepTensor` in `type_check.rs` | `src/analyzer/type_check.rs` | Types accessible |
| P2.1.2 | In array type checking: when array has const-known length, create `DepArray` and track constraints | `src/analyzer/type_check.rs` | `let a: [i32; 5]` creates DepArray with length=Lit(5) |
| P2.1.3 | In array index checking: when index is const-known, verify against `DepArray` length — emit compile error on OOB | `src/analyzer/type_check.rs` | `a[10]` on len-5 array → SE error |
| P2.1.4 | In tensor type checking: when shapes are const-known, verify matmul/reshape/broadcast using `tensor_verify.rs` | `src/analyzer/type_check.rs` | `matmul(3x4, 5x6)` → shape error at compile time |
| P2.1.5 | Tests: const array OOB, tensor shape mismatch, valid shapes pass, unknown shapes deferred | `src/analyzer/` tests | 6 new tests |

### P2.2: Plugin System → Compiler Pipeline (3 tasks)

| # | Task | File(s) | Verify |
|---|------|---------|--------|
| P2.2.1 | In `main.rs` compilation flow: after parse, before analyze — run `PluginRegistry::run_ast_phase()` | `src/main.rs` | Built-in lint plugins run on every compile |
| P2.2.2 | Add `--plugin <path>` CLI flag to load external plugins via `load_plugin_from_path()` | `src/main.rs` | External .so/.dylib plugin loaded and executed |
| P2.2.3 | Tests: built-in TodoLint fires on `todo!()` in .fj; NamingConventionLint fires on camelCase function | tests | 2 new tests |

---

## Phase 3: LSP v3 → Server Wiring (7 tasks)

**Goal:** Connect `lsp_v3/` semantic tokens, diagnostics, and refactoring to the actual `tower-lsp` server.

| # | Task | File(s) | Verify |
|---|------|---------|--------|
| P3.1 | In `lsp/server.rs` `did_open`/`did_change`: parse document into AST, cache per URI | `src/lsp/server.rs` | AST cached on file open/change |
| P3.2 | Build symbol table from cached AST (using analyzer's scope chain) | `src/lsp/server.rs` | Symbol table with scope info |
| P3.3 | Replace text-search `goto_definition` with `lsp_v3/semantic.rs` symbol table lookup | `src/lsp/server.rs` | Finds correct definition in nested scopes |
| P3.4 | Wire `lsp_v3/semantic.rs` `generate_semantic_tokens()` into `semantic_tokens_full` handler | `src/lsp/server.rs` | VS Code shows colored semantic tokens |
| P3.5 | Wire `lsp_v3/diagnostics.rs` quick-fix actions into `code_action` handler | `src/lsp/server.rs` | "Remove unused variable" action appears |
| P3.6 | Wire `lsp_v3/refactoring.rs` `rename_symbol()` into `rename` handler (AST-based) | `src/lsp/server.rs` | Rename works across scopes correctly |
| P3.7 | Tests: goto-def with shadowed vars, semantic tokens for keyword/ident/literal, rename refactoring | `src/lsp/` tests | 5 new tests |

---

## Phase 4: Incremental Compilation → Real Pipeline (5 tasks)

**Goal:** Replace `simulate_compile()` with real `tokenize()`→`parse()`→`analyze()` and add disk persistence.

| # | Task | File(s) | Verify |
|---|------|---------|--------|
| P4.1 | Replace `simulate_compile(source)` with real `tokenize(source)` + `parse(tokens)` + `analyze(program)` — return serialized AST bytes | `src/compiler/incremental/pipeline.rs` | Incremental build invokes real compiler stages |
| P4.2 | Add disk cache using `serde_json` — save `ArtifactCache` to `.fj-cache/` on exit, load on start | `src/compiler/incremental/cache.rs` | Cache persists across builds |
| P4.3 | Add `fj build --incremental` flag to `main.rs` — use `IncrementalCompiler` for multi-file projects | `src/main.rs` | Flag parsed, incremental build runs |
| P4.4 | In `IncrementalCompiler::rebuild()`: only re-lex/parse/analyze changed files + transitive dependents | `src/compiler/incremental/pipeline.rs` | Second build skips unchanged files |
| P4.5 | Tests: build project, modify one file, rebuild — verify only changed module recompiled | tests | 2 new tests |

---

## Phase 5: Concurrency v2 → Real Threads (5 tasks)

**Goal:** Make actor mailboxes use real `tokio::sync::mpsc` channels instead of single-threaded VecDeque.

| # | Task | File(s) | Verify |
|---|------|---------|--------|
| P5.1 | Replace `Mailbox` VecDeque with `tokio::sync::mpsc::channel` (bounded) | `src/concurrency_v2/actors.rs` | Mailbox is thread-safe |
| P5.2 | `ActorInstance::spawn()` — run actor message loop in `tokio::spawn()` task | `src/concurrency_v2/actors.rs` | Actor runs on tokio runtime |
| P5.3 | `ActorRef::send()` — async send via mpsc sender | `src/concurrency_v2/actors.rs` | Cross-thread message delivery |
| P5.4 | Wire `AsyncScope` to use `tokio::task::JoinSet` for structured concurrency | `src/concurrency_v2/scopes.rs` | Scoped tasks run truly parallel |
| P5.5 | Tests: spawn 2 actors, send messages between them, verify delivery order | `src/concurrency_v2/` tests | 3 new tests using `#[tokio::test]` |

---

## Phase 6: Distributed Transport Completion (5 tasks)

**Goal:** Complete the RPC connection pool and tensor allreduce with real TCP transport (transport.rs already has real tokio TCP).

| # | Task | File(s) | Verify |
|---|------|---------|--------|
| P6.1 | Replace simulated `RpcConnectionPool` with real `TcpStream` pool using `transport::TransportNode` | `src/distributed/rpc.rs` | RPC calls go over real TCP |
| P6.2 | Wire `RpcService::call()` to serialize request, send via transport, deserialize response | `src/distributed/rpc.rs` | Remote procedure call works end-to-end |
| P6.3 | In `tensors.rs`: implement `ring_allreduce_real()` that sends tensor shards via transport | `src/distributed/tensors.rs` | Tensor data actually transferred between nodes |
| P6.4 | Wire `ClusterManager::heartbeat()` to use transport for real node-to-node health checks | `src/distributed/cluster.rs` | Heartbeat over TCP, failure detected |
| P6.5 | Tests: start 2 TransportNodes on localhost, RPC call, verify response; 2-node allreduce | `src/distributed/` tests | 3 new `#[tokio::test]` tests |

---

## Phase 7: Interpreter Builtins (9 tasks)

**Goal:** Add WebSocket and MQTT builtins referenced by app templates.

### P7.1: WebSocket Builtins (4 tasks)

| # | Task | File(s) | Verify |
|---|------|---------|--------|
| P7.1.1 | Implement `ws_accept(socket)`, `ws_send(ws, msg)`, `ws_recv(ws)`, `ws_close(ws)` — in-memory simulation with RFC 6455 frame format | `src/interpreter/eval.rs` | Builtins callable from .fj |
| P7.1.2 | Register ws_* signatures in `type_check.rs` builtin table | `src/analyzer/type_check/register.rs` | Analyzer accepts ws_* calls |
| P7.1.3 | Tests: WebSocket echo roundtrip in .fj | `tests/eval_tests.rs` | 2 tests |
| P7.1.4 | Update `examples/template_web_service.fj` to use ws_* builtins | `examples/template_web_service.fj` | Template runs without error |

### P7.2: MQTT Builtins (5 tasks)

| # | Task | File(s) | Verify |
|---|------|---------|--------|
| P7.2.1 | Implement in-memory `MqttBroker` (topic→subscribers map, message queue per subscriber) | `src/stdlib_v3/net.rs` | Broker routes messages |
| P7.2.2 | Implement `mqtt_connect`, `mqtt_publish`, `mqtt_subscribe`, `mqtt_receive`, `mqtt_disconnect` builtins | `src/interpreter/eval.rs` | Builtins callable from .fj |
| P7.2.3 | Register mqtt_* signatures in type checker | `src/analyzer/type_check/register.rs` | Analyzer accepts mqtt_* calls |
| P7.2.4 | Tests: publish-subscribe roundtrip | `tests/eval_tests.rs` | 2 tests |
| P7.2.5 | Update `examples/template_iot_edge.fj` to use mqtt_* builtins | `examples/template_iot_edge.fj` | Template runs without error |

---

## Phase 8: Playground WASM (10 tasks)

**Goal:** Build a real browser-based playground using the existing `codegen/wasm/` module + wasm-bindgen.

| # | Task | File(s) | Verify |
|---|------|---------|--------|
| P8.1 | Add `wasm-bindgen` dependency under `[target.'cfg(target_arch = "wasm32")'.dependencies]` | `Cargo.toml` | Dep resolves |
| P8.2 | Create `src/playground/wasm_api.rs` with `#[wasm_bindgen]` exports: `eval_source`, `tokenize_json`, `format_source` | `src/playground/wasm_api.rs` | Compiles to wasm32 |
| P8.3 | Ensure interpreter works without filesystem/network (stub I/O for wasm32) | `src/playground/wasm_api.rs` | No panics on eval |
| P8.4 | Add `build-playground.sh` script using wasm-pack | root | Script produces `playground/pkg/` |
| P8.5 | Update `playground/src/executor.js` to import real wasm module | `playground/src/executor.js` | JS imports wasm eval |
| P8.6 | Update `playground/src/worker.js` to use wasm-based eval | `playground/src/worker.js` | Worker runs .fj code in browser |
| P8.7 | Add wasm32 build check to CI | `.github/workflows/ci.yml` | CI verifies wasm builds |
| P8.8 | Add playground build to `docs.yml` deploy | `.github/workflows/docs.yml` | Playground deployed to Pages |
| P8.9 | Test: 5 example programs run correctly in playground | manual test | Correct output |
| P8.10 | Document playground in `playground/README.md` | `playground/README.md` | Architecture documented |

---

## Phase 9: GUI Windowing (10 tasks)

**Goal:** Add real OS windowing via `winit` + `softbuffer` for the existing `src/gui/` widget system.

| # | Task | File(s) | Verify |
|---|------|---------|--------|
| P9.1 | Add `winit = { version = "0.30", optional = true }` and `softbuffer = { version = "0.4", optional = true }` | `Cargo.toml` | Dependencies resolve |
| P9.2 | Add `gui = ["dep:winit", "dep:softbuffer"]` feature flag | `Cargo.toml` | Feature defined |
| P9.3 | Implement `WinitBackend` — create real OS window, event loop | `src/gui/platform.rs` | Window opens on screen |
| P9.4 | Translate winit events → `src/gui/` `Event` enum (mouse, keyboard, resize) | `src/gui/platform.rs` | Events dispatched to widgets |
| P9.5 | Implement `SoftbufferRenderer` — blit `Canvas` pixels to OS window | `src/gui/platform.rs` | Pixels visible on screen |
| P9.6 | Implement `run_app(root_widget)` entry point with event loop | `src/gui/mod.rs` | App runs with real event loop |
| P9.7 | Add `fj gui <file.fj>` CLI command | `src/main.rs` | Command opens GUI window |
| P9.8 | Wire gui builtins (`gui_window`, `gui_button`, `gui_label`) into interpreter | `src/interpreter/eval.rs` | .fj code creates windows |
| P9.9 | Example: `examples/gui_hello.fj` — window with button and label | `examples/gui_hello.fj` | Window opens, button works |
| P9.10 | Tests: headless widget render test (feature-gated) | `src/gui/` tests | 2 tests |

---

## Phase 10: Documentation & Verification (8 tasks)

**Goal:** Update all documentation to reflect real state, verify everything passes.

| # | Task | File(s) | Verify |
|---|------|---------|--------|
| P10.1 | Update `docs/GAP_ANALYSIS_V2.md` — reclassify modules per audit findings | `docs/GAP_ANALYSIS_V2.md` | Accurate per-module status |
| P10.2 | Update `CLAUDE.md` — test counts, LOC, accurate feature list | `CLAUDE.md` | Numbers match reality |
| P10.3 | `cargo test --lib` — ALL pass | — | 0 failures |
| P10.4 | `cargo test --features native` — ALL pass | — | 0 failures |
| P10.5 | `cargo clippy -- -D warnings` — 0 warnings | — | Clean |
| P10.6 | `cargo fmt -- --check` — clean | — | Clean |
| P10.7 | All 173 examples pass `fj check` (minus 3 selfhost by design) | script | 170/173 pass |
| P10.8 | Tag release | git tag | Version tagged |

---

## Dependency Graph

```
Phase 1 (Compiler Wiring) ──────────┐
Phase 2 (Analyzer Integration) ─────┤
Phase 3 (LSP v3 Wiring) ────────────┤
Phase 4 (Incremental Compilation) ───┤
Phase 5 (Concurrency v2 Threads) ────┼──→ Phase 10 (Verify + Release)
Phase 6 (Distributed Transport) ─────┤
Phase 7 (Interpreter Builtins) ──────┤
Phase 8 (Playground WASM) ───────────┤
Phase 9 (GUI Windowing) ────────────┘

Phase 2.1 depends on Phase 1 (analyzer needs security config for context checks)
Phase 6 depends on Phase 5 (distributed needs real async)
All others are independent and parallelizable.
```

---

## Priority Order

| Priority | Phase | Reason | Tasks | Est. LOC |
|----------|-------|--------|-------|----------|
| 1 | **Phase 1** — Compiler Pipeline Wiring | Highest impact: 10K LOC of security/optimizer/profiler becomes real | 18 | ~800 |
| 2 | **Phase 2** — Analyzer Integration | Compile-time shape/bounds checking + plugin system | 8 | ~400 |
| 3 | **Phase 3** — LSP v3 Wiring | Developer experience: semantic tokens, goto-def, refactoring | 7 | ~500 |
| 4 | **Phase 4** — Incremental Compilation | Build performance for large projects | 5 | ~300 |
| 5 | **Phase 7** — Interpreter Builtins | Unblocks app templates (WebSocket, MQTT) | 9 | ~600 |
| 6 | **Phase 5** — Concurrency v2 Threads | Real multi-threaded actors | 5 | ~200 |
| 7 | **Phase 6** — Distributed Transport | Complete RPC + tensor allreduce | 5 | ~400 |
| 8 | **Phase 8** — Playground WASM | Browser-based try-it — adoption critical | 10 | ~500 |
| 9 | **Phase 9** — GUI Windowing | Most visible: real OS windows | 10 | ~800 |
| 10 | **Phase 10** — Verification | Must be last | 8 | ~200 |

---

## Removed from Plan (no longer gaps)

These were in the original V9 or identified as gaps in V2, but the 2026-03-29 audit proves they are already real:

| Module | LOC | Why Removed |
|--------|-----|-------------|
| `stdlib_v3/formats.rs` | 1,458 | 100% real — hand-written JSON/TOML/CSV parsers with 57 tests |
| `stdlib_v3/system.rs` | 1,340 | 100% real — real std::process/fs/env with 38 tests |
| `stdlib_v3/crypto.rs` | 1,536 | 95% real — SHA/AES/Ed25519/Argon2 via real crates, 51 tests |
| `verify/tensor_verify.rs` | 600 | 100% real — standalone shape checker (integration added in Phase 2) |
| `codegen/ptx.rs` | 711 | 100% real — complete PTX text emitter, 19 tests |
| `codegen/wasm/` | 2,921 | 95% real — binary emitter + compiler, 40 tests |
| `rt_pipeline/` | 2,554 | 85% real — pipeline framework, not a compiler gap |
| Feature-gated CI (smt, cpp-ffi, python-ffi) | — | DONE — CI already runs these (Phase 5 of old V9) |
| `Self-hosting parser upgrade` | — | Enhancement, not gap closure |
| `Template external integrations` (Docker, BLE, TensorBoard) | — | New features, not documented gaps |

---

## Summary

| Phase | Tasks | Est. LOC | Effort |
|-------|-------|----------|--------|
| 1. Compiler Pipeline Wiring | 18 | ~800 | High — touches Cranelift |
| 2. Analyzer Integration | 8 | ~400 | Medium — type_check.rs + plugin |
| 3. LSP v3 Wiring | 7 | ~500 | Medium — tower-lsp integration |
| 4. Incremental Compilation | 5 | ~300 | Medium — pipeline hookup |
| 5. Concurrency v2 Threads | 5 | ~200 | Medium — tokio migration |
| 6. Distributed Transport | 5 | ~400 | Medium — wire existing TCP |
| 7. Interpreter Builtins | 9 | ~600 | Low — well-defined interface |
| 8. Playground WASM | 10 | ~500 | Medium — wasm-bindgen + JS |
| 9. GUI Windowing | 10 | ~800 | High — winit integration |
| 10. Verification | 8 | ~200 | Low — testing + docs |
| **TOTAL** | **85** | **~4,700** | |

**Key difference from original V9:**
- Original V9: 110 tasks, ~10,000 LOC — included reimplementation of already-real modules
- Revised V9: 85 tasks, ~4,700 LOC — focused on integration, not reimplementation
- 25 tasks removed (unnecessary), all remaining gaps actually closed
- Every task has a concrete verification method

**When all 85 tasks are complete, every module in the codebase will be wired into the compiler pipeline and functional end-to-end.**

---

*GAP_CLOSURE_PLAN_V9_REVISED.md — Version 2.0 — 2026-03-29*
*Based on per-module code audit, not documentation assumptions.*
