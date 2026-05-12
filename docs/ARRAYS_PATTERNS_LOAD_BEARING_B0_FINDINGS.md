# arrays.rs + patterns.rs Load-Bearing Audit — B0 Findings

> **Phase:** Action C extension to v35.7.2 tensor_shapes deletion. Same
> Compass §5.1 "Bekukan dep types" verdict, extended to the two remaining
> dead modules in `src/dependent/`.
> **Audit date:** 2026-05-12 (post-EOS-33, on `lanjutkan` first-step).
> **Plan Hygiene §6.8 R1:** Audit only. Action decision is Fajar's.
> **Predecessor B0:** `docs/TENSOR_SHAPES_LOAD_BEARING_B0_FINDINGS.md` §5
> (already flagged arrays + patterns as same dead pattern; this B0 makes
> the deletion-prep explicit and re-verifies post-EOS-33 state).

## §1. Scope

Confirm post-EOS-33 that `src/dependent/arrays.rs` and `src/dependent/patterns.rs`
remain zero-production-consumer (i.e., state has not drifted since the
2026-05-12 tensor_shapes B0). Identify exact test-site deletion ranges in
`src/interpreter/eval/mod.rs`. No code changes — audit-only deliverable.

## §2. API surfaces (HEAD `94c61998`)

### 2.1 arrays.rs — 502 LOC, 23 lib `#[test]` fns

| Pub item | Line | Role |
|---|---|---|
| `struct DepArray { element_ty, len }` | 18 | Dependent array carrying compile-time length |
| `fn infer_array_literal_length` | 64 | Infer NatValue length from literal arity |
| `fn check_literal_length` | 69 | Length-vs-annotation check |
| `enum BoundsCheckResult` | 87 | Bounds outcome (InBounds / OutOfBounds / Unknown) |
| `fn check_bounds` | 97 | Compile-time bounds check |
| `fn concat_type` | 124 | Concat type derivation (m+n length) |
| `enum SliceConversion` | 143 | Slice-conversion outcome |
| `fn check_slice_conversion` | 153 | Slice compatibility check |
| `fn validate_window` | 175 | windows(K) validity check |
| `fn windows_count` | 199 | windows(K) result-length derivation |
| `fn split_at_types` | 217 | split_at(k) type pair derivation |
| `fn validate_split_index` | 230 | split_at(k) bounds check |
| `fn format_length_mismatch` | 251 | Error formatter |
| `enum VecConversion` | 272 | Vec↔array conversion outcome |
| `fn check_vec_to_array` | 282 | Runtime-checked vec→array |

### 2.2 patterns.rs — 867 LOC, 40 lib `#[test]` fns

| Pub item | Line | Role |
|---|---|---|
| `enum NatPattern` | 16 | Literal / Range / Var / Wildcard nat pattern |
| `struct NatMatchArm` | 43 | Pattern + body pair |
| `fn nat_pattern_matches` | 51 | Pattern-value match |
| `enum ExhaustivenessResult` | 68 | Exhaustive / NonExhaustive / Redundant |
| `fn check_nat_exhaustiveness` | 81 | Pattern-set exhaustiveness check |
| `struct ProofWitness` | 119 | Term + constraint pair |
| `fn prove_constraint` | 127 | Mechanical proof check |
| `enum SafeIndexResult` | 144 | Index-safety verdict |
| `fn check_safe_index` | 156 | Compile-time safe-index check |
| `struct DepBranch` | 177 | Dependent if/else branch |
| `enum NatCondition` | 188 | Eq / Lt / Le / Gt / Ge / And / Or / Not |
| `fn eval_nat_condition` | 200 | Evaluate nat condition with env |
| `fn resolve_dep_branch` | 218 | Pick branch via condition |
| `struct WhereClause` | 234 | Where-clause storage |
| `struct NatRange` | 274 | [lo, hi] nat range |
| `struct InductiveProof` | 313 | base/step proof structure |
| `enum ProofTerm` | 341 | Refl / Sym / Trans / Cong / Trustme |
| `struct DepFnSignature` | 380 | Dependent fn type sig |
| `fn resolve_dep_return_type` | 392 | Return-type resolution |

## §3. Consumer trace (HEAD `94c61998` — re-verified live 2026-05-12)

### 3.1 Direct `use crate::dependent::arrays` consumers

```
$ grep -rln "use crate::dependent::arrays\|dependent::arrays::" src/ tests/ examples/
src/interpreter/eval/mod.rs
```

**Only one consumer.** All matches inside `dt2_*` `#[test]` fns
(L5978..L6125, 10 tests dt2_1..dt2_10). Zero production code paths.

### 3.2 Direct `use crate::dependent::patterns` consumers

```
$ grep -rln "use crate::dependent::patterns\|dependent::patterns::" src/ tests/ examples/
src/interpreter/eval/mod.rs
```

