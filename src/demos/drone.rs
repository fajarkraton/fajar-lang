//! Drone firmware demo — flight controller skeleton, sensor fusion, real-time
//! inference, motor control, failsafe logic, telemetry, cross-compilation.

use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// S37.2: Sensor Fusion — Complementary Filter
// ═══════════════════════════════════════════════════════════════════════

/// IMU sensor reading (accelerometer + gyroscope).
#[derive(Debug, Clone, Copy)]
pub struct ImuReading {
    /// Accelerometer X (m/s^2).
    pub accel_x: f64,
    /// Accelerometer Y (m/s^2).
    pub accel_y: f64,
    /// Accelerometer Z (m/s^2).
    pub accel_z: f64,
    /// Gyroscope X (rad/s).
    pub gyro_x: f64,
    /// Gyroscope Y (rad/s).
    pub gyro_y: f64,
    /// Gyroscope Z (rad/s).
    pub gyro_z: f64,
}

/// Attitude estimate from sensor fusion.
#[derive(Debug, Clone, Copy)]
pub struct Attitude {
    /// Roll angle (radians).
    pub roll: f64,
    /// Pitch angle (radians).
    pub pitch: f64,
    /// Yaw angle (radians).
    pub yaw: f64,
}

impl Default for Attitude {
    fn default() -> Self {
        Self {
            roll: 0.0,
            pitch: 0.0,
            yaw: 0.0,
        }
    }
}

/// Complementary filter for IMU sensor fusion.
///
/// Blends accelerometer (low-freq, noisy) with gyroscope (high-freq, drifts)
/// using tunable alpha parameter.
#[derive(Debug, Clone)]
pub struct ComplementaryFilter {
    /// Blending coefficient (0..1, higher = more gyro trust).
    pub alpha: f64,
    /// Current attitude estimate.
    pub attitude: Attitude,
}

impl ComplementaryFilter {
    /// Creates a new filter with the given alpha.
    pub fn new(alpha: f64) -> Self {
        Self {
            alpha: alpha.clamp(0.0, 1.0),
            attitude: Attitude::default(),
        }
    }

    /// Updates attitude estimate with a new IMU reading.
    pub fn update(&mut self, imu: &ImuReading, dt: f64) {
        // Accelerometer-derived angles
        let accel_roll = imu.accel_y.atan2(imu.accel_z);
        let accel_pitch =
            (-imu.accel_x).atan2((imu.accel_y * imu.accel_y + imu.accel_z * imu.accel_z).sqrt());

        // Gyroscope integration
        let gyro_roll = self.attitude.roll + imu.gyro_x * dt;
        let gyro_pitch = self.attitude.pitch + imu.gyro_y * dt;
        let gyro_yaw = self.attitude.yaw + imu.gyro_z * dt;

        // Complementary blend
        self.attitude.roll = self.alpha * gyro_roll + (1.0 - self.alpha) * accel_roll;
        self.attitude.pitch = self.alpha * gyro_pitch + (1.0 - self.alpha) * accel_pitch;
        self.attitude.yaw = gyro_yaw; // No magnetometer → gyro-only yaw
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S37.3: Real-Time Inference
// ═══════════════════════════════════════════════════════════════════════

/// Obstacle avoidance inference result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AvoidanceAction {
    /// Continue current path.
    Continue,
    /// Turn left.
    TurnLeft,
    /// Turn right.
    TurnRight,
    /// Climb to avoid obstacle.
    Climb,
    /// Emergency stop.
    EmergencyStop,
}

impl fmt::Display for AvoidanceAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Continue => write!(f, "CONTINUE"),
            Self::TurnLeft => write!(f, "TURN_LEFT"),
            Self::TurnRight => write!(f, "TURN_RIGHT"),
            Self::Climb => write!(f, "CLIMB"),
            Self::EmergencyStop => write!(f, "EMERGENCY_STOP"),
        }
    }
}

