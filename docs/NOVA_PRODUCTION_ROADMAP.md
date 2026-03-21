# FajarOS Nova — Production Readiness Roadmap

> **Date:** 2026-03-21
> **Goal:** Make FajarOS Nova a 100% usable operating system
> **Current:** v0.6.0 "Ascension" — 10,760 LOC, 155 commands, QEMU verified
> **Target:** v1.0.0 "Genesis" — self-hosting, multi-process, persistent, networked

---

## Gap Analysis: What's Done vs What's Missing

### DONE (Working in QEMU)

| Feature | Status | Quality |
|---------|--------|---------|
| Boot (Multiboot2 + GRUB2) | Working | Production |
| Serial console (COM1) | Working | Production |
| VGA text mode (80×25) | Working | Production |
| PS/2 keyboard (scancode → ASCII) | Working | Production |
| GDT + IDT + PIC + PIT (100Hz) | Working | Production |
| 4-level paging (PML4) | Working | Production |
| Bitmap frame allocator (128MB) | Working | Production |
| Freelist heap (kmalloc/kfree) | Working | Production |
| Slab allocator | Working | Production |
| NVMe block device (R/W sectors) | Working | Production |
| FAT32 filesystem (R/W files) | Working | Production |
| VFS (/, /dev, /proc, /mnt) | Working | Production |
| RamFS (64 entries) | Working | Production |
| IPC message queue (4 msgs/PID) | Working | Production |
| Process table v2 (16 PIDs) | Working | Partial |
| SYSCALL/SYSRET (Ring 0↔3) | Working | Partial |
| Ring 3 user programs (3 installed) | Working | Demo only |
| USB XHCI (enumerate + SCSI) | Working | Partial |
| USB FAT32 mount | Working | Partial |
| Virtio-net (virtqueue TX/RX) | Working | Untested real ping |
| SMP (AP trampoline) | Working | Boot only |
| ELF64 parser | Working | Partial |
| 155 shell commands | Working | Production |
| NX bit, stack guard page | Working | Production |

### NOT DONE (Critical Gaps)

| # | Gap | Impact | Effort |
|---|-----|--------|--------|
| 1 | **SYS_EXIT halts CPU** instead of returning to shell | User programs can't return | 2 hrs |
| 2 | **No preemptive scheduling** — timer ticks but doesn't switch processes | Single-process only | 8 hrs |
| 3 | **No per-process page tables** — all processes share identity map | No memory isolation | 6 hrs |
| 4 | **SYS_READ not implemented** — user programs can't read keyboard | No interactive programs | 3 hrs |
| 5 | **No ELF loading from filesystem** — programs are hardcoded bytes | Can't load external binaries | 4 hrs |
| 6 | **No real ping reply** — virtio-net TX works but RX untested | Network partially fake | 2 hrs |
| 7 | **Spawn/wait/kill simulated** — commands print text but don't work | Process lifecycle fake | 4 hrs |
| 8 | **No pipe data flow** — pipe infrastructure exists but no real data | IPC pipes are stubs | 3 hrs |
| 9 | **SCSI READ(10) untested** — CBW/CSW built but bulk transfer not verified | USB read might not work | 3 hrs |
| 10 | **No DHCP** — IP is hardcoded 10.0.2.15 | Can't join real networks | 4 hrs |
| 11 | **No TCP** — only ICMP/ARP/IPv4 | Can't do HTTP/SSH | 8 hrs |
| 12 | **No real hardware test** — only QEMU | Unknown on real iron | 4 hrs |
| 13 | **No CI/CD** — manual build + test | Regressions possible | 2 hrs |

---

## Roadmap: 7 Milestones to Production

### Milestone 1: "Return to Shell" (v0.7) — ~8 hrs
**Goal:** User programs return cleanly to kernel shell after exit

