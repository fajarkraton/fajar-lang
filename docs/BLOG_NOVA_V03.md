# FajarOS Nova v0.3 "Endurance" — A Real OS in a Custom Language

> **Author:** Fajar (PrimeCore.id)
> **Date:** 2026-03-21
> **Release:** Nova v0.3.0 "Endurance"

---

## What is FajarOS Nova?

FajarOS Nova is a **bare-metal x86_64 operating system** written entirely in **Fajar Lang** — a custom systems programming language. No C, no Rust, no assembly (except auto-generated). Just `.fj` source code compiled to a bootable ELF.

### v0.3 Stats

```
Kernel:    8,327 lines of Fajar Lang
Binary:    215 KB ELF (x86_64, statically linked)
Commands:  135 shell commands
Functions: 328 @kernel functions
Boot:      Multiboot2 → GRUB2 → 64-bit long mode
```

---

## What's New in v0.3

### NVMe Storage (Phase 11)
Real NVMe SSD driver — admin queue, I/O queue, sector read/write.
```
[NVMe] Controller enabled
[NVMe] I/O queues ready
[NVMe] Sector 0 read OK — boot signature found (0x55AA)
```

### FAT32 Filesystem (Phase 12)
Read AND write files on a FAT32 partition:
- `fatls` — list directory
- `fatcat file.txt` — read file
- `fatwrite file.txt Hello` — create file
- `fatrm file.txt` — delete file
- Files persist across reboots

### VFS + /dev + /proc (Phase 13)
Unified filesystem with virtual devices:
- `/` — ramfs (64 files, 832KB)
- `/dev/null`, `/dev/zero`, `/dev/random`
- `/proc/version`, `/proc/uptime`
- `/mnt` — FAT32 on NVMe

### SMP Multi-Core (Phase 14)
AP trampoline: 16-bit → 32-bit → 64-bit at 0x8000.
INIT-SIPI-SIPI to boot application processors.
Verified with 4, 8, and 24 cores on KVM.

### TCP/IP Network Stack (Phase 15)
Ethernet frame builder, ARP cache, IPv4 with checksum, ICMP ping.
```
$ ping 10.0.2.2
Sent 54 byte ICMP echo to 10.0.2.2
Reply: 64 bytes, ttl=64, time<1ms
```

### ELF Loader + Syscalls (Phase 16)
ELF64 parser, PT_LOAD segment loading, 8 syscalls.
`exec` command loads ELF from FAT32 and transitions to Ring 3.

### Process Management (v0.3)
Fork, exit, waitpid — 16-process table with zombie reaping.

### PS/2 Keyboard (v0.3)
Scancode set 1 → ASCII, ring buffer, IRQ1 via `port_inb(0x60)`.

### Shell Scripting (v0.3)
`source init.sh` — execute commands from FAT32 file.

### Pipes (v0.3)
8 pipes × 4KB, create/read/write, per-process FD table.

---

## The Compiler Story

Fajar Lang compiled to bare-metal required 30+ new builtins:

| Builtin | Purpose |
|---------|---------|
| `volatile_read/write_u64` | NVMe 64-bit registers |
| `buffer_read/write_u32_le` | FAT32 little-endian fields |
| `port_inb/outb/inw/outw` | PS/2 keyboard, PCI I/O |
| `pci_write32` | Enable NVMe bus master |
| `ltr`, `lgdt_mem`, `lidt_mem` | GDT/TSS for Ring 3 |
| `hlt`, `cli`, `sti`, `pause` | CPU control |
| `cpuid_eax/ebx/ecx/edx` | Feature detection |
| `memcpy_buf`, `memset_buf` | Fast buffer operations |
| `iretq_to_user` | Ring 0 → Ring 3 transition |

Also fixed: parser bug where `(expr)` on a new line was chained as a function call.

---

## Hardware Validation

Tested on Intel i9-14900HX (24 cores) via KVM:

| Config | Boot | NVMe | FAT32 | SMP |
|--------|------|------|-------|-----|
| KVM basic | PASS | fallback | — | — |
| KVM + NVMe | PASS | PASS | PASS | — |
| KVM + SMP 8 | PASS | — | — | PASS |
| KVM + SMP 24 | PASS | — | — | PASS |
| Full config | PASS | PASS | PASS | PASS |

---

## Boot Sequence

```
Multiboot2 → GDT → IDT → 4-level Paging → Bitmap Allocator
→ Heap → PIT 100Hz → Keyboard → NVMe → FAT32 → VFS
→ Network → ELF/Syscall → Process Table → Shell
```

12 subsystems initialized before the first shell prompt.

---

## Version History

| Version | LOC | Commands | Key Features |
|---------|-----|----------|-------------|
| v0.1 | 4,944 | 102 | Boot, ramfs, VGA, PCI, MNIST |
| v0.2 | 7,313 | 122 | +NVMe, FAT32 read, VFS, SMP, Net, ELF |
| **v0.3** | **8,327** | **135** | **+FAT32 write, Ring 3, fork, keyboard, pipes** |

---

## Try It

```bash
# Build
cargo build --release --features native
cargo run --release --features native -- build --target x86_64-none \
    examples/fajaros_nova_kernel.fj

# Boot
qemu-system-x86_64 -cdrom nova.iso \
    -drive file=disk.img,format=raw,if=none,id=nvme0 \
    -device nvme,serial=FJ001,drive=nvme0 \
    -boot d -smp 4 -m 256M -serial stdio
```

---

*FajarOS Nova — 8,327 lines of Fajar Lang that boot into a real operating system.*
