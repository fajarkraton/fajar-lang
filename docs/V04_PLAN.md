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

### Sprint 1: Generic Enum Infrastructure
**Goal:** `enum Option<T> { Some(T), None }` with typed payloads

- [ ] S1.1 — Enum payload type tracking: `enum_payload_types: HashMap<String, Vec<(String, Type)>>`
- [ ] S1.2 — Enum monomorphization: `Option__mono_i64`, `Option__mono_str`, etc.
- [ ] S1.3 — Type-aware pattern matching: extract payload with correct Cranelift type
- [ ] S1.4 — Multi-field variants: `Rect(f64, f64)` → stack slot with 2 fields
- [ ] S1.5 — Generic enum in function signatures: `fn unwrap<T>(opt: Option<T>) -> T`
- [ ] S1.6 — 6 tests: generic Option, generic Result, multi-field variant, pattern match typed

### Sprint 2: Option<T> and Result<T,E> in Practice
**Goal:** `mutex.try_lock() -> Option<i64>`, `fn parse(s: str) -> Result<i64, str>`

- [ ] S2.1 — Option return from methods: try_lock, HashMap.get
- [ ] S2.2 — Result return from functions: parse_int, file operations
- [ ] S2.3 — `?` operator with typed Result<T,E> (not just tag/payload)
- [ ] S2.4 — `match` exhaustiveness for generic enums
- [ ] S2.5 — 4 tests: option_return, result_return, typed_question_mark, exhaustive_match

### Sprint 3: Scope-Level Drop/Cleanup
**Goal:** Resources auto-cleaned at block scope exit, not just function exit

- [ ] S3.1 — Scope tracking: `scope_stack: Vec<Vec<(String, OwnedKind)>>`
- [ ] S3.2 — Block entry/exit: push/pop scope on `{ }` blocks
- [ ] S3.3 — Auto-cleanup at scope exit: emit free calls for scope-local resources
- [ ] S3.4 — Drop trait: `trait Drop { fn drop(&mut self) }` with codegen support
- [ ] S3.5 — MutexGuard: auto-unlock when guard goes out of scope
- [ ] S3.6 — 5 tests: scope_cleanup, nested_scopes, drop_trait, mutex_guard, early_return

### Sprint 4: Formal Future/Poll Types
**Goal:** `Future<T>`, `Poll<T>` as proper generic enums (builds on S1-S2)

- [ ] S4.1 — Built-in `Poll<T>` enum: `Ready(T)`, `Pending`
- [ ] S4.2 — Built-in `Future<T>` trait: `fn poll(&mut self) -> Poll<T>`
- [ ] S4.3 — Async fn return type: `async fn foo() -> T` returns `Future<T>`
- [ ] S4.4 — `.await` type checking: reject `.await` on non-Future
- [ ] S4.5 — 4 tests: poll_enum, future_trait, async_return_type, await_type_check

### Sprint 5: Lazy Async (Optional / Stretch)
**Goal:** State machine compilation for multi-await async functions

- [ ] S5.1 — State enum generation: one variant per await point
- [ ] S5.2 — Transform sequential code → state machine poll function
- [ ] S5.3 — Waker integration: wake() reschedules on executor
- [ ] S5.4 — Round-robin executor: ready queue + time-slice yield
- [ ] S5.5 — 4 tests: lazy_poll, state_machine, waker_reschedule, round_robin

### Sprint 6: Polish & MNIST
**Goal:** Real MNIST >90%, example programs, release

- [ ] S6.1 — MNIST training with real data (download + train + eval)
- [ ] S6.2 — Remaining example programs (update for new features)
- [ ] S6.3 — Update docs for generic enums, Drop, lazy async
- [ ] S6.4 — v0.4 release tag + GitHub Release
- [ ] S6.5 — 3 tests: mnist_accuracy, examples_native, release_smoke

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

- [ ] `enum Option<T> { Some(T), None }` compiles and works in native codegen
- [ ] `mutex.try_lock()` returns `Option<i64>` (not raw 0/1)
- [ ] MutexGuard auto-unlocks at scope exit
- [ ] MNIST classifier > 90% accuracy on test set
- [ ] All existing 2,573 tests still pass (zero regression)

---

*V04_PLAN.md v1.0 | Created 2026-03-10*