/// Simulated obstacle detection from sensor data.
pub fn infer_avoidance(distances: &[f64; 4]) -> AvoidanceAction {
    let front = distances[0];
    let left = distances[1];
    let right = distances[2];
    let bottom = distances[3];

    if front < 0.5 || bottom < 0.3 {
        AvoidanceAction::EmergencyStop
    } else if front < 2.0 {
        if left > right {
            AvoidanceAction::TurnLeft
        } else {
            AvoidanceAction::TurnRight
        }
    } else if bottom < 1.0 {
        AvoidanceAction::Climb
    } else {
        AvoidanceAction::Continue
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S37.4: Motor Control
// ═══════════════════════════════════════════════════════════════════════

/// PWM duty cycle for a motor (0.0 to 1.0).
#[derive(Debug, Clone, Copy)]
pub struct MotorOutput {
    /// Front-left motor duty.
    pub fl: f64,
    /// Front-right motor duty.
    pub fr: f64,
    /// Back-left motor duty.
    pub bl: f64,
    /// Back-right motor duty.
    pub br: f64,
}

impl MotorOutput {
    /// Creates a hover output (all motors equal).
    pub fn hover(throttle: f64) -> Self {
        let t = throttle.clamp(0.0, 1.0);
        Self {
            fl: t,
            fr: t,
            bl: t,
            br: t,
        }
    }

    /// Mixes attitude corrections into motor outputs.
    pub fn mix(throttle: f64, roll: f64, pitch: f64, yaw: f64) -> Self {
        Self {
            fl: (throttle + pitch + roll - yaw).clamp(0.0, 1.0),
            fr: (throttle + pitch - roll + yaw).clamp(0.0, 1.0),
            bl: (throttle - pitch + roll + yaw).clamp(0.0, 1.0),
            br: (throttle - pitch - roll - yaw).clamp(0.0, 1.0),
        }
    }

    /// Returns the average duty cycle.
    pub fn average(&self) -> f64 {
        (self.fl + self.fr + self.bl + self.br) / 4.0
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S37.5: Failsafe Logic
// ═══════════════════════════════════════════════════════════════════════

/// Failsafe trigger conditions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailsafeTrigger {
    /// Watchdog timer expired (no heartbeat).
    WatchdogTimeout,
    /// Battery critically low.
    LowBattery,
    /// GPS geofence violation.
    GeofenceViolation,
    /// Communication link lost.
    LinkLost,
}

impl fmt::Display for FailsafeTrigger {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WatchdogTimeout => write!(f, "WATCHDOG_TIMEOUT"),
            Self::LowBattery => write!(f, "LOW_BATTERY"),
            Self::GeofenceViolation => write!(f, "GEOFENCE_VIOLATION"),
            Self::LinkLost => write!(f, "LINK_LOST"),
        }
    }
}

/// GPS position.
#[derive(Debug, Clone, Copy)]
pub struct GpsPosition {
    /// Latitude in degrees.
    pub lat: f64,
    /// Longitude in degrees.
    pub lon: f64,
    /// Altitude in meters.
    pub alt: f64,
}

/// GPS geofence (circular).
#[derive(Debug, Clone, Copy)]
pub struct Geofence {
    /// Center latitude.
    pub center_lat: f64,
    /// Center longitude.
    pub center_lon: f64,
    /// Radius in meters.
    pub radius_m: f64,
    /// Maximum altitude in meters.
    pub max_alt_m: f64,
}

impl Geofence {
    /// Checks if a position is within the geofence.
    pub fn contains(&self, pos: &GpsPosition) -> bool {
        let dlat = (pos.lat - self.center_lat) * 111_320.0; // meters
        let dlon = (pos.lon - self.center_lon) * 111_320.0 * self.center_lat.to_radians().cos();
        let dist = (dlat * dlat + dlon * dlon).sqrt();
        dist <= self.radius_m && pos.alt <= self.max_alt_m
    }
}

/// Checks all failsafe conditions.
pub fn check_failsafe(
    battery_pct: f64,
    last_heartbeat_ms: u64,
    watchdog_timeout_ms: u64,
    gps: &GpsPosition,
    fence: &Geofence,
) -> Option<FailsafeTrigger> {
    if last_heartbeat_ms > watchdog_timeout_ms {
        return Some(FailsafeTrigger::WatchdogTimeout);
    }
    if battery_pct < 10.0 {
        return Some(FailsafeTrigger::LowBattery);
    }
    if !fence.contains(gps) {
        return Some(FailsafeTrigger::GeofenceViolation);
    }
    None
}

// ═══════════════════════════════════════════════════════════════════════
// S37.6: Telemetry
// ═══════════════════════════════════════════════════════════════════════

/// Flight telemetry packet sent at 10Hz over UART.
#[derive(Debug, Clone)]
pub struct TelemetryPacket {
    /// Attitude (roll, pitch, yaw in radians).
    pub attitude: Attitude,
    /// Altitude in meters.
    pub altitude: f64,
    /// Battery percentage.
    pub battery_pct: f64,
    /// GPS position.
    pub gps: GpsPosition,
    /// Current flight mode.
    pub mode: FlightMode,
    /// Sequence number.
    pub seq: u32,
}

/// Flight mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlightMode {
    /// On the ground, motors off.
    Disarmed,
    /// Hovering in place.
    Hover,
    /// Following waypoints.
    Mission,
    /// Returning to home.
    ReturnToHome,
    /// Emergency landing.
    EmergencyLand,
}

