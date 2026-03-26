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

### Sprint Q5.1: Cross-compile + Deploy (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| Q5.1.1 | Cross-compile v6.1.0 | `cargo build --release --target aarch64-unknown-linux-gnu` | [x] |
| Q5.1.2 | Deploy to Q6A | SCP binary via SSH to radxa@192.168.50.94 | [x] |
| Q5.1.3 | Verify JIT on ARM64 | Run fibonacci, pattern matching, traits | [x] |
| Q5.1.4 | Verify AOT on ARM64 | Compile native aarch64 ELF, run | [x] |
| Q5.1.5 | Test 38 new methods | Iterator, f-string, trait object methods | [x] |
| Q5.1.6 | Test REPL on ARM64 | Interactive REPL session | [x] |
| Q5.1.7 | Test fj test runner | @test annotation execution | [x] |
| Q5.1.8 | Test fj doc gen | Documentation generation | [x] |
| Q5.1.9 | Test fj watch | File watcher rebuild | [x] |
| Q5.1.10 | ARM64 benchmark suite | interpreter/JIT/AOT comparison | [x] |

### Sprint Q5.2: Hardware Features (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| Q5.2.1 | Vulkan compute | GPU matmul/relu on Adreno 643 | [x] |
| Q5.2.2 | QNN CPU inference | INT8 MNIST on Hexagon CPU backend | [x] |
| Q5.2.3 | QNN GPU inference | FP32 MNIST on Adreno GPU backend | [x] |
| Q5.2.4 | GPIO blink | LED blink on pin 96 | [x] |
| Q5.2.5 | NVMe performance | Read/write throughput test | [x] |
| Q5.2.6 | OpenCL test | gpu_matmul/gpu_relu via OpenCL | [x] |
| Q5.2.7 | Camera test | MIPI CSI frame capture | [x] |
| Q5.2.8 | Thermal monitoring | CPU temp under load | [x] |
| Q5.2.9 | I2C/SPI sensors | Read accelerometer/gyroscope | [x] |
| Q5.2.10 | Model export pipeline | ONNX→QNN DLC→deploy→inference | [x] |

### Sprint Q5.3: Full Verification + Docs (8 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| Q5.3.1 | Run all 106 examples | Verify 106/106 pass on Q6A | [x] |
| Q5.3.2 | Run all 55 Q6A examples | Verify Q6A-specific examples | [x] |
| Q5.3.3 | Multi-accelerator test | CPU→GPU→NPU dispatch chain | [x] |
| Q5.3.4 | 24-hour stress test | Continuous inference stability | [x] |
| Q5.3.5 | Power consumption | Idle/load power measurements | [x] |
| Q5.3.6 | ARM64 vs x86_64 report | Performance comparison table | [x] |
| Q5.3.7 | Q6A documentation update | Update Q6A_*.md for v6.1.0 | [x] |
| Q5.3.8 | Release blog post | Blog post for Q6A deployment | [x] |

---

## Option 6: Nova v2.0 "Phoenix" (14 sprints, 140 tasks) ✅ COMPLETE

**Goal:** GUI, audio, real persistence, POSIX compliance
**Effort:** ~28 hours
**Files:** `examples/nova_phoenix_{gui,audio,persist,posix,net}.fj` + `docs/NOVA_PHOENIX.md`

### Phase N1: GUI Framework (4 sprints, 40 tasks) ✅

#### Sprint N1.1: Framebuffer + Primitives (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| N1.1.1 | VirtIO-GPU framebuffer init | 640x480x32bpp via VirtIO-GPU | [x] |
| N1.1.2 | Pixel drawing | `draw_pixel(x, y, color)` | [x] |
| N1.1.3 | Line drawing | Bresenham's algorithm | [x] |
| N1.1.4 | Rectangle | `fill_rect()`, `draw_rect()` | [x] |
| N1.1.5 | Circle | Midpoint circle algorithm | [x] |
| N1.1.6 | Font rendering | 8x16 bitmap font, `draw_char()`, `draw_text()` | [x] |
| N1.1.7 | Double buffering | Back buffer → front buffer swap | [x] |
| N1.1.8 | Color palette | 16 named colors + RGB(r,g,b) | [x] |
| N1.1.9 | Screen clear | `clear_screen(color)` | [x] |
| N1.1.10 | Demo: bouncing ball | Animate a ball on screen | [x] |

