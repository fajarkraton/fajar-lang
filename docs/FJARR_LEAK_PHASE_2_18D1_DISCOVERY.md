---
phase: FJARR_LEAK Phase 2 — 18.D.1 prep discovery (analyzer infrastructure already exists)
plan: docs/FJARR_LEAK_PLAN.md §2 row 18.D.1 + decision file §Migration-path Phase 2
status: DISCOVERY 2026-05-08 (no code changes yet; surfacing to user before pivot)
b0: docs/FJARR_LEAK_PHASE_2_B0_FINDINGS.md (committed `dd0d3fa5`)
purpose: surface that the original 14h Phase 2 plan dramatically over-scoped — affine-array infrastructure already exists in the Rust analyzer; gap is much smaller (~2-4h, not 14h)
---

# FJARR_LEAK Phase 2 — 18.D.1 Discovery Note

> The plan called for building "linear-types-lite affine `[T]`" from
> scratch (~14h estimate). 18.D.1 prep exploration revealed the
> infrastructure is **already 90% built**: `MoveTracker`, `OwnershipState`,
> `is_copy_type_strict`, and `ME001 UseAfterMove` all exist and work.
> Phase 2 is much smaller than originally planned.

## §1 — What already exists in the Rust analyzer

| Component | File:line | Status |
|---|---|---|
| `MoveTracker` (per-scope ownership states) | `src/analyzer/borrow_lite.rs:138-156` | ✅ exists, fully functional |
| `OwnershipState::Owned` / `Moved` | `src/analyzer/borrow_lite.rs` | ✅ exists |
| `mark_moved()` / `check_use()` API | `src/analyzer/borrow_lite.rs:187, 198` | ✅ exists |
| `is_copy_type_strict()` — Array/Struct/Str → Move | `src/analyzer/borrow_lite.rs:514` | ✅ exists |
| `ME001 UseAfterMove` enum variant + diagnostic | `src/analyzer/type_check/mod.rs:902-914` | ✅ exists |
| ME001 emission gated by `strict_ownership` flag | `src/analyzer/type_check/check.rs:2001` | ✅ exists |
| `tc.strict_ownership = true` activator | `src/analyzer/type_check/mod.rs:1927` | ✅ exists |
| `is_copy()` helper picks lenient vs strict | `src/analyzer/type_check/mod.rs:1940-1946` | ✅ exists |

**The plan's "SE017 UseAfterMove" already exists as ME001.** The plan
named it SE017 (Semantic Error category) but the analyzer ships it in
the Memory Error category (ME). This is a naming question only — the
underlying detection works.

## §2 — What's the actual gap?

Three concrete missing pieces for Phase 2:

| Gap | What | Estimated effort |
|---|---|---|
| **G1** Default `strict_ownership = false` | Currently arrays are Copy in default mode (per `is_copy_type` permissive — `Type::Array(_)` returns true at L583). To make Phase 1's heap-still-heap caveat compile-time-checkable, self-host source compilation needs `strict_ownership = ON`. Either: (a) flag default-on for self-host source pipeline, (b) per-file/per-fn opt-in, or (c) global default-on with `--lenient` opt-out for REPL/interpreter. | ~30-60min (depending on activation strategy decision) |
| **G2** `.clone()` builtin recognition | When `arr.clone()` is called on a `[T]`, the analyzer must NOT mark `arr` as moved (clone preserves ownership). Need: analyzer pattern-match `MethodCall { receiver, method: "clone", .. }` → skip `mark_moved`. | ~30-45min |
| **G3** `_fj_arr_clone` codegen preamble | The runtime side: `static _FjArr* _fj_arr_clone(_FjArr* a)` that allocates a fresh `_FjArr` from the arena + memcpys the buffer. Pairs with G2's analyzer recognition. Stage 2 byte-equality must be preserved (text-only preamble change). | ~30-45min |

**Total estimated effort post-discovery: ~2-3h** (vs 14h base / 18h
cap in original plan §5).

## §3 — Why the original plan over-scoped

The plan §1 candidate D was:

> "Linear-types-lite (move-only `_FjArr`; double-use is a compile error;
> explicit `clone()` for sharing) | Type system: `[T]` is affine; analyzer
> rejects use-after-move; codegen emits `free` at last-use"

This wording suggested a from-scratch implementation. The reality is the
Rust analyzer **already has linear-types-lite for non-Copy types** —
introduced in earlier work (the Rust analyzer's borrow-lite module is
called "lite" precisely because it implements affine semantics without
full lifetimes, exactly per Compass §6.2's Hylo-style mutable-value-
semantics aspiration).

