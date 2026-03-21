# FajarOS Nova v1.0.0 "Genesis" — Complete Implementation Plan

> **Date:** 2026-03-21
> **Author:** Fajar (PrimeCore.id) + Claude Opus 4.6
> **Baseline:** v0.6.0 "Ascension" — 10,760 LOC kernel, 388 @kernel fns, 155 commands
> **Target:** v1.0.0 "Genesis" — multitasking, protected, persistent, networked, real hardware
> **Scope:** 10 phases, 95 tasks, ~71 hours
> **Architecture:** Compiler enhancements (Phase 0) + 7 OS milestones (Phase 1-7) + 2 polish phases (Phase 8-9)

---

## Phase 0: Fajar Lang Compiler Enhancements

> **MUST complete before OS milestones.** The compiler currently lacks critical
> builtins and codegen features needed for a real OS.

### Phase 0A: CR3/CR2 Builtins (Sprint CE1 — 2 hours, 6 tasks)

**Why:** Per-process page tables require switching CR3 on context switch.
Page fault handler needs CR2 to know the faulting address.
Currently: `write_cr4`/`read_cr4` exist but `write_cr3`/`read_cr3`/`read_cr2` do NOT.

**Template:** Follow exact pattern of `read_cr4`/`write_cr4` (runtime_bare.rs line 2036).

| # | Task | File | Detail |
|---|------|------|--------|
| 0A.1 | Add `fj_rt_bare_write_cr3(val)` | `src/codegen/cranelift/runtime_bare.rs` | `unsafe { asm!("mov cr3, {}", in(reg) val) }` — same pattern as write_cr4 |
| 0A.2 | Add `fj_rt_bare_read_cr3() -> i64` | `src/codegen/cranelift/runtime_bare.rs` | `unsafe { asm!("mov {}, cr3", out(reg) val) }` — same pattern as read_cr4 |
| 0A.3 | Add `fj_rt_bare_read_cr2() -> i64` | `src/codegen/cranelift/runtime_bare.rs` | `unsafe { asm!("mov {}, cr2", out(reg) val) }` — CR2 holds page fault address |
| 0A.4 | Register in codegen | `src/codegen/cranelift/mod.rs` ~line 6128 | Add `("fj_rt_bare_write_cr3", "write_cr3", &sig_i64)` etc. to bare-metal builtin list |
| 0A.5 | Register in analyzer | `src/analyzer/type_check/register.rs` ~line 313 | Add `("write_cr3", vec![Type::I64], Type::Void)`, `("read_cr3", vec![], Type::I64)`, `("read_cr2", vec![], Type::I64)` |
| 0A.6 | Tests: 3 codegen tests | `src/codegen/cranelift/tests.rs` | Test that `write_cr3`, `read_cr3`, `read_cr2` compile without error in bare-metal context |

**Quality Gate:** `cargo test --features native` — all pass, `read_cr3()` available in .fj code.

---

### Phase 0B: @interrupt ISR Wrapper Generation (Sprint CE2 — 5 hours, 8 tasks)

**Why:** Timer-driven preemptive scheduling (M2) needs an ISR that saves ALL 15 GPRs before
any Fajar Lang code runs. Currently `@interrupt` annotation is parsed and tracked in
`interrupt_fns: Vec<String>` but NO codegen wrapper is generated.

**Template:** Follow `@entry` → `_start` wrapper pattern (mod.rs lines 11585-11722).

