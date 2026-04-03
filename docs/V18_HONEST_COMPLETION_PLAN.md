# V18 "Integrity" — Honest Framework-to-Production Plan

> **Date:** 2026-04-03
> **Goal:** Wire 14 framework areas into production E2E — no batch-marking, no inflated claims
> **Rule:** [x] = user can `fj <command>` and it works. Each task verified individually.
> **Baseline:** V17 audit — 33/56 modules production, 18 framework, 3 stub

---

## Phase 1: Critical Foundation (9 tasks, ~1 week)

Fix the things users hit immediately.

| # | Task | Area | Effort | Verification |
|---|------|------|--------|-------------|
| 1.1 | Test ALL tensor aliases blocked in `@kernel` (zeros, ones, randn, matmul, relu, sigmoid, softmax, backward, grad, Dense, Adam) | @kernel | S | `cargo test --test context_safety_tests` |
| 1.2 | Test heap builtins blocked in `@kernel` (concat, map_insert, to_string) | @kernel | S | `cargo test --test context_safety_tests ke001` |
| 1.3 | Add missing builtins to `heap_builtins` set (concat, map_insert, map_get, map_remove) | @kernel | S | Tests from 1.2 pass |
| 1.4 | Add `http_get`/`http_post` synchronous builtins (real std::net) | Networking | M | `fj run test_http.fj` → prints HTTP response |
| 1.5 | Register http_get/http_post in analyzer known builtins | Networking | S | `fj check test_http.fj` → no SE004 |
| 1.6 | Add `tcp_connect`/`tcp_send`/`tcp_recv` builtins | Networking | M | `fj run test_tcp.fj` → connects to TCP |
| 1.7 | Fix HashMap `map_insert`/`map_get` if still broken | Bugs | M | `fj run` → insert then get returns value |
| 1.8 | Integration test: `@kernel fn` with tensor call → REJECTED by `fj check` | @kernel E2E | S | `cargo test context_safety` |
| 1.9 | Integration test: `@device fn` calling `mem_alloc` → REJECTED | @device E2E | S | `cargo test context_safety` |

**Gate:** `cargo test --lib && cargo test --test context_safety_tests && cargo clippy -- -D warnings`

---

## Phase 2: High-Impact User Features (12 tasks, ~2 weeks)

### 2A: Complete Networking (4 tasks)

| # | Task | Effort | Verification |
|---|------|--------|-------------|
| 2.1 | Wire `http_serve` with route handler callbacks from .fj | M | `fj run http_server.fj` → `curl localhost:8080/health` responds |
| 2.2 | Add `dns_resolve(hostname) -> str` builtin | S | `fj run` → `dns_resolve("google.com")` returns IP |
| 2.3 | Write example: HTTP echo server with 3 routes | S | `fj run examples/http_echo_server.fj` serves correctly |
| 2.4 | Write example: WebSocket client | S | `fj run examples/ws_client.fj` (--features websocket) |

### 2B: Real Generator Yield/Resume (3 tasks)

| # | Task | Effort | Verification |
|---|------|--------|-------------|
| 2.5 | Implement generator resume via `ControlFlow::Yield` in interpreter | L | `gen fn range(n) { for i in 0..n { yield i } }` produces values lazily |
| 2.6 | Wire `gen fn` keyword in parser (if not already) | M | `fj dump-ast` shows GeneratorFn node |
| 2.7 | Add generator integration test | S | `cargo test --test eval_tests generator` |

### 2C: FFI v2 — Call C Functions (3 tasks)

| # | Task | Effort | Verification |
|---|------|--------|-------------|
| 2.8 | Wire `ffi_load_library(path)` + `ffi_call(handle, name, args)` builtins | L | `ffi_call(libc, "getpid", [])` returns PID |
| 2.9 | Connect `fj bindgen` output to `ffi_call` | M | `fj bindgen math.h && fj run math.fj` calls cos(3.14) |
| 2.10 | Block `ffi_call` inside `@safe` context | S | `fj check` rejects `@safe fn f() { ffi_call(...) }` |

### 2D: @kernel/@device Edge Cases (2 tasks)

| # | Task | Effort | Verification |
|---|------|--------|-------------|
| 2.11 | Enforce context for method calls: `tensor.reshape()` in @kernel → error | M | `fj check` emits KE002 |
| 2.12 | ⚠️ STRETCH: Enforce transitive context (fn that wraps tensor ops) | L | Warning first, error later |

**Note on 2.12:** Transitive taint analysis is architecturally hard — requires call-graph annotation propagation. Ship as warning first, promote to error next release.

**Gate:** `cargo test --lib && fj run examples/http_echo_server.fj`

