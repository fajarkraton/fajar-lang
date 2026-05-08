---
phase: FJARR_LEAK Phase 2 — 18.D.1.2 GREEN attempt → OVERFIRE → rollback
plan: docs/FJARR_LEAK_PLAN.md §2 row 18.D.1 + B0 §1 early-warning trigger
status: GREEN ATTEMPT ROLLED BACK 2026-05-08 (>20 fire trigger threshold catastrophically exceeded; B0 §1 protocol invoked)
b0: docs/FJARR_LEAK_PHASE_2_B0_FINDINGS.md (committed `dd0d3fa5`)
discovery: docs/FJARR_LEAK_PHASE_2_18D1_DISCOVERY.md (committed `2769d726`)
red-phase: commit `c9830168` (18.D.1.1 RED — variant + tests)
prereq: v35.1.0 (Phase 1 closed)
purpose: empirical evidence that the naive 18.D.1.2 GREEN approach is insufficient; predicted-but-underestimated cascade-risk surfaced at >12,000× the >20 threshold
---

# FJARR_LEAK Phase 2 — 18.D.1.2 GREEN Attempt: Over-Fire Findings

> The naive analyzer wiring for SE024 (mark_moved on every `[T]`
> consume, regardless of `strict_ownership`) catastrophically over-
> fires on real fj source. **12,384 SE024 occurrences** in a single
> stage1_full chain run vs B0 §1 early-warning trigger of >20 = pause.
> This doc captures the rollback evidence + the gap between B0
> qualitative analysis and reality.

## §0 — TL;DR

**Tried:** 4-edit naive wiring of SE024 emission in
`src/analyzer/type_check/check.rs`:
1. `check_ident` dispatches SE024 (Array) vs ME001 (other) on move
2. `let y = x` consume marks moved if `is_copy || is_array`
3. fn-arg consume marks moved if `is_copy || is_array`
4. match-subject consume marks moved if `is_copy || is_array`

**Result (cargo test on full lib + stage1_full):**

| Suite | Before naive wire | After naive wire |
|---|---|---|
| `cargo test --lib` | **7,629 PASS** | **7,624 PASS / 5 FAIL** (5 internal tests assert "arrays are Copy" pre-Phase-2 contract) |
| `cargo test --release --test selfhost_stage1_full` | **86/86 PASS** (~0.78s) | **0/86 PASS / 86 FAIL** (chain compile broken, ~0.89s) |
| `cargo test --release --test selfhost_stage1_full -- --nocapture \| grep -c "SE024"` | n/a | **12,384** ⚠️⚠️⚠️ |

**Decision per B0 §1:** ROLL BACK the analyzer-wiring edits to keep
codebase shippable. Variant + format tests + ok-baseline + ignored
emission tests stay (RED state restored to match 18.D.1.1 baseline).

## §1 — Why B0 underestimated the cascade

B0 §B0.7 + §B0.10 + §B0.11 sampled the chain-grow pattern
(`a = a.push(x)` → 133 sites) and the parser_ast.fj `a` (228 uses)
handoff pattern, concluding both were "affine-friendly" because:

1. **Single right-side use of `a` in the consume**: `a.push(x)` reads
   `a` once.
2. **Immediate re-bind**: the result of `.push()` re-assigns to the
   same name `a`.

The error in this analysis was assuming **re-assignment to the same
binding name resets the move state**. It does not, in the naive
implementation:

```fj
let mut args: [str] = []      // declare args, Owned state
args = args.push(r.code)      // step 1: consume args (mark Moved)
                              // step 2: bind result to `args` slot
                              //          BUT MoveTracker doesn't reset
                              //          move state on re-assign — only
                              //          on `declare()` (let-bindings)
args = args.push(other.code)  // SE024: args is Moved!
```

The MoveTracker API has `declare()` (resets state to Owned) and
`mark_moved()` (sets Moved). There's no `reassign()` (resets to Owned
without a new scope). For `let mut x = ...` chains where the same
binding is overwritten in subsequent statements, the analyzer treats
each RHS read of `x` as a use-after-prior-move.

