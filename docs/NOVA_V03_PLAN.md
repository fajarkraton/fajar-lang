# FajarOS Nova v0.3 "Endurance" — Implementation Plan

> **Date:** 2026-03-20
> **Author:** Fajar (PrimeCore.id) + Claude Opus 4.6
> **Context:** Nova v0.2.0 complete (7,313 LOC, 122 cmd, NVMe+FAT32+VFS+SMP+Net+ELF)
> **Goal:** Real user-space execution, persistent file writes, hardware drivers, shell scripting
> **Codename:** "Endurance" — the OS that runs real programs

---

## Pre-requisite: Fajar Lang Enhancements

Nova v0.3 is **blocked** on 10 compiler/language fixes. These MUST be done first.

```
Tier 1 — BLOCKING (must complete before Nova v0.3 kernel work):
  E1: Parser (expr) bug fix           — clean kernel code, no workarounds
  E2: port_inb builtin                — hardware I/O port reads
  E3: port_inw / port_ind             — 16-bit and 32-bit port I/O
  E4: ltr / lgdt_mem / lidt_mem       — task register + descriptor tables
  E5: SYSCALL/SYSRET MSR setup        — wrmsr for STAR/LSTAR/SFMASK

Tier 2 — IMPORTANT (greatly improves kernel code quality):
  E6: memcmp_buf builtin              — buffer comparison for FAT32/ELF
  E7: memcpy_buf builtin              — fast buffer copy (not byte-by-byte)
  E8: Array repeat syntax [0; N]      — zero-init arrays without loops

Tier 3 — NICE-TO-HAVE (defer if time-constrained):
  E9:  Variadic print for @kernel     — kernel printf equivalent
  E10: Trait objects in @kernel        — driver dispatch tables
```

---

## Execution Plan (3 Stages, 18 Sprints, ~180 Tasks)

```
Stage A: Fajar Lang Enhancements    [░░░░░░░░░░]  3 sprints   — compiler fixes for Nova
Stage B: Nova v0.3 Core             [░░░░░░░░░░]  9 sprints   — user-space, FAT32 write, USB
Stage C: Nova v0.3 Polish           [░░░░░░░░░░]  6 sprints   — scripting, init, hardening
```

**Target: ~10,000 LOC kernel, 150+ commands, user-space programs running**

---

## Stage A: Fajar Lang Enhancements (3 sprints, 30 tasks)

### Sprint A1: Parser + Port I/O (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| A1.1 | Fix parser `(expr)` after call | Modify parse_postfix: don't chain `(` as call if prev was complete stmt | [ ] |
| A1.2 | Test: `fn(); (x + 1)` parses correctly | Verify two separate statements, not nested call | [ ] |
| A1.3 | Test: `volatile_write(); (val)` pattern | Ensure kernel patterns compile without workaround | [ ] |
| A1.4 | Add `port_inb(port) -> i64` | Read byte from x86 I/O port — runtime_fns + linker asm | [ ] |
| A1.5 | Add `port_inw(port) -> i64` | Read 16-bit word from I/O port | [ ] |
| A1.6 | Add `port_ind(port) -> i64` | Read 32-bit dword from I/O port | [ ] |
| A1.7 | Add `port_outw(port, val)` | Write 16-bit word to I/O port | [ ] |
| A1.8 | Add `port_outd(port, val)` | Write 32-bit dword to I/O port | [ ] |
| A1.9 | Register all port_* in analyzer + interpreter | Stubs for hosted mode | [ ] |
| A1.10 | Native tests for port I/O | 4 tests: inb/inw/ind/outb compilation | [ ] |

### Sprint A2: CPU Control Builtins (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| A2.1 | Add `ltr(selector)` | Load Task Register — needed for TSS | [ ] |
| A2.2 | Add `lgdt_mem(addr)` | Load GDT from memory address | [ ] |
| A2.3 | Add `lidt_mem(addr)` | Load IDT from memory address | [ ] |
| A2.4 | Add `sgdt(buf)` / `sidt(buf)` | Store GDT/IDT pointer to buffer | [ ] |
| A2.5 | Add `syscall_setup(star, lstar, sfmask)` | Configure SYSCALL/SYSRET MSRs | [ ] |
| A2.6 | Add `swapgs()` | Swap GS base (user ↔ kernel) | [ ] |
| A2.7 | Add `int_n(vector)` | Software interrupt (for testing syscall path) | [ ] |
| A2.8 | Register all in analyzer + codegen (JIT+AOT) | Both paths | [ ] |
| A2.9 | Linker asm for all new builtins | x86_64 assembly in linker.rs | [ ] |
| A2.10 | Native tests for CPU builtins | Compilation tests (no-std) | [ ] |

