//! Cross-platform support and distribution infrastructure for Fajar Lang.
//!
//! Provides runtime platform detection, path normalization, line ending handling,
//! binary distribution management, installer generation, version information,
//! and platform-specific optimization hints.
//!
//! All platform-specific logic uses `cfg!()` runtime macros (not `#[cfg]` attributes)
//! so that every code path compiles on every host. This ensures the full module is
//! always type-checked regardless of the build target.

use std::fmt;

use thiserror::Error;

// ═══════════════════════════════════════════════════════════════════════
// Error types
// ═══════════════════════════════════════════════════════════════════════

/// Errors from cross-platform and distribution operations.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum CrossPlatformError {
    /// The requested platform is not supported for the operation.
    #[error("CP001: unsupported platform: {0}")]
    UnsupportedPlatform(String),

    /// Path conversion or normalization failed.
    #[error("CP002: invalid path: {reason}")]
    InvalidPath {
        /// Description of what went wrong.
        reason: String,
    },

    /// URI conversion failed.
    #[error("CP003: invalid URI: {reason}")]
    InvalidUri {
        /// Description of what went wrong.
        reason: String,
    },

    /// A build or distribution configuration error.
    #[error("CP004: distribution error: {reason}")]
    DistributionError {
        /// Description of the problem.
        reason: String,
    },

    /// Installer generation failed.
    #[error("CP005: installer generation error: {reason}")]
    InstallerError {
        /// Description of the problem.
        reason: String,
    },

    /// Checksum verification failed.
    #[error("CP006: checksum mismatch for '{artifact}': expected {expected}, got {actual}")]
    ChecksumMismatch {
        /// Name of the artifact.
        artifact: String,
        /// Expected checksum.
        expected: String,
        /// Actual checksum.
        actual: String,
    },
}

// ═══════════════════════════════════════════════════════════════════════
// 1. PlatformDetector — Runtime platform detection
// ═══════════════════════════════════════════════════════════════════════

/// Operating system classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Platform {
    /// Linux (any distribution).
    Linux,
    /// macOS (Apple Silicon or Intel).
    MacOS,
    /// Microsoft Windows.
    Windows,
    /// FreeBSD.
    FreeBSD,
    /// Unknown or unsupported operating system.
    Unknown,
}

impl fmt::Display for Platform {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Linux => write!(f, "linux"),
            Self::MacOS => write!(f, "macos"),
            Self::Windows => write!(f, "windows"),
            Self::FreeBSD => write!(f, "freebsd"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

/// CPU architecture classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Architecture {
    /// x86-64 / AMD64.
    X86_64,
    /// ARM 64-bit (AArch64).
    Aarch64,
    /// RISC-V 64-bit.
    Riscv64,
    /// WebAssembly 32-bit.
    Wasm32,
    /// Unknown or unsupported architecture.
    Unknown,
}

impl fmt::Display for Architecture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::X86_64 => write!(f, "x86_64"),
            Self::Aarch64 => write!(f, "aarch64"),
            Self::Riscv64 => write!(f, "riscv64"),
            Self::Wasm32 => write!(f, "wasm32"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

/// Byte order of the host platform.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Endianness {
    /// Least-significant byte first.
    Little,
    /// Most-significant byte first.
    Big,
}

impl fmt::Display for Endianness {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Little => write!(f, "little-endian"),
            Self::Big => write!(f, "big-endian"),
        }
    }
}

/// CPU feature flags detected at runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CpuFeatures {
    /// x86: SSE 4.2 string/comparison instructions.
    pub has_sse42: bool,
    /// x86: 256-bit AVX2 vector instructions.
    pub has_avx2: bool,
    /// x86: 512-bit AVX-512 vector instructions.
    pub has_avx512: bool,
    /// ARM: NEON SIMD instructions.
    pub has_neon: bool,
    /// ARM: Scalable Vector Extension.
    pub has_sve: bool,
}

impl CpuFeatures {
    /// Detects CPU features for the current host.
    ///
    /// Uses `cfg!` macros for architecture detection and conservative
    /// defaults. On x86_64 Linux, SSE4.2 is assumed present; AVX2/AVX-512
    /// require explicit detection (conservatively false here). On AArch64,
    /// NEON is assumed present.
    pub fn detect() -> Self {
        let is_x86_64 = cfg!(target_arch = "x86_64");
        let is_aarch64 = cfg!(target_arch = "aarch64");

        Self {
            has_sse42: is_x86_64,
            has_avx2: false,   // conservative — real detection needs CPUID
            has_avx512: false, // conservative — rare outside server CPUs
            has_neon: is_aarch64,
            has_sve: false, // conservative — SVE not yet widespread
        }
    }

    /// Returns the maximum SIMD width in bits for this CPU.
    pub fn max_simd_width_bits(&self) -> u32 {
        if self.has_avx512 {
            512
        } else if self.has_avx2 {
            256
        } else if self.has_sse42 || self.has_neon {
            128
        } else {
            64 // scalar fallback
        }
    }
}

impl fmt::Display for CpuFeatures {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut features = Vec::new();
        if self.has_sse42 {
            features.push("sse4.2");
        }
        if self.has_avx2 {
            features.push("avx2");
        }
        if self.has_avx512 {
            features.push("avx-512");
        }
        if self.has_neon {
            features.push("neon");
        }
        if self.has_sve {
            features.push("sve");
        }
        if features.is_empty() {
            write!(f, "none")
        } else {
            write!(f, "{}", features.join(", "))
        }
    }
}

/// Comprehensive information about the host platform.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlatformInfo {
    /// Operating system.
    pub os: Platform,
    /// CPU architecture.
    pub arch: Architecture,
    /// Virtual memory page size in bytes.
    pub page_size: usize,
    /// Number of logical CPU cores.
    pub cpu_count: usize,
    /// Detected CPU SIMD / vector features.
    pub cpu_features: CpuFeatures,
    /// Byte order of the host.
    pub endianness: Endianness,
}

impl PlatformInfo {
    /// Detects the current host platform at runtime.
    ///
    /// Uses `cfg!` macros and `std::thread::available_parallelism` for
    /// portable detection that compiles on every target.
    pub fn detect() -> Self {
        Self {
            os: detect_os(),
            arch: detect_arch(),
            page_size: detect_page_size(),
            cpu_count: detect_cpu_count(),
            cpu_features: CpuFeatures::detect(),
            endianness: detect_endianness(),
        }
    }

    /// Returns the Rust-style target triple string (e.g., `x86_64-unknown-linux-gnu`).
    pub fn target_triple(&self) -> String {
        let arch = match self.arch {
            Architecture::X86_64 => "x86_64",
            Architecture::Aarch64 => "aarch64",
            Architecture::Riscv64 => "riscv64gc",
            Architecture::Wasm32 => "wasm32",
            Architecture::Unknown => "unknown",
        };
        let os = match self.os {
            Platform::Linux => "unknown-linux-gnu",
            Platform::MacOS => "apple-darwin",
            Platform::Windows => "pc-windows-msvc",
            Platform::FreeBSD => "unknown-freebsd",
            Platform::Unknown => "unknown-unknown",
        };
        format!("{arch}-{os}")
    }
}

impl fmt::Display for PlatformInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {} ({}, page={}B, cpus={}, simd=[{}])",
            self.os, self.arch, self.endianness, self.page_size, self.cpu_count, self.cpu_features,
        )
    }
}

/// Detects the host operating system via `cfg!`.
fn detect_os() -> Platform {
    if cfg!(target_os = "linux") {
        Platform::Linux
    } else if cfg!(target_os = "macos") {
        Platform::MacOS
    } else if cfg!(target_os = "windows") {
        Platform::Windows
    } else if cfg!(target_os = "freebsd") {
        Platform::FreeBSD
    } else {
        Platform::Unknown
    }
}

/// Detects the host CPU architecture via `cfg!`.
fn detect_arch() -> Architecture {
    if cfg!(target_arch = "x86_64") {
        Architecture::X86_64
    } else if cfg!(target_arch = "aarch64") {
        Architecture::Aarch64
    } else if cfg!(target_arch = "riscv64") {
        Architecture::Riscv64
    } else if cfg!(target_arch = "wasm32") {
        Architecture::Wasm32
    } else {
        Architecture::Unknown
    }
}

/// Detects the host page size.
///
/// Returns 4096 as the default for most platforms.
fn detect_page_size() -> usize {
    // Most modern platforms use 4 KiB pages. macOS on Apple Silicon
    // uses 16 KiB, but cfg! cannot detect that at compile time, so
    // we use the common default. A real implementation would call
    // sysconf(_SC_PAGESIZE) on Unix.
    if cfg!(target_os = "macos") && cfg!(target_arch = "aarch64") {
        16384
    } else {
        4096
    }
}

/// Detects the number of logical CPUs.
fn detect_cpu_count() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
}

/// Detects host byte order via `cfg!`.
fn detect_endianness() -> Endianness {
    if cfg!(target_endian = "little") {
        Endianness::Little
    } else {
        Endianness::Big
    }
}

// ═══════════════════════════════════════════════════════════════════════
// 2. PathNormalizer — Cross-platform path handling
// ═══════════════════════════════════════════════════════════════════════

/// Cross-platform path normalization utilities.
///
/// All methods are pure functions operating on string paths. They do not
/// touch the filesystem and are safe to call on any platform.
pub struct PathNormalizer;

