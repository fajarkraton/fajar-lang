//! RISC-V bare metal support.
//!
//! Provides simulated drivers for RISC-V kernel development:
//! - SiFive UART driver
//! - PLIC (Platform-Level Interrupt Controller)
//! - Machine-mode trap handling
//! - CSR (Control and Status Register) abstraction

use thiserror::Error;

// ═══════════════════════════════════════════════════════════════════════
// Constants
// ═══════════════════════════════════════════════════════════════════════

/// SiFive UART base address (QEMU virt).
pub const UART0_BASE: u64 = 0x1000_0000;
/// PLIC base address (QEMU virt).
pub const PLIC_BASE: u64 = 0x0C00_0000;
/// UART0 IRQ number on PLIC.
pub const UART0_IRQ: u32 = 10;

// ═══════════════════════════════════════════════════════════════════════
// RISC-V errors
// ═══════════════════════════════════════════════════════════════════════

/// RISC-V peripheral errors.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum RiscvError {
    /// UART not initialized.
    #[error("UART not initialized")]
    UartNotInit,
    /// Invalid PLIC IRQ source.
    #[error("invalid PLIC IRQ source: {irq_source} (max 1023)")]
    InvalidIrqSource { irq_source: u32 },
    /// Invalid priority value.
    #[error("invalid priority: {priority} (max 7)")]
    InvalidPriority { priority: u32 },
}

// ═══════════════════════════════════════════════════════════════════════
// SiFive UART
// ═══════════════════════════════════════════════════════════════════════

/// Simulated SiFive UART (compatible with QEMU virt machine).
///
/// Simple MMIO UART with txdata/rxdata registers.
#[derive(Debug)]
pub struct SifiveUart {
    /// Base MMIO address.
    base: u64,
    /// Whether UART is initialized.
    initialized: bool,
    /// Output buffer.
    output: Vec<u8>,
    /// Input buffer.
    input: Vec<u8>,
}

impl SifiveUart {
    /// Create a UART at the given base address.
    pub fn new(base: u64) -> Self {
        Self {
            base,
            initialized: false,
            output: Vec::new(),
            input: Vec::new(),
        }
    }

    /// Create UART0 for QEMU virt machine.
    pub fn qemu_virt() -> Self {
        Self::new(UART0_BASE)
    }

    /// Initialize the UART.
    pub fn init(&mut self) {
        self.initialized = true;
    }

    /// Write a single byte.
    pub fn putc(&mut self, ch: u8) -> Result<(), RiscvError> {
        if !self.initialized {
            return Err(RiscvError::UartNotInit);
        }
        self.output.push(ch);
        Ok(())
    }

    /// Write a string.
    pub fn puts(&mut self, s: &str) -> Result<(), RiscvError> {
        for byte in s.bytes() {
            self.putc(byte)?;
        }
        Ok(())
    }

    /// Read a byte (non-blocking).
    pub fn getc(&mut self) -> Result<Option<u8>, RiscvError> {
        if !self.initialized {
            return Err(RiscvError::UartNotInit);
        }
        if self.input.is_empty() {
            Ok(None)
        } else {
            Ok(Some(self.input.remove(0)))
        }
    }

    /// Push data into input buffer.
    pub fn push_input(&mut self, data: &[u8]) {
        self.input.extend_from_slice(data);
    }

    /// Get captured output as string.
    pub fn output_string(&self) -> String {
        String::from_utf8_lossy(&self.output).to_string()
    }

    /// Whether UART is initialized.
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Get base address.
    pub fn base(&self) -> u64 {
        self.base
    }
}

// ═══════════════════════════════════════════════════════════════════════
// PLIC (Platform-Level Interrupt Controller)
// ═══════════════════════════════════════════════════════════════════════

/// Simulated RISC-V PLIC.
///
/// Supports up to 1024 interrupt sources with 8 priority levels.
/// Each source can be enabled/disabled per hart (CPU core) context.
#[derive(Debug)]
pub struct Plic {
    /// Base MMIO address.
    base: u64,
    /// Priority for each source (0-7, 0 = disabled).
    priorities: Vec<u32>,
    /// Enable bits per source.
    enabled: Vec<bool>,
    /// Pending interrupts.
    pending: Vec<bool>,
    /// Priority threshold.
    threshold: u32,
    /// Maximum sources.
    max_sources: u32,
}

