# V11 Option 6: Language Features — 4-Week Detailed Plan

## Context

Fajar Lang's borrow checker is 80% real but has critical gaps: NLL computed but unused, all types treated as Copy, lifetimes parsed but not validated. This plan closes every gap in 4 phases (1 week each), 26 tasks, ~2,505 LOC, 80 new tests.

**Key discovery:** `is_copy_type()` returns `true` for all types, making move tracking dead code. Fix with `--strict-ownership` flag.

---

## Phase A (Week 1): Wire NLL + Strict Ownership — 375 LOC, 10 tests

| # | Task | File (line) | LOC | Verify |
|---|------|-------------|-----|--------|
| A1 | Add `strict_ownership: bool` to TypeChecker + `--strict-ownership` CLI flag | `type_check/mod.rs:1248`, `mod.rs:41` | 30 | `TypeChecker::new_strict()` has field `true` |
| A2 | Real `is_copy_type_strict()`: primitives=Copy, String/Array/Struct=Move | `borrow_lite.rs:302-332` | 50 | `is_copy_type_strict(Type::Str) == false` |
| A3 | Wire strict ownership into move tracking in `check_let` + `check_call` | `check.rs:509-531, 1277` | 60 | Strict mode: `let t = s; println(s)` → ME001 |
| A4 | Wire move-while-borrowed check for function args | `check.rs:522-528` | 20 | `let r = &s; let t = s;` → ME003 |
| A5 | Verify NLL borrow release works with strict mode | `check.rs:422-440` | 15 | Sequential borrows allowed after last use |
| A6 | 10 strict-mode borrow/move tests | `type_check/mod.rs` tests | 200 | All 10 assert specific errors |

**Deps:** A1 ← A2 ← A3 ← A4, A5; all ← A6

---

## Phase B (Week 2): Lifetime Validation + Elision — 530 LOC, 10 tests

| # | Task | File (line) | LOC | Verify |
|---|------|-------------|-----|--------|
| B1 | Add `LifetimeEnv` (HashMap name→id) to TypeChecker, register fn lifetime params | `type_check/mod.rs:1189`, `check.rs:120` | 60 | `<'a>(&'a i32)` resolves to valid ID |
| B2 | Extend `Type::Ref`/`RefMut` with `Option<u32>` lifetime ID, update ~20 match sites | `type_check/mod.rs:159-162` | 120 | `&'a i32` → `Ref(I32, Some(1))` |
| B3 | Enforce lifetime elision rules 1-3: error on ambiguous output | `check.rs:2380-2459` | 50 | `fn f(x: &'a, y: &'b) -> &i32` → error |
| B4 | Detect dangling references (return &local) | `check.rs:182` | 80 | `fn f() -> &i32 { let x = 1; &x }` → ME010 |
| B5 | Validate lifetime params on struct reference fields | `check.rs` struct checking | 40 | `struct S { r: &i32 }` → warning |
| B6 | 10 lifetime validation tests | `type_check/mod.rs` tests | 180 | All 10 assert specific errors |

**Deps:** B1 ← B2 ← B3; B1 ← B4; B1 ← B5; all ← B6

---

## Phase C (Week 3): Advanced Borrows — 630 LOC, 10 tests

| # | Task | File (line) | LOC | Verify |
|---|------|-------------|-----|--------|
| C1 | Two-phase borrows: `borrow_mut_two_phase()` (Reserved→Activated) | `borrow_lite.rs`, use `polonius/two_phase.rs` concepts | 100 | `push(v, len(v))` OK in strict mode |
| C2 | Reborrowing: `reborrow_imm()` — create `&T` from `&mut T` | `borrow_lite.rs`, `check.rs:1211` | 70 | `let r2 = &*r1` where r1: &mut OK |
| C3 | Field-level borrows: `BorrowPath::Field(var, field)`, disjoint field tracking | `borrow_lite.rs` | 120 | `&mut s.x + &mut s.y` OK |
| C4 | Drop order validation: refs dropped before referents | `check.rs` in `check_block:1660` | 80 | Reverse-decl-order drop validated |
| C5 | Borrow through function call params | `check.rs:1277` in `check_call` | 60 | `fn f(x: &i32)` borrows argument |
| C6 | 10 advanced borrow tests | `type_check/mod.rs` tests | 200 | Two-phase, reborrow, fields, drop |

**Deps:** C1-C5 independent; all ← C6

---

## Phase D (Week 4): Error Messages + 50 Tests + Docs — 970 LOC, 50 tests

| # | Task | File | LOC | Verify |
|---|------|------|-----|--------|
| D1 | Enhance ME001-ME010 errors with suggestion hints | `type_check/mod.rs` Display impls | 100 | "consider cloning" hint in output |
| D2 | NLL-aware error spans (show last-use position) | `check.rs` | 40 | Error shows "borrow used until line X" |
| D3 | 20 end-to-end safety_tests.rs integration tests | `tests/safety_tests.rs` | 400 | Full pipeline: parse→analyze→verify |
| D4 | 10 edge-case unit tests (branch moves, loops, closures) | `type_check/mod.rs` tests | 200 | Conditional moves, async borrows |
| D5 | 10 borrow_lite/cfg unit tests | `borrow_lite.rs`, `cfg.rs` tests | 150 | MoveTracker + NllInfo internals |
| D6 | Module documentation + design doc | `borrow_lite.rs`, `cfg.rs` doc comments | 80 | Accurate doc comments |

**Deps:** D1-D2 need Phase A-C; D3-D5 need Phase A-C; D6 independent

---

## Summary

| Phase | Week | LOC | Tests | Outcome |
|-------|------|-----|-------|---------|
| A | 1 | 375 | 10 | Real move errors fire, NLL integrated |
| B | 2 | 530 | 10 | Dangling ref detection, lifetime elision |
| C | 3 | 630 | 10 | Two-phase, reborrow, field borrows |
| D | 4 | 970 | 50 | Rust-quality errors, 80 total tests |
| **Total** | **4 weeks** | **2,505** | **80** | **Production borrow checker** |

## Risk Mitigation

1. **Backward compat:** `--strict-ownership` flag guards all new behavior. Default mode unchanged. 7,468 existing tests unaffected.
2. **Type::Ref change:** Use `Ref(inner, _)` pattern in all existing matches to ignore new lifetime field.
3. **Polonius not replaced:** Cherry-pick concepts (two-phase, reborrow) into borrow_lite. Full Polonius deferred.
4. **Field tracking scope:** Only single-level `s.x` for V11. Nested `s.x.y` deferred.

## Verification

After all 4 phases: `cargo test --lib` passes 7,548+ tests (80 new). `--strict-ownership` flag enables production borrow checking. Error messages include hints and NLL-aware spans.

## Critical Files
- `src/analyzer/borrow_lite.rs` — MoveTracker + new features
- `src/analyzer/type_check/check.rs` — Integration point for all checks
- `src/analyzer/type_check/mod.rs` — TypeChecker struct + error types
- `src/analyzer/cfg.rs` — NLL liveness
- `src/analyzer/scope.rs` — Symbol table
- `tests/safety_tests.rs` — End-to-end integration tests
