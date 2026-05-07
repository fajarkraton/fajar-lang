---
decision: _FjArr realloc-leak strategy
date: 2026-05-07
status: ACCEPTED 2026-05-07 (batch-approved with companion D-files)
prereq: docs/FJARR_LEAK_PLAN.md, docs/FJARR_LEAK_B0_FINDINGS.md
plan: docs/FJARR_LEAK_PLAN.md §1
---

# FJARR_LEAK Decision — _FjArr realloc-leak strategy

## Choice

**Choice: F** (hybrid: A-now arena + D-Phase-19 linear-types)

> **Status: ACCEPTED 2026-05-07.** Batch-approved with the two companion
> decision files. Strategies `C` (refcounting) and `E` (opt-in @scoped)
> remained pre-rejected per kompas §6.2 + B0.7 evidence.

## Rationale (≥3 sentences)

Strategy F **stages cleanly**: ship the cheap-and-fast Strategy A
arena-with-copy-grow now (~8h total including B0 + decision overhead)
to get the immediate ~2.73 MB-per-fjc-self-compile-run leak under
control, while explicitly committing the project to Strategy D
(linear-types-lite affine `[T]`) as Phase 19 work for the long-term
@kernel-future-compat win.

Strategy A alone (no D follow-on) hides the heap dependency — the
arena IS heap, just freed at exit. Strategic Compass §4.1 says
@kernel must reject heap **at compile time**, not just "free at
exit." Strategy A by itself would compose poorly with a future
@kernel mode.

Strategy D alone is the structurally-best answer (zero overhead, real
@kernel compat, aligned with Compass §6.2 Hylo-style mutable value
semantics) but requires:
- Analyzer change to reject `[T]` reuse-after-move (new SE017 code)
- Codegen change to emit `free` at last-use of every affine binding
- Possible `.clone()` annotations in self-host source itself (audit
  count needed before commit per plan §4 risk register entry)

That's ~14h work in one commit, high risk to the 95-test self-host
green-board, and the @kernel mode that benefits doesn't ship until
later phases anyway. F sequences A→D so we get B0.7's unbounded-growth
class fixed within the session, with D's structural elegance as a
named future commit.

The B0 audit (`docs/FJARR_LEAK_B0_FINDINGS.md`) confirmed the leak is
real (2.73 MB / 53,818 blocks per run, exactly linear unbounded
growth at 88 bytes/array per iteration). Strategy E (opt-in `@scoped`)
was **pre-rejected** by B0.7 evidence: opt-in safety leaves the
default-unbounded-growth in place, which is a non-starter for the
embedded niche named in Compass §3.1 (STM32N6 / Cortex-M55 / ESP32 /
Hexagon long-running consumer programs).

## @kernel-future-compat

**Compatible: yes (with caveat)**

Strategy F's Phase-1 (A-now arena) is **compatible-by-deferral** with
@kernel: a future @kernel mode can simply forbid `_fj_arr_new`
allocations entirely, and user code in @kernel uses stack-allocated
`[T; N]` fixed arrays (covered by separate plan, not this one).
Strategy F's Phase-2 (D-later linear types) is **compatible-by-design**:
the same affine type system carries over to @kernel mode minus the
heap allocator.

Honest caveat: between F's Phase-1 ship (target v35.1.0) and Phase-2
ship (target v36.x or later), the project IS in the "default heap-
freed-at-exit" state for `[T]` arrays in @safe / @device. Embedded
niche users targeting STM32N6 today would need to use `[T; N]` fixed
arrays exclusively; dynamic `[T]` arrays in their hot path leak until
exit. This is documented honestly, not hidden.

## Migration path

### Phase 1 (this session, target v35.1.0)

1. **Pre-flight committed:** `docs/FJARR_LEAK_B0_FINDINGS.md`
   (already committed in commit `f5448b03`).

2. **18.A.1**: Switch `_fj_arr_new` and `_fj_arr_grow` in
   `stdlib/codegen.fj` `emit_preamble` to allocate via existing
   `_fj_arena_alloc` (added in R15 closure commit `3a3dd586`).
   `_fj_arr_grow` becomes copy-grow (alloc fresh larger, memcpy old
   data, no realloc). ~30 LOC change.

3. **18.A.2**: Re-baseline regression test
   `tests/selfhost_fjarr_leak_baseline.rs` (new file) from current
   88-bytes-per-array to **0 still-reachable** under valgrind.

