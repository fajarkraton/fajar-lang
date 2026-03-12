//! Radxa Dragon Q6A board support package.
//!
//! The Dragon Q6A is an edge AI SBC based on the Qualcomm QCS6490 SoC,
//! featuring an ARM64 CPU, Adreno 643 GPU, and 12 TOPS Hexagon 770 NPU.
//!
//! # Hardware Overview
//!
//! - **SoC**: Qualcomm QCS6490 (Dragonwing), TSMC 6nm
//! - **CPU**: Kryo 670 ARMv8.2-A — 1x A78@2.71GHz + 3x A78@2.4GHz + 4x A55@1.96GHz
//! - **GPU**: Adreno 643 @ 812MHz — OpenCL 2.0, Vulkan 1.1
//! - **NPU**: Hexagon 770 (V68) — 12 TOPS INT8
//! - **RAM**: LPDDR5 up to 16GB @ 3200MHz (~25.6 GB/s)
//! - **GPIO**: 40-pin header — 7 UART, 6 I2C, 7 SPI, I2S, I3C
//! - **Display**: HDMI 4K@30 + MIPI DSI 4-lane
//! - **Camera**: 3x MIPI CSI (1x 4-lane + 2x 2-lane)
//! - **Storage**: microSD, eMMC, UFS, M.2 NVMe SSD
//! - **Network**: GbE, WiFi 6, Bluetooth 5.4
//! - **OS**: Ubuntu 24.04, kernel 6.16.x (near-mainline)
//! - **Power**: 12V USB-C PD, ~5W SoC TDP
//! - **Size**: 85mm x 56mm
//!
//! This is a Linux userspace target (`aarch64-unknown-linux-gnu`).
//! GPIO access is via `/dev/gpiochip4` using the Linux chardev interface.
//!
//! # Cross-Compilation
//!
//! ```bash
//! rustup target add aarch64-unknown-linux-gnu
//! cargo build --release --target aarch64-unknown-linux-gnu
//! scp target/aarch64-unknown-linux-gnu/release/fj radxa@<ip>:~/
//! ```

use super::{Board, BspArch, MemoryAttr, MemoryRegion, Peripheral};
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// CPU Information
// ═══════════════════════════════════════════════════════════════════════

/// CPU cluster in the Kryo 670 tri-cluster DynamIQ design.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CpuCluster {
    /// Prime cluster: 1x Cortex-A78 @ 2.71 GHz.
    Prime,
    /// Gold cluster: 3x Cortex-A78 @ 2.40 GHz.
    Gold,
    /// Silver cluster: 4x Cortex-A55 @ 1.96 GHz.
    Silver,
}

impl fmt::Display for CpuCluster {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CpuCluster::Prime => write!(f, "Prime (1x A78 @ 2.71GHz)"),
            CpuCluster::Gold => write!(f, "Gold (3x A78 @ 2.40GHz)"),
            CpuCluster::Silver => write!(f, "Silver (4x A55 @ 1.96GHz)"),
        }
    }
}

/// CPU information for the Kryo 670.
#[derive(Debug, Clone)]
pub struct CpuInfo {
    /// CPU name.
    pub name: String,
    /// ISA version.
    pub isa: String,
    /// Total core count.
    pub cores: u32,
    /// Maximum frequency in Hz (Prime cluster).
    pub max_freq_hz: u32,
    /// L3/SLC cache size in bytes.
    pub l3_cache_bytes: u32,
}

impl CpuInfo {
    /// Creates CPU info for the QCS6490 Kryo 670.
    pub fn qcs6490_default() -> Self {
        Self {
            name: "Kryo 670".to_string(),
            isa: "ARMv8.2-A".to_string(),
            cores: 8,
            max_freq_hz: 2_710_000_000,
            l3_cache_bytes: 3 * 1024 * 1024, // 3 MB shared SLC
        }
    }
}

impl fmt::Display for CpuInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} ({}, {} cores, max {}MHz, L3 {}KB)",
            self.name,
            self.isa,
            self.cores,
            self.max_freq_hz / 1_000_000,
            self.l3_cache_bytes / 1024,
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════
// GPU Capabilities — Adreno 643
// ═══════════════════════════════════════════════════════════════════════

/// GPU capabilities for the Adreno 643 GPU.
#[derive(Debug, Clone)]
pub struct GpuCapabilities {
    /// GPU name.
    pub name: String,
    /// Clock speed in MHz.
    pub clock_mhz: u32,
    /// Vulkan version string.
    pub vulkan_version: String,
    /// OpenCL version string.
    pub opencl_version: String,
    /// OpenGL ES version string.
    pub gles_version: String,
    /// Estimated FP32 performance in GFLOPS.
    pub fp32_gflops: u32,
}

impl GpuCapabilities {
    /// Creates GPU capabilities for the Adreno 643 on QCS6490.
    pub fn adreno643_default() -> Self {
        Self {
            name: "Adreno 643".to_string(),
            clock_mhz: 812,
            vulkan_version: "1.1".to_string(),
            opencl_version: "2.0".to_string(),
            gles_version: "3.2".to_string(),
            fp32_gflops: 773, // vkpeak benchmark: 773 GFLOPS FP32, 1581 GFLOPS FP16
        }
    }
}

impl fmt::Display for GpuCapabilities {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} @ {}MHz (Vulkan {}, OpenCL {}, ~{} GFLOPS FP32)",
            self.name, self.clock_mhz, self.vulkan_version, self.opencl_version, self.fp32_gflops,
        )
    }
}

/// Checks if the Adreno 643 GPU is available via sysfs.
pub fn gpu_available() -> bool {
    std::path::Path::new("/sys/class/kgsl/kgsl-3d0").exists()
}

/// Returns GPU capability information for the Adreno 643.
pub fn gpu_info() -> GpuCapabilities {
    GpuCapabilities::adreno643_default()
}

// ═══════════════════════════════════════════════════════════════════════
// NPU Capabilities — Hexagon 770 (V68)
// ═══════════════════════════════════════════════════════════════════════

/// Supported data types for NPU inference on Hexagon 770.
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

/// Hexagon 770 sub-components.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HexagonComponent {
    /// Scalar VLIW processor for general DSP operations.
    Scalar,
    /// HVX (Hexagon Vector eXtensions) — 1024-bit SIMD, dual units.
    Hvx,
    /// HTA/HMX (Hexagon Matrix eXtension) — dedicated tensor accelerator.
    Hmx,
}

impl fmt::Display for HexagonComponent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HexagonComponent::Scalar => write!(f, "Hexagon Scalar (VLIW)"),
            HexagonComponent::Hvx => write!(f, "HVX (1024-bit SIMD)"),
            HexagonComponent::Hmx => write!(f, "HMX (Tensor Accelerator)"),
        }
    }
}

/// NPU capabilities for the Hexagon 770 processor.
#[derive(Debug, Clone)]
pub struct NpuCapabilities {
    /// NPU name.
    pub name: String,
    /// ISA version (V68).
    pub isa_version: String,
    /// Performance in TOPS (tera operations per second).
    pub tops: u32,
    /// QNN SDK version string.
    pub qnn_version: String,
    /// Supported data types.
    pub supported_dtypes: Vec<NpuDtype>,
    /// HTP skel library name (V68-specific).
    pub htp_skel_lib: String,
}

impl NpuCapabilities {
    /// Creates NPU capabilities for the Hexagon 770 on QCS6490.
    pub fn hexagon770_default() -> Self {
        Self {
            name: "Hexagon 770".to_string(),
            isa_version: "V68".to_string(),
            tops: 12,
            qnn_version: "2.37.1".to_string(),
            supported_dtypes: vec![NpuDtype::F16, NpuDtype::Int8, NpuDtype::Int4],
            htp_skel_lib: "libQnnHtpV68Skel.so".to_string(),
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
            "{} ({}, {} TOPS, QNN {}, dtypes: [{}])",
            self.name,
            self.isa_version,
            self.tops,
            self.qnn_version,
            dtypes.join(", ")
        )
    }
}

/// Checks if the Hexagon NPU runtime is available.
///
/// On real hardware, checks for the QNN HTP library and the ADSP RPC device.
pub fn npu_available() -> bool {
    std::path::Path::new("/usr/lib/libQnnHtp.so").exists()
        || std::path::Path::new("/dev/adsprpc-smd").exists()
}

/// Returns NPU capability information for the Hexagon 770.
pub fn npu_info() -> NpuCapabilities {
    NpuCapabilities::hexagon770_default()
}

// ═══════════════════════════════════════════════════════════════════════
// QNN Inference Wrapper
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

