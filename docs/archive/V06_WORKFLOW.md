# Workflow — Fajar Lang v0.6 "Horizon"

> Target: Production infrastructure — LLVM, debugger, BSP, registry, lifetimes, RTOS, advanced ML
> Timeline: 28 sprints, ~280 tasks, 4-6 months
> Baseline: v0.5 "Ascendancy" complete (1,767 tests, ~101K LOC)
> Created: 2026-03-11

---

## 1. Development Philosophy

```
CORRECTNESS > SAFETY > USABILITY > PERFORMANCE

"If it compiles in Fajar Lang, it's safe to deploy on real hardware
AND debug live systems AND train production models."
```

### Core Principles (v0.6 additions)

1. **CORRECTNESS first** — unchanged from v1.0
2. **DUAL BACKEND parity** — LLVM and Cranelift must produce identical results for the same program
3. **DEBUGGABLE always** — every feature must be step-debuggable in VS Code
4. **HARDWARE reality** — BSP features must work on real boards (STM32, ESP32, RP2040), not just QEMU
5. **ECOSYSTEM security** — registry packages must be authenticated, checksummed, and immutable
6. **REAL-TIME safety** — RTOS annotations enable compile-time enforcement of timing constraints
7. **ML correctness** — LSTM/GRU backward passes verified against numerical gradients

---

## 2. Sprint Cycle (1-2 weeks)

```
┌──────────────────────────────────────────────────────────────┐
│                    SPRINT CYCLE (1-2 weeks)                    │
│                                                                │
│   ┌─ PLAN ──→ DESIGN ──→ TEST ──→ IMPL ──→ VERIFY ──→ INT ─┐ │
│   │                                                          │ │
│   │  1. Read V06_PLAN.md → find current sprint               │ │
│   │  2. Read V06_SKILLS.md → get implementation patterns     │ │
│   │  3. Design public interface (trait, struct, fn sigs)      │ │
│   │  4. Write tests FIRST (RED phase)                        │ │
│   │  5. Implement minimally (GREEN phase)                    │ │
│   │  6. cargo test + clippy + fmt                            │ │
│   │  7. Integration test with adjacent sprints               │ │
│   │  8. Update V06_PLAN.md, mark tasks [x], commit           │ │
│   │                                                          │ │
│   └──────────────────────────────────────────────────────────┘ │
│                                                                │
│   End of Sprint:                                               │
│   • ALL tests pass (including all previous: 1,767+ baseline)  │
│   • Clippy zero warnings                                       │
│   • Formatted (cargo fmt)                                      │
│   • Tasks marked [x] in V06_PLAN.md                            │
│   • At least 10 new tests per sprint                           │
│   • Feature-gated code compiles with AND without feature flag  │
│                                                                │
└──────────────────────────────────────────────────────────────┘
```

---

## 3. Quality Gates

### 3.1 Per-Task Gate

Before marking any task `[x]`:

```
[ ] Tests pass:     cargo test
[ ] Native tests:   cargo test --features native (if touching codegen)
[ ] LLVM tests:     cargo test --features llvm (if touching LLVM backend)
[ ] Clippy clean:   cargo clippy -- -D warnings
[ ] Formatted:      cargo fmt -- --check
[ ] No .unwrap():   grep -r "\.unwrap()" src/ (only in tests)
[ ] Documented:     all new pub items have /// doc comments
[ ] Feature gate:   new dependencies behind appropriate feature flag
```

### 3.2 Per-Sprint Gate

```
[ ] All per-task gates pass
[ ] Zero regressions (1,767+ existing tests still pass)
[ ] New tests added (minimum 10 per sprint)
[ ] V06_PLAN.md updated with [x] marks
[ ] No feature gate leaks (code with #[cfg(feature = "X")] compiles without X too)
```

### 3.3 Per-Phase Gate

```
[ ] All sprint gates in the phase pass
[ ] Integration test: feature works end-to-end (e.g., LLVM compiles and runs fibonacci)
[ ] At least 1 new example program (.fj) demonstrating the phase feature
[ ] Documentation: mdBook chapter or section for the feature
[ ] Benchmark: performance comparison where applicable
```

### 3.4 Release Gate (v0.6)