impl fmt::Display for FlightMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Disarmed => write!(f, "DISARMED"),
            Self::Hover => write!(f, "HOVER"),
            Self::Mission => write!(f, "MISSION"),
            Self::ReturnToHome => write!(f, "RTH"),
            Self::EmergencyLand => write!(f, "EMER_LAND"),
        }
    }
}

impl TelemetryPacket {
    /// Formats the telemetry packet as a UART-friendly string.
    pub fn to_uart_string(&self) -> String {
        format!(
            "$TEL,{},{:.2},{:.2},{:.2},{:.1},{:.0},{:.6},{:.6},{:.1},{},{}",
            self.seq,
            self.attitude.roll.to_degrees(),
            self.attitude.pitch.to_degrees(),
            self.attitude.yaw.to_degrees(),
            self.altitude,
            self.battery_pct,
            self.gps.lat,
            self.gps.lon,
            self.gps.alt,
            self.mode,
            checksum_nmea(&format!(
                "{},{:.2},{:.2}",
                self.seq, self.attitude.roll, self.altitude
            ))
        )
    }
}

/// Simple XOR checksum (NMEA-style).
fn checksum_nmea(data: &str) -> String {
    let cs = data.bytes().fold(0u8, |acc, b| acc ^ b);
    format!("{cs:02X}")
}

// ═══════════════════════════════════════════════════════════════════════
// S37.1: Flight Controller Skeleton
// ═══════════════════════════════════════════════════════════════════════

/// Flight controller main loop step result.
#[derive(Debug, Clone)]
pub struct ControlStep {
    /// Current attitude.
    pub attitude: Attitude,
    /// Motor outputs.
    pub motors: MotorOutput,
    /// Avoidance action taken.
    pub avoidance: AvoidanceAction,
    /// Failsafe triggered (if any).
    pub failsafe: Option<FailsafeTrigger>,
    /// Step number.
    pub step: u32,
}

