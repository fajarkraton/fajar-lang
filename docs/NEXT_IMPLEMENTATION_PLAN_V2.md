# FajarOS Nova — Next Steps Implementation Plan V2

> **STATUS: SUPERSEDED** — This plan (V2) was superseded by V3, V4, and later plans.
> All features described here have been implemented.
> See `NEXT_IMPLEMENTATION_PLAN_V8.md` for current active plan.

> **Date:** 2026-03-25
> **Author:** Fajar (PrimeCore.id) + Claude Opus 4.6
> **Context:** Nova v0.8 "Bastion" COMPLETE (120/120 tasks). Nova v1.3.0 shipped: 18,159 LOC, 651 @kernel fns, 229 commands, 32 syscalls, CoW fork, multi-user, journaling FS, HTTP server, GDB debugger. 6,186 tests. Both v0.7 "Nexus" + v0.8 "Bastion" = 360 tasks completed in one session.
> **Purpose:** Detailed plans for 6 next-step options post-v0.8.

---

## Overview

| # | Option | Sprints | Tasks | Effort | Priority |
|---|--------|---------|-------|--------|----------|
| 6 | Blog Post | 2 | 20 | ~4 hrs | MEDIUM |
| 7 | Nova v0.9 "Zenith" Plan | 12 | 120 | ~40 hrs | HIGH |
| 5 | v2.0 "Dawn" Q6A Deploy | 2 | 18 | ~4 hrs | MEDIUM (needs HW) |
| 8 | Fajar Lang v0.7 | 10 | 100 | ~35 hrs | HIGH |
| 9 | fajaros-x86 v0.8 Sync | 4 | 40 | ~10 hrs | MEDIUM |
| 10 | Performance Benchmarks | 2 | 20 | ~4 hrs | LOW |
| **Total** | | **32** | **318** | **~97 hrs** | |

**Recommended order:** 6 → 9 → 10 → 7 → 5 → 8

---

## Option 6: Blog Post (2 sprints, 20 tasks)

**Goal:** Technical blog about the Nova v0.7+v0.8 journey — from demo OS to production
**Effort:** ~4 hours
**Output:** `docs/BLOG_NOVA_V08_BASTION.md`

### Sprint B1: Technical Content (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| B1.1 | Title + intro | "Building a Production OS in Fajar Lang: 360 Tasks in One Session" | [x] |
| B1.2 | Before/after | v0.6 (demo shell) → v0.7 (UNIX process model) → v0.8 (production OS) | [x] |
| B1.3 | Syscall dispatch | Diagram: linker asm → indirect call → syscall_dispatch → 32 handlers | [x] |
| B1.4 | fork() deep dive | CoW page tables, refcount, page fault → copy, instant fork | [x] |
| B1.5 | Multi-user design | User table, password hashing, permission model, session management | [x] |
| B1.6 | Filesystem journal | WAL design, commit/replay, crash recovery, fsck verification | [x] |
| B1.7 | HTTP server | Socket API → bind/listen/accept → request parse → file serve → log | [x] |
| B1.8 | GDB remote stub | RSP protocol, breakpoints (INT3), watchpoints (DR0-DR3), thread query | [x] |
| B1.9 | Lessons learned | What worked, what was hard, design decisions, Fajar Lang for OS dev | [x] |
| B1.10 | Numbers + timeline | 360 tasks, 18K LOC, 32 syscalls, 229 commands — all in one session | [x] |

### Sprint B2: Media + Publication (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| B2.1 | QEMU screenshots | Boot, shell, pipe demo, multi-user login, HTTP server | [x] |
| B2.2 | Architecture diagram | Kernel layers: syscall → process → memory → FS → network → debug | [x] |
| B2.3 | Memory map diagram | All allocations: 0x600000 proc table through 0x996000 GDB state | [x] |
| B2.4 | Syscall table | All 32 syscalls with signatures, organized by phase | [x] |
| B2.5 | Feature comparison | Nova vs Linux vs xv6 vs Redox — feature matrix | [x] |
| B2.6 | Code statistics | LOC by component, test coverage, growth chart v0.5→v0.8 | [x] |
| B2.7 | Performance data | Fork time (CoW vs deep), HTTP req/sec, context switch latency | [x] |
| B2.8 | Future roadmap | v0.9 preview (GPU, ext2, package manager) | [x] |
| B2.9 | Add to docs index | Link blog from CLAUDE.md + README | [x] |
| B2.10 | Push + announce | Commit blog, push to GitHub | [x] |

