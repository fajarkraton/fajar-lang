# Lesson 7: Generics and Traits

## Objectives

By the end of this lesson, you will be able to:

- Define generic functions and structs with type parameters
- Define traits to describe shared behavior
- Implement traits for your types
- Use trait bounds to constrain generics

## Generics: One Function, Many Types

Instead of writing separate functions for each type, generics let you write one version that works with any type.

```fajar
fn identity<T>(x: T) -> T {
    x
}

fn main() {
    let a = identity(42)         // T = i64
    let b = identity("hello")   // T = str
    let c = identity(3.14)      // T = f64

    println(a)   // 42
    println(b)   // hello
    println(c)   // 3.14
}
```

## Generic Structs

Structs can also be generic:

```fajar
struct Pair<T> {
    first: T,
    second: T
}

impl<T> Pair<T> {
    fn new(a: T, b: T) -> Pair<T> {
        Pair { first: a, second: b }
    }
}

fn main() {
    let ints = Pair::new(1, 2)
    println(ints.first)    // 1
    println(ints.second)   // 2

    let strs = Pair::new("hello", "world")
    println(strs.first)    // hello
    println(strs.second)   // world
}
```

### Multiple Type Parameters

```fajar
struct KeyValue<K, V> {
    key: K,
    value: V
}

fn main() {
    let entry = KeyValue { key: "name", value: 42 }
    println(entry.key)     // name
    println(entry.value)   // 42
}
```

## Traits: Shared Behavior

A trait defines a set of methods that types can implement. Think of it as an interface or contract.

```fajar
trait Describable {
    fn describe(self) -> str
}
```

Any type that implements `Describable` promises to provide a `describe` method.

### Implementing a Trait

```fajar
trait Describable {
    fn describe(self) -> str
}

struct Dog {
    name: str,
    breed: str
}

impl Describable for Dog {
    fn describe(self) -> str {
        f"{self.name} is a {self.breed}"
    }
}

struct Car {
    make: str,
    year: i64
}

impl Describable for Car {
    fn describe(self) -> str {
        f"{self.year} {self.make}"
    }
}

fn main() {
    let dog = Dog { name: "Rex", breed: "Labrador" }
    let car = Car { make: "Toyota", year: 2024 }

    println(dog.describe())   // Rex is a Labrador
    println(car.describe())   // 2024 Toyota
}
```

## Trait Bounds

You can require that a generic type implements a specific trait:

```fajar
fn print_description<T: Describable>(item: T) {
    println(item.describe())
}

fn main() {
    let dog = Dog { name: "Rex", breed: "Labrador" }
    print_description(dog)   // Rex is a Labrador
}
```

## Common Built-in Traits

Fajar Lang has several built-in traits:

```fajar
trait Display {
    fn to_string(self) -> str
}

trait Clone {
    fn clone(self) -> Self
}

trait PartialEq {
    fn eq(self, other: Self) -> bool
}
```

### Implementing Display

```fajar
struct Point {
    x: f64,
    y: f64
}

impl Display for Point {
    fn to_string(self) -> str {
        f"({self.x}, {self.y})"
    }
}

fn main() {
    let p = Point { x: 3.0, y: 4.0 }
    println(p.to_string())   // (3.0, 4.0)
}
```

## Practical Example: A Sortable Collection

```fajar
trait Comparable {
    fn compare(self, other: Self) -> i64
}

struct Score {
    name: str,
    value: i64
}

impl Comparable for Score {
    fn compare(self, other: Score) -> i64 {
        self.value - other.value
    }
}

fn find_max<T: Comparable>(items: [T]) -> T {
    let mut best = items[0]
    for i in 1..len(items) {
        if items[i].compare(best) > 0 {
            best = items[i]
        }
    }
    best
}

fn main() {
    let scores = [
        Score { name: "Alice", value: 85 },
        Score { name: "Bob", value: 92 },
        Score { name: "Carol", value: 78 }
    ]
    let winner = find_max(scores)
    println(f"Winner: {winner.name} with {winner.value}")
}
```

**Expected output:**

```
Winner: Bob with 92
```

## Exercises

### Exercise 7.1: Generic min/max (*)

Write generic functions `min_val<T: Comparable>(a: T, b: T) -> T` and `max_val<T: Comparable>(a: T, b: T) -> T`. Test with integers.

**Expected output:**

```
Min: 3
Max: 7
```

### Exercise 7.2: Area Trait (**)

Define a trait `HasArea` with method `fn area(self) -> f64`. Implement it for `Circle { radius: f64 }` and `Square { side: f64 }`. Write a function `print_area<T: HasArea>(shape: T)` and test both.

**Expected output:**

```
Area: 78.53981633974483
Area: 25.0
```

### Exercise 7.3: Stack<T> (***)

Implement a generic `Stack<T>` struct with methods `push(item: T)`, `pop() -> Option<T>`, and `is_empty() -> bool`. Test with both integers and strings.

**Expected output:**

```
Popped: Some(30)
Popped: Some(20)
Popped: Some(10)
Empty: true
```
