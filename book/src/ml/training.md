# Training

## Training Loop

```fajar
let optimizer = Adam::new(0.001)

for epoch in 0..100 {
    let pred = forward(x_train, weights)
    let loss = cross_entropy(pred, y_train)
    backward(loss)
    optimizer.step()
    optimizer.zero_grad()

    if epoch % 10 == 0 {
        println("Epoch " + to_string(epoch) + " loss: " + to_string(loss))
    }
}
```

## Data Loading

```fajar
let loader = DataLoader::new(data, labels, 32)  // batch size 32
loader.shuffle()

while loader.has_next() {
    let batch = loader.next_batch()
    let batch_labels = loader.next_labels()
    // ... training step
}
loader.reset()
```

## Model Checkpointing

```fajar
// Save
checkpoint_save("model.ckpt", weights, epoch, loss)

// Load
let (weights, epoch, loss) = checkpoint_load("model.ckpt")
```

## MNIST Example

```fajar
// Load MNIST data
let images = parse_idx_images("train-images.idx")
let labels = parse_idx_labels("train-labels.idx")

// Simple neural network
let w1 = xavier(784, 128)
let w2 = xavier(128, 10)

for epoch in 0..10 {
    let h = relu(matmul(images, w1))
    let out = softmax(matmul(h, w2))
    let loss = cross_entropy(out, labels)
    backward(loss)
    // update weights...
}
```

## INT8 Quantization

For embedded deployment:

```fajar
let w_q = quantize_int8(weights)     // float32 -> int8
let w_f = dequantize(w_q)            // int8 -> float32
```
