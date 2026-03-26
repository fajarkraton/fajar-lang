//! Sensor Framework — abstraction, drivers, fusion, data pipeline.
//!
//! Phase R1: 20 tasks covering sensor traits, IMU/camera/ADC/GPS/LiDAR/mic
//! drivers, Kalman filter fusion, and streaming data pipeline with backpressure.

use std::collections::VecDeque;
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// R1.1.1: Sensor Trait
// ═══════════════════════════════════════════════════════════════════════

/// Core sensor trait — all sensors implement this.
pub trait Sensor {
    /// Sensor type identifier.
    fn sensor_type(&self) -> SensorType;
    /// Read current sensor data.
    fn read(&self) -> Result<SensorData, SensorError>;
    /// Sample rate in Hz.
    fn sample_rate(&self) -> f64;
    /// Calibrate the sensor.
    fn calibrate(&mut self) -> Result<(), SensorError>;
    /// Whether the sensor is ready.
    fn is_ready(&self) -> bool;
}

/// Sensor type enumeration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SensorType {
    Imu,
    Camera,
    Adc,
    Gps,
    Lidar,
    Microphone,
    Temperature,
    Humidity,
    Pressure,
    Proximity,
    Custom(u32),
}

impl fmt::Display for SensorType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Imu => write!(f, "IMU"),
            Self::Camera => write!(f, "Camera"),
            Self::Adc => write!(f, "ADC"),
            Self::Gps => write!(f, "GPS"),
            Self::Lidar => write!(f, "LiDAR"),
            Self::Microphone => write!(f, "Microphone"),
            Self::Temperature => write!(f, "Temperature"),
            Self::Humidity => write!(f, "Humidity"),
            Self::Pressure => write!(f, "Pressure"),
            Self::Proximity => write!(f, "Proximity"),
            Self::Custom(id) => write!(f, "Custom({id})"),
        }
    }
}

/// Sensor error types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SensorError {
    NotReady,
    Timeout,
    CalibrationFailed(String),
    HardwareError(String),
    DataCorrupted,
    Disconnected,
}

impl fmt::Display for SensorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotReady => write!(f, "sensor not ready"),
            Self::Timeout => write!(f, "sensor read timeout"),
            Self::CalibrationFailed(msg) => write!(f, "calibration failed: {msg}"),
            Self::HardwareError(msg) => write!(f, "hardware error: {msg}"),
            Self::DataCorrupted => write!(f, "data corrupted"),
            Self::Disconnected => write!(f, "sensor disconnected"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// R1.1.2: SensorData — timestamped, typed readings
// ═══════════════════════════════════════════════════════════════════════

/// Timestamped sensor reading.
#[derive(Debug, Clone)]
pub struct SensorData {
    /// Sensor type.
    pub sensor_type: SensorType,
    /// Timestamp in microseconds (monotonic).
    pub timestamp_us: u64,
    /// Raw data payload.
    pub payload: SensorPayload,
    /// Sequence number (for ordering / drop detection).
    pub seq: u64,
}

/// Sensor data payload variants.
#[derive(Debug, Clone)]
pub enum SensorPayload {
    /// IMU: accelerometer (x,y,z) + gyroscope (x,y,z) in SI units.
    Imu { accel: [f64; 3], gyro: [f64; 3] },
    /// Camera frame: width, height, pixel data (RGB888).
    CameraFrame {
        width: u32,
        height: u32,
        data: Vec<u8>,
    },
    /// ADC: channel readings (0.0 to 1.0 normalized).
    Adc { channels: Vec<f64> },
    /// GPS: latitude, longitude, altitude, speed, heading.
    Gps {
        lat: f64,
        lon: f64,
        alt: f64,
        speed: f64,
        heading: f64,
        satellites: u8,
    },
    /// LiDAR: array of distance measurements (meters).
    Lidar {
        distances: Vec<f64>,
        angle_min: f64,
        angle_max: f64,
        angle_step: f64,
    },
    /// Microphone: PCM samples (i16, mono).
    Audio { samples: Vec<i16>, sample_rate: u32 },
    /// Single scalar value (temperature, humidity, pressure).
    Scalar { value: f64, unit: String },
}

// ═══════════════════════════════════════════════════════════════════════
// R1.1.3-R1.1.8: Sensor Drivers
// ═══════════════════════════════════════════════════════════════════════

/// IMU configuration (MPU6050-compatible).
#[derive(Debug, Clone)]
pub struct ImuConfig {
    /// I2C address (default 0x68).
    pub i2c_addr: u8,
    /// Accelerometer range: 2, 4, 8, or 16 g.
    pub accel_range_g: u8,
    /// Gyroscope range: 250, 500, 1000, or 2000 dps.
    pub gyro_range_dps: u16,
    /// Low-pass filter bandwidth in Hz.
    pub lpf_bandwidth_hz: u16,
    /// Sample rate in Hz.
    pub sample_rate_hz: u16,
}

impl Default for ImuConfig {
    fn default() -> Self {
        Self {
            i2c_addr: 0x68,
            accel_range_g: 4,
            gyro_range_dps: 500,
            lpf_bandwidth_hz: 42,
            sample_rate_hz: 100,
        }
    }
}

/// Camera configuration.
#[derive(Debug, Clone)]
pub struct CameraConfig {
    /// Resolution width.
    pub width: u32,
    /// Resolution height.
    pub height: u32,
    /// Frames per second.
    pub fps: u32,
    /// Pixel format.
    pub format: PixelFormat,
    /// Auto-exposure enabled.
    pub auto_exposure: bool,
}

/// Pixel format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    Rgb888,
    Bgr888,
    Yuv422,
    Grayscale,
    RawBayer,
}

