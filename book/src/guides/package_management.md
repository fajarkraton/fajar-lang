# Package Management Guide

Fajar Lang has a built-in package manager for creating, publishing,
and consuming reusable libraries.

## Creating a New Package

```bash
fj new my_library --lib     # Create a library package
fj new my_app               # Create a binary package
```

Library packages have `src/lib.fj` as the entry point instead of `src/main.fj`.

## Adding Dependencies

Edit `fj.toml` manually:

```toml
[dependencies]
fj-math = "0.1"
fj-json = "0.2"
```

Or use the CLI:

```bash
fj add fj-math              # Add latest version
fj add fj-json@0.2          # Add specific version
fj add fj-bench --dev       # Add as dev dependency
```

## Standard Packages

| Package | Domain | Description |
|---------|--------|-------------|
| `fj-math` | General | Math functions, linear algebra, statistics |
| `fj-nn` | ML | Neural network layers, optimizers, losses |
| `fj-hal` | Embedded | Hardware abstraction layer traits |
| `fj-drivers` | Embedded | GPIO, SPI, I2C, UART drivers |
| `fj-http` | Network | HTTP client and server |
| `fj-json` | Data | JSON parser and serializer |
| `fj-crypto` | Security | AES, SHA, HMAC, key derivation |

## Using a Package

After adding a dependency, import it in your `.fj` code:

```fajar
use fj_math::sqrt
use fj_json::parse

fn main() {
    let val = sqrt(144.0)
    println(f"Square root: {val}")

    let data = parse("{\"key\": 42}")
    println(f"Parsed: {data}")
}
```

## Publishing a Package

### Prepare

Ensure your `fj.toml` has the required fields:

```toml
[package]
name = "my-utils"
version = "0.1.0"
description = "Utility functions for Fajar Lang"
license = "MIT"
repository = "https://github.com/you/my-utils"
```

### Publish

```bash
fj publish                   # Publish to the registry
```

### Versioning

Follow semantic versioning (SemVer):
- **0.1.0 -> 0.1.1** -- patch (bug fixes)
- **0.1.0 -> 0.2.0** -- minor (new features, backward compatible)
- **0.1.0 -> 1.0.0** -- major (breaking changes)

## Searching and Installing

```bash
fj search math               # Search the registry
fj install fj-math           # Install globally
```

## Lock File

`fj.lock` is generated automatically and records exact dependency versions.
Commit this file for reproducible builds.

```bash
fj update                    # Update dependencies to latest compatible versions
fj update fj-math            # Update a specific dependency
```

## Private Registries

Configure a private registry in `~/.fj/config.toml`:

```toml
[registries.internal]
url = "https://registry.company.com/fj"
token = "your-auth-token"
```

Then use:

```toml
[dependencies]
internal-lib = { version = "1.0", registry = "internal" }
```
