# Effect System

Fajar Lang has a formal algebraic effect system that integrates with the context annotation system (`@kernel`, `@device`, `@safe`, `@unsafe`). Effects let you declare, track, and handle side effects at the type level.

## Why Effects?

Traditional languages mix side effects implicitly. Fajar Lang makes effects explicit:
- **The compiler enforces** that `@kernel` code can't perform tensor operations
- **The compiler enforces** that `@device` code can't access hardware registers
- **Pure functions** have no effects — guaranteed by the type system

## Declaring Effects

```fajar
effect Console {
    fn log(msg: str) -> void
    fn read_line() -> str
}

effect FileSystem {
    fn read(path: str) -> str
    fn write(path: str, data: str) -> void
    fn exists(path: str) -> bool
}
```

## Using Effects with `with` Clause

Functions declare their effects using the `with` clause:

```fajar
fn greet() with IO {
    println("Hello, world!")
}

fn read_sensor() -> i64 with Hardware, IO {
    // Can use both hardware and I/O operations
    volatile_read(0x40001000)
}

fn pure_add(a: i64, b: i64) -> i64 {
    // No `with` clause = pure function (no effects)
    a + b
}
```

## Built-in Effects

| Effect | Domain | Description |
|--------|--------|-------------|
| `IO` | I/O | Console, file, network operations |
| `Alloc` | Memory | Heap allocation |
| `Hardware` | OS | Register access, IRQ, port I/O |
| `Tensor` | ML | Tensor/matrix operations |
| `Panic` | Control | Panic/abort |
| `Async` | Concurrency | Async/await operations |
| `State` | Mutation | Mutable state access |
| `Exception` | Error | Exception throwing |

## Context-Effect Mapping

Effects are enforced by context annotations:

| Context | Allowed Effects | Forbidden |
|---------|----------------|-----------|
| `@kernel` | Hardware, IO, State, Panic | Alloc, Tensor |
| `@device` | Tensor, Alloc, IO, Panic | Hardware |
| `@safe` | Panic only | IO, Alloc, Hardware, Tensor |
| `@unsafe` | All | None |

```fajar
@kernel fn read_hw() with Hardware {
    // OK: Hardware allowed in @kernel
    0
}

@kernel fn bad() with Alloc {
    // ERROR EE006: Alloc forbidden in @kernel
}

@device fn inference() with Tensor {
    // OK: Tensor allowed in @device
    0
}
```

## Handle Expressions

Handle expressions intercept effect operations:

```fajar
effect Logger {
    fn log(msg: str) -> void
}

fn main() {
    let result = handle {
        // body runs with Logger effects
        42
    } with {
        Logger::log(msg) => {
            // Handle the log operation
            println(msg)
            resume(0)
        }
    }
}
```

## Custom Effects

Define your own effects for domain-specific side effects:

```fajar
effect Database {
    fn query(sql: str) -> str
    fn execute(sql: str) -> i64
}

fn db_operation() with Database {
    // Caller must provide a Database handler
    0
}
```

## Effect Errors

| Code | Error | Description |
|------|-------|-------------|
| EE001 | UndeclaredEffect | Function performs effect not in `with` clause |
| EE002 | UnknownEffect | Effect name not found in registry |
| EE004 | DuplicateEffectDecl | Effect already declared |
| EE005 | ResumeOutsideHandler | `resume` used outside `handle` expression |
| EE006 | EffectForbiddenInContext | Effect not allowed in @kernel/@device/@safe |
