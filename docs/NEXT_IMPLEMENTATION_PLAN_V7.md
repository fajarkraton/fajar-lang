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

*(CLI commands: `fj cluster status`, `fj run --nodes 4 train.fj`,
`fj deploy --cluster app.fj`. Configuration: cluster.toml with node
addresses, roles, transport settings. Dashboard: real-time cluster
view with node health, actor distribution, message throughput.
Documentation: actor model guide, cluster setup, distributed ML tutorial.
Examples: chat server, distributed MapReduce, fleet management,
edge-cloud inference pipeline, distributed sensor fusion.
Blog: "Distributed ML in Fajar Lang")*

---

## Option 2: WASI Preview 2 & Component Model (6 sprints, 60 tasks)

**Goal:** First-class WASI P2 support — components, WIT interfaces, sandboxed modules
**Impact:** Deploy Fajar Lang to any WASI-compatible runtime (Wasmtime, WasmEdge, Spin)

### Phase W1: WASI Preview 2 Core (2 sprints, 20 tasks)

*(wasi:cli/run interface, wasi:io/streams, wasi:filesystem/preopens,
wasi:sockets/tcp+udp, wasi:clocks/wall-clock+monotonic-clock,
wasi:random/random, wasi:http/incoming-handler+outgoing-handler,
environment variables, command-line args, exit code,
stdin/stdout/stderr mapping, file descriptor management,
pollable abstraction, async I/O via wasi:io/poll,
capability-based security model, resource handle lifecycle,
WASI adapter layer (preview1 → preview2 shim),
wasi-sdk integration, WASI test suite compliance,
WASI preview 2 spec conformance report)*

### Phase W2: Component Model (2 sprints, 20 tasks)

*(WIT (Wasm Interface Type) parser, WIT → Fajar binding generator,
component binary format (module → component wrapper),
interface imports/exports declaration, resource types (handle + drop),
variant/record/enum/flags/tuple/list/option/result WIT types,
canonical ABI (lift/lower functions), component composition (linking),
component dependency resolution, `fj build --component` CLI,
component metadata (name, version, interfaces),
component validation (type checking imports/exports),
cross-language component linking (Fajar + Rust + Go),
component registry (warg protocol), shared-nothing isolation,
component instantiation with capability injection,
WIT documentation generator, component model test suite,
component size optimization, canonical ABI benchmark)*

### Phase W3: Runtime & Deployment (2 sprints, 20 tasks)

*(Wasmtime integration (embed runtime), WasmEdge integration,
Spin (Fermyon) app template, Cloudflare Workers deployment,
Fastly Compute@Edge deployment, AWS Lambda (custom runtime),
Deno/Node.js Wasm import, browser component instantiation,
Docker + Wasm (containerd-wasm-shim), WASI-NN for ML inference,
WASI-threads for parallel computation, wasi:keyvalue interface,
wasi:messaging interface, wasi:observe/tracing interface,
component hot-reload (no-downtime deploy), serverless template,
edge function template, IoT device deployment (via Wasm runtime),
WASI Preview 2 benchmark suite, blog: "Fajar Lang on WASI")*

---

## Option 3: GPU Training Pipeline (CUDA/ROCm) (8 sprints, 80 tasks)

**Goal:** Real GPU-accelerated training — not just inference, full backward pass on GPU
**Impact:** 10-100x training speedup; competitive with PyTorch for small-medium models

### Phase GT1: GPU Kernel Compilation (3 sprints, 30 tasks)

*(CUDA PTX code generation from @device functions, ROCm HIP code gen,
kernel launch configuration (grid/block dimensions), shared memory allocation,
warp-level primitives (shuffle, vote, ballot), atomic operations (add, CAS),
texture memory for read-only data, constant memory for hyperparameters,
dynamic parallelism (kernel launches kernel), cooperative groups,
multi-stream execution (overlap compute + transfer), unified memory,
NVRTC (runtime compilation) integration, PTX JIT cache,
kernel occupancy calculator, register pressure analysis,
bank conflict detection, coalesced memory access analysis,
FP16/BF16 tensor cores on Ampere/Ada, TF32 tensor cores,
INT8 tensor cores for quantized training, WMMA API,
custom CUDA allocator (pooled), async memcpy (H2D, D2H),
peer-to-peer GPU communication (NVLink), GPU error checking (CUDA errors),
multi-GPU detection (nvidia-smi parsing), GPU memory manager,
kernel profiling (nvprof/nsight integration), kernel auto-tuning,
CUDA compilation cache, CUDA test suite)*

