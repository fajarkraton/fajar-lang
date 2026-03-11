//! ESP32 WiFi FFI bindings for Fajar Lang.
//!
//! Provides simulation stubs for ESP32 WiFi station mode, access point mode,
//! scanning, and event handling. Models the ESP-IDF WiFi driver API without
//! requiring actual hardware or `esp-idf-sys` dependencies.
//!
//! # API Groups
//!
//! - **Station mode**: connect to an existing access point
//! - **AP mode**: create a software access point
//! - **Scanning**: discover nearby WiFi networks
//! - **Events**: connection state change notifications
//! - **Teardown**: disconnect and stop WiFi driver
//!
//! # Usage
//!
//! ```rust
//! use fajar_lang::iot::wifi::*;
//!
//! let mut state = WifiState::new();
//! state.init().unwrap();
//! let ip = state.connect_sta("MyNetwork", "password").unwrap();
//! assert_eq!(ip.ip_addr, [192, 168, 4, 2]);
//! ```

use thiserror::Error;

// ═══════════════════════════════════════════════════════════════════════
// Error types
// ═══════════════════════════════════════════════════════════════════════

/// Errors from ESP32 WiFi operations.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum WifiError {
    /// WiFi driver not initialized.
    #[error("wifi not initialized: call wifi_init() first")]
    NotInitialized,

    /// WiFi driver already initialized.
    #[error("wifi already initialized")]
    AlreadyInitialized,

    /// WiFi already connected in station mode.
    #[error("wifi already connected to '{ssid}'")]
    AlreadyConnected {
        /// Current SSID.
        ssid: String,
    },

    /// WiFi not connected (cannot disconnect).
    #[error("wifi not connected")]
    NotConnected,

    /// Invalid SSID (empty or exceeds 32 bytes).
    #[error("invalid SSID: {reason}")]
    InvalidSsid {
        /// Reason the SSID is invalid.
        reason: String,
    },

    /// Invalid password (exceeds 64 bytes or too short for WPA).
    #[error("invalid password: {reason}")]
    InvalidPassword {
        /// Reason the password is invalid.
        reason: String,
    },

    /// Connection to AP failed.
    #[error("connection failed: {reason}")]
    ConnectionFailed {
        /// Reason for failure.
        reason: String,
    },

    /// AP mode start failed.
    #[error("AP mode failed: {reason}")]
    ApStartFailed {
        /// Reason for failure.
        reason: String,
    },

    /// Scan operation failed.
    #[error("scan failed: {reason}")]
    ScanFailed {
        /// Reason for failure.
        reason: String,
    },

    /// Invalid channel number (must be 1-14).
    #[error("invalid channel: {channel} (valid: 1-14)")]
    InvalidChannel {
        /// The invalid channel number.
        channel: u8,
    },

    /// Internal driver error.
    #[error("driver error: {reason}")]
    DriverError {
        /// Reason for failure.
        reason: String,
    },
}

// ═══════════════════════════════════════════════════════════════════════
// Constants
// ═══════════════════════════════════════════════════════════════════════

/// Maximum SSID length in bytes.
pub const MAX_SSID_LEN: usize = 32;

/// Maximum password length in bytes.
pub const MAX_PASSWORD_LEN: usize = 64;

/// Minimum WPA password length.
pub const MIN_WPA_PASSWORD_LEN: usize = 8;

/// Maximum number of AP mode connections.
pub const MAX_AP_CONNECTIONS: u8 = 10;

/// Default AP channel.
pub const DEFAULT_AP_CHANNEL: u8 = 1;

// ═══════════════════════════════════════════════════════════════════════
// Data types
// ═══════════════════════════════════════════════════════════════════════

/// WiFi authentication mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthMode {
    /// Open network (no authentication).
    Open,
    /// WPA2 Personal (PSK).
    WPA2Personal,
    /// WPA3 Personal (SAE).
    WPA3Personal,
}

impl std::fmt::Display for AuthMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthMode::Open => write!(f, "Open"),
            AuthMode::WPA2Personal => write!(f, "WPA2-PSK"),
            AuthMode::WPA3Personal => write!(f, "WPA3-SAE"),
        }
    }
}

