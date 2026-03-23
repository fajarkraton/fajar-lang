# Compile-Time Evaluation

Fajar Lang supports Zig-style compile-time code execution via `comptime` blocks. Code inside `comptime` is evaluated during compilation and replaced with the computed result.

## Comptime Blocks

```fajar
let x = comptime { 6 * 7 }        // → 42
let table = comptime {
    let a = 10
    let b = 20
    a + b                           // → 30
}
```

## Comptime Functions

```fajar
comptime fn factorial(n: i64) -> i64 {
    if n <= 1 { 1 } else { n * factorial(n - 1) }
}
const FACT_10: i64 = comptime { factorial(10) }  // → 3628800
```

## Supported Operations

Integer/float arithmetic, boolean logic, comparisons, bitwise ops, if/else, let bindings, function calls, arrays, strings.

## Restrictions (CT007)

I/O, extern calls, and heap allocation are forbidden in comptime.

## Error Codes

CT001 (NotComptime), CT002 (Overflow), CT003 (DivByZero), CT004 (UndefinedVar), CT005 (UndefinedFn), CT006 (RecursionLimit), CT007 (IoForbidden), CT008 (TypeError).
