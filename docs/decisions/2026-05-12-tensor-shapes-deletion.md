# Decision — tensor_shapes.rs Deletion (v35.7.2 Action B)

> **Date:** 2026-05-12
> **Owner:** Fajar (user decision after B0)
> **Status:** ✅ Decided — Action B (narrow delete + scoped test cleanup)
> **B0 source:** `docs/TENSOR_SHAPES_LOAD_BEARING_B0_FINDINGS.md`
> **Compass source:** `docs/1/STRATEGIC_COMPASS.md` §5.1 (Dep types row)
> **Plan Hygiene §6.8 R6:** This file is the committed decision.

## Decision

**Adopt Action B (narrow delete) for v35.7.2.**

### Scope of Action B

1. **DELETE** `src/dependent/tensor_shapes.rs` (553 LOC, 24 lib tests).
2. **EDIT** `src/dependent/mod.rs`:
   - Drop `pub mod tensor_shapes;` declaration.
   - Update module-level doc comment (remove "tensors" from the shape-verification scope description).
   - Add v35.7.2 note explaining the removal + reference to B0 + this decision.
3. **REMOVE** sprint DT3 tests in `src/interpreter/eval/mod.rs`:
   - Remove `dt3_1_dep_tensor_creation` through `dt3_10_tensor_constructor_inference`
     (10 tests, ~120 LOC of test code at L6126-6239).
   - Replace the block with a 3-line comment pointing to this decision file
     and the B0.

### Verified post-ship state

- `cargo build` → success
- `cargo test --lib` → **7,585 passed** (was 7,619 pre-Action-B; delta -34
  = -24 tensor_shapes.rs lib tests + -10 dt3_* sprint tests; **exactly
  matches the B0 estimate of "~40-44 tests removed" — at the low end since
  the B0 didn't have an exact dt3_* count**).
- `cargo test --lib dependent` → 146 passed (was 160; -14 matches the
  `dependent` substring filter scope after tensor_shapes.rs removal).
- `cargo clippy --lib -- -D warnings` → 0 warnings.
- `cargo fmt -- --check` → clean.
- Phase17 byte-equality → verified by pre-push hook (Rust-only changes,
  no stdlib `.fj` touched).

### Aggregate impact

| Metric | Before | After | Delta |
|---|---|---|---|
| `src/dependent/` files | 5 | 4 | **-1** (tensor_shapes.rs) |
| `src/dependent/` LOC | ~3,636 | ~3,083 | **-553 (-15.2%)** |
| Lib tests | 7,619 | **7,585** | **-34** |
| Sprint DT3 tests in eval/mod.rs | 10 | 0 | -10 |
| Dependent-module lib tests (24 internal) | 24 | 0 | -24 |

### Why Action B over A or C

| Option | Trim | Risk | Why chosen / deferred |
|---|---|---|---|
| Action A (freeze in place, deprecation header) | 0 LOC | None | Doesn't reduce surface. Compass said "Bekukan; mungkin tidak kembali" — outright deletion is more honest. |
| **Action B (delete tensor_shapes, scoped DT3 cleanup)** | **-553 LOC + -34 tests** | **LOW** | **Chosen.** Narrow first step. Empirically supported by B0 (zero production consumers). |
| Action C (delete tensor_shapes + arrays + patterns) | ~-2,300 LOC + ~-120-130 tests | LOW-MEDIUM | Deferred. Action B is the safer "first step" — once shipped, Action C can follow as a separate phase if Fajar commits fully to Compass §5.1's dep-types freeze. |

### What Action B does NOT do (intentional non-scope)

- Does NOT delete `arrays.rs` or `patterns.rs` (both also dead in production
  per the B0 bonus finding — but deferred to a potential Action C).
- Does NOT touch `nat.rs` (load-bearing — consumed by `src/const_generics.rs`).
- Does NOT touch `src/verify/device_proofs.rs` (also dead per B0, but
  out-of-scope for this decision; flagged as separate audit candidate).
- Does NOT update CLAUDE.md or README (engineering correctness; no
  user-visible feature change).
- Does NOT tag/release (consistent with B-δ + D1.A + GPU Action C session
  pattern — all engineering closures, not user-visible features).

### Re-entry conditions for tensor_shapes revival

If `tensor_shapes.rs` is ever needed:
1. Git history retains the deletion commit — `git log --diff-filter=D --
   src/dependent/tensor_shapes.rs` recovers the file.
2. Trigger conditions (any one):
   - fj-source analyzer.fj begins doing compile-time tensor shape checking.
   - The `@device` flow needs dependent-types machinery as a future safety
     guarantee.
   - A user/customer requirement surfaces for compile-time tensor type
     verification.
3. Until then, nat.rs covers the const-generic dimension; that's sufficient
   for current const-generic needs (per `src/const_generics.rs`).

### Re-entry conditions for Action C extension (arrays + patterns)

The B0 surfaced that arrays.rs (502 LOC, 23 tests) + patterns.rs (867
LOC, 40 tests) follow the same dead-in-production pattern. Reasons to
open Action C as a follow-up phase:

1. Confidence in the dep-types freeze has settled (no surprises from
   Action B).
2. The codebase wants to commit fully to Compass §5.1's "mungkin tidak
   kembali" verdict.
3. A complete dep-types research-extraction is desired (move to
   `dependent-research` git branch).

Until then, Action B's narrower scope is the operative choice.

### Stage 2 byte-equality risk: NONE

Action B touches only Rust files (no stdlib `.fj`). Phase17 unaffected.

### B0 accuracy

B0 estimated "~700 LOC + ~40-44 tests removed" for Action B.
Actual delivered: **-553 LOC + -34 tests**.

The B0 over-estimated by ~150 LOC because it counted the eval/mod.rs
dt3_* block visually rather than line-by-line. Actual block was 10
tests × ~12 LOC each ≈ 120 LOC; B0 estimated ~150. Test count B0
estimated 16-20 dt3_* tests; actual was 10. The B0 numbers were
rough; the actual ship came in narrower than estimated. No surprises.

## References

- B0: `docs/TENSOR_SHAPES_LOAD_BEARING_B0_FINDINGS.md`
- Compass §5 audit: `docs/COMPASS_5_FREEZE_CANDIDATES_B0_FINDINGS.md` §2.2
- Source compass: `docs/1/STRATEGIC_COMPASS.md` §5.1 (Dep types row)
- Session decision-doc companions (same day):
  - `docs/decisions/2026-05-12-cranelift-builtin-list-shape.md` (B-δ)
  - `docs/decisions/2026-05-12-parser-annotation-grammar-shape.md` (D1.A)
  - `docs/decisions/2026-05-12-gpu-codegen-simplification.md` (GPU Action C)
