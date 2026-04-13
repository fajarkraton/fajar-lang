# FajarQuant v3 "Adaptive Per-Head Method Selection" — Production Plan

> **Version:** 1.2 (B-fix) | **Created:** 2026-04-13 | **Updated:** 2026-04-13 | **Author:** Fajar + Claude Opus 4.6
> **Predecessor:** V26 C1.6 Path B (complete, 2026-04-13)
> **Rule compliance:** CLAUDE.md §6.8 (8 Plan Hygiene Rules) + §6.9 (7 Research Integrity Rules)
> **Surprise budget:** +25% standard, +30% for Phase B (algorithm, high uncertainty)

---

## Context

FajarQuant v2 (B4.G result): NO single method dominates across KV-head
architectures. KIVI wins 6/9 cells, TQ outlier wins Gemma 2-bit, FQ v2
is strongest non-KIVI on MQA/narrow-GQA. Root cause: v2 uses ONE strategy
for ALL heads. v3 solves this by profiling each head and selecting the
optimal quantization strategy per-head at inference time.

**Blue ocean:** No existing paper does per-head method selection. Every
competitor (KIVI, TurboQuant, SpinQuant, KVQuant) uses one algorithm for
all heads. v3 is a meta-system, not a single algorithm.

**Target:** v3 wins ≥8/9 cells in the 3-model × 3-bit evaluation matrix
by matching KIVI on wide-GQA (selecting KIVI-like path) and beating TQ
on MQA (selecting rotation+outlier path).

## v3 Architecture: 5-Path Adaptive System

```
                     ┌──────────────┐
   KV head tensor →  │  PROFILER    │ → kurtosis, σ1/σ2, outlier%, CV, asymmetry
                     └──────┬───────┘
                            │
                     ┌──────▼───────┐
                     │  SELECTOR    │ → per-(layer, head, k/v) strategy assignment
                     └──────┬───────┘
                            │
              ┌─────────────┼─────────────────────┐
              │             │             │        │        │
         ┌────▼───┐   ┌────▼───┐   ┌────▼───┐ ┌──▼───┐ ┌──▼───┐
         │ Path A │   │ Path B │   │ Path C │ │Path D│ │Path E│
         │ KIVI   │   │ PCA    │   │ Hadam  │ │Resid.│ │Asym. │
         │ perchan│   │ rotate │   │+outlier│ │quant │ │quant │
         └────┬───┘   └────┬───┘   └────┬───┘ └──┬───┘ └──┬───┘
              └─────────────┴─────────────┴────────┴────────┘
                                    │
                     ┌──────────────▼──────────────┐
                     │  STRATEGY VERIFIER (B2.V)   │
                     │  mini-batch PPL check per   │
                     │  head — swap if better found │
                     └─────────────────────────────┘
```

### 5 Quantization Paths (vs 4 in original plan)

| Path | Strategy | When to use | Source |
|------|----------|-------------|--------|
| **A** | KIVI-like per-channel symmetric | Regular distributions (low kurtosis, high channel CV) | Reuse v2 KIVI code |
| **B** | Calibrated PCA rotation + per-coord | Structured low-rank (high σ1/σ2) | Reuse v2 PCA code |
| **C** | Hadamard + adaptive outlier extraction | Extreme outliers (high kurtosis) | Adapt v2 TQ outlier |
| **D** | Residual quantization (base + residual) | 2-bit regime where all single-pass methods struggle | NEW |
| **E** | Asymmetric per-channel (non-zero zero-point) | Non-zero-centered distributions (high asymmetry) | NEW |

### 3 Enhancements Beyond Original Plan

1. **Path E (Asymmetric):** Real KV cache distributions are often non-zero-centered (especially values after LayerNorm). Asymmetric quant (scale + zero_point per channel) captures this better than symmetric.

2. **Strategy Verifier (B2.V):** After initial threshold-based assignment, run a mini-batch validation (5 chunks) per head. If a different strategy produces lower reconstruction MSE, swap. This prevents threshold overfitting.

3. **Auto-Tuning Threshold Search (B2.T):** Instead of fixed thresholds, grid search over calibration data with cross-validation (80/20 split). Produces per-model-family optimal thresholds.

---

## Phase A: Fajar Lang Tensor Enhancements (prerequisite)

> **Goal:** Add 9 missing tensor operations needed by v3 profiler + quantizer.
> **Effort:** 32h (25.5h + 25% surprise) = ~4 working days
> **Repo:** `~/Documents/Fajar Lang/`

### A0: Pre-flight audit

