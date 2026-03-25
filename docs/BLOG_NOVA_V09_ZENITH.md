# From 18K to 20K LOC: FajarOS Nova v0.9 "Zenith"

> **Author:** Fajar (PrimeCore.id)
> **Date:** 2026-03-25
> **Tags:** OS development, Fajar Lang, x86_64, GPU compute, ext2, TCP, init system

---

## The Summit

FajarOS Nova v0.9 "Zenith" represents the peak of the Nova kernel — every major OS subsystem now has a real implementation. In one sprint, we added a VirtIO-GPU driver, an ext2 filesystem, a proper TCP state machine, an init system with runlevels, and a package manager. The monolithic kernel grew from 18,159 to **20,176 lines** of Fajar Lang, with **757 @kernel functions** and **34 syscalls**.

Then we synced it all to the modular **fajaros-x86** repo: 112 → **126 .fj files**, 36,031 lines across 9 directory categories.

```
Nova v0.9 "Zenith" at a glance:
  Kernel:    20,176 LOC (single file) / 36K LOC (modular)
  Functions: 757 @kernel
  Syscalls:  34 (was 32)
  Commands:  240+ shell commands
  New:       GPU, ext2, TCP v2, init, pkg manager
```

---

## VirtIO-GPU: Pixels from Bare Metal

The most visually dramatic addition. Nova now detects a VirtIO-GPU device on the PCI bus, initializes its virtqueues, and can draw to a 320×200×32bpp framebuffer.

```fajar
@kernel fn gpu_draw_pixel(x: i64, y: i64, color: i64) {
    if x < 0 || x >= GPU_FB_WIDTH || y < 0 || y >= GPU_FB_HEIGHT { return }
    let offset = (y * GPU_FB_WIDTH + x) * GPU_FB_BPP
    volatile_write_u32_le(GPU_FB_BASE + offset, color)
}
```

The full VirtIO-GPU protocol is implemented: `RESOURCE_CREATE_2D`, `RESOURCE_ATTACH_BACKING`, `SET_SCANOUT`, `TRANSFER_TO_HOST_2D`, and `RESOURCE_FLUSH`. Every command goes through the VirtIO control virtqueue at 0x9E0000.

### GPU Compute Dispatch

Beyond display, Nova has a compute buffer pool — 16 slots of 4KB each — with CPU-fallback kernels:

- **matmul**: Naive O(n³) matrix multiply with dimension checking
- **vecadd**: Element-wise vector addition

These are exposed as syscalls (`SYS_GPU_ALLOC`, `SYS_GPU_DISPATCH`) so user processes can request compute operations. The `gpubench` command benchmarks an 8×8 matmul and verifies correctness:

```
nova> gpubench
GPU Compute Benchmark:
  8x8 matmul: 0 ms
  C[0,0] = 36 (expected 36)
```

---

## ext2: A Real Filesystem

Before v0.9, Nova had FAT32 (for USB/NVMe compatibility) and RamFS (for the in-memory root). Now it has **ext2** — the classic Linux filesystem format.

### On-Disk Layout

```
Sector 0-1:   Boot (reserved)
Sector 2-3:   Superblock (1KB, magic 0xEF53)
Sector 4-7:   Block bitmap (16K blocks trackable)
Sector 8-11:  Inode bitmap (256 inodes)
Sector 12-139: Inode table (256 × 128B)
Sector 140+:  Data blocks (4KB each)
```

The implementation is simplified but correct: bitmap allocators for blocks and inodes, 128-byte inodes with 12 direct block pointers, and 128-byte directory entries with lookup-by-name.

### mkfs.ext2

You can format an NVMe partition from the shell:

```
nova> mkfs.ext2
Formatting ext2...
ext2 formatted:
  Inodes: 256
  Blocks: 16384
  Block size: 4096 bytes
  Root inode: 2
```

The formatter writes the superblock (with magic 0xEF53), clears both bitmaps, creates root inode 2 (directory, mode 0755), and marks inodes 1-2 as used. Standard ext2 conventions.

### File Operations

Full CRUD: `ext2_create()` allocates an inode + data block, adds a directory entry, and writes the inode back. `ext2_read_file()` follows block pointers. `ext2_unlink()` clears the directory entry (block reclamation is TODO — just like early Linux).

---

## TCP State Machine: RFC 793 in Fajar Lang

The old TCP implementation was a minimal send/receive pair. v0.9 has a **real state machine** with all 11 RFC 793 states:

```
CLOSED → LISTEN → SYN_RCVD → ESTABLISHED → FIN_WAIT_1 → FIN_WAIT_2 → TIME_WAIT
                  SYN_SENT ↗               CLOSE_WAIT → LAST_ACK
                                            CLOSING ↗
```

### TCP Control Blocks

16 connections tracked simultaneously, each with a 128-byte TCB:

```
Per TCB:
  +0:  state        +32: snd_nxt      +64: timer
  +8:  local_port   +40: snd_una      +72: retries
  +16: remote_ip    +48: rcv_nxt      +80: socket
  +24: remote_port  +56: window       +88: rx_len
```

The retransmit timer fires every 200ms (20 ticks). After 5 failed retries, the connection resets. The `tcpstat` command shows active connections:

```
nova> tcpstat
TCP connections:
  [0] :7 → :0 seq=0 ack=0 LISTEN
  [1] :49201 → :23 seq=1001 ack=0 SYN_SENT
```

### Multi-Client Server

`tcp_server_accept()` allocates a new TCB for each incoming connection, copying the listen socket's local port. The echo server on port 7 demonstrates this.

---

## Network Statistics + UDP

### Stats Counters

