# FajarOS Nova v0.6 "Ascension" — Implementation Plan

> **Date:** 2026-03-21
> **Author:** Fajar (PrimeCore.id) + Claude Opus 4.6
> **Context:** Nova v0.5.0 shipped (9,637 LOC, 148 commands, Ring 3, SYSCALL/SYSRET, NVMe+FAT32, VFS, SMP, virtio-net virtqueues, XHCI init, const fn). All 6 fixes from NEXT_SESSION_PLAN complete.
> **Codename:** "Ascension" — the OS that ascends from prototype to production
> **Goal:** Test everything, release v3.5.0, complete USB, improve language, plan v0.6

---

## Current State

```
Fajar Lang:  v3.4.0 — const fn, 6,051 tests, ~152K LOC Rust
Nova:        v0.5.0 "Transcendence" — 9,637 LOC, 365 @kernel fns, 148 commands
Repos:       fajar-lang (monolithic kernel) + fajaros-x86 (35 modular .fj files)
Ring 3:      SYSCALL/SYSRET with "Hello Ring 3!" working
Storage:     NVMe + FAT32 + VFS + RamFS
Network:     Virtio-net virtqueues (TX/RX implemented, needs QEMU testing)
USB:         XHCI init + slot enable + address device (needs testing)
Compiler:    const fn with compile-time evaluation (fib(10)=55)
```

---

## Phase A: Test & Verify in QEMU (3 sprints, 30 tasks)

**Goal:** Verify all Nova features work in QEMU. Fix bugs found during testing.
**Effort:** ~6 hours
**Priority:** HIGHEST — untested code is broken code

### Sprint A1: Boot & Core Verification (10 tasks)

| # | Task | QEMU Command | Expected Result | Status |
|---|------|-------------|-----------------|--------|
| A1.1 | Basic boot (serial) | `make run` | Boot banner + "nova>" prompt | [x] |
| A1.2 | Boot with KVM | `make run-kvm` | Same, faster boot | [x] |
| A1.3 | Boot with VGA | `make run-vga` | VGA text mode, colored banner | [x] |
| A1.4 | Shell commands: help | Type `help` | List 160 commands | [x] |
| A1.5 | Shell commands: uname, uptime, cpuinfo | Type each | Correct output | [x] |
| A1.6 | Shell commands: meminfo, frames, heap | Type each | Memory stats shown | [x] |
| A1.7 | Shell commands: clear, echo hello | Type each | Screen clears, echo works | [x] |
| A1.8 | Shell commands: ps, lspci | Type each | Process list, PCI devices | [x] |
| A1.9 | Keyboard: shift, caps lock, arrows | Press keys | Uppercase, history navigation | [x] |
| A1.10 | Verify serial output matches VGA | Compare serial + VGA | Consistent output | [x] |

### Sprint A2: Storage & Filesystem Verification (10 tasks)

| # | Task | QEMU Command | Expected Result | Status |
|---|------|-------------|-----------------|--------|
| A2.1 | NVMe detection | `make run-nvme` + `nvme` | NVMe controller found | [x] |
| A2.2 | NVMe read/write | `disk_read 0` / `disk_write 0` | Sector R/W works | [x] |
| A2.3 | FAT32 mount | `fat32mount` | FAT32 filesystem mounted | [x] |
| A2.4 | FAT32 list | `fat32ls` | Root directory listing | [x] |
| A2.5 | FAT32 cat | `fat32cat <file>` | File contents shown | [x] |
| A2.6 | FAT32 write | `fatwrite test.txt hello` | File created | [x] |
| A2.7 | FAT32 delete | `fatrm test.txt` | File removed | [x] |
| A2.8 | VFS mounts | `mounts` | /, /dev, /proc, /mnt listed | [x] |
| A2.9 | /dev/random | `devread random` | Random bytes shown | [x] |
| A2.10 | /proc/version | `procversion` | Kernel version string | [x] |

### Sprint A3: Network & USB & Ring 3 Verification (10 tasks)

