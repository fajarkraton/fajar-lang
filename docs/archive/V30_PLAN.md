# Fajar Lang v3.0 "Singularity" — Implementation Plan

> **Focus:** Production-grade ecosystem — HKT, structured concurrency, distributed compute, advanced ML, GPU codegen, workspaces, time-travel debugging, deployment
> **Timeline:** 32 sprints, ~320 tasks across 8 phases
> **Prerequisite:** v2.0 "Transcendence" COMPLETE
> **Target release:** 2027 Q2

---

## Motivation

v2.0 gave Fajar Lang dependent types, linear types, formal verification, and full self-hosting. v3.0 makes the language **production-deployable at scale**:

1. **Higher-Kinded Types** enable generic abstractions over containers (`Functor`, `Monad`) — write once, works for `Option`, `Result`, `Vec`, `Future`
2. **Structured Concurrency** guarantees no leaked tasks, no orphan goroutines — every async scope has a well-defined lifetime
3. **Distributed Computing** enables multi-node ML training and microservices — built into the language, not bolted on
4. **Native GPU Codegen** generates PTX/SPIR-V directly — no CUDA SDK dependency, kernel fusion for 10x throughput
5. **Time-Travel Debugging** records execution and replays backwards — find the root cause of any bug instantly

No existing language combines all five in a single toolchain.

---

## Phase Overview

| Phase | Name | Sprints | Tasks | Focus |
|-------|------|---------|-------|-------|
| 1 | Higher-Kinded Types | S1-S4 | 40 | HKT, Functor/Monad, do-notation, type-level programming |
| 2 | Structured Concurrency | S5-S8 | 40 | Async scopes, cancellation, actors, STM, supervision |
| 3 | Distributed Computing | S9-S12 | 40 | RPC framework, distributed tensors, cluster scheduling |
| 4 | Advanced ML v2 | S13-S16 | 40 | Transformer inference, diffusion, RL, model serving |
| 5 | Native GPU Codegen | S17-S20 | 40 | PTX/SPIR-V generation, kernel fusion, shared memory |
| 6 | Package Ecosystem v2 | S21-S24 | 40 | Workspaces, build scripts, cross-compilation matrix |
| 7 | Debugger v2 | S25-S28 | 40 | Time-travel debugging, record/replay, memory visualization |
| 8 | Production Deployment | S29-S32 | 40 | Containers, health checks, metrics/tracing, hot reload |

---

## Phase 1: Higher-Kinded Types (S1-S4)

> Abstractions over type constructors — `Functor`, `Monad`, `Applicative` — enabling generic programming over `Option`, `Result`, `Vec`, `Future`.

### Sprint S1 — HKT Foundation

- [x] S1.1 — Type Constructor Kinds: Extend type system with `* -> *` kinds (e.g., `Option` has kind `Type -> Type`)
- [x] S1.2 — Kind Checker: Implement kind inference and checking — reject `Functor<i32>` (wrong kind)
- [x] S1.3 — Type Constructor Parameters: Allow traits to accept type constructors as parameters (`trait Functor<F: * -> *>`)
- [x] S1.4 — Kind Annotations: Optional kind annotations on type parameters (`fn foo<F: * -> *>(x: F<i32>)`)
- [x] S1.5 — Partial Application: Support partial type application (`Result<_, Error>` as a `* -> *` type)
- [x] S1.6 — Kind Unification: Extend type unification to work at the kind level
- [x] S1.7 — Kind Error Messages: Produce clear diagnostics for kind mismatches ("expected `* -> *`, found `*`")
- [x] S1.8 — Higher-Rank Kinds: Support `(* -> *) -> *` for abstractions over type constructors themselves
- [x] S1.9 — Kind Defaults: Default kind is `*` when unspecified, infer higher kinds from usage
- [x] S1.10 — Unit Tests: 15+ tests for kind checking, inference, unification, error messages, partial application

### Sprint S2 — Functor / Applicative / Monad

- [x] S2.1 — Functor Trait: Define `trait Functor<F: * -> *> { fn fmap<A, B>(fa: F<A>, f: fn(A) -> B) -> F<B> }`
- [x] S2.2 — Applicative Trait: Define `trait Applicative<F: * -> *>: Functor<F> { fn pure<A>(a: A) -> F<A>; fn ap<A, B>(ff: F<fn(A) -> B>, fa: F<A>) -> F<B> }`
- [x] S2.3 — Monad Trait: Define `trait Monad<F: * -> *>: Applicative<F> { fn bind<A, B>(fa: F<A>, f: fn(A) -> F<B>) -> F<B> }`
- [x] S2.4 — Option Instances: Implement `Functor`, `Applicative`, `Monad` for `Option<T>`
- [x] S2.5 — Result Instances: Implement `Functor`, `Applicative`, `Monad` for `Result<T, E>`
- [x] S2.6 — Vec Instances: Implement `Functor` for `Vec<T>` (fmap = map)
- [x] S2.7 — Monad Laws Verification: Test associativity, left identity, right identity for all instances
- [x] S2.8 — Foldable Trait: Define `trait Foldable<F: * -> *> { fn fold<A, B>(fa: F<A>, init: B, f: fn(B, A) -> B) -> B }`
- [x] S2.9 — Traversable Trait: Define `trait Traversable<F: * -> *>: Foldable<F> + Functor<F> { fn traverse ... }`
- [x] S2.10 — Unit Tests: 15+ tests for fmap, pure, bind, monad laws, foldable, traversable

### Sprint S3 — Do-Notation

