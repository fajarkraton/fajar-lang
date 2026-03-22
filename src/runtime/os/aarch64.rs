//! AArch64 (ARM64) bare metal support.
//!
//! Provides simulated drivers for ARM64 kernel development:
//! - UART PL011 serial interface
//! - GPIO controller
//! - ARM generic timer
//! - Exception vector table

use thiserror::Error;

// ═══════════════════════════════════════════════════════════════════════
// Constants
// ═══════════════════════════════════════════════════════════════════════

/// UART PL011 base address (Raspberry Pi 3).
pub const UART0_BASE: u64 = 0x3F20_1000;
/// GPIO base address (Raspberry Pi 3).
pub const GPIO_BASE: u64 = 0x3F20_0000;
/// ARM Generic Timer frequency (typically 62.5 MHz on RPi3).
pub const TIMER_FREQ: u64 = 62_500_000;

// ═══════════════════════════════════════════════════════════════════════
// AArch64 errors
// ═══════════════════════════════════════════════════════════════════════

/// AArch64 peripheral errors.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum Aarch64Error {
    /// Invalid GPIO pin number.
    #[error("invalid GPIO pin: {pin} (max 53)")]
    InvalidPin { pin: u8 },
    /// UART not initialized.
    #[error("UART not initialized")]
    UartNotInit,
    /// Process table is full (max 16 EL0 processes).
    #[error("EL0 process table full (max {MAX_EL0_PROCESSES})")]
    ProcessTableFull,
}

// ═══════════════════════════════════════════════════════════════════════
// UART PL011
// ═══════════════════════════════════════════════════════════════════════

/// Simulated UART PL011 serial interface.
///
/// Models the ARM PL011 UART used on Raspberry Pi and many ARM SoCs.
/// Supports init, putc, getc, and string output.
#[derive(Debug)]
pub struct UartPl011 {
    /// Base MMIO address.
    base: u64,
    /// Whether UART has been initialized.
    initialized: bool,
    /// Baud rate.
    baud_rate: u32,
    /// Output buffer (captured bytes).
    output: Vec<u8>,
    /// Input buffer (simulated received bytes).
    input: Vec<u8>,
}

impl UartPl011 {
    /// Create a UART at the given base address.
    pub fn new(base: u64) -> Self {
        Self {
            base,
            initialized: false,
            baud_rate: 115200,
            output: Vec::new(),
            input: Vec::new(),
        }
    }

    /// Create UART0 for Raspberry Pi 3.
    pub fn rpi3() -> Self {
        Self::new(UART0_BASE)
    }

    /// Initialize the UART (115200 baud, 8N1).
    pub fn init(&mut self) {
        self.initialized = true;
    }

    /// Initialize with custom baud rate.
    pub fn init_with_baud(&mut self, baud: u32) {
        self.baud_rate = baud;
        self.initialized = true;
    }

    /// Write a single character.
    pub fn putc(&mut self, ch: u8) -> Result<(), Aarch64Error> {
        if !self.initialized {
            return Err(Aarch64Error::UartNotInit);
        }
        self.output.push(ch);
        Ok(())
    }

    /// Write a string.
    pub fn puts(&mut self, s: &str) -> Result<(), Aarch64Error> {
        for byte in s.bytes() {
            self.putc(byte)?;
        }
        Ok(())
    }

    /// Read a character (non-blocking).
    pub fn getc(&mut self) -> Result<Option<u8>, Aarch64Error> {
        if !self.initialized {
            return Err(Aarch64Error::UartNotInit);
        }
        if self.input.is_empty() {
            Ok(None)
        } else {
            Ok(Some(self.input.remove(0)))
        }
    }

    /// Push data into input buffer (simulated receive).
    pub fn push_input(&mut self, data: &[u8]) {
        self.input.extend_from_slice(data);
    }

    /// Get captured output as string.
    pub fn output_string(&self) -> String {
        String::from_utf8_lossy(&self.output).to_string()
    }

    /// Clear output buffer.
    pub fn clear_output(&mut self) {
        self.output.clear();
    }