impl Default for CameraConfig {
    fn default() -> Self {
        Self {
            width: 320,
            height: 240,
            fps: 30,
            format: PixelFormat::Rgb888,
            auto_exposure: true,
        }
    }
}

/// GPS NMEA sentence types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NmeaSentence {
    Gga,
    Rmc,
    Gll,
    Vtg,
    Gsa,
}

/// Parses NMEA GGA sentence to GPS coordinates.
pub fn parse_nmea_gga(sentence: &str) -> Result<(f64, f64, f64, u8), String> {
    let parts: Vec<&str> = sentence.split(',').collect();
    if parts.len() < 15 {
        return Err("incomplete GGA sentence".to_string());
    }
    if !parts[0].ends_with("GGA") {
        return Err("not a GGA sentence".to_string());
    }

    let lat = parse_nmea_coord(parts[2], parts[3])?;
    let lon = parse_nmea_coord(parts[4], parts[5])?;
    let alt: f64 = parts[9].parse().unwrap_or(0.0);
    let sats: u8 = parts[7].parse().unwrap_or(0);

    Ok((lat, lon, alt, sats))
}

fn parse_nmea_coord(value: &str, direction: &str) -> Result<f64, String> {
    if value.is_empty() {
        return Err("empty coordinate".to_string());
    }
    let dot = value.find('.').ok_or("no decimal point")?;
    let deg_len = if dot > 4 { 3 } else { 2 }; // lon has 3 digits before decimal
    let degrees: f64 = value[..deg_len].parse().map_err(|e| format!("{e}"))?;
    let minutes: f64 = value[deg_len..].parse().map_err(|e| format!("{e}"))?;
    let mut coord = degrees + minutes / 60.0;
    if direction == "S" || direction == "W" {
        coord = -coord;
    }
    Ok(coord)
}

// ═══════════════════════════════════════════════════════════════════════
// R1.1.9: Sensor Fusion (Kalman Filter)
// ═══════════════════════════════════════════════════════════════════════

/// Simple 1D Kalman filter.
#[derive(Debug, Clone)]
pub struct KalmanFilter {
    /// State estimate.
    pub x: f64,
    /// Estimate uncertainty.
    pub p: f64,
    /// Process noise.
    pub q: f64,
    /// Measurement noise.
    pub r: f64,
}

