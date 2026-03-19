# FajarOS Nova — System Architecture

> x86_64 bare-metal OS written 100% in Fajar Lang

## System Layers

```
┌─────────────────────────────────────────────────────────┐
│ Layer 5: Shell (102 commands)                           │
│   help, ls, cat, grep, sort, calc, neofetch, demo...   │
├─────────────────────────────────────────────────────────┤
│ Layer 4: RAM Filesystem                                 │
│   64 inodes, 832KB data, CRUD + grep + sort + md5      │
├─────────────────────────────────────────────────────────┤
│ Layer 3: Drivers                                        │
│   PS/2 Keyboard (Shift/Caps/Arrows), VGA Text (80×25), │
│   PCI Bus (32 devices), Serial UART (COM1: 0x3F8)      │
├─────────────────────────────────────────────────────────┤
│ Layer 2: Microkernel                                    │
│   IDT (256 vectors), PIC (8259A), PIT (100 Hz),        │
│   GDT+TSS, SYSCALL/SYSRET, Preemptive Scheduler,      │
│   128MB Paging (PML4, 2MB pages), Bump Allocator       │
├─────────────────────────────────────────────────────────┤
│ Layer 1: Boot                                           │
│   Multiboot2 header, 32→64 trampoline, SSE enable      │
├─────────────────────────────────────────────────────────┤
│ Layer 0: Hardware — Intel x86_64                        │
│   QEMU or real hardware (i9-14900HX target)            │
└─────────────────────────────────────────────────────────┘
```

## Memory Map

```
Physical Address    Size     Description
──────────────────  ───────  ─────────────────────────────────
0x0000_0000         1 MB     Real Mode IVT + BDA + EBDA (reserved)
0x000B_8000         4 KB     VGA text buffer (80×25 chars)
0x000E_0000         128 KB   BIOS ROM / ACPI tables (RSDP here)
0x0010_0000         64 KB    Kernel .text (code)
0x0011_1000         8 KB     Kernel .rodata (strings, constants)
0x0040_0000         108 MB   Kernel heap (bump allocator)
0x0050_0000         512 KB   Tensor scratch / matmul workspace
0x0058_0000         512 KB   Sort temp buffer
0x0060_0000         4 KB     Process table (16 PIDs × 256 bytes)
0x006F_800          64 B     Command line buffer
0x006F_880          8 B      Command buffer length
0x006F_900          512 B    Command history (8 × 64 bytes)
0x006F_A00          8 B      VGA cursor row
0x006F_A08          8 B      VGA cursor column
0x006F_BE0          8 B      Shift key state (0/1)
0x006F_BE8          8 B      CapsLock state (0/1)
0x006F_BF0          16 B     Keyboard buffer metadata
0x006F_C00          8 B      History count
0x006F_C08          8 B      History navigation index
0x006F_E00          8 B      Current PID
0x006F_E08          8 B      Process count
0x0070_0000         8 B      ramfs: file count
0x0070_0008         8 B      ramfs: next data offset
0x0070_0100         8 KB     ramfs: inode table (64 × 128 bytes)
0x0071_0000         832 KB   ramfs: data area
0x007F_0000         64 KB    Kernel stack
0x0800_0000         —        End of identity-mapped region (128 MB)
```

## Process Table Structure

Each process occupies 256 bytes at `0x600000 + pid * 256`:

```
Offset  Size  Field       Description
──────  ────  ─────────   ────────────────────────
+0      8 B   pid         Process ID (0-15)
+8      8 B   state       0=dead, 1=ready, 2=running, 3=blocked
+16     8 B   rsp         Saved stack pointer
+24     8 B   entry       Entry point address
+32     8 B   ticks       CPU ticks consumed
+40     8 B   priority    Scheduling priority (0=highest)
+48     8 B   parent_pid  Parent process ID
+56     8 B   exit_code   Exit code (when zombie/dead)
+64     16 B  name        Process name (null-terminated)
+80     176 B reserved    Future: FPU state, signals, etc.
```

## RAM Filesystem Structure

