# Lesson 6: Ownership and Borrowing

## Objectives

By the end of this lesson, you will be able to:

- Understand Fajar Lang's ownership model
- Use move semantics to transfer ownership
- Borrow values with `&` (shared) and `&mut` (exclusive)
- Avoid common ownership errors

## Why Ownership?

Fajar Lang guarantees memory safety without a garbage collector. Every value has exactly one **owner**. When the owner goes out of scope, the value is dropped (cleaned up). This prevents use-after-free, double-free, and data races.

## The Three Rules

1. Each value has exactly **one owner** at a time
2. When the owner goes out of scope, the value is **dropped**
3. Ownership can be **transferred** (moved) or **borrowed** (referenced)

## Move Semantics

When you assign a value to another variable, ownership **moves**:

```fajar
fn main() {
    let name = "Fajar"
    let greeting = name   // ownership moves to 'greeting'
    println(greeting)     // OK
    // println(name)      // ERROR: 'name' was moved
}
```

After a move, the original variable can no longer be used. The compiler catches this at compile time.

### Move in Function Calls

Passing a value to a function also moves it:

```fajar
fn take_ownership(s: str) {
    println(f"Got: {s}")
}

fn main() {
    let msg = "hello"
    take_ownership(msg)
    // println(msg)    // ERROR: 'msg' was moved into take_ownership
}
```

## Borrowing with &

Instead of moving, you can **borrow** a value with `&`. The original owner keeps ownership.

```fajar
fn print_length(s: &str) {
    println(f"Length: {len(s)}")
}

fn main() {
    let msg = "hello world"
    print_length(&msg)   // borrow msg
    println(msg)         // still valid -- we only borrowed it
}
```

### Rules of Shared Borrowing

You can have **multiple shared references** at the same time:

```fajar
fn main() {
    let data = "important"
    let r1 = &data
    let r2 = &data
    println(r1)   // OK
    println(r2)   // OK
    println(data)  // OK -- original still valid
}
```

## Mutable Borrowing with &mut

If you need to modify a borrowed value, use `&mut`. But you can only have **one mutable reference** at a time, and no shared references while a mutable reference exists.

```fajar
fn add_exclamation(s: &mut str) {
    *s = f"{*s}!"
}

fn main() {
    let mut greeting = "Hello"
    add_exclamation(&mut greeting)
    println(greeting)   // Hello!
}
```

### The Exclusivity Rule

```fajar
fn main() {
    let mut x = 10

    let r1 = &mut x
    // let r2 = &mut x    // ERROR: cannot borrow 'x' as mutable more than once
    // let r3 = &x        // ERROR: cannot borrow 'x' as shared while mutably borrowed

    *r1 = 20
    println(x)    // 20
}
```

This rule prevents data races at compile time.

## Scope and Lifetimes

References cannot outlive the data they point to:

```fajar
fn main() {
    let r;
    {
        let x = 42
        r = &x
    }
    // println(r)   // ERROR: 'x' was dropped, reference is dangling
}
```

The compiler ensures references are always valid.

## Returning References

Functions can return borrowed values, but the data must live long enough:

```fajar
fn longest(a: &str, b: &str) -> &str {
    if len(a) > len(b) { a } else { b }
}

fn main() {
    let s1 = "hello"
    let s2 = "world!"
    let result = longest(&s1, &s2)
    println(result)   // world!
}
```

## Practical Example: Building a Log

```fajar
struct Logger {
    entries: [str]
}

impl Logger {
    fn new() -> Logger {
        Logger { entries: [] }
    }

    fn log(&mut self, message: str) {
        self.entries.push(message)
    }

    fn dump(&self) {
        for entry in self.entries {
            println(entry)
        }
    }
}

fn main() {
    let mut logger = Logger::new()
    logger.log("Started")
    logger.log("Processing")
    logger.log("Done")
    logger.dump()
}
```

**Expected output:**

```
Started
Processing
Done
```

## Exercises

### Exercise 6.1: Ownership Transfer (*)

Write a function `consume(s: str)` that prints the string. In `main`, create a string, pass it to `consume`, and verify that you cannot use it afterward (add a comment showing the error).

**Expected output:**

```
Consumed: hello
```

### Exercise 6.2: Shared Borrowing (**)

Write a function `count_vowels(s: &str) -> i64` that counts the number of vowels (a, e, i, o, u) in a string by borrowing it. Call it twice on the same string to prove the original is not moved.

**Expected output:**

```
Vowels: 3
Vowels: 3
Original: hello world
```

### Exercise 6.3: Mutable Borrowing (***)

Create a mutable array of integers. Write a function `double_all(arr: &mut [i64])` that doubles every element in place. Print the array before and after calling the function.

**Expected output:**

```
Before: [1, 2, 3, 4, 5]
After: [2, 4, 6, 8, 10]
```