### Sprint A3: Buffer Ops + Array Syntax (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| A3.1 | Add `memcmp_buf(a, b, len) -> i64` | Compare two memory buffers, return 0 if equal | [ ] |
| A3.2 | Add `memcpy_buf(dst, src, len)` | Copy len bytes (faster than byte loop) | [ ] |
| A3.3 | Add `memset_buf(dst, val, len)` | Fill buffer with byte value | [ ] |
| A3.4 | Register buffer ops in all layers | Runtime, codegen, analyzer, interpreter, linker | [ ] |
| A3.5 | Array repeat: `[0; 512]` parser | Parse `[expr; count]` as repeat-init array | [ ] |
| A3.6 | Array repeat: interpreter eval | Evaluate to Array with `count` copies of `expr` | [ ] |
| A3.7 | Array repeat: codegen emit | Emit loop or memset for zero-init in native | [ ] |
| A3.8 | Test: `[0; 512]` in @kernel | Verify bare-metal compilation | [ ] |
| A3.9 | Remove kernel workarounds | Replace intermediate variables caused by parser bug | [ ] |
| A3.10 | Full test suite: no regressions | cargo test --features native --lib: all pass | [ ] |

---

## Stage B: Nova v0.3 Core (9 sprints, 90 tasks)

### Sprint B1: FAT32 Write — Free Cluster + FAT Update (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| B1.1 | `fat32_find_free_cluster()` | Scan FAT for entry == 0x00000000 | [ ] |
| B1.2 | `fat32_alloc_cluster(prev)` | Allocate cluster, link to prev in FAT | [ ] |
| B1.3 | `fat32_write_fat_entry(cluster, val)` | Write 4-byte FAT entry to disk | [ ] |
| B1.4 | `fat32_free_cluster_chain(start)` | Mark all clusters in chain as free | [ ] |
| B1.5 | `fat32_write_cluster(cluster, buf)` | Write cluster data to disk sectors | [ ] |
| B1.6 | `fat32_sync_fat()` | Flush FAT table changes to NVMe | [ ] |
| B1.7 | `fat32_create_dir_entry(dir, name, cluster, size)` | Add entry to directory | [ ] |
| B1.8 | `fat32_delete_dir_entry(dir, name)` | Mark entry as 0xE5 (deleted) | [ ] |
| B1.9 | Shell: `fatwrite <file> <text>` | Create/overwrite file on FAT32 | [ ] |
| B1.10 | Test: write + read back | Write file, read back, verify content matches | [ ] |

### Sprint B2: FAT32 Write — File Operations (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| B2.1 | `fat32_create_file(name, data, len)` | Full create: alloc clusters + dir entry + write data | [ ] |
| B2.2 | `fat32_append_file(name, data, len)` | Extend existing file with new data | [ ] |
| B2.3 | `fat32_delete_file(name)` | Delete dir entry + free cluster chain | [ ] |
| B2.4 | `fat32_rename(old, new)` | Update directory entry name field | [ ] |
| B2.5 | Shell: `fatrm <file>` | Delete file from FAT32 | [ ] |
| B2.6 | Shell: `fatmkdir <name>` | Create directory (alloc cluster + "." + "..") | [ ] |
| B2.7 | Shell: `fatcp <src> <dst>` | Copy file within FAT32 | [ ] |
| B2.8 | Persistence test | Write file → reboot QEMU → read back → verify | [ ] |
| B2.9 | Free space calculation | Count free clusters × cluster_size | [ ] |
| B2.10 | Shell: `df` | Display FAT32 free/used space | [ ] |