| # | Task | Verification | Est |
|---|------|-------------|-----|
| A0.1 | Baseline test count + ndarray axis API check | `cargo test --lib 2>&1 \| tail -1` shows count; `grep var_axis Cargo.lock` | 0.5h |
| A0.2 | Multi-repo state check | `bash scripts/multi-repo-check.sh` GREEN | 5 min |

### A1-A3: Per-Axis Statistical Reductions

| # | Task | Verification | Est |
|---|------|-------------|-----|
| A1 | `var_axis(tensor, axis)` in `ops.rs` + builtin wire | `cargo test --lib -- var_axis` ≥ 3 tests pass | 2h |
| A2 | `std_axis(tensor, axis)` in `ops.rs` + builtin wire | `cargo test --lib -- std_axis` ≥ 3 tests pass | 1h |
| A3 | `kurtosis_axis(tensor, axis)` — excess kurtosis E[(x-μ)⁴]/σ⁴ - 3 | `cargo test --lib -- kurtosis` ≥ 3 tests pass | 3h |

**Implementation:** ndarray has native `var_axis(Axis(n), ddof)` and `std_axis`. Kurtosis: manual via `map_axis`.

### A4: SVD Ratio

| # | Task | Verification | Est |
|---|------|-------------|-----|
| A4 | `svd_ratio(tensor)` → σ1/σ2 via eigendecomp of AᵀA | `cargo test --lib -- svd_ratio` ≥ 3 tests pass | 4h |

**Implementation:** Eigendecomp of covariance (already used in fajarquant adaptive.rs). No new dependency — compute via power iteration or ndarray built-in.

### A5: Tensor Select

| # | Task | Verification | Est |
|---|------|-------------|-----|
| A5 | `select(tensor, dim, index)` — extract slice along dim | `cargo test --lib -- select` ≥ 3 tests pass | 2h |

**Implementation:** `slice_axis(Axis(dim), idx..idx+1)` then `remove_axis`.

### A6: Per-Channel Quantization (KIVI-style)

| # | Task | Verification | Est |
|---|------|-------------|-----|
| A6 | `quantize_per_channel(tensor, bits, axis)` — per-channel scale factors | `cargo test --lib -- per_channel` ≥ 5 tests pass | 5h |

**Implementation:** New `PerChannelQuantizedValue` struct in `quantize.rs` with `scales: Vec<f64>` (one per channel). Requires new `Value::PerChannelQuantized` variant OR extend existing `QuantizedValue` with optional `channel_scales`.

### A7: Residual Quantization

| # | Task | Verification | Est |
|---|------|-------------|-----|
| A7 | `quantize_residual(tensor, bits_base, bits_res)` | `cargo test --lib -- residual` ≥ 3 tests pass | 4h |

**Implementation:** Quantize → dequantize → compute residual → quantize residual → return (base, residual) pair.

### A7.5: Asymmetric Quantization (Path E support)

| # | Task | Verification | Est |
|---|------|-------------|-----|
| A7.5 | `quantize_asymmetric(tensor, bits, axis)` — per-channel with non-zero zero_point | `cargo test --lib -- asymmetric` ≥ 4 tests pass | 4h |

**Implementation:** Extend `QuantizedValue` or create `AsymmetricQuantizedValue` with `zero_points: Vec<f64>`. Formula: `q = round((x - zero_point) / scale)`, `x_hat = q * scale + zero_point`. Zero point = `(min + max) / 2` per channel.

### A8-A9: Outlier Detection

| # | Task | Verification | Est |
|---|------|-------------|-----|
| A8 | `abs_max_axis(tensor, axis)` — per-channel absolute max | `cargo test --lib -- abs_max` ≥ 3 tests pass | 1.5h |
| A9 | `topk_indices(tensor, k, axis)` — top-K channel indices | `cargo test --lib -- topk` ≥ 3 tests pass | 3h |

### A9.5: Profiling Statistics Builtins

| # | Task | Verification | Est |
|---|------|-------------|-----|
| A9.5a | `tensor_skewness(tensor, axis)` — asymmetry measure for Path E selector | `cargo test --lib -- skewness` ≥ 3 tests pass | 2h |
| A9.5b | `tensor_channel_cv(tensor, axis)` — coefficient of variation per channel | `cargo test --lib -- channel_cv` ≥ 3 tests pass | 1.5h |

### A10: Builtin Wiring + Integration Test

