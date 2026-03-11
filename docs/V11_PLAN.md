# Fajar Lang v1.1 "Ascension" — Implementation Plan

> **Focus:** Real hardware acceleration + ecosystem infrastructure
> **Timeline:** 40 sprints, ~400 tasks across 10 phases
> **Prerequisite:** v1.0.0 "Genesis" COMPLETE (3,392 tests, ~194K LOC)
> **Target release:** 2026 Q3

---

## Motivation

v1.0.0 "Genesis" established Fajar Lang as a complete systems programming language with an interpreter, bytecode VM, Cranelift native backend, ML runtime, OS primitives, concurrency, and a self-hosting bootstrap. But every piece of hardware acceleration — GPU compute, tensor cores, NPU dispatch — was simulated or stub-based. The compiler targets real ISAs but has never driven a real accelerator. The package ecosystem exists in code but has no live registry. The documentation is thorough but lives only on disk.

v1.1 "Ascension" makes it **REAL**:

1. **Real hardware detection** — probe CPUID, enumerate CUDA devices, discover NPUs at runtime.
2. **Real numeric formats** — FP4, FP8, BF16, structured sparsity — the formats that modern AI silicon actually uses.
3. **Real accelerator dispatch** — compile and run inference on Intel NPUs, AMD XDNA, and NVIDIA Blackwell Tensor Cores.
4. **Real deployment** — CI/CD pipelines, binary releases, a live package registry, a browser playground.
5. **Real demos** — drone firmware on Jetson Thor, MNIST on a physical GPU, an OS kernel booting on QEMU.

The gap between "it compiles" and "it runs on hardware" is the gap v1.1 closes. After Ascension, Fajar Lang is not a proof of concept — it is a deployable tool.

---

## Phase Overview

| Phase | Name | Sprints | Tasks | Focus |
|-------|------|---------|-------|-------|
| 1 | Hardware Detection & Runtime | S1-S4 | 40 | Platform detection, CPU feature probing (AVX-512, AMX, NPU, CUDA), runtime accelerator selection |
| 2 | Modern Numeric Formats | S5-S8 | 40 | FP4, FP8 (E5M2/E4M3), BF16, 4:2 structured sparsity in tensor runtime |
| 3 | NPU Integration | S9-S12 | 40 | @npu context, Intel OpenVINO + AMD XDNA dispatch, ONNX-to-NPU pipeline |
| 4 | Jetson Thor BSP | S13-S16 | 40 | T4000/T5000 support, sm_101, CUDA 13.0, MIG partitioning, JetPack 7.1 |
| 5 | AVX-512 / AMX / Blackwell Codegen | S17-S20 | 40 | Intel AMX tile ops, AVX10.2 codegen, NVIDIA tcgen05.mma PTX, APX 32-GPR |
| 6 | CI/CD & Binary Distribution | S21-S24 | 40 | GitHub Actions (test + cross-compile), binary releases per platform, OSS-Fuzz |
| 7 | Package Registry & Website | S25-S28 | 40 | registry.fajarlang.dev (Cloudflare Workers+D1), fajarlang.dev landing page |
| 8 | Online Playground | S29-S32 | 40 | Compile .fj in browser via Wasm, share snippets, embedded in docs |
| 9 | Multi-Accelerator Dispatch | S33-S36 | 40 | @infer auto-dispatch: CPU-to-NPU-to-GPU fallback, heterogeneous execution |
| 10 | Real-World Demos | S37-S40 | 40 | Drone firmware on Jetson Thor, MNIST on real GPU, OS kernel on QEMU |

**Total: 40 sprints, 400 tasks**

---

## Phase 1: Hardware Detection & Runtime (S1-S4)

> Detect what the machine actually has — CPUs, GPUs, NPUs — and build a unified accelerator registry.

### Sprint S1 — CPU Feature Detection

- [x] S1.1 — CPUID Wrapper: Implement safe Rust wrapper around x86 CPUID instruction with leaf/subleaf enumeration
- [x] S1.2 — AVX-512 Detection: Detect AVX-512F, AVX-512BW, AVX-512VNNI, AVX-512BF16 via CPUID leaf 7 and XCR0
- [x] S1.3 — AMX Detection: Detect Intel AMX-BF16, AMX-INT8, AMX-FP16 via CPUID.07H:EDX bits and XSAVE support
- [x] S1.4 — SSE/AVX Baseline: Detect SSE4.2, AVX2, FMA3 as minimum SIMD capabilities for fallback paths
- [x] S1.5 — Feature Flags Struct: Define `CpuFeatures` struct with bitfield storage for all detected ISA extensions
- [x] S1.6 — ARM64 Feature Detection: Read AARCH64 ID_AA64ISAR0_EL1/ID_AA64PFR0_EL1 for SVE, SVE2, SME, dotprod
- [x] S1.7 — RISC-V Feature Detection: Parse RISC-V ISA string for V (vector), Zfh (half-float), Zvfh extensions
- [x] S1.8 — Runtime Cache: Cache detected features in thread-local static; lazy_static initialization on first query
- [x] S1.9 — Feature Query API: Expose `hw::cpu::has_avx512()`, `hw::cpu::has_amx()`, etc. as Fajar Lang builtins
- [x] S1.10 — Unit Tests: 10+ tests covering feature detection on current platform with mock CPUID for cross-platform

### Sprint S2 — GPU Discovery

- [x] S2.1 — CUDA Device Enumeration: Call cuDeviceGetCount/cuDeviceGet via CUDA driver API to list all GPUs
- [x] S2.2 — Compute Capability Query: Retrieve sm_XX version per device (sm_89 for Ada, sm_100/sm_101 for Blackwell)
- [x] S2.3 — Memory Query: Query total/free VRAM via cuMemGetInfo, report per-device memory in GpuDevice struct
- [x] S2.4 — Multi-GPU Topology: Detect NVLink/PCIe connectivity between GPUs via cuDeviceGetP2PAttribute
- [x] S2.5 — GPU Feature Struct: Define `GpuDevice { id, name, compute_cap, vram_total, vram_free, bus_type, tensor_cores }`
- [x] S2.6 — Tensor Core Detection: Identify Tensor Core generation (1st-5th) from compute capability mapping
- [x] S2.7 — Driver Version Check: Query CUDA driver version, validate minimum required version (12.0+), warn on mismatch
- [x] S2.8 — Fallback Without CUDA: Graceful degradation when libcuda.so is not found — log warning, GPU list empty
- [x] S2.9 — GPU Info Display: Format GPU details for `fj --hw-info` output (name, CC, VRAM, Tensor Core gen)
- [x] S2.10 — Unit Tests: Test GPU enumeration on systems with/without CUDA, mock driver for CI environments

### Sprint S3 — NPU Detection

