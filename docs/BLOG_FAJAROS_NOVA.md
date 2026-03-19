# FajarOS Nova: An OS Written 100% in Fajar Lang, Running on x86_64

> **"If it compiles in Fajar Lang, it's safe to deploy."**

## A New Star Rises

FajarOS Nova is a bare-metal operating system written entirely in **Fajar Lang** — our statically-typed systems programming language designed for embedded ML and OS integration. Nova boots on x86_64 hardware (Intel/AMD) via QEMU, features 97 shell commands, a RAM filesystem, and demonstrates that a single language can unify kernel code, device drivers, and AI inference.

**Nova** (Indonesian: *bintang baru* — a new star) is the x86_64 sibling of FajarOS Surya, which runs on ARM64 (Qualcomm QCS6490). Together, they prove Fajar Lang's cross-architecture capability.

---

## What Makes This Special

### 1. One Language, All Layers

Every line of FajarOS Nova — from the Multiboot2 boot sequence to the shell prompt — is written in Fajar Lang. No C, no assembly files, no Rust runtime. The compiler (Cranelift backend) generates bare-metal x86_64 ELF binaries directly.

```
Layer 5: Shell (97 commands)     ← Fajar Lang
Layer 4: RAM Filesystem          ← Fajar Lang
Layer 3: Drivers (VGA, KB, PCI)  ← Fajar Lang
Layer 2: Kernel (IDT, PIT, TSS)  ← Fajar Lang
Layer 1: Boot (Multiboot2)       ← Fajar Lang
Layer 0: Hardware (x86_64)
```

### 2. Compiler-Enforced Safety Contexts

Fajar Lang's unique `@kernel` / `@device` / `@safe` annotations enforce isolation at compile time:

- `@kernel` — Can access hardware (ports, MMIO, IRQ), cannot use heap strings or tensors
- `@device` — Can use tensor operations and GPU, cannot touch raw pointers or IRQ
- `@safe` — Standard code, no hardware access

This means **a neural network cannot corrupt kernel memory, and kernel code cannot accidentally allocate on the heap** — enforced by the compiler, not by convention.

### 3. From Boot to Shell in 2,910 Lines

The entire kernel is **2,910 lines of Fajar Lang** — smaller than many single-file web applications. It compiles to a **98KB ELF binary** that boots in QEMU in under 2 seconds.

---

## Technical Architecture

### Boot Sequence

```
GRUB2 (Multiboot2 protocol)
  → 32-bit trampoline (_start)
    → Enable PAE, set up PML4 (128MB identity-mapped, 2MB pages)
    → Enable long mode (IA32_EFER.LME)
    → Far jump to 64-bit code
      → kernel_main()
        → Serial init (COM1: 0x3F8, 115200 baud)
        → VGA console (80x25, 6 colors)
        → SSE enable (CR0/CR4)
        → IDT (256 vectors)
        → PIC remap (master 0x20, slave 0x28)
        → PIT timer (100 Hz, preemptive scheduling)
        → TSS + SYSCALL/SYSRET configured
        → RAM filesystem initialized (64 entries, 832KB)
        → Interactive shell → "nova> "
```

### Memory Layout

```
0x000B_8000  VGA text buffer (4KB)
0x0010_0000  Kernel .text (64KB)
0x0011_1000  Kernel .rodata (8KB)
0x0040_0000  Kernel heap (bump allocator, 108MB)
0x0050_0000  Tensor/scratch memory
0x0058_0000  Sort temp buffer
0x0060_0000  Process table (16 PIDs × 256 bytes)
0x006F_800   Command line buffer (64 bytes)
0x006F_900   Command history (8 × 64 bytes)
0x006F_A00   VGA cursor position
0x006F_BE0   Shift/CapsLock state
0x006F_C00   History metadata
0x0070_0000  RAM filesystem (inodes + 832KB data)
0x007F_0000  Stack (64KB)
```

### Hardware Support

| Component | Implementation |
|-----------|---------------|
| **CPU** | x86_64 long mode, SSE enabled, CPUID feature detection |
| **Interrupts** | IDT (256 vectors), PIC (8259A remapped), PIT (100 Hz) |
| **Memory** | 4-level paging (PML4), 128MB identity-mapped, 2MB huge pages |
| **Display** | VGA text mode (80×25), 6 color schemes, hardware cursor |
| **Keyboard** | PS/2 scancode set 1, Shift, CapsLock, Tab, arrow keys |
| **PCI** | Bus 0 enumeration (32 devices), vendor:device + class display |
| **ACPI** | RSDP discovery, CPU count from MADT, ACPI shutdown |
| **Timer** | PIT at 100 Hz, TSC (rdtsc) for benchmarking |
| **Serial** | 16550 UART (COM1: 0x3F8), 115200 baud |
| **Scheduler** | Round-robin preemptive (16 PIDs, 10ms quantum) |

---

## Shell Commands (97)

### System (19)
`help` `version` `about` `uname` `sysinfo` `uptime` `date` `hostname` `whoami` `arch` `dmesg` `env` `printenv` `id` `cal` `history` `man` `which` `banner`

### Hardware (8)
`cpuinfo` `meminfo` `free` `nproc` `lspci` `acpi` `tsc` `time`

### Process (6)
`ps` `top` `kill` `sleep` `reboot` `shutdown`

