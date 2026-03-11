//! AVX10.2 and APX (Advanced Performance Extensions) code generation.
//!
//! # AVX10.2
//!
//! Converged AVX-512 for all x86 (client + server):
//! - 256-bit mandatory, 512-bit optional
//! - New comparison, conversion, and minmax instructions
//! - YMM promotion for efficient 256-bit SIMD
//!
//! # APX (Advanced Performance Extensions)
//!
//! - 32 General Purpose Registers (R16-R31)
//! - REX2 prefix for Extended GPR encoding
//! - NDD (Non-Destructive Destination) 3-operand forms
//! - Reduced register pressure for compute-heavy loops

use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// AVX10.2 Features
// ═══════════════════════════════════════════════════════════════════════

/// AVX10.2 feature set (CPUID leaf 24H).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Avx10Features {
    /// AVX10 version (0 = not supported, 1 = v1, 2 = v2).
    pub version: u8,
    /// Whether 256-bit vector length is supported (always true if version > 0).
    pub has_256bit: bool,
    /// Whether 512-bit vector length is supported (optional in AVX10).
    pub has_512bit: bool,
    /// New comparison instructions.
    pub new_compare: bool,
    /// New conversion instructions.
    pub new_convert: bool,
    /// New minmax instructions.
    pub new_minmax: bool,
    /// Saturating conversion instructions.
    pub sat_convert: bool,
}

impl Avx10Features {
    /// Full AVX10.2 with 512-bit support.
    pub fn v2_full() -> Self {
        Self {
            version: 2,
            has_256bit: true,
            has_512bit: true,
            new_compare: true,
            new_convert: true,
            new_minmax: true,
            sat_convert: true,
        }
    }

    /// AVX10.2 client (256-bit only, no 512-bit).
    pub fn v2_client() -> Self {
        Self {
            version: 2,
            has_256bit: true,
            has_512bit: false,
            new_compare: true,
            new_convert: true,
            new_minmax: true,
            sat_convert: true,
        }
    }

    /// No AVX10 support.
    pub fn none() -> Self {
        Self {
            version: 0,
            has_256bit: false,
            has_512bit: false,
            new_compare: false,
            new_convert: false,
            new_minmax: false,
            sat_convert: false,
        }
    }

    /// Whether AVX10 is available at all.
    pub fn is_available(&self) -> bool {
        self.version > 0
    }

    /// Whether AVX10 version 2 is available.
    pub fn is_v2(&self) -> bool {
        self.version >= 2
    }

    /// Best available vector width in bits.
    pub fn best_vector_width(&self) -> u32 {
        if self.has_512bit {
            512
        } else if self.has_256bit {
            256
        } else {
            0
        }
    }
}

impl fmt::Display for Avx10Features {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.version == 0 {
            return write!(f, "AVX10: None");
        }
        write!(
            f,
            "AVX10.{} ({}bit{})",
            self.version,
            if self.has_512bit { 512 } else { 256 },
            if self.new_minmax { ", MinMax" } else { "" }
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════
// AVX10.2 Instructions
// ═══════════════════════════════════════════════════════════════════════

/// AVX10.2 new instruction mnemonics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Avx10Op {
    // New comparison
    Vcomisbf16,
    Vucomisbf16,

    // New conversion
    Vcvt2ps2phx,
    Vcvtbf162ibs,
    Vcvtph2ibs,
    Vcvtps2ibs,

    // New minmax
    Vminmaxps,
    Vminmaxpd,
    Vminmaxph,

    // Saturating conversion
    Vcvttps2ibs,
    Vcvttpd2ibs,
}

impl fmt::Display for Avx10Op {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Avx10Op::Vcomisbf16 => "vcomisbf16",
            Avx10Op::Vucomisbf16 => "vucomisbf16",
            Avx10Op::Vcvt2ps2phx => "vcvt2ps2phx",
            Avx10Op::Vcvtbf162ibs => "vcvtbf162ibs",
            Avx10Op::Vcvtph2ibs => "vcvtph2ibs",
            Avx10Op::Vcvtps2ibs => "vcvtps2ibs",
            Avx10Op::Vminmaxps => "vminmaxps",
            Avx10Op::Vminmaxpd => "vminmaxpd",
            Avx10Op::Vminmaxph => "vminmaxph",
            Avx10Op::Vcvttps2ibs => "vcvttps2ibs",
            Avx10Op::Vcvttpd2ibs => "vcvttpd2ibs",
        };
        write!(f, "{}", name)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// APX (Advanced Performance Extensions)
// ═══════════════════════════════════════════════════════════════════════

/// APX feature set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ApxFeatures {
    /// APX is supported (32 GPRs available).
    pub supported: bool,
    /// NDD (Non-Destructive Destination) supported.
    pub ndd: bool,
    /// REX2 prefix encoding supported.
    pub rex2: bool,
    /// Number of available GPRs (16 or 32).
    pub gpr_count: u8,
}

