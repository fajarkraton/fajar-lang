# V26 Phase C1.6 — Path B Master Plan (FajarQuant v2.12)

> **Created:** 2026-04-12
> **Phase:** C1.6 Path B (test-first, fix, language support, paper rewrite)
> **Decision provenance:** docs/V26_C1_6_METHODOLOGY.md (option (b) → R-α.1 → Path B)
> **Surprise budget:** +30% (Phase C standard, applies per sub-task)
> **Rule compliance:** CLAUDE.md §6.8 Plan Hygiene Rules 1-8 + §6.9 Research Integrity Rules 1-7
> **Supersedes:** docs/V26_PRODUCTION_PLAN.md rows C1.6.0-C1.6.5 (the prefix+target → R-α.1 → naive smoke pipeline)

---

## Why this plan exists

The original C1.6 sequence (PPL eval on Mistral + Qwen2-7B with the existing
script) was built on three implicit assumptions that turned out to be wrong:

1. **Assumption:** the existing `eval_perplexity.py` script was a faithful
   implementation of canonical PPL evaluation. **Reality:** it used a custom
   prefix+target post-hoc cache mutation protocol that diverges from
   KIVI/KVQuant/SKVQ. The resulting numbers were not comparable to literature.

2. **Assumption:** FajarQuant's PCA-based approach beats TurboQuant on PPL
   because the existing JSON shows it does. **Reality:** with the canonical
   protocol (R-α.1 model surgery, commit `c9b2ff5` → `3015545`), FajarQuant
   loses to TurboQuant by 5.6× on Gemma 4 E2B 2-bit smoke test (FQ +77.34
   vs TQ +13.71 PPL above FP16). The win was an artifact of the broken
   protocol.

3. **Assumption:** TurboQuant in the comparison is the published baseline.
   **Reality:** my port is a "naive TurboQuant" — random orthogonal rotation
   + Lloyd-Max only, **without** the published TurboQuant's top-15% bf16
   outlier preservation. Even the head-to-head comparison is unfair.

Path B exists to systematically **correct all three assumptions** with a
data-driven research workflow, then build FajarQuant v2 on the resulting
truth. The "FajarQuant v2.12" name (per user request) marks the version
break: v2.12 is the first version that defensibly beats published TurboQuant
across multiple architectures under canonical protocol.

The literature sweep that informed this plan covered KIVI (ICML 2024),
KVQuant (NeurIPS 2024), QuaRot (2024), SpinQuant (ICLR 2025), FlatQuant
(ICML 2025), RotateKV (IJCAI 2025), OTT (ACL 2025), AsymKV (COLING 2025),
PolarQuant (AISTATS 2026), TurboQuant (ICLR 2026), KVTC (ICLR 2026),
KVLinC (Oct 2025), VecInfer (Oct 2025). The dominant 2025-2026 patterns
are: (a) calibrated rotations beat per-chunk; (b) Hadamard/learned beat
PCA on outlier-heavy data; (c) outlier extraction is mandatory; (d) DP
bit allocation is the new state of the art for compression-quality
trade-off.

---

## Phases and gates

| Phase | Goal | Effort | Gate |
|---|---|---|---|
| **B1** | Baseline data collection (v1 numbers via canonical protocol, 3 models × 3 bits × 4 methods incl. fair TurboQuant) | 6-10 h | B1.0 pre-flight audit, B1.G results signed off |
| **B2** | Diagnosis + v2 design lock-in | 3-5 h | B2.D `V26_C1_6_V2_DESIGN.md` decision file (Rule 6) |
| **B3** | FajarQuant v2.12 implementation | 2-5 days | B3.S smoke test pass + B3.D ablation deltas committed |
| **B4** | v2.12 validation against canonical protocol on 3 models | 6-10 h | B4.G go/no-go file: does v2 beat TurboQuant by ≥X% on ≥2/3 models |
| **B5** | Fajar Lang language support (L1-L7) | 5-10 days, parallel with B3-B6 | B5.G self-host bench: v2 in Fajar Lang faster than Python ref |
| **B6** | Paper rewrite (cross-pollination narrative) | 2-3 days | B6.A pre-publication audit |
| **B7** | Final validation + multi-repo sync + release | 1-2 days | B7.R release tag + multi-repo state check (Rule 8) |

