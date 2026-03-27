# Neural Network Layers

Fajar Lang provides built-in neural network layer types for constructing models. All layers require `@device` or `@unsafe` context.

## Dense (Fully Connected)

A linear transformation layer: `output = input @ W + b`.

### Constructor

```fajar
Dense(in_features: i64, out_features: i64) -> Dense
```

### Forward

```fajar
dense.forward(input: Tensor) -> Tensor
```

### Parameters

| Parameter | Shape | Description |
|-----------|-------|-------------|
| `weight` | `[in_features, out_features]` | Weight matrix |
| `bias` | `[1, out_features]` | Bias vector |

### Example

```fajar
@device
fn example_dense() {
    let layer = Dense(784, 128)
    let input = randn(32, 784)     // batch of 32, 784 features
    let output = layer.forward(input)  // shape: [32, 128]
}
```

## Conv2d (2D Convolution)

Applies a 2D convolution over an input signal.

### Constructor

```fajar
Conv2d(
    in_channels: i64,
    out_channels: i64,
    kernel_size: i64,
) -> Conv2d
```

### Forward

```fajar
conv.forward(input: Tensor) -> Tensor
```

### Parameters

| Parameter | Shape | Description |
|-----------|-------|-------------|
| `weight` | `[out_ch, in_ch, kH, kW]` | Convolution kernels |
| `bias` | `[out_ch]` | Per-channel bias |

### Example

```fajar
@device
fn example_conv() {
    let conv = Conv2d(1, 32, 3)          // 1 input channel, 32 output, 3x3 kernel
    let input = randn(8, 1, 28, 28)     // batch of 8, 1 channel, 28x28
    let output = conv.forward(input)     // shape: [8, 32, 26, 26]
}
```

## MultiHeadAttention

Multi-head self-attention mechanism used in transformers.

### Constructor

```fajar
MultiHeadAttention(
    embed_dim: i64,
    num_heads: i64,
) -> MultiHeadAttention
```

### Forward

```fajar
attn.forward(query: Tensor, key: Tensor, value: Tensor) -> Tensor
```

### Parameters

| Parameter | Shape | Description |
|-----------|-------|-------------|
| `W_q` | `[embed_dim, embed_dim]` | Query projection |
| `W_k` | `[embed_dim, embed_dim]` | Key projection |
| `W_v` | `[embed_dim, embed_dim]` | Value projection |
| `W_o` | `[embed_dim, embed_dim]` | Output projection |

### Example

```fajar
@device
fn example_attention() {
    let attn = MultiHeadAttention(512, 8)  // 512 dim, 8 heads
    let x = randn(16, 10, 512)            // batch 16, seq_len 10, dim 512
    let output = attn.forward(x, x, x)    // self-attention
}
```

## BatchNorm

Batch normalization: normalizes activations to zero mean and unit variance.

### Constructor

```fajar
BatchNorm(num_features: i64) -> BatchNorm
```

### Forward

```fajar
bn.forward(input: Tensor) -> Tensor
```

### Parameters

| Parameter | Shape | Description |
|-----------|-------|-------------|
| `gamma` | `[num_features]` | Scale parameter (learned) |
| `beta` | `[num_features]` | Shift parameter (learned) |
| `running_mean` | `[num_features]` | Running mean (tracking) |
| `running_var` | `[num_features]` | Running variance (tracking) |

### Example

```fajar
@device
fn example_batchnorm() {
    let bn = BatchNorm(128)
    let x = randn(32, 128)
    let normed = bn.forward(x)  // normalized output
}
```

## Dropout

Randomly zeroes elements during training to prevent overfitting.

### Constructor

```fajar
Dropout(rate: f64) -> Dropout
```

### Forward

```fajar
dropout.forward(input: Tensor) -> Tensor
```

### Example

```fajar
@device
fn example_dropout() {
    let drop = Dropout(0.5)           // 50% dropout rate
    let x = randn(32, 256)
    let output = drop.forward(x)      // ~50% of values zeroed during training
}
```

## Embedding

Lookup table mapping integer indices to dense vectors.

### Constructor

```fajar
Embedding(num_embeddings: i64, embedding_dim: i64) -> Embedding
```

### Forward

```fajar
embed.forward(indices: Tensor) -> Tensor
```

### Parameters

| Parameter | Shape | Description |
|-----------|-------|-------------|
| `weight` | `[num_embeddings, embedding_dim]` | Embedding table |

### Example

```fajar
@device
fn example_embedding() {
    let embed = Embedding(10000, 256)    // vocab size 10k, dim 256
    let tokens = from_data([[1.0, 42.0, 7.0]])  // 3 token indices
    let vectors = embed.forward(tokens)  // shape: [1, 3, 256]
}
```

## Building a Model

Combine layers to build a complete network:

```fajar
@device
fn build_classifier() {
    // Layers
    let conv1 = Conv2d(1, 32, 3)
    let conv2 = Conv2d(32, 64, 3)
    let bn1 = BatchNorm(32)
    let bn2 = BatchNorm(64)
    let fc = Dense(64 * 5 * 5, 10)
    let drop = Dropout(0.25)

    // Forward pass
    let x = randn(16, 1, 28, 28)
    let h = relu(bn1.forward(conv1.forward(x)))
    let h = relu(bn2.forward(conv2.forward(h)))
    let h = flatten(h)
    let h = drop.forward(h)
    let output = softmax(fc.forward(h))
}
```
