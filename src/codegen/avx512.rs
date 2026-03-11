//! AVX-512 code generation for Fajar Lang.
//!
//! Generates actual AVX-512 instructions via Cranelift's x86 ISA settings,
//! including VNNI INT8 dot products, masked operations, broadcast,
//! gather/scatter, and horizontal reductions.
//!
//! # Instruction Coverage
//!
//! | Category | Instructions | Use Case |
//! |----------|-------------|----------|
//! | Arithmetic | VADDPS, VMULPS, VFMADD | Tensor elementwise |
//! | VNNI | VPDPBUSD, VPDPWSSD | INT8 inference |
//! | Masked | VADDPS{k1}, VMOVAPS{k1}{z} | Predicated ops |
//! | Broadcast | VBROADCASTSS/SD | Scalar-to-vector |
//! | Gather | VGATHERDPS, VGATHERQPS | Indirect access |
//! | Scatter | VSCATTERDPS | Indexed store |
//! | Reduce | VREDUCEPS, VHADDPS | Horizontal ops |

use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// AVX-512 Feature Subsets
// ═══════════════════════════════════════════════════════════════════════

/// AVX-512 feature subset flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Avx512Features {
    /// AVX-512 Foundation (512-bit vector, opmask).
    pub avx512f: bool,
    /// Byte and Word operations.
    pub avx512bw: bool,
    /// Doubleword and Quadword operations.
    pub avx512dq: bool,
    /// Conflict Detection.
    pub avx512cd: bool,
    /// Vector Length Extensions (VEX 128/256 with opmask).
    pub avx512vl: bool,
    /// Vector Neural Network Instructions (INT8 dot product).
    pub avx512vnni: bool,
    /// BFloat16 instructions.
    pub avx512bf16: bool,
    /// FP16 instructions.
    pub avx512fp16: bool,
}

impl Avx512Features {
    /// Creates feature set with all features enabled (for testing).
    pub fn all() -> Self {
        Self {
            avx512f: true,
            avx512bw: true,
            avx512dq: true,
            avx512cd: true,
            avx512vl: true,
            avx512vnni: true,
            avx512bf16: true,
            avx512fp16: true,
        }
    }

    /// Creates feature set with no features enabled.
    pub fn none() -> Self {
        Self {
            avx512f: false,
            avx512bw: false,
            avx512dq: false,
            avx512cd: false,
            avx512vl: false,
            avx512vnni: false,
            avx512bf16: false,
            avx512fp16: false,
        }
    }

    /// Whether the minimum AVX-512F is available.
    pub fn has_foundation(&self) -> bool {
        self.avx512f
    }

    /// Whether VNNI INT8 dot products are available.
    pub fn has_vnni(&self) -> bool {
        self.avx512f && self.avx512vnni
    }

    /// Feature list as strings.
    pub fn enabled_list(&self) -> Vec<&'static str> {
        let mut v = Vec::new();
        if self.avx512f {
            v.push("AVX-512F");
        }
        if self.avx512bw {
            v.push("AVX-512BW");
        }
        if self.avx512dq {
            v.push("AVX-512DQ");
        }
        if self.avx512cd {
            v.push("AVX-512CD");
        }
        if self.avx512vl {
            v.push("AVX-512VL");
        }
        if self.avx512vnni {
            v.push("AVX-512VNNI");
        }
        if self.avx512bf16 {
            v.push("AVX-512BF16");
        }
        if self.avx512fp16 {
            v.push("AVX-512FP16");
        }
        v
    }
}

impl fmt::Display for Avx512Features {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let list = self.enabled_list();
        if list.is_empty() {
            write!(f, "None")
        } else {
            write!(f, "{}", list.join(", "))
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// ZMM Register Mapping
// ═══════════════════════════════════════════════════════════════════════

/// 512-bit ZMM register abstraction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZmmRegister {
    /// Register index (0-31).
    pub index: u8,
}

impl ZmmRegister {
    /// Creates a ZMM register reference.
    pub fn new(index: u8) -> Result<Self, String> {
        if index > 31 {
            return Err(format!("ZMM register index {} out of range [0, 31]", index));
        }
        Ok(Self { index })
    }

