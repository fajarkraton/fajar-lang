## Option 8: Fajar Lang v0.7 (10 sprints, 100 tasks)

**Goal:** Major language improvements — async v2, pattern matching, trait objects v2, macro system
**Effort:** ~35 hours
**Codename:** Language v0.7 "Illumination"

### Phase AA: Async/Await V2 (2 sprints, 20 tasks)

#### Sprint AA1: Async Runtime (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| AA1.1 | `async fn` desugaring | Transform async fn → state machine struct | [ ] |
| AA1.2 | `Future` trait | `poll(cx: &mut Context) -> Poll<T>` | [ ] |
| AA1.3 | `await` expression | Yield point in state machine | [ ] |
| AA1.4 | Task spawner | `spawn(future)` → add to executor queue | [ ] |
| AA1.5 | Simple executor | Single-threaded poll loop | [ ] |
| AA1.6 | Waker mechanism | Wake task when I/O ready | [ ] |
| AA1.7 | `select!` macro | Wait for first of multiple futures | [ ] |
| AA1.8 | Async channels | `async_send()` / `async_recv()` | [ ] |
| AA1.9 | Async file I/O | Non-blocking read/write | [ ] |
| AA1.10 | 10 integration tests | async fn, await, spawn, executor | [ ] |

#### Sprint AA2: Async Ecosystem (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| AA2.1 | `async for` loops | Iterate over async stream | [ ] |
| AA2.2 | Timeout | `timeout(duration, future)` | [ ] |
| AA2.3 | Join | `join!(a, b, c)` — wait for all | [ ] |
| AA2.4 | Async TCP client | Non-blocking TCP connect + read/write | [ ] |
| AA2.5 | Async HTTP client | `http_get(url).await` | [ ] |
| AA2.6 | Error propagation | `?` in async context | [ ] |
| AA2.7 | Async closures | `async |x| { ... }` | [ ] |
| AA2.8 | Pin safety | Ensure futures are not moved after poll | [ ] |
| AA2.9 | Benchmark | Async vs sync performance comparison | [ ] |
| AA2.10 | 10 integration tests | async for, timeout, join, HTTP | [ ] |

### Phase BB: Pattern Matching V2 (2 sprints, 20 tasks)

#### Sprint BB1: Advanced Patterns (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| BB1.1 | Nested patterns | `match x { Some(Some(v)) => ... }` | [ ] |
| BB1.2 | Guard clauses | `match x { n if n > 0 => ... }` | [ ] |
| BB1.3 | Binding patterns | `match x { val @ Some(_) => use val }` | [ ] |
| BB1.4 | Tuple patterns | `let (a, b, c) = tuple` | [ ] |
| BB1.5 | Struct patterns | `let Point { x, y } = point` | [ ] |
| BB1.6 | Slice patterns | `match arr { [first, .., last] => ... }` | [ ] |
| BB1.7 | Range patterns | `match n { 1..=5 => ... }` | [ ] |
| BB1.8 | Exhaustiveness check | Warn on non-exhaustive match | [ ] |
| BB1.9 | `if let` expression | `if let Some(v) = opt { ... }` | [ ] |
| BB1.10 | 10 integration tests | All pattern types | [ ] |

#### Sprint BB2: Pattern Compilation (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| BB2.1 | Decision tree | Compile patterns to efficient if-else tree | [ ] |
| BB2.2 | Redundancy check | Warn on unreachable patterns | [ ] |
| BB2.3 | `while let` | `while let Some(v) = iter.next() { ... }` | [ ] |
| BB2.4 | `let else` | `let Some(v) = opt else { return }` | [ ] |
| BB2.5 | Or-patterns in match | `match x { 1 | 2 | 3 => ... }` | [ ] |
| BB2.6 | Constant patterns | `match x { MY_CONST => ... }` | [ ] |
| BB2.7 | Ref patterns | `match &x { &ref v => ... }` | [ ] |
| BB2.8 | Codegen: pattern to Cranelift | Efficient code for complex patterns | [ ] |
| BB2.9 | Benchmark: match vs if-else | Verify pattern match is efficient | [ ] |
| BB2.10 | 10 integration tests | Decision tree, redundancy, codegen | [ ] |

