# Fajar Lang v0.9 "Convergence" — Implementation Plan

> **Focus:** Effect system, compile-time evaluation, macro system, SIMD vectorization, security hardening, async I/O
> **Timeline:** 28 sprints, ~280 tasks, 4-6 months
> **Prerequisite:** v0.8 "Apex" RELEASED
> **Theme:** *"Converge all subsystems — effects, comptime, macros, SIMD, security — into a production-ready whole"*

---

## Motivation

v0.8 delivered GPU training, GAT, incremental compilation, model optimization, DAP debugger, and LoRaWAN. But critical gaps remain for language maturity and production deployment:

- **No effect system** — side effects (I/O, allocation, exceptions) are invisible in the type system; safety-critical code needs effect tracking
- **No compile-time evaluation** — embedded ML needs compile-time shape checking, constant folding, and `const fn` for zero-cost abstractions
- **No macro system** — users can't extend syntax or generate boilerplate; `derive`, `cfg`, custom attributes all missing
- **No SIMD intrinsics** — leaving 4-16x performance on the table for tensor ops, crypto, and signal processing
- **No security hardening** — stack canaries, CFI, ASLR, and sanitizers not integrated into the compilation pipeline
- **No async I/O** — async/await exists but has no real I/O backend; io_uring and async networking needed
- **No stability guarantees** — API surface not frozen; no deprecation warnings; no edition system

v0.9 targets these gaps to make Fajar Lang production-ready for safety-critical embedded ML.

---

## Architecture Decisions

| # | Decision | Rationale |
|---|----------|-----------|
| 1 | Algebraic effects via `effect` keyword + handlers | Structured side-effect control, better than monads for embedded |
| 2 | `const fn` + `comptime {}` blocks | Zig-inspired comptime for zero-cost embedded abstractions |
| 3 | Declarative macros (`macro_rules!`) + procedural macros | Rust-compatible macro model, familiar to systems programmers |
| 4 | Platform-specific SIMD via `@simd` annotation | Auto-vectorization + manual intrinsics, portable across ARM/x86 |
| 5 | Stack canaries + shadow stack via Cranelift | Hardware-assisted CFI where available (ARM PAC, Intel CET) |
| 6 | Async I/O via io_uring simulation + epoll fallback | Modern Linux I/O, simulation stubs for portability |
| 7 | Edition system (edition = "2026") | Allows breaking changes without breaking existing code |

---

## Sprint Plan

### Phase 1: Effect System & Algebraic Effects `P0` `CRITICAL`

#### Sprint 1: Effect Declarations `P0`

**Goal:** Define effects as first-class types in the language

- [x] S1.1 — `src/analyzer/effects.rs`: Effect system module with `EffectDecl`, `EffectOp` structs
- [x] S1.2 — `EffectKind` enum: IO, Alloc, Panic, Async, State, Exception — 6 built-in effects
- [x] S1.3 — `effect` keyword in lexer (TokenKind::Effect) and parser (parse_effect_decl)
- [x] S1.4 — AST node: `Stmt::EffectDecl { name, operations: Vec<EffectOp> }` with typed ops
- [x] S1.5 — `EffectOp` struct: name, params, return_type — each operation is a typed signature
- [x] S1.6 — `EffectSet` type: ordered set of effects for function signatures (`fn foo() -> i32 / IO + Alloc`)
- [x] S1.7 — Effect parsing: `fn read_file(path: str) -> str / IO` syntax for effect annotations
- [x] S1.8 — Effect inference: functions without annotations get effects inferred from body
- [x] S1.9 — `EffectRegistry`: global registry of declared effects, validates uniqueness
- [x] S1.10 — 10 tests: effect declaration parsing, effect set operations, registry lookup

#### Sprint 2: Effect Handlers `P0`

**Goal:** Handle effects with `handle` blocks that provide implementations

- [x] S2.1 — `handle` keyword in lexer and parser — `handle <expr> { <effect> => <handler> }`
- [x] S2.2 — AST node: `Expr::Handle { body, handlers: Vec<EffectHandler> }`
- [x] S2.3 — `EffectHandler` struct: effect_name, op_handlers (map of op_name -> closure)
- [x] S2.4 — Handler resolution: find matching handler for effect op, error if unhandled
- [x] S2.5 — `resume` keyword: continue computation after effect is handled (delimited continuation)
- [x] S2.6 — Effect polymorphism: generic functions that are polymorphic over effects
- [x] S2.7 — Effect subtyping: `IO + Alloc` is a subtype of `IO` (can do less)
- [x] S2.8 — `perform` keyword: explicitly perform an effect operation (`perform IO.read_line()`)
- [x] S2.9 — Handler scoping: nested handlers, inner handler shadows outer for same effect
- [x] S2.10 — 10 tests: handler matching, resume continuation, nested handlers, effect polymorphism

