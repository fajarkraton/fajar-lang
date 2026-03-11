//! LoRaWAN protocol simulation for Fajar Lang.
//!
//! Implements LoRaWAN 1.0.4 MAC layer with Class A/B/C modes,
//! OTAA join, uplink/downlink, ADR, and regional frequency plans.
//! All APIs work in simulation mode without actual hardware or
//! LoRa radio dependencies.
//!
//! # API Groups
//!
//! - **Configuration**: device EUI, app key, frequency plan
//! - **Join**: OTAA join procedure (simulated)
//! - **Class A**: uplink with RX1/RX2 windows
//! - **Class B**: beacon-synchronized receive slots
//! - **Class C**: continuous receive window
//! - **ADR**: adaptive data rate optimization
//! - **MAC commands**: link check, duty cycle, RX param setup
//! - **Multicast**: group messaging support
//! - **IotStack**: unified WiFi + BLE + MQTT + LoRaWAN interface
//!
//! # Usage
//!
//! ```rust
//! use fajar_lang::iot::lorawan::*;
//!
//! let config = LoRaConfig::new(FrequencyPlan::EU868);
//! let mut device = LoRaDevice::new(config);
//! let accept = device.join_otaa().unwrap();
//! device.send_uplink(1, &[0x19, 0x80], true).unwrap();
//! ```

use std::collections::HashMap;
use thiserror::Error;

// ═══════════════════════════════════════════════════════════════════════
// Error types
// ═══════════════════════════════════════════════════════════════════════

/// Errors from LoRaWAN operations.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum LoRaError {
    /// Device not joined to a network.
    #[error("device not joined: call join_otaa() first")]
    NotJoined,

    /// Device already joined to a network.
    #[error("device already joined to network (dev_addr: {dev_addr:08X})")]
    AlreadyJoined {
        /// Current device address.
        dev_addr: u32,
    },

    /// OTAA join request failed.
    #[error("join failed: {reason}")]
    JoinFailed {
        /// Reason for failure.
        reason: String,
    },

    /// Invalid configuration parameter.
    #[error("invalid config: {reason}")]
    InvalidConfig {
        /// Reason the config is invalid.
        reason: String,
    },

    /// Uplink transmission failed.
    #[error("uplink failed: {reason}")]
    UplinkFailed {
        /// Reason for failure.
        reason: String,
    },

    /// Payload exceeds maximum allowed size.
    #[error("payload too large: {size} bytes (max: {max} for DR{data_rate})")]
    PayloadTooLarge {
        /// Actual payload size.
        size: usize,
        /// Maximum allowed size.
        max: usize,
        /// Current data rate index.
        data_rate: u8,
    },

    /// Invalid port number (must be 1-223).
    #[error("invalid port: {port} (valid: 1-223)")]
    InvalidPort {
        /// The invalid port number.
        port: u8,
    },

    /// Duty cycle limit exceeded.
    #[error("duty cycle limit exceeded: {airtime_ms}ms used of {limit_ms}ms")]
    DutyCycleExceeded {
        /// Airtime used in the current period.
        airtime_ms: u64,
        /// Maximum allowed airtime.
        limit_ms: u64,
    },

    /// Frame counter overflow or replay detected.
    #[error("frame counter error: {reason}")]
    FrameCounterError {
        /// Reason for the error.
        reason: String,
    },

    /// MAC command processing error.
    #[error("MAC command error: {reason}")]
    MacCommandError {
        /// Reason for the error.
        reason: String,
    },

    /// Class B/C mode error.
    #[error("class mode error: {reason}")]
    ClassModeError {
        /// Reason for the error.
        reason: String,
    },

    /// Multicast group error.
    #[error("multicast error: {reason}")]
    MulticastError {
        /// Reason for the error.
        reason: String,
    },
}

// ═══════════════════════════════════════════════════════════════════════
// Constants
// ═══════════════════════════════════════════════════════════════════════

/// Maximum LoRaWAN payload size at lowest data rate (SF12BW125).
pub const MAX_PAYLOAD_SF12: usize = 51;

/// Maximum LoRaWAN payload size at highest data rate (SF7BW125).
pub const MAX_PAYLOAD_SF7: usize = 222;

/// Minimum valid FPort (1).
pub const MIN_FPORT: u8 = 1;

/// Maximum valid FPort (223).
pub const MAX_FPORT: u8 = 223;

/// Maximum frame counter value before rollover.
pub const MAX_FCNT: u32 = 0xFFFF_FFFF;

/// Default RX1 delay in seconds.
pub const DEFAULT_RX1_DELAY_SECS: u8 = 1;

/// Default RX2 delay in seconds (RX1 + 1).
pub const DEFAULT_RX2_DELAY_SECS: u8 = 2;

/// Maximum number of multicast groups.
pub const MAX_MULTICAST_GROUPS: usize = 4;

/// Default Class B ping slot periodicity (every 128 seconds).
pub const DEFAULT_PING_SLOT_PERIOD: u32 = 128;

/// Default duty cycle limit (1% = 36 seconds per hour).
pub const DEFAULT_DUTY_CYCLE_LIMIT_MS: u64 = 36_000;

/// Duty cycle measurement window in milliseconds (1 hour).
pub const DUTY_CYCLE_WINDOW_MS: u64 = 3_600_000;

// ═══════════════════════════════════════════════════════════════════════
// Frequency plans
// ═══════════════════════════════════════════════════════════════════════

/// LoRaWAN regional frequency plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FrequencyPlan {
    /// Europe 863-870 MHz.
    EU868,
    /// US 902-928 MHz.
    US915,
    /// Australia 915-928 MHz.
    AU915,
    /// Asia 923 MHz.
    AS923,
    /// India 865-867 MHz.
    IN865,
    /// Korea 920-923 MHz.
    KR920,
}

impl FrequencyPlan {
    /// Returns the number of uplink channels for this plan.
    pub fn channel_count(self) -> u8 {
        match self {
            FrequencyPlan::EU868 => 16,
            FrequencyPlan::US915 => 72,
            FrequencyPlan::AU915 => 72,
            FrequencyPlan::AS923 => 16,
            FrequencyPlan::IN865 => 16,
            FrequencyPlan::KR920 => 16,
        }
    }

    /// Returns the number of available data rates.
    pub fn data_rate_count(self) -> u8 {
        match self {
            FrequencyPlan::EU868 => 7,
            FrequencyPlan::US915 => 14,
            FrequencyPlan::AU915 => 14,
            FrequencyPlan::AS923 => 8,
            FrequencyPlan::IN865 => 6,
            FrequencyPlan::KR920 => 6,
        }
    }

    /// Returns the maximum TX power in dBm.
    pub fn max_tx_power_dbm(self) -> i8 {
        match self {
            FrequencyPlan::EU868 => 16,
            FrequencyPlan::US915 => 30,
            FrequencyPlan::AU915 => 30,
            FrequencyPlan::AS923 => 16,
            FrequencyPlan::IN865 => 30,
            FrequencyPlan::KR920 => 14,
        }
    }

    /// Returns the default RX2 frequency in Hz.
    pub fn default_rx2_frequency(self) -> u32 {
        match self {
            FrequencyPlan::EU868 => 869_525_000,
            FrequencyPlan::US915 => 923_300_000,
            FrequencyPlan::AU915 => 923_300_000,
            FrequencyPlan::AS923 => 923_200_000,
            FrequencyPlan::IN865 => 866_550_000,
            FrequencyPlan::KR920 => 921_900_000,
        }
    }
}

