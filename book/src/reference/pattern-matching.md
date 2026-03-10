# Pattern Matching

## Basic Match

Match on integers:

```fajar
let day = match n {
    1 => "Monday",
    2 => "Tuesday",
    3 => "Wednesday",
    _ => "other"
}
```

## Enum Patterns

Destructure enum variants:

```fajar
enum Result {
    Ok(i64),
    Err(str)
}

match result {
    Ok(value) => println("Success: " + to_string(value)),
    Err(msg) => println("Error: " + msg)
}
```

## Wildcard Pattern

`_` matches anything:

```fajar
match x {
    0 => println("zero"),
    _ => println("non-zero")
}
```

## Variable Binding

Bind matched value to a name:

```fajar
match x {
    0 => println("zero"),
    n => println("got: " + to_string(n))
}
```

## Exhaustiveness

Match must cover all cases. The compiler warns about non-exhaustive patterns:

```fajar
enum Direction { North, South, East, West }

match dir {
    Direction::North => println("up"),
    Direction::South => println("down"),
    // Missing East and West would produce a warning
    _ => println("sideways")
}
```

## Match as Expression

Match returns a value:

```fajar
let label = match score / 10 {
    10 => "perfect",
    9 => "excellent",
    8 => "good",
    _ => "keep trying"
}
```
