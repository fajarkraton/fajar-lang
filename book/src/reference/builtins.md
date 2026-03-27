# Builtin Functions

Fajar Lang provides global builtin functions available in every scope without imports.

## Output Functions

### `print(value: any) -> void`

Prints a value to stdout without a trailing newline.

```fajar
print("Loading")
print(".")
print(".")
// Output: Loading..
```

### `println(value: any) -> void`

Prints a value to stdout followed by a newline.

```fajar
println("Hello, world!")
println(42)
println(3.14)
```

### `eprintln(value: any) -> void`

Prints a value to stderr followed by a newline. Used for error messages and diagnostics.

```fajar
eprintln("Error: file not found")
```

## Inspection Functions

### `len(collection: Array | str | HashMap) -> i64`

Returns the number of elements in an array, characters in a string, or entries in a map.

```fajar
let arr = [1, 2, 3]
println(len(arr))       // 3
println(len("hello"))   // 5
```

### `type_of(value: any) -> str`

Returns the runtime type name of a value as a string.

```fajar
println(type_of(42))        // "i64"
println(type_of("hello"))   // "str"
println(type_of([1, 2]))    // "Array"
println(type_of(true))      // "bool"
```

### `dbg(value: any) -> any`

Prints the value with its type and source location to stderr, then returns the value. Useful for debugging expressions inline.

```fajar
let x = dbg(2 + 3)  // [dbg] 5 (i64)
let y = dbg(x * 2)  // [dbg] 10 (i64)
```

## Assertion Functions

### `assert(condition: bool) -> void`

Panics with error RE006 if the condition is false.

```fajar
assert(1 + 1 == 2)         // passes
assert(len([1, 2]) == 2)   // passes
// assert(false)            // panics: assertion failed
```

### `assert_eq(left: any, right: any) -> void`

Panics with error RE006 if the two values are not equal. Shows both values on failure.

```fajar
assert_eq(2 + 2, 4)
assert_eq("hello", "hello")
// assert_eq(1, 2)  // panics: assertion failed: 1 != 2
```

## Control Flow Functions

### `panic(message: str) -> never`

Immediately terminates execution with an error message.

```fajar
fn divide(a: i64, b: i64) -> i64 {
    if b == 0 {
        panic("division by zero")
    }
    a / b
}
```

### `todo() -> never`

Marks unfinished code. Panics at runtime with "not yet implemented".

```fajar
fn future_feature() -> i64 {
    todo()
}
```

## Conversion Functions

### `to_string(value: any) -> str`

Converts any value to its string representation.

```fajar
let s = to_string(42)      // "42"
let t = to_string(3.14)    // "3.14"
let u = to_string(true)    // "true"
```

### `to_int(value: str | f64 | bool) -> i64`

Converts a value to an integer. Truncates floats. `true` becomes 1, `false` becomes 0.

```fajar
let x = to_int(3.14)    // 3
let y = to_int(true)    // 1
```

### `to_float(value: str | i64 | bool) -> f64`

Converts a value to a float.

```fajar
let x = to_float(42)    // 42.0
let y = to_float(true)  // 1.0
```

### `parse_int(s: str) -> Result<i64, str>`

Parses a string as an integer. Returns `Ok(value)` or `Err(message)`.

```fajar
let result = parse_int("42")
match result {
    Ok(n) => println(n),
    Err(e) => eprintln(e),
}
```

### `parse_float(s: str) -> Result<f64, str>`

Parses a string as a float. Returns `Ok(value)` or `Err(message)`.

```fajar
let result = parse_float("3.14")
match result {
    Ok(f) => println(f),
    Err(e) => eprintln(e),
}
```

## Collection Functions

### `push(array: &mut Array, value: any) -> void`

Appends a value to the end of an array.

```fajar
let mut arr = [1, 2, 3]
push(arr, 4)
println(arr)  // [1, 2, 3, 4]
```

### `pop(array: &mut Array) -> any`

Removes and returns the last element of an array. Panics if empty.

```fajar
let mut arr = [1, 2, 3]
let last = pop(arr)
println(last)  // 3
println(arr)   // [1, 2]
```

## Constructors

### `Some(value: T) -> Option<T>`

Wraps a value in an `Option`, indicating presence.

```fajar
let x: Option<i64> = Some(42)
```

### `None -> Option<T>`

The absent variant of `Option`.

```fajar
let x: Option<i64> = None
```

### `Ok(value: T) -> Result<T, E>`

Wraps a success value in a `Result`.

```fajar
let x: Result<i64, str> = Ok(42)
```

### `Err(error: E) -> Result<T, E>`

Wraps an error value in a `Result`.

```fajar
let x: Result<i64, str> = Err("not found")
```
