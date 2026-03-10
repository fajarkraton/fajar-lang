# Language Tour

A quick overview of Fajar Lang's key features.

## Variables and Types

```fajar
let name = "Fajar"        // immutable string
let mut count = 0          // mutable integer
const MAX: i64 = 100       // compile-time constant
```

## Functions

```fajar
fn fibonacci(n: i64) -> i64 {
    if n <= 1 { return n }
    fibonacci(n - 1) + fibonacci(n - 2)
}
```

## Structs and Methods

```fajar
struct Point { x: f64, y: f64 }

impl Point {
    fn distance(self, other: Point) -> f64 {
        let dx = self.x - other.x
        let dy = self.y - other.y
        sqrt(dx * dx + dy * dy)
    }
}
```

## Enums and Pattern Matching

```fajar
enum Shape {
    Circle(f64),
    Rect(f64, f64)
}

fn area(s: Shape) -> f64 {
    match s {
        Shape::Circle(r) => 3.14159 * r * r,
        Shape::Rect(w, h) => w * h
    }
}
```

## Error Handling

```fajar
fn safe_divide(a: i64, b: i64) -> Result<i64, str> {
    if b == 0 { return Err("division by zero") }
    Ok(a / b)
}

let result = safe_divide(10, 3)?   // propagate with ?
```

## Pipeline Operator

```fajar
fn double(x: i64) -> i64 { x * 2 }
fn add_one(x: i64) -> i64 { x + 1 }

let result = 5 |> double |> add_one   // 11
```

## Context Annotations (Unique Feature)

Fajar enforces domain isolation at compile time:

```fajar
@kernel fn read_port() -> i64 {
    port_read(0x60)       // OK: hardware access allowed
    // zeros(3, 3)        // ERROR: no tensor in @kernel
}

@device fn infer(input: Tensor) -> Tensor {
    relu(matmul(input, weights))  // OK: tensor ops allowed
    // port_read(0x60)            // ERROR: no hardware in @device
}
```

## Concurrency

```fajar
let handle = Thread::spawn(fn() -> i64 { 42 })
let result = handle.join()    // 42

let (tx, rx) = Channel::new()
tx.send(100)
let val = rx.recv()           // 100
```

## Embedded ML

```fajar
let x = zeros(3, 3)           // 3x3 zero tensor
let w = xavier(3, 3)          // Xavier initialization
let y = relu(matmul(x, w))    // forward pass
backward(y)                    // compute gradients
```

## Native Compilation

Fajar compiles to native machine code via Cranelift, achieving 100-400x speedup over the interpreter:

```bash
fj run --native program.fj    # JIT compilation
fj build --release program.fj # AOT compilation to binary
```
