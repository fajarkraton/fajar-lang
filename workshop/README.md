# Fajar Lang Workshop (3 Hours)

A hands-on workshop covering language fundamentals, ML capabilities, and embedded systems programming.

## Prerequisites

- Rust toolchain installed (for building fj from source), or pre-built fj binary
- A text editor (VS Code with Fajar Lang extension recommended)
- Basic programming experience in any language

## Part 1: Language Fundamentals (60 minutes)

### 1.1 Setup and Hello World (10 min)
- Install Fajar Lang: `cargo install --path .` or download binary
- Create `hello.fj` and run with `fj run hello.fj`
- Explore the REPL: `fj repl`

### 1.2 Types and Variables (15 min)
- Primitive types: `i32`, `f64`, `bool`, `str`, `char`
- `let` vs `let mut`, type inference
- Exercise: Temperature converter (Celsius to Fahrenheit)

### 1.3 Functions and Control Flow (15 min)
- Function definitions with return types
- `if`/`else` as expressions, `match`, `while`, `for..in`
- Pipeline operator `|>`
- Exercise: FizzBuzz with match and pipeline

### 1.4 Structs, Enums, and Error Handling (20 min)
- Defining structs with methods (`impl`)
- Enums with data: `Option<T>`, `Result<T, E>`
- The `?` operator for error propagation
- Exercise: Build a simple calculator with error handling

### Slide References
- slides/01-fundamentals.pdf (provided separately)
- examples/hello.fj, examples/fibonacci.fj

---

## Part 2: ML and Tensor Operations (60 minutes)

### 2.1 First-Class Tensors (15 min)
- Creating tensors: `zeros`, `ones`, `randn`, `from_data`
- Basic ops: add, mul, matmul, reshape, transpose
- Exercise: Matrix multiplication by hand vs tensor ops

### 2.2 Autograd and Training (20 min)
- `requires_grad`, `backward()`, `grad()`
- Building a simple linear regression
- Loss functions: `mse_loss`, `cross_entropy`
- Exercise: Train a linear model on synthetic data

### 2.3 Neural Network Layers (15 min)
- Dense, Conv2d, BatchNorm, Dropout
- Building a small MLP for MNIST
- Exercise: Modify the MLP architecture and observe accuracy

### 2.4 Context Annotations for Safety (10 min)
- `@device` context: tensor ops allowed, no raw pointers
- `@safe` context: no direct tensor or hardware access
- Bridge pattern between contexts
- Exercise: Write a safe inference pipeline

### Slide References
- slides/02-ml.pdf (provided separately)
- examples/mnist.fj, examples/linear_regression.fj

---

## Part 3: Embedded and OS Programming (60 minutes)

### 3.1 The @kernel Context (15 min)
- What `@kernel` enables: raw pointers, IRQ, syscalls, allocators
- What `@kernel` forbids: heap allocation, tensor ops
- Exercise: Write a simple memory-mapped I/O reader

### 3.2 Cross-Domain Bridge (15 min)
- Sensor data (`@kernel`) to inference (`@device`) to action (`@safe`)
- The bridge pattern in practice
- Exercise: Build a sensor-to-decision pipeline

### 3.3 Cross-Compilation (15 min)
- Targeting ARM64 and RISC-V: `fj build --target aarch64`
- no_std and bare-metal considerations
- Exercise: Compile a blink program for ARM64

### 3.4 FajarOS Demo (15 min)
- Tour of FajarOS Nova (x86_64 bare-metal kernel in Fajar Lang)
- Boot sequence, shell commands, syscalls
- Live demo: QEMU boot and shell interaction
- Discussion: Where Fajar Lang fits in the embedded AI landscape

### Slide References
- slides/03-embedded.pdf (provided separately)
- examples/blink.fj, examples/drone_pipeline.fj

---

## Exercises Summary

| # | Exercise | Part | Difficulty |
|---|----------|------|------------|
| 1 | Temperature converter | 1 | Beginner |
| 2 | FizzBuzz with pipeline | 1 | Beginner |
| 3 | Calculator with Result | 1 | Intermediate |
| 4 | Matrix multiplication | 2 | Beginner |
| 5 | Linear regression | 2 | Intermediate |
| 6 | MNIST MLP tuning | 2 | Intermediate |
| 7 | Safe inference pipeline | 2 | Intermediate |
| 8 | Memory-mapped I/O | 3 | Advanced |
| 9 | Sensor-to-decision bridge | 3 | Advanced |
| 10 | ARM64 cross-compile | 3 | Advanced |

## Materials Checklist

- [ ] Pre-built `fj` binaries for Linux/macOS/Windows
- [ ] Slide decks (01-fundamentals, 02-ml, 03-embedded)
- [ ] Exercise starter files in `workshop/exercises/`
- [ ] Solution files in `workshop/solutions/`
- [ ] VS Code extension .vsix for offline install
