# Fajar Lang v0.8 "Apex" — Implementation Plan

> **Focus:** GPU training, GAT/async traits, incremental compilation, model optimization, DAP debugger
> **Timeline:** 28 sprints, ~280 tasks, 4-6 months
> **Prerequisite:** v0.7 "Zenith" RELEASED
> **Theme:** *"Reach the apex — GPU-powered training, type system maturity, compiler intelligence"*

---

## Motivation

v0.7 delivered WebAssembly, IoT connectivity, Polonius borrow checker, Transformer ML, and package signing. But critical gaps remain for production-grade embedded ML:

- **No GPU-accelerated training** — inference is fast but training on CPU is 10-100x slower than GPU
- **No Generic Associated Types (GAT)** — async traits, lending iterators, and zero-cost abstractions blocked
- **No incremental compilation** — full recompile on every change, slow iteration for large codebases
- **No model pruning/distillation** — deployed models are larger than necessary for edge devices
- **No DAP debugger** — can't step-debug Fajar programs in VS Code
- **No LoRaWAN** — long-range IoT deployments need sub-GHz radio support
- **No auto-differentiation for custom ops** — users can't define custom loss functions with gradients

v0.8 targets these gaps to make Fajar Lang the complete platform for GPU-trained, edge-deployed ML.

---

## Architecture Decisions

| # | Decision | Rationale |
|---|----------|-----------|
| 1 | CUDA tensor core kernels via PTX emission | Direct GPU control, no wgpu overhead for training |
| 2 | cuDNN-style conv/matmul wrappers | Proven algorithms, avoid reimplementing GEMM |
| 3 | GAT via associated type projections | Same approach as Rust, enables async trait + lending iterator |
| 4 | File-level dependency graph for incremental | Fine-grained enough, avoids per-function complexity |
| 5 | Structured pruning (channel/filter) | Hardware-friendly, no sparse tensor support needed |
| 6 | Knowledge distillation with soft labels | Standard teacher-student approach, works with existing loss functions |
| 7 | DAP (Debug Adapter Protocol) via tower-lsp extension | Reuse existing LSP server, VS Code native support |
| 8 | LoRaWAN via simulation stubs (like WiFi/BLE) | Same pattern as IoT module, real hardware via feature gate |
| 9 | JVP/VJP framework for custom autodiff | Forward-mode (JVP) + reverse-mode (VJP), composable |
| 10 | Artifact cache in `~/.fj/cache/` | Persistent across builds, hash-based invalidation |

---

## Sprint Plan

### Phase 1: GPU-Accelerated Training `P0` `CRITICAL`

#### Sprint 1: CUDA Device Management `P0`

**Goal:** GPU device detection, memory allocation, host-device transfer

- [ ] S1.1 — `src/runtime/ml/gpu/mod.rs`: GPU module with `GpuDevice` trait, `CudaDevice` struct
- [ ] S1.2 — `detect_gpu_devices() -> Vec<GpuDeviceInfo>`: enumerate GPUs (simulation: return 1 RTX 4090)
- [ ] S1.3 — `GpuDeviceInfo` struct: name, compute_capability, memory_bytes, cuda_cores, tensor_cores
- [ ] S1.4 — `gpu_alloc(size_bytes) -> Result<GpuBuffer>`: device memory allocation simulation
- [ ] S1.5 — `gpu_free(buffer)`: device memory deallocation
- [ ] S1.6 — `host_to_device(data: &[f64]) -> Result<GpuBuffer>`: upload tensor data to GPU
- [ ] S1.7 — `device_to_host(buffer: &GpuBuffer) -> Result<Vec<f64>>`: download tensor data from GPU
- [ ] S1.8 — `GpuBuffer` struct: device_id, ptr (u64), size_bytes, dtype — with Drop for auto-free
- [ ] S1.9 — `GpuMemoryPool`: pre-allocate memory pool, sub-allocate for tensors to reduce alloc overhead
- [ ] S1.10 — 10 tests: device detection, alloc/free, host-device transfer roundtrip, memory pool

#### Sprint 2: GPU Tensor Kernels `P0`

**Goal:** Core tensor operations as GPU kernels

- [ ] S2.1 — `gpu_matmul(a: &GpuBuffer, b: &GpuBuffer, m, n, k) -> GpuBuffer`: matrix multiply kernel
- [ ] S2.2 — `gpu_elementwise_add/sub/mul/div`: element-wise binary operations
- [ ] S2.3 — `gpu_relu/sigmoid/tanh/gelu`: activation function kernels
- [ ] S2.4 — `gpu_softmax(buffer, axis) -> GpuBuffer`: numerically stable softmax
- [ ] S2.5 — `gpu_transpose(buffer, rows, cols) -> GpuBuffer`: matrix transpose
- [ ] S2.6 — `gpu_reduce_sum/max/mean(buffer, axis) -> GpuBuffer`: reduction operations
- [ ] S2.7 — `gpu_conv2d(input, weight, bias, stride, padding) -> GpuBuffer`: 2D convolution
- [ ] S2.8 — `gpu_batch_norm(input, gamma, beta, running_mean, running_var) -> GpuBuffer`
- [ ] S2.9 — `GpuKernelConfig`: block_size, grid_size, shared_memory for launch configuration
- [ ] S2.10 — 10 tests: matmul correctness, elementwise ops, activation fns, softmax, conv2d shape

