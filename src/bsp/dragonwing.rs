//! Qualcomm Dragonwing IQ8 MPU board support package.
//!
//! The Dragonwing IQ8 is the MPU brain of the Arduino VENTUNO Q,
//! running ARM64 Linux with a 40 TOPS Hexagon Tensor NPU.
//!
//! # Hardware Overview
//!
//! - CPU: Octa-core ARM64 (Kryo cores)
//! - GPU: Adreno @ 877MHz (Vulkan 1.3)
//! - NPU: Hexagon Tensor, 40 TOPS
//! - RAM: 16GB LPDDR5
//! - Storage: 64GB eMMC + NVMe expansion
//! - Connectivity: WiFi6, BT5.3, 2.5GbE
//!
//! This is a Linux userspace target, so memory regions are conceptual
//! (managed by the kernel) rather than physical address maps.

use super::{Board, BspArch, MemoryAttr, MemoryRegion, Peripheral};
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// GPU Capabilities
// ═══════════════════════════════════════════════════════════════════════

/// GPU capabilities for the Adreno GPU on Dragonwing IQ8.
#[derive(Debug, Clone)]
pub struct GpuCapabilities {
    /// GPU name.
    pub name: String,
    /// Clock speed in MHz.
    pub clock_mhz: u32,
    /// Whether Vulkan is supported.
    pub vulkan_supported: bool,
    /// Number of compute units.
    pub compute_units: u32,
}

impl GpuCapabilities {
    /// Creates GPU capabilities for the Dragonwing IQ8 Adreno GPU.
    pub fn dragonwing_default() -> Self {
        Self {
            name: "Adreno".to_string(),
            clock_mhz: 877,
            vulkan_supported: true,
            compute_units: 6,
        }
    }
}

impl fmt::Display for GpuCapabilities {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} @ {}MHz, Vulkan={}, {} CUs",
            self.name, self.clock_mhz, self.vulkan_supported, self.compute_units
        )
    }
}

/// Checks if the Adreno GPU is available via sysfs.
///
/// In simulation mode, this always returns a stub result.
/// On real hardware, it would check `/sys/class/kgsl/`.
pub fn gpu_available() -> bool {
    // Simulation: check if the sysfs path exists
    std::path::Path::new("/sys/class/kgsl/").exists()
}

/// Returns GPU capability information for the Dragonwing IQ8.
pub fn gpu_info() -> GpuCapabilities {
    GpuCapabilities::dragonwing_default()
}

// ═══════════════════════════════════════════════════════════════════════
// NPU Capabilities
// ═══════════════════════════════════════════════════════════════════════

/// Supported data types for NPU inference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NpuDtype {
    /// 16-bit floating point.
    F16,
    /// 8-bit integer (quantized).
    Int8,
    /// 4-bit integer (quantized).
    Int4,
}

impl fmt::Display for NpuDtype {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NpuDtype::F16 => write!(f, "F16"),
            NpuDtype::Int8 => write!(f, "INT8"),
            NpuDtype::Int4 => write!(f, "INT4"),
        }
    }
}

/// NPU capabilities for the Hexagon Tensor processor.
#[derive(Debug, Clone)]
pub struct NpuCapabilities {
    /// NPU name.
    pub name: String,
    /// Performance in TOPS (tera operations per second).
    pub tops: u32,
    /// QNN SDK version string.
    pub qnn_version: String,
    /// Supported data types.
    pub supported_dtypes: Vec<NpuDtype>,
}

impl NpuCapabilities {
    /// Creates NPU capabilities for the Dragonwing IQ8 Hexagon Tensor.
    pub fn dragonwing_default() -> Self {
        Self {
            name: "Hexagon Tensor".to_string(),
            tops: 40,
            qnn_version: "2.25.0".to_string(),
            supported_dtypes: vec![NpuDtype::F16, NpuDtype::Int8, NpuDtype::Int4],
        }
    }
}

impl fmt::Display for NpuCapabilities {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let dtypes: Vec<String> = self
            .supported_dtypes
            .iter()
            .map(|d| d.to_string())
            .collect();
        write!(
            f,
            "{} ({} TOPS, QNN {}, dtypes: [{}])",
            self.name,
            self.tops,
            self.qnn_version,
            dtypes.join(", ")
        )
    }
}

/// Checks if the Hexagon NPU runtime is available.
///
/// In simulation mode, this always returns false.
/// On real hardware, it would check if `libQnnHtp.so` is loadable.
pub fn npu_available() -> bool {
    // Simulation: check if the QNN HTP library exists
    std::path::Path::new("/usr/lib/libQnnHtp.so").exists()
}

