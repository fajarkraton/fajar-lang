# String Interpolation

Fajar Lang supports f-strings for embedding expressions in string literals.

## Syntax

Prefix a string with `f` and use `{expr}` for interpolation:

```fajar
let name = "World"
println(f"Hello, {name}!")
// Hello, World!
```

## Expressions

Any expression can appear inside `{}`:

```fajar
let x = 10
let y = 20
println(f"{x} + {y} = {x + y}")
// 10 + 20 = 30
```

## Multiple Types

F-strings automatically convert values to strings:

```fajar
let n: i64 = 42
let pi = 3.14159
let flag = true
println(f"n={n}, pi={pi}, flag={flag}")
// n=42, pi=3.14159, flag=true
```

## Function Calls

```fajar
fn greet(name: str) -> str {
    f"Hello, {name}!"
}

println(greet("Fajar"))
// Hello, Fajar!
```

## Escaped Braces

Use `{{` and `}}` for literal braces:

```fajar
println(f"Use {{braces}} for literal braces")
// Use {braces} for literal braces
```

## With Collections

```fajar
let nums = [1, 2, 3]
println(f"Array has {len(nums)} elements")
// Array has 3 elements
```