| # | Task | File | Detail |
|---|------|------|--------|
| 0B.1 | Design ISR wrapper layout | — | Document: ISR pushes 15 GPRs (120 bytes), calls @interrupt fn body, pops 15 GPRs, sends EOI, IRETQ |
| 0B.2 | Generate `__isr_FNAME` wrapper function | `src/codegen/cranelift/mod.rs` after _start generation (~line 11722) | For each function in `interrupt_fns`: create Cranelift function `__isr_FNAME` with no params/returns. Use `builder.ins()` to emit: push regs → `call FNAME` → pop regs → EOI → return |
| 0B.3 | Push/pop all GPRs in Cranelift IR | `src/codegen/cranelift/mod.rs` | Use stack slots for 15 registers: RAX,RBX,RCX,RDX,RSI,RDI,RBP,R8-R15. Save via `builder.ins().stack_store()`, restore via `builder.ins().stack_load()` |
| 0B.4 | Emit EOI (end of interrupt) | `src/codegen/cranelift/mod.rs` | After pop: `mov al, 0x20; out 0x20, al` — emit via Cranelift `iconst` + call to bare `port_outb` |
| 0B.5 | Use `iretq` instead of `ret` | `src/codegen/cranelift/mod.rs` | The wrapper function must end with `iretq` not `ret`. Emit as inline asm or use a bare-metal `iretq()` builtin (may need new builtin `iretq_from_isr`) |
| 0B.6 | Export `__isr_FNAME` symbol | `src/codegen/cranelift/mod.rs` | Symbol must be exported so IDT can reference it. Use `Linkage::Export` |
| 0B.7 | Test: `@interrupt fn timer() {}` compiles | `src/codegen/cranelift/tests.rs` | Verify `@interrupt fn foo() { let x = 1 }` generates valid ISR wrapper with register save/restore |
| 0B.8 | Wire IDT vector to ISR | `src/codegen/linker.rs` | If `@interrupt fn timer_handler()` exists, set IDT[0x20] to point to `__isr_timer_handler` instead of default handler |

**Alternative (simpler):** If Cranelift can't easily emit push/pop sequences (it works at SSA level, not register level), generate the ISR wrapper as raw bytes in `linker.rs` like the SYSCALL stub, using `fn_addr("timer_handler")` for the call target.

**Quality Gate:** `@interrupt fn foo() {}` compiles and produces valid x86_64 ISR code.

---

### Phase 0C: Extended SYSCALL Dispatch (Sprint CE3 — 3 hours, 6 tasks)

**Why:** Current SYSCALL stub at 0x8200 hardcodes only 2 syscalls (WRITE=1, EXIT=60).
For M1-M4, we need: EXIT(0), WRITE(1), READ(2), OPEN(3), CLOSE(4), GETPID(9), YIELD(10),
BRK(12), SPAWN(7), WAIT(8), KILL(15).

**Approach:** Replace inline dispatch with `call syscall_dispatch` to a Fajar Lang function.

| # | Task | File | Detail |
|---|------|------|--------|
| 0C.1 | Add `syscall_dispatch(num, a0, a1, a2) -> i64` | `linker.rs` or kernel .fj | Fajar Lang function that matches syscall number and calls handlers. Already partially exists at line 3535 of kernel. |
| 0C.2 | Modify SYSCALL stub to call dispatch | `src/codegen/linker.rs` ~line 2068 | New stub: save regs → `mov rdi, rax; mov rsi, rdi_user; mov rdx, rsi_user; mov rcx, rdx_user` → `call [dispatch_addr]` → restore → `sysretq`. Use `fn_addr("syscall_dispatch")` stored at fixed address (e.g., 0x652030). |
| 0C.3 | SYS_EXIT returns to kernel | kernel .fj + linker.rs | SYS_EXIT handler: mark process ZOMBIE, set `user_exited` flag at 0x652028, restore kernel RSP from 0x652020, `ret` (not HLT) |
| 0C.4 | SYS_READ (fd=0, keyboard) | kernel .fj | Read from keyboard ring buffer (0x892000), busy-wait if empty, return char in RAX |
| 0C.5 | SYS_GETPID, SYS_YIELD | kernel .fj | GETPID returns current PID, YIELD calls scheduler |
| 0C.6 | Test: user program calls 4 syscalls | kernel .fj | Install test program: WRITE("Hi\n") → READ(char) → WRITE(char) → EXIT(0). Verify sequential I/O. |

**SYSCALL Stub Redesign (at 0x8200, ~80 bytes):**

