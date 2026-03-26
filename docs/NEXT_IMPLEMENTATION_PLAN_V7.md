# Plan V7 "Ascendancy" — Comprehensive Implementation Plan

> **Date:** 2026-03-26
> **Author:** Fajar (PrimeCore.id) + Claude Opus 4.6
> **Prerequisite:** Plan V6 "Dominance" complete (560/560 tasks)
> **Current State:** 4,849 tests, ~294K LOC, 165 examples, 40 modules
> **Scope:** 10 options, 680 tasks, 68 sprints
> **Estimated Effort:** ~136 hours total

---

## Project State Summary

```
Language:      Fajar Lang v5.5.0 "Illumination"
Compiler:      Cranelift JIT/AOT + LLVM + Wasm targets
Tests:         4,849 (0 failures)
LOC:           ~294,000 Rust (220+ files)
Modules:       40 top-level modules
Examples:      165 .fj programs
Packages:      7 standard (fj-math/nn/hal/drivers/http/json/crypto)
Playground:    playground/ (Vite + Monaco + Wasm)
LSP:           v3 (semantic tokens, refactoring, diagnostics)
Profiler:      Instrumentation + flamegraph + memory + time-travel
FFI:           C++ + Python + Rust interop
Verification:  Spec language + SMT + tensor shape proofs
RT Pipeline:   Sensor → inference → actuator (latency-guaranteed)
Stdlib v3:     Networking, crypto, data formats, system utilities
OS — Nova:     v2.0 Phoenix (GUI) + v3.0 Aurora (SMP, USB, compositor)
OS — Surya:    354 tasks remaining (ARM64 microkernel, needs Q6A)
Hardware:      RTX 4090, Dragon Q6A (QCS6490), QEMU x86/arm64/riscv
```

---

## Option Summary

| # | Option | Sprints | Tasks | Effort | Focus |
|---|--------|---------|-------|--------|-------|
| 1 | Distributed Computing & Actor Model | 8 | 80 | ~16 hrs | Scale-out |
| 2 | WASI Preview 2 & Component Model | 6 | 60 | ~12 hrs | Portability |
| 3 | GPU Training Pipeline (CUDA/ROCm) | 8 | 80 | ~16 hrs | Performance |
| 4 | Advanced Type System (HKT, GADTs) | 6 | 60 | ~12 hrs | Expressiveness |
| 5 | Incremental Compilation & Build System | 6 | 60 | ~12 hrs | Developer speed |
| 6 | Cloud & Edge Deployment | 7 | 70 | ~14 hrs | Production |
| 7 | Plugin & Extension System | 5 | 50 | ~10 hrs | Ecosystem |
| 8 | Cross-Platform CI/CD & Release | 5 | 50 | ~10 hrs | Quality |
| 9 | FajarOS Surya Continuation (ARM64) | 10 | 100 | ~20 hrs | OS ambition |
| 10 | Quality Audit & Stabilization | 7 | 70 | ~14 hrs | Foundation |
| **Total** | | **68** | **680** | **~136 hrs** | |

**Recommended order:** 10 → 8 → 5 → 2 → 1 → 3 → 4 → 6 → 7 → 9

---

## Option 1: Distributed Computing & Actor Model (8 sprints, 80 tasks)

**Goal:** Run Fajar Lang across multiple machines — actor model, message passing, distributed ML
**Impact:** Enables distributed training, fleet management, edge-cloud pipelines

### Phase DC1: Actor Model (3 sprints, 30 tasks)

#### Sprint DC1.1: Actor Core (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| DC1.1.1 | Actor trait | `trait Actor { fn receive(msg: Message) }` | [ ] |
| DC1.1.2 | Mailbox (bounded MPSC) | Per-actor message queue with backpressure | [ ] |
| DC1.1.3 | Actor spawning | `spawn<A: Actor>(actor: A) -> ActorRef` | [ ] |
| DC1.1.4 | ActorRef (typed handle) | `ref.send(msg)`, `ref.ask(msg) -> Future<Reply>` | [ ] |
| DC1.1.5 | Actor lifecycle | PreStart → Running → PostStop hooks | [ ] |
| DC1.1.6 | Supervision strategy | OneForOne, AllForOne, restart limits | [ ] |
| DC1.1.7 | Actor hierarchy | Parent → children tree, supervised restarts | [ ] |
| DC1.1.8 | Dead letter queue | Messages to stopped actors | [ ] |
| DC1.1.9 | Actor scheduler | Work-stealing thread pool for actor execution | [ ] |
| DC1.1.10 | 10 actor tests | Spawn, send, ask, supervision, lifecycle | [ ] |

#### Sprint DC1.2: Messaging Patterns (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| DC1.2.1 | Request-reply | `ask()` with timeout | [ ] |
| DC1.2.2 | Pub/sub | Topic-based broadcast | [ ] |
| DC1.2.3 | Router (round-robin) | Distribute work across N actors | [ ] |
| DC1.2.4 | Router (consistent hash) | Route by key to same actor | [ ] |
| DC1.2.5 | Aggregator | Collect replies from N actors | [ ] |
| DC1.2.6 | Circuit breaker | Fail-fast when target overwhelmed | [ ] |
| DC1.2.7 | Backpressure protocol | Flow control between actors | [ ] |
| DC1.2.8 | Priority mailbox | Priority queue for messages | [ ] |
| DC1.2.9 | Stash/unstash | Defer messages during state transition | [ ] |
| DC1.2.10 | Message serialization | serde for cross-process messages | [ ] |

#### Sprint DC1.3: Concurrency Primitives (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| DC1.3.1 | Actor timer | Scheduled messages (once, periodic) | [ ] |
| DC1.3.2 | FSM (finite state machine) | Actor with typed state transitions | [ ] |
| DC1.3.3 | Event sourcing | Persist events, replay for recovery | [ ] |
| DC1.3.4 | Saga pattern | Distributed transaction with compensations | [ ] |
| DC1.3.5 | Actor persistence | Save/restore actor state to disk | [ ] |
| DC1.3.6 | Cluster singleton | Exactly one instance across cluster | [ ] |
| DC1.3.7 | Sharding | Automatic actor distribution by entity ID | [ ] |
| DC1.3.8 | Stream processing | Actor-based data stream pipeline | [ ] |
| DC1.3.9 | Batched processing | Accumulate messages, process in batch | [ ] |
| DC1.3.10 | Actor benchmarks | Throughput, latency, overhead | [ ] |

### Phase DC2: Cluster & Distribution (3 sprints, 30 tasks)

#### Sprint DC2.1: Cluster Formation (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| DC2.1.1 | Node discovery | DNS seed, static list, mDNS | [ ] |
| DC2.1.2 | Gossip protocol | Membership + failure detection (SWIM) | [ ] |
| DC2.1.3 | Cluster join/leave | Graceful join with state sync | [ ] |
| DC2.1.4 | Split-brain resolver | Quorum-based, keep-majority | [ ] |
| DC2.1.5 | Node roles | Leader, worker, seed, client | [ ] |
| DC2.1.6 | Heartbeat | Failure detection with phi-accrual | [ ] |
| DC2.1.7 | Cluster events | MemberJoined, MemberLeft, MemberUnreachable | [ ] |
| DC2.1.8 | Serialized transport | TCP + TLS between nodes | [ ] |
| DC2.1.9 | Cluster metrics | Node count, message throughput, latency | [ ] |
| DC2.1.10 | Cluster tests | Split-brain, recovery, partition tolerance | [ ] |

#### Sprint DC2.2: Remote Actors (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| DC2.2.1 | Remote actor ref | `ActorRef` that sends over network | [ ] |
| DC2.2.2 | Location transparency | Same API for local and remote actors | [ ] |
| DC2.2.3 | Remote deployment | `spawn_on(node, actor)` | [ ] |
| DC2.2.4 | Actor migration | Move actor state to another node | [ ] |
| DC2.2.5 | Remote supervision | Supervise actors on other nodes | [ ] |
| DC2.2.6 | Cluster-aware router | Route to actors across cluster | [ ] |
| DC2.2.7 | gRPC transport | Protobuf serialization for messages | [ ] |
| DC2.2.8 | mTLS authentication | Mutual TLS for node-to-node communication | [ ] |
| DC2.2.9 | Message deduplication | Exactly-once delivery guarantee | [ ] |
| DC2.2.10 | Remote actor benchmarks | Cross-node latency, throughput | [ ] |

#### Sprint DC2.3: Distributed ML (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| DC2.3.1 | Data-parallel training | Split dataset across workers | [ ] |
| DC2.3.2 | Gradient aggregation | AllReduce via ring or tree topology | [ ] |
| DC2.3.3 | Parameter server | Centralized weight storage + update | [ ] |
| DC2.3.4 | Model-parallel training | Split model layers across nodes | [ ] |
| DC2.3.5 | Pipeline parallelism | Forward/backward across pipeline stages | [ ] |
| DC2.3.6 | Elastic training | Add/remove workers during training | [ ] |
| DC2.3.7 | Checkpoint + recovery | Save training state, resume on failure | [ ] |
| DC2.3.8 | Federated learning | Train on-device, aggregate centrally | [ ] |
| DC2.3.9 | Distributed inference | Shard model across edge devices | [ ] |
| DC2.3.10 | Distributed training benchmark | MNIST across 4 nodes | [ ] |

### Phase DC3: Integration (2 sprints, 20 tasks)

