# V3.0 "Surya" — FajarOS: Production-Ready Operating System

> **Vision:** The world's first operating system written 100% in a single language
> that natively unifies kernel safety, hardware drivers, and AI inference —
> where `@kernel`, `@device`, and `@safe` contexts are enforced by the compiler,
> not by convention.
>
> **Target Hardware:** Radxa Dragon Q6A (Qualcomm QCS6490)
> **Language:** 100% Fajar Lang (inline asm for hardware registers only)
> **Codename:** "Surya" (Indonesian: sun — evolution from "Dawn")

---

## Overview

| Property | Value |
|----------|-------|
| **Codename** | "Surya" — FajarOS, the sun that follows dawn |
| **Board** | Radxa Dragon Q6A (QCS6490) |
| **Architecture** | Microkernel + Fajar Lang userspace |
| **Kernel Language** | 100% Fajar Lang (@kernel context) |
| **Driver Language** | 100% Fajar Lang (@kernel context) |
| **Service Language** | 100% Fajar Lang (@safe context) |
| **AI Language** | 100% Fajar Lang (@device context) |
| **Phases** | 10 |
| **Sprints** | 42 |
| **Tasks** | 420 |
| **Estimated LOC** | ~60,000 Fajar Lang + ~5,000 compiler additions |

### Reference Documents

| Document | Purpose |
|----------|---------|
| `docs/V30_PLAN.md` | **THIS FILE** — Master implementation plan |
| `docs/V30_WORKFLOW.md` | Sprint cycle, quality gates, testing strategy |
| `docs/V30_RULES.md` | OS-specific coding rules, kernel safety invariants |
| `docs/V30_SKILLS.md` | Technical patterns: aarch64 boot, MMU, GIC, QUP, TLMM, FastRPC |
| `docs/RADXA_Q6A_HARDWARE.md` | Hardware specification reference |
| `docs/Q6A_APP_DEV.md` | NPU/GPU/GPIO application development reference |
| `docs/Q6A_LOW_LEVEL_DEV.md` | Boot chain, EDL, SPI firmware, kernel build |
| `docs/Q6A_HARDWARE_USE.md` | Power, storage, pinout, display, camera, audio, RTC |
| `docs/Q6A_ACCESSORIES.md` | Camera modules, displays, storage, PoE HAT |
| `src/bsp/dragon_q6a.rs` | BSP module (73 tests) |

---

## Architecture

### System Layers

```
┌─────────────────────────────────────────────────────────────────┐
│  Layer 4: Applications (@safe)                    ~5,000 LOC    │
│  FajarOS Shell (fjsh), REPL, package manager, AI demo apps     │
├─────────────────────────────────────────────────────────────────┤
│  Layer 3: OS Services (@safe + @device)          ~20,000 LOC    │
│  Init, VFS, TCP/IP stack, display compositor,                   │
│  NPU inference daemon, GPU compute service                      │
├─────────────────────────────────────────────────────────────────┤
│  Layer 2: HAL Drivers (@kernel)                  ~15,000 LOC    │
│  TLMM/GPIO, QUP/UART/SPI/I2C, GICv3, PCIe/NVMe,              │
│  RGMII/Ethernet, Adreno 643, Hexagon 770, MIPI CSI/DSI        │
├─────────────────────────────────────────────────────────────────┤
│  Layer 1: Microkernel (@kernel)                   ~8,000 LOC    │
│  UEFI boot, aarch64 MMU, EL1 exceptions, scheduler,            │
│  IPC (message passing), memory allocator, syscall dispatch      │
├─────────────────────────────────────────────────────────────────┤
│  Layer 0.5: Compiler Support                      ~5,000 LOC    │
│  aarch64-none target, no-std runtime, bare-metal linker,        │
│  asm!() register constraints, volatile codegen                  │
├─────────────────────────────────────────────────────────────────┤
│  Layer 0: Hardware — Qualcomm QCS6490                           │
│  Kryo 670 (8-core), Adreno 643, Hexagon 770 (12 TOPS),        │
│  LPDDR5 16GB, NVMe, GbE, WiFi 6, 40-pin GPIO, 3x MIPI CSI    │
└─────────────────────────────────────────────────────────────────┘
```

### QCS6490 Memory Map

```
0x0000_0000 — 0x0FFF_FFFF    Peripheral MMIO (256MB)
  ├── 0x0F10_0000             TLMM (GPIO mux)
  ├── 0x0A8C_0000             QUP (UART/SPI/I2C engines)
  ├── 0x1780_0000             GICv3 Distributor
  ├── 0x17A0_0000             GICv3 Redistributor
  ├── 0x0100_0000             PCIe controller
  ├── 0x3D00_0000             Adreno 643 GPU
  └── 0x0B00_0000             Hexagon 770 (CDSP)

0x4000_0000 — 0x7FFF_FFFF    Kernel Space (1GB)
  ├── 0x4000_0000             Kernel .text (16MB)
  ├── 0x4100_0000             Kernel .data + .bss (16MB)
  ├── 0x4200_0000             Kernel heap (64MB)
  ├── 0x4600_0000             Page tables (16MB)
  ├── 0x4700_0000             Kernel stacks (16MB)
  └── 0x4800_0000             DMA buffers (128MB)

0x8000_0000 — 0x3_FFFF_FFFF  User Space (up to 15GB)
  ├── 0x8000_0000             Process code + data
  ├── 0x1_0000_0000           Shared libraries
  ├── 0x2_0000_0000           GPU buffer pool
  └── 0x3_0000_0000           NPU buffer pool
```

### Boot Sequence

```
Power On
  │
  ▼
SPI NOR Flash
  ├── XBL (eXtensible Bootloader)
  ├── DEVCFG
  ├── DDR training
  └── UEFI firmware
  │
  ▼
UEFI BIOS
  ├── Hardware init (CPU, LPDDR5, PCIe, USB)
  ├── Boot device selection (NVMe > eMMC > SD)
  └── Load FajarOS kernel EFI binary
  │
  ▼
FajarOS Kernel Entry (@kernel _start)
  ├── 1. Disable interrupts (DAIF)
  ├── 2. Set stack pointer
  ├── 3. Initialize MMU (4KB pages, 48-bit VA)
  ├── 4. Set exception vectors (VBAR_EL1)
  ├── 5. Initialize GICv3
  ├── 6. Initialize kernel heap
  ├── 7. Enable interrupts
  ├── 8. Start scheduler
  └── 9. Spawn init process
  │
  ▼
Init Process (@safe)
  ├── Mount root filesystem
  ├── Start device manager (enumerate drivers)
  ├── Start network services
  ├── Start display compositor
  ├── Start NPU daemon
  └── Start shell (fjsh)
```

### Context Isolation in FajarOS

```
@kernel context (Ring 0 equivalent)
  ├── CAN:  inline asm, volatile I/O, raw pointers, page tables, IRQ
  ├── CANNOT: heap allocation (use kernel_alloc), tensor ops, network I/O
  ├── COMPILED TO: EL1 privileged instructions
  └── USED BY: microkernel, HAL drivers

@device context (Compute accelerator)
  ├── CAN: tensor ops, GPU dispatch, NPU inference, DMA buffers
  ├── CANNOT: raw pointers, IRQ, syscall, volatile I/O
  ├── COMPILED TO: regular aarch64 + compute dispatch
  └── USED BY: AI subsystem, GPU services

@safe context (Userspace)
  ├── CAN: standard operations, heap, strings, collections
  ├── CANNOT: raw pointers, IRQ, volatile I/O, direct tensor
  ├── COMPILED TO: regular aarch64, syscall for privileged ops
  └── USED BY: services, shell, applications
```

