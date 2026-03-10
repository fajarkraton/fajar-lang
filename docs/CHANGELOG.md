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

## [1.0.0] — v1.0 Foundation Complete

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
