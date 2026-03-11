//! Arduino VENTUNO Q composite board support package.
//!
//! The VENTUNO Q has a dual-brain architecture:
//! - **MCU**: STM32H5F5 (Cortex-M33, Zephyr RTOS) for real-time I/O
//! - **MPU**: Qualcomm Dragonwing IQ8 (ARM64 Linux, 40 TOPS NPU) for AI inference
//!
//! This module provides unified build, IPC, and deployment abstractions
//! for targeting both processors from a single Fajar Lang codebase.

use super::{Board, BspArch, MemoryAttr, MemoryRegion, Peripheral};
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// Context Routing
// ═══════════════════════════════════════════════════════════════════════

/// Target processor for a given code region.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextTarget {
    /// Route to the MCU (STM32H5F5, Cortex-M33).
    Mcu,
    /// Route to the MPU (Dragonwing IQ8, ARM64 Linux).
    Mpu,
    /// Either target (determined by call graph analysis).
    Either,
}

impl fmt::Display for ContextTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ContextTarget::Mcu => write!(f, "MCU (STM32H5F5)"),
            ContextTarget::Mpu => write!(f, "MPU (Dragonwing IQ8)"),
            ContextTarget::Either => write!(f, "Either"),
        }
    }
}

/// Routes a Fajar Lang context annotation to the appropriate target processor.
///
/// - `@kernel` -> MCU (real-time, bare-metal, Zephyr)
/// - `@device` (with NPU) -> MPU (Linux, NPU inference)
/// - `@safe` -> Either (determined by call graph)
/// - `@unsafe` -> Either
pub fn route_context(annotation: &str) -> ContextTarget {
    match annotation {
        "@kernel" => ContextTarget::Mcu,
        "@device" => ContextTarget::Mpu,
        "@safe" => ContextTarget::Either,
        "@unsafe" => ContextTarget::Either,
        _ => ContextTarget::Either,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// IPC Protocol
// ═══════════════════════════════════════════════════════════════════════

/// IPC channel type between MCU and MPU.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IpcChannelType {
    /// Shared memory (for high-bandwidth transfers).
    SharedMemory,
    /// UART serial (for simple messages).
    Uart,
}

impl fmt::Display for IpcChannelType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IpcChannelType::SharedMemory => write!(f, "SharedMemory"),
            IpcChannelType::Uart => write!(f, "UART"),
        }
    }
}

/// IPC protocol configuration.
#[derive(Debug, Clone)]
pub struct IpcProtocol {
    /// Channel type.
    pub channel_type: IpcChannelType,
    /// Message format version.
    pub msg_format: u8,
}

impl IpcProtocol {
    /// Creates a default IPC protocol (shared memory, format v1).
    pub fn default_shared_memory() -> Self {
        Self {
            channel_type: IpcChannelType::SharedMemory,
            msg_format: 1,
        }
    }

    /// Creates a UART-based IPC protocol.
    pub fn uart() -> Self {
        Self {
            channel_type: IpcChannelType::Uart,
            msg_format: 1,
        }
    }
}

/// IPC message type identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IpcMessageType {
    /// Sensor data from MCU to MPU (0x01).
    SensorData,
    /// Inference result from MPU to MCU (0x02).
    InferenceResult,
    /// Actuator command from MPU to MCU (0x03).
    ActuatorCommand,
    /// Heartbeat / keepalive (0x04).
    Heartbeat,
    /// Error condition (0xFF).
    Error,
}

impl IpcMessageType {
    /// Returns the numeric ID for this message type.
    pub fn id(&self) -> u16 {
        match self {
            IpcMessageType::SensorData => 0x01,
            IpcMessageType::InferenceResult => 0x02,
            IpcMessageType::ActuatorCommand => 0x03,
            IpcMessageType::Heartbeat => 0x04,
            IpcMessageType::Error => 0xFF,
        }
    }

    /// Creates a message type from a numeric ID.
    pub fn from_id(id: u16) -> Option<Self> {
        match id {
            0x01 => Some(IpcMessageType::SensorData),
            0x02 => Some(IpcMessageType::InferenceResult),
            0x03 => Some(IpcMessageType::ActuatorCommand),
            0x04 => Some(IpcMessageType::Heartbeat),
            0xFF => Some(IpcMessageType::Error),
            _ => None,
        }
    }
}

