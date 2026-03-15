# Q6A ML Pipeline: Train → Export → Deploy → Infer

> End-to-end machine learning pipeline from host training to NPU inference on Dragon Q6A.

---

## Overview

```
Host (x86_64)                          Dragon Q6A (aarch64)
┌──────────────┐                       ┌──────────────────────┐
│ Train model  │                       │ NPU Inference        │
│ in Fajar Lang│──export──►FJML/FJMQ──►│ (Hexagon 770, 12T)  │
│ (SGD/Adam)   │                       │ QNN HTP backend      │
└──────────────┘                       └──────────────────────┘
```

## Step 1: Train on Host

Create a training script in Fajar Lang:

```fajar
// train.fj
fn main() {
    // Initialize weights (Xavier)
    let mut w1 = tensor_xavier(4, 16)
    tensor_set_requires_grad(w1, true)
    let mut w2 = tensor_xavier(16, 3)
    tensor_set_requires_grad(w2, true)

    // Create optimizer
    let opt = optimizer_sgd(0.1, 0.9)

    // Training loop
    let mut epoch = 0
    while epoch < 10 {
        // ... forward pass, loss, backward, optimizer step ...
        epoch = epoch + 1
    }

    // Save weights
    model_save("model.fjml", "w1", w1, "w2", w2)
    model_save_quantized("model.fjmq", "w1", w1, "w2", w2)
}
```

Run:
```bash
fj run train.fj
```

Output files:
- `model.fjml` — Full precision weights (f64, FJML format)
- `model.fjmq` — INT8 quantized weights (FJMQ format)

## Step 2: Export Formats

### FJML (Fajar ML)
- Full precision f64 per element
- Used for fine-tuning, accuracy verification
- Format: `FJML` magic + version + named tensor array
- API: `model_save(path, name1, tensor1, name2, tensor2, ...)`

### FJMQ (Fajar ML Quantized)
- INT8 per element with per-tensor scale factor
- ~8x smaller than FJML
- API: `model_save_quantized(path, name1, tensor1, name2, tensor2, ...)`

### Size comparison (131-param model)

| Format | Size | Precision |
|--------|------|-----------|
| FJML   | 1,132 bytes | f64 |
| FJMQ   |   247 bytes | INT8 |

## Step 3: Convert to QNN (on host or Q6A)

```bash
# Convert FJML → ONNX (external Python tool)
python3 fjml_to_onnx.py model.fjml --output model.onnx

# Or use QNN tools directly with FJMQ for INT8 inference
# QNN SDK required: https://developer.qualcomm.com/software/qualcomm-ai-engine-direct

# ONNX → QNN model library
qnn-onnx-converter --input_network model.onnx --output_path model_qnn

# Quantize to INT8 for HTP backend
qnn-model-lib-generator \
    --model model_qnn.cpp \
    --backend libQnnHtp.so \
    --output_path model.so

# Generate context binary (optimized for Hexagon 770)
qnn-context-binary-generator \
    --model model.so \
    --backend libQnnHtp.so \
    --output_dir .
```

## Step 4: Deploy to Q6A

```bash
# Copy model to Q6A
scp model.bin radxa@q6a:/opt/fj/models/

# Or use the fj deploy command (planned)
fj deploy --board dragon-q6a model.bin
```

## Step 5: Run NPU Inference

```fajar
// infer.fj — runs on Dragon Q6A
fn main() {
    let model = npu_load("/opt/fj/models/model.bin")
    let input = tensor_zeros(1, 4)  // prepare input
    let qbuf = qnn_quantize(input, "uint8")
    let result = npu_infer(model, qbuf)
    let output = qnn_dequantize(result)
    let predicted = tensor_argmax(output)
    println("Prediction: " + to_string(predicted))
}
```

Run on Q6A:
```bash
fj run --board dragon-q6a infer.fj
```

## Builtins Reference

### Training
| Builtin | Description |
|---------|-------------|
| `tensor_xavier(rows, cols)` | Xavier-initialized weight matrix |
| `tensor_set_requires_grad(t, bool)` | Enable gradient tracking |
| `tensor_backward(loss)` | Reverse-mode autodiff |
| `optimizer_sgd(lr, momentum)` | Create SGD optimizer |
| `optimizer_adam(lr)` | Create Adam optimizer |
| `optimizer_step(opt, tensor)` | Apply gradient update, returns updated tensor |
| `optimizer_zero_grad(tensor)` | Reset gradients, returns tensor |

### Export
| Builtin | Description |
|---------|-------------|
| `model_save(path, n1, t1, n2, t2, ...)` | Save as FJML (f64) |
| `model_save_quantized(path, n1, t1, ...)` | Save as FJMQ (INT8) |

### NPU Inference
| Builtin | Description |
|---------|-------------|
| `npu_available()` | Check if NPU is present |
| `npu_info()` | Get NPU hardware info |
| `npu_load(path)` | Load QNN context binary |
| `npu_infer(model, input)` | Run inference on NPU |
| `qnn_quantize(tensor, dtype)` | Quantize for QNN backend |
| `qnn_dequantize(handle)` | Dequantize NPU output |

## Complete Example

See `examples/mnist_train_full.fj` for a complete training pipeline:
- Architecture: 4 → 16 (ReLU) → 3 (Softmax)
- 131 parameters, SGD optimizer
- Trains 10 epochs, saves FJML + FJMQ

## Performance Targets (on Dragon Q6A)

| Model | Format | Latency | FPS |
|-------|--------|---------|-----|
| MobileNetV2 | INT8 | ~3 ms | ~333 |
| ResNet-50 | INT8 | ~5 ms | ~200 |
| YOLOv8n | INT8 | ~33 ms | ~30 |
| MNIST (custom) | INT8 | < 1 ms | > 1000 |

---

*v2.0 "Dawn" — Fajar Lang on Radxa Dragon Q6A*
