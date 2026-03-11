//! Power management APIs for embedded targets.
//!
//! Provides sleep modes, wake sources, clock gating, voltage scaling,
//! and power budget estimation. All operations are simulation stubs
//! suitable for testing firmware power management logic.

use std::collections::HashSet;
use thiserror::Error;

// ═══════════════════════════════════════════════════════════════════════
// Error types
// ═══════════════════════════════════════════════════════════════════════

/// Errors from power management operations.
#[derive(Debug, Clone, Error, PartialEq)]
pub enum PowerError {
    /// Attempted to enter an unsupported power mode.
    #[error("unsupported power mode: {mode}")]
    UnsupportedMode {
        /// The mode description.
        mode: String,
    },

    /// Wake source configuration failed.
    #[error("invalid wake source: {reason}")]
    InvalidWakeSource {
        /// Description of why the wake source is invalid.
        reason: String,
    },

    /// Peripheral clock operation failed.
    #[error("clock error for '{peripheral}': {reason}")]
    ClockError {
        /// The peripheral name.
        peripheral: String,
        /// Description of the error.
        reason: String,
    },

    /// Voltage scaling operation failed.
    #[error("voltage scale error: {reason}")]
    VoltageScaleError {
        /// Description of the error.
        reason: String,
    },

    /// Power budget calculation error.
    #[error("power budget error: {reason}")]
    BudgetError {
        /// Description of the error.
        reason: String,
    },
}

// ═══════════════════════════════════════════════════════════════════════
// Power modes
// ═══════════════════════════════════════════════════════════════════════

/// Processor power modes, modeled after ARM Cortex-M low-power states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PowerMode {
    /// Normal run mode — all clocks active.
    Run,
    /// Sleep mode (WFI) — CPU clock stopped, peripherals active.
    Sleep,
    /// Stop mode (SLEEPDEEP) — all clocks stopped, SRAM retained.
    Stop,
    /// Standby mode — minimal power, only wakeup logic powered.
    Standby,
    /// Full shutdown — no power, requires external reset.
    Shutdown,
}

impl PowerMode {
    /// Returns a human-readable description including wake latency
    /// and approximate power consumption characteristics.
    pub fn description(&self) -> &'static str {
        match self {
            Self::Run => "Run: all clocks active, ~100mA, 0us wake latency",
            Self::Sleep => "Sleep (WFI): CPU stopped, peripherals active, ~20mA, <1us wake",
            Self::Stop => "Stop (SLEEPDEEP): all clocks stopped, SRAM retained, ~2mA, ~5us wake",
            Self::Standby => "Standby: minimal power, wakeup logic only, ~10uA, ~50us wake",
            Self::Shutdown => "Shutdown: no power, external reset required, ~1uA, >1ms wake",
        }
    }

    /// Returns the typical wake latency in microseconds for this mode.
    pub fn wake_latency_us(&self) -> u32 {
        match self {
            Self::Run => 0,
            Self::Sleep => 1,
            Self::Stop => 5,
            Self::Standby => 50,
            Self::Shutdown => 1000,
        }
    }

    /// Returns the approximate current draw in milliamps.
    pub fn typical_current_ma(&self) -> f64 {
        match self {
            Self::Run => 100.0,
            Self::Sleep => 20.0,
            Self::Stop => 2.0,
            Self::Standby => 0.01,
            Self::Shutdown => 0.001,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Wake sources
// ═══════════════════════════════════════════════════════════════════════

/// GPIO edge sensitivity for wake-on-pin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Edge {
    /// Wake on rising edge.
    Rising,
    /// Wake on falling edge.
    Falling,
    /// Wake on either edge.
    Both,
}

/// Sources that can wake the processor from a low-power mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WakeSource {
    /// Wake on external interrupt line.
    Interrupt(u32),
    /// Wake on RTC alarm at the given epoch timestamp.
    RtcAlarm(u64),
    /// Wake on GPIO pin edge transition.
    GpioPin {
        /// The pin number.
        pin: u32,
        /// Which edge(s) trigger the wake.
        edge: Edge,
    },
    /// Wake after a timer expires.
    WakeupTimer {
        /// Duration in milliseconds before waking.
        duration_ms: u32,
    },
}