/// WiFi configuration for station or AP mode.
#[derive(Debug, Clone)]
pub struct WifiConfig {
    /// Network SSID (max 32 bytes).
    pub ssid: String,
    /// Network password (max 64 bytes).
    pub password: String,
    /// Authentication mode.
    pub auth_mode: AuthMode,
    /// WiFi channel (1-14).
    pub channel: u8,
    /// Maximum connections (AP mode only, 1-10).
    pub max_connections: u8,
}

impl WifiConfig {
    /// Creates a new WiFi configuration with WPA2 defaults.
    pub fn new(ssid: &str, password: &str) -> Self {
        Self {
            ssid: ssid.to_string(),
            password: password.to_string(),
            auth_mode: if password.is_empty() {
                AuthMode::Open
            } else {
                AuthMode::WPA2Personal
            },
            channel: DEFAULT_AP_CHANNEL,
            max_connections: 4,
        }
    }

    /// Sets the authentication mode.
    pub fn with_auth_mode(mut self, mode: AuthMode) -> Self {
        self.auth_mode = mode;
        self
    }

    /// Sets the WiFi channel.
    pub fn with_channel(mut self, channel: u8) -> Self {
        self.channel = channel;
        self
    }

    /// Sets the maximum connections for AP mode.
    pub fn with_max_connections(mut self, max: u8) -> Self {
        self.max_connections = max;
        self
    }

    /// Validates the configuration.
    pub fn validate(&self) -> Result<(), WifiError> {
        validate_ssid(&self.ssid)?;
        validate_password(&self.password, self.auth_mode)?;
        validate_channel(self.channel)?;
        if self.max_connections == 0 || self.max_connections > MAX_AP_CONNECTIONS {
            return Err(WifiError::ApStartFailed {
                reason: format!(
                    "max_connections must be 1-{MAX_AP_CONNECTIONS}, got {}",
                    self.max_connections
                ),
            });
        }
        Ok(())
    }
}

/// IP address information returned after successful connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IpInfo {
    /// IPv4 address as 4 octets.
    pub ip_addr: [u8; 4],
    /// Subnet mask as 4 octets.
    pub netmask: [u8; 4],
    /// Default gateway as 4 octets.
    pub gateway: [u8; 4],
}

impl IpInfo {
    /// Creates a new IP info struct.
    pub fn new(ip_addr: [u8; 4], netmask: [u8; 4], gateway: [u8; 4]) -> Self {
        Self {
            ip_addr,
            netmask,
            gateway,
        }
    }

    /// Returns a dotted-decimal string representation of the IP address.
    pub fn ip_string(&self) -> String {
        format!(
            "{}.{}.{}.{}",
            self.ip_addr[0], self.ip_addr[1], self.ip_addr[2], self.ip_addr[3]
        )
    }
}

impl std::fmt::Display for IpInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "ip={} netmask={}.{}.{}.{} gw={}.{}.{}.{}",
            self.ip_string(),
            self.netmask[0],
            self.netmask[1],
            self.netmask[2],
            self.netmask[3],
            self.gateway[0],
            self.gateway[1],
            self.gateway[2],
            self.gateway[3],
        )
    }
}

/// A discovered WiFi access point from a scan.
#[derive(Debug, Clone)]
pub struct AccessPoint {
    /// Network SSID.
    pub ssid: String,
    /// Received signal strength indicator (dBm).
    pub rssi: i8,
    /// WiFi channel.
    pub channel: u8,
    /// Authentication mode.
    pub auth_mode: AuthMode,
}

impl AccessPoint {
    /// Creates a new access point entry.
    pub fn new(ssid: &str, rssi: i8, channel: u8, auth_mode: AuthMode) -> Self {
        Self {
            ssid: ssid.to_string(),
            rssi,
            channel,
            auth_mode,
        }
    }
}

/// WiFi event emitted by the driver.
#[derive(Debug, Clone, PartialEq)]
pub enum WifiEvent {
    /// Successfully connected to AP.
    Connected,
    /// Disconnected from AP.
    Disconnected,
    /// Obtained IP address via DHCP.
    GotIp(IpInfo),
    /// Lost IP address.
    LostIp,
}

// ═══════════════════════════════════════════════════════════════════════
// Validation helpers
// ═══════════════════════════════════════════════════════════════════════

