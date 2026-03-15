# V2.0 "Dawn" — Radxa Dragon Q6A Hardware Deployment

> **Target:** Deploy Fajar Lang on Radxa Dragon Q6A (QCS6490) edge AI SBC.
> **Hardware:** Purchased by Fajar (PrimeCore.id) — Maret 2026.
> **Reference:** `docs/RADXA_Q6A_HARDWARE.md` — Full hardware specification.
> **App Dev Reference:** `docs/Q6A_APP_DEV.md` — Official Radxa app-dev documentation digest.
> **Low-Level Dev Reference:** `docs/Q6A_LOW_LEVEL_DEV.md` — Boot, EDL, SPI firmware, kernel/OS build.
> **Hardware Usage Reference:** `docs/Q6A_HARDWARE_USE.md` — Power, storage, GPIO pinout, display, camera, audio, RTC.
> **Accessories Reference:** `docs/Q6A_ACCESSORIES.md` — Cameras, displays, storage modules, PoE HAT.
> **Official Docs:** https://docs.radxa.com/en/dragon/q6a

---

## Overview

| Property | Value |
|----------|-------|
| **Codename** | "Dawn" — Fajar Lang's first physical hardware deployment |
| **Board** | Radxa Dragon Q6A |
| **SoC** | Qualcomm QCS6490 (Dragonwing), TSMC 6nm |
| **CPU** | Kryo 670 ARMv8.2-A — 1x A78@2.7GHz + 3x A78@2.4GHz + 4x A55@1.9GHz |
| **GPU** | Adreno 643 @ 812MHz — OpenCL 2.0, Vulkan 1.1 |
| **NPU** | Hexagon 770 (V68) — 12 TOPS INT8 |
| **RAM** | LPDDR5 up to 16GB |
| **GPIO** | 40-pin (7 UART, 6 I2C, 7 SPI, I2S, I3C), 3.3V, /dev/gpiochip4 |
| **Target** | `aarch64-unknown-linux-gnu` |
| **OS** | Ubuntu 24.04, kernel 6.16.x |
| **Phases** | 6 |
| **Sprints** | 24 |
| **Tasks** | 240 |

---

## Progress Summary

> **Last updated:** 2026-03-15 | **Tests:** 5,344+ (0 failures) | **Examples:** 60/60 pass on Q6A

| Phase | Sprints | Tasks Done | Tasks Total | Status |
|-------|---------|------------|-------------|--------|
| **1 — Foundation** | S1-S4 | 40 | 40 | **COMPLETE** |
| **2 — On-Device** | S5-S8 | 28 | 40 | S5 9/10, S6 5/10, S7 7/10, S8 6/10 |
| **3 — AI/ML NPU** | S9-S14 | 32 | 60 | S9 9/10, S11 **COMPLETE**, S12 **COMPLETE**, S13 3/10 |
| **4 — GPU Compute** | S15-S18 | 0 | 40 | Not started |
| **5 — Edge AI Apps** | S19-S22 | 0 | 40 | Not started |
| **6 — Production** | S23-S24 | 0 | 20 | Not started |
| **TOTAL** | **24** | **100** | **240** | **42% complete** |

### Sprint Completion Detail

| Sprint | Name | Done/Total | Notes |
|--------|------|------------|-------|
| S1 | Cross-Compilation Toolchain | 10/10 | COMPLETE |
| S2 | Dragon Q6A BSP Module | 10/10 | COMPLETE |
| S3 | 40-Pin GPIO HAL | 10/10 | COMPLETE |
| S4 | UART/I2C/SPI HAL | 10/10 | COMPLETE |
| S5 | Deploy & Run on Q6A | **9/10** | 60/60 examples pass, benchmarks done, REPL+NEON verified |
| S6 | Native Codegen on ARM64 | **5/10** | JIT works (128x speedup), AOT blocked (Cranelift reloc), 5863/5864 tests |
| S7 | GPIO Blinky on Q6A | **7/10** | GPIO verified on real HW (gpioset/gpioget gpiochip4) |
| S8 | Serial Communication | 6/10 | Software done, HW tests pending |
| S9 | QNN SDK Setup | **9/10** | All backends verified, docs + qnn_version() builtin done |
| S10 | ONNX → QNN Pipeline | 0/10 | Needs QNN tools on host |
| S11 | QNN FFI Integration | **10/10** | **COMPLETE** — all builtins verified on real Q6A NPU |
| S12 | Fajar Lang NPU Builtins | **10/10** | **COMPLETE** — 1000 inferences in 4ms, q/dq roundtrip ok |
| S13 | NPU Training Pipeline | **3/10** | 13.1 train + 13.2 export + 13.10 docs done |
| S14 | Camera → NPU Pipeline | 0/10 | Needs camera module |
| S15-S24 | GPU/Edge/Production | 0/100 | Not started |

### What's Implemented (Software-Side, No Board Required)

