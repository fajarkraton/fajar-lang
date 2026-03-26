# GAP_ANALYSIS_V2.md — Honest Codebase Audit

> **Date:** 2026-03-26
> **Auditor:** Claude Opus 4.6 (automated code audit)
> **Scope:** Every module in src/ (292,000 LOC, 220+ files)
> **Method:** Agent-based code reading, grep verification, function body inspection
> **Purpose:** Identify gaps between plan documentation and actual implementation

---

## Executive Summary

The Fajar Lang codebase contains **~136,000 LOC of production-quality code** (47%) that is fully functional and verified. The core compiler (lexer, parser, analyzer, interpreter, Cranelift codegen, ML runtime) is **100% real**. However, several modules added in Plan V6-V7 are **framework code** — well-designed type definitions with passing tests, but lacking the external integrations (networking, FFI bindings, solver calls) needed to actually function.

**This document exists to ensure absolute integrity in our documentation.**

---

## Tier 1: Production Real (136,000 LOC)

These modules are fully implemented, tested, and functional today.

### Core Compiler Pipeline

| Module | Files | LOC | Tests | Status |
|--------|-------|-----|-------|--------|
| Lexer | `src/lexer/` (3 files) | 2,200 | 96 | 90+ token kinds, UTF-8, escape sequences, error recovery |
| Parser | `src/parser/` (5 files) | 4,800 | 195 | Pratt parser, 19 precedence levels, 25+ Expr variants |
| Analyzer | `src/analyzer/` (15 files) | 8,000 | 429 | Type inference, scope resolution, borrow checking, NLL CFG |
| Interpreter | `src/interpreter/` (6 files) | 6,000 | 177 | Tree-walking eval, checked arithmetic, closures, patterns |
| Bytecode VM | `src/vm/` (4 files) | 1,200 | 20 | 43 opcodes, stack-based, call frames, function dispatch |
| CLI | `src/main.rs` | 2,500 | — | run, repl, check, build, fmt, lsp, new, dump-tokens, dump-ast |

### Native Codegen

| Module | Files | LOC | Tests | Status |
|--------|-------|-----|-------|--------|
| Cranelift JIT/AOT | `src/codegen/cranelift/` (15 files) | 40,000 | 700+ | Real cranelift_codegen, JITModule, ObjectCompiler, monomorphization |
| Cranelift runtime_fns | `runtime_fns.rs` | 7,600 | — | 150+ extern "C" runtime functions (strings, tensors, async, closures) |
| Cranelift runtime_bare | `runtime_bare.rs` | 2,500 | 57 | Bare-metal memcpy/memset/UART/GPIO/I2C/NVMe simulation |
| Cranelift runtime_user | `runtime_user.rs` | 200 | — | User-mode syscall wrappers (x86_64) |
| LLVM Backend | `src/codegen/llvm/` (2 files) | 3,835 | 20 | Real inkwell integration, 108 LLVM API references |
| Linker Scripts | `src/codegen/linker.rs` | 3,690 | 9 | GNU ld for ARM64/RISC-V/x86_64, startup asm, vector tables |
| ABI | `src/codegen/abi.rs` | 66 | 1 | Cranelift function signatures, SystemV calling convention |
| Analysis | `src/codegen/analysis.rs` | 815 | — | Stack estimation, recursion detection, memory layout |
| Interop | `src/codegen/interop.rs` | 2,273 | — | C/Python/Wasm binding generation, SBOM |
| PGO | `src/codegen/pgo.rs` | 1,074 | — | Profile instrumentation, hot/cold classification, inlining hints |

### ML & Hardware Runtime

