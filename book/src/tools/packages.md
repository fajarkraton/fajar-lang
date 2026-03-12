# Package Manager

## Project Configuration

Every Fajar project has a `fj.toml`:

```toml
[package]
name = "my_project"
version = "0.1.0"
edition = "2025"

[build]
target = "x86_64"
opt_level = "speed"

[dependencies]
fj-math = "0.3"
fj-nn = "0.3"
fj-json = "0.3"
```

## Creating a Project

```bash
fj new my_project
```

Creates:

```
my_project/
  fj.toml
  src/
    main.fj
```

## Building

```bash
cd my_project
fj build              # debug build
fj build --release    # optimized build
fj run                # build and run
```

## Dependencies

### Adding Packages

```bash
fj add fj-math              # latest version
fj add fj-nn@0.3.0          # specific version
fj add fj-hal --features stm32  # with feature flags
```

### Lock File

`fj.lock` records exact resolved versions for reproducible builds. Commit this file.

### Version Resolution

Dependencies are resolved using the PubGrub algorithm — conflict-driven clause learning that finds a satisfying version assignment or reports an exact conflict.

## Standard Packages

| Package | Domain | Description |
|---------|--------|-------------|
| `fj-math` | General | Extended math functions |
| `fj-nn` | ML | Neural network layers and training |
| `fj-hal` | Embedded | Hardware abstraction layer |
| `fj-drivers` | Embedded | I2C, SPI, UART, CAN drivers |
| `fj-http` | Network | HTTP client/server |
| `fj-json` | Data | JSON parsing/generation |
| `fj-crypto` | Security | Cryptographic primitives |

## Standard Library

Built-in modules (no dependency needed):

| Module | Contents |
|--------|----------|
| `std::io` | print, println, eprintln, read_file, write_file, append_file, file_exists |
| `std::math` | PI, E, sqrt, sin, cos, tan, abs, pow, floor, ceil, round, clamp, min, max |
| `std::string` | trim, split, replace, contains, starts_with, ends_with, parse_int, parse_float, to_uppercase, to_lowercase, substring, char_at, repeat |
| `std::collections` | Array (15+ methods), HashMap (8 builtins + 7 methods) |
| `std::convert` | to_string, to_int, to_float, `as` cast |
| `nn::tensor` | zeros, ones, randn, eye, xavier, from_data, arange, linspace |
| `nn::ops` | matmul, relu, sigmoid, softmax, tanh, gelu, cross_entropy, mse_loss |
| `os::memory` | mem_alloc, mem_free, mem_read, mem_write, page_map, page_unmap |
| `os::irq` | irq_register, irq_enable, irq_disable |

## Registry

### Publishing

```bash
fj publish              # publish current package
fj publish --dry-run    # validate without publishing
```

### Search

```bash
fj search "neural network"    # search registry
```

### Yanking

```bash
fj yank my-package@1.0.0     # prevent new installs of a version
```

Yanked versions are not selected for new dependency resolution but existing lock files continue to work.

### Authentication

```bash
fj login --token <api-token>  # authenticate with registry
```

## Workspaces

Manage multiple related packages in a single repository:

```toml
# fj.toml (workspace root)
[workspace]
members = [
    "core",
    "runtime",
    "cli",
]

[workspace.dependencies]
fj-math = "0.3"
```

Workspace packages share dependencies and build in topological order.

## Build Scripts

Custom build steps for native library detection:

```toml
[package]
build = "build.fj"
```

```fajar
// build.fj
fn main() {
    if system_has_library("libcuda") {
        println("cargo:rustc-cfg=feature=\"cuda\"")
    }
}
```

## Conditional Compilation

```fajar
#[cfg(target_os = "linux")]
fn platform_init() { ... }

#[cfg(feature = "gpu")]
fn gpu_compute() { ... }

#[cfg(all(target_arch = "aarch64", feature = "neon"))]
fn simd_fast_path() { ... }
```