- **QNN FFI bindings** (`src/runtime/ml/npu/qnn.rs`): Full `dlopen` → `QnnInterface_getProviders` → function table, cfg-gated for aarch64
- **QNN error codes**: 24 QNN error codes mapped to `NpuRuntimeError`
- **QNN data types**: 13 types (INT8/UINT8/F16/F32/BF16/etc.) with `NpuDtype` conversion
- **QNN tensor descriptors**: `QnnTensorDescriptor` (input/output), `QnnClientBuffer`, `QnnScaleOffset`
- **QNN backend**: `QnnBackend` with `load_model()`, `execute()`, `unload_model()` — real + simulation paths
- **QNN buffer conversion**: `QnnBuffer::from_tensor()` / `to_tensor()` with 5 quantization formats
- **Interpreter builtins**: `qnn_quantize(tensor, dtype) → handle`, `qnn_dequantize(handle) → tensor`
- **Type checker**: QNN builtins registered with proper Tensor/I64/Str types
- **Examples**: `q6a_npu_classify.fj` (MobileNetV2), `q6a_npu_detect.fj` (YOLOv8n)
- **Model export**: `model_save(path, name, tensor, ...)` → FJML (f64), `model_save_quantized(...)` → FJMQ (INT8)
- **Training example**: `mnist_train_full.fj` — full pipeline: Xavier init → forward → cross-entropy → backward → SGD → save
- **Tests**: 46 QNN unit tests + 7 integration tests

### Blocking Dependencies

```
Board setup (S5.1: flash Ubuntu 24.04) blocks:
  └── All Phase 2 hardware tests (S5-S8)
  └── QNN SDK install (S9)
  └── On-device NPU testing (S11.10, S12.10)
  └── All Phase 4-6 (GPU, Edge AI, Production)
```

---

## Phase 1: Foundation — Cross-Compile & BSP (Sprints 1-4)

### Sprint 1: Cross-Compilation Toolchain

| # | Task | Status |
|---|------|--------|
| 1.1 | Verify `rustup target add aarch64-unknown-linux-gnu` installs cleanly | [x] |
| 1.2 | Install `gcc-aarch64-linux-gnu` and `g++-aarch64-linux-gnu` cross-compiler | [x] |
| 1.3 | Configure `.cargo/config.toml` with `[target.aarch64-unknown-linux-gnu]` linker | [x] |
| 1.4 | Cross-compile `fj` binary: `cargo build --release --target aarch64-unknown-linux-gnu` | [x] |
| 1.5 | Resolve any cross-compilation errors (ndarray, tokio, cranelift, etc.) | [x] |
| 1.6 | Verify binary type: `file target/aarch64.../release/fj` → ELF 64-bit ARM aarch64 | [x] |
| 1.7 | Create `scripts/cross-build-q6a.sh` helper script | [x] |
| 1.8 | Document cross-compilation setup in `docs/CROSS_COMPILE.md` | [x] |
| 1.9 | Test binary size: target < 20MB stripped | [x] |
| 1.10 | Add `--board dragon-q6a` CLI flag to `fj build` command | [x] |

### Sprint 2: Dragon Q6A BSP Module

| # | Task | Status |
|---|------|--------|
| 2.1 | Create `src/bsp/dragon_q6a.rs` module with `DragonQ6A` struct | [x] |
| 2.2 | Implement `Board` trait: name, arch (Aarch64Linux), cpu_frequency (2_710_000_000) | [x] |
| 2.3 | Define memory regions: RAM (up to 16GB), NVMe, eMMC, microSD | [x] |
| 2.4 | Define peripherals: 40-pin GPIO, 3x MIPI-CSI, HDMI, MIPI-DSI, USB, ETH, WiFi, BT | [x] |
| 2.5 | Implement GPU capabilities: Adreno 643 @ 812MHz, Vulkan 1.1, OpenCL 2.0 | [x] |
| 2.6 | Implement NPU capabilities: Hexagon 770, 12 TOPS, V68 ISA, QNN SDK | [x] |
| 2.7 | Implement `generate_linker_script()` for Linux userspace ELF | [x] |
| 2.8 | Implement `generate_startup_code()` for Linux userspace entry | [x] |
| 2.9 | Register in `src/bsp/mod.rs`: add `pub mod dragon_q6a;` and `board_by_name("dragon-q6a")` | [x] |
| 2.10 | Write 20+ unit tests for DragonQ6A BSP | [x] |

### Sprint 3: 40-Pin GPIO HAL

| # | Task | Status |
|---|------|--------|
| 3.1 | Define `Q6aGpio` struct with pin number, function, and gpiochip device path | [x] |
| 3.2 | Map all 12 GPIO pins to physical pin numbers and alternate functions | [x] |
| 3.3 | Implement `GpioPin` HAL trait: `set_direction()`, `write()`, `read()`, `toggle()` | [x] |
| 3.4 | GPIO access via `/dev/gpiochip4` with sysfs fallback | [x] |
| 3.5 | Pin function multiplexing: `line_to_physical()` / `physical_to_line()` mapping | [x] |
| 3.6 | Create `gpio_pins()` listing all GPIO-capable pin numbers on 40-pin header | [x] |
| 3.7 | Edge detection API: `set_edge(Edge::Rising/Falling/Both)` | [x] |
| 3.8 | Pull-up/pull-down configuration: `set_pull(Pull::Up/Down/None)` | [x] |
| 3.9 | Simulation mode for testing on x86_64 host (`new_simulated()`) | [x] |
| 3.10 | 7 unit tests for GPIO HAL (in `fajar-q6a` repo) | [x] |

