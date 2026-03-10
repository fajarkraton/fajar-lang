# Fajar Lang v1.1 Roadmap

> Planned features and improvements for the next release after v1.0.

---

## Timeline

**Target release:** 3-4 months after v1.0

---

## 1. GPU Compute Backend

**Priority:** P0 — Most requested feature for ML workloads.

- [ ] WebGPU backend via `wgpu` + WGSL shader generation
- [ ] Tensor operations on GPU (matmul, conv2d, element-wise ops)
- [ ] Automatic host-device memory transfer
- [ ] `@gpu` context annotation for GPU-accelerated code blocks
- [ ] Benchmark: GPU vs CPU inference latency comparison
- [ ] Support for NVIDIA CUDA (via wgpu/Vulkan backend)

**Goal:** 10-100x speedup for large tensor operations on GPU-capable hardware.

---

## 2. Async / Await

**Priority:** P1 — Enables concurrent I/O and networking.

- [ ] `async fn` and `.await` syntax
- [ ] Lightweight task runtime (no OS threads for embedded)
- [ ] Cooperative scheduling for bare-metal targets
- [ ] `async trait` support
- [ ] Integration with IRQ system (interrupt-driven async)
- [ ] Async I/O: UART, SPI, I2C non-blocking operations

**Goal:** Enable concurrent sensor reading and ML inference without threads.

---

## 3. LLVM Backend

**Priority:** P1 — Better optimization than Cranelift for production builds.

- [ ] LLVM IR generation via `inkwell` (LLVM C API bindings)
- [ ] Full optimization pipeline (-O0 through -O3)
- [ ] Link-time optimization (LTO)
- [ ] Profile-guided optimization (PGO)
- [ ] Target support: x86_64, aarch64, riscv64, armv7, thumbv7
- [ ] `fj build --backend=llvm` CLI option

**Goal:** Production-quality native binaries with competitive performance.

---

## 4. Package Registry Hosting

**Priority:** P1 — Enable community package sharing.

- [ ] Central package registry server (`registry.fajarlang.dev`)
- [ ] `fj publish` uploads package to registry
- [ ] `fj install <package>` downloads and caches packages
- [ ] Package search and discovery (web UI)
- [ ] API key authentication for publishing
- [ ] Package README rendering and documentation
- [ ] Dependency vulnerability scanning

**Goal:** Enable the Fajar Lang ecosystem to grow with community contributions.

---

## 5. Full Borrow Checker

**Priority:** P2 — Stronger safety guarantees.

- [ ] Non-Lexical Lifetimes (NLL) — v1.0 has move semantics only
- [ ] Immutable borrow (`&T`) and mutable borrow (`&mut T`) tracking
- [ ] Borrow checker errors (ME002-ME008) fully enforced
- [ ] Lifetime elision rules (no explicit lifetime annotations needed)
- [ ] Reborrowing and two-phase borrowing
- [ ] Integration with trait system (borrowed trait objects)

**Goal:** Memory safety without garbage collection, Rust-level guarantees.

---

## 6. Advanced ML Features

**Priority:** P2 — Expand ML capabilities.

- [ ] Conv2d backward pass (gradient through convolution)
- [ ] Multi-head attention with reshape/split
- [ ] Recurrent layers (LSTM, GRU)
- [ ] DataLoader with multi-threaded prefetching
- [ ] Learning rate scheduling (step, cosine annealing, warmup)
- [ ] Mixed-precision training (FP16/BF16)
- [ ] ONNX model import/export
- [ ] TFLite model import for embedded deployment

**Goal:** Train and deploy production ML models entirely in Fajar Lang.

---

## 7. Self-Hosting

**Priority:** P3 — Language maturity milestone.

- [ ] Fajar Lang lexer written in Fajar Lang
- [ ] Fajar Lang parser written in Fajar Lang
- [ ] Bootstrap: compile self-hosted compiler with Rust compiler, then self-compile
- [ ] Binary reproducibility verification
- [ ] Self-hosted compiler passes full test suite

**Goal:** Prove the language is expressive enough to implement itself.

---

## 8. Tooling Improvements

**Priority:** P2 — Developer experience.

- [ ] `fj test` — built-in test runner with `#[test]` attribute
- [ ] `fj bench` — built-in benchmark framework
- [ ] `fj doc` — documentation generation from doc comments
- [ ] LSP: auto-completion, go-to-definition, rename refactoring
- [ ] Debugger integration (DAP protocol)
- [ ] `fj watch` — file watcher with auto-rebuild
- [ ] VS Code extension: syntax highlighting, inline diagnostics, code actions

**Goal:** IDE-quality developer experience comparable to Rust.

---

## 9. Embedded Ecosystem

**Priority:** P1 — Core differentiator.

- [ ] Board support packages (BSP) for popular boards:
  - STM32F4, STM32H7 (ARM Cortex-M)
  - ESP32 (Xtensa/RISC-V)
  - Raspberry Pi Pico (RP2040)
  - Arduino (AVR — stretch goal)
- [ ] Real hardware CI (GitHub Actions + self-hosted runners)
- [ ] RTOS integration (FreeRTOS task spawning from Fajar Lang)
- [ ] Over-the-air (OTA) update support for deployed ML models
- [ ] Power management APIs (sleep modes, wake sources)

**Goal:** Deploy Fajar Lang on real embedded hardware with production reliability.

---

## Contributing

See `docs/CONTRIBUTING.md` for how to contribute to Fajar Lang development.

Feature requests and bug reports: [GitHub Issues](https://github.com/user/fajar-lang/issues)

---

*ROADMAP_V1.1.md — Created 2026-03-06*
