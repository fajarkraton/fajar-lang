# Algebraic Effects

Fajar Lang supports algebraic effects for structured side-effect control. Effects let you declare, perform, and handle side effects without hard-coding their implementation.

## Declaring Effects

```fajar
effect Console {
    fn read_line() -> str
    fn write_line(msg: str) -> void
}

effect State<T> {
    fn get() -> T
    fn set(val: T) -> void
}
```

## Performing Effects

Use `perform` to invoke an effect operation within a function. The function signature must declare which effects it uses with `/ EffectName`:

```fajar
fn greet(name: str) -> void / Console {
    perform Console.write_line(f"Hello, {name}!")
}

fn increment() -> void / State<i64> {
    let current = perform State.get()
    perform State.set(current + 1)
}
```

## Handling Effects

Use `handle` blocks to provide implementations for effects:

```fajar
fn main() {
    handle greet("Fajar") {
        Console.write_line(msg) => {
            println(msg)
            resume
        }
    }
}
```

The `resume` keyword continues execution from where the effect was performed. You can also `abort` to stop execution or `transform` the resumed value.

## Effect Inference

The compiler automatically infers effect sets for functions. The `#[pure]` annotation asserts a function has no effects:

```fajar
#[pure]
fn add(a: i64, b: i64) -> i64 {
    a + b  // No effects — compiler verifies this
}
```

## Context Interaction

Effects interact with context annotations:
- `@kernel` functions cannot use `Alloc` effects (no heap in kernel)
- `@device` functions cannot use `IO` effects (no raw I/O)
- The compiler enforces these restrictions statically

## Built-in Effects

| Effect | Description |
|--------|-------------|
| `IO` | File/network I/O operations |
| `Alloc` | Heap memory allocation |
| `Panic` | Panic/abort |
| `Async` | Asynchronous operations |
| `State` | Mutable state |
| `Exception` | Exception handling |

Effect error codes: EE001-EE008.
