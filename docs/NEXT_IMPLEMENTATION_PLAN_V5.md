# Fajar Lang + FajarOS — Implementation Plan V5

> **Date:** 2026-03-25
> **Author:** Fajar (PrimeCore.id) + Claude Opus 4.6
> **Status:** Post Plan V3 (240/268) + V4 (all done except B). Mega-session: ~500 tasks.
> **Current:** Fajar Lang v6.1.0, FajarOS Nova v2.0.0, fajaros-x86 v2.0.0 (139 files, 37K LOC)
> **Purpose:** Comprehensive per-sprint, per-task plans for 8 options, 518 tasks.

---

## Overview

| # | Option | Sprints | Tasks | Effort | Priority |
|---|--------|---------|-------|--------|----------|
| 1 | Self-Hosting Compiler v2 | 10 | 100 | ~20 hrs | HIGH |
| 2 | GPU Compute Backend | 6 | 60 | ~12 hrs | MEDIUM |
| 3 | Package Registry | 4 | 40 | ~8 hrs | HIGH |
| 4 | Fajar Lang v0.9 | 8 | 80 | ~16 hrs | HIGH |
| 5 | Q6A Full Deploy | 3 | 28 | ~6 hrs | BLOCKED |
| 6 | Nova v2.0 "Phoenix" | 14 | 140 | ~28 hrs | MEDIUM |
| 7 | Education Platform | 4 | 40 | ~8 hrs | LOW |
| 8 | Benchmarks Suite | 3 | 30 | ~6 hrs | MEDIUM |
| **Total** | | **52** | **518** | **~104 hrs** | |

**Recommended order:** 4 → 3 → 8 → 1 → 2 → 7 → 6 → 5 (when Q6A available)

---

## Option 1: Self-Hosting Compiler v2 (10 sprints, 100 tasks)

**Goal:** Write Fajar Lang compiler in Fajar Lang — full bootstrap
**Codename:** "Ouroboros"
**Effort:** ~20 hours

### Phase S1: Lexer in Fajar Lang (2 sprints, 20 tasks)

#### Sprint S1.1: Tokenizer Core (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| S1.1.1 | Create stdlib/selfhost/lexer.fj | Main file structure, Token enum | [x] |
| S1.1.2 | Cursor struct | peek, advance, is_eof, position tracking | [x] |
| S1.1.3 | Whitespace + comment skipping | `//' line comments, `/* */` block comments | [x] |
| S1.1.4 | Integer literals | Decimal, hex (0x), binary (0b), octal (0o), underscores | [x] |
| S1.1.5 | Float literals | `3.14`, `1e10`, `1.5e-3` | [x] |
| S1.1.6 | String literals | `"hello"`, escape sequences `\n \t \\ \"` | [x] |
| S1.1.7 | Char literals | `'a'`, `'\n'` | [x] |
| S1.1.8 | Identifiers + keywords | 50+ keywords, contextual keywords (tensor, grad, etc.) | [x] |
| S1.1.9 | Operators + punctuation | 40+ operators, multi-char (`==`, `!=`, `|>`, `..=`) | [x] |
| S1.1.10 | 10 tokenizer tests | Write .fj tests that tokenize sample programs | [x] |

#### Sprint S1.2: Tokenizer Advanced + Verification (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| S1.2.1 | F-string tokenization | `f"Hello {name}"` → string parts + expressions | [x] |
| S1.2.2 | Annotation tokens | `@kernel`, `@device`, `@safe`, `@unsafe` | [x] |
| S1.2.3 | Lifetime tokens | `'a`, `'static` | [x] |
| S1.2.4 | Attribute tokens | `#[derive(Debug)]`, `#[cfg(test)]` | [x] |
| S1.2.5 | Error recovery | Continue tokenizing after error, collect all errors | [x] |
| S1.2.6 | Span tracking | Line:column for every token | [x] |
| S1.2.7 | Tokenize hello.fj | Self-host tokenizer produces same tokens as Rust tokenizer | [x] |
| S1.2.8 | Tokenize fibonacci.fj | Verify on real program | [x] |
| S1.2.9 | Tokenize array_methods.fj | Verify closures, methods, pipes | [x] |
| S1.2.10 | Benchmark: .fj vs Rust tokenizer | Compare speed and correctness | [x] |

### Phase S2: Parser in Fajar Lang (3 sprints, 30 tasks)

#### Sprint S2.1: Expression Parser (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| S2.1.1 | AST types in .fj | Expr, Stmt, Item enums | [x] |
| S2.1.2 | Pratt parser core | `parse_expr(min_precedence)` — 19 levels | [x] |
| S2.1.3 | Literal expressions | Int, Float, Bool, String, Char, Null | [x] |
| S2.1.4 | Binary expressions | `+`, `-`, `*`, `/`, `%`, `==`, `!=`, `<`, `>`, etc. | [x] |
| S2.1.5 | Unary expressions | `!`, `-`, `~`, `&`, `&mut` | [x] |
| S2.1.6 | Call expressions | `f(a, b)`, `obj.method(a)` | [x] |
| S2.1.7 | Index expressions | `arr[i]`, `map["key"]` | [x] |
| S2.1.8 | Closure expressions | `\|x, y\| x + y`, `\|x: i32\| -> i32 { ... }` | [x] |
| S2.1.9 | If/match expressions | `if cond { a } else { b }`, `match x { ... }` | [x] |
| S2.1.10 | 10 parser tests | Parse sample expressions, verify AST | [x] |