| # | Task | Verification | Est |
|---|------|-------------|-----|
| A10.1 | Register all 12 builtins in `builtins.rs` dispatch + `mod.rs` env + `register.rs` analyzer | `cargo run -- run examples/tensor_stats_demo.fj` | 2.5h |
| A10.2 | Integration test file `tests/tensor_axis_ops.rs` | `cargo test --test tensor_axis_ops` ≥ 20 tests pass | 2.5h |
| A10.3 | Example `examples/tensor_stats_demo.fj` — all new ops exercised | E2E runs without error | 1h |

### Gate A-DONE

```
cargo test --lib && cargo clippy --lib -- -D warnings && cargo fmt -- --check
```
Test count must increase by ≥40 from A0 baseline. All 12 new functions tested.

**Prevention (Rule 3):** Meta-test `all_axis_ops_reject_invalid_axis()` that verifies all axis-accepting functions reject axis ≥ ndim. Meta-test `all_quant_modes_roundtrip()` that verifies per-channel, asymmetric, and residual roundtrip within tolerance.

**Key files:**
- `src/runtime/ml/ops.rs` — 9 new public functions (var, std, kurtosis, svd_ratio, select, abs_max, topk, skewness, channel_cv)
- `src/runtime/ml/quantize.rs` — per-channel + asymmetric + residual quantization
- `src/interpreter/eval/builtins.rs` — 12 new dispatch entries
- `src/interpreter/eval/mod.rs` — 12 new names in builtin env
- `src/analyzer/type_check/register.rs` — 12 new type registrations

> **Phase A effort revised:** 38h (30h + 25% surprise) = ~5 working days

---

## Phase B: FajarQuant v3 Python Algorithm

> **Goal:** Build the adaptive per-head quantization system in Python.
> **Effort:** 37.5h (30h + 30% surprise) = ~5 working days
> **Repo:** `~/Documents/fajarquant/`

### B0: Pre-flight + Literature Review

| # | Task | Verification | Est |
|---|------|-------------|-----|
| B0.1 | Reproduce v2 baseline: re-run 1 cell as sanity | `diff <(jq .kivi_2bit.ppl data/kv_cache/perplexity_v2_mistral.json) <(echo 23.96)` | 0.5h |
| B0.2 | Literature sweep: ≥8 papers on per-head/mixed-precision KV quant | `docs/V26_C1_6_V3_LITERATURE.md` committed with landscape table | 3h |
| B0.3 | GPU + disk state check | `nvidia-smi` free ≥ 14 GB; `df -h /` free ≥ 200 GB | 5 min |

**Papers to survey (§6.9 R2):** KIVI, KVQuant, SKVQ, TurboQuant, MiKV (mixed-precision per-head), GEAR (residual KV quant), QServe (channel rebalancing), Coupled Quantization.

### B1: Per-Head Statistical Profiler

| # | Task | Verification | Est |
|---|------|-------------|-----|
| B1.1 | `scripts/profile_kv_heads.py` — per-(layer, head, k/v) kurtosis, σ1/σ2, outlier%, channel_var_cv | `python3 scripts/profile_kv_heads.py --model google/gemma-4-E2B --output data/calibration/profile_gemma.json` | 6h |

Output format: `{ "layers": [ { "heads": [ { "k": { "kurtosis": ..., "svd_ratio": ..., "outlier_frac": ..., "cv": ... }, "v": {...} } ] } ] }`

### B2: Strategy Selector + Auto-Tuning

| # | Task | Verification | Est |
|---|------|-------------|-----|
| B2.1 | `scripts/strategy_selector.py` — threshold-based mapping (5 paths) | `python3 -m pytest tests/test_strategy_selector.py` | 4h |
| B2.T | Auto-tuning: grid search over thresholds on calibration data with 80/20 cross-val | `scripts/tune_thresholds.py --model gemma --output data/calibration/thresholds_gemma.json` produces optimal thresholds | 4h |
| B2.V | Strategy verifier: mini-batch PPL check per head, swap if better strategy found | `python3 -m pytest tests/test_strategy_verifier.py` ≥ 3 tests | 3h |

Decision tree (5 paths):
```
for each (layer, head, k_or_v):
  stats = profile[layer][head][k_or_v]

  if stats.channel_var_cv > T_cv:          → Path A (KIVI-like per-channel)
  elif stats.svd_ratio > T_svd:            → Path B (PCA rotation)
  elif stats.kurtosis > T_kurt:            → Path C (Hadamard + outlier)
  elif abs(stats.skewness) > T_skew:       → Path E (Asymmetric per-channel)
  elif bits == 2 and stats.kurtosis > 3:   → Path D (Residual)
  else:                                    → Path A (KIVI, safe default)

  # B2.V: Verify assignment against mini-batch reconstruction MSE
  if verify_mode:
    best = argmin([mse(path, head_data) for path in [A,B,C,D,E]])
    if best != assigned: swap
```

