//! # CPU Feature Detection
//!
//! Safe runtime detection of CPU ISA extensions across x86_64, AArch64, and
//! RISC-V architectures. All detection is runtime-only — no compile-time
//! assumptions about available features.
//!
//! ## Supported Features
//!
//! | Architecture | Features |
//! |-------------|----------|
//! | x86_64 | SSE4.2, AVX2, FMA3, AVX-512F/BW/VNNI/BF16, AMX-BF16/INT8/FP16 |
//! | AArch64 | NEON, SVE, SVE2, BF16, I8MM, DotProd |
//! | RISC-V | V (vector), Zfh (half-float), Zvfh |

use serde::Serialize;
use std::sync::OnceLock;

// ═══════════════════════════════════════════════════════════════════════
// CPU Vendor
// ═══════════════════════════════════════════════════════════════════════

/// CPU vendor identification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum CpuVendor {
    /// Intel Corporation
    Intel,
    /// Advanced Micro Devices
    Amd,
    /// ARM Holdings (AArch64)
    Arm,
    /// RISC-V International
    RiscV,
    /// Unknown or unsupported vendor
    Unknown,
}

impl std::fmt::Display for CpuVendor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CpuVendor::Intel => write!(f, "Intel"),
            CpuVendor::Amd => write!(f, "AMD"),
            CpuVendor::Arm => write!(f, "ARM"),
            CpuVendor::RiscV => write!(f, "RISC-V"),
            CpuVendor::Unknown => write!(f, "Unknown"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// CPU Features
// ═══════════════════════════════════════════════════════════════════════

/// Detected CPU features and ISA extensions.
///
/// Constructed via [`CpuFeatures::detect()`] which probes the current
/// platform at runtime. Fields are architecture-agnostic booleans —
/// features not applicable to the current architecture are always `false`.
///
/// # Examples
///
/// ```
/// use fajar_lang::hw::CpuFeatures;
///
/// let cpu = CpuFeatures::detect();
/// println!("Vendor: {}", cpu.vendor);
/// println!("AVX2: {}", cpu.avx2);
/// ```
#[derive(Debug, Clone, Serialize)]
pub struct CpuFeatures {
    // ── Identity ──────────────────────────────────────────────────────
    /// CPU vendor (Intel, AMD, ARM, RISC-V, Unknown).
    pub vendor: CpuVendor,
    /// CPU model name string (e.g., "13th Gen Intel Core i9-13900K").
    pub model_name: String,
    /// Target architecture string (e.g., "x86_64", "aarch64", "riscv64").
    pub arch: String,

    // ── x86_64: Baseline SIMD ─────────────────────────────────────────
    /// SSE4.2 — baseline SIMD (string operations, CRC32).
    pub sse42: bool,
    /// AVX2 — 256-bit integer SIMD.
    pub avx2: bool,
    /// FMA3 — fused multiply-add (3-operand).
    pub fma: bool,

    // ── x86_64: AVX-512 Family ────────────────────────────────────────
    /// AVX-512F — Foundation (512-bit float/int ops).
    pub avx512f: bool,
    /// AVX-512BW — Byte and Word operations.
    pub avx512bw: bool,
    /// AVX-512VNNI — Vector Neural Network Instructions (INT8 dot product).
    pub avx512vnni: bool,
    /// AVX-512BF16 — BFloat16 conversion and dot product.
    pub avx512bf16: bool,

    // ── x86_64: AMX (Advanced Matrix Extensions) ──────────────────────
    /// AMX-BF16 — Tile BF16 matrix multiply-accumulate.
    pub amx_bf16: bool,
    /// AMX-INT8 — Tile INT8 matrix multiply-accumulate.
    pub amx_int8: bool,
    /// AMX-FP16 — Tile FP16 matrix multiply-accumulate.
    pub amx_fp16: bool,

    // ── AArch64 ───────────────────────────────────────────────────────
    /// NEON / Advanced SIMD (128-bit, baseline on AArch64).
    pub neon: bool,
    /// SVE — Scalable Vector Extension (variable-length vectors).
    pub sve: bool,
    /// SVE2 — Scalable Vector Extension v2.
    pub sve2: bool,
    /// BF16 — BFloat16 instructions (AArch64).
    pub arm_bf16: bool,
    /// I8MM — INT8 matrix multiply (AArch64).
    pub arm_i8mm: bool,
    /// DotProd — dot product instructions (AArch64).
    pub arm_dotprod: bool,

    // ── RISC-V ────────────────────────────────────────────────────────
    /// V — RISC-V Vector extension.
    pub riscv_v: bool,
    /// Zfh — RISC-V half-precision float extension.
    pub riscv_zfh: bool,
}

impl Default for CpuFeatures {
    fn default() -> Self {
        Self {
            vendor: CpuVendor::Unknown,
            model_name: String::new(),
            arch: String::new(),
            sse42: false,
            avx2: false,
            fma: false,
            avx512f: false,
            avx512bw: false,
            avx512vnni: false,
            avx512bf16: false,
            amx_bf16: false,
            amx_int8: false,
            amx_fp16: false,
            neon: false,
            sve: false,
            sve2: false,
            arm_bf16: false,
            arm_i8mm: false,
            arm_dotprod: false,
            riscv_v: false,
            riscv_zfh: false,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Runtime Cache (OnceLock)
// ═══════════════════════════════════════════════════════════════════════

/// Global cached CPU features — initialized once on first access.
static CPU_FEATURES_CACHE: OnceLock<CpuFeatures> = OnceLock::new();

impl CpuFeatures {
    /// Detect CPU features for the current platform.
    ///
    /// This is a lightweight operation — results are cached after first call.
    pub fn detect() -> Self {
        CPU_FEATURES_CACHE.get_or_init(Self::probe).clone()
    }

    /// Get a reference to cached features (avoids clone).
    pub fn cached() -> &'static CpuFeatures {
        CPU_FEATURES_CACHE.get_or_init(Self::probe)
    }

    /// Probe the current CPU — called once by OnceLock.
    fn probe() -> CpuFeatures {
        #[cfg(target_arch = "x86_64")]
        {
            Self::probe_x86_64()
        }
        #[cfg(target_arch = "aarch64")]
        {
            Self::probe_aarch64()
        }
        #[cfg(target_arch = "riscv64")]
        {
            Self::probe_riscv64()
        }
        #[cfg(not(any(
            target_arch = "x86_64",
            target_arch = "aarch64",
            target_arch = "riscv64"
        )))]
        {
            CpuFeatures {
                arch: std::env::consts::ARCH.to_string(),
                ..Default::default()
            }
        }
    }

    // ═══════════════════════════════════════════════════════════════════
    // x86_64 Detection
    // ═══════════════════════════════════════════════════════════════════

    /// Probe x86_64 CPU features via CPUID instruction.
    #[cfg(target_arch = "x86_64")]
    fn probe_x86_64() -> CpuFeatures {
        let vendor = Self::detect_x86_vendor();
        let model_name = Self::detect_x86_model_name();

        CpuFeatures {
            vendor,
            model_name,
            arch: "x86_64".to_string(),

            // Baseline SIMD — use std macros (safe, no unsafe needed)
            sse42: std::is_x86_feature_detected!("sse4.2"),
            avx2: std::is_x86_feature_detected!("avx2"),
            fma: std::is_x86_feature_detected!("fma"),

            // AVX-512 family
            avx512f: std::is_x86_feature_detected!("avx512f"),
            avx512bw: std::is_x86_feature_detected!("avx512bw"),
            avx512vnni: std::is_x86_feature_detected!("avx512vnni"),
            avx512bf16: std::is_x86_feature_detected!("avx512bf16"),

            // AMX — detected via CPUID leaf 7 subleaf 0, EDX bits
            amx_bf16: Self::detect_amx_bf16(),
            amx_int8: Self::detect_amx_int8(),
            amx_fp16: Self::detect_amx_fp16(),

            // Non-x86 features are false
            neon: false,
            sve: false,
            sve2: false,
            arm_bf16: false,
            arm_i8mm: false,
            arm_dotprod: false,
            riscv_v: false,
            riscv_zfh: false,
        }
    }

    /// Detect x86 vendor from CPUID leaf 0.
    #[cfg(target_arch = "x86_64")]
    #[allow(unused_unsafe)]
    fn detect_x86_vendor() -> CpuVendor {
        // SAFETY: CPUID leaf 0 is always available on x86_64
        let result = unsafe { std::arch::x86_64::__cpuid(0) };
        let mut vendor_bytes = [0u8; 12];
        vendor_bytes[0..4].copy_from_slice(&result.ebx.to_le_bytes());
        vendor_bytes[4..8].copy_from_slice(&result.edx.to_le_bytes());
        vendor_bytes[8..12].copy_from_slice(&result.ecx.to_le_bytes());

        match &vendor_bytes {
            b"GenuineIntel" => CpuVendor::Intel,
            b"AuthenticAMD" => CpuVendor::Amd,
            _ => CpuVendor::Unknown,
        }
    }

    /// Read x86 CPU model name from CPUID leaves 0x80000002-0x80000004.
    #[cfg(target_arch = "x86_64")]
    #[allow(unused_unsafe)]
    fn detect_x86_model_name() -> String {
        // Check if extended CPUID is supported
        // SAFETY: CPUID leaf 0x80000000 is always available on x86_64
        let ext_max = unsafe { std::arch::x86_64::__cpuid(0x8000_0000) };
        if ext_max.eax < 0x8000_0004 {
            return String::from("Unknown x86_64 CPU");
        }

        let mut name_bytes = [0u8; 48];
        for (i, leaf) in (0x8000_0002u32..=0x8000_0004).enumerate() {
            // SAFETY: Leaf availability checked above
            let result = unsafe { std::arch::x86_64::__cpuid(leaf) };
            let offset = i * 16;
            name_bytes[offset..offset + 4].copy_from_slice(&result.eax.to_le_bytes());
            name_bytes[offset + 4..offset + 8].copy_from_slice(&result.ebx.to_le_bytes());
            name_bytes[offset + 8..offset + 12].copy_from_slice(&result.ecx.to_le_bytes());
            name_bytes[offset + 12..offset + 16].copy_from_slice(&result.edx.to_le_bytes());
        }

        String::from_utf8_lossy(&name_bytes)
            .trim_end_matches('\0')
            .trim()
            .to_string()
    }

    /// Detect AMX-BF16 via CPUID leaf 7, subleaf 0, EDX bit 22.
    #[cfg(target_arch = "x86_64")]
    #[allow(unused_unsafe)]
    fn detect_amx_bf16() -> bool {
        // SAFETY: CPUID leaf 0 always available
        let max_leaf = unsafe { std::arch::x86_64::__cpuid(0) }.eax;
        if max_leaf < 7 {
            return false;
        }
        // SAFETY: Leaf 7 available (checked above)
        let result = unsafe { std::arch::x86_64::__cpuid_count(7, 0) };
        (result.edx >> 22) & 1 == 1
    }

    /// Detect AMX-INT8 via CPUID leaf 7, subleaf 0, EDX bit 25.
    #[cfg(target_arch = "x86_64")]
    #[allow(unused_unsafe)]
    fn detect_amx_int8() -> bool {
        // SAFETY: CPUID leaf 0 is always available on x86_64
        let max_leaf = unsafe { std::arch::x86_64::__cpuid(0) }.eax;
        if max_leaf < 7 {
            return false;
        }
        // SAFETY: Leaf 7 available (checked above)
        let result = unsafe { std::arch::x86_64::__cpuid_count(7, 0) };
        (result.edx >> 25) & 1 == 1
    }

    /// Detect AMX-FP16 via CPUID leaf 7, subleaf 1, EAX bit 21.
    #[cfg(target_arch = "x86_64")]
    #[allow(unused_unsafe)]
    fn detect_amx_fp16() -> bool {
        // SAFETY: CPUID leaf 0 is always available on x86_64
        let max_leaf = unsafe { std::arch::x86_64::__cpuid(0) }.eax;
        if max_leaf < 7 {
            return false;
        }
        // SAFETY: Leaf 7 available (checked above)
        let result = unsafe { std::arch::x86_64::__cpuid_count(7, 1) };
        (result.eax >> 21) & 1 == 1
    }

    // ═══════════════════════════════════════════════════════════════════
    // AArch64 Detection
    // ═══════════════════════════════════════════════════════════════════

    /// Probe AArch64 CPU features via /proc/cpuinfo or std feature detection.
    #[cfg(target_arch = "aarch64")]
    fn probe_aarch64() -> CpuFeatures {
        let cpuinfo = std::fs::read_to_string("/proc/cpuinfo").unwrap_or_default();
        let model_name = Self::parse_cpuinfo_field(&cpuinfo, "model name")
            .or_else(|| Self::parse_cpuinfo_field(&cpuinfo, "CPU implementer"))
            .unwrap_or_else(|| "AArch64 CPU".to_string());

        CpuFeatures {
            vendor: CpuVendor::Arm,
            model_name,
            arch: "aarch64".to_string(),

            // x86 features are false
            sse42: false,
            avx2: false,
            fma: false,
            avx512f: false,
            avx512bw: false,
            avx512vnni: false,
            avx512bf16: false,
            amx_bf16: false,
            amx_int8: false,
            amx_fp16: false,

            // AArch64 features via std macros
            neon: std::arch::is_aarch64_feature_detected!("neon"),
            sve: std::arch::is_aarch64_feature_detected!("sve"),
            sve2: std::arch::is_aarch64_feature_detected!("sve2"),
            arm_bf16: std::arch::is_aarch64_feature_detected!("bf16"),
            arm_i8mm: std::arch::is_aarch64_feature_detected!("i8mm"),
            arm_dotprod: std::arch::is_aarch64_feature_detected!("dotprod"),

            riscv_v: false,
            riscv_zfh: false,
        }
    }

    // ═══════════════════════════════════════════════════════════════════
    // RISC-V Detection
    // ═══════════════════════════════════════════════════════════════════

    /// Probe RISC-V features by parsing the ISA string from /proc/cpuinfo.
    #[cfg(target_arch = "riscv64")]
    fn probe_riscv64() -> CpuFeatures {
        let cpuinfo = std::fs::read_to_string("/proc/cpuinfo").unwrap_or_default();
        let isa_string = Self::parse_cpuinfo_field(&cpuinfo, "isa")
            .unwrap_or_default()
            .to_lowercase();
        let model_name = Self::parse_cpuinfo_field(&cpuinfo, "uarch")
            .or_else(|| Self::parse_cpuinfo_field(&cpuinfo, "model name"))
            .unwrap_or_else(|| "RISC-V CPU".to_string());

        CpuFeatures {
            vendor: CpuVendor::RiscV,
            model_name,
            arch: "riscv64".to_string(),

            // x86 features are false
            sse42: false,
            avx2: false,
            fma: false,
            avx512f: false,
            avx512bw: false,
            avx512vnni: false,
            avx512bf16: false,
            amx_bf16: false,
            amx_int8: false,
            amx_fp16: false,

            // AArch64 features are false
            neon: false,
            sve: false,
            sve2: false,
            arm_bf16: false,
            arm_i8mm: false,
            arm_dotprod: false,

            // RISC-V: parse ISA string for extensions
            riscv_v: isa_string.contains("_v_") || isa_string.ends_with("v"),
            riscv_zfh: isa_string.contains("zfh"),
        }
    }

    // ═══════════════════════════════════════════════════════════════════
    // Helpers
    // ═══════════════════════════════════════════════════════════════════

    /// Parse a field from /proc/cpuinfo format ("key : value\n").
    #[cfg(any(target_arch = "aarch64", target_arch = "riscv64"))]
    fn parse_cpuinfo_field(cpuinfo: &str, field: &str) -> Option<String> {
        for line in cpuinfo.lines() {
            let parts: Vec<&str> = line.splitn(2, ':').collect();
            if parts.len() == 2 && parts[0].trim().eq_ignore_ascii_case(field) {
                return Some(parts[1].trim().to_string());
            }
        }
        None
    }

    // ═══════════════════════════════════════════════════════════════════
    // Query API (public convenience methods)
    // ═══════════════════════════════════════════════════════════════════

    /// Returns `true` if any AVX-512 extension is available.
    pub fn has_avx512(&self) -> bool {
        self.avx512f
    }

    /// Returns `true` if any AMX extension is available.
    pub fn has_amx(&self) -> bool {
        self.amx_bf16 || self.amx_int8 || self.amx_fp16
    }

    /// Returns `true` if SVE (any version) is available.
    pub fn has_sve(&self) -> bool {
        self.sve || self.sve2
    }

    /// Returns `true` if RISC-V vector extension is available.
    pub fn has_riscv_vector(&self) -> bool {
        self.riscv_v
    }

    /// Returns the best available SIMD width in bits.
    ///
    /// - AVX-512: 512
    /// - AVX2: 256
    /// - SSE4.2 / NEON: 128
    /// - Fallback: 64 (scalar)
    pub fn best_simd_width(&self) -> u32 {
        if self.avx512f {
            512
        } else if self.avx2 {
            256
        } else if self.sse42 || self.neon {
            128
        } else {
            64
        }
    }

    /// Returns a compact list of detected feature names.
    pub fn feature_list(&self) -> Vec<&'static str> {
        let mut features = Vec::new();

        // x86_64
        if self.sse42 {
            features.push("SSE4.2");
        }
        if self.avx2 {
            features.push("AVX2");
        }
        if self.fma {
            features.push("FMA3");
        }
        if self.avx512f {
            features.push("AVX-512F");
        }
        if self.avx512bw {
            features.push("AVX-512BW");
        }
        if self.avx512vnni {
            features.push("AVX-512VNNI");
        }
        if self.avx512bf16 {
            features.push("AVX-512BF16");
        }
        if self.amx_bf16 {
            features.push("AMX-BF16");
        }
        if self.amx_int8 {
            features.push("AMX-INT8");
        }
        if self.amx_fp16 {
            features.push("AMX-FP16");
        }

        // AArch64
        if self.neon {
            features.push("NEON");
        }
        if self.sve {
            features.push("SVE");
        }
        if self.sve2 {
            features.push("SVE2");
        }
        if self.arm_bf16 {
            features.push("BF16");
        }
        if self.arm_i8mm {
            features.push("I8MM");
        }
        if self.arm_dotprod {
            features.push("DotProd");
        }

        // RISC-V
        if self.riscv_v {
            features.push("RVV");
        }
        if self.riscv_zfh {
            features.push("Zfh");
        }

        features
    }

    /// Format CPU info for human-readable display.
    pub fn display_info(&self) -> String {
        let mut out = String::new();
        out.push_str("── CPU ──────────────────────────────────\n");
        out.push_str(&format!("  Vendor:     {}\n", self.vendor));
        out.push_str(&format!("  Model:      {}\n", self.model_name));
        out.push_str(&format!("  Arch:       {}\n", self.arch));
        out.push_str(&format!("  SIMD width: {}-bit\n", self.best_simd_width()));

        let features = self.feature_list();
        if features.is_empty() {
            out.push_str("  Features:   (none detected)\n");
        } else {
            out.push_str(&format!("  Features:   {}\n", features.join(", ")));
        }

        // AMX summary (if any)
        if self.has_amx() {
            let amx_features: Vec<&str> = [
                self.amx_bf16.then_some("BF16"),
                self.amx_int8.then_some("INT8"),
                self.amx_fp16.then_some("FP16"),
            ]
            .into_iter()
            .flatten()
            .collect();
            out.push_str(&format!("  AMX:        {}\n", amx_features.join(", ")));
        }

        out
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── S1.1: CPUID Wrapper ───────────────────────────────────────────

    #[test]
    fn detect_returns_valid_vendor() {
        let cpu = CpuFeatures::detect();
        // On x86_64, vendor should be Intel or AMD
        // On other arches, it should be Arm/RiscV/Unknown
        #[cfg(target_arch = "x86_64")]
        assert!(
            cpu.vendor == CpuVendor::Intel || cpu.vendor == CpuVendor::Amd,
            "x86_64 vendor should be Intel or AMD, got: {:?}",
            cpu.vendor
        );
        #[cfg(target_arch = "aarch64")]
        assert_eq!(cpu.vendor, CpuVendor::Arm);
    }

    #[test]
    fn detect_returns_non_empty_model_name() {
        let cpu = CpuFeatures::detect();
        assert!(!cpu.model_name.is_empty(), "model name should not be empty");
    }

    #[test]
    fn detect_returns_correct_arch() {
        let cpu = CpuFeatures::detect();
        assert_eq!(cpu.arch, std::env::consts::ARCH);
    }

    // ── S1.2: AVX-512 Detection ───────────────────────────────────────

    #[test]
    fn avx512_implies_avx2() {
        let cpu = CpuFeatures::detect();
        // AVX-512 requires AVX2 as a prerequisite
        if cpu.avx512f {
            assert!(cpu.avx2, "AVX-512F should imply AVX2");
        }
    }

    #[test]
    fn avx512bw_implies_avx512f() {
        let cpu = CpuFeatures::detect();
        if cpu.avx512bw {
            assert!(cpu.avx512f, "AVX-512BW should imply AVX-512F");
        }
    }

    // ── S1.3: AMX Detection ───────────────────────────────────────────

    #[test]
    fn amx_query_consistent() {
        let cpu = CpuFeatures::detect();
        let has = cpu.has_amx();
        assert_eq!(
            has,
            cpu.amx_bf16 || cpu.amx_int8 || cpu.amx_fp16,
            "has_amx() should match individual AMX flags"
        );
    }

    // ── S1.4: SSE/AVX Baseline ────────────────────────────────────────

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn x86_64_has_sse42() {
        // SSE4.2 is baseline for all modern x86_64 CPUs
        let cpu = CpuFeatures::detect();
        assert!(cpu.sse42, "SSE4.2 should be available on modern x86_64");
    }

    #[test]
    fn avx2_implies_sse42() {
        let cpu = CpuFeatures::detect();
        if cpu.avx2 {
            assert!(cpu.sse42, "AVX2 should imply SSE4.2");
        }
    }

    // ── S1.5: Feature Flags Struct ────────────────────────────────────

    #[test]
    fn default_features_all_false() {
        let cpu = CpuFeatures::default();
        assert!(!cpu.sse42);
        assert!(!cpu.avx2);
        assert!(!cpu.avx512f);
        assert!(!cpu.amx_bf16);
        assert!(!cpu.neon);
        assert!(!cpu.sve);
        assert!(!cpu.riscv_v);
        assert_eq!(cpu.vendor, CpuVendor::Unknown);
    }

    #[test]
    fn feature_list_reflects_detected_features() {
        let cpu = CpuFeatures::detect();
        let list = cpu.feature_list();
        // feature_list should have entries if we have features
        if cpu.sse42 {
            assert!(list.contains(&"SSE4.2"));
        }
        if cpu.avx2 {
            assert!(list.contains(&"AVX2"));
        }
        if cpu.avx512f {
            assert!(list.contains(&"AVX-512F"));
        }
    }

    // ── S1.6: ARM64 Feature Detection ─────────────────────────────────

    #[test]
    #[cfg(target_arch = "aarch64")]
    fn aarch64_has_neon() {
        // NEON is mandatory on AArch64
        let cpu = CpuFeatures::detect();
        assert!(cpu.neon, "NEON should be available on all AArch64");
    }

    // ── S1.7: RISC-V Feature Detection ────────────────────────────────

    #[test]
    fn riscv_vector_query_consistent() {
        let cpu = CpuFeatures::detect();
        assert_eq!(cpu.has_riscv_vector(), cpu.riscv_v);
    }

    // ── S1.8: Runtime Cache ───────────────────────────────────────────

    #[test]
    fn cached_returns_same_as_detect() {
        let detected = CpuFeatures::detect();
        let cached = CpuFeatures::cached();
        assert_eq!(detected.vendor, cached.vendor);
        assert_eq!(detected.model_name, cached.model_name);
        assert_eq!(detected.avx2, cached.avx2);
        assert_eq!(detected.avx512f, cached.avx512f);
    }

    #[test]
    fn cached_is_idempotent() {
        let a = CpuFeatures::cached();
        let b = CpuFeatures::cached();
        // Should return the same static reference
        assert!(std::ptr::eq(a, b), "cached() should return same reference");
    }

    // ── S1.9: Feature Query API ───────────────────────────────────────

    #[test]
    fn best_simd_width_reasonable() {
        let cpu = CpuFeatures::detect();
        let width = cpu.best_simd_width();
        assert!(
            width >= 64 && width <= 512,
            "SIMD width should be 64-512, got: {}",
            width
        );
    }

    #[test]
    fn has_avx512_consistent() {
        let cpu = CpuFeatures::detect();
        assert_eq!(cpu.has_avx512(), cpu.avx512f);
    }

    #[test]
    fn has_sve_consistent() {
        let cpu = CpuFeatures::detect();
        assert_eq!(cpu.has_sve(), cpu.sve || cpu.sve2);
    }

    // ── S1.10: Display and Serialization ──────────────────────────────

    #[test]
    fn display_info_contains_vendor() {
        let cpu = CpuFeatures::detect();
        let info = cpu.display_info();
        assert!(info.contains("Vendor:"));
        assert!(info.contains("Model:"));
        assert!(info.contains("Arch:"));
        assert!(info.contains("SIMD width:"));
    }

    #[test]
    fn vendor_display_format() {
        assert_eq!(format!("{}", CpuVendor::Intel), "Intel");
        assert_eq!(format!("{}", CpuVendor::Amd), "AMD");
        assert_eq!(format!("{}", CpuVendor::Arm), "ARM");
        assert_eq!(format!("{}", CpuVendor::RiscV), "RISC-V");
        assert_eq!(format!("{}", CpuVendor::Unknown), "Unknown");
    }

    #[test]
    fn cpu_features_serializable() {
        let cpu = CpuFeatures::detect();
        let json = serde_json::to_string(&cpu);
        assert!(json.is_ok(), "CpuFeatures should serialize to JSON");
        let json_str = json.expect("serialization works");
        assert!(json_str.contains("vendor"));
        assert!(json_str.contains("arch"));
    }

    #[test]
    fn feature_list_no_duplicates() {
        let cpu = CpuFeatures::detect();
        let list = cpu.feature_list();
        let mut seen = std::collections::HashSet::new();
        for f in &list {
            assert!(seen.insert(f), "duplicate feature in list: {}", f);
        }
    }
}