- [x] S3.1 — Intel NPU Discovery: Detect Intel Meteor Lake / Lunar Lake NPU via /sys/class/accel or OpenVINO API
- [x] S3.2 — AMD XDNA Discovery: Detect AMD XDNA 2 NPU via amdxdna kernel driver in /dev/accel/accel*
- [x] S3.3 — Capability Negotiation: Query NPU supported operations (INT8, FP16, BF16) and max batch size
- [x] S3.4 — TOPS Reporting: Calculate and report peak TOPS (tera operations per second) for each detected NPU
- [x] S3.5 — NPU Feature Struct: Define `NpuDevice { vendor, model, tops, supported_dtypes, max_batch, driver_version }`
- [x] S3.6 — Qualcomm Hexagon Stub: Stub detection for Qualcomm Hexagon DSP (Snapdragon X Elite) for future support
- [x] S3.7 — Apple ANE Stub: Stub detection for Apple Neural Engine via IOKit for macOS (placeholder, not fully implemented)
- [x] S3.8 — NPU Health Check: Verify NPU is responsive with a trivial inference test (identity matrix multiply)
- [x] S3.9 — NPU Info Display: Format NPU details for `fj --hw-info` output (vendor, TOPS, dtypes)
- [x] S3.10 — Unit Tests: Test NPU detection with mock sysfs/driver, test graceful absence on non-NPU machines

### Sprint S4 — Accelerator Registry

- [x] S4.1 — HardwareProfile Struct: Unified `HardwareProfile { cpus: Vec<CpuFeatures>, gpus: Vec<GpuDevice>, npus: Vec<NpuDevice> }`
- [x] S4.2 — Accelerator Ranking: Score each accelerator by TOPS/FLOPS, sort by capability for dispatch priority
- [x] S4.3 — Fallback Chain: Define CPU-only → NPU → GPU fallback chain with user-overridable priority config
- [x] S4.4 — Profile Serialization: Serialize HardwareProfile to JSON for caching and remote reporting
- [x] S4.5 — CLI Command `fj --hw-info`: Implement CLI flag that prints full hardware profile in human-readable format
- [x] S4.6 — CLI Command `fj --hw-json`: Implement CLI flag that outputs hardware profile as machine-readable JSON
- [x] S4.7 — Accelerator Selection API: `hw::select_best(task_type) -> Accelerator` returns optimal device for workload
- [x] S4.8 — Environment Override: `FJ_ACCELERATOR=cpu|npu|gpu` env var to force specific accelerator in dispatch
- [x] S4.9 — Integration with Interpreter: Wire HardwareProfile into Interpreter/Compiler context for runtime dispatch
- [x] S4.10 — Integration Tests: End-to-end test: detect hardware, serialize profile, select accelerator, verify fallback

---

## Phase 2: Modern Numeric Formats (S5-S8)

> Implement the numeric types that modern AI silicon actually computes in.

### Sprint S5 — FP8 Types

- [x] S5.1 — E5M2 Format: Implement 8-bit float with 5-bit exponent, 2-bit mantissa (IEEE 754 proposal), bit layout
- [x] S5.2 — E4M3 Format: Implement 8-bit float with 4-bit exponent, 3-bit mantissa, NaN handling (single NaN encoding)
- [x] S5.3 — FP8 Arithmetic: Add, subtract, multiply, divide for E5M2 and E4M3 with correct rounding (RNE)
- [x] S5.4 — FP8-to-F32 Conversion: Lossless upcast from E5M2/E4M3 to f32 via exponent bias adjustment
- [x] S5.5 — F32-to-FP8 Conversion: Downcasting with saturation, rounding modes (RNE, stochastic rounding)
- [x] S5.6 — FP8 Tensor Integration: Extend TensorValue to hold FP8 data, storage as packed u8 arrays
- [x] S5.7 — FP8 Quantization Pipeline: `tensor.quantize_fp8(format="e4m3")` with calibration statistics
- [x] S5.8 — FP8 Dequantization: `tensor.dequantize()` restores f32 with scale factor application
- [x] S5.9 — Type System Integration: Add `fp8e5m2` and `fp8e4m3` as primitive types in lexer, parser, and analyzer
- [x] S5.10 — Unit Tests: 15+ tests covering conversion accuracy, overflow saturation, NaN propagation, round-trip fidelity

### Sprint S6 — FP4 Types

- [x] S6.1 — E2M1 Format: Implement 4-bit float with 2-bit exponent, 1-bit mantissa, value range [-6.0, 6.0]
- [x] S6.2 — NVFP4 Two-Level Scaling: Implement NVIDIA's block-level scale (per-32 elements) + tensor-level scale
- [x] S6.3 — Packing 8 Values per u32: Pack/unpack 8 FP4 values into a single u32 for memory-efficient storage
- [x] S6.4 — FP4 Arithmetic: Multiply-accumulate in f32 after upcast, store result back as FP4 with scaling
- [x] S6.5 — FP4-to-F32 Conversion: Upcast with two-level scale application: `value * block_scale * tensor_scale`
- [x] S6.6 — F32-to-FP4 Conversion: Downcasting with optimal scale computation via absmax calibration
- [x] S6.7 — FP4 Tensor Storage: Extend tensor backend to store FP4 data as packed u32 arrays with scale metadata
- [x] S6.8 — FP4 Quantization API: `tensor.quantize_fp4(block_size=32)` returns quantized tensor + scales
- [x] S6.9 — Type System Integration: Add `fp4` as primitive type, enforce no direct arithmetic (must dequantize first)
- [x] S6.10 — Unit Tests: 12+ tests for packing, conversion accuracy, scale computation, quantization round-trip

### Sprint S7 — BF16 Support

- [x] S7.1 — BF16 Format: Implement bfloat16 (1 sign, 8 exponent, 7 mantissa) using u16 storage
- [x] S7.2 — BF16 Arithmetic: Add, sub, mul, div via f32 upcasting with truncation back to bf16
- [x] S7.3 — F32-to-BF16 Conversion: Truncation-based (fast) and round-to-nearest-even (accurate) modes
- [x] S7.4 — BF16-to-F32 Conversion: Zero-extend lower 16 mantissa bits, no precision loss on exponent
- [x] S7.5 — BF16 Tensor Ops: matmul, element-wise ops, reductions operating on bf16 tensor data
- [x] S7.6 — Mixed-Precision Training: Forward pass in bf16, loss computation in f32, backward in bf16, master weights in f32
- [x] S7.7 — Loss Scaling: Dynamic loss scaling for bf16 training — scale up gradients, skip step on overflow
- [x] S7.8 — BF16 Type in Language: Add `bf16` as primitive type with implicit promotion rules to f32
- [x] S7.9 — Cranelift BF16 Lowering: Lower bf16 ops to f32 ops with conversion instructions in native codegen
- [x] S7.10 — Unit Tests: 15+ tests for arithmetic accuracy, mixed-precision training convergence, loss scaling

### Sprint S8 — Structured Sparsity

- [x] S8.1 — 4:2 Sparsity Pattern: Implement detection and enforcement of 4:2 pattern (2 zeros per 4 elements)
- [x] S8.2 — Sparse Metadata Format: Store sparsity mask as 2-bit indices per group of 4 elements (NVIDIA format)
- [x] S8.3 — CSR Storage: Implement Compressed Sparse Row format for general sparse tensors
- [x] S8.4 — CSC Storage: Implement Compressed Sparse Column format for column-major sparse access
- [x] S8.5 — Sparse-Dense MatMul: Multiply sparse matrix (4:2 or CSR) by dense matrix with 2x throughput path
- [x] S8.6 — Pruning API: `model.prune(sparsity=0.5, pattern="4:2")` with magnitude-based weight selection
- [x] S8.7 — Pruning Schedule: Gradual pruning during training — ramp sparsity from 0 to target over N steps
- [x] S8.8 — Sparse Tensor Type: `SparseTensor { data, indices, indptr, format, shape }` in runtime value system
- [x] S8.9 — Sparsity Analysis: `tensor.sparsity()` returns fraction of zeros, `tensor.is_structured_sparse()` check
- [x] S8.10 — Unit Tests: 12+ tests for pattern detection, CSR/CSC correctness, sparse matmul accuracy, pruning