// ═══════════════════════════════════════════════════════════════════════
// Power controller (sleep + wake)
// ═══════════════════════════════════════════════════════════════════════

/// Simulated power management controller.
///
/// Tracks the current power mode and configured wake sources.
#[derive(Debug)]
pub struct PowerController {
    /// The current power mode.
    current_mode: PowerMode,
    /// Configured wake sources.
    wake_sources: Vec<WakeSource>,
    /// Number of times sleep has been entered (for testing).
    sleep_count: u64,
}

impl PowerController {
    /// Creates a new power controller in Run mode.
    pub fn new() -> Self {
        Self {
            current_mode: PowerMode::Run,
            wake_sources: Vec::new(),
            sleep_count: 0,
        }
    }

    /// Returns the current power mode.
    pub fn current_mode(&self) -> PowerMode {
        self.current_mode
    }

    /// Returns the number of times a sleep mode has been entered.
    pub fn sleep_count(&self) -> u64 {
        self.sleep_count
    }

    /// Returns the currently configured wake sources.
    pub fn wake_sources(&self) -> &[WakeSource] {
        &self.wake_sources
    }

    /// Enters a sleep mode, simulating WFI / SLEEPDEEP behavior.
    ///
    /// In simulation, this records the mode transition and increments
    /// the sleep counter. Real hardware would halt the CPU here.
    pub fn enter_sleep_mode(&mut self, mode: PowerMode) -> Result<(), PowerError> {
        if mode == PowerMode::Run {
            return Err(PowerError::UnsupportedMode {
                mode: "Run is not a sleep mode".to_string(),
            });
        }
        self.validate_wake_sources_for_mode(mode)?;
        self.current_mode = mode;
        self.sleep_count += 1;
        // Simulation: immediately "wake" back to Run
        self.current_mode = PowerMode::Run;
        Ok(())
    }

    /// Configures a wake source (EXTI line, RTC alarm, GPIO pin, or timer).
    pub fn configure_wake_source(&mut self, source: WakeSource) -> Result<(), PowerError> {
        self.validate_wake_source(&source)?;
        self.wake_sources.push(source);
        Ok(())
    }

    /// Clears all configured wake sources.
    pub fn clear_wake_sources(&mut self) {
        self.wake_sources.clear();
    }

    /// Validates a single wake source configuration.
    fn validate_wake_source(&self, source: &WakeSource) -> Result<(), PowerError> {
        match source {
            WakeSource::Interrupt(line) => {
                if *line > 255 {
                    return Err(PowerError::InvalidWakeSource {
                        reason: format!("interrupt line {line} exceeds max 255"),
                    });
                }
            }
            WakeSource::GpioPin { pin, .. } => {
                if *pin > 127 {
                    return Err(PowerError::InvalidWakeSource {
                        reason: format!("GPIO pin {pin} exceeds max 127"),
                    });
                }
            }
            WakeSource::WakeupTimer { duration_ms } => {
                if *duration_ms == 0 {
                    return Err(PowerError::InvalidWakeSource {
                        reason: "wakeup timer duration must be > 0".to_string(),
                    });
                }
            }
            WakeSource::RtcAlarm(_) => { /* any timestamp is valid */ }
        }
        Ok(())
    }

    /// Ensures at least one wake source is configured for deep sleep modes.
    fn validate_wake_sources_for_mode(&self, mode: PowerMode) -> Result<(), PowerError> {
        let needs_wake = matches!(mode, PowerMode::Stop | PowerMode::Standby);
        if needs_wake && self.wake_sources.is_empty() {
            return Err(PowerError::UnsupportedMode {
                mode: format!("{:?} requires at least one wake source configured", mode),
            });
        }
        Ok(())
    }
}

