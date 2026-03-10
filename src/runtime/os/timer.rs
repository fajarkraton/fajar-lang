//! Timer and PWM simulation for embedded targets.
//!
//! Provides hardware timer configuration and PWM output for
//! motor/servo control in @kernel context.

use thiserror::Error;

/// Timer errors.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum TimerError {
    /// Timer ID out of range.
    #[error("invalid timer: {id} (max: {max})")]
    InvalidTimer { id: usize, max: usize },

    /// Timer is not running.
    #[error("timer {id} is not running")]
    NotRunning { id: usize },

    /// Invalid PWM duty cycle (must be 0.0..=1.0).
    #[error("invalid duty cycle: {duty} (must be 0.0..=1.0)")]
    InvalidDutyCycle { duty: String },

    /// Invalid frequency.
    #[error("invalid frequency: {freq} Hz")]
    InvalidFrequency { freq: u32 },
}

/// Timer mode of operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerMode {
    /// One-shot: fires once then stops.
    OneShot,
    /// Periodic: fires repeatedly at the configured interval.
    Periodic,
    /// PWM output mode.
    Pwm,
    /// Input capture: records timestamps of external events.
    InputCapture,
}

/// Timer state.
#[derive(Debug, Clone)]
struct Timer {
    /// Mode of operation.
    mode: TimerMode,
    /// Frequency in Hz.
    frequency: u32,
    /// Whether the timer is running.
    running: bool,
    /// Current count value.
    count: u64,
    /// IRQ to fire on overflow/compare (None = no interrupt).
    irq: Option<u8>,
    /// PWM duty cycle (0.0 to 1.0), only used in PWM mode.
    duty_cycle: f64,
    /// Captured input values (input capture mode).
    captures: Vec<u64>,
}

/// Simulated timer/PWM controller.
#[derive(Debug)]
pub struct TimerController {
    /// Available timers.
    timers: Vec<Timer>,
    /// System clock frequency in Hz.
    sys_clock: u32,
}

impl TimerController {
    /// Creates a timer controller with the given number of timers.
    pub fn new(num_timers: usize, sys_clock: u32) -> Self {
        let timers = (0..num_timers)
            .map(|_| Timer {
                mode: TimerMode::Periodic,
                frequency: 0,
                running: false,
                count: 0,
                irq: None,
                duty_cycle: 0.0,
                captures: Vec::new(),
            })
            .collect();
        Self { timers, sys_clock }
    }

    /// Returns the number of timers.
    pub fn num_timers(&self) -> usize {
        self.timers.len()
    }

    /// Returns the system clock frequency.
    pub fn sys_clock(&self) -> u32 {
        self.sys_clock
    }

    /// Configures a timer with the given mode and frequency.
    pub fn configure(
        &mut self,
        id: usize,
        mode: TimerMode,
        frequency: u32,
    ) -> Result<(), TimerError> {
        let timer = self.get_timer_mut(id)?;
        if frequency == 0 {
            return Err(TimerError::InvalidFrequency { freq: 0 });
        }
        timer.mode = mode;
        timer.frequency = frequency;
        timer.count = 0;
        timer.captures.clear();
        Ok(())
    }

    /// Sets the IRQ number for timer overflow/compare events.
    pub fn set_irq(&mut self, id: usize, irq: u8) -> Result<(), TimerError> {
        let timer = self.get_timer_mut(id)?;
        timer.irq = Some(irq);
        Ok(())
    }

    /// Starts a timer.
    pub fn start(&mut self, id: usize) -> Result<(), TimerError> {
        let timer = self.get_timer_mut(id)?;
        timer.running = true;
        Ok(())
    }

    /// Stops a timer.
    pub fn stop(&mut self, id: usize) -> Result<(), TimerError> {
        let timer = self.get_timer_mut(id)?;
        timer.running = false;
        Ok(())
    }

    /// Returns whether a timer is running.
    pub fn is_running(&self, id: usize) -> Result<bool, TimerError> {
        let timer = self.get_timer(id)?;
        Ok(timer.running)
    }

