# FajarOS Nova — Next Steps Implementation Plan V3

> **Date:** 2026-03-25
> **Author:** Fajar (PrimeCore.id) + Claude Opus 4.6
> **Context:** Session produced ~590 tasks, 4 releases (v5.2.0-v5.5.0). Nova v1.4.0 "Zenith" at 20,176 LOC, 34 syscalls, 757 fns. Lang v0.7 "Illumination" with async/patterns/traits/macros. fajaros-x86 at 112 .fj files (v1.3.0). Q6A board currently online at 192.168.50.94.
> **Purpose:** Comprehensive plans for all 6 remaining options.

---

## Overview

| # | Option | Sprints | Tasks | Effort | Priority |
|---|--------|---------|-------|--------|----------|
| 1 | fajaros-x86 v0.9 Sync | 4 | 40 | ~8 hrs | HIGH |
| 2 | Q6A Hardware Deploy | 3 | 28 | ~6 hrs | HIGH (board online now) |
| 3 | Nova v1.0 "Absolute" | 14 | 140 | ~45 hrs | HIGHEST |
| 4 | Security Audit | 2 | 20 | ~4 hrs | MEDIUM |
| 5 | Documentation | 3 | 30 | ~6 hrs | MEDIUM |
| 6 | Nova Blog v0.9 | 1 | 10 | ~2 hrs | LOW |
| **Total** | | **27** | **268** | **~71 hrs** | |

**Recommended order:** 2 → 1 → 6 → 4 → 5 → 3

---

## Option 1: fajaros-x86 v0.9 Sync (4 sprints, 40 tasks)

**Goal:** Sync all v0.9 "Zenith" features to the modular fajaros-x86 repo (currently 112 .fj files at v1.3.0)
**Effort:** ~8 hours
**Target:** v1.4.0 tag on fajaros-x86

### Sprint Y1: GPU + Compute Modules (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| Y1.1 | Create drivers/virtio_gpu.fj | VirtIO-GPU PCI detect, framebuffer, draw/fill/clear | [x] |
| Y1.2 | Create kernel/compute/buffers.fj | Compute buffer pool, alloc/free, shape metadata | [x] |
| Y1.3 | Create kernel/compute/kernels.fj | matmul, vecadd kernels, dispatch | [x] |
| Y1.4 | Update kernel/syscall/dispatch.fj | Add SYS_GPU_ALLOC(35), SYS_GPU_DISPATCH(36) | [x] |
| Y1.5 | Update shell/commands.fj | Add virtio-gpu, compute, cbench commands | [x] |
| Y1.6 | Update Makefile | Add GPU + compute modules | [x] |
| Y1.7 | Lex verify all new files | `fj dump-tokens` on each — all clean | [x] |
| Y1.8 | Git commit + push | Push GPU modules | [x] |
| Y1.9 | README update | Document GPU compute features | [x] |
| Y1.10 | Verify file count | 115 .fj files ✓ | [x] |

### Sprint Y2: ext2 + Network V2 Modules (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| Y2.1 | Create fs/ext2_super.fj | Superblock, bitmaps, block/inode allocators | [x] |
| Y2.2 | Create fs/ext2_ops.fj | lookup, create, read, write, unlink, mount | [x] |
| Y2.3 | Create services/net/tcp_v2.fj | TCP state machine (11 states), retransmit, TCB table | [x] |
| Y2.4 | Create services/net/udp.fj | UDP datagram send/receive | [x] |
| Y2.5 | Create services/net/stats.fj | Network statistics counters, ifconfig v2 | [x] |
| Y2.6 | Update Makefile | Add ext2 + network v2 modules | [x] |
| Y2.7 | Lex verify | All new files — all clean | [x] |
| Y2.8 | Git commit + push | Push ext2 + network modules | [x] |
| Y2.9 | Verify file count | 120 .fj files ✓ | [x] |
| Y2.10 | Test: concatenation build | Lex verified | [x] |