Initial thresholds (refined by B2.T auto-tuning):
- `T_cv = 2.0` (channel variance coefficient of variation)
- `T_svd = 5.0` (σ1/σ2 ratio)
- `T_kurt = 6.0` (excess kurtosis)
- `T_skew = 1.5` (absolute skewness)

### B3-B5.5: Quantization Paths (mostly reuse v2 code)

| # | Task | Verification | Est |
|---|------|-------------|-----|
| B3 | Path A: KIVI-like — reuse `apply_kivi_*_4d` from `quant_attention.py` | Already tested | 0h |
| B4 | Path B: PCA rotation — reuse `apply_fajarquant_v2_4d` from `quant_attention_v2.py` | Already tested | 0h |
| B5 | Path C: Hadamard + adaptive outlier — adapt TQ outlier per-head with auto threshold (not fixed 15%) | Smoke test synthetic | 2h |
| B5.5 | Path E: Asymmetric per-channel — `apply_asymmetric_4d` with non-zero zero_point | `python3 -m pytest tests/test_asymmetric.py` ≥ 3 tests | 3h |

### B6: Residual Quantization (Path D)

| # | Task | Verification | Est |
|---|------|-------------|-----|
| B6.1 | `scripts/quant_residual.py` — base quant + residual quant pipeline | `python3 -m pytest tests/test_residual.py` ≥ 5 tests | 5h |
| B6.2 | Residual bit allocation optimizer — determine optimal base/residual split per head | Unit test: optimal split for known synthetic distributions | 2h |

### B7: Multi-Path Dispatcher

| # | Task | Verification | Est |
|---|------|-------------|-----|
| B7.1 | `scripts/quant_attention_v3.py` — per-head dispatch based on strategy map | Smoke test: 1 model, all 5 methods logged per-head | 6h |
| B7.2 | Same-strategy batching optimization — group heads with same path for vectorized execution | Benchmark: batched dispatch ≤ 1.5x overhead vs uniform method | 2h |

### B8: Calibration Pipeline v3

| # | Task | Verification | Est |
|---|------|-------------|-----|
| B8.1 | `scripts/calibrate_fq_v3.py` — profile + calibrate + auto-tune + verify + save | `.npz` with per-head strategy + per-head calibration data + threshold config | 5h |
| B8.2 | Online profiling mode — profile during first N tokens, no offline calibration needed | Smoke test: `--online-profile --warmup-tokens 512` flag works | 3h |

### B.G: Gate (pre-validation)

| # | Task | Verification | Est |
|---|------|-------------|-----|
| B.G.1 | Smoke test on Gemma 2-bit (10 samples) | v3 PPL < v2 PPL on same args | 0.5h |
| B.G.2 | Strategy assignment log: verify ≥2 different paths used across all models | `jq '.strategy_counts' data/calibration/fq_v3_*.npz.meta` shows multi-path | 0.5h |
| B.G.3 | Decision file | `docs/V26_C1_6_V3_DESIGN.md` committed with strategy rationale | 0.5h |

---

## Phase B-fix: Repair Selector + Tuner + Verifier Pipeline

> **Goal:** Fix the 5 defects found by C4 Gemma audit so v3 is genuinely
> adaptive, not a hardcoded lookup table.
> **Triggered by:** C4 Gemma results (v3 = TQ exactly, selector = lookup table)
> **Findings:** `docs/V26_C1_6_V3_BFIX_FINDINGS.md` (5 defects, commit `ce9eca1`+)
> **Effort:** 20h (16h + 25% surprise) = ~2.5 working days
> **Repo:** `~/Documents/fajarquant/`
> **Rule:** §6.8 Rule 3 — every fix must spawn a prevention mechanism

### B-fix.0: Pre-flight — Verify Calibration Data Availability

| # | Task | Verification | Est |
|---|------|-------------|-----|
| B-fix.0.1 | Verify `.npz` calibration files contain real KV cache data per head | `python3 -c "import numpy as np; d=np.load('data/calibration/fq_v2_gemma_4_e2b.npz'); print(list(d.keys())[:10])"` shows layer_*_k/v arrays | 15 min |
| B-fix.0.2 | Check all 3 models have `.npz` with per-head KV cache chunks | Same command for mistral + qwen2 `.npz` | 15 min |

### B-fix.1: Remove Architecture Gates from Strategy Selector (D1 fix)

