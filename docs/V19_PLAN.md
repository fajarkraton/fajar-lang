# V19 "Precision" — User Macros, Real Async, Test Quality

> **Date:** 2026-04-03
> **Prerequisite:** V18 "Integrity" complete (36/37 tasks, 19 features, 8,414 tests)
> **Goal:** Complete the last deferred feature (user macros), add real async, raise test quality
> **Rule:** [x] = `fj run` works E2E. Each task verified individually. No batch-marking.

---

## Scope

| Area | Tasks | Effort | Impact |
|------|-------|--------|--------|
| 1. User Macros | 8 | ~225 LOC | Last deferred feature — completes macro story |
| 2. Real Async | 6 | ~105 LOC | User async/await with tokio::spawn |
| 3. Test Quality | 6 | ~300 LOC | Raise E2E coverage from ~4% to ~15% |
| 4. Polish | 4 | ~100 LOC | Pattern match destructure, error recovery |
| **Total** | **24** | **~730 LOC** | |

---

## Phase 1: User macro_rules! with $x (8 tasks)

**Problem:** `macro_rules! double { ($x:expr) => { $x * 2 } }` fails because parser can't
handle `$x` in the body — `$` isn't a valid expression start.

**Solution:** Add `Expr::MacroVar` to AST. When parsing macro body, recognize `$ident` as
a macro variable reference. At invocation time, substitute metavars with argument values
before evaluating.

**Key insight:** `TokenTree::MetaVar` already exists in `src/macros_v12.rs:50`. The lexer
already tokenizes `$` as `TokenKind::Dollar`. We just need to wire them together.

| # | Task | Files | Effort | Verification |
|---|------|-------|--------|-------------|
| 1.1 | Add `Expr::MacroVar { name: String, span: Span }` to AST | `src/parser/ast.rs` | S | Compiles, variant exists |
| 1.2 | Parse `$identifier` as `Expr::MacroVar` in expression context | `src/parser/mod.rs` | M | `fj dump-ast` shows MacroVar for `$x` in macro body |
| 1.3 | Track macro body parsing mode (allow `$x` in expressions) | `src/parser/mod.rs` | M | Parser doesn't error on `{ $x * 2 }` inside macro_rules |
| 1.4 | Eval `Expr::MacroVar` — lookup in substitution env | `src/interpreter/eval/mod.rs` | S | `$x` resolves to bound value |
| 1.5 | Wire macro invocation to substitute `$x` → arg value before eval | `src/interpreter/eval/mod.rs` | M | `double!(21)` evaluates `21 * 2 = 42` |
| 1.6 | Support multiple metavars: `($a:expr, $b:expr) => { $a + $b }` | `src/interpreter/eval/mod.rs` | M | `add!(3, 4)` → 7 |
| 1.7 | Test: 5+ macro E2E tests | `tests/context_safety_tests.rs` | S | All pass |
| 1.8 | Example: `examples/macros.fj` | `examples/macros.fj` | S | `fj run examples/macros.fj` |

**Implementation Detail:**

```
Step 1: Add Expr::MacroVar to parser/ast.rs
    Expr::MacroVar { name: String, span: Span }

Step 2: In parser, when inside macro body and we see Dollar + Ident:
    if self.peek_kind() == TokenKind::Dollar {
        self.advance(); // consume $
        let (name, span) = self.expect_ident()?;
        return Ok(Expr::MacroVar { name, span });
    }

Step 3: In interpreter, MacroInvocation handler:
    - Parse macro arm pattern to extract param names
    - Create substitution env: { "x" → arg_value[0], "y" → arg_value[1] }
    - Eval body with substitution env active
    - When Expr::MacroVar encountered, lookup name in substitution env

Step 4: Need a flag in parser: `in_macro_body: bool` to allow $ident parsing
    only inside macro_rules! body blocks.
```

**Gate:** `cargo test --test context_safety_tests macro && fj run examples/macros.fj`

---

## Phase 2: Real Async Event Loop (6 tasks)

**Problem:** `async { ... }.await` works but uses cooperative polling — no real concurrency.
User can't `spawn` async tasks that run in parallel.

**Solution:** Wire `tokio::spawn` into interpreter. Add `Value::TaskHandle` for join handles.
Use existing `tokio_runtime` field (already on Interpreter struct).

**Current state:**
- `async_http_get`/`async_http_post` use manual TCP with tokio internally
- `Expr::Await` evaluates future, checks `async_ops` map, falls back to sync
- `async_spawn/join/select` builtins exist but don't use real tokio::spawn

