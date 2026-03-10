# BENCHMARKS

> Performance Targets & Measurement — Fajar Lang

---

## 1. Performance Philosophy

Fajar Lang Phase 1-4 menggunakan tree-walking interpreter, sehingga performance TIDAK menjadi prioritas utama. Prioritas saat ini: **CORRECTNESS > SAFETY > PERFORMANCE**.

> **Phase 5:** Bytecode VM dan LLVM backend akan meningkatkan performance secara signifikan. Target Phase 5: 10-100x improvement dari tree-walking.

---

## 2. Compiler Pipeline Benchmarks

| Benchmark | Input | Target (Phase 1) | Target (Phase 5) |
|-----------|-------|-------------------|-------------------|
| Lexer throughput | 10K lines source | < 50ms | < 5ms |
| Parser throughput | 10K lines source | < 100ms | < 10ms |
| Analyzer throughput | 10K lines source | < 200ms | < 20ms |
| Full pipeline | hello.fj (10 lines) | < 10ms | < 1ms |
| Full pipeline | fibonacci.fj (20 lines) | < 50ms | < 5ms |
| REPL response | Single expression | < 100ms | < 10ms |
| Error reporting | File with 10 errors | < 100ms | < 10ms |

---

## 3. Interpreter Benchmarks

| Benchmark | Description | Target (Tree-walk) | Target (Bytecode) |
|-----------|-------------|---------------------|---------------------|
| fibonacci(30) | Recursive fibonacci | < 500ms | < 50ms |
| factorial(20) | Recursive factorial | < 10ms | < 1ms |
| loop 1M iterations | Simple counter | < 2s | < 100ms |
| string concat 10K | Build large string | < 1s | < 100ms |
| array sort 10K | In-place sort | < 2s | < 200ms |
| struct creation 100K | Allocate + init | < 3s | < 300ms |

---

## 4. Tensor/ML Benchmarks

| Benchmark | Description | Target (CPU) | Target (GPU, Phase 5) |
|-----------|-------------|--------------|------------------------|
| matmul 100x100 | Matrix multiply | < 1ms | < 0.1ms |
| matmul 1000x1000 | Large matmul | < 100ms | < 5ms |
| forward MNIST | 784→128→10 | < 5ms | < 0.5ms |
| backward MNIST | Full backprop | < 10ms | < 1ms |
| XOR training 1K epochs | 2→8→1 network | < 500ms | < 50ms |
| MNIST training 1 epoch | 60K samples | < 30s | < 3s |
| relu 1M elements | Activation function | < 1ms | < 0.1ms |
| softmax 10K elements | With exp + sum | < 1ms | < 0.1ms |

---

## 5. Memory Usage Benchmarks

| Scenario | Target Memory |
|----------|---------------|
| Compiler idle (loaded, no file) | < 10 MB |
| Compile 1K line file | < 50 MB |
| Compile 10K line file | < 200 MB |
| REPL session (1 hour) | < 100 MB |
| Tensor 1000x1000 float32 | ~4 MB (exact: 4,000,000 bytes) |
| Training loop 100 epochs (no leak) | Memory stable, not growing |

---

## 6. Measurement Tools

```bash
# Criterion benchmarks
cargo bench

# Memory profiling
cargo install cargo-valgrind
cargo valgrind test

# Flame graph
cargo install flamegraph
cargo flamegraph --bin fajar-lang -- run examples/fibonacci.fj

# Custom timing dalam code
use std::time::Instant;
let start = Instant::now();
// ... operation ...
println!("Took: {:?}", start.elapsed());
```

---

*Benchmarks Version: 1.0 | Akan diupdate setiap phase transition*