The B0 audit (`docs/FJARR_LEAK_PHASE_2_B0_FINDINGS.md`) didn't surface
this because B0 was scoped to *cascade-risk in self-host source*, not
to *what the Rust analyzer can already do*. 18.D.1 prep is the natural
moment for this discovery.

## §4 — Three open design questions for G1 (activation strategy)

Choosing G1's activation strategy is the actual design decision for
Phase 2:

### G1.A — Global default-on, `--lenient` opt-out

`strict_ownership = true` becomes the default. Existing interpreter
REPL / one-shot `fj run file.fj` flips lenient with a flag.

- **Pro:** matches Compass §4.4 "@safe sebagai default" — affine is the
  safe behavior; lenient is the opt-out.
- **Con:** breaks every existing example/tutorial that has `let v = [1,2,3]; foo(v); bar(v)` patterns, even when "foo doesn't actually mutate v." Big test fallout (likely 50+ test failures).

### G1.B — Per-file `@strict` annotation OR per-fn `@strict` annotation

Opt-in via annotation. Self-host stdlib gets `@strict` at the top.
Other code default-lenient.

- **Pro:** zero blast radius on existing tests/examples.
- **Con:** "default lenient = default leak" — Compass §4.4 anti-pattern. Phase 1 already filed this as a tension point in decision file §@kernel-future-compat.

### G1.C — Activate ONLY when self-host source is compiled (auto-detect)

The Rust CLI detects "this is a stdlib/*.fj file being compiled by
the chain" and flips strict mode. User-source compilation stays
lenient until G1.A is ready.

- **Pro:** progresses Phase 2 in self-host without breaking user code yet.
- **Con:** stratifies the language (self-host source is "stricter" than user source) — confusing semantics; users learn lenient first, then have to relearn affine.

### G1.D — Inverse: keep current default, add explicit `--strict` CLI flag

Phase 2 ships the `--strict` flag + .clone() builtin + recommend it
for production code. Defaults unchanged.

- **Pro:** zero blast radius; opt-in for users who want it.
- **Con:** doesn't actually advance toward Compass §4.1's "@kernel must reject heap at compile time" — the safety isn't enforced unless user opts in.

## §5 — Recommended pivot

**D-LITE** (this discovery) replaces the 14h "build affine semantics
from scratch" plan with a 2-3h plan that closes G1 + G2 + G3:

| Step | Estimated | What |
|---|---|---|
| Decision on G1 activation strategy (A/B/C/D) | ~15min | Committed file `docs/decisions/2026-05-08-phase2-strict-mode-activation.md` |
| Implementation + test passage | ~2-3h | G1 flag flip + G2 .clone() recognition + G3 _fj_arr_clone preamble |
| Closure docs + v35.2.0 ship | ~1h | FJARR_LEAK_PHASE_2_FINDINGS.md + CHANGELOG + CLAUDE.md §3 |
| **Total** | **~4h** | (vs 14-18h original plan §5 budget) |

The original "Strategy D / 14h" estimate stays in the decision file as
the reverse-cost (in case G2 or G3 surface unexpected complications).

## §6 — STOP for user authorization

The Phase 2 plan budget materially changed (~14h → ~4h). The
activation-strategy choice for G1 (A/B/C/D) is a fresh design decision
that wasn't in the original Choice F decision file. **No analyzer
code changes proceed until user authorizes:**

1. The pivot itself (D-LITE replacing original 14h Strategy D scope)
2. The G1 activation strategy choice (A/B/C/D — each has different blast radius)

Per CLAUDE.md §6.8 R6 (decision-gate hygiene) + the prior session
pattern (B0 yellow-light → user OK → 18.D.1 prep → THIS discovery →
user OK), this is a natural pause point. The analyzer one-way-door
hasn't fired yet — flipping `strict_ownership` default is reversible
(just flip back) until production-source-compilation actually requires
it.

---

*FJARR_LEAK_PHASE_2_18D1_DISCOVERY — written 2026-05-08. Surfaces that
analyzer's affine-array machinery is 90% built (MoveTracker +
ME001 + is_copy_type_strict), reducing Phase 2 from ~14h to ~2-3h. Three
gaps remain: G1 default strict_ownership activation strategy (4 sub-
options A/B/C/D), G2 .clone() builtin analyzer recognition, G3
_fj_arr_clone codegen preamble. Awaiting user authorization for pivot
+ G1 sub-decision before any code change.*