/// QNN backend target for inference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QnnBackend {
    /// CPU backend (any model, slowest).
    Cpu,
    /// GPU backend (FP16/FP32, Adreno 643).
    Gpu,
    /// HTP backend (INT8/INT16, Hexagon 770 NPU, fastest).
    Htp,
}

impl QnnBackend {
    /// Returns the shared library name for this backend.
    pub fn library_name(&self) -> &'static str {
        match self {
            QnnBackend::Cpu => "libQnnCpu.so",
            QnnBackend::Gpu => "libQnnGpu.so",
            QnnBackend::Htp => "libQnnHtp.so",
        }
    }
}

impl fmt::Display for QnnBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            QnnBackend::Cpu => write!(f, "CPU"),
            QnnBackend::Gpu => write!(f, "GPU (Adreno 643)"),
            QnnBackend::Htp => write!(f, "HTP (Hexagon 770 NPU)"),
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
    /// Backend used for this model.
    pub backend: QnnBackend,
}

/// Loads a QNN model from the given path (simulation mode).
///
/// In simulation mode, creates a stub model with default shapes.
/// On real hardware, this would use the QNN SDK to load the model
/// via `dlopen("libQnnHtp.so")` and the QNN C API.
pub fn qnn_load_model(path: &str, backend: QnnBackend) -> Result<QnnModel, QnnError> {
    if path.is_empty() {
        return Err(QnnError::ModelNotFound("(empty path)".to_string()));
    }

    let name = path
        .rsplit('/')
        .next()
        .unwrap_or(path)
        .trim_end_matches(".bin")
        .trim_end_matches(".so")
        .trim_end_matches(".dlc")
        .to_string();

    Ok(QnnModel {
        name,
        input_shape: vec![1, 3, 224, 224],
        output_shape: vec![1, 1000],
        loaded: true,
        backend,
    })
}

/// Runs inference on a loaded QNN model (simulation mode).
///
/// In simulation mode, returns a zero vector of the output shape size.
/// On real hardware, this would execute the model on the selected backend.
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

    let output_size: usize = model.output_shape.iter().product();
    Ok(vec![0.0; output_size])
}

// ═══════════════════════════════════════════════════════════════════════
// ONNX to QNN Export Pipeline
// ═══════════════════════════════════════════════════════════════════════

/// Generates the command to convert an ONNX model to QNN format.
pub fn onnx_to_qnn_command(onnx_path: &str, output_path: &str) -> String {
    format!("qnn-onnx-converter --input_network {onnx_path} --output_path {output_path}")
}

/// Generates the command to quantize a QNN model to INT8.
pub fn qnn_quantize_command(model_path: &str, calibration_list: &str) -> String {
    format!(
        "qnn-onnx-converter --input_network {model_path} \
         --input_list {calibration_list} \
         --act_bw 8 --weight_bw 8"
    )
}

/// Generates the command to create a QNN context binary for the HTP backend.
pub fn qnn_context_binary_command(model_path: &str, output_path: &str) -> String {
    format!(
        "qnn-context-binary-generator \
         --model {model_path} \
         --backend libQnnHtp.so \
         --output_dir {output_path}"
    )
}

/// Generates the command to run inference with qnn-net-run.
pub fn qnn_net_run_command(model_path: &str, backend: QnnBackend) -> String {
    format!(
        "qnn-net-run --model {model_path} --backend {}",
        backend.library_name()
    )
}

// ═══════════════════════════════════════════════════════════════════════
// 40-Pin GPIO Header
// ═══════════════════════════════════════════════════════════════════════

/// GPIO pin function (mutually exclusive, selected via Device Tree overlay).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinFunction {
    /// General-purpose input/output.
    Gpio,
    /// UART transmit.
    UartTx,
    /// UART receive.
    UartRx,
    /// I2C data.
    I2cSda,
    /// I2C clock.
    I2cScl,
    /// SPI master-out slave-in.
    SpiMosi,
    /// SPI master-in slave-out.
    SpiMiso,
    /// SPI chip select.
    SpiCs,
    /// I2S word select.
    I2sWs,
    /// I2S data.
    I2sData,
    /// Master clock output.
    Mclk,
    /// Power supply (3.3V or 5V).
    Power,
    /// Ground.
    Ground,
}

impl fmt::Display for PinFunction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PinFunction::Gpio => write!(f, "GPIO"),
            PinFunction::UartTx => write!(f, "UART_TX"),
            PinFunction::UartRx => write!(f, "UART_RX"),
            PinFunction::I2cSda => write!(f, "I2C_SDA"),
            PinFunction::I2cScl => write!(f, "I2C_SCL"),
            PinFunction::SpiMosi => write!(f, "SPI_MOSI"),
            PinFunction::SpiMiso => write!(f, "SPI_MISO"),
            PinFunction::SpiCs => write!(f, "SPI_CS"),
            PinFunction::I2sWs => write!(f, "I2S_WS"),
            PinFunction::I2sData => write!(f, "I2S_DATA"),
            PinFunction::Mclk => write!(f, "MCLK"),
            PinFunction::Power => write!(f, "POWER"),
            PinFunction::Ground => write!(f, "GND"),
        }
    }
}

/// A physical pin on the 40-pin GPIO header.
#[derive(Debug, Clone)]
pub struct HeaderPin {
    /// Physical pin number (1-40).
    pub physical: u8,
    /// GPIO number (for gpiochip4), or None for power/ground.
    pub gpio_num: Option<u32>,
    /// Default function.
    pub default_function: PinFunction,
    /// Alternate functions available via Device Tree overlays.
    pub alt_functions: Vec<PinFunction>,
    /// Human-readable label.
    pub label: String,
}

