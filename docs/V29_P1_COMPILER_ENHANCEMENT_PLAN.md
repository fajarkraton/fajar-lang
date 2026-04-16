# V29.P1 — Compiler Enhancement: @noinline + Silent-Build-Failure Closure

**Phase:** V29.P1 "Compiler Enhancement" (cross-repo)
**Parent:** V29 "Hardening" (FajarOS P2 + prerequisite compiler fixes)
**Status:** PLAN (2026-04-16) — execution pending user go-ahead per sub-phase
**Plan Hygiene:** satisfies Rules 1–8 (see §11 self-check at end)
**Signed by:** Muhamad Fajar Putranto
**Signed at:** 2026-04-16

---

## 1. Problem Statement

FajarOS kernel has been **silently failing to build** since commit
`fajaros-x86@5670b4e` (2026-04-16 14:35) despite the Makefile reporting
success. The root cause is a two-layer bug:

### 1.1 Primary blocker: `@noinline` not recognized by lexer

Fajar Lang compiler binary (`target/release/fj`, built 2026-04-14 11:46)
rejects `@noinline` as "unknown annotation":

```
x unknown annotation '@noinline' at <combined.fj line>:1
  help: valid annotations: @kernel @device @safe @unsafe @ffi
```

The codegen layer (`src/codegen/llvm/mod.rs:3277`) has full support
for the `noinline` attribute name, but the lexer never produces that
token because the `ANNOTATIONS` table in `src/lexer/token.rs:743-774`
does not list `"noinline"`. Current annotations in the table:

```
kernel, device, npu, gpu, safe, unsafe, ffi, panic_handler, no_std,
entry, repr_c, repr_packed, simd, test, should_panic, ignore,
section, infer, interrupt, message, requires, ensures, invariant,
derive, pure, shared, app, host
```

### 1.2 Secondary blocker: `fj build` silent failure + Makefile false-OK

When `fj build` hits an LE001 "unknown annotation" error, the compiler:
- Emits the error to stderr
- Does **not** produce the output ELF file
- Exits with status **0** (success!)

Then `fajaros-x86/Makefile:258-259` pipes stderr through `grep -v`
filter and unconditionally prints `[OK] LLVM kernel built` on the
next line. There is no `test -f $(KERNEL_LLVM)` guard, so the
"success" message fires even when no ELF exists.

### 1.3 Blast Radius

- **Duration invisible:** 2026-04-16 14:35 → 16:30 (~2 hours) on this
  session alone; potentially longer if earlier sessions also missed it
- **Kernel running stale:** every `make run-kvm-llvm` since
  commit `5670b4e` has booted whatever ELF existed from BEFORE
  `@noinline` was added. The V28.5 multilingual output documented
  in commit `fajaros-x86@5670b4e` was NOT produced by a kernel with
  `@noinline` active.
- **V29.P2.SMEP step 1 commit `fajaros-x86@0396286` unverified:** the
  new `pte_audit.fj` code passed Makefile's false-OK but was not
  actually included in a real ELF. Step 2 (boot auto-invoke) failed
  silently because of this.

### 1.4 Prevention Layer Gap (Rule 3)

The Makefile printing `[OK]` without verifying the ELF exists is a
bug-class (silent-build-failure). A fix that only patches the
lexer without also adding a build gate leaves the bug-class open
for the next compiler feature that hits a similar lexer gap.

---

## 2. Scope (Cross-Repo)

### 2.1 Fajar Lang (primary)
| File | Change |
|------|--------|
| `src/lexer/token.rs` | Add `m.insert("noinline", TokenKind::AtNoInline)` to ANNOTATIONS; add `AtNoInline` variant to `TokenKind` enum |
| `src/lexer/mod.rs` | Error message update: include `@noinline` in valid-annotations hint |
| `src/lib.rs:269` | Same error message update |
| `src/parser/<annotation-consuming path>` | Accept `AtNoInline` where annotations are valid |
| `src/analyzer/<context checks>` | Verify `@noinline` has no context restriction (none needed; pure codegen hint) |
| `tests/annotation_tests.rs` (new) | Lexer+parser+codegen integration test for `@noinline` |
| `tests/codegen_annotation_coverage.rs` (new) | **Prevention test** — iterate all codegen match arms and verify lexer accepts each annotation name |

