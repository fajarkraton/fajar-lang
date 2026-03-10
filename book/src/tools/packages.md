# Package Manager

## Project Configuration

Every Fajar project has a `fj.toml`:

```toml
[package]
name = "my_project"
version = "0.1.0"
edition = "2026"

[build]
target = "x86_64"
opt_level = "speed"

[dependencies]
# future: dependency resolution
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

## Standard Library

Fajar includes built-in modules:

| Module | Contents |
|--------|----------|
| `std::io` | print, println, read_file, write_file |
| `std::math` | PI, E, sqrt, sin, cos, abs, pow |
| `std::string` | trim, split, replace, contains |
| `std::collections` | Array methods, HashMap |
| `nn::tensor` | zeros, ones, randn, xavier |
| `nn::ops` | matmul, relu, sigmoid, softmax |
| `os::memory` | mem_alloc, page_map |
