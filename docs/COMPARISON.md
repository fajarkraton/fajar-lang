# Fajar Lang vs Other Languages — Feature Comparison

> For embedded ML + OS integration use cases.

---

## Language Comparison Matrix

| Feature | Fajar Lang | Rust | Zig | C++ | Python |
|---------|-----------|------|-----|-----|--------|
| **Type Safety** | Static, inferred | Static, inferred | Static, inferred | Static (weak) | Dynamic |
| **Memory Safety** | Ownership + move semantics | Ownership + lifetimes | Manual + safety checks | Manual (RAII) | GC |
| **Native Compilation** | Cranelift JIT/AOT | LLVM | LLVM + custom | LLVM/GCC | Interpreted |
| **First-class Tensors** | Yes (`tensor` type) | No (crate) | No (library) | No (library) | Yes (NumPy) |
| **Autograd** | Built-in (tape-based) | No (tch-rs) | No | No (LibTorch) | Yes (PyTorch) |
| **Bare-metal Support** | `@kernel` context | `#![no_std]` | `@import("std")` | Freestanding | No |
| **Context Isolation** | `@kernel/@device/@safe` | None (manual) | None | None | None |
| **Embedded ML** | Native (quantization, stack tensors) | Via crates | Via C libs | Via TFLite | No |
| **Compile-time Shape Check** | Planned (v1.1) | No | No | Template tricks | No |
| **Learning Curve** | Low (no lifetimes) | High (lifetimes) | Medium | High (templates) | Low |
| **Package Manager** | Built-in (`fj.toml`) | Cargo | Zig build | CMake/Conan | pip |
| **REPL** | Yes | No (evcxr) | No | No (cling) | Yes |
| **LSP Support** | Yes | Yes (rust-analyzer) | Yes (zls) | Yes (clangd) | Yes (Pylance) |

---

## Unique Fajar Lang Advantages

### 1. Context Isolation (No Other Language Has This)

```fajar
@kernel fn read_sensor() -> [f32; 4] {
    // Can use: raw pointers, IRQ, port I/O, page tables
    // Cannot use: heap allocation, tensors, strings
    let data = port_read(0x68)
    // ...
}

@device fn infer(x: tensor) -> tensor {
    // Can use: tensors, neural networks, heap
    // Cannot use: raw pointers, IRQ, port I/O
    let w = tensor_xavier(4, 8)
    tensor_matmul(x, w)
}

@safe fn bridge() {
    // Safe bridge: connects kernel and device domains
    let raw = read_sensor()
    let result = infer(tensor_from_data(raw, [1, 4]))
}
```

**Why this matters:** In Rust/C++, there's nothing preventing you from accidentally calling `malloc()` inside an interrupt handler or dereferencing a raw pointer in ML code. Fajar Lang's context system makes these mistakes compile-time errors.

### 2. Tensor as First-Class Type

```fajar
let w = tensor_xavier(784, 128)    // Xavier initialization
let x = tensor_randn([1, 784])     // Random input
let y = tensor_relu(tensor_matmul(x, w))  // Forward pass
tensor_backward(y)                  // Autograd backward pass
let grad = tensor_grad(w)          // Get gradients
```

No need to import a separate ML framework. Tensors are part of the language.

### 3. Embedded-First ML Pipeline

```
Train (laptop) → Quantize (INT8) → Export (FJMQ) → Deploy (bare-metal)
```

All steps use the same language. No Python→C++ context switching.

---

## When to Choose Fajar Lang

| Use Case | Best Choice | Why |
|----------|-------------|-----|
| Drone with on-board ML | **Fajar Lang** | Context isolation ensures safety |
| General systems programming | Rust | Mature ecosystem, proven safety |
| Game engine | C++ | Performance, mature tooling |
| ML research | Python | Ecosystem (PyTorch, JAX) |
| Embedded without ML | Zig or Rust | Simpler, no tensor overhead |
| Safety-critical embedded ML | **Fajar Lang** | Context isolation + type safety + native tensors |
| IoT with edge inference | **Fajar Lang** | Quantization + bare-metal + ML in one language |

---

## Performance Comparison (v1.0)

| Benchmark | Fajar Lang (interpreter) | Fajar Lang (native) | C (gcc -O2) |
|-----------|-------------------------|--------------------:|------------:|
| fibonacci(20) | ~26ms | < 1ms | < 1ms |
| Loop 1000 iterations | ~293us | < 10us | < 1us |
| Matrix multiply 64x64 | ~5ms (ndarray) | — | ~2ms (BLAS) |
| INT8 inference (784→128→10) | ~1ms | — | ~0.5ms |

*Note: Native codegen is JIT-only for i64. Full native pipeline planned for v1.1.*

---

*COMPARISON.md — Created 2026-03-06*