#### Sprint S2.2: Statement + Item Parser (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| S2.2.1 | Let statements | `let x = 42`, `let mut y: i32 = 0` | [x] |
| S2.2.2 | Assignment | `x = 5`, `arr[i] = val`, `obj.field = val` | [x] |
| S2.2.3 | Return/break/continue | `return expr`, `break 'label`, `continue` | [x] |
| S2.2.4 | While/for/loop | `while cond { }`, `for x in iter { }`, `loop { }` | [x] |
| S2.2.5 | Function definitions | `fn name(params) -> RetType { body }` | [x] |
| S2.2.6 | Struct definitions | `struct Name { field: Type }` | [x] |
| S2.2.7 | Enum definitions | `enum Name { Variant(Type) }` | [x] |
| S2.2.8 | Trait + impl | `trait T { }`, `impl T for S { }` | [x] |
| S2.2.9 | Use/mod statements | `use std::io::println`, `mod math` | [x] |
| S2.2.10 | 10 statement tests | Parse full programs, verify structure | [x] |

#### Sprint S2.3: Parser Completion + Testing (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| S2.3.1 | Generics parsing | `<T>`, `<T: Bound>`, `where T: Display` | [x] |
| S2.3.2 | Pattern matching | `match x { Some(v) => ..., None => ... }` | [x] |
| S2.3.3 | Type expressions | `i32`, `[T; N]`, `Option<T>`, `Result<T, E>`, `fn(A) -> B` | [x] |
| S2.3.4 | Async/await | `async fn`, `.await`, `async { }` | [x] |
| S2.3.5 | Error recovery | Skip to next statement on parse error | [x] |
| S2.3.6 | Parse hello.fj | Full program parse in .fj | [x] |
| S2.3.7 | Parse fibonacci.fj | Recursive functions | [x] |
| S2.3.8 | Parse fajaros_nova_kernel.fj | 21,187 lines (stress test) | [x] |
| S2.3.9 | AST pretty-printer | Print parsed AST back as source code | [x] |
| S2.3.10 | Compare AST output | .fj parser vs Rust parser — identical AST | [x] |

### Phase S3: Code Generation (2 sprints, 20 tasks)

#### Sprint S3.1: C Backend (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| S3.1.1 | C codegen scaffold | AST → C source code (transpiler) | [x] |
| S3.1.2 | Functions → C functions | `fn add(a: i32, b: i32) -> i32` → `int add(int a, int b)` | [x] |
| S3.1.3 | Structs → C structs | Field layout, alignment | [x] |
| S3.1.4 | Control flow → C | if/while/for/match → C equivalents | [x] |
| S3.1.5 | Arrays → C arrays | Stack arrays, heap arrays (malloc) | [x] |
| S3.1.6 | String handling | String type → `char*` with length | [x] |
| S3.1.7 | Closures → C | Function pointer + environment struct | [x] |
| S3.1.8 | Runtime library | `fj_print()`, `fj_alloc()`, `fj_panic()` in C | [x] |
| S3.1.9 | Compile hello.fj → hello.c → binary | End-to-end verification | [x] |
| S3.1.10 | Compile fibonacci.fj → C → binary | Verify correctness | [x] |

#### Sprint S3.2: Optimization + Testing (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| S3.2.1 | Constant folding | `1 + 2` → `3` at compile time | [x] |
| S3.2.2 | Dead code elimination | Remove unreachable functions | [x] |
| S3.2.3 | Inline small functions | Functions < 5 statements | [x] |
| S3.2.4 | Type inference in codegen | Resolve `let x = 42` → `int x = 42` | [x] |
| S3.2.5 | Error messages | "line X: type mismatch: expected i32, got str" | [x] |
| S3.2.6 | Compile 10 example programs | Verify all produce correct output | [x] |
| S3.2.7 | Compile array_methods.fj | Closures + higher-order methods | [x] |
| S3.2.8 | Performance comparison | .fj compiler speed vs Rust compiler speed | [x] |
| S3.2.9 | Memory safety | No buffer overflows in generated C code | [x] |
| S3.2.10 | Documentation | SELFHOST.md — how the self-hosted compiler works | [x] |

### Phase S4: Bootstrap (2 sprints, 20 tasks)

#### Sprint S4.1: Stage 1 Bootstrap (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| S4.1.1 | Compile lexer.fj with Rust `fj` | → lexer.c → lexer binary | [x] |
| S4.1.2 | Compile parser.fj with Rust `fj` | → parser.c → parser binary | [x] |
| S4.1.3 | Compile codegen.fj with Rust `fj` | → codegen.c → codegen binary | [x] |
| S4.1.4 | Link stage-1 compiler | lexer + parser + codegen = `fj-stage1` | [x] |
| S4.1.5 | Test stage-1 on hello.fj | Verify output matches Rust `fj` | [x] |
| S4.1.6 | Test stage-1 on fibonacci.fj | Verify correctness | [x] |
| S4.1.7 | Test stage-1 on 10 examples | Verify all produce correct output | [x] |
| S4.1.8 | Fix divergences | Any difference from Rust compiler = bug | [x] |
| S4.1.9 | Stage-1 test suite | Automated comparison: `fj-stage1` vs `fj` | [x] |
| S4.1.10 | Document bootstrap process | Step-by-step build instructions | [x] |

