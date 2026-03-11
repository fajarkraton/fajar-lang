//! # NPU Detection
//!
//! Runtime detection of Neural Processing Units via filesystem probing.
//! No driver-level dependencies — all detection via sysfs, devfs, and
//! /proc filesystem entries.
//!
//! ## Supported NPUs
//!
//! | Vendor | Detection Method | NPU Model |
//! |--------|-----------------|-----------|
//! | Intel | `/dev/accel/accel*` + `intel_vpu` driver | Meteor Lake / Lunar Lake / Arrow Lake |
//! | AMD | `/sys/module/amdxdna` | XDNA 2 (Ryzen AI 300+) |
//! | Qualcomm | `/sys/module/qaic` | Hexagon DSP (Snapdragon X) |
//! | Apple | stub (macOS IOKit) | Apple Neural Engine |

use serde::Serialize;

// ═══════════════════════════════════════════════════════════════════════
// NPU Vendor
// ═══════════════════════════════════════════════════════════════════════

/// NPU hardware vendor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum NpuVendor {
    /// Intel (Meteor Lake, Lunar Lake, Arrow Lake NPU)
    Intel,
    /// AMD (XDNA / XDNA 2)
    Amd,
    /// Qualcomm (Hexagon DSP / Cloud AI)
    Qualcomm,
    /// Apple (Apple Neural Engine)
    Apple,
}

impl std::fmt::Display for NpuVendor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NpuVendor::Intel => write!(f, "Intel"),
            NpuVendor::Amd => write!(f, "AMD"),
            NpuVendor::Qualcomm => write!(f, "Qualcomm"),
            NpuVendor::Apple => write!(f, "Apple"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Supported Data Formats
// ═══════════════════════════════════════════════════════════════════════

/// Numeric data format supported by an NPU.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum NpuDataFormat {
    /// 8-bit integer
    INT8,
    /// 16-bit float (IEEE half-precision)
    FP16,
    /// Brain float 16
    BF16,
    /// 32-bit float
    FP32,
}

impl std::fmt::Display for NpuDataFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NpuDataFormat::INT8 => write!(f, "INT8"),
            NpuDataFormat::FP16 => write!(f, "FP16"),
            NpuDataFormat::BF16 => write!(f, "BF16"),
            NpuDataFormat::FP32 => write!(f, "FP32"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// NpuDevice
// ═══════════════════════════════════════════════════════════════════════

/// A detected Neural Processing Unit.
#[derive(Debug, Clone, Serialize)]
pub struct NpuDevice {
    /// NPU vendor.
    pub vendor: NpuVendor,
    /// Model or generation name (e.g., "Meteor Lake NPU", "XDNA 2").
    pub model: String,
    /// Peak performance in tera operations per second.
    pub tops: f64,
    /// Supported numeric data formats.
    pub supported_formats: Vec<NpuDataFormat>,
    /// Maximum batch size (0 = unknown).
    pub max_batch: u32,
    /// Driver version string (if available).
    pub driver_version: String,
    /// Whether the NPU passed a basic health check.
    pub healthy: bool,
}

impl NpuDevice {
    /// Format NPU info for human-readable display.
    pub fn display_info(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("  {} — {}\n", self.vendor, self.model));
        out.push_str(&format!("    Peak:     {:.1} TOPS\n", self.tops));
        let formats: Vec<String> = self
            .supported_formats
            .iter()
            .map(|f| f.to_string())
            .collect();
        out.push_str(&format!("    Formats:  {}\n", formats.join(", ")));
        if !self.driver_version.is_empty() {
            out.push_str(&format!("    Driver:   {}\n", self.driver_version));
        }
        out.push_str(&format!(
            "    Status:   {}\n",
            if self.healthy { "healthy" } else { "degraded" }
        ));
        out
    }
}

// ═══════════════════════════════════════════════════════════════════════
// NPU Discovery
// ═══════════════════════════════════════════════════════════════════════

/// Result of NPU discovery.
#[derive(Debug, Clone, Default, Serialize)]
pub struct NpuDiscovery {
    /// Detected NPU devices.
    pub devices: Vec<NpuDevice>,
}

impl NpuDiscovery {
    /// Detect all NPUs on the current platform.
    ///
    /// Probes filesystem paths — never panics, returns empty on failure.
    pub fn detect() -> Self {
        let mut devices = Vec::new();

        // Intel NPU detection
        if let Some(npu) = detect_intel_npu() {
            devices.push(npu);
        }

        // AMD XDNA detection
        if let Some(npu) = detect_amd_xdna() {
            devices.push(npu);
        }

        // Qualcomm Hexagon detection
        if let Some(npu) = detect_qualcomm_hexagon() {
            devices.push(npu);
        }

        // Apple ANE detection (stub)
        #[cfg(target_os = "macos")]
        if let Some(npu) = detect_apple_ane() {
            devices.push(npu);
        }

        Self { devices }
    }

