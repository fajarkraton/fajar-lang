# BENCHMARKS

> Performance Targets & Measurement — Fajar Lang v3.0

---

## 1. Performance Philosophy

Prioritas: **CORRECTNESS > SAFETY > PERFORMANCE**

Fajar Lang menyediakan 4 execution backend dengan trade-off berbeda:

| Backend | Speed | Startup | Use Case |
|---------|-------|---------|----------|
| Interpreter (tree-walk) | Slow | Instant | REPL, debugging, prototyping |
| Bytecode VM | Medium | Fast | Portable execution |
| Cranelift (JIT/AOT) | Fast | Medium | Development, embedded |
| LLVM (AOT) | Fastest | Slow | Production, release builds |

---

## 2. Compiler Pipeline Benchmarks

| Benchmark | Input | Target |
|-----------|-------|--------|
| Lexer throughput | 10K lines | < 10ms |
| Parser throughput | 10K lines | < 20ms |
| Analyzer throughput | 10K lines | < 50ms |
| Full pipeline (Cranelift JIT) | hello.fj | < 5ms |
| Full pipeline (LLVM AOT) | hello.fj | < 500ms |
| Incremental recompile | Single file change | < 100ms |
| REPL response | Single expression | < 10ms |
| Error reporting | File with 10 errors | < 10ms |

---

## 3. Execution Benchmarks

| Benchmark | Interpreter | Native (Cranelift) | Native (LLVM -O2) |
|-----------|------------|-------------------:|-------------------:|
| fibonacci(20) | ~26ms | < 1ms | < 0.5ms |
| fibonacci(30) | ~500ms | < 5ms | < 2ms |
| factorial(20) | ~10ms | < 0.1ms | < 0.05ms |
| Loop 1M iterations | ~2s | < 5ms | < 1ms |
| String concat 10K | ~1s | < 50ms | < 20ms |
| Array sort 10K | ~2s | < 100ms | < 50ms |
| Struct creation 100K | ~3s | < 150ms | < 80ms |

---

## 4. Tensor/ML Benchmarks

| Benchmark | CPU (ndarray) | GPU (wgpu/CUDA) |
|-----------|--------------|-----------------|
| matmul 100x100 | < 1ms | < 0.1ms |
| matmul 1000x1000 | < 100ms | < 5ms |
| Forward MNIST (784-128-10) | < 5ms | < 0.5ms |
| Backward MNIST | < 10ms | < 1ms |
| XOR training 1K epochs | < 500ms | < 50ms |
| MNIST training 1 epoch | < 30s | < 3s |
| relu 1M elements | < 1ms | < 0.1ms |
| softmax 10K elements | < 1ms | < 0.1ms |
| INT8 inference (784-128-10) | < 0.5ms | N/A |
| LSTM forward (seq=32) | < 10ms | < 1ms |

---

## 5. Memory Usage Benchmarks

| Scenario | Target Memory |
|----------|---------------|
| Compiler idle | < 10 MB |
| Compile 1K line file | < 50 MB |
| Compile 10K line file | < 200 MB |
| REPL session (1 hour) | < 100 MB |
| Tensor 1000x1000 f32 | ~4 MB |
| Training loop 100 epochs | Stable (no leak) |
| LSP server idle | < 50 MB |
| LSP with 100 files | < 200 MB |

---

## 6. Cross-Compilation Benchmarks

| Target | Build Time | Binary Size |
|--------|-----------|-------------|
| x86_64-linux (native) | Baseline | ~8 MB |
| aarch64-linux | +10% | ~8 MB |
| riscv64-linux | +15% | ~9 MB |
| thumbv7em-none (bare metal) | +5% | < 1 MB |
| wasm32-wasi | +20% | < 2 MB |

---

## 7. Measurement Tools

```bash
# Criterion benchmarks
cargo bench

# Run with timing
fj run --time examples/fibonacci.fj

# Profile with flamegraph
cargo install flamegraph
cargo flamegraph --bin fajar-lang -- run examples/fibonacci.fj

# Memory profiling
cargo install cargo-valgrind
cargo valgrind test

# Built-in profiler
fj run --profile examples/mnist.fj
# Outputs: call counts, avg time per function, hotspots
```

---

*Benchmarks Version: 3.0 | Updated: 2026-03-12 (v3.0 — all backends, GPU, cross-compilation)*