### 2.2 FajarOS x86
| File | Change |
|------|--------|
| `Makefile:258-259` | Replace unconditional `@echo [OK]` with `test -f $(KERNEL_LLVM) && @echo [OK] ... || { echo [FAIL] ELF not produced; exit 1; }` |
| `scripts/git-hooks/pre-commit` | New check 5/5: reject commit if `make build-llvm` reports OK but ELF missing (catch silent failures before they leave dev machine) |
| `docs/V28_5_CLOSED.md` | Retroactive callout: @noinline was not active during multilingual test |
| `CHANGELOG.md [3.4.0]` | Addendum acknowledging the retroactive finding |

### 2.3 FajarQuant
No direct changes. The kernel-port shim (`kernel/compute/{fajarquant,turboquant}.fj`) does not use `@noinline`; unaffected.

### 2.4 Documentation (memory/claude-context)
| File | Change |
|------|--------|
| `~/.claude/projects/-home-primecore-Documents-Fajar-Lang/memory/MEMORY.md` | Correct V28.5 status line — @noinline never active; multilingual success came from kernel WITHOUT it |
| `memory/project_v28_1_gemma3.md` | Same retrospective note |
| `CLAUDE.md` (Fajar Lang) | §3 Version History: note V27.5 compiler did not ship @noinline lexer support |

---

## 3. Skills & Knowledge Required

| Area | Depth | Reference |
|------|-------|-----------|
| **Fajar Lang lexer internals** | Medium — understand `scan_annotation`, `lookup_annotation`, `ANNOTATIONS` HashMap, `TokenKind` enum layout | `src/lexer/{token.rs, mod.rs}` |
| **Fajar Lang parser annotation flow** | Light — find where annotation tokens are consumed for function decls | `src/parser/<dfs>` |
| **Fajar Lang analyzer context** | Light — verify `@noinline` needs no semantic gate (pure codegen hint) | `src/analyzer/` |
| **LLVM attributes** | Medium — understand `NoInline` vs `AlwaysInline`, relationship to `@inline("never")` alias | inkwell crate docs + `src/codegen/llvm/mod.rs:3244-3309` |
| **Rust testing** | Medium — `cargo test --lib`, integration test patterns | CLAUDE.md §9 |
| **Make + Bash build gates** | Light — `test -f`, exit status, `set -e` semantics | POSIX Make reference |
| **Git multi-repo workflow** | Light — commit, cross-repo verify cycle | `CLAUDE.md §10` |
| **Plan Hygiene retrospectives** | Medium — honest correction of inflated claims without losing audit trail | `CLAUDE.md §6.8` Rule 7 |

**Skill gaps flagged:** none. All required knowledge is in existing codebase + CLAUDE.md + prior commits. No external research needed.

---

## 4. Phased Approach

### Phase V29.P1.P0 — Pre-Flight Audit (this plan's findings)

| # | Task | Verification | Est |
|---|------|--------------|-----|
| P0.1 | Reproduce @noinline rejection via `fj build` direct call | `fj build ... 2>&1 \| grep "unknown annotation '@noinline'"` → 1 match | 0.05h |
| P0.2 | Verify ANNOTATIONS table lacks "noinline" | `grep -c '"noinline"' Fajar\ Lang/src/lexer/token.rs` → 0 | 0.05h |
| P0.3 | Verify codegen has noinline arm | `grep -c '"noinline" =>' Fajar\ Lang/src/codegen/llvm/mod.rs` → ≥1 | 0.05h |
| P0.4 | Verify Makefile missing ELF gate | `grep -A1 'fj build' fajaros-x86/Makefile \| grep -c 'test -f'` → 0 | 0.05h |
| P0.5 | Commit this plan doc + P0 findings | `git log --oneline \| head -1 \| grep v29-p1-p0` → 1 | 0.1h |

