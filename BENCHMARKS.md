# Fajar Lang Benchmarks

Performance measurements of Fajar Lang programs compared to equivalent
implementations in Rust, C, and Python.

## Test Environment

- **Hardware:** AMD/Intel x86_64, 32GB RAM
- **OS:** Linux 6.17
- **Fajar Lang:** v7.0.0 (interpreter mode)
- **Rust:** 1.87.0 (release build)
- **GCC:** 14.x (-O2)
- **Python:** 3.12

## Microbenchmarks

| Benchmark | Fajar (interp) | Fajar (native) | Rust | C | Python |
|-----------|---------------:|---------------:|-----:|--:|-------:|
| fibonacci(30) recursive | — ms | — ms | — ms | — ms | — ms |
| fibonacci(40) iterative | — ms | — ms | — ms | — ms | — ms |
| quicksort 10K elements | — ms | — ms | — ms | — ms | — ms |
| string concat 1K | — ms | — ms | — ms | — ms | — ms |
| matrix 64x64 multiply | — ms | — ms | — ms | — ms | — ms |
| pattern match 10K | — ms | — ms | — ms | — ms | — ms |
| closure calls 100K | — ms | — ms | — ms | — ms | — ms |
| mandelbrot 80x80 | — ms | — ms | — ms | — ms | — ms |
| n-body 1K steps | — ms | — ms | — ms | — ms | — ms |
| binary trees depth 14 | — ms | — ms | — ms | — ms | — ms |

> Results marked `—` will be filled when benchmarks are run on the target machine.
> Run `examples/benchmarks/run_benchmarks.sh` to generate results.

## Application Benchmarks

| Benchmark | Fajar (interp) | Fajar (native) | Notes |
|-----------|---------------:|---------------:|-------|
| Tokenize 10K lines | — ms | — ms | Lexer performance |
| Parse 10K lines | — ms | — ms | Parser performance |
| Type check 10K lines | — ms | — ms | Analyzer performance |
| MNIST 1 epoch | — ms | — ms | ML training pipeline |
| HTTP request/response | — ms | — ms | Network I/O |

## Running Benchmarks

```bash
# Run all microbenchmarks (interpreter mode)
./examples/benchmarks/run_benchmarks.sh

# Run one specific benchmark
./examples/benchmarks/run_benchmarks.sh fib_recursive

# Compiler internal benchmarks (criterion)
cargo bench
```

## Benchmark Programs

All benchmark source files are in `examples/benchmarks/`:

| File | Description |
|------|-------------|
| `fib_recursive.fj` | Recursive Fibonacci — function call overhead |
| `fib_iterative.fj` | Iterative Fibonacci — loop performance |
| `quicksort.fj` | Quicksort — array ops + recursion |
| `string_concat.fj` | String concatenation — allocation pressure |
| `matrix_multiply.fj` | Matrix multiply — tensor/ndarray backend |
| `pattern_match.fj` | Enum pattern matching — dispatch overhead |
| `closure_overhead.fj` | Closure calls — closure dispatch cost |
| `mandelbrot.fj` | Mandelbrot fractal — floating-point + loops |
| `nbody.fj` | N-body simulation — physics computation |
| `binary_trees.fj` | Binary tree checksum — allocation + recursion |

## Historical Performance

Performance tracking across releases:

| Version | fibonacci(30) | mandelbrot 80x80 | Tests |
|---------|-------------:|------------------:|------:|
| v0.1 | ~26ms | — | 100 |
| v1.0 | — | — | 1,563 |
| v6.1 | — | — | 5,563 |

> Historical data will be populated as benchmarks are run across versions.

## Optimization Guide

For best performance in Fajar Lang:

1. **Use native compilation:** `fj build --release program.fj` (10-100x faster)
2. **Prefer iteration over recursion:** Loop-based code avoids call overhead
3. **Use tensor ops for math:** `matmul()`, `randn()` use optimized ndarray backend
4. **Minimize string allocation:** Pre-allocate or use `f""` formatting
5. **Enable optimizations:** `fj build --opt-level 2` runs constant folding, dead code elimination