### Sprint B3: User-Space GDT + TSS (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| B3.1 | Expand GDT: user code segment (DPL=3) | Selector 0x18 with RPL=3 | [ ] |
| B3.2 | Expand GDT: user data segment (DPL=3) | Selector 0x20 with RPL=3 | [ ] |
| B3.3 | TSS (Task State Segment) | 104-byte TSS with RSP0 for kernel stack | [ ] |
| B3.4 | Load TSS with `ltr` | TR points to TSS in GDT | [ ] |
| B3.5 | IST (Interrupt Stack Table) | IST1 for double fault, IST2 for NMI | [ ] |
| B3.6 | Kernel stack per-process | RSP0 in TSS updated on context switch | [ ] |
| B3.7 | User page table setup | PML4 entry with USER bit for user pages | [ ] |
| B3.8 | Map user code pages | ELF PT_LOAD → user-accessible pages | [ ] |
| B3.9 | Map user stack | 64KB at 0x7FFF_0000 with USER+WRITABLE | [ ] |
| B3.10 | Test: GDT reload + TSS load in QEMU | Verify no triple fault | [ ] |

### Sprint B4: SYSCALL/SYSRET Path (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| B4.1 | Configure STAR MSR | Kernel CS/SS in bits 47:32, User CS/SS in 63:48 | [ ] |
| B4.2 | Configure LSTAR MSR | Point to syscall entry handler | [ ] |
| B4.3 | Configure SFMASK MSR | Mask IF flag on syscall entry | [ ] |
| B4.4 | Syscall entry stub | Save registers, switch to kernel stack, call dispatch | [ ] |
| B4.5 | Syscall exit (SYSRET) | Restore registers, return to user RIP/RSP | [ ] |
| B4.6 | Syscall dispatch table | 32 entries: SYS_EXIT(0), SYS_WRITE(1), SYS_READ(2), ... | [ ] |
| B4.7 | SYS_WRITE → VGA + serial | Write from user buffer to console | [ ] |
| B4.8 | SYS_EXIT → process terminate | Clean up process, free pages | [ ] |
| B4.9 | SYS_MMAP → allocate user pages | Frame alloc + map with USER bit | [ ] |
| B4.10 | Test: user program calls SYS_WRITE | "Hello from user!" on VGA | [ ] |

### Sprint B5: ELF Exec → User Mode (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| B5.1 | `exec` loads ELF + transitions to Ring 3 | Full path: load → map → IRETQ | [ ] |
| B5.2 | User stack: push argc/argv | Arguments on user stack before entry | [ ] |
| B5.3 | Return from user: SYS_EXIT | Process returns to kernel cleanly | [ ] |
| B5.4 | Minimal user-space libc | `_start` → `main()` → `SYS_EXIT` | [ ] |
| B5.5 | Compile hello.fj → hello.elf | Fajar Lang → ELF binary for Nova | [ ] |
| B5.6 | Put hello.elf on FAT32 image | mkfs.fat + copy to test disk | [ ] |
| B5.7 | `exec hello.elf` → prints "Hello!" | Full pipeline working | [ ] |
| B5.8 | Multiple user programs | Load 3 different ELFs sequentially | [ ] |
| B5.9 | Process exit code | SYS_EXIT(code) visible to parent | [ ] |
| B5.10 | Shell: `run <file>` | Alias for exec with better UX | [ ] |

### Sprint B6: Process Management (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| B6.1 | Process table v2 | 16 slots: PID, state, page_table, kernel_stack, user_stack | [ ] |
| B6.2 | SYS_FORK | Clone process: copy page tables (COW later), new PID | [ ] |
| B6.3 | SYS_WAITPID | Parent blocks until child exits | [ ] |
| B6.4 | SYS_EXEC | Replace process image with new ELF | [ ] |
| B6.5 | SYS_GETPID / SYS_GETPPID | Return current/parent PID | [ ] |
| B6.6 | Process cleanup on exit | Free pages, release FDs, signal parent | [ ] |
| B6.7 | Orphan reaping | Init process (PID 1) reaps orphaned children | [ ] |
| B6.8 | Shell: `jobs` | List background processes | [ ] |
| B6.9 | Shell: `wait <pid>` | Wait for specific process | [ ] |
| B6.10 | Test: fork → exec → wait cycle | Parent forks, child execs, parent waits | [ ] |