#### Sprint S4.2: Stage 2 Bootstrap + Verification (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| S4.2.1 | Compile lexer.fj with `fj-stage1` | Stage-2 lexer | [x] |
| S4.2.2 | Compile parser.fj with `fj-stage1` | Stage-2 parser | [x] |
| S4.2.3 | Compile codegen.fj with `fj-stage1` | Stage-2 codegen | [x] |
| S4.2.4 | Link stage-2 compiler | `fj-stage2` | [x] |
| S4.2.5 | Verify: stage-1 output == stage-2 output | Fixed-point bootstrap | [x] |
| S4.2.6 | Binary reproducibility | Same input → byte-identical output | [x] |
| S4.2.7 | Fuzz stage-2 compiler | 60s fuzz run on self-hosted compiler | [x] |
| S4.2.8 | Performance: stage-2 vs Rust `fj` | Compilation speed comparison | [x] |
| S4.2.9 | Release `fj-selfhost` binary | Package self-hosted compiler | [x] |
| S4.2.10 | Blog: "Fajar Lang Compiles Itself" | Technical write-up | [x] |

---

## Option 2: GPU Compute Backend (6 sprints, 60 tasks)

**Goal:** tensor_matmul() runs on GPU via wgpu/Vulkan
**Effort:** ~12 hours

### Sprint G1.1: wgpu Device + Buffer (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| G1.1.1 | Enable `gpu` feature flag | `cargo build --features gpu` | [x] |
| G1.1.2 | GPU device initialization | wgpu::Instance → Adapter → Device → Queue | [x] |
| G1.1.3 | Buffer creation | Create GPU buffers from tensor data | [x] |
| G1.1.4 | CPU → GPU upload | Copy tensor f64 data to GPU buffer | [x] |
| G1.1.5 | GPU → CPU download | Read result buffer back to CPU | [x] |
| G1.1.6 | Buffer pool | Reuse buffers to avoid allocation overhead | [x] |
| G1.1.7 | Error handling | GPU errors → FjError::Gpu variant | [x] |
| G1.1.8 | Fallback detection | `gpu_available()` → bool | [x] |
| G1.1.9 | Device info | `gpu_info()` → name, memory, compute units | [x] |
| G1.1.10 | 10 GPU tests | Buffer create, upload, download, fallback | [x] |

### Sprint G1.2: Compute Pipeline (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| G1.2.1 | WGSL shader loading | Load .wgsl compute shaders at runtime | [x] |
| G1.2.2 | Pipeline creation | ComputePipeline from WGSL source | [x] |
| G1.2.3 | Bind group layout | Uniform + storage buffer bindings | [x] |
| G1.2.4 | Dispatch | `encoder.dispatch_workgroups(x, y, z)` | [x] |
| G1.2.5 | Synchronization | Wait for GPU completion | [x] |
| G1.2.6 | Shader cache | Cache compiled pipelines by name | [x] |
| G1.2.7 | Workgroup sizing | Auto-calculate optimal workgroup dimensions | [x] |
| G1.2.8 | Memory layout | Row-major f32 buffer for GPU, f64 for CPU | [x] |
| G1.2.9 | Precision handling | f64 (CPU) ↔ f32 (GPU) conversion | [x] |
| G1.2.10 | Benchmark: pipeline overhead | Measure dispatch latency | [x] |

### Sprint G2.1: WGSL Compute Kernels (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| G2.1.1 | vecadd.wgsl | Element-wise vector addition | [x] |
| G2.1.2 | matmul.wgsl | Matrix multiplication (tiled) | [x] |
| G2.1.3 | relu.wgsl | ReLU activation | [x] |
| G2.1.4 | sigmoid.wgsl | Sigmoid activation | [x] |
| G2.1.5 | softmax.wgsl | Softmax (reduce + normalize) | [x] |
| G2.1.6 | transpose.wgsl | Matrix transpose | [x] |
| G2.1.7 | scale.wgsl | Scalar multiplication | [x] |
| G2.1.8 | conv2d.wgsl | 2D convolution (im2col approach) | [x] |
| G2.1.9 | Verify all kernels | Compare GPU output vs CPU reference | [x] |
| G2.1.10 | Kernel benchmark suite | Time each kernel at various sizes | [x] |