**Phase P0 total: 0.3h**
**Deliverable:** this plan file + initial findings commit

### Phase V29.P1.P1 — Lexer + Parser @noinline Support

| # | Task | Verification | Est |
|---|------|--------------|-----|
| P1.1 | Add `AtNoInline` variant to `TokenKind` enum (src/lexer/token.rs) | `grep -c 'AtNoInline' src/lexer/token.rs` → ≥2 (enum + test) | 0.1h |
| P1.2 | Add `"noinline" → AtNoInline` to ANNOTATIONS lazy HashMap | `cargo test --lib lexer::token::tests::lookup_annotation` passes with noinline case | 0.1h |
| P1.3 | Update error hint message (2 sites: lexer/mod.rs:1436, lib.rs:269) to include `@noinline` | `grep -c '@noinline' src/lexer/mod.rs src/lib.rs` → ≥2 | 0.05h |
| P1.4 | Verify parser accepts the new token (likely no change needed — annotation consumer is generic) | write test: `fn parse_noinline_annotation_on_fn` — annotated fn parses without error | 0.15h |
| P1.5 | Verify analyzer has no blocker (likely no context restriction needed) | `cargo test --lib analyzer` all pass | 0.1h |
| P1.6 | Add integration test: tiny `.fj` with `@noinline fn f() {}` compiles to LLVM IR with `noinline` function attribute | new test in `tests/codegen_annotation_integration.rs`: `nm build/test.o \| grep noinline` OR LLVM IR text contains `#\d+ = { noinline }` | 0.2h |
| P1.7 | Prevention test: codegen annotation match arms (`inline`, `noinline`, `cold`, `interrupt`, `section`) each have a lexer ANNOTATIONS entry (meta-test; fails if future codegen support lands without lexer wiring) | new test `tests/codegen_annotation_coverage.rs`: constant array vs ANNOTATIONS lookup | 0.3h |

**Phase P1 total: 1.0h**
**Deliverable:** Fajar Lang source changes committed; all tests pass including new coverage meta-test

### Phase V29.P1.P2 — Rebuild fj + Verify FajarOS Kernel Compiles

| # | Task | Verification | Est |
|---|------|--------------|-----|
| P2.1 | `cargo build --release` in Fajar Lang | `ls -la target/release/fj` — mtime >= now, size sane | 0.2h |
| P2.2 | Run `cargo test --lib` to confirm no regression | output: `test result: ok. <N> passed; 0 failed; 0 ignored` where N ≥ 7611 (V27.5 baseline) | 0.1h |
| P2.3 | Clean FajarOS build dir: `cd fajaros-x86 && rm -f build/fajaros-llvm.elf build/combined.fj` | `ls build/fajaros-llvm.elf` → "No such file" | 0.02h |
| P2.4 | Run `make build-llvm` in FajarOS | `ls -la build/fajaros-llvm.elf` — file exists, size > 1 MB | 0.1h |
| P2.5 | Verify @noinline attributes reached LLVM IR (spot check) | `objdump -d build/fajaros-llvm.elf \| grep -c "<km_vecmat_packed_v8>:"` ≥ 1 (function preserved, not inlined away) | 0.1h |

**Phase P2 total: 0.5h**
**Deliverable:** Fresh `fj` binary that accepts @noinline; FajarOS ELF reproducibly built

### Phase V29.P1.P3 — Makefile Silent-Failure Gate (Prevention Layer, Rule 3)

| # | Task | Verification | Est |
|---|------|--------------|-----|
| P3.1 | Replace `@echo "[OK] ..."` at Makefile:259 with guarded form: `@test -f $(KERNEL_LLVM) && echo "[OK] ..." \|\| { echo "[FAIL] $(KERNEL_LLVM) not produced despite fj build exit 0"; exit 1; }` | Inspect Makefile diff | 0.1h |
| P3.2 | Add commit-msg-style regression test: temporarily break a source file, run `make build-llvm`, confirm the FAIL message + exit 1 | scripted: introduce temp syntax error → `make build-llvm; echo $?` → nonzero | 0.1h |
| P3.3 | Update `scripts/git-hooks/pre-commit` to fail on silent-build scenario (check 5/5) | after introducing temp error: `git commit` fails at pre-commit check 5 | 0.1h |