#### Sprint 3: GPU Autograd & Training Loop `P0`

**Goal:** Backward pass on GPU, optimizer step, training loop

- [ ] S3.1 — `GpuTape`: record GPU operations for backward pass (parallel to CPU AutogradTape)
- [ ] S3.2 — `gpu_backward(tape, loss) -> HashMap<String, GpuBuffer>`: compute gradients on GPU
- [ ] S3.3 — `GpuSGD` optimizer: `step(params, grads, lr)` — GPU-side parameter update
- [ ] S3.4 — `GpuAdam` optimizer: first/second moment buffers on GPU, bias correction
- [ ] S3.5 — `GpuTrainingLoop`: forward → loss → backward → optimizer step, all on GPU
- [ ] S3.6 — `mixed_precision_gpu`: FP32 master weights, FP16 forward/backward, loss scaling
- [ ] S3.7 — `gpu_gradient_clipping(grads, max_norm) -> GpuBuffer`: prevent exploding gradients
- [ ] S3.8 — `DataPrefetcher`: async host-to-device transfer overlapped with computation
- [ ] S3.9 — `GpuBenchmark`: measure FLOPS, memory bandwidth, kernel launch overhead
- [ ] S3.10 — 10 tests: autograd tape, backward pass, SGD step, Adam step, training loop convergence

#### Sprint 4: Multi-GPU & Benchmarks `P0`

**Goal:** Data-parallel training across multiple GPUs, performance benchmarks

- [ ] S4.1 — `DataParallel` wrapper: replicate model across GPUs, split batch, aggregate gradients
- [ ] S4.2 — `gpu_all_reduce(buffers: &[GpuBuffer]) -> GpuBuffer`: cross-GPU gradient aggregation
- [ ] S4.3 — `GpuSyncBarrier`: synchronize multiple GPU streams before aggregation
- [ ] S4.4 — `gpu_scatter(data, devices) -> Vec<GpuBuffer>`: distribute data to multiple GPUs
- [ ] S4.5 — `gpu_gather(buffers) -> GpuBuffer`: collect results from multiple GPUs
- [ ] S4.6 — Benchmark: MNIST training GPU vs CPU (expect 10-50x speedup simulation)
- [ ] S4.7 — Benchmark: Transformer training GPU vs CPU
- [ ] S4.8 — GPU memory profiler: `gpu_memory_usage() -> GpuMemStats` (peak, current, fragmentation)
- [ ] S4.9 — `examples/gpu_mnist_train.fj`: GPU-accelerated MNIST training example
- [ ] S4.10 — 10 tests: data parallel, all-reduce, scatter/gather, benchmarks, memory profiler

### Phase 2: Generic Associated Types (GAT) `P0`

#### Sprint 5: GAT Parsing & Type System `P0`

**Goal:** Parse GAT syntax, extend type system with associated type projections

- [ ] S5.1 — Parser: `type Item<'a>` in trait definitions — lifetime-parameterized associated types
- [ ] S5.2 — Parser: `type Output<T>` — type-parameterized associated types
- [ ] S5.3 — AST: `AssociatedType` node with optional generic params and bounds
- [ ] S5.4 — Type system: `TypeProjection` — `<T as Trait>::Item<'a>` resolution
- [ ] S5.5 — Type checker: validate GAT params match trait definition
- [ ] S5.6 — Impl validation: ensure associated type impl provides correct params
- [ ] S5.7 — Where clause support: `where Self::Item<'a>: Clone`
- [ ] S5.8 — Higher-kinded type approximation: `trait Functor { type Output<T>; }`
- [ ] S5.9 — GAT lifetime bounds: `type Item<'a> where Self: 'a`
- [ ] S5.10 — 10 tests: parsing, type resolution, impl validation, where clauses, HKT patterns

#### Sprint 6: Async Trait Methods `P0`

**Goal:** `async fn` in traits using GAT desugaring

