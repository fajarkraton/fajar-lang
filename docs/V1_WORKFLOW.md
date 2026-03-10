# Workflow — Fajar Lang v1.0 (Embedded ML + OS)

> Target: "Bahasa terbaik untuk embedded ML + OS integration"
> Timeline: 6 bulan → v1.0 release

---

## 1. Development Philosophy

```
CORRECTNESS > SAFETY > USABILITY > PERFORMANCE

"If it compiles in Fajar Lang, it's safe to deploy on hardware."
```

### Core Workflow Loop

```
┌──────────────────────────────────────────────────────────┐
│                    SPRINT CYCLE (1 week)                  │
│                                                          │
│   ┌─ PLAN ──→ DESIGN ──→ TEST ──→ IMPL ──→ VERIFY ─┐   │
│   │                                                  │   │
│   │  1. Read spec + architecture docs                │   │
│   │  2. Write public interface (types, fn sigs)      │   │
│   │  3. Write tests FIRST (RED)                      │   │
│   │  4. Implement minimally (GREEN)                  │   │
│   │  5. cargo test + clippy + fmt + bench            │   │
│   │  6. Update TASKS, commit, next task              │   │
│   │                                                  │   │
│   └──────────────────────────────────────────────────┘   │
│                                                          │
│   End of Sprint:                                         │
│   • All tests pass                                       │
│   • Benchmarks run (no regressions)                      │
│   • Examples updated                                     │
│   • CHANGELOG entry written                              │
│   • Tag: v0.X.Y                                          │
└──────────────────────────────────────────────────────────┘
```

---

## 2. Session Protocol

Every Claude Code session:

```
1. READ     → CLAUDE.md (auto-loaded)
2. READ     → docs/V1_TASKS.md (find next uncompleted task)
3. READ     → docs/V1_RULES.md (coding conventions)
4. ORIENT   → "What is the next uncompleted task?"
5. ACT      → Execute per TDD (test → impl → verify)
6. VERIFY   → cargo test && clippy && fmt && bench (if applicable)
7. UPDATE   → Mark task [x] in V1_TASKS.md
8. COMMIT   → Only when user requests
```

---

## 3. Quality Gates

### Per-Task Gate
- [ ] Tests written BEFORE implementation
- [ ] All tests pass
- [ ] No `.unwrap()` in `src/` (only tests)
- [ ] All `pub` items have `///` doc comments
- [ ] `cargo clippy -- -D warnings` clean
- [ ] `cargo fmt -- --check` clean

### Per-Sprint Gate
- [ ] All sprint tasks completed
- [ ] No test regressions
- [ ] Benchmark comparison (no >10% regression)
- [ ] At least 1 new example program
- [ ] CHANGELOG updated

### Per-Milestone Gate (v0.X.0)
- [ ] All milestone tasks completed
- [ ] Full test suite passes
- [ ] All examples run correctly
- [ ] cargo doc compiles
- [ ] Release notes written
- [ ] Git tag created

---

## 4. Branch Strategy

```
main          ← stable releases only (tagged v0.X.Y)
develop       ← integration branch (PR target)
feat/XXX      ← feature branches (1 per sprint task)
fix/XXX       ← bugfix branches
release/v0.X  ← release preparation
```

### Commit Convention

```
<type>(<scope>): <description>

Types: feat, fix, test, refactor, docs, perf, ci, chore
Scope: lexer, parser, analyzer, interp, runtime, vm, codegen, cli, stdlib

Examples:
  feat(analyzer): implement move semantics checking
  fix(interp): handle division by zero for floats
  perf(vm): optimize constant folding pass
  test(stdlib): add HashMap integration tests
```

---

## 5. CI/CD Pipeline

```yaml
# .github/workflows/ci.yml
on: [push, pull_request]

jobs:
  test:
    strategy:
      matrix:
        os: [ubuntu-latest, macos-latest]
        rust: [stable, nightly]
    steps:
      - cargo fmt -- --check
      - cargo clippy -- -D warnings
      - cargo test --all-targets
      - cargo test --doc
      - cargo bench --no-run  # compile benchmarks only

  coverage:
    steps:
      - cargo tarpaulin --out Xml
      - upload coverage to codecov

  release:
    if: startsWith(github.ref, 'refs/tags/v')
    steps:
      - cargo build --release
      - create GitHub release with binary
```

---

## 6. Release Schedule

```
v0.2.0  — Month 1   — Native compilation (Cranelift MVP)
v0.3.0  — Month 2   — Generics + Traits + FFI
v0.4.0  — Month 3   — Ownership system + borrow checker
v0.5.0  — Month 4   — Embedded ML stdlib (Conv2d, Attention, DataLoader)
v0.6.0  — Month 5   — Cross-compilation + embedded targets
v1.0.0  — Month 6   — Production release (self-hosting goal)
```

---

## 7. Testing Pyramid

```
                    ┌─────────┐
                    │   E2E   │  .fj programs → expected output
                   ─┤  (30+)  ├─
                  / └─────────┘ \
                 /   Integration  \
                ─┤    (200+)      ├─   Cross-component tests
               / └───────────────┘ \
              /     Unit Tests       \
             ─┤      (800+)          ├─  Per-function tests
            / └─────────────────────┘ \
           /      Property Tests        \
          ─┤       (50+)                ├─  proptest invariants
         / └───────────────────────────┘ \
        /         Fuzzing                  \
       ─┤        (continuous)              ├─  cargo-fuzz
        └─────────────────────────────────┘
```

---

*V1_WORKFLOW.md v1.0 — Created 2026-03-05*