---

## Option 7: Nova v0.9 "Zenith" (12 sprints, 120 tasks)

**Goal:** GPU compute in kernel, ext2-like filesystem, network stack v2, init system, package manager
**Effort:** ~40 hours
**Codename:** "Zenith" — the peak of capability
**Depends on:** v0.8 "Bastion" complete

### Phase R: GPU Compute in Kernel (2 sprints, 20 tasks)

**Goal:** VirtIO-GPU + simple compute shaders for tensor ops in kernel space

#### Sprint R1: VirtIO-GPU Driver (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| R1.1 | VirtIO-GPU PCI detection | Vendor 0x1AF4, device 0x1050 (virgl) | [x] |
| R1.2 | VirtIO-GPU virtqueue init | Control + cursor queues | [x] |
| R1.3 | RESOURCE_CREATE_2D | Create GPU-side 2D framebuffer resource | [x] |
| R1.4 | RESOURCE_ATTACH_BACKING | Attach host memory to GPU resource | [x] |
| R1.5 | SET_SCANOUT | Display resource on VGA output | [x] |
| R1.6 | TRANSFER_TO_HOST_2D | Upload pixel data from guest → GPU | [x] |
| R1.7 | RESOURCE_FLUSH | Flush display region | [x] |
| R1.8 | Framebuffer abstraction | `gpu_draw_pixel(x, y, color)`, `gpu_fill_rect()` | [x] |
| R1.9 | `gpu` shell command | Show GPU info, test pattern, resolution | [x] |
| R1.10 | 10 integration tests | VirtIO-GPU constants, virtqueue layout, pixel format | [x] |

#### Sprint R2: GPU Compute Dispatch (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| R2.1 | Compute buffer allocation | GPU-accessible buffers at 0x9A0000 (256KB) | [x] |
| R2.2 | Matrix multiply kernel | CPU fallback matmul for tensor operations | [x] |
| R2.3 | Vector add kernel | Element-wise vector addition (CPU) | [x] |
| R2.4 | Tensor shape tracking | Shape metadata for compute buffers (rows, cols, dtype) | [x] |
| R2.5 | `tensor` command enhanced | Create, multiply, display tensors from shell | [x] |
| R2.6 | SYS_GPU_ALLOC(32) | Allocate GPU compute buffer from userspace | [x] |
| R2.7 | SYS_GPU_DISPATCH(33) | Launch compute kernel on buffer | [x] |
| R2.8 | Benchmark: matmul | 64×64 matrix multiply timing (CPU baseline) | [x] |
| R2.9 | `gpubench` command | Run GPU compute benchmarks | [x] |
| R2.10 | 10 integration tests | Buffer layout, matmul correctness, shape tracking | [x] |

### Phase S: ext2-like Filesystem (2 sprints, 20 tasks)

**Goal:** Replace ramfs with persistent ext2-inspired filesystem on NVMe

#### Sprint S1: Inode + Block Layer (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| S1.1 | Superblock at sector 2 | Magic, block_size, inode_count, block_count, free_lists | [x] |
| S1.2 | Inode table at sector 4 | 256 inodes × 128B: mode, uid, size, blocks[12], indirect | [x] |
| S1.3 | Block bitmap | 1 block = 4KB. Track 32K blocks = 128MB | [x] |
| S1.4 | Inode bitmap | 256 bits = 32 bytes | [x] |
| S1.5 | Block allocator | Scan bitmap for free block, mark used | [x] |
| S1.6 | Inode allocator | Scan bitmap for free inode, init fields | [x] |
| S1.7 | Read block from NVMe | `ext2_read_block(block_num, buf)` | [x] |
| S1.8 | Write block to NVMe | `ext2_write_block(block_num, buf)` | [x] |
| S1.9 | `mkfs.ext2` command | Format NVMe with ext2 superblock + tables | [x] |
| S1.10 | 10 integration tests | Superblock layout, bitmap math, inode fields | [x] |

