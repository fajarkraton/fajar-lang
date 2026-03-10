# Variables & Types

## Variable Declaration

Use `let` to declare variables. All variables are immutable by default.

```fajar
let x = 42           // type inferred as i64
let name = "Fajar"   // type inferred as str
let pi = 3.14159     // type inferred as f64
let active = true    // type inferred as bool
```

Use `mut` for mutable variables:

```fajar
let mut counter = 0
counter = counter + 1
```

## Type Annotations

Explicit types with colon syntax:

```fajar
let x: i64 = 42
let ratio: f64 = 0.75
let flag: bool = true
let msg: str = "hello"
```

## Integer Types

| Type | Size | Range |
|------|------|-------|
| `i8` | 8-bit | -128 to 127 |
| `i16` | 16-bit | -32,768 to 32,767 |
| `i32` | 32-bit | -2^31 to 2^31-1 |
| `i64` | 64-bit | -2^63 to 2^63-1 (default) |
| `i128` | 128-bit | -2^127 to 2^127-1 |
| `u8` | 8-bit | 0 to 255 |
| `u16` | 16-bit | 0 to 65,535 |
| `u32` | 32-bit | 0 to 2^32-1 |
| `u64` | 64-bit | 0 to 2^64-1 |
| `u128` | 128-bit | 0 to 2^128-1 |
| `isize` | pointer-sized | platform dependent |
| `usize` | pointer-sized | platform dependent |

## Float Types

| Type | Size | Precision |
|------|------|-----------|
| `f32` | 32-bit | ~7 decimal digits |
| `f64` | 64-bit | ~15 decimal digits (default) |

## Constants

Constants are always immutable and must have a type annotation:

```fajar
const MAX_SIZE: usize = 1024
const PI: f64 = 3.14159265358979
```

## Type Casting

Use `as` for explicit type conversion:

```fajar
let x: i64 = 42
let y: f64 = x as f64      // 42.0
let z: i32 = x as i32      // narrowing cast
```

## Arrays

Stack-allocated arrays with fixed size:

```fajar
let nums = [1, 2, 3, 4, 5]
let first = nums[0]         // 1
```

Dynamic heap-allocated arrays:

```fajar
let mut arr: [i64] = []
arr = arr.push(10)
arr = arr.push(20)
let length = len(arr)       // 2
```

## Tuples

```fajar
let pair = (42, "hello")
let x = pair.0              // 42
let y = pair.1              // "hello"
```

## Strings

Strings are UTF-8 encoded:

```fajar
let greeting = "Hello, World!"
let length = len(greeting)          // 13
let upper = greeting.to_uppercase() // "HELLO, WORLD!"
let trimmed = "  hi  ".trim()       // "hi"
```

## Option Type

Null-safe programming with `Option`:

```fajar
let x: i64 = 42
let some_val = Some(x)      // wraps value
let no_val = None            // no value
```

## Ownership

Variables follow move semantics for non-copy types. Copy types (integers, floats, booleans, strings) are implicitly copied:

```fajar
let a = [1, 2, 3]
let b = a            // a is moved, cannot use a after this
```
