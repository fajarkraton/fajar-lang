# V06-V15 Gap Closure Plan — Production-Level E2E

> **Date:** 2026-04-03
> **Context:** V18 fixed 19 major gaps. This plan covers what STILL remains.
> **Standard:** Same as V19 — every task has concrete `fj run` verification with expected output.
> **Standard:** [x] = user can type command and see correct result. NO exceptions.

---

## Executive Summary

After V18 "Integrity", the V06-V15 gap status is:

| Category | Fixed by V18 | Still Framework | Intentional Sim | Total Gap |
|----------|-------------|-----------------|-----------------|-----------|
| Networking | 6 features | 0 | 0 | **0** |
| FFI | 3 features | 0 | 0 | **0** |
| Context enforcement | transitive | 0 | 0 | **0** |
| Generators | gen fn + yield | 0 | 0 | **0** |
| LLVM | fixed | 0 | 0 | **0** |
| RTOS/IoT | - | 0 | 2 modules | **sim by design** |
| Demos | 4 demos | ~10 demos | 0 | **10** |
| LSP v3 | partial | quick fixes | 0 | **1** |
| Accelerator | - | dispatch | 0 | **1** |
| Debugger v2 | - | record/replay | 0 | **1** |
| Package v2 | - | build scripts | 0 | **1** |
| ML advanced | transformer | diffusion/RL | 0 | **2** |
| RT pipeline | - | executor | 0 | **1** |
| JIT tiered | - | baseline | 0 | **defer** |
| Plugin | - | dynamic load | 0 | **defer** |
| Const/Hardening | const fn | ~20% remaining | 0 | **1** |

**Remaining actionable gaps: 18 items across 8 modules.**
**Intentionally deferred: JIT tiered, plugin system (low ROI for effort).**
**Intentionally simulation: RTOS, IoT (correct for hosted language).**

---

## Modules That Are DONE (No Gap)

These V06-V15 features are **fully production** after V18:

- **Networking:** http_get, http_post, http_listen, tcp_connect/send/recv, dns_resolve
- **FFI:** ffi_load_library, ffi_register, ffi_call, fj bindgen → FFI output
- **Context enforcement:** @kernel/@device blocks tensor/heap/OS + transitive + method calls
- **Generators:** gen fn + yield, for-in iteration
- **Channels:** channel_create, channel_send, channel_recv
- **LLVM backend:** compiles, 8,433 tests pass
- **AOT build:** fj build → working ELF with println
- **LSP:** tower-lsp server, hover, completion (v2 type-driven), diagnostics
- **Deploy:** fj deploy --container → Dockerfile
- **Crypto:** SHA-256/384/512, AES-GCM (real crates)
- **Regex:** regex_match, regex_find, regex_replace (real regex crate)
- **GPU codegen:** fj build --target spirv/ptx produces real GPU assembly
- **WASI P2:** fj build --target wasm32-wasi-p2 produces WASM component
- **BSP:** fj build --board stm32f407 generates bare-metal binary
- **Verification:** fj verify runs symbolic execution
- **Self-hosting:** fj bootstrap runs stage 1 validation
- **Package:** fj add/publish/search/tree/audit all work

---

## Modules Intentionally Simulation (NOT Gaps)

| Module | Why Simulation | Correct? |
|--------|---------------|----------|
| `src/rtos/` | FreeRTOS/Zephyr APIs — need actual RTOS hardware or emulator | Yes — simulation correct for hosted development |
| `src/iot/` | WiFi/BLE/MQTT/LoRaWAN — need ESP32 or hardware | Yes — simulation correct for host-side testing |

**These are NOT gaps.** A hosted language compiler correctly simulates embedded APIs.
Real hardware integration would require `--features freertos`/`--features esp32` with
actual C library linking — that's deployment-time, not compiler-time.

---

## Modules Intentionally Deferred (Low ROI)

| Module | Why Defer | When |
|--------|----------|------|
| `src/jit/` | Tiered JIT needs baseline compiler + OSR — L effort, low ROI for batch programs | V21+ if profiling shows hot loops |
| `src/plugin/` | Dynamic .so plugin loading — M effort, no user demand yet | V21+ when ecosystem grows |

---

## Remaining Gaps: 18 Tasks Across 8 Modules

### Gap 1: Demo Conversion (5 tasks, M effort)

**Problem:** `src/demos/` has 14 Rust reference implementations not accessible from .fj.
V18 converted 2 (drone, mini_os). 10+ remain.

| # | Task | Verification |
|---|------|-------------|
| G1.1 | Convert `database_demo.rs` → `examples/database_demo.fj` | `fj run examples/database_demo.fj` — creates DB, inserts, queries |
| G1.2 | Convert `web_fullstack_demo.rs` → `examples/web_demo.fj` | `fj run examples/web_demo.fj` — HTTP routes + JSON |
| G1.3 | Convert `embedded_ml_demo.rs` → `examples/embedded_ml_demo.fj` | `fj run examples/embedded_ml_demo.fj` — sensor → inference |
| G1.4 | Convert `cli_tool_demo.rs` → `examples/cli_tool_demo.fj` | `fj run examples/cli_tool_demo.fj` — arg parsing + output |
| G1.5 | Wire all demos into `fj demo --list` | `fj demo --list` shows all available demos |

### Gap 2: LSP v3 Quick Fixes (2 tasks, S effort)

