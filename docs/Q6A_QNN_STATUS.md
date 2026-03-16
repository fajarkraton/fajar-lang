# Dragon Q6A — QNN Backend Status

> QNN SDK v2.40.0 on Radxa Dragon Q6A (QCS6490)

---

## Backend Validation Results

| Backend | Hardware | Libraries | Unit Test | Status |
|---------|----------|-----------|-----------|--------|
| **GPU** | Supported | Found | **PASSED** | **WORKING** |
| **DSP/HTP** | Supported | Found | **FAILED** | BLOCKED (testsig) |
| **CPU** | Supported | Found | N/A | WORKING |

### GPU Backend (WORKING)
- Library: `/usr/lib/libQnnGpu.so`
- Core: Adreno(TM) 642 (OpenCL 3.0, QUALCOMM build 0838.2)
- Status: Fully operational, no signing required
- Use case: FP32 inference, tensor operations

### HTP/DSP Backend (BLOCKED)
- Library: `/usr/lib/libQnnHtp.so`
- Skeleton: `/usr/lib/rfsa/adsp/libQnnHtpV68Skel.so`
- Core: Hexagon Architecture V68
- Error: `Error while executing the sum function. Please use testsig if using unsigned images.`
- Root cause: Device requires Qualcomm test signature for unsigned DSP code
- Fix: Generate testsig via Qualcomm developer portal, or use signed system image

### CPU Backend (WORKING)
- Library: `/usr/lib/libQnnCpu.so`
- Status: Operational (fallback path)

---

## Available QNN Tools

| Tool | Path | Purpose |
|------|------|---------|
| `qnn-net-run` | `/usr/bin/qnn-net-run` | Run inference with QNN model |
| `qnn-context-binary-generator` | `/usr/bin/qnn-context-binary-generator` | Generate HTP context binary |
| `qnn-platform-validator` | `/usr/bin/qnn-platform-validator` | Validate backend capabilities |
| `qnn-throughput-net-run` | `/usr/bin/qnn-throughput-net-run` | Throughput benchmarking |
| `qnn-profile-viewer` | `/usr/bin/qnn-profile-viewer` | Profile analysis |
| `qnn-context-binary-utility` | `/usr/bin/qnn-context-binary-utility` | Context binary inspection |

**NOT available on Q6A (x86 host only):**
- `qnn-onnx-converter` — ONNX → QNN model conversion
- `qnn-model-lib-generator` — Generate model .so library

---

## QNN Libraries on Q6A

```
/usr/lib/libQnnCpu.so
/usr/lib/libQnnDsp.so
/usr/lib/libQnnGpu.so
/usr/lib/libQnnHtp.so
/usr/lib/libQnnHta.so
/usr/lib/libQnnLpai.so
/usr/lib/libQnnHtpPrepare.so
/usr/lib/libQnnHtpV68Stub.so
/usr/lib/libQnnHtpV68CalculatorStub.so
/usr/lib/libQnnIr.so
/usr/lib/libQnnModelDlc.so
/usr/lib/libQnnSaver.so
/usr/lib/libQnnSystem.so
/usr/lib/libQnnTFLiteDelegate.so
/usr/lib/libQnnGenAiTransformer.so
/usr/lib/libQnnGenAiTransformerModel.so
/usr/lib/libQnnGenAiTransformerCpuOpPkg.so
```

### ADSP/DSP Skeleton Libraries
```
/usr/lib/rfsa/adsp/libQnnHtpV68Skel.so
/usr/lib/rfsa/adsp/libQnnHtpV68.so
/usr/lib/rfsa/adsp/libQnnHtpV73Skel.so
/usr/lib/rfsa/adsp/libQnnHtpV73.so
/usr/lib/rfsa/adsp/libQnnHtpV75Skel.so
/usr/lib/rfsa/adsp/libQnnHtpV75.so
/usr/lib/rfsa/adsp/libCalculator_skel.so
/usr/lib/rfsa/adsp/libQnnSaver.so
/usr/lib/rfsa/adsp/libQnnSystem.so
```

---

## GPU Name Discrepancy

The Adreno GPU is reported differently by different tools:
- **QNN platform-validator**: "Adreno(TM) 642" (device tier)
- **OpenCL clinfo**: "Adreno(TM) 635"
- **Qualcomm spec sheet**: "Adreno 643"

All three refer to the same physical GPU. The variation comes from:
- OpenCL driver version reporting an older marketing name
- QNN using internal device tier numbers
- Spec sheet using the official product name

---

## Fajar Lang QNN Integration

### Working Builtins (CPU + QNN GPU fallback)
```fajar
gpu_available()    // → true on Q6A
gpu_info()         // → "Adreno(TM) 635 | OpenCL 3.0 | 3793 MB"
gpu_matmul(a, b)   // GPU-accelerated matrix multiply
gpu_relu(t)        // GPU-accelerated ReLU
gpu_sigmoid(t)     // GPU-accelerated sigmoid
gpu_mul(a, b)      // GPU element-wise multiply
gpu_transpose(t)   // GPU transpose
gpu_sum(t)         // GPU reduction sum
qnn_quantize(t, dtype)   // Quantize tensor (f32/f16/int8/uint8/bf16)
qnn_dequantize(handle)   // Dequantize back to tensor
```

### Example: QNN GPU Inference
```fajar
// See examples/q6a_qnn_gpu_infer.fj
let input = tensor_randn(224, 64)
let weights = tensor_xavier(64, 4)
let z = gpu_matmul(input, weights)
let activated = gpu_relu(z)
let output = tensor_softmax(activated)
let predicted = tensor_argmax(output)
```

---

*Document: Q6A_QNN_STATUS.md | v2.0 "Dawn" | 2026-03-16*
