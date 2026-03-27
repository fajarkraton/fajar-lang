# Lesson 14: Testing

## Objectives

By the end of this lesson, you will be able to:

- Write unit tests with the `@test` attribute
- Test for expected panics with `@should_panic`
- Use `assert`, `assert_eq`, and `assert_ne`
- Run tests with the `fj test` command
- Understand property-based testing concepts

## Why Test?

Fajar Lang's compiler catches many bugs at compile time (type errors, ownership violations, context misuse). But logic errors still slip through. Tests verify that your code does what you intend.

## Your First Test

Tests are functions annotated with `@test`:

```fajar
fn add(a: i64, b: i64) -> i64 {
    a + b
}

@test
fn test_add_positive() {
    assert_eq(add(2, 3), 5)
}

@test
fn test_add_negative() {
    assert_eq(add(-1, -2), -3)
}

@test
fn test_add_zero() {
    assert_eq(add(0, 0), 0)
}
```

Run tests with:

```bash
fj test
```

Output:

```
Running 3 tests...
  test_add_positive ... ok
  test_add_negative ... ok
  test_add_zero     ... ok

3 passed, 0 failed
```

## Assertion Functions

Fajar Lang provides three built-in assertions:

```fajar
@test
fn test_assertions() {
    // assert: check a boolean condition
    assert(2 + 2 == 4)

    // assert_eq: check two values are equal
    assert_eq(len("hello"), 5)

    // assert_ne: check two values are NOT equal
    assert_ne(1, 2)
}
```

When an assertion fails, the test reports the file, line, and values involved:

```
FAILED: test_assertions at line 5
  assert_eq failed:
    left:  4
    right: 5
```

## Testing for Panics

Use `@should_panic` to verify that a function panics as expected:

```fajar
fn divide(a: i64, b: i64) -> i64 {
    if b == 0 {
        panic("division by zero")
    }
    a / b
}

@test
@should_panic
fn test_divide_by_zero_panics() {
    divide(10, 0)
}
```

The test passes if the function panics, and fails if it does not.

## Ignoring Tests

Use `@ignore` to skip a test temporarily:

```fajar
@test
@ignore
fn test_slow_computation() {
    // This test takes too long, skip it for now
    let result = expensive_compute()
    assert_eq(result, 42)
}
```

Ignored tests appear in the output but are not run:

```
  test_slow_computation ... ignored
```

## Organizing Tests

Group related tests in a module:

```fajar
mod math {
    pub fn factorial(n: i64) -> i64 {
        if n <= 1 { 1 } else { n * factorial(n - 1) }
    }

    pub fn fibonacci(n: i64) -> i64 {
        if n <= 0 { 0 }
        else if n == 1 { 1 }
        else { fibonacci(n - 1) + fibonacci(n - 2) }
    }
}

@test
fn test_factorial_base() {
    assert_eq(math::factorial(0), 1)
    assert_eq(math::factorial(1), 1)
}

@test
fn test_factorial_five() {
    assert_eq(math::factorial(5), 120)
}

@test
fn test_fibonacci_sequence() {
    assert_eq(math::fibonacci(0), 0)
    assert_eq(math::fibonacci(1), 1)
    assert_eq(math::fibonacci(10), 55)
}
```

## Testing Error Handling

Test that functions return the correct `Result` or `Option`:

```fajar
fn parse_positive(s: str) -> Result<i64, str> {
    let n = parse_int(s)?
    if n <= 0 {
        Err("not positive")
    } else {
        Ok(n)
    }
}

@test
fn test_parse_valid() {
    match parse_positive("42") {
        Ok(n) => assert_eq(n, 42),
        Err(_) => panic("expected Ok")
    }
}

@test
fn test_parse_negative() {
    match parse_positive("-5") {
        Ok(_) => panic("expected Err"),
        Err(msg) => assert_eq(msg, "not positive")
    }
}

@test
fn test_parse_invalid() {
    match parse_positive("abc") {
        Ok(_) => panic("expected Err"),
        Err(_) => assert(true)  // any error is fine
    }
}
```

## Property-Based Testing

Instead of testing specific inputs, property-based tests check that a property holds for many random inputs:

```fajar
@test
fn test_reverse_twice_is_identity() {
    // For any array, reversing twice gives back the original
    let data = [1, 2, 3, 4, 5]
    let mut copy = data.clone()
    copy.reverse()
    copy.reverse()
    assert_eq(copy, data)
}

@test
fn test_sort_is_idempotent() {
    // Sorting an already-sorted array changes nothing
    let mut data = [5, 3, 1, 4, 2]
    data.sort()
    let first_sort = data.clone()
    data.sort()
    assert_eq(data, first_sort)
}

@test
fn test_length_after_push() {
    // Pushing always increases length by 1
    let mut arr = [1, 2, 3]
    let before = len(arr)
    arr.push(4)
    assert_eq(len(arr), before + 1)
}
```

## Running Specific Tests

```bash
# Run all tests
fj test

# Run tests matching a pattern
fj test factorial

# Run tests in a specific file
fj test src/math.fj

# Run with verbose output
fj test --verbose
```

## Test-Driven Development (TDD) Workflow

The recommended workflow for writing Fajar Lang code:

1. **Write a failing test** -- define what the function should do
2. **Run tests** -- see the test fail (RED)
3. **Write minimal code** -- make the test pass (GREEN)
4. **Refactor** -- clean up while tests still pass
5. **Repeat**

```fajar
// Step 1: Write the test first
@test
fn test_clamp() {
    assert_eq(clamp(5, 0, 10), 5)    // within range
    assert_eq(clamp(-3, 0, 10), 0)   // below min
    assert_eq(clamp(15, 0, 10), 10)  // above max
}

// Step 2: Run tests -- they fail because clamp() doesn't exist

// Step 3: Write the implementation
fn clamp(value: i64, min_val: i64, max_val: i64) -> i64 {
    if value < min_val {
        min_val
    } else if value > max_val {
        max_val
    } else {
        value
    }
}

// Step 4: Run tests -- they pass
```

## Exercises

### Exercise 14.1: Test Suite (*)

Write a function `is_palindrome(s: str) -> bool` that checks if a string reads the same forward and backward. Write at least 4 tests: an empty string, a single character, a palindrome ("racecar"), and a non-palindrome ("hello").

**Expected test output:**

```
Running 4 tests...
  test_empty_string    ... ok
  test_single_char     ... ok
  test_palindrome      ... ok
  test_not_palindrome  ... ok

4 passed, 0 failed
```

### Exercise 14.2: Edge Cases (**)

Write a function `safe_divide(a: f64, b: f64) -> Result<f64, str>`. Write tests covering: normal division, division by zero, dividing zero by a number, and very large numbers. Use `@should_panic` where appropriate.

**Expected test output:**

```
Running 4 tests...
  test_normal_division   ... ok
  test_divide_by_zero    ... ok
  test_zero_numerator    ... ok
  test_large_numbers     ... ok

4 passed, 0 failed
```