### Sprint B7: Keyboard + PS/2 Driver (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| B7.1 | `port_inb(0x60)` for scancode read | Real keyboard input via I/O port | [ ] |
| B7.2 | IRQ1 handler for keyboard | IDT vector 0x21 → read scancode | [ ] |
| B7.3 | Scancode → ASCII lookup table | Set 1 scancodes (make/break) | [ ] |
| B7.4 | Keyboard ring buffer | 64-byte circular buffer for key events | [ ] |
| B7.5 | SYS_READ(0) → keyboard input | User process reads stdin | [ ] |
| B7.6 | Line-buffered input | Backspace, Enter to submit line | [ ] |
| B7.7 | Ctrl+C signal | SIGINT to foreground process | [ ] |
| B7.8 | Special keys | Arrow keys, Home, End, Delete | [ ] |
| B7.9 | Shell: interactive input from user | Read command from keyboard buffer | [ ] |
| B7.10 | Test: user program reads keyboard | Echoes typed characters | [ ] |

### Sprint B8: USB/XHCI Foundation (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| B8.1 | USB PCI discovery | Find XHCI controller (class 0x0C, subclass 0x03, progif 0x30) | [ ] |
| B8.2 | XHCI BAR mapping | Map MMIO capability registers | [ ] |
| B8.3 | XHCI capability parse | CAPLENGTH, HCSPARAMS1/2/3, HCCPARAMS | [ ] |
| B8.4 | XHCI operational registers | USBCMD, USBSTS, PAGESIZE, DNCTRL | [ ] |
| B8.5 | Command ring setup | Allocate TRB ring (256 entries) | [ ] |
| B8.6 | Event ring setup | Allocate event ring + ERST | [ ] |
| B8.7 | DCBAA (Device Context Base Address Array) | For device slot allocation | [ ] |
| B8.8 | Port status check | Enumerate connected USB devices | [ ] |
| B8.9 | Shell: `lsusb` | List detected USB devices | [ ] |
| B8.10 | Test: QEMU `-device qemu-xhci` | Verify XHCI detection | [ ] |

### Sprint B9: Virtio-Blk + Real NVMe Test (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| B9.1 | Virtio-blk driver | PCI vendor 0x1AF4 device 0x1001 | [ ] |
| B9.2 | Virtqueue setup | Descriptor + available + used rings | [ ] |
| B9.3 | Virtio feature negotiation | VIRTIO_BLK_F_SIZE_MAX, SEG_MAX | [ ] |
| B9.4 | Virtio-blk read | Read sector via virtqueue | [ ] |
| B9.5 | Virtio-blk write | Write sector via virtqueue | [ ] |
| B9.6 | Register as blk_dev 2 | Virtio-blk alongside NVMe | [ ] |
| B9.7 | MBR partition table parse | 4 partition entries from sector 0 | [ ] |
| B9.8 | GPT support (basic) | GPT header at LBA 1, partition entries | [ ] |
| B9.9 | NVMe write persistence test | Write → reboot → verify on QEMU NVMe | [ ] |
| B9.10 | Benchmark: NVMe vs Virtio-blk | Sequential read/write throughput | [ ] |

---

## Stage C: Nova v0.3 Polish (6 sprints, 60 tasks)

### Sprint C1: Shell Scripting (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| C1.1 | Script file format | `#!/fj` shebang, one command per line | [ ] |
| C1.2 | `source <file>` command | Read lines from FAT32, execute each | [ ] |
| C1.3 | Comment support | Lines starting with `#` are ignored | [ ] |
| C1.4 | Variable assignment | `$var=value`, expand `$var` in commands | [ ] |
| C1.5 | Conditional: `if`/`then`/`fi` | Simple condition (exit code check) | [ ] |
| C1.6 | Loop: `while`/`do`/`done` | Repeat commands | [ ] |
| C1.7 | `/etc/init.sh` autorun | Execute script at boot after shell init | [ ] |
| C1.8 | Script arguments | `source script.sh arg1 arg2` | [ ] |
| C1.9 | `echo` with variable expansion | `echo "Hello $user"` | [ ] |
| C1.10 | Test: boot script sets hostname | `/etc/init.sh` → `hostname mybox` | [ ] |

