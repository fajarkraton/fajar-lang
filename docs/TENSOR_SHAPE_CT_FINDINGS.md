# Tensor Shape Compile-Time — Track Closure Findings (Compass §6.3)

> **Date:** 2026-06-12 (single session, P1→P4)
> **Plan:** `docs/TENSOR_SHAPE_CT_PLAN.md` · **B0:** `docs/TENSOR_SHAPE_CT_B0_FINDINGS.md`
> **Decision:** `docs/decisions/2026-06-12-tensor-shape-ct.md` (D1a/D2a/D3a ACCEPTED)
> **Commits:** P1 `e02ca768` · P2 `12d4effd` · P3 `35e6fa96` · P4 (this commit)

## Verdict: TRACK CLOSED — all 4 phases [x], E2E

CLAUDE.md §1 design principle 4 ("shape checked at compile time") is now
TRUE in its gradual form, end-to-end through `fj check`:

| Capability | Before | After |
|---|---|---|
| `matmul(zeros(2,3), zeros(4,5))` un-annotated | passed check, died at runtime | **TE001 at check** with span |
| `take(zeros(2,2))` into `Tensor<f64>[3,3]` param | passed check | **SE004** naming both shapes |
| `return "hello"` from `-> i64` (ANY type) | **never checked** | SE004 (general soundness fix) |
| `Tensor<f64>[B, I] × [I, O]` symbolic dims | no syntax | unified per call site; conflict → **TE011**; bound symbols substitute into return shape |
| Diagnostics | code+span only | + domain hints (TE001 traces constructors; TE011 names the symbol) |

Gradualness invariant held throughout: non-literal dims stay dynamic and
are never falsely rejected; runtime checks remain active (D1a).

## Per-phase actuals vs estimates

| Phase | Est (cap) | Actual | Variance |
|---|---|---|---|
| P1 shape-aware builtins | 5-7h (9h) | ~1h40m | -70% |
| P2 boundary + return | 3-5h (6h) | ~1h10m | -70% |
| P3 symbolic dims | 8-12h (16h) | ~1h50m | -80% |
| P4 diagnostics + docs + corpus | 2-4h (5h) | ~1h | -70% |
| **Total** | **~36h cap** | **~5h40m** | **-84%** |

Root cause of the overestimate (same lesson as HONEST_AUDIT_V33): the
machinery already existed deeper than the plan assumed — B0 found the
checker, syntax, and even `SymbolicShape` live; the work was wiring.

## Notable discoveries logged during the track

1. **P2 general soundness gap** — `return`-statement type checking was
   missing for ALL types, not just tensors. `fn f() -> i64 { return "x" }`
   passed `fj check` until `12d4effd`.
2. **TE010 collision** — plan said TE010 for symbolic conflicts; TE010 was
   already GPU OOM (runtime/gpu) so the new code is **TE011**.
3. **Tutorial Chapter 8 used non-parsing syntax** — `Tensor<f32, [128]>`
   (the form B0-P2 proved invalid) and stale error codes; rewritten with
   the real syntax + symbolic dims in P4.
4. **Test-counting convention** — the headline "integ" figure produced by
   `cargo test --tests` INCLUDES the lib unittest binary (e.g. 9,560 at
   track close = 6,616 lib + 2,944 integ-only). The historical "lib +
   integ + doc = total" formula therefore double-counts lib. Recorded
   here per §6.6 R3; convention now stated explicitly in CLAUDE.md §3.
5. **VM builtin gap (pre-existing)** — `fj run --vm` lacks `zeros` et al.
   (RE004), isolated via plain-annotation comparison; unrelated to this
   track.

## Prevention layer (§6.8 R3)

`tests/tensor_shape_ct.rs` — 18-probe permanent corpus (all 6 B0 probes +
P1 propagation/gradualness + P2 return enforcement + P3 unification/
substitution + P4 hint goldens), runs in the ci.yml main job on every
push. Error-code gate: TE011 cataloged + covered
(`audit_error_codes.py --strict` gap=0, covered 126).

## Gates at track close (all green)

```
cargo test --lib                       6,616 PASS / 0 FAIL
cargo test --tests --no-fail-fast      9,560 PASS / 0 FAIL (81 files; incl. lib binary)
cargo test --test tensor_shape_ct         18 PASS
cargo test --test error_code_coverage    104 PASS
selfhost phase17 byte-equality           4/4 (preserved through all 4 phases)
stress --test-threads=64 ×5              5/5
cargo clippy --all-targets -- -D warnings  exit 0
cargo fmt -- --check                       exit 0
audit_error_codes.py --strict              gap=0
audit_unsafe.py --strict                   PASS
examples sweep                             36 red = identical pre-track baseline
stdlib sweep                                9 red = identical pre-track baseline
E2E: fj check emits TE001/SE004/TE011 with spans + hints; fj run executes
     valid symbolic programs (interpreter); CI green at P1, P2, P3 commits
```

## Self-check (§6.8)

R1 B0 committed before work ✓ · R2 every phase verified by runnable
commands ✓ · R3 prevention corpus in CI ✓ · R4 no agent numbers (all
first-hand) ✓ · R5 variance tagged per commit ✓ · R6 D1-D3 decision file
ACCEPTED before P3 ✓ · R7 CLAUDE.md §1/§3/§7 + README + SPEC + GRAMMAR +
TUTORIAL + ERROR_CODES synced ✓ · R8 single-repo ✓.

## What stays open (honest scope line)

- Intra-body symbol equality (`x @ w` inside `dense` proving `I == I`
  without call-site info) — noted in D3a as future work.
- Static-prove-then-elide runtime checks (D1 option b) — future perf phase.
- VM tensor builtins (pre-existing gap, discovery #5).