### Phase GT2: Training Operators (3 sprints, 30 tasks)

*(GPU matmul (tiled, shared memory), GPU conv2d (im2col + GEMM),
GPU batch normalization (fused forward + backward), GPU dropout (curand),
GPU attention (flash attention algorithm), GPU softmax (online algorithm),
GPU cross-entropy loss, GPU MSE loss, GPU layer normalization,
GPU GELU activation, GPU embedding lookup, GPU pooling (max, avg),
GPU transpose, GPU concat, GPU split, GPU reshape (zero-copy),
GPU backward: matmul grad, GPU backward: conv2d grad,
GPU backward: batch norm grad, GPU backward: attention grad,
GPU optimizer: SGD with momentum, GPU optimizer: Adam/AdamW,
GPU learning rate scheduler, GPU gradient clipping (norm, value),
GPU mixed precision training (FP16 forward, FP32 accumulate),
GPU gradient checkpointing (recompute vs store),
cuBLAS integration for GEMM, cuDNN integration for conv/pool/norm,
NCCL integration for multi-GPU allreduce, training loop orchestration,
GPU training benchmark (MNIST, CIFAR-10))*

### Phase GT3: Integration & Optimization (2 sprints, 20 tasks)

*(Automatic GPU/CPU dispatch (based on tensor size), lazy evaluation DAG,
operation fusion (conv+bn+relu → single kernel), memory planning (liveness),
gradient accumulation (micro-batching), async data loading (prefetch to GPU),
GPU-aware data pipeline, checkpoint save/load (GPU → disk → GPU),
TensorBoard-compatible logging, training progress visualization,
distributed GPU training (data parallel), model export (GPU → ONNX),
GPU inference server (batched requests), A/B model testing,
GPU memory fragmentation defragmentation, GPU thermal throttle detection,
`fj train --gpu examples/mnist.fj`, `fj train --gpus 0,1 --distributed`,
GPU training documentation, GPU setup guide (CUDA/ROCm/Metal),
blog: "GPU Training in Fajar Lang")*

---

## Option 4: Advanced Type System (HKT, GADTs) (6 sprints, 60 tasks)

**Goal:** Higher-kinded types, GADTs, type-level programming — Haskell-level expressiveness
**Impact:** Enable generic programming patterns impossible in Rust/Go (Functor, Monad, etc.)

### Phase AT1: Higher-Kinded Types (2 sprints, 20 tasks)

*(Type constructor parameters (`F<_>` where F is itself a type parameter),
Functor trait: `trait Functor<F> { fn map<A,B>(fa: F<A>, f: fn(A)->B) -> F<B> }`,
Applicative: `trait Applicative<F>: Functor<F> { fn pure<A>(a: A) -> F<A> }`,
Monad: `trait Monad<F>: Applicative<F> { fn flat_map<A,B>(fa: F<A>, f: fn(A)->F<B>) -> F<B> }`,
Traverse: `trait Traverse<T>: Functor<T> { fn traverse<F,A,B>(...) }`,
HKT encoding in type checker, kind system (* → *, * → * → *),
kind inference, kind error messages, HKT monomorphization,
Option as Functor/Monad, Result as Functor/Monad, Array as Functor,
Future as Functor/Monad, do-notation desugaring (`do { x <- read(); ... }`),
monad transformers (OptionT, ResultT), free monad,
HKT + effect system integration, HKT tests (20), HKT spec update)*

### Phase AT2: GADTs & Type-Level Programming (2 sprints, 20 tasks)

*(Generalized Algebraic Data Types: `enum Expr<T> { IntLit(i64) : Expr<i64>, ... }`,
pattern matching with GADT refinement, type equality witnesses,
type-level natural numbers (Zero, Succ), type-level lists (Nil, Cons),
type-level booleans (True, False), type-level arithmetic (Add, Mul),
type-level conditionals (If), singleton types, dependent pairs,
length-indexed vectors (`Vec<T, N>` where N is type-level),
safe matrix operations via GADT (dimensions in types),
proof terms (type-level witnesses for safety properties),
type-safe printf (format string checked at compile time),
type-safe SQL query builder (schema in types),
type-safe state machine (valid transitions only),
GADT + pattern match exhaustiveness, GADT inference limitations,
GADT tests (20), GADT specification, GADT examples)*

