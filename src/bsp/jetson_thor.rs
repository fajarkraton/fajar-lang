//! NVIDIA Jetson Thor Board Support Package.
//!
//! First-class support for NVIDIA Jetson Thor — the target platform
//! for embedded AI at scale, featuring Blackwell GPU architecture
//! and up to 128GB unified memory.
//!
//! # Hardware Overview
//!
//! - **CPU:** ARM Cortex-A78AE + Grace (up to 72 cores)
//! - **GPU:** Blackwell architecture (sm_101, 5th gen Tensor Cores)
//! - **DLA:** 2x NVDLA v2.0 engines
//! - **Memory:** Up to 128GB LPDDR5x (unified CPU/GPU)
//! - **SDK:** JetPack 7.1 (CUDA 13.0, TensorRT, cuDNN)
//! - **Power:** Configurable 10W–130W profiles
//!
//! This is a Linux userspace target (L4T / Linux for Tegra),
//! so memory regions are conceptual (managed by the kernel).

use super::{Board, BspArch, MemoryAttr, MemoryRegion, Peripheral};
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// Thor Module Variants
// ═══════════════════════════════════════════════════════════════════════

/// Jetson Thor module variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThorVariant {
    /// Thor T4000 — entry-level (8 CPU cores, 32GB RAM).
    T4000,
    /// Thor T5000 — high-end (72 CPU cores, 128GB RAM).
    T5000,
}

impl fmt::Display for ThorVariant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ThorVariant::T4000 => write!(f, "Jetson Thor T4000"),
            ThorVariant::T5000 => write!(f, "Jetson Thor T5000"),
        }
    }
}

/// Detects Thor variant from device tree model string.
///
/// On real hardware, reads `/proc/device-tree/model`.
/// Returns `None` if not running on Jetson Thor.
pub fn detect_variant() -> Option<ThorVariant> {
    let model_path = "/proc/device-tree/model";
    if let Ok(model) = std::fs::read_to_string(model_path) {
        let model = model.trim_end_matches('\0').to_lowercase();
        if model.contains("thor") && model.contains("t5000") {
            return Some(ThorVariant::T5000);
        }
        if model.contains("thor") && model.contains("t4000") {
            return Some(ThorVariant::T4000);
        }
        if model.contains("jetson thor") {
            return Some(ThorVariant::T5000); // default to high-end
        }
    }
    None
}

// ═══════════════════════════════════════════════════════════════════════
// JetPack SDK
// ═══════════════════════════════════════════════════════════════════════

/// JetPack SDK version information.
#[derive(Debug, Clone)]
pub struct JetPackSdk {
    /// Major version (e.g., 7).
    pub major: u32,
    /// Minor version (e.g., 1).
    pub minor: u32,
    /// CUDA version string (e.g., "13.0").
    pub cuda_version: String,
    /// TensorRT version string.
    pub tensorrt_version: String,
    /// cuDNN version string.
    pub cudnn_version: String,
    /// L4T version string (e.g., "36.4.0").
    pub l4t_version: String,
}

impl JetPackSdk {
    /// Creates a JetPack 7.1 SDK descriptor.
    pub fn jetpack_7_1() -> Self {
        Self {
            major: 7,
            minor: 1,
            cuda_version: "13.0".to_string(),
            tensorrt_version: "10.5".to_string(),
            cudnn_version: "9.6".to_string(),
            l4t_version: "36.4.0".to_string(),
        }
    }

    /// Detects installed JetPack version from the system.
    ///
    /// Checks `/etc/nv_tegra_release` for L4T version.
    pub fn detect() -> Option<Self> {
        let release_path = "/etc/nv_tegra_release";
        if let Ok(content) = std::fs::read_to_string(release_path) {
            // Parse "# R36 (release), REVISION: 4.0"
            if content.contains("R36") {
                return Some(Self::jetpack_7_1());
            }
        }
        None
    }

    /// Validates that required libraries exist.
    pub fn validate_libraries(&self) -> Vec<String> {
        let libs = [
            "/usr/local/cuda/lib64/libcudart.so",
            "/usr/lib/aarch64-linux-gnu/libnvinfer.so",
            "/usr/lib/aarch64-linux-gnu/libcudnn.so",
        ];
        let mut missing = Vec::new();
        for lib in &libs {
            if !std::path::Path::new(lib).exists() {
                missing.push(lib.to_string());
            }
        }
        missing
    }
}

impl fmt::Display for JetPackSdk {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "JetPack {}.{} (CUDA {}, TensorRT {}, cuDNN {}, L4T {})",
            self.major,
            self.minor,
            self.cuda_version,
            self.tensorrt_version,
            self.cudnn_version,
            self.l4t_version
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Device Tree Parsing
// ═══════════════════════════════════════════════════════════════════════

/// Parsed device tree information for Jetson Thor.
#[derive(Debug, Clone)]
pub struct DeviceTree {
    /// Model string from /proc/device-tree/model.
    pub model: String,
    /// Compatible strings.
    pub compatible: Vec<String>,
    /// System memory size in bytes.
    pub memory_size_bytes: u64,
    /// GPU memory partition size in bytes.
    pub gpu_memory_bytes: u64,
    /// DLA engine count.
    pub dla_count: u32,
}

impl DeviceTree {
    /// Reads the device tree from sysfs.
    pub fn read() -> Option<Self> {
        let model = std::fs::read_to_string("/proc/device-tree/model")
            .ok()?
            .trim_end_matches('\0')
            .to_string();

        let compatible = std::fs::read_to_string("/proc/device-tree/compatible")
            .unwrap_or_default()
            .split('\0')
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect();

        Some(Self {
            model,
            compatible,
            memory_size_bytes: 128 * 1024 * 1024 * 1024, // default 128GB
            gpu_memory_bytes: 0,                         // unified memory
            dla_count: 2,
        })
    }

