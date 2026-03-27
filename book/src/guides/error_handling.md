# Error Handling Guide

Fajar Lang uses `Result<T, E>` and `Option<T>` for error handling -- errors
are values, not exceptions. This guide covers practical patterns.

## The Basics

### Option -- Nullable Values

```fajar
let found: Option<i32> = Some(42)
let missing: Option<i32> = None

match found {
    Some(val) => println(f"Got: {val}"),
    None => println("Nothing here"),
}
```

### Result -- Operations That Can Fail

```fajar
fn divide(a: f64, b: f64) -> Result<f64, str> {
    if b == 0.0 {
        Err("division by zero")
    } else {
        Ok(a / b)
    }
}

match divide(10.0, 3.0) {
    Ok(val) => println(f"Result: {val}"),
    Err(msg) => println(f"Error: {msg}"),
}
```

## The ? Operator

Propagate errors up the call stack concisely:

```fajar
fn read_config() -> Result<Config, str> {
    let text = read_file("config.toml")?    // Returns Err early if file missing
    let config = parse_toml(text)?          // Returns Err early if parse fails
    Ok(config)
}
```

Without `?`, the equivalent code would require nested match statements.
The `?` operator extracts the `Ok` value or returns the `Err` immediately.

## Common Patterns

### Unwrap with Default

```fajar
let name = get_env("USER").unwrap_or("anonymous")
let port = parse_int(port_str).unwrap_or(8080)
```

### Map and AndThen (Chaining)

```fajar
let upper = get_name()
    .map(|n| n.to_upper())
    .unwrap_or("UNKNOWN")

let result = read_file("data.txt")
    .and_then(|text| parse_json(text))
    .and_then(|json| extract_field(json, "name"))
```

### Early Return Pattern

```fajar
fn process_order(id: i32) -> Result<Receipt, str> {
    let order = find_order(id)?
    let inventory = check_stock(order.item)?
    let payment = charge_card(order.total)?
    let receipt = create_receipt(order, payment)?
    Ok(receipt)
}
```

### Collecting Results

```fajar
let inputs = ["1", "2", "abc", "4"]
let results: Vec<Result<i32, str>> = inputs.iter()
    .map(|s| parse_int(s))
    .collect()

// Filter only successful parses
let valid: Vec<i32> = inputs.iter()
    .filter_map(|s| parse_int(s).ok())
    .collect()
```

## Custom Error Types

```fajar
enum AppError {
    NotFound(str),
    Permission(str),
    Network(i32),
}

fn load_user(id: i32) -> Result<User, AppError> {
    if id < 0 {
        return Err(AppError::NotFound(f"User {id} not found"))
    }
    // ...
    Ok(user)
}
```

## Error Handling in Main

The `main` function can return `Result`:

```fajar
fn main() -> Result<(), str> {
    let config = read_config()?
    let server = start_server(config)?
    server.run()?
    Ok(())
}
```

If `main` returns `Err`, the program prints the error and exits with code 1.

## Anti-Patterns to Avoid

| Do Not | Instead |
|--------|---------|
| Ignore errors silently | Always handle or propagate with `?` |
| Use `unwrap()` in libraries | Return `Result` or `Option` |
| Catch-all with `_` | Match each error variant explicitly |
| Use `panic()` for expected errors | Reserve panic for truly unrecoverable bugs |