/// Returns the complete 40-pin GPIO header pinout for the Dragon Q6A.
///
/// Logic level: 3.3V. GPIO device: `/dev/gpiochip4`.
pub fn gpio_header_pinout() -> Vec<HeaderPin> {
    vec![
        // Row 1 (odd pins)
        HeaderPin {
            physical: 1,
            gpio_num: None,
            default_function: PinFunction::Power,
            alt_functions: vec![],
            label: "3V3".into(),
        },
        HeaderPin {
            physical: 2,
            gpio_num: None,
            default_function: PinFunction::Power,
            alt_functions: vec![],
            label: "5V".into(),
        },
        HeaderPin {
            physical: 3,
            gpio_num: Some(24),
            default_function: PinFunction::I2cSda,
            alt_functions: vec![PinFunction::Gpio],
            label: "GPIO24/I2C6_SDA".into(),
        },
        HeaderPin {
            physical: 4,
            gpio_num: None,
            default_function: PinFunction::Power,
            alt_functions: vec![],
            label: "5V".into(),
        },
        HeaderPin {
            physical: 5,
            gpio_num: Some(25),
            default_function: PinFunction::I2cScl,
            alt_functions: vec![PinFunction::Gpio],
            label: "GPIO25/I2C6_SCL".into(),
        },
        HeaderPin {
            physical: 6,
            gpio_num: None,
            default_function: PinFunction::Ground,
            alt_functions: vec![],
            label: "GND".into(),
        },
        HeaderPin {
            physical: 7,
            gpio_num: Some(96),
            default_function: PinFunction::Mclk,
            alt_functions: vec![PinFunction::Gpio],
            label: "GPIO96/MCLK".into(),
        },
        HeaderPin {
            physical: 8,
            gpio_num: Some(22),
            default_function: PinFunction::UartTx,
            alt_functions: vec![PinFunction::Gpio],
            label: "GPIO22/UART5_TX".into(),
        },
        HeaderPin {
            physical: 9,
            gpio_num: None,
            default_function: PinFunction::Ground,
            alt_functions: vec![],
            label: "GND".into(),
        },
        HeaderPin {
            physical: 10,
            gpio_num: Some(23),
            default_function: PinFunction::UartRx,
            alt_functions: vec![PinFunction::Gpio],
            label: "GPIO23/UART5_RX".into(),
        },
        HeaderPin {
            physical: 11,
            gpio_num: Some(29),
            default_function: PinFunction::I2cScl,
            alt_functions: vec![PinFunction::Gpio],
            label: "GPIO29/I2C7_SCL".into(),
        },
        HeaderPin {
            physical: 12,
            gpio_num: Some(97),
            default_function: PinFunction::I2cSda,
            alt_functions: vec![PinFunction::Gpio],
            label: "GPIO97/I2C0_SDA".into(),
        },
        HeaderPin {
            physical: 13,
            gpio_num: Some(0),
            default_function: PinFunction::Gpio,
            alt_functions: vec![],
            label: "GPIO0".into(),
        },
        HeaderPin {
            physical: 14,
            gpio_num: None,
            default_function: PinFunction::Ground,
            alt_functions: vec![],
            label: "GND".into(),
        },
        HeaderPin {
            physical: 15,
            gpio_num: Some(1),
            default_function: PinFunction::I2cScl,
            alt_functions: vec![PinFunction::Gpio],
            label: "GPIO1/I2C0_SCL".into(),
        },
        HeaderPin {
            physical: 16,
            gpio_num: Some(26),
            default_function: PinFunction::UartTx,
            alt_functions: vec![PinFunction::Gpio],
            label: "GPIO26/UART6_TX".into(),
        },
        HeaderPin {
            physical: 17,
            gpio_num: None,
            default_function: PinFunction::Power,
            alt_functions: vec![],
            label: "3V3".into(),
        },
        HeaderPin {
            physical: 18,
            gpio_num: Some(27),
            default_function: PinFunction::UartRx,
            alt_functions: vec![PinFunction::Gpio],
            label: "GPIO27/UART6_RX".into(),
        },
        HeaderPin {
            physical: 19,
            gpio_num: Some(49),
            default_function: PinFunction::SpiMosi,
            alt_functions: vec![PinFunction::Gpio],
            label: "GPIO49/SPI12_MOSI".into(),
        },
        HeaderPin {
            physical: 20,
            gpio_num: None,
            default_function: PinFunction::Ground,
            alt_functions: vec![],
            label: "GND".into(),
        },
        HeaderPin {
            physical: 21,
            gpio_num: Some(48),
            default_function: PinFunction::SpiMiso,
            alt_functions: vec![PinFunction::Gpio],
            label: "GPIO48/SPI12_MISO".into(),
        },
        HeaderPin {
            physical: 22,
            gpio_num: Some(50),
            default_function: PinFunction::UartTx,
            alt_functions: vec![PinFunction::Gpio],
            label: "GPIO50/UART12_TX".into(),
        },
        HeaderPin {
            physical: 23,
            gpio_num: Some(57),
            default_function: PinFunction::I2cScl,
            alt_functions: vec![PinFunction::Gpio],
            label: "GPIO57/I2C14_SCL".into(),
        },
        HeaderPin {
            physical: 24,
            gpio_num: Some(51),
            default_function: PinFunction::SpiCs,
            alt_functions: vec![PinFunction::Gpio],
            label: "GPIO51/SPI12_CS".into(),
        },
        HeaderPin {
            physical: 25,
            gpio_num: None,
            default_function: PinFunction::Ground,
            alt_functions: vec![],
            label: "GND".into(),
        },
        HeaderPin {
            physical: 26,
            gpio_num: Some(8),
            default_function: PinFunction::I2cSda,
            alt_functions: vec![PinFunction::Gpio],
            label: "GPIO8/I2C2_SDA".into(),
        },
        HeaderPin {
            physical: 27,
            gpio_num: Some(9),
            default_function: PinFunction::I2cScl,
            alt_functions: vec![PinFunction::Gpio],
            label: "GPIO9/I2C2_SCL".into(),
        },
        HeaderPin {
            physical: 28,
            gpio_num: Some(31),
            default_function: PinFunction::UartRx,
            alt_functions: vec![PinFunction::Gpio],
            label: "GPIO31/UART7_RX".into(),
        },
        HeaderPin {
            physical: 29,
            gpio_num: None,
            default_function: PinFunction::Ground,
            alt_functions: vec![],
            label: "GND".into(),
        },
        HeaderPin {
            physical: 30,
            gpio_num: Some(28),
            default_function: PinFunction::I2cSda,
            alt_functions: vec![PinFunction::Gpio],
            label: "GPIO28/I2C7_SDA".into(),
        },
        HeaderPin {
            physical: 31,
            gpio_num: Some(30),
            default_function: PinFunction::UartTx,
            alt_functions: vec![PinFunction::Gpio],
            label: "GPIO30/UART7_TX".into(),
        },
        HeaderPin {
            physical: 32,
            gpio_num: Some(56),
            default_function: PinFunction::SpiMiso,
            alt_functions: vec![PinFunction::Gpio],
            label: "GPIO56/SPI14_MISO".into(),
        },
        HeaderPin {
            physical: 33,
            gpio_num: Some(59),
            default_function: PinFunction::SpiCs,
            alt_functions: vec![PinFunction::Gpio],
            label: "GPIO59/SPI14_CS".into(),
        },
        HeaderPin {
            physical: 34,
            gpio_num: None,
            default_function: PinFunction::Ground,
            alt_functions: vec![],
            label: "GND".into(),
        },
        HeaderPin {
            physical: 35,
            gpio_num: Some(100),
            default_function: PinFunction::I2sWs,
            alt_functions: vec![PinFunction::Gpio],
            label: "GPIO100/I2S_WS".into(),
        },
        HeaderPin {
            physical: 36,
            gpio_num: None,
            default_function: PinFunction::Ground,
            alt_functions: vec![],
            label: "--".into(),
        },
        HeaderPin {
            physical: 37,
            gpio_num: Some(58),
            default_function: PinFunction::Gpio,
            alt_functions: vec![],
            label: "GPIO58".into(),
        },
        HeaderPin {
            physical: 38,
            gpio_num: Some(98),
            default_function: PinFunction::I2sData,
            alt_functions: vec![PinFunction::Gpio],
            label: "GPIO98/I2S_DATA0".into(),
        },
        HeaderPin {
            physical: 39,
            gpio_num: None,
            default_function: PinFunction::Ground,
            alt_functions: vec![],
            label: "GND".into(),
        },
        HeaderPin {
            physical: 40,
            gpio_num: Some(99),
            default_function: PinFunction::I2sData,
            alt_functions: vec![PinFunction::Gpio],
            label: "GPIO99/I2S_DATA1".into(),
        },
    ]
}

/// Returns the GPIO device path for the 40-pin header.
pub fn gpio_device_path() -> &'static str {
    "/dev/gpiochip4"
}

/// Returns the number of usable GPIO pins on the 40-pin header.
pub fn gpio_pin_count() -> usize {
    gpio_header_pinout()
        .iter()
        .filter(|p| p.gpio_num.is_some())
        .count()
}

/// Looks up a header pin by physical pin number (1-40).
pub fn pin_by_physical(num: u8) -> Option<HeaderPin> {
    gpio_header_pinout().into_iter().find(|p| p.physical == num)
}

/// Looks up a header pin by GPIO number.
pub fn pin_by_gpio(gpio: u32) -> Option<HeaderPin> {
    gpio_header_pinout()
        .into_iter()
        .find(|p| p.gpio_num == Some(gpio))
}

// ═══════════════════════════════════════════════════════════════════════
// Camera Integration (MIPI CSI via V4L2)
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

/// MIPI CSI camera port on the Dragon Q6A.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CsiPort {
    /// Primary camera: 4-lane MIPI CSI.
    Csi0FourLane,
    /// Secondary camera: 2-lane MIPI CSI.
    Csi1TwoLane,
    /// Tertiary camera: 2-lane MIPI CSI.
    Csi2TwoLane,
}

impl fmt::Display for CsiPort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CsiPort::Csi0FourLane => write!(f, "CSI0 (4-lane)"),
            CsiPort::Csi1TwoLane => write!(f, "CSI1 (2-lane)"),
            CsiPort::Csi2TwoLane => write!(f, "CSI2 (2-lane)"),
        }
    }
}

/// Camera configuration for V4L2 capture.
#[derive(Debug, Clone)]
pub struct CameraConfig {
    /// Camera port.
    pub port: CsiPort,
    /// V4L2 device index.
    pub device_index: u8,
    /// Capture width in pixels.
    pub width: u32,
    /// Capture height in pixels.
    pub height: u32,
    /// Pixel format.
    pub format: CameraFormat,
}

impl CameraConfig {
    /// Creates a new camera configuration.
    pub fn new(
        port: CsiPort,
        device_index: u8,
        width: u32,
        height: u32,
        format: CameraFormat,
    ) -> Self {
        Self {
            port,
            device_index,
            width,
            height,
            format,
        }
    }

    /// Creates a default 1080p NV12 camera configuration for the primary port.
    pub fn default_1080p() -> Self {
        Self::new(CsiPort::Csi0FourLane, 0, 1920, 1080, CameraFormat::Nv12)
    }
}

/// Compatible camera module for the Dragon Q6A.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CameraModule {
    /// Radxa Camera 4K — 4K resolution, 31-pin 0.3mm FPC.
    Camera4K,
    /// Radxa Camera 8M 219 — IMX219 8MP, 15-pin 1.0mm FPC.
    Camera8M219,
    /// Radxa Camera 12M 577 — IMX577 12MP, 31-pin 0.3mm FPC.
    Camera12M577,
    /// Radxa Camera 13M 214 — 13MP, 31-pin 0.3mm FPC.
    Camera13M214,
}

