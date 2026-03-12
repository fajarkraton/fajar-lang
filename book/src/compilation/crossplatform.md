# Cross-Platform Support

Fajar Lang runs on Linux, macOS, and Windows with consistent behavior across platforms.

## Platform Detection

```fajar
use runtime::platform

let platform = Platform::detect()
println(f"OS: {platform.os}")           // linux, macos, windows
println(f"Arch: {platform.arch}")       // x86_64, aarch64, riscv64
println(f"Endian: {platform.endian}")   // little, big
```

## Cross-Compilation

```bash
# x86_64 Linux (default)
fj build program.fj

# ARM64 Linux
fj build --target aarch64-unknown-linux-gnu program.fj

# RISC-V 64 Linux
fj build --target riscv64gc-unknown-linux-gnu program.fj

# ARM64 macOS
fj build --target aarch64-apple-darwin program.fj

# Windows (cross from Linux)
fj build --target x86_64-pc-windows-gnu program.fj

# Bare metal ARM (embedded)
fj build --target thumbv7em-none-eabihf program.fj

# WebAssembly
fj build --target wasm32-wasi program.fj
```

## QEMU Testing

Test cross-compiled binaries without physical hardware:

```bash
fj build --target aarch64-unknown-linux-gnu program.fj
fj run --qemu aarch64 ./program
```

Supported QEMU targets: `aarch64`, `riscv64`, `x86_64`.

## Path Handling

Fajar Lang normalizes paths across platforms:

```fajar
// Windows: C:\Users\fajar\project\src\main.fj
// Unix:    /home/fajar/project/src/main.fj
// URI:     file:///home/fajar/project/src/main.fj

let uri = path_to_uri("src/main.fj")
let path = uri_to_path(uri)
```

## Line Endings

Automatically detects and normalizes line endings:
- **LF** (`\n`) — Unix/macOS (default output)
- **CRLF** (`\r\n`) — Windows
- **CR** (`\r`) — Legacy macOS

## Binary Distribution

```bash
# Generate platform-specific installers
fj dist --shell          # install.sh (Linux/macOS)
fj dist --powershell     # install.ps1 (Windows)
fj dist --homebrew       # Homebrew formula
fj dist --deb            # Debian package

# Generate shell completions
fj completions bash > /etc/bash_completion.d/fj
fj completions zsh > ~/.zfunc/_fj
fj completions fish > ~/.config/fish/completions/fj.fish
```

## Support Tiers

| Tier | Targets | Guarantee |
|------|---------|-----------|
| Tier 1 | x86_64-linux, aarch64-linux, x86_64-macos | Full CI, all tests |
| Tier 2 | aarch64-macos, x86_64-windows, riscv64-linux | CI, most tests |
| Tier 3 | thumbv7em-none, wasm32-wasi, x86_64-windows-gnu | Compiles, basic tests |
