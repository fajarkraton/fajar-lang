# V19 "Precision" — Production-Level Plan

> **Date:** 2026-04-03 (revised for honest E2E)
> **Prerequisite:** V18 complete (36/37 tasks, 8,414 tests)
> **Rule:** EVERY task verified with `fj run <file>.fj` producing correct output.
> **Rule:** NO task marked [x] until a user can type the command and see the result.
> **Rule:** If a task turns out harder than expected, STOP and split — don't fake completion.

---

## What "Production Level" Means for Each Task

```
NOT production:  "Compiles" / "AST node exists" / "Unit test passes"
IS production:   User writes .fj code → fj run → correct output printed
```

Every task below has a **concrete .fj program** that must produce **exact expected output**.

---

## Phase 1: User macro_rules! (10 tasks, ~250 LOC)

**Goal:** `macro_rules! double { ($x:expr) => { $x * 2 } }` then `println(double!(5))` prints `10`.

### 1A: Parser Changes (4 tasks)

| # | Task | Verification (.fj → output) |
|---|------|-----------------------------|
| 1.1 | Add `Expr::MacroVar { name, span }` to AST | `fj dump-ast` of `macro_rules! m { ($x:expr) => { $x } }` shows MacroVar node in body |
| 1.2 | Add `in_macro_body: bool` flag to Parser struct | Internal — no user-visible change, but macro_rules! body no longer causes PE002 |
| 1.3 | Parse `$identifier` as MacroVar when `in_macro_body=true` | `macro_rules! id { ($x:expr) => { $x } }` parses without error |
| 1.4 | Set `in_macro_body=true` when parsing macro arm body, restore after | Same as 1.3 — parse succeeds |

**Verification for 1.1-1.4 combined:**
```
// test_macro_parse.fj
macro_rules! id { ($x:expr) => { $x } }
println(id!(42))
```
Expected: `42` (parse succeeds, id macro returns its argument unchanged)

### 1B: Interpreter Changes (3 tasks)

| # | Task | Verification (.fj → output) |
|---|------|-----------------------------|
| 1.5 | Handle `Expr::MacroVar` in eval_expr — lookup name in macro substitution env | Part of 1.7 |
| 1.6 | In MacroInvocation handler: build substitution map from pattern params + args | Part of 1.7 |
| 1.7 | Wire end-to-end: macro_rules! define → invoke → substitute → eval | See below |

**Verification for 1.7:**
```
// test_macro_e2e.fj
macro_rules! double { ($x:expr) => { $x * 2 } }
macro_rules! add { ($a:expr, $b:expr) => { $a + $b } }

println(double!(21))     // Expected: 42
println(add!(10, 20))    // Expected: 30
println(double!(add!(3, 4)))  // Expected: 14
```
Expected output:
```
42
30
14
```

### 1C: Tests & Example (3 tasks)

| # | Task | Verification |
|---|------|-------------|
| 1.8 | Integration test: single-arg macro | `cargo test --test context_safety_tests macro_single_arg` passes |
| 1.9 | Integration test: multi-arg macro | `cargo test --test context_safety_tests macro_multi_arg` passes |
| 1.10 | Example file | `fj run examples/macros.fj` produces correct output |

**examples/macros.fj content:**
```fajar
// User-defined macros — Fajar Lang V19
macro_rules! square { ($x:expr) => { $x * $x } }
macro_rules! max { ($a:expr, $b:expr) => { if $a > $b { $a } else { $b } } }
macro_rules! repeat_print { ($msg:expr, $n:expr) => {
    for i in 0..$n { println($msg) }
} }

println(square!(5))         // 25
println(max!(10, 20))       // 20
repeat_print!("hello", 3)   // hello (3 times)
```

**Phase 1 Gate:**
```bash
fj run examples/macros.fj   # prints 25, 20, hello, hello, hello
cargo test --test context_safety_tests macro  # all macro tests pass
cargo test --lib && cargo clippy -- -D warnings  # no regressions
```

