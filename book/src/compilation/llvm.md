# LLVM Backend

Fajar Lang includes an LLVM backend alongside Cranelift, providing access to LLVM's mature optimization infrastructure.

## Usage

```bash
# Install LLVM (Ubuntu/Debian)
sudo apt-get install llvm-18-dev libpolly-18-dev libzstd-dev

# Build with LLVM support
cargo build --release --features llvm

# Run with LLVM JIT
fj run --llvm examples/fibonacci.fj

# Run with LLVM AOT (compile to native binary)
fj build --llvm --output my_program
```

## Optimization Levels

| Flag | Level | Description |
|------|-------|-------------|
| `-O0` | None | No optimization, fastest compile |
| `-O1` | Less | Basic optimizations |
| `-O2` | Default | Standard optimizations |
| `-O3` | Aggressive | Maximum optimization |

```bash
fj build --llvm -O3 examples/benchmark.fj
```

## LTO (Link-Time Optimization)

```bash
fj build --llvm --lto examples/large_project.fj
```

LTO performs whole-program optimization across module boundaries — function inlining, dead code elimination, and constant propagation across files.

## Backend Comparison

| Feature | Cranelift | LLVM |
|---------|-----------|------|
| Compile speed | Fast (~1ms/fn) | Slower (~10ms/fn) |
| Code quality | Good | Excellent |
| Optimization levels | O0, O2 | O0-O3 |
| LTO | No | Yes |
| Target coverage | x86_64, aarch64, riscv64 | All major targets |
| Embedded targets | Limited | Full (thumbv7em, xtensa) |
| Debug info | Basic | Full DWARF |

**Recommendation**: Use Cranelift for development (fast iteration), LLVM for production builds (maximum performance).

## When to Use LLVM

- **Production releases** — maximum optimization
- **Embedded MCU targets** — thumbv7em, thumbv6m, Xtensa (Cranelift cannot target these)
- **LTO across modules** — whole-program optimization
- **Profile-guided optimization (PGO)** — LLVM supports PGO profiles
