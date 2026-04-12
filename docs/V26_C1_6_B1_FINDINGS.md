# V26 C1.6 Path B — Phase B1.0 Pre-flight Audit Findings

> **Date:** 2026-04-12
> **Phase:** Path B → B1.0 (pre-flight audit, mandatory before B1.1)
> **Plan reference:** `docs/V26_C1_6_PATH_B_PLAN.md` §B1.0
> **Rule compliance:** CLAUDE.md §6.8 Plan Hygiene Rule 1 (pre-flight audit mandatory)

## Audit results — all GREEN ✅

### B1.0.1 GPU state
```
Card:       NVIDIA GeForce RTX 4090 Laptop GPU
Free:       15933 MiB / 16376 MiB total
Util:       0%
Compute apps: none
```
**Status:** ✅ GREEN. Sufficient memory for 7B model FP16 (~14 GB) + KV cache + activations at seq_len=2048. No competing processes (Buku reindex_qdrant.py from earlier session has terminated).

### B1.0.2 Disk space
```
/dev/nvme0n1p2  937G  652G  238G  74%  /
```
**Status:** ✅ GREEN. 238 GB free is comfortable headroom for new perplexity_v1_baseline_*.json files (each <1 MB) + future v2 calibration data + paper artifacts.

### B1.0.3 Model cache
```
9.6 GB   ~/.cache/huggingface/hub/models--google--gemma-4-E2B
14  GB   ~/.cache/huggingface/hub/models--mistralai--Mistral-7B-v0.1
15  GB   ~/.cache/huggingface/hub/models--Qwen--Qwen2-7B
```
**Status:** ✅ GREEN. All three target models cached locally. Zero download cost for B1.2 / B1.3 / B1.4 GPU runs. Llama 2 7B (B1.5, optional) is still gated on Meta HF access — deferred per existing C1.0.0 pending status.

### B1.0.4 Patched script integrity
```
[quant_attention] self-test starting...
[quant_attention] self-test PASS
  FP32→FQ-2bit relerr: 0.4528
  FP32→FQ-4bit relerr: 0.0915

quant_attention imports OK
eval_perplexity imports OK
```
**Status:** ✅ GREEN. R-α.1 model surgery infrastructure (commits `5611063` + `3015545`) is intact. Self-test re-run yields slightly different relerr from initial run (0.4485 → 0.4528 at 2-bit, 0.0901 → 0.0915 at 4-bit) due to PyTorch RNG seeding differences across process invocations — both are within expected stochastic variation for synthetic test data, no regression.

### B1.0.5 Findings file committed
This file. Commit will land in the B1.0 phase commit alongside this finding.

## Surprises (Plan Hygiene Rule 5 disclosure)

**No surprises this preflight.** All checks landed within expectations. The earlier session's Buku GPU contention was a one-time scheduling issue that has since cleared.

## Implication for downstream B1.1+ work

- **B1.1 (TurboQuant outlier port):** unblocked. Coding work, no GPU dependency until B1.1.4 smoke test.
- **B1.2 (Gemma full run):** unblocked. Estimated wall clock ~25-35 min for 16 cells (5 methods × 3 bits + FP16 baseline) at seq_len=2048 max_samples=30.
- **B1.3 (Mistral full run):** unblocked. Estimated ~75-100 min wall clock.
- **B1.4 (Qwen2-7B full run):** unblocked. Estimated ~75-100 min wall clock.
- **B1.5 (Llama 2):** still blocked on Meta HF access (C1.0.0). Deferred — Path B does NOT depend on Llama 2 numbers; the 3-model story (Gemma + Mistral + Qwen2-7B) covers all 3 KV-head architectures (MQA, GQA-32:8, GQA-28:4) sufficient for the cross-architecture validation narrative.

## Effort

| Sub-task | Estimate | Actual | Variance |
|---|---|---|---|
| B1.0.1 GPU snapshot | 5 min | 1 min | -80% |
| B1.0.2 Disk check | 5 min | 1 min | -80% |
| B1.0.3 Model cache check | 10 min | 1 min | -90% |
| B1.0.4 Script integrity | 2 min | 2 min | 0% |
| B1.0.5 Findings file | 30 min | 15 min | -50% |
| **B1.0 total** | **52 min** | **20 min** | **-62%** |

Phase C surprise budget +30%. Within budget on the under side.

## Gate status

**B1.0 → B1.1 gate:** ✅ PASS. All 5 checks GREEN, this findings file ready for commit, no follow-up actions required before B1.1.

The next concrete step is **B1.1.1**: implement `apply_turboquant_4d_with_outliers` in `scripts/quant_attention.py` per the published TurboQuant ICLR 2026 spec (top 15% high-variance channels stored in fp16, remaining 85% quantized via random ortho rotation + per-coord uniform).