**Total effort:** 3-5 weeks elapsed (effort spread over 3 repos: fajar-lang, fajarquant, fajaros-x86).

---

## B1: Baseline data collection (P0 blocker)

> **Goal:** Get the truth about FajarQuant v1 + naive-TQ + outlier-TQ + KIVI
> on 3 models × 3 bit widths under canonical R-α.1 protocol. This is the
> "v1 baseline" the v2 paper section will be measured against.

### B1.0: Pre-flight audit (Rule 1 mandatory)

| # | Task | Verification | Est. |
|---|------|-------------|------|
| B1.0.1 | GPU state snapshot | `nvidia-smi --query-gpu=memory.free,utilization.gpu --format=csv` ≥ 14000 MiB free, no Buku-style competing processes | 5 min |
| B1.0.2 | Disk space snapshot (KV cache + model cache) | `df -h ~/.cache/huggingface ~/Documents/fajarquant/data` ≥ 100 GB free | 5 min |
| B1.0.3 | All 3 models cached | `du -sh ~/.cache/huggingface/hub/models--google--gemma-4-E2B ~/.cache/huggingface/hub/models--mistralai--Mistral-7B-v0.1 ~/.cache/huggingface/hub/models--Qwen--Qwen2-7B` returns 3 directories with size > 10 GB each | 10 min |
| B1.0.4 | Patched script integrity | `python3 scripts/quant_attention.py` self-test PASS + `python3 -c "from quant_attention import patch_model_for_quantization, unpatch_model"` | 2 min |
| B1.0.5 | Findings file committed | `cd ~/Documents/Fajar Lang && test -f docs/V26_C1_6_B1_FINDINGS.md && git log --oneline -1 docs/V26_C1_6_B1_FINDINGS.md` | 30 min |

**Gate:** B1.0.5 committed, all checks GREEN. B1.1+ blocked until then.

### B1.1: TurboQuant outlier-aware port (fairness fix)

| # | Task | Verification | Est. |
|---|------|-------------|------|
| B1.1.1 | Add `apply_turboquant_4d_with_outliers(data, bits, outlier_pct=0.15)` to `quant_attention.py`: identify top-`outlier_pct` channels by per-coord variance, store in fp16, quantize the rest. Per the published TurboQuant ICLR 2026 method | `cd ~/Documents/fajarquant && python3 -c "from quant_attention import apply_turboquant_4d_with_outliers; import torch; x=torch.randn(1,4,512,128); y=apply_turboquant_4d_with_outliers(x, 2); assert y.shape == x.shape and torch.isfinite(y).all()"` | 1 h |
| B1.1.2 | Wire `turboquant_outlier` as 5th dispatch method in `_quantize_kv` | `grep "turboquant_outlier" scripts/quant_attention.py` ≥ 1 occurrence | 0.2 h |
| B1.1.3 | Wire `turboquant_outlier` as 5th method in `eval_perplexity.py main()` loop | `python3 scripts/eval_perplexity.py --help` shows method enumeration includes the new one OR runs without error in the loop | 0.2 h |
| B1.1.4 | Smoke test: 5 samples Gemma 2-bit, all 5 methods, assert finite + correct ordering (KIVI worst, TQ outlier ≤ TQ naive) | `python3 scripts/eval_perplexity.py --model google/gemma-4-E2B --bits 2 --max-samples 5 --seq-len 512 --output /tmp/b1_1_smoke.json && jq '.turboquant_outlier_2bit.ppl, .turboquant_2bit.ppl' /tmp/b1_1_smoke.json` returns 2 numbers, first ≤ second | 5 min GPU |

### B1.2: Full Gemma 4 E2B canonical run

| # | Task | Verification | Est. |
|---|------|-------------|------|
| B1.2.1 | Run at `--seq-len 2048 --max-samples 30 --bits 2,3,4` (5 methods × 3 bits + FP16 baseline = 16 cells) | `cd ~/Documents/fajarquant && python3 scripts/eval_perplexity.py --model google/gemma-4-E2B --seq-len 2048 --max-samples 30 --bits 2,3,4 --output data/kv_cache/perplexity_v1_baseline_gemma.json && jq '.fp16.tokens' data/kv_cache/perplexity_v1_baseline_gemma.json` ≥ 50000 | ~30 min GPU |
| B1.2.2 | Sanity check FP16 PPL is reasonable (Gemma 4 E2B WikiText-2 expected ~7-15 with 2048 ctx) | `jq '.fp16.ppl' data/kv_cache/perplexity_v1_baseline_gemma.json` returns value in [5, 25] | 1 min |

