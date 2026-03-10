# Control Flow

## If/Else

`if`/`else` are expressions that return values:

```fajar
let max = if a > b { a } else { b }
```

Multi-branch:

```fajar
if score >= 90 {
    println("A")
} else if score >= 80 {
    println("B")
} else {
    println("C")
}
```

## While Loops

```fajar
let mut i = 0
while i < 10 {
    println(i)
    i = i + 1
}
```

## For Loops

Iterate over ranges:

```fajar
for i in 0..10 {
    println(i)     // 0, 1, 2, ..., 9
}

for i in 0..=10 {
    println(i)     // 0, 1, 2, ..., 10 (inclusive)
}
```

## Loop

Infinite loop, exit with `break`:

```fajar
let mut count = 0
loop {
    count = count + 1
    if count >= 5 { break }
}
```

## Break and Continue

```fajar
for i in 0..100 {
    if i % 2 == 0 { continue }   // skip even numbers
    if i > 10 { break }          // stop at 10
    println(i)                     // 1, 3, 5, 7, 9
}
```

## Match Expressions

Pattern matching on values:

```fajar
let label = match x {
    0 => "zero",
    1 => "one",
    _ => "other"
}
```

Match on enums:

```fajar
match result {
    Some(value) => println(value),
    None => println("nothing")
}
```
