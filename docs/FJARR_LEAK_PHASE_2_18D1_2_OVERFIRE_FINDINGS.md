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

| Metric | Value |
|---|---|
| SE024 occurrences in single stage1_full run | **12,384** |
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
| **E1** Chain-grow re-assign reset | ~1-2h | `args = args.push(x)` should declare-fresh on LHS rebind. Touch let-stmt path + assign-stmt path in `check.rs`. |
| **E2** Method-receiver consume tracking | ~2-3h | When `arr.method(...)` is checked + `arr_ty: [T]`, mark `arr` moved (or the equivalent if method takes `&self`/`&mut self`). Need to know each method's self-kind for `[T]`. |
| **E3** Branch-merge analysis | ~3-4h | Per-branch MoveTracker scope; merge at end-of-if. Already partially handled for nested fns; needs scope-savepoint API addition. |
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
