# Next Session Implementation Plan — Nova v0.5 "Transcendence"

> **Date:** 2026-03-21
> **Author:** Fajar (PrimeCore.id) + Claude Opus 4.6
> **Context:** Nova v0.4.0 shipped, v3.3.0 shipped, 37 commits in one session
> **Goal:** Real user programs running, real network packets, real USB devices
> **Codename:** "Transcendence" — the OS that crosses the Ring 0/3 boundary

---

## Current State

```
Fajar Lang v3.3.0:  Edition 2024, [0;N] syntax, 30+ OS builtins, fn pointers
Nova v0.4.0:        8,523 LOC, 140 commands, NVMe+FAT32+VFS+SMP+Net+ELF
                    User binary installed at 0x2000000 but NOT yet executing
                    SYSCALL handler NOT wired (LSTAR MSR not configured)
                    Virtio-net TX simulated (packet built but not sent)
                    XHCI detected but no device enumeration
                    NVMe timeout too short for KVM + SMP>4
```

### What's DONE vs What's FAKE

| Feature | Real | Simulated |
|---------|------|-----------|
| NVMe sector R/W | **REAL** (QEMU + KVM verified) | — |
| FAT32 mount/ls/cat/write/rm | **REAL** (persistence verified) | — |
| VFS mount table | **REAL** (4 mounts: /, /dev, /proc, /mnt) | — |
| /dev/null, /dev/zero, /dev/random | **REAL** (rdrand for random) | — |
| SMP AP boot | **REAL** (trampoline installed, IPI sent) | APs enter HLT loop only |
| Network ping | — | **SIMULATED** (packet built, not sent) |
| ELF exec | Segments loaded to memory | **NOT EXECUTING** (no IRETQ yet) |
| Ring 3 user code | Installed at 0x2000000 | **NOT RUNNING** (no SYSCALL handler) |
| USB XHCI | PCI detected | **NO DRIVER** |
| Keyboard via port_inb | Code ready | **NOT WIRED** to IRQ1 handler |

---

## Priority Order (Critical Path First)

```
Fix 1:  NVMe Timeout Tuning         [░░░░░░░░░░]  1 sprint   — 10 min fix, unblocks KVM+SMP
Fix 2:  SYSCALL/SYSRET Wiring       [░░░░░░░░░░]  2 sprints  — THE missing piece for Ring 3
Fix 3:  Real Ring 3 Execution       [░░░░░░░░░░]  2 sprints  — hello.elf actually prints
Fix 4:  Virtio-Net Real TX/RX       [░░░░░░░░░░]  3 sprints  — real packets on wire
Fix 5:  Fajar Lang const fn         [░░░░░░░░░░]  2 sprints  — compile-time eval
Fix 6:  USB Mass Storage             [░░░░░░░░░░]  3 sprints  — read USB stick
```

**Rationale:**
1. NVMe timeout is a 10-minute fix that unblocks all KVM+SMP testing
2. SYSCALL handler is THE critical blocker — without it, Ring 3 programs crash
3. Once SYSCALL works, hello.elf actually runs (the "wow" moment)
4. Virtio-net turns simulated ping into real network packets
5. const fn improves the language for all future work
6. USB is the most complex, saved for last

---

## Fix 1: NVMe Timeout Tuning (1 sprint, 10 tasks)

**Goal:** NVMe works reliably under KVM with SMP 4-24 cores
**Effort:** ~30 minutes
**Why first:** Quick win, unblocks all hardware testing

### Sprint F1: NVMe Polling Fix

| # | Task | Detail | Status |
|---|------|--------|--------|
| F1.1 | Increase admin CQ poll timeout | 1,000,000 → 50,000,000 iterations | [ ] |
| F1.2 | Increase I/O CQ poll timeout | Same increase for I/O path | [ ] |
| F1.3 | Add delay between poll iterations | `pause()` instruction for CPU efficiency | [ ] |
| F1.4 | Test KVM + SMP=4 + NVMe | Verify full init in 10s | [ ] |
| F1.5 | Test KVM + SMP=8 + NVMe | Verify full init in 15s | [ ] |
| F1.6 | Test KVM + SMP=24 + NVMe | Verify at least partial init | [ ] |
| F1.7 | Add NVMe init timing | rdtsc before/after, print elapsed | [ ] |
| F1.8 | Graceful timeout message | Print which step timed out | [ ] |
| F1.9 | Update NOVA_HARDWARE_TEST.md | New KVM results after fix | [ ] |
| F1.10 | CI: KVM test (if available) | Test with `-enable-kvm` in CI | [ ] |