```asm
__syscall_entry:
    swapgs
    mov [0x652010], rsp           ; save user RSP
    mov rsp, [0x652008]           ; load kernel RSP
    push rcx                      ; save user RIP
    push r11                      ; save user RFLAGS
    ; Prepare args for syscall_dispatch(num, arg0, arg1, arg2)
    ; RAX=syscall_num, RDI=arg0, RSI=arg1, RDX=arg2
    ; x86_64 C ABI: RDI=arg0, RSI=arg1, RDX=arg2, RCX=arg3
    mov rcx, rdx                  ; arg2 → RCX (4th C arg)
    mov rdx, rsi                  ; arg1 → RDX (3rd C arg)
    mov rsi, rdi                  ; arg0 → RSI (2nd C arg)
    mov rdi, rax                  ; syscall_num → RDI (1st C arg)
    call [0x652030]               ; call syscall_dispatch (fn_addr stored here)
    ; RAX = return value from dispatch
    pop r11                       ; restore user RFLAGS
    pop rcx                       ; restore user RIP
    mov rsp, [0x652010]           ; restore user RSP
    swapgs
    sysretq
```

**Quality Gate:** SYSCALL from Ring 3 routes to Fajar Lang dispatch function. SYS_EXIT returns to shell.

---

### Phase 0D: x86_64-user Compilation Target (Sprint CE4 — 4 hours, 6 tasks)

**Why:** Currently user programs are hand-assembled x86 machine code bytes.
For M4 (ELF from disk), we need to compile .fj files into Ring 3 ELF binaries.

| # | Task | File | Detail |
|---|------|------|--------|
| 0D.1 | Add `--target x86_64-user` CLI option | `src/main.rs` | New target enum variant. When selected: no bare-metal runtime, no kernel builtins. |
| 0D.2 | User-mode runtime library | `src/codegen/cranelift/runtime_user.rs` (new) | Minimal runtime: `fj_rt_user_println(buf, len)` → calls SYS_WRITE via SYSCALL instruction. `fj_rt_user_exit(code)` → calls SYS_EXIT via SYSCALL. |
| 0D.3 | Syscall wrapper in runtime | `src/codegen/cranelift/runtime_user.rs` | `fn fj_rt_user_syscall(num, a0, a1, a2) -> i64` → inline asm: `mov rax, num; mov rdi, a0; ...; syscall; ret` |
| 0D.4 | Generate _start for user ELF | `src/codegen/cranelift/mod.rs` | When target=user: `_start` calls `main()`, then `SYS_EXIT(return_value)`. No BSS zeroing, no trampoline. |
| 0D.5 | Linker config for user ELF | `src/codegen/linker.rs` | User ELF: entry at 0x400000 (standard Linux VA), PT_LOAD segments, no Multiboot2 header, no GDT/IDT setup |
| 0D.6 | Test: compile + run user ELF | tests/ | `fj build --target x86_64-user hello.fj -o hello.elf` → valid ELF64. Load into Nova via `exec` → prints "Hello!" via SYSCALL. |

**Quality Gate:** `fj build --target x86_64-user` produces valid ELF that runs in Nova Ring 3.

---

## Phase 0 Summary

| Sprint | Tasks | Effort | Unblocks |
|--------|-------|--------|----------|
| CE1: CR3/CR2 builtins | 6 | 2 hrs | M3 (Memory Protection) |
| CE2: @interrupt ISR | 8 | 5 hrs | M2 (Multitasking) |
| CE3: SYSCALL dispatch | 6 | 3 hrs | M1 (Return to Shell) |
| CE4: x86_64-user target | 6 | 4 hrs | M4 (ELF from Disk) |
| **Total Phase 0** | **26** | **14 hrs** | **All OS milestones** |

---

## Phase 1: "Return" (v0.7) — SYS_EXIT Returns to Shell

> **Depends on:** Phase 0C (SYSCALL dispatch)
> **Effort:** ~6 hours | 8 tasks

### Technical Approach

With Phase 0C done, the SYSCALL stub calls `syscall_dispatch()` in Fajar Lang. SYS_EXIT
handler saves kernel RSP before `iretq_to_user`, and SYS_EXIT restores it.

