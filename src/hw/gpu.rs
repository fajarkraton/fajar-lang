//! # GPU Discovery
//!
//! Runtime detection of NVIDIA GPUs via the CUDA Driver API. The driver
//! library (`libcuda.so` / `nvcuda.dll`) is loaded dynamically at runtime
//! so that Fajar Lang compiles and runs without any CUDA dependency.
//!
//! ## Supported Features
//!
//! | Query | API |
//! |-------|-----|
//! | Device count | `cuDeviceGetCount` |
//! | Device name | `cuDeviceGetName` |
//! | Compute capability | `cuDeviceGetAttribute` (major/minor) |
//! | Total VRAM | `cuDeviceTotalMem_v2` |
//! | Multiprocessor count | `cuDeviceGetAttribute` |
//! | PCI bus/device IDs | `cuDeviceGetAttribute` |
//! | Driver version | `cuDriverGetVersion` |

use serde::Serialize;

// ═══════════════════════════════════════════════════════════════════════
// GPU Bus Type
// ═══════════════════════════════════════════════════════════════════════

/// Interconnect type between GPU and host.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum GpuBusType {
    /// PCI Express
    PCIe,
    /// NVIDIA NVLink
    NVLink,
    /// Integrated (shared memory)
    Integrated,
    /// Unknown bus type
    Unknown,
}

impl std::fmt::Display for GpuBusType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GpuBusType::PCIe => write!(f, "PCIe"),
            GpuBusType::NVLink => write!(f, "NVLink"),
            GpuBusType::Integrated => write!(f, "Integrated"),
            GpuBusType::Unknown => write!(f, "Unknown"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tensor Core Generation
// ═══════════════════════════════════════════════════════════════════════

/// NVIDIA Tensor Core generation, derived from compute capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum TensorCoreGen {
    /// No Tensor Cores (pre-Volta, CC < 7.0)
    None,
    /// 1st gen (Volta, CC 7.0)
    Gen1,
    /// 2nd gen (Turing, CC 7.5)
    Gen2,
    /// 3rd gen (Ampere, CC 8.0-8.6)
    Gen3,
    /// 4th gen (Hopper/Ada Lovelace, CC 8.9-9.0)
    Gen4,
    /// 5th gen (Blackwell, CC 10.0-10.1)
    Gen5,
}

impl TensorCoreGen {
    /// Determine Tensor Core generation from compute capability.
    pub fn from_compute_capability(major: u32, minor: u32) -> Self {
        match (major, minor) {
            (10, _) => TensorCoreGen::Gen5, // Blackwell
            (9, _) => TensorCoreGen::Gen4,  // Hopper
            (8, 9) => TensorCoreGen::Gen4,  // Ada Lovelace
            (8, _) => TensorCoreGen::Gen3,  // Ampere
            (7, 5) => TensorCoreGen::Gen2,  // Turing
            (7, 0) => TensorCoreGen::Gen1,  // Volta
            _ => TensorCoreGen::None,       // Pre-Volta or unknown
        }
    }

    /// Returns whether FP4 Tensor Core operations are supported.
    pub fn supports_fp4(&self) -> bool {
        matches!(self, TensorCoreGen::Gen5)
    }

    /// Returns whether FP8 Tensor Core operations are supported.
    pub fn supports_fp8(&self) -> bool {
        matches!(self, TensorCoreGen::Gen4 | TensorCoreGen::Gen5)
    }

    /// Returns whether BF16 Tensor Core operations are supported.
    pub fn supports_bf16(&self) -> bool {
        matches!(
            self,
            TensorCoreGen::Gen3 | TensorCoreGen::Gen4 | TensorCoreGen::Gen5
        )
    }
}

