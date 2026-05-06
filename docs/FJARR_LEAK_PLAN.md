# Plan: _FjArr realloc leak closure

**Source: Plan agent dispatch 2026-05-07.** Read-only agent; saved here for review.

## Executive Summary (≈200 words)

`_fj_arr_new` and `_fj_arr_grow` in `stdlib/codegen.fj` lines 382–392 are the residual heap-leak class after R15 string-arena closure (commit `3a3dd586`). Every `[i64]`/`[str]` value in fj-emitted code mallocs an `_FjArr` struct plus a doubling `void**` buffer, and neither is ever freed. Closing this honestly is harder than R15 because: (a) the bump arena cannot host realloc (buffers may move); (b) per-array lifetime varies (struct fields, fn returns, shared references); (c) Strategic Compass §4.1 names `@kernel` no-heap as **non-negotiable**, so the chosen strategy must compose with — not undermine — a future heap-free kernel mode. This plan opens with a B0 pre-flight that quantifies the leak today via valgrind on the stage-2 fjc binary against its own source, enumerates five candidate strategies (per-program arena, RAII-style emission, refcount, linear-types-lite, opt-in `@scoped`) with explicit `@kernel` compatibility scoring, and surfaces the choice as a committed decision file `docs/decisions/2026-05-07-fjarr-leak-strategy.md` that pre-commit hooks can mechanically check (§6.8 R6). Verification gates use runnable valgrind commands plus the existing `phase17_stage2_native_triple_test` byte-equality invariant. Prevention layer adds a CI valgrind job + grep-guard against new `malloc(` literals in `emit_preamble`. Total estimate: 8–14h (B0 + decision) before any implementation; +25% surprise margin.

---

## §0 — Pre-flight Audit (B0 subphase, §6.8 R1)

| # | Question | Runnable command | What we record |
|---|---|---|---|
| B0.1 | How many `_FjArr` allocations does the stage-2 fjc binary leak when compiling its own source? | `cargo test --tests --no-run` then run the same chain as `tests/selfhost_phase17_self_compile.rs` (steps 1–3) to materialize `/tmp/fjc-triple-stage1`; then `valgrind --leak-check=full --show-leak-kinds=all /tmp/fjc-triple-stage1 /tmp/fjc_triple_combined.fj /tmp/leak_probe.c 2>&1 \| tee /tmp/leak_b01.log` | Bytes "definitely lost" + "indirectly lost" + count of `_FjArr` blocks (grep for `_fj_arr_new` in stack traces) |
| B0.2 | Per-allocation breakdown — struct vs buffer? | `grep -c "by 0x.*: _fj_arr_new" /tmp/leak_b01.log; grep -c "by 0x.*: _fj_arr_grow" /tmp/leak_b01.log` | Two integers; ratio confirms whether one or two leak classes |
| B0.3 | How many user-code call sites does this affect today? | `grep -nE "_fj_arr_new\\(\\)" /tmp/fjc_triple_stage1.c \| wc -l` | Number of array literals + grow chains |
| B0.4 | What about `_FjArr` in struct field, struct heap-allocated? | `printf 'fn main() { let v: [i64] = [1,2,3]; println(to_string(v.len())); }\n' > /tmp/leak_repro1.fj; cargo run -- run --emit-c /tmp/leak_repro1.fj > /tmp/leak_repro1.c; gcc /tmp/leak_repro1.c -o /tmp/leak_repro1; valgrind --leak-check=full /tmp/leak_repro1 2>&1 \| grep -E "definitely lost\|indirectly lost"` | Baseline: ≥48 bytes leaked |
| B0.5 | Array returned from fn — same? | Same as B0.4 with `fn make() -> [i64] { [1,2] }; fn main() { let v = make(); … }` | Confirms return-by-value sub-class |
| B0.6 | Sanity: R15 string-arena still 0 leaks? | `grep -E "(_fj_substring\|_fj_concat2\|_fj_arr_join_str\|_fj_to_string)" /tmp/leak_b01.log` | Should be empty |
| B0.7 | Cumulative leak in long-running consumer (STM32N6 niche)? | Loop reproducer 1000× + `valgrind --leak-check=full --error-exitcode=1` | Linear growth = unbounded P0 case |
| B0.8 | fjc-stage1 binary peak RSS? | `/usr/bin/time -v /tmp/fjc-triple-stage1 ... 2>&1 \| grep "Maximum resident"` | Anchor for risk-register |
| B0.9 | Phase 17 perf claim still holds? | `time cargo run --release -- run /tmp/fjc_triple_combined.fj > /dev/null; time /tmp/fjc-triple-stage1 ...` | Unchanged ratio |

