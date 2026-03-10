# Benchmark Baselines: Fajar vs C vs Rust

## Setup
- Platform: x86_64 Linux 6.17.0
- C compiler: gcc -O2
- Rust compiler: rustc -O (--release equivalent)
- Fajar: Cranelift JIT (OptLevel::Speed)

## Results (us/iter, lower is better)

| Benchmark | C (gcc -O2) | Rust (-O) | Fajar (default) | Fajar (speed) |
|-----------|-------------|-----------|-----------------|---------------|
| fib(20) | 9.74 | 0.03* | TBD | TBD |
| sum(0..10000) | <0.001** | <0.001** | TBD | TBD |
| bubble_sort(10) | 0.165 | <0.001** | TBD | TBD |

### Notes

- `*` Rust achieves near-zero time for fib(20) due to LLVM aggressive optimization
  (likely tail-call optimization or compile-time evaluation). This is not representative
  of actual recursive fib performance.
- `**` Both C and Rust constant-fold the sum loop and bubble sort at -O2/release level,
  computing the result at compile time. This shows the power of mature optimizers.
- Fajar uses Cranelift which has simpler optimizations than LLVM. The comparison is
  most fair for recursive algorithms where compile-time evaluation is not possible.

## How to Run

```bash
# C baselines
cd benches/baselines
gcc -O2 -o fib fibonacci.c && ./fib
gcc -O2 -o sum sum_loop.c && ./sum
gcc -O2 -o bubble bubble_sort.c && ./bubble

# Rust baselines
rustc -O -o fib_rs fibonacci.rs && ./fib_rs
rustc -O -o sum_rs sum_loop.rs && ./sum_rs
rustc -O -o bubble_rs bubble_sort.rs && ./bubble_rs

# Fajar (via criterion)
cargo bench --features native
```
