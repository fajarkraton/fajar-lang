# Frequently Asked Questions

## General

### 1. What is Fajar Lang?

Fajar Lang is a statically-typed systems programming language designed for
embedded ML and OS integration. It lets you write OS kernels and neural
networks in the same codebase, with compile-time safety guarantees.

### 2. Who created Fajar Lang?

Fajar Lang was created by Muhamad Fajar Putranto (TaxPrime / PrimeCore.id)
in 2026. It is written in Rust and uses Cranelift for native code generation.

### 3. Is Fajar Lang open source?

Yes. Fajar Lang is open source and available on GitHub at
github.com/fajarkraton/fajar-lang.

### 4. What is the file extension for Fajar Lang?

`.fj` -- for example, `main.fj`, `utils.fj`.

### 5. What platforms does Fajar Lang support?

Linux, macOS, and Windows for development. Cross-compilation targets include
ARM64 (AArch64), RISC-V, and WebAssembly.

## Language Design

### 6. How is Fajar Lang different from Rust?

Fajar Lang simplifies Rust's ownership model (no lifetime annotations),
adds native tensor types and ML operations, and introduces context annotations
(`@kernel`, `@device`, `@safe`, `@unsafe`) for hardware isolation.

### 7. Does Fajar Lang have a garbage collector?

No. Fajar Lang uses ownership-based memory management similar to Rust.
When a value goes out of scope, it is automatically dropped.

### 8. Does Fajar Lang have null?

No. Fajar Lang uses `Option<T>` (either `Some(value)` or `None`).
This eliminates null pointer dereferences at compile time.

### 9. Does Fajar Lang have exceptions?

No. Error handling uses `Result<T, E>` and the `?` operator for propagation.
Errors are values, not control flow exceptions.

### 10. Why no semicolons?

Fajar Lang uses newlines as statement terminators, like Python and Go.
This reduces visual noise without ambiguity because expressions and
statements are syntactically distinct.

### 11. What does `@kernel` mean?

`@kernel` is a context annotation that enables OS-level operations
(raw pointers, IRQs, page tables) while forbidding heap allocation
and tensor operations. The compiler enforces these rules.

### 12. What does `@device` mean?

`@device` enables ML and accelerator operations (tensors, GPU dispatch)
while forbidding raw pointers and interrupt handling. This prevents
accidental hardware access from ML code.

### 13. Can `@kernel` and `@device` code interact?

Not directly. Use a `@safe` function as a bridge between them.
This ensures clean separation between OS and ML domains.

## Syntax

### 14. How do I declare a variable?

```fajar
let x = 42              // Immutable
let mut y = 0           // Mutable
const MAX: i32 = 1024   // Compile-time constant
```

### 15. How do I write a function?

```fajar
fn add(a: i32, b: i32) -> i32 {
    a + b
}
```

The last expression in a block is the return value (no `return` needed).

### 16. How do string interpolation (f-strings) work?

```fajar
let name = "Fajar"
println(f"Hello, {name}!")
println(f"2 + 2 = {2 + 2}")
```

### 17. What is the pipeline operator?

`|>` passes the left-hand value as the first argument to the right-hand function:

```fajar
5 |> double |> add_one   // = add_one(double(5))
```

### 18. How does pattern matching work?

```fajar
match value {
    0 => println("zero"),
    1..=9 => println("single digit"),
    n if n > 100 => println("big"),
    _ => println("other"),
}
```

Match must be exhaustive -- all cases must be covered.

## ML and Tensors

### 19. How do I create a tensor?

```fajar
let a = zeros(3, 4)
let b = randn(64, 128)
let c = from_data([[1.0, 2.0], [3.0, 4.0]])
```

### 20. How does automatic differentiation work?

Call `.backward()` on a loss value to compute gradients:

```fajar
let loss = mse_loss(predictions, targets)
loss.backward()
let gradient = x.grad()
```

### 21. Can I run ML models on microcontrollers?

Yes. Use INT8 quantization to shrink model size, then deploy with
`@device` context on ARM64 or RISC-V targets.

### 22. Does Fajar Lang support GPU?

Yes, with CUDA and Vulkan backends. Use the GPU codegen module for
PTX/SPIR-V output, or the Radxa Dragon Q6A BSP for QNN inference.

## Tooling

### 23. How do I start the REPL?

```bash
fj repl
```

### 24. How do I format my code?

```bash
fj fmt                   # Format all .fj files
fj fmt --check           # Check without modifying
```

### 25. How do I run tests?

```bash
fj test                  # Run all @test functions
```

### 26. Does Fajar Lang have an LSP?

Yes. Run `fj lsp` to start the Language Server Protocol server.
VS Code, Neovim, and Helix are supported.

## Building and Deployment

### 27. How do I cross-compile for ARM64?

```bash
fj build --target arm64
```

### 28. What is the binary size?

The `fj` compiler binary is about 7.8 MB in release mode. User programs
vary but are typically under 1 MB for simple applications.

### 29. Can I use Fajar Lang with Docker?

Yes. Use a multi-stage Dockerfile: build with Rust in the first stage,
copy the binary to a minimal image in the second stage. See the
[Deployment Guide](./guides/deployment.md).

### 30. How do I call C libraries from Fajar Lang?

Use `extern "C"` blocks to declare C functions, then call them in
`@unsafe` or `@ffi` context. See the [FFI Guide](./guides/ffi_guide.md).