### B1.3: Full Mistral 7B canonical run

| # | Task | Verification | Est. |
|---|------|-------------|------|
| B1.3.1 | Run at same args | `python3 scripts/eval_perplexity.py --model mistralai/Mistral-7B-v0.1 --seq-len 2048 --max-samples 30 --bits 2,3,4 --output data/kv_cache/perplexity_v1_baseline_mistral.json` produces 16 cells | ~1.5 h GPU |
| B1.3.2 | Sanity check Mistral 7B FP16 PPL (literature ~5-6 on WikiText-2) | `jq '.fp16.ppl' data/kv_cache/perplexity_v1_baseline_mistral.json` returns value in [4, 12] | 1 min |

### B1.4: Full Qwen2-7B canonical run

| # | Task | Verification | Est. |
|---|------|-------------|------|
| B1.4.1 | Run at same args | `python3 scripts/eval_perplexity.py --model Qwen/Qwen2-7B --seq-len 2048 --max-samples 30 --bits 2,3,4 --output data/kv_cache/perplexity_v1_baseline_qwen2_7b.json` produces 16 cells | ~1.5 h GPU |
| B1.4.2 | Sanity check Qwen2-7B FP16 PPL | `jq '.fp16.ppl' data/kv_cache/perplexity_v1_baseline_qwen2_7b.json` returns value in [4, 12] | 1 min |

### B1.5: Llama 2 7B (gated, optional)

| # | Task | Verification | Est. |
|---|------|-------------|------|
| B1.5.1 | If C1.0.0 Meta access has landed: full Llama 2 7B run | `huggingface-cli download meta-llama/Llama-2-7b-hf --include "config.json" --local-dir /tmp/l2 && rm -rf /tmp/l2` exit 0 → run; else skip with note | ~1.5 h GPU OR skip |

### B1.G: Baseline results sign-off gate

| # | Task | Verification | Est. |
|---|------|-------------|------|
| B1.G.1 | Aggregate v1 baseline into `docs/V26_C1_6_BASELINE_RESULTS.md` (all 3-4 model files merged into one table, pretty-printed, with notes per cell) | `cd ~/Documents/fajarquant && test -f docs/V26_C1_6_BASELINE_RESULTS.md && grep -c '\|' docs/V26_C1_6_BASELINE_RESULTS.md` ≥ 60 (table rows) | 0.5 h |
| B1.G.2 | Multi-repo state check (Rule 8) | `cd ~/Documents/Fajar\ Lang && git status -sb && cd ~/Documents/fajarquant && git status -sb && cd ~/Documents/fajaros-x86 && git status -sb` | 5 min |
| B1.G.3 | Phase B1 commit + push fajarquant | `cd ~/Documents/fajarquant && git log --oneline --grep "v26-c1.6-b1" \| wc -l` ≥ 1 | 10 min |

**Gate:** B1.G.1 + B1.G.2 + B1.G.3 all done. v1 baseline is canonical-protocol truth, signed off.

**B1 effort total:** 1h coding + 0.5h coding + ~5h GPU + 1h docs + multi-repo sync = **~7-8h**

---

## B2: Diagnosis + v2 design lock-in (Rule 6 mechanical gate)

> **Goal:** Use the v1 baseline data to identify which root cause(s) dominate
> per-model, then commit to a specific v2 design via a mechanical decision file.

### B2.1: Per-model failure analysis

| # | Task | Verification | Est. |
|---|------|-------------|------|
| B2.1.1 | Compute Δ(FQ - TQ_outlier) per model per bit width — where does FQ lose, by how much | Script that reads 3 baseline JSONs and produces a matrix `(model × bit) → Δ_PPL`, written to `docs/V26_C1_6_DIAGNOSIS.md` | 1 h |
| B2.1.2 | Compute correlation between FQ loss magnitude and (a) head_dim, (b) num_kv_heads, (c) eigenvalue concentration in FP16 K (offline analysis on a sampled chunk per model) | `docs/V26_C1_6_DIAGNOSIS.md` has §"Failure Pattern" with numerical evidence | 1 h |
| B2.1.3 | Hypothesis ranking: which root cause (RC1 outlier alignment / RC2 per-chunk noise / RC3 RoPE ordering / RC4 K-V independent / RC5 no outlier extraction) explains the largest variance in observed deltas | `docs/V26_C1_6_DIAGNOSIS.md` has §"Root Cause Ranking" with weights | 0.5 h |

