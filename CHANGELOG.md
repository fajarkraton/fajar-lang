# Changelog

All notable changes to Fajar Lang are documented here.

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