#### Sprint 3: Effect Checking `P0`

**Goal:** Static effect checking in the type checker

- [x] S3.1 — Effect checking pass in `type_check.rs`: verify all effects are handled or propagated
- [x] S3.2 — `EE001` error: unhandled effect — function performs effect not in its signature
- [x] S3.3 — `EE002` error: effect mismatch — handler provides wrong type for effect operation
- [x] S3.4 — `EE003` error: missing handler — effect declared but no handler in scope
- [x] S3.5 — Effect coercion: pure functions (no effects) can be used where effectful functions expected
- [x] S3.6 — Context interaction: `@kernel` implies no `Alloc` effect; `@device` implies no `IO` effect
- [x] S3.7 — Effect erasure optimization: effects with trivial handlers compiled away at codegen
- [x] S3.8 — `#[pure]` annotation: function promises no side effects, compiler verifies
- [x] S3.9 — Effect compatibility with async: `async fn` implicitly has `Async` effect
- [x] S3.10 — 10 tests: effect checking errors, context interaction, pure annotation, async effects

#### Sprint 4: Effect System Integration `P0`

**Goal:** Integrate effects with existing language features

- [x] S4.1 — Effect-aware closures: closures capture effect requirements from environment
- [x] S4.2 — Effect-aware generics: `fn map<T, E: Effect>(f: fn(T) -> T / E) -> T / E`
- [x] S4.3 — Built-in `IO` effect handler: default implementation using Rust's std::io
- [x] S4.4 — Built-in `Alloc` effect handler: heap allocation tracking for @kernel context
- [x] S4.5 — Built-in `Exception` effect handler: try/catch desugaring via effect handling
- [x] S4.6 — Effect-aware trait methods: trait methods can declare effects
- [x] S4.7 — Effect documentation: effects shown in `fj doc` output
- [x] S4.8 — Effect inference across modules: cross-module effect propagation
- [x] S4.9 — `no_effect` bound: constraint that generic parameter has no effects
- [x] S4.10 — 10 tests: closures with effects, generics, built-in handlers, cross-module inference

### Phase 2: Compile-Time Evaluation `P0` `CRITICAL`

#### Sprint 5: const fn Foundations `P0`

**Goal:** Compile-time function evaluation for constant expressions

- [x] S5.1 — `src/compiler/comptime.rs`: Compile-time evaluator module with `ConstEval` struct
- [x] S5.2 — `const fn` declaration: `const fn max(a: i32, b: i32) -> i32 { if a > b { a } else { b } }`
- [x] S5.3 — `ConstValue` enum: Int(i128), Float(f64), Bool(bool), Str(String), Array(Vec<ConstValue>), Tuple(Vec<ConstValue>)
- [x] S5.4 — Const expression evaluator: arithmetic, comparison, logical ops on ConstValue
- [x] S5.5 — Const control flow: if/else, match (no loops in const context initially)
- [x] S5.6 — `const` blocks: `const { <expr> }` evaluates at compile time, result is a constant
- [x] S5.7 — Const validation: error if const fn calls non-const fn, accesses mutable state, or has IO
- [x] S5.8 — Const generic parameters: `struct Array<const N: usize> { data: [T; N] }`
- [x] S5.9 — Const propagation in analyzer: fold constant expressions during analysis
- [x] S5.10 — 10 tests: const fn eval, const blocks, const generics, validation errors

#### Sprint 6: comptime Blocks `P0`

**Goal:** Zig-style comptime for arbitrary compile-time computation