| # | Task | QEMU Command | Expected Result | Status |
|---|------|-------------|-----------------|--------|
| A3.1 | Virtio-net detect | `make run-net` + `ifconfig` | Real MAC (not fake 52:54:00:12:34:56) | [x] |
| A3.2 | Virtio-net BAR0 | `ifconfig` | BAR0 address shown, "active" | [x] |
| A3.3 | Real ping TX | `ping` | "Packet sent via virtio-net TX" | [x] |
| A3.4 | ICMP reply RX | `ping` | "Reply from 10.0.2.2: time=Xus" OR timeout | [x] |
| A3.5 | ARP cache | `arp` | ARP entries shown after ping | [x] |
| A3.6 | XHCI detect | `make run` with `-device qemu-xhci` + `lsusb` | XHCI controller listed | [x] |
| A3.7 | XHCI init | `usbinit` | "Controller running, N device(s)" | [x] |
| A3.8 | USB device enum | `-device usb-storage,drive=usbdisk` + `usbinit` | Slot enabled, device addressed | [x] |
| A3.9 | Ring 3 hello | Boot with default config | "[RING3]..." in serial | [x] |
| A3.10 | SMP boot | `make run-smp` | Boot with 4 cores, no crash | [x] |

### A-Phase Quality Gate
- [x] All 30 verification tasks checked (30/30 ✅)
- [x] Bug list documented: NVMe hangs at Identify Namespace with real NVMe device (ramdisk fallback works)
- [x] Serial I/O mirroring added (console_putchar → COM1, shell reads COM1)

---

## Phase B: Fajar Lang v3.5.0 Release (1 sprint, 10 tasks)

**Goal:** Ship v3.5.0 with const fn, virtio-net, XHCI, modular fajaros-x86
**Effort:** ~1 hour
**Depends on:** Phase A (testing complete)

### Sprint B1: Release Engineering (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| B1.1 | Version bump | Cargo.toml → 5.1.0 | [x] |
| B1.2 | CHANGELOG update | Add v5.1.0 section with all new features | [x] |
| B1.3 | Update CLAUDE.md | Current stats (6,750+ tests, 290K LOC) | [x] |
| B1.4 | Update Nova banner | v1.0.0 → v1.1.0 "Ascension" in kernel_main() | [x] |
| B1.5 | Update fajaros-x86 README | Version bumped, serial I/O documented | [x] |
| B1.6 | Clippy clean | `cargo clippy -- -D warnings` — zero warnings | [x] |
| B1.7 | Fmt check | `cargo fmt -- --check` — clean | [x] |
| B1.8 | Full test suite | `cargo test --features native` — 6,750+ pass | [x] |
| B1.9 | Git tag | `git tag v5.1.0` on fajar-lang | [x] |
| B1.10 | GitHub release | github.com/fajarkraton/fajar-lang/releases/tag/v5.1.0 | [x] |

### B-Phase Quality Gate
- [x] `cargo test --features native` — 6,750+ pass
- [x] `cargo clippy -- -D warnings` — 0 warnings
- [x] `cargo fmt -- --check` — clean
- [x] CHANGELOG.md updated
- [x] Git tag v5.1.0 created

---

## Phase C: USB Mass Storage Complete (3 sprints, 30 tasks)

**Goal:** Read files from a USB stick in FajarOS Nova via XHCI + SCSI + FAT32
**Effort:** ~8 hours
**Depends on:** Phase A (XHCI verified working in QEMU)

### Sprint C1: Control Transfers + GET_DESCRIPTOR (10 tasks)

**Prerequisite:** XHCI controller running, slot enabled, device addressed

| # | Task | Detail | Status |
|---|------|--------|--------|
| C1.1 | Transfer Ring per endpoint | Allocate 64-TRB ring for EP0 at XHCI_XFER_BUF | [x] |
| C1.2 | Setup TRB | Build 8-byte USB SETUP packet as Setup TRB | [x] |
| C1.3 | Data TRB | Build Data TRB pointing to receive buffer | [x] |
| C1.4 | Status TRB | Build Status TRB (zero-length, direction toggle) | [x] |
| C1.5 | Ring doorbell for EP0 | Doorbell(slot_id, EP0_target=1) | [x] |
| C1.6 | Poll Transfer Event | Wait for Transfer Event TRB on event ring | [x] |
| C1.7 | GET_DESCRIPTOR (device) | bRequest=6, wValue=0x0100, wLength=18 | [x] |
| C1.8 | Parse device descriptor | Extract VID, PID, bDeviceClass, bNumConfigurations | [x] |
| C1.9 | GET_DESCRIPTOR (config) | bRequest=6, wValue=0x0200, wLength=255 | [x] |
| C1.10 | Parse config descriptor | Extract interfaces, endpoints, bInterfaceClass | [x] |

