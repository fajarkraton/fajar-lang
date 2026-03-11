//! IoT connectivity module for Fajar Lang.
//!
//! Provides WiFi, BLE, MQTT, and OTA firmware update simulation stubs
//! for ESP32-based IoT development. All APIs work in simulation mode
//! without actual hardware or networking dependencies.
//!
//! # Module Structure
//!
//! ```text
//! iot/
//! ├── wifi.rs   — ESP32 WiFi station/AP mode, scanning, events
//! ├── ble.rs    — BLE GATT server/client, advertising, scanning
//! ├── mqtt.rs   — MQTT 3.1.1 client with QoS, subscriptions, last will
//! └── ota.rs    — OTA firmware & ML model update with A/B partitions
//! ```
//!
//! # Design
//!
//! These are simulation stubs following the same pattern as
//! [`crate::rtos::freertos`] and [`crate::rtos::zephyr`]. The APIs model
//! real ESP-IDF networking primitives so that Fajar Lang programs can be
//! developed and tested on the host, then deployed to actual hardware
//! when the `esp32` feature links against `esp-idf-sys`.

pub mod ble;
pub mod mqtt;
pub mod ota;
pub mod wifi;

// Re-exports for convenience
pub use ble::{
    BleConfig, BleDevice, BleError, BleEvent, CharProps, GattCharacteristic, GattService,
};
pub use mqtt::{LastWill, MqttClient, MqttConfig, MqttError, MqttMessage, QoS};
pub use ota::{OtaConfig, OtaError, OtaPartition, UpdateInfo, VersionManifest};
pub use wifi::{AccessPoint, AuthMode, IpInfo, WifiConfig, WifiError, WifiEvent, WifiState};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iot_module_re_exports_wifi_types() {
        let config = WifiConfig::new("TestSSID", "password123");
        assert_eq!(config.ssid, "TestSSID");
        assert_eq!(config.auth_mode, AuthMode::WPA2Personal);

        let ip = IpInfo::new([192, 168, 1, 100], [255, 255, 255, 0], [192, 168, 1, 1]);
        assert_eq!(ip.ip_addr, [192, 168, 1, 100]);
    }

    #[test]
    fn iot_module_re_exports_ble_types() {
        let config = BleConfig::new("FajarDevice");
        assert_eq!(config.device_name, "FajarDevice");

        let props = CharProps::READ | CharProps::NOTIFY;
        assert!(props.contains(CharProps::READ));
    }

    #[test]
    fn iot_module_re_exports_mqtt_types() {
        let config = MqttConfig::new("mqtt://localhost", "fj-client-1");
        assert_eq!(config.client_id, "fj-client-1");
        assert_eq!(config.keepalive_secs, 60);
    }

    #[test]
    fn iot_module_re_exports_ota_types() {
        let config = OtaConfig::new("https://ota.example.com/firmware");
        assert!(config.verify_signature);
        assert!(config.rollback_on_failure);
    }
}
