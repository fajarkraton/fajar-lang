# FajarQuant: Hardware-Aware Adaptive Vector Quantization for Embedded ML Inference

> **Paper Outline — Working Draft**
> **Target venue:** Systems/ML conference (MLSys, OSDI, or ISCA)
> **Authors:** Muhamad Fajar Putranto (TaxPrime/PrimeCore.id)

---

## Abstract (~250 words)

Vector quantization is critical for deploying large language models on
resource-constrained devices. TurboQuant (Zandieh et al., 2025) achieves
near-optimal MSE distortion using random orthogonal rotation followed by
coordinate-wise scalar quantization, but its worst-case 2.7x gap from
optimal leaves room for improvement. We present **FajarQuant**, a
hardware-aware adaptive quantization system built natively into Fajar Lang,
a systems programming language with compile-time context enforcement.
FajarQuant introduces three innovations: (1) **PCA-based adaptive rotation**
that replaces random rotation with data-specific eigenvector alignment,
reducing MSE by 49-86% on structured data; (2) **fused quantized attention**
that computes attention scores directly on quantized KV cache entries via
codebook lookup, eliminating O(N*d) dequantization memory; and (3)
**hierarchical multi-resolution** allocation that assigns more bits to
recent tokens and fewer to old tokens, achieving better quality at lower
total bit budget. Uniquely, Fajar Lang's @kernel/@device annotation system
provides compile-time guarantees that quantization code is allocation-free
(@kernel) and attention code is pointer-safe (@device), enabling safe
deployment on embedded targets without runtime checks.

---

## 1. Introduction

### 1.1 Problem: LLM Inference on Embedded Devices

- KV cache memory dominates inference cost for long sequences
- 7B model at 4K context: ~4 GB KV cache in FP16
- Embedded targets (Hexagon DSP, Cortex-M, RISC-V): 256 KB - 8 MB SRAM
- Need: 8-64x compression with acceptable quality loss

### 1.2 Existing Work

- **Product Quantization (PQ):** Partition + per-subspace codebook. O(d) lookups.
- **TurboQuant (Zandieh et al., 2025):** Random rotation + Lloyd-Max. 2.7x gap.
- **AQLM, QuIP#:** Weight quantization (different from KV cache quantization).
- **Gap:** No existing system exploits data structure OR provides compile-time safety.

### 1.3 Contributions

1. Adaptive rotation: data-driven rotation via PCA per attention head
2. Fused codebook attention: zero-copy attention computation
3. Hierarchical bit allocation: recency-aware bit budget
4. Compile-time safety: @kernel/@device enforcement (unique to Fajar Lang)

---

## 2. Background: TurboQuant

### 2.1 Random Rotation + Scalar Quantization

- Lemma 1: After rotation, coordinates ~ Beta((d-1)/2, (d-1)/2)
- Lloyd-Max optimal quantizer for Beta distribution
- Algorithm 1 (QUANT_mse): Pi*x → coordinate-wise quantize
- Algorithm 2 (QUANT_prod): MSE at (b-1) bits + 1-bit QJL on residual

### 2.2 Distortion Bounds

- D_mse ≤ (sqrt(3)*pi/2) * 1/4^b (Theorem 1)
- D_prod: unbiased inner product estimator (Theorem 2)
- Gap from optimal: ~2.7x (constant, independent of d)

---

## 3. FajarQuant: Adaptive Rotation (Innovation 1)

### 3.1 Motivation

KV cache vectors are NOT uniform on the hypersphere — they have strong
structure due to the attention mechanism. Top eigenvalues of the KV
covariance matrix capture 90%+ of variance.

### 3.2 PCA-Based Rotation

- Calibration phase: collect N vectors per (layer, head)
- Compute covariance C = (1/N) * Σ x_i * x_i^T
- Eigenvectors of C form the rotation: Pi_h = eigvecs(C)
- After PCA rotation: coordinates ~ N(0, lambda_k), NOT Beta

### 3.3 Data-Specific Codebook

- Lloyd-Max on the actual rotated coordinate distribution
- Codebook adapts to each head's specific distribution shape
- Fallback: TurboQuant random rotation before calibration completes

### 3.4 Theoretical Analysis

- **Theorem (informal):** For low-rank data with effective rank r << d,
  adaptive rotation concentrates (1-epsilon) fraction of variance into
  r coordinates, enabling (d/r)x better utilization of bit budget.
- **Empirical result:** 49-86% MSE reduction vs random rotation (d=16-32).

---

## 4. FajarQuant: Fused Quantized Attention (Innovation 2)

### 4.1 Key Insight

