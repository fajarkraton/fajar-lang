# Plan: FajarQuant Research Paper — Sprint 5

> **Goal:** Publication-ready paper draft for MLSys / OSDI / ISCA
> **Pages:** ~14 main + 2.5 appendix
> **Writing time:** ~4 weeks (can be parallelized)
> **Target:** "FajarQuant: Hardware-Aware Adaptive Vector Quantization for Embedded ML Inference"

---

## Draft Abstract (248 words)

> Vector quantization is critical for deploying large language models on
> resource-constrained embedded devices, where KV cache memory dominates
> inference cost. TurboQuant (Zandieh et al., 2025) achieves near-optimal
> MSE distortion using random orthogonal rotation followed by coordinate-wise
> Lloyd-Max quantization, but its worst-case 2.7x gap from optimal and
> data-agnostic design leave significant room for structured data.
>
> We present FajarQuant, a hardware-aware adaptive quantization system built
> natively into Fajar Lang. FajarQuant introduces three innovations:
> (1) PCA-based adaptive rotation that replaces random rotation with per-head
> eigenvector alignment, reducing MSE by 49% at 2 bits, 76% at 3 bits, and
> 86% at 4 bits on structured KV cache data; (2) fused quantized attention
> that computes scores directly on quantized KV via codebook lookup,
> eliminating O(N*d) dequantization memory; and (3) hierarchical
> multi-resolution allocation that assigns more bits to recent tokens,
> achieving 48.7% bit savings vs flat allocation at 10K context.
>
> Uniquely, Fajar Lang's @kernel/@device annotations provide compile-time
> guarantees that quantization code is allocation-free and attention code is
> pointer-safe, enabling safe deployment without runtime checks.

---

## Section-by-Section Plan

| # | Section | Pages | Depends On | Key Content |
|---|---------|-------|------------|-------------|
| 1 | Introduction | 1.5 | None | Problem: 7B model KV = ~4GB. Gap: 2.7x. 3 contributions |
| 2 | Background | 1.5 | None | TurboQuant: Beta PDF, Lloyd-Max, Alg 1&2, bounds |
| 3 | Adaptive Rotation | 2.0 | Benchmarks | PCA rotation, Theorem 3 (MSE bound), **Fig 4** |
| 4 | Fused Attention | 1.5 | Timing bench | Algebraic identity proof, memory analysis, **Table 2** |
| 5 | Hierarchical | 1.5 | Hier bench | BitSchedule, tier allocation, budget analysis, **Table 3** |
| 6 | Compile-Time Safety | 1.0 | None | @kernel/@device, zero runtime overhead |
| 7 | Evaluation | 2.5 | ALL benches | Tables 1-4, Figures 9-11 |
| 8 | Related Work | 0.75 | None | TurboQuant, AQLM, QuIP#, FlexGen |
| 9 | Conclusion | 0.25 | All | Restate headline numbers, future work |
| A | Algorithms | 1.0 | Sec 3-5 | Pseudocode for 4 algorithms |
| B | Proofs | 1.0 | Sec 3 | 3 formal proof sketches |
| C | Code | 0.5 | Sec 6 | @kernel/@device examples |

---

## 11 Figures Needed

| # | Description | Type | Source |
|---|---|---|---|
| 1 | System architecture (3-layer stack) | Diagram | FAJARQUANT_PLAN.md |
| 2 | Beta PDF for d=16,64,128,256 | Line plot | `beta_pdf()` |
| 3 | Eigenvalue spectrum (rank 4 vs 16 vs full) | Bar chart | Synthetic data |
| 4 | MSE vs bits (adaptive vs random) — KEY | Line plot | `fajarquant_compare()` |
| 5 | Memory: FP16 vs quantized vs fused | Bar chart | Analytical formulas |
| 6 | Tier allocation (4b/3b/2b/1b zones) | Horizontal bar | `BitSchedule` |
| 7 | Budget: hierarchical vs flat | Area chart | `total_bits()` |
| 8 | Context lattice (@kernel/@device/@safe) | Diagram | effects.rs |
| 9 | MSE vs bits (full data, all dims) | Line + error bars | Extended benchmarks |
| 10 | Dimension scaling (d vs improvement%) | Line plot | Benchmarks |
| 11 | MSE per tier (quality per bit level) | Bar chart | `mse_per_tier()` |

---

## 4 Data Tables

### Table 1: MSE Distortion (MAIN RESULT)

Need: d ∈ {16, 32, 64, 128}, b ∈ {1,2,3,4}, n=1000, 5 seeds → mean ± std

