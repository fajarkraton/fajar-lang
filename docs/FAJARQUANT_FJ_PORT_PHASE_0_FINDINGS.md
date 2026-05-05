---
phase: 0 — Pre-flight audit for FajarQuant Rust → Fajar Lang port
status: CLOSED 2026-05-05 (audit complete, gaps cleared, plan calibrated)
budget: 0.5-1d planned + 25% surprise = 1.25d cap
actual: ~30min Claude time (much less than budgeted because gaps are minimal)
variance: -90%
artifacts:
  - This findings doc
  - Audit baseline: fajarquant @ v0.4.0 (commit 192c14d, 50/50 lib tests PASS)
  - fj-lang @ v33.2.0 (commit 5e6384b5, FAJAROS_100PCT TERMINAL COMPLETE)
prereq: Option B agreement (port Rust algorithm to fj; keep Python training; keep vendored TL2)
---

# FajarQuant Rust → Fajar Lang Port — Phase 0 Findings

> Phase 0 of FajarQuant port plan. Audits Rust crate to identify
> fj-lang capability gaps, calibrates phase scope, decides whether
> Phase 1 (gap closure) sub-phases are needed. **Outcome: gaps are
> minimal; plan can proceed straight to Phase 2 (port simple modules)
> after a small Phase 1 scoping pass.**

## 0.1 — Rust crate baseline reproduction

| Metric | Value |
|---|---|
| Version | fajarquant 0.4.0 (commit `192c14d`) |
| `cargo build --release` | clean |
| `cargo test --release --lib` | 50/50 PASS in 0.09s |
| Pure-Rust algorithm LOC (port surface) | 2,649 across 7 files |
| Skipped (vendor TL2) | 1,502 LOC across 2 files (`cpu_kernels/tl2.rs` + `tl2_encoder.rs`) |

Per-module LOC and complexity:

| Module | LOC | Complexity | Hotspots |
|---|---|---|---|
| `lib.rs` | 84 | trivial | docs + re-exports only |
| `cpu_kernels/mod.rs` | 28 | trivial | re-exports |
| `cpu_kernels/scalar_baseline.rs` | 263 | low | scalar reference impl |
| `fused_attention.rs` | 320 | med | codebook dot product, KV cache |
| `hierarchical.rs` | 401 | low | exponential bit decay schedule |
| `kivi.rs` | 500 | med | KIVI baseline (per-head per-channel quant) |
| `adaptive.rs` | 518 | **HIGH** | PCA rotation via power iteration |
| `turboquant.rs` | 535 | med | Lloyd-Max codebook + grouping |
| **Total port surface** | **2,649** | — | — |

## 0.2 — External Rust deps (sangat minim)

```toml
[dependencies]
ndarray = "0.16"

[build-dependencies]
cc = "1.0"  # only for vendored TL2 (skipped)

[dev-dependencies]
ndarray-rand = "0.15"  # tests only
criterion = "0.5"      # benchmarks only
```

Only ONE production dep: `ndarray`. Everything else is dev/build-only.

## 0.3 — ndarray usage inventory

Top method calls across pure-Rust modules:

| Method | Count | fj-lang equivalent |
|---|---|---|
| `.iter()` | 12 | ✅ fj-lang iterator support |
| `.len()` | 10 | ✅ builtin |
| `.dot(&...)` | 6 | ✅ `tensor_matmul` / `dot` |
| `.map(&...)` | 7 | ✅ closures + `.map()` |
| `.collect()` | 6 | ✅ |
| `.t()` (transpose) | 2 | ✅ `tensor_transpose` |
| `.sum()` | 2 | ✅ `tensor_sum` |
| `.fold(...)` | 2 | ✅ |
| `.sqrt()` | 4 | ✅ stdlib |
| `Array1::from_vec(...)` | 2 | ✅ `tensor_from_data` |
| `Array2::zeros((r, c))` | 2 | ✅ `tensor_zeros` |
| `Array1::from_shape_fn(d, |i| ...)` | 4 | ⚠️ closure-init — needs verification |

Type usage:

| Type | Usage | fj-lang equivalent |
|---|---|---|
| `Array1<f64>` | vectors | ✅ 1-D `Tensor` (TensorValue with shape `[N]`) |
| `Array2<f64>` | matrices | ✅ 2-D `Tensor` (TensorValue with shape `[R, C]`) |
| `Array2<u8>` | quant indices | ✅ `Tensor` supports any dtype incl. u8 |
| `Vec<Array1<f64>>` | calibration buffers | ✅ `Vec<Tensor>` or stacked 2-D tensor |
| `HashMap<(usize, usize), Array2<f64>>` | per-(layer,head) rotations | ✅ fj-lang Map |