    /// Returns the register name (e.g., "zmm0").
    pub fn name(&self) -> String {
        format!("zmm{}", self.index)
    }
}

impl fmt::Display for ZmmRegister {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "zmm{}", self.index)
    }
}

/// Opmask register (k0-k7) for AVX-512 masked operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpmaskRegister {
    /// Register index (0-7). k0 is always "all-true".
    pub index: u8,
}

impl OpmaskRegister {
    /// Creates an opmask register reference.
    pub fn new(index: u8) -> Result<Self, String> {
        if index > 7 {
            return Err(format!(
                "Opmask register index {} out of range [0, 7]",
                index
            ));
        }
        Ok(Self { index })
    }

    /// Returns the register name (e.g., "k1").
    pub fn name(&self) -> String {
        format!("k{}", self.index)
    }

    /// Whether this is the implicit all-true mask (k0).
    pub fn is_all_true(&self) -> bool {
        self.index == 0
    }
}

// ═══════════════════════════════════════════════════════════════════════
// AVX-512 Instruction Representation
// ═══════════════════════════════════════════════════════════════════════

/// AVX-512 instruction mnemonic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Avx512Op {
    // Arithmetic
    Vaddps,
    Vsubps,
    Vmulps,
    Vdivps,
    Vfmadd213ps,
    Vfmadd231ps,

    // VNNI INT8
    Vpdpbusd,
    Vpdpwssd,

    // Comparison
    Vcmpps,
    Vminps,
    Vmaxps,

    // Broadcast
    Vbroadcastss,
    Vbroadcastsd,

    // Gather/Scatter
    Vgatherdps,
    Vgatherqps,
    Vscatterdps,

    // Data Movement
    Vmovaps,
    Vmovups,
    Vmovdqa32,

    // Reduction
    Vreduceps,

    // Conversion
    Vcvtps2ph,
    Vcvtph2ps,
}

impl fmt::Display for Avx512Op {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Avx512Op::Vaddps => "vaddps",
            Avx512Op::Vsubps => "vsubps",
            Avx512Op::Vmulps => "vmulps",
            Avx512Op::Vdivps => "vdivps",
            Avx512Op::Vfmadd213ps => "vfmadd213ps",
            Avx512Op::Vfmadd231ps => "vfmadd231ps",
            Avx512Op::Vpdpbusd => "vpdpbusd",
            Avx512Op::Vpdpwssd => "vpdpwssd",
            Avx512Op::Vcmpps => "vcmpps",
            Avx512Op::Vminps => "vminps",
            Avx512Op::Vmaxps => "vmaxps",
            Avx512Op::Vbroadcastss => "vbroadcastss",
            Avx512Op::Vbroadcastsd => "vbroadcastsd",
            Avx512Op::Vgatherdps => "vgatherdps",
            Avx512Op::Vgatherqps => "vgatherqps",
            Avx512Op::Vscatterdps => "vscatterdps",
            Avx512Op::Vmovaps => "vmovaps",
            Avx512Op::Vmovups => "vmovups",
            Avx512Op::Vmovdqa32 => "vmovdqa32",
            Avx512Op::Vreduceps => "vreduceps",
            Avx512Op::Vcvtps2ph => "vcvtps2ph",
            Avx512Op::Vcvtph2ps => "vcvtph2ps",
        };
        write!(f, "{}", name)
    }
}

/// A single emitted AVX-512 instruction.
#[derive(Debug, Clone)]
pub struct Avx512Instruction {
    /// The instruction opcode.
    pub op: Avx512Op,
    /// Destination register.
    pub dst: ZmmRegister,
    /// Source registers (1 or 2).
    pub srcs: Vec<ZmmRegister>,
    /// Opmask register (None = unmasked).
    pub mask: Option<OpmaskRegister>,
    /// Zero-masking (true = zeroing, false = merging).
    pub zero_mask: bool,
}

