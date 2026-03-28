# FajarOS Nova — Hardware Verification Plan

> **Date:** 2026-03-28
> **Target Hardware:** Intel Core i9-14900HX (Lenovo Legion Pro), Radxa Dragon Q6A
> **Kernel:** fajaros_nova_kernel.fj — 21,187 lines, 819 @kernel functions

---

## x86_64 Verification (Lenovo Legion Pro)

### Boot Tests

| # | Test | Method | Expected Result | Status |
|---|------|--------|-----------------|--------|
| H1 | Boot to serial output | QEMU -serial stdio | "FajarOS Nova" banner | [ ] |
| H2 | Boot to VGA text | QEMU -display gtk | Text on screen | [ ] |
| H3 | Shell prompt | Serial | "nova>" prompt | [ ] |
| H4 | Basic commands | Serial type | help, uname, ps work | [ ] |
| H5 | Bare-metal USB boot | dd to USB, BIOS boot | Boots on real hardware | [ ] |

### Subsystem Tests

| # | Test | Method | Expected Result | Status |
|---|------|--------|-----------------|--------|
| H6 | NVMe detection | QEMU -drive nvme | Controller identified | [ ] |
| H7 | FAT32 read/write | QEMU with FAT32 image | File operations work | [ ] |
| H8 | Network (virtio-net) | QEMU -net nic,virtio | ARP/ICMP/TCP work | [ ] |
| H9 | SMP (4 cores) | QEMU -smp 4 | All APs initialized | [ ] |
| H10 | USB (XHCI) | QEMU -device qemu-xhci | Devices enumerated | [ ] |
| H11 | GPU (virtio-gpu) | QEMU -device virtio-gpu | Framebuffer active | [ ] |
| H12 | Process fork/exec | Shell command | Child processes run | [ ] |
| H13 | Ring 3 user programs | Boot | "Hello Ring 3!" printed | [ ] |
| H14 | GDB remote debug | QEMU -s -S + gdb | Breakpoints work | [ ] |

### Performance Tests

| # | Test | Method | Expected Result | Status |
|---|------|--------|-----------------|--------|
| H15 | Boot time | Serial timestamp | < 2 seconds to shell | [ ] |
| H16 | Context switch latency | Timer measurement | < 1ms | [ ] |
| H17 | NVMe sector read | Benchmark command | < 10ms per 4KB | [ ] |

---

## ARM64 Verification (Radxa Dragon Q6A / QCS6490)

| # | Test | Method | Expected Result | Status |
|---|------|--------|-----------------|--------|
| A1 | SSH to Q6A | ssh radxa@192.168.50.94 | Connected | [ ] |
| A2 | Cross-compile for ARM64 | fj build --target aarch64 | Binary produced | [ ] |
| A3 | Boot FajarOS Surya | Load via adb/fastboot | Serial output | [ ] |
| A4 | GPIO blink LED | GPIO pin toggle | LED blinks | [ ] |
| A5 | QNN inference | QNN model load + run | Output tensor correct | [ ] |
| A6 | Vulkan compute | VkComputePipeline | matmul result correct | [ ] |
| A7 | Camera capture | V4L2 | Frame captured | [ ] |

---

## QEMU Automated Test Commands

```bash
# Quick boot test (10 seconds)
qemu-system-x86_64 -kernel examples/fajaros_nova_kernel \
    -serial stdio -display none -no-reboot -m 128M

# Full test with NVMe + network
qemu-system-x86_64 -kernel examples/fajaros_nova_kernel \
    -serial stdio -display none -no-reboot -m 128M \
    -drive file=/tmp/nova_nvme.img,if=none,id=nvme0,format=raw \
    -device nvme,serial=deadbeef,drive=nvme0 \
    -net nic,model=virtio -net user \
    -smp 4

# GDB debug session
qemu-system-x86_64 -kernel examples/fajaros_nova_kernel \
    -serial stdio -display none -no-reboot -m 128M -s -S &
gdb -ex "target remote :1234" -ex "break kernel_main" -ex "continue"

# Run automated test script
./examples/fajaros_nova_test.sh
```

---

## Sign-Off Criteria

For production release, ALL of the following must pass:

- [ ] x86_64 QEMU: all 17 tests (H1-H17) green
- [ ] ARM64 Q6A: tests A1-A2 green (A3-A7 when hardware available)
- [ ] CI: nova.yml workflow passes on every push
- [ ] Kernel: `fj check` reports 0 errors, 0 warnings
- [ ] Test scripts: all 4 scripts (v1, v2, v3, kvm) pass