impl Default for PowerController {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Clock gating
// ═══════════════════════════════════════════════════════════════════════

/// Simulated peripheral clock controller.
///
/// Tracks which peripherals have their clocks enabled. Disabling
/// unused peripheral clocks reduces active power consumption.
#[derive(Debug)]
pub struct ClockController {
    /// Set of peripherals with clocks enabled.
    enabled_peripherals: HashSet<String>,
    /// System clock frequency in MHz.
    frequency_mhz: u32,
}

impl ClockController {
    /// Creates a new clock controller at the given frequency.
    pub fn new(frequency_mhz: u32) -> Self {
        Self {
            enabled_peripherals: HashSet::new(),
            frequency_mhz,
        }
    }

    /// Returns the system clock frequency in MHz.
    pub fn frequency_mhz(&self) -> u32 {
        self.frequency_mhz
    }

    /// Sets the system clock frequency in MHz.
    pub fn set_frequency_mhz(&mut self, freq: u32) -> Result<(), PowerError> {
        if freq == 0 {
            return Err(PowerError::ClockError {
                peripheral: "system".to_string(),
                reason: "frequency must be > 0".to_string(),
            });
        }
        self.frequency_mhz = freq;
        Ok(())
    }

    /// Enables the clock for the named peripheral.
    pub fn clock_enable(&mut self, peripheral: &str) -> Result<(), PowerError> {
        if peripheral.is_empty() {
            return Err(PowerError::ClockError {
                peripheral: peripheral.to_string(),
                reason: "peripheral name cannot be empty".to_string(),
            });
        }
        self.enabled_peripherals.insert(peripheral.to_string());
        Ok(())
    }

    /// Disables the clock for the named peripheral.
    pub fn clock_disable(&mut self, peripheral: &str) -> Result<(), PowerError> {
        if !self.enabled_peripherals.contains(peripheral) {
            return Err(PowerError::ClockError {
                peripheral: peripheral.to_string(),
                reason: "peripheral clock is not enabled".to_string(),
            });
        }
        self.enabled_peripherals.remove(peripheral);
        Ok(())
    }

    /// Returns whether the named peripheral clock is enabled.
    pub fn is_enabled(&self, peripheral: &str) -> bool {
        self.enabled_peripherals.contains(peripheral)
    }

    /// Returns the number of peripherals with clocks enabled.
    pub fn enabled_count(&self) -> usize {
        self.enabled_peripherals.len()
    }

    /// Returns all enabled peripheral names (sorted for determinism).
    pub fn enabled_list(&self) -> Vec<String> {
        let mut list: Vec<String> = self.enabled_peripherals.iter().cloned().collect();
        list.sort();
        list
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Voltage scaling
// ═══════════════════════════════════════════════════════════════════════

/// Voltage scaling levels (modeled after STM32 VOS registers).
///
/// Higher voltage allows higher clock frequency but increases power.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VoltageScale {
    /// VOS1: highest voltage, maximum frequency.
    Vos1,
    /// VOS2: medium voltage, reduced maximum frequency.
    Vos2,
    /// VOS3: lowest voltage, lowest maximum frequency.
    Vos3,
}

impl VoltageScale {
    /// Returns the maximum allowed clock frequency (MHz) at this scale.
    pub fn max_frequency_mhz(&self) -> u32 {
        match self {
            Self::Vos1 => 480,
            Self::Vos2 => 300,
            Self::Vos3 => 170,
        }
    }

    /// Returns the approximate core voltage in volts.
    pub fn voltage(&self) -> f64 {
        match self {
            Self::Vos1 => 1.3,
            Self::Vos2 => 1.1,
            Self::Vos3 => 0.9,
        }
    }
}

/// Simulated voltage regulator.
#[derive(Debug)]
pub struct VoltageRegulator {
    /// Current voltage scale.
    scale: VoltageScale,
}

impl VoltageRegulator {
    /// Creates a new voltage regulator at VOS1 (max performance).
    pub fn new() -> Self {
        Self {
            scale: VoltageScale::Vos1,
        }
    }

    /// Returns the current voltage scale.
    pub fn scale(&self) -> VoltageScale {
        self.scale
    }