/// Returns NPU capability information for the Dragonwing IQ8.
pub fn npu_info() -> NpuCapabilities {
    NpuCapabilities::dragonwing_default()
}

// ═══════════════════════════════════════════════════════════════════════
// QNN Inference Wrapper (Simulation)
// ═══════════════════════════════════════════════════════════════════════

/// Errors from QNN model operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QnnError {
    /// Model file not found at the specified path.
    ModelNotFound(String),
    /// Failed to load the model into the QNN runtime.
    LoadFailed(String),
    /// Inference execution failed.
    InferenceFailed(String),
    /// Input/output shape does not match the model.
    ShapeMismatch {
        /// Expected shape.
        expected: Vec<usize>,
        /// Actual shape.
        actual: Vec<usize>,
    },
    /// QNN runtime is not available on this system.
    RuntimeNotAvailable,
}

impl fmt::Display for QnnError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            QnnError::ModelNotFound(path) => write!(f, "QNN model not found: {path}"),
            QnnError::LoadFailed(msg) => write!(f, "QNN model load failed: {msg}"),
            QnnError::InferenceFailed(msg) => write!(f, "QNN inference failed: {msg}"),
            QnnError::ShapeMismatch { expected, actual } => {
                write!(
                    f,
                    "QNN shape mismatch: expected {expected:?}, got {actual:?}"
                )
            }
            QnnError::RuntimeNotAvailable => write!(f, "QNN runtime not available"),
        }
    }
}

/// A loaded QNN model for NPU inference.
#[derive(Debug, Clone)]
pub struct QnnModel {
    /// Model name.
    pub name: String,
    /// Expected input shape.
    pub input_shape: Vec<usize>,
    /// Expected output shape.
    pub output_shape: Vec<usize>,
    /// Whether the model is loaded and ready.
    pub loaded: bool,
}

/// Loads a QNN model from the given path (simulation mode).
///
/// In simulation mode, this creates a stub model with default shapes.
/// On real hardware, this would use the QNN SDK to load the model.
pub fn qnn_load_model(path: &str) -> Result<QnnModel, QnnError> {
    if path.is_empty() {
        return Err(QnnError::ModelNotFound("(empty path)".to_string()));
    }

    // Extract model name from path
    let name = path
        .rsplit('/')
        .next()
        .unwrap_or(path)
        .trim_end_matches(".bin")
        .trim_end_matches(".so")
        .trim_end_matches(".dlc")
        .to_string();

    // Simulation: create a stub model
    Ok(QnnModel {
        name,
        input_shape: vec![1, 3, 224, 224],
        output_shape: vec![1, 1000],
        loaded: true,
    })
}

/// Runs inference on a loaded QNN model (simulation mode).
///
/// In simulation mode, this returns a zero vector of the output shape size.
/// On real hardware, this would execute the model on the Hexagon NPU.
pub fn qnn_infer(model: &QnnModel, input_data: &[f64]) -> Result<Vec<f64>, QnnError> {
    if !model.loaded {
        return Err(QnnError::LoadFailed("model not loaded".to_string()));
    }

    let expected_input_size: usize = model.input_shape.iter().product();
    if input_data.len() != expected_input_size {
        return Err(QnnError::ShapeMismatch {
            expected: model.input_shape.clone(),
            actual: vec![input_data.len()],
        });
    }

    // Simulation: return zeros matching the output shape
    let output_size: usize = model.output_shape.iter().product();
    Ok(vec![0.0; output_size])
}

// ═══════════════════════════════════════════════════════════════════════
// ONNX to QNN Export Pipeline
// ═══════════════════════════════════════════════════════════════════════

/// Generates the command to convert an ONNX model to QNN format.
///
/// Uses the `qnn-onnx-converter` tool from the Qualcomm AI Engine Direct SDK.
pub fn onnx_to_qnn_command(onnx_path: &str, output_path: &str) -> String {
    format!("qnn-onnx-converter --input_network {onnx_path} --output_path {output_path}")
}

/// Generates the command to create a QNN context binary for the HTP backend.
///
/// Uses the `qnn-context-binary-generator` tool from the QNN SDK.
pub fn qnn_context_binary_command(model_path: &str, output_path: &str) -> String {
    format!(
        "qnn-context-binary-generator \
         --model {model_path} \
         --backend libQnnHtp.so \
         --output_dir {output_path}"
    )
}