impl PathNormalizer {
    /// Normalizes a path to use the platform-appropriate separator.
    ///
    /// On Windows-like contexts, converts `/` to `\`.
    /// On Unix-like contexts, converts `\` to `/`.
    /// Also collapses multiple consecutive separators into one.
    pub fn normalize(path: &str) -> String {
        let use_backslash = cfg!(target_os = "windows");
        let normalized = if use_backslash {
            path.replace('/', "\\")
        } else {
            path.replace('\\', "/")
        };
        Self::collapse_separators(&normalized)
    }

    /// Converts a filesystem path to a `file://` URI.
    ///
    /// Handles Unix paths (`/home/user` -> `file:///home/user`) and
    /// Windows drive letters (`C:\Users` -> `file:///C:/Users`).
    pub fn to_uri(path: &str) -> Result<String, CrossPlatformError> {
        if path.is_empty() {
            return Err(CrossPlatformError::InvalidPath {
                reason: "empty path cannot be converted to URI".to_string(),
            });
        }

        let forward = path.replace('\\', "/");

        // Windows drive letter: C:/... -> file:///C:/...
        if forward.len() >= 2
            && forward.as_bytes()[0].is_ascii_alphabetic()
            && forward.as_bytes()[1] == b':'
        {
            return Ok(format!("file:///{}", Self::percent_encode_path(&forward)));
        }

        // Unix absolute path: /home/... -> file:///home/...
        if forward.starts_with('/') {
            return Ok(format!("file://{}", Self::percent_encode_path(&forward)));
        }

        Err(CrossPlatformError::InvalidPath {
            reason: format!("path '{}' is not absolute", path),
        })
    }

    /// Converts a `file://` URI back to a filesystem path.
    ///
    /// Strips the `file://` prefix and handles Windows drive letters.
    pub fn from_uri(uri: &str) -> Result<String, CrossPlatformError> {
        if !uri.starts_with("file://") {
            return Err(CrossPlatformError::InvalidUri {
                reason: format!("URI '{}' does not start with file://", uri),
            });
        }

        let after_scheme = &uri[7..]; // skip "file://"

        // file:///C:/... -> C:/...
        if after_scheme.len() >= 3
            && after_scheme.as_bytes()[0] == b'/'
            && after_scheme.as_bytes()[1].is_ascii_alphabetic()
            && after_scheme.as_bytes()[2] == b':'
        {
            let decoded = Self::percent_decode(&after_scheme[1..]);
            return Ok(Self::normalize_for_platform(&decoded));
        }

        // file:///home/... -> /home/...
        if after_scheme.starts_with('/') {
            let decoded = Self::percent_decode(after_scheme);
            return Ok(Self::normalize_for_platform(&decoded));
        }

        Err(CrossPlatformError::InvalidUri {
            reason: format!("URI '{}' has no valid path component", uri),
        })
    }

    /// Joins path components with the platform-appropriate separator.
    ///
    /// Removes trailing separators from all components except the last,
    /// and inserts a single separator between them.
    pub fn join_paths(components: &[&str]) -> String {
        if components.is_empty() {
            return String::new();
        }
        let sep = if cfg!(target_os = "windows") {
            '\\'
        } else {
            '/'
        };

        let mut result = String::new();
        for (i, component) in components.iter().enumerate() {
            let trimmed = if i == 0 {
                component.trim_end_matches(['/', '\\'])
            } else {
                component
                    .trim_start_matches(['/', '\\'])
                    .trim_end_matches(['/', '\\'])
            };
            if !trimmed.is_empty() {
                if !result.is_empty() && !result.ends_with('/') && !result.ends_with('\\') {
                    result.push(sep);
                }
                result.push_str(trimmed);
            }
        }
        result
    }

    /// Makes `to` relative to `from`.
    ///
    /// Both paths must be absolute. Returns a relative path from `from` to `to`.
    pub fn make_relative(from: &str, to: &str) -> Result<String, CrossPlatformError> {
        let from_norm = from.replace('\\', "/");
        let to_norm = to.replace('\\', "/");

        if !from_norm.starts_with('/') && !Self::has_drive_letter(&from_norm) {
            return Err(CrossPlatformError::InvalidPath {
                reason: format!("'from' path '{}' is not absolute", from),
            });
        }
        if !to_norm.starts_with('/') && !Self::has_drive_letter(&to_norm) {
            return Err(CrossPlatformError::InvalidPath {
                reason: format!("'to' path '{}' is not absolute", to),
            });
        }

        let from_parts: Vec<&str> = from_norm.split('/').filter(|s| !s.is_empty()).collect();
        let to_parts: Vec<&str> = to_norm.split('/').filter(|s| !s.is_empty()).collect();

        // Find the common prefix length.
        let common = from_parts
            .iter()
            .zip(to_parts.iter())
            .take_while(|(a, b)| a == b)
            .count();

        let up_count = from_parts.len() - common;
        let mut result_parts: Vec<&str> = vec![".."; up_count];
        for part in &to_parts[common..] {
            result_parts.push(part);
        }

        if result_parts.is_empty() {
            Ok(".".to_string())
        } else {
            Ok(result_parts.join("/"))
        }
    }

    /// Collapses multiple consecutive separators into one.
    fn collapse_separators(path: &str) -> String {
        let mut result = String::with_capacity(path.len());
        let mut prev_was_sep = false;
        for (i, ch) in path.chars().enumerate() {
            let is_sep = ch == '/' || ch == '\\';
            if is_sep && prev_was_sep && i > 1 {
                continue;
            }
            result.push(ch);
            prev_was_sep = is_sep;
        }
        result
    }

    /// Percent-encodes spaces and special characters in a path for URIs.
    fn percent_encode_path(path: &str) -> String {
        let mut encoded = String::with_capacity(path.len());
        for ch in path.chars() {
            match ch {
                ' ' => encoded.push_str("%20"),
                '#' => encoded.push_str("%23"),
                '?' => encoded.push_str("%3F"),
                _ => encoded.push(ch),
            }
        }
        encoded
    }

    /// Decodes percent-encoded characters in a URI path.
    fn percent_decode(s: &str) -> String {
        let mut result = String::with_capacity(s.len());
        let bytes = s.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'%' && i + 2 < bytes.len() {
                let hi = bytes[i + 1];
                let lo = bytes[i + 2];
                if let (Some(h), Some(l)) = (hex_val(hi), hex_val(lo)) {
                    result.push((h * 16 + l) as char);
                    i += 3;
                    continue;
                }
            }
            result.push(bytes[i] as char);
            i += 1;
        }
        result
    }

    /// Normalizes a decoded path to the current platform's separator.
    fn normalize_for_platform(path: &str) -> String {
        if cfg!(target_os = "windows") {
            path.replace('/', "\\")
        } else {
            path.to_string()
        }
    }

    /// Returns `true` if the path starts with a drive letter (e.g., `C:`).
    fn has_drive_letter(path: &str) -> bool {
        path.len() >= 2 && path.as_bytes()[0].is_ascii_alphabetic() && path.as_bytes()[1] == b':'
    }
}

/// Converts a hex ASCII character to its numeric value.
fn hex_val(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// 3. LineEndingHandler — Line ending normalization
// ═══════════════════════════════════════════════════════════════════════

/// Line ending style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LineEnding {
    /// Unix-style: `\n` (LF).
    Lf,
    /// Windows-style: `\r\n` (CRLF).
    CrLf,
    /// Classic Mac-style: `\r` (CR).
    Cr,
}

impl fmt::Display for LineEnding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Lf => write!(f, "LF"),
            Self::CrLf => write!(f, "CRLF"),
            Self::Cr => write!(f, "CR"),
        }
    }
}

impl LineEnding {
    /// Returns the byte sequence for this line ending.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Lf => "\n",
            Self::CrLf => "\r\n",
            Self::Cr => "\r",
        }
    }
}

/// Line ending detection and conversion utilities.
///
/// Used by the formatter, LSP, and file I/O to ensure consistent
/// line endings across platforms.
pub struct LineEndingHandler;

impl LineEndingHandler {
    /// Detects the dominant line ending style in a string.
    ///
    /// Counts occurrences of each style and returns the most frequent.
    /// Returns `LineEnding::Lf` for empty strings or strings with no
    /// line endings.
    pub fn detect(content: &str) -> LineEnding {
        let mut lf_count: usize = 0;
        let mut crlf_count: usize = 0;
        let mut cr_count: usize = 0;

        let bytes = content.as_bytes();
        let len = bytes.len();
        let mut i = 0;
        while i < len {
            if bytes[i] == b'\r' {
                if i + 1 < len && bytes[i + 1] == b'\n' {
                    crlf_count += 1;
                    i += 2;
                } else {
                    cr_count += 1;
                    i += 1;
                }
            } else if bytes[i] == b'\n' {
                lf_count += 1;
                i += 1;
            } else {
                i += 1;
            }
        }

        if crlf_count >= lf_count && crlf_count >= cr_count && crlf_count > 0 {
            LineEnding::CrLf
        } else if cr_count > lf_count && cr_count > 0 {
            LineEnding::Cr
        } else {
            LineEnding::Lf
        }
    }

    /// Normalizes all line endings in `content` to LF (`\n`).
    ///
    /// Handles CRLF, CR, and mixed line endings.
    pub fn normalize_to_lf(content: &str) -> String {
        Self::convert(content, LineEnding::Lf)
    }

