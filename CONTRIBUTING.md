# Contributing to Fajar Lang

Thank you for your interest in contributing to Fajar Lang! This guide covers everything you need to get started.

## Table of Contents

- [Getting Started](#getting-started)
- [Development Environment](#development-environment)
- [Building from Source](#building-from-source)
- [Development Workflow](#development-workflow)
- [Branch Strategy](#branch-strategy)
- [Commit Convention](#commit-convention)
- [Pull Request Process](#pull-request-process)
- [Code Style](#code-style)
- [Testing Requirements](#testing-requirements)
- [Finding Tasks](#finding-tasks)
- [Communication](#communication)

---

## Getting Started

### Prerequisites

- **Rust** (stable, latest) — install via [rustup](https://rustup.rs/)
- **Git** 2.30+
- **Linux, macOS, or Windows** (Linux recommended for full feature set)

Optional:
- **LLVM 18** — for the LLVM backend (`sudo apt-get install llvm-18-dev libpolly-18-dev libzstd-dev`)
- **QEMU** — for testing FajarOS examples

---

## Development Environment

### Recommended tools

- **IDE:** VS Code with the Fajar Lang extension (`editors/vscode/`)
- **Rust Analyzer:** for Rust code navigation and completion
- **cargo-watch:** `cargo install cargo-watch` for auto-rebuild on save

### Clone the repository

```bash
git clone https://github.com/fajarkraton/fajar-lang.git
cd fajar-lang
```

---

## Building from Source

```bash
# Debug build (fast compilation)
cargo build

# Release build (optimized binary)
cargo build --release

# Build with LLVM backend
cargo build --release --features llvm

# Build with native codegen tests
cargo build --features native

# Run the binary
cargo run -- run examples/hello.fj
```

The built binary is at `target/release/fj` (release) or `target/debug/fj` (debug).

---

## Development Workflow

We follow **Test-Driven Development (TDD)**. Every change must be accompanied by tests.

### The TDD cycle

1. **Think** — understand the task and read relevant documentation
2. **Design** — define the public interface first (function signatures, types, enums)
3. **Test** — write tests BEFORE implementation (RED phase)
4. **Implement** — write minimal code to make tests pass (GREEN phase)
5. **Verify** — run the full quality gate
6. **Update** — mark task complete in the relevant plan document

### Quality gate (must pass before every commit)

```bash
# Run all tests
cargo test

# Run all tests including native codegen
cargo test --features native

# Lint (zero warnings required)
cargo clippy -- -D warnings

# Format check
cargo fmt -- --check

# Apply formatting
cargo fmt
```

All three checks (test, clippy, fmt) must pass. CI will reject pull requests that fail any of these.

---

## Branch Strategy

```
main          <- stable releases only (tagged vX.Y.Z)
develop       <- integration branch (default PR target)
feat/XXX      <- feature branches (one per task or feature)
fix/XXX       <- bugfix branches
release/vX.Y  <- release preparation branches
```

### Creating a branch

```bash
# Feature branch
git checkout develop
git pull origin develop
git checkout -b feat/my-feature

# Bugfix branch
git checkout develop
git pull origin develop
git checkout -b fix/my-bugfix
```

---

## Commit Convention

We use [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<scope>): <description>
```

### Types

| Type | When to use |
|------|------------|
| `feat` | New feature or capability |
| `fix` | Bug fix |
| `test` | Adding or modifying tests |
| `refactor` | Code restructuring without behavior change |
| `docs` | Documentation only |
| `perf` | Performance improvement |
| `ci` | CI/CD configuration |
| `chore` | Build system, dependencies, tooling |

### Scopes

| Scope | Module |
|-------|--------|
| `lexer` | Tokenization |
| `parser` | Parsing and AST |
| `analyzer` | Semantic analysis, type checking |
| `interp` | Tree-walking interpreter |
| `vm` | Bytecode VM |
| `codegen` | Cranelift/LLVM/Wasm backends |
| `runtime` | OS and ML runtime |
| `cli` | Command-line interface |
| `lsp` | Language Server Protocol |
| `stdlib` | Standard library |
| `bsp` | Board support packages |

### Examples

```
feat(analyzer): add GAT constraint checking
fix(codegen): resolve i8/i64 coercion in merge blocks
test(interp): add pipeline operator evaluation tests
refactor(parser): extract macro expansion into separate module
docs(stdlib): document HashMap methods
perf(vm): optimize bytecode dispatch loop
```

---

## Pull Request Process

1. **Create your branch** from `develop` (or `main` for hotfixes)
2. **Make your changes** following the TDD workflow
3. **Run the quality gate** locally:
   ```bash
   cargo test --features native && cargo clippy -- -D warnings && cargo fmt -- --check
   ```
4. **Push your branch** and open a pull request against `develop`
5. **Fill out the PR template:**
   - Summary of changes (what and why)
   - Test plan (how to verify)
   - Related issues or tasks
6. **Respond to review feedback** promptly
7. **Squash and merge** once approved

### PR checklist

- [ ] All tests pass (`cargo test --features native`)
- [ ] Clippy clean (`cargo clippy -- -D warnings`)
- [ ] Code formatted (`cargo fmt -- --check`)
- [ ] No `.unwrap()` in `src/` (only allowed in `tests/`)
- [ ] No `unsafe` without `// SAFETY:` comment
- [ ] All `pub` items have doc comments
- [ ] New functions have at least one test
- [ ] Commit messages follow the convention

---

## Code Style

### Rust conventions

| Element | Convention | Example |
|---------|-----------|---------|
| Types, traits, enums | `PascalCase` | `TokenKind`, `FjError` |
| Functions, variables, modules | `snake_case` | `tokenize()`, `token_count` |
| Constants, statics | `SCREAMING_CASE` | `MAX_RECURSION_DEPTH` |
| Lifetimes | Short lowercase | `'src`, `'a`, `'ctx` |
| Type parameters | `PascalCase` | `T`, `U` |
| Error codes | Prefix + number | `SE004`, `KE001`, `CE003` |

### Rules

- **Maximum 50 lines per function** — break large functions into smaller ones
- **No `.unwrap()` in `src/`** — use `Result`, `Option`, or `.expect("reason")` (only in `main.rs`)
- **No `panic!()` in library code** — return errors instead
- **No `unsafe` outside `src/codegen/` and `src/runtime/os/`** — every `unsafe` block requires a `// SAFETY:` comment
- **All `pub` items must have doc comments** — use `///` for public API
- **Collect all errors** — show all diagnostics at once, not just the first one

### Dependency direction (strict)

```
ALLOWED:
  main.rs -> interpreter -> analyzer -> parser -> lexer
  main.rs -> vm -> parser -> lexer
  interpreter -> runtime/os
  interpreter -> runtime/ml
  main.rs -> codegen

FORBIDDEN:
  lexer -> parser (no upward dependencies)
  parser -> interpreter
  runtime/os <-> runtime/ml (siblings, no cross-dependencies)
  Any cycle
```

---

## Testing Requirements

### Test categories

| Category | Location | Purpose |
|----------|----------|---------|
| Unit tests | `#[cfg(test)] mod tests` in each file | Per-function testing |
| Integration | `tests/eval_tests.rs` | Full pipeline (lex -> parse -> analyze -> eval) |
| ML | `tests/ml_tests.rs` | Tensor ops, autograd, optimizers |
| OS | `tests/os_tests.rs` | Memory, IRQ, syscall |
| Safety | `tests/safety_tests.rs` | Move, borrow, overflow, bounds |
| Property | `tests/property_tests.rs` | Proptest invariants |

### Test naming convention

```rust
// Pattern: <what>_<when>_<expected>
fn lexer_produces_int_token_for_decimal_literal() { ... }
fn analyzer_rejects_heap_alloc_in_kernel_context() { ... }
```

### Running tests

```bash
# All default tests
cargo test

# Include native codegen tests
cargo test --features native

# Specific test file
cargo test --test eval_tests

# Specific test pattern
cargo test -- fibonacci

# With output
cargo test -- --nocapture
```

---

## Finding Tasks

### Current development

- **v0.7 tasks** — `docs/V07_PLAN.md` (if active)
- **v0.6 tasks** — `docs/V06_PLAN.md` (complete, for reference)

### Where to look for work

1. **GitHub Issues** — [github.com/fajarkraton/fajar-lang/issues](https://github.com/fajarkraton/fajar-lang/issues)
2. **Plan documents** — look for unchecked `[ ]` tasks in `docs/` plan files
3. **TODO comments** — `grep -r "TODO" src/` in the codebase
4. **Clippy suggestions** — run `cargo clippy` and fix warnings

### Good first issues

Look for issues labeled `good-first-issue` on GitHub. These are smaller, well-scoped tasks suitable for new contributors.

---

## Communication

- **GitHub Issues** — bug reports, feature requests
- **GitHub Discussions** — questions, ideas, show-and-tell
- **Pull Requests** — code review and collaboration
- **Email** — fajar@primecore.id (project lead)
- **Security** — security@primecore.id (vulnerability reports only, see [SECURITY.md](SECURITY.md))

### Code of Conduct

All contributors are expected to follow our [Code of Conduct](CODE_OF_CONDUCT.md). We are committed to providing a welcoming and inclusive environment for everyone.

---

## Community Channels

We want every contributor to feel connected and supported. Here is where to find us:

| Channel | Purpose | Link |
|---------|---------|------|
| **GitHub Discussions** | Long-form questions, RFCs, show-and-tell | [github.com/fajarkraton/fajar-lang/discussions](https://github.com/fajarkraton/fajar-lang/discussions) |
| **Discord** | Real-time chat, help, and community | [discord.gg/fajarlang](https://discord.gg/fajarlang) |
| **Weekly Office Hours** | Live Q&A with maintainers (every Thursday 14:00 UTC) | Held on Discord in the `#office-hours` voice channel |

Discord channels of note:
- `#general` -- introductions and casual discussion
- `#help` -- get help with Fajar Lang code or compiler issues
- `#contributions` -- discuss open tasks, get guidance on PRs
- `#showcase` -- share what you have built with Fajar Lang
- `#os-dev` -- FajarOS kernel development
- `#ml` -- embedded ML, tensor ops, model training

---

## Mentorship Program

We run a mentorship program for contributors who want structured guidance while working on Fajar Lang. The program pairs new contributors with experienced maintainers for a focused 3-month engagement.

### How it works

1. **Apply** -- open a GitHub Discussion in the "Mentorship" category with:
   - Your background (programming experience, familiarity with Rust/compilers)
   - What area of Fajar Lang interests you (compiler, runtime, ML, OS, tooling, docs)
   - How many hours per week you can commit (minimum 4 hours/week recommended)
2. **Matching** -- a maintainer with expertise in your area of interest will be assigned as your mentor within 2 weeks
3. **Kickoff** -- you and your mentor schedule a 30-minute video call to set goals for the 3 months
4. **Work** -- your mentor will:
   - Help you pick appropriate tasks (starting with `good-first-issue`, progressing to larger features)
   - Review your PRs with detailed, educational feedback
   - Hold biweekly 1-on-1 check-ins (15-30 minutes)
   - Answer questions async on Discord
5. **Completion** -- at the end of 3 months, you and your mentor write a short retrospective. Successful mentees are invited to mentor future contributors.

### Expectations

- **Mentees:** commit to regular participation, ask questions early, submit at least 2 PRs during the program
- **Mentors:** respond within 48 hours, provide constructive and patient feedback, respect the mentee's pace

The program runs on a rolling basis -- you can apply at any time.

---

## Contributor Recognition

Every contribution to Fajar Lang matters and we make sure contributors are recognized.

### How we recognize contributions

| Recognition | Details |
|-------------|---------|
| **CHANGELOG credits** | Every contributor whose PR is merged is listed by name in the [CHANGELOG](docs/CHANGELOG.md) for the corresponding release |
| **Monthly spotlight** | Each month, one contributor is highlighted in a blog post on [fajarlang.dev/blog](https://fajarlang.dev/blog) describing their work and impact |
| **All-Contributors bot** | We use the [all-contributors](https://allcontributors.org/) specification to recognize all types of contributions (code, docs, tests, design, mentoring, translations) in the README |
| **Release notes** | Major feature contributors are credited in GitHub Release notes |
| **Security researchers** | Valid security reports are credited in release notes (with permission) per our [Bug Bounty Program](SECURITY.md#bug-bounty-program) |

### Contribution types we recognize

Not all contributions are code. We value and credit:
- Bug reports with reproducible test cases
- Documentation improvements and translations
- Test coverage additions
- Reviewing pull requests
- Helping others in GitHub Discussions or Discord
- Writing tutorials, blog posts, or conference talks about Fajar Lang
- Design work (logos, website, diagrams)

To add yourself to the contributors list, comment on your merged PR with:
```
@all-contributors please add @username for code, test, doc
```

---

## Architecture Overview

For a detailed understanding of the codebase before making changes, see:

- [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) — system design and module contracts
- [`docs/FAJAR_LANG_SPEC.md`](docs/FAJAR_LANG_SPEC.md) — language specification
- [`docs/ERROR_CODES.md`](docs/ERROR_CODES.md) — error code catalog (78+ codes)
- [`CLAUDE.md`](CLAUDE.md) — comprehensive project reference (auto-loaded by Claude Code)

---

Thank you for contributing to Fajar Lang! Every contribution, from fixing a typo to adding a new backend, helps make the language better.