impl std::fmt::Display for TensorCoreGen {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TensorCoreGen::None => write!(f, "None"),
            TensorCoreGen::Gen1 => write!(f, "1st gen (Volta)"),
            TensorCoreGen::Gen2 => write!(f, "2nd gen (Turing)"),
            TensorCoreGen::Gen3 => write!(f, "3rd gen (Ampere)"),
            TensorCoreGen::Gen4 => write!(f, "4th gen (Ada/Hopper)"),
            TensorCoreGen::Gen5 => write!(f, "5th gen (Blackwell)"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// GPU Architecture Name
// ═══════════════════════════════════════════════════════════════════════

/// Map compute capability to architecture codename.
fn arch_name(major: u32, minor: u32) -> &'static str {
    match (major, minor) {
        (10, _) => "Blackwell",
        (9, _) => "Hopper",
        (8, 9) => "Ada Lovelace",
        (8, _) => "Ampere",
        (7, 5) => "Turing",
        (7, 0) => "Volta",
        (6, _) => "Pascal",
        (5, _) => "Maxwell",
        _ => "Unknown",
    }
}

/// Estimate CUDA core count from SM count and architecture.
fn estimate_cuda_cores(major: u32, sm_count: u32) -> u32 {
    let cores_per_sm = match major {
        10 => 128, // Blackwell
        9 => 128,  // Hopper
        8 => 128,  // Ampere / Ada Lovelace
        7 => 64,   // Volta / Turing
        6 => 128,  // Pascal (GP100=64, GP10x=128)
        5 => 128,  // Maxwell
        _ => 64,
    };
    sm_count * cores_per_sm
}

// ═══════════════════════════════════════════════════════════════════════
// GpuDevice
// ═══════════════════════════════════════════════════════════════════════

/// A detected NVIDIA GPU device.
#[derive(Debug, Clone, Serialize)]
pub struct GpuDevice {
    /// Device index (0-based).
    pub id: u32,
    /// Device name (e.g., "NVIDIA GeForce RTX 4090").
    pub name: String,
    /// Architecture codename (e.g., "Ada Lovelace").
    pub architecture: String,
    /// Compute capability major version.
    pub compute_capability_major: u32,
    /// Compute capability minor version.
    pub compute_capability_minor: u32,
    /// Total video memory in bytes.
    pub vram_total_bytes: u64,
    /// Number of streaming multiprocessors.
    pub sm_count: u32,
    /// Estimated CUDA core count.
    pub cuda_cores: u32,
    /// Tensor Core generation.
    pub tensor_cores: TensorCoreGen,
    /// PCI bus ID.
    pub pci_bus_id: u32,
    /// PCI device ID.
    pub pci_device_id: u32,
    /// Bus type (PCIe, NVLink, etc.).
    pub bus_type: GpuBusType,
}

impl GpuDevice {
    /// Compute capability as "major.minor" string (e.g., "8.9").
    pub fn compute_capability_str(&self) -> String {
        format!(
            "{}.{}",
            self.compute_capability_major, self.compute_capability_minor
        )
    }

    /// sm_XX target string (e.g., "sm_89").
    pub fn sm_target(&self) -> String {
        format!(
            "sm_{}{}",
            self.compute_capability_major, self.compute_capability_minor
        )
    }

    /// Total VRAM in human-readable format (e.g., "16.0 GB").
    pub fn vram_display(&self) -> String {
        let gb = self.vram_total_bytes as f64 / (1024.0 * 1024.0 * 1024.0);
        if gb >= 1.0 {
            format!("{:.1} GB", gb)
        } else {
            let mb = self.vram_total_bytes as f64 / (1024.0 * 1024.0);
            format!("{:.0} MB", mb)
        }
    }

    /// Format GPU info for human-readable display.
    pub fn display_info(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("  GPU {} — {}\n", self.id, self.name));
        out.push_str(&format!(
            "    Architecture: {} ({})\n",
            self.architecture,
            self.sm_target()
        ));
        out.push_str(&format!("    VRAM:         {}\n", self.vram_display()));
        out.push_str(&format!(
            "    SMs:          {} ({} CUDA cores)\n",
            self.sm_count, self.cuda_cores
        ));
        out.push_str(&format!("    Tensor Cores: {}\n", self.tensor_cores));
        out.push_str(&format!("    Bus:          {}\n", self.bus_type));
        if self.tensor_cores.supports_fp8() {
            out.push_str("    Formats:      FP8 ");
            if self.tensor_cores.supports_fp4() {
                out.push_str("FP4 ");
            }
            out.push_str("BF16 FP16 INT8\n");
        } else if self.tensor_cores.supports_bf16() {
            out.push_str("    Formats:      BF16 FP16 INT8\n");
        }
        out
    }
}

