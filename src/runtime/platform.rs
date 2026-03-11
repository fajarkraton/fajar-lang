//! Cross-platform support for Fajar Lang.
//!
//! Platform detection, path normalization, QEMU target configuration,
//! endianness handling, and compatibility utilities for building and
//! testing Fajar Lang programs across multiple platforms.

use std::fmt;

use thiserror::Error;

// ═══════════════════════════════════════════════════════════════════════
// Error types
// ═══════════════════════════════════════════════════════════════════════

/// Errors from cross-platform operations.
#[derive(Debug, Clone, Error, PartialEq)]
pub enum PlatformError {
    /// The requested platform is not supported.
    #[error("unsupported platform: {0}")]
    Unsupported(String),

    /// Path normalization failed.
    #[error("invalid path for {platform}: {reason}")]
    InvalidPath {
        /// Target platform.
        platform: String,
        /// Description of the problem.
        reason: String,
    },

    /// QEMU target is not available.
    #[error("QEMU target not available: {0}")]
    QemuUnavailable(String),
}

// ═══════════════════════════════════════════════════════════════════════
// Platform detection
// ═══════════════════════════════════════════════════════════════════════

/// Host or target platform.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Platform {
    /// Linux (any architecture).
    Linux,
    /// macOS (any architecture).
    MacOS,
    /// Windows.
    Windows,
    /// Bare-metal embedded target (no OS).
    Embedded,
}

impl fmt::Display for Platform {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Linux => write!(f, "linux"),
            Self::MacOS => write!(f, "macos"),
            Self::Windows => write!(f, "windows"),
            Self::Embedded => write!(f, "embedded"),
        }
    }
}

/// Detects the host platform at runtime.
///
/// Returns `Platform::Embedded` only if explicitly configured;
/// otherwise detects Linux, macOS, or Windows.
pub fn detect_platform() -> Platform {
    if cfg!(target_os = "linux") {
        Platform::Linux
    } else if cfg!(target_os = "macos") {
        Platform::MacOS
    } else if cfg!(target_os = "windows") {
        Platform::Windows
    } else {
        Platform::Embedded
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Path normalization
// ═══════════════════════════════════════════════════════════════════════

/// Normalizes a file path for the given platform.
///
/// Converts separators: `/` on Linux/macOS, `\` on Windows.
/// Embedded paths are left as-is.
pub fn normalize_path(path: &str, platform: Platform) -> String {
    match platform {
        Platform::Windows => path.replace('/', "\\"),
        Platform::Linux | Platform::MacOS => path.replace('\\', "/"),
        Platform::Embedded => path.to_string(),
    }
}

/// Returns `true` if the path is absolute for the given platform.
pub fn is_absolute_path(path: &str, platform: Platform) -> bool {
    match platform {
        Platform::Linux | Platform::MacOS | Platform::Embedded => path.starts_with('/'),
        Platform::Windows => {
            // e.g., "C:\..." or "\\..."
            (path.len() >= 3
                && path.as_bytes()[1] == b':'
                && (path.as_bytes()[2] == b'\\' || path.as_bytes()[2] == b'/'))
                || path.starts_with("\\\\")
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// QEMU targets
// ═══════════════════════════════════════════════════════════════════════

/// QEMU target for cross-platform testing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QemuTarget {
    /// ARM Cortex-M (microcontroller).
    CortexM,
    /// AArch64 (64-bit ARM).
    AArch64,
    /// RISC-V 64-bit.
    RiscV,
}

impl fmt::Display for QemuTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CortexM => write!(f, "cortex-m"),
            Self::AArch64 => write!(f, "aarch64"),
            Self::RiscV => write!(f, "riscv64"),
        }
    }
}

impl QemuTarget {
    /// Returns the QEMU system binary name for this target.
    pub fn qemu_binary(&self) -> &'static str {
        match self {
            Self::CortexM => "qemu-system-arm",
            Self::AArch64 => "qemu-system-aarch64",
            Self::RiscV => "qemu-system-riscv64",
        }
    }

    /// Returns the QEMU machine type for this target.
    pub fn machine_type(&self) -> &'static str {
        match self {
            Self::CortexM => "lm3s6965evb",
            Self::AArch64 => "virt",
            Self::RiscV => "virt",
        }
    }

    /// Returns the QEMU CPU model for this target.
    pub fn cpu_model(&self) -> &'static str {
        match self {
            Self::CortexM => "cortex-m3",
            Self::AArch64 => "cortex-a53",
            Self::RiscV => "rv64",
        }
    }
}

