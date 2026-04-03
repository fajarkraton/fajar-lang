# FajarQuant — Complete E2E Implementation Plan

> **Date:** 2026-04-03
> **Author:** Fajar (TaxPrime / PrimeCore.id)
> **Target:** Fajar Lang V22 "Quantum"
> **Status:** PLANNING
> **Standard:** Every task has concrete `fj run` verification. [x] = user can run it.
> **Paper target:** "FajarQuant: Hardware-Aware Adaptive Vector Quantization for Embedded ML Inference"

---

## Executive Summary

FajarQuant is a next-generation vector quantization system built natively into
Fajar Lang, designed to **surpass TurboQuant** (Zandieh et al., Google Research,
arXiv:2504.19874, April 2025) through three architectural innovations only
possible in a language with compile-time context enforcement and hardware-aware
dispatch.

### TurboQuant Baseline (What We Match)

| Property | TurboQuant | Bit Budget |
|----------|-----------|------------|
| MSE distortion | D_mse <= (sqrt(3)pi/2) * 1/4^b | b bits/coord |
| IP distortion (unbiased) | D_prod <= (sqrt(3)pi^2/d) * ||y||^2 * 1/4^b | b bits/coord |
| Gap from optimal | ~2.7x (constant factor) | all b >= 1 |
| KV cache quality | Neutral at 3.5 bits, marginal at 2.5 bits | 4x+ compression |
| NN search | Outperforms Product Quantization | indexing time ~0 |

### FajarQuant Innovations (What We Beat)

| Innovation | Expected Improvement | Mechanism |
|-----------|---------------------|-----------|
| **Adaptive rotation per head** | 2.7x gap -> ~1.3-1.5x | PCA-based rotation instead of random |
| **Fused quantized attention** | 2-4x throughput | Skip dequantize, compute in quantized domain |
| **Hierarchical multi-resolution** | 10-20% quality at same budget | More bits for recent tokens |

---

## Architecture Overview

```
                      FajarQuant System Architecture

  .fj source code
       |
       v
  ┌─────────────────────────────────────────────────────────┐
  │  COMPILER LAYER (compile-time)                          │
  │                                                         │
  │  @kernel fn quantize(...)     @device fn attention(...) │
  │       │                              │                  │
  │       │  verify: no heap,            │  verify: no raw  │
  │       │  no tensor alloc             │  pointers        │
  │       │                              │                  │
  │  Shape check: codebook[2^b] matches tensor dim          │
  │  Budget check: @requires(total_bits <= budget)          │
  └────────────┬─────────────────────────┬──────────────────┘
               │                         │
               v                         v
  ┌────────────────────┐   ┌─────────────────────────┐
  │  QUANTIZATION      │   │  FUSED COMPUTE          │
  │  RUNTIME           │   │  RUNTIME                │
  │                    │   │                         │
  │  Phase A: Calibrate│   │  Quantized Attention    │
  │   - PCA per head   │   │   - Codebook lookup +   │
  │   - Optimal Pi_h   │   │     accumulate in 1 pass│
  │                    │   │   - No dequant needed   │
  │  Phase B: Online   │   │                         │
  │   - Rotate: Pi_h*x │   │  Hierarchical Schedule  │
  │   - Quantize: LM   │   │   - Recent: 4 bits     │
  │   - Pack: b-bit    │   │   - Old: 2 bits        │
  │                    │   │   - Ancient: 1 bit      │
  └────────────────────┘   └─────────────────────────┘
               │                         │
               v                         v
  ┌─────────────────────────────────────────────────────────┐
  │  HARDWARE DISPATCH                                      │
  │                                                         │
  │  accelerate("quantized_attn", kv)                       │
  │    -> NPU: native INT4 on Hexagon DSP                   │
  │    -> GPU: CUDA INT8 tensor cores                       │
  │    -> CPU: AVX2/NEON SIMD                               │
  └─────────────────────────────────────────────────────────┘
```

---

## Phase 0: Prerequisites — Linear Algebra & Tensor Ops (18 tasks)

> **Goal:** Add the 6 blocker operations that both TurboQuant and FajarQuant need.
> **LOC estimate:** ~1,800
> **Files modified:** `src/runtime/ml/ops.rs`, `src/interpreter/eval/builtins.rs`,
>   `src/interpreter/eval/mod.rs`, `src/analyzer/type_check/register.rs`, `Cargo.toml`

