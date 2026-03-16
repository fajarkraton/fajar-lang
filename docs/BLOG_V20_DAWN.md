# Fajar Lang on Radxa Dragon Q6A: Embedded ML Meets Real Hardware

> v2.0 "Dawn" — The first physical hardware deployment of Fajar Lang

---

## Introduction

Fajar Lang is a statically-typed systems programming language designed for embedded ML and OS integration. With v2.0 "Dawn", we deployed it for the first time on real hardware: the **Radxa Dragon Q6A**, a Qualcomm QCS6490-based edge AI single-board computer.

This post covers what we built, what we learned, and what's next.

## The Hardware

The Dragon Q6A packs serious compute into a compact form factor:

- **CPU**: Kryo 670 (1x A78@2.7GHz + 3x A78@2.4GHz + 4x A55@1.9GHz)
- **GPU**: Adreno 643 @ 812MHz (OpenCL 3.0, ~773 GFLOPS FP32)
- **NPU**: Hexagon 770 V68 (12 TOPS INT8)
- **RAM**: LPDDR5 up to 16GB
- **GPIO**: 40-pin header (7 UART, 6 I2C, 7 SPI, I2S, I3C)

At ~$60-$125 depending on configuration, it's one of the most capable edge AI boards available.

## What We Achieved

### Cross-Compilation Pipeline

Fajar Lang cross-compiles from x86_64 to ARM64 in under a minute:

```bash
cargo build --release --target aarch64-unknown-linux-gnu
scp target/aarch64.../release/fj radxa@192.168.100.2:/usr/local/bin/
```

The resulting 6.8MB binary runs all 106 example programs natively on the Q6A.

### GPU Compute via OpenCL

We added 7 GPU-accelerated tensor builtins that automatically use the Adreno GPU when available:

```fajar
let a = tensor_randn(256, 256)
let b = tensor_randn(256, 256)
let c = gpu_matmul(a, b)    // 4x faster than CPU at 256x256
let h = gpu_relu(c)          // element-wise activation on GPU
```

On x86_64 development machines, the same code falls back to CPU tensor operations — zero code changes needed.

### QNN SDK Integration

We integrated Qualcomm's QNN SDK for NPU inference:

```fajar
let input = tensor_randn(1, 256)
let qbuf = qnn_quantize(input, "f32")    // quantize for QNN
let result = qnn_dequantize(qbuf)         // dequantize back
```

The QNN GPU backend works out of the box (no device signing required). The HTP/DSP backend requires a Qualcomm test signature for unsigned code.

### 55 Hardware-Specific Examples

We created 55 Q6A-specific examples covering:

- **Sensor fusion**: IMU complementary filter, predictive maintenance
- **Edge AI**: Object detection, activity recognition, anomaly detection
- **Video processing**: H.264/H.265 encode/decode, RTSP streaming, multi-camera
- **Production**: Fleet management, A/B testing, batch scheduling
- **GPU training**: On-device neural network training with SGD/Adam

All 55 examples verified on real Q6A hardware via SSH.

### Performance Numbers

| Benchmark | Result |
|-----------|--------|
| fib(30) native vs interpreted | 128x speedup |
| GPU matmul 256x256 | 4.0x vs CPU |
| GPU matmul 512x512 | 6.8x vs CPU |
| All 5,147 tests | PASS (0 failures) |
| Cross-compile time | 51 seconds |
| Binary size | 6.8 MB stripped |

## Lessons Learned

1. **Adreno GPU naming is confusing**: OpenCL reports "635", QNN says "642", spec says "643". Same GPU.
2. **QNN HTP requires signing**: Unlike GPU/CPU backends, the Hexagon DSP won't run unsigned code.
3. **Unified memory is a win**: No CPU-GPU data transfer overhead on mobile SoCs.
4. **ARM64 NEON auto-vectorization works**: ndarray gets SIMD for free on aarch64.

## What's Next

The remaining 20% of v2.0 tasks are mostly hardware-blocked:
- Camera module (IMX219) for the full camera->NPU->GPIO pipeline
- I2C/SPI sensors for real peripheral testing
- ~~Vulkan compute (blocked by system loader version)~~ — **FIXED:** Mesa Turnip 25.2.8 installed, Vulkan 1.3 working

After v2.0, **FajarOS v3.0 "Surya"** will bring a full OS written in Fajar Lang, targeting the Dragon Q6A as the reference platform.

---

*Fajar Lang v2.0 "Dawn" | March 2026 | PrimeCore.id*
