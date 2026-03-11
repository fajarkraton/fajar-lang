//! OTA (Over-The-Air) firmware and model update for Fajar Lang.
//!
//! Provides simulation stubs for firmware update, verification, A/B
//! partitioning, rollback, and ML model hot-swap. Models the ESP-IDF
//! OTA API without requiring actual hardware or network connectivity.
//!
//! # API Groups
//!
//! - **Check**: query update server for new firmware versions
//! - **Download**: fetch firmware binary (simulation)
//! - **Verify**: validate firmware integrity via checksum
//! - **Flash**: write firmware to inactive partition (A/B scheme)
//! - **Rollback**: revert to previous partition on failure
//! - **Model update**: hot-swap ML model files
//!
//! # A/B Partition Scheme
//!
//! ```text
//! ┌────────────┐  ┌────────────┐
//! │ Partition A │  │ Partition B │
//! │  (active)   │  │ (inactive)  │
//! └────────────┘  └────────────┘
//!       ↑               ↑
//!     boot            OTA target
//! ```
//!
//! After flashing, the inactive partition becomes the boot target.
//! If validation fails, `ota_rollback()` reverts to the previous partition.
//!
//! # Usage
//!
//! ```rust
//! use fajar_lang::iot::ota::*;
//!
//! let mut manager = OtaManager::new(OtaConfig::new("https://ota.example.com"));
//! if let Some(info) = manager.check_update().unwrap() {
//!     let firmware = manager.download_firmware(&info.download_url).unwrap();
//!     manager.verify(&firmware, &info.sha256).unwrap();
//!     manager.flash(&firmware).unwrap();
//! }
//! ```

use thiserror::Error;

// ═══════════════════════════════════════════════════════════════════════
// Error types
// ═══════════════════════════════════════════════════════════════════════

/// Errors from OTA operations.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum OtaError {
    /// Update check failed.
    #[error("update check failed: {reason}")]
    CheckFailed {
        /// Reason for failure.
        reason: String,
    },

    /// Firmware download failed.
    #[error("download failed: {reason}")]
    DownloadFailed {
        /// Reason for failure.
        reason: String,
    },

    /// Firmware verification failed (checksum mismatch).
    #[error("verification failed: expected {expected}, got {actual}")]
    VerificationFailed {
        /// Expected checksum.
        expected: String,
        /// Actual checksum.
        actual: String,
    },

    /// Flash write failed.
    #[error("flash failed: {reason}")]
    FlashFailed {
        /// Reason for failure.
        reason: String,
    },

    /// Rollback failed (no previous partition).
    #[error("rollback failed: {reason}")]
    RollbackFailed {
        /// Reason for failure.
        reason: String,
    },

    /// Model update failed.
    #[error("model update failed: {reason}")]
    ModelUpdateFailed {
        /// Reason for failure.
        reason: String,
    },

    /// Invalid URL.
    #[error("invalid URL: {reason}")]
    InvalidUrl {
        /// Reason the URL is invalid.
        reason: String,
    },

    /// Firmware too large for partition.
    #[error("firmware too large: {size} bytes (partition max: {max})")]
    FirmwareTooLarge {
        /// Firmware size.
        size: u64,
        /// Maximum partition size.
        max: u64,
    },

    /// No update available.
    #[error("no update available")]
    NoUpdateAvailable,
}

// ═══════════════════════════════════════════════════════════════════════
// Constants
// ═══════════════════════════════════════════════════════════════════════

/// Default update check interval in seconds (1 hour).
pub const DEFAULT_CHECK_INTERVAL: u32 = 3600;

/// Default partition size in bytes (1 MB).
pub const DEFAULT_PARTITION_SIZE: u64 = 1024 * 1024;

// ═══════════════════════════════════════════════════════════════════════
// Data types
// ═══════════════════════════════════════════════════════════════════════