- [x] S3.1 — Do-Block Syntax: Parse `do { x <- expr; y <- expr; return z }` as syntactic sugar for monadic bind chains
- [x] S3.2 — Desugaring: Transform `do { x <- e1; e2 }` into `bind(e1, fn(x) { e2 })`
- [x] S3.3 — Let-in-Do: Support `do { let x = expr; ... }` for non-monadic bindings within do-blocks
- [x] S3.4 — Pattern Matching in Do: Support `do { (a, b) <- expr; ... }` with pattern destructuring
- [x] S3.5 — Type Inference in Do: Infer the monad type from the first `<-` expression
- [x] S3.6 — Do for Option: `do { x <- get_user(id); y <- x.email; return y }` — None short-circuits
- [x] S3.7 — Do for Result: `do { x <- parse(s)?; y <- validate(x)?; return y }` — Err short-circuits
- [x] S3.8 — Do for Async: `do { x <- fetch(url).await; y <- parse(x).await; return y }`
- [x] S3.9 — Nested Do Blocks: Support `do { x <- do { ... }; ... }` with correct scoping
- [x] S3.10 — Unit Tests: 15+ tests for desugaring, type inference, Option/Result/async do-blocks, nested blocks

### Sprint S4 — Type-Level Programming

- [x] S4.1 — Type-Level Functions: Allow type aliases with parameters that compute types (`type IfElse<C, T, F> = ...`)
- [x] S4.2 — Associated Type Families: Type families in traits (`trait Collection { type Elem; type Iter: Iterator<Item = Self::Elem> }`)
- [x] S4.3 — Type-Level Booleans: `type True` and `type False` for compile-time conditionals
- [x] S4.4 — Type-Level Lists: `type HNil` and `type HCons<H, T>` for heterogeneous lists
- [x] S4.5 — Compile-Time Type Selection: `type Select<C: Bool, T, F>` evaluates to `T` if `C = True`, else `F`
- [x] S4.6 — Phantom Types: Zero-cost type tags for state machines (`struct Locked; struct Unlocked; struct Door<State>`)
- [x] S4.7 — Type Witnesses: `fn prove<A, B>(eq: TypeEq<A, B>, a: A) -> B` for safe type coercion
- [x] S4.8 — GAT + HKT Interaction: Ensure GATs from v0.8 work correctly with HKT type constructors
- [x] S4.9 — Type-Level Computation Limits: Configurable recursion limit for type-level computation (default: 256)
- [x] S4.10 — Unit Tests: 15+ tests for type families, HList, phantom types, type witnesses, recursion limits

---

## Phase 2: Structured Concurrency (S5-S8)

> Every concurrent task has a well-defined scope and lifetime — no leaked tasks, no orphan goroutines, guaranteed cleanup.

### Sprint S5 — Async Scopes

- [x] S5.1 — Scope Primitive: `async_scope { |s| s.spawn(task1); s.spawn(task2); }` — scope waits for all spawned tasks
- [x] S5.2 — Scope Lifetime: Scope owns all spawned tasks — when scope ends, all tasks are joined or cancelled
- [x] S5.3 — Scope Error Propagation: If any task in a scope panics, cancel siblings and propagate error to parent
- [x] S5.4 — Nested Scopes: Support `async_scope { |s| s.spawn(async_scope { |s2| ... }); }` with correct nesting
- [x] S5.5 — Scope Return Values: Scope collects return values from all spawned tasks into a Vec
- [x] S5.6 — Concurrency Limit: `async_scope_with(max_concurrent: 4) { ... }` — limit parallel task count
- [x] S5.7 — Scope Timeout: `async_scope_with(timeout: Duration) { ... }` — cancel all tasks on timeout
- [x] S5.8 — Task Priority: `s.spawn_with_priority(High, task)` — priority-based scheduling within scope
- [x] S5.9 — Scope Metrics: Track task count, completion time, cancellation count per scope
- [x] S5.10 — Unit Tests: 15+ tests for scope lifecycle, error propagation, nesting, limits, timeout, metrics

### Sprint S6 — Cancellation & Graceful Shutdown

- [x] S6.1 — Cancellation Token: `CancellationToken` shared across tasks, cooperative cancellation via `token.is_cancelled()`
- [x] S6.2 — Cancel Propagation: Parent cancellation automatically cancels all child tasks/scopes
- [x] S6.3 — Cancel-Safe Operations: Mark I/O operations as cancel-safe or cancel-unsafe, lint for unsafe cancellation
- [x] S6.4 — Cleanup Handlers: `defer { cleanup_code }` runs on both normal completion and cancellation
- [x] S6.5 — Graceful Shutdown: `shutdown_signal()` returns a future that resolves on SIGTERM/SIGINT
- [x] S6.6 — Drain Mode: On shutdown, stop accepting new work, finish in-progress tasks, then exit
- [x] S6.7 — Shutdown Timeout: Force-kill tasks that don't finish within shutdown timeout (default: 30s)
- [x] S6.8 — Cancellation Reasons: `token.cancel_with_reason("timeout")` — attach context to cancellation
- [x] S6.9 — Select with Cancel: `select { task1.await, task2.await, token.cancelled() }` — first-to-complete with cancel
- [x] S6.10 — Unit Tests: 15+ tests for token lifecycle, propagation, cleanup, shutdown, drain, select

### Sprint S7 — Actor Model

