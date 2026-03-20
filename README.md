# Fajar Lang (`fj`)

A statically-typed systems programming language for embedded ML and OS integration.

**The only language where an OS kernel and a neural network can share the same codebase, type system, and compiler.**

[![CI](https://github.com/fajarkraton/fajar-lang/actions/workflows/ci.yml/badge.svg)](https://github.com/fajarkraton/fajar-lang/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/fajarkraton/fajar-lang)](https://github.com/fajarkraton/fajar-lang/releases)
[![Tests](https://img.shields.io/badge/tests-5%2C469_passing-brightgreen)]()
[![LOC](https://img.shields.io/badge/LOC-152K_Rust-blue)]()
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

## Why Fajar Lang?

Existing languages force you to choose: **Rust** for systems, **Python** for ML, **C** for embedded. Fajar Lang unifies all three domains with compile-time safety guarantees:

- `@kernel` context: raw memory, IRQ, syscalls — no heap, no tensors
- `@device` context: tensors, autograd, inference — no raw pointers
- `@safe` context: calls both — compiler enforces isolation

```fajar
@kernel fn read_sensor() -> [f32; 4] {
    let data = port_read!(0x3F00, 4)
    data
}

@device fn infer(x: Tensor) -> Tensor {
    x @ weights |> relu |> softmax
}

@safe fn bridge() -> Action {
    let raw = read_sensor()
    let result = infer(Tensor::from_slice(raw))
    Action::from_prediction(result)
}
```

## Features

### Language
- **Rust-inspired syntax** — familiar to systems programmers, no lifetime annotations
- **Dual-context safety** — `@kernel`/`@device`/`@safe`/`@unsafe` enforced at compile time
- **Native tensor types** — `Tensor` is a first-class citizen with compile-time shape checking
- **Algebraic effects** — structured side-effect control with handlers and delimited continuations
- **Pattern matching** — exhaustive `match` on enums, structs, tuples with `Option<T>` / `Result<T,E>`
- **Pipeline operator** — `x |> f |> g` for functional data flow
- **Generics & traits** — monomorphized generics, trait-based dispatch, GAT, async traits
- **Macro system** — `macro_rules!`, `#[derive(...)]`, `#[cfg(...)]`, attribute macros
- **Compile-time evaluation** — `const fn`, `comptime {}` blocks, tensor shape verification
- **String interpolation** — `f"Hello {name}, {1 + 2}"`

### Compilation
- **3 backends** — Cranelift (JIT+AOT), LLVM (O0-O3), WebAssembly
- **Incremental compilation** — file-level dependency graph, artifact caching
- **Cross-compilation** — ARM64, RISC-V, x86_64, Wasm
- **PGO** — profile-guided optimization
- **SIMD** — auto-vectorization + manual intrinsics (SSE/AVX/NEON/SVE/RVV)
- **Security hardening** — stack canaries, CFI, ASan/MSan simulation, `-fharden`

### ML Runtime
- **70+ tensor ops** — matmul, conv2d, transpose, reshape, etc.
- **Autograd** — tape-based reverse-mode differentiation
- **Layers** — Dense, Conv2d, MultiHeadAttention, BatchNorm, LSTM, GRU, Embedding, Dropout
- **Optimizers** — SGD (momentum), Adam, AdamW, RMSprop
- **Training** — MNIST 90%+, mixed precision (FP16/BF16), INT8 quantization
- **GPU** — CUDA simulation, multi-GPU data parallelism, GPU autograd
- **Model optimization** — structured pruning, knowledge distillation, compression pipeline
- **Export** — ONNX, TFLite, custom autodiff (JVP/VJP)

### OS & Embedded
- **Memory management** — 4-level page tables, virtual/physical mapping
- **Interrupts** — IDT, GDT, IRQ handlers, inline assembly
- **Drivers** — VGA, serial, keyboard, PIT, I2C, SPI, DMA, CAN-FD
- **RTOS** — FreeRTOS/Zephyr FFI, RTIC compile-time scheduling
- **IoT** — WiFi, BLE, MQTT, LoRaWAN, OTA updates
- **BSP** — STM32, ESP32, nRF52, RPi4, Jetson Orin Nano
- **Power** — sleep modes, wake sources, battery life estimation

### Concurrency
- **Threads** — spawn, join, thread-local storage
- **Sync** — Mutex, RwLock, Condvar, Barrier, atomics (CAS, fence)
- **Async** — async/await, work-stealing executor, streams
- **Channels** — unbounded, bounded, select, close
- **Async I/O** — io_uring/epoll simulation, TCP/UDP, HTTP, WebSocket

### Tooling
- **REPL** — multi-line, `:type`, `:help`, analyzer-aware
- **LSP** — diagnostics, completion, hover, rename, inlay hints, workspace symbols
- **DAP debugger** — breakpoints, stepping, variables, watch expressions, VS Code integration
- **Formatter** — `fj fmt` with configurable style (trailing commas, brace style, import sorting)
- **Test framework** — `@test`, `@should_panic`, `@ignore`, `fj test`
- **Doc generation** — `///` comments, `fj doc` HTML output
- **Package manager** — `fj.toml`, registry, `fj add`, signing, SBOM
- **VS Code extension** — syntax highlighting, snippets, LSP client

## Quickstart

### Install from source

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

# Native JIT — Cranelift (fastest)
fj run --native examples/native_hello.fj

# Native JIT — LLVM (requires llvm-18-dev)
fj run --llvm examples/native_hello.fj

# Bytecode VM
fj run --vm examples/hello.fj
```

### LLVM backend (optional)

```bash
sudo apt-get install llvm-18-dev libpolly-18-dev libzstd-dev
cargo build --release --features llvm
```

### Start the REPL

```bash
fj repl
```

## Examples

### Hello World

```fajar
fn main() {
    println("Hello from Fajar Lang!")
}
```

### Fibonacci (Native)

```fajar
fn fibonacci(n: i64) -> i64 {
    if n <= 1 { n }
    else { fibonacci(n - 1) + fibonacci(n - 2) }
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
        println(f"Predicted: {tensor_argmax(output)}")
        epoch = epoch + 1
    }
}
```

### Algebraic Effects

```fajar
effect Console {
    fn read_line() -> str
    fn write_line(msg: str) -> void
}

fn greet(name: str) -> void / Console {
    perform Console.write_line(f"Hello, {name}!")
}

fn main() {
    handle greet("Fajar") {
        Console.write_line(msg) => {
            println(msg)
            resume
        }
    }
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

### Async HTTP Server

```fajar
async fn serve(addr: str, port: i32) {
    let listener = tcp_bind(f"{addr}:{port}")
    loop {
        let stream = listener.accept().await
        let request = stream.read().await
        let response = match parse_route(request) {
            "/health" => json_response(200, "{\"status\": \"ok\"}"),
            "/predict" => run_inference(request),
            _ => json_response(404, "{\"error\": \"not found\"}")
        }
        stream.write(response).await
    }
}
```

## CLI Commands

| Command | Description |
|---------|-------------|
| `fj run <file.fj>` | Execute with interpreter |
| `fj run --native <file.fj>` | Execute with Cranelift JIT |
| `fj run --llvm <file.fj>` | Execute with LLVM JIT |
| `fj run --vm <file.fj>` | Execute with bytecode VM |
| `fj repl` | Start interactive REPL |
| `fj check <file.fj>` | Type-check without executing |
| `fj build` | Build current project |
| `fj new <name>` | Create a new project |
| `fj test <file.fj>` | Run `@test` functions |
| `fj fmt <file.fj>` | Format source code |
| `fj doc <file.fj>` | Generate HTML docs |
| `fj watch <file.fj>` | Auto-run on file changes |
| `fj bench <file.fj>` | Run micro-benchmarks |
| `fj lsp` | Start LSP server |
| `fj add <package>` | Add package dependency |
| `fj dump-tokens <file.fj>` | Show lexer output |
| `fj dump-ast <file.fj>` | Show parser AST |

## Project Structure

```
src/
  lexer/              -- Tokenization (82+ token kinds)
  parser/             -- AST (Pratt + recursive descent, macros)
  analyzer/           -- Types, scope, borrow checker, effects, GAT
  interpreter/        -- Tree-walking evaluator
  vm/                 -- Bytecode compiler + VM (45 opcodes)
  codegen/
    cranelift/        -- Cranelift JIT/AOT (150+ runtime fns)
    llvm/             -- LLVM backend (inkwell, O0-O3)
    wasm/             -- WebAssembly backend
  compiler/           -- Incremental compilation, comptime, security, editions
  runtime/
    os/               -- Memory, IRQ, syscall, paging, drivers, power
    ml/               -- Tensor, autograd, layers, optimizers, GPU, pruning
    gpu/              -- CUDA + wgpu backends
    simd.rs           -- SIMD vector types + auto-vectorization
    async_io.rs       -- io_uring, TCP/UDP, HTTP, WebSocket
  debugger/           -- DAP server (breakpoints, stepping, variables)
  debugger_v2/        -- Time-travel debugging, record/replay, profiling
  deployment/         -- Containers, observability, runtime mgmt, security
  gpu_codegen/        -- PTX + SPIR-V backends, kernel fusion, GPU memory
  package_v2/         -- Workspaces, build scripts, cross-compilation
  ml_advanced/        -- Transformer, diffusion, RL, model serving
  rtos/               -- FreeRTOS, Zephyr, RTIC integration
  iot/                -- WiFi, BLE, MQTT, LoRaWAN, OTA
  bsp/                -- Board support packages
  lsp/                -- Language Server Protocol
  formatter/          -- Code formatter
  package/            -- Package manager, registry, signing
stdlib/               -- Fajar Lang stdlib (.fj source)
examples/             -- 126 example .fj programs
tests/                -- Integration tests (eval, ML, OS, safety, property)
benches/              -- Criterion benchmarks
packages/             -- 7 standard packages
editors/vscode/       -- VS Code extension
book/                 -- mdBook documentation (40+ pages)
docs/                 -- 44+ reference documents
```

## Stats

| Metric | Value |
|--------|-------|
| Rust LOC | ~152,000 |
| Source files | 220+ `.rs` files |
| Tests | 6,045 native + 566 integration = 6,471 total (0 failures) |
| Examples | 126 `.fj` programs |
| Error codes | 78+ across 9 categories |
| Documentation | 50+ docs + 40-page mdBook |
| Standard packages | 7 (math, nn, hal, drivers, http, json, crypto) |
| Codegen backends | 3 (Cranelift, LLVM, WebAssembly) |
| **FajarOS Nova** | **8,327 LOC kernel, 135 commands, 215KB ELF** |
| Hardware verified | QEMU + KVM (Intel i9-14900HX, 24 cores) |

## Releases

| Version | Codename | Highlights |
|---------|----------|------------|
| **[v3.2.0](https://github.com/fajarkraton/fajar-lang/releases/tag/v3.2.0)** | **Surya Rising** | **`const` in body, or-patterns, tensor short aliases, real HW sensor monitoring, Q6A edge deployment, FajarOS Nova 102 cmd, VirtIO/VFS/network drivers, anomaly detection, systemd service** |
| [v3.1.1](https://github.com/fajarkraton/fajar-lang/releases/tag/v3.1.1) | Surya Enablers | Labeled break/continue, const folding, @kernel codegen enforcement, @interrupt attribute, 90+ bare-metal builtins |
| **[v3.0.0](https://github.com/fajarkraton/fajar-lang/releases/tag/v3.0.0)** | **Singularity** | **HKT, structured concurrency, distributed computing, advanced ML v2, GPU codegen (PTX/SPIR-V), package ecosystem v2, debugger v2, production deployment** |
| [v2.0.0](https://github.com/fajarkraton/fajar-lang/releases/tag/v2.0.0) | Transcendence | Dependent types, linear types, formal verification, tiered JIT, effect system v2, GC mode, self-hosting v2, LSP v2 |
| [v1.1.0](https://github.com/fajarkraton/fajar-lang/releases/tag/v1.1.0) | Ascension | Hardware discovery, NPU, FP8/BF16, Jetson Thor BSP, AVX-512/AMX, CI/CD, package registry, multi-accelerator, real-world demos |
| [v1.0.0](https://github.com/fajarkraton/fajar-lang/releases/tag/v1.0.0) | Genesis | First stable release — fuzzing, conformance testing, string interning, cross-platform distribution, LSP completion, documentation site, C/Python/Wasm interop, release engineering |
| [v0.9.0](https://github.com/fajarkraton/fajar-lang/releases/tag/v0.9.0) | Convergence | Effect system, compile-time eval, macros, SIMD, security hardening, async I/O, editions |
| [v0.8.0](https://github.com/fajarkraton/fajar-lang/releases/tag/v0.8.0) | Apex | GPU training, GAT, incremental compilation, model optimization, DAP debugger, LoRaWAN |
| [v0.7.0](https://github.com/fajarkraton/fajar-lang/releases/tag/v0.7.0) | Zenith | Wasm, ESP32 IoT, Polonius borrow checker, Transformer, PGO, RTIC, package signing |
| [v0.6.0](https://github.com/fajarkraton/fajar-lang/releases/tag/v0.6.0) | Horizon | LLVM backend, debugger, BSP, registry, lifetimes, RTOS, LSTM/GRU, VENTUNO Q |
| [v0.5.0](https://github.com/fajarkraton/fajar-lang/releases/tag/v0.5.0) | Ascendancy | Test framework, doc comments, trait objects, iterators, string interpolation |
| [v0.4.0](https://github.com/fajarkraton/fajar-lang/releases/tag/v0.4.0) | Sovereignty | Generic enums, RAII/Drop, Future/Poll, lazy async |
| [v0.3.0](https://github.com/fajarkraton/fajar-lang/releases/tag/v0.3.0) | Dominion | Concurrency, async/await, ML native, self-hosting, bare metal |

## Contributing

See [CONTRIBUTING.md](docs/CONTRIBUTING.md) for guidelines.

Commit format: `<type>(<scope>): <description>` (e.g., `feat(effects): add algebraic effect handlers`)

## License

MIT License. See [LICENSE](LICENSE) for details.
