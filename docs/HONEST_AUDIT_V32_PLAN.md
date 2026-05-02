---
phase: HONEST_AUDIT_V32 — deep re-audit of Fajar Lang post-V26
status: in_progress 2026-05-02
budget: 20-30h Claude work + 4-6h GPU/wall-clock for stress runs
        +30% surprise (high-uncertainty audit per §6.8 R5)
prereq: V26 audit (`docs/HONEST_AUDIT_V26.md` + `HONEST_STATUS_V26.md`)
        as baseline; CLAUDE.md V31.4; FajarOS Nova v3.9.0 + FajarQuant
        v0.4.0 cross-references
artifacts:
  - docs/HONEST_AUDIT_V32.md (this audit's findings)
  - docs/HONEST_STATUS_V32.md (if classifications shift)
  - docs/HONEST_AUDIT_V32_PHASE_<N>_FINDINGS.md (per-phase intermediate)
---

# Honest Audit V32 — Deep Re-Audit Plan

> **Why now.** V26 audit ran 2026-04-11. Since then: V27 (hardened) +
> V27.5 (Compiler Prep with -97% effort variance, suspicious) + V28.5
> (multilingual + 4 fixes, v8 coherence gap open) + V29.P1 (@noinline
> + @inline + @cold) + V29.P2 (SMEP) + V29.P3 + V29.P3.P6 (security
> triple) + V30 (TRACK 1+2+3+4 + GEMMA3) + V30.SIM (LLVM codegen bug
> CONFIRMED, deferred to V31) + V31.B.P2 (@no_vectorize WORKAROUND for
> the V30.SIM-confirmed bug, not root-cause fix) + V31.C (Phase D) +
> V31.D (Track D) + V31.4 closure cycle. ~3 weeks of accumulated
> changes; the V26 baseline is now stale enough that fresh hands-on
> verification is warranted, especially given:
>
> 1. **V27.5 effort variance −97%** (5.6h actual vs 196h estimated)
>    suggests scaffold-shipped-as-done. Items: @app/@host/@interrupt/
>    Cap<T>/refinement-params/IPC stub gen/AI scheduler builtins/
>    fb_set_base. Need to verify these are E2E-tested, not just
>    parser-accepted.
> 2. **LLVM O2 miscompile is unfixed at root.** V31.B.P2 ships
>    `@no_vectorize` workaround; FajarOS Nova kernel still uses gcc
>    C bypass for vecmat + lmhead. M9 "Fajar Lang clean" milestone
>    (V31_MASTER_PLAN.md) was NOT achieved.
> 3. **v8 coherence gap (V28.5 open)** — undocumented scope; needs
>    classification.
> 4. **CLAUDE.md doc-drift risk:** §3 claims "0 [f], 0 [s]" — verify
>    no regressions snuck in via the V27-V31 change cycle.
>
> Per CLAUDE.md §6.6 Documentation Integrity Rules + §6.8 Plan Hygiene,
> baseline drift this large warrants a fresh full audit, not a delta.

## 1. Why a deep audit, not a quick one

User stated 2026-05-02: *"Kita re-audit mendalam lagi Fajar Lang, jangan audit cepat"*

Quick audit would only re-tally test/LOC counts. Deep audit means:

- Hands-on verify every [x] module's callable surface (not just `cargo test`)
- Run every `.fj` example to confirm it works on its claimed backend
- Hands-on smoke every `fj` CLI subcommand, not just count them
- Per-feature E2E verify for every V27-V31 addition (not just parser-accepts)
- Cross-cutting soundness probes (borrow checker holes, kernel/device
  enforcement matrix, codegen-vs-interpreter semantic equivalence)
- Document gaps with actionable verification commands

This is the kind of audit that takes 20-30h. We're committing to that.

## 2. Six-phase structure (sequential)

| Phase | Subject | Effort | Output | Risk |
|---|---|---|---|---|
| 1 | Baseline + change-since-V26 inventory | 1-2h | per-version timeline + claim-list to verify | low |
| 2 | Mechanical verification (test/LOC/clippy/fmt/unwrap/stress) | 3-4h | numerical scorecard vs CLAUDE.md claims | low |
| 3 | Per-module callable-surface audit (54 [x]) | 5-8h | per-module pass/fail table | medium |
| 4 | Deep audit recent additions (V27.5 + V29.P1 + V31.B.P2) | 5-8h | per-feature scorecard with verification cmd | **HIGH** (V27.5 -97% variance is the audit's main risk) |
| 5 | Cross-cutting (soundness, kernel/device matrix, 4-backend equivalence, tensor type) | 4-6h | soundness gap list + matrix coverage report | medium |
| 6 | Writeup HONEST_AUDIT_V32.md + HONEST_STATUS_V32.md | 2-3h | committed audit + decision doc + CLAUDE.md sync | low |

**Total: 20-31h Claude work.** +30% surprise budget (audits are
notoriously prone to surfacing more than expected) → cap **40h**.

## 3. Mechanical decision gates per phase

**Phase 2 PASS criteria:**
- `cargo test --lib` exits 0
- `cargo test --lib --test-threads=64` × 5 runs all exit 0 (§6.5 stress test)
- `cargo clippy --lib -- -D warnings` exits 0
- `cargo fmt -- --check` exits 0
- `python3 scripts/audit_unwrap.py` outputs only header row (0 unwraps)
- Numbers within ±5% of CLAUDE.md §3 claims (test count, LOC, modules, examples, CLI commands)

**Phase 3 PASS criteria:**
- For each of 54 [x] modules: callable surface identified + smoke command runs
- Any module without callable surface OR with broken surface → demoted to [f] in V32

**Phase 4 PASS criteria (most-rigorous):**
- V27.5 items: each has at least 1 E2E test that exercises it from `.fj` source
- V29.P1: lexer + codegen + 5-layer prevention chain all green
- V31.B.P2: `@no_vectorize` E2E confirmed; LLVM O2 miscompile root-cause status documented
- v8 coherence gap: scope identified + classified (real bug vs paper-over)

**Phase 5 PASS criteria:**
- Borrow checker: known holes catalogued; new soundness probes attempted
- @kernel/@device enforcement: all 8 rows of CLAUDE.md §5.3 table verified by tests
- 4-backend equivalence: representative examples produce identical output on interpreter/VM/Cranelift/LLVM
- Tensor shape-check: TE001-TE009 error code tests all pass

**Phase 6 PASS criteria:**
- Audit doc lands; CLAUDE.md updated to reflect V32 status; CHANGELOG entry added

## 4. Tools strategy

- **Foreground audit work:** main agent (this conversation)
- **Per-module surface verification (Phase 3 bulk):** can dispatch to general-purpose agent in parallel batches (54 modules / 8-per-batch = ~7 batches)
- **Hands-on commands:** Bash with timeouts for long-running tests
- **Stress tests (Phase 2):** background bash + Monitor for tail
- **Writeup (Phase 6):** Write to docs/

## 5. Non-modifications

This audit does NOT:
- Fix any bug it surfaces (those are filed for follow-up)
- Modify production code (only audit-related test files if needed)
- Change CLAUDE.md until Phase 6 (audit-induced sync only)
- Re-classify [x] → [f] until evidence is overwhelming + Phase 6 writeup

## 6. Self-check (CLAUDE.md §6.8)

| Rule | Status |
|---|---|
| §6.8 R1 pre-flight audit | YES — this plan IS the pre-flight; subsequent phases are the audit itself |
| §6.8 R2 verification = runnable commands | YES — every Phase has runnable commands in §3 PASS criteria |
| §6.8 R3 prevention layer | partial — audit surfaces gaps; prevention layers (test additions, CI gates) ship in follow-up commits, not in audit itself |
| §6.8 R4 numbers cross-checked | YES — Phase 2 explicitly cross-checks every CLAUDE.md number with hand-run command |
| §6.8 R5 surprise budget +30% | YES — 20-31h × 1.3 = 40h cap |
| §6.8 R6 mechanical decision gates | YES — §3 PASS criteria are mechanical |
| §6.8 R7 public-artifact sync | YES — Phase 6 explicitly syncs CLAUDE.md + CHANGELOG + status doc |
| §6.8 R8 multi-repo state check | YES — verified at plan-write: fajar-lang clean origin/main, fajaros-x86 + fajarquant cross-references read-only this audit |

8/8 satisfied. Audit AUTHORIZED.

## 7. Self-check (CLAUDE.md §6.6 — documentation integrity)

This audit's purpose is to ENFORCE §6.6 across the codebase. By
construction it:
- §6.6 R1 ([x] = E2E working): Phase 3 specifically audits this
- §6.6 R2 (verification method per task): Phase 2-5 all use runnable commands
- §6.6 R3 (no inflated stats): Phase 2 hand-verifies every claim
- §6.6 R4 (no stub plans): not directly applicable (audit not plan)
- §6.6 R5 (audit before building): exactly this audit's purpose
- §6.6 R6 (distinguish real vs framework): Phase 3 + 4 explicitly check this

## 8. Surprise budget tracking

Per-phase tags in commit messages:
```
docs(audit-v32-phase-N): subject [actual Xh, est Yh, +Z%]
```

Cumulative variance tracked in audit doc Phase 6.

---

*HONEST_AUDIT_V32 plan v1.0 — written 2026-05-02. Phase 1 begins
immediately upon this plan-doc commit.*
