# Decision: Delete dependent::arrays + dependent::patterns

**Date:** 2026-05-12 (post-EOS-33, Action C extension)
**Decider:** Fajar Putranto (delegated authority: "Fajar Lang harus 100% produksi dan terbaik sesuai dengan tujuannya")
**Status:** APPROVED — SHIP IMMEDIATELY
**Predecessor decisions:** `2026-05-12-tensor-shapes-deletion.md` (same session, Action B)
**Audit:** `docs/ARRAYS_PATTERNS_LOAD_BEARING_B0_FINDINGS.md` (B0 verdict: both modules DEAD in production)

## Context

Sprint DT2 (dependent arrays, ~v0.3 era) and Sprint DT4 (dependent
patterns) produced research-quality Rust modules that prove dependent-types
design concepts in isolation:

- `src/dependent/arrays.rs` — 502 LOC, 23 lib tests. `DepArray` struct
  carrying compile-time length, `check_bounds`, `concat_type`, `split_at_types`,
  `windows_count`, `validate_window`, etc.
- `src/dependent/patterns.rs` — 867 LOC, 40 lib tests. `NatPattern` enum,
  exhaustiveness check, `ProofWitness`, `InductiveProof`, `DepFnSignature`,
  etc.

Both modules **never crossed the research→production threshold.** The
audit confirms:
- ZERO production consumers across `src/` (analyzer, codegen, interpreter,
  runtime).
- ONLY consumer is `src/interpreter/eval/mod.rs` inside `dt2_*` + `dt4_*`
  `#[test]` fns (20 tests total).
- ZERO references in `stdlib/*.fj` (Stage 2 byte-equality safe).

## The Compass §5.1 verdict

Strategic Compass `docs/1/STRATEGIC_COMPASS.md` §5.1 explicitly puts
**dependent types in the FREEZE column**: *"Bekukan; mungkin tidak kembali."*

Rationale per Compass: dependent types add compile-time/type-system
complexity that does not advance Fajar Lang's stated mission ("embedded
ML + OS integration"). Compile-time array-bounds proving and refinement
typing are nice-to-have for verification research, not load-bearing for
embedded ML deployment or OS-kernel safety. `@kernel` / `@device` /
context isolation (Compass §4.4, fully closed v35.6.0) cover the safety
guarantees the language commits to.

## Decision

**Delete both modules** + their test consumers. Honor the Compass §5.1
freeze by removing the dead surface entirely rather than maintaining it
indefinitely.

Reasons:
1. **"100% produksi" mandate.** Production-quality codebase must not
   carry research prototypes that the strategic compass has explicitly
   frozen. Half-promoted code is anti-CLAUDE.md ("no half-finished
   implementations").
2. **Maintenance burden without value.** Keeping the modules requires
   ongoing clippy/fmt/test discipline (63 lib tests + 20 eval/mod.rs
   tests) for code with zero callers.
3. **Pattern continuity.** Same session already deleted `tensor_shapes.rs`
   (commit `94c61998`, Action B) under the same Compass §5.1 verdict.
   Action C extension closes the dead-surface cleanup uniformly.
4. **Re-entry is well-defined.** Git history retains the deletion. If
   the Compass §5.1 verdict is ever reversed (e.g., fj-source
   `analyzer.fj` gains compile-time array-shape / pattern-exhaustiveness
   checking), `git log --diff-filter=D -- src/dependent/arrays.rs`
   recovers the file. Until then, `nat.rs` covers the const-generic
   dimension and is sufficient for current needs (per `src/const_generics.rs`).

## Scope (concrete)

| Change | Detail |
|---|---|
| Delete file | `src/dependent/arrays.rs` (502 LOC, 23 tests) |
| Delete file | `src/dependent/patterns.rs` (867 LOC, 40 tests) |
| Edit | `src/dependent/mod.rs` — remove `pub mod arrays;` + `pub mod patterns;`; update header to extend the v35.7.2 dep-types-freeze note |
| Edit | `src/interpreter/eval/mod.rs` — remove `dt2_1..dt2_10` (L~5976-6125) + `dt4_1..dt4_10` (L~6131-6265) test blocks |
| Keep | `src/dependent/nat.rs` (load-bearing — consumed by `src/const_generics.rs`) |
| Keep | `src/dependent/mod.rs` (dispatch — keeps `pub mod nat;` only) |
| Keep | `dt1_1..dt1_10` tests in `eval/mod.rs` (test `nat.rs`, load-bearing) |

**Estimated delta:** -1,654 LOC, -83 lib tests, lib count 7,585 → ~7,502.

## Stage 2 byte-equality risk

NONE. Both modules are Rust-only, never compiled by fjc. `stdlib/*.fj`
contains zero references to any pub item (re-verified live during B0).
Phase17 self-host gate (4/4) unaffected.

## Engineering gates

Post-deletion verification commands (commit message will cite):

```bash
cargo test --lib 2>&1 | tail -3                    # expect ~7,502 passed
cargo clippy --lib -- -D warnings                  # expect 0 warnings
cargo fmt -- --check                               # expect clean
cargo test --test context_safety_tests             # expect 149 passed
cargo test --release --test selfhost_phase17_self_compile -- --test-threads=1
                                                   # expect 4 passed (byte-equality)
```

Pre-push hook on `git push origin main` runs the full self-host gate.

## Re-entry conditions

Reverse this decision if and only if:
1. Compass §5.1 dependent-types verdict is explicitly overturned by Fajar
   in a follow-up decision file, AND
2. A concrete production consumer is identified (analyzer wire-up or
   `@device` machinery requirement), AND
3. The wire-up plan goes through Plan Hygiene §6.8 (B0 → plan → phased
   ship) before any code restoration.

Until then: status frozen. Files recoverable from git via:
```
git log --diff-filter=D --follow -- src/dependent/arrays.rs
git log --diff-filter=D --follow -- src/dependent/patterns.rs
```

## Sign-off

- B0 audit: `docs/ARRAYS_PATTERNS_LOAD_BEARING_B0_FINDINGS.md` (committed `4e7038e2`).
- Predecessor (tensor_shapes B0 + deletion): `b71aa0a3` + `94c61998`.
- Compass source: `docs/1/STRATEGIC_COMPASS.md` §5.1.
- Same-session companion freezes: GPU codegen Action C (`439f8cfc`, deleted metal+hlsl, froze spirv).

*Decision committed in same session as ship. Action C extension closes
the dep-types freeze with the same precision as Action B.*