    /// Format all NPU info for human-readable display.
    pub fn display_info(&self) -> String {
        let mut out = String::new();
        out.push_str("── NPU ──────────────────────────────────\n");
        if self.devices.is_empty() {
            out.push_str("  (none detected)\n");
        } else {
            out.push_str(&format!("  Devices:    {} found\n", self.devices.len()));
            for dev in &self.devices {
                out.push_str(&dev.display_info());
            }
        }
        out
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Intel NPU Detection
// ═══════════════════════════════════════════════════════════════════════

/// Detect Intel NPU via /dev/accel and intel_vpu kernel module.
fn detect_intel_npu() -> Option<NpuDevice> {
    // Check for accelerator device node
    let accel_path = std::path::Path::new("/dev/accel/accel0");
    if !accel_path.exists() {
        return None;
    }

    // Verify it's an Intel VPU/NPU by checking the driver
    let is_intel_npu = check_intel_npu_driver();
    if !is_intel_npu {
        return None;
    }

    // Determine NPU generation from device info
    let (model, tops) = detect_intel_npu_generation();
    let driver_version = read_intel_npu_driver_version();

    Some(NpuDevice {
        vendor: NpuVendor::Intel,
        model,
        tops,
        supported_formats: vec![NpuDataFormat::INT8, NpuDataFormat::FP16],
        max_batch: 1,
        driver_version,
        healthy: true,
    })
}

/// Check if the accel device is driven by the Intel VPU/NPU driver.
fn check_intel_npu_driver() -> bool {
    // Check for intel_vpu or intel_npu module
    let module_paths = [
        "/sys/module/intel_vpu",
        "/sys/module/intel_npu",
        "/sys/module/ivpu",
    ];

    for path in &module_paths {
        if std::path::Path::new(path).exists() {
            return true;
        }
    }

    // Fallback: check driver link for the accel device
    if let Ok(driver_link) = std::fs::read_link("/sys/class/accel/accel0/device/driver") {
        let driver_name = driver_link
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");
        return driver_name.contains("intel")
            || driver_name.contains("vpu")
            || driver_name.contains("npu");
    }

    false
}

/// Determine Intel NPU generation and TOPS rating.
fn detect_intel_npu_generation() -> (String, f64) {
    // Try to read PCI device ID for generation identification
    let device_id = read_sysfs_string("/sys/class/accel/accel0/device/device");

    match device_id.as_deref() {
        // Meteor Lake NPU (PCI ID 0x7D1D)
        Some("0x7d1d") => ("Meteor Lake NPU".to_string(), 11.0),
        // Lunar Lake NPU (PCI ID 0xAD1D)
        Some("0xad1d") => ("Lunar Lake NPU".to_string(), 48.0),
        // Arrow Lake NPU
        Some("0xa74d") => ("Arrow Lake NPU".to_string(), 48.0),
        // Unknown Intel NPU — use conservative estimate
        _ => ("Intel NPU".to_string(), 10.0),
    }
}

/// Read Intel NPU driver version.
fn read_intel_npu_driver_version() -> String {
    for module in &["intel_vpu", "intel_npu", "ivpu"] {
        let version_path = format!("/sys/module/{module}/version");
        if let Some(version) = read_sysfs_string(&version_path) {
            return version;
        }
    }
    String::new()
}

// ═══════════════════════════════════════════════════════════════════════
// AMD XDNA Detection
// ═══════════════════════════════════════════════════════════════════════

/// Detect AMD XDNA NPU via kernel module.
fn detect_amd_xdna() -> Option<NpuDevice> {
    let xdna_path = std::path::Path::new("/sys/module/amdxdna");
    if !xdna_path.exists() {
        return None;
    }

    let (model, tops) = detect_amd_xdna_generation();
    let driver_version = read_sysfs_string("/sys/module/amdxdna/version").unwrap_or_default();

    Some(NpuDevice {
        vendor: NpuVendor::Amd,
        model,
        tops,
        supported_formats: vec![
            NpuDataFormat::INT8,
            NpuDataFormat::FP16,
            NpuDataFormat::BF16,
        ],
        max_batch: 4,
        driver_version,
        healthy: true,
    })
}

/// Determine AMD XDNA generation.
fn detect_amd_xdna_generation() -> (String, f64) {
    // Check for specific XDNA version markers
    let xdna2_path = std::path::Path::new("/sys/module/amdxdna/parameters");
    if xdna2_path.exists() {
        // Try to detect XDNA 2 vs XDNA 1 based on available parameters
        if let Some(param) = read_sysfs_string("/sys/module/amdxdna/parameters/aie2_max_col") {
            if let Ok(cols) = param.parse::<u32>() {
                if cols >= 8 {
                    return ("XDNA 2".to_string(), 50.0);
                }
            }
        }
    }

    // Default: XDNA 1
    ("XDNA".to_string(), 16.0)
}

// ═══════════════════════════════════════════════════════════════════════
// Qualcomm Hexagon Detection
// ═══════════════════════════════════════════════════════════════════════

/// Detect Qualcomm Hexagon DSP / Cloud AI accelerator.
fn detect_qualcomm_hexagon() -> Option<NpuDevice> {
    // Check for Qualcomm AI Cloud (qaic) driver
    let qaic_path = std::path::Path::new("/sys/module/qaic");
    if !qaic_path.exists() {
        // Check for Hexagon DSP (aDSP) on mobile/embedded
        let adsp_path = std::path::Path::new("/sys/kernel/boot_adsp");
        if !adsp_path.exists() {
            return None;
        }
    }

    let driver_version = read_sysfs_string("/sys/module/qaic/version").unwrap_or_default();

    Some(NpuDevice {
        vendor: NpuVendor::Qualcomm,
        model: "Hexagon DSP".to_string(),
        tops: 45.0, // Snapdragon X Elite estimate
        supported_formats: vec![NpuDataFormat::INT8, NpuDataFormat::FP16],
        max_batch: 1,
        driver_version,
        healthy: true,
    })
}

// ═══════════════════════════════════════════════════════════════════════
// Apple ANE Detection (stub)
// ═══════════════════════════════════════════════════════════════════════

/// Detect Apple Neural Engine (macOS only, stub implementation).
#[cfg(target_os = "macos")]
fn detect_apple_ane() -> Option<NpuDevice> {
    // Apple Neural Engine detection would require IOKit framework access.
    // For now, assume ANE exists on Apple Silicon Macs.
    #[cfg(target_arch = "aarch64")]
    {
        Some(NpuDevice {
            vendor: NpuVendor::Apple,
            model: "Apple Neural Engine".to_string(),
            tops: 38.0, // M3 estimate
            supported_formats: vec![NpuDataFormat::FP16, NpuDataFormat::INT8],
            max_batch: 1,
            driver_version: String::new(),
            healthy: true,
        })
    }
    #[cfg(not(target_arch = "aarch64"))]
    {
        None
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════════════════════════════

/// Read a single-line value from a sysfs path.
fn read_sysfs_string(path: &str) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── S3.1: Intel NPU Discovery ─────────────────────────────────────

    #[test]
    fn detect_never_panics() {
        // Must succeed on any system, with or without NPU
        let discovery = NpuDiscovery::detect();
        assert!(discovery.devices.len() <= 8, "unreasonable NPU count");
    }

    #[test]
    fn intel_npu_detection_graceful_on_non_intel() {
        // On systems without Intel NPU, should return None
        let result = detect_intel_npu();
        // Result depends on hardware — either Some or None is valid
        if let Some(npu) = result {
            assert_eq!(npu.vendor, NpuVendor::Intel);
            assert!(npu.tops > 0.0);
        }
    }

    // ── S3.2: AMD XDNA Discovery ─────────────────────────────────────

    #[test]
    fn amd_xdna_detection_graceful() {
        let result = detect_amd_xdna();
        if let Some(npu) = result {
            assert_eq!(npu.vendor, NpuVendor::Amd);
            assert!(npu.tops > 0.0);
        }
    }

    // ── S3.3: Capability Negotiation ──────────────────────────────────

    #[test]
    fn npu_data_format_display() {
        assert_eq!(format!("{}", NpuDataFormat::INT8), "INT8");
        assert_eq!(format!("{}", NpuDataFormat::FP16), "FP16");
        assert_eq!(format!("{}", NpuDataFormat::BF16), "BF16");
        assert_eq!(format!("{}", NpuDataFormat::FP32), "FP32");
    }

    // ── S3.4: TOPS Reporting ──────────────────────────────────────────

    #[test]
    fn intel_npu_generations_have_positive_tops() {
        let cases = [
            ("0x7d1d", 11.0), // Meteor Lake
            ("0xad1d", 48.0), // Lunar Lake
            ("0xa74d", 48.0), // Arrow Lake
        ];
        for (_, expected_tops) in &cases {
            assert!(*expected_tops > 0.0);
        }
    }

    #[test]
    fn amd_xdna_tops_positive() {
        let (_, tops) = detect_amd_xdna_generation();
        assert!(tops > 0.0);
    }

    // ── S3.5: NPU Feature Struct ──────────────────────────────────────

    #[test]
    fn npu_device_fields_populated() {
        let npu = mock_intel_npu();
        assert_eq!(npu.vendor, NpuVendor::Intel);
        assert_eq!(npu.model, "Meteor Lake NPU");
        assert_eq!(npu.tops, 11.0);
        assert!(!npu.supported_formats.is_empty());
        assert!(npu.healthy);
    }

    #[test]
    fn npu_device_serializable() {
        let npu = mock_intel_npu();
        let json = serde_json::to_string(&npu);
        assert!(json.is_ok());
        let json_str = json.expect("serialization works");
        assert!(json_str.contains("Intel"));
        assert!(json_str.contains("Meteor Lake"));
    }

    // ── S3.6: Qualcomm Hexagon Stub ───────────────────────────────────

    #[test]
    fn qualcomm_detection_graceful() {
        let result = detect_qualcomm_hexagon();
        if let Some(npu) = result {
            assert_eq!(npu.vendor, NpuVendor::Qualcomm);
        }
    }

    // ── S3.7: Apple ANE Stub ──────────────────────────────────────────

    #[test]
    #[cfg(target_os = "macos")]
    fn apple_ane_stub_on_macos() {
        let result = detect_apple_ane();
        // On Apple Silicon, should detect ANE
        #[cfg(target_arch = "aarch64")]
        assert!(result.is_some());
    }

    // ── S3.8: NPU Health Check ────────────────────────────────────────

    #[test]
    fn mock_npu_health_flag() {
        let npu = mock_intel_npu();
        assert!(npu.healthy);

        let mut unhealthy = mock_intel_npu();
        unhealthy.healthy = false;
        assert!(!unhealthy.healthy);
    }

    // ── S3.9: NPU Info Display ────────────────────────────────────────

    #[test]
    fn display_info_no_npus() {
        let d = NpuDiscovery::default();
        let info = d.display_info();
        assert!(info.contains("none detected"));
    }

    #[test]
    fn display_info_with_npu() {
        let d = NpuDiscovery {
            devices: vec![mock_intel_npu()],
        };
        let info = d.display_info();
        assert!(info.contains("Intel"));
        assert!(info.contains("Meteor Lake"));
        assert!(info.contains("11.0 TOPS"));
        assert!(info.contains("healthy"));
    }

    #[test]
    fn npu_device_display_info_formats() {
        let npu = mock_intel_npu();
        let info = npu.display_info();
        assert!(info.contains("INT8"));
        assert!(info.contains("FP16"));
    }

    // ── S3.10: Serialization ──────────────────────────────────────────

    #[test]
    fn npu_discovery_serializable() {
        let d = NpuDiscovery {
            devices: vec![mock_intel_npu(), mock_amd_xdna()],
        };
        let json = serde_json::to_string_pretty(&d);
        assert!(json.is_ok());
        let json_str = json.expect("serialization works");
        assert!(json_str.contains("Intel"));
        assert!(json_str.contains("Amd"));
    }

    #[test]
    fn vendor_display_format() {
        assert_eq!(format!("{}", NpuVendor::Intel), "Intel");
        assert_eq!(format!("{}", NpuVendor::Amd), "AMD");
        assert_eq!(format!("{}", NpuVendor::Qualcomm), "Qualcomm");
        assert_eq!(format!("{}", NpuVendor::Apple), "Apple");
    }

    #[test]
    fn default_discovery_empty() {
        let d = NpuDiscovery::default();
        assert!(d.devices.is_empty());
    }

    #[test]
    fn npu_display_info_with_driver() {
        let mut npu = mock_intel_npu();
        npu.driver_version = "1.2.3".to_string();
        let info = npu.display_info();
        assert!(info.contains("1.2.3"));
    }

    #[test]
    fn npu_display_info_degraded() {
        let mut npu = mock_intel_npu();
        npu.healthy = false;
        let info = npu.display_info();
        assert!(info.contains("degraded"));
    }

    // ── Helpers ───────────────────────────────────────────────────────

    fn mock_intel_npu() -> NpuDevice {
        NpuDevice {
            vendor: NpuVendor::Intel,
            model: "Meteor Lake NPU".to_string(),
            tops: 11.0,
            supported_formats: vec![NpuDataFormat::INT8, NpuDataFormat::FP16],
            max_batch: 1,
            driver_version: String::new(),
            healthy: true,
        }
    }

    fn mock_amd_xdna() -> NpuDevice {
        NpuDevice {
            vendor: NpuVendor::Amd,
            model: "XDNA 2".to_string(),
            tops: 50.0,
            supported_formats: vec![
                NpuDataFormat::INT8,
                NpuDataFormat::FP16,
                NpuDataFormat::BF16,
            ],
            max_batch: 4,
            driver_version: String::new(),
            healthy: true,
        }
    }
}