### Sprint C2: USB Mass Storage Detection (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| C2.1 | Find mass storage interface | bInterfaceClass=0x08, bInterfaceSubClass=0x06, bInterfaceProtocol=0x50 | [x] |
| C2.2 | Extract bulk endpoints | Find bulk IN + bulk OUT endpoint addresses | [x] |
| C2.3 | SET_CONFIGURATION | bRequest=9, wValue=1 — activate first config | [x] |
| C2.4 | Configure Endpoint command | XHCI Configure Endpoint with bulk IN/OUT rings | [x] |
| C2.5 | Allocate bulk transfer rings | 64 TRBs each for bulk IN + bulk OUT | [x] |
| C2.6 | SCSI INQUIRY | CBW opcode 0x12 → get device name + type | [x] |
| C2.7 | Parse INQUIRY response | Extract vendor, product, revision strings | [x] |
| C2.8 | SCSI TEST UNIT READY | CBW opcode 0x00 → check device ready | [x] |
| C2.9 | SCSI READ CAPACITY | CBW opcode 0x25 → total sectors + sector size | [x] |
| C2.10 | `lsusb` with details | Show VID:PID, class, speed, capacity | [x] |

### Sprint C3: Bulk-Only Transport + FAT32 Mount (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| C3.1 | CBW build function | 31-byte Command Block Wrapper (signature 0x43425355) | [x] |
| C3.2 | CSW parse function | 13-byte Command Status Wrapper verification | [x] |
| C3.3 | SCSI READ(10) | CBW opcode 0x28: read N sectors from LBA | [x] |
| C3.4 | SCSI WRITE(10) | CBW opcode 0x2A: write N sectors to LBA | [x] |
| C3.5 | Bulk transfer wrapper | Send CBW → data phase → receive CSW | [x] |
| C3.6 | Register as blk_dev 2 | USB mass storage in block device table | [x] |
| C3.7 | `usbread <lba>` command | Read + hex dump single sector from USB | [x] |
| C3.8 | Mount FAT32 from USB | `mount /dev/usb0 /usb` → FAT32 init on blk_dev 2 | [x] |
| C3.9 | `usbls` / `usbcat` commands | List + read files from USB FAT32 | [x] |
| C3.10 | QEMU test: read file from USB | `-drive file=usb.img,if=none,id=usbdisk -device usb-storage,drive=usbdisk` | [x] |

### C-Phase Quality Gate
- [x] `lsusb` shows VID:PID of USB storage device
- [x] `usbinit` enables slot + addresses device + reads descriptor
- [x] SCSI INQUIRY returns device name (vendor + product strings)
- [x] SCSI READ(10) reads sector data
- [x] FAT32 file listing from USB stick in QEMU
- [x] Bug fixed: xhci_configure_bulk_endpoints spurious NoOp removed
- [x] Added: SCSI TEST_UNIT_READY, WRITE(10), usbread/usbls/usbcat commands
- [x] fajaros-x86 modular kernel synced (552→1253 lines)

---

## Phase D: New Language Features (2 sprints, 20 tasks)

**Goal:** Improve Fajar Lang with const arrays, const structs, better errors
**Effort:** ~4 hours
**Depends on:** Phase B (v3.5.0 released)

### Sprint D1: const Arrays & Structs (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| D1.1 | `const TABLE: [i64; 4] = [1, 2, 3, 4]` | Static array in const context | [x] |
| D1.2 | `const TABLE = [0; 256]` | Repeat syntax `[expr; count]` in const | [x] |
| D1.3 | Const array indexing | `const X = TABLE[2]` at compile time | [x] |
| D1.4 | Const array in codegen | Emit as static data in .rodata | [x] |
| D1.5 | `const fn` returning array | `const fn make_table() -> [i64; 4]` | [x] |
| D1.6 | Const struct init | `const ORIGIN = Point { x: 0, y: 0 }` | [x] |
| D1.7 | Const struct field access | `const X = ORIGIN.x` at compile time | [x] |
| D1.8 | Const fn body validation | Error on heap alloc, I/O, mutable ref in const fn | [x] |
| D1.9 | Tests: 10 const array/struct cases | Verify codegen + interpreter | [x] |
| D1.10 | Document: FAJAR_LANG_SPEC.md | const fn + const arrays section | [x] |

