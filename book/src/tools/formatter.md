# Formatter

## Usage

```bash
fj fmt file.fj           # format a file in place
fj fmt --check file.fj   # check formatting without modifying
```

## Formatting Rules

The Fajar formatter enforces consistent style:

- 4-space indentation
- No trailing whitespace
- Single blank line between top-level items
- Consistent brace placement
- Aligned struct field types

## Example

Before:

```fajar
fn add( a:i64,b:i64 )->i64{
a+b
}
struct Point{x:f64,y:f64}
```

After:

```fajar
fn add(a: i64, b: i64) -> i64 {
    a + b
}

struct Point {
    x: f64,
    y: f64
}
```
