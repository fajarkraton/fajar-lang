# Fajar Lang for C++ Developers

Fajar Lang gives you C++'s power with memory safety and modern ergonomics.

## What's Familiar

| C++ | Fajar Lang |
|-----|-----------|
| `int x = 42;` | `let x: i64 = 42` |
| `struct Point { double x; };` | `struct Point { x: f64 }` |
| `enum class Color { Red, Green };` | `enum Color { Red, Green }` |
| `template<typename T>` | `fn identity<T>(x: T) -> T` |
| `if (x > 0) { ... }` | `if x > 0 { ... }` |
| `for (auto& item : vec)` | `for item in vec { ... }` |
| `constexpr` | `comptime` |

## What You Gain

### Memory Safety (No Segfaults)

```fajar
// Ownership prevents use-after-free
let s = "hello"
let t = s           // s is moved
// println(s)       // ERROR: use after move

// Borrows prevent dangling references
let x = 42
let r = &x          // immutable borrow — safe
```

### No Undefined Behavior

- Integer overflow is checked (not silent wraparound)
- Null safety via `Option<T>` (no null pointers)
- Bounds checking on arrays
- No uninitialized variables

### Context-Enforced Safety

```fajar
@kernel fn driver() {
    // Compiler GUARANTEES: no heap, no tensor ops
    // Perfect for OS kernel code
}

@device fn inference() {
    // Compiler GUARANTEES: no raw pointer, no IRQ
    // Perfect for ML inference
}
```

### Effect System (No Hidden Side Effects)

```fajar
fn pure_math(a: i64, b: i64) -> i64 { a + b }
// No `with` clause = pure function, guaranteed

fn io_fn() with IO { println("hello") }
// Explicit: this function does I/O
```

## C++ → Fajar Lang Cheat Sheet

```fajar
// C++: std::vector<int> v = {1, 2, 3};
let v = vec![1, 2, 3]

// C++: auto result = condition ? a : b;
let result = if condition { a } else { b }

// C++: std::optional<int>
let maybe: Option<i64> = Some(42)

// C++: try { ... } catch (...) { ... }
let result = risky_fn()  // Returns Result<T, E>
match result {
    Ok(val) => println(val),
    Err(e) => println(e),
}

// C++: constexpr int fact(int n) { ... }
comptime fn fact(n: i64) -> i64 {
    if n <= 1 { 1 } else { n * fact(n - 1) }
}
```