/// Runs one iteration of the flight controller.
#[allow(clippy::too_many_arguments)]
pub fn control_loop_step(
    filter: &mut ComplementaryFilter,
    imu: &ImuReading,
    distances: &[f64; 4],
    battery_pct: f64,
    gps: &GpsPosition,
    fence: &Geofence,
    step: u32,
    dt: f64,
) -> ControlStep {
    // Step 1: Sensor fusion
    filter.update(imu, dt);
    let attitude = filter.attitude;

    // Step 2: Obstacle avoidance inference
    let avoidance = infer_avoidance(distances);

    // Step 3: Motor mixing
    let motors = match avoidance {
        AvoidanceAction::EmergencyStop => MotorOutput::hover(0.0),
        AvoidanceAction::Climb => MotorOutput::mix(0.7, 0.0, 0.05, 0.0),
        AvoidanceAction::TurnLeft => MotorOutput::mix(0.5, 0.0, 0.0, -0.1),
        AvoidanceAction::TurnRight => MotorOutput::mix(0.5, 0.0, 0.0, 0.1),
        AvoidanceAction::Continue => MotorOutput::hover(0.5),
    };

    // Step 4: Failsafe check
    let failsafe = check_failsafe(battery_pct, 0, 1000, gps, fence);

    ControlStep {
        attitude,
        motors,
        avoidance,
        failsafe,
        step,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S37.7: Cross-Compile Target
// ═══════════════════════════════════════════════════════════════════════

/// Cross-compilation target for Jetson Thor.
#[derive(Debug, Clone)]
pub struct JetsonThorTarget {
    /// Target triple.
    pub triple: String,
    /// JetPack version.
    pub jetpack_version: String,
    /// CUDA compute capability.
    pub cuda_cc: String,
    /// Linker flags.
    pub linker_flags: Vec<String>,
}

impl Default for JetsonThorTarget {
    fn default() -> Self {
        Self {
            triple: "aarch64-unknown-linux-gnu".to_string(),
            jetpack_version: "7.1".to_string(),
            cuda_cc: "10.0".to_string(),
            linker_flags: vec![
                "-L/usr/local/cuda/lib64".to_string(),
                "-lcudart".to_string(),
                "-lnvinfer".to_string(),
            ],
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S37.8: Simulation Mode
// ═══════════════════════════════════════════════════════════════════════

/// Simulated sensor provider for QEMU testing.
#[derive(Debug, Clone)]
pub struct SimulatedSensors {
    /// Current step.
    step: u32,
    /// Simulated altitude.
    altitude: f64,
}

impl SimulatedSensors {
    /// Creates a new simulated sensor provider.
    pub fn new() -> Self {
        Self {
            step: 0,
            altitude: 10.0,
        }
    }

    /// Generates a simulated IMU reading.
    pub fn read_imu(&mut self) -> ImuReading {
        self.step += 1;
        let t = self.step as f64 * 0.033; // 30Hz
        ImuReading {
            accel_x: 0.1 * (t * 2.0).sin(),
            accel_y: 0.05 * (t * 3.0).cos(),
            accel_z: -9.81 + 0.02 * (t * 5.0).sin(),
            gyro_x: 0.01 * (t * 1.5).sin(),
            gyro_y: 0.02 * (t * 2.5).cos(),
            gyro_z: 0.005,
        }
    }

    /// Generates simulated distance readings.
    pub fn read_distances(&self) -> [f64; 4] {
        [5.0, 3.0, 4.0, self.altitude] // front, left, right, bottom
    }

    /// Returns simulated GPS position.
    pub fn read_gps(&self) -> GpsPosition {
        GpsPosition {
            lat: -6.2088,
            lon: 106.8456,
            alt: self.altitude,
        }
    }
}

impl Default for SimulatedSensors {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S37.9: Demo Script
// ═══════════════════════════════════════════════════════════════════════

/// Generates the step-by-step demo reproduction guide.
pub fn demo_script() -> String {
    [
        "# Drone Firmware Demo — Reproduction Guide",
        "",
        "## Prerequisites",
        "- Fajar Lang toolchain (`fj` binary)",
        "- QEMU with aarch64 support (for simulation)",
        "- Optional: NVIDIA Jetson Thor with JetPack 7.1 (for real hardware)",
        "",
        "## Steps",
        "",
        "### 1. Build for simulation",
        "```bash",
        "fj build --target aarch64-unknown-linux-gnu examples/drone_firmware_demo.fj",
        "```",
        "",
        "### 2. Run in QEMU",
        "```bash",
        "qemu-system-aarch64 -M virt -cpu cortex-a76 -kernel build/drone_firmware",
        "```",
        "",
        "### 3. Observe telemetry",
        "Serial output shows 10Hz telemetry: attitude, altitude, battery, GPS.",
        "",
        "### 4. Cross-compile for Jetson Thor",
        "```bash",
        "fj build --target aarch64-jetson-thor --jetpack 7.1 examples/drone_firmware_demo.fj",
        "```",
        "",
        "### 5. Deploy and test",
        "Flash to Jetson Thor, connect via UART, observe real-time inference at 30Hz.",
    ]
    .join("\n")
}

// ═══════════════════════════════════════════════════════════════════════
// S37.10: Demo Video Script
// ═══════════════════════════════════════════════════════════════════════

/// Generates the 3-minute video script outline.
pub fn video_script() -> String {
    [
        "# Drone Firmware Demo — Video Script (3 minutes)",
        "",
        "## 0:00-0:30 — Introduction",
        "\"Today we're running a full drone flight controller written in Fajar Lang.\"",
        "Show code in editor: sensor fusion, obstacle avoidance, motor control.",
        "",
        "## 0:30-1:00 — Build & Deploy",
        "Terminal: `fj build --target aarch64-jetson-thor`",
        "Show cross-compilation output, binary size.",
        "",
        "## 1:00-2:00 — Flight Simulation",
        "QEMU aarch64: boot, see telemetry streaming at 10Hz.",
        "Show attitude, altitude, battery, GPS in real-time.",
        "Trigger obstacle: see avoidance action change to TURN_LEFT.",
        "",
        "## 2:00-2:30 — Failsafe Demo",
        "Simulate low battery: see EMERGENCY_LAND trigger.",
        "Simulate geofence violation: see RTH activate.",
        "",
        "## 2:30-3:00 — Conclusion",
        "\"One language: sensor fusion, ML inference, motor control, failsafe.\"",
        "\"Fajar Lang — from simulation to silicon.\"",
    ]
    .join("\n")
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // S37.1: Flight controller
    #[test]
    fn s37_1_control_loop_step() {
        let mut filter = ComplementaryFilter::new(0.98);
        let imu = ImuReading {
            accel_x: 0.0,
            accel_y: 0.0,
            accel_z: -9.81,
            gyro_x: 0.0,
            gyro_y: 0.0,
            gyro_z: 0.0,
        };
        let distances = [5.0, 3.0, 4.0, 10.0];
        let gps = GpsPosition {
            lat: -6.2088,
            lon: 106.8456,
            alt: 10.0,
        };
        let fence = Geofence {
            center_lat: -6.2088,
            center_lon: 106.8456,
            radius_m: 500.0,
            max_alt_m: 120.0,
        };
        let step = control_loop_step(&mut filter, &imu, &distances, 80.0, &gps, &fence, 0, 0.033);
        assert_eq!(step.avoidance, AvoidanceAction::Continue);
        assert!(step.failsafe.is_none());
        assert!(step.motors.average() > 0.0);
    }

    // S37.2: Sensor fusion
    #[test]
    fn s37_2_complementary_filter() {
        let mut filter = ComplementaryFilter::new(0.98);
        let imu = ImuReading {
            accel_x: 0.0,
            accel_y: 0.0,
            accel_z: -9.81,
            gyro_x: 0.1,
            gyro_y: 0.0,
            gyro_z: 0.0,
        };
        filter.update(&imu, 0.033);
        // Should produce a small roll from gyro
        assert!(filter.attitude.roll.abs() > 0.0);
    }

    // S37.3: Real-time inference
    #[test]
    fn s37_3_obstacle_avoidance() {
        assert_eq!(
            infer_avoidance(&[5.0, 3.0, 4.0, 10.0]),
            AvoidanceAction::Continue
        );
        assert_eq!(
            infer_avoidance(&[0.3, 3.0, 4.0, 10.0]),
            AvoidanceAction::EmergencyStop
        );
        assert_eq!(
            infer_avoidance(&[1.5, 5.0, 3.0, 10.0]),
            AvoidanceAction::TurnLeft
        );
        assert_eq!(
            infer_avoidance(&[1.5, 3.0, 5.0, 10.0]),
            AvoidanceAction::TurnRight
        );
        assert_eq!(
            infer_avoidance(&[5.0, 3.0, 4.0, 0.8]),
            AvoidanceAction::Climb
        );
    }

    // S37.4: Motor control
    #[test]
    fn s37_4_motor_hover() {
        let m = MotorOutput::hover(0.5);
        assert!((m.fl - 0.5).abs() < 0.001);
        assert!((m.average() - 0.5).abs() < 0.001);
    }

    #[test]
    fn s37_4_motor_mix() {
        let m = MotorOutput::mix(0.5, 0.1, 0.0, 0.0);
        // Roll correction: FL and BL get +roll, FR and BR get -roll
        assert!(m.fl > m.fr);
    }

    // S37.5: Failsafe
    #[test]
    fn s37_5_failsafe_low_battery() {
        let gps = GpsPosition {
            lat: 0.0,
            lon: 0.0,
            alt: 10.0,
        };
        let fence = Geofence {
            center_lat: 0.0,
            center_lon: 0.0,
            radius_m: 1000.0,
            max_alt_m: 120.0,
        };
        let result = check_failsafe(5.0, 0, 1000, &gps, &fence);
        assert_eq!(result, Some(FailsafeTrigger::LowBattery));
    }

    #[test]
    fn s37_5_failsafe_watchdog() {
        let gps = GpsPosition {
            lat: 0.0,
            lon: 0.0,
            alt: 10.0,
        };
        let fence = Geofence {
            center_lat: 0.0,
            center_lon: 0.0,
            radius_m: 1000.0,
            max_alt_m: 120.0,
        };
        let result = check_failsafe(80.0, 2000, 1000, &gps, &fence);
        assert_eq!(result, Some(FailsafeTrigger::WatchdogTimeout));
    }

    #[test]
    fn s37_5_failsafe_geofence() {
        let gps = GpsPosition {
            lat: 10.0,
            lon: 10.0,
            alt: 10.0,
        };
        let fence = Geofence {
            center_lat: 0.0,
            center_lon: 0.0,
            radius_m: 100.0,
            max_alt_m: 120.0,
        };
        let result = check_failsafe(80.0, 0, 1000, &gps, &fence);
        assert_eq!(result, Some(FailsafeTrigger::GeofenceViolation));
    }

    #[test]
    fn s37_5_failsafe_none() {
        let gps = GpsPosition {
            lat: 0.0,
            lon: 0.0,
            alt: 10.0,
        };
        let fence = Geofence {
            center_lat: 0.0,
            center_lon: 0.0,
            radius_m: 1000.0,
            max_alt_m: 120.0,
        };
        let result = check_failsafe(80.0, 0, 1000, &gps, &fence);
        assert!(result.is_none());
    }

    // S37.6: Telemetry
    #[test]
    fn s37_6_telemetry_packet() {
        let pkt = TelemetryPacket {
            attitude: Attitude {
                roll: 0.1,
                pitch: -0.05,
                yaw: 1.5,
            },
            altitude: 15.0,
            battery_pct: 85.0,
            gps: GpsPosition {
                lat: -6.2088,
                lon: 106.8456,
                alt: 15.0,
            },
            mode: FlightMode::Mission,
            seq: 42,
        };
        let uart = pkt.to_uart_string();
        assert!(uart.starts_with("$TEL,42"));
        assert!(uart.contains("MISSION"));
    }

    // S37.7: Cross-compile target
    #[test]
    fn s37_7_jetson_target() {
        let target = JetsonThorTarget::default();
        assert_eq!(target.triple, "aarch64-unknown-linux-gnu");
        assert_eq!(target.jetpack_version, "7.1");
        assert!(target.linker_flags.iter().any(|f| f.contains("cudart")));
    }

    // S37.8: Simulation mode
    #[test]
    fn s37_8_simulated_sensors() {
        let mut sim = SimulatedSensors::new();
        let imu = sim.read_imu();
        assert!(imu.accel_z < 0.0); // Gravity
        let distances = sim.read_distances();
        assert_eq!(distances.len(), 4);
        let gps = sim.read_gps();
        assert!(gps.alt > 0.0);
    }

    // S37.9: Demo script
    #[test]
    fn s37_9_demo_script() {
        let script = demo_script();
        assert!(script.contains("Reproduction Guide"));
        assert!(script.contains("qemu"));
        assert!(script.contains("Jetson Thor"));
    }

    // S37.10: Video script
    #[test]
    fn s37_10_video_script() {
        let script = video_script();
        assert!(script.contains("Video Script"));
        assert!(script.contains("3 minutes"));
        assert!(script.contains("Failsafe"));
    }
}
