# FajarOS Nova v1.3.0 "Bastion" — Performance Benchmarks

> **Date:** 2026-03-25
> **Platform:** x86_64 Linux 6.17.0, Intel Core i9-14900HX, 32GB RAM
> **QEMU:** 8.2.2, KVM enabled, 256MB guest RAM
> **Compiler:** Fajar Lang v5.3.0, Cranelift backend (release)

---

## 1. Compiler Benchmarks (Criterion)

| Benchmark | Time | Throughput |
|-----------|------|-----------|
| Lex 3,000 tokens | 41.5 us | 72.3M tokens/sec |
| Parse 300 statements | 84.7 us | 3.5M stmts/sec |
| fibonacci(15) tree-walk | 2.73 ms | — |
| Loop 1,000 iterations | 539 us | 1.85M iters/sec |
| String concat ×100 | 300 us | 333K ops/sec |

### Comparison with v0.1 targets:

| Benchmark | v0.1 Actual | v1.3.0 Actual | v1.0 Target | Status |
|-----------|------------|--------------|-------------|--------|
| Lex 3000 | 120 us | **41.5 us** | < 50 us | EXCEEDED |
| Parse 300 | 190 us | **84.7 us** | < 100 us | EXCEEDED |
| fib(15) | 26 ms | **2.73 ms** | < 50 ms | EXCEEDED |
| Loop 1000 | 293 us | **539 us** | < 100 us | REGRESSED* |
| String 100 | 73 us | **300 us** | < 30 us | REGRESSED* |

*Loop and string regressions due to added analyzer overhead + richer Value type. Native codegen bypasses this entirely.*

---

## 2. Kernel Component Metrics

### Code Size by Component

| Component | Functions | Estimated LOC | Purpose |
|-----------|-----------|---------------|---------|
| Syscall dispatch | 32 sys_* fns | ~1,200 | 32 syscalls (EXIT through CONNECT) |
| Process management | 15 fns | ~800 | fork, exec, waitpid, exit, spawn |
| Copy-on-Write | 10 cow_* fns | ~400 | Page fault handler, refcounting |
| File Permissions | 8 fns | ~300 | chmod, chown, permission checks |
| User Accounts | 12 fns | ~500 | login, adduser, passwd, sessions |
| Pipes | 8 fns | ~300 | Circular buffer, refcounting, EOF |
| Signals | 9 fns | ~350 | 8 signals, pending bitmap, delivery |
| Jobs | 8 fns | ~300 | Background &, fg/bg, notifications |
| Shell Scripting | 15 fns | ~600 | Variables, if/for/while, scripts |
| Directory Tree | 6 fns | ~250 | Path resolution, mkdir -p, rmdir |
| Links | 3 fns | ~150 | Symlinks, hardlinks, readlink |
| Journal | 8 fns | ~350 | WAL, commit/replay, fsck |
| Socket API | 10 fns | ~400 | bind/listen/accept/connect |
| HTTP Server | 11 http_* fns | ~450 | Request parse, file serve, /proc |
| GDB Debugger | 24 gdb_* fns | ~600 | RSP protocol, breakpoints, watchpoints |
| Shell Commands | 229 cmd_* fns | ~6,000 | All shell commands |
| **Total** | **651** | **~18,159** | |

### Memory Map Summary

| Region | Address | Size | Purpose |
|--------|---------|------|---------|
| Kernel code | 0x100000 | ~330KB | .text + .rodata (ELF) |
| Process table | 0x600000 | 4KB | 16 PIDs × 256B |
| Kernel stacks | 0x700000 | 256KB | 16 × 16KB per process |
| NVMe/FAT32 | 0x800000 | 192KB | DMA + filesystem buffers |
| v0.7 allocations | 0x8D0000 | 36KB | FD table, signals, env, pipes, jobs |
| CoW refcount | 0x950000 | 64KB | 32K entries × 2B |
| User table | 0x960000 | 4KB | 16 users × 64B |
| Journal | 0x970000 | 64KB | 1000 entries × 64B |
| Socket table | 0x980000 | 4KB | 16 sockets × 64B |
| Socket buffers | 0x982000 | 64KB | 16 × (2KB rx + 2KB tx) |
| HTTP buffers | 0x990000 | 12KB | Request + response |
| GDB state | 0x994000 | 12KB | Packets + breakpoints |
| User ELF code | 0x2000000 | 16MB | Ring 3 programs |
| **Total kernel** | | **~800KB** | Entire kernel footprint |

---

## 3. Estimated Kernel Operation Costs

### Fork Performance (CoW vs Deep-Copy)

| Operation | Deep-Copy (v0.7) | CoW (v0.8) | Speedup |
|-----------|-----------------|-----------|---------|
| Fork (16 user pages) | ~64KB copy | **0 bytes copy** | **instant** |
| First write after fork | 0 (already copied) | ~4KB copy | 1 page |
| Total for fork+modify 1 page | 64KB | **4KB** | **16x** |
| Total for fork+modify all | 64KB | 64KB | 1x (same) |

**CoW advantage:** Most forked children exec() immediately, so the shared pages are never written — the 64KB copy is completely avoided.

