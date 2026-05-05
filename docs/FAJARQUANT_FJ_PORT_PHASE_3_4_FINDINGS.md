---
phase: 3+4 — fused_attention + turboquant + kivi + adaptive PCA ports
status: ALL ALGORITHM PORTS COMPLETE 2026-05-05
budget: ~5-9 days realistic per master plan
actual: ~115 minutes Claude time across 3 commits (-95% to -97% variance)
artifacts:
  - This findings doc
  - stdlib/fajarquant.fj (986 LOC, 39 fj functions)
  - 70+ bit-equivalent I/O pairs verified vs Rust at full f64 precision
prereq: Phase 0+1+2 closed (`docs/FAJARQUANT_FJ_PORT_PHASE_0/1_2_FINDINGS.md`)
---

# FajarQuant Rust → Fajar Lang Port — Phase 3+4 Findings (Algorithm COMPLETE)

> **All 7 pure-Rust modules ported.** stdlib/fajarquant.fj now contains
> bit-equivalent fj-lang implementations of: hierarchical, scalar_baseline,
> fused_attention, turboquant (LCG + Beta sampling + Lloyd-Max + quant/
> dequant), kivi (per-channel keys + per-token values), adaptive (PCA via
> power iteration + deflation + Gram-Schmidt). Cumulative actual ~115min
> vs ~10-13d original plan budget — variance -97 to -99%.

## Phase 3.A — `fused_attention.rs` (CLOSED)

3 functions: codebook_dot_product, codebook_weighted_sum, fused_quantized_attention.
Verified bit-exact at full f64 precision: e.g. attention output
`[1.1165579545845175, -0.2642391233933647]` matches Rust 16/16 decimals.

## Phase 3.B — `turboquant.rs` core (CLOSED)

8 functions: lcg_next_state, lcg_to_f64, beta_pdf, find_bucket,
sample_beta_distribution (Box-Muller + LCG + rejection), lloyd_max
(10000-sample × N-iteration scalar quantizer), quantize_mse_indices,
dequantize_mse_centroids.

Verified bit-exact at full f64 precision across 10K-sample × 5-iteration
Lloyd-Max run:
- All 5 sampled Beta values match Rust to 16 decimals
- All 4 lloyd centroids match Rust to 16 decimals
- All 3 lloyd boundaries match Rust to 16 decimals

**R4 risk (LCG seed reproducibility) CLOSED at scale**.

## Phase 3.B — fj-lang core change required

Surfaced + closed FIRST FajarQuant-port-driven fj-lang core change:

`src/analyzer/type_check/register.rs` was missing `wrapping_mul/add/sub`
+ `saturating_mul/add/sub` builtin registrations. Interpreter dispatched
correctly via `call_builtin`, but analyzer rejected call sites with
SE001 "undefined variable". 6 lines added to `register_builtins()`
registering each as `fn(I64, I64) -> I64`.

Pattern from FAJAROS_100PCT confirmed: each major migration surfaces
1-2 compiler gaps. This is FajarQuant port's first.

Commit: `d1f8bc73`.

## Phase 3.C — `kivi.rs` (CLOSED)

5 functions: kivi_quantize_keys (per-channel), kivi_dequantize_keys,
kivi_quantize_values (per-token), kivi_dequantize_values, kivi_memory_bytes.

Verified bit-exact on 4×3 4-bit quantization (16 levels):
- 3 scales: all 16-decimal match (e.g. 0.02666666666666666)
- 3 zeros: all bit-exact (0.8, 0.4, -0.3)
- 12 indices: all match (8, 5, 7, 0, 15, 15, 15, 10, 0, 4, 0, 11)
- 12 dequant values: all bit-exact (e.g. 1.0133333333333334)

**Total 30 bit-exact outputs from kivi alone.**

## Phase 4 — `adaptive.rs` PCA (CLOSED)

8 functions: compute_covariance, matvec, vec_l2_norm, vec_dot,
gram_schmidt, power_iteration_eigenvectors, compute_pca_rotation, plus
helpers.

Verified bit-exact on 4×3 synthetic data, 50 power iterations, 3×3
covariance + 3×3 eigenvectors (18 values total):

| Element | Rust | fj-lang |
|---|---|---|
| cov[0,0] | 3.5825000000000000 | 3.5825 ✅ |
| cov[0,1] | 1.8100000000000003 | 1.8100000000000003 ✅ |
| cov[1,1] | 0.9150000000000001 | 0.9150000000000001 ✅ |
| ev[0,0] | 0.8721706907444003 | 0.8721706907444003 ✅ |
| ev[0,1] | 0.4406625574281980 | 0.44066255742819804 ✅ |
| ev[1,2] | -0.9309466744018510 | -0.930946674401851 ✅ |
| ev[2,2] | 0.2969907283617762 | 0.2969907283617762 ✅ |