### B2.2: v2 design candidate evaluation

| # | Task | Verification | Est. |
|---|------|-------------|------|
| B2.2.1 | Re-evaluate F2-A through F2-F design options against the per-model failure pattern (= which option targets the dominant root cause for our 3 models) | `docs/V26_C1_6_DIAGNOSIS.md` has §"Design Option Scoring" with 4 columns: option, target RC, est. effort, expected PPL gain | 0.5 h |
| B2.2.2 | Pick the design (default: F2-D PCA + outlier extraction; deviate only if data clearly says otherwise) | `docs/V26_C1_6_DIAGNOSIS.md` has §"Decision" with chosen option label + rationale | 0.5 h |

### B2.D: Mechanical decision gate

| # | Task | Verification | Est. |
|---|------|-------------|------|
| B2.D.1 | Write `docs/V26_C1_6_V2_DESIGN.md` (mirrors V26_C1_6_METHODOLOGY.md format): chosen option, full algorithm spec, effort breakdown, ablation roadmap, fallback plan | `cd ~/Documents/fajarquant && test -f docs/V26_C1_6_V2_DESIGN.md && grep -q "Decision: F2-" docs/V26_C1_6_V2_DESIGN.md` | 1 h |
| B2.D.2 | Commit decision file BEFORE any v2 code | `cd ~/Documents/fajarquant && git log --oneline --grep "v26-c1.6-b2.d" \| wc -l` ≥ 1 | 5 min |

**Gate:** B2.D.2 commit lands. B3 blocked until then. CLAUDE.md §6.8 Rule 6 enforced.

**B2 effort total:** ~3-4 h.

---

## B3: FajarQuant v2.12 implementation

> **Goal:** Implement the chosen v2 design from B2.D in `quant_attention_v2.py`
> (separate file, leaves v1 intact for ablation comparison) and a parallel
> `eval_perplexity_v2.py` driver.

### B3.1: Module skeleton

| # | Task | Verification | Est. |
|---|------|-------------|------|
| B3.1.1 | Create `scripts/quant_attention_v2.py` with stub for chosen design (e.g. F2-D: `apply_fajarquant_v2_4d` + outlier extraction + per-layer calibrated PCA loader) | `python3 scripts/quant_attention_v2.py` self-test PASS | 1-2 h |
| B3.1.2 | Reuse the 3 per-architecture forwards from `quant_attention.py` via import (same monkey-patch infrastructure, just different `_QUANT_METHOD` dispatch) | `grep "from quant_attention import" scripts/quant_attention_v2.py` shows reused machinery | 0.2 h |

### B3.2: Calibration script (if v2 needs it)

| # | Task | Verification | Est. |
|---|------|-------------|------|
| B3.2.1 | Write `scripts/calibrate_fajarquant_v2.py`: load model, run 128 WikiText samples through, extract per-layer K/V tensors, compute per-layer PCA (or learned rotation per chosen design), save to `data/calibration/fq_v2_<model>.npz` | `python3 scripts/calibrate_fajarquant_v2.py --model google/gemma-4-E2B --output data/calibration/fq_v2_gemma.npz && ls -la data/calibration/fq_v2_gemma.npz` exists, ≥ 1 MB | 2-4 h |
| B3.2.2 | Calibration loader integrated into v2 quantization functions | `python3 -c "from quant_attention_v2 import load_calibration; load_calibration('data/calibration/fq_v2_gemma.npz')"` succeeds | 0.5 h |

### B3.3: v2 quantization core (depends on chosen design)

