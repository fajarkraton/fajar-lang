//! Mini OS on QEMU demo — x86_64/ARM64 boot, page table, interrupts,
//! serial console, VGA text mode, kernel panic, shell, build system.

use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// S39.1: x86_64 Boot
// ═══════════════════════════════════════════════════════════════════════

/// x86_64 boot stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootStage {
    /// BIOS/UEFI firmware.
    Firmware,
    /// Bootloader (GRUB/Limine).
    Bootloader,
    /// Long mode entry.
    LongMode,
    /// Kernel init.
    KernelInit,
    /// Kernel ready.
    Ready,
}

impl fmt::Display for BootStage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Firmware => write!(f, "FIRMWARE"),
            Self::Bootloader => write!(f, "BOOTLOADER"),
            Self::LongMode => write!(f, "LONG_MODE"),
            Self::KernelInit => write!(f, "KERNEL_INIT"),
            Self::Ready => write!(f, "READY"),
        }
    }
}

/// Simulates the x86_64 boot sequence.
pub fn boot_sequence_x86() -> Vec<(BootStage, String)> {
    vec![
        (
            BootStage::Firmware,
            "POST complete, loading bootloader".to_string(),
        ),
        (
            BootStage::Bootloader,
            "Limine v8.0 — loading kernel ELF".to_string(),
        ),
        (
            BootStage::LongMode,
            "GDT loaded, CR3 set, long mode active".to_string(),
        ),
        (
            BootStage::KernelInit,
            "IDT installed, serial initialized".to_string(),
        ),
        (BootStage::Ready, "FajarOS v1.1 ready".to_string()),
    ]
}

// ═══════════════════════════════════════════════════════════════════════
// S39.2: Page Table Setup
// ═══════════════════════════════════════════════════════════════════════

/// Page table entry flags.
#[derive(Debug, Clone, Copy)]
pub struct PageFlags {
    /// Present bit.
    pub present: bool,
    /// Writable.
    pub writable: bool,
    /// User-accessible.
    pub user: bool,
    /// Write-through caching.
    pub write_through: bool,
    /// Cache disabled.
    pub cache_disabled: bool,
    /// No-execute.
    pub no_execute: bool,
}

impl PageFlags {
    /// Kernel read-write flags.
    pub fn kernel_rw() -> Self {
        Self {
            present: true,
            writable: true,
            user: false,
            write_through: false,
            cache_disabled: false,
            no_execute: false,
        }
    }

    /// Kernel read-only flags.
    pub fn kernel_ro() -> Self {
        Self {
            present: true,
            writable: false,
            user: false,
            write_through: false,
            cache_disabled: false,
            no_execute: false,
        }
    }

    /// Converts flags to a raw u64 page table entry value.
    pub fn to_raw(&self) -> u64 {
        let mut flags = 0u64;
        if self.present {
            flags |= 1;
        }
        if self.writable {
            flags |= 1 << 1;
        }
        if self.user {
            flags |= 1 << 2;
        }
        if self.write_through {
            flags |= 1 << 3;
        }
        if self.cache_disabled {
            flags |= 1 << 4;
        }
        if self.no_execute {
            flags |= 1 << 63;
        }
        flags
    }
}

/// 4-level page table structure.
#[derive(Debug, Clone)]
pub struct PageTableSetup {
    /// Number of PML4 entries used.
    pub pml4_entries: usize,
    /// Identity-mapped region (bytes).
    pub identity_mapped_bytes: u64,
    /// Higher-half kernel base address.
    pub kernel_base: u64,
    /// Higher-half kernel size.
    pub kernel_size: u64,
}

impl Default for PageTableSetup {
    fn default() -> Self {
        Self {
            pml4_entries: 2,                               // Identity + higher-half
            identity_mapped_bytes: 4 * 1024 * 1024 * 1024, // 4 GB
            kernel_base: 0xFFFF_FFFF_8000_0000,            // Higher-half
            kernel_size: 2 * 1024 * 1024,                  // 2 MB
        }
    }
}