impl CameraModule {
    /// Returns the sensor name (if known).
    pub fn sensor(&self) -> &'static str {
        match self {
            CameraModule::Camera4K => "unknown",
            CameraModule::Camera8M219 => "IMX219",
            CameraModule::Camera12M577 => "IMX577",
            CameraModule::Camera13M214 => "unknown",
        }
    }

    /// Returns the rsetup overlay name to enable this camera.
    pub fn rsetup_overlay(&self) -> &'static str {
        match self {
            CameraModule::Camera4K => "Enable Radxa Camera 4K on CAM1",
            CameraModule::Camera8M219 => "Enable IMX219 on CAM1",
            CameraModule::Camera12M577 => "Enable IMX577 camera on CAM1",
            CameraModule::Camera13M214 => "Enable Camera 13M 214 on CAM1",
        }
    }

    /// Returns the FPC connector pin count.
    pub fn fpc_pins(&self) -> u8 {
        match self {
            CameraModule::Camera8M219 => 15, // 15-pin 1.0mm pitch
            _ => 31,                          // 31-pin 0.3mm pitch
        }
    }

    /// Returns all compatible camera modules.
    pub fn all() -> &'static [CameraModule] {
        &[
            CameraModule::Camera4K,
            CameraModule::Camera8M219,
            CameraModule::Camera12M577,
            CameraModule::Camera13M214,
        ]
    }
}

impl fmt::Display for CameraModule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CameraModule::Camera4K => write!(f, "Radxa Camera 4K"),
            CameraModule::Camera8M219 => write!(f, "Radxa Camera 8M 219 (IMX219)"),
            CameraModule::Camera12M577 => write!(f, "Radxa Camera 12M 577 (IMX577)"),
            CameraModule::Camera13M214 => write!(f, "Radxa Camera 13M 214"),
        }
    }
}

/// Generates the libcamera qcam preview command.
pub fn libcamera_preview_command(width: u32, height: u32) -> String {
    format!("./qcam --renderer=gles --stream pixelformat=YUYV,width={width},height={height}")
}

/// Generates a v4l2-ctl command to capture a frame.
pub fn camera_capture_command(config: &CameraConfig, output_path: &str) -> String {
    format!(
        "v4l2-ctl --device /dev/video{} \
         --set-fmt-video=width={},height={},pixelformat={} \
         --stream-mmap --stream-count=1 --stream-to={output_path}",
        config.device_index, config.width, config.height, config.format
    )
}

// ═══════════════════════════════════════════════════════════════════════
// Deploy Commands
// ═══════════════════════════════════════════════════════════════════════

/// Generates SCP + SSH commands to deploy a binary to the Q6A over the network.
///
/// Default credentials: `radxa@<ip>`, password: `radxa`.
pub fn deploy_command(binary_path: &str, target_host: &str) -> String {
    let binary_name = binary_path.rsplit('/').next().unwrap_or(binary_path);
    format!(
        "scp {binary_path} {target_host}:~/bin/ && \
         ssh {target_host} 'chmod +x ~/bin/{binary_name} && ~/bin/{binary_name}'"
    )
}

/// Generates the cross-build command for the Dragon Q6A.
pub fn cross_build_command(project_path: &str) -> String {
    format!(
        "cd {project_path} && \
         cargo build --release --target aarch64-unknown-linux-gnu"
    )
}

// ═══════════════════════════════════════════════════════════════════════
// Performance Tuning (CPU/GPU Governor)
// ═══════════════════════════════════════════════════════════════════════

/// CPU frequency policy for the tri-cluster Kryo 670.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CpuPolicy {
    /// Policy 0: Silver cluster (4x A55), 300 MHz - 1.96 GHz.
    Silver,
    /// Policy 4: Gold cluster (3x A78), 691 MHz - 2.4 GHz.
    Gold,
    /// Policy 7: Prime cluster (1x A78), 806 MHz - 2.71 GHz.
    Prime,
}

impl CpuPolicy {
    /// Returns the sysfs path for this CPU policy.
    pub fn sysfs_path(&self) -> &'static str {
        match self {
            CpuPolicy::Silver => "/sys/devices/system/cpu/cpufreq/policy0",
            CpuPolicy::Gold => "/sys/devices/system/cpu/cpufreq/policy4",
            CpuPolicy::Prime => "/sys/devices/system/cpu/cpufreq/policy7",
        }
    }

    /// Returns the maximum frequency in KHz for this cluster.
    pub fn max_freq_khz(&self) -> u32 {
        match self {
            CpuPolicy::Silver => 1_958_400,
            CpuPolicy::Gold => 2_400_000,
            CpuPolicy::Prime => 2_710_000,
        }
    }
}

/// Available GPU frequencies in Hz for the Adreno 643.
pub const GPU_FREQUENCIES_HZ: [u32; 6] = [
    315_000_000,
    450_000_000,
    550_000_000,
    608_000_000,
    700_000_000,
    812_000_000,
];

/// Returns the sysfs path for GPU devfreq governor control.
pub fn gpu_devfreq_path() -> &'static str {
    "/sys/class/devfreq/3d00000.gpu"
}

/// NPU device path for FastRPC access.
pub fn npu_fastrpc_device() -> &'static str {
    "/dev/fastrpc-cdsp"
}

// ═══════════════════════════════════════════════════════════════════════
// Boot Chain & SPI Firmware
// ═══════════════════════════════════════════════════════════════════════

/// SPI NOR flash partition in the boot chain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpiPartition {
    /// eXtensible Bootloader — first stage after BootROM.
    Xbl,
    /// Device configuration data.
    Devcfg,
    /// LPDDR5 memory training data.
    Ddr,
    /// UEFI BIOS firmware.
    Uefi,
}

impl SpiPartition {
    /// Returns the partition name as used by edl-ng.
    pub fn name(&self) -> &'static str {
        match self {
            SpiPartition::Xbl => "xbl",
            SpiPartition::Devcfg => "devcfg",
            SpiPartition::Ddr => "ddr",
            SpiPartition::Uefi => "uefi",
        }
    }

    /// Returns all SPI partitions in boot order.
    pub fn all() -> &'static [SpiPartition] {
        &[
            SpiPartition::Xbl,
            SpiPartition::Devcfg,
            SpiPartition::Ddr,
            SpiPartition::Uefi,
        ]
    }
}

impl fmt::Display for SpiPartition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SpiPartition::Xbl => write!(f, "XBL (eXtensible Bootloader)"),
            SpiPartition::Devcfg => write!(f, "DEVCFG (Device Configuration)"),
            SpiPartition::Ddr => write!(f, "DDR (LPDDR5 Training Data)"),
            SpiPartition::Uefi => write!(f, "UEFI (BIOS Firmware)"),
        }
    }
}

/// EDL (Emergency Download) mode USB identifiers.
pub const EDL_USB_VENDOR_ID: u16 = 0x05c6;
/// EDL USB product ID (Qualcomm 9008 mode).
pub const EDL_USB_PRODUCT_ID: u16 = 0x9008;

/// Minimum required SPI firmware version for current OS images.
pub const MIN_SPI_FIRMWARE_VERSION: &str = "20251230";

/// Generates the edl-ng command to flash SPI boot firmware.
///
/// Requires the device to be in EDL mode (USB `05c6:9008`).
pub fn edl_flash_spi_command(firmware_dir: &str) -> String {
    format!(
        "sudo edl-ng --memory=spinor \
         rawprogram {firmware_dir}/rawprogram0.xml {firmware_dir}/patch0.xml \
         --loader={firmware_dir}/prog_firehose_ddr.elf"
    )
}

/// Generates the edl-ng command to erase a specific SPI partition.
///
/// **WARNING:** Erasing prevents boot. Re-flash immediately after.
pub fn edl_erase_partition_command(partition: SpiPartition, loader_path: &str) -> String {
    format!(
        "sudo edl-ng --memory spinor erase-part {} -l {loader_path}",
        partition.name()
    )
}

/// Returns the kernel source repository URL.
pub fn kernel_repo() -> &'static str {
    "https://github.com/radxa-pkg/linux-qcom.git"
}

/// Returns the OS SDK (rsdk) repository URL.
pub fn os_sdk_repo() -> &'static str {
    "https://github.com/RadxaOS-SDK/rsdk.git"
}

// ═══════════════════════════════════════════════════════════════════════
// Storage Interfaces
// ═══════════════════════════════════════════════════════════════════════