| Module | Files | LOC | Tests | Status |
|--------|-------|-----|-------|--------|
| ML Tensor | `src/runtime/ml/tensor.rs` | 500 | — | ndarray-backed, shape validation, broadcasting |
| Autograd | `src/runtime/ml/autograd.rs` | 150 | — | Tape-based reverse-mode, chain rule backward() |
| ML Ops | `src/runtime/ml/ops.rs` | 300 | — | matmul (ndarray::dot), transpose, reshape, broadcast |
| ML Layers | `src/runtime/ml/layers.rs` | 500+ | — | Dense, Conv2d, Attention, BatchNorm, Dropout, Embedding |
| ML Optimizers | `src/runtime/ml/optim.rs` | 300+ | — | SGD (momentum), Adam (lr) |
| ML Quantization | `src/runtime/ml/quantize.rs` + 4 files | 500+ | — | INT8, FP8, FP4, BF16, fixed-point |
| OS Runtime | `src/runtime/os/` (20 files) | 2,000 | 30 | Memory/IRQ/syscall/paging simulation (appropriate for hosted lang) |
| GPU Runtime | `src/runtime/gpu/` (8 files) | 2,358 | — | GpuDevice trait, CpuFallback, CUDA backend (real cuInit) |
| QNN NPU | `src/runtime/ml/npu/qnn.rs` | 500+ | 22 | Real QNN FFI via dlopen, tested on Q6A hardware |

### Hardware & Board Support

| Module | Files | LOC | Tests | Status |
|--------|-------|-----|-------|--------|
| Dragon Q6A BSP | `src/bsp/dragon_q6a/` (2 files) | 4,375 | 73 | Real Vulkan/ash (6 SPIR-V kernels), real QNN, verified on hardware |
| BSP Framework | `src/bsp/` (10 files) | 3,000 | 13 | 8 board impls (STM32/ESP32/RP2040/Q6A/Jetson), linker scripts |
| HW Detection | `src/hw/` (3 files) | 1,500 | 19 | CPU (CPUID/AVX-512/AMX), GPU (CUDA dlopen), NPU detection |
| ARM64 ASM | `src/codegen/aarch64_asm.rs` | 500+ | — | System register encoding, GIC, timer registers |

### Developer Tools

| Module | Files | LOC | Tests | Status |
|--------|-------|-----|-------|--------|
| Formatter | `src/formatter/` (3 files) | 1,827 | 59 | Full pretty-printer, TOML config, import sorting |
| LSP Server | `src/lsp/` (4 files) | 1,468 | 31 | tower-lsp, diagnostics, completion, DAP, DWARF |
| Debugger v1 | `src/debugger/` (5 files) | 1,500+ | 19 | Breakpoints, step modes, conditions, logpoints |
| Debugger v2 | `src/debugger_v2/` (5 files) | 1,500+ | — | Time-travel replay, reverse execution, state diff |
| Package Manager | `src/package/` (12 files) | 1,500+ | — | SemVer, registry, resolver, lock files, publish |
| Docgen | `src/docgen.rs` | 713 | — | AST extraction, HTML generation, markdown |
| Testing | `src/testing/` (2 files) | 3,500+ | — | Fuzz harness, conformance, regression, @test |
| Selfhost | `src/selfhost/` (5 files) | 500+ | — | 3-stage bootstrap, binary comparison |
| FFI (original) | `src/interpreter/ffi.rs` | 300+ | 13 | Real libloading, symbol lookup, C function calls |

### FajarOS (Fajar Lang Source)

| Module | Files | LOC | Status |
|--------|-------|-----|--------|
| Nova v1.4 kernel | `examples/fajaros_nova_kernel.fj` | 20,176 | Real x86_64 bare-metal OS, 757 @kernel fns, QEMU verified |
| Nova Aurora | `examples/nova_aurora_*.fj` (4 files) | 2,000 | Real SMP/USB/compositor/services |
| Nova Phoenix | `examples/nova_phoenix_*.fj` (5 files) | 5,118 | GUI/audio/persist/posix/net kernel extensions |

---

## Tier 2: Real Logic, Needs External Integration (18,000 LOC)

These modules contain **real algorithms and data structures** that work correctly in isolation, but lack the external integration layer (network sockets, FFI bindings, solver calls) to be fully functional.

