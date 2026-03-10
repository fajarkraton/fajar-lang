# Error Handling

## Result Type

Functions that can fail return `Result`:

```fajar
fn divide(a: i64, b: i64) -> Result<i64, str> {
    if b == 0 {
        return Err("division by zero")
    }
    Ok(a / b)
}
```

## Pattern Matching on Results

```fajar
match divide(10, 3) {
    Ok(value) => println("Result: " + to_string(value)),
    Err(msg) => println("Error: " + msg)
}
```

## The ? Operator

Propagate errors up the call stack:

```fajar
fn safe_calc(a: i64, b: i64) -> Result<i64, str> {
    let x = divide(a, b)?    // returns Err early if divide fails
    Ok(x + 1)
}
```

## Option Type

For values that may be absent:

```fajar
fn find(arr: [i64], target: i64) -> Option<i64> {
    // returns Some(index) or None
}

match find(data, 42) {
    Some(idx) => println("Found at " + to_string(idx)),
    None => println("Not found")
}
```

## String Parsing

`parse_int` and `parse_float` return Result-like enum values:

```fajar
let result = parse_int("42")
match result {
    Ok(n) => println(n),
    Err(e) => println("Parse error: " + e)
}
```

## Assertions

For invariant checking:

```fajar
assert(x > 0)                  // panics if false
assert_eq(actual, expected)    // panics if not equal
```