**B0 deliverable:** `docs/SELFHOST_FJ_PHASE_18_B0_FINDINGS.md` — closes downstream gate. Pre-commit blocks downstream §1/§2 work until this file exists in `git ls-files`.

---

## §1 — Design Space + Decision Gate

### 1.1 Candidates Table

| ID | Strategy | Mechanism | `@kernel` (§4.1) compat | Implementation surface | Run-time cost | Compat with 95 self-host tests |
|----|----------|-----------|-------------------------|------------------------|---------------|--------------------------------|
| **A** | Per-program arena (no realloc — copy-on-grow into fresh arena chunk) | Allocate `_FjArr` struct + buffer from arena; on grow, alloc fresh + memcpy; freed at exit via existing `_fj_arena_free_all` | ❌ Hides heap dependency behind `atexit`. `@kernel` mode banning heap would still need this code path stripped — net: arena is heap. | Smallest — extend existing R15 arena with copy-grow path. ~30 LOC | One extra `memcpy` per grow (worst-case `O(N)` amortized constant). | High — preamble-only change; byte-equality preserved if generated text deterministic. |
| **B** | RAII-style emission (compiler emits `free(arr->data); free(arr)` at scope end) | Codegen tracks `[T]`-typed `let` bindings per scope; `END_BODY` emits inline `_fj_arr_free(name)` | ⚠️ Default still heap-on, but `@kernel` mode could refuse `_fj_arr_new` calls entirely → composes cleanly. Same model as Drop. | Largest — needs scope-stack in `codegen_driver.fj`, ownership tracking, move-out detection. | Zero alloc overhead; one extra `free` per binding. | Medium-risk — alters generated C. Byte-equality test will diff; needs full chain re-baseline. |
| **C** | Reference counting (`_fj_arr_retain` / `_fj_arr_release` at copy/drop) | Add `refcount` field to `_FjArr`; emit retain at assignment/param-pass, release at scope end | ❌ Refcount metadata is heap. Hidden runtime cost violates Compass §6.2 "RC dengan hidden cost menabrak semangat embedded." | Medium-large — needs analyzer to know "this is a copy site". | Real overhead: 1 atomic op per copy; cycle-leak class stays open. | High-risk — both byte-equality and runtime semantics drift. |
| **D** | Linear-types-lite (move-only `_FjArr`; double-use is a compile error; explicit `clone()` for sharing) | Type system: `[T]` is affine; analyzer rejects use-after-move; codegen emits `free` at last-use | ✅ **Best fit**. `@kernel` mode = same affine type system minus heap allocator. Composes by subtraction. Aligns with Compass §6.2 Hylo-style mutable value semantics. | Largest — analyzer change + codegen change. Needs SE017 use-after-move. | Zero overhead; identical to manual free. | Medium-risk — existing self-host source uses pass-by-value; some sites need `.clone()`. |
| **E** | Status-quo + opt-in `@scoped` annotation | Default keeps malloc/realloc; user opts in with `@scoped fn` for B-style auto-free | ⚠️ Default-leak in 2026 + 1y is still default-leak. Compass §4.4 "@safe sebagai default" rejects opt-in safety. | Small (B-style limited to annotated fns) | Zero (opt-in users only) | Low-risk — annotated fns are strict superset. |
| **F** | Hybrid: A as default + D for kernel mode | Default emit uses A. `@kernel` fn call sites compile-error on `_fj_arr_new`. Future Phase 19+ migrates default to D. | ✅ Stages cleanly: A buys time, D arrives later behind feature flag. | Phase 1 = small (A); Phase 2 = D. | A: low; D: zero. | A phase preserves byte-equality. |

### 1.2 Tradeoff Summary