| # | Task | Detail | Effort |
|---|------|--------|--------|
| 1.1 | Save kernel RSP in `cmd_run_program` | Before `iretq_to_user`: `volatile_write_u64(0x652020, kernel_rsp_value)` using inline assembly to read current RSP | 1 hr |
| 1.2 | SYS_EXIT handler in `syscall_dispatch` | `if num == 0 { proc_v2_exit(current_pid, arg0); restore kernel RSP from 0x652020; return }` — sets flag at 0x652028 | 1 hr |
| 1.3 | Shell detects user exit | After `iretq_to_user` returns (via SYS_EXIT restore), check flag 0x652028, clear it, redisplay prompt | 0.5 hr |
| 1.4 | SYS_WRITE handler | `if num == 1 { serial_write(arg0_buf, arg1_len); return len }` — write user buffer to serial | 0.5 hr |
| 1.5 | SYS_READ handler (keyboard) | `if num == 2 { while !kb_has_char() { pause() }; return kb_pop_char() }` — block until key | 1 hr |
| 1.6 | SYS_GETPID handler | `if num == 9 { return current_pid }` | 0.25 hr |
| 1.7 | Interactive user program | Install program slot 3: reads line via SYS_READ, echoes via SYS_WRITE, then SYS_EXIT | 1 hr |
| 1.8 | Test: `run0` → `run1` → `run2` → all return | Sequential execution verified in QEMU | 0.75 hr |

### Quality Gate
- [ ] `run0` → "Hello Ring 3!" → back to `nova>` prompt (no hang, no HLT)
- [ ] `run1` → "Goodbye Ring 3!" → back to prompt
- [ ] Interactive program reads keyboard input from Ring 3

---

## Phase 2: "Multitask" (v0.8) — Preemptive Scheduling

> **Depends on:** Phase 0B (@interrupt ISR), Phase 1 (SYS_EXIT works)
> **Effort:** ~10 hours | 10 tasks

### Technical Approach

With @interrupt ISR generation, write scheduler in Fajar Lang:
```fajar
@interrupt fn timer_handler() {
    save_current_process_regs()
    let next = pick_next_ready()
    if next != current_pid {
        switch_to(next)
    }
    // EOI auto-generated by @interrupt wrapper
}
```

| # | Task | Detail | Effort |
|---|------|--------|--------|
| 2.1 | Define context frame in process table | Offsets +96 to +248: 15 GPRs + RIP + RSP + RFLAGS + CR3 + CS (10 × 8 = 160 bytes) | 0.5 hr |
| 2.2 | `save_context(pid)` function | Save all GPRs from ISR stack frame to process table. The @interrupt wrapper pushes regs to stack — copy from stack to proc table. | 2 hrs |
| 2.3 | `restore_context(pid)` function | Copy saved regs from process table to ISR stack frame. When @interrupt wrapper pops, it restores the NEW process's registers. | 2 hrs |
| 2.4 | `@interrupt fn timer_handler()` | Call `save_context(current)`, call `pick_next()`, call `restore_context(next)`, update `current_pid`. | 1 hr |
| 2.5 | `pick_next()` round-robin | Scan PIDs from current+1, wrap at 16, find first READY. Return current if none. | 0.5 hr |
| 2.6 | `sched_spawn(entry, stack, name)` | Allocate PID, set state=READY, init context frame (RIP=entry, RSP=stack, RFLAGS=0x202, CS=0x23). | 1 hr |
| 2.7 | `spawn <name>` shell command | Look up program in registry, call `sched_spawn`, print PID. | 0.5 hr |
| 2.8 | `kill <pid>` command (real) | Set state=ZOMBIE, free stack frame. If killing current, force reschedule. | 0.5 hr |
| 2.9 | `wait <pid>` command (real) | Block shell until target PID becomes ZOMBIE, print exit code. | 0.5 hr |
| 2.10 | Test: interleaved output | `spawn hello` + `spawn goodbye` → timer switches, both print on serial | 1.5 hrs |

### Quality Gate
- [ ] Two spawned processes produce interleaved output
- [ ] `ps` shows running processes with correct state
- [ ] `kill 2` terminates a running process
- [ ] Shell (PID 0) remains responsive

---

## Phase 3: "Protect" (v0.9) — Per-Process Page Tables

> **Depends on:** Phase 0A (CR3/CR2 builtins), Phase 2 (context switch)
> **Effort:** ~8 hours | 8 tasks