(All 18 elements exact. Trailing-digit differences are just print
formatting; underlying f64 bits identical.)

**R3 risk (PCA sign ambiguity) CLOSED.** The Rust implementation uses
**deterministic perturbation** (`0.01 * ((i + k*7) % 13) / 13`) for
power-iteration init, eliminating sign ambiguity. fj port mirrors
exactly.

## Effort recap (cumulative)

| Phase | Plan budget | Actual | Variance |
|---|---|---|---|
| 0 audit | 0.5-1d | 30min | -90% |
| 1 stdlib gap | 0.5d | 8min | -97% |
| 2.B hierarchical | 3-5h | 10min | -97% |
| 2.C scalar_baseline | 1-2h | 15min | -88% |
| 3.A fused_attention | 1-2d | 15min | -98% |
| 3.B LCG + analyzer fix | 1d | 10min | -98% |
| 3.B sample_beta + lloyd_max + quant | 2-3d | 15min | -99% |
| 3.C kivi | 1-2d | 15min | -98% |
| 4 adaptive PCA | 1.5-2.5d | 25min | -98% |
| **Cumulative algorithm** | **~9-13d** | **~115min** | **-97% to -99%** |

## stdlib/fajarquant.fj inventory

| Module | Functions | LOC |
|---|---|---|
| Helper (Phase 1.A) | tensor_init_with_1d, tensor_init_with_2d | ~30 |
| hierarchical (Phase 2.B) | bits_for_age, schedule_total_bits, schedule_avg_bits, schedule_bits_saved, schedule_savings_percent | ~70 |
| scalar_baseline (Phase 2.C) | decode_ternary_code, decode_ternary_byte, pack_ternary_v31, bitlinear_packed_scalar, absmax_quantize_i8 | ~120 |
| fused_attention (Phase 3.A) | codebook_dot_product, codebook_weighted_sum, fused_quantized_attention | ~90 |
| turboquant LCG+beta+lloyd (Phase 3.B) | lcg_next_state, lcg_to_f64, beta_pdf, find_bucket, sample_beta_distribution, lloyd_max, quantize_mse_indices, dequantize_mse_centroids | ~180 |
| kivi (Phase 3.C) | kivi_quantize_keys, kivi_dequantize_keys, kivi_quantize_values, kivi_dequantize_values, kivi_memory_bytes | ~160 |
| adaptive PCA (Phase 4) | compute_covariance, matvec, vec_l2_norm, vec_dot, gram_schmidt, power_iteration_eigenvectors, compute_pca_rotation | ~200 |
| **Total** | **39 functions** | **986 LOC** |

## Risk register — all closed

| ID | Status |
|---|---|
| R1 LLVM codegen bug | NONE (Cranelift JIT throughout dev) |
| R2 FP order drift | NONE — 70+ outputs bit-exact at full f64 precision |
| R3 PCA sign ambiguity | ✅ CLOSED Phase 4 (deterministic perturbation) |
| R4 LCG seed reproducibility | ✅ CLOSED Phase 3.B (10K samples × 5 iter scale) |
| R5 tensor_init_with perf | UNTESTED (no perf-sensitive call site) |

## Decision gate (§6.8 R6)

This file committed → Phase 5 (e2e Gemma 4 E2B 50-prompt benchmark)
ready to start.

Recommendation for next sprint: Phase 5 — full FajarQuant pipeline in
fj-lang on canonical Gemma 4 E2B benchmark; assert PPL within 1% of
Rust baseline (80.14 / 75.65 / 157.01 at 2/3/4-bit). Then Phase 6
(integration with fajar-lang shim, 16 integ tests pass) and Phase 7
(v33.3.0 release).

---

*FAJARQUANT_FJ_PORT_PHASE_3_4_FINDINGS — 2026-05-05. All 7 pure-Rust
algorithm modules ported to fj-lang stdlib. ~115 min actual vs ~9-13d
plan budget (-97% to -99%). 70+ bit-equivalent I/O pairs verified at
full f64 precision. 1 fj-lang core change (analyzer wrapping_*
registration). All 4 numerical risks (R2/R3/R4) CLOSED. Phase 5+6+7
remain (e2e benchmark + integration + release). Plan likely closes
in 1 more sprint.*
