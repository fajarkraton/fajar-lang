# Installation

## From Source (Recommended)

```bash
git clone https://github.com/fajarkraton/fajar-lang.git
cd fajar-lang
cargo build --release
```

The binary will be at `target/release/fj`.

### Feature Flags

```bash
# Default (interpreter + bytecode VM + Cranelift native)
cargo build --release

# With LLVM backend (requires llvm-18-dev)
sudo apt-get install llvm-18-dev libpolly-18-dev libzstd-dev
cargo build --release --features llvm

# With GPU support
cargo build --release --features gpu

# All features
cargo build --release --features "native llvm gpu"
```

## Verify Installation

```bash
fj --version
# fajar-lang 3.0.0

fj run examples/hello.fj
# Hello, Fajar Lang!
```

## System Requirements

- **Rust** 1.80+ (for building from source)
- **OS**: Linux, macOS, Windows
- **Optional**: LLVM 18 (for `--features llvm`)
- **Optional**: QEMU (for cross-compilation testing: `qemu-system-aarch64`, `qemu-system-riscv64`)
- **Optional**: Cross-compilers for embedded targets

## Compilation Backends

| Backend | Flag | Speed | Optimization | Use Case |
|---------|------|-------|-------------|----------|
| Interpreter | (default) | Instant | None | Learning, REPL |
| Bytecode VM | `--vm` | Fast | Basic | Quick scripts |
| Cranelift | `--native` | ~1ms/fn | Good | Development |
| LLVM | `--llvm` | ~10ms/fn | Excellent | Production |
| WebAssembly | `--wasm` | Medium | Good | Browser/edge |

## Editor Setup

### VS Code

Install the "Fajar Lang" extension from the `editors/vscode/` directory:

```bash
cd editors/vscode
npm install && npm run build
# Then install the .vsix file in VS Code
```

The extension provides: syntax highlighting, snippets, LSP integration, debug configuration.