| # | Task | Verification | Est. |
|---|------|-------------|------|
| B3.3.1 | Implement v2 algorithm per `V26_C1_6_V2_DESIGN.md` spec | unit tests in `tests/test_quant_attention_v2.py` (relerr bounds, shape checks, dispatch parity) | 1-3 days |
| B3.3.2 | If F2-A: calibrated PCA loader + apply | covered by B3.2 |
| B3.3.3 | If F2-D: outlier extraction (top 1% channels by variance, fp16 storage) + PCA on rest | covered by B3.3.1 |
| B3.3.4 | If F2-F: A + D + E combined (calibrated PCA + outlier extraction + DP bit allocation) | covered by B3.3.1, plus DP allocator |

### B3.S: v2 smoke test (echoes B3.G of v1)

| # | Task | Verification | Est. |
|---|------|-------------|------|
| B3.S.1 | Smoke test on Gemma 4 E2B 10 samples 512-ctx 2-bit (mirrors C1.6.1.5-v2) | `python3 scripts/eval_perplexity_v2.py --model google/gemma-4-E2B --bits 2 --max-samples 10 --seq-len 512 --output /tmp/b3_smoke.json` produces 16 cells, FQ_v2_2bit PPL < FQ_v1_2bit PPL from B1 baseline (improvement) | 5 min GPU |
| B3.S.2 | If smoke test FAILS to improve over v1: pause, write `docs/V26_C1_6_B3_FAILURE_ANALYSIS.md`, escalate to user | failure file exists OR smoke pass condition met | n/a |

**Gate B3.S:** v2 smoke better than v1 smoke at same args. If not, do not proceed to B4 — go back to B2 with new diagnosis.

**B3 effort total:** 2-5 days depending on F2-A vs F2-D vs F2-F choice.

---

## B4: v2.12 validation (3 models × 3 bits)

### B4.1: Full Gemma re-run

| # | Task | Verification | Est. |
|---|------|-------------|------|
| B4.1.1 | Run v2 at canonical args | `python3 scripts/eval_perplexity_v2.py --model google/gemma-4-E2B --seq-len 2048 --max-samples 30 --bits 2,3,4 --output data/kv_cache/perplexity_v2_gemma.json` produces 16 cells | ~30 min GPU |

### B4.2: Full Mistral re-run

| # | Task | Verification | Est. |
|---|------|-------------|------|
| B4.2.1 | Run v2 | `python3 scripts/eval_perplexity_v2.py --model mistralai/Mistral-7B-v0.1 ...` | ~1.5 h GPU |

### B4.3: Full Qwen2-7B re-run

| # | Task | Verification | Est. |
|---|------|-------------|------|
| B4.3.1 | Run v2 | `python3 scripts/eval_perplexity_v2.py --model Qwen/Qwen2-7B ...` | ~1.5 h GPU |

### B4.4: v1 vs v2 delta analysis

| # | Task | Verification | Est. |
|---|------|-------------|------|
| B4.4.1 | Build per-model delta table (PPL_v1 - PPL_v2 per bit per method) | `docs/V26_C1_6_V2_RESULTS.md` table | 0.5 h |
| B4.4.2 | Compute per-model "v2 wins" matrix (does v2 beat TQ_outlier? by how much?) | `docs/V26_C1_6_V2_RESULTS.md` §"v2 vs TurboQuant Outlier" matrix | 0.5 h |

### B4.G: Mechanical go/no-go gate

| # | Task | Verification | Est. |
|---|------|-------------|------|
| B4.G.1 | Write `docs/V26_C1_6_V2_GONOGO.md`: does v2.12 beat TQ_outlier by ≥10% on at least 2 of 3 models at 2-bit? | file exists with explicit "Decision: GO" or "Decision: NO-GO" + rationale | 0.5 h |
| B4.G.2 | If NO-GO: pause Phase B6 (paper rewrite), back to B2 with new diagnosis | n/a |
| B4.G.3 | If GO: commit go decision file, proceed to B5+B6 | `git log --oneline --grep "v26-c1.6-b4.g"` ≥ 1 | 5 min |

**B4 effort total:** ~6-10 h (mostly GPU).

---

## B5: Fajar Lang language support (parallel with B3-B6, optional)

> **Goal:** Make FajarQuant v2 implementable cleanly in Fajar Lang itself
> (`fajaros-x86/kernel/compute/fajarquant_v2.fj`) so the embedded story is
> defensible. Each L# task is an independent language-level addition.

### B5.L1: Native quantized tensor types