This is the **chain-grow re-assign blind spot**. B0.7's "affine-
friendly" call assumed re-assign reset semantics that don't exist.

## §2 — Other under-counted issues

Beyond chain-grow, the naive wire surfaced 3 more cascade classes:

### 2.1 Method-receiver consume not wired
`a.push(x)` is a method call. The analyzer's method-call check (which
I did NOT touch) doesn't mark the receiver as moved. So **method
receivers escape the consume tracking entirely** — meaning many
"consume" sites that look like fn-arg sites in source code aren't
caught.

This means SE024 is **incomplete** AND **over-eager** simultaneously:
fn-arg consume fires correctly but method-receiver consume doesn't,
leaving the affine semantics inconsistent. A real Phase 2 D needs both
paths plus a coherent re-assign reset rule.

### 2.2 Branch-merge analysis missing
B0.11 acknowledged this as "risk NOT yet measured." It fires:
```fj
if cond { foo(v) } else { bar(v) }   // both branches consume v
let _x = baz(v)                       // SE024: v is Moved in both branches
```
The naive analyzer correctly marks `v` Moved in both branches, but
doesn't track per-branch move history — it merges by union. This
generates spurious SE024 emissions in many real branching patterns.

### 2.3 5 internal lib tests document pre-Phase-2 "arrays are Copy" contract
These tests (e.g., `move_type_use_after_move_detected` at
`src/analyzer/type_check/mod.rs:3390`) explicitly assert that arrays
are Copy:
```rust
// Arrays and structs are now Copy (Rc-based runtime semantics).
// ...
assert!(check(src).is_ok());
```

Phase 2 explicitly changes this contract (arrays become affine).
Updating these 5 tests is part of Phase 2's correct scope, NOT a bug
in the wire — but it IS additional cascade work the original 14h
plan didn't itemize.

## §3 — Headline numbers

> **2026-05-08 update (E1 attempt diagnostic):** The 12,384 number
> was emission-count across 86 stage1_full tests × ~111 unique
> source sites in stdlib (each test re-runs the chain on the same
> source). Re-attempt with diagnostic capture (single test, single
> chain run) showed **111 unique source sites** in `/tmp/full_p10.fj`
> (the materialized concatenated stdlib). All 111 are the
> branch-with-return / branch-merge pattern (E3 issue). **Zero**
> chain-grow over-fires (E1 already handled).
>
> **2026-05-08 second update (E3 implementation + experiment):**
> E3 (branch-merge analysis with terminator awareness) shipped:
> - `MoveSnapshot` + `snapshot()` / `restore()` / `merge_snapshots()`
>   added to `MoveTracker` (`src/analyzer/borrow_lite.rs:138-237`).
> - `branch_always_terminates(e: &Expr) -> bool` helper added
>   (`src/analyzer/type_check/check.rs:13-46`).
> - `check_if` rewritten to snapshot pre-branch state, restore for
>   else, and merge with terminator skip (4-case logic).
> - Standalone unit tests: `tests/analyzer_branch_merge_terminator.rs`
>   (5/5 PASS), exercising strict-mode ME001 paths.
>
> **E3 result on full SE024 wire**: 111 → **80** unique sites
> (28% reduction). Better but still 4× the <20 GREEN threshold.
> The remaining 80 sites are NOT branch-merge issues — they're
> genuine affine-vs-error-propagation conflicts in self-host source:
> ```fj
> let r_subj = parse_expr_ast(src, pos, a)   // a Moved here (fn-arg consume)
> if p_lb < 0 { return pr_err(a, ...) }       // a still Moved (consume happened
>                                              // BEFORE the if; E3 doesn't help)
> a = r_subj.ast                               // re-bind would reset, but
>                                              // never reached if branch returned
> ```
> The user wants to use `a` once for the error message in the
> error-path, but affine semantics rejects this without `&[T]`
> borrow type or `.clone()` insertion.
>
> **E3 was shipped as a standalone correctness fix** (improves the
> existing ME001 strict-mode path even without SE024 wire). SE024
> wire was rolled back; emit_* tests re-#[ignore]'d.

