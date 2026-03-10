# Fajar Lang

**The systems programming language for embedded ML + OS integration.**

Fajar Lang (`fj`) is a statically-typed systems programming language designed for the unique intersection of embedded systems and machine learning. It's the only language where an OS kernel and a neural network can share the same codebase, type system, and compiler — with safety guarantees that no existing language provides.

## Why Fajar Lang?

| Challenge | Other Languages | Fajar Lang |
|-----------|----------------|------------|
| Embedded ML inference | Python (too slow), C (unsafe) | First-class tensor types, no heap at inference |
| OS + ML in one binary | Separate languages, FFI glue | `@kernel` + `@device` contexts, compiler-enforced isolation |
| Safety on hardware | Runtime checks or nothing | Compile-time context checking, move semantics |
| Cross-compilation | Complex toolchain setup | `fj build --target aarch64-unknown-none-elf` |

## Design Principles

1. **Explicitness over magic** — no hidden allocation or hidden cost
2. **Dual-context safety** — `@kernel` disables heap+tensor; `@device` disables raw pointers
3. **Rust-inspired but simpler** — ownership lite without lifetime annotations
4. **Native tensor types** — `Tensor` is a first-class citizen in the type system

## Quick Example

```fajar
// Classify sensor data using a pre-trained neural network
fn classify(sensor: Tensor, w1: Tensor, b1: Tensor, w2: Tensor, b2: Tensor) -> i64 {
    let z1 = tensor_add(tensor_matmul(sensor, w1), b1)
    let a1 = tensor_relu(z1)
    let z2 = tensor_add(tensor_matmul(a1, w2), b2)
    let output = tensor_softmax(z2)
    tensor_argmax(output)
}

fn main() {
    let sensor = read_imu()
    let class = classify(sensor, w1, b1, w2, b2)
    if class == 2 {
        activate_alert()
    }
}
```

## Target Audience

- **Embedded AI engineers** — drone, robot, IoT
- **OS research teams** — AI-integrated kernels
- **Safety-critical ML systems** — automotive, aerospace, medical

## Current Status

Fajar Lang v1.0 includes:
- Full compiler pipeline (lexer, parser, type checker, interpreter, native codegen)
- Cranelift-based JIT and AOT compilation for x86_64, aarch64, and riscv64
- Complete ML runtime (tensors, autograd, optimizers, layers, quantization)
- OS runtime (memory management, interrupts, syscalls, DMA, timers)
- QEMU-based testing for cross-compiled binaries
- 1300+ tests, 15 examples, 12 benchmarks
