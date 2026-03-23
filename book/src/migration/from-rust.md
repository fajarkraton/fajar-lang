# Fajar Lang for Rust Developers

If you know Rust, you'll feel at home in Fajar Lang — with some exciting additions.

## What's Familiar

| Rust | Fajar Lang | Notes |
|------|-----------|-------|
| `let x = 42;` | `let x = 42` | No semicolons needed |
| `let mut x = 0;` | `let mut x = 0` | Same mutability |
| `fn add(a: i32, b: i32) -> i32` | `fn add(a: i64, b: i64) -> i64` | Same syntax |
| `struct Point { x: f64 }` | `struct Point { x: f64 }` | Identical |
| `enum Option<T>` | `enum Option<T>` | Identical |
| `match x { ... }` | `match x { ... }` | Identical |
| `impl Trait for Type` | `impl Trait for Type` | Identical |
| `Result<T, E>` | `Result<T, E>` | Built-in |
| `cargo build` | `fj build` | Same workflow |

## What's New (Features Rust Doesn't Have)

### 1. Effect System

Rust has no formal effect tracking. Fajar Lang does:

```fajar
// Effects are declared in function signatures
fn read_sensor() -> i64 with Hardware, IO {
    volatile_read(0x40001000)
}

// The compiler enforces:
// - @kernel code can't use Tensor effects
// - @device code can't use Hardware effects
// - @safe code can't use IO or Alloc
```

### 2. Linear Types

Rust has affine types (used *at most* once). Fajar Lang has linear types (used *exactly* once):

```fajar
linear struct FileHandle { fd: i64 }

fn open(path: str) -> linear FileHandle { ... }
fn close(handle: linear FileHandle) { ... }

fn bad() {
    let f = open("data.txt")
    // ERROR: linear value 'f' not consumed!
}
```

### 3. Comptime Evaluation

Like Zig's comptime, but in a Rust-like language:

```fajar
comptime fn factorial(n: i64) -> i64 {
    if n <= 1 { 1 } else { n * factorial(n - 1) }
}
const FACT_10: i64 = comptime { factorial(10) }
```

### 4. Context Annotations

No equivalent in Rust — compiler-enforced domain isolation:

```fajar
@kernel fn os_code() { ... }   // No heap, no tensors
@device fn ml_code() { ... }   // No hardware access
@safe fn app_code() { ... }    // No hardware, no tensors, no I/O
```

### 5. First-Class Tensors

```fajar
let t = zeros(3, 4)         // 3x4 tensor
let r = matmul(a, b)        // Matrix multiply
let g = backward(loss)      // Autograd
```

## Key Differences

| Rust | Fajar Lang |
|------|-----------|
| Lifetime annotations (`'a`) | Optional (elision rules cover most cases) |
| `unsafe { }` | `@unsafe fn name() { }` |
| `#[derive(Debug)]` | `@derive(Debug)` |
| `proc_macro` | `macro_rules!` + built-in macros |
| `cargo` | `fj` (build, run, test, doc, fmt, lsp) |
| `.unwrap()` | Not allowed in library code |
| `println!()` | `println()` (function, not macro) |

## Build Commands

```bash
fj build program.fj              # Cranelift (fast)
fj build program.fj --release    # LLVM O2 (optimized)
fj run program.fj                # Build and run
fj test                          # Run @test functions
fj doc program.fj                # Generate docs
```
