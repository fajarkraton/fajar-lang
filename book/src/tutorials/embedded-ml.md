# Your First ML Model on Bare Metal

This tutorial walks through building a simple neural network that runs on embedded hardware using Fajar Lang.

## Step 1: Define the Model

```fajar
@device fn forward(input: Tensor, w1: Tensor, w2: Tensor) -> Tensor {
    let hidden = relu(matmul(input, w1))
    let output = sigmoid(matmul(hidden, w2))
    output
}
```

## Step 2: Initialize Weights

```fajar
let w1 = xavier(4, 8)     // 4 inputs -> 8 hidden
let w2 = xavier(8, 1)     // 8 hidden -> 1 output
```

## Step 3: Training Loop

```fajar
let lr = 0.01
let optimizer = SGD::new(lr)

for epoch in 0..100 {
    let pred = forward(x_train, w1, w2)
    let loss = mse_loss(pred, y_train)
    backward(loss)
    optimizer.step()
    optimizer.zero_grad()
}
```

## Step 4: Quantize for Embedded

```fajar
let w1_q = quantize_int8(w1)
let w2_q = quantize_int8(w2)
// INT8 inference uses integer-only arithmetic (no FPU needed)
```

## Step 5: Deploy to Bare Metal

```fajar
@kernel fn main() {
    let sensor_data = read_sensors()
    let input = Tensor::from_slice(sensor_data)
    let result = forward(input, w1_q, w2_q)
    if result > 0.5 {
        activate_motor()
    }
}
```

## Cross-Domain Bridge

The `@safe` context bridges `@kernel` (hardware) and `@device` (ML):

```fajar
@kernel fn read_sensors() -> [f32; 4] { /* hardware access */ }
@device fn predict(x: Tensor) -> Tensor { /* ML inference */ }

@safe fn control_loop() {
    let raw = read_sensors()
    let tensor = Tensor::from_slice(raw)
    let action = predict(tensor)
    execute_action(action)
}
```

The compiler enforces that `@kernel` code never touches tensors, and `@device` code never touches raw hardware. Violations are compile-time errors, not runtime crashes.
