# CHANGELOG

> Version History & Release Notes — Fajar Lang

Semua perubahan penting pada Fajar Lang didokumentasikan di file ini.

Format mengikuti [Keep a Changelog](https://keepachangelog.com/). Versioning mengikuti [Semantic Versioning](https://semver.org/).

```
Kategori perubahan:
  Added      — fitur baru
  Changed    — perubahan fitur existing
  Deprecated — fitur yang akan dihapus
  Removed    — fitur yang dihapus
  Fixed      — bug fix
  Security   — vulnerability fix
```

---

## [3.1.0] — 2026-03-19 "Surya Enablers"

### Added
- **String literals in @kernel** — `println("text")` compiles to `.rodata` section in bare-metal. No heap allocation. Eliminates 200+ `putc()` calls in FajarOS.
- **90+ bare-metal runtime functions** — GPIO (8), UART (7), SPI (3), I2C (4), Timer (9), DMA (7), VFS (6), Network (11), Display (8), Process (10), Syscall (8), MMU (3)
- **`svc()` builtin** — user-mode syscall via assembly stub (`svc #0; ret`)
- **`tlbi_va()` builtin** — per-VA TLB flush for MMU management
- **`switch_ttbr0()` builtin** — TTBR0 switch + TLB flush for per-process page tables
- **`MSR SP_EL0` stub** — set user stack pointer for EL0 processes
- **EL0 vector stubs** — `__exc_sync_lower` and `__exc_irq_lower` with context switch support
- **36 native codegen tests** — GPIO, UART, SPI, I2C, Timer, DMA, Storage, Network, Display, Process, kernel boot, labeled break/continue (3), const eval (3), @kernel enforcement (4), plus 15 interpreter/unit tests
- **3 new examples** — `hal_blinky.fj`, `fajaros_kernel.fj`, `fajaros_shell.fj`
- **Labeled break/continue** — `'outer: while/for/loop { break 'outer }` syntax. Parser, interpreter, and Cranelift codegen support. Exits/continues outer loops from nested loops.
- **Compile-time constant folding** — `try_const_eval()` evaluates `const X: i64 = 4096 * 16` at compile time. Supports arithmetic, bitwise, unary, power, and chained const references.
- **@kernel codegen enforcement** — `is_kernel_forbidden_builtin()` blocks tensor ops, file I/O, heap-allocating builtins in @kernel functions at codegen time. Error code CE011.
- **`labeled_loops` in CodegenCtx** — `HashMap<String, (Block, Block)>` tracks label → (header, exit) for Cranelift codegen of labeled break/continue.
- **`const_values` in CodegenCtx** — `HashMap<String, i64>` stores compile-time constant values for chained const folding.
- **`COMPILER_ENHANCEMENT_PLAN.md`** — 5 sprints, 48 tasks. Sprints 4-5 complete.
- **`NEXT_STEPS_PLAN.md`** — EL0, labeled break, v3.1 release plan

### Fixed
- **`return` in bare-metal** — void return now emits `return_(&[iconst(0)])` matching i64 signature
- **Sequential SVC calls** — advance ELR_EL1 directly via `asm!("mrs/add/msr")` instead of writing to saved frame
- **SPSR_EL1 save/restore** — exception frames expanded from 256 to 272 bytes
- **`print()` in bare-metal** — registered `__print_str` → `fj_rt_bare_print` in AOT
- **`println()` newline** — added `fj_rt_bare_println` (print + `\n`) assembly stub
- **WXN cleared in MMU enable** — prevents writable+executable conflict
- **TLB flush before MMU enable** — clears stale firmware entries
- **CI cross-compile** — suppressed `execute_graph` dead_code warning on aarch64

### Changed
- **`nostd.rs`**: `allow_strings=true` for bare-metal config (strings → `.rodata`)
- **`is_io_builtin()`**: removed println/print from IO blocklist (file I/O still blocked)
- **Exception frame**: 256→272 bytes (added SPSR_EL1 at offset 256)
- **`mmu_enable` stub**: added TLB flush + WXN clear before enabling

### Refactored
- **`parser/mod.rs`** (4,931 LOC) → `mod.rs` (2,480) + `items.rs` (893) + `expr.rs` (1,571)
- **`eval.rs`** (8,603 LOC) → `eval/mod.rs` (2,457) + `builtins.rs` (5,019) + `methods.rs` (1,181)
- **`type_check.rs`** (7,524 LOC) → `type_check/mod.rs` (4,123) + `register.rs` (1,551) + `check.rs` (1,884)
- Total: 4 monolithic files (27,370 LOC) → 12 focused modules

### Verified on Hardware
- **Radxa Dragon Q6A** (QCS6490): JIT fib(40) in 0.65s (1,246× speedup)
- **GPIO96**: real hardware toggle on Q6A
- **QNN CPU**: MNIST inference in 24ms (42 inf/sec)
- **QNN GPU**: MNIST inference in 262ms (Adreno 643 via OpenCL 3.0)
- **FajarOS EL0**: user process at unprivileged level with MMU protection

---

## [2.0.0-dawn] — 2026-03-16 "Dawn" (Q6A Hardware Deployment)

### Added
- **Dragon Q6A BSP**: Full board support for Radxa Dragon Q6A (QCS6490 edge AI SBC)
- **Cross-compilation**: `cargo build --release --target aarch64-unknown-linux-gnu` → 6.8MB binary
- **GPU builtins**: `gpu_available()`, `gpu_info()`, `gpu_matmul()`, `gpu_add()`, `gpu_relu()`, `gpu_sigmoid()` — OpenCL Adreno 635 + CPU fallback
- **NPU builtins**: `qnn_version()`, `npu_info()`, `qnn_quantize()`, `qnn_dequantize()` — Hexagon 770 via QNN SDK v2.40
- **Edge AI builtins**: `cpu_temp()`, `cpu_freq()`, `mem_usage()`, `sys_uptime()`, `log_to_file()`, `process_id()`, `sleep_ms()`
- **Watchdog**: `watchdog_start()`, `watchdog_kick()`, `watchdog_stop()` — software watchdog timer
- **Cache**: `cache_set()`, `cache_get()`, `cache_clear()` — inference result caching
- **File utilities**: `file_size()`, `dir_list()`, `env_var()`
- **15 Q6A examples**: blinky, button_led, uart_echo/gps, i2c_sensor, spi_display, pwm_servo, npu_classify/detect, system_monitor, stress_test, edge_deploy, smart_doorbell, plant_monitor, anomaly_detect, ai_server
- **Production tooling**: systemd service, monitoring script, deployment guide, quickstart guide, pinout reference
- **Tests**: 5,376 total (0 failures), verified on real Q6A hardware

### Performance (Q6A)
- Cold start → first inference: **4ms**
- JIT speedup: **128x** vs interpreted (fib30)
- Cranelift JIT: works on ARM64
- GPU detection: Adreno 635, OpenCL 3.0
- NPU: Hexagon 770, 12 TOPS INT8

---

## [3.0.0] — 2026-03-12 "Singularity"

### Added
- **Phase 1 — Higher-Kinded Types** (`src/hkt/`): `TypeConstructor` with kind system (`Kind::Star`, `Kind::Arrow`), `HktApplication` with kind checking, `Functor`/`Monad`/`Applicative` trait encoding, `MonadTransformer` stack composition, `TypeLambda` with beta reduction, `TypeFamilyDef` with closed/open families and overlap checking
- **Phase 2 — Structured Concurrency** (`src/concurrency_v2/`): `TaskScope` with structured spawning and join-all semantics, `Nursery` pattern (child tasks cancelled on parent exit), `CancellationToken` cooperative cancellation, `StructuredChannel` with scope-bound lifecycle, `FlowControl` (backpressure, rate limiting, windowing, batching), `ConcurrencyLimiter` with `Semaphore`-based slot control
- **Phase 3 — Distributed Computing** (`src/distributed/`): `ActorSystem` with `ActorRef` message passing, `Supervisor` (one-for-one/all-for-one/rest-for-one), `ConsensusProtocol` with Raft (leader election, log replication, heartbeat), `DistributedKV` with consistent hashing and virtual nodes, `CrdtCounter`/`CrdtGCounter`/`CrdtLwwRegister`/`CrdtOrSet` with `CrdtMerge` trait, `RemoteActor` with `RpcCall`/`RpcResponse` serialization
- **Phase 4 — Advanced ML v2** (`src/ml_advanced/`): `TransformerBlock` with multi-head self-attention, `InferenceEngine` with KV-cache and batched inference, `DiffusionModel` with `NoiseSchedule` (linear/cosine/sigmoid) and forward/reverse process, `DdpmSampler`/`DdimSampler`, `RlEnvironment` with `RlAgent` trait, `PolicyGradient` REINFORCE, `DqnAgent` with replay buffer and epsilon-greedy, `ModelServer` with request batching and health monitoring
- **Phase 5 — Native GPU Codegen** (`src/gpu_codegen/`): `PtxModule` with PTX assembly emission (registers, types, thread indexing, shared memory, atomics), `SpirVModule` with SPIR-V word emission (capabilities, entry points, SSBOs, barriers), `FusionGraph` for kernel fusion (elementwise chains, reduction chains, memory planning, tile tuning), `DeviceAllocator` with best-fit free-list, `TransferDesc` for H2D/D2H/D2D, fragmentation analysis, `GpuTopology` for multi-GPU
- **Phase 6 — Package Ecosystem v2** (`src/package_v2/`): `Workspace` with shared dependencies and topological build ordering, `BuildScript` with directive parsing and native library detection, `CfgPredicate` (All/Any/Not/KeyValue/Flag) with `CfgContext` evaluation, `FeatureSet` with transitive resolution, `TargetTriple` parsing with bare-metal detection, `BuildMatrix` generation, `QemuRunner`, `SupportTier` classification
- **Phase 7 — Debugger v2** (`src/debugger_v2/`): `EventLog` execution recording with `DeltaPatch` compression, `RingBuffer` for size-limited recording, `RecordFilter` for selective capture, `ReplaySession` with forward/reverse stepping and continue, `Watchpoint` with `WatchCondition`, `RootCauseTrace`, `HeapMap` with fragmentation analysis, `RefGraph` cycle detection, `LeakReport`, `CpuProfile` with flame graph generation, `generate_hints()` for PGO suggestions
- **Phase 8 — Production Deployment** (`src/deployment/`): `DockerConfig` with multi-stage Dockerfile generation (scratch/distroless/alpine), static linking (musl), `ComposeProject` YAML generation, `HealthReport` with component checks, `K8sDeployment` manifest generation, `HelmChart`, structured `Logger` with JSON output, `MetricsRegistry` (counter/gauge/histogram) with Prometheus exposition, `Span` distributed tracing (W3C traceparent), `AlertRule` evaluation, `ShutdownController` (phased hooks), `HotReloadConfig`, `FlagRegistry` with rollout%, `ConnectionDrainer`, `Supervisor` with exponential backoff, `RollingUpdate`, `MemoryLimiter`, `ThreadPoolConfig` adaptive scaling, `TlsConfig`, JWT validation, `RateLimiter` (token bucket), `CorsConfig`, `SecretStore`, `AuditLog`, input sanitization (XSS/SQL/command/path traversal), `audit_dependencies` CVE scanning

### New Modules
- `src/hkt/` — 4 files (constructors.rs, traits.rs, lambdas.rs, families.rs)
- `src/concurrency_v2/` — 4 files (scope.rs, nursery.rs, flow.rs, limiter.rs)
- `src/distributed/` — 4 files (actors.rs, consensus.rs, kv_store.rs, crdt.rs)
- `src/ml_advanced/` — 4 files (transformer.rs, diffusion.rs, reinforcement.rs, serving.rs)
- `src/gpu_codegen/` — 4 files (ptx.rs, spirv.rs, fusion.rs, gpu_memory.rs)
- `src/package_v2/` — 4 files (workspaces.rs, build_scripts.rs, conditional.rs, cross_compile.rs)
- `src/debugger_v2/` — 4 files (recording.rs, replay.rs, memory_viz.rs, profiling.rs)
- `src/deployment/` — 4 files (containers.rs, observability.rs, runtime_mgmt.rs, security.rs)

### Stats
- New files: 32 source modules across 8 phases
- New tests: ~540
- Sprints: 32 (8 phases × 4 sprints)
- Total: 320 tasks, all complete
- Total tests: 5,144 (0 failures)
- Total LOC: ~230,000 Rust

---

## [2.0.0] — 2026-03-12 "Transcendence"

### Added
- **Phase 1 — Dependent Types** (`src/dependent/`): Type-level natural numbers (`Nat`, `Zero`, `Succ`), `NatConstraint` arithmetic, `DependentArray<T, N>` with compile-time bounds checking, `DependentTensor<N, M>` with shape verification (matmul `Tensor<A,B> * Tensor<B,C> -> Tensor<A,C>`), reshape proof (`A*B == C*D`), dependent pattern matching, `DependentTypeChecker`, exhaustiveness for nat patterns, `DependentCodegen` for type erasure
- **Phase 2 — Linear Types** (`src/linear/`): `LinearChecker` with exactly-once usage enforcement, `LinearType`/`AffineType` distinction, `UseTracker` for tracking use/move/drop, `LinearResource` (FileHandle, GpioPin, DmaBuffer, GpuBuffer, Mutex), `BorrowingProtocol` for temporary non-consuming access, `HardwareHandle` with `must_use` enforcement, `PinProtocol` for GPIO configure-once semantics, linear error codes (LN001-LN008)
- **Phase 3 — Formal Verification** (`src/verification/`): `ContractParser` with `requires`/`ensures`/`invariant` annotations, `RuntimeVerifier` for dynamic contract checking, `SmtContext` with Z3/CVC5 solver abstraction, expression encoding to SMT-LIB (QF_BV integer theory, array theory), `VerifiedFunction` pipeline, automatic bounds/overflow/null safety proofs, `@verified` annotation, loop termination hints (`decreases`), `@kernel @verified` composition for page table bounds, stack depth, IRQ latency verification
- **Phase 4 — Tiered JIT** (`src/jit/`): Per-function `ExecutionCounter` with hot threshold (default: 100), `BaselineJit` for fast compilation (<1ms), `OptimizingJit` with inlining/constant propagation/dead code elimination, `ProfileCollector` with branch/type profiles, tier promotion (Interpreter→Baseline→Optimizing), `OsrPoint` for on-stack replacement at loop headers with state transfer, `DeoptInfo` for optimistic optimization bailout, `JitCache` keyed by (function_hash, opt_level)
- **Phase 5 — Effect System v2** (`src/effects/`): `EffectHandler` with resume/abort/transform, `EffectInferenceEngine` with bottom-up inference and fixed-point iteration, `EffectPolyVar` for effect polymorphism with unification, `AsyncEffectMapping` from effects to context annotations, `LinearEffectCheck` for linear type interaction, `EffectErasure` for zero-overhead codegen, `EffectDocEntry` for LSP/IDE, stdlib effect annotations
- **Phase 6 — GC Mode** (`src/gc/`): `RcType` with strong/weak counts and `CycleCollector` (DFS cycle detection), `GcHeap` with tri-color mark-sweep, generational collection (young/old), write barriers, finalization, `MemoryMode` (Owned/RefCounted/Tracing) selectable via `--gc` flag, `@kernel` prohibition, cross-module GC compatibility, `GcBenchmarks` with throughput/latency/pause metrics
- **Phase 7 — Self-Hosting v2** (`src/selfhost/`): `FjType`-based type checker with scope resolution, Hindley-Milner unification, borrow analysis in .fj, `IrBuilder` for Cranelift IR generation from .fj, `BootstrapChain` (Stage0→Stage1→Stage2), `BinaryComparison` for byte-for-byte verification, `SourceProvenance` with deterministic FNV-1a hashing, `CrossPlatformBuild` reproducibility verification, `BuildCache` content-addressable
- **Phase 8 — Language Server v2** (`src/lsp_v2/`): `TraitImplIndex` with incremental updates, `goto_implementation`, blanket impl display, `UnsatisfiedBound` diagnostics, `AssocTypeBinding` resolution, `TraitObjectInfo` with vtable layout, `ImplSuggestion` skeleton generation, `OrphanViolation` checking, declarative `MacroDefinition` with `MacroArm` pattern/template, `MacroExpander` with hygiene (`HygieneContext`), recursive expansion (limit 128), `MacroSourceMap` for error location mapping, `ExpectedType` analysis, `SynthesizedExpr` for type-driven completion, `FillSuggestion`, `PatternSuggestion` for exhaustive match, `ImportSuggestion`, `PostfixCompletion` (.if/.match/.let/.dbg), `SnippetTemplate`, `RankedCompletion` with multi-factor scoring, `ExtractFunction`/`ExtractVariable`/`InlineFunction`/`InlineVariable` refactorings, `RenameSymbol` across workspace, `MoveModule` with import updates, `ExtractTrait`, `ChangeSignature`, `ConvertFunctionMethod`

### New Modules
- `src/dependent/` — 4 files (nat.rs, arrays.rs, tensor.rs, patterns.rs)
- `src/linear/` — 4 files (checker.rs, resources.rs, borrowing.rs, hardware.rs)
- `src/verification/` — 4 files (contracts.rs, smt.rs, verified.rs, kernel_verify.rs)
- `src/jit/` — 4 files (counter.rs, baseline.rs, optimizing.rs, osr.rs)
- `src/effects/` — 4 files (handlers.rs, inference.rs, polymorphism.rs, interop.rs)
- `src/gc/` — 4 files (refcount.rs, tracing.rs, integration.rs, benchmarks.rs)
- `src/selfhost/` — 4 files (analyzer_fj.rs, codegen_fj.rs, bootstrap.rs, reproducible.rs)
- `src/lsp_v2/` — 4 files (traits.rs, macros.rs, completion.rs, refactoring.rs)

### Stats
- New files: 32 source modules across 8 phases
- New tests: ~700 (320 tasks)
- Sprints: 32 (8 phases × 4 sprints)
- Total tests: 4,626 (0 failures)
- Total LOC: ~210,000 Rust

---

## [1.0.0] — 2026-03-11 "Genesis"

### Added
- **Phase 1 — Stability & Conformance Testing** (`src/testing/stability.rs`): `FuzzHarness` with `FuzzTarget` (Lexer/Parser/Analyzer/Interpreter/Formatter/Vm), `FuzzConfig`, grammar-aware input generation (`GrammarGen`), `CorpusManager` with delta-debugging minimization, `ConformanceRunner` with `// expect:`/`// expect-error:` annotation parsing, `ConformanceCategory` (8 categories), `RegressionHarness` with snapshot management (`Snapshot`/`SnapshotManager`), `BaselineRecorder`/`BaselineComparator` for performance regression detection, `BisectHelper` for commit bisection, `ErrorPolisher` with `ErrorQuality` scoring (0-100), `ErrorCatalog` (78 error codes across 9 categories), `ErrorAudit` with quality bar checking
- **Phase 2 — Performance Engineering** (`src/compiler/performance.rs`): `StringInterner` with `Symbol` indices and `SyncInterner` (thread-safe), `InlineCache` (monomorphic/polymorphic/megamorphic), `DispatchTable` for fast binary operation dispatch via pre-computed 2D type-tag table, `TailCallOptimizer` detecting self-recursive tail calls and transforming to loops, `ConstFolder` for compile-time expression evaluation (arithmetic, comparison, string concat), `CompilationTimer` with per-phase timing breakdown and `--timings` output format, `ValueOptimizer` with `SmallString` (22-byte SSO) and `CompactValue` (16-byte tagged union)
- **Phase 3 — Cross-Platform & Distribution** (`src/runtime/crossplatform.rs`): `PlatformDetector` with runtime OS/arch/CPU feature detection, `PathNormalizer` with `to_uri`/`from_uri` (Windows drive letter handling), `LineEndingHandler` (LF/CRLF/CR detection and normalization), `BinaryDistributor` with `DistProfile` (LTO=fat, strip, codegen-units=1), `InstallerGenerator` (shell/PowerShell/Homebrew/Debian/completions for bash/zsh/fish/PowerShell), `VersionInfo` (short/long/JSON formats), `PlatformOptimizer` (I/O backend selection, SIMD width, thread pool, memory config)
- **Phase 4 — Language Server Completion** (`src/lsp/advanced.rs`): `SymbolIndex` with cross-scope resolution (fn/let/const/struct/enum/trait/impl), `ReferencesFinder` with read/write/definition/import classification, `CodeActionProvider` (7 actions: make mutable, add type annotation, extract function, inline variable, convert if/else to match, add missing import, add missing fields), `SemanticTokenizer` (16 token types, 8 modifier flags, context annotation highlighting), `SignatureHelper` with 16+ built-in signatures and active parameter tracking, `CallHierarchyProvider` with incoming/outgoing call graph
- **Phase 5 — Documentation & Learning** (`src/package/documentation.rs`): `ReferenceGenerator` with cross-reference resolution and HTML generation, `TutorialBuilder` (10 progressive tutorials, exercises, prev/next navigation), `DocEnhancer` with `SearchIndex` (JSON), `DocTheme` (dark/light/auto CSS), `BreadcrumbTrail`, `DeprecationBanner`, `PlaygroundCompiler` with `PlaygroundSandbox`, `ShareEncoder` (base64 URL), `ExampleLibrary`, `DocValidator` with coverage reporting, `SiteGenerator` with sidebar navigation and version selector
- **Phase 6 — Ecosystem & Interop** (`src/codegen/interop.rs`): `CBindgen` (C/C++ header generation with include guards, `extern "C"`, stdint.h types, packed structs, variadic functions), `PyBindgen` (Python `__init__.py` + `.pyi` type stubs, NumPy ndarray tensor bridge, enum.Enum wrappers), `WasmComponent` (WIT interface/world generation, record/variant/resource types), `PackageAuditor` (vulnerability scanning, license compliance, `SbomGenerator` CycloneDX JSON, `YankManager`), `InteropTypeMapper` (5 target languages × 19 Fajar types)
- **Phase 7 — Release Engineering** (`src/compiler/release.rs`): `ReleasePipeline` (6 stages: Test→Build→Sign→Publish→Verify→Announce), `BinarySizeOptimizer` (section/function/crate size analysis, optimization suggestions, debug/release/dist profiles), `StabilityChecker` (API snapshot/diff, breaking change detection, SemVer validation), `ChangelogGenerator` (conventional commit parsing, 7 categories, migration guide generation), `QualityGateRunner` (8 quality checks: tests, clippy, fmt, coverage, benchmarks, binary size), `ReleaseNotes` (GitHub release, blog post, tweet generators)

### Stats
- New files: 7 source modules (`testing/stability.rs`, `compiler/performance.rs`, `runtime/crossplatform.rs`, `lsp/advanced.rs`, `package/documentation.rs`, `codegen/interop.rs`, `compiler/release.rs`)
- New tests: 280 (40 per module)
- Sprints: 28 (7 phases × 4 sprints)
- Total: 280 tasks, all complete

---

## [1.1.0] — 2026-03-12 "Ascension"

### Added
- **Phase 1-2 — Hardware Detection**: CPU feature detection (SSE/AVX/NEON/SVE/RVV via CPUID/MRS), GPU discovery (CUDA Driver API), NPU detection (Intel VPU, AMD XDNA, Qualcomm Hexagon, Apple ANE), accelerator registry with ranking and fallback chains
- **Phase 3-4 — Modern Numeric Formats & BSP**: FP8 (E4M3/E5M2), FP4, BF16, structured sparsity (2:4, 4:8), Jetson Thor BSP (Blackwell GPU, MIG partitioning, power management)
- **Phase 5-6 — Advanced SIMD & CI/CD**: AVX-512, AMX (TMUL), AVX10.2+APX, Blackwell PTX emitters, CI/CD pipeline, binary distribution, installer generation
- **Phase 7-8 — Package Registry & Playground**: Package registry with search/download/version resolution, online playground with Wasm sandbox and share encoding
- **Phase 9-10 — Multi-Accelerator & Demos**: CPU/GPU/NPU/FPGA dispatch with cost model and profiling, real-world demos (drone controller, medical imaging, autonomous vehicle, smart factory)

### Stats
- New files: 70 source modules across 10 phases
- New tests: ~593
- Sprints: 40 (10 phases × 4 sprints)
- Total: 400 tasks, all complete
- Total tests: ~3,985 (0 failures)
- Total LOC: ~220,000 Rust

---

## [0.9.0] — 2026-03-11 "Convergence"

### Added
- **Phase 1 — Effect System** (`src/analyzer/effects.rs`): Algebraic effects with `EffectKind` (IO, Alloc, Panic, Async, State, Exception), `EffectDecl`/`EffectOp` declarations, `EffectSet` for function signatures, `EffectRegistry` with built-in effects, `EffectHandler` with `OpHandler` and `ResumePoint` for delimited continuations, `HandlerScopeStack` for nested handler resolution, effect checking (EE001-EE008 error codes), context interaction (@kernel/no-Alloc, @device/no-IO), `#[pure]` annotation checking, effect-aware closures/generics/trait methods, cross-module effect inference, `no_effect` bound
- **Phase 2 — Compile-Time Evaluation** (`src/compiler/comptime.rs`): `ConstEval` evaluator with `ConstValue` (Int/Float/Bool/Str/Array/Tuple/Struct/Enum/FnPtr/Null), `ConstExpr` AST, const arithmetic/comparison/logical/control-flow, `ComptimeBlock` interpreter with string manipulation and array generation, `@comptime` parameter annotation, compile-time assertions, `Shape` type with `ShapeDim` (Const/Dynamic), matmul/broadcast/conv2d/reshape shape validation (TE009), layer chain inference, `ConstCache` memoization, const recursion limit (128), const loops, const struct/enum/function pointer construction, const eval metrics
- **Phase 3 — Macro System** (`src/parser/macros.rs`): Declarative macros with `MacroRule`/`MacroDef`/`MacroMatcher` (Literal/Variable/Repetition), `FragmentKind` (Expr/Ident/Ty/Stmt/Block/Pat/Literal/TokenTree), `RepKind` (ZeroOrMore/OneOrMore), `MacroExpander` with hygiene (gensym), recursive expansion (limit 64), built-in macros (vec!/println!/format!/assert!/cfg!/dbg!), `DeriveMacro` system (Debug/Clone/PartialEq/Hash/Default/Serialize), `AttributeMacro` (Cfg/CfgFeature/Inline/Deprecated/Allow/Deny/Repr), `CfgExpr` evaluator, `compile_error!/include!/env!/file!/line!/column!/stringify!` utilities, macro error reporting with expansion trace
- **Phase 4 — SIMD & Vectorization** (`src/runtime/simd.rs`): `SimdVector` with `SimdType` (I32x4/F32x4/I32x8/F32x8/F64x2/F64x4/I8x16/I16x8), lane-wise arithmetic/comparison/shuffle, `SimdCapability` runtime detection, platform intrinsics (SSE: mm_add_ps/mm_mul_ps/mm_shuffle_ps, AVX: mm256_add_ps/mm256_fmadd_ps, AVX-512: mm512_add_ps/mm512_mask_add_ps, NEON: vaddq_f32/vmulq_f32, SVE: sve_add_f32_pred, RISC-V V: rvv_fadd), SIMD tensor ops (4x4 matmul, elementwise, reduction, relu/sigmoid/tanh/softmax, dot product, conv1d, quantize), `VectorizationPass` auto-vectorizer with loop analysis, cost model, `VectorizationReport`
- **Phase 5 — Security Hardening** (`src/compiler/security.rs`): `SecurityConfig` with comprehensive flags, `CanaryGenerator` per-function stack canaries, `StackClashProtector` with guard page probing, `ShadowStack` for return address protection, stack depth analysis, `CfiMetadata` with type hash validation, forward/backward-edge CFI, `VTableGuard`, `JumpTableGuard`, `FunctionDiversifier` ROP mitigation, `AddressSanitizer` (shadow memory, red zones, quarantine), `MemorySanitizer` (uninitialized memory tracking), `LeakDetector`, double-free detection, `PicGenerator`, `RelroSection`, `NxStackConfig`, `FortifyChecker`, `SecurityAudit` with findings/report, binary hardening score (0-100), `-fharden` flag
- **Phase 6 — Async I/O & Networking** (`src/runtime/async_io.rs`): `IoBackend` trait with `IoUringBackend`/`EpollBackend` simulation, `IoOp` (Read/Write/Accept/Connect/Close), `BufReader`/`BufWriter`, `TcpListener`/`TcpStream`/`UdpSocket` simulation, `SocketAddr` parsing, DNS resolution, `ConnectionPool`, TLS handshake simulation, HTTP/1.1 (`HttpRequest`/`HttpResponse` parse/build, `HttpClient`/`HttpServer` with routing, `HttpMiddleware`, CORS), WebSocket frames (encode/decode, upgrade handshake), gRPC stubs, HTTP/2 framing, `ProtocolStack`, `TokenBucket` rate limiter, `CircuitBreaker`, `RetryPolicy` with exponential backoff
- **Phase 7 — Production Polish** (`src/compiler/edition.rs`, `src/compiler/benchmark.rs`): `Edition` system (Edition2025/Edition2026) with keyword reservation, `DeprecationWarning`, `MigrationTool` with auto-fix suggestions, edition compatibility checking, `StabilityLevel` (Stable/Unstable/Deprecated), `FeatureGate`, `ApiSurface`/`ApiItem`/`ApiDiff` for API stability, `StabilityChecker`, SemVer validation, API diff report generation, `BenchmarkSuite` with `Benchmark`/`BenchmarkResult`, overhead measurement, regression detection

### New Examples
- `examples/effects_demo.fj` — Algebraic effects for I/O, state, exceptions
- `examples/simd_image.fj` — SIMD image processing (blur, sharpen, edge detect)
- `examples/http_server.fj` — Async HTTP server with routing and JSON

### Stats
- New files: 8 source modules, 3 examples
- New tests: 270 (across 8 modules, ~34 per module)
- Sprints: 28 (7 phases × 4 sprints)
- Total: 280 tasks, all complete

---

## [0.8.0] — 2026-03-11 "Apex"

### Added
- **Phase 1 — GPU-Accelerated Training** (`src/runtime/ml/gpu/mod.rs`): CUDA device simulation, GPU tensor kernels (matmul, elementwise, activations, softmax, conv2d, batch norm, transpose, reduce), GPU autograd tape, GpuSGD/GpuAdam optimizers, mixed precision (FP16/FP32), data prefetcher, multi-GPU data parallelism (scatter/gather/all-reduce), GPU benchmarks
- **Phase 2 — Generic Associated Types (GAT)** (`src/analyzer/gat.rs`, `async_trait.rs`, `lending.rs`, `gat_errors.rs`): GAT type system with `AssociatedTypeDef`, `TypeProjection`, `GatRegistry`, async trait desugaring (`async fn` → `impl Future`), object safety checking, `#[async_trait]` boxing, lifetime capture analysis, lending iterator validation (`WindowsIter`, `ChunksIter`, `LinesIter`), GAT error messages (GE001-GE003)
- **Phase 3 — Incremental Compilation** (`src/compiler/incremental/`): File-level dependency graph, SHA-256 content hashing, change detection, transitive dependents, topological sort, cycle detection, artifact cache with pruning, `IncrementalCompiler` pipeline, compilation benchmarks with lazy parsing/analysis
- **Phase 4 — Model Optimization** (`src/runtime/ml/pruning.rs`, `distillation.rs`, `custom_grad.rs`, `compression.rs`): Structured pruning (magnitude/gradient/random, channel/filter), knowledge distillation (soft labels, KL divergence, feature/attention transfer, progressive distillation), custom autodiff (JVP/VJP, custom op registry, numerical gradient check, built-in Swish/Mish/FocalLoss), end-to-end compression pipeline (train → prune → distill → quantize → export)
- **Phase 5 — DAP Debugger** (`src/debugger/dap/`): Source map, breakpoint manager, debug state, DWARF stubs, DAP protocol (Initialize/Launch/SetBreakpoints/Continue/Next/StepIn/StepOut/StackTrace/Variables/Evaluate), watch expressions, conditional breakpoints, hit count breakpoints, log points, exception breakpoints, set-variable, VS Code launch.json configuration, debug console, hover evaluation
- **Phase 6 — LoRaWAN IoT** (`src/iot/lorawan.rs`): LoRaWAN 1.0.4 MAC layer, OTAA join, Class A/B/C modes, 6 frequency plans (EU868/US915/AU915/AS923/IN865/KR920), adaptive data rate, frame counter tracking, beacon synchronization, multicast groups, MAC commands (LinkCheck/DeviceTime/NewChannel/RxParamSetup/DutyCycleReq), duty cycle enforcement, `IotStack` integration (WiFi+BLE+MQTT+LoRaWAN)
- **Phase 7 — Production Polish & Release**:
  - Parser error recovery (`src/parser/recovery.rs`): `RecoveryStrategy`, `ErrorRecovery`, `PartialAst`, `CascadeFilter`, `DidYouMean` (Levenshtein), `MissingSemicolon` detection, `DelimiterTracker`, `TypeMismatchContext`
  - Formatter configuration (`src/formatter/config.rs`): `FormatterConfig`, `TrailingComma`, `BraceStyle`, `ImportSortOrder`, `CommentPreservation`, `ExpressionWrapper`, `SignatureWrapper`, TOML loading
  - Documentation generator (`src/package/docgen.rs`): `DocItem`, `DocModule`, `parse_doc_comments`, `render_markdown`, `CrossReference`, `DocSearch`, `DocGenerator`, HTML/JSON output
  - Runtime profiler (`src/runtime/profiler.rs`): `Profiler`, `FunctionProfile`, `MemorySnapshot`, `FlameGraphEntry`, `AllocationTracker`, `SamplingProfiler`, `ProfileReport` (text/JSON/flamegraph)
  - Cross-platform support (`src/runtime/platform.rs`): `Platform` detection, path normalization, `QemuTarget`, `PlatformConfig`, `EndianOrder` conversion, `PointerWidth`, Unicode path support

### New Examples
- `examples/gpu_mnist_train.fj` — GPU-accelerated MNIST training
- `examples/debug_demo.fj` — Step debugging showcase
- `examples/compression_pipeline.fj` — Model compression pipeline
- `examples/lorawan_sensor.fj` — LoRaWAN sensor node
- `examples/lorawan_gateway.fj` — LoRaWAN gateway with MQTT bridge
- `examples/lending_iterator.fj` — GAT lending iterator patterns

### Stats
- New files: 5 source modules, 2 examples
- New tests: 50 (10 per new source module)
- Sprints: 23-28 (Phase 7, 60 tasks)
- Total phases in v0.8: 7 phases, 28 sprints

---

## [0.7.0] — 2026-03-11 "Zenith"

### Added
- **Phase 1 — Wasm Compilation**: Wasm target backend, wasm32 type mapping, memory model, component model stubs
- **Phase 2 — Wasm Runtime**: WASI preview2 integration, imports/exports, linear memory management
- **Phase 3 — Embedded Wasm**: Wasm-on-MCU runtime, size-optimized binaries, Wasm interpreter for resource-constrained targets
- **Phase 4 — BSP Framework**: Board Support Package abstraction, HalPin/HalSpi/HalI2c/HalUart traits, BSP registry, auto-configuration
- **Phase 5 — Peripheral Drivers**: I2C/SPI/UART/CAN-FD driver framework, DMA integration, interrupt-driven I/O, driver trait hierarchy
- **Phase 6 — RTOS Integration**: Real-time scheduling primitives, priority-based tasks, FreeRTOS/Zephyr FFI stubs, tick-based timing
- **Phase 7 — Power Management**: PowerMode (Run/Sleep/Stop/Standby/Shutdown), WakeSource (Interrupt/RtcAlarm/GpioPin/WakeupTimer), ClockController with peripheral clock gating, VoltageScale (VOS1/VOS2/VOS3), PowerBudget battery life estimation
- **Hardware CI Framework**: HardwareCiConfig, ProbeType (StLink/JLink/Picoprobe/Esptool), TestRunner with flash_and_run simulation, EmbeddedTest harness, QEMU fallback (QemuConfig, detect_physical_board, run_qemu), SerialCapture, BoardMatrix with skip_unavailable, JUnit XML report generation
- **LSP Completion**: CompletionProvider with context-aware candidates (Dot/DoubleColon/Angle/Default triggers), struct field completion, builtin and keyword suggestions
- **LSP Rename**: RenameProvider with find_all_references (whole-word matching) and rename_symbol producing TextEdit operations
- **LSP Inlay Hints**: InlayHintProvider with heuristic type inference for let bindings (i64/f64/str/bool/Array)
- **LSP Workspace Symbols**: WorkspaceSymbolProvider with fuzzy search across fn/struct/enum/trait/const/mod definitions
- **Example**: `rtic_blinky.fj` — RTIC-style interrupt-driven LED blinky with WFI sleep

### Stats
- New files: `src/runtime/os/power.rs`, `src/runtime/os/hardware_ci.rs`, `src/lsp/completion.rs`
- New tests: 40 (10 per sprint across Sprints 25-28)
- New example: `examples/rtic_blinky.fj`
- Sprints: 25-28 (4 sprints, 40 tasks)

---

## [0.6.0] — 2026-03-11 "Horizon"

### Added
- **Phase 1 — LLVM Backend** (`src/codegen/llvm/`): LLVM JIT/AOT compilation via inkwell, full expression/control flow/function support, optimization passes (O0-O3, LTO), `fj run --llvm` CLI integration
- **Phase 2 — DAP Debugger** (`src/debugger/dap/`): Debug Adapter Protocol server with VS Code integration, breakpoints (line/conditional/hit count/log), stepping (in/out/over), variable inspection, stack traces, DWARF debug info
- **Phase 3 — Board Support Packages** (`src/bsp/`): BSP abstraction (`Board` trait), STM32F4/ESP32-S3/nRF52840/RPi4/Jetson Orin Nano targets, HAL traits (GPIO/SPI/I2C/UART/PWM), auto-configuration from `fj.toml`
- **Phase 4 — Package Registry** (`src/package/registry.rs`): Yanking, auth tokens, sparse index, download counting, name collision detection
- **Phase 5 — Lifetime Annotations**: `TokenKind::Lifetime`, elision rules (same as Rust), SE021/ME009/ME010 error codes
- **Phase 6 — RTOS Integration** (`src/runtime/os/rtos.rs`): FreeRTOS FFI, task/queue/mutex/semaphore/timer, realtime annotations
- **Phase 7 — Advanced ML**: LSTM/GRU layers, AdamW optimizer, LR schedulers, DataLoader, mixed precision training
- **Phase 8 — Arduino VENTUNO Q**: STM32H5 BSP, CAN-FD HAL, Zephyr RTOS integration, dual-target build (MCU + MPU)

### Stats
- Sprints: 35 (8 phases)
- Total: 280 tasks, all complete

---

## [0.5.0] — 2026-03-11 "Ascendancy"

### Added
- **Test Framework**: `@test`, `@should_panic`, `@ignore` annotations with `fj test` CLI command
- **Test Runner**: discover, run, filter, and report test results with colored output
- **Doc Comments**: `///` doc comment syntax attached to functions, structs, enums, traits
- **Doc Generation**: `fj doc` command generates HTML documentation with Markdown rendering
- **Doc Tests**: code blocks in `///` comments are extracted and run as tests
- **Trait Objects**: `dyn Trait` with vtable-based dynamic dispatch in interpreter
- **Iterator Protocol**: `.iter()`, `.map()`, `.filter()`, `.take()`, `.enumerate()` lazy combinators
- **Iterator Consumers**: `.collect()`, `.sum()`, `.count()`, `.fold()`, `.next()`
- **String Interpolation**: `f"Hello {name}, age {age}"` with expression evaluation
- **Error Recovery**: parser continues after errors, synchronizes on `;`, `}`, keywords
- **Suggestion Engine**: "did you mean X?" for misspelled identifiers (Levenshtein distance)
- **Type Mismatch Hints**: SE004 now suggests `as` casts and conversion functions
- **Unused Import Warnings**: SE019 for `use` declarations that are never referenced
- **Unreachable Pattern Warnings**: SE020 for match arms after wildcard `_` catch-all
- **`fj watch`**: poll-based file watching with `--test` auto-run mode
- **`fj bench`**: micro-benchmark framework (discovers parameterless functions, 10 iterations)
- **REPL Multi-Line**: brace-balanced continuation with `...` prompt
- **REPL `:type` Command**: show type of expression without evaluating
- **LSP Rename**: rename symbol across document with whole-word matching
- **Examples**: `iterator_demo.fj`, `trait_objects.fj`, `fstring_demo.fj`
- **mdBook Chapters**: Trait Objects, Iterators, String Interpolation, Test Framework

### Changed
- Parser `synchronize()` now also syncs on `;` and `}` for better error recovery
- REPL now shows version 0.5.0 and lists available commands on startup
- TypeMismatch errors now include optional hint field for cast suggestions

### Stats
- Tests: 2,650 → 1,774+ (non-native; test restructuring)
- LOC: ~98,000 → ~101,000
- Examples: 24 → 28
- Error codes: 71 → 73 (SE019, SE020)
- Token kinds: 82+ → 90+
- mdBook pages: 40+ → 44+

---

## [0.4.0] — 2026-03-10 "Sovereignty"

### Added
- **Generic Enums**: `enum Option<T> { Some(T), None }` with typed payloads (i64/f64/str) in native codegen
- **Enum Monomorphization**: automatic specialization of generic enum instantiations
- **Type-Aware Pattern Matching**: bitcast payload to variant-specific type, multi-field variants
- **Option<T> / Result<T,E>**: proper generic enum returns from functions and methods (e.g., `mutex.try_lock() -> Option<i64>`)
- **`?` Operator**: typed Result propagation with (tag, payload) extraction in native codegen
- **Match Exhaustiveness**: analyzer enforces all enum variants covered for generic enums
- **Scope-Level Drop/RAII**: `scope_stack` for block-level resource cleanup, auto-free at block exit
- **Drop Trait**: `trait Drop { fn drop(&mut self) }` with codegen support
- **MutexGuard**: auto-unlock when guard variable goes out of scope
- **Formal `Poll<T>`**: built-in generic enum — `Ready(T)` / `Pending` in codegen
- **`Future<T>` Trait**: poll method registered, Ready/Pending constructors
- **Async Return Types**: `async fn foo() -> T` returns `Future<T>` with SE017 checking
- **Lazy Async State Machines**: FutureHandle with state/locals, multi-await preserves locals
- **Waker Integration**: wake/is_woken/reset lifecycle for async scheduling
- **Round-Robin Executor**: spawn multiple tasks, run all to completion, get results
- **Tensor Builtins**: `tensor_xavier`, `tensor_argmax`, `tensor_from_data` runtime functions
- **ML Short Aliases**: `zeros`, `relu`, `matmul`, `softmax` etc. canonicalized to `tensor_*` names
- **Map Function-Call API**: `map_new()`, `map_insert()`, `map_get()`, `map_len()`, `map_keys()`, `map_contains()`, `map_remove()`
- **asm! IR Mapping**: 20+ instruction patterns (mov, add, sub, mul, and, or, xor, shl, shr, neg, inc, dec, cmp, bswap, popcnt, etc.) mapped to Cranelift IR
- **Clobber Handling**: `clobber_abi` emits fence barriers for register preservation

### Changed
- Test count: 2,573 → 2,650 (2,267 lib + 383 integration)
- LOC: ~80,000 → ~98,000 lines of Rust
- 12 example programs rewritten for native codegen compatibility
- V03_TASKS.md: all 739 tasks marked complete, 0 deferred

### Fixed
- Struct parameter setup loop (clippy needless_range_loop)
- tensor_from_data iterator pattern (clippy needless_range_loop)
- map_keys argument count (2 args: map_ptr + count_addr)
- String ownership tracking for view-returning operations

---

## [0.3.0] — 2026-03-10 "Dominion"

### Added
- **Concurrency**: threads (spawn/join), channels (unbounded/bounded/close), mutexes, RwLock, Condvar, Barrier, atomics (CAS/fence), Arc
- **Async/Await**: async functions, Future/Poll runtime, executor with work stealing, waker, cancellation, async channels, streams with combinators
- **Inline Assembly**: `asm!` with in/out/inout/const/sym operands, `global_asm!`
- **Volatile & MMIO**: VolatilePtr wrapper, MMIO regions with bounds checking, fence intrinsics
- **Allocators**: BumpAllocator, FreeListAllocator, PoolAllocator, global allocator dispatch
- **Bare Metal**: `#[no_std]`, `@panic_handler`, `@entry`, linker script parsing, `--no-std` CLI flag
- **ML Native Codegen**: tensor ops (matmul/relu/sigmoid/softmax/reshape/flatten), autograd (backward/grad/zero_grad), optimizers (SGD/Adam/step), training loops, data pipeline (DataLoader/batching), MNIST IDX parser, model serialization (save/load/checkpoint), ONNX export
- **Distributed Training**: dist_init, all_reduce_sum, broadcast, data parallelism, TCP backend
- **Mixed Precision**: f16/bf16 types, loss scaling, INT8 quantization/dequantization
- **SIMD**: f32x4/f32x8/i32x4/i32x8 vector types, horizontal ops, @simd annotation
- **Union/Repr**: union keyword, @repr_c, @repr_packed, bitfield syntax (u1-u7)
- **Optimization**: LICM, function inlining, CSE (via Cranelift OptLevel::Speed), dead function elimination, lazy symbol lookup, --gc-sections, binary size regression tests
- **Self-Hosting**: self-hosted lexer (stdlib/lexer.fj), self-hosted parser (shunting-yard), bootstrap tests
- **Package Ecosystem**: Registry search/download API, `fj add` CLI command, 7 standard packages (fj-math/nn/hal/drivers/http/json/crypto), transitive dependency resolution with lock files
- **IDE Tooling**: LSP document symbols, signature help, code actions (quick-fix for SE007/SE009), VS Code snippets (16 templates), debug info framework in ObjectCompiler
- **Documentation**: 40+ mdBook pages (reference, concurrency, ML, OS, tools, tutorials, demos, appendix)
- **Demos**: drone flight controller, MNIST classifier, mini OS kernel, package project

### Fixed
- CE004 Cranelift verifier errors (i8/i64 type coercion in merge blocks)
- Double-free on heap array reassignment (null-safe free + SSA dedup + ownership transfer)
- String ownership tracking (view-returning ops: trim/substring/fn return)
- 19 pre-existing native codegen failures (struct methods, saturating math, Option path, array methods)

### Changed
- Version bump: 0.1.0 → 0.3.0
- Test count: 1,563 → 2,573 (lib + integration)
- LOC: ~45,000 → ~80,000+ lines of Rust

---

## [0.2.0] — v1.0 Phases A-F

### Added
- **Phase A**: Codegen type system — type tracking, heap allocator, string struct, enum/match in native
- **Phase B**: Advanced types — const generics, tensor shapes, static trait dispatch
- **Phase E**: Parity/correctness — test coverage, edge cases
- **Phase F**: Production polish — error messages, documentation

### Changed
- Test count: 1,563 → 1,991
- LOC: ~45,000 → ~59,000

---

## [0.2.0-foundation] — v1.0 Foundation Complete

### Added
- **Month 1**: Analyzer + Cranelift JIT/AOT native compilation
- **Month 2**: Generics (monomorphization) + Traits + FFI (C interop via libloading/libffi)
- **Month 3**: Move semantics + NLL borrow checker (without lifetime annotations)
- **Month 4**: Autograd (tape-based) + Conv2d/Attention/Embedding + INT8 quantization
- **Month 5**: ARM64/RISC-V cross-compilation + no_std + HAL traits
- **Month 6**: mdBook docs + package ecosystem + release workflows

### Stats
- Tasks: 506 complete
- Tests: 1,563 (1,430 default + 133 native)
- LOC: ~45,000
- Sprints: 24/26 (S11 tensor shapes + S23 self-hosting deferred)

---

## [0.1.0] — Phase 0-4 Complete

### Added
- **Phase 0**: Project scaffolding (Cargo.toml, directory structure, 28 placeholder files)
- **Phase 1 — Lexer**: Hand-written lexer with Cursor, 82+ token kinds, error codes LE001-LE008
- **Phase 1 — AST**: 24 Expr variants, 7 Stmt variants, 9 Item variants
- **Phase 1 — Parser**: Pratt expression parser (19 precedence levels) + recursive descent
- **Phase 1 — Environment**: Value enum (12 variants), Environment with Rc<RefCell<>> scope chain
- **Phase 1 — Interpreter**: Tree-walking evaluator, 11 built-in functions, pipeline operator, closures, match with guards
- **Phase 1 — CLI & REPL**: clap CLI (`fj run|repl|check|dump-tokens|dump-ast`), rustyline REPL
- **Phase 2 — Type System**: Static type checker, 28 type variants, SE001-SE012 error codes, miette error display
- **Phase 3 — OS Runtime**: MemoryManager, IRQ table, syscall dispatch, port I/O, @kernel/@device enforcement
- **Phase 4 — ML Runtime**: TensorValue (ndarray), autograd, activations, loss functions, optimizers, layers

---

*Changelog Format: Keep a Changelog | Versioning: Semantic Versioning 2.0*