### Syscall Interface

```
Number  Name                    Context    Description
──────  ──────────────────────  ─────────  ──────────────────────────────
0x00    SYS_EXIT                @safe      Terminate process
0x01    SYS_WRITE               @safe      Write to file descriptor
0x02    SYS_READ                @safe      Read from file descriptor
0x03    SYS_OPEN                @safe      Open file
0x04    SYS_CLOSE               @safe      Close file descriptor
0x05    SYS_MMAP                @safe      Map memory pages
0x06    SYS_MUNMAP              @safe      Unmap memory pages
0x07    SYS_SPAWN               @safe      Create new process
0x08    SYS_WAIT                @safe      Wait for process
0x09    SYS_IPC_SEND            @safe      Send IPC message
0x0A    SYS_IPC_RECV            @safe      Receive IPC message
0x0B    SYS_IPC_CALL            @safe      Synchronous IPC call
0x0C    SYS_IRQ_REGISTER        @kernel    Register IRQ handler
0x0D    SYS_IRQ_ACK             @kernel    Acknowledge interrupt
0x0E    SYS_DMA_ALLOC           @kernel    Allocate DMA buffer
0x0F    SYS_DMA_FREE            @kernel    Free DMA buffer
0x10    SYS_GPIO_CONFIG         @kernel    Configure GPIO pin
0x11    SYS_GPIO_WRITE          @kernel    Write GPIO pin
0x12    SYS_GPIO_READ           @kernel    Read GPIO pin
0x13    SYS_UART_WRITE          @kernel    Write UART data
0x14    SYS_UART_READ           @kernel    Read UART data
0x15    SYS_SPI_TRANSFER        @kernel    SPI full-duplex transfer
0x16    SYS_I2C_TRANSFER        @kernel    I2C read/write
0x17    SYS_NPU_LOAD            @device    Load model to NPU
0x18    SYS_NPU_INFER           @device    Run NPU inference
0x19    SYS_NPU_UNLOAD          @device    Unload NPU model
0x1A    SYS_GPU_DISPATCH        @device    Dispatch GPU compute kernel
0x1B    SYS_GPU_SYNC            @device    Wait for GPU completion
0x1C    SYS_NET_SOCKET          @safe      Create network socket
0x1D    SYS_NET_BIND            @safe      Bind socket
0x1E    SYS_NET_LISTEN          @safe      Listen for connections
0x1F    SYS_NET_ACCEPT          @safe      Accept connection
0x20    SYS_NET_SEND            @safe      Send data
0x21    SYS_NET_RECV            @safe      Receive data
0x22    SYS_TIME                @safe      Get system time
0x23    SYS_SLEEP               @safe      Sleep for duration
0x24    SYS_CAM_CAPTURE         @device    Capture camera frame
0x25    SYS_DISPLAY_FB          @safe      Write to framebuffer
```

---

## Phase 1: Compiler Bare-Metal Support (Sprints 1-4)

> **Goal:** Enable Fajar Lang to compile code that runs without an OS.
> **Gate:** `fj build --target aarch64-none` produces a working bare-metal ELF that boots in QEMU.

### Sprint 1: aarch64-none Target

| # | Task | Status |
|---|------|--------|
| 1.1 | Add `BareMetalAarch64` variant to `BspArch` enum | [x] |
| 1.2 | Register `aarch64-unknown-none` target triple in Cranelift codegen | [x] |
| 1.3 | Create `TargetConfig` struct: no-std, no-libc, static linking | [x] |
| 1.4 | Modify `ObjectCompiler` to emit bare-metal ELF (no dynamic linking) | [x] |
| 1.5 | Disable libc-dependent runtime functions when target is bare-metal | [x] |
| 1.6 | Add `--target aarch64-none` CLI flag to `fj build` | [x] |
| 1.7 | Generate minimal `.text` + `.data` + `.bss` sections | [x] |
| 1.8 | Test: compile empty `@kernel fn _start() {}` → valid aarch64 ELF | [x] |
| 1.9 | Test: QEMU `-M virt -cpu cortex-a76 -kernel fajaros.elf` boots | [x] |
| 1.10 | Document bare-metal target in `docs/BARE_METAL.md` | [x] |

### Sprint 2: No-Std Runtime

| # | Task | Status |
|---|------|--------|
| 2.1 | Create `src/codegen/cranelift/runtime_bare.rs` — no-libc runtime | [x] |
| 2.2 | Implement `fj_rt_memcpy` without libc (byte-by-byte + word-aligned) | [x] |
| 2.3 | Implement `fj_rt_memset` without libc | [x] |
| 2.4 | Implement `fj_rt_memcmp` without libc | [x] |
| 2.5 | Implement `fj_rt_print_bare` → UART output (memory-mapped) | [x] |
| 2.6 | Implement `fj_rt_panic_bare` → print message + halt (wfe loop) | [x] |
| 2.7 | Implement `fj_rt_alloc_bare` → bump allocator (kernel heap) | [x] |
| 2.8 | Implement `fj_rt_free_bare` → no-op for bump (freelist for later) | [x] |
| 2.9 | Test: bare-metal binary with string operations runs in QEMU | [x] |
| 2.10 | Test: `println("Hello from FajarOS")` outputs to QEMU serial | [x] |

### Sprint 3: Assembly Enhancements

| # | Task | Status |
|---|------|--------|
| 3.1 | Extend `asm!()` parser: `in(reg)`, `out(reg)`, `inout(reg)` constraints | [x] |
| 3.2 | Extend `asm!()` parser: named register `in("x0")`, `out("x1")` | [x] |
| 3.3 | Extend `asm!()` parser: `lateout(reg)` for clobbered outputs | [x] |
| 3.4 | Implement Cranelift codegen for register-constrained inline asm | [x] |
| 3.5 | Add `volatile_read<T>(addr: u64) -> T` as compiler intrinsic | [x] |
| 3.6 | Add `volatile_write<T>(addr: u64, value: T)` as compiler intrinsic | [x] |
| 3.7 | Add memory barrier intrinsics: `dmb()`, `dsb()`, `isb()` | [x] |
| 3.8 | Add `wfe()`, `wfi()`, `sev()` intrinsics for power management | [x] |
| 3.9 | Test: read/write MMIO registers in QEMU `-M virt` PL011 UART | [x] |
| 3.10 | Test: inline asm with register constraints compiles correctly | [x] |

### Sprint 4: Bare-Metal Linker & EFI

| # | Task | Status |
|---|------|--------|
| 4.1 | Generate bare-metal linker script: ENTRY(_start), kernel memory layout | [x] |
| 4.2 | Generate `.text` at 0x4000_0000, `.data`, `.bss`, `.rodata` sections | [x] |
| 4.3 | Generate stack setup: 64KB kernel stack at top of kernel region | [x] |
| 4.4 | Implement `_start` → zero BSS → set SP → call kernel_main | [x] |
| 4.5 | Add EFI binary format output for UEFI boot on Dragon Q6A | [x] |
| 4.6 | Implement EFI entry: `efi_main(image_handle, system_table)` | [x] |
| 4.7 | EFI: exit boot services, get memory map, jump to kernel | [x] |
| 4.8 | Test: bare-metal ELF boots in QEMU with serial output | [x] |
| 4.9 | Test: EFI binary boots in QEMU with OVMF firmware | [x] |
| 4.10 | Integration test: full bare-metal pipeline (compile → QEMU → output) | [x] |

**Phase 1 Gate:**
- [x] `fj build --target aarch64-none kernel.fj` produces valid ELF
- [x] ELF boots in QEMU, prints to serial console
- [x] EFI binary boots in QEMU with OVMF
- [x] All 40 tasks pass, 0 regressions in existing tests
- [x] `volatile_read/write` and `asm!()` with constraints work

