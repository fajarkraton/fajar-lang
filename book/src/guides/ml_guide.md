# Machine Learning Guide

Fajar Lang has first-class support for machine learning with native tensor types,
automatic differentiation, and a full training pipeline. This guide walks through
the complete ML workflow.

## Tensor Basics

Tensors are first-class citizens in Fajar Lang:

```fajar
let a = zeros(3, 4)             // 3x4 zero matrix
let b = ones(2, 2)              // 2x2 ones matrix
let c = randn(64, 128)          // Random normal 64x128
let d = eye(4)                  // 4x4 identity matrix
let e = from_data([[1.0, 2.0], [3.0, 4.0]])
```

### Operations

```fajar
let sum = a + b
let product = matmul(a, transpose(b))
let reshaped = reshape(c, [128, 64])
let sliced = a[0:2, 1:3]        // Slice rows 0-1, cols 1-2
```

## Building a Model

Define layers using the `layer` keyword:

```fajar
let model = Sequential([
    Dense(784, 128),
    relu,
    Dense(128, 64),
    relu,
    Dense(64, 10),
    softmax,
])
```

### Custom Layers

```fajar
struct MyAttention {
    query: Dense,
    key: Dense,
    value: Dense,
}

impl MyAttention {
    fn new(dim: i32) -> MyAttention {
        MyAttention {
            query: Dense(dim, dim),
            key: Dense(dim, dim),
            value: Dense(dim, dim),
        }
    }

    fn forward(self, x: Tensor) -> Tensor {
        let q = self.query.forward(x)
        let k = self.key.forward(x)
        let v = self.value.forward(x)
        let scores = matmul(q, transpose(k)) / sqrt(q.shape[1] as f64)
        matmul(softmax(scores), v)
    }
}
```

## Training Pipeline

### Step 1: Prepare Data

```fajar
let (train_images, train_labels) = load_mnist_train()
let (test_images, test_labels) = load_mnist_test()

// Normalize to [0, 1]
let x_train = train_images / 255.0
let x_test = test_images / 255.0
```

### Step 2: Define Model and Optimizer

```fajar
let model = Sequential([
    Dense(784, 128), relu,
    Dense(128, 10), softmax,
])

let optimizer = Adam(lr: 0.001)
let loss_fn = cross_entropy
```

### Step 3: Training Loop

```fajar
for epoch in 0..10 {
    let total_loss = 0.0

    for (batch_x, batch_y) in dataloader(x_train, train_labels, batch_size: 32) {
        // Forward pass
        let predictions = model.forward(batch_x)
        let loss = loss_fn(predictions, batch_y)

        // Backward pass
        loss.backward()

        // Update weights
        optimizer.step(model.parameters())
        optimizer.zero_grad()

        total_loss += loss.item()
    }

    println(f"Epoch {epoch}: loss = {total_loss}")
}
```

### Step 4: Evaluate

```fajar
let test_pred = model.forward(x_test)
let acc = accuracy(test_pred, test_labels)
println(f"Test accuracy: {acc * 100.0}%")
```

## Quantization for Embedded

Reduce model size with INT8 quantization:

```fajar
let quantized = quantize_int8(model)
let size_reduction = model.size() / quantized.size()
println(f"Size reduced {size_reduction}x")

// Inference still works
let pred = quantized.forward(input)
```

## ONNX Export/Import

```fajar
// Export trained model
onnx_export(model, "model.onnx")

// Import pre-trained model
let imported = onnx_import("resnet18.onnx")
let result = imported.forward(input_tensor)
```

## Deployment on Embedded

```fajar
@device
fn run_inference(sensor_data: [f32; 4]) -> i32 {
    let input = Tensor::from_slice(sensor_data)
    let model = load_quantized("model.bin")
    let output = model.forward(input)
    argmax(output)
}
```

## Loss Functions

| Function | Use Case |
|----------|----------|
| `mse_loss` | Regression |
| `cross_entropy` | Multi-class classification |
| `bce_loss` | Binary classification |
| `l1_loss` | Sparse regression |

## Optimizers

| Optimizer | Description |
|-----------|-------------|
| `SGD(lr, momentum)` | Stochastic gradient descent |
| `Adam(lr)` | Adaptive moment estimation |

## Metrics

```fajar
let acc = accuracy(predictions, labels)
let prec = precision(predictions, labels)
let rec = recall(predictions, labels)
let f1 = f1_score(predictions, labels)
```