**Problem:** LSP code_action handler has 4 hardcoded fixes (SE001/SE002/SE007/SE009).
`src/lsp_v3/diagnostics.rs` has structured QuickFix system not wired.

| # | Task | Verification |
|---|------|-------------|
| G2.1 | Wire lsp_v3 `QuickFixKind` into LSP code_action handler | VS Code shows "did you mean?" for typos |
| G2.2 | Add quick fix for SE004 type mismatch (suggest cast) | VS Code suggests `to_int(x)` when string given where int expected |

### Gap 3: Debugger v2 Record/Replay (3 tasks, M effort)

**Problem:** `src/debugger_v2/` has recording infrastructure but `fj debug --record` not wired.

| # | Task | Verification |
|---|------|-------------|
| G3.1 | Wire `--record` flag to interpreter — capture eval events | `fj debug --record trace.json examples/hello.fj` produces trace file |
| G3.2 | Wire `--replay` flag to replay trace | `fj debug --replay trace.json` replays execution with output |
| G3.3 | Add test: record → replay round-trip produces same output | `cargo test debug_replay_roundtrip` |

### Gap 4: Package v2 Build Scripts (2 tasks, M effort)

**Problem:** `src/package_v2/` defines build script types but no execution engine.

| # | Task | Verification |
|---|------|-------------|
| G4.1 | Parse `[build-script]` section from fj.toml | `fj build` reads build-script config |
| G4.2 | Execute build script before compilation, apply output directives | `fj build` with build script runs custom pre-build step |

### Gap 5: ML Advanced — Diffusion (2 tasks, M effort)

**Problem:** `src/ml_advanced/` has diffusion model types but not exposed as builtins.

| # | Task | Verification |
|---|------|-------------|
| G5.1 | Add `diffusion_create(steps)` and `diffusion_denoise(model, noisy, step)` builtins | `fj run` creates diffusion model, denoises a tensor |
| G5.2 | Example: `examples/diffusion_demo.fj` | `fj run examples/diffusion_demo.fj` — shows denoising steps |

### Gap 6: RT Pipeline (2 tasks, L effort)

**Problem:** `src/rt_pipeline/` defines sensor→ML→actuator pipeline but no executor.

| # | Task | Verification |
|---|------|-------------|
| G6.1 | Add `pipeline_create/pipeline_add_stage/pipeline_run` builtins | `fj run` with pipeline stages executes in sequence |
| G6.2 | Example: `examples/rt_pipeline_demo.fj` — sensor read → inference → motor output | `fj run examples/rt_pipeline_demo.fj` |

### Gap 7: Accelerator Dispatch (1 task, L effort)

**Problem:** `src/accelerator/` classifies workloads but doesn't dispatch to GPU/NPU.

| # | Task | Verification |
|---|------|-------------|
| G7.1 | Wire `accelerate(fn)` builtin — classifies workload, dispatches to best device | `fj run` with `accelerate(matmul_fn)` prints "dispatched to GPU" or "dispatched to CPU" |

### Gap 8: Const/Hardening Finish (1 task, S effort)

**Problem:** ~20% of const support and hardening checks remain.

| # | Task | Verification |
|---|------|-------------|
| G8.1 | Wire `const_type_name(T)` and `const_field_names(T)` reflection builtins | `fj run` with `println(const_type_name(42))` prints "i64" |

---

## Priority Matrix

| Priority | Tasks | Why |
|----------|-------|-----|
| **HIGH** | G2 (LSP fixes), G1 (demos), G8 (const) | Quick wins, high user visibility |
| **MEDIUM** | G3 (debugger), G4 (build scripts), G5 (diffusion) | Strong value, moderate effort |
| **LOW** | G6 (rt_pipeline), G7 (accelerator) | High effort, niche use case |

---

## Execution Recommendation

**Option A: Integrate into V19 sessions**
- Add G1 (demos) + G2 (LSP) + G8 (const) to V19 Phase 5 — quick wins
- G3-G5 become V20
- G6-G7 become V21

**Option B: Dedicated V20 "Completeness"**
- V19 = macros + pattern match + async + tests (as planned)
- V20 = all 18 gap closure tasks (demos, debugger, build scripts, ML, pipeline, accelerator)

**Option C: Spread across V19-V21 by priority**
- V19 += HIGH priority gaps (G1, G2, G8) — 8 tasks
- V20 = MEDIUM priority gaps (G3, G4, G5) — 7 tasks
- V21 = LOW priority gaps (G6, G7) — 3 tasks

**Recommended: Option C** — spreads work evenly, highest impact first.

---

## After All Gaps Closed

| Metric | Current (V18) | After Gap Closure |
|--------|---------------|-------------------|
| Modules production | ~38/56 (68%) | ~48/56 (86%) |
| Modules simulation | 2 (rtos, iot) | 2 (by design) |
| Modules deferred | 2 (jit, plugin) | 2 (by design) |
| Modules framework | 14 | ~4 (const_bench, const_alloc, const_reflect, const_stdlib) |
| CLI commands | 27 production | 30+ production |
| Examples | 263 .fj | 275+ .fj |

---

*V06-V15 Gap Closure Plan — 2026-04-03*
*18 tasks, 8 modules. Every task has `fj run` verification.*
*Intentional simulation (rtos/iot) and deferrals (jit/plugin) documented honestly.*
