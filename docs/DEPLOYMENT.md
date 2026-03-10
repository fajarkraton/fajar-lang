# DEPLOYMENT

> Build, Package & Distribution Guide — Fajar Lang

---

## 1. Build Modes

| Mode | Command | Output | Gunakan Untuk |
|------|---------|--------|---------------|
| Debug | `cargo build` | `target/debug/fajar-lang` | Development, testing |
| Release | `cargo build --release` | `target/release/fajar-lang` | Benchmarks, distribution |
| Check | `cargo check` | (no binary) | Quick validation |

### 1.1 Release Profile

```toml
# Cargo.toml
[profile.release]
opt-level = 3
lto = true          # Link-time optimization
codegen-units = 1   # Better optimization, slower compile
strip = true        # Strip debug symbols
```

---

## 2. Binary Distribution

### 2.1 Cross-Compilation

```bash
# Install cross-compilation targets
rustup target add x86_64-unknown-linux-gnu
rustup target add x86_64-apple-darwin
rustup target add x86_64-pc-windows-gnu
rustup target add aarch64-unknown-linux-gnu

# Build untuk target
cargo build --release --target x86_64-unknown-linux-gnu
cargo build --release --target aarch64-unknown-linux-gnu
```

### 2.2 Naming Convention

```
fajar-lang-{version}-{target}.tar.gz

Contoh:
  fajar-lang-0.1.0-x86_64-linux.tar.gz
  fajar-lang-0.1.0-aarch64-linux.tar.gz
  fajar-lang-0.1.0-x86_64-macos.tar.gz
  fajar-lang-0.1.0-x86_64-windows.zip
```

---

## 3. Package Structure

```
fajar-lang-0.1.0/
  bin/
    fj                    # Main binary
  lib/
    stdlib/
      core.fj             # Core standard library
      os.fj               # OS primitives
      nn.fj               # Neural network library
  examples/
    hello.fj
    fibonacci.fj
    memory_map.fj
    mnist_forward.fj
    ai_kernel_monitor.fj
  docs/
    README.md
    CHANGELOG.md
    LICENSE
```

---

## 4. Installation Script

```bash
#!/bin/bash
# install.sh
set -e

VERSION="${1:-0.1.0}"
INSTALL_DIR=${FJ_HOME:-$HOME/.fajar-lang}

mkdir -p $INSTALL_DIR/bin
mkdir -p $INSTALL_DIR/lib/stdlib

cp bin/fj $INSTALL_DIR/bin/
cp -r lib/stdlib/* $INSTALL_DIR/lib/stdlib/

echo "export PATH=\$PATH:$INSTALL_DIR/bin" >> ~/.bashrc
echo "Fajar Lang $VERSION installed to $INSTALL_DIR"
echo "Run: source ~/.bashrc && fj --version"
```

---

## 5. CI/CD Release Pipeline

```yaml
# .github/workflows/release.yml
name: Release
on:
  push:
    tags: ['v*']
jobs:
  build:
    strategy:
      matrix:
        target:
          - x86_64-unknown-linux-gnu
          - aarch64-unknown-linux-gnu
          - x86_64-apple-darwin
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo build --release --target ${{ matrix.target }}
      - uses: softprops/action-gh-release@v1
        with:
          files: target/${{ matrix.target }}/release/fajar-lang
```

---

## 6. Version Management

```bash
# Bump version
# 1. Update Cargo.toml version
# 2. Update CHANGELOG.md
# 3. Commit
git commit -am "release: v0.1.0"

# Tag
git tag -a v0.1.0 -m "Release v0.1.0: Core Language Foundation"
git push origin v0.1.0
```

---

## 7. Docker Image (Optional)

```dockerfile
# Dockerfile
FROM rust:1.75-slim as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
COPY --from=builder /app/target/release/fajar-lang /usr/local/bin/fj
COPY --from=builder /app/stdlib /usr/local/lib/fajar-lang/stdlib
ENTRYPOINT ["fj"]
```

```bash
# Build & Run
docker build -t fajar-lang:0.1.0 .
docker run fajar-lang:0.1.0 run /app/examples/hello.fj
```

---

*Deployment Version: 1.0 | Akan aktif setelah Phase 1 complete*