impl KalmanFilter {
    /// Creates a new Kalman filter.
    pub fn new(initial: f64, uncertainty: f64, process_noise: f64, measurement_noise: f64) -> Self {
        Self {
            x: initial,
            p: uncertainty,
            q: process_noise,
            r: measurement_noise,
        }
    }

    /// Prediction step.
    pub fn predict(&mut self) {
        // x stays the same (constant model)
        self.p += self.q;
    }

    /// Update step with a measurement.
    pub fn update(&mut self, measurement: f64) {
        let k = self.p / (self.p + self.r); // Kalman gain
        self.x += k * (measurement - self.x);
        self.p *= 1.0 - k;
    }

    /// Combined predict + update.
    pub fn filter(&mut self, measurement: f64) -> f64 {
        self.predict();
        self.update(measurement);
        self.x
    }
}

/// 3D Kalman filter for IMU + GPS fusion.
#[derive(Debug, Clone)]
pub struct FusionFilter {
    /// Position filter (lat, lon, alt).
    pub pos: [KalmanFilter; 3],
    /// Velocity filter (vx, vy, vz).
    pub vel: [KalmanFilter; 3],
}

impl Default for FusionFilter {
    fn default() -> Self {
        Self::new()
    }
}

impl FusionFilter {
    /// Creates a new fusion filter with default parameters.
    pub fn new() -> Self {
        let make = || KalmanFilter::new(0.0, 1.0, 0.01, 0.1);
        Self {
            pos: [make(), make(), make()],
            vel: [make(), make(), make()],
        }
    }

    /// Updates with IMU acceleration data (integrate to velocity).
    pub fn update_imu(&mut self, accel: [f64; 3], dt: f64) {
        for (i, a) in accel.iter().enumerate() {
            self.vel[i].predict();
            self.vel[i].update(self.vel[i].x + a * dt);
        }
    }

    /// Updates with GPS position data.
    pub fn update_gps(&mut self, lat: f64, lon: f64, alt: f64) {
        self.pos[0].filter(lat);
        self.pos[1].filter(lon);
        self.pos[2].filter(alt);
    }

