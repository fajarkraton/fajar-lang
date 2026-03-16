# Dragon Q6A (QCS6490) — Edge AI Board

The Radxa Dragon Q6A is Fajar Lang's primary edge AI deployment target, featuring Qualcomm's QCS6490 SoC with a dedicated NPU, GPU, and high-performance CPU cluster.

## Hardware Overview

| Component | Specification |
|-----------|--------------|
| **SoC** | Qualcomm QCS6490 (TSMC 6nm) |
| **CPU** | Kryo 670: 1x A78@2.7GHz + 3x A78@2.4GHz + 4x A55@1.9GHz |
| **GPU** | Adreno 643 @ 812MHz (OpenCL 3.0, Vulkan 1.1) |
| **NPU** | Hexagon 770 V68 — 12 TOPS INT8 |
| **RAM** | LPDDR5 up to 16GB |
| **GPIO** | 40-pin header: 7 UART, 6 I2C, 7 SPI, I2S, I3C |

## Quick Start

```bash
# Cross-compile
cargo build --release --target aarch64-unknown-linux-gnu

# Deploy
scp target/aarch64-unknown-linux-gnu/release/fj radxa@192.168.100.2:/usr/local/bin/fj

# Run
ssh radxa@192.168.100.2 'fj run /tmp/hello.fj'
```

## System Monitoring Builtins

```fajar
let temp = cpu_temp()       // CPU temp in millidegrees
let freq = cpu_freq()       // CPU frequency in kHz
let mem = mem_usage()        // Memory usage percentage
let up = sys_uptime()        // Uptime in seconds
let gpu = gpu_available()    // GPU detected?
let npu = npu_info()         // NPU capabilities
```

## GPU Compute (Adreno 635)

```fajar
// GPU-accelerated operations (auto-fallback to CPU)
let c = gpu_matmul(a, b)
let d = gpu_relu(c)
let e = gpu_sigmoid(d)
let f = gpu_add(x, y)
```

## NPU Inference (Hexagon 770)

```fajar
let handle = qnn_quantize(tensor, 0)  // INT8 quantization
let result = qnn_dequantize(handle)    // Back to f64 tensor
```

## Production Deployment

```fajar
// Watchdog + logging pattern
let wd = watchdog_start(5000)
while true {
    watchdog_kick(wd)
    let result = run_inference()
    log_to_file("/var/log/fj/app.log", result)
    sleep_ms(100)
}
```

## Performance

| Operation | Time |
|-----------|------|
| Cold start → first inference | 4 ms |
| JIT vs interpreted | 128x faster |
| Tensor matmul (NEON) | < 1 ms |

## References

- [Quick Start Guide](../../../docs/Q6A_QUICKSTART.md)
- [Production Deployment](../../../docs/Q6A_PRODUCTION.md)
- [GPIO Pinout](../../../docs/Q6A_PINOUT.md)
- [ML Pipeline](../../../docs/Q6A_ML_PIPELINE.md)