- [x] S6.1 — `comptime` keyword in lexer and parser — `comptime { <stmts> }`
- [x] S6.2 — AST node: `Expr::Comptime { body: Vec<Stmt> }` — evaluated at compile time
- [x] S6.3 — Comptime interpreter: subset of runtime interpreter that runs during compilation
- [x] S6.4 — Comptime type generation: `comptime` can produce types, used for metaprogramming
- [x] S6.5 — Comptime string manipulation: format!, concat!, stringify! at compile time
- [x] S6.6 — Comptime array generation: generate lookup tables, CRC tables at compile time
- [x] S6.7 — `@comptime` annotation on function params: param must be known at compile time
- [x] S6.8 — Comptime assertions: `comptime { assert(size_of::<T>() <= 64) }` — fails compilation
- [x] S6.9 — Comptime integration with const generics: `comptime` resolves generic const params
- [x] S6.10 — 10 tests: comptime blocks, type generation, string manipulation, assertions

#### Sprint 7: Compile-Time Tensor Shapes `P0`

**Goal:** Tensor shape checking at compile time using const evaluation

- [x] S7.1 — `Shape` as const type: `const SHAPE: [usize; 2] = [28, 28]` for MNIST
- [x] S7.2 — Shape arithmetic at compile time: `matmul(A: Tensor<M,K>, B: Tensor<K,N>) -> Tensor<M,N>`
- [x] S7.3 — Shape validation: compile error if shapes don't match (`TE009` ShapeMismatchComptime)
- [x] S7.4 — Broadcast rules at compile time: `[3, 1] + [1, 4]` → `[3, 4]`
- [x] S7.5 — Conv2d output shape: `(H - K + 2P) / S + 1` computed at compile time
- [x] S7.6 — Reshape validation: product of dimensions must match at compile time
- [x] S7.7 — Shape inference for layer chains: `Dense(784, 128) -> Dense(128, 10)` type-safe
- [x] S7.8 — Dynamic dimension marker: `Tensor<?, 10>` allows one dynamic dimension
- [x] S7.9 — Shape error messages: show expected vs actual shapes with full computation trace
- [x] S7.10 — 10 tests: shape arithmetic, matmul shapes, broadcast, conv2d shapes, reshape validation

#### Sprint 8: Const Evaluation Optimization `P1`

**Goal:** Optimize compile-time evaluation for practical use

- [x] S8.1 — Const memoization: cache results of const fn calls with same arguments
- [x] S8.2 — Const recursion limit: MAX_CONST_RECURSION = 128, error on exceeded
- [x] S8.3 — Const loop support: `const for` loops with bounded iteration count
- [x] S8.4 — Const struct construction: `const ORIGIN: Point = Point { x: 0.0, y: 0.0 }`
- [x] S8.5 — Const enum construction: `const NONE: Option<i32> = None`
- [x] S8.6 — Const function pointers: `const FN_PTR: fn(i32) -> i32 = double`
- [x] S8.7 — Const in match patterns: `match x { const MAX => ..., _ => ... }`
- [x] S8.8 — Const evaluation metrics: report time spent in const eval during compilation
- [x] S8.9 — Const eval cache persistence: save const eval results in artifact cache
- [x] S8.10 — 10 tests: memoization, recursion limit, loops, structs, enums, function pointers

### Phase 3: Macro System `P1`

#### Sprint 9: Declarative Macros `P1`

**Goal:** Pattern-matching macros similar to Rust's `macro_rules!`

- [x] S9.1 — `src/parser/macros.rs`: Macro expansion module with `MacroRule`, `MacroMatcher`
- [x] S9.2 — `macro_rules!` syntax: `macro_rules! vec { ($($x:expr),*) => { ... } }`
- [x] S9.3 — Token tree representation: `TokenTree` enum (Token, Group, Delimited)
- [x] S9.4 — Macro matchers: `$x:expr`, `$x:ident`, `$x:ty`, `$x:stmt`, `$x:block`, `$x:pat`
- [x] S9.5 — Repetition: `$(...)*` (zero or more), `$(...)+` (one or more), `$(...),*` (with separator)
- [x] S9.6 — Macro expansion: substitute matched fragments into template, re-parse result
- [x] S9.7 — Macro hygiene: generated identifiers don't collide with user code (gensym)
- [x] S9.8 — Recursive macros: macros can invoke themselves (with recursion limit 64)
- [x] S9.9 — Built-in macros: `vec![]`, `println!()`, `format!()`, `assert!()`, `cfg!()`, `dbg!()`
- [x] S9.10 — 10 tests: macro definition, pattern matching, repetition, hygiene, recursion

#### Sprint 10: Derive Macros `P1`

**Goal:** Auto-derive trait implementations via macros

