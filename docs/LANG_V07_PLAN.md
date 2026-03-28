## Option 8: Fajar Lang v0.7 (10 sprints, 100 tasks)

**Goal:** Major language improvements — async v2, pattern matching, trait objects v2, macro system
**Effort:** ~35 hours
**Codename:** Language v0.7 "Illumination"

### Phase AA: Async/Await V2 (2 sprints, 20 tasks)

#### Sprint AA1: Async Runtime (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| AA1.1 | `async fn` desugaring | Transform async fn → state machine struct | [x] |
| AA1.2 | `Future` trait | `poll(cx: &mut Context) -> Poll<T>` | [x] |
| AA1.3 | `await` expression | Yield point in state machine | [x] |
| AA1.4 | Task spawner | `spawn(future)` → add to executor queue | [x] |
| AA1.5 | Simple executor | Single-threaded poll loop | [x] |
| AA1.6 | Waker mechanism | Wake task when I/O ready | [x] |
| AA1.7 | `select!` macro | Wait for first of multiple futures | [x] |
| AA1.8 | Async channels | `async_send()` / `async_recv()` | [x] |
| AA1.9 | Async file I/O | Non-blocking read/write | [x] |
| AA1.10 | 10 integration tests | async fn, await, spawn, executor | [x] |

#### Sprint AA2: Async Ecosystem (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| AA2.1 | `async for` loops | Iterate over async stream | [x] |
| AA2.2 | Timeout | `timeout(duration, future)` | [x] |
| AA2.3 | Join | `join!(a, b, c)` — wait for all | [x] |
| AA2.4 | Async TCP client | Non-blocking TCP connect + read/write | [x] |
| AA2.5 | Async HTTP client | `http_get(url).await` | [x] |
| AA2.6 | Error propagation | `?` in async context | [x] |
| AA2.7 | Async closures | `async |x| { ... }` | [x] |
| AA2.8 | Pin safety | Ensure futures are not moved after poll | [x] |
| AA2.9 | Benchmark | Async vs sync performance comparison | [x] |
| AA2.10 | 10 integration tests | async for, timeout, join, HTTP | [x] |

### Phase BB: Pattern Matching V2 (2 sprints, 20 tasks)

#### Sprint BB1: Advanced Patterns (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| BB1.1 | Nested patterns | `match x { Some(Some(v)) => ... }` | [x] |
| BB1.2 | Guard clauses | `match x { n if n > 0 => ... }` | [x] |
| BB1.3 | Binding patterns | `match x { val @ Some(_) => use val }` | [x] |
| BB1.4 | Tuple patterns | `let (a, b, c) = tuple` | [x] |
| BB1.5 | Struct patterns | `let Point { x, y } = point` | [x] |
| BB1.6 | Slice patterns | `match arr { [first, .., last] => ... }` | [x] |
| BB1.7 | Range patterns | `match n { 1..=5 => ... }` | [x] |
| BB1.8 | Exhaustiveness check | Warn on non-exhaustive match | [x] |
| BB1.9 | `if let` expression | `if let Some(v) = opt { ... }` | [x] |
| BB1.10 | 10 integration tests | All pattern types | [x] |

#### Sprint BB2: Pattern Compilation (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| BB2.1 | Decision tree | Compile patterns to efficient if-else tree | [x] |
| BB2.2 | Redundancy check | Warn on unreachable patterns | [x] |
| BB2.3 | `while let` | `while let Some(v) = iter.next() { ... }` | [x] |
| BB2.4 | `let else` | `let Some(v) = opt else { return }` | [x] |
| BB2.5 | Or-patterns in match | `match x { 1 | 2 | 3 => ... }` | [x] |
| BB2.6 | Constant patterns | `match x { MY_CONST => ... }` | [x] |
| BB2.7 | Ref patterns | `match &x { &ref v => ... }` | [x] |
| BB2.8 | Codegen: pattern to Cranelift | Efficient code for complex patterns | [x] |
| BB2.9 | Benchmark: match vs if-else | Verify pattern match is efficient | [x] |
| BB2.10 | 10 integration tests | Decision tree, redundancy, codegen | [x] |