### 0A: New Tensor Operations in `ops.rs` (10 tasks)

| # | Task | Signature | Verification |
|---|------|-----------|-------------|
| 0.1 | `tensor_sign(x)` — element-wise sign | `sign(tensor) -> tensor` | `sign(from_data([-3,0,5],[3]))` -> `[-1,0,1]` |
| 0.2 | `tensor_argmin(x)` — index of minimum element | `argmin(tensor) -> i64` | `argmin(from_data([3,1,2],[3]))` -> `1` |
| 0.3 | `tensor_norm(x)` — L2 norm | `norm(tensor) -> f64` | `norm(from_data([3,4],[2]))` -> `5.0` |
| 0.4 | `tensor_dot(x, y)` — inner/dot product | `dot(tensor, tensor) -> f64` | `dot(from_data([1,2],[2]), from_data([3,4],[2]))` -> `11.0` |
| 0.5 | `tensor_exp(x)` — element-wise e^x | `exp(tensor) -> tensor` | `exp(zeros([2]))` -> `[1.0, 1.0]` |
| 0.6 | `tensor_log(x)` — element-wise ln(x) | `log_tensor(tensor) -> tensor` | `log_tensor(from_data([1,E],[2]))` -> `[0.0, 1.0]` |
| 0.7 | `tensor_sqrt(x)` — element-wise sqrt | `sqrt_tensor(tensor) -> tensor` | `sqrt_tensor(from_data([4,9],[2]))` -> `[2.0, 3.0]` |
| 0.8 | `tensor_abs(x)` — element-wise abs | `abs_tensor(tensor) -> tensor` | `abs_tensor(from_data([-3,4],[2]))` -> `[3.0, 4.0]` |
| 0.9 | `tensor_clamp(x, min, max)` — element-wise clamp | `clamp_tensor(t, -1.0, 1.0) -> tensor` | Values clamped to range |
| 0.10 | `tensor_where(cond, x, y)` — conditional select | `where_tensor(mask, a, b) -> tensor` | Select elements based on condition |

**Implementation notes:**
- All operations use `ndarray::ArrayD<f64>` via `TensorValue::data()` and `TensorValue::from_ndarray()`
- Each op is ~20-40 lines in `ops.rs` + ~15 lines wiring in `builtins.rs`
- Total: ~400 LOC

### 0B: Linear Algebra Operations (4 tasks)

| # | Task | Dependency | Verification |
|---|------|-----------|-------------|
| 0.11 | Add `ndarray-linalg` to Cargo.toml (feature-gated `linalg`) | Needs LAPACK/OpenBLAS | `cargo build --features linalg` compiles |
| 0.12 | `qr_decompose(matrix)` -> (Q, R) | ndarray-linalg QR | Q is orthogonal, Q*R = original |
| 0.13 | `random_orthogonal(d)` -> matrix | QR of randn(d,d) | `dot(Q, transpose(Q))` ~= identity |
| 0.14 | `pca(data, k)` -> (components, explained_variance) | SVD or eigen | Top-k components of data matrix |

**Implementation notes:**
- Feature-gated under `--features linalg` to avoid mandatory LAPACK dependency
- Without `linalg` feature: fallback to Gram-Schmidt (approximate but no external dep)
- ~400 LOC including fallback

### 0C: Scalar Math Builtins (2 tasks)

| # | Task | Verification |
|---|------|-------------|
| 0.15 | `exp(x)` — scalar e^x | `exp(1.0)` -> `2.718...` |
| 0.16 | `gamma(x)` — Gamma function (via Lanczos approx) | `gamma(5.0)` -> `24.0` (= 4!) |

**Implementation notes:**
- `exp()` is trivial: `f64::exp(x)`
- Gamma via Lanczos approximation: ~50 LOC, no external deps
- Total: ~80 LOC

### 0D: Integration Tests (2 tasks)

| # | Task | Verification |
|---|------|-------------|
| 0.17 | `tests/linalg_tests.rs` — test all 16 new ops | `cargo test --test linalg_tests` |
| 0.18 | Example: `examples/linalg_demo.fj` | `fj run examples/linalg_demo.fj` |

**Phase 0 gate:**
```bash
fj run examples/linalg_demo.fj       # all ops work
cargo test --test linalg_tests        # all pass
cargo test --lib && cargo clippy      # no regressions
```