- **A** cheapest but hides heap → undermines Compass §4.1.
- **B** honest but largest codegen change.
- **C** rejected by Compass §6.2 (RC violates embedded ethos).
- **D** strategically best fit, largest engineering scope.
- **E** entrenches default-leak — anti-pattern under §4.4.
- **F** sequences A→D, paying short-term cost for long-term linear-types win.

### 1.3 Decision Gate File

**Path:** `docs/decisions/2026-05-07-fjarr-leak-strategy.md`

Required sections (pre-commit hook greps for these literal headers):
1. `## Choice` — single line: `Choice: A | B | C | D | E | F`
2. `## Rationale (≥3 sentences)`
3. `## @kernel-future-compat` — yes/no + 2-sentence justification, citing Compass §4.1 / §6.2
4. `## Migration path` — staged roll-out keeping 95 self-host tests passing
5. `## Surprise budget` — `+25%` baseline; high-uncertainty bumps `+30%`
6. `## Rejected candidates` — one line each for the 5 not chosen
7. `## Reverse-cost` — what unwinding the choice costs

**This plan does NOT auto-decide.**

---

## §2 — Phased Task Breakdown

| Row | Action | File path(s) | Runnable verification | Surprise |
|-----|--------|--------------|-----------------------|----------|
| 18.0.1 | Run B0.1–B0.9 audit; commit findings | `docs/SELFHOST_FJ_PHASE_18_B0_FINDINGS.md` (new) | `test -f docs/SELFHOST_FJ_PHASE_18_B0_FINDINGS.md && grep -q "B0.1" $_ && grep -q "B0.7" $_` | +25% |
| 18.0.2 | Add regression baseline test | `tests/selfhost_fjarr_leak_baseline.rs` (new) | `cargo test --test selfhost_fjarr_leak_baseline -- --include-ignored 2>&1 \| grep "test result: ok"` | +25% |
| 18.0.3 | Authoring decision gate file | `docs/decisions/2026-05-07-fjarr-leak-strategy.md` (new) | `bash scripts/check_decision_file.sh docs/decisions/2026-05-07-fjarr-leak-strategy.md` | +30% |
| 18.0.4 | Add `scripts/check_decision_file.sh` | `scripts/check_decision_file.sh` (new) | `bash scripts/check_decision_file.sh docs/decisions/2026-05-07-fjarr-leak-strategy.md && echo OK` | +25% |
| 18.A.1 | (if A) Switch malloc → arena + copy-grow | `stdlib/codegen.fj` (edit lines 382–392) | (a) phase17_stage2_native_triple_test PASS (byte-equality holds) (b) `valgrind --leak-check=full --error-exitcode=1` 0 lost (c) full integ 10,489 PASS | +25% |
| 18.A.2 | Re-baseline leak test from current to 0 | `tests/selfhost_fjarr_leak_baseline.rs` (edit) | leak_baseline test PASS | +25% |
| 18.B.1 | (if B) Add scope-stack + emit free at END_BODY | `stdlib/codegen_driver.fj` (edit), `stdlib/codegen.fj` (preamble adds `_fj_arr_free`) | (a) phase17 will diff — must rebaseline (b) valgrind 0 lost (c) integ 10,489 PASS | +30% |
| 18.B.2 | (if B) Move-out detection on `return v`/`let y = x` | `stdlib/codegen_driver.fj` (edit) | No double-free reproducer test | +30% |
| 18.D.1 | (if D) Affine `[T]`; SE017 UseAfterMove; `.clone()` builtin | `src/analyzer/*.rs`, `stdlib/codegen.fj` (preamble +_fj_arr_clone) | (a) lib 7,629 PASS (b) new SE017 test (c) phase17 PASS after rebase | +30% |
| 18.D.2 | (if D) Codegen emits free at last-use of affine `[T]` | `stdlib/codegen_driver.fj` (edit) | valgrind 0 lost | +30% |
| 18.F.* | (if F) Phase 18.A rows + Phase 18.D rows tagged deferred Phase 19 | combination | All 18.A verifications + CHANGELOG entry naming Phase 19 | +30% |
| 18.Z.1 | Update CLAUDE.md §3 with v35.1.0 entry | `CLAUDE.md` (edit) | `grep -q "v35.1.0" CLAUDE.md && grep -q "_FjArr" CLAUDE.md` | +25% |
| 18.Z.2 | Phase 18 closure findings doc | `docs/SELFHOST_FJ_PHASE_18_FINDINGS.md` (new) | `grep -E "definitely lost: 0\|md5" docs/SELFHOST_FJ_PHASE_18_FINDINGS.md` | +25% |