| # | Task | Detail | Effort |
|---|------|--------|--------|
| 3.1 | `clone_kernel_pml4()` | Allocate frame for new PML4, copy entries 0-63 (kernel identity map). User entries start empty. Return phys addr. | 2 hrs |
| 3.2 | Map user code + stack per process | For each spawn: allocate pages for code region + stack region, map with PAGE_USER in process's PML4. | 1.5 hrs |
| 3.3 | Store CR3 in process table | `proc_v2_set(pid, 48, pml4_addr)` — page_table field at +48. | 0.5 hr |
| 3.4 | CR3 switch in context switch | In `restore_context`: `write_cr3(proc_v2_get(next_pid, 48))`. Flush TLB. | 1 hr |
| 3.5 | Page fault handler (IDT vector 14) | `@interrupt fn page_fault_handler()`: read CR2, check if address valid for current process, kill process if not. | 1.5 hrs |
| 3.6 | GPF handler for Ring 3 (IDT vector 13) | Privilege violation → print faulting address → kill process → reschedule. | 0.5 hr |
| 3.7 | Test: user can't access kernel | User program reads 0x100000 → page fault → killed → shell continues. | 0.5 hr |
| 3.8 | Test: process A can't read B | Two processes with separate PML4s. Cross-access → fault. | 0.5 hr |

### Quality Gate
- [ ] Each process has its own PML4
- [ ] `write_cr3` called on every context switch
- [ ] Kernel survives user page fault (kills process, not kernel)

---

## Phase 4: "Load" (v1.0-alpha) — ELF from Filesystem

> **Depends on:** Phase 0D (x86_64-user target), Phase 3 (per-process pages)
> **Effort:** ~6 hours | 7 tasks

| # | Task | Detail | Effort |
|---|------|--------|--------|
| 4.1 | `exec <path>` command | Read file from FAT32 to ELF buffer (0x880000). Validate ELF magic + machine. | 1 hr |
| 4.2 | Load PT_LOAD segments | Parse program headers, allocate pages in new PML4, copy file data, zero-fill BSS. | 1.5 hrs |
| 4.3 | User stack allocation | Allocate 16 pages (64KB), map with PAGE_USER. Set initial RSP. | 0.5 hr |
| 4.4 | Spawn from ELF | Create process entry with entry=e_entry, stack=allocated, pml4=new. Add to scheduler. | 1 hr |
| 4.5 | Compile hello.fj for user target | `fj build --target x86_64-user examples/hello_user.fj -o hello.elf` — produces Ring 3 ELF. | 0.5 hr |
| 4.6 | Copy ELF to USB → exec in Nova | Write hello.elf to FAT32 USB image, boot Nova, `exec /usb/hello.elf` → runs. | 1 hr |
| 4.7 | Test: 3 different ELF programs | hello.elf, counter.elf, fib.elf — all compile, load, and execute from FAT32. | 0.5 hr |

### Quality Gate
- [ ] `exec /mnt/hello.elf` → loads ELF → prints output → returns to shell
- [ ] `fj build --target x86_64-user` produces valid user ELF

---

## Phase 5: "Connect" (v1.0-beta) — Real Networking

> **Depends on:** Phase 1 (SYS_EXIT works — boot to shell)
> **Effort:** ~10 hours | 8 tasks

| # | Task | Detail | Effort |
|---|------|--------|--------|
| 5.1 | Verify ping in QEMU | Boot with `-netdev user -device virtio-net-pci`, test `ping`. Debug RX path if no reply. | 1 hr |
| 5.2 | Virtio-net RX via interrupt | Add IOAPIC routing for virtio-net IRQ. ISR calls `net_rx_poll()` and processes packet. | 2 hrs |
| 5.3 | UDP send/receive | Build UDP header (src/dst port, length, checksum). Needed for DHCP. | 1 hr |
| 5.4 | DHCP client | Discover (broadcast): src=0.0.0.0:68, dst=255.255.255.255:67. Parse Offer → send Request → parse Ack → configure IP. | 3 hrs |
| 5.5 | Verified real ping | After DHCP, `ping 10.0.2.2` → real ICMP echo reply with RTT. | 0.5 hr |
| 5.6 | TCP SYN/ACK handshake | SYN → SYN-ACK → ACK. Track seq/ack numbers, connection state. | 1.5 hrs |
| 5.7 | TCP data send/recv | Send HTTP GET, receive response body. | 0.5 hr |
| 5.8 | `wget` command | `wget http://10.0.2.2:8080/hello.txt` → TCP → HTTP GET → save to FAT32. | 0.5 hr |