### Sprint C2: Init Process + Service Management (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| C2.1 | Init process (PID 1) | First process, spawns shell, reaps orphans | [ ] |
| C2.2 | `/etc/inittab` config | Define which services to start | [ ] |
| C2.3 | Service restart on crash | If child exits with error, restart after delay | [ ] |
| C2.4 | Shutdown sequence | Sync filesystems → kill processes → ACPI poweroff | [ ] |
| C2.5 | Reboot sequence | Sync → kill → keyboard controller reset | [ ] |
| C2.6 | Signal infrastructure | SIGTERM, SIGKILL delivery to processes | [ ] |
| C2.7 | SYS_KILL syscall | Send signal to process by PID | [ ] |
| C2.8 | Process group | Group processes for signal broadcast | [ ] |
| C2.9 | Shell: `service list/start/stop` | Manage running services | [ ] |
| C2.10 | Test: init → shell → user program → exit | Full lifecycle | [ ] |

### Sprint C3: Pipe + Redirect (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| C3.1 | Pipe: `cmd1 \| cmd2` | Connect stdout of cmd1 to stdin of cmd2 | [ ] |
| C3.2 | File descriptor table per process | 16 FDs: 0=stdin, 1=stdout, 2=stderr | [ ] |
| C3.3 | Redirect: `cmd > file` | Write stdout to FAT32 file | [ ] |
| C3.4 | Redirect: `cmd < file` | Read stdin from FAT32 file | [ ] |
| C3.5 | Append: `cmd >> file` | Append stdout to file | [ ] |
| C3.6 | SYS_PIPE syscall | Create pipe, return read/write FDs | [ ] |
| C3.7 | SYS_DUP2 syscall | Duplicate FD (for redirect) | [ ] |
| C3.8 | `/dev/console` as FD | stdin/stdout default to VGA console | [ ] |
| C3.9 | Test: `ls \| grep txt` | Pipeline working | [ ] |
| C3.10 | Test: `echo hello > /mnt/test.txt` | Redirect to FAT32 file | [ ] |

### Sprint C4: File Operations + Persistence (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| C4.1 | Unified `ls` | List ramfs or FAT32 depending on path | [ ] |
| C4.2 | Unified `cat` | Read from any VFS mount | [ ] |
| C4.3 | Unified `cp` | Copy across mounts (FAT32 ↔ ramfs) | [ ] |
| C4.4 | Unified `rm` | Delete from any writable mount | [ ] |
| C4.5 | Unified `mkdir` | Create directory on FAT32 | [ ] |
| C4.6 | `save` command | Save ramfs state to FAT32 | [ ] |
| C4.7 | `load` command | Restore ramfs from FAT32 | [ ] |
| C4.8 | Hostname persistence | Write to /mnt/etc/hostname | [ ] |
| C4.9 | Command history persistence | Save/load from /mnt/etc/history | [ ] |
| C4.10 | Test: full persist cycle | Set hostname → reboot → verify hostname | [ ] |

### Sprint C5: Security + Hardening (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| C5.1 | SMEP enforcement | Prevent kernel executing user pages | [ ] |
| C5.2 | SMAP enforcement | Prevent kernel accessing user pages without STAC/CLAC | [ ] |
| C5.3 | NX bit on data pages | Non-executable stack + heap | [ ] |
| C5.4 | W^X enforcement | No page is both writable and executable | [ ] |
| C5.5 | Stack canary | Guard page below kernel stack | [ ] |
| C5.6 | Process memory isolation | Each process has separate page table | [ ] |
| C5.7 | Kernel ASLR (basic) | Randomize heap base with rdrand | [ ] |
| C5.8 | Syscall argument validation | Check buffer addresses are in user space | [ ] |
| C5.9 | Resource limits | Max memory per process, max open FDs | [ ] |
| C5.10 | Security audit | Review all privilege transitions | [ ] |

### Sprint C6: Release + Documentation (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| C6.1 | Update FAJAROS_NOVA_V2_PLAN.md | Mark all v0.3 tasks | [ ] |
| C6.2 | NOVA_USERLAND.md | Document syscall ABI, ELF format, libc | [ ] |
| C6.3 | NOVA_DRIVERS.md | Document NVMe, FAT32, USB, Virtio | [ ] |
| C6.4 | NOVA_SECURITY.md | Document SMEP/SMAP/NX/W^X model | [ ] |
| C6.5 | Update CI | QEMU boot test with user-space program | [ ] |
| C6.6 | Update CHANGELOG.md | Nova v0.3 release notes | [ ] |
| C6.7 | Update README.md | Nova v0.3 features + examples | [ ] |
| C6.8 | Performance benchmarks | NVMe, FAT32, context switch, syscall latency | [ ] |
| C6.9 | Tag nova-v0.3.0 | Git tag + push | [ ] |
| C6.10 | Blog: BLOG_NOVA_V03.md | Technical deep-dive post | [ ] |

