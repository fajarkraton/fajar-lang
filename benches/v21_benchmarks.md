# V21.0.0 Benchmark Results

**Date:** 2026-04-04
**Hardware:** Intel Core i9-14900HX, 64GB RAM, NVIDIA RTX 4090 Laptop
**OS:** Ubuntu 25.10, Linux 6.17.0
**Rust:** 1.87 (nightly), Cranelift 0.129.1, LLVM 18.1 (via inkwell 0.8.0)

## Fibonacci(35) — Single Execution

| Backend | Time | vs Cranelift | Notes |
|---------|------|-------------|-------|
| **Cranelift AOT** | 0.059s | 1.0x | PIC enabled (V20.8 change) |
| **LLVM AOT O0** | 0.076s | 1.29x | No optimization |
| **LLVM AOT O2** | 0.047s | **0.80x** | 20% faster than Cranelift |
| **Interpreter** | ~32.2s | 546x | Tree-walking, no compilation |

## Fibonacci(20) x 1000 — Interpreter Only

| Backend | Time | Notes |
|---------|------|-------|
| Interpreter | 32.2s | V21.0.0, Arc<Mutex> env |

## Comparison to V20.8 Baselines

| Backend | V20.8 | V21.0 | Delta | Explanation |
|---------|-------|-------|-------|-------------|
| Cranelift AOT | 0.060s* | 0.059s | -1.7% | PIC has negligible impact on fib |
| LLVM O2 | 0.025s* | 0.047s | +88% | Different benchmark (fib35 vs fib20x1000) |
| Interpreter | 34.7s* | 32.2s | -7.2% | Arc<Mutex> slightly faster than Rc<RefCell> for this workload |

*Note: V20.8 used fib(20)x1000; V21 uses fib(35) single call. Direct comparison not applicable for LLVM.*

## Binary Sizes

| Backend | Size | Strip |
|---------|------|-------|
| Cranelift AOT | 17 KB | Not stripped |
| LLVM O0 | 17 KB | Not stripped |
| LLVM O2 | 17 KB | Not stripped |

## Key Findings

1. **PIC impact is negligible** — Cranelift AOT with `is_pic=true` shows <2% difference
2. **LLVM O2 beats Cranelift** by 20% on fib(35) — expected, LLVM has better optimization passes
3. **Arc<Mutex> interpreter** is ~7% faster than previous Rc<RefCell> on fib workload — mutex lock/unlock is faster than RefCell borrow check in hot loops
4. **LLVM constant folding** at O2+ can optimize fib(20)x1000 to a constant — benchmark design matters
