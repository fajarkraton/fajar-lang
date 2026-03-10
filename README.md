# Fajar Lang (`fj`)

A statically-typed systems programming language for embedded ML and OS integration.

**The only language where an OS kernel and a neural network can share the same codebase, type system, and compiler.**

[![CI](https://github.com/primecore-id/fajar-lang/actions/workflows/ci.yml/badge.svg)](https://github.com/primecore-id/fajar-lang/actions/workflows/ci.yml)

## Features

- **Dual-context safety** -- `@kernel` disables heap/tensor, `@device` disables raw pointers. Compiler enforces domain isolation.
- **Native tensor types** -- `Tensor` is a first-class citizen in the type system with shape checking.
- **Rust-inspired syntax** -- Familiar to Rust/C developers, but simpler (no lifetime annotations).
- **Pipeline operator** -- `x |> f |> g` for functional-style data flow.
- **Pattern matching** -- Exhaustive `match` on enums, structs, tuples.
- **Context annotations** -- `@safe`, `@kernel`, `@device`, `@unsafe` for compile-time safety guarantees.
- **Built-in ML runtime** -- Autograd, tensor ops, optimizers (SGD, Adam), loss functions.
- **OS primitives** -- Memory management, IRQ handling, syscall dispatch for kernel development.
- **Beautiful errors** -- Rust-style diagnostics with source highlighting via miette.

## Quickstart

### Build from source

```bash
git clone https://github.com/primecore-id/fajar-lang.git
cd fajar-lang
cargo build --release
```

The binary is at `target/release/fj`.

### Install via Cargo

```bash
cargo install fajar-lang
```

### Run a program

```bash
fj run examples/hello.fj
```

### Start the REPL

```bash
fj repl
```

## Example Programs

### Hello World

```fajar
// hello.fj
use std::io::println

fn main() -> void {
    println("Hello from Fajar Lang!")
}
```

### Fibonacci

```fajar
// fibonacci.fj
fn fibonacci(n: i64) -> i64 {
    if n <= 1 {
        n
    } else {
        fibonacci(n - 1) + fibonacci(n - 2)
    }
}

fn main() -> void {
    for i in 0..10 {
        println(fibonacci(i))
    }
}
```

### Context Annotations

```fajar
@kernel fn collect_sensor() -> [f32; 4] {
    // OS domain: raw memory access allowed, no heap/tensor
    let data = port_read!(0x3F00, 4)
    data
}

@device fn infer(x: Tensor) -> Tensor {
    // ML domain: tensor ops allowed, no raw pointers
    let w = zeros(4, 2)
    x @ w |> relu
}

@safe fn bridge() -> i64 {
    // Safe bridge: can call both @kernel and @device
    let raw = collect_sensor()
    let input = Tensor::from_slice(raw)
    let result = infer(input)
    result.argmax()
}
```

## CLI Commands

| Command | Description |
|---------|-------------|
| `fj run <file.fj>` | Execute a Fajar Lang program |
| `fj run` | Run project entry point (from `fj.toml`) |
| `fj repl` | Start interactive REPL |
| `fj check <file.fj>` | Parse and type-check (no execution) |
| `fj build` | Build current project |
| `fj new <name>` | Create a new project |
| `fj fmt <file.fj>` | Format source code |
| `fj dump-tokens <file.fj>` | Show lexer output |
| `fj dump-ast <file.fj>` | Show parser AST output |
| `fj lsp` | Start Language Server Protocol server |

## Feature Matrix

| Feature | Status |
|---------|--------|
| Lexer (tokenization) | Working |
| Parser (Pratt + recursive descent) | Working |
| Semantic analyzer (types, scope, context) | Working |
| Tree-walking interpreter | Working |
| Bytecode VM (45 opcodes) | Working |
| REPL | Working |
| Pattern matching (`match`) | Working |
| Closures & first-class functions | Working |
| Pipeline operator (`\|>`) | Working |
| Context annotations (@safe/@kernel/@device) | Working |
| OS runtime (memory, IRQ, syscall) | Working |
| ML runtime (tensor, autograd, ops) | Working |
| Formatter (`fj fmt`) | Working |
| LSP server | Working |
| Package manager (`fj.toml`) | Working |
| Native compiler (Cranelift) | Planned (v1.0) |
| Generics & traits | Planned (v1.0) |
| Borrow checker | Planned (v1.0) |
| FFI (C interop) | Planned (v1.0) |
| Cross-compilation (ARM, RISC-V) | Planned (v1.0) |

## Project Structure

```
src/
  lib.rs            -- Module declarations + FjError
  main.rs           -- CLI entry point (clap)
  lexer/            -- Tokenization
  parser/           -- AST generation (Pratt + recursive descent)
  analyzer/         -- Semantic analysis (types, scope, context)
  interpreter/      -- Tree-walking evaluator
  vm/               -- Bytecode compiler + virtual machine
  runtime/
    os/             -- Memory, IRQ, syscall primitives
    ml/             -- Tensor, autograd, ops, optimizers
  formatter/        -- Code formatter
  lsp/              -- Language Server Protocol
  package/          -- Project manifest (fj.toml)
examples/           -- Example .fj programs
tests/              -- Integration tests
docs/               -- Documentation
```

## License

MIT License. See [LICENSE](LICENSE) for details.

## Contributing

See [CONTRIBUTING.md](docs/CONTRIBUTING.md) for guidelines.

Commit format: `<type>(<scope>): <description>` (e.g., `feat(lexer): add string interpolation`)
