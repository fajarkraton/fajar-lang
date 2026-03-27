# Tensor API

Tensors are first-class types in Fajar Lang, designed for machine learning and numerical computing. All tensor functions require `@device` or `@unsafe` context.

## Creating Tensors

| Function | Signature | Description |
|----------|-----------|-------------|
| `zeros` | `zeros(rows: i64, cols: i64) -> Tensor` | All-zero tensor |
| `ones` | `ones(rows: i64, cols: i64) -> Tensor` | All-one tensor |
| `randn` | `randn(rows: i64, cols: i64) -> Tensor` | Random normal distribution |
| `eye` | `eye(n: i64) -> Tensor` | Identity matrix |
| `from_data` | `from_data(data: [[f64]]) -> Tensor` | From nested arrays |
| `arange` | `arange(start: f64, end: f64, step: f64) -> Tensor` | Evenly spaced values |
| `linspace` | `linspace(start: f64, end: f64, count: i64) -> Tensor` | Linearly spaced values |

### Examples

```fajar
@device
fn create_tensors() {
    let a = zeros(3, 4)                    // 3x4 zero matrix
    let b = ones(2, 2)                     // 2x2 ones matrix
    let c = randn(10, 5)                   // 10x5 random normal
    let I = eye(3)                         // 3x3 identity
    let d = from_data([[1.0, 2.0], [3.0, 4.0]])  // 2x2 from data
    let r = arange(0.0, 10.0, 1.0)        // [0, 1, 2, ..., 9]
    let s = linspace(0.0, 1.0, 5)         // [0.0, 0.25, 0.5, 0.75, 1.0]
}
```

## Arithmetic Operations

| Function | Signature | Description |
|----------|-----------|-------------|
| `add` | `add(a: Tensor, b: Tensor) -> Tensor` | Element-wise addition |
| `sub` | `sub(a: Tensor, b: Tensor) -> Tensor` | Element-wise subtraction |
| `mul` | `mul(a: Tensor, b: Tensor) -> Tensor` | Element-wise multiplication |
| `div` | `div(a: Tensor, b: Tensor) -> Tensor` | Element-wise division |
| `matmul` | `matmul(a: Tensor, b: Tensor) -> Tensor` | Matrix multiplication |

Operators `+`, `-`, `*`, `/` also work on tensors. Use `@` for matrix multiply.

```fajar
@device
fn arithmetic() {
    let a = from_data([[1.0, 2.0], [3.0, 4.0]])
    let b = from_data([[5.0, 6.0], [7.0, 8.0]])

    let sum = a + b              // element-wise add
    let product = a * b          // element-wise multiply
    let result = a @ b           // matrix multiply
    let scaled = mul(a, ones(2, 2) * 2.0)  // scale by 2
}
```

## Shape Operations

| Function | Signature | Description |
|----------|-----------|-------------|
| `transpose` | `transpose(t: Tensor) -> Tensor` | Swap rows and columns |
| `reshape` | `reshape(t: Tensor, rows: i64, cols: i64) -> Tensor` | Change shape (same total elements) |
| `flatten` | `flatten(t: Tensor) -> Tensor` | Flatten to 1D |
| `squeeze` | `squeeze(t: Tensor) -> Tensor` | Remove dimensions of size 1 |
| `split` | `split(t: Tensor, chunks: i64, dim: i64) -> [Tensor]` | Split along dimension |
| `concat` | `concat(tensors: [Tensor], dim: i64) -> Tensor` | Concatenate along dimension |

```fajar
@device
fn shapes() {
    let a = randn(3, 4)                // shape: [3, 4]
    let b = transpose(a)               // shape: [4, 3]
    let c = reshape(a, 6, 2)           // shape: [6, 2]
    let d = flatten(a)                 // shape: [12]

    let parts = split(a, 3, 0)        // 3 tensors of shape [1, 4]
    let joined = concat(parts, 0)      // back to shape [3, 4]
}
```

## Activation Functions

| Function | Signature | Description |
|----------|-----------|-------------|
| `relu` | `relu(t: Tensor) -> Tensor` | max(0, x) |
| `sigmoid` | `sigmoid(t: Tensor) -> Tensor` | 1 / (1 + exp(-x)) |
| `tanh` | `tanh(t: Tensor) -> Tensor` | Hyperbolic tangent |
| `softmax` | `softmax(t: Tensor) -> Tensor` | Softmax normalization |
| `gelu` | `gelu(t: Tensor) -> Tensor` | Gaussian Error Linear Unit |
| `leaky_relu` | `leaky_relu(t: Tensor, alpha: f64) -> Tensor` | Leaky ReLU |

```fajar
@device
fn activations() {
    let x = randn(4, 4)
    let a = relu(x)              // zero out negatives
    let b = sigmoid(x)           // squash to (0, 1)
    let c = softmax(x)           // probability distribution
    let d = gelu(x)              // smooth approximation of ReLU
    let e = leaky_relu(x, 0.01)  // small slope for negatives
}
```

## Practical Example

A simple linear regression step:

```fajar
@device
fn linear_step(
    X: Tensor, y: Tensor,
    W: Tensor, b: Tensor,
    lr: f64
) -> (Tensor, Tensor) {
    // Forward pass
    let pred = matmul(X, W) + b

    // Compute loss (MSE)
    let diff = pred - y
    let loss = mul(diff, diff)

    // Gradient step
    let grad_W = matmul(transpose(X), diff)
    let grad_b = diff

    let W_new = W - mul(grad_W, ones(1, 1) * lr)
    let b_new = b - mul(grad_b, ones(1, 1) * lr)

    (W_new, b_new)
}
```