---

## Phase 3: NPU Integration (S9-S12)

> Wire up real neural processing units — Intel, AMD, and future vendors — with a new @npu context.

### Sprint S9 — @npu Context

- [x] S9.1 — Lexer Token: Add `Npu` as context annotation token, recognized after `@` prefix
- [x] S9.2 — Parser Support: Parse `@npu fn ...` and `@npu { ... }` blocks, attach NpuContext to AST nodes
- [x] S9.3 — Analyzer Rules: @npu disallows raw pointers, heap allocation, and direct OS primitives (tensor-only)
- [x] S9.4 — Type Checking: Enforce that @npu functions only accept/return tensor types and scalar numerics
- [x] S9.5 — Context Isolation: @npu cannot call @kernel functions; @kernel cannot call @npu functions
- [x] S9.6 — @npu + @device Interaction: @device functions can call @npu functions (both tensor-friendly)
- [x] S9.7 — Error Codes: Define NE001-NE004 (NpuError) for context violations in @npu
- [x] S9.8 — Diagnostic Messages: miette-powered error messages with @npu context violation explanations
- [x] S9.9 — Documentation: Add @npu to language spec, context annotation table, and security model
- [x] S9.10 — Unit Tests: 15+ tests covering @npu allowed/disallowed operations, cross-context calls, error codes

### Sprint S10 — Intel OpenVINO Backend

- [x] S10.1 — OpenVINO C API Bindings: Generate safe Rust wrappers for ov_core, ov_model, ov_infer_request
- [x] S10.2 — Model Compilation: Load ONNX model via `ov_core_compile_model()`, target "NPU" device plugin
- [x] S10.3 — Inference Request: Create infer request, set input tensors, run synchronous inference, read output
- [x] S10.4 — Async Execution: `ov_infer_request_start_async()` with callback-based completion notification
- [x] S10.5 — INT8 Optimization: Enable NPU INT8 inference via `ov_core_set_property()` with quantization hints
- [x] S10.6 — FP16 Optimization: Enable NPU FP16 mode for higher throughput on supported models
- [x] S10.7 — Tensor Bridge: Convert Fajar Lang TensorValue to OpenVINO ov_tensor and back without data copy
- [x] S10.8 — Error Handling: Map OpenVINO status codes to Fajar Lang NpuError with meaningful diagnostics
- [x] S10.9 — Feature Gate: Gate behind `--features openvino` to avoid hard dependency on OpenVINO runtime
- [x] S10.10 — Unit Tests: 10+ tests with mock OpenVINO API, integration test on real NPU hardware if available

### Sprint S11 — AMD XDNA Backend

- [x] S11.1 — XDNA Driver Interface: Open /dev/accel/accel0 via ioctl, query XDNA 2 NPU capabilities
- [x] S11.2 — AIE Tile Programming: Submit computation graph to AI Engine tiles via XDNA runtime API
- [x] S11.3 — Vitis AI Runtime: Integrate Vitis AI runtime library for model loading and execution on XDNA
- [x] S11.4 — 50 TOPS Dispatch: Benchmark and validate throughput targeting 50 TOPS on AMD Ryzen AI 300 series
- [x] S11.5 — INT8 Inference Path: Quantized INT8 model execution on XDNA with per-tensor scaling
- [x] S11.6 — Memory Management: Allocate/free device buffers via XDNA DMA, zero-copy where possible
- [x] S11.7 — Tensor Bridge: Convert Fajar Lang TensorValue to XDNA buffer format and back
- [x] S11.8 — Error Handling: Map XDNA error codes to Fajar Lang NpuError, handle device-busy gracefully
- [x] S11.9 — Feature Gate: Gate behind `--features xdna` to avoid hard dependency on AMD driver stack
- [x] S11.10 — Unit Tests: 10+ tests with mock XDNA driver, integration test on real AMD NPU if available

### Sprint S12 — ONNX-to-NPU Pipeline

- [x] S12.1 — ONNX Model Loading: Parse ONNX protobuf format, build computation graph in memory
- [x] S12.2 — Graph Optimization: Constant folding, dead node elimination, shape inference on ONNX graph
- [x] S12.3 — Operator Fusion: Fuse Conv+BN+ReLU, MatMul+Add, and other common patterns for NPU efficiency
- [x] S12.4 — NPU-Specific Quantization: Calibration-based INT8 quantization targeting NPU operator support
- [x] S12.5 — Model Partitioning: Split graph into NPU-executable and CPU-fallback subgraphs automatically
- [x] S12.6 — Pipeline Assembly: Chain load→optimize→quantize→partition→compile into single `npu_compile()` API
- [x] S12.7 — Caching Compiled Models: Cache compiled NPU blobs to disk, skip recompilation on repeat inference
- [x] S12.8 — Benchmarking Harness: Measure latency (p50/p99), throughput (inferences/sec), memory usage per model
- [x] S12.9 — Example: ResNet-18 inference on NPU via ONNX pipeline, end-to-end from .onnx file to prediction
- [x] S12.10 — Unit Tests: 12+ tests for graph optimization correctness, fusion patterns, quantization accuracy

---

## Phase 4: Jetson Thor BSP (S13-S16)

> First-class support for NVIDIA Jetson Thor — the target platform for embedded AI at scale.

### Sprint S13 — Jetson Thor Platform

- [x] S13.1 — T4000/T5000 Detection: Identify Jetson Thor module variant via /proc/device-tree/model string
- [x] S13.2 — JetPack 7.1 SDK: Link against JetPack 7.1 CUDA libraries, validate SDK version at build time
- [x] S13.3 — AArch64 Cross-Compilation: Configure Cranelift for aarch64 target with Thor-specific CPU features
- [x] S13.4 — Device Tree Parsing: Read Thor device tree for memory layout, peripheral addresses, GPU configuration
- [x] S13.5 — L4T Integration: Integrate with Linux for Tegra (L4T) BSP for kernel module loading and device access
- [x] S13.6 — Thor CPU Config: Detect Cortex-A78AE + Grace CPU cores, configure thread affinity for RT tasks
- [x] S13.7 — Thor Memory Map: Map DRAM regions (up to 128GB), carve out GPU/DLA/PVA memory partitions
- [x] S13.8 — Boot Configuration: Generate Thor-compatible boot scripts for Fajar Lang firmware images
- [x] S13.9 — Platform Abstraction: `platform::jetson_thor()` returns ThorPlatform with all capabilities
- [x] S13.10 — Unit Tests: 10+ tests with mock device tree, cross-compilation smoke test, platform detection

### Sprint S14 — Blackwell GPU on Thor

