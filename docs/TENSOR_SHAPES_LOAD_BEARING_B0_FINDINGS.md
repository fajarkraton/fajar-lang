# tensor_shapes.rs Load-Bearing Audit — B0 Findings

> **Phase:** Strategic Compass §5.1 implementation — Dependent types
> "Bekukan" verdict load-bearing audit.
> **Audit date:** 2026-05-12 (EOS-33, post-GPU-Action-C).
> **Plan Hygiene §6.8 R1:** Audit only. Strategic decision is Fajar's.

## §1. Scope

The Compass §5 freeze-candidates B0 (`docs/COMPASS_5_FREEZE_CANDIDATES_B0_FINDINGS.md`)
recommended for dependent types:

> AUDIT first, then likely FREEZE in-place. `tensor_shapes.rs` may be
> load-bearing for @device. Don't cut blind.

This B0 verifies whether `src/dependent/tensor_shapes.rs` is load-bearing
for the `@device` flow / tensor shape checking, or whether it's dead
research code.

## §2. tensor_shapes.rs API surface (HEAD `439f8cfc`)

File: 553 LOC, 24 `#[test]`-fn lib tests.

| Pub item | Line | Role |
|---|---|---|
| `struct DepTensor { element_ty, dims }` | 20 | Dependent tensor type carrying compile-time dims |
| `fn infer_from_constructor` | 75 | Infer tensor type from constructor name + args |
| `fn check_matmul` | 95 | Compile-time matmul shape check |
| `fn transpose_type` | 124 | Compile-time transpose type derivation |
| `fn check_reshape` | 142 | Compile-time reshape compatibility |
| `fn flatten_type` | 175 | Compile-time flatten type derivation |
| `enum BroadcastResult` | 189 | Broadcast outcome enum |
| `fn check_broadcast` | 206 | Compile-time broadcast compatibility check |
| `const MAX_TENSOR_RANK: usize = 4` | 266 | Rank cap |
| `fn higher_rank_tensor` | 269 | Construct higher-rank dep tensor |
| `fn format_shape_error` | 290 | Error formatter |

## §3. Consumer trace (HEAD `439f8cfc`)

### 3.1 Direct `use crate::dependent::tensor_shapes` consumers

Grep `use crate::dependent::tensor_shapes\|dependent::tensor_shapes::`
across `src/ tests/ examples/`:

```
src/interpreter/eval/mod.rs
```

**Only one consumer.** And every match in `src/interpreter/eval/mod.rs`
is inside a `#[test]` function. Verified by reading L6120-6135 context
(sprint DT3 unit tests `dt3_1_dep_tensor_creation` etc., all
`#[test]`-annotated, none called from production code paths).

### 3.2 References by symbol name (DepTensor / check_matmul / etc.)

Grep symbol names across `src/`:

```
src/interpreter/eval/mod.rs    # only inside #[test] — see above
src/verify/device_proofs.rs    # SEPARATE definitions of `check_tensor_shapes`
                               # and `check_broadcast_compatibility` —
                               # NOT consuming tensor_shapes.rs (verified
                               # via grep `use .*tensor_shapes`: 0 matches)
src/runtime/ml/ops.rs          # FALSE POSITIVE — grep found no actual
                               # tensor_shapes references; "check"+"matmul"
                               # are unrelated substrings
```

**`src/verify/device_proofs.rs` confirms it does NOT import tensor_shapes.rs.**
Its `check_tensor_shapes` and `check_broadcast_compatibility` functions
operate on its own `DeviceFunction` / `DeviceViolation` types, not on
`DepTensor`. They're a parallel, independent implementation.

### 3.3 Does production code ever transitively touch tensor_shapes?

Production callers of `src/verify/*` (per `grep -rln "use crate::verify"`):
- `src/main.rs` → uses `verify::spec`, `verify::symbolic` (NOT device_proofs)
- `src/analyzer/type_check/check.rs` → uses `verify::tensor_verify` (NOT device_proofs)
- `src/interpreter/eval/mod.rs` → uses `verify::smt`, `verify::symbolic` (NOT device_proofs)

**Zero production callers of `verify::device_proofs`.** And `verify::device_proofs`
doesn't consume `tensor_shapes.rs` anyway.

