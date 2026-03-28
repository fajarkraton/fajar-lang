# FajarOS Surya — ARM64 Bare-Metal OS

ARM64 kernel for Radxa Dragon Q6A (QCS6490) written in Fajar Lang.

## Status

| Feature | Status |
|---------|--------|
| Cross-compilation | **WORKS** — `fj build --target aarch64-unknown-none-elf --no-std` |
| ELF output | **82 KB** aarch64 binary |
| PL011 UART | Implemented in fajaros_arm64_boot.fj |
| MMU (4KB pages) | Implemented in fajaros_arm64_mmu.fj |
| IPC | Implemented in fajaros_arm64_ipc.fj |
| Multi-process | Implemented in fajaros_arm64_multi.fj |
| Shell | Implemented in fajaros_arm64_shell.fj |
| Q6A BSP | 73 tests passing (Vulkan + QNN) |
| QEMU ARM64 | Supported via `qemu-system-aarch64` |
| Q6A hardware | Pending (SSH: radxa@192.168.50.94) |

## ARM64 Examples

| File | Lines | Description |
|------|-------|-------------|
| `fajaros_arm64_boot.fj` | 164 | PL011 UART, GIC, timer, EL2→EL1 |
| `fajaros_arm64_mmu.fj` | 142 | MMU with 4KB page tables |
| `fajaros_arm64_ipc.fj` | 145 | Shared memory IPC |
| `fajaros_arm64_multi.fj` | 159 | Multi-process + scheduler |
| `fajaros_arm64_shell.fj` | 103 | Interactive shell |

All 5 compile to native ARM64 ELF via Cranelift cross-compilation.

## Build

```bash
# Single file
fj build --target aarch64-unknown-none-elf --no-std examples/fajaros_arm64_boot.fj

# QEMU test
qemu-system-aarch64 -M virt -cpu cortex-a72 -nographic \
    -kernel examples/fajaros_arm64_boot -serial mon:stdio
```

## Q6A Hardware Target

| Component | Specification |
|-----------|--------------|
| SoC | Qualcomm QCS6490 |
| CPU | Kryo 670 (Cortex-A78 + A55) |
| GPU | Adreno 643 |
| NPU | QNN v2.40 (Hexagon DSP) |
| RAM | 7.4 GB |
| SSH | radxa@192.168.50.94 |

### Hardware Test Plan

```bash
# 1. SSH to Q6A
ssh radxa@192.168.50.94

# 2. Copy binary
scp examples/fajaros_arm64_boot radxa@192.168.50.94:~/

# 3. GPIO test (needs root)
sudo ./gpio_test

# 4. QNN inference
./qnn_inference_test model.dlc

# 5. Vulkan compute
./vulkan_matmul_test
```