---

## Phase 1: TurboQuant Baseline (14 tasks)

> **Goal:** Faithful implementation of Algorithms 1 & 2 from the paper.
> **LOC estimate:** ~1,200
> **New file:** `src/runtime/ml/turboquant.rs`

### 1A: Core Data Structures (3 tasks)

| # | Task | Contents |
|---|------|----------|
| 1.1 | `Codebook` struct | `centroids: Vec<f64>`, `boundaries: Vec<f64>`, `bit_width: u8` |
| 1.2 | `QuantizedVector` struct | `indices: Vec<u8>`, `norm: f64`, `rotation_id: u64` |
| 1.3 | `TurboQuantConfig` struct | `bit_width: u8`, `dim: usize`, `rotation: ArrayD<f64>` |

### 1B: Lloyd-Max Scalar Quantizer (3 tasks)

| # | Task | Verification |
|---|------|-------------|
| 1.4 | `beta_pdf(x, d)` — PDF of coordinate after rotation (Lemma 1) | Matches analytical formula |
| 1.5 | `lloyd_max(pdf, b)` — optimal 1D quantizer for given distribution | For b=1, centroids ~= +/- sqrt(2/pi*d) |
| 1.6 | `precompute_codebooks(d, max_b)` — codebooks for b=1..max_b | Store as lookup table |

**Implementation notes:**
- Lloyd-Max is iterative: start with uniform centroids, update centroids = E[X|X in bucket],
  update boundaries = midpoints. Converges in ~20 iterations.
- Pre-compute for d=128,256,512,768,1024,1536,2048,3072,4096 and b=1,2,3,4.
- ~300 LOC

### 1C: TurboQuant_mse — Algorithm 1 (3 tasks)

| # | Task | Verification |
|---|------|-------------|
| 1.7 | `quant_mse(x, Pi, codebook)` — quantize vector | Returns index vector |
| 1.8 | `dequant_mse(idx, Pi_T, codebook)` — reconstruct | `norm(x - dequant_mse(quant_mse(x))) < threshold` |
| 1.9 | End-to-end MSE test | MSE matches paper's D_mse bounds (Table: 0.36, 0.117, 0.03, 0.009 for b=1,2,3,4) |

**Pseudocode (from paper Algorithm 1):**
```
QUANT_mse(x):
  y = Pi * x                      // random rotation
  for j in 0..d:
    idx[j] = argmin_k |y[j] - c_k|  // nearest centroid
  return idx

DEQUANT_mse(idx):
  for j in 0..d:
    y_tilde[j] = c[idx[j]]         // lookup centroid
  x_tilde = Pi^T * y_tilde          // inverse rotation
  return x_tilde
```

### 1D: TurboQuant_prod — Algorithm 2 (3 tasks)

| # | Task | Verification |
|---|------|-------------|
| 1.10 | `qjl_quantize(r, S)` — sign(S * r) 1-bit QJL | Returns sign vector in {-1, +1}^d |
| 1.11 | `qjl_dequantize(qjl, S, gamma)` — sqrt(pi/2)/d * gamma * S^T * qjl | Unbiased IP estimator |
| 1.12 | `quant_prod(x, config)` — full inner-product quantizer | `E[<y, dequant(quant(x))>] = <y, x>` (unbiased) |

**Pseudocode (from paper Algorithm 2):**
```
QUANT_prod(x):
  idx = QUANT_mse(x)  with bit-width b-1
  r = x - DEQUANT_mse(idx)           // residual
  qjl = sign(S * r)                  // 1-bit QJL on residual
  return (idx, qjl, ||r||_2)

DEQUANT_prod(idx, qjl, gamma):
  x_mse = DEQUANT_mse(idx)
  x_qjl = sqrt(pi/2)/d * gamma * S^T * qjl
  return x_mse + x_qjl
```

### 1E: Integration (2 tasks)

| # | Task | Verification |
|---|------|-------------|
| 1.13 | Wire builtins: `turboquant_create`, `turboquant_encode`, `turboquant_decode`, `turboquant_inner_product` | `fj run` works |
| 1.14 | Example: `examples/turboquant_demo.fj` | Shows MSE and IP distortion at b=1,2,3,4 |