/// Validates an SSID string.
fn validate_ssid(ssid: &str) -> Result<(), WifiError> {
    if ssid.is_empty() {
        return Err(WifiError::InvalidSsid {
            reason: "SSID cannot be empty".to_string(),
        });
    }
    if ssid.len() > MAX_SSID_LEN {
        return Err(WifiError::InvalidSsid {
            reason: format!("SSID exceeds {MAX_SSID_LEN} bytes (got {})", ssid.len()),
        });
    }
    Ok(())
}

/// Validates a password for the given auth mode.
fn validate_password(password: &str, auth_mode: AuthMode) -> Result<(), WifiError> {
    if password.len() > MAX_PASSWORD_LEN {
        return Err(WifiError::InvalidPassword {
            reason: format!(
                "password exceeds {MAX_PASSWORD_LEN} bytes (got {})",
                password.len()
            ),
        });
    }
    match auth_mode {
        AuthMode::Open => {}
        AuthMode::WPA2Personal | AuthMode::WPA3Personal => {
            if password.len() < MIN_WPA_PASSWORD_LEN {
                return Err(WifiError::InvalidPassword {
                    reason: format!(
                        "WPA password must be at least {MIN_WPA_PASSWORD_LEN} characters (got {})",
                        password.len()
                    ),
                });
            }
        }
    }
    Ok(())
}

/// Validates a WiFi channel number.
fn validate_channel(channel: u8) -> Result<(), WifiError> {
    if channel == 0 || channel > 14 {
        return Err(WifiError::InvalidChannel { channel });
    }
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// WiFi state machine (simulation)
// ═══════════════════════════════════════════════════════════════════════

/// Internal driver mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WifiMode {
    /// Not connected.
    Idle,
    /// Station mode (connected to AP).
    Station,
    /// Access point mode (hosting AP).
    Ap,
}

/// Simulated WiFi driver state.
///
/// Models the ESP-IDF WiFi driver lifecycle:
/// `new()` -> `init()` -> `connect_sta()` or `start_ap()` -> `disconnect()` -> `stop()`.
#[derive(Debug)]
pub struct WifiState {
    /// Whether the driver has been initialized.
    initialized: bool,
    /// Current operating mode.
    mode: WifiMode,
    /// SSID of the connected/hosted network.
    connected_ssid: Option<String>,
    /// Assigned IP information (station mode).
    ip_info: Option<IpInfo>,
    /// Event log for testing.
    events: Vec<WifiEvent>,
}

impl WifiState {
    /// Creates a new uninitialized WiFi state.
    pub fn new() -> Self {
        Self {
            initialized: false,
            mode: WifiMode::Idle,
            connected_ssid: None,
            ip_info: None,
            events: Vec::new(),
        }
    }

    /// Initializes the WiFi driver (simulation).
    ///
    /// In real ESP-IDF this initializes NVS, netif, event loop, and WiFi driver.
    pub fn init(&mut self) -> Result<(), WifiError> {
        if self.initialized {
            return Err(WifiError::AlreadyInitialized);
        }
        self.initialized = true;
        Ok(())
    }

    /// Returns whether the driver is initialized.
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Returns whether the driver is connected (station mode).
    pub fn is_connected(&self) -> bool {
        self.mode == WifiMode::Station && self.connected_ssid.is_some()
    }

    /// Returns whether the driver is in AP mode.
    pub fn is_ap_mode(&self) -> bool {
        self.mode == WifiMode::Ap
    }

    /// Returns the current IP info, if connected.
    pub fn ip_info(&self) -> Option<&IpInfo> {
        self.ip_info.as_ref()
    }

    /// Returns the connected SSID, if any.
    pub fn connected_ssid(&self) -> Option<&str> {
        self.connected_ssid.as_deref()
    }

    /// Returns the event log.
    pub fn events(&self) -> &[WifiEvent] {
        &self.events
    }

    /// Connects to a WiFi access point in station mode (simulation).
    ///
    /// Returns simulated IP information on success.
    pub fn connect_sta(&mut self, ssid: &str, password: &str) -> Result<IpInfo, WifiError> {
        if !self.initialized {
            return Err(WifiError::NotInitialized);
        }
        if let Some(ref current) = self.connected_ssid {
            return Err(WifiError::AlreadyConnected {
                ssid: current.clone(),
            });
        }
        validate_ssid(ssid)?;
        // Determine auth mode from password
        let auth_mode = if password.is_empty() {
            AuthMode::Open
        } else {
            AuthMode::WPA2Personal
        };
        validate_password(password, auth_mode)?;

        // Simulation: assign a deterministic IP
        let ip = IpInfo::new([192, 168, 4, 2], [255, 255, 255, 0], [192, 168, 4, 1]);

        self.mode = WifiMode::Station;
        self.connected_ssid = Some(ssid.to_string());
        self.ip_info = Some(ip);
        self.events.push(WifiEvent::Connected);
        self.events.push(WifiEvent::GotIp(ip));

        Ok(ip)
    }

