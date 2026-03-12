# Radxa Dragon Q6A — Hardware Reference for Fajar Lang

> Target hardware untuk implementasi Fajar Lang pada edge AI SBC berbasis Qualcomm QCS6490.
> Board dibeli oleh Fajar (PrimeCore.id) — Maret 2026.

---

## 1. Overview

| Property | Value |
|----------|-------|
| **Board** | Radxa Dragon Q6A |
| **SoC** | Qualcomm QCS6490 (Dragonwing) |
| **Process** | TSMC 6nm (N6) |
| **Form Factor** | 85mm x 56mm (credit-card SBC) |
| **Price** | $59.50 - $124.29 (tergantung RAM) |
| **Longevity** | 15-year Qualcomm IoT commitment |
| **Power** | USB Type-C 12V (PD), 18-24W PSU recommended |
| **TDP** | ~5W SoC, ~7W module typical |

---

## 2. CPU — Kryo 670 (ARMv8.2-A)

Tri-cluster DynamIQ design, 8 cores total:

| Cluster | Cores | ARM Core | Clock | Role |
|---------|-------|----------|-------|------|
| **Prime** | 1x | Cortex-A78 | 2.71 GHz | High-performance single-thread |
| **Gold** | 3x | Cortex-A78 | 2.40 GHz | Performance multi-thread |
| **Silver** | 4x | Cortex-A55 | 1.96 GHz | Efficiency / background |

### Cache Hierarchy

| Level | A78 (Prime/Gold) | A55 (Silver) |
|-------|-------------------|--------------|
| L1 I-Cache | 64 KB/core (4-way) | 32 KB/core (4-way) |
| L1 D-Cache | 64 KB/core (4-way) | 32 KB/core (4-way) |
| L2 Cache | 512 KB/core (private) | 128 KB/core (private) |
| L3 / SLC | 3 MB shared | 3 MB shared |

### Key ISA Features (ARMv8.2-A)
- **NEON/ASIMD**: 128-bit SIMD, fp16 arithmetic
- **DotProd**: INT8 dot product instructions (SDOT/UDOT) — kunci untuk ML inference
- **FP16**: Native half-precision floating point
- **LSE**: Large System Extensions (atomic operations)
- **CRC32**: Hardware CRC instructions
- **AES/SHA**: Crypto acceleration

### Rust/Fajar Lang Target
```
Target triple: aarch64-unknown-linux-gnu
LLVM target:   aarch64-linux-gnu
Kernel ARCH:   arm64
```

---

## 3. GPU — Adreno 643

| Property | Value |
|----------|-------|
| Clock | 812 MHz |
| OpenGL ES | 3.2 |
| Vulkan | 1.1 |
| OpenCL | 2.0 |
| DirectX | Feature Level 12 |
| Est. FP32 | ~700+ GFLOPS (812 MHz, higher than Adreno 642L @ 550 MHz) |

### Compute Capabilities
- **OpenCL 2.0**: Shared virtual memory, generic address space, pipes
- **Vulkan Compute**: Compute shaders for GPU workloads
- **Adreno GPU SDK**: Available from Qualcomm for optimized GPU compute

---

## 4. NPU — Hexagon 770 (AI Engine 6th Gen)

| Property | Value |
|----------|-------|
| DSP Version | Hexagon 770 |
| ISA Version | V68 |
| AI Performance | **12 TOPS** (INT8) |
| Architecture | VLIW + HVX (SIMD) + HTA/HMX (Tensor) |

### Hexagon 770 Components
1. **Hexagon Scalar**: General-purpose DSP operations (VLIW)
2. **HVX (Hexagon Vector eXtensions)**: 1024-bit SIMD vectors, dual units
3. **HTA/HMX (Hexagon Matrix eXtension)**: Dedicated tensor/matrix accelerator

### AI SDK Stack

```
Application Layer
    |
    v
+-- QNN (Qualcomm AI Engine Direct) SDK --+
|   - qnn-onnx-converter                  |
|   - qnn-model-lib-generator             |
|   - qnn-net-run                         |
+------------------------------------------+
    |
    v
Backend Libraries:
    libQnnCpu.so    (CPU fallback, any model)
    libQnnGpu.so    (Adreno GPU, FP16/FP32)
    libQnnHtp.so    (Hexagon NPU, INT8/INT16 quantized)
        |
        v
    libQnnHtpV68Skel.so  (V68-specific DSP firmware)
    libqnnhtpv68.cat      (V68 context binary)
```

### QNN Deployment Pipeline
```
1. Train model (PyTorch/TF/ONNX) on host
2. Convert: qnn-onnx-converter --input_network model.onnx
3. Quantize: --input_list calibration.txt (INT8 default)
4. Compile: qnn-model-lib-generator --> model.so
5. Deploy: qnn-net-run --model model.so --backend libQnnHtp.so
```