| # | Task | Verification | Est |
|---|------|-------------|-----|
| B-fix.1.1 | Remove `is_mqa`/`is_wide_gqa` gates from `strategy_selector.py`. Use ONLY the threshold-based decision tree for ALL architectures. Keep `n_kv_heads` as a profile stat but do not use it to override path selection. | `python3 -c "from strategy_selector import select_strategy; print(select_strategy({'channel_var_cv': 3.0, 'svd_ratio': 1.0, 'kurtosis': 0.5, 'skewness': 0.1}, 2, n_kv_heads=1))"` → prints `A` (not `C`) | 1h |
| B-fix.1.2 | Re-run selector on all 3 model profiles × 3 bits with pure threshold tree | `strategy_*bit.json` files updated, Gemma now shows diverse paths (not 100% C) | 0.5h |
| B-fix.1.3 | Add regression test: `assert select_strategy(high_cv_stats, 2, n_kv_heads=1) == "A"` — MQA does NOT force Path C | `python3 -m pytest tests/test_strategy_selector.py -k mqa_not_forced` PASS | 0.5h |

**Prevention (Rule 3):** Regression test B-fix.1.3 ensures architecture
gates cannot be re-introduced without test failure.

### B-fix.2: Replace Scoring Function with Real MSE (D2 fix)

| # | Task | Verification | Est |
|---|------|-------------|-----|
| B-fix.2.1 | Rewrite `tune_thresholds.py` scoring: load KV cache from `.npz`, for each threshold combo, assign strategies, compute reconstruction MSE per head, sum total MSE. Lower MSE = better score. | `python3 scripts/tune_thresholds.py --profile data/calibration/profile_gemma_4_e2b.json --calibration data/calibration/fq_v2_gemma_4_e2b.npz --bits 2 --output data/calibration/thresholds_gemma_2bit.json` produces different thresholds than before | 3h |
| B-fix.2.2 | Add 80/20 cross-validation: split calibration chunks into train/test, tune on train, evaluate on test, report both scores. | Output JSON has `train_mse` and `test_mse` fields | 2h |
| B-fix.2.3 | Run tuner for all 9 cells (3 models × 3 bits) | 9 `thresholds_*.json` files with real MSE scores | 1.5h |

**Key change:** Scoring function takes `calibration_npz` as input,
loads real KV cache data, applies each path's quantize→dequantize
roundtrip, measures MSE. No heuristic, no architecture assumption.

### B-fix.3: Replace Synthetic Data with Real KV Cache in Verifier (D3 fix)

| # | Task | Verification | Est |
|---|------|-------------|-----|
| B-fix.3.1 | Rewrite `strategy_verifier.py`: load real KV cache chunks from `.npz` instead of `torch.randn`. Test all 5 paths (including B) per head. | `python3 scripts/strategy_verifier.py --profile ... --strategy ... --calibration ... --output ...` produces ≥1 swap for at least 1 model | 3h |
| B-fix.3.2 | Re-run verifier on all 9 cells with real data | `strategy_*_verified.json` files updated, some with non-zero swaps | 0.5h |

**Key change:** `_apply_path` uses real head data from `.npz[layer_i_k]`
slice instead of `torch.randn(1,1,64,head_dim)*mean_abs`. Path B is
re-enabled.

### B-fix.4: Investigate PPL < FP16 Anomaly (D5 investigation)

| # | Task | Verification | Est |
|---|------|-------------|-----|
| B-fix.4.1 | Write standalone `scripts/eval_fp16_baseline.py` — loads model, evaluates WikiText-2 WITHOUT any patch/unpatch or quantization import | PPL value within 0.5 of 28.13 → anomaly is NOT from patching | 1h |
| B-fix.4.2 | If anomaly persists: test with different cache implementation (`StaticCache`, manual tensor list) to rule out DynamicCache | Document finding in `docs/V26_C1_6_V3_BFIX_FINDINGS.md` §Defect 5 addendum | 1h |
| B-fix.4.3 | If anomaly confirmed as model property (small model regularization): document as known limitation, no fix needed | Add note to findings doc | 0.5h |

### B-fix.5: Re-run C4 Gemma with Fixed Pipeline

| # | Task | Verification | Est |
|---|------|-------------|-----|
| B-fix.5.1 | Re-generate strategies for Gemma (all 3 bits) with fixed selector + tuner + verifier | New `strategy_gemma_4_e2b_*bit.json` files show different distribution than before | 0.5h |
| B-fix.5.2 | Re-run C4 Gemma v3 eval | `data/kv_cache/perplexity_v3_gemma.json` updated with new results | 1.5h GPU |
| B-fix.5.3 | Compare: does v3 now differ from TQ at 2-bit? Does v3 beat v2 at 3-bit? | Honest assessment in commit message | 0.5h |

