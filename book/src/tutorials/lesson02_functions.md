# Lesson 2: Functions

## Objectives

By the end of this lesson, you will be able to:

- Define functions with parameters and return types
- Call functions and use their return values
- Write recursive functions
- Understand expression-based returns

## Defining a Function

Functions are declared with `fn`. Parameters have explicit types. The return type comes after `->`.

```fajar
fn greet(name: str) {
    println(f"Hello, {name}!")
}

fn main() {
    greet("Fajar")
    greet("World")
}
```

**Expected output:**

```
Hello, Fajar!
Hello, World!
```

## Return Values

Use `->` to declare the return type. The last expression in the function body is the return value (no semicolon needed), or you can use `return` explicitly.

```fajar
fn add(a: i64, b: i64) -> i64 {
    a + b
}

fn multiply(a: i64, b: i64) -> i64 {
    return a * b
}

fn main() {
    let sum = add(3, 4)
    let product = multiply(3, 4)
    println(sum)       // 7
    println(product)   // 12
}
```

## Multiple Parameters

Functions can take any number of parameters:

```fajar
fn describe(name: str, age: i64, score: f64) -> str {
    f"{name} is {age} years old with score {score}"
}

fn main() {
    let info = describe("Fajar", 30, 95.5)
    println(info)
}
```

## Functions as Building Blocks

Break complex logic into small, focused functions:

```fajar
fn square(x: f64) -> f64 {
    x * x
}

fn distance(x1: f64, y1: f64, x2: f64, y2: f64) -> f64 {
    let dx = x2 - x1
    let dy = y2 - y1
    sqrt(square(dx) + square(dy))
}

fn main() {
    let d = distance(0.0, 0.0, 3.0, 4.0)
    println(d)  // 5.0
}
```

## Recursion

A function can call itself. This is called recursion. Every recursive function needs a **base case** to stop.

```fajar
fn factorial(n: i64) -> i64 {
    if n <= 1 {
        1
    } else {
        n * factorial(n - 1)
    }
}

fn main() {
    println(factorial(5))   // 120
    println(factorial(10))  // 3628800
}
```

### Fibonacci

The classic recursive example:

```fajar
fn fibonacci(n: i64) -> i64 {
    if n <= 0 {
        0
    } else if n == 1 {
        1
    } else {
        fibonacci(n - 1) + fibonacci(n - 2)
    }
}

fn main() {
    let mut i = 0
    while i < 10 {
        println(fibonacci(i))
        i = i + 1
    }
}
```

**Expected output:**

```
0
1
1
2
3
5
8
13
21
34
```

## Nested Functions

You can define helper functions inside other functions:

```fajar
fn main() {
    fn is_even(n: i64) -> bool {
        n % 2 == 0
    }

    println(is_even(4))   // true
    println(is_even(7))   // false
}
```

## Exercises

### Exercise 2.1: Circle Area (*)

Write a function `circle_area(radius: f64) -> f64` that returns the area of a circle (use `PI * radius * radius`). Call it with radius `5.0` and print the result.

**Expected output:**

```
78.53981633974483
```

### Exercise 2.2: Power Function (**)

Write a recursive function `power(base: i64, exp: i64) -> i64` that computes `base` raised to the `exp` power. Test with `power(2, 10)`.

**Expected output:**

```
1024
```

### Exercise 2.3: GCD (***)

Write a recursive function `gcd(a: i64, b: i64) -> i64` that computes the greatest common divisor using the Euclidean algorithm: `gcd(a, b) = gcd(b, a % b)` with base case `gcd(a, 0) = a`. Test with `gcd(48, 18)`.

**Expected output:**

```
6
```
