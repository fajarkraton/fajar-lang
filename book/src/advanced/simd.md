# SIMD & Vectorization

Fajar Lang provides both manual SIMD intrinsics and automatic vectorization for high-performance numerical code.

## SIMD Vector Types

```fajar
let a: f32x4 = f32x4(1.0, 2.0, 3.0, 4.0)
let b: f32x4 = f32x4(5.0, 6.0, 7.0, 8.0)
let c = a + b  // f32x4(6.0, 8.0, 10.0, 12.0) — lane-wise addition
```

| Type | Lanes | Width | Extension |
|------|-------|-------|-----------|
| `i32x4` / `f32x4` | 4 | 128-bit | SSE / NEON |
| `i32x8` / `f32x8` | 8 | 256-bit | AVX2 |
| `f64x2` | 2 | 128-bit | SSE2 |
| `f64x4` | 4 | 256-bit | AVX |
| `i8x16` | 16 | 128-bit | SSE2 / NEON |
| `i16x8` | 8 | 128-bit | SSE2 / NEON |

## Operations

```fajar
let sum = a + b          // lane-wise add
let prod = a * b         // lane-wise multiply
let cmp = a > b          // lane-wise comparison (returns mask)
let shuf = shuffle(a, b, [0, 4, 1, 5])  // interleave
let reduced = horizontal_sum(a)  // sum all lanes → scalar
```

## Platform Intrinsics

```fajar
// x86 SSE
let r = mm_add_ps(a, b)
let r = mm_mul_ps(a, b)
let r = mm_shuffle_ps(a, b, 0x1B)

// x86 AVX
let r = mm256_add_ps(a, b)
let r = mm256_fmadd_ps(a, b, c)  // fused multiply-add

// x86 AVX-512
let r = mm512_add_ps(a, b)
let r = mm512_mask_add_ps(src, mask, a, b)  // masked operation

// ARM NEON
let r = vaddq_f32(a, b)
let r = vmulq_f32(a, b)

// ARM SVE (scalable vectors)
let r = sve_add_f32_pred(pred, a, b)

// RISC-V V
let r = rvv_fadd(a, b)
```

## Auto-Vectorization

The compiler can automatically vectorize loops:

```fajar
@simd
fn add_arrays(a: [f32], b: [f32], result: [f32]) {
    let mut i = 0
    while i < len(a) {
        result[i] = a[i] + b[i]
        // Compiler vectorizes to f32x8 on AVX2
        i = i + 1
    }
}
```

The `@simd` annotation hints the compiler to vectorize. The auto-vectorizer analyzes:
- Loop trip count
- Memory access patterns (stride, alignment)
- Data dependencies
- Cost model (is SIMD actually faster?)

## SIMD for Tensors

Built-in SIMD-accelerated tensor operations:

```fajar
// 4x4 matrix multiply — uses SIMD internally
let c = matmul(a, b)

// Elementwise operations — auto-vectorized
let activated = relu(tensor)
let normalized = softmax(tensor)

// Dot product — uses horizontal sum
let similarity = dot(vec_a, vec_b)

// Quantization — INT8 packing with SIMD
let quantized = quantize_int8(float_tensor, scale, zero_point)
```

## Runtime Detection

SIMD capability is detected at runtime:

```fajar
let caps = SimdCapability::detect()
if caps.has_avx512 {
    avx512_path(data)
} else if caps.has_avx2 {
    avx2_path(data)
} else {
    scalar_path(data)
}
```