Production callers of `dependent::*` (per `grep -rln "use crate::dependent"`):
- `src/const_generics.rs` → `dependent::nat::{ConstType, MonoKey, NatConstraint, NatValue}` only.
- `src/interpreter/eval/mod.rs` → `dependent::*` inside `#[test]` fns only.

**`dependent::nat` is load-bearing** (consumed by `const_generics.rs`).
**`dependent::tensor_shapes` is NOT load-bearing in production.**

## §4. Verdict: tensor_shapes.rs is DEAD in production

`tensor_shapes.rs` (553 LOC, 24 lib tests + sprint DT3 tests in eval/mod.rs)
has **zero production consumers**. It's research code that proves out the
dependent-types-for-tensor-shapes design but isn't wired into:
- The Rust analyzer's `@device` enforcement
- The interpreter's runtime tensor ops
- The codegen path for any backend
- Any user-facing CLI subcommand

The `@device` annotation handling in production lives in
`src/analyzer/` directly (per CLAUDE.md §5.3 + the v35.6.0 D-α work).
It does NOT route through `tensor_shapes.rs`.

## §5. Broader finding: arrays.rs + patterns.rs same pattern

While auditing, I checked the sibling files:

| File | LOC | tests | production consumers | Status |
|---|---|---|---|---|
| `dependent/nat.rs` | 1,707 | 73 | `src/const_generics.rs` (1 fn) | **Load-bearing** |
| `dependent/arrays.rs` | 502 | 23 | `src/interpreter/eval/mod.rs` inside `#[test]` only | **Dead in production** |
| `dependent/patterns.rs` | 867 | 40 | `src/interpreter/eval/mod.rs` inside `#[test]` only | **Dead in production** |
| `dependent/tensor_shapes.rs` | 553 | 24 | `src/interpreter/eval/mod.rs` inside `#[test]` only | **Dead in production** |
| `dependent/mod.rs` | 7 | 0 | dispatch only | n/a |

**Aggregate dead surface: arrays + patterns + tensor_shapes = 1,922 LOC + 87 lib tests +
~30-40 sprint DT2/DT3 tests in eval/mod.rs (test-only consumers).**

The Compass §5.1 dependent types verdict ("Bekukan; mungkin tidak kembali")
is empirically supported by this audit. Only `nat.rs` survives the
load-bearing test.

Note: also `verify::device_proofs.rs` (per §3.3) appears to be similarly
dead — zero production callers. That's a separate audit candidate.

## §6. Three action paths for tensor_shapes.rs (Fajar to pick)

### Action A — Freeze in place

- Add module-level deprecation header to tensor_shapes.rs citing this
  B0 + Compass §5.1.
- No code/test changes. 0 LOC reduction.
- **Effort:** ~5min.
- **Risk:** None.

### Action B — Delete tensor_shapes.rs + remove sprint DT3 tests

- Delete `src/dependent/tensor_shapes.rs` (553 LOC, 24 tests).
- Remove the ~16-20 `dt3_*` sprint tests from `src/interpreter/eval/mod.rs`
  that reference `DepTensor`/`check_matmul`/etc. (estimate from visible
  L6120-6210 range).
- Update `src/dependent/mod.rs` to drop `pub mod tensor_shapes;`.
- **Effort:** ~20-30min.
- **Risk:** LOW. No production consumers; deletion safe.
- **LOC delta:** -553 + -~150 (eval/mod.rs dt3_* tests) ≈ **-700 LOC**.
- **Test delta:** -24 (tensor_shapes.rs) + -~16-20 (eval/mod.rs dt3_*) ≈ **-40-44 tests**.

### Action C — Broader: delete tensor_shapes + arrays + patterns (whole dep-types-dead-paths)

- Delete arrays.rs (502 LOC, 23 tests) + patterns.rs (867, 40) +
  tensor_shapes.rs (553, 24) = 1,922 LOC + 87 lib tests.
- Remove all dt1_*/dt2_*/dt3_* sprint tests from eval/mod.rs
  (~30-40 tests).
- Keep `dependent/nat.rs` (load-bearing).
- Update `dependent/mod.rs`.
- **Effort:** ~1-1.5h.
- **Risk:** LOW-MEDIUM. Larger churn, more eval/mod.rs edits, but no
  production consumers across the three deleted modules.