    /// Sets the voltage scale.
    ///
    /// In real hardware, this would configure the internal regulator
    /// and wait for the voltage to stabilize.
    pub fn set_voltage_scale(
        &mut self,
        vos: VoltageScale,
        clock_mhz: u32,
    ) -> Result<(), PowerError> {
        if clock_mhz > vos.max_frequency_mhz() {
            return Err(PowerError::VoltageScaleError {
                reason: format!(
                    "clock {}MHz exceeds {:?} max {}MHz — reduce clock first",
                    clock_mhz,
                    vos,
                    vos.max_frequency_mhz()
                ),
            });
        }
        self.scale = vos;
        Ok(())
    }
}

impl Default for VoltageRegulator {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Power budget
// ═══════════════════════════════════════════════════════════════════════

/// Power budget estimation for battery-powered embedded systems.
///
/// Computes estimated battery life from active/sleep current draw
/// and battery capacity.
#[derive(Debug, Clone)]
pub struct PowerBudget {
    /// Active mode current draw in milliamps.
    pub active_current_ma: f64,
    /// Sleep mode current draw in milliamps.
    pub sleep_current_ma: f64,
    /// Wake latency overhead in microseconds.
    pub wake_latency_us: u32,
    /// Battery capacity in milliamp-hours.
    pub battery_capacity_mah: f64,
}

impl PowerBudget {
    /// Creates a new power budget.
    pub fn new(
        active_current_ma: f64,
        sleep_current_ma: f64,
        wake_latency_us: u32,
        battery_capacity_mah: f64,
    ) -> Self {
        Self {
            active_current_ma,
            sleep_current_ma,
            wake_latency_us,
            battery_capacity_mah,
        }
    }

    /// Estimates battery life in hours assuming 100% sleep mode.
    ///
    /// Returns 0.0 if sleep current is zero or negative (invalid).
    pub fn estimated_battery_life_hours(&self) -> f64 {
        if self.sleep_current_ma <= 0.0 {
            return 0.0;
        }
        self.battery_capacity_mah / self.sleep_current_ma
    }

