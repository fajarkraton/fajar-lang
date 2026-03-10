# Your First ML Model on Bare Metal — Fajar Lang Tutorial

Learn how to build, train, and deploy a neural network for embedded inference using Fajar Lang.

## Prerequisites

- Fajar Lang installed (`cargo install fajar-lang` or build from source)
- Basic understanding of neural networks
- (Optional) QEMU for cross-platform testing

## Step 1: Define the Model Architecture

Our goal: classify sensor data into 3 categories (idle, moving, alert) using a simple 2-layer neural network.

```fajar
// Architecture: 4 inputs → 4 hidden (ReLU) → 3 outputs (Softmax)
const num_inputs: i64 = 4
const num_hidden: i64 = 4
const num_outputs: i64 = 3
```

**Why this architecture?**
- 4 inputs: typical IMU sensor (accel_x, accel_y, accel_z, gyro_z)
- 4 hidden neurons: small enough for MCU, large enough to learn patterns
- 3 outputs: our classification categories

## Step 2: Initialize Weights

Xavier initialization keeps gradients stable during training:

```fajar
fn init_model() {
    let w1 = tensor_xavier(4, 4)    // 4x4 weight matrix
    let b1 = tensor_zeros([1, 4])   // bias vector
    let w2 = tensor_xavier(4, 3)    // 4x3 weight matrix
    let b2 = tensor_zeros([1, 3])   // bias vector
}
```

## Step 3: Forward Pass

The forward pass transforms input → prediction:

```fajar
fn forward(input: Tensor, w1: Tensor, b1: Tensor, w2: Tensor, b2: Tensor) -> Tensor {
    // Hidden layer: ReLU activation
    let z1 = tensor_add(tensor_matmul(input, w1), b1)
    let a1 = tensor_relu(z1)

    // Output layer: Softmax for probabilities
    let z2 = tensor_add(tensor_matmul(a1, w2), b2)
    let output = tensor_softmax(z2)
    output
}
```

**What happens at each step:**
1. `tensor_matmul(input, w1)` — matrix multiply: [1,4] × [4,4] → [1,4]
2. `tensor_add(..., b1)` — add bias
3. `tensor_relu(z1)` — zero out negative values (non-linearity)
4. Same for layer 2, but with softmax (converts to probabilities)

## Step 4: Run Inference

```fajar
fn classify(sensor_data: Tensor) -> str {
    let prediction = tensor_argmax(forward(sensor_data, w1, b1, w2, b2))
    if prediction == 0 { "idle" }
    else if prediction == 1 { "moving" }
    else { "alert" }
}
```

`tensor_argmax` returns the index of the highest probability — our predicted class.

## Step 5: Pre-trained Weights for Deployment

For embedded deployment, hardcode trained weights instead of training on-device:

```fajar
fn load_w1() -> Tensor {
    tensor_from_data([
        0.5, -0.3, 0.1, 0.8,
        -0.2, 0.6, -0.4, 0.3,
        0.7, -0.1, 0.9, -0.5,
        -0.6, 0.4, -0.2, 0.7
    ], [4, 4])
}
```

In production, these weights come from training on a host machine and are stored in flash memory.

## Step 6: Sensor Reading Loop

```fajar
fn main() {
    let w1 = load_w1()
    let b1 = load_b1()
    let w2 = load_w2()
    let b2 = load_b2()

    for step in 0..100 {
        let sensor = read_imu()  // Read accelerometer + gyroscope
        let prediction = infer(sensor, w1, b1, w2, b2)
        let label = class_name(prediction)

        if prediction == 2 {
            // Alert! Take action
            activate_buzzer()
        }
    }
}
```

## Step 7: Cross-Compile for ARM64

Build for an embedded ARM target:

```bash
# Compile to aarch64 object file
fj build --target aarch64-unknown-linux-gnu examples/embedded_inference.fj

# Link with cross-compiler
aarch64-linux-gnu-gcc -static -o inference inference.o rt_entry.c

# Test on QEMU
qemu-aarch64 ./inference
```

## Step 8: no_std Verification

Fajar Lang can verify your code is bare-metal safe:

```fajar
// These are OK in no_std:
fn compute(a: i64, b: i64) -> i64 { a + b }
let result = tensor_argmax(output)  // Returns i64

// These would be flagged as violations:
// let data = read_file("config.txt")  // File I/O forbidden
// let t = tensor_zeros([100, 100])    // Heap allocation forbidden
```

The `nostd` checker catches:
- File I/O operations
- Heap-dependent tensor operations
- String allocations
- Dynamic memory usage

## Complete Example

See `examples/embedded_inference.fj` for a complete working example:

```bash
fj run examples/embedded_inference.fj
```

Output:
```
=== Fajar Lang Embedded Inference ===
Model: 4→4→3 (pre-trained, no heap at inference)

Running inference on 9 sensor readings:
  Step 0: predicted = alert
  Step 1: predicted = alert
  ...

=== Results ===
Idle: 0 | Moving: 0 | Alert: 9
```

## Next Steps

- **Stack tensors**: Use `StackTensor<N>` for zero-allocation inference (see `src/runtime/ml/stack_tensor.rs`)
- **Fixed-point math**: Use `Q16_16` or `Q8_8` for MCUs without FPU (see `src/runtime/ml/fixed_point.rs`)
- **Cross-domain bridge**: Combine `@kernel` (OS) and `@device` (ML) in a single program
- **Real hardware**: Flash the binary to an STM32 or Raspberry Pi Pico

## Key Takeaways

1. **Fajar Lang makes embedded ML first-class** — tensor operations are built into the language
2. **No Python dependency** — train and deploy from the same language
3. **Memory-safe by default** — the compiler catches unsafe patterns
4. **Cross-compilation built in** — target ARM64, RISC-V from any host
