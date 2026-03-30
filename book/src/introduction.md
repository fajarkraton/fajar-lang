# Fajar Lang

**The systems programming language for embedded ML + OS integration.**

Fajar Lang (`fj`) is a statically-typed systems programming language designed for the unique intersection of embedded systems and machine learning. It's the only language where an OS kernel and a neural network can share the same codebase, type system, and compiler — with safety guarantees that no existing language provides.

## Why Fajar Lang?

| Challenge | Other Languages | Fajar Lang |
|-----------|----------------|------------|
| Embedded ML inference | Python (too slow), C (unsafe) | First-class tensor types, no heap at inference |
| OS + ML in one binary | Separate languages, FFI glue | `@kernel` + `@device` contexts, compiler-enforced isolation |
| Safety on hardware | Runtime checks or nothing | Compile-time context checking, move semantics |
| Cross-compilation | Complex toolchain setup | `fj build --target aarch64-unknown-none-elf` |

## Design Principles

1. **Explicitness over magic** — no hidden allocation or hidden cost
2. **Dual-context safety** — `@kernel` disables heap+tensor; `@device` disables raw pointers
3. **Rust-inspired but simpler** — ownership lite without lifetime annotations
4. **Native tensor types** — `Tensor` is a first-class citizen in the type system

## Quick Example

```fajar
// Classify sensor data using a pre-trained neural network
fn classify(sensor: Tensor, w1: Tensor, b1: Tensor, w2: Tensor, b2: Tensor) -> i64 {
    let z1 = tensor_add(tensor_matmul(sensor, w1), b1)
    let a1 = tensor_relu(z1)
    let z2 = tensor_add(tensor_matmul(a1, w2), b2)
    let output = tensor_softmax(z2)
    tensor_argmax(output)
}

fn main() {
    let sensor = read_imu()
    let class = classify(sensor, w1, b1, w2, b2)
    if class == 2 {
        activate_alert()
    }
}
```

## Target Audience

- **Embedded AI engineers** — drone, robot, IoT
- **OS research teams** — AI-integrated kernels
- **Safety-critical ML systems** — automotive, aerospace, medical

## Current Status

Fajar Lang v9.0.1 "Ascension" is the latest stable release — **100% production-ready**:

- **~340,000 lines of Rust** — 7,468 tests, 0 clippy warnings, 37 test suites
- **178 example programs** — 175 pass `fj check` (3 self-hosting by design)
- **Native codegen** — Cranelift JIT+AOT, LLVM backend (O0-O3), WebAssembly
- **Full ML stack** — tensors, autograd, training, MNIST 90%+, ONNX export
- **Async/Await** — real tokio I/O (sleep, http_get/post, spawn, join, select)
- **Networking** — WebSocket (tungstenite+TLS), MQTT (rumqttc), BLE (btleplug), HTTP/HTTPS
- **HTTP framework** — router + middleware + handler dispatch + HTTPS (native-tls)
- **Regex** — core stdlib (match, find, replace, captures) with compiled cache
- **GUI** — winit windowing, bitmap font text, button hover/click, flex layout
- **OS development** — FajarOS Nova (x86_64) + FajarOS Surya (ARM64)
- **Embedded targets** — ARM64, RISC-V cross-compilation, no_std, HAL traits
- **37 standard packages** — math, nn, hal, http, json, crypto, mqtt, db
- **Developer tools** — LSP (inlay hints, signature help), VS Code extension, formatter, test framework, doc generator
- **Distributed computing** — real TCP RPC, async actors (tokio), clustering
- **Security** — stack canary in Cranelift, bounds check, overflow check, linter, taint analysis
- **Profiling** — function-level profiling in interpreter + Cranelift native code
- **Time-travel debugger** — record/replay, reverse stepping, flame graphs

## About the Author

**Muhamad Fajar Putranto, SE., SH., MH.** — serial entrepreneur with 28+ years of professional experience spanning taxation, technology, and innovation.

| Year | Milestone |
|------|-----------|
| 1996 | Started career at Indonesia's Directorate General of Taxes (DJP) |
| ~2008 | Founded **IndoFace** — Indonesian social network with **180,000+ members**. One of Indonesia's pioneering homegrown social platforms |
| 2012 | Co-founded **TaxPrime** — Indonesia's largest independent tax consulting firm (240+ professionals). Collaborating firm of Andersen Global and member of PrimeGlobal |
| 2012 | Founded **PrimeCore.id** — Global Business Solutions. Strategic business development, ESG & sustainability advisory, legal restructuring |
| 2021-2024 | **ITR World Tax "Highly Regarded"** — Tax Controversy Leader, 4 consecutive years |
| 2023 | Founded **InkubatorX** — ecosystem company (digital economy, renewable energy, nano technology) |
| 2023 | **Wakil Ketua Dewan Pembina ACEXI** — Vice Chair, Carbon Emission Expert Association of Indonesia |
| 2025 | Elected **Ketua Umum IKANAS STAN** 2025-2028 — leading ~80,000 alumni |
| 2026 | Created **Fajar Lang** — systems programming language for embedded ML & OS development |

> *"From tax auditor to social network founder, from global business advisory to carbon emission governance, to programming language creator — the journey of building things that matter."*
