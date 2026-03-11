//! BLE GATT server and client bindings for Fajar Lang.
//!
//! Provides simulation stubs for ESP32 BLE (Bluetooth Low Energy)
//! GATT server/client operations, advertising, and scanning. Models
//! the ESP-IDF BLE API without requiring actual hardware.
//!
//! # API Groups
//!
//! - **Initialization**: configure BLE device name, appearance, advertising
//! - **GATT Server**: register services, characteristics, handle read/write
//! - **Advertising**: start/stop BLE advertisements
//! - **Notifications**: push data to connected clients
//! - **Scanning**: discover nearby BLE peripherals
//!
//! # Usage
//!
//! ```rust
//! use fajar_lang::iot::ble::*;
//!
//! let config = BleConfig::new("FajarSensor");
//! let mut server = BleServer::new(config).unwrap();
//!
//! let service = GattService::new(
//!     [0x00, 0x00, 0x18, 0x0D, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
//!     vec![],
//! );
//! let handle = server.register_service(service).unwrap();
//! ```

use std::sync::atomic::{AtomicU16, Ordering};
use thiserror::Error;

// ═══════════════════════════════════════════════════════════════════════
// Error types
// ═══════════════════════════════════════════════════════════════════════

/// Errors from BLE operations.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum BleError {
    /// BLE stack not initialized.
    #[error("BLE not initialized")]
    NotInitialized,

    /// BLE stack already initialized.
    #[error("BLE already initialized")]
    AlreadyInitialized,

    /// Invalid device name (empty or too long).
    #[error("invalid device name: {reason}")]
    InvalidDeviceName {
        /// Reason the name is invalid.
        reason: String,
    },

    /// Service registration failed.
    #[error("service registration failed: {reason}")]
    ServiceRegistrationFailed {
        /// Reason for failure.
        reason: String,
    },

    /// Invalid service handle.
    #[error("invalid service handle: {handle}")]
    InvalidServiceHandle {
        /// The invalid handle value.
        handle: u16,
    },

    /// Advertising start failed.
    #[error("advertising failed: {reason}")]
    AdvertisingFailed {
        /// Reason for failure.
        reason: String,
    },

    /// Notification send failed.
    #[error("notification failed: {reason}")]
    NotificationFailed {
        /// Reason for failure.
        reason: String,
    },

    /// No active connection for the given connection ID.
    #[error("no active connection: {conn_id}")]
    NoConnection {
        /// The connection ID.
        conn_id: u16,
    },

    /// Scan operation failed.
    #[error("scan failed: {reason}")]
    ScanFailed {
        /// Reason for failure.
        reason: String,
    },

    /// Characteristic not found.
    #[error("characteristic not found: handle {handle}")]
    CharacteristicNotFound {
        /// The characteristic handle.
        handle: u16,
    },
}

// ═══════════════════════════════════════════════════════════════════════
// Constants
// ═══════════════════════════════════════════════════════════════════════

/// Maximum BLE device name length.
pub const MAX_DEVICE_NAME_LEN: usize = 29;

/// Default advertising interval in milliseconds.
pub const DEFAULT_ADV_INTERVAL_MS: u32 = 100;

/// Default BLE appearance (generic sensor = 0x0540).
pub const DEFAULT_APPEARANCE: u16 = 0x0540;

// ═══════════════════════════════════════════════════════════════════════
// Characteristic properties (bitflags)
// ═══════════════════════════════════════════════════════════════════════

/// BLE GATT characteristic property flags.
///
/// Combined with bitwise OR to specify which operations a characteristic supports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CharProps(u8);

impl CharProps {
    /// Characteristic supports read operations.
    pub const READ: CharProps = CharProps(0x02);
    /// Characteristic supports write operations.
    pub const WRITE: CharProps = CharProps(0x08);
    /// Characteristic supports notifications.
    pub const NOTIFY: CharProps = CharProps(0x10);
    /// Characteristic supports write without response.
    pub const WRITE_NR: CharProps = CharProps(0x04);
    /// Characteristic supports indications.
    pub const INDICATE: CharProps = CharProps(0x20);

    /// Creates empty (no properties) flags.
    pub fn empty() -> Self {
        CharProps(0)
    }

    /// Returns the raw bits.
    pub fn bits(self) -> u8 {
        self.0
    }

