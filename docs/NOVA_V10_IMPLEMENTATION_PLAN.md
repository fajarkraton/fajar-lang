# FajarOS Nova v1.0.0 "Genesis" — Detailed Implementation Plan

> **Date:** 2026-03-21
> **Author:** Fajar (PrimeCore.id) + Claude Opus 4.6
> **Baseline:** v0.6.0 "Ascension" — 10,760 LOC, 155 commands, 388 @kernel fns
> **Target:** v1.0.0 "Genesis" — fully usable OS with multitasking, protection, persistence
> **Estimated:** 7 milestones, 73 tasks, ~58 hours

---

## Technical Baseline (Current State)

### SYSCALL Stub (93 bytes at 0x8200)

```
Entry: SWAPGS → save user RSP to 0x652010 → load kernel RSP from 0x652008
       → push RCX(RIP)/R11(RFLAGS)/RDI/RSI/RDX
Dispatch:
  RAX=1  (SYS_WRITE): serial LODSB loop to 0x3F8 → return 0
  RAX=60 (SYS_EXIT):  HLT → infinite loop (BROKEN — never returns)
  Other:               return -1
Exit: pop regs → restore user RSP → SWAPGS → SYSRETQ
```

**Problem #1:** SYS_EXIT does HLT. Must instead restore kernel state and return to shell.
**Problem #2:** Only 2 syscalls in stub. Dispatch table exists in Fajar Lang code but stub doesn't call it.

### Process Table v2 (0x890000, 16 × 256 bytes)

```
Per process: state(+0) pid(+8) ppid(+16) entry(+24) user_rsp(+32)
             kernel_rsp(+40) page_table(+48) exit_code(+56) name[32](+64)
```

**Problem #3:** No saved registers (RAX-R15, RIP, RSP, RFLAGS) for context switch.
**Problem #4:** All processes share PML4 at 0x70000 — no isolation.

### Timer

```
PIT at 100 Hz (10ms tick), read_timer_ticks() builtin
Timer IRQ handler: increments counter only — no context switch
```

**Problem #5:** Timer ticks but doesn't trigger scheduler.

### Page Tables

```
PML4 at 0x70000: 64 × 2MB identity-mapped pages = 128MB
map_page(): creates 4KB pages via frame allocation
No CR3 switching per process
```

### Memory Map

```
0x000000-0x0FFFFF:  Reserved (1MB)
0x100000-0x120000:  Kernel .text/.rodata (128KB)
0x400000-0x580000:  Heap (1.5MB)
0x580000-0x581000:  Frame bitmap (4KB)
0x600000-0x700000:  Old process table + shell state
0x650000-0x660000:  Per-CPU data + FPU save area
0x700000-0x7E0000:  RamFS (896KB)
0x70000-0x74000:   PML4 page tables (16KB)
0x7F0000-0x800000:  Kernel stack (64KB)
0x8000-0x8200:     AP trampoline + SYSCALL stub
0x800000-0x807000:  NVMe queues + state
0x820000-0x823000:  FAT32 buffers
0x860000-0x880000:  Network state + buffers
0x880000-0x890000:  ELF buffer (64KB)
0x890000-0x8B0000:  Process table v2 + virtqueues + XHCI
0x8B0000-0x8C0000:  Program registry
0xA00000-0xB00000:  Ramdisk (1MB)
0x2000000-0x2030000: User programs (Ring 3)
0x2F00000-0x3000000: User stack
```

---

## Milestone 1: "Return" (v0.7) — SYS_EXIT Returns to Shell

**Goal:** Run user program → it exits → shell prompt reappears
**Effort:** ~8 hours | 10 tasks

### Technical Approach

The SYS_EXIT stub currently does `HLT`. We need it to:
1. Restore kernel RSP to the saved value (before `iretq_to_user` was called)
2. Jump back to the instruction after `iretq_to_user` in `kernel_main`

