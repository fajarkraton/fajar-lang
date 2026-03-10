# Tensor Operations

Tensors are first-class types in Fajar Lang, backed by ndarray for efficient computation.

## Creating Tensors

```fajar
let z = zeros(3, 4)          // 3x4 zero tensor
let o = ones(2, 2)           // 2x2 ones tensor
let r = randn(5, 5)          // 5x5 random normal
let e = eye(3)               // 3x3 identity matrix
let w = xavier(4, 8)         // Xavier-initialized weights
```

## From Data

```fajar
let t = from_data([1.0, 2.0, 3.0, 4.0], [2, 2])  // 2x2 tensor
let s = arange(0.0, 10.0, 1.0)                     // [0, 1, ..., 9]
let l = linspace(0.0, 1.0, 5)                      // [0, 0.25, 0.5, 0.75, 1.0]
```

## Arithmetic

```fajar
let a = ones(3, 3)
let b = ones(3, 3)
let sum = add(a, b)         // element-wise addition
let diff = sub(a, b)        // element-wise subtraction
let prod = mul(a, b)        // element-wise multiplication
let quot = div(a, b)        // element-wise division
```

## Matrix Operations

```fajar
let c = matmul(a, b)        // matrix multiplication
let t = transpose(a)        // transpose
let r = reshape(a, [1, 9])  // reshape to 1x9
let f = flatten(a)          // flatten to 1D
```

## Activation Functions

```fajar
let y = relu(x)             // max(0, x)
let y = sigmoid(x)          // 1 / (1 + exp(-x))
let y = tanh_act(x)         // hyperbolic tangent
let y = softmax(x)          // normalized exponential
let y = gelu(x)             // Gaussian error linear unit
let y = leaky_relu(x)       // max(0.01x, x)
```

## Tensor Properties

```fajar
let shape = tensor_shape(t)  // dimensions
let size = tensor_size(t)    // total elements
```

## Gradient Tracking

```fajar
set_requires_grad(t, true)  // enable gradient tracking
backward(loss)               // compute gradients
let g = grad(t)              // access gradient
zero_grad(t)                 // reset gradients
```
