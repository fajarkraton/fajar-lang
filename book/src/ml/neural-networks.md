# Neural Networks

## Layers

Fajar provides built-in neural network layers:

```fajar
let dense = Dense(784, 128)              // fully connected
let conv = Conv2d(3, 16, 3)              // convolution
let attn = MultiHeadAttention(512, 8)    // attention
let bn = BatchNorm(128)                  // batch normalization
let drop = Dropout(0.5)                  // dropout
let emb = Embedding(10000, 256)          // word embeddings
```

## Forward Pass

```fajar
@device fn forward(x: Tensor) -> Tensor {
    let h1 = relu(dense1.forward(x))
    let h2 = relu(dense2.forward(h1))
    softmax(output_layer.forward(h2))
}
```

## Loss Functions

```fajar
let l = mse_loss(predicted, target)         // mean squared error
let l = cross_entropy(predicted, target)    // cross-entropy
let l = bce_loss(predicted, target)         // binary cross-entropy
let l = l1_loss(predicted, target)          // L1 (MAE)
```

## Optimizers

```fajar
let sgd = SGD::new(0.01)                   // learning rate 0.01
let adam = Adam::new(0.001)                 // Adam optimizer

// Training step
optimizer.step()
optimizer.zero_grad()
```

## Metrics

```fajar
let acc = accuracy(predicted, labels)
let prec = precision(predicted, labels)
let rec = recall(predicted, labels)
let f1 = f1_score(predicted, labels)
```

## Complete Example

```fajar
@device fn train_step(x: Tensor, y: Tensor, w: Tensor) -> f64 {
    let pred = sigmoid(matmul(x, w))
    let loss = mse_loss(pred, y)
    backward(loss)
    loss
}
```