#### Sprint N1.2: Window Manager (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| N1.2.1 | Window struct | x, y, width, height, title, z-order | [x] |
| N1.2.2 | Window creation | `create_window(title, x, y, w, h)` | [x] |
| N1.2.3 | Window rendering | Title bar + client area + border | [x] |
| N1.2.4 | Window stacking | Z-order management (raise/lower) | [x] |
| N1.2.5 | Window moving | Click title bar + drag | [x] |
| N1.2.6 | Window close | Close button | [x] |
| N1.2.7 | Desktop background | Solid color or gradient | [x] |
| N1.2.8 | Taskbar | Bottom bar with window list | [x] |
| N1.2.9 | Mouse cursor | Hardware or software cursor rendering | [x] |
| N1.2.10 | Demo: 3 windows | Show multiple overlapping windows | [x] |

#### Sprint N1.3: Widget Toolkit (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| N1.3.1 | Button widget | Click handler, hover state | [x] |
| N1.3.2 | Label widget | Text display | [x] |
| N1.3.3 | TextInput widget | Editable text field, cursor | [x] |
| N1.3.4 | Checkbox widget | Toggle on/off | [x] |
| N1.3.5 | ListView widget | Scrollable list | [x] |
| N1.3.6 | Layout: vertical stack | Stack widgets vertically | [x] |
| N1.3.7 | Layout: horizontal stack | Stack widgets horizontally | [x] |
| N1.3.8 | Event system | Click, key, mouse move → widget dispatch | [x] |
| N1.3.9 | Focus management | Tab between widgets | [x] |
| N1.3.10 | Demo: calculator app | GUI calculator with buttons | [x] |

#### Sprint N1.4: Applications (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| N1.4.1 | Terminal emulator | GUI window running the shell | [x] |
| N1.4.2 | File manager | List/navigate directories | [x] |
| N1.4.3 | Text editor | Basic editing with syntax highlighting | [x] |
| N1.4.4 | System monitor | CPU, memory, process list (graphical) | [x] |
| N1.4.5 | Image viewer | Display raw bitmap images | [x] |
| N1.4.6 | Settings app | Change hostname, colors, resolution | [x] |
| N1.4.7 | `startx` command | Switch from text to GUI mode | [x] |
| N1.4.8 | Screenshot command | Capture framebuffer to file | [x] |
| N1.4.9 | QEMU `-device virtio-gpu-pci` test | Verify GUI in QEMU | [x] |
| N1.4.10 | Blog: "GUI in Fajar Lang" | Screenshots + code walkthrough | [x] |

### Phase N2: Audio Driver (2 sprints, 20 tasks) ✅

#### Sprint N2.1: Intel HDA Controller (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| N2.1.1 | PCI HDA detection | Scan bus for class 0x04 subclass 0x03 | [x] |
| N2.1.2 | Controller reset + init | GCTL reset, CORB/RIRB setup | [x] |
| N2.1.3 | Codec communication | CORB/RIRB verb send/receive | [x] |
| N2.1.4 | Codec enumeration | Walk function groups + audio widgets | [x] |
| N2.1.5 | DAC configuration | Stream/channel + converter format | [x] |
| N2.1.6 | Pin configuration | Output enable, EAPD | [x] |
| N2.1.7 | Stream descriptor setup | BDL, CBL, LVI, format | [x] |
| N2.1.8 | DMA buffer allocation | 256KB ring buffer | [x] |
| N2.1.9 | Stream start/stop | RUN bit, IOCE | [x] |
| N2.1.10 | IRQ handler | Buffer completion, status clear | [x] |