```
Base: 0x700000

Header (16 bytes):
  +0:  file_count (i64)
  +8:  next_data_offset (i64, starts at 0x710000)

Inode Table (0x700100, 64 entries × 128 bytes):
  +0:   name[32]     Filename (null-terminated bytes)
  +32:  size (i64)   File size in bytes
  +40:  data_ptr     Absolute address of file data
  +48:  type (i64)   1=regular file, 2=directory
  +56:  reserved     Future: timestamps, permissions

Data Area (0x710000 — 0x7E0000, 832 KB):
  Files stored sequentially (append-only allocation)
  No fragmentation handling (simple bump allocator)
```

## Interrupt Architecture

```
Vector  Source          Handler
──────  ─────────────  ────────────────────────────
0       #DE            Division by zero
1       #DB            Debug
2       NMI            Non-maskable interrupt
3       #BP            Breakpoint
6       #UD            Invalid opcode
8       #DF            Double fault (IST stack)
13      #GP            General protection fault
14      #PF            Page fault (CR2 = faulting addr)
32      PIT (IRQ0)     Timer tick → scheduler_tick()
33      KBD (IRQ1)     PS/2 keyboard → scancode buffer
0xFF    Spurious       Ignore
```

## Keyboard Input Pipeline

```
PS/2 Controller (port 0x60)
  → IRQ1 → PIC vector 33
    → Keyboard ISR: read scancode, push to ring buffer
      → Shell loop: kb_has_data() → kb_read_scancode()
        → Handle extended (0xE0 prefix → arrows)
        → Handle shift make/break (0x2A/0xAA, 0x36/0xB6)
        → Handle CapsLock toggle (0x3A)
        → Ignore break codes (>= 0x80)
        → sc2ascii(scancode) with shift/caps XOR
          → cmdbuf_push(ascii)
          → console_putchar(ascii, color)
          → vga_update_cursor()
```

## VGA Console

- **Buffer**: 0xB8000 (80 × 25 × 2 bytes = 4000 bytes)
- **Format**: `[char_byte][attr_byte]` pairs
- **Attributes**: `(bg << 4) | fg` — 16 colors each
- **Cursor**: Software-tracked at 0x6FA00 (row) + 0x6FA08 (col)
- **Hardware cursor**: Updated via ports 0x3D4/0x3D5 (CRT controller)
- **Scrolling**: `console_scroll()` — memmove rows up, clear bottom row
- **Colors used**: WHITE_ON_BLACK (0x0F), WHITE_ON_BLUE (0x1F), GREEN (0x0A), CYAN (0x0B), YELLOW (0x0E), RED (0x0C)

## Build Pipeline

```
Fajar Lang Source (.fj)
  → Lexer (tokenize)
    → Parser (AST)
      → Analyzer (type check, @kernel enforcement)
        → Cranelift Codegen (x86_64-unknown-none target)
          → Multiboot2 header generation
          → 32→64 bit trampoline (assembly)
          → Runtime stubs (serial, volatile I/O, interrupts)
          → Linker (ld, bare-metal, custom layout)
            → ELF64 x86_64 binary
              → grub-mkrescue → ISO
                → qemu-system-x86_64 -cdrom
```

## Compiler Builtins (27)

| Category | Builtins |
|----------|----------|
| **I/O** | `port_outb`, `port_inb`, `x86_serial_init` |
| **Volatile** | `volatile_read/write` (8/16/32/64-bit variants) |
| **String** | `str_len`, `str_byte_at` |
| **CPU** | `sse_enable`, `cpuid_eax/ebx/ecx/edx`, `rdtsc` |
| **Interrupts** | `idt_init`, `pic_remap`, `pit_init`, `irq_enable` |
| **System** | `tss_init`, `syscall_init`, `set_current_pid` |
| **Keyboard** | `kb_has_data`, `kb_read_scancode` |
| **Timer** | `read_timer_ticks` |
| **PCI** | `pci_read32` |
| **ACPI** | `acpi_find_rsdp`, `acpi_get_cpu_count`, `acpi_shutdown` |

---

*FajarOS Nova Architecture v1.0 — March 2026*
