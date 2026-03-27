# Primitive Types

Fajar Lang provides a complete set of primitive types for systems programming, ML, and general-purpose use.

## Integer Types

| Type | Size | Range |
|------|------|-------|
| `i8` | 1 byte | -128 to 127 |
| `i16` | 2 bytes | -32,768 to 32,767 |
| `i32` | 4 bytes | -2,147,483,648 to 2,147,483,647 |
| `i64` | 8 bytes | -9.2 x 10^18 to 9.2 x 10^18 |
| `i128` | 16 bytes | -1.7 x 10^38 to 1.7 x 10^38 |
| `u8` | 1 byte | 0 to 255 |
| `u16` | 2 bytes | 0 to 65,535 |
| `u32` | 4 bytes | 0 to 4,294,967,295 |
| `u64` | 8 bytes | 0 to 1.8 x 10^19 |
| `u128` | 16 bytes | 0 to 3.4 x 10^38 |
| `isize` | pointer-sized | Platform-dependent (i64 on 64-bit) |
| `usize` | pointer-sized | Platform-dependent (u64 on 64-bit) |

```fajar
let byte: u8 = 255
let count: i32 = -42
let big: i128 = 170_141_183_460_469_231_731_687_303_715_884_105_727
let index: usize = 0
```

Integer literals support underscores for readability and base prefixes:

```fajar
let hex: i32 = 0xFF
let bin: u8 = 0b1010_0110
let oct: i32 = 0o777
let big: i64 = 1_000_000
```

## Floating-Point Types

| Type | Size | Precision | Range |
|------|------|-----------|-------|
| `f32` | 4 bytes | ~7 decimal digits | 1.2 x 10^-38 to 3.4 x 10^38 |
| `f64` | 8 bytes | ~15 decimal digits | 2.2 x 10^-308 to 1.8 x 10^308 |

```fajar
let pi: f64 = 3.14159265358979
let temp: f32 = 98.6
let sci: f64 = 1.5e-10
```

The default floating-point type for untyped literals is `f64`.

## Boolean Type

| Type | Size | Values |
|------|------|--------|
| `bool` | 1 byte | `true`, `false` |

```fajar
let active: bool = true
let done = false  // inferred as bool
```

## Character Type

| Type | Size | Description |
|------|------|-------------|
| `char` | 4 bytes | A single Unicode scalar value (U+0000 to U+10FFFF) |

```fajar
let letter: char = 'A'
let emoji: char = '\u{1F600}'
let newline: char = '\n'
```

## String Type

| Type | Size | Description |
|------|------|-------------|
| `str` | variable | UTF-8 encoded, heap-allocated string |

```fajar
let name: str = "Fajar Lang"
let greeting = f"Hello, {name}!"
let multiline = "line one\nline two"
```

## Void Type

| Type | Size | Description |
|------|------|-------------|
| `void` | 0 bytes | The unit type; represents no meaningful value |

Functions that return nothing implicitly return `void`.

```fajar
fn greet(name: str) -> void {
    println(f"Hello, {name}")
}
```

## Never Type

| Type | Size | Description |
|------|------|-------------|
| `never` | 0 bytes | The bottom type; represents computations that never complete |

Used for functions that always panic, loop forever, or call `exit`.

```fajar
fn abort(msg: str) -> never {
    panic(msg)
}
```

## Type Casting

Use `as` for explicit type conversions between numeric types:

```fajar
let x: i32 = 42
let y: f64 = x as f64       // 42.0
let z: u8 = 200 as u8       // 200
let w: i32 = 3.14 as i32    // 3 (truncates)
```

Casting between incompatible types (e.g., `str` to `i32`) is a compile error. Use `parse_int` or `parse_float` instead.

## Default Values

| Type | Default |
|------|---------|
| Integer types | `0` |
| Float types | `0.0` |
| `bool` | `false` |
| `char` | `'\0'` |
| `str` | `""` |

## Type Inference

Fajar Lang infers types when the type annotation is omitted:

```fajar
let x = 42          // inferred i64
let y = 3.14        // inferred f64
let z = "hello"     // inferred str
let b = true        // inferred bool
```