impl ApxFeatures {
    /// Full APX with 32 GPRs.
    pub fn full() -> Self {
        Self {
            supported: true,
            ndd: true,
            rex2: true,
            gpr_count: 32,
        }
    }

    /// No APX (legacy 16 GPRs).
    pub fn none() -> Self {
        Self {
            supported: false,
            ndd: false,
            rex2: false,
            gpr_count: 16,
        }
    }

    /// Whether extended GPRs (R16-R31) are available.
    pub fn has_extended_gprs(&self) -> bool {
        self.supported && self.gpr_count > 16
    }

    /// Returns the number of extra GPRs available beyond the legacy 16.
    pub fn extra_gprs(&self) -> u8 {
        if self.supported {
            self.gpr_count.saturating_sub(16)
        } else {
            0
        }
    }
}

impl fmt::Display for ApxFeatures {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if !self.supported {
            write!(f, "APX: None (16 GPRs)")
        } else {
            write!(
                f,
                "APX: {} GPRs{}{}",
                self.gpr_count,
                if self.ndd { ", NDD" } else { "" },
                if self.rex2 { ", REX2" } else { "" }
            )
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Extended GPR
// ═══════════════════════════════════════════════════════════════════════

/// x86_64 General Purpose Register (extended to R0-R31 with APX).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Gpr {
    /// Register index (0-31).
    pub index: u8,
}

impl Gpr {
    /// Creates a GPR reference.
    pub fn new(index: u8) -> Result<Self, String> {
        if index > 31 {
            return Err(format!("GPR index {} out of range [0, 31]", index));
        }
        Ok(Self { index })
    }

    /// Whether this is an extended register (R16-R31, APX only).
    pub fn is_extended(&self) -> bool {
        self.index >= 16
    }

    /// Whether this register requires REX2 prefix encoding.
    pub fn needs_rex2(&self) -> bool {
        self.index >= 16
    }

    /// Returns the register name.
    pub fn name(&self) -> &'static str {
        match self.index {
            0 => "rax",
            1 => "rcx",
            2 => "rdx",
            3 => "rbx",
            4 => "rsp",
            5 => "rbp",
            6 => "rsi",
            7 => "rdi",
            8 => "r8",
            9 => "r9",
            10 => "r10",
            11 => "r11",
            12 => "r12",
            13 => "r13",
            14 => "r14",
            15 => "r15",
            16 => "r16",
            17 => "r17",
            18 => "r18",
            19 => "r19",
            20 => "r20",
            21 => "r21",
            22 => "r22",
            23 => "r23",
            24 => "r24",
            25 => "r25",
            26 => "r26",
            27 => "r27",
            28 => "r28",
            29 => "r29",
            30 => "r30",
            31 => "r31",
            _ => unreachable!(),
        }
    }
}

impl fmt::Display for Gpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// NDD (Non-Destructive Destination) Instructions
// ═══════════════════════════════════════════════════════════════════════

/// NDD instruction form: dst = src1 op src2 (3-operand, non-destructive).
#[derive(Debug, Clone)]
pub struct NddInstruction {
    /// Operation mnemonic.
    pub op: &'static str,
    /// Destination register (not overwritten by inputs).
    pub dst: Gpr,
    /// First source register.
    pub src1: Gpr,
    /// Second source register.
    pub src2: Gpr,
}

impl fmt::Display for NddInstruction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}, {}, {}", self.op, self.dst, self.src1, self.src2)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Register Allocator Extension
// ═══════════════════════════════════════════════════════════════════════

/// Extended register allocator for APX (32 GPRs).
#[derive(Debug)]
pub struct ExtendedRegAlloc {
    /// APX feature flags.
    pub features: ApxFeatures,
    /// Which registers are currently in use.
    used: [bool; 32],
    /// Total spills during allocation.
    pub spill_count: u32,
}

impl ExtendedRegAlloc {
    /// Creates a new register allocator.
    pub fn new(features: ApxFeatures) -> Self {
        let mut used = [false; 32];
        // RSP (4) and RBP (5) are reserved
        used[4] = true;
        used[5] = true;
        Self {
            features,
            used,
            spill_count: 0,
        }
    }

    /// Number of available GPRs.
    pub fn available_count(&self) -> u8 {
        self.features.gpr_count
    }

