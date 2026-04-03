# Re-Audit V17 — Phase 6+7: Test Quality & Document Reconciliation

> **Date:** 2026-04-03
> **Scope:** Test quality classification + cross-reference of V13-V16 task documents

---

## Phase 6: Test Quality Assessment

### Test Classification by Module (from exploration sampling)

| Module | Tests | E2E % | Unit % | Shallow % | Quality |
|--------|-------|-------|--------|-----------|---------|
| codegen/cranelift | 1,119 | 30% | 35% | 25% | **GENUINE** — compile-and-run tests |
| interpreter | 604 | 60% | 30% | 10% | **GENUINE** — eval_source behavioral |
| analyzer | 519 | 40% | 40% | 20% | **GENUINE** — type error detection |
| package | 386 | 40% | 50% | 10% | **GENUINE** — dependency resolution |
| ffi_v2 | 358 | 25% | 55% | 20% | **MOSTLY GENUINE** — pyo3 tests real |
| verify | 349 | 10% | 65% | 25% | **INFLATED** — mostly struct tests |
| bsp | 336 | 10% | 50% | 40% | **INFLATED** — display/struct tests |
| compiler | 325 | 20% | 55% | 25% | **INFLATED** — many display format tests |
| distributed | 322 | 35% | 50% | 15% | **GENUINE** — real TCP tests |
| selfhost | 320 | 20% | 50% | 30% | **INFLATED** — parser tests only |
| demos | 317 | 10% | 60% | 30% | **INFLATED** — reference only |
| wasi_p2 | 244 | 5% | 55% | 40% | **INFLATED** — mostly tokenization |
| parser | 220 | 50% | 40% | 10% | **GENUINE** — AST structure tests |
| stdlib_v3 | 212 | 30% | 50% | 20% | **GENUINE** — crypto/net tests |
| rtos | 174 | 15% | 50% | 35% | **MIXED** — simulation behavioral |
| lsp | 172 | 25% | 50% | 25% | **GENUINE** — position/diagnostic tests |
| dependent | 156 | 10% | 60% | 30% | **INFLATED** — mostly struct tests |
| lexer | 143 | 60% | 30% | 10% | **GENUINE** — tokenization behavioral |
| gui | 118 | 10% | 50% | 40% | **MIXED** — color/widget struct tests |
| gpu_codegen | 112 | 15% | 50% | 35% | **INFLATED** — SPIR-V without correctness |

### Overall Test Quality Estimate

| Category | Estimated Count | % of 8,280 |
|----------|----------------|------------|
| **GENUINE E2E** (eval_source, compile-run, CLI) | ~2,500 | 30% |
| **GENUINE Unit** (real algorithm testing) | ~3,300 | 40% |
| **SHALLOW** (struct creation, display format) | ~1,700 | 20% |
| **MIXED/UNCLEAR** | ~780 | 10% |

**~70% of tests are genuinely useful**, ~20% are shallow (test that types exist, format correctly), ~10% unclear.

---

## Phase 7: Document Reconciliation

### V13_TASKS.md — 712 [x] claimed

**Spot-check results (15 random tasks):**
- 8/15 have corresponding code (types + functions exist)
- 4/15 have types but no E2E integration (should be [f])
- 2/15 have code but are behind feature gates (not default-usable)
- 1/15 claims example file that doesn't exist (verified_ml.fj missing)

**Estimated real [x] rate:** ~55% (real code wired to CLI or interpreter)
**Estimated [f] rate:** ~40% (code exists but not user-accessible)
**Estimated inflated:** ~5% (claims don't match code)

**Corrected V13 estimate:** ~390 real [x], ~285 [f], ~37 inflated → **~390/712 = 55% genuine**

### V14_TASKS.md — Self-Contradictory

The document header explicitly states:
> "True remaining: 210 tasks are [f] (framework/test-only, not end-to-end working)."

But the summary table shows 500/500 [x]. The header is the honest part — it was written during a re-audit. The summary table was never updated.

**From our Phase 1-5 findings, V14 areas:**
- Release & Polish (50): Most [x] (tooling works) → ~45 real [x]
- Production Hardening (50): Most [x] (tests + CI) → ~40 real [x]
- FajarOS Nova (100): Header says 80→20 real [x]. Our Phase 5 confirms OS runtime is simulation. → ~25 real [x]
- Real-World Validation (100): Header says 75→25 real [x]. → ~30 real [x]
- Effect System (40): Effects work in interpreter. → ~34 real [x]
- Dependent Types (40): Refinement types work, Pi/Sigma not in parser. → ~22 real [x]
- GPU Shaders (40): SPIR-V/PTX verified. → ~38 real [x]
- LSP v4 (40): tower-lsp server real. → ~33 real [x]
- Package Registry (40): Registry server works. → ~35 real [x]

**Corrected V14 estimate:** ~302 real [x], ~178 [f], ~20 inflated → **~302/500 = 60% genuine**

### V15_TASKS.md — 113 [x], 4 [f] claimed (of ~120)

V15 was the most recent and most careful. Bug fixes and integration work.

**From our findings:**
- Option 1 (Bug Fixes): 30 tasks, mostly verified → ~28 real [x]
- Option 2 (Integration): MNIST pipeline, FFI, CLI → ~16 real [x], ~14 [f]
- Option 3 (Hardening): Benchmarks, recursion limit → ~5 real [x], ~25 [f]
- Option 4 (Docs/Release): Quality gates, examples → ~6 real [x], ~24 [f]

**Corrected V15 estimate:** ~55 real [x], ~65 [f] → **~55/120 = 46% genuine**

### V16_TASKS.md — 7 [x], 5 [f] claimed (of ~123)

V16 was actually well-marked already. Only 12 tasks have marks (the rest don't have checkbox format).

**Based on our GPU codegen verification:** SPIR-V and PTX confirmed working.

### CLAUDE.md Discrepancies

| Claim | CLAUDE.md | Actual | Correction Needed |
|-------|-----------|--------|------------------|
| Tests | 8,475 | 8,317 | Update to 8,317 |
| LOC | ~486,000 | 473,909 | Update to ~474K |
| Files | 442 | 441 | Update to 441 |
| Examples | 216+ | 257 | Update to 257 |
| Formatting | "Clean" | 70 diffs in 16 files | Fix: "70 fmt diffs" |
| .unwrap() in src | "0" (implied by NEVER rule) | 43 production | Document: 43 |
| todo!() in src | "0" (implied) | 14 | Document: 14 |
| V14 status | "494/500 [x] (99%)" | ~302/500 (60%) | Correct to 60% |
| V15 status | "120/120 COMPLETE" | ~55/120 real [x] | Correct to 46% |
| Native tests | "pass" | Stack overflow crash | Document bug |
| Context enforcement | Full table (Section 5.3) | NOT enforced | Mark as NOT WORKING |

### GAP_ANALYSIS_V2.md — "100% PRODUCTION"

This claim is incorrect. Actual status:
- 33/56 modules (59%) are PRODUCTION
- 18/56 modules (32%) are FRAMEWORK
- 3/56 modules (5%) are STUBS
- 2/56 modules (4%) are PARTIAL

The correct claim would be: **"59% PRODUCTION, 32% FRAMEWORK, 9% STUB/PARTIAL"**

---

*Phase 6+7 complete — 2026-04-03*
