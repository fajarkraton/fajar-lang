---
decision: @safe default = ergonomic bridge (can call @kernel + @device)
date: 2026-05-10
status: ACCEPTED 2026-05-10 (D-α; D-β rejected)
prereq: docs/KERNEL_MODE_B0_FINDINGS.md, docs/KERNEL_MODE_PHASE_A_B0_FINDINGS.md
plan: docs/KERNEL_MODE_PHASE_A_B0_FINDINGS.md §5
gate: D1 (Phase A Bucket 3 path)
---

# Phase A D1 — @safe semantic for cross-context calls

## Choice

**Choice: D-α** — `@safe` is an **ergonomic bridge layer**: it MAY
call `@kernel` and `@device` functions directly. SE021
(KernelCallInSafe) and SE022 (DeviceCallInSafe) emission is removed.

D-β (tighten doc to match current SE021/SE022 enforcement) is
**rejected**.

> **Status: ACCEPTED 2026-05-10.** Mandatory pre-flight for Phase A.1 +
> A.3 in `docs/KERNEL_MODE_PHASE_A_B0_FINDINGS.md` §5. Phase A may
> not commit downstream without this decision file checked in.

## Rationale (≥3 sentences)

CLAUDE.md §5.3 enforcement table is the authoritative language-design
specification, and it has stated since v17 that `@safe` permits both
`@kernel` and `@device` calls (rows "Call `@device` function" and
"Call `@kernel` function" both show **OK** for the @safe column).
STRATEGIC_COMPASS.md §4.4 reinforces this: "Naik ke `@kernel`/`@device`
adalah opt-in" implies @safe must be able to invoke them — otherwise
"opt-in to @kernel" is incoherent (you'd be unable to call kernel
functions from anywhere except @kernel itself, defeating the layered
model). The CLAUDE.md §5.4 worked example `bridge() -> Action` is
literally a bridge fn calling `read_sensor()` (`@kernel`) and
`infer()` (`@device`); making @safe block these calls would make the
flagship demo of the language uncompilable.

The current SE021/SE022 enforcement contradicts all three of these
authoritative sources. The fact that the lib test
`safe_fn_can_call_both_kernel_and_device` is authored expecting OK
(and only passes today because `ScopeKind::Function` ≠ `ScopeKind::Safe`
bypasses the check) confirms the project's design intent matches D-α.
The SE021/SE022 emission paths are vestigial — they predate the
microkernel-isolation refinement of Compass §4.4 and were never
reconciled with the §5.3 table.

D-β would require editing CLAUDE.md §5.3, STRATEGIC_COMPASS.md §4.4,
the §5.4 example, and the test name `safe_fn_can_call_both_kernel_and_device`
all in lockstep. That's higher cost AND backs out the strategic
compass position; D-α aligns code with the canonical doc at lower cost.

## Pre-rejected: D-β (tighten Compass + analyzer match)

D-β is rejected because:

1. **Compass authoritativeness.** CLAUDE.md §5.3 + STRATEGIC_COMPASS.md
   §4.4 + the §5.4 worked bridge example are the canonical
   specification of @safe semantics. Aligning code to spec is correct
   direction; aligning spec to code is the doc-debt failure mode
   Plan Hygiene Rule 6 (V26 surfaced this exact pattern).
2. **Higher edit cost.** D-β touches 3 docs + ≥1 test + 0 LOC of
   analyzer; D-α touches 0 docs + ~3 tests + 8 LOC of analyzer. D-β
   is more files-changed.
3. **Worse ergonomics.** Without @safe→@kernel/@device, there's no
   place for the Compass-flagged "bridge" pattern. Forcing every
   bridge to be `@unsafe` would push users into the strongest possible
   escape valve for the most common cross-domain call site.

## Pre-rejected: keep current dual state

A "do nothing, document it" path exists in principle (leave SE021/SE022
firing for explicit `@safe`, leave default permissive) but is rejected
because:

1. It violates Plan Hygiene Rule 6 (decisions must be mechanical, not
   prose). Two contradictory sources of truth (CLAUDE.md vs analyzer)
   cannot coexist as a stable state.
2. v35.5.0 just closed §4.4 at the type-system level (affine). Closing
   it at the context level (this Phase A) requires the contradiction
   to resolve in one direction first.

## Mechanical implementation

Phase A.2 will, in a single commit:

1. Delete `src/analyzer/type_check/check.rs:1989-1996` (the SE021 +
   SE022 emission block inside the `if in_safe { }` branch).
2. Either delete or downgrade-comment the `KernelCallInSafe` +
   `DeviceCallInSafe` SemanticError variants and their codegen/LSP
   mappings, depending on whether they're referenced elsewhere
   (audit count: ≤1h).
3. Re-run `cargo test --lib` — `safe_fn_can_call_both_kernel_and_device`
   should still pass (it already does today via the `ScopeKind::Function`
   bypass; D-α removes the bypass need).
4. Audit any tests that *assert* SE021/SE022 fires — invert or delete
   per case.

Estimated effort: ~30min for D-α implementation alone. Phase A.2
commit message will reference this decision file path verbatim per
Plan Hygiene Rule 5.

## What this does NOT change

- KE001/KE002/KE003/DE001/DE002 enforcement: unchanged. `@kernel` still
  blocks heap+tensor+device-call; `@device` still blocks raw-pointer+
  kernel-call. The microkernel isolation is preserved at the *strict*
  context boundaries.
- v35.5.0's affine semantics (D-FULL): completely independent.
- The 4 enforcement layers (analyzer, Cranelift codegen, self-host
  analyzer_fj, LSP): only the `@safe → kernel/device` rules drop;
  every other context rule keeps its layered enforcement.

## Forward chain

This decision unblocks:
- Phase A.1 (`is_inside_function()` extension) — independent of D1, can
  run in parallel.
- Phase A.2 (D-α implementation) — gated by this file.
- Phase A.3 (stdlib + examples annotation cascade) — gated by A.1+A.2.
- Phase A.4 (final flip @ check.rs:160) — gated by A.3 fully green.
- Phase A.5 (closure ship — v35.6.0 minor bump).

## Verification

```bash
# Pre-commit hook should refuse Phase A.2 commits if this file is missing
test -f docs/decisions/2026-05-10-default-safe-bridge.md
grep -q "Choice: D-α" docs/decisions/2026-05-10-default-safe-bridge.md
```

(The hook itself is a Phase A.2 deliverable; this decision file is its
input.)