/// OTA update configuration.
#[derive(Debug, Clone)]
pub struct OtaConfig {
    /// Update server URL.
    pub server_url: String,
    /// Interval between automatic update checks (seconds).
    pub check_interval_secs: u32,
    /// Whether to verify firmware signature/checksum.
    pub verify_signature: bool,
    /// Whether to rollback on boot validation failure.
    pub rollback_on_failure: bool,
}

impl OtaConfig {
    /// Creates a new OTA configuration with default settings.
    pub fn new(server_url: &str) -> Self {
        Self {
            server_url: server_url.to_string(),
            check_interval_secs: DEFAULT_CHECK_INTERVAL,
            verify_signature: true,
            rollback_on_failure: true,
        }
    }

    /// Sets the check interval.
    pub fn with_check_interval(mut self, secs: u32) -> Self {
        self.check_interval_secs = secs;
        self
    }

    /// Sets whether to verify firmware signatures.
    pub fn with_verify(mut self, verify: bool) -> Self {
        self.verify_signature = verify;
        self
    }

    /// Sets whether to rollback on failure.
    pub fn with_rollback(mut self, rollback: bool) -> Self {
        self.rollback_on_failure = rollback;
        self
    }

    /// Validates the configuration.
    pub fn validate(&self) -> Result<(), OtaError> {
        if self.server_url.is_empty() {
            return Err(OtaError::InvalidUrl {
                reason: "server URL cannot be empty".to_string(),
            });
        }
        if !self.server_url.starts_with("http://") && !self.server_url.starts_with("https://") {
            return Err(OtaError::InvalidUrl {
                reason: "server URL must start with http:// or https://".to_string(),
            });
        }
        Ok(())
    }
}

/// Information about an available firmware update.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateInfo {
    /// Firmware version string (semver).
    pub version: String,
    /// Firmware binary size in bytes.
    pub size_bytes: u64,
    /// SHA-256 checksum of the firmware binary (hex string).
    pub sha256: String,
    /// URL to download the firmware binary.
    pub download_url: String,
    /// Human-readable release notes.
    pub release_notes: String,
}

impl UpdateInfo {
    /// Creates a new update info entry.
    pub fn new(
        version: &str,
        size_bytes: u64,
        sha256: &str,
        download_url: &str,
        release_notes: &str,
    ) -> Self {
        Self {
            version: version.to_string(),
            size_bytes,
            sha256: sha256.to_string(),
            download_url: download_url.to_string(),
            release_notes: release_notes.to_string(),
        }
    }
}

/// A/B partition identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OtaPartition {
    /// First partition.
    A,
    /// Second partition.
    B,
}

impl OtaPartition {
    /// Returns the other (inactive) partition.
    pub fn other(self) -> Self {
        match self {
            OtaPartition::A => OtaPartition::B,
            OtaPartition::B => OtaPartition::A,
        }
    }
}

impl std::fmt::Display for OtaPartition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OtaPartition::A => write!(f, "A"),
            OtaPartition::B => write!(f, "B"),
        }
    }
}

/// Version manifest describing available firmware and model updates.
///
/// Designed for JSON serialization from the update server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionManifest {
    /// Firmware version.
    pub version: String,
    /// Firmware download URL.
    pub firmware_url: String,
    /// ML model download URL (optional).
    pub model_url: Option<String>,
    /// Minimum hardware version required.
    pub min_hw_version: String,
    /// Human-readable changelog.
    pub changelog: String,
}

impl VersionManifest {
    /// Creates a new version manifest.
    pub fn new(version: &str, firmware_url: &str, min_hw_version: &str, changelog: &str) -> Self {
        Self {
            version: version.to_string(),
            firmware_url: firmware_url.to_string(),
            model_url: None,
            min_hw_version: min_hw_version.to_string(),
            changelog: changelog.to_string(),
        }
    }

