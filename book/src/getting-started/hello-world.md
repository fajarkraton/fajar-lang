# Hello World

Create a file called `hello.fj`:

```fajar
fn main() {
    println("Hello, Fajar Lang!")
}
```

Run it:

```bash
fj run hello.fj
```

Output:

```
Hello, Fajar Lang!
```

## What's Happening

1. `fn main()` — entry point, just like C or Rust
2. `println("...")` — built-in function to print a line
3. No semicolons needed — newlines separate statements
4. No explicit return type — `void` is the default

## Variables

```fajar
fn main() {
    let name = "Fajar"
    let version = 1
    let pi = 3.14159

    print("Language: ")
    println(name)
    print("Version: ")
    println(version)
}
```

## Functions

```fajar
fn add(a: i64, b: i64) -> i64 {
    a + b
}

fn main() {
    let result = add(3, 4)
    println(result)  // 7
}
```

The last expression in a block is its return value — no `return` keyword needed.

## REPL

Start an interactive session:

```bash
fj repl
```

```
fj> let x = 42
fj> x * 2
84
fj> fn square(n: i64) -> i64 { n * n }
fj> square(7)
49
```