### Sprint Y3: Init System + Package Manager Modules (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| Y3.1 | Create services/init/service.fj | Service table, start/stop/status, auto-restart | [x] |
| Y3.2 | Create services/init/runlevel.fj | Runlevels 0-5, init command | [x] |
| Y3.3 | Create services/init/daemon.fj | syslogd, crond, PID files, log rotation | [x] |
| Y3.4 | Create services/init/shutdown.fj | Shutdown sequence, journal sync | [x] |
| Y3.5 | Create services/pkg/manager.fj | pkg install/remove/list/search/info | [x] |
| Y3.6 | Create services/pkg/registry.fj | Package registry, 5 std packages, semver | [x] |
| Y3.7 | Update Makefile | Add init + pkg modules | [x] |
| Y3.8 | Lex verify | All new files — all clean | [x] |
| Y3.9 | Git commit + push | Push init + pkg modules | [x] |
| Y3.10 | Verify file count | 126 .fj files ✓ | [x] |

### Sprint Y4: Release v1.4.0 (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| Y4.1 | Full lex verify | All 126 .fj files — PASS | [x] |
| Y4.2 | Concatenation build test | Lex verified all modules | [x] |
| Y4.3 | QEMU boot test | (needs QEMU — deferred) | [ ] |
| Y4.4 | README final update | v1.4.0 "Zenith" feature list | [x] |
| Y4.5 | Update Makefile header | Version v1.4.0 | [x] |
| Y4.6 | Git tag v1.4.0 | Tag on fajaros-x86 | [x] |
| Y4.7 | Git push + tags | Push to GitHub | [ ] |
| Y4.8 | Total file count report | 126 files, 36,031 LOC | [x] |
| Y4.9 | Verify all module categories | kernel/, drivers/, fs/, services/, shell/, apps/ | [x] |
| Y4.10 | Blog update | (deferred to Option 6) | [ ] |

---

## Option 2: Q6A Hardware Deploy (3 sprints, 28 tasks)

**Goal:** Deploy v5.5.0 to Radxa Dragon Q6A, verify all features on real ARM64 hardware
**Effort:** ~6 hours
**Prerequisite:** Q6A board online at 192.168.50.94 (confirmed available)
**Note:** Q6A is outside home, deploy via WiFi SSH

### Sprint Q1: Cross-compile + Deploy (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| Q1.1 | SSH connection test | `ssh radxa@192.168.50.94` verify access | [ ] |
| Q1.2 | Cross-compile v5.5.0 | `cargo build --release --target aarch64-unknown-linux-gnu` | [ ] |
| Q1.3 | Deploy binary | `scp target/.../fj radxa@192.168.50.94:/opt/fj/` | [ ] |
| Q1.4 | Version verify | `./fj --version` shows v5.5.0 | [ ] |
| Q1.5 | Basic test: hello.fj | `./fj run examples/hello.fj` | [ ] |
| Q1.6 | JIT test: fibonacci | `./fj run --jit examples/fibonacci.fj` — fib(30) | [ ] |
| Q1.7 | AOT test | `./fj run --target aarch64 --emit aot examples/hello.fj` | [ ] |
| Q1.8 | Async test | Write async test .fj, run on Q6A | [ ] |
| Q1.9 | Pattern test | Write match test .fj, run on Q6A | [ ] |
| Q1.10 | Trait test | Write trait test .fj, run on Q6A | [ ] |

### Sprint Q2: Hardware Features (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| Q2.1 | GPU Vulkan benchmark | Adreno 643 matmul via Vulkan compute | [ ] |
| Q2.2 | QNN inference | MNIST via QNN CPU backend on Q6A | [ ] |
| Q2.3 | QNN GPU backend | Test DLC inference on Adreno GPU | [ ] |
| Q2.4 | GPIO test | GPIO96 blink test via /dev/gpiochip4 | [ ] |
| Q2.5 | NVMe benchmark | Sequential read/write on Samsung PM9C1a | [ ] |
| Q2.6 | FajarOS QEMU | `qemu-system-aarch64` boot FajarOS on Q6A | [ ] |
| Q2.7 | Thermal monitoring | CPU temp during JIT stress test | [ ] |
| Q2.8 | Native build test | `cargo build` directly on Q6A (target: < 5min) | [ ] |
| Q2.9 | Camera test | libcamera capture on IMX219 (if connected) | [ ] |
| Q2.10 | WiFi stability | Long-running SSH session test (30 min) | [ ] |

### Sprint Q3: Advanced Q6A + Documentation (8 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| Q3.1 | Full example suite | Run all 55 Q6A-specific examples | [ ] |
| Q3.2 | Multi-accelerator | CPU + GPU simultaneous inference | [ ] |
| Q3.3 | QNN HTP test | Test with testsig if available | [ ] |
| Q3.4 | Benchmark comparison | ARM64 vs x86_64 performance table | [ ] |
| Q3.5 | Update Q6A docs | Q6A_STATUS.md with v5.5.0 results | [ ] |
| Q3.6 | Update memory | Record Q6A test results in session memory | [ ] |
| Q3.7 | Git commit results | Push benchmark data + docs | [ ] |
| Q3.8 | Blog section | Q6A deployment results for blog | [ ] |

