# Deployment Guide

This guide covers deploying Fajar Lang applications in production
using Docker, cross-compilation, and CI/CD pipelines.

## Building for Production

```bash
fj build --release
```

The output binary is statically linked and self-contained:

```bash
ls -lh target/release/my_app
# -rwxr-xr-x 1 user user 7.8M my_app
```

## Docker Deployment

### Multi-Stage Dockerfile

```dockerfile
# Build stage
FROM rust:1.87-slim AS builder
RUN cargo install fj
COPY . /app
WORKDIR /app
RUN fj build --release

# Runtime stage
FROM debian:bookworm-slim
COPY --from=builder /app/target/release/my_app /usr/local/bin/
EXPOSE 8080
CMD ["my_app"]
```

### Build and Run

```bash
docker build -t my-fj-app .
docker run -p 8080:8080 my-fj-app
```

### Minimal Image (Scratch)

For the smallest possible container:

```dockerfile
FROM scratch
COPY --from=builder /app/target/release/my_app /my_app
ENTRYPOINT ["/my_app"]
```

## Cross-Compilation for Deployment

### ARM64 Server (AWS Graviton, etc.)

```bash
fj build --release --target arm64
scp target/release/my_app user@arm-server:/opt/
```

### RISC-V Target

```bash
fj build --release --target riscv64
```

## CI/CD with GitHub Actions

### Basic Workflow

```yaml
# .github/workflows/ci.yml
name: CI

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo build --release
      - run: cargo test
      - run: cargo clippy -- -D warnings

  release:
    needs: test
    if: startsWith(github.ref, 'refs/tags/')
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo build --release
      - uses: softprops/action-gh-release@v1
        with:
          files: target/release/fj
```

### Cross-Platform Build Matrix

```yaml
jobs:
  build:
    strategy:
      matrix:
        os: [ubuntu-latest, macos-latest, windows-latest]
        rust: [stable, nightly]
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@master
        with:
          toolchain: ${{ matrix.rust }}
      - run: cargo test
```

## Systemd Service

Deploy as a Linux service:

```ini
# /etc/systemd/system/my-fj-app.service
[Unit]
Description=My Fajar Lang Application
After=network.target

[Service]
Type=simple
User=www-data
ExecStart=/usr/local/bin/my_app
Restart=on-failure
RestartSec=5
Environment=FJ_LOG=info

[Install]
WantedBy=multi-user.target
```

```bash
sudo systemctl enable my-fj-app
sudo systemctl start my-fj-app
```

## Health Checks

Add a health endpoint to your application:

```fajar
fn health_check() -> Response {
    Response::json(200, "{\"status\": \"ok\"}")
}
```

For Docker:

```dockerfile
HEALTHCHECK --interval=30s CMD curl -f http://localhost:8080/health || exit 1
```

## Environment Configuration

```fajar
fn main() {
    let port = get_env("PORT").unwrap_or("8080")
    let db_url = get_env("DATABASE_URL")?
    let log_level = get_env("FJ_LOG").unwrap_or("info")
}
```
