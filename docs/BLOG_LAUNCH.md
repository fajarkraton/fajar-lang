# Introducing Fajar Lang: The Only Language Where OS Kernels and Neural Networks Share One Compiler

**TL;DR:** Fajar Lang is a statically-typed systems language for embedded AI where `@kernel` code can't accidentally call tensor ops and `@device` code can't accidentally access hardware registers — enforced at compile time. One language, one binary, one type system for flight controllers, ML inference, and mission planning.

---

## The Problem

Building embedded AI systems today requires stitching together multiple languages:

- **C/C++** for the OS kernel and hardware drivers
- **Python** for ML training (then export to ONNX/TFLite)
- **Rust** or **C** for the inference runtime
- **Custom glue code** to bridge between domains

Each boundary is a potential bug. A flight controller that accidentally calls `malloc` during an interrupt handler. A neural network inference path that accidentally writes to a hardware register. These bugs are subtle, dangerous, and caught only at runtime — if you're lucky.

## The Solution: Compiler-Enforced Domain Safety

Fajar Lang introduces **context annotations** that partition your code into safety domains:

```fajar
@kernel fn read_imu() -> ImuReading with Hardware {
    // Only hardware access allowed here
    // Compiler REJECTS: tensor ops, heap allocation
    volatile_read(IMU_ADDR)
}

@device fn classify(image: Tensor) -> Detection with Tensor {
    // Only tensor/compute ops allowed here
    // Compiler REJECTS: hardware access, raw pointers
    softmax(matmul(image, weights))
}

@safe fn plan_mission(det: Detection) -> Action {
    // Purest domain: no hardware, no tensors
    // Only safe application logic
    if det.confidence > 0.8 { Action::Avoid } else { Action::Continue }
}
```

The compiler enforces these boundaries. Violations produce clear errors:

```
EE006: effect 'Tensor' is forbidden in @kernel context
EE006: effect 'Hardware' is forbidden in @device context
```

**No runtime checks. No overhead. Compile-time guarantees.**

## What Makes Fajar Lang Unique

### 1. Effect System (Feature Rust Doesn't Have)

Functions declare their side effects explicitly:

```fajar
fn read_sensor() -> i64 with Hardware, IO { ... }
fn inference(x: Tensor) -> Tensor with Tensor { ... }
fn pure_math(a: i64, b: i64) -> i64 { ... }  // no effects = pure
```

The compiler tracks effect propagation and rejects violations.

### 2. Compile-Time Evaluation (Zig's Killer Feature)

```fajar
comptime fn factorial(n: i64) -> i64 {
    if n <= 1 { 1 } else { n * factorial(n - 1) }
}
const LOOKUP: i64 = comptime { factorial(10) }  // = 3,628,800 at compile time
```

### 3. Linear Types (Beyond Rust's Affine Types)

```fajar
linear struct FileHandle { fd: i64 }
// Must be consumed exactly once — can't leak, can't use twice
```

### 4. First-Class Tensor Operations

```fajar
let x = zeros(3, 4)
let y = matmul(x, weights)
let a = relu(y)
backward(loss)  // autograd
```

### 5. One Binary, Three Domains

The killer demo: a 639-line drone controller where flight control (`@kernel`), ML inference (`@device`), and mission planning (`@safe`) coexist in a single `.fj` file with compiler-enforced safety.

## By the Numbers

| Metric | Value |
|--------|-------|
| Tests | 5,547 (0 failures) |
| LOC (compiler) | ~290,000 Rust |
| Self-hosted compiler | 1,268 LOC in Fajar Lang |
| Examples | 130+ `.fj` programs |
| Backends | Cranelift (dev) + LLVM (release) + Wasm |
| Targets | x86_64, ARM64, RISC-V, Wasm |
| Error codes | 80+ across 10 categories |
| IDE support | VS Code (LSP + DAP debugger) |

## Architecture

```
Source (.fj)
    │
    ├─ Lexer (tokenize)
    ├─ Parser (recursive descent + Pratt)
    ├─ Analyzer (types, effects, ownership, NLL borrows)
    │
    ├─ Cranelift Backend (fast compile, dev mode)
    ├─ LLVM Backend (optimized, --release)
    └─ Wasm Backend (browser, playground)
```

## Getting Started

```bash
# Build and run
fj run hello.fj

# Build optimized release
fj build program.fj --release

# Start REPL
fj repl

# Generate docs
fj doc program.fj

# Generate playground
fj playground
```

## What's Next

- Phase 8: Community building (Discord, newsletter, meetups)
- Formal verification integration (SMT/Z3)
- GPU compute backend (CUDA/Vulkan)
- Production deployment on Radxa Dragon Q6A

---

*Fajar Lang is open source. The compiler, standard library, and all examples are available on GitHub.*

*"The only language where an OS kernel and a neural network can share the same codebase, type system, and compiler, with safety guarantees that no existing language provides."*