### Sprint 4: UART/I2C/SPI HAL

| # | Task | Status |
|---|------|--------|
| 4.1 | Implement `Q6aUart` for 7 UART ports (UART0, 2, 5, 6, 7, 12, 14) | [x] |
| 4.2 | UART via `/dev/ttyMSM*` serial devices, configurable baud rate | [x] |
| 4.3 | Implement `Uart` HAL trait: `init()`, `write_byte()`, `read_byte()`, `write_bytes()` | [x] |
| 4.4 | Implement `Q6aI2c` for 6 I2C buses (I2C0, 2, 6, 7, 12, 14) | [x] |
| 4.5 | I2C via `/dev/i2c-*` devices using Linux i2c-dev interface | [x] |
| 4.6 | Implement `I2c` HAL trait: `write()`, `read()`, `write_read()` | [x] |
| 4.7 | Implement `Q6aSpi` for 7 SPI buses (SPI0, 2, 5, 6, 7, 12, 14) | [x] |
| 4.8 | SPI via `/dev/spidev*` devices using Linux spidev interface | [x] |
| 4.9 | Implement `Spi` HAL trait: `transfer()`, `write_bytes()`, `read_bytes()` | [x] |
| 4.10 | 10 unit tests for UART/I2C/SPI HAL (in `fajar-q6a` repo) | [x] |

---

## Phase 2: On-Device Deployment & Testing (Sprints 5-8)

### Sprint 5: Deploy & Run on Q6A

| # | Task | Status |
|---|------|--------|
| 5.1 | Set up Q6A board: flash Ubuntu 24.04, configure Ethernet/SSH | [x] |
| 5.2 | Deploy `fj` binary via SCP: cross-compile + scp to Q6A | [x] |
| 5.3 | Run all 60 .fj examples on Q6A, verify 60/60 pass | [x] |
| 5.4 | Benchmark interpreter performance on ARM64 (fibonacci, loop, string) | [x] |
| 5.5 | Compare ARM64 vs x86_64 performance numbers (ARM64 ~2x slower, tensor 1.7x) | [x] |
| 5.6 | Create `scripts/deploy-q6a.sh` for one-command deploy+run | [x] |
| 5.7 | Set up `fj` in PATH on Q6A: `/usr/local/bin/fj` | [x] |
| 5.8 | Test REPL mode on Q6A terminal | [x] |
| 5.9 | Verify tensor operations work on ARM64 (ndarray NEON auto-vectorization) | [x] |
| 5.10 | Document deployment procedure in `docs/Q6A_DEPLOY.md` | [x] |

### Sprint 6: Native Codegen on ARM64

| # | Task | Status |
|---|------|--------|
| 6.1 | Verify Cranelift `aarch64` backend generates correct ARM64 code | [x] |
| 6.2 | Test `fj run --native` on Q6A (Cranelift JIT on ARM64) — 128x speedup | [x] |
| 6.3 | Test AOT (Cranelift AOT → ARM64 ELF) — BLOCKED: Aarch64AdrPrelPgHi21 reloc | [ ] |
| 6.4 | Run native codegen tests on Q6A: 5863/5864 pass (1 AOT reloc skip) | [x] |
| 6.5 | Benchmark native vs interpreted: fib(30) 128x, loop 50x faster | [x] |
| 6.6 | Test LLVM backend on ARM64 (inkwell targeting aarch64-linux-gnu) | [ ] |
| 6.7 | Verify ARM64 NEON SIMD instructions in generated code | [ ] |
| 6.8 | Test cross-compiled native binaries run correctly | [x] |
| 6.9 | Profile with `perf` on Q6A: identify hot spots in interpreter | [ ] |
| 6.10 | Create ARM64-specific benchmark suite in `benches/arm64_bench.rs` | [ ] |

### Sprint 7: GPIO Blinky on Q6A

| # | Task | Status |
|---|------|--------|
| 7.1 | Create `examples/q6a_blinky.fj` — toggle GPIO pin via `/dev/gpiochip4` | [x] |
| 7.2 | Wire LED to GPIO pin 7 (GPIO96/MCLK) with current-limiting resistor | [ ] |
| 7.3 | Implement `gpio_open()`, `gpio_set_direction()`, `gpio_write()`, `gpio_read()`, `gpio_toggle()`, `gpio_close()` builtins | [x] |
| 7.4 | Test GPIO read from push button on pin 13 (GPIO0) | [x] |
| 7.5 | Create `examples/q6a_button_led.fj` — button controls LED | [x] |
| 7.6 | Implement `delay_ms()` / `delay_us()` builtins using `std::thread::sleep` | [x] |
| 7.7 | Test I2C sensor read (e.g., BME280 temperature/humidity) | [ ] |
| 7.8 | Create `examples/q6a_i2c_sensor.fj` — read I2C sensor data | [x] |
| 7.9 | Test SPI display output (e.g., SSD1306 OLED) | [ ] |
| 7.10 | Create `examples/q6a_spi_display.fj` — draw text on OLED + verified GPIO on real Q6A hardware | [x] |

### Sprint 8: Serial Communication