---

## Phase 2: Pattern Match Destructuring (8 tasks, ~200 LOC)

**Goal:** `match http_get("http://example.com") { Ok(body) => println(body), Err(e) => println(e) }`

This is the **most impactful usability improvement** — without it, users can't ergonomically
use Result/Option returns from http_get, tcp_connect, ffi_call, etc.

### 2A: Parser (3 tasks)

| # | Task | Verification |
|---|------|-------------|
| 2.1 | Add `Pattern` enum to AST: `Literal`, `Ident`, `Wildcard`, `EnumDestruct { variant, binding }` | Compiles |
| 2.2 | Parse match arm patterns: `Ok(name)`, `Err(e)`, `Some(v)`, `None`, `_` | `fj dump-ast` shows EnumDestruct pattern |
| 2.3 | Preserve backward compat: existing match with literals/idents still works | Existing tests pass |

### 2B: Interpreter (3 tasks)

| # | Task | Verification |
|---|------|-------------|
| 2.4 | Match evaluator: when pattern is `EnumDestruct("Ok", "val")`, check if target is `Value::Enum { variant: "Ok", data }`, bind data to "val" | Part of 2.6 |
| 2.5 | Handle `None` pattern (no binding) and `_` wildcard | Part of 2.6 |
| 2.6 | Wire end-to-end | See below |

**Verification for 2.6:**
```
// test_pattern_match.fj
let result = http_get("http://example.com")
match result {
    Ok(body) => println(f"Got {len(body)} bytes"),
    Err(e) => println(f"Error: {e}"),
}

let maybe = map_get(map_new(), "missing")
match maybe {
    Some(v) => println(v),
    None => println("not found"),
}
```
Expected: prints byte count from example.com, then "not found"

### 2C: Tests (2 tasks)

| # | Task | Verification |
|---|------|-------------|
| 2.7 | Integration tests: Ok/Err/Some/None destructure | `cargo test --test context_safety_tests pattern_match` |
| 2.8 | Example: `examples/pattern_match.fj` | `fj run examples/pattern_match.fj` |

**Phase 2 Gate:**
```bash
fj run examples/pattern_match.fj  # correct output
cargo test --test context_safety_tests pattern  # all pass
cargo test --lib  # no regressions
```

---

## Phase 3: Real Async I/O (6 tasks, ~120 LOC)

**Goal:** `async_sleep(100)` actually waits 100ms. `async_spawn` + `async_join` for parallel HTTP.

**Scope limitation (honest):** Real parallel execution of .fj code is blocked by the interpreter
not being Send+Sync. V19 limits async to **I/O-bound operations** (HTTP, TCP, sleep) that
happen outside the interpreter. CPU-bound parallel execution deferred to V20.

### 3A: Core (3 tasks)

| # | Task | Verification |
|---|------|-------------|
| 3.1 | Wire `async_sleep(ms)` to real `tokio::time::sleep` | See below |
| 3.2 | Wire `async_spawn(fn_name, ...args)` to execute async builtin on tokio | Part of 3.4 |
| 3.3 | Wire `async_join(task_id)` to block until task completes | Part of 3.4 |

**Verification for 3.1:**
```
// test_async_sleep.fj
let start = 0  // no real timer, but sleep should block
async_sleep(200)
println("slept 200ms")
```
Expected: prints "slept 200ms" after visible pause (~200ms)

### 3B: Parallel I/O (2 tasks)

| # | Task | Verification |
|---|------|-------------|
| 3.4 | async_spawn for HTTP: spawn two http_get in parallel, join both | See below |
| 3.5 | async_timeout(ms, fn_name): timeout if operation exceeds ms | `async_timeout(50, ...)` on slow operation → Err("timeout") |

**Verification for 3.4:**
```
// test_async_parallel.fj
// Spawn two HTTP gets
let t1 = async_spawn("async_http_get", "http://example.com")
let t2 = async_spawn("async_http_get", "http://example.com")
let r1 = async_join(t1)
let r2 = async_join(t2)
println("Both done")
println(type_of(r1))
println(type_of(r2))
```
Expected: "Both done" + types printed. Should complete faster than 2 sequential gets.