impl PageTableSetup {
    /// Returns a description of the page table layout.
    pub fn describe(&self) -> String {
        format!(
            "4-level paging: {} PML4 entries, identity-map first {}GB, kernel at {:#x}",
            self.pml4_entries,
            self.identity_mapped_bytes / (1024 * 1024 * 1024),
            self.kernel_base,
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S39.3: Interrupt Handler
// ═══════════════════════════════════════════════════════════════════════

/// x86_64 interrupt vector numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterruptVector {
    /// Division by zero (#DE, vector 0).
    DivideByZero,
    /// Page fault (#PF, vector 14).
    PageFault,
    /// Double fault (#DF, vector 8).
    DoubleFault,
    /// Keyboard IRQ (vector 33).
    Keyboard,
    /// Timer IRQ (vector 32).
    Timer,
}

impl InterruptVector {
    /// Returns the vector number.
    pub fn vector(&self) -> u8 {
        match self {
            Self::DivideByZero => 0,
            Self::PageFault => 14,
            Self::DoubleFault => 8,
            Self::Keyboard => 33,
            Self::Timer => 32,
        }
    }
}

/// IDT entry description.
#[derive(Debug, Clone)]
pub struct IdtEntry {
    /// Interrupt vector.
    pub vector: InterruptVector,
    /// Handler function name.
    pub handler: String,
    /// Whether this is an interrupt gate (vs trap gate).
    pub is_interrupt_gate: bool,
}

/// Returns the IDT configuration for the demo kernel.
pub fn idt_config() -> Vec<IdtEntry> {
    vec![
        IdtEntry {
            vector: InterruptVector::DivideByZero,
            handler: "divide_by_zero_handler".to_string(),
            is_interrupt_gate: true,
        },
        IdtEntry {
            vector: InterruptVector::PageFault,
            handler: "page_fault_handler".to_string(),
            is_interrupt_gate: true,
        },
        IdtEntry {
            vector: InterruptVector::DoubleFault,
            handler: "double_fault_handler".to_string(),
            is_interrupt_gate: true,
        },
        IdtEntry {
            vector: InterruptVector::Keyboard,
            handler: "keyboard_irq_handler".to_string(),
            is_interrupt_gate: true,
        },
        IdtEntry {
            vector: InterruptVector::Timer,
            handler: "timer_irq_handler".to_string(),
            is_interrupt_gate: true,
        },
    ]
}

// ═══════════════════════════════════════════════════════════════════════
// S39.4: Serial Console (UART 16550)
// ═══════════════════════════════════════════════════════════════════════

/// UART 16550 port addresses.
pub const COM1_BASE: u16 = 0x3F8;

/// Serial port configuration.
#[derive(Debug, Clone)]
pub struct SerialConfig {
    /// Base I/O port.
    pub port: u16,
    /// Baud rate.
    pub baud_rate: u32,
    /// Data bits (5, 6, 7, 8).
    pub data_bits: u8,
    /// Stop bits (1, 2).
    pub stop_bits: u8,
    /// Parity mode.
    pub parity: ParityMode,
}

/// Parity mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParityMode {
    /// No parity.
    None,
    /// Odd parity.
    Odd,
    /// Even parity.
    Even,
}

impl Default for SerialConfig {
    fn default() -> Self {
        Self {
            port: COM1_BASE,
            baud_rate: 115200,
            data_bits: 8,
            stop_bits: 1,
            parity: ParityMode::None,
        }
    }
}

impl SerialConfig {
    /// Returns the divisor for the baud rate generator.
    pub fn divisor(&self) -> u16 {
        (115200 / self.baud_rate) as u16
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S39.5: VGA Text Mode
// ═══════════════════════════════════════════════════════════════════════

/// VGA text color.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VgaColor {
    /// Black.
    Black = 0,
    /// Blue.
    Blue = 1,
    /// Green.
    Green = 2,
    /// Cyan.
    Cyan = 3,
    /// Red.
    Red = 4,
    /// Light grey.
    LightGrey = 7,
    /// White.
    White = 15,
}

/// VGA text buffer dimensions.
pub const VGA_WIDTH: usize = 80;
/// VGA text buffer height.
pub const VGA_HEIGHT: usize = 25;

/// VGA text buffer.
#[derive(Debug, Clone)]
pub struct VgaBuffer {
    /// Character data (row-major).
    chars: Vec<u8>,
    /// Color data (row-major).
    colors: Vec<u8>,
    /// Current cursor row.
    pub cursor_row: usize,
    /// Current cursor column.
    pub cursor_col: usize,
}

impl VgaBuffer {
    /// Creates a new empty VGA buffer.
    pub fn new() -> Self {
        Self {
            chars: vec![b' '; VGA_WIDTH * VGA_HEIGHT],
            colors: vec![0x07; VGA_WIDTH * VGA_HEIGHT], // light grey on black
            cursor_row: 0,
            cursor_col: 0,
        }
    }

