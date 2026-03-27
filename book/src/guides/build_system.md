# Build System Guide

Fajar Lang uses `fj.toml` as the project manifest and `fj` as the build tool.
This guide covers the full build system.

## fj.toml Format

```toml
[package]
name = "my_app"
version = "1.0.0"
edition = "2026"
authors = ["Your Name <you@example.com>"]
description = "A Fajar Lang application"
license = "MIT"

[dependencies]
fj-math = "0.1"
fj-json = "0.2"
fj-http = "0.1"

[dev-dependencies]
fj-bench = "0.1"

[build]
target = "native"          # native | arm64 | riscv64 | wasm
optimize = "release"       # debug | release
features = ["ml", "os"]

[build.cross]
linker = "aarch64-linux-gnu-gcc"
sysroot = "/usr/aarch64-linux-gnu"
```

## Build Commands

### fj build

Compiles the project according to `fj.toml`:

```bash
fj build                   # Default build
fj build --release         # Optimized build
fj build --target arm64    # Cross-compile
```

Output goes to `target/debug/` or `target/release/`.

### fj run

Build and execute in one step:

```bash
fj run                     # Run from fj.toml
fj run src/main.fj         # Run a specific file
fj run --vm src/main.fj    # Run with bytecode VM instead of tree-walker
fj run -- arg1 arg2        # Pass arguments to the program
```

### fj test

Run all test files matching `tests/**/*.fj` and functions with `@test`:

```bash
fj test                    # Run all tests
fj test tests/math.fj      # Run specific test file
fj test --filter "parse"   # Run tests matching a pattern
```

### fj check

Type-check without compiling or executing:

```bash
fj check                   # Check entire project
fj check src/lib.fj        # Check a specific file
```

This runs the lexer, parser, and semantic analyzer. It catches type errors,
unused variables, unreachable code, and context violations.

### fj fmt

Format Fajar Lang source files:

```bash
fj fmt                     # Format all .fj files in project
fj fmt src/main.fj         # Format a specific file
fj fmt --check             # Check formatting without modifying
```

The formatter uses AST-based pretty-printing for consistent style.

## Build Targets

| Target | Description | Use Case |
|--------|-------------|----------|
| `native` | Host architecture | Development, servers |
| `arm64` | AArch64 cross-compile | Embedded Linux, Raspberry Pi |
| `riscv64` | RISC-V cross-compile | RISC-V boards |
| `wasm` | WebAssembly | Browser, serverless |
| `no_std` | Bare-metal (no OS) | OS kernels, firmware |

## Project Layout Conventions

```
my_project/
  fj.toml              # Project manifest (required)
  src/
    main.fj             # Binary entry point
    lib.fj              # Library root
    utils/
      math.fj           # Module: utils::math
  tests/
    test_core.fj        # Integration tests
  examples/
    demo.fj             # Example programs
  benches/
    bench_sort.fj       # Benchmarks
```

## Environment Variables

| Variable | Purpose | Default |
|----------|---------|---------|
| `FJ_HOME` | Fajar Lang install directory | `~/.fj` |
| `FJ_TARGET` | Override build target | `native` |
| `FJ_LOG` | Compiler log level | `warn` |