### B-fix.G: Execute B.G Gate (D4 fix)

| # | Task | Verification | Est |
|---|------|-------------|-----|
| B-fix.G.1 | Smoke test: v3 PPL vs v2-best PPL for Gemma (compare to v2a results) | Document comparison | 0.5h |
| B-fix.G.2 | Verify ≥2 paths used for ≥2 models | `jq '.n_unique_paths' data/calibration/strategy_*_2bit.json` all ≥ 2 | 5 min |
| B-fix.G.3 | Commit `docs/V26_C1_6_V3_DESIGN.md` — honest Gemma C4 results (before and after fix), architecture analysis, decision rationale | File exists and committed | 1h |

### B-fix Gate

All B-fix tasks MUST pass before proceeding to C5/C6:
```
[ ] Architecture gates removed from strategy_selector.py?        (B-fix.1)
[ ] Tuner uses real MSE with cross-validation?                   (B-fix.2)
[ ] Verifier uses real KV cache data?                            (B-fix.3)
[ ] PPL < FP16 investigated and documented?                      (B-fix.4)
[ ] Gemma re-eval shows different results than pre-fix?           (B-fix.5)
[ ] B.G gate executed and design doc committed?                  (B-fix.G)
```
Six YES = proceed to C5. Any NO = block.

---

## Phase C: Validation (3 models × 3 bits)

> **Goal:** Full canonical evaluation — prove v3 adds genuine value.
> **Effort:** 24h (19h + 25% surprise) = ~3 working days
> **Repo:** `~/Documents/fajarquant/`
> **Prerequisite:** Phase B-fix complete (all 6 gates pass)
>
> **NOTE (v1.2):** C4 Gemma was run once pre-fix (results:
> `perplexity_v3_gemma.json`). B-fix.5 re-runs Gemma with fixed pipeline.
> C4 below refers to the POST-fix Gemma result from B-fix.5.

### C0: Pre-flight

| # | Task | Verification | Est |
|---|------|-------------|-----|
| C0.1 | GPU clear, models cached | `nvidia-smi` free ≥ 14 GB | 5 min |
| C0.2 | v2 baseline reproducible | Re-run 1 random cell, compare to JSON | 0.5h |
| C0.3 | B-fix gate confirmed | All 6 checkboxes in B-fix Gate = YES | 5 min |

### C1-C3: Re-calibrate All Models with Fixed Pipeline

| # | Task | Verification | Est |
|---|------|-------------|-----|
| C1 | Gemma: already done in B-fix.5.1 | Verify `strategy_gemma_4_e2b_*bit.json` updated | 5 min |
| C2 | Mistral: re-run selector + tuner + verifier with fixed scripts | New `strategy_mistral_*bit.json` with real-MSE-based thresholds | 1.5h GPU |
| C3 | Qwen2: re-run selector + tuner + verifier with fixed scripts | New `strategy_qwen2_*bit.json` | 1.5h GPU |

### C4-C6: Full Perplexity Eval

| # | Task | Verification | Est |
|---|------|-------------|-----|
| C4 | Gemma v3 eval — already done in B-fix.5.2 | `data/kv_cache/perplexity_v3_gemma.json` is post-fix | 0 min |
| C5 | Mistral v3 eval | `data/kv_cache/perplexity_v3_mistral.json` | 3h GPU |
| C6 | Qwen2 v3 eval | `data/kv_cache/perplexity_v3_qwen2.json` | 3h GPU |

### C7-C9: Analysis + Ablation + Gate

| # | Task | Verification | Est |
|---|------|-------------|-----|
| C7 | Delta analysis: v3 vs v2-best vs KIVI vs TQ per cell | `docs/V26_C1_6_V3_RESULTS.md` with 3-model × 6-method table, includes pre-fix vs post-fix Gemma | 2h |
| C8 | Strategy ablation: per-head assignments + PPL contribution per model | `docs/V26_C1_6_V3_ABLATION.md` — shows which heads use which path | 2h |
| C8.5 | Path contribution analysis: disable each path → measure PPL degradation | Ablation table in results doc | 2h |
| C9 | Go/No-Go gate | `docs/V26_C1_6_V3_GONOGO.md` with "Decision: GO" or "NO-GO" | 1h |

### Go/No-Go Criteria (v1.2 — revised for honesty)