    /// Starts a WiFi access point (simulation).
    pub fn start_ap(&mut self, ssid: &str, password: &str, channel: u8) -> Result<(), WifiError> {
        if !self.initialized {
            return Err(WifiError::NotInitialized);
        }
        validate_ssid(ssid)?;
        let auth_mode = if password.is_empty() {
            AuthMode::Open
        } else {
            AuthMode::WPA2Personal
        };
        validate_password(password, auth_mode)?;
        validate_channel(channel)?;

        self.mode = WifiMode::Ap;
        self.connected_ssid = Some(ssid.to_string());
        // AP gets its own IP
        self.ip_info = Some(IpInfo::new(
            [192, 168, 4, 1],
            [255, 255, 255, 0],
            [192, 168, 4, 1],
        ));

        Ok(())
    }

    /// Scans for nearby WiFi networks (simulation).
    ///
    /// Returns a list of simulated access points.
    pub fn scan(&self) -> Result<Vec<AccessPoint>, WifiError> {
        if !self.initialized {
            return Err(WifiError::NotInitialized);
        }
        // Simulation: return a few fake APs
        Ok(vec![
            AccessPoint::new("HomeNetwork", -45, 6, AuthMode::WPA2Personal),
            AccessPoint::new("Office5G", -62, 36, AuthMode::WPA3Personal),
            AccessPoint::new("CafeWiFi", -78, 1, AuthMode::Open),
        ])
    }

    /// Disconnects from the current network.
    pub fn disconnect(&mut self) -> Result<(), WifiError> {
        if !self.initialized {
            return Err(WifiError::NotInitialized);
        }
        if self.mode == WifiMode::Idle {
            return Err(WifiError::NotConnected);
        }
        self.events.push(WifiEvent::Disconnected);
        if self.ip_info.is_some() {
            self.events.push(WifiEvent::LostIp);
        }
        self.mode = WifiMode::Idle;
        self.connected_ssid = None;
        self.ip_info = None;

        Ok(())
    }

    /// Stops the WiFi driver and releases resources.
    pub fn stop(&mut self) -> Result<(), WifiError> {
        if !self.initialized {
            return Err(WifiError::NotInitialized);
        }
        // Disconnect first if needed
        if self.mode != WifiMode::Idle {
            self.disconnect()?;
        }
        self.initialized = false;
        Ok(())
    }
}