    /// Converts all line endings in `content` to the target style.
    ///
    /// First normalizes everything to LF, then converts to the target.
    pub fn convert(content: &str, target: LineEnding) -> String {
        // Step 1: normalize everything to LF.
        // Replace CRLF first (before standalone CR).
        let lf_only = content.replace("\r\n", "\n").replace('\r', "\n");

        // Step 2: convert from LF to target.
        match target {
            LineEnding::Lf => lf_only,
            LineEnding::CrLf => lf_only.replace('\n', "\r\n"),
            LineEnding::Cr => lf_only.replace('\n', "\r"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// 4. BinaryDistributor — Release binary management
// ═══════════════════════════════════════════════════════════════════════

/// A build target for distribution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    /// Target operating system.
    pub os: Platform,
    /// Target CPU architecture.
    pub arch: Architecture,
    /// Additional feature flags (e.g., "native", "gpu", "cuda").
    pub features: Vec<String>,
}

impl Target {
    /// Creates a new build target.
    pub fn new(os: Platform, arch: Architecture) -> Self {
        Self {
            os,
            arch,
            features: Vec::new(),
        }
    }

    /// Adds a feature flag to this target.
    pub fn with_feature(mut self, feature: impl Into<String>) -> Self {
        self.features.push(feature.into());
        self
    }

    /// Returns the Rust target triple for this target.
    pub fn triple(&self) -> String {
        let arch = match self.arch {
            Architecture::X86_64 => "x86_64",
            Architecture::Aarch64 => "aarch64",
            Architecture::Riscv64 => "riscv64gc",
            Architecture::Wasm32 => "wasm32",
            Architecture::Unknown => "unknown",
        };
        let os = match self.os {
            Platform::Linux => "unknown-linux-gnu",
            Platform::MacOS => "apple-darwin",
            Platform::Windows => "pc-windows-msvc",
            Platform::FreeBSD => "unknown-freebsd",
            Platform::Unknown => "unknown-unknown",
        };
        format!("{arch}-{os}")
    }

    /// Returns the binary file name for this target (with extension).
    pub fn binary_name(&self, base: &str) -> String {
        if self.os == Platform::Windows {
            format!("{base}.exe")
        } else {
            base.to_string()
        }
    }
}

impl fmt::Display for Target {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.triple())
    }
}

/// A release artifact with metadata and integrity information.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseArtifact {
    /// File name of the artifact (e.g., `fj-1.0.0-x86_64-linux.tar.gz`).
    pub name: String,
    /// Build target this artifact was compiled for.
    pub target: Target,
    /// File size in bytes.
    pub size_bytes: u64,
    /// SHA-256 checksum hex string.
    pub sha256: String,
    /// Optional cryptographic signature (e.g., PGP detached sig).
    pub signature: Option<String>,
}

impl ReleaseArtifact {
    /// Creates a new release artifact.
    pub fn new(
        name: impl Into<String>,
        target: Target,
        size_bytes: u64,
        sha256: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            target,
            size_bytes,
            sha256: sha256.into(),
            signature: None,
        }
    }

    /// Attaches a signature to this artifact.
    pub fn with_signature(mut self, sig: impl Into<String>) -> Self {
        self.signature = Some(sig.into());
        self
    }

    /// Returns the human-readable file size.
    pub fn human_size(&self) -> String {
        let bytes = self.size_bytes;
        if bytes < 1024 {
            format!("{bytes} B")
        } else if bytes < 1024 * 1024 {
            format!("{:.1} KiB", bytes as f64 / 1024.0)
        } else if bytes < 1024 * 1024 * 1024 {
            format!("{:.1} MiB", bytes as f64 / (1024.0 * 1024.0))
        } else {
            format!("{:.2} GiB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
        }
    }
}

/// Panic strategy for the build.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PanicStrategy {
    /// Unwind the stack (default).
    Unwind,
    /// Abort the process immediately.
    Abort,
}

impl fmt::Display for PanicStrategy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unwind => write!(f, "unwind"),
            Self::Abort => write!(f, "abort"),
        }
    }
}

/// Build configuration for a release binary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildConfig {
    /// Cargo profile name (e.g., "release", "release-dist").
    pub profile: String,
    /// Link-time optimization setting.
    pub lto: LtoSetting,
    /// Whether to strip debug symbols from the binary.
    pub strip: bool,
    /// Number of codegen units (1 = max optimization, higher = faster compile).
    pub codegen_units: u32,
    /// Panic strategy (unwind or abort).
    pub panic_strategy: PanicStrategy,
    /// Optimization level (0-3, or "s"/"z" for size).
    pub opt_level: String,
}

/// Link-time optimization setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LtoSetting {
    /// No LTO.
    Off,
    /// Thin LTO (faster compile, good optimization).
    Thin,
    /// Fat LTO (slowest compile, best optimization).
    Fat,
}

impl fmt::Display for LtoSetting {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Off => write!(f, "off"),
            Self::Thin => write!(f, "thin"),
            Self::Fat => write!(f, "fat"),
        }
    }
}

/// Distribution-optimized build profile.
///
/// Produces the smallest, fastest release binary.
pub struct DistProfile;

impl DistProfile {
    /// Returns the `BuildConfig` for distribution builds.
    ///
    /// Settings: opt-level=3, LTO=fat, strip=true, codegen-units=1, panic=abort.
    pub fn config() -> BuildConfig {
        BuildConfig {
            profile: "release-dist".to_string(),
            lto: LtoSetting::Fat,
            strip: true,
            codegen_units: 1,
            panic_strategy: PanicStrategy::Abort,
            opt_level: "3".to_string(),
        }
    }

    /// Generates the `[profile.release-dist]` TOML section.
    pub fn to_cargo_toml_section() -> String {
        let cfg = Self::config();
        format!(
            "[profile.release-dist]\n\
             inherits = \"release\"\n\
             opt-level = {}\n\
             lto = \"{}\"\n\
             strip = {}\n\
             codegen-units = {}\n\
             panic = \"{}\"",
            cfg.opt_level, cfg.lto, cfg.strip, cfg.codegen_units, cfg.panic_strategy,
        )
    }
}

/// A manifest listing all release artifacts for a version.
#[derive(Debug, Clone, PartialEq)]
pub struct ArtifactManifest {
    /// Version string (e.g., "1.0.0").
    pub version: String,
    /// Release date in ISO 8601 format.
    pub date: String,
    /// All artifacts in this release.
    pub artifacts: Vec<ReleaseArtifact>,
}

impl ArtifactManifest {
    /// Creates a new empty manifest.
    pub fn new(version: impl Into<String>, date: impl Into<String>) -> Self {
        Self {
            version: version.into(),
            date: date.into(),
            artifacts: Vec::new(),
        }
    }

    /// Adds an artifact to the manifest.
    pub fn add_artifact(&mut self, artifact: ReleaseArtifact) {
        self.artifacts.push(artifact);
    }

    /// Returns the total size of all artifacts in bytes.
    pub fn total_size_bytes(&self) -> u64 {
        self.artifacts.iter().map(|a| a.size_bytes).sum()
    }

    /// Finds an artifact matching the given target.
    pub fn find_for_target(&self, os: Platform, arch: Architecture) -> Option<&ReleaseArtifact> {
        self.artifacts
            .iter()
            .find(|a| a.target.os == os && a.target.arch == arch)
    }
}

/// SHA-256 checksum file generator.
///
/// Generates checksum files in the standard `sha256sum` format:
/// `<hex-hash>  <filename>`.
pub struct ChecksumGenerator;

impl ChecksumGenerator {
    /// Generates a checksum file from a manifest.
    ///
    /// Each line is `<sha256>  <filename>`.
    pub fn generate(manifest: &ArtifactManifest) -> String {
        let mut lines: Vec<String> = manifest
            .artifacts
            .iter()
            .map(|a| format!("{}  {}", a.sha256, a.name))
            .collect();
        lines.sort();
        lines.join("\n")
    }