**Only one consumer.** All matches inside `dt4_*` `#[test]` fns
(L6133..L6265, 10 tests dt4_1..dt4_10). Zero production code paths.

### 3.3 Cross-check: load-bearing siblings

| Module | Production consumers | Verdict |
|---|---|---|
| `dependent::nat` | `src/const_generics.rs` (4 imports: `ConstType`, `MonoKey`, `NatConstraint`, `NatValue`) | **Load-bearing — keep** |
| `dependent::arrays` | `src/interpreter/eval/mod.rs` only inside `dt2_*` `#[test]` | **Dead in production** |
| `dependent::patterns` | `src/interpreter/eval/mod.rs` only inside `dt4_*` `#[test]` | **Dead in production** |
| `dependent::tensor_shapes` | (deleted v35.7.2) | n/a |

### 3.4 `dependent/mod.rs` current state

```rust
//! Dependent types — const generics, type-level integers, compile-time shape
//! verification for arrays.
//!
//! v35.7.2 (2026-05-12): `tensor_shapes` module removed per Compass §5.1
//! dependent-types verdict and `docs/TENSOR_SHAPES_LOAD_BEARING_B0_FINDINGS.md`
//! (dead in production — only consumed by sprint DT3 tests, never by
//! production analyzer/codegen). `arrays` + `patterns` follow the same
//! dead pattern; their deletion deferred to a follow-up Action C if/when
//! Fajar decides to commit fully to the Compass §5.1 dep-types freeze.

pub mod arrays;
pub mod nat;
pub mod patterns;
```

The header comment already documents the deferral. Action C extension
closes that loop by deleting both modules + updating the header.

## §4. Verdict: arrays.rs + patterns.rs DEAD in production (state unchanged post-EOS-33)

Both modules confirmed zero production consumers. Identical pattern to
tensor_shapes.rs. Predecessor B0 verdict stands; no state drift.

## §5. Deletion scope (concrete)

### 5.1 File deletions (mechanical)

| Path | LOC | Lib tests removed |
|---|---|---|
| `src/dependent/arrays.rs` | 502 | 23 |
| `src/dependent/patterns.rs` | 867 | 40 |
| **Total** | **1,369** | **63** |

### 5.2 `src/dependent/mod.rs` edits

- Remove `pub mod arrays;` line
- Remove `pub mod patterns;` line
- Update header comment: extend the v35.7.2 dep-types-freeze note to
  cover arrays + patterns as well, citing this B0 + a new
  `docs/decisions/2026-05-12-arrays-patterns-deletion.md`.

### 5.3 `src/interpreter/eval/mod.rs` test-site deletions

| Range | Tests | Lines (approx) |
|---|---|---|
| `dt2_1_dep_array_creation` .. `dt2_10_dep_array_length_propagation` | 10 | 5976..6125 (~150) |
| `dt4_1_nat_pattern_literal_match` .. `dt4_10_inductive_proof` | 10 | 6131..6265 (~135) |
| **Subtotal** | **20** | **~285 LOC** |

Existing v35.7.2 comment block (L6126..6128) about DT3 removal should
be expanded to also note DT2 + DT4 removal in the same session.

### 5.4 Keep untouched

- `dt1_1` .. `dt1_10` (L5857..5976): tests `dependent::nat` — load-bearing, MUST NOT delete.
- All LS1-LS4 tests starting at L6275 (`ls1_*`): unrelated, untouched.

## §6. Aggregate numbers (Action C extension)

- **LOC delta:** -1,369 (file deletions) + -~285 (test sites) ≈ **-1,654 LOC**
- **Lib test delta:** -63 (file tests) + -20 (eval/mod.rs tests) ≈ **-83 tests**
- **Post-deletion lib count estimate:** 7,585 - 83 = **~7,502 lib tests**
- **Test-target gate impact:** Stage 2 byte-equality unaffected (Rust-only changes; no stdlib `.fj` touched). Phase17 gate stays 4/4.
- **`context_safety_tests.rs` impact:** None (149/149 untouched).

Variance vs predecessor B0 §6 Action C estimate (~2,300 LOC, 120-130 tests):
this audit's precise count is **~28% smaller** than that earlier ballpark
because tensor_shapes is now already deleted (v35.7.2), so Action C extension
is the residual scope only.

## §7. Stage 2 byte-equality risk

NONE. arrays.rs + patterns.rs are Rust-only, never compiled by fjc.
`stdlib/*.fj` does not reference any pub item from either module
(re-verified: `grep -r "DepArray\|NatPattern\|check_bounds" stdlib/`
returns zero matches). Phase17 gate unaffected.

## §8. Recommendation