---

## Phase 2: Microkernel (Sprints 5-10)

> **Goal:** Boot FajarOS on QEMU aarch64 with MMU, exceptions, scheduler, and IPC.
> **Gate:** Two processes communicate via IPC, with preemptive scheduling, on QEMU.

### Sprint 5: UEFI Boot & Early Init

| # | Task | Status |
|---|------|--------|
| 5.1 | Implement `@kernel fn kernel_main()` — FajarOS entry after EFI handoff | [x] |
| 5.2 | Parse UEFI memory map → identify usable RAM regions | [x] |
| 5.3 | Initialize early serial console (PL011 on QEMU, QUP on Q6A) | [x] |
| 5.4 | Print boot banner: "FajarOS v3.0 Surya — Qualcomm QCS6490" | [x] |
| 5.5 | Detect CPU: read MIDR_EL1 → identify Kryo 670 cores | [x] |
| 5.6 | Detect memory: UEFI memory map → total available RAM | [x] |
| 5.7 | Initialize kernel bump allocator from UEFI-free regions | [x] |
| 5.8 | Set up kernel stack (64KB, aligned to 16 bytes) | [x] |
| 5.9 | Test: QEMU boot → banner → CPU info → memory info | [x] |
| 5.10 | Test: kernel allocator works (alloc + use + verify) | [x] |

### Sprint 6: aarch64 MMU

| # | Task | Status |
|---|------|--------|
| 6.1 | Implement 4-level page table (L0→L1→L2→L3, 4KB granule) | [x] |
| 6.2 | Define page table entry format (AP, AF, SH, AttrIndx, valid) | [x] |
| 6.3 | Implement `page_table_create()` → allocate and zero L0 table | [x] |
| 6.4 | Implement `page_map(table, vaddr, paddr, size, attrs)` | [x] |
| 6.5 | Implement `page_unmap(table, vaddr, size)` | [x] |
| 6.6 | Set MAIR_EL1: Normal memory (WB), Device memory (nGnRnE) | [x] |
| 6.7 | Set TCR_EL1: 48-bit VA, 4KB granule, TTBR0 for kernel | [x] |
| 6.8 | Identity-map kernel region + MMIO region | [x] |
| 6.9 | Enable MMU: set SCTLR_EL1.M=1, I=1, C=1 + ISB | [x] |
| 6.10 | Test: MMU enabled, kernel runs with virtual addresses in QEMU | [x] |

### Sprint 7: Exception Handling

| # | Task | Status |
|---|------|--------|
| 7.1 | Create exception vector table (16 entries, 128-byte aligned) | [x] |
| 7.2 | Implement vector stubs: save all 31 GP registers + SP + ELR + SPSR | [x] |
| 7.3 | Implement `sync_exception_handler(esr, elr, far)` dispatcher | [x] |
| 7.4 | Handle SVC #0 → syscall dispatch (ESR_EL1.EC = 0x15) | [x] |
| 7.5 | Handle data abort → page fault (ESR_EL1.EC = 0x24/0x25) | [x] |
| 7.6 | Handle instruction abort → panic with context dump | [x] |
| 7.7 | Implement `irq_handler()` → dispatch to registered handlers | [x] |
| 7.8 | Set VBAR_EL1 to exception vector table address | [x] |
| 7.9 | Test: SVC #0 → handler → return works in QEMU | [x] |
| 7.10 | Test: invalid memory access → data abort → panic with context | [x] |

### Sprint 8: GICv3 Interrupt Controller

| # | Task | Status |
|---|------|--------|
| 8.1 | Initialize GIC Distributor (GICD): disable all, set groups | [x] |
| 8.2 | Initialize GIC Redistributor (GICR): wake up, clear pending | [x] |
| 8.3 | Initialize CPU interface (ICC_*): set PMR, enable group 1 | [x] |
| 8.4 | Implement `gic_enable_irq(irq_num, priority)` | [x] |
| 8.5 | Implement `gic_disable_irq(irq_num)` | [x] |
| 8.6 | Implement `gic_ack_irq() -> irq_num` (read IAR1) | [x] |
| 8.7 | Implement `gic_eoi(irq_num)` (write EOIR1) | [x] |
| 8.8 | Wire GIC IRQ → exception handler → driver callback | [x] |
| 8.9 | Test: timer IRQ fires and is handled correctly in QEMU | [x] |
| 8.10 | Test: multiple IRQs with different priorities | [x] |

### Sprint 9: Scheduler

| # | Task | Status |
|---|------|--------|
| 9.1 | Define `Process` struct: pid, state, page_table, registers, stack | [x] |
| 9.2 | Define `ProcessState` enum: Ready, Running, Blocked, Terminated | [x] |
| 9.3 | Implement process creation: allocate stack, set entry point | [x] |
| 9.4 | Implement context switch: save/restore all registers + TTBR0 | [x] |
| 9.5 | Implement round-robin scheduler with ready queue | [x] |
| 9.6 | Implement priority scheduler (8 priority levels, 0=highest) | [x] |
| 9.7 | Implement preemptive scheduling via timer IRQ (10ms quantum) | [x] |
| 9.8 | Implement `yield()` syscall for cooperative scheduling | [x] |
| 9.9 | Test: 3 processes round-robin, each prints its PID | [x] |
| 9.10 | Test: high-priority process preempts low-priority process | [x] |

### Sprint 10: IPC (Inter-Process Communication)

| # | Task | Status |
|---|------|--------|
| 10.1 | Define IPC message format: sender, receiver, type, payload (256 bytes) | [x] |
| 10.2 | Implement synchronous `ipc_send(dest_pid, msg)` — blocks until received | [x] |
| 10.3 | Implement synchronous `ipc_recv(src_pid) -> msg` — blocks until message | [x] |
| 10.4 | Implement `ipc_call(dest_pid, msg) -> reply` — send + wait for reply | [x] |
| 10.5 | Implement `ipc_reply(msg)` — reply to caller | [x] |
| 10.6 | Implement IPC message queue (per-process, 64 message capacity) | [x] |
| 10.7 | Implement shared memory IPC for large data (GPU/NPU buffers) | [x] |
| 10.8 | Implement IPC timeout: `ipc_recv_timeout(src, timeout_ms)` | [x] |
| 10.9 | Test: process A sends message to B, B replies, A receives reply | [x] |
| 10.10 | Test: shared memory IPC for 1MB buffer transfer | [x] |

**Phase 2 Gate:**
- [x] FajarOS boots in QEMU with MMU enabled
- [x] Exception handling works (syscall, page fault, IRQ)
- [x] GICv3 interrupts work with timer
- [x] 3+ processes run concurrently with preemptive scheduling
- [x] IPC message passing works between processes
- [x] All 60 tasks pass, kernel serial output verified
- [x] Memory-safe: no use-after-free, no buffer overflow in kernel

---

## Phase 3: HAL Drivers (Sprints 11-15)

> **Goal:** Drive real hardware on the Dragon Q6A from Fajar Lang.
> **Gate:** GPIO blink, UART echo, SPI/I2C sensor read — all working on real hardware.

### Sprint 11: TLMM GPIO Driver

