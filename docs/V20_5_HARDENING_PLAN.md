# V20.5 "Hardening" — Fix Before Build

> **Date:** 2026-04-03
> **Priority:** CRITICAL — must complete before FajarQuant
> **Rule:** Every fix verified with `cargo test`. Every builtin has at least 2 tests.
> **Lesson:** V13-V15 inflation happened because we built on shaky foundations.
>            V20.5 exists to prevent repeating that mistake.

---

## Why This Exists

Post-V20 audit revealed 7 serious issues. Building FajarQuant (~7,300 LOC of
advanced quantization algorithms) on top of builtins that return HashMaps,
silently swallow errors, and have zero tests would be engineering malpractice.

**V20.5 is NOT optional. It's the foundation FajarQuant needs.**

---

## Audit Summary

### What's Real (Leave Alone)
- Core compiler pipeline: lexer → parser → analyzer → interpreter ✅
- INT8 quantization: 10 tests, mathematically sound ✅  
- @kernel/@device context enforcement: transitive taint tracking ✅
- User macros with $x metavariables ✅
- Pattern match destructuring (Ok/Err/Some/None) ✅
- Real async (tokio): async_sleep, async_spawn, async_join ✅
- Tensor ops: matmul, relu, sigmoid, softmax, randn, zeros, ones ✅
- String ops, HashMap ops, f-strings, for loops — all solid ✅
- 8,287 lib tests, 1,268 integration tests — pass rate 100% ✅

### What Needs Fixing (This Plan)
1. 9 HashMap workaround builtins → proper implementations
2. 0 tests for V20 builtins → minimum 2 tests each
3. Fake GPU dispatch → honest about CPU-only, real dispatch deferred
4. Synchronous actors → honest labeling, or real async
5. Error-swallowing pipeline → proper error propagation
6. eval_tests stack overflow → depth guard in interpreter
7. Dead ML code → document honestly, wire what's useful

---

## Phase 1: Interpreter Stability (5 tasks)

> Fix crashes and silent failures first.

| # | Task | Verification |
|---|------|-------------|
| 1.1 | Add AST nesting depth guard in interpreter eval_expr | 50-level nested if no longer crashes, returns clean error |
| 1.2 | Fix eval_tests: `t3_5_deep_nesting_if` — either guard or increase stack | `cargo test --test eval_tests` passes |
| 1.3 | Fix safety_tests: `safety_stack_overflow_mutual_recursion` | `cargo test --test safety_tests` passes |
| 1.4 | pipeline_run: propagate errors instead of swallowing | Stage failure stops pipeline, returns Err |
| 1.5 | Integration test: pipeline with failing stage returns error | `cargo test pipeline_error_propagation` |

**Implementation for 1.1:**
```rust
// In eval_expr, track AST nesting depth (separate from call_depth)
fn eval_expr(&mut self, expr: &Expr) -> EvalResult {
    self.ast_depth += 1;
    if self.ast_depth > MAX_AST_DEPTH {
        self.ast_depth -= 1;
        return Err(RuntimeError::StackOverflow { 
            depth: MAX_AST_DEPTH, backtrace: "AST nesting too deep".into() 
        }.into());
    }
    let result = self.eval_expr_inner(expr);
    self.ast_depth -= 1;
    result
}
```

---

## Phase 2: Honest Builtin Labels (4 tasks)

> Don't fake what we can't deliver. Label honestly.

| # | Task | Verification |
|---|------|-------------|
| 2.1 | `accelerate()` — rename to `classify_workload()`, return classification only (no fake dispatch) | Returns `{class: "ComputeBound", suggested_device: "GPU"}` — honest |
| 2.2 | `actor_spawn/send` — rename to `simulated_actor_spawn/send`, document as synchronous simulation | Docs say "simulated — no threading" |
| 2.3 | Add deprecation warning when calling `accelerate()` — suggest `classify_workload()` | Warning printed to stderr |
| 2.4 | Update fj demo --list descriptions to be honest about simulation | No false claims |

**Principle:** Better to say "this is a simulation" than pretend it's real dispatch.
When we add real GPU dispatch later, we add `gpu_dispatch()` as a new builtin.

---

## Phase 3: Test Coverage for V20 Builtins (12 tasks)

> Every builtin gets at least 2 unit tests.

| # | Builtin | Test 1 | Test 2 |
|---|---------|--------|--------|
| 3.1 | `diffusion_create(steps)` | Returns map with correct steps/schedule | Works with steps=10, 100, 1000 |
| 3.2 | `diffusion_denoise(model, tensor, step)` | Output shape matches input | Step 0 vs step 99 produce different magnitudes |
| 3.3 | `rl_agent_create(state_dim, action_dim)` | Returns map with correct dims | state array has correct length |
| 3.4 | `rl_agent_step(agent, action)` | Returns reward and done | Different actions produce different rewards |
| 3.5 | `pipeline_create/add_stage` | Empty pipeline has 0 stages | Adding 3 stages shows count=3 |
| 3.6 | `pipeline_run` | 3-stage pipeline produces correct output | Error in stage propagates (after fix 1.4) |
| 3.7 | `actor_spawn` | Returns map with name and status | Different names produce different actors |
| 3.8 | `actor_send` | Calls handler and returns result | Handler with side effects works |
| 3.9 | `const_alloc(size)` | Returns map with correct size | size=0 and size=65536 both work |
| 3.10 | `const_size_of` | i64=8, bool=1, str depends on length | Tensor size scales with elements |
| 3.11 | `const_align_of` | i64=8, bool=1, char=4 | Consistent with platform ABI |
| 3.12 | `map_get_or(map, key, default)` | Returns value when key exists | Returns default when key missing |

