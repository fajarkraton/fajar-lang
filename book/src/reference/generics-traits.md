# Generics & Traits

## Generic Functions

```fajar
fn max<T>(a: T, b: T) -> T {
    if a > b { a } else { b }
}

let m = max(10, 20)          // i64
let f = max(1.5, 2.7)        // f64
```

Fajar uses monomorphization: each generic instantiation generates specialized code with no runtime overhead.

## Multiple Type Parameters

```fajar
fn pair<A, B>(a: A, b: B) -> (A, B) {
    (a, b)
}
```

## Traits

Define shared behavior:

```fajar
trait Display {
    fn to_string(self) -> str
}

trait Area {
    fn area(self) -> f64
}
```

## Implementing Traits

```fajar
struct Circle {
    radius: f64
}

impl Area for Circle {
    fn area(self) -> f64 {
        3.14159 * self.radius * self.radius
    }
}

impl Display for Circle {
    fn to_string(self) -> str {
        "Circle(r=" + to_string(self.radius) + ")"
    }
}
```

## Trait Bounds

Constrain generic types:

```fajar
fn print_area<T: Area>(shape: T) {
    println(shape.area())
}
```

## Const Generics

Parameterize by compile-time constants:

```fajar
fn zeros<const N: usize>() -> [f64; N] {
    // array of N zeros
}
```

## Static Dispatch

All trait dispatch in Fajar is static (monomorphized). There are no vtables or dynamic dispatch, making it suitable for embedded systems where every byte counts.
