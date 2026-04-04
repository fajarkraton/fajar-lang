# Changelog

All notable changes to Fajar Lang are documented here.

## [20.8.0] — 2026-04-04 "Perfection"

### Added
- **FajarQuant**: Complete vector quantization system (7 phases, ~4,700 LOC)
  - TurboQuant baseline: Lloyd-Max quantizer, Algorithm 1 & 2
  - Innovation 1: PCA-based adaptive rotation (49-86% MSE improvement)
  - Innovation 2: Fused quantized attention (zero-copy codebook compute)
  - Innovation 3: Hierarchical multi-resolution bit allocation
  - Paper outline: `docs/FAJARQUANT_PAPER_OUTLINE.md`
- **Native JIT**: `fj run --jit` compiles hot functions via Cranelift (76x speedup on fib(30))
- **GPU Discovery**: `gpu_discover()` detects NVIDIA GPUs via CUDA Driver API
- **12 New Tensor/Scalar Ops**: sign, argmin, norm, dot, exp_tensor, log_tensor, sqrt_tensor, abs_tensor, clamp_tensor, where_tensor, exp, gamma
- **String Free Functions**: split, trim, contains, starts_with, ends_with, replace
- **read_file_text**: Convenience builtin returning string directly
- **RuntimeError Source Spans**: Division-by-zero, index OOB, undefined var now show file:line
- **Plugin CLI**: `fj plugin list`, `fj plugin load <path.so>`
- **Strict Mode**: `fj run --strict` rejects simulated builtins
- 31 V20 builtin tests, 20 tensor op tests, 22 FajarQuant tests, 8 safety tests, 8 E2E tests

### Changed
- **Tensor Display**: Now shows actual values (NumPy-like format), not just shape
- **matmul**: Auto-reshapes 1D tensors (dot product for vectors)
- **accelerate()**: Uses real CUDA GPU detection (detected RTX 4090, 9728 cores)
- **rl_agent_step**: Normalized -0.0 → 0.0

### Fixed
- `fj build` env var handling: wrapped std::env::set_var in unsafe{} (Rust >= 1.83)
- 2 registry_cli test failures (stale SQLite cleanup)
- `accelerate()` + `actor_send()`: replaced error-swallowing unwrap_or with ? propagation

### Removed
- 20,512 LOC dead code: src/demos/ (16,257), generators_v12.rs (372), ml/data.rs (236), 6 dead const_* modules (3,644)

### Stats
- 7,999 lib tests (0 failures) + 2,400+ integration tests
- ~459K LOC (down from 479K)
- 131/131 audit tests pass (100%)
- 42 [x] production + 5 [sim] simulated + 5 [f] framework + 3 [s] stub
- FajarQuant: 49-86% MSE improvement over TurboQuant
- JIT: 76x speedup on fib(30) with --features native

## [12.6.0] — 2026-04-02 "Infinity"

### Added
- **Effect Composition**: `effect Combined = IO + State` syntax in parser, analyzer, interpreter
- **Effect Row Polymorphism**: `with IO, ..r` open row variable syntax
- **Effect Statistics**: `fj run --effect-stats` prints runtime effect usage
- **AST-Driven GPU Codegen**: `fj build --target <spirv|ptx|metal|hlsl> input.fj`
- **GPU Workgroup Size**: `@gpu(workgroup=256)` annotation with shared memory support
- **Refinement Types**: `{ x: i32 | x > 0 }` with runtime predicate checking
- **Pi Types**: `Pi(n: usize) -> [f64; n]` dependent function type syntax
- **Sigma Types**: `Sigma(n: usize, [f64; n])` dependent pair type syntax
- **Async Registry Server**: tokio-based HTTP with CORS, HMAC-SHA256 signing
- **Rate Limiting**: Token bucket rate limiter for registry API
- **API Key Auth**: Registry publish authentication
- **Search Ranking**: Relevance-ranked package search (exact > prefix > substring > description)
- **Predictive LSP Completions**: Context-aware suggestions (let=, fn(, @annotation)
- **Code Lens Resolve**: LSP code_lens_resolve handler wired to tower-lsp
- **Boot Verification**: `fj verify --verbose` analyzes kernel boot patterns
- **Driver Interface Check**: Struct conformance verification for driver-like types
- **FFI Library Detection**: `fj hw-info` shows OpenCV, PostgreSQL, Python, PyTorch, QEMU availability
- **QEMU Boot Test**: Multiboot kernel boots in QEMU, serial output verified
- **OpenCV FFI Test**: Real C → OpenCV 4.6.0 image processing verified
- 8 new example programs (effect, GPU, refinement, Pi/Sigma, MNIST, kernel)

### Changed
- GPU codegen reads .fj source files instead of hardcoded kernels
- Registry server uses tokio::net::TcpListener (was std::net)
- Package signing uses HMAC-SHA256 via sha2 crate (was DefaultHasher)
- Effect declarations registered in analyzer first pass (was second pass)

### Stats
- 8,478 tests (0 failures)
- ~486K LOC (442 Rust files)
- 218 example .fj programs
- V14: 500/500 tasks complete
- V15: 98/120 tasks complete

## [12.5.0] — 2026-04-02

### Added
- V16 Horizon features: MNIST builtins, full pipeline, tutorials
- SPIR-V + PTX codegen via `fj build --target spirv/ptx`

## [12.4.0] — 2026-03-31

### Added
- V16 Horizon 97% production: 8,102 tests

## [12.3.0] — 2026-03-30

### Added
- V16 Horizon complete: 8,096 tests, 47 .fj programs