**Test location:** `tests/v20_builtin_tests.rs` (new file)

---

## Phase 4: FajarQuant Prerequisites — Real Tensor Ops (10 tasks)

> These are Phase 0 from FajarQuant plan, but they benefit ALL of Fajar Lang.

| # | Task | Signature | Test |
|---|------|-----------|------|
| 4.1 | `sign(tensor)` — element-wise sign | `[-3,0,5]` → `[-1,0,1]` | 2 tests |
| 4.2 | `argmin(tensor)` — index of minimum | `[3,1,2]` → `1` | 2 tests |
| 4.3 | `norm(tensor)` — L2 norm | `[3,4]` → `5.0` | 2 tests |
| 4.4 | `dot(a, b)` — inner/dot product | `[1,2]·[3,4]` → `11.0` | 2 tests |
| 4.5 | `exp_tensor(tensor)` — element-wise e^x | `[0,1]` → `[1.0, 2.718...]` | 2 tests |
| 4.6 | `log_tensor(tensor)` — element-wise ln | `[1,E]` → `[0.0, 1.0]` | 2 tests |
| 4.7 | `sqrt_tensor(tensor)` — element-wise sqrt | `[4,9]` → `[2.0, 3.0]` | 2 tests |
| 4.8 | `abs_tensor(tensor)` — element-wise abs | `[-3,4]` → `[3.0, 4.0]` | 2 tests |
| 4.9 | `exp(x)` — scalar e^x | `exp(1.0)` → `2.718...` | 2 tests |
| 4.10 | `gamma(x)` — Gamma function | `gamma(5.0)` → `24.0` | 2 tests |

**Implementation:** Each is ~20 lines in `ops.rs` + ~15 lines in `builtins.rs`.
These are REAL operations on ndarray, not HashMap workarounds.

---

## Phase 5: Documentation Honesty (4 tasks)

> Update all docs to reflect reality.

| # | Task | Verification |
|---|------|-------------|
| 5.1 | Update CLAUDE.md — correct module counts, mark simulations as [sim] | Counts match reality |
| 5.2 | Update MEMORY.md — correct V20 status | No inflated claims |
| 5.3 | Create `docs/HONEST_STATUS_V20.md` — per-builtin real vs simulated table | Every builtin labeled |
| 5.4 | Update V19_V21 plan — mark simulated builtins honestly | [sim] tag on simulated features |

**New labeling system:**
```
[x]   = production — user can run it, correct results
[sim] = simulated — runs but fakes underlying mechanism (e.g., CPU pretends to be GPU)
[f]   = framework — code exists but not callable from .fj
[s]   = stub — near-empty
```

---

## Task Summary

| Phase | Tasks | Purpose |
|-------|-------|---------|
| 1: Stability | 5 | Fix crashes, error propagation |
| 2: Honest Labels | 4 | No fake claims |
| 3: Test Coverage | 12 | Every V20 builtin tested |
| 4: Real Tensor Ops | 10 | FajarQuant prerequisites (real, not HashMap) |
| 5: Doc Honesty | 4 | Update all documentation |
| **Total** | **35** | **Solid foundation for FajarQuant** |

---

## Execution Order

```
Session 1:  Phase 1 (stability) + Phase 2 (honest labels)     — 9 tasks
Session 2:  Phase 3 (test coverage) + Phase 4 (tensor ops)    — 22 tasks  
Session 3:  Phase 5 (docs) + commit + verify                  — 4 tasks
```

---

## After V20.5: What Changes

| Before (V20) | After (V20.5) |
|-------------|---------------|
| 48/56 modules "production" | ~42 production, 6 simulated, 5 framework, 3 stub |
| accelerate() = "GPU dispatch" | classify_workload() = honest classification |
| actor_spawn() = "actor model" | simulated_actor_spawn() = synchronous simulation |
| 0 tests for V20 builtins | 24+ tests for V20 builtins |
| eval_tests crash | eval_tests pass (depth guard) |
| pipeline swallows errors | pipeline propagates errors |
| HashMap workaround builtins | Still HashMap (acceptable for simulations) but TESTED and LABELED |

**The module count goes DOWN from 48 to ~42. This is CORRECT.**
**Honest 42 is worth more than inflated 48.**

---

## Gate: V20.5 Complete When

```bash
cargo test --lib                          # 8,300+ pass, 0 fail
cargo test --test eval_tests              # PASS (no crash)
cargo test --test safety_tests            # PASS (no crash)
cargo test --test v20_builtin_tests       # ALL pass (24+ tests)
cargo clippy -- -D warnings               # 0 warnings
fj run examples/fajarquant_prerequisites.fj  # tensor ops demo works
```

Only THEN do we start FajarQuant Phase 1.

---

*V20.5 "Hardening" — 35 tasks, honest foundation*
*"Honest 42 modules is worth more than inflated 48."*
*Written with the lesson of V13-V15: NEVER build advanced features on untested foundations.*
