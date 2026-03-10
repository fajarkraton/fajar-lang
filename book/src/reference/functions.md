# Functions

## Basic Functions

```fajar
fn add(a: i64, b: i64) -> i64 {
    a + b
}

fn greet(name: str) {
    println("Hello, " + name)
}
```

The last expression in a function body is the return value (no semicolon needed).

## Explicit Return

Use `return` for early exit:

```fajar
fn abs(x: i64) -> i64 {
    if x < 0 { return -x }
    x
}
```

## Recursion

```fajar
fn factorial(n: i64) -> i64 {
    if n <= 1 { return 1 }
    n * factorial(n - 1)
}
```

## Generics

Functions can be generic over types:

```fajar
fn identity<T>(x: T) -> T {
    x
}

let n = identity(42)       // i64
let s = identity("hello")  // str
```

## Function Pointers

Functions are first-class values:

```fajar
fn double(x: i64) -> i64 { x * 2 }
fn apply(f: fn(i64) -> i64, x: i64) -> i64 { f(x) }

let result = apply(double, 5)  // 10
```

## Higher-Order Functions

Arrays support `map`, `filter`, and `reduce`:

```fajar
fn double(x: i64) -> i64 { x * 2 }
fn is_even(x: i64) -> bool { x % 2 == 0 }
fn sum(a: i64, b: i64) -> i64 { a + b }

let nums: [i64] = [1, 2, 3, 4]
let doubled = nums.map(double)         // [2, 4, 6, 8]
let evens = nums.filter(is_even)       // [2, 4]
let total = nums.reduce(sum, 0)        // 10
```

## Pipeline Operator

Chain function calls with `|>`:

```fajar
fn double(x: i64) -> i64 { x * 2 }
fn add_one(x: i64) -> i64 { x + 1 }

let result = 5 |> double |> add_one   // 11
```

## Context Annotations

Functions can be annotated with execution contexts:

```fajar
@kernel fn read_sensor() -> i64 {
    // Has access to hardware, no heap, no tensor
    port_read(0x40)
}

@device fn inference(data: Tensor) -> Tensor {
    // Has access to tensors, no raw pointers
    relu(matmul(data, weights))
}

@safe fn process() -> i64 {
    // Default: no hardware, no raw pointers
    42
}
```