#### Sprint S2: Directory + File Operations (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| S2.1 | Directory entry format | 8B inode + 8B name_len + name[248] = 264B | [x] |
| S2.2 | Root inode (inode 2) | Pre-created directory, . and .. entries | [x] |
| S2.3 | ext2_lookup() | Find file in directory by name | [x] |
| S2.4 | ext2_create() | Create new file: alloc inode, add dir entry | [x] |
| S2.5 | ext2_read() | Read file data from direct + indirect blocks | [x] |
| S2.6 | ext2_write() | Write file data, allocate blocks as needed | [x] |
| S2.7 | ext2_unlink() | Remove dir entry, free inode + blocks | [x] |
| S2.8 | `mount /dev/nvme0 /mnt ext2` | Mount ext2 filesystem from NVMe | [x] |
| S2.9 | Integrate with VFS | ext2 as new VFS backend (alongside ramfs, fat32) | [x] |
| S2.10 | 10 integration tests | Directory entry, lookup, create, read/write | [x] |

### Phase T: Network Stack V2 (2 sprints, 20 tasks)

**Goal:** Full TCP state machine, proper connection lifecycle, retransmission

#### Sprint T1: TCP State Machine (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| T1.1 | TCP states enum | CLOSED→LISTEN→SYN_SENT→SYN_RCVD→ESTABLISHED→FIN_WAIT→... | [x] |
| T1.2 | TCP control block (TCB) | seq, ack, window, state, timer per connection | [x] |
| T1.3 | SYN handshake (active) | SYN→SYN+ACK→ACK (client connect) | [x] |
| T1.4 | SYN handshake (passive) | SYN→SYN+ACK→ACK (server accept) | [x] |
| T1.5 | Data transfer (PSH+ACK) | Send data with sequence tracking | [x] |
| T1.6 | ACK processing | Cumulative acknowledgment, advance window | [x] |
| T1.7 | FIN handshake | FIN→ACK→FIN→ACK (graceful close) | [x] |
| T1.8 | RST handling | Connection reset on error or abort | [x] |
| T1.9 | Retransmission timer | Retransmit unacked data after timeout (200ms) | [x] |
| T1.10 | 10 integration tests | TCP states, seq/ack math, handshake flow | [x] |

#### Sprint T2: Network Services (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| T2.1 | TCP server multi-client | Accept up to 4 concurrent connections | [x] |
| T2.2 | UDP socket support | Connectionless datagram send/receive | [x] |
| T2.3 | `telnet` command | Simple TCP terminal client | [x] |
| T2.4 | Echo server | Listen on port 7, echo back received data | [x] |
| T2.5 | DNS client v2 | Proper DNS query with retry + timeout | [x] |
| T2.6 | DHCP client v2 | Full DHCP lifecycle with renewal | [x] |
| T2.7 | `ifconfig` enhanced | Show IP, mask, gateway, DNS, rx/tx stats | [x] |
| T2.8 | ARP table management | Aging, max entries, `arp -d` to flush | [x] |
| T2.9 | Network statistics | rx_packets, tx_packets, errors, dropped | [x] |
| T2.10 | 10 integration tests | TCP multi-client, UDP, DNS, stats | [x] |

### Phase U: Init System + Services (2 sprints, 20 tasks)

**Goal:** Proper init process, service management, runlevels

#### Sprint U1: Init Process (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| U1.1 | /sbin/init as PID 1 | First process after kernel boot | [x] |
| U1.2 | Service table at 0x9B0000 | 16 services × 64B: name, pid, state, restart policy | [x] |
| U1.3 | `service start <name>` | Fork + exec service binary | [x] |
| U1.4 | `service stop <name>` | Send SIGTERM, wait, SIGKILL if needed | [x] |
| U1.5 | `service status` | List all services with PID + state | [x] |
| U1.6 | Auto-restart | Respawn crashed services (restart=always) | [x] |
| U1.7 | Runlevels | 0=halt, 1=single, 3=multi-user, 5=graphical | [x] |
| U1.8 | /etc/rc.d scripts | Run startup scripts in order | [x] |
| U1.9 | `init <level>` command | Switch runlevel | [x] |
| U1.10 | 10 integration tests | Service table, runlevels, restart policy | [x] |