| # | Task | Status |
|---|------|--------|
| 8.1 | Create `examples/q6a_uart_echo.fj` — UART loopback test | [x] |
| 8.2 | Test UART5 (pins 8/10) at 115200 baud | [ ] |
| 8.3 | Test UART6 (pins 16/18) for sensor communication | [ ] |
| 8.4 | Implement `uart_open()`, `uart_write_byte()`, `uart_read_byte()`, `uart_write_str()`, `uart_close()` builtins | [x] |
| 8.5 | Create `examples/q6a_uart_gps.fj` — parse NMEA from GPS module | [x] |
| 8.6 | Test I2S audio output (pins 35, 38, 39, 40) | [ ] |
| 8.7 | Implement PWM builtins: `pwm_open()`, `pwm_set_frequency()`, `pwm_set_duty()`, `pwm_enable()`, `pwm_disable()`, `pwm_close()` | [x] |
| 8.8 | Create `examples/q6a_pwm_servo.fj` — control servo motor | [x] |
| 8.9 | Test I3C bus (next-gen I2C) if available in kernel driver | [ ] |
| 8.10 | Write integration test suite for all GPIO/serial/PWM/SPI operations | [x] |

---

## Phase 3: AI/ML on Hexagon NPU (Sprints 9-14)

### Sprint 9: QNN SDK Setup

| # | Task | Status |
|---|------|--------|
| 9.1 | Install Qualcomm AI Engine Direct (QNN) SDK on Q6A | [x] |
| 9.2 | Verify `libQnnHtp.so` and `libQnnHtpV68Skel.so` are present | [x] |
| 9.3 | Verify `libqnnhtpv68.cat` context binary exists | [x] |
| 9.4 | Test `qnn-net-run` with a sample model on HTP backend | [x] |
| 9.5 | Verify NPU detection: `/dev/fastrpc-cdsp`, CDSP running | [x] |
| 9.6 | Benchmark CPU vs NPU inference latency with MobileNet | [ ] |
| 9.7 | Test QNN CPU backend (`libQnnCpu.so`) — verified present on Q6A | [x] |
| 9.8 | Test QNN GPU backend (`libQnnGpu.so`) — verified present on Q6A | [x] |
| 9.9 | Document QNN SDK setup in `docs/Q6A_QNN_SETUP.md` | [x] |
| 9.10 | Create `qnn_version()` builtin — detects QNN SDK version from dpkg | [x] |

### Sprint 10: ONNX → QNN Pipeline

| # | Task | Status |
|---|------|--------|
| 10.1 | Install `qnn-onnx-converter` tool on host (x86_64) | [ ] |
| 10.2 | Export Fajar Lang trained model to ONNX format | [ ] |
| 10.3 | Convert ONNX → QNN model: `qnn-onnx-converter --input_network model.onnx` | [ ] |
| 10.4 | Quantize to INT8: `--input_list calibration.txt` with representative data | [ ] |
| 10.5 | Compile model library: `qnn-model-lib-generator → model.so` | [ ] |
| 10.6 | Generate HTP context binary: `qnn-context-binary-generator` | [ ] |
| 10.7 | Deploy compiled model to Q6A: `/opt/fj/models/` | [ ] |
| 10.8 | Run inference: `qnn-net-run --model model.so --backend libQnnHtp.so` | [ ] |
| 10.9 | Verify INT8 accuracy vs FP32 baseline (target: < 1% accuracy loss) | [ ] |
| 10.10 | Create `scripts/export-qnn.sh` automation script | [ ] |

### Sprint 11: QNN FFI Integration

| # | Task | Status |
|---|------|--------|
| 11.1 | Create `src/runtime/ml/npu/qnn.rs` — FFI bindings to libQnnHtp.so | [x] |
| 11.2 | Implement `dlopen("libQnnHtp.so")` dynamic loading | [x] |
| 11.3 | Bind QNN functions: `QnnInterface_getProviders`, `QnnContext_create`, `QnnGraph_execute` | [x] |
| 11.4 | Implement `qnn_load_model()` with real QNN backend (not simulation) | [x] |
| 11.5 | Implement `qnn_infer()` with real NPU execution | [x] |
| 11.6 | Handle QNN error codes → Fajar Lang `QnnError` mapping | [x] |
| 11.7 | Implement model input/output tensor buffer management | [x] |
| 11.8 | Support multiple concurrent models loaded | [x] |
| 11.9 | Write unit tests with mock QNN library | [x] |
| 11.10 | Write integration test on Q6A with real NPU — all 7 builtins verified | [x] |

### Sprint 12: Fajar Lang NPU Builtins

| # | Task | Status |
|---|------|--------|
| 12.1 | Add `npu_load(path: str) -> i64` builtin to interpreter | [x] |
| 12.2 | Add `npu_infer(model: i64, input: i64) -> i64` builtin | [x] |
| 12.3 | Add `npu_available() -> bool` builtin for runtime detection | [x] |
| 12.4 | Add `npu_info() -> str` builtin returning NPU specs | [x] |
| 12.5 | Register builtins in analyzer type checker | [x] |
| 12.6 | Implement Tensor → QNN buffer conversion (f64 → INT8 quantized) | [x] |
| 12.7 | Implement QNN output → Tensor conversion (INT8 → f64 dequantized) | [x] |
| 12.8 | Create `examples/q6a_npu_classify.fj` — image classification on NPU | [x] |
| 12.9 | Create `examples/q6a_npu_detect.fj` — object detection on NPU | [x] |
| 12.10 | Benchmark NPU inference: 1000 inferences in 4ms (simulation), q/dq roundtrip ok | [x] |

