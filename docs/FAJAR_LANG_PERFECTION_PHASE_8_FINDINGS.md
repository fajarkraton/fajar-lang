---
phase: FAJAR_LANG_PERFECTION P8 — LLVM O2 miscompile root-cause-or-upstream
status: CLOSED 2026-05-03 (engineering-side; upstream filing requires founder external action)
budget: ~45min actual (est 40-60h plan; +50% surprise = 90h cap; -99% under)
plan: docs/FAJAR_LANG_PERFECTION_PLAN.md §3 P8 + §4 P8 PASS criteria
---

# Phase 8 Findings — LLVM O2 vecmat miscompile

## Summary

P8 closed engineering-side in ~45min — the heaviest-uncertainty phase
in the perfection plan landed under budget by ~99% because the bug is
**already deeply quarantined** (3 layers shipping in production), and
the missing piece is the upstream-filing step which requires founder
external action (LLVM project account + bug-tracker authorization).

| Item | Status | Effort | PASS criterion |
|---|---|---|---|
| A1 — Document 3 quarantine layers + filing draft | ✅ CLOSED | ~25min | repro doc + filing checklist |
| A1 — Regression gate (`@no_vectorize` codegen tests) | ✅ CLOSED | ~15min | 2 LLVM tests, both PASS |
| Pre-flight + this doc | — | ~5min | findings + commit |

## A1 — LLVM O2 miscompile (CLOSED engineering-side)

### Three quarantine layers verified

`docs/LLVM_O2_VECMAT_MISCOMPILE_REPRO.md` documents the 3 layers
shipping in production:

1. **`@no_vectorize` annotation** — disables LLVM SSE/AVX/AVX2/AVX512
   target-features + `no-implicit-float` on tagged functions. Lexer +
   parser + codegen wired since V31.B.P2.
2. **gcc C bypass** — `km_vecmat_packed_v8` reimplemented in C, linked
   alongside Fajar Lang output. Shipped in fajaros-x86 since
   commit `6af7319` (V30 Track 3 P3.6).
3. **Architectural choice** — Phase D (V31.C) chose MatMul-Free LLM
   (HGRN-Bit), eliminating the large-vecmat hot path from critical
   path entirely.

### New regression gate

`src/codegen/llvm/mod.rs::tests` ships **2 new tests** (gated on
`--features llvm`) that protect Layer 1 from silent breakage:

- `at_no_vectorize_emits_no_implicit_float_and_target_features` —
  asserts a `@no_vectorize` function in the AST produces LLVM IR
  with both `no-implicit-float` and the negative-vector
  `target-features` string attributes.
- `at_no_vectorize_does_not_affect_regular_functions` — defensive:
  a function without `@no_vectorize` must NOT inherit the restrictive
  attribute group.

If a future codegen edit accidentally drops or weakens the
`@no_vectorize` codegen path, these tests fire on the next
`cargo test --features llvm`, surfacing the regression before it
reaches a release.

### Upstream filing draft

`docs/LLVM_O2_VECMAT_MISCOMPILE_REPRO.md` ships a **paste-ready filing
template** with:
- Reduced description suitable for github.com/llvm/llvm-project
- Trigger characteristics (loop shape, loop bounds, target features)
- Workaround details (the `@no_vectorize` attribute approach)
- Filing checklist: 6 steps the founder takes when ready to file
- Post-filing actions: update G1 in HONEST_AUDIT_V32, mark M9 closed

### Pre-existing test fix (opportunistic)

While running the new test gate, found `llvm_compile_float_literal`
failing with a stale assertion: body uses `make_float_lit(1.25)` but
the assertion checked `ir.contains("3.14")`. This was a leftover from
an earlier P3 wave (clippy `approx_constant` fix that updated the
literal but not the assertion). Fixed in same commit. **162/162**
LLVM tests now pass.

## Honest scope (per §6.6 R6)

Plan PASS criterion has two paths:
- **(a) root-cause identified + fixed** — 5-8 days of LLVM-internals
  bisect work; out of scope for a single-session perfection-plan
  pass without dedicated kernel-toolchain investigation.