impl std::fmt::Display for FrequencyPlan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FrequencyPlan::EU868 => write!(f, "EU868"),
            FrequencyPlan::US915 => write!(f, "US915"),
            FrequencyPlan::AU915 => write!(f, "AU915"),
            FrequencyPlan::AS923 => write!(f, "AS923"),
            FrequencyPlan::IN865 => write!(f, "IN865"),
            FrequencyPlan::KR920 => write!(f, "KR920"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Data rates
// ═══════════════════════════════════════════════════════════════════════

/// LoRaWAN data rate (EU868 encoding).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DataRate {
    /// SF12/125kHz — longest range, lowest throughput.
    SF12BW125,
    /// SF11/125kHz.
    SF11BW125,
    /// SF10/125kHz.
    SF10BW125,
    /// SF9/125kHz.
    SF9BW125,
    /// SF8/125kHz.
    SF8BW125,
    /// SF7/125kHz — shortest range, highest throughput.
    SF7BW125,
    /// SF7/250kHz — high throughput, reduced range.
    SF7BW250,
}

impl DataRate {
    /// Returns the data rate index (DR0-DR6).
    pub fn index(self) -> u8 {
        match self {
            DataRate::SF12BW125 => 0,
            DataRate::SF11BW125 => 1,
            DataRate::SF10BW125 => 2,
            DataRate::SF9BW125 => 3,
            DataRate::SF8BW125 => 4,
            DataRate::SF7BW125 => 5,
            DataRate::SF7BW250 => 6,
        }
    }

    /// Returns the maximum payload size in bytes for this data rate.
    pub fn max_payload_size(self) -> usize {
        match self {
            DataRate::SF12BW125 => 51,
            DataRate::SF11BW125 => 51,
            DataRate::SF10BW125 => 51,
            DataRate::SF9BW125 => 115,
            DataRate::SF8BW125 => 222,
            DataRate::SF7BW125 => 222,
            DataRate::SF7BW250 => 222,
        }
    }

    /// Creates a data rate from an index (DR0-DR6).
    ///
    /// Returns `None` for invalid indices.
    pub fn from_index(index: u8) -> Option<Self> {
        match index {
            0 => Some(DataRate::SF12BW125),
            1 => Some(DataRate::SF11BW125),
            2 => Some(DataRate::SF10BW125),
            3 => Some(DataRate::SF9BW125),
            4 => Some(DataRate::SF8BW125),
            5 => Some(DataRate::SF7BW125),
            6 => Some(DataRate::SF7BW250),
            _ => None,
        }
    }

    /// Returns the estimated airtime in milliseconds for a given payload size.
    ///
    /// This is a simplified calculation for simulation purposes.
    pub fn estimated_airtime_ms(self, payload_bytes: usize) -> u64 {
        let sf = match self {
            DataRate::SF12BW125 => 12,
            DataRate::SF11BW125 => 11,
            DataRate::SF10BW125 => 10,
            DataRate::SF9BW125 => 9,
            DataRate::SF8BW125 => 8,
            DataRate::SF7BW125 | DataRate::SF7BW250 => 7,
        };
        // Simplified: airtime roughly doubles per SF increment
        let base_ms = 10u64 + payload_bytes as u64;
        base_ms * (1u64 << (sf - 7))
    }
}

impl std::fmt::Display for DataRate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DataRate::SF12BW125 => write!(f, "SF12/125kHz"),
            DataRate::SF11BW125 => write!(f, "SF11/125kHz"),
            DataRate::SF10BW125 => write!(f, "SF10/125kHz"),
            DataRate::SF9BW125 => write!(f, "SF9/125kHz"),
            DataRate::SF8BW125 => write!(f, "SF8/125kHz"),
            DataRate::SF7BW125 => write!(f, "SF7/125kHz"),
            DataRate::SF7BW250 => write!(f, "SF7/250kHz"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Configuration
// ═══════════════════════════════════════════════════════════════════════

/// LoRaWAN device configuration.
#[derive(Debug, Clone)]
pub struct LoRaConfig {
    /// Device EUI (unique 8-byte identifier).
    pub dev_eui: [u8; 8],
    /// Application EUI (8-byte identifier).
    pub app_eui: [u8; 8],
    /// Application key (16-byte AES-128 root key).
    pub app_key: [u8; 16],
    /// Regional frequency plan.
    pub frequency_plan: FrequencyPlan,
    /// ADR (Adaptive Data Rate) enabled.
    pub adr_enabled: bool,
    /// Initial data rate index (0-6).
    pub initial_data_rate: u8,
    /// Initial TX power index.
    pub initial_tx_power: u8,
}

impl LoRaConfig {
    /// Creates a new LoRaWAN configuration with default EUIs and key.
    ///
    /// The default EUI/key values are suitable for simulation only.
    pub fn new(frequency_plan: FrequencyPlan) -> Self {
        Self {
            dev_eui: [0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77],
            app_eui: [0x70, 0xB3, 0xD5, 0x7E, 0xD0, 0x00, 0x00, 0x01],
            app_key: [
                0x2B, 0x7E, 0x15, 0x16, 0x28, 0xAE, 0xD2, 0xA6, 0xAB, 0xF7, 0x15, 0x88, 0x09, 0xCF,
                0x4F, 0x3C,
            ],
            frequency_plan,
            adr_enabled: true,
            initial_data_rate: 0,
            initial_tx_power: 0,
        }
    }

    /// Sets the device EUI.
    pub fn with_dev_eui(mut self, dev_eui: [u8; 8]) -> Self {
        self.dev_eui = dev_eui;
        self
    }

    /// Sets the application EUI.
    pub fn with_app_eui(mut self, app_eui: [u8; 8]) -> Self {
        self.app_eui = app_eui;
        self
    }

    /// Sets the application key.
    pub fn with_app_key(mut self, app_key: [u8; 16]) -> Self {
        self.app_key = app_key;
        self
    }

    /// Enables or disables ADR.
    pub fn with_adr(mut self, enabled: bool) -> Self {
        self.adr_enabled = enabled;
        self
    }

    /// Sets the initial data rate index.
    pub fn with_data_rate(mut self, dr: u8) -> Self {
        self.initial_data_rate = dr;
        self
    }

    /// Validates the configuration.
    pub fn validate(&self) -> Result<(), LoRaError> {
        if self.dev_eui == [0u8; 8] {
            return Err(LoRaError::InvalidConfig {
                reason: "dev_eui cannot be all zeros".to_string(),
            });
        }
        let max_dr = self.frequency_plan.data_rate_count();
        if self.initial_data_rate >= max_dr {
            return Err(LoRaError::InvalidConfig {
                reason: format!(
                    "initial_data_rate {} exceeds max {} for {}",
                    self.initial_data_rate,
                    max_dr - 1,
                    self.frequency_plan
                ),
            });
        }
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Device state
// ═══════════════════════════════════════════════════════════════════════

/// LoRaWAN device operating state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoRaDeviceState {
    /// Device is idle (not joined).
    Idle,
    /// Device is performing OTAA join.
    Joining,
    /// Device has successfully joined the network.
    Joined,
    /// Device is transmitting an uplink.
    Transmitting,
    /// Device is in a receive window (RX1/RX2).
    Receiving,
    /// Device is in sleep/low-power mode.
    Sleep,
}

impl std::fmt::Display for LoRaDeviceState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoRaDeviceState::Idle => write!(f, "Idle"),
            LoRaDeviceState::Joining => write!(f, "Joining"),
            LoRaDeviceState::Joined => write!(f, "Joined"),
            LoRaDeviceState::Transmitting => write!(f, "Transmitting"),
            LoRaDeviceState::Receiving => write!(f, "Receiving"),
            LoRaDeviceState::Sleep => write!(f, "Sleep"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Join accept
// ═══════════════════════════════════════════════════════════════════════

/// Result of a successful OTAA join procedure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JoinAccept {
    /// Assigned device address (4 bytes).
    pub dev_addr: u32,
    /// Network session key (16 bytes).
    pub nwk_skey: [u8; 16],
    /// Application session key (16 bytes).
    pub app_skey: [u8; 16],
    /// RX1 delay in seconds.
    pub rx_delay: u8,
}

// ═══════════════════════════════════════════════════════════════════════
// Downlink message
// ═══════════════════════════════════════════════════════════════════════

/// A received downlink message from the network server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Downlink {
    /// FPort (1-223).
    pub port: u8,
    /// Decrypted payload.
    pub payload: Vec<u8>,
    /// Downlink frame counter.
    pub fcnt: u32,
    /// Whether this was received in RX1 or RX2.
    pub rx_window: u8,
}

// ═══════════════════════════════════════════════════════════════════════
// LoRaWAN events
// ═══════════════════════════════════════════════════════════════════════

/// Events emitted by the LoRaWAN stack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoRaEvent {
    /// Device successfully joined the network.
    Joined {
        /// Assigned device address.
        dev_addr: u32,
    },
    /// Join attempt failed.
    JoinFailed {
        /// Reason for failure.
        reason: String,
    },
    /// Uplink was sent successfully.
    UplinkSent {
        /// Frame counter of the uplink.
        fcnt: u32,
        /// Whether it was confirmed.
        confirmed: bool,
    },
    /// Downlink was received.
    DownlinkReceived {
        /// The received downlink.
        port: u8,
        /// Downlink frame counter.
        fcnt: u32,
    },
    /// Class B beacon received.
    BeaconReceived {
        /// Beacon timestamp (GPS time).
        gps_time: u32,
    },
    /// Link check response received.
    LinkCheckOk {
        /// Demodulation margin (dB).
        margin: u8,
        /// Number of gateways that received the uplink.
        gw_count: u8,
    },
}