### Phase AT3: Type Classes & Deriving (2 sprints, 20 tasks)

*(Orphan rules (coherence), associated type defaults,
blanket implementations (`impl<T: Debug> Display for T`),
negative trait bounds (`!Send`), auto traits (marker traits),
derive macros: `#[derive(Debug, Clone, PartialEq, Hash, Serialize)]`,
custom derive: user-defined derive macros, derive for enums,
derive for generic structs, derive with constraints,
newtype derive (transparent delegation), sealed traits,
specialization (more specific impl overrides generic),
trait aliases (`trait ReadWrite = Read + Write`),
`impl Trait` in argument position, `impl Trait` in return position,
existential types (`type Foo = impl Bar`), type erasure patterns,
const trait methods, trait object safety rules (comprehensive),
type class tests (20), type class specification)*

---

## Option 5: Incremental Compilation & Build System (6 sprints, 60 tasks)

**Goal:** Sub-second recompilation for small changes — file-level dependency tracking
**Impact:** Critical for developer experience; currently full rebuild on every change

### Phase IC1: Dependency Graph (2 sprints, 20 tasks)

*(Module dependency DAG (imports/exports), file → module mapping,
AST hashing (detect unchanged files), signature hashing (detect ABI changes),
incremental parsing (only re-parse changed files), incremental analysis
(only re-check affected modules), query-based architecture (salsa-style),
demand-driven computation, memoized query results, change propagation
(invalidate dependents), parallel analysis (per-module), dependency cycle
detection, build order computation (topological sort), cache directory
(`.fj-cache/`), cache serialization (bincode), cache invalidation
(on compiler version change), stale cache cleanup, dependency visualization
(`fj deps --graph`), build profiling (`fj build --timings`),
incremental build tests)*

### Phase IC2: Compilation Cache (2 sprints, 20 tasks)

*(Per-function IR cache (Cranelift IR), per-module object cache (.o files),
linking cache (only re-link changed objects), content-addressable storage
(hash-based dedup), shared cache (team-wide, CI), remote cache (S3/GCS),
cache compression (zstd), cache hit/miss metrics, cache size management
(LRU eviction), cache warming on CI, distributed cache protocol,
binary artifact cache, test result cache (skip unchanged tests),
incremental codegen (only recompile changed functions), hot-reload
(replace module at runtime), file watcher integration (`fj watch`),
build daemon (persistent compiler process), build progress reporting
(ETA, files remaining), parallel compilation (per-module),
build system benchmarks)*

### Phase IC3: Package Build System (2 sprints, 20 tasks)

*(Workspace support (multi-package projects), build scripts (`build.fj`),
conditional compilation (`#[cfg(target_os = "linux")]`), feature flags
(`[features]` in fj.toml), build profiles (dev, release, bench),
cross-compilation targets, custom linker configuration, output types
(binary, library, cdylib, staticlib, wasm), artifact naming conventions,
build metadata (git hash, build time), reproducible builds (lockfile),
`fj clean`, `fj check` (type-check only), `fj build --release`,
`fj build --target wasm32-wasi`, build cache sharing between packages,
workspace dependency deduplication, `fj test --changed` (only test affected),
package build graph visualization, build system documentation,
build system integration tests)*

---

## Option 6: Cloud & Edge Deployment (7 sprints, 70 tasks)

**Goal:** One-command deploy to cloud (AWS/GCP/Azure), edge (Cloudflare/Fastly), IoT
**Impact:** Bridge the gap from "it compiles" to "it runs in production"

### Phase CD1: Container & Serverless (3 sprints, 30 tasks)

*(Dockerfile generation (`fj deploy --dockerfile`), multi-stage build
(builder + runtime), distroless base images, Docker Compose for services,
Kubernetes deployment manifest generator, Helm chart template,
AWS Lambda deployment (custom runtime, provided.al2023),
Google Cloud Run deployment, Azure Container Apps deployment,
Cloudflare Workers (Wasm), Fastly Compute@Edge (Wasm),
Fly.io deployment, Railway deployment, Vercel Edge Functions,
health check endpoint generation, graceful shutdown handler,
container resource limits (CPU, memory), auto-scaling configuration,
secret management (env vars, mounted files), logging integration
(structured JSON), tracing integration (OpenTelemetry), metrics export
(Prometheus), Kubernetes liveness/readiness probes, service mesh
(Istio sidecar), blue-green deployment, canary deployment,
rollback mechanism, deployment documentation, deployment benchmarks,
`fj deploy --aws-lambda`, `fj deploy --cloudflare`)*