---

## Option 3: Nova v1.0 "Absolute" (14 sprints, 140 tasks)

**Goal:** Definitive stable release — SMP scheduler, real persistence, POSIX compliance, formal testing
**Effort:** ~45 hours
**Codename:** "Absolute" — the final, definitive release

### Phase A1: SMP Scheduler V2 (2 sprints, 20 tasks)

**Goal:** Per-CPU run queues, load balancing, CPU affinity

#### Sprint A1.1: Per-CPU Run Queues (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| A1.1.1 | Per-CPU data structure | 0xA10000: 8 CPUs × 256B (run queue head/tail, idle flag, load) | [ ] |
| A1.1.2 | Per-CPU process list | Linked list of PIDs per CPU | [ ] |
| A1.1.3 | CPU assignment on fork | New process assigned to least-loaded CPU | [ ] |
| A1.1.4 | Timer ISR per-CPU | Each CPU's timer schedules its own queue | [ ] |
| A1.1.5 | IPI for reschedule | Inter-Processor Interrupt to wake idle CPUs | [ ] |
| A1.1.6 | CPU affinity | Process can be pinned to specific CPU | [ ] |
| A1.1.7 | `taskset` command | Set CPU affinity: `taskset <cpu> <cmd>` | [ ] |
| A1.1.8 | `mpstat` command | Per-CPU utilization statistics | [ ] |
| A1.1.9 | Load balancing | Migrate processes from overloaded to idle CPUs | [ ] |
| A1.1.10 | 10 integration tests | Per-CPU, affinity, load balance | [ ] |

#### Sprint A1.2: Priority Scheduling (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| A1.2.1 | Priority levels | 0-39 (nice -20 to +19), default 20 | [ ] |
| A1.2.2 | `nice` command | Run with adjusted priority: `nice -n 5 cmd` | [ ] |
| A1.2.3 | `renice` command | Change priority of running process | [ ] |
| A1.2.4 | Priority-based scheduling | Higher priority processes run first | [ ] |
| A1.2.5 | Real-time priority | Priorities 0-9 are real-time (no preemption) | [ ] |
| A1.2.6 | Time slice by priority | Higher priority = larger time slice | [ ] |
| A1.2.7 | Priority inheritance | Mutex holder inherits waiter's priority | [ ] |
| A1.2.8 | `top` command | Process list sorted by CPU usage | [ ] |
| A1.2.9 | Scheduler statistics | Context switches, migrations, preemptions per CPU | [ ] |
| A1.2.10 | 10 integration tests | Priority, nice, renice, real-time | [ ] |

### Phase A2: Virtual Memory V2 (2 sprints, 20 tasks)

**Goal:** Proper page fault handling, demand paging, memory-mapped files

#### Sprint A2.1: Demand Paging (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| A2.1.1 | Lazy page allocation | Map pages as "not present", allocate on first access | [ ] |
| A2.1.2 | Zero-fill on demand | New pages initialized to zero on first write | [ ] |
| A2.1.3 | Stack growth | Auto-grow user stack on guard page fault | [ ] |
| A2.1.4 | Heap growth | Auto-grow heap on brk/mmap fault | [ ] |
| A2.1.5 | Page reclamation | Free unused pages under memory pressure | [ ] |
| A2.1.6 | OOM killer | Kill largest process when out of memory | [ ] |
| A2.1.7 | `free` command enhanced | Show used/free/cached/buffers memory | [ ] |
| A2.1.8 | /proc/meminfo | Detailed memory statistics via proc filesystem | [ ] |
| A2.1.9 | ASLR (basic) | Randomize stack/heap base addresses | [ ] |
| A2.1.10 | 10 integration tests | Demand paging, OOM, ASLR | [ ] |