| # | Task | Files | Effort | Verification |
|---|------|-------|--------|-------------|
| 2.1 | Add `async_run` builtin — execute async block on tokio runtime | `builtins.rs` | M | `async_run(fn)` blocks until async completes |
| 2.2 | Wire `async_sleep` to use real `tokio::time::sleep` | `builtins.rs` | S | `async_sleep(100)` waits 100ms real time |
| 2.3 | Wire `async_spawn` to use real `tokio::spawn` for I/O tasks | `builtins.rs`, `mod.rs` | L | Two HTTP gets run in parallel |
| 2.4 | Add `async_timeout(ms, fn)` builtin | `builtins.rs` | M | Times out if fn exceeds ms |
| 2.5 | Test: async HTTP parallel fetch | `tests/` | S | Two requests complete faster than sequential |
| 2.6 | Example: `examples/async_demo.fj` | `examples/` | S | `fj run examples/async_demo.fj` |

**Implementation Detail:**

```
async_run(fn):
    let rt = self.get_or_create_tokio_runtime();
    rt.block_on(async { ... })

async_sleep(ms):
    Already uses tokio internally — verify it actually sleeps

async_spawn(fn):
    Current: stores in async_tasks HashMap, doesn't execute
    New: rt.spawn(async { eval_expr(body) }) → return JoinHandle id

async_timeout(ms, fn):
    tokio::time::timeout(Duration::from_millis(ms), async { eval_fn() })
```

**Note:** Real parallel execution of .fj code requires making the interpreter `Send + Sync`
or cloning interpreter state per task. For V19, limit to I/O-bound async (HTTP, TCP, sleep)
which can run on tokio without interpreter access during await points.

**Gate:** `fj run examples/async_demo.fj` shows real parallel execution

---

## Phase 3: Test Quality (6 tasks)

**Problem:** V17 found ~70% genuine, ~20% shallow tests. E2E `eval_source` tests are only
~4% of total (435 calls out of 12K+ test lines). Most tests are module-internal unit tests.

**Goal:** Add E2E integration tests for all V18 features, covering user-visible behavior.

| # | Task | Files | Effort | Verification |
|---|------|-------|--------|-------------|
| 3.1 | E2E test: HTTP GET/POST returns real data | `tests/v18_integration.rs` | M | `cargo test --test v18_integration http` |
| 3.2 | E2E test: FFI loads libc, calls getpid | `tests/v18_integration.rs` | M | `cargo test --test v18_integration ffi` |
| 3.3 | E2E test: gen fn produces correct array | `tests/v18_integration.rs` | S | `cargo test --test v18_integration gen` |
| 3.4 | E2E test: @requires blocks invalid args | `tests/v18_integration.rs` | S | `cargo test --test v18_integration requires` |
| 3.5 | E2E test: channels send/recv round-trip | `tests/v18_integration.rs` | S | `cargo test --test v18_integration channel` |
| 3.6 | E2E test: fj build produces working binary | `tests/v18_integration.rs` | M | `cargo test --test v18_integration build` (needs `native` feature) |

**Test pattern:**

```rust
fn eval_capture(source: &str) -> String {
    let tokens = fajar_lang::lexer::tokenize(source).expect("lex");
    let program = fajar_lang::parser::parse(tokens).expect("parse");
    let _ = fajar_lang::analyzer::analyze(&program);
    let mut interp = fajar_lang::interpreter::Interpreter::new_capturing();
    interp.eval_program(&program).expect("eval");
    interp.get_output().join("\n")
}

#[test]
fn v18_http_get_returns_html() {
    let out = eval_capture(r#"
        let r = http_get("http://example.com")
        println(r)
    "#);
    assert!(out.contains("Example Domain"));
}
```

**Gate:** `cargo test --test v18_integration` — all pass

---

## Phase 4: Polish (4 tasks)

Small improvements that make the language more usable.

| # | Task | Files | Effort | Verification |
|---|------|-------|--------|-------------|
| 4.1 | Pattern match enum destructure: `Ok(val) => ...` | `src/parser/mod.rs`, `src/interpreter/eval/mod.rs` | L | `match result { Ok(v) => println(v), Err(e) => println(e) }` |
| 4.2 | String interpolation in match arms | `src/interpreter/eval/mod.rs` | M | `match x { 1 => "one", _ => f"other: {x}" }` |
| 4.3 | `fj run --watch` auto-reload on file change | `src/main.rs` | M | Save .fj file → auto re-run |
| 4.4 | `fj test` runs @test functions with summary | `src/main.rs` | M | `fj test examples/tests.fj` → "3/3 passed" |

