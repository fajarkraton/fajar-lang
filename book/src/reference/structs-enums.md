# Structs & Enums

## Structs

Define custom data types:

```fajar
struct Point {
    x: f64,
    y: f64
}

let p = Point { x: 3.0, y: 4.0 }
println(p.x)   // 3.0
```

## Methods

Implement methods with `impl`:

```fajar
struct Circle {
    radius: f64
}

impl Circle {
    fn area(self) -> f64 {
        3.14159 * self.radius * self.radius
    }

    fn scale(self, factor: f64) -> Circle {
        Circle { radius: self.radius * factor }
    }
}

let c = Circle { radius: 5.0 }
println(c.area())    // 78.53975
```

## Enums

Define algebraic data types:

```fajar
enum Color {
    Red,
    Green,
    Blue,
    Custom(i64)
}

let c = Color::Red
```

## Enums with Data

```fajar
enum Shape {
    Circle(f64),
    Rectangle(f64, f64)
}

let s = Shape::Circle(5.0)
```

## Option and Result

Built-in enum types for null-safety and error handling:

```fajar
// Option<T> for nullable values
let x = Some(42)
let y = None

// Result<T, E> for fallible operations
let ok = Ok(42)
let err = Err("something went wrong")
```

## Pattern Matching on Enums

```fajar
match shape {
    Shape::Circle(r) => println("Circle with radius " + to_string(r)),
    Shape::Rectangle(w, h) => println("Rectangle " + to_string(w) + "x" + to_string(h))
}
```