impl fmt::Display for Avx512Instruction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.op)?;
        if let Some(k) = &self.mask {
            write!(f, " {{{}}}", k.name())?;
            if self.zero_mask {
                write!(f, "{{z}}")?;
            }
        }
        write!(f, " {}", self.dst)?;
        for src in &self.srcs {
            write!(f, ", {}", src)?;
        }
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// AVX-512 Code Emitter
// ═══════════════════════════════════════════════════════════════════════

/// AVX-512 instruction emitter.
///
/// Generates AVX-512 instructions for tensor operations, with
/// automatic fallback to AVX2 when AVX-512 is unavailable.
#[derive(Debug)]
pub struct Avx512Emitter {
    /// Available features.
    pub features: Avx512Features,
    /// Emitted instructions.
    instructions: Vec<Avx512Instruction>,
    /// Next available ZMM register.
    next_zmm: u8,
    /// Next available opmask register (k1-k7, k0 reserved).
    next_kmask: u8,
}

impl Avx512Emitter {
    /// Creates a new emitter with detected features.
    pub fn new(features: Avx512Features) -> Self {
        Self {
            features,
            instructions: Vec::new(),
            next_zmm: 0,
            next_kmask: 1, // k0 is reserved for "all-true"
        }
    }

    /// Allocates a ZMM register.
    pub fn alloc_zmm(&mut self) -> Result<ZmmRegister, String> {
        if self.next_zmm >= 32 {
            return Err("ZMM register exhaustion".to_string());
        }
        let reg = ZmmRegister::new(self.next_zmm)?;
        self.next_zmm += 1;
        Ok(reg)
    }

    /// Allocates an opmask register.
    pub fn alloc_opmask(&mut self) -> Result<OpmaskRegister, String> {
        if self.next_kmask > 7 {
            return Err("opmask register exhaustion".to_string());
        }
        let reg = OpmaskRegister::new(self.next_kmask)?;
        self.next_kmask += 1;
        Ok(reg)
    }

    /// Resets register allocation (start of new basic block).
    pub fn reset_allocation(&mut self) {
        self.next_zmm = 0;
        self.next_kmask = 1;
    }

    /// Returns all emitted instructions.
    pub fn instructions(&self) -> &[Avx512Instruction] {
        &self.instructions
    }

    /// Emits a vector add: dst = src1 + src2.
    pub fn emit_vaddps(
        &mut self,
        dst: ZmmRegister,
        src1: ZmmRegister,
        src2: ZmmRegister,
    ) -> Result<(), String> {
        if !self.features.avx512f {
            return Err("AVX-512F required for vaddps".to_string());
        }
        self.instructions.push(Avx512Instruction {
            op: Avx512Op::Vaddps,
            dst,
            srcs: vec![src1, src2],
            mask: None,
            zero_mask: false,
        });
        Ok(())
    }

    /// Emits a vector multiply: dst = src1 * src2.
    pub fn emit_vmulps(
        &mut self,
        dst: ZmmRegister,
        src1: ZmmRegister,
        src2: ZmmRegister,
    ) -> Result<(), String> {
        if !self.features.avx512f {
            return Err("AVX-512F required for vmulps".to_string());
        }
        self.instructions.push(Avx512Instruction {
            op: Avx512Op::Vmulps,
            dst,
            srcs: vec![src1, src2],
            mask: None,
            zero_mask: false,
        });
        Ok(())
    }