- [ ] S6.1 — `async fn` in trait: desugar to `fn method() -> impl Future<Output = T>`
- [ ] S6.2 — GAT bridge: `type MethodFuture<'a>: Future<Output = T> where Self: 'a`
- [ ] S6.3 — Async trait impl: verify return type implements `Future`
- [ ] S6.4 — Object-safe async traits: `Box<dyn AsyncTrait>` with dynamic dispatch
- [ ] S6.5 — `#[async_trait]` attribute for automatic boxing (ergonomic mode)
- [ ] S6.6 — Async trait method call: `.await` on trait method returns
- [ ] S6.7 — Lifetime capture in async trait: ensure borrowed data lives long enough
- [ ] S6.8 — Default async trait methods: `async fn default_method(&self) { ... }`
- [ ] S6.9 — `examples/async_trait_demo.fj`: async reader/writer traits with implementations
- [ ] S6.10 — 10 tests: desugaring, impl validation, object safety, lifetime capture, defaults

#### Sprint 7: Lending Iterators & Streaming `P0`

**Goal:** GAT-enabled lending iterators that borrow from the container

- [ ] S7.1 — `LendingIterator` trait: `type Item<'a> where Self: 'a; fn next(&mut self) -> Option<Self::Item<'_>>`
- [ ] S7.2 — `WindowsIter`: yields overlapping windows borrowing from the slice
- [ ] S7.3 — `ChunksIter`: yields non-overlapping chunks borrowing from the array
- [ ] S7.4 — `LinesIter`: yields string slices from a string buffer
- [ ] S7.5 — `map()` adapter: `fn map<F>(self, f: F) -> Map<Self, F>` preserving lending
- [ ] S7.6 — `filter()` adapter: `fn filter<F>(self, f: F) -> Filter<Self, F>`
- [ ] S7.7 — `for_each()` consumer: `fn for_each<F>(self, f: F)` with lending semantics
- [ ] S7.8 — `StreamingIterator` for async: `type Item<'a>; async fn next(&mut self) -> Option<Self::Item<'_>>`
- [ ] S7.9 — Integration: standard library `Array` gains lending `windows()` and `chunks()` methods
- [ ] S7.10 — 10 tests: lending next, windows, chunks, lines, map/filter adapters, streaming

#### Sprint 8: GAT Error Messages & Polish `P1`

**Goal:** Clear error messages for GAT misuse, migration guide

- [ ] S8.1 — Error: "associated type `Item` requires lifetime parameter" with suggestion
- [ ] S8.2 — Error: "impl does not match trait: expected 1 type parameter, found 0"
- [ ] S8.3 — Error: "borrowed data does not live long enough for GAT projection"
- [ ] S8.4 — Error: "async trait method requires `#[async_trait]` for object safety"
- [ ] S8.5 — Suggestion: "consider adding lifetime bound `where Self: 'a`"
- [ ] S8.6 — Error codes: GE001 MissingGatParams, GE002 GatBoundMismatch, GE003 GatLifetimeCapture
- [ ] S8.7 — GAT inference: auto-infer lifetime parameters where unambiguous
- [ ] S8.8 — Performance: GAT resolution completes in < 50ms for programs up to 10K LOC
- [ ] S8.9 — `--gat` feature flag: enable GAT (default on from v0.8)
- [ ] S8.10 — 10 tests: all error messages, suggestions, inference, feature flag

### Phase 3: Incremental Compilation `P1`

#### Sprint 9: Dependency Graph `P1`

**Goal:** Build file-level dependency graph for change detection

- [ ] S9.1 — `src/compiler/incremental/mod.rs`: incremental compilation module
- [ ] S9.2 — `FileNode` struct: path, content_hash (SHA-256), dependencies Vec<FilePath>, exports Vec<Symbol>
- [ ] S9.3 — `DependencyGraph`: directed graph of FileNode dependencies
- [ ] S9.4 — `build_dependency_graph(files: &[FilePath]) -> DependencyGraph`: parse imports/use statements
- [ ] S9.5 — `compute_content_hash(source: &str) -> String`: SHA-256 of normalized source
- [ ] S9.6 — `detect_changes(old_graph, new_graph) -> Vec<FilePath>`: compare hashes, find changed files
- [ ] S9.7 — `transitive_dependents(changed: &[FilePath]) -> Vec<FilePath>`: all files that need recompile
- [ ] S9.8 — `topological_sort(graph) -> Vec<FilePath>`: compilation order respecting dependencies
- [ ] S9.9 — Cycle detection: error if circular imports found
- [ ] S9.10 — 10 tests: graph building, hash computation, change detection, transitive deps, topo sort, cycles

#### Sprint 10: Artifact Cache `P1`

**Goal:** Cache compiled artifacts, skip recompilation for unchanged files

- [ ] S10.1 — `ArtifactCache` struct: cache_dir (~/.fj/cache/), artifacts HashMap<ContentHash, CachedArtifact>
- [ ] S10.2 — `CachedArtifact`: content_hash, compiled_at, artifact_type (AST/IR/Object), data bytes
- [ ] S10.3 — `cache_store(hash, artifact) -> Result<()>`: write artifact to disk cache
- [ ] S10.4 — `cache_lookup(hash) -> Option<CachedArtifact>`: check if valid cached artifact exists
- [ ] S10.5 — `cache_invalidate(hash)`: remove specific artifact
- [ ] S10.6 — `cache_prune(max_age_days, max_size_mb)`: garbage collect old/large cache entries
- [ ] S10.7 — Cache key: content_hash + compiler_version + target + optimization_level
- [ ] S10.8 — Parallel cache reads: multiple files can check cache simultaneously
- [ ] S10.9 — `IncrementalCompiler`: orchestrates graph + cache for smart recompilation
- [ ] S10.10 — 10 tests: store/lookup roundtrip, invalidation, pruning, cache key, parallel reads