    /// Writes a character at the cursor position.
    pub fn write_char(&mut self, ch: u8, fg: VgaColor, bg: VgaColor) {
        if ch == b'\n' {
            self.new_line();
            return;
        }

        let idx = self.cursor_row * VGA_WIDTH + self.cursor_col;
        if idx < self.chars.len() {
            self.chars[idx] = ch;
            self.colors[idx] = (bg as u8) << 4 | (fg as u8);
        }

        self.cursor_col += 1;
        if self.cursor_col >= VGA_WIDTH {
            self.new_line();
        }
    }

    /// Writes a string.
    pub fn write_str(&mut self, s: &str, fg: VgaColor, bg: VgaColor) {
        for ch in s.bytes() {
            self.write_char(ch, fg, bg);
        }
    }

    /// Moves to the next line, scrolling if needed.
    fn new_line(&mut self) {
        self.cursor_col = 0;
        self.cursor_row += 1;
        if self.cursor_row >= VGA_HEIGHT {
            self.scroll();
        }
    }

    /// Scrolls the buffer up by one line.
    fn scroll(&mut self) {
        // Shift all rows up
        for row in 1..VGA_HEIGHT {
            for col in 0..VGA_WIDTH {
                let dst = (row - 1) * VGA_WIDTH + col;
                let src = row * VGA_WIDTH + col;
                self.chars[dst] = self.chars[src];
                self.colors[dst] = self.colors[src];
            }
        }
        // Clear last row
        let last_start = (VGA_HEIGHT - 1) * VGA_WIDTH;
        for col in 0..VGA_WIDTH {
            self.chars[last_start + col] = b' ';
            self.colors[last_start + col] = 0x07;
        }
        self.cursor_row = VGA_HEIGHT - 1;
    }