### Sprint 13: NPU Training Pipeline

| # | Task | Status |
|---|------|--------|
| 13.1 | Train MNIST model in Fajar Lang on host (x86_64) | [x] |
| 13.2 | Export trained weights via `model_save`/`model_save_quantized` (FJML/FJMQ) | [x] |
| 13.3 | Convert ONNX → QNN INT8 for Hexagon 770 | [ ] |
| 13.4 | Deploy quantized MNIST model to Q6A `/opt/fj/models/mnist_int8.so` | [ ] |
| 13.5 | Run MNIST inference on NPU: verify > 90% accuracy | [ ] |
| 13.6 | Benchmark MNIST inference latency on NPU (target: < 1ms per image) | [ ] |
| 13.7 | Create end-to-end pipeline: `fj train → fj export → fj deploy → fj infer` | [ ] |
| 13.8 | Test with larger model: ResNet-18 INT8 on NPU | [ ] |
| 13.9 | Test mixed precision: INT8 convolutions + FP16 fully-connected | [ ] |
| 13.10 | Document training→deployment pipeline in `docs/Q6A_ML_PIPELINE.md` | [x] |

### Sprint 14: Camera → NPU Pipeline

| # | Task | Status |
|---|------|--------|
| 14.1 | Access MIPI-CSI camera via V4L2 (`/dev/video*`) | [ ] |
| 14.2 | Capture frame from camera into Fajar Lang Tensor | [ ] |
| 14.3 | Implement image preprocessing: resize, normalize, channel-order conversion | [ ] |
| 14.4 | Pipeline: Camera frame → preprocess → NPU inference → result | [ ] |
| 14.5 | Create `examples/q6a_camera_classify.fj` — live camera classification | [ ] |
| 14.6 | Implement frame rate measurement and display | [ ] |
| 14.7 | Test with all 3 cameras (CSI0 4-lane, CSI1 2-lane, CSI2 2-lane) | [ ] |
| 14.8 | Implement Spectra ISP integration for image quality enhancement | [ ] |
| 14.9 | Test continuous inference loop at target 30 FPS | [ ] |
| 14.10 | Create `examples/q6a_camera_detect.fj` — live object detection | [ ] |

---

## Phase 4: GPU Compute — Adreno 643 (Sprints 15-18)

### Sprint 15: OpenCL 2.0 Setup

| # | Task | Status |
|---|------|--------|
| 15.1 | Verify OpenCL runtime on Q6A: `clinfo` shows Adreno 643 | [ ] |
| 15.2 | Install OpenCL headers and ICD loader | [ ] |
| 15.3 | Create `src/bsp/dragon_q6a/opencl.rs` — OpenCL FFI bindings | [ ] |
| 15.4 | Implement platform/device/context/queue initialization | [ ] |
| 15.5 | Write basic kernel: vector addition in OpenCL C | [ ] |
| 15.6 | Implement buffer create/read/write for OpenCL memory objects | [ ] |
| 15.7 | Test kernel compilation and execution on Adreno 643 | [ ] |
| 15.8 | Implement error handling for OpenCL error codes | [ ] |
| 15.9 | Benchmark GPU vs CPU for vector operations | [ ] |
| 15.10 | Write 10+ unit tests for OpenCL integration | [ ] |

### Sprint 16: GPU Tensor Operations

| # | Task | Status |
|---|------|--------|
| 16.1 | Implement GPU matrix multiplication kernel (OpenCL) | [ ] |
| 16.2 | Implement GPU element-wise operations (add, mul, relu, sigmoid) | [ ] |
| 16.3 | Implement GPU transpose kernel | [ ] |
| 16.4 | Implement GPU reduction kernels (sum, max, argmax) | [ ] |
| 16.5 | Add `gpu_matmul(a: Tensor, b: Tensor) -> Tensor` builtin | [ ] |
| 16.6 | Add `gpu_relu(t: Tensor) -> Tensor` builtin | [ ] |
| 16.7 | Automatic CPU↔GPU data transfer (Tensor pinned memory) | [ ] |
| 16.8 | Benchmark GPU matmul vs CPU matmul on Q6A | [ ] |
| 16.9 | Create `examples/q6a_gpu_matmul.fj` — GPU-accelerated matrix multiply | [ ] |
| 16.10 | Test GPU compute with various tensor sizes (128, 256, 512, 1024) | [ ] |

### Sprint 17: Vulkan Compute

| # | Task | Status |
|---|------|--------|
| 17.1 | Verify Vulkan 1.1 support: `vulkaninfo` on Q6A | [ ] |
| 17.2 | Create `src/bsp/dragon_q6a/vulkan.rs` — Vulkan compute pipeline | [ ] |
| 17.3 | Implement Vulkan instance/device/queue setup for compute | [ ] |
| 17.4 | Write GLSL compute shaders for tensor operations | [ ] |
| 17.5 | Implement Vulkan buffer management for tensor data | [ ] |
| 17.6 | Implement descriptor sets and pipeline layout | [ ] |
| 17.7 | Test Vulkan compute shader execution on Adreno 643 | [ ] |
| 17.8 | Compare Vulkan vs OpenCL performance on Adreno 643 | [ ] |
| 17.9 | Create `examples/q6a_vulkan_compute.fj` — Vulkan-accelerated tensor ops | [ ] |
| 17.10 | Write 10+ unit tests for Vulkan compute | [ ] |