// ═══════════════════════════════════════════════════════════════════════
// ADR (Adaptive Data Rate)
// ═══════════════════════════════════════════════════════════════════════

/// Adaptive Data Rate controller.
///
/// Adjusts data rate and TX power based on link quality to optimize
/// battery life and network capacity.
#[derive(Debug, Clone)]
pub struct AdaptiveDataRate {
    /// Current data rate.
    data_rate: DataRate,
    /// Current TX power index.
    tx_power_index: u8,
    /// Link margin from last link check (dB).
    link_margin_db: i8,
    /// Target SNR for the current data rate (dB).
    target_snr_db: i8,
    /// Number of consecutive uplinks without downlink response.
    uplink_count_since_downlink: u32,
    /// Whether ADR is enabled.
    enabled: bool,
}

impl AdaptiveDataRate {
    /// Creates a new ADR controller.
    pub fn new(initial_dr: DataRate, enabled: bool) -> Self {
        let target_snr = Self::target_snr_for_dr(initial_dr);
        Self {
            data_rate: initial_dr,
            tx_power_index: 0,
            link_margin_db: 0,
            target_snr_db: target_snr,
            uplink_count_since_downlink: 0,
            enabled,
        }
    }

    /// Returns the current data rate.
    pub fn data_rate(&self) -> DataRate {
        self.data_rate
    }

    /// Returns the current TX power index.
    pub fn tx_power_index(&self) -> u8 {
        self.tx_power_index
    }

    /// Returns the link margin in dB.
    pub fn link_margin_db(&self) -> i8 {
        self.link_margin_db
    }

    /// Returns whether ADR is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Returns the number of uplinks since last downlink.
    pub fn uplink_count_since_downlink(&self) -> u32 {
        self.uplink_count_since_downlink
    }

    /// Updates the link margin from a link check response.
    pub fn update_link_margin(&mut self, margin_db: i8) {
        self.link_margin_db = margin_db;
        self.uplink_count_since_downlink = 0;
    }

    /// Records that an uplink was sent without a downlink response.
    pub fn record_uplink(&mut self) {
        self.uplink_count_since_downlink += 1;
    }

    /// Adjusts data rate based on current link conditions.
    ///
    /// Returns `true` if the data rate was changed.
    pub fn adjust_data_rate(&mut self) -> bool {
        if !self.enabled {
            return false;
        }

        let snr_margin = self.link_margin_db - self.target_snr_db;
        let old_dr = self.data_rate;

        if snr_margin > 6 {
            // Good signal: try increasing data rate (faster)
            if let Some(higher) = DataRate::from_index(self.data_rate.index() + 1) {
                self.data_rate = higher;
                self.target_snr_db = Self::target_snr_for_dr(self.data_rate);
            }
        } else if snr_margin < -3 {
            // Poor signal: decrease data rate (more robust)
            if self.data_rate.index() > 0 {
                if let Some(lower) = DataRate::from_index(self.data_rate.index() - 1) {
                    self.data_rate = lower;
                    self.target_snr_db = Self::target_snr_for_dr(self.data_rate);
                }
            }
        }

        self.data_rate != old_dr
    }