/// Storage interface type available on the Dragon Q6A.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageInterface {
    /// M.2 M Key 2230 NVMe SSD (PCIe Gen3 x2, ~1.6 GB/s read).
    NvmeM2,
    /// eMMC module via combo connector.
    Emmc,
    /// UFS module via combo connector.
    Ufs,
    /// MicroSD/SDHC/SDXC card slot.
    MicroSd,
}

impl fmt::Display for StorageInterface {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StorageInterface::NvmeM2 => write!(f, "M.2 M Key 2230 NVMe (PCIe Gen3 x2)"),
            StorageInterface::Emmc => write!(f, "eMMC module (combo connector)"),
            StorageInterface::Ufs => write!(f, "UFS module (combo connector)"),
            StorageInterface::MicroSd => write!(f, "MicroSD/SDHC/SDXC"),
        }
    }
}

/// Measured NVMe read speed in MB/s (PCIe Gen3 x2).
pub const NVME_READ_SPEED_MBS: u32 = 1649;
/// Measured NVMe write speed in MB/s (PCIe Gen3 x2).
pub const NVME_WRITE_SPEED_MBS: u32 = 1467;

// ═══════════════════════════════════════════════════════════════════════
// Power & RTC
// ═══════════════════════════════════════════════════════════════════════

/// Power supply method for the Dragon Q6A.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerMethod {
    /// 12V USB Type-C with PD protocol (recommended, min 2A).
    UsbTypeCPd,
    /// 12V via external power header pins (12V + GND).
    ExternalHeader,
    /// Power over Ethernet via PoE HAT accessory.
    PoeHat,
}

impl fmt::Display for PowerMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PowerMethod::UsbTypeCPd => write!(f, "12V USB Type-C PD (recommended, min 2A)"),
            PowerMethod::ExternalHeader => write!(f, "12V External Header (12V + GND pins)"),
            PowerMethod::PoeHat => write!(f, "PoE+ HAT (Ethernet power)"),
        }
    }
}

/// RTC device path.
pub const RTC_DEVICE: &str = "/dev/rtc0";
/// RTC chip model.
pub const RTC_CHIP: &str = "DS1307";
/// RTC battery type.
pub const RTC_BATTERY: &str = "CR2032";
/// Ethernet interface name.
pub const ETH_INTERFACE: &str = "enp1s0";

/// ALSA audio device for playback (Card 0, Device 1).
pub const ALSA_PLAYBACK: &str = "hw:0,1";
/// ALSA audio device for capture (Card 0, Device 2).
pub const ALSA_CAPTURE: &str = "hw:0,2";

// ═══════════════════════════════════════════════════════════════════════
// Board Implementation
// ═══════════════════════════════════════════════════════════════════════

/// Radxa Dragon Q6A edge AI SBC.
///
/// A credit-card sized ARM64 Linux SBC powered by Qualcomm QCS6490,
/// featuring Adreno 643 GPU, Hexagon 770 NPU (12 TOPS), and a 40-pin GPIO header.
///
/// This is Fajar Lang's first physical hardware deployment target ("Dawn").
pub struct DragonQ6A {
    /// RAM size in GB (4, 6, 8, 12, or 16).
    ram_gb: u8,
}

impl DragonQ6A {
    /// Creates a new Dragon Q6A board instance with the specified RAM size.
    pub fn new(ram_gb: u8) -> Self {
        Self { ram_gb }
    }

    /// Creates a Dragon Q6A with the maximum 16GB RAM configuration.
    pub fn new_16gb() -> Self {
        Self { ram_gb: 16 }
    }

    /// Returns the RAM size in GB.
    pub fn ram_gb(&self) -> u8 {
        self.ram_gb
    }

    /// Returns CPU information for the Kryo 670.
    pub fn cpu_info(&self) -> CpuInfo {
        CpuInfo::qcs6490_default()
    }

    /// Returns GPU capabilities for the Adreno 643.
    pub fn gpu_capabilities(&self) -> GpuCapabilities {
        gpu_info()
    }

    /// Returns NPU capabilities for the Hexagon 770.
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

    /// Returns the 40-pin GPIO header pinout.
    pub fn gpio_pinout(&self) -> Vec<HeaderPin> {
        gpio_header_pinout()
    }

    /// Returns the number of usable GPIO pins.
    pub fn gpio_count(&self) -> usize {
        gpio_pin_count()
    }
}

impl Default for DragonQ6A {
    fn default() -> Self {
        Self::new_16gb()
    }
}

impl Board for DragonQ6A {
    fn name(&self) -> &str {
        "Radxa Dragon Q6A"
    }

    fn arch(&self) -> BspArch {
        BspArch::Aarch64Linux
    }

    fn memory_regions(&self) -> Vec<MemoryRegion> {
        // Convert to u64 first to avoid overflow (16GB > u32::MAX)
        let ram_bytes_u64 = (self.ram_gb as u64) * 1024 * 1024 * 1024;
        // Cap at u32::MAX for the MemoryRegion API (actual size tracked by ram_gb field)
        let ram_region_size = if ram_bytes_u64 > u32::MAX as u64 {
            u32::MAX
        } else {
            ram_bytes_u64 as u32
        };

        vec![
            // RAM: LPDDR5 (configurable 4-16GB)
            MemoryRegion::new("RAM", 0x0000_0000, ram_region_size, MemoryAttr::Rwx),
            // NVMe SSD (conceptual, kernel-managed)
            MemoryRegion::new("NVME", 0x0000_0000, u32::MAX, MemoryAttr::Rw),
            // eMMC storage (conceptual)
            MemoryRegion::new("EMMC", 0x0000_0000, u32::MAX, MemoryAttr::Rw),
        ]
    }

    fn peripherals(&self) -> Vec<Peripheral> {
        let mut periphs = vec![
            // 40-pin GPIO header (via /dev/gpiochip4)
            Peripheral::new("GPIO_HEADER", 0x0000_0040),
            // UART ports (7 available)
            Peripheral::new("UART0", 0x0000_0100),
            Peripheral::new("UART2", 0x0000_0102),
            Peripheral::new("UART5", 0x0000_0105),
            Peripheral::new("UART6", 0x0000_0106),
            Peripheral::new("UART7", 0x0000_0107),
            Peripheral::new("UART12", 0x0000_010C),
            Peripheral::new("UART14", 0x0000_010E),
            // I2C buses (6 available)
            Peripheral::new("I2C0", 0x0000_0200),
            Peripheral::new("I2C2", 0x0000_0202),
            Peripheral::new("I2C6", 0x0000_0206),
            Peripheral::new("I2C7", 0x0000_0207),
            Peripheral::new("I2C12", 0x0000_020C),
            Peripheral::new("I2C14", 0x0000_020E),
            // SPI buses (7 available)
            Peripheral::new("SPI0", 0x0000_0300),
            Peripheral::new("SPI2", 0x0000_0302),
            Peripheral::new("SPI5", 0x0000_0305),
            Peripheral::new("SPI6", 0x0000_0306),
            Peripheral::new("SPI7", 0x0000_0307),
            Peripheral::new("SPI12", 0x0000_030C),
            Peripheral::new("SPI14", 0x0000_030E),
            // I2S audio
            Peripheral::new("MI2S0", 0x0000_0400),
            // I3C (next-gen I2C)
            Peripheral::new("I3C0", 0x0000_0500),
            // MIPI CSI cameras
            Peripheral::new("MIPI_CSI0", 0x0000_0600), // 4-lane
            Peripheral::new("MIPI_CSI1", 0x0000_0601), // 2-lane
            Peripheral::new("MIPI_CSI2", 0x0000_0602), // 2-lane
            // Display
            Peripheral::new("HDMI", 0x0000_0700),
            Peripheral::new("MIPI_DSI", 0x0000_0701),
            // USB
            Peripheral::new("USB31_A", 0x0000_0800), // USB 3.1 Type-A
            Peripheral::new("USB20_A0", 0x0000_0810), // USB 2.0 #1
            Peripheral::new("USB20_A1", 0x0000_0811), // USB 2.0 #2
            Peripheral::new("USB20_A2", 0x0000_0812), // USB 2.0 #3
            // Network
            Peripheral::new("ETH_1G", 0x0000_0900), // Gigabit Ethernet
            Peripheral::new("WIFI6", 0x0000_0910),  // WiFi 6 (802.11ax)
            Peripheral::new("BT54", 0x0000_0920),   // Bluetooth 5.4
        ];

        // Add GPIO registers to GPIO_HEADER peripheral
        if let Some(gpio) = periphs.iter_mut().find(|p| p.name == "GPIO_HEADER") {
            gpio.add_register("CHIP", 0x00, 4); // gpiochip4
            gpio.add_register("PIN_COUNT", 0x04, 4); // 26 usable GPIO pins
        }

        periphs
    }