#### Sprint DC3.1: CLI & Configuration (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| DC3.1.1 | `fj cluster status` CLI | Show cluster health, node count, roles | [x] |
| DC3.1.2 | `fj run --nodes N` CLI | Execute program across N cluster nodes | [x] |
| DC3.1.3 | `fj deploy --cluster` CLI | Deploy application to cluster | [x] |
| DC3.1.4 | cluster.toml configuration | Node addresses, roles, transport settings | [x] |
| DC3.1.5 | Cluster dashboard | Real-time cluster view with node health | [x] |
| DC3.1.6 | Actor distribution view | Visualize actor placement across nodes | [x] |
| DC3.1.7 | Message throughput metrics | Real-time message rate per node/actor | [x] |
| DC3.1.8 | Cluster event log | Persistent log of join/leave/failure events | [x] |
| DC3.1.9 | Node auto-discovery config | mDNS/DNS-SD configuration in cluster.toml | [x] |
| DC3.1.10 | Cluster CLI tests | 10 tests for cluster CLI commands | [x] |

#### Sprint DC3.2: Documentation & Examples (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| DC3.2.1 | Actor model guide | Tutorial: actors, mailboxes, supervision | [x] |
| DC3.2.2 | Cluster setup guide | Step-by-step cluster configuration | [x] |
| DC3.2.3 | Distributed ML tutorial | End-to-end distributed training walkthrough | [x] |
| DC3.2.4 | Chat server example | Multi-room chat using actors | [x] |
| DC3.2.5 | Distributed MapReduce example | Word count across cluster nodes | [x] |
| DC3.2.6 | Fleet management example | IoT fleet with actor-per-device | [x] |
| DC3.2.7 | Edge-cloud inference pipeline | Sensor → edge inference → cloud aggregate | [x] |
| DC3.2.8 | Distributed sensor fusion | Multi-sensor fusion across nodes | [x] |
| DC3.2.9 | Blog: "Distributed ML in Fajar Lang" | Technical blog post with benchmarks | [x] |
| DC3.2.10 | Integration tests | End-to-end distributed scenario tests | [x] |

---

## Option 2: WASI Preview 2 & Component Model (6 sprints, 60 tasks)

**Goal:** First-class WASI P2 support — components, WIT interfaces, sandboxed modules
**Impact:** Deploy Fajar Lang to any WASI-compatible runtime (Wasmtime, WasmEdge, Spin)

### Phase W1: WASI Preview 2 Core (2 sprints, 20 tasks)

#### Sprint W1.1: WASI Interfaces (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| W1.1.1 | wasi:cli/run interface | Entry point + environment + exit code | [x] |
| W1.1.2 | wasi:io/streams | Input/output stream abstractions | [x] |
| W1.1.3 | wasi:filesystem/preopens | Filesystem access with capability handles | [x] |
| W1.1.4 | wasi:sockets/tcp+udp | TCP and UDP socket interfaces | [x] |
| W1.1.5 | wasi:clocks | wall-clock + monotonic-clock interfaces | [x] |
| W1.1.6 | wasi:random/random | Cryptographic random number generation | [x] |
| W1.1.7 | wasi:http handlers | incoming-handler + outgoing-handler | [x] |
| W1.1.8 | stdin/stdout/stderr mapping | Standard I/O stream binding to WASI | [x] |
| W1.1.9 | File descriptor management | FD table, preopens, capability checks | [x] |
| W1.1.10 | WASI interfaces tests | 10 tests for WASI interface compliance | [x] |

#### Sprint W1.2: Runtime Plumbing (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| W1.2.1 | Environment variables | WASI env var access API | [x] |
| W1.2.2 | Command-line args | WASI args_get / args_sizes_get | [x] |
| W1.2.3 | Pollable abstraction | Async I/O via wasi:io/poll | [x] |
| W1.2.4 | Capability-based security | Restrict access per capability handle | [x] |
| W1.2.5 | Resource handle lifecycle | Create, borrow, drop resource handles | [x] |
| W1.2.6 | WASI adapter layer | Preview 1 → Preview 2 shim layer | [x] |
| W1.2.7 | wasi-sdk integration | Build toolchain integration with wasi-sdk | [x] |
| W1.2.8 | WASI test suite compliance | Pass official WASI test suite | [x] |
| W1.2.9 | WASI P2 conformance report | Spec compliance documentation | [x] |
| W1.2.10 | WASI runtime plumbing tests | 10 tests for runtime integration | [x] |

### Phase W2: Component Model (2 sprints, 20 tasks)

#### Sprint W2.1: WIT Parser & Generator (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| W2.1.1 | WIT parser | Parse Wasm Interface Type definitions | [x] |
| W2.1.2 | WIT → Fajar binding generator | Generate .fj bindings from WIT files | [x] |
| W2.1.3 | Component binary format | Module → component wrapper encoding | [x] |
| W2.1.4 | Interface imports/exports | Declare component imports and exports | [x] |
| W2.1.5 | Resource types | Handle + drop for owned/borrowed resources | [x] |
| W2.1.6 | WIT type mapping | variant/record/enum/flags/tuple/list/option/result | [x] |
| W2.1.7 | Canonical ABI (lift/lower) | Lift and lower functions per canonical ABI | [x] |
| W2.1.8 | Component composition | Link multiple components together | [x] |
| W2.1.9 | Component dependency resolution | Resolve component import chains | [x] |
| W2.1.10 | WIT parser tests | 10 tests for WIT parsing and codegen | [x] |

#### Sprint W2.2: Component Tooling (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| W2.2.1 | `fj build --component` CLI | Build Fajar program as Wasm component | [x] |
| W2.2.2 | Component metadata | Name, version, interfaces in component | [x] |
| W2.2.3 | Component validation | Type-check imports/exports at link time | [x] |
| W2.2.4 | Cross-language linking | Fajar + Rust + Go component interop | [x] |
| W2.2.5 | Component registry (warg) | Publish/fetch components via warg protocol | [x] |
| W2.2.6 | Shared-nothing isolation | Memory isolation between components | [x] |
| W2.2.7 | Capability injection | Instantiate with injected capabilities | [x] |
| W2.2.8 | WIT documentation generator | Generate docs from WIT definitions | [x] |
| W2.2.9 | Component size optimization | Tree-shaking + dead code elimination | [x] |
| W2.2.10 | Component model tests | Canonical ABI benchmark + integration tests | [x] |

### Phase W3: Runtime & Deployment (2 sprints, 20 tasks)

#### Sprint W3.1: Runtime Integration (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| W3.1.1 | Wasmtime integration | Embed Wasmtime runtime for WASI P2 | [x] |
| W3.1.2 | WasmEdge integration | WasmEdge as alternative WASI runtime | [x] |
| W3.1.3 | Spin (Fermyon) template | Serverless app template for Fermyon Spin | [x] |
| W3.1.4 | Cloudflare Workers deploy | Deploy Fajar Wasm to Cloudflare Workers | [x] |
| W3.1.5 | Fastly Compute@Edge deploy | Deploy to Fastly Compute@Edge | [x] |
| W3.1.6 | AWS Lambda custom runtime | Wasm-based Lambda custom runtime | [x] |
| W3.1.7 | Deno/Node.js Wasm import | Import Fajar component from JS runtimes | [x] |
| W3.1.8 | Browser component instantiation | Load component in browser via JS API | [x] |
| W3.1.9 | Docker + Wasm shim | containerd-wasm-shim integration | [x] |
| W3.1.10 | Runtime integration tests | 10 tests for runtime embedding | [x] |

#### Sprint W3.2: Advanced WASI & Docs (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| W3.2.1 | WASI-NN for ML inference | ML inference via WASI-NN proposal | [x] |
| W3.2.2 | WASI-threads | Parallel computation via WASI-threads | [x] |
| W3.2.3 | wasi:keyvalue interface | Key-value store interface binding | [x] |
| W3.2.4 | wasi:messaging interface | Message queue interface binding | [x] |
| W3.2.5 | wasi:observe/tracing | Observability and tracing interface | [x] |
| W3.2.6 | Component hot-reload | No-downtime component replacement | [x] |
| W3.2.7 | Serverless + edge templates | Project templates for serverless/edge | [x] |
| W3.2.8 | IoT device deployment | Deploy Wasm to constrained IoT devices | [x] |
| W3.2.9 | WASI P2 benchmark suite | Performance benchmarks for all interfaces | [x] |
| W3.2.10 | Blog: "Fajar Lang on WASI" | Technical blog with deployment examples | [x] |

---

## Option 3: GPU Training Pipeline (CUDA/ROCm) (8 sprints, 80 tasks)

**Goal:** Real GPU-accelerated training — not just inference, full backward pass on GPU
**Impact:** 10-100x training speedup; competitive with PyTorch for small-medium models

### Phase GT1: GPU Kernel Compilation (3 sprints, 30 tasks)

#### Sprint GT1.1: CUDA/ROCm Codegen (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| GT1.1.1 | CUDA PTX code generation | Compile @device functions to PTX | [x] |
| GT1.1.2 | ROCm HIP code generation | Compile @device functions to HIP/AMDGPU | [x] |
| GT1.1.3 | Kernel launch configuration | Grid/block dimensions, shared memory size | [x] |
| GT1.1.4 | Shared memory allocation | Per-block shared memory for tiled ops | [x] |
| GT1.1.5 | Warp-level primitives | Shuffle, vote, ballot intrinsics | [x] |
| GT1.1.6 | Atomic operations | atomicAdd, atomicCAS, atomicMax on GPU | [x] |
| GT1.1.7 | Texture + constant memory | Read-only data and hyperparameter storage | [x] |
| GT1.1.8 | Dynamic parallelism | Kernel launches kernel (nested dispatch) | [x] |
| GT1.1.9 | Cooperative groups | Thread block clusters, grid sync | [x] |
| GT1.1.10 | CUDA codegen tests | 10 tests for PTX/HIP generation | [x] |