**Phase P3 total: 0.3h**
**Deliverable:** Silent-build-failure bug class closed permanently

### Phase V29.P1.P4 — FajarOS Retest + Honesty Retrospective

**Decision gate (Rule 6):** before P4, commit `docs/V29_P1_DECISION.md`
recording: keep @noinline (retest V28.5) OR revert @noinline (accept
V28.5 stability regression). Expected choice is KEEP since codegen
already supports it and P2 will have produced a working binary.

| # | Task | Verification | Est |
|---|------|--------------|-----|
| P4.1 | `docs/V29_P1_DECISION.md` committed before any retest work | `ls fajar-lang/docs/V29_P1_DECISION.md; git log --oneline -1 docs/V29_P1_DECISION.md` | 0.1h |
| P4.2 | Full FajarOS build+boot retest: `make clean && make iso-llvm && make test-serial` | Log contains `nova>` prompt + zero EXC:13 in first 50 tokens during Gemma 3 test | 0.2h |
| P4.3 | V28.5 multilingual retest with real @noinline: boot QEMU, run `ask`, capture output | ≥50 multilingual tokens with zero EXC:13 (V28.5 gate verification — this time with @noinline actually compiled in) | 0.2h |
| P4.4 | Update `fajaros-x86/docs/V28_5_CLOSED.md` with retroactive callout (box at top): "@noinline not active during original V28.5 multilingual demo; re-verified on <date> with @noinline compiled and present in ELF" | PR diff review | 0.1h |
| P4.5 | Update `fajaros-x86/CHANGELOG.md [3.4.0]` with addendum note | same | 0.1h |
| P4.6 | Update `memory/MEMORY.md` + `memory/project_v28_1_gemma3.md` with honest correction | memory diff review | 0.1h |
| P4.7 | Update `fajar-lang/CLAUDE.md` §3 Version History: note V27.5 compiler did not ship @noinline; V29.P1 added it retroactively | CLAUDE.md diff review | 0.1h |

**Phase P4 total: 1.0h**
**Deliverable:** FajarOS V28.5 multilingual success re-confirmed with @noinline actually active; all docs corrected with honest retrospective

### Phase V29.P1.P5 — Resume V29.P2.SMEP Step 2

