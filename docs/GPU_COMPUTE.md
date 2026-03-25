# Fajar Lang — GPU Compute Backend

> Hardware-accelerated tensor operations via wgpu (Vulkan/Metal/DX12).

---

## Architecture

```
Fajar Lang Program
    │
    ├── @device fn classify(img: Tensor) → dispatched to GPU
    │       │
    │       ├── wgpu backend (cross-platform)
    │       │     Vulkan (Linux/Windows), Metal (macOS), DX12 (Windows)
    │       │
    │       ├── Vulkan/ash backend (bare-metal, Q6A Adreno)
    │       │
    │       ├── CUDA backend (NVIDIA GPUs)
    │       │
    │       └── CPU fallback (always available)
    │
    └── Auto-dispatch: GPU if tensor > 1024 elements, CPU otherwise
```

## Backends

| Backend | Feature Flag | Platforms | GPU |
|---------|-------------|-----------|-----|
| wgpu | `--features gpu` | All | Any wgpu-compatible |
| Vulkan/ash | `--features vulkan` | Linux, ARM64 | Adreno, Mali, NVIDIA |
| CUDA | `--features cuda` | Linux, Windows | NVIDIA only |
| CPU fallback | (always on) | All | N/A |

## WGSL Compute Shaders

8 pre-built compute shaders in `shaders/`:

| Shader | File | Workgroup | Operation |
|--------|------|-----------|-----------|
| Vector Add | `vecadd.wgsl` | 256 | C[i] = A[i] + B[i] |
| Matrix Multiply | `matmul.wgsl` | 16x16 | C = A * B (tiled) |
| ReLU | `relu.wgsl` | 256 | max(0, x) |
| Sigmoid | `sigmoid.wgsl` | 256 | 1/(1+exp(-x)) |
| Softmax | `softmax.wgsl` | 1 | exp(x-max)/sum |
| Transpose | `transpose.wgsl` | 16x16 | B[j][i] = A[i][j] |
| Scale | `scale.wgsl` | 256 | y = x * scalar |
| Conv2D | `conv2d.wgsl` | 16x16 | 2D convolution |

## Rust Implementation

| File | Lines | Purpose |
|------|-------|---------|
| `runtime/gpu/wgpu_backend.rs` | 794 | wgpu device, buffer, pipeline |
| `runtime/gpu/cpu_fallback.rs` | 384 | CPU reference implementation |
| `runtime/gpu/tensor_bridge.rs` | 333 | Tensor ↔ GPU buffer |
| `runtime/gpu/cuda_backend.rs` | 270 | CUDA simulation |
| `runtime/gpu/kernel.rs` | 200 | Kernel abstractions |
| `runtime/gpu/buffer.rs` | 118 | Buffer management |
| `runtime/gpu/device.rs` | 117 | Device trait |
| `runtime/gpu/mod.rs` | 140 | Module declarations |
| `bsp/dragon_q6a/vulkan.rs` | 2,202 | Vulkan on Adreno 643 |
| **Total** | **4,558** | |

## Usage

```bash
# Build with GPU support
cargo build --release --features gpu

# Check GPU availability
fj gpu-info

# Run with GPU acceleration
fj run --gpu examples/mnist_training.fj
```

## Auto-Dispatch Policy

| Tensor Size | Backend | Reason |
|-------------|---------|--------|
| < 1,024 elements | CPU | GPU dispatch overhead > compute time |
| >= 1,024 elements | GPU | GPU parallelism wins |
| Not available | CPU | Graceful fallback |

---

*GPU Compute Backend — Fajar Lang v6.1.0*
*8 WGSL shaders, 4,558 lines Rust, wgpu + Vulkan + CUDA + CPU*