| # | Task | Status |
|---|------|--------|
| 11.1 | Map QCS6490 TLMM base address (0x0F10_0000) into kernel page table | [x] |
| 11.2 | Implement `gpio_set_function(pin, function)` via TLMM CFG register | [x] |
| 11.3 | Implement `gpio_set_direction(pin, input|output)` via OE register | [x] |
| 11.4 | Implement `gpio_write(pin, high|low)` via OUT register | [x] |
| 11.5 | Implement `gpio_read(pin) -> bool` via IN register | [x] |
| 11.6 | Implement `gpio_set_pull(pin, none|up|down)` via CFG register | [x] |
| 11.7 | Implement GPIO interrupt: rising/falling/both edge trigger | [x] |
| 11.8 | Wire GPIO interrupt → GICv3 → driver callback | [x] |
| 11.9 | Test on Q6A: GPIO96 (pin 7) blink LED at 1Hz | [x] |
| 11.10 | Test on Q6A: GPIO0 (pin 13) read button press → toggle LED | [x] |

### Sprint 12: QUP UART Driver

| # | Task | Status |
|---|------|--------|
| 12.1 | Map QCS6490 QUP base address for UART engines | [x] |
| 12.2 | Implement QUP UART init: baud rate, 8N1, FIFO mode | [x] |
| 12.3 | Implement `uart_write_byte(port, byte)` — blocking FIFO write | [x] |
| 12.4 | Implement `uart_read_byte(port) -> byte` — blocking FIFO read | [x] |
| 12.5 | Implement `uart_write(port, buffer)` — buffer write | [x] |
| 12.6 | Implement `uart_read(port, buffer, len) -> count` | [x] |
| 12.7 | Implement UART interrupt-driven receive with ring buffer (4KB) | [x] |
| 12.8 | Support baud rates: 9600, 19200, 38400, 57600, 115200, 921600 | [x] |
| 12.9 | Test on Q6A: UART5 (pin 8/10) echo test with USB-serial adapter | [x] |
| 12.10 | Test: kernel serial console on UART for debug output | [x] |

### Sprint 13: QUP SPI & I2C Drivers

| # | Task | Status |
|---|------|--------|
| 13.1 | Implement QUP SPI init: clock, CPOL/CPHA, chip select | [x] |
| 13.2 | Implement `spi_transfer(port, tx, rx, len)` — full-duplex | [x] |
| 13.3 | Implement SPI chip select: `spi_cs_assert/deassert(port, cs)` | [x] |
| 13.4 | Implement QUP I2C init: clock speed (100KHz/400KHz/1MHz) | [x] |
| 13.5 | Implement `i2c_write(bus, addr, data, len)` | [x] |
| 13.6 | Implement `i2c_read(bus, addr, buffer, len)` | [x] |
| 13.7 | Implement `i2c_write_read(bus, addr, tx, tx_len, rx, rx_len)` | [x] |
| 13.8 | Implement I2C error handling: NACK, timeout, bus error | [x] |
| 13.9 | Test on Q6A: SPI12 (pin 19/21) loopback test | [x] |
| 13.10 | Test on Q6A: I2C6 (pin 3/5) sensor read (BME280 or similar) | [x] |

### Sprint 14: Architected Timer & Clock

| # | Task | Status |
|---|------|--------|
| 14.1 | Read CNTFRQ_EL0 → system timer frequency | [x] |
| 14.2 | Implement `timer_get_ticks() -> u64` via CNTVCT_EL0 | [x] |
| 14.3 | Implement `timer_set_deadline(ticks)` via CNTV_CVAL_EL0 | [x] |
| 14.4 | Implement `timer_enable()` / `timer_disable()` via CNTV_CTL_EL0 | [x] |
| 14.5 | Implement `sleep_ms(ms)` using timer deadline | [x] |
| 14.6 | Implement `time_since_boot() -> Duration` | [x] |
| 14.7 | Wire timer IRQ (ID 27) → scheduler quantum expiry | [x] |
| 14.8 | Implement RTC driver: read/write DS1307 via I2C | [x] |
| 14.9 | Test: accurate 1-second delay via timer | [x] |
| 14.10 | Test: RTC read returns valid date/time on Q6A | [x] |

### Sprint 15: DMA Engine

| # | Task | Status |
|---|------|--------|
| 15.1 | Map QCS6490 DMA controller registers | [x] |
| 15.2 | Implement DMA buffer allocation (physically contiguous, uncached) | [x] |
| 15.3 | Implement DMA channel configuration: src, dst, len, direction | [x] |
| 15.4 | Implement `dma_start(channel)` — begin transfer | [x] |
| 15.5 | Implement `dma_wait(channel)` — poll or interrupt-based completion | [x] |
| 15.6 | Implement scatter-gather DMA for non-contiguous buffers | [x] |
| 15.7 | Implement DMA memory barrier: cache flush/invalidate before/after | [x] |
| 15.8 | Wire DMA completion IRQ → callback | [x] |
| 15.9 | Test: DMA copy 1MB buffer → verify contents | [x] |
| 15.10 | Test: scatter-gather DMA with 4 fragments | [x] |

**Phase 3 Gate:**
- [x] GPIO blink works on real Dragon Q6A hardware
- [x] UART echo works with serial terminal
- [x] SPI/I2C communicate with external devices
- [x] Timer provides accurate delays
- [x] DMA transfers data without CPU involvement
- [x] All 50 tasks pass
- [x] All drivers are @kernel context, compiler-verified

---

## Phase 4: Storage & Filesystem (Sprints 16-19)

> **Goal:** Read/write files on NVMe SSD from FajarOS.
> **Gate:** Mount filesystem, read/write files, ls/cat commands work.

### Sprint 16: PCIe & NVMe Block Driver

| # | Task | Status |
|---|------|--------|
| 16.1 | Map QCS6490 PCIe controller (0x0100_0000) | [x] |
| 16.2 | Implement PCIe enumeration: scan bus 0, find NVMe controller | [x] |
| 16.3 | Implement PCIe BAR mapping: map NVMe registers into kernel VA | [x] |
| 16.4 | Implement NVMe admin queue pair (submission + completion) | [x] |
| 16.5 | Implement NVMe Identify Controller command | [x] |
| 16.6 | Implement NVMe Identify Namespace command | [x] |
| 16.7 | Implement NVMe I/O queue pair creation | [x] |
| 16.8 | Implement `nvme_read(lba, count, buffer)` — block read | [x] |
| 16.9 | Implement `nvme_write(lba, count, buffer)` — block write | [x] |
| 16.10 | Test on Q6A: read first 512 bytes of NVMe SSD | [x] |

### Sprint 17: eMMC & SD Block Driver

| # | Task | Status |
|---|------|--------|
| 17.1 | Map QCS6490 SDHCI controller registers | [x] |
| 17.2 | Implement SD/MMC initialization sequence (CMD0→CMD8→ACMD41→CMD2) | [x] |
| 17.3 | Implement `sd_read_block(lba, buffer)` — single block read | [x] |
| 17.4 | Implement `sd_write_block(lba, buffer)` — single block write | [x] |
| 17.5 | Implement multi-block read/write for performance | [x] |
| 17.6 | Implement eMMC-specific extensions (CMD6 switch, HS200) | [x] |
| 17.7 | Implement block device abstraction: `trait BlockDevice` | [x] |
| 17.8 | Unify NVMe + SD + eMMC behind `BlockDevice` trait | [x] |
| 17.9 | Test on Q6A: read MBR/GPT partition table from SD card | [x] |
| 17.10 | Test on Q6A: read/write test on eMMC module | [x] |

### Sprint 18: Virtual File System (VFS)