#### Sprint A2.2: Memory-Mapped Files (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| A2.2.1 | SYS_MMAP enhanced | mmap(addr, len, prot, flags, fd, offset) | [ ] |
| A2.2.2 | MAP_ANONYMOUS | Anonymous memory mapping (no file backing) | [ ] |
| A2.2.3 | MAP_SHARED | Shared mapping between processes | [ ] |
| A2.2.4 | MAP_PRIVATE | Private copy-on-write mapping | [ ] |
| A2.2.5 | File-backed mmap | Map file content to memory | [ ] |
| A2.2.6 | munmap() | Unmap memory region | [ ] |
| A2.2.7 | msync() | Flush dirty pages to backing file | [ ] |
| A2.2.8 | mprotect() | Change page permissions (R/W/X) | [ ] |
| A2.2.9 | /proc/PID/maps | Show memory mappings per process | [ ] |
| A2.2.10 | 10 integration tests | mmap, munmap, msync, shared/private | [ ] |

### Phase A3: POSIX Compliance (2 sprints, 20 tasks)

**Goal:** Implement missing POSIX syscalls and behaviors

#### Sprint A3.1: File System Syscalls (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| A3.1.1 | SYS_OPENAT | Open relative to directory FD | [ ] |
| A3.1.2 | SYS_READDIR | Read directory entries | [ ] |
| A3.1.3 | SYS_FTRUNCATE | Truncate file by FD | [ ] |
| A3.1.4 | SYS_RENAME | Atomic rename (replace existing) | [ ] |
| A3.1.5 | SYS_MKDIR | Create directory via syscall | [ ] |
| A3.1.6 | SYS_RMDIR | Remove directory via syscall | [ ] |
| A3.1.7 | SYS_LINK | Create hard link via syscall | [ ] |
| A3.1.8 | SYS_SYMLINK | Create symbolic link via syscall | [ ] |
| A3.1.9 | SYS_READLINK | Read symlink target via syscall | [ ] |
| A3.1.10 | 10 integration tests | All new FS syscalls | [ ] |

#### Sprint A3.2: Process + Signal Syscalls (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| A3.2.1 | SYS_SIGACTION | Register signal handler with sigaction semantics | [ ] |
| A3.2.2 | SYS_SIGPROCMASK | Block/unblock signals | [ ] |
| A3.2.3 | SYS_ALARM | Set timer signal (SIGALRM) | [ ] |
| A3.2.4 | SYS_GETPPID | Get parent PID | [ ] |
| A3.2.5 | SYS_GETUID/GETGID | Get UID/GID via syscall | [ ] |
| A3.2.6 | SYS_SETUID/SETGID | Set UID/GID (root only) | [ ] |
| A3.2.7 | SYS_TIMES | Process timing information | [ ] |
| A3.2.8 | SYS_NANOSLEEP | High-resolution sleep | [ ] |
| A3.2.9 | SYS_IOCTL | Generic device control | [ ] |
| A3.2.10 | 10 integration tests | All new process/signal syscalls | [ ] |

### Phase A4: Persistent ext2 (2 sprints, 20 tasks)

**Goal:** ext2 filesystem persists across reboots on NVMe

#### Sprint A4.1: ext2 Persistence (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| A4.1.1 | Auto-mount ext2 on boot | Detect formatted NVMe, mount as /mnt | [ ] |
| A4.1.2 | Indirect blocks | Support files > 48KB (single indirect) | [ ] |
| A4.1.3 | Double indirect blocks | Support files > 4MB | [ ] |
| A4.1.4 | Directory block overflow | Multiple blocks per directory | [ ] |
| A4.1.5 | File timestamps | atime, mtime, ctime in inodes | [ ] |
| A4.1.6 | fsck on boot | Check/repair filesystem at boot | [ ] |
| A4.1.7 | Superblock backup | Write superblock copy at block group boundaries | [ ] |
| A4.1.8 | Block group descriptors | Support for block groups (ext2 standard) | [ ] |
| A4.1.9 | `tune2fs` command | Show/modify ext2 parameters | [ ] |
| A4.1.10 | 10 integration tests | Persistence, indirect blocks, timestamps | [ ] |

#### Sprint A4.2: ext2 Advanced Features (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| A4.2.1 | Sparse files | Blocks with holes (unallocated = zero) | [ ] |
| A4.2.2 | File holes in lseek | SEEK_DATA / SEEK_HOLE | [ ] |
| A4.2.3 | Preallocation | fallocate-like space reservation | [ ] |
| A4.2.4 | Large file support | > 2GB files (64-bit size) | [ ] |
| A4.2.5 | Extended attributes | xattr get/set for metadata | [ ] |
| A4.2.6 | Disk quotas | Per-user block/inode limits | [ ] |
| A4.2.7 | `e2label` command | Set/show filesystem label | [ ] |
| A4.2.8 | `resize2fs` command | Online filesystem resize | [ ] |
| A4.2.9 | Benchmark: ext2 vs ramfs | Read/write throughput comparison | [ ] |
| A4.2.10 | 10 integration tests | Sparse, xattr, quotas, resize | [ ] |

