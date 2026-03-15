# Q6A QNN SDK Setup Guide

> Setting up Qualcomm AI Engine Direct (QNN) SDK on Radxa Dragon Q6A for NPU inference.

---

## Prerequisites

- Radxa Dragon Q6A running Ubuntu 24.04
- SSH access configured
- Network connection (apt packages required)

## Installation

QNN SDK v2.40 is available as apt packages on the Q6A Ubuntu 24.04 image:

```bash
sudo apt update
sudo apt install -y libqnn-dev libqnn1 qnn-tools
```

## Verify Installation

### Check QNN packages
```bash
dpkg -l | grep qnn
# Expected:
# libqnn-dev   2.40.0.251030-0ubuntu1  arm64  QNN SDK - Development files
# libqnn1      2.40.0.251030-0ubuntu1  arm64  QNN SDK - Libraries
# qnn-tools    2.40.0.251030-0ubuntu1  arm64  QNN SDK - Binary tools
```

### Check QNN libraries
```bash
ls /usr/lib/libQnnHtp.so          # HTP (Hexagon Tensor Processor) backend
ls /usr/lib/libQnnCpu.so          # CPU fallback backend
ls /usr/lib/libQnnGpu.so          # GPU (Adreno) backend
ls /usr/lib/libQnnDsp.so          # DSP backend
ls /usr/lib/rfsa/adsp/libQnnHtpV68Skel.so  # Hexagon V68 skeleton
```

### Check NPU/CDSP availability
```bash
ls /dev/fastrpc-cdsp              # FastRPC device for CDSP communication
cat /sys/bus/platform/drivers/fastrpc/*/subsys_state  # Should show "on"
```

### Test qnn-net-run
```bash
qnn-net-run --version             # Should print version info
```

### Test from Fajar Lang
```fajar
fn main() {
    println("NPU available: " + to_string(npu_available()))
    println("NPU info: " + npu_info())
}
```
Expected output:
```
NPU available: true
NPU info: Hexagon 770 V68, 12 TOPS INT8, QNN SDK
```

## Available Backends

| Backend | Library | Hardware | Precision | Use Case |
|---------|---------|----------|-----------|----------|
| HTP | `libQnnHtp.so` | Hexagon 770 NPU | INT8/INT16 | Production inference (fastest) |
| CPU | `libQnnCpu.so` | Kryo 670 CPU | FP32 | Debugging, accuracy verification |
| GPU | `libQnnGpu.so` | Adreno 643 GPU | FP16/FP32 | Medium-performance inference |
| DSP | `libQnnDsp.so` | Hexagon DSP | INT8 | Audio/signal processing |

## Hexagon V68 Details

| Property | Value |
|----------|-------|
| Architecture | Hexagon 770 (V68) |
| Compute | 12 TOPS INT8 |
| Skeleton | `libQnnHtpV68Skel.so` |
| FastRPC | `/dev/fastrpc-cdsp` |
| Supported types | INT8, INT16, FP16 (via HTP) |

## Fajar Lang NPU Builtins

| Builtin | Description |
|---------|-------------|
| `npu_available()` | Returns `true` if NPU/HTP is present |
| `npu_info()` | Returns NPU hardware info string |
| `npu_load(path)` | Load QNN context binary for inference |
| `npu_infer(model, input)` | Run inference on NPU |
| `qnn_quantize(tensor, dtype)` | Quantize tensor for QNN backend |
| `qnn_dequantize(handle)` | Dequantize NPU output |

## Model Deployment Pipeline

```
Host (x86_64)                          Dragon Q6A (aarch64)
┌──────────────┐                       ┌──────────────────────┐
│ Train model  │                       │ NPU Inference        │
│ in Fajar Lang│──FJML/FJMQ──►ONNX──►│ qnn-net-run or       │
│ (SGD/Adam)   │                       │ Fajar Lang builtins  │
└──────────────┘                       └──────────────────────┘
```

1. Train on host: `fj run train.fj` → `model.fjml` / `model.fjmq`
2. Convert: `fjml_to_onnx.py` → ONNX → `qnn-onnx-converter` → QNN model
3. Deploy: `scp model.bin radxa@q6a:/opt/fj/models/`
4. Infer: `fj run infer.fj` (uses `npu_load` + `npu_infer`)

See `docs/Q6A_ML_PIPELINE.md` for the complete pipeline documentation.

---

*v2.0 "Dawn" — QNN SDK v2.40 on Radxa Dragon Q6A*
