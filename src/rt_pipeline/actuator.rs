//! Actuator Framework — motors, servos, GPIO, CAN bus, PID control, safety.
//!
//! Phase R2.2: 10 tasks covering actuator abstraction, motor/servo drivers,
//! PID controller, command smoothing, and fail-safe mechanisms.

use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// R2.2.1: Actuator Trait
// ═══════════════════════════════════════════════════════════════════════

/// Core actuator trait.
pub trait Actuator {
    /// Actuator type identifier.
    fn actuator_type(&self) -> ActuatorType;
    /// Send a command to the actuator.
    fn act(&mut self, cmd: &Command) -> Result<(), ActuatorError>;
    /// Emergency stop — immediately set safe state.
    fn emergency_stop(&mut self) -> Result<(), ActuatorError>;
    /// Whether the actuator is ready.
    fn is_ready(&self) -> bool;
}

/// Actuator type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActuatorType {
    PwmMotor,
    Servo,
    GpioOutput,
    CanBus,
    Stepper,
    Solenoid,
    Custom(u32),
}

/// Actuator command.
#[derive(Debug, Clone)]
pub enum Command {
    /// Set motor speed (-1.0 to 1.0, negative = reverse).
    MotorSpeed(f64),
    /// Set servo angle (0.0 to 180.0 degrees).
    ServoAngle(f64),
    /// Set GPIO pin high or low.
    GpioSet(u32, bool),
    /// Send CAN frame.
    CanFrame { id: u32, data: Vec<u8> },
    /// Set stepper motor position (steps).
    StepperPosition(i64),
    /// Emergency stop.
    Stop,
}

/// Actuator error.
#[derive(Debug, Clone, PartialEq)]
pub enum ActuatorError {
    NotReady,
    OutOfRange(String),
    HardwareError(String),
    SafetyInterlock(String),
    CommunicationError(String),
    Timeout,
}