| Module | LOC | What Works | What's Missing | Effort to Complete |
|--------|-----|-----------|----------------|-------------------|
| `src/lsp_v3/semantic.rs` | 800 | Semantic token encoding, delta positions | Symbol resolution from analyzer | ~4 hrs |
| `src/lsp_v3/refactoring.rs` | 800 | Rename validation, extract function codegen | AST-based captured variable analysis | ~4 hrs |
| `src/lsp_v3/diagnostics.rs` | 768 | Diagnostic suggestions, quick fixes | Integration with analyzer errors | ~2 hrs |
| `src/dependent/` | 2,529 | Type-level nat, const generics, shape system | Full HKT (kind system), GADTs | ~16 hrs |
| `src/compiler/incremental/` | 2,780 | Dependency graph, content hashing, topo sort | **Hook into main compiler pipeline** | ~8 hrs |
| `src/codegen/wasm/` | 2,921 | Wasm binary encoding, basic WASI | Upgrade to WASI Preview 2, component model | ~12 hrs |
| `src/stdlib_v3/crypto.rs` | 539 | base64, hex, constant_time_eq, JWT validation | **SHA-256/AES/RSA computation** (need RustCrypto) | ~8 hrs |
| `src/stdlib_v3/net.rs` | 774 | URL parser, HTTP builder, rate limiter | **TCP/UDP sockets** (need std::net/tokio) | ~8 hrs |
| `src/stdlib_v3/formats.rs` | 500 | JSON/TOML/YAML struct definitions | Actual parsing/serialization logic | ~6 hrs |
| `src/stdlib_v3/system.rs` | 500 | Path/env/process types | Real process spawning, file watching | ~4 hrs |
| `src/verify/tensor_verify.rs` | 500 | Shape constraints, matmul verification | SMT solver integration for proofs | ~8 hrs |
| `src/concurrency_v2/` | 500 | Actor definitions, supervision strategy | Real message-passing runtime | ~8 hrs |
| `src/rt_pipeline/` | 2,554 | Context bridge, latency scheduling | Real sensor I/O integration | ~4 hrs |
| `src/playground/` | 2,215 | Wasm config, sandbox, print capture | Actual wasm-pack build pipeline | ~4 hrs |

**Total effort to complete Tier 2:** ~96 hours

---

## Tier 3: Framework Only (8,200 LOC)

These modules contain **only type definitions and tests**. The tests pass because they test the type system, not actual functionality. These need to be either **rewritten with real implementations** or **honestly documented as design specifications**.

| Module | LOC | What Exists | What It Cannot Do | Effort to Implement |
|--------|-----|-------------|-------------------|-------------------|
| `src/distributed/` | 2,734 | ClusterNode, RPC structs, fault tolerance enums | **Cannot send a single network message** — 0 std::net/tokio references | ~20 hrs |
| `src/ffi_v2/cpp.rs` | 574 | CppType enum, Itanium name mangling | **Cannot parse any C++ header** — no libclang/clang-sys binding | ~16 hrs |
| `src/ffi_v2/python.rs` | 602 | PyValue enum, PyCall builder, venv paths | **Cannot call any Python function** — no pyo3/libpython | ~16 hrs |
| `src/verify/smt.rs` | 500 | SolverBackend enum, SmtResult, ProofCache | **Cannot solve any formula** — no z3-sys FFI | ~12 hrs |
| `src/profiler/` | 2,505 | Instrument/flamegraph/memory structs | **Cannot profile any program** — no perf/sampling integration | ~12 hrs |
| `src/codegen/ptx.rs` | ~1,000 | PTX instruction types, tensor core structs | **Cannot execute any GPU kernel** — no CUDA runtime | ~20 hrs |
| Plugin system | 0 | **Does not exist** | — | ~16 hrs |
| FajarOS Surya kernel | 0 | **Not in this repo** (separate fajar-os repo) | — | N/A (separate project) |

**Total effort to implement Tier 3:** ~112 hours

---

## Plan Accuracy Assessment

### Plans with 100% Accuracy (no gaps)

| Plan | Tasks | Verified |
|------|-------|---------|
| V1 Implementation (Month 1-6) | 506 | All core deliverables exist with real code |
| V03 Tasks (refactoring) | 739 | All module extractions verified |
| V04 Plan (enums, Drop, async) | 40 | Generic enums, RAII, Future/Poll all real |
| V05 Plan (test/doc/traits/iter) | 80 | @test, ///, dyn Trait, .iter()/.map() all real |