    /// Whether UART is initialized.
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Get base address.
    pub fn base(&self) -> u64 {
        self.base
    }

    /// Get baud rate.
    pub fn baud_rate(&self) -> u32 {
        self.baud_rate
    }
}

// ═══════════════════════════════════════════════════════════════════════
// GPIO
// ═══════════════════════════════════════════════════════════════════════

/// GPIO pin mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpioMode {
    /// Input mode.
    Input,
    /// Output mode.
    Output,
    /// Alternate function 0-5.
    AltFn(u8),
}

/// Simulated GPIO controller.
///
/// Models the BCM2837 GPIO on Raspberry Pi 3 (54 pins).
#[derive(Debug)]
pub struct GpioController {
    /// Pin modes (54 pins max).
    modes: Vec<GpioMode>,
    /// Pin output values.
    values: Vec<bool>,
    /// Maximum pin count.
    max_pins: u8,
}

impl GpioController {
    /// Create a new GPIO controller.
    pub fn new(max_pins: u8) -> Self {
        let count = max_pins as usize;
        Self {
            modes: vec![GpioMode::Input; count],
            values: vec![false; count],
            max_pins,
        }
    }

    /// Create for Raspberry Pi 3 (54 GPIO pins).
    pub fn rpi3() -> Self {
        Self::new(54)
    }

    /// Set pin mode.
    pub fn set_mode(&mut self, pin: u8, mode: GpioMode) -> Result<(), Aarch64Error> {
        if pin >= self.max_pins {
            return Err(Aarch64Error::InvalidPin { pin });
        }
        self.modes[pin as usize] = mode;
        Ok(())
    }

    /// Read pin value.
    pub fn read_pin(&self, pin: u8) -> Result<bool, Aarch64Error> {
        if pin >= self.max_pins {
            return Err(Aarch64Error::InvalidPin { pin });
        }
        Ok(self.values[pin as usize])
    }

    /// Write pin value (only works in Output mode).
    pub fn write_pin(&mut self, pin: u8, value: bool) -> Result<(), Aarch64Error> {
        if pin >= self.max_pins {
            return Err(Aarch64Error::InvalidPin { pin });
        }
        self.values[pin as usize] = value;
        Ok(())
    }

    /// Get pin mode.
    pub fn get_mode(&self, pin: u8) -> Result<GpioMode, Aarch64Error> {
        if pin >= self.max_pins {
            return Err(Aarch64Error::InvalidPin { pin });
        }
        Ok(self.modes[pin as usize])
    }