- [x] S7.1 — Actor Trait: `trait Actor { type Message; fn handle(&mut self, msg: Self::Message) -> Action; }`
- [x] S7.2 — Actor Spawn: `let addr = spawn_actor(MyActor::new())` — returns typed address for sending messages
- [x] S7.3 — Message Send: `addr.send(msg).await` — async message delivery with backpressure
- [x] S7.4 — Actor Mailbox: Bounded MPSC channel per actor, configurable capacity (default: 1024)
- [x] S7.5 — Actor Lifecycle: `fn started(&mut self)`, `fn stopped(&mut self)` lifecycle hooks
- [x] S7.6 — Supervision Strategy: `OneForOne` (restart failed actor), `AllForOne` (restart all siblings), `RestForOne`
- [x] S7.7 — Actor Registry: Named actors accessible via `registry.get::<MyActor>("name")`
- [x] S7.8 — Request-Response: `let resp = addr.ask(request).await` — typed request/response pattern
- [x] S7.9 — Actor State Persistence: `trait PersistentActor { fn snapshot(&self) -> Bytes; fn restore(bytes: Bytes) -> Self; }`
- [x] S7.10 — Unit Tests: 15+ tests for spawn, send, ask, supervision, lifecycle, registry, persistence

### Sprint S8 — Software Transactional Memory

- [x] S8.1 — TVar Primitive: `TVar<T>` — transactional variable, read/write only within `atomically { ... }`
- [x] S8.2 — STM Transaction: `atomically { let x = tvar.read(); tvar.write(x + 1); }` — atomic, isolated, consistent
- [x] S8.3 — Retry Semantics: `retry` blocks until any read TVar changes, then re-executes transaction
- [x] S8.4 — OrElse Combinator: `atomically { tx1.or_else(tx2) }` — try tx1, if it retries, try tx2
- [x] S8.5 — Conflict Detection: Optimistic concurrency — detect read/write conflicts, retry on conflict
- [x] S8.6 — Nested Transactions: Support nested `atomically` blocks with correct rollback semantics
- [x] S8.7 — STM + Async: Allow STM transactions within async contexts, integrate with async runtime
- [x] S8.8 — TVar Collections: `TMap<K, V>` and `TQueue<T>` — transactional HashMap and queue
- [x] S8.9 — STM Metrics: Track commit/retry/conflict counts per transaction for performance tuning
- [x] S8.10 — Unit Tests: 15+ tests for atomicity, isolation, retry, orElse, conflicts, nested, async integration

---

## Phase 3: Distributed Computing (S9-S12)

> Built-in RPC, distributed tensors, and cluster scheduling — multi-node ML training and microservices without external frameworks.

### Sprint S9 — RPC Framework

- [x] S9.1 — Service Definition: `@rpc trait UserService { fn get_user(id: u64) -> Result<User, Error>; }`
- [x] S9.2 — Code Generation: Generate client stub and server skeleton from `@rpc` trait definition
- [x] S9.3 — Serialization: Binary serialization format for RPC messages (compact, zero-copy where possible)
- [x] S9.4 — Transport Layer: TCP transport with connection pooling and multiplexing
- [x] S9.5 — Service Discovery: `Registry` for service name → address resolution
- [x] S9.6 — Load Balancing: Round-robin, least-connections, weighted strategies for multi-server targets
- [x] S9.7 — Retry Policy: Configurable retry with exponential backoff and circuit breaker
- [x] S9.8 — Streaming RPC: `fn stream_data(query: Query) -> Stream<DataChunk>` — server-side streaming
- [x] S9.9 — Bidirectional Streaming: Client and server both stream messages concurrently
- [x] S9.10 — Unit Tests: 15+ tests for service generation, serialization roundtrip, discovery, load balancing, retry

### Sprint S10 — Distributed Tensors

- [x] S10.1 — Tensor Sharding: `tensor.shard(dim=0, num_shards=4)` — split tensor across nodes
- [x] S10.2 — Shard Placement: Placement strategy — round-robin, locality-aware, memory-balanced
- [x] S10.3 — Distributed MatMul: Parallel matrix multiplication across shards with result aggregation
- [x] S10.4 — AllReduce: Sum/average gradients across all nodes for synchronized training
- [x] S10.5 — Ring AllReduce: Bandwidth-optimal ring-based all-reduce for large tensors
- [x] S10.6 — Parameter Server: Centralized parameter storage with async gradient updates
- [x] S10.7 — Data Parallelism: Automatic batch splitting across GPU nodes with gradient sync
- [x] S10.8 — Model Parallelism: Split large model layers across nodes (pipeline parallelism)
- [x] S10.9 — Communication Backend: NCCL-style collectives over TCP/shared memory
- [x] S10.10 — Unit Tests: 15+ tests for sharding, placement, allreduce, ring topology, data/model parallelism

### Sprint S11 — Cluster Scheduling

- [x] S11.1 — Node Discovery: Automatic peer discovery via multicast or seed nodes
- [x] S11.2 — Resource Advertisement: Each node advertises CPU cores, GPU count, memory, network bandwidth
- [x] S11.3 — Task Placement: Schedule tasks to nodes based on resource requirements and availability
- [x] S11.4 — Fault Detection: Heartbeat-based failure detection with configurable timeout (default: 10s)
- [x] S11.5 — Task Migration: Move running task to another node on failure (checkpoint + restore)
- [x] S11.6 — Work Stealing: Idle nodes steal tasks from overloaded nodes for load balancing
- [x] S11.7 — Barrier Synchronization: Global barrier across all cluster nodes for synchronized phases
- [x] S11.8 — Cluster Topology: Aware of rack/datacenter topology for locality-optimized scheduling
- [x] S11.9 — Resource Quotas: Per-user/per-project resource limits and fair scheduling
- [x] S11.10 — Unit Tests: 15+ tests for discovery, placement, fault detection, migration, work stealing, barriers

### Sprint S12 — Fault Tolerance

