# Fajar Lang Tutorial

A hands-on guide to writing real programs in Fajar Lang (`fj`), a
statically-typed systems language for embedded ML + OS integration.

This tutorial assumes:
- You have `fj` installed (build from source: `cargo build --release`,
  binary at `target/release/fj`).
- Basic familiarity with at least one typed language (Rust, TypeScript,
  Swift, Kotlin).

It does NOT assume:
- ML/AI background (Chapter 8 introduces tensors from scratch).
- OS internals knowledge (Chapter 9 builds up gradually).

After 10 chapters you will have written: a calculator REPL, a CSV→JSON
tool, a stateful TCP server, a tiny tensor inference loop, and a
context-isolated kernel module.

---

## Table of contents

| # | Chapter | What you build | New concepts |
|---|---|---|---|
| 1 | [Hello, Fajar Lang](#chapter-1--hello-fajar-lang) | "Hello, world" + a calculator | `let`, `fn`, `if`, REPL workflow |
| 2 | [Types and patterns](#chapter-2--types-and-patterns) | Shape area calculator | Structs, enums, `match`, exhaustiveness |
| 3 | [Errors as values](#chapter-3--errors-as-values) | Safe number parser | `Option`, `Result`, `?` operator |
| 4 | [Ownership and borrowing](#chapter-4--ownership-and-borrowing) | Linked list | Move semantics, `&T` / `&mut T`, polonius |
| 5 | [Generics and traits](#chapter-5--generics-and-traits) | Generic stack | `<T>`, trait bounds, monomorphization |
| 6 | [Iterators and pipelines](#chapter-6--iterators-and-pipelines) | CSV→JSON pipeline | `map`, `filter`, `fold`, `\|>` operator |
| 7 | [Async and effects](#chapter-7--async-and-effects) | TCP echo server | `async`, `.await`, effect rows |
| 8 | [Tensors and ML](#chapter-8--tensors-and-ml) | MNIST inference | `Tensor<T>`, `@device`, autograd |
| 9 | [Kernel context and `@kernel`](#chapter-9--kernel-context-and-kernel) | Tiny kernel module | `@kernel`, memory primitives, KE codes |
| 10 | [Putting it together: a robot](#chapter-10--putting-it-together-a-robot) | Sensor → ML → action loop | Cross-context bridges |

Appendices link to:
- [Error code reference](ERROR_CODES.md)
- [Standard library spec](STDLIB_SPEC.md)
- [Language grammar](GRAMMAR_REFERENCE.md)

---

## Chapter 1 — Hello, Fajar Lang

The smallest valid program:

```fajar
fn main() {
    println("Hello, Fajar Lang!")
}
```

Save as `hello.fj` and run:

```bash
fj run hello.fj
```

`fj run` lexes, parses, type-checks, and evaluates. For a quick REPL,
type `fj repl` and try expressions.

### A calculator that evaluates one line

```fajar
fn main() {
    let a = 6
    let b = 7
    println(f"answer = {a * b}")
}
```

Note:
- Variables are immutable by default (`let`). For mutation use `let mut`.
- `f"…"` is an interpolated f-string (Chapter 6 has more examples).
- `i64` is the default integer type; you can be explicit:
  `let a: i64 = 6`.

**Exercises:**
1. Add a `square(n: i64) -> i64` function and call it from `main`.
2. Read user input. Hint: `let line = read_line()`.

---

## Chapter 2 — Types and patterns

Fajar Lang has structs and tagged enums (sum types).

```fajar
struct Point {
    x: f64,
    y: f64,
}

enum Shape {
    Circle(f64),
    Rect(f64, f64),
    Triangle(f64, f64, f64),
}

fn area(s: Shape) -> f64 {
    match s {
        Shape::Circle(r) => 3.141592653589793 * r * r,
        Shape::Rect(w, h) => w * h,
        Shape::Triangle(a, b, c) => {
            let s = (a + b + c) / 2.0
            (s * (s - a) * (s - b) * (s - c)).sqrt()
        }
    }
}
```

`match` is **exhaustive** — the compiler emits `SE011` if a variant is
not handled. This catches forgotten cases at compile time, not in
production.

```fajar
// SE011 if the `Triangle` arm is missing.
match s {
    Shape::Circle(r) => 0.0,
    Shape::Rect(_, _) => 0.0,
}
```

---

## Chapter 3 — Errors as values

There is **no exception throwing** in Fajar Lang. Failure paths are
expressed as values via `Option<T>` and `Result<T, E>`.

```fajar
fn parse_port(s: str) -> Result<u16, str> {
    match parse_int(s) {
        Ok(n) if n >= 0 && n <= 65535 => Ok(n as u16),
        Ok(_) => Err("port out of range"),
        Err(_) => Err("not a number"),
    }
}
```

The `?` operator propagates the error case:

```fajar
fn connect(host: str, port_str: str) -> Result<Conn, str> {
    let port = parse_port(port_str)?
    let conn = open_tcp(host, port)?
    Ok(conn)
}
```

`?` desugars to `match … { Ok(v) => v, Err(e) => return Err(e) }`.

---

## Chapter 4 — Ownership and borrowing

Fajar Lang has **ownership lite** — Rust-style move semantics without
explicit lifetime annotations. The polonius borrow checker enforces:

- Moving a value invalidates the source binding.
- You can have many `&T` shared borrows OR one `&mut T` exclusive
  borrow at a time.
- A reference cannot outlive the value it borrows.

```fajar
fn main() {
    let s1 = String::from("hello")
    let s2 = s1                    // move
    // println(s1)                 // ME001 use-after-move
    println(s2)                    // OK
}
```

Borrow:

```fajar
fn main() {
    let mut v = [1, 2, 3]
    let r = &v[0]                  // shared borrow
    // v[0] = 99                   // ME004 cannot mutate while borrowed
    println(*r)
}
```

The error catalog in `docs/ERROR_CODES.md` §8 lists every borrow-rule
diagnostic (`ME001`–`ME013`).

---

## Chapter 5 — Generics and traits

Functions and types can be generic. Trait bounds constrain what
operations the generic must support.

```fajar
trait Cmp {
    fn lt(self, other: Self) -> bool
}

fn min<T: Cmp>(a: T, b: T) -> T {
    if a.lt(b) { a } else { b }
}

impl Cmp for i64 {
    fn lt(self, other: i64) -> bool { self < other }
}

fn main() {
    println(f"min = {min(5, 3)}")  // 3
}
```

Generics monomorphize: each `T` substitution produces specialized
machine code. No runtime dispatch overhead, full inlining. Errors at
specialization fire as `SE014` (unsatisfied trait bound).

---

## Chapter 6 — Iterators and pipelines

Standard combinators on iterators feel familiar from JS/Python:

```fajar
fn sum_squares(xs: [i64]) -> i64 {
    xs |> map(|x| x * x) |> sum()
}
```

The `|>` pipeline operator threads the left-hand value as the first
argument of the right-hand call. Equivalent to: `sum(map(xs, |x| x*x))`.

A CSV→JSON pipeline:

```fajar
fn csv_to_json(input: str) -> str {
    input
        |> lines()
        |> map(|line| line |> split(","))
        |> filter(|row| len(row) > 0)
        |> enumerate()
        |> map(|pair| format_row(pair))
        |> collect_with(",", "[", "]")
}
```

See `examples/cli_tools/csv_to_json.fj` for the full version.

---

## Chapter 7 — Async and effects

Fajar Lang has **algebraic effects** in addition to async/await. The
effect system tracks side effects in function signatures so that
purity violations are caught at compile time.

```fajar
async fn fetch_status(url: str) -> Result<i64, str> {
    let conn = open_tcp_async("api.example.com", 443).await?
    conn.send(format("GET {} HTTP/1.1\r\n\r\n", url)).await?
    let response = conn.recv_until("\r\n").await?
    parse_status_code(response)
}
```

Effect annotations:

```fajar
fn pure_calc(x: i64) -> i64 with Pure { x * x + 1 }
fn read_log() -> str with IO { read_file("/var/log/app.log") }
```

Calling an `IO` function from a `Pure` function fires `EE007` (purity
violation). See `docs/ERROR_CODES.md` §11 for the full effect-error
catalog.

---

## Chapter 8 — Tensors and ML

Tensors are **first-class** in the type system, with shape checked at
compile time when shapes are known constants.

```fajar
@device
fn relu_layer(x: Tensor<f32, [128]>) -> Tensor<f32, [128]> {
    relu(x)
}

@device
fn forward(input: Tensor<f32, [784]>) -> Tensor<f32, [10]> {
    input
        |> linear::<784, 128>()
        |> relu_layer()
        |> linear::<128, 10>()
        |> softmax()
}
```

The `@device` annotation gates this code into the **device context**
where tensor ops are allowed but raw pointers and `@kernel` syscalls
are forbidden (`DE001`, `DE002`). Compile-time shape mismatches fire
as `TE002` / `TE003`.

For full-pipeline MNIST inference (load weights, infer, evaluate),
see `docs/tutorials/mnist.md`.

---

## Chapter 9 — Kernel context and `@kernel`

The `@kernel` annotation marks code that runs without a heap allocator
or tensor backend — typically interrupt handlers, page-fault handlers,
and bootstrap code.

```fajar
@kernel
fn handle_timer_interrupt(ctx: *mut SavedRegs) {
    // No heap (KE001), no tensors (KE002).
    let count = mmio_read(0xfee00000)
    mmio_write(0xfee000b0, 0)
}
```

`@kernel` and `@device` are mutually exclusive (`KE003` / `DE002`).
`@safe` (the default) cannot reach hardware (`SE020`).

Cross-context bridge pattern:

```fajar
@kernel fn read_sensor() -> [f32; 4] { /* MMIO */ }
@device fn infer(x: Tensor<f32, [4]>) -> Tensor<f32, [3]> { /* model */ }

@safe fn control_loop() -> Action {
    let raw = read_sensor()                    // syscall ABI
    let result = infer(Tensor::from_slice(raw))  // IPC ABI
    Action::from_prediction(result)
}
```

See `docs/tutorials/os_development.md` for a from-scratch FajarOS
kernel walkthrough.

---

## Chapter 10 — Putting it together: a robot

Combine all 9 prior chapters into a sensor → infer → actuate loop:

```fajar
@kernel fn imu_read() -> [f32; 6] { /* hardware MMIO */ }

@device fn balance_model(state: Tensor<f32, [6]>) -> Tensor<f32, [2]> {
    state
        |> linear::<6, 16>()
        |> relu()
        |> linear::<16, 2>()
        |> tanh()
}

@safe
async fn loop_body() -> Result<(), str> {
    let imu = imu_read()
    let action = balance_model(Tensor::from_slice(imu))
    let (left, right) = (action[0], action[1])
    motor_set(0, left).await?
    motor_set(1, right).await?
    Ok(())
}

@safe
async fn run() {
    loop {
        match loop_body().await {
            Ok(_) => continue,
            Err(e) => {
                log(f"loop error: {e}")
                break
            }
        }
        sleep_ms(10).await
    }
}
```

Annotations enforce isolation: `imu_read` cannot call `balance_model`
(KE003), `balance_model` cannot call `imu_read` (DE002), `run` calls
both via `@safe` ABI bridges. The compiler verifies isolation at every
edge.

---

## Where to go next

- **Examples:** browse `examples/` — `cli_tools/` for everyday utilities,
  `recipes/` for deeper patterns, `nova/` for a full kernel.
- **Spec:** `docs/FAJAR_LANG_SPEC.md` is the authoritative language
  spec.
- **Error catalog:** `docs/ERROR_CODES.md` lists every diagnostic with
  trigger snippets.
- **Stdlib:** `docs/STDLIB_SPEC.md` documents every public function in
  `std::*` and `nn::*`.
- **Architecture:** `docs/ARCHITECTURE.md` describes the compiler
  pipeline and module dependency graph.
- **Domain tutorials:** `docs/tutorials/embedded_ml.md`,
  `docs/tutorials/mnist.md`, `docs/tutorials/os_development.md`.

---

*Tutorial v1.0 — 2026-05-03. P6.E3 of FAJAR_LANG_PERFECTION_PLAN. 10
chapters covering basics → advanced. Iteratively expanded as new
examples land in `examples/`.*
