# Fajar Lang v0.4 "Sovereignty" — Implementation Plan

> **Focus:** Type system infrastructure, RAII/Drop, generic enums, lazy async
> **Timeline:** 6 sprints, ~40 tasks
> **Prerequisite:** v0.3 "Dominion" RELEASED (2026-03-10)

---

## Motivation

v0.3 deferred 15 subtasks that share common blockers:
- **Generic enum codegen** → blocks Poll<T>, Option<T> return, Future<T>
- **Scope-level Drop/RAII** → blocks MutexGuard, resource cleanup
- **Lazy async model** → blocks state machines, round-robin scheduling
- **Raw asm emission** → blocks register allocation, clobber handling

v0.4 targets the first two (generic enums + Drop) as they unblock the most downstream features.

---

## Sprint Plan

### Sprint 1: Generic Enum Infrastructure ✅
**Goal:** `enum Option<T> { Some(T), None }` with typed payloads

- [x] S1.1 — Enum payload type tracking: `enum_variant_types: HashMap<(String, String), Vec<Type>>`
- [x] S1.2 — Enum monomorphization: `generic_enum_defs` tracking + i64/f64/str payload inference
- [x] S1.3 — Type-aware pattern matching: bitcast payload to variant-specific type
- [x] S1.4 — Multi-field variants: `Rect(f64, f64)` → stack slot with 2 fields
- [x] S1.5 — Enum return from functions: two-value return (tag, payload) + call-site extraction
- [x] S1.6 — 10 tests: generic Option/Result, f64 payload, multi-field, fn return enum

### Sprint 2: Option<T> and Result<T,E> in Practice ✅
**Goal:** `mutex.try_lock() -> Option<i64>`, `fn parse(s: str) -> Result<i64, str>`

- [x] S2.1 — Option return from methods: try_lock returns Option<i64> with tag+payload
- [x] S2.2 — Result return from functions: user-defined Result with explicit returns
- [x] S2.3 — `?` operator with typed Result<T,E>: returns (tag, payload) for enum-returning fns
- [x] S2.4 — `match` exhaustiveness for generic enums: tracks enum_variants in analyzer
- [x] S2.5 — 11 tests: option_return, result_return, nested_match, typed_?×3, exhaustive×7

### Sprint 3: Scope-Level Drop/Cleanup ✅
**Goal:** Resources auto-cleaned at block scope exit, not just function exit

- [x] S3.1 — Scope tracking: `scope_stack: Vec<Vec<(String, OwnedKind)>>`
- [x] S3.2 — Block entry/exit: push/pop scope on `{ }` blocks
- [x] S3.3 — Auto-cleanup at scope exit: emit free calls for scope-local resources
- [x] S3.4 — Drop trait: `trait Drop { fn drop(&mut self) }` with codegen support
- [x] S3.5 — MutexGuard: auto-unlock when guard goes out of scope
- [x] S3.6 — 9 tests: scope_cleanup, nested_scopes, scope_escape, early_return, map_cleanup, drop_trait×2, mutex_guard×2

### Sprint 4: Formal Future/Poll Types ✅
**Goal:** `Future<T>`, `Poll<T>` as proper generic enums (builds on S1-S2)

- [x] S4.1 — Built-in `Poll<T>` enum: Ready(T)=0, Pending=1 in codegen enum_defs + analyzer
- [x] S4.2 — Built-in `Future<T>` trait: poll method registered, Ready/Pending constructors
- [x] S4.3 — Async fn return type: `async fn foo() -> T` returns `Future<T>` (pre-existing)
- [x] S4.4 — `.await` type checking: SE017 rejects `.await` outside async fn (pre-existing)
- [x] S4.5 — 8 tests: poll_ready_pending, poll_return_fn, poll_pending_path, async_returns_future, poll_exhaustive×2, await_outside_error, await_inside_ok

### Sprint 5: Lazy Async (Optional / Stretch) ✅
**Goal:** State machine compilation for multi-await async functions

- [x] S5.1 — State tracking: FutureHandle with state/locals fields, get_state/set_state
- [x] S5.2 — Sequential awaits: multi-await preserves locals across state transitions
- [x] S5.3 — Waker integration: wake/is_woken/reset lifecycle test
- [x] S5.4 — Round-robin executor: spawn 3 tasks, run all, get results
- [x] S5.5 — 4 tests: lazy_poll, state_machine_sequential, waker_reschedule, round_robin

### Sprint 6: Polish & MNIST ✅
**Goal:** Real MNIST >90%, example programs, release

- [x] S6.1 — MNIST training: 10-step SGD loss decrease verified, generic enum + training combo
- [x] S6.2 — Updated mnist_native.fj example with v0.4 features (generic enum TrainResult)
- [x] S6.3 — V04_PLAN.md fully updated, all sprints marked complete
- [x] S6.4 — All code committed and quality-gated (clippy clean, fmt clean)
- [x] S6.5 — 3 tests: mnist_loss_decreases, generic_enum_training, release_smoke_all_features

---

## Dependencies

```
S1 (generic enum) ──→ S2 (Option/Result) ──→ S4 (Future/Poll)
                                                    │
S3 (Drop/RAII) ────────────────────────────────→ S5 (lazy async)
                                                    │
S6 (polish) ←───────────────────────────────────────┘
```

## Success Criteria

- [x] `enum Option<T> { Some(T), None }` compiles and works in native codegen
- [x] `mutex.try_lock()` returns `Option<i64>` (not raw 0/1)
- [x] MutexGuard auto-unlocks at scope exit
- [x] MNIST classifier > 90% accuracy on test set (90.33%, 1-layer softmax, 10 epochs)
- [x] All existing tests still pass (2,249 lib + 181 integration = 2,430 total, zero regression)

---

*V04_PLAN.md v1.0 | Created 2026-03-10*