---

## Phase 3: Ecosystem Polish (10 tasks, ~2 weeks)

### 3A: LSP Feature Merge (3 tasks)

| # | Task | Effort | Verification |
|---|------|--------|-------------|
| 3.1 | Wire lsp_v2 TypeDrivenCompleter into `fj lsp` | M | VS Code: type `tensor_` → see completions |
| 3.2 | Wire lsp_v3 SemanticTokenProvider into `fj lsp` | M | VS Code: semantic highlighting works |
| 3.3 | Wire lsp_v3 quick fixes into code actions | M | VS Code: "did you mean?" suggestions |

### 3B: Fix `fj build` Linking (2 tasks)

| # | Task | Effort | Verification |
|---|------|--------|-------------|
| 3.4 | Generate C runtime stubs for println/print/len in AOT binary | M | `fj build hello.fj -o hello && ./hello` prints output |
| 3.5 | Add integration test for `fj build` producing working binary | S | `cargo test --test launch_tests build_and_run` |

### 3C: Convert Demos to .fj Examples (3 tasks)

| # | Task | Effort | Verification |
|---|------|--------|-------------|
| 3.6 | Convert drone controller demo to examples/drone_demo.fj | S | `fj run examples/drone_demo.fj` |
| 3.7 | Convert mini OS demo to examples/mini_os_demo.fj | S | `fj run examples/mini_os_demo.fj` |
| 3.8 | Add `fj demo <name>` CLI command | S | `fj demo drone` runs drone demo |

### 3D: LLVM Backend (2 tasks)

| # | Task | Effort | Verification |
|---|------|--------|-------------|
| 3.9 | Fix LLVM compile errors (CE009 void type, missing fields) | M | `cargo test --features llvm --lib` passes |
| 3.10 | Add `fj run --llvm fibonacci.fj` integration test | S | `cargo test --features llvm` |

**Gate:** `cargo test --lib && fj build examples/hello.fj -o hello && ./hello`

---

## Phase 4: Advanced — Stretch (8 tasks, ~3 weeks)

Each is architecturally hard. Honest effort estimates.

| # | Task | Effort | Verification |
|---|------|--------|-------------|
| 4.1 | Wire user `macro_rules!` expansion at parse time | L | `macro_rules! double { ($x:expr) => { $x * 2 } }; println(double!(21))` → 42 |
| 4.2 | Real async event loop (replace cooperative with tokio spawn) | L | `async { sleep(100); 42 }.await` returns 42 after 100ms |
| 4.3 | Wire const fn compile-time evaluation | M | `const fn square(x) { x * x }; println(square(5))` evaluates at compile time |
| 4.4 | Wire refinement type checks into runtime assertions | M | `fn f(x: i64 where x > 0)` + `f(-1)` → refinement error |
| 4.5 | Wire actor system as `actor_spawn`/`actor_send` builtins | L | Actor spawn, send, receive works from .fj |
| 4.6 | Wire deployment module to `fj deploy --container` | M | `fj deploy --container hello.fj` → Dockerfile |
| 4.7 | Wire transformer as `transformer_create` builtin | M | Forward pass from .fj works |
| 4.8 | `fj bootstrap` Stage 0 produces non-zero binary | L | Binary size > 0 bytes |

---

## Explicitly NOT Planned

| Item | Reason |
|------|--------|
| Distributed multi-node execution | Needs 3+ nodes, can't verify with `fj` alone |
| BLE real hardware | Needs Bluetooth hardware, feature-gated correctly |
| RTOS/IoT simulation | Intentional design — not a gap |
| GUI without --features gui | winit/softbuffer correctly feature-gated |
| Plugin system | No user demand yet |

---

## Summary

| Phase | Tasks | Effort | Module count [f]→[x] |
|-------|-------|--------|----------------------|
| 1: Foundation | 9 | ~1 week | +0 (fixes, not new modules) |
| 2: User Features | 12 | ~2 weeks | +3 (networking, generators, FFI) |
| 3: Polish | 10 | ~2 weeks | +4 (lsp_v2/v3, deployment, demos) |
| 4: Stretch | 8 | ~3 weeks | +4 (macros, async, const, actors) |
| **Total** | **39** | **~8 weeks** | **+11 modules [f]→[x]** |

After V18: **44/56 modules production (79%)**, up from 33/56 (59%).

Remaining 12 modules stay [f] by design (rtos, iot, demos-as-rust, hardening, const_alloc/reflect/stdlib/bench, plugin, jit, rt_pipeline).

---

*V18 "Integrity" Plan — 2026-04-03 — Written with honesty, verified against actual code*
