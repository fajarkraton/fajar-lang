# Macros

Fajar Lang provides function-like macros, declarative `macro_rules!`, and `@derive` for automatic trait implementations.

## Built-in Macros

```fajar
let arr = vec![1, 2, 3]              // Array from elements
let s = stringify!(42)                // → "42"
let msg = concat!("hello", " world") // → "hello world"
dbg!(value)                           // Debug print + return
todo!()                               // Panic: "not yet implemented"
env!("HOME")                          // Environment variable
```

## Function-Like Macros

Any `name!(args)` or `name![args]` is a macro invocation:

```fajar
let nums = vec![1, 2, 3, 4, 5]
let text = concat!("count: ", len(nums))
```

## Declarative Macros

```fajar
macro_rules! answer {
    () => { 42 }
}
```

## @derive

Automatically implement traits on structs:

```fajar
@derive(Debug, Clone, PartialEq)
struct Point { x: f64, y: f64 }
```

Supported derives: Debug, Clone, PartialEq, Default, Hash.

## Macro Registry

11 built-in macros: vec, stringify, concat, dbg, todo, env, include, cfg, line, file, column.
