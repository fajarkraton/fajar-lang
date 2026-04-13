# Fajar Lang V27.5 "Compiler Prep" — Comprehensive Plan

## Context

Deep re-audit for V28-V33 readiness found that **6 of 10 reported gaps are ALREADY IMPLEMENTED** (Result/?, module resolver, borrow checker, effects system, incremental compilation, async codegen). This is a major positive finding — the compiler is more ready than initially assessed.

**4 real gaps remain + 7 enhancements needed.** This plan addresses ALL of them before V28 starts. No going back and forth — everything fixed upfront.

**Total: 196h base, 245h with +25% surprise budget (~6 weeks)**

---

## Audit Corrections (What's Already Done)

| Initially Reported Gap | Actual Status | Evidence |
|----------------------|---------------|----------|
| Result<T,E> + ? operator | ✅ COMPLETE | builtins.rs:9233, 5 tests |
| Module file resolver | ✅ COMPLETE | builtins.rs:9526, handles `mod foo;` |
| Borrow checker | ✅ COMPLETE (lite) | borrow_lite.rs, 1,253 LOC |
| Effects system | ✅ COMPLETE | effects.rs, 2,306 LOC, EE001-EE008 |
| Incremental compilation | ✅ COMPLETE | incremental/, 9,377 LOC |
| Async codegen | ✅ COMPLETE | Intentional eager model, 51 LOC |

---

## Phase 0: Pre-Flight Audit (4h → 5h)

12 runnable verification checks. Gate: `docs/V27_5_A0_FINDINGS.md` committed.

| # | Check | Command | Expected |
|---|-------|---------|----------|
| A0.1 | Lib tests | `cargo test --lib 2>&1 \| tail -1` | 7,611+ pass |
| A0.2 | Integ tests | `cargo test --test '*' 2>&1 \| tail -1` | 2,553+ pass |
| A0.3 | Version | `grep '^version' Cargo.toml` | "27.0.0" |
| A0.4 | Interrupt LOC | `grep -rn 'interrupt' src/codegen/ \| wc -l` | baseline |
| A0.5 | @host/@app | `grep -rn 'AtHost\|AtApp' src/lexer/token.rs \| wc -l` | 0 |
| A0.6 | Refinement checks | `grep -rn 'refinement' src/interpreter/ \| wc -l` | ~2 |
| A0.7 | Tensor max | `grep 'MAX_KERNEL_TENSOR_DIM' src/runtime/os/ai_kernel.rs` | 16 |
| A0.8 | Selfhost LOC | `wc -l src/selfhost/*.rs` | ~15,880 |
| A0.9 | Wrapper calls | `grep -rn 'generate_interrupt_wrapper' src/ \| grep -v test` | 0 non-test |
| A0.10 | FB builtins | `grep -rn 'fb_init' src/interpreter/eval/builtins.rs` | exists |
| A0.11 | Cap<T> | `grep -rn 'CapType' src/analyzer/ \| wc -l` | 0 |
| A0.12 | Full CI | `cargo test --lib && cargo clippy -- -D warnings && cargo fmt -- --check` | green |

---

## Phase 1: V28 Blockers (56h → 70h)

### P1.1: Kernel Tensor Size 16→128 (4h)
- **File:** `src/runtime/os/ai_kernel.rs:84`
- **Change:** `MAX_KERNEL_TENSOR_DIM = 16` → `128`
- **Verify:** `cargo test --lib -- ai_kernel`

### P1.2: AI Scheduler Builtins (16h)
- **Files:** `src/runtime/os/ai_kernel.rs`, `src/interpreter/eval/builtins.rs`, `src/analyzer/type_check/register.rs`
- **Add:** `tensor_workload_hint(rows, cols)` → FLOP estimate, `schedule_ai_task(id, priority, deadline)` → slot
- **Verify:** `cargo test --lib -- tensor_workload_hint schedule_ai_task`

### P1.3: @interrupt Handler Integration (24h)
**P1.3a** Wire `generate_interrupt_wrapper()` into AOT pipeline (8h)
- **File:** `src/codegen/cranelift/mod.rs:6632+` — add accessor + call after compile
- **Verify:** `cargo test --lib --features native -- aot_interrupt`

**P1.3b** x86_64 interrupt wrapper (8h)
- **File:** `src/codegen/linker.rs:3894+` — mirror ARM64 with push/iretq
- **Verify:** `cargo test --lib -- interrupt_wrapper_x86`

**P1.3c** Ensure wrappers reach linker output (8h)
- Emit generated assembly to object file
- **Verify:** `cargo test --lib --features native -- interrupt_wrapper_emitted`

### P1.4: VESA Framebuffer MMIO Builtins (12h)
- **Files:** `src/interpreter/eval/builtins.rs`, `src/codegen/cranelift/runtime_bare.rs`
- **Add:** `fb_set_base(addr)`, `fb_fill_rect(x,y,w,h,color)`, `fb_scroll(lines)`
- **Verify:** `cargo test --lib --features native -- fb_fill_rect fb_scroll`

### Phase 1 Gate
```
[ ] MAX_KERNEL_TENSOR_DIM == 128
[ ] AI scheduler builtins callable from .fj
[ ] @interrupt generates wrappers (ARM64 + x86_64)
[ ] Wrappers emitted to AOT output
[ ] Framebuffer builtins registered
```