### Sprint 18: GPU Training on Device

| # | Task | Status |
|---|------|--------|
| 18.1 | Implement GPU-accelerated forward pass (matmul + activation) | [ ] |
| 18.2 | Implement GPU-accelerated backward pass (gradient computation) | [ ] |
| 18.3 | Implement GPU-accelerated optimizer step (SGD, Adam) | [ ] |
| 18.4 | Implement CPU↔GPU gradient synchronization | [ ] |
| 18.5 | Train simple model (XOR, iris) entirely on Adreno 643 | [ ] |
| 18.6 | Benchmark GPU training vs CPU training on Q6A | [ ] |
| 18.7 | Create `examples/q6a_gpu_train.fj` — on-device GPU training | [ ] |
| 18.8 | Test memory management: avoid GPU OOM with large batches | [ ] |
| 18.9 | Implement GPU memory pool for training allocations | [ ] |
| 18.10 | Document GPU compute in `docs/Q6A_GPU_COMPUTE.md` | [ ] |

---

## Phase 5: Edge AI Applications (Sprints 19-22)

### Sprint 19: Camera → NPU → GPIO Pipeline

| # | Task | Status |
|---|------|--------|
| 19.1 | Full pipeline: Camera → preprocess → NPU inference → GPIO actuator | [ ] |
| 19.2 | Create `examples/q6a_smart_doorbell.fj` — detect person → trigger buzzer | [ ] |
| 19.3 | Create `examples/q6a_plant_monitor.fj` — classify plant health → I2C display | [ ] |
| 19.4 | Implement watchdog timer for reliable edge deployment | [ ] |
| 19.5 | Implement automatic NPU fallback to CPU if NPU unavailable | [ ] |
| 19.6 | Test continuous 24/7 operation stability (1 hour stress test) | [ ] |
| 19.7 | Implement logging to file for edge deployments | [ ] |
| 19.8 | Implement power management: CPU governor control from Fajar Lang | [ ] |
| 19.9 | Create `examples/q6a_anomaly_detect.fj` — sensor anomaly detection | [ ] |
| 19.10 | Test thermal management: monitor CPU/GPU temperature during inference | [ ] |

### Sprint 20: Multi-Sensor Fusion

| # | Task | Status |
|---|------|--------|
| 20.1 | Read multiple I2C sensors simultaneously (accelerometer, gyroscope, magnetometer) | [ ] |
| 20.2 | Implement sensor data fusion in Fajar Lang (complementary filter) | [ ] |
| 20.3 | Create `examples/q6a_imu_fusion.fj` — 9-axis IMU data fusion | [ ] |
| 20.4 | Implement SPI high-speed data acquisition (ADC sampling) | [ ] |
| 20.5 | Create ring buffer for continuous sensor data stream | [ ] |
| 20.6 | ML inference on fused sensor data (activity recognition) | [ ] |
| 20.7 | Create `examples/q6a_activity_recognition.fj` — classify motion patterns | [ ] |
| 20.8 | Implement UART-based inter-board communication (Q6A → Arduino/MCU) | [ ] |
| 20.9 | Test multi-camera simultaneous capture (CSI0 + CSI1 + CSI2) | [ ] |
| 20.10 | Benchmark sensor read latency for real-time control applications | [ ] |

### Sprint 21: Network AI Services

| # | Task | Status |
|---|------|--------|
| 21.1 | Implement HTTP server in Fajar Lang running on Q6A | [ ] |
| 21.2 | REST API endpoint for NPU inference: POST /infer with image data | [ ] |
| 21.3 | WebSocket streaming for continuous camera + inference results | [ ] |
| 21.4 | Create `examples/q6a_ai_server.fj` — AI inference web server | [ ] |
| 21.5 | Implement MQTT client for IoT sensor data publishing | [ ] |
| 21.6 | Create `examples/q6a_mqtt_sensor.fj` — publish sensor data to MQTT broker | [ ] |
| 21.7 | Implement model hot-reload: update model without restarting | [ ] |
| 21.8 | Implement inference result caching for repeated queries | [ ] |
| 21.9 | Test network throughput: target > 100 inferences/second via HTTP | [ ] |
| 21.10 | Implement TLS/SSL for secure inference API | [ ] |

### Sprint 22: Video Processing Pipeline