## 0.4 — fj-lang Tensor stdlib coverage (THE KEY FINDING)

`grep "tensor_" src/interpreter/eval/builtins.rs | sort -u` returns **62
tensor builtins** exposed to `.fj` source code, including all FajarQuant
needs:

```
tensor_abs_max          tensor_argmax           tensor_concat
tensor_eye              tensor_flatten          tensor_from_data
tensor_full             tensor_kurtosis         tensor_linspace
tensor_matmul           tensor_max              tensor_mean
tensor_min              tensor_normalize        tensor_ones
tensor_randn            tensor_reduce           tensor_reshape
tensor_row              tensor_rows             tensor_select
tensor_set              tensor_shape            tensor_skewness
tensor_softmax          tensor_squeeze          tensor_std_axis
tensor_sub              tensor_sum              tensor_svd_ratio
tensor_topk             tensor_transpose        tensor_unsqueeze
tensor_var_axis         tensor_xavier           tensor_zeros
... + 27 more
```

Particularly notable:
- ✅ `tensor_var_axis` — per-axis variance (used in adaptive PCA)
- ✅ `tensor_std_axis` — per-axis std (KIVI per-channel scaling)
- ✅ `tensor_kurtosis_axis` / `tensor_skewness_axis` — distribution shape
- ✅ `tensor_svd_ratio` — already supports SVD-related computation
- ✅ `tensor_softmax` — fused attention denominator
- ✅ `tensor_topk` — codebook nearest-neighbor

`examples/tensor_stats_demo.fj` line 3 documents these exact ops as
**"all 12 new ops needed for FajarQuant v3 profiler"** — fj-lang stdlib
was already designed with FajarQuant integration in mind.

## 0.5 — Linalg gap CLEARED

Initial concern: `adaptive.rs` PCA rotation might require LAPACK/SVD
external library. **AUDIT FINDING: NOT A GAP.**

`adaptive.rs` line 152: `power_iteration_eigenvectors(&cov, dim, 50)` —
PCA implemented via **pure power iteration** with deflation + Gram-Schmidt
orthogonalization. No external linalg dependency. All ops are basic
matrix-vector multiply + element-wise arithmetic + sqrt — fully
implementable in fj-lang with existing tensor builtins.

```rust
fn power_iteration_eigenvectors(cov: &Array2<f64>, dim: usize, iterations: usize) -> Array2<f64> {
    let mut eigenvectors = Array2::zeros((dim, dim));
    let mut deflated = cov.clone();
    for k in 0..dim {
        let mut v = random_vector(dim);
        for _ in 0..iterations {
            v = deflated.dot(&v);  // ← tensor_matmul
            v /= v.dot(&v).sqrt();  // ← tensor_dot + sqrt
        }
        // ... store eigenvector, deflate
    }
    gram_schmidt(&mut eigenvectors, dim);
    eigenvectors
}
```

This is ~50 LOC of pure numerical code, mechanical to port.

## 0.6 — Iterator/closure support in fj-lang

Verified via existing examples that fj-lang supports closures and
iterator-style chaining:

- `examples/array_methods.fj` — `.map`, `.filter`, `.fold`, `.collect`
- `examples/gat_demo.fj` — closures with `|x| x * 2` syntax
- `examples/iterators_demo.fj` — multi-stage iterator pipelines
- `examples/bench_pipeline.fj` — fold + map combinations

Rust patterns like `vec.iter().map(...).collect::<Vec<_>>()` translate
directly to fj-lang `vec.map(...)`.

## 0.7 — `Array1::from_shape_fn(d, |i| ...)` — minor gap

Rust uses `Array1::from_shape_fn` 4× to initialize 1-D arrays via
closure. fj-lang doesn't have an exact equivalent; closest patterns:

- `tensor_arange` for sequential init
- `tensor_linspace` for evenly-spaced
- Build via loop + `tensor_set`
- Or: hand-roll `tensor_from_shape_fn` builtin (~30 LOC in
  `src/runtime/ml/`)

**Recommendation**: write a small fj-lang stdlib helper
`fn tensor_init_with(shape: [i64], f: closure) -> Tensor` in pure fj. No
fj-lang core change needed.

## 0.8 — Const generics for tensor dimensions

Rust uses `Array2<f64>` (dim type-erased to `Ix2`) — fj-lang `Tensor`
also dynamic. **No const-generic gap to close** for this port; just
type as `Tensor` everywhere. Bounds checking happens at runtime
(matching Rust ndarray behavior).

## 0.9 — Risk reassessment