---

## Phase 2: V29 Prep — IPC Stubs (24h → 30h)

### P2.1: IPC Proxy/Stub Auto-Generation (24h)
- **New file:** `src/codegen/ipc_stub.rs` (~300 LOC)
- **Generates from `service {} + @message struct`:**
  - Server dispatch loop (ipc_recv → handler routing)
  - Client proxy functions (serialize → ipc_call → deserialize)
  - Message ID constants
- **Verify:** `cargo test --lib -- ipc_stub`

### Phase 2 Gate
```
[ ] service {} generates dispatch loop
[ ] Client proxies auto-generated
[ ] IPC001/IPC002 still enforced
```

---

## Phase 3: V30-V31 Prep — @app + @host (20h → 25h)

### P3.1: @app Annotation (8h)
- **Files:** `src/lexer/token.rs` (4 points), `src/parser/mod.rs` (2 points), `src/analyzer/type_check/check.rs`
- **Semantics:** GUI entry point, must return i64, implies @safe, at most one per program
- **Verify:** `cargo test --lib -- at_app app_annotation`

### P3.2: @host Annotation (12h)
- **Files:** Same token/parser pattern + `src/selfhost/bootstrap_v2.rs`, `src/main.rs:1822`
- **Semantics:** Stage 1 compiler context, enables file I/O (read_file, write_file)
- **Wire:** `fj bootstrap` command does real Stage 0→1 comparison
- **Verify:** `cargo test --lib -- at_host bootstrap`

### Phase 3 Gate
```
[ ] @app and @host lexed + parsed
[ ] @app enforces i64 return + single entry
[ ] @host enables file I/O in Stage 1
[ ] fj bootstrap runs real comparison
```

---

## Phase 4: V33 Prep — Dependent Types + Cap<T> (72h → 90h)

### P4.1: Runtime Refinement Predicates (32h)
- **Files:** `src/dependent/nat.rs`, `src/interpreter/eval/mod.rs`, `src/codegen/cranelift/mod.rs`
- **Add checks at:** function params, return values, mutable assignments (currently only let-bind)
- **Cranelift:** Emit `trapnz` for refinement violations
- **15 tests** covering all predicate variants × all check sites
- **Verify:** `cargo test --lib -- refinement_param refinement_return refinement_assign`

### P4.2: Capability Type `Cap<T>` (40h)
- **Files:** `src/analyzer/type_check/mod.rs` (new Type variant), `src/interpreter/value.rs`, `src/codegen/cranelift/`
- **Design:** Linear (affine) type — used exactly once, cannot copy, blocked across @device→@kernel
- **Builtins:** `cap_new(val)`, `cap_unwrap(cap)`, `cap_is_valid(cap)`
- **Zero overhead:** Cap<i64> same representation as i64 in codegen
- **12 tests** covering lifecycle, move semantics, cross-context blocking
- **Verify:** `cargo test --lib -- cap_type cap_new cap_unwrap`

### Phase 4 Gate
```
[ ] Refinements checked at let + param + return + assign
[ ] Cranelift emits trap instructions
[ ] Cap<T> type in analyzer
[ ] Cap<T> linear semantics enforced
[ ] Cap<T> zero-overhead in codegen
```

---

## Phase 5: Quality + Prevention (20h → 25h)

### P5.1: Bare-Metal Test Coverage (16h)
- `tests/compilation_stress.rs` — 50K LOC synthetic, <30s
- `tests/interrupt_codegen.rs` — verify wrapper symbols
- `tests/framebuffer_mmio.rs` — fb builtins compile
- `tests/refinement_e2e.rs` — predicate violations produce clear errors
- `tests/capability_tests.rs` — Cap<T> lifecycle

### P5.2: CI Gates (4h)
- `bare-metal-codegen` job in `.github/workflows/ci.yml`
- `stress-test` job (timeout 10min)
- Version sync check

### Phase 5 Gate
```
[ ] 5 new test files
[ ] CI has bare-metal + stress jobs
[ ] All tests pass
```

---

## Effort Summary

| Phase | Base | Budget (+25%) |
|-------|------|---------------|
| P0 Pre-flight | 4h | **5h** |
| P1 V28 blockers | 56h | **70h** |
| P2 V29 IPC stubs | 24h | **30h** |
| P3 V30-V31 @app/@host | 20h | **25h** |
| P4 V33 dep types + Cap<T> | 72h | **90h** |
| P5 Quality + CI | 20h | **25h** |
| **Total** | **196h** | **245h** |

## Dependency Graph
```
P0 → all phases
P1.1, P1.2, P1.4 → independent (parallel)
P1.3a + P1.3b → P1.3c (sequential)
P2, P3 → independent of P1 (parallel after P0)
P4.1 → P4.2 (sequential, shares type system)
P5 → last (tests all prior work)
```

## Plan Hygiene (§6.8)
```
[x] Pre-flight audit (P0)                    (Rule 1)
[x] Runnable verification per task            (Rule 2)
[x] Prevention per phase (CI gates, tests)    (Rule 3)
[x] Numbers from actual LOC analysis          (Rule 4)
[x] +25% surprise budget                     (Rule 5)
[x] Gates are committed files                 (Rule 6)
[x] Public artifact sync (version badges)     (Rule 7)
[x] Multi-repo check in P0                    (Rule 8)
```