### Plans with Gaps

| Plan | Claimed | Actual Real | Gap | Affected Modules |
|------|---------|-------------|-----|-----------------|
| V06 "Dominance" | 560 tasks [x] | ~360 real | ~200 framework | FFI v2, Stdlib v3, Verify, LSP v3 partial |
| V07 "Ascendancy" | 680 tasks [x] | ~150 real | ~530 framework | Distributed, WASI, GPU, Types, Incremental, Cloud, Plugins, Surya |

### What "Framework Complete" Means

When a V06/V07 task is marked [x] but the module is framework-only, it means:
- The **type system design** is complete (enums, structs, traits defined)
- The **API surface** is designed (function signatures, error types)
- **Unit tests pass** (testing the type system and API contracts)
- The **architecture** is sound (module boundaries, error handling)

What it does NOT mean:
- The module can perform its intended function
- External dependencies are integrated
- The feature works end-to-end

---

## Root Cause Analysis

### Why Did This Happen?

1. **Velocity over depth**: V06/V07 had aggressive task counts (560 + 680 = 1,240 tasks). The natural approach was to design the architecture first and mark design tasks complete.

2. **Tests that test types, not behavior**: A test like `assert_eq!(ClusterNode::new("n1").id().0, 1)` passes because it tests struct construction, not distributed networking.

3. **No integration tests for external deps**: The CI runs `cargo test --lib` which tests internal logic. There are no tests that verify "can this module actually connect to a network?" or "does this actually call Z3?"

4. **Separate concerns conflated**: "Design the RPC protocol" and "Implement the RPC protocol over TCP" are different tasks but were treated as one.

---

## Corrective Actions

### Immediate: Documentation Honesty

This GAP_ANALYSIS_V2.md serves as the correction. All plans remain as historical records, but this document provides the honest assessment.

### Strategic: V8 Plan Revision

Plan V8 "Dominion" should prioritize converting Tier 2 and Tier 3 modules into real implementations. The revised V8 plan addresses this directly.

### Process: Prevent Recurrence

1. **Task definition rule**: Every task must specify its **verification method** (unit test, integration test, CLI command, QEMU boot, hardware test)
2. **"Framework" vs "Implementation" distinction**: If a task only creates types/traits, label it "[f]" (framework). Only mark "[x]" when the feature works end-to-end.
3. **Integration test requirement**: Every module that depends on external systems must have at least one integration test that verifies the external connection.
4. **No stub plans**: Every plan option must have detailed task tables (already enforced — see `memory/feedback_complete_plans.md`)

---

## Module-Level Recommendations for V8

### Priority 1: Complete Tier 2 (High value, low effort)

These modules have 60-80% of the work done. Completing them gives the most return:

1. **Incremental compilation** → Hook cache into main compiler (8 hrs)
2. **stdlib_v3/crypto** → Add RustCrypto dependency for SHA/AES/RSA (8 hrs)
3. **stdlib_v3/net** → Add std::net TCP/UDP + basic HTTP client (8 hrs)
4. **LSP v3** → Connect semantic/refactoring to analyzer symbol table (10 hrs)
5. **Wasm** → Upgrade to WASI Preview 2 interfaces (12 hrs)

### Priority 2: Implement Tier 3 Core (High value, medium effort)

These are the most impactful missing features:

1. **Distributed runtime** → Add tokio TCP transport, real actor mailboxes (20 hrs)
2. **FFI v2 C++** → Add clang-sys for header parsing (16 hrs)
3. **FFI v2 Python** → Add pyo3 for CPython embedding (16 hrs)
4. **SMT verification** → Add z3-sys for formula solving (12 hrs)

### Priority 3: New Capabilities (V8 original scope)

After fixing gaps, proceed with V8 options:
1. Self-hosting compiler
2. Package registry
3. IDE experience
4. Application templates

---

*This document will be updated as gaps are closed. Each module's status should be re-verified before marking complete.*