**Example .fj program:**
```fajar
// TurboQuant demo
let config = turboquant_create(128, 3)  // dim=128, bits=3
let x = randn([128])
let encoded = turboquant_encode(config, x)
let decoded = turboquant_decode(config, encoded)
let mse = norm(x - decoded) / 128.0
println(f"MSE distortion (b=3): {mse}")  // should be ~0.03/128
```

**Phase 1 gate:**
```bash
fj run examples/turboquant_demo.fj
cargo test --lib turboquant              # bounds match paper
```

---

## Phase 2: FajarQuant Innovation 1 — Adaptive Rotation (10 tasks)

> **Goal:** Replace random rotation with PCA-based per-head rotation.
> **LOC estimate:** ~800
> **New file:** `src/runtime/ml/fajarquant/adaptive.rs`

### 2A: Per-Head Calibration (4 tasks)

| # | Task | Verification |
|---|------|-------------|
| 2.1 | `CalibrationBuffer` — accumulates vectors per head during warmup | Stores up to N calibration vectors |
| 2.2 | `compute_pca_rotation(buffer)` — PCA on accumulated vectors | Returns orthogonal rotation matrix |
| 2.3 | `RotationCache` — stores Pi_h per (layer, head) | Indexed by (layer_idx, head_idx) |
| 2.4 | `is_calibrated(cache, layer, head)` -> bool | True after N vectors seen |

**Algorithm:**
```
CALIBRATE(layer_h, head_h):
  // During first ~100 tokens:
  buffer[layer_h][head_h].append(kv_vector)
  
  if buffer.len() >= CALIBRATION_SIZE:
    // Compute data covariance
    C = (1/N) * sum(x_i * x_i^T)
    // Eigendecompose: C = V * Lambda * V^T
    // Rotation = V (eigenvectors as rotation basis)
    Pi_h = eigenvectors(C)
    cache.store(layer_h, head_h, Pi_h)
```

### 2B: Adaptive Quantizer (4 tasks)

| # | Task | Verification |
|---|------|-------------|
| 2.5 | `adaptive_codebook(rotation, calibration_data, b)` — data-specific codebook | Distortion lower than universal codebook |
| 2.6 | `fajarquant_encode_adaptive(x, rotation, codebook)` | Uses per-head rotation + codebook |
| 2.7 | `fajarquant_decode_adaptive(encoded, rotation, codebook)` | Reconstruction matches |
| 2.8 | Fallback: use TurboQuant random rotation before calibration | Seamless transition |

### 2C: Benchmarking (2 tasks)

| # | Task | Verification |
|---|------|-------------|
| 2.9 | Compare MSE: adaptive vs random rotation on synthetic data | Adaptive MSE < random MSE |
| 2.10 | Example: `examples/fajarquant_adaptive_demo.fj` | Shows distortion improvement |

**Why this beats TurboQuant:**
```
TurboQuant: random Pi -> coordinates ~ Beta(d) -> universal codebook
  Gap from optimal: ~2.7x (worst-case assumption)

FajarQuant: PCA Pi_h -> coordinates ~ N(0, lambda_k) -> adapted codebook  
  Gap from optimal: ~1.3-1.5x (data-specific, exploits structure)
  
Theoretical justification:
  - KV cache vectors are LOW-RANK (not worst-case on hypersphere)
  - Top-k eigenvalues capture 90%+ of variance
  - Adapted rotation concentrates energy in fewer coordinates
  - Codebook needs fewer bits for concentrated distribution
```

**Phase 2 gate:**
```bash
fj run examples/fajarquant_adaptive_demo.fj  # shows improvement
cargo test --lib fajarquant::adaptive        # all pass
```

---

## Phase 3: FajarQuant Innovation 2 — Fused Quantized Attention (8 tasks)

> **Goal:** Compute attention scores directly on quantized KV, skip full dequantize.
> **LOC estimate:** ~1,500
> **New file:** `src/runtime/ml/fajarquant/fused_attention.rs`

### 3A: Codebook-Based Attention (4 tasks)