| # | Task | Verification | Est. |
|---|------|-------------|------|
| B5.L1.1 | Add `Quantized<T, BITS>` struct to stdlib in Fajar Lang | `cargo run -- run examples/quantized_tensor.fj` succeeds | 1 day |
| B5.L1.2 | Compiler enforces dequant before float math (analyzer rule SE017 QuantizedNotDequantized) | analyzer test in `tests/integration/quant_type_safety.rs` | 0.5 day |
| B5.L1.3 | New error code `SE017` documented in `docs/ERROR_CODES.md` | `grep SE017 docs/ERROR_CODES.md` ≥ 1 | 0.2 day |

### B5.L2: Hadamard transform builtin

| # | Task | Verification | Est. |
|---|------|-------------|------|
| B5.L2.1 | Add `nn::hadamard(x)` builtin with compile-time D=power-of-two check | `examples/hadamard_demo.fj` runs | 1 day |
| B5.L2.2 | Cranelift/LLVM backend codegen (AVX2 butterfly via inline asm) | benchmark `bench/hadamard_simd.rs` shows ≥ 2x speedup vs scalar | 1 day |

### B5.L3: Compile-time PCA matrices as const tensors

| # | Task | Verification | Est. |
|---|------|-------------|------|
| B5.L3.1 | `include_calibration!("path/to/file.bin")` macro/builtin that loads at compile-time and stores as `const ROTATION: Tensor<f32, D, D>` | `examples/calibrated_rotation.fj` runs | 1 day |
| B5.L3.2 | Compiler verifies orthogonality at build time (call `verify_orthogonal!()` macro) | `tests/integration/calibrated_rotation_orthogonal.rs` | 0.5 day |

### B5.L4: @device fn quantization kernels (existing feature, just write code)

| # | Task | Verification | Est. |
|---|------|-------------|------|
| B5.L4.1 | Implement v2 quantization in Fajar Lang under `@device` context | `cargo run -- check fajaros-x86/kernel/compute/fajarquant_v2.fj` zero errors | 1-2 days |

### B5.L5: AVX2/AES-NI inline asm for hot paths (existing FFI v2)

| # | Task | Verification | Est. |
|---|------|-------------|------|
| B5.L5.1 | Inline asm Hadamard butterfly + outlier scan | `bench/fajarquant_v2_native.rs` ≥ 1.5x speedup vs naive Fajar Lang impl | 1 day |

### B5.L6: Compile-time shape verification for quantized matmul

| # | Task | Verification | Est. |
|---|------|-------------|------|
| B5.L6.1 | Generic `matmul_quantized<M, N, K>` with where clause on shape | `tests/integration/quant_matmul_shape.rs` | 1 day |

### B5.L7: Stack-allocated `QuantizedKVCache` with const max length

| # | Task | Verification | Est. |
|---|------|-------------|------|
| B5.L7.1 | Add `QuantizedKVCache<MAX_LEN, N_LAYERS, HEAD_DIM>` struct | `examples/stack_kv_cache.fj` runs without heap | 1.5 days |
| B5.L7.2 | RAII for KV cache lifetimes (automatic cleanup) | analyzer test for double-free prevention | 0.5 day |

### B5.G: Self-host benchmark gate

| # | Task | Verification | Est. |
|---|------|-------------|------|
| B5.G.1 | FajarQuant v2 implemented in Fajar Lang (`fajaros-x86/kernel/compute/fajarquant_v2.fj`) | `cargo run -- run fajaros-x86/kernel/compute/fajarquant_v2.fj` produces same output as Python ref | 1 day |
| B5.G.2 | Native Fajar Lang impl ≥ 1x speed of Python ref (preferably 2-5x) | `bench/fajarquant_v2_native_vs_python.sh` | 0.5 day |

**B5 effort total:** 5-10 days (parallel with B3-B6, optional but high value for embedded story).

---

## B6: Paper rewrite

> **Goal:** Update `paper/fajarquant.tex` to reflect the v1 → v2 narrative
> with cross-pollination from KVTC/SpinQuant/Hadamard literature.

### B6.1: Methodology section rewrite

| # | Task | Verification | Est. |
|---|------|-------------|------|
| B6.1.1 | Add §"Methodology" subsection on canonical PPL evaluation protocol (KVQuant/SKVQ-style non-overlapping chunks, model surgery, reference KVQuant + KIVI as protocol sources) | `grep -c "non-overlapping" paper/fajarquant.tex` ≥ 1 | 0.5 day |
| B6.1.2 | Acknowledge v1 → v2 transition (this is the key honesty section) | `grep "v1 → v2" paper/fajarquant.tex` ≥ 1 | 0.5 day |