- [x] S12.1 — Checkpointing: Periodic state snapshots for long-running distributed computations
- [x] S12.2 — Checkpoint Storage: Local disk, NFS, or object storage backends for checkpoints
- [x] S12.3 — Recovery: Restart failed computation from latest checkpoint, skip completed work
- [x] S12.4 — Exactly-Once Semantics: Idempotent operations + deduplication for exactly-once message delivery
- [x] S12.5 — Saga Pattern: Distributed transaction with compensating actions for each step
- [x] S12.6 — Dead Letter Queue: Failed messages routed to DLQ for manual inspection/retry
- [x] S12.7 — Circuit Breaker: Per-service circuit breaker (closed → open → half-open) with configurable thresholds
- [x] S12.8 — Bulkhead Isolation: Isolate failure domains — one failing service doesn't take down the system
- [x] S12.9 — Chaos Engineering: `@chaos` annotation to inject random failures for resilience testing
- [x] S12.10 — Unit Tests: 15+ tests for checkpointing, recovery, exactly-once, saga, circuit breaker, bulkhead

---

## Phase 4: Advanced ML v2 (S13-S16)

> Production ML inference and training — Transformer, diffusion models, reinforcement learning, model serving.

### Sprint S13 — Transformer Inference

- [x] S13.1 — Multi-Head Self-Attention: Efficient scaled dot-product attention with Q/K/V projections
- [x] S13.2 — Causal Masking: Lower-triangular mask for autoregressive (decoder-only) transformers
- [x] S13.3 — Rotary Position Embeddings: RoPE for relative position encoding (used by LLaMA, Mistral)
- [x] S13.4 — KV Cache: Key-value cache for incremental decoding — avoid recomputing past tokens
- [x] S13.5 — Flash Attention: Memory-efficient attention via tiling — O(N) memory instead of O(N^2)
- [x] S13.6 — Grouped Query Attention: GQA for reduced memory bandwidth (used by LLaMA 2)
- [x] S13.7 — Layer Normalization: RMSNorm (pre-norm) and LayerNorm with fused operations
- [x] S13.8 — SwiGLU Activation: Gated linear unit activation (used by LLaMA, PaLM)
- [x] S13.9 — Token Sampling: Temperature, top-k, top-p (nucleus) sampling for text generation
- [x] S13.10 — Unit Tests: 15+ tests for attention scores, masking, RoPE, KV cache, sampling strategies

### Sprint S14 — Diffusion Models

- [x] S14.1 — Noise Schedule: Linear and cosine noise schedules for forward diffusion process
- [x] S14.2 — UNet Architecture: Time-conditioned UNet with downsampling/upsampling blocks and skip connections
- [x] S14.3 — Sinusoidal Timestep Embedding: Encode diffusion timestep as positional embedding
- [x] S14.4 — Forward Process: Add Gaussian noise to data according to noise schedule
- [x] S14.5 — Reverse Process: Predict and remove noise iteratively (DDPM sampling)
- [x] S14.6 — DDIM Sampling: Deterministic sampling with fewer steps (10-50 instead of 1000)
- [x] S14.7 — Classifier-Free Guidance: Conditional generation with guidance scale parameter
- [x] S14.8 — Latent Diffusion: Operate in latent space via encoder/decoder for efficiency
- [x] S14.9 — Image Generation Pipeline: Full text-to-image pipeline (text encoder → UNet → decoder)
- [x] S14.10 — Unit Tests: 15+ tests for noise schedules, UNet forward pass, sampling, guidance, latent space

### Sprint S15 — Reinforcement Learning

- [x] S15.1 — Environment Trait: `trait Env { type State; type Action; fn step(action) -> (State, f64, bool); fn reset() -> State; }`
- [x] S15.2 — Replay Buffer: Experience replay with uniform and prioritized sampling
- [x] S15.3 — DQN Agent: Deep Q-Network with target network and epsilon-greedy exploration
- [x] S15.4 — Policy Gradient: REINFORCE algorithm with baseline subtraction
- [x] S15.5 — PPO Agent: Proximal Policy Optimization with clipped surrogate objective
- [x] S15.6 — GAE: Generalized Advantage Estimation for variance reduction
- [x] S15.7 — Multi-Agent: Support multiple agents with shared/independent policies
- [x] S15.8 — Vectorized Environments: Run N environment instances in parallel for faster training
- [x] S15.9 — Reward Shaping: Curriculum learning with staged reward functions
- [x] S15.10 — Unit Tests: 15+ tests for environment interface, replay buffer, DQN update, PPO loss, GAE computation

### Sprint S16 — Model Serving

- [x] S16.1 — Inference Server: HTTP/gRPC endpoint for model inference with batching
- [x] S16.2 — Dynamic Batching: Accumulate requests into batches for GPU efficiency (max wait: 10ms)
- [x] S16.3 — Model Registry: Load/unload models by name and version, A/B testing support
- [x] S16.4 — Input Validation: Schema-based request validation with type-checked tensor shapes
- [x] S16.5 — Output Postprocessing: Top-k, softmax, argmax, label mapping in serving pipeline
- [x] S16.6 — Model Warmup: Pre-run inference on dummy data to warm JIT/caches before serving
- [x] S16.7 — Health & Readiness: `/health` and `/ready` endpoints for load balancer integration
- [x] S16.8 — Inference Metrics: Latency histogram, throughput counter, batch size distribution
- [x] S16.9 — Model Versioning: Blue-green deployment with traffic splitting between model versions
- [x] S16.10 — Unit Tests: 15+ tests for batching, registry, validation, warmup, health checks, metrics

---

## Phase 5: Native GPU Codegen (S17-S20)

> Generate PTX/SPIR-V directly from Fajar Lang — no CUDA SDK dependency, kernel fusion, shared memory primitives.

