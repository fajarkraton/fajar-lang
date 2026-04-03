# Full Remediation Plan — Fajar Lang Codebase Health

> **Date:** 2026-04-03
> **Author:** Fajar (TaxPrime / PrimeCore.id)
> **Trigger:** Post-V20 deep audit revealed structural issues that MUST be fixed before
>            building advanced features (FajarQuant, V21 Hardware).
> **Rule:** Every fix verified with `cargo test`. No regressions.
> **Rule:** [x] only when `fj run` produces correct output.
> **Lesson:** V13-V15 built features on untested foundations → 40-55% inflation.
>           This plan ensures we NEVER repeat that.

---

## Table of Contents

1. [Audit Findings Summary](#1-audit-findings-summary)
2. [Tier 1: Critical Stability (V20.5)](#2-tier-1-critical-stability-v205)
3. [Tier 2: Error Quality (V20.6)](#3-tier-2-error-quality-v206)
4. [Tier 3: Code Cleanup (V20.7)](#4-tier-3-code-cleanup-v207)
5. [Tier 4: FajarQuant Prerequisites](#5-tier-4-fajarquant-prerequisites)
6. [Execution Schedule](#6-execution-schedule)
7. [Gate Criteria](#7-gate-criteria)

---

## 1. Audit Findings Summary

### 1.1 What's SOLID (Don't Touch)

| Component | Evidence |
|-----------|---------|
| Lexer, Parser, Analyzer | 8,287 lib tests pass, 0 failures |
| @kernel/@device enforcement | Transitive taint tracking verified (V18 fix) |
| INT8 quantization | 10 dedicated tests, mathematically sound |
| Core tensor ops | matmul, relu, sigmoid, softmax, randn, zeros — all working |
| User macros ($x metavars) | V19 verified E2E |
| Pattern match (Ok/Err/Some/None) | V19 verified E2E |
| Real async (tokio) | async_sleep, async_spawn, async_join verified |
| Integer overflow/div-by-zero | All arithmetic uses checked operations (RE001, RE009) |
| Recursion depth limit | 64 debug / 1024 release, properly caught (RE003) |
| Language spec compliance | trait, enum, type, use/mod, range, loop, tuple, as — all working |
| eval_tests | 948 tests, 944 pass (4 crash — addressed below) |
| safety_tests | 96 tests, 92 pass (4 crash — addressed below) |

### 1.2 What's BROKEN (This Plan Fixes)

| # | Category | Severity | Count | Detail |
|---|----------|----------|-------|--------|
| A | **4 test crashes** (stack overflow) | CRITICAL | 4 tests | Interpreter overflows Rust stack before catching |
| B | **9 HashMap workaround builtins** | HIGH | 9 builtins | Return Map instead of typed values |
| C | **0 tests for V20 builtins** | HIGH | 6 builtins | Zero coverage = zero confidence |
| D | **pipeline_run swallows errors** | HIGH | 1 fn | Stage failure logged, not propagated |
| E | **Fake GPU dispatch** | HIGH | 1 builtin | `accelerate()` always runs on CPU |
| F | **Synchronous actors** | HIGH | 2 builtins | actor_spawn/send = synchronous call_fn() |
| G | **RuntimeError has no source spans** | MEDIUM | entire system | Errors say WHAT but not WHERE |
| H | **Environment parent = Rc (not Weak)** | MEDIUM | 1 field | Memory leak risk in long REPL sessions |
| I | **43,566 lines dead code** | LOW | 17 modules | 9.1% of codebase never called |
| J | **5 unwraps in critical path** | LOW | 5 calls | parser/mod.rs (4) + eval/mod.rs (1) |

### 1.3 Precise Measurements

```
Files:
  src/interpreter/eval/mod.rs     9,612 lines  (1 unwrap, 0 panic in production)
  src/interpreter/eval/builtins.rs 9,538 lines  (0 unwrap, 0 panic in production)
  src/parser/mod.rs               ???   lines  (4 unwrap, 0 panic in production)
  src/interpreter/env.rs                       (parent: Option<Rc<RefCell<Environment>>>)
  RuntimeError enum                            (NO span field)

Crashing tests:
  eval_tests::t3_5_deep_nesting_if              50-level nested if → SIGABRT
  eval_tests::t3_7_recursive_fib_deep           fib(25) naive → SIGABRT
  safety_tests::safety_stack_overflow_infinite   inf(n) { inf(n) } → SIGABRT
  safety_tests::safety_stack_overflow_mutual     a→b→a→b→... → SIGABRT

Dead modules (17 total, 43,566 lines):
  src/demos/         16,257 lines  (demo .rs files, never called)
  src/rtos/           8,043 lines  (FreeRTOS/Zephyr framework)
  src/iot/            5,033 lines  (WiFi/BLE/MQTT framework)
  src/lsp_v2/         3,395 lines  (superseded by src/lsp/)
  src/rt_pipeline/    2,554 lines  (wired via builtin, but module itself has more)
  src/package_v2/     2,221 lines  (build_scripts partially wired)
  8x const_* modules  4,531 lines  (const_bench, const_generics, etc.)
  generators_v12       372 lines  (superseded by V18 generators)
  wasi_v12             395 lines  (superseded by wasi_p2)
  stdlib/               95 lines  (empty re-exports)
  runtime/ml/data.rs    236 lines  (superseded by dataloader.rs)
```

---

## 2. Tier 1: Critical Stability (V20.5) — 38 tasks

> **Priority:** MUST DO before any new feature.
> **Goal:** All tests pass, no crashes, no silent errors, every V20 builtin tested.

### Phase 1.1: Fix 4 Crashing Tests (6 tasks)

The root cause: interpreter's recursive `eval_expr` consumes Rust stack frames
without limit for AST nesting. `call_depth` only tracks function calls, not
AST expression nesting. 50 nested `if` = 50 recursive `eval_expr` calls =
Rust stack overflow before the interpreter's own depth check fires.

**Fix strategy:** Run deep-recursion tests in threads with larger stacks
(same pattern as `h3_2_fibonacci_20_under_200ms` fix).

| # | Task | Verification |
|---|------|-------------|
| 1.1.1 | Wrap `t3_5_deep_nesting_if` in 16MB thread | `cargo test --test eval_tests t3_5` passes |
| 1.1.2 | Wrap `t3_7_recursive_fib_deep` in 16MB thread | `cargo test --test eval_tests t3_7_recursive_fib_deep` passes |
| 1.1.3 | Wrap `safety_stack_overflow_infinite_recursion` in 16MB thread | `cargo test --test safety_tests infinite` passes |
| 1.1.4 | Wrap `safety_stack_overflow_mutual_recursion` in 16MB thread | `cargo test --test safety_tests mutual` passes |
| 1.1.5 | Verify: `cargo test --test eval_tests` — ALL 948 pass | 0 crashes |
| 1.1.6 | Verify: `cargo test --test safety_tests` — ALL 96 pass | 0 crashes |

**Implementation pattern (proven in benchmark_validation fix):**
```rust
#[test]
fn t3_5_deep_nesting_if() {
    let result = std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(|| {
            // ... original test body ...
        })
        .expect("thread spawn")
        .join();
    result.expect("test panicked");
}
```

### Phase 1.2: Fix pipeline_run Error Propagation (2 tasks)

| # | Task | Verification |
|---|------|-------------|
| 1.2.1 | `pipeline_run`: stage failure returns `Err` instead of logging and continuing | Failing stage stops pipeline |
| 1.2.2 | Integration test: pipeline with bad function name returns error | `cargo test pipeline_error` |

**Before (broken):**
```rust
Err(e) => {
    eprintln!("{msg}");  // log and CONTINUE — WRONG
}
```

**After (correct):**
```rust
Err(e) => {
    return Err(RuntimeError::TypeError(
        format!("pipeline stage '{stage_name}' failed: {e}")
    ).into());
}
```

### Phase 1.3: Honest Labels for Simulated Builtins (4 tasks)

Don't remove fake builtins — just label them honestly.

| # | Task | Verification |
|---|------|-------------|
| 1.3.1 | `accelerate()` — add `[sim]` prefix to output: `{"device":"[sim] GPU", ...}` | Output clearly shows simulation |
| 1.3.2 | `actor_spawn/send` — add `[sim]` to status: `{"status":"[sim] Starting", ...}` | Output clearly shows simulation |
| 1.3.3 | Update `fj demo --list` descriptions for simulated demos | Descriptions say "simulated" where appropriate |
| 1.3.4 | Print one-time warning on first use of simulated builtin | `[warn] accelerate() is simulated (CPU-only). GPU dispatch requires --features cuda` |

### Phase 1.4: Test Coverage for V20 Builtins (14 tasks)

New file: `tests/v20_builtin_tests.rs`

| # | Builtin | Test 1 (happy path) | Test 2 (edge case) |
|---|---------|---------------------|-------------------|
| 1.4.1 | `diffusion_create(steps)` | steps=100 → map with "DiffusionModel" type | steps=1 works (minimum) |
| 1.4.2 | `diffusion_denoise(m, t, step)` | Shape preserved after denoise | Step 0 vs step 99 produce different magnitudes |
| 1.4.3 | `rl_agent_create(sd, ad)` | state_dim=4, action_dim=2 → correct map | state array length = state_dim |
| 1.4.4 | `rl_agent_step(agent, action)` | Returns map with reward + done | Different actions → different rewards |
| 1.4.5 | `pipeline_create` + `add_stage` | 0 stages → add 3 → count=3 | Stage names preserved |
| 1.4.6 | `pipeline_run` (happy) | 3-function chain produces correct result | Identity pipeline: input = output |
| 1.4.7 | `pipeline_run` (error) | Bad function name → error returned | Error message contains stage name |
| 1.4.8 | `accelerate(fn, input)` | Returns map with device + class | Tensor input works |
| 1.4.9 | `actor_spawn(name, fn)` | Returns map with name + status | Unique names produce unique addrs |
| 1.4.10 | `actor_send(actor, msg)` | Calls handler, returns result | Handler with string arg works |
| 1.4.11 | `actor_supervise(actor, strategy)` | Adds supervision field to map | one_for_one and all_for_one both work |
| 1.4.12 | `const_alloc(size)` | size=4096 → correct map | size=0 works (edge case) |
| 1.4.13 | `const_size_of / const_align_of` | i64=8, bool=1, char=4, str varies | Tensor size scales with elements |
| 1.4.14 | `map_get_or(map, key, default)` | Key exists → return value | Key missing → return default |

### Phase 1.5: Fix 5 Unwraps in Critical Path (4 tasks)

| # | Task | File | Verification |
|---|------|------|-------------|
| 1.5.1 | Fix 4 unwraps in parser/mod.rs → proper error handling | parser/mod.rs | `cargo test --lib parser` passes |
| 1.5.2 | Fix 1 unwrap in interpreter/eval/mod.rs → proper error handling | eval/mod.rs | `cargo test --lib interpreter` passes |
| 1.5.3 | Add `#[deny(clippy::unwrap_used)]` to parser/mod.rs | parser/mod.rs | Clippy enforces no new unwraps |
| 1.5.4 | Add `#[deny(clippy::unwrap_used)]` to interpreter/eval/mod.rs | eval/mod.rs | Clippy enforces no new unwraps |

### Phase 1.6: Documentation Honesty (8 tasks)

| # | Task | Verification |
|---|------|-------------|
| 1.6.1 | Create `docs/HONEST_STATUS_V20_5.md` — per-builtin status table | Every builtin labeled [x], [sim], or [f] |
| 1.6.2 | Update CLAUDE.md module counts: 42 [x], 6 [sim], 5 [f], 3 [s] | Matches reality |
| 1.6.3 | Update CLAUDE.md test counts after all fixes | Accurate numbers |
| 1.6.4 | Update MEMORY.md with V20.5 status | Correct for next session |
| 1.6.5 | Mark V20 builtins honestly in CLAUDE.md §11 (stdlib overview) | [sim] tag on simulated |
| 1.6.6 | Add new labeling system to CLAUDE.md §6.6 | [x], [sim], [f], [s] defined |
| 1.6.7 | Update `docs/V19_V21_COMPLETE_56_PLAN.md` — correct module counts | Honest numbers |
| 1.6.8 | Commit with message explaining the correction | Git history records the honesty |

**New labeling system:**
```
[x]   = PRODUCTION — user runs it, correct results, tested
[sim] = SIMULATED — runs correctly but underlying mechanism is fake
         (e.g., CPU pretends to be GPU, synchronous pretends to be async)
[f]   = FRAMEWORK — code exists, not callable from .fj
[s]   = STUB — near-empty placeholder
```

**Corrected module counts:**
```
Before V20.5:  48 [x], 0 [sim], 5 [f], 3 [s]  ← INFLATED
After V20.5:   42 [x], 6 [sim], 5 [f], 3 [s]  ← HONEST

Simulated [sim] modules (moved from [x]):
  - accelerator (classify_workload works, GPU dispatch faked)
  - concurrency_v2 (actor API works, threading faked)
  - rt_pipeline (pipeline_run works, real-time scheduling faked)
  - ml_advanced/diffusion (noise schedule works, UNet faked)
  - ml_advanced/reinforcement (env.step works, DQN/PPO faked)
  - debugger_v2 (record/replay works, reverse stepping faked)
```

---

## 3. Tier 2: Error Quality (V20.6) — 18 tasks

> **Priority:** SHOULD DO before FajarQuant.
> **Goal:** Errors show file:line, Environment doesn't leak memory.

### Phase 2.1: Add Source Spans to RuntimeError (8 tasks)

Currently RuntimeError says "RE001: division by zero" but NOT where.
After fix: "RE001: division by zero at examples/math.fj:42:5"

| # | Task | Verification |
|---|------|-------------|
| 2.1.1 | Add optional `span: Option<Span>` field to RuntimeError enum | Compiles, existing tests pass |
| 2.1.2 | Add `with_span(self, span: Span)` method to RuntimeError | Chainable: `RuntimeError::DivisionByZero.with_span(span)` |
| 2.1.3 | Propagate span in `eval_binary()` — arithmetic errors get span | `1/0` shows line number |
| 2.1.4 | Propagate span in `eval_call()` — function errors get span | `undefined_fn()` shows line number |
| 2.1.5 | Propagate span in `eval_index()` — array bounds errors get span | `arr[99]` shows line number |
| 2.1.6 | Update `FjDiagnostic::from_runtime_error()` to use span | CLI shows caret pointing to error |
| 2.1.7 | Integration test: runtime error shows correct line | Span in error output |
| 2.1.8 | Example: `fj run` with intentional error shows file:line | User-visible improvement |

**Implementation sketch:**
```rust
pub enum RuntimeError {
    #[error("RE001: division by zero")]
    DivisionByZero,
    // ... other variants ...
}

impl RuntimeError {
    /// Attach source location to this error.
    pub fn with_span(self, span: crate::lexer::Span) -> EvalError {
        EvalError::RuntimeWithSpan(self, span)
    }
}

// New variant in EvalError:
pub enum EvalError {
    Runtime(RuntimeError),
    RuntimeWithSpan(RuntimeError, Span),  // NEW
    Control(Box<ControlFlow>),
}
```

### Phase 2.2: Fix Environment Memory Leak (4 tasks)

Change parent pointer from `Rc` to `Weak` to break reference cycles.

| # | Task | Verification |
|---|------|-------------|
| 2.2.1 | Change `Environment.parent` from `Option<Rc<RefCell<>>>` to `Option<Weak<RefCell<>>>` | Compiles |
| 2.2.2 | Update all `parent.as_ref()` calls to `parent.upgrade()` with fallback | Existing tests pass |
| 2.2.3 | Add REPL memory leak test: create 10,000 closures, check Rc strong counts | No unbounded growth |
| 2.2.4 | Update closure capture to keep strong Rc where needed, Weak for parent chain | Closures still work correctly |

**Risk:** This is a delicate change. Closures need strong references to their
captured environment, but the parent chain should use Weak to break cycles.

**Implementation:**
```rust
// env.rs — BEFORE:
parent: Option<Rc<RefCell<Environment>>>,

// env.rs — AFTER:
parent: Option<std::rc::Weak<RefCell<Environment>>>,

// Lookup — BEFORE:
if let Some(parent) = &self.parent {
    parent.borrow().get(name)
}

// Lookup — AFTER:
if let Some(parent_weak) = &self.parent {
    if let Some(parent) = parent_weak.upgrade() {
        parent.borrow().get(name)
    } else {
        None  // parent was deallocated
    }
}
```

### Phase 2.3: Parser Unwrap Cleanup (3 tasks)

The 4 unwraps in parser/mod.rs are the only production unwraps in critical code.

| # | Task | Verification |
|---|------|-------------|
| 2.3.1 | Find and fix all 4 unwraps in parser/mod.rs → return ParseError | `cargo test --lib parser` passes |
| 2.3.2 | Find and fix 1 unwrap in interpreter/eval/mod.rs → return RuntimeError | `cargo test --lib interpreter` passes |
| 2.3.3 | Add `#![deny(clippy::unwrap_used)]` to parser and interpreter crate roots | CI prevents future unwraps |

### Phase 2.4: Verification (3 tasks)

| # | Task | Verification |
|---|------|-------------|
| 2.4.1 | Full test suite: `cargo test` (all test targets) | 0 failures, 0 crashes |
| 2.4.2 | Clippy clean: `cargo clippy -- -D warnings` | 0 warnings |
| 2.4.3 | Format check: `cargo fmt -- --check` | 0 diffs |

---

## 4. Tier 3: Code Cleanup (V20.7) — 16 tasks

> **Priority:** NICE TO HAVE. Can run parallel with early FajarQuant work.
> **Goal:** Remove dead code, consolidate modules.

### Phase 3.1: Dead Code Audit & Decision (6 tasks)

For each dead module, decide: KEEP (wire later), ARCHIVE (move to examples/archive/),
or REMOVE (delete from src/).

| # | Module | Lines | Decision | Reason |
|---|--------|-------|----------|--------|
| 3.1.1 | `src/demos/` | 16,257 | ARCHIVE | Useful as reference, not part of compiler |
| 3.1.2 | `src/rtos/` | 8,043 | KEEP [f] | V21 Hardware needs this |
| 3.1.3 | `src/iot/` | 5,033 | KEEP [f] | V21 Hardware needs this |
| 3.1.4 | `src/lsp_v2/` | 3,395 | REMOVE | Superseded by src/lsp/ |
| 3.1.5 | `generators_v12` + `wasi_v12` + `stdlib/` | 862 | REMOVE | Legacy, superseded |
| 3.1.6 | 8x `const_*` modules | 4,531 | KEEP [f] | FajarQuant Phase 5 needs const_reflect |

### Phase 3.2: Consolidate Simulated Builtins (4 tasks)

Move all simulated builtins into a clearly separated section of builtins.rs.

| # | Task | Verification |
|---|------|-------------|
| 3.2.1 | Group all `[sim]` builtins under `// === SIMULATED BUILTINS ===` comment block | Code organized |
| 3.2.2 | Add `fn is_simulated(name: &str) -> bool` helper | Can query simulation status |
| 3.2.3 | `fj run --strict` mode: reject simulated builtins | For production use |
| 3.2.4 | Update analyzer to warn on simulated builtin usage | "warning: accelerate() is simulated" |

### Phase 3.3: Documentation Cleanup (4 tasks)

| # | Task | Verification |
|---|------|-------------|
| 3.3.1 | Create `docs/ARCHITECTURE_V20.md` — updated architecture with honest module status | Replaces outdated docs |
| 3.3.2 | Mark V17 audit as SUPERSEDED in its header | No confusion with outdated claims |
| 3.3.3 | Update `docs/V19_V21_COMPLETE_56_PLAN.md` — mark completed, simulated, remaining | Accurate roadmap |
| 3.3.4 | Clean up docs/ — remove or archive outdated plans (V06-V15 era) | Fewer docs, more accurate |

### Phase 3.4: Verification (2 tasks)

| # | Task | Verification |
|---|------|-------------|
| 3.4.1 | `cargo test` all targets pass | 0 failures |
| 3.4.2 | LOC count after cleanup | Reduced by ~20K+ (removed dead code) |

---

## 5. Tier 4: FajarQuant Prerequisites — 12 tasks

> **Priority:** Do immediately after Tier 1+2. These benefit ALL of Fajar Lang.
> **Goal:** Add real tensor operations that FajarQuant AND general users need.

### Phase 4.1: New Tensor Operations (10 tasks)

These are REAL operations on ndarray. Not HashMap workarounds.

| # | Op | Impl in `ops.rs` | Builtin wire | Test |
|---|-----|------------------|-------------|------|
| 4.1.1 | `sign(tensor)` | `data.mapv(f64::signum)` | `"sign"` | `sign([-3,0,5]) → [-1,0,1]` |
| 4.1.2 | `argmin(tensor)` | `data.iter().enumerate().min_by()` | `"argmin"` | `argmin([3,1,2]) → 1` |
| 4.1.3 | `norm(tensor)` | `data.mapv(|x| x*x).sum().sqrt()` | `"norm"` | `norm([3,4]) → 5.0` |
| 4.1.4 | `dot(a, b)` | `a.data() * b.data() → sum` | `"dot"` | `dot([1,2],[3,4]) → 11.0` |
| 4.1.5 | `exp_tensor(t)` | `data.mapv(f64::exp)` | `"exp_tensor"` | `exp_tensor([0,1]) → [1.0, 2.718...]` |
| 4.1.6 | `log_tensor(t)` | `data.mapv(f64::ln)` | `"log_tensor"` | `log_tensor([1,E]) → [0.0, 1.0]` |
| 4.1.7 | `sqrt_tensor(t)` | `data.mapv(f64::sqrt)` | `"sqrt_tensor"` | `sqrt_tensor([4,9]) → [2.0, 3.0]` |
| 4.1.8 | `abs_tensor(t)` | `data.mapv(f64::abs)` | `"abs_tensor"` | `abs_tensor([-3,4]) → [3.0, 4.0]` |
| 4.1.9 | `exp(x)` scalar | `f64::exp(x)` | `"exp"` | `exp(1.0) → 2.718...` |
| 4.1.10 | `gamma(x)` scalar | Lanczos approximation | `"gamma"` | `gamma(5.0) → 24.0` |

**Implementation per op:** ~20 lines in ops.rs + ~15 lines in builtins.rs + ~5 lines register.rs
**Total:** ~400 LOC

### Phase 4.2: Integration (2 tasks)

| # | Task | Verification |
|---|------|-------------|
| 4.2.1 | `tests/tensor_ops_tests.rs` — 20 tests for all new ops | `cargo test --test tensor_ops_tests` |
| 4.2.2 | `examples/linalg_demo.fj` — demo of all new ops | `fj run examples/linalg_demo.fj` |

---

## 6. Execution Schedule

```
┌─────────────────────────────────────────────────────────────────┐
│  SESSION 1: Tier 1 Phases 1.1-1.3 (Stability)                  │
│                                                                 │
│  12 tasks:                                                      │
│  - Fix 4 crashing tests (16MB thread wrapper)                   │
│  - Fix pipeline_run error propagation                           │
│  - Add honest [sim] labels to simulated builtins                │
│  - Verify: cargo test --test eval_tests + safety_tests          │
│                                                                 │
│  Gate: ALL eval_tests (948) + safety_tests (96) pass            │
├─────────────────────────────────────────────────────────────────┤
│  SESSION 2: Tier 1 Phases 1.4-1.5 (Test Coverage + Unwraps)    │
│                                                                 │
│  18 tasks:                                                      │
│  - 14 V20 builtin tests (v20_builtin_tests.rs)                 │
│  - Fix 5 unwraps in critical path                               │
│  - Add clippy deny for unwrap_used                              │
│                                                                 │
│  Gate: cargo test --test v20_builtin_tests ALL pass             │
├─────────────────────────────────────────────────────────────────┤
│  SESSION 3: Tier 1 Phase 1.6 + Tier 2 Phases 2.1-2.2           │
│                                                                 │
│  20 tasks:                                                      │
│  - Documentation honesty (8 tasks)                              │
│  - RuntimeError source spans (8 tasks)                          │
│  - Environment Weak<> fix (4 tasks)                             │
│                                                                 │
│  Gate: Runtime errors show file:line, REPL doesn't leak         │
├─────────────────────────────────────────────────────────────────┤
│  SESSION 4: Tier 2 Phases 2.3-2.4 + Tier 4 (Tensor Ops)        │
│                                                                 │
│  17 tasks:                                                      │
│  - Parser unwrap cleanup (3 tasks)                              │
│  - Verification (3 tasks)                                       │
│  - 10 new tensor ops + 2 integration tasks                     │
│                                                                 │
│  Gate: sign, argmin, norm, dot, exp all work in .fj             │
├─────────────────────────────────────────────────────────────────┤
│  SESSION 5: Tier 3 (Cleanup) — OPTIONAL, can defer              │
│                                                                 │
│  16 tasks:                                                      │
│  - Dead code audit + archive/remove                             │
│  - Consolidate simulated builtins                               │
│  - Documentation cleanup                                        │
│                                                                 │
│  Gate: LOC reduced by ~20K, docs accurate                       │
├─────────────────────────────────────────────────────────────────┤
│  AFTER REMEDIATION: Ready for FajarQuant Phase 1                │
│                                                                 │
│  Foundation:                                                    │
│  - All tests pass (0 crashes, 0 failures)                       │
│  - Every builtin tested (24+ new tests)                         │
│  - Errors show file:line (source spans)                         │
│  - No memory leaks (Weak parent pointers)                       │
│  - 10 new tensor ops for quantization math                      │
│  - Honest documentation (no inflated claims)                    │
└─────────────────────────────────────────────────────────────────┘
```

---

## 7. Gate Criteria

### After Session 1 (Stability):
```bash
cargo test --test eval_tests              # 948/948 pass
cargo test --test safety_tests            # 96/96 pass
cargo test --test benchmark_validation    # ALL pass (already fixed)
# Simulated builtins print [sim] prefix
```

### After Session 2 (Test Coverage):
```bash
cargo test --test v20_builtin_tests       # 28/28 pass (14 builtins × 2 tests)
cargo clippy -- -D warnings               # 0 warnings
# Zero unwraps in parser/mod.rs and eval/mod.rs
```

### After Session 3 (Error Quality + Docs):
```bash
# Runtime error shows source location:
# RE001: division by zero
#   --> examples/math.fj:42:5
#    |
# 42 |     let x = 1 / 0
#    |             ^^^^^
cargo test --lib                          # ALL pass
# CLAUDE.md says 42 [x], 6 [sim], 5 [f], 3 [s]
```

### After Session 4 (Tensor Ops):
```bash
fj run examples/linalg_demo.fj           # All tensor ops work
cargo test --test tensor_ops_tests        # 20/20 pass
cargo test --lib                          # 8,300+ pass, 0 fail
cargo clippy -- -D warnings               # 0 warnings
cargo fmt -- --check                      # 0 diffs
```

### Final Gate (Ready for FajarQuant):
```bash
cargo test                                # ALL targets pass, 0 crashes
# Module status: 42 [x], 6 [sim], 5 [f], 3 [s] = 56 total
# New tensor ops: sign, argmin, norm, dot, exp_tensor, log_tensor,
#                 sqrt_tensor, abs_tensor, exp (scalar), gamma (scalar)
# Every V20 builtin has >= 2 tests
# RuntimeError has source spans
# Environment uses Weak parent pointers
# Documentation matches reality
```

---

## Appendix A: Complete Builtin Status Table

### Production [x] — Tested, Real Implementation

| Builtin | Tests | Verified |
|---------|-------|---------|
| println, print | 100+ implicit | Every test uses it |
| len, type_of, assert, assert_eq | 50+ | Core builtins |
| push, pop, sort, reverse | 10+ | Array ops |
| map_new, map_insert, map_get, map_get_or, map_remove, map_contains_key, map_keys, map_values, map_len | 8+ | HashMap ops |
| zeros, ones, randn, from_data, eye, xavier, arange, linspace | 20+ | Tensor creation |
| matmul, transpose, reshape, flatten, squeeze, concat, split | 15+ | Tensor ops |
| relu, sigmoid, tanh, softmax, gelu, leaky_relu | 10+ | Activations |
| mse_loss, cross_entropy, bce_loss, l1_loss | 8+ | Loss functions |
| backward, grad, requires_grad, set_requires_grad | 10+ | Autograd |
| Dense, Conv2d, MultiHeadAttention, forward, layer_params | 8+ | Layers |
| SGD, Adam, step, zero_grad | 6+ | Optimizers |
| accuracy, precision, recall, f1_score | 4+ | Metrics |
| quantize_int8, dequantize_int8, quantized_matmul | 10 | INT8 quantization |
| http_get, http_post, tcp_connect, dns_resolve | 4+ | Networking |
| ffi_load_library, ffi_call | 2+ | FFI |
| channel_create, channel_send, channel_recv | 3+ | Channels |
| async_sleep, async_spawn, async_join, async_timeout | 4+ | Async I/O |
| read_file, write_file, file_exists | 3+ | File I/O |
| regex_match, regex_find, regex_replace | 3+ | Regex |
| sha256, aes_encrypt, aes_decrypt | 3+ | Crypto |
| const_type_name, const_field_names | 2+ | Reflection |
| macro_rules! | 5+ | User macros |
| turboquant_create (if impl'd) | - | Future |

### Simulated [sim] — Runs but Fakes Underlying Mechanism

| Builtin | What's Real | What's Faked | Tests (after V20.5) |
|---------|-----------|-------------|---------------------|
| accelerate() | Workload classification | GPU/NPU dispatch (always CPU) | 2 |
| actor_spawn() | Creates actor map | Threading (synchronous) | 2 |
| actor_send() | Calls handler function | Mailbox/async (synchronous) | 2 |
| actor_supervise() | Stores strategy | Restart/monitoring (no-op) | 2 |
| diffusion_create() | Noise schedule math | UNet architecture | 2 |
| diffusion_denoise() | Scaling operation | Real denoising process | 2 |
| rl_agent_create() | Creates env struct | Neural network agent | 2 |
| rl_agent_step() | Simple env.step() | Real RL training | 2 |
| pipeline_run() | Calls functions sequentially | RT scheduling/deadlines | 2 |
| const_alloc() | Creates allocation descriptor | Actual .rodata placement | 2 |

### Framework [f] — Code Exists, Not Callable from .fj

| Module | Lines | Wire Planned |
|--------|-------|-------------|
| rtos/ | 8,043 | V21 Hardware |
| iot/ | 5,033 | V21 Hardware |
| const_* (8 modules) | 4,531 | FajarQuant Phase 5 |
| demos/ | 16,257 | Archive candidate |

---

## Appendix B: Files Modified Per Session

### Session 1:
```
tests/eval_tests.rs          — wrap 2 tests in 16MB thread
tests/safety_tests.rs        — wrap 2 tests in 16MB thread
src/interpreter/eval/builtins.rs — fix pipeline_run, add [sim] labels
```

### Session 2:
```
tests/v20_builtin_tests.rs   — NEW (28 tests)
src/parser/mod.rs             — fix 4 unwraps
src/interpreter/eval/mod.rs   — fix 1 unwrap
```

### Session 3:
```
src/interpreter/eval/mod.rs   — RuntimeError span propagation
src/interpreter/env.rs        — Weak parent pointer
docs/HONEST_STATUS_V20_5.md  — NEW
CLAUDE.md                     — corrected counts
```

### Session 4:
```
src/runtime/ml/ops.rs         — 10 new tensor operations
src/interpreter/eval/builtins.rs — wire 10 new ops
src/analyzer/type_check/register.rs — register 10 new ops
src/interpreter/eval/mod.rs   — add to builtin name list
tests/tensor_ops_tests.rs    — NEW (20 tests)
examples/linalg_demo.fj      — NEW
```

---

*Full Remediation Plan — 84 tasks, 4 tiers, 5 sessions*
*"Fix the foundation before building the tower."*
*Every task has concrete verification. Every claim matches reality.*