### Table 2: Memory Savings (Fused Attention)

| Seq Length | FP16 KV | Quantized b=3 | Fused Extra | Standard Extra |
|---|---|---|---|---|
| 1K, d=128 | 512 KB | 32 KB | 64 B | 256 KB |
| 4K, d=128 | 2 MB | 128 KB | 64 B | 1 MB |
| 16K, d=128 | 8 MB | 512 KB | 64 B | 4 MB |

### Table 3: Hierarchical Budget

| Tier | Tokens | Bits | Total |
|---|---|---|---|
| Tier 1 (4b) | 256 | 4 | 1,024 |
| Tier 2 (3b) | 768 | 3 | 2,304 |
| Tier 3 (2b) | 3,072 | 2 | 6,144 |
| Tier 4 (1b) | 5,904 | 1 | 5,904 |
| **Total** | 10,000 | 1.54 avg | **15,376 (48.7% savings)** |

### Table 4: End-to-End Combined (NEW)

FajarQuant (all 3 innovations) vs TurboQuant baseline at various configs.

---

## 3 Proof Sketches

### Proof 1: Adaptive MSE Bound (Theorem 3)
- After PCA: coordinate k has variance λ_k
- Lloyd-Max for N(0, λ_k): MSE ∝ λ_k / 4^b
- Total: MSE_adaptive = Σλ_k / (d·4^b) = trace(C) / (d·4^b)
- Random: 2.7x gap × d/r_eff factor from eigenvalue concentration
- Improvement: O(d/r_eff) when effective rank r_eff << d

### Proof 2: Fused Attention Correctness
- q^T · dequant(k) = q^T · Pi^T · c[idx] = (Pi·q)^T · c[idx]
- = Σ_j (Pi·q)[j] · c[idx[j]] = codebook_dot_product(Pi·q, idx, c)
- Already verified numerically (< 1e-10 error in test)

### Proof 3: Budget Savings
- B_hier = Σ(n_i · b_i) + (N - Σn_i) · b_min
- B_flat = N · b_flat
- Savings = 1 - B_hier/B_flat (direct computation)

---

## Benchmarks to Run

```bash
# 1. Extended MSE grid (d × b × seeds):
cargo run --release -- run examples/fajarquant_paper_benchmark.fj > paper/data/mse.tsv

# 2. Timing (fused vs standard attention):
cargo bench --bench fajarquant_bench > paper/data/timing.txt

# 3. Hierarchical budget analysis:
cargo run --release -- run examples/fajarquant_hierarchical_bench.fj > paper/data/hier.tsv
```

**Code changes needed:**
- Add seed parameter to `fajarquant_compare` builtin
- Add `compare_adaptive_vs_random_seeded()` in adaptive.rs
- Create `examples/fajarquant_paper_benchmark.fj` with full grid
- Create `benches/fajarquant_bench.rs` for criterion timing

---

## Writing Order

**Phase A (Days 1-2):** Infrastructure
- Create `paper/` dir, LaTeX template, references.bib (15-20 entries)
- Extend benchmark scripts with seed support

**Phase B (Days 3-5):** Run all benchmarks, generate figures

**Phase C (Days 6-12):** Independent sections (parallelizable)
- Section 2 (Background), 6 (Safety), 8 (Related Work), 1 (Intro)

**Phase D (Days 13-22):** Data-dependent sections
- Section 3 (Adaptive), 4 (Fused), 5 (Hierarchical), 7 (Evaluation)

**Phase E (Days 23-28):** Finalize
- Appendices, Conclusion, Abstract revision, proofread

---

## Files to Create

| File | Purpose |
|------|---------|
| `paper/fajarquant.tex` | Main LaTeX |
| `paper/references.bib` | Bibliography |
| `paper/figures/` | Generated plots |
| `paper/data/` | Raw benchmark data |
| `examples/fajarquant_paper_benchmark.fj` | Extended benchmark grid |
| `benches/fajarquant_bench.rs` | Criterion timing |

## Files to Modify

| File | Change |
|------|--------|
| `builtins.rs` | Add seed param to fajarquant_compare |
| `adaptive.rs` | Add seeded comparison variant |
| `hierarchical.rs` | Wire compare_hierarchical_vs_flat to builtins |

---

*Paper writing can start in parallel with Sprints 3-4 (UNet + RL)*
*BibTeX: TurboQuant, AQLM, QuIP#, FlexGen, Lloyd-Max (1982), Vaswani (2017), etc.*