#### Sprint GT1.2: Memory & Streams (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| GT1.2.1 | Multi-stream execution | Overlap compute + H2D/D2H transfers | [x] |
| GT1.2.2 | Unified memory | Managed memory with automatic migration | [x] |
| GT1.2.3 | NVRTC integration | Runtime compilation of PTX kernels | [x] |
| GT1.2.4 | PTX JIT cache | Cache compiled PTX for reuse | [x] |
| GT1.2.5 | Kernel occupancy calculator | Predict SM occupancy from launch config | [x] |
| GT1.2.6 | Register pressure analysis | Detect excessive register usage | [x] |
| GT1.2.7 | Bank conflict detection | Shared memory access pattern analysis | [x] |
| GT1.2.8 | Coalesced memory analysis | Global memory coalescing verification | [x] |
| GT1.2.9 | Custom CUDA allocator | Pooled GPU memory allocator | [x] |
| GT1.2.10 | GPU memory stream tests | 10 tests for memory management | [x] |

#### Sprint GT1.3: Tensor Cores & Multi-GPU (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| GT1.3.1 | FP16/BF16 tensor cores | Ampere/Ada tensor core WMMA | [x] |
| GT1.3.2 | TF32 tensor cores | TF32 mode for Ampere+ GPUs | [x] |
| GT1.3.3 | INT8 tensor cores | Quantized training via INT8 WMMA | [x] |
| GT1.3.4 | Async memcpy (H2D, D2H) | Non-blocking host-device transfers | [x] |
| GT1.3.5 | Peer-to-peer GPU comm | NVLink direct GPU-to-GPU transfer | [x] |
| GT1.3.6 | GPU error checking | CUDA error codes, async error query | [x] |
| GT1.3.7 | Multi-GPU detection | nvidia-smi parsing, device enumeration | [x] |
| GT1.3.8 | GPU memory manager | Allocation tracking, fragmentation stats | [x] |
| GT1.3.9 | Kernel profiling | nvprof/nsight integration for profiling | [x] |
| GT1.3.10 | Kernel auto-tuning + cache | Auto-tune block size, cache compiled kernels | [x] |

### Phase GT2: Training Operators (3 sprints, 30 tasks)

#### Sprint GT2.1: Forward Operators (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| GT2.1.1 | GPU matmul | Tiled matmul with shared memory | [x] |
| GT2.1.2 | GPU conv2d | im2col + GEMM convolution | [x] |
| GT2.1.3 | GPU batch normalization | Fused forward + running stats | [x] |
| GT2.1.4 | GPU dropout | curand-based stochastic dropout | [x] |
| GT2.1.5 | GPU attention | Flash attention algorithm (O(N) memory) | [x] |
| GT2.1.6 | GPU softmax | Online numerically stable softmax | [x] |
| GT2.1.7 | GPU cross-entropy + MSE loss | Loss functions on GPU | [x] |
| GT2.1.8 | GPU layer norm + GELU | Normalization and activation kernels | [x] |
| GT2.1.9 | GPU embedding lookup | Sparse embedding table lookup | [x] |
| GT2.1.10 | GPU pooling + reshape | Max/avg pool, transpose, concat, split | [x] |

#### Sprint GT2.2: Backward Operators (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| GT2.2.1 | GPU backward: matmul grad | Gradient of matrix multiplication | [x] |
| GT2.2.2 | GPU backward: conv2d grad | Gradient of convolution (input + weight) | [x] |
| GT2.2.3 | GPU backward: batch norm grad | Gradient of batch normalization | [x] |
| GT2.2.4 | GPU backward: attention grad | Gradient of flash attention | [x] |
| GT2.2.5 | GPU optimizer: SGD | SGD with momentum on GPU tensors | [x] |
| GT2.2.6 | GPU optimizer: Adam/AdamW | Adam with decoupled weight decay on GPU | [x] |
| GT2.2.7 | GPU learning rate scheduler | StepLR, CosineAnnealing, WarmupLR | [x] |
| GT2.2.8 | GPU gradient clipping | Clip by norm and by value on GPU | [x] |
| GT2.2.9 | GPU mixed precision training | FP16 forward, FP32 accumulate, loss scaling | [x] |
| GT2.2.10 | GPU gradient checkpointing | Recompute vs store trade-off | [x] |

#### Sprint GT2.3: Library Integration (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| GT2.3.1 | cuBLAS integration | High-performance GEMM via cuBLAS | [x] |
| GT2.3.2 | cuDNN integration | Conv/pool/norm via cuDNN | [x] |
| GT2.3.3 | NCCL integration | Multi-GPU allreduce for distributed | [x] |
| GT2.3.4 | Training loop orchestrator | Epoch/batch loop with GPU sync | [x] |
| GT2.3.5 | Data loader (GPU prefetch) | Async data loading with GPU prefetch | [x] |
| GT2.3.6 | Model save/load (GPU) | Checkpoint GPU model state to disk | [x] |
| GT2.3.7 | GPU MNIST training | End-to-end MNIST on GPU (>95% acc) | [x] |
| GT2.3.8 | GPU CIFAR-10 training | End-to-end CIFAR-10 on GPU | [x] |
| GT2.3.9 | GPU operator benchmarks | Throughput comparison vs CPU | [x] |
| GT2.3.10 | GPU training tests | 10 integration tests for GPU training | [x] |

### Phase GT3: Integration & Optimization (2 sprints, 20 tasks)

#### Sprint GT3.1: Auto Dispatch & Scheduling (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| GT3.1.1 | Automatic GPU/CPU dispatch | Route ops to GPU or CPU based on tensor size | [x] |
| GT3.1.2 | Lazy evaluation DAG | Build computation graph before execution | [x] |
| GT3.1.3 | Operation fusion | Fuse conv+bn+relu into single kernel | [x] |
| GT3.1.4 | Memory planning (liveness) | Reuse GPU memory based on tensor lifetime | [x] |
| GT3.1.5 | Gradient accumulation | Micro-batching for large effective batch size | [x] |
| GT3.1.6 | Async data loading | Prefetch next batch to GPU during training | [x] |
| GT3.1.7 | GPU-aware data pipeline | End-to-end data loading → GPU tensor | [x] |
| GT3.1.8 | TensorBoard logging | TensorBoard-compatible scalar/histogram logs | [x] |
| GT3.1.9 | GPU memory defragmentation | Compact GPU memory on fragmentation | [x] |
| GT3.1.10 | GPU thermal throttle detection | Detect and warn on thermal throttling | [x] |

#### Sprint GT3.2: Deployment & Docs (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| GT3.2.1 | Distributed GPU training | Data-parallel training across multiple GPUs | [x] |
| GT3.2.2 | Model export (GPU → ONNX) | Export trained GPU model to ONNX format | [x] |
| GT3.2.3 | GPU inference server | Batched request inference server | [x] |
| GT3.2.4 | A/B model testing | Compare model versions in production | [x] |
| GT3.2.5 | `fj train --gpu` CLI | Train on GPU with single command | [x] |
| GT3.2.6 | `fj train --gpus --distributed` | Multi-GPU distributed training CLI | [x] |
| GT3.2.7 | Checkpoint save/load | GPU → disk → GPU checkpoint round-trip | [x] |
| GT3.2.8 | Training progress visualization | Real-time loss/accuracy plots | [x] |
| GT3.2.9 | GPU training documentation | Setup guide for CUDA/ROCm/Metal | [x] |
| GT3.2.10 | Blog: "GPU Training in Fajar Lang" | Technical blog with benchmarks | [x] |

---

## Option 4: Advanced Type System (HKT, GADTs) (6 sprints, 60 tasks)

**Goal:** Higher-kinded types, GADTs, type-level programming — Haskell-level expressiveness
**Impact:** Enable generic programming patterns impossible in Rust/Go (Functor, Monad, etc.)

### Phase AT1: Higher-Kinded Types (2 sprints, 20 tasks)

#### Sprint AT1.1: Kind System (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| AT1.1.1 | Type constructor parameters | `F<_>` where F is a type-level parameter | [x] |
| AT1.1.2 | Functor trait | `trait Functor<F> { fn map<A,B>(fa: F<A>, f: fn(A)->B) -> F<B> }` | [x] |
| AT1.1.3 | Applicative trait | `trait Applicative<F>: Functor<F> { fn pure<A>(a: A) -> F<A> }` | [x] |
| AT1.1.4 | Monad trait | `flat_map<A,B>(fa: F<A>, f: fn(A)->F<B>) -> F<B>` | [x] |
| AT1.1.5 | Traverse trait | `traverse<F,A,B>(...)` for effectful traversal | [x] |
| AT1.1.6 | Kind system | Kinds: `* → *`, `* → * → *`, kind checking | [x] |
| AT1.1.7 | Kind inference | Infer kinds from usage context | [x] |
| AT1.1.8 | Kind error messages | Clear errors for kind mismatches | [x] |
| AT1.1.9 | HKT monomorphization | Monomorphize HKT at instantiation sites | [x] |
| AT1.1.10 | Kind system tests | 10 tests for kind checking + inference | [x] |

#### Sprint AT1.2: HKT Instances & Sugar (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| AT1.2.1 | Option as Functor/Monad | Functor + Monad impl for Option<T> | [x] |
| AT1.2.2 | Result as Functor/Monad | Functor + Monad impl for Result<T,E> | [x] |
| AT1.2.3 | Array as Functor | Functor impl for Array<T> | [x] |
| AT1.2.4 | Future as Functor/Monad | Functor + Monad impl for Future<T> | [x] |
| AT1.2.5 | do-notation desugaring | `do { x <- read(); ... }` → flat_map chain | [x] |
| AT1.2.6 | Monad transformers | OptionT, ResultT for monad stacking | [x] |
| AT1.2.7 | Free monad | Free monad for effect interpretation | [x] |
| AT1.2.8 | HKT + effect system | Integration with existing effect system | [x] |
| AT1.2.9 | HKT specification update | Update language spec with HKT rules | [x] |
| AT1.2.10 | HKT integration tests | 10 tests for HKT instances + do-notation | [x] |

