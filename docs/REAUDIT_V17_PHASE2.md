# Re-Audit V17 — Phase 2: Native Codegen

> **Date:** 2026-04-03
> **Scope:** codegen/cranelift, codegen/llvm, codegen supporting modules

---

## Summary

| Component | Status | Evidence |
|-----------|--------|----------|
| Cranelift JIT (`fj run --native`) | **[p] PARTIAL** | Numeric computation works, strings broken |
| Cranelift AOT (`fj build`) | **[p] PARTIAL** | Produces ELF binary, but runtime linking fails for println |
| LLVM backend (`fj run --llvm`) | **[s] BROKEN** | CE009 error, tests don't compile (33 errors) |
| runtime_fns.rs (442 extern C fns) | **[x] PRODUCTION** | Real implementations bridging JIT to Rust stdlib |
| runtime_bare.rs (160 extern C fns) | **[f] FRAMEWORK** | Bare-metal stubs, needs hardware |
| opt_passes.rs (56 tests) | **[x] PRODUCTION** | Real optimizations, wired into main.rs |
| aarch64_asm.rs (27 tests) | **[x] PRODUCTION** | Real ARM64 instruction encoding |
| linker.rs (~60 tests) | **[x] PRODUCTION** | Real linker script generation, used by fj build |
| security.rs (72 tests) | **[f] FRAMEWORK** | Data structures only, no hardening logic |
| pgo.rs (20 tests) | **[f] FRAMEWORK** | Data structures, real PGO would need LLVM working |
| ptx.rs (46 tests) | **[f] FRAMEWORK** | PTX string generation, no GPU compilation |
| amx/avx10/avx512 (72 tests) | **[f] FRAMEWORK** | Instruction encoding tables, not integrated |

---

## Cranelift JIT — [p] PARTIAL

### What WORKS:
- Integer arithmetic, recursion: `fib(20) = 6765` ✅
- Array operations, for loops: `sum(1..100) = 5050` ✅
- Struct field access: `p.x + p.y = 30` ✅
- Function calls, closures ✅
- 1,119 tests pass (skipping 1 stack overflow)

### What's BROKEN:
- **String handling in JIT:** f-string interpolation outputs raw pointers (`134618166626800` instead of `"fib(20) = 6765"`)
- **String from match:** `match c { Color::Red => "red" }` outputs pointer, not string
- **Stack overflow:** `native_fibonacci_matches_interpreter` test causes SIGABRT
- **AOT linking:** `fj build` with println → undefined reference to `fj_rt_println_str`

### Root Cause (AOT linking):
AOT compilation generates a `.o` file and links with `cc`. But runtime functions (fj_rt_*) are defined in Rust code, not in a separate `.a` library. JIT mode works because functions are registered in the Cranelift JIT module's symbol table. AOT mode would need a separate runtime library to link against.

---

## LLVM Backend — [s] BROKEN

- `fj run --llvm examples/hello.fj` → CE009 (void return type mismatch)
- `cargo test --features llvm --lib codegen::llvm` → **33 compilation errors** 
- Missing struct fields: `effect_row_var`, others added to AST after LLVM code was written
- The LLVM backend is **completely out of sync** with the current AST

---

## Codegen Submodule Detail

### opt_passes.rs — [x] PRODUCTION
- Constant folding: `2 + 3` → `5` (tested on parsed AST)
- Loop unrolling detection, strength reduction (`x * 2` → shift)
- Dead code elimination
- **Wired into main.rs** via `OptPipeline::new().run(&program)`

### aarch64_asm.rs — [x] PRODUCTION
- Real ARM64 instruction encoding (ISB, MOVZ, MRS, LDR etc.)
- Tests verify exact 32-bit opcode values
- Used by linker.rs for bare-metal startup code

### linker.rs — [x] PRODUCTION
- Generates real GNU ld linker scripts (MEMORY, SECTIONS, ENTRY)
- Generates startup code for x86_64/AArch64/RISC-V
- **Wired into main.rs** via `board.generate_linker_script()`

### security.rs — [f] FRAMEWORK
- StackCanaryConfig, AllocationBudget, BoundsCheckMode defined
- Canary call site wired in cranelift/mod.rs but actual __canary_generate is a stub
- 72 tests all test data structures, not actual hardening

### pgo.rs — [f] FRAMEWORK
- PgoMode, PgoConfig, BranchWeight structs
- Counter arithmetic works but no actual instrumentation
- Depends on LLVM backend which is broken

### ptx.rs — [f] FRAMEWORK
- PtxEmitter generates PTX assembly strings
- Tests verify string content (".version 8.6", "tcgen05.mma")
- No integration with `fj build` or NVIDIA toolchain

### amx.rs / avx10.rs / avx512.rs — [f] FRAMEWORK
- Instruction encoding lookup tables (72 tests combined)
- Not integrated into actual code generation
- Would need LLVM or inline assembly support

---

## Bugs Found in Phase 2

| # | Bug | Severity | Evidence |
|---|-----|----------|----------|
| 1 | **JIT string handling broken** | HIGH | f-strings and match strings output raw pointers |
| 2 | **LLVM backend out of sync** | HIGH | 33 compile errors, CE009 runtime error |
| 3 | **AOT linking fails for runtime fns** | MEDIUM | `fj build` with println → undefined reference |
| 4 | **Stack overflow in native test** | MEDIUM | native_fibonacci_matches_interpreter → SIGABRT |

---

## Test Counts

| Module | Tests | Quality |
|--------|-------|---------|
| cranelift (JIT+AOT) | 1,119 pass (+1 crash) | GENUINE — compile-and-run tests |
| LLVM | 0 (can't compile) | BROKEN |
| security | 72 | STRUCTURAL — data structure tests |
| opt_passes | 56 | BEHAVIORAL — real optimization |
| ptx | 46 | SHALLOW — string format checks |
| aarch64_asm | 27 | BEHAVIORAL — real opcode verification |
| amx+avx10+avx512 | 72 | STRUCTURAL — encoding tables |
| pgo | 20 | STRUCTURAL — config structs |
| analysis | 15 | STRUCTURAL |
| optimizer | 22 | MIXED |
| perf_report | 10 | STRUCTURAL |
| nostd | 10 | STRUCTURAL |
| benchmarks | 9 | STRUCTURAL |
| interop | 40 | MIXED |

**Total codegen tests:** 1,518 (pass) + 1 (crash) + LLVM (broken)

---

## Phase 2 Conclusion

Cranelift JIT is a **real, working native compiler** for numeric computation. It produces correct results for integers, floats, structs, arrays, loops, and recursion. **1,119 tests verify this.**

However, **string handling in JIT is broken** (outputs pointers), **AOT linking is incomplete** (missing runtime library), and the **LLVM backend is completely broken** (won't compile).

**Overall verdict:** Cranelift [p] PARTIAL, LLVM [s] BROKEN, supporting modules mixed [x]/[f].

---

*Phase 2 complete — 2026-04-03*