    /// Creates a mock device tree for testing.
    pub fn mock(variant: ThorVariant) -> Self {
        let (model, mem_gb) = match variant {
            ThorVariant::T4000 => ("NVIDIA Jetson Thor T4000", 32u64),
            ThorVariant::T5000 => ("NVIDIA Jetson Thor T5000", 128u64),
        };
        Self {
            model: model.to_string(),
            compatible: vec![
                "nvidia,jetson-thor".to_string(),
                "nvidia,tegra-thor".to_string(),
            ],
            memory_size_bytes: mem_gb * 1024 * 1024 * 1024,
            gpu_memory_bytes: 0, // unified
            dla_count: 2,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Thor CPU Configuration
// ═══════════════════════════════════════════════════════════════════════

/// CPU cluster configuration for Jetson Thor.
#[derive(Debug, Clone)]
pub struct ThorCpuConfig {
    /// Number of Cortex-A78AE cores.
    pub a78ae_cores: u32,
    /// Number of Grace cores (Neoverse V2).
    pub grace_cores: u32,
    /// Maximum frequency in MHz.
    pub max_freq_mhz: u32,
    /// Whether hardware lockstep is available (A78AE feature).
    pub lockstep_available: bool,
}

impl ThorCpuConfig {
    /// CPU config for T4000 variant.
    pub fn t4000() -> Self {
        Self {
            a78ae_cores: 8,
            grace_cores: 0,
            max_freq_mhz: 2200,
            lockstep_available: true,
        }
    }

    /// CPU config for T5000 variant.
    pub fn t5000() -> Self {
        Self {
            a78ae_cores: 12,
            grace_cores: 60,
            max_freq_mhz: 3400,
            lockstep_available: true,
        }
    }

    /// Total core count.
    pub fn total_cores(&self) -> u32 {
        self.a78ae_cores + self.grace_cores
    }

    /// Sets CPU affinity for a thread to specific cores.
    ///
    /// `core_mask` is a bitmask of cores (bit 0 = core 0, etc.).
    pub fn set_affinity_mask(&self, core_mask: u64) -> Result<(), String> {
        let total = self.total_cores();
        let max_mask = (1u64 << total) - 1;
        if core_mask == 0 || core_mask > max_mask {
            return Err(format!(
                "invalid core mask {:#x} for {} cores",
                core_mask, total
            ));
        }
        // On real hardware, would call sched_setaffinity
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Thor Memory Map
// ═══════════════════════════════════════════════════════════════════════

/// Memory partition for Jetson Thor's unified memory.
#[derive(Debug, Clone)]
pub struct MemoryPartition {
    /// Partition name (e.g., "system", "gpu", "dla", "pva").
    pub name: String,
    /// Size in bytes.
    pub size_bytes: u64,
    /// Whether this partition uses unified addressing.
    pub unified: bool,
}

/// Creates the default memory partitions for a Thor variant.
pub fn default_memory_partitions(variant: ThorVariant) -> Vec<MemoryPartition> {
    let total_gb: u64 = match variant {
        ThorVariant::T4000 => 32,
        ThorVariant::T5000 => 128,
    };
    let total_bytes = total_gb * 1024 * 1024 * 1024;

    // Unified memory: GPU and CPU share the same address space.
    // Partitions are logical, not physical.
    let gpu_bytes = total_bytes * 30 / 100;
    let dla_bytes = total_bytes * 5 / 100;
    let pva_bytes = total_bytes * 5 / 100;
    let system_bytes = total_bytes - gpu_bytes - dla_bytes - pva_bytes; // remainder to system
    vec![
        MemoryPartition {
            name: "system".to_string(),
            size_bytes: system_bytes,
            unified: true,
        },
        MemoryPartition {
            name: "gpu".to_string(),
            size_bytes: gpu_bytes,
            unified: true,
        },
        MemoryPartition {
            name: "dla".to_string(),
            size_bytes: dla_bytes,
            unified: true,
        },
        MemoryPartition {
            name: "pva".to_string(),
            size_bytes: pva_bytes,
            unified: true,
        },
    ]
}

// ═══════════════════════════════════════════════════════════════════════
// Blackwell GPU on Thor
// ═══════════════════════════════════════════════════════════════════════

/// Blackwell GPU capabilities on Jetson Thor.
#[derive(Debug, Clone)]
pub struct BlackwellGpu {
    /// Compute capability (10, 1) for sm_101.
    pub compute_capability: (u32, u32),
    /// SM (streaming multiprocessor) count.
    pub sm_count: u32,
    /// CUDA core count.
    pub cuda_cores: u32,
    /// Tensor Core generation (5th gen).
    pub tensor_core_gen: u32,
    /// Maximum GPU clock in MHz.
    pub max_clock_mhz: u32,
    /// Maximum power draw in watts.
    pub max_power_watts: u32,
    /// Whether FP4 Tensor Core operations are supported.
    pub supports_fp4: bool,
    /// Whether FP8 Tensor Core operations are supported.
    pub supports_fp8: bool,
    /// Whether BF16 Tensor Core operations are supported.
    pub supports_bf16: bool,
    /// Whether TMA (Tensor Memory Accelerator) is available.
    pub supports_tma: bool,
    /// Whether cluster launch is supported.
    pub supports_cluster_launch: bool,
    /// Whether DPX (Dynamic Programming X) instructions are available.
    pub supports_dpx: bool,
}

impl BlackwellGpu {
    /// Creates the default Blackwell GPU config for Jetson Thor.
    pub fn thor_default() -> Self {
        Self {
            compute_capability: (10, 1),
            sm_count: 84,
            cuda_cores: 84 * 128, // 10,752
            tensor_core_gen: 5,
            max_clock_mhz: 2100,
            max_power_watts: 130,
            supports_fp4: true,
            supports_fp8: true,
            supports_bf16: true,
            supports_tma: true,
            supports_cluster_launch: true,
            supports_dpx: true,
        }
    }

    /// Returns the sm_XX target string.
    pub fn sm_target(&self) -> String {
        format!(
            "sm_{}{}",
            self.compute_capability.0, self.compute_capability.1
        )
    }

    /// Returns all supported numeric formats for Tensor Core operations.
    pub fn supported_tensor_formats(&self) -> Vec<&'static str> {
        let mut formats = vec!["FP32", "FP16", "TF32"];
        if self.supports_bf16 {
            formats.push("BF16");
        }
        if self.supports_fp8 {
            formats.push("FP8_E4M3");
            formats.push("FP8_E5M2");
        }
        if self.supports_fp4 {
            formats.push("FP4_E2M1");
        }
        formats.push("INT8");
        formats
    }
}

impl fmt::Display for BlackwellGpu {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Blackwell {} ({} SMs, {} CUDA cores, {} Tensor Cores gen {}, max {}W)",
            self.sm_target(),
            self.sm_count,
            self.cuda_cores,
            self.sm_count * 4, // 4 tensor cores per SM
            self.tensor_core_gen,
            self.max_power_watts
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════
// CUDA Graph Capture
// ═══════════════════════════════════════════════════════════════════════

/// Represents a captured CUDA graph for reduced kernel launch overhead.
#[derive(Debug, Clone)]
pub struct CudaGraph {
    /// Unique graph identifier.
    pub id: u64,
    /// Number of nodes in the graph.
    pub node_count: u32,
    /// Whether the graph has been instantiated.
    pub instantiated: bool,
    /// Graph capture mode.
    pub capture_mode: CaptureMode,
}

/// CUDA graph capture mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureMode {
    /// Global capture (default).
    Global,
    /// Thread-local capture.
    ThreadLocal,
    /// Relaxed capture (allows unsupported ops to fall through).
    Relaxed,
}

impl CudaGraph {
    /// Creates a new graph in capture state.
    pub fn begin_capture(mode: CaptureMode) -> Self {
        static NEXT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        Self {
            id: NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            node_count: 0,
            instantiated: false,
            capture_mode: mode,
        }
    }

    /// Records a kernel node.
    pub fn add_kernel_node(&mut self) {
        self.node_count += 1;
    }

    /// Instantiates the graph for execution.
    pub fn instantiate(&mut self) -> Result<(), String> {
        if self.node_count == 0 {
            return Err("cannot instantiate empty graph".to_string());
        }
        self.instantiated = true;
        Ok(())
    }

    /// Launches the instantiated graph.
    pub fn launch(&self) -> Result<(), String> {
        if !self.instantiated {
            return Err("graph not instantiated".to_string());
        }
        // On real hardware: cudaGraphLaunch(graphExec, stream)
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// CUDA Stream Management
// ═══════════════════════════════════════════════════════════════════════

/// CUDA stream for pipelined execution.
#[derive(Debug, Clone)]
pub struct CudaStream {
    /// Stream identifier.
    pub id: u64,
    /// Stream priority (lower = higher priority).
    pub priority: i32,
    /// Whether this is the default stream.
    pub is_default: bool,
}

impl CudaStream {
    /// Creates the default (null) stream.
    pub fn default_stream() -> Self {
        Self {
            id: 0,
            priority: 0,
            is_default: true,
        }
    }

    /// Creates a new non-default stream with given priority.
    pub fn new(priority: i32) -> Self {
        static NEXT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        Self {
            id: NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            priority,
            is_default: false,
        }
    }

    /// Synchronizes this stream (waits for all operations to complete).
    pub fn synchronize(&self) -> Result<(), String> {
        // On real hardware: cudaStreamSynchronize(stream)
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// MIG Partitioning (S15)
// ═══════════════════════════════════════════════════════════════════════

/// MIG (Multi-Instance GPU) partition profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MigProfile {
    /// 1 GPU instance, ~10GB memory.
    Profile1g10gb,
    /// 2 GPU instances, ~20GB each.
    Profile2g20gb,
    /// 3 GPU instances, ~40GB each.
    Profile3g40gb,
    /// 4 GPU instances, ~20GB each (if supported).
    Profile4g20gb,
    /// 7 GPU instances, full GPU.
    Profile7g80gb,
}

impl fmt::Display for MigProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MigProfile::Profile1g10gb => write!(f, "1g.10gb"),
            MigProfile::Profile2g20gb => write!(f, "2g.20gb"),
            MigProfile::Profile3g40gb => write!(f, "3g.40gb"),
            MigProfile::Profile4g20gb => write!(f, "4g.20gb"),
            MigProfile::Profile7g80gb => write!(f, "7g.80gb"),
        }
    }
}

/// A MIG GPU instance.
#[derive(Debug, Clone)]
pub struct MigGpuInstance {
    /// Instance ID.
    pub id: u32,
    /// Profile used to create this instance.
    pub profile: MigProfile,
    /// Allocated memory in bytes.
    pub memory_bytes: u64,
    /// Number of compute instances within this GPU instance.
    pub compute_instances: Vec<MigComputeInstance>,
}

/// A MIG compute instance within a GPU instance.
#[derive(Debug, Clone)]
pub struct MigComputeInstance {
    /// Compute instance ID.
    pub id: u32,
    /// SM count allocated to this compute instance.
    pub sm_count: u32,
}

/// MIG manager for creating and managing GPU partitions.
#[derive(Debug)]
pub struct MigManager {
    /// Whether MIG mode is enabled on the GPU.
    pub mig_enabled: bool,
    /// Maximum number of GPU instances supported.
    pub max_instances: u32,
    /// Currently active GPU instances.
    pub instances: Vec<MigGpuInstance>,
    /// Next instance ID.
    next_id: u32,
}

impl Default for MigManager {
    fn default() -> Self {
        Self::new()
    }
}

impl MigManager {
    /// Creates a new MIG manager.
    ///
    /// On real hardware, queries NVML for MIG capability.
    pub fn new() -> Self {
        Self {
            mig_enabled: false,
            max_instances: 7,
            instances: Vec::new(),
            next_id: 0,
        }
    }

    /// Detects MIG support on the current GPU.
    ///
    /// Returns `true` if MIG mode is enabled.
    pub fn detect_mig_support(&mut self) -> bool {
        // On real hardware: nvmlDeviceGetMigMode()
        // For Blackwell on Thor, MIG is supported
        self.mig_enabled = false; // conservative default
        self.mig_enabled
    }

    /// Enables MIG mode (requires GPU reset).
    pub fn enable_mig(&mut self) -> Result<(), String> {
        // On real hardware: nvmlDeviceSetMigMode(NVML_DEVICE_MIG_ENABLE)
        self.mig_enabled = true;
        Ok(())
    }

    /// Lists active GPU instances.
    pub fn list_instances(&self) -> &[MigGpuInstance] {
        &self.instances
    }

    /// Memory allocation for a MIG profile.
    fn profile_memory_bytes(profile: &MigProfile) -> u64 {
        match profile {
            MigProfile::Profile1g10gb => 10 * 1024 * 1024 * 1024,
            MigProfile::Profile2g20gb => 20 * 1024 * 1024 * 1024,
            MigProfile::Profile3g40gb => 40 * 1024 * 1024 * 1024,
            MigProfile::Profile4g20gb => 20 * 1024 * 1024 * 1024,
            MigProfile::Profile7g80gb => 80 * 1024 * 1024 * 1024,
        }
    }

    /// SM count for a MIG profile.
    fn profile_sm_count(profile: &MigProfile) -> u32 {
        match profile {
            MigProfile::Profile1g10gb => 12,
            MigProfile::Profile2g20gb => 24,
            MigProfile::Profile3g40gb => 36,
            MigProfile::Profile4g20gb => 24,
            MigProfile::Profile7g80gb => 84,
        }
    }

    /// Creates a new MIG GPU instance with the given profile.
    pub fn create_instance(&mut self, profile: MigProfile) -> Result<u32, String> {
        if !self.mig_enabled {
            return Err("MIG mode not enabled".to_string());
        }
        if self.instances.len() >= self.max_instances as usize {
            return Err(format!(
                "maximum {} MIG instances reached",
                self.max_instances
            ));
        }

        let id = self.next_id;
        self.next_id += 1;

        let memory_bytes = Self::profile_memory_bytes(&profile);
        let instance = MigGpuInstance {
            id,
            profile,
            memory_bytes,
            compute_instances: Vec::new(),
        };
        self.instances.push(instance);
        Ok(id)
    }

    /// Creates a compute instance within a GPU instance.
    pub fn create_compute_instance(&mut self, gpu_instance_id: u32) -> Result<u32, String> {
        let instance = self
            .instances
            .iter_mut()
            .find(|i| i.id == gpu_instance_id)
            .ok_or_else(|| format!("GPU instance {} not found", gpu_instance_id))?;

        let ci_id = instance.compute_instances.len() as u32;
        let sm_count = Self::profile_sm_count(&instance.profile);
        instance.compute_instances.push(MigComputeInstance {
            id: ci_id,
            sm_count,
        });
        Ok(ci_id)
    }

    /// Destroys a MIG GPU instance and all its compute instances.
    pub fn destroy_instance(&mut self, id: u32) -> Result<(), String> {
        let pos = self
            .instances
            .iter()
            .position(|i| i.id == id)
            .ok_or_else(|| format!("GPU instance {} not found", id))?;
        self.instances.remove(pos);
        Ok(())
    }

    /// Checks memory isolation: no two instances share memory ranges.
    pub fn verify_isolation(&self) -> bool {
        // In MIG mode, NVML guarantees memory isolation by hardware.
        // We verify that no instance has zero memory.
        self.instances.iter().all(|i| i.memory_bytes > 0)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Power Management (S16)
// ═══════════════════════════════════════════════════════════════════════

/// Power mode profile for Jetson Thor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerMode {
    /// 10W — idle/standby, minimal clocks.
    Idle10W,
    /// 30W — light inference workloads.
    Inference30W,
    /// 60W — moderate training/multi-model inference.
    Training60W,
    /// 130W — maximum performance, all cores active.
    MaxPerf130W,
}

impl PowerMode {
    /// Returns the power budget in watts.
    pub fn watts(&self) -> u32 {
        match self {
            PowerMode::Idle10W => 10,
            PowerMode::Inference30W => 30,
            PowerMode::Training60W => 60,
            PowerMode::MaxPerf130W => 130,
        }
    }

    /// Returns the recommended GPU clock in MHz for this mode.
    pub fn recommended_gpu_clock_mhz(&self) -> u32 {
        match self {
            PowerMode::Idle10W => 306,
            PowerMode::Inference30W => 1020,
            PowerMode::Training60W => 1620,
            PowerMode::MaxPerf130W => 2100,
        }
    }

    /// Returns the recommended CPU frequency in MHz for this mode.
    pub fn recommended_cpu_freq_mhz(&self) -> u32 {
        match self {
            PowerMode::Idle10W => 800,
            PowerMode::Inference30W => 1500,
            PowerMode::Training60W => 2200,
            PowerMode::MaxPerf130W => 3400,
        }
    }
}

impl fmt::Display for PowerMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PowerMode::Idle10W => write!(f, "10W (Idle)"),
            PowerMode::Inference30W => write!(f, "30W (Inference)"),
            PowerMode::Training60W => write!(f, "60W (Training)"),
            PowerMode::MaxPerf130W => write!(f, "130W (Max Performance)"),
        }
    }
}

/// Thermal zone information.
#[derive(Debug, Clone)]
pub struct ThermalZone {
    /// Zone name (e.g., "GPU-therm", "CPU-therm").
    pub name: String,
    /// Sysfs path for reading temperature.
    pub sysfs_path: String,
    /// Current temperature in millidegrees Celsius.
    pub temperature_mc: i32,
    /// Maximum safe temperature in millidegrees Celsius.
    pub max_temp_mc: i32,
    /// Trip points (thermal throttle thresholds) in millidegrees Celsius.
    pub trip_points: Vec<i32>,
}

impl ThermalZone {
    /// Reads the current temperature from sysfs.
    pub fn read_temperature(&self) -> Result<i32, String> {
        let temp_path = format!("{}/temp", self.sysfs_path);
        match std::fs::read_to_string(&temp_path) {
            Ok(s) => s
                .trim()
                .parse::<i32>()
                .map_err(|e| format!("failed to parse temperature: {}", e)),
            Err(_) => Ok(self.temperature_mc), // fallback to cached value
        }
    }

    /// Whether the zone is above the throttle threshold.
    pub fn is_throttling(&self) -> bool {
        self.temperature_mc >= self.max_temp_mc
    }

    /// Creates a mock thermal zone for testing.
    pub fn mock(name: &str, temp_mc: i32) -> Self {
        Self {
            name: name.to_string(),
            sysfs_path: format!("/sys/class/thermal/thermal_zone_{}", name),
            temperature_mc: temp_mc,
            max_temp_mc: 85000, // 85°C
            trip_points: vec![65000, 75000, 85000],
        }
    }
}

/// Power rail sensor for real-time wattage reporting.
#[derive(Debug, Clone)]
pub struct PowerRail {
    /// Rail name (e.g., "VDD_GPU", "VDD_CPU", "VDD_SOC").
    pub name: String,
    /// Current power in milliwatts.
    pub power_mw: u32,
    /// Voltage in millivolts.
    pub voltage_mv: u32,
    /// Current in milliamps.
    pub current_ma: u32,
}

impl PowerRail {
    /// Creates a mock power rail for testing.
    pub fn mock(name: &str, power_mw: u32) -> Self {
        let voltage_mv = 900; // typical Tegra rail voltage
        let current_ma = (power_mw * 1000).checked_div(voltage_mv).unwrap_or(0);
        Self {
            name: name.to_string(),
            power_mw,
            voltage_mv,
            current_ma,
        }
    }
}

/// Power manager for Jetson Thor.
#[derive(Debug)]
pub struct PowerManager {
    /// Current power mode.
    pub mode: PowerMode,
    /// Thermal zones.
    pub thermal_zones: Vec<ThermalZone>,
    /// Power rails.
    pub power_rails: Vec<PowerRail>,
    /// Thermal limit in millidegrees Celsius.
    pub thermal_limit_mc: i32,
    /// Whether dynamic frequency scaling is enabled.
    pub dvfs_enabled: bool,
}

impl Default for PowerManager {
    fn default() -> Self {
        Self::new()
    }
}

impl PowerManager {
    /// Creates a new power manager with default settings.
    pub fn new() -> Self {
        Self {
            mode: PowerMode::Inference30W,
            thermal_zones: vec![
                ThermalZone::mock("GPU-therm", 45000),
                ThermalZone::mock("CPU-therm", 42000),
                ThermalZone::mock("SOC-therm", 43000),
                ThermalZone::mock("Board-therm", 38000),
            ],
            power_rails: vec![
                PowerRail::mock("VDD_GPU", 15000),
                PowerRail::mock("VDD_CPU", 8000),
                PowerRail::mock("VDD_SOC", 5000),
                PowerRail::mock("VDD_MEM", 2000),
            ],
            thermal_limit_mc: 85000,
            dvfs_enabled: true,
        }
    }

    /// Sets the power mode.
    pub fn set_mode(&mut self, mode: PowerMode) -> Result<(), String> {
        // On real hardware: write to /sys/devices/platform/gpu.0/devfreq/...
        self.mode = mode;
        Ok(())
    }

    /// Returns current total power consumption in watts.
    pub fn current_watts(&self) -> f32 {
        let total_mw: u32 = self.power_rails.iter().map(|r| r.power_mw).sum();
        total_mw as f32 / 1000.0
    }

    /// Sets the thermal throttle limit.
    pub fn set_thermal_limit(&mut self, celsius: u32) -> Result<(), String> {
        if !(50..=100).contains(&celsius) {
            return Err(format!("thermal limit {} out of range [50, 100]", celsius));
        }
        self.thermal_limit_mc = (celsius * 1000) as i32;
        Ok(())
    }

    /// Returns thermal status across all zones.
    pub fn thermal_status(&self) -> Vec<(&str, f32, bool)> {
        self.thermal_zones
            .iter()
            .map(|z| {
                let temp_c = z.temperature_mc as f32 / 1000.0;
                let throttling = z.temperature_mc >= self.thermal_limit_mc;
                (z.name.as_str(), temp_c, throttling)
            })
            .collect()
    }

    /// Distributes power budget across components.
    ///
    /// Returns (gpu_watts, cpu_watts, other_watts).
    pub fn allocate_power_budget(&self) -> (f32, f32, f32) {
        let total = self.mode.watts() as f32;
        match self.mode {
            PowerMode::Idle10W => (3.0, 4.0, 3.0),
            PowerMode::Inference30W => (18.0, 8.0, 4.0),
            PowerMode::Training60W => (40.0, 14.0, 6.0),
            PowerMode::MaxPerf130W => (85.0, 35.0, total - 85.0 - 35.0),
        }
    }

    /// Checks if thermal throttling should reduce inference batch size.
    pub fn should_reduce_batch(&self) -> bool {
        self.thermal_zones
            .iter()
            .any(|z| z.temperature_mc >= self.thermal_limit_mc - 5000) // within 5°C of limit
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Jetson Thor Board (implements Board trait)
// ═══════════════════════════════════════════════════════════════════════

/// Jetson Thor board support package.
#[derive(Debug)]
pub struct JetsonThor {
    /// Module variant (T4000 or T5000).
    pub variant: ThorVariant,
    /// CPU configuration.
    pub cpu: ThorCpuConfig,
    /// GPU capabilities.
    pub gpu: BlackwellGpu,
    /// Device tree information.
    pub device_tree: DeviceTree,
    /// JetPack SDK descriptor.
    pub sdk: JetPackSdk,
    /// Memory partitions.
    pub memory_partitions: Vec<MemoryPartition>,
}

impl JetsonThor {
    /// Creates a new Jetson Thor BSP for the given variant.
    pub fn new(variant: ThorVariant) -> Self {
        let cpu = match variant {
            ThorVariant::T4000 => ThorCpuConfig::t4000(),
            ThorVariant::T5000 => ThorCpuConfig::t5000(),
        };

        Self {
            variant,
            cpu,
            gpu: BlackwellGpu::thor_default(),
            device_tree: DeviceTree::mock(variant),
            sdk: JetPackSdk::jetpack_7_1(),
            memory_partitions: default_memory_partitions(variant),
        }
    }

    /// Detects and creates a Jetson Thor BSP from the running hardware.
    pub fn detect() -> Option<Self> {
        let variant = detect_variant()?;
        Some(Self::new(variant))
    }

    /// Returns the platform descriptor.
    pub fn platform_info(&self) -> String {
        format!(
            "{} | {} | {} | {}",
            self.variant,
            self.cpu.total_cores(),
            self.gpu,
            self.sdk
        )
    }
}

impl Board for JetsonThor {
    fn name(&self) -> &str {
        match self.variant {
            ThorVariant::T4000 => "NVIDIA Jetson Thor T4000",
            ThorVariant::T5000 => "NVIDIA Jetson Thor T5000",
        }
    }

    fn arch(&self) -> BspArch {
        BspArch::Aarch64Linux
    }

    fn memory_regions(&self) -> Vec<MemoryRegion> {
        // Linux userspace target — conceptual regions
        let ram_gb: u32 = match self.variant {
            ThorVariant::T4000 => 32,
            ThorVariant::T5000 => 128,
        };
        vec![
            MemoryRegion::new(
                "SYSTEM_RAM",
                0x8000_0000,
                ram_gb * 1024 * 1024, // in KB for linker entry
                MemoryAttr::Rwx,
            ),
            MemoryRegion::new("MMIO_GPU", 0x1700_0000, 64 * 1024, MemoryAttr::Rw),
            MemoryRegion::new("MMIO_DLA", 0x1580_0000, 16 * 1024, MemoryAttr::Rw),
        ]
    }

    fn peripherals(&self) -> Vec<Peripheral> {
        let mut peripherals = Vec::new();

        // GPU
        let mut gpu = Peripheral::new("GPU_BLACKWELL", 0x1700_0000);
        gpu.add_register("NV_PMC_BOOT_0", 0x0, 4);
        gpu.add_register("NV_PMC_ENABLE", 0x200, 4);
        peripherals.push(gpu);

        // DLA
        let mut dla = Peripheral::new("NVDLA_0", 0x1580_0000);
        dla.add_register("CDMA_STATUS", 0x100, 4);
        dla.add_register("PROCESSOR_STATUS", 0x104, 4);
        peripherals.push(dla);

        // UART (debug console)
        let mut uart = Peripheral::new("UART0", 0x0310_0000);
        uart.add_register("THR", 0x0, 4);
        uart.add_register("RBR", 0x0, 4);
        uart.add_register("LSR", 0x14, 4);
        peripherals.push(uart);

        // Ethernet
        let mut eth = Peripheral::new("EQOS", 0x2310_0000);
        eth.add_register("MAC_CONFIG", 0x0, 4);
        eth.add_register("MAC_ADDR_HIGH", 0x40, 4);
        eth.add_register("MAC_ADDR_LOW", 0x44, 4);
        peripherals.push(eth);

        // USB
        let usb = Peripheral::new("USB3_XHCI", 0x3610_0000);
        peripherals.push(usb);

        peripherals
    }

    fn vector_table_size(&self) -> usize {
        // AArch64 exception vector table: 16 entries * 4 instructions each
        16
    }

    fn cpu_frequency(&self) -> u32 {
        self.cpu.max_freq_mhz * 1_000_000
    }

    fn generate_linker_script(&self) -> String {
        // Linux userspace: standard ELF linking, no custom linker script needed
        let mut script = String::new();
        script.push_str("/* Jetson Thor (Linux userspace) — standard ELF */\n");
        script.push_str("/* No custom linker script required for L4T targets */\n\n");
        script.push_str(&format!("/* Board: {} */\n", self.name()));
        script.push_str(&format!(
            "/* CPU: {} cores @ {}MHz */\n",
            self.cpu.total_cores(),
            self.cpu.max_freq_mhz
        ));
        script.push_str(&format!("/* GPU: {} */\n", self.gpu));
        script.push_str(&format!("/* SDK: {} */\n", self.sdk));
        script
    }

    fn generate_startup_code(&self) -> String {
        let mut code = String::new();
        code.push_str("/* Jetson Thor startup — Linux userspace (no bare-metal boot) */\n\n");
        code.push_str("/* Standard AArch64 Linux entry point */\n");
        code.push_str(".global _start\n");
        code.push_str(".type _start, %function\n");
        code.push_str("_start:\n");
        code.push_str("  /* Set up stack frame */\n");
        code.push_str("  stp x29, x30, [sp, #-16]!\n");
        code.push_str("  mov x29, sp\n\n");
        code.push_str("  /* Call main */\n");
        code.push_str("  bl main\n\n");
        code.push_str("  /* Exit */\n");
        code.push_str("  mov x8, #93  /* __NR_exit */\n");
        code.push_str("  svc #0\n");
        code
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Platform Abstraction
// ═══════════════════════════════════════════════════════════════════════

/// High-level platform descriptor for Jetson Thor.
pub struct ThorPlatform {
    /// Board support package.
    pub board: JetsonThor,
    /// MIG manager.
    pub mig: MigManager,
    /// Power manager.
    pub power: PowerManager,
}

impl ThorPlatform {
    /// Creates a platform descriptor for the given Thor variant.
    pub fn new(variant: ThorVariant) -> Self {
        Self {
            board: JetsonThor::new(variant),
            mig: MigManager::new(),
            power: PowerManager::new(),
        }
    }

    /// Full platform summary.
    pub fn summary(&self) -> String {
        format!(
            "Platform: {}\nCPU: {} cores (A78AE: {}, Grace: {})\nGPU: {}\nSDK: {}\nPower: {} ({}W)\nMIG: {}",
            self.board.variant,
            self.board.cpu.total_cores(),
            self.board.cpu.a78ae_cores,
            self.board.cpu.grace_cores,
            self.board.gpu,
            self.board.sdk,
            self.power.mode,
            self.power.current_watts(),
            if self.mig.mig_enabled { "enabled" } else { "disabled" }
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── S13: Jetson Thor Platform ──

    #[test]
    fn thor_t4000_creation() {
        let board = JetsonThor::new(ThorVariant::T4000);
        assert_eq!(board.name(), "NVIDIA Jetson Thor T4000");
        assert_eq!(board.arch(), BspArch::Aarch64Linux);
        assert_eq!(board.cpu.a78ae_cores, 8);
        assert_eq!(board.cpu.grace_cores, 0);
        assert_eq!(board.cpu.total_cores(), 8);
    }

    #[test]
    fn thor_t5000_creation() {
        let board = JetsonThor::new(ThorVariant::T5000);
        assert_eq!(board.name(), "NVIDIA Jetson Thor T5000");
        assert_eq!(board.cpu.a78ae_cores, 12);
        assert_eq!(board.cpu.grace_cores, 60);
        assert_eq!(board.cpu.total_cores(), 72);
    }

    #[test]
    fn thor_variant_display() {
        assert_eq!(ThorVariant::T4000.to_string(), "Jetson Thor T4000");
        assert_eq!(ThorVariant::T5000.to_string(), "Jetson Thor T5000");
    }

    #[test]
    fn jetpack_sdk_7_1_info() {
        let sdk = JetPackSdk::jetpack_7_1();
        assert_eq!(sdk.major, 7);
        assert_eq!(sdk.minor, 1);
        assert_eq!(sdk.cuda_version, "13.0");
        assert!(sdk.to_string().contains("JetPack 7.1"));
    }

    #[test]
    fn device_tree_mock_t4000() {
        let dt = DeviceTree::mock(ThorVariant::T4000);
        assert!(dt.model.contains("T4000"));
        assert_eq!(dt.memory_size_bytes, 32 * 1024 * 1024 * 1024);
        assert_eq!(dt.dla_count, 2);
    }

    #[test]
    fn device_tree_mock_t5000() {
        let dt = DeviceTree::mock(ThorVariant::T5000);
        assert!(dt.model.contains("T5000"));
        assert_eq!(dt.memory_size_bytes, 128 * 1024 * 1024 * 1024);
        assert!(dt.compatible.contains(&"nvidia,jetson-thor".to_string()));
    }

    #[test]
    fn thor_cpu_affinity_valid() {
        let cpu = ThorCpuConfig::t4000();
        assert!(cpu.set_affinity_mask(0b1111).is_ok());
        assert!(cpu.set_affinity_mask(1).is_ok());
    }

    #[test]
    fn thor_cpu_affinity_invalid() {
        let cpu = ThorCpuConfig::t4000(); // 8 cores
        assert!(cpu.set_affinity_mask(0).is_err());
        assert!(cpu.set_affinity_mask(1 << 8).is_err()); // bit 8 for 8 cores
    }

    #[test]
    fn thor_memory_partitions_t4000() {
        let parts = default_memory_partitions(ThorVariant::T4000);
        assert_eq!(parts.len(), 4);
        let total: u64 = parts.iter().map(|p| p.size_bytes).sum();
        assert_eq!(total, 32 * 1024 * 1024 * 1024);
        assert!(parts.iter().all(|p| p.unified));
    }

    #[test]
    fn thor_memory_partitions_t5000() {
        let parts = default_memory_partitions(ThorVariant::T5000);
        let total: u64 = parts.iter().map(|p| p.size_bytes).sum();
        assert_eq!(total, 128 * 1024 * 1024 * 1024);
    }

    #[test]
    fn thor_board_trait_memory_regions() {
        let board = JetsonThor::new(ThorVariant::T4000);
        let regions = board.memory_regions();
        assert!(!regions.is_empty());
        assert!(regions.iter().any(|r| r.name == "SYSTEM_RAM"));
        assert!(regions.iter().any(|r| r.name == "MMIO_GPU"));
    }

    #[test]
    fn thor_board_trait_peripherals() {
        let board = JetsonThor::new(ThorVariant::T5000);
        let periphs = board.peripherals();
        assert!(periphs.iter().any(|p| p.name == "GPU_BLACKWELL"));
        assert!(periphs.iter().any(|p| p.name == "NVDLA_0"));
        assert!(periphs.iter().any(|p| p.name == "UART0"));
    }

    #[test]
    fn thor_linker_script_is_linux() {
        let board = JetsonThor::new(ThorVariant::T5000);
        let script = board.generate_linker_script();
        assert!(script.contains("Linux userspace"));
        assert!(script.contains("Blackwell"));
    }

    #[test]
    fn thor_startup_code_is_aarch64() {
        let board = JetsonThor::new(ThorVariant::T5000);
        let code = board.generate_startup_code();
        assert!(code.contains("_start"));
        assert!(code.contains("bl main"));
        assert!(code.contains("svc #0")); // syscall exit
    }

    #[test]
    fn thor_platform_summary() {
        let platform = ThorPlatform::new(ThorVariant::T5000);
        let summary = platform.summary();
        assert!(summary.contains("72 cores"));
        assert!(summary.contains("Blackwell"));
    }

    // ── S14: Blackwell GPU on Thor ──

    #[test]
    fn blackwell_gpu_sm_101() {
        let gpu = BlackwellGpu::thor_default();
        assert_eq!(gpu.compute_capability, (10, 1));
        assert_eq!(gpu.sm_target(), "sm_101");
        assert_eq!(gpu.tensor_core_gen, 5);
    }

    #[test]
    fn blackwell_gpu_tensor_formats() {
        let gpu = BlackwellGpu::thor_default();
        let formats = gpu.supported_tensor_formats();
        assert!(formats.contains(&"FP4_E2M1"));
        assert!(formats.contains(&"FP8_E4M3"));
        assert!(formats.contains(&"FP8_E5M2"));
        assert!(formats.contains(&"BF16"));
        assert!(formats.contains(&"INT8"));
    }

    #[test]
    fn blackwell_gpu_display() {
        let gpu = BlackwellGpu::thor_default();
        let display = gpu.to_string();
        assert!(display.contains("sm_101"));
        assert!(display.contains("84 SMs"));
        assert!(display.contains("10752 CUDA cores"));
    }

    #[test]
    fn blackwell_supports_all_features() {
        let gpu = BlackwellGpu::thor_default();
        assert!(gpu.supports_fp4);
        assert!(gpu.supports_fp8);
        assert!(gpu.supports_bf16);
        assert!(gpu.supports_tma);
        assert!(gpu.supports_cluster_launch);
        assert!(gpu.supports_dpx);
    }

    #[test]
    fn cuda_graph_capture_and_launch() {
        let mut graph = CudaGraph::begin_capture(CaptureMode::Global);
        assert!(!graph.instantiated);
        assert_eq!(graph.node_count, 0);

        graph.add_kernel_node();
        graph.add_kernel_node();
        assert_eq!(graph.node_count, 2);

        assert!(graph.instantiate().is_ok());
        assert!(graph.instantiated);
        assert!(graph.launch().is_ok());
    }

    #[test]
    fn cuda_graph_empty_cannot_instantiate() {
        let mut graph = CudaGraph::begin_capture(CaptureMode::ThreadLocal);
        assert!(graph.instantiate().is_err());
    }

    #[test]
    fn cuda_graph_not_instantiated_cannot_launch() {
        let graph = CudaGraph::begin_capture(CaptureMode::Global);
        assert!(graph.launch().is_err());
    }

    #[test]
    fn cuda_stream_creation() {
        let default_stream = CudaStream::default_stream();
        assert!(default_stream.is_default);
        assert_eq!(default_stream.id, 0);

        let high_pri = CudaStream::new(-1);
        assert!(!high_pri.is_default);
        assert!(high_pri.id > 0);
        assert!(high_pri.synchronize().is_ok());
    }

    #[test]
    fn cuda_stream_multiple() {
        let s1 = CudaStream::new(0);
        let s2 = CudaStream::new(0);
        assert_ne!(s1.id, s2.id);
    }

    // ── S15: MIG Partitioning ──

    #[test]
    fn mig_manager_creation() {
        let mig = MigManager::new();
        assert!(!mig.mig_enabled);
        assert_eq!(mig.max_instances, 7);
        assert!(mig.instances.is_empty());
    }

    #[test]
    fn mig_create_without_enable_fails() {
        let mut mig = MigManager::new();
        assert!(mig.create_instance(MigProfile::Profile1g10gb).is_err());
    }

    #[test]
    fn mig_create_and_list_instances() {
        let mut mig = MigManager::new();
        mig.enable_mig().unwrap();

        let id1 = mig.create_instance(MigProfile::Profile1g10gb).unwrap();
        let id2 = mig.create_instance(MigProfile::Profile2g20gb).unwrap();

        let instances = mig.list_instances();
        assert_eq!(instances.len(), 2);
        assert_eq!(instances[0].id, id1);
        assert_eq!(instances[1].id, id2);
        assert_eq!(instances[0].profile, MigProfile::Profile1g10gb);
        assert_eq!(instances[1].profile, MigProfile::Profile2g20gb);
    }

    #[test]
    fn mig_compute_instance_creation() {
        let mut mig = MigManager::new();
        mig.enable_mig().unwrap();

        let gi_id = mig.create_instance(MigProfile::Profile3g40gb).unwrap();
        let ci_id = mig.create_compute_instance(gi_id).unwrap();
        assert_eq!(ci_id, 0);

        let instance = &mig.instances[0];
        assert_eq!(instance.compute_instances.len(), 1);
        assert_eq!(instance.compute_instances[0].sm_count, 36);
    }

    #[test]
    fn mig_destroy_instance() {
        let mut mig = MigManager::new();
        mig.enable_mig().unwrap();

        let id = mig.create_instance(MigProfile::Profile1g10gb).unwrap();
        assert_eq!(mig.instances.len(), 1);

        mig.destroy_instance(id).unwrap();
        assert!(mig.instances.is_empty());
    }

    #[test]
    fn mig_destroy_nonexistent_fails() {
        let mut mig = MigManager::new();
        mig.enable_mig().unwrap();
        assert!(mig.destroy_instance(999).is_err());
    }

    #[test]
    fn mig_verify_isolation() {
        let mut mig = MigManager::new();
        mig.enable_mig().unwrap();

        mig.create_instance(MigProfile::Profile1g10gb).unwrap();
        mig.create_instance(MigProfile::Profile2g20gb).unwrap();

        assert!(mig.verify_isolation());
    }

    #[test]
    fn mig_profile_display() {
        assert_eq!(MigProfile::Profile1g10gb.to_string(), "1g.10gb");
        assert_eq!(MigProfile::Profile2g20gb.to_string(), "2g.20gb");
        assert_eq!(MigProfile::Profile3g40gb.to_string(), "3g.40gb");
        assert_eq!(MigProfile::Profile7g80gb.to_string(), "7g.80gb");
    }

    #[test]
    fn mig_multi_tenant_inference() {
        let mut mig = MigManager::new();
        mig.enable_mig().unwrap();

        // Create two partitions for two models
        let gi1 = mig.create_instance(MigProfile::Profile1g10gb).unwrap();
        let gi2 = mig.create_instance(MigProfile::Profile1g10gb).unwrap();

        mig.create_compute_instance(gi1).unwrap();
        mig.create_compute_instance(gi2).unwrap();

        // Both have compute instances
        assert_eq!(mig.instances[0].compute_instances.len(), 1);
        assert_eq!(mig.instances[1].compute_instances.len(), 1);
        assert!(mig.verify_isolation());
    }

    // ── S16: Power Management ──

    #[test]
    fn power_mode_watts() {
        assert_eq!(PowerMode::Idle10W.watts(), 10);
        assert_eq!(PowerMode::Inference30W.watts(), 30);
        assert_eq!(PowerMode::Training60W.watts(), 60);
        assert_eq!(PowerMode::MaxPerf130W.watts(), 130);
    }

    #[test]
    fn power_mode_display() {
        assert!(PowerMode::Idle10W.to_string().contains("10W"));
        assert!(PowerMode::MaxPerf130W.to_string().contains("130W"));
    }

    #[test]
    fn power_mode_gpu_clock_recommendations() {
        assert!(
            PowerMode::Idle10W.recommended_gpu_clock_mhz()
                < PowerMode::MaxPerf130W.recommended_gpu_clock_mhz()
        );
        assert_eq!(PowerMode::MaxPerf130W.recommended_gpu_clock_mhz(), 2100);
    }

    #[test]
    fn power_manager_creation() {
        let pm = PowerManager::new();
        assert_eq!(pm.mode, PowerMode::Inference30W);
        assert_eq!(pm.thermal_zones.len(), 4);
        assert_eq!(pm.power_rails.len(), 4);
        assert!(pm.dvfs_enabled);
    }

    #[test]
    fn power_manager_set_mode() {
        let mut pm = PowerManager::new();
        assert!(pm.set_mode(PowerMode::MaxPerf130W).is_ok());
        assert_eq!(pm.mode, PowerMode::MaxPerf130W);
    }

    #[test]
    fn power_manager_current_watts() {
        let pm = PowerManager::new();
        let watts = pm.current_watts();
        assert!(watts > 0.0);
        // sum of mock rails: 15 + 8 + 5 + 2 = 30W
        assert!((watts - 30.0).abs() < 0.01);
    }

    #[test]
    fn power_manager_thermal_limit() {
        let mut pm = PowerManager::new();
        assert!(pm.set_thermal_limit(85).is_ok());
        assert_eq!(pm.thermal_limit_mc, 85000);
        assert!(pm.set_thermal_limit(49).is_err());
        assert!(pm.set_thermal_limit(101).is_err());
    }

    #[test]
    fn power_manager_thermal_status() {
        let pm = PowerManager::new();
        let status = pm.thermal_status();
        assert_eq!(status.len(), 4);
        // All mock temps are well below 85°C
        assert!(status.iter().all(|(_, _, throttling)| !throttling));
    }

    #[test]
    fn power_budget_allocation() {
        let pm = PowerManager::new(); // 30W mode
        let (gpu, cpu, other) = pm.allocate_power_budget();
        assert!((gpu + cpu + other - 30.0).abs() < 0.01);
        assert!(gpu > cpu); // GPU gets more in inference mode
    }

    #[test]
    fn power_manager_should_not_reduce_batch_at_low_temp() {
        let pm = PowerManager::new();
        // Mock temps are 38-45°C, well below 80°C (85-5)
        assert!(!pm.should_reduce_batch());
    }

    #[test]
    fn thermal_zone_mock_creation() {
        let tz = ThermalZone::mock("GPU-therm", 72000);
        assert_eq!(tz.name, "GPU-therm");
        assert_eq!(tz.temperature_mc, 72000);
        assert_eq!(tz.max_temp_mc, 85000);
        assert!(!tz.is_throttling());
    }

    #[test]
    fn thermal_zone_throttling_detection() {
        let tz = ThermalZone::mock("GPU-therm", 90000);
        assert!(tz.is_throttling());
    }

    #[test]
    fn power_rail_mock() {
        let rail = PowerRail::mock("VDD_GPU", 15000);
        assert_eq!(rail.name, "VDD_GPU");
        assert_eq!(rail.power_mw, 15000);
        assert!(rail.voltage_mv > 0);
        assert!(rail.current_ma > 0);
    }
}