    /// Set pin input value (for simulation/testing).
    pub fn sim_set_input(&mut self, pin: u8, value: bool) -> Result<(), Aarch64Error> {
        if pin >= self.max_pins {
            return Err(Aarch64Error::InvalidPin { pin });
        }
        self.values[pin as usize] = value;
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// ARM Generic Timer
// ═══════════════════════════════════════════════════════════════════════

/// Simulated ARM Generic Timer.
///
/// Uses the CNTPCT_EL0 and CNTP_TVAL_EL0 system registers.
#[derive(Debug)]
pub struct ArmTimer {
    /// Timer frequency in Hz.
    frequency: u64,
    /// Current tick count (simulated CNTPCT_EL0).
    counter: u64,
    /// Timer compare value.
    compare: u64,
    /// Whether the timer interrupt is enabled.
    enabled: bool,
    /// Whether the timer has fired.
    fired: bool,
}

impl ArmTimer {
    /// Create a new ARM generic timer.
    pub fn new(frequency: u64) -> Self {
        Self {
            frequency,
            counter: 0,
            compare: 0,
            enabled: false,
            fired: false,
        }
    }

    /// Create with standard RPi3 frequency.
    pub fn rpi3() -> Self {
        Self::new(TIMER_FREQ)
    }

    /// Enable the timer with a compare value (ticks until interrupt).
    pub fn enable(&mut self, ticks: u64) {
        self.compare = self.counter + ticks;
        self.enabled = true;
        self.fired = false;
    }

    /// Disable the timer.
    pub fn disable(&mut self) {
        self.enabled = false;
    }

    /// Advance the counter by the given number of ticks.
    pub fn advance(&mut self, ticks: u64) {
        self.counter += ticks;
        if self.enabled && self.counter >= self.compare {
            self.fired = true;
        }
    }

    /// Check and clear the fired flag.
    pub fn check_fired(&mut self) -> bool {
        if self.fired {
            self.fired = false;
            true
        } else {
            false
        }
    }

    /// Set a periodic delay in microseconds.
    pub fn delay_us(&self, us: u64) -> u64 {
        us * self.frequency / 1_000_000
    }

    /// Set a periodic delay in milliseconds.
    pub fn delay_ms(&self, ms: u64) -> u64 {
        ms * self.frequency / 1_000
    }

    /// Get the current counter value.
    pub fn counter(&self) -> u64 {
        self.counter
    }

    /// Get the frequency.
    pub fn frequency(&self) -> u64 {
        self.frequency
    }

    /// Whether the timer is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Exception vectors
// ═══════════════════════════════════════════════════════════════════════

/// AArch64 exception types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExceptionType {
    /// Synchronous exception (e.g., SVC, data abort).
    Synchronous,
    /// IRQ (normal interrupt).
    Irq,
    /// FIQ (fast interrupt).
    Fiq,
    /// SError (system error).
    SError,
}

/// AArch64 exception vector table.
///
/// Contains 16 entries (4 exception types × 4 exception levels).
#[derive(Debug)]
pub struct ExceptionVectorTable {
    /// Handler names indexed by (level, type).
    handlers: Vec<Option<String>>,
}

impl ExceptionVectorTable {
    /// Create an empty exception vector table.
    pub fn new() -> Self {
        Self {
            handlers: vec![None; 16],
        }
    }

    /// Set a handler for the given exception level and type.
    ///
    /// Level: 0 = current EL with SP_EL0, 1 = current EL with SP_ELx,
    /// 2 = lower EL (AArch64), 3 = lower EL (AArch32).
    pub fn set_handler(&mut self, level: u8, exc_type: ExceptionType, name: &str) {
        let idx = self.index(level, exc_type);
        self.handlers[idx] = Some(name.to_string());
    }

    /// Get the handler for the given level and type.
    pub fn get_handler(&self, level: u8, exc_type: ExceptionType) -> Option<&str> {
        let idx = self.index(level, exc_type);
        self.handlers[idx].as_deref()
    }

    /// Number of registered handlers.
    pub fn handler_count(&self) -> usize {
        self.handlers.iter().filter(|h| h.is_some()).count()
    }

    fn index(&self, level: u8, exc_type: ExceptionType) -> usize {
        let type_idx = match exc_type {
            ExceptionType::Synchronous => 0,
            ExceptionType::Irq => 1,
            ExceptionType::Fiq => 2,
            ExceptionType::SError => 3,
        };
        (level.min(3) as usize) * 4 + type_idx
    }
}

impl Default for ExceptionVectorTable {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// EL0 User Space Process Model
// ═══════════════════════════════════════════════════════════════════════

/// Context frame size in bytes (saved by SAVE_CONTEXT macro).
///
/// Layout: 30 GP regs (x0-x29) + LR + ELR_EL1 + SPSR_EL1 + SP_EL0 = 288 bytes.
pub const CONTEXT_FRAME_SIZE: usize = 288;

/// Offset of SP_EL0 in the saved context frame.
pub const CONTEXT_SP_EL0_OFFSET: usize = 264;

/// Offset of SPSR_EL1 in the saved context frame.
pub const CONTEXT_SPSR_OFFSET: usize = 256;

/// Offset of ELR_EL1 in the saved context frame.
pub const CONTEXT_ELR_OFFSET: usize = 248;

/// Maximum number of EL0 processes.
pub const MAX_EL0_PROCESSES: usize = 16;

/// Default user stack size (64 KB per process).
pub const USER_STACK_SIZE: u64 = 64 * 1024;

/// Default kernel stack size per process (16 KB).
pub const KERNEL_STACK_SIZE: u64 = 16 * 1024;

/// SPSR value for EL0t with interrupts enabled.
pub const SPSR_EL0T: u64 = 0x0000_0000;

/// EL0 process state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum El0State {
    /// Process slot is unused.
    Free,
    /// Process is ready to run.
    Ready,
    /// Process is currently running on CPU.
    Running,
    /// Process is blocked (waiting for syscall/event).
    Blocked,
    /// Process has exited.
    Exited(i64),
}

/// ARM64 page table Access Permission bits for EL0.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageAccess {
    /// RW at EL1, no access at EL0 (kernel-only page).
    KernelRW,
    /// RW at both EL1 and EL0 (user read-write page).
    UserRW,
    /// RO at both EL1 and EL0 (user read-only page).
    UserRO,
    /// No access from EL0, RO from EL1.
    KernelRO,
}

impl PageAccess {
    /// Convert to AP[2:1] bits for page table descriptor.
    ///
    /// ARM64 Stage 1 AP encoding:
    ///   AP[2:1] = 0b00 → EL1 RW, EL0 RW
    ///   AP[2:1] = 0b01 → EL1 RW, EL0 no access
    ///   AP[2:1] = 0b10 → EL1 RO, EL0 RO
    ///   AP[2:1] = 0b11 → EL1 RO, EL0 no access
    pub fn to_ap_bits(self) -> u64 {
        match self {
            PageAccess::UserRW => 0b00 << 6,    // AP[2:1] = 00
            PageAccess::KernelRW => 0b01 << 6,  // AP[2:1] = 01
            PageAccess::UserRO => 0b10 << 6,    // AP[2:1] = 10
            PageAccess::KernelRO => 0b11 << 6,  // AP[2:1] = 11
        }
    }