- [x] S10.1 — `#[derive(...)]` attribute parsing in the parser
- [x] S10.2 — `DeriveMacro` trait: `fn expand(item: &Item) -> Vec<Item>` for code generation
- [x] S10.3 — Built-in `#[derive(Debug)]`: generate `to_string()` method for structs/enums
- [x] S10.4 — Built-in `#[derive(Clone)]`: generate `clone()` method with field-by-field copy
- [x] S10.5 — Built-in `#[derive(PartialEq)]`: generate `eq()` method comparing all fields
- [x] S10.6 — Built-in `#[derive(Hash)]`: generate `hash()` method for HashMap keys
- [x] S10.7 — Built-in `#[derive(Default)]`: generate `default()` with zero/empty values
- [x] S10.8 — Built-in `#[derive(Serialize)]`: generate ONNX/JSON serialization code
- [x] S10.9 — Custom derive: users can register derive macros via `#[proc_macro_derive]`
- [x] S10.10 — 10 tests: derive Debug, Clone, PartialEq, Hash, Default, Serialize, custom derive

#### Sprint 11: Attribute Macros `P1`

**Goal:** Custom attributes that transform items

- [x] S11.1 — `#[attr]` and `#[attr(...)]` parsing for arbitrary attributes
- [x] S11.2 — `AttributeMacro` trait: `fn expand(attr: &TokenTree, item: &Item) -> Vec<Item>`
- [x] S11.3 — Built-in `#[cfg(target = "...")]`: conditional compilation by target
- [x] S11.4 — Built-in `#[cfg(feature = "...")]`: conditional compilation by feature
- [x] S11.5 — Built-in `#[inline]` / `#[inline(always)]`: inlining hints for codegen
- [x] S11.6 — Built-in `#[deprecated(message = "...")]`: deprecation warnings in analyzer
- [x] S11.7 — Built-in `#[allow(...)]` / `#[deny(...)]`: lint control attributes
- [x] S11.8 — Built-in `#[repr(C)]` / `#[repr(packed)]`: memory layout control
- [x] S11.9 — Custom attribute macros: users can register via `#[proc_macro_attribute]`
- [x] S11.10 — 10 tests: cfg, feature gates, inline, deprecated, repr, custom attributes

#### Sprint 12: Macro Utilities & Integration `P1`

**Goal:** Complete macro system with utilities and error reporting

- [x] S12.1 — `compile_error!("message")`: user-triggered compile error in macros
- [x] S12.2 — `include!("path")`: include file contents as tokens
- [x] S12.3 — `env!("VAR")`: read environment variable at compile time
- [x] S12.4 — `file!()`, `line!()`, `column!()`: source location macros
- [x] S12.5 — `stringify!(expr)`: convert expression to string literal
- [x] S12.6 — Macro error reporting: show expansion trace on error, point to macro definition
- [x] S12.7 — Macro documentation: `///` comments on macros shown in `fj doc`
- [x] S12.8 — Macro export: `pub macro_rules!` exports macro from module
- [x] S12.9 — Macro import: `use module::macro_name!` imports macro
- [x] S12.10 — 10 tests: compile_error, include, env, source location, error reporting, import/export

### Phase 4: SIMD & Vectorization `P1`

#### Sprint 13: SIMD Type System `P1`

**Goal:** SIMD vector types as first-class language primitives

- [x] S13.1 — `src/runtime/simd.rs`: SIMD module with vector type definitions
- [x] S13.2 — Vector types: `v128`, `v256`, `v512` — 128/256/512-bit SIMD vectors
- [x] S13.3 — Typed vectors: `i32x4`, `f32x4`, `i32x8`, `f32x8`, `f64x2`, `f64x4`
- [x] S13.4 — Vector construction: `i32x4(1, 2, 3, 4)`, `f32x4::splat(0.0)`
- [x] S13.5 — Lane access: `v[0]`, `v[1]` — individual lane read/write
- [x] S13.6 — Vector arithmetic: `+`, `-`, `*`, `/` on vector types (lane-wise)
- [x] S13.7 — Comparison: `==`, `<`, `>` return mask vectors (`m32x4`)
- [x] S13.8 — Shuffle/swizzle: `v.shuffle(0, 2, 1, 3)` — lane reordering
- [x] S13.9 — `SimdCapability` detection: runtime query for SSE/AVX/NEON/SVE support
- [x] S13.10 — 10 tests: vector construction, arithmetic, comparison, shuffle, capability detection