#### Sprint 11: Incremental Pipeline Integration `P1`

**Goal:** Integrate incremental compilation into the main build pipeline

- [ ] S11.1 — `fj build --incremental` CLI flag (default on for repeat builds)
- [ ] S11.2 — First build: full compile, populate cache for all files
- [ ] S11.3 — Subsequent builds: hash files, check cache, recompile only changed + dependents
- [ ] S11.4 — `fj build --clean-cache`: clear artifact cache, force full rebuild
- [ ] S11.5 — Build report: "Compiled 3/50 files (94% cache hit rate)"
- [ ] S11.6 — Watch mode foundation: `fj build --watch` detects file changes, triggers incremental rebuild
- [ ] S11.7 — Parallel compilation: independent files compiled in parallel (rayon-style simulation)
- [ ] S11.8 — Error recovery: if cached artifact is corrupt, fall back to full recompile
- [ ] S11.9 — Cache statistics: `fj cache stats` shows hit rate, size, entry count
- [ ] S11.10 — 10 tests: incremental flag, cache hit/miss, clean cache, build report, watch trigger, parallel

#### Sprint 12: Compilation Speed Benchmarks `P1`

**Goal:** Measure and optimize compilation speed

- [ ] S12.1 — Benchmark: 100-file project full build time (baseline)
- [ ] S12.2 — Benchmark: 100-file project incremental build (1 file changed)
- [ ] S12.3 — Benchmark: 100-file project incremental build (10 files changed)
- [ ] S12.4 — Target: incremental rebuild < 100ms for single-file change
- [ ] S12.5 — Lazy parsing: only parse changed files, reuse cached ASTs
- [ ] S12.6 — Lazy analysis: only re-analyze files whose dependencies changed
- [ ] S12.7 — Memory-mapped cache: `mmap` artifact files for zero-copy reads (simulation)
- [ ] S12.8 — Compile time profiling: `fj build --time-report` shows time per compilation phase
- [ ] S12.9 — Bottleneck identification: report slowest files and phases
- [ ] S12.10 — 10 tests: benchmark harness, lazy parsing, lazy analysis, time report format

### Phase 4: Model Optimization `P1`

#### Sprint 13: Structured Pruning `P1`

**Goal:** Remove unimportant channels/filters to reduce model size

- [ ] S13.1 — `src/runtime/ml/pruning.rs`: pruning module
- [ ] S13.2 — `PruningStrategy` enum: MagnitudeBased, GradientBased, RandomBased
- [ ] S13.3 — `ChannelPruning`: compute channel importance (L1 norm of filter weights), prune lowest
- [ ] S13.4 — `prune_dense(layer, ratio) -> PrunedDense`: remove columns with lowest magnitude
- [ ] S13.5 — `prune_conv2d(layer, ratio) -> PrunedConv2d`: remove filters with lowest L1 norm
- [ ] S13.6 — `PruningSchedule`: gradual pruning (start_ratio → target_ratio over N epochs)
- [ ] S13.7 — `fine_tune_after_pruning()`: retrain pruned model for accuracy recovery
- [ ] S13.8 — `pruning_report(model) -> PruningReport`: show parameter reduction, estimated speedup
- [ ] S13.9 — `PruningMask`: binary mask tracking which weights are pruned (for iterative pruning)
- [ ] S13.10 — 10 tests: magnitude pruning, channel selection, schedule, fine-tuning, report

#### Sprint 14: Knowledge Distillation `P1`

**Goal:** Train small student model from large teacher model

- [ ] S14.1 — `DistillationConfig`: teacher model, student model, temperature, alpha (soft/hard loss mix)
- [ ] S14.2 — `soft_labels(teacher_logits, temperature) -> Array2<f64>`: softmax with temperature scaling
- [ ] S14.3 — `distillation_loss(student_logits, teacher_logits, targets, alpha, temperature) -> f64`
- [ ] S14.4 — `DistillationTrainer`: orchestrates teacher inference + student training
- [ ] S14.5 — `feature_distillation(teacher_features, student_features) -> f64`: intermediate layer matching
- [ ] S14.6 — `attention_transfer(teacher_attn_maps, student_attn_maps) -> f64`: attention map alignment
- [ ] S14.7 — Progressive distillation: reduce teacher size in stages
- [ ] S14.8 — `distillation_report()`: accuracy comparison (teacher vs student), compression ratio
- [ ] S14.9 — `examples/distillation_demo.fj`: distill large model to small edge model
- [ ] S14.10 — 10 tests: soft labels, distillation loss, trainer loop, feature matching, report