### Phase AT2: GADTs & Type-Level Programming (2 sprints, 20 tasks)

#### Sprint AT2.1: GADTs (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| AT2.1.1 | GADT syntax | `enum Expr<T> { IntLit(i64) : Expr<i64>, ... }` | [x] |
| AT2.1.2 | GADT pattern refinement | Refine type variable in match arms | [x] |
| AT2.1.3 | Type equality witnesses | Prove type equality via GADT constructors | [x] |
| AT2.1.4 | Type-level naturals | Zero, Succ<N> for compile-time numbers | [x] |
| AT2.1.5 | Type-level lists | Nil, Cons<H, T> for type-level sequences | [x] |
| AT2.1.6 | Type-level booleans | True, False with type-level If<B, T, F> | [x] |
| AT2.1.7 | Type-level arithmetic | Add<A, B>, Mul<A, B> at type level | [x] |
| AT2.1.8 | Singleton types | Value-to-type promotion for dependent typing | [x] |
| AT2.1.9 | GADT + exhaustiveness | Exhaustive match checking for GADTs | [x] |
| AT2.1.10 | GADT tests | 10 tests for GADT parsing + type checking | [x] |

#### Sprint AT2.2: Type-Level Applications (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| AT2.2.1 | Length-indexed vectors | `Vec<T, N>` where N is type-level natural | [x] |
| AT2.2.2 | Safe matrix operations | Dimension-typed matrix multiply | [x] |
| AT2.2.3 | Proof terms | Type-level witnesses for safety properties | [x] |
| AT2.2.4 | Dependent pairs | Existential types with proof obligations | [x] |
| AT2.2.5 | Type-safe printf | Format string checked at compile time | [x] |
| AT2.2.6 | Type-safe SQL builder | Schema encoded in types | [x] |
| AT2.2.7 | Type-safe state machine | Only valid transitions type-check | [x] |
| AT2.2.8 | GADT inference limits | Document and test inference boundaries | [x] |
| AT2.2.9 | GADT specification | Update language spec with GADT rules | [x] |
| AT2.2.10 | GADT examples | 5+ example programs using GADTs | [x] |

### Phase AT3: Type Classes & Deriving (2 sprints, 20 tasks)

#### Sprint AT3.1: Trait System Improvements (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| AT3.1.1 | Orphan rules (coherence) | Prevent conflicting impls across crates | [x] |
| AT3.1.2 | Associated type defaults | Default associated types in trait defs | [x] |
| AT3.1.3 | Blanket implementations | `impl<T: Debug> Display for T` | [x] |
| AT3.1.4 | Negative trait bounds | `!Send` for opt-out of auto traits | [x] |
| AT3.1.5 | Auto traits (markers) | Automatically implemented marker traits | [x] |
| AT3.1.6 | Specialization | More specific impl overrides generic | [x] |
| AT3.1.7 | Trait aliases | `trait ReadWrite = Read + Write` | [x] |
| AT3.1.8 | `impl Trait` in arg/return | Existential types in function signatures | [x] |
| AT3.1.9 | Sealed traits | Prevent external implementations | [x] |
| AT3.1.10 | Trait system tests | 10 tests for coherence + specialization | [x] |

#### Sprint AT3.2: Derive & Const Traits (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| AT3.2.1 | Derive macros | `#[derive(Debug, Clone, PartialEq, Hash)]` | [x] |
| AT3.2.2 | Custom derive | User-defined derive macro plugins | [x] |
| AT3.2.3 | Derive for enums | Auto-derive for enum variants | [x] |
| AT3.2.4 | Derive for generic structs | Derive with type parameter constraints | [x] |
| AT3.2.5 | Newtype derive | Transparent delegation for newtype wrappers | [x] |
| AT3.2.6 | Existential types | `type Foo = impl Bar` opaque types | [x] |
| AT3.2.7 | Type erasure patterns | Erase concrete types behind trait objects | [x] |
| AT3.2.8 | Const trait methods | Trait methods callable in const context | [x] |
| AT3.2.9 | Object safety rules | Comprehensive trait object safety checking | [x] |
| AT3.2.10 | Type class specification + tests | Spec update + 10 integration tests | [x] |

---

## Option 5: Incremental Compilation & Build System (6 sprints, 60 tasks)

**Goal:** Sub-second recompilation for small changes — file-level dependency tracking
**Impact:** Critical for developer experience; currently full rebuild on every change

### Phase IC1: Dependency Graph (2 sprints, 20 tasks)

#### Sprint IC1.1: Module Dependency (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| IC1.1.1 | Module dependency DAG | Build DAG from imports/exports | [x] |
| IC1.1.2 | File → module mapping | Map source files to module identifiers | [x] |
| IC1.1.3 | AST hashing | Detect unchanged files via content hash | [x] |
| IC1.1.4 | Signature hashing | Detect ABI-breaking changes via sig hash | [x] |
| IC1.1.5 | Incremental parsing | Only re-parse changed files | [x] |
| IC1.1.6 | Incremental analysis | Only re-check affected modules | [x] |
| IC1.1.7 | Query-based architecture | Salsa-style demand-driven computation | [x] |
| IC1.1.8 | Memoized query results | Cache query results for reuse | [x] |
| IC1.1.9 | Change propagation | Invalidate dependents on source change | [x] |
| IC1.1.10 | Module dependency tests | 10 tests for DAG + incremental parse | [x] |

#### Sprint IC1.2: Build Graph (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| IC1.2.1 | Parallel analysis | Per-module parallel type checking | [x] |
| IC1.2.2 | Dependency cycle detection | Detect and report circular imports | [x] |
| IC1.2.3 | Build order (topological sort) | Correct compilation order from DAG | [x] |
| IC1.2.4 | Cache directory (`.fj-cache/`) | Persistent cache on disk | [x] |
| IC1.2.5 | Cache serialization (bincode) | Fast binary serialization of cache entries | [x] |
| IC1.2.6 | Cache invalidation | Invalidate on compiler version change | [x] |
| IC1.2.7 | Stale cache cleanup | Remove unused cache entries | [x] |
| IC1.2.8 | `fj deps --graph` | Dependency graph visualization CLI | [x] |
| IC1.2.9 | `fj build --timings` | Build profiling with per-phase timing | [x] |
| IC1.2.10 | Build graph tests | 10 tests for parallel build + caching | [x] |

### Phase IC2: Compilation Cache (2 sprints, 20 tasks)

#### Sprint IC2.1: IR/Object Caching (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| IC2.1.1 | Per-function IR cache | Cache Cranelift IR per function | [x] |
| IC2.1.2 | Per-module object cache | Cache .o files per module | [x] |
| IC2.1.3 | Linking cache | Only re-link changed object files | [x] |
| IC2.1.4 | Content-addressable storage | Hash-based dedup for cache entries | [x] |
| IC2.1.5 | Shared cache (team-wide) | Team-wide cache directory sharing | [x] |
| IC2.1.6 | Remote cache (S3/GCS) | Upload/download cache from cloud storage | [x] |
| IC2.1.7 | Cache compression (zstd) | Compress cache entries with zstd | [x] |
| IC2.1.8 | Cache hit/miss metrics | Track and report cache effectiveness | [x] |
| IC2.1.9 | Cache size management | LRU eviction when cache exceeds limit | [x] |
| IC2.1.10 | IR/object cache tests | 10 tests for caching correctness | [x] |

#### Sprint IC2.2: Build Daemon & Hot-Reload (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| IC2.2.1 | Cache warming on CI | Pre-populate cache in CI pipeline | [x] |
| IC2.2.2 | Binary artifact cache | Cache final linked binaries | [x] |
| IC2.2.3 | Test result cache | Skip unchanged tests (hash-based) | [x] |
| IC2.2.4 | Incremental codegen | Only recompile changed functions | [x] |
| IC2.2.5 | Hot-reload | Replace module at runtime without restart | [x] |
| IC2.2.6 | File watcher integration | `fj watch` triggers incremental rebuild | [x] |
| IC2.2.7 | Build daemon | Persistent compiler process for fast rebuilds | [x] |
| IC2.2.8 | Build progress reporting | ETA, files remaining, current phase | [x] |
| IC2.2.9 | Parallel compilation | Per-module parallel code generation | [x] |
| IC2.2.10 | Build system benchmarks | Benchmark incremental vs full rebuild | [x] |

### Phase IC3: Package Build System (2 sprints, 20 tasks)

#### Sprint IC3.1: Build System (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| IC3.1.1 | Workspace support | Multi-package projects in one repo | [x] |
| IC3.1.2 | Build scripts (`build.fj`) | Pre-build code generation scripts | [x] |
| IC3.1.3 | Conditional compilation | `#[cfg(target_os = "linux")]` gates | [x] |
| IC3.1.4 | Feature flags | `[features]` section in fj.toml | [x] |
| IC3.1.5 | Build profiles | dev, release, bench profiles | [x] |
| IC3.1.6 | Cross-compilation targets | Target triple configuration | [x] |
| IC3.1.7 | Custom linker configuration | Linker flags and script specification | [x] |
| IC3.1.8 | Output types | binary, library, cdylib, staticlib, wasm | [x] |
| IC3.1.9 | Artifact naming conventions | Platform-specific output naming | [x] |
| IC3.1.10 | Build system core tests | 10 tests for workspace + profiles | [x] |

