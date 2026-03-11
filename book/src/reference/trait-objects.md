# Trait Objects & Dynamic Dispatch

Fajar Lang supports **trait objects** via `dyn Trait`, enabling runtime polymorphism with vtable-based dispatch.

## Basic Usage

Define a trait, then use `dyn Trait` as a type:

```fajar
trait Shape {
    fn area() -> f64
    fn name() -> str
}

struct Circle { radius: f64 }

impl Shape for Circle {
    fn area() -> f64 { 3.14159 * self.radius * self.radius }
    fn name() -> str { "Circle" }
}

// Accept any Shape via dynamic dispatch
fn print_shape(s: dyn Shape) {
    println(f"{s.name()}: area = {s.area()}")
}
```

## Trait Object Variables

You can store a concrete type in a `dyn Trait` variable:

```fajar
let shape: dyn Shape = Circle { radius: 5.0 }
println(shape.area())  // vtable dispatch
```

## How It Works

When a value is coerced to `dyn Trait`, Fajar builds a **vtable** — a table of function pointers for all trait methods. Method calls on trait objects use vtable lookup instead of static dispatch.

## Object Safety

Only traits with methods that take `self` (no generic parameters) can be used as trait objects. The trait must exist and the concrete type must implement it.

## Multiple Implementations

Different types can implement the same trait, and all can be used through `dyn Trait`:

```fajar
struct Rectangle { width: f64, height: f64 }
struct Triangle { base: f64, height: f64 }

impl Shape for Rectangle { ... }
impl Shape for Triangle { ... }

// All three work with print_shape
print_shape(Circle { radius: 5.0 })
print_shape(Rectangle { width: 4.0, height: 6.0 })
print_shape(Triangle { base: 3.0, height: 8.0 })
```
