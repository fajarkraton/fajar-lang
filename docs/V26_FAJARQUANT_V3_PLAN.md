# FajarQuant v3 "Adaptive Per-Head Method Selection" — Production Plan

> **Version:** 1.1 (enhanced) | **Created:** 2026-04-13 | **Author:** Fajar + Claude Opus 4.6
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

## Phase C: Validation (3 models × 3 bits)

> **Goal:** Full canonical evaluation — prove v3 ≥ 7/9 cells.
> **Effort:** 24h (19h + 25% surprise) = ~3 working days
> **Repo:** `~/Documents/fajarquant/`

### C0: Pre-flight

| # | Task | Verification | Est |
|---|------|-------------|-----|
| C0.1 | GPU clear, models cached | `nvidia-smi` free ≥ 14 GB | 5 min |
| C0.2 | v2 baseline reproducible | Re-run 1 random cell, compare to JSON | 0.5h |

### C1-C3: Calibrate + Profile All Models

| # | Task | Verification | Est |
|---|------|-------------|-----|
| C1 | Gemma calibrate + profile | `data/calibration/fq_v3_gemma*.npz` + `profile_gemma.json` | 2h GPU |
| C2 | Mistral calibrate + profile | Same pattern | 2h GPU |
| C3 | Qwen2 calibrate + profile | Same pattern | 2h GPU |

### C4-C6: Full Perplexity Eval

| # | Task | Verification | Est |
|---|------|-------------|-----|
| C4 | Gemma v3 eval (2/3/4-bit) | `data/kv_cache/perplexity_v3_gemma.json` | 1.5h GPU |
| C5 | Mistral v3 eval | `data/kv_cache/perplexity_v3_mistral.json` | 3h GPU |
| C6 | Qwen2 v3 eval | `data/kv_cache/perplexity_v3_qwen2.json` | 3h GPU |

### C7-C9: Analysis + Ablation + Gate

| # | Task | Verification | Est |
|---|------|-------------|-----|
| C7 | Delta analysis: v3 vs v2 vs KIVI vs TQ per cell | `docs/V26_C1_6_V3_RESULTS.md` with 3-model × 6-method table | 2h |
| C8 | Strategy ablation: for each model, log per-head strategy assignments + PPL contribution | `docs/V26_C1_6_V3_ABLATION.md` — shows which heads use which path | 2h |
| C8.5 | Path contribution analysis: disable each path → measure PPL degradation | Ablation table in results doc | 2h |
| C9 | Go/No-Go gate | `docs/V26_C1_6_V3_GONOGO.md` with "Decision: GO" or "NO-GO" | 1h |

### Go/No-Go Criteria

| Model | Bit | Target PPL | Comparison |
|-------|-----|-----------|------------|
| Gemma | 2 | ≤ 40.0 | Beat TQ outlier (39.73) or match within 2% |
| Gemma | 3 | ≤ 22.5 | Match KIVI (21.90) within 3% |
| Mistral | 2 | ≤ 25.0 | Match KIVI (23.96) within 5% |
| Mistral | 3 | ≤ 6.5 | Match KIVI (5.99) within 10% |
| Mistral | 4 | ≤ 5.85 | Match KIVI (5.73) within 2% |
| Qwen2 | 2 | ≤ 47.0 | Beat KIVI (46.70) or match |
| Qwen2 | 3 | ≤ 8.1 | Match KIVI (8.01) within 1% |
| **Overall** | | **≥ 8/9 cells won** | v3 best or within 2% of best |

### Fallback if gate fails
If ≥8/9 not achieved on first pass:
1. Re-run B2.T auto-tuning with expanded threshold grid
2. Re-run B2.V verifier with larger mini-batch (10 chunks instead of 5)
3. Budget: 2 iteration cycles before declaring final result

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

## Risk Register

| Risk | P | Impact | Mitigation |
|------|---|--------|------------|
| Per-head dispatch too slow in Python | Med | Med | B7.2 same-strategy batching; pre-compute strategy map |
| Thresholds overfit to calibration set | **High** | **High** | B2.T auto-tuning with 80/20 cross-val; B2.V mini-batch verifier |
| Residual quant doesn't help at 2-bit | Med | Med | Drop Path D; strengthen Paths A-C-E |
| Asymmetric quant adds complexity without benefit | Med | Low | Path E is optional — selector only routes there when skewness justifies it |
| Gemma 1-head has no per-head diversity | High | Low | v3 degenerates to best-of-5 paths for single head — still picks optimal |
| All Mistral heads want KIVI | Med | Low | This IS the target — v3 matching KIVI on Mistral is the win condition |
| SVD computation needs new Rust dependency | Low | High | Use power iteration on AᵀA (no new dep); fajarquant adaptive.rs already does eigendecomp |
| Online profiling mode is inaccurate with few tokens | Med | Med | Minimum 256 warmup tokens; flag as "approximate" in output |
| Strategy verifier changes too many assignments | Low | Med | Cap swap rate at 20% of heads; log all swaps for audit |

---

## Timeline Summary

| Phase | Days | Cumulative | Gate |
|-------|------|-----------|------|
| **A** Fajar Lang enhancements (12 ops) | 5 | Day 5 | `cargo test` +40 tests |
| **B** Python v3 algorithm (5 paths + verifier + auto-tune) | 6 | Day 11 | Smoke test 1 model, ≥2 paths used |
| **C** Validation (3 models × 3 bits) | 3.5 | Day 14.5 | ≥ 8/9 cells |
| **D** Native builtins | 2.5 | Day 17 | E2E `.fj` runs |
| **E** Paper + release | 1.5 | **Day 18.5** | `verify_paper_tables.py` PASS |

**Total: ~18.5 working days (~4 weeks)**

### Effort Breakdown

| Phase | Raw hours | Surprise | Total |
|-------|----------|---------|-------|
| A | 30h | +25% = 38h | 5 days |
| B | 40h | +30% = 52h | 6 days |
| C | 22h | +25% = 27.5h | 3.5 days |
| D | 17h | +25% = 21h | 2.5 days |
| E | 10h | +25% = 12.5h | 1.5 days |
| **Total** | **119h** | **151h** | **18.5 days** |

---

## Plan Hygiene Self-Check (§6.8)

```
[x] Pre-flight audit exists per phase (A0/B0/C0)?              (Rule 1)
[x] Every task has runnable verification command?              (Rule 2)
[x] Prevention mechanisms per phase (meta-test, verify script)? (Rule 3)
[x] Agent numbers will be cross-checked with Bash?             (Rule 4)
[x] Effort variance tagged in commit messages?                 (Rule 5)
[x] Decisions committed as files (design, strategy, go/no-go)? (Rule 6)
[x] Public artifacts synced in Phase E?                        (Rule 7)
[x] Multi-repo state check in A0 + E7?                        (Rule 8)
```

## Research Integrity Self-Check (§6.9)

```
[x] Canonical R-α.1 protocol from KIVI + KVQuant?             (R1)
[x] Literature review ≥ 8 papers in B0.2?                     (R2)
[x] All baselines with full features (KIVI/TQ outlier)?        (R3)
[x] Calibration once, reuse (not per-chunk)?                   (R4)
[x] Outlier handling in Path C + Path D?                       (R5)
[x] Validation (Phase C) before paper claims (Phase E)?        (R6)
[x] verify_paper_tables.py --strict gate in E4?                (R7)
```