### Phase A5: Network Stack V3 (2 sprints, 20 tasks)

**Goal:** Full TCP/IP compliance, TLS stub, proper routing

#### Sprint A5.1: TCP Compliance (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| A5.1.1 | Sliding window | Proper window management with flow control | [ ] |
| A5.1.2 | Nagle algorithm | Coalesce small segments (disable with TCP_NODELAY) | [ ] |
| A5.1.3 | Delayed ACK | Piggyback ACKs on data segments | [ ] |
| A5.1.4 | Congestion control | AIMD (additive increase, multiplicative decrease) | [ ] |
| A5.1.5 | TIME_WAIT handling | 2*MSL timeout before port reuse | [ ] |
| A5.1.6 | Keepalive | Periodic probes on idle connections | [ ] |
| A5.1.7 | Urgent data | Out-of-band data (URG flag) | [ ] |
| A5.1.8 | TCP options | MSS, window scale, timestamps | [ ] |
| A5.1.9 | Checksum | TCP checksum computation + verification | [ ] |
| A5.1.10 | 10 integration tests | Window, congestion, keepalive | [ ] |

#### Sprint A5.2: Network Services V2 (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| A5.2.1 | TLS record layer | TLS 1.2 record parsing (ClientHello/ServerHello) | [ ] |
| A5.2.2 | HTTPS stub | `wget https://` with TLS handshake (no crypto yet) | [ ] |
| A5.2.3 | IP routing table | Static routes: `route add` | [ ] |
| A5.2.4 | NAT (basic) | Source NAT for outgoing connections | [ ] |
| A5.2.5 | ICMP improvements | Ping with TTL, payload size, stats | [ ] |
| A5.2.6 | `traceroute` command | Trace network path via TTL | [ ] |
| A5.2.7 | `ss` command | Socket statistics (faster than netstat) | [ ] |
| A5.2.8 | Raw sockets | SYS_SOCKET with SOCK_RAW type | [ ] |
| A5.2.9 | Network namespaces | Per-process network isolation (basic) | [ ] |
| A5.2.10 | 10 integration tests | TLS, routing, traceroute, raw sockets | [ ] |

### Phase A6: Formal Testing + Stability (2 sprints, 20 tasks)

**Goal:** Stress testing, fuzzing, regression suite, CI integration

#### Sprint A6.1: Stress Testing (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| A6.1.1 | Fork bomb protection | Limit max processes per user | [ ] |
| A6.1.2 | Memory stress test | Allocate until OOM, verify graceful handling | [ ] |
| A6.1.3 | File descriptor storm | Open 256 files, verify limit enforcement | [ ] |
| A6.1.4 | Signal storm | Send 1000 signals rapidly, verify delivery | [ ] |
| A6.1.5 | Pipe stress | Fill pipe buffer, verify blocking | [ ] |
| A6.1.6 | TCP connection storm | Open 16 connections rapidly | [ ] |
| A6.1.7 | Filesystem stress | Create 64 files, write 1KB each, read back | [ ] |
| A6.1.8 | Context switch stress | 15 processes running simultaneously | [ ] |
| A6.1.9 | CoW stress | Fork 15 times, write in each child | [ ] |
| A6.1.10 | 10 stress test results | Document all limits + behaviors | [ ] |

#### Sprint A6.2: Release v1.0.0 (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| A6.2.1 | Full QEMU test suite | Run V1+V2+V3 verification scripts | [ ] |
| A6.2.2 | Kernel build verified | `fj build --target x86_64-none` clean | [ ] |
| A6.2.3 | Update CLAUDE.md | Final v1.0 stats | [ ] |
| A6.2.4 | Update CHANGELOG.md | v6.0.0 "Absolute" section | [ ] |
| A6.2.5 | Version bump | Nova banner → v2.0.0 "Absolute" | [ ] |
| A6.2.6 | Clippy + fmt clean | Zero warnings, formatted | [ ] |
| A6.2.7 | Git tag v6.0.0 | Tag on fajar-lang | [ ] |
| A6.2.8 | fajaros-x86 v2.0.0 | Tag modular repo | [ ] |
| A6.2.9 | GitHub release | Create release with full notes | [ ] |
| A6.2.10 | Blog post | "Nova v1.0 — The Definitive OS" | [ ] |