### Quality Gate
- [ ] `ping 10.0.2.2` → real ICMP reply with RTT
- [ ] DHCP assigns IP address from QEMU
- [ ] `wget` downloads file via HTTP

---

## Phase 6: "Sustain" (v1.0-rc) — Persistence + Init

> **Depends on:** Phase 2 (multitasking), Phase 4 (ELF loading)
> **Effort:** ~8 hours | 8 tasks

| # | Task | Detail | Effort |
|---|------|--------|--------|
| 6.1 | FAT32 dirty sector tracking | Bitmap at 0x8C0000 (1 bit per sector). Set bit on `blk_write`. | 1 hr |
| 6.2 | `sync` command (real) | Iterate dirty bitmap, call `nvme_write_sectors` for each dirty sector. Clear bitmap. | 1 hr |
| 6.3 | Persistence test | Write file → `sync` → `reboot` (0xFE) → re-mount → verify file. | 1 hr |
| 6.4 | Init process (PID 1) | After kernel init, spawn init. Init spawns shell (PID 2). If shell exits, init respawns. | 1.5 hrs |
| 6.5 | Clean shutdown | `shutdown` → (1) SIGTERM all, (2) wait 1s, (3) SIGKILL, (4) sync, (5) ACPI off. | 1 hr |
| 6.6 | SIGTERM / SIGKILL | SIGTERM: set flag → process checks on next syscall. SIGKILL: immediate terminate. | 1 hr |
| 6.7 | Auto-mount at boot | On boot, if NVMe FAT32 exists → mount at /mnt. If USB FAT32 → mount at /usb. | 0.5 hr |
| 6.8 | Filesystem consistency check | On mount: verify BPB signature, check cluster chain. Print warning if corrupt. | 1 hr |

### Quality Gate
- [ ] `echo hello > /mnt/test.txt` → `sync` → `reboot` → `cat /mnt/test.txt` → "hello"
- [ ] Init respawns shell on exit
- [ ] `shutdown` cleanly syncs + powers off

---

## Phase 7: "Genesis" (v1.0.0) — Production Release

> **Depends on:** All phases complete
> **Effort:** ~6 hours | 8 tasks

| # | Task | Detail | Effort |
|---|------|--------|--------|
| 7.1 | GitHub Actions CI | Build + `cargo test --features native` + QEMU smoke test on push. | 1.5 hrs |
| 7.2 | User manual (Markdown) | Boot guide, 155+ command reference, syscall table, programming guide. | 1.5 hrs |
| 7.3 | Create bootable USB stick | `grub-mkrescue` → ISO → `dd` to USB → boot on real hardware. | 0.5 hr |
| 7.4 | Real hardware test | Boot FajarOS on Intel i9-14900HX (Lenovo Legion Pro). Serial + VGA + keyboard. | 1 hr |
| 7.5 | Performance benchmarks | Boot time, fib(30) in Ring 3, file I/O throughput, context switch latency. | 0.5 hr |
| 7.6 | Security audit | NX bit, stack guard, Ring 3 isolation, no buffer overflow in shell parser. | 0.5 hr |
| 7.7 | Release v1.0.0 | Cargo.toml → v4.0.0, CHANGELOG, git tag, GitHub release with ISO + binaries. | 0.25 hr |
| 7.8 | Blog post | "FajarOS: a complete OS written 100% in Fajar Lang" — architecture + benchmarks. | 0.25 hr |

### Quality Gate
- [ ] CI green on GitHub Actions
- [ ] Boots on real Intel i9-14900HX
- [ ] All commands work, user programs execute, files persist

---

## Complete Dependency Graph