---

## Dependency Graph

```
Stage A (Fajar Lang Enhancements)
  ├── A1: Parser fix + port I/O
  ├── A2: CPU control builtins (ltr, lgdt, syscall_setup)
  └── A3: Buffer ops + array syntax
        │
        ▼
Stage B (Nova v0.3 Core)
  ├── B1-B2: FAT32 write (depends on A3: memcpy_buf)
  ├── B3-B4: User-space GDT/TSS/SYSCALL (depends on A2: ltr, syscall_setup)
  ├── B5-B6: ELF exec + process mgmt (depends on B3-B4)
  ├── B7: Keyboard driver (depends on A1: port_inb)
  ├── B8: USB/XHCI (depends on A1: port I/O + B3: MMIO mapping)
  └── B9: Virtio-blk (depends on A1: port I/O)
        │
        ▼
Stage C (Nova v0.3 Polish)
  ├── C1: Shell scripting (depends on B2: FAT32 read/write)
  ├── C2: Init process (depends on B5-B6: process mgmt)
  ├── C3: Pipe + redirect (depends on B6: FD table)
  ├── C4: Persistence (depends on B2: FAT32 write)
  ├── C5: Security (depends on B3-B4: user-space)
  └── C6: Release
```

## Architecture Target (v0.3)

```
              User Space (Ring 3)
  ┌──────────┬──────────┬──────────┐
  │ hello.elf│server.elf│  shell   │
  └────┬─────┴────┬─────┴────┬─────┘
       │ SYSCALL   │          │
  ═════╪══════════╪══════════╪═══════ Ring 0/3 boundary
       │          │          │
  ┌────┴──────────┴──────────┴─────┐
  │     Syscall Entry (LSTAR)       │
  │     Save regs, switch RSP0      │
  ├─────────────────────────────────┤
  │  Syscall Dispatch (32 entries)   │
  │  EXIT WRITE READ FORK EXEC      │
  │  WAITPID MMAP BRK PIPE DUP2     │
  ├─────────────────────────────────┤
  │  VFS Layer (mount table, FDs)    │
  ├────────┬────────┬───────────────┤
  │ ramfs  │ FAT32  │ devfs/procfs  │
  │  (/)   │ (/mnt) │ (/dev /proc)  │
  ├────────┴────────┴───────────────┤
  │  Block Device Layer              │
  ├────────┬────────┬───────────────┤
  │  NVMe  │virtio  │  USB mass     │
  ├────────┴────────┴───────────────┤
  │  Process Manager (fork/exec)     │
  │  SMP Scheduler (per-CPU queues)  │
  ├─────────────────────────────────┤
  │  Memory Manager (paging, COW)    │
  ├────────┬────────┬───────────────┤
  │  LAPIC │ IOAPIC │ PCI/DMA/USB   │
  ├────────┴────────┴───────────────┤
  │  TCP/IP Stack (ARP/IP/ICMP/TCP)  │
  ├─────────────────────────────────┤
  │  Keyboard │ VGA │ Serial │ Net   │
  └─────────────────────────────────┘
          Hardware (x86_64)
```

## Quality Gates

**Per Sprint:**
- All tasks checked
- QEMU boot test passes
- No kernel panics or triple faults
- cargo test --features native: all pass

**Stage A Gate:**
- Parser bug fixed, 12 workarounds removed from kernel
- All port I/O + CPU builtins compile in bare-metal AOT
- [0; N] syntax works in @kernel

**Stage B Gate:**
- User-space "Hello World" prints via SYS_WRITE
- FAT32 file survives reboot
- fork → exec → wait cycle works

**Stage C Gate:**
- Shell script runs at boot from /mnt/etc/init.sh
- SMEP/SMAP/NX enabled without triple fault
- All 150+ commands functional

## Estimated Kernel Size

```
Current (v0.2):   7,313 LOC | 122 commands | 197KB ELF
Target  (v0.3): ~12,000 LOC | 150 commands | ~300KB ELF
Growth:          +4,700 LOC | +28 commands
```

---

*FajarOS Nova v0.3 "Endurance" — the OS that runs real user programs.*
