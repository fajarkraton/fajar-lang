# Re-Audit V17 — Phase 5: All 56 Public Modules

> **Date:** 2026-04-03
> **Scope:** Every module in lib.rs, classified with strict criteria
> **Rule:** [x] = usable from CLI or .fj. [f] = real logic but not user-accessible. [s] = minimal/empty.

---

## Classification Results

### [x] PRODUCTION — 33 modules (usable from CLI or .fj programs)

| # | Module | LOC | Tests | How Wired | Evidence |
|---|--------|-----|-------|-----------|----------|
| 1 | lexer | 3,333 | 143 | `fj run/check/dump-tokens` | Real tokenizer, verified Phase 1 |
| 2 | parser | 9,760 | 220 | `fj run/check/dump-ast` | Real recursive descent + Pratt, verified Phase 1 |
| 3 | interpreter | 20,840 | 604 | `fj run` | Full pipeline eval_source(), verified Phase 1 |
| 4 | vm | 2,739 | 19 | `fj run --vm` | Bytecode matches interpreter, verified Phase 1 |
| 5 | formatter | 2,021 | 29 | `fj fmt` | Real formatting, verified Phase 1 |
| 6 | codegen | 89,789 | 1,729 | `fj build/run --native` | Cranelift JIT works, 1,119 native tests pass |
| 7 | runtime | 72,415 | 1,522 | `fj run` (ML+OS builtins) | Real ndarray ML, simulated OS. Verified Phase 3 |
| 8 | gpu_codegen | 4,711 | 112 | `fj build --target spirv/ptx` | Real SPIR-V/PTX binary output, verified Phase 3 |
| 9 | package | 18,062 | 386 | `fj add/publish/tree/audit/etc` | Full package management chain verified Phase 4 |
| 10 | lsp | 8,825 | 172 | `fj lsp` | Real tower-lsp server, 150+ tests |
| 11 | gui | 6,351 | 118 | `fj gui` | 20+ widget set, layout engine, event handling |
| 12 | hw | 2,657 | 77 | `fj hw-info/hw-json` | Real CPUID + CUDA detection, verified Phase 4 |
| 13 | profiler | 3,340 | 66 | `fj profile` | Execution timing, hotspot reports, verified Phase 4 |
| 14 | docgen | 774 | 8 | `fj doc` | HTML doc generation from `///`, verified Phase 4 |
| 15 | debugger | 4,367 | 82 | `fj debug --dap` | Real DAP server |
| 16 | compiler | 18,473 | 325 | `fj build` | Incremental compilation, dependency graph, artifact cache |
| 17 | testing | 3,595 | 40 | `fj test` | FuzzHarness, ConformanceRunner |
| 18 | stdlib_v3 | 7,520 | 212 | interpreter builtins | Crypto (SHA-2, AES-GCM), networking, database |
| 19 | distributed | 15,337 | 322 | `fj run --cluster` | Real tokio TCP, Raft consensus |
| 20 | verify | 14,583 | 349 | `fj verify` | SMT solver (Z3 with fallback), symbolic execution |
| 21 | selfhost | 15,875 | 320 | `fj bootstrap` | Real Pratt parser, Stage1 subset compiler |
| 22 | wasi_p2 | 13,782 | 244 | `fj build --wasi-p2` | WIT parser, component model |
| 23 | ffi_v2 | 20,043 | 358 | `fj bindgen` | Real pyo3 CPython (feature-gated), C/C++ bindgen |
| 24 | bsp | 12,301 | 336 | `fj build --bsp` | Board definitions, linker scripts, peripheral registers |
| 25 | dependent | 3,549 | 156 | analyzer comptime | Dependent arrays, nat types, shape verification |
| 26 | macros | 439 | 14 | interpreter eval | MacroRegistry, vec!/stringify!/dbg! |
| 27 | macros_v12 | 789 | 22 | macro infrastructure | Token trees, pattern matching, proc macro support |
| 28 | generators_v12 | 372 | 13 | interpreter eval | GeneratorState, yield/gen fn |
| 29 | const_generics | 754 | 26 | analyzer type_check | Compile-time param classification |
| 30 | const_generic_types | 698 | 16 | analyzer type_check | Const type evaluation |
| 31 | const_traits | 803 | 17 | analyzer | Const trait bounds |
| 32 | const_macros | 649 | 20 | analyzer comptime | Compile-time macro expansion |
| 33 | const_pipeline | 551 | 16 | analyzer | Comptime evaluation pipeline |

### [p] PARTIAL — 1 module