    /// Returns the target SNR for a given data rate.
    fn target_snr_for_dr(dr: DataRate) -> i8 {
        match dr {
            DataRate::SF12BW125 => -20,
            DataRate::SF11BW125 => -17,
            DataRate::SF10BW125 => -15,
            DataRate::SF9BW125 => -12,
            DataRate::SF8BW125 => -10,
            DataRate::SF7BW125 | DataRate::SF7BW250 => -7,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Duty cycle tracking
// ═══════════════════════════════════════════════════════════════════════

/// Tracks cumulative TX airtime for duty cycle enforcement.
///
/// LoRaWAN requires devices to obey regional duty cycle limits
/// (e.g., 1% for EU868 sub-bands).
#[derive(Debug, Clone)]
pub struct DutyCycleTracker {
    /// Total TX airtime in the current window (ms).
    tx_airtime_ms: u64,
    /// Duty cycle limit for the current window (ms).
    duty_cycle_limit_ms: u64,
    /// Timestamp when the current window started (ms, simulated).
    window_start_ms: u64,
}

impl DutyCycleTracker {
    /// Creates a new duty cycle tracker.
    pub fn new(limit_ms: u64) -> Self {
        Self {
            tx_airtime_ms: 0,
            duty_cycle_limit_ms: limit_ms,
            window_start_ms: 0,
        }
    }

    /// Returns the total TX airtime used in the current window.
    pub fn tx_airtime_ms(&self) -> u64 {
        self.tx_airtime_ms
    }

    /// Returns the duty cycle limit.
    pub fn duty_cycle_limit_ms(&self) -> u64 {
        self.duty_cycle_limit_ms
    }

    /// Returns the start of the current measurement window (ms).
    pub fn window_start_ms(&self) -> u64 {
        self.window_start_ms
    }

    /// Returns whether the device can transmit with the given airtime.
    pub fn can_transmit(&self, airtime_ms: u64) -> bool {
        self.tx_airtime_ms + airtime_ms <= self.duty_cycle_limit_ms
    }

    /// Records a transmission with the given airtime.
    pub fn record_transmission(&mut self, airtime_ms: u64) {
        self.tx_airtime_ms += airtime_ms;
    }

    /// Resets the duty cycle window (called periodically).
    pub fn reset_window(&mut self) {
        self.tx_airtime_ms = 0;
        self.window_start_ms += DUTY_CYCLE_WINDOW_MS;
    }

    /// Sets the duty cycle limit (from a MAC command).
    pub fn set_limit(&mut self, limit_ms: u64) {
        self.duty_cycle_limit_ms = limit_ms;
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Class B configuration
// ═══════════════════════════════════════════════════════════════════════

/// Configuration for Class B beacon-synchronized mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassBConfig {
    /// Ping slot periodicity in seconds (power of 2: 1, 2, 4, ..., 128).
    pub ping_slot_period: u32,
    /// Beacon frequency in Hz (0 = use default for frequency plan).
    pub beacon_frequency: u32,
}

impl ClassBConfig {
    /// Creates a new Class B configuration with defaults.
    pub fn new() -> Self {
        Self {
            ping_slot_period: DEFAULT_PING_SLOT_PERIOD,
            beacon_frequency: 0,
        }
    }

    /// Sets the ping slot periodicity.
    pub fn with_ping_slot_period(mut self, period: u32) -> Self {
        self.ping_slot_period = period;
        self
    }

    /// Sets the beacon frequency.
    pub fn with_beacon_frequency(mut self, freq: u32) -> Self {
        self.beacon_frequency = freq;
        self
    }
}

impl Default for ClassBConfig {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Beacon info
// ═══════════════════════════════════════════════════════════════════════

/// Information from a received Class B beacon.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BeaconInfo {
    /// GPS time from the beacon (seconds since GPS epoch).
    pub gps_time: u32,
    /// Gateway coordinates (latitude, longitude) in 1/1000 degrees, if present.
    pub gateway_coordinates: Option<(i32, i32)>,
    /// Beacon info descriptor byte.
    pub info_desc: u8,
}

// ═══════════════════════════════════════════════════════════════════════
// Multicast groups
// ═══════════════════════════════════════════════════════════════════════

/// A LoRaWAN multicast group for receiving group messages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MulticastGroup {
    /// Multicast device address.
    pub addr: u32,
    /// Multicast network session key.
    pub nwk_skey: [u8; 16],
    /// Multicast application session key.
    pub app_skey: [u8; 16],
    /// Downlink frame counter for this group.
    pub fcnt_down: u32,
}

impl MulticastGroup {
    /// Creates a new multicast group.
    pub fn new(addr: u32, nwk_skey: [u8; 16], app_skey: [u8; 16]) -> Self {
        Self {
            addr,
            nwk_skey,
            app_skey,
            fcnt_down: 0,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// MAC commands
// ═══════════════════════════════════════════════════════════════════════

/// LoRaWAN MAC commands exchanged between device and network server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MacCommand {
    /// Link check request/response.
    LinkCheck {
        /// Demodulation margin (dB) — in response only.
        margin: Option<u8>,
        /// Number of gateways — in response only.
        gw_count: Option<u8>,
    },
    /// Device time request/response.
    DeviceTime {
        /// GPS epoch seconds — in response only.
        gps_time: Option<u32>,
    },
    /// New channel configuration from network.
    NewChannel {
        /// Channel index.
        ch_index: u8,
        /// Frequency in Hz.
        frequency: u32,
        /// Minimum data rate.
        min_dr: u8,
        /// Maximum data rate.
        max_dr: u8,
    },
    /// RX parameter setup from network.
    RxParamSetup {
        /// RX1 data rate offset.
        rx1_dr_offset: u8,
        /// RX2 data rate.
        rx2_data_rate: u8,
        /// RX2 frequency in Hz.
        rx2_frequency: u32,
    },
    /// Duty cycle request from network.
    DutyCycleReq {
        /// Max duty cycle exponent (0 = no limit, 1..15 = 1/2^n).
        max_duty_cycle: u8,
    },
}

// ═══════════════════════════════════════════════════════════════════════
// Device class mode
// ═══════════════════════════════════════════════════════════════════════

/// LoRaWAN operating class.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceClass {
    /// Class A: baseline, uplink-initiated with two short RX windows.
    A,
    /// Class B: beacon-synchronized receive slots.
    B,
    /// Class C: continuous receive window (except during TX).
    C,
}

impl std::fmt::Display for DeviceClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeviceClass::A => write!(f, "Class A"),
            DeviceClass::B => write!(f, "Class B"),
            DeviceClass::C => write!(f, "Class C"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// LoRaWAN device (simulation)
// ═══════════════════════════════════════════════════════════════════════

/// Simulated LoRaWAN end device.
///
/// Models the LoRaWAN 1.0.4 device lifecycle:
/// `new()` -> `join_otaa()` -> `send_uplink()` / `receive_downlink()`.
///
/// Supports Class A, B, and C operating modes, ADR, frame counter
/// tracking, duty cycle enforcement, and multicast groups.
#[derive(Debug)]
pub struct LoRaDevice {
    /// Device configuration.
    config: LoRaConfig,
    /// Current operating state.
    state: LoRaDeviceState,
    /// Current device class.
    device_class: DeviceClass,
    /// Assigned device address (set after join).
    dev_addr: Option<u32>,
    /// Network session key (set after join).
    nwk_skey: Option<[u8; 16]>,
    /// Application session key (set after join).
    app_skey: Option<[u8; 16]>,
    /// Uplink frame counter.
    fcnt_up: u32,
    /// Downlink frame counter.
    fcnt_down: u32,
    /// RX1 delay in seconds.
    rx_delay: u8,
    /// ADR controller.
    adr: AdaptiveDataRate,
    /// Duty cycle tracker.
    duty_cycle: DutyCycleTracker,
    /// Pending downlinks (simulated RX window).
    pending_downlinks: Vec<Downlink>,
    /// Event log.
    events: Vec<LoRaEvent>,
    /// Class B configuration (if enabled).
    class_b_config: Option<ClassBConfig>,
    /// Last received beacon info.
    last_beacon: Option<BeaconInfo>,
    /// Joined multicast groups.
    multicast_groups: HashMap<u32, MulticastGroup>,
    /// MAC command queue (pending responses).
    pending_mac_commands: Vec<MacCommand>,
}

impl LoRaDevice {
    /// Creates a new LoRaWAN device in idle state.
    pub fn new(config: LoRaConfig) -> Self {
        let initial_dr =
            DataRate::from_index(config.initial_data_rate).unwrap_or(DataRate::SF12BW125);
        let adr = AdaptiveDataRate::new(initial_dr, config.adr_enabled);
        Self {
            config,
            state: LoRaDeviceState::Idle,
            device_class: DeviceClass::A,
            dev_addr: None,
            nwk_skey: None,
            app_skey: None,
            fcnt_up: 0,
            fcnt_down: 0,
            rx_delay: DEFAULT_RX1_DELAY_SECS,
            adr,
            duty_cycle: DutyCycleTracker::new(DEFAULT_DUTY_CYCLE_LIMIT_MS),
            pending_downlinks: Vec::new(),
            events: Vec::new(),
            class_b_config: None,
            last_beacon: None,
            multicast_groups: HashMap::new(),
            pending_mac_commands: Vec::new(),
        }
    }

    /// Returns the current device state.
    pub fn state(&self) -> LoRaDeviceState {
        self.state
    }

    /// Returns the current device class.
    pub fn device_class(&self) -> DeviceClass {
        self.device_class
    }

    /// Returns the device configuration.
    pub fn config(&self) -> &LoRaConfig {
        &self.config
    }

    /// Returns the assigned device address, if joined.
    pub fn dev_addr(&self) -> Option<u32> {
        self.dev_addr
    }

    /// Returns the uplink frame counter.
    pub fn fcnt_up(&self) -> u32 {
        self.fcnt_up
    }

    /// Returns the downlink frame counter.
    pub fn fcnt_down(&self) -> u32 {
        self.fcnt_down
    }

    /// Returns the current data rate.
    pub fn current_data_rate(&self) -> DataRate {
        self.adr.data_rate()
    }

    /// Returns a reference to the ADR controller.
    pub fn adr(&self) -> &AdaptiveDataRate {
        &self.adr
    }

    /// Returns a reference to the duty cycle tracker.
    pub fn duty_cycle(&self) -> &DutyCycleTracker {
        &self.duty_cycle
    }

    /// Returns the event log.
    pub fn events(&self) -> &[LoRaEvent] {
        &self.events
    }

    /// Returns the last received beacon info.
    pub fn last_beacon(&self) -> Option<&BeaconInfo> {
        self.last_beacon.as_ref()
    }

    /// Returns the Class B configuration, if enabled.
    pub fn class_b_config(&self) -> Option<&ClassBConfig> {
        self.class_b_config.as_ref()
    }

    /// Returns the joined multicast groups.
    pub fn multicast_groups(&self) -> &HashMap<u32, MulticastGroup> {
        &self.multicast_groups
    }

    /// Returns the network session key, if joined.
    pub fn nwk_skey(&self) -> Option<&[u8; 16]> {
        self.nwk_skey.as_ref()
    }

    /// Returns the application session key, if joined.
    pub fn app_skey(&self) -> Option<&[u8; 16]> {
        self.app_skey.as_ref()
    }

    /// Returns the RX1 delay in seconds.
    pub fn rx_delay(&self) -> u8 {
        self.rx_delay
    }

    /// Returns any pending MAC command responses.
    pub fn pending_mac_commands(&self) -> &[MacCommand] {
        &self.pending_mac_commands
    }

    /// Performs an OTAA join procedure (simulation).
    ///
    /// Simulates the exchange of JoinRequest and JoinAccept messages.
    /// On success, the device transitions to `Joined` state with
    /// assigned session keys and device address.
    pub fn join_otaa(&mut self) -> Result<JoinAccept, LoRaError> {
        self.config.validate()?;

        if let Some(addr) = self.dev_addr {
            return Err(LoRaError::AlreadyJoined { dev_addr: addr });
        }

        self.state = LoRaDeviceState::Joining;

        // Simulation: derive deterministic session keys and address
        let accept = generate_join_accept(&self.config);

        self.state = LoRaDeviceState::Joined;
        self.dev_addr = Some(accept.dev_addr);
        self.nwk_skey = Some(accept.nwk_skey);
        self.app_skey = Some(accept.app_skey);
        self.rx_delay = accept.rx_delay;
        self.fcnt_up = 0;
        self.fcnt_down = 0;

        self.events.push(LoRaEvent::Joined {
            dev_addr: accept.dev_addr,
        });

        Ok(accept)
    }

    /// Sends an uplink message (Class A).
    ///
    /// Validates the payload size against the current data rate limit,
    /// checks duty cycle, increments the frame counter, and records
    /// the transmission.
    pub fn send_uplink(
        &mut self,
        port: u8,
        payload: &[u8],
        confirmed: bool,
    ) -> Result<(), LoRaError> {
        self.ensure_joined()?;
        validate_port(port)?;

        let max_size = self.adr.data_rate().max_payload_size();
        if payload.len() > max_size {
            return Err(LoRaError::PayloadTooLarge {
                size: payload.len(),
                max: max_size,
                data_rate: self.adr.data_rate().index(),
            });
        }

        // Check duty cycle
        let airtime = self.adr.data_rate().estimated_airtime_ms(payload.len());
        if !self.duty_cycle.can_transmit(airtime) {
            return Err(LoRaError::DutyCycleExceeded {
                airtime_ms: self.duty_cycle.tx_airtime_ms() + airtime,
                limit_ms: self.duty_cycle.duty_cycle_limit_ms(),
            });
        }

        // Check frame counter overflow
        if self.fcnt_up == MAX_FCNT {
            return Err(LoRaError::FrameCounterError {
                reason: "uplink frame counter overflow".to_string(),
            });
        }

        self.state = LoRaDeviceState::Transmitting;
        self.duty_cycle.record_transmission(airtime);

        let fcnt = self.fcnt_up;
        self.fcnt_up += 1;
        self.adr.record_uplink();

        self.events.push(LoRaEvent::UplinkSent { fcnt, confirmed });
        self.state = LoRaDeviceState::Receiving;

        // After RX windows close, return to joined
        self.state = LoRaDeviceState::Joined;

        Ok(())
    }

    /// Receives a pending downlink message, if any.
    ///
    /// Simulates the RX1/RX2 receive window behavior.
    pub fn receive_downlink(&mut self) -> Option<Downlink> {
        if self.state != LoRaDeviceState::Joined {
            return None;
        }
        let dl = self.pending_downlinks.pop();
        if let Some(ref d) = dl {
            self.fcnt_down = d.fcnt;
            self.events.push(LoRaEvent::DownlinkReceived {
                port: d.port,
                fcnt: d.fcnt,
            });
        }
        dl
    }

    /// Injects a simulated downlink for testing.
    pub fn simulate_downlink(&mut self, port: u8, payload: Vec<u8>) {
        let fcnt = self.fcnt_down + 1;
        self.pending_downlinks.push(Downlink {
            port,
            payload,
            fcnt,
            rx_window: 1,
        });
    }

    /// Enables Class B mode with beacon synchronization.
    pub fn enable_class_b(&mut self, config: ClassBConfig) -> Result<(), LoRaError> {
        self.ensure_joined()?;
        if self.device_class == DeviceClass::B {
            return Err(LoRaError::ClassModeError {
                reason: "already in Class B mode".to_string(),
            });
        }
        self.class_b_config = Some(config);
        self.device_class = DeviceClass::B;
        Ok(())
    }

    /// Enables Class C mode (continuous receive).
    pub fn enable_class_c(&mut self) -> Result<(), LoRaError> {
        self.ensure_joined()?;
        if self.device_class == DeviceClass::C {
            return Err(LoRaError::ClassModeError {
                reason: "already in Class C mode".to_string(),
            });
        }
        self.class_b_config = None;
        self.device_class = DeviceClass::C;
        Ok(())
    }

    /// Reverts to Class A mode.
    pub fn revert_to_class_a(&mut self) -> Result<(), LoRaError> {
        self.ensure_joined()?;
        self.class_b_config = None;
        self.device_class = DeviceClass::A;
        Ok(())
    }

    /// Simulates receiving a Class B beacon.
    pub fn simulate_beacon(&mut self, beacon: BeaconInfo) {
        self.last_beacon = Some(beacon.clone());
        self.events.push(LoRaEvent::BeaconReceived {
            gps_time: beacon.gps_time,
        });
    }

    /// Joins a multicast group.
    pub fn join_multicast_group(&mut self, group: MulticastGroup) -> Result<(), LoRaError> {
        self.ensure_joined()?;
        if self.multicast_groups.len() >= MAX_MULTICAST_GROUPS {
            return Err(LoRaError::MulticastError {
                reason: format!(
                    "maximum of {} multicast groups reached",
                    MAX_MULTICAST_GROUPS
                ),
            });
        }
        if self.multicast_groups.contains_key(&group.addr) {
            return Err(LoRaError::MulticastError {
                reason: format!("already member of group {:08X}", group.addr),
            });
        }
        self.multicast_groups.insert(group.addr, group);
        Ok(())
    }

    /// Leaves a multicast group.
    pub fn leave_multicast_group(&mut self, addr: u32) -> Result<(), LoRaError> {
        if self.multicast_groups.remove(&addr).is_none() {
            return Err(LoRaError::MulticastError {
                reason: format!("not a member of group {:08X}", addr),
            });
        }
        Ok(())
    }

    /// Processes a MAC command from the network server.
    ///
    /// Returns an optional response MAC command to send in the next uplink.
    pub fn process_mac_command(&mut self, cmd: MacCommand) -> Option<MacCommand> {
        match cmd {
            MacCommand::LinkCheck { margin, gw_count } => {
                if let (Some(m), Some(g)) = (margin, gw_count) {
                    self.adr.update_link_margin(m as i8);
                    self.events.push(LoRaEvent::LinkCheckOk {
                        margin: m,
                        gw_count: g,
                    });
                    None
                } else {
                    // Request: queue a LinkCheck request
                    Some(MacCommand::LinkCheck {
                        margin: None,
                        gw_count: None,
                    })
                }
            }
            MacCommand::DutyCycleReq { max_duty_cycle } => {
                let limit = if max_duty_cycle == 0 {
                    DEFAULT_DUTY_CYCLE_LIMIT_MS * 100
                } else {
                    DUTY_CYCLE_WINDOW_MS / (1u64 << u64::from(max_duty_cycle))
                };
                self.duty_cycle.set_limit(limit);
                None
            }
            MacCommand::RxParamSetup {
                rx1_dr_offset: _,
                rx2_data_rate: _,
                rx2_frequency: _,
            } => {
                // Acknowledge the RX parameter change
                None
            }
            MacCommand::NewChannel { .. } => {
                // Acknowledge the new channel
                None
            }
            MacCommand::DeviceTime { gps_time } => {
                if gps_time.is_some() {
                    // Response received, no further action
                    None
                } else {
                    // Request: queue a DeviceTime request
                    Some(MacCommand::DeviceTime { gps_time: None })
                }
            }
        }
    }

    /// Sets the device to sleep mode.
    pub fn sleep(&mut self) {
        self.state = LoRaDeviceState::Sleep;
    }

    /// Wakes the device from sleep mode.
    pub fn wake(&mut self) {
        if self.state == LoRaDeviceState::Sleep {
            self.state = if self.dev_addr.is_some() {
                LoRaDeviceState::Joined
            } else {
                LoRaDeviceState::Idle
            };
        }
    }

    /// Ensures the device is joined; returns an error if not.
    fn ensure_joined(&self) -> Result<(), LoRaError> {
        if self.dev_addr.is_none() {
            return Err(LoRaError::NotJoined);
        }
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// IoT stack (unified interface)
// ═══════════════════════════════════════════════════════════════════════

/// Protocol availability flags for the IoT stack.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IotProtocols {
    /// Whether WiFi is available.
    pub wifi: bool,
    /// Whether BLE is available.
    pub ble: bool,
    /// Whether MQTT is available.
    pub mqtt: bool,
    /// Whether LoRaWAN is available.
    pub lorawan: bool,
}

impl IotProtocols {
    /// Creates flags with all protocols disabled.
    pub fn none() -> Self {
        Self {
            wifi: false,
            ble: false,
            mqtt: false,
            lorawan: false,
        }
    }

    /// Creates flags with all protocols enabled.
    pub fn all() -> Self {
        Self {
            wifi: true,
            ble: true,
            mqtt: true,
            lorawan: true,
        }
    }
}

/// Unified IoT connectivity stack combining WiFi, BLE, MQTT, and LoRaWAN.
///
/// Provides a single entry point for managing multiple IoT protocols
/// in an embedded application. Each protocol is independently
/// configurable and can be enabled/disabled at runtime.
#[derive(Debug)]
pub struct IotStack {
    /// Which protocols are enabled.
    protocols: IotProtocols,
    /// LoRaWAN device (owned by the stack).
    lorawan_device: Option<LoRaDevice>,
    /// Protocol-level event log.
    event_log: Vec<String>,
}

impl IotStack {
    /// Creates a new IoT stack with the specified protocols.
    pub fn new(protocols: IotProtocols) -> Self {
        Self {
            protocols,
            lorawan_device: None,
            event_log: Vec::new(),
        }
    }

    /// Returns the enabled protocols.
    pub fn protocols(&self) -> &IotProtocols {
        &self.protocols
    }

    /// Returns the event log.
    pub fn event_log(&self) -> &[String] {
        &self.event_log
    }

    /// Returns a reference to the LoRaWAN device, if configured.
    pub fn lorawan_device(&self) -> Option<&LoRaDevice> {
        self.lorawan_device.as_ref()
    }

    /// Returns a mutable reference to the LoRaWAN device, if configured.
    pub fn lorawan_device_mut(&mut self) -> Option<&mut LoRaDevice> {
        self.lorawan_device.as_mut()
    }

    /// Configures and attaches a LoRaWAN device to the stack.
    pub fn attach_lorawan(&mut self, device: LoRaDevice) -> Result<(), LoRaError> {
        if !self.protocols.lorawan {
            return Err(LoRaError::InvalidConfig {
                reason: "LoRaWAN protocol not enabled in stack".to_string(),
            });
        }
        self.lorawan_device = Some(device);
        self.event_log.push("LoRaWAN device attached".to_string());
        Ok(())
    }

    /// Sends data via the best available protocol.
    ///
    /// Tries LoRaWAN first (if joined), falling back to logging.
    pub fn send_data(&mut self, payload: &[u8]) -> Result<(), LoRaError> {
        if let Some(ref mut device) = self.lorawan_device {
            if device.dev_addr().is_some() {
                device.send_uplink(1, payload, false)?;
                self.event_log
                    .push(format!("sent {} bytes via LoRaWAN", payload.len()));
                return Ok(());
            }
        }

        self.event_log
            .push(format!("no connected protocol for {} bytes", payload.len()));
        Err(LoRaError::NotJoined)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Validation helpers
// ═══════════════════════════════════════════════════════════════════════

/// Validates a LoRaWAN FPort number.
fn validate_port(port: u8) -> Result<(), LoRaError> {
    if !(MIN_FPORT..=MAX_FPORT).contains(&port) {
        return Err(LoRaError::InvalidPort { port });
    }
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// Simulation helpers
// ═══════════════════════════════════════════════════════════════════════

/// Generates a simulated JoinAccept from device configuration.
///
/// Derives deterministic session keys and device address from the
/// config EUIs and key for reproducible test behavior.
fn generate_join_accept(config: &LoRaConfig) -> JoinAccept {
    // Derive a deterministic device address from dev_eui
    let dev_addr = u32::from_be_bytes([
        config.dev_eui[4],
        config.dev_eui[5],
        config.dev_eui[6],
        config.dev_eui[7],
    ]);

    // Derive session keys by XOR-mixing app_key with EUI bytes
    let mut nwk_skey = config.app_key;
    for (i, b) in config.dev_eui.iter().enumerate() {
        nwk_skey[i] ^= b;
    }
    let mut app_skey = config.app_key;
    for (i, b) in config.app_eui.iter().enumerate() {
        app_skey[i + 8] ^= b;
    }

    JoinAccept {
        dev_addr,
        nwk_skey,
        app_skey,
        rx_delay: DEFAULT_RX1_DELAY_SECS,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Free-standing convenience functions
// ═══════════════════════════════════════════════════════════════════════

/// Creates a new LoRaWAN device with the given frequency plan.
pub fn lorawan_new(plan: FrequencyPlan) -> LoRaDevice {
    LoRaDevice::new(LoRaConfig::new(plan))
}

/// Performs OTAA join (convenience wrapper).
pub fn lorawan_join(device: &mut LoRaDevice) -> Result<JoinAccept, LoRaError> {
    device.join_otaa()
}

/// Sends an uplink message (convenience wrapper).
pub fn lorawan_send(
    device: &mut LoRaDevice,
    port: u8,
    payload: &[u8],
    confirmed: bool,
) -> Result<(), LoRaError> {
    device.send_uplink(port, payload, confirmed)
}

/// Receives a downlink message (convenience wrapper).
pub fn lorawan_receive(device: &mut LoRaDevice) -> Option<Downlink> {
    device.receive_downlink()
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── Sprint 21: MAC Layer Tests ──

    #[test]
    fn s21_1_config_validation_succeeds() {
        let config = LoRaConfig::new(FrequencyPlan::EU868);
        assert!(config.validate().is_ok());
        assert_eq!(config.frequency_plan, FrequencyPlan::EU868);
        assert!(config.adr_enabled);

        // All-zero dev_eui is invalid
        let bad = LoRaConfig::new(FrequencyPlan::EU868).with_dev_eui([0u8; 8]);
        assert!(matches!(
            bad.validate(),
            Err(LoRaError::InvalidConfig { .. })
        ));

        // Invalid data rate for plan
        let bad = LoRaConfig::new(FrequencyPlan::EU868).with_data_rate(99);
        assert!(matches!(
            bad.validate(),
            Err(LoRaError::InvalidConfig { .. })
        ));
    }

    #[test]
    fn s21_2_otaa_join_succeeds() {
        let config = LoRaConfig::new(FrequencyPlan::EU868);
        let mut device = LoRaDevice::new(config);
        assert_eq!(device.state(), LoRaDeviceState::Idle);
        assert!(device.dev_addr().is_none());

        let accept = device.join_otaa().unwrap();
        assert_eq!(device.state(), LoRaDeviceState::Joined);
        assert_eq!(device.dev_addr(), Some(accept.dev_addr));
        assert_eq!(accept.rx_delay, DEFAULT_RX1_DELAY_SECS);
        assert_eq!(device.fcnt_up(), 0);
        assert_eq!(device.fcnt_down(), 0);

        // Event recorded
        assert_eq!(device.events().len(), 1);
        assert!(matches!(device.events()[0], LoRaEvent::Joined { .. }));
    }

    #[test]
    fn s21_3_otaa_join_already_joined_fails() {
        let mut device = lorawan_new(FrequencyPlan::EU868);
        device.join_otaa().unwrap();

        let err = device.join_otaa().unwrap_err();
        assert!(matches!(err, LoRaError::AlreadyJoined { .. }));
    }

    #[test]
    fn s21_4_uplink_succeeds() {
        let mut device = lorawan_new(FrequencyPlan::EU868);
        device.join_otaa().unwrap();

        device.send_uplink(1, &[0x19, 0x80], true).unwrap();
        assert_eq!(device.fcnt_up(), 1);

        device.send_uplink(2, &[0xAB], false).unwrap();
        assert_eq!(device.fcnt_up(), 2);

        // Events recorded (join + 2 uplinks)
        assert_eq!(device.events().len(), 3);
        assert!(matches!(
            device.events()[1],
            LoRaEvent::UplinkSent {
                fcnt: 0,
                confirmed: true
            }
        ));
    }

    #[test]
    fn s21_5_uplink_not_joined_fails() {
        let mut device = lorawan_new(FrequencyPlan::US915);

        let err = device.send_uplink(1, &[0x42], false).unwrap_err();
        assert_eq!(err, LoRaError::NotJoined);
    }

    #[test]
    fn s21_6_downlink_receive() {
        let mut device = lorawan_new(FrequencyPlan::EU868);
        device.join_otaa().unwrap();

        // No pending downlinks
        assert!(device.receive_downlink().is_none());

        // Inject a simulated downlink
        device.simulate_downlink(10, vec![0xDE, 0xAD]);
        let dl = device.receive_downlink().unwrap();
        assert_eq!(dl.port, 10);
        assert_eq!(dl.payload, vec![0xDE, 0xAD]);
        assert_eq!(dl.fcnt, 1);
        assert_eq!(device.fcnt_down(), 1);
    }

    #[test]
    fn s21_7_frame_counter_tracking() {
        let mut device = lorawan_new(FrequencyPlan::EU868);
        device.join_otaa().unwrap();

        // Send several uplinks, verify counter increments
        for i in 0u32..5 {
            device.send_uplink(1, &[i as u8], false).unwrap();
            assert_eq!(device.fcnt_up(), i + 1);
        }

        // Downlink counter tracks separately
        device.simulate_downlink(1, vec![0x01]);
        device.receive_downlink().unwrap();
        assert_eq!(device.fcnt_down(), 1);
        assert_eq!(device.fcnt_up(), 5);
    }

    #[test]
    fn s21_8_adr_adjusts_data_rate() {
        let config = LoRaConfig::new(FrequencyPlan::EU868);
        let mut device = LoRaDevice::new(config);
        device.join_otaa().unwrap();

        // Initial DR is SF12BW125 (index 0)
        assert_eq!(device.current_data_rate(), DataRate::SF12BW125);

        // Simulate good signal: link margin >> target SNR
        let response = device.process_mac_command(MacCommand::LinkCheck {
            margin: Some(20),
            gw_count: Some(3),
        });
        assert!(response.is_none());

        // ADR should increase data rate
        let changed = device.adr.adjust_data_rate();
        assert!(changed);
        assert!(device.adr.data_rate().index() > 0);
    }

    #[test]
    fn s21_9_frequency_plan_properties() {
        assert_eq!(FrequencyPlan::EU868.channel_count(), 16);
        assert_eq!(FrequencyPlan::US915.channel_count(), 72);
        assert_eq!(FrequencyPlan::EU868.max_tx_power_dbm(), 16);
        assert_eq!(FrequencyPlan::US915.max_tx_power_dbm(), 30);
        assert_eq!(FrequencyPlan::EU868.data_rate_count(), 7);
        assert_eq!(FrequencyPlan::KR920.data_rate_count(), 6);
        assert_eq!(FrequencyPlan::EU868.default_rx2_frequency(), 869_525_000);
    }

    #[test]
    fn s21_10_invalid_port_and_payload() {
        let mut device = lorawan_new(FrequencyPlan::EU868);
        device.join_otaa().unwrap();

        // Port 0 is reserved
        let err = device.send_uplink(0, &[0x01], false).unwrap_err();
        assert!(matches!(err, LoRaError::InvalidPort { port: 0 }));

        // Port 224+ is reserved
        let err = device.send_uplink(224, &[0x01], false).unwrap_err();
        assert!(matches!(err, LoRaError::InvalidPort { port: 224 }));

        // Payload too large for SF12
        let big_payload = vec![0u8; MAX_PAYLOAD_SF12 + 1];
        let err = device.send_uplink(1, &big_payload, false).unwrap_err();
        assert!(matches!(err, LoRaError::PayloadTooLarge { .. }));
    }

    // ── Sprint 22: Class B/C & Integration Tests ──

    #[test]
    fn s22_1_enable_class_b() {
        let mut device = lorawan_new(FrequencyPlan::EU868);
        device.join_otaa().unwrap();
        assert_eq!(device.device_class(), DeviceClass::A);

        let config = ClassBConfig::new().with_ping_slot_period(32);
        device.enable_class_b(config).unwrap();
        assert_eq!(device.device_class(), DeviceClass::B);
        assert_eq!(device.class_b_config().unwrap().ping_slot_period, 32);

        // Already in Class B
        let err = device.enable_class_b(ClassBConfig::new()).unwrap_err();
        assert!(matches!(err, LoRaError::ClassModeError { .. }));
    }

    #[test]
    fn s22_2_enable_class_c() {
        let mut device = lorawan_new(FrequencyPlan::EU868);
        device.join_otaa().unwrap();

        device.enable_class_c().unwrap();
        assert_eq!(device.device_class(), DeviceClass::C);

        // Already in Class C
        let err = device.enable_class_c().unwrap_err();
        assert!(matches!(err, LoRaError::ClassModeError { .. }));

        // Revert to Class A
        device.revert_to_class_a().unwrap();
        assert_eq!(device.device_class(), DeviceClass::A);
    }

    #[test]
    fn s22_3_class_b_not_joined_fails() {
        let mut device = lorawan_new(FrequencyPlan::EU868);

        let err = device.enable_class_b(ClassBConfig::new()).unwrap_err();
        assert_eq!(err, LoRaError::NotJoined);

        let err = device.enable_class_c().unwrap_err();
        assert_eq!(err, LoRaError::NotJoined);
    }

    #[test]
    fn s22_4_beacon_sync() {
        let mut device = lorawan_new(FrequencyPlan::EU868);
        device.join_otaa().unwrap();
        device.enable_class_b(ClassBConfig::new()).unwrap();

        let beacon = BeaconInfo {
            gps_time: 1_300_000_000,
            gateway_coordinates: Some((52370, 4900)),
            info_desc: 0x01,
        };
        device.simulate_beacon(beacon);

        let last = device.last_beacon().unwrap();
        assert_eq!(last.gps_time, 1_300_000_000);
        assert_eq!(last.gateway_coordinates, Some((52370, 4900)));

        // Event recorded
        let beacon_events: Vec<_> = device
            .events()
            .iter()
            .filter(|e| matches!(e, LoRaEvent::BeaconReceived { .. }))
            .collect();
        assert_eq!(beacon_events.len(), 1);
    }

    #[test]
    fn s22_5_multicast_group_join_leave() {
        let mut device = lorawan_new(FrequencyPlan::EU868);
        device.join_otaa().unwrap();

        let group = MulticastGroup::new(0x01020304, [0xAA; 16], [0xBB; 16]);
        device.join_multicast_group(group).unwrap();
        assert_eq!(device.multicast_groups().len(), 1);
        assert!(device.multicast_groups().contains_key(&0x01020304));

        // Duplicate join fails
        let dup = MulticastGroup::new(0x01020304, [0xCC; 16], [0xDD; 16]);
        let err = device.join_multicast_group(dup).unwrap_err();
        assert!(matches!(err, LoRaError::MulticastError { .. }));

        // Leave
        device.leave_multicast_group(0x01020304).unwrap();
        assert_eq!(device.multicast_groups().len(), 0);

        // Leave non-existent fails
        let err = device.leave_multicast_group(0xDEADBEEF).unwrap_err();
        assert!(matches!(err, LoRaError::MulticastError { .. }));
    }

    #[test]
    fn s22_6_multicast_group_limit() {
        let mut device = lorawan_new(FrequencyPlan::EU868);
        device.join_otaa().unwrap();

        for i in 0..MAX_MULTICAST_GROUPS {
            let group = MulticastGroup::new(i as u32, [i as u8; 16], [i as u8; 16]);
            device.join_multicast_group(group).unwrap();
        }

        let extra = MulticastGroup::new(0xFF, [0xFF; 16], [0xFF; 16]);
        let err = device.join_multicast_group(extra).unwrap_err();
        assert!(matches!(err, LoRaError::MulticastError { .. }));
    }

    #[test]
    fn s22_7_mac_commands() {
        let mut device = lorawan_new(FrequencyPlan::EU868);
        device.join_otaa().unwrap();

        // LinkCheck response
        let resp = device.process_mac_command(MacCommand::LinkCheck {
            margin: Some(10),
            gw_count: Some(2),
        });
        assert!(resp.is_none());
        assert_eq!(device.adr().link_margin_db(), 10);

        // LinkCheck request (no data)
        let resp = device.process_mac_command(MacCommand::LinkCheck {
            margin: None,
            gw_count: None,
        });
        assert!(resp.is_some());

        // DutyCycleReq
        device.process_mac_command(MacCommand::DutyCycleReq { max_duty_cycle: 4 });
        // 3600000 / 2^4 = 225000
        assert_eq!(device.duty_cycle().duty_cycle_limit_ms(), 225_000);

        // NewChannel
        let resp = device.process_mac_command(MacCommand::NewChannel {
            ch_index: 3,
            frequency: 867_100_000,
            min_dr: 0,
            max_dr: 5,
        });
        assert!(resp.is_none());
    }

    #[test]
    fn s22_8_duty_cycle_enforcement() {
        let mut device = lorawan_new(FrequencyPlan::EU868);
        device.join_otaa().unwrap();

        // Use a generous duty cycle limit that accommodates first uplink
        device.duty_cycle = DutyCycleTracker::new(500); // 500 ms limit

        // First uplink should succeed (airtime ~352ms at SF12)
        device.send_uplink(1, &[0x01], false).unwrap();

        // Exhaust remaining duty cycle budget
        device.duty_cycle.record_transmission(200);

        // Next uplink should fail due to duty cycle
        let err = device.send_uplink(1, &[0x02], false).unwrap_err();
        assert!(matches!(err, LoRaError::DutyCycleExceeded { .. }));

        // Reset window allows transmission again
        device.duty_cycle.reset_window();
        device.send_uplink(1, &[0x03], false).unwrap();
    }

    #[test]
    fn s22_9_sleep_wake_cycle() {
        let mut device = lorawan_new(FrequencyPlan::EU868);
        device.join_otaa().unwrap();
        assert_eq!(device.state(), LoRaDeviceState::Joined);

        device.sleep();
        assert_eq!(device.state(), LoRaDeviceState::Sleep);

        device.wake();
        assert_eq!(device.state(), LoRaDeviceState::Joined);

        // Wake an idle device
        let mut idle_device = lorawan_new(FrequencyPlan::EU868);
        idle_device.sleep();
        idle_device.wake();
        assert_eq!(idle_device.state(), LoRaDeviceState::Idle);
    }

    #[test]
    fn s22_10_iot_stack_integration() {
        let mut stack = IotStack::new(IotProtocols::all());
        assert!(stack.protocols().lorawan);

        // Attach LoRaWAN device
        let mut device = lorawan_new(FrequencyPlan::EU868);
        device.join_otaa().unwrap();
        stack.attach_lorawan(device).unwrap();

        assert!(stack.lorawan_device().is_some());
        assert_eq!(stack.event_log().len(), 1);

        // Send data through the stack
        stack.send_data(&[0x42, 0x43]).unwrap();
        assert_eq!(stack.event_log().len(), 2);
        assert!(stack.event_log()[1].contains("LoRaWAN"));

        // Stack with LoRaWAN disabled rejects attachment
        let mut no_lora = IotStack::new(IotProtocols::none());
        let device2 = lorawan_new(FrequencyPlan::US915);
        let err = no_lora.attach_lorawan(device2).unwrap_err();
        assert!(matches!(err, LoRaError::InvalidConfig { .. }));
    }
}
