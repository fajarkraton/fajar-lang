# FajarOS Nova — Boot Sequence

## Overview

FajarOS Nova boots via the **Multiboot2** protocol using GRUB2 as the bootloader. The kernel transitions from 32-bit protected mode (provided by GRUB) to 64-bit long mode via a hand-written trampoline, then enters `kernel_main()` — all in Fajar Lang.

## Step-by-Step Boot

### Stage 0: UEFI/BIOS → GRUB2

```
Power On
  → UEFI firmware (POST, DDR init, PCIe enumeration)
  → Load GRUB2 from EFI partition or ISO
  → GRUB2 reads grub.cfg:
      set timeout=0
      menuentry "FajarOS Nova" {
          multiboot2 /boot/fajaros.elf
          boot
      }
  → GRUB2 loads ELF into memory at 0x100000
  → GRUB2 sets up Multiboot2 info struct
  → Jump to kernel entry point (0x100040)
  → CPU is in 32-bit protected mode, paging OFF
```

### Stage 1: Multiboot2 Header (0x100000)

The ELF binary starts with a `.multiboot_header` section:

```
Offset  Bytes     Value              Description
0x0000  4         0xE85250D6         Multiboot2 magic
0x0004  4         0x00000000         Architecture (0 = i386/x86)
0x0008  4         0x00000040         Header length (64 bytes)
0x000C  4         checksum           -(magic + arch + length)
0x0010  ...       Tags               Address tag, entry tag, end tag
```

### Stage 2: 32-bit Trampoline (_start)

Entry at 0x100040, CPU in 32-bit protected mode:

```asm
_start:
    cli                         ; Disable interrupts

    ; 1. Set up identity-mapped page tables at 0x70000
    ;    PML4[0] → PDPT at 0x71000
    ;    PDPT[0] → PD at 0x72000
    ;    PD[0..63] → 2MB pages (0x00000000 - 0x07FFFFFF)
    ;    Each PD entry: base | PRESENT | WRITABLE | HUGE (bit 7)

    ; 2. Load PML4 address into CR3
    mov eax, 0x70000
    mov cr3, eax

    ; 3. Enable PAE (CR4 bit 5)
    mov eax, cr4
    or eax, (1 << 5)           ; PAE
    mov cr4, eax

    ; 4. Enable Long Mode (IA32_EFER.LME, MSR 0xC0000080 bit 8)
    mov ecx, 0xC0000080
    rdmsr
    or eax, (1 << 8)           ; LME
    wrmsr

    ; 5. Enable Paging (CR0 bit 31)
    mov eax, cr0
    or eax, (1 << 31)          ; PG
    mov cr0, eax

    ; 6. Load 64-bit GDT
    lgdt [gdt_ptr]

    ; 7. Far jump to 64-bit code segment
    jmp 0x08:long_mode_entry   ; CS = kernel code segment
```

### Stage 3: 64-bit Long Mode Entry

```asm
long_mode_entry:               ; Now in 64-bit long mode!
    ; Reload data segments
    mov ax, 0x10               ; Kernel data segment
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax
    mov ss, ax

    ; Set up stack
    mov rsp, 0x7F00000         ; Stack at 127 MB

    ; Zero BSS section
    ; ... (memset .bss to 0)

    ; Print "[BOOT32]" to serial (COM1: 0x3F8)
    ; ... (port_outb sequence)

    ; Call kernel_main (Fajar Lang code!)
    call kernel_main

    ; Halt if kernel_main returns
halt_loop:
    hlt
    jmp halt_loop
```

### Stage 4: kernel_main() — Fajar Lang

```fajar
@kernel fn kernel_main() {
    // 1. Initialize serial console
    x86_serial_init(0, 115200)          // COM1, 115200 baud

    // 2. Clear VGA screen
    console_init()                       // Clear 80×25 VGA buffer

    // 3. Enable SSE (required for Cranelift code)
    sse_enable()                         // CR0.EM=0, CR4.OSFXSR=1

    // 4. Set up Interrupt Descriptor Table
    idt_init()                           // 256 vectors, exception handlers

    // 5. Remap PIC (Programmable Interrupt Controller)
    pic_remap()                          // Master: 0x20-0x27, Slave: 0x28-0x2F

    // 6. Start PIT timer at 100 Hz
    pit_init(100)                        // 10ms quantum for preemption

    // 7. Load Task State Segment
    tss_init()                           // RSP0 for Ring 3→0 transitions

    // 8. Configure SYSCALL/SYSRET MSRs
    syscall_init()                       // IA32_STAR, IA32_LSTAR, IA32_FMASK

    // 9. Initialize RAM filesystem
    ramfs_init()                         // 64 inodes, pre-populate /etc /tmp

    // 10. Initialize command history
    history_init()                       // 8-slot ring buffer

    // 11. Initialize process table
    // ... (zero 4KB at 0x600000, set PID 0 as running)

    // 12. Initialize keyboard state
    // ... (shift=0, caps=0)

    // 13. Enable hardware interrupts
    irq_enable()                         // STI instruction

    // 14. Serial confirmation
    println("[NOVA] FajarOS Nova v0.1.0 booted")
    println("[NOVA] 100 shell commands ready")

    // 15. Display boot banner on VGA
    cprintln("FajarOS Nova v0.1.0 — x86_64 Shell", WHITE_ON_BLUE)

    // 16. Enter shell loop
    cprint("nova> ", GREEN_ON_BLACK)
    // ... keyboard polling + command dispatch
}
```

## Boot Timeline

```
Time    Event
──────  ────────────────────────────────────────
0.0ms   GRUB2 loads kernel ELF
0.1ms   32-bit trampoline: page tables, PAE, LME
0.2ms   Far jump to 64-bit long mode
0.3ms   Serial init → "[BOOT32]" on COM1
0.5ms   SSE + IDT + PIC + PIT setup
1.0ms   TSS + SYSCALL MSRs configured
1.2ms   RAM filesystem initialized
1.5ms   Interrupts enabled, timer ticking
2.0ms   Boot banner displayed, "nova>" prompt
```

## GDT Layout

```
Entry  Selector  Type           DPL  Description
─────  ────────  ─────────────  ───  ──────────────────
0      0x00      NULL           -    Required null descriptor
1      0x08      Code64 (L=1)   0    Kernel code segment
2      0x10      Data64         0    Kernel data segment
3      0x18      Code64 (L=1)   3    User code segment
4      0x20      Data64         3    User data segment
5      0x28      TSS64          0    Task State Segment (104 bytes)
```

## Page Table Layout (4-Level)

```
CR3 → PML4 at 0x70000
  PML4[0] → PDPT at 0x71000
    PDPT[0] → PD at 0x72000
      PD[0]  → 0x00000000 (2MB page, P|RW|PS)
      PD[1]  → 0x00200000 (2MB page, P|RW|PS)
      ...
      PD[63] → 0x07E00000 (2MB page, P|RW|PS)

Total: 64 × 2MB = 128 MB identity-mapped
Flags: Present (bit 0), Read/Write (bit 1), Page Size 2MB (bit 7)
```

## Verification

The boot sequence is verified by the automated test suite (`examples/fajaros_nova_test.sh`):

1. Serial output contains `[BOOT32]` — trampoline reached 64-bit
2. Serial output contains `[NOVA]...booted` — kernel_main() reached
3. Serial output contains `shell commands ready` — all init complete
4. Serial output contains `RamFS` — filesystem initialized
5. Serial output contains `VGA console` — display ready
6. VGA screenshot shows boot banner and `nova>` prompt

---

*FajarOS Nova Boot Sequence v1.0 — March 2026*