Standard: score = q^T * dequant(k) = q^T * Pi^T * c[idx]
Fused:    score = (Pi*q)^T * c[idx] = Σ_j q_rot[j] * c[idx[j]]

Same computation, but fused version NEVER allocates dequantized vector.

### 4.2 Memory Analysis

- Standard: O(N * d) extra memory for dequantized keys per query
- Fused: O(2^b) memory for codebook (fits in L1 cache)
- Saving: O(N * d) → O(1) per attention head

### 4.3 Implementation

- `codebook_dot_product(q_rot, k_indices, codebook)` — O(d) lookups
- `codebook_weighted_sum(weights, v_indices, codebook)` — O(N*d) lookups
- Total: same asymptotic compute, O(N*d) less memory

---

## 5. FajarQuant: Hierarchical Multi-Resolution (Innovation 3)

### 5.1 Observation

In autoregressive generation, recent tokens receive most attention weight.
Spending equal bits on all tokens wastes budget on rarely-accessed old tokens.

### 5.2 Bit Scheduling Strategies

1. **Fixed tiers:** [4b, 3b, 2b, 1b] at [256, 1024, 4096, ∞] tokens
2. **Exponential decay:** b(age) = max(b_min, b_max * exp(-λ * age))
3. **Attention-aware:** b(pos) ∝ attn_score(pos) (requires forward pass)

### 5.3 Re-Quantization

When tokens age beyond a tier boundary, they are re-quantized at fewer bits.
The original vector is stored for re-quantization accuracy.

### 5.4 Budget Analysis

Same total bits, better allocation:
- Flat 3b × 10K tokens = 30K bits
- Hierarchical: 4b×256 + 3b×768 + 2b×3072 + 1b×6K = ~16K bits (47% savings)

---

## 6. Compile-Time Safety (Unique to Fajar Lang)

### 6.1 Dual-Context System

- @kernel: No heap allocation, no tensor operations → quantize safely
- @device: No raw pointers → attention safely
- @requires: Precondition annotations checked at compile time

### 6.2 Safety Guarantees

```fajar
@kernel fn quantize(x: [f32; 128], cb: [f32; 8]) -> [u8; 128] {
    // GUARANTEED: no heap alloc, no tensor, no GC pressure
}

@device fn attention(q: Tensor, cache: QuantizedKV) -> Tensor {
    // GUARANTEED: no raw pointers, no buffer overflows
}
```

No other quantization framework provides these compile-time guarantees.

---

## 7. Evaluation

### 7.1 Experimental Setup

- Fajar Lang v20.5 on x86_64 Linux
- Dimensions: d ∈ {16, 32, 64, 128}
- Bit-widths: b ∈ {1, 2, 3, 4}
- Calibration: 100 vectors per head

### 7.2 MSE Distortion (Table 1)

| Bits | TurboQuant MSE | FajarQuant MSE | Improvement |
|------|---------------|----------------|-------------|
| b=1  | baseline      | ~comparable    | ~0%         |
| b=2  | baseline      | -49%           | significant |
| b=3  | baseline      | -76%           | large       |
| b=4  | baseline      | -86%           | very large  |

### 7.3 Memory Savings (Table 2)

Hierarchical allocation vs flat allocation at same quality level.

### 7.4 Fused Attention Throughput (Table 3)

Memory bandwidth savings from codebook-based attention.

---

## 8. Related Work

- TurboQuant (Zandieh et al., 2025): Our baseline
- AQLM (Egiazarian et al., 2024): Weight quantization, not KV cache
- QuIP# (Tseng et al., 2024): Incoherence + LDLQ for weights
- FlexGen (Sheng et al., 2023): Offloading, not quantization
- Fajar Lang (Putranto, 2026): Language with dual-context safety

---

## 9. Conclusion

FajarQuant achieves 49-86% lower MSE than TurboQuant on structured data
by exploiting the low-rank structure of KV cache vectors through adaptive
PCA rotation. Combined with fused codebook attention (zero extra memory)
and hierarchical bit allocation (47% fewer total bits), FajarQuant enables
practical LLM inference on embedded devices. Fajar Lang's compile-time
@kernel/@device enforcement provides unique safety guarantees unavailable
in any existing quantization framework.

---

## Appendix A: Algorithm Pseudocode

(Algorithms 1-2 from TurboQuant + our extensions)

## Appendix B: Proof Sketches

(Informal bounds for adaptive rotation improvement)

## Appendix C: Fajar Lang Code Listings

(Key .fj examples demonstrating the complete pipeline)

---

*Draft outline — v1.0, 2026-04-04*
*Implementation: src/runtime/ml/turboquant.rs + src/runtime/ml/fajarquant/*
