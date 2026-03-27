# Lesson 10: Tensors and Machine Learning

## Objectives

By the end of this lesson, you will be able to:

- Create tensors with `zeros`, `ones`, `randn`, and `from_data`
- Perform tensor operations (add, multiply, matmul, reshape)
- Use autograd for automatic differentiation
- Build a simple training loop with `backward()`

## What Are Tensors?

Tensors are multi-dimensional arrays -- the fundamental data structure of machine learning. In Fajar Lang, `Tensor` is a **first-class type**, meaning the compiler understands tensor shapes and operations natively.

## Creating Tensors

```fajar
@device
fn main() {
    // Zeros: create a 3x4 tensor filled with 0.0
    let a = zeros(3, 4)
    println(a)

    // Ones: create a 2x3 tensor filled with 1.0
    let b = ones(2, 3)
    println(b)

    // Random normal: create a 2x2 tensor with random values
    let c = randn(2, 2)
    println(c)

    // From specific data
    let d = from_data([[1.0, 2.0], [3.0, 4.0]])
    println(d)

    // Identity matrix
    let e = eye(3)
    println(e)
}
```

Notice the `@device` annotation -- tensor operations require device context.

## Tensor Shapes

Every tensor has a shape describing its dimensions:

```fajar
@device
fn main() {
    let t = zeros(3, 4)
    println(t.shape())    // [3, 4]
    println(t.ndim())     // 2
    println(t.size())     // 12
}
```

## Basic Operations

Tensors support element-wise arithmetic:

```fajar
@device
fn main() {
    let a = from_data([[1.0, 2.0], [3.0, 4.0]])
    let b = from_data([[5.0, 6.0], [7.0, 8.0]])

    // Element-wise operations
    let sum = a + b
    println(sum)      // [[6.0, 8.0], [10.0, 12.0]]

    let product = a * b
    println(product)  // [[5.0, 12.0], [21.0, 32.0]]

    // Scalar operations
    let scaled = a * 2.0
    println(scaled)   // [[2.0, 4.0], [6.0, 8.0]]
}
```

## Matrix Multiplication

The `matmul` operation is the backbone of neural networks:

```fajar
@device
fn main() {
    let a = from_data([[1.0, 2.0], [3.0, 4.0]])
    let b = from_data([[5.0, 6.0], [7.0, 8.0]])

    let c = matmul(a, b)
    println(c)   // [[19.0, 22.0], [43.0, 50.0]]
}
```

### Shape Rules for matmul

`matmul(A, B)` requires: A is [M, K] and B is [K, N], result is [M, N].

```fajar
@device
fn main() {
    let a = zeros(3, 4)   // [3, 4]
    let b = zeros(4, 2)   // [4, 2]
    let c = matmul(a, b)  // [3, 2] -- inner dimensions must match
    println(c.shape())    // [3, 2]
}
```

## Reshaping and Transposing

```fajar
@device
fn main() {
    let a = from_data([[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]])
    println(a.shape())         // [2, 3]

    let b = reshape(a, 3, 2)
    println(b.shape())         // [3, 2]

    let c = transpose(a)
    println(c.shape())         // [3, 2]
    println(c)                 // [[1.0, 4.0], [2.0, 5.0], [3.0, 6.0]]
}
```

## Activation Functions

Neural networks need non-linear activation functions:

```fajar
@device
fn main() {
    let x = from_data([[-1.0, 0.0, 1.0, 2.0]])

    println(relu(x))       // [[0.0, 0.0, 1.0, 2.0]]
    println(sigmoid(x))    // [[0.269, 0.5, 0.731, 0.881]]
    println(tanh(x))       // [[-0.762, 0.0, 0.762, 0.964]]
}
```

## Autograd: Automatic Differentiation

Autograd tracks operations on tensors and computes gradients automatically via `backward()`.

```fajar
@device
fn main() {
    // Create tensors with gradient tracking
    let x = from_data([[2.0, 3.0]])
    set_requires_grad(x, true)

    // Forward pass: y = x^2
    let y = x * x

    // Compute sum for scalar loss
    let loss = sum(y)
    println(f"Loss: {loss}")    // Loss: 13.0

    // Backward pass: compute gradients
    backward(loss)

    // dy/dx = 2x
    let grad_x = grad(x)
    println(f"Gradient: {grad_x}")   // Gradient: [[4.0, 6.0]]
}
```

## A Simple Training Loop

Putting it all together -- learn weights to approximate a function:

```fajar
@device
fn main() {
    // Target: learn w such that w * x ≈ y
    let x = from_data([[1.0], [2.0], [3.0], [4.0]])
    let y_target = from_data([[2.0], [4.0], [6.0], [8.0]])

    // Initialize weight randomly
    let mut w = randn(1, 1)
    set_requires_grad(w, true)

    let lr = 0.01

    for epoch in 0..100 {
        // Forward pass
        let y_pred = matmul(x, w)
        let diff = y_pred - y_target
        let loss = sum(diff * diff) / 4.0

        if epoch % 20 == 0 {
            println(f"Epoch {epoch}, Loss: {loss}")
        }

        // Backward pass
        backward(loss)

        // Update weight (gradient descent)
        let g = grad(w)
        w = w - g * lr
        set_requires_grad(w, true)
    }

    println(f"Learned weight: {w}")   // Should be close to [[2.0]]
}
```

**Expected output (approximate):**

```
Epoch 0, Loss: 25.3
Epoch 20, Loss: 0.12
Epoch 40, Loss: 0.001
Epoch 60, Loss: 0.00001
Epoch 80, Loss: 0.0000001
Learned weight: [[1.9999]]
```

## Exercises

### Exercise 10.1: Tensor Basics (*)

Create a 3x3 identity matrix using `eye(3)`. Multiply it by a vector `[1.0, 2.0, 3.0]` using `matmul`. Verify the result is the same vector.

**Expected output:**

```
[[1.0], [2.0], [3.0]]
```

### Exercise 10.2: Activation Comparison (**)

Create a tensor with values `[-2.0, -1.0, 0.0, 1.0, 2.0]`. Apply `relu`, `sigmoid`, and `softmax` to it. Print all three results to compare how each activation transforms the data.

**Expected output (approximate):**

```
ReLU:    [0.0, 0.0, 0.0, 1.0, 2.0]
Sigmoid: [0.119, 0.269, 0.5, 0.731, 0.881]
Softmax: [0.011, 0.030, 0.082, 0.224, 0.607]
```

### Exercise 10.3: Linear Regression (***)

Extend the training loop example to learn both a weight and a bias: `y = w * x + b`. Use `x = [1, 2, 3, 4, 5]` and `y = [3, 5, 7, 9, 11]` (the true relationship is `y = 2x + 1`). Train for 200 epochs and print the final `w` and `b`.

**Expected output (approximate):**

```
w: 2.0, b: 1.0
```