impl Plic {
    /// Create a new PLIC with the given number of sources.
    pub fn new(base: u64, max_sources: u32) -> Self {
        let count = max_sources as usize;
        Self {
            base,
            priorities: vec![0; count],
            enabled: vec![false; count],
            pending: vec![false; count],
            threshold: 0,
            max_sources,
        }
    }

    /// Create for QEMU virt machine (1024 sources).
    pub fn qemu_virt() -> Self {
        Self::new(PLIC_BASE, 1024)
    }

    /// Set priority for an interrupt source.
    pub fn set_priority(&mut self, source: u32, priority: u32) -> Result<(), RiscvError> {
        if source >= self.max_sources {
            return Err(RiscvError::InvalidIrqSource { irq_source: source });
        }
        if priority > 7 {
            return Err(RiscvError::InvalidPriority { priority });
        }
        self.priorities[source as usize] = priority;
        Ok(())
    }

    /// Enable an interrupt source.
    pub fn enable(&mut self, source: u32) -> Result<(), RiscvError> {
        if source >= self.max_sources {
            return Err(RiscvError::InvalidIrqSource { irq_source: source });
        }
        self.enabled[source as usize] = true;
        Ok(())
    }

    /// Disable an interrupt source.
    pub fn disable(&mut self, source: u32) -> Result<(), RiscvError> {
        if source >= self.max_sources {
            return Err(RiscvError::InvalidIrqSource { irq_source: source });
        }
        self.enabled[source as usize] = false;
        Ok(())
    }

    /// Set the priority threshold.
    pub fn set_threshold(&mut self, threshold: u32) -> Result<(), RiscvError> {
        if threshold > 7 {
            return Err(RiscvError::InvalidPriority {
                priority: threshold,
            });
        }
        self.threshold = threshold;
        Ok(())
    }

    /// Trigger a pending interrupt (simulated).
    pub fn trigger(&mut self, source: u32) -> Result<(), RiscvError> {
        if source >= self.max_sources {
            return Err(RiscvError::InvalidIrqSource { irq_source: source });
        }
        self.pending[source as usize] = true;
        Ok(())
    }

    /// Claim the highest-priority pending interrupt.
    ///
    /// Returns the source number, or None if no interrupts pending.
    pub fn claim(&mut self) -> Option<u32> {
        let mut best_source = None;
        let mut best_priority = 0;

        for (i, (&pending, &enabled)) in self.pending.iter().zip(self.enabled.iter()).enumerate() {
            if pending && enabled {
                let priority = self.priorities[i];
                if priority > self.threshold && priority > best_priority {
                    best_priority = priority;
                    best_source = Some(i as u32);
                }
            }
        }

        if let Some(source) = best_source {
            self.pending[source as usize] = false;
        }

        best_source
    }

    /// Complete interrupt handling (acknowledge).
    pub fn complete(&mut self, _source: u32) {
        // In real hardware, writing to the complete register
        // signals the PLIC that the interrupt has been handled.
    }

    /// Get base address.
    pub fn base(&self) -> u64 {
        self.base
    }

    /// Get priority threshold.
    pub fn threshold(&self) -> u32 {
        self.threshold
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Machine-mode trap handling
// ═══════════════════════════════════════════════════════════════════════

/// RISC-V trap cause codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrapCause {
    /// User software interrupt.
    UserSoftware,
    /// Supervisor software interrupt.
    SupervisorSoftware,
    /// Machine software interrupt.
    MachineSoftware,
    /// User timer interrupt.
    UserTimer,
    /// Supervisor timer interrupt.
    SupervisorTimer,
    /// Machine timer interrupt.
    MachineTimer,
    /// User external interrupt.
    UserExternal,
    /// Supervisor external interrupt.
    SupervisorExternal,
    /// Machine external interrupt.
    MachineExternal,
    /// Instruction address misaligned.
    InstructionMisaligned,
    /// Illegal instruction.
    IllegalInstruction,
    /// Environment call from U-mode.
    EcallUser,
    /// Environment call from S-mode.
    EcallSupervisor,
    /// Environment call from M-mode.
    EcallMachine,
    /// Load page fault.
    LoadPageFault,
    /// Store page fault.
    StorePageFault,
}

/// Simulated trap handler table for machine mode.
#[derive(Debug)]
pub struct TrapTable {
    /// Handler names by trap cause.
    handlers: std::collections::HashMap<u8, String>,
    /// Trap dispatch log.
    log: Vec<TrapCause>,
}

impl TrapTable {
    /// Create a new empty trap table.
    pub fn new() -> Self {
        Self {
            handlers: std::collections::HashMap::new(),
            log: Vec::new(),
        }
    }