### B6.2: Cross-Model PPL table rewrite

| # | Task | Verification | Est. |
|---|------|-------------|------|
| B6.2.1 | Replace existing `tab:ppl` with new `tab:ppl_v1_v2_crossmodel` showing 3 models × 3 bits × 5 methods (FP16, FQ_v1, FQ_v2, KIVI, TurboQuant_outlier) | grep table caption ≥ 1 | 0.5 day |
| B6.2.2 | Update abstract line 40 with v2 numbers, retire old "FQ wins 2-bit" claim, replace with verified v2 claim | `grep "PPL" paper/fajarquant.tex \| head -5` shows new numbers | 0.2 day |
| B6.2.3 | Update `tab:e2e` PPL rows with v2 numbers | `grep "PPL.*WikiText" paper/fajarquant.tex` shows new numbers | 0.2 day |

### B6.3: Related Work expansion

| # | Task | Verification | Est. |
|---|------|-------------|------|
| B6.3.1 | Add §"Related Work" entries for KVTC, SpinQuant, FlatQuant, RotateKV, KVLinC, OTT, AsymKV — clarify what FajarQuant v2 borrows and what's novel | `grep -c "\\\\cite{kvtc\\|spinquant\\|flatquant\\|rotatekv\\|kvlinc\\|ott\\|asymkv}" paper/fajarquant.tex` ≥ 7 | 1 day |

### B6.4: New §"FajarQuant v2: Outlier-Aware Calibrated PCA" (or whatever fix)

| # | Task | Verification | Est. |
|---|------|-------------|------|
| B6.4.1 | Write §FajarQuant v2 section: algorithm, mechanism, ablation table v1 vs v2 vs each component | `grep "FajarQuant v2" paper/fajarquant.tex` ≥ 1 + new section header | 0.5 day |

### B6.5: §"Embedded Deployment" (Fajar Lang language story)

| # | Task | Verification | Est. |
|---|------|-------------|------|
| B6.5.1 | Add §Embedded Deployment subsection — Fajar Lang implementation, bare-metal kernel benchmark, AVX2/AES-NI speedup | depends on B5.G done | 0.5 day |

### B6.A: Pre-publication audit (Rule 9 mandatory)

| # | Task | Verification | Est. |
|---|------|-------------|------|
| B6.A.1 | Every numerical claim in paper backed by `verify_paper_tables.py` claim entry | `python3 scripts/verify_paper_tables.py --strict` exit 0 | 0.5 day |
| B6.A.2 | Every section claim has citation to source | `bib_audit.sh` script that flags unbacked claims | 0.5 day |
| B6.A.3 | Reproducibility appendix updated: hardware snapshot, package versions, exact seeds | `paper/REPRODUCIBILITY.md` exists | 0.5 day |

**B6 effort total:** ~3-4 days.

---

## B7: Final validation + release

### B7.1: Multi-repo state check (Rule 8)

| # | Task | Verification | Est. |
|---|------|-------------|------|
| B7.1.1 | All 3 repos (fajar-lang, fajarquant, fajaros-x86) have zero unpushed commits | `bash scripts/multi-repo-check.sh` GREEN | 5 min |
| B7.1.2 | All 3 repos have zero uncommitted changes | (same script) | included |

### B7.2: Reproducibility smoke

| # | Task | Verification | Est. |
|---|------|-------------|------|
| B7.2.1 | `cd ~/Documents/fajarquant && bash reproduce.sh` runs end-to-end on a fresh clone | CI smoke test in `.github/workflows/reproducibility.yml` | 0.5 day |

### B7.3: Release artifacts

| # | Task | Verification | Est. |
|---|------|-------------|------|
| B7.3.1 | Tag fajarquant `v0.2.0-fajarquant-v2.12` | `gh release view v0.2.0-fajarquant-v2.12` shows release | 0.5 h |
| B7.3.2 | Update fajarquant README with v2 numbers + KVTC/SpinQuant comparison | `grep "v2.12" README.md` ≥ 1 | 0.5 h |
| B7.3.3 | Update Fajar Lang CLAUDE.md §3 Current Status to reflect FajarQuant v2 work complete | edit | 0.2 h |
| B7.3.4 | Update root `CHANGELOG.md` with V27 entry (or V26.2 if minor) | edit | 0.5 h |

