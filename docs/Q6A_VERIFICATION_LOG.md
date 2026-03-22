# Q6A MNIST Inference Verification Log

**Date:** 2026-03-22
**Hardware:** Radxa Dragon Q6A (QCS6490)
**QNN SDK:** v2.40.0.251030
**OS:** Ubuntu 24.04 LTS, Linux 6.18.2-3-qcom aarch64

---

## 1. Test Setup

- **Model:** `mnist_trained_fp32.dlc` (FP32, 421KB) — trained MLP (784→128→10)
- **Test set:** 100 MNIST digit images (28×28 float32, normalized)
- **Ground truth:** Labels verified from `labels.npy`
- **Tool:** `qnn-net-run` from QNN SDK v2.40
- **Input format:** Raw float32 (3136 bytes = 784 × 4 bytes per sample)

## 2. Results

### CPU Backend (libQnnCpu.so — Kryo 670, 8 cores)

| Metric | Value |
|--------|-------|
| **Accuracy** | **99/100 = 99%** |
| **Misclassified** | 1 (sample_062: label=9, predicted=5, conf=3.199) |
| **Total time (100 samples)** | 29-54ms (3 runs) |
| **Per-inference latency** | ~0.33ms |
| **Throughput** | ~3,000 inferences/sec |

### GPU Backend (libQnnGpu.so — Adreno 643 @ 812MHz)

| Metric | Value |
|--------|-------|
| **Accuracy** | **99/100 = 99%** |
| **Total time (100 samples)** | 312-444ms (3 runs) |
| **Per-inference latency** | ~3.6ms |
| **Throughput** | ~278 inferences/sec |

### INT8 Quantized (mnist_mlp_int8.dlc)

| Metric | Value |
|--------|-------|
| **Accuracy** | 1/100 = 1% (FAILED) |
| **Note** | Uniform output ~0.1016 across all classes — quantization loss |
| **Root cause** | DLC quantized without proper calibration data |

### Benchmark Summary (CPU vs GPU, FP32)

```
Backend   | Accuracy | Latency (100 samples) | Per-inference
----------|----------|-----------------------|-------------
CPU       | 99%      | 29-54ms               | ~0.33ms
GPU       | 99%      | 312-444ms             | ~3.6ms
INT8 CPU  | 1%       | 40ms                  | ~0.4ms (broken)
```

**Winner:** CPU backend — 10× faster than GPU for this small model (MLP has no parallelism to benefit from GPU).

## 3. System State During Test

| Metric | Value |
|--------|-------|
| CPU temperature | 59°C |
| GPU temperature | 58°C |
| Memory available | 6.1 GB / 7.4 GB |
| Kernel | 6.18.2-3-qcom (PREEMPT_DYNAMIC) |
| CPU cores | 8 (Kryo 670: 1×A78 + 3×A78 + 4×A55) |

## 4. QNN Backend Availability

| Backend | Library | Status |
|---------|---------|--------|
| CPU | libQnnCpu.so | WORKING |
| GPU | libQnnGpu.so | WORKING (FP32 DLC) |
| HTP | libQnnHtp.so | Installed, needs testsig |
| DSP | libQnnDsp.so | Installed, needs testsig |

## 5. Fajar Lang Integration

- **Fajar Lang binary:** Cross-compiled aarch64 (15.5MB), runs on Q6A
- **Example:** `examples/q6a_mnist_inference.fj` — runs full pipeline on Q6A
- **QNN builtins:** `qnn_quantize()`, `qnn_dequantize()`, `qnn_version()` working
- **Tensor builtins:** `randn()`, `matmul()`, `relu()`, `softmax()`, `argmax()` working on ARM64
- **File I/O:** `file_exists()`, `read_file()` working for sysfs/model paths

## 6. Model Details

```
Graph: graph_gi91vc9e
Input:  tensor "input"    [1,1,28,28] float32 (3136 bytes)
Output: tensor "output_0" [1,10]      float32 (40 bytes)
```

## 7. Files on Q6A

```
/home/radxa/models/
├── mnist_trained_fp32.dlc      (421 KB — FP32, WORKING)
├── mnist_mlp_int8.dlc          (113 KB — INT8, broken quantization)
├── mnist_trained_int8.dlc      (113 KB — INT8, needs recalibration)
├── input_list_10.txt           (10 sample paths)
├── input_list_100.txt          (100 sample paths)
└── mnist_test/
    ├── sample_000.raw ... sample_099.raw  (100 × 3136 bytes)
    └── labels.npy                          (100 int64 labels)

/home/radxa/results/
├── cpu_fp32/Result_0..99/output_0.raw     (CPU FP32 outputs)
├── gpu_100/Result_0..99/output_0.raw      (GPU FP32 outputs)
└── prof_cpu/, prof_gpu/                   (profiling data)
```

## 8. Conclusion

**MNIST inference on Radxa Dragon Q6A: VERIFIED.**

- 99% accuracy on 100 test digits (CPU and GPU backends)
- Sub-millisecond latency on CPU (0.33ms/inference)
- Fajar Lang `q6a_mnist_inference.fj` example runs end-to-end on real hardware
- INT8 quantized model needs re-quantization with proper calibration

**Next steps:**
- Re-quantize INT8 model with proper calibration data
- Test HTP backend (requires Qualcomm testsig)
- Camera→NPU real-time pipeline (requires camera module)