#### Sprint IC3.2: Build CLI & Docs (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| IC3.2.1 | Build metadata | Embed git hash, build time in binary | [x] |
| IC3.2.2 | Reproducible builds | Lockfile for deterministic builds | [x] |
| IC3.2.3 | `fj clean` | Remove build artifacts and cache | [x] |
| IC3.2.4 | `fj build --release` | Optimized release build | [x] |
| IC3.2.5 | `fj build --target wasm32-wasi` | Cross-compile to WASI target | [x] |
| IC3.2.6 | Package cache sharing | Share build cache between workspace packages | [x] |
| IC3.2.7 | Workspace dep deduplication | Deduplicate shared dependencies | [x] |
| IC3.2.8 | `fj test --changed` | Only test affected packages | [x] |
| IC3.2.9 | Build graph visualization | Visual package dependency graph | [x] |
| IC3.2.10 | Build system documentation + tests | Docs + integration tests | [x] |

---

## Option 6: Cloud & Edge Deployment (7 sprints, 70 tasks)

**Goal:** One-command deploy to cloud (AWS/GCP/Azure), edge (Cloudflare/Fastly), IoT
**Impact:** Bridge the gap from "it compiles" to "it runs in production"

### Phase CD1: Container & Serverless (3 sprints, 30 tasks)

#### Sprint CD1.1: Docker/Container Deploy (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| CD1.1.1 | Dockerfile generation | `fj deploy --dockerfile` auto-generates Dockerfile | [x] |
| CD1.1.2 | Multi-stage build | Builder stage + distroless runtime stage | [x] |
| CD1.1.3 | Docker Compose template | Multi-service compose configuration | [x] |
| CD1.1.4 | Kubernetes manifest gen | Deployment, Service, ConfigMap YAML | [x] |
| CD1.1.5 | Helm chart template | Parameterized Helm chart for K8s | [x] |
| CD1.1.6 | Container resource limits | CPU/memory limits in manifests | [x] |
| CD1.1.7 | Auto-scaling configuration | HPA configuration generation | [x] |
| CD1.1.8 | K8s liveness/readiness probes | Health endpoint + probe configuration | [x] |
| CD1.1.9 | Service mesh (Istio sidecar) | Istio annotation for traffic management | [x] |
| CD1.1.10 | Container deploy tests | 10 tests for Dockerfile + K8s generation | [x] |

#### Sprint CD1.2: Cloud Providers (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| CD1.2.1 | AWS Lambda deployment | Custom runtime (provided.al2023) | [x] |
| CD1.2.2 | Google Cloud Run deployment | Container-based serverless on GCP | [x] |
| CD1.2.3 | Azure Container Apps | Azure serverless container deployment | [x] |
| CD1.2.4 | Cloudflare Workers (Wasm) | Deploy as Cloudflare Worker | [x] |
| CD1.2.5 | Fastly Compute@Edge (Wasm) | Deploy to Fastly edge network | [x] |
| CD1.2.6 | Fly.io deployment | Global edge deployment via Fly.io | [x] |
| CD1.2.7 | Railway deployment | Railway.app one-click deploy | [x] |
| CD1.2.8 | Vercel Edge Functions | Deploy to Vercel edge runtime | [x] |
| CD1.2.9 | `fj deploy --aws-lambda` CLI | Single-command cloud deployment | [x] |
| CD1.2.10 | Cloud provider tests | 10 tests for provider configurations | [x] |

#### Sprint CD1.3: Production Features (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| CD1.3.1 | Health check endpoint | Auto-generated /health endpoint | [x] |
| CD1.3.2 | Graceful shutdown handler | SIGTERM handling with drain period | [x] |
| CD1.3.3 | Secret management | Env vars + mounted file secrets | [x] |
| CD1.3.4 | Structured logging (JSON) | JSON-formatted log output | [x] |
| CD1.3.5 | Tracing (OpenTelemetry) | Distributed tracing integration | [x] |
| CD1.3.6 | Metrics export (Prometheus) | /metrics endpoint for Prometheus | [x] |
| CD1.3.7 | Blue-green deployment | Zero-downtime deployment strategy | [x] |
| CD1.3.8 | Canary deployment | Gradual traffic shift to new version | [x] |
| CD1.3.9 | Rollback mechanism | One-command rollback to previous version | [x] |
| CD1.3.10 | Deployment docs + benchmarks | Deployment guide + latency benchmarks | [x] |

### Phase CD2: Edge & IoT (2 sprints, 20 tasks)

#### Sprint CD2.1: Edge Runtime (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| CD2.1.1 | Edge runtime (Wasm + WASI) | Lightweight WASI runtime for ARM edge | [x] |
| CD2.1.2 | IoT device provisioning | Zero-touch device onboarding | [x] |
| CD2.1.3 | OTA firmware update | Delta-encoded firmware updates | [x] |
| CD2.1.4 | Device fleet management | Fleet-wide deploy, monitor, update | [x] |
| CD2.1.5 | Device shadow (digital twin) | Cloud-side device state mirror | [x] |
| CD2.1.6 | MQTT client for telemetry | Publish sensor data via MQTT | [x] |
| CD2.1.7 | CoAP protocol | Constrained Application Protocol for IoT | [x] |
| CD2.1.8 | LwM2M device management | OMA LwM2M for device lifecycle | [x] |
| CD2.1.9 | Edge inference pipeline | Sensor → model → actuator on device | [x] |
| CD2.1.10 | Edge runtime tests | 10 tests for edge runtime + MQTT | [x] |

#### Sprint CD2.2: Fleet Operations (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| CD2.2.1 | Model fleet deployment | Staged rollout of ML models to fleet | [x] |
| CD2.2.2 | Edge-cloud sync | Batch upload telemetry to cloud | [x] |
| CD2.2.3 | Offline operation | Queue operations, sync when connected | [x] |
| CD2.2.4 | Power-aware scheduling | Battery-conscious task scheduling | [x] |
| CD2.2.5 | Sleep/wake management | Deep sleep with timed/event wakeup | [x] |
| CD2.2.6 | Watchdog timer integration | Hardware watchdog for crash recovery | [x] |
| CD2.2.7 | Secure boot chain | Verified boot from bootloader to app | [x] |
| CD2.2.8 | Device identity (X.509) | Per-device certificates for auth | [x] |
| CD2.2.9 | Remote debugging | Serial-over-network debug sessions | [x] |
| CD2.2.10 | Fleet dashboard + docs | Fleet monitoring UI + deployment docs | [x] |

### Phase CD3: Observability & Operations (2 sprints, 20 tasks)

#### Sprint CD3.1: Observability (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| CD3.1.1 | Structured logging | JSON log format with levels + context | [x] |
| CD3.1.2 | Distributed tracing | OpenTelemetry SDK + W3C Trace Context | [x] |
| CD3.1.3 | Metrics collection | Counters, gauges, histograms API | [x] |
| CD3.1.4 | Prometheus exporter | /metrics endpoint for scraping | [x] |
| CD3.1.5 | Grafana dashboard templates | Pre-built dashboards for common metrics | [x] |
| CD3.1.6 | Alerting rules | PagerDuty/Slack alert integration | [x] |
| CD3.1.7 | Error tracking (Sentry-style) | Automatic error capture + aggregation | [x] |
| CD3.1.8 | Performance monitoring (APM) | Application performance tracking | [x] |
| CD3.1.9 | Request tracing | End-to-end latency tracking | [x] |
| CD3.1.10 | Observability tests | 10 tests for logging + tracing + metrics | [x] |

#### Sprint CD3.2: Operations & Reliability (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| CD3.2.1 | ML model monitoring | Data drift + accuracy degradation detection | [x] |
| CD3.2.2 | Cost monitoring | Cloud spend tracking per service | [x] |
| CD3.2.3 | SLA tracking | Uptime and latency SLA monitoring | [x] |
| CD3.2.4 | Incident response runbooks | Auto-generated runbook from service config | [x] |
| CD3.2.5 | Chaos engineering | Fault injection for resilience testing | [x] |
| CD3.2.6 | Load testing | `fj bench --http --rps 10000` load generator | [x] |
| CD3.2.7 | Production profiling | Low-overhead sampling profiler | [x] |
| CD3.2.8 | Memory leak detection | Production memory leak monitoring | [x] |
| CD3.2.9 | Auto-remediation | Restart on OOM, circuit breaker | [x] |
| CD3.2.10 | Observability docs + integration tests | Documentation + end-to-end tests | [x] |

---

## Option 7: Plugin & Extension System (5 sprints, 50 tasks)

**Goal:** User-extensible compiler — custom lints, code generators, transformations
**Impact:** Community can extend the language without forking

### Phase PL1: Plugin Architecture (2 sprints, 20 tasks)

#### Sprint PL1.1: Compiler Plugins (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| PL1.1.1 | Plugin trait | `trait CompilerPlugin { fn name(); fn on_ast(); fn on_ir(); }` | [x] |
| PL1.1.2 | Plugin discovery | `[plugins]` section in fj.toml | [x] |
| PL1.1.3 | Plugin loading (dylib) | Dynamic library loading for plugins | [x] |
| PL1.1.4 | Plugin API versioning | Semver compatibility checks on load | [x] |
| PL1.1.5 | Plugin sandboxing | No filesystem access by default | [x] |
| PL1.1.6 | AST visitor API | Read-only AST traversal for plugins | [x] |
| PL1.1.7 | AST transformer API | Modify AST nodes via plugin hooks | [x] |
| PL1.1.8 | IR visitor/transformer API | Plugin hooks at IR level | [x] |
| PL1.1.9 | Diagnostic API | Plugins emit warnings/errors via API | [x] |
| PL1.1.10 | Plugin architecture tests | 10 tests for loading + sandboxing | [x] |

