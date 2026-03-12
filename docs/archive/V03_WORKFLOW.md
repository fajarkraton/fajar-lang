# Workflow — Fajar Lang v0.3 "Dominion"

> Target: Make Fajar Lang a real contender for OS kernel + AI/ML infrastructure
> Timeline: 12 months, 52 sprints
> Baseline: v0.2 complete (1,991 tests, 59K LOC)
> Current (2026-03-09): 2,161 tests, ~75K LOC, S1-S8 complete

---

## 1. Development Philosophy

```
CORRECTNESS > SAFETY > USABILITY > PERFORMANCE

"If it compiles in Fajar Lang, it's safe to deploy on hardware AND train a model."
```

### Core Principles (v0.3 additions)

1. **CORRECTNESS first** — unchanged from v1.0
2. **CONCURRENCY safety** — data races are compile-time errors, not runtime bugs
3. **HARDWARE reality** — every OS feature must boot on QEMU; every GPU feature must run on real hardware
4. **ML production** — training must produce correct gradients; inference must be deterministic
5. **REFACTOR before extend** — the 17K-line cranelift.rs has been split into 12-file module structure (S1 ✅)

---

## 2. Sprint Cycle (1 week)

```
┌──────────────────────────────────────────────────────────┐
│                    SPRINT CYCLE (1 week)                  │
│                                                          │
│   ┌─ PLAN ──→ DESIGN ──→ TEST ──→ IMPL ──→ VERIFY ─┐   │
│   │                                                  │   │
│   │  1. Read V03_TASKS.md → find next sprint         │   │
│   │  2. Read V03_SKILLS.md → get patterns            │   │
│   │  3. Write PUBLIC INTERFACE first                  │   │
│   │  4. Write tests FIRST (RED phase)                │   │
│   │  5. Implement minimally (GREEN phase)            │   │
│   │  6. cargo test + clippy + fmt + bench            │   │
│   │  7. Update V03_TASKS.md, commit                  │   │
│   │                                                  │   │
│   └──────────────────────────────────────────────────┘   │
│                                                          │
│   End of Sprint:                                         │
│   • ALL tests pass (including previous)                  │
│   • Clippy zero warnings                                 │
│   • Formatted                                            │
│   • Tasks marked [x]                                     │
│   • At least 1 working test for each new feature         │
│                                                          │
└──────────────────────────────────────────────────────────┘
```

---

## 3. Quality Gates

### 3.1 Per-Task Gate

Before marking any task `[x]`:

```
[ ] Tests pass:     cargo test --features native
[ ] Clippy clean:   cargo clippy --features native -- -D warnings
[ ] Formatted:      cargo fmt -- --check
[ ] No .unwrap():   grep -r "\.unwrap()" src/ (only in tests)
[ ] Documented:     all new pub items have /// doc comments
[ ] JIT+AOT parity: new features work in both CraneliftCompiler and ObjectCompiler
```

### 3.2 Per-Sprint Gate

At the end of each sprint:

```
[ ] All per-task gates pass
[ ] Zero regressions (all 1,991+ existing tests still pass)
[ ] New tests added (minimum 5 per sprint)
[ ] V03_TASKS.md updated with [x] marks
[ ] Memory: no obvious leaks (run with valgrind on key tests)
```

### 3.3 Per-Quarter Gate

At the end of each quarter:

```
[ ] All sprint gates pass
[ ] At least 1 new example program (.fj)
[ ] Benchmark comparison with previous quarter
[ ] Architecture documentation updated
[ ] No accumulated tech debt (refactor before proceeding)
[ ] Feature demo: can show the quarter's capability in action
```

### 3.4 Release Gate (v0.3)

```
[ ] 4,000+ tests, zero failures
[ ] 5 real-world demos working
[ ] Documentation: tutorial + reference + guides
[ ] Binary releases for Linux, macOS, Windows
[ ] Bootstrap: self-lexer compiles and produces correct output
[ ] QEMU: mini kernel boots and runs shell
[ ] MNIST: trains in native codegen, >90% accuracy
[ ] GPU: tensor ops run on Vulkan or CUDA (or skips gracefully)
```

---

## 4. Session Protocol (Claude Code)

Every Claude Code session MUST follow this order:

```
1. READ  → CLAUDE.md (auto-loaded)
2. READ  → docs/V03_TASKS.md (find current sprint / user request)
3. READ  → docs/V03_SKILLS.md (if implementing complex feature)
4. READ  → memory/MEMORY.md (auto-loaded — check recent context)
5. ORIENT → "What does the user want?"
6. ACT   → Execute per TDD workflow (test first, implement, verify)
7. VERIFY → cargo test --features native && cargo clippy && cargo fmt --check
8. UPDATE → Mark tasks [x] in V03_TASKS.md, update MEMORY.md
```

---

## 5. Branching Strategy (v0.3)

```
main              <- stable releases only (tagged v0.X.Y)
develop           <- integration branch (PR target)
v03/q1-refactor   <- Quarter 1 work
v03/q2-os         <- Quarter 2 work
v03/q3-gpu-ml     <- Quarter 3 work
v03/q4-release    <- Quarter 4 work
feat/S01-split    <- Per-sprint feature branches
feat/S05-threads  <- Per-sprint feature branches
fix/XXX           <- Bugfix branches
```

### Commit Convention (unchanged)

```
Format: <type>(<scope>): <description>

Types: feat, fix, test, refactor, docs, perf, ci, chore
Scope: lexer, parser, analyzer, codegen, concurrency, gpu, runtime, cli, stdlib

Examples:
  refactor(codegen): split cranelift.rs into 14 focused modules
  feat(concurrency): implement Mutex with RAII MutexGuard
  feat(codegen): add inline assembly support for @kernel context
  feat(gpu): vulkan compute backend for tensor matmul
  test(concurrency): add race condition stress tests
```

---

## 6. Refactoring Rules (v0.3 specific)

### 6.1 Cranelift Split (Sprint 1)

The 17,241-line `cranelift.rs` MUST be split before ANY new v0.3 feature.

Rules:
1. **One commit per extracted module** — easy to review/revert
2. **Zero behavior change** — pure structural refactor
3. **Tests after each extraction** — `cargo test --features native` must pass
4. **Use `pub(crate)`** — extracted functions visible within codegen crate only
5. **Shared state via reference** — `CodegenCtx` passed as `&mut` to all functions
6. **No new features during refactor** — resist the temptation to "also fix X"

### 6.2 Module Size Limits

After refactoring:
- **Max 2,000 lines per module** (target: 500-1,000)
- **Max 50 lines per function** (unchanged from v1.0)
- **Max 5 levels of nesting** (if/match/loop/etc.)
- If a module exceeds 2,000 lines, split it further before adding more code

### 6.3 Cross-Module Dependencies

When extracting from cranelift.rs:
- Identify all shared state (fields of CodegenCtx, CraneliftCompiler, ObjectCompiler)
- Identify all shared helper functions (type inference, error creation)
- Put shared state in `context.rs`, helpers in appropriate module
- Use `use super::context::CodegenCtx` for type access
- Never introduce circular dependencies between extracted modules

---

## 7. Concurrency Development Rules (v0.3 specific)

### 7.1 Thread Safety

```
1. ALL shared data MUST be wrapped in Arc<Mutex<T>> or Arc<RwLock<T>>
2. ALL thread-local data MUST use thread_local! macro
3. NEVER use unsafe for thread synchronization — use atomics or locks
4. Test with at least 4 threads for any concurrent feature
5. Test with at least 1000 iterations for race condition detection
```

### 7.2 Async Safety

```
1. Async functions MUST NOT block the executor
2. Long-running sync ops MUST be moved to a blocking thread pool
3. All Futures MUST be Send (no non-Send data across await points)
4. Test with at least 100 concurrent tasks
```

### 7.3 Atomic Safety

```
1. Document memory ordering choice for every atomic operation
2. Prefer SeqCst unless performance requires weaker ordering
3. Use acquire/release pairs for producer-consumer patterns
4. Test CAS loops with contention (multiple threads CAS-ing same value)
```

---

## 8. OS Development Rules (v0.3 specific)

### 8.1 Inline Assembly