    fn vector_table_size(&self) -> usize {
        // ARM64 Linux: no bare-metal vector table.
        0
    }

    fn cpu_frequency(&self) -> u32 {
        // Prime cluster max: 2.71 GHz
        2_710_000_000
    }

    fn generate_linker_script(&self) -> String {
        let mut script = String::new();
        script.push_str("/* Radxa Dragon Q6A — Linux userspace target */\n");
        script.push_str("/* No custom linker script needed for Linux ELF */\n");
        script.push_str("/* Uses system default: aarch64-unknown-linux-gnu */\n\n");
        script.push_str("/* Cross-build: */\n");
        script.push_str("/*   cargo build --release --target aarch64-unknown-linux-gnu */\n");
        script.push_str("/* Deploy: */\n");
        script.push_str("/*   scp target/aarch64.../release/fj radxa@<ip>:~/bin/ */\n");
        script
    }

    fn generate_startup_code(&self) -> String {
        let mut code = String::new();
        code.push_str("/* Radxa Dragon Q6A — Linux userspace target */\n");
        code.push_str("/* Standard Linux ELF entry — no custom startup needed */\n\n");
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

    // — Board Identity —

    #[test]
    fn q6a_board_identity() {
        let board = DragonQ6A::new_16gb();
        assert_eq!(board.name(), "Radxa Dragon Q6A");
        assert_eq!(board.arch(), BspArch::Aarch64Linux);
        assert_eq!(board.cpu_frequency(), 2_710_000_000);
        assert_eq!(board.ram_gb(), 16);
    }

    #[test]
    fn q6a_default_is_16gb() {
        let board = DragonQ6A::default();
        assert_eq!(board.ram_gb(), 16);
    }

    #[test]
    fn q6a_custom_ram() {
        let board = DragonQ6A::new(8);
        assert_eq!(board.ram_gb(), 8);
    }

    // — CPU —

    #[test]
    fn q6a_cpu_info() {
        let board = DragonQ6A::new_16gb();
        let cpu = board.cpu_info();
        assert_eq!(cpu.name, "Kryo 670");
        assert_eq!(cpu.isa, "ARMv8.2-A");
        assert_eq!(cpu.cores, 8);
        assert_eq!(cpu.max_freq_hz, 2_710_000_000);
        assert_eq!(cpu.l3_cache_bytes, 3 * 1024 * 1024);
    }

    #[test]
    fn q6a_cpu_display() {
        let cpu = CpuInfo::qcs6490_default();
        let s = format!("{cpu}");
        assert!(s.contains("Kryo 670"));
        assert!(s.contains("ARMv8.2-A"));
        assert!(s.contains("8 cores"));
        assert!(s.contains("2710MHz"));
    }

    #[test]
    fn q6a_cpu_cluster_display() {
        assert!(format!("{}", CpuCluster::Prime).contains("A78"));
        assert!(format!("{}", CpuCluster::Prime).contains("2.71GHz"));
        assert!(format!("{}", CpuCluster::Gold).contains("3x A78"));
        assert!(format!("{}", CpuCluster::Silver).contains("4x A55"));
    }

    // — GPU —

    #[test]
    fn q6a_gpu_capabilities() {
        let board = DragonQ6A::new_16gb();
        let gpu = board.gpu_capabilities();
        assert_eq!(gpu.name, "Adreno 643");
        assert_eq!(gpu.clock_mhz, 812);
        assert_eq!(gpu.vulkan_version, "1.1");
        assert_eq!(gpu.opencl_version, "2.0");
        assert_eq!(gpu.gles_version, "3.2");
        assert_eq!(gpu.fp32_gflops, 773);
    }

    #[test]
    fn q6a_gpu_display() {
        let gpu = GpuCapabilities::adreno643_default();
        let s = format!("{gpu}");
        assert!(s.contains("Adreno 643"));
        assert!(s.contains("812MHz"));
        assert!(s.contains("Vulkan 1.1"));
        assert!(s.contains("OpenCL 2.0"));
    }

    // — NPU —

    #[test]
    fn q6a_npu_capabilities() {
        let board = DragonQ6A::new_16gb();
        let npu = board.npu_capabilities();
        assert_eq!(npu.name, "Hexagon 770");
        assert_eq!(npu.isa_version, "V68");
        assert_eq!(npu.tops, 12);
        assert_eq!(npu.htp_skel_lib, "libQnnHtpV68Skel.so");
        assert!(npu.supported_dtypes.contains(&NpuDtype::F16));
        assert!(npu.supported_dtypes.contains(&NpuDtype::Int8));
        assert!(npu.supported_dtypes.contains(&NpuDtype::Int4));
    }

    #[test]
    fn q6a_npu_display() {
        let npu = NpuCapabilities::hexagon770_default();
        let s = format!("{npu}");
        assert!(s.contains("Hexagon 770"));
        assert!(s.contains("V68"));
        assert!(s.contains("12 TOPS"));
        assert!(s.contains("QNN 2.37.1"));
    }

    #[test]
    fn q6a_npu_dtype_display() {
        assert_eq!(format!("{}", NpuDtype::F16), "F16");
        assert_eq!(format!("{}", NpuDtype::Int8), "INT8");
        assert_eq!(format!("{}", NpuDtype::Int4), "INT4");
    }

    #[test]
    fn q6a_hexagon_component_display() {
        assert!(format!("{}", HexagonComponent::Scalar).contains("VLIW"));
        assert!(format!("{}", HexagonComponent::Hvx).contains("1024-bit"));
        assert!(format!("{}", HexagonComponent::Hmx).contains("Tensor"));
    }

    // — QNN —

    #[test]
    fn q6a_qnn_backend_library_names() {
        assert_eq!(QnnBackend::Cpu.library_name(), "libQnnCpu.so");
        assert_eq!(QnnBackend::Gpu.library_name(), "libQnnGpu.so");
        assert_eq!(QnnBackend::Htp.library_name(), "libQnnHtp.so");
    }

    #[test]
    fn q6a_qnn_backend_display() {
        assert!(format!("{}", QnnBackend::Htp).contains("Hexagon 770"));
        assert!(format!("{}", QnnBackend::Gpu).contains("Adreno 643"));
    }

    #[test]
    fn q6a_qnn_load_model_success() {
        let model = qnn_load_model("/opt/models/mobilenet.dlc", QnnBackend::Htp);
        assert!(model.is_ok());
        let model = model.unwrap();
        assert_eq!(model.name, "mobilenet");
        assert!(model.loaded);
        assert_eq!(model.backend, QnnBackend::Htp);
        assert_eq!(model.input_shape, vec![1, 3, 224, 224]);
        assert_eq!(model.output_shape, vec![1, 1000]);
    }

    #[test]
    fn q6a_qnn_load_model_empty_path() {
        let result = qnn_load_model("", QnnBackend::Htp);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            QnnError::ModelNotFound("(empty path)".to_string())
        );
    }

    #[test]
    fn q6a_qnn_infer_success() {
        let model = qnn_load_model("/opt/models/test.dlc", QnnBackend::Htp).unwrap();
        let input_size: usize = model.input_shape.iter().product();
        let input = vec![0.5; input_size];
        let result = qnn_infer(&model, &input);
        assert!(result.is_ok());
        let output = result.unwrap();
        let expected_size: usize = model.output_shape.iter().product();
        assert_eq!(output.len(), expected_size);
    }

    #[test]
    fn q6a_qnn_infer_shape_mismatch() {
        let model = qnn_load_model("/opt/models/test.dlc", QnnBackend::Cpu).unwrap();
        let result = qnn_infer(&model, &[1.0, 2.0, 3.0]);
        assert!(result.is_err());
        match result.unwrap_err() {
            QnnError::ShapeMismatch { expected, actual } => {
                assert_eq!(expected, vec![1, 3, 224, 224]);
                assert_eq!(actual, vec![3]);
            }
            other => panic!("Expected ShapeMismatch, got {other:?}"),
        }
    }

    #[test]
    fn q6a_qnn_infer_unloaded() {
        let model = QnnModel {
            name: "test".into(),
            input_shape: vec![1, 4],
            output_shape: vec![1, 2],
            loaded: false,
            backend: QnnBackend::Htp,
        };
        assert!(qnn_infer(&model, &[1.0, 2.0, 3.0, 4.0]).is_err());
    }

    #[test]
    fn q6a_qnn_error_display() {
        let e = QnnError::ModelNotFound("/bad".into());
        assert!(format!("{e}").contains("/bad"));
        assert!(format!("{}", QnnError::RuntimeNotAvailable).contains("not available"));
    }

    // — ONNX/QNN Pipeline —

