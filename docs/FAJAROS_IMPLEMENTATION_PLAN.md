# FajarOS Implementation Plan — Production & Commercial Ready

> **Date:** 2026-03-28
> **Author:** Claude Opus 4.6
> **Scope:** FajarOS Nova (x86_64) + FajarOS Surya (ARM64)
> **Goal:** Native-compiled OS kernel, QEMU verified, hardware tested

---

## Current State

| Component | Status |
|-----------|--------|
| Kernel source | 21,187 lines, 819 @kernel functions |
| Type check | **0 errors** (`fj check` clean) |
| Pre-compiled binary | 331 KB (working in QEMU) |
| Native compile (minimal) | **WORKS** (`fj build --target x86_64-none --no-std`) |
| Native compile (full kernel) | **FAILS** — duplicate `tcp_connect` function signatures |
| Extension files (Phoenix/Aurora) | 13 files, 17 fail `fj check` (unregistered builtins) |
| QEMU test scripts | 4 scripts, verified |
| CI pipeline | `nova.yml` — check + boot test |
| Hardware testing | Not yet (needs physical access) |

---

## Phase A: Kernel Native Compilation (15 tasks)

**Goal:** `fj build --target x86_64-none --no-std kernel.fj` produces a bootable binary.

### A1: Fix Kernel for Native Compilation

| Task | Description | File | Verify |
|------|-------------|------|--------|
| A1.1 | Remove duplicate `tcp_connect` — keep 4-param version, update 2-param callers | `kernel.fj` | Compiles |
| A1.2 | Scan for all duplicate function names in kernel | `kernel.fj` | 0 duplicates |
| A1.3 | Fix any remaining codegen errors (run `fj build` iteratively) | `kernel.fj` | Clean build |
| A1.4 | Verify native binary size < 500 KB | `fj build --release` | Size OK |
| A1.5 | Verify native binary boots in QEMU | QEMU | Serial output |

### A2: Bare-Metal Runtime Functions

| Task | Description | File | Verify |
|------|-------------|------|--------|
| A2.1 | Audit all `runtime_bare.rs` functions used by kernel | `runtime_bare.rs` | All present |
| A2.2 | Verify VGA output from native binary | QEMU + VGA | Text visible |
| A2.3 | Verify serial output from native binary | QEMU + serial | "FajarOS" banner |
| A2.4 | Verify interrupt handling (IDT setup + timer) | QEMU | Timer ticks |
| A2.5 | Verify memory management (alloc/free) | QEMU | No crashes |

### A3: Linker Script & Boot

| Task | Description | File | Verify |
|------|-------------|------|--------|
| A3.1 | Generate correct linker script for x86_64 bare metal | `linker.rs` | Script correct |
| A3.2 | Add Multiboot2 header to compiled binary | linker script | GRUB loads it |
| A3.3 | Create `make native` target in Makefile | `examples/nova/Makefile` | Builds native |
| A3.4 | Boot native kernel in QEMU (end-to-end) | QEMU | Shell prompt |
| A3.5 | Document native build process | `examples/nova/README.md` | Instructions |

---

## Phase B: Extension Files Cleanup (10 tasks)

**Goal:** All 13 Phoenix/Aurora .fj files pass `fj check` with 0 errors.

| Task | Description | File | Verify |
|------|-------------|------|--------|
| B1 | Register missing builtins used by Phoenix GUI | `type_check/register.rs` | 0 errors |
| B2 | Register missing builtins used by Phoenix POSIX | `type_check/register.rs` | 0 errors |
| B3 | Register missing builtins used by Phoenix net | `type_check/register.rs` | 0 errors |
| B4 | Register missing builtins used by Phoenix persist | `type_check/register.rs` | 0 errors |
| B5 | Register missing builtins used by Phoenix audio | `type_check/register.rs` | 0 errors |
| B6 | Register missing builtins used by Aurora services | `type_check/register.rs` | 0 errors |
| B7 | Register missing builtins used by Aurora compositor | `type_check/register.rs` | 0 errors |
| B8 | Register missing builtins used by Aurora SMP | `type_check/register.rs` | 0 errors |
| B9 | Register missing builtins used by Aurora USB | `type_check/register.rs` | 0 errors |
| B10 | Verify all 173 example .fj files pass `fj check` | all examples | 173/173 pass |

---

## Phase C: Kernel Modularization (10 tasks)

**Goal:** Split 21K-line monolithic kernel into manageable modules.