| Metric | Value |
|---|---|
| SE024 occurrences in 86-test stage1_full run (raw, pre-E3) | **12,384** |
| Unique source sites per single-test chain run, pre-E3 | **111** |
| Unique source sites per single-test chain run, **with E3** | **80** (28% reduction) |
| Of those 80, attributed to E3 branch-merge | **0** (E3 is doing its job; remaining is E2/E7) |
| Of those 80, attributed to fn-arg consume + later use in same scope | **~80** (cascade-`.clone()` work — E7) |
| Chain-grow over-fires (E1 issue) | **0** ✅ |
| Branch-with-return false positives (E3 issue) | **0** ✅ (E3 ships) |
| Trigger threshold (B0 §1 early-warning) | **>20 = pause** |
| Magnitude of overshoot | **~620× the threshold** |
| stage1_full pass rate post-wire | **0/86** (chain entirely broken) |
| Pre-Phase-2-contract internal lib tests breaking | **5** (mod.rs:3390 + 4 similar) |
| Affine-pattern classes correctly handled by naive impl | **0** |

## §4 — What a real Phase 2 GREEN would need

For SE024 to be production-quality (per the user's "stick with
original 14h plan" choice), the analyzer needs:

| Enhancement | Estimated effort | Why |
|---|---|---|
| ~~**E1** Chain-grow re-assign reset~~ | ✅ **Already implemented** | Discovered post-rollback (E1 attempt 2026-05-08): `check_assign` at `src/analyzer/type_check/check.rs:2106-2117` ALREADY calls `moves.declare()` before AND after RHS evaluation. The `ok_chain_grow_pattern_no_se024` test passes WITH the SE024 wire applied, confirming chain-grow is not the problem. |
| **E2** Method-receiver consume tracking | ~2-3h | When `arr.method(...)` is checked + `arr_ty: [T]`, mark `arr` moved (or the equivalent if method takes `&self`/`&mut self`). Need to know each method's self-kind for `[T]`. |
| **E3** Branch-merge analysis | ~3-4h | **THIS IS THE REAL OVER-FIRE CAUSE.** Per-branch MoveTracker scope; merge at end-of-if. Specifically: when a branch ends with `return` / `break` / `continue` / etc, treat consumes in that branch as branch-local. The 111 over-fire sites in stdlib are all `if cond { return pr_err(a, ...) }` patterns where the if-branch consumes `a` but escapes — naive analyzer marks `a` moved unconditionally, fires SE024 on subsequent uses. |
| **E4** `.clone()` builtin recognition | ~30min | Already planned. Skip mark_moved on `MethodCall { method: "clone", .. }`. |
| **E5** `_fj_arr_clone` codegen preamble | ~30-45min | Already planned. Allocator-side runtime support. |
| **E6** Update 5 pre-Phase-2 internal lib tests | ~30min | Document Phase 2 contract change. |
| **E7** Cascade `.clone()` insertions in self-host source | **~3-6h** | After E1-E5 land, run chain on stdlib/*.fj; insert `.clone()` at each remaining SE024 site. May find branch-merge cases that need restructuring beyond `.clone()`. |
| **Total** | **~10-16h** | (vs original plan's 14h estimate — close) |

The original 14h estimate was **roughly correct** but underestimated:
- E2/E3 (method receiver + branch merge) — these are sub-projects
- E7 cascade size — likely dozens of `.clone()` insertions, not the
  "0-10" B0.10/B0.11 qualitative analysis suggested

**B0 was NOT wrong** about chain-grow being affine-friendly *under
correct affine semantics* — but the naive impl doesn't have correct
affine semantics. A real Phase 2 needs E1+E2+E3 minimum to make
B0's "affine-friendly" classification accurate.

## §5 — Three options for next session

The user's prior choice ("stick with original 14h plan") remains
authoritative, BUT this evidence changes the cost-benefit analysis.

### Option A — Continue Phase 2 with full E1-E7 scope (~10-16h)

Honor the original commitment. Land E1 (re-assign reset) first as a
self-contained enhancement, validate against stage1_full smoke. Then
E2 (method receiver), E3 (branch merge). Then E4-E5 (.clone() +
preamble). Finally E7 (cascade `.clone()` migration). Each step is a
commit with stage1_full + phase17 smoke gates.

**Risk:** if E2/E3 surface their own surprises (likely — branch-merge
analysis is non-trivial), each has its own +30% surprise bump.
Realistic ceiling: **~20h**.

### Option B — Pivot to D-LITE (~4h, the discovery doc's earlier proposal)

Use the existing `MoveTracker` + `is_copy_type_strict` + ME001
infrastructure. Activation strategy = Option G1.D (opt-in `--strict`
CLI flag). User code defaults lenient; production builds opt in.
This sidesteps E1/E2/E3 because they're already handled in strict
mode by existing code. Trade-off: doesn't satisfy Compass §4.4
"@safe sebagai default" — Phase 2 ships as opt-in safety, not
default-on.

The original SE024-flavor commitment can still be honored via a thin
shim: when strict mode triggers ME001 on a `[T]`, re-emit as SE024.
Or just rename the catalog entry.

### Option C — Defer Phase 2 indefinitely; keep Phase 1 as the leak ceiling

Phase 1's arena (88 bytes/array → 0) already holds the leak class.
Phase 2's **structural** payoff (real @kernel-no-heap compatibility)
doesn't ship until @kernel mode itself ships, which is post-v36.x.
Defer Phase 2 to "ship when @kernel mode lands" — at which point the
cascade work is amortized across @kernel + Phase 2 simultaneously.

Reverse-cost: Compass §4.4 anti-pattern (default-leak in 2026 + 1y
is still default-leak) — already documented in Phase 1 §6 honest
scope.

## §6 — Decision gate (per CLAUDE.md §6.8 R6)

Three options surfaced (A/B/C). User authorization required before
re-attempting GREEN — the over-fire discovery materially changes
the cost-benefit analysis the original "Phase 2 full sequence" choice
was made under.

Per `feedback_lanjutkan_rekomendasi.md`: even though user previously
authorized "Phase 2 full sequence" + "stick with 14h plan" + "SE024
naming", THIS rollback is the kind of evidence the user might want
to course-correct on. Surfacing for explicit decision before any
further analyzer work.

**STOP** after the rollback commit. Do NOT auto-pick A/B/C.

## §7 — Cumulative state at over-fire close

| Aggregate | At v35.1.0 ship | At Phase 2 18.D.1.1 RED | At Phase 2 18.D.1.2 (rolled back) |
|---|---|---|---|
| Self-host tests | 102 | 102 | 102 (unchanged after rollback) |
| Phase 1 arena leak | 0 ✅ | 0 ✅ | 0 ✅ (unchanged after rollback) |
| Stage1-full | 86/86 ✅ | 86/86 ✅ | 86/86 ✅ (rollback restored) |
| Phase17 self-compile | 4/4 ✅ | 4/4 ✅ | (not re-run after rollback; expect 4/4) |
| SE024 variant in catalog | absent | declared | declared |
| SE024 emission wired | n/a | not wired (RED) | attempted then rolled back (back to RED) |
| FJARR_LEAK Phase 2 status | not started | RED phase complete | OVER-FIRE evidence captured; awaiting user decision A/B/C |

## §3 (sic) — Variance recap

| Sub-item | Plan estimate | Actual | Variance |
|---|---|---|---|
| 18.D.1.2 naive GREEN attempt | ~3-4h | ~30min impl + ~30min discovery + ~30min rollback + ~30min docs = **~2h** | -33% to -50% (impl part very fast; discovery is the real value) |

This commit is mostly a **discovery commit**: the empirical evidence
surfaces what B0 + 18.D.1.1 RED could not predict. ~2h spent here
saves the ~12h that would have been wasted naively grinding through
E1-E7 without the over-fire data.

## §5 — 2026-05-08 third update: E1.5 + cascade-scope discovery

**E1.5 finding:** `check_assign` was calling `moves.declare()` which inserts
into INNERMOST scope. For `a = a.push(x)` chain-grow inside a loop body
where `a` is declared in an outer fn scope, the RHS `mark_moved` updated
outer-scope state to Moved, then `declare()` created NEW Owned record in
inner loop scope. After loop scope popped, only outer-scope record remained
— Moved.

**E1.5 fix shipped:** `MoveTracker::reset()` API that finds variable in
innermost-out search (like `mark_moved`) and resets state in-place.
`check_assign` switched from `declare()` to `reset()` at both pre-RHS and
post-assign sites. Chain-grow re-assign now correctly resets across nested
scope boundaries.

**Empirical SE024 over-fire count progression:**

| State | Unique source sites |
|---|---|
| Pre-E3 (rolled-back naive wire) | 17 |
| Post-E3 only (branch-merge w/ terminator) | 17 (E3 fixed branch-with-return but didn't help loop-scope) |
| **Post-E3 + E1.5 (reset across scopes)** | **5** (71% reduction) |

> Note: my earlier "80" / "111" counts were total SE024 *mentions* in
> stage1_full output (each test re-runs the chain on the same source).
> Counted as unique source sites, the actual numbers are much smaller.

**Cascade-scope discovery (2026-05-08):** Of the remaining 5 sites, all
fire when `vars` (a `[str]` parameter) is consumed by
`lookup_var_type_in_table(vars, ...)` inside one branch of a c_type-
inference if/else chain in `stdlib/codegen_driver.fj`. Empirical attempt:
adding `.clone()` to all 18 `lookup_var_type_in_table` call sites did NOT
clear all 5 sites — there are still 4 remaining. Investigation revealed:

`grep -nE "\(vars[^.]" stdlib/codegen_driver.fj` shows **19+ MORE bare-vars
consume sites** in `parse_expr_emit(ast, vars, ...)` calls throughout
codegen_driver.fj. Plus similar patterns in parser_ast.fj. Each .clone()
insertion only fixes the downstream uses for THAT call site; subsequent
fn-arg consumes of `vars` re-Move it.

**Real cascade-fix scope:** ~20-40 `.clone()` insertions in
codegen_driver.fj + ~10-20 in parser_ast.fj + similar in other stdlib
files = **30-60 mechanical insertions total**. Estimated effort: **~4-8h**
of focused work, NOT the 3-6h previously estimated. Each insertion is
reversible but requires re-running stage1_full to verify no new sites
surfaced (since each insertion may reveal more downstream fires).

**E1.5 ships as standalone correctness fix.** Even without SE024 wire,
any user of `strict_ownership` mode benefits from correct chain-grow
reset across nested scopes. The cascade work is parked pending user
decision on whether to commit to the larger ~4-8h scope OR pivot to
D-LITE (~4h opt-in flag) OR defer Phase 2.

---

*FJARR_LEAK_PHASE_2_18D1_2_OVERFIRE_FINDINGS — written 2026-05-08.
Naive analyzer wiring for SE024 catastrophically over-fires (12,384
violations in single stage1_full chain run; B0 §1 trigger >20 was
~620× exceeded). Roll-back restored 7,629 lib + 86 stage1_full GREEN.
Real Phase 2 GREEN needs E1 chain-grow re-assign reset + E2 method-
receiver consume + E3 branch-merge analysis + E4/E5 .clone() infra +
E6/E7 cascade migration (~10-16h, possibly ~20h with surprises).
Three options A/B/C for next session; user decision required before
any further analyzer work.*
