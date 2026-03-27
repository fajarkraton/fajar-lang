# Migration Guide

Coming from Rust, C, or Python? This guide shows how Fajar Lang compares
and how to translate your existing knowledge.

## From Rust

Fajar Lang is heavily inspired by Rust but simplifies several areas.

### Syntax Comparison

| Concept | Rust | Fajar Lang |
|---------|------|------------|
| Variable | `let x: i32 = 5;` | `let x: i32 = 5` |
| Mutable | `let mut x = 5;` | `let mut x = 5` |
| Function | `fn add(a: i32, b: i32) -> i32 { a + b }` | `fn add(a: i32, b: i32) -> i32 { a + b }` |
| Struct | `struct Point { x: f64, y: f64 }` | `struct Point { x: f64, y: f64 }` |
| Enum | `enum Color { Red, Blue }` | `enum Color { Red, Blue }` |
| Match | `match x { 1 => "one", _ => "other" }` | `match x { 1 => "one", _ => "other" }` |
| String interp | `format!("Hello {name}")` | `f"Hello {name}"` |
| Print | `println!("{}", x);` | `println(f"{x}")` |
| Error prop | `let v = foo()?;` | `let v = foo()?` |

### Key Differences from Rust

1. **No semicolons** -- statements end at newline
2. **No lifetime annotations** -- ownership lite with simpler rules
3. **No macro system** (Rust-style) -- uses `@annotations` instead
4. **Native tensor types** -- `Tensor` is a first-class type
5. **Context annotations** -- `@kernel`, `@device`, `@safe`, `@unsafe`
6. **f-strings** -- `f"Hello {name}"` instead of `format!()` macro
7. **No `Box`, `Rc`, `Arc` in user code** -- managed by the compiler

### What Stays the Same

- Ownership and move semantics
- Borrow checking (many `&T` or one `&mut T`)
- Pattern matching and exhaustiveness
- Traits and generics with monomorphization
- `Result<T, E>` and `Option<T>`
- `impl` blocks for methods

## From C/C++

### Syntax Comparison

| Concept | C/C++ | Fajar Lang |
|---------|-------|------------|
| Variable | `int x = 5;` | `let x: i32 = 5` |
| Pointer | `int* p = &x;` | `let p = &x` |
| Array | `int arr[3] = {1,2,3};` | `let arr = [1, 2, 3]` |
| Function | `int add(int a, int b) { return a+b; }` | `fn add(a: i32, b: i32) -> i32 { a + b }` |
| Struct | `struct Point { double x, y; };` | `struct Point { x: f64, y: f64 }` |
| Null check | `if (p != NULL)` | `match opt { Some(v) => ..., None => ... }` |
| Cast | `(float)x` | `x as f64` |
| Header files | `#include <stdio.h>` | `use std::io` |

### What C Developers Gain

- **No manual memory management** -- ownership system handles it
- **No null pointer bugs** -- `Option<T>` replaces null
- **No buffer overflows** -- bounds checking by default
- **No undefined behavior** -- compiler prevents UB
- **No header files** -- modules with `use`

### What to Watch For

- No pointer arithmetic (except in `@kernel`/`@unsafe`)
- No implicit type conversions -- use `as` explicitly
- Variables are immutable by default -- use `let mut` for mutable

## From Python

### Syntax Comparison

| Concept | Python | Fajar Lang |
|---------|--------|------------|
| Variable | `x = 42` | `let x = 42` |
| Type hint | `x: int = 42` | `let x: i32 = 42` |
| Function | `def add(a, b): return a + b` | `fn add(a: i32, b: i32) -> i32 { a + b }` |
| String | `f"Hello {name}"` | `f"Hello {name}"` |
| List | `[1, 2, 3]` | `[1, 2, 3]` |
| Dict | `{"a": 1}` | `HashMap::from([("a", 1)])` |
| Class | `class Point:` | `struct Point { ... }` |
| None | `None` | `None` (with `Option<T>`) |
| Lambda | `lambda x: x * 2` | `\|x\| x * 2` |
| Import | `import math` | `use std::math` |
| Tensor | `np.zeros((3, 4))` | `zeros(3, 4)` |

### What Python Developers Gain

- **10-100x faster execution** -- compiled, not interpreted
- **Type safety** -- catch bugs at compile time, not runtime
- **Native ML** -- tensors without NumPy dependency
- **Embedded deployment** -- compile to bare-metal ARM/RISC-V
- **Concurrency** -- real threads, not GIL-limited

### What to Watch For

- Types are required in function signatures
- Blocks use `{ }`, not indentation
- No dynamic typing -- types are checked at compile time
- Errors are values (`Result`), not exceptions (`try/except`)
