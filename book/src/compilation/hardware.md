# Hardware Detection

Fajar Lang detects hardware capabilities at runtime for optimal code path selection.

## CPU Detection

```fajar
use hw::cpu

let vendor = cpu::vendor()        // "GenuineIntel", "AuthenticAMD", "ARM"
let model = cpu::model_name()     // "Intel Core i9-13900K"

// Feature detection
if cpu::has_avx512() {
    simd_fast_path(data)          // Use AVX-512 instructions
} else if cpu::has_avx2() {
    simd_path(data)               // Fallback to AVX2
} else {
    scalar_path(data)             // Scalar fallback
}
```

### Detected Features

| Architecture | Features |
|-------------|----------|
| x86_64 | SSE, SSE2, SSE4.1, SSE4.2, AVX, AVX2, AVX-512, FMA, BMI2, AMX |
| ARM64 | NEON, SVE, SVE2, FP16, DotProd, BF16 |
| RISC-V | RVV (vector extension), Zba, Zbb |

## GPU Discovery

```fajar
use hw::gpu

let gpus = gpu::enumerate()
for g in gpus {
    println(f"GPU: {g.name}, VRAM: {g.vram_mb}MB, Compute: {g.compute_capability}")
}

// Example output:
// GPU: NVIDIA RTX 4090, VRAM: 24576MB, Compute: 8.9
```

Supported APIs: CUDA Driver API, Vulkan.

## NPU Detection

```fajar
use hw::npu

let npus = npu::enumerate()
// Detects: Intel VPU, AMD XDNA, Qualcomm Hexagon, Apple ANE
```

## Accelerator Registry

All detected hardware is collected into a unified registry:

```fajar
use accelerator::dispatch

let registry = AcceleratorRegistry::detect()

// Score accelerators for a workload
let best = registry.best_for(Workload::MatMul { m: 1024, n: 1024, k: 1024 })
// Returns: GPU (if available) > NPU > CPU

// Fallback chain
let chain = registry.fallback_chain(Workload::Inference)
// [GPU, NPU, CPU] — tries each in order
```

## Multi-Accelerator Dispatch

The `@infer` context automatically selects the best accelerator:

```fajar
@infer
fn classify(input: Tensor) -> i64 {
    // Compiler automatically dispatches to:
    // - GPU if large batch
    // - NPU if INT8 quantized
    // - CPU as fallback
    let output = forward(input, model)
    argmax(output)
}
```

### Cost Model

Dispatch decisions consider:
- **Compute intensity** — FLOPS required
- **Memory bandwidth** — data transfer cost (H2D/D2H)
- **Latency sensitivity** — whether milliseconds matter
- **Power budget** — battery vs. wall power
- **Data location** — avoid unnecessary transfers

## Runtime Profiling

```fajar
use accelerator::profiler

let profiler = AcceleratorProfiler::new()
profiler.start()

// ... run workloads ...

let report = profiler.report()
// Shows: time per accelerator, utilization %, data transfer overhead
```