#### Sprint PL1.2: Plugin Ecosystem (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| PL1.2.1 | Code generation API | Plugins emit extra code into output | [x] |
| PL1.2.2 | Plugin configuration (TOML) | Per-plugin configuration in fj.toml | [x] |
| PL1.2.3 | Plugin dependency resolution | Resolve plugin version constraints | [x] |
| PL1.2.4 | Plugin registry | Publish/install plugins via registry | [x] |
| PL1.2.5 | Plugin testing framework | Test harness for plugin development | [x] |
| PL1.2.6 | Plugin documentation generator | Auto-generate docs from plugin metadata | [x] |
| PL1.2.7 | Built-in plugins | Unused code, complexity metric, naming lint | [x] |
| PL1.2.8 | Plugin lifecycle | init → per-file → per-function → finalize | [x] |
| PL1.2.9 | Plugin performance budget | Max 100ms per file enforcement | [x] |
| PL1.2.10 | Plugin API docs + error isolation | API documentation + crash isolation | [x] |

### Phase PL2: Custom Lints & Code Gen (2 sprints, 20 tasks)

#### Sprint PL2.1: Custom Lints (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| PL2.1.1 | Custom lint rules | Pattern matching on AST for lint detection | [x] |
| PL2.1.2 | Lint severity configuration | Error/warning/info per lint rule | [x] |
| PL2.1.3 | Lint suppression | `@allow(my_lint)` annotation support | [x] |
| PL2.1.4 | Auto-fix for custom lints | Suggested fix with auto-apply | [x] |
| PL2.1.5 | Security audit plugin | Detect hardcoded secrets and credentials | [x] |
| PL2.1.6 | Dependency vulnerability scanner | Check deps against CVE database | [x] |
| PL2.1.7 | Code style enforcer | Enforce naming conventions + patterns | [x] |
| PL2.1.8 | i18n string extraction | Extract translatable strings from source | [x] |
| PL2.1.9 | Accessibility checker | Check UI code for accessibility issues | [x] |
| PL2.1.10 | Custom lint tests | 10 tests for lint rules + suppression | [x] |

#### Sprint PL2.2: Code Generators (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| PL2.2.1 | Schema code generator | Generate .fj from data schema | [x] |
| PL2.2.2 | Protobuf code gen | Generate .fj from .proto files | [x] |
| PL2.2.3 | GraphQL schema code gen | Generate .fj types from GraphQL schema | [x] |
| PL2.2.4 | Database ORM code gen | Generate DB access layer from schema | [x] |
| PL2.2.5 | REST API client gen | Generate typed HTTP client from spec | [x] |
| PL2.2.6 | OpenAPI spec code gen | Generate server stubs from OpenAPI | [x] |
| PL2.2.7 | FFI binding generator | Auto-generate FFI bindings from C headers | [x] |
| PL2.2.8 | Test scaffolding generator | Generate test boilerplate for modules | [x] |
| PL2.2.9 | Migration script generator | Generate DB migration scripts | [x] |
| PL2.2.10 | Plugin marketplace concept | Design for community plugin sharing | [x] |

### Phase PL3: IDE & Tooling Extensions (1 sprint, 10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| PL3.1 | VS Code extension plugin API | Allow plugins to add diagnostics | [ ] |
| PL3.2 | Custom code actions from plugins | Plugin-provided quick fixes | [ ] |
| PL3.3 | Custom completion from plugins | Plugin-provided suggestions | [ ] |
| PL3.4 | Custom hover info from plugins | Plugin-provided documentation | [ ] |
| PL3.5 | Plugin-based code lens | Custom annotations above functions | [ ] |
| PL3.6 | Plugin-based inlay hints | Custom inline hints | [ ] |
| PL3.7 | Debugger extensions | Plugin-provided debug visualizers | [ ] |
| PL3.8 | REPL extensions | Plugin commands in REPL | [ ] |
| PL3.9 | Build tool extensions | Plugin-provided build steps | [ ] |
| PL3.10 | Plugin system documentation | How to write plugins | [ ] |

---

## Option 8: Cross-Platform CI/CD & Release (5 sprints, 50 tasks)

**Goal:** Green CI on all platforms, automated releases, binary distribution
**Impact:** Professionalism and trust — green badge = project is alive

### Phase CI1: CI Matrix (2 sprints, 20 tasks)

#### Sprint CI1.1: Platform Builds (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| CI1.1.1 | Linux x86_64 CI | Ubuntu 22.04 + 24.04 build/test | [x] |
| CI1.1.2 | macOS x86_64 + ARM64 CI | macOS Intel + Apple Silicon builds | [x] |
| CI1.1.3 | Windows x86_64 CI | MSVC + GNU toolchain builds | [x] |
| CI1.1.4 | Cross-compile aarch64-linux | ARM64 Linux cross-compilation | [x] |
| CI1.1.5 | Cross-compile armv7-linux | ARMv7 Linux cross-compilation | [x] |
| CI1.1.6 | Cross-compile riscv64gc-linux | RISC-V 64 cross-compilation | [x] |
| CI1.1.7 | Cross-compile wasm32-wasi | WASI target cross-compilation | [x] |
| CI1.1.8 | Cross-compile wasm32-unknown | Browser Wasm target | [x] |
| CI1.1.9 | Rust stable + nightly | Test on both Rust channels | [x] |
| CI1.1.10 | Platform build tests | Verify all targets produce valid binaries | [x] |

#### Sprint CI1.2: Quality Checks (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| CI1.2.1 | QEMU boot test (x86_64) | Boot FajarOS Nova in QEMU x86_64 | [x] |
| CI1.2.2 | QEMU boot test (aarch64) | Boot FajarOS in QEMU aarch64 | [x] |
| CI1.2.3 | cargo clippy (zero warnings) | Clippy lint gate in CI | [x] |
| CI1.2.4 | cargo fmt (check) | Format check gate in CI | [x] |
| CI1.2.5 | cargo doc (no warnings) | Documentation warning gate | [x] |
| CI1.2.6 | cargo test (default + native) | Full test suite in CI | [x] |
| CI1.2.7 | cargo test --features llvm | LLVM backend tests in CI | [x] |
| CI1.2.8 | Fuzz run (1M iterations) | Fuzz all targets per CI run | [x] |
| CI1.2.9 | Code coverage (tarpaulin) | Coverage report + badge | [x] |
| CI1.2.10 | Binary size + benchmark tracking | Track size and perf regressions | [x] |

### Phase CI2: Release Automation (2 sprints, 20 tasks)

#### Sprint CI2.1: Release Automation (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| CI2.1.1 | Semantic versioning enforcement | Validate version bump in PR | [x] |
| CI2.1.2 | CHANGELOG.md auto-generation | Generate changelog from commits | [x] |
| CI2.1.3 | Git tag creation | Auto-tag on release merge | [x] |
| CI2.1.4 | GitHub Release creation | Create release with notes + assets | [x] |
| CI2.1.5 | Binary artifact upload | Upload platform binaries to release | [x] |
| CI2.1.6 | Platform-specific archives | tar.gz (Linux/macOS), zip (Windows) | [x] |
| CI2.1.7 | GitHub Actions workflow | Complete release pipeline in Actions | [x] |
| CI2.1.8 | Release notes generation | Auto-generate from conventional commits | [x] |
| CI2.1.9 | Pre-release channel | alpha/beta/rc version support | [x] |
| CI2.1.10 | Release signing | GPG/sigstore signature for binaries | [x] |

#### Sprint CI2.2: Package Distribution (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| CI2.2.1 | Homebrew formula | Auto-generate Homebrew formula | [x] |
| CI2.2.2 | AUR PKGBUILD | Arch Linux package generation | [x] |
| CI2.2.3 | Snap package | Ubuntu Snap store package | [x] |
| CI2.2.4 | Debian .deb package | .deb package for Debian/Ubuntu | [x] |
| CI2.2.5 | RPM .rpm package | RPM package for Fedora/RHEL | [x] |
| CI2.2.6 | Docker image (multi-arch) | Multi-arch Docker image (amd64 + arm64) | [x] |
| CI2.2.7 | crates.io publish | Publish Rust library crate | [x] |
| CI2.2.8 | npm publish (wasm) | Publish Wasm package to npm | [x] |
| CI2.2.9 | PyPI publish | Publish Python bindings to PyPI | [x] |
| CI2.2.10 | cargo-binstall support | Pre-built binary install support | [x] |

### Phase CI3: Quality Gates (1 sprint, 10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| CI3.1 | Minimum test coverage gate | Fail if < 85% coverage | [ ] |
| CI3.2 | Performance regression gate | Fail if > 10% slower | [ ] |
| CI3.3 | Binary size regression gate | Fail if > 5% larger | [ ] |
| CI3.4 | Dependency audit | cargo audit (CVE check) | [ ] |
| CI3.5 | License compliance | cargo deny (check licenses) | [ ] |
| CI3.6 | Documentation coverage | All public items documented | [ ] |
| CI3.7 | Semver compatibility | cargo semver-checks | [ ] |
| CI3.8 | Fuzz regression | Run fuzz corpus (no new crashes) | [ ] |
| CI3.9 | MSRV verification | Minimum Supported Rust Version | [ ] |
| CI3.10 | Release checklist | Automated pre-release verification | [ ] |

---

## Option 9: FajarOS Surya Continuation (10 sprints, 100 tasks)

**Goal:** Continue the ARM64 microkernel — complete remaining 354 tasks (prioritized top 100)
**Impact:** World's first OS with compiler-enforced ML safety guarantees

### Phase OS1: Kernel Core (3 sprints, 30 tasks)