- **(b) reproducible repro filed at github.com/llvm/llvm-project +
  workaround documented as permanent** — what this phase ships:
    * workaround documented (3 layers, source-traced, regression-tested)
    * filing draft prepared (paste-ready)
    * filing itself = founder external action

The bug is currently **defended in depth**:
- Layer 1 (`@no_vectorize`) catches per-function regressions
- Layer 2 (gcc C bypass) protects the production fajaros-x86 kernel
- Layer 3 (architectural HGRN-Bit choice) means future LLM work
  doesn't re-enter the vulnerable code path

Until founder files the upstream LLVM bug, M9 remains *technically*
OPEN per the strict reading of P8 PASS criterion (b). But the
**production risk is mitigated**, and the **filing readiness** that
P8 was meant to deliver is shipped.

## Verification commands (all green)

```
cargo test --release --features llvm --lib codegen::llvm   162 PASS / 0 FAIL
cargo test --lib --release                                7626 PASS / 0 FAIL
cargo clippy --tests --release --features llvm -- -D warnings exit 0
cargo fmt -- --check                                          exit 0
```

## Self-check (CLAUDE.md §6.8)

| Rule | Status |
|---|---|
| §6.8 R1 pre-flight audit | YES — surveyed quarantine layers + AUDIT_V32 G1 |
| §6.8 R2 verification = runnable commands | YES — see Verification |
| §6.8 R3 prevention layer per phase | YES — 2 codegen regression tests |
| §6.8 R4 numbers cross-checked | YES — 162 LLVM tests + 7626 lib stress |
| §6.8 R5 surprise budget | YES — under cap by ~99% (45min vs 40-60h+) |
| §6.8 R6 mechanical decision gates | YES — regression tests fire on attribute drop |
| §6.8 R7 public-artifact sync | partial — CHANGELOG entry deferred to next push |
| §6.8 R8 multi-repo state check | YES — Layer 2 names fajaros-x86 commit `6af7319` |

7/8 fully + 1 partial.

## Self-check (CLAUDE.md §6.6 — documentation integrity)

| Rule | Status |
|---|---|
| §6.6 R1 ([x] = E2E working) | YES — regression tests exercise full LLVM codegen path |
| §6.6 R2 verification per task | YES — every PASS criterion has runnable command |
| §6.6 R3 no inflated stats | YES — engineering-side closure honestly distinguished from upstream-file step |
| §6.6 R4 no stub plans | YES — repro doc has full filing checklist, not placeholder |
| §6.6 R5 audit before building | YES — pre-flight reviewed quarantine layer 1-3 status |
| §6.6 R6 real vs framework | YES — explicit PASS criterion (b) interpretation, M9 status honest |

6/6 satisfied.

## Onward to P9 (final phase)

Per the perfection plan §3 ordering, P9 = closeout synthesis is the
last phase:
- HONEST_AUDIT_V33 written with exit-criteria scorecard for all 25 work-items
- CLAUDE.md fully synced (V32 → V33 banner)
- CHANGELOG `[Unreleased]` → tagged release vNN.0.0
- Push to origin, all green

P9 effort estimate: 4-6h (+25% surprise = 8h cap). Final closeout +
release sync. After P9: FAJAR_LANG_PERFECTION_PLAN is COMPLETE per
its own §10 exit criteria (modulo the external founder actions
documented in F3 and P8 closure).

---

*P8 fully CLOSED engineering-side 2026-05-03 in single session. Total
~45min (vs 40-60h estimate; -99% under).*

**P8.A1** — 3 quarantine layers documented + 2 regression tests +
upstream-filing draft. M9 milestone status honestly: defended in
depth in production; technical "upstream-filed" closure deferred to
founder external action.

P0+P1+P2+P3+P4+P5+P6+P7+P8 of FAJAR_LANG_PERFECTION_PLAN are now
CLOSED (9 of 10 phases). Remaining: P9 synthesis closeout.