/// Constructs a QEMU command line for running a binary.
///
/// Returns the command as a single string (not yet split into args).
pub fn qemu_command(target: QemuTarget, binary: &str) -> String {
    format!(
        "{} -machine {} -cpu {} -nographic -kernel {}",
        target.qemu_binary(),
        target.machine_type(),
        target.cpu_model(),
        binary,
    )
}

// ═══════════════════════════════════════════════════════════════════════
// PlatformConfig — feature availability
// ═══════════════════════════════════════════════════════════════════════

/// Available features for a platform.
#[derive(Debug, Clone, PartialEq)]
pub struct PlatformConfig {
    /// Platform this configuration describes.
    pub platform: Platform,
    /// Whether the platform has a filesystem.
    pub has_filesystem: bool,
    /// Whether the platform has networking.
    pub has_networking: bool,
    /// Whether the platform has heap allocation.
    pub has_heap: bool,
    /// Whether the platform supports threads.
    pub has_threads: bool,
    /// Whether the platform supports floating-point.
    pub has_fpu: bool,
    /// Pointer width.
    pub pointer_width: PointerWidth,
    /// Default byte order.
    pub endian: EndianOrder,
}

impl PlatformConfig {
    /// Returns the default configuration for a platform.
    pub fn for_platform(platform: Platform) -> Self {
        match platform {
            Platform::Linux | Platform::MacOS => Self {
                platform,
                has_filesystem: true,
                has_networking: true,
                has_heap: true,
                has_threads: true,
                has_fpu: true,
                pointer_width: PointerWidth::Bits64,
                endian: EndianOrder::Little,
            },
            Platform::Windows => Self {
                platform,
                has_filesystem: true,
                has_networking: true,
                has_heap: true,
                has_threads: true,
                has_fpu: true,
                pointer_width: PointerWidth::Bits64,
                endian: EndianOrder::Little,
            },
            Platform::Embedded => Self {
                platform,
                has_filesystem: false,
                has_networking: false,
                has_heap: false,
                has_threads: false,
                has_fpu: false,
                pointer_width: PointerWidth::Bits32,
                endian: EndianOrder::Little,
            },
        }
    }

    /// Returns `true` if the platform supports the full standard
    /// library (filesystem + heap + threads).
    pub fn has_full_std(&self) -> bool {
        self.has_filesystem && self.has_heap && self.has_threads
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Endianness
// ═══════════════════════════════════════════════════════════════════════

/// Byte order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EndianOrder {
    /// Least-significant byte first (x86, ARM default, RISC-V default).
    Little,
    /// Most-significant byte first (network order, some ARM configs).
    Big,
}

impl fmt::Display for EndianOrder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Little => write!(f, "little-endian"),
            Self::Big => write!(f, "big-endian"),
        }
    }
}

/// Converts a byte slice between endianness orders.
///
/// If `from` and `to` are the same, returns the data unchanged.
/// Otherwise reverses the bytes (assuming a single scalar value).
pub fn convert_endianness(data: &[u8], from: EndianOrder, to: EndianOrder) -> Vec<u8> {
    if from == to || data.is_empty() {
        data.to_vec()
    } else {
        data.iter().rev().copied().collect()
    }
}

/// Swaps the byte order of a 4-byte (u32-sized) slice in-place.
///
/// Returns the swapped bytes as a new vector.
pub fn swap_bytes_u32(data: &[u8; 4]) -> [u8; 4] {
    [data[3], data[2], data[1], data[0]]
}

/// Swaps the byte order of an 8-byte (u64-sized) slice.
pub fn swap_bytes_u64(data: &[u8; 8]) -> [u8; 8] {
    [
        data[7], data[6], data[5], data[4], data[3], data[2], data[1], data[0],
    ]
}

// ═══════════════════════════════════════════════════════════════════════
// Pointer width
// ═══════════════════════════════════════════════════════════════════════

/// Pointer width of the target platform.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PointerWidth {
    /// 32-bit pointers (embedded Cortex-M, RISC-V RV32).
    Bits32,
    /// 64-bit pointers (desktop, AArch64, RV64).
    Bits64,
}

impl PointerWidth {
    /// Returns the width in bytes.
    pub fn bytes(&self) -> usize {
        match self {
            Self::Bits32 => 4,
            Self::Bits64 => 8,
        }
    }

    /// Returns the width in bits.
    pub fn bits(&self) -> usize {
        match self {
            Self::Bits32 => 32,
            Self::Bits64 => 64,
        }
    }
}

impl fmt::Display for PointerWidth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}-bit", self.bits())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Unicode path support
// ═══════════════════════════════════════════════════════════════════════