impl fmt::Display for IpcMessageType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IpcMessageType::SensorData => write!(f, "SensorData"),
            IpcMessageType::InferenceResult => write!(f, "InferenceResult"),
            IpcMessageType::ActuatorCommand => write!(f, "ActuatorCommand"),
            IpcMessageType::Heartbeat => write!(f, "Heartbeat"),
            IpcMessageType::Error => write!(f, "Error"),
        }
    }
}

/// An IPC message between MCU and MPU.
#[derive(Debug, Clone)]
pub struct IpcMessage {
    /// Message type.
    pub msg_type: u16,
    /// Payload length in bytes.
    pub payload_len: u16,
    /// Payload data.
    pub payload: Vec<u8>,
    /// CRC16 checksum of (msg_type || payload_len || payload).
    pub checksum: u16,
}

/// Maximum payload size for an IPC message (4KB).
pub const IPC_MAX_PAYLOAD: usize = 4096;

/// IPC header size: msg_type (2) + payload_len (2) + checksum (2) = 6 bytes.
pub const IPC_HEADER_SIZE: usize = 6;

impl IpcMessage {
    /// Creates a new IPC message with computed checksum.
    pub fn new(msg_type: IpcMessageType, payload: Vec<u8>) -> Self {
        let payload_len = payload.len() as u16;
        let mut msg = Self {
            msg_type: msg_type.id(),
            payload_len,
            payload,
            checksum: 0,
        };
        msg.checksum = msg.compute_checksum();
        msg
    }

    /// Computes the CRC16 checksum over (msg_type || payload_len || payload).
    fn compute_checksum(&self) -> u16 {
        let mut sum: u32 = 0;
        sum = sum.wrapping_add(self.msg_type as u32);
        sum = sum.wrapping_add(self.payload_len as u32);
        for &b in &self.payload {
            sum = sum.wrapping_add(b as u32);
        }
        (sum & 0xFFFF) as u16
    }

    /// Validates the checksum.
    pub fn verify_checksum(&self) -> bool {
        self.checksum == self.compute_checksum()
    }
}

/// Errors from IPC message encoding/decoding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IpcError {
    /// Invalid or missing header bytes.
    InvalidHeader,
    /// Checksum does not match computed value.
    ChecksumMismatch {
        /// Expected checksum.
        expected: u16,
        /// Actual checksum.
        actual: u16,
    },
    /// Payload exceeds maximum allowed size.
    PayloadTooLarge(usize),
    /// Unknown message type ID.
    UnknownMessageType(u16),
}

impl fmt::Display for IpcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IpcError::InvalidHeader => write!(f, "IPC: invalid header"),
            IpcError::ChecksumMismatch { expected, actual } => {
                write!(
                    f,
                    "IPC: checksum mismatch (expected {expected:#06X}, got {actual:#06X})"
                )
            }
            IpcError::PayloadTooLarge(size) => {
                write!(f, "IPC: payload too large ({size} > {IPC_MAX_PAYLOAD})")
            }
            IpcError::UnknownMessageType(id) => {
                write!(f, "IPC: unknown message type {id:#06X}")
            }
        }
    }
}

/// Encodes an IPC message into a byte buffer.
///
/// Wire format (little-endian):
/// ```text
/// [msg_type: u16][payload_len: u16][payload: N bytes][checksum: u16]
/// ```
pub fn encode_message(msg: &IpcMessage) -> Result<Vec<u8>, IpcError> {
    if msg.payload.len() > IPC_MAX_PAYLOAD {
        return Err(IpcError::PayloadTooLarge(msg.payload.len()));
    }

    let mut buf = Vec::with_capacity(IPC_HEADER_SIZE + msg.payload.len());
    buf.extend_from_slice(&msg.msg_type.to_le_bytes());
    buf.extend_from_slice(&msg.payload_len.to_le_bytes());
    buf.extend_from_slice(&msg.payload);
    buf.extend_from_slice(&msg.checksum.to_le_bytes());
    Ok(buf)
}

