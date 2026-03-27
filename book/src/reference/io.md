# File I/O

Fajar Lang provides built-in functions for file operations and console output. All I/O functions are available globally.

## Console Output

### `print(value: any) -> void`

Writes a value to stdout without a trailing newline.

```fajar
print("Progress: ")
print(42)
print("%\n")
```

### `println(value: any) -> void`

Writes a value to stdout followed by a newline.

```fajar
println("Hello, world!")
println(42)
```

### `eprintln(value: any) -> void`

Writes a value to stderr followed by a newline. Use for error messages and diagnostics so they do not mix with normal program output.

```fajar
eprintln("Warning: configuration file missing")
eprintln(f"Error code: {code}")
```

## File Operations

### `read_file(path: str) -> Result<str, str>`

Reads the entire contents of a file as a UTF-8 string.

Returns `Ok(contents)` on success or `Err(message)` on failure.

```fajar
match read_file("config.toml") {
    Ok(contents) => {
        println(f"Config loaded: {contents.len()} chars")
    },
    Err(e) => {
        eprintln(f"Failed to read config: {e}")
    },
}
```

### `write_file(path: str, contents: str) -> Result<void, str>`

Writes a string to a file, creating it if it does not exist and overwriting any existing contents.

```fajar
let data = "name = \"Fajar\"\nversion = \"1.0\""
match write_file("output.toml", data) {
    Ok(_) => println("File written successfully"),
    Err(e) => eprintln(f"Write failed: {e}"),
}
```

### `append_file(path: str, contents: str) -> Result<void, str>`

Appends a string to the end of a file. Creates the file if it does not exist.

```fajar
let entry = f"[{timestamp}] Event logged\n"
match append_file("app.log", entry) {
    Ok(_) => {},
    Err(e) => eprintln(f"Log write failed: {e}"),
}
```

### `file_exists(path: str) -> bool`

Checks whether a file exists at the given path.

```fajar
if file_exists("config.toml") {
    let config = read_file("config.toml")
} else {
    println("Using default configuration")
}
```

## Practical Examples

### Reading and Processing a CSV File

```fajar
match read_file("data.csv") {
    Ok(contents) => {
        let lines = contents.split("\n")
        for line in lines {
            let fields = line.split(",")
            println(f"Name: {fields[0]}, Score: {fields[1]}")
        }
    },
    Err(e) => eprintln(f"Error: {e}"),
}
```

### Writing a Report

```fajar
let mut report = "# Report\n\n"
report = report + f"Total items: {count}\n"
report = report + f"Average score: {avg}\n"
report = report + f"Generated: {timestamp}\n"

match write_file("report.md", report) {
    Ok(_) => println("Report saved"),
    Err(e) => eprintln(f"Failed: {e}"),
}
```

### Append-Only Log

```fajar
fn log(level: str, message: str) {
    let line = f"[{level}] {message}\n"
    match append_file("app.log", line) {
        Ok(_) => {},
        Err(e) => eprintln(f"Log error: {e}"),
    }
}

log("INFO", "Application started")
log("WARN", "Low memory")
log("ERROR", "Connection refused")
```

## Error Handling Pattern

All file operations that can fail return `Result`. Use `match` or the `?` operator:

```fajar
// With match
match read_file("data.txt") {
    Ok(data) => process(data),
    Err(e) => eprintln(e),
}

// With ? operator (inside a function returning Result)
fn load_config() -> Result<str, str> {
    let contents = read_file("config.toml")?
    Ok(contents)
}
```
