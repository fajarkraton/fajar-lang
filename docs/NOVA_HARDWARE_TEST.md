# FajarOS Nova — Hardware Validation Results

> **Date:** 2026-03-21
> **Hardware:** Intel Core i9-14900HX (24 cores, 32 threads), Lenovo Legion Pro
> **Method:** QEMU with KVM acceleration (-enable-kvm)

## Test Matrix

| Config | Boot | NVMe | FAT32 | VFS | NET | ELF | PROC | KB | SMP |
|--------|------|------|-------|-----|-----|-----|------|----|-----|
| QEMU emulated | PASS | PASS | PASS | PASS | PASS | PASS | PASS | PASS | PASS (4 cores) |
| KVM, no disk | PASS | fallback | — | PASS | PASS | PASS | PASS | PASS | — |
| KVM + NVMe, SMP=1 | PASS | PASS | PASS | PASS | PASS | PASS | PASS | PASS | — |
| KVM + NVMe, SMP=4 | PASS | PASS* | PASS | PASS | PASS | PASS | PASS | PASS | PASS |
| KVM + NVMe, SMP=8 | PASS | SLOW** | — | PASS | PASS | PASS | PASS | PASS | PASS |
| KVM + NVMe, SMP=24 | PASS | fallback | — | PASS | PASS | PASS | PASS | PASS | PASS |
| KVM + NVMe + XHCI | PASS | PASS | PASS | PASS | PASS | PASS | PASS | PASS | — |
| KVM + SMP=8, no NVMe | PASS | fallback | — | PASS | PASS | PASS | PASS | PASS | PASS |
| KVM + SMP=24, no NVMe | PASS | fallback | — | PASS | PASS | PASS | PASS | PASS | PASS |

*PASS: Full NVMe init takes ~8s under KVM with SMP=4
**SLOW: NVMe Identify stalls with SMP=8 (timeout too short for multi-core KVM)

## Key Findings

1. **KVM boot works on all configs** — no triple faults, no crashes
2. **NVMe driver works under KVM** but polling timeout needs tuning for SMP>4
3. **SMP up to 24 cores** boots successfully (BSP only — AP boot is interactive)
4. **USB XHCI** detected under KVM
5. **Ramdisk fallback** works correctly when NVMe is not available or times out

## Performance Notes

- KVM NVMe init slower than QEMU emulated (real PCI config space access vs simulated)
- Boot-to-shell: ~3s (no NVMe) / ~8s (with NVMe) under KVM
- Interpreter fib(30): 47.9s / JIT fib(30): 0.109s (440x speedup)

## Recommendation

- Increase NVMe polling timeout from 1,000,000 to 10,000,000 for KVM compatibility
- SMP + NVMe: use SMP=1 or SMP=4 for reliable NVMe init
- Real hardware boot (non-KVM): requires bootable USB — deferred to physical test