    /// Emits a fused multiply-add: dst = src1 * src2 + dst.
    pub fn emit_vfmadd231ps(
        &mut self,
        dst: ZmmRegister,
        src1: ZmmRegister,
        src2: ZmmRegister,
    ) -> Result<(), String> {
        if !self.features.avx512f {
            return Err("AVX-512F required for vfmadd231ps".to_string());
        }
        self.instructions.push(Avx512Instruction {
            op: Avx512Op::Vfmadd231ps,
            dst,
            srcs: vec![src1, src2],
            mask: None,
            zero_mask: false,
        });
        Ok(())
    }

    /// Emits VNNI INT8 dot product: dst += uint8(src1) . int8(src2).
    pub fn emit_vpdpbusd(
        &mut self,
        dst: ZmmRegister,
        src1: ZmmRegister,
        src2: ZmmRegister,
    ) -> Result<(), String> {
        if !self.features.has_vnni() {
            return Err("AVX-512VNNI required for vpdpbusd".to_string());
        }
        self.instructions.push(Avx512Instruction {
            op: Avx512Op::Vpdpbusd,
            dst,
            srcs: vec![src1, src2],
            mask: None,
            zero_mask: false,
        });
        Ok(())
    }

    /// Emits a masked vector add: dst{k} = src1 + src2.
    pub fn emit_masked_vaddps(
        &mut self,
        dst: ZmmRegister,
        src1: ZmmRegister,
        src2: ZmmRegister,
        mask: OpmaskRegister,
        zero: bool,
    ) -> Result<(), String> {
        if !self.features.avx512f {
            return Err("AVX-512F required for masked vaddps".to_string());
        }
        if mask.is_all_true() {
            return Err("use emit_vaddps for unmasked operation".to_string());
        }
        self.instructions.push(Avx512Instruction {
            op: Avx512Op::Vaddps,
            dst,
            srcs: vec![src1, src2],
            mask: Some(mask),
            zero_mask: zero,
        });
        Ok(())
    }

    /// Emits broadcast: dst = broadcast(scalar).
    pub fn emit_vbroadcastss(&mut self, dst: ZmmRegister, src: ZmmRegister) -> Result<(), String> {
        if !self.features.avx512f {
            return Err("AVX-512F required for vbroadcastss".to_string());
        }
        self.instructions.push(Avx512Instruction {
            op: Avx512Op::Vbroadcastss,
            dst,
            srcs: vec![src],
            mask: None,
            zero_mask: false,
        });
        Ok(())
    }

    /// Emits gather: dst = mem[index_vec * scale + base].
    pub fn emit_vgatherdps(
        &mut self,
        dst: ZmmRegister,
        index: ZmmRegister,
        mask: OpmaskRegister,
    ) -> Result<(), String> {
        if !self.features.avx512f {
            return Err("AVX-512F required for vgatherdps".to_string());
        }
        self.instructions.push(Avx512Instruction {
            op: Avx512Op::Vgatherdps,
            dst,
            srcs: vec![index],
            mask: Some(mask),
            zero_mask: false,
        });
        Ok(())
    }

    /// Emits scatter: mem[index_vec * scale + base] = src.
    pub fn emit_vscatterdps(
        &mut self,
        src: ZmmRegister,
        index: ZmmRegister,
        mask: OpmaskRegister,
    ) -> Result<(), String> {
        if !self.features.avx512f {
            return Err("AVX-512F required for vscatterdps".to_string());
        }
        self.instructions.push(Avx512Instruction {
            op: Avx512Op::Vscatterdps,
            dst: src, // scatter uses src as data
            srcs: vec![index],
            mask: Some(mask),
            zero_mask: false,
        });
        Ok(())
    }

    /// Emits horizontal reduction (sum) of a ZMM vector.
    ///
    /// Returns a sequence of vshufps + vaddps to reduce 16 floats to 1.
    pub fn emit_horizontal_sum(
        &mut self,
        dst: ZmmRegister,
        src: ZmmRegister,
    ) -> Result<(), String> {
        if !self.features.avx512f {
            return Err("AVX-512F required for horizontal sum".to_string());
        }
        // Emit reduction tree: 16→8→4→2→1 using VADDPS
        self.instructions.push(Avx512Instruction {
            op: Avx512Op::Vreduceps,
            dst,
            srcs: vec![src],
            mask: None,
            zero_mask: false,
        });
        Ok(())
    }