    /// Verifies that a given hash matches the expected hash for an artifact.
    pub fn verify(
        artifact_name: &str,
        expected_sha256: &str,
        actual_sha256: &str,
    ) -> Result<(), CrossPlatformError> {
        if expected_sha256 == actual_sha256 {
            Ok(())
        } else {
            Err(CrossPlatformError::ChecksumMismatch {
                artifact: artifact_name.to_string(),
                expected: expected_sha256.to_string(),
                actual: actual_sha256.to_string(),
            })
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// 5. InstallerGenerator — Installation script generation
// ═══════════════════════════════════════════════════════════════════════

/// Configuration for installer scripts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallerConfig {
    /// Directory where the `fj` binary will be installed.
    pub install_dir: String,
    /// Whether to add the install directory to the user's PATH.
    pub add_to_path: bool,
    /// Whether to create symlinks (e.g., `/usr/local/bin/fj`).
    pub create_symlinks: bool,
    /// Version to install.
    pub version: String,
    /// Base URL for downloading release artifacts.
    pub download_base_url: String,
}

impl Default for InstallerConfig {
    fn default() -> Self {
        Self {
            install_dir: "$HOME/.fj/bin".to_string(),
            add_to_path: true,
            create_symlinks: true,
            version: "latest".to_string(),
            download_base_url: "https://github.com/fajar-lang/fj/releases/download".to_string(),
        }
    }
}

/// Generates Unix shell installer scripts (`install.sh`).
pub struct ShellInstaller;

impl ShellInstaller {
    /// Generates a POSIX-compatible `install.sh` script.
    ///
    /// The script detects OS/architecture, downloads the appropriate
    /// binary, installs it, and optionally adds it to PATH.
    pub fn generate(config: &InstallerConfig) -> String {
        let mut script = String::new();
        script.push_str("#!/bin/sh\n");
        script.push_str("# Fajar Lang installer — https://fajarlang.org\n");
        script.push_str("# Generated by fj crossplatform module\n");
        script.push_str("set -eu\n\n");

        Self::append_detect_functions(&mut script);
        Self::append_main_function(&mut script, config);

        script
    }

    /// Appends OS and architecture detection shell functions.
    fn append_detect_functions(script: &mut String) {
        script.push_str("detect_os() {\n");
        script.push_str("  case \"$(uname -s)\" in\n");
        script.push_str("    Linux*)  echo \"linux\" ;;\n");
        script.push_str("    Darwin*) echo \"macos\" ;;\n");
        script.push_str("    FreeBSD*) echo \"freebsd\" ;;\n");
        script.push_str("    *)       echo \"unknown\" ;;\n");
        script.push_str("  esac\n");
        script.push_str("}\n\n");

        script.push_str("detect_arch() {\n");
        script.push_str("  case \"$(uname -m)\" in\n");
        script.push_str("    x86_64|amd64)  echo \"x86_64\" ;;\n");
        script.push_str("    aarch64|arm64) echo \"aarch64\" ;;\n");
        script.push_str("    riscv64)       echo \"riscv64\" ;;\n");
        script.push_str("    *)             echo \"unknown\" ;;\n");
        script.push_str("  esac\n");
        script.push_str("}\n\n");
    }

    /// Appends the main() install function with download and PATH setup.
    fn append_main_function(script: &mut String, config: &InstallerConfig) {
        script.push_str("main() {\n");
        script.push_str("  OS=$(detect_os)\n");
        script.push_str("  ARCH=$(detect_arch)\n");
        script.push_str(&format!("  VERSION=\"{}\"\n", config.version));
        script.push_str(&format!("  INSTALL_DIR=\"{}\"\n", config.install_dir));
        script.push_str(&format!("  BASE_URL=\"{}\"\n\n", config.download_base_url));

        Self::append_download_logic(script);

        if config.add_to_path {
            Self::append_path_setup(script, &config.install_dir);
        }

        script.push_str("\n  echo \"Fajar Lang v${VERSION} installed to ${INSTALL_DIR}/fj\"\n");
        script.push_str("  echo \"Run 'fj --version' to verify.\"\n");
        script.push_str("}\n\nmain\n");
    }

    /// Appends platform validation, download, and extraction logic.
    fn append_download_logic(script: &mut String) {
        script.push_str("  if [ \"$OS\" = \"unknown\" ] || [ \"$ARCH\" = \"unknown\" ]; then\n");
        script.push_str("    echo \"Error: unsupported platform: $OS-$ARCH\" >&2\n");
        script.push_str("    exit 1\n");
        script.push_str("  fi\n\n");

        script.push_str("  ARCHIVE=\"fj-${VERSION}-${ARCH}-${OS}.tar.gz\"\n");
        script.push_str("  URL=\"${BASE_URL}/v${VERSION}/${ARCHIVE}\"\n\n");

        script.push_str("  echo \"Installing Fajar Lang v${VERSION} for ${OS}/${ARCH}...\"\n");
        script.push_str("  mkdir -p \"$INSTALL_DIR\"\n\n");

        script.push_str("  if command -v curl >/dev/null 2>&1; then\n");
        script.push_str("    curl -fsSL \"$URL\" | tar xz -C \"$INSTALL_DIR\"\n");
        script.push_str("  elif command -v wget >/dev/null 2>&1; then\n");
        script.push_str("    wget -qO- \"$URL\" | tar xz -C \"$INSTALL_DIR\"\n");
        script.push_str("  else\n");
        script.push_str("    echo \"Error: curl or wget required\" >&2\n");
        script.push_str("    exit 1\n");
        script.push_str("  fi\n\n");

        script.push_str("  chmod +x \"$INSTALL_DIR/fj\"\n");
    }

    /// Appends shell rc PATH modification logic.
    fn append_path_setup(script: &mut String, install_dir: &str) {
        script.push_str("\n  # Add to PATH\n");
        script.push_str("  SHELL_RC=\"\"\n");
        script.push_str("  case \"$SHELL\" in\n");
        script.push_str("    */zsh)  SHELL_RC=\"$HOME/.zshrc\" ;;\n");
        script.push_str("    */bash) SHELL_RC=\"$HOME/.bashrc\" ;;\n");
        script.push_str("    *)      SHELL_RC=\"$HOME/.profile\" ;;\n");
        script.push_str("  esac\n");
        script.push_str("  if [ -n \"$SHELL_RC\" ]; then\n");
        script.push_str(&format!(
            "    if ! grep -q '{}' \"$SHELL_RC\" 2>/dev/null; then\n",
            install_dir
        ));
        script.push_str(&format!(
            "      echo 'export PATH=\"{}:$PATH\"' >> \"$SHELL_RC\"\n",
            install_dir
        ));
        script.push_str("    fi\n");
        script.push_str("  fi\n");
    }
}

/// Generates PowerShell installer scripts (`install.ps1`).
pub struct PowerShellInstaller;

impl PowerShellInstaller {
    /// Generates an `install.ps1` script for Windows.
    pub fn generate(config: &InstallerConfig) -> String {
        let mut s = String::new();
        s.push_str("# Fajar Lang installer for Windows\n");
        s.push_str("# Generated by fj crossplatform module\n");
        s.push_str("$ErrorActionPreference = 'Stop'\n\n");

        let install_dir = config.install_dir.replace("$HOME", "$env:USERPROFILE");
        s.push_str(&format!("$Version = '{}'\n", config.version));
        s.push_str(&format!("$InstallDir = '{}'\n", install_dir));
        s.push_str(&format!("$BaseUrl = '{}'\n\n", config.download_base_url));

        Self::append_download_section(&mut s);

        if config.add_to_path {
            Self::append_path_section(&mut s);
        }

        s.push_str("Write-Host \"Fajar Lang v${Version} installed to ${InstallDir}\\fj.exe\"\n");
        s.push_str("Write-Host \"Run 'fj --version' to verify.\"\n");
        s
    }

    /// Appends arch detection, download, and extraction logic.
    fn append_download_section(s: &mut String) {
        s.push_str(
            "$Arch = if ([Environment]::Is64BitOperatingSystem) { 'x86_64' } else { 'unknown' }\n",
        );
        s.push_str("if ($Arch -eq 'unknown') {\n");
        s.push_str("    Write-Error 'Unsupported architecture'\n");
        s.push_str("    exit 1\n");
        s.push_str("}\n\n");
        s.push_str("$Archive = \"fj-${Version}-${Arch}-windows.zip\"\n");
        s.push_str("$Url = \"${BaseUrl}/v${Version}/${Archive}\"\n");
        s.push_str("$TempFile = Join-Path $env:TEMP $Archive\n\n");
        s.push_str("Write-Host \"Installing Fajar Lang v${Version} for Windows/${Arch}...\"\n\n");
        s.push_str("if (!(Test-Path $InstallDir)) {\n");
        s.push_str("    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null\n");
        s.push_str("}\n\n");
        s.push_str("Invoke-WebRequest -Uri $Url -OutFile $TempFile\n");
        s.push_str("Expand-Archive -Path $TempFile -DestinationPath $InstallDir -Force\n");
        s.push_str("Remove-Item $TempFile\n\n");
    }

    /// Appends user PATH environment variable modification.
    fn append_path_section(s: &mut String) {
        s.push_str("# Add to user PATH\n");
        s.push_str("$CurrentPath = [Environment]::GetEnvironmentVariable('Path', 'User')\n");
        s.push_str("if ($CurrentPath -notlike \"*$InstallDir*\") {\n");
        s.push_str(
            "    [Environment]::SetEnvironmentVariable('Path', \"${CurrentPath};${InstallDir}\", 'User')\n",
        );
        s.push_str("}\n\n");
    }
}

/// Generates a Homebrew formula (Ruby) for macOS/Linux distribution.
pub struct HomebrewFormula;

impl HomebrewFormula {
    /// Generates a Homebrew formula Ruby file.
    pub fn generate(
        version: &str,
        sha256_macos_arm: &str,
        sha256_macos_x86: &str,
        sha256_linux_x86: &str,
        download_base: &str,
    ) -> String {
        let mut formula = String::new();
        formula.push_str("class Fj < Formula\n");
        formula.push_str("  desc \"Fajar Lang — Systems programming language for OS and AI/ML\"\n");
        formula.push_str("  homepage \"https://fajarlang.org\"\n");
        formula.push_str(&format!("  version \"{version}\"\n"));
        formula.push_str("  license \"MIT\"\n\n");

        formula.push_str("  on_macos do\n");
        formula.push_str("    if Hardware::CPU.arm?\n");
        formula.push_str(&format!(
            "      url \"{download_base}/v{version}/fj-{version}-aarch64-macos.tar.gz\"\n"
        ));
        formula.push_str(&format!("      sha256 \"{sha256_macos_arm}\"\n"));
        formula.push_str("    else\n");
        formula.push_str(&format!(
            "      url \"{download_base}/v{version}/fj-{version}-x86_64-macos.tar.gz\"\n"
        ));
        formula.push_str(&format!("      sha256 \"{sha256_macos_x86}\"\n"));
        formula.push_str("    end\n");
        formula.push_str("  end\n\n");

        formula.push_str("  on_linux do\n");
        formula.push_str(&format!(
            "    url \"{download_base}/v{version}/fj-{version}-x86_64-linux.tar.gz\"\n"
        ));
        formula.push_str(&format!("    sha256 \"{sha256_linux_x86}\"\n"));
        formula.push_str("  end\n\n");

        formula.push_str("  def install\n");
        formula.push_str("    bin.install \"fj\"\n");
        formula.push_str("  end\n\n");

        formula.push_str("  test do\n");
        formula.push_str("    system \"#{bin}/fj\", \"--version\"\n");
        formula.push_str("  end\n");
        formula.push_str("end\n");

        formula
    }
}

/// Generates Debian package control file.
pub struct DebianPackage;

impl DebianPackage {
    /// Generates a `debian/control` file for `.deb` packaging.
    pub fn generate_control(version: &str, arch: &str) -> String {
        let deb_arch = match arch {
            "x86_64" => "amd64",
            "aarch64" => "arm64",
            "riscv64" => "riscv64",
            other => other,
        };

        let mut control = String::new();
        control.push_str("Package: fj\n");
        control.push_str(&format!("Version: {version}\n"));
        control.push_str(&format!("Architecture: {deb_arch}\n"));
        control.push_str("Maintainer: Fajar <fajar@fajarlang.org>\n");
        control.push_str("Section: devel\n");
        control.push_str("Priority: optional\n");
        control
            .push_str("Description: Fajar Lang — Systems programming language for OS and AI/ML\n");
        control
            .push_str(" A statically-typed systems programming language designed for embedded\n");
        control.push_str(" machine learning and operating system integration, featuring native\n");
        control.push_str(" tensor types, context-based safety annotations, and Rust-inspired\n");
        control.push_str(" ownership semantics.\n");
        control.push_str("Homepage: https://fajarlang.org\n");

        control
    }
}

/// Generates shell completion scripts.
pub struct CompletionGenerator;

/// Shell type for completion script generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Shell {
    /// Bash shell.
    Bash,
    /// Zsh shell.
    Zsh,
    /// Fish shell.
    Fish,
    /// PowerShell.
    PowerShell,
}

impl fmt::Display for Shell {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bash => write!(f, "bash"),
            Self::Zsh => write!(f, "zsh"),
            Self::Fish => write!(f, "fish"),
            Self::PowerShell => write!(f, "powershell"),
        }
    }
}

