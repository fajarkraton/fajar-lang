//! Programmable Interval Timer (PIT 8253/8254) for x86.
//!
//! The PIT generates periodic interrupts (IRQ 0, vector 0x20)
//! at a configurable frequency. Standard PC uses 1.193182 MHz
//! crystal oscillator divided by a reload value.

use thiserror::Error;

// ═══════════════════════════════════════════════════════════════════════
// Constants
// ═══════════════════════════════════════════════════════════════════════

/// PIT base frequency (Hz).
pub const PIT_BASE_FREQ: u32 = 1_193_182;
/// PIT channel 0 data port.
pub const PIT_CHANNEL0: u16 = 0x40;
/// PIT command register port.
pub const PIT_COMMAND: u16 = 0x43;
/// Default tick frequency (100 Hz = 10ms per tick).
pub const DEFAULT_FREQ_HZ: u32 = 100;

// ═══════════════════════════════════════════════════════════════════════
// PIT errors
// ═══════════════════════════════════════════════════════════════════════

/// PIT timer errors.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum PitError {
    /// Requested frequency out of range.
    #[error("PIT frequency out of range: {freq} Hz (min 19, max {PIT_BASE_FREQ})")]
    FrequencyOutOfRange { freq: u32 },
}

// ═══════════════════════════════════════════════════════════════════════
// PIT timer
// ═══════════════════════════════════════════════════════════════════════

/// Simulated Programmable Interval Timer.
///
/// Tracks tick count and configurable frequency.
#[derive(Debug)]
pub struct PitTimer {
    /// Configured frequency in Hz.
    frequency: u32,
    /// Reload divisor value.
    divisor: u16,
    /// Total ticks since init.
    ticks: u64,
    /// Whether the timer is running.
    running: bool,
}

impl PitTimer {
    /// Create a new PIT with the given frequency.
    pub fn new(frequency: u32) -> Result<Self, PitError> {
        if !(19..=PIT_BASE_FREQ).contains(&frequency) {
            return Err(PitError::FrequencyOutOfRange { freq: frequency });
        }
        let divisor = (PIT_BASE_FREQ / frequency) as u16;
        Ok(Self {
            frequency,
            divisor,
            ticks: 0,
            running: false,
        })
    }

    /// Create with default 100 Hz frequency.
    pub fn default_100hz() -> Self {
        Self::new(DEFAULT_FREQ_HZ).expect("100Hz is valid")
    }

    /// Start the timer.
    pub fn start(&mut self) {
        self.running = true;
    }

    /// Stop the timer.
    pub fn stop(&mut self) {
        self.running = false;
    }

    /// Handle a tick interrupt (call from IRQ handler).
    pub fn tick(&mut self) {
        if self.running {
            self.ticks += 1;
        }
    }

    /// Get total tick count.
    pub fn ticks(&self) -> u64 {
        self.ticks
    }

    /// Get elapsed time in milliseconds.
    pub fn elapsed_ms(&self) -> u64 {
        if self.frequency == 0 {
            return 0;
        }
        self.ticks * 1000 / self.frequency as u64
    }

    /// Get elapsed time in seconds (integer).
    pub fn elapsed_secs(&self) -> u64 {
        if self.frequency == 0 {
            return 0;
        }
        self.ticks / self.frequency as u64
    }

    /// Get the configured frequency.
    pub fn frequency(&self) -> u32 {
        self.frequency
    }

    /// Get the reload divisor.
    pub fn divisor(&self) -> u16 {
        self.divisor
    }

    /// Whether the timer is running.
    pub fn is_running(&self) -> bool {
        self.running
    }

    /// Reset the tick counter.
    pub fn reset(&mut self) {
        self.ticks = 0;
    }

    /// Get the port commands needed to program the PIT.
    ///
    /// Returns `(command_byte, divisor_lo, divisor_hi)`.
    pub fn program_bytes(&self) -> (u8, u8, u8) {
        // Channel 0, lobyte/hibyte, rate generator (mode 2)
        let command: u8 = 0x34;
        let lo = (self.divisor & 0xFF) as u8;
        let hi = ((self.divisor >> 8) & 0xFF) as u8;
        (command, lo, hi)
    }
}

impl Default for PitTimer {
    fn default() -> Self {
        Self::default_100hz()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pit_default_100hz() {
        let pit = PitTimer::default_100hz();
        assert_eq!(pit.frequency(), 100);
        assert!(!pit.is_running());
        assert_eq!(pit.ticks(), 0);
    }

    #[test]
    fn pit_tick_counting() {
        let mut pit = PitTimer::default_100hz();
        pit.start();
        for _ in 0..100 {
            pit.tick();
        }
        assert_eq!(pit.ticks(), 100);
        assert_eq!(pit.elapsed_secs(), 1);
        assert_eq!(pit.elapsed_ms(), 1000);
    }

    #[test]
    fn pit_stopped_no_ticks() {
        let mut pit = PitTimer::default_100hz();
        // Not started — ticks should not count
        pit.tick();
        pit.tick();
        assert_eq!(pit.ticks(), 0);
    }

    #[test]
    fn pit_frequency_out_of_range() {
        assert!(PitTimer::new(0).is_err());
        assert!(PitTimer::new(18).is_err());
        assert!(PitTimer::new(PIT_BASE_FREQ + 1).is_err());
    }

    #[test]
    fn pit_program_bytes() {
        let pit = PitTimer::default_100hz();
        let (cmd, lo, hi) = pit.program_bytes();
        assert_eq!(cmd, 0x34);
        // Divisor for 100Hz: 1193182 / 100 = 11931
        let divisor = pit.divisor();
        assert_eq!(lo, (divisor & 0xFF) as u8);
        assert_eq!(hi, ((divisor >> 8) & 0xFF) as u8);
    }

    #[test]
    fn pit_reset() {
        let mut pit = PitTimer::default_100hz();
        pit.start();
        pit.tick();
        pit.tick();
        assert_eq!(pit.ticks(), 2);
        pit.reset();
        assert_eq!(pit.ticks(), 0);
    }
}