/// Decodes an IPC message from a byte buffer.
pub fn decode_message(bytes: &[u8]) -> Result<IpcMessage, IpcError> {
    // Minimum size: header (4 bytes) + checksum (2 bytes) = 6 bytes
    if bytes.len() < IPC_HEADER_SIZE {
        return Err(IpcError::InvalidHeader);
    }

    let msg_type = u16::from_le_bytes([bytes[0], bytes[1]]);
    let payload_len = u16::from_le_bytes([bytes[2], bytes[3]]);

    // Validate message type
    if IpcMessageType::from_id(msg_type).is_none() {
        return Err(IpcError::UnknownMessageType(msg_type));
    }

    let expected_len = 4 + payload_len as usize + 2;
    if bytes.len() < expected_len {
        return Err(IpcError::InvalidHeader);
    }

    let payload = bytes[4..4 + payload_len as usize].to_vec();
    let checksum_offset = 4 + payload_len as usize;
    let checksum = u16::from_le_bytes([bytes[checksum_offset], bytes[checksum_offset + 1]]);

    if payload.len() > IPC_MAX_PAYLOAD {
        return Err(IpcError::PayloadTooLarge(payload.len()));
    }

    let msg = IpcMessage {
        msg_type,
        payload_len,
        payload,
        checksum,
    };

    if !msg.verify_checksum() {
        return Err(IpcError::ChecksumMismatch {
            expected: msg.compute_checksum(),
            actual: checksum,
        });
    }

    Ok(msg)
}

// ═══════════════════════════════════════════════════════════════════════
// Dual-Target Build
// ═══════════════════════════════════════════════════════════════════════

/// Dual-target build configuration.
#[derive(Debug, Clone)]
pub struct DualTargetBuild {
    /// Entry point function for the MCU binary.
    pub mcu_entry: String,
    /// Entry point function for the MPU binary.
    pub mpu_entry: String,
    /// Type names shared between MCU and MPU.
    pub shared_types: Vec<String>,
    /// IPC protocol configuration.
    pub ipc_protocol: IpcProtocol,
}

impl DualTargetBuild {
    /// Creates a new dual-target build configuration.
    pub fn new(mcu_entry: &str, mpu_entry: &str) -> Self {
        Self {
            mcu_entry: mcu_entry.to_string(),
            mpu_entry: mpu_entry.to_string(),
            shared_types: Vec::new(),
            ipc_protocol: IpcProtocol::default_shared_memory(),
        }
    }

    /// Adds a shared type name.
    pub fn add_shared_type(&mut self, type_name: &str) {
        self.shared_types.push(type_name.to_string());
    }
}

/// Build manifest for dual-target compilation.
#[derive(Debug, Clone)]
pub struct DualBuildConfig {
    /// MCU entry point function.
    pub mcu_entry: String,
    /// MPU entry point function.
    pub mpu_entry: String,
    /// IPC channel configuration.
    pub ipc_channel: IpcChannelType,
    /// Shared type names.
    pub shared_types: Vec<String>,
}

/// Generates build commands for both MCU and MPU targets.
///
/// Returns `(mcu_build_cmd, mpu_build_cmd)`.
pub fn generate_build_commands(config: &DualBuildConfig, board_name: &str) -> (String, String) {
    let mcu_cmd = format!(
        "fj build --target thumbv8m.main-none-eabihf \
         --entry {} --board {board_name}-mcu",
        config.mcu_entry
    );

    let mpu_cmd = format!(
        "fj build --target aarch64-unknown-linux-gnu \
         --entry {} --board {board_name}-mpu",
        config.mpu_entry
    );

    (mcu_cmd, mpu_cmd)
}

// ═══════════════════════════════════════════════════════════════════════
// Memory Budget (Dual)
// ═══════════════════════════════════════════════════════════════════════