impl CompletionGenerator {
    /// Generates a completion script for the given shell.
    pub fn generate(shell: Shell) -> String {
        match shell {
            Shell::Bash => Self::generate_bash(),
            Shell::Zsh => Self::generate_zsh(),
            Shell::Fish => Self::generate_fish(),
            Shell::PowerShell => Self::generate_powershell(),
        }
    }

    /// Generates bash completions.
    fn generate_bash() -> String {
        let mut s = String::new();
        s.push_str("# Fajar Lang bash completions\n");
        s.push_str("_fj() {\n");
        s.push_str("  local cur prev commands\n");
        s.push_str("  COMPREPLY=()\n");
        s.push_str("  cur=\"${COMP_WORDS[COMP_CWORD]}\"\n");
        s.push_str("  prev=\"${COMP_WORDS[COMP_CWORD-1]}\"\n");
        s.push_str("  commands=\"run repl check build fmt lsp new dump-tokens dump-ast\"\n\n");
        s.push_str("  if [ \"$COMP_CWORD\" -eq 1 ]; then\n");
        s.push_str("    COMPREPLY=( $(compgen -W \"$commands\" -- \"$cur\") )\n");
        s.push_str("  elif [ \"$prev\" = \"run\" ] || [ \"$prev\" = \"check\" ] || [ \"$prev\" = \"fmt\" ]; then\n");
        s.push_str("    COMPREPLY=( $(compgen -f -X '!*.fj' -- \"$cur\") )\n");
        s.push_str("  fi\n");
        s.push_str("}\n");
        s.push_str("complete -F _fj fj\n");
        s
    }

    /// Generates zsh completions.
    fn generate_zsh() -> String {
        let mut s = String::new();
        s.push_str("#compdef fj\n");
        s.push_str("# Fajar Lang zsh completions\n\n");
        s.push_str("_fj() {\n");
        s.push_str("  local -a commands\n");
        s.push_str("  commands=(\n");
        s.push_str("    'run:Execute a Fajar Lang program'\n");
        s.push_str("    'repl:Start interactive REPL'\n");
        s.push_str("    'check:Type-check without execution'\n");
        s.push_str("    'build:Build from fj.toml'\n");
        s.push_str("    'fmt:Format source files'\n");
        s.push_str("    'lsp:Start LSP server'\n");
        s.push_str("    'new:Create a new project'\n");
        s.push_str("    'dump-tokens:Show lexer output'\n");
        s.push_str("    'dump-ast:Show parser output'\n");
        s.push_str("  )\n\n");
        s.push_str("  _arguments \\\n");
        s.push_str("    '1:command:->cmds' \\\n");
        s.push_str("    '*:file:_files -g \"*.fj\"'\n\n");
        s.push_str("  case $state in\n");
        s.push_str("    cmds) _describe 'command' commands ;;\n");
        s.push_str("  esac\n");
        s.push_str("}\n\n");
        s.push_str("_fj\n");
        s
    }

    /// Generates fish completions.
    fn generate_fish() -> String {
        let mut s = String::new();
        s.push_str("# Fajar Lang fish completions\n\n");
        s.push_str(
            "complete -c fj -n '__fish_use_subcommand' -a run -d 'Execute a Fajar Lang program'\n",
        );
        s.push_str(
            "complete -c fj -n '__fish_use_subcommand' -a repl -d 'Start interactive REPL'\n",
        );
        s.push_str("complete -c fj -n '__fish_use_subcommand' -a check -d 'Type-check without execution'\n");
        s.push_str("complete -c fj -n '__fish_use_subcommand' -a build -d 'Build from fj.toml'\n");
        s.push_str("complete -c fj -n '__fish_use_subcommand' -a fmt -d 'Format source files'\n");
        s.push_str("complete -c fj -n '__fish_use_subcommand' -a lsp -d 'Start LSP server'\n");
        s.push_str("complete -c fj -n '__fish_use_subcommand' -a new -d 'Create a new project'\n");
        s.push_str(
            "complete -c fj -n '__fish_use_subcommand' -a dump-tokens -d 'Show lexer output'\n",
        );
        s.push_str(
            "complete -c fj -n '__fish_use_subcommand' -a dump-ast -d 'Show parser output'\n",
        );
        s.push_str(
            "complete -c fj -n '__fish_seen_subcommand_from run check fmt' -F -r -a '*.fj'\n",
        );
        s
    }

    /// Generates PowerShell completions.
    fn generate_powershell() -> String {
        let mut s = String::new();
        s.push_str("# Fajar Lang PowerShell completions\n\n");
        s.push_str("Register-ArgumentCompleter -CommandName fj -ScriptBlock {\n");
        s.push_str("    param($wordToComplete, $commandAst, $cursorPosition)\n\n");
        s.push_str("    $commands = @(\n");
        s.push_str("        [CompletionResult]::new('run', 'run', 'ParameterValue', 'Execute a Fajar Lang program')\n");
        s.push_str("        [CompletionResult]::new('repl', 'repl', 'ParameterValue', 'Start interactive REPL')\n");
        s.push_str("        [CompletionResult]::new('check', 'check', 'ParameterValue', 'Type-check without execution')\n");
        s.push_str("        [CompletionResult]::new('build', 'build', 'ParameterValue', 'Build from fj.toml')\n");
        s.push_str("        [CompletionResult]::new('fmt', 'fmt', 'ParameterValue', 'Format source files')\n");
        s.push_str(
            "        [CompletionResult]::new('lsp', 'lsp', 'ParameterValue', 'Start LSP server')\n",
        );
        s.push_str("        [CompletionResult]::new('new', 'new', 'ParameterValue', 'Create a new project')\n");
        s.push_str("    )\n\n");
        s.push_str(
            "    $commands | Where-Object { $_.CompletionText -like \"$wordToComplete*\" }\n",
        );
        s.push_str("}\n");
        s
    }
}

// ═══════════════════════════════════════════════════════════════════════
// 6. VersionInfo — Detailed version information
// ═══════════════════════════════════════════════════════════════════════

/// Comprehensive version and build information.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionInfo {
    /// Semantic version string (e.g., "1.0.0").
    pub version: String,
    /// Git commit hash (short or full).
    pub git_hash: String,
    /// Build date in ISO 8601 format.
    pub build_date: String,
    /// Rust target triple used for the build.
    pub target_triple: String,
    /// Cargo features enabled during the build.
    pub features: Vec<String>,
    /// Rust compiler version used.
    pub rust_version: String,
}

