# Fajar Lang for VS Code

**The official VS Code extension for [Fajar Lang](https://github.com/fajarkraton/fajar-lang)** — a systems programming language for embedded ML & OS development.

## Features

### Syntax Highlighting
Full TextMate grammar for `.fj` files with support for:
- Keywords (`fn`, `let`, `struct`, `enum`, `trait`, `async`, `await`)
- Context annotations (`@kernel`, `@device`, `@safe`, `@unsafe`)
- ML/tensor operations, OS primitives
- String interpolation (`f"Hello {name}"`)
- Doc comments (`///`)

### Language Server Protocol (LSP)
Connect to the `fj lsp` server for:
- **Diagnostics** — real-time error highlighting
- **Inlay hints** — type annotations, parameter names
- **Signature help** — parameter info as you type
- **Go to definition** — jump to function/struct/trait definitions
- **Hover** — type info and documentation
- **Completion** — context-aware auto-complete
- **Rename** — safe symbol renaming
- **Semantic tokens** — rich syntax coloring
- **Code lens** — reference counts, test runners

### Debugging (DAP)
Debug `.fj` programs with the built-in DAP adapter:
- Breakpoints (line, conditional, logpoints)
- Step over/into/out
- Variable inspection
- Call stack viewing

### Code Snippets
Quick-start snippets for common patterns:
- `fn` — function definition
- `struct` — struct with fields
- `test` — `@test` function
- `kernel` — `@kernel` annotated function
- `device` — `@device` annotated function

### Task Integration
Build tasks integrated with VS Code:
- **Ctrl+Shift+B** — `fj build`
- Task definitions for `fj run`, `fj test`, `fj check`, `fj fmt`

## Requirements

- [Fajar Lang compiler](https://github.com/fajarkraton/fajar-lang) (`fj` binary) installed and in PATH
- VS Code 1.75.0 or later

## Installation

### From Marketplace
Search "Fajar Lang" in VS Code Extensions (Ctrl+Shift+X) and click Install.

### From Source
```bash
git clone https://github.com/fajarkraton/fajar-lang
cd fajar-lang/editors/vscode
code --install-extension fajar-lang-9.0.1.vsix
```

## Configuration

| Setting | Default | Description |
|---------|---------|-------------|
| `fajarLang.server.path` | `fj` | Path to the Fajar Lang binary |
| `fajarLang.debug.fjPath` | `fj` | Path to fj binary for debugging |
| `fajarLang.debug.stopOnEntry` | `true` | Stop at program entry point |

## About Fajar Lang

Fajar Lang (`fj`) is a statically-typed systems programming language for embedded ML and OS development. 340K LOC Rust compiler, 7,468 tests, Cranelift + LLVM backends, real async/await, WebSocket, MQTT, HTTP framework, and more.

- **GitHub:** [github.com/fajarkraton/fajar-lang](https://github.com/fajarkraton/fajar-lang)
- **Docs:** [fajarkraton.github.io/fajar-lang](https://fajarkraton.github.io/fajar-lang/)
- **License:** MIT

---

Made with care in Indonesia by [Muhamad Fajar Putranto](https://github.com/fajarkraton)
