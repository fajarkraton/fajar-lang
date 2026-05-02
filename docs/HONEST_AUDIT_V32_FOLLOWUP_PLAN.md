---
phase: HONEST_AUDIT_V32 follow-up — fix 4 of 5 surfaced gaps
status: in_progress 2026-05-02
budget: ~3h Claude work, +25% surprise = 3.75h cap
prereq: HONEST_AUDIT_V32.md (commit `5c08f511`) — closes audit, surfaces 5 gaps
deferred: G1 (LLVM O2 miscompile, 5-8 days) — non-blocking, opportunistic
artifacts:
  - tests/eval_tests.rs (G3 unit test addition)
  - CLAUDE.md (G4 + G5 doc sync)
  - tests/<new file or existing> (G2 @interrupt E2E)
---

# V32 Audit Follow-up — 4-Gap Fix Plan v1.0

> **Why this plan exists.** HONEST_AUDIT_V32 §6 ranked 5 recommended
> actions. Items 1-4 are tiny (G3 ~10min, G4 ~5min, G5 ~10min, G2 ~2h),
> total ~3h. Item 5 (G1 LLVM upstream filing, 5-8 days) is opportunistic
> and out of scope here. Per CLAUDE.md §6.8 R1 we plan before executing
> for trackability and surprise-budget hygiene; the audit doc IS the
> plan baseline, this plan adds per-fix verify commands + ordering.

## 1. Sub-task table

| # | Gap | Effort | Verify command | Decision gate |
|---|---|---|---|---|
| F1 | **G5** numerical drift sync (CLAUDE.md §3) | ~10 min | Each new number matches hand-run cmd at audit §3 actuals column | All 6 drifts updated; `scripts/check_version_sync.sh` PASS |
| F2 | **G4** TE001-TE009 doc fix (CLAUDE.md §7) | ~5 min | `grep "TE001" CLAUDE.md` shows "1 variant, 9 scenarios" framing | One-shot edit |
| F3 | **G3** call_main TypeError unit test | ~10 min | `cargo test --release call_main_rejects_non_function` exits 0 | Test added + passes |
| F4 | **G2** @interrupt E2E test | ~2h | (a) pre-flight: existing IR-grep test pattern found OR new pattern created. (b) `cargo test` passes new `@interrupt_compiles_with_naked_attribute` test | New test compiles `.fj` with @interrupt + asserts `naked` + `noinline` + `.text.interrupt` |

**Order:** F1 → F2 → F3 → F4. F1+F2+F3 are doc/single-test fixes that
unblock no dependencies; F4 has the only unknown (test infrastructure
for IR-grep). Doing F1-F3 first builds momentum; F4 happens at the end
where surprise-budget headroom is most useful.

## 2. Total budget

- F1: 10 min
- F2: 5 min
- F3: 10 min
- F4: 120 min
- **Total: 145 min ≈ 2.4h**
- +25% surprise: **3h cap**

Per-fix commit message tags (per §6.8 R5):
```
fix(audit-v32-followup-fN): subject [actual Xmin, est Ymin, +Z%]
```

## 3. Decision gates per fix

### F1 (G5 numerical drift sync)
- **PASS criteria:** All 6 drifts updated AND `bash scripts/check_version_sync.sh` exits 0 AND `cargo test --lib --release` still passes
- **Failure mode:** numerical drift sync invalidates a prior CLAUDE.md claim → revert + re-classify in audit doc

### F2 (G4 TE doc fix)
- **PASS criteria:** Updated text says "TE001 (9 scenarios)" or equivalent; no claim of "TE001-TE009 codes"
- **Failure mode:** edit too aggressive (touches unrelated §7 sections) → revert + re-edit narrowly

### F3 (G3 call_main test)
- **PASS criteria:** New test in `tests/eval_tests.rs` named `call_main_rejects_non_function_main`; exercise `let main = 42`; assert error matches `TypeError` with message containing "is not a function"
- **Failure mode:** existing test infrastructure differs from expected pattern → adapt test signature

### F4 (G2 @interrupt E2E test)
- **PRE-FLIGHT (5-15 min):**
  - Search for existing `.fj` → IR-grep test patterns: `grep -rn "FJ_EMIT_IR\|llvm.*IR.*test\|llvm.*emit_ir\|backend.*llvm.*test" tests/` etc.
  - Search for tests that build via `fj build`: `grep -rn "fj_build\|cmd_build\|backend.*llvm" tests/`
  - Decision: (a) reuse existing pattern, OR (b) write a new test using `cargo run -- build --backend llvm`, OR (c) defer the test if E2E infrastructure isn't trivial
- **PASS criteria:** Test compiles `@interrupt fn isr() {...}`, captures generated LLVM IR (or object file), and asserts:
  - Function has `naked` attribute
  - Function has `noinline` attribute
  - Function in section `.text.interrupt`
- **Failure mode (likely):** if pre-flight finds NO existing IR-grep pattern, scope-shrink to a unit test that calls codegen API directly (skipping `.fj` source compilation). Documented in commit message as "scope reduction due to absent IR-grep infrastructure."
- **Defer trigger:** if pre-flight + scope-shrink BOTH fail (no easy hook), defer F4 to a separate session and note in audit §6 row 4

## 4. Order rationale

**F1 → F2 → F3 → F4** because:
1. F1+F2 are pure doc edits (CLAUDE.md). They share an edit context, batch well, and their failure modes are revertible.
2. F3 is a tiny test addition with known infrastructure (eval_tests.rs).
3. F4 is the only item with REAL unknowns. Putting it last means we hit the surprise budget where it matters; if F1-F3 came in under estimate (likely), F4 has more cushion.

## 5. What gets committed

Two commit chains:
- **Commit A:** F1 + F2 (CLAUDE.md sync) — single edit-batch
- **Commit B:** F3 (call_main test)
- **Commit C:** F4 (@interrupt test) — only if pre-flight succeeds
- **Commit D (closeout):** update HONEST_AUDIT_V32.md §6 to mark items closed; CHANGELOG entry; push.

If F4 defers, commit C skipped and §6 row 4 gets "deferred to V32.1" annotation.

## 6. Self-check (CLAUDE.md §6.8)

| Rule | Status |
|---|---|
| §6.8 R1 pre-flight audit | YES — HONEST_AUDIT_V32 IS the pre-flight; F4 has its own pre-flight sub-step |
| §6.8 R2 verification = runnable commands | YES — §1 + §3 both list runnable commands |
| §6.8 R3 prevention layer | partial — F1+F2 sync drift; long-term §6.8 R3 prevention via better doc-drift tooling is out of scope (would itself be a project) |
| §6.8 R4 numbers cross-checked | YES — F1 sources every number from §3 actuals which are hand-verified |
| §6.8 R5 surprise budget +25% | YES — 145 min × 1.25 = ~180 min cap |
| §6.8 R6 mechanical decision gates | YES — §3 PASS criteria are mechanical |
| §6.8 R7 public-artifact sync | YES — F1+F2 IS the public-artifact sync (CLAUDE.md is the public-facing master ref) |
| §6.8 R8 multi-repo state check | YES — fajar-lang clean origin/main; fajaros-x86 + fajarquant unchanged in this work |

8/8 satisfied. Follow-up AUTHORIZED.

---

*V32 follow-up plan v1.0 — written 2026-05-02. F1 (CLAUDE.md numerical
drift sync) starts immediately upon plan-doc commit. F4 includes a
pre-flight sub-step before its main work.*
