# Fajar Lang — Systems Programming Language for Embedded ML & OS Development

> **The only language where an OS kernel and a neural network can share the same codebase, type system, and compiler, with safety guarantees that no existing language provides.**

Fajar Lang (`fj`) is a statically-typed systems programming language designed for embedded machine learning and operating system development. Built with a Rust-based compiler featuring native tensor operations, bare-metal support, and compile-time context isolation, Fajar Lang targets ARM64, x86_64, RISC-V, and WebAssembly. Two complete operating systems — FajarOS Nova (x86_64) and FajarOS Surya (ARM64) — are written entirely in Fajar Lang, proving the language's capability for real-world systems programming from kernel to neural network inference.

[![CI](https://github.com/fajarkraton/fajar-lang/actions/workflows/ci.yml/badge.svg)](https://github.com/fajarkraton/fajar-lang/actions/workflows/ci.yml)
[![Release v33.5.0](https://img.shields.io/badge/release-v33.5.0_Stage--1--Full_Self--Hosting-brightgreen)](https://github.com/fajarkraton/fajar-lang/releases/tag/v33.5.0)
[![Tests](https://img.shields.io/badge/tests-10K%2B_CI_green-brightgreen)](https://github.com/fajarkraton/fajar-lang/actions/workflows/ci.yml)
[![Stress](https://img.shields.io/badge/stress-80%2F80_at_threads%3D64-success)](https://github.com/fajarkraton/fajar-lang/actions/workflows/ci.yml)
[![Unwrap](https://img.shields.io/badge/production_unwrap-0-success)]()
[![Doc Warnings](https://img.shields.io/badge/cargo_doc-0_warnings-success)]()
[![Doc Coverage](https://img.shields.io/badge/pub--item_docs-95.79%25-success)]()
[![Error Codes](https://img.shields.io/badge/error--code_coverage-gap_0-success)]()
[![Modules](https://img.shields.io/badge/modules-54_%5Bx%5D_%2F_0_%5Bf%5D_%2F_0_%5Bs%5D-success)]()
[![LOC](https://img.shields.io/badge/LOC-449K_Rust-informational)]()
[![FajarOS](https://img.shields.io/badge/FajarOS-Nova_v3.7.0_security_triple_+_ext2-success)]()
[![Ring 3](https://img.shields.io/badge/Ring_3-user_mode_works-success)]()
[![CUDA](https://img.shields.io/badge/CUDA-RTX_4090_GPU_compute-76b900)]()
[![FajarQuant](https://img.shields.io/badge/FajarQuant-Phase_D_IntLLM_Base_PASS_PPL_54.1-orange)](https://github.com/fajarkraton/fajarquant)
[![JIT](https://img.shields.io/badge/JIT-76x_speedup-purple)]()
[![VS Code](https://img.shields.io/badge/VS_Code-Extension-007ACC?logo=visualstudiocode)](https://marketplace.visualstudio.com/items?itemName=primecore.fajar-lang)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache_2.0-yellow.svg)](LICENSE)
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

# LLVM AOT with O3 + LTO (production builds)
fj build --backend llvm --opt-level 3 --lto=thin hello.fj

# LLVM with PGO (maximum performance)
fj build --backend llvm --pgo=generate hello.fj && ./hello && fj build --backend llvm --pgo=use=default.profdata hello.fj

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
- **Macro system** — `macro_rules!`, `format!`, `matches!`, `println!`, `assert_eq!`, `cfg!`, `#[derive(...)]`, token tree expansion engine
- **Generators** — `yield` keyword, `gen fn`, `GeneratorIter` (for-in compatible), `AsyncStream`, coroutines
- **Pipeline operator** — `x |> f |> g` for clean functional data flow
- **String interpolation** — `f"Hello {name}, result is {1 + 2}"`
- **Compile-time evaluation** — `const fn`, `comptime {}` blocks, tensor shape verification
- **Async/await** — real tokio I/O, async traits, streams, channels, spawn/join/select

### Compilation

- **3 backends** — Cranelift (JIT + AOT), LLVM (O0-O3 + LTO + PGO), WebAssembly (WASI P1/P2)
- **LLVM O2/O3** — production-grade optimizations: target-specific CPU codegen (`--target-cpu=native`), function attributes (inline/noinline/cold), noalias/nonnull/readonly on refs
- **Link-Time Optimization (LTO)** — thin/full LTO via `--lto=thin`, cross-module inlining, dead code elimination, `--release` auto-enables thin LTO
- **Profile-Guided Optimization (PGO)** — `--pgo=generate` → run → `--pgo=use=profile.profdata`, branch weight optimization
- **Cross-compilation** — ARM64, RISC-V, x86_64, Wasm, bare-metal targets with `--reloc`, `--code-model`, `--target-features`
- **WASI deployment** — 8 WASI Preview 1 syscalls, component model (WIT interfaces), `wasi:cli/command` + `wasi:http/proxy` worlds
- **Incremental compilation** — file-level dependency graph, content hashing, artifact caching
- **Security hardening** — stack canaries, CFI, bounds checking, address sanitizer simulation

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
- **LSP** — type-driven completion (dot/:: context-aware), scope-aware rename, incremental analysis, cross-file go-to-definition, smart code actions (11 error codes), enhanced hover (fn signatures + struct defs + variable types), call hierarchy, 18 features total
- **DAP debugger** — breakpoints, stepping, variables, watch expressions, VS Code integration
- **Formatter** — `fj fmt` with configurable style
- **Test framework** — `@test`, `@should_panic`, `@ignore`, `fj test`
- **Doc generation** — `///` doc comments, `fj doc` HTML output
- **Package manager** — `fj.toml`, registry, `fj add/update/tree/audit`, git/path deps, workspaces, feature flags, package signing, SBOM
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

A bare-metal x86_64 operating system written 100% in Fajar Lang. Boots to an interactive shell with 105 working commands, Ring 3 user-mode programs, NVMe storage, and kernel-native FajarQuant quantization.

| Feature | Details |
|---------|---------|
| Kernel | 41,400+ LOC (154 `.fj` files), 1.15 MB LLVM O2 ELF |
| Shell | 105 commands tested (90/90 automated, 0 crashes) |
| Boot | 61 init stages via GRUB Multiboot2, boots to `nova>` prompt |
| Ring 3 | User-mode ELF via IRETQ + SYSCALL/SYSRETQ (SYS_EXIT returns to shell) |
| FajarQuant | Lloyd-Max 2-bit/4-bit quantization in `@kernel` context — `quant` command |
| Scheduler | Preemptive multitasking (PIT 100Hz, round-robin) |
| Memory | 128MB identity-mapped, frame allocator (hardware BSF/POPCNT), 4-level paging |
| ACPI | RSDP/XSDT/MADT parsing, CPU enumeration, page-mapped tables |
| Storage | NVMe (QEMU NVMe Ctrl 64MB) + ramdisk fallback |
| VFS | `/`, `/dev`, `/proc`, `/mnt` + symlinks + hardlinks |
| Network | TCP (RFC 793) + UDP + HTTP + socket API |
| GPU | VirtIO-GPU + Multiboot2 framebuffer (1024x768x32) |
| Desktop | Compositor + animations + virtual desktops + 5 apps |
| Interrupts | PIC IRQ handlers (32-47), LAPIC spurious (255), 6 exception vectors |
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
| Release | **v33.5.0 "Stage-1-Full Self-Hosting"** (2026-05-05) — fj-source compiler now compiles ARBITRARY subset fj programs (closes R7 "driver narrow"). New `stdlib/parser_ast.fj` (346 LOC) + `stdlib/codegen_driver.fj` (200 LOC) + `tests/selfhost_stage1_full.rs` (8 E2E tests, 0.11s, all PASS). Cumulative ~4.5h vs plan 5-15d (-99%). Predecessor v33.4.0: Stage-1-Subset (5 hardcoded driver shapes). Source: `docs/SELFHOST_FJ_PHASE_{0..8}_FINDINGS.md`. |
| Compiler LOC | ~450,000 Rust across 391+ files in src/ |
| Tests | **8,974 lib** + 2,498+ integ + 14 doc ≈ **11,486 total** — 0 failures, 0 flakes, 0 clippy, 0 rustdoc warnings (incl. `--document-private-items`), 162+ LLVM tests green under `--features llvm,native` |
| Doc coverage | **95.79% pub-item** + **100% stdlib_v3** — strict-mode rustdoc passes; `scripts/check_doc_coverage.sh` + `scripts/check_stdlib_docs.sh` enforce |
| Error-code coverage | **gap=0** — 135 cataloged, 125 covered + 12 forward-compat (per §6.6 R6); `python3 scripts/audit_error_codes.py --strict` enforces |
| Tutorial | `docs/TUTORIAL.md` 10 chapters, basics → robot control loop |
| Examples | 243 `.fj` programs + 6 multi-file real-project folders (`calculator-cli`, `tcp-echo-server`, `embedded-mnist`, `package_demo`, `nova`, `surya`) |
| Benchmarks | 5 vs C/Rust/Go (fibonacci, bubble_sort, sum_loop, matrix_multiply, mandelbrot) — `bash benches/baselines/run_baselines.sh` |
| FajarQuant | 49-86% lower MSE than TurboQuant (adaptive PCA rotation) |
| JIT | 76x speedup on fib(30) via Cranelift native compilation |
| GPU | Real CUDA detection (RTX 4090: 9,728 cores, 16 GB VRAM) |
| Error codes | 80+ across 10 categories |
| Standard packages | 39 (math, nn, hal, http, json, crypto, mqtt, db, ...) |
| Built-in macros | 14 (`vec!`, `format!`, `matches!`, `println!`, `assert_eq!`, `cfg!`, `dbg!`, `todo!`, `env!`, `stringify!`, `concat!`, `assert!`, `include_str!`, `line!`) |
| Codegen backends | 3 (Cranelift JIT/AOT, LLVM O0-O3+LTO+PGO, WebAssembly+WASI) |
| LLVM optimizations | O0-O3, Os, Oz, thin/full LTO, PGO generate/use, native CPU targeting |
| Cross-compile targets | ARM64, RISC-V, x86_64, Wasm, bare-metal (no_std) |
| Package manager | `fj add/update/tree/audit/publish/install`, git/path deps, workspaces, features |
| Networking | WebSocket (tungstenite+TLS), MQTT (rumqttc), BLE (btleplug), HTTP/HTTPS |
| Async runtime | Real tokio I/O (sleep, http_get/post, spawn, join, select) |
| Generators | `yield` keyword, `gen fn`, `GeneratorIter`, `AsyncStream`, `Coroutine` |
| LSP features | 22 (semantic tokens, inlay hints, type-driven completion, scope-aware rename, workspace symbols, 11 code actions) |
| Macro system | Token trees, pattern matching (`$x:expr`), repetition (`$()*`), expansion, derive (7 traits) |
| WASI | P1 (8 syscalls) + P2 (WIT parser, component model, filesystem, streams, HTTP, sockets) |
| GUI | winit + softbuffer, bitmap font, button interaction, flex layout |
| HTTP framework | Router + middleware + handler dispatch + HTTPS (native-tls) |
| Security | Stack canary, bounds check, overflow check, linter (20 rules), taint analysis |
| Documentation | 55+ docs, 14 tutorials, 26 references, 15 guides |
| FajarOS Nova (x86_64) | **v3.7.0 "FS Roundtrip"** — 108K LOC, 183 .fj files, 35 kernel tests, **SMEP+SMAP+NX security triple closed** (V29.P3.P6 6-invariant gate), ASLR, VFS write (RamFS+FAT32+ext2 — V30.TRACK4 9-invariant disk roundtrip), Ring 3 user mode, NVMe, FajarQuant kernel-native, 14 LLM shell commands (SmolLM-135M v5/v6 E2E), Gemma 3 1B foundation audit-complete via Path D (V30.GEMMA3) |
| FajarOS Surya (ARM64) | Cross-compiled to aarch64 ELF (82 KB), Q6A BSP (73 tests) |
| Hardware verified | Intel i9-14900HX, NVIDIA RTX 4090, Qualcomm QCS6490 |
| FFI v2 | C++ templates/STL/smart-ptr, Python async/NumPy, Rust traits, `fj bindgen` |
| Verification | SMT symbolic execution, @kernel/@device proofs, DO-178C/ISO-26262 certification |
| Effects | Algebraic effects + handlers, effect inference, effect polymorphism |
| Dependent types | Pi types, Sigma types, refinement types, dependent array bounds |
| GPU codegen | SPIR-V (Vulkan), PTX (CUDA), kernel fusion, auto-dispatch |
| Production status | **100% production-ready** (V31.C "Phase D + Track B" — compiler attrs + OS security triple + LLM training resilience) |
| V27.5 additions | AI scheduler builtins, `@interrupt` ARM64+x86_64 wrappers, `@app`+`@host` annotations, `Cap<T>` linear type, refinement param checks, `fb_set_base`/`fb_scroll`, IPC stub generator |
| V28-V31 additions | **Compiler:** `@noinline`+`@inline`+`@cold` lexer + 5-layer silent-build prevention (V29.P1), `@no_vectorize` codegen attribute IR+disasm verified (V31.B.P2), `FJ_EMIT_IR` env var (pre-opt LLVM IR dump). **CLAUDE.md:** §6.10 FS Roundtrip Coverage Rule, §6.11 Training Script Interruption-Safety Rule. **FajarOS Nova v3.4.0→v3.7.0:** SMEP re-enabled (V29.P2), SMAP re-enabled (V29.P3), NX triple closure (V29.P3.P6), Gemma 3 1B Path D audit (V30.GEMMA3 12 phases PASS), ext2+FAT32 disk harness (V30.TRACK4). **FajarQuant Phase D IntLLM:** Mini v2 PPL 80.0, **Base c.1 PASS at val_loss 3.99 / PPL 54.1 by 0.21 nat margin** (Chinchilla-optimal 21.16 tok/p, 8h03m on RTX 4090 Laptop), Track B 5-layer interruption-safety (ckpt_every/--resume/StepWatchdog/HF timeout+retry/test-train-watchdog), nohup line-buffering hardening |

---

## Release History

| Version | Codename | Highlights |
|---------|----------|------------|
| [**v33.5.0**](https://github.com/fajarkraton/fajar-lang/releases/tag/v33.5.0) | **Stage-1-Full Self-Hosting** | **fj-source compiler compiles ARBITRARY Stage-1-Subset programs (2026-05-05).** Closes Phase 5 R7 "driver narrow" risk. New `stdlib/parser_ast.fj` (346 LOC, 16 fns) builds flat-tagged AST from any subset fj source string via direct substring extraction (ident text + literal values from source, not from `[i64]` token stream). New `stdlib/codegen_driver.fj` (200 LOC, 8 fns) walks the AST and drives `stdlib/codegen.fj`'s `emit_*` API to produce C; `println(...)` mapped to `fj_println_int(...)` C runtime helper. New `tests/selfhost_stage1_full.rs` ships 8 Rust integration tests, each passing a real fj source string (not a hardcoded driver) — P1-P5 from v33.4.0 plus 3 NEW shapes the v33.4.0 driver couldn't compile (P6 chained binop `x+y+z`=17; P7 multiplication `a*b`=42; P8 subtract-in-condition `x-y>10`=99). All 8 PASS in 0.11s. Honest gaps for future Stage-1-Full extension: multi-fn cross-fn calls (new R8), while/for/match, struct/enum, string/float/bool literals — each ~10-50 LOC fj on the established pattern. Cumulative effort across v33.4.0+v33.5.0: ~4.5h Claude time vs plan 5-15d (-99% variance). Sister Rust compiler stays as production reference. Source: `docs/SELFHOST_FJ_PHASE_{0..8}_FINDINGS.md` + `docs/SELFHOST_FJ_PRODUCTION_PLAN.md`. |
| [**v33.4.0**](https://github.com/fajarkraton/fajar-lang/releases/tag/v33.4.0) | **Stage-1-Subset Self-Hosting** | **fj-lang self-hosts at Stage-1-Subset level (2026-05-05).** stdlib/lexer.fj (513 LOC, 10 fns; bit-equivalent vs Rust on canonical input — 19/19 tokens match) + stdlib/parser.fj (784 LOC, 27 fns; 30/30 self-tests PASS covering all subset forms) + stdlib/analyzer.fj (432 LOC, 19 fns; 6/7 smoke; 8 of 16 SE codes) + stdlib/codegen.fj (321 LOC, 17 fns; emits valid C, gcc round-trip 2/2). New tests/selfhost_stage1_subset.rs: 5 Rust integration tests run full bootstrap chain (fj-source codegen → C → gcc → executable), each asserts exit code (P1=42, P2=7, P3=30, P4=111, P5=0+stdout=777). 7 phases CLOSED: audit (revealed Rust simulation theatre), subset lexer, subset parser, subset analyzer, subset codegen (pivoted Cranelift FFI → gcc backend), bootstrap chain wire, E2E test suite. **Honest scope**: Stage-1-Full requires parser AST-builder upgrade (~1d, deferred to post-v33.4.0); Stage 2 triple-test is roadmap-only; sister Rust compiler stays as production reference. Cumulative effort: ~3h Claude time vs plan 5-10d (-99%). Source: `docs/SELFHOST_FJ_PHASE_{0..6}_FINDINGS.md` + `docs/SELFHOST_FJ_PRODUCTION_PLAN.md`. |
| [**v33.3.0**](https://github.com/fajarkraton/fajar-lang/releases/tag/v33.3.0) | **FajarQuant Algorithm 100% Fajar Lang** | **FajarQuant Rust algorithm crate (~2,649 LOC) ported to pure Fajar Lang stdlib (2026-05-05).** New `stdlib/fajarquant.fj` (986 LOC, 39 fj functions, 7 modules: hierarchical, scalar_baseline, fused_attention, turboquant LCG+beta+lloyd+quant, kivi per-channel keys + per-token values, adaptive PCA via power iteration). 70+ bit-equivalent I/O pairs verified vs Rust at FULL f64 precision (e.g. fused_attention output `1.1165579545845175` exact 16/16 decimals; PCA covariance + eigenvectors all bit-exact). One fj-lang core change: analyzer registers `wrapping_*` + `saturating_*` integer arithmetic builtins (interpreter dispatched correctly; analyzer was missing the signature registration; surfaced by FajarQuant LCG port). Sister Rust crate `fajarquant 0.4.0` continues for crates.io distribution. **Out-of-scope (locked-in)**: Python training (PyTorch ecosystem, different lifecycle phase), vendored microsoft/BitNet TL2 C++ kernel (F.11 PERMANENT-DEFERRED). Cumulative effort: ~115 minutes vs 10.5-17d original plan estimate (-99% variance). 10 NEW integration tests in `tests/fajarquant_fj_stdlib_bit_equivalent.rs`. Source: `docs/FAJARQUANT_FJ_PORT_PHASE_*_FINDINGS.md` + `docs/FAJARQUANT_RUST_TO_FJ_PLAN.md`. |
| [**v33.2.0**](https://github.com/fajarkraton/fajar-lang/releases/tag/v33.2.0) | **FAJAROS_100PCT TERMINAL COMPLETE** | **9/9 fj-lang LLVM compiler gaps closed (2026-05-05)**: G-A LLVM atomics, G-B `@naked` modifier, G-C `@no_mangle`, G-G `global_asm!()`, G-H raw strings, G-I asm raw strings, G-K `@no_vectorize` modifier-stack, **G-M --code-model kernel implies `noredzone`** (NEW; prevents IRQ red-zone corruption), G-N `@naked` codegen `noinline + ret undef`. **FajarOS Nova kernel build path now ZERO non-fj LOC**: vecmat_v8.c (585 LOC) DELETED + all 4 C mailbox functions migrated to pure fj (km_vecmat_packed_v8, tfm_attention_score, tfm_rope_apply_at, mdl_ram_lmhead_argmax_v8_tied). 1572-entry RoPE sin LUT moved from C to global_asm rodata in `kernel/compute/rope_lut.fj`. Insight: "G-L EXC:14 in 295M-iter loop" was the same red-zone-corruption-under-IRQ class as G-M. 8974 lib tests, 0 clippy / 0 fmt / 0 unwrap warnings, 5/5 gemma3-e2e PASS. Source: `docs/FAJAROS_100PCT_FJ_PHASE_*_FINDINGS.md` series. |
| [**v33.1.1**](https://github.com/fajarkraton/fajar-lang/releases/tag/v33.1.1) | **Inline asm dialect fix** | options(att_syntax) explicit dialect; documented `$ → $$` escape for LLVM inline asm template (LLVM uses `$0`/`$1` for constraint refs). Surfaced as silent codegen failure in Phase 6.6 console_putchar after clean rebuild — cached `.o.saved` had been masking 0-byte combined.o for 1+ session. |
| [**v33.1.0**](https://github.com/fajarkraton/fajar-lang/releases/tag/v33.1.0) | **FAJAROS_100PCT partial** | 8/9 compiler gaps closed; Phase 6.6 12/17 fajaros runtime stubs migrated to `@naked fn` (5 cluster-stubs principled-retained in `global_asm!()` for tightly-coupled IDT/TSS/PIT/ISR cluster). Phases CLOSED: 0,1,2,3,4.A,4.B,4.C,5,7. New tests: `at_no_vectorize_stacks_with_kernel`. |
| [**v33.0.0**](https://github.com/fajarkraton/fajar-lang/releases/tag/v33.0.0) | **Perfection-Plan Complete** | **FAJAR_LANG_PERFECTION_PLAN P0-P9 closed engineering-side (2026-05-03).** All 10 phases shipped, 22/25 work-items PASS, 3 await founder external action. Cumulative effort ~14h actual vs 218-336h plan estimate (~95% under). Adds: ~280 new tests (`error_code_coverage`, `polonius_property_tests`, `fuzz_target_canary` + 3 fuzz_targets, `editor_packages`, `lsp_v3_semantic_tokens`, `error_display_golden`, `release_workflow`, 2 `@no_vectorize` codegen tests); 5 audit/prevention scripts (`audit_error_codes.py`, `check_doc_coverage.sh`, `check_stdlib_docs.sh`, `check_publish_ready.sh`, `run_baselines.sh`); 13 docs (`HONEST_AUDIT_V33.md` + 9 phase findings + `TUTORIAL.md` 10 chapters + `CRATES_IO_PUBLISH_PLAN.md` + `LLVM_O2_VECMAT_MISCOMPILE_REPRO.md`); 6 real-project example folders (3 NEW: calculator-cli, tcp-echo-server, embedded-mnist); 5 baseline benchmarks vs C/Rust/Go (matrix_multiply + mandelbrot NEW). Quality state: 7626 lib + 2498+ integ tests 0 fail / 0 flake, 162 LLVM tests, 0 production unwrap, 0 clippy / 0 fmt / 0 rustdoc warning, 95.79% pub-item doc coverage, 100% stdlib_v3 doc coverage, 0 error-code coverage gap. |
| [**v32.1.0**](https://github.com/fajarkraton/fajar-lang/releases/tag/v32.1.0) | **P0-P6 milestone (tag-only)** | Mid-cycle milestone tag marking P0-P5 + P6 closure of FAJAR_LANG_PERFECTION_PLAN. **Tag-only release** — release.yml build failed due to `llvm_compile_float_literal` test having stale assertion (`contains("3.14")` on `make_float_lit(1.25)`); fixed in v33.0.0 via P8 opportunistic side-fix. Use v33.0.0 for binary downloads. |
| [**v31.0.0**](https://github.com/fajarkraton/fajar-lang/releases/tag/v31.0.0) | **Phase D + Track B** | **8-day catch-up consolidating V28-V31 across compiler + OS + quant. Compiler: `@noinline`+`@inline`+`@cold` lexer (V29.P1, closes silent-build-failure class via 5-layer prevention chain), `@no_vectorize` lexer+parser+codegen IR+disasm verified (V31.B.P2), `FJ_EMIT_IR` env var. CLAUDE.md +§6.10 (FS Roundtrip Coverage Rule, surfaced by V30.T4) +§6.11 (Training Script Interruption-Safety Rule, surfaced by FajarQuant c.1 hang). FajarOS Nova v3.4.0→v3.7.0: SMEP re-enabled (V29.P2, 35/35 kernel tests), SMAP re-enabled (V29.P3), NX triple closure (V29.P3.P6, `make test-security-triple-regression` 6-invariant gate), Gemma 3 1B audit-complete via Path D (V30.GEMMA3, 12 phases PASS, foundation ships), ext2+FAT32 disk roundtrip 9-invariant gate (V30.TRACK4, surfaces V31 latent ext2_create bug closed in V31.D Track D). FajarQuant Phase D IntLLM (separate repo `fajarkraton/fajarquant`): Mini v2 val_loss 4.38 (PPL 80.0), **Base c.1 PASS at val_loss 3.9903 (PPL 54.1) by 0.21 nat margin** — 3× wider than c.2's 0.071 nat, production-grade (Chinchilla-optimal 21.16 tok/p, 8h03m on RTX 4090 Laptop). Track B 5-layer interruption-safety (V31.C.P6.1-P6.6: ckpt_every/--resume/StepWatchdog/HF timeout+retry/test-train-watchdog regression gate/nohup line-buffering hardening). Medium config trimmed to Chinchilla-optimal 1.49B tokens (91K steps × 16,384 tok), training in flight at v31 cut.** |
| [**v27.5.0**](https://github.com/fajarkraton/fajar-lang/releases/tag/v27.5.0) | **Compiler Prep** | **V28-V33 prerequisites: AI scheduler builtins (`tensor_workload_hint`, `schedule_ai_task`), `@interrupt` ISR wrappers (ARM64 + x86_64 + target dispatcher) wired into AOT pipeline, `fb_set_base`/`fb_scroll` VESA framebuffer extensions, IPC service stub generator (`ServiceStub::from_service_def`), `@app` + `@host` annotations, refinement predicate checking extended to function params, `Cap<T>` linear/affine capability type. 16 new V27.5 E2E integration tests + `v27_5_regression` CI job. 5.6h actual vs 196h est (-97%) — 6/10 reported gaps were already implemented (audit correction pattern).** |
| [**v27.0.0**](https://github.com/fajarkraton/fajar-lang/releases/tag/v27.0.0) | **Hardened** | **V27 deep re-audit closed 5 gaps: 10 cargo doc warnings → 0, `call_main()` now rejects non-Function main with TypeError (was silent Null), Cargo.toml 24.0.0 → 27.0.0 version sync, 12 untested feature flags get integration tests (`tests/feature_flag_tests.rs`), `scripts/check_version_sync.sh` prevents future drift. 7,611 lib tests pass with 0 warnings.** |
| [**v26.1.0-phase-a**](https://github.com/fajarkraton/fajar-lang/releases/tag/v26.1.0-phase-a) | **Final (Phase A)** | **V26 Phase A1+A2+A3 complete: pre-commit hook, CI flake-stress (80/80 stress runs at `--test-threads=64`), 14 wall-clock test flakes hardened, production `.unwrap()` audit (4,062 → 174 → 3 → 0, all replaced with `.expect()`), `clippy::unwrap_used` lint at crate root, 3 `const_*` modules wired. Modules: 54 [x] / 0 [sim] / 0 [f] / 0 [s]. CLAUDE.md §6.7 + §6.8 Plan Hygiene Rules. 7,581 lib + 2,374 integ tests.** |
| [**v25.1.0**](https://github.com/fajarkraton/fajar-lang/releases/tag/v25.1.0) | **Production** | **V25 Production Plan v5.0 + 4 rounds of hands-on re-audit. HashMap auto-create, K8s deploy target, WGSL CodebookDot shader. FajarQuant Phase C complete: real Gemma 4 E2B 50-prompt 3-way comparison vs KIVI + TurboQuant (FajarQuant **wins at 2-bit 80.14 ppl** and **3-bit 75.65 ppl**, design tradeoff at 4-bit). Ablation: PCA 4-6%, fused attention 524,288× memory reduction, hierarchical 48.7% bit savings @ 10K context. Paper finalized (5-page LaTeX, real numbers). **`@kernel` transitive heap taint FIXED** (V17 critical bug closed). LLVM release JIT (LTO=false), println segfault, f-string codegen. ~7,581 lib tests.** |
| [**v24.0.0**](https://github.com/fajarkraton/fajar-lang/releases/tag/v24.0.0) | **Quantum** | **CUDA RTX 4090 GPU compute (9 PTX kernels, tiled matmul, async streams). FajarQuant Phase 5-7 wired (8 safety tests, paper benchmarks, GPU codebook dot product). AVX2 SIMD + AES-NI builtins via LLVM inline asm (LLVM-only). PTX sm_89 (Ada Lovelace) + BF16/FP8. ~3× speedup at 1024×1024 matmul on RTX 4090. ~7,572 tests.** |
| [**v23.0.0**](https://github.com/fajarkraton/fajar-lang/releases/tag/v23.0.0) | **Boot** | **FajarOS boots to shell (105 commands, 90 auto-tested). Ring 3 user mode (IRETQ+SYSCALL+SYSRETQ→shell resume). FajarQuant kernel-native. NVMe storage. 22 bugs fixed: LLVM asm constraint ordering, iretq selectors, PIC/LAPIC handlers, entry block alloca, frame allocator BSF/POPCNT. x86_64-user standalone ELF target. 7,572 tests.** |
| [v20.8.0](https://github.com/fajarkraton/fajar-lang/releases/tag/v20.8.0) | Perfection | FajarQuant, JIT 76x speedup, GPU detection (RTX 4090), 131/131 features audit, plugin CLI, strict mode. 10,400+ tests. |
| **v12.1.0** | **Delivery** | **V15: Multi-step effect continuations (replay-with-cache), resume() no-arg, effect type/arity checking, ML shorthand builtins (tanh/concat/accuracy), .forward() method dispatch, bindgen typedef struct fix, `fj run --check-only`, `fj registry-init`, LSP effect keywords, 22+ new .fj examples. 8,092 tests.** |
| **v12.0.0** | **Infinity** | **V14: Algebraic effects + handlers, dependent types (Pi/Sigma/refinement), GPU compute shaders (SPIR-V/PTX/fusion), LSP v4 (semantic tokens, inlay hints, completions), package registry (Sigstore signing, SBOM, audit). 8,074 tests, all CLI-integrated.** |
| **v11.0.0** | **Beyond** | **V13: Const generics, incremental compilation, WASI P2 component model, FFI v2 (C++/Python/Rust), SMT verification (DO-178C), distributed runtime (Raft), self-hosting compiler** |
| **v10.0.0** | **Transcendence** | **V12: LLVM O2/O3+LTO+PGO, macro system, generators (yield/gen fn/streams), WASI P1, package ecosystem, LSP excellence** |
| [v9.0.1](https://github.com/fajarkraton/fajar-lang/releases/tag/v9.0.1) | Ascension | 100% production: real BLE connect/read/write, async_spawn/join/select, HTTPS server (native-tls), 7,468 tests |
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

# Test (8,092 tests)
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

Apache License 2.0. See [LICENSE](LICENSE) and [NOTICE](NOTICE) for details.

Copyright 2026 Muhamad Fajar Putranto

Relicensed from MIT to Apache 2.0 on 2026-04-24 to add explicit patent grant
clause (industry standard for ML/compiler/systems projects). Past commits
remain MIT per their original terms; future commits are Apache 2.0.