impl fmt::Display for ActuatorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotReady => write!(f, "actuator not ready"),
            Self::OutOfRange(msg) => write!(f, "out of range: {msg}"),
            Self::HardwareError(msg) => write!(f, "hardware error: {msg}"),
            Self::SafetyInterlock(msg) => write!(f, "safety interlock: {msg}"),
            Self::CommunicationError(msg) => write!(f, "communication error: {msg}"),
            Self::Timeout => write!(f, "actuator timeout"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// R2.2.2-R2.2.5: Motor, Servo, GPIO, CAN
// ═══════════════════════════════════════════════════════════════════════

/// PWM motor configuration.
#[derive(Debug, Clone)]
pub struct PwmMotorConfig {
    /// PWM pin number.
    pub pwm_pin: u32,
    /// Direction pin number.
    pub dir_pin: u32,
    /// PWM frequency in Hz.
    pub frequency_hz: u32,
    /// Maximum duty cycle (0.0 to 1.0).
    pub max_duty: f64,
    /// Ramp rate (change per second, for smoothing).
    pub ramp_rate: f64,
}

impl Default for PwmMotorConfig {
    fn default() -> Self {
        Self {
            pwm_pin: 0,
            dir_pin: 1,
            frequency_hz: 20000,
            max_duty: 1.0,
            ramp_rate: 2.0,
        }
    }
}

/// Servo configuration.
#[derive(Debug, Clone)]
pub struct ServoConfig {
    /// PWM pin.
    pub pin: u32,
    /// Minimum pulse width in microseconds (0°).
    pub min_pulse_us: u32,
    /// Maximum pulse width in microseconds (180°).
    pub max_pulse_us: u32,
    /// Maximum angular speed (degrees per second).
    pub max_speed_dps: f64,
}

impl Default for ServoConfig {
    fn default() -> Self {
        Self {
            pin: 0,
            min_pulse_us: 500,
            max_pulse_us: 2500,
            max_speed_dps: 300.0,
        }
    }
}

impl ServoConfig {
    /// Converts angle to pulse width.
    pub fn angle_to_pulse(&self, angle: f64) -> u32 {
        let clamped = angle.clamp(0.0, 180.0);
        let fraction = clamped / 180.0;
        let range = self.max_pulse_us - self.min_pulse_us;
        self.min_pulse_us + (fraction * range as f64) as u32
    }
}

/// CAN bus configuration.
#[derive(Debug, Clone)]
pub struct CanConfig {
    /// Bitrate (e.g., 500000 for 500kbps).
    pub bitrate: u32,
    /// CAN-FD enabled.
    pub fd_enabled: bool,
    /// Maximum data length (8 for CAN, 64 for CAN-FD).
    pub max_dlc: u8,
    /// Acceptance filter ID.
    pub filter_id: u32,
    /// Acceptance filter mask.
    pub filter_mask: u32,
}

impl Default for CanConfig {
    fn default() -> Self {
        Self {
            bitrate: 500000,
            fd_enabled: false,
            max_dlc: 8,
            filter_id: 0,
            filter_mask: 0,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// R2.2.6: Safety Interlock
// ═══════════════════════════════════════════════════════════════════════

/// Safety condition checker.
#[derive(Debug, Clone)]
pub struct SafetyInterlock {
    /// Safety rules.
    pub rules: Vec<SafetyRule>,
    /// Whether the interlock is tripped.
    pub tripped: bool,
    /// Trip reason.
    pub trip_reason: Option<String>,
}

/// A single safety rule.
#[derive(Debug, Clone)]
pub struct SafetyRule {
    /// Rule name.
    pub name: String,
    /// Rule description.
    pub description: String,
    /// Rule kind.
    pub kind: SafetyRuleKind,
}

/// Safety rule kinds.
#[derive(Debug, Clone)]
pub enum SafetyRuleKind {
    /// Maximum value for a reading.
    MaxValue { sensor: String, threshold: f64 },
    /// Minimum value for a reading.
    MinValue { sensor: String, threshold: f64 },
    /// Maximum rate of change.
    MaxRate { sensor: String, max_per_second: f64 },
    /// Heartbeat timeout (stop if no update in N ms).
    Heartbeat { timeout_ms: u64 },
    /// Custom predicate (name of check function).
    Custom { check_fn: String },
}

impl Default for SafetyInterlock {
    fn default() -> Self {
        Self::new()
    }
}

impl SafetyInterlock {
    /// Creates a new interlock with no rules.
    pub fn new() -> Self {
        Self {
            rules: Vec::new(),
            tripped: false,
            trip_reason: None,
        }
    }

    /// Adds a safety rule.
    pub fn add_rule(&mut self, rule: SafetyRule) {
        self.rules.push(rule);
    }

    /// Trips the interlock.
    pub fn trip(&mut self, reason: &str) {
        self.tripped = true;
        self.trip_reason = Some(reason.to_string());
    }

    /// Resets the interlock (requires manual reset).
    pub fn reset(&mut self) {
        self.tripped = false;
        self.trip_reason = None;
    }

    /// Number of rules.
    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// R2.2.7: PID Controller
// ═══════════════════════════════════════════════════════════════════════

/// PID controller.
#[derive(Debug, Clone)]
pub struct PidController {
    /// Proportional gain.
    pub kp: f64,
    /// Integral gain.
    pub ki: f64,
    /// Derivative gain.
    pub kd: f64,
    /// Integral accumulator.
    pub integral: f64,
    /// Previous error (for derivative).
    pub prev_error: f64,
    /// Output limits.
    pub output_min: f64,
    pub output_max: f64,
    /// Anti-windup limit for integral.
    pub integral_limit: f64,
    /// Setpoint.
    pub setpoint: f64,
}

impl PidController {
    /// Creates a new PID controller.
    pub fn new(kp: f64, ki: f64, kd: f64) -> Self {
        Self {
            kp,
            ki,
            kd,
            integral: 0.0,
            prev_error: 0.0,
            output_min: -1.0,
            output_max: 1.0,
            integral_limit: 100.0,
            setpoint: 0.0,
        }
    }

    /// Sets the setpoint (target value).
    pub fn set_target(&mut self, target: f64) {
        self.setpoint = target;
    }

    /// Computes the PID output given the current measurement.
    pub fn compute(&mut self, measurement: f64, dt: f64) -> f64 {
        let error = self.setpoint - measurement;

        // Proportional
        let p = self.kp * error;

        // Integral (with anti-windup)
        self.integral += error * dt;
        self.integral = self
            .integral
            .clamp(-self.integral_limit, self.integral_limit);
        let i = self.ki * self.integral;

        // Derivative
        let derivative = if dt > 0.0 {
            (error - self.prev_error) / dt
        } else {
            0.0
        };
        let d = self.kd * derivative;

        self.prev_error = error;

        // Clamp output
        (p + i + d).clamp(self.output_min, self.output_max)
    }

    /// Resets the controller state.
    pub fn reset(&mut self) {
        self.integral = 0.0;
        self.prev_error = 0.0;
    }
}

// ═══════════════════════════════════════════════════════════════════════
// R2.2.8: Command Smoothing (Ramp Rate Limiter)
// ═══════════════════════════════════════════════════════════════════════

/// Ramp rate limiter for smooth actuator transitions.
#[derive(Debug, Clone)]
pub struct RampLimiter {
    /// Current output value.
    pub current: f64,
    /// Maximum change per second.
    pub max_rate: f64,
}

impl RampLimiter {
    /// Creates a new ramp limiter.
    pub fn new(initial: f64, max_rate: f64) -> Self {
        Self {
            current: initial,
            max_rate,
        }
    }

    /// Applies ramp limiting to a target value.
    pub fn apply(&mut self, target: f64, dt: f64) -> f64 {
        let max_change = self.max_rate * dt;
        let diff = target - self.current;
        let clamped = diff.clamp(-max_change, max_change);
        self.current += clamped;
        self.current
    }
}

// ═══════════════════════════════════════════════════════════════════════
// R2.2.10: Fail-Safe Defaults
// ═══════════════════════════════════════════════════════════════════════

/// Fail-safe configuration for an actuator.
#[derive(Debug, Clone)]
pub struct FailSafeConfig {
    /// Default command on communication loss.
    pub default_command: Command,
    /// Timeout before entering fail-safe (ms).
    pub timeout_ms: u64,
    /// Last command timestamp (for timeout detection).
    pub last_command_ms: u64,
}

impl FailSafeConfig {
    /// Checks if the fail-safe timeout has elapsed.
    pub fn is_timed_out(&self, now_ms: u64) -> bool {
        now_ms - self.last_command_ms > self.timeout_ms
    }

    /// Updates the last command timestamp.
    pub fn touch(&mut self, now_ms: u64) {
        self.last_command_ms = now_ms;
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn r2_2_servo_angle_to_pulse() {
        let cfg = ServoConfig::default();
        assert_eq!(cfg.angle_to_pulse(0.0), 500);
        assert_eq!(cfg.angle_to_pulse(90.0), 1500);
        assert_eq!(cfg.angle_to_pulse(180.0), 2500);
    }

    #[test]
    fn r2_2_servo_clamp() {
        let cfg = ServoConfig::default();
        assert_eq!(cfg.angle_to_pulse(-10.0), 500); // clamped to 0
        assert_eq!(cfg.angle_to_pulse(200.0), 2500); // clamped to 180
    }

    #[test]
    fn r2_6_safety_interlock() {
        let mut interlock = SafetyInterlock::new();
        interlock.add_rule(SafetyRule {
            name: "max_temp".to_string(),
            description: "Temperature must be < 80°C".to_string(),
            kind: SafetyRuleKind::MaxValue {
                sensor: "temp".to_string(),
                threshold: 80.0,
            },
        });
        assert_eq!(interlock.rule_count(), 1);
        assert!(!interlock.tripped);

        interlock.trip("temperature exceeded 80°C");
        assert!(interlock.tripped);
        assert_eq!(
            interlock.trip_reason.as_deref(),
            Some("temperature exceeded 80°C")
        );

        interlock.reset();
        assert!(!interlock.tripped);
    }

    #[test]
    fn r2_7_pid_controller() {
        let mut pid = PidController::new(1.0, 0.1, 0.01);
        pid.set_target(10.0);

        // Simulate approaching setpoint
        let mut measurement = 0.0;
        for _ in 0..100 {
            let output = pid.compute(measurement, 0.01);
            measurement += output * 0.1; // simple plant model
        }
        // Should be close to setpoint
        assert!((measurement - 10.0).abs() < 2.0);
    }

    #[test]
    fn r2_7_pid_reset() {
        let mut pid = PidController::new(1.0, 0.5, 0.0);
        pid.set_target(5.0);
        pid.compute(0.0, 0.1);
        pid.compute(0.0, 0.1);
        assert!(pid.integral != 0.0);
        pid.reset();
        assert_eq!(pid.integral, 0.0);
        assert_eq!(pid.prev_error, 0.0);
    }

    #[test]
    fn r2_8_ramp_limiter() {
        let mut ramp = RampLimiter::new(0.0, 1.0); // max 1.0/s
        let v1 = ramp.apply(10.0, 0.1); // max change = 0.1
        assert!((v1 - 0.1).abs() < 0.001);
        let v2 = ramp.apply(10.0, 0.1);
        assert!((v2 - 0.2).abs() < 0.001);
    }

    #[test]
    fn r2_8_ramp_limiter_reverse() {
        let mut ramp = RampLimiter::new(1.0, 2.0);
        let v = ramp.apply(-1.0, 0.1); // max change = 0.2
        assert!((v - 0.8).abs() < 0.001); // 1.0 - 0.2
    }

    #[test]
    fn r2_10_failsafe() {
        let mut fs = FailSafeConfig {
            default_command: Command::Stop,
            timeout_ms: 1000,
            last_command_ms: 5000,
        };
        assert!(!fs.is_timed_out(5500)); // 500ms < 1000ms
        assert!(fs.is_timed_out(6500)); // 1500ms > 1000ms
        fs.touch(6500);
        assert!(!fs.is_timed_out(7000)); // 500ms < 1000ms
    }

    #[test]
    fn r2_2_actuator_error_display() {
        assert_eq!(format!("{}", ActuatorError::NotReady), "actuator not ready");
        assert_eq!(format!("{}", ActuatorError::Timeout), "actuator timeout");
    }

    #[test]
    fn r2_2_can_config_default() {
        let cfg = CanConfig::default();
        assert_eq!(cfg.bitrate, 500000);
        assert!(!cfg.fd_enabled);
        assert_eq!(cfg.max_dlc, 8);
    }
}