### Phase CC: Trait Objects V2 (2 sprints, 20 tasks)

#### Sprint CC1: Dynamic Dispatch (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| CC1.1 | `dyn Trait` with generics | `Box<dyn Iterator<Item=i64>>` | [ ] |
| CC1.2 | Multi-trait objects | `dyn Read + Write` | [ ] |
| CC1.3 | Object safety rules | Enforce: no Self, no generics in methods | [ ] |
| CC1.4 | Vtable layout | Method pointers + drop fn + size/align | [ ] |
| CC1.5 | Dynamic dispatch codegen | Cranelift indirect calls via vtable | [ ] |
| CC1.6 | `impl dyn Trait` | Add methods to trait objects | [ ] |
| CC1.7 | Downcasting | `dyn Any` → concrete type (with type_id) | [ ] |
| CC1.8 | Trait upcasting | `dyn Derived` → `dyn Base` | [ ] |
| CC1.9 | Object-safe auto-detection | Compiler determines object safety | [ ] |
| CC1.10 | 10 integration tests | Vtable, dispatch, downcasting | [ ] |

#### Sprint CC2: Associated Types + GATs (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| CC2.1 | Associated types | `trait Iterator { type Item; }` | [ ] |
| CC2.2 | Where clauses | `fn foo<T>() where T: Display + Clone` | [ ] |
| CC2.3 | GATs (basic) | `trait Lending { type Item<'a>; }` | [ ] |
| CC2.4 | Impl Trait in return | `fn foo() -> impl Display` | [ ] |
| CC2.5 | Trait aliases | `trait ReadWrite = Read + Write` | [ ] |
| CC2.6 | Supertraits | `trait Derived: Base { ... }` | [ ] |
| CC2.7 | Default type params | `trait Foo<T = i64> { ... }` | [ ] |
| CC2.8 | Negative impls | `impl !Send for Foo` (marker) | [ ] |
| CC2.9 | Coherence check | Orphan rules for trait implementations | [ ] |
| CC2.10 | 10 integration tests | Associated types, GATs, supertraits | [ ] |

### Phase DD: Macro System (2 sprints, 20 tasks)

#### Sprint DD1: Declarative Macros (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| DD1.1 | `macro_rules!` syntax | Pattern → expansion template | [ ] |
| DD1.2 | Token tree matching | `$ident:ident`, `$expr:expr`, `$ty:ty` | [ ] |
| DD1.3 | Repetition | `$($x:expr),*` → zero or more | [ ] |
| DD1.4 | Macro expansion | Replace tokens in template with matched | [ ] |
| DD1.5 | Hygiene (basic) | Macro-generated names don't leak | [ ] |
| DD1.6 | `vec![]` macro | `vec![1, 2, 3]` → array construction | [ ] |
| DD1.7 | `println!` macro | `println!("x = {}", x)` | [ ] |
| DD1.8 | `assert!` macro | `assert!(condition, "message")` | [ ] |
| DD1.9 | Nested macros | Macro calling macro | [ ] |
| DD1.10 | 10 integration tests | macro_rules, repetition, hygiene | [ ] |

#### Sprint DD2: Proc Macros (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| DD2.1 | Derive macros | `#[derive(Debug, Clone)]` | [ ] |
| DD2.2 | Attribute macros | `#[test]`, `#[bench]` | [ ] |
| DD2.3 | Function-like macros | `sql!(SELECT * FROM users)` | [ ] |
| DD2.4 | TokenStream API | Parse + construct token streams | [ ] |
| DD2.5 | `derive(Debug)` | Auto-generate debug formatting | [ ] |
| DD2.6 | `derive(Clone)` | Auto-generate field-wise clone | [ ] |
| DD2.7 | `derive(PartialEq)` | Auto-generate equality comparison | [ ] |
| DD2.8 | Custom derive | User-defined derive macros | [ ] |
| DD2.9 | Macro error reporting | Clear errors for macro expansion failures | [ ] |
| DD2.10 | 10 integration tests | Derive, attribute, function macros | [ ] |

---