**SHIP Action C extension.** Risk is identical-or-lower than the v35.7.2
tensor_shapes deletion (smaller surface, same dead-in-production pattern,
plus the predecessor B0 already flagged this as a follow-up). The
`dependent/mod.rs` header already documents the deferral; ship closes
that documented promise.

Compass-aligned: completes the §5.1 dependent-types freeze verdict
("Bekukan; mungkin tidak kembali") by deleting the entire dead surface
in `dependent/`. Only `nat.rs` survives (rightly — it's the one module
load-bearing for const generics).

## §9. Re-entry conditions (post-deletion)

If arrays.rs or patterns.rs is ever needed:
1. `git log --diff-filter=D -- src/dependent/arrays.rs` (and patterns.rs)
   recovers the file from this session's deletion commit.
2. Same re-entry trigger as tensor_shapes: fj-source `analyzer.fj` gains
   compile-time array-shape / pattern-exhaustiveness checking, OR the
   `@device` flow needs dependent-array machinery as a future safety
   guarantee.
3. Until then, `nat.rs` covers the const-generic dimension and is
   sufficient (per `src/const_generics.rs`).

## §10. Action paths (Fajar to pick)

### Action A — Freeze in place
- Header-only comment in `dependent/mod.rs` (already partial — extend).
- 0 LOC reduction. **Effort:** ~5min. **Risk:** None.

### Action B — Delete one of the two (pick arrays OR patterns)
- Half-scope. **Effort:** ~30-45min. **Risk:** LOW.
- Not recommended: arbitrary split with no engineering rationale.

### Action C extension — Delete both (recommended)
- Full closure of dep-types freeze. **Effort:** ~1-1.5h. **Risk:** LOW.
- **LOC delta:** ~-1,654. **Test delta:** ~-83.

## §11. Verification commands (run pre- and post-deletion)

```bash
cd "/home/primecore/Documents/Fajar Lang"

# Pre-flight (run now, confirm B0 baseline)
grep -rln "use crate::dependent::arrays\|dependent::arrays::" src/ tests/ examples/
# expect: src/interpreter/eval/mod.rs (only)

grep -rln "use crate::dependent::patterns\|dependent::patterns::" src/ tests/ examples/
# expect: src/interpreter/eval/mod.rs (only)

grep -rln "use crate::dependent::nat" src/ | grep -v "src/dependent/"
# expect: src/const_generics.rs (load-bearing)

grep -r "DepArray\|NatPattern\|check_bounds\|nat_pattern_matches\|infer_array_literal_length\|prove_constraint" stdlib/
# expect: empty (Stage 2 byte-equality safe)

# Post-deletion (when shipped)
cargo test --lib 2>&1 | tail -3                          # expect ~7,502 passed
cargo clippy --lib -- -D warnings                        # expect: 0 warnings
cargo fmt -- --check                                     # expect: clean
cargo test --release --test selfhost_phase17_self_compile -- --test-threads=1
                                                         # expect: 4 passed (byte-equality preserved)
cargo test --test context_safety_tests                   # expect: 149 passed
```

## §12. Self-check (§6.8 audit checklist)

```
[x] Pre-flight audit (B0) exists for the Phase            (Rule 1)
[x] Every task has runnable verification command           (Rule 2 — §11)
[ ] Prevention mechanism added (hook/CI/rule)              (Rule 3 — Action C ship adds updated dependent/mod.rs header + commit-msg cite)
[x] Agent-produced numbers cross-checked with Bash         (Rule 4 — all consumer grep verified live; pub-item counts verified)
[ ] Effort variance tagged in commit message               (Rule 5 — at deletion commit time)
[ ] Decisions are committed files                          (Rule 6 — pending decision file at ship)
[x] Public-artifact drift swept                            (Rule 7 — done EOS-33 same session)
[x] Multi-repo state checked                               (Rule 8 — done EOS-33 same session)
```

## §13. Source artifacts

- This file: `docs/ARRAYS_PATTERNS_LOAD_BEARING_B0_FINDINGS.md`
- Predecessor B0 (tensor_shapes): `docs/TENSOR_SHAPES_LOAD_BEARING_B0_FINDINGS.md` §5 (already flagged arrays + patterns as bonus finding)
- v35.7.2 deletion commit: `94c61998` (tensor_shapes Action B)
- Compass §5.1 source: `docs/1/STRATEGIC_COMPASS.md` (dep types row, "Bekukan; mungkin tidak kembali")
- Compass §5 freeze-candidates audit: `docs/COMPASS_5_FREEZE_CANDIDATES_B0_FINDINGS.md` §2.2

---

*B0 written 2026-05-12 post-EOS-33 (on `lanjutkan` first-step). ~15min
actual / ~15min estimate (on-budget). Verdict: state unchanged; both
modules dead in production. Recommendation: Action C extension (delete
both + scoped test cleanup). Decision pending Fajar.*
