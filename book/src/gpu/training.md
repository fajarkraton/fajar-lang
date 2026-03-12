# GPU-Accelerated Training

Fajar Lang supports GPU-accelerated neural network training with automatic differentiation.

## Basic GPU Training

```fajar
@device
fn train_mnist() {
    let w1 = tensor_xavier(784, 128)
    let w2 = tensor_xavier(128, 10)
    let lr = 0.01

    let mut epoch = 0
    while epoch < 10 {
        let input = load_batch("mnist_train", 32)  // Batch size 32
        let target = load_labels("mnist_train", 32)

        // Forward pass
        let hidden = tensor_relu(tensor_matmul(input, w1))
        let output = tensor_softmax(tensor_matmul(hidden, w2))

        // Loss
        let loss = cross_entropy(output, target)

        // Backward pass (autograd)
        backward(loss)
        let grad_w1 = grad(w1)
        let grad_w2 = grad(w2)

        // Update weights
        w1 = tensor_sub(w1, tensor_mul_scalar(grad_w1, lr))
        w2 = tensor_sub(w2, tensor_mul_scalar(grad_w2, lr))

        zero_grad()
        epoch = epoch + 1
    }
}
```

## Mixed Precision Training

Use FP16 for forward/backward, FP32 for weight updates:

```fajar
@device
fn mixed_precision_step(model: Model, input: Tensor, target: Tensor) {
    // Cast to FP16 for speed
    let input_fp16 = tensor_cast(input, DType::FP16)
    let output_fp16 = forward(model, input_fp16)

    // Loss in FP32 for stability
    let loss = cross_entropy(tensor_cast(output_fp16, DType::FP32), target)

    // Scale loss to prevent FP16 underflow
    let scaled_loss = tensor_mul_scalar(loss, 1024.0)
    backward(scaled_loss)

    // Update in FP32
    optimizer.step()
    optimizer.zero_grad()
}
```

## Multi-GPU Data Parallelism

```fajar
@device
fn distributed_train(model: Model, dataset: DataLoader) {
    let world_size = gpu_count()

    for batch in dataset {
        // Scatter data across GPUs
        let local_batch = scatter(batch, world_size)

        // Each GPU computes forward + backward
        let local_loss = forward_backward(model, local_batch)

        // All-reduce gradients
        all_reduce_gradients(model)

        // Synchronized weight update
        optimizer.step()
    }
}
```

## Optimizers

| Optimizer | Description |
|-----------|-------------|
| `SGD` | Stochastic gradient descent with momentum |
| `Adam` | Adaptive learning rate |
| `AdamW` | Adam with decoupled weight decay |
| `RMSprop` | Root mean square propagation |

## Learning Rate Schedulers

```fajar
let scheduler = CosineAnnealing { initial_lr: 0.01, T_max: 100 }
// Also: StepLR, ExponentialLR, WarmupCosine, OneCycleLR
```

## Model Optimization Pipeline

```
Train → Prune → Distill → Quantize (INT8) → Export (ONNX)
```

Each step reduces model size and inference latency while preserving accuracy.