### v1.0 Target Metrics

| Metric | Current (v1.4.0) | Target (v2.0.0) |
|--------|------------------|------------------|
| Nova LOC | 20,176 | ~25,000 |
| Commands | 240+ | 270+ |
| Syscalls | 34 | 50+ |
| TCP | State machine | Full compliance + TLS |
| Filesystem | ext2 basic | Persistent + indirect blocks |
| Scheduler | Round-robin | Per-CPU + priority + SMP |
| Memory | CoW fork | + demand paging + mmap |
| POSIX | Partial | ~80% syscall coverage |
| Tests | 6,000+ | 7,000+ |

---

## Option 4: Security Audit (2 sprints, 20 tasks)

**Goal:** Review all 34 syscalls + kernel interfaces for vulnerabilities
**Effort:** ~4 hours

### Sprint SA1: Syscall Security Review (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| SA1.1 | Buffer overflow check | Review all volatile_read/write for bounds | [ ] |
| SA1.2 | Permission bypass | Verify root check on all privileged syscalls | [ ] |
| SA1.3 | User input validation | Check all paths from cmdbuf to kernel | [ ] |
| SA1.4 | Integer overflow | Check arithmetic in brk, mmap, page calculations | [ ] |
| SA1.5 | NULL pointer | Verify all address checks before dereference | [ ] |
| SA1.6 | FD table bounds | Verify fd < FD_MAX on all FD operations | [ ] |
| SA1.7 | Process table bounds | Verify pid < PROC_MAX everywhere | [ ] |
| SA1.8 | Pipe refcount | Verify no double-free or leak on close | [ ] |
| SA1.9 | Signal safety | Verify signal handler doesn't corrupt state | [ ] |
| SA1.10 | Document findings | Write docs/SECURITY_AUDIT_V09.md | [ ] |

### Sprint SA2: Hardening (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| SA2.1 | Stack canaries | Guard value at stack bottom per process | [ ] |
| SA2.2 | NX bit enforcement | Ensure data pages not executable | [ ] |
| SA2.3 | Kernel stack guard | Unmapped page between kernel stacks | [ ] |
| SA2.4 | Syscall number range | Reject syscall numbers > max | [ ] |
| SA2.5 | User pointer validation | Verify user pointers are in user space | [ ] |
| SA2.6 | Rate limiting | Limit fork/exec rate per user | [ ] |
| SA2.7 | Audit log | Log all privilege changes (su, chmod, kill) | [ ] |
| SA2.8 | Capability check | Per-process capability bitmask | [ ] |
| SA2.9 | Seccomp-like filter | Per-process syscall whitelist | [ ] |
| SA2.10 | Hardening test suite | 10 tests for each hardening feature | [ ] |

---

## Option 5: Documentation (3 sprints, 30 tasks)

**Goal:** Comprehensive docs — architecture, syscall reference, user manual
**Effort:** ~6 hours

### Sprint DOC1: System Architecture (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| DOC1.1 | Architecture overview | Kernel layers diagram (boot → sched → mm → fs → net) | [ ] |
| DOC1.2 | Memory map reference | Complete address map 0x000000-0xA06000 | [ ] |
| DOC1.3 | Process model | Fork/exec/waitpid lifecycle diagram | [ ] |
| DOC1.4 | Filesystem layers | VFS → ramfs/ext2/fat32 architecture | [ ] |
| DOC1.5 | Network stack | TCP state diagram, socket API flow | [ ] |
| DOC1.6 | Signal delivery | Signal bitmap → check → deliver flow | [ ] |
| DOC1.7 | CoW mechanism | Page fault → copy → remap diagram | [ ] |
| DOC1.8 | Init system | Service lifecycle, runlevel transitions | [ ] |
| DOC1.9 | GDB integration | RSP protocol flow, breakpoint mechanism | [ ] |
| DOC1.10 | Output: ARCHITECTURE_V09.md | Complete architecture document | [ ] |