#### Sprint 15: Custom Autodiff (JVP/VJP) `P1`

**Goal:** User-definable gradients for custom operations

- [ ] S15.1 — `src/runtime/ml/custom_grad.rs`: custom gradient module
- [ ] S15.2 — `CustomOp` trait: `forward(inputs) -> Output` + `backward(grad_output) -> Vec<grad_input>`
- [ ] S15.3 — `JVP` (Jacobian-Vector Product): forward-mode autodiff for tangent propagation
- [ ] S15.4 — `VJP` (Vector-Jacobian Product): reverse-mode autodiff for gradient computation
- [ ] S15.5 — `register_custom_op(name, forward_fn, backward_fn)`: register user-defined op
- [ ] S15.6 — Autograd tape integration: custom ops recorded on tape like built-in ops
- [ ] S15.7 — `numerical_gradient_check(op, inputs, epsilon)`: verify custom gradients numerically
- [ ] S15.8 — Composition: chain multiple custom ops, gradients compose correctly
- [ ] S15.9 — Built-in custom ops library: Swish, Mish, GELU variants, Focal Loss
- [ ] S15.10 — 10 tests: custom op registration, JVP, VJP, tape integration, gradient check, composition

#### Sprint 16: Model Compression Pipeline `P1`

**Goal:** End-to-end pipeline: train → prune → distill → quantize → export

- [ ] S16.1 — `CompressionPipeline` struct: sequence of compression stages
- [ ] S16.2 — Stage: `Train(config)` → initial model training
- [ ] S16.3 — Stage: `Prune(ratio, strategy)` → structured pruning with fine-tuning
- [ ] S16.4 — Stage: `Distill(teacher, config)` → knowledge distillation to smaller architecture
- [ ] S16.5 — Stage: `Quantize(dtype)` → INT8/FP16 quantization (existing quantize module)
- [ ] S16.6 — Stage: `Export(format)` → ONNX or TFLite export
- [ ] S16.7 — Pipeline report: model size at each stage, accuracy at each stage, total compression ratio
- [ ] S16.8 — `fj compress --model model.fj --target-size 1MB`: automatic pipeline selection
- [ ] S16.9 — `examples/compression_pipeline.fj`: full pipeline demo (10MB → 500KB model)
- [ ] S16.10 — 10 tests: pipeline stages, report generation, auto-selection, end-to-end

### Phase 5: DAP Debugger `P2`

#### Sprint 17: Debug Info Generation `P2`

**Goal:** Emit debug information for source-level debugging

- [ ] S17.1 — `src/debugger/dap/mod.rs`: DAP module
- [ ] S17.2 — `DebugInfo` struct: source_map (instruction → source line), local_variables, breakpoints
- [ ] S17.3 — `SourceMap`: mapping from compiled instruction index to (file, line, column)
- [ ] S17.4 — `VariableInfo`: name, type_name, scope (local/parameter/global), memory_location
- [ ] S17.5 — Generate debug info during compilation: tag each instruction with source location
- [ ] S17.6 — `BreakpointManager`: set/clear breakpoints by file:line, track hit counts
- [ ] S17.7 — `DebugState`: current instruction, call stack frames, local variable values
- [ ] S17.8 — DWARF-compatible debug sections (simulation for native codegen)
- [ ] S17.9 — Inline function mapping: step into/over inlined functions
- [ ] S17.10 — 10 tests: source map, variable info, breakpoint management, debug state, DWARF

#### Sprint 18: DAP Protocol Implementation `P2`

**Goal:** Implement Debug Adapter Protocol for VS Code integration

- [ ] S18.1 — DAP message types: Initialize, Launch, SetBreakpoints, Continue, Next, StepIn, StepOut
- [ ] S18.2 — DAP response types: Capabilities, Breakpoint, StackTrace, Scopes, Variables
- [ ] S18.3 — `DapServer` struct: handle JSON-RPC messages over stdin/stdout
- [ ] S18.4 — `initialize` handler: report capabilities (supportsStepBack, supportsEvaluate)
- [ ] S18.5 — `launch` handler: start interpreter with debug info, pause at entry
- [ ] S18.6 — `setBreakpoints` handler: register breakpoints, return verified locations
- [ ] S18.7 — `continue/next/stepIn/stepOut` handlers: advance execution with debug control
- [ ] S18.8 — `stackTrace` handler: return call stack frames with source locations
- [ ] S18.9 — `variables` handler: return local variables with current values
- [ ] S18.10 — 10 tests: message parsing, capabilities, breakpoint setting, step operations, variable display

#### Sprint 19: Watch Expressions & Conditional Breakpoints `P2`

**Goal:** Advanced debugging features

