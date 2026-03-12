# Neural Networks

## Layers

Fajar provides built-in neural network layers:

```fajar
// Fully connected
let dense = Dense(784, 128)

// Convolution
let conv = Conv2d(3, 16, 3)              // in_channels, out_channels, kernel_size

// Attention
let attn = MultiHeadAttention(512, 8)    // embed_dim, num_heads

// Normalization
let bn = BatchNorm(128)                  // features
let ln = LayerNorm(512)                  // features

// Recurrent
let lstm = LSTM(128, 64)                 // input_size, hidden_size
let gru = GRU(128, 64)                  // 25% fewer params than LSTM

// Other
let drop = Dropout(0.5)                 // dropout rate
let emb = Embedding(10000, 256)         // vocab_size, embed_dim
```

## Forward Pass

```fajar
@device fn forward(x: Tensor) -> Tensor {
    let h1 = relu(dense1.forward(x))
    let h2 = relu(dense2.forward(h1))
    softmax(output_layer.forward(h2))
}
```

## Recurrent Layers

### LSTM (Long Short-Term Memory)

4 gates: forget, input, output, candidate. Handles long-range dependencies:

```fajar
@device fn sequence_model(input: Tensor) -> Tensor {
    let lstm = LSTM(128, 64)
    let mut hidden = zeros(1, 64)
    let mut cell = zeros(1, 64)

    for t in 0..seq_len {
        let x_t = input.row(t)
        let (h, c) = lstm.forward(x_t, hidden, cell)
        hidden = h
        cell = c
    }
    hidden  // Final hidden state
}
```

### GRU (Gated Recurrent Unit)

2 gates: reset, update. Simpler and faster than LSTM:

```fajar
@device fn gru_model(input: Tensor) -> Tensor {
    let gru = GRU(128, 64)
    let mut hidden = zeros(1, 64)

    for t in 0..seq_len {
        hidden = gru.forward(input.row(t), hidden)
    }
    hidden
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
let sgd = SGD::new(0.01)                   // SGD with learning rate
let sgd_m = SGD::new(0.01, momentum: 0.9)  // SGD with momentum
let adam = Adam::new(0.001)                 // Adam
let adamw = AdamW::new(0.001, weight_decay: 0.01)  // Decoupled weight decay

// Training step
optimizer.step()
optimizer.zero_grad()
```

## Learning Rate Schedulers

```fajar
let scheduler = StepLR { initial_lr: 0.01, step_size: 30, gamma: 0.1 }
let scheduler = CosineAnnealing { initial_lr: 0.01, T_max: 100 }
let scheduler = WarmupCosine { warmup_steps: 1000, initial_lr: 0.01 }
let scheduler = OneCycleLR { max_lr: 0.01, total_steps: 10000 }

// After each epoch:
scheduler.step()
let current_lr = scheduler.get_lr()
```

## Data Loading

```fajar
let loader = DataLoader {
    dataset: "mnist_train",
    batch_size: 32,
    shuffle: true,
}

for (batch_x, batch_y) in loader {
    let loss = train_step(batch_x, batch_y)
}
```

## Metrics

```fajar
let acc = accuracy(predicted, labels)
let prec = precision(predicted, labels)
let rec = recall(predicted, labels)
let f1 = f1_score(predicted, labels)
```

## Activation Functions

| Function | Formula | Use Case |
|----------|---------|----------|
| `relu(x)` | max(0, x) | Hidden layers (default) |
| `sigmoid(x)` | 1/(1+e^-x) | Binary classification |
| `tanh(x)` | (e^x-e^-x)/(e^x+e^-x) | RNN hidden states |
| `softmax(x)` | e^xi / Σe^xj | Multi-class output |
| `gelu(x)` | x·Φ(x) | Transformer layers |
| `leaky_relu(x)` | max(0.01x, x) | Prevents dead neurons |

## Complete Example

```fajar
@device fn train_mnist() {
    let w1 = xavier(784, 128)
    let w2 = xavier(128, 10)
    let optimizer = Adam::new(0.001)

    let loader = DataLoader { dataset: "mnist_train", batch_size: 32, shuffle: true }

    let mut epoch = 0
    while epoch < 10 {
        for (x, y) in loader {
            let hidden = relu(matmul(x, w1))
            let output = softmax(matmul(hidden, w2))
            let loss = cross_entropy(output, y)

            backward(loss)
            optimizer.step()
            optimizer.zero_grad()
        }
        epoch = epoch + 1
    }
}
```