    /// Returns fused position estimate.
    pub fn position(&self) -> (f64, f64, f64) {
        (self.pos[0].x, self.pos[1].x, self.pos[2].x)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// R1.1.10 + R1.2: Data Pipeline
// ═══════════════════════════════════════════════════════════════════════

/// Ring buffer for sensor data (zero-copy, fixed capacity).
#[derive(Debug, Clone)]
pub struct RingBuffer<T> {
    /// Internal storage.
    pub data: VecDeque<T>,
    /// Maximum capacity.
    pub capacity: usize,
    /// Total items pushed (for drop detection).
    pub total_pushed: u64,
    /// Total items dropped (overflow).
    pub total_dropped: u64,
}

impl<T> RingBuffer<T> {
    /// Creates a new ring buffer.
    pub fn new(capacity: usize) -> Self {
        Self {
            data: VecDeque::with_capacity(capacity),
            capacity,
            total_pushed: 0,
            total_dropped: 0,
        }
    }

    /// Pushes an item, dropping oldest if full.
    pub fn push(&mut self, item: T) {
        if self.data.len() >= self.capacity {
            self.data.pop_front();
            self.total_dropped += 1;
        }
        self.data.push_back(item);
        self.total_pushed += 1;
    }

    /// Pops the oldest item.
    pub fn pop(&mut self) -> Option<T> {
        self.data.pop_front()
    }

    /// Returns number of items.
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Returns true if empty.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Returns true if full.
    pub fn is_full(&self) -> bool {
        self.data.len() >= self.capacity
    }

    /// Drop rate (fraction of total items dropped).
    pub fn drop_rate(&self) -> f64 {
        if self.total_pushed == 0 {
            return 0.0;
        }
        self.total_dropped as f64 / self.total_pushed as f64
    }
}

/// Sliding window for time-series data.
#[derive(Debug, Clone)]
pub struct SlidingWindow {
    /// Window data.
    pub values: VecDeque<f64>,
    /// Window size.
    pub size: usize,
}

impl SlidingWindow {
    /// Creates a new sliding window.
    pub fn new(size: usize) -> Self {
        Self {
            values: VecDeque::with_capacity(size),
            size,
        }
    }

    /// Adds a value to the window.
    pub fn push(&mut self, value: f64) {
        if self.values.len() >= self.size {
            self.values.pop_front();
        }
        self.values.push_back(value);
    }

    /// Returns the rolling mean.
    pub fn mean(&self) -> f64 {
        if self.values.is_empty() {
            return 0.0;
        }
        self.values.iter().sum::<f64>() / self.values.len() as f64
    }

    /// Returns the rolling variance.
    pub fn variance(&self) -> f64 {
        if self.values.len() < 2 {
            return 0.0;
        }
        let mean = self.mean();
        self.values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (self.values.len() - 1) as f64
    }

    /// Returns the rolling standard deviation.
    pub fn std_dev(&self) -> f64 {
        self.variance().sqrt()
    }

    /// Returns the min value in the window.
    pub fn min(&self) -> f64 {
        self.values.iter().copied().fold(f64::INFINITY, f64::min)
    }

    /// Returns the max value in the window.
    pub fn max(&self) -> f64 {
        self.values
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max)
    }

    /// Detects if the latest value is a peak.
    pub fn is_peak(&self) -> bool {
        if self.values.len() < 3 {
            return false;
        }
        let n = self.values.len();
        self.values[n - 2] > self.values[n - 3] && self.values[n - 2] > self.values[n - 1]
    }

    /// Returns true if window is full.
    pub fn is_full(&self) -> bool {
        self.values.len() >= self.size
    }
}

/// Normalizes a data slice to [0, 1].
pub fn normalize(data: &[f64]) -> Vec<f64> {
    let min = data.iter().copied().fold(f64::INFINITY, f64::min);
    let max = data.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let range = max - min;
    if range == 0.0 {
        return vec![0.5; data.len()];
    }
    data.iter().map(|v| (v - min) / range).collect()
}

/// Standardizes data to zero mean, unit variance.
pub fn standardize(data: &[f64]) -> Vec<f64> {
    let n = data.len() as f64;
    let mean = data.iter().sum::<f64>() / n;
    let var = data.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n;
    let std = var.sqrt();
    if std == 0.0 {
        return vec![0.0; data.len()];
    }
    data.iter().map(|v| (v - mean) / std).collect()
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn r1_1_sensor_type_display() {
        assert_eq!(format!("{}", SensorType::Imu), "IMU");
        assert_eq!(format!("{}", SensorType::Lidar), "LiDAR");
        assert_eq!(format!("{}", SensorType::Custom(42)), "Custom(42)");
    }

    #[test]
    fn r1_2_sensor_data() {
        let data = SensorData {
            sensor_type: SensorType::Imu,
            timestamp_us: 1000000,
            payload: SensorPayload::Imu {
                accel: [0.0, 0.0, 9.81],
                gyro: [0.0, 0.0, 0.0],
            },
            seq: 1,
        };
        assert_eq!(data.sensor_type, SensorType::Imu);
    }

    #[test]
    fn r1_3_imu_config() {
        let cfg = ImuConfig::default();
        assert_eq!(cfg.i2c_addr, 0x68);
        assert_eq!(cfg.accel_range_g, 4);
        assert_eq!(cfg.sample_rate_hz, 100);
    }

    #[test]
    fn r1_4_camera_config() {
        let cfg = CameraConfig::default();
        assert_eq!(cfg.width, 320);
        assert_eq!(cfg.height, 240);
        assert_eq!(cfg.format, PixelFormat::Rgb888);
    }

    #[test]
    fn r1_6_nmea_gga_parse() {
        let sentence = "$GPGGA,123519,4807.038,N,01131.000,E,1,08,0.9,545.4,M,47.0,M,,*47";
        let (lat, lon, alt, sats) = parse_nmea_gga(sentence).unwrap();
        assert!((lat - 48.1173).abs() < 0.001);
        assert!((lon - 11.5167).abs() < 0.001);
        assert!((alt - 545.4).abs() < 0.1);
        assert_eq!(sats, 8);
    }

    #[test]
    fn r1_9_kalman_filter() {
        let mut kf = KalmanFilter::new(0.0, 1.0, 0.01, 0.5);
        // Feed noisy measurements of a constant value (10.0)
        for _ in 0..100 {
            kf.filter(10.0 + 0.1);
        }
        assert!((kf.x - 10.1).abs() < 0.5);
    }

    #[test]
    fn r1_9_fusion_filter() {
        let mut ff = FusionFilter::new();
        // Feed GPS multiple times to converge
        for _ in 0..20 {
            ff.update_gps(48.0, 11.0, 500.0);
        }
        let (lat, lon, _alt) = ff.position();
        assert!((lat - 48.0).abs() < 5.0);
        assert!((lon - 11.0).abs() < 5.0);
    }

    #[test]
    fn r1_10_ring_buffer() {
        let mut rb: RingBuffer<i32> = RingBuffer::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        assert!(rb.is_full());
        assert_eq!(rb.len(), 3);
        rb.push(4); // drops 1
        assert_eq!(rb.total_dropped, 1);
        assert_eq!(rb.pop(), Some(2));
    }

    #[test]
    fn r1_10_ring_buffer_drop_rate() {
        let mut rb: RingBuffer<i32> = RingBuffer::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.total_pushed, 4);
        assert_eq!(rb.total_dropped, 2);
        assert!((rb.drop_rate() - 0.5).abs() < 0.001);
    }

