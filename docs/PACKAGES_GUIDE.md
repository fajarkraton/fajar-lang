# Fajar Lang — Package Guide

> Create, publish, and manage packages for the Fajar Lang ecosystem.

---

## Quick Start

### Create a New Package

```bash
# Create a new package project
fj new my-package
cd my-package

# Structure created:
# my-package/
#   fj.toml       — package manifest
#   src/
#     lib.fj      — library entry point
```

### fj.toml Manifest

```toml
[package]
name = "my-package"
version = "1.0.0"
description = "My awesome Fajar Lang package"
authors = ["Your Name <you@example.com>"]
license = "MIT"
entry = "src/lib.fj"

[dependencies]
fj-math = "^3.0"
fj-json = "^1.0"
```

### Add Dependencies

```bash
# Add a package dependency
fj add fj-math

# Add with version constraint
fj add fj-json@^1.0

# Remove a dependency
fj remove fj-json

# Update all dependencies
fj update
```

### Build and Test

```bash
# Build the package
fj build

# Run tests
fj test

# Format source
fj fmt
```

---

## Publishing

### Login to Registry

```bash
# Get an API token from registry.fajarlang.dev
fj login --token <YOUR_API_TOKEN>
```

### Publish

```bash
# Validate and publish
fj publish

# This will:
# 1. Validate fj.toml (name, version, entry)
# 2. Run fj check (type-check all files)
# 3. Create tarball (.fjpkg)
# 4. Sign with Ed25519 key
# 5. Upload to registry
```

### Yank a Version

```bash
# Hide a broken version (does not delete)
fj yank my-package 1.0.0
```

---

## Standard Packages

| Package | Version | Description |
|---------|---------|-------------|
| **fj-math** | 3.0.0 | Math functions: trig, linear algebra, statistics |
| **fj-nn** | 3.0.0 | Neural networks: Dense, Conv2d, LSTM, optimizers |
| **fj-hal** | 3.0.0 | Hardware abstraction: GPIO, I2C, SPI, UART |
| **fj-drivers** | 3.0.0 | Device drivers: NVMe, USB, VirtIO |
| **fj-http** | 3.0.0 | HTTP client/server, REST API |
| **fj-json** | 3.0.0 | JSON parse/serialize |
| **fj-crypto** | 3.0.0 | Cryptography: SHA-256, AES, Ed25519 |

### Using a Standard Package

```fajar
use fj_math::sqrt
use fj_math::PI

fn circle_area(r: f64) -> f64 {
    PI * r * r
}

fn main() -> void {
    println(f"Area of r=5: {circle_area(5.0)}")
    println(f"sqrt(2) = {sqrt(2.0)}")
}
```

---

## Dependency Resolution

Fajar Lang uses a **PubGrub solver** for dependency resolution — the same algorithm used by Dart's pub and Swift Package Manager.

### Version Constraints

| Syntax | Meaning |
|--------|---------|
| `"^1.2.3"` | Compatible: >=1.2.3, <2.0.0 |
| `"~1.2.3"` | Patch-level: >=1.2.3, <1.3.0 |
| `">=1.0, <2.0"` | Range |
| `"1.2.3"` | Exact version |
| `"*"` | Any version |

### Lockfile

`fj.lock` pins exact resolved versions:

```toml
[[package]]
name = "fj-math"
version = "3.0.0"
checksum = "sha256:abc123..."

[[package]]
name = "fj-json"
version = "1.2.1"
checksum = "sha256:def456..."
dependencies = ["fj-math ^3.0"]
```

---

## Security

### Package Signing

All published packages are signed with Ed25519:

```bash
# Generate signing key (first time)
fj keygen

# Key stored in ~/.fj/signing-key.ed25519
# Public key published to registry with your account
```

### Verification

```bash
# Verify package signature
fj verify my-package 1.0.0

# Audit dependencies for known vulnerabilities
fj audit
```

### SBOM

```bash
# Generate Software Bill of Materials
fj sbom --format cyclonedx

# Output: sbom.json with full dependency tree
```

---

## Package Directory Structure

```
my-package/
  fj.toml           — manifest (name, version, deps)
  fj.lock            — lockfile (pinned versions)
  src/
    lib.fj           — library entry point (pub functions)
    utils.fj         — internal modules
  tests/
    test_main.fj     — test files
  examples/
    basic.fj         — usage examples
  README.md          — package documentation
  LICENSE            — license file
```

---

## Creating a Package from Scratch

### 1. Initialize

```bash
mkdir fj-hello && cd fj-hello
```

### 2. Create fj.toml

```toml
[package]
name = "fj-hello"
version = "0.1.0"
description = "Hello world package for Fajar Lang"
authors = ["Fajar <fajar@primecore.id>"]
license = "MIT"
entry = "src/lib.fj"
```

### 3. Write Library Code

```fajar
// src/lib.fj

pub fn greet(name: str) -> str {
    f"Hello, {name}! Welcome to Fajar Lang."
}

pub fn greet_world() -> str {
    greet("World")
}

pub fn repeat_greet(name: str, times: i64) -> [str] {
    let mut greetings = []
    let mut i = 0
    while i < times {
        greetings.push(greet(name))
        i = i + 1
    }
    greetings
}
```

### 4. Write Tests

```fajar
// tests/test_main.fj

use fj_hello::greet
use fj_hello::greet_world

@test fn test_greet() {
    assert_eq(greet("Alice"), "Hello, Alice! Welcome to Fajar Lang.")
}

@test fn test_greet_world() {
    assert(greet_world().contains("World"))
}
```

### 5. Publish

```bash
fj login --token $FJ_TOKEN
fj publish
# => Published fj-hello@0.1.0 (signed, sha256:...)
```

---

## Registry API

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/packages` | GET | List/search packages |
| `/api/packages/:name` | GET | Package details + versions |
| `/api/packages/:name/:version/download` | GET | Download tarball |
| `/api/publish` | POST | Upload package (auth required) |
| `/api/yank/:name/:version` | POST | Yank version (auth required) |
| `/api/login` | POST | Get API token |

---

*Package Guide — Fajar Lang v6.1.0*
*Registry: registry.fajarlang.dev*