// ═══════════════════════════════════════════════════════════════════════
// Camera Integration (V4L2)
// ═══════════════════════════════════════════════════════════════════════

/// Camera pixel format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CameraFormat {
    /// YUYV (4:2:2 packed).
    Yuyv,
    /// MJPEG compressed.
    Mjpeg,
    /// NV12 (4:2:0 semi-planar).
    Nv12,
    /// Raw Bayer RGGB.
    RawBayer,
}

impl fmt::Display for CameraFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CameraFormat::Yuyv => write!(f, "YUYV"),
            CameraFormat::Mjpeg => write!(f, "MJPEG"),
            CameraFormat::Nv12 => write!(f, "NV12"),
            CameraFormat::RawBayer => write!(f, "RGGB"),
        }
    }
}

/// Camera configuration for V4L2 capture.
#[derive(Debug, Clone)]
pub struct CameraConfig {
    /// Camera index (0-2 for triple MIPI-CSI on Dragonwing).
    pub index: u8,
    /// Capture width in pixels.
    pub width: u32,
    /// Capture height in pixels.
    pub height: u32,
    /// Pixel format.
    pub format: CameraFormat,
}

impl CameraConfig {
    /// Creates a new camera configuration.
    pub fn new(index: u8, width: u32, height: u32, format: CameraFormat) -> Self {
        Self {
            index,
            width,
            height,
            format,
        }
    }

    /// Creates a default 1080p MJPEG camera configuration.
    pub fn default_1080p(index: u8) -> Self {
        Self::new(index, 1920, 1080, CameraFormat::Mjpeg)
    }
}

/// Generates a v4l2-ctl command to open/configure a camera device.
pub fn camera_open_command(config: &CameraConfig) -> String {
    format!(
        "v4l2-ctl --device /dev/video{} --set-fmt-video=width={},height={},pixelformat={}",
        config.index, config.width, config.height, config.format
    )
}

/// Generates a v4l2-ctl command to capture a single frame.
pub fn camera_capture_command(config: &CameraConfig, output_path: &str) -> String {
    format!(
        "v4l2-ctl --device /dev/video{} \
         --set-fmt-video=width={},height={},pixelformat={} \
         --stream-mmap --stream-count=1 --stream-to={output_path}",
        config.index, config.width, config.height, config.format
    )
}

// ═══════════════════════════════════════════════════════════════════════
// Deploy Commands
// ═══════════════════════════════════════════════════════════════════════

/// Generates SCP + SSH commands to deploy a binary to the MPU over the network.
pub fn deploy_mpu_command(binary_path: &str, target_host: &str) -> String {
    format!(
        "scp {binary_path} {target_host}:/opt/fj/bin/ && \
         ssh {target_host} 'chmod +x /opt/fj/bin/{binary_name} && /opt/fj/bin/{binary_name}'",
        binary_name = binary_path.rsplit('/').next().unwrap_or(binary_path)
    )
}

/// Generates a probe-rs command to flash firmware to the STM32H5 MCU.
pub fn deploy_mcu_command(firmware_path: &str, probe: &str) -> String {
    format!("probe-rs run --chip STM32H5F5 --probe {probe} {firmware_path}")
}

// ═══════════════════════════════════════════════════════════════════════
// Board Implementation
// ═══════════════════════════════════════════════════════════════════════

/// Qualcomm Dragonwing IQ8 MPU board.
///
/// This is the high-performance ARM64 Linux brain of the Arduino VENTUNO Q,
/// featuring an octa-core CPU, Adreno GPU, and 40 TOPS Hexagon NPU.
pub struct DragonwingIQ8 {
    _private: (),
}

impl DragonwingIQ8 {
    /// Creates a new Dragonwing IQ8 board instance.
    pub fn new() -> Self {
        Self { _private: () }
    }

    /// Returns GPU capabilities for the onboard Adreno GPU.
    pub fn gpu_capabilities(&self) -> GpuCapabilities {
        gpu_info()
    }

    /// Returns NPU capabilities for the onboard Hexagon Tensor NPU.
    pub fn npu_capabilities(&self) -> NpuCapabilities {
        npu_info()
    }

    /// Checks if the GPU is available on this system.
    pub fn gpu_available(&self) -> bool {
        gpu_available()
    }

    /// Checks if the NPU runtime is available on this system.
    pub fn npu_available(&self) -> bool {
        npu_available()
    }
}