### Sprint S17 — PTX Backend

- [x] S17.1 — PTX IR Types: Map Fajar Lang types to PTX register types (.u32, .u64, .f32, .f64, .pred)
- [x] S17.2 — Kernel Entry: Generate `.entry` kernel functions with `.param` grid/block parameters
- [x] S17.3 — Thread Indexing: Emit `%tid.x`, `%ctaid.x`, `%ntid.x` for thread/block/grid indexing
- [x] S17.4 — Arithmetic Ops: Generate PTX arithmetic (add, mul, fma, div, rem) for all numeric types
- [x] S17.5 — Memory Ops: Load/store to global (`.global`), shared (`.shared`), local (`.local`) memory
- [x] S17.6 — Control Flow: Generate PTX branch (`@pred bra`), loop, and predicated instructions
- [x] S17.7 — Special Functions: `__syncthreads()`, `atomicAdd`, `__shfl_sync` warp primitives
- [x] S17.8 — PTX Assembly Output: Emit valid PTX 7.0+ assembly text for CUDA driver API loading
- [x] S17.9 — Grid Launch Config: Compute optimal grid/block dimensions from tensor shape
- [x] S17.10 — Unit Tests: 15+ tests for type mapping, thread indexing, arithmetic, memory, control flow, sync

### Sprint S18 — SPIR-V Backend

- [x] S18.1 — SPIR-V Module: Generate valid SPIR-V binary module with compute shader capability
- [x] S18.2 — SPIR-V Types: Map Fajar Lang types to SPIR-V OpTypeInt/OpTypeFloat/OpTypeVector
- [x] S18.3 — Compute Shader Entry: OpEntryPoint with GlobalInvocationId built-in for thread indexing
- [x] S18.4 — Storage Buffers: SSBO (Shader Storage Buffer Object) for input/output tensor data
- [x] S18.5 — Workgroup Memory: Shared memory via Workgroup storage class
- [x] S18.6 — Barrier: OpControlBarrier for workgroup synchronization
- [x] S18.7 — SPIR-V Validation: Validate generated SPIR-V against specification rules
- [x] S18.8 — Vulkan Dispatch: Launch SPIR-V compute shaders via Vulkan compute queue
- [x] S18.9 — Backend Selection: `--gpu-backend=ptx|spirv|auto` flag, auto-detect NVIDIA vs other GPUs
- [x] S18.10 — Unit Tests: 15+ tests for module generation, type mapping, compute entry, buffers, barriers

### Sprint S19 — Kernel Fusion

- [x] S19.1 — Fusion Analysis: Detect fuseable kernel sequences (e.g., matmul → relu → add bias)
- [x] S19.2 — Element-wise Fusion: Fuse chains of element-wise operations into a single kernel
- [x] S19.3 — Reduction Fusion: Fuse reduction operations (sum, max) with preceding element-wise ops
- [x] S19.4 — Memory Planning: Eliminate intermediate allocations between fused kernels
- [x] S19.5 — Tiling Strategy: Tile large operations for shared memory utilization
- [x] S19.6 — Auto-Tuning: Try multiple tile sizes and configurations, pick fastest for target GPU
- [x] S19.7 — Fusion Graph: Build dataflow graph of GPU operations, identify fusion opportunities
- [x] S19.8 — Fusion Limits: Configurable max kernel size and register pressure limits
- [x] S19.9 — Fusion Report: `--gpu-report` shows fusion decisions, speedup estimates
- [x] S19.10 — Unit Tests: 15+ tests for fusion detection, element-wise chains, tiling, memory planning

### Sprint S20 — GPU Memory Management

- [x] S20.1 — Device Memory Allocator: Pool allocator for GPU memory with configurable initial/max size
- [x] S20.2 — Host-Device Transfer: Async `memcpy_h2d` / `memcpy_d2h` with pinned host memory
- [x] S20.3 — Unified Memory: Optional unified memory mode for development convenience
- [x] S20.4 — Memory Pool: Sub-allocator with free-list for avoiding per-operation cudaMalloc overhead
- [x] S20.5 — Memory Defragmentation: Periodic defragmentation of GPU memory pool
- [x] S20.6 — Out-of-Memory Handling: Graceful OOM with spill-to-host fallback
- [x] S20.7 — Multi-GPU Memory: Peer-to-peer memory access between GPUs on same node
- [x] S20.8 — Memory Profiling: Track allocation/deallocation, peak usage, transfer bandwidth
- [x] S20.9 — Zero-Copy Tensors: Tensors backed by pinned host memory accessible from GPU
- [x] S20.10 — Unit Tests: 15+ tests for allocation, transfer, pool, OOM handling, multi-GPU, profiling

---

## Phase 6: Package Ecosystem v2 (S21-S24)

> Workspaces, build scripts, conditional compilation, and a cross-compilation matrix for production package management.

### Sprint S21 — Workspaces

- [x] S21.1 — Workspace Definition: `[workspace]` in root `fj.toml` listing member packages
- [x] S21.2 — Shared Dependencies: Workspace-level dependency resolution — all members share versions
- [x] S21.3 — Workspace Build: `fj build --workspace` builds all members in dependency order
- [x] S21.4 — Workspace Test: `fj test --workspace` runs tests for all members
- [x] S21.5 — Inter-Package Dependencies: Members can depend on sibling packages via `path = "../sibling"`
- [x] S21.6 — Workspace Inheritance: Members inherit `[package.edition]`, `[package.license]` from workspace
- [x] S21.7 — Selective Build: `fj build -p package-name` builds only specified member and its deps
- [x] S21.8 — Workspace Metadata: `[workspace.metadata]` for custom keys used by tooling
- [x] S21.9 — Virtual Workspace: Workspace without a root package — just coordinates members
- [x] S21.10 — Unit Tests: 15+ tests for workspace parsing, shared deps, build order, inheritance, selective build