/// Generates a dual memory budget report for both MCU and MPU.
///
/// All sizes are in bytes.
pub fn dual_memory_budget(
    mcu_flash_used: u32,
    mcu_sram_used: u32,
    mpu_ram_used_mb: u32,
    mpu_storage_used_mb: u32,
) -> String {
    let mut report = String::new();
    report.push_str("VENTUNO Q Dual Memory Budget\n");
    report.push_str(&"=".repeat(60));
    report.push('\n');

    // MCU section
    report.push_str("\n[MCU] STM32H5F5 (Cortex-M33)\n");
    report.push_str(&"-".repeat(40));
    report.push('\n');

    let mcu_flash_total: u32 = 2 * 1024 * 1024; // 2MB flash
    let mcu_sram_total: u32 = 640 * 1024; // 640KB SRAM
    let flash_pct = (mcu_flash_used as f64 / mcu_flash_total as f64) * 100.0;
    let sram_pct = (mcu_sram_used as f64 / mcu_sram_total as f64) * 100.0;

    report.push_str(&format!(
        "  Flash: {:>7} / {:>7} ({:>5.1}%)\n",
        format_size_bytes(mcu_flash_used),
        format_size_bytes(mcu_flash_total),
        flash_pct
    ));
    report.push_str(&format!(
        "  SRAM:  {:>7} / {:>7} ({:>5.1}%)\n",
        format_size_bytes(mcu_sram_used),
        format_size_bytes(mcu_sram_total),
        sram_pct
    ));

    // MPU section
    report.push_str("\n[MPU] Dragonwing IQ8 (ARM64 Linux)\n");
    report.push_str(&"-".repeat(40));
    report.push('\n');

    let mpu_ram_total_mb: u32 = 16 * 1024; // 16GB in MB
    let mpu_storage_total_mb: u32 = 64 * 1024; // 64GB in MB
    let ram_pct = (mpu_ram_used_mb as f64 / mpu_ram_total_mb as f64) * 100.0;
    let storage_pct = (mpu_storage_used_mb as f64 / mpu_storage_total_mb as f64) * 100.0;

    report.push_str(&format!(
        "  RAM:     {:>6}MB / {:>6}MB ({:>5.1}%)\n",
        mpu_ram_used_mb, mpu_ram_total_mb, ram_pct
    ));
    report.push_str(&format!(
        "  Storage: {:>6}MB / {:>6}MB ({:>5.1}%)\n",
        mpu_storage_used_mb, mpu_storage_total_mb, storage_pct
    ));

    report.push('\n');
    report.push_str(&"=".repeat(60));
    report.push('\n');

    // Warnings
    let mut warnings = false;
    if mcu_flash_used > mcu_flash_total {
        report.push_str("WARNING: MCU Flash budget exceeded!\n");
        warnings = true;
    }
    if mcu_sram_used > mcu_sram_total {
        report.push_str("WARNING: MCU SRAM budget exceeded!\n");
        warnings = true;
    }
    if mpu_ram_used_mb > mpu_ram_total_mb {
        report.push_str("WARNING: MPU RAM budget exceeded!\n");
        warnings = true;
    }
    if mpu_storage_used_mb > mpu_storage_total_mb {
        report.push_str("WARNING: MPU Storage budget exceeded!\n");
        warnings = true;
    }
    if !warnings {
        report.push_str("OK: All regions within budget.\n");
    }

    report
}