// ═══════════════════════════════════════════════════════════════════════
// GPU Discovery Info (driver-level metadata)
// ═══════════════════════════════════════════════════════════════════════

/// Result of GPU discovery — devices plus driver metadata.
#[derive(Debug, Clone, Default, Serialize)]
pub struct GpuDiscovery {
    /// Detected GPU devices.
    pub devices: Vec<GpuDevice>,
    /// CUDA driver version (e.g., 12080 for 12.8).
    pub driver_version: Option<u32>,
    /// Whether CUDA runtime is available.
    pub cuda_available: bool,
}

impl GpuDiscovery {
    /// Discover all NVIDIA GPUs by dynamically loading the CUDA driver API.
    ///
    /// Returns an empty result if CUDA is not installed or no GPUs are found.
    /// Never panics — all errors result in graceful degradation.
    pub fn detect() -> Self {
        Self::try_detect().unwrap_or_default()
    }

    /// Internal detection that may fail.
    fn try_detect() -> Result<GpuDiscovery, GpuDetectError> {
        let lib = load_cuda_driver()?;

        // Initialize CUDA
        let cu_init: CuInitFn = *unsafe { lib.get(b"cuInit\0") }
            .map_err(|_| GpuDetectError::SymbolNotFound("cuInit"))?;
        let result = unsafe { cu_init(0) };
        if result != 0 {
            return Err(GpuDetectError::CudaError("cuInit", result));
        }

        // Get driver version
        let driver_version = get_driver_version(&lib);

        // Check minimum driver version (12.0 = 12000)
        if let Some(ver) = driver_version {
            if ver < 12000 {
                eprintln!(
                    "warning: CUDA driver version {}.{} detected, minimum 12.0 recommended",
                    ver / 1000,
                    (ver % 1000) / 10
                );
            }
        }

        // Get device count
        let cu_device_get_count: CuDeviceGetCountFn = *unsafe { lib.get(b"cuDeviceGetCount\0") }
            .map_err(|_| GpuDetectError::SymbolNotFound("cuDeviceGetCount"))?;
        let mut count: i32 = 0;
        let result = unsafe { cu_device_get_count(&mut count) };
        if result != 0 {
            return Err(GpuDetectError::CudaError("cuDeviceGetCount", result));
        }

        if count == 0 {
            return Ok(GpuDiscovery {
                devices: Vec::new(),
                driver_version,
                cuda_available: true,
            });
        }

        // Load remaining symbols
        let cu_device_get: CuDeviceGetFn = *unsafe { lib.get(b"cuDeviceGet\0") }
            .map_err(|_| GpuDetectError::SymbolNotFound("cuDeviceGet"))?;
        let cu_device_get_name: CuDeviceGetNameFn = *unsafe { lib.get(b"cuDeviceGetName\0") }
            .map_err(|_| GpuDetectError::SymbolNotFound("cuDeviceGetName"))?;
        let cu_device_get_attribute: CuDeviceGetAttributeFn =
            *unsafe { lib.get(b"cuDeviceGetAttribute\0") }
                .map_err(|_| GpuDetectError::SymbolNotFound("cuDeviceGetAttribute"))?;
        let cu_device_total_mem: CuDeviceTotalMemFn = *unsafe { lib.get(b"cuDeviceTotalMem_v2\0") }
            .map_err(|_| GpuDetectError::SymbolNotFound("cuDeviceTotalMem_v2"))?;

        // Enumerate devices
        let mut devices = Vec::with_capacity(count as usize);
        for i in 0..count {
            let mut device: i32 = 0;
            let result = unsafe { cu_device_get(&mut device, i) };
            if result != 0 {
                continue;
            }

            let name = get_device_name(&cu_device_get_name, device);
            let cc_major = get_device_attr(&cu_device_get_attribute, device, 75); // COMPUTE_CAPABILITY_MAJOR
            let cc_minor = get_device_attr(&cu_device_get_attribute, device, 76); // COMPUTE_CAPABILITY_MINOR
            let sm_count = get_device_attr(&cu_device_get_attribute, device, 16); // MULTIPROCESSOR_COUNT
            let pci_bus = get_device_attr(&cu_device_get_attribute, device, 33); // PCI_BUS_ID
            let pci_dev = get_device_attr(&cu_device_get_attribute, device, 34); // PCI_DEVICE_ID
            let integrated = get_device_attr(&cu_device_get_attribute, device, 18); // INTEGRATED

            let mut total_mem: usize = 0;
            unsafe {
                cu_device_total_mem(&mut total_mem, device);
            }

            let major = cc_major as u32;
            let minor = cc_minor as u32;
            let sms = sm_count as u32;

            let bus_type = if integrated != 0 {
                GpuBusType::Integrated
            } else {
                GpuBusType::PCIe
            };

            devices.push(GpuDevice {
                id: i as u32,
                name,
                architecture: arch_name(major, minor).to_string(),
                compute_capability_major: major,
                compute_capability_minor: minor,
                vram_total_bytes: total_mem as u64,
                sm_count: sms,
                cuda_cores: estimate_cuda_cores(major, sms),
                tensor_cores: TensorCoreGen::from_compute_capability(major, minor),
                pci_bus_id: pci_bus as u32,
                pci_device_id: pci_dev as u32,
                bus_type,
            });
        }

        Ok(GpuDiscovery {
            devices,
            driver_version,
            cuda_available: true,
        })
    }