### 3C: Example (1 task)

| # | Task | Verification |
|---|------|-------------|
| 3.6 | `examples/async_demo.fj` | `fj run examples/async_demo.fj` |

**Phase 3 Gate:**
```bash
fj run examples/async_demo.fj  # completes with parallel output
cargo test --lib  # no regressions
```

---

## Phase 4: E2E Integration Tests (6 tasks, ~200 LOC)

**Goal:** Comprehensive test file covering ALL V18+V19 features with real eval_source calls.

| # | Task | Verification |
|---|------|-------------|
| 4.1 | Create `tests/v19_e2e_tests.rs` with helper | Compiles |
| 4.2 | Test: user macro_rules! round-trip | `cargo test --test v19_e2e_tests macro` |
| 4.3 | Test: pattern match destructure Ok/Err/Some/None | `cargo test --test v19_e2e_tests pattern` |
| 4.4 | Test: gen fn + yield + for-in | `cargo test --test v19_e2e_tests generator` |
| 4.5 | Test: @requires precondition blocks invalid args | `cargo test --test v19_e2e_tests requires` |
| 4.6 | Test: channels round-trip | `cargo test --test v19_e2e_tests channel` |

**Each test follows this pattern:**
```rust
#[test]
fn v19_macro_double() {
    let out = eval_capture(r#"
        macro_rules! double { ($x:expr) => { $x * 2 } }
        println(double!(21))
    "#);
    assert_eq!(out.trim(), "42");
}
```

**Phase 4 Gate:**
```bash
cargo test --test v19_e2e_tests  # ALL pass
```

---

## Phase 5: Polish (4 tasks, ~100 LOC)

| # | Task | Verification |
|---|------|-------------|
| 5.1 | `fj test file.fj` — run @test functions, print summary | `fj test examples/tests.fj` → "3/3 passed" |
| 5.2 | `fj run --watch file.fj` — re-run on file change | Save file → auto re-run (uses `notify` crate or polling) |
| 5.3 | String interpolation works in all expression positions | `let x = f"hello {1+2}"` → "hello 3" |
| 5.4 | Update CLAUDE.md + memory with V19 final numbers | Docs accurate |

**Phase 5 Gate:**
```bash
fj test examples/tests.fj  # summary printed
fj run examples/macros.fj  # all examples work
cargo test --lib && cargo test --test v19_e2e_tests && cargo clippy -- -D warnings
```

---

## NOT In V19 (Honestly Deferred)

| Item | Reason | When |
|------|--------|------|
| CPU-bound parallel async | Interpreter not Send+Sync | V20 (requires Arc<Mutex> refactor) |
| Proc macros | Needs separate compilation model | V20+ |
| Pattern match with guards (`if x > 0`) | Lower priority than destructure | V20 |
| Real package registry server | Needs hosted infra | V20 |
| WASM playground | Needs wasm-pack build | V20 |

---

## Execution Order (Recommended)

```
Session 1: Phase 1 (macros) — the only V18 deferred task
Session 2: Phase 2 (pattern match) — most impactful usability fix
Session 3: Phase 3 (async) + Phase 4 (tests)
Session 4: Phase 5 (polish) + commit + push
```

Each session should end with a commit + push. Each task verified before marking [x].

---

## Final Checklist (Per Task)

- [ ] .fj program written that exercises the feature
- [ ] `fj run` produces exact expected output
- [ ] `cargo test --lib` — no regressions
- [ ] `cargo clippy -- -D warnings` — clean
- [ ] Task marked [x] only after ALL above pass

---

*V19 "Precision" Plan (Revised) — 34 tasks, 5 phases, ~870 LOC*
*Every task has a concrete .fj verification program with expected output.*
*Written with the lesson of V13-V15: no inflation, no batch-marking, no framework-only.*
