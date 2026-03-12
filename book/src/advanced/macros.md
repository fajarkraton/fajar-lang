# Macros

Fajar Lang provides a declarative macro system inspired by Rust, with hygienic expansion and built-in derive macros.

## Declarative Macros

Define macros with `macro_rules!`:

```fajar
macro_rules! vec {
    ($($elem:expr),*) => {
        {
            let mut arr = []
            $(arr.push($elem);)*
            arr
        }
    }
}

let nums = vec![1, 2, 3, 4, 5]
```

## Fragment Types

| Fragment | Matches | Example |
|----------|---------|---------|
| `$x:expr` | Any expression | `1 + 2`, `f(x)` |
| `$x:ident` | Identifier | `foo`, `my_var` |
| `$x:ty` | Type expression | `i64`, `Vec<T>` |
| `$x:stmt` | Statement | `let x = 5` |
| `$x:block` | Block | `{ ... }` |
| `$x:pat` | Pattern | `Some(x)`, `_` |
| `$x:literal` | Literal | `42`, `"hello"` |

## Repetition

Use `$(...)*` for zero-or-more and `$(...)+` for one-or-more:

```fajar
macro_rules! println {
    ($fmt:expr, $($arg:expr),*) => {
        print(format!($fmt, $($arg),*))
        print("\n")
    }
}
```

## Derive Macros

Automatically implement traits with `#[derive(...)]`:

```fajar
#[derive(Debug, Clone, PartialEq, Hash, Default)]
struct Point {
    x: f64,
    y: f64,
}
```

Available derive macros: `Debug`, `Clone`, `PartialEq`, `Hash`, `Default`, `Serialize`.

## Attribute Macros

```fajar
#[cfg(target_os = "linux")]
fn linux_only() { ... }

#[inline]
fn hot_path(x: i64) -> i64 { x * 2 }

#[deprecated(since = "0.9.0", note = "use new_api() instead")]
fn old_api() { ... }

#[repr(C)]
struct CCompatible { x: i32, y: i32 }
```

## Built-in Macros

| Macro | Description |
|-------|-------------|
| `vec![...]` | Create array from elements |
| `println!(fmt, ...)` | Print formatted line |
| `format!(fmt, ...)` | Format string |
| `assert!(expr)` | Assert expression is true |
| `dbg!(expr)` | Debug print with location |
| `cfg!(pred)` | Conditional compilation check |
| `compile_error!(msg)` | Emit compile error |
| `env!(key)` | Environment variable at compile time |
| `file!()` | Current file name |
| `line!()` | Current line number |

## Hygiene

Macros use hygienic expansion — variables introduced inside a macro do not conflict with variables at the call site. Each expansion gets unique gensym identifiers to prevent name collisions.

Recursive expansion is limited to 64 levels to prevent infinite loops.
