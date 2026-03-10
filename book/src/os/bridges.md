# Cross-Domain Bridges

The bridge pattern connects `@kernel` (hardware) and `@device` (ML) contexts through `@safe` code.

## The Problem

Kernel code needs sensor data. ML code needs that data as tensors. But:
- `@kernel` cannot create tensors (KE002)
- `@device` cannot access hardware (DE001)

## The Solution

```fajar
@kernel fn read_sensors() -> [f32; 4] {
    [
        port_read(0x40) as f32,
        port_read(0x41) as f32,
        port_read(0x42) as f32,
        port_read(0x43) as f32
    ]
}

@device fn infer(input: Tensor) -> Tensor {
    let hidden = relu(matmul(input, weights))
    sigmoid(matmul(hidden, output_weights))
}

@safe fn control_loop() {
    let raw = read_sensors()                    // @kernel -> raw data
    let tensor = Tensor::from_slice(raw)        // convert to tensor
    let prediction = infer(tensor)              // @device -> ML inference
    let action = Action::from_prediction(prediction)
    execute(action)                              // back to @kernel
}
```

## Data Flow

```
@kernel (hardware)
    |
    | raw sensor data [f32; 4]
    v
@safe (bridge)
    |
    | Tensor::from_slice()
    v
@device (ML inference)
    |
    | prediction result
    v
@safe (bridge)
    |
    | convert to action
    v
@kernel (actuator control)
```

## Why Bridges Matter

- **Type-safe**: The compiler verifies each context only uses allowed operations
- **Zero overhead**: Context annotations are compile-time only, no runtime checks
- **Clear data flow**: The bridge pattern makes domain boundaries explicit
- **Testable**: Each context can be unit-tested independently