- [x] S14.1 — sm_101 Compute Capability: Register sm_101 in GPU feature database, map to Blackwell architecture
- [x] S14.2 — CUDA 13.0 API: Bind new CUDA 13.0 APIs for Blackwell features (TMA, cluster launch, DPX)
- [x] S14.3 — 5th Gen Tensor Core Access: Configure Tensor Core dispatch for FP4/FP8/BF16/INT8 matrix operations
- [x] S14.4 — FP4 Inference Path: End-to-end FP4 inference using Tensor Cores: quantize → compute → dequantize
- [x] S14.5 — FP8 Training Path: Mixed FP8 training with E4M3 forward, E5M2 backward, FP32 master weights
- [x] S14.6 — CUDA Graph Capture: Capture inference workload as CUDA graph for reduced kernel launch overhead
- [x] S14.7 — Unified Memory: Enable CUDA Unified Memory for Thor's coherent CPU-GPU memory architecture
- [x] S14.8 — Stream Management: Create and manage multiple CUDA streams for pipelined inference
- [x] S14.9 — Blackwell Feature Flags: Register all Blackwell features in HardwareProfile GPU capabilities
- [x] S14.10 — Unit Tests: 12+ tests for sm_101 detection, Tensor Core dispatch, FP4/FP8 paths, CUDA graphs

### Sprint S15 — MIG Partitioning

- [x] S15.1 — MIG Support Detection: Query GPU for MIG capability via NVML, check if MIG mode is enabled
- [x] S15.2 — Partition Enumeration: List existing MIG instances (GPU instances + compute instances) via NVML
- [x] S15.3 — Partition Creation: Create MIG GPU instance with specified profile (1g.10gb, 2g.20gb, etc.)
- [x] S15.4 — Compute Instance: Create compute instance within GPU instance for isolated execution
- [x] S15.5 — Per-Partition Inference: Bind CUDA context to specific MIG partition, run inference in isolation
- [x] S15.6 — Resource Isolation: Verify memory isolation between partitions (no cross-partition access)
- [x] S15.7 — Partition Destruction: Clean teardown of compute instance and GPU instance, release resources
- [x] S15.8 — MIG Configuration API: `gpu.mig_create(profile="1g.10gb")`, `gpu.mig_list()`, `gpu.mig_destroy(id)`
- [x] S15.9 — Multi-Tenant Inference: Demonstrate two models running simultaneously on separate MIG partitions
- [x] S15.10 — Unit Tests: 10+ tests with mock NVML, integration test on MIG-capable GPU if available

### Sprint S16 — Thor Power Management

- [x] S16.1 — NVPM API Bindings: Bind NVIDIA Power Manager API for reading/setting power modes
- [x] S16.2 — Power Mode Profiles: Define profiles: 10W (idle), 30W (inference), 60W (training), 130W (max perf)
- [x] S16.3 — Thermal Monitoring: Read GPU/CPU temperatures via thermal zones, trigger throttling at thresholds
- [x] S16.4 — Thermal Throttling API: `power::set_thermal_limit(85)` with automatic clock reduction on breach
- [x] S16.5 — Dynamic Frequency Scaling: Set GPU/CPU clock frequencies based on workload demand
- [x] S16.6 — Power Budget Allocation: Distribute power budget across CPU, GPU, DLA when all are active
- [x] S16.7 — Energy Measurement: Read power rail sensors for real-time wattage reporting per component
- [x] S16.8 — Power Profile API: `power::set_mode("30W")`, `power::current_watts()`, `power::thermal_status()`
- [x] S16.9 — Integration with Scheduler: Reduce inference batch size when thermal throttling is active
- [x] S16.10 — Unit Tests: 10+ tests with mock NVPM/thermal sensors, power mode transition verification

---

## Phase 5: AVX-512 / AMX / Blackwell Codegen (S17-S20)

> Generate actual SIMD and accelerator instructions — not library calls, but real machine code.

### Sprint S17 — AVX-512 Codegen

- [x] S17.1 — Cranelift AVX-512 Backend: Enable AVX-512F instruction emission in Cranelift ISA settings
- [x] S17.2 — 512-bit Vector Type: Map `simd16xf32` to ZMM registers in Cranelift IR for 16-wide f32 ops
- [x] S17.3 — VNNI INT8 Dot Product: Emit VPDPBUSD instruction for INT8 dot product (4x throughput over scalar)
- [x] S17.4 — Masked Operations: Emit masked load/store/arithmetic using AVX-512 opmask registers (k1-k7)
- [x] S17.5 — Broadcast Operations: Emit VBROADCASTSS/SD for scalar-to-vector broadcast in inner loops
- [x] S17.6 — Gather/Scatter: Emit VGATHERDPS/VSCATTERDPS for indirect memory access patterns
- [x] S17.7 — Reduction Operations: Emit horizontal add/max/min using AVX-512 reduce instructions
- [x] S17.8 — Auto-Vectorization Hints: Detect tensor inner loops amenable to AVX-512 and emit vector code
- [x] S17.9 — Fallback to AVX2: Generate AVX2 fallback when AVX-512 is not available, runtime dispatch
- [x] S17.10 — Unit Tests: 15+ tests validating AVX-512 instruction emission, correctness vs scalar reference

### Sprint S18 — Intel AMX Integration

- [x] S18.1 — TILECFG Instruction: Emit LDTILECFG to configure AMX tile registers (rows, cols per tile)
- [x] S18.2 — TDPBF16PS Instruction: Emit BF16 tile matrix multiply-accumulate into FP32 accumulator
- [x] S18.3 — TDPBSSD Instruction: Emit INT8 signed tile matrix multiply-accumulate (16x64 * 64x16 per op)
- [x] S18.4 — TDPFP16PS Instruction: Emit FP16 tile matrix multiply-accumulate (AMX-FP16 extension)
- [x] S18.5 — Tile Load/Store: Emit TILELOADD/TILESTORED for moving data between memory and tile registers
- [x] S18.6 — Tile Register Allocation: Manage 8 tile registers (TMM0-TMM7) in Cranelift register allocator
- [x] S18.7 — 1KB Tile MatMul: Demonstrate 1024-byte tile multiply: 16x64 BF16 * 64x16 BF16 → 16x16 FP32
- [x] S18.8 — AMX Kernel for Dense Layer: Compile Fajar Lang Dense layer forward pass using AMX tile ops
- [x] S18.9 — AMX Context Save/Restore: Handle XSAVE/XRSTOR for AMX state across function calls
- [x] S18.10 — Unit Tests: 12+ tests for TILECFG, tile matmul correctness, register allocation, context switch

### Sprint S19 — AVX10.2 + APX

- [x] S19.1 — AVX10.2 Detection: Detect AVX10.2 support via CPUID leaf 24H, version and vector length
- [x] S19.2 — AVX10.2 New Instructions: Emit new comparison, conversion, and minmax instructions from AVX10.2
- [x] S19.3 — YMM Promotion: Use 256-bit YMM encodings for AVX10.2 where AVX-512 ZMM is unavailable
- [x] S19.4 — APX Detection: Detect Advanced Performance Extensions (APX) via CPUID for 32-GPR support
- [x] S19.5 — 32 GPR Register Allocation: Extend Cranelift x86_64 register allocator to use R16-R31 (APX)
- [x] S19.6 — EGPR Encoding: Emit REX2 prefix for Extended General Purpose Registers in instruction encoding
- [x] S19.7 — NDD Instructions: Emit Non-Destructive Destination forms (3-operand) enabled by APX
- [x] S19.8 — Reduced Register Pressure: Benchmark register spill reduction with 32 GPRs vs 16 GPRs
- [x] S19.9 — Combined AVX10.2+APX: Test combined codegen path using both extensions simultaneously
- [x] S19.10 — Unit Tests: 12+ tests for AVX10.2 instruction emission, APX register allocation, NDD encoding

