# Lesson 3: Control Flow

## Objectives

By the end of this lesson, you will be able to:

- Use `if`/`else` as expressions
- Write `match` statements for pattern matching
- Loop with `while`, `for..in`, and `loop`
- Control loops with `break` and `continue`

## if/else

In Fajar Lang, `if`/`else` is an **expression** -- it returns a value.

```fajar
fn main() {
    let age = 25

    if age >= 18 {
        println("Adult")
    } else {
        println("Minor")
    }

    // if as an expression
    let status = if age >= 18 { "adult" } else { "minor" }
    println(status)
}
```

**Expected output:**

```
Adult
adult
```

### Chained Conditions

```fajar
fn classify(score: i64) -> str {
    if score >= 90 {
        "A"
    } else if score >= 80 {
        "B"
    } else if score >= 70 {
        "C"
    } else {
        "F"
    }
}

fn main() {
    println(classify(95))   // A
    println(classify(82))   // B
    println(classify(65))   // F
}
```

## match

Pattern matching is one of Fajar Lang's most powerful features. It is exhaustive -- you must handle all cases.

```fajar
fn describe(x: i64) -> str {
    match x {
        0 => "zero",
        1 => "one",
        2 => "two",
        _ => "many"
    }
}

fn main() {
    println(describe(0))    // zero
    println(describe(1))    // one
    println(describe(42))   // many
}
```

The `_` is a wildcard that matches anything not already covered.

### Match with Ranges

```fajar
fn temperature_feel(temp: i64) -> str {
    match temp {
        -50..=0 => "freezing",
        1..=15 => "cold",
        16..=25 => "comfortable",
        26..=35 => "warm",
        _ => "hot"
    }
}

fn main() {
    println(temperature_feel(-10))  // freezing
    println(temperature_feel(22))   // comfortable
    println(temperature_feel(40))   // hot
}
```

## while Loops

Repeat while a condition is true:

```fajar
fn main() {
    let mut count = 0
    while count < 5 {
        println(count)
        count = count + 1
    }
}
```

**Expected output:**

```
0
1
2
3
4
```

## for..in Loops

Iterate over a range or collection:

```fajar
fn main() {
    // Range-based for loop
    for i in 0..5 {
        println(i)
    }

    // Inclusive range
    for i in 1..=3 {
        println(f"item {i}")
    }
}
```

**Expected output:**

```
0
1
2
3
4
item 1
item 2
item 3
```

## loop (Infinite Loop with break)

`loop` repeats forever until you `break`:

```fajar
fn main() {
    let mut n = 1
    loop {
        if n > 100 {
            break
        }
        n = n * 2
    }
    println(n)  // 128 (first power of 2 > 100)
}
```

## break and continue

- `break` exits the loop immediately
- `continue` skips to the next iteration

```fajar
fn main() {
    // Print only odd numbers from 1 to 10
    for i in 1..=10 {
        if i % 2 == 0 {
            continue
        }
        println(i)
    }
}
```

**Expected output:**

```
1
3
5
7
9
```

### Finding a Value

```fajar
fn main() {
    let target = 7
    let mut found = false

    for i in 0..100 {
        if i * i == target * target {
            println(f"Found: {i}")
            found = true
            break
        }
    }

    if !found {
        println("Not found")
    }
}
```

## Nesting Control Flow

Combine loops and conditionals for more complex logic:

```fajar
fn fizzbuzz(n: i64) {
    for i in 1..=n {
        if i % 15 == 0 {
            println("FizzBuzz")
        } else if i % 3 == 0 {
            println("Fizz")
        } else if i % 5 == 0 {
            println("Buzz")
        } else {
            println(i)
        }
    }
}

fn main() {
    fizzbuzz(15)
}
```

## Exercises

### Exercise 3.1: Number Classifier (*)

Write a function that takes an integer and prints whether it is "positive", "negative", or "zero".

**Expected output for inputs -5, 0, 42:**

```
negative
zero
positive
```

### Exercise 3.2: Sum of Multiples (**)

Using a `for` loop, compute the sum of all multiples of 3 or 5 below 100. Print the result.

**Expected output:**

```
2318
```

### Exercise 3.3: Collatz Sequence (***)

Write a program that prints the Collatz sequence starting from 27. The rules are: if `n` is even, divide by 2; if odd, compute `3*n + 1`. Stop when `n` reaches 1. Print each value and the total number of steps.

**Expected output (first and last lines):**

```
27
82
41
...
2
1
Steps: 111
```
