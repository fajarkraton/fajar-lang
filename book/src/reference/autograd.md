# Autograd and Training

Fajar Lang includes a tape-based automatic differentiation engine for training neural networks. All autograd operations require `@device` or `@unsafe` context.

## Gradient Tracking

### `requires_grad(t: Tensor) -> bool`

Returns whether a tensor is tracking gradients.

```fajar
let w = randn(3, 3)
println(requires_grad(w))  // false
```

### `set_requires_grad(t: &mut Tensor, flag: bool) -> void`

Enables or disables gradient tracking for a tensor.

```fajar
let mut w = randn(3, 3)
set_requires_grad(w, true)
println(requires_grad(w))  // true
```

### `backward() -> void`

Computes gradients for all tensors in the computation graph by backpropagation. Call on a scalar loss tensor.

```fajar
@device
fn train_step() {
    let mut w = randn(2, 1)
    set_requires_grad(w, true)

    let x = from_data([[1.0, 2.0], [3.0, 4.0]])
    let y = from_data([[5.0], [11.0]])

    let pred = matmul(x, w)
    let loss = mse_loss(pred, y)

    backward()
}
```

### `grad(t: Tensor) -> Tensor`

Retrieves the gradient of a tensor after `backward()` has been called.

```fajar
backward()
let gradient = grad(w)
println(gradient)  // gradient tensor
```

### `zero_grad() -> void`

Resets all accumulated gradients to zero. Call before each training step.

```fajar
for epoch in 0..100 {
    zero_grad()
    let loss = forward_pass()
    backward()
    optimizer.step()
}
```

## Loss Functions

| Function | Signature | Description |
|----------|-----------|-------------|
| `mse_loss` | `mse_loss(pred: Tensor, target: Tensor) -> Tensor` | Mean Squared Error |
| `cross_entropy` | `cross_entropy(pred: Tensor, target: Tensor) -> Tensor` | Cross-Entropy Loss |
| `bce_loss` | `bce_loss(pred: Tensor, target: Tensor) -> Tensor` | Binary Cross-Entropy |
| `l1_loss` | `l1_loss(pred: Tensor, target: Tensor) -> Tensor` | Mean Absolute Error |

### Loss Examples

```fajar
@device
fn compute_losses() {
    let pred = from_data([[0.8, 0.1, 0.1]])
    let target = from_data([[1.0, 0.0, 0.0]])

    let mse = mse_loss(pred, target)
    let ce = cross_entropy(pred, target)
    let bce = bce_loss(sigmoid(pred), target)
    let l1 = l1_loss(pred, target)
}
```

## Optimizers

### SGD (Stochastic Gradient Descent)

Creates an SGD optimizer with learning rate and optional momentum.

```fajar
@device
fn train_with_sgd() {
    let mut w = randn(10, 1)
    set_requires_grad(w, true)

    let optimizer = SGD(lr: 0.01, momentum: 0.9)

    for epoch in 0..100 {
        zero_grad()
        let pred = matmul(x, w)
        let loss = mse_loss(pred, y)
        backward()
        optimizer.step()
    }
}
```

### Adam

Creates an Adam optimizer with learning rate (uses default beta1=0.9, beta2=0.999).

```fajar
@device
fn train_with_adam() {
    let mut w = randn(10, 1)
    set_requires_grad(w, true)

    let optimizer = Adam(lr: 0.001)

    for epoch in 0..100 {
        zero_grad()
        let pred = matmul(x, w)
        let loss = cross_entropy(pred, y)
        backward()
        optimizer.step()
    }
}
```

## Metrics

| Function | Signature | Description |
|----------|-----------|-------------|
| `accuracy` | `accuracy(pred: Tensor, target: Tensor) -> f64` | Classification accuracy |
| `precision` | `precision(pred: Tensor, target: Tensor) -> f64` | Precision score |
| `recall` | `recall(pred: Tensor, target: Tensor) -> f64` | Recall score |
| `f1_score` | `f1_score(pred: Tensor, target: Tensor) -> f64` | F1 harmonic mean |

```fajar
@device
fn evaluate(model_pred: Tensor, labels: Tensor) {
    let acc = accuracy(model_pred, labels)
    let prec = precision(model_pred, labels)
    let rec = recall(model_pred, labels)
    let f1 = f1_score(model_pred, labels)

    println(f"Accuracy:  {acc}")
    println(f"Precision: {prec}")
    println(f"Recall:    {rec}")
    println(f"F1 Score:  {f1}")
}
```

## Complete Training Loop

```fajar
@device
fn train_mnist() {
    // Load data
    let x_train = randn(1000, 784)
    let y_train = randn(1000, 10)

    // Initialize weights
    let mut w1 = randn(784, 128)
    let mut b1 = zeros(1, 128)
    let mut w2 = randn(128, 10)
    let mut b2 = zeros(1, 10)

    set_requires_grad(w1, true)
    set_requires_grad(b1, true)
    set_requires_grad(w2, true)
    set_requires_grad(b2, true)

    let optimizer = Adam(lr: 0.001)

    for epoch in 0..50 {
        zero_grad()

        // Forward
        let h = relu(matmul(x_train, w1) + b1)
        let out = softmax(matmul(h, w2) + b2)

        // Loss
        let loss = cross_entropy(out, y_train)

        // Backward
        backward()
        optimizer.step()

        if epoch % 10 == 0 {
            let acc = accuracy(out, y_train)
            println(f"Epoch {epoch}: acc = {acc}")
        }
    }
}
```
