# Performance Guide

This guide covers profiling, benchmarking, and optimization techniques
for Fajar Lang programs.

## Benchmarking with fj bench

Create benchmark files in `benches/`:

```fajar
@bench
fn bench_matrix_multiply() {
    let a = randn(256, 256)
    let b = randn(256, 256)
    matmul(a, b)
}

@bench
fn bench_sort_10k() {
    let data = randn_vec(10000)
    sort(data)
}
```

Run benchmarks:

```bash
fj bench                          # Run all benchmarks
fj bench --output results.json    # Export results
```

## Profiling with fj profile

Profile your program to find hotspots:

```bash
fj profile run my_app.fj                     # Text report
fj profile run my_app.fj --format chrome     # Chrome DevTools format
fj profile run my_app.fj --format speedscope # Speedscope format
```

The profiler tracks function entry/exit times, allocation counts,
and call frequencies.

### Reading Profile Output

```
Function                    Calls    Total(ms)   Self(ms)   Allocs
-----------------------------------------------------------------
main                            1      245.3       0.2         3
  process_batch                50      240.1       5.0       150
    matmul                   5000      180.5     180.5         0
    softmax                  5000       54.6      54.6         0
```

Focus on functions with high `Self(ms)` -- those are the true hotspots.

## Optimization Techniques

### Use Native Compilation

The tree-walking interpreter is 10-50x slower than native:

```bash
fj build --release           # Cranelift-compiled native binary
```

### Avoid Unnecessary Allocations

```fajar
// Slow: allocates a new string each iteration
for i in 0..1000 {
    let s = f"Item {i}"
    process(s)
}

// Better: reuse a buffer
let mut buf = String::with_capacity(64)
for i in 0..1000 {
    buf.clear()
    buf.push_str(f"Item {i}")
    process(buf)
}
```

### Prefer Stack Over Heap

```fajar
// Heap-allocated array
let data = [0; 10000]

// Stack-allocated fixed array (faster for small sizes)
let data: [i32; 64] = [0; 64]
```

### Batch Tensor Operations

```fajar
// Slow: element-wise loop
for i in 0..len(tensor) {
    tensor[i] = tensor[i] * 2.0
}

// Fast: vectorized operation
let result = tensor * 2.0
```

### Use SIMD When Available

```fajar
use std::simd

let a = simd::f32x8::load(data_a)
let b = simd::f32x8::load(data_b)
let c = a + b  // 8 additions in one instruction
c.store(result)
```

## Compiler Optimizations

Fajar Lang applies these optimizations at `--release`:

| Optimization | Description |
|-------------|-------------|
| Dead function elimination | Removes unused functions |
| LICM | Moves loop-invariant code outside loops |
| CSE | Eliminates common subexpressions |
| Inlining | Inlines small functions at call sites |
| Constant folding | Evaluates constant expressions at compile time |

## Performance Comparison

| Operation | Interpreter | Bytecode VM | Native (Cranelift) |
|-----------|-------------|-------------|--------------------|
| fib(30) | ~500ms | ~80ms | ~5ms |
| Matrix 256x256 | ~2s | ~800ms | ~50ms |
| Sort 10K | ~150ms | ~30ms | ~2ms |
