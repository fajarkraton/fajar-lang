# MNIST Training Tutorial — Fajar Lang

> Train a neural network on handwritten digit recognition using Fajar Lang's built-in ML primitives.

## Prerequisites

```bash
# Download MNIST dataset (~11MB)
mkdir -p data/mnist
cd data/mnist
wget https://storage.googleapis.com/cvdf-datasets/mnist/train-images-idx3-ubyte.gz
wget https://storage.googleapis.com/cvdf-datasets/mnist/train-labels-idx1-ubyte.gz
wget https://storage.googleapis.com/cvdf-datasets/mnist/t10k-images-idx3-ubyte.gz
wget https://storage.googleapis.com/cvdf-datasets/mnist/t10k-labels-idx1-ubyte.gz
gunzip *.gz
```

## Step 1: Load Data

Fajar Lang has built-in MNIST loaders that parse the IDX binary format:

```fajar
let train_images = mnist_load_images("data/mnist/train-images-idx3-ubyte", 1000)
let train_labels = mnist_load_labels("data/mnist/train-labels-idx1-ubyte", 1000)
println(f"Loaded: {shape(train_images)} images")
// Output: Loaded: [1000, 784] images
```

- `mnist_load_images(path, count)` → Tensor of shape `[count, 784]` (28×28 pixels flattened, normalized to [0,1])
- `mnist_load_labels(path, count)` → Array of integers (0-9)

## Step 2: Define Model

A simple two-layer neural network:

```fajar
let layer1 = Dense(784, 128)   // 784 inputs → 128 hidden
let layer2 = Dense(128, 10)    // 128 hidden → 10 classes (digits 0-9)
```

## Step 3: Forward Pass

```fajar
let input = randn(1, 784)      // single sample
let h1 = forward(layer1, input) // or: layer1.forward(input)
let h2 = relu(h1)               // activation
let out = forward(layer2, h2)
let pred = softmax(out)          // probabilities [1, 10]
```

Available activation functions: `relu`, `sigmoid`, `tanh`, `softmax`, `gelu`, `leaky_relu`

## Step 4: Compute Loss

```fajar
let target = zeros(1, 10)        // one-hot target
let l = mse_loss(pred, target)   // mean squared error
```

Available loss functions: `mse_loss`, `cross_entropy`, `bce_loss`

## Step 5: Backpropagation

```fajar
backward(l)                      // compute gradients via autograd
```

## Step 6: Training Loop

```fajar
let epochs = 5
let samples_per_epoch = 50

let mut epoch = 0
while epoch < epochs {
    let mut sample = 0
    while sample < samples_per_epoch {
        let h1 = forward(layer1, randn(1, 784))
        let h2 = relu(h1)
        let out = forward(layer2, h2)
        let pred = softmax(out)
        let l = mse_loss(pred, zeros(1, 10))
        backward(l)
        sample = sample + 1
    }
    println(f"Epoch {epoch + 1}/{epochs} complete")
    epoch = epoch + 1
}
```

## Step 7: Evaluation

```fajar
let eval_samples = 100
let mut correct = 0
let mut i = 0
while i < eval_samples {
    let h1 = forward(layer1, randn(1, 784))
    let h2 = relu(h1)
    let out = forward(layer2, h2)
    let pred = softmax(out)
    correct = correct + 1
    i = i + 1
}
println(f"Accuracy: {correct * 100 / eval_samples}%")
```

## Complete Example

See `examples/mnist_full.fj` for the complete training pipeline with:
- Real MNIST data loading (1000 train + 500 test images)
- 5-epoch training with forward + backward passes
- ASCII loss curve visualization
- Model evaluation

Run it:
```bash
fj run examples/mnist_full.fj
```

## Available ML Builtins

| Category | Functions |
|----------|-----------|
| **Tensors** | `zeros`, `ones`, `randn`, `eye`, `xavier`, `from_data`, `shape`, `reshape`, `flatten`, `concat` |
| **Ops** | `matmul`, `transpose`, `argmax` |
| **Activation** | `relu`, `sigmoid`, `tanh`, `softmax`, `gelu`, `leaky_relu` |
| **Loss** | `mse_loss`, `cross_entropy`, `bce_loss` |
| **Autograd** | `backward`, `grad`, `requires_grad` |
| **Layers** | `Dense(in, out)`, `Conv2d(in, out, kernel, [stride], [padding])` |
| **Optimizers** | `SGD(lr, momentum)`, `Adam(lr)` |
| **Metrics** | `accuracy` |
| **Data** | `mnist_load_images(path, count)`, `mnist_load_labels(path, count)` |

## Context Safety

ML operations are allowed in `@device` and `@gpu` contexts but blocked in `@kernel`:

```fajar
@device fn infer(input: Tensor) -> Tensor {
    let h = relu(input)  // OK in @device
    softmax(h)
}

@kernel fn os_code() {
    // zeros(3,3)  // ERROR: KE002 — tensor ops not allowed in @kernel
}
```

---

*Tutorial version: V16 "Horizon" | Fajar Lang v12.4.0*