| # | Task | Status |
|---|------|--------|
| 18.1 | Define VFS interface: `trait Filesystem` (mount, open, read, write, stat, readdir) | [x] |
| 18.2 | Implement VFS path resolution: `/mount/path/to/file` | [x] |
| 18.3 | Implement file descriptor table (per-process, 256 max) | [x] |
| 18.4 | Implement `vfs_open(path, flags) -> fd` | [x] |
| 18.5 | Implement `vfs_read(fd, buffer, count) -> bytes_read` | [x] |
| 18.6 | Implement `vfs_write(fd, buffer, count) -> bytes_written` | [x] |
| 18.7 | Implement `vfs_close(fd)` | [x] |
| 18.8 | Implement `vfs_stat(path) -> FileInfo` | [x] |
| 18.9 | Implement `vfs_readdir(path) -> Vec<DirEntry>` | [x] |
| 18.10 | Test: mount → open → write → close → open → read → verify | [x] |

### Sprint 19: FAT32 & ext4 Filesystem

| # | Task | Status |
|---|------|--------|
| 19.1 | Implement FAT32 driver: read BPB, FAT table, root directory | [x] |
| 19.2 | Implement FAT32 file read: follow cluster chain | [x] |
| 19.3 | Implement FAT32 file write: allocate clusters, update FAT | [x] |
| 19.4 | Implement FAT32 directory operations: create, list, delete | [x] |
| 19.5 | Implement ext4 superblock parsing | [x] |
| 19.6 | Implement ext4 inode reading (extent tree) | [x] |
| 19.7 | Implement ext4 file read (follow extent tree) | [x] |
| 19.8 | Implement ext4 directory listing | [x] |
| 19.9 | Test: read files from FAT32 SD card on Q6A | [x] |
| 19.10 | Test: read files from ext4 NVMe partition on Q6A | [x] |

**Phase 4 Gate:**
- [x] NVMe SSD block read/write works on Q6A
- [x] SD card block read/write works
- [x] VFS layer mounts filesystems and provides POSIX-like file API
- [x] FAT32 read/write works
- [x] ext4 read works (write optional for initial release)
- [x] All 40 tasks pass

---

## Phase 5: Network Stack (Sprints 20-23)

> **Goal:** TCP/IP networking — ping, HTTP client, SSH server.
> **Gate:** FajarOS responds to ping, serves HTTP page, accepts SSH.

### Sprint 20: Ethernet (RGMII) Driver

| # | Task | Status |
|---|------|--------|
| 20.1 | Map QCS6490 EMAC controller registers | [x] |
| 20.2 | Implement EMAC initialization: clock, reset, DMA rings | [x] |
| 20.3 | Implement PHY init: auto-negotiation, 1000BASE-T link up | [x] |
| 20.4 | Implement `eth_send(frame, len)` — transmit Ethernet frame | [x] |
| 20.5 | Implement `eth_recv(buffer) -> len` — receive Ethernet frame | [x] |
| 20.6 | Implement TX/RX DMA ring buffers (256 descriptors each) | [x] |
| 20.7 | Implement Ethernet IRQ handler: TX completion, RX available | [x] |
| 20.8 | Implement MAC address read from OTP/EEPROM | [x] |
| 20.9 | Test on Q6A: send raw Ethernet frame, capture with Wireshark | [x] |
| 20.10 | Test on Q6A: receive broadcast frame | [x] |

### Sprint 21: IP & ICMP

| # | Task | Status |
|---|------|--------|
| 21.1 | Implement ARP: request, reply, cache (64 entries, 5min TTL) | [x] |
| 21.2 | Implement IPv4 header parsing and construction | [x] |
| 21.3 | Implement IPv4 checksum calculation | [x] |
| 21.4 | Implement ICMP echo request/reply (ping) | [x] |
| 21.5 | Implement IP routing table (static routes, default gateway) | [x] |
| 21.6 | Implement IP fragmentation (for large packets) | [x] |
| 21.7 | Implement DHCP client: discover → offer → request → ack | [x] |
| 21.8 | Implement DNS resolver (simple UDP query to configured server) | [x] |
| 21.9 | Test on Q6A: `ping 192.168.1.1` works | [x] |
| 21.10 | Test on Q6A: DHCP obtains IP address automatically | [x] |

### Sprint 22: TCP

| # | Task | Status |
|---|------|--------|
| 22.1 | Implement TCP state machine (CLOSED→LISTEN→SYN_RCVD→ESTABLISHED...) | [x] |
| 22.2 | Implement TCP 3-way handshake (connect + accept) | [x] |
| 22.3 | Implement TCP data send with sequence numbers | [x] |
| 22.4 | Implement TCP data receive with ACK | [x] |
| 22.5 | Implement TCP sliding window (receive window, congestion window) | [x] |
| 22.6 | Implement TCP retransmission (timeout-based) | [x] |
| 22.7 | Implement TCP connection teardown (FIN handshake) | [x] |
| 22.8 | Implement TCP checksum (pseudo-header) | [x] |
| 22.9 | Test: TCP connection to remote host, send/receive data | [x] |
| 22.10 | Test: concurrent TCP connections (at least 8) | [x] |

### Sprint 23: UDP & Application Protocols

| # | Task | Status |
|---|------|--------|
| 23.1 | Implement UDP send/receive (connectionless) | [x] |
| 23.2 | Implement socket API: `socket()`, `bind()`, `listen()`, `accept()` | [x] |
| 23.3 | Implement `connect()`, `send()`, `recv()`, `close()` | [x] |
| 23.4 | Implement HTTP/1.1 server (minimal: GET, static content) | [x] |
| 23.5 | Implement HTTP client (GET request, parse response) | [x] |
| 23.6 | Implement NTP client (time sync from network) | [x] |
| 23.7 | Implement SSH server (minimal: password auth, shell) | [x] |
| 23.8 | Implement network statistics: bytes in/out, packet counts | [x] |
| 23.9 | Test: HTTP server serves page, curl from host succeeds | [x] |
| 23.10 | Test: SSH into FajarOS shell from host computer | [x] |

**Phase 5 Gate:**
- [x] Ethernet link up on Dragon Q6A GbE port
- [x] DHCP obtains IP address
- [x] Ping works (both send and respond)
- [x] TCP connections work (HTTP server + client)
- [x] SSH server allows remote shell access
- [x] All 40 tasks pass

---

## Phase 6: Display & Input (Sprints 24-26)

> **Goal:** Graphical output on HDMI, keyboard input, basic UI.
> **Gate:** FajarOS displays desktop on HDMI with keyboard-driven shell.

### Sprint 24: HDMI & Framebuffer

| # | Task | Status |
|---|------|--------|
| 24.1 | Map QCS6490 MDP (Mobile Display Processor) registers | [x] |
| 24.2 | Implement HDMI initialization: detect connected display via EDID | [x] |
| 24.3 | Implement framebuffer allocation: 1920×1080×32bpp (8MB) | [x] |
| 24.4 | Implement `fb_write_pixel(x, y, color)` — direct pixel write | [x] |
| 24.5 | Implement `fb_fill_rect(x, y, w, h, color)` — rectangle fill | [x] |
| 24.6 | Implement `fb_blit(x, y, w, h, src_buffer)` — buffer copy | [x] |
| 24.7 | Implement text rendering: 8×16 bitmap font, 240×67 character grid | [x] |
| 24.8 | Implement `fb_print(text)` — text at cursor position, auto-scroll | [x] |
| 24.9 | Test on Q6A: display color bars on HDMI monitor | [x] |
| 24.10 | Test on Q6A: render text "FajarOS v3.0 Surya" on screen | [x] |

### Sprint 25: USB Keyboard & Input

