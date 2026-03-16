# Dragon Q6A GPU Compute Guide

> GPU-accelerated tensor operations on the Radxa Dragon Q6A using Fajar Lang GPU builtins.

---

## Hardware: Adreno 643 GPU

| Property | Value |
|----------|-------|
| **GPU** | Qualcomm Adreno 643 (part of QCS6490 SoC) |
| **Clock** | 812 MHz max |
| **FP32 Performance** | ~773 GFLOPS |
| **API: OpenCL** | 3.0 (Adreno OpenCL driver) |
| **API: Vulkan** | 1.1 (BLOCKED — see below) |
| **Global Memory** | 3793 MB (shared with system LPDDR5) |
| **Max Workgroup Size** | 1024 |
| **Max Compute Units** | 2 |
| **Driver** | `/vendor/lib64/libOpenCL.so` |

### Vulkan Status: BLOCKED

Vulkan 1.1 is supported by the Adreno 643 hardware, but the system Vulkan loader version (v3) is incompatible with the required version (v5). Until the loader is updated via a system image refresh, Vulkan compute is unavailable. All GPU builtins use the OpenCL 3.0 path instead.

```
$ vulkaninfo 2>&1 | head -5
ERROR: [Loader Message] vkCreateInstance: Found no drivers!
# Loader version 3 cannot discover Adreno Vulkan ICD (requires v5)
```

---

## GPU Builtins

Fajar Lang provides 7 GPU-accelerated tensor builtins. On the Dragon Q6A, these use the Adreno 643 GPU via OpenCL 3.0. On systems without a GPU (or during development on x86_64), they automatically fall back to equivalent CPU tensor operations with identical results.

### Summary Table

| Builtin | Signature | Description |
|---------|-----------|-------------|
| `gpu_matmul` | `(a: Tensor, b: Tensor) -> Tensor` | Matrix multiplication |
| `gpu_add` | `(a: Tensor, b: Tensor) -> Tensor` | Element-wise addition |
| `gpu_mul` | `(a: Tensor, b: Tensor) -> Tensor` | Element-wise multiplication |
| `gpu_relu` | `(t: Tensor) -> Tensor` | ReLU activation (max(0, x)) |
| `gpu_sigmoid` | `(t: Tensor) -> Tensor` | Sigmoid activation (1/(1+e^-x)) |
| `gpu_transpose` | `(t: Tensor) -> Tensor` | Matrix transpose |
| `gpu_sum` | `(t: Tensor) -> f64` | Sum all elements (returns scalar) |

### Utility Builtins

| Builtin | Signature | Description |
|---------|-----------|-------------|
| `gpu_available()` | `() -> bool` | Check if GPU (OpenCL) is available |
| `gpu_info()` | `() -> str` | GPU device info string |

---

## Builtin Details

### `gpu_matmul(a, b) -> Tensor`

GPU-accelerated matrix multiplication. Computes `C = A @ B` where A is (M x K) and B is (K x N), producing C of shape (M x N).

```fajar
let a = tensor_randn(64, 128)
let b = tensor_randn(128, 32)
let c = gpu_matmul(a, b)
// c is shape 64x32
```

**Performance:** On Adreno 643, 256x256 matmul completes in ~2.1ms (vs ~8.5ms CPU).

### `gpu_add(a, b) -> Tensor`

Element-wise tensor addition. Both tensors must have the same shape. Supports broadcasting for bias addition (1 x N added to M x N).

```fajar
let x = tensor_randn(32, 16)
let bias = tensor_zeros(1, 16)
let y = gpu_add(x, bias)
```

### `gpu_mul(a, b) -> Tensor`

Element-wise (Hadamard) tensor multiplication. Both tensors must have the same shape.

```fajar
let a = tensor_randn(8, 8)
let b = tensor_randn(8, 8)
let c = gpu_mul(a, b)
```

### `gpu_relu(t) -> Tensor`

ReLU activation function applied element-wise: `max(0, x)`. Standard activation for hidden layers.

```fajar
let z = gpu_matmul(input, weights)
let h = gpu_relu(z)  // all negative values become 0
```

### `gpu_sigmoid(t) -> Tensor`

Sigmoid activation function applied element-wise: `1 / (1 + exp(-x))`. Output range is (0, 1). Useful for binary classification output layers.

```fajar
let logits = gpu_matmul(hidden, w_out)
let probs = gpu_sigmoid(logits)  // values in (0, 1)
```

### `gpu_transpose(t) -> Tensor`

Matrix transpose. Converts (M x N) to (N x M).

```fajar
let a = tensor_randn(3, 5)   // 3x5
let b = gpu_transpose(a)      // 5x3
```

### `gpu_sum(t) -> f64`

Sum all elements of a tensor, returning a scalar float value. Useful for loss monitoring during training.

```fajar
let t = tensor_ones(4, 4)
let s = gpu_sum(t)  // 16.0
```

---

## GPU Inference Pipeline

A typical inference pipeline using GPU builtins:

```fajar
// Load or initialize weights
let w1 = tensor_xavier(128, 64)
let w2 = tensor_xavier(64, 10)

// Input tensor (e.g., from sensor data)
let input = tensor_randn(1, 128)

// Forward pass — GPU-accelerated
let h = gpu_relu(gpu_matmul(input, w1))     // hidden layer
let logits = gpu_matmul(h, w2)               // output layer
let probs = tensor_softmax(logits)           // probabilities
let class = tensor_argmax(probs)             // predicted class

println(f"Predicted class: {class}")
```

---

## GPU Training Pipeline

On-device training uses GPU builtins for the forward pass combined with autograd for the backward pass:

```fajar
// Initialize weights with gradient tracking
let mut w1 = tensor_xavier(2, 8)
tensor_set_requires_grad(w1, true)
let mut w2 = tensor_xavier(8, 2)
tensor_set_requires_grad(w2, true)

// Create optimizer
let opt = optimizer_sgd(0.1, 0.9)

// Training loop
let mut epoch = 0
while epoch < 50 {
    let x = tensor_randn(1, 2)        // input
    let target = tensor_zeros(1, 2)     // target

    // Forward pass (GPU-accelerated)
    let h = gpu_relu(gpu_matmul(x, w1))
    let output = tensor_softmax(gpu_matmul(h, w2))

    // Loss + backward + optimizer step
    let loss_t = tensor_cross_entropy(output, target)
    tensor_backward(loss_t)
    w1 = optimizer_step(opt, w1)
    w2 = optimizer_step(opt, w2)
    w1 = optimizer_zero_grad(w1)
    w2 = optimizer_zero_grad(w2)

    epoch = epoch + 1
}
```

See `examples/q6a_gpu_train.fj` for a complete XOR classifier training example.

---

## CPU Fallback Behavior

All GPU builtins have transparent CPU fallback:

| Builtin | GPU Path (Q6A) | CPU Fallback |
|---------|---------------|--------------|
| `gpu_matmul` | OpenCL kernel | `tensor_matmul` (ndarray BLAS) |
| `gpu_add` | OpenCL kernel | `tensor_add` (ndarray) |
| `gpu_mul` | OpenCL kernel | `tensor_mul` (ndarray) |
| `gpu_relu` | OpenCL kernel | `tensor_relu` (element-wise) |
| `gpu_sigmoid` | OpenCL kernel | `tensor_sigmoid` (element-wise) |
| `gpu_transpose` | OpenCL kernel | `tensor_transpose` (ndarray) |
| `gpu_sum` | OpenCL reduce | `tensor_sum` (ndarray) |

This means the same `.fj` program runs correctly on both the Q6A board and a development laptop. The GPU path is selected automatically when `gpu_available()` returns `true`.

---

## Performance Benchmarks

Benchmarks measured on Dragon Q6A (Adreno 643, OpenCL 3.0) vs CPU (Kryo 670, 8 cores):

| Operation | Size | GPU (ms) | CPU (ms) | Speedup |
|-----------|------|----------|----------|---------|
| `gpu_matmul` | 32x32 | 0.8 | 1.2 | 1.5x |
| `gpu_matmul` | 64x64 | 1.2 | 3.1 | 2.6x |
| `gpu_matmul` | 128x128 | 1.8 | 6.9 | 3.8x |
| `gpu_matmul` | 256x256 | 2.1 | 8.5 | 4.0x |
| `gpu_matmul` | 512x512 | 4.7 | 32.1 | 6.8x |
| `gpu_relu` | 64x64 | 0.3 | 0.1 | 0.3x (overhead) |
| `gpu_relu` | 512x512 | 0.4 | 0.9 | 2.3x |
| `gpu_sigmoid` | 256x256 | 0.5 | 2.1 | 4.2x |
| `gpu_add` | 256x256 | 0.4 | 0.5 | 1.3x |
| `gpu_sum` | 512x512 | 0.3 | 0.4 | 1.3x |

**Key observations:**
- GPU is most beneficial for large matmul operations (4x+ speedup at 256x256 and above)
- For small tensors (< 64x64), CPU may be faster due to GPU dispatch overhead
- Element-wise operations show less speedup than matmul due to lower arithmetic intensity
- Memory transfer between CPU and GPU adds latency for small workloads

### Training Benchmark

| Model | Epochs | GPU (s) | CPU (s) | Speedup |
|-------|--------|---------|---------|---------|
| XOR (2→8→2) | 50 | 0.31 | 0.28 | 0.9x (overhead) |
| Iris (4→16→3) | 100 | 1.2 | 2.8 | 2.3x |
| MNIST-like (784→128→10) | 10 | 3.1 | 11.4 | 3.7x |

For small models like XOR, GPU overhead dominates. For larger models, GPU training shows significant speedup.

---

## Memory Management

The Adreno 643 shares system LPDDR5 memory (up to 3793 MB available for GPU). Key considerations:

- **No separate GPU memory:** Unified memory architecture means no explicit CPU→GPU transfers
- **Max allocation:** Single buffer limited to ~1.5 GB (driver constraint)
- **OOM protection:** GPU builtins gracefully fall back to CPU if GPU allocation fails
- **Best practice:** Keep batch sizes reasonable (< 256 for 512x512 tensors)

---

## Examples

| Example | Description |
|---------|-------------|
| `examples/q6a_gpu_matmul.fj` | Basic GPU matrix multiply and inference pipeline |
| `examples/q6a_gpu_bench.fj` | GPU vs CPU performance comparison |
| `examples/q6a_gpu_train.fj` | GPU-accelerated XOR classifier training |

---

## Troubleshooting

| Problem | Solution |
|---------|----------|
| `gpu_available()` returns `false` | Check `/vendor/lib64/libOpenCL.so` exists; run `clinfo` |
| GPU slower than CPU for small tensors | Expected — use CPU path for tensors < 64x64 |
| Vulkan not available | BLOCKED: loader v3 vs required v5. Use OpenCL path |
| OOM on large tensors | Reduce batch size; GPU shares system RAM |
| `gpu_matmul` shape mismatch | Inner dimensions must match: (M,K) @ (K,N) |

---

*Document: Q6A_GPU_COMPUTE.md | Sprint 15-18 (Phase 4: GPU Compute) | v2.0 "Dawn"*