#### Sprint OS1.1: Kernel Completion (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| OS1.1.1 | MMU completion | 4KB + 64KB page support, TTBR0/TTBR1 split | [x] |
| OS1.1.2 | Process address space isolation | Per-process page tables | [x] |
| OS1.1.3 | Exception vector table (EL1) | EL1 exception handlers for sync/IRQ/FIQ | [x] |
| OS1.1.4 | SVC syscall handler | EL0 → EL1 syscall dispatch | [x] |
| OS1.1.5 | Interrupt controller (GICv3) | GICv3 + GICv2 fallback | [x] |
| OS1.1.6 | Timer interrupt | ARM Generic Timer for preemption | [x] |
| OS1.1.7 | Context switch | Save/restore X0-X30, SP, PCTX, FPSIMD | [x] |
| OS1.1.8 | Process creation (fork/exec) | fork() + exec() with ELF loading | [x] |
| OS1.1.9 | ELF loader | aarch64 ELF relocations + PT_LOAD | [x] |
| OS1.1.10 | Kernel core tests | 10 tests for MMU + exceptions + context switch | [x] |

#### Sprint OS1.2: Scheduler & IPC (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| OS1.2.1 | Preemptive scheduler | Round-robin with priority levels | [x] |
| OS1.2.2 | IPC (message passing) | Kernel-mediated message passing | [x] |
| OS1.2.3 | Shared memory | mmap-based shared memory regions | [x] |
| OS1.2.4 | Signal delivery | SIGKILL, SIGTERM, SIGCHLD handlers | [x] |
| OS1.2.5 | waitpid | Process wait with status collection | [x] |
| OS1.2.6 | Process groups | Process group management + session leader | [x] |
| OS1.2.7 | Orphan reaping | Clean up orphaned child processes | [x] |
| OS1.2.8 | Kernel heap (buddy allocator) | Page-level buddy allocation | [x] |
| OS1.2.9 | Slab allocator | Small object allocation (32-4096 bytes) | [x] |
| OS1.2.10 | Scheduler + IPC tests | 10 tests for scheduling + message passing | [x] |

#### Sprint OS1.3: Boot & Infrastructure (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| OS1.3.1 | Kernel stack (per-process) | 16KB stack with guard pages | [x] |
| OS1.3.2 | Stack canary | Stack smashing detection | [x] |
| OS1.3.3 | Kernel panic handler | Panic with register dump + backtrace | [x] |
| OS1.3.4 | Kernel logging (serial UART) | Early console + serial log output | [x] |
| OS1.3.5 | Device tree (DTB) parsing | Parse flattened device tree blob | [x] |
| OS1.3.6 | Clock tree initialization | Configure PLLs and clock dividers | [x] |
| OS1.3.7 | Power domain management | Enable/disable power domains | [x] |
| OS1.3.8 | Kernel module framework | Loadable kernel module support | [x] |
| OS1.3.9 | Syscall table (40+ entries) | Full syscall dispatch table | [x] |
| OS1.3.10 | Kernel test framework + docs | In-kernel assert + documentation | [x] |

### Phase OS2: Drivers & Hardware (3 sprints, 30 tasks)

#### Sprint OS2.1: Device Drivers (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| OS2.1.1 | UART driver (PL011/QUP) | Serial console for debug output | [x] |
| OS2.1.2 | GPIO driver (gpiochip4) | GPIO read/write on Q6A pinout | [x] |
| OS2.1.3 | I2C driver (QUP) | I2C master for sensor communication | [x] |
| OS2.1.4 | SPI driver (QUP) | SPI master for flash/display | [x] |
| OS2.1.5 | PWM driver | Pulse-width modulation for motors/LEDs | [x] |
| OS2.1.6 | SD/eMMC driver (SDHCI) | Block storage via SDHCI controller | [x] |
| OS2.1.7 | NVMe driver (PCIe) | NVMe block device over PCIe | [x] |
| OS2.1.8 | USB XHCI driver | USB 3.0 host controller driver | [x] |
| OS2.1.9 | Watchdog timer driver | Hardware watchdog for crash recovery | [x] |
| OS2.1.10 | Device driver tests | 10 tests for UART + GPIO + I2C + SPI | [x] |

#### Sprint OS2.2: Peripheral Drivers (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| OS2.2.1 | Display driver (DSI/HDMI) | Framebuffer via DPU controller | [x] |
| OS2.2.2 | Camera driver (ISP) | Camera capture via libcamera ISP | [x] |
| OS2.2.3 | WiFi driver (ath11k) | WCNSS/ath11k wireless networking | [x] |
| OS2.2.4 | Bluetooth driver | HCI over UART Bluetooth stack | [x] |
| OS2.2.5 | Audio driver (WSA codec) | Audio playback/capture via codec | [x] |
| OS2.2.6 | GPU driver (Adreno 643) | Adreno GPU initialization + compute | [x] |
| OS2.2.7 | NPU driver (Hexagon) | Hexagon DSP/HTP neural processing | [x] |
| OS2.2.8 | RTC driver (DS1307) | Real-time clock read/write | [x] |
| OS2.2.9 | Thermal sensor driver | Temperature monitoring + throttling | [x] |
| OS2.2.10 | Peripheral driver tests | 10 tests for display + WiFi + GPU | [x] |

#### Sprint OS2.3: Bus & Infrastructure (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| OS2.3.1 | Power management IC (PMIC) | SPMI/PMIC regulator control | [x] |
| OS2.3.2 | Clock controllers | GCC, DISPCC, GPUCC clock setup | [x] |
| OS2.3.3 | Pin controller (TLMM) | Pin muxing and configuration | [x] |
| OS2.3.4 | DMA engine (BAM/GPI) | DMA transfers for high-throughput I/O | [x] |
| OS2.3.5 | IOMMU (SMMU) | I/O memory management unit | [x] |
| OS2.3.6 | Virtio drivers (QEMU) | net, blk, console, gpu for QEMU testing | [x] |
| OS2.3.7 | PCIe host controller | PCIe enumeration and BAR mapping | [x] |
| OS2.3.8 | USB Type-C (UCSI) | Type-C role detection and PD | [x] |
| OS2.3.9 | Driver model | `trait Driver { fn probe(); fn remove(); fn suspend(); }` | [x] |
| OS2.3.10 | Interrupt routing + docs | GIC → driver routing + driver docs | [x] |

### Phase OS3: Filesystem & Services (2 sprints, 20 tasks)

#### Sprint OS3.1: Filesystem (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| OS3.1.1 | VFS layer | Inode operations + dentry cache | [x] |
| OS3.1.2 | ramfs | In-memory filesystem | [x] |
| OS3.1.3 | ext2 read support | Read-only ext2 filesystem driver | [x] |
| OS3.1.4 | FAT32 read/write | FAT32 filesystem with write support | [x] |
| OS3.1.5 | procfs (/proc) | Process information pseudo-filesystem | [x] |
| OS3.1.6 | sysfs + devfs + tmpfs | /sys, /dev (mknod), /tmp filesystems | [x] |
| OS3.1.7 | Pipe filesystem | In-kernel pipe for IPC | [x] |
| OS3.1.8 | mount/umount syscalls | Filesystem mount/unmount operations | [x] |
| OS3.1.9 | Path resolution | Symlink follow + canonical path | [x] |
| OS3.1.10 | Filesystem tests | 10 tests for VFS + FAT32 + procfs | [x] |

#### Sprint OS3.2: Services & Networking (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| OS3.2.1 | File descriptor table | Per-process FD table (256 max) | [x] |
| OS3.2.2 | File locking (flock) | Advisory file locking | [x] |
| OS3.2.3 | Directory operations | opendir, readdir, mkdir, rmdir | [x] |
| OS3.2.4 | Init process (PID 1) | System init with service startup | [x] |
| OS3.2.5 | Service manager | start/stop/restart services | [x] |
| OS3.2.6 | Shell | Built-in: ls, cat, echo, cd, pwd, ps, kill | [x] |
| OS3.2.7 | Cron daemon | Scheduled task execution | [x] |
| OS3.2.8 | Syslog daemon | System logging service | [x] |
| OS3.2.9 | Network stack (TCP/IP) | Minimal TCP/IP + DHCP client + DNS resolver | [x] |
| OS3.2.10 | Services + network tests | 10 tests for init + shell + networking | [x] |

### Phase OS4: Q6A Hardware Integration (2 sprints, 20 tasks)

#### Sprint OS4.1: Hardware Deploy (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| OS4.1.1 | Cross-compile aarch64-none | Bare-metal aarch64 cross-compilation | [x] |
| OS4.1.2 | UEFI boot on Q6A | Boot FajarOS Surya via UEFI on Q6A | [x] |
| OS4.1.3 | GPIO blink test (GPIO96) | Blink LED on GPIO96 from kernel | [x] |
| OS4.1.4 | I2C sensor read | Read temperature/humidity sensor via I2C | [x] |
| OS4.1.5 | SPI flash test | Read/write SPI NOR flash | [x] |
| OS4.1.6 | WiFi scan + connect | Scan WiFi networks and connect | [x] |
| OS4.1.7 | Bluetooth scan | Discover Bluetooth devices | [x] |
| OS4.1.8 | Camera capture | Capture frame via libcamera ISP | [x] |
| OS4.1.9 | QNN NPU inference (CPU) | Run INT8 model on QNN CPU backend | [x] |
| OS4.1.10 | Hardware deploy tests | 10 tests for boot + GPIO + I2C + WiFi | [x] |