---

## Fix 2: SYSCALL/SYSRET Handler (2 sprints, 20 tasks)

**Goal:** User programs can call the kernel via SYSCALL instruction
**Effort:** ~4 hours (most complex piece)
**Why second:** Required for Ring 3 programs to do anything useful

### Sprint S1: SYSCALL Entry Point (10 tasks)

The x86_64 SYSCALL instruction requires:
- **IA32_STAR** (0xC0000081): kernel/user CS/SS segments
- **IA32_LSTAR** (0xC0000082): kernel entry RIP
- **IA32_SFMASK** (0xC0000083): RFLAGS mask (disable interrupts on entry)
- **IA32_FMASK** same as SFMASK on AMD

| # | Task | Detail | Status |
|---|------|--------|--------|
| S1.1 | GDT layout for SYSCALL/SYSRET | Kernel CS=0x08, SS=0x10, User CS=0x20\|3, SS=0x18\|3 | [ ] |
| S1.2 | Verify GDT has user segments | Check existing GDT at boot for DPL=3 entries | [ ] |
| S1.3 | Add user CS/SS to GDT if missing | 64-bit user code + data segments | [ ] |
| S1.4 | Configure IA32_STAR MSR | `wrmsr(0xC0000081, (0x13 << 48) \| (0x08 << 32))` | [ ] |
| S1.5 | Write SYSCALL entry stub | Assembly: save user RSP/RIP, switch to kernel stack | [ ] |
| S1.6 | Configure IA32_LSTAR MSR | Point to syscall_entry address | [ ] |
| S1.7 | Configure IA32_SFMASK MSR | Mask IF flag (disable interrupts on syscall entry) | [ ] |
| S1.8 | Enable SYSCALL in EFER | Set EFER.SCE (bit 0) via wrmsr | [ ] |
| S1.9 | Kernel stack per-process | RSP0 stored in per-CPU data or TSS | [ ] |
| S1.10 | Test: SYSCALL from Ring 0 | Verify MSR configuration doesn't crash | [ ] |

### Sprint S2: Syscall Dispatch + SYSRET (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| S2.1 | Syscall entry: save registers | Push RAX,RCX,R11,RDI,RSI,RDX,R10,R8,R9 | [ ] |
| S2.2 | Syscall entry: switch to kernel RSP | Load RSP0 from TSS or per-CPU area | [ ] |
| S2.3 | Syscall dispatch by RAX | RAX=syscall number, call handler function | [ ] |
| S2.4 | SYS_WRITE(1): fd, buf, len | Write from user buffer to VGA console | [ ] |
| S2.5 | SYS_EXIT(60): code | Mark process as zombie, return to scheduler | [ ] |
| S2.6 | SYS_GETPID(39): return PID | Read from per-CPU current_pid | [ ] |
| S2.7 | SYS_BRK(12): expand heap | Allocate + map user pages | [ ] |
| S2.8 | SYSRET: restore registers | Pop saved regs, SYSRETQ to user RIP/RSP | [ ] |
| S2.9 | User buffer validation | Check buf address is in user space before reading | [ ] |
| S2.10 | Test: int3 from Ring 3 | Trigger exception in user mode, handle in kernel | [ ] |

### Key Technical Detail

The SYSCALL entry stub must be raw machine code in the kernel binary because Fajar Lang can't express the register save/restore needed. Options:

**Option A:** Write the stub as raw bytes via `volatile_write_u8` (like the AP trampoline)
**Option B:** Add a new builtin `syscall_entry_stub(handler_addr)` that generates the stub
**Option C:** Use the linker.rs assembly section to embed the stub

**Recommended:** Option A (raw bytes) — consistent with how we wrote the AP trampoline. The stub is ~80 bytes:

```
syscall_entry:
    swapgs                      ; switch GS to kernel per-CPU data
    mov [gs:0x10], rsp         ; save user RSP
    mov rsp, [gs:0x08]         ; load kernel RSP
    push rcx                    ; save user RIP (SYSCALL stores in RCX)
    push r11                    ; save user RFLAGS (SYSCALL stores in R11)
    push rdi                    ; arg0
    push rsi                    ; arg1
    push rdx                    ; arg2
    ; RAX = syscall number, dispatch to handler
    call syscall_dispatch
    pop rdx
    pop rsi
    pop rdi
    pop r11
    pop rcx
    mov rsp, [gs:0x10]         ; restore user RSP
    swapgs                      ; switch back to user GS
    sysretq                     ; return to user (RCX=RIP, R11=RFLAGS)
```

