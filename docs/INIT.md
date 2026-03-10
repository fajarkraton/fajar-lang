# INIT.md — First Session Initialization Guide

> Dokumen ini KHUSUS untuk sesi pertama Claude Code. Setelah project di-scaffold, tidak perlu lagi.

## Tujuan Dokumen Ini

Panduan langkah-demi-langkah untuk memulai project Fajar Lang dari nol di Claude Code dengan Claude Opus 4.6. Ikuti urutan ini SEKALI di sesi pertama.

## Pre-requisites

Pastikan semua terinstall sebelum memulai:

```bash
# Rust (latest stable)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup update stable
rustup component add clippy rustfmt

# Claude Code
npm install -g @anthropic-ai/claude-code

# Git
git --version

# Verify
cargo --version   # should be 1.75+
claude --version
git --version
```

## Step 1 — Create Project

```bash
# Create new Rust project
cargo new fajar-lang
cd fajar-lang

# Initialize git
git init
git add .
git commit -m "chore: initial cargo project"

# Create branch for Phase 1
git checkout -b phase-1
```

## Step 2 — Copy Documentation

```bash
# Create docs directory
mkdir -p docs

# Copy all documentation files from fajar-lang-docs/
cp fajar-lang-docs/CLAUDE.md ./CLAUDE.md
cp fajar-lang-docs/docs/FAJAR_LANG_SPEC.md docs/
cp fajar-lang-docs/docs/ARCHITECTURE.md docs/
cp fajar-lang-docs/docs/WORKFLOW.md docs/
cp fajar-lang-docs/docs/RULES.md docs/
cp fajar-lang-docs/docs/PLANNING.md docs/
cp fajar-lang-docs/docs/TASKS.md docs/
cp fajar-lang-docs/docs/SKILLS.md docs/
cp fajar-lang-docs/docs/AGENTS.md docs/
# ... copy all other docs

# Commit docs
git add .
git commit -m "docs: add all Fajar Lang documentation"
```

## Step 3 — Setup Cargo.toml

Replace the default Cargo.toml with:

```toml
[package]
name = "fajar-lang"
version = "0.1.0"
edition = "2021"
description = "Fajar Lang — Systems language for OS and AI/ML"
license = "MIT"

[[bin]]
name = "fj"
path = "src/main.rs"

[dependencies]
logos = "0.14"
thiserror = "2.0"
miette = "7.0"
clap = { version = "4.5", features = ["derive"] }
rustyline = "14.0"
ndarray = "0.16"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
indexmap = "2.0"

[dev-dependencies]
criterion = "0.5"
pretty_assertions = "1.4"

[profile.release]
opt-level = 3
lto = true
```

## Step 4 — Create Directory Structure

```bash
# Create source directories
mkdir -p src/{lexer,parser,analyzer,interpreter}
mkdir -p src/runtime/{os,ml}
mkdir -p src/stdlib
mkdir -p tests examples benches

# Create placeholder files
touch src/lexer/{mod.rs,token.rs,cursor.rs}
touch src/parser/{mod.rs,ast.rs,pratt.rs}
touch src/analyzer/{mod.rs,type_check.rs,scope.rs,borrow_lite.rs}
touch src/interpreter/{mod.rs,env.rs,eval.rs,value.rs}
touch src/runtime/os/{mod.rs,memory.rs,irq.rs,syscall.rs}
touch src/runtime/ml/{mod.rs,tensor.rs,autograd.rs,ops.rs,optim.rs}
touch src/stdlib/{os.rs,nn.rs}
touch tests/{lexer_tests.rs,parser_tests.rs,eval_tests.rs,os_tests.rs,ml_tests.rs}
touch benches/interpreter_bench.rs
```

## Step 5 — Create Minimal Rust Files

Create minimal content so `cargo build` succeeds:

**src/lib.rs:**
```rust
//! # Fajar Lang
//!
//! A systems programming language for OS development and AI/ML.
//! Built with a Rust interpreter.

pub mod lexer;
pub mod parser;
pub mod analyzer;
pub mod interpreter;
pub mod runtime;

/// Top-level error type for the Fajar Lang pipeline.
#[derive(Debug)]
pub enum FjError {
    // Will be expanded in Phase 1
    NotImplemented,
}
```

**src/main.rs:**
```rust
//! Fajar Lang CLI entry point.
fn main() {
    println!("Fajar Lang v0.1.0");
    println!("Run `fj --help` for usage (coming in Sprint 1.6)");
}
```

**src/lexer/mod.rs:**
```rust
//! Fajar Lang lexer — converts source code to tokens.
pub mod token;
pub mod cursor;
```

**src/parser/mod.rs:**
```rust
//! Fajar Lang parser — converts tokens to AST.
pub mod ast;
pub mod pratt;
```

**src/analyzer/mod.rs:**
```rust
//! Semantic analyzer — type checking and name resolution.
pub mod type_check;
pub mod scope;
pub mod borrow_lite;
```

**src/interpreter/mod.rs:**
```rust
//! Tree-walking interpreter for Fajar Lang.
pub mod env;
pub mod eval;
pub mod value;
```

**src/runtime/mod.rs:**
```rust
//! Fajar Lang runtime — OS and ML execution backends.
pub mod os;
pub mod ml;
```

**src/runtime/os/mod.rs:**
```rust
//! OS runtime — memory, IRQ, syscall primitives.
pub mod memory;
pub mod irq;
pub mod syscall;
```

**src/runtime/ml/mod.rs:**
```rust
//! ML runtime — tensor operations and autograd.
pub mod tensor;
pub mod autograd;
pub mod ops;
pub mod optim;
```

## Step 6 — Create Example Files

**examples/hello.fj:**
```fajar
// Hello World in Fajar Lang
use std::io::println

fn main() -> void {
    println("Hello from Fajar Lang!")
}
```

**examples/fibonacci.fj:**
```fajar
// Fibonacci in Fajar Lang
fn fibonacci(n: i32) -> i32 {
    if n <= 1 {
        n
    } else {
        fibonacci(n - 1) + fibonacci(n - 2)
    }
}

fn main() -> void {
    for i in 0..10 {
        println(fibonacci(i))
    }
}
```

## Step 7 — Verify Build

```bash
cargo build 2>&1
# Should succeed (warnings OK at this stage)

cargo test 2>&1
# Should show: running 0 tests

cargo clippy 2>&1
# Review warnings
```

## Step 8 — Commit & Start Development

```bash
git add .
git commit -m "chore: scaffold project structure with all modules"
```

## Step 9 — Start Claude Code

```bash
# Start Claude Code in project directory
cd fajar-lang
claude

# First message to Claude Code:
```

```
I'm starting development of Fajar Lang — a systems programming language for
OS development and AI/ML. The project is scaffolded and documented.

Please:
1. Read CLAUDE.md (the init file)
2. Read docs/PLANNING.md
3. Read docs/TASKS.md
4. Confirm what the next task is and begin

Use Claude Opus 4.6 with high effort for architecture questions.
We are starting at Task T1.1.1 in Sprint 1.1 (Lexer).
```

---

*INIT.md Version: 1.0 | Use ONCE for first session only*