| # | Task | Verification |
|---|------|-------------|
| 3.1 | `QuantizedKVCache` struct — stores quantized K and V per layer | `cache.store(layer, token_idx, k_quant, v_quant)` |
| 3.2 | `codebook_dot_product(query, k_indices, codebook)` — compute q*k without dequantize | Result matches `dot(q, dequant(k))` within epsilon |
| 3.3 | `codebook_weighted_sum(attn_weights, v_indices, codebook)` — compute attn*V without dequantize | Result matches `matmul(attn, dequant(V))` within epsilon |
| 3.4 | `fused_quantized_attention(Q, K_quant, V_quant, codebook)` — full attention on quantized KV | Output matches standard attention within tolerance |

**Key insight:**
```
Standard:     score = q^T * dequant(k) = q^T * c[idx]
              = sum_j q[j] * c[idx[j]]

Fused:        score = sum_j q[j] * c[idx[j]]
              // SAME computation, but we never allocate dequant(k) as a tensor!
              // Saves O(d) memory per token and O(N*d) total for sequence

For V aggregation:
Standard:     output = sum_i attn[i] * dequant(v_i)  // allocate N dequantized vectors
Fused:        output[j] = sum_i attn[i] * c[v_idx[i][j]]  // accumulate directly
              // Same result, O(N*d) less memory
```

### 3B: KV Cache Manager (2 tasks)

| # | Task | Verification |
|---|------|-------------|
| 3.5 | `KVCacheManager` — manages quantized KV across layers/heads | `manager.append(layer, head, k, v)` quantizes and stores |
| 3.6 | `cache_memory_usage(manager)` -> (quantized_bytes, full_precision_bytes, ratio) | Shows compression ratio |

### 3C: Integration (2 tasks)

| # | Task | Verification |
|---|------|-------------|
| 3.7 | Wire builtins: `kv_cache_create`, `kv_cache_append`, `kv_cache_attention` | `fj run` works |
| 3.8 | Example: `examples/fajarquant_fused_demo.fj` | Shows memory savings + correct output |

**Performance model:**
```
Standard attention with TurboQuant:
  Memory:  O(N * d * sizeof(f16))     for dequantized KV
  Compute: O(N * d) dequant + O(N * d) matmul = O(2 * N * d)

FajarQuant fused attention:
  Memory:  O(N * d * b/8)             for quantized KV (no dequant buffer)
  Compute: O(N * d * 2^b) lookups     (codebook is small, fits in L1 cache)
  
  For b=3: 2^b = 8 lookups vs 1 multiply+add -> ~same compute
  But ZERO extra memory allocation -> 4-8x less memory bandwidth
```

**Phase 3 gate:**
```bash
fj run examples/fajarquant_fused_demo.fj
cargo test --lib fajarquant::fused_attention
```

---

## Phase 4: FajarQuant Innovation 3 — Hierarchical Multi-Resolution (8 tasks)

> **Goal:** Allocate more bits to important tokens, fewer to old tokens.
> **LOC estimate:** ~600
> **New file:** `src/runtime/ml/fajarquant/hierarchical.rs`

### 4A: Bit Budget Scheduler (3 tasks)

| # | Task | Verification |
|---|------|-------------|
| 4.1 | `BitSchedule` — defines bit allocation per token position | `schedule.bits_for(position, total_tokens)` |
| 4.2 | `exponential_decay_schedule(base_bits, min_bits, decay_rate)` | Recent tokens get `base_bits`, old tokens get `min_bits` |
| 4.3 | `attention_aware_schedule(attn_scores, budget)` | Tokens with high attention get more bits |

**Bit allocation strategies:**
```
Strategy 1: Exponential Decay
  bits(pos) = max(min_bits, base_bits * exp(-decay * age))
  
  Position:   [current] [recent] ... [old]  ... [ancient]
  Bits:          4         3          2          1

Strategy 2: Attention-Aware (requires a forward pass first)
  bits(pos) = round(budget * attn_score(pos) / sum(attn_scores))
  
  Tokens that the model ACTUALLY attends to get more bits.

Strategy 3: Fixed Tiers
  Last 256 tokens:   4 bits  (highest quality)
  Tokens 257-1024:   3 bits  (high quality)
  Tokens 1025-4096:  2 bits  (medium quality)
  Tokens 4097+:      1 bit   (low quality, but still unbiased IP)
```

### 4B: Multi-Resolution Quantizer (3 tasks)

| # | Task | Verification |
|---|------|-------------|
| 4.4 | `hierarchical_encode(kv_cache, schedule)` — re-quantize old tokens to fewer bits | Total bits <= budget |
| 4.5 | `hierarchical_decode(encoded, schedule, codebooks)` — decode with per-tier codebook | Correct reconstruction per tier |
| 4.6 | `promote_demote(cache, new_schedule)` — change bit allocation on existing cache | Seamless re-quantization |