### Sprint S20 — Blackwell PTX

- [x] S20.1 — PTX Emission Framework: Build PTX text assembly emitter for sm_100 and sm_101 targets
- [x] S20.2 — tcgen05.mma Instruction: Emit 5th-gen Tensor Core MMA (matrix multiply-accumulate) warp-level ops
- [x] S20.3 — TMEM Access: Emit Tensor Memory (TMEM) 256KB access instructions for warp-private storage
- [x] S20.4 — FP4 Tensor Core Dispatch: Emit tcgen05.mma.fp4 for 4-bit Tensor Core matrix multiply
- [x] S20.5 — FP8 Tensor Core Dispatch: Emit tcgen05.mma.e4m3/e5m2 for 8-bit Tensor Core operations
- [x] S20.6 — BF16 Tensor Core Dispatch: Emit tcgen05.mma.bf16 for BF16 matrix operations on Blackwell
- [x] S20.7 — TMA (Tensor Memory Accelerator): Emit TMA bulk copy instructions for efficient data movement
- [x] S20.8 — Cluster Launch: Emit cooperative_groups cluster launch for multi-SM coordination
- [x] S20.9 — PTX-to-CUBIN: Invoke ptxas to compile emitted PTX to device binary (CUBIN) for loading
- [x] S20.10 — Unit Tests: 12+ tests for PTX correctness, Tensor Core output validation, CUBIN compilation

---

## Phase 6: CI/CD & Binary Distribution (S21-S24)

> Automated quality gates and binary releases for every platform.

### Sprint S21 — GitHub Actions CI

- [ ] S21.1 — CI Workflow File: Create `.github/workflows/ci.yml` with push/PR triggers on main and develop
- [ ] S21.2 — Linux x86_64 Job: Ubuntu 24.04 runner, cargo test + clippy + fmt, Rust stable + nightly
- [ ] S21.3 — macOS ARM64 Job: macos-14 (M1) runner, cargo test, validate ARM64 codegen paths
- [ ] S21.4 — Windows x86_64 Job: windows-latest runner, cargo test, MSVC and GNU toolchain variants
- [ ] S21.5 — Feature Matrix: Test `--features native`, `--features gpu`, default features, and `--all-features`
- [ ] S21.6 — Artifact Caching: Cache cargo registry, target directory, and compiled dependencies across runs
- [ ] S21.7 — Test Result Reporting: Upload test results as GitHub check annotations, fail PR on any test failure
- [ ] S21.8 — Clippy Enforcement: Run `cargo clippy -- -D warnings` as required check, block merge on warnings
- [ ] S21.9 — Format Check: Run `cargo fmt -- --check` as required check, block merge on formatting issues
- [ ] S21.10 — Badge Setup: Add CI status badge to README.md, link to workflow runs page

### Sprint S22 — Cross-Compilation Pipeline

- [ ] S22.1 — ARM64 Linux Target: Cross-compile to aarch64-unknown-linux-gnu using cross-rs in CI
- [ ] S22.2 — RISC-V Target: Cross-compile to riscv64gc-unknown-linux-gnu, validate binary headers
- [ ] S22.3 — Windows x86_64 Target: Cross-compile from Linux to x86_64-pc-windows-gnu with MinGW
- [ ] S22.4 — macOS ARM64 Target: Native build on macos-14 runner for aarch64-apple-darwin
- [ ] S22.5 — Static Linking: Produce statically-linked binaries (musl on Linux) for zero-dependency deployment
- [ ] S22.6 — Binary Size Optimization: Enable LTO, strip symbols, codegen-units=1 for minimal release binary
- [ ] S22.7 — QEMU Smoke Test: Run cross-compiled ARM64/RISC-V binary under QEMU in CI to verify execution
- [ ] S22.8 — Binary Size Tracking: Record binary size per target per commit, alert on >10% regression
- [ ] S22.9 — Cross-Compile Matrix: CI job matrix: 4 targets * 2 profiles (debug + release) = 8 jobs
- [ ] S22.10 — Unit Tests: Verify cross-compiled binaries produce correct output for hello.fj on each target

### Sprint S23 — OSS-Fuzz Integration

- [ ] S23.1 — Fuzz Targets: Create fuzz targets for lexer (arbitrary byte input), parser (token stream), analyzer
- [ ] S23.2 — Grammar-Aware Fuzzer: Build grammar-aware fuzzer that generates syntactically-plausible .fj programs
- [ ] S23.3 — Corpus Seeding: Seed fuzzer with all 24 example .fj programs and test case inputs
- [ ] S23.4 — OSS-Fuzz Integration: Register project with Google OSS-Fuzz, create Dockerfile and build script
- [ ] S23.5 — CI Fuzz Job: Run fuzzer for 5 minutes on each PR, report any crashes as test failures
- [ ] S23.6 — Crash Reproduction: Script to reproduce fuzz crashes locally from OSS-Fuzz testcase artifacts
- [ ] S23.7 — Coverage Tracking: Integrate fuzz coverage with Codecov/Coveralls to track explored code paths
- [ ] S23.8 — Regression Corpus: Maintain corpus of previously-found crashes as permanent regression tests
- [ ] S23.9 — Sanitizer Builds: Fuzz with AddressSanitizer, MemorySanitizer (where applicable to Rust unsafe)
- [ ] S23.10 — Unit Tests: Verify fuzz targets build and run, test crash reproduction script

### Sprint S24 — Release Automation

- [ ] S24.1 — Semantic Versioning: Enforce semver via cargo-semver-checks, detect breaking API changes
- [ ] S24.2 — Changelog Generation: Auto-generate CHANGELOG.md from conventional commits using git-cliff
- [ ] S24.3 — Release Workflow: Create `.github/workflows/release.yml` triggered by version tag push (v*.*.*)
- [ ] S24.4 — Binary Packaging (tar.gz): Package Linux/macOS binaries as .tar.gz with README and LICENSE
- [ ] S24.5 — Binary Packaging (zip): Package Windows binary as .zip with README and LICENSE
- [ ] S24.6 — Debian Package: Build .deb package for Ubuntu/Debian with proper metadata and postinst script
- [ ] S24.7 — RPM Package: Build .rpm package for Fedora/RHEL with proper spec file
- [ ] S24.8 — GitHub Release Upload: Upload all binary artifacts to GitHub Release with checksums (SHA256)
- [ ] S24.9 — Install Script: Create `curl -sSf https://fajarlang.dev/install.sh | sh` installer
- [ ] S24.10 — Unit Tests: Verify package contents, checksum generation, install script on clean container

---

## Phase 7: Package Registry & Website (S25-S28)

> A live ecosystem — publish packages, discover libraries, showcase the language.

### Sprint S25 — Registry Backend