    /// Register a handler for a trap cause.
    pub fn set_handler(&mut self, cause: TrapCause, name: &str) {
        self.handlers
            .insert(Self::cause_code(cause), name.to_string());
    }

    /// Dispatch a trap, returning the handler name.
    pub fn dispatch(&mut self, cause: TrapCause) -> Option<&str> {
        self.log.push(cause);
        let code = Self::cause_code(cause);
        self.handlers.get(&code).map(|s| s.as_str())
    }

    /// Get the dispatch log.
    pub fn log(&self) -> &[TrapCause] {
        &self.log
    }

    /// Number of registered handlers.
    pub fn handler_count(&self) -> usize {
        self.handlers.len()
    }

    fn cause_code(cause: TrapCause) -> u8 {
        match cause {
            TrapCause::UserSoftware => 0,
            TrapCause::SupervisorSoftware => 1,
            TrapCause::MachineSoftware => 3,
            TrapCause::UserTimer => 4,
            TrapCause::SupervisorTimer => 5,
            TrapCause::MachineTimer => 7,
            TrapCause::UserExternal => 8,
            TrapCause::SupervisorExternal => 9,
            TrapCause::MachineExternal => 11,
            TrapCause::InstructionMisaligned => 0x80,
            TrapCause::IllegalInstruction => 0x82,
            TrapCause::EcallUser => 0x88,
            TrapCause::EcallSupervisor => 0x89,
            TrapCause::EcallMachine => 0x8B,
            TrapCause::LoadPageFault => 0x8D,
            TrapCause::StorePageFault => 0x8F,
        }
    }
}

impl Default for TrapTable {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn riscv_uart_init_and_write() {
        let mut uart = SifiveUart::qemu_virt();
        assert!(!uart.is_initialized());
        uart.init();
        uart.puts("Hello RISC-V!").unwrap();
        assert_eq!(uart.output_string(), "Hello RISC-V!");
    }

    #[test]
    fn riscv_uart_not_init() {
        let mut uart = SifiveUart::qemu_virt();
        assert_eq!(uart.putc(b'x'), Err(RiscvError::UartNotInit));
    }

    #[test]
    fn plic_priority_and_claim() {
        let mut plic = Plic::qemu_virt();
        plic.set_priority(UART0_IRQ, 5).unwrap();
        plic.enable(UART0_IRQ).unwrap();
        plic.trigger(UART0_IRQ).unwrap();

        let claimed = plic.claim();
        assert_eq!(claimed, Some(UART0_IRQ));
        // After claim, no more pending
        assert_eq!(plic.claim(), None);
    }

    #[test]
    fn plic_threshold_filters() {
        let mut plic = Plic::qemu_virt();
        plic.set_priority(UART0_IRQ, 3).unwrap();
        plic.enable(UART0_IRQ).unwrap();
        plic.set_threshold(5).unwrap(); // threshold > priority
        plic.trigger(UART0_IRQ).unwrap();

        // Should not claim (below threshold)
        assert_eq!(plic.claim(), None);
    }

    #[test]
    fn trap_table_dispatch() {
        let mut traps = TrapTable::new();
        traps.set_handler(TrapCause::MachineTimer, "timer_handler");
        traps.set_handler(TrapCause::MachineExternal, "external_handler");

        assert_eq!(
            traps.dispatch(TrapCause::MachineTimer),
            Some("timer_handler")
        );
        assert_eq!(traps.dispatch(TrapCause::EcallUser), None);
        assert_eq!(traps.log().len(), 2);
    }

    #[test]
    fn plic_invalid_source() {
        let mut plic = Plic::qemu_virt();
        assert_eq!(
            plic.set_priority(1024, 1),
            Err(RiscvError::InvalidIrqSource { irq_source: 1024 })
        );
    }
}
