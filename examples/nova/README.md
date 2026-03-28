# FajarOS Nova v1.4.0 "Zenith"

x86_64 bare-metal operating system written 100% in Fajar Lang.

## Statistics

| Metric | Value |
|--------|-------|
| Lines of code | 21,187 |
| @kernel functions | 819 |
| Shell commands | 240+ |
| Syscalls | 34 |
| Type-check errors | **0** |

## Subsystems

| Subsystem | Description |
|-----------|-------------|
| **Memory** | Bitmap frame allocator (128MB), freelist heap (1.5MB), page tables (4KB mapping) |
| **Processes** | CoW fork, exec, waitpid, signals, job control, 16 process table |
| **Scheduler** | Preemptive round-robin, timer-driven context switch |
| **Ring 3** | User-mode programs via SYSCALL/MSR, 5 built-in programs |
| **Filesystem** | VFS (ramfs, FAT32, devfs, procfs), hierarchical directories |
| **Storage** | NVMe driver (admin + I/O queues), ramdisk, block device table |
| **Network** | TCP state machine (RFC 793), UDP, ARP, ICMP, HTTP server |
| **USB** | XHCI controller, device enumeration, control transfers |
| **GPU** | VirtIO-GPU framebuffer (320x200), compute dispatch |
| **SMP** | AP trampoline (INIT-SIPI-SIPI), per-CPU data |
| **Debug** | GDB remote stub (RSP protocol, breakpoints, watchpoints) |
| **Users** | Multi-user (16 accounts), login/logout, passwd, chmod/chown |
| **Services** | Init system, syslogd, crond, auto-restart, runlevels |
| **Shell** | Pipes, redirects, $VAR, scripts, if/for/while, 240+ commands |
| **Packages** | pkg install/remove/list/search/update (5 std packages) |

## Build & Run

```bash
# Type-check kernel
fj check examples/fajaros_nova_kernel.fj

# Boot in QEMU
cd examples/nova && make run

# Run automated tests
cd examples/nova && make test
```

## Files

| File | Description |
|------|-------------|
| `fajaros_nova_kernel.fj` | Main kernel source (21,187 lines) |
| `fajaros_nova_kernel` | Pre-compiled flat binary (331KB) |
| `fajaros_nova_boot.fj` | Minimal boot example |
| `fajaros_nova_minimal.fj` | Minimal kernel (serial hello) |
| `fajaros_nova_test.sh` | Automated QEMU test suite |
| `fajaros_nova_v2_test.sh` | v0.8 Bastion test suite |
| `fajaros_nova_v3_test.sh` | v0.7 Nexus shell test suite |
| `fajaros_nova_kvm_test.sh` | KVM acceleration test |
| `nova_phoenix_*.fj` | Phoenix extensions (GUI, POSIX, net, persist, audio) |
| `nova_aurora_*.fj` | Aurora extensions (SMP, compositor, services, USB) |

## Verified On

- **QEMU:** x86_64, 128MB RAM, NVMe, virtio-net, XHCI, SMP 4 cores
- **Hardware target:** Intel Core i9-14900HX (Lenovo Legion Pro)
