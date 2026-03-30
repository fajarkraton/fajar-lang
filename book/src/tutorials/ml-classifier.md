# ML Image Classifier

Build a neural network classifier using Fajar Lang's built-in tensor operations and autograd.

## What You'll Build

A simple neural network that:
- Creates weight tensors
- Performs matrix multiplication (forward pass)
- Applies activation functions (relu, softmax)
- Computes loss and runs backward pass (autograd)

## Step 1: Create the Model

```fajar
fn main() {
    println("=== Neural Network Classifier ===")

    // Create weight matrices
    let w1 = randn(4, 8)    // Input layer: 4 features → 8 hidden
    let w2 = randn(8, 3)    // Hidden → 3 classes

    // Require gradients for training
    set_requires_grad(w1, true)
    set_requires_grad(w2, true)

    println(f"Layer 1: 4x8 weights")
    println(f"Layer 2: 8x3 weights")
```

## Step 2: Forward Pass

```fajar
    // Create input: batch of 2 samples, 4 features each
    let x = randn(2, 4)

    // Forward pass
    let h = matmul(x, w1)      // Linear: x @ w1
    let h_act = relu(h)         // Activation: ReLU
    let logits = matmul(h_act, w2)  // Linear: h @ w2
    let probs = softmax(logits)     // Output: softmax probabilities

    println(f"Input shape: 2x4")
    println(f"Output probabilities computed")
```

## Step 3: Compute Loss

```fajar
    // Create target (one-hot: class 0 for sample 1, class 2 for sample 2)
    let target = zeros(2, 3)

    // Cross-entropy loss
    let loss_val = cross_entropy(probs, target)
    println(f"Loss: {loss_val}")
```

## Step 4: Backward Pass (Autograd)

```fajar
    // Backward pass — computes gradients for w1 and w2
    backward(loss_val)

    // Check gradients
    let g1 = grad(w1)
    let g2 = grad(w2)
    println("Gradients computed for both layers")
```

## Step 5: Training Loop

```fajar
    // Simple SGD training loop
    let lr = 0.01
    let epochs = 5
    let mut epoch = 0
    while epoch < epochs {
        // Forward
        let h = relu(matmul(x, w1))
        let out = softmax(matmul(h, w2))
        let loss = cross_entropy(out, target)

        // Backward
        backward(loss)

        println(f"Epoch {epoch}: loss = {loss}")
        epoch = epoch + 1
    }

    println("Training complete")
}
```

## Key Concepts

| Operation | Builtin |
|-----------|---------|
| Create tensor | `zeros(rows, cols)`, `ones(r, c)`, `randn(r, c)` |
| Matrix multiply | `matmul(a, b)` |
| Activations | `relu(t)`, `sigmoid(t)`, `softmax(t)`, `tanh(t)` |
| Loss functions | `cross_entropy(pred, target)`, `mse_loss(pred, target)` |
| Autograd | `set_requires_grad(t, true)`, `backward(loss)`, `grad(t)` |
| Reshape | `reshape(t, rows, cols)`, `transpose(t)` |

## What Makes Fajar Lang Special for ML

1. **Tensors are first-class types** — not a library, part of the language
2. **Autograd is built-in** — tape-based reverse-mode differentiation
3. **@device context** — compile-time isolation for ML code (no raw pointers)
4. **Cross-domain bridge** — `@kernel` sensor data → `@device` inference in one codebase

## Full Source

See [`examples/ml_inference_api.fj`](https://github.com/fajarkraton/fajar-lang/blob/main/examples/ml_inference_api.fj)