---

## Fix 3: Real Ring 3 Execution (2 sprints, 20 tasks)

**Goal:** `runhello` command actually prints "Hello Ring 3!" from user space
**Depends on:** Fix 2 (SYSCALL handler)

### Sprint R1: IRETQ to User Mode (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| R1.1 | TSS setup | 104-byte TSS with RSP0 for kernel stack | [ ] |
| R1.2 | Load TSS via `ltr` | TR register points to TSS descriptor in GDT | [ ] |
| R1.3 | Verify GDT user segments | Confirm DPL=3 code (0x20) and data (0x18) | [ ] |
| R1.4 | IRETQ stack frame | Push SS, RSP, RFLAGS, CS, RIP for user mode | [ ] |
| R1.5 | Map user pages with PAGE_USER | Code + stack pages have PTE.U bit set | [ ] |
| R1.6 | Set RSP to user stack | 0x2FF0FF0 (top of user stack page) | [ ] |
| R1.7 | Call `iretq_to_user(entry, user_rsp, 0x202)` | Transition to Ring 3 | [ ] |
| R1.8 | User code executes SYS_WRITE | "Hello Ring 3!\n" appears on VGA | [ ] |
| R1.9 | User code executes SYS_EXIT | Returns to kernel cleanly | [ ] |
| R1.10 | Verify: no triple fault | Clean transition + return cycle | [ ] |

### Sprint R2: Multiple User Programs (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| R2.1 | Second user program | "Goodbye Ring 3!\n" | [ ] |
| R2.2 | Third user program | Counter: "Count: 1\nCount: 2\n..." | [ ] |
| R2.3 | Run programs sequentially | hello → goodbye → counter | [ ] |
| R2.4 | ELF from FAT32 → exec → output | Full pipeline working | [ ] |
| R2.5 | User heap via SYS_BRK | Allocate memory in user space | [ ] |
| R2.6 | Page fault handler | Catch invalid user access, kill process | [ ] |
| R2.7 | GPF handler for Ring 3 | Privilege violation → process killed | [ ] |
| R2.8 | Process exit code visible | `wait` command shows exit code | [ ] |
| R2.9 | Shell: `run <file>` | Load ELF from FAT32, exec in Ring 3 | [ ] |
| R2.10 | CI: user program test | Verify "Hello Ring 3!" in serial output | [ ] |

---

## Fix 4: Virtio-Net Real TX/RX (3 sprints, 30 tasks)

**Goal:** `ping 10.0.2.2` sends a real ICMP packet and receives a reply
**Depends on:** Port I/O builtins (already done)
**Effort:** ~6 hours

### Sprint V1: Virtio Device Init (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| V1.1 | Virtio PCI legacy BAR0 | Read I/O port base from BAR0 | [ ] |
| V1.2 | Device status negotiation | ACKNOWLEDGE → DRIVER → FEATURES_OK → DRIVER_OK | [ ] |
| V1.3 | Feature bits read/write | VIRTIO_NET_F_MAC, F_STATUS | [ ] |
| V1.4 | Queue size query | Read from BAR0+0x0C (max queue entries) | [ ] |
| V1.5 | Allocate virtqueue memory | Descriptors + Available + Used rings | [ ] |
| V1.6 | Queue address notify | Write physical address to BAR0+0x08 | [ ] |
| V1.7 | MAC address read | 6 bytes from BAR0+0x14..0x19 | [ ] |
| V1.8 | Interrupt enable | IRQ via PCI interrupt line | [ ] |
| V1.9 | Device ready | Write DRIVER_OK to status register | [ ] |
| V1.10 | Test: `ifconfig` shows real MAC | Not fake 52:54:00:12:34:56 | [ ] |

### Sprint V2: Packet TX/RX (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| V2.1 | TX: fill descriptor | Physical addr + length + flags | [ ] |
| V2.2 | TX: add to available ring | Write descriptor index to avail ring | [ ] |
| V2.3 | TX: kick device | Write queue index to BAR0+0x10 | [ ] |
| V2.4 | TX: check used ring | Device marks descriptor as consumed | [ ] |
| V2.5 | RX: pre-fill descriptors | Allocate RX buffers, add to avail ring | [ ] |
| V2.6 | RX: interrupt handler | Read used ring, process received packet | [ ] |
| V2.7 | Virtio-net header | 10-byte header prepended to each packet | [ ] |
| V2.8 | Send raw ethernet frame | Build frame + virtio header, TX | [ ] |
| V2.9 | Receive ethernet frame | Parse ethertype, dispatch to ARP/IP | [ ] |
| V2.10 | Test: send ARP request | See ARP on host via tcpdump | [ ] |