#### Sprint N2.2: PCM + Mixer + Sounds (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| N2.2.1 | PCM format support | 16/24-bit, 44.1/48kHz, stereo | [x] |
| N2.2.2 | Mixer init | Master, PCM, system channels | [x] |
| N2.2.3 | Volume control | 0-100 → HDA gain 0-63 | [x] |
| N2.2.4 | Mute toggle | Hardware mute via amp gain | [x] |
| N2.2.5 | Sine wave generator | Triangle approximation | [x] |
| N2.2.6 | Square wave beep | Configurable frequency + duration | [x] |
| N2.2.7 | System sounds | Startup, error, notification, click, shutdown | [x] |
| N2.2.8 | WAV file parser | RIFF header, PCM data extraction | [x] |
| N2.2.9 | WAV playback | Stream WAV to DMA buffer | [x] |
| N2.2.10 | Shell commands | audio_init, volume, mute, beep, play | [x] |

### Phase N3: Real Persistence (3 sprints, 30 tasks) ✅

#### Sprint N3.1: ext2 Journal (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| N3.1.1 | Journal superblock | JBD2-compatible header | [x] |
| N3.1.2 | Begin transaction | Allocate txn state | [x] |
| N3.1.3 | Add block to txn | Collect dirty blocks (max 64) | [x] |
| N3.1.4 | Descriptor block | Write block tags | [x] |
| N3.1.5 | Data blocks to journal | Sequential journal writes | [x] |
| N3.1.6 | Commit block | CRC32 checksum | [x] |
| N3.1.7 | Write final locations | Blocks to actual disk positions | [x] |
| N3.1.8 | Journal recovery | Scan + replay after crash | [x] |
| N3.1.9 | Journal CRC verification | Validate commit integrity | [x] |
| N3.1.10 | Journal head/tail management | Circular buffer wrap | [x] |

#### Sprint N3.2: ext2 Core Operations (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| N3.2.1 | Superblock read | Validate EXT2_MAGIC | [x] |
| N3.2.2 | Group descriptor parse | Inode table, bitmap locations | [x] |
| N3.2.3 | Inode read | Direct + indirect + double indirect | [x] |
| N3.2.4 | File read | Block-by-block with inode mapping | [x] |
| N3.2.5 | File write (journaled) | Transaction-wrapped block writes | [x] |
| N3.2.6 | Inode update | Size, timestamps via journal | [x] |
| N3.2.7 | NVMe read wrapper | Simplified command submission | [x] |
| N3.2.8 | NVMe write wrapper | Simplified command submission | [x] |
| N3.2.9 | Multiboot2 header check | Verify magic + checksum | [x] |
| N3.2.10 | GRUB config generation | grub.cfg with menuentry | [x] |

#### Sprint N3.3: fsck (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| N3.3.1 | fsck superblock check | Magic, block count consistency | [x] |
| N3.3.2 | Journal replay on dirty | Auto-recovery on unclean mount | [x] |
| N3.3.3 | Inode table scan | Orphan detection, link count | [x] |
| N3.3.4 | Size vs blocks check | Detect over-allocated inodes | [x] |
| N3.3.5 | Block bitmap audit | Count free per group | [x] |
| N3.3.6 | Directory structure check | Verify `.` and `..` entries | [x] |
| N3.3.7 | Root inode validation | Inode 2 must be directory | [x] |
| N3.3.8 | Mark filesystem clean | Write SB_STATE = 1 | [x] |
| N3.3.9 | Error/fixed counters | Track issues found + repaired | [x] |
| N3.3.10 | Boot-time fsck | Auto-run on mount | [x] |

### Phase N4: POSIX v2 (3 sprints, 30 tasks) ✅

#### Sprint N4.1: mmap (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| N4.1.1 | VMA table (32 entries) | Start, end, prot, flags, fd, offset | [x] |
| N4.1.2 | sys_mmap | Anonymous + file-backed mapping | [x] |
| N4.1.3 | sys_munmap | Unmap + free physical pages | [x] |
| N4.1.4 | sys_msync | Write-back shared mappings | [x] |
| N4.1.5 | Page fault handler | Demand paging for file-backed mmap | [x] |
| N4.1.6 | Free region finder | Gap search from 0x40000000 | [x] |
| N4.1.7 | 4-level page table walk | PML4 → PDPT → PD → PT mapping | [x] |
| N4.1.8 | TLB invalidation | INVLPG per unmapped page | [x] |
| N4.1.9 | Virt-to-phys translation | Walk page table for address | [x] |
| N4.1.10 | MAP_FIXED support | Caller-specified address | [x] |

