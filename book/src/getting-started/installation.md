# Installation

## From Source (Recommended)

```bash
git clone https://github.com/primecore-id/fajar-lang.git
cd fajar-lang
cargo build --release
```

The binary will be at `target/release/fj`.

### With Native Codegen (Cranelift)

```bash
cargo build --release --features native
```

This enables `fj run --native` for JIT compilation and `fj build` for AOT compilation.

## Verify Installation

```bash
fj --version
# fajar-lang 0.1.0

fj run examples/hello.fj
# Hello, Fajar Lang!
```

## System Requirements

- **Rust** 1.75+ (for building from source)
- **OS**: Linux, macOS, Windows (WSL2)
- **Optional**: QEMU for cross-compilation testing
- **Optional**: Cross-compilers for embedded targets