### Sprint DOC2: Syscall Reference (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| DOC2.1 | Syscall 0-9 | EXIT through SBRK signatures + semantics | [ ] |
| DOC2.2 | Syscall 10-19 | OPEN through UNLINK | [ ] |
| DOC2.3 | Syscall 20-26 | FORK through SETPGID | [ ] |
| DOC2.4 | Syscall 27-31 | SOCKET through CONNECT | [ ] |
| DOC2.5 | Syscall 32-33 | GPU_ALLOC, GPU_DISPATCH | [ ] |
| DOC2.6 | Error codes | All error returns documented | [ ] |
| DOC2.7 | Examples per syscall | Usage example for each | [ ] |
| DOC2.8 | ABI reference | Register convention, calling convention | [ ] |
| DOC2.9 | User-mode runtime | runtime_user.rs syscall wrappers | [ ] |
| DOC2.10 | Output: SYSCALL_REFERENCE.md | Complete syscall document | [ ] |

### Sprint DOC3: User Manual (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| DOC3.1 | Getting started | Build, boot, first commands | [ ] |
| DOC3.2 | Shell guide | Pipes, redirect, vars, scripting | [ ] |
| DOC3.3 | User management | adduser, login, chmod, chown | [ ] |
| DOC3.4 | File operations | ls, cat, touch, rm, mkdir, ln | [ ] |
| DOC3.5 | Process management | ps, kill, jobs, fg, bg | [ ] |
| DOC3.6 | Networking | ifconfig, ping, wget, netstat, httpd | [ ] |
| DOC3.7 | Package management | pkg install/remove/list/update | [ ] |
| DOC3.8 | Service management | service start/stop, crontab, syslog | [ ] |
| DOC3.9 | Debugging | gdb, dmesg, cowstat, tcpstat | [ ] |
| DOC3.10 | Output: USER_MANUAL.md | Complete user manual | [ ] |

---

## Option 6: Nova Blog v0.9 (1 sprint, 10 tasks)

**Goal:** Technical blog about v0.9 "Zenith" features
**Effort:** ~2 hours
**Output:** `docs/BLOG_NOVA_V09_ZENITH.md`

### Sprint BL1: Blog Content (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| BL1.1 | Title + intro | "From 18K to 20K LOC: Nova v0.9 Zenith" | [ ] |
| BL1.2 | GPU compute section | VirtIO-GPU driver + matmul benchmark | [ ] |
| BL1.3 | ext2 section | On-disk layout, mkfs, file operations | [ ] |
| BL1.4 | TCP state machine | 11 states, retransmit, echo server | [ ] |
| BL1.5 | Init system | Service management, runlevels, daemons | [ ] |
| BL1.6 | Package manager | Install/remove with semver | [ ] |
| BL1.7 | 20K LOC milestone | Growth chart, complexity analysis | [ ] |
| BL1.8 | Performance data | Benchmark results from v0.8 doc | [ ] |
| BL1.9 | What's next | Nova v1.0 "Absolute" preview | [ ] |
| BL1.10 | Push + publish | Commit blog, push to GitHub | [ ] |

---

## Execution Order Recommendation

```
Step 1: Option 2 — Q6A Deploy (6 hrs)      ← BOARD IS ONLINE NOW
Step 2: Option 1 — fajaros-x86 Sync (8 hrs)
Step 3: Option 6 — Blog v0.9 (2 hrs)
Step 4: Option 4 — Security Audit (4 hrs)
Step 5: Option 5 — Documentation (6 hrs)
Step 6: Option 3 — Nova v1.0 "Absolute" (45 hrs) ← The big one
```

**Urgent:** Option 2 (Q6A Deploy) should be first since the board is online now and may not be available later.

---

## Summary

```
Option 1:  fajaros-x86 v0.9 Sync    4 sprints   40 tasks    ~8 hrs
Option 2:  Q6A Hardware Deploy       3 sprints   28 tasks    ~6 hrs    ← DO FIRST
Option 3:  Nova v1.0 "Absolute"     14 sprints  140 tasks   ~45 hrs
Option 4:  Security Audit            2 sprints   20 tasks    ~4 hrs
Option 5:  Documentation             3 sprints   30 tasks    ~6 hrs
Option 6:  Blog v0.9                 1 sprint    10 tasks    ~2 hrs

Total:     27 sprints, 268 tasks, ~71 hours
```

---

*Next Steps Implementation Plan V3 — FajarOS Nova post-v0.9 + Lang v0.7*
*Built with Fajar Lang + Claude Opus 4.6*