### Sprint D2: Error Recovery & Diagnostics (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| D2.1 | Error: non-const op in const fn | Clear error message: "heap allocation not allowed in const fn" | [x] |
| D2.2 | Error: mutable binding in const fn | "mutable variables not allowed in const fn" | [x] |
| D2.3 | Error: non-const fn call in const fn | "function 'X' is not const" | [x] |
| D2.4 | Error: const fn recursion limit | "const fn recursion limit exceeded (128 levels)" | [x] |
| D2.5 | Error: const fn overflow | "arithmetic overflow in const fn evaluation" | [x] |
| D2.6 | Const fn suggestion | When calling non-const fn in const context, suggest adding `const` | [x] |
| D2.7 | Better type mismatch errors | Show expected vs actual type with source location | [x] |
| D2.8 | Unused const warning | Warn when const defined but never used | [x] |
| D2.9 | Tests: 10 error message cases | Verify error output quality | [x] |
| D2.10 | Error codes: CT009-CT013 | New error codes for const fn violations | [x] |

### D-Phase Quality Gate
- [x] `const TABLE: [i64; 4] = [1, 2, 3, 4]` works in codegen
- [x] `const ORIGIN = Point { x: 0, y: 0 }` works
- [x] Non-const operations in const fn produce clear error messages
- [x] All tests pass (5,912 total, 0 regressions)
- [x] FAJAR_LANG_SPEC.md updated
- [x] ERROR_CODES.md updated with CT009-CT013
- [x] ComptimeValue::Struct + Tuple added to comptime evaluator
- [x] 19 new tests (10 const struct/array + 9 error diagnostics)

---

## Phase E: FajarOS Nova v0.6 Architecture (3 sprints, 30 tasks)

**Goal:** Transform Nova from interactive shell to real multitasking OS
**Effort:** ~12 hours
**Depends on:** Phase A (verified), Phase C (USB working)

### Sprint E1: Real Preemptive Scheduler (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| E1.1 | Timer IRQ context switch | LAPIC/PIT fires → save regs → pick next → restore | [x] |
| E1.2 | Per-process kernel stack | Each PID gets 16KB kernel stack at KSTACK_BASE + pid*0x4000 | [x] |
| E1.3 | Context frame struct | 20 registers × 8 = 160 bytes (15 GPRs + IRETQ: RIP,CS,RFLAGS,RSP,SS) | [x] |
| E1.4 | save_context(pid) | Software save for voluntary yield (CR3 + state→READY) | [x] |
| E1.5 | restore_context(pid) | Software restore (set PID, CR3 switch, TSS.RSP0 update) | [x] |
| E1.6 | Round-robin pick_next() | pick_next_process(): scan PIDs for READY/RUNNING | [x] |
| E1.7 | Timer ISR calls scheduler | linker.rs __isr_timer: push 15 GPRs → round-robin → IRETQ | [x] |
| E1.8 | `spawn` command | spawn hello/counter/fib/test/init — named process registry | [x] |
| E1.9 | Multiple processes running | hello_process + counter_process + fibonacci_process + sched_demo | [x] |
| E1.10 | Test: preemption works | 10 integration tests (round-robin, frame layout, interleave, exit) | [x] |

### Sprint E2: Multiple Ring 3 Programs (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| E2.1 | User program registry | PROG_REGISTRY at 0x8B0000: 8 slots × 64 bytes | [x] |
| E2.2 | `install` builtin | prog_install_at + prog_install_counter_v2 + prog_install_fibonacci | [x] |
| E2.3 | `run <name>` command | cmd_run: 4-byte name match against registry | [x] |
| E2.4 | SYS_WRITE from Ring 3 | SYSCALL stub offset 25: lodsb → serial out | [x] |
| E2.5 | SYS_EXIT from Ring 3 | SYSCALL stub offset 51: restore kernel RSP → RET | [x] |
| E2.6 | SYS_GETPID from Ring 3 | SYSCALL stub offset 67: mov rax,[0x6FE00] | [x] |
| E2.7 | hello.elf user program | Slot 0 @ 0x2040000 — "Hello Ring 3!\n" | [x] |
| E2.8 | counter.elf user program | Slot 3 @ 0x20A0000 — loop '1'..'9' via register | [x] |
| E2.9 | fibonacci.elf user program | Slot 4 @ 0x20C0000 — r12/r13 loop, fib(20)=6765 | [x] |
| E2.10 | Test: 3 programs sequential | ring3test + run0/run3/run4 + 10 integration tests | [x] |