    /// Returns whether the given flag is set.
    pub fn contains(self, other: CharProps) -> bool {
        (self.0 & other.0) == other.0
    }
}

impl std::ops::BitOr for CharProps {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        CharProps(self.0 | rhs.0)
    }
}

impl std::ops::BitOrAssign for CharProps {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl std::ops::BitAnd for CharProps {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        CharProps(self.0 & rhs.0)
    }
}

impl std::fmt::Display for CharProps {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut parts = Vec::new();
        if self.contains(CharProps::READ) {
            parts.push("READ");
        }
        if self.contains(CharProps::WRITE_NR) {
            parts.push("WRITE_NR");
        }
        if self.contains(CharProps::WRITE) {
            parts.push("WRITE");
        }
        if self.contains(CharProps::NOTIFY) {
            parts.push("NOTIFY");
        }
        if self.contains(CharProps::INDICATE) {
            parts.push("INDICATE");
        }
        write!(f, "{}", parts.join("|"))
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Data types
// ═══════════════════════════════════════════════════════════════════════

/// BLE configuration.
#[derive(Debug, Clone)]
pub struct BleConfig {
    /// Device name advertised over BLE.
    pub device_name: String,
    /// GAP appearance value.
    pub appearance: u16,
    /// Advertising interval in milliseconds.
    pub adv_interval_ms: u32,
}

impl BleConfig {
    /// Creates a new BLE configuration with default settings.
    pub fn new(device_name: &str) -> Self {
        Self {
            device_name: device_name.to_string(),
            appearance: DEFAULT_APPEARANCE,
            adv_interval_ms: DEFAULT_ADV_INTERVAL_MS,
        }
    }

    /// Sets the GAP appearance value.
    pub fn with_appearance(mut self, appearance: u16) -> Self {
        self.appearance = appearance;
        self
    }

    /// Sets the advertising interval.
    pub fn with_adv_interval(mut self, interval_ms: u32) -> Self {
        self.adv_interval_ms = interval_ms;
        self
    }

    /// Validates the configuration.
    pub fn validate(&self) -> Result<(), BleError> {
        if self.device_name.is_empty() {
            return Err(BleError::InvalidDeviceName {
                reason: "device name cannot be empty".to_string(),
            });
        }
        if self.device_name.len() > MAX_DEVICE_NAME_LEN {
            return Err(BleError::InvalidDeviceName {
                reason: format!(
                    "device name exceeds {MAX_DEVICE_NAME_LEN} bytes (got {})",
                    self.device_name.len()
                ),
            });
        }
        Ok(())
    }
}

/// A GATT characteristic within a service.
#[derive(Debug, Clone)]
pub struct GattCharacteristic {
    /// 128-bit UUID.
    pub uuid: [u8; 16],
    /// Property flags (READ, WRITE, NOTIFY, etc.).
    pub properties: CharProps,
    /// Permission byte (application-defined).
    pub permissions: u8,
    /// Current value.
    pub value: Vec<u8>,
    /// Assigned handle (set during registration).
    pub handle: u16,
}

impl GattCharacteristic {
    /// Creates a new GATT characteristic.
    pub fn new(uuid: [u8; 16], properties: CharProps, permissions: u8) -> Self {
        Self {
            uuid,
            properties,
            permissions,
            value: Vec::new(),
            handle: 0,
        }
    }

    /// Sets the initial value of the characteristic.
    pub fn with_value(mut self, value: Vec<u8>) -> Self {
        self.value = value;
        self
    }
}

/// A GATT service containing one or more characteristics.
#[derive(Debug, Clone)]
pub struct GattService {
    /// 128-bit service UUID.
    pub uuid: [u8; 16],
    /// Characteristics in this service.
    pub characteristics: Vec<GattCharacteristic>,
    /// Assigned handle (set during registration).
    pub handle: u16,
}

impl GattService {
    /// Creates a new GATT service.
    pub fn new(uuid: [u8; 16], characteristics: Vec<GattCharacteristic>) -> Self {
        Self {
            uuid,
            characteristics,
            handle: 0,
        }
    }

