# CLI Reference

## Usage

```
fj <command> [options] [file]
```

## Commands

### `run` — Execute a program

```bash
fj run program.fj             # interpreter (tree-walking)
fj run --vm program.fj        # bytecode VM
fj run --native program.fj    # Cranelift JIT (fastest)
fj run --llvm program.fj      # LLVM JIT (best optimization)
fj run --wasm program.fj      # WebAssembly runtime
```

### `check` — Type-check without running

```bash
fj check program.fj           # report errors without execution
```

### `build` — Compile to binary

```bash
fj build                               # build from fj.toml
fj build program.fj                    # debug build
fj build --release program.fj          # optimized build
fj build --target aarch64 program.fj   # cross-compile for ARM64
fj build --target riscv64 program.fj   # cross-compile for RISC-V
fj build --target wasm32 program.fj    # compile to WebAssembly
fj build --llvm -O3 program.fj         # LLVM with max optimization
fj build --llvm --lto program.fj       # LLVM with link-time optimization
```

### `repl` — Interactive shell

```bash
fj repl                        # start REPL with analyzer
```

REPL features: multi-line input, `:type expr` to inspect types, `:help` for commands.

### `test` — Run tests

```bash
fj test program.fj             # run @test functions
fj test --filter "sort"        # filter by test name
```

### `bench` — Run benchmarks

```bash
fj bench program.fj            # micro-benchmark (10 iterations)
```

### `doc` — Generate documentation

```bash
fj doc program.fj              # generate HTML from /// comments
fj doc --open                  # generate and open in browser
```

### `watch` — Auto-run on changes

```bash
fj watch program.fj            # re-run on file save
fj watch --test program.fj     # re-run tests on save
```

### `fmt` — Format source code

```bash
fj fmt program.fj              # format in place
fj fmt --check program.fj      # check without modifying
```

### `new` — Create a project

```bash
fj new my_project              # creates directory with fj.toml
```

### `add` — Add a dependency

```bash
fj add fj-math                 # add package to fj.toml
fj add fj-nn@0.3.0             # add specific version
```

### `lsp` — Start Language Server

```bash
fj lsp                         # LSP for editor integration
```

### `debug` — Start debugger

```bash
fj debug program.fj            # DAP debug session
fj debug --record program.fj   # record for time-travel
```

### `deploy` — Deployment helpers

```bash
fj deploy --dockerfile         # generate Dockerfile
fj deploy --compose            # generate docker-compose.yml
fj deploy --k8s                # generate Kubernetes manifests
fj deploy --helm               # generate Helm chart
```

### `audit` — Security audit

```bash
fj audit                       # scan dependencies for CVEs
```

### `flash` — Flash to hardware

```bash
fj flash --board stm32f407     # flash to connected board
fj run --qemu --board stm32f407 program.fj  # run on QEMU
```

### Diagnostic Commands

```bash
fj dump-tokens program.fj      # show lexer output
fj dump-ast program.fj         # show parser AST
```

## Project Configuration

Projects use `fj.toml`:

```toml
[package]
name = "my_project"
version = "0.1.0"
edition = "2026"

[build]
target = "x86_64"
opt_level = "speed"

[dependencies]
fj-math = "0.3"
fj-nn = "0.3"
```