Every packet now updates counters at 0xA06000:

```
nova> netstat
Network statistics:
  RX packets: 0     TX packets: 0
  RX bytes:   0     TX bytes:   0
  Errors:     0     Dropped:    0
  TCP conns:  1     UDP dgrams: 3
```

### ARP Aging

The ARP cache now expires entries after 30 seconds (300 ticks at 100Hz). This prevents stale MAC address mappings — essential for real networks where devices come and go.

### UDP + DNS v2

The new UDP implementation builds proper headers (big-endian port + length + checksum). DNS resolution now retries 3 times before giving up, using incrementing source ports to avoid NAT confusion.

---

## Init System: Services Done Right

### Service Manager

16 services tracked in a 64-byte table per service:

```
Per service:
  +0:  name[16]      +32: restart_policy
  +16: pid            +40: start_tick
  +24: state          +48: restart_count
```

Three restart policies: `NO` (manual only), `ALWAYS` (restart on any exit), `ON_FAILURE` (restart only on non-zero exit). The `service` command manages everything:

```
nova> service status
Services:
  syslogd: running
  crond: running
  httpd: stopped
  sshd: running (restarts: 1)
```

### Runlevels

Classic UNIX runlevels: 0 (halt), 1 (single-user), 3 (multi-user), 5 (graphical). The `init <level>` command switches between them. `init 0` triggers a clean shutdown.

### Daemons

- **syslogd**: Ring buffer at 0x9B5000, timestamped entries, rotation at 64KB
- **crond**: 8-slot crontab, tick-based intervals, executes shell commands
- **PID files**: Track which service owns which PID for clean shutdown ordering

### Shutdown Sequence

`shutdown_services()` stops services in reverse registration order, commits the journal, and syncs the filesystem. No more abrupt halts.

---

## Package Manager

32-package database with semver comparison. Five standard packages ship pre-registered:

| Package | Version | Description |
|---------|---------|-------------|
| fj-math | 1.0.0 | Math library |
| fj-nn | 1.0.0 | Neural network |
| fj-hal | 1.0.0 | Hardware abstraction |
| fj-http | 1.0.0 | HTTP client/server |
| fj-crypto | 1.0.0 | Cryptography |

The `pkg` command supports `install`, `remove`, `list`, `search`, and `info` subcommands. Packages track state (available/installed), version, dependencies, file count, and install timestamp.

---

## Modular Build: 126 Files

The fajaros-x86 repo now organizes Nova into 126 modular `.fj` files across 9 categories:

```
kernel/   62 files  — mm, sched, syscall, ipc, security, debug, compute
drivers/  10 files  — serial, vga, keyboard, pci, nvme, virtio, gpu, xhci
fs/        9 files  — ramfs, fat32, vfs, ext2, journal, fsck, links
services/ 26 files  — init, net, gpu, display, input, gui, auth, pkg, vfs
shell/     6 files  — commands, pipes, redirect, vars, control, scripting
apps/      6 files  — user programs, editor, compiler, pkgmgr, mnist
arch/      2 files  — aarch64 boot, Q6A tests
tests/     4 files  — kernel tests, benchmarks, context enforcement
lib/       1 file   — user-mode syscall wrappers
```

The `Makefile` concatenates them in dependency order into a single `combined.fj`, which the Fajar Lang compiler turns into one ELF binary. Total: **36,031 lines**.

---

## Performance Snapshot

| Operation | Time |
|-----------|------|
| Cold boot to shell | ~50ms (QEMU) |
| 8×8 matmul (compute) | < 1ms |
| ext2 mkfs | ~5ms |
| TCP connect (loopback) | < 1ms |
| Service start | immediate |

All measurements on QEMU with KVM. Real hardware (i9-14900HX) would be faster.

---

## What's Next: Nova v1.0 "Absolute"

v0.9 "Zenith" is the peak. v1.0 "Absolute" will be the **definitive** release:

- **SMP Scheduler V2**: Per-CPU run queues, load balancing, priority scheduling
- **Virtual Memory V2**: Demand paging, mmap, OOM killer, ASLR
- **POSIX Compliance**: openat, readdir, sigaction, nanosleep, ioctl
- **Persistent ext2**: Indirect blocks, timestamps, fsck on boot
- **Network V3**: Sliding window, congestion control, TLS record layer
- **Stress Testing**: Fork bomb protection, FD storm, memory pressure

14 sprints, 140 tasks. The goal: a complete, correct, stress-tested OS — all in Fajar Lang.

---

## The Numbers

```
v0.6 "Ascension"  →  12,954 LOC  →  181 commands  →  5 syscalls
v0.7 "Nexus"      →  15,732 LOC  →  200 commands  → 26 syscalls
v0.8 "Bastion"    →  18,159 LOC  →  220 commands  → 32 syscalls
v0.9 "Zenith"     →  20,176 LOC  →  240 commands  → 34 syscalls

Growth: +7,222 LOC in 4 releases, zero lines of C or assembly.
Everything compiled with: fj build --target x86_64-none
```

Every line of kernel code is checked by the Fajar Lang compiler's context system. `@kernel` functions cannot allocate heap strings or create tensors. `@device` functions cannot touch hardware registers. If it compiles, the privilege boundaries are correct.

That's the promise of Fajar Lang: **if it compiles, it's safe to deploy.**

---

*Built with Fajar Lang + Claude Opus 4.6*
*GitHub: [github.com/fajarkraton/fajar-lang](https://github.com/fajarkraton/fajar-lang)*
*FajarOS: [github.com/fajarkraton/fajaros-x86](https://github.com/fajarkraton/fajaros-x86)*