#### Sprint 14: Platform SIMD Intrinsics `P1`

**Goal:** Architecture-specific SIMD operations

- [x] S14.1 — x86 SSE intrinsics: `_mm_add_ps`, `_mm_mul_ps`, `_mm_shuffle_ps` (via simulation)
- [x] S14.2 — x86 AVX intrinsics: `_mm256_add_ps`, `_mm256_fmadd_ps` (via simulation)
- [x] S14.3 — x86 AVX-512 intrinsics: `_mm512_add_ps`, mask operations (via simulation)
- [x] S14.4 — ARM NEON intrinsics: `vaddq_f32`, `vmulq_f32`, `vld1q_f32` (via simulation)
- [x] S14.5 — ARM SVE intrinsics: scalable vector length, predicated operations (via simulation)
- [x] S14.6 — RISC-V V intrinsics: `vfadd`, `vfmul`, variable-length vectors (via simulation)
- [x] S14.7 — `@simd` function annotation: hint to auto-vectorize loops
- [x] S14.8 — SIMD ABI: vector types passed in SIMD registers (XMM/YMM/ZMM, Q/D, V)
- [x] S14.9 — SIMD alignment: `#[align(16)]` / `#[align(32)]` for vector-aligned data
- [x] S14.10 — 10 tests: SSE ops, AVX ops, NEON ops, SVE ops, auto-vectorization hint

#### Sprint 15: SIMD Tensor Operations `P1`

**Goal:** SIMD-accelerated tensor primitives

- [x] S15.1 — SIMD matmul: 4x4 block matrix multiply using f32x4 vectors
- [x] S15.2 — SIMD elementwise: vectorized add/sub/mul/div for tensor data
- [x] S15.3 — SIMD reduction: horizontal sum, max, min across vector lanes
- [x] S15.4 — SIMD activation: vectorized ReLU, sigmoid approximation, tanh approximation
- [x] S15.5 — SIMD softmax: vectorized exp approximation + reduction
- [x] S15.6 — SIMD dot product: `f32x4` dot product with horizontal add
- [x] S15.7 — SIMD conv1d: sliding window convolution with packed multiply-add
- [x] S15.8 — SIMD quantize: vectorized float-to-int8 conversion with rounding
- [x] S15.9 — SIMD benchmark: compare scalar vs SIMD for matmul, relu, softmax
- [x] S15.10 — 10 tests: SIMD matmul correctness, elementwise, reduction, activation, benchmark

#### Sprint 16: Auto-Vectorization `P1`

**Goal:** Automatic loop vectorization by the compiler

- [x] S16.1 — `src/compiler/vectorize.rs`: Auto-vectorization pass module
- [x] S16.2 — Loop analysis: detect vectorizable loops (no loop-carried dependencies)
- [x] S16.3 — Trip count analysis: determine loop iteration count for vector width selection
- [x] S16.4 — Vectorization transform: replace scalar ops with vector ops, add prologue/epilogue
- [x] S16.5 — Cost model: estimate speedup of vectorized vs scalar loop, skip if not profitable
- [x] S16.6 — Gather/scatter: handle non-contiguous memory access patterns
- [x] S16.7 — Reduction vectorization: `sum += a[i]` → vector accumulate + horizontal reduce
- [x] S16.8 — Conditional vectorization: masked operations for loops with if-conditions
- [x] S16.9 — Vectorization report: `--emit=vectorization-report` shows which loops were vectorized
- [x] S16.10 — 10 tests: simple loop, trip count, cost model, reduction, conditional, report

### Phase 5: Security Hardening `P1`

#### Sprint 17: Stack Protection `P1`

**Goal:** Runtime stack overflow and buffer overflow protection

- [x] S17.1 — `src/compiler/security.rs`: Security hardening module with `SecurityConfig`
- [x] S17.2 — Stack canaries: insert random canary value before return address, check on return
- [x] S17.3 — `CanaryGenerator`: per-function random canary, checked at function epilogue
- [x] S17.4 — Stack clash protection: probe guard pages on large stack allocations
- [x] S17.5 — `__stack_chk_fail` handler: abort with diagnostic on canary violation
- [x] S17.6 — `-fsanitize=stack` flag: enable stack protection in compilation
- [x] S17.7 — Shadow stack: separate return address stack (simulation of Intel CET / ARM PAC)
- [x] S17.8 — Stack usage analysis: static analysis of maximum stack depth per function
- [x] S17.9 — Stack overflow detection: guard page at end of stack, SIGSEGV handler
- [x] S17.10 — 10 tests: canary insertion, canary check, stack clash, shadow stack, overflow detection