### Sprint G2.2: Integration (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| G2.2.1 | Hook GPU into tensor_matmul | Auto-dispatch to GPU if available | [x] |
| G2.2.2 | Hook GPU into tensor_relu | GPU activation for large tensors | [x] |
| G2.2.3 | Hook GPU into tensor_softmax | GPU softmax | [x] |
| G2.2.4 | Size threshold | Only use GPU for tensors > 1024 elements | [x] |
| G2.2.5 | Multi-operation fusion | Chain ops without CPU roundtrip | [x] |
| G2.2.6 | Memory management | Track GPU allocations, prevent leaks | [x] |
| G2.2.7 | MNIST on GPU | Run MNIST inference with GPU acceleration | [x] |
| G2.2.8 | `gpu_benchmark` command | Compare CPU vs GPU for matmul at N=64,128,256,512 | [x] |
| G2.2.9 | Update examples | `examples/gpu_matmul.fj`, `examples/gpu_mnist.fj` | [x] |
| G2.2.10 | Documentation | GPU_COMPUTE.md — setup, usage, benchmarks | [x] |

### Sprint G3: Auto-Dispatch + Benchmarks (10 tasks each — 2 sprints)

| # | Task | Detail | Status |
|---|------|--------|--------|
| G3.1 | Auto-dispatch policy | CPU < 1K elements, GPU >= 1K | [x] |
| G3.2 | Runtime device selection | `@device` annotation routes to GPU | [x] |
| G3.3 | Mixed precision support | FP16 on GPU, FP64 on CPU | [x] |
| G3.4 | Multi-GPU support | Detect multiple GPUs, round-robin dispatch | [x] |
| G3.5 | Vulkan backend (via ash) | Alternative to wgpu for bare-metal | [x] |
| G3.6 | Q6A Adreno backend | OpenCL/Vulkan on Adreno 643 | [x] |
| G3.7 | Benchmark: matmul 64-1024 | CPU vs GPU speedup table | [x] |
| G3.8 | Benchmark: MNIST end-to-end | Full inference pipeline | [x] |
| G3.9 | Benchmark: training loop | Forward + backward + update on GPU | [x] |
| G3.10 | Release blog post | "GPU Compute in Fajar Lang" | [x] |

---

## Option 3: Package Registry (4 sprints, 40 tasks)

**Goal:** `fj publish` → registry, `fj add` → dependency resolution
**Effort:** ~8 hours

### Sprint P1: Registry Server (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| P1.1 | Create `registry/` project | Rust web server (axum or actix) | [x] |
| P1.2 | SQLite database schema | packages, versions, downloads, users | [x] |
| P1.3 | `POST /api/publish` | Upload package tarball + metadata | [x] |
| P1.4 | `GET /api/packages` | List packages with search | [x] |
| P1.5 | `GET /api/packages/:name` | Package details + versions | [x] |
| P1.6 | `GET /api/packages/:name/:version/download` | Download tarball | [x] |
| P1.7 | Package storage | Local filesystem or S3-compatible | [x] |
| P1.8 | API authentication | Token-based auth for publish | [x] |
| P1.9 | Rate limiting | Prevent abuse | [x] |
| P1.10 | Deploy to Cloudflare Workers or fly.io | Production deployment | [x] |

### Sprint P2: CLI Integration (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| P2.1 | `fj publish` command | Pack + upload to registry | [x] |
| P2.2 | `fj add <pkg>` command | Add dependency to fj.toml | [x] |
| P2.3 | `fj update` command | Update all dependencies to latest compatible | [x] |
| P2.4 | `fj remove <pkg>` command | Remove dependency | [x] |
| P2.5 | `fj search <query>` command | Search registry | [x] |
| P2.6 | `fj info <pkg>` command | Show package details | [x] |
| P2.7 | fj.toml `[dependencies]` section | Parse and resolve | [x] |
| P2.8 | fj.lock lockfile | Pin exact versions | [x] |
| P2.9 | `fj login` / `fj logout` | Registry authentication | [x] |
| P2.10 | 10 CLI tests | publish, add, update, search, info | [x] |

### Sprint P3: Dependency Resolution (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| P3.1 | Semver parsing | `^1.2.3`, `~1.2`, `>=1.0, <2.0`, `*` | [x] |
| P3.2 | Version compatibility | Cargo-style semver matching | [x] |
| P3.3 | PubGrub solver core | Conflict-driven clause learning | [x] |
| P3.4 | Dependency graph | Transitive dependency resolution | [x] |
| P3.5 | Cycle detection | Error on circular dependencies | [x] |
| P3.6 | Version conflict reporting | "pkg A requires X>=2, B requires X<2" | [x] |
| P3.7 | Offline mode | Use cached packages when offline | [x] |
| P3.8 | Workspace support | Multi-package projects | [x] |
| P3.9 | 10 resolution tests | Diamond deps, conflicts, cycles | [x] |
| P3.10 | Documentation | PACKAGES.md — how to create and publish | [x] |

### Sprint P4: Security + Standard Packages (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| P4.1 | Package signing | Ed25519 signatures on tarballs | [x] |
| P4.2 | Checksum verification | SHA-256 on download | [x] |
| P4.3 | Yanking | `fj yank <pkg> <version>` — mark version as broken | [x] |
| P4.4 | Audit trail | Log all publish/yank events | [x] |
| P4.5 | Publish fj-math | Standard math library | [x] |
| P4.6 | Publish fj-json | JSON parser/serializer | [x] |
| P4.7 | Publish fj-http | HTTP client/server | [x] |
| P4.8 | Publish fj-crypto | Cryptographic primitives | [x] |
| P4.9 | Publish fj-test | Testing framework | [x] |
| P4.10 | Registry web UI | Browse packages in browser | [x] |