| # | Task | Detail | Effort |
|---|------|--------|--------|
| 1.1 | Fix SYS_EXIT handler | Instead of HLT: restore kernel RSP, return to shell loop | 2 hrs |
| 1.2 | Save kernel state before IRETQ | Store RSP/RIP at known address so SYS_EXIT can restore | 1 hr |
| 1.3 | `run` command returns | After user program exits, shell prompt reappears | 1 hr |
| 1.4 | SYS_READ (keyboard) | User program blocks on keyboard input via SYSCALL | 2 hrs |
| 1.5 | Interactive user program | User program that reads a line, echoes it, then exits | 1 hr |
| 1.6 | Test: run hello → run goodbye → run interactive | Sequential program execution verified | 1 hr |

**Quality Gate:** `run0` → prints "Hello Ring 3!" → returns to `nova>` prompt

---

### Milestone 2: "Multitasking" (v0.8) — ~12 hrs
**Goal:** Multiple processes run concurrently with timer-driven preemption

| # | Task | Detail | Effort |
|---|------|--------|--------|
| 2.1 | Context frame: 256 bytes per process | RAX-R15, RIP, RSP, RFLAGS, CR3, FPU state | 1 hr |
| 2.2 | save_context / restore_context | Store/load all GPRs to process table | 2 hrs |
| 2.3 | Timer ISR → scheduler | PIT IRQ: save current → pick_next → restore → IRET | 3 hrs |
| 2.4 | Round-robin scheduler | Cycle through READY processes, 10ms quantum | 1 hr |
| 2.5 | `spawn` command (real) | Create process, set entry point, add to ready queue | 2 hrs |
| 2.6 | `kill` command (real) | Set process state to ZOMBIE, free resources | 1 hr |
| 2.7 | `wait` command (real) | Block until child exits, return exit code | 1 hr |
| 2.8 | Test: 2 processes printing alternately | Timer switches between them, both produce output | 1 hr |

**Quality Gate:** `spawn hello` + `spawn goodbye` → both print interleaved via preemption

---

### Milestone 3: "Memory Protection" (v0.9) — ~8 hrs
**Goal:** Each process has its own page table, can't access other processes

| # | Task | Detail | Effort |
|---|------|--------|--------|
| 3.1 | Per-process PML4 | Clone kernel page tables, add user pages per process | 3 hrs |
| 3.2 | CR3 switch on context switch | Load process PML4 during restore_context | 1 hr |
| 3.3 | User page allocation | Map user code + stack + heap pages per process | 2 hrs |
| 3.4 | Page fault handler | Catch invalid access → kill process, don't kernel panic | 1 hr |
| 3.5 | Test: process can't read kernel memory | Access 0x100000 from Ring 3 → page fault → killed | 1 hr |

**Quality Gate:** Process A can't read Process B's memory

---

### Milestone 4: "ELF from Disk" (v1.0-alpha) — ~6 hrs
**Goal:** Load and run ELF binaries from FAT32 filesystem

| # | Task | Detail | Effort |
|---|------|--------|--------|
| 4.1 | `exec <file>` command | Read ELF from FAT32, parse headers, load segments | 2 hrs |
| 4.2 | Relocatable loading | Map PT_LOAD segments to user VA, set entry point | 2 hrs |
| 4.3 | Dynamic stack allocation | Allocate user stack pages, set RSP | 1 hr |
| 4.4 | Cross-compile user programs | `fj build --target x86_64-user hello.fj -o hello.elf` | 1 hr |
| 4.5 | Test: write hello.elf to USB → exec hello | Full pipeline from source to execution | 1 hr (bonus) |

**Quality Gate:** `exec /mnt/hello.elf` → runs from FAT32 → returns to shell

---

### Milestone 5: "Real Network" (v1.0-beta) — ~10 hrs
**Goal:** DHCP, real ping, TCP connect, basic HTTP