impl VersionInfo {
    /// Creates a new `VersionInfo` with all fields.
    pub fn new(
        version: impl Into<String>,
        git_hash: impl Into<String>,
        build_date: impl Into<String>,
        target_triple: impl Into<String>,
        features: Vec<String>,
        rust_version: impl Into<String>,
    ) -> Self {
        Self {
            version: version.into(),
            git_hash: git_hash.into(),
            build_date: build_date.into(),
            target_triple: target_triple.into(),
            features,
            rust_version: rust_version.into(),
        }
    }

    /// Returns a short version string: `"fj <version>"`.
    pub fn format_short(&self) -> String {
        format!("fj {}", self.version)
    }

    /// Returns a multi-line version string with all details.
    pub fn format_long(&self) -> String {
        let features_str = if self.features.is_empty() {
            "none".to_string()
        } else {
            self.features.join(", ")
        };
        format!(
            "fj {}\n\
             commit:  {}\n\
             built:   {}\n\
             target:  {}\n\
             features: {}\n\
             rustc:   {}",
            self.version,
            self.git_hash,
            self.build_date,
            self.target_triple,
            features_str,
            self.rust_version,
        )
    }

    /// Returns a JSON-formatted version string (machine-readable).
    pub fn format_json(&self) -> String {
        let features_json: Vec<String> =
            self.features.iter().map(|f| format!("\"{}\"", f)).collect();
        format!(
            "{{\n\
             \x20 \"version\": \"{}\",\n\
             \x20 \"git_hash\": \"{}\",\n\
             \x20 \"build_date\": \"{}\",\n\
             \x20 \"target_triple\": \"{}\",\n\
             \x20 \"features\": [{}],\n\
             \x20 \"rust_version\": \"{}\"\n\
             }}",
            self.version,
            self.git_hash,
            self.build_date,
            self.target_triple,
            features_json.join(", "),
            self.rust_version,
        )
    }
}

impl fmt::Display for VersionInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.format_short())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// 7. PlatformOptimizer — Platform-specific optimization hints
// ═══════════════════════════════════════════════════════════════════════

/// I/O backend available on the current platform.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IoBackend {
    /// Linux io_uring (kernel 5.6+).
    IoUring,
    /// Linux epoll (fallback).
    Epoll,
    /// macOS/BSD kqueue.
    Kqueue,
    /// Windows I/O Completion Ports.
    Iocp,
    /// Generic poll-based fallback.
    Poll,
}

impl fmt::Display for IoBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IoUring => write!(f, "io_uring"),
            Self::Epoll => write!(f, "epoll"),
            Self::Kqueue => write!(f, "kqueue"),
            Self::Iocp => write!(f, "iocp"),
            Self::Poll => write!(f, "poll"),
        }
    }
}

/// Selects the best I/O backend for the current platform.
pub struct IoBackendSelector;

impl IoBackendSelector {
    /// Returns the recommended I/O backend for the given platform.
    ///
    /// - Linux: io_uring (preferred) or epoll (fallback)
    /// - macOS/FreeBSD: kqueue
    /// - Windows: IOCP
    /// - Unknown: generic poll
    pub fn select(platform: Platform) -> IoBackend {
        match platform {
            Platform::Linux => {
                // io_uring requires kernel 5.6+. We use it by default
                // on Linux and document the kernel version requirement.
                IoBackend::IoUring
            }
            Platform::MacOS | Platform::FreeBSD => IoBackend::Kqueue,
            Platform::Windows => IoBackend::Iocp,
            Platform::Unknown => IoBackend::Poll,
        }
    }

    /// Returns the fallback I/O backend for the given platform.
    pub fn fallback(platform: Platform) -> IoBackend {
        match platform {
            Platform::Linux => IoBackend::Epoll,
            Platform::MacOS | Platform::FreeBSD => IoBackend::Kqueue,
            Platform::Windows => IoBackend::Iocp,
            Platform::Unknown => IoBackend::Poll,
        }
    }
}

/// SIMD width selector based on CPU features.
pub struct SimdSelector;

impl SimdSelector {
    /// Returns the optimal SIMD width in bits for the given CPU features.
    pub fn optimal_width(features: &CpuFeatures) -> u32 {
        features.max_simd_width_bits()
    }

    /// Returns the number of f32 elements that fit in one SIMD register.
    pub fn f32_lanes(features: &CpuFeatures) -> u32 {
        features.max_simd_width_bits() / 32
    }

    /// Returns the number of f64 elements that fit in one SIMD register.
    pub fn f64_lanes(features: &CpuFeatures) -> u32 {
        features.max_simd_width_bits() / 64
    }
}

/// Thread pool configuration based on platform capabilities.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadPoolConfig {
    /// Number of worker threads for CPU-bound tasks.
    pub worker_threads: usize,
    /// Number of threads for blocking I/O tasks.
    pub blocking_threads: usize,
    /// Stack size per thread in bytes.
    pub stack_size: usize,
}

impl ThreadPoolConfig {
    /// Computes an optimal thread pool configuration for the platform.
    ///
    /// Worker threads = available CPUs (for CPU-bound work).
    /// Blocking threads = 2x available CPUs (for I/O-bound work).
    /// Stack size = 2 MiB (default) or 8 MiB on platforms with cheap
    /// virtual memory.
    pub fn optimal(info: &PlatformInfo) -> Self {
        let cpus = info.cpu_count.max(1);
        let stack_size = match info.os {
            Platform::Linux | Platform::MacOS | Platform::FreeBSD => 2 * 1024 * 1024, // 2 MiB
            Platform::Windows => 1024 * 1024, // 1 MiB (Windows default)
            Platform::Unknown => 2 * 1024 * 1024,
        };
        Self {
            worker_threads: cpus,
            blocking_threads: cpus * 2,
            stack_size,
        }
    }
}

impl fmt::Display for ThreadPoolConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "workers={}, blocking={}, stack={}KiB",
            self.worker_threads,
            self.blocking_threads,
            self.stack_size / 1024,
        )
    }
}

/// Memory configuration hints for the platform.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryConfig {
    /// Page size in bytes.
    pub page_size: usize,
    /// Whether huge pages (2 MiB / 1 GiB) are likely supported.
    pub huge_pages_supported: bool,
    /// Whether ASLR (address space layout randomization) is active.
    pub aslr_enabled: bool,
    /// Recommended alignment for SIMD buffers.
    pub simd_alignment: usize,
}

impl MemoryConfig {
    /// Detects memory configuration from platform info.
    pub fn detect(info: &PlatformInfo) -> Self {
        let huge_pages = matches!(info.os, Platform::Linux);
        let aslr = matches!(
            info.os,
            Platform::Linux | Platform::MacOS | Platform::Windows | Platform::FreeBSD
        );
        let simd_alignment = match info.cpu_features.max_simd_width_bits() {
            512 => 64,
            256 => 32,
            _ => 16,
        };

        Self {
            page_size: info.page_size,
            huge_pages_supported: huge_pages,
            aslr_enabled: aslr,
            simd_alignment,
        }
    }
}

impl fmt::Display for MemoryConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "page={}B, huge_pages={}, aslr={}, simd_align={}B",
            self.page_size, self.huge_pages_supported, self.aslr_enabled, self.simd_alignment,
        )
    }
}

/// Aggregated platform optimization recommendations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlatformOptimizations {
    /// Recommended I/O backend.
    pub io_backend: IoBackend,
    /// Optimal SIMD width in bits.
    pub simd_width_bits: u32,
    /// Thread pool configuration.
    pub thread_pool: ThreadPoolConfig,
    /// Memory configuration.
    pub memory: MemoryConfig,
}

impl PlatformOptimizations {
    /// Computes all optimization recommendations for the current platform.
    pub fn detect() -> Self {
        let info = PlatformInfo::detect();
        Self::for_platform(&info)
    }

    /// Computes optimization recommendations for a specific platform.
    pub fn for_platform(info: &PlatformInfo) -> Self {
        Self {
            io_backend: IoBackendSelector::select(info.os),
            simd_width_bits: SimdSelector::optimal_width(&info.cpu_features),
            thread_pool: ThreadPoolConfig::optimal(info),
            memory: MemoryConfig::detect(info),
        }
    }
}

impl fmt::Display for PlatformOptimizations {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "io={}, simd={}bit, {}, {}",
            self.io_backend, self.simd_width_bits, self.thread_pool, self.memory,
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests — 40 tests (s9_1 through s12_10)
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ────────────────────────────────────────────────────────────
    // Sprint 9: Platform Detection (s9_1 — s9_10)
    // ────────────────────────────────────────────────────────────

    #[test]
    fn s9_1_detect_host_os() {
        let info = PlatformInfo::detect();
        // We are always running on a desktop OS for tests.
        assert!(
            info.os == Platform::Linux
                || info.os == Platform::MacOS
                || info.os == Platform::Windows
                || info.os == Platform::FreeBSD,
            "detected OS should be a known desktop platform, got {:?}",
            info.os
        );
    }

    #[test]
    fn s9_2_detect_host_arch() {
        let info = PlatformInfo::detect();
        assert!(
            info.arch == Architecture::X86_64
                || info.arch == Architecture::Aarch64
                || info.arch == Architecture::Riscv64,
            "detected arch should be a known CPU arch, got {:?}",
            info.arch
        );
    }

    #[test]
    fn s9_3_platform_display_strings() {
        assert_eq!(Platform::Linux.to_string(), "linux");
        assert_eq!(Platform::MacOS.to_string(), "macos");
        assert_eq!(Platform::Windows.to_string(), "windows");
        assert_eq!(Platform::FreeBSD.to_string(), "freebsd");
        assert_eq!(Platform::Unknown.to_string(), "unknown");
    }

