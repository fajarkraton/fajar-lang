# Fajar Lang (`fj`)

A statically-typed systems programming language for embedded ML and OS integration.

**The only language where an OS kernel and a neural network can share the same codebase, type system, and compiler.**

[![CI](https://github.com/fajarkraton/fajar-lang/actions/workflows/ci.yml/badge.svg)](https://github.com/fajarkraton/fajar-lang/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/fajarkraton/fajar-lang)](https://github.com/fajarkraton/fajar-lang/releases)

## Features

- **Dual-context safety** -- `@kernel` disables heap/tensor, `@device` disables raw pointers. Compiler enforces domain isolation.
- **Native tensor types** -- `Tensor` is a first-class citizen in the type system with shape checking.
- **Rust-inspired syntax** -- Familiar to Rust/C developers, but simpler (no lifetime annotations).
- **Dual compilation backends** -- Cranelift JIT/AOT + LLVM backend (O0-O3 optimization, JIT, object/assembly emission).
- **Concurrency** -- Threads, channels, mutexes, atomics, async/await with work-stealing executor.
- **Pipeline operator** -- `x |> f |> g` for functional-style data flow.
- **Pattern matching** -- Exhaustive `match` on enums, structs, tuples with generic `Option<T>` / `Result<T,E>`.
- **Context annotations** -- `@safe`, `@kernel`, `@device`, `@unsafe` for compile-time safety guarantees.
- **Built-in ML runtime** -- Autograd, tensor ops, optimizers (SGD, Adam), loss functions, MNIST training.
- **OS primitives** -- Memory management, IRQ handling, syscall dispatch, inline assembly, bare metal output.
- **Self-hosting** -- Lexer and parser written in Fajar Lang itself, bootstrapped and verified.
- **Beautiful errors** -- Rust-style diagnostics with source highlighting via miette.

## Quickstart

### Build from source

```bash
git clone https://github.com/fajarkraton/fajar-lang.git
cd fajar-lang
cargo build --release
```

The binary is at `target/release/fj`.

### Run a program

```bash
# Tree-walking interpreter (default)
fj run examples/hello.fj

# Native JIT compilation — Cranelift (380x faster)
fj run --native examples/native_hello.fj

# Native JIT compilation — LLVM (requires llvm-18-dev)
fj run --llvm examples/native_hello.fj

# Bytecode VM
fj run --vm examples/hello.fj
```

### Build with LLVM backend

```bash
# LLVM backend requires llvm-18-dev
sudo apt-get install llvm-18-dev libpolly-18-dev libzstd-dev

# Build with LLVM feature
cargo build --release --features llvm

# Run tests with LLVM
cargo test --features llvm
```

### Start the REPL

```bash
fj repl
```

## Example Programs

### Hello World

```fajar
fn main() {
    println("Hello from Fajar Lang!")
}
```

### Fibonacci (Native Codegen)

```fajar
fn fibonacci(n: i64) -> i64 {
    if n <= 1 {
        n
    } else {
        fibonacci(n - 1) + fibonacci(n - 2)
    }
}

fn main() -> i64 {
    fibonacci(30)
}
```

### MNIST Training

```fajar
fn forward(input: Tensor, w1: Tensor, w2: Tensor) -> Tensor {
    let hidden = tensor_relu(tensor_matmul(input, w1))
    tensor_softmax(tensor_matmul(hidden, w2))
}

fn main() {
    let w1 = tensor_xavier(4, 8)
    let w2 = tensor_xavier(8, 3)

    let mut epoch = 0
    while epoch < 5 {
        let input = tensor_rand(1, 4)
        let output = forward(input, w1, w2)
        let predicted = tensor_argmax(output)
        println("Predicted: " + to_string(predicted))
        epoch = epoch + 1
    }
}
```

### Context Annotations (Dual-Domain Safety)

```fajar
@kernel fn collect_sensor() -> [f32; 4] {
    // OS domain: raw memory access allowed, no heap/tensor
    let data = port_read!(0x3F00, 4)
    data
}

@device fn infer(x: Tensor) -> Tensor {
    // ML domain: tensor ops allowed, no raw pointers
    let w = zeros(4, 2)
    x @ w |> relu
}

@safe fn bridge() -> i64 {
    // Safe bridge: can call both @kernel and @device
    let raw = collect_sensor()
    let input = Tensor::from_slice(raw)
    let result = infer(input)
    result.argmax()
}
```

### Concurrency

```fajar
fn main() -> i64 {
    let m = Mutex::new(0)
    let t1 = thread_spawn(fn() -> i64 {
        mutex_lock(m)
        mutex_store(m, mutex_load(m) + 1)
        mutex_unlock(m)
        0
    })
    thread_join(t1)
    mutex_load(m)
}
```

## CLI Commands

| Command | Description |
|---------|-------------|
| `fj run <file.fj>` | Execute a Fajar Lang program (interpreter) |
| `fj run --native <file.fj>` | Execute with native JIT compilation |
| `fj run --vm <file.fj>` | Execute with bytecode VM |
| `fj repl` | Start interactive REPL |
| `fj check <file.fj>` | Parse and type-check (no execution) |
| `fj build` | Build current project |
| `fj new <name>` | Create a new project |
| `fj fmt <file.fj>` | Format source code |
| `fj dump-tokens <file.fj>` | Show lexer output |
| `fj dump-ast <file.fj>` | Show parser AST output |
| `fj test <file.fj>` | Run `@test` annotated functions |
| `fj doc <file.fj>` | Generate HTML documentation from `///` comments |
| `fj watch <file.fj>` | Watch files and auto-run on change |
| `fj bench <file.fj>` | Run micro-benchmarks on parameterless functions |
| `fj lsp` | Start Language Server Protocol server |

## Feature Matrix

| Category | Feature | Status |
|----------|---------|--------|
| **Frontend** | Lexer (82+ token kinds) | Working |
| | Parser (Pratt + recursive descent, 19 precedence levels) | Working |
| | Semantic analyzer (types, scope, context, NLL borrow) | Working |
| **Execution** | Tree-walking interpreter | Working |
| | Bytecode VM (45 opcodes) | Working |
| | Native compiler — Cranelift (JIT + AOT) | Working |
| | Native compiler — LLVM (JIT + AOT, O0-O3) | Working |
| | Cross-compilation (ARM64, RISC-V) | Working |
| **Type System** | Generics & monomorphization | Working |
| | Traits & static dispatch | Working |
| | Generic enums (`Option<T>`, `Result<T,E>`) | Working |
| | Move semantics & NLL borrow checker | Working |
| | Pattern matching (exhaustive `match`) | Working |
| **Concurrency** | Threads (spawn, join, Arc) | Working |
| | Channels (unbounded, bounded, close) | Working |
| | Synchronization (Mutex, RwLock, Condvar, Barrier) | Working |
| | Atomics (load, store, CAS, fence) | Working |
| | Async/await with work-stealing executor | Working |
| | Streams with map/filter/take combinators | Working |
| **ML Runtime** | Tensor ops (matmul, relu, softmax, etc.) | Working |
| | Autograd (tape-based reverse-mode) | Working |
| | Optimizers (SGD, Adam) | Working |
| | Layers (Dense, Conv2d, Attention, BatchNorm) | Working |
| | MNIST training (90%+ accuracy) | Working |
| | ONNX export, mixed precision (f16/bf16), INT8 quant | Working |
| | Distributed training (all_reduce, data parallelism) | Working |
| **OS Runtime** | Memory management (virtual/physical) | Working |
| | IRQ handling, syscall dispatch | Working |
| | Inline assembly (`asm!`, `global_asm!`) | Working |
| | Bare metal (`#[no_std]`, `@entry`, linker scripts) | Working |
| | Paging (x86_64, ARM64, RISC-V) | Working |
| **Safety** | Context annotations (@safe/@kernel/@device/@unsafe) | Working |
| | Ownership & move semantics | Working |
| | RAII / Drop trait (scope-level cleanup) | Working |
| | Integer overflow checking | Working |
| **FFI** | C interop (libloading, libffi) | Working |
| | SIMD (f32x4/f32x8/i32x4/i32x8) | Working |
| | Union/repr (@repr_c, @repr_packed, bitfields) | Working |
| **v0.5** | Test framework (`@test`, `@should_panic`, `@ignore`) | Working |
| | Doc comments (`///`) + `fj doc` HTML generation | Working |
| | Trait objects (`dyn Trait`, vtable dispatch) | Working |
| | Iterator protocol (`.map()`, `.filter()`, `.collect()`, etc.) | Working |
| | String interpolation (`f"Hello {name}"`) | Working |
| | Error recovery (multi-error, suggestions, hints) | Working |
| | `fj watch` (auto-run on file change) | Working |
| | `fj bench` (micro-benchmarks) | Working |
| **v0.6** | LLVM backend (inkwell, expressions, control flow, functions) | In Progress |
| | LLVM JIT execution + AOT object/assembly emission | Working |
| | LLVM optimization passes (O0-O3 via new pass manager) | Working |
| **Tools** | REPL (multi-line, `:type`, `:help`) | Working |
| | Formatter (`fj fmt`) | Working |
| | LSP server (diagnostics, completion, hover, rename) | Working |
| | Package manager (`fj.toml`, registry, `fj add`) | Working |
| | VS Code extension (syntax, snippets, LSP) | Working |
| **Self-Hosting** | Self-hosted lexer (stdlib/lexer.fj) | Working |
| | Self-hosted parser (shunting-yard) | Working |
| | Bootstrap verification (interpreter vs native) | Working |

## Project Structure

```
src/
  lib.rs              -- Module declarations + FjError
  main.rs             -- CLI entry point (clap)
  lexer/              -- Tokenization (82+ token kinds)
  parser/             -- AST generation (Pratt + recursive descent)
  analyzer/           -- Semantic analysis (types, scope, context, NLL borrow)
  interpreter/        -- Tree-walking evaluator
  vm/                 -- Bytecode compiler + virtual machine (45 opcodes)
  codegen/
    cranelift/        -- Cranelift backend (JIT + AOT, 150+ runtime fns)
      compile/        -- Expression, statement, control flow compilation
    llvm/             -- LLVM backend (inkwell, JIT + AOT, O0-O3)
  runtime/
    os/               -- Memory, IRQ, syscall, paging, GDT/IDT, serial, VGA
    ml/               -- Tensor, autograd, ops, optimizers, layers, ONNX
  formatter/          -- Code formatter
  lsp/                -- Language Server Protocol (tower-lsp)
  package/            -- Project manifest (fj.toml), registry
  stdlib/             -- Rust-side stdlib bindings
stdlib/               -- Fajar Lang stdlib (.fj source: core, nn, os, hal, drivers, lexer)
examples/             -- 24 example .fj programs
tests/                -- Integration tests (eval, ML, OS, autograd, property, safety, cross-compile)
benches/              -- Criterion benchmarks (interpreter, embedded, concurrency)
packages/             -- 7 standard packages (fj-math, fj-nn, fj-hal, fj-drivers, fj-http, fj-json, fj-crypto)
editors/vscode/       -- VS Code extension
book/                 -- mdBook documentation (40+ pages)
docs/                 -- 44 reference documents
```

## Stats

| Metric | Value |
|--------|-------|
| Rust LOC | ~101,000 |
| Tests | 1,767 default + 36 LLVM = 1,803+ (0 failures) |
| Examples | 24 `.fj` programs |
| Error codes | 71 across 9 categories |
| Documentation | 47 docs + 40-page mdBook |
| Standard packages | 7 |
| Codegen backends | 2 (Cranelift + LLVM) |

## Releases

| Version | Codename | Highlights |
|---------|----------|------------|
| v0.6.0 | Horizon | LLVM backend, debugger, BSP, registry, lifetimes, RTOS, advanced ML *(in progress)* |
| [v0.5.0](https://github.com/fajarkraton/fajar-lang/releases/tag/v0.5.0) | Ascendancy | Test framework, doc comments, trait objects, iterators, string interpolation, error recovery |
| [v0.4.0](https://github.com/fajarkraton/fajar-lang/releases/tag/v0.4.0) | Sovereignty | Generic enums, RAII/Drop, Future/Poll, lazy async |
| [v0.3.0](https://github.com/fajarkraton/fajar-lang/releases/tag/v0.3.0) | Dominion | Concurrency, async/await, ML native, self-hosting, bare metal |

## License

MIT License. See [LICENSE](LICENSE) for details.

## Contributing

See [CONTRIBUTING.md](docs/CONTRIBUTING.md) for guidelines.

Commit format: `<type>(<scope>): <description>` (e.g., `feat(codegen): add generic enum support`)