/// Returns `true` if the platform natively supports Unicode file paths.
///
/// All modern desktop OSes support Unicode; embedded targets may not.
pub fn supports_unicode_paths(platform: Platform) -> bool {
    match platform {
        Platform::Linux | Platform::MacOS | Platform::Windows => true,
        Platform::Embedded => false,
    }
}

/// Checks whether a path contains only ASCII characters.
pub fn is_ascii_path(path: &str) -> bool {
    path.is_ascii()
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn s27_1_detect_platform() {
        let p = detect_platform();
        // We are running on a desktop OS.
        assert!(p == Platform::Linux || p == Platform::MacOS || p == Platform::Windows);
    }

    #[test]
    fn s27_2_platform_display() {
        assert_eq!(Platform::Linux.to_string(), "linux");
        assert_eq!(Platform::Windows.to_string(), "windows");
        assert_eq!(Platform::Embedded.to_string(), "embedded");
    }

    #[test]
    fn s27_3_normalize_path_linux() {
        let path = "src\\parser\\mod.rs";
        let norm = normalize_path(path, Platform::Linux);
        assert_eq!(norm, "src/parser/mod.rs");
    }

    #[test]
    fn s27_4_normalize_path_windows() {
        let path = "src/parser/mod.rs";
        let norm = normalize_path(path, Platform::Windows);
        assert_eq!(norm, "src\\parser\\mod.rs");
    }

    #[test]
    fn s27_5_absolute_path_detection() {
        assert!(is_absolute_path("/home/user", Platform::Linux));
        assert!(!is_absolute_path("src/main.rs", Platform::Linux));
        assert!(is_absolute_path("C:\\Users", Platform::Windows));
        assert!(is_absolute_path("\\\\server", Platform::Windows));
        assert!(!is_absolute_path("src\\main.rs", Platform::Windows));
    }

    #[test]
    fn s27_6_qemu_command_generation() {
        let cmd = qemu_command(QemuTarget::AArch64, "kernel.bin");
        assert!(cmd.contains("qemu-system-aarch64"));
        assert!(cmd.contains("-machine virt"));
        assert!(cmd.contains("-cpu cortex-a53"));
        assert!(cmd.contains("kernel.bin"));

        let cmd_rv = qemu_command(QemuTarget::RiscV, "fw.elf");
        assert!(cmd_rv.contains("qemu-system-riscv64"));
        assert!(cmd_rv.contains("fw.elf"));
    }

    #[test]
    fn s27_7_endianness_conversion() {
        let data = vec![0x01, 0x02, 0x03, 0x04];
        let swapped = convert_endianness(&data, EndianOrder::Little, EndianOrder::Big);
        assert_eq!(swapped, vec![0x04, 0x03, 0x02, 0x01]);

        // Same order -> no change.
        let same = convert_endianness(&data, EndianOrder::Little, EndianOrder::Little);
        assert_eq!(same, data);
    }

    #[test]
    fn s27_8_swap_bytes() {
        let data32: [u8; 4] = [0xAA, 0xBB, 0xCC, 0xDD];
        let swapped32 = swap_bytes_u32(&data32);
        assert_eq!(swapped32, [0xDD, 0xCC, 0xBB, 0xAA]);

        let data64: [u8; 8] = [1, 2, 3, 4, 5, 6, 7, 8];
        let swapped64 = swap_bytes_u64(&data64);
        assert_eq!(swapped64, [8, 7, 6, 5, 4, 3, 2, 1]);
    }

    #[test]
    fn s27_9_platform_config() {
        let linux_cfg = PlatformConfig::for_platform(Platform::Linux);
        assert!(linux_cfg.has_full_std());
        assert!(linux_cfg.has_fpu);
        assert_eq!(linux_cfg.pointer_width, PointerWidth::Bits64);

        let embedded_cfg = PlatformConfig::for_platform(Platform::Embedded);
        assert!(!embedded_cfg.has_full_std());
        assert!(!embedded_cfg.has_fpu);
        assert_eq!(embedded_cfg.pointer_width, PointerWidth::Bits32);
    }

    #[test]
    fn s27_10_unicode_path_support() {
        assert!(supports_unicode_paths(Platform::Linux));
        assert!(supports_unicode_paths(Platform::Windows));
        assert!(!supports_unicode_paths(Platform::Embedded));

        assert!(is_ascii_path("/home/user/file.fj"));
        assert!(!is_ascii_path("/home/user/dokumen/fajar.fj\u{00E9}"));
    }
}
