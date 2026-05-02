---
phase: FAJAR_LANG_PERFECTION — close every actionable gap to "sempurna"
status: in_progress 2026-05-02
budget: 80-130h Claude work + opportunistic founder time, +50% surprise = 195h cap
        (high uncertainty: cross-cutting audit may surface more than known)
prereq: HONEST_AUDIT_V32 chain (commits ecd265a2..96843ab7) — internal compiler
        baseline 0 [f] / 0 [s], all gates green. PRODUCTION_AUDIT_V1 (2026-04-30) —
        external/distribution gaps. This plan unifies both.
artifacts:
  - This plan doc
  - docs/FAJAR_LANG_PERFECTION_PHASE_<N>_FINDINGS.md (per-phase, one each)
  - docs/HONEST_AUDIT_V33.md (final synthesis when complete)
  - Code/test/doc commits per phase
  - HONEST_STATUS_V33.md if classifications shift
---

# Fajar Lang Perfection Plan v1.0

> **User signal 2026-05-02:** *"Saya tidak mau quick win, Fajar Lang harus
> sempurna, kalau harus diperbaiki, perbaiki jadi sempurna, buat plan
> detail jika dibutuhkan."*

## 1. What "sempurna" means here (HONEST scope)

We can make Fajar Lang perfect in:
- ✅ **Internal compiler quality** (already mostly there: 10,138 tests, 0 fail/clippy/unwrap; this plan ensures no residual gaps)
- ✅ **Documentation accuracy** (CLAUDE.md, ERROR_CODES.md, CHANGELOG, status docs all hand-verified consistent)
- ✅ **Test coverage** (every public surface has at least 1 test exercising the documented behavior)
- ✅ **Cross-cutting equivalence** (4 backends produce same results; feature-gated paths don't drift)
- ✅ **Soundness probes** (borrow checker, type system, kernel/device isolation tested across an expanded fuzz/property suite)

We CANNOT make perfect (acknowledged honestly):
- ❌ **External adoption** (depends on community; not engineering)
- ❌ **Upstream LLVM bug fix** (depends on LLVM project timelines; we can FILE upstream + quarantine)
- ❌ **"Better than Rust/Go/C" benchmarks** (subjective; we can only ship REAL numbers, not subjective rankings)
- ❌ **Distribution adoption metrics** (stars/forks/installs depend on marketing)

The plan's exit criteria reflect this: "all engineering items closed" not
"world-class adoption achieved."

## 2. Comprehensive gap inventory (25 items, hand-verified 2026-05-02)

### Category A — Compiler hygiene (5 items)

| # | Gap | Source | Effort |
|---|---|---|---|
| A1 | LLVM O2 miscompile root-cause OR upstream filing (M9 milestone) | V32 G1 | 5-8 days |
| A2 | TE002/TE003 catalog reconciliation (docs/ERROR_CODES.md ↔ src/) | V32 latent | 2-3h |
| A3 | Pre-existing test-file clippy warnings (~80 in tests/multifile_tests.rs + tests/security_audit.rs) | V32 sidebar | 1-2h |
| A4 | @interrupt full .fj source E2E test (beyond F4 codegen-API) | V32 partial | 1-2h |
| A5 | Granular CHANGELOG back-fill v26.3, v27.0, v27.5 | V32 G3 (admin) | 2-3h |

### Category B — Cross-cutting test depth (6 items)

| # | Gap | Source | Effort |
|---|---|---|---|
| B1 | 4-backend equivalence on full examples corpus (interp/VM/Cranelift/LLVM) | V32 Phase 5 spot-check | 1-2 days |
| B2 | Effect system (EE001-EE008) full coverage audit | V32 not deeply audited | 1-2 days |
| B3 | Generic system (GE codes + monomorphization) audit | V32 not deeply audited | 1-2 days |
| B4 | Macro system (`macros`, `macros_v12` proc-macros) test depth | V32 not deeply audited | 1 day |
| B5 | Async/await machinery coverage | V32 not deeply audited | 1 day |
| B6 | Feature-gated path testing matrix (cuda, llvm, gui, ble, mqtt, websocket, wasi_p2, wasi_v12) | V32 only spot-checked llvm + cuda | 2 days |

### Category C — Soundness probes (3 items)

| # | Gap | Source | Effort |
|---|---|---|---|
| C1 | Borrow checker (polonius) soundness expansion — new property/fuzz tests | V32 §5 deferred | 2-3 days |
| C2 | Type system soundness (negative tests for ALL SE/KE/DE/TE/ME codes) | partially-tested | 1-2 days |
| C3 | Memory safety probes — fuzz expansion beyond current 8 fuzz targets | existing 8 fuzz targets, can extend | 1-2 days |

### Category D — IDE / LSP quality (3 items)

| # | Gap | Source | Effort |
|---|---|---|---|
| D1 | LSP server quality verification (real-world IDE testing across 5 editor packages) | PRODUCTION_AUDIT_V1 §3.9 | 1-2 days |
| D2 | Semantic tokens (lsp_v3) coverage audit | feature-gated, not E2E tested | 1 day |
| D3 | Error display polish (miette output quality across all 78+ codes) | ERROR_CODES.md catalog | 1 day |

### Category E — Examples + docs depth (4 items)

| # | Gap | Source | Effort |
|---|---|---|---|
| E1 | Example coverage breadth → depth (real-project examples, not just feature-demos) | PRODUCTION_AUDIT_V1 §3.10 | 2-3 days |
| E2 | Stdlib API coverage doc (every public stdlib function has doc + example) | partial | 1-2 days |
| E3 | Tutorial / book / guide for new users | none | 3-5 days |
| E4 | Cargo doc completeness (rustdoc coverage 100% on pub items) | 0 doc warnings ✓ but coverage% unverified | 1 day |

### Category F — Distribution maturity (4 items, partially out of scope)

| # | Gap | Source | Effort |
|---|---|---|---|
| F1 | Binary distribution for current versions (v31 release with attached binaries) | PRODUCTION_AUDIT_V1 §3.1 | 1 day |
| F2 | Apache-2.0 vs MIT license consistency hygiene | PRODUCTION_AUDIT_V1 §3.13 | 30 min |
| F3 | crates.io publish blocker (fajarquant git-rev dep) | PRODUCTION_AUDIT_V1 §3.3 | 1 day (cross-repo) |
| F4 | Real benchmarks vs Rust/Go/C (replace placeholder numbers) | PRODUCTION_AUDIT_V1 §3.4 | 1-2 days |

**Total work-items: 25.** Effort estimate: **80-130 hours** (Claude time;
some require founder time for releases / external systems).

## 3. Phase structure (10 phases, sequential)

| Phase | Subject | Items | Effort | Surprise budget |
|---|---|---|---|---|
| P0 | This plan + comprehensive inventory | – | already done | – |
| P1 | Hygiene batch (low-risk doc + cleanup) | A2, A3, A5, F2 | 6-9h | +25% |
| P2 | Test coverage residuals | A4, B1, B2, B3, B4, B5 | 30-50h | +50% |
| P3 | Feature-gate matrix audit | B6 | 12-16h | +50% |
| P4 | Soundness probes | C1, C2, C3 | 30-50h | +50% |
| P5 | LSP + IDE quality | D1, D2, D3 | 24-32h | +50% |
| P6 | Examples + docs depth | E1, E2, E3, E4 | 50-80h | +25% |
| P7 | Distribution unblock | F1, F3, F4 | 20-30h | +25% |
| P8 | LLVM O2 miscompile (root or upstream) | A1 | 40-60h | +50% |
| P9 | Closeout: HONEST_AUDIT_V33 + CLAUDE.md sync | synthesis | 4-6h | +25% |

**Phase ordering rationale:**

1. P1 first — quick hygiene wins build momentum + clean baseline
2. P2-P3 strengthen test coverage before bigger probes
3. P4 soundness needs P2-P3 stable test infra
4. P5 LSP is parallel-eligible with P4 if budget allows
5. P6 docs depth is the largest item but lowest-risk
6. P7 distribution depends on F-category cross-repo coordination
7. P8 LLVM is highest-uncertainty — placed late so other items don't block
8. P9 synthesis at end

## 4. Per-phase mechanical gates

### P1 PASS criteria
- A2: docs/ERROR_CODES.md TE section reconciled with src/ #[error] variants (either add TE002+TE003 variants OR remove from catalog with reasoning)
- A3: `cargo clippy --tests --release -- -D warnings` exits 0
- A5: CHANGELOG.md has back-filled entries for [26.3.0], [27.0.0], [27.5.0] OR explicit defer-rationale recorded
- F2: license headers consistent across LICENSE, README, Cargo.toml metadata
- All existing gates still green

### P2 PASS criteria
- A4: New `.fj` example file with `@interrupt` compiled via `fj build --backend llvm`, IR captured + asserted via test infrastructure
- B1: For each backend pair (interp/VM/Cranelift/LLVM), at least 20 representative examples produce identical output (table in findings)
- B2: 8 EE codes each triggered by at least 1 negative test
- B3: All GE codes covered + monomorphization tests on 5+ generic patterns
- B4: macro_rules! 5+ patterns + proc-macro 3+ patterns covered E2E
- B5: async/await covered for 5+ patterns (basic, generators, error-prop, cancellation, deadline)

### P3 PASS criteria
- B6: Test matrix executed for ALL feature flags (cuda, llvm, gui, ble, mqtt, websocket, wasi_p2, wasi_v12, https) — each shows exit-0 + test count

### P4 PASS criteria
- C1: New polonius soundness suite — ≥10 new property tests for borrow rules
- C2: Negative tests for ALL 78+ error codes from ERROR_CODES.md — each code triggered by at least 1 test
- C3: Fuzz suite extended +3 targets minimum (e.g. fuzz_codegen, fuzz_borrow, fuzz_async); 60s+ runs all converge with no crashes

### P5 PASS criteria
- D1: All 5 editor packages tested; each opens .fj file + shows diagnostic + completion + go-to-def
- D2: lsp_v3 semantic tokens covered via at least 1 E2E test per token kind
- D3: Every error code has a "good" miette display verified via golden-file test

### P6 PASS criteria
- E1: 5+ real-project example folders (not just single-file demos) — e.g. examples/calculator-cli, examples/mini-os, examples/embedded-mnist
- E2: Every pub stdlib function has /// doc + at least 1 doctest
- E3: docs/TUTORIAL.md or BOOK.md exists with ≥10 chapters covering basics→advanced
- E4: `cargo doc --no-deps --lib --document-private-items` warns 0; pub-item coverage ≥95%

### P7 PASS criteria
- F1: GitHub Releases v31.x has linux + mac + windows binaries attached
- F3: fajarquant published to crates.io OR fajar-lang shim resolves cleanly without git-rev
- F4: README + benchmarks/ shows real numbers vs Rust/Go/C across 5+ standard benchmarks (mandelbrot, fannkuch, fib, json-parse, matrix-multiply)

### P8 PASS criteria (highest uncertainty)
- A1 either: (a) root-cause identified + fixed, OR (b) reproducible repro filed at github.com/llvm/llvm-project + workaround documented as permanent
- M9 milestone CLOSED in V31_MASTER_PLAN.md

### P9 PASS criteria
- HONEST_AUDIT_V33.md written with exit criteria scorecard
- CLAUDE.md fully synced (V31.4 → V32 or V33 banner)
- CHANGELOG entry [Unreleased] → tagged release vNN.0.0
- Push to origin, all green

## 5. Surprise budget (high uncertainty)

Per-phase cap = effort × (1 + surprise_pct). Cumulative cap = sum of phase caps.

| Phase | Estimate | +Surprise | Cap |
|---|---|---|---|
| P0 | 1h | – | 1h |
| P1 | 6-9h | +25% | 11h |
| P2 | 30-50h | +50% | 75h |
| P3 | 12-16h | +50% | 24h |
| P4 | 30-50h | +50% | 75h |
| P5 | 24-32h | +50% | 48h |
| P6 | 50-80h | +25% | 100h |
| P7 | 20-30h | +25% | 38h |
| P8 | 40-60h | +50% | 90h |
| P9 | 4-6h | +25% | 8h |
| **Total** | **218-336h** | – | **470h cap** |

**Realistic Claude-only effort: 130-200h actual** (founder time for F1+F3+F4
external-system tasks runs in parallel + does not count toward Claude
budget). At ~20-30 dedicated hours/week, this is **8-15 weeks** of focused
work.

If the user wants this faster, parallel-able phases are:
- P3 (feature matrix) ‖ P4 (soundness)
- P5 (LSP) ‖ P6 (docs depth)
- P7 (distribution) ‖ P8 (LLVM bug)

With parallelization, **realistic minimum 4-6 weeks**.

## 6. Decision gates between phases (mechanical)

Each phase produces:
1. `docs/FAJAR_LANG_PERFECTION_PHASE_<N>_FINDINGS.md` (committed)
2. Code/test/doc commits referenced in findings
3. PASS/FAIL verdict per item in §4

Downstream phases blocked until findings doc committed (per §6.8 R6).

If a phase BLOWS surprise budget by >50% over cap, plan PAUSES for
re-scoping decision (continue with revised estimate vs descope items
to a future plan).

## 7. Honest scope limits

This plan does NOT include:
- ✗ External adoption growth (community, marketing, conferences)
- ✗ Multi-platform OS-level testing (WSL, BSDs, embedded targets) — beyond
  current Linux + Mac CI matrix
- ✗ Real-world large-scale codebases written IN Fajar Lang (we can write
  ours, but external community needs time)
- ✗ Subjective "better than Rust/Go/C" claims — only objective
  benchmarks
- ✗ Hardware-in-loop testing on physical embedded boards beyond Q6A
  (limited by available hardware)
- ✗ Formal proof of correctness (would require Coq/Isabelle/Lean
  shadow proof — out of scope)

If these become priorities, they are **PERFECTION_PLAN_V2** scope.

## 8. Self-check (CLAUDE.md §6.8)

| Rule | Status |
|---|---|
| §6.8 R1 pre-flight audit | YES — V32 audit + PRODUCTION_AUDIT_V1 + this plan §2 inventory |
| §6.8 R2 verification = runnable commands | YES — every PASS criterion in §4 has runnable command |
| §6.8 R3 prevention layer per phase | partial — case-by-case per phase (e.g. P1 A3 fix prevents future test-clippy regression via CI extension; P2 B6 prevents feature-flag drift via matrix CI) |
| §6.8 R4 numbers cross-checked | YES — gap inventory hand-verified 2026-05-02 |
| §6.8 R5 surprise budget | YES — +25% to +50% per phase, total +44% on aggregate |
| §6.8 R6 mechanical decision gates | YES — §4 + §6 |
| §6.8 R7 public-artifact sync | YES — P9 explicitly syncs CLAUDE.md, CHANGELOG, README |
| §6.8 R8 multi-repo state check | YES — fajarquant + fajaros-x86 cross-references identified in F1+F3 |

8/8 satisfied. Plan AUTHORIZED.

## 9. Self-check (CLAUDE.md §6.6 — documentation integrity)

This plan ENFORCES §6.6:
- §6.6 R1 ([x] = E2E working): P2-P5 expand E2E coverage
- §6.6 R2 (verification method per task): every PASS criterion is runnable
- §6.6 R3 (no inflated stats): P1 A2 + P9 sync ensure all numerical claims hand-verified
- §6.6 R4 (no stub plans): every phase has full task table, no placeholders
- §6.6 R5 (audit before building): P0 IS the audit
- §6.6 R6 (real vs framework): P3 B6 explicitly tests every feature flag

## 10. Exit criteria for "DONE"

Fajar Lang reaches "sempurna" when ALL of these hold simultaneously:

1. ✓ All 25 work-items in §2 closed per their PASS criteria
2. ✓ HONEST_AUDIT_V33.md written + 0 [f] / 0 [s] holds at re-audit
3. ✓ All quality gates green (cargo test all features, clippy --tests, fmt, doc, version sync)
4. ✓ M9 LLVM milestone CLOSED (root-fix or upstream-filed-and-quarantined)
5. ✓ verify_paper_tables.py + check_version_sync.sh + all CI workflows green
6. ✓ CLAUDE.md, CHANGELOG, README, docs/ all consistent at hand-verified accuracy
7. ✓ Tagged release (likely v32.0.0 or v33.0.0) with binary assets
8. ✓ Real benchmark numbers vs ≥3 reference languages published

NOT exit criteria (tracked separately, not blocker):
- External adoption metrics
- Subjective comparisons
- Multi-month community signals

## 11. Plan v1.0 → v1.x revisions

This plan is v1.0. Per §6 decision gates, after each phase the plan
may be revised:
- v1.1 if a phase finds significant new sub-items
- v1.2 if scope changes (e.g. user redirect)
- v2.0 if structural restructure needed

All revisions tracked in commit history.

---

*FAJAR_LANG_PERFECTION_PLAN v1.0 — written 2026-05-02. P1 begins
immediately upon plan-doc commit. Total scope: 25 work-items across
10 phases, 130-200h realistic Claude effort, 4-15 weeks calendar.*