### Phase CD2: Edge & IoT (2 sprints, 20 tasks)

*(Edge runtime (Wasm + WASI on ARM), IoT device provisioning,
OTA firmware update (delta encoding), device fleet management,
device shadow (digital twin), MQTT client for telemetry,
CoAP protocol for constrained devices, LwM2M device management,
edge inference pipeline (sensor → model → actuator),
model deployment to fleet (staged rollout), edge-cloud sync
(batch upload telemetry), offline operation (queue + sync),
power-aware scheduling on battery devices, sleep/wake management,
watchdog timer integration, secure boot chain,
device identity (X.509 certificates), remote debugging (serial over network),
fleet dashboard, edge deployment documentation)*

### Phase CD3: Observability & Operations (2 sprints, 20 tasks)

*(Structured logging (JSON, log levels, context), distributed tracing
(OpenTelemetry SDK, W3C Trace Context), metrics collection (counters,
gauges, histograms), Prometheus exporter, Grafana dashboard templates,
alerting rules (PagerDuty/Slack integration), error tracking (Sentry-style),
performance monitoring (APM), request tracing (end-to-end latency),
ML model monitoring (data drift, accuracy degradation),
cost monitoring (cloud spend per service), SLA tracking,
incident response runbook generator, chaos engineering
(fault injection), load testing (`fj bench --http --rps 10000`),
profiling in production (sampling), memory leak detection in production,
auto-remediation (restart on OOM), observability documentation,
observability integration tests)*

---

## Option 7: Plugin & Extension System (5 sprints, 50 tasks)

**Goal:** User-extensible compiler — custom lints, code generators, transformations
**Impact:** Community can extend the language without forking

### Phase PL1: Plugin Architecture (2 sprints, 20 tasks)

*(Plugin trait: `trait CompilerPlugin { fn name(); fn on_ast(); fn on_ir(); }`,
plugin discovery (fj.toml `[plugins]` section), plugin loading (dylib),
plugin API versioning, plugin sandboxing (no filesystem access by default),
AST visitor API (read-only), AST transformer API (modify AST),
IR visitor/transformer API, diagnostic API (emit warnings/errors),
code generation API (emit extra code), plugin configuration (TOML),
plugin dependency resolution, plugin registry (package manager),
plugin testing framework, plugin documentation generator,
built-in plugins: unused code, complexity metric, naming conventions,
plugin lifecycle (init, per-file, per-function, finalize),
plugin performance budget (max 100ms per file), plugin error isolation,
plugin API documentation)*

### Phase PL2: Custom Lints & Code Gen (2 sprints, 20 tasks)

*(Custom lint rules (pattern matching on AST), lint severity configuration,
lint suppression (`@allow(my_lint)`), auto-fix for custom lints,
code generator plugins (generate .fj from schema), protobuf code gen,
GraphQL schema code gen, database ORM code gen, REST API client gen,
OpenAPI spec code gen, FFI binding generator plugin,
documentation generator plugin, benchmark code gen,
test scaffolding generator, migration script generator,
i18n string extraction, accessibility checker,
security audit plugin (detect hardcoded secrets),
dependency vulnerability scanner, code style enforcer,
plugin marketplace concept)*

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

*(Linux x86_64 (Ubuntu 22.04/24.04), macOS x86_64 + ARM64,
Windows x86_64 (MSVC + GNU), cross-compile: aarch64-linux,
cross-compile: armv7-linux, cross-compile: riscv64gc-linux,
cross-compile: wasm32-wasi, cross-compile: wasm32-unknown-unknown,
Rust stable + nightly, QEMU boot test (x86_64), QEMU boot test (aarch64),
cargo clippy (zero warnings), cargo fmt (check), cargo doc (no warnings),
cargo test (default features), cargo test --features native,
cargo test --features llvm, fuzz run (1M iterations per target),
benchmark comparison (vs previous release), code coverage (tarpaulin),
binary size tracking, CI caching (cargo registry + target))*