- [ ] S25.1 — Cloudflare Workers API: Create Workers project for registry.fajarlang.dev with REST routes
- [ ] S25.2 — D1 Database Schema: Design tables: packages, versions, users, api_keys, downloads
- [ ] S25.3 — Package Upload Endpoint: `POST /api/v1/packages` accepting .tar.gz with fj.toml metadata
- [ ] S25.4 — Package Download Endpoint: `GET /api/v1/packages/{name}/{version}` serving .tar.gz artifact
- [ ] S25.5 — Package Search Endpoint: `GET /api/v1/search?q=...` with full-text search on name and description
- [ ] S25.6 — Authentication: API key-based auth for publish, `fj login` stores key in ~/.fj/credentials
- [ ] S25.7 — Version Validation: Enforce semver on upload, reject duplicate versions, validate fj.toml schema
- [ ] S25.8 — Rate Limiting: Cloudflare rate limiting on upload (10/hour) and download (1000/hour) per IP
- [ ] S25.9 — R2 Storage: Store package tarballs in Cloudflare R2 for cost-effective blob storage
- [ ] S25.10 — Unit Tests: 15+ tests for API routes, auth flow, version validation, search ranking

### Sprint S26 — Registry Client

- [ ] S26.1 — `fj publish` Command: Package current project into .tar.gz, upload to registry with API key auth
- [ ] S26.2 — `fj install` Command: Download package from registry, extract to local packages/ directory
- [ ] S26.3 — `fj search` Command: Query registry search endpoint, display results in formatted table
- [ ] S26.4 — `fj login` Command: Accept API key interactively, store in ~/.fj/credentials file (mode 0600)
- [ ] S26.5 — `fj yank` Command: Mark a published version as yanked (not deleted, just hidden from search)
- [ ] S26.6 — Local Cache: Cache downloaded packages in ~/.fj/cache/ with content-addressed storage
- [ ] S26.7 — Dependency Resolution: Resolve transitive dependencies from fj.toml, download all required packages
- [ ] S26.8 — Lockfile: Generate fj.lock with exact resolved versions and checksums for reproducible builds
- [ ] S26.9 — Offline Mode: `fj install --offline` uses only cached packages, fails if missing
- [ ] S26.10 — Unit Tests: 12+ tests for publish flow, install flow, dependency resolution, lockfile generation

### Sprint S27 — Landing Page

- [ ] S27.1 — Domain Setup: Configure fajarlang.dev DNS on Cloudflare, SSL certificate via Cloudflare Pages
- [ ] S27.2 — Static Site Generator: Build landing page with clean HTML/CSS (no heavy frameworks), responsive design
- [ ] S27.3 — Hero Section: Tagline, one-liner install command, animated code example showing Fajar Lang syntax
- [ ] S27.4 — Feature Showcase: 4 cards: Embedded ML, OS Integration, Safety, Performance — with icons and descriptions
- [ ] S27.5 — Quickstart Section: 3-step guide: install, write hello.fj, run with `fj run hello.fj`
- [ ] S27.6 — Benchmark Comparisons: Performance charts comparing Fajar Lang to C, Rust, Python for key workloads
- [ ] S27.7 — Code Examples: Tabbed code examples (ML inference, OS kernel, embedded sensor, pattern matching)
- [ ] S27.8 — Footer: Links to GitHub, documentation, playground, registry, community (Discord placeholder)
- [ ] S27.9 — SEO + Meta Tags: OpenGraph, Twitter cards, structured data (JSON-LD) for search engine visibility
- [ ] S27.10 — Deployment: Deploy to Cloudflare Pages with automatic builds from docs-site branch

### Sprint S28 — Documentation Portal

- [ ] S28.1 — mdBook Deployment: Deploy existing 40+ page mdBook to docs.fajarlang.dev via Cloudflare Pages
- [ ] S28.2 — API Reference Hosting: Generate API reference from doc comments, host at docs.fajarlang.dev/api
- [ ] S28.3 — Version Selector: Dropdown to switch between v1.0 and v1.1 documentation versions
- [ ] S28.4 — Search Integration: Add Pagefind or Algolia search index across all documentation pages
- [ ] S28.5 — Tutorial Section: Step-by-step tutorials: "Hello World", "Build a Calculator", "Train MNIST"
- [ ] S28.6 — Interactive Examples: Link code examples to playground for "Try it" functionality
- [ ] S28.7 — Dark Mode: Implement dark/light theme toggle with OS preference detection
- [ ] S28.8 — Mobile Navigation: Responsive sidebar navigation that collapses to hamburger on mobile
- [ ] S28.9 — Analytics: Cloudflare Web Analytics (privacy-respecting) for page view tracking
- [ ] S28.10 — Unit Tests: Verify all documentation links resolve, code examples compile, search returns results

---

## Phase 8: Online Playground (S29-S32)

> Run Fajar Lang in the browser — no install required.

### Sprint S29 — Wasm Compiler Target

- [ ] S29.1 — Wasm Build Target: Configure Cargo for wasm32-unknown-unknown target, exclude native-only crates
- [ ] S29.2 — Lexer in Wasm: Compile lexer module to Wasm, expose `tokenize()` via wasm-bindgen
- [ ] S29.3 — Parser in Wasm: Compile parser module to Wasm, expose `parse()` via wasm-bindgen
- [ ] S29.4 — Interpreter in Wasm: Compile tree-walking interpreter to Wasm, expose `eval_source()` via wasm-bindgen
- [ ] S29.5 — Memory Sandbox: Limit Wasm linear memory to 64MB, implement OOM handling for runaway programs
- [ ] S29.6 — Execution Timeout: Implement instruction counter in Wasm interpreter, abort after 10M instructions
- [ ] S29.7 — Print Capture: Redirect `print`/`println` output to a string buffer returned to JavaScript
- [ ] S29.8 — Error Formatting: Format miette diagnostics as plain text (no ANSI codes) for browser display
- [ ] S29.9 — Wasm Bundle Size: Optimize Wasm binary size with wasm-opt, target < 5MB compressed
- [ ] S29.10 — Unit Tests: Verify Wasm compilation, test eval_source in Node.js/wasm-pack test environment

### Sprint S30 — Playground UI

- [ ] S30.1 — Monaco Editor: Integrate Monaco Editor (VS Code engine) with Fajar Lang syntax highlighting
- [ ] S30.2 — Syntax Theme: Create custom Fajar Lang theme for Monaco matching docs site color scheme
- [ ] S30.3 — Run Button: "Run" button that sends editor content to Wasm eval_source(), displays result
- [ ] S30.4 — Output Panel: Split-pane output panel showing stdout, return value, and execution time
- [ ] S30.5 — Error Display: Show miette-style error messages with line highlights in editor gutter
- [ ] S30.6 — Loading State: Show spinner during Wasm initialization and execution, handle timeout gracefully
- [ ] S30.7 — Keyboard Shortcuts: Ctrl+Enter to run, Ctrl+S to save (to localStorage), Ctrl+L to clear output
- [ ] S30.8 — Responsive Layout: Editor and output panels stack vertically on mobile, side-by-side on desktop
- [ ] S30.9 — Local Storage: Auto-save editor content to localStorage, restore on page load
- [ ] S30.10 — Unit Tests: Playwright/Cypress end-to-end tests for run flow, error display, keyboard shortcuts

### Sprint S31 — Share & Embed