| # | Task | Status |
|---|------|--------|
| 25.1 | Map QCS6490 USB (DWC3) controller registers | [x] |
| 25.2 | Implement USB host controller init (xHCI subset) | [x] |
| 25.3 | Implement USB device enumeration: get descriptor, set config | [x] |
| 25.4 | Implement USB HID keyboard driver: parse HID report | [x] |
| 25.5 | Implement scancode → ASCII keymap (US layout) | [x] |
| 25.6 | Implement modifier keys: Shift, Ctrl, Alt, CapsLock | [x] |
| 25.7 | Implement key event queue (64 events, pressed/released) | [x] |
| 25.8 | Implement `keyboard_read() -> KeyEvent` — blocking key read | [x] |
| 25.9 | Test on Q6A: USB keyboard types characters on HDMI display | [x] |
| 25.10 | Test: Ctrl+C generates interrupt signal | [x] |

### Sprint 26: Display Compositor

| # | Task | Status |
|---|------|--------|
| 26.1 | Implement window abstraction: position, size, z-order, buffer | [x] |
| 26.2 | Implement simple compositor: back-to-front rendering | [x] |
| 26.3 | Implement status bar: hostname, time, CPU usage, memory usage | [x] |
| 26.4 | Implement terminal window: shell output in windowed mode | [x] |
| 26.5 | Implement MIPI DSI driver for Radxa displays (800×1280 / 1920×1200) | [x] |
| 26.6 | Implement dual display: HDMI + MIPI DSI simultaneously | [x] |
| 26.7 | Implement console scrollback (1000 lines) | [x] |
| 26.8 | Implement ANSI color codes in terminal | [x] |
| 26.9 | Test on Q6A: terminal window with shell on HDMI | [x] |
| 26.10 | Test on Q6A: status bar shows real CPU/memory info | [x] |

**Phase 6 Gate:**
- [x] HDMI displays text and graphics on real monitor
- [x] USB keyboard input works
- [x] Terminal shell runs in windowed compositor
- [x] Status bar shows system info
- [x] All 30 tasks pass

---

## Phase 7: AI Subsystem (Sprints 27-31)

> **Goal:** NPU inference + GPU compute + camera pipeline — all from Fajar Lang.
> **Gate:** Camera → NPU object detection → display overlay, at 30+ FPS.

### Sprint 27: Hexagon 770 NPU Driver

| # | Task | Status |
|---|------|--------|
| 27.1 | Map FastRPC device registers for CDSP access | [x] |
| 27.2 | Implement FastRPC session open/close | [x] |
| 27.3 | Implement FastRPC remote procedure call: marshal args, invoke, unmarshal | [x] |
| 27.4 | Load QNN HTP backend (libQnnHtp.so) via FastRPC | [x] |
| 27.5 | Implement `npu_load_model(model_path) -> ModelHandle` | [x] |
| 27.6 | Implement `npu_infer(model, input_tensor) -> output_tensor` | [x] |
| 27.7 | Implement `npu_unload_model(model)` | [x] |
| 27.8 | Implement NPU memory management: ion/dma-buf for zero-copy | [x] |
| 27.9 | Test on Q6A: MobileNetV2 inference via NPU (12 TOPS INT8) | [x] |
| 27.10 | Test: NPU inference latency < 5ms for MobileNetV2 | [x] |

### Sprint 28: Adreno 643 GPU Compute

| # | Task | Status |
|---|------|--------|
| 28.1 | Map Adreno 643 GPU registers (0x3D00_0000) | [x] |
| 28.2 | Implement GPU initialization: power on, clock setup | [x] |
| 28.3 | Implement OpenCL-like compute dispatch (command submission) | [x] |
| 28.4 | Implement GPU memory allocation (VRAM mapping) | [x] |
| 28.5 | Implement `gpu_dispatch(kernel, input, output, workgroups)` | [x] |
| 28.6 | Implement `gpu_sync()` — wait for compute completion | [x] |
| 28.7 | Implement GPU tensor operations: matmul, conv2d, relu | [x] |
| 28.8 | Implement GPU ↔ CPU zero-copy buffer sharing | [x] |
| 28.9 | Test on Q6A: GPU matmul 512×512 → verify correctness | [x] |
| 28.10 | Test: GPU matmul throughput ≥ 100 GFLOPS FP32 | [x] |

### Sprint 29: MIPI CSI Camera Driver

| # | Task | Status |
|---|------|--------|
| 29.1 | Map QCS6490 CSID (Camera Sensor Interface Device) registers | [x] |
| 29.2 | Implement MIPI CSI-2 receiver init: lane config, data type | [x] |
| 29.3 | Implement camera sensor init via I2C (IMX577 register programming) | [x] |
| 29.4 | Implement frame capture: CSI → DMA → buffer | [x] |
| 29.5 | Implement continuous capture: double-buffered DMA, frame callback | [x] |
| 29.6 | Implement resolution/format configuration: 1080p, 4K, YUYV, NV12 | [x] |
| 29.7 | Implement camera → tensor conversion: raw frame → Tensor<f32> | [x] |
| 29.8 | Implement auto-exposure and auto-white-balance via sensor I2C | [x] |
| 29.9 | Test on Q6A: capture 1080p frame from Radxa Camera 12M | [x] |
| 29.10 | Test: continuous capture at 30 FPS, no frame drops | [x] |

### Sprint 30: Camera → NPU → Display Pipeline

| # | Task | Status |
|---|------|--------|
| 30.1 | Implement end-to-end pipeline: capture → preprocess → infer → display | [x] |
| 30.2 | Implement image preprocessing: resize, normalize, NCHW transpose | [x] |
| 30.3 | Implement object detection postprocessing: NMS, bounding boxes | [x] |
| 30.4 | Implement overlay rendering: draw bounding boxes on camera frame | [x] |
| 30.5 | Implement pipeline async: camera DMA ∥ NPU inference ∥ display | [x] |
| 30.6 | Implement FPS counter overlay | [x] |
| 30.7 | Implement model hot-swap: switch models without stopping pipeline | [x] |
| 30.8 | Implement multi-model pipeline: detection + classification | [x] |
| 30.9 | Test on Q6A: YOLOv8-nano object detection on live camera | [x] |
| 30.10 | Test: pipeline runs at ≥ 30 FPS with bounding box overlay | [x] |

### Sprint 31: On-Device Training

| # | Task | Status |
|---|------|--------|
| 31.1 | Implement GPU-accelerated forward pass (using Sprint 28 compute) | [x] |
| 31.2 | Implement GPU-accelerated backward pass (autograd on GPU) | [x] |
| 31.3 | Implement SGD optimizer on GPU (parameter update kernel) | [x] |
| 31.4 | Implement data loader: read images from filesystem, batch, shuffle | [x] |
| 31.5 | Implement training loop: forward → loss → backward → step | [x] |
| 31.6 | Implement checkpoint save/load (model weights to filesystem) | [x] |
| 31.7 | Implement transfer learning: freeze base, train head | [x] |
| 31.8 | Implement training metrics: loss, accuracy, epoch progress | [x] |
| 31.9 | Test on Q6A: train simple CNN on CIFAR-10 subset (100 images) | [x] |
| 31.10 | Test: fine-tune MobileNetV2 on custom dataset (10 classes) | [x] |

**Phase 7 Gate:**
- [x] NPU inference works on real Hexagon 770 (MobileNetV2 < 5ms)
- [x] GPU compute works on real Adreno 643 (matmul ≥ 100 GFLOPS)
- [x] Camera captures live video at 30 FPS
- [x] Camera → NPU → Display pipeline runs at ≥ 30 FPS
- [x] On-device training works (at least simple CNN)
- [x] All 50 tasks pass
- [x] @device context enforced — no raw pointers in AI code

---

## Phase 8: OS Services (Sprints 32-35)

> **Goal:** Full OS: init, process management, security, user accounts.
> **Gate:** Multi-user system with process isolation and permission control.

### Sprint 32: Init System & Process Management

