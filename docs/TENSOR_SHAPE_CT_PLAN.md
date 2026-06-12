# Tensor Shape Compile-Time Plan (Compass §6.3)

> **Date:** 2026-06-12 · **B0:** `docs/TENSOR_SHAPE_CT_B0_FINDINGS.md` (read first)
> **Goal:** make CLAUDE.md §1 principle 4 ("shape checked at compile time")
> TRUE for un-annotated code, fn boundaries, and shape-generic functions —
> the core embedded-ML differentiator per Compass §6.3.
> **Non-goal:** reopening dependent types (EOS-37 freeze stands). This is
> wiring + one surface-syntax addition, per B0 §5.

## §0. Decision gates (CLAUDE.md §6.8 R6 — committed file required before P3)

| # | Decision | Options | Recommendation |
|---|---|---|---|
| D1 | Runtime checks after static pass? | (a) keep always; (b) elide when statically proven | **(a) keep** for v1 — elision is a later perf phase; correctness > perf |
| D2 | Dynamic annotation form | (a) existing `Tensor<f64>[]`-style omission + `[*, 10]` wildcards; (b) add `Tensor<f64>[?]` | **(a)** — zero new syntax; `?` adds nothing the wildcard lacks |
| D3 | Symbolic-dim surface syntax (P3) | (a) uppercase ident in dim slot `Tensor<f64>[M, K]`, scoped to the fn, no decl needed; (b) explicit `fn f<[M, K]>` param list | **(a)** — matches wildcard precedent, no generics-grammar surgery; collision with type names impossible in dim slot |

Gate: `docs/decisions/2026-06-XX-tensor-shape-ct.md` ACCEPTED before P3 starts. P1-P2 are decision-free (pure gap-fix).

## §1. Phase P1 — shape-aware builtins (front door)

Fix B0 root cause: constructors fold integer-literal args into concrete
`dims`; shape-computing builtins get real signatures.

| # | Task | Files | Verify (runnable) |
|---|---|---|---|
| 1.1 | `zeros/ones/randn/fill/eye` with int-literal args → `Type::Tensor { dims: [Some(n)...] }`; non-literal args → dyn (unchanged) | `type_check/check.rs` call-typing + `register.rs` | `fj check` on B0-P1 file exits ≠0 with TensorShapeMismatch |
| 1.2 | `matmul(a,b)` call form routes through `matmul_shape()` + `verify_matmul` (same path as `@`) | `check.rs` (special-case builtin call) | B0-P1 probe red at check; `a @ b` and `matmul(a,b)` give identical diagnostics |
| 1.3 | Element-wise builtins (`add/sub/mul/relu/sigmoid/...`) propagate via `elementwise_shape()`; `transpose` swaps dims; `reshape` with literal target checks element-count (TE003-class semantic) | `check.rs` | new negative tests per op |
| 1.4 | Tensor literal `[[1,2],[3,4]]`-to-tensor paths (if typed as tensor) carry dims | `check.rs` | unit test |
| 1.5 | Regression: full suite + self-host byte-equality untouched | — | `cargo test --lib && cargo test --tests`; phase17 4/4 |

**Exit criterion:** B0 probes P1 and P4 both RED at `fj check` (P4 falls out:
`zeros(2,2)` now concrete `[2,2]` vs param `[3,3]` → existing P6 machinery
rejects). Est: 5-7h (+25% → cap 9h).

**Risk (pre-flight, §6.8 R1):** existing `.fj` examples/tests that rely on
dyn-everywhere may newly fail check — run `examples/` sweep
(`for f in examples/*.fj; do fj check $f; done`) BEFORE and AFTER; any new
red is either a real latent bug (fix the example, document as win) or an
over-strict inference (fix the rule). Budgeted inside the cap.

## §2. Phase P2 — boundary enforcement + return-position inference