/// Formats a byte count as a human-readable string.
fn format_size_bytes(bytes: u32) -> String {
    if bytes >= 1024 * 1024 {
        format!("{}MB", bytes / (1024 * 1024))
    } else if bytes >= 1024 {
        format!("{}KB", bytes / 1024)
    } else {
        format!("{}B", bytes)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Board Implementation
// ═══════════════════════════════════════════════════════════════════════

/// Arduino VENTUNO Q composite board.
///
/// This board combines an STM32H5F5 MCU (Cortex-M33, real-time I/O)
/// with a Qualcomm Dragonwing IQ8 MPU (ARM64 Linux, 40 TOPS NPU).
pub struct VentunoQ {
    _private: (),
}

impl VentunoQ {
    /// Creates a new VENTUNO Q board instance.
    pub fn new() -> Self {
        Self { _private: () }
    }

    /// Returns true because this is a dual-target board.
    pub fn is_dual_target(&self) -> bool {
        true
    }

    /// Returns the Rust target triple for the MCU (Cortex-M33).
    pub fn mcu_target(&self) -> &str {
        "thumbv8m.main-none-eabihf"
    }

    /// Returns the Rust target triple for the MPU (ARM64 Linux).
    pub fn mpu_target(&self) -> &str {
        "aarch64-unknown-linux-gnu"
    }

    /// Returns the MCU board name.
    pub fn mcu_board_name(&self) -> &str {
        "stm32h5f5"
    }

    /// Returns the MPU board name.
    pub fn mpu_board_name(&self) -> &str {
        "dragonwing-iq8"
    }
}

impl Default for VentunoQ {
    fn default() -> Self {
        Self::new()
    }
}

impl Board for VentunoQ {
    fn name(&self) -> &str {
        "Arduino VENTUNO Q"
    }

    fn arch(&self) -> BspArch {
        // Primary architecture is the MPU (Linux) side
        BspArch::Aarch64Linux
    }

    fn memory_regions(&self) -> Vec<MemoryRegion> {
        // Combined view: MCU + MPU memory regions
        vec![
            // MCU Flash (STM32H5F5)
            MemoryRegion::new("MCU_FLASH", 0x0800_0000, 2 * 1024 * 1024, MemoryAttr::Rx),
            // MCU SRAM (STM32H5F5)
            MemoryRegion::new("MCU_SRAM", 0x2000_0000, 640 * 1024, MemoryAttr::Rw),
            // MPU RAM (Dragonwing, conceptual)
            MemoryRegion::new("MPU_RAM", 0x0000_0000, u32::MAX, MemoryAttr::Rwx),
        ]
    }

    fn peripherals(&self) -> Vec<Peripheral> {
        // Combined peripheral view
        vec![
            // MCU peripherals
            Peripheral::new("MCU_GPIO", 0x4202_0000),
            Peripheral::new("MCU_FDCAN1", 0x4000_A400),
            Peripheral::new("MCU_FDCAN2", 0x4000_A800),
            Peripheral::new("MCU_USART1", 0x4001_1000),
            // MPU peripherals
            Peripheral::new("MPU_USB_C", 0x0000_0001),
            Peripheral::new("MPU_MIPI_CSI0", 0x0000_0010),
            Peripheral::new("MPU_MIPI_CSI1", 0x0000_0011),
            Peripheral::new("MPU_MIPI_CSI2", 0x0000_0012),
            Peripheral::new("MPU_ETH_2G5", 0x0000_0020),
            Peripheral::new("MPU_WIFI6", 0x0000_0030),
        ]
    }

    fn vector_table_size(&self) -> usize {
        // MCU vector table (Cortex-M33): 16 system + ~150 IRQs
        166
    }

    fn cpu_frequency(&self) -> u32 {
        // MCU clock: 250 MHz (STM32H5F5)
        250_000_000
    }

    fn generate_linker_script(&self) -> String {
        let mut script = String::new();
        script.push_str("/* Arduino VENTUNO Q — Dual-Target Board */\n");
        script.push_str("/* MCU: STM32H5F5 (Cortex-M33 @ 250MHz) */\n");
        script.push_str("/* MPU: Dragonwing IQ8 (ARM64 Linux) */\n\n");
        script.push_str("/* MCU linker script (thumbv8m.main-none-eabihf) */\n");
        script.push_str("MEMORY\n{\n");
        script.push_str("  FLASH (rx) : ORIGIN = 0x08000000, LENGTH = 2048K\n");
        script.push_str("  SRAM  (rw) : ORIGIN = 0x20000000, LENGTH = 640K\n");
        script.push_str("}\n\n");
        script.push_str("ENTRY(Reset_Handler)\n\n");
        script.push_str("/* MPU uses default Linux aarch64 linker script */\n");
        script
    }

    fn generate_startup_code(&self) -> String {
        let mut code = String::new();
        code.push_str("/* Arduino VENTUNO Q — MCU Startup (Cortex-M33) */\n\n");
        code.push_str(".syntax unified\n");
        code.push_str(".cpu cortex-m33\n");
        code.push_str(".fpu fpv5-sp-d16\n");
        code.push_str(".thumb\n\n");
        code.push_str(".global Reset_Handler\n");
        code.push_str(".type Reset_Handler, %function\n");
        code.push_str("Reset_Handler:\n");
        code.push_str("  ldr r0, =_sdata\n");
        code.push_str("  ldr r1, =_edata\n");
        code.push_str("  ldr r2, =_sidata\n");
        code.push_str("  movs r3, #0\n");
        code.push_str("  b CopyDataLoop\n\n");
        code.push_str("CopyDataInit:\n");
        code.push_str("  ldr r4, [r2, r3]\n");
        code.push_str("  str r4, [r0, r3]\n");
        code.push_str("  adds r3, r3, #4\n\n");
        code.push_str("CopyDataLoop:\n");
        code.push_str("  adds r4, r0, r3\n");
        code.push_str("  cmp r4, r1\n");
        code.push_str("  bcc CopyDataInit\n\n");
        code.push_str("  ldr r2, =_sbss\n");
        code.push_str("  ldr r4, =_ebss\n");
        code.push_str("  movs r3, #0\n");
        code.push_str("  b ZeroBssLoop\n\n");
        code.push_str("ZeroBssInit:\n");
        code.push_str("  str r3, [r2]\n");
        code.push_str("  adds r2, r2, #4\n\n");
        code.push_str("ZeroBssLoop:\n");
        code.push_str("  cmp r2, r4\n");
        code.push_str("  bcc ZeroBssInit\n\n");
        code.push_str("  /* Enable FPU (Cortex-M33) */\n");
        code.push_str("  ldr r0, =0xE000ED88\n");
        code.push_str("  ldr r1, [r0]\n");
        code.push_str("  orr r1, r1, #(0xF << 20)\n");
        code.push_str("  str r1, [r0]\n");
        code.push_str("  dsb\n");
        code.push_str("  isb\n\n");
        code.push_str("  bl main\n");
        code.push_str("  b .\n");
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
    fn ventuno_q_board_identity() {
        let board = VentunoQ::new();
        assert_eq!(board.name(), "Arduino VENTUNO Q");
        assert_eq!(board.arch(), BspArch::Aarch64Linux);
        assert_eq!(board.cpu_frequency(), 250_000_000);
    }

    #[test]
    fn ventuno_q_default_trait() {
        let board = VentunoQ::default();
        assert_eq!(board.name(), "Arduino VENTUNO Q");
    }

    #[test]
    fn ventuno_q_is_dual_target() {
        let board = VentunoQ::new();
        assert!(board.is_dual_target());
    }

    #[test]
    fn ventuno_q_target_triples() {
        let board = VentunoQ::new();
        assert_eq!(board.mcu_target(), "thumbv8m.main-none-eabihf");
        assert_eq!(board.mpu_target(), "aarch64-unknown-linux-gnu");
    }

    #[test]
    fn ventuno_q_board_names() {
        let board = VentunoQ::new();
        assert_eq!(board.mcu_board_name(), "stm32h5f5");
        assert_eq!(board.mpu_board_name(), "dragonwing-iq8");
    }

    #[test]
    fn ventuno_q_memory_regions_dual() {
        let board = VentunoQ::new();
        let regions = board.memory_regions();
        assert!(regions.len() >= 3);
        assert!(regions.iter().any(|r| r.name == "MCU_FLASH"));
        assert!(regions.iter().any(|r| r.name == "MCU_SRAM"));
        assert!(regions.iter().any(|r| r.name == "MPU_RAM"));
    }

    #[test]
    fn ventuno_q_peripherals_both_domains() {
        let board = VentunoQ::new();
        let periphs = board.peripherals();
        // MCU peripherals
        assert!(periphs.iter().any(|p| p.name == "MCU_GPIO"));
        assert!(periphs.iter().any(|p| p.name == "MCU_FDCAN1"));
        // MPU peripherals
        assert!(periphs.iter().any(|p| p.name == "MPU_USB_C"));
        assert!(periphs.iter().any(|p| p.name == "MPU_WIFI6"));
    }

    #[test]
    fn ventuno_q_linker_script_dual() {
        let board = VentunoQ::new();
        let script = board.generate_linker_script();
        assert!(script.contains("VENTUNO Q"));
        assert!(script.contains("FLASH"));
        assert!(script.contains("SRAM"));
        assert!(script.contains("aarch64"));
    }

    #[test]
    fn ventuno_q_startup_code_cortex_m33() {
        let board = VentunoQ::new();
        let code = board.generate_startup_code();
        assert!(code.contains("cortex-m33"));
        assert!(code.contains("Reset_Handler"));
        assert!(code.contains("Enable FPU"));
        assert!(code.contains("bl main"));
    }

    // ── Context Routing Tests ──────────────────────────────────────

    #[test]
    fn context_routing_kernel_to_mcu() {
        assert_eq!(route_context("@kernel"), ContextTarget::Mcu);
    }

    #[test]
    fn context_routing_device_to_mpu() {
        assert_eq!(route_context("@device"), ContextTarget::Mpu);
    }

    #[test]
    fn context_routing_safe_to_either() {
        assert_eq!(route_context("@safe"), ContextTarget::Either);
    }

    #[test]
    fn context_routing_unsafe_to_either() {
        assert_eq!(route_context("@unsafe"), ContextTarget::Either);
    }

    #[test]
    fn context_routing_unknown_to_either() {
        assert_eq!(route_context("@unknown"), ContextTarget::Either);
    }

    #[test]
    fn context_target_display() {
        assert!(format!("{}", ContextTarget::Mcu).contains("MCU"));
        assert!(format!("{}", ContextTarget::Mpu).contains("MPU"));
        assert!(format!("{}", ContextTarget::Either).contains("Either"));
    }

    // ── IPC Tests ──────────────────────────────────────────────────

    #[test]
    fn ipc_message_type_ids() {
        assert_eq!(IpcMessageType::SensorData.id(), 0x01);
        assert_eq!(IpcMessageType::InferenceResult.id(), 0x02);
        assert_eq!(IpcMessageType::ActuatorCommand.id(), 0x03);
        assert_eq!(IpcMessageType::Heartbeat.id(), 0x04);
        assert_eq!(IpcMessageType::Error.id(), 0xFF);
    }

    #[test]
    fn ipc_message_type_from_id() {
        assert_eq!(
            IpcMessageType::from_id(0x01),
            Some(IpcMessageType::SensorData)
        );
        assert_eq!(
            IpcMessageType::from_id(0x02),
            Some(IpcMessageType::InferenceResult)
        );
        assert_eq!(IpcMessageType::from_id(0x99), None);
    }

    #[test]
    fn ipc_encode_decode_roundtrip() {
        let original = IpcMessage::new(IpcMessageType::SensorData, vec![0x01, 0x02, 0x03, 0x04]);

        let encoded = encode_message(&original).unwrap();
        let decoded = decode_message(&encoded).unwrap();

        assert_eq!(decoded.msg_type, original.msg_type);
        assert_eq!(decoded.payload_len, original.payload_len);
        assert_eq!(decoded.payload, original.payload);
        assert_eq!(decoded.checksum, original.checksum);
    }

    #[test]
    fn ipc_encode_decode_heartbeat() {
        let msg = IpcMessage::new(IpcMessageType::Heartbeat, vec![]);
        let encoded = encode_message(&msg).unwrap();
        let decoded = decode_message(&encoded).unwrap();
        assert_eq!(decoded.msg_type, IpcMessageType::Heartbeat.id());
        assert!(decoded.payload.is_empty());
    }

    #[test]
    fn ipc_decode_invalid_header() {
        let result = decode_message(&[0x01, 0x02]);
        assert_eq!(result.unwrap_err(), IpcError::InvalidHeader);
    }

    #[test]
    fn ipc_decode_unknown_message_type() {
        let bytes = [0x99, 0x00, 0x00, 0x00, 0x00, 0x00];
        let result = decode_message(&bytes);
        assert_eq!(result.unwrap_err(), IpcError::UnknownMessageType(0x0099));
    }

    #[test]
    fn ipc_decode_checksum_mismatch() {
        let msg = IpcMessage::new(IpcMessageType::SensorData, vec![0xAA, 0xBB]);
        let mut encoded = encode_message(&msg).unwrap();
        // Corrupt the checksum
        let last = encoded.len() - 1;
        encoded[last] ^= 0xFF;
        let result = decode_message(&encoded);
        assert!(matches!(
            result.unwrap_err(),
            IpcError::ChecksumMismatch { .. }
        ));
    }

    #[test]
    fn ipc_checksum_verification() {
        let msg = IpcMessage::new(IpcMessageType::ActuatorCommand, vec![0x10, 0x20, 0x30]);
        assert!(msg.verify_checksum());

        let mut bad_msg = msg.clone();
        bad_msg.checksum = 0xDEAD;
        assert!(!bad_msg.verify_checksum());
    }

    #[test]
    fn ipc_error_display() {
        let e = IpcError::InvalidHeader;
        assert!(format!("{e}").contains("invalid header"));

        let e = IpcError::PayloadTooLarge(9999);
        assert!(format!("{e}").contains("9999"));

        let e = IpcError::UnknownMessageType(0x42);
        assert!(format!("{e}").contains("0x0042"));
    }

    #[test]
    fn ipc_protocol_default() {
        let proto = IpcProtocol::default_shared_memory();
        assert_eq!(proto.channel_type, IpcChannelType::SharedMemory);
        assert_eq!(proto.msg_format, 1);
    }

    #[test]
    fn ipc_protocol_uart() {
        let proto = IpcProtocol::uart();
        assert_eq!(proto.channel_type, IpcChannelType::Uart);
    }

    // ── Build Command Tests ────────────────────────────────────────

    #[test]
    fn dual_target_build_creation() {
        let mut build = DualTargetBuild::new("mcu_main", "mpu_main");
        build.add_shared_type("SensorReading");
        build.add_shared_type("InferenceResult");

        assert_eq!(build.mcu_entry, "mcu_main");
        assert_eq!(build.mpu_entry, "mpu_main");
        assert_eq!(build.shared_types.len(), 2);
    }

    #[test]
    fn generate_build_commands_format() {
        let config = DualBuildConfig {
            mcu_entry: "mcu_main".to_string(),
            mpu_entry: "mpu_main".to_string(),
            ipc_channel: IpcChannelType::SharedMemory,
            shared_types: vec!["SensorData".to_string()],
        };

        let (mcu_cmd, mpu_cmd) = generate_build_commands(&config, "ventuno-q");

        assert!(mcu_cmd.contains("thumbv8m.main-none-eabihf"));
        assert!(mcu_cmd.contains("mcu_main"));
        assert!(mcu_cmd.contains("ventuno-q-mcu"));

        assert!(mpu_cmd.contains("aarch64-unknown-linux-gnu"));
        assert!(mpu_cmd.contains("mpu_main"));
        assert!(mpu_cmd.contains("ventuno-q-mpu"));
    }

    // ── Memory Budget Tests ────────────────────────────────────────

    #[test]
    fn dual_memory_budget_within_limits() {
        let report = dual_memory_budget(
            100 * 1024, // 100KB flash
            32 * 1024,  // 32KB SRAM
            512,        // 512MB RAM
            2 * 1024,   // 2GB storage
        );

        assert!(report.contains("VENTUNO Q"));
        assert!(report.contains("STM32H5F5"));
        assert!(report.contains("Dragonwing IQ8"));
        assert!(report.contains("OK: All regions within budget."));
    }

    #[test]
    fn dual_memory_budget_exceeded() {
        let report = dual_memory_budget(
            3 * 1024 * 1024, // 3MB flash (exceeds 2MB)
            32 * 1024,
            512,
            2 * 1024,
        );

        assert!(report.contains("WARNING"));
        assert!(report.contains("MCU Flash budget exceeded"));
    }

    #[test]
    fn ipc_message_type_display() {
        assert_eq!(format!("{}", IpcMessageType::SensorData), "SensorData");
        assert_eq!(
            format!("{}", IpcMessageType::InferenceResult),
            "InferenceResult"
        );
        assert_eq!(
            format!("{}", IpcMessageType::ActuatorCommand),
            "ActuatorCommand"
        );
        assert_eq!(format!("{}", IpcMessageType::Heartbeat), "Heartbeat");
        assert_eq!(format!("{}", IpcMessageType::Error), "Error");
    }

    #[test]
    fn ipc_channel_type_display() {
        assert_eq!(format!("{}", IpcChannelType::SharedMemory), "SharedMemory");
        assert_eq!(format!("{}", IpcChannelType::Uart), "UART");
    }
}