### 4C: Integration (2 tasks)

| # | Task | Verification |
|---|------|-------------|
| 4.7 | Wire builtins: `bit_schedule_create`, `kv_cache_hierarchical` | `fj run` works |
| 4.8 | Example: `examples/fajarquant_hierarchical_demo.fj` | Shows quality improvement vs flat allocation |

**Why this beats TurboQuant:**
```
Same total bit budget, different allocation:

TurboQuant (flat):       3 bits * 10000 tokens = 30000 total bits
FajarQuant (hierarchical): 
  4 bits * 256 tokens    =  1024 bits  (recent, high quality)
  3 bits * 768 tokens    =  2304 bits
  2 bits * 3072 tokens   =  6144 bits
  1 bit  * 6760 tokens   =  6760 bits
  Total:                   16232 bits  (FEWER total bits!)

  OR: use the saved bits to give recent tokens even MORE precision:
  5 bits * 256 tokens    =  1280 bits  (very high quality)
  3 bits * 2048 tokens   =  6144 bits
  2 bits * 7696 tokens   = 15392 bits
  Total:                   22816 bits  (still less!)

Result: better quality where it matters, lower total storage.
```

**Phase 4 gate:**
```bash
fj run examples/fajarquant_hierarchical_demo.fj
cargo test --lib fajarquant::hierarchical
```

---

## Phase 5: Compiler Integration & @kernel/@device Enforcement (6 tasks)

> **Goal:** Leverage Fajar Lang's unique dual-context system for safety guarantees.
> **LOC estimate:** ~400

| # | Task | Verification |
|---|------|-------------|
| 5.1 | `@kernel fn quantize_kv(...)` — verify zero-allocation quantization | Compiler rejects heap/tensor ops inside |
| 5.2 | `@device fn quantized_attention(...)` — verify no raw pointer | Compiler rejects pointer ops inside |
| 5.3 | `@requires(bits_per_coord >= 1)` precondition on quantize | Compiler rejects b=0 |
| 5.4 | `@requires(total_bits(schedule) <= budget)` on hierarchical | Compiler rejects over-budget |
| 5.5 | Shape checking: codebook size = 2^b matches bit_width | Compile-time error on mismatch |
| 5.6 | Integration test: `tests/fajarquant_safety_tests.rs` | Context violations caught at compile time |

**This is unique to Fajar Lang — no other framework can provide these guarantees:**
```fajar
// COMPILE ERROR: tensor operation in @kernel context
@kernel fn bad_quantize(x: [f32; 128]) -> [u8; 64] {
    let t = zeros([128])  // KE002: tensor in @kernel
    // ...
}

// COMPILE ERROR: raw pointer in @device context
@device fn bad_attention(q: Tensor, cache: *mut u8) -> Tensor {
    let v = *cache  // DE001: raw pointer in @device
    // ...
}

// CORRECT: each context respects its constraints
@kernel fn quantize(x: [f32; 128], codebook: [f32; 8]) -> [u8; 128] {
    // Pure integer/array operations — no heap, no tensor
    let mut idx = [0u8; 128]
    for i in 0..128 {
        idx[i] = nearest_centroid(x[i], codebook)
    }
    idx
}

@device fn attention(q: Tensor, k_idx: [u8; N], codebook: Tensor) -> Tensor {
    // Tensor operations allowed — no raw pointers
    codebook_dot_product(q, k_idx, codebook)
}
```

---

## Phase 6: Benchmarks & Comparison (8 tasks)

> **Goal:** Empirically validate FajarQuant vs TurboQuant.
> **LOC estimate:** ~600

### 6A: Synthetic Benchmarks (4 tasks)

| # | Task | Metric | Verification |
|---|------|--------|-------------|
| 6.1 | MSE distortion vs bit-width (b=1..4) | D_mse for FajarQuant vs TurboQuant | Plot: FajarQuant below TurboQuant curve |
| 6.2 | IP distortion vs bit-width | D_prod for both | FajarQuant unbiased + lower variance |
| 6.3 | Quantization throughput (vectors/sec) | Speed for d=128,512,1536,3072 | FajarQuant within 2x of TurboQuant speed |
| 6.4 | Memory usage comparison | Bytes for N=10000 vectors | Hierarchical uses less total memory |

