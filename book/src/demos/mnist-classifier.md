# Demo: MNIST Classifier

A neural network training pipeline demonstrating Fajar Lang's ML stack with native compilation.

## Architecture

```
Input (784) → Dense(128, ReLU) → Dense(10, Softmax) → Cross-Entropy Loss → SGD Update
```

## Key Features

- **First-class tensors**: `zeros`, `xavier`, `randn`, `matmul`, `softmax`
- **Autograd**: `backward()`, `grad()`, `zero_grad()`, `set_requires_grad()`
- **Optimizers**: SGD with learning rate
- **Loss functions**: `cross_entropy` for multi-class classification
- **Training loop**: Epoch-based with loss tracking

## Training Pipeline

```fajar
// Forward pass
let hidden = relu(matmul(input, w1))
let output = softmax(matmul(hidden, w2))
let loss_val = cross_entropy(output, target)

// Backward + update
backward(loss_val)
zero_grad(w1)
zero_grad(w2)
```

## Native Compilation

The classifier compiles to native code via Cranelift for maximum performance:

```bash
fj build examples/mnist_classifier.fj --output mnist
./mnist
```

## Source

See `examples/mnist_classifier.fj` and `examples/mnist_native.fj`.
