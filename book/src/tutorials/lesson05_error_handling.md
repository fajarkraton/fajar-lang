# Lesson 5: Error Handling

## Objectives

By the end of this lesson, you will be able to:

- Use `Option<T>` for values that may be absent
- Use `Result<T, E>` for operations that may fail
- Propagate errors with the `?` operator
- Handle errors gracefully with `match`

## The Problem: Things Go Wrong

What happens when you divide by zero? Or look up a key that does not exist? In Fajar Lang, we handle these cases explicitly with types -- no hidden exceptions, no null pointer surprises.

## Option: Maybe a Value

`Option<T>` represents a value that might be absent. It has two variants:

- `Some(value)` -- the value is present
- `None` -- no value

```fajar
fn find_first_even(numbers: [i64]) -> Option<i64> {
    for n in numbers {
        if n % 2 == 0 {
            return Some(n)
        }
    }
    None
}

fn main() {
    let nums = [1, 3, 4, 7, 8]
    let result = find_first_even(nums)

    match result {
        Some(n) => println(f"Found even number: {n}"),
        None => println("No even numbers found")
    }
}
```

**Expected output:**

```
Found even number: 4
```

### Safely Accessing Optional Values

```fajar
fn get_name(id: i64) -> Option<str> {
    match id {
        1 => Some("Alice"),
        2 => Some("Bob"),
        _ => None
    }
}

fn main() {
    let name = get_name(1)
    match name {
        Some(n) => println(f"Hello, {n}!"),
        None => println("User not found")
    }

    let unknown = get_name(99)
    match unknown {
        Some(n) => println(f"Hello, {n}!"),
        None => println("User not found")
    }
}
```

**Expected output:**

```
Hello, Alice!
User not found
```

## Result: Success or Error

`Result<T, E>` represents an operation that can either succeed or fail:

- `Ok(value)` -- success with a value
- `Err(error)` -- failure with an error

```fajar
fn divide(a: f64, b: f64) -> Result<f64, str> {
    if b == 0.0 {
        Err("division by zero")
    } else {
        Ok(a / b)
    }
}

fn main() {
    let result1 = divide(10.0, 3.0)
    match result1 {
        Ok(val) => println(f"Result: {val}"),
        Err(msg) => println(f"Error: {msg}")
    }

    let result2 = divide(5.0, 0.0)
    match result2 {
        Ok(val) => println(f"Result: {val}"),
        Err(msg) => println(f"Error: {msg}")
    }
}
```

**Expected output:**

```
Result: 3.3333333333333335
Error: division by zero
```

## The ? Operator: Error Propagation

Writing `match` for every Result gets verbose. The `?` operator propagates errors automatically -- if the value is `Err`, the function returns early with that error.

```fajar
fn parse_and_double(input: str) -> Result<i64, str> {
    let n = parse_int(input)?
    Ok(n * 2)
}

fn main() {
    match parse_and_double("21") {
        Ok(val) => println(val),    // 42
        Err(e) => println(f"Error: {e}")
    }

    match parse_and_double("abc") {
        Ok(val) => println(val),
        Err(e) => println(f"Error: {e}")
    }
}
```

**Expected output:**

```
42
Error: invalid integer
```

## Chaining with ?

The `?` operator shines when you chain multiple fallible operations:

```fajar
fn read_config_value(path: str, key: str) -> Result<i64, str> {
    let content = read_file(path)?
    let value = parse_int(content)?
    Ok(value)
}
```

Each `?` returns early if the operation fails, so the happy path reads linearly.

## Combining Option and Result

You can convert between them as needed:

```fajar
fn lookup(data: [i64], index: i64) -> Option<i64> {
    if index >= 0 && index < len(data) {
        Some(data[index])
    } else {
        None
    }
}

fn main() {
    let items = [10, 20, 30]

    match lookup(items, 1) {
        Some(val) => println(f"Found: {val}"),
        None => println("Index out of bounds")
    }

    match lookup(items, 5) {
        Some(val) => println(f"Found: {val}"),
        None => println("Index out of bounds")
    }
}
```

**Expected output:**

```
Found: 20
Index out of bounds
```

## Exercises

### Exercise 5.1: Safe Division (*)

Write a function `safe_sqrt(x: f64) -> Result<f64, str>` that returns `Err("negative input")` if `x < 0.0`, or `Ok(sqrt(x))` otherwise. Test with `25.0` and `-4.0`.

**Expected output:**

```
Ok: 5.0
Error: negative input
```

### Exercise 5.2: Chain of Checks (**)

Write a function `validate_age(input: str) -> Result<str, str>` that:
1. Parses the string to an integer (propagate error with `?`)
2. Returns `Err("too young")` if age < 0
3. Returns `Err("unrealistic")` if age > 150
4. Returns `Ok("valid")` otherwise

Test with "25", "-5", "200", and "abc".

**Expected output:**

```
valid
too young
unrealistic
parse error
```

### Exercise 5.3: Option Chaining (***)

Write a function `first_positive(numbers: [i64]) -> Option<i64>` that returns the first positive number in the array. Write another function `double_first_positive(numbers: [i64]) -> Option<i64>` that doubles it if found. Test with `[-3, -1, 0, 4, 7]` and `[-5, -2, -1]`.

**Expected output:**

```
Some(8)
None
```
