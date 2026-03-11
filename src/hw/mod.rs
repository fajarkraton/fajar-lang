//! # Hardware Detection & Runtime
//!
//! Detects platform capabilities at runtime — CPUs, GPUs, NPUs — and builds
//! a unified accelerator registry for dispatch decisions.
//!
//! ## Architecture
//!
//! ```text
//! HardwareProfile
//! ├── CpuFeatures (CPUID / /proc/cpuinfo / ISA string)
//! ├── Vec<GpuDevice>  (CUDA driver API — Phase 1 S2)
//! └── Vec<NpuDevice>  (sysfs / OpenVINO — Phase 1 S3)
//! ```

pub mod cpu;

pub use cpu::CpuFeatures;

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
    // pub gpus: Vec<GpuDevice>,  // Phase 1 S2
    // pub npus: Vec<NpuDevice>,  // Phase 1 S3
}

impl HardwareProfile {
    /// Detect all available hardware on the current platform.
    pub fn detect() -> Self {
        Self {
            cpu: CpuFeatures::detect(),
        }
    }

    /// Create a profile with no hardware capabilities (for testing / CI).
    pub fn empty() -> Self {
        Self {
            cpu: CpuFeatures::default(),
        }
    }

    /// Format hardware info for human-readable CLI output.
    pub fn display_info(&self) -> String {
        let mut out = String::new();
        out.push_str("=== Fajar Lang Hardware Profile ===\n\n");
        out.push_str(&self.cpu.display_info());
        out
    }

    /// Serialize hardware profile to JSON.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

// Re-export serde_json for to_json
use serde_json;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_returns_valid_profile() {
        let profile = HardwareProfile::detect();
        // Should always succeed on any platform
        let info = profile.display_info();
        assert!(info.contains("CPU"));
    }

    #[test]
    fn empty_profile_has_no_features() {
        let profile = HardwareProfile::empty();
        assert!(!profile.cpu.avx512f);
        assert!(!profile.cpu.amx_bf16);
    }

    #[test]
    fn profile_serializes_to_json() {
        let profile = HardwareProfile::detect();
        let json = profile.to_json().expect("serialization should succeed");
        assert!(json.contains("vendor"));
        assert!(json.contains("model_name"));
    }

    #[test]
    fn display_info_contains_header() {
        let profile = HardwareProfile::detect();
        let info = profile.display_info();
        assert!(info.contains("Hardware Profile"));
    }
}