    /// Adds a characteristic to the service.
    pub fn add_characteristic(mut self, char: GattCharacteristic) -> Self {
        self.characteristics.push(char);
        self
    }
}

/// BLE event emitted by the stack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BleEvent {
    /// A client connected. Payload is connection ID.
    Connected(u16),
    /// A client disconnected. Payload is connection ID.
    Disconnected(u16),
    /// A client wrote to a characteristic. Handle + data.
    WriteRequest(u16, Vec<u8>),
    /// A client read from a characteristic. Handle.
    ReadRequest(u16),
}

/// A discovered BLE peripheral from scanning.
#[derive(Debug, Clone)]
pub struct BleDevice {
    /// Device name (may be absent).
    pub name: Option<String>,
    /// 6-byte MAC address.
    pub addr: [u8; 6],
    /// Signal strength (dBm).
    pub rssi: i8,
}

impl BleDevice {
    /// Creates a new BLE device entry.
    pub fn new(name: Option<String>, addr: [u8; 6], rssi: i8) -> Self {
        Self { name, addr, rssi }
    }

    /// Returns a colon-separated MAC address string.
    pub fn addr_string(&self) -> String {
        format!(
            "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
            self.addr[0], self.addr[1], self.addr[2], self.addr[3], self.addr[4], self.addr[5]
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Handle allocator
// ═══════════════════════════════════════════════════════════════════════

/// Global handle counter for simulation.
static NEXT_HANDLE: AtomicU16 = AtomicU16::new(1);

/// Allocates the next unique handle.
fn alloc_handle() -> u16 {
    NEXT_HANDLE.fetch_add(1, Ordering::Relaxed)
}

// ═══════════════════════════════════════════════════════════════════════
// BLE server (simulation)
// ═══════════════════════════════════════════════════════════════════════

/// Simulated BLE GATT server.
///
/// Models the ESP-IDF BLE GATT server lifecycle:
/// `new()` -> `register_service()` -> `start_advertising()` -> `notify()`.
#[derive(Debug)]
pub struct BleServer {
    /// Server configuration.
    config: BleConfig,
    /// Whether the server is initialized.
    initialized: bool,
    /// Registered services.
    services: Vec<GattService>,
    /// Whether advertising is active.
    advertising: bool,
    /// Simulated active connections.
    connections: Vec<u16>,
    /// Event log for testing.
    events: Vec<BleEvent>,
}

impl BleServer {
    /// Creates and initializes a new BLE server (simulation).
    pub fn new(config: BleConfig) -> Result<Self, BleError> {
        config.validate()?;
        Ok(Self {
            config,
            initialized: true,
            services: Vec::new(),
            advertising: false,
            connections: Vec::new(),
            events: Vec::new(),
        })
    }

    /// Returns the server configuration.
    pub fn config(&self) -> &BleConfig {
        &self.config
    }

    /// Returns whether the server is initialized.
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Returns whether advertising is active.
    pub fn is_advertising(&self) -> bool {
        self.advertising
    }

    /// Returns the registered services.
    pub fn services(&self) -> &[GattService] {
        &self.services
    }

    /// Returns the event log.
    pub fn events(&self) -> &[BleEvent] {
        &self.events
    }

    /// Returns the number of active connections.
    pub fn connection_count(&self) -> usize {
        self.connections.len()
    }

    /// Registers a GATT service and assigns handles.
    ///
    /// Returns the service handle.
    pub fn register_service(&mut self, mut service: GattService) -> Result<u16, BleError> {
        if !self.initialized {
            return Err(BleError::NotInitialized);
        }
        let svc_handle = alloc_handle();
        service.handle = svc_handle;

        // Assign handles to each characteristic
        for char in &mut service.characteristics {
            char.handle = alloc_handle();
        }

        self.services.push(service);
        Ok(svc_handle)
    }

    /// Starts BLE advertising (simulation).
    pub fn start_advertising(&mut self, _service_uuids: &[[u8; 16]]) -> Result<(), BleError> {
        if !self.initialized {
            return Err(BleError::NotInitialized);
        }
        self.advertising = true;
        Ok(())
    }

    /// Stops BLE advertising.
    pub fn stop_advertising(&mut self) {
        self.advertising = false;
    }

    /// Sends a notification to a connected client (simulation).
    pub fn notify(&mut self, char_handle: u16, conn_id: u16, data: &[u8]) -> Result<(), BleError> {
        if !self.initialized {
            return Err(BleError::NotInitialized);
        }
        if !self.connections.contains(&conn_id) {
            return Err(BleError::NoConnection { conn_id });
        }
        // Verify the characteristic exists and supports NOTIFY
        let char_found = self
            .services
            .iter()
            .flat_map(|s| &s.characteristics)
            .any(|c| c.handle == char_handle && c.properties.contains(CharProps::NOTIFY));

        if !char_found {
            return Err(BleError::CharacteristicNotFound {
                handle: char_handle,
            });
        }

        // Log the notification event (simulation)
        let _ = data; // In simulation, we accept but don't transmit
        Ok(())
    }

    /// Simulates a client connection (for testing).
    pub fn simulate_connect(&mut self, conn_id: u16) {
        self.connections.push(conn_id);
        self.events.push(BleEvent::Connected(conn_id));
    }

    /// Simulates a client disconnection (for testing).
    pub fn simulate_disconnect(&mut self, conn_id: u16) {
        self.connections.retain(|&c| c != conn_id);
        self.events.push(BleEvent::Disconnected(conn_id));
    }

    /// Simulates a write request from a client (for testing).
    pub fn simulate_write(&mut self, char_handle: u16, data: Vec<u8>) {
        // Update the characteristic value
        for service in &mut self.services {
            for char in &mut service.characteristics {
                if char.handle == char_handle {
                    char.value = data.clone();
                    break;
                }
            }
        }
        self.events.push(BleEvent::WriteRequest(char_handle, data));
    }

    /// Simulates a read request from a client (for testing).
    pub fn simulate_read(&mut self, char_handle: u16) {
        self.events.push(BleEvent::ReadRequest(char_handle));
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Free-standing convenience functions
// ═══════════════════════════════════════════════════════════════════════

/// Initializes the BLE stack (simulation stub).
pub fn ble_init(config: BleConfig) -> Result<BleServer, BleError> {
    BleServer::new(config)
}

/// Registers a GATT service. Returns the assigned service handle.
pub fn ble_register_service(server: &mut BleServer, service: GattService) -> Result<u16, BleError> {
    server.register_service(service)
}

/// Starts BLE advertising with the given service UUIDs.
pub fn ble_start_advertising(
    server: &mut BleServer,
    service_uuids: &[[u8; 16]],
) -> Result<(), BleError> {
    server.start_advertising(service_uuids)
}

/// Sends a BLE notification to a connected client.
pub fn ble_notify(
    server: &mut BleServer,
    char_handle: u16,
    conn_id: u16,
    data: &[u8],
) -> Result<(), BleError> {
    server.notify(char_handle, conn_id, data)
}

/// Scans for nearby BLE peripherals (simulation stub).
///
/// The `duration_ms` parameter would control scan duration on real hardware.
pub fn ble_scan(_duration_ms: u32) -> Result<Vec<BleDevice>, BleError> {
    // Simulation: return a few fake devices
    Ok(vec![
        BleDevice::new(
            Some("FajarSensor-1".to_string()),
            [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0x01],
            -55,
        ),
        BleDevice::new(None, [0x11, 0x22, 0x33, 0x44, 0x55, 0x66], -72),
        BleDevice::new(
            Some("SmartLock".to_string()),
            [0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0x42],
            -80,
        ),
    ])
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn test_uuid(id: u8) -> [u8; 16] {
        let mut uuid = [0u8; 16];
        uuid[2] = id;
        uuid
    }

    #[test]
    fn ble_init_succeeds() {
        let config = BleConfig::new("TestDevice");
        let server = ble_init(config).unwrap();
        assert!(server.is_initialized());
        assert!(!server.is_advertising());
        assert_eq!(server.services().len(), 0);
    }

    #[test]
    fn ble_init_empty_name_fails() {
        let config = BleConfig::new("");
        let err = ble_init(config).unwrap_err();
        assert!(matches!(err, BleError::InvalidDeviceName { .. }));
    }

    #[test]
    fn ble_register_service_assigns_handles() {
        let config = BleConfig::new("TestDevice");
        let mut server = ble_init(config).unwrap();

        let char1 =
            GattCharacteristic::new(test_uuid(1), CharProps::READ | CharProps::NOTIFY, 0x01);
        let char2 = GattCharacteristic::new(test_uuid(2), CharProps::WRITE, 0x02);
        let service = GattService::new(test_uuid(0x18), vec![char1, char2]);

        let handle = server.register_service(service).unwrap();
        assert!(handle > 0);

        let svc = &server.services()[0];
        assert_eq!(svc.handle, handle);
        assert!(svc.characteristics[0].handle > 0);
        assert!(svc.characteristics[1].handle > 0);
        assert_ne!(svc.characteristics[0].handle, svc.characteristics[1].handle);
    }

    #[test]
    fn ble_start_advertising_succeeds() {
        let config = BleConfig::new("TestDevice");
        let mut server = ble_init(config).unwrap();

        let service = GattService::new(test_uuid(0x18), vec![]);
        server.register_service(service).unwrap();

        server.start_advertising(&[test_uuid(0x18)]).unwrap();
        assert!(server.is_advertising());

        server.stop_advertising();
        assert!(!server.is_advertising());
    }

    #[test]
    fn ble_notify_succeeds() {
        let config = BleConfig::new("TestDevice");
        let mut server = ble_init(config).unwrap();

        let char1 =
            GattCharacteristic::new(test_uuid(1), CharProps::READ | CharProps::NOTIFY, 0x01);
        let service = GattService::new(test_uuid(0x18), vec![char1]);
        server.register_service(service).unwrap();

        let char_handle = server.services()[0].characteristics[0].handle;

        // Simulate a connection
        server.simulate_connect(1);
        assert_eq!(server.connection_count(), 1);

        // Send notification
        server.notify(char_handle, 1, &[0x42, 0x43]).unwrap();
    }

    #[test]
    fn ble_notify_no_connection_fails() {
        let config = BleConfig::new("TestDevice");
        let mut server = ble_init(config).unwrap();

        let char1 = GattCharacteristic::new(test_uuid(1), CharProps::NOTIFY, 0x01);
        let service = GattService::new(test_uuid(0x18), vec![char1]);
        server.register_service(service).unwrap();

        let char_handle = server.services()[0].characteristics[0].handle;

        let err = server.notify(char_handle, 99, &[0x42]).unwrap_err();
        assert!(matches!(err, BleError::NoConnection { conn_id: 99 }));
    }

    #[test]
    fn ble_events_recorded() {
        let config = BleConfig::new("TestDevice");
        let mut server = ble_init(config).unwrap();

        server.simulate_connect(1);
        server.simulate_disconnect(1);

        let events = server.events();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0], BleEvent::Connected(1));
        assert_eq!(events[1], BleEvent::Disconnected(1));
    }

    #[test]
    fn ble_simulate_write_updates_value() {
        let config = BleConfig::new("TestDevice");
        let mut server = ble_init(config).unwrap();

        let char1 = GattCharacteristic::new(test_uuid(1), CharProps::READ | CharProps::WRITE, 0x03)
            .with_value(vec![0x00]);
        let service = GattService::new(test_uuid(0x18), vec![char1]);
        server.register_service(service).unwrap();

        let char_handle = server.services()[0].characteristics[0].handle;
        server.simulate_write(char_handle, vec![0xFF, 0xAB]);

        assert_eq!(
            server.services()[0].characteristics[0].value,
            vec![0xFF, 0xAB]
        );
        assert!(matches!(
            server.events().last(),
            Some(BleEvent::WriteRequest(_, _))
        ));
    }

    #[test]
    fn ble_scan_returns_devices() {
        let devices = ble_scan(3000).unwrap();
        assert_eq!(devices.len(), 3);
        assert_eq!(devices[0].name, Some("FajarSensor-1".to_string()));
        assert_eq!(devices[0].addr_string(), "AA:BB:CC:DD:EE:01");
        assert!(devices[1].name.is_none());
    }

    #[test]
    fn ble_char_props_bitflags() {
        let props = CharProps::READ | CharProps::WRITE | CharProps::NOTIFY;
        assert!(props.contains(CharProps::READ));
        assert!(props.contains(CharProps::WRITE));
        assert!(props.contains(CharProps::NOTIFY));
        assert!(!props.contains(CharProps::INDICATE));
        assert_eq!(props.bits(), 0x02 | 0x08 | 0x10);

        let empty = CharProps::empty();
        assert!(!empty.contains(CharProps::READ));
        assert_eq!(empty.bits(), 0);
    }
}