| # | Task | Detail | Effort |
|---|------|--------|--------|
| 5.1 | Virtio-net RX interrupt | Handle received packets via IRQ instead of polling | 2 hrs |
| 5.2 | DHCP client | Discover → Offer → Request → Ack → get real IP | 3 hrs |
| 5.3 | Real ping verified | `ping 10.0.2.2` → ICMP echo reply received | 1 hr |
| 5.4 | TCP SYN/ACK handshake | 3-way handshake to establish TCP connection | 2 hrs |
| 5.5 | TCP data send/recv | Send HTTP GET request, receive response | 2 hrs |

**Quality Gate:** `ping 10.0.2.2` → real reply with RTT

---

### Milestone 6: "Self-Sustaining" (v1.0-rc) — ~8 hrs
**Goal:** OS can build + run programs without host, persistent storage

| # | Task | Detail | Effort |
|---|------|--------|--------|
| 6.1 | FAT32 write-back to NVMe | `sync` actually flushes dirty sectors | 2 hrs |
| 6.2 | Persistence test | Write file → reboot → file still exists | 1 hr |
| 6.3 | Init process (PID 1) | Auto-start shell after kernel init, respawn on exit | 2 hrs |
| 6.4 | Clean shutdown | `shutdown` → sync filesystems → ACPI power off | 1 hr |
| 6.5 | Signal handling | SIGTERM → process cleanup, SIGKILL → immediate kill | 2 hrs |

**Quality Gate:** Write file → reboot → read same file

---

### Milestone 7: "Production Release" (v1.0.0) — ~6 hrs
**Goal:** Polish, documentation, CI/CD, real hardware test

| # | Task | Detail | Effort |
|---|------|--------|--------|
| 7.1 | CI pipeline (GitHub Actions) | Auto-build kernel on push, run QEMU smoke test | 2 hrs |
| 7.2 | User manual | Boot guide, command reference, programming guide | 2 hrs |
| 7.3 | Real hardware boot | Create bootable USB, test on Intel i9-14900HX | 1 hr |
| 7.4 | Performance benchmarks | fib(30), matmul, file I/O, boot time | 0.5 hr |
| 7.5 | Release v1.0.0 | CHANGELOG, git tag, GitHub release, blog post | 0.5 hr |

**Quality Gate:** Boots on real hardware, all 155+ commands work, user programs execute

---

## Summary

```
Current:     v0.6.0 "Ascension"     10,760 LOC    155 commands
                                     │
Milestone 1: v0.7 "Return"          ~11,500 LOC   SYS_EXIT → shell        ~8 hrs
                                     │
Milestone 2: v0.8 "Multitask"       ~13,000 LOC   Preemptive scheduler     ~12 hrs
                                     │
Milestone 3: v0.9 "Protect"         ~14,000 LOC   Per-process page tables  ~8 hrs
                                     │
Milestone 4: v1.0α "Load"           ~15,000 LOC   ELF from FAT32           ~6 hrs
                                     │
Milestone 5: v1.0β "Connect"        ~16,500 LOC   DHCP + TCP + HTTP        ~10 hrs
                                     │
Milestone 6: v1.0rc "Sustain"       ~17,500 LOC   Persistence + init       ~8 hrs
                                     │
Milestone 7: v1.0.0 "Genesis"       ~18,000 LOC   Real hardware + CI       ~6 hrs
```

**Total estimated: ~58 hours of development (7 milestones, ~50 tasks)**

## Priority Order (Critical Path)

```
[MUST]  Milestone 1 → 2 → 3 → 4    (Process lifecycle: exit → schedule → protect → load)
[HIGH]  Milestone 5                   (Networking: DHCP → ping → TCP)
[MED]   Milestone 6                   (Persistence: write-back → init → shutdown)
[LOW]   Milestone 7                   (Polish: CI → docs → real HW → release)
```

The critical path is Milestones 1→2→3→4: once user programs can exit cleanly, run concurrently, have memory protection, and load from disk, the OS is fundamentally "usable". Milestones 5-7 are important but not blockers.

---

*FajarOS Nova: from 10K LOC demo to 18K LOC production OS*
*Estimated: 7 milestones, ~58 hours, ~50 tasks*
*Built with Fajar Lang + Claude Opus 4.6*
