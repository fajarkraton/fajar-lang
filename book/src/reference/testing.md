# Test Framework

Fajar Lang has a built-in test framework with `@test` annotations and the `fj test` command.

## Writing Tests

Annotate functions with `@test`:

```fajar
fn add(a: i64, b: i64) -> i64 { a + b }

@test
fn test_add() {
    assert_eq(add(2, 3), 5)
}

@test
fn test_add_negative() {
    assert_eq(add(-1, 1), 0)
}
```

## Running Tests

```bash
fj test file.fj             # run all tests
fj test file.fj --filter add  # run tests matching "add"
```

## Assertions

| Function | Description |
|----------|-------------|
| `assert(cond)` | Fail if condition is false |
| `assert_eq(a, b)` | Fail if `a != b`, show both values |

## Annotations

### `@should_panic`

Expect the test to panic:

```fajar
@test @should_panic
fn test_division_by_zero() {
    let x = 1 / 0
}
```

### `@ignore`

Skip the test unless `--include-ignored`:

```fajar
@test @ignore
fn test_slow() {
    // skipped by default
}
```

Run ignored tests:
```bash
fj test file.fj --include-ignored
```

## Test Output

```
running 5 tests
test test_add             ... ok
test test_add_negative    ... ok
test test_factorial       ... ok
test test_division_by_zero ... ok (should panic)
test test_slow            ... ignored

test result: ok. 4 passed; 0 failed; 1 ignored
```

## Doc Tests

Code blocks in `///` doc comments are automatically extracted and run as tests:

```fajar
/// Adds two numbers.
///
/// ```
/// assert_eq(add(2, 3), 5)
/// ```
fn add(a: i64, b: i64) -> i64 { a + b }
```
