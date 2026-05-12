# Decision: Delete verify::device_proofs (Path A — narrow)

**Date:** 2026-05-12 (post-EOS-34, Path A first-step closure)
**Decider:** Fajar Putranto — "Mulai dari A"
**Status:** APPROVED — SHIP IMMEDIATELY (narrow scope)
**Audit:** `docs/DEVICE_PROOFS_LOAD_BEARING_B0_FINDINGS.md` (commit `bf65136b`)
**Path:** A — narrow deletion of `src/verify/device_proofs.rs` only.
Paths B/C (broader verify/ cleanup) deferred pending Fajar's later
strategic call.

## Context

`src/verify/device_proofs.rs` — Sprint V4 `@device` safety-proofs module,
1,121 LOC + 24 self-tests. Module header self-admits "All simulated (no
real Z3)". Audit verified:

- ZERO direct imports anywhere in `src/`, `tests/`, `examples/`,
  `benches/`, `stdlib/`.
- ZERO symbol-level references outside the file (no `DeviceFunction`,
  `DeviceViolation`, `check_tensor_shapes`, `check_broadcast_compatibility`,
  `DeviceSafetyChecker`, `DeviceCheckConfig`).
- ZERO CLI exposure, ZERO feature-flag gating.
- ZERO non-self test consumers (24 internal `v4_*` self-tests only).

## The Compass §5.1 verdict

`docs/1/STRATEGIC_COMPASS.md` §5.1 explicitly:

> | SMT verification (DO-178C) | Diklaim ada | **Bekukan**. Butuh tim untuk certification serius. |

And in the Compass Decision Framework:

> | Tambah formal proof / SMT verification? | ⏸️ Bekukan kecuali untuk niche safety-critical certification |

device_proofs.rs (Sprint V4) is part of the SMT-verification family
explicitly frozen by §5.1. The simulated nature ("All simulated (no real
Z3)") confirms it never crossed the research→production threshold and
the freeze rationale ("Butuh tim untuk certification serius") fits.

## Decision

**Delete `src/verify/device_proofs.rs`** + update `verify/mod.rs` to drop
the `pub mod` declaration and the doc-comment reference.

Path A scope (narrow) chosen over B/C because:
1. Path A is mechanically safe — zero consumers, zero risk.
2. Closes the specific audit candidate flagged by predecessor
   tensor_shapes B0 §3.3.
3. Validates the deletion pattern works on the verify/ family before
   committing to broader B/C scope.
4. Preserves optionality — Fajar can return to Path B (6 more
   zero-consumer modules) or Path C (full SMT freeze with test cleanups)
   in a later session without compounding risk.

## Scope (concrete)

| Change | Detail |
|---|---|
| Delete file | `src/verify/device_proofs.rs` (1,121 LOC, 24 self-tests) |
| Edit | `src/verify/mod.rs` — remove `pub mod device_proofs;` (line 15) + the doc-comment bullet for `device_proofs` (line 10) + update module-level header to note the deletion + Compass §5.1 rationale |

**Estimated delta:** -1,121 LOC (file) + minor mod.rs edits, -24 lib tests, lib count 7,502 → 7,478.

## Stage 2 byte-equality risk

NONE. device_proofs.rs is Rust-only, never compiled by fjc. `stdlib/*.fj`
has zero references to any of its symbols (verified live during B0).
Phase17 self-host gate (4/4) unaffected.

## Engineering gates (run pre-commit)

```bash
cargo build --lib                                  # expect clean
cargo test --lib 2>&1 | tail -3                    # expect 7,478 passed
cargo clippy --lib -- -D warnings                  # expect 0 warnings
cargo fmt -- --check                               # expect clean
cargo test --test context_safety_tests             # expect 149 passed
cargo test --release --test selfhost_phase17_self_compile -- --test-threads=1
                                                   # expect 4 passed (byte-equality)
```

Pre-push hook on `git push origin main` runs the full self-host gate.

## Deferred paths (carry forward for future sessions)

- **Path B (recommended in B0 but deferred):** Delete 6 remaining
  zero-consumer modules — kernel_proofs (987 LOC, V3) + proof_cache
  (1,239 LOC, V5) + properties (1,194 LOC, V2) + inference (1,401 LOC) +
  theories (1,170 LOC) + benchmarks (1,125 LOC). Total ~-7,116 LOC. No
  risk (zero consumers each). Re-entry condition: a future session
  decides to extend Path A pattern to the rest of the dead surface.
- **Path C (further extension):** Plus test-only modules (certification,
  pipeline, smt) and their test consumers in eval/mod.rs +
  tests/feature_flag_tests.rs + tests/nova_v2_tests.rs. ~+~3,766 LOC
  beyond Path B, more careful boundary work. Re-entry condition: Fajar
  decides the test surface is also worth shedding.
- **Path D (Compass-literal freeze):** Add header deprecation comments
  to remaining modules. Counter-recommended in B0 §6 ("worst of both")
  but available if Fajar wants the lighter-touch option.

## Re-entry conditions (post-deletion)

Reverse this decision if and only if:
1. Compass §5.1 SMT-verification verdict is explicitly overturned by
   Fajar in a follow-up decision file, AND
2. A certification team commitment exists (per Compass rationale —
   "Butuh tim untuk certification serius"), AND
3. The wire-up plan goes through Plan Hygiene §6.8 (B0 → plan → phased
   ship) before any code restoration.

Until then: status frozen. File recoverable from git via:
```
git log --diff-filter=D --follow -- src/verify/device_proofs.rs
```

## Sign-off

- B0 audit: `docs/DEVICE_PROOFS_LOAD_BEARING_B0_FINDINGS.md` (committed `bf65136b`).
- Predecessor audit candidate flag: `docs/TENSOR_SHAPES_LOAD_BEARING_B0_FINDINGS.md` §3.3.
- Companion freeze decisions same session: `2026-05-12-tensor-shapes-deletion.md`, `2026-05-12-arrays-patterns-deletion.md`.
- Compass source: `docs/1/STRATEGIC_COMPASS.md` §5.1 + Decision Framework.

*Path A is the first-step minimal closure of the verify/ dead-surface
audit. Paths B/C remain open follow-ups.*