### Phase CI2: Release Automation (2 sprints, 20 tasks)

*(Semantic versioning enforcement, CHANGELOG.md auto-generation,
git tag creation, GitHub Release creation, binary artifact upload,
platform-specific archives (tar.gz Linux/macOS, zip Windows),
homebrew formula generation, AUR PKGBUILD generation, snap package,
Debian .deb package, RPM .rpm package, Docker image (multi-arch),
crates.io publish (library), npm publish (wasm package),
PyPI publish (Python bindings), cargo-binstall support,
GitHub Actions release workflow, release notes generation (from commits),
pre-release (alpha/beta/rc) channel, release signing (GPG/sigstore))*

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

*(MMU completion (4KB + 64KB pages), process address space isolation,
kernel/user page table split (TTBR0/TTBR1), exception vector table (EL1),
SVC syscall handler (EL0 → EL1), interrupt controller (GICv3 + GICv2),
timer interrupt (ARM Generic Timer), context switch (save/restore X0-X30, SP, PCTX),
process creation (fork/exec), ELF loader (aarch64 relocations),
preemptive scheduler (round-robin with priorities), IPC (message passing),
shared memory, signal delivery (SIGKILL, SIGTERM, SIGCHLD),
waitpid, process groups, session leader, orphan reaping,
kernel heap (buddy allocator), slab allocator for small objects,
kernel stack (per-process, 16KB), guard pages, stack canary,
kernel panic handler, kernel logging (serial UART), boot log,
device tree (DTB) parsing, clock tree initialization,
power domain management, early console output,
kernel module framework, syscall table (40+ entries),
kernel test framework (in-kernel assert), kernel documentation)*

### Phase OS2: Drivers & Hardware (3 sprints, 30 tasks)

*(UART (PL011/QUP) driver, GPIO driver (gpiochip4 on Q6A),
I2C driver (QUP), SPI driver (QUP), PWM driver,
SD/eMMC driver (SDHCI), NVMe driver (PCIe), USB XHCI driver,
Display driver (DSI/HDMI via DPU), Camera driver (libcamera ISP),
WiFi driver (WCNSS/ath11k), Bluetooth driver (HCI over UART),
Audio driver (WSA macro codec), GPU driver (Adreno 643 stub),
NPU driver (Hexagon DSP/HTP), watchdog timer driver,
RTC driver (DS1307), thermal sensor driver, power management IC,
regulators (SPMI/PMIC), clock controllers (GCC, DISPCC, GPUCC),
pin controller (TLMM), DMA engine (BAM/GPI), IOMMU (SMMU),
virtio drivers (QEMU: net, blk, console, gpu),
PCIe host controller, USB Type-C (UCSI), charger IC driver,
driver model (probe/remove/suspend/resume),
hardware abstraction: `trait Driver { fn probe(); }`,
interrupt routing (GIC → driver), driver documentation)*

### Phase OS3: Filesystem & Services (2 sprints, 20 tasks)

*(VFS layer (inode operations, dentry cache), ramfs (memory filesystem),
ext2 read support, FAT32 read/write support, procfs (/proc),
sysfs (/sys), devfs (/dev with mknod), tmpfs, pipe filesystem,
mount/umount syscalls, path resolution (with symlink follow),
file descriptor table (per-process, 256 max), file locking (flock),
directory operations (opendir, readdir, mkdir, rmdir),
init process (PID 1), service manager (start/stop/restart),
shell (built-in commands: ls, cat, echo, cd, pwd, ps, kill),
cron daemon (scheduled tasks), syslog daemon, network stack
(TCP/IP minimal), DHCP client, DNS resolver)*

### Phase OS4: Q6A Hardware Integration (2 sprints, 20 tasks)

*(Cross-compile aarch64-unknown-none, UEFI boot on Q6A,
GPIO blink test (GPIO96), I2C sensor read, SPI flash test,
WiFi scan + connect, Bluetooth scan, camera capture (libcamera),
QNN NPU inference (CPU backend), QNN NPU inference (GPU backend),
Vulkan compute shader on Adreno 643, display output (DSI),
USB device enumeration, NVMe read/write, audio playback,
thermal monitoring, power management (suspend/resume),
real-time clock, kernel boot time optimization (< 2s),
full system benchmark, hardware validation report)*

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