#### Sprint OS4.2: Accelerators & Validation (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| OS4.2.1 | QNN NPU inference (GPU) | Run model on QNN GPU backend | [x] |
| OS4.2.2 | Vulkan compute on Adreno | Compute shader on Adreno 643 GPU | [x] |
| OS4.2.3 | Display output (DSI) | Framebuffer output via DSI display | [x] |
| OS4.2.4 | USB device enumeration | List and identify USB devices | [x] |
| OS4.2.5 | NVMe read/write | Block I/O on NVMe storage | [x] |
| OS4.2.6 | Audio playback | Play audio via WSA codec | [x] |
| OS4.2.7 | Thermal monitoring | Read thermal zones + throttle policy | [x] |
| OS4.2.8 | Power management | Suspend/resume with state preservation | [x] |
| OS4.2.9 | Kernel boot time (< 2s) | Optimize boot to shell under 2 seconds | [x] |
| OS4.2.10 | Full system benchmark + report | Complete validation report for Q6A | [x] |

---

## Option 10: Quality Audit & Stabilization (7 sprints, 70 tasks)

**Goal:** Zero known bugs, zero clippy warnings, 90%+ coverage, all examples verified
**Impact:** Foundation for everything else — trust the code before building more

### Phase QA1: Code Quality (3 sprints, 30 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| QA1.1 | `cargo clippy` zero warnings | Fix all lint warnings | [x] |
| QA1.2 | `cargo fmt` clean | All files formatted | [x] |
| QA1.3 | `cargo doc` no warnings | All public items documented | [x] |
| QA1.4 | Remove dead code | `#[allow(dead_code)]` audit | [x] |
| QA1.5 | Remove `.unwrap()` in src/ | Replace with proper error handling | [x] |
| QA1.6 | Audit `unsafe` blocks | Verify all have `// SAFETY:` comment | [x] |
| QA1.7 | Fix Windows CI | `temp_dir()` instead of `/tmp/` | [x] |
| QA1.8 | Fix macOS CI | ARM64 cross-compilation | [x] |
| QA1.9 | Dependency update | `cargo update`, fix breaking changes | [x] |
| QA1.10 | MSRV pin | Set minimum supported Rust version | [x] |
| QA1.11 | Fuzz all targets | 10M iterations per target (lexer/parser/analyzer/interp) | [x] |
| QA1.12 | Fix fuzz findings | Address the 1 known leak + any new crashes | [x] |
| QA1.13 | Property test expansion | 50 new proptest invariants | [x] |
| QA1.14 | Integration test expansion | 50 new eval_tests | [x] |
| QA1.15 | Benchmark baseline | Record all benchmarks for regression tracking | [x] |
| QA1.16 | Code coverage report | tarpaulin → 85%+ target | [x] |
| QA1.17 | Error message audit | All errors have codes, spans, suggestions | [x] |
| QA1.18 | REPL stability | Test 100 common REPL interactions | [x] |
| QA1.19 | Memory leak check | Valgrind/ASan on test suite | [x] |
| QA1.20 | Thread safety audit | Check all `Rc` → should be `Arc`? | [x] |
| QA1.21 | API consistency audit | Naming conventions across all modules | [x] |
| QA1.22 | Error handling patterns | Consistent `Result`/`Option` usage | [x] |
| QA1.23 | Public API review | Remove accidental public items | [x] |
| QA1.24 | Deprecation warnings | Mark old APIs deprecated + suggest new | [x] |
| QA1.25 | Documentation audit | README, CLAUDE.md, all docs up-to-date | [x] |
| QA1.26 | Example verification | Run all 165 examples, fix failures | [x] |
| QA1.27 | Panic audit | Ensure no panics in library code | [x] |
| QA1.28 | Large file audit | Split files > 5000 lines | [x] |
| QA1.29 | Test naming conventions | Consistent `<what>_<when>_<expected>` | [x] |
| QA1.30 | Module documentation | Every mod.rs has module-level docs | [x] |

### Phase QA2: QEMU Verification (2 sprints, 20 tasks)

| # | Task | Details | Status |
|---|------|---------|--------|
| QA2.1 | QEMU x86_64 boot test | Nova kernel boots in QEMU x86_64 | [x] |
| QA2.2 | QEMU aarch64 boot test | Verify aarch64 target | [x] |
| QA2.3 | QEMU riscv64 boot test | Verify riscv64 target | [x] |
| QA2.4 | NVMe initialization | NVMe driver init verified | [x] |
| QA2.5 | FAT32 mount | FAT32 filesystem mount verified | [x] |
| QA2.6 | VFS operations | VFS read/write/mkdir/ls verified | [x] |
| QA2.7 | Network stack test | TCP/UDP stack verified | [x] |
| QA2.8 | SMP boot verification | AP startup via INIT-SIPI-SIPI | [x] |
| QA2.9 | Process creation test | fork/exec/waitpid verified | [x] |
| QA2.10 | Syscall table verification | 34 syscalls dispatched correctly | [x] |
| QA2.11 | Signal delivery test | Signal delivery + handlers | [x] |
| QA2.12 | Pipe test | Circular 4KB buffer, EOF, shell pipes | [x] |
| QA2.13 | Fork/exec test | CoW fork + ELF exec | [x] |
| QA2.14 | Preemptive scheduling | Timer-driven context switch | [x] |
| QA2.15 | Memory allocation test | Page allocator + heap | [x] |
| QA2.16 | Page fault handler | CoW page fault handling | [x] |
| QA2.17 | Shell command verification | 240+ commands functional | [x] |
| QA2.18 | Boot time measurement | Boot to shell < 2s in QEMU | [x] |
| QA2.19 | Memory usage measurement | Kernel < 16MB RAM | [x] |
| QA2.20 | QEMU test documentation | Test procedures documented | [x] |

### Phase QA3: Performance Validation (2 sprints, 20 tasks)

| # | Task | Details | Status |
|---|------|---------|--------|
| QA3.1 | Fibonacci benchmark | interpreter/JIT/AOT comparison | [x] |
| QA3.2 | Array sort benchmark | Sort 10K elements | [x] |
| QA3.3 | String operations benchmark | Concat, split, replace | [x] |
| QA3.4 | HashMap benchmark | Insert/lookup/delete 10K entries | [x] |
| QA3.5 | Tensor operations benchmark | matmul, relu, softmax | [x] |
| QA3.6 | ML training benchmark | MNIST 90%+ accuracy | [x] |
| QA3.7 | Compilation speed benchmark | 1000-line program compile time | [x] |
| QA3.8 | Binary size measurement | Release binary < 10MB | [x] |
| QA3.9 | Startup time measurement | REPL start < 100ms | [x] |
| QA3.10 | Memory usage profiling | Idle interpreter < 50MB | [x] |
| QA3.11 | Fajar vs Rust comparison | fib(30) relative perf | [x] |
| QA3.12 | Fajar vs Python comparison | fib(30) relative perf | [x] |
| QA3.13 | Flame graph analysis | Top 5 hot functions identified | [x] |
| QA3.14 | Hot-path optimization | Top 5 functions optimized | [x] |
| QA3.15 | Inlining heuristic tuning | Small fns auto-inlined | [x] |
| QA3.16 | Register allocation check | Cranelift regalloc quality | [x] |
| QA3.17 | JIT warm-up time | First-call overhead < 10ms | [x] |
| QA3.18 | AOT compilation time | Full program < 5s | [x] |
| QA3.19 | Cranelift vs LLVM codegen | Quality comparison documented | [x] |
| QA3.20 | Performance documentation | Benchmark results in docs/ | [x] |

---

## Execution Strategy

### Path A — "Stabilize First" (recommended)
```
10 (Quality)  →  8 (CI/CD)  →  5 (Build System)  →  2 (WASI)  →  1 (Distributed)
```
Foundation first, then expand.

### Path B — "Feature Push"
```
4 (Type System)  →  3 (GPU)  →  1 (Distributed)  →  6 (Cloud)  →  7 (Plugins)
```
Maximum capability, fix quality later.

### Path C — "Ship It"
```
10 (Quality)  →  8 (CI/CD)  →  6 (Cloud)  →  2 (WASI)  →  7 (Plugins)
```
Production-ready deployment pipeline.

### Path D — "OS Ambition"
```
9 (FajarOS Surya)  →  10 (Quality)  →  3 (GPU)  →  1 (Distributed)
```
Complete the OS vision first.

---

## Decision Matrix

| Criterion | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 |
|-----------|---|---|---|---|---|---|---|---|---|---|
| **User impact** | ★★★ | ★★★★ | ★★★ | ★★ | ★★★★★ | ★★★★ | ★★★ | ★★★★ | ★★ | ★★★★★ |
| **Differentiation** | ★★★★ | ★★★ | ★★★★★ | ★★★★★ | ★★ | ★★★ | ★★★ | ★★ | ★★★★★ | ★★ |
| **Effort** | High | Medium | High | Medium | Medium | High | Medium | Medium | Very High | High |
| **Dependency** | None | Wasm | CUDA | Analyzer | None | WASI | Compiler | CI | Q6A (partial) | None |
| **Risk** | Medium | Low | High | Medium | Low | Medium | Low | Low | High | Low |

---

## Summary

```
Option 1:   Distributed Computing      8 sprints   80 tasks    ~16 hrs
Option 2:   WASI Preview 2             6 sprints   60 tasks    ~12 hrs
Option 3:   GPU Training Pipeline      8 sprints   80 tasks    ~16 hrs
Option 4:   Advanced Type System       6 sprints   60 tasks    ~12 hrs
Option 5:   Incremental Compilation    6 sprints   60 tasks    ~12 hrs
Option 6:   Cloud & Edge Deployment    7 sprints   70 tasks    ~14 hrs
Option 7:   Plugin & Extension System  5 sprints   50 tasks    ~10 hrs
Option 8:   CI/CD & Release            5 sprints   50 tasks    ~10 hrs
Option 9:   FajarOS Surya (ARM64)     10 sprints  100 tasks    ~20 hrs
Option 10:  Quality Audit              7 sprints   70 tasks    ~14 hrs

Total:     68 sprints, 680 tasks, ~136 hours
```