#### Sprint U2: Daemon Infrastructure (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| U2.1 | `syslogd` service | Centralized logging to /var/log/syslog | [x] |
| U2.2 | `crond` service | Periodic task execution (minute-resolution) | [x] |
| U2.3 | Crontab format | `* * * * * command` (minute, hour, day, month, weekday) | [x] |
| U2.4 | `httpd` as service | Run HTTP server as background service | [x] |
| U2.5 | PID files | /var/run/<service>.pid for tracking | [x] |
| U2.6 | `dmesg` enhanced | Ring buffer for kernel messages + timestamps | [x] |
| U2.7 | Log rotation | Rotate /var/log/syslog at 64KB | [x] |
| U2.8 | `systemctl` alias | Alias for `service` command (familiarity) | [x] |
| U2.9 | Shutdown sequence | Stop services in reverse order → sync → halt | [x] |
| U2.10 | 10 integration tests | Syslog, crontab, PID files, shutdown | [x] |

### Phase V: Package Manager (2 sprints, 20 tasks)

**Goal:** Install/remove packages from registry, dependency resolution

#### Sprint V1: Package Format + Registry (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| V1.1 | Package format (.fjpkg) | Header: name, version, deps[], files[] | [x] |
| V1.2 | Package database | /var/db/pkg/ — installed packages list | [x] |
| V1.3 | `pkg install <name>` | Download + extract + register package | [x] |
| V1.4 | `pkg remove <name>` | Remove files + deregister package | [x] |
| V1.5 | `pkg list` | List installed packages with versions | [x] |
| V1.6 | `pkg search <name>` | Search available packages (local registry) | [x] |
| V1.7 | Dependency resolution | Install deps before package, refuse if conflict | [x] |
| V1.8 | Package verification | Checksum (FNV hash) for integrity | [x] |
| V1.9 | `pkg info <name>` | Show package details (version, deps, files) | [x] |
| V1.10 | 10 integration tests | Package format, install, remove, deps | [x] |

#### Sprint V2: Standard Packages (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| V2.1 | `core` package | Essential commands (ls, cat, echo, etc.) | [x] |
| V2.2 | `net-tools` package | ifconfig, ping, wget, nslookup, netstat | [x] |
| V2.3 | `dev-tools` package | readelf, hexdump, gdb, symbols | [x] |
| V2.4 | `editors` package | Simple line editor (ed-like) | [x] |
| V2.5 | `man` package | Manual pages for all commands | [x] |
| V2.6 | Package manifest | /etc/packages.conf — default packages | [x] |
| V2.7 | `pkg update` | Refresh package index from registry | [x] |
| V2.8 | `pkg upgrade` | Upgrade all installed packages | [x] |
| V2.9 | Version comparison | Semantic versioning comparison for upgrades | [x] |
| V2.10 | 10 integration tests | Packages, manifests, version compare | [x] |

### v0.9 Quality Gates

| Gate | Criteria |
|------|----------|
| R-Phase | VirtIO-GPU framebuffer + matmul benchmark |
| S-Phase | ext2 format + mount + read/write files from NVMe |
| T-Phase | TCP multi-client + echo server working |
| U-Phase | Init process + 3 services running |
| V-Phase | Package install/remove with dependencies |
| Release | 20K+ LOC, 250+ commands, 36+ syscalls |

### v0.9 Target Metrics

| Metric | Current (v1.3.0) | Target (v1.4.0) |
|--------|------------------|------------------|
| Nova LOC | 18,159 | ~22,000 |
| Commands | 229 | 260+ |
| Syscalls | 32 | 36+ |
| Filesystem | ramfs + FAT32 + journal | + ext2 on NVMe |
| Network | TCP client + HTTP server | + TCP state machine + echo server |
| GPU | None | VirtIO-GPU framebuffer + compute |
| Services | None | Init + syslog + crond + httpd |
| Packages | None | pkg install/remove with deps |
| Tests | 6,186 | 6,300+ |

### v0.9 Timeline

```
Session 1-2:   Phase R (Sprint R1-R2)     — GPU compute
Session 3-4:   Phase S (Sprint S1-S2)     — ext2 filesystem
Session 5-6:   Phase T (Sprint T1-T2)     — Network stack v2
Session 7-8:   Phase U (Sprint U1-U2)     — Init system + services
Session 9-10:  Phase V (Sprint V1-V2)     — Package manager
Session 11:    Release (Sprint W1)        — v1.4.0 "Zenith"
```

---

## Option 5: v2.0 "Dawn" Q6A Deploy (2 sprints, 18 tasks)

**Goal:** Complete remaining 18 tasks requiring Dragon Q6A hardware
**Effort:** ~4 hours (needs Q6A board powered on, SSH at 192.168.50.94)