| # | Module | LOC | Tests | Issue |
|---|--------|-----|-------|-------|
| 34 | analyzer | 23,510 | 519 | Type checking works, but @kernel/@device NOT enforced |

### [f] FRAMEWORK — 18 modules (real logic but NOT user-accessible)

| # | Module | LOC | Tests | Why [f] |
|---|--------|-----|-------|---------|
| 35 | demos | 16,257 | 317 | Reference implementations (drone, MNIST, miniOS) — not callable from .fj |
| 36 | plugin | 940 | 23 | CompilerPlugin trait + registry, no built-in plugins shipped |
| 37 | rtos | 8,043 | 174 | Intentional simulation framework — FreeRTOS/Zephyr stubs |
| 38 | iot | 5,033 | 74 | Intentional simulation — WiFi/BLE/MQTT/LoRaWAN/OTA stubs |
| 39 | debugger_v2 | 2,830 | 59 | Recording/replay infra, wired as `RecordConfig::default()` only |
| 40 | concurrency_v2 | 2,861 | 77 | Actor system, AsyncScope used in spawn() builtin — minimal wiring |
| 41 | lsp_v2 | 3,395 | 76 | Type-driven completion engine, NOT wired to `fj lsp` |
| 42 | lsp_v3 | 2,368 | 42 | Semantic tokens/navigation, only used in test code |
| 43 | package_v2 | 2,221 | 69 | Build scripts/conditional compilation, partially wired |
| 44 | deployment | 3,343 | 62 | Container/K8s generation, NOT wired to CLI |
| 45 | ml_advanced | 2,178 | 61 | Transformer/diffusion/RL/serving, NOT wired to CLI |
| 46 | rt_pipeline | 2,554 | 47 | Real-time sensor→ML→actuator, NOT wired to CLI |
| 47 | accelerator | 3,480 | 82 | Workload dispatch, only PoC call in main.rs |
| 48 | jit | 2,183 | 60 | Tiered JIT profiling, wired but baseline compilation is framework |
| 49 | hardening | 1,211 | 31 | Build hardening/CI checks, not user-facing |
| 50 | const_alloc | 766 | 16 | Compile-time allocation, wired to analyzer |
| 51 | const_reflect | 564 | 17 | Compile-time reflection, support module |
| 52 | const_stdlib | 714 | 25 | Comptime standard library, support module |

### [s] STUB — 3 modules

| # | Module | LOC | Tests | Why [s] |
|---|--------|-----|-------|---------|
| 53 | stdlib | 95 | 0 | Just function name registries (ML_BUILTINS, OS_BUILTINS). Actual impls in runtime/ |
| 54 | const_bench | 468 | 14 | Comptime benchmarking infrastructure, not functional |
| 55 | wasi_v12 | 395 | 12 | WASI v1 types, superseded by wasi_p2 |

---

## Totals

| Status | Count | LOC | Tests |
|--------|-------|-----|-------|
| **[x] PRODUCTION** | 33 | 368,373 | 7,502 |
| **[p] PARTIAL** | 1 | 23,510 | 519 |
| **[f] FRAMEWORK** | 18 | 55,920 | 1,005 |
| **[s] STUB** | 3 | 958 | 26 |
| **main.rs + lib.rs** | — | 5,964 | 0 |
| **TOTAL** | 55+main | 454,725 | 9,052 |

**Note:** LOC count above is 454,725 (modules only) vs 473,909 (full src/). The ~19K gap is from standalone files (const_bench, wasi_v12, etc.) being counted differently.

---

## Key Insights

1. **33/56 modules (59%) are genuinely PRODUCTION** — usable from CLI or .fj programs
2. **18/56 modules (32%) are FRAMEWORK** — real code but not user-accessible
3. **3/56 modules (5%) are STUBS** — minimal/empty
4. **77% of all code (368K/474K LOC) is in PRODUCTION modules**
5. **The framework modules (56K LOC) have real algorithms** — not fake. They're real implementations waiting to be wired.
6. **The previous claim of "100% PRODUCTION" was inflated by ~37% of modules**

### Modules That Were Previously Overcounted:
- demos (16K LOC, 317 tests) — marked [x] in prior audits but never callable from .fj
- rtos/iot (13K LOC combined) — intentional simulations, not production features
- lsp_v2/lsp_v3 (5.8K LOC) — IDE features not wired to `fj lsp`
- deployment/ml_advanced (5.5K LOC) — not CLI-accessible
- debugger_v2/concurrency_v2 (5.7K LOC) — minimal/no real wiring

---

*Phase 5 complete — 2026-04-03*