---

## Option 4: Fajar Lang v0.9 (8 sprints, 80 tasks)

**Goal:** Advanced type system + performance
**Effort:** ~16 hours

### Phase T1: Generic Associated Types (2 sprints, 20 tasks)

#### Sprint T1.1: GAT Parsing + Analysis (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| T1.1.1 | Parse `type Item<'a>` in traits | Associated type with lifetime parameter | [x] |
| T1.1.2 | Parse GAT in impl blocks | `type Item<'a> = &'a T` | [x] |
| T1.1.3 | Analyzer: GAT resolution | Resolve `Self::Item<'a>` to concrete type | [x] |
| T1.1.4 | Analyzer: GAT bound checking | Verify GAT satisfies trait bounds | [x] |
| T1.1.5 | Iterator trait with GAT | `trait Iterator { type Item<'a>; fn next(&'a self) -> Option<Self::Item<'a>>; }` | [x] |
| T1.1.6 | LendingIterator pattern | Iterator that borrows from self | [x] |
| T1.1.7 | Interpreter: GAT dispatch | Resolve GAT at runtime | [x] |
| T1.1.8 | Codegen: GAT monomorphization | Specialize GAT per concrete type | [x] |
| T1.1.9 | 10 GAT tests | Basic, iterator, lending, bounds | [x] |
| T1.1.10 | GAT examples | `examples/gat_iterator.fj`, `examples/gat_lending.fj` | [x] |

#### Sprint T1.2: GAT Advanced + Patterns (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| T1.2.1 | Multiple GATs per trait | `type Key; type Value<'a>;` | [x] |
| T1.2.2 | GAT with type bounds | `type Item<'a>: Display + 'a` | [x] |
| T1.2.3 | GAT in where clauses | `where T: Iterator<Item<'a> = &'a str>` | [x] |
| T1.2.4 | GAT default types | `type Item<'a> = &'a Self` | [x] |
| T1.2.5 | Parser collection trait | `trait Collection { type Iter<'a>: Iterator; fn iter(&self) -> Self::Iter<'_>; }` | [x] |
| T1.2.6 | Monad-like pattern | `trait Functor { type Output<U>; fn map<U>(self, f: fn(T) -> U) -> Self::Output<U>; }` | [x] |
| T1.2.7 | GAT + async | `type Future<'a>: Future<Output = T>` | [x] |
| T1.2.8 | Error messages | Clear errors for GAT violations | [x] |
| T1.2.9 | 10 advanced GAT tests | Collections, functors, async | [x] |
| T1.2.10 | Update FAJAR_LANG_SPEC.md | Document GAT syntax and semantics | [x] |

### Phase T2: Effect System (2 sprints, 20 tasks)

#### Sprint T2.1: Effect Declaration + Checking (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| T2.1.1 | Parse `effect` keyword | `effect IO { fn read() -> str; fn write(s: str); }` | [x] |
| T2.1.2 | Parse effect annotation on fn | `fn foo() -> i32 ! IO` (may perform IO) | [x] |
| T2.1.3 | Parse `handle` block | `handle { risky() } with { IO::read => "mock" }` | [x] |
| T2.1.4 | Analyzer: effect tracking | Track which functions have which effects | [x] |
| T2.1.5 | Analyzer: effect propagation | Callee effects propagate to caller | [x] |
| T2.1.6 | Analyzer: effect checking | Error if unhandled effect | [x] |
| T2.1.7 | Built-in effects | `IO`, `Allocate`, `Panic`, `Async` | [x] |
| T2.1.8 | Pure functions | `fn pure_add(a: i32, b: i32) -> i32` — no effects allowed | [x] |
| T2.1.9 | 10 effect tests | Declaration, annotation, propagation, handling | [x] |
| T2.1.10 | Effect examples | `examples/effects_io.fj`, `examples/effects_pure.fj` | [x] |

#### Sprint T2.2: Effect Handlers + Algebraic Effects (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| T2.2.1 | Interpreter: effect dispatch | Handle effects at runtime via handler table | [x] |
| T2.2.2 | Resumption | Handler can resume computation after handling | [x] |
| T2.2.3 | Multi-shot continuations | Handler can resume multiple times | [x] |
| T2.2.4 | Effect composition | `fn foo() -> i32 ! IO + Allocate` | [x] |
| T2.2.5 | Effect polymorphism | `fn run<E>(f: fn() -> T ! E) -> T` | [x] |
| T2.2.6 | Exception as effect | `effect Exception { fn throw(msg: str) -> never; }` | [x] |
| T2.2.7 | State as effect | `effect State<S> { fn get() -> S; fn put(s: S); }` | [x] |
| T2.2.8 | Codegen: effect lowering | Effects → CPS transformation or exception tables | [x] |
| T2.2.9 | 10 algebraic effect tests | Resumption, multi-shot, state, exception | [x] |
| T2.2.10 | Blog: "Algebraic Effects in Fajar Lang" | Technical write-up | [x] |

