# FajarQuant v2.12 — Fajar Lang Language Support Design (Phase B5)

> **Purpose:** Detailed designs for L1-L7 language-level features that
> give FajarQuant v2 a unique "language-integrated quantization" angle.
> No other KV quantization method runs natively in a systems programming
> language with compile-time safety guarantees.
>
> **Prerequisites:** B4.G GO decision (v2 algorithm works). L1-L7 can be
> implemented in parallel with B3-B4 as long as the API surface is
> stable. Each L-task is independent and can be landed incrementally.

---

## L1: Native Quantized Tensor Types

### Design

```fajar
// New built-in generic struct in std::nn::quant
struct Quantized<T: Numeric, const BITS: usize> {
    data: Tensor<u8>,              // packed bit storage
    scale: Tensor<T>,              // per-channel scale factors
    zero_point: Tensor<T>,         // per-channel zero points
    original_shape: [usize; 4],    // (B, H, S, D) for KV cache
    bit_width: usize,              // redundant with BITS but useful for runtime dispatch
}

// Constructor
fn quantize<T: Float, const BITS: usize>(
    tensor: Tensor<T>,
    method: QuantMethod,
) -> Quantized<T, BITS> { ... }

// Dequantize
fn dequantize<T: Float, const BITS: usize>(
    q: &Quantized<T, BITS>
) -> Tensor<T> { ... }

// Compile-time safety: using a Quantized<T, B> in a float matmul
// without dequantize is a SE017 error:
//   SE017 QuantizedNotDequantized: cannot use Quantized<f16, 2> where
//   Tensor<f16> is expected. Call dequantize() first.

// Bit-width-aware operations (native, no dequant needed):
fn quantized_add<T, const B: usize>(
    a: &Quantized<T, B>,
    b: &Quantized<T, B>,
) -> Quantized<T, B> { ... }  // add in quantized domain
```

### Analyzer rule SE017

```
SE017 QuantizedNotDequantized
  When: a Quantized<T, B> value appears where Tensor<T> is expected
  Level: Error (blocks compilation)
  Fix: wrap in dequantize(q)
  Rationale: prevents accidental use of packed u8 data as float values,
             which would produce garbage numerics
```

### Implementation scope

| Component | Changes | LOC est |
|---|---|---|
| `src/analyzer/types.rs` | New `TypeKind::Quantized` variant with BITS parameter | ~50 |
| `src/analyzer/type_check.rs` | SE017 check on type mismatch | ~30 |
| `src/interpreter/eval/builtins.rs` | `quantize()` + `dequantize()` builtins | ~100 |
| `src/runtime/ml/mod.rs` | Wire builtins to runtime | ~20 |
| `docs/ERROR_CODES.md` | Add SE017 entry | ~10 |
| `tests/integration/quant_type_safety.rs` | 10+ tests for SE017 + quantize/dequantize roundtrip | ~100 |
| `examples/quantized_tensor.fj` | Demo program | ~30 |
| **Total** | | **~340 LOC** |

---

## L2: Hadamard Transform Builtin

### Design

```fajar
// New builtin in std::nn namespace
// Compile-time constraint: D must be a power of 2
fn hadamard<const D: usize>(x: Tensor<f32, _, D>) -> Tensor<f32, _, D>
where D.is_power_of_two()
{
    // Internally: recursive butterfly factorization
    // H_n = [[H_{n/2}, H_{n/2}], [H_{n/2}, -H_{n/2}]] / sqrt(2)
    // Applied via fast Walsh-Hadamard transform (FWHT) in O(D log D)
    // No memory allocation beyond the output tensor
}

// Inverse (Hadamard is self-inverse up to scaling)
fn hadamard_inverse<const D: usize>(x: Tensor<f32, _, D>) -> Tensor<f32, _, D>
where D.is_power_of_two()
{
    hadamard(x)  // H^{-1} = H / D (self-adjoint)
}

// Usage in FajarQuant v2:
@device
fn quantize_value(v: Tensor<f32, S, D>) -> Quantized<f16, 2> {
    let rotated = nn::hadamard(v)          // compile-time D power-of-2 check
    let quant = nn::quantize(rotated, QuantMethod::PerCoord)
    quant
}
```

### Implementation — AVX2 butterfly (Cranelift + LLVM backends)

```
// Pseudo-LLVM IR for D=128 FWHT butterfly stage
// Each stage: for pairs (i, i+stride): temp = a[i], a[i] = temp + a[i+stride], a[i+stride] = temp - a[i+stride]
// 7 stages for D=128 (log2(128) = 7)
// Each stage operates on 128 elements, 4 at a time with AVX2 ymm registers
```

### Scope

| Component | Changes | LOC est |
|---|---|---|
| `src/interpreter/eval/builtins.rs` | `nn_hadamard` impl via ndarray (interpreter path) | ~60 |
| `src/codegen/cranelift/ml_ops.rs` | FWHT butterfly codegen (Cranelift intrinsic) | ~150 |
| `src/codegen/llvm/ml_ops.rs` | FWHT with AVX2 inline asm | ~200 |
| `src/analyzer/type_check.rs` | Power-of-two compile-time check | ~20 |
| `benches/hadamard_simd.rs` | Benchmark FWHT vs scalar, verify ≥2x speedup | ~80 |
| `examples/hadamard_demo.fj` | Demo | ~20 |
| **Total** | | **~530 LOC** |