**Phase ordering:** 18.0.1 → 18.0.2 → 18.0.3 → 18.0.4 → (decision) → 18.{A,B,D,F}.* → 18.Z.1 → 18.Z.2.

---

## §3 — Prevention Layer

1. **Pre-commit hook** (`scripts/git-hooks/pre-commit`):
   - If staged diff touches `stdlib/codegen.fj` lines ~380–415, require `docs/decisions/2026-05-07-fjarr-leak-strategy.md` exists at HEAD.
   - Reject any new `malloc(`/`realloc(` literals inside `emit_preamble` strings unless commented `// LEAK_BUDGET_EXCEPTION:`.

2. **CI valgrind job** (`.github/workflows/leak-gate.yml`):
   - Triggers on PRs touching codegen.fj/codegen_driver.fj.
   - Asserts `definitely lost: 0` AND `indirectly lost: 0` via `valgrind --error-exitcode=1`.

3. **CLAUDE.md §6.12 (new):** "Heap allocations in `emit_preamble` must either flow through `_fj_arena_alloc` OR pair with explicit free path documented in the same commit."

---

## §4 — Risk Register

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|-----------|
| `phase17_stage2_native_triple_test` byte-equality fails | High (any preamble text change) | Blocks v35.1.0 release | Strategy A is text-only, deterministic; preserves byte-equality. B/D require explicit re-baselining. |
| Self-host test regression | Medium | Blocks closure | Run full integ at every row. Stage A first (smallest blast radius). |
| `_FjArr` referenced in test harnesses | Medium | Hidden dep | `grep -rn "_FjArr\|_fj_arr_" tests/ src/` before edit. |
| valgrind unavailable on CI | Low | CI flake | `apt-get install valgrind`; fallback `-fsanitize=address`. |
| Strategy D requires `.clone()` insertions in self-host source | Medium-High | Cascade re-baselining | Audit `[T]` reuse sites BEFORE D: `grep -E "let.*: \\[" stdlib/*.fj \| wc -l`. If >50, prefer F. |
| RSS regression from arena retention | Low | Negligible | Compare to B0.8 RSS anchor; flag if >10%. |
| FajarOS-x86 depends on emit_preamble shape | Medium | Cross-repo break | Multi-repo state check before commit. |
| Future `@kernel` mode invalidates strategy A | Low (deferred) | Need redo A→D | F explicitly stages this. Document in decision file's "Reverse-cost". |
| Public-artifact drift (README/CHANGELOG/release notes) | Medium | Misleading | Row 18.Z.1 audits in same session. |

---

## §5 — Total Budget Estimate

| Phase | Strategy A | Strategy B | Strategy D | Strategy F |
|-------|-----------|-----------|-----------|-----------|
| B0 audit + decision gate (rows 18.0.*) | 3h | 3h | 3h | 3h |
| Implementation (row 18.X.*) | 1h | 4h | 8h | 1h (A only) |
| Verification + baseline test rebase | 1h | 2h | 2h | 1h |
| Findings doc + CLAUDE.md sync | 1h | 1h | 1h | 1h |
| **Subtotal** | **6h** | **10h** | **14h** | **6h** |
| **+25% surprise (default)** | **7.5h** | **12.5h** | — | **7.5h** |
| **+30% (high-uncertainty: D)** | — | — | **18h** | — |

Recommended budget upper bound: **8–18h Claude time**, depending on §1 decision. If F (A-now, D-Phase-19): commit to 8h for v35.1.0, add Phase 19 (~14h) to roadmap.

---

### Critical Files

- `/home/primecore/Documents/Fajar Lang/stdlib/codegen.fj`
- `/home/primecore/Documents/Fajar Lang/stdlib/codegen_driver.fj`
- `/home/primecore/Documents/Fajar Lang/tests/selfhost_phase17_self_compile.rs`
- `/home/primecore/Documents/Fajar Lang/scripts/git-hooks/pre-commit`
- `/home/primecore/Documents/Fajar Lang/docs/1/STRATEGIC_COMPASS.md`
