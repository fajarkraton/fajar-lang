---
phase: HONEST_AUDIT_V32 Phase 1 — change-since-V26 inventory
status: CLOSED 2026-05-02
budget: ~1h actual (est 1-2h, within budget)
---

# Phase 1 Findings — Change-since-V26 Inventory

**Source:** CHANGELOG.md `[31.0.0] — 2026-04-23 "Phase D + Track B"` entry,
which consolidates v26.3.0 + v27.0.0 + v27.5.0 + v31.0.0 (granular back-fill
deferred per CHANGELOG note). Also: V31_MASTER_PLAN.md, V31_FAJARLANG_P0_FINDINGS.md.

## 24 concrete claims to verify (categorized)

### Category A — Compiler attributes (4 claims)

| # | Claim | Source | Verify Phase |
|---|---|---|---|
| A1 | `@noinline`, `@inline`, `@cold` lexer recognition | V29.P1 | P3 (lexer test), P4 |
| A2 | 5-layer silent-build-failure prevention chain (lexer + codegen test + Makefile ELF-gate + pre-commit + install-hooks) | V29.P1 | P4 |
| A3 | `@no_vectorize` codegen attribute E2E (lexer + parser + codegen, IR + disasm verified) | V31.B.P2 | P4 |
| A4 | `FJ_EMIT_IR` env var dumps pre-opt LLVM IR | V31.B.P2-adjacent | P4 |

### Category B — V27.5 Compiler Prep additions (14 claims, HIGHEST RISK)

V27.5 had **5.6h actual vs 196h estimated = -97% effort variance**. Either
estimate was wildly inflated, or items shipped as scaffold without full
production testing. Each item below needs hands-on E2E verification.

| # | Claim | Source | Verify Phase |
|---|---|---|---|
| B1 | `tensor_workload_hint(rows, cols)` builtin callable from .fj | V27.5 | P4 |
| B2 | `schedule_ai_task(id, priority, deadline)` builtin callable from .fj | V27.5 | P4 |
| B3 | `@interrupt` ISR wrappers — ARM64 target | V27.5 | P4 |
| B4 | `@interrupt` ISR wrappers — x86_64 target | V27.5 | P4 |
| B5 | `@interrupt` ISR target dispatcher wired to AOT pipeline | V27.5 | P4 |
| B6 | `@app` annotation (GUI app entry) — has at least 1 working example | V27.5 | P4 |
| B7 | `@host` annotation (Stage 1 self-hosting) — has at least 1 working example | V27.5 | P4 |
| B8 | `Cap<T>` linear/affine capability type with `cap_new`/`cap_unwrap`/`cap_is_valid` | V27.5 | P4 |
| B9 | Refinement predicates on function parameters (extended from let-binding) | V27.5 | P4 |
| B10 | `fb_set_base(addr)` + `fb_scroll(lines)` VESA framebuffer | V27.5 | P4 |
| B11 | `ServiceStub::from_service_def()` IPC stub generator | V27.5 | P4 |
| B12 | `MAX_KERNEL_TENSOR_DIM` 16 → 128 | V27.5 | P4 |
| B13 | `tests/v27_5_compiler_prep.rs` exists with 16 E2E integration tests | V27.5 | P4 (file existence + run) |
| B14 | `v27_5_regression` CI job in `.github/workflows/` | V27.5 | P4 |

### Category C — Earlier additions (3 claims)

| # | Claim | Source | Verify Phase |
|---|---|---|---|
| C1 | `tests/feature_flag_tests.rs` 12 untested feature flag tests | V27.0 | P3 |
| C2 | `scripts/check_version_sync.sh` (V27 A4 prevention layer) | V27.0 | P4 |
| C3 | `call_main()` rejects non-Function main with TypeError (was silent Null) | V27.0 | P5 |

### Category D — CLAUDE.md rules + meta (3 claims)

| # | Claim | Source | Verify Phase |
|---|---|---|---|
| D1 | §6.10 Filesystem Roundtrip Coverage Rule codified | V30.TRACK4 | P1 (read CLAUDE.md) — DONE |
| D2 | §6.11 Training Script Interruption-Safety Rule codified | V31.C | P1 (read CLAUDE.md) — DONE |
| D3 | CLAUDE.md banner Version sync (V31.0 → V31.4 latest) | various | P2 |

### Category E — CLAUDE.md numerical claims (Phase 2 mechanical)

| # | Claim | Source | Phase |
|---|---|---|---|
| E1 | `cargo test --lib` exits 0 (claimed "0 failures, 0 flakes") | CLAUDE.md §3 | P2 |
| E2 | Stress test `cargo test --lib --test-threads=64` passes 5x | CLAUDE.md §6.5 | P2 |
| E3 | `cargo clippy --lib -- -D warnings` exits 0 | CLAUDE.md §3, CHANGELOG | P2 |
| E4 | `cargo fmt -- --check` exits 0 | CLAUDE.md §3 | P2 |
| E5 | 0 production `.unwrap()` in `src/` | CLAUDE.md §3 | P2 |
| E6 | 54 [x] / 0 [sim] / 0 [f] / 0 [s] modules | CLAUDE.md §3 | P3 |
| E7 | ~7,611 lib tests + 2,553 integ tests + 14 doc tests | CLAUDE.md §3 | P2 |
| E8 | ~448K LOC Rust (394 src/ files) | CLAUDE.md §3 | P2 |
| E9 | 14 MB release binary | CLAUDE.md §3 | P2 |
| E10 | 23 CLI subcommands | CLAUDE.md §3 (NOTE: PRODUCTION_AUDIT_V1 says 39, drift?) | P2 |
| E11 | 238 .fj examples | CLAUDE.md §3 (PRODUCTION_AUDIT_V1 says 243, drift?) | P2 |
| E12 | 0 doc warnings | CLAUDE.md §3 | P2 |

## Open gaps documented but explicitly NOT closed

| # | Gap | Source | Notes |
|---|---|---|---|
| G1 | M9 "Fajar Lang clean" — LLVM O2 miscompile UNFIXED at root | V31_MASTER_PLAN §3 | `@no_vectorize` is workaround; FajarOS Nova still uses gcc C bypass |
| G2 | v8 coherence gap (V28.5 open) | older memory | Scope undocumented; needs classification |
| G3 | Granular CHANGELOG back-fill for v26.3 / v27.0 / v27.5 | CHANGELOG 31.0.0 notes | "Deferred follow-up; no functional gap" |
| G4 | LLVM O2 miscompile root-cause vs upstream-LLVM-bug status | V31.B.P2 + V31_MASTER_PLAN §5 risk-3 | "Keep C bypass, ship @no_vectorize as workaround" — was filed upstream? |

## Phase 1 conclusion

24 verifiable claims + 4 open gaps catalogued. Highest risk concentration:

- **Category B (14 claims)**: V27.5 -97% effort variance — most likely place
  to find scaffold-shipped-as-done
- **Gap G1**: LLVM O2 miscompile is the single largest unsolved technical
  gap; affects paper integrity claim
- **Gap G2**: v8 coherence gap is unscoped — could be either trivial doc
  drift or a real feature gap

Phase 2 (mechanical) starts immediately on Category E claims. Phase 4
(deep audit recent additions) handles Category A + B + C + D.

---

*Phase 1 closed 2026-05-02. Inventory complete.*