### Alternative AI Paths
- **ONNX Runtime + QNN EP**: Run ONNX models with QNN execution provider
- **PyTorch ExecuTorch**: Qualcomm backend for PyTorch Mobile
- **TensorFlow Lite + Hexagon Delegate**: TFLite with NPU acceleration

---

## 5. Memory & Storage

### RAM
| Option | Type | Bandwidth |
|--------|------|-----------|
| 4 GB | LPDDR5 @ 3200 MHz | ~25.6 GB/s |
| 6 GB | LPDDR5 @ 3200 MHz | ~25.6 GB/s |
| 8 GB | LPDDR5 @ 3200 MHz | ~25.6 GB/s |
| 12 GB | LPDDR5 @ 3200 MHz | ~25.6 GB/s |
| **16 GB** | LPDDR5 @ 3200 MHz | ~25.6 GB/s |

Dual-channel, 32-bit total bus width (2x 16-bit channels).

### Storage Options
| Type | Interface | Notes |
|------|-----------|-------|
| QSPI NOR Flash | 32 MB | Boot firmware (SPI) |
| MicroSD | SD 3.0 | Bootable, hot-swappable |
| eMMC Module | eMMC 5.1 | Onboard module slot |
| UFS Module | UFS 2.x | High-speed onboard |
| **M.2 NVMe SSD** | M.2 M-Key 2230 | PCIe Gen3, fastest option |

### Boot Priority
```
USB > MicroSD > NVMe SSD > eMMC Module > UFS Module
```

---

## 6. Connectivity

| Interface | Details |
|-----------|---------|
| **Ethernet** | 1x Gigabit (10/100/1000), PoE support via HAT |
| **WiFi** | WiFi 6 (802.11ax), 2.4/5 GHz, external antenna |
| **Bluetooth** | 5.4, external antenna |
| **USB 3.1** | 1x Type-A (HOST/OTG) |
| **USB 2.0** | 3x Type-A (HOST) |

---

## 7. Display & Camera

### Display
| Port | Resolution | Notes |
|------|-----------|-------|
| HDMI | 4K @ 30Hz | Standard HDMI connector |
| MIPI DSI | 4-lane | Flat cable, for embedded displays |

### Camera
| Port | Lanes | Notes |
|------|-------|-------|
| MIPI CSI #1 | 4-lane | Primary camera |
| MIPI CSI #2 | 2-lane | Secondary camera |
| MIPI CSI #3 | 2-lane | Tertiary camera |

### Video Processing
- **Decode**: 4K @ 60fps (H.264, H.265, VP9)
- **Encode**: 4K @ 30fps (H.264, H.265)
- **HDR**: HDR10, HDR10+
- **ISP**: Qualcomm Spectra triple ISP (up to 3 cameras simultaneous)

---

## 8. 40-Pin GPIO Header

**Logic Level: 3.3V** | **GPIO Device: /dev/gpiochip4**

```
                    +-----+
          3V3  [1]  |     |  [2]  5V
  GPIO24/I2C6_SDA  [3]  |     |  [4]  5V
  GPIO25/I2C6_SCL  [5]  |     |  [6]  GND
     GPIO96/MCLK  [7]  |     |  [8]  GPIO22/UART5_TX
              GND  [9]  |     | [10]  GPIO23/UART5_RX
  GPIO29/I2C7_SCL [11]  |     | [12]  GPIO97/I2C0_SDA
           GPIO0 [13]  |     | [14]  GND
   GPIO1/I2C0_SCL [15]  |     | [16]  GPIO26/UART6_TX
          3V3 [17]  |     | [18]  GPIO27/UART6_RX
 GPIO49/SPI12_MOSI [19]  |     | [20]  GND
 GPIO48/SPI12_MISO [21]  |     | [22]  GPIO50/UART12_TX
 GPIO57/I2C14_SCL [23]  |     | [24]  GPIO51/SPI12_CS
              GND [25]  |     | [26]  GPIO8/I2C2_SDA
   GPIO9/I2C2_SCL [27]  |     | [28]  GPIO31/UART7_RX
              GND [29]  |     | [30]  GPIO28/I2C7_SDA
  GPIO30/UART7_TX [31]  |     | [32]  GPIO56/SPI14_MISO
  GPIO59/SPI14_CS [33]  |     | [34]  GND
    GPIO100/I2S_WS [35]  |     | [36]  --
          GPIO58 [37]  |     | [38]  GPIO98/I2S_DATA0
              GND [39]  |     | [40]  GPIO99/I2S_DATA1
                    +-----+
```