    /// Estimates battery life given a duty cycle (fraction of time active).
    ///
    /// `duty_cycle` should be in 0.0..=1.0 where 1.0 means always active.
    pub fn estimated_life_with_duty_cycle(&self, duty_cycle: f64) -> Result<f64, PowerError> {
        let clamped = duty_cycle.clamp(0.0, 1.0);
        let avg_current =
            clamped * self.active_current_ma + (1.0 - clamped) * self.sleep_current_ma;
        if avg_current <= 0.0 {
            return Err(PowerError::BudgetError {
                reason: "average current must be positive".to_string(),
            });
        }
        Ok(self.battery_capacity_mah / avg_current)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Convenience entry point (matches sprint spec)
// ═══════════════════════════════════════════════════════════════════════

/// Enters a sleep mode on the given power controller.
///
/// This is a free-function convenience wrapper around
/// [`PowerController::enter_sleep_mode`].
pub fn enter_sleep_mode(
    controller: &mut PowerController,
    mode: PowerMode,
) -> Result<(), PowerError> {
    controller.enter_sleep_mode(mode)
}

/// Configures a wake source on the given power controller.
///
/// This is a free-function convenience wrapper around
/// [`PowerController::configure_wake_source`].
pub fn configure_wake_source(
    controller: &mut PowerController,
    source: WakeSource,
) -> Result<(), PowerError> {
    controller.configure_wake_source(source)
}

/// Enables a peripheral clock on the given clock controller.
pub fn clock_enable(controller: &mut ClockController, peripheral: &str) -> Result<(), PowerError> {
    controller.clock_enable(peripheral)
}

/// Disables a peripheral clock on the given clock controller.
pub fn clock_disable(controller: &mut ClockController, peripheral: &str) -> Result<(), PowerError> {
    controller.clock_disable(peripheral)
}

/// Sets the voltage scale on the given regulator.
pub fn set_voltage_scale(
    regulator: &mut VoltageRegulator,
    vos: VoltageScale,
    clock_mhz: u32,
) -> Result<(), PowerError> {
    regulator.set_voltage_scale(vos, clock_mhz)
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // S25.1: PowerMode enum and descriptions
    #[test]
    fn power_mode_descriptions_and_latencies() {
        assert!(PowerMode::Run.description().contains("0us"));
        assert!(PowerMode::Sleep.description().contains("WFI"));
        assert!(PowerMode::Stop.description().contains("SLEEPDEEP"));
        assert!(PowerMode::Standby.description().contains("wakeup"));
        assert!(PowerMode::Shutdown.description().contains("external reset"));

        assert_eq!(PowerMode::Run.wake_latency_us(), 0);
        assert_eq!(PowerMode::Sleep.wake_latency_us(), 1);
        assert_eq!(PowerMode::Shutdown.wake_latency_us(), 1000);

        assert!(PowerMode::Run.typical_current_ma() > PowerMode::Sleep.typical_current_ma());
        assert!(PowerMode::Standby.typical_current_ma() < 1.0);
    }

    // S25.2: enter_sleep_mode simulation
    #[test]
    fn enter_sleep_and_wake_cycle() {
        let mut pc = PowerController::new();
        assert_eq!(pc.current_mode(), PowerMode::Run);
        assert_eq!(pc.sleep_count(), 0);

        // Sleep mode does not require wake sources
        pc.enter_sleep_mode(PowerMode::Sleep).unwrap();
        assert_eq!(pc.sleep_count(), 1);
        // After simulation, mode returns to Run
        assert_eq!(pc.current_mode(), PowerMode::Run);

        // Run is not a valid sleep mode
        assert!(pc.enter_sleep_mode(PowerMode::Run).is_err());
    }

    // S25.3: Wake source configuration
    #[test]
    fn configure_wake_sources() {
        let mut pc = PowerController::new();

        pc.configure_wake_source(WakeSource::Interrupt(5)).unwrap();
        pc.configure_wake_source(WakeSource::RtcAlarm(1700000000))
            .unwrap();
        pc.configure_wake_source(WakeSource::GpioPin {
            pin: 13,
            edge: Edge::Rising,
        })
        .unwrap();
        pc.configure_wake_source(WakeSource::WakeupTimer { duration_ms: 500 })
            .unwrap();

        assert_eq!(pc.wake_sources().len(), 4);

        // Invalid: interrupt line > 255
        assert!(pc
            .configure_wake_source(WakeSource::Interrupt(300))
            .is_err());
        // Invalid: GPIO pin > 127
        assert!(pc
            .configure_wake_source(WakeSource::GpioPin {
                pin: 200,
                edge: Edge::Both,
            })
            .is_err());
        // Invalid: zero duration timer
        assert!(pc
            .configure_wake_source(WakeSource::WakeupTimer { duration_ms: 0 })
            .is_err());
    }

    // S25.4: Stop/Standby require wake sources
    #[test]
    fn deep_sleep_requires_wake_source() {
        let mut pc = PowerController::new();

        // Stop without wake source fails
        assert!(pc.enter_sleep_mode(PowerMode::Stop).is_err());
        // Standby without wake source fails
        assert!(pc.enter_sleep_mode(PowerMode::Standby).is_err());

        // Add a wake source, now it works
        pc.configure_wake_source(WakeSource::RtcAlarm(9999))
            .unwrap();
        pc.enter_sleep_mode(PowerMode::Stop).unwrap();
        assert_eq!(pc.sleep_count(), 1);
    }

    // S25.5: Clock gating enable/disable
    #[test]
    fn clock_gating_enable_disable() {
        let mut cc = ClockController::new(168);
        assert_eq!(cc.frequency_mhz(), 168);
        assert_eq!(cc.enabled_count(), 0);

        cc.clock_enable("USART1").unwrap();
        cc.clock_enable("SPI2").unwrap();
        cc.clock_enable("I2C1").unwrap();
        assert_eq!(cc.enabled_count(), 3);
        assert!(cc.is_enabled("USART1"));
        assert!(!cc.is_enabled("USART2"));

        cc.clock_disable("SPI2").unwrap();
        assert_eq!(cc.enabled_count(), 2);
        assert!(!cc.is_enabled("SPI2"));

        // Disable already-disabled peripheral fails
        assert!(cc.clock_disable("SPI2").is_err());
        // Empty name fails
        assert!(cc.clock_enable("").is_err());
    }

    // S25.6: ClockController enabled list
    #[test]
    fn clock_controller_enabled_list_sorted() {
        let mut cc = ClockController::new(72);
        cc.clock_enable("TIM2").unwrap();
        cc.clock_enable("ADC1").unwrap();
        cc.clock_enable("DMA1").unwrap();

        let list = cc.enabled_list();
        assert_eq!(list, vec!["ADC1", "DMA1", "TIM2"]);
    }

    // S25.7: PowerBudget battery life estimation
    #[test]
    fn power_budget_battery_life() {
        let budget = PowerBudget::new(
            100.0,  // active: 100mA
            0.01,   // sleep: 10uA
            50,     // wake latency: 50us
            2000.0, // battery: 2000mAh
        );

        // Pure sleep: 2000mAh / 0.01mA = 200,000 hours
        let hours = budget.estimated_battery_life_hours();
        assert!((hours - 200_000.0).abs() < 0.1);

        // 1% duty cycle: avg = 0.01*100 + 0.99*0.01 = 1.0099 mA
        let duty_hours = budget.estimated_life_with_duty_cycle(0.01).unwrap();
        assert!(duty_hours > 1900.0 && duty_hours < 2100.0);

        // 100% active: 2000/100 = 20 hours
        let active_hours = budget.estimated_life_with_duty_cycle(1.0).unwrap();
        assert!((active_hours - 20.0).abs() < 0.1);
    }

    // S25.8: PowerBudget edge cases
    #[test]
    fn power_budget_zero_sleep_current() {
        let budget = PowerBudget::new(100.0, 0.0, 0, 2000.0);
        assert_eq!(budget.estimated_battery_life_hours(), 0.0);
    }

    // S25.9: Voltage scaling
    #[test]
    fn voltage_scaling_levels() {
        let mut reg = VoltageRegulator::new();
        assert_eq!(reg.scale(), VoltageScale::Vos1);
        assert_eq!(VoltageScale::Vos1.max_frequency_mhz(), 480);
        assert_eq!(VoltageScale::Vos2.max_frequency_mhz(), 300);
        assert_eq!(VoltageScale::Vos3.max_frequency_mhz(), 170);

        // Scale down to VOS3 at 160MHz — OK
        reg.set_voltage_scale(VoltageScale::Vos3, 160).unwrap();
        assert_eq!(reg.scale(), VoltageScale::Vos3);

        // Try VOS3 at 200MHz — exceeds max, should fail
        assert!(reg.set_voltage_scale(VoltageScale::Vos3, 200).is_err());

        // VOS2 at 200MHz — OK
        reg.set_voltage_scale(VoltageScale::Vos2, 200).unwrap();
        assert_eq!(reg.scale(), VoltageScale::Vos2);

        // Voltage values are sensible
        assert!(VoltageScale::Vos1.voltage() > VoltageScale::Vos3.voltage());
    }

    // S25.10: Free-function convenience wrappers
    #[test]
    fn free_function_wrappers() {
        let mut pc = PowerController::new();
        let mut cc = ClockController::new(168);
        let mut reg = VoltageRegulator::new();

        configure_wake_source(&mut pc, WakeSource::Interrupt(10)).unwrap();
        enter_sleep_mode(&mut pc, PowerMode::Stop).unwrap();

        clock_enable(&mut cc, "GPIOA").unwrap();
        assert!(cc.is_enabled("GPIOA"));
        clock_disable(&mut cc, "GPIOA").unwrap();
        assert!(!cc.is_enabled("GPIOA"));

        set_voltage_scale(&mut reg, VoltageScale::Vos2, 250).unwrap();
        assert_eq!(reg.scale(), VoltageScale::Vos2);
    }
}