- [ ] S19.1 — `evaluate` handler: evaluate expression in current scope, return value
- [ ] S19.2 — Watch expressions: list of expressions evaluated on each stop
- [ ] S19.3 — Conditional breakpoints: `break if count > 10` — evaluate condition at breakpoint
- [ ] S19.4 — Hit count breakpoints: break after N hits (`hitCondition: ">=5"`)
- [ ] S19.5 — Log points: `logMessage: "x = {x}"` — print without stopping
- [ ] S19.6 — Exception breakpoints: break on RuntimeError, SemanticError
- [ ] S19.7 — `setVariable` handler: modify variable value during debugging
- [ ] S19.8 — Memory view: `readMemory` handler for raw memory inspection (pointer values)
- [ ] S19.9 — `disconnect` handler: clean shutdown of debug session
- [ ] S19.10 — 10 tests: evaluate, watch, conditional break, hit count, log point, exception break

#### Sprint 20: VS Code Debug Extension `P2`

**Goal:** VS Code extension for Fajar Lang debugging

- [ ] S20.1 — `launch.json` configuration: `{ "type": "fj", "request": "launch", "program": "main.fj" }`
- [ ] S20.2 — Debug adapter registration in VS Code extension (editors/vscode/)
- [ ] S20.3 — Breakpoint gutter icons: set/remove breakpoints by clicking line gutter
- [ ] S20.4 — Variables panel: show local variables, parameters, globals with types and values
- [ ] S20.5 — Call stack panel: show function call chain with source locations
- [ ] S20.6 — Debug toolbar: Continue, Step Over, Step Into, Step Out, Restart, Stop
- [ ] S20.7 — Debug console: evaluate expressions, inspect runtime state
- [ ] S20.8 — Hover evaluation: hover over variable to see current value
- [ ] S20.9 — `examples/debug_demo.fj`: program designed to showcase debugging features
- [ ] S20.10 — 10 tests: launch config schema, adapter registration, variable display, stack trace

### Phase 6: LoRaWAN IoT `P2`

#### Sprint 21: LoRaWAN MAC Layer `P2`

**Goal:** LoRaWAN 1.0.4 MAC protocol simulation

- [ ] S21.1 — `src/iot/lorawan.rs`: LoRaWAN module
- [ ] S21.2 — `LoRaConfig` struct: dev_eui, app_eui, app_key (128-bit), frequency_plan (EU868/US915/AU915)
- [ ] S21.3 — `FrequencyPlan` enum with channel tables, data rates, max TX power
- [ ] S21.4 — `LoRaDevice` struct: state machine (Idle → Joining → Joined → Transmitting → Receiving)
- [ ] S21.5 — OTAA join: `join_request() -> Result<JoinAccept>` simulation
- [ ] S21.6 — `send_uplink(port, payload, confirmed) -> Result<()>`: class A uplink
- [ ] S21.7 — `receive_downlink() -> Option<Downlink>`: RX1/RX2 window simulation
- [ ] S21.8 — Frame counter tracking: FCntUp, FCntDown — reject replayed frames
- [ ] S21.9 — Adaptive Data Rate (ADR): adjust spreading factor based on link margin
- [ ] S21.10 — 10 tests: config validation, join sequence, uplink/downlink, frame counters, ADR

#### Sprint 22: LoRaWAN Class B/C & Integration `P2`

**Goal:** Class B (beacon) and Class C (continuous RX) modes, integration with IoT module

- [ ] S22.1 — Class B: `enable_class_b(ping_slot_period)` — periodic receive windows
- [ ] S22.2 — Beacon synchronization: `BeaconInfo` struct with GPS time, gateway coordinates
- [ ] S22.3 — Class C: `enable_class_c()` — continuous receive except during TX
- [ ] S22.4 — `LoRaEvent` enum: Joined, UplinkSent, DownlinkReceived, BeaconReceived, LinkCheckOk
- [ ] S22.5 — Multicast: `join_multicast_group(addr, nwk_skey, app_skey)`
- [ ] S22.6 — MAC commands: LinkCheck, DeviceTime, NewChannel, RxParamSetup
- [ ] S22.7 — Duty cycle enforcement: track TX airtime, respect regional limits
- [ ] S22.8 — Integration: `IotStack` struct combining WiFi + BLE + MQTT + LoRaWAN
- [ ] S22.9 — `examples/lorawan_sensor.fj`: sensor node with LoRaWAN uplink every 10 minutes
- [ ] S22.10 — 10 tests: class B/C modes, beacon sync, multicast, MAC commands, duty cycle

### Phase 7: Production Polish & Release `P2`

#### Sprint 23: Error Recovery & Diagnostics `P2`

**Goal:** Better error recovery in parser and analyzer for IDE experience