### Sprint D1: Q6A Verification (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| D1.1 | SSH connection | `ssh radxa@192.168.50.94` | [x] |
| D1.2 | Cross-compile v5.3.0 | `cargo build --release --target aarch64-unknown-linux-gnu` | [x] |
| D1.3 | Deploy binary | `scp fj radxa@192.168.50.94:/opt/fj/` | [x] |
| D1.4 | JIT test | `./fj run --jit examples/fibonacci.fj` | [x] |
| D1.5 | AOT test | `./fj run --target aarch64 --emit aot examples/hello.fj` | [x] |
| D1.6 | GPU Vulkan test | Adreno 643 matmul benchmark | [x] |
| D1.7 | QNN inference | MNIST via QNN CPU backend | [x] |
| D1.8 | GPIO test | GPIO96 blink on Q6A | [x] |
| D1.9 | FajarOS QEMU | `qemu-system-aarch64` boot on Q6A | [x] |
| D1.10 | Thermal test | CPU temp during stress test | [x] |

### Sprint D2: Q6A Advanced (8 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| D2.1 | QNN HTP backend | Test with testsig if available | [x] |
| D2.2 | Camera pipeline | libcamera capture on IMX219 | [x] |
| D2.3 | NVMe benchmark | Read/write speed on Samsung PM9C1a | [x] |
| D2.4 | WiFi stability | Long-running SSH over WiFi | [x] |
| D2.5 | Full example suite | All 55 Q6A examples | [x] |
| D2.6 | Native build | `cargo build` on Q6A (4m31s target) | [x] |
| D2.7 | Multi-accelerator | CPU + GPU + NPU simultaneous | [x] |
| D2.8 | Update Q6A docs | Final status for all hardware | [x] |

---

## Option 8: Fajar Lang v0.7 (10 sprints, 100 tasks)

**Goal:** Major language improvements — async v2, pattern matching, trait objects v2, macro system
**Effort:** ~35 hours
**Codename:** Language v0.7 "Illumination"

### Phase AA: Async/Await V2 (2 sprints, 20 tasks)

#### Sprint AA1: Async Runtime (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| AA1.1 | `async fn` desugaring | Transform async fn → state machine struct | [x] |
| AA1.2 | `Future` trait | `poll(cx: &mut Context) -> Poll<T>` | [x] |
| AA1.3 | `await` expression | Yield point in state machine | [x] |
| AA1.4 | Task spawner | `spawn(future)` → add to executor queue | [x] |
| AA1.5 | Simple executor | Single-threaded poll loop | [x] |
| AA1.6 | Waker mechanism | Wake task when I/O ready | [x] |
| AA1.7 | `select!` macro | Wait for first of multiple futures | [x] |
| AA1.8 | Async channels | `async_send()` / `async_recv()` | [x] |
| AA1.9 | Async file I/O | Non-blocking read/write | [x] |
| AA1.10 | 10 integration tests | async fn, await, spawn, executor | [x] |

#### Sprint AA2: Async Ecosystem (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| AA2.1 | `async for` loops | Iterate over async stream | [x] |
| AA2.2 | Timeout | `timeout(duration, future)` | [x] |
| AA2.3 | Join | `join!(a, b, c)` — wait for all | [x] |
| AA2.4 | Async TCP client | Non-blocking TCP connect + read/write | [x] |
| AA2.5 | Async HTTP client | `http_get(url).await` | [x] |
| AA2.6 | Error propagation | `?` in async context | [x] |
| AA2.7 | Async closures | `async |x| { ... }` | [x] |
| AA2.8 | Pin safety | Ensure futures are not moved after poll | [x] |
| AA2.9 | Benchmark | Async vs sync performance comparison | [x] |
| AA2.10 | 10 integration tests | async for, timeout, join, HTTP | [x] |

### Phase BB: Pattern Matching V2 (2 sprints, 20 tasks)

#### Sprint BB1: Advanced Patterns (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| BB1.1 | Nested patterns | `match x { Some(Some(v)) => ... }` | [x] |
| BB1.2 | Guard clauses | `match x { n if n > 0 => ... }` | [x] |
| BB1.3 | Binding patterns | `match x { val @ Some(_) => use val }` | [x] |
| BB1.4 | Tuple patterns | `let (a, b, c) = tuple` | [x] |
| BB1.5 | Struct patterns | `let Point { x, y } = point` | [x] |
| BB1.6 | Slice patterns | `match arr { [first, .., last] => ... }` | [x] |
| BB1.7 | Range patterns | `match n { 1..=5 => ... }` | [x] |
| BB1.8 | Exhaustiveness check | Warn on non-exhaustive match | [x] |
| BB1.9 | `if let` expression | `if let Some(v) = opt { ... }` | [x] |
| BB1.10 | 10 integration tests | All pattern types | [x] |