### Sprint S22 — Build Scripts

- [x] S22.1 — Build Script Support: `build = "build.fj"` in fj.toml — runs before compilation
- [x] S22.2 — Environment Variables: Build script can set `FJ_CFG_*` variables for conditional compilation
- [x] S22.3 — Native Library Detection: `fj_build::find_library("openssl")` — pkg-config integration
- [x] S22.4 — Code Generation: Build script can generate .fj source files in `OUT_DIR`
- [x] S22.5 — Rerun Triggers: `println!("fj:rerun-if-changed=path")` for incremental build script execution
- [x] S22.6 — Link Flags: `println!("fj:rustc-link-lib=ssl")` to pass linker flags
- [x] S22.7 — Feature Detection: Probe for system capabilities (SIMD, GPU, OS features) at build time
- [x] S22.8 — Proto/Schema Compilation: Build script compiles .proto/.fbs schema files to .fj source
- [x] S22.9 — Build Script Dependencies: Separate `[build-dependencies]` section in fj.toml
- [x] S22.10 — Unit Tests: 15+ tests for script execution, env vars, code gen, rerun triggers, link flags

### Sprint S23 — Conditional Compilation

- [x] S23.1 — Cfg Attributes: `@cfg(target_os = "linux")` on functions, structs, modules
- [x] S23.2 — Feature Flags: `@cfg(feature = "gpu")` — user-selectable features in fj.toml
- [x] S23.3 — Target Architecture: `@cfg(target_arch = "aarch64")` for architecture-specific code
- [x] S23.4 — Cfg Combinators: `@cfg(all(linux, feature="gpu"))`, `@cfg(any(...))`, `@cfg(not(...))`
- [x] S23.5 — Platform Modules: Entire module conditional on platform (`@cfg(windows) mod win_impl;`)
- [x] S23.6 — Cfg in Tests: `@cfg(test)` for test-only code, `@cfg(bench)` for benchmark-only code
- [x] S23.7 — Cfg Checking: Warn on unknown cfg keys to catch typos (`@cfg(taget_os)` → did you mean `target_os`?)
- [x] S23.8 — Default Features: `[features] default = ["std"]` — default-on features
- [x] S23.9 — Feature Dependencies: `[features] gpu = ["cuda", "driver"]` — features that enable other features
- [x] S23.10 — Unit Tests: 15+ tests for cfg parsing, feature flags, combinators, platform modules, checking

### Sprint S24 — Cross-Compilation Matrix

- [x] S24.1 — Target Triple: Parse `<arch>-<vendor>-<os>-<abi>` target triples (e.g., `aarch64-unknown-linux-gnu`)
- [x] S24.2 — Sysroot Management: Download and cache target sysroots for cross-compilation
- [x] S24.3 — Cross Linker: Detect and configure cross-linker (e.g., `aarch64-linux-gnu-gcc`)
- [x] S24.4 — Build Matrix: `fj build --target=aarch64,riscv64,wasm32` — build for multiple targets
- [x] S24.5 — Target Feature Detection: Detect available CPU features for target (e.g., NEON for aarch64)
- [x] S24.6 — QEMU Testing: `fj test --target=aarch64 --runner=qemu` — run cross-compiled tests under QEMU
- [x] S24.7 — Docker Cross-Build: Generate Dockerfiles for cross-compilation environments
- [x] S24.8 — Release Matrix: Build release binaries for all supported targets in CI
- [x] S24.9 — Platform Support Tiers: Tier 1 (CI tested), Tier 2 (cross-compiles), Tier 3 (best-effort)
- [x] S24.10 — Unit Tests: 15+ tests for triple parsing, sysroot paths, linker detection, matrix generation, tiers

---

## Phase 7: Debugger v2 (S25-S28)

> Time-travel debugging, record/replay, memory visualization — find the root cause of any bug by replaying execution backwards.

### Sprint S25 — Execution Recording

- [x] S25.1 — Record Mode: `fj run --record program.fj` — record all state changes during execution
- [x] S25.2 — Event Log: Record function entries/exits, variable assignments, heap allocations, I/O operations
- [x] S25.3 — Compact Encoding: Delta-compress state changes — only record what changed, not full snapshots
- [x] S25.4 — Recording Format: Binary format with index for random access to any execution point
- [x] S25.5 — Recording Overhead: < 10x slowdown for record mode (target: 2-5x)
- [x] S25.6 — Selective Recording: `@record` annotation on specific functions to reduce overhead
- [x] S25.7 — Recording Size Limits: Configurable max recording size with ring-buffer mode for long runs
- [x] S25.8 — I/O Capture: Record all file reads, network responses for deterministic replay
- [x] S25.9 — Thread Recording: Record thread scheduling decisions for deterministic multi-threaded replay
- [x] S25.10 — Unit Tests: 15+ tests for event recording, delta compression, format, overhead, selective, I/O capture

### Sprint S26 — Time-Travel Replay

- [x] S26.1 — Replay Mode: `fj replay recording.fjrec` — replay execution with full debugger support
- [x] S26.2 — Reverse Continue: `reverse-continue` — run backwards to previous breakpoint
- [x] S26.3 — Reverse Step: `reverse-step` — step backwards one statement
- [x] S26.4 — Reverse Search: Find last assignment to a variable, last call to a function
- [x] S26.5 — Watchpoint Reverse: When a watchpoint triggers, travel back to where the value was set
- [x] S26.6 — Root Cause Analysis: Trace back from a crash/assertion to the first wrong value
- [x] S26.7 — Timeline View: Visual timeline of execution with zoomable regions and bookmarks
- [x] S26.8 — Diff at Points: Compare program state at two points in execution
- [x] S26.9 — Replay Fidelity: Guarantee bit-exact reproduction of original execution
- [x] S26.10 — Unit Tests: 15+ tests for replay correctness, reverse stepping, search, diff, fidelity

