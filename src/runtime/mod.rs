//! Fajar Lang runtime — OS, ML, and GPU execution backends.

pub mod async_io;
pub mod crossplatform;
pub mod gpu;
pub mod ml;
pub mod os;
pub mod platform;
pub mod profiler;
pub mod simd;

// Re-export IoT connectivity (WiFi, BLE, MQTT, LoRaWAN, OTA).
pub use crate::iot;

/// Returns the list of supported IoT protocol names.
pub fn iot_protocol_names() -> Vec<&'static str> {
    vec!["wifi", "ble", "mqtt", "lorawan", "ota"]
}