| Original risk | Phase 0 finding | Updated severity |
|---|---|---|
| Linalg gap (SVD) | Power iteration in source — no external linalg | ✅ CLEARED |
| Tensor stat ops missing | 62 builtins exposed, all major ops present | ✅ CLEARED |
| Iterator/closure ergonomics | Multiple working examples | ✅ CLEARED |
| `Array1::from_shape_fn` closure init | Minor — small fj stdlib helper | 🟡 LOW (~30min work) |
| Const generics for tensor dims | Not used in this port | ✅ N/A |
| **Compiler bugs surfaced by port** | (unknown until ports start) | 🟡 MEDIUM (FAJAROS_100PCT pattern) |

Single residual risk: fj-lang LLVM backend bugs may surface during
port (per FAJAROS_100PCT pattern of 1-2 gaps per major migration).
Mitigation: port via Cranelift JIT path during dev (fj-lang's primary
backend); switch to LLVM only for final integration. Cranelift was
fully production-validated through V33.

## 0.10 — Calibrated plan changes

Original plan estimate: 10-18 days realistic (12-24 with surprise).

**Phase 0 finding revisions:**
- Phase 1 (gap closure) compresses from 1-3 days to **~0.5 day** (just
  the `tensor_init_with` helper + verification tests). All other Phase 1
  sub-tasks were premature — ops already exist.
- Phase 4 (adaptive.rs with PCA) drops from "HIGH risk 2-4 days" to
  **"MEDIUM risk 1.5-2.5 days"** because power iteration is straightforward.
- Phase 0 itself came in at 30min vs 0.5-1d budgeted (-90% variance).

**Updated effort estimate:**

| Phase | Was | Updated |
|---|---|---|
| 0 | 0.5-1d | ✅ 30min DONE |
| 1 | 1-3d | 0.5d (tensor_init_with helper + verify) |
| 2 | 1-2d | 1-2d unchanged (660 LOC simple) |
| 3 | 3-5d | 3-5d unchanged (1,355 LOC mid) |
| 4 | 2-4d | 1.5-2.5d (PCA easier than feared) |
| 5 | 1-2d | 1-2d unchanged |
| 6 | 1d | 1d unchanged |
| 7 | 0.5d | 0.5d unchanged |
| **Total realistic** | **10-18.5d** | **8.5-13.5d (-15-30%)** |
| With +25% surprise | 12-24d | **10.5-17d** |

## 0.11 — Decision points (resolved)

1. ✅ **Scope**: 7 pure-Rust modules (~2,649 LOC). Skip TL2 (1,502 LOC vendored).
2. **Target location**: still TBD — recommend **Option C** (`fajar-lang` repo `src/runtime/ml/fajarquant/*.fj`) so it ships as fj-lang stdlib for easy `use fajarquant` from any .fj program. Rust crate `fajarquant` repo stays as-is for `crates.io` distribution.
3. ✅ **Linalg gap mitigation**: NONE NEEDED. Power iteration is pure arithmetic.
4. **Versioning**: tag fj-lang as **v33.3.0** when port lands (minor — adds stdlib module, doesn't break API).
5. **Timing**: ready to start now post-Phase-0.

## 0.12 — Phase 1 scope (compressed)

Per Phase 0 findings, Phase 1 is now a single sub-task:

| Task | Verification |
|---|---|
| 1.A Add `tensor_init_with(shape, closure)` helper to fj-lang stdlib (or write as pure-fj helper file) — provides equivalent to ndarray's `Array1::from_shape_fn` | new `tests/tensor_init_with.rs` 3+ cases; existing 8974 lib tests still PASS |
| 1.B Phase 1 findings doc | committed |

**Effort**: ~30min — half day budget is for surprise discovery.

## Decision gate (§6.8 R6)

This file committed → Phase 1 (compressed scope) UNBLOCKED. Decision
points #2 (target location) and #4 (versioning) resolved by founder
preference; recommendation is Option C (fj-lang repo) + v33.3.0 minor tag.

---

*FAJARQUANT_FJ_PORT_PHASE_0_FINDINGS — 2026-05-05. Audit complete in
~30min vs 0.5-1d budget (-90%). All major risks CLEARED. fj-lang
stdlib has 62 tensor builtins covering all FajarQuant ops; PCA uses
power iteration (no SVD library needed); iterators/closures working;
single minor gap is tensor_init_with helper. Plan effort revised
DOWN from 12-24d to 10.5-17d. Phase 1 compressed from 1-3d to ~0.5d.
Recommend proceeding to Phase 1 + Phase 2 (port simple modules:
hierarchical.rs + scalar_baseline.rs).*
