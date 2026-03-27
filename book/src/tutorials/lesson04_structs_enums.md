# Lesson 4: Structs and Enums

## Objectives

By the end of this lesson, you will be able to:

- Define and instantiate structs
- Add methods to structs with `impl`
- Define enums with data payloads
- Use pattern matching on enums

## Structs

Structs group related data together. Each field has a name and a type.

```fajar
struct Point {
    x: f64,
    y: f64
}

fn main() {
    let p = Point { x: 3.0, y: 4.0 }
    println(p.x)   // 3.0
    println(p.y)   // 4.0
}
```

### Mutable Structs

To modify fields, the variable must be `mut`:

```fajar
struct Counter {
    value: i64
}

fn main() {
    let mut c = Counter { value: 0 }
    println(c.value)    // 0
    c.value = c.value + 1
    println(c.value)    // 1
}
```

## Methods with impl

Use `impl` to attach methods to a struct. The first parameter `self` refers to the instance.

```fajar
struct Rectangle {
    width: f64,
    height: f64
}

impl Rectangle {
    fn area(self) -> f64 {
        self.width * self.height
    }

    fn perimeter(self) -> f64 {
        2.0 * (self.width + self.height)
    }

    fn is_square(self) -> bool {
        self.width == self.height
    }
}

fn main() {
    let rect = Rectangle { width: 5.0, height: 3.0 }
    println(rect.area())        // 15.0
    println(rect.perimeter())   // 16.0
    println(rect.is_square())   // false

    let square = Rectangle { width: 4.0, height: 4.0 }
    println(square.is_square()) // true
}
```

## Structs with Methods That Return Structs

```fajar
struct Vec2 {
    x: f64,
    y: f64
}

impl Vec2 {
    fn add(self, other: Vec2) -> Vec2 {
        Vec2 { x: self.x + other.x, y: self.y + other.y }
    }

    fn length(self) -> f64 {
        sqrt(self.x * self.x + self.y * self.y)
    }

    fn display(self) {
        println(f"({self.x}, {self.y})")
    }
}

fn main() {
    let a = Vec2 { x: 1.0, y: 2.0 }
    let b = Vec2 { x: 3.0, y: 4.0 }
    let c = a.add(b)
    c.display()           // (4.0, 6.0)
    println(c.length())   // 7.211102550927978
}
```

## Enums

Enums define a type that can be one of several variants. Variants can carry data.

```fajar
enum Direction {
    North,
    South,
    East,
    West
}

fn describe_direction(d: Direction) -> str {
    match d {
        Direction::North => "Going north",
        Direction::South => "Going south",
        Direction::East => "Going east",
        Direction::West => "Going west"
    }
}

fn main() {
    let heading = Direction::North
    println(describe_direction(heading))
}
```

### Enums with Data

Variants can hold values of different types:

```fajar
enum Shape {
    Circle(f64),
    Rectangle(f64, f64),
    Triangle(f64, f64, f64)
}

fn area(s: Shape) -> f64 {
    match s {
        Shape::Circle(radius) => PI * radius * radius,
        Shape::Rectangle(w, h) => w * h,
        Shape::Triangle(a, b, c) => {
            let s = (a + b + c) / 2.0
            sqrt(s * (s - a) * (s - b) * (s - c))
        }
    }
}

fn main() {
    let circle = Shape::Circle(5.0)
    let rect = Shape::Rectangle(4.0, 6.0)

    println(area(circle))   // 78.53981633974483
    println(area(rect))     // 24.0
}
```

## Combining Structs and Enums

Structs and enums work naturally together:

```fajar
struct Student {
    name: str,
    grade: Grade
}

enum Grade {
    Pass(i64),
    Fail,
    Incomplete
}

fn status(student: Student) -> str {
    match student.grade {
        Grade::Pass(score) => f"{student.name} passed with {score}",
        Grade::Fail => f"{student.name} failed",
        Grade::Incomplete => f"{student.name} is incomplete"
    }
}

fn main() {
    let s1 = Student { name: "Alice", grade: Grade::Pass(92) }
    let s2 = Student { name: "Bob", grade: Grade::Fail }
    println(status(s1))
    println(status(s2))
}
```

**Expected output:**

```
Alice passed with 92
Bob failed
```

## Exercises

### Exercise 4.1: Bank Account (*)

Define a `BankAccount` struct with `owner: str` and `balance: f64`. Add methods `deposit(amount: f64)` and `display()`. Create an account, deposit 500.0, and display it.

**Expected output:**

```
Account: Fajar, Balance: 500.0
```

### Exercise 4.2: Traffic Light (**)

Define a `TrafficLight` enum with variants `Red`, `Yellow`, `Green`. Write a function `action(light: TrafficLight) -> str` that returns "stop", "caution", or "go". Test all three.

**Expected output:**

```
stop
caution
go
```

### Exercise 4.3: Expression Evaluator (***)

Define an enum `Expr` with variants `Num(f64)`, `Add(Expr, Expr)`, `Mul(Expr, Expr)`. Write a recursive function `eval(e: Expr) -> f64` that evaluates the expression. Test with `(2 + 3) * 4`.

**Expected output:**

```
20.0
```