### 6B: Application Benchmarks (4 tasks)

| # | Task | Metric | Verification |
|---|------|--------|-------------|
| 6.5 | KV cache compression ratio | Bits per channel for quality-neutral | FajarQuant neutral at ~2.5-3.0 bits (vs TurboQuant 3.5) |
| 6.6 | Nearest neighbor recall@10 | On synthetic embeddings | FajarQuant >= TurboQuant recall |
| 6.7 | Fused attention throughput | Tokens/sec with quantized KV | 2-4x vs dequantize-then-compute |
| 6.8 | Example: `examples/fajarquant_benchmark.fj` | All metrics in one demo | `fj run examples/fajarquant_benchmark.fj` |

**Phase 6 gate:**
```bash
fj run examples/fajarquant_benchmark.fj  # prints comparison table
```

---

## Phase 7: Full Demo & Documentation (6 tasks)

> **Goal:** Production-ready examples, tests, documentation.

| # | Task | Verification |
|---|------|-------------|
| 7.1 | `examples/fajarquant_kv_cache.fj` — full KV cache quantization pipeline | `fj run` shows compression + quality |
| 7.2 | `examples/fajarquant_nn_search.fj` — nearest neighbor with quantized vectors | `fj run` shows recall@k |
| 7.3 | `examples/fajarquant_embedded.fj` — embedded ML inference with quantized model | Uses @kernel/@device bridge |
| 7.4 | `tests/fajarquant_e2e_tests.rs` — comprehensive E2E tests | `cargo test --test fajarquant_e2e_tests` |
| 7.5 | `docs/FAJARQUANT_API.md` — API reference | All builtins documented |
| 7.6 | `docs/FAJARQUANT_PAPER_OUTLINE.md` — paper outline for submission | Sections, theorems, experiments planned |

---

## Complete Builtin API

### TurboQuant Baseline

```fajar
// Creation
let config = turboquant_create(dim, bit_width)
let config = turboquant_create_prod(dim, bit_width)  // inner-product variant

// Encode / Decode
let encoded = turboquant_encode(config, vector)
let decoded = turboquant_decode(config, encoded)

// Inner product estimation (unbiased for _prod variant)
let ip = turboquant_inner_product(config, encoded_x, y)
```

### FajarQuant Extensions

```fajar
// Adaptive rotation (Innovation 1)
let calibrator = fajarquant_calibrator(dim, num_heads, num_layers)
fajarquant_calibrate(calibrator, layer, head, vector)  // feed calibration data
let config = fajarquant_adaptive_config(calibrator, bit_width)  // after calibration

// Fused quantized attention (Innovation 2)
let cache = kv_cache_create(num_layers, num_heads, dim, bit_width)
kv_cache_append(cache, layer, head, k_vector, v_vector)
let output = kv_cache_attention(cache, layer, head, query)  // fused, no dequant
let stats = kv_cache_stats(cache)  // compression ratio, memory usage

// Hierarchical multi-resolution (Innovation 3)
let schedule = bit_schedule_decay(base_bits: 4, min_bits: 1, decay: 0.001)
let schedule = bit_schedule_tiered([4, 3, 2, 1], [256, 1024, 4096])  // tiers
kv_cache_set_schedule(cache, schedule)
```

### Linear Algebra (Prerequisites)

```fajar
// Tensor operations
let s = sign(tensor)          // element-wise sign
let i = argmin(tensor)        // index of minimum
let n = norm(tensor)          // L2 norm
let d = dot(a, b)             // inner product
let e = exp_tensor(tensor)    // element-wise e^x
let l = log_tensor(tensor)    // element-wise ln
let s = sqrt_tensor(tensor)   // element-wise sqrt
let a = abs_tensor(tensor)    // element-wise abs

// Linear algebra (--features linalg)
let (q, r) = qr(matrix)           // QR decomposition
let orth = random_orthogonal(d)    // random rotation matrix
let (components, variance) = pca(data, k)  // PCA

// Scalar math
let e = exp(1.0)              // 2.718...
let g = gamma(5.0)            // 24.0
```

---

## Task Summary