#### Sprint 18: Control Flow Integrity `P1`

**Goal:** Prevent control flow hijacking attacks

- [x] S18.1 — Forward-edge CFI: validate indirect call targets against type signature
- [x] S18.2 — `CfiMetadata`: per-function type hash, checked at indirect call sites
- [x] S18.3 — Backward-edge CFI: shadow stack protects return addresses
- [x] S18.4 — Function pointer validation: check pointer target is valid function entry
- [x] S18.5 — VTable integrity: read-only VTable with type hash verification
- [x] S18.6 — CFI error handler: `__cfi_check_fail` with diagnostic information
- [x] S18.7 — `-fsanitize=cfi` flag: enable CFI in compilation
- [x] S18.8 — Jump table protection: bounds-check switch/match jump tables
- [x] S18.9 — Return-oriented programming (ROP) mitigation: diversify function prologues
- [x] S18.10 — 10 tests: forward CFI, backward CFI, vtable integrity, function pointer validation

#### Sprint 19: Memory Safety Runtime `P1`

**Goal:** Runtime memory safety checks beyond compile-time analysis

- [x] S19.1 — AddressSanitizer (ASan) simulation: detect use-after-free, buffer overflow at runtime
- [x] S19.2 — `ShadowMemory`: track allocation state (allocated/freed/poisoned) per byte
- [x] S19.3 — Red zones: padding around heap allocations to detect overflow
- [x] S19.4 — Quarantine: delay reuse of freed memory to detect use-after-free
- [x] S19.5 — MemorySanitizer (MSan) simulation: detect use of uninitialized memory
- [x] S19.6 — `UndefinedBehavior` error type: UB001-UB005 for runtime safety violations
- [x] S19.7 — `-fsanitize=address` flag: enable ASan in compilation
- [x] S19.8 — Leak detection: report unfreed allocations at program exit
- [x] S19.9 — Double-free detection: track freed pointers, error on second free
- [x] S19.10 — 10 tests: use-after-free, buffer overflow, uninitialized, leak, double-free detection

#### Sprint 20: Secure Compilation `P1`

**Goal:** Compilation-level security features

- [x] S20.1 — ASLR support: position-independent code (PIC) generation for executables
- [x] S20.2 — RELRO: full relocation read-only for GOT/PLT protection
- [x] S20.3 — NX stack: mark stack as non-executable in ELF headers
- [x] S20.4 — Fortify source: replace unsafe builtins with bounds-checking versions
- [x] S20.5 — `-fharden` flag: enable all security features at once
- [x] S20.6 — Security audit report: `fj audit` command summarizes security posture
- [x] S20.7 — Dependency vulnerability scanning: check package deps against known CVEs
- [x] S20.8 — Binary hardening score: rate compiled binary 0-100 based on enabled protections
- [x] S20.9 — Secure defaults: security features enabled by default in release builds
- [x] S20.10 — 10 tests: PIC generation, RELRO, NX stack, fortify, audit report, hardening score

### Phase 6: Async I/O & Networking `P2`

#### Sprint 21: Async I/O Backend `P2`

**Goal:** Real async I/O operations with io_uring simulation

- [x] S21.1 — `src/runtime/async_io.rs`: Async I/O module with `IoBackend` trait
- [x] S21.2 — `IoUringBackend` simulation: submission queue, completion queue, sqe/cqe structs
- [x] S21.3 — Async file read: `async fn read_file(path: str) -> Result<str, IoError>`
- [x] S21.4 — Async file write: `async fn write_file(path: str, data: str) -> Result<(), IoError>`
- [x] S21.5 — Async accept: `async fn accept(listener: TcpListener) -> Result<TcpStream, IoError>`
- [x] S21.6 — Async connect: `async fn connect(addr: str) -> Result<TcpStream, IoError>`
- [x] S21.7 — `EpollBackend` simulation: fallback for non-io_uring systems
- [x] S21.8 — Readiness notification: `poll_ready()` integration with async executor
- [x] S21.9 — Buffered I/O: `BufReader`, `BufWriter` wrappers with async support
- [x] S21.10 — 10 tests: async file read/write, async accept/connect, epoll fallback, buffered I/O