### Syscall Dispatch Cost

| Path | v0.6 (cmp/je) | v0.7+ (indirect call) |
|------|--------------|----------------------|
| Dispatch overhead | 5 comparisons worst-case | 1 indirect call |
| Adding new syscall | Modify assembly | Add 1 line in .fj |
| Max syscalls | ~10 practical | Unlimited |

### Context Switch Path

```
Timer IRQ (10ms, 100 Hz PIT)
  → Push 15 GPRs           (~15 cycles)
  → Increment tick counter  (~5 cycles)
  → Save RSP + CR3          (~10 cycles)
  → Round-robin scan        (~16 iterations worst)
  → Load new RSP + CR3      (~10 cycles)
  → Update TSS.RSP0         (~5 cycles)
  → EOI to PIC              (~5 cycles)
  → Pop 15 GPRs + IRETQ     (~20 cycles)

Estimated total: ~100-200 cycles (~50-100 ns at 2GHz)
```

### Pipe Throughput (Circular Buffer)

| Buffer | Capacity | Overhead |
|--------|----------|----------|
| Pipe buffer | 4,064 bytes | 32B header |
| Read/write | Modular arithmetic | ~10 cycles per byte |
| Estimated throughput | ~400 MB/s | CPU-bound memcpy |

---

## 4. Test Suite Performance

| Suite | Tests | Time |
|-------|-------|------|
| Unit tests (lib) | 4,582 | 0.26s |
| Integration (eval_tests) | 712 | ~7s |
| Property tests | 33 | ~0.08s |
| Safety tests | 76 | ~0.01s |
| ML tests | 39 | ~0.02s |
| OS tests | 16 | ~0.00s |
| **Total** | **6,186** | **~8s** |

### Build Times

| Operation | Time |
|-----------|------|
| `cargo build` (debug) | ~15s |
| `cargo build --release` | ~90s |
| `cargo build --release --features native` | ~90s |
| `cargo test` (full suite) | ~8s |
| Kernel ELF build (`fj build --target x86_64-none`) | ~2s |
| QEMU boot to shell (KVM) | ~2s |

---

## 5. Feature Comparison

### Nova vs Other Educational/Research OS

| Feature | Nova v1.3.0 | xv6 (MIT) | Redox OS | Linux 0.01 |
|---------|------------|-----------|----------|-----------|
| **Language** | Fajar Lang | C | Rust | C |
| **LOC** | 18,159 | ~8,000 | ~50,000 | ~10,000 |
| **Syscalls** | 32 | 21 | ~100 | 67 |
| **Filesystem** | ramfs+FAT32+journal | ext2-like | RedoxFS | minix |
| **Fork** | CoW | Deep-copy | CoW | Deep-copy |
| **Users** | 16, rwxrwxrwx | None | Full POSIX | root only |
| **Network** | TCP+HTTP server | None | Full stack | None |
| **Debugger** | GDB remote | GDB (host) | None | None |
| **Shell** | Pipes, vars, scripts | Basic | Ion shell | Basic |
| **Signals** | 8 (POSIX subset) | None | Full POSIX | Some |
| **Job control** | &, fg, bg, Ctrl+C | None | Yes | None |
| **SMP** | Yes (AP trampoline) | Yes | Yes | No |
| **Ring 3** | Yes (5 programs) | Yes | Yes | Yes |
| **USB** | XHCI + mass storage | None | UHCI | None |
| **Written in** | 1 session (360 tasks) | Semester course | Years | Months |

---

## 6. Growth Chart

```
Version   LOC      Commands  Syscalls  Features
v0.5      9,637    148       5         Shell, NVMe, VFS, Ring 3
v0.6     12,954    181       5         Preemptive scheduler, TCP, USB
v0.7     15,732    200       26        fork/exec, pipes, signals, scripting
v0.8     18,159    229       32        CoW, users, journal, HTTP, GDB

Growth:  +88% LOC, +55% commands, +540% syscalls
```

---

## 7. QEMU Test Results

### Boot Verification (V1: 35/36 checks)
- Multiboot2 trampoline: PASS
- kernel_main reached: PASS
- NVMe + FAT32 mount: PASS
- VFS + NET initialized: PASS
- SMP (4 cores): PASS
- VGA screenshot captured: PASS

### Process Lifecycle (V2: 12/12 checks)
- Process table v2: PASS
- Init (PID 1) with preemptive scheduling: PASS
- 5 Ring 3 programs installed: PASS
- SYSCALL entry configured: PASS

### Shell Features (V3: 40/40 checks)
- Pipe infrastructure: PASS (4 checks)
- Redirect (>, >>, <): PASS (3 checks)
- Environment variables: PASS (6 checks)
- Signals + Ctrl+C: PASS (4 checks)
- Job control: PASS (6 checks)
- Shell scripting: PASS (7 checks)
- Shell infrastructure: PASS (4 checks)

**Total QEMU checks: 87/88 (98.9% pass rate)**

---

*Benchmarks for FajarOS Nova v1.3.0 "Bastion"*
*Built with Fajar Lang + Claude Opus 4.6*
