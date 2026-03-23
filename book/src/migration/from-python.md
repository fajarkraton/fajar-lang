# Fajar Lang for Python/ML Developers

Fajar Lang combines Python's ML ergonomics with systems-level performance.

## Why Switch from Python?

| Python | Fajar Lang |
|--------|-----------|
| Interpreted (slow) | Compiled (fast — Cranelift/LLVM) |
| GIL limits parallelism | Real threads + async/await |
| No type safety | Static types, compile-time checks |
| pip install | `fj add` (with PubGrub resolution) |
| PyTorch/TensorFlow | Built-in tensor types |
| Can't write OS code | `@kernel` for bare-metal |

## Familiar ML Patterns

```fajar
// Tensor operations (like NumPy/PyTorch)
let x = zeros(3, 4)              // 3x4 matrix
let w = randn(4, 2)              // random weights
let y = matmul(x, w)             // matrix multiply
let a = relu(y)                  // activation
let s = softmax(a)               // probabilities

// Autograd (like PyTorch)
set_requires_grad(w)
let loss = cross_entropy(pred, target)
backward(loss)
let g = grad(w)                  // gradients
```

## Key Differences from Python

### Static Types (but with inference)

```fajar
// Types are inferred when possible
let x = 42              // i64
let name = "Fajar"      // str
let pi = 3.14           // f64

// Explicit when needed
let count: i64 = 0
fn add(a: i64, b: i64) -> i64 { a + b }
```

### Ownership (no garbage collector)

```fajar
let data = [1, 2, 3]
let copy = data          // data is moved!
// process(data)         // ERROR: data was moved

// Use references to share without moving:
fn sum(arr: &[i64]) -> i64 { ... }
```

### Compile-Time ML Constants

```fajar
// Precompute lookup tables at compile time
comptime fn sigmoid_table() -> [f64; 256] {
    // Generated at compile time — zero runtime cost!
}

const TABLE: [f64; 256] = comptime { sigmoid_table() }
```

### Deploy to Hardware

```fajar
@device fn inference(input: Tensor) -> Tensor with Tensor {
    // This runs on embedded hardware (ARM64, RISC-V)
    // Compiler guarantees: no hardware access in @device
    matmul(input, weights)
}

@kernel fn read_sensor() -> [f64; 4] with Hardware {
    // Read from hardware sensor
    // Compiler guarantees: no tensor ops in @kernel
}
```

## Python → Fajar Lang Cheat Sheet

```fajar
// Python: x = [1, 2, 3]
let x = vec![1, 2, 3]

// Python: for i in range(10):
for i in 0..10 { println(i) }

// Python: if x > 0: ... else: ...
let result = if x > 0 { "positive" } else { "non-positive" }

// Python: def f(x): return x * 2
fn f(x: i64) -> i64 { x * 2 }

// Python: f"{name} is {age}"
let msg = f"Hello {name}"

// Python: import math
use std::math

// Python: print("hello")
println("hello")
```

## Performance Comparison

| Task | Python | Fajar Lang |
|------|--------|-----------|
| fibonacci(30) | ~500ms | ~0.4ms (native) |
| Matrix 1000x1000 | ~1s (NumPy) | ~2s (ndarray) |
| Cold start | ~50ms | ~4ms |
| Binary size | N/A (needs runtime) | ~80KB |
