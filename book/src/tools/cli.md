# CLI Reference

## Usage

```
fj <command> [options] [file]
```

## Commands

### `run` — Execute a program

```bash
fj run program.fj           # interpreter (tree-walking)
fj run --vm program.fj      # bytecode VM
fj run --native program.fj  # native JIT compilation (fastest)
```

### `check` — Type-check without running

```bash
fj check program.fj         # report errors without execution
```

### `build` — Compile to binary

```bash
fj build program.fj                   # debug build
fj build --release program.fj         # optimized build
fj build --target aarch64 program.fj  # cross-compile for ARM64
fj build --target riscv64 program.fj  # cross-compile for RISC-V
```

### `repl` — Interactive shell

```bash
fj repl                     # start REPL with analyzer
```

### `new` — Create a project

```bash
fj new my_project           # creates directory with fj.toml
```

### `fmt` — Format source code

```bash
fj fmt program.fj           # format in place
```

### `lsp` — Start Language Server

```bash
fj lsp                      # LSP for editor integration
```

### Diagnostic Commands

```bash
fj dump-tokens program.fj   # show lexer output
fj dump-ast program.fj      # show parser AST
```

## Project Configuration

Projects use `fj.toml`:

```toml
[package]
name = "my_project"
version = "0.1.0"

[build]
target = "x86_64"
opt_level = "speed"
```
