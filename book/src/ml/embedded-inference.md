# Embedded Inference

## Quantized Models

Fajar supports INT8 quantization for inference without a floating-point unit:

```fajar
// Quantize trained weights
let w1_q = quantize_int8(w1)
let w2_q = quantize_int8(w2)

// INT8 inference uses only integer arithmetic
@device fn inference(input: Tensor) -> Tensor {
    let h = relu(matmul(input, w1_q))
    sigmoid(matmul(h, w2_q))
}
```

## Cross-Compilation

Build for embedded targets:

```bash
fj build --target aarch64 --release model.fj   # ARM Cortex-A
fj build --target riscv64 --release model.fj   # RISC-V
```

## No-FPU Targets

INT8 quantized models work on processors without floating-point hardware. The quantization converts:

- Weights: `f32` -> `i8` (256 levels)
- Activations: `f32` -> `i8`
- All arithmetic: integer multiply-accumulate

## Memory Footprint

| Model | Float32 | INT8 | Savings |
|-------|---------|------|---------|
| Dense(784,128) | 392 KB | 98 KB | 4x |
| Dense(128,10) | 5 KB | 1.3 KB | 4x |

## Bare-Metal Deployment

```fajar
#[no_std]
@entry
@kernel fn main() {
    let input = read_sensor_data()
    let result = inference(Tensor::from_slice(input))
    if result > 0.5 {
        activate_output()
    }
}
```

## ONNX Export

Export models for deployment on other platforms:

```fajar
let model = build_onnx_model(layers)
onnx_save("model.onnx", model)
```