| # | Task | Verify |
|---|---|---|
| 2.1 | Audit `is_compatible` matrix for `Tensor`: known×known mismatch = error (exists, P6), known×dyn = OK (gradual), rank mismatch when both ranks known = error | table-driven unit tests (9 cells) |
| 2.2 | Fn return type `-> Tensor<f64>[2, 3]` checked against inferred body shape | negative test: body returns `[3,2]` → SE004 at check |
| 2.3 | Shape flows through `let` without annotation (already works once P1 feeds dims — verify chain: `let a = zeros(2,3); let c = a @ zeros(3,4); c @ zeros(5,5)` → red) | chained probe red at check |
| 2.4 | Examples + stdlib `.fj` sweep green | sweep command above; 309/309 baseline preserved |

Est: 3-5h (+25% → cap 6h).

## §3. Phase P3 — symbolic dims at fn boundaries (the differentiator)

`fn dense(x: Tensor<f64>[B, I], w: Tensor<f64>[I, O]) -> Tensor<f64>[O]`
— per-call-site unification: bind `B/I/O` from arg shapes, error on
conflict (`I` vs `I`), substitute into return type. Reuses
`SymbolicShape`; monomorphization NOT needed (shape erased at runtime,
checking is analyzer-only).

| # | Task | Verify |
|---|---|---|
| 3.0 | Decision file D1-D3 ACCEPTED + committed | `bash scripts/check_decision_file.sh` pattern |
| 3.1 | Parser: uppercase ident allowed in dim slot of `TypeExpr::Tensor` | `fj dump-ast` golden |
| 3.2 | Analyzer: per-call unification map; conflict → new `TE010 SymbolicDimMismatch` (catalog + `audit_error_codes.py` entry) | negative tests: `dense(zeros(4,10), zeros(11,2))` → TE010 names `I: 10 vs 11` |
| 3.3 | Return substitution + propagation to caller | chained-call probe |
| 3.4 | Error message quality: dual-span (param decl + arg site) with bound values table | `error_display_golden`-style test |
| 3.5 | Tutorial chapter + `docs/FAJAR_LANG_SPEC.md` + `GRAMMAR_REFERENCE.md` sync | docs build + grep |

Est: 8-12h (+30% high-uncertainty → cap 16h).

## §4. Phase P4 — diagnostics, docs honesty, prevention layer

| # | Task | Verify |
|---|---|---|
| 4.1 | Domain suggestion: shape errors in `@device` fns hint the originating constructor span | golden test |
| 4.2 | Promote B0 probe corpus → `tests/tensor_shape_ct.rs` (≥12 cases incl. all 6 B0 probes) | `cargo test --test tensor_shape_ct` |
| 4.3 | CLAUDE.md §1 principle 4 + README tensor row updated to the now-true claim; ERROR_CODES.md TE010 | `check_version_sync.sh`-style grep in CI optional |
| 4.4 | Prevention (§6.8 R3): probe corpus runs in ci.yml main job (it does, via 4.2) + pre-push unchanged | CI green |

Est: 2-4h (+25% → cap 5h).

## §5. Sequencing + scope guard

P1 → P2 ship together (one release, minor bump — new compile errors are
**stricter-check** changes; per semver-for-langs precedent v35.5.0
affine-default this is acceptable in a minor with CHANGELOG migration
notes). P3+P4 next session(s). Total cap ≈ 36h across 3-4 sessions.

Compass §7 check: deepens core differentiator ✓ · solo-manageable ✓ ·
no new deps ✓ · runtime untouched (D1a) ✓.

## §6. Plan self-check (CLAUDE.md §6.8)

```
[x] R1 pre-flight audit committed (TENSOR_SHAPE_CT_B0_FINDINGS.md)
[x] R2 every task row has a runnable verification
[x] R3 prevention layer (P4.2 probe corpus in CI)
[ ] R4 n/a — no agent-produced numbers in this plan
[x] R5 estimates carry +25-30% surprise budget
[x] R6 decision gate D1-D3 = committed file blocking P3
[x] R7 public artifacts (CLAUDE.md/README/spec) sync scheduled at P4.3
[x] R8 single-repo plan; no cross-repo state involved
```