---

## L3: Compile-Time Calibrated Rotation Matrices

### Design

```fajar
// Macro/builtin that loads binary file at compile time
// File format: raw f32 little-endian, shape (D, D)
const ROTATION_LAYER_0: Tensor<f32, 256, 256> =
    include_calibration!("data/calibration/fq_v2_gemma/rotation_layer_0.bin")

// Optional orthogonality verification at build time
const_assert!(nn::is_orthogonal(ROTATION_LAYER_0, tolerance: 1e-5))

// Usage:
@device
fn quantize_key_layer_0(k: Tensor<f32, S, 256>) -> Quantized<f16, 2> {
    let centered = k - ROTATION_LAYER_0_MEAN
    let rotated = nn::matmul(centered, ROTATION_LAYER_0.transpose())
    nn::quantize(rotated, QuantMethod::PerCoord)
}
```

### Scope

| Component | Changes | LOC est |
|---|---|---|
| `src/parser/macros.rs` | `include_calibration!` macro parsing | ~50 |
| `src/analyzer/const_eval.rs` | Load + shape-check at compile time | ~80 |
| `src/interpreter/eval/builtins.rs` | Runtime fallback loader | ~30 |
| `src/analyzer/const_eval.rs` | `is_orthogonal()` const fn | ~40 |
| **Total** | | **~200 LOC** |

---

## L4: @device fn Quantization Kernels (existing feature)

### Design

```fajar
// @device context already exists — compiler enforces:
// - No heap allocation
// - No raw pointer dereference outside @kernel
// - No tensor creation (only operate on passed-in tensors)

@device
fn fq_v2_quantize_kv(
    k: &mut Tensor<f32, S, D>,
    v: &mut Tensor<f32, S, D>,
    rotation_k: &Tensor<f32, D, D>,
    rotation_v: &Tensor<f32, D, D>,
    bits: usize,
) {
    // In-place quantization — no allocation
    nn::rotate_inplace(k, rotation_k)
    nn::quantize_inplace(k, bits, QuantMethod::PerCoord)
    nn::inverse_rotate_inplace(k, rotation_k)

    nn::rotate_inplace(v, rotation_v)
    nn::quantize_inplace(v, bits, QuantMethod::PerCoord)
    nn::inverse_rotate_inplace(v, rotation_v)
}
```

### Scope

No new Fajar Lang features needed — @device already works. Just need to
write the actual .fj code using existing builtins.

| Component | Changes | LOC est |
|---|---|---|
| `fajaros-x86/kernel/compute/fajarquant_v2.fj` | New file with v2 algorithm | ~300 |
| `tests/integration/fajarquant_v2_device.rs` | @device context validation tests | ~80 |
| **Total** | | **~380 LOC** |

---

## L5: AVX2/AES-NI Inline Assembly for Hot Paths

### Design

```fajar
// Already supported via FFI v2 (V18) + inline asm (V24)
// Target: FWHT butterfly + per-coord quantization in a single fused kernel

@device
@unsafe  // inline asm requires @unsafe context
fn fwht_quantize_avx2(
    data: &mut [f32; 128],   // exactly 128 elements (head_dim)
    bits: usize,
    scale: &[f32; 128],
    zero: &[f32; 128],
) {
    // Stage 1-7: FWHT butterfly via AVX2 vaddps/vsubps
    asm! {
        "vmovaps ymm0, [{data}]"         // load 8 floats
        "vmovaps ymm1, [{data} + 32]"    // load next 8
        "vaddps  ymm2, ymm0, ymm1"       // a + b
        "vsubps  ymm3, ymm0, ymm1"       // a - b
        "vmovaps [{data}], ymm2"          // store
        "vmovaps [{data} + 32], ymm3"
        // ... 7 stages × 16 butterfly operations each
    }

    // Per-coord quantize via AVX2 vminps/vmaxps/vroundps
    asm! {
        "vmovaps ymm0, [{data}]"
        "vmovaps ymm4, [{scale}]"
        "vmovaps ymm5, [{zero}]"
        "vsubps  ymm0, ymm0, ymm5"       // data - zero
        "vdivps  ymm0, ymm0, ymm4"       // (data - zero) / scale
        "vroundps ymm0, ymm0, 0"         // round to nearest
        // clamp to [0, levels]
        "vmulps  ymm0, ymm0, ymm4"       // restore scale
        "vaddps  ymm0, ymm0, ymm5"       // restore zero
        "vmovaps [{data}], ymm0"
    }
}
```

### Scope

| Component | Changes | LOC est |
|---|---|---|
| `fajaros-x86/kernel/compute/fajarquant_v2.fj` | Inline asm hot paths | ~200 (added to L4 file) |
| `benches/fajarquant_v2_native.rs` | Benchmark native vs interpreter | ~100 |
| **Total** | | **~300 LOC** |

---

## L6: Compile-Time Shape Verification for Quantized Matmul

