//! # Hardware Detection & Runtime
//!
//! Detects platform capabilities at runtime — CPUs, GPUs, NPUs — and builds
//! a unified accelerator registry for dispatch decisions.
//!
//! ## Architecture
//!
//! ```text
//! HardwareProfile
//! ├── CpuFeatures   (CPUID / /proc/cpuinfo / ISA string)
//! ├── GpuDiscovery  (CUDA driver API — dynamically loaded)
//! └── Vec<NpuDevice> (sysfs / OpenVINO — Phase 1 S3)
//! ```

pub mod cpu;
pub mod gpu;
pub mod npu;

pub use cpu::CpuFeatures;
pub use gpu::{GpuDevice, GpuDiscovery, TensorCoreGen};
pub use npu::{NpuDevice, NpuDiscovery, NpuVendor};

use serde::Serialize;

// ═══════════════════════════════════════════════════════════════════════
// Hardware Profile
// ═══════════════════════════════════════════════════════════════════════

/// Unified hardware profile describing all detected accelerators.
///
/// Constructed once at startup via [`HardwareProfile::detect()`] and cached
/// for the lifetime of the process.
#[derive(Debug, Clone, Serialize)]
pub struct HardwareProfile {
    /// Detected CPU features (ISA extensions, vendor, model).
    pub cpu: CpuFeatures,
    /// Detected GPU devices and CUDA driver info.
    pub gpu: GpuDiscovery,
    /// Detected NPU devices (Intel VPU, AMD XDNA, Qualcomm Hexagon, Apple ANE).
    pub npu: NpuDiscovery,
}

impl HardwareProfile {
    /// Detect all available hardware on the current platform.
    pub fn detect() -> Self {
        Self {
            cpu: CpuFeatures::detect(),
            gpu: GpuDiscovery::detect(),
            npu: NpuDiscovery::detect(),
        }
    }

    /// Create a profile with no hardware capabilities (for testing / CI).
    pub fn empty() -> Self {
        Self {
            cpu: CpuFeatures::default(),
            gpu: GpuDiscovery::default(),
            npu: NpuDiscovery::default(),
        }
    }

    /// Format hardware info for human-readable CLI output.
    pub fn display_info(&self) -> String {
        let mut out = String::new();
        out.push_str("=== Fajar Lang Hardware Profile ===\n\n");
        out.push_str(&self.cpu.display_info());
        out.push('\n');
        out.push_str(&self.gpu.display_info());
        out.push('\n');
        out.push_str(&self.npu.display_info());
        out
    }

    /// Serialize hardware profile to JSON.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_returns_valid_profile() {
        let profile = HardwareProfile::detect();
        let info = profile.display_info();
        assert!(info.contains("CPU"));
        assert!(info.contains("GPU"));
        assert!(info.contains("NPU"));
    }

    #[test]
    fn empty_profile_has_no_features() {
        let profile = HardwareProfile::empty();
        assert!(!profile.cpu.avx512f);
        assert!(!profile.cpu.amx_bf16);
        assert!(!profile.gpu.cuda_available);
        assert!(profile.gpu.devices.is_empty());
        assert!(profile.npu.devices.is_empty());
    }

    #[test]
    fn profile_serializes_to_json() {
        let profile = HardwareProfile::detect();
        let json = profile.to_json().expect("serialization should succeed");
        assert!(json.contains("vendor"));
        assert!(json.contains("cuda_available"));
    }

    #[test]
    fn display_info_contains_sections() {
        let profile = HardwareProfile::detect();
        let info = profile.display_info();
        assert!(info.contains("Hardware Profile"));
        assert!(info.contains("── CPU"));
        assert!(info.contains("── GPU"));
        assert!(info.contains("── NPU"));
    }
}
