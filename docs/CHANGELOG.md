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
- Test count: 1,563 → 2,016+ (lib) + 381 (integration)
- LOC: ~45,000 → ~80,000+ lines of Rust

---

## [Unreleased]

### Added
- **Phase 0**: Project scaffolding (Cargo.toml, directory structure, 28 placeholder files)
- **Phase 1 — Lexer** (Sprint 1.1): Hand-written lexer with Cursor, 82 tests
  - TokenKind enum (keywords, operators, literals, annotations)
  - Error codes LE001-LE008 (unexpected char, unterminated string, empty/multi-char literal, number overflow, etc.)
- **Phase 1 — AST** (Sprint 1.2): 24 Expr variants, 7 Stmt variants, 9 Item variants, 33 tests
- **Phase 1 — Parser** (Sprint 1.3): Pratt expression parser (19 precedence levels) + recursive descent, 94 tests
  - All expression/statement/item types parsed
  - Error recovery with synchronization, ParseError PE001-PE010
- **Phase 1 — Environment** (Sprint 1.4): Value enum (12 variants), Environment with Rc<RefCell<>> scope chain, 33 tests
- **Phase 1 — Interpreter** (Sprint 1.5): Tree-walking evaluator, 69 tests
  - All 24 expression types, control flow (return/break/continue via ControlFlow signals)
  - 11 built-in functions (print, println, len, type_of, push, pop, to_string, to_int, to_float, assert, assert_eq)
  - Pipeline operator, closures with capture, match with guards, struct/enum instantiation
  - Recursion limit: 256 depth
- **Phase 1 — CLI & REPL** (Sprint 1.6): clap CLI (`fj run|repl|check|dump-tokens|dump-ast`), rustyline REPL
  - Exit codes: 0=success, 1=runtime error, 2=compile error, 3=usage error
- **Phase 2 — Type System** (Sprint 2.1-2.10): Static type checker with two-pass analysis
  - Type enum (28 variants incl. distinct i8-i128, u8-u128, f32, f64, IntLiteral, FloatLiteral)
  - All 12 SemanticError codes (SE001-SE012) implemented
  - SymbolTable with lexical scoping, ScopeKind enum (7 variants), function/struct/enum registration
  - Distinct integer/float types: `i32 ≠ i64`, `f32 ≠ f64` — no implicit widening
  - IntLiteral/FloatLiteral inference for unsuffixed numeric literals
  - SE009 UnusedVariable (warning), SE010 UnreachableCode (warning), SE011 NonExhaustiveMatch
  - break/continue validated inside loop scope, return validated inside function scope
  - miette integration: beautiful source-highlighted error display with codes, spans, help text
  - Analyzer wired into CLI `check` and `run` commands
- **Phase 3 — OS Runtime** (Sprint 3.1-3.9): Complete OS subsystem
  - MemoryManager: bump allocator, alloc/free, read/write (u8/u32/u64/bytes), bounds checking
  - VirtAddr/PhysAddr distinct newtype structs (type-safe addresses)
  - PageTable: map/unmap pages, translate with offset, PageFlags (READ/WRITE/EXEC/USER)
  - IrqTable: register/unregister handlers, enable/disable, dispatch with logging
  - SyscallTable: define/undefine handlers, dispatch with arg count validation
  - PortIO: simulated x86 port read/write, default COM1/keyboard status
  - OsRuntime: combined subsystem struct
  - Pointer(u64) runtime value type
  - 16 OS builtins wired into interpreter (mem_*, page_*, irq_*, port_*)
  - @kernel/@device context enforcement: KE003, DE001, DE002 error codes
  - examples/memory_map.fj: working OS demo
  - 10 OS integration tests + 7 context enforcement tests
- **Phase 3 — OS Runtime Gap Fixes** (Sprint 3.10):
  - KE001 HeapAllocInKernel enforcement: push/pop/to_string blocked in @kernel context
  - KE002 TensorInKernel enforcement: tensor_builtins set ready (populated in Phase 4)
  - syscall_define/syscall_dispatch wired as interpreter builtins + type checker signatures
  - stdlib/os.fj: OS standard library (wrapper functions, constants)
  - src/stdlib/os.rs + src/stdlib/mod.rs: Rust stdlib module
  - 6 new integration tests (kernel init sequence, IRQ lifecycle, syscall from .fj)
  - 6 new KE001 unit tests
  - Total: 496 tests, all passing