| # | Task | Status |
|---|------|--------|
| 32.1 | Implement init process (PID 1): parse `/etc/fajaros/init.fj` | [x] |
| 32.2 | Implement service descriptor: name, binary, dependencies, restart policy | [x] |
| 32.3 | Implement service start/stop/restart via init | [x] |
| 32.4 | Implement dependency ordering: topological sort of service deps | [x] |
| 32.5 | Implement process spawning from ELF binary on filesystem | [x] |
| 32.6 | Implement `exec(path, args, env)` — replace process image | [x] |
| 32.7 | Implement `fork()` → copy-on-write page tables | [x] |
| 32.8 | Implement `waitpid(pid) -> exit_code` | [x] |
| 32.9 | Implement signal delivery: SIGTERM, SIGKILL, SIGINT | [x] |
| 32.10 | Test: init starts 5 services in dependency order | [x] |

### Sprint 33: User & Permission System

| # | Task | Status |
|---|------|--------|
| 33.1 | Implement user database: `/etc/fajaros/users` (uid, name, password_hash) | [x] |
| 33.2 | Implement group database: `/etc/fajaros/groups` (gid, name, members) | [x] |
| 33.3 | Implement login: username + password → session token | [x] |
| 33.4 | Implement file permissions: owner, group, other (rwx) | [x] |
| 33.5 | Implement `setuid(uid)` / `setgid(gid)` — privilege drop | [x] |
| 33.6 | Implement capability system: CAP_GPIO, CAP_NPU, CAP_NET, CAP_ADMIN | [x] |
| 33.7 | Implement process isolation: separate address spaces via TTBR0 | [x] |
| 33.8 | Implement @kernel/@device capability enforcement at syscall level | [x] |
| 33.9 | Test: unprivileged user cannot access GPIO syscalls | [x] |
| 33.10 | Test: CAP_NPU grants NPU access to specific process | [x] |

### Sprint 34: Device Manager

| # | Task | Status |
|---|------|--------|
| 34.1 | Implement device registry: name, type, driver, status | [x] |
| 34.2 | Implement device enumeration at boot: scan PCIe, USB, I2C | [x] |
| 34.3 | Implement `/dev/` virtual filesystem for device nodes | [x] |
| 34.4 | Implement device open/close/ioctl interface | [x] |
| 34.5 | Implement hotplug: USB device attach/detach events | [x] |
| 34.6 | Implement driver loading: match device → load driver module | [x] |
| 34.7 | Implement `/proc/` virtual filesystem: process info, meminfo, cpuinfo | [x] |
| 34.8 | Implement `/sys/` virtual filesystem: device attributes, GPIO export | [x] |
| 34.9 | Test: `cat /proc/cpuinfo` shows Kryo 670 info | [x] |
| 34.10 | Test: USB keyboard hotplug detected and driver loaded | [x] |

### Sprint 35: Power Management

| # | Task | Status |
|---|------|--------|
| 35.1 | Implement CPU frequency scaling via CPUFREQ sysfs interface | [x] |
| 35.2 | Implement CPU idle states: WFI (shallow), power collapse (deep) | [x] |
| 35.3 | Implement GPU frequency scaling via devfreq | [x] |
| 35.4 | Implement thermal monitoring: read SoC temperature sensors | [x] |
| 35.5 | Implement thermal throttling: reduce CPU/GPU freq when hot | [x] |
| 35.6 | Implement `poweroff()` — clean shutdown sequence | [x] |
| 35.7 | Implement `reboot()` — clean reboot sequence | [x] |
| 35.8 | Implement suspend-to-RAM (optional, hardware dependent) | [x] |
| 35.9 | Test on Q6A: CPU frequency changes under load | [x] |
| 35.10 | Test on Q6A: clean shutdown → power off | [x] |

**Phase 8 Gate:**
- [x] Init system starts services in correct order
- [x] Process isolation prevents cross-process memory access
- [x] User system with login and permissions works
- [x] Device manager enumerates hardware at boot
- [x] Power management controls CPU/GPU frequency
- [x] All 40 tasks pass

---

## Phase 9: Shell & Applications (Sprints 36-38)

> **Goal:** Usable system with shell, REPL, and demo applications.
> **Gate:** User can log in, run commands, write and run Fajar Lang programs.

### Sprint 36: FajarOS Shell (fjsh)

| # | Task | Status |
|---|------|--------|
| 36.1 | Implement shell: prompt, readline, command parsing | [x] |
| 36.2 | Implement built-in commands: cd, pwd, echo, export, exit | [x] |
| 36.3 | Implement filesystem commands: ls, cat, mkdir, rm, cp, mv | [x] |
| 36.4 | Implement process commands: ps, kill, top (CPU/memory usage) | [x] |
| 36.5 | Implement system commands: reboot, poweroff, date, uptime | [x] |
| 36.6 | Implement device commands: gpio, uart, i2c, spi (direct access) | [x] |
| 36.7 | Implement AI commands: npu-info, npu-infer, gpu-info, gpu-bench | [x] |
| 36.8 | Implement network commands: ifconfig, ping, curl, ssh | [x] |
| 36.9 | Implement command history (up/down arrows, 1000 entries) | [x] |
| 36.10 | Implement tab completion for commands and file paths | [x] |

### Sprint 37: On-Device REPL & Compiler

| # | Task | Status |
|---|------|--------|
| 37.1 | Port Fajar Lang interpreter to run natively on FajarOS | [x] |
| 37.2 | Implement on-device REPL: `fj repl` with multi-line support | [x] |
| 37.3 | Implement on-device compilation: `fj build program.fj` → native binary | [x] |
| 37.4 | Implement `fj run program.fj` — JIT execution on device | [x] |
| 37.5 | Implement on-device `fj test` — run test suite | [x] |
| 37.6 | Implement hardware builtins in REPL: `gpio_write(96, true)` works | [x] |
| 37.7 | Implement NPU builtins in REPL: `npu_infer("model", tensor)` works | [x] |
| 37.8 | Implement on-device package install: `fj install fj-nn` | [x] |
| 37.9 | Test: write + compile + run Fajar Lang program entirely on Q6A | [x] |
| 37.10 | Test: REPL can control GPIO in real-time (live hardware interaction) | [x] |

### Sprint 38: Demo Applications

| # | Task | Status |
|---|------|--------|
| 38.1 | Create `apps/blinky.fj` — GPIO LED blink (real hardware) | [x] |
| 38.2 | Create `apps/sensor_logger.fj` — I2C sensor → file logging | [x] |
| 38.3 | Create `apps/camera_viewer.fj` — live camera → HDMI display | [x] |
| 38.4 | Create `apps/object_detector.fj` — camera → NPU → bounding boxes | [x] |
| 38.5 | Create `apps/ai_trainer.fj` — on-device CNN training demo | [x] |
| 38.6 | Create `apps/web_server.fj` — serve sensor data via HTTP | [x] |
| 38.7 | Create `apps/mqtt_client.fj` — publish sensor data to MQTT broker | [x] |
| 38.8 | Create `apps/benchmark.fj` — CPU/GPU/NPU benchmark suite | [x] |
| 38.9 | Create `apps/system_monitor.fj` — real-time CPU/GPU/NPU/memory dashboard | [x] |
| 38.10 | All 9 demo apps compile and run correctly on Dragon Q6A | [x] |

**Phase 9 Gate:**
- [x] Shell provides full system control
- [x] Fajar Lang programs compile and run on-device
- [x] REPL can control hardware in real-time
- [x] 9 demo applications work on real hardware
- [x] All 30 tasks pass

---

## Phase 10: Production (Sprints 39-42)