| Model | Bit | Target PPL | Comparison |
|-------|-----|-----------|------------|
| Gemma | 2 | < 39.73 | **Must beat** TQ outlier (not just match) |
| Gemma | 3 | ≤ 22.5 | Match KIVI (21.90) within 3% |
| Gemma | 4 | < 26.51 | Must beat v2a (26.51) — previous best |
| Mistral | 2 | ≤ 25.0 | Match KIVI (23.96) within 5% |
| Mistral | 3 | ≤ 6.5 | Match KIVI (5.99) within 10% |
| Mistral | 4 | ≤ 5.85 | Match KIVI (5.73) within 2% |
| Qwen2 | 2 | ≤ 47.0 | Beat KIVI (46.70) or match |
| Qwen2 | 3 | ≤ 8.1 | Match KIVI (8.01) within 1% |
| **Overall** | | **≥ 7/9 cells won** | v3 best or within 2% of best |

**v1.2 changes from v1.1:**
- Gemma 2-bit: tightened from "match within 2%" to "must beat" (v1.1 target
  was met trivially by equaling TQ)
- Gemma 4-bit: added v2a comparison (26.51) as new bar to clear
- Overall: relaxed from 8/9 to 7/9 — honest after discovering that
  "adaptive" on MQA (1 KV head) has limited headroom

### Fallback if gate fails
If ≥7/9 not achieved on first pass:
1. Re-run B-fix.2 auto-tuning with expanded threshold grid (10 values per dim = 10,000 combos)
2. Re-run B-fix.3 verifier with larger mini-batch (10 chunks instead of 5)
3. Consider per-bit-width thresholds (not shared across 2/3/4-bit)
4. Budget: 2 iteration cycles before declaring final result

---

## Phase D: Fajar Lang Native v3 Builtins

> **Goal:** Native v3 pipeline callable from `.fj` programs.
> **Effort:** 21h (17h + 25% surprise) = ~2.5 working days
> **Repo:** `~/Documents/Fajar Lang/` + `~/Documents/fajarquant/`

| # | Task | Verification | Est |
|---|------|-------------|-----|
| D1 | `fq_v3_profile_head(tensor)` builtin | `cargo test --lib -- fq_v3_profile` | 4h |
| D2 | `fq_v3_select_strategy(profile)` builtin | `cargo test --lib -- fq_v3_select` | 3h |
| D3 | `fq_v3_quantize_head(tensor, strategy, bits)` multi-path dispatch | `cargo test --lib -- fq_v3_quantize` | 5h |
| D4 | Integration test `tests/fajarquant_v3.rs` | `cargo test --test fajarquant_v3` ≥ 10 tests | 3h |
| D5 | Example `examples/fajarquant_v3.fj` | `cargo run -- run examples/fajarquant_v3.fj` | 2h |

---

## Phase E: Paper Update + Release

> **Goal:** Update paper with v3 results, publish.
> **Effort:** 12.5h (10h + 25% surprise) = ~1.5 working days

| # | Task | Verification | Est |
|---|------|-------------|-----|
| E1 | Paper: new §FajarQuant v3 algorithm section | `grep "Per-Head" paper/fajarquant.tex` ≥ 1 | 3h |
| E2 | Paper: replace tab:ppl_crossmodel with v3 data | All numbers from v3 JSON | 2h |
| E3 | Paper: update abstract + conclusion | Reflect v3 results honestly | 1h |
| E4 | `verify_paper_tables.py --strict` with v3 claims | Exit 0 | 2h |
| E5 | README + CHANGELOG + CLAUDE.md | All updated | 1h |
| E6 | Tag `v0.3.0-fajarquant-v3` + GitHub release | `gh release view` | 0.5h |
| E7 | Push all repos + verify | `multi-repo-check.sh` GREEN | 0.5h |

---

## Risk Register (v1.2 — updated with B-fix lessons)

| Risk | P | Impact | Mitigation | Status |
|------|---|--------|------------|--------|
| Per-head dispatch too slow in Python | Med | Med | B7.2 same-strategy batching; pre-compute strategy map | Open |
| Thresholds overfit to calibration set | **High** | **High** | ~~B2.T auto-tuning with 80/20 cross-val~~ → B-fix.2 real MSE + cross-val | **MATERIALIZED** (D2) — B2.T had no real data |
| Residual quant doesn't help at 2-bit | Med | Med | Drop Path D; strengthen Paths A-C-E | Open |
| Asymmetric quant adds complexity without benefit | Med | Low | Path E is optional — selector only routes there when skewness justifies it | Open |
| **Selector degenerates to lookup table** | **High** | **Critical** | ~~v3 degenerates to best-of-5~~ → B-fix.1 removes gates | **MATERIALIZED** (D1) — architecture gates hardcoded |
| All Mistral heads want KIVI | Med | Low | v3 matching KIVI on Mistral is the win condition | Open |
| **Verifier cannot detect wrong assignments** | **High** | **High** | ~~Cap swap rate~~ → B-fix.3 real KV cache data | **MATERIALIZED** (D3) — synthetic data useless |
| **Gate skipped under pressure** | **High** | **Med** | B-fix.G mechanical gate execution | **MATERIALIZED** (D4) |
| **PPL < FP16 baseline anomaly** | **Med** | **Low** | B-fix.4 standalone baseline investigation | **MATERIALIZED** (D5) — pre-existing |
| SVD computation needs new Rust dependency | Low | High | Use power iteration on AᵀA (no new dep) | Open |
| Online profiling mode is inaccurate with few tokens | Med | Med | Minimum 256 warmup tokens | Open |