```
[ ] All 28 sprints complete
[ ] All existing tests pass (zero regression)
[ ] 4,000+ tests total (280+ new)
[ ] LLVM backend: fibonacci(30) ≥ 10% faster than Cranelift
[ ] Debugger: VS Code step-through demo works
[ ] BSP: at least 1 board produces flashable firmware
[ ] Registry: publish + install cycle works
[ ] Lifetime annotations: `fn longest<'a>` compiles
[ ] RTOS: task_spawn creates FreeRTOS task (QEMU test)
[ ] LSTM: sequence classification with backward pass
[ ] CHANGELOG.md updated
[ ] CLAUDE.md updated
[ ] README.md updated with new features
```

---

## 4. Session Protocol (Claude Code)

Every Claude Code session MUST follow this order:

```
1. READ  → CLAUDE.md (auto-loaded)
2. READ  → docs/V06_PLAN.md (find current sprint / user request)
3. READ  → docs/V06_SKILLS.md (if implementing complex feature)
4. READ  → memory/MEMORY.md (auto-loaded — check recent context)
5. ORIENT → "What does the user want?"
6. ACT   → Execute per TDD workflow (test first, implement, verify)
7. VERIFY → cargo test && cargo clippy -- -D warnings && cargo fmt --check
8. UPDATE → Mark tasks [x] in V06_PLAN.md, update MEMORY.md if significant
```

---

## 5. Parallel Development Tracks

v0.6 has 5 independent tracks that can be developed in parallel:

```
Track A (Critical Path): Phase 1 (LLVM) → Phase 3 (BSP) → Phase 6 (RTOS)
Track B (Independent):   Phase 2 (Debugger)
Track C (Independent):   Phase 4 (Registry)
Track D (Independent):   Phase 5 (Lifetimes)
Track E (Independent):   Phase 7 (Advanced ML)
```

### Recommended Development Order

```
Phase 1 (LLVM Backend)        ← Start here. Critical path blocker.
  ↓
Phase 2 (Debugger)            ← Can start in parallel after S1-S2
Phase 4 (Registry)            ← Can start in parallel (fully independent)
Phase 5 (Lifetimes)           ← Can start in parallel (analyzer only)
Phase 7 (Advanced ML)         ← Can start in parallel (runtime only)
  ↓
Phase 3 (BSP)                 ← Needs LLVM for Thumb/Xtensa targets
  ↓
Phase 6 (RTOS)                ← Needs BSP for hardware targets
```

---

## 6. Branching Strategy (v0.6)

```
main                  ← stable releases only (tagged v0.X.Y)
develop               ← integration branch (PR target)
v06/phase1-llvm       ← Phase 1 work (LLVM backend)
v06/phase2-debugger   ← Phase 2 work (Debugger)
v06/phase3-bsp        ← Phase 3 work (BSP)
v06/phase4-registry   ← Phase 4 work (Registry)
v06/phase5-lifetimes  ← Phase 5 work (Lifetimes)
v06/phase6-rtos       ← Phase 6 work (RTOS)
v06/phase7-ml         ← Phase 7 work (Advanced ML)
feat/S01-llvm-infra   ← Per-sprint feature branches
fix/XXX               ← Bugfix branches
```

### Commit Convention (unchanged)

```
Format: <type>(<scope>): <description>

Types: feat, fix, test, refactor, docs, perf, ci, chore
Scope: llvm, debugger, dap, bsp, registry, lifetimes, rtos, ml, cli

Examples:
  feat(llvm): add inkwell context setup and type mapping
  feat(debugger): implement DAP server with breakpoint support
  feat(bsp): add STM32F407 memory map and linker script
  feat(registry): implement PubGrub dependency resolver
  feat(lifetimes): add lifetime elision rules to analyzer
  feat(rtos): add FreeRTOS task spawn FFI wrapper
  feat(ml): implement LSTM cell with BPTT backward