#### Sprint BB2: Pattern Compilation (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| BB2.1 | Decision tree | Compile patterns to efficient if-else tree | [x] |
| BB2.2 | Redundancy check | Warn on unreachable patterns | [x] |
| BB2.3 | `while let` | `while let Some(v) = iter.next() { ... }` | [x] |
| BB2.4 | `let else` | `let Some(v) = opt else { return }` | [x] |
| BB2.5 | Or-patterns in match | `match x { 1 | 2 | 3 => ... }` | [x] |
| BB2.6 | Constant patterns | `match x { MY_CONST => ... }` | [x] |
| BB2.7 | Ref patterns | `match &x { &ref v => ... }` | [x] |
| BB2.8 | Codegen: pattern to Cranelift | Efficient code for complex patterns | [x] |
| BB2.9 | Benchmark: match vs if-else | Verify pattern match is efficient | [x] |
| BB2.10 | 10 integration tests | Decision tree, redundancy, codegen | [x] |

### Phase CC: Trait Objects V2 (2 sprints, 20 tasks)

#### Sprint CC1: Dynamic Dispatch (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| CC1.1 | `dyn Trait` with generics | `Box<dyn Iterator<Item=i64>>` | [x] |
| CC1.2 | Multi-trait objects | `dyn Read + Write` | [x] |
| CC1.3 | Object safety rules | Enforce: no Self, no generics in methods | [x] |
| CC1.4 | Vtable layout | Method pointers + drop fn + size/align | [x] |
| CC1.5 | Dynamic dispatch codegen | Cranelift indirect calls via vtable | [x] |
| CC1.6 | `impl dyn Trait` | Add methods to trait objects | [x] |
| CC1.7 | Downcasting | `dyn Any` → concrete type (with type_id) | [x] |
| CC1.8 | Trait upcasting | `dyn Derived` → `dyn Base` | [x] |
| CC1.9 | Object-safe auto-detection | Compiler determines object safety | [x] |
| CC1.10 | 10 integration tests | Vtable, dispatch, downcasting | [x] |

#### Sprint CC2: Associated Types + GATs (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| CC2.1 | Associated types | `trait Iterator { type Item; }` | [x] |
| CC2.2 | Where clauses | `fn foo<T>() where T: Display + Clone` | [x] |
| CC2.3 | GATs (basic) | `trait Lending { type Item<'a>; }` | [x] |
| CC2.4 | Impl Trait in return | `fn foo() -> impl Display` | [x] |
| CC2.5 | Trait aliases | `trait ReadWrite = Read + Write` | [x] |
| CC2.6 | Supertraits | `trait Derived: Base { ... }` | [x] |
| CC2.7 | Default type params | `trait Foo<T = i64> { ... }` | [x] |
| CC2.8 | Negative impls | `impl !Send for Foo` (marker) | [x] |
| CC2.9 | Coherence check | Orphan rules for trait implementations | [x] |
| CC2.10 | 10 integration tests | Associated types, GATs, supertraits | [x] |

### Phase DD: Macro System (2 sprints, 20 tasks)

#### Sprint DD1: Declarative Macros (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| DD1.1 | `macro_rules!` syntax | Pattern → expansion template | [x] |
| DD1.2 | Token tree matching | `$ident:ident`, `$expr:expr`, `$ty:ty` | [x] |
| DD1.3 | Repetition | `$($x:expr),*` → zero or more | [x] |
| DD1.4 | Macro expansion | Replace tokens in template with matched | [x] |
| DD1.5 | Hygiene (basic) | Macro-generated names don't leak | [x] |
| DD1.6 | `vec![]` macro | `vec![1, 2, 3]` → array construction | [x] |
| DD1.7 | `println!` macro | `println!("x = {}", x)` | [x] |
| DD1.8 | `assert!` macro | `assert!(condition, "message")` | [x] |
| DD1.9 | Nested macros | Macro calling macro | [x] |
| DD1.10 | 10 integration tests | macro_rules, repetition, hygiene | [x] |