    #[test]
    fn s9_4_architecture_display_strings() {
        assert_eq!(Architecture::X86_64.to_string(), "x86_64");
        assert_eq!(Architecture::Aarch64.to_string(), "aarch64");
        assert_eq!(Architecture::Riscv64.to_string(), "riscv64");
        assert_eq!(Architecture::Wasm32.to_string(), "wasm32");
        assert_eq!(Architecture::Unknown.to_string(), "unknown");
    }

    #[test]
    fn s9_5_cpu_features_x86_detection() {
        let features = CpuFeatures::detect();
        if cfg!(target_arch = "x86_64") {
            assert!(features.has_sse42, "x86_64 should have SSE4.2");
            assert!(!features.has_neon, "x86_64 should not have NEON");
        }
    }

    #[test]
    fn s9_6_cpu_features_simd_width() {
        // SSE4.2 only: 128 bits
        let sse_only = CpuFeatures {
            has_sse42: true,
            has_avx2: false,
            has_avx512: false,
            has_neon: false,
            has_sve: false,
        };
        assert_eq!(sse_only.max_simd_width_bits(), 128);

        // AVX2: 256 bits
        let avx2 = CpuFeatures {
            has_avx2: true,
            ..sse_only
        };
        assert_eq!(avx2.max_simd_width_bits(), 256);

        // AVX-512: 512 bits
        let avx512 = CpuFeatures {
            has_avx512: true,
            ..avx2
        };
        assert_eq!(avx512.max_simd_width_bits(), 512);
    }

    #[test]
    fn s9_7_cpu_features_display() {
        let features = CpuFeatures {
            has_sse42: true,
            has_avx2: true,
            has_avx512: false,
            has_neon: false,
            has_sve: false,
        };
        assert_eq!(features.to_string(), "sse4.2, avx2");

        let empty = CpuFeatures {
            has_sse42: false,
            has_avx2: false,
            has_avx512: false,
            has_neon: false,
            has_sve: false,
        };
        assert_eq!(empty.to_string(), "none");
    }

    #[test]
    fn s9_8_platform_info_target_triple() {
        let info = PlatformInfo {
            os: Platform::Linux,
            arch: Architecture::X86_64,
            page_size: 4096,
            cpu_count: 8,
            cpu_features: CpuFeatures::detect(),
            endianness: Endianness::Little,
        };
        assert_eq!(info.target_triple(), "x86_64-unknown-linux-gnu");

        let mac_arm = PlatformInfo {
            os: Platform::MacOS,
            arch: Architecture::Aarch64,
            ..info.clone()
        };
        assert_eq!(mac_arm.target_triple(), "aarch64-apple-darwin");
    }

    #[test]
    fn s9_9_endianness_detection() {
        let info = PlatformInfo::detect();
        // All common desktop platforms are little-endian.
        assert_eq!(info.endianness, Endianness::Little);
    }

    #[test]
    fn s9_10_page_size_and_cpu_count() {
        let info = PlatformInfo::detect();
        assert!(info.page_size >= 4096, "page size should be at least 4096");
        assert!(info.cpu_count >= 1, "should detect at least 1 CPU");
    }

    // ────────────────────────────────────────────────────────────
    // Sprint 10: Path & Line Ending (s10_1 — s10_10)
    // ────────────────────────────────────────────────────────────

    #[test]
    fn s10_1_path_normalize_separators() {
        // On the current platform, normalize should produce the right separator.
        let result = PathNormalizer::normalize("src\\parser\\mod.rs");
        if cfg!(target_os = "windows") {
            assert_eq!(result, "src\\parser\\mod.rs");
        } else {
            assert_eq!(result, "src/parser/mod.rs");
        }
    }

    #[test]
    fn s10_2_path_collapse_multiple_separators() {
        let result = PathNormalizer::normalize("src///parser//mod.rs");
        if cfg!(target_os = "windows") {
            assert_eq!(result, "src\\parser\\mod.rs");
        } else {
            assert_eq!(result, "src/parser/mod.rs");
        }
    }

    #[test]
    fn s10_3_path_to_uri_unix() {
        let uri = PathNormalizer::to_uri("/home/user/file.fj").unwrap();
        assert_eq!(uri, "file:///home/user/file.fj");
    }

    #[test]
    fn s10_4_path_to_uri_windows_drive() {
        let uri = PathNormalizer::to_uri("C:\\Users\\file.fj").unwrap();
        assert_eq!(uri, "file:///C:/Users/file.fj");
    }

    #[test]
    fn s10_5_path_from_uri_unix() {
        let path = PathNormalizer::from_uri("file:///home/user/file.fj").unwrap();
        if cfg!(target_os = "windows") {
            assert_eq!(path, "\\home\\user\\file.fj");
        } else {
            assert_eq!(path, "/home/user/file.fj");
        }
    }

    #[test]
    fn s10_6_path_from_uri_windows_drive() {
        let path = PathNormalizer::from_uri("file:///C:/Users/file.fj").unwrap();
        if cfg!(target_os = "windows") {
            assert_eq!(path, "C:\\Users\\file.fj");
        } else {
            assert_eq!(path, "C:/Users/file.fj");
        }
    }

    #[test]
    fn s10_7_path_join() {
        let joined = PathNormalizer::join_paths(&["/home/user", "projects", "fj", "src"]);
        if cfg!(target_os = "windows") {
            assert_eq!(joined, "/home/user\\projects\\fj\\src");
        } else {
            assert_eq!(joined, "/home/user/projects/fj/src");
        }
    }

    #[test]
    fn s10_8_path_make_relative() {
        let rel =
            PathNormalizer::make_relative("/home/user/projects", "/home/user/projects/fj/src")
                .unwrap();
        assert_eq!(rel, "fj/src");

        let up =
            PathNormalizer::make_relative("/home/user/projects/fj", "/home/user/docs").unwrap();
        assert_eq!(up, "../../docs");
    }

    #[test]
    fn s10_9_line_ending_detection() {
        assert_eq!(LineEndingHandler::detect("hello\nworld\n"), LineEnding::Lf);
        assert_eq!(
            LineEndingHandler::detect("hello\r\nworld\r\n"),
            LineEnding::CrLf
        );
        assert_eq!(LineEndingHandler::detect("hello\rworld\r"), LineEnding::Cr);
        // Empty string defaults to LF.
        assert_eq!(LineEndingHandler::detect(""), LineEnding::Lf);
    }

    #[test]
    fn s10_10_line_ending_conversion() {
        let crlf_input = "line1\r\nline2\r\nline3";
        let lf_result = LineEndingHandler::normalize_to_lf(crlf_input);
        assert_eq!(lf_result, "line1\nline2\nline3");

        let back_to_crlf = LineEndingHandler::convert(&lf_result, LineEnding::CrLf);
        assert_eq!(back_to_crlf, "line1\r\nline2\r\nline3");

        let to_cr = LineEndingHandler::convert(&lf_result, LineEnding::Cr);
        assert_eq!(to_cr, "line1\rline2\rline3");
    }

    // ────────────────────────────────────────────────────────────
    // Sprint 11: Distribution (s11_1 — s11_10)
    // ────────────────────────────────────────────────────────────

    #[test]
    fn s11_1_target_triple_generation() {
        let t = Target::new(Platform::Linux, Architecture::X86_64);
        assert_eq!(t.triple(), "x86_64-unknown-linux-gnu");

        let tw = Target::new(Platform::Windows, Architecture::X86_64);
        assert_eq!(tw.triple(), "x86_64-pc-windows-msvc");
    }

    #[test]
    fn s11_2_target_binary_name() {
        let linux = Target::new(Platform::Linux, Architecture::X86_64);
        assert_eq!(linux.binary_name("fj"), "fj");

        let windows = Target::new(Platform::Windows, Architecture::X86_64);
        assert_eq!(windows.binary_name("fj"), "fj.exe");
    }

    #[test]
    fn s11_3_release_artifact_human_size() {
        let a = ReleaseArtifact::new(
            "fj.tar.gz",
            Target::new(Platform::Linux, Architecture::X86_64),
            512,
            "abc123",
        );
        assert_eq!(a.human_size(), "512 B");

        let b = ReleaseArtifact::new(
            "fj.tar.gz",
            Target::new(Platform::Linux, Architecture::X86_64),
            2 * 1024 * 1024,
            "abc123",
        );
        assert_eq!(b.human_size(), "2.0 MiB");

        let c = ReleaseArtifact::new(
            "fj.tar.gz",
            Target::new(Platform::Linux, Architecture::X86_64),
            1536,
            "abc123",
        );
        assert_eq!(c.human_size(), "1.5 KiB");
    }

    #[test]
    fn s11_4_dist_profile_config() {
        let cfg = DistProfile::config();
        assert_eq!(cfg.opt_level, "3");
        assert_eq!(cfg.lto, LtoSetting::Fat);
        assert!(cfg.strip);
        assert_eq!(cfg.codegen_units, 1);
        assert_eq!(cfg.panic_strategy, PanicStrategy::Abort);
    }

    #[test]
    fn s11_5_dist_profile_toml() {
        let toml = DistProfile::to_cargo_toml_section();
        assert!(toml.contains("[profile.release-dist]"));
        assert!(toml.contains("lto = \"fat\""));
        assert!(toml.contains("strip = true"));
        assert!(toml.contains("codegen-units = 1"));
        assert!(toml.contains("panic = \"abort\""));
    }