```
Phase 0A (CR3/CR2)         Phase 0B (@interrupt)       Phase 0C (SYSCALL)       Phase 0D (user target)
  2 hrs                      5 hrs                       3 hrs                    4 hrs
    │                          │                           │                        │
    │                          │                           ▼                        │
    │                          │                     Phase 1 (Return)               │
    │                          │                       6 hrs                        │
    │                          │                           │                        │
    │                          ▼                           │                        │
    │                    Phase 2 (Multitask)  ◄─────────────┘                       │
    │                      10 hrs                                                   │
    │                          │                                                    │
    ▼                          │                                                    │
Phase 3 (Protect)  ◄───────────┘                                                   │
  8 hrs                                                                             │
    │                                                                               ▼
    └──────────────────────────────► Phase 4 (Load) ◄───────────────────────────────┘
                                       6 hrs
                                         │
Phase 5 (Connect)                        │               Phase 6 (Sustain)
  10 hrs                                 │                 8 hrs
    │                                    │                   │
    └────────────────┬───────────────────┘                   │
                     │                                       │
                     ▼                                       │
               Phase 7 (Genesis) ◄───────────────────────────┘
                  6 hrs
```

---

## Complete Summary Table

| # | Phase | Codename | Tasks | Effort | LOC Added | Cumulative |
|---|-------|----------|-------|--------|-----------|------------|
| 0A | CR3/CR2 builtins | — | 6 | 2 hrs | +60 | 10,820 |
| 0B | @interrupt ISR | — | 8 | 5 hrs | +200 | 11,020 |
| 0C | SYSCALL dispatch | — | 6 | 3 hrs | +150 | 11,170 |
| 0D | x86_64-user target | — | 6 | 4 hrs | +300 | 11,470 |
| 1 | Return to Shell | "Return" | 8 | 6 hrs | +200 | 11,670 |
| 2 | Multitasking | "Multitask" | 10 | 10 hrs | +600 | 12,270 |
| 3 | Memory Protection | "Protect" | 8 | 8 hrs | +400 | 12,670 |
| 4 | ELF from Disk | "Load" | 7 | 6 hrs | +300 | 12,970 |
| 5 | Real Network | "Connect" | 8 | 10 hrs | +700 | 13,670 |
| 6 | Persistence + Init | "Sustain" | 8 | 8 hrs | +400 | 14,070 |
| 7 | Production Release | "Genesis" | 8 | 6 hrs | +200 | 14,270 |
| | **TOTAL** | | **95** | **~71 hrs** | **+3,510** | **14,270** |

---

## Session Planning (7 sessions)

```
Session 1:  Phase 0A + 0C          (CR3 builtins + SYSCALL dispatch)          5 hrs
Session 2:  Phase 0B + 0D          (@interrupt ISR + user target)             9 hrs
Session 3:  Phase 1                (Return to Shell)                          6 hrs
Session 4:  Phase 2                (Multitasking)                            10 hrs
Session 5:  Phase 3 + 4            (Memory Protection + ELF Load)            14 hrs
Session 6:  Phase 5 + 6            (Networking + Persistence)                18 hrs
Session 7:  Phase 7                (Release)                                  6 hrs
```

### Accelerated Path (skip non-critical)

If time is limited, the MINIMUM viable path is:
```
Phase 0A + 0C → Phase 1 → Phase 2 → Phase 3 → Phase 7
     5 hrs       6 hrs     10 hrs     8 hrs      6 hrs = 35 hrs

Result: multitasking OS with memory protection, runs on real hardware.
Skip: user ELF loading (M4), networking (M5), persistence (M6).
```

---

## Risk Mitigation

| Risk | Phase | Impact | Mitigation |
|------|-------|--------|------------|
| @interrupt codegen too complex for Cranelift | 0B | M2 blocked | Fallback: raw-byte ISR stub (like SYSCALL stub) |
| CR3 switch causes triple fault | 3 | Kernel crash | Keep kernel entries in ALL page tables, test incrementally |
| Timer ISR re-entrancy | 2 | Corruption | Mask interrupts in ISR (IF=0 on entry, auto by CPU) |
| SYSCALL + timer IRQ race | 1+2 | Corruption | SFMASK masks IF during SYSCALL (already configured) |
| User ELF too complex | 4 | M4 delayed | Use hand-installed programs (already work) |
| TCP state machine bugs | 5 | Networking broken | Start with UDP + ICMP only |
| NVMe write-back data loss | 6 | Filesystem corrupt | Copy-on-write or journal (defer to post-v1.0) |

---

*FajarOS Nova v1.0.0 "Genesis" — from 10,760 to 14,270 LOC*
*95 tasks across 10 phases (4 compiler + 7 OS) in ~71 hours*
*Written 100% in Fajar Lang, built with Claude Opus 4.6*