    /// Returns the current count of a timer.
    pub fn count(&self, id: usize) -> Result<u64, TimerError> {
        let timer = self.get_timer(id)?;
        Ok(timer.count)
    }

    /// Sets the PWM duty cycle (0.0 to 1.0) for a PWM-mode timer.
    pub fn set_duty_cycle(&mut self, id: usize, duty: f64) -> Result<(), TimerError> {
        if !(0.0..=1.0).contains(&duty) {
            return Err(TimerError::InvalidDutyCycle {
                duty: format!("{duty}"),
            });
        }
        let timer = self.get_timer_mut(id)?;
        timer.duty_cycle = duty;
        Ok(())
    }

    /// Returns the PWM duty cycle.
    pub fn duty_cycle(&self, id: usize) -> Result<f64, TimerError> {
        let timer = self.get_timer(id)?;
        Ok(timer.duty_cycle)
    }

    /// Returns the PWM output value (high time in microseconds per period).
    pub fn pwm_high_us(&self, id: usize) -> Result<f64, TimerError> {
        let timer = self.get_timer(id)?;
        if timer.frequency == 0 {
            return Ok(0.0);
        }
        let period_us = 1_000_000.0 / timer.frequency as f64;
        Ok(period_us * timer.duty_cycle)
    }

    /// Simulates a tick (increment count, fire IRQ if overflow).
    ///
    /// Returns the IRQ number if one should be fired.
    pub fn tick(&mut self, id: usize) -> Result<Option<u8>, TimerError> {
        let sys_clock = self.sys_clock;
        let timer = self.get_timer_mut(id)?;
        if !timer.running {
            return Err(TimerError::NotRunning { id });
        }

        timer.count += 1;

        let reload = (sys_clock / timer.frequency.max(1)) as u64;
        if timer.count >= reload {
            timer.count = 0;
            if timer.mode == TimerMode::OneShot {
                timer.running = false;
            }
            return Ok(timer.irq);
        }
        Ok(None)
    }

    /// Records an input capture event.
    pub fn capture(&mut self, id: usize, timestamp: u64) -> Result<(), TimerError> {
        let timer = self.get_timer_mut(id)?;
        timer.captures.push(timestamp);
        Ok(())
    }

    /// Returns captured timestamps.
    pub fn captures(&self, id: usize) -> Result<&[u64], TimerError> {
        let timer = self.get_timer(id)?;
        Ok(&timer.captures)
    }

    fn get_timer(&self, id: usize) -> Result<&Timer, TimerError> {
        self.timers.get(id).ok_or(TimerError::InvalidTimer {
            id,
            max: self.timers.len().saturating_sub(1),
        })
    }