This phase is **not part of V29.P1**. It's the handoff point. After
P4 completes, V29.P2.SMEP step 2 (boot auto-invoke + leak identification)
can resume on a sound foundation (compiler that builds, ELF that's
actually produced, audit trail that's honest).

No tasks in this phase — just a status marker for transition.

---

## 5. Effort Summary

| Phase | Tasks | Base | +25% buffer |
|-------|------:|-----:|-----------:|
| P0 Pre-flight | 5 | 0.3h | 0.4h |
| P1 Lexer+Parser | 7 | 1.0h | 1.3h |
| P2 Rebuild+verify | 5 | 0.5h | 0.6h |
| P3 Makefile gate | 3 | 0.3h | 0.4h |
| P4 FajarOS retest + retrospective | 7 | 1.0h | 1.3h |
| **TOTAL** | **27** | **3.1h** | **4.0h** |

**High-variance phases:** P1 (±40% uncertainty on parser/analyzer
touch) and P4 (retest may surface unrelated issues from 2h stale
kernel state).

---

## 6. Surprise Budget Tracking (Rule 5)

Per CLAUDE.md §6.8 Rule 5, every commit tags variance:

```
feat(v29-p1-p1.2): @noinline in ANNOTATIONS table
  [actual 0.15h, est 0.1h, +50%]

fix(v29-p1-p2.4): rebuild FajarOS kernel, ELF 1.47 MB
  [actual 0.2h, est 0.1h, +100%]
```

If phase average exceeds +25%, next phase budget escalates to +40%.

---

## 7. Prevention Layers (Rule 3)

Each phase installs at least one durable prevention mechanism:

| Phase | Prevention mechanism |
|-------|----------------------|
| P1 | `tests/codegen_annotation_coverage.rs` — meta-test iterates codegen match arms and fails if any annotation name is missing from lexer ANNOTATIONS. Future compiler contributors will see this fail before merging. |
| P3 | `Makefile:259` ELF gate — `fj build` success must be accompanied by ELF on disk or phase fails with exit 1. |
| P3 | `scripts/git-hooks/pre-commit` check 5/5 — silent-build-failure cannot leave dev machine. |
| P4 | V28_5_CLOSED.md retroactive callout preserves audit trail: future readers see BOTH the original claim AND the retrospective correction. |

---

## 8. Gates & Decisions (Rule 6)

| Gate | Before Phase | File |
|------|--------------|------|
| Pre-flight findings | P1 | `fajar-lang/docs/V29_P1_FINDINGS.md` (auto-generated at P0.5) |
| Keep/Revert decision | P4 | `fajar-lang/docs/V29_P1_DECISION.md` (explicit commit, tracked by hooks) |
| Retest passes | P5 handoff | P4.3 log committed to `fajaros-x86/docs/V28_5_RETEST.md` |

---

## 9. Risk Register

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|-----------|
| Parser needs non-trivial change to accept `AtNoInline` | Low | Medium | P1.4 test will surface; escalate to +40% budget |
| Analyzer has hidden context restriction for `@noinline` | Low | Medium | P1.5 test will surface; fallback = add context gate in analyzer |
| Kernel with @noinline + full V28.5 fix causes NEW regression | Low | High | P4.2 baseline test catches regressions; revert via P4 DECISION file if severe |
| Rebuild touches other compiler behavior (7,611 test regression) | Very Low | High | P2.2 full-suite test required; zero-regression gate |
| `make test-serial` hangs under new kernel (EXC:13 returns despite @noinline) | Medium | High | DECISION at P4 pre-gate — accept-and-document OR deeper debug escalation to V29.P2 research track |

---

## 10. Self-Check — Plan Hygiene Rule 6.8 (All 8)

```
[x] 1. Pre-flight audit mandatory                    — Phase P0 satisfies this
[x] 2. Verification commands runnable                — every task has literal shell command
[x] 3. Prevention layer per phase                    — P1 meta-test, P3 Makefile gate + hook, P4 retrospective
[x] 4. Multi-agent audit cross-check mandatory       — P1 findings cross-checked via direct `fj build` call in this plan
[x] 5. Surprise budget +25% minimum, tracked         — §6 tagged, escalation trigger defined
[x] 6. Decision gates mechanical files               — §8 lists 3 gate files
[x] 7. Public-facing artifact sync                   — P4 covers V28_5_CLOSED, CHANGELOG, memory, CLAUDE.md
[x] 8. Multi-repo state check                        — §2 enumerates all 4 repos/dirs (fajar-lang, fajaros-x86, fajarquant, memory)
```

All 8 YES = plan ships.

---

## 11. Author Acknowledgement

Honesty rule (CLAUDE.md §6.8 Rule 7 + user memory `feedback_honesty_upfront`):
this plan exists because the V28.5 multilingual success claim was partially
misattributed — the @noinline fix documented in commit `fajaros-x86@5670b4e`
was not compiled into the kernel that produced the multilingual output.
This plan closes the compiler gap so that subsequent V28.5-equivalent tests
actually exercise the @noinline stabilization, and installs prevention
mechanisms so similar silent-build-failures cannot recur.

The ~2-hour window between commit `5670b4e` and discovery was invisible
because the Makefile lied about build success. Closing that Makefile
gap (Phase P3) is the most important durable contribution of this plan —
it prevents the entire bug class for every future compiler feature.

---

*V29.P1 Compiler Enhancement Plan — committed 2026-04-16 as the first
deliverable of the phase, per Plan Hygiene Rule 1.*
