# String Methods

Fajar Lang strings (`str`) are UTF-8 encoded and heap-allocated. All string methods are available as method calls on string values.

## Method Reference

| Method | Signature | Description |
|--------|-----------|-------------|
| `len` | `len() -> i64` | Number of characters |
| `trim` | `trim() -> str` | Remove leading and trailing whitespace |
| `split` | `split(sep: str) -> [str]` | Split by separator into array |
| `replace` | `replace(old: str, new: str) -> str` | Replace all occurrences |
| `contains` | `contains(sub: str) -> bool` | Check if substring exists |
| `starts_with` | `starts_with(prefix: str) -> bool` | Check prefix |
| `ends_with` | `ends_with(suffix: str) -> bool` | Check suffix |
| `to_uppercase` | `to_uppercase() -> str` | Convert to uppercase |
| `to_lowercase` | `to_lowercase() -> str` | Convert to lowercase |
| `chars` | `chars() -> [char]` | Split into character array |
| `parse_int` | `parse_int() -> Result<i64, str>` | Parse as integer |
| `parse_float` | `parse_float() -> Result<f64, str>` | Parse as float |
| `repeat` | `repeat(n: i64) -> str` | Repeat string n times |
| `substring` | `substring(start: i64, end: i64) -> str` | Extract substring |
| `index_of` | `index_of(sub: str) -> Option<i64>` | Find first occurrence index |

## Examples

### `len`

Returns the number of characters (not bytes).

```fajar
let s = "Hello"
println(s.len())  // 5
```

### `trim`

Removes whitespace from both ends.

```fajar
let padded = "  hello  "
println(padded.trim())  // "hello"
```

### `split`

Splits the string by a separator and returns an array of substrings.

```fajar
let csv = "a,b,c,d"
let parts = csv.split(",")
println(parts)  // ["a", "b", "c", "d"]
```

### `replace`

Replaces all occurrences of a substring.

```fajar
let text = "hello world"
let updated = text.replace("world", "Fajar")
println(updated)  // "hello Fajar"
```

### `contains`

Checks whether the string contains a substring.

```fajar
let msg = "error: file not found"
if msg.contains("error") {
    eprintln(msg)
}
```

### `starts_with` / `ends_with`

Check prefix or suffix.

```fajar
let path = "/home/user/file.fj"
println(path.starts_with("/home"))   // true
println(path.ends_with(".fj"))       // true
```

### `to_uppercase` / `to_lowercase`

Case conversion.

```fajar
let name = "Fajar"
println(name.to_uppercase())  // "FAJAR"
println(name.to_lowercase())  // "fajar"
```

### `chars`

Splits the string into an array of individual characters.

```fajar
let word = "hi!"
let ch = word.chars()
println(ch)  // ['h', 'i', '!']
```

### `parse_int` / `parse_float`

Parse string content as a number. Returns `Result<T, str>`.

```fajar
let num = "42".parse_int()
match num {
    Ok(n) => println(n * 2),    // 84
    Err(e) => eprintln(e),
}

let pi = "3.14".parse_float()
match pi {
    Ok(f) => println(f),
    Err(e) => eprintln(e),
}
```

### `repeat`

Repeats the string a given number of times.

```fajar
let sep = "-".repeat(40)
println(sep)  // "----------------------------------------"
```

### `substring`

Extracts a substring by character indices (start inclusive, end exclusive).

```fajar
let text = "Hello, world!"
let word = text.substring(0, 5)
println(word)  // "Hello"
```

### `index_of`

Returns the index of the first occurrence of a substring, or `None`.

```fajar
let text = "hello world"
match text.index_of("world") {
    Some(i) => println(f"Found at {i}"),  // Found at 6
    None => println("Not found"),
}
```

## String Interpolation (F-Strings)

Use `f"..."` syntax for inline expression evaluation:

```fajar
let name = "Fajar"
let age = 30
println(f"Name: {name}, Age: {age}")
println(f"Next year: {age + 1}")
```

## String Concatenation

Use `+` to concatenate strings:

```fajar
let greeting = "Hello" + ", " + "world!"
println(greeting)  // "Hello, world!"
```