    #[test]
    fn q6a_onnx_to_qnn_command() {
        let cmd = onnx_to_qnn_command("model.onnx", "/tmp/out");
        assert!(cmd.contains("qnn-onnx-converter"));
        assert!(cmd.contains("model.onnx"));
    }

    #[test]
    fn q6a_qnn_quantize_command() {
        let cmd = qnn_quantize_command("model.onnx", "calib.txt");
        assert!(cmd.contains("act_bw 8"));
        assert!(cmd.contains("weight_bw 8"));
        assert!(cmd.contains("calib.txt"));
    }

    #[test]
    fn q6a_qnn_context_binary_command() {
        let cmd = qnn_context_binary_command("/tmp/model.cpp", "/tmp/out");
        assert!(cmd.contains("qnn-context-binary-generator"));
        assert!(cmd.contains("libQnnHtp.so"));
    }

    #[test]
    fn q6a_qnn_net_run_command() {
        let cmd = qnn_net_run_command("model.so", QnnBackend::Htp);
        assert!(cmd.contains("qnn-net-run"));
        assert!(cmd.contains("libQnnHtp.so"));
    }

    // — GPIO —

    #[test]
    fn q6a_gpio_header_has_40_pins() {
        let pins = gpio_header_pinout();
        assert_eq!(pins.len(), 40);
    }

    #[test]
    fn q6a_gpio_usable_pins() {
        let count = gpio_pin_count();
        assert_eq!(count, 27, "should have 27 usable GPIO pins");
    }

    #[test]
    fn q6a_gpio_device_path() {
        assert_eq!(gpio_device_path(), "/dev/gpiochip4");
    }

    #[test]
    fn q6a_pin_by_physical() {
        let pin1 = pin_by_physical(1).unwrap();
        assert_eq!(pin1.label, "3V3");
        assert_eq!(pin1.default_function, PinFunction::Power);
        assert!(pin1.gpio_num.is_none());

        let pin7 = pin_by_physical(7).unwrap();
        assert_eq!(pin7.gpio_num, Some(96));
        assert_eq!(pin7.default_function, PinFunction::Mclk);

        let pin13 = pin_by_physical(13).unwrap();
        assert_eq!(pin13.gpio_num, Some(0));
        assert_eq!(pin13.default_function, PinFunction::Gpio);

        assert!(pin_by_physical(41).is_none());
    }

    #[test]
    fn q6a_pin_by_gpio() {
        let pin = pin_by_gpio(22).unwrap();
        assert_eq!(pin.physical, 8);
        assert_eq!(pin.default_function, PinFunction::UartTx);
        assert!(pin.label.contains("UART5_TX"));

        assert!(pin_by_gpio(999).is_none());
    }

    #[test]
    fn q6a_gpio_power_and_ground_pins() {
        let pins = gpio_header_pinout();
        let power: Vec<_> = pins
            .iter()
            .filter(|p| p.default_function == PinFunction::Power)
            .collect();
        let ground: Vec<_> = pins
            .iter()
            .filter(|p| p.default_function == PinFunction::Ground)
            .collect();
        assert!(
            power.len() >= 4,
            "should have at least 4 power pins (3V3+5V)"
        );
        assert!(ground.len() >= 7, "should have at least 7 ground pins");
    }

    #[test]
    fn q6a_gpio_uart_pins() {
        let pins = gpio_header_pinout();
        let uart_tx: Vec<_> = pins
            .iter()
            .filter(|p| p.default_function == PinFunction::UartTx)
            .collect();
        let uart_rx: Vec<_> = pins
            .iter()
            .filter(|p| p.default_function == PinFunction::UartRx)
            .collect();
        assert!(uart_tx.len() >= 3, "should have UART TX pins");
        assert!(uart_rx.len() >= 3, "should have UART RX pins");
    }

    #[test]
    fn q6a_gpio_i2c_pins() {
        let pins = gpio_header_pinout();
        let sda: Vec<_> = pins
            .iter()
            .filter(|p| p.default_function == PinFunction::I2cSda)
            .collect();
        let scl: Vec<_> = pins
            .iter()
            .filter(|p| p.default_function == PinFunction::I2cScl)
            .collect();
        assert!(sda.len() >= 3, "should have I2C SDA pins");
        assert!(scl.len() >= 3, "should have I2C SCL pins");
    }

    #[test]
    fn q6a_gpio_spi_pins() {
        let pins = gpio_header_pinout();
        let spi: Vec<_> = pins
            .iter()
            .filter(|p| {
                matches!(
                    p.default_function,
                    PinFunction::SpiMosi | PinFunction::SpiMiso | PinFunction::SpiCs
                )
            })
            .collect();
        assert!(spi.len() >= 4, "should have SPI pins (MOSI, MISO, CS)");
    }

    #[test]
    fn q6a_gpio_i2s_pins() {
        let pins = gpio_header_pinout();
        let i2s: Vec<_> = pins
            .iter()
            .filter(|p| {
                matches!(
                    p.default_function,
                    PinFunction::I2sWs | PinFunction::I2sData
                )
            })
            .collect();
        assert!(i2s.len() >= 3, "should have I2S pins (WS, DATA0, DATA1)");
    }

    #[test]
    fn q6a_pin_function_display() {
        assert_eq!(format!("{}", PinFunction::Gpio), "GPIO");
        assert_eq!(format!("{}", PinFunction::UartTx), "UART_TX");
        assert_eq!(format!("{}", PinFunction::I2cSda), "I2C_SDA");
        assert_eq!(format!("{}", PinFunction::SpiMosi), "SPI_MOSI");
        assert_eq!(format!("{}", PinFunction::I2sWs), "I2S_WS");
        assert_eq!(format!("{}", PinFunction::Power), "POWER");
        assert_eq!(format!("{}", PinFunction::Ground), "GND");
    }

    // — Camera —

    #[test]
    fn q6a_camera_ports() {
        assert!(format!("{}", CsiPort::Csi0FourLane).contains("4-lane"));
        assert!(format!("{}", CsiPort::Csi1TwoLane).contains("2-lane"));
        assert!(format!("{}", CsiPort::Csi2TwoLane).contains("2-lane"));
    }

    #[test]
    fn q6a_camera_default_1080p() {
        let cam = CameraConfig::default_1080p();
        assert_eq!(cam.port, CsiPort::Csi0FourLane);
        assert_eq!(cam.width, 1920);
        assert_eq!(cam.height, 1080);
        assert_eq!(cam.format, CameraFormat::Nv12);
    }

    #[test]
    fn q6a_camera_modules() {
        let modules = CameraModule::all();
        assert_eq!(modules.len(), 4);
    }

    #[test]
    fn q6a_camera_module_sensors() {
        assert_eq!(CameraModule::Camera8M219.sensor(), "IMX219");
        assert_eq!(CameraModule::Camera12M577.sensor(), "IMX577");
        assert_eq!(CameraModule::Camera4K.sensor(), "unknown");
    }

    #[test]
    fn q6a_camera_module_fpc_pins() {
        assert_eq!(CameraModule::Camera8M219.fpc_pins(), 15);
        assert_eq!(CameraModule::Camera4K.fpc_pins(), 31);
        assert_eq!(CameraModule::Camera12M577.fpc_pins(), 31);
    }

    #[test]
    fn q6a_camera_module_rsetup_overlay() {
        assert!(CameraModule::Camera12M577.rsetup_overlay().contains("IMX577"));
        assert!(CameraModule::Camera8M219.rsetup_overlay().contains("IMX219"));
    }

    #[test]
    fn q6a_camera_module_display() {
        assert!(format!("{}", CameraModule::Camera12M577).contains("IMX577"));
        assert!(format!("{}", CameraModule::Camera8M219).contains("IMX219"));
        assert!(format!("{}", CameraModule::Camera4K).contains("4K"));
    }

    #[test]
    fn q6a_libcamera_preview_command() {
        let cmd = libcamera_preview_command(1920, 1080);
        assert!(cmd.contains("qcam"));
        assert!(cmd.contains("YUYV"));
        assert!(cmd.contains("1920"));
        assert!(cmd.contains("1080"));
    }

    #[test]
    fn q6a_camera_format_display() {
        assert_eq!(format!("{}", CameraFormat::Yuyv), "YUYV");
        assert_eq!(format!("{}", CameraFormat::Nv12), "NV12");
        assert_eq!(format!("{}", CameraFormat::Mjpeg), "MJPEG");
        assert_eq!(format!("{}", CameraFormat::RawBayer), "RGGB");
    }

    #[test]
    fn q6a_camera_capture_command() {
        let cam = CameraConfig::new(CsiPort::Csi0FourLane, 0, 640, 480, CameraFormat::Yuyv);
        let cmd = camera_capture_command(&cam, "/tmp/frame.raw");
        assert!(cmd.contains("/dev/video0"));
        assert!(cmd.contains("640"));
        assert!(cmd.contains("YUYV"));
        assert!(cmd.contains("/tmp/frame.raw"));
    }