impl Default for DragonwingIQ8 {
    fn default() -> Self {
        Self::new()
    }
}

impl Board for DragonwingIQ8 {
    fn name(&self) -> &str {
        "Dragonwing IQ8"
    }

    fn arch(&self) -> BspArch {
        BspArch::Aarch64Linux
    }

    fn memory_regions(&self) -> Vec<MemoryRegion> {
        // Linux userspace: memory is managed by the kernel.
        // We represent conceptual regions (sizes capped to u32::MAX for the API).
        vec![
            // RAM: 16GB (capped to u32 max ~4GB for struct; actual size in description)
            MemoryRegion::new("RAM", 0x0000_0000, u32::MAX, MemoryAttr::Rwx),
            // eMMC storage: 64GB (represented as 4GB cap; conceptual only)
            MemoryRegion::new("EMMC", 0x0000_0000, u32::MAX, MemoryAttr::Rw),
        ]
    }

    fn peripherals(&self) -> Vec<Peripheral> {
        vec![
            // USB-C (primary)
            Peripheral::new("USB-C", 0x0000_0001),
            // Dual USB 3.0 Type-A
            Peripheral::new("USB3_A0", 0x0000_0002),
            Peripheral::new("USB3_A1", 0x0000_0003),
            // Triple MIPI-CSI camera interfaces
            Peripheral::new("MIPI_CSI0", 0x0000_0010),
            Peripheral::new("MIPI_CSI1", 0x0000_0011),
            Peripheral::new("MIPI_CSI2", 0x0000_0012),
            // 2.5 Gigabit Ethernet
            Peripheral::new("ETH_2G5", 0x0000_0020),
            // Wi-Fi 6 (2.4/5/6 GHz)
            Peripheral::new("WIFI6", 0x0000_0030),
            // Bluetooth 5.3
            Peripheral::new("BT53", 0x0000_0031),
        ]
    }

    fn vector_table_size(&self) -> usize {
        // ARM64 Linux: no bare-metal vector table; interrupt handling via kernel.
        0
    }

    fn cpu_frequency(&self) -> u32 {
        // Kryo octa-core, max frequency ~2.5 GHz (big cores)
        2_500_000_000
    }

    fn generate_linker_script(&self) -> String {
        let mut script = String::new();
        script.push_str("/* Dragonwing IQ8 — Linux userspace target */\n");
        script.push_str("/* No custom linker script needed for Linux ELF */\n");
        script.push_str("/* Uses system default: aarch64-unknown-linux-gnu */\n\n");
        script.push_str("/* Build with: */\n");
        script.push_str("/*   fj build --target aarch64-unknown-linux-gnu */\n");
        script
    }