    /// Check if this page is accessible from EL0.
    pub fn is_user_accessible(self) -> bool {
        matches!(self, PageAccess::UserRW | PageAccess::UserRO)
    }
}

/// An EL0 user-space process.
///
/// Represents a process that runs at Exception Level 0 (unprivileged).
/// The kernel manages its page tables, stacks, and saved context.
#[derive(Debug, Clone)]
pub struct El0Process {
    /// Process ID (1-based, 0 = invalid).
    pub pid: u16,
    /// Process state.
    pub state: El0State,
    /// Entry point address (ELR_EL1 for first ERET).
    pub entry: u64,
    /// User stack pointer (SP_EL0).
    pub user_sp: u64,
    /// User stack base (bottom of allocated stack region).
    pub user_stack_base: u64,
    /// Kernel stack pointer (used when handling exceptions from this process).
    pub kernel_sp: u64,
    /// Kernel stack base.
    pub kernel_stack_base: u64,
    /// TTBR0 value (physical address of user page table L0).
    pub ttbr0: u64,
    /// Saved context frame pointer (points into kernel stack).
    /// After preemption, this holds the SP at exception entry.
    pub saved_sp: u64,
    /// Exit code (valid when state == Exited).
    pub exit_code: i64,
}

impl El0Process {
    /// Create a new EL0 process.
    ///
    /// - `pid`: Process ID
    /// - `entry`: Code entry point (will be loaded into ELR_EL1)
    /// - `user_stack_top`: Top of user stack (grows downward)
    /// - `kernel_stack_top`: Top of kernel stack for exception handling
    /// - `ttbr0`: Physical address of per-process page table
    pub fn new(
        pid: u16,
        entry: u64,
        user_stack_top: u64,
        user_stack_base: u64,
        kernel_stack_top: u64,
        kernel_stack_base: u64,
        ttbr0: u64,
    ) -> Self {
        Self {
            pid,
            state: El0State::Ready,
            entry,
            user_sp: user_stack_top,
            user_stack_base,
            kernel_sp: kernel_stack_top,
            kernel_stack_base,
            ttbr0,
            saved_sp: 0,
            exit_code: 0,
        }
    }