| Phase | Tasks | LOC | New Files |
|-------|-------|-----|-----------|
| 0: Prerequisites | 18 | ~1,800 | ops.rs changes, linalg_tests.rs |
| 1: TurboQuant baseline | 14 | ~1,200 | turboquant.rs |
| 2: Adaptive rotation | 10 | ~800 | fajarquant/adaptive.rs |
| 3: Fused attention | 8 | ~1,500 | fajarquant/fused_attention.rs |
| 4: Hierarchical | 8 | ~600 | fajarquant/hierarchical.rs |
| 5: Compiler integration | 6 | ~400 | safety tests |
| 6: Benchmarks | 8 | ~600 | benchmark examples |
| 7: Demo & docs | 6 | ~400 | examples, docs, tests |
| **Total** | **78** | **~7,300** | **6 new modules** |

---

## Execution Schedule

```
Sprint 1 (1 session):  Phase 0 — Prerequisites (tensor ops, linalg, math)
Sprint 2 (1 session):  Phase 1 — TurboQuant baseline (faithful implementation)
Sprint 3 (1 session):  Phase 2 + 3 — Adaptive rotation + Fused attention
Sprint 4 (1 session):  Phase 4 + 5 — Hierarchical + Compiler integration
Sprint 5 (1 session):  Phase 6 + 7 — Benchmarks + Demos + Paper outline
```

---

## Theoretical Guarantees (What the Paper Would Prove)

### Theorem (FajarQuant_adaptive MSE bound)

For any vector x in R^d where x lies in a k-dimensional subspace (k << d),
and the adaptive rotation Pi_h is computed from PCA of the data distribution,
the MSE distortion satisfies:

```
D_mse(FajarQuant_adaptive) <= C(f_Y, b) * (sum_{i=1}^{d} lambda_i)
```

where lambda_i are the eigenvalues of the data covariance and f_Y is the
marginal distribution of rotated coordinates. When the data is low-rank
(lambda_i decays quickly), this is STRICTLY LESS than TurboQuant's bound
of C(f_X, b) * d, since sum(lambda_i) < d for non-isotropic distributions.

### Theorem (Fused attention correctness)

The fused quantized attention output equals the standard
dequantize-then-compute attention output exactly (no additional error):

```
fused_attention(Q, K_quant, V_quant, codebook) 
  = standard_attention(Q, dequant(K_quant), dequant(V_quant))
```

This is because codebook lookup is commutative with linear operations.

### Theorem (Hierarchical optimality)

Given a fixed total bit budget B and token importance weights w_i,
the optimal bit allocation that minimizes weighted distortion:

```
min sum_i w_i * D_mse(b_i)   subject to   sum_i b_i <= B
```

is solved by water-filling: allocate more bits to tokens with higher
importance weight, with the allocation b_i proportional to log(w_i).

---

## Risk Mitigation

| Risk | Impact | Mitigation |
|------|--------|------------|
| ndarray-linalg LAPACK dependency | Phase 0 blocks | Gram-Schmidt fallback (no LAPACK needed) |
| PCA calibration overhead | Phase 2 slower than random | Only ~100 vectors needed, amortized over sequence |
| Fused attention numerical precision | Phase 3 accuracy | Kahan summation for accumulation |
| Codebook doesn't fit L1 cache | Phase 3 slower | For b<=4, codebook is 16 entries * 8 bytes = 128 bytes (always fits) |
| Hierarchical re-quantization cost | Phase 4 overhead | Only re-quantize on tier boundary crossings |

---

## Success Criteria

1. **TurboQuant parity:** Match paper's MSE/IP distortion bounds at b=1,2,3,4
2. **Adaptive improvement:** MSE distortion at least 30% lower than TurboQuant on structured data
3. **Fused speedup:** At least 2x throughput improvement on quantized attention
4. **Hierarchical savings:** Same quality at 20% fewer total bits
5. **Safety guarantees:** All @kernel/@device violations caught at compile time
6. **All examples run:** `fj run examples/fajarquant_*.fj` produces correct output
7. **Paper-ready:** Theorems stated, experiments reproducible, figures generated

---

*FajarQuant E2E Plan — 78 tasks, 7 phases, ~7,300 LOC*
*"The first vector quantization system with compile-time safety guarantees and hardware-aware dispatch."*
*Built on Fajar Lang V22 "Quantum" — where an OS kernel and a neural network share the same type system.*
