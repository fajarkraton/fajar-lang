# Testing Guide

Fajar Lang has a built-in test framework with annotations, assertions,
and property testing support.

## Writing Tests

Use the `@test` annotation:

```fajar
@test
fn test_addition() {
    assert_eq(2 + 2, 4)
}

@test
fn test_string_concat() {
    let greeting = "hello" + " " + "world"
    assert_eq(greeting, "hello world")
}
```

## Expecting Panics

```fajar
@test
@should_panic
fn test_divide_by_zero() {
    let _ = 1 / 0
}

@test
@should_panic("index out of bounds")
fn test_array_bounds() {
    let arr = [1, 2, 3]
    let _ = arr[10]
}
```

## Ignoring Tests

```fajar
@test
@ignore
fn test_slow_operation() {
    // Skipped unless --include-ignored is passed
    let result = heavy_computation()
    assert(result > 0)
}
```

## Running Tests

```bash
fj test                          # Run all tests
fj test tests/math.fj            # Run specific file
fj test --filter "string"        # Filter by name
fj test --include-ignored        # Include @ignore tests
```

## Assertions

| Function | Description |
|----------|-------------|
| `assert(condition)` | Fails if false |
| `assert_eq(left, right)` | Fails if not equal |
| `assert_ne(left, right)` | Fails if equal |
| `panic(message)` | Always fails with message |

## Test Organization

Group related tests in modules:

```fajar
mod tests {
    use super::*

    @test
    fn test_parse_valid() {
        let result = parse("42")
        assert_eq(result, Ok(42))
    }

    @test
    fn test_parse_invalid() {
        let result = parse("abc")
        assert(result.is_err())
    }
}
```

## Table-Driven Tests

```fajar
@test
fn test_fibonacci() {
    let cases = [
        (0, 0), (1, 1), (2, 1), (3, 2),
        (4, 3), (5, 5), (10, 55),
    ]
    for (input, expected) in cases {
        assert_eq(fib(input), expected)
    }
}
```

## Property Testing

Test invariants over random inputs:

```fajar
@test
@property(iterations: 1000)
fn test_sort_preserves_length(data: [i32]) {
    let sorted = sort(data)
    assert_eq(len(sorted), len(data))
}

@test
@property(iterations: 500)
fn test_reverse_is_involution(s: str) {
    assert_eq(reverse(reverse(s)), s)
}
```

## Benchmarks

```bash
fj bench                         # Run all benchmarks
fj bench benches/sort.fj         # Run specific benchmark
```

```fajar
@bench
fn bench_fibonacci() {
    fib(30)
}
```