### Phase CC: Trait Objects V2 (2 sprints, 20 tasks)

#### Sprint CC1: Dynamic Dispatch (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| CC1.1 | `dyn Trait` with generics | `Box<dyn Iterator<Item=i64>>` | [x] |
| CC1.2 | Multi-trait objects | `dyn Read + Write` | [x] |
| CC1.3 | Object safety rules | Enforce: no Self, no generics in methods | [x] |
| CC1.4 | Vtable layout | Method pointers + drop fn + size/align | [x] |
| CC1.5 | Dynamic dispatch codegen | Cranelift indirect calls via vtable | [x] |
| CC1.6 | `impl dyn Trait` | Add methods to trait objects | [x] |
| CC1.7 | Downcasting | `dyn Any` → concrete type (with type_id) | [x] |
| CC1.8 | Trait upcasting | `dyn Derived` → `dyn Base` | [x] |
| CC1.9 | Object-safe auto-detection | Compiler determines object safety | [x] |
| CC1.10 | 10 integration tests | Vtable, dispatch, downcasting | [x] |

#### Sprint CC2: Associated Types + GATs (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| CC2.1 | Associated types | `trait Iterator { type Item; }` | [x] |
| CC2.2 | Where clauses | `fn foo<T>() where T: Display + Clone` | [x] |
| CC2.3 | GATs (basic) | `trait Lending { type Item<'a>; }` | [x] |
| CC2.4 | Impl Trait in return | `fn foo() -> impl Display` | [x] |
| CC2.5 | Trait aliases | `trait ReadWrite = Read + Write` | [x] |
| CC2.6 | Supertraits | `trait Derived: Base { ... }` | [x] |
| CC2.7 | Default type params | `trait Foo<T = i64> { ... }` | [x] |
| CC2.8 | Negative impls | `impl !Send for Foo` (marker) | [x] |
| CC2.9 | Coherence check | Orphan rules for trait implementations | [x] |
| CC2.10 | 10 integration tests | Associated types, GATs, supertraits | [x] |

### Phase DD: Macro System (2 sprints, 20 tasks)

#### Sprint DD1: Declarative Macros (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| DD1.1 | `macro_rules!` syntax | Pattern → expansion template | [x] |
| DD1.2 | Token tree matching | `$ident:ident`, `$expr:expr`, `$ty:ty` | [x] |
| DD1.3 | Repetition | `$($x:expr),*` → zero or more | [x] |
| DD1.4 | Macro expansion | Replace tokens in template with matched | [x] |
| DD1.5 | Hygiene (basic) | Macro-generated names don't leak | [x] |
| DD1.6 | `vec![]` macro | `vec![1, 2, 3]` → array construction | [x] |
| DD1.7 | `println!` macro | `println!("x = {}", x)` | [x] |
| DD1.8 | `assert!` macro | `assert!(condition, "message")` | [x] |
| DD1.9 | Nested macros | Macro calling macro | [x] |
| DD1.10 | 10 integration tests | macro_rules, repetition, hygiene | [x] |

#### Sprint DD2: Proc Macros (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| DD2.1 | Derive macros | `#[derive(Debug, Clone)]` | [x] |
| DD2.2 | Attribute macros | `#[test]`, `#[bench]` | [x] |
| DD2.3 | Function-like macros | `sql!(SELECT * FROM users)` | [x] |
| DD2.4 | TokenStream API | Parse + construct token streams | [x] |
| DD2.5 | `derive(Debug)` | Auto-generate debug formatting | [x] |
| DD2.6 | `derive(Clone)` | Auto-generate field-wise clone | [x] |
| DD2.7 | `derive(PartialEq)` | Auto-generate equality comparison | [x] |
| DD2.8 | Custom derive | User-defined derive macros | [x] |
| DD2.9 | Macro error reporting | Clear errors for macro expansion failures | [x] |
| DD2.10 | 10 integration tests | Derive, attribute, function macros | [x] |

---
