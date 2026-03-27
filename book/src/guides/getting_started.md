# Getting Started with Fajar Lang

This guide walks you through installing Fajar Lang, writing your first program,
using the REPL, and setting up a project with `fj.toml`.

## Installation

### From Source (Recommended)

```bash
git clone https://github.com/fajarkraton/fajar-lang.git
cd fajar-lang
cargo build --release
sudo cp target/release/fj /usr/local/bin/
```

### Verify Installation

```bash
fj --version
# fj 6.1.0 "Illumination"
```

### System Requirements

- Rust 1.87+ (for building from source)
- Linux, macOS, or Windows
- 512 MB RAM minimum (4 GB recommended for ML workloads)

## Your First Program

Create a file called `hello.fj`:

```fajar
fn main() {
    println("Hello, Fajar Lang!")
}
```

Run it:

```bash
fj run hello.fj
# Hello, Fajar Lang!
```

## Using the REPL

Start an interactive session:

```bash
fj repl
```

```
fj> let x = 42
fj> let y = x * 2
fj> println(f"Result: {y}")
Result: 84
fj> fn square(n: i32) -> i32 { n * n }
fj> square(7)
49
```

The REPL remembers definitions across lines, supports multi-line input,
and runs the full analysis pipeline (type checking included).

Press `Ctrl+D` to exit.

## Creating a Project

Use `fj new` to scaffold a project:

```bash
fj new my_project
cd my_project
```

This creates:

```
my_project/
  fj.toml        # Project manifest
  src/
    main.fj       # Entry point
  tests/
    test_main.fj  # Test file
```

### fj.toml

```toml
[package]
name = "my_project"
version = "0.1.0"
edition = "2026"

[dependencies]
fj-math = "0.1"

[build]
target = "native"
optimize = "release"
```

### Build and Run

```bash
fj build          # Compile the project
fj run            # Build and run
fj test           # Run all tests
fj check          # Type-check without compiling
```

## Type Checking Only

To check your code without running it:

```bash
fj check hello.fj
```

This runs the lexer, parser, and semantic analyzer but skips execution.
Useful for catching errors quickly in large codebases.

## Inspecting Internals

For debugging or learning:

```bash
fj dump-tokens hello.fj   # Show lexer output
fj dump-ast hello.fj      # Show parser AST
```

## Next Steps

- [Build System Guide](./build_system.md) -- project configuration in depth
- [Error Handling Guide](./error_handling.md) -- Result, Option, and the ? operator
- [ML Guide](./ml_guide.md) -- training your first neural network
- [Language Reference](../reference/variables.md) -- full language details