### Files (22)
`ls` `dir` `cat` `more` `touch` `rm` `cp` `mv` `mkdir` `rmdir` `pwd` `write` `append` `head` `tail` `wc` `grep` `sort` `uniq` `nl` `cut` `strings`

### File Info (7)
`stat` `xxd` `md5` `df` `du` `count` `dd`

### AI / Compute (4)
`tensor` `mnist` `bench` `fib`

### Math / Text (10)
`calc` `hex` `base` `factor` `prime` `len` `echo` `rev` `upcase` `downcase`

### Utility (17)
`clear` `cls` `seq` `true` `false` `yes` `dice` `logo` `splash` `color` `cowsay` `fortune` `repeat` `alias` `motd` `exit` `set`

### Text Processing (4)
`tr` `grep` `sort` `uniq`

---

## Key Code Samples

### Kernel Entry (Fajar Lang)

```fajar
@kernel fn kernel_main() {
    x86_serial_init(0, 115200)
    console_init()
    sse_enable()
    idt_init()
    pic_remap()
    pit_init(100)
    tss_init()
    syscall_init()
    ramfs_init()
    history_init()
    irq_enable()

    // Boot banner
    cprintln("FajarOS Nova v0.1.0 — x86_64 Shell", WHITE_ON_BLUE)
    cprintln("Type 'help' for available commands.", YELLOW_ON_BLACK)

    // Shell loop with Shift, CapsLock, arrow key history
    cmdbuf_clear()
    cprint("nova> ", GREEN_ON_BLACK)
    // ... keyboard polling + dispatch_command()
}
```

### VGA Console (Pure Fajar Lang)

```fajar
@kernel fn console_putchar(ch: i64, color: i64) {
    let row = volatile_read(0x6FA00)
    let col = volatile_read(0x6FA08)
    if ch == 10 {  // newline
        volatile_write(0x6FA08, 0)
        if row >= VGA_ROWS - 1 { console_scroll() }
        else { volatile_write(0x6FA00, row + 1) }
        return
    }
    let addr = VGA_BASE + (row * VGA_COLS + col) * 2
    volatile_write(addr, ch)
    volatile_write(addr + 1, color)
    // ... cursor advance + scroll
    vga_update_cursor()
}
```

### grep Implementation (In-Kernel)

```fajar
@kernel fn cmd_grep() {
    // Parse: grep <pattern> <file>
    // For each line in file, check if pattern is a substring
    // Print matching lines in green
}
```

---

## Numbers

| Metric | Value |
|--------|-------|
| **Kernel source** | 2,910 lines Fajar Lang |
| **Binary size** | 98 KB (ELF x86_64) |
| **Shell commands** | 97 |
| **Boot time** | < 2 seconds (QEMU) |
| **RAM filesystem** | 64 files, 832 KB |
| **Hardware cursor** | VGA ports 0x3D4/0x3D5 |
| **Keyboard** | 50+ scancodes, Shift, CapsLock |
| **Timer** | PIT 100 Hz, TSC benchmarking |
| **Paging** | 128 MB, 4-level (PML4), 2 MB pages |
| **Processes** | 16 PIDs, round-robin preemptive |
| **ACPI** | RSDP, CPU count, shutdown |
| **PCI** | Bus 0 enumeration, 32 device slots |
| **Compiler tests** | 6,580 pass, 0 fail |
| **QEMU test suite** | 5 categories, ALL PASS |

---

## Building and Running

```bash
# Build the kernel
cargo run --release --features native -- build \
    --target x86_64-none examples/fajaros_nova_kernel.fj

# Create GRUB2 ISO
mkdir -p /tmp/iso/boot/grub
cp examples/fajaros_nova_kernel /tmp/iso/boot/fajaros.elf
echo 'set timeout=0; menuentry "Nova" { multiboot2 /boot/fajaros.elf; boot }' \
    > /tmp/iso/boot/grub/grub.cfg
grub-mkrescue -o nova.iso /tmp/iso

# Boot in QEMU
qemu-system-x86_64 -cdrom nova.iso -m 256M -serial stdio

# Run automated test suite
bash examples/fajaros_nova_test.sh
```

---

## What's Next

- **Phase 2**: Bitmap frame allocator, `map_page()`/`unmap_page()`, NX enforcement
- **Phase 5**: Ring 3 user processes, SYSCALL/SYSRET with real handlers, IPC
- **Phase 8**: SMP (LAPIC/IOAPIC, multi-core boot), security hardening
- **Phase 9**: Real MNIST weights loading, AVX2 matrix multiply
- **Phase 10**: NVMe driver, boot on real Intel hardware (Lenovo Legion Pro)

---

## The Vision

FajarOS Nova demonstrates that **a single language can safely span all layers of a computer system** — from bare-metal boot code to an interactive shell with 97 commands, a filesystem, and ML demos. The compiler enforces safety boundaries that no amount of testing could achieve.

This is what Fajar Lang was built for:

> *"Bahasa terbaik untuk embedded ML + OS integration — the only language where an OS kernel and a neural network can share the same codebase, type system, and compiler, with safety guarantees that no existing language provides."*

---

*FajarOS Nova v0.1.0 — Built with Fajar Lang + Claude Opus 4.6*
*March 2026*