### Phase T3: Comptime (2 sprints, 20 tasks)

#### Sprint T3.1: Compile-Time Evaluation (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| T3.1.1 | Parse `comptime { }` blocks | Compile-time evaluated code blocks | [x] |
| T3.1.2 | Parse `const fn` enhanced | Const functions that run at compile time | [x] |
| T3.1.3 | Const generics | `fn create_array<const N: usize>() -> [i32; N]` | [x] |
| T3.1.4 | Compile-time interpreter | Run subset of Fajar Lang at compile time | [x] |
| T3.1.5 | Const evaluation of expressions | `const X: i32 = 2 + 3` → evaluate at compile time | [x] |
| T3.1.6 | Const string operations | `const S: str = f"size_{N}"` | [x] |
| T3.1.7 | Const array generation | `const TABLE: [i32; 256] = comptime { generate_table() }` | [x] |
| T3.1.8 | Static assertions | `comptime { assert(size_of::<T>() <= 8) }` | [x] |
| T3.1.9 | 10 comptime tests | Blocks, const fn, const generics | [x] |
| T3.1.10 | Update spec | Document comptime syntax | [x] |

#### Sprint T3.2: Const Generics + Applications (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| T3.2.1 | Tensor shape as const generic | `Tensor<f32, [const M, const N]>` | [x] |
| T3.2.2 | Shape checking at compile time | `matmul(A: Tensor<M,K>, B: Tensor<K,N>) -> Tensor<M,N>` | [x] |
| T3.2.3 | Fixed-size arrays | `[T; N]` where N is const | [x] |
| T3.2.4 | Const arithmetic | `N + M`, `N * M` in type positions | [x] |
| T3.2.5 | Const-if | `if const N > 0 { ... }` — compile-time branching | [x] |
| T3.2.6 | Const loops | `for const i in 0..N { ... }` — unrolled at compile time | [x] |
| T3.2.7 | Build-time code generation | Generate lookup tables, dispatch tables | [x] |
| T3.2.8 | Embedded: const config | `const CLOCK_HZ: u32 = comptime { board_clock() }` | [x] |
| T3.2.9 | 10 const generic tests | Tensors, arrays, shape checking | [x] |
| T3.2.10 | Example: compile-time MNIST | Pre-compute weight matrices at compile time | [x] |

### Phase T4: SIMD Intrinsics (2 sprints, 20 tasks)

#### Sprint T4.1: SIMD Types + Operations (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| T4.1.1 | SIMD type: `f32x4` | 4-wide float vector | [x] |
| T4.1.2 | SIMD type: `f32x8` | 8-wide (AVX) | [x] |
| T4.1.3 | SIMD type: `i32x4`, `i32x8` | Integer vectors | [x] |
| T4.1.4 | SIMD arithmetic | `a + b`, `a * b`, `a - b` on vector types | [x] |
| T4.1.5 | SIMD load/store | `f32x4::load(ptr)`, `.store(ptr)` | [x] |
| T4.1.6 | SIMD shuffle | `a.shuffle(b, mask)` | [x] |
| T4.1.7 | SIMD reduce | `a.sum()`, `a.min()`, `a.max()` | [x] |
| T4.1.8 | SIMD comparison | `a == b`, `a < b` → mask | [x] |
| T4.1.9 | Auto-vectorization hints | `@simd` annotation on loops | [x] |
| T4.1.10 | 10 SIMD tests | Arithmetic, load/store, reduce, shuffle | [x] |

#### Sprint T4.2: SIMD Integration + Platforms (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| T4.2.1 | SSE4.2 backend | Map to x86 SSE intrinsics | [x] |
| T4.2.2 | AVX2 backend | Map to x86 AVX intrinsics | [x] |
| T4.2.3 | AVX-512 backend | Map to x86 AVX-512 intrinsics | [x] |
| T4.2.4 | NEON backend | Map to ARM64 NEON intrinsics | [x] |
| T4.2.5 | SVE backend | Map to ARM SVE (variable-width) | [x] |
| T4.2.6 | SIMD tensor matmul | Vectorized inner loop for matmul | [x] |
| T4.2.7 | SIMD activation functions | Vectorized relu, sigmoid, tanh | [x] |
| T4.2.8 | Benchmark: scalar vs SIMD | Speedup comparison for tensor ops | [x] |
| T4.2.9 | Example: SIMD neural network | Vectorized forward pass | [x] |
| T4.2.10 | Documentation | SIMD_GUIDE.md — types, operations, platforms | [x] |

---

## Option 5: Q6A Full Deploy (3 sprints, 28 tasks) — BLOCKED

**Status:** Board offline (user di luar rumah)
**Goal:** Deploy v6.1.0 with 38 new methods to Dragon Q6A

*(Same as Plan V3/V4 Option 2 — execute when Q6A available)*

---

## Option 6: Nova v2.0 "Phoenix" (14 sprints, 140 tasks)

**Goal:** GUI, audio, real persistence, POSIX compliance
**Effort:** ~28 hours

### Phase N1: GUI Framework (4 sprints, 40 tasks)

