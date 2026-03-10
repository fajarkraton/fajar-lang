# Demo: Package Project

A multi-dependency project demonstrating Fajar Lang's package system.

## Project Structure

```
sensor-classifier/
├── fj.toml           # Package manifest with dependencies
└── src/
    └── main.fj       # Entry point
```

## fj.toml

```toml
[package]
name = "sensor-classifier"
version = "0.1.0"
entry = "src/main.fj"

[dependencies]
fj-math = "^0.1.0"
fj-nn = "^0.1.0"
fj-hal = "^0.1.0"
```

## Package Commands

```bash
# Create a new project
fj new my-project

# Add a dependency
fj add fj-math --version "^0.1.0"

# Build the project
fj build

# Run the project
fj run

# Publish to local registry
fj publish
```

## Available Standard Packages

| Package | Description |
|---------|-------------|
| `fj-math` | Math utilities (vectors, matrices) |
| `fj-nn` | Neural network layers and ops |
| `fj-hal` | Hardware abstraction layer |
| `fj-drivers` | Device drivers |
| `fj-http` | HTTP client/server |
| `fj-json` | JSON parsing/serialization |
| `fj-crypto` | Cryptographic utilities |

## Dependency Resolution

Fajar Lang supports:
- **Semantic versioning**: `1.2.3`
- **Caret constraints**: `^1.0.0` (compatible updates)
- **Tilde constraints**: `~1.2.0` (patch updates only)
- **Range constraints**: `>=1.0.0`, `<2.0.0`
- **Transitive resolution**: Dependencies of dependencies are resolved automatically
- **Lock files**: `fj.lock` ensures reproducible builds

## Source

See `examples/package_demo/` for the full project.