impl Default for WifiState {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Free-standing convenience functions (mirror ESP-IDF API)
// ═══════════════════════════════════════════════════════════════════════

/// Initializes the WiFi subsystem (simulation stub).
///
/// In real ESP-IDF, this calls `nvs_flash_init()`, `esp_netif_init()`,
/// `esp_event_loop_create_default()`, and `esp_wifi_init()`.
pub fn wifi_init() -> Result<WifiState, WifiError> {
    let mut state = WifiState::new();
    state.init()?;
    Ok(state)
}

/// Connects to a WiFi network in station mode (simulation stub).
pub fn wifi_connect_sta(
    state: &mut WifiState,
    ssid: &str,
    password: &str,
) -> Result<IpInfo, WifiError> {
    state.connect_sta(ssid, password)
}

/// Starts a WiFi access point (simulation stub).
pub fn wifi_start_ap(
    state: &mut WifiState,
    ssid: &str,
    password: &str,
    channel: u8,
) -> Result<(), WifiError> {
    state.start_ap(ssid, password, channel)
}

/// Scans for nearby WiFi networks (simulation stub).
pub fn wifi_scan(state: &WifiState) -> Result<Vec<AccessPoint>, WifiError> {
    state.scan()
}

/// Disconnects from the current WiFi network.
pub fn wifi_disconnect(state: &mut WifiState) -> Result<(), WifiError> {
    state.disconnect()
}

/// Stops the WiFi driver and releases all resources.
pub fn wifi_stop(state: &mut WifiState) -> Result<(), WifiError> {
    state.stop()
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wifi_init_succeeds() {
        let state = wifi_init().unwrap();
        assert!(state.is_initialized());
        assert!(!state.is_connected());
        assert!(!state.is_ap_mode());
    }

    #[test]
    fn wifi_double_init_fails() {
        let mut state = WifiState::new();
        state.init().unwrap();
        let err = state.init().unwrap_err();
        assert_eq!(err, WifiError::AlreadyInitialized);
    }

    #[test]
    fn wifi_connect_sta_succeeds() {
        let mut state = wifi_init().unwrap();
        let ip = wifi_connect_sta(&mut state, "TestNet", "password123").unwrap();
        assert_eq!(ip.ip_addr, [192, 168, 4, 2]);
        assert_eq!(ip.netmask, [255, 255, 255, 0]);
        assert_eq!(ip.gateway, [192, 168, 4, 1]);
        assert!(state.is_connected());
        assert_eq!(state.connected_ssid(), Some("TestNet"));
    }

    #[test]
    fn wifi_connect_without_init_fails() {
        let mut state = WifiState::new();
        let err = state.connect_sta("Test", "password123").unwrap_err();
        assert_eq!(err, WifiError::NotInitialized);
    }

    #[test]
    fn wifi_connect_invalid_ssid() {
        let mut state = wifi_init().unwrap();
        let err = state.connect_sta("", "password123").unwrap_err();
        assert!(matches!(err, WifiError::InvalidSsid { .. }));

        let long_ssid = "a".repeat(33);
        let err = state.connect_sta(&long_ssid, "password123").unwrap_err();
        assert!(matches!(err, WifiError::InvalidSsid { .. }));
    }

    #[test]
    fn wifi_connect_invalid_password() {
        let mut state = wifi_init().unwrap();
        // WPA2 requires >= 8 chars
        let err = state.connect_sta("TestNet", "short").unwrap_err();
        assert!(matches!(err, WifiError::InvalidPassword { .. }));
    }

    #[test]
    fn wifi_start_ap_succeeds() {
        let mut state = wifi_init().unwrap();
        wifi_start_ap(&mut state, "FajarAP", "securepass", 6).unwrap();
        assert!(state.is_ap_mode());
        assert_eq!(state.connected_ssid(), Some("FajarAP"));
    }

    #[test]
    fn wifi_scan_returns_simulated_aps() {
        let state = wifi_init().unwrap();
        let aps = wifi_scan(&state).unwrap();
        assert_eq!(aps.len(), 3);
        assert_eq!(aps[0].ssid, "HomeNetwork");
        assert_eq!(aps[0].rssi, -45);
        assert_eq!(aps[1].auth_mode, AuthMode::WPA3Personal);
        assert_eq!(aps[2].auth_mode, AuthMode::Open);
    }

    #[test]
    fn wifi_disconnect_succeeds() {
        let mut state = wifi_init().unwrap();
        state.connect_sta("TestNet", "password123").unwrap();
        assert!(state.is_connected());

        wifi_disconnect(&mut state).unwrap();
        assert!(!state.is_connected());
        assert!(state.connected_ssid().is_none());
    }

    #[test]
    fn wifi_events_recorded() {
        let mut state = wifi_init().unwrap();
        state.connect_sta("TestNet", "password123").unwrap();
        state.disconnect().unwrap();

        let events = state.events();
        assert_eq!(events.len(), 4);
        assert_eq!(events[0], WifiEvent::Connected);
        assert!(matches!(events[1], WifiEvent::GotIp(_)));
        assert_eq!(events[2], WifiEvent::Disconnected);
        assert_eq!(events[3], WifiEvent::LostIp);
    }

    #[test]
    fn wifi_config_validation() {
        let config = WifiConfig::new("GoodSSID", "goodpassword");
        assert!(config.validate().is_ok());

        let bad = WifiConfig::new("", "goodpassword");
        assert!(bad.validate().is_err());

        let bad_channel = WifiConfig::new("Test", "password123").with_channel(0);
        assert!(bad_channel.validate().is_err());

        let bad_channel = WifiConfig::new("Test", "password123").with_channel(15);
        assert!(bad_channel.validate().is_err());
    }
}