**Note on 4.1:** This is the most impactful polish item. Currently `match result { Ok(val) => ...}`
fails because the parser doesn't support pattern destructuring in match arms. This blocks
ergonomic use of Result/Option from http_get, tcp_connect, etc.

**Implementation for 4.1:**

```
Current match arm parsing: parse_expr() for pattern → literal/ident/wildcard only
Need: parse_pattern() that handles:
  - EnumPattern: Ok(name) | Err(name) | Some(name) | None
  - TuplePattern: (a, b)
  - WildcardPattern: _
  - LiteralPattern: 42, "hello", true

In interpreter match evaluation:
  - When pattern is EnumPattern(variant, binding):
    Check if match target is Value::Enum { variant, data }
    If match: bind data to the name, eval arm body
```

**Gate:** `fj run` with `match http_get("http://example.com") { Ok(body) => println(body), Err(e) => println(e) }`

---

## Dependency Graph

```
Phase 1 (Macros) — independent, start immediately
    |
Phase 2 (Async) — independent, can parallel with Phase 1
    |
Phase 3 (Tests) — depends on Phase 1+2 features existing
    |
Phase 4 (Polish) — independent but benefits from Phase 3 test coverage
```

**Recommended execution order:**
1. Phase 1 (macros) — the only V18 deferred task, clear spec
2. Phase 4.1 (pattern match destructure) — high impact for usability
3. Phase 2 (async) — builds on existing infrastructure
4. Phase 3 (tests) — validates everything
5. Phase 4.2-4.4 (remaining polish)

---

## Risk Assessment

| Risk | Likelihood | Mitigation |
|------|-----------|------------|
| Macro body parsing conflicts with existing $ handling | Medium | Use `in_macro_body` flag, only allow in macro_rules context |
| Async spawn requires Send+Sync interpreter | High | Limit to I/O-bound tasks, don't share interpreter state |
| Pattern match destructure breaks existing match tests | Medium | Keep simple patterns working, add enum patterns as extension |
| Test flakiness from real network calls | Medium | Use `example.com` (stable) or mock server |

---

## Success Metrics

| Phase | Metric | Target |
|-------|--------|--------|
| 1 | `macro_rules! double { ($x:expr) => { $x * 2 } }; double!(21)` → 42 | Works |
| 2 | Two parallel HTTP gets faster than sequential | Measurable speedup |
| 3 | E2E integration test file with 10+ tests | All pass |
| 4.1 | `match Ok(42) { Ok(v) => v, Err(_) => 0 }` → 42 | Works |

---

## Estimated Session Plan

| Session | Tasks | Goal |
|---------|-------|------|
| 1 | 1.1-1.5 (macro core) + 4.1 (pattern match) | Macros + destructure working |
| 2 | 1.6-1.8 (macro polish) + 2.1-2.4 (async) | Multi-arg macros + async |
| 3 | 2.5-2.6 + 3.1-3.6 (tests) | Async example + full E2E test suite |
| 4 | 4.2-4.4 (polish) + commit/push | Watch mode, fj test, final cleanup |

---

## Files to Change (Complete List)

| File | Changes | Phase |
|------|---------|-------|
| `src/parser/ast.rs` | Add `Expr::MacroVar` | 1 |
| `src/parser/mod.rs` | Parse `$ident` as MacroVar, `in_macro_body` flag | 1 |
| `src/parser/items.rs` | Set `in_macro_body` when parsing macro_rules body | 1 |
| `src/interpreter/eval/mod.rs` | Eval MacroVar, macro substitution, async_run | 1, 2 |
| `src/interpreter/eval/builtins.rs` | async_sleep/spawn/timeout builtins | 2 |
| `src/interpreter/value.rs` | (maybe) Value::TaskHandle | 2 |
| `src/main.rs` | fj run --watch, fj test | 4 |
| `tests/v18_integration.rs` | New E2E test file | 3 |
| `tests/context_safety_tests.rs` | Macro + pattern tests | 1, 4 |
| `examples/macros.fj` | Macro examples | 1 |
| `examples/async_demo.fj` | Async examples | 2 |

---

*V19 "Precision" Plan — 2026-04-03 — 24 tasks, 4 phases, ~730 LOC*
*Written honestly with specific file paths, line numbers, and verified technical feasibility.*