#### Sprint 22: TCP/UDP Networking `P2`

**Goal:** Async networking primitives

- [x] S22.1 — `src/runtime/net.rs`: Networking module with `TcpListener`, `TcpStream`, `UdpSocket`
- [x] S22.2 — `TcpListener::bind(addr)`: create listening socket on address:port
- [x] S22.3 — `TcpStream::read/write`: async read and write on TCP connections
- [x] S22.4 — `UdpSocket::send_to/recv_from`: async UDP send and receive
- [x] S22.5 — DNS resolution: `resolve(hostname) -> Vec<IpAddr>` (simulation)
- [x] S22.6 — Socket options: SO_REUSEADDR, TCP_NODELAY, SO_KEEPALIVE
- [x] S22.7 — Timeout support: `with_timeout(duration, future)` wrapper
- [x] S22.8 — Connection pooling: `ConnectionPool` for reusing TCP connections
- [x] S22.9 — TLS support: `TlsStream` wrapping `TcpStream` with simulated handshake
- [x] S22.10 — 10 tests: TCP listener/stream, UDP socket, DNS resolve, timeout, TLS handshake

#### Sprint 23: HTTP Client/Server `P2`

**Goal:** HTTP/1.1 protocol implementation

- [x] S23.1 — `src/runtime/http.rs`: HTTP module with `HttpRequest`, `HttpResponse`
- [x] S23.2 — HTTP request parsing: method, path, headers, body from byte stream
- [x] S23.3 — HTTP response building: status code, headers, body serialization
- [x] S23.4 — `HttpClient`: `async fn get/post/put/delete(url) -> HttpResponse`
- [x] S23.5 — `HttpServer`: route registration, request dispatch, middleware chain
- [x] S23.6 — Content types: JSON, form-urlencoded, multipart, plain text
- [x] S23.7 — Chunked transfer encoding: streaming request/response bodies
- [x] S23.8 — Keep-alive: connection reuse with configurable idle timeout
- [x] S23.9 — CORS: cross-origin resource sharing headers and preflight
- [x] S23.10 — 10 tests: request parse, response build, client GET/POST, server routing, chunked

#### Sprint 24: Protocol Integration `P2`

**Goal:** Higher-level protocol support

- [x] S24.1 — WebSocket support: upgrade handshake, frame encoding/decoding, ping/pong
- [x] S24.2 — gRPC stubs: protobuf-compatible message serialization (simulation)
- [x] S24.3 — MQTT over TCP: connect to broker, subscribe, publish (extends IoT module)
- [x] S24.4 — HTTP/2 framing: stream multiplexing, header compression (HPACK simulation)
- [x] S24.5 — `ProtocolStack`: unified API for TCP/UDP/WebSocket/MQTT selection
- [x] S24.6 — Rate limiting: token bucket rate limiter for server endpoints
- [x] S24.7 — Circuit breaker: fault tolerance pattern for client connections
- [x] S24.8 — Retry policy: exponential backoff with jitter for failed requests
- [x] S24.9 — Metrics collection: request count, latency histogram, error rate
- [x] S24.10 — 10 tests: WebSocket frames, gRPC stubs, MQTT integration, rate limiting, circuit breaker

### Phase 7: Production Polish & Release `P2`

#### Sprint 25: Edition System `P2`

**Goal:** Version stability with edition-based migration

- [x] S25.1 — `src/compiler/edition.rs`: Edition system module with `Edition` enum (Edition2025, Edition2026)
- [x] S25.2 — `edition = "2026"` field in `fj.toml` manifest — default for new projects
- [x] S25.3 — Edition-specific keyword reservation: new keywords only reserved in later editions
- [x] S25.4 — `#[deprecated(since = "0.9", note = "use X instead")]`: deprecation with edition
- [x] S25.5 — `fj fix --edition 2026`: auto-migration tool for edition changes
- [x] S25.6 — Edition compatibility: libraries compiled with edition 2025 link with edition 2026
- [x] S25.7 — Warning for edition-specific code: "this will be an error in edition 2026"
- [x] S25.8 — Edition documentation: guide for upgrading between editions
- [x] S25.9 — Feature stability levels: stable, unstable, deprecated — gated by edition
- [x] S25.10 — 10 tests: edition parsing, deprecation warnings, migration tool, cross-edition linking