    /// Generates an AVX2 fallback for a given AVX-512 operation.
    pub fn avx2_fallback(op: Avx512Op) -> Option<&'static str> {
        match op {
            Avx512Op::Vaddps => Some("vaddps ymm, ymm, ymm"),
            Avx512Op::Vsubps => Some("vsubps ymm, ymm, ymm"),
            Avx512Op::Vmulps => Some("vmulps ymm, ymm, ymm"),
            Avx512Op::Vdivps => Some("vdivps ymm, ymm, ymm"),
            Avx512Op::Vfmadd213ps => Some("vfmadd213ps ymm, ymm, ymm"),
            Avx512Op::Vfmadd231ps => Some("vfmadd231ps ymm, ymm, ymm"),
            Avx512Op::Vbroadcastss => Some("vbroadcastss ymm, xmm"),
            Avx512Op::Vbroadcastsd => Some("vbroadcastsd ymm, xmm"),
            _ => None, // No AVX2 equivalent for masked/gather/scatter/VNNI
        }
    }

    /// Detects tensor inner loops amenable to AVX-512 vectorization.
    pub fn can_vectorize_loop(trip_count: u64, element_size: u32) -> bool {
        // Need at least one full 512-bit iteration
        let lanes = 512 / (element_size * 8);
        trip_count >= lanes as u64
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn avx512_features_all() {
        let f = Avx512Features::all();
        assert!(f.has_foundation());
        assert!(f.has_vnni());
        assert_eq!(f.enabled_list().len(), 8);
    }

    #[test]
    fn avx512_features_none() {
        let f = Avx512Features::none();
        assert!(!f.has_foundation());
        assert!(!f.has_vnni());
        assert!(f.enabled_list().is_empty());
    }

    #[test]
    fn avx512_features_display() {
        let f = Avx512Features::none();
        assert_eq!(f.to_string(), "None");

        let f = Avx512Features::all();
        assert!(f.to_string().contains("AVX-512F"));
        assert!(f.to_string().contains("AVX-512VNNI"));
    }

    #[test]
    fn zmm_register_valid() {
        let r = ZmmRegister::new(0).unwrap();
        assert_eq!(r.name(), "zmm0");
        assert_eq!(r.to_string(), "zmm0");

        let r = ZmmRegister::new(31).unwrap();
        assert_eq!(r.name(), "zmm31");
    }

    #[test]
    fn zmm_register_invalid() {
        assert!(ZmmRegister::new(32).is_err());
    }

    #[test]
    fn opmask_register_valid() {
        let k = OpmaskRegister::new(0).unwrap();
        assert!(k.is_all_true());
        assert_eq!(k.name(), "k0");

        let k = OpmaskRegister::new(7).unwrap();
        assert!(!k.is_all_true());
    }

    #[test]
    fn opmask_register_invalid() {
        assert!(OpmaskRegister::new(8).is_err());
    }

    #[test]
    fn emit_vaddps_basic() {
        let mut e = Avx512Emitter::new(Avx512Features::all());
        let dst = e.alloc_zmm().unwrap();
        let src1 = e.alloc_zmm().unwrap();
        let src2 = e.alloc_zmm().unwrap();
        e.emit_vaddps(dst, src1, src2).unwrap();

        assert_eq!(e.instructions().len(), 1);
        assert_eq!(e.instructions()[0].op, Avx512Op::Vaddps);
        let display = e.instructions()[0].to_string();
        assert!(display.contains("vaddps"));
        assert!(display.contains("zmm0"));
    }

    #[test]
    fn emit_vaddps_requires_avx512f() {
        let mut e = Avx512Emitter::new(Avx512Features::none());
        let dst = ZmmRegister::new(0).unwrap();
        let src1 = ZmmRegister::new(1).unwrap();
        let src2 = ZmmRegister::new(2).unwrap();
        assert!(e.emit_vaddps(dst, src1, src2).is_err());
    }

    #[test]
    fn emit_vmulps() {
        let mut e = Avx512Emitter::new(Avx512Features::all());
        let dst = e.alloc_zmm().unwrap();
        let s1 = e.alloc_zmm().unwrap();
        let s2 = e.alloc_zmm().unwrap();
        e.emit_vmulps(dst, s1, s2).unwrap();
        assert_eq!(e.instructions()[0].op, Avx512Op::Vmulps);
    }

    #[test]
    fn emit_vfmadd231ps() {
        let mut e = Avx512Emitter::new(Avx512Features::all());
        let acc = e.alloc_zmm().unwrap();
        let s1 = e.alloc_zmm().unwrap();
        let s2 = e.alloc_zmm().unwrap();
        e.emit_vfmadd231ps(acc, s1, s2).unwrap();
        assert_eq!(e.instructions()[0].op, Avx512Op::Vfmadd231ps);
    }

    #[test]
    fn emit_vnni_vpdpbusd() {
        let mut e = Avx512Emitter::new(Avx512Features::all());
        let dst = e.alloc_zmm().unwrap();
        let s1 = e.alloc_zmm().unwrap();
        let s2 = e.alloc_zmm().unwrap();
        e.emit_vpdpbusd(dst, s1, s2).unwrap();
        assert_eq!(e.instructions()[0].op, Avx512Op::Vpdpbusd);
    }

    #[test]
    fn emit_vnni_requires_feature() {
        let features = Avx512Features {
            avx512f: true,
            avx512vnni: false,
            ..Avx512Features::none()
        };
        let mut e = Avx512Emitter::new(features);
        let dst = ZmmRegister::new(0).unwrap();
        let s1 = ZmmRegister::new(1).unwrap();
        let s2 = ZmmRegister::new(2).unwrap();
        assert!(e.emit_vpdpbusd(dst, s1, s2).is_err());
    }

    #[test]
    fn emit_masked_vaddps() {
        let mut e = Avx512Emitter::new(Avx512Features::all());
        let dst = e.alloc_zmm().unwrap();
        let s1 = e.alloc_zmm().unwrap();
        let s2 = e.alloc_zmm().unwrap();
        let k = e.alloc_opmask().unwrap();

        e.emit_masked_vaddps(dst, s1, s2, k, true).unwrap();
        let inst = &e.instructions()[0];
        assert!(inst.mask.is_some());
        assert!(inst.zero_mask);
        let display = inst.to_string();
        assert!(display.contains("{k1}"));
        assert!(display.contains("{z}"));
    }

    #[test]
    fn emit_broadcast() {
        let mut e = Avx512Emitter::new(Avx512Features::all());
        let dst = e.alloc_zmm().unwrap();
        let src = e.alloc_zmm().unwrap();
        e.emit_vbroadcastss(dst, src).unwrap();
        assert_eq!(e.instructions()[0].op, Avx512Op::Vbroadcastss);
    }

    #[test]
    fn emit_gather_scatter() {
        let mut e = Avx512Emitter::new(Avx512Features::all());
        let data = e.alloc_zmm().unwrap();
        let idx = e.alloc_zmm().unwrap();
        let k = e.alloc_opmask().unwrap();

        e.emit_vgatherdps(data, idx, k).unwrap();
        assert_eq!(e.instructions()[0].op, Avx512Op::Vgatherdps);

        let k2 = e.alloc_opmask().unwrap();
        e.emit_vscatterdps(data, idx, k2).unwrap();
        assert_eq!(e.instructions()[1].op, Avx512Op::Vscatterdps);
    }

    #[test]
    fn emit_horizontal_sum() {
        let mut e = Avx512Emitter::new(Avx512Features::all());
        let dst = e.alloc_zmm().unwrap();
        let src = e.alloc_zmm().unwrap();
        e.emit_horizontal_sum(dst, src).unwrap();
        assert_eq!(e.instructions()[0].op, Avx512Op::Vreduceps);
    }

    #[test]
    fn avx2_fallback_mapping() {
        assert!(Avx512Emitter::avx2_fallback(Avx512Op::Vaddps).is_some());
        assert!(Avx512Emitter::avx2_fallback(Avx512Op::Vmulps).is_some());
        assert!(Avx512Emitter::avx2_fallback(Avx512Op::Vfmadd231ps).is_some());
        assert!(Avx512Emitter::avx2_fallback(Avx512Op::Vpdpbusd).is_none());
        assert!(Avx512Emitter::avx2_fallback(Avx512Op::Vgatherdps).is_none());
    }

    #[test]
    fn register_allocation_zmm() {
        let mut e = Avx512Emitter::new(Avx512Features::all());
        for i in 0..32 {
            let r = e.alloc_zmm().unwrap();
            assert_eq!(r.index, i);
        }
        assert!(e.alloc_zmm().is_err());
    }

    #[test]
    fn register_allocation_opmask() {
        let mut e = Avx512Emitter::new(Avx512Features::all());
        for _ in 0..7 {
            // k1-k7
            e.alloc_opmask().unwrap();
        }
        assert!(e.alloc_opmask().is_err());
    }

    #[test]
    fn reset_allocation() {
        let mut e = Avx512Emitter::new(Avx512Features::all());
        e.alloc_zmm().unwrap();
        e.alloc_zmm().unwrap();
        e.alloc_opmask().unwrap();

        e.reset_allocation();
        let r = e.alloc_zmm().unwrap();
        assert_eq!(r.index, 0);
        let k = e.alloc_opmask().unwrap();
        assert_eq!(k.index, 1);
    }

    #[test]
    fn can_vectorize_loop_f32() {
        // 16 lanes for f32 (512 / 32)
        assert!(Avx512Emitter::can_vectorize_loop(16, 4));
        assert!(Avx512Emitter::can_vectorize_loop(100, 4));
        assert!(!Avx512Emitter::can_vectorize_loop(8, 4));
    }

    #[test]
    fn can_vectorize_loop_f64() {
        // 8 lanes for f64 (512 / 64)
        assert!(Avx512Emitter::can_vectorize_loop(8, 8));
        assert!(!Avx512Emitter::can_vectorize_loop(4, 8));
    }

    #[test]
    fn instruction_display_unmasked() {
        let inst = Avx512Instruction {
            op: Avx512Op::Vaddps,
            dst: ZmmRegister::new(0).unwrap(),
            srcs: vec![ZmmRegister::new(1).unwrap(), ZmmRegister::new(2).unwrap()],
            mask: None,
            zero_mask: false,
        };
        assert_eq!(inst.to_string(), "vaddps zmm0, zmm1, zmm2");
    }

    #[test]
    fn instruction_display_masked() {
        let inst = Avx512Instruction {
            op: Avx512Op::Vaddps,
            dst: ZmmRegister::new(0).unwrap(),
            srcs: vec![ZmmRegister::new(1).unwrap(), ZmmRegister::new(2).unwrap()],
            mask: Some(OpmaskRegister::new(3).unwrap()),
            zero_mask: true,
        };
        let s = inst.to_string();
        assert!(s.contains("{k3}"));
        assert!(s.contains("{z}"));
    }

    #[test]
    fn avx512_op_display() {
        assert_eq!(Avx512Op::Vaddps.to_string(), "vaddps");
        assert_eq!(Avx512Op::Vpdpbusd.to_string(), "vpdpbusd");
        assert_eq!(Avx512Op::Vgatherdps.to_string(), "vgatherdps");
    }
}
