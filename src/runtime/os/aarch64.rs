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
}