    /// Get the SPSR value for entering this process at EL0.
    pub fn spsr(&self) -> u64 {
        SPSR_EL0T // M[3:0]=0000 (EL0t), all interrupts enabled
    }
}

/// Process table for EL0 processes.
///
/// Manages up to `MAX_EL0_PROCESSES` user-space processes.
/// The kernel uses this table for scheduling, context switching,
/// and syscall dispatch.
#[derive(Debug)]
pub struct El0ProcessTable {
    /// Process slots (index 0 = PID 1).
    processes: Vec<Option<El0Process>>,
    /// Currently running PID (0 = none/kernel).
    current_pid: u16,
}

impl El0ProcessTable {
    /// Create an empty process table.
    pub fn new() -> Self {
        let mut processes = Vec::with_capacity(MAX_EL0_PROCESSES);
        for _ in 0..MAX_EL0_PROCESSES {
            processes.push(None);
        }
        Self {
            processes,
            current_pid: 0,
        }
    }

    /// Spawn a new EL0 process. Returns PID on success.
    pub fn spawn(
        &mut self,
        entry: u64,
        user_stack_top: u64,
        user_stack_base: u64,
        kernel_stack_top: u64,
        kernel_stack_base: u64,
        ttbr0: u64,
    ) -> Result<u16, Aarch64Error> {
        // Find free slot
        for (i, slot) in self.processes.iter_mut().enumerate() {
            if slot.is_none() {
                let pid = (i + 1) as u16;
                *slot = Some(El0Process::new(
                    pid,
                    entry,
                    user_stack_top,
                    user_stack_base,
                    kernel_stack_top,
                    kernel_stack_base,
                    ttbr0,
                ));
                return Ok(pid);
            }
        }
        Err(Aarch64Error::ProcessTableFull)
    }

    /// Get a process by PID.
    pub fn get(&self, pid: u16) -> Option<&El0Process> {
        if pid == 0 || pid as usize > self.processes.len() {
            return None;
        }
        self.processes[(pid - 1) as usize].as_ref()
    }

    /// Get a mutable process by PID.
    pub fn get_mut(&mut self, pid: u16) -> Option<&mut El0Process> {
        if pid == 0 || pid as usize > self.processes.len() {
            return None;
        }
        self.processes[(pid - 1) as usize].as_mut()
    }

    /// Get the currently running PID.
    pub fn current_pid(&self) -> u16 {
        self.current_pid
    }

    /// Set the currently running PID.
    pub fn set_current(&mut self, pid: u16) {
        self.current_pid = pid;
    }

    /// Mark a process as exited.
    pub fn exit(&mut self, pid: u16, code: i64) -> bool {
        if let Some(proc) = self.get_mut(pid) {
            proc.state = El0State::Exited(code);
            proc.exit_code = code;
            if self.current_pid == pid {
                self.current_pid = 0;
            }
            true
        } else {
            false
        }
    }

    /// Remove an exited process, freeing the slot.
    pub fn reap(&mut self, pid: u16) -> bool {
        if pid == 0 || pid as usize > self.processes.len() {
            return false;
        }
        let idx = (pid - 1) as usize;
        if let Some(proc) = &self.processes[idx] {
            if matches!(proc.state, El0State::Exited(_)) {
                self.processes[idx] = None;
                return true;
            }
        }
        false
    }

    /// Count of active (non-exited, non-free) processes.
    pub fn active_count(&self) -> usize {
        self.processes
            .iter()
            .filter(|s| {
                s.as_ref()
                    .map(|p| !matches!(p.state, El0State::Exited(_)))
                    .unwrap_or(false)
            })
            .count()
    }

    /// Get next ready process for round-robin scheduling.
    pub fn next_ready(&self, after_pid: u16) -> Option<u16> {
        let start = after_pid as usize; // 0-based after PID
        for offset in 1..=MAX_EL0_PROCESSES {
            let idx = (start + offset - 1) % MAX_EL0_PROCESSES;
            if let Some(proc) = &self.processes[idx] {
                if proc.state == El0State::Ready {
                    return Some(proc.pid);
                }
            }
        }
        None
    }
}

impl Default for El0ProcessTable {
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