    fn generate_startup_code(&self) -> String {
        let mut code = String::new();
        code.push_str("/* Dragonwing IQ8 — Linux userspace target */\n");
        code.push_str("/* No custom startup code needed for Linux ELF */\n");
        code.push_str("/* The dynamic linker handles initialization */\n\n");
        code.push_str(".global _start\n");
        code.push_str(".type _start, @function\n");
        code.push_str("_start:\n");
        code.push_str("  bl main\n");
        code.push_str("  mov x0, #0\n");
        code.push_str("  mov x8, #93  /* __NR_exit */\n");
        code.push_str("  svc #0\n");
        code
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dragonwing_board_identity() {
        let board = DragonwingIQ8::new();
        assert_eq!(board.name(), "Dragonwing IQ8");
        assert_eq!(board.arch(), BspArch::Aarch64Linux);
        assert_eq!(board.cpu_frequency(), 2_500_000_000);
    }

    #[test]
    fn dragonwing_default_trait() {
        let board = DragonwingIQ8::default();
        assert_eq!(board.name(), "Dragonwing IQ8");
    }

    #[test]
    fn dragonwing_memory_regions() {
        let board = DragonwingIQ8::new();
        let regions = board.memory_regions();
        assert_eq!(regions.len(), 2);
        assert_eq!(regions[0].name, "RAM");
        assert_eq!(regions[1].name, "EMMC");
    }

    #[test]
    fn dragonwing_peripherals_include_cameras() {
        let board = DragonwingIQ8::new();
        let periphs = board.peripherals();
        let cameras: Vec<_> = periphs
            .iter()
            .filter(|p| p.name.starts_with("MIPI_CSI"))
            .collect();
        assert_eq!(cameras.len(), 3, "should have triple MIPI-CSI");
    }

    #[test]
    fn dragonwing_peripherals_include_connectivity() {
        let board = DragonwingIQ8::new();
        let periphs = board.peripherals();
        assert!(periphs.iter().any(|p| p.name == "WIFI6"));
        assert!(periphs.iter().any(|p| p.name == "BT53"));
        assert!(periphs.iter().any(|p| p.name == "ETH_2G5"));
    }

    #[test]
    fn dragonwing_gpu_capabilities() {
        let board = DragonwingIQ8::new();
        let gpu = board.gpu_capabilities();
        assert_eq!(gpu.name, "Adreno");
        assert_eq!(gpu.clock_mhz, 877);
        assert!(gpu.vulkan_supported);
        assert!(gpu.compute_units > 0);
    }

    #[test]
    fn dragonwing_gpu_display() {
        let gpu = GpuCapabilities::dragonwing_default();
        let display = format!("{gpu}");
        assert!(display.contains("Adreno"));
        assert!(display.contains("877MHz"));
    }

    #[test]
    fn dragonwing_npu_capabilities() {
        let board = DragonwingIQ8::new();
        let npu = board.npu_capabilities();
        assert_eq!(npu.name, "Hexagon Tensor");
        assert_eq!(npu.tops, 40);
        assert!(npu.supported_dtypes.contains(&NpuDtype::F16));
        assert!(npu.supported_dtypes.contains(&NpuDtype::Int8));
        assert!(npu.supported_dtypes.contains(&NpuDtype::Int4));
    }

    #[test]
    fn dragonwing_npu_display() {
        let npu = NpuCapabilities::dragonwing_default();
        let display = format!("{npu}");
        assert!(display.contains("Hexagon Tensor"));
        assert!(display.contains("40 TOPS"));
        assert!(display.contains("QNN 2.25.0"));
    }

    #[test]
    fn dragonwing_qnn_load_model_success() {
        let model = qnn_load_model("/opt/models/mobilenet.dlc");
        assert!(model.is_ok());
        let model = model.unwrap();
        assert_eq!(model.name, "mobilenet");
        assert!(model.loaded);
        assert_eq!(model.input_shape, vec![1, 3, 224, 224]);
        assert_eq!(model.output_shape, vec![1, 1000]);
    }

    #[test]
    fn dragonwing_qnn_load_model_empty_path() {
        let result = qnn_load_model("");
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            QnnError::ModelNotFound("(empty path)".to_string())
        );
    }

    #[test]
    fn dragonwing_qnn_infer_success() {
        let model = qnn_load_model("/opt/models/test.dlc").unwrap();
        let input_size: usize = model.input_shape.iter().product();
        let input = vec![0.5; input_size];
        let result = qnn_infer(&model, &input);
        assert!(result.is_ok());
        let output = result.unwrap();
        let expected_output_size: usize = model.output_shape.iter().product();
        assert_eq!(output.len(), expected_output_size);
        // Simulation returns zeros
        assert!(output.iter().all(|&v| v == 0.0));
    }

    #[test]
    fn dragonwing_qnn_infer_shape_mismatch() {
        let model = qnn_load_model("/opt/models/test.dlc").unwrap();
        let wrong_input = vec![1.0; 10]; // Wrong size
        let result = qnn_infer(&model, &wrong_input);
        assert!(result.is_err());
        match result.unwrap_err() {
            QnnError::ShapeMismatch { expected, actual } => {
                assert_eq!(expected, vec![1, 3, 224, 224]);
                assert_eq!(actual, vec![10]);
            }
            other => panic!("Expected ShapeMismatch, got {other:?}"),
        }
    }

    #[test]
    fn dragonwing_qnn_infer_unloaded_model() {
        let model = QnnModel {
            name: "test".to_string(),
            input_shape: vec![1, 4],
            output_shape: vec![1, 2],
            loaded: false,
        };
        let result = qnn_infer(&model, &[1.0, 2.0, 3.0, 4.0]);
        assert!(result.is_err());
    }

    #[test]
    fn dragonwing_onnx_to_qnn_command() {
        let cmd = onnx_to_qnn_command("model.onnx", "/tmp/model_qnn");
        assert!(cmd.contains("qnn-onnx-converter"));
        assert!(cmd.contains("model.onnx"));
        assert!(cmd.contains("/tmp/model_qnn"));
    }

    #[test]
    fn dragonwing_qnn_context_binary_command() {
        let cmd = qnn_context_binary_command("/tmp/model.cpp", "/tmp/output");
        assert!(cmd.contains("qnn-context-binary-generator"));
        assert!(cmd.contains("libQnnHtp.so"));
        assert!(cmd.contains("/tmp/model.cpp"));
        assert!(cmd.contains("/tmp/output"));
    }