- [ ] S23.1 — Parser error recovery: skip to next statement on syntax error, continue parsing
- [ ] S23.2 — `RecoveryStrategy` enum: SkipToSemicolon, SkipToBrace, SkipToKeyword
- [ ] S23.3 — Partial AST: return partially-parsed AST even with errors (for LSP)
- [ ] S23.4 — Analyzer error recovery: continue type checking after first error
- [ ] S23.5 — Error cascading prevention: suppress errors caused by earlier errors
- [ ] S23.6 — "Did you mean?" suggestions: Levenshtein distance for misspelled identifiers
- [ ] S23.7 — Missing semicolon detection: "expected `;` after expression statement"
- [ ] S23.8 — Unclosed delimiter detection: "unclosed `{` opened at line 42"
- [ ] S23.9 — Type mismatch context: "expected `i32` because of return type annotation"
- [ ] S23.10 — 10 tests: recovery strategies, partial AST, cascading prevention, suggestions

#### Sprint 24: Formatter Improvements `P2`

**Goal:** Production-quality code formatter

- [ ] S24.1 — Configurable line width: `fj fmt --max-width 100` (default 80)
- [ ] S24.2 — Trailing comma policy: Always, Never, Consistent (match existing)
- [ ] S24.3 — Brace style: SameLine (K&R), NextLine (Allman)
- [ ] S24.4 — Import sorting: alphabetical, grouped by source (std, external, local)
- [ ] S24.5 — Comment preservation: don't move or reformat comments
- [ ] S24.6 — Expression wrapping: break long expressions at operators
- [ ] S24.7 — Function signature wrapping: one parameter per line if exceeds width
- [ ] S24.8 — `fj.toml` formatter config section: `[fmt] max_width = 100`
- [ ] S24.9 — `fj fmt --check` returns non-zero if formatting needed (CI gate)
- [ ] S24.10 — 10 tests: line width, trailing comma, brace style, import sort, wrapping

#### Sprint 25: Documentation Generator `P2`

**Goal:** Generate API documentation from doc comments

- [ ] S25.1 — `fj doc` CLI command: generate HTML documentation from .fj source files
- [ ] S25.2 — Parse `///` doc comments on functions, structs, enums, traits
- [ ] S25.3 — Markdown rendering in doc comments (headers, code blocks, lists)
- [ ] S25.4 — Cross-reference links: `[`OtherType`]` links to that type's documentation
- [ ] S25.5 — Module hierarchy: navigation sidebar with module tree
- [ ] S25.6 — Search: fuzzy search across all documented items
- [ ] S25.7 — Example extraction: code blocks in doc comments become runnable examples
- [ ] S25.8 — `fj doc --open`: generate and open in browser
- [ ] S25.9 — JSON output: `fj doc --format json` for programmatic consumption
- [ ] S25.10 — 10 tests: comment parsing, markdown rendering, cross-refs, search, JSON output

#### Sprint 26: Profiler Integration `P2`

**Goal:** Built-in profiler for performance optimization

- [ ] S26.1 — `src/runtime/profiler.rs`: profiler module
- [ ] S26.2 — `Profiler` struct: function_times HashMap<String, Vec<Duration>>, call_counts, memory_snapshots
- [ ] S26.3 — `@profile` annotation: instrument function for timing
- [ ] S26.4 — `fj run --profile program.fj`: execute with profiling enabled
- [ ] S26.5 — Flame graph output: `fj profile --flamegraph output.svg`
- [ ] S26.6 — Hot function report: top 10 functions by total time, with call counts
- [ ] S26.7 — Memory profiler: track allocations, peak memory, allocation hotspots
- [ ] S26.8 — `ProfileReport` struct: to_text(), to_json(), to_flamegraph_data()
- [ ] S26.9 — Sampling profiler: periodic stack trace capture (simulation)
- [ ] S26.10 — 10 tests: function timing, call counting, memory tracking, report formats, flamegraph

#### Sprint 27: Cross-Platform Testing `P2`

**Goal:** Ensure Fajar Lang works on macOS, Windows, Linux, and QEMU targets

- [ ] S27.1 — Platform abstraction: `Platform` enum (Linux, MacOS, Windows, Embedded)
- [ ] S27.2 — Path handling: normalize path separators for each platform
- [ ] S27.3 — QEMU test matrix: Cortex-M, AArch64, RISC-V targets
- [ ] S27.4 — CI workflow: GitHub Actions matrix for Linux + macOS + Windows
- [ ] S27.5 — Platform-specific tests: skip hardware tests on CI without boards
- [ ] S27.6 — Endianness handling: ensure tensor serialization works on big-endian
- [ ] S27.7 — 32-bit support: verify codegen works for 32-bit targets (i32 pointers)
- [ ] S27.8 — Unicode path support: file paths with non-ASCII characters
- [ ] S27.9 — `PlatformConfig` struct: features available per platform
- [ ] S27.10 — 10 tests: platform detection, path normalization, QEMU commands, endianness, unicode

