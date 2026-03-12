# Formal Verification

Fajar Lang supports formal verification through contract annotations and SMT solver integration. Verified code comes with mathematical proofs of correctness.

## Contracts

Add preconditions, postconditions, and invariants to functions:

```fajar
@verified
fn binary_search(arr: [i64], target: i64) -> Option<usize>
    requires arr.is_sorted()
    ensures match result {
        Some(i) => arr[i] == target,
        None => !arr.contains(target),
    }
{
    // Implementation...
}
```

### requires

Preconditions that must hold when the function is called:

```fajar
fn divide(a: i64, b: i64) -> i64
    requires b != 0
{
    a / b
}
```

### ensures

Postconditions that must hold when the function returns. Use `result` to refer to the return value:

```fajar
fn abs(x: i64) -> i64
    ensures result >= 0
    ensures result == x || result == -x
{
    if x >= 0 { x } else { -x }
}
```

### invariant

Loop invariants for proving termination and correctness:

```fajar
fn sum(arr: [i64]) -> i64 {
    let mut total = 0
    let mut i = 0
    while i < len(arr)
        invariant total == arr[0..i].sum()
        invariant i <= len(arr)
        decreases len(arr) - i
    {
        total = total + arr[i]
        i = i + 1
    }
    total
}
```

## SMT Integration

The compiler encodes verification conditions into SMT-LIB format and checks them with Z3 or CVC5:

- Integer arithmetic → QF_BV (bit-vector theory)
- Array operations → array theory
- Pointer operations → memory theory

## @verified Annotation

```fajar
@verified
@kernel
fn map_page(virt: VirtAddr, phys: PhysAddr) -> Result<(), PageError>
    requires virt.is_aligned(4096)
    requires phys.is_aligned(4096)
    ensures page_table.contains(virt)
{
    // Compiler proves this is safe
}
```

## Automatic Proofs

The compiler can automatically prove:
- **Bounds safety** — array accesses within bounds
- **Overflow safety** — arithmetic won't overflow
- **Null safety** — Option types properly checked
- **Division safety** — divisor is non-zero

## Interaction with @kernel

`@kernel @verified` enables proving hardware-critical properties:
- Page table bounds correctness
- Stack depth within limits
- IRQ handler latency constraints