```
1. ALWAYS test on QEMU before claiming hardware support
2. ALWAYS provide architecture-specific variants (x86, ARM, RISC-V)
3. ALWAYS add // SAFETY: comment before inline asm
4. NEVER use inline asm in @safe or @device context
5. Feature-gate architecture-specific code: #[cfg(target_arch = "x86_64")]
```

### 8.2 Bare Metal

```
1. #[no_std] code MUST NOT use heap allocation unless custom allocator set
2. #[panic_handler] MUST be defined for bare-metal targets
3. Linker scripts MUST define MEMORY and SECTIONS at minimum
4. ALL MMIO access MUST use volatile_read/volatile_write
5. ALL register access MUST document bit layout in comments
```

### 8.3 Hardware Testing

```
1. QEMU first, real hardware second
2. Serial port output for debugging (no VGA dependency)
3. Timeout on all hardware tests (max 10 seconds)
4. Skip real hardware tests in CI (only run manually)
```

---

## 9. GPU Development Rules (v0.3 specific)

### 9.1 Abstraction

```
1. ALL GPU code goes through GpuDevice trait — never call Vulkan/CUDA directly
2. CPU fallback MUST exist for every GPU operation
3. GPU tests MUST skip gracefully if no GPU available
4. Memory management: explicit upload/download, no implicit copies
```

### 9.2 Performance

```
1. Batch GPU operations — minimize host-device synchronization
2. Pre-allocate GPU buffers — no per-operation allocation
3. Use async compute where possible (overlap compute + transfer)
4. Benchmark against CPU baseline — GPU must be >10x faster to justify
```

---

## 10. ML Development Rules (v0.3 specific)

### 10.1 Numerical Correctness

```
1. Gradient checks: compare autograd vs numerical gradient (epsilon=1e-4)
2. Use relative error, not absolute: |computed - expected| / max(|expected|, 1e-8)
3. MNIST accuracy >90% is the minimum bar
4. Loss must monotonically decrease (on average) during training
```

### 10.2 Tensor Ops

```
1. Every tensor op in native codegen MUST match interpreter output (bitwise)
2. Shape checking at compile time where possible, runtime where not
3. Document broadcasting rules for each operation
4. No in-place mutation of tensors used in autograd graph
```

---

## 11. Testing Strategy (v0.3)

### 11.1 Test Categories

| Category | Location | Purpose |
|----------|----------|---------|
| Unit | `#[cfg(test)]` in each module | Per-function correctness |
| Native | `#[cfg(test)]` with `--features native` | Cranelift codegen |
| Integration | `tests/*.rs` | Full pipeline end-to-end |
| Concurrency | `tests/concurrency_tests.rs` | Thread safety, races |
| GPU | `tests/gpu_tests.rs` | GPU compute correctness |
| Bare Metal | `tests/bare_metal_tests.rs` | QEMU boot tests |
| Self-Hosting | `tests/self_host_tests.rs` | Bootstrap verification |
| Benchmark | `benches/*.rs` | Performance regression |

### 11.2 Test Naming Convention

```rust
// Pattern: <domain>_<what>_<expected>
fn thread_spawn_join_returns_value() { ... }
fn mutex_lock_unlock_preserves_data() { ... }
fn gpu_matmul_matches_cpu() { ... }
fn asm_nop_compiles_and_runs() { ... }
fn native_mnist_accuracy_above_90() { ... }
```

### 11.3 Stress Tests

For concurrency features, ALWAYS include:
```rust
#[test]
fn stress_concurrent_counter_1000_threads() {
    // Must produce deterministic result
    // Run 1000 threads, each incrementing counter 1000 times
    // Final value must be 1_000_000
}
```

---

## 12. Documentation Requirements

### Per Feature:
- [ ] Doc comment on all pub items
- [ ] At least 1 code example in doc comment
- [ ] Error codes documented in ERROR_CODES.md

### Per Sprint:
- [ ] Sprint summary in commit message
- [ ] V03_TASKS.md updated

### Per Quarter:
- [ ] mdBook chapter for major feature area
- [ ] Example programs demonstrating capabilities

### Release:
- [ ] Complete tutorial (10 chapters)
- [ ] Standard library reference
- [ ] Migration guide from v0.2

---

*V03_WORKFLOW.md v1.0 — Development workflow for v0.3 "Dominion" | Created 2026-03-08*