    #[test]
    fn s11_6_artifact_manifest_operations() {
        let mut manifest = ArtifactManifest::new("1.0.0", "2026-03-11");
        let a1 = ReleaseArtifact::new(
            "fj-1.0.0-x86_64-linux.tar.gz",
            Target::new(Platform::Linux, Architecture::X86_64),
            5_000_000,
            "aabbccdd",
        );
        let a2 = ReleaseArtifact::new(
            "fj-1.0.0-aarch64-macos.tar.gz",
            Target::new(Platform::MacOS, Architecture::Aarch64),
            4_800_000,
            "eeff0011",
        );
        manifest.add_artifact(a1);
        manifest.add_artifact(a2);

        assert_eq!(manifest.artifacts.len(), 2);
        assert_eq!(manifest.total_size_bytes(), 9_800_000);

        let found = manifest.find_for_target(Platform::Linux, Architecture::X86_64);
        assert!(found.is_some());
        assert_eq!(found.unwrap().sha256, "aabbccdd");

        let not_found = manifest.find_for_target(Platform::Windows, Architecture::X86_64);
        assert!(not_found.is_none());
    }

    #[test]
    fn s11_7_checksum_generation() {
        let mut manifest = ArtifactManifest::new("1.0.0", "2026-03-11");
        manifest.add_artifact(ReleaseArtifact::new(
            "fj-linux.tar.gz",
            Target::new(Platform::Linux, Architecture::X86_64),
            1000,
            "aaaa1111",
        ));
        manifest.add_artifact(ReleaseArtifact::new(
            "fj-macos.tar.gz",
            Target::new(Platform::MacOS, Architecture::X86_64),
            900,
            "bbbb2222",
        ));

        let checksums = ChecksumGenerator::generate(&manifest);
        assert!(checksums.contains("aaaa1111  fj-linux.tar.gz"));
        assert!(checksums.contains("bbbb2222  fj-macos.tar.gz"));
    }

    #[test]
    fn s11_8_checksum_verify_pass() {
        let result = ChecksumGenerator::verify("fj.tar.gz", "abc123", "abc123");
        assert!(result.is_ok());
    }

    #[test]
    fn s11_9_checksum_verify_fail() {
        let result = ChecksumGenerator::verify("fj.tar.gz", "abc123", "def456");
        assert!(result.is_err());
        match result {
            Err(CrossPlatformError::ChecksumMismatch {
                artifact,
                expected,
                actual,
            }) => {
                assert_eq!(artifact, "fj.tar.gz");
                assert_eq!(expected, "abc123");
                assert_eq!(actual, "def456");
            }
            _ => panic!("expected ChecksumMismatch error"),
        }
    }

    #[test]
    fn s11_10_artifact_with_signature() {
        let a = ReleaseArtifact::new(
            "fj.tar.gz",
            Target::new(Platform::Linux, Architecture::X86_64),
            1000,
            "sha256hex",
        )
        .with_signature("pgp-sig-data");

        assert_eq!(a.signature, Some("pgp-sig-data".to_string()));
    }

    // ────────────────────────────────────────────────────────────
    // Sprint 12: Installers & Optimization (s12_1 — s12_10)
    // ────────────────────────────────────────────────────────────

    #[test]
    fn s12_1_shell_installer_generation() {
        let config = InstallerConfig::default();
        let script = ShellInstaller::generate(&config);
        assert!(script.starts_with("#!/bin/sh"));
        assert!(script.contains("detect_os()"));
        assert!(script.contains("detect_arch()"));
        assert!(script.contains("curl"));
        assert!(script.contains("wget"));
        assert!(script.contains("tar xz"));
    }

    #[test]
    fn s12_2_powershell_installer_generation() {
        let config = InstallerConfig::default();
        let script = PowerShellInstaller::generate(&config);
        assert!(script.contains("$ErrorActionPreference"));
        assert!(script.contains("Invoke-WebRequest"));
        assert!(script.contains("Expand-Archive"));
        assert!(script.contains("Path"));
    }

    #[test]
    fn s12_3_homebrew_formula_generation() {
        let formula = HomebrewFormula::generate(
            "1.0.0",
            "sha256_arm",
            "sha256_x86",
            "sha256_linux",
            "https://example.com/releases",
        );
        assert!(formula.contains("class Fj < Formula"));
        assert!(formula.contains("version \"1.0.0\""));
        assert!(formula.contains("sha256 \"sha256_arm\""));
        assert!(formula.contains("sha256 \"sha256_x86\""));
        assert!(formula.contains("sha256 \"sha256_linux\""));
        assert!(formula.contains("bin.install \"fj\""));
    }

    #[test]
    fn s12_4_debian_control_generation() {
        let control = DebianPackage::generate_control("1.0.0", "x86_64");
        assert!(control.contains("Package: fj"));
        assert!(control.contains("Version: 1.0.0"));
        assert!(control.contains("Architecture: amd64"));
        assert!(control.contains("Maintainer:"));
        assert!(control.contains("Description:"));
    }

    #[test]
    fn s12_5_completion_scripts_bash_zsh_fish_powershell() {
        let bash = CompletionGenerator::generate(Shell::Bash);
        assert!(bash.contains("_fj()"));
        assert!(bash.contains("complete -F _fj fj"));

        let zsh = CompletionGenerator::generate(Shell::Zsh);
        assert!(zsh.contains("#compdef fj"));
        assert!(zsh.contains("_describe"));

        let fish = CompletionGenerator::generate(Shell::Fish);
        assert!(fish.contains("complete -c fj"));
        assert!(fish.contains("run"));

        let ps = CompletionGenerator::generate(Shell::PowerShell);
        assert!(ps.contains("Register-ArgumentCompleter"));
        assert!(ps.contains("CompletionResult"));
    }

    #[test]
    fn s12_6_version_info_formats() {
        let v = VersionInfo::new(
            "1.0.0",
            "abc1234",
            "2026-03-11",
            "x86_64-unknown-linux-gnu",
            vec!["native".to_string(), "gpu".to_string()],
            "1.85.0",
        );
        assert_eq!(v.format_short(), "fj 1.0.0");

        let long = v.format_long();
        assert!(long.contains("fj 1.0.0"));
        assert!(long.contains("commit:  abc1234"));
        assert!(long.contains("built:   2026-03-11"));
        assert!(long.contains("target:  x86_64-unknown-linux-gnu"));
        assert!(long.contains("features: native, gpu"));
        assert!(long.contains("rustc:   1.85.0"));

        let json = v.format_json();
        assert!(json.contains("\"version\": \"1.0.0\""));
        assert!(json.contains("\"git_hash\": \"abc1234\""));
        assert!(json.contains("\"native\""));
        assert!(json.contains("\"gpu\""));
    }

    #[test]
    fn s12_7_io_backend_selection() {
        assert_eq!(
            IoBackendSelector::select(Platform::Linux),
            IoBackend::IoUring
        );
        assert_eq!(
            IoBackendSelector::select(Platform::MacOS),
            IoBackend::Kqueue
        );
        assert_eq!(
            IoBackendSelector::select(Platform::Windows),
            IoBackend::Iocp
        );
        assert_eq!(
            IoBackendSelector::select(Platform::FreeBSD),
            IoBackend::Kqueue
        );
        assert_eq!(
            IoBackendSelector::select(Platform::Unknown),
            IoBackend::Poll
        );

        // Fallback on Linux is epoll
        assert_eq!(
            IoBackendSelector::fallback(Platform::Linux),
            IoBackend::Epoll
        );
    }

    #[test]
    fn s12_8_simd_selector() {
        let features = CpuFeatures {
            has_sse42: true,
            has_avx2: true,
            has_avx512: false,
            has_neon: false,
            has_sve: false,
        };
        assert_eq!(SimdSelector::optimal_width(&features), 256);
        assert_eq!(SimdSelector::f32_lanes(&features), 8);
        assert_eq!(SimdSelector::f64_lanes(&features), 4);
    }

    #[test]
    fn s12_9_thread_pool_config() {
        let info = PlatformInfo {
            os: Platform::Linux,
            arch: Architecture::X86_64,
            page_size: 4096,
            cpu_count: 8,
            cpu_features: CpuFeatures::detect(),
            endianness: Endianness::Little,
        };
        let pool = ThreadPoolConfig::optimal(&info);
        assert_eq!(pool.worker_threads, 8);
        assert_eq!(pool.blocking_threads, 16);
        assert_eq!(pool.stack_size, 2 * 1024 * 1024);
    }

    #[test]
    fn s12_10_memory_config_detection() {
        let info = PlatformInfo {
            os: Platform::Linux,
            arch: Architecture::X86_64,
            page_size: 4096,
            cpu_count: 4,
            cpu_features: CpuFeatures {
                has_sse42: true,
                has_avx2: true,
                has_avx512: false,
                has_neon: false,
                has_sve: false,
            },
            endianness: Endianness::Little,
        };
        let mem = MemoryConfig::detect(&info);
        assert_eq!(mem.page_size, 4096);
        assert!(mem.huge_pages_supported);
        assert!(mem.aslr_enabled);
        assert_eq!(mem.simd_alignment, 32); // AVX2 -> 32-byte alignment
    }
}
