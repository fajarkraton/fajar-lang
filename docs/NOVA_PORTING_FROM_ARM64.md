# FajarOS: Porting from ARM64 (Surya) to x86_64 (Nova)

## Architecture Comparison

| Aspect | ARM64 (Surya/Q6A) | x86_64 (Nova) |
|--------|-------------------|---------------|
| **Boot** | UEFI → kernel EL1 | Multiboot2 (GRUB2) → 32→64 trampoline |
| **Privilege** | EL0 (user) / EL1 (kernel) | Ring 3 (user) / Ring 0 (kernel) |
| **Exceptions** | VBAR_EL1 (4 groups × 4) | IDT (256 vectors) |
| **Paging** | TTBR0/TTBR1, 4KB granule | CR3 → PML4, 4KB/2MB pages |
| **Syscalls** | SVC instruction | SYSCALL/SYSRET (MSRs) |
| **IRQ** | GICv3 (Distributor + Redistributor) | PIC 8259A or LAPIC/IOAPIC |
| **Timer** | Architected Timer (CNTV) | PIT (8254) or LAPIC Timer |
| **Serial** | PL011 UART (MMIO) | 16550 UART (I/O ports) |
| **Display** | Framebuffer (MIPI DSI) | VGA text mode (0xB8000) |
| **Context** | 31 GPR + SPSR (272 bytes) | 16 GPR + RFLAGS (~136 bytes) |

## Key Differences

### 1. Boot Sequence

**ARM64 (Surya):**
```
UEFI → FajarOS ELF → kernel already at EL1 (64-bit)
- MMU already enabled by firmware
- Set VBAR_EL1 for exception vectors
- Configure MAIR, TCR, SCTLR for caching
```

**x86_64 (Nova):**
```
GRUB2 → Multiboot2 → 32-bit protected mode → manual transition:
1. Build PML4 page tables
2. Enable PAE (CR4)
3. Enable Long Mode (IA32_EFER.LME)
4. Enable Paging (CR0.PG)
5. Load 64-bit GDT
6. Far jump to 64-bit code segment
```

### 2. I/O Model

**ARM64:** All I/O is memory-mapped (MMIO). `volatile_read/write` to device registers.

**x86_64:** Two I/O models:
- **Port I/O** (`in`/`out` instructions): keyboard (0x60), serial (0x3F8), PCI config (0xCF8)
- **MMIO**: VGA buffer (0xB8000), LAPIC (0xFEE00000), IOAPIC (0xFEC00000)

### 3. Interrupt Architecture

**ARM64 (GICv3):**
```
CPU Interface → Distributor → SPIs/PPIs/SGIs
- Write ICC_SGI1R_EL1 for software interrupt
- Read IAR, write EOIR for EOI
- PPI 30 = architected timer
```

**x86_64 (PIC/APIC):**
```
PIC 8259A (legacy): Master (IRQ 0-7) + Slave (IRQ 8-15)
- IRQ0 = PIT timer (vector 0x20)
- IRQ1 = keyboard (vector 0x21)
- EOI: outb(0x20, 0x20)

LAPIC/IOAPIC (modern, needed for SMP):
- LAPIC at 0xFEE00000 (per-CPU)
- IOAPIC at 0xFEC00000 (shared)
- EOI: write 0 to LAPIC offset 0xB0
```

### 4. Page Table Format

Both use 4-level page tables, but format differs:

**ARM64:**
```
TTBR0_EL1 → L0 → L1 → L2 → L3
- Each entry: 64 bits
- Bits [47:12] = output address
- Bits [1:0] = 01 (block) or 11 (table)
- Attributes in upper/lower bits (MAIR index, AP, SH, AF, etc.)
```

**x86_64:**
```
CR3 → PML4 → PDPT → PD → PT
- Each entry: 64 bits
- Bits [51:12] = physical address
- Bit 0 = Present
- Bit 1 = Read/Write
- Bit 2 = User/Supervisor
- Bit 7 = Page Size (2MB if in PD)
- Bit 63 = NX (No Execute)
```

### 5. Context Switch

**ARM64 (Surya):**
```
Save: X0-X30, SP_EL0, ELR_EL1, SPSR_EL1 (272 bytes)
Restore: reverse order
Switch: write TTBR0_EL1 for per-process pages, TLBI VMALLE1
```

**x86_64 (Nova):**
```
Save: RAX-R15, RBP, RIP, RFLAGS, RSP (136 bytes)
Restore: pop all, ret
Switch: write CR3 for per-process pages (auto TLB flush)
```

### 6. Shared Fajar Lang Code

The following kernel code is **identical** between ARM64 and x86_64:

- RAM filesystem (ramfs_init, ramfs_find, cmd_ls, cmd_cat, etc.)
- Command buffer (cmdbuf_push, cmdbuf_pop, cmdbuf_match)
- Command history (history_push, history_navigate)
- Shell commands (most cmd_* functions)
- Calculator, math, text processing
- Process table management (spawn, kill, wait, ps)
- VGA console logic (console_putchar, cprint, cprintln — same algorithm, different base address)

**Architecture-specific code:**
- Boot trampoline (asm)
- Interrupt setup (IDT vs VBAR)
- Timer init (PIT vs architected timer)
- Serial init (port I/O vs MMIO)
- Keyboard driver (PS/2 vs device-specific)
- Page table setup (PML4 vs TTBR)

### 7. Porting Effort Estimate

| Component | Effort | Notes |
|-----------|--------|-------|
| Boot | HIGH | Completely different (Multiboot2 vs UEFI) |
| Interrupts | HIGH | IDT + PIC vs VBAR + GICv3 |
| Paging | MEDIUM | Same 4-level concept, different format |
| Timer | LOW | PIT vs arch timer, same tick concept |
| Serial | LOW | Port I/O vs MMIO, same baud rate |
| Drivers | MEDIUM | PS/2 keyboard vs device-specific |
| Shell | NONE | 100% portable Fajar Lang code |
| Filesystem | NONE | 100% portable |
| Commands | NONE | 100% portable |

**~30% of kernel code is architecture-specific, ~70% is portable Fajar Lang.**

---

*FajarOS Porting Guide v1.0 — March 2026*