4. **18.0.4 prevention**: pre-commit hook addition + CI valgrind
   gate per plan §3.

5. **18.Z**: CLAUDE.md update with v35.1.0 entry; close-section in
   `docs/SELFHOST_FJ_PHASE_18_FINDINGS.md`.

### Phase 2 (Phase 19, deferred — target v36.x)

6. **18.D-style work** as Phase 19 entry on roadmap:
   - Audit `[T]` reuse sites in self-host source (`grep -E "let.*: \\["
     stdlib/*.fj | wc -l`). Plan §4 R5 risk: if >50 sites, expect
     cascade re-baselining.
   - Analyzer changes to introduce SE017 UseAfterMove for `[T]`.
   - Codegen emits `free(arr->data); free(arr)` at last-use; arena
     becomes legacy code path that gets feature-flagged off.
   - `.clone()` builtin for explicit deep-copy.

This staging is documented in CHANGELOG with target month for Phase 19
work so we don't pretend it ships in Phase 1.

All 95 existing self-host tests + future Phase 18 leak-baseline test
must remain green throughout.

## Surprise budget

**Phase 1: +25% baseline** (Strategy A is text-only preamble change,
B0 audit confirmed all infrastructure exists, low uncertainty).

**Phase 2 (deferred): +30% high-uncertainty** (Strategy D needs SE017
analyzer code-path, .clone() builtin, possible self-host re-baseline,
multiple unknowns).

Phase 1 budget: 6h base × 1.25 = **7.5h cap**. With B0 + decision-doc
overhead: **8h cap** total to v35.1.0 ship.

Variance tag: `fix(fjarr-leak-phase1): close R15-array via arena
copy-grow [actual Xh, est 6h, +Y%]`.

## Rejected candidates

- **A alone (per-program arena, no D follow-on)**: cheapest but hides
  heap dependency behind `atexit`. Strategic Compass §4.1 says
  @kernel must enforce no-heap at compile time. A-alone composes
  poorly. Used as Phase 1 of F, NOT as standalone strategy.

- **B (RAII-style emission, free at scope end)**: structurally honest
  about lifetimes but largest codegen change in one commit. Move-out
  detection on `return v` and `let y = x` is a significant codegen
  enhancement. Plan §2 row 18.B.* tags +30% surprise. Better delivered
  through D (which subsumes B's end-of-scope free with affine-type
  semantics).

- **C (reference counting via `_fj_arr_retain`/`_fj_arr_release`)**:
  pre-rejected per Compass §6.2 quote: "RC dengan hidden cost
  menabrak semangat embedded." Cycle leak class stays open, hidden
  per-copy atomic op overhead. **Not viable for embedded niche.**

- **D alone (linear types in single commit)**: structurally best fit
  but largest engineering scope. ~14h work + cascade risk to self-host
  source + 95-test green board. Ship as F's Phase 2 instead, after
  v35.1.0 demonstrates A's correctness.

- **E (opt-in `@scoped` annotation)**: pre-rejected per B0.7.
  Default-unbounded-growth for any user who doesn't opt in violates
  Compass §4.4 ("@safe sebagai default") and is non-viable for
  embedded niche where the leak is the killer feature. **Not viable
  even with B0.7 evidence.**

## Reverse-cost

**Phase 1 (Strategy A): low.** The arena is a ~30 LOC preamble change.
If Strategy A turns out to interact badly with Phase 2 (D linear types)
when implementation lands, the arena code is removed in same commit
that wires the affine type system. No external-facing API changes.

**Phase 2 (Strategy D): high.** Once linear types ship, reverting them
means re-introducing the leak class. By then, however, the dependency
on D's no-leak semantics has spread (user code may rely on `[T]`
being affine; reverting would silently re-permit shared mutation).
Phase 2 is "one-way door." Hence the careful staging — A-first to buy
time, D when the project is ready to commit.

**Stage 2 byte-equality:** Phase 1 (A) is text-only and deterministic
→ preserves byte-equality (md5 unchanged). Phase 2 (D) WILL change
emitted C (free calls inserted) → md5 must be re-baselined; that's
documented at plan-time, not surprise.

---

*ACCEPTED 2026-05-07. Phase 1 (A-now arena) implementation proceeds
per migration path §3 above. Phase 2 (D-later linear types) deferred
to v36.x roadmap entry. Variance tracking per §6.8 R5: tag Phase 1
commit with `[actual Xh, est 6h, +Y%]`.*