#### Sprint 28: Release Preparation `P2`

**Goal:** Version bumps, changelog, final testing, documentation

- [ ] S28.1 — Version bump: Cargo.toml to 0.8.0, CLAUDE.md status update
- [ ] S28.2 — CHANGELOG.md: v0.8.0 "Apex" entry with all 7 phases summarized
- [ ] S28.3 — mdBook: new chapters for GPU training, GAT, incremental compilation, debugger
- [ ] S28.4 — `examples/gpu_transformer_train.fj`: GPU-accelerated Transformer training
- [ ] S28.5 — `examples/lending_iterator.fj`: GAT lending iterator patterns
- [ ] S28.6 — `examples/lorawan_gateway.fj`: LoRaWAN gateway with MQTT bridge
- [ ] S28.7 — Benchmark suite: GPU vs CPU, incremental vs full build, pruned vs unpruned model
- [ ] S28.8 — Regression test: full test suite passes (2,550+ baseline, zero failures)
- [ ] S28.9 — Update CLAUDE.md with v0.8 completion status, new test count, LOC
- [ ] S28.10 — 10 tests: version string, changelog entry, examples compile, benchmarks run

---

## Dependencies

```
Phase 1 (GPU Training) ─────────── Requires v0.7 ML runtime (autograd, optimizers)
Phase 2 (GAT) ──────────────────── Independent (type system extension)
Phase 3 (Incremental) ──────────── Independent (compiler infrastructure)
Phase 4 (Model Optimization) ───── Requires Phase 1 (GPU training loop for fine-tuning)
Phase 4 (Custom Autodiff) ──────── Requires v0.7 autograd tape
Phase 5 (DAP Debugger) ─────────── Independent (new subsystem)
Phase 6 (LoRaWAN) ──────────────── Requires v0.7 IoT module (src/iot/)
Phase 7 (Polish) ───────────────── Independent (quality improvements)
```

**Critical path:** Phase 1 → Phase 4 S13-S16 (pruning/distillation need GPU training)

**Parallel tracks:**
- Track A: Phase 1 (GPU) → Phase 4 (Model Optimization)
- Track B: Phase 2 (GAT, independent type system)
- Track C: Phase 3 (Incremental compilation, independent)
- Track D: Phase 5 (DAP Debugger, independent)
- Track E: Phase 6 (LoRaWAN, independent)
- Track F: Phase 7 (Polish, independent)

---

## Success Criteria

- [ ] GPU matmul 10x faster than CPU for 1024x1024 matrices (simulation benchmark)
- [ ] `async fn` works in traits with proper lifetime handling
- [ ] Lending iterator borrows from container without clone
- [ ] Incremental rebuild < 100ms for single-file change in 100-file project (simulation)
- [ ] Structured pruning reduces model parameters by 50% with < 2% accuracy loss (simulation)
- [ ] Knowledge distillation produces 10x smaller model with < 5% accuracy loss (simulation)
- [ ] Custom autodiff passes numerical gradient check for all registered ops
- [ ] DAP debugger sets breakpoints, steps through code, inspects variables
- [ ] LoRaWAN OTAA join succeeds, uplink/downlink exchange works (simulation)
- [ ] `fj doc` generates navigable HTML documentation from doc comments
- [ ] `fj run --profile` produces flame graph data
- [ ] All existing 2,550 tests pass (zero regressions), 0 clippy warnings

---

## Stats Targets

| Metric | v0.7 (current) | v0.8 (target) |
|--------|----------------|---------------|
| Tests | 2,550 | 5,500+ |
| LOC | ~130,000 | ~180,000 |
| Examples | 40 | 50+ |
| Error codes | 105+ | 120+ |
| Codegen backends | 3 (Cranelift + LLVM + Wasm) | 3 (+ GPU kernel emission) |
| ML layers | Transformer, LSTM, GRU, Dense, Conv2d, etc. | + Pruned, Distilled variants |
| IoT protocols | WiFi, BLE, MQTT | + LoRaWAN |
| Debug support | printf-style | + DAP (VS Code step debugger) |
| Compilation | Full rebuild only | + Incremental with artifact cache |
| Type system | Generics, traits, Polonius | + GAT, async traits, lending iterators |

---

## Non-Goals (Deferred to v0.9+)

- Full self-hosted compiler — self-hosting lexer/parser done in v0.3, full compiler deferred
- Thread-safe garbage collector — ownership model is the Fajar way
- WebGPU compute shaders — focus on CUDA for training, WebGPU deferred
- Algebraic effects — interesting but niche, defer to future release
- Dependent types — compile-time shape checking deferred
- Hot code reloading — complex runtime support, defer
- Language server for remote development — SSH/container debugging deferred
- Multi-target linking — single binary for multiple architectures deferred

---

*V08_PLAN.md v1.0 | Created 2026-03-11 | 7 Phases, 28 Sprints, 280 Tasks*