#### Sprint N4.2: select/poll + Pipe v3 (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| N4.2.1 | sys_poll | Poll fds for POLLIN/POLLOUT/POLLERR | [x] |
| N4.2.2 | sys_select | Convert to poll internally | [x] |
| N4.2.3 | Pipe/socket/file status | Type-specific readiness checks | [x] |
| N4.2.4 | Timeout handling | PIT tick comparison | [x] |
| N4.2.5 | sys_pipe2 | 64KB buffer, reader/writer refcount | [x] |
| N4.2.6 | sys_mkfifo | Named pipe creation | [x] |
| N4.2.7 | pipe_read | Blocking with wake-on-data | [x] |
| N4.2.8 | pipe_write | Blocking with wake-on-space | [x] |
| N4.2.9 | EOF detection | Writer close → reader gets 0 | [x] |
| N4.2.10 | FD allocation | Type-tagged file descriptors | [x] |

#### Sprint N4.3: /proc + Signals (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| N4.3.1 | /proc/cpuinfo | Model, MHz, cores, flags | [x] |
| N4.3.2 | /proc/meminfo | Total, free, available | [x] |
| N4.3.3 | /proc/uptime | Seconds since boot | [x] |
| N4.3.4 | /proc/version | OS version string | [x] |
| N4.3.5 | /proc/<pid>/status | Name, state, PID, PPid | [x] |
| N4.3.6 | sys_sigaction | Register handler + flags | [x] |
| N4.3.7 | sys_sigprocmask | Block/unblock signals | [x] |
| N4.3.8 | sys_kill | Send signal + enqueue | [x] |
| N4.3.9 | Signal delivery | Default actions + user handlers | [x] |
| N4.3.10 | Signal queue (16 deep) | Sender PID, sigval | [x] |

### Phase N5: Networking v4 (2 sprints, 20 tasks) ✅

#### Sprint N5.1: DHCP v2 + NTP (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| N5.1.1 | DHCP state machine | INIT→SELECTING→REQUESTING→BOUND→RENEWING | [x] |
| N5.1.2 | DHCP Discover | Broadcast with options 53, 55, 12 | [x] |
| N5.1.3 | DHCP Offer handling | Parse offered IP + options | [x] |
| N5.1.4 | DHCP Request | Unicast with options 50, 54 | [x] |
| N5.1.5 | DHCP ACK → configure | Set IP, mask, gateway, DNS | [x] |
| N5.1.6 | Lease renewal timer | T1 (50%), T2 (87.5%) | [x] |
| N5.1.7 | DHCP option parser | Subnet, router, DNS, domain, lease | [x] |
| N5.1.8 | NTP query | RFC 5905 client, T1/T2/T3/T4 | [x] |
| N5.1.9 | NTP offset calculation | `((T2-T1)+(T3-T4))/2` | [x] |
| N5.1.10 | System clock sync | Adjust clock from NTP response | [x] |

#### Sprint N5.2: Multicast + IPv6 + HTTP/2 (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| N5.2.1 | IGMP join | Send Membership Report v2 | [x] |
| N5.2.2 | IGMP leave | Send Leave to 224.0.0.2 | [x] |
| N5.2.3 | Multicast MAC filter | `01:00:5E` + lower 23 bits | [x] |
| N5.2.4 | Multicast group table | 8 groups | [x] |
| N5.2.5 | IPv6 link-local address | EUI-64 from MAC | [x] |
| N5.2.6 | NDP Neighbor Solicitation | DAD for address verification | [x] |
| N5.2.7 | HTTP/2 connection preface | `PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n` | [x] |
| N5.2.8 | HTTP/2 frame parser | 9-byte header + type dispatch | [x] |
| N5.2.9 | HTTP/2 SETTINGS exchange | Parse + ACK settings | [x] |
| N5.2.10 | HTTP/2 PING/GOAWAY/WINDOW_UPDATE | Control frame handling | [x] |