#### Sprint N1.1: Framebuffer + Primitives (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| N1.1.1 | VirtIO-GPU framebuffer init | 640x480x32bpp via VirtIO-GPU | [ ] |
| N1.1.2 | Pixel drawing | `draw_pixel(x, y, color)` | [ ] |
| N1.1.3 | Line drawing | Bresenham's algorithm | [ ] |
| N1.1.4 | Rectangle | `fill_rect()`, `draw_rect()` | [ ] |
| N1.1.5 | Circle | Midpoint circle algorithm | [ ] |
| N1.1.6 | Font rendering | 8x16 bitmap font, `draw_char()`, `draw_text()` | [ ] |
| N1.1.7 | Double buffering | Back buffer → front buffer swap | [ ] |
| N1.1.8 | Color palette | 16 named colors + RGB(r,g,b) | [ ] |
| N1.1.9 | Screen clear | `clear_screen(color)` | [ ] |
| N1.1.10 | Demo: bouncing ball | Animate a ball on screen | [ ] |

#### Sprint N1.2: Window Manager (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| N1.2.1 | Window struct | x, y, width, height, title, z-order | [ ] |
| N1.2.2 | Window creation | `create_window(title, x, y, w, h)` | [ ] |
| N1.2.3 | Window rendering | Title bar + client area + border | [ ] |
| N1.2.4 | Window stacking | Z-order management (raise/lower) | [ ] |
| N1.2.5 | Window moving | Click title bar + drag | [ ] |
| N1.2.6 | Window close | Close button | [ ] |
| N1.2.7 | Desktop background | Solid color or gradient | [ ] |
| N1.2.8 | Taskbar | Bottom bar with window list | [ ] |
| N1.2.9 | Mouse cursor | Hardware or software cursor rendering | [ ] |
| N1.2.10 | Demo: 3 windows | Show multiple overlapping windows | [ ] |

#### Sprint N1.3: Widget Toolkit (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| N1.3.1 | Button widget | Click handler, hover state | [ ] |
| N1.3.2 | Label widget | Text display | [ ] |
| N1.3.3 | TextInput widget | Editable text field, cursor | [ ] |
| N1.3.4 | Checkbox widget | Toggle on/off | [ ] |
| N1.3.5 | ListView widget | Scrollable list | [ ] |
| N1.3.6 | Layout: vertical stack | Stack widgets vertically | [ ] |
| N1.3.7 | Layout: horizontal stack | Stack widgets horizontally | [ ] |
| N1.3.8 | Event system | Click, key, mouse move → widget dispatch | [ ] |
| N1.3.9 | Focus management | Tab between widgets | [ ] |
| N1.3.10 | Demo: calculator app | GUI calculator with buttons | [ ] |

#### Sprint N1.4: Applications (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| N1.4.1 | Terminal emulator | GUI window running the shell | [ ] |
| N1.4.2 | File manager | List/navigate directories | [ ] |
| N1.4.3 | Text editor | Basic editing with syntax highlighting | [ ] |
| N1.4.4 | System monitor | CPU, memory, process list (graphical) | [ ] |
| N1.4.5 | Image viewer | Display raw bitmap images | [ ] |
| N1.4.6 | Settings app | Change hostname, colors, resolution | [ ] |
| N1.4.7 | `startx` command | Switch from text to GUI mode | [ ] |
| N1.4.8 | Screenshot command | Capture framebuffer to file | [ ] |
| N1.4.9 | QEMU `-device virtio-gpu-pci` test | Verify GUI in QEMU | [ ] |
| N1.4.10 | Blog: "GUI in Fajar Lang" | Screenshots + code walkthrough | [ ] |

### Phase N2: Audio Driver (2 sprints, 20 tasks)

*(10 tasks each: Intel HDA detection/init, PCM format, mixer, playback, system sounds)*

### Phase N3: Real Persistence (3 sprints, 30 tasks)

*(30 tasks: ext2 full journaling, boot from disk, GRUB integration, filesystem repair)*

### Phase N4: POSIX v2 (3 sprints, 30 tasks)

*(30 tasks: mmap file-backed, select/poll, pipe v3, /proc/PID/, signal queue)*

### Phase N5: Networking v4 (2 sprints, 20 tasks)

*(20 tasks: DHCP v2, NTP time sync, multicast, IPv6 stub, HTTP/2 stub)*

---

## Option 7: Education Platform (4 sprints, 40 tasks)

**Goal:** Interactive tutorial, playground, course material
**Effort:** ~8 hours

### Sprint ED1: Interactive Tutorial (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| ED1.1 | Tutorial framework | Step-by-step lessons with code editor | [ ] |
| ED1.2 | Lesson 1: Hello World | Variables, functions, println | [ ] |
| ED1.3 | Lesson 2: Control Flow | if/else, while, for, match | [ ] |
| ED1.4 | Lesson 3: Data Structures | Arrays, structs, enums, maps | [ ] |
| ED1.5 | Lesson 4: Functions | Closures, higher-order, pipeline | [ ] |
| ED1.6 | Lesson 5: Error Handling | Option, Result, ? operator | [ ] |
| ED1.7 | Lesson 6: Traits | Trait definition, impl, polymorphism | [ ] |
| ED1.8 | Lesson 7: Async | async/await, join, spawn | [ ] |
| ED1.9 | Lesson 8: OS Development | @kernel, volatile, interrupt handlers | [ ] |
| ED1.10 | Lesson 9: ML | Tensors, autograd, training loop | [ ] |

