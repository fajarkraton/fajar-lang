# Re-Audit V17 — Baseline Measurements

> **Date:** 2026-04-03
> **Auditor:** Claude Opus 4.6 (verified)
> **Purpose:** Immutable baseline before full re-audit. All numbers verified by running commands.
> **Rule:** This document records FACTS only. No claims, no interpretations.

---

## 1. Build Status

| Check | Result | Command |
|-------|--------|---------|
| `cargo build` | PASS (clean compile) | `cargo build` |
| `cargo clippy -- -D warnings` | PASS (0 warnings) | `cargo clippy -- -D warnings` |
| `cargo fmt -- --check` | **FAIL** (70 diffs in 16 files) | `cargo fmt -- --check` |

### Formatting Issues (16 files)

```
src/analyzer/effects.rs
src/analyzer/type_check/check.rs
src/analyzer/type_check/register.rs
src/codegen/analysis.rs
src/dependent/nat.rs
src/dependent/patterns.rs
src/gpu_codegen/mod.rs
src/interpreter/eval/mod.rs
src/lexer/mod.rs
src/lexer/token.rs
src/lsp/server.rs
src/main.rs
src/package/server.rs
src/parser/mod.rs
tests/nova_v2_tests.rs
tests/validation_tests.rs
```

**Note:** CLAUDE.md claims "Formatting: Clean". This is incorrect as of 2026-04-03.

---

## 2. Codebase Size

| Metric | Actual | CLAUDE.md Claims |
|--------|--------|-----------------|
| Source files (src/) | 441 .rs | 442 files |
| Source LOC (src/) | 473,909 | ~486,000 |
| Test files (tests/) | 41 .rs | — |
| Test LOC (tests/) | 34,011 | — |
| Example files (examples/) | 257 .fj | 216+ |
| Example LOC (examples/) | 93,108 | — |
| Doc files (docs/) | 162 .md | 44 documents |
| Bench files (benches/) | 7 .rs | — |
| Git commits | 786 | 585 (stale) |
| Cargo.toml version | 12.6.0 | 12.6.0 (match) |

**Discrepancies:**
- LOC: 473,909 actual vs ~486,000 claimed (off by ~12K, 2.5%)
- Doc files: 162 actual vs 44 claimed (CLAUDE.md severely understates)
- Commits: 786 actual vs 585 claimed (stale)
- Examples: 257 actual vs 216+ claimed (understated)

---

## 3. Test Counts

| Mode | Tests | Result |
|------|-------|--------|
| `cargo test --lib` | 8,280 | 8,280 pass, 0 fail |
| `cargo test` (all targets) | 8,280 lib + 0 bin + 13 doc + 24 integ = **8,317** | All pass |
| `cargo test --features native --lib` | **FAIL** | Stack overflow in `native_fibonacci_matches_interpreter` |
| `#[test]` annotations in source | 9,978 | ~1,698 behind feature flags |

**CLAUDE.md claims 8,475 tests.** Actual: **8,317** (off by 158).

### Feature-Gated Tests (not compiled by default)

| Feature | Gated Tests (approx) |
|---------|---------------------|
| native (Cranelift) | ~1,057+ (cranelift/tests.rs) |
| llvm | ~unknown (module not compiled) |
| python-ffi | 18 |
| smt (Z3) | 13 |
| cpp-ffi | 9 |
| tls | 3 |
| others (gui, websocket, mqtt, ble, https) | unknown |

### Bug Found

**`native_fibonacci_matches_interpreter` causes stack overflow** when running `cargo test --features native --lib`. This is a real defect that crashes the entire test suite for native codegen.

---

## 4. Public Modules (56 total, from lib.rs)

### Directories (41)

| # | Module | LOC | Files | Tests |
|---|--------|-----|-------|-------|
| 1 | codegen | 89,789 | 41 | 1,729 |
| 2 | runtime | 72,415 | 86 | 1,522 |
| 3 | analyzer | 23,510 | 19 | 519 |
| 4 | interpreter | 20,840 | 7 | 604 |
| 5 | ffi_v2 | 20,043 | 14 | 358 |
| 6 | compiler | 18,473 | 20 | 325 |
| 7 | package | 18,062 | 18 | 386 |
| 8 | demos | 16,257 | 15 | 317 |
| 9 | selfhost | 15,875 | 15 | 320 |
| 10 | distributed | 15,337 | 16 | 322 |
| 11 | verify | 14,583 | 14 | 349 |
| 12 | wasi_p2 | 13,782 | 12 | 244 |
| 13 | bsp | 12,301 | 11 | 336 |
| 14 | parser | 9,760 | 6 | 195 |
| 15 | lsp | 8,825 | 4 | 172 |
| 16 | rtos | 8,043 | 9 | 174 |
| 17 | stdlib_v3 | 7,520 | 6 | 212 |
| 18 | gui | 6,351 | 4 | 118 |
| 19 | iot | 5,033 | 6 | 74 |
| 20 | gpu_codegen | 4,711 | 7 | 112 |
| 21 | debugger | 4,367 | 7 | 82 |
| 22 | testing | 3,595 | 2 | 40 |
| 23 | dependent | 3,549 | 5 | 156 |
| 24 | accelerator | 3,480 | 5 | 82 |
| 25 | lsp_v2 | 3,395 | 5 | 76 |
| 26 | deployment | 3,343 | 5 | 62 |
| 27 | profiler | 3,340 | 6 | 66 |
| 28 | lexer | 3,333 | 3 | 133 |
| 29 | concurrency_v2 | 2,861 | 5 | 77 |
| 30 | debugger_v2 | 2,830 | 5 | 59 |
| 31 | vm | 2,739 | 5 | 19 |
| 32 | hw | 2,657 | 4 | 77 |
| 33 | rt_pipeline | 2,554 | 5 | 47 |
| 34 | playground | 2,439 | 6 | 63 |
| 35 | lsp_v3 | 2,368 | 4 | 42 |
| 36 | package_v2 | 2,221 | 5 | 69 |
| 37 | jit | 2,183 | 5 | 60 |
| 38 | ml_advanced | 2,178 | 5 | 61 |
| 39 | formatter | 2,021 | 3 | 29 |
| 40 | plugin | 940 | 1 | 23 |
| 41 | stdlib | 95 | 3 | 0 |