#### Sprint 26: API Stability & Versioning `P2`

**Goal:** Stable public API surface with SemVer guarantees

- [x] S26.1 — `src/compiler/stability.rs`: API stability checker module
- [x] S26.2 — `#[stable(since = "0.9")]`: mark public API as stable
- [x] S26.3 — `#[unstable(feature = "...")]`: gate unstable features behind feature flags
- [x] S26.4 — Breaking change detection: compare public API between versions
- [x] S26.5 — SemVer validation: ensure version bump matches API change level
- [x] S26.6 — API diff report: `fj api-diff v0.8 v0.9` shows added/removed/changed items
- [x] S26.7 — Deprecation timeline: warn for 2 minor versions before removal
- [x] S26.8 — Feature flag registry: track all unstable features and their stabilization status
- [x] S26.9 — Public API documentation: auto-generate stable API reference
- [x] S26.10 — 10 tests: stability attributes, breaking change detection, SemVer validation, API diff

#### Sprint 27: Benchmark Suite & Performance `P2`

**Goal:** Comprehensive performance benchmarks and optimization

- [x] S27.1 — `benches/v09_bench.rs`: v0.9 feature benchmarks (effects, comptime, SIMD, security)
- [x] S27.2 — Effect system overhead: measure cost of effect handling vs direct calls
- [x] S27.3 — Comptime evaluation speed: benchmark const fn evaluation time
- [x] S27.4 — SIMD speedup measurement: scalar vs SIMD for matmul, relu, softmax
- [x] S27.5 — Security overhead: measure cost of stack canaries, CFI, ASan
- [x] S27.6 — Async I/O throughput: measure requests/second for HTTP server
- [x] S27.7 — Macro expansion time: measure compile-time cost of macro expansion
- [x] S27.8 — Compilation speed regression test: ensure v0.9 features don't slow compilation >10%
- [x] S27.9 — Binary size tracking: report size impact of each security feature
- [x] S27.10 — 10 tests: benchmark correctness, regression detection, overhead measurement

#### Sprint 28: Documentation & Release `P2`

**Goal:** Final documentation, examples, and release preparation

- [x] S28.1 — mdBook chapter: "Effect System" — declaring effects, handlers, pure functions
- [x] S28.2 — mdBook chapter: "Compile-Time Evaluation" — const fn, comptime, tensor shapes
- [x] S28.3 — mdBook chapter: "Macros" — declarative, derive, attribute, built-in macros
- [x] S28.4 — mdBook chapter: "SIMD Programming" — vector types, intrinsics, auto-vectorization
- [x] S28.5 — mdBook chapter: "Security" — stack protection, CFI, sanitizers, hardening
- [x] S28.6 — Example: `effects_demo.fj` — algebraic effects for I/O, state, exceptions
- [x] S28.7 — Example: `simd_image.fj` — SIMD image processing (blur, sharpen, edge detect)
- [x] S28.8 — Example: `http_server.fj` — async HTTP server with routing and JSON
- [x] S28.9 — CHANGELOG.md update with full v0.9.0 entry
- [x] S28.10 — Version bump to 0.9.0 in Cargo.toml, update CLAUDE.md status

---

## Success Criteria

| Metric | Target |
|--------|--------|
| All 280 tasks | `[x]` checked |
| Tests | 3,100+ total, 0 failures |
| Clippy | Zero warnings (`-D warnings`) |
| Formatting | `cargo fmt -- --check` passes |
| Examples | 49+ `.fj` programs |
| LOC | ~185,000 Rust |
| New modules | 10+ new source files |
| Benchmarks | SIMD shows >2x speedup vs scalar |
| Security | Binary hardening score >80 with `-fharden` |

---

## Phase Dependencies

```
Phase 1 (Effects) ──────┐
Phase 2 (Comptime) ─────┼── Independent, can run in parallel
Phase 3 (Macros) ───────┘
         │
         v
Phase 4 (SIMD) ←── depends on comptime for const vector width
Phase 5 (Security) ←── independent
Phase 6 (Async I/O) ←── independent
         │
         v
Phase 7 (Release) ←── depends on all above being complete
```

---

*V09_PLAN.md — Created 2026-03-11*