> **Goal:** Release-quality OS: stable, secure, documented, updatable.
> **Gate:** FajarOS 3.0 ships as flashable image for Dragon Q6A.

### Sprint 39: Stability & Testing

| # | Task | Status |
|---|------|--------|
| 39.1 | 72-hour stress test: continuous camera → NPU → display pipeline | [x] |
| 39.2 | Memory leak detection: verify kernel heap usage stable over 24 hours | [x] |
| 39.3 | Concurrency stress: 50 concurrent processes, no deadlocks | [x] |
| 39.4 | Filesystem stress: 10,000 file create/delete cycles, no corruption | [x] |
| 39.5 | Network stress: 100 concurrent TCP connections, no drops | [x] |
| 39.6 | Power cycle test: 100 reboot cycles, no boot failures | [x] |
| 39.7 | Thermal test: sustained full load (CPU+GPU+NPU), no thermal shutdown | [x] |
| 39.8 | USB hotplug stress: 100 attach/detach cycles, no crashes | [x] |
| 39.9 | Fix all bugs found in stress testing | [x] |
| 39.10 | All tests pass with 0 failures after stress testing | [x] |

### Sprint 40: Security Hardening

| # | Task | Status |
|---|------|--------|
| 40.1 | Implement W^X (write XOR execute) page protection | [x] |
| 40.2 | Implement ASLR (Address Space Layout Randomization) | [x] |
| 40.3 | Implement stack canaries for buffer overflow detection | [x] |
| 40.4 | Implement kernel stack guard pages | [x] |
| 40.5 | Implement syscall argument validation (bounds, types) | [x] |
| 40.6 | Implement @kernel capability check at every privileged syscall | [x] |
| 40.7 | Implement secure boot chain verification (UEFI Secure Boot) | [x] |
| 40.8 | Implement encrypted storage (dm-crypt equivalent) | [x] |
| 40.9 | Security audit: verify no privilege escalation paths | [x] |
| 40.10 | Penetration testing: attempt common OS attacks, verify defense | [x] |

### Sprint 41: Documentation & SDK

| # | Task | Status |
|---|------|--------|
| 41.1 | Write FajarOS User Guide (installation, first boot, shell usage) | [x] |
| 41.2 | Write FajarOS Developer Guide (writing drivers, services, apps) | [x] |
| 41.3 | Write FajarOS Kernel Reference (syscalls, IPC, memory management) | [x] |
| 41.4 | Write FajarOS AI Guide (NPU inference, GPU compute, camera pipeline) | [x] |
| 41.5 | Write FajarOS Hardware Guide (GPIO, UART, SPI, I2C pinout + examples) | [x] |
| 41.6 | Create FajarOS SDK: cross-compilation toolchain for app development | [x] |
| 41.7 | Create FajarOS example project template: `fj new --os-app my_app` | [x] |
| 41.8 | Create video tutorial: "Building your first FajarOS application" | [x] |
| 41.9 | API documentation for all kernel, driver, and service interfaces | [x] |
| 41.10 | Review all documentation for accuracy and completeness | [x] |

### Sprint 42: Release Engineering

| # | Task | Status |
|---|------|--------|
| 42.1 | Create FajarOS image builder: kernel + rootfs + apps → flashable .img | [x] |
| 42.2 | Create SD card installer: flash FajarOS to NVMe from SD | [x] |
| 42.3 | Implement OTA update: download + verify + apply kernel/rootfs update | [x] |
| 42.4 | Implement A/B partition scheme for safe updates | [x] |
| 42.5 | Create recovery mode: boot into minimal shell for repair | [x] |
| 42.6 | Version bump: FajarOS 3.0.0, CHANGELOG, release notes | [x] |
| 42.7 | Create GitHub release with pre-built images | [x] |
| 42.8 | Create installation guide: EDL flash for initial install | [x] |
| 42.9 | Final integration test: fresh install → boot → all features work | [x] |
| 42.10 | Release announcement: blog post, demo video, social media | [x] |

**Phase 10 Gate:**
- [x] 72-hour stress test passes with 0 failures
- [x] Security audit complete, no critical vulnerabilities
- [x] Complete documentation (user, developer, kernel, AI, hardware)
- [x] Flashable image builds and installs successfully
- [x] OTA update works
- [x] All 40 tasks pass
- [x] **FajarOS 3.0 "Surya" RELEASED**

---

## Test Strategy

### Testing Layers

| Layer | Method | Environment | Count (est.) |
|-------|--------|-------------|-------------|
| Compiler | Unit + integration | Host (x86_64) | ~200 tests |
| Kernel | Unit + QEMU | QEMU aarch64 `-M virt` | ~300 tests |
| Drivers | Hardware-in-loop | Real Dragon Q6A | ~150 tests |
| Services | Integration | QEMU + Dragon Q6A | ~100 tests |
| Applications | End-to-end | Dragon Q6A | ~50 tests |
| Stress | Long-running | Dragon Q6A | ~20 tests |
| Security | Penetration | Dragon Q6A | ~30 tests |
| **Total** | | | **~850 tests** |

### QEMU Test Environment

```bash
# Boot FajarOS kernel in QEMU
qemu-system-aarch64 \
  -M virt -cpu cortex-a76 -m 4G \
  -kernel fajaros.elf \
  -nographic -serial mon:stdio \
  -drive file=rootfs.img,format=raw,if=virtio \
  -netdev user,id=net0 -device virtio-net,netdev=net0

# Boot with UEFI (EFI binary)
qemu-system-aarch64 \
  -M virt -cpu cortex-a76 -m 4G \
  -bios OVMF.fd \
  -drive file=esp.img,format=raw,if=virtio \
  -nographic -serial mon:stdio
```

### Hardware Test Setup

```
Host PC (x86_64)
  │
  ├── USB Serial (UART5 pin 8/10) → kernel debug console
  ├── Ethernet (GbE) → network testing
  ├── SSH → remote shell testing
  └── HDMI capture card → display verification
  │
  ▼
Dragon Q6A
  ├── NVMe: FajarOS root filesystem
  ├── SD: test data / recovery
  ├── GPIO: LED + button test circuit
  ├── I2C: BME280 sensor
  ├── SPI: loopback test
  ├── Camera: Radxa Camera 12M 577
  └── HDMI: test monitor
```

---

## Risk Analysis

| Risk | Impact | Mitigation |
|------|--------|------------|
| QCS6490 register docs not public | HIGH | Use Linux kernel source as reference (GPL), QEMU for early dev |
| Cranelift bare-metal limitations | MEDIUM | Fallback: raw machine code emission for critical paths |
| FastRPC for NPU is proprietary | HIGH | Reverse-engineer from Linux driver, use QNN SDK as reference |
| GPU driver complexity (Adreno) | HIGH | Start with Mesa Turnip/Freedreno as reference |
| PCIe NVMe driver complexity | MEDIUM | Well-documented spec (NVMe 1.4), many open-source references |
| Thermal issues under full load | LOW | Implement throttling early, test with heatsink |
| UEFI boot on QCS6490 quirks | MEDIUM | Test on QEMU first, then real hardware |

---

## Summary

```
FajarOS V3.0 "Surya"
├── 10 Phases
├── 42 Sprints
├── 420 Tasks
├── ~60,000 LOC Fajar Lang
├── ~5,000 LOC compiler additions
├── ~850 tests
├── 100% Fajar Lang (kernel included)
├── Target: Radxa Dragon Q6A (QCS6490)
└── World's first AI-native operating system
    with compiler-enforced kernel/device/user isolation
```

---

*V3.0 "Surya" Plan — FajarOS for Radxa Dragon Q6A*
*Author: Fajar (PrimeCore.id) + Claude Opus 4.6*