    // — Memory & Peripherals —

    #[test]
    fn q6a_memory_regions() {
        let board = DragonQ6A::new_16gb();
        let regions = board.memory_regions();
        assert_eq!(regions.len(), 3);
        assert_eq!(regions[0].name, "RAM");
        assert_eq!(regions[1].name, "NVME");
        assert_eq!(regions[2].name, "EMMC");
    }

    #[test]
    fn q6a_peripherals_gpio_header() {
        let board = DragonQ6A::new_16gb();
        let periphs = board.peripherals();
        assert!(periphs.iter().any(|p| p.name == "GPIO_HEADER"));
        let gpio = periphs.iter().find(|p| p.name == "GPIO_HEADER").unwrap();
        assert!(!gpio.registers.is_empty());
    }

    #[test]
    fn q6a_peripherals_uart_count() {
        let board = DragonQ6A::new_16gb();
        let periphs = board.peripherals();
        let uarts: Vec<_> = periphs
            .iter()
            .filter(|p| p.name.starts_with("UART"))
            .collect();
        assert_eq!(uarts.len(), 7, "should have 7 UART ports");
    }

    #[test]
    fn q6a_peripherals_i2c_count() {
        let board = DragonQ6A::new_16gb();
        let periphs = board.peripherals();
        let i2c: Vec<_> = periphs
            .iter()
            .filter(|p| p.name.starts_with("I2C"))
            .collect();
        assert_eq!(i2c.len(), 6, "should have 6 I2C buses");
    }

    #[test]
    fn q6a_peripherals_spi_count() {
        let board = DragonQ6A::new_16gb();
        let periphs = board.peripherals();
        let spi: Vec<_> = periphs
            .iter()
            .filter(|p| p.name.starts_with("SPI"))
            .collect();
        assert_eq!(spi.len(), 7, "should have 7 SPI buses");
    }

    #[test]
    fn q6a_peripherals_cameras() {
        let board = DragonQ6A::new_16gb();
        let periphs = board.peripherals();
        let cams: Vec<_> = periphs
            .iter()
            .filter(|p| p.name.starts_with("MIPI_CSI"))
            .collect();
        assert_eq!(cams.len(), 3, "should have 3 MIPI CSI cameras");
    }

    #[test]
    fn q6a_peripherals_connectivity() {
        let board = DragonQ6A::new_16gb();
        let periphs = board.peripherals();
        assert!(periphs.iter().any(|p| p.name == "ETH_1G"));
        assert!(periphs.iter().any(|p| p.name == "WIFI6"));
        assert!(periphs.iter().any(|p| p.name == "BT54"));
    }

    #[test]
    fn q6a_peripherals_display() {
        let board = DragonQ6A::new_16gb();
        let periphs = board.peripherals();
        assert!(periphs.iter().any(|p| p.name == "HDMI"));
        assert!(periphs.iter().any(|p| p.name == "MIPI_DSI"));
    }

    // — Deploy —

    #[test]
    fn q6a_deploy_command() {
        let cmd = deploy_command("/tmp/fj", "radxa@192.168.1.100");
        assert!(cmd.contains("scp"));
        assert!(cmd.contains("ssh"));
        assert!(cmd.contains("radxa@192.168.1.100"));
        assert!(cmd.contains("fj"));
    }

    #[test]
    fn q6a_cross_build_command() {
        let cmd = cross_build_command("/home/user/fajar-lang");
        assert!(cmd.contains("cargo build"));
        assert!(cmd.contains("--release"));
        assert!(cmd.contains("aarch64-unknown-linux-gnu"));
    }

    // — Linker/Startup —

    #[test]
    fn q6a_linker_script_linux() {
        let board = DragonQ6A::new_16gb();
        let script = board.generate_linker_script();
        assert!(script.contains("Linux userspace"));
        assert!(script.contains("aarch64-unknown-linux-gnu"));
    }

    #[test]
    fn q6a_startup_code_linux() {
        let board = DragonQ6A::new_16gb();
        let code = board.generate_startup_code();
        assert!(code.contains("_start"));
        assert!(code.contains("bl main"));
        assert!(code.contains("__NR_exit"));
    }

    #[test]
    fn q6a_vector_table_zero() {
        let board = DragonQ6A::new_16gb();
        assert_eq!(board.vector_table_size(), 0);
    }

    // — Boot Chain & SPI —

    #[test]
    fn q6a_spi_partitions() {
        let parts = SpiPartition::all();
        assert_eq!(parts.len(), 4);
        assert_eq!(parts[0], SpiPartition::Xbl);
        assert_eq!(parts[3], SpiPartition::Uefi);
    }

    #[test]
    fn q6a_spi_partition_names() {
        assert_eq!(SpiPartition::Xbl.name(), "xbl");
        assert_eq!(SpiPartition::Devcfg.name(), "devcfg");
        assert_eq!(SpiPartition::Ddr.name(), "ddr");
        assert_eq!(SpiPartition::Uefi.name(), "uefi");
    }

    #[test]
    fn q6a_spi_partition_display() {
        assert!(format!("{}", SpiPartition::Xbl).contains("eXtensible Bootloader"));
        assert!(format!("{}", SpiPartition::Uefi).contains("BIOS"));
        assert!(format!("{}", SpiPartition::Ddr).contains("LPDDR5"));
    }

    #[test]
    fn q6a_edl_usb_ids() {
        assert_eq!(EDL_USB_VENDOR_ID, 0x05c6);
        assert_eq!(EDL_USB_PRODUCT_ID, 0x9008);
    }

    #[test]
    fn q6a_edl_flash_command() {
        let cmd = edl_flash_spi_command("/tmp/fw");
        assert!(cmd.contains("edl-ng"));
        assert!(cmd.contains("--memory=spinor"));
        assert!(cmd.contains("rawprogram0.xml"));
        assert!(cmd.contains("patch0.xml"));
        assert!(cmd.contains("prog_firehose_ddr.elf"));
    }

    #[test]
    fn q6a_edl_erase_command() {
        let cmd = edl_erase_partition_command(SpiPartition::Uefi, "/tmp/prog_firehose_ddr.elf");
        assert!(cmd.contains("erase-part uefi"));
        assert!(cmd.contains("prog_firehose_ddr.elf"));
    }

    #[test]
    fn q6a_kernel_repo_url() {
        assert!(kernel_repo().contains("linux-qcom"));
    }

    #[test]
    fn q6a_os_sdk_repo_url() {
        assert!(os_sdk_repo().contains("rsdk"));
    }

    // — Storage —

    #[test]
    fn q6a_storage_interface_display() {
        assert!(format!("{}", StorageInterface::NvmeM2).contains("PCIe Gen3 x2"));
        assert!(format!("{}", StorageInterface::Emmc).contains("eMMC"));
        assert!(format!("{}", StorageInterface::Ufs).contains("UFS"));
        assert!(format!("{}", StorageInterface::MicroSd).contains("MicroSD"));
    }

    #[test]
    fn q6a_nvme_speeds() {
        assert_eq!(NVME_READ_SPEED_MBS, 1649);
        assert_eq!(NVME_WRITE_SPEED_MBS, 1467);
    }

    // — Power & RTC —

    #[test]
    fn q6a_power_method_display() {
        assert!(format!("{}", PowerMethod::UsbTypeCPd).contains("12V"));
        assert!(format!("{}", PowerMethod::UsbTypeCPd).contains("PD"));
        assert!(format!("{}", PowerMethod::ExternalHeader).contains("Header"));
        assert!(format!("{}", PowerMethod::PoeHat).contains("PoE"));
    }

    #[test]
    fn q6a_rtc_constants() {
        assert_eq!(RTC_DEVICE, "/dev/rtc0");
        assert_eq!(RTC_CHIP, "DS1307");
        assert_eq!(RTC_BATTERY, "CR2032");
    }

    #[test]
    fn q6a_eth_and_audio_constants() {
        assert_eq!(ETH_INTERFACE, "enp1s0");
        assert_eq!(ALSA_PLAYBACK, "hw:0,1");
        assert_eq!(ALSA_CAPTURE, "hw:0,2");
    }

    // — Board Info —

    #[test]
    fn q6a_board_gpio_count() {
        let board = DragonQ6A::new_16gb();
        assert_eq!(board.gpio_count(), 27);
    }

    #[test]
    fn q6a_board_gpio_pinout_length() {
        let board = DragonQ6A::new_16gb();
        assert_eq!(board.gpio_pinout().len(), 40);
    }
}
