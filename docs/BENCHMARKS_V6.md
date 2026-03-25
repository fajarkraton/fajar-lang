# Fajar Lang v6.1.0 — Benchmark Results

> **Date:** 2026-03-25
> **Platform:** Intel Core i9-14900HX, 32GB DDR5, Linux 6.17.0
> **Binary:** 7.8 MB release build (Cranelift backend)

---

## Interpreter Benchmarks

| Benchmark | Time | Notes |
|-----------|------|-------|
| fib(30) recursive | **2.70s** | 832,040 calls, tree recursion |
| Array sort (10 elements) | **<1ms** | Built-in sort |
| Array sort (20 elements) | **<1ms** | Built-in sort |
| Pipeline (filter→map→sum, 20 elements) | **<1ms** | 3 chained closures |
| Map + fold (5! = 120) | **<1ms** | Closure accumulation |
| Sum of squares (1-10) | **<1ms** | map + sum |

## Compilation Benchmarks

| Operation | Time |
|-----------|------|
| `cargo build --release` | ~47s |
| `fj dump-tokens` (21K lines Nova kernel) | <1s |
| `fj run hello.fj` | <5ms |
| REPL startup | <3ms |
| LSP server startup | <5ms |

## Quality Metrics

| Metric | Value |
|--------|-------|
| Tests | 5,685+ passing (0 failures) |
| Fuzz runs | 2,357,767 (0 crashes) |
| Clippy warnings | 0 |
| Binary size | 7.8 MB |
| Integration tests | 923 |
| Property tests | 33 |

## Example Programs

| Example | Lines | Time | Output |
|---------|-------|------|--------|
| hello.fj | 3 | <1ms | "Hello from Fajar Lang!" |
| fibonacci.fj | 8 | <1ms | First 10 fibonacci numbers |
| array_methods.fj | 55 | <1ms | 22 method demonstrations |
| gat_demo.fj | 55 | <1ms | Traits, builder, pipelines |
| effects_demo.fj | 50 | <1ms | Result/Option handling |
| comptime_demo.fj | 40 | <1ms | Const fn, factorial, fibonacci |
| simd_demo.fj | 50 | <1ms | Vector ops, dot product |
| bench_pipeline.fj | 30 | <1ms | Filter→map→fold chains |

## FajarOS Nova Kernel

| Metric | Value |
|--------|-------|
| Kernel size | 21,187 LOC |
| Lex time | <1s (123,378 tokens) |
| Modular build (139 files) | <2s concatenation |
| Syscalls | 50 |
| Shell commands | 280+ |

---

*Benchmarks collected on 2026-03-25 with Fajar Lang v6.1.0*