- **LOC delta:** ~-2,300.
- **Test delta:** ~-120-130 tests.

## §7. Recommendation

**Action B for v35.7.2** — narrow scope, single-file deletion + scoped
test removal. Aligns with Compass §5.1, honest deletion of confirmed-dead
code, manageable scope.

Action C is the maximalist version. If Fajar wants to commit fully to
freezing dependent types as the Compass recommended ("mungkin tidak kembali"),
Action C is the right move. But Action B is the safer "first step" — once
shipped, Action C can follow as a separate phase if desired.

Action A (freeze-only) preserves the code without a re-entry condition
that's actually unlikely to fire. Less satisfying than B/C.

## §8. Re-entry conditions (post-deletion)

If `tensor_shapes.rs` (or arrays.rs/patterns.rs) is ever needed:
1. Git history retains the deletion commit — `git log --diff-filter=D --
   src/dependent/tensor_shapes.rs` recovers the file.
2. Re-entry conditions same as Compass §5.1 dep-types: fj-source
   analyzer.fj gains compile-time tensor-shape checking, OR the
   `@device` flow needs dependent-types machinery as a future safety
   guarantee.
3. Until then, nat.rs covers the const-generic dimension and is
   sufficient for current const-generic needs (per `const_generics.rs`).

## §9. Stage 2 byte-equality risk

NONE for any of the three actions. tensor_shapes.rs / arrays.rs /
patterns.rs are Rust-only, not in stdlib `.fj`. Phase17 unaffected.

## §10. Self-check (§6.8 audit checklist)

```
[x] Pre-flight audit (B0) exists for the Phase            (Rule 1)
[x] Every task has runnable verification command           (Rule 2 — §11)
[ ] Prevention mechanism added (hook/CI/rule)              (Rule 3 — Action B/C ship would add updated dependent/mod.rs)
[x] Agent-produced numbers cross-checked with Bash         (Rule 4 — all consumer grep verified live)
[ ] Effort variance tagged in commit message               (Rule 5 — at commit time)
[ ] Decisions are committed files                          (Rule 6 — pending Fajar's choice)
[x] Public-artifact drift swept                            (Rule 7 — done earlier in session)
[x] Multi-repo state checked                               (Rule 8 — done earlier in session)
```

## §11. Verification commands

```bash
cd "/home/primecore/Documents/Fajar Lang"

# Re-verify consumer count
grep -rln "use crate::dependent::tensor_shapes\|dependent::tensor_shapes::" src/ tests/ examples/
# expect: src/interpreter/eval/mod.rs (only)

# Re-verify production callers of verify::device_proofs
grep -rln "use crate::verify::device_proofs\|verify::device_proofs::" src/ tests/ examples/ \
    | grep -v "src/verify/device_proofs.rs"
# expect: empty

# Re-verify dependent::nat is genuinely production-load-bearing
grep -rln "use crate::dependent::nat" src/ | grep -v "src/dependent/"
# expect: src/const_generics.rs

# Post-Action-B verify (when shipped):
cargo test --lib  # expect 7,619 - ~40-44 = ~7,575-7,579 passed
cargo clippy --lib -- -D warnings  # 0 warnings
cargo fmt -- --check  # clean
cargo test --release --test selfhost_phase17_self_compile -- --test-threads=1
# expect: 4 passed (Stage 2 byte-equality preserved — Rust-only changes)
```

## §12. Source artifacts

- This file: `docs/TENSOR_SHAPES_LOAD_BEARING_B0_FINDINGS.md`
- Compass §5 audit: `docs/COMPASS_5_FREEZE_CANDIDATES_B0_FINDINGS.md` §2.2 (Dep types row)
- Source compass: `docs/1/STRATEGIC_COMPASS.md` §5.1 (Dep types row)
- Companion GPU codegen decision (same session pattern): `docs/decisions/2026-05-12-gpu-codegen-simplification.md`

---

*B0 written 2026-05-12 EOS-33. ~25min actual. Verdict: tensor_shapes.rs
dead in production. Recommendation: Action B (delete + scoped test
cleanup). Bonus finding: arrays.rs + patterns.rs same dead pattern;
Action C extends to those. Decision pending Fajar.*