```

---

## 7. Feature Gate Rules (v0.6 specific)

### 7.1 Required Feature Gates

| Phase | Feature Flag | Dependencies |
|-------|-------------|--------------|
| Phase 1 | `llvm` | inkwell |
| Phase 2 | (default) | dap, gimli |
| Phase 3 | `bsp` | (implies `llvm`) |
| Phase 4 | `registry` | axum, sqlx, sha2 (separate binary) |
| Phase 5 | (default) | (analyzer only, no new deps) |
| Phase 6 | `rtos` | (FFI declarations only) |
| Phase 7 | (default) | half |

### 7.2 Feature Gate Rules

```
1. ALL new crate dependencies MUST be feature-gated (except analyzer-only changes)
2. Code MUST compile without any optional features: `cargo check`
3. Code MUST compile with each feature individually: `cargo check --features llvm`
4. CI must test: default, native, llvm, native+llvm
5. Feature-gated modules use: #[cfg(feature = "llvm")] mod llvm;
6. Feature-gated tests use: #[cfg(feature = "llvm")] #[test]
```

---

## 8. LLVM Development Rules (v0.6 specific)

### 8.1 Backend Parity

```
1. Every test case that passes on Cranelift MUST also pass on LLVM (where applicable)
2. Maintain a parity test list: tests that run on both backends
3. Runtime functions (fj_rt_*) are shared between backends — do NOT duplicate
4. Use CodegenBackend trait for backend-agnostic code in main.rs
```

### 8.2 LLVM IR Debugging

```
1. Always print IR during development: module.print_to_string()
2. Verify IR with: module.verify() — catch errors before JIT
3. For optimization debugging: print IR before and after pass manager
4. Use LLVM_SYS_180_PREFIX env var if LLVM not in default path
```

---

## 9. Debugger Development Rules (v0.6 specific)

### 9.1 DAP Protocol Compliance

```
1. Test against VS Code's built-in DAP client (the standard reference)
2. Always return correct sequence numbers in responses
3. Events (stopped, terminated) must be sent asynchronously
4. Handle disconnect gracefully — no orphan interpreter threads
```

### 9.2 Debug Hook Performance

```
1. Debug hook in eval_stmt MUST be zero-cost when debugger not attached
2. Use Option<Arc<Mutex<DebugState>>> — None when no debugger
3. Avoid allocations in the hot path (should_stop check)
4. Conditional breakpoint evaluation reuses existing eval_source
```

---

## 10. BSP Development Rules (v0.6 specific)

### 10.1 Hardware Abstraction

```
1. ALL hardware access through HAL traits — never direct register writes in user code
2. Board-specific code isolated in src/bsp/<board>.rs
3. Linker scripts generated, not hand-written (except for customization)
4. Startup code generated from board description
```

### 10.2 Testing Strategy

```
1. QEMU first for Cortex-M: `qemu-system-arm -machine lm3s6965evb`
2. Real hardware second: STM32F407 Discovery via ST-Link
3. Semihosting for CI tests: print results to host console
4. Binary size check: firmware must fit in Flash (1MB for STM32F407)
```

---

## 11. Registry Development Rules (v0.6 specific)

### 11.1 Security

```
1. ALL packages signed with API token
2. Checksums (SHA256) verified on download
3. Immutable versions: same version + different content = reject
4. Rate limiting on publish endpoint
5. Name squatting prevention: fj-math = fj_math (equivalent)
```

### 11.2 Compatibility

```
1. Binary publish format compatible with Cargo conventions
2. Sparse index for fast resolution
3. Lock files for reproducible builds
4. Semver-compliant version ranges
```

---

## 12. ML Development Rules (v0.6 specific)

### 12.1 Numerical Correctness

```
1. LSTM/GRU backward verified against numerical gradients (epsilon=1e-4)
2. AdamW weight decay tested: compare with known PyTorch results
3. LR schedulers tested: compare schedule curve with expected formula
4. Mixed precision: FP16 forward + FP32 accumulation preserves accuracy
```

### 12.2 Training Pipeline

```
1. DataLoader must not deadlock (bounded channel, proper shutdown)
2. Early stopping must save best model before stopping
3. Checkpoint format must be forward-compatible (version field)
4. Dropout must be disabled during eval (train/eval mode)
```

---

## 13. Testing Strategy (v0.6)

### 13.1 Test Categories

| Category | Location | Purpose |
|----------|----------|---------|
| Unit | `#[cfg(test)]` in each module | Per-function correctness |
| Native | `#[cfg(test)]` with `--features native` | Cranelift codegen |
| LLVM | `#[cfg(test)]` with `--features llvm` | LLVM codegen |
| Integration | `tests/*.rs` | Full pipeline end-to-end |
| DAP | `tests/dap_tests.rs` | Debugger protocol tests |
| BSP | `tests/bsp_tests.rs` | Board support tests |
| Registry | `packages/fj-registry/tests/` | Registry server tests |
| ML | `tests/ml_tests.rs` + extensions | RNN, optimizer, scheduler tests |

### 13.2 Test Naming Convention

```rust
// Pattern: <domain>_<what>_<expected>
fn llvm_fibonacci_matches_cranelift() { ... }
fn dap_breakpoint_stops_at_correct_line() { ... }
fn bsp_stm32f407_linker_script_has_flash() { ... }
fn registry_publish_creates_version() { ... }
fn lifetime_elision_single_input_ref() { ... }
fn rtos_task_spawn_returns_handle() { ... }
fn lstm_backward_gradient_matches_numerical() { ... }
```

---

## 14. Documentation Requirements

### Per Sprint:
- [ ] Doc comments on all new pub items
- [ ] At least 1 code example in doc comment
- [ ] V06_PLAN.md updated with [x] marks

### Per Phase:
- [ ] mdBook chapter for the feature area
- [ ] At least 1 example .fj program
- [ ] Error codes documented (if new)

### Release:
- [ ] CHANGELOG.md v0.6.0 entry
- [ ] README.md updated feature matrix
- [ ] CLAUDE.md updated status
- [ ] All examples run successfully

---

*V06_WORKFLOW.md v1.0 — Development workflow for v0.6 "Horizon" | Created 2026-03-11*
