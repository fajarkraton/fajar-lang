# Cross-Compilation Guide

> Build Fajar Lang programs for ARM64 targets from an x86_64 host.

---

## Quick Start

```bash
# 1. Install cross-compiler toolchain
sudo apt install gcc-aarch64-linux-gnu g++-aarch64-linux-gnu

# 2. Add Rust target
rustup target add aarch64-unknown-linux-gnu

# 3. Build fj binary for ARM64
./scripts/cross-build-q6a.sh

# 4. Deploy to Dragon Q6A
./scripts/deploy-q6a.sh <ip-address>
```

---

## Supported Targets

| Target | Triple | Board | Use Case |
|--------|--------|-------|----------|
| ARM64 Linux | `aarch64-unknown-linux-gnu` | Dragon Q6A, Jetson Thor | Edge AI SBC |
| ARM64 Bare-Metal | `aarch64-unknown-none` | Custom boards | OS kernel dev |
| RISC-V 64 Linux | `riscv64gc-unknown-linux-gnu` | SiFive, StarFive | RISC-V SBC |
| RISC-V 64 Bare-Metal | `riscv64gc-unknown-none-elf` | Custom boards | RISC-V firmware |
| x86_64 Linux | `x86_64-unknown-linux-gnu` | Host (default) | Desktop/server |

---

## Cross-Compile the `fj` Compiler

### Prerequisites

```bash
# Ubuntu/Debian
sudo apt install gcc-aarch64-linux-gnu g++-aarch64-linux-gnu
rustup target add aarch64-unknown-linux-gnu
```

### Build

```bash
cargo build --release --target aarch64-unknown-linux-gnu
```

The cross-linker is configured in `.cargo/config.toml`:

```toml
[target.aarch64-unknown-linux-gnu]
linker = "aarch64-linux-gnu-gcc"
```

### Verify

```bash
file target/aarch64-unknown-linux-gnu/release/fj
# ELF 64-bit LSB pie executable, ARM aarch64

ls -lh target/aarch64-unknown-linux-gnu/release/fj
# ~6.7 MB (5.5 MB stripped)
```

---

## Cross-Compile Fajar Lang Programs

### Using `--target` flag

```bash
# Compile .fj program to ARM64 native binary
fj build program.fj --target aarch64-unknown-linux-gnu -o program
```

### Using `--board` flag

```bash
# Compile for Dragon Q6A (auto-selects aarch64-unknown-linux-gnu)
fj build program.fj --board dragon-q6a -o program
```

### Bare-Metal Cross-Compilation

```bash
# Compile for bare-metal ARM64 (no OS)
fj build kernel.fj --target aarch64-unknown-none --no-std --linker-script kernel.ld
```

---

## Deploy to Dragon Q6A

### Manual

```bash
scp target/aarch64-unknown-linux-gnu/release/fj radxa@<ip>:~/bin/fj
ssh radxa@<ip> "chmod +x ~/bin/fj && ~/bin/fj run program.fj"
```

### Script

```bash
# Deploy binary only
./scripts/deploy-q6a.sh <ip>

# Deploy with examples
./scripts/deploy-q6a.sh <ip> --examples

# Deploy and run
./scripts/deploy-q6a.sh <ip> --run program.fj
```

---

## Binary Sizes

| Configuration | Size |
|--------------|------|
| Release (unstripped) | ~6.7 MB |
| Release (stripped) | ~5.5 MB |
| Debug | ~90 MB |

---

## Troubleshooting

| Problem | Solution |
|---------|----------|
| `linker 'aarch64-linux-gnu-gcc' not found` | `sudo apt install gcc-aarch64-linux-gnu` |
| `can't find crate for 'std'` | `rustup target add aarch64-unknown-linux-gnu` |
| Binary won't run on Q6A | Check `file` output — must be `ARM aarch64` |
| Permission denied on Q6A | `chmod +x ~/bin/fj` |
| Shared library not found | Install `libm`, `libc` on target (Ubuntu has them) |

---

*Cross-Compile Guide v1.0 | Target: Radxa Dragon Q6A (QCS6490)*