    // UART tests
    #[test]
    fn uart_init_and_write() {
        let mut uart = UartPl011::rpi3();
        assert!(!uart.is_initialized());

        uart.init();
        assert!(uart.is_initialized());

        uart.puts("Hello ARM!").unwrap();
        assert_eq!(uart.output_string(), "Hello ARM!");
    }

    #[test]
    fn uart_not_init_error() {
        let mut uart = UartPl011::rpi3();
        assert_eq!(uart.putc(b'x'), Err(Aarch64Error::UartNotInit));
    }

    #[test]
    fn uart_input() {
        let mut uart = UartPl011::rpi3();
        uart.init();
        uart.push_input(b"AB");
        assert_eq!(uart.getc().unwrap(), Some(b'A'));
        assert_eq!(uart.getc().unwrap(), Some(b'B'));
        assert_eq!(uart.getc().unwrap(), None);
    }

    // GPIO tests
    #[test]
    fn gpio_set_mode_and_write() {
        let mut gpio = GpioController::rpi3();
        gpio.set_mode(17, GpioMode::Output).unwrap();
        assert_eq!(gpio.get_mode(17).unwrap(), GpioMode::Output);

        gpio.write_pin(17, true).unwrap();
        assert!(gpio.read_pin(17).unwrap());
    }

    #[test]
    fn gpio_invalid_pin() {
        let gpio = GpioController::rpi3();
        assert_eq!(gpio.read_pin(54), Err(Aarch64Error::InvalidPin { pin: 54 }));
    }

    #[test]
    fn gpio_input_simulation() {
        let mut gpio = GpioController::rpi3();
        gpio.sim_set_input(5, true).unwrap();
        assert!(gpio.read_pin(5).unwrap());
    }

    // Timer tests
    #[test]
    fn arm_timer_delay() {
        let timer = ArmTimer::rpi3();
        let ticks = timer.delay_ms(1);
        assert_eq!(ticks, 62_500); // 62.5MHz * 1ms
    }

    #[test]
    fn arm_timer_fire() {
        let mut timer = ArmTimer::rpi3();
        timer.enable(1000);
        assert!(timer.is_enabled());

        timer.advance(999);
        assert!(!timer.check_fired());

        timer.advance(1);
        assert!(timer.check_fired());
        // Second check should be false (cleared)
        assert!(!timer.check_fired());
    }

    // Exception vector tests
    #[test]
    fn exception_vector_table() {
        let mut vt = ExceptionVectorTable::new();
        vt.set_handler(1, ExceptionType::Irq, "irq_handler");
        vt.set_handler(1, ExceptionType::Synchronous, "sync_handler");

        assert_eq!(vt.get_handler(1, ExceptionType::Irq), Some("irq_handler"));
        assert_eq!(
            vt.get_handler(1, ExceptionType::Synchronous),
            Some("sync_handler")
        );
        assert_eq!(vt.get_handler(0, ExceptionType::Irq), None);
        assert_eq!(vt.handler_count(), 2);
    }

    // ═══════════════════════════════════════════════════════════════
    // EL0 Process Model tests
    // ═══════════════════════════════════════════════════════════════

    #[test]
    fn el0_process_creation() {
        let proc = El0Process::new(
            1,              // pid
            0x1_0000,       // entry
            0x8_0000,       // user_stack_top
            0x7_0000,       // user_stack_base
            0x4_8000,       // kernel_stack_top
            0x4_4000,       // kernel_stack_base
            0x20_0000,      // ttbr0
        );
        assert_eq!(proc.pid, 1);
        assert_eq!(proc.state, El0State::Ready);
        assert_eq!(proc.entry, 0x1_0000);
        assert_eq!(proc.user_sp, 0x8_0000);
        assert_eq!(proc.kernel_sp, 0x4_8000);
        assert_eq!(proc.ttbr0, 0x20_0000);
        assert_eq!(proc.spsr(), SPSR_EL0T); // EL0t, IRQs enabled
    }