    /// Sets the ML model URL.
    pub fn with_model_url(mut self, url: &str) -> Self {
        self.model_url = Some(url.to_string());
        self
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Checksum computation (simple simulation)
// ═══════════════════════════════════════════════════════════════════════

/// Computes a simple checksum of the given data.
///
/// This is a simulation-only hash function. On real hardware, this would
/// use SHA-256 from a crypto library. We use a deterministic hash that
/// is sufficient for testing verification logic.
pub fn compute_checksum(data: &[u8]) -> String {
    // Simple simulation: use a basic hash combining all bytes.
    // We produce a 64-char hex string to mimic SHA-256 output format.
    let mut h: [u64; 4] = [
        0x6a09_e667_f3bc_c908,
        0xbb67_ae85_84ca_a73b,
        0x3c6e_f372_fe94_f82b,
        0xa54f_f53a_5f1d_36f1,
    ];

    for (i, &byte) in data.iter().enumerate() {
        let idx = i % 4;
        h[idx] = h[idx].wrapping_mul(31).wrapping_add(byte as u64);
        // Mix between lanes
        let next = (idx + 1) % 4;
        h[next] ^= h[idx].rotate_left(7);
    }

    format!("{:016x}{:016x}{:016x}{:016x}", h[0], h[1], h[2], h[3])
}

// ═══════════════════════════════════════════════════════════════════════
// OTA manager (simulation)
// ═══════════════════════════════════════════════════════════════════════

/// Simulated OTA update manager.
///
/// Models the ESP-IDF OTA lifecycle:
/// `check_update()` -> `download_firmware()` -> `verify()` -> `flash()`.
///
/// Supports A/B partitioning with rollback.
#[derive(Debug)]
pub struct OtaManager {
    /// Manager configuration.
    config: OtaConfig,
    /// Currently active partition.
    active_partition: OtaPartition,
    /// Firmware version on partition A.
    version_a: String,
    /// Firmware version on partition B.
    version_b: Option<String>,
    /// Simulated partition data for A.
    data_a: Vec<u8>,
    /// Simulated partition data for B.
    data_b: Vec<u8>,
    /// Maximum partition size in bytes.
    partition_size: u64,
    /// Current running firmware version.
    current_version: String,
    /// Model file path (simulation).
    model_path: Option<String>,
}

impl OtaManager {
    /// Creates a new OTA manager.
    pub fn new(config: OtaConfig) -> Self {
        Self {
            config,
            active_partition: OtaPartition::A,
            version_a: "1.0.0".to_string(),
            version_b: None,
            data_a: Vec::new(),
            data_b: Vec::new(),
            partition_size: DEFAULT_PARTITION_SIZE,
            current_version: "1.0.0".to_string(),
            model_path: None,
        }
    }

    /// Returns the active partition.
    pub fn active_partition(&self) -> OtaPartition {
        self.active_partition
    }

    /// Returns the current firmware version.
    pub fn current_version(&self) -> &str {
        &self.current_version
    }

    /// Returns the model path, if set.
    pub fn model_path(&self) -> Option<&str> {
        self.model_path.as_deref()
    }

    /// Returns the manager configuration.
    pub fn config(&self) -> &OtaConfig {
        &self.config
    }

    /// Sets the maximum partition size.
    pub fn set_partition_size(&mut self, size: u64) {
        self.partition_size = size;
    }

    /// Checks the update server for available updates (simulation).
    ///
    /// Returns `Ok(Some(info))` if an update is available,
    /// `Ok(None)` if already up to date.
    pub fn check_update(&self) -> Result<Option<UpdateInfo>, OtaError> {
        self.config.validate()?;

        // Simulation: always report a newer version available
        let new_version = increment_version(&self.current_version);
        let simulated_firmware = generate_simulated_firmware(&new_version);
        let checksum = compute_checksum(&simulated_firmware);

        Ok(Some(UpdateInfo::new(
            &new_version,
            simulated_firmware.len() as u64,
            &checksum,
            &format!("{}/firmware/{}.bin", self.config.server_url, new_version),
            &format!("Update from {} to {}", self.current_version, new_version),
        )))
    }

    /// Downloads firmware from the given URL (simulation).
    ///
    /// Returns simulated firmware bytes.
    pub fn download_firmware(&self, _url: &str) -> Result<Vec<u8>, OtaError> {
        self.config.validate()?;

        // Simulation: generate deterministic firmware bytes
        let new_version = increment_version(&self.current_version);
        Ok(generate_simulated_firmware(&new_version))
    }

    /// Verifies firmware integrity by comparing checksums.
    pub fn verify(&self, firmware: &[u8], expected_sha256: &str) -> Result<(), OtaError> {
        let actual = compute_checksum(firmware);
        if actual != expected_sha256 {
            return Err(OtaError::VerificationFailed {
                expected: expected_sha256.to_string(),
                actual,
            });
        }
        Ok(())
    }

    /// Flashes firmware to the inactive partition (simulation).
    ///
    /// After flashing, the inactive partition becomes active on next boot.
    pub fn flash(&mut self, firmware: &[u8]) -> Result<(), OtaError> {
        if firmware.len() as u64 > self.partition_size {
            return Err(OtaError::FirmwareTooLarge {
                size: firmware.len() as u64,
                max: self.partition_size,
            });
        }
        if firmware.is_empty() {
            return Err(OtaError::FlashFailed {
                reason: "firmware is empty".to_string(),
            });
        }

        let target = self.active_partition.other();
        let new_version = increment_version(&self.current_version);

        match target {
            OtaPartition::A => {
                self.data_a = firmware.to_vec();
                self.version_a = new_version.clone();
            }
            OtaPartition::B => {
                self.data_b = firmware.to_vec();
                self.version_b = Some(new_version.clone());
            }
        }

        // Switch active partition
        self.active_partition = target;
        self.current_version = new_version;

        Ok(())
    }

    /// Rolls back to the previous partition (simulation).
    pub fn rollback(&mut self) -> Result<(), OtaError> {
        let prev = self.active_partition.other();
        let has_valid_prev = match prev {
            OtaPartition::A => !self.data_a.is_empty() || !self.version_a.is_empty(),
            OtaPartition::B => self.version_b.is_some(),
        };

        if !has_valid_prev {
            return Err(OtaError::RollbackFailed {
                reason: format!("no valid firmware on partition {prev}"),
            });
        }

        // Restore previous version
        let prev_version = match prev {
            OtaPartition::A => self.version_a.clone(),
            OtaPartition::B => self.version_b.clone().unwrap_or_default(),
        };

        self.active_partition = prev;
        self.current_version = prev_version;

        Ok(())
    }

    /// Updates the ML model file (simulation).
    ///
    /// Downloads a new model and replaces the existing one.
    pub fn update_model(&mut self, _url: &str, model_path: &str) -> Result<(), OtaError> {
        if model_path.is_empty() {
            return Err(OtaError::ModelUpdateFailed {
                reason: "model path cannot be empty".to_string(),
            });
        }

        // Simulation: just record the model path as updated
        self.model_path = Some(model_path.to_string());
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Free-standing convenience functions
// ═══════════════════════════════════════════════════════════════════════

/// Checks the update server for available firmware (simulation stub).
pub fn ota_check_update(manager: &OtaManager) -> Result<Option<UpdateInfo>, OtaError> {
    manager.check_update()
}

/// Downloads firmware from a URL (simulation stub).
pub fn ota_download_firmware(manager: &OtaManager, url: &str) -> Result<Vec<u8>, OtaError> {
    manager.download_firmware(url)
}

/// Verifies firmware integrity.
pub fn ota_verify(
    manager: &OtaManager,
    firmware: &[u8],
    expected_sha256: &str,
) -> Result<(), OtaError> {
    manager.verify(firmware, expected_sha256)
}

/// Flashes firmware to the inactive partition (simulation stub).
pub fn ota_flash(manager: &mut OtaManager, firmware: &[u8]) -> Result<(), OtaError> {
    manager.flash(firmware)
}

/// Rolls back to the previous partition.
pub fn ota_rollback(manager: &mut OtaManager) -> Result<(), OtaError> {
    manager.rollback()
}

/// Updates an ML model file (simulation stub).
pub fn ota_update_model(
    manager: &mut OtaManager,
    url: &str,
    model_path: &str,
) -> Result<(), OtaError> {
    manager.update_model(url, model_path)
}

// ═══════════════════════════════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════════════════════════════

/// Increments a semver-style version string (patch bump).
///
/// "1.2.3" -> "1.2.4", "1.0.0" -> "1.0.1"
fn increment_version(version: &str) -> String {
    let parts: Vec<&str> = version.split('.').collect();
    if parts.len() != 3 {
        return format!("{version}.1");
    }
    let major = parts[0];
    let minor = parts[1];
    let patch: u32 = parts[2].parse().unwrap_or(0);
    format!("{major}.{minor}.{}", patch + 1)
}

/// Generates simulated firmware bytes for a given version.
fn generate_simulated_firmware(version: &str) -> Vec<u8> {
    // Create deterministic but version-specific firmware data
    let mut data = Vec::with_capacity(256);
    // Header: magic bytes
    data.extend_from_slice(b"FJFW");
    // Version string (null-padded to 16 bytes)
    let ver_bytes = version.as_bytes();
    let copy_len = ver_bytes.len().min(16);
    data.extend_from_slice(&ver_bytes[..copy_len]);
    data.resize(data.len() + (16 - copy_len), 0);
    // Simulated code section (deterministic pattern)
    for i in 0u8..236 {
        data.push(i.wrapping_mul(7).wrapping_add(version.len() as u8));
    }
    data
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ota_config_validation() {
        let config = OtaConfig::new("https://ota.example.com");
        assert!(config.validate().is_ok());
        assert!(config.verify_signature);
        assert!(config.rollback_on_failure);

        let bad = OtaConfig::new("");
        assert!(bad.validate().is_err());

        let bad = OtaConfig::new("ftp://wrong");
        assert!(bad.validate().is_err());
    }

    #[test]
    fn ota_check_update_returns_info() {
        let config = OtaConfig::new("https://ota.example.com");
        let manager = OtaManager::new(config);

        let info = manager.check_update().unwrap();
        assert!(info.is_some());

        let info = info.unwrap();
        assert_eq!(info.version, "1.0.1");
        assert!(!info.sha256.is_empty());
        assert!(info.download_url.contains("ota.example.com"));
    }

    #[test]
    fn ota_download_firmware_returns_bytes() {
        let config = OtaConfig::new("https://ota.example.com");
        let manager = OtaManager::new(config);

        let firmware = manager
            .download_firmware("https://ota.example.com/firmware/1.0.1.bin")
            .unwrap();
        assert!(!firmware.is_empty());
        // Check magic header
        assert_eq!(&firmware[..4], b"FJFW");
    }

    #[test]
    fn ota_verify_correct_checksum() {
        let data = b"test firmware data";
        let checksum = compute_checksum(data);

        let config = OtaConfig::new("https://ota.example.com");
        let manager = OtaManager::new(config);
        assert!(manager.verify(data, &checksum).is_ok());
    }

    #[test]
    fn ota_verify_wrong_checksum() {
        let data = b"test firmware data";

        let config = OtaConfig::new("https://ota.example.com");
        let manager = OtaManager::new(config);
        let err = manager.verify(data, "0000000000000000").unwrap_err();
        assert!(matches!(err, OtaError::VerificationFailed { .. }));
    }

    #[test]
    fn ota_flash_switches_partition() {
        let config = OtaConfig::new("https://ota.example.com");
        let mut manager = OtaManager::new(config);
        assert_eq!(manager.active_partition(), OtaPartition::A);
        assert_eq!(manager.current_version(), "1.0.0");

        let firmware = vec![0xDE, 0xAD, 0xBE, 0xEF];
        manager.flash(&firmware).unwrap();

        assert_eq!(manager.active_partition(), OtaPartition::B);
        assert_eq!(manager.current_version(), "1.0.1");
    }

    #[test]
    fn ota_flash_empty_firmware_fails() {
        let config = OtaConfig::new("https://ota.example.com");
        let mut manager = OtaManager::new(config);

        let err = manager.flash(&[]).unwrap_err();
        assert!(matches!(err, OtaError::FlashFailed { .. }));
    }

    #[test]
    fn ota_flash_too_large_fails() {
        let config = OtaConfig::new("https://ota.example.com");
        let mut manager = OtaManager::new(config);
        manager.set_partition_size(10); // 10 bytes max

        let firmware = vec![0u8; 20];
        let err = manager.flash(&firmware).unwrap_err();
        assert!(matches!(err, OtaError::FirmwareTooLarge { .. }));
    }

    #[test]
    fn ota_rollback_restores_previous() {
        let config = OtaConfig::new("https://ota.example.com");
        let mut manager = OtaManager::new(config);

        // Flash to partition B
        manager.flash(&[0x01, 0x02]).unwrap();
        assert_eq!(manager.active_partition(), OtaPartition::B);
        assert_eq!(manager.current_version(), "1.0.1");

        // Rollback to partition A
        manager.rollback().unwrap();
        assert_eq!(manager.active_partition(), OtaPartition::A);
        assert_eq!(manager.current_version(), "1.0.0");
    }

    #[test]
    fn ota_update_model_succeeds() {
        let config = OtaConfig::new("https://ota.example.com");
        let mut manager = OtaManager::new(config);

        manager
            .update_model(
                "https://models.example.com/v2.onnx",
                "/models/inference.onnx",
            )
            .unwrap();
        assert_eq!(manager.model_path(), Some("/models/inference.onnx"));
    }

    #[test]
    fn ota_update_model_empty_path_fails() {
        let config = OtaConfig::new("https://ota.example.com");
        let mut manager = OtaManager::new(config);

        let err = manager
            .update_model("https://models.example.com/v2.onnx", "")
            .unwrap_err();
        assert!(matches!(err, OtaError::ModelUpdateFailed { .. }));
    }

    #[test]
    fn ota_full_update_cycle() {
        let config = OtaConfig::new("https://ota.example.com");
        let mut manager = OtaManager::new(config);

        // 1. Check for update
        let info = manager.check_update().unwrap().unwrap();
        assert_eq!(info.version, "1.0.1");

        // 2. Download
        let firmware = manager.download_firmware(&info.download_url).unwrap();
        assert!(!firmware.is_empty());

        // 3. Verify
        manager.verify(&firmware, &info.sha256).unwrap();

        // 4. Flash
        manager.flash(&firmware).unwrap();
        assert_eq!(manager.current_version(), "1.0.1");
        assert_eq!(manager.active_partition(), OtaPartition::B);
    }

    #[test]
    fn ota_partition_other() {
        assert_eq!(OtaPartition::A.other(), OtaPartition::B);
        assert_eq!(OtaPartition::B.other(), OtaPartition::A);
    }

    #[test]
    fn checksum_deterministic() {
        let data = b"hello fajar lang";
        let c1 = compute_checksum(data);
        let c2 = compute_checksum(data);
        assert_eq!(c1, c2);
        assert_eq!(c1.len(), 64); // 4 * 16 hex chars

        // Different data produces different checksum
        let c3 = compute_checksum(b"different data");
        assert_ne!(c1, c3);
    }

    #[test]
    fn version_increment() {
        assert_eq!(increment_version("1.0.0"), "1.0.1");
        assert_eq!(increment_version("1.2.3"), "1.2.4");
        assert_eq!(increment_version("0.7.9"), "0.7.10");
    }

    #[test]
    fn version_manifest_creation() {
        let manifest = VersionManifest::new(
            "1.1.0",
            "https://ota.example.com/1.1.0.bin",
            "rev3",
            "Bug fixes and improvements",
        )
        .with_model_url("https://models.example.com/v3.onnx");

        assert_eq!(manifest.version, "1.1.0");
        assert!(manifest.model_url.is_some());
        assert_eq!(manifest.min_hw_version, "rev3");
    }
}