### Sprint S27 — Memory Visualization

- [x] S27.1 — Heap Map: Visual representation of heap — blocks, sizes, fragmentation
- [x] S27.2 — Allocation Timeline: Graph of heap usage over time with allocation/deallocation events
- [x] S27.3 — Reference Graph: Visual graph of object references — detect cycles, dangling refs
- [x] S27.4 — Stack Visualization: Visual stack frames with variable values and sizes
- [x] S27.5 — Memory Leak Detection: Track allocations without corresponding frees, report likely leaks
- [x] S27.6 — Memory Diff: Compare heap state between two execution points
- [x] S27.7 — Ownership Visualization: Show ownership tree — who owns what, borrow relationships
- [x] S27.8 — Tensor Memory Map: Visualize tensor layout in memory — strides, padding, alignment
- [x] S27.9 — Cache Analysis: Simulate cache behavior, highlight cache-unfriendly access patterns
- [x] S27.10 — Unit Tests: 15+ tests for heap tracking, reference graphs, leak detection, ownership viz, tensor layout

### Sprint S28 — Performance Profiling

- [x] S28.1 — CPU Profiler: Sampling profiler with configurable frequency (default: 1000Hz)
- [x] S28.2 — Flame Graph: Generate interactive flame graphs from profiling data
- [x] S28.3 — Hot Path Detection: Identify functions consuming most CPU time, rank by inclusive/exclusive time
- [x] S28.4 — Memory Profiler: Track allocation rate, peak usage, allocation hot spots
- [x] S28.5 — Async Profiler: Profile async task scheduling — time in poll, time waiting, wakeup latency
- [x] S28.6 — GPU Profiler: Track GPU kernel launch/completion times, occupancy, memory transfers
- [x] S28.7 — Lock Contention: Detect mutex contention hot spots — time waiting vs. time holding
- [x] S28.8 — I/O Profiler: Track I/O wait time — disk, network, IPC latency breakdown
- [x] S28.9 — Profile-Guided Hints: Suggest optimizations based on profiling data (inline, vectorize, cache)
- [x] S28.10 — Unit Tests: 15+ tests for sampling, flame graph generation, hot path, memory profiling, lock contention

---

## Phase 8: Production Deployment (S29-S32)

> Deploy Fajar Lang services to production — containers, health checks, metrics, tracing, graceful shutdown, hot reload.

### Sprint S29 — Container Support

- [x] S29.1 — Dockerfile Generation: `fj deploy --docker` generates optimized multi-stage Dockerfile
- [x] S29.2 — Minimal Base Image: FROM scratch / distroless for smallest possible container image
- [x] S29.3 — Static Linking: Produce fully static binaries (musl libc) for container deployment
- [x] S29.4 — Container Config: Read config from environment variables and config files, 12-factor app style
- [x] S29.5 — Docker Compose: Generate `docker-compose.yml` for multi-service Fajar Lang applications
- [x] S29.6 — Health Check: Container health check endpoint configured in Dockerfile HEALTHCHECK
- [x] S29.7 — Resource Limits: Respect cgroup memory/CPU limits, adjust thread pool and buffer sizes
- [x] S29.8 — Kubernetes Manifest: `fj deploy --k8s` generates Deployment, Service, ConfigMap YAML
- [x] S29.9 — Helm Chart: Generate Helm chart for parameterized Kubernetes deployment
- [x] S29.10 — Unit Tests: 15+ tests for Dockerfile generation, config loading, resource detection, manifest generation

### Sprint S30 — Observability

- [x] S30.1 — Structured Logging: `log::info!("request processed", method = "GET", status = 200, latency_ms = 42)`
- [x] S30.2 — Log Levels: Trace, Debug, Info, Warn, Error with runtime-configurable level
- [x] S30.3 — Metrics Registry: Counter, Gauge, Histogram metrics with Prometheus-compatible exposition
- [x] S30.4 — Prometheus Endpoint: `/metrics` HTTP endpoint in Prometheus text format
- [x] S30.5 — Distributed Tracing: OpenTelemetry-compatible spans with trace_id, span_id, parent_span_id
- [x] S30.6 — Trace Context Propagation: Propagate trace context across async tasks and RPC calls
- [x] S30.7 — Custom Metrics: `@metric(counter, "requests_total")` annotation on functions
- [x] S30.8 — Alerting Rules: Define alert thresholds in config — trigger when metric exceeds threshold
- [x] S30.9 — Dashboard Generation: `fj observe --dashboard` generates Grafana dashboard JSON
- [x] S30.10 — Unit Tests: 15+ tests for structured logging, metrics exposition, trace propagation, alerting

### Sprint S31 — Runtime Management

- [x] S31.1 — Graceful Shutdown: Handle SIGTERM/SIGINT, drain connections, flush buffers, then exit
- [x] S31.2 — Hot Reload: Reload configuration without restart — `fj reload` sends SIGHUP
- [x] S31.3 — Feature Flags Runtime: Toggle features at runtime via config file or API endpoint
- [x] S31.4 — Connection Draining: Stop accepting new connections, finish in-flight requests, then shutdown
- [x] S31.5 — Process Supervision: Restart on crash with exponential backoff (max 5 restarts in 60s)
- [x] S31.6 — Rolling Update: Zero-downtime deployment via listen-socket handoff between old/new process
- [x] S31.7 — Memory Limits: Configurable heap limit with graceful OOM (reject new work instead of crash)
- [x] S31.8 — Thread Pool Tuning: Adaptive thread pool sizing based on CPU count and load
- [x] S31.9 — Runtime Info Endpoint: `/info` endpoint with version, uptime, config, active connections
- [x] S31.10 — Unit Tests: 15+ tests for shutdown sequence, config reload, feature flags, draining, supervision