    #[test]
    fn el0_process_table_spawn() {
        let mut table = El0ProcessTable::new();
        assert_eq!(table.active_count(), 0);

        let pid = table.spawn(0x1000, 0x8000, 0x7000, 0x4800, 0x4400, 0x20000).unwrap();
        assert_eq!(pid, 1);
        assert_eq!(table.active_count(), 1);

        let proc = table.get(pid).unwrap();
        assert_eq!(proc.entry, 0x1000);
        assert_eq!(proc.state, El0State::Ready);
    }

    #[test]
    fn el0_process_table_multiple_spawn() {
        let mut table = El0ProcessTable::new();

        let pid1 = table.spawn(0x1000, 0x8000, 0x7000, 0x4800, 0x4400, 0x20000).unwrap();
        let pid2 = table.spawn(0x2000, 0x9000, 0x8000, 0x5800, 0x5400, 0x30000).unwrap();
        let pid3 = table.spawn(0x3000, 0xA000, 0x9000, 0x6800, 0x6400, 0x40000).unwrap();

        assert_eq!(pid1, 1);
        assert_eq!(pid2, 2);
        assert_eq!(pid3, 3);
        assert_eq!(table.active_count(), 3);
    }

    #[test]
    fn el0_process_table_full() {
        let mut table = El0ProcessTable::new();

        for i in 0..MAX_EL0_PROCESSES {
            let pid = table.spawn(
                (i as u64 + 1) * 0x1000,
                0x8_0000 + (i as u64) * 0x1_0000,
                0x7_0000 + (i as u64) * 0x1_0000,
                0x4_0000 + (i as u64) * 0x4000,
                0x3_C000 + (i as u64) * 0x4000,
                (i as u64 + 1) * 0x20_0000,
            ).unwrap();
            assert_eq!(pid, (i + 1) as u16);
        }

        // 17th spawn should fail
        let result = table.spawn(0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF);
        assert_eq!(result, Err(Aarch64Error::ProcessTableFull));
    }

    #[test]
    fn el0_process_exit_and_reap() {
        let mut table = El0ProcessTable::new();
        let pid = table.spawn(0x1000, 0x8000, 0x7000, 0x4800, 0x4400, 0x20000).unwrap();

        // Set as running
        table.set_current(pid);
        table.get_mut(pid).unwrap().state = El0State::Running;
        assert_eq!(table.current_pid(), pid);

        // Exit
        assert!(table.exit(pid, 42));
        assert_eq!(table.current_pid(), 0); // auto-cleared
        assert_eq!(table.get(pid).unwrap().state, El0State::Exited(42));
        assert_eq!(table.get(pid).unwrap().exit_code, 42);

        // Reap
        assert!(table.reap(pid));
        assert!(table.get(pid).is_none());
    }

    #[test]
    fn el0_process_reap_only_exited() {
        let mut table = El0ProcessTable::new();
        let pid = table.spawn(0x1000, 0x8000, 0x7000, 0x4800, 0x4400, 0x20000).unwrap();

        // Cannot reap a Ready process
        assert!(!table.reap(pid));

        // Can reap after exit
        table.exit(pid, 0);
        assert!(table.reap(pid));
    }

    #[test]
    fn el0_process_next_ready_round_robin() {
        let mut table = El0ProcessTable::new();
        let pid1 = table.spawn(0x1000, 0x8000, 0x7000, 0x4800, 0x4400, 0x20000).unwrap();
        let pid2 = table.spawn(0x2000, 0x9000, 0x8000, 0x5800, 0x5400, 0x30000).unwrap();
        let pid3 = table.spawn(0x3000, 0xA000, 0x9000, 0x6800, 0x6400, 0x40000).unwrap();

        // From PID 0 (kernel), next ready should be PID 1
        assert_eq!(table.next_ready(0), Some(pid1));

        // From PID 1, next ready should be PID 2
        assert_eq!(table.next_ready(pid1), Some(pid2));

        // From PID 2, next ready should be PID 3
        assert_eq!(table.next_ready(pid2), Some(pid3));

        // From PID 3, wraps around to PID 1
        assert_eq!(table.next_ready(pid3), Some(pid1));
    }

