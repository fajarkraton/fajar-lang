# Compile-Time Evaluation

Fajar Lang can evaluate expressions at compile time, enabling zero-cost abstractions and compile-time tensor shape checking.

## Const Functions

Mark functions as `const fn` to allow compile-time evaluation:

```fajar
const fn factorial(n: i64) -> i64 {
    if n <= 1 { 1 }
    else { n * factorial(n - 1) }
}

const FACT_10: i64 = factorial(10)  // Computed at compile time
```

## Comptime Blocks

Use `comptime {}` for arbitrary compile-time computation:

```fajar
const LOOKUP_TABLE: [i64; 256] = comptime {
    let mut table = [0; 256]
    let mut i = 0
    while i < 256 {
        table[i] = i * i
        i = i + 1
    }
    table
}
```

## Tensor Shape Verification

The compiler verifies tensor shapes at compile time:

```fajar
// Shape<3, 4> @ Shape<4, 5> → Shape<3, 5>  ✅
let a: Tensor<3, 4> = zeros(3, 4)
let b: Tensor<4, 5> = zeros(4, 5)
let c = matmul(a, b)  // Compiler verifies 4 == 4

// Shape<3, 4> @ Shape<5, 6> → TE009 error  ❌
let d: Tensor<5, 6> = zeros(5, 6)
let e = matmul(a, d)  // Compile error: inner dimensions 4 != 5
```

Shape validation covers: matmul, broadcast, conv2d, reshape, and layer chains.

## @comptime Parameters

```fajar
fn repeat<@comptime N: usize>(value: i64) -> [i64; N] {
    comptime {
        let mut arr = [0; N]
        let mut i = 0
        while i < N {
            arr[i] = value
            i = i + 1
        }
        arr
    }
}
```

## Compile-Time Assertions

```fajar
const_assert!(size_of::<Point>() == 16)
const_assert!(BUFFER_SIZE >= 1024)
```

## Supported Operations

Comptime evaluation supports: arithmetic, comparison, logical operators, control flow (if/else, while, for), string manipulation, array/struct/enum construction, and function pointer creation. Recursion is limited to 128 levels.