### Sprint V3: Real ICMP Ping (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| V3.1 | ARP request → reply | Send "who has 10.0.2.2?", receive MAC | [ ] |
| V3.2 | ARP cache update | Store gateway MAC from reply | [ ] |
| V3.3 | ICMP echo request via TX | Build IP+ICMP packet, send to gateway | [ ] |
| V3.4 | ICMP echo reply via RX | Receive and parse ping reply | [ ] |
| V3.5 | RTT measurement | rdtsc before TX, after RX, compute ms | [ ] |
| V3.6 | Shell: `ping` with real output | "Reply from 10.0.2.2: time=Xms" | [ ] |
| V3.7 | QEMU TAP networking | `-netdev tap` for host-visible packets | [ ] |
| V3.8 | Packet statistics | Real RX/TX counters | [ ] |
| V3.9 | Error handling | Timeout, no reply, ARP failure | [ ] |
| V3.10 | CI: ping test | Verify ping succeeds in QEMU | [ ] |

---

## Fix 5: Fajar Lang const fn (2 sprints, 20 tasks)

**Goal:** `const fn` functions evaluated at compile time
**Effort:** ~4 hours

### Sprint C1: const fn Parser + Evaluator (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| C1.1 | `const fn` declaration | Parser: `const fn add(a: i64, b: i64) -> i64 { a + b }` | [ ] |
| C1.2 | AST: `is_const` flag on FnDef | Mark function as compile-time evaluable | [ ] |
| C1.3 | Const fn body validation | Only allow: arithmetic, bitwise, if/match, const calls | [ ] |
| C1.4 | Const fn call in const context | `const X = const_add(1, 2)` evaluates at compile time | [ ] |
| C1.5 | Recursive const fn | `const fn fib(n) = if n <= 1 { n } else { fib(n-1) + fib(n-2) }` | [ ] |
| C1.6 | Const fn with arrays | `const fn make_table() -> [i64; 4]` | [ ] |
| C1.7 | Error: non-const op in const fn | heap allocation, I/O, etc. → compile error | [ ] |
| C1.8 | Const fn in @kernel | Lookup tables computed at compile time | [ ] |
| C1.9 | Tests: 10 const fn cases | Arithmetic, recursive, conditional | [ ] |
| C1.10 | Document: FAJAR_LANG_SPEC.md | const fn section | [ ] |

### Sprint C2: const Arrays + Structs (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| C2.1 | `const TABLE: [i64; 4] = [1, 2, 3, 4]` | Static array at compile time | [ ] |
| C2.2 | `const TABLE = [0; 256]` | Repeat syntax in const context | [ ] |
| C2.3 | Const struct init | `const ORIGIN = Point { x: 0, y: 0 }` | [ ] |
| C2.4 | Const indexing | `const X = TABLE[2]` | [ ] |
| C2.5 | Const in codegen | Emit as static data in .rodata section | [ ] |
| C2.6 | Const fn call result as const | `const RESULT = const_compute(42)` | [ ] |
| C2.7 | Error: mutable in const | `const fn bad() { let mut x = 0 }` → error | [ ] |
| C2.8 | Const fn across modules | Import const fn from other file | [ ] |
| C2.9 | Tests: 10 const array/struct cases | Verify .rodata placement | [ ] |
| C2.10 | Version bump to v3.4.0 | Cargo.toml + CHANGELOG + tag | [ ] |

---

## Fix 6: USB Mass Storage (3 sprints, 30 tasks)

**Goal:** Read files from a USB stick in FajarOS Nova
**Effort:** ~8 hours (most complex)
**Depends on:** Port I/O builtins, MMIO mapping

### Sprint U1: XHCI Controller Init (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| U1.1 | XHCI BAR0 MMIO mapping | Map capability + operational registers | [ ] |
| U1.2 | Read CAPLENGTH | Operational regs at BAR0 + CAPLENGTH | [ ] |
| U1.3 | Read HCSPARAMS1 | Max slots, max interrupts, max ports | [ ] |
| U1.4 | Controller halt + reset | USBCMD.RS=0, wait USBSTS.HCH=1, USBCMD.HCRST=1 | [ ] |
| U1.5 | DCBAA allocation | Device Context Base Address Array (MaxSlots × 8) | [ ] |
| U1.6 | Command ring allocation | 256 TRBs × 16 bytes, set CRCR register | [ ] |
| U1.7 | Event ring allocation | ERST + event ring segment | [ ] |
| U1.8 | Interrupter setup | Set IMAN, IMOD, ERSTBA, ERSTSZ | [ ] |
| U1.9 | Controller run | Set USBCMD.RS=1 | [ ] |
| U1.10 | Test: XHCI running on QEMU | `-device qemu-xhci` + USBSTS.HCH=0 | [ ] |