    /// Returns the character at a position.
    pub fn char_at(&self, row: usize, col: usize) -> u8 {
        self.chars[row * VGA_WIDTH + col]
    }
}

impl Default for VgaBuffer {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S39.6: Kernel Panic Display
// ═══════════════════════════════════════════════════════════════════════

/// CPU register dump for panic display.
#[derive(Debug, Clone, Default)]
pub struct RegisterDump {
    /// General-purpose registers.
    pub rax: u64,
    pub rbx: u64,
    pub rcx: u64,
    pub rdx: u64,
    pub rsp: u64,
    pub rbp: u64,
    pub rip: u64,
    /// Flags register.
    pub rflags: u64,
    /// CR2 (page fault address).
    pub cr2: u64,
}

impl RegisterDump {
    /// Formats the register dump.
    pub fn format(&self) -> String {
        format!(
            "RAX={:#018x} RBX={:#018x} RCX={:#018x}\nRDX={:#018x} RSP={:#018x} RBP={:#018x}\nRIP={:#018x} RFLAGS={:#018x} CR2={:#018x}",
            self.rax, self.rbx, self.rcx,
            self.rdx, self.rsp, self.rbp,
            self.rip, self.rflags, self.cr2,
        )
    }
}

/// Kernel panic information.
#[derive(Debug, Clone)]
pub struct KernelPanic {
    /// Panic message.
    pub message: String,
    /// Register state at panic.
    pub registers: RegisterDump,
    /// Stack trace (list of return addresses).
    pub stack_trace: Vec<u64>,
}

impl KernelPanic {
    /// Formats the panic for VGA display.
    pub fn format_vga(&self) -> String {
        let mut out = String::new();
        out.push_str("!!! KERNEL PANIC !!!\n\n");
        out.push_str(&format!("  {}\n\n", self.message));
        out.push_str("Registers:\n");
        out.push_str(&format!("  {}\n\n", self.registers.format()));
        out.push_str("Stack trace:\n");
        for (i, addr) in self.stack_trace.iter().enumerate() {
            out.push_str(&format!("  #{i}: {addr:#018x}\n"));
        }
        out
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S39.7: ARM64 Boot
// ═══════════════════════════════════════════════════════════════════════

/// ARM64 boot sequence.
pub fn boot_sequence_arm64() -> Vec<(BootStage, String)> {
    vec![
        (BootStage::Firmware, "UEFI firmware, DTB loaded".to_string()),
        (
            BootStage::Bootloader,
            "Loading kernel Image at 0x40200000".to_string(),
        ),
        (
            BootStage::LongMode,
            "EL2 -> EL1, MMU configured, TTBR0/TTBR1 set".to_string(),
        ),
        (
            BootStage::KernelInit,
            "GICv3 initialized, PL011 UART ready".to_string(),
        ),
        (BootStage::Ready, "FajarOS v1.1 (aarch64) ready".to_string()),
    ]
}

// ═══════════════════════════════════════════════════════════════════════
// S39.8: Simple Shell
// ═══════════════════════════════════════════════════════════════════════

/// Shell built-in command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellCommand {
    /// Show help.
    Help,
    /// Show system info.
    Info,
    /// Reboot the system.
    Reboot,
    /// Show memory map.
    Mem,
    /// Clear screen.
    Clear,
    /// Unknown command.
    Unknown(String),
}

/// Parses a shell command string.
pub fn parse_shell_command(input: &str) -> ShellCommand {
    match input.trim() {
        "help" => ShellCommand::Help,
        "info" => ShellCommand::Info,
        "reboot" => ShellCommand::Reboot,
        "mem" => ShellCommand::Mem,
        "clear" => ShellCommand::Clear,
        other => ShellCommand::Unknown(other.to_string()),
    }
}

/// Executes a shell command and returns the output.
pub fn execute_shell_command(cmd: &ShellCommand) -> String {
    match cmd {
        ShellCommand::Help => {
            "Available commands: help, info, reboot, mem, clear".to_string()
        }
        ShellCommand::Info => {
            "FajarOS v1.1 \"Ascension\" | Arch: x86_64 | RAM: 128 MB".to_string()
        }
        ShellCommand::Reboot => "Rebooting...".to_string(),
        ShellCommand::Mem => {
            "Memory map:\n  0x00000000-0x00100000: BIOS (1 MB)\n  0x00100000-0x08000000: Kernel + Heap (127 MB)\n  0xFFFF800000000000: Higher-half kernel".to_string()
        }
        ShellCommand::Clear => "[CLEAR SCREEN]".to_string(),
        ShellCommand::Unknown(cmd) => format!("Unknown command: {cmd}"),
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S39.9: Build System
// ═══════════════════════════════════════════════════════════════════════

/// Build target for bare-metal OS.
#[derive(Debug, Clone)]
pub struct BareBuildConfig {
    /// Target architecture.
    pub arch: String,
    /// Output format.
    pub output: String,
    /// Linker script path.
    pub linker_script: String,
    /// Bootloader (GRUB or Limine).
    pub bootloader: String,
    /// No-std mode.
    pub no_std: bool,
}

impl BareBuildConfig {
    /// x86_64 bare-metal config.
    pub fn x86_64() -> Self {
        Self {
            arch: "x86_64".to_string(),
            output: "iso".to_string(),
            linker_script: "kernel/linker_x86_64.ld".to_string(),
            bootloader: "limine".to_string(),
            no_std: true,
        }
    }

    /// aarch64 bare-metal config.
    pub fn aarch64() -> Self {
        Self {
            arch: "aarch64".to_string(),
            output: "bin".to_string(),
            linker_script: "kernel/linker_aarch64.ld".to_string(),
            bootloader: "u-boot".to_string(),
            no_std: true,
        }
    }

    /// Returns the QEMU command to run this kernel.
    pub fn qemu_command(&self) -> String {
        match self.arch.as_str() {
            "x86_64" => format!(
                "qemu-system-x86_64 -cdrom build/fajeros.{} -serial stdio -no-reboot",
                self.output
            ),
            "aarch64" => {
                "qemu-system-aarch64 -M virt -cpu cortex-a76 -kernel build/fajeros.bin -serial stdio".to_string()
            }
            _ => "echo 'unsupported architecture'".to_string(),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S39.10: Video Script
// ═══════════════════════════════════════════════════════════════════════

/// Generates the mini OS video script.
pub fn video_script() -> String {
    [
        "# Mini OS Demo — Video Script (3 minutes)",
        "",
        "## 0:00-0:30 — Boot Sequence",
        "QEMU x86_64: show Limine bootloader, kernel loading, serial output.",
        "\"Written entirely in Fajar Lang — 0 lines of C.\"",
        "",
        "## 0:30-1:15 — Shell Interaction",
        "Type `help`, `info`, `mem` in the kernel shell.",
        "Show VGA text output with color.",
        "",
        "## 1:15-1:45 — Interrupt Handling",
        "Trigger a divide-by-zero: show exception handler output.",
        "Press keyboard: show IRQ handler receiving scancodes.",
        "",
        "## 1:45-2:15 — Kernel Panic",
        "Trigger a page fault at address 0xDEAD.",
        "Show full register dump, stack trace on red VGA background.",
        "",
        "## 2:15-2:45 — ARM64 Boot",
        "Switch to QEMU aarch64: boot same kernel, show serial output.",
        "\"Same source code, two architectures.\"",
        "",
        "## 2:45-3:00 — Conclusion",
        "\"A real OS kernel in Fajar Lang — @kernel context guarantees no heap, no tensor.\"",
    ]
    .join("\n")
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // S39.1: x86_64 boot
    #[test]
    fn s39_1_boot_sequence() {
        let seq = boot_sequence_x86();
        assert_eq!(seq.len(), 5);
        assert_eq!(seq[0].0, BootStage::Firmware);
        assert_eq!(seq[4].0, BootStage::Ready);
    }

    // S39.2: Page table
    #[test]
    fn s39_2_page_flags() {
        let flags = PageFlags::kernel_rw();
        let raw = flags.to_raw();
        assert_eq!(raw & 1, 1); // present
        assert_eq!(raw & 2, 2); // writable
        assert_eq!(raw & 4, 0); // not user

        let ro = PageFlags::kernel_ro();
        assert_eq!(ro.to_raw() & 2, 0); // not writable
    }

    #[test]
    fn s39_2_page_table_setup() {
        let setup = PageTableSetup::default();
        assert_eq!(setup.pml4_entries, 2);
        assert_eq!(setup.kernel_base, 0xFFFF_FFFF_8000_0000);
        let desc = setup.describe();
        assert!(desc.contains("4-level"));
    }

    // S39.3: Interrupts
    #[test]
    fn s39_3_idt_config() {
        let idt = idt_config();
        assert_eq!(idt.len(), 5);
        assert_eq!(idt[0].vector.vector(), 0);
        assert_eq!(idt[1].vector.vector(), 14);
        assert_eq!(idt[3].vector.vector(), 33);
    }

    // S39.4: Serial console
    #[test]
    fn s39_4_serial_config() {
        let cfg = SerialConfig::default();
        assert_eq!(cfg.baud_rate, 115200);
        assert_eq!(cfg.divisor(), 1);
    }

    // S39.5: VGA text mode
    #[test]
    fn s39_5_vga_buffer() {
        let mut vga = VgaBuffer::new();
        vga.write_str("Hello", VgaColor::White, VgaColor::Black);
        assert_eq!(vga.char_at(0, 0), b'H');
        assert_eq!(vga.char_at(0, 4), b'o');
        assert_eq!(vga.cursor_col, 5);
    }

    #[test]
    fn s39_5_vga_scroll() {
        let mut vga = VgaBuffer::new();
        for _ in 0..26 {
            vga.write_str("line\n", VgaColor::White, VgaColor::Black);
        }
        // Should have scrolled
        assert_eq!(vga.cursor_row, VGA_HEIGHT - 1);
    }

    // S39.6: Kernel panic
    #[test]
    fn s39_6_kernel_panic() {
        let panic = KernelPanic {
            message: "page fault at 0xDEAD".to_string(),
            registers: RegisterDump {
                rip: 0xFFFF_8000_0010_0000,
                cr2: 0xDEAD,
                ..Default::default()
            },
            stack_trace: vec![0xFFFF_8000_0010_0000, 0xFFFF_8000_0020_0000],
        };
        let output = panic.format_vga();
        assert!(output.contains("KERNEL PANIC"));
        assert!(output.contains("page fault"));
        assert!(output.contains("0x000000000000dead"));
    }

    // S39.7: ARM64 boot
    #[test]
    fn s39_7_arm64_boot() {
        let seq = boot_sequence_arm64();
        assert_eq!(seq.len(), 5);
        assert!(seq[2].1.contains("EL2"));
        assert!(seq[4].1.contains("aarch64"));
    }

    // S39.8: Shell
    #[test]
    fn s39_8_shell_parse() {
        assert_eq!(parse_shell_command("help"), ShellCommand::Help);
        assert_eq!(parse_shell_command("info"), ShellCommand::Info);
        assert_eq!(parse_shell_command("reboot"), ShellCommand::Reboot);
        assert_eq!(parse_shell_command("mem"), ShellCommand::Mem);
        assert_eq!(parse_shell_command("clear"), ShellCommand::Clear);
        assert!(matches!(
            parse_shell_command("foo"),
            ShellCommand::Unknown(_)
        ));
    }

    #[test]
    fn s39_8_shell_execute() {
        let help = execute_shell_command(&ShellCommand::Help);
        assert!(help.contains("help"));
        let info = execute_shell_command(&ShellCommand::Info);
        assert!(info.contains("FajarOS"));
        let unknown = execute_shell_command(&ShellCommand::Unknown("xyz".into()));
        assert!(unknown.contains("Unknown"));
    }

    // S39.9: Build system
    #[test]
    fn s39_9_build_config() {
        let x86 = BareBuildConfig::x86_64();
        assert_eq!(x86.arch, "x86_64");
        assert!(x86.no_std);
        assert!(x86.qemu_command().contains("qemu-system-x86_64"));

        let arm = BareBuildConfig::aarch64();
        assert!(arm.qemu_command().contains("qemu-system-aarch64"));
    }

    // S39.10: Video script
    #[test]
    fn s39_10_video_script() {
        let script = video_script();
        assert!(script.contains("Video Script"));
        assert!(script.contains("ARM64"));
        assert!(script.contains("Kernel Panic"));
    }
}
