# FajarOS Nova — Modular Kernel Source

The 21,211-line monolithic kernel split into 14 manageable modules.

## Modules

| Module | Lines | Description |
|--------|-------|-------------|
| `memory.fj` | 585 | Bitmap frame allocator (128MB), page tables (4KB), freelist heap (1.5MB) |
| `ipc.fj` | 110 | Per-process message queues (4 × 64-byte slots) |
| `security.fj` | 55 | Multi-user (16 accounts), login/logout, permissions |
| `smp.fj` | 293 | AP trampoline (INIT-SIPI-SIPI), per-CPU run queues |
| `nvme.fj` | 737 | NVMe 1.4 driver (admin + I/O queues, sector R/W) |
| `fat32.fj` | 750 | FAT32 filesystem (cluster chains, 8.3 names, R/W) |
| `vfs.fj` | 322 | Virtual Filesystem Switch (ramfs, FAT32, devfs, procfs) |
| `network.fj` | 1,458 | TCP (RFC 793), UDP, ARP, ICMP, HTTP server |
| `syscall.fj` | 410 | 34 syscalls via table dispatch |
| `process.fj` | 780 | CoW fork, exec, waitpid, signals, preemptive scheduler |
| `shell_core.fj` | 4,500 | Shell engine: pipes, redirects, $VAR, if/for/while |
| `shell_commands.fj` | 4,000 | 240+ shell commands |
| `services.fj` | 3,100 | Init system, syslogd, crond, package manager |
| `extensions.fj` | 4,111 | USB (XHCI), GPU, GDB stub, late additions |

**Total: 21,211 lines across 14 modules**

## Build

```bash
# Concatenate modules into single kernel source
./build.sh

# Concatenate + type check
./build.sh --check

# Concatenate + native compile
./build.sh --compile
```

The concatenation order matters — modules that define constants and helper
functions must come before modules that use them.

## Why Concatenation?

Fajar Lang's `use` module system works for interpreter and hosted targets,
but bare-metal `@kernel` compilation currently requires a single source file.
The module split is for **readability and maintainability** — the build
script reassembles them into a single file for compilation.

When the compiler adds bare-metal multi-file support, these modules can
be imported directly with `use nova::memory`, `use nova::network`, etc.