### Sprint S32 — Security & Auth

- [x] S32.1 — TLS Configuration: `tls.cert` and `tls.key` in config, automatic HTTPS with Let's Encrypt support
- [x] S32.2 — JWT Validation: Parse and validate JWT tokens with configurable issuer, audience, algorithms
- [x] S32.3 — API Key Auth: `@auth(api_key)` annotation for API key authentication on endpoints
- [x] S32.4 — Rate Limiting: Per-client rate limiting with token bucket algorithm
- [x] S32.5 — CORS Configuration: Configurable CORS headers for browser-accessible APIs
- [x] S32.6 — Secret Management: `fj secret set KEY=VALUE` — encrypted at-rest secrets, injected as env vars
- [x] S32.7 — Audit Logging: Log all authentication and authorization decisions with timestamp and client info
- [x] S32.8 — Input Sanitization: Automatic input sanitization for string parameters (XSS, SQL injection prevention)
- [x] S32.9 — Dependency Audit: `fj audit` scans dependencies for known vulnerabilities (CVE database)
- [x] S32.10 — Unit Tests: 15+ tests for TLS config, JWT parsing, rate limiting, CORS, sanitization, audit

---

## Dependencies

```
Phase 1 (HKT) ─────────────────→ Phase 4 (ML v2) uses Monad for async ML
         │
         └──────────────────────→ Phase 6 (Packages) uses type families for conditional types

Phase 2 (Concurrency) ─────────→ Phase 3 (Distributed) builds on structured concurrency
         │
         └──────────────────────→ Phase 8 (Deployment) uses graceful shutdown + cancellation

Phase 3 (Distributed) ─────────→ Phase 4 (ML v2) distributed training
                                → Phase 8 (Deployment) service deployment

Phase 5 (GPU Codegen) ─────────→ Phase 4 (ML v2) GPU-accelerated inference

Phase 7 (Debugger v2) standalone, can start in parallel with Phase 5-6
```

**Parallelism opportunities:**
- Phases 1 and 2 can run in parallel (HKT and concurrency are independent)
- Phase 5 (GPU Codegen) is independent of Phases 1-4
- Phase 6 (Packages) and Phase 7 (Debugger) are independent
- Phase 8 (Deployment) depends on Phase 2 (concurrency patterns)

---

## Success Criteria

| Criterion | Target |
|-----------|--------|
| Tasks complete | 320/320 |
| Test suite | 5,500+ tests (0 failures) |
| HKT type system | Functor/Applicative/Monad for Option, Result, Vec, Future |
| Do-notation | Desugars correctly for all monad instances |
| Structured concurrency | Zero leaked tasks — scope guarantees all tasks complete or cancel |
| Actor model | Typed actor mailbox, supervision tree with restart strategies |
| Distributed compute | 4-node distributed matmul produces correct results |
| Transformer inference | LLaMA-style model runs with KV cache and Flash Attention |
| GPU codegen | PTX generated from Fajar Lang compiles and runs on NVIDIA GPU |
| Kernel fusion | 2x+ speedup from fusing matmul → relu → bias chains |
| Workspaces | Multi-package workspace builds and tests in correct order |
| Time-travel debug | Replay execution backwards, find root cause of assertion failure |
| Container deploy | `fj deploy --docker` produces working container from Fajar Lang project |
| Observability | Prometheus metrics, OpenTelemetry traces from annotated functions |
| LOC | ~250,000+ lines of Rust |

---

## Release Gate

All of the following MUST pass before tagging v3.0.0:

```bash
# Code quality
cargo test                             # all pass
cargo test --features native           # all pass (including codegen)
cargo clippy -- -D warnings            # zero warnings
cargo fmt -- --check                   # clean

# HKT
# Functor/Monad instances compile and pass monad laws for Option, Result, Vec
# Do-notation desugars correctly
# Type-level computation terminates within recursion limit

# Structured concurrency
# async_scope guarantees all tasks join before scope exit
# Cancellation propagates from parent to all children
# Actor supervision restarts failed actors correctly

# Distributed
# 4-node simulated cluster runs distributed AllReduce
# Checkpoint/restore recovers from simulated node failure
# RPC client/server roundtrip with serialization

# ML v2
# Multi-head attention produces correct output for known inputs
# KV cache matches non-cached attention output
# PPO agent learns CartPole-like environment

# GPU codegen
# PTX output assembles and runs vector addition kernel
# SPIR-V output validates against specification
# Kernel fusion eliminates intermediate allocations

# Packages
# Workspace with 3 members builds in correct order
# Conditional compilation excludes platform-specific code
# Cross-compilation produces valid aarch64 binary

# Debugger v2
# Record mode captures execution trace
# Replay mode reproduces identical execution
# Reverse-step walks backwards through recorded execution

# Deployment
# Docker image builds and runs from generated Dockerfile
# /metrics endpoint serves Prometheus-compatible output
# Graceful shutdown completes in-flight requests before exit

# Phase verification
# All 8 phases verified (320/320 tasks marked [x])

# Documentation
# CHANGELOG.md updated with v3.0.0 entry
```

---

*V30_PLAN.md v1.0 | Created 2026-03-12*
