# CONTRIBUTING

> Panduan Kontribusi & Development — Fajar Lang

---

## 1. Development Model

Fajar Lang menggunakan model pengembangan hybrid: AI-assisted development dengan Claude Opus 4.6 melalui Claude Code, dipandu oleh dokumentasi yang ketat.

| Aspek | Pendekatan |
|-------|------------|
| Primary Developer | Claude Opus 4.6 via Claude Code |
| Architecture Review | Human (Fajar) + Orchestrator Agent |
| Code Quality | Automated: `cargo test` + `clippy` + `fmt` |
| Branch Strategy | Git Flow: `main`, `phase-N` branches |
| Review Process | Per-sprint: human review pada phase transition |

---

## 2. Git Workflow

### 2.1 Branch Naming

```
main              # stable releases only (tagged v0.X.Y)
develop           # integration branch (PR target)
feat/XXX          # feature branches (1 per sprint task)
fix/XXX           # bugfix branches
release/v0.X      # release preparation
```

### 2.2 Commit Convention

```
Format: <type>(<scope>): <description>

Types:
  feat     — new feature
  fix      — bug fix
  test     — adding/updating tests
  refactor — code restructuring
  docs     — documentation only
  chore    — build, config, tooling
  perf     — performance improvement

Examples:
  feat(lexer): implement string literal tokenization
  fix(parser): handle trailing comma in function args
  test(interpreter): add fibonacci integration test
  docs(planning): mark Sprint 1.1 complete
```

### 2.3 Sprint / Release Checklist

Sebelum merge ke `main`:

- [ ] Semua tasks di V1_TASKS.md untuk sprint tersebut: **DONE**
- [ ] `cargo test`: 100% passing
- [ ] `cargo clippy -- -D warnings`: zero warnings
- [ ] `cargo fmt -- --check`: formatted
- [ ] `cargo doc`: compiles cleanly
- [ ] `examples/` semua berjalan
- [ ] CHANGELOG.md: updated dengan fitur baru
- [ ] Human review: arsitektur dan keputusan desain

---

## 3. Code Review Standards

Setiap kode harus memenuhi checklist berikut sebelum commit:

| Kategori | Requirement | Automated? |
|----------|-------------|------------|
| Correctness | Semua test pass | ✅ (`cargo test`) |
| Linting | Zero clippy warnings | ✅ (`cargo clippy`) |
| Formatting | Consistent style | ✅ (`cargo fmt`) |
| Documentation | Semua pub items have doc comments | ✅ (`cargo doc`) |
| Safety | No `.unwrap()` in `src/` (only tests) | Manual review |
| Safety | `// SAFETY:` comment on setiap `unsafe` block | Manual review |
| Testing | Setiap function baru punya minimal 1 test | Manual review |
| Architecture | Dependency direction sesuai ARCHITECTURE.md | Manual review |
| Tasks | TASKS.md updated | Manual check |

---

## 4. Adding New Features

Untuk menambahkan fitur baru ke Fajar Lang, ikuti urutan ini:

1. Baca **FAJAR_LANG_SPEC.md** — pastikan fitur sudah ada di spec
2. Baca **ARCHITECTURE.md** — pahami di mana komponen berada
3. Update **V1_TASKS.md** — tambahkan task baru jika belum ada
4. Tulis **test DULU** (TDD) — test harus gagal awalnya
5. **Implementasi minimal** — hanya cukup agar test lulus
6. **Refactor** — perbaiki kode, test tetap hijau
7. Run full check: `cargo test && cargo clippy && cargo fmt`
8. Update **V1_TASKS.md** — mark task done
9. Commit dengan conventional commit format

---

## 5. File Ownership

| Directory | Owner Agent | Primary Concern |
|-----------|-------------|-----------------|
| `src/lexer/` | Language Core Agent | Tokenization |
| `src/parser/` | Language Core Agent | AST generation |
| `src/analyzer/` | Language Core Agent | Type checking, context validation |
| `src/interpreter/` | Orchestrator | Evaluation pipeline |
| `src/runtime/os/` | OS Runtime Agent | Memory, IRQ, syscall |
| `src/runtime/ml/` | ML Runtime Agent | Tensor, autograd, ops |
| `src/codegen/` | Codegen Agent | Cranelift native compilation |
| `src/vm/` | Language Core Agent | Bytecode compilation + VM |
| `src/formatter/` | Tooling Agent | fj fmt |
| `src/lsp/` | Tooling Agent | Language Server Protocol |
| `src/package/` | Tooling Agent | Package manager (fj.toml) |
| `src/stdlib/` | Orchestrator | Stdlib Rust bindings |
| `src/main.rs` | Tooling Agent | CLI & REPL |
| `stdlib/` | Orchestrator | Fajar Lang stdlib (.fj) |
| `packages/` | Orchestrator | Core packages (fj-hal, fj-nn, etc.) |
| `docs/` | Orchestrator | All documentation |
| `tests/` | Respective component owner | Test files |
| `examples/` | Orchestrator | Example programs |

---

## 6. Quick Commands

```bash
# Build & Test
cargo build                           # debug build
cargo test                            # run all tests
cargo clippy -- -D warnings           # linting
cargo fmt                             # format code

# Run Fajar Lang programs
cargo run -- run examples/hello.fj    # execute program
cargo run -- repl                     # interactive REPL
cargo run -- check file.fj            # type-check only

# Documentation
cargo doc --open                      # generate + view docs
```

---

## 7. Roadmap

See `docs/ROADMAP_V1.1.md` for planned features in the next release.

---

*Contributing Version: 2.0 | v1.0 Release | AI-Assisted Development with Human Oversight*