| # | Task | Status |
|---|------|--------|
| 22.1 | Implement H.264 hardware decode on Q6A (V4L2 M2M) | [ ] |
| 22.2 | Implement H.265 hardware encode for inference result overlay | [ ] |
| 22.3 | Implement RTSP server for live camera + inference overlay | [ ] |
| 22.4 | Create `examples/q6a_video_detect.fj` — real-time video object detection | [ ] |
| 22.5 | Implement bounding box overlay on decoded frames | [ ] |
| 22.6 | Test 4K@30 decode → inference → 1080p@30 encode pipeline | [ ] |
| 22.7 | Implement multi-stream: 3 cameras → 3 inference pipelines | [ ] |
| 22.8 | Implement HDR10 support for camera capture | [ ] |
| 22.9 | Benchmark video pipeline latency (target: < 50ms glass-to-glass) | [ ] |
| 22.10 | Document video processing in `docs/Q6A_VIDEO_PIPELINE.md` | [ ] |

---

## Phase 6: Production & Release (Sprints 23-24)

### Sprint 23: Production Hardening

| # | Task | Status |
|---|------|--------|
| 23.1 | Implement systemd service for auto-start on boot | [ ] |
| 23.2 | Implement OTA (over-the-air) firmware update mechanism | [ ] |
| 23.3 | Implement crash recovery and automatic restart | [ ] |
| 23.4 | Implement resource monitoring (CPU, RAM, GPU, NPU utilization) | [ ] |
| 23.5 | Implement log rotation and remote log shipping | [ ] |
| 23.6 | Security audit: no exposed ports, TLS everywhere, signed binaries | [ ] |
| 23.7 | Test cold boot → first inference time (target: < 5 seconds) | [ ] |
| 23.8 | Test SD card / NVMe wear leveling for 24/7 operation | [ ] |
| 23.9 | Create production deployment guide: `docs/Q6A_PRODUCTION.md` | [ ] |
| 23.10 | Create hardware BOM (bill of materials) for complete edge AI kit | [ ] |

### Sprint 24: Release & Documentation

| # | Task | Status |
|---|------|--------|
| 24.1 | Update CLAUDE.md with Q6A board support | [ ] |
| 24.2 | Update CHANGELOG.md with v2.0 "Dawn" features | [ ] |
| 24.3 | Create `docs/Q6A_QUICKSTART.md` — 5-minute getting started guide | [ ] |
| 24.4 | Create `docs/Q6A_PINOUT.md` — 40-pin header reference card | [ ] |
| 24.5 | Record demo video: camera → NPU → GPIO on Q6A | [ ] |
| 24.6 | Publish cross-compile Docker image for reproducible builds | [ ] |
| 24.7 | Create GitHub Release with pre-built ARM64 binary | [ ] |
| 24.8 | Update mdBook with Q6A chapter | [ ] |
| 24.9 | Write blog post: "Fajar Lang on Radxa Dragon Q6A" | [ ] |
| 24.10 | Tag release: `v2.0.0-dawn` | [ ] |

---

## Architecture Diagram

```
                        ┌─────────────────────────────────────────────────┐
                        │         Radxa Dragon Q6A (QCS6490)              │
                        │                                                  │
  Camera ──────────┐    │  ┌──────────┐   ┌──────────┐   ┌──────────┐   │
  (MIPI CSI)       │    │  │   CPU    │   │   GPU    │   │   NPU    │   │
                   │    │  │ Kryo 670 │   │Adreno 643│   │Hexagon770│   │
  I2C Sensors ─────┤    │  │ 8 cores  │   │ 812 MHz  │   │ 12 TOPS  │   │
  SPI Devices ─────┤    │  │ 2.7 GHz  │   │ OpenCL   │   │  INT8    │   │
  UART Modules ────┤    │  │          │   │ Vulkan   │   │  QNN SDK │   │
                   │    │  └────┬─────┘   └────┬─────┘   └────┬─────┘   │
  GPIO (40-pin) ───┤    │       │              │              │          │
  /dev/gpiochip4   │    │       └──────────────┼──────────────┘          │
                   │    │                      │                          │
                   │    │              ┌───────┴────────┐                 │
                   │    │              │  Fajar Lang    │                 │
                   │    │              │  Runtime (fj)  │                 │
                   │    │              │                │                 │
                   │    │              │  ┌──────────┐  │                 │
                   └────┼──────────────┤  │ BSP:     │  │                 │
                        │              │  │ Q6A HAL  │  │                 │
                        │              │  │ GPIO/I2C │  │                 │
                        │              │  │ SPI/UART │  │                 │
                        │              │  │ NPU/GPU  │  │                 │
                        │              │  └──────────┘  │                 │
                        │              └────────────────┘                 │
                        │                                                  │
                        │  16GB LPDDR5 │ NVMe SSD │ WiFi6 │ GbE │ BT5.4 │
                        └─────────────────────────────────────────────────┘
```

## Deployment Flow

```
Host (x86_64)                          Dragon Q6A (aarch64)
┌─────────────────┐                    ┌─────────────────────┐
│ 1. Write .fj    │                    │                     │
│ 2. cargo build  │ ──── SCP ─────────>│ 4. ./fj run app.fj  │
│    --target     │                    │                     │
│    aarch64-...  │                    │ 5. GPIO/NPU/GPU     │
│ 3. fj export    │ ──── SCP ─────────>│    auto-detected    │
│    --onnx model │    (model.so)      │                     │
└─────────────────┘                    └─────────────────────┘

Train (host) → Export (ONNX) → Convert (QNN INT8) → Deploy (Q6A) → Infer (NPU 12 TOPS)
```