    #[test]
    fn dragonwing_camera_config_creation() {
        let cam = CameraConfig::new(0, 1920, 1080, CameraFormat::Mjpeg);
        assert_eq!(cam.index, 0);
        assert_eq!(cam.width, 1920);
        assert_eq!(cam.height, 1080);
        assert_eq!(cam.format, CameraFormat::Mjpeg);
    }

    #[test]
    fn dragonwing_camera_default_1080p() {
        let cam = CameraConfig::default_1080p(1);
        assert_eq!(cam.index, 1);
        assert_eq!(cam.width, 1920);
        assert_eq!(cam.height, 1080);
        assert_eq!(cam.format, CameraFormat::Mjpeg);
    }

    #[test]
    fn dragonwing_camera_open_command() {
        let cam = CameraConfig::new(0, 640, 480, CameraFormat::Yuyv);
        let cmd = camera_open_command(&cam);
        assert!(cmd.contains("/dev/video0"));
        assert!(cmd.contains("width=640"));
        assert!(cmd.contains("height=480"));
        assert!(cmd.contains("YUYV"));
    }

    #[test]
    fn dragonwing_camera_capture_command() {
        let cam = CameraConfig::new(2, 1280, 720, CameraFormat::Nv12);
        let cmd = camera_capture_command(&cam, "/tmp/frame.raw");
        assert!(cmd.contains("/dev/video2"));
        assert!(cmd.contains("1280"));
        assert!(cmd.contains("720"));
        assert!(cmd.contains("NV12"));
        assert!(cmd.contains("/tmp/frame.raw"));
        assert!(cmd.contains("--stream-mmap"));
    }

    #[test]
    fn dragonwing_deploy_mpu_command() {
        let cmd = deploy_mpu_command("/tmp/app", "ventuno@192.168.1.100");
        assert!(cmd.contains("scp"));
        assert!(cmd.contains("ssh"));
        assert!(cmd.contains("ventuno@192.168.1.100"));
        assert!(cmd.contains("/opt/fj/bin/"));
    }

    #[test]
    fn dragonwing_deploy_mcu_command() {
        let cmd = deploy_mcu_command("firmware.elf", "stlink");
        assert!(cmd.contains("probe-rs"));
        assert!(cmd.contains("STM32H5F5"));
        assert!(cmd.contains("stlink"));
        assert!(cmd.contains("firmware.elf"));
    }

    #[test]
    fn dragonwing_linker_script_linux() {
        let board = DragonwingIQ8::new();
        let script = board.generate_linker_script();
        assert!(script.contains("Linux userspace"));
        assert!(script.contains("aarch64-unknown-linux-gnu"));
    }

    #[test]
    fn dragonwing_startup_code_linux() {
        let board = DragonwingIQ8::new();
        let code = board.generate_startup_code();
        assert!(code.contains("_start"));
        assert!(code.contains("bl main"));
        assert!(code.contains("__NR_exit"));
    }

    #[test]
    fn dragonwing_vector_table_size_zero() {
        let board = DragonwingIQ8::new();
        assert_eq!(board.vector_table_size(), 0);
    }

    #[test]
    fn dragonwing_qnn_error_display() {
        let e = QnnError::ModelNotFound("/bad/path".to_string());
        assert!(format!("{e}").contains("/bad/path"));

        let e = QnnError::RuntimeNotAvailable;
        assert!(format!("{e}").contains("not available"));

        let e = QnnError::ShapeMismatch {
            expected: vec![1, 3],
            actual: vec![2, 4],
        };
        assert!(format!("{e}").contains("shape mismatch"));
    }

    #[test]
    fn dragonwing_npu_dtype_display() {
        assert_eq!(format!("{}", NpuDtype::F16), "F16");
        assert_eq!(format!("{}", NpuDtype::Int8), "INT8");
        assert_eq!(format!("{}", NpuDtype::Int4), "INT4");
    }

    #[test]
    fn dragonwing_camera_format_display() {
        assert_eq!(format!("{}", CameraFormat::Yuyv), "YUYV");
        assert_eq!(format!("{}", CameraFormat::Mjpeg), "MJPEG");
        assert_eq!(format!("{}", CameraFormat::Nv12), "NV12");
        assert_eq!(format!("{}", CameraFormat::RawBayer), "RGGB");
    }
}