    /// Format driver version as "major.minor" string.
    pub fn driver_version_str(&self) -> String {
        match self.driver_version {
            Some(v) => format!("{}.{}", v / 1000, (v % 1000) / 10),
            None => "N/A".to_string(),
        }
    }

    /// Format all GPU info for human-readable display.
    pub fn display_info(&self) -> String {
        let mut out = String::new();
        out.push_str("── GPU ──────────────────────────────────\n");
        if !self.cuda_available {
            out.push_str("  CUDA:       not available\n");
            return out;
        }
        out.push_str(&format!("  CUDA Driver: {}\n", self.driver_version_str()));
        if self.devices.is_empty() {
            out.push_str("  Devices:    (none detected)\n");
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
// CUDA Driver API — dynamic loading
// ═══════════════════════════════════════════════════════════════════════

/// Error during GPU detection.
#[derive(Debug)]
enum GpuDetectError {
    /// Could not load CUDA driver library.
    LibraryNotFound,
    /// Could not find a required symbol.
    SymbolNotFound(#[allow(dead_code)] &'static str),
    /// CUDA API returned an error code.
    CudaError(#[allow(dead_code)] &'static str, #[allow(dead_code)] i32),
}

// CUDA Driver API function signatures
type CuInitFn = unsafe extern "C" fn(flags: u32) -> i32;
type CuDeviceGetCountFn = unsafe extern "C" fn(count: *mut i32) -> i32;
type CuDeviceGetFn = unsafe extern "C" fn(device: *mut i32, ordinal: i32) -> i32;
type CuDeviceGetNameFn = unsafe extern "C" fn(name: *mut u8, len: i32, device: i32) -> i32;
type CuDeviceGetAttributeFn =
    unsafe extern "C" fn(value: *mut i32, attrib: i32, device: i32) -> i32;
type CuDeviceTotalMemFn = unsafe extern "C" fn(bytes: *mut usize, device: i32) -> i32;
type CuDriverGetVersionFn = unsafe extern "C" fn(version: *mut i32) -> i32;

/// Attempt to load the CUDA driver library.
fn load_cuda_driver() -> Result<libloading::Library, GpuDetectError> {
    #[cfg(target_os = "linux")]
    let lib_names = &["libcuda.so.1", "libcuda.so"];
    #[cfg(target_os = "windows")]
    let lib_names = &["nvcuda.dll"];
    #[cfg(target_os = "macos")]
    let lib_names: &[&str] = &[]; // No CUDA on macOS

    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    let lib_names: &[&str] = &[];

    for name in lib_names {
        // SAFETY: Loading a well-known system library. The library is kept alive
        // for the duration of GPU discovery.
        if let Ok(lib) = unsafe { libloading::Library::new(name) } {
            return Ok(lib);
        }
    }

    Err(GpuDetectError::LibraryNotFound)
}

/// Get CUDA driver version.
fn get_driver_version(lib: &libloading::Library) -> Option<u32> {
    let func: CuDriverGetVersionFn = *unsafe { lib.get(b"cuDriverGetVersion\0") }.ok()?;
    let mut version: i32 = 0;
    let result = unsafe { func(&mut version) };
    if result == 0 {
        Some(version as u32)
    } else {
        None
    }
}

/// Get device name string.
fn get_device_name(func: &CuDeviceGetNameFn, device: i32) -> String {
    let mut name_buf = [0u8; 256];
    let result = unsafe { func(name_buf.as_mut_ptr(), 256, device) };
    if result != 0 {
        return String::from("Unknown GPU");
    }
    let nul_pos = name_buf.iter().position(|&b| b == 0).unwrap_or(256);
    String::from_utf8_lossy(&name_buf[..nul_pos])
        .trim()
        .to_string()
}

/// Get a device attribute value.
fn get_device_attr(func: &CuDeviceGetAttributeFn, device: i32, attrib: i32) -> i32 {
    let mut value: i32 = 0;
    unsafe {
        func(&mut value, attrib, device);
    }
    value
}

// ═══════════════════════════════════════════════════════════════════════
// Multi-GPU Topology
// ═══════════════════════════════════════════════════════════════════════

/// P2P connectivity between two GPUs.
#[derive(Debug, Clone, Serialize)]
pub struct GpuP2PLink {
    /// Source GPU index.
    pub gpu_a: u32,
    /// Destination GPU index.
    pub gpu_b: u32,
    /// Whether peer-to-peer access is supported.
    pub p2p_supported: bool,
}

impl GpuDiscovery {
    /// Detect P2P connectivity between all GPU pairs.
    ///
    /// Returns empty Vec if < 2 GPUs or CUDA unavailable.
    pub fn detect_topology(&self) -> Vec<GpuP2PLink> {
        if self.devices.len() < 2 || !self.cuda_available {
            return Vec::new();
        }

        // Without a CUDA context, we can infer topology from PCI bus IDs:
        // same bus = likely NVLink-capable, different bus = PCIe only
        let mut links = Vec::new();
        for i in 0..self.devices.len() {
            for j in (i + 1)..self.devices.len() {
                let a = &self.devices[i];
                let b = &self.devices[j];
                links.push(GpuP2PLink {
                    gpu_a: a.id,
                    gpu_b: b.id,
                    p2p_supported: a.pci_bus_id == b.pci_bus_id,
                });
            }
        }
        links
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── S2.1: CUDA Device Enumeration ─────────────────────────────────

    #[test]
    fn detect_never_panics() {
        // Must succeed on any system, with or without CUDA
        let discovery = GpuDiscovery::detect();
        // cuda_available may be true or false — both are valid
        assert!(discovery.devices.len() <= 16, "unreasonable GPU count");
    }

    // ── S2.2: Compute Capability Query ────────────────────────────────

    #[test]
    fn compute_capability_str_format() {
        let dev = mock_gpu(8, 9, "RTX 4090");
        assert_eq!(dev.compute_capability_str(), "8.9");
    }

    #[test]
    fn sm_target_format() {
        let dev = mock_gpu(8, 9, "RTX 4090");
        assert_eq!(dev.sm_target(), "sm_89");
        let dev2 = mock_gpu(10, 0, "RTX 5090");
        assert_eq!(dev2.sm_target(), "sm_100");
    }

    // ── S2.3: Memory Query ────────────────────────────────────────────

    #[test]
    fn vram_display_gb() {
        let mut dev = mock_gpu(8, 9, "RTX 4090");
        dev.vram_total_bytes = 24 * 1024 * 1024 * 1024; // 24 GB
        assert_eq!(dev.vram_display(), "24.0 GB");
    }

    #[test]
    fn vram_display_mb() {
        let mut dev = mock_gpu(5, 0, "GTX 750");
        dev.vram_total_bytes = 512 * 1024 * 1024; // 512 MB
        assert_eq!(dev.vram_display(), "512 MB");
    }

    // ── S2.4: Multi-GPU Topology ──────────────────────────────────────

    #[test]
    fn topology_empty_for_single_gpu() {
        let discovery = GpuDiscovery {
            devices: vec![mock_gpu(8, 9, "RTX 4090")],
            driver_version: Some(12080),
            cuda_available: true,
        };
        assert!(discovery.detect_topology().is_empty());
    }

    #[test]
    fn topology_detects_pairs() {
        let mut gpu1 = mock_gpu(8, 9, "RTX 4090 #1");
        gpu1.id = 1;
        gpu1.pci_bus_id = 2;
        let discovery = GpuDiscovery {
            devices: vec![mock_gpu(8, 9, "RTX 4090 #0"), gpu1],
            driver_version: Some(12080),
            cuda_available: true,
        };
        let links = discovery.detect_topology();
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].gpu_a, 0);
        assert_eq!(links[0].gpu_b, 1);
    }

    // ── S2.5: GPU Feature Struct ──────────────────────────────────────

    #[test]
    fn gpu_device_fields_populated() {
        let dev = mock_gpu(8, 9, "NVIDIA GeForce RTX 4090");
        assert_eq!(dev.id, 0);
        assert_eq!(dev.name, "NVIDIA GeForce RTX 4090");
        assert_eq!(dev.compute_capability_major, 8);
        assert_eq!(dev.compute_capability_minor, 9);
        assert_eq!(dev.architecture, "Ada Lovelace");
    }

    // ── S2.6: Tensor Core Detection ───────────────────────────────────

    #[test]
    fn tensor_core_gen_mapping() {
        assert_eq!(
            TensorCoreGen::from_compute_capability(5, 0),
            TensorCoreGen::None
        );
        assert_eq!(
            TensorCoreGen::from_compute_capability(7, 0),
            TensorCoreGen::Gen1
        );
        assert_eq!(
            TensorCoreGen::from_compute_capability(7, 5),
            TensorCoreGen::Gen2
        );
        assert_eq!(
            TensorCoreGen::from_compute_capability(8, 0),
            TensorCoreGen::Gen3
        );
        assert_eq!(
            TensorCoreGen::from_compute_capability(8, 6),
            TensorCoreGen::Gen3
        );
        assert_eq!(
            TensorCoreGen::from_compute_capability(8, 9),
            TensorCoreGen::Gen4
        );
        assert_eq!(
            TensorCoreGen::from_compute_capability(9, 0),
            TensorCoreGen::Gen4
        );
        assert_eq!(
            TensorCoreGen::from_compute_capability(10, 0),
            TensorCoreGen::Gen5
        );
        assert_eq!(
            TensorCoreGen::from_compute_capability(10, 1),
            TensorCoreGen::Gen5
        );
    }

    #[test]
    fn tensor_core_format_support() {
        let gen5 = TensorCoreGen::Gen5;
        assert!(gen5.supports_fp4());
        assert!(gen5.supports_fp8());
        assert!(gen5.supports_bf16());

        let gen4 = TensorCoreGen::Gen4;
        assert!(!gen4.supports_fp4());
        assert!(gen4.supports_fp8());
        assert!(gen4.supports_bf16());

        let gen3 = TensorCoreGen::Gen3;
        assert!(!gen3.supports_fp4());
        assert!(!gen3.supports_fp8());
        assert!(gen3.supports_bf16());

        let gen2 = TensorCoreGen::Gen2;
        assert!(!gen2.supports_bf16());
    }

    // ── S2.7: Driver Version Check ────────────────────────────────────

    #[test]
    fn driver_version_str_format() {
        let d = GpuDiscovery {
            driver_version: Some(12080),
            ..Default::default()
        };
        assert_eq!(d.driver_version_str(), "12.8");

        let d2 = GpuDiscovery {
            driver_version: Some(11070),
            ..Default::default()
        };
        assert_eq!(d2.driver_version_str(), "11.7");

        let d3 = GpuDiscovery::default();
        assert_eq!(d3.driver_version_str(), "N/A");
    }

    // ── S2.8: Fallback Without CUDA ───────────────────────────────────

    #[test]
    fn default_discovery_no_cuda() {
        let d = GpuDiscovery::default();
        assert!(!d.cuda_available);
        assert!(d.devices.is_empty());
        assert!(d.driver_version.is_none());
    }

    // ── S2.9: GPU Info Display ────────────────────────────────────────

    #[test]
    fn display_info_no_cuda() {
        let d = GpuDiscovery::default();
        let info = d.display_info();
        assert!(info.contains("not available"));
    }

    #[test]
    fn display_info_with_gpu() {
        let d = GpuDiscovery {
            devices: vec![mock_gpu(8, 9, "RTX 4090")],
            driver_version: Some(12080),
            cuda_available: true,
        };
        let info = d.display_info();
        assert!(info.contains("RTX 4090"));
        assert!(info.contains("Ada Lovelace"));
        assert!(info.contains("sm_89"));
        assert!(info.contains("12.8"));
    }

    #[test]
    fn device_display_includes_tensor_core_formats() {
        let dev = mock_gpu(8, 9, "RTX 4090");
        let info = dev.display_info();
        assert!(info.contains("4th gen"));
        assert!(info.contains("FP8"));
    }

    // ── S2.10: Serialization ──────────────────────────────────────────

    #[test]
    fn gpu_device_serializable() {
        let dev = mock_gpu(8, 9, "RTX 4090");
        let json = serde_json::to_string(&dev);
        assert!(json.is_ok());
        let json_str = json.expect("serialization works");
        assert!(json_str.contains("RTX 4090"));
        assert!(json_str.contains("Ada Lovelace"));
    }

    #[test]
    fn gpu_discovery_serializable() {
        let d = GpuDiscovery {
            devices: vec![mock_gpu(8, 9, "RTX 4090")],
            driver_version: Some(12080),
            cuda_available: true,
        };
        let json = serde_json::to_string_pretty(&d);
        assert!(json.is_ok());
    }

    #[test]
    fn arch_name_mapping() {
        assert_eq!(arch_name(10, 0), "Blackwell");
        assert_eq!(arch_name(9, 0), "Hopper");
        assert_eq!(arch_name(8, 9), "Ada Lovelace");
        assert_eq!(arch_name(8, 0), "Ampere");
        assert_eq!(arch_name(7, 5), "Turing");
        assert_eq!(arch_name(7, 0), "Volta");
        assert_eq!(arch_name(6, 1), "Pascal");
    }

    #[test]
    fn bus_type_display() {
        assert_eq!(format!("{}", GpuBusType::PCIe), "PCIe");
        assert_eq!(format!("{}", GpuBusType::NVLink), "NVLink");
        assert_eq!(format!("{}", GpuBusType::Integrated), "Integrated");
    }

    #[test]
    fn tensor_core_gen_display() {
        assert_eq!(format!("{}", TensorCoreGen::None), "None");
        assert_eq!(format!("{}", TensorCoreGen::Gen4), "4th gen (Ada/Hopper)");
        assert_eq!(format!("{}", TensorCoreGen::Gen5), "5th gen (Blackwell)");
    }

    // ── Helper ────────────────────────────────────────────────────────

    fn mock_gpu(major: u32, minor: u32, name: &str) -> GpuDevice {
        GpuDevice {
            id: 0,
            name: name.to_string(),
            architecture: arch_name(major, minor).to_string(),
            compute_capability_major: major,
            compute_capability_minor: minor,
            vram_total_bytes: 16 * 1024 * 1024 * 1024,
            sm_count: 128,
            cuda_cores: estimate_cuda_cores(major, 128),
            tensor_cores: TensorCoreGen::from_compute_capability(major, minor),
            pci_bus_id: 1,
            pci_device_id: 0,
            bus_type: GpuBusType::PCIe,
        }
    }
}
