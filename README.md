# Fajar Lang — Systems Programming Language for Embedded ML & OS Development

> **The only language where an OS kernel and a neural network can share the same codebase, type system, and compiler, with safety guarantees that no existing language provides.**

Fajar Lang (`fj`) is a statically-typed systems programming language designed for embedded machine learning and operating system development. Built with a Rust-based compiler featuring native tensor operations, bare-metal support, and compile-time context isolation, Fajar Lang targets ARM64, x86_64, RISC-V, and WebAssembly. Two complete operating systems — FajarOS Nova (x86_64) and FajarOS Surya (ARM64) — are written entirely in Fajar Lang, proving the language's capability for real-world systems programming from kernel to neural network inference.

[![CI](https://github.com/fajarkraton/fajar-lang/actions/workflows/ci.yml/badge.svg)](https://github.com/fajarkraton/fajar-lang/actions/workflows/ci.yml)
[![Release v9.0.1](https://img.shields.io/badge/release-v9.0.1_Ascension-blue)](https://github.com/fajarkraton/fajar-lang/releases/tag/v9.0.1)
[![Tests](https://img.shields.io/badge/tests-7%2C468_passing-brightgreen)](https://github.com/fajarkraton/fajar-lang/actions/workflows/ci.yml)
[![LOC](https://img.shields.io/badge/LOC-340K_Rust-informational)]()
[![Production](https://img.shields.io/badge/status-100%25_Production-success)]()
[![VS Code](https://img.shields.io/badge/VS_Code-Extension-007ACC?logo=visualstudiocode)](https://marketplace.visualstudio.com/items?itemName=primecore.fajar-lang)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![Made in Indonesia](https://img.shields.io/badge/Made_in-Indonesia-red)]()

---

## Table of Contents

- [Why Fajar Lang?](#why-fajar-lang)
- [Quick Start](#quick-start)
- [Feature Highlights](#feature-highlights)
- [Code Examples](#code-examples)
- [FajarOS — Operating Systems in Fajar Lang](#fajaros--operating-systems-in-fajar-lang)
- [Performance Benchmarks](#performance-benchmarks)
- [Language Comparison](#language-comparison)
- [CLI Reference](#cli-reference)
- [Project Structure](#project-structure)
- [Performance Benchmarks](#performance-benchmarks)
- [Project Stats](#project-stats)
- [Release History](#release-history)
- [Documentation](#documentation)
- [Contributing](#contributing)
- [Community](#community)
- [License](#license)

---

## Why Fajar Lang?

Existing languages force you to choose: **Rust** for systems, **Python** for ML, **C** for embedded. Fajar Lang unifies all three domains with compile-time safety guarantees through its unique context annotation system:

- `@kernel` — OS primitives, raw memory, IRQ, syscalls. No heap, no tensors.
- `@device` — Tensor operations, autograd, inference. No raw pointers.
- `@safe` — The default. Calls both domains. The compiler enforces isolation.
- `@unsafe` — Full access when you need it. Requires explicit opt-in.

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

If it compiles, the kernel code cannot accidentally trigger a heap allocation, and the ML code cannot accidentally dereference a raw pointer. **Safety by construction.**

---

## Quick Start

### Install from source

```bash
git clone https://github.com/fajarkraton/fajar-lang.git
cd fajar-lang
cargo build --release
```

The binary is at `target/release/fj`. Add it to your `PATH`:

```bash
export PATH="$PWD/target/release:$PATH"
```

### Run your first program

Create `hello.fj`:

```fajar
fn main() {
    println("Hello from Fajar Lang!")
}
```

```bash
fj run hello.fj
```

### Choose your backend

```bash
# Tree-walking interpreter (default, great for scripting)
fj run hello.fj

# Cranelift JIT (fast native execution)
fj run --native hello.fj

# LLVM JIT (requires llvm-18-dev, full optimizations)
fj run --llvm hello.fj

# Bytecode VM
fj run --vm hello.fj
```

### LLVM backend (optional)

```bash
sudo apt-get install llvm-18-dev libpolly-18-dev libzstd-dev
cargo build --release --features llvm
```

### Optional Feature Flags

```bash
cargo build --features gui          # Real OS windowing (winit + softbuffer)
cargo build --features smt          # Z3 SMT solver (needs libz3-dev)
cargo build --features cpp-ffi      # C++ FFI via libclang (needs libclang-dev)
cargo build --features python-ffi   # Python interop via pyo3 (needs python3-dev)
cargo build --features llvm         # LLVM backend (needs llvm-18-dev)
cargo build --features vulkan       # Vulkan compute (needs Vulkan SDK)
```

### Start the REPL

```bash
fj repl
```

---

## Feature Highlights

### Language

- **Rust-inspired syntax** — familiar to systems programmers, no lifetime annotations required
- **Dual-context safety** — `@kernel` / `@device` / `@safe` / `@unsafe` enforced at compile time
- **Native tensor types** — `Tensor` is a first-class citizen with compile-time shape checking
- **Generics and traits** — monomorphized generics, trait objects (`dyn Trait`), GAT, async traits
- **Pattern matching** — exhaustive `match` on enums, structs, tuples with `Option<T>` / `Result<T,E>`
- **Algebraic effects** — structured side-effect control with handlers and delimited continuations
- **Macro system** — `macro_rules!`, `#[derive(...)]`, `#[cfg(...)]`, attribute macros
- **Pipeline operator** — `x |> f |> g` for clean functional data flow
- **String interpolation** — `f"Hello {name}, result is {1 + 2}"`
- **Compile-time evaluation** — `const fn`, `comptime {}` blocks, tensor shape verification
- **Async/await** — work-stealing executor, async traits, streams, channels

### Compilation

- **3 backends** — Cranelift (JIT + AOT), LLVM (O0-O3 + LTO), WebAssembly
- **Cross-compilation** — ARM64, RISC-V, x86_64, Wasm, bare-metal targets
- **Incremental compilation** — file-level dependency graph, artifact caching
- **Profile-guided optimization (PGO)** — instrument, collect, optimize
- **SIMD** — auto-vectorization + manual intrinsics (SSE/AVX/NEON/SVE/RVV)
- **Security hardening** — stack canaries, CFI, address sanitizer simulation

### ML Runtime

- **70+ tensor operations** — matmul, conv2d, transpose, reshape, slice, concat, and more
- **Autograd** — tape-based reverse-mode automatic differentiation
- **Neural network layers** — Dense, Conv2d, MultiHeadAttention, BatchNorm, LSTM, GRU, Embedding, Dropout
- **Optimizers** — SGD (with momentum), Adam, AdamW, RMSprop, learning rate schedulers
- **Training** — MNIST 90%+, mixed precision (FP16/BF16), INT8 quantization
- **GPU support** — CUDA simulation, Vulkan compute, multi-GPU data parallelism
- **Model optimization** — structured pruning, knowledge distillation, compression pipeline
- **Export** — ONNX, TFLite, GGUF, Safetensors

### OS and Embedded

- **Memory management** — 4-level page tables, virtual/physical mapping, Copy-on-Write fork
- **Interrupts** — IDT, GDT, IRQ handlers, inline assembly (`asm!`)
- **Drivers** — VGA, serial, keyboard, PIT, NVMe, USB, VirtIO, I2C, SPI, DMA, CAN-FD
- **RTOS integration** — FreeRTOS, Zephyr FFI, RTIC compile-time scheduling
- **IoT** — WiFi, BLE, MQTT, LoRaWAN, OTA firmware updates
- **Board support** — STM32, ESP32, nRF52, Raspberry Pi, Radxa Dragon Q6A, Jetson

### Tooling

- **REPL** — multi-line editing, `:type` inspection, `:help`, analyzer-aware
- **LSP** — diagnostics, completion, hover, rename, inlay hints, workspace symbols
- **DAP debugger** — breakpoints, stepping, variables, watch expressions, VS Code integration
- **Formatter** — `fj fmt` with configurable style
- **Test framework** — `@test`, `@should_panic`, `@ignore`, `fj test`
- **Doc generation** — `///` doc comments, `fj doc` HTML output
- **Package manager** — `fj.toml`, registry, `fj add`, package signing, SBOM
- **VS Code extension** — syntax highlighting, snippets, LSP client

---

## Code Examples

### Hello World

```fajar
fn main() {
    println("Hello from Fajar Lang!")
}
```

### Fibonacci (Native Compiled)

```fajar
fn fibonacci(n: i64) -> i64 {
    if n <= 1 { n }
    else { fibonacci(n - 1) + fibonacci(n - 2) }
}

fn main() -> i64 {
    fibonacci(30)
}
```

### Neural Network Training

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

### Concurrency with Mutex

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

### Kernel and Device Bridge

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

---

## FajarOS — Operating Systems in Fajar Lang

Two complete operating systems are written 100% in Fajar Lang, demonstrating the language's capability for real systems programming.

### FajarOS Nova (x86_64)

A bare-metal x86_64 operating system. The kernel is a single 20,176-line Fajar Lang file compiled to a bootable ELF binary.

| Feature | Details |
|---------|---------|
| Kernel | 20,176 LOC, 757 `@kernel` functions |
| Shell | 240+ commands |
| Scheduler | Preemptive multitasking (timer-driven, round-robin) |
| Memory | Copy-on-Write fork, 4-level page tables, refcounting |
| Ring 3 | 5 user programs via `SYSCALL` instruction |
| Syscalls | 34 via table dispatch (EXIT through GPU_DISPATCH) |
| Storage | NVMe + FAT32 + ext2 + USB + ramdisk + journaling |
| VFS | `/`, `/dev`, `/proc`, `/mnt` + symlinks + hardlinks |
| Network | TCP (RFC 793) + UDP + HTTP server + socket API |
| GPU | VirtIO-GPU framebuffer (320x200) + compute dispatch |
| Users | Multi-user (16 accounts), login/logout, chmod/chown |
| Services | Init system (16 services), runlevels, syslogd, crond |
| Packages | `pkg install/remove/list/search/update/upgrade` |
| SMP | Multi-core boot (INIT-SIPI-SIPI) |
| ELF | ELF64 loader, PT_LOAD, exec from FAT32/ramfs |
| Processes | fork(CoW)/exec/waitpid, signals, job control |
| Pipes | Circular 4KB buffer, shell `\|` operator, redirects |
| Debug | GDB remote stub (RSP protocol, breakpoints, watchpoints) |
| Test | QEMU + KVM verified |

### FajarOS Surya (ARM64)

A microkernel OS targeting the Radxa Dragon Q6A (Qualcomm QCS6490). Verified on real hardware via SSH.

| Feature | Details |
|---------|---------|
| Architecture | Microkernel: `@kernel` + HAL drivers + `@safe` services + `@device` AI |
| Hardware | Radxa Dragon Q6A — Kryo 670, Adreno 643 GPU, Hexagon 770 NPU |
| Memory | MMU with 2MB identity-mapped blocks, EL0 user space |
| Scheduler | Preemptive with 10 syscalls, IPC message passing |
| AI Inference | QNN SDK (CPU + GPU backends), 24ms per inference |
| Shell | 65+ commands |
| Verified | JIT fib(40) in 0.68s (480x speedup), GPIO blink, QNN inference |

---

## Performance Benchmarks

Benchmarks on Intel Core i9-14900HX (24 cores, Linux 6.17):

| Benchmark | Interpreter | Cranelift JIT | LLVM JIT | Speedup |
|-----------|------------|---------------|----------|---------|
| fibonacci(30) | ~500ms | 3.9ms | 3.2ms | 128-156x |
| Loop 1M iterations | ~293ms | 5.8ms | 4.1ms | 50-71x |
| Lex 3000 tokens | 120us | - | - | - |
| Parse 300 statements | 190us | - | - | - |
| String concat 100 | 73us | - | - | - |

ARM64 benchmarks on Radxa Dragon Q6A (Qualcomm QCS6490, 8-core Kryo 670):

| Benchmark | Interpreter | Cranelift JIT | Speedup |
|-----------|------------|---------------|---------|
| fibonacci(30) | ~1000ms | 7.8ms | 128x |
| fibonacci(40) | - | 680ms | - |
| Loop 1M iterations | ~600ms | 12ms | 50x |
| QNN inference (INT8) | - | 24ms | - |
| Cold start to inference | - | 4ms | - |

---

## Language Comparison

| Feature | Fajar Lang | Rust | C | Python | Zig |
|---------|-----------|------|---|--------|-----|
| Static typing | Yes | Yes | Yes | No | Yes |
| Memory safety | Ownership (no lifetimes) | Ownership + lifetimes | Manual | GC | Manual |
| Native tensors | First-class `Tensor` type | Library (ndarray) | No | Library (NumPy) | No |
| Autograd | Built-in | No | No | Library (PyTorch) | No |
| OS kernel support | `@kernel` context | `#![no_std]` | Yes | No | Yes |
| GPU compute | Built-in (CUDA/Vulkan) | Library (wgpu) | Library | Library (CUDA) | No |
| Context isolation | `@kernel`/`@device`/`@safe` | No | No | No | No |
| Compile-time safety | Shape checking + context | Borrow checker | None | None | Comptime |
| Cross-compilation | ARM64, RISC-V, Wasm | Yes | Yes | No | Yes |
| REPL | Built-in | Third-party | No | Built-in | No |
| Package manager | Built-in (`fj add`) | Cargo | No (CMake) | pip | Zig build |
| Embedded ML inference | INT8 + NPU (QNN) | Manual | Manual | No | No |

---

## CLI Reference

| Command | Description |
|---------|-------------|
| `fj run <file.fj>` | Execute with tree-walking interpreter |
| `fj run --native <file.fj>` | Execute with Cranelift JIT |
| `fj run --llvm <file.fj>` | Execute with LLVM JIT |
| `fj run --vm <file.fj>` | Execute with bytecode VM |
| `fj repl` | Start interactive REPL |
| `fj check <file.fj>` | Type-check without executing |
| `fj build` | Build project from `fj.toml` |
| `fj new <name>` | Create a new project |
| `fj test` | Run `@test` functions |
| `fj fmt <file.fj>` | Format source code |
| `fj doc <file.fj>` | Generate HTML documentation |
| `fj watch <file.fj>` | Auto-run on file changes |
| `fj bench <file.fj>` | Run micro-benchmarks |
| `fj lsp` | Start Language Server Protocol server |
| `fj add <package>` | Add a package dependency |
| `fj gui <file.fj>` | Launch GUI window (`--features gui`) |
| `fj debug <file.fj>` | Start DAP debugger session |
| `fj profile <file.fj>` | Profile function call timings |
| `fj verify <file.fj>` | Formal verification (`--features smt`) |
| `fj dump-tokens <file.fj>` | Inspect lexer output |
| `fj dump-ast <file.fj>` | Inspect parser AST |
| `fj install <package>` | Install package from registry |
| `fj search <query>` | Search package registry |
| `fj publish` | Publish package to registry |

---

## Project Structure

```
fajar-lang/
  src/
    lexer/              Tokenization (82+ token kinds)
    parser/             AST (Pratt + recursive descent, macros)
    analyzer/           Types, scope, borrow checker, NLL CFG
    interpreter/        Tree-walking evaluator + async runtime (tokio bridge)
    vm/                 Bytecode compiler + VM (45 opcodes)
    codegen/
      cranelift/        Cranelift JIT/AOT (150+ runtime fns, security canary)
      llvm/             LLVM backend (inkwell, O0-O3 + LTO + PGO)
      wasm/             WebAssembly backend
    runtime/
      os/               Memory, IRQ, syscall, paging, drivers
      ml/               Tensor, autograd, layers, optimizers, GPU
    gui/                Windowing (winit), widgets, layout, bitmap font
    debugger/           DAP server (breakpoints, stepping, time-travel)
    lsp/                LSP server (inlay hints, signature help, semantic tokens)
    formatter/          Code formatter
    package/            Package manager, registry, signing, SBOM
    profiler/           Function profiling (interpreter + Cranelift native)
    distributed/        TCP RPC, clustering, tensor allreduce
    concurrency_v2/     Async actors (tokio::spawn + mpsc)
    stdlib_v3/          Crypto, networking, database, formats, regex
    bsp/                Board support packages (STM32, ESP32, Q6A)
    plugin/             Compiler plugin system (AST-phase)
    playground/         WASM playground (wasm-bindgen)
  stdlib/               Fajar Lang standard library (.fj source)
  examples/             178 example .fj programs
  tests/                Integration tests (eval, ML, OS, safety, property)
  benches/              Criterion benchmarks
  packages/             37 standard packages
  editors/vscode/       VS Code extension
  book/                 mdBook documentation (40+ pages)
  playground/           Browser-based playground (Monaco + WASM)
  docs/                 55+ reference documents
```

---

## Performance Benchmarks

Fibonacci(35) single execution — Intel i9-14900HX, Ubuntu 25.10:

| Language | Backend | Time | vs C |
|----------|---------|------|------|
| C | gcc -O2 | 0.020s | 1.0x |
| Go | 1.24 | 0.056s | 2.8x |
| **Fajar Lang** | **Cranelift JIT** | **0.240s** | **12x** |
| Python | CPython 3.13 | 0.533s | 27x |
| Fajar Lang | Interpreter | 34.7s | Scripting mode |

> **Cranelift JIT is faster than Python and within reach of Go.** For maximum performance, use `fj build --backend llvm --opt-level 2` for AOT-compiled binaries comparable to C/Rust.
>
> Full results: [`benches/baselines/RESULTS.md`](benches/baselines/RESULTS.md)

---

## Project Stats

| Metric | Value |
|--------|-------|
| Compiler LOC | 339,769 Rust across 343 files |
| Tests | 7,468 (0 failures, 0 clippy warnings, 37 test suites) |
| Examples | 178 `.fj` programs (175 pass `fj check`) |
| Error codes | 80+ across 10 categories |
| Standard packages | 37 (math, nn, hal, http, json, crypto, mqtt, db, ...) |
| Built-in functions | 430+ (runtime, networking, async, regex, GUI, HTTP framework) |
| Codegen backends | 3 (Cranelift, LLVM, WebAssembly) |
| Cross-compile targets | ARM64, RISC-V, x86_64, Wasm |
| Networking | WebSocket (tungstenite+TLS), MQTT (rumqttc), BLE (btleplug), HTTP/HTTPS |
| Async runtime | Real tokio I/O (sleep, http_get/post, spawn, join, select) |
| GUI | winit + softbuffer, bitmap font, button interaction, flex layout |
| HTTP framework | Router + middleware + handler dispatch + HTTPS (native-tls) |
| Regex | Core stdlib (match, find, replace, captures) with compiled cache |
| Security | Stack canary, bounds check, overflow check, linter (20 rules), taint analysis |
| Documentation | 55+ docs, 14 tutorials, 26 references, 15 guides |
| FajarOS Nova (x86_64) | 21,396 LOC, 835 functions, 270+ commands, 34 syscalls |
| FajarOS Surya (ARM64) | Cross-compiled to aarch64 ELF (82 KB), Q6A BSP (73 tests) |
| Hardware verified | Intel i9-14900HX, NVIDIA RTX 4090, Qualcomm QCS6490 |
| Production status | **100% production-ready** (all modules verified by audit) |

---

## Release History

| Version | Codename | Highlights |
|---------|----------|------------|
| **[v9.0.1](https://github.com/fajarkraton/fajar-lang/releases/tag/v9.0.1)** | **Ascension** | **100% production: real BLE connect/read/write, async_spawn/join/select, HTTPS server (native-tls), 7,468 tests** |
| [v9.0.0](https://github.com/fajarkraton/fajar-lang/releases/tag/v9.0.0) | Ascension | V10 features: async/await real tokio I/O, HTTP server framework, regex stdlib, LSP enhanced, 4 real-world examples |
| [v8.0.0](https://github.com/fajarkraton/fajar-lang/releases/tag/v8.0.0) | Dominion | All gaps closed: real WebSocket (tungstenite+TLS), MQTT (rumqttc), BLE (btleplug), GUI text+interaction, compiler wiring |
| [v7.0.0](https://github.com/fajarkraton/fajar-lang/releases/tag/v7.0.0) | Integrity | Full production audit, 214→0 kernel errors, native OS compile, 14 IoT builtins, security hardening, WASM playground |
| [v6.1.0](https://github.com/fajarkraton/fajar-lang/releases/tag/v6.1.0) | Illumination | V8 "Dominion" 810 tasks, self-hosting, package registry, IDE, security, GUI |
| [v5.5.0](https://github.com/fajarkraton/fajar-lang/releases/tag/v5.5.0) | Illumination | Async/await patterns, declarative macros, derive patterns, advanced traits, const fn |
| [v5.4.0](https://github.com/fajarkraton/fajar-lang/releases/tag/v5.4.0) | Zenith | FajarOS Nova 20K LOC, GPU compute, ext2, TCP, init system, packages |
| [v5.3.0](https://github.com/fajarkraton/fajar-lang/releases/tag/v5.3.0) | Bastion | NVMe, FAT32, USB, VFS, network stack, multi-user, GDB stub |
| [v5.2.0](https://github.com/fajarkraton/fajar-lang/releases/tag/v5.2.0) | Nexus | fork/exec/waitpid, pipes, signals, job control, scripting |
| [v5.0.0](https://github.com/fajarkraton/fajar-lang/releases/tag/v5.0.0) | Sovereignty | Multi-binary build, @safe enforcement, 9-dtype tensors, typed IPC |
| [v3.2.0](https://github.com/fajarkraton/fajar-lang/releases/tag/v3.2.0) | Surya Rising | Q6A edge deployment, FajarOS Nova, VirtIO/VFS/network drivers |
| [v3.0.0](https://github.com/fajarkraton/fajar-lang/releases/tag/v3.0.0) | Singularity | HKT, structured concurrency, GPU codegen (PTX/SPIR-V) |
| [v2.0.0](https://github.com/fajarkraton/fajar-lang/releases/tag/v2.0.0) | Transcendence | Dependent types, linear types, formal verification, tiered JIT |
| [v1.0.0](https://github.com/fajarkraton/fajar-lang/releases/tag/v1.0.0) | Genesis | First stable release, fuzzing, cross-platform, LSP, C/Python interop |
| [v0.6.0](https://github.com/fajarkraton/fajar-lang/releases/tag/v0.6.0) | Horizon | LLVM backend, debugger, BSP, RTOS, LSTM/GRU |
| [v0.5.0](https://github.com/fajarkraton/fajar-lang/releases/tag/v0.5.0) | Ascendancy | Test framework, trait objects, iterators, string interpolation |
| [v0.3.0](https://github.com/fajarkraton/fajar-lang/releases/tag/v0.3.0) | Dominion | Concurrency, async/await, ML native, self-hosting, bare metal |

---

## Documentation

| Resource | Link |
|----------|------|
| Language Specification | [`docs/FAJAR_LANG_SPEC.md`](docs/FAJAR_LANG_SPEC.md) |
| Grammar Reference (EBNF) | [`docs/GRAMMAR_REFERENCE.md`](docs/GRAMMAR_REFERENCE.md) |
| Architecture | [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) |
| Error Code Catalog | [`docs/ERROR_CODES.md`](docs/ERROR_CODES.md) |
| Standard Library API | [`docs/STDLIB_SPEC.md`](docs/STDLIB_SPEC.md) |
| Security Model | [`docs/SECURITY.md`](docs/SECURITY.md) |
| Examples Guide | [`docs/EXAMPLES.md`](docs/EXAMPLES.md) |
| mdBook (40+ pages) | [`book/`](book/) |
| Roadmap | [`docs/ROADMAP_V1.1.md`](docs/ROADMAP_V1.1.md) |

---

## Contributing

We welcome contributions! See [CONTRIBUTING.md](CONTRIBUTING.md) for the full guide.

```bash
# Build
cargo build

# Test (6,286 tests)
cargo test --features native

# Lint
cargo clippy -- -D warnings

# Format
cargo fmt
```

Commit format: `<type>(<scope>): <description>` (e.g., `feat(analyzer): add GAT constraint checking`)

---

## About the Author

**Muhamad Fajar Putranto, SE., SH., MH.** — serial entrepreneur with 28+ years of professional experience spanning taxation, technology, and innovation.

| Year | Milestone |
|------|-----------|
| 1996 | Started career at Indonesia's Directorate General of Taxes (DJP) |
| ~2008 | Founded **[IndoFace](https://indoface.wordpress.com/)** — Indonesian social network with **180,000+ members** (status updates, photo sharing, blog, voting, classified ads). One of Indonesia's pioneering homegrown social platforms |
| 2012 | Co-founded **[TaxPrime](https://www.taxprime.net/)** — now Indonesia's largest independent tax consulting firm (240+ professionals, 27 ex-DJP/Customs officials). Collaborating firm of [Andersen Global](https://andersen.com/) and member of [PrimeGlobal](https://www.primeglobal.net/) (300 firms, 110+ countries) |
| 2012 | Founded **[PrimeCore.id](https://www.primecore.id/)** — Global Business Solutions (affiliate of TaxPrime). Strategic business development, ESG & sustainability advisory, legal restructuring, global mobility, independent board services. Offices in Jakarta, Gresik, Surabaya, Cikarang. Ecosystem: TaxPrime, [Associa](https://www.primecore.id/) (HR), [TheTitan.Asia](https://www.primecore.id/) (private wealth), [InkubatorX](https://www.primecore.id/) (enterprise) |
| 2021-2024 | **[ITR World Tax "Highly Regarded"](https://www.itrworldtax.com/Lawyer/TaxPrime/Muhamad-Fajar-Putranto-Indonesia/Profile/7273)** — Tax Controversy Leader, 4 consecutive years. **World TP "Transfer Pricing Expert"** (2021) |
| 2022 | **[Indonesia Most Inspirational Taxpayers](https://www.taxprime.net/awards/the-most-inspirational-taxpayer/)** — Majalah Pajak, Tax Practitioner category |
| 2023 | Founded **[InkubatorX](https://www.pajak.com/ekonomi/inkubatorx-riset-akselerasi-pengembangan-hilirisasi/)** — ecosystem company (digital economy, renewable energy, nano technology). Developed **Nanosix** (nano curcumin, nano coconut water, biodiesel nano additive) |
| 2023 | **Wakil Ketua Dewan Pembina [ACEXI](https://acexi.org/)** (Asosiasi Ahli Emisi Karbon Indonesia) — Vice Chair of the Board of Trustees. Carbon emission expert association with MoU with BKI & BSN. Focus: capacity building, policy advocacy, ESG standardization, sustainability verification |
| 2025 | Elected **[Ketua Umum IKANAS STAN](https://www.pajak.com/ekonomi/fajar-putranto-terpilih-jadi-ketua-ikanas-stan-siap-konsolidasi/)** 2025-2028 — leading ~80,000 alumni of Indonesia's State College of Accountancy |
| 2026 | Created **Fajar Lang** — systems programming language for embedded ML & OS development. Built **FajarOS** — bare-metal OS on x86_64 and ARM64 (Radxa Dragon Q6A) |

> *"From tax auditor to social network founder, from global business advisory to carbon emission governance, to programming language creator — the journey of building things that matter."*

---

## Community

- **GitHub:** [github.com/fajarkraton/fajar-lang](https://github.com/fajarkraton/fajar-lang)
- **Issues:** [github.com/fajarkraton/fajar-lang/issues](https://github.com/fajarkraton/fajar-lang/issues)
- **Discussions:** [github.com/fajarkraton/fajar-lang/discussions](https://github.com/fajarkraton/fajar-lang/discussions)
- **Email:** fajar@primecore.id
- **Security:** security@primecore.id

### Dibuat di Indonesia

Fajar Lang adalah bahasa pemrograman sistem yang dibuat di Indonesia, dirancang untuk pengembangan embedded ML dan sistem operasi. Nama "Fajar" berarti "dawn" (fajar) dalam Bahasa Indonesia — mewakili era baru dalam pemrograman sistem yang menggabungkan keamanan, kecerdasan buatan, dan pemrograman bare-metal dalam satu bahasa.

---

## License

MIT License. See [LICENSE](LICENSE) for details.

Copyright (c) 2025-2026 Muhamad Fajar Putranto (TaxPrime / PrimeCore.id)