| Task | Description | Output | Verify |
|------|-------------|--------|--------|
| C1 | Extract memory subsystem (frame allocator, heap, paging) | `nova/memory.fj` | Compiles |
| C2 | Extract process management (fork, exec, scheduler) | `nova/process.fj` | Compiles |
| C3 | Extract filesystem (VFS, ramfs, FAT32) | `nova/filesystem.fj` | Compiles |
| C4 | Extract network stack (TCP, UDP, ARP, HTTP) | `nova/network.fj` | Compiles |
| C5 | Extract device drivers (NVMe, USB, virtio, serial) | `nova/drivers.fj` | Compiles |
| C6 | Extract shell (commands, pipes, scripting) | `nova/shell.fj` | Compiles |
| C7 | Extract security (users, permissions, login) | `nova/security.fj` | Compiles |
| C8 | Extract debug (GDB stub, breakpoints) | `nova/debug.fj` | Compiles |
| C9 | Create `nova/kernel.fj` main that imports all modules | `nova/kernel.fj` | Compiles |
| C10 | Verify modular kernel produces identical binary | comparison | Binary matches |

---

## Phase D: QEMU Automated Testing (10 tasks)

**Goal:** Every kernel subsystem tested automatically in CI.

| Task | Description | Verify |
|------|-------------|--------|
| D1 | Boot test — kernel reaches shell prompt in < 5 seconds | CI green |
| D2 | Command test — `help`, `uname`, `ps`, `ls` produce correct output | CI green |
| D3 | File test — `touch`, `cat`, `rm`, `mkdir` work | CI green |
| D4 | Process test — fork, exec, waitpid, signals | CI green |
| D5 | Network test — ARP, ICMP ping, TCP connect | CI green |
| D6 | NVMe test — read/write sectors | CI green |
| D7 | SMP test — 4 cores initialize | CI green |
| D8 | User test — login, passwd, chmod | CI green |
| D9 | Shell test — pipes, redirects, scripts | CI green |
| D10 | Stress test — 15 forks, heavy I/O, no crash for 60 seconds | CI green |

---

## Phase E: FajarOS Surya ARM64 (15 tasks)

**Goal:** ARM64 kernel for Radxa Dragon Q6A hardware.

| Task | Description | Verify |
|------|-------------|--------|
| E1 | Cross-compile: `fj build --target aarch64-unknown-none` | Binary produced |
| E2 | ARM64 linker script (DRAM at 0x40000000) | Script correct |
| E3 | ARM64 boot stub (EL2 → EL1 transition) | QEMU boots |
| E4 | ARM64 UART driver (PL011) | Serial output |
| E5 | ARM64 MMU setup (4KB pages, TTBR0/TTBR1) | Memory works |
| E6 | ARM64 exception handling (EL1 vectors) | Interrupts work |
| E7 | ARM64 timer (CNTPCT_EL0 + CNTP_CTL_EL0) | Timer ticks |
| E8 | ARM64 process management (context switch) | Processes run |
| E9 | Q6A board support (QCS6490 specifics) | Q6A boots |
| E10 | Q6A GPIO driver | LED blinks |
| E11 | Q6A QNN NPU inference | Model runs |
| E12 | Q6A Vulkan compute | Shader executes |
| E13 | QEMU ARM64 CI job | CI green |
| E14 | Q6A hardware test documentation | Documented |
| E15 | FajarOS Surya v1.0 release | Tagged |

---

## Phase F: Production Hardening (10 tasks)

**Goal:** Kernel reliability and security for real-world deployment.

| Task | Description | Verify |
|------|-------------|--------|
| F1 | Stack guard pages — detect stack overflow per process | No silent corruption |
| F2 | ASLR — randomize user-mode stack/heap base | Addresses vary |
| F3 | W^X — pages cannot be writable AND executable | Enforced |
| F4 | Kernel heap hardening — canaries on alloc blocks | Corruption detected |
| F5 | NX bit on data pages | Code injection blocked |
| F6 | Syscall argument validation — bounds check all pointers | No kernel panics |
| F7 | OOM killer — terminate largest process on out-of-memory | System survives |
| F8 | Watchdog timer — reboot if kernel hangs > 30 seconds | Auto-recovery |
| F9 | Panic handler — print register dump + stack trace | Debug info shown |
| F10 | Kernel test suite in .fj (using @test framework) | Tests pass |

---

## Summary

| Phase | Tasks | Priority | Dependency |
|-------|-------|----------|------------|
| **A: Native Compilation** | 15 | **HIGHEST** | None |
| **B: Extension Cleanup** | 10 | HIGH | None |
| **C: Modularization** | 10 | MEDIUM | A |
| **D: QEMU Testing** | 10 | HIGH | A |
| **E: ARM64 Surya** | 15 | MEDIUM | A |
| **F: Hardening** | 10 | LOW | A, D |
| **TOTAL** | **70** | | |

**Critical path:** Phase A (native compile) unblocks everything else.

---

*FAJAROS_IMPLEMENTATION_PLAN.md — v1.0 — 2026-03-28*
*Written by Claude Opus 4.6*