- [ ] S31.1 — URL-Encoded Sharing: Compress code with lz-string, encode in URL fragment (#code=...) for sharing
- [ ] S31.2 — Short URLs: Generate short URLs via Cloudflare KV (play.fajarlang.dev/s/{id}) for long programs
- [ ] S31.3 — Copy Share Link: "Share" button copies URL to clipboard with toast notification
- [ ] S31.4 — oEmbed Endpoint: Implement oEmbed provider at fajarlang.dev/oembed for rich embeds in docs/blogs
- [ ] S31.5 — iframe Embed API: `<iframe src="play.fajarlang.dev/embed?code=...">` for embedding in external sites
- [ ] S31.6 — Embed Options: URL params for embed: `theme=dark|light`, `readonly=true`, `autorun=true`
- [ ] S31.7 — Social Preview Cards: Generate OpenGraph image with code preview for shared playground links
- [ ] S31.8 — Twitter/X Card: Twitter card meta tags showing code snippet and language name when shared
- [ ] S31.9 — Embed in mdBook: Add "Try it" buttons in documentation that open playground with pre-filled code
- [ ] S31.10 — Unit Tests: Test URL encoding/decoding, short URL generation, oEmbed response format

### Sprint S32 — Example Gallery

- [ ] S32.1 — Gallery Page: Grid layout page listing all playground examples with title, description, difficulty
- [ ] S32.2 — Difficulty Levels: Tag examples as Beginner (green), Intermediate (yellow), Advanced (red)
- [ ] S32.3 — Hello World Example: Basic hello world with variable declaration, function call, print output
- [ ] S32.4 — Pattern Matching Example: Enum definition, match expression, exhaustive pattern coverage
- [ ] S32.5 — Struct & Methods Example: Point struct, impl block, method calls, operator use
- [ ] S32.6 — Error Handling Example: Result type, `?` operator, match on Ok/Err, error propagation
- [ ] S32.7 — Tensor Operations Example: Create tensor, matmul, activation function, shape manipulation
- [ ] S32.8 — ML Training Example: Simple linear regression with autograd, optimizer, training loop
- [ ] S32.9 — Pipeline Operator Example: Chain transformations with `|>`, demonstrate functional style
- [ ] S32.10 — Guided Tutorial: Step-by-step tutorial with 5 incremental examples building a calculator

---

## Phase 9: Multi-Accelerator Dispatch (S33-S36)

> Automatic inference dispatch across CPU, NPU, and GPU — the right hardware for the right workload.

### Sprint S33 — @infer Context

- [ ] S33.1 — Lexer Token: Add `Infer` as context annotation token, recognized after `@` prefix
- [ ] S33.2 — Parser Support: Parse `@infer fn ...` and `@infer { ... }` blocks, attach InferContext to AST
- [ ] S33.3 — Analyzer Rules: @infer allows tensor ops and scalar compute, disallows raw pointers and OS primitives
- [ ] S33.4 — Type Checking: @infer functions must return tensor or scalar, input types auto-dispatched
- [ ] S33.5 — Compile-Time Hints: `@infer(prefer=gpu)` or `@infer(prefer=npu)` to hint preferred accelerator
- [ ] S33.6 — Context Compatibility: @infer can call @device and @npu functions, cannot call @kernel
- [ ] S33.7 — Error Codes: Define IE001-IE004 (InferError) for @infer context violations
- [ ] S33.8 — Diagnostic Messages: miette-powered error messages for @infer misuse with suggested fixes
- [ ] S33.9 — Documentation: Add @infer to language spec, context table, examples in playground
- [ ] S33.10 — Unit Tests: 15+ tests for @infer allowed/disallowed ops, hint parsing, cross-context calls

### Sprint S34 — Dispatch Runtime

- [ ] S34.1 — Dispatch Decision Engine: Score available accelerators for given workload (matmul size, dtype, batch)
- [ ] S34.2 — CPU Fallback Path: Always-available CPU execution path for any @infer function
- [ ] S34.3 — NPU Dispatch Path: Route to NPU when model fits NPU constraints (INT8/FP16, supported ops)
- [ ] S34.4 — GPU Dispatch Path: Route to GPU for large matmuls, training, and FP4/FP8 inference
- [ ] S34.5 — Automatic Fallback: If preferred accelerator fails (OOM, unsupported op), fall back to next in chain
- [ ] S34.6 — Latency Profiling: First-run calibration measures actual latency per accelerator, caches results
- [ ] S34.7 — Workload Classification: Classify workloads as compute-bound, memory-bound, or latency-sensitive
- [ ] S34.8 — Dispatch Cache: Cache dispatch decisions per (model_hash, input_shape, available_hw) tuple
- [ ] S34.9 — Dispatch Logging: Log dispatch decisions at debug level for user visibility into hardware selection
- [ ] S34.10 — Unit Tests: 12+ tests for dispatch scoring, fallback chain, caching, latency profiling mock

### Sprint S35 — Heterogeneous Execution

- [ ] S35.1 — Graph Partitioning: Split computation graph into subgraphs per device (CPU, NPU, GPU)
- [ ] S35.2 — Data Transfer: Implement host-to-device and device-to-host tensor transfer with pinned memory
- [ ] S35.3 — Transfer Optimization: Overlap computation and data transfer using double-buffering
- [ ] S35.4 — Pipeline Parallelism: Execute subgraphs on different devices in pipeline (layer N on GPU, N+1 on NPU)
- [ ] S35.5 — Synchronization: Barrier-based synchronization between devices at subgraph boundaries
- [ ] S35.6 — Memory Pool: Pre-allocate device memory pools to avoid per-inference allocation overhead
- [ ] S35.7 — Multi-GPU Split: Distribute large models across multiple GPUs (tensor parallel or pipeline parallel)
- [ ] S35.8 — Heterogeneous Batch: Process different batch items on different accelerators simultaneously
- [ ] S35.9 — Execution Plan Visualization: Generate DOT graph showing which subgraph runs on which device
- [ ] S35.10 — Unit Tests: 12+ tests for graph partitioning, data transfer correctness, pipeline parallelism

### Sprint S36 — Profiler & Visualizer

- [ ] S36.1 — Per-Device Timer: Record wall-clock time per device per subgraph execution
- [ ] S36.2 — Memory Transfer Overhead: Measure and report bytes transferred between host and each device
- [ ] S36.3 — Roofline Model: Compute operational intensity and plot against device peak for bottleneck analysis
- [ ] S36.4 — Flame Graph Output: Generate flame graph SVG from profiling data (compatible with inferno)
- [ ] S36.5 — CLI Profiler: `fj profile run model.fj` outputs per-device timing breakdown to stdout
- [ ] S36.6 — JSON Profile Export: Export profiling data as Chrome Trace Event format for chrome://tracing
- [ ] S36.7 — Throughput Metrics: Report inferences/second, tokens/second, samples/second for common workloads
- [ ] S36.8 — Memory Watermark: Track peak memory usage per device across entire inference run
- [ ] S36.9 — Comparison Mode: `fj profile --compare cpu,gpu model.fj` runs on both and shows side-by-side
- [ ] S36.10 — Unit Tests: 10+ tests for timer accuracy, roofline computation, flame graph generation, JSON export

---

## Phase 10: Real-World Demos (S37-S40)

> Prove it works on real hardware with real workloads.

### Sprint S37 — Drone Firmware Demo

- [ ] S37.1 — Flight Controller Skeleton: Fajar Lang program with main loop: read sensors → infer → actuate
- [ ] S37.2 — Sensor Fusion: IMU (accelerometer + gyroscope) data fusion using complementary filter in .fj
- [ ] S37.3 — Real-Time Inference: Obstacle avoidance model running at 30Hz on Jetson Thor GPU
- [ ] S37.4 — Motor Control: PWM output generation for 4 brushless motors via HAL trait implementation
- [ ] S37.5 — Failsafe Logic: Watchdog timer, low-battery cutoff, GPS fence violation handling
- [ ] S37.6 — Telemetry: Serial UART output of flight state (attitude, altitude, battery, GPS) at 10Hz
- [ ] S37.7 — Cross-Compile for Thor: Build drone firmware targeting aarch64 Jetson Thor with JetPack 7.1
- [ ] S37.8 — Simulation Mode: Run same firmware in QEMU aarch64 with simulated sensor inputs
- [ ] S37.9 — Demo Script: Step-by-step reproduction guide: build, flash, boot, observe telemetry
- [ ] S37.10 — Demo Video Script: Outline for 3-minute video showing build, deploy, and flight simulation

### Sprint S38 — MNIST on Real GPU

- [ ] S38.1 — MNIST Data Loader: Load MNIST dataset from IDX files into Fajar Lang tensors
- [ ] S38.2 — LeNet-5 Model: Define LeNet-5 (Conv2d → ReLU → Pool → Conv2d → ReLU → Pool → Dense → Dense) in .fj
- [ ] S38.3 — GPU Training Loop: Train LeNet-5 on real RTX 5090/4090 GPU with CUDA backend, 10 epochs
- [ ] S38.4 — FP32 Baseline: Establish baseline accuracy (>98%) and training time in FP32
- [ ] S38.5 — BF16 Mixed Precision: Re-train with BF16 mixed precision, compare accuracy and speedup
- [ ] S38.6 — FP8 Quantized Inference: Post-training quantization to FP8 (E4M3), measure accuracy retention
- [ ] S38.7 — FP4 Quantized Inference: Aggressive quantization to FP4, measure accuracy vs latency tradeoff
- [ ] S38.8 — PyTorch Comparison: Benchmark identical LeNet-5 in PyTorch, compare training time and inference latency
- [ ] S38.9 — Results Table: Generate markdown table with accuracy, training time, inference latency across formats
- [ ] S38.10 — Demo Video Script: Outline for 3-minute video showing training, accuracy curves, benchmark results

### Sprint S39 — Mini OS on QEMU

- [ ] S39.1 — x86_64 Boot: Boot bare-metal Fajar Lang kernel on QEMU x86_64, reach protected mode
- [ ] S39.2 — Page Table Setup: 4-level page table initialization, identity-map first 4GB, higher-half kernel
- [ ] S39.3 — Interrupt Handler: IDT setup, handle divide-by-zero, page fault, double fault, keyboard IRQ
- [ ] S39.4 — Serial Console: UART 16550 driver for serial output, kernel log to serial port
- [ ] S39.5 — VGA Text Mode: 80x25 VGA text buffer with color attributes, scroll support
- [ ] S39.6 — Kernel Panic Display: Panic handler showing register dump, stack trace, error message on VGA
- [ ] S39.7 — ARM64 Boot: Boot on QEMU aarch64 (virt machine), reach EL1, set up MMU
- [ ] S39.8 — Simple Shell: Keyboard input → command parser → execute built-in commands (help, info, reboot)
- [ ] S39.9 — Build System: `fj build --target bare-x86_64` produces bootable ISO image via GRUB/Limine
- [ ] S39.10 — Demo Video Script: Outline for 3-minute video showing boot sequence, shell interaction, panic handling

### Sprint S40 — Integration Showcase

- [ ] S40.1 — End-to-End Demo: Single Fajar Lang project combining OS kernel + ML inference + hardware dispatch
- [ ] S40.2 — Scenario: Sensor Read to Prediction: @kernel reads sensor → @device preprocesses → @infer predicts → @kernel actuates
- [ ] S40.3 — Context Safety Demo: Show compiler rejecting @kernel code using tensor ops, @device using raw pointers
- [ ] S40.4 — Multi-Format Demo: Same model inferred in FP32, BF16, FP8, FP4 with accuracy comparison table
- [ ] S40.5 — Playground Integration: All demo code runnable in online playground (interpreter-mode subset)
- [ ] S40.6 — Tutorial Documentation: 20-page tutorial walking through building the end-to-end demo from scratch
- [ ] S40.7 — Benchmark Suite: Publish all benchmark results: training time, inference latency, binary size, memory usage
- [ ] S40.8 — Blog Post Draft: 2000-word announcement post: "Fajar Lang v1.1: From Simulation to Silicon"
- [ ] S40.9 — Video Script: 10-minute video script covering all 3 demos + playground + ecosystem
- [ ] S40.10 — Release Checklist: Final verification of all 400 tasks, test suite, documentation, release notes

---

## Dependencies

```
Phase 1 (HW Detect) ──────→ Phase 3 (NPU)
         │
         ├──────────────────→ Phase 4 (Jetson Thor)
         │
         └──────────────────→ Phase 5 (Codegen)

Phase 2 (Numeric)   ──────→ Phase 5 (Codegen)
         │
         └──────────────────→ Phase 9 (Multi-Accel)

Phase 3 (NPU)       ──────→ Phase 9 (Multi-Accel)

Phase 4 (Jetson)    ──────→ Phase 10 (Demos)

Phase 5 (Codegen)   ──────→ Phase 9 (Multi-Accel)

Phase 6 (CI/CD)     ──────→ Phase 7 (Registry)

Phase 7 (Registry)  ──────→ Phase 8 (Playground)

Phase 9 (Multi-Accel) ────→ Phase 10 (Demos)
```

**Parallelism opportunities:**
- Phases 1 and 2 can run in parallel (no dependency between hardware detection and numeric formats)
- Phases 3 and 4 can run in parallel once Phase 1 is complete
- Phases 6 and the early parts of Phase 5 can run in parallel
- Phase 7 and Phase 8 backend work can overlap with hardware phases

---

## Success Criteria

| Criterion | Target |
|-----------|--------|
| Tasks complete | 400/400 |
| Test suite | 4,000+ tests (0 failures) |
| Jetson Thor benchmark | T5000 FP4 inference latency published |
| Website | fajarlang.dev live with all sections |
| Playground | play.fajarlang.dev operational, Wasm < 5MB |
| Package registry | registry.fajarlang.dev accepting uploads |
| Real-hardware demos | At least 3 demo videos (drone, MNIST, OS) |
| Binary releases | 4 platforms (Linux x86_64, Linux ARM64, macOS ARM64, Windows x86_64) |
| CI/CD | All quality gates automated, zero manual steps |
| Documentation | docs.fajarlang.dev with search, version selector, dark mode |

---

## Release Gate

All of the following MUST pass before tagging v1.1.0:

```bash
# Code quality
cargo test                             # all pass
cargo test --features native           # all pass (including codegen)
cargo clippy -- -D warnings            # zero warnings
cargo fmt -- --check                   # clean

# Phase verification
# All 10 phases verified (400/400 tasks marked [x])

# Documentation
# CHANGELOG.md updated with v1.1.0 entry
# docs.fajarlang.dev deployed with v1.1 content

# Releases
# GitHub release v1.1.0 created with binaries for all 4 platforms
# Package registry accepting uploads
# Playground operational

# Demos
# At least 3 demo recordings complete
```

---

*V11_PLAN.md v1.0 | Created 2026-03-11*