### Sprint E3: Persistent Storage + Real Network (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| E3.1 | NVMe write-back | `sync` command flushes dirty FAT32 sectors | [ ] |
| E3.2 | Persistent file test | Write file, reboot, verify file still exists | [ ] |
| E3.3 | FAT32 from NVMe at boot | Auto-mount /mnt/nvme0 if NVMe has FAT32 | [ ] |
| E3.4 | DHCP client (minimal) | Discover → Offer → Request → Ack for IP assignment | [ ] |
| E3.5 | TCP connect (SYN handshake) | 3-way handshake to remote server | [ ] |
| E3.6 | TCP data send/recv | Send HTTP GET, receive response | [ ] |
| E3.7 | `wget` command | Fetch URL via TCP/HTTP → save to FAT32 | [ ] |
| E3.8 | DNS resolver (minimal) | Query 10.0.2.3 (QEMU DNS) for hostname → IP | [ ] |
| E3.9 | `nslookup` command | `nslookup example.com` → IP address | [ ] |
| E3.10 | Network demo | `wget http://10.0.2.2:8080/hello.txt` → save → cat | [ ] |

### E-Phase Quality Gate
- [ ] Timer-driven preemptive scheduling works (2+ processes)
- [ ] 3 Ring 3 user programs run successfully
- [ ] File persistence across reboot (NVMe + FAT32)
- [ ] At least DHCP + ICMP ping with real IP from QEMU
- [ ] All serial + VGA output correct

---

## Dependency Graph

```
Phase A: Test & Verify (6 hrs)
    |
    +---> Phase B: v3.5.0 Release (1 hr)
    |         |
    |         +---> Phase D: Language Features (4 hrs)
    |
    +---> Phase C: USB Mass Storage (8 hrs)
    |
    +---> Phase E: Nova v0.6 Architecture (12 hrs)
              |
              +---> E1: Preemptive Scheduler
              +---> E2: Multiple Ring 3 Programs
              +---> E3: Persistent Storage + Network
```

## Timeline

```
Session 1:  Phase A (Sprint A1-A3)    — Test everything in QEMU
            Phase B (Sprint B1)        — Ship v3.5.0
Session 2:  Phase C (Sprint C1-C2)    — USB control transfers + detection
Session 3:  Phase C (Sprint C3)       — Mass storage BOT + mount
            Phase D (Sprint D1)        — const arrays + structs
Session 4:  Phase D (Sprint D2)        — Error diagnostics
            Phase E (Sprint E1)        — Preemptive scheduler
Session 5:  Phase E (Sprint E2-E3)    — Ring 3 programs + network
```

## Target Metrics

| Metric | Current (v0.5) | Target (v0.6) |
|--------|---------------|---------------|
| Nova LOC | 9,637 | ~13,000 |
| Nova commands | 148 | 165+ |
| Shell commands verified | 0 (untested) | 148 (all tested) |
| User programs in Ring 3 | 1 (hello) | 3+ (hello, counter, fib) |
| Network | Virtqueue impl (untested) | Real ICMP ping verified |
| USB | XHCI init (untested) | Mass storage read/write |
| Preemptive scheduling | None (cooperative) | Timer-driven round-robin |
| Fajar Lang version | v3.4.0 | v3.5.0 |
| Fajar Lang tests | 6,051 | 6,100+ |
| const fn features | Basic (int only) | Arrays + structs + errors |
| Persistent storage | RAM only | NVMe write-back |

## Summary

```
Phase A:  Test & Verify         3 sprints   30 tasks    ~6 hrs    HIGHEST priority
Phase B:  v3.5.0 Release        1 sprint    10 tasks    ~1 hr     After A
Phase C:  USB Mass Storage      3 sprints   30 tasks    ~8 hrs    After A
Phase D:  Language Features     2 sprints   20 tasks    ~4 hrs    After B
Phase E:  Nova v0.6 Arch        3 sprints   30 tasks    ~12 hrs   After A+C

Total:    12 sprints, 120 tasks, ~31 hours
```

---

*Nova v0.6 "Ascension" — from prototype to production*
*Built with Fajar Lang + Claude Opus 4.6*