### Sprint ED2-ED4: Playground, Course, Community (30 tasks)

*(Playground: WebAssembly REPL in browser, shareable links. Course: university-level curriculum. Community: Discord, forum, contributor guide)*

---

## Option 8: Benchmarks Suite (3 sprints, 30 tasks)

**Goal:** Formal benchmarks vs Rust, C, Python, Zig
**Effort:** ~6 hours

### Sprint B1: Microbenchmarks (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| B1.1 | Fibonacci (recursive) | fj vs Rust vs C vs Python | [ ] |
| B1.2 | Fibonacci (iterative) | Same comparison | [ ] |
| B1.3 | Array sort (quicksort) | N=10K, 100K, 1M elements | [ ] |
| B1.4 | String concatenation | 10K, 100K iterations | [ ] |
| B1.5 | HashMap insert/lookup | 10K, 100K entries | [ ] |
| B1.6 | Matrix multiply | 64x64, 128x128, 256x256 | [ ] |
| B1.7 | Tokenize source file | Lex 10K lines of code | [ ] |
| B1.8 | Pattern matching | Deep match with 100 branches | [ ] |
| B1.9 | Closure overhead | 1M closure calls | [ ] |
| B1.10 | Compile time | Time to compile 5K line program | [ ] |

### Sprint B2: Application Benchmarks (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| B2.1 | Binary trees | Allocate/deallocate tree nodes | [ ] |
| B2.2 | N-body simulation | Gravitational physics | [ ] |
| B2.3 | Mandelbrot set | Fractal computation | [ ] |
| B2.4 | JSON parsing | Parse 1MB JSON file | [ ] |
| B2.5 | HTTP server throughput | Requests per second | [ ] |
| B2.6 | MNIST training | Time to train 1 epoch | [ ] |
| B2.7 | Regular expression | Match patterns in 1MB text | [ ] |
| B2.8 | File I/O | Read/write 100MB file | [ ] |
| B2.9 | Concurrency | Channel throughput, mutex contention | [ ] |
| B2.10 | Memory usage | Peak RSS for each benchmark | [ ] |

### Sprint B3: Reporting + CI (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| B3.1 | Benchmark harness | Automated runner with warm-up, iterations | [ ] |
| B3.2 | Statistical analysis | Mean, median, stddev, percentiles | [ ] |
| B3.3 | Comparison charts | Bar charts: fj vs Rust vs C vs Python | [ ] |
| B3.4 | CI integration | Run benchmarks on each release | [ ] |
| B3.5 | Historical tracking | Track performance across versions | [ ] |
| B3.6 | Regression detection | Alert if benchmark degrades > 10% | [ ] |
| B3.7 | BENCHMARKS.md | Formatted results table | [ ] |
| B3.8 | Website benchmark page | Public benchmark results | [ ] |
| B3.9 | Blog: "Fajar Lang Performance" | Analysis and comparison | [ ] |
| B3.10 | Optimization guide | Tips for writing fast Fajar Lang code | [ ] |

---

## Execution Order Recommendation

```
Phase 1 — Language Polish:
  4  → Fajar Lang v0.9 (GATs, effects, comptime, SIMD)     ~16 hrs
  8  → Benchmarks Suite (validate performance)               ~6 hrs

Phase 2 — Ecosystem:
  3  → Package Registry (community growth)                   ~8 hrs
  7  → Education Platform (adoption)                         ~8 hrs

Phase 3 — Performance:
  2  → GPU Compute Backend (tensor acceleration)             ~12 hrs

Phase 4 — Validation:
  1  → Self-Hosting Compiler (ultimate proof)                ~20 hrs

Phase 5 — OS:
  6  → Nova v2.0 "Phoenix" (GUI, audio, persistence)        ~28 hrs

Phase 6 — Hardware:
  5  → Q6A Deploy (when board available)                     ~6 hrs
```

**Total: 52 sprints, 518 tasks, ~104 hours**

---

## Summary

```
Option 1:  Self-Hosting Compiler    10 sprints  100 tasks   ~20 hrs
Option 2:  GPU Compute Backend       6 sprints   60 tasks   ~12 hrs
Option 3:  Package Registry          4 sprints   40 tasks    ~8 hrs
Option 4:  Fajar Lang v0.9          8 sprints   80 tasks   ~16 hrs
Option 5:  Q6A Deploy                3 sprints   28 tasks    ~6 hrs  BLOCKED
Option 6:  Nova v2.0 "Phoenix"     14 sprints  140 tasks   ~28 hrs
Option 7:  Education Platform        4 sprints   40 tasks    ~8 hrs
Option 8:  Benchmarks Suite          3 sprints   30 tasks    ~6 hrs

Total:     52 sprints, 518 tasks, ~104 hours
```

---

*Next Steps Implementation Plan V5 — Fajar Lang v6.1.0 + FajarOS Nova v2.0.0*
*Built with Fajar Lang + Claude Opus 4.6*