    /// Allocates a register.
    pub fn alloc(&mut self) -> Result<Gpr, String> {
        let max = self.features.gpr_count as usize;
        for i in 0..max {
            if !self.used[i] {
                self.used[i] = true;
                return Gpr::new(i as u8);
            }
        }
        self.spill_count += 1;
        Err("register spill required".to_string())
    }

    /// Frees a register.
    pub fn free(&mut self, reg: Gpr) {
        if reg.index != 4 && reg.index != 5 {
            // Don't free RSP/RBP
            self.used[reg.index as usize] = false;
        }
    }

    /// Number of currently used registers.
    pub fn used_count(&self) -> usize {
        self.used
            .iter()
            .take(self.features.gpr_count as usize)
            .filter(|&&u| u)
            .count()
    }

    /// Number of currently free registers.
    pub fn free_count(&self) -> usize {
        self.features.gpr_count as usize - self.used_count()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Combined Codegen for AVX10.2 + APX
// ═══════════════════════════════════════════════════════════════════════

/// Combined AVX10.2 + APX codegen context.
#[derive(Debug)]
pub struct Avx10ApxContext {
    /// AVX10 features.
    pub avx10: Avx10Features,
    /// APX features.
    pub apx: ApxFeatures,
    /// Register allocator.
    pub reg_alloc: ExtendedRegAlloc,
    /// YMM promotion enabled (use 256-bit ops when 512-bit unavailable).
    pub ymm_promotion: bool,
}

impl Avx10ApxContext {
    /// Creates a combined context.
    pub fn new(avx10: Avx10Features, apx: ApxFeatures) -> Self {
        Self {
            avx10,
            apx,
            reg_alloc: ExtendedRegAlloc::new(apx),
            ymm_promotion: !avx10.has_512bit && avx10.has_256bit,
        }
    }

    /// Returns the best vector width for this configuration.
    pub fn vector_width(&self) -> u32 {
        self.avx10.best_vector_width()
    }

    /// Whether YMM (256-bit) promotion is active.
    pub fn is_ymm_promoted(&self) -> bool {
        self.ymm_promotion
    }

    /// Summary of available extensions.
    pub fn summary(&self) -> String {
        format!(
            "{} | {} | Vector: {}bit | Regs: {}/{}",
            self.avx10,
            self.apx,
            self.vector_width(),
            self.reg_alloc.used_count(),
            self.reg_alloc.available_count()
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── AVX10.2 ──

    #[test]
    fn avx10_v2_full() {
        let f = Avx10Features::v2_full();
        assert!(f.is_available());
        assert!(f.is_v2());
        assert_eq!(f.best_vector_width(), 512);
    }

    #[test]
    fn avx10_v2_client() {
        let f = Avx10Features::v2_client();
        assert!(f.is_available());
        assert!(f.is_v2());
        assert_eq!(f.best_vector_width(), 256);
        assert!(!f.has_512bit);
    }

    #[test]
    fn avx10_none() {
        let f = Avx10Features::none();
        assert!(!f.is_available());
        assert_eq!(f.best_vector_width(), 0);
    }

    #[test]
    fn avx10_display() {
        let f = Avx10Features::v2_full();
        let s = f.to_string();
        assert!(s.contains("AVX10.2"));
        assert!(s.contains("512bit"));
    }

    #[test]
    fn avx10_op_display() {
        assert_eq!(Avx10Op::Vminmaxps.to_string(), "vminmaxps");
        assert_eq!(Avx10Op::Vcvt2ps2phx.to_string(), "vcvt2ps2phx");
        assert_eq!(Avx10Op::Vcomisbf16.to_string(), "vcomisbf16");
    }

    // ── APX ──

    #[test]
    fn apx_full() {
        let f = ApxFeatures::full();
        assert!(f.has_extended_gprs());
        assert_eq!(f.extra_gprs(), 16);
        assert_eq!(f.gpr_count, 32);
    }

    #[test]
    fn apx_none() {
        let f = ApxFeatures::none();
        assert!(!f.has_extended_gprs());
        assert_eq!(f.extra_gprs(), 0);
        assert_eq!(f.gpr_count, 16);
    }

    #[test]
    fn apx_display() {
        let f = ApxFeatures::full();
        let s = f.to_string();
        assert!(s.contains("32 GPRs"));
        assert!(s.contains("NDD"));
        assert!(s.contains("REX2"));
    }

    // ── GPR ──

    #[test]
    fn gpr_legacy_names() {
        assert_eq!(Gpr::new(0).unwrap().name(), "rax");
        assert_eq!(Gpr::new(1).unwrap().name(), "rcx");
        assert_eq!(Gpr::new(4).unwrap().name(), "rsp");
        assert_eq!(Gpr::new(15).unwrap().name(), "r15");
    }

    #[test]
    fn gpr_extended_names() {
        assert_eq!(Gpr::new(16).unwrap().name(), "r16");
        assert_eq!(Gpr::new(31).unwrap().name(), "r31");
    }

    #[test]
    fn gpr_invalid() {
        assert!(Gpr::new(32).is_err());
    }

    #[test]
    fn gpr_is_extended() {
        assert!(!Gpr::new(0).unwrap().is_extended());
        assert!(!Gpr::new(15).unwrap().is_extended());
        assert!(Gpr::new(16).unwrap().is_extended());
        assert!(Gpr::new(31).unwrap().is_extended());
    }

    #[test]
    fn gpr_needs_rex2() {
        assert!(!Gpr::new(0).unwrap().needs_rex2());
        assert!(Gpr::new(16).unwrap().needs_rex2());
    }

    // ── NDD ──

    #[test]
    fn ndd_instruction_display() {
        let inst = NddInstruction {
            op: "add",
            dst: Gpr::new(16).unwrap(),
            src1: Gpr::new(0).unwrap(),
            src2: Gpr::new(1).unwrap(),
        };
        assert_eq!(inst.to_string(), "add r16, rax, rcx");
    }

    // ── Register Allocator ──

    #[test]
    fn reg_alloc_legacy_16() {
        let mut alloc = ExtendedRegAlloc::new(ApxFeatures::none());
        assert_eq!(alloc.available_count(), 16);
        // 16 - 2 reserved (RSP, RBP) = 14 free
        assert_eq!(alloc.free_count(), 14);

        let mut allocated = Vec::new();
        for _ in 0..14 {
            allocated.push(alloc.alloc().unwrap());
        }
        assert!(alloc.alloc().is_err());
        assert_eq!(alloc.spill_count, 1);
    }

    #[test]
    fn reg_alloc_apx_32() {
        let mut alloc = ExtendedRegAlloc::new(ApxFeatures::full());
        assert_eq!(alloc.available_count(), 32);
        // 32 - 2 reserved = 30 free
        assert_eq!(alloc.free_count(), 30);

        for _ in 0..30 {
            alloc.alloc().unwrap();
        }
        assert!(alloc.alloc().is_err());
    }

    #[test]
    fn reg_alloc_free_and_reuse() {
        let mut alloc = ExtendedRegAlloc::new(ApxFeatures::none());
        let r1 = alloc.alloc().unwrap();
        let r2 = alloc.alloc().unwrap();
        let initial_free = alloc.free_count();

        alloc.free(r1);
        assert_eq!(alloc.free_count(), initial_free + 1);

        // Re-allocate should get the freed register
        let r3 = alloc.alloc().unwrap();
        assert_eq!(r3.index, r1.index);

        alloc.free(r2);
    }

    #[test]
    fn reg_alloc_cannot_free_rsp_rbp() {
        let mut alloc = ExtendedRegAlloc::new(ApxFeatures::none());
        let before = alloc.used_count();
        alloc.free(Gpr::new(4).unwrap()); // RSP
        alloc.free(Gpr::new(5).unwrap()); // RBP
        assert_eq!(alloc.used_count(), before);
    }

    #[test]
    fn reg_alloc_spill_reduction_with_apx() {
        // Simulate a workload needing 20 registers
        let mut legacy = ExtendedRegAlloc::new(ApxFeatures::none());
        let mut apx = ExtendedRegAlloc::new(ApxFeatures::full());

        for _ in 0..20 {
            let _ = legacy.alloc();
            let _ = apx.alloc();
        }

        // Legacy should have spills, APX should not
        assert!(legacy.spill_count > 0);
        assert_eq!(apx.spill_count, 0);
    }

    // ── Combined ──

    #[test]
    fn combined_avx10_apx_context() {
        let ctx = Avx10ApxContext::new(Avx10Features::v2_full(), ApxFeatures::full());
        assert_eq!(ctx.vector_width(), 512);
        assert!(!ctx.is_ymm_promoted());
        let summary = ctx.summary();
        assert!(summary.contains("AVX10.2"));
        assert!(summary.contains("32 GPRs"));
    }

    #[test]
    fn ymm_promotion_when_no_512bit() {
        let ctx = Avx10ApxContext::new(Avx10Features::v2_client(), ApxFeatures::full());
        assert!(ctx.is_ymm_promoted());
        assert_eq!(ctx.vector_width(), 256);
    }
}