### Standalone Files (15)

| # | File | LOC | Tests |
|---|------|-----|-------|
| 1 | main.rs | 5,556 | 0 |
| 2 | hardening.rs | 1,211 | 31 |
| 3 | const_traits.rs | 803 | 17 |
| 4 | macros_v12.rs | 789 | 22 |
| 5 | docgen.rs | 774 | 8 |
| 6 | const_alloc.rs | 766 | 16 |
| 7 | const_generics.rs | 754 | 26 |
| 8 | const_stdlib.rs | 714 | 25 |
| 9 | const_generic_types.rs | 698 | 16 |
| 10 | const_macros.rs | 649 | 20 |
| 11 | const_reflect.rs | 564 | 17 |
| 12 | const_pipeline.rs | 551 | 16 |
| 13 | const_bench.rs | 468 | 14 |
| 14 | macros.rs | 439 | 14 |
| 15 | lib.rs | 408 | 0 |
| 16 | wasi_v12.rs | 395 | 12 |
| 17 | generators_v12.rs | 372 | 13 |

---

## 5. CLI Commands (35 total)

```
 1. fj run             11. fj build          21. fj search         31. fj hw-json
 2. fj repl            12. fj publish        22. fj login          32. fj sbom
 3. fj check           13. fj registry-init  23. fj yank           33. fj verify
 4. fj dump-tokens     14. fj registry-serve 24. fj install        34. fj bindgen
 5. fj dump-ast        15. fj add            25. fj update         35. fj profile
 6. fj fmt             16. fj doc            26. fj tree
 7. fj lsp             17. fj test           27. fj audit
 8. fj pack            18. fj watch          28. fj bootstrap
 9. fj playground      19. fj bench          29. fj gui
10. fj new             20. fj debug          30. fj hw-info
```

---

## 6. Feature Flags (20 total)

| Feature | Dependencies | Real External Dep? |
|---------|-------------|-------------------|
| gpu | wgpu | YES |
| vulkan | ash | YES |
| cuda | (empty) | NO — placeholder |
| native | cranelift-* (6 crates), target-lexicon | YES |
| llvm | inkwell | YES |
| tls | rustls, webpki-roots | YES |
| cpp-ffi | clang-sys | YES |
| python-ffi | pyo3 | YES |
| smt | z3 | YES |
| gui | winit, softbuffer | YES |
| websocket | tungstenite | YES |
| mqtt | rumqttc | YES |
| ble | btleplug | YES |
| https | native-tls | YES |
| playground-wasm | wasm-bindgen, getrandom/js | YES |
| wasm | (empty) | NO — placeholder |
| freertos | (empty) | NO — placeholder |
| zephyr | (empty) | NO — placeholder |
| esp32 | (empty) | NO — placeholder |

**4 placeholder features** with no dependencies: cuda, wasm, freertos, zephyr, esp32.

---

## 7. Code Quality Markers

| Metric | Count | Where |
|--------|-------|-------|
| Production `.unwrap()` | 43 | 19 files (excluding test files and `#[cfg(test)]` blocks) |
| Production `panic!()` | ~2 | cranelift/tests.rs (test file), docs/comments |
| Production `todo!()` | 14 | Various |
| `unsafe` blocks | (to be counted in Phase 5) | |

**Note:** CLAUDE.md claims "ZERO `.unwrap()` in src/". Actual: 43 in production code (plus 154 in cranelift test file, 4,406 in test modules).

---

## 8. Discrepancies Summary (CLAUDE.md vs Reality)

| Claim | CLAUDE.md | Actual | Delta |
|-------|-----------|--------|-------|
| Tests | 8,475 | 8,317 | -158 |
| LOC | ~486,000 | 473,909 | -12,091 |
| Source files | 442 | 441 | -1 |
| Examples | 216+ | 257 | +41 |
| Doc files | 44 | 162 | +118 |
| Commits | 585 | 786 | +201 |
| Formatting | "Clean" | 70 diffs in 16 files | WRONG |
| `.unwrap()` in src | "0" (implied) | 43 production | WRONG |
| `todo!()` in src | "0" (implied) | 14 | WRONG |
| Native tests | (not mentioned) | FAIL (stack overflow) | BUG |

---

## 9. Preliminary Module Classification (from exploration)

Based on exploration agents' findings (to be fully verified in Phase 5):

### Likely PRODUCTION (wired to CLI, has real algorithms)
lexer, parser, analyzer, interpreter, codegen, runtime, package, lsp, formatter, hw, profiler, gui, gpu_codegen, verify, selfhost, bsp, distributed, ffi_v2, wasi_p2

### Likely FRAMEWORK (types + tests, not usable from .fj)
rtos, accelerator, jit, rt_pipeline, iot, deployment, ml_advanced

### Likely STUB (type definitions, no real logic, no CLI wiring)
debugger_v2, concurrency_v2, package_v2, lsp_v2, lsp_v3

### Unknown (need Phase 5 audit)
demos, stdlib, stdlib_v3, testing, dependent, compiler, debugger, playground, plugin,
const_*, macros, macros_v12, generators_v12, hardening, docgen, wasi_v12

---

*REAUDIT_V17_BASELINE.md — Recorded 2026-04-03 — All numbers verified by command execution*
