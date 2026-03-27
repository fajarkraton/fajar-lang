# Lesson 1: Hello World

## Objectives

By the end of this lesson, you will be able to:

- Write and run your first Fajar Lang program
- Declare variables with `let` and `let mut`
- Use the four basic types: `i64`, `f64`, `bool`, `str`
- Print output with `println`

## Your First Program

Create a file called `hello.fj` and type the following:

```fajar
fn main() {
    println("Hello, Fajar Lang!")
}
```

Run it with:

```bash
fj run hello.fj
```

You should see `Hello, Fajar Lang!` printed to the terminal.

## Variables

In Fajar Lang, variables are declared with `let`. By default, they are **immutable** -- you cannot change them after assignment.

```fajar
fn main() {
    let name: str = "Fajar"
    let age: i64 = 30
    let height: f64 = 175.5
    let is_developer: bool = true

    println(name)
    println(age)
    println(height)
    println(is_developer)
}
```

### Type Inference

You do not always need to write the type. Fajar Lang can infer it:

```fajar
fn main() {
    let x = 42          // inferred as i64
    let pi = 3.14159    // inferred as f64
    let flag = false     // inferred as bool
    let greeting = "Hi" // inferred as str

    println(x)
    println(pi)
    println(flag)
    println(greeting)
}
```

### Mutable Variables

If you need to change a variable after creation, use `let mut`:

```fajar
fn main() {
    let mut counter = 0
    println(counter)    // prints: 0

    counter = counter + 1
    println(counter)    // prints: 1

    counter = counter + 1
    println(counter)    // prints: 2
}
```

Trying to reassign an immutable variable is a compile-time error:

```fajar
fn main() {
    let x = 10
    x = 20   // ERROR: cannot assign to immutable variable 'x'
}
```

## Constants

For values that never change and are known at compile time, use `const`:

```fajar
const MAX_USERS: i64 = 1000

fn main() {
    println(MAX_USERS)  // prints: 1000
}
```

## String Interpolation

Fajar Lang supports f-strings for embedding expressions inside strings:

```fajar
fn main() {
    let name = "Fajar"
    let year = 2026
    println(f"Hello, {name}! Welcome to {year}.")
}
```

**Expected output:** `Hello, Fajar! Welcome to 2026.`

## Basic Arithmetic

```fajar
fn main() {
    let a = 10
    let b = 3

    println(a + b)   // 13
    println(a - b)   // 7
    println(a * b)   // 30
    println(a / b)   // 3 (integer division)
    println(a % b)   // 1 (remainder)

    let x = 2.5
    let y = 1.5
    println(x + y)   // 4.0
    println(x * y)   // 3.75
}
```

## Exercises

### Exercise 1.1: Personal Introduction (*)

Write a program that declares your name, age, and favorite number, then prints them each on a separate line using `println`.

**Expected output (example):**

```
Fajar
30
7
```

### Exercise 1.2: Temperature Converter (**)

Write a program that converts 100 degrees Fahrenheit to Celsius using the formula `C = (F - 32) * 5.0 / 9.0`. Store the result in a variable and print it.

**Expected output:**

```
37.77777777777778
```

### Exercise 1.3: Swap Two Variables (**)

Declare two mutable variables `a = 10` and `b = 20`. Swap their values using a temporary variable, then print both to confirm the swap.

**Expected output:**

```
20
10
```