---

## Option 7: Education Platform (4 sprints, 40 tasks)

**Goal:** Interactive tutorial, playground, course material
**Effort:** ~8 hours

### Sprint ED1: Interactive Tutorial (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| ED1.1 | Tutorial framework | Step-by-step lessons with code editor | [x] |
| ED1.2 | Lesson 1: Hello World | Variables, functions, println | [x] |
| ED1.3 | Lesson 2: Control Flow | if/else, while, for, match | [x] |
| ED1.4 | Lesson 3: Data Structures | Arrays, structs, enums, maps | [x] |
| ED1.5 | Lesson 4: Functions | Closures, higher-order, pipeline | [x] |
| ED1.6 | Lesson 5: Error Handling | Option, Result, ? operator | [x] |
| ED1.7 | Lesson 6: Traits | Trait definition, impl, polymorphism | [x] |
| ED1.8 | Lesson 7: Async | async/await, join, spawn | [x] |
| ED1.9 | Lesson 8: OS Development | @kernel, volatile, interrupt handlers | [x] |
| ED1.10 | Lesson 9: ML | Tensors, autograd, training loop | [x] |

### Sprint ED2: Playground (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| ED2.1 | WebAssembly compiler | Compile Fajar Lang to Wasm in browser | [x] |
| ED2.2 | Monaco editor setup | Syntax highlighting, auto-indent | [x] |
| ED2.3 | Output panel | Show println output and errors | [x] |
| ED2.4 | Share via URL | Encode source in URL hash | [x] |
| ED2.5 | Example gallery | Browse and run example programs | [x] |
| ED2.6 | Dark/light theme | Toggle between themes | [x] |
| ED2.7 | Mobile responsive | Work on tablet/phone screens | [x] |
| ED2.8 | Keyboard shortcuts | Ctrl+Enter to run, Ctrl+S to format | [x] |
| ED2.9 | Error highlighting | Inline error markers in editor | [x] |
| ED2.10 | Playground deployment | Deploy to GitHub Pages / Vercel | [x] |

### Sprint ED3: University Course (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| ED3.1 | Course syllabus | 14-week CS course outline | [x] |
| ED3.2 | Week 1-2: Basics | Variables, types, control flow, functions | [x] |
| ED3.3 | Week 3-4: Data structures | Arrays, structs, enums, HashMap | [x] |
| ED3.4 | Week 5-6: Memory | Ownership, borrowing, move semantics | [x] |
| ED3.5 | Week 7-8: Generics/Traits | Type parameters, trait bounds, impl | [x] |
| ED3.6 | Week 9-10: Concurrency | Threads, channels, async/await | [x] |
| ED3.7 | Week 11-12: ML | Tensors, autograd, training loop, MNIST | [x] |
| ED3.8 | Week 13-14: OS | @kernel, bare-metal, interrupts, scheduling | [x] |
| ED3.9 | Assignments | 7 programming assignments with auto-grading | [x] |
| ED3.10 | Final project | Capstone project options (CLI, ML, OS) | [x] |

### Sprint ED4: Community (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| ED4.1 | Discord server | Channels: general, help, showcase, dev | [x] |
| ED4.2 | Forum setup | GitHub Discussions or Discourse | [x] |
| ED4.3 | Contributing guide | CONTRIBUTING.md with PR process | [x] |
| ED4.4 | Code of Conduct | Contributor Covenant adoption | [x] |
| ED4.5 | Issue templates | Bug report, feature request, RFC | [x] |
| ED4.6 | Good first issues | 10 labeled starter issues | [x] |
| ED4.7 | Style guide | Official Fajar Lang coding style | [x] |
| ED4.8 | Branding assets | Logo, colors, fonts, guidelines | [x] |
| ED4.9 | Social media | Twitter/X, LinkedIn presence | [x] |
| ED4.10 | Community documentation | Governance model, decision process | [x] |

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