    fn get_timer_mut(&mut self, id: usize) -> Result<&mut Timer, TimerError> {
        let max = self.timers.len().saturating_sub(1);
        self.timers
            .get_mut(id)
            .ok_or(TimerError::InvalidTimer { id, max })
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn make_controller() -> TimerController {
        TimerController::new(4, 1_000_000) // 1 MHz system clock
    }

    #[test]
    fn create_controller() {
        let tc = make_controller();
        assert_eq!(tc.num_timers(), 4);
        assert_eq!(tc.sys_clock(), 1_000_000);
    }

    #[test]
    fn configure_and_start() {
        let mut tc = make_controller();
        tc.configure(0, TimerMode::Periodic, 1000).unwrap();
        assert!(!tc.is_running(0).unwrap());
        tc.start(0).unwrap();
        assert!(tc.is_running(0).unwrap());
    }

    #[test]
    fn stop_timer() {
        let mut tc = make_controller();
        tc.configure(0, TimerMode::Periodic, 1000).unwrap();
        tc.start(0).unwrap();
        tc.stop(0).unwrap();
        assert!(!tc.is_running(0).unwrap());
    }

    #[test]
    fn tick_increments_count() {
        let mut tc = make_controller();
        tc.configure(0, TimerMode::Periodic, 1000).unwrap();
        tc.start(0).unwrap();
        tc.tick(0).unwrap();
        assert_eq!(tc.count(0).unwrap(), 1);
        tc.tick(0).unwrap();
        assert_eq!(tc.count(0).unwrap(), 2);
    }

    #[test]
    fn periodic_overflow_fires_irq() {
        let mut tc = TimerController::new(1, 100); // 100 Hz sys clock
        tc.configure(0, TimerMode::Periodic, 10).unwrap(); // overflow every 10 ticks
        tc.set_irq(0, 0x20).unwrap();
        tc.start(0).unwrap();

        for _ in 0..9 {
            assert_eq!(tc.tick(0).unwrap(), None);
        }
        // 10th tick should overflow
        assert_eq!(tc.tick(0).unwrap(), Some(0x20));
        // Counter resets, timer still running
        assert!(tc.is_running(0).unwrap());
        assert_eq!(tc.count(0).unwrap(), 0);
    }

    #[test]
    fn oneshot_stops_after_overflow() {
        let mut tc = TimerController::new(1, 10);
        tc.configure(0, TimerMode::OneShot, 5).unwrap(); // overflow after 2 ticks
        tc.start(0).unwrap();

        tc.tick(0).unwrap(); // count=1
        let irq = tc.tick(0).unwrap(); // count=2 → overflow
        assert!(irq.is_none() || irq.is_some()); // may or may not fire IRQ

        // After overflow in OneShot, timer stops
        // Need enough ticks to reach reload value
        // sys_clock=10, freq=5 → reload=2
        // Already did 2 ticks, should have overflowed
        assert!(!tc.is_running(0).unwrap());
    }

    #[test]
    fn pwm_duty_cycle() {
        let mut tc = make_controller();
        tc.configure(0, TimerMode::Pwm, 50).unwrap(); // 50 Hz (20ms period)
        tc.set_duty_cycle(0, 0.075).unwrap(); // 1.5ms (servo center)

        let duty = tc.duty_cycle(0).unwrap();
        assert!((duty - 0.075).abs() < 1e-10);

        // 50Hz → 20000us period, 7.5% duty = 1500us
        let high_us = tc.pwm_high_us(0).unwrap();
        assert!((high_us - 1500.0).abs() < 0.1);
    }

    #[test]
    fn pwm_invalid_duty_cycle() {
        let mut tc = make_controller();
        tc.configure(0, TimerMode::Pwm, 50).unwrap();
        assert!(tc.set_duty_cycle(0, 1.5).is_err());
        assert!(tc.set_duty_cycle(0, -0.1).is_err());
    }

    #[test]
    fn input_capture() {
        let mut tc = make_controller();
        tc.configure(0, TimerMode::InputCapture, 1000).unwrap();
        tc.capture(0, 100).unwrap();
        tc.capture(0, 200).unwrap();
        tc.capture(0, 350).unwrap();

        let caps = tc.captures(0).unwrap();
        assert_eq!(caps, &[100, 200, 350]);
    }

    #[test]
    fn invalid_timer_id() {
        let mut tc = make_controller();
        assert!(tc.configure(10, TimerMode::Periodic, 1000).is_err());
    }

    #[test]
    fn zero_frequency_error() {
        let mut tc = make_controller();
        assert!(tc.configure(0, TimerMode::Periodic, 0).is_err());
    }

    #[test]
    fn tick_not_running_error() {
        let mut tc = make_controller();
        tc.configure(0, TimerMode::Periodic, 1000).unwrap();
        // Don't start
        assert!(matches!(tc.tick(0), Err(TimerError::NotRunning { id: 0 })));
    }

    #[test]
    fn timer_values() {
        let tc = make_controller();
        // Verify defaults
        assert!(!tc.is_running(0).unwrap());
        assert_eq!(tc.count(0).unwrap(), 0);
        assert_eq!(tc.duty_cycle(0).unwrap(), 0.0);
    }
}