### Design

```fajar
// Generic quantized matmul with compile-time shape check
fn matmul_quantized<const M: usize, const N: usize, const K: usize>(
    q: Tensor<f16, M, K>,          // query
    kv_quant: Quantized<f16, 2>,   // quantized key cache
) -> Tensor<f16, M, N>
where
    kv_quant.original_shape[3] == K,  // head_dim matches
    kv_quant.original_shape[2] == N,  // seq_len matches
{
    let kv_fp16 = dequantize(kv_quant)  // explicit dequant
    nn::matmul(q, kv_fp16.transpose())
}

// Compiler error at call site if shapes mismatch:
//   SE018 ShapeMismatch: matmul_quantized expects kv_quant.original_shape[3] == 128
//   but got Quantized with original_shape[3] == 256
```

### Scope

| Component | Changes | LOC est |
|---|---|---|
| `src/analyzer/type_check.rs` | Const generic where clause on struct fields | ~60 |
| `src/analyzer/types.rs` | `TypeKind::QuantizedField` accessor | ~30 |
| `docs/ERROR_CODES.md` | Add SE018 ShapeMismatch for quantized | ~10 |
| **Total** | | **~100 LOC** |

---

## L7: Stack-Allocated QuantizedKVCache

### Design

```fajar
// Compile-time-sized KV cache for embedded deployment
// No heap allocation — lives on the stack or in a static memory region

struct QuantizedKVCache<
    const MAX_LEN: usize,
    const N_LAYERS: usize,
    const N_HEADS: usize,
    const HEAD_DIM: usize,
    const BITS: usize,
> {
    keys:   [[Quantized<f16, BITS>; N_HEADS]; N_LAYERS],
    values: [[Quantized<f16, BITS>; N_HEADS]; N_LAYERS],
    len: usize,    // current number of cached positions (≤ MAX_LEN)
}

impl<...> QuantizedKVCache<...> {
    fn new() -> Self {
        // Zero-initialize, no heap
        Self { keys: [[Quantized::empty(); N_HEADS]; N_LAYERS], ..., len: 0 }
    }

    fn update(
        &mut self,
        layer_idx: usize,
        new_keys: &[Quantized<f16, BITS>; N_HEADS],
        new_values: &[Quantized<f16, BITS>; N_HEADS],
    ) {
        assert!(self.len < MAX_LEN, "KV cache overflow at MAX_LEN={MAX_LEN}")
        // Copy new K/V into the next position, increment len
        // RAII: automatic cleanup when cache goes out of scope
    }

    fn get(&self, layer_idx: usize) -> (&[Quantized<f16, BITS>; N_HEADS],
                                          &[Quantized<f16, BITS>; N_HEADS]) {
        (&self.keys[layer_idx], &self.values[layer_idx])
    }
}

// Usage for SmolLM-135M on FajarOS:
const CACHE: QuantizedKVCache<2048, 12, 9, 64, 2> = QuantizedKVCache::new()

// Size at compile time: 12 layers × 2 (K+V) × 9 heads × 2048 tokens × 64 dim × 2 bits / 8
//                     = 12 × 2 × 9 × 2048 × 64 × 0.25 = ~7 MB
// Fits in L3 cache of i9-14900HX (36 MB) ← key perf insight
```

### Scope

| Component | Changes | LOC est |
|---|---|---|
| `src/analyzer/types.rs` | Multi-const-generic struct support | ~80 |
| `src/interpreter/eval/structs.rs` | Stack-allocated large array init | ~60 |
| `src/interpreter/eval/builtins.rs` | KV cache update/get builtins | ~80 |
| `src/runtime/os/memory.rs` | Stack frame size extension for large const arrays | ~40 |
| `examples/stack_kv_cache.fj` | Demo showing 0-heap KV cache | ~50 |
| `tests/integration/stack_kv_cache.rs` | RAII cleanup + overflow tests | ~60 |
| **Total** | | **~370 LOC** |

---

## Effort summary

| Task | LOC est | Time est | Priority |
|---|---|---|---|
| **L1** Quantized tensor types + SE017 | ~340 | 1.5 days | P0 (enables all others) |
| **L2** Hadamard builtin + AVX2 | ~530 | 2 days | P1 (v2 needs Hadamard) |
| **L3** Const calibration matrices | ~200 | 1.5 days | P1 (v2 needs calibration) |
| **L4** @device fn quant kernels | ~380 | 2 days | P2 (Fajar Lang showcase) |
| **L5** AVX2 inline asm hot paths | ~300 | 1 day | P2 (perf, needs L4) |
| **L6** Shape verification SE018 | ~100 | 1 day | P3 (nice to have) |
| **L7** Stack-allocated KV cache | ~370 | 1.5 days | P2 (embedded story) |
| **Total** | **~2,220 LOC** | **~10.5 days** | |

**Minimum viable set for paper §Embedded Deployment:** L1 + L2 + L4 = ~1,250 LOC, ~5.5 days.
**Full set for maximum paper impact:** all L1-L7 = ~2,220 LOC, ~10.5 days.

---

*Document version: 2026-04-12 v1.0. Implementation starts after B4.G GO decision.*