### Available Peripheral Buses (via Device Tree overlays)

| Bus | Instances | Pins |
|-----|-----------|------|
| **UART** | UART0, 2, 5, 6, 7, 12, 14 | 7 ports |
| **I2C** | I2C0, 2, 6, 7, 12, 14 | 6 buses |
| **SPI** | SPI0, 2, 5, 6, 7, 12, 14 | 7 buses |
| **I2S** | MI2S0 (MCLK, SCK, WS, DATA0, DATA1) | 5 pins |
| **I3C** | I3C0 (next-gen I2C) | 2 pins |
| **GPIO** | All pins configurable as GPIO | 26 pins |

Pin functions are **mutually exclusive** — selected via Device Tree overlays.

---

## 9. Operating System Support

| OS | Kernel | Status |
|----|--------|--------|
| **Ubuntu 24.04** (Noble) | Linux 6.16.x | Primary, near-mainline |
| **Debian** | Linux 6.16.x | Supported |
| **Armbian** | Linux 6.18 | Community |
| **Android 15/16** | Android kernel | CS/ES |
| **Yocto (Qualcomm Linux)** | BSP kernel | Enterprise |
| **Windows 11 IoT** | Windows | Enterprise |
| **Fedora / Arch / Deepin** | Mainline | Community |

### Kernel Source
```
https://github.com/Deka-Embedded-Linux/linux-dragon-q6a
```

### Default Credentials
- Username: `radxa`
- Password: `radxa`

---

## 10. Cross-Compilation for Fajar Lang

### Toolchain Setup
```bash
# Install aarch64 cross-compiler
sudo apt install gcc-aarch64-linux-gnu g++-aarch64-linux-gnu

# Rust target
rustup target add aarch64-unknown-linux-gnu

# Fajar Lang cross-compile
cargo build --release --target aarch64-unknown-linux-gnu

# Deploy to Q6A
scp target/aarch64-unknown-linux-gnu/release/fj radxa@<ip>:~/
```

### Cranelift Target
Cranelift supports `aarch64` natively. Fajar Lang's Cranelift backend can generate ARM64 code directly.

### LLVM Target
LLVM target triple: `aarch64-linux-gnu`. Fajar Lang's LLVM backend (inkwell) can target this architecture.

---

## 11. Fajar Lang Implementation Strategy

### Phase 1: Cross-Compile & Run
- Cross-compile `fj` binary for aarch64-unknown-linux-gnu
- Deploy and run all 50 examples on Q6A
- Benchmark interpreter and native codegen performance

### Phase 2: GPIO/HAL Integration
- Implement Q6A BSP (Board Support Package)
- Map 40-pin GPIO to Fajar Lang HAL traits
- Support I2C, SPI, UART, GPIO from .fj programs
- Use `/dev/gpiochip4` for GPIO access

### Phase 3: AI/ML on Hexagon NPU
- Integrate QNN SDK for NPU inference
- Export Fajar Lang trained models to ONNX
- Deploy quantized models via `libQnnHtp.so`
- Target: 12 TOPS INT8 inference from .fj programs

### Phase 4: GPU Compute (Adreno 643)
- OpenCL 2.0 compute kernels from Fajar Lang
- Vulkan compute shaders for tensor operations
- GPU-accelerated training on-device

### Phase 5: Real-Time / Bare Metal
- RTOS integration for real-time tasks
- Camera pipeline with Spectra ISP
- Edge AI inference pipeline: Camera -> NPU -> GPIO actuator

---

## 12. Comparison with Previous Targets

| Feature | VENTUNO Q (STM32H5) | Radxa Dragon Q6A |
|---------|---------------------|-------------------|
| CPU | Cortex-M33 @ 250 MHz | 4x A78 + 4x A55 @ 2.7 GHz |
| RAM | 640 KB SRAM | Up to 16 GB LPDDR5 |
| Storage | 2 MB Flash | NVMe SSD + eMMC + SD |
| AI | None | 12 TOPS Hexagon NPU |
| GPU | None | Adreno 643 (OpenCL 2.0) |
| OS | Bare metal / Zephyr | Full Linux (Ubuntu 24.04) |
| Context | @kernel only | @kernel + @device + @safe |
| Use Case | Microcontroller | Edge AI SBC |

Dragon Q6A adalah **leap besar** — dari MCU ke full Linux SBC dengan NPU. Fajar Lang bisa menjalankan **seluruh fitur** (interpreter, native codegen, ML training, GPU compute) di hardware ini.

---

*Document Version: 1.0 | Created: 2026-03-12 | Hardware: Radxa Dragon Q6A (QCS6490)*