### B7.4: Public-artifact sync (Rule 7)

| # | Task | Verification | Est. |
|---|------|-------------|------|
| B7.4.1 | Update fajaros-x86 README badge if there's a FajarQuant version reference | edit | 0.2 h |
| B7.4.2 | Update fajar-lang root README with FajarQuant v2 link | edit | 0.2 h |
| B7.4.3 | GitHub release notes for all 3 repos | `gh release view` for each repo | 0.5 h |

**B7 effort total:** ~1-2 days.

---

## Surprise budget tracking

Phase C standard +30%. Apply per sub-task. Tag every commit with `[actual Xh, est Yh, ±Z%]`.

| Phase | Conservative est | +30% budget | Expected actual |
|---|---|---|---|
| B1 | 7-8 h | 9-10 h | TBD |
| B2 | 3-4 h | 4-5 h | TBD |
| B3 | 2-5 days | 2.5-6.5 days | TBD |
| B4 | 6-10 h | 8-13 h | TBD |
| B5 | 5-10 days | 6.5-13 days | TBD |
| B6 | 3-4 days | 4-5 days | TBD |
| B7 | 1-2 days | 1.3-2.6 days | TBD |
| **Total** | **3-5 weeks** | **4-6.5 weeks** | TBD |

If average variance exceeds +30%, escalate next sub-phase to +40% budget (Plan Hygiene Rule 5).

---

## Decision gate summary

| Gate | When | What | Blocks |
|---|---|---|---|
| B1.0.5 | Pre-B1.1 | Pre-flight audit findings file committed | All B1.1+ |
| B1.G.1+G.2+G.3 | Post-B1 | Baseline results signed off, multi-repo synced | B2 |
| B2.D.2 | Post-B2 | `V26_C1_6_V2_DESIGN.md` committed (Rule 6) | B3 |
| B3.S.1 | Post-B3.S | v2 smoke beats v1 smoke | B4 |
| B4.G.3 | Post-B4 | `V26_C1_6_V2_GONOGO.md` says GO | B5, B6 |
| B5.G.2 | Optional post-B5 | Native Fajar Lang impl matches Python ref speed | B6.5 only |
| B6.A.3 | Post-B6 | Pre-publication audit GREEN | B7 |
| B7.1.1 | Post-B7 | Multi-repo state check GREEN (Rule 8) | Release |

---

## Self-check before any plan/audit commit (Plan Hygiene Rule 8 self-checklist)

```
[x] Pre-flight audit (B1.0) exists for the Phase?               (Rule 1)
[x] Every task has a runnable verification command?             (Rule 2)
[x] At least one prevention mechanism added (hook/CI/rule)?     (Rule 3)
    → §6.9 Research Integrity Rules added to CLAUDE.md
    → memory/feedback_research_integrity.md created
    → ablation table requirement in B6 acts as prevention
[x] Agent-produced numbers cross-checked with Bash?             (Rule 4)
    → Literature search results verified via WebFetch on each paper
[x] Effort variance tagged in commit message?                   (Rule 5)
    → Surprise budget tracked per phase, +30% Phase C standard
[x] Decisions are committed files, not prose paragraphs?        (Rule 6)
    → B2.D.2 V2_DESIGN.md gate, B4.G.3 GONOGO file gate
[x] Internal doc fixes audited for public-artifact drift?       (Rule 7)
    → B7.4.x explicit public-artifact sync sub-tasks
[x] Multi-repo state check run before starting work?            (Rule 8)
    → B1.0.1, B1.G.2, B7.1.1 all run multi-repo-check.sh
```

Eight YES = ship.

---

## Plan version history

- **2026-04-12 v1.0** — Created as comprehensive Path B plan after C1.6 R-α.1
  smoke test revealed FajarQuant v1 loses to TurboQuant on canonical PPL.
  Replaces docs/V26_PRODUCTION_PLAN.md C1.6.0-C1.6.5 row sequence with the
  test → diagnose → fix → validate → publish workflow informed by
  literature sweep of 13 KV-quant papers (2024-2026).