## Key Differences: Dragon Q6A vs Dragonwing IQ8

| Feature | Dragonwing IQ8 (existing BSP) | Dragon Q6A (new BSP) |
|---------|------------------------------|----------------------|
| **Role** | MPU module in VENTUNO Q | Standalone SBC |
| **NPU** | 40 TOPS (Hexagon Tensor) | 12 TOPS (Hexagon 770 V68) |
| **GPU** | Adreno @ 877MHz, Vulkan 1.3 | Adreno 643 @ 812MHz, Vulkan 1.1 |
| **GPIO** | None (via MCU IPC) | 40-pin header, /dev/gpiochip4 |
| **RAM** | 16GB LPDDR5 (fixed) | 4-16GB LPDDR5 (configurable) |
| **Network** | 2.5GbE | 1GbE |
| **Bluetooth** | 5.3 | 5.4 |
| **Display** | HDMI (via MCU) | HDMI 4K@30 + MIPI DSI |
| **Storage** | eMMC + NVMe | eMMC + UFS + NVMe + microSD |
| **OS** | Linux (custom) | Ubuntu 24.04 (mainline) |
| **Form Factor** | Module (in VENTUNO Q) | Credit-card SBC (85x56mm) |
| **Price** | Part of VENTUNO Q | $59.50 - $124.29 standalone |
| **Context** | `@device` only | `@safe` + `@device` + `@kernel` (all) |

---

## Official SDK & Tools Stack (from Radxa docs)

### QAIRT SDK v2.37.1

| Tool | Purpose | Command |
|------|---------|---------|
| `qairt-converter` | ONNX/TF/PyTorch → DLC | `qairt-converter --input_network model.onnx -d 'input' 1,3,224,224` |
| `qairt-quantizer` | DLC → INT8 DLC | `qairt-quantizer --input_dlc model.dlc --input_list calib.txt` |
| `qnn-context-binary-generator` | DLC → Context Binary | `qnn-context-binary-generator --model lib.so --backend libQnnHtp.so --dlc_path model.dlc` |
| `qnn-net-run` | Run inference | `qnn-net-run --backend libQnnHtp.so --retrieve_context model.bin --input_list test.txt` |
| `genie-t2t-run` | LLM inference | `genie-t2t-run -c config.json -p 'prompt'` |

### NPU Runtime Libraries

```
libQnnHtp.so          → HTP backend (NPU inference)
libQnnHtpV68Stub.so   → V68 stub library
libQnnHtpV68Skel.so   → V68 DSP firmware skeleton
libQnnCpu.so          → CPU fallback backend
libQnnGpu.so          → GPU backend (FP16)
```

### FastRPC Device Nodes

```
/dev/fastrpc-adsp     → Application DSP
/dev/fastrpc-cdsp     → Compute DSP (NPU)
/dev/fastrpc-cdsp-secure → Secure compute DSP
```

### Pre-built Models Available (QCS6490)

| Model | Type | Performance |
|-------|------|-------------|
| ResNet50 (INT8) | Classification | Few ms |
| YOLOv8-det | Object detection | ~33ms inference |
| GoogLeNet | Classification | Few ms |
| Inception v3 | Classification (TFLite) | Few ms |
| FCN-ResNet50 | Segmentation | Few ms |
| Real-ESRGAN | 4x super-resolution | Few ms |
| **Llama 3.2-1B** | LLM | 12 tok/s gen, 172 tok/s prompt |
| **Qwen 2.5-0.5B** | LLM | 24 tok/s gen, 309 tok/s prompt |

### GPU Benchmark (vkpeak on Adreno 643)

| Metric | Performance |
|--------|------------|
| FP32 scalar | **773 GFLOPS** |
| FP16 vec4 | **1,581 GFLOPS** |
| INT8 dotprod | **1,176 GIOPS** |
| Memory bandwidth | 9.06 GB/s |

### CPU Frequency Scaling

| Policy | Cluster | Range |
|--------|---------|-------|
| policy0 | Silver (4x A55) | 300 MHz - 1.96 GHz |
| policy4 | Gold (3x A78) | 691 MHz - 2.4 GHz |
| policy7 | Prime (1x A78) | 806 MHz - 2.71 GHz |

### GPIO Control

```bash
sudo apt install python3-periphery
# Access: /dev/gpiochip4
# Python: from periphery import GPIO; gpio = GPIO("/dev/gpiochip4", 25, "out")
```

### Alternative AI Inference Paths

| Path | Library | Notes |
|------|---------|-------|
| QAIRT native | qnn-net-run | Best performance, Context Binary format |
| ONNX Runtime QNN EP | onnxruntime_qnn wheel | Python API, `providers=["QNNExecutionProvider"]` |
| TFLite Delegate | qtld-net-run | TFLite models with `--backend htp` |
| QAI AppBuilder | Python library | Simplified deployment API |

### Docker for Development

```bash
docker pull radxazifeng278/qairt-npu:v1.0  # QCS6490 QAIRT SDK
```

---

*V2.0 "Dawn" Plan Version: 1.2 | Updated: 2026-03-15 | 72/240 tasks (30%) | Hardware: Radxa Dragon Q6A (QCS6490)*
*Source: docs.radxa.com/en/dragon/q6a/app-dev*
