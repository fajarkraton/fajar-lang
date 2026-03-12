# Dependent Types

Fajar Lang supports dependent types — types that depend on values. This enables compile-time verification of array bounds, tensor shapes, and protocol correctness.

## Type-Level Natural Numbers

```fajar
type Nat = Zero | Succ(Nat)

type Two = Succ(Succ(Zero))
type Three = Succ(Succ(Succ(Zero)))
```

## Dependent Arrays

`DependentArray<T, N>` carries its length in the type:

```fajar
fn safe_head<T, N: Nat>(arr: DependentArray<T, Succ(N)>) -> T {
    arr[0]  // Guaranteed non-empty — Succ(N) is always >= 1
}

let arr: DependentArray<i64, Three> = [1, 2, 3]
let first = safe_head(arr)  // OK: Three = Succ(Two), matches Succ(N)

let empty: DependentArray<i64, Zero> = []
// safe_head(empty)  // Compile error: Zero doesn't match Succ(N)
```

## Dependent Tensors

Tensor operations are shape-checked at the type level:

```fajar
fn matmul<A, B, C>(
    x: DependentTensor<A, B>,
    y: DependentTensor<B, C>
) -> DependentTensor<A, C> {
    // Inner dimensions must match (B == B)
    tensor_matmul(x, y)
}

let w: DependentTensor<784, 128> = xavier(784, 128)
let x: DependentTensor<1, 784> = input_batch()
let h = matmul(x, w)  // Type: DependentTensor<1, 128>
```

Reshape requires a proof that dimensions multiply correctly:

```fajar
// reshape(Tensor<2, 6>) -> Tensor<3, 4>  ✅  (2*6 == 3*4 == 12)
// reshape(Tensor<2, 6>) -> Tensor<3, 5>  ❌  (2*6 != 3*5)
```

## Dependent Pattern Matching

```fajar
match tensor.shape() {
    Shape(1, n) => println(f"vector of length {n}"),
    Shape(m, n) => println(f"matrix {m}×{n}"),
}
```

## Type Erasure at Runtime

Dependent types are erased during code generation — they exist only at compile time for verification. There is no runtime cost.