### Sprint U2: Device Enumeration (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| U2.1 | Port status check | Read PORTSC for each port | [ ] |
| U2.2 | Port reset | Set PORTSC.PR=1, wait for reset complete | [ ] |
| U2.3 | Enable Slot command | TRB: Enable Slot → get slot ID | [ ] |
| U2.4 | Address Device command | Set device address, allocate input context | [ ] |
| U2.5 | Get Device Descriptor | Control transfer: GET_DESCRIPTOR (device) | [ ] |
| U2.6 | Parse device descriptor | VID/PID, class, num configurations | [ ] |
| U2.7 | Get Configuration Descriptor | Read interfaces + endpoints | [ ] |
| U2.8 | Set Configuration | Activate first configuration | [ ] |
| U2.9 | Shell: `lsusb` with details | Show VID:PID, class, speed | [ ] |
| U2.10 | Test: enumerate USB keyboard | QEMU `-device usb-kbd` | [ ] |

### Sprint U3: Mass Storage BOT (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| U3.1 | Find mass storage interface | Class 0x08, subclass 0x06, protocol 0x50 | [ ] |
| U3.2 | Bulk-Only Transport (BOT) | CBW (Command Block Wrapper) build | [ ] |
| U3.3 | SCSI INQUIRY command | Get device name + type | [ ] |
| U3.4 | SCSI READ CAPACITY | Get total sectors + sector size | [ ] |
| U3.5 | SCSI READ(10) command | Read sectors from USB drive | [ ] |
| U3.6 | SCSI WRITE(10) command | Write sectors to USB drive | [ ] |
| U3.7 | CSW (Command Status Wrapper) | Parse status after each command | [ ] |
| U3.8 | Register as blk_dev 2 | USB mass storage in block device table | [ ] |
| U3.9 | Mount FAT32 from USB | `mount /dev/usb0 /usb` | [ ] |
| U3.10 | Test: read file from USB stick | QEMU `-drive file=usb.img,if=none,id=usb0 -device usb-storage,drive=usb0` | [ ] |

---

## Dependency Graph

```
Fix 1: NVMe Timeout (30 min)
    │
    ▼
Fix 2: SYSCALL Handler (4 hrs) ←── CRITICAL PATH
    │
    ▼
Fix 3: Ring 3 Execution (3 hrs)
    │
    ├──► Fix 4: Virtio-Net (6 hrs) ──► Real ping working
    │
    └──► Fix 5: const fn (4 hrs) ──► Better language
              │
              ▼
         Fix 6: USB (8 hrs) ──► Read USB stick
```

## Timeline

```
Hour 1:     Fix 1 (NVMe timeout)     — quick win
Hours 2-5:  Fix 2 (SYSCALL handler)   — most critical
Hours 5-8:  Fix 3 (Ring 3 execution)  — "wow" moment
Hours 8-14: Fix 4 (Virtio-net)        — real networking
Hours 14-18: Fix 5 (const fn)         — language improvement
Hours 18-26: Fix 6 (USB mass storage) — hardware expansion
```

## Target Metrics

| Metric | Current (v0.4) | Target (v0.5) |
|--------|---------------|---------------|
| Nova LOC | 8,523 | ~11,000 |
| Nova commands | 140 | 155+ |
| User programs | 0 running | 3+ running in Ring 3 |
| Network | Simulated | Real virtio-net TX/RX |
| USB | PCI detected | Mass storage read/write |
| NVMe + KVM + SMP | SMP≤4 | SMP≤24 |
| Fajar Lang version | v3.3.0 | v3.4.0 (const fn) |

## Quality Gates

**Fix 1 Gate:** KVM + SMP=8 + NVMe → full boot
**Fix 2 Gate:** `int3` from Ring 3 → handled by kernel (no triple fault)
**Fix 3 Gate:** "Hello Ring 3!" appears on VGA from user program
**Fix 4 Gate:** `ping 10.0.2.2` → real ICMP reply with RTT
**Fix 5 Gate:** `const fn fib(10)` → 55 at compile time
**Fix 6 Gate:** `fatls` on USB FAT32 stick → file listing

---

*Nova v0.5 "Transcendence" — the OS that truly crosses the Ring 0/3 boundary.*