**B-fix post-mortem:** 5/9 original risks materialized. Root cause: B2.T and
B2.V implementations took shortcuts (heuristic scoring, synthetic data) that
the plan explicitly prohibited. The B.G gate that should have caught this was
skipped. Lesson: plans need **integration tests** for their own gates — a
"did B.G produce a file" check in the pipeline, not just prose.

---

## Timeline Summary (v1.2 — with B-fix)

| Phase | Days | Cumulative | Gate |
|-------|------|-----------|------|
| **A** Fajar Lang enhancements (12 ops) | 5 | Day 5 | `cargo test` +40 tests | DONE |
| **B** Python v3 algorithm (5 paths + verifier + auto-tune) | 6 | Day 11 | Smoke test 1 model | DONE |
| **B-fix** Repair selector + tuner + verifier | 2.5 | Day 13.5 | 6-point gate, Gemma re-eval |
| **C** Validation (3 models × 3 bits) | 3.5 | Day 17 | ≥ 7/9 cells |
| **D** Native builtins | 2.5 | Day 19.5 | E2E `.fj` runs |
| **E** Paper + release | 1.5 | **Day 21** | `verify_paper_tables.py` PASS |

**Total: ~21 working days (~4.5 weeks)** (+2.5 days from v1.1 due to B-fix)

### Effort Breakdown (v1.2)

| Phase | Raw hours | Surprise | Total | Status |
|-------|----------|---------|-------|--------|
| A | 30h | +25% = 38h | 5 days | DONE |
| B | 40h | +30% = 52h | 6 days | DONE (with defects) |
| **B-fix** | **16h** | **+25% = 20h** | **2.5 days** | **NEXT** |
| C | 22h | +25% = 27.5h | 3.5 days | blocked on B-fix |
| D | 17h | +25% = 21h | 2.5 days | |
| E | 10h | +25% = 12.5h | 1.5 days | |
| **Total** | **135h** | **171h** | **21 days** |

---

## Plan Hygiene Self-Check (§6.8) — v1.2 re-audit

```
[x] Pre-flight audit exists per phase (A0/B0/C0/B-fix.0)?      (Rule 1)
[x] Every task has runnable verification command?              (Rule 2)
[x] Prevention mechanisms per phase (B-fix.1.3 regression test)? (Rule 3)
[x] Agent numbers will be cross-checked with Bash?             (Rule 4)
[x] Effort variance tagged in commit messages?                 (Rule 5)
[!] Decisions committed as files — B.G.3 was SKIPPED (D4)     (Rule 6) → FIXED in B-fix.G.3
[x] Public artifacts synced in Phase E?                        (Rule 7)
[x] Multi-repo state check in A0 + E7?                        (Rule 8)
```

**v1.2 note:** Rule 6 failure was the root cause of D4. B-fix.G.3 is the
mechanical fix. Additionally, B-fix.G gate now checks for file existence
before allowing C5/C6.

## Research Integrity Self-Check (§6.9) — v1.2 re-audit

```
[x] Canonical R-α.1 protocol from KIVI + KVQuant?             (R1)
[x] Literature review ≥ 8 papers in B0.2 (14 done)?           (R2)
[x] All baselines with full features (KIVI/TQ outlier)?        (R3)
[!] Calibration once, reuse — BUT tuner/verifier didn't use it (R4) → FIXED in B-fix.2/3
[x] Outlier handling in Path C + Path D?                       (R5)
[x] Validation (Phase C) before paper claims (Phase E)?        (R6)
[x] verify_paper_tables.py --strict gate in E4?                (R7)
```

**v1.2 note:** R4 technically passed (calibration data existed) but the
tuner and verifier failed to USE the calibration data — they substituted
heuristic scoring and synthetic random data. B-fix.2 and B-fix.3 enforce
that calibration `.npz` is a required input.
