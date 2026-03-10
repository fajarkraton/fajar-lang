//! OS runtime — memory, IRQ, and syscall primitives.
//!
//! Only accessible from `@kernel` or `@unsafe` context.

pub mod dma;
pub mod irq;
pub mod memory;
pub mod syscall;
pub mod timer;

pub use dma::{DmaController, DmaError};
pub use irq::{IrqError, IrqTable};
pub use memory::{MemoryError, MemoryManager, PageFlags, PageTable, PhysAddr, VirtAddr};
pub use syscall::{SyscallError, SyscallTable};
pub use timer::{TimerController, TimerError};

/// Combined OS runtime state.
///
/// Holds all OS subsystems that the interpreter uses when executing
/// `@kernel` or `@unsafe` code.
#[derive(Debug)]
pub struct OsRuntime {
    /// Memory manager (heap + page table).
    pub memory: MemoryManager,
    /// Interrupt request table.
    pub irq: IrqTable,
    /// System call table.
    pub syscall: SyscallTable,
    /// Port I/O subsystem.
    pub port_io: PortIO,
    /// DMA controller.
    pub dma: DmaController,
    /// Timer/PWM controller.
    pub timer: TimerController,
}

impl OsRuntime {
    /// Creates a new OS runtime with default settings.
    pub fn new() -> Self {
        Self {
            memory: MemoryManager::with_default_size(),
            irq: IrqTable::new(),
            syscall: SyscallTable::new(),
            port_io: PortIO::new(),
            dma: DmaController::new(4),
            timer: TimerController::new(4, 1_000_000),
        }
    }
}

impl Default for OsRuntime {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Port I/O (Sprint 3.5)
// ═══════════════════════════════════════════════════════════════════════

/// Simulated x86 port I/O subsystem.
///
/// Provides `port_read` and `port_write` for simulated hardware ports.
/// Some standard ports have pre-configured behavior (serial, keyboard status).
#[derive(Debug)]
pub struct PortIO {
    /// Port values (port number → last written value).
    ports: std::collections::HashMap<u16, u8>,
}

/// Standard serial port (COM1 data register).
pub const PORT_COM1_DATA: u16 = 0x3F8;
/// COM1 line status register.
pub const PORT_COM1_STATUS: u16 = 0x3FD;
/// Keyboard data port.
pub const PORT_KEYBOARD_DATA: u16 = 0x60;
/// Keyboard status port.
pub const PORT_KEYBOARD_STATUS: u16 = 0x64;

impl PortIO {
    /// Creates a new port I/O subsystem.
    pub fn new() -> Self {
        let mut ports = std::collections::HashMap::new();
        // COM1 line status: transmitter empty (bit 5) + ready (bit 0)
        ports.insert(PORT_COM1_STATUS, 0x21);
        // Keyboard status: output buffer empty
        ports.insert(PORT_KEYBOARD_STATUS, 0x00);
        Self { ports }
    }

    /// Writes a byte to the given port.
    pub fn write(&mut self, port: u16, value: u8) {
        self.ports.insert(port, value);
    }

    /// Reads a byte from the given port.
    pub fn read(&self, port: u16) -> u8 {
        self.ports.get(&port).copied().unwrap_or(0xFF)
    }
}

impl Default for PortIO {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn os_runtime_creates_all_subsystems() {
        let rt = OsRuntime::new();
        assert_eq!(rt.memory.size(), memory::DEFAULT_MEMORY_SIZE);
        assert!(!rt.irq.is_enabled());
        assert_eq!(rt.syscall.syscall_count(), 0);
    }

    // ── Port I/O ──

    #[test]
    fn port_write_and_read() {
        let mut pio = PortIO::new();
        pio.write(0x80, 0x42);
        assert_eq!(pio.read(0x80), 0x42);
    }

    #[test]
    fn port_read_unset_returns_ff() {
        let pio = PortIO::new();
        assert_eq!(pio.read(0x99), 0xFF);
    }

    #[test]
    fn port_com1_status_default() {
        let pio = PortIO::new();
        let status = pio.read(PORT_COM1_STATUS);
        assert_ne!(status, 0xFF); // Should have default value
        assert!(status & 0x20 != 0); // Transmitter empty bit set
    }

    #[test]
    fn port_serial_write_data() {
        let mut pio = PortIO::new();
        pio.write(PORT_COM1_DATA, b'A');
        assert_eq!(pio.read(PORT_COM1_DATA), b'A');
    }
}