**Strategy:** Before calling `iretq_to_user`, save the "return address" (next instruction's RIP) and kernel RSP to known addresses. SYS_EXIT reads these and does a "longjmp" back.

### Tasks

| # | Task | Detail | Bytes/LOC |
|---|------|--------|-----------|
| 1.1 | **Add return-address save point** | Before `iretq_to_user(addr, stack, flags)`: save kernel RSP to `0x652020` and set a flag at `0x652028` = 1 (meaning "user program active"). After `iretq_to_user` returns (which it won't normally), continue to shell. | ~10 LOC |
| 1.2 | **Modify SYS_EXIT in SYSCALL stub** | Replace `HLT; JMP -2` with: load kernel RSP from `0x652020`, clear flag at `0x652028`, JMP to label after `iretq_to_user` call. Problem: we can't JMP to a Fajar Lang function from raw asm. **Alternative:** SYS_EXIT sets a "user_exited" flag at `0x652028` and does SYSRETQ to a known "exit trampoline" address. | ~30 bytes |
| 1.3 | **Exit trampoline approach** | Install a small user-mode stub at `0x2FFF000`: reads the "exited" flag from kernel memory (mapped PAGE_USER read-only) and loops. The shell loop checks this flag on each iteration. When set, the shell knows the user program exited and re-displays prompt. | ~20 LOC |
| 1.4 | **Simpler approach: SYS_EXIT → kernel stack + RET** | In SYSCALL stub, SYS_EXIT does: restore kernel RSP from `0x652020`, then `RET` (which pops the return address that `cmd_run_program` pushed when calling `iretq_to_user`). BUT `iretq_to_user` is a builtin that doesn't push a return address on kernel stack — it directly does IRETQ. **Real fix:** wrap the call in a helper that saves RSP before the builtin call. | ~15 LOC |
| 1.5 | **Implement the wrapper** | Create `@kernel fn run_user_program(code_addr, stack_addr)` that: (a) saves kernel RSP to `0x652020`, (b) calls `iretq_to_user`, (c) after return label: restore shell state. The SYSCALL stub's SYS_EXIT handler restores `0x652020` → RSP and jumps to (c). | ~30 LOC |
| 1.6 | **Rewrite SYSCALL stub SYS_EXIT** | Replace bytes 57-59 (`HLT; JMP -2`) with: `MOV RSP, [0x652020]; SWAPGS; RET` (7 bytes). This returns through the kernel call stack back to `cmd_run_program`. | ~10 bytes |
| 1.7 | **Test: `run0` → "Hello Ring 3!" → `nova>` prompt** | Verify user program prints output and shell resumes. | 1 test |
| 1.8 | **SYS_READ (keyboard input)** | Add RAX=0 handler to SYSCALL stub: read from keyboard ring buffer (`0x892000`), block until char available, return char in RAX. | ~20 bytes |
| 1.9 | **Interactive user program** | Install program at slot 3: reads one line from keyboard via SYS_READ, echoes it via SYS_WRITE, then SYS_EXIT. Tests user I/O. | ~40 LOC |
| 1.10 | **Sequential execution test** | `run0` → `run1` → `run2` → all return to shell. | 1 test |

### Key Insight: SYS_EXIT Fix

The cleanest approach is to modify the SYSCALL stub (93 bytes at 0x8200) to handle SYS_EXIT as:

```asm
; Instead of HLT:
mov rsp, [0x652020]    ; restore kernel RSP (saved before iretq_to_user)
pop rdx                ; pop saved args (match the push sequence)
pop rsi
pop rdi
pop r11
pop rcx
swapgs                 ; back to kernel GS
; Now RSP is back to kernel_main's stack frame
; Set a "user_exited" flag
mov byte [0x652028], 1
; Return to caller of iretq_to_user
ret
```

But `iretq_to_user` is a compiler builtin that modifies the stack. The implementation must account for this.

**Simplest working approach:** Modify `cmd_run_program` to save RSP before calling `iretq_to_user` using inline assembly or a volatile write. Then SYS_EXIT restores that RSP and returns.

### Memory Changes

| Address | New Purpose |
|---------|------------|
| 0x652020 | Kernel RSP save (before user program launch) |
| 0x652028 | User-exited flag (1 = user program returned) |

### Quality Gate
- [ ] `run0` prints "Hello Ring 3!" and returns to `nova>` prompt
- [ ] `run1` prints "Goodbye Ring 3!" and returns
- [ ] Sequential: `run0` → `run1` → `run2` all succeed
- [ ] No triple fault or hang after SYS_EXIT

---

## Milestone 2: "Multitask" (v0.8) — Preemptive Scheduling

**Goal:** Multiple processes run concurrently via timer-driven context switching
**Effort:** ~12 hours | 12 tasks

### Technical Approach

The PIT timer fires IRQ0 (vector 0x20) every 10ms. Currently, the handler just increments a counter. We need it to:
1. Save all registers of current process
2. Call scheduler to pick next READY process
3. Restore all registers of next process
4. IRETQ to resume next process

**Challenge:** The timer ISR runs in kernel mode. The register save must happen BEFORE any Fajar Lang code runs (because Fajar Lang code clobbers registers).

**Solution:** Install a custom ISR stub (like the SYSCALL stub) that saves all registers, then calls a Fajar Lang scheduler function, then restores registers and IRETs.

### Context Frame (256 bytes per process, at PROC_TABLE_V2 + pid*256 + 96)

```
+96:  saved_rax    (8 bytes)
+104: saved_rbx    (8 bytes)
+112: saved_rcx    (8 bytes)
+120: saved_rdx    (8 bytes)
+128: saved_rsi    (8 bytes)
+136: saved_rdi    (8 bytes)
+144: saved_rbp    (8 bytes)
+152: saved_r8     (8 bytes)
+160: saved_r9     (8 bytes)
+168: saved_r10    (8 bytes)
+176: saved_r11    (8 bytes)
+184: saved_r12    (8 bytes)
+192: saved_r13    (8 bytes)
+200: saved_r14    (8 bytes)
+208: saved_r15    (8 bytes)
+216: saved_rip    (8 bytes)
+224: saved_rsp    (8 bytes)
+232: saved_rflags (8 bytes)
+240: saved_cr3    (8 bytes)
+248: saved_cs     (8 bytes)  — 0x08 (kernel) or 0x23 (user)
```

### Tasks

| # | Task | Detail | Effort |
|---|------|--------|--------|
| 2.1 | **Define context frame layout** | Add constants for register offsets within process table entry (+96 to +248) | 0.5 hr |
| 2.2 | **Timer ISR stub (raw asm)** | Install ~120-byte ISR stub at 0x8300: push all GPRs, call `sched_tick()`, pop all GPRs, IRETQ. Write via volatile_write_u8 like SYSCALL stub. | 3 hrs |
| 2.3 | **Replace IDT vector 0x20** | Point IDT entry for timer (IRQ0 = vector 0x20) to new ISR stub at 0x8300 instead of default handler | 0.5 hr |
| 2.4 | **`sched_tick()` function** | Fajar Lang function called from ISR stub: save current process registers (from ISR stack frame), call `pick_next()`, restore next process registers, return to ISR stub. | 2 hrs |
| 2.5 | **`pick_next()` scheduler** | Round-robin: scan PIDs from current+1, wrap around, find first READY process. If none found, return current PID (idle). | 1 hr |
| 2.6 | **`sched_spawn(entry, name)` function** | Allocate PID, set state=READY, set entry point, allocate user stack, initialize context frame (RIP=entry, RSP=stack_top, RFLAGS=0x202, CS=0x23 for Ring 3 or 0x08 for kernel). | 1 hr |
| 2.7 | **`spawn` command (real)** | Parse program name from command buffer, look up in registry, call `sched_spawn()`. | 1 hr |
| 2.8 | **`kill` command (real)** | Set process state to ZOMBIE, free resources. If target is current process, force schedule. | 0.5 hr |
| 2.9 | **`wait` command (real)** | Block current process until target becomes ZOMBIE, return exit code. | 0.5 hr |
| 2.10 | **Kernel idle process** | PID 0 runs the shell loop. When no other process is READY, scheduler returns to PID 0. | 0.5 hr |
| 2.11 | **Test: two processes print alternately** | Spawn two "hello" processes, both print via SYS_WRITE. Timer switches between them. Serial output shows interleaved output. | 1 hr |
| 2.12 | **Test: spawn + wait + exit code** | `spawn hello` → `wait 1` → "exit code 0" | 0.5 hr |

### Timer ISR Stub (~120 bytes at 0x8300)

```asm
timer_isr:
    ; CPU already pushed SS, RSP, RFLAGS, CS, RIP on interrupt stack
    push rax
    push rbx
    push rcx
    push rdx
    push rsi
    push rdi
    push rbp
    push r8
    push r9
    push r10
    push r11
    push r12
    push r13
    push r14
    push r15

    ; RSP now points to saved register frame on stack
    mov rdi, rsp          ; arg0 = pointer to saved registers
    call sched_tick       ; Fajar Lang function — may switch RSP!

    pop r15
    pop r14
    pop r13
    pop r12
    pop r11
    pop r10
    pop r9
    pop r8
    pop rbp
    pop rdi
    pop rsi
    pop rdx
    pop rcx
    pop rbx
    pop rax

    ; Send EOI to PIC
    mov al, 0x20
    out 0x20, al

    iretq                 ; return to (possibly different) process
```

### Quality Gate
- [ ] Timer ISR fires and saves/restores registers without crash
- [ ] `spawn hello` creates a new process that runs in Ring 3
- [ ] Two spawned processes produce interleaved output on serial
- [ ] `kill 2` terminates a running process
- [ ] Kernel shell (PID 0) remains responsive during all of this

---

## Milestone 3: "Protect" (v0.9) — Per-Process Page Tables

**Goal:** Each process has its own address space, can't access others' memory
**Effort:** ~8 hours | 8 tasks

### Technical Approach

Currently all processes share PML4 at 0x70000 (identity-mapped 128MB). We need:
1. Clone kernel page tables for each new process
2. Add user-specific pages (code + stack + heap)
3. Switch CR3 on every context switch

### Tasks

| # | Task | Detail | Effort |
|---|------|--------|--------|
| 3.1 | **`clone_kernel_page_table()` → new PML4** | Allocate frame for new PML4, copy kernel entries (0-63), leave user entries empty. Return physical address of new PML4. | 2 hrs |
| 3.2 | **Map user pages per process** | For each process: map code pages (from ELF or installed program), stack pages, and optionally heap pages with PAGE_USER flag. | 1.5 hrs |
| 3.3 | **Store CR3 in process table** | `proc_v2_set(pid, 48, new_pml4_addr)` — save page table base per process. | 0.5 hr |
| 3.4 | **CR3 switch in context switch** | In `sched_tick()`: `write_cr3(proc_v2_get(next_pid, 48))` before restoring registers. | 1 hr |
| 3.5 | **Page fault handler** | IDT vector 14 (page fault): read CR2 (faulting address), check if address belongs to current process. If not, kill process and return to scheduler. | 1.5 hrs |
| 3.6 | **Lazy page allocation (optional)** | On page fault in user heap range, allocate a new frame and map it. | 1 hr |
| 3.7 | **Test: process can't read kernel memory** | User program tries to read 0x100000 (kernel .text) → page fault → process killed → shell continues. | 0.5 hr |
| 3.8 | **Test: process A can't read process B** | Two user processes with different page tables. Process A tries to access Process B's memory → fault. | 0.5 hr |

### Quality Gate
- [ ] Each spawned process gets its own PML4
- [ ] CR3 is switched on every context switch
- [ ] User program accessing kernel memory causes page fault → process killed
- [ ] Kernel continues running after killing a faulting user process

---

## Milestone 4: "Load" (v1.0-alpha) — ELF from Filesystem

**Goal:** Load and execute ELF64 binaries from FAT32
**Effort:** ~6 hours | 7 tasks

### Tasks

| # | Task | Detail | Effort |
|---|------|--------|--------|
| 4.1 | **`exec <file>` shell command** | Read file from FAT32 (or USB FAT32) into ELF buffer at 0x880000. | 1 hr |
| 4.2 | **ELF validation** | Check magic, e_machine=0x3E, e_type=EXEC or DYN. Reject invalid ELF with error message. | 0.5 hr |
| 4.3 | **Load PT_LOAD segments** | For each PT_LOAD: allocate pages, map with PAGE_USER, copy data from ELF buffer, zero-fill BSS. | 1.5 hrs |
| 4.4 | **Set up user stack** | Allocate 16 pages (64KB) for user stack, map with PAGE_USER. Set RSP to top of stack. | 0.5 hr |
| 4.5 | **Transition to Ring 3** | Create process entry, set entry point from ELF header (e_entry), call `sched_spawn()`. | 1 hr |
| 4.6 | **Cross-compile user program** | Add `--target x86_64-user` to Fajar Lang compiler: generate ELF with e_entry, PT_LOAD, SYSCALL for I/O. | 1 hr |
| 4.7 | **End-to-end test** | Write hello.fj → compile → copy to USB → mount in Nova → `exec /usb/hello.elf` → "Hello!" on serial. | 0.5 hr |

### Quality Gate
- [ ] `exec /mnt/hello.elf` loads ELF from FAT32 and runs in Ring 3
- [ ] Program output appears on serial/VGA via SYS_WRITE
- [ ] Program exits cleanly and shell resumes

---

## Milestone 5: "Connect" (v1.0-beta) — Real Networking

**Goal:** DHCP, verified ping, TCP connection
**Effort:** ~10 hours | 8 tasks

### Tasks

| # | Task | Detail | Effort |
|---|------|--------|--------|
| 5.1 | **Verify ping in QEMU** | Boot with `-netdev user -device virtio-net-pci`, run `ping`, check if ICMP echo reply is received. Debug if not. | 1 hr |
| 5.2 | **Virtio-net RX notifications** | Currently polling-based. Add virtio-net IRQ handler via IOAPIC/MSI-X to trigger packet processing on receive. | 2 hrs |
| 5.3 | **DHCP Discover** | Build UDP packet: src=0.0.0.0:68, dst=255.255.255.255:67, DHCP Discover options. Send via virtio-net. | 1.5 hrs |
| 5.4 | **DHCP Offer → Request → Ack** | Parse Offer (get offered IP, server IP, lease time), send DHCP Request, parse Ack, configure interface. | 2 hrs |
| 5.5 | **Verified real ping** | After DHCP assigns IP, `ping 10.0.2.2` → real ICMP echo reply received and printed. | 0.5 hr |
| 5.6 | **TCP SYN/ACK handshake** | Build TCP SYN, send, receive SYN-ACK, send ACK. Track connection state (seq/ack numbers). | 1.5 hrs |
| 5.7 | **TCP data send/recv** | Send HTTP GET request, receive response, print to serial. | 1 hr |
| 5.8 | **`wget` command** | `wget http://10.0.2.2:8080/hello.txt` → TCP connect → HTTP GET → save response to FAT32. | 0.5 hr |

### Quality Gate
- [ ] `ping 10.0.2.2` → real ICMP echo reply with RTT displayed
- [ ] DHCP acquires IP address from QEMU's built-in DHCP server
- [ ] TCP connection established (3-way handshake verified)

---

## Milestone 6: "Sustain" (v1.0-rc) — Persistence + Init

**Goal:** Files survive reboot, proper init process, clean shutdown
**Effort:** ~8 hours | 8 tasks

### Tasks

| # | Task | Detail | Effort |
|---|------|--------|--------|
| 6.1 | **FAT32 dirty tracking** | Track which sectors were modified (bitmap). On `sync`, write only dirty sectors to NVMe. | 2 hrs |
| 6.2 | **`sync` command (real)** | Iterate dirty bitmap, call `nvme_write_sectors` for each dirty sector. Clear bitmap after flush. | 1 hr |
| 6.3 | **Persistence test** | Write file → `sync` → reboot (keyboard 0xFE) → re-mount → verify file still exists. | 1 hr |
| 6.4 | **Init process (PID 1)** | After kernel init, spawn init process. Init starts shell. If shell exits, init respawns it. Kernel (PID 0) runs idle loop. | 1.5 hrs |
| 6.5 | **Clean shutdown sequence** | `shutdown` → (1) signal all processes SIGTERM, (2) wait 1s, (3) SIGKILL remaining, (4) sync filesystems, (5) ACPI poweroff. | 1 hr |
| 6.6 | **Signal support (basic)** | SIGTERM (15) = set flag on process, process checks flag on next syscall. SIGKILL (9) = immediate terminate. | 1 hr |
| 6.7 | **`reboot` clean** | `reboot` → sync → reset via keyboard controller 0xFE. | 0.25 hr |
| 6.8 | **Boot-time fsck** | On mount, verify FAT32 BPB signature, cluster chain consistency. Print warning if corrupt. | 0.25 hr |

### Quality Gate
- [ ] `echo "hello" > /mnt/test.txt` → `sync` → `reboot` → `cat /mnt/test.txt` → "hello"
- [ ] Init process (PID 1) auto-starts shell
- [ ] `shutdown` cleans up all processes and powers off

---

## Milestone 7: "Genesis" (v1.0.0) — Production Release

**Goal:** CI/CD, documentation, real hardware test, release
**Effort:** ~6 hours | 8 tasks

### Tasks

| # | Task | Detail | Effort |
|---|------|--------|--------|
| 7.1 | **GitHub Actions CI** | On push: `cargo test --features native`, build kernel ELF, QEMU smoke test (boot + serial check). | 1.5 hrs |
| 7.2 | **User manual** | Boot guide, shell command reference (155+ commands), syscall reference, programming guide. | 1.5 hrs |
| 7.3 | **Create bootable USB** | `grub-mkrescue` → ISO → `dd` to USB stick → boot on real hardware. | 0.5 hr |
| 7.4 | **Real hardware test** | Boot FajarOS on Intel i9-14900HX (Lenovo Legion Pro). Test: serial, VGA, keyboard, NVMe, shell. | 1 hr |
| 7.5 | **Performance benchmarks** | Measure: boot time, fib(30) in Ring 3, file read throughput, context switch latency. | 0.5 hr |
| 7.6 | **Security audit** | Verify: NX bit, stack guard, Ring 3 can't access kernel, no buffer overflows in shell parsing. | 0.5 hr |
| 7.7 | **Release v1.0.0** | Version bump, CHANGELOG, git tag, GitHub release with binaries + ISO. | 0.25 hr |
| 7.8 | **Blog post** | "FajarOS: A complete OS written 100% in Fajar Lang" — architecture, features, benchmarks. | 0.25 hr |

### Quality Gate
- [ ] CI green on GitHub Actions
- [ ] Boots on real Intel i9-14900HX hardware
- [ ] All 155+ shell commands work
- [ ] At least 3 user programs run in Ring 3
- [ ] File persistence across reboot verified

---

## Cross-Milestone Dependencies

```
M1 (Return to Shell)
 │
 ├─► M2 (Multitasking) ──► M3 (Memory Protection) ──► M4 (ELF from Disk)
 │                                                       │
 │                                                       ▼
 │                                                 M7 (Release)
 │                                                       ▲
 ├─► M5 (Networking) ────────────────────────────────────┘
 │                                                       │
 └─► M6 (Persistence) ──────────────────────────────────┘
```

**Critical Path:** M1 → M2 → M3 → M4 (34 hours)
**Parallel Path:** M5 (can start after M1), M6 (can start after M1)
**Final:** M7 (after all milestones)

---

## LOC Projections

| Milestone | New LOC | Cumulative | Key Files Modified |
|-----------|---------|------------|-------------------|
| v0.6.0 (now) | — | 10,760 | — |
| v0.7 (Return) | +300 | 11,060 | SYSCALL stub, cmd_run_program |
| v0.8 (Multitask) | +800 | 11,860 | Timer ISR stub, sched_tick, spawn/kill/wait |
| v0.9 (Protect) | +500 | 12,360 | clone_page_table, CR3 switch, page fault handler |
| v1.0α (Load) | +400 | 12,760 | exec command, ELF loader integration |
| v1.0β (Connect) | +700 | 13,460 | DHCP, TCP, wget |
| v1.0rc (Sustain) | +400 | 13,860 | dirty tracking, init process, shutdown sequence |
| v1.0.0 (Genesis) | +200 | 14,060 | CI config, docs, benchmarks |

**Total: ~3,300 new LOC to reach v1.0.0**

---

## Risk Assessment

| Risk | Impact | Mitigation |
|------|--------|------------|
| Timer ISR corrupts registers | Kernel crash | Extensive QEMU testing with `-d int` debug |
| CR3 switch breaks identity mapping | Triple fault | Keep kernel pages in all page tables |
| SYSCALL from Ring 3 + timer IRQ race | Corruption | Mask interrupts in SYSCALL stub (already done via SFMASK) |
| ELF loader loads malformed binary | Kernel crash | Validate all ELF fields before loading |
| TCP state machine bugs | Network hangs | Start with UDP-only, add TCP incrementally |
| Real hardware has different quirks | Boot failure | Test on QEMU first, fix HW-specific issues after |

---

## Session Planning (5 sessions × ~12 hours each)

```
Session 1:  M1 (Return to Shell)                    8 hrs
Session 2:  M2 (Multitasking)                       12 hrs
Session 3:  M3 (Memory Protection) + M4 (ELF)       14 hrs
Session 4:  M5 (Networking) + M6 (Persistence)       18 hrs
Session 5:  M7 (Release)                             6 hrs
```

---

*FajarOS Nova: from 10,760 LOC to 14,060 LOC — 73 tasks across 7 milestones*
*Target: a real, usable, multitasking OS written 100% in Fajar Lang*
*Built with Claude Opus 4.6*
