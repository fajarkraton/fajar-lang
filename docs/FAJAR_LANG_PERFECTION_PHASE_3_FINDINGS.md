---
phase: FAJAR_LANG_PERFECTION P3 — feature-gate matrix audit
status: CLOSED 2026-05-02
budget: ~2h actual (est 12-16h, -85%; +50% surprise = 24h cap, far under)
plan: docs/FAJAR_LANG_PERFECTION_PLAN.md §3 P3 + §4 P3 PASS criteria
---

# Phase 3 Findings — Feature-Gate Matrix Audit

## Summary

P3 closed in ~2h vs 12-16h estimate (-85% under). Three sub-items
delivered:

| # | Item | Effort | Status |
|---|---|---|---|
| W | Fix --features wasm structural bugs | 30min | ✅ |
| PW | Fix --features playground-wasm API drift | 15min | ✅ |
| CI | Feature-matrix CI gate extension | 30min | ✅ |

Plus pre-flight verification + closeout = ~30min.

## Why effort was -85% under

- P1.A3-fix2 commit (`b63f6d76`) had already fixed clippy across 16/18
  features, leaving only wasm + playground-wasm as P3 work.
- wasm structural bugs were stale-code maintenance, not novel bugs —
  AST schema drift on FnDef + Stmt + Expr field additions since the
  test fixtures were last updated.
- playground-wasm API drift was 1 function call (`PrettyPrinter::new()`
  → `formatter::format()`).
- CI gate extension is mechanical YAML edit.

## Per-item detail

### P3.W — wasm structural bug fixes (5 errors closed)

`src/codegen/wasm/mod.rs` test mod had stale AST literals not updated
when fields were added to ast types:

| Error | Field gap | Fix |
|---|---|---|
| E0063 FnDef | missing `effect_row_var`, `is_gen`, `no_inline` | added 3 fields with defaults |
| E0063 Stmt::Let (3×) | missing `linear` | added `linear: false` to all 3 literals |
| Invalid `label: _` syntax | While needs Option<String>, not match-pattern | replaced with `label: None` |
| Missing Default WasmFuncBody | clippy E0309 | added `impl Default` |
| Missing Default WasmModule | clippy E0309 | added `impl Default` |
| Unused `float_lit`/`bool_lit`/`string_lit` test helpers | dead_code under -D warnings | added `#[allow(dead_code)]` per fn |
| approx_constant 3.14 (2×) | clippy approx | replaced with 1.25 (test semantics unchanged) |

10 fixes in single file, all in `#[cfg(test)] mod tests`.

### P3.PW — playground-wasm API drift fix (1 error + 1 collateral)

`src/playground/wasm_api.rs:89` referenced `crate::formatter::PrettyPrinter::new()`
which doesn't exist. Current formatter API is `formatter::format(source: &str) -> Result<String, _>`.

Replaced PrettyPrinter usage with `crate::formatter::format(code).unwrap_or_else(|_| code.to_string())`.

Collateral: `tests/feature_flag_tests.rs:264` had `assert!(true)` which
clippy rejects under `-D warnings` for the playground-wasm feature
build path. Replaced with documented compile-only smoke (no assertion).

### P3.CI — feature-matrix CI gate (extension to .github/workflows/ci.yml)

CI feature matrix expanded from 4 features (llvm, smt, cpp-ffi, python-ffi)
to **20 features**:

```
Pre-existing (with apt deps): llvm, smt, cpp-ffi, python-ffi
Added in P3.CI:               native, gui, cuda, gpu, vulkan, tls,
                              https, websocket, mqtt, ble, wasm,
                              playground-wasm, freertos, zephyr, esp32
```

Plus extended each feature job to run BOTH `cargo clippy --features X`
AND `cargo clippy --tests --features X -- -D warnings`. Closes the
long-standing gap where feature-gated test paths accumulated lint debt
invisibly.

System deps added per feature where needed:
- gui: libxkbcommon-dev, libwayland-dev
- ble: libdbus-1-dev
- All others: pure Cargo (no apt/pip)

## Quality gates (full feature matrix)

```
cargo clippy --tests --release [--features X] -- -D warnings → EXIT=0 for ALL of:
  default, gpu, vulkan, cuda, native, llvm, tls, cpp-ffi, python-ffi,
  smt, gui, websocket, mqtt, ble, https, playground-wasm, wasm,
  freertos, zephyr, esp32

= 20/20 features clippy-clean (was 16/18 post-P1.A3-fix2; 18/18 +2 new = 20)
```

Default-feature gates:
```
cargo test --lib --release       → 7,626 PASS, 0 fail
cargo test --test '*' --release  → 2,575 PASS, 0 fail
cargo clippy --tests --release -- -D warnings → EXIT=0
cargo fmt -- --check             → EXIT=0
```

## What was learned

1. **Stale AST literals in test fixtures** are a recurring class of bug.
   When AST struct gets new fields, their `#[derive(...)]` generates
   new code, but hand-written `Item::FnDef(FnDef { ... })` test
   literals don't auto-update. The `tests/llvm_e2e_tests.rs` fix in P1
   followup F4 was the same class. Future class-prevention idea:
   `Default::default()` + spread for test fixtures, OR generate
   fixtures via macro.

2. **CI matrix gaps surface real bugs.** `wasm` had been broken for
   multiple releases (per P1 audit findings), only caught by the
   manual matrix audit because CI didn't include wasm in the
   feature-test job. Now it does.

3. **Effort estimates inflated when scope is "audit existing".**
   12-16h estimate assumed novel-test-writing per feature; actual
   work was 80% mechanical fixes + 20% CI YAML.

## Self-check (CLAUDE.md §6.8)

| Rule | Status |
|---|---|
| §6.8 R1 pre-flight audit | YES — feature matrix executed live |
| §6.8 R2 verification = runnable commands | YES — full matrix shown |
| §6.8 R3 prevention layer | YES — CI matrix expansion (16 new feature jobs) |
| §6.8 R4 numbers cross-checked | YES — every feature's 0-error verified live |
| §6.8 R5 surprise budget | YES — under cap (-85% vs estimate) |
| §6.8 R6 mechanical decision gates | YES — all features must pass clippy --tests |
| §6.8 R7 public-artifact sync | partial — CHANGELOG entry deferred to P9 |
| §6.8 R8 multi-repo state check | YES — fajar-lang only |

7/8 fully + 1 partial.

## Onward to P4

P4 = Soundness probes (C1+C2+C3). Estimated 30-50h, +50% surprise = 75h cap.
Largest remaining phase. Will:
- C1 Borrow checker (polonius) soundness expansion — ≥10 new property tests
- C2 Type system soundness — negative tests for ALL 78+ error codes
- C3 Memory safety probes — fuzz suite extended +3 targets minimum

P3 surfaced that AST schema drift creates stale-test-literal bugs;
P4 may surface similar drift in soundness-test fixtures.

---

*P3 closed 2026-05-02. 20/20 features clippy-clean. 16-feature CI
matrix gate added. ~2h actual vs 12-16h estimate (-85%).*