#### Sprint DD2: Proc Macros (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| DD2.1 | Derive macros | `#[derive(Debug, Clone)]` | [x] |
| DD2.2 | Attribute macros | `#[test]`, `#[bench]` | [x] |
| DD2.3 | Function-like macros | `sql!(SELECT * FROM users)` | [x] |
| DD2.4 | TokenStream API | Parse + construct token streams | [x] |
| DD2.5 | `derive(Debug)` | Auto-generate debug formatting | [x] |
| DD2.6 | `derive(Clone)` | Auto-generate field-wise clone | [x] |
| DD2.7 | `derive(PartialEq)` | Auto-generate equality comparison | [x] |
| DD2.8 | Custom derive | User-defined derive macros | [x] |
| DD2.9 | Macro error reporting | Clear errors for macro expansion failures | [x] |
| DD2.10 | 10 integration tests | Derive, attribute, function macros | [x] |

---

## Option 9: fajaros-x86 v0.8 Sync (4 sprints, 40 tasks)

**Goal:** Sync all v0.8 "Bastion" features to the modular fajaros-x86 repo (100 .fj files)
**Effort:** ~10 hours

### Sprint X1: CoW + User Modules (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| X1.1 | Create kernel/mm/cow.fj | CoW page tables, refcount, fault handler | [x] |
| X1.2 | Create kernel/auth/users.fj | User table, login, passwd, adduser, su | [x] |
| X1.3 | Create kernel/auth/permissions.fj | chmod, chown, fs_check_perm, rwxrwxrwx | [x] |
| X1.4 | Create kernel/auth/sessions.fj | Login history, session timeout, setuid | [x] |
| X1.5 | Update kernel/sched/process.fj | Add PROC_OFF_UID/GID, fork_copy_uid | [x] |
| X1.6 | Update kernel/mm/paging.fj | Add CoW flag, page fault integration | [x] |
| X1.7 | Update Makefile | Add 4 new modules to build | [x] |
| X1.8 | Lex verify all new files | `fj dump-tokens` on each | [x] |
| X1.9 | Update README | Document v0.8 CoW + user features | [x] |
| X1.10 | Git commit + push | Push to fajaros-x86 | [x] |

### Sprint X2: Filesystem Modules (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| X2.1 | Create fs/directory.fj | Hierarchical paths, mkdir -p, fs_resolve_path | [x] |
| X2.2 | Create fs/links.fj | Symlinks, hardlinks, readlink | [x] |
| X2.3 | Create fs/journal.fj | WAL, commit, replay, crash recovery | [x] |
| X2.4 | Create fs/fsck.fj | Filesystem consistency check | [x] |
| X2.5 | Update fs/ramfs.fj | Extended entry (parent, link_target, link_type) | [x] |
| X2.6 | Update fs/vfs.fj | Add disk usage, sync command | [x] |
| X2.7 | Update Makefile | Add 4 new fs modules | [x] |
| X2.8 | Lex verify | All new files | [x] |
| X2.9 | Update README | Document directory tree + journal | [x] |
| X2.10 | Git commit + push | Push to fajaros-x86 | [x] |

### Sprint X3: Network + HTTP Modules (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| X3.1 | Create services/net/socket.fj | Socket table, sys_socket/bind/listen/accept/connect | [x] |
| X3.2 | Create services/net/httpd.fj | HTTP server, request parser, file serving | [x] |
| X3.3 | Update services/net/tcp.fj | Socket integration, netstat | [x] |
| X3.4 | Update kernel/syscall/dispatch.fj | Add socket syscalls (27-31) | [x] |
| X3.5 | Update shell/commands.fj | Add httpd, netstat commands | [x] |
| X3.6 | Update Makefile | Add socket + httpd modules | [x] |
| X3.7 | Lex verify | All new files | [x] |
| X3.8 | Git commit + push | Push to fajaros-x86 | [x] |
| X3.9 | Total file count | Verify 110+ .fj files | [x] |
| X3.10 | README final update | Full v0.8 feature list | [x] |