- **Phase 4 — ML/AI Runtime** (Sprint 4.1-4.10): Complete ML subsystem
  - TensorValue: ndarray-backed, shape/grad/requires_grad/TensorId, creation (zeros/ones/randn/eye/full/from_data)
  - Element-wise ops: add/sub/mul/div/neg with NumPy-style broadcasting
  - Matrix ops: matmul (2D, inner dim validation), transpose, flatten, reshape
  - Reductions: sum, mean (scalar output)
  - Activation functions: relu, sigmoid, tanh, softmax (log-sum-exp trick), gelu, leaky_relu
  - Loss functions: mse_loss, cross_entropy, bce_loss (with epsilon clamping)
  - Tape-based autograd: Tape, TapeEntry, GradFn, tracked ops with backward for all arithmetic/matrix/activation/reduction ops
  - reduce_broadcast: gradient reduction for broadcast dimensions
  - numerical_gradient: central difference utility for gradient checking
  - Optimizers: SGD (with momentum), Adam (with bias-corrected moments)
  - Layers: Dense (Xavier init), Dropout (inverted scaling), BatchNorm (per-feature normalization)
  - 27 ML builtins wired into interpreter + type checker + KE002 enforcement
  - stdlib/nn.fj: Fajar Lang ML standard library, src/stdlib/nn.rs: ML_BUILTINS const
  - 23 ML integration tests (MNIST forward pass, gradient flow, numerical correctness, KE002)
- **Phase 4 — Gap Fixes** (Sprint 4.11):
  - Shape manipulation: flatten, squeeze, unsqueeze builtins
  - Additional reductions: max, min, argmax builtins
  - Additional creation: arange, linspace, xavier builtins
  - Additional loss: l1_loss builtin
  - 38 new builtins total (27 original + 11 gap-fix)
  - examples/mnist_forward.fj: working MNIST forward pass example
  - 8 new ML integration tests + 21 new unit tests for gap-fix ops
  - Total: 660 tests (598 unit + 12 eval + 31 ml + 16 os + 3 doc), all passing
- Documentation suite: 24 documents covering specification, architecture, testing, security, etc.
- Example programs: `hello.fj`, `fibonacci.fj`, `factorial.fj`
- 12 end-to-end integration tests in `tests/eval_tests.rs`

---

## [0.1.0] — Target: Phase 1 Complete

### Planned
- Complete lexer with all TokenKind variants
- AST definition for all expression and statement types
- Recursive descent parser with Pratt expression parsing
- Tree-walking interpreter for core language features
- CLI with subcommands: `run`, `repl`, `check`, `dump-tokens`, `dump-ast`
- REPL with rustyline
- Error display with miette
- Example programs: `hello.fj`, `fibonacci.fj`, `factorial.fj`

---

## [0.2.0] — Target: Phase 2 Complete

### Planned
- Type inference (Hindley-Milner lite)
- Generic types
- Tensor type with compile-time shape checking
- Context annotation enforcement (`@kernel`, `@device`, `@safe`, `@unsafe`)

---

## [0.3.0] — Target: Phase 3 Complete

### Planned
- OS Runtime: MemoryManager, IRQ table, syscall dispatch
- Virtual memory simulation
- Port I/O simulation
- `@kernel` context full enforcement

---

## [0.4.0] — Target: Phase 4 Complete

### Planned
- ML Runtime: TensorValue with autograd
- All activation functions and loss functions
- Neural network layers: Dense, Conv2d, Attention
- Optimizers: SGD, Adam
- SIMD acceleration via ndarray BLAS
- Integration tests: MNIST forward, XOR training

---

## [1.0.0] — Target: Phase 5-7 Complete

### Planned
- Bytecode VM for improved performance
- LLVM backend for native compilation
- GPU support via wgpu (`@device(gpu)`)
- Package manager (`fj.toml`, `fj add`, `fj build`)
- LSP for IDE integration
- Code formatter (`fj fmt`)
- Complete standard library
- Production-ready documentation site

---

*Changelog Format: Keep a Changelog | Versioning: Semantic Versioning 2.0*