    #[test]
    fn el0_process_next_ready_skips_running() {
        let mut table = El0ProcessTable::new();
        let pid1 = table.spawn(0x1000, 0x8000, 0x7000, 0x4800, 0x4400, 0x20000).unwrap();
        let pid2 = table.spawn(0x2000, 0x9000, 0x8000, 0x5800, 0x5400, 0x30000).unwrap();

        // Mark PID 1 as Running
        table.get_mut(pid1).unwrap().state = El0State::Running;

        // From PID 0, skip Running PID 1, find Ready PID 2
        assert_eq!(table.next_ready(0), Some(pid2));
    }

    #[test]
    fn el0_process_next_ready_none_available() {
        let mut table = El0ProcessTable::new();
        assert_eq!(table.next_ready(0), None);

        // All processes exited
        let pid = table.spawn(0x1000, 0x8000, 0x7000, 0x4800, 0x4400, 0x20000).unwrap();
        table.exit(pid, 0);
        assert_eq!(table.next_ready(0), None);
    }

    #[test]
    fn el0_process_spawn_reuse_slot() {
        let mut table = El0ProcessTable::new();
        let pid1 = table.spawn(0x1000, 0x8000, 0x7000, 0x4800, 0x4400, 0x20000).unwrap();
        assert_eq!(pid1, 1);

        // Exit and reap PID 1
        table.exit(pid1, 0);
        table.reap(pid1);

        // New spawn should reuse slot 0 → PID 1
        let pid_new = table.spawn(0x2000, 0x9000, 0x8000, 0x5800, 0x5400, 0x30000).unwrap();
        assert_eq!(pid_new, 1);
    }

    // ═══════════════════════════════════════════════════════════════
    // Page Access tests
    // ═══════════════════════════════════════════════════════════════

    #[test]
    fn page_access_ap_bits() {
        // AP[2:1] encoding
        assert_eq!(PageAccess::UserRW.to_ap_bits(), 0b00 << 6);    // 0x00
        assert_eq!(PageAccess::KernelRW.to_ap_bits(), 0b01 << 6);  // 0x40
        assert_eq!(PageAccess::UserRO.to_ap_bits(), 0b10 << 6);    // 0x80
        assert_eq!(PageAccess::KernelRO.to_ap_bits(), 0b11 << 6);  // 0xC0
    }

    #[test]
    fn page_access_user_accessible() {
        assert!(PageAccess::UserRW.is_user_accessible());
        assert!(PageAccess::UserRO.is_user_accessible());
        assert!(!PageAccess::KernelRW.is_user_accessible());
        assert!(!PageAccess::KernelRO.is_user_accessible());
    }

    // ═══════════════════════════════════════════════════════════════
    // Context frame layout tests
    // ═══════════════════════════════════════════════════════════════

    #[test]
    fn context_frame_layout() {
        // 288 bytes = 36 × 8-byte slots, 16-byte aligned
        assert_eq!(CONTEXT_FRAME_SIZE, 288);
        assert_eq!(CONTEXT_FRAME_SIZE % 16, 0);

        // SP_EL0 at offset 264 (slot 33)
        assert_eq!(CONTEXT_SP_EL0_OFFSET, 264);

        // SPSR at offset 256 (slot 32)
        assert_eq!(CONTEXT_SPSR_OFFSET, 256);

        // ELR at offset 248 (slot 31, paired with LR at 240)
        assert_eq!(CONTEXT_ELR_OFFSET, 248);
    }

    #[test]
    fn spsr_el0t_value() {
        // EL0t: M[3:0] = 0b0000, all DAIF clear
        assert_eq!(SPSR_EL0T, 0);
    }
}