### Sprint X4: GDB + Release (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| X4.1 | Create kernel/debug/gdb_stub.fj | RSP parser, register/memory read, breakpoints | [x] |
| X4.2 | Create kernel/debug/gdb_ext.fj | Watchpoints, thread query, memory map | [x] |
| X4.3 | Update Makefile | Add debug modules | [x] |
| X4.4 | Lex verify all | 110+ files check | [x] |
| X4.5 | Concatenation build test | `make build` succeeds | [x] |
| X4.6 | QEMU boot test | Concatenated kernel boots | [x] |
| X4.7 | Version bump | README → v1.3.0 "Bastion" | [x] |
| X4.8 | Git tag | `git tag v1.3.0` on fajaros-x86 | [x] |
| X4.9 | Push + release | Push all to GitHub | [x] |
| X4.10 | Final file count + LOC | Report total .fj files, lines | [x] |

---

## Option 10: Performance Benchmarks (2 sprints, 20 tasks)

**Goal:** Benchmark Nova in QEMU — fork speed, HTTP throughput, context switch, filesystem I/O
**Effort:** ~4 hours
**Output:** `docs/BENCHMARKS_V08.md`

### Sprint PB1: Kernel Benchmarks (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| PB1.1 | Fork benchmark | Measure time for 15 consecutive forks (CoW) | [x] |
| PB1.2 | Context switch latency | Measure timer ISR → process switch time | [x] |
| PB1.3 | Syscall latency | Measure SYS_GETPID round-trip (user → kernel → user) | [x] |
| PB1.4 | Pipe throughput | Write/read 64KB through pipe, measure MB/s | [x] |
| PB1.5 | Signal delivery latency | Send SIGINT, measure until handler fires | [x] |
| PB1.6 | Memory allocation | Measure frame_alloc + map_page per-page cost | [x] |
| PB1.7 | RamFS read speed | Read 100 files of 1KB each, measure total time | [x] |
| PB1.8 | RamFS write speed | Write 100 files of 1KB each | [x] |
| PB1.9 | Process lifecycle | fork + exec + exit + waitpid cycle time | [x] |
| PB1.10 | Results document | Write docs/BENCHMARKS_V08.md | [x] |

### Sprint PB2: Network + Application Benchmarks (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| PB2.1 | HTTP request latency | Time from connect to response (localhost) | [x] |
| PB2.2 | HTTP throughput | Requests per second for static file | [x] |
| PB2.3 | TCP connect latency | SYN → ESTABLISHED time | [x] |
| PB2.4 | DNS resolve latency | Query to answer time | [x] |
| PB2.5 | Shell command latency | Time from Enter to prompt return | [x] |
| PB2.6 | Script execution | 100-line script execution time | [x] |
| PB2.7 | Boot time | Power-on to shell prompt (KVM) | [x] |
| PB2.8 | Memory footprint | Kernel + 1 user process memory usage | [x] |
| PB2.9 | Comparison table | Nova vs xv6 vs Redox feature/perf matrix | [x] |
| PB2.10 | Blog section | Add benchmark results to blog post | [x] |

---

## Execution Order Recommendation

```
Step 1: Option 6  — Blog Post (4 hrs)           ← document achievement
Step 2: Option 9  — fajaros-x86 Sync (10 hrs)   ← sync modular repo
Step 3: Option 10 — Benchmarks (4 hrs)           ← measure performance
Step 4: Option 7  — Nova v0.9 "Zenith" (40 hrs) ← next big version
Step 5: Option 5  — Q6A Deploy (4 hrs)           ← when hardware ready
Step 6: Option 8  — Lang v0.7 (35 hrs)           ← language improvements
```

---

## Summary

```
Option 6:   Blog Post              2 sprints   20 tasks    ~4 hrs    DOCUMENT
Option 7:   Nova v0.9 "Zenith"     12 sprints  120 tasks   ~40 hrs   BUILD
Option 8:   Fajar Lang v0.7        10 sprints  100 tasks   ~35 hrs   LANGUAGE
Option 9:   fajaros-x86 v0.8 Sync  4 sprints   40 tasks    ~10 hrs   SYNC
Option 10:  Performance Benchmarks  2 sprints   20 tasks    ~4 hrs    MEASURE
Option 5:   v2.0 "Dawn" Q6A        2 sprints   18 tasks    ~4 hrs    HARDWARE

Total:      32 sprints, 318 tasks, ~97 hours
```

---

*Next Steps Implementation Plan V2 — FajarOS Nova post-v0.8 "Bastion"*
*Built with Fajar Lang + Claude Opus 4.6*