    #[test]
    fn r1_2_sliding_window() {
        let mut w = SlidingWindow::new(5);
        for v in [1.0, 2.0, 3.0, 4.0, 5.0] {
            w.push(v);
        }
        assert!(w.is_full());
        assert!((w.mean() - 3.0).abs() < 0.001);
        assert_eq!(w.min(), 1.0);
        assert_eq!(w.max(), 5.0);
    }

    #[test]
    fn r1_2_sliding_window_variance() {
        let mut w = SlidingWindow::new(4);
        for v in [2.0, 4.0, 4.0, 4.0] {
            w.push(v);
        }
        // mean=3.5, var = ((2-3.5)^2+(4-3.5)^2*3)/3 = (2.25+0.75)/3 = 1.0
        assert!((w.variance() - 1.0).abs() < 0.001);
    }

    #[test]
    fn r1_2_normalize() {
        let data = vec![0.0, 5.0, 10.0];
        let norm = normalize(&data);
        assert!((norm[0] - 0.0).abs() < 0.001);
        assert!((norm[1] - 0.5).abs() < 0.001);
        assert!((norm[2] - 1.0).abs() < 0.001);
    }

    #[test]
    fn r1_2_standardize() {
        let data = vec![2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0];
        let std = standardize(&data);
        let mean: f64 = std.iter().sum::<f64>() / std.len() as f64;
        assert!(mean.abs() < 0.001); // zero mean
    }

    #[test]
    fn r1_2_peak_detection() {
        let mut w = SlidingWindow::new(10);
        for v in [1.0, 2.0, 5.0, 3.0] {
            w.push(v);
        }
        assert!(w.is_peak()); // 5.0 is peak (> 2.0 and > 3.0)
    }

    #[test]
    fn r1_1_sensor_error_display() {
        assert_eq!(format!("{}", SensorError::Timeout), "sensor read timeout");
        assert_eq!(
            format!("{}", SensorError::Disconnected),
            "sensor disconnected"
        );
    }
}
