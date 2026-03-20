//! SIMD & Vectorization runtime for Fajar Lang.
//!
//! Phase 4 of v0.9 — provides simulated SIMD vector types, platform intrinsics,
//! SIMD-accelerated tensor operations, and an auto-vectorization analysis pass.
//!
//! All operations are *simulated* in portable Rust (no actual `#[target_feature]`
//! or `std::arch` intrinsics) so the module compiles and runs on every host.

use std::fmt;
use thiserror::Error;

// ═══════════════════════════════════════════════════════════════════════
// Error types
// ═══════════════════════════════════════════════════════════════════════

/// Errors from SIMD operations.
#[derive(Debug, Clone, Error, PartialEq)]
pub enum SimdError {
    /// Lane index is out of bounds for the vector width.
    #[error("SIMD lane index {index} out of bounds for width {width}")]
    LaneOutOfBounds {
        /// Requested lane index.
        index: usize,
        /// Vector width (number of lanes).
        width: usize,
    },

    /// Operand widths do not match.
    #[error("SIMD width mismatch: left has {left} lanes, right has {right}")]
    WidthMismatch {
        /// Left operand width.
        left: usize,
        /// Right operand width.
        right: usize,
    },

    /// Division by zero in a lane.
    #[error("SIMD division by zero in lane {lane}")]
    DivisionByZero {
        /// Lane that contained zero.
        lane: usize,
    },

    /// Shuffle index out of bounds.
    #[error("SIMD shuffle index {index} out of bounds for width {width}")]
    ShuffleOutOfBounds {
        /// Bad shuffle index value.
        index: usize,
        /// Vector width.
        width: usize,
    },

    /// Array length does not match expected vector width.
    #[error("SIMD array length {got} does not match vector width {expected}")]
    ArrayLengthMismatch {
        /// Expected length (vector width).
        expected: usize,
        /// Actual array length.
        got: usize,
    },

    /// Unsupported SIMD platform.
    #[error("unsupported SIMD platform: {0}")]
    UnsupportedPlatform(String),

    /// Alignment requirement not met.
    #[error("alignment {required} required, buffer aligned to {actual}")]
    AlignmentError {
        /// Required alignment in bytes.
        required: usize,
        /// Actual alignment.
        actual: usize,
    },

    /// Auto-vectorization analysis error.
    #[error("vectorization failed: {0}")]
    VectorizationError(String),
}

// ═══════════════════════════════════════════════════════════════════════
// Sprint 13 — SIMD Type System
// ═══════════════════════════════════════════════════════════════════════

/// Element types supported by SIMD vectors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SimdElementType {
    /// 8-bit signed integer.
    I8,
    /// 16-bit signed integer.
    I16,
    /// 32-bit signed integer.
    I32,
    /// 32-bit float.
    F32,
    /// 64-bit float.
    F64,
}

impl fmt::Display for SimdElementType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::I8 => write!(f, "i8"),
            Self::I16 => write!(f, "i16"),
            Self::I32 => write!(f, "i32"),
            Self::F32 => write!(f, "f32"),
            Self::F64 => write!(f, "f64"),
        }
    }
}

/// Catalogue of concrete SIMD vector types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SimdType {
    /// 4 lanes of `i32` (128-bit).
    I32x4,
    /// 4 lanes of `f32` (128-bit).
    F32x4,
    /// 8 lanes of `i32` (256-bit).
    I32x8,
    /// 8 lanes of `f32` (256-bit).
    F32x8,
    /// 2 lanes of `f64` (128-bit).
    F64x2,
    /// 4 lanes of `f64` (256-bit).
    F64x4,
    /// 16 lanes of `i8` (128-bit).
    I8x16,
    /// 8 lanes of `i16` (128-bit).
    I16x8,
}

impl SimdType {
    /// Number of lanes in this vector type.
    pub fn lane_count(self) -> usize {
        match self {
            Self::I32x4 | Self::F32x4 | Self::F64x4 => 4,
            Self::I32x8 | Self::F32x8 | Self::I16x8 => 8,
            Self::F64x2 => 2,
            Self::I8x16 => 16,
        }
    }

    /// Element type for this vector type.
    pub fn element_type(self) -> SimdElementType {
        match self {
            Self::I32x4 | Self::I32x8 => SimdElementType::I32,
            Self::F32x4 | Self::F32x8 => SimdElementType::F32,
            Self::F64x2 | Self::F64x4 => SimdElementType::F64,
            Self::I8x16 => SimdElementType::I8,
            Self::I16x8 => SimdElementType::I16,
        }
    }

    /// Total size of the vector in bytes.
    pub fn byte_size(self) -> usize {
        match self {
            Self::I32x4 | Self::F32x4 | Self::F64x2 | Self::I8x16 | Self::I16x8 => 16,
            Self::I32x8 | Self::F32x8 | Self::F64x4 => 32,
        }
    }
}

impl fmt::Display for SimdType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::I32x4 => write!(f, "I32x4"),
            Self::F32x4 => write!(f, "F32x4"),
            Self::I32x8 => write!(f, "I32x8"),
            Self::F32x8 => write!(f, "F32x8"),
            Self::F64x2 => write!(f, "F64x2"),
            Self::F64x4 => write!(f, "F64x4"),
            Self::I8x16 => write!(f, "I8x16"),
            Self::I16x8 => write!(f, "I16x8"),
        }
    }
}

// ── SimdVector<T, N> ────────────────────────────────────────────────

/// Generic SIMD vector holding `N` lanes of element type `T`.
///
/// This is a *simulation* — the backing store is a plain `Vec<f64>` so
/// that the same code path works on every host.  Concrete type aliases
/// (`I32x4`, `F32x4`, …) are provided below.
#[derive(Debug, Clone, PartialEq)]
pub struct SimdVector {
    /// Lane values stored as f64 (lossless for i32/f32/f64).
    lanes: Vec<f64>,
    /// The logical SIMD type.
    simd_type: SimdType,
}

impl SimdVector {
    /// Create a vector from an explicit lane array.
    ///
    /// Returns `Err` if `values.len()` does not equal the type's lane count.
    pub fn from_array(simd_type: SimdType, values: &[f64]) -> Result<Self, SimdError> {
        let width = simd_type.lane_count();
        if values.len() != width {
            return Err(SimdError::ArrayLengthMismatch {
                expected: width,
                got: values.len(),
            });
        }
        Ok(Self {
            lanes: values.to_vec(),
            simd_type,
        })
    }

    /// Create a vector with every lane set to `value`.
    pub fn splat(simd_type: SimdType, value: f64) -> Self {
        Self {
            lanes: vec![value; simd_type.lane_count()],
            simd_type,
        }
    }

    /// Create a zero vector.
    pub fn zero(simd_type: SimdType) -> Self {
        Self::splat(simd_type, 0.0)
    }

    /// Number of lanes.
    pub fn width(&self) -> usize {
        self.lanes.len()
    }

    /// The SIMD type.
    pub fn simd_type(&self) -> SimdType {
        self.simd_type
    }

    /// Read a single lane.
    pub fn get(&self, index: usize) -> Result<f64, SimdError> {
        if index >= self.lanes.len() {
            return Err(SimdError::LaneOutOfBounds {
                index,
                width: self.lanes.len(),
            });
        }
        Ok(self.lanes[index])
    }

    /// Write a single lane.
    pub fn set(&mut self, index: usize, value: f64) -> Result<(), SimdError> {
        if index >= self.lanes.len() {
            return Err(SimdError::LaneOutOfBounds {
                index,
                width: self.lanes.len(),
            });
        }
        self.lanes[index] = value;
        Ok(())
    }

    /// Raw lane slice.
    pub fn lanes(&self) -> &[f64] {
        &self.lanes
    }
}

// ── Lane-wise arithmetic ────────────────────────────────────────────

impl SimdVector {
    /// Lane-wise addition.
    pub fn add(&self, rhs: &Self) -> Result<Self, SimdError> {
        self.binary_op(rhs, |a, b| Ok(a + b))
    }

    /// Lane-wise subtraction.
    pub fn sub(&self, rhs: &Self) -> Result<Self, SimdError> {
        self.binary_op(rhs, |a, b| Ok(a - b))
    }

    /// Lane-wise multiplication.
    pub fn mul(&self, rhs: &Self) -> Result<Self, SimdError> {
        self.binary_op(rhs, |a, b| Ok(a * b))
    }

    /// Lane-wise division.
    pub fn div(&self, rhs: &Self) -> Result<Self, SimdError> {
        for (i, &v) in rhs.lanes.iter().enumerate() {
            if v == 0.0 {
                return Err(SimdError::DivisionByZero { lane: i });
            }
        }
        self.binary_op(rhs, |a, b| Ok(a / b))
    }

    /// Apply a binary operation lane-wise.
    fn binary_op<F>(&self, rhs: &Self, f: F) -> Result<Self, SimdError>
    where
        F: Fn(f64, f64) -> Result<f64, SimdError>,
    {
        if self.lanes.len() != rhs.lanes.len() {
            return Err(SimdError::WidthMismatch {
                left: self.lanes.len(),
                right: rhs.lanes.len(),
            });
        }
        let mut out = Vec::with_capacity(self.lanes.len());
        for (a, b) in self.lanes.iter().zip(rhs.lanes.iter()) {
            out.push(f(*a, *b)?);
        }
        Ok(Self {
            lanes: out,
            simd_type: self.simd_type,
        })
    }
}

// ── Comparison (returning MaskVector) ───────────────────────────────

/// Boolean mask vector — one bool per lane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaskVector {
    /// Per-lane mask values.
    bits: Vec<bool>,
}

impl MaskVector {
    /// Create a mask from a bool slice.
    pub fn from_bools(bits: &[bool]) -> Self {
        Self {
            bits: bits.to_vec(),
        }
    }

    /// Number of lanes.
    pub fn width(&self) -> usize {
        self.bits.len()
    }

    /// Read a single lane.
    pub fn get(&self, index: usize) -> Option<bool> {
        self.bits.get(index).copied()
    }

    /// Count `true` lanes.
    pub fn count_true(&self) -> usize {
        self.bits.iter().filter(|&&b| b).count()
    }

    /// Raw slice.
    pub fn bits(&self) -> &[bool] {
        &self.bits
    }

    /// Lane-wise AND.
    pub fn and(&self, rhs: &Self) -> Self {
        let bits = self
            .bits
            .iter()
            .zip(rhs.bits.iter())
            .map(|(&a, &b)| a && b)
            .collect();
        Self { bits }
    }

    /// Lane-wise OR.
    pub fn or(&self, rhs: &Self) -> Self {
        let bits = self
            .bits
            .iter()
            .zip(rhs.bits.iter())
            .map(|(&a, &b)| a || b)
            .collect();
        Self { bits }
    }

    /// Lane-wise NOT.
    pub fn not(&self) -> Self {
        Self {
            bits: self.bits.iter().map(|&b| !b).collect(),
        }
    }
}

impl SimdVector {
    /// Lane-wise equality comparison.
    pub fn cmp_eq(&self, rhs: &Self) -> Result<MaskVector, SimdError> {
        self.cmp_op(rhs, |a, b| (a - b).abs() < f64::EPSILON)
    }

    /// Lane-wise less-than comparison.
    pub fn cmp_lt(&self, rhs: &Self) -> Result<MaskVector, SimdError> {
        self.cmp_op(rhs, |a, b| a < b)
    }

    /// Lane-wise greater-than comparison.
    pub fn cmp_gt(&self, rhs: &Self) -> Result<MaskVector, SimdError> {
        self.cmp_op(rhs, |a, b| a > b)
    }

    /// Internal: apply a per-lane comparison.
    fn cmp_op<F>(&self, rhs: &Self, f: F) -> Result<MaskVector, SimdError>
    where
        F: Fn(f64, f64) -> bool,
    {
        if self.lanes.len() != rhs.lanes.len() {
            return Err(SimdError::WidthMismatch {
                left: self.lanes.len(),
                right: rhs.lanes.len(),
            });
        }
        let bits: Vec<bool> = self
            .lanes
            .iter()
            .zip(rhs.lanes.iter())
            .map(|(&a, &b)| f(a, b))
            .collect();
        Ok(MaskVector { bits })
    }
}

// ── Shuffle / Swizzle ───────────────────────────────────────────────

impl SimdVector {
    /// Reorder lanes according to `indices`.
    ///
    /// Each element of `indices` selects a lane from `self`.
    pub fn shuffle(&self, indices: &[usize]) -> Result<Self, SimdError> {
        if indices.len() != self.lanes.len() {
            return Err(SimdError::ArrayLengthMismatch {
                expected: self.lanes.len(),
                got: indices.len(),
            });
        }
        let mut out = Vec::with_capacity(indices.len());
        for &idx in indices {
            if idx >= self.lanes.len() {
                return Err(SimdError::ShuffleOutOfBounds {
                    index: idx,
                    width: self.lanes.len(),
                });
            }
            out.push(self.lanes[idx]);
        }
        Ok(Self {
            lanes: out,
            simd_type: self.simd_type,
        })
    }
}

// ── SIMD Capability Detection ───────────────────────────────────────

/// Runtime (simulated) SIMD capability flags.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimdCapability {
    /// x86 SSE available.
    pub has_sse: bool,
    /// x86 AVX available.
    pub has_avx: bool,
    /// x86 AVX-512 available.
    pub has_avx512: bool,
    /// ARM NEON available.
    pub has_neon: bool,
    /// ARM SVE available.
    pub has_sve: bool,
}

/// Detect SIMD capabilities for the current (simulated) host.
///
/// In simulation mode the returned capability reflects the *compile-time*
/// target: x86_64 gets SSE+AVX, aarch64 gets NEON, everything else Generic.
pub fn detect_simd_capabilities() -> SimdCapability {
    #[cfg(target_arch = "x86_64")]
    {
        SimdCapability {
            has_sse: true,
            has_avx: true,
            has_avx512: false,
            has_neon: false,
            has_sve: false,
        }
    }
    #[cfg(target_arch = "aarch64")]
    {
        SimdCapability {
            has_sse: false,
            has_avx: false,
            has_avx512: false,
            has_neon: true,
            has_sve: false,
        }
    }
    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    {
        SimdCapability {
            has_sse: false,
            has_avx: false,
            has_avx512: false,
            has_neon: false,
            has_sve: false,
        }
    }
}

impl fmt::Display for SimdCapability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut feats = Vec::new();
        if self.has_sse {
            feats.push("SSE");
        }
        if self.has_avx {
            feats.push("AVX");
        }
        if self.has_avx512 {
            feats.push("AVX-512");
        }
        if self.has_neon {
            feats.push("NEON");
        }
        if self.has_sve {
            feats.push("SVE");
        }
        if feats.is_empty() {
            write!(f, "none")
        } else {
            write!(f, "{}", feats.join(", "))
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Sprint 14 — Platform SIMD Intrinsics
// ═══════════════════════════════════════════════════════════════════════

/// Target SIMD platform.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SimdPlatform {
    /// x86 SSE (128-bit).
    X86Sse,
    /// x86 AVX (256-bit).
    X86Avx,
    /// x86 AVX-512 (512-bit).
    X86Avx512,
    /// ARM NEON (128-bit).
    ArmNeon,
    /// ARM SVE (scalable).
    ArmSve,
    /// RISC-V V extension (variable length).
    RiscvV,
    /// Portable scalar fallback.
    Generic,
}

impl fmt::Display for SimdPlatform {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::X86Sse => write!(f, "x86-SSE"),
            Self::X86Avx => write!(f, "x86-AVX"),
            Self::X86Avx512 => write!(f, "x86-AVX512"),
            Self::ArmNeon => write!(f, "ARM-NEON"),
            Self::ArmSve => write!(f, "ARM-SVE"),
            Self::RiscvV => write!(f, "RISC-V V"),
            Self::Generic => write!(f, "generic"),
        }
    }
}

/// Annotation hint for auto-vectorization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimdAnnotation {
    /// Preferred platform (or `None` for auto-selection).
    pub platform: Option<SimdPlatform>,
    /// Preferred vector width in lanes (or `None` for auto).
    pub width: Option<usize>,
    /// Whether to force vectorization even if the cost model says no.
    pub force: bool,
}

impl SimdAnnotation {
    /// Default `@simd` annotation — auto everything.
    pub fn auto() -> Self {
        Self {
            platform: None,
            width: None,
            force: false,
        }
    }

    /// Annotation targeting a specific platform.
    pub fn with_platform(platform: SimdPlatform) -> Self {
        Self {
            platform: Some(platform),
            width: None,
            force: false,
        }
    }
}

/// Platform-specific SIMD intrinsic operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SimdIntrinsic {
    // x86 SSE (128-bit, F32x4)
    /// `_mm_add_ps` — packed f32 add.
    MmAddPs,
    /// `_mm_mul_ps` — packed f32 multiply.
    MmMulPs,
    /// `_mm_shuffle_ps` — packed f32 shuffle.
    MmShufflePs,

    // x86 AVX (256-bit, F32x8)
    /// `_mm256_add_ps` — packed f32 add (256-bit).
    Mm256AddPs,
    /// `_mm256_fmadd_ps` — fused multiply-add (256-bit).
    Mm256FmaddPs,

    // x86 AVX-512 (512-bit)
    /// `_mm512_add_ps` — packed f32 add (512-bit).
    Mm512AddPs,
    /// AVX-512 masked operation.
    Mm512MaskOp,

    // ARM NEON (128-bit)
    /// `vaddq_f32` — NEON packed f32 add.
    VaddqF32,
    /// `vmulq_f32` — NEON packed f32 mul.
    VmulqF32,
    /// `vld1q_f32` — NEON vector load.
    Vld1qF32,

    // ARM SVE (scalable)
    /// SVE predicated add.
    SveAdd,

    // RISC-V V
    /// `vfadd` — RVV floating add.
    RvvFadd,
    /// `vfmul` — RVV floating multiply.
    RvvFmul,
}

impl SimdIntrinsic {
    /// The platform that owns this intrinsic.
    pub fn platform(self) -> SimdPlatform {
        match self {
            Self::MmAddPs | Self::MmMulPs | Self::MmShufflePs => SimdPlatform::X86Sse,
            Self::Mm256AddPs | Self::Mm256FmaddPs => SimdPlatform::X86Avx,
            Self::Mm512AddPs | Self::Mm512MaskOp => SimdPlatform::X86Avx512,
            Self::VaddqF32 | Self::VmulqF32 | Self::Vld1qF32 => SimdPlatform::ArmNeon,
            Self::SveAdd => SimdPlatform::ArmSve,
            Self::RvvFadd | Self::RvvFmul => SimdPlatform::RiscvV,
        }
    }
}

// ── x86 SSE simulation ─────────────────────────────────────────────

/// Simulate `_mm_add_ps`: lane-wise f32 add on `F32x4`.
pub fn mm_add_ps(a: &SimdVector, b: &SimdVector) -> Result<SimdVector, SimdError> {
    check_type(a, SimdType::F32x4)?;
    check_type(b, SimdType::F32x4)?;
    a.add(b)
}

/// Simulate `_mm_mul_ps`: lane-wise f32 mul on `F32x4`.
pub fn mm_mul_ps(a: &SimdVector, b: &SimdVector) -> Result<SimdVector, SimdError> {
    check_type(a, SimdType::F32x4)?;
    check_type(b, SimdType::F32x4)?;
    a.mul(b)
}

/// Simulate `_mm_shuffle_ps` with a 4-element index array.
pub fn mm_shuffle_ps(a: &SimdVector, indices: &[usize; 4]) -> Result<SimdVector, SimdError> {
    check_type(a, SimdType::F32x4)?;
    a.shuffle(indices)
}

// ── x86 AVX simulation ─────────────────────────────────────────────

/// Simulate `_mm256_add_ps`: lane-wise f32 add on `F32x8`.
pub fn mm256_add_ps(a: &SimdVector, b: &SimdVector) -> Result<SimdVector, SimdError> {
    check_type(a, SimdType::F32x8)?;
    check_type(b, SimdType::F32x8)?;
    a.add(b)
}

/// Simulate `_mm256_fmadd_ps`: fused multiply-add `a * b + c` on `F32x8`.
pub fn mm256_fmadd_ps(
    a: &SimdVector,
    b: &SimdVector,
    c: &SimdVector,
) -> Result<SimdVector, SimdError> {
    check_type(a, SimdType::F32x8)?;
    check_type(b, SimdType::F32x8)?;
    check_type(c, SimdType::F32x8)?;
    let ab = a.mul(b)?;
    ab.add(c)
}

// ── x86 AVX-512 simulation ─────────────────────────────────────────

/// Simulate `_mm512_add_ps`: lane-wise f32 add on 16 lanes.
pub fn mm512_add_ps(a: &[f32; 16], b: &[f32; 16]) -> [f32; 16] {
    let mut out = [0.0f32; 16];
    for i in 0..16 {
        out[i] = a[i] + b[i];
    }
    out
}

/// Simulate an AVX-512 masked operation: only lanes where `mask` is true
/// receive the result of `a + b`; other lanes keep the value from `a`.
pub fn mm512_mask_add(a: &[f32; 16], b: &[f32; 16], mask: &[bool; 16]) -> [f32; 16] {
    let mut out = *a;
    for i in 0..16 {
        if mask[i] {
            out[i] = a[i] + b[i];
        }
    }
    out
}

// ── ARM NEON simulation ─────────────────────────────────────────────

/// Simulate `vaddq_f32`: NEON lane-wise f32 add on `F32x4`.
pub fn vaddq_f32(a: &SimdVector, b: &SimdVector) -> Result<SimdVector, SimdError> {
    check_type(a, SimdType::F32x4)?;
    check_type(b, SimdType::F32x4)?;
    a.add(b)
}

/// Simulate `vmulq_f32`: NEON lane-wise f32 mul on `F32x4`.
pub fn vmulq_f32(a: &SimdVector, b: &SimdVector) -> Result<SimdVector, SimdError> {
    check_type(a, SimdType::F32x4)?;
    check_type(b, SimdType::F32x4)?;
    a.mul(b)
}

/// Simulate `vld1q_f32`: load 4 f32 values into an `F32x4` vector.
pub fn vld1q_f32(data: &[f32; 4]) -> SimdVector {
    SimdVector {
        lanes: data.iter().map(|&v| f64::from(v)).collect(),
        simd_type: SimdType::F32x4,
    }
}

// ── ARM SVE simulation ──────────────────────────────────────────────

/// Simulate a predicated SVE vector add.
///
/// `vl` is the *scalable vector length* (number of f32 lanes available).
/// Only lanes where `pred[i]` is true are computed.
pub fn sve_add_predicated(
    a: &[f32],
    b: &[f32],
    pred: &[bool],
    vl: usize,
) -> Result<Vec<f32>, SimdError> {
    if a.len() < vl || b.len() < vl || pred.len() < vl {
        return Err(SimdError::ArrayLengthMismatch {
            expected: vl,
            got: a.len().min(b.len()).min(pred.len()),
        });
    }
    let mut out = vec![0.0f32; vl];
    for i in 0..vl {
        out[i] = if pred[i] { a[i] + b[i] } else { a[i] };
    }
    Ok(out)
}

// ── RISC-V V simulation ────────────────────────────────────────────

/// Simulate `vfadd.vv`: RISC-V V floating add with variable vector length.
pub fn rvv_fadd(a: &[f32], b: &[f32], vl: usize) -> Result<Vec<f32>, SimdError> {
    if a.len() < vl || b.len() < vl {
        return Err(SimdError::ArrayLengthMismatch {
            expected: vl,
            got: a.len().min(b.len()),
        });
    }
    Ok((0..vl).map(|i| a[i] + b[i]).collect())
}

/// Simulate `vfmul.vv`: RISC-V V floating mul with variable vector length.
pub fn rvv_fmul(a: &[f32], b: &[f32], vl: usize) -> Result<Vec<f32>, SimdError> {
    if a.len() < vl || b.len() < vl {
        return Err(SimdError::ArrayLengthMismatch {
            expected: vl,
            got: a.len().min(b.len()),
        });
    }
    Ok((0..vl).map(|i| a[i] * b[i]).collect())
}

// ── Aligned buffer ──────────────────────────────────────────────────

/// A simulated aligned memory buffer for SIMD loads/stores.
///
/// Real alignment would use `std::alloc::Layout` — here we simply
/// record the *intended* alignment and store data in a `Vec<u8>`.
#[derive(Debug, Clone)]
pub struct AlignedBuffer {
    /// Requested alignment in bytes (16, 32, or 64).
    alignment: usize,
    /// Raw data bytes.
    data: Vec<u8>,
}

impl AlignedBuffer {
    /// Create a new buffer of `size` bytes at the requested alignment.
    ///
    /// Returns `Err` if `alignment` is not a power of two or less than 16.
    pub fn new(size: usize, alignment: usize) -> Result<Self, SimdError> {
        if !alignment.is_power_of_two() || alignment < 16 {
            return Err(SimdError::AlignmentError {
                required: 16,
                actual: alignment,
            });
        }
        Ok(Self {
            alignment,
            data: vec![0u8; size],
        })
    }

    /// Alignment in bytes.
    pub fn alignment(&self) -> usize {
        self.alignment
    }

    /// Buffer size in bytes.
    pub fn size(&self) -> usize {
        self.data.len()
    }

    /// Read the raw data slice.
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Write data into the buffer.
    pub fn write(&mut self, offset: usize, src: &[u8]) -> Result<(), SimdError> {
        if offset + src.len() > self.data.len() {
            return Err(SimdError::ArrayLengthMismatch {
                expected: self.data.len(),
                got: offset + src.len(),
            });
        }
        self.data[offset..offset + src.len()].copy_from_slice(src);
        Ok(())
    }
}

/// Helper: assert that a vector has the expected `SimdType`.
fn check_type(v: &SimdVector, expected: SimdType) -> Result<(), SimdError> {
    if v.simd_type() != expected {
        return Err(SimdError::UnsupportedPlatform(format!(
            "expected {expected}, got {}",
            v.simd_type()
        )));
    }
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// Sprint 15 — SIMD Tensor Operations
// ═══════════════════════════════════════════════════════════════════════

/// SIMD-accelerated 4x4 block matrix multiply (tile-based).
///
/// Multiplies two row-major `4x4` matrices stored as 16-element f32 slices
/// using `F32x4` vectors for the inner products.
pub fn simd_matmul_4x4(a: &[f32; 16], b: &[f32; 16]) -> [f32; 16] {
    let mut out = [0.0f32; 16];
    for row in 0..4 {
        for col in 0..4 {
            let mut sum = 0.0f32;
            for k in 0..4 {
                sum += a[row * 4 + k] * b[k * 4 + col];
            }
            out[row * 4 + col] = sum;
        }
    }
    out
}

/// SIMD-accelerated element-wise addition of f32 slices.
///
/// Processes 4 elements at a time using simulated `F32x4`.
pub fn simd_elementwise_add(a: &[f32], b: &[f32]) -> Result<Vec<f32>, SimdError> {
    if a.len() != b.len() {
        return Err(SimdError::WidthMismatch {
            left: a.len(),
            right: b.len(),
        });
    }
    let mut out = Vec::with_capacity(a.len());
    let chunks = a.len() / 4;
    for i in 0..chunks {
        let base = i * 4;
        for j in 0..4 {
            out.push(a[base + j] + b[base + j]);
        }
    }
    // scalar epilogue
    for i in (chunks * 4)..a.len() {
        out.push(a[i] + b[i]);
    }
    Ok(out)
}

/// SIMD-accelerated element-wise multiplication of f32 slices.
pub fn simd_elementwise_mul(a: &[f32], b: &[f32]) -> Result<Vec<f32>, SimdError> {
    if a.len() != b.len() {
        return Err(SimdError::WidthMismatch {
            left: a.len(),
            right: b.len(),
        });
    }
    let mut out = Vec::with_capacity(a.len());
    for (x, y) in a.iter().zip(b.iter()) {
        out.push(x * y);
    }
    Ok(out)
}

/// Horizontal sum across all elements (SIMD reduction).
pub fn simd_horizontal_sum(data: &[f32]) -> f32 {
    let mut sum = 0.0f32;
    for &v in data {
        sum += v;
    }
    sum
}

/// Horizontal max across all elements (SIMD reduction).
pub fn simd_horizontal_max(data: &[f32]) -> Option<f32> {
    if data.is_empty() {
        return None;
    }
    let mut mx = data[0];
    for &v in &data[1..] {
        if v > mx {
            mx = v;
        }
    }
    Some(mx)
}

/// Horizontal min across all elements (SIMD reduction).
pub fn simd_horizontal_min(data: &[f32]) -> Option<f32> {
    if data.is_empty() {
        return None;
    }
    let mut mn = data[0];
    for &v in &data[1..] {
        if v < mn {
            mn = v;
        }
    }
    Some(mn)
}

/// Vectorized ReLU: `max(0, x)` for each element.
pub fn simd_relu(data: &[f32]) -> Vec<f32> {
    data.iter()
        .map(|&x| if x > 0.0 { x } else { 0.0 })
        .collect()
}

/// Vectorized sigmoid approximation: `1 / (1 + exp(-x))`.
///
/// Uses a rational approximation for speed.
pub fn simd_sigmoid(data: &[f32]) -> Vec<f32> {
    data.iter()
        .map(|&x| {
            let x64 = f64::from(x);
            let s = 1.0 / (1.0 + (-x64).exp());
            s as f32
        })
        .collect()
}

/// Vectorized tanh approximation.
pub fn simd_tanh(data: &[f32]) -> Vec<f32> {
    data.iter()
        .map(|&x| {
            let x64 = f64::from(x);
            x64.tanh() as f32
        })
        .collect()
}

/// Fast vectorized exp approximation (Schraudolph's method concept).
///
/// Falls back to `f64::exp` cast to f32 for correctness in simulation.
fn fast_exp_f32(x: f32) -> f32 {
    let x64 = f64::from(x);
    x64.exp() as f32
}

/// Vectorized softmax: `exp(x_i) / sum(exp(x_j))`.
pub fn simd_softmax(data: &[f32]) -> Vec<f32> {
    if data.is_empty() {
        return Vec::new();
    }
    // numerical stability: subtract max
    let max_val = simd_horizontal_max(data).unwrap_or(0.0);
    let exps: Vec<f32> = data.iter().map(|&x| fast_exp_f32(x - max_val)).collect();
    let sum: f32 = simd_horizontal_sum(&exps);
    if sum == 0.0 {
        return vec![0.0; data.len()];
    }
    exps.iter().map(|&e| e / sum).collect()
}

/// SIMD dot product: multiply corresponding elements then sum.
pub fn simd_dot_product(a: &[f32], b: &[f32]) -> Result<f32, SimdError> {
    if a.len() != b.len() {
        return Err(SimdError::WidthMismatch {
            left: a.len(),
            right: b.len(),
        });
    }
    let products: Vec<f32> = a.iter().zip(b.iter()).map(|(&x, &y)| x * y).collect();
    Ok(simd_horizontal_sum(&products))
}

/// SIMD 1-D convolution with a kernel (sliding window).
///
/// Output length = `input.len() - kernel.len() + 1`.
pub fn simd_conv1d(input: &[f32], kernel: &[f32]) -> Result<Vec<f32>, SimdError> {
    if kernel.is_empty() || kernel.len() > input.len() {
        return Err(SimdError::ArrayLengthMismatch {
            expected: input.len(),
            got: kernel.len(),
        });
    }
    let out_len = input.len() - kernel.len() + 1;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let mut acc = 0.0f32;
        for (j, &k) in kernel.iter().enumerate() {
            acc += input[i + j] * k;
        }
        out.push(acc);
    }
    Ok(out)
}

/// Vectorized f32-to-i8 quantization with rounding and clamping.
///
/// `scale` maps the float range to [-128, 127].
pub fn simd_quantize_f32_to_i8(data: &[f32], scale: f32) -> Vec<i8> {
    data.iter()
        .map(|&x| {
            let scaled = x * scale;
            let rounded = scaled.round();
            let clamped = rounded.clamp(-128.0, 127.0);
            clamped as i8
        })
        .collect()
}

// ── SIMD Benchmark ──────────────────────────────────────────────────

/// Result of a scalar-vs-SIMD micro-benchmark.
#[derive(Debug, Clone, PartialEq)]
pub struct BenchmarkResult {
    /// Name of the operation benchmarked.
    pub operation: String,
    /// Number of elements processed.
    pub element_count: usize,
    /// Scalar wall-clock time in nanoseconds (simulated).
    pub scalar_ns: u64,
    /// SIMD wall-clock time in nanoseconds (simulated).
    pub simd_ns: u64,
    /// Speedup factor (scalar / simd).
    pub speedup: f64,
}

impl fmt::Display for BenchmarkResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: {}x speedup ({} ns scalar, {} ns simd, {} elements)",
            self.operation, self.speedup, self.scalar_ns, self.simd_ns, self.element_count
        )
    }
}

/// A collector for SIMD benchmark results.
#[derive(Debug, Clone)]
pub struct SimdBenchmark {
    /// Collected results.
    results: Vec<BenchmarkResult>,
}

impl SimdBenchmark {
    /// Create a new empty benchmark collector.
    pub fn new() -> Self {
        Self {
            results: Vec::new(),
        }
    }

    /// Add a result.
    pub fn record(&mut self, result: BenchmarkResult) {
        self.results.push(result);
    }

    /// All recorded results.
    pub fn results(&self) -> &[BenchmarkResult] {
        &self.results
    }

    /// Average speedup across all recorded results.
    pub fn average_speedup(&self) -> f64 {
        if self.results.is_empty() {
            return 1.0;
        }
        let sum: f64 = self.results.iter().map(|r| r.speedup).sum();
        sum / self.results.len() as f64
    }
}

impl Default for SimdBenchmark {
    fn default() -> Self {
        Self::new()
    }
}

/// Run a simulated scalar-vs-SIMD benchmark for a given operation.
///
/// `op` names the operation, `size` is the number of f32 elements.
/// The simulated speedup uses a heuristic: 4x for SSE-width ops.
pub fn run_simd_benchmark(op: &str, size: usize) -> BenchmarkResult {
    // Simulate: scalar takes `size * 5` ns, SIMD takes `size * 5 / 4` ns
    let scalar_ns = (size as u64).saturating_mul(5);
    let simd_ns = (size as u64).saturating_mul(5) / 4;
    let speedup = if simd_ns == 0 {
        1.0
    } else {
        scalar_ns as f64 / simd_ns as f64
    };
    BenchmarkResult {
        operation: op.to_string(),
        element_count: size,
        scalar_ns,
        simd_ns,
        speedup,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Sprint 16 — Auto-Vectorization
// ═══════════════════════════════════════════════════════════════════════

/// Describes a loop body operation for vectorization analysis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoopOp {
    /// `out[i] = a[i] + b[i]`
    Add,
    /// `out[i] = a[i] - b[i]`
    Sub,
    /// `out[i] = a[i] * b[i]`
    Mul,
    /// `out[i] = a[i] / b[i]`
    Div,
    /// `acc += a[i]` (reduction)
    ReduceAdd,
    /// `acc *= a[i]` (reduction)
    ReduceMul,
    /// `acc = max(acc, a[i])` (reduction)
    ReduceMax,
    /// `acc = min(acc, a[i])` (reduction)
    ReduceMin,
    /// Conditional: `if pred[i] { op }` — partially vectorizable with mask.
    Conditional,
    /// Unknown / opaque operation — not vectorizable.
    Unknown,
}

impl fmt::Display for LoopOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Add => write!(f, "add"),
            Self::Sub => write!(f, "sub"),
            Self::Mul => write!(f, "mul"),
            Self::Div => write!(f, "div"),
            Self::ReduceAdd => write!(f, "reduce_add"),
            Self::ReduceMul => write!(f, "reduce_mul"),
            Self::ReduceMax => write!(f, "reduce_max"),
            Self::ReduceMin => write!(f, "reduce_min"),
            Self::Conditional => write!(f, "conditional"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

/// Information about a loop extracted during analysis.
#[derive(Debug, Clone, PartialEq)]
pub struct LoopInfo {
    /// Name of the induction variable (e.g., `"i"`).
    pub induction_var: String,
    /// Lower bound (inclusive).
    pub lower_bound: usize,
    /// Upper bound (exclusive).
    pub upper_bound: usize,
    /// Operations detected in the loop body.
    pub body_ops: Vec<LoopOp>,
    /// Whether the loop carries a data dependency across iterations.
    pub has_carried_dep: bool,
}

impl LoopInfo {
    /// Trip count (number of iterations).
    pub fn trip_count(&self) -> usize {
        self.upper_bound.saturating_sub(self.lower_bound)
    }

    /// Whether all body operations are vectorizable.
    pub fn is_vectorizable(&self) -> bool {
        if self.body_ops.is_empty() {
            return false;
        }
        self.body_ops
            .iter()
            .all(|op| !matches!(op, LoopOp::Unknown))
    }

    /// Whether the loop contains only reduction operations.
    pub fn is_reduction(&self) -> bool {
        if self.body_ops.is_empty() {
            return false;
        }
        self.body_ops.iter().all(|op| {
            matches!(
                op,
                LoopOp::ReduceAdd | LoopOp::ReduceMul | LoopOp::ReduceMax | LoopOp::ReduceMin
            )
        })
    }
}

/// Analyse a described loop body and produce a `LoopInfo`.
///
/// In a real compiler this would walk the AST/IR. Here, the caller
/// provides the structural information directly.
pub fn analyze_loop(
    var: &str,
    lower: usize,
    upper: usize,
    ops: Vec<LoopOp>,
    carried_dep: bool,
) -> LoopInfo {
    LoopInfo {
        induction_var: var.to_string(),
        lower_bound: lower,
        upper_bound: upper,
        body_ops: ops,
        has_carried_dep: carried_dep,
    }
}

/// A planned vectorization transformation.
#[derive(Debug, Clone, PartialEq)]
pub struct VectorizationPlan {
    /// The loop this plan applies to.
    pub loop_info: LoopInfo,
    /// Scalar width (always 1).
    pub scalar_width: usize,
    /// Chosen SIMD vector width in lanes.
    pub vector_width: usize,
    /// Number of prologue scalar iterations (alignment).
    pub prologue_count: usize,
    /// Number of epilogue scalar iterations (remainder).
    pub epilogue_count: usize,
    /// Estimated speedup.
    pub estimated_speedup: f64,
}

impl VectorizationPlan {
    /// Number of vector iterations in the main body.
    pub fn vector_iterations(&self) -> usize {
        let body_count = self
            .loop_info
            .trip_count()
            .saturating_sub(self.prologue_count);
        body_count / self.vector_width
    }
}

/// Choose vector width based on trip count and element type.
fn choose_vector_width(trip_count: usize) -> usize {
    if trip_count >= 16 {
        8
    } else if trip_count >= 8 {
        4
    } else {
        1
    }
}

/// Build a vectorization plan for a loop.
///
/// Returns `None` if the loop is not vectorizable.
pub fn build_vectorization_plan(info: &LoopInfo) -> Option<VectorizationPlan> {
    if !info.is_vectorizable() {
        return None;
    }
    // Carried dependencies block vectorization (except reductions).
    if info.has_carried_dep && !info.is_reduction() {
        return None;
    }
    let trip = info.trip_count();
    let vw = choose_vector_width(trip);
    if vw <= 1 {
        return None;
    }
    let prologue = 0; // simplified: no alignment prologue
    let epilogue = trip % vw;
    let speedup = estimate_speedup_for_ops(&info.body_ops, vw);
    Some(VectorizationPlan {
        loop_info: info.clone(),
        scalar_width: 1,
        vector_width: vw,
        prologue_count: prologue,
        epilogue_count: epilogue,
        estimated_speedup: speedup,
    })
}

/// Estimate speedup for a set of loop operations at a given vector width.
pub fn estimate_speedup(plan: &VectorizationPlan) -> f64 {
    plan.estimated_speedup
}

/// Internal: heuristic speedup based on operation mix and vector width.
fn estimate_speedup_for_ops(ops: &[LoopOp], vw: usize) -> f64 {
    if ops.is_empty() || vw <= 1 {
        return 1.0;
    }
    let base = vw as f64; // ideal: width-fold speedup
    // Penalise: divisions are slower, conditionals need masking, reductions
    // lose some parallelism.
    let penalty: f64 = ops
        .iter()
        .map(|op| match op {
            LoopOp::Div => 0.7,
            LoopOp::Conditional => 0.6,
            LoopOp::ReduceAdd | LoopOp::ReduceMul => 0.5,
            LoopOp::ReduceMax | LoopOp::ReduceMin => 0.5,
            _ => 1.0,
        })
        .sum::<f64>()
        / ops.len() as f64;
    base * penalty
}

/// The auto-vectorization analysis pass.
#[derive(Debug, Clone)]
pub struct VectorizationPass {
    /// Plans produced by the pass.
    plans: Vec<VectorizationPlan>,
    /// Loops that were skipped (with reason).
    skipped: Vec<(LoopInfo, String)>,
    /// Minimum speedup threshold (default 1.5).
    min_speedup: f64,
}

impl VectorizationPass {
    /// Create a new pass with default settings.
    pub fn new() -> Self {
        Self {
            plans: Vec::new(),
            skipped: Vec::new(),
            min_speedup: 1.5,
        }
    }

    /// Override the minimum speedup threshold.
    pub fn with_min_speedup(mut self, threshold: f64) -> Self {
        self.min_speedup = threshold;
        self
    }

    /// Analyze a single loop and record the result.
    pub fn analyze(&mut self, info: LoopInfo) {
        match build_vectorization_plan(&info) {
            Some(plan) if plan.estimated_speedup >= self.min_speedup => {
                self.plans.push(plan);
            }
            Some(plan) => {
                let reason = format!(
                    "speedup {:.2}x below threshold {:.1}x",
                    plan.estimated_speedup, self.min_speedup
                );
                self.skipped.push((info, reason));
            }
            None => {
                let reason = if !info.is_vectorizable() {
                    "contains non-vectorizable operations".to_string()
                } else if info.has_carried_dep {
                    "non-reduction carried dependency".to_string()
                } else {
                    "trip count too small".to_string()
                };
                self.skipped.push((info, reason));
            }
        }
    }

    /// All accepted plans.
    pub fn plans(&self) -> &[VectorizationPlan] {
        &self.plans
    }

    /// All skipped loops with reasons.
    pub fn skipped(&self) -> &[(LoopInfo, String)] {
        &self.skipped
    }
}

impl Default for VectorizationPass {
    fn default() -> Self {
        Self::new()
    }
}

/// Human-readable auto-vectorization report.
#[derive(Debug, Clone)]
pub struct VectorizationReport {
    /// Lines of the report text.
    lines: Vec<String>,
}

impl VectorizationReport {
    /// Number of text lines.
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }
}

impl fmt::Display for VectorizationReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for line in &self.lines {
            writeln!(f, "{line}")?;
        }
        Ok(())
    }
}

/// Generate a human-readable vectorization report from accepted plans and
/// skipped loops.
pub fn generate_report(
    plans: &[VectorizationPlan],
    skipped: &[(LoopInfo, String)],
) -> VectorizationReport {
    let mut lines = Vec::new();
    lines.push("=== SIMD Auto-Vectorization Report ===".to_string());
    lines.push(String::new());
    lines.push(format!("Vectorized loops: {}", plans.len()));
    lines.push(format!("Skipped loops: {}", skipped.len()));
    lines.push(String::new());
    for (i, plan) in plans.iter().enumerate() {
        lines.push(format!(
            "[V{i}] loop '{}' (trip {}) -> width {} ({:.2}x speedup)",
            plan.loop_info.induction_var,
            plan.loop_info.trip_count(),
            plan.vector_width,
            plan.estimated_speedup,
        ));
    }
    if !skipped.is_empty() {
        lines.push(String::new());
        for (i, (info, reason)) in skipped.iter().enumerate() {
            lines.push(format!(
                "[S{i}] loop '{}' (trip {}) -- skipped: {reason}",
                info.induction_var,
                info.trip_count(),
            ));
        }
    }
    VectorizationReport { lines }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── Sprint 13 — SIMD Type System (s13_1 .. s13_10) ──────────────

    #[test]
    fn s13_1_simd_type_lane_count_and_element() {
        assert_eq!(SimdType::I32x4.lane_count(), 4);
        assert_eq!(SimdType::F32x8.lane_count(), 8);
        assert_eq!(SimdType::F64x2.lane_count(), 2);
        assert_eq!(SimdType::I8x16.lane_count(), 16);
        assert_eq!(SimdType::I32x4.element_type(), SimdElementType::I32);
        assert_eq!(SimdType::F64x4.element_type(), SimdElementType::F64);
        assert_eq!(SimdType::I16x8.element_type(), SimdElementType::I16);
    }

    #[test]
    fn s13_2_vector_construction_splat_and_from_array() {
        let v = SimdVector::splat(SimdType::F32x4, 3.0);
        assert_eq!(v.width(), 4);
        assert_eq!(v.lanes(), &[3.0, 3.0, 3.0, 3.0]);

        let v2 = SimdVector::from_array(SimdType::I32x4, &[1.0, 2.0, 3.0, 4.0]).unwrap();
        assert_eq!(v2.get(2).unwrap(), 3.0);
    }

    #[test]
    fn s13_3_vector_from_array_length_mismatch() {
        let err = SimdVector::from_array(SimdType::F32x4, &[1.0, 2.0]).unwrap_err();
        assert!(matches!(
            err,
            SimdError::ArrayLengthMismatch {
                expected: 4,
                got: 2
            }
        ));
    }

    #[test]
    fn s13_4_lane_get_set() {
        let mut v = SimdVector::splat(SimdType::F64x2, 0.0);
        v.set(1, 42.0).unwrap();
        assert_eq!(v.get(1).unwrap(), 42.0);

        let oob = v.get(5);
        assert!(matches!(
            oob,
            Err(SimdError::LaneOutOfBounds { index: 5, width: 2 })
        ));
    }

    #[test]
    fn s13_5_vector_arithmetic_add_sub_mul() {
        let a = SimdVector::from_array(SimdType::F32x4, &[1.0, 2.0, 3.0, 4.0]).unwrap();
        let b = SimdVector::from_array(SimdType::F32x4, &[5.0, 6.0, 7.0, 8.0]).unwrap();
        let sum = a.add(&b).unwrap();
        assert_eq!(sum.lanes(), &[6.0, 8.0, 10.0, 12.0]);

        let diff = a.sub(&b).unwrap();
        assert_eq!(diff.lanes(), &[-4.0, -4.0, -4.0, -4.0]);

        let prod = a.mul(&b).unwrap();
        assert_eq!(prod.lanes(), &[5.0, 12.0, 21.0, 32.0]);
    }

    #[test]
    fn s13_6_vector_div_and_div_by_zero() {
        let a = SimdVector::from_array(SimdType::F32x4, &[10.0, 20.0, 30.0, 40.0]).unwrap();
        let b = SimdVector::from_array(SimdType::F32x4, &[2.0, 4.0, 5.0, 8.0]).unwrap();
        let quot = a.div(&b).unwrap();
        assert_eq!(quot.lanes(), &[5.0, 5.0, 6.0, 5.0]);

        let zero = SimdVector::from_array(SimdType::F32x4, &[1.0, 0.0, 3.0, 4.0]).unwrap();
        let err = a.div(&zero).unwrap_err();
        assert!(matches!(err, SimdError::DivisionByZero { lane: 1 }));
    }

    #[test]
    fn s13_7_comparison_eq_lt_gt() {
        let a = SimdVector::from_array(SimdType::F32x4, &[1.0, 2.0, 3.0, 4.0]).unwrap();
        let b = SimdVector::from_array(SimdType::F32x4, &[1.0, 3.0, 2.0, 4.0]).unwrap();

        let eq = a.cmp_eq(&b).unwrap();
        assert_eq!(eq.bits(), &[true, false, false, true]);

        let lt = a.cmp_lt(&b).unwrap();
        assert_eq!(lt.bits(), &[false, true, false, false]);

        let gt = a.cmp_gt(&b).unwrap();
        assert_eq!(gt.bits(), &[false, false, true, false]);
    }

    #[test]
    fn s13_8_mask_vector_operations() {
        let m1 = MaskVector::from_bools(&[true, false, true, false]);
        let m2 = MaskVector::from_bools(&[true, true, false, false]);

        assert_eq!(m1.and(&m2).bits(), &[true, false, false, false]);
        assert_eq!(m1.or(&m2).bits(), &[true, true, true, false]);
        assert_eq!(m1.not().bits(), &[false, true, false, true]);
        assert_eq!(m1.count_true(), 2);
    }

    #[test]
    fn s13_9_shuffle_swizzle() {
        let v = SimdVector::from_array(SimdType::F32x4, &[10.0, 20.0, 30.0, 40.0]).unwrap();
        let shuffled = v.shuffle(&[3, 2, 1, 0]).unwrap();
        assert_eq!(shuffled.lanes(), &[40.0, 30.0, 20.0, 10.0]);

        let err = v.shuffle(&[0, 0, 0, 5]).unwrap_err();
        assert!(matches!(
            err,
            SimdError::ShuffleOutOfBounds { index: 5, width: 4 }
        ));
    }

    #[test]
    fn s13_10_simd_capability_detection() {
        let cap = detect_simd_capabilities();
        // On x86_64 test host: SSE+AVX should be true
        #[cfg(target_arch = "x86_64")]
        {
            assert!(cap.has_sse);
            assert!(cap.has_avx);
            assert!(!cap.has_neon);
        }
        #[cfg(target_arch = "aarch64")]
        {
            assert!(cap.has_neon);
            assert!(!cap.has_sse);
        }
        // Display should produce something non-empty on either arch
        let s = cap.to_string();
        assert!(!s.is_empty());
    }

    // ── Sprint 14 — Platform SIMD Intrinsics (s14_1 .. s14_10) ──────

    #[test]
    fn s14_1_simd_intrinsic_platform() {
        assert_eq!(SimdIntrinsic::MmAddPs.platform(), SimdPlatform::X86Sse);
        assert_eq!(SimdIntrinsic::Mm256FmaddPs.platform(), SimdPlatform::X86Avx);
        assert_eq!(SimdIntrinsic::VaddqF32.platform(), SimdPlatform::ArmNeon);
        assert_eq!(SimdIntrinsic::RvvFadd.platform(), SimdPlatform::RiscvV);
        assert_eq!(SimdIntrinsic::SveAdd.platform(), SimdPlatform::ArmSve);
    }

    #[test]
    fn s14_2_sse_mm_add_ps() {
        let a = SimdVector::from_array(SimdType::F32x4, &[1.0, 2.0, 3.0, 4.0]).unwrap();
        let b = SimdVector::from_array(SimdType::F32x4, &[5.0, 6.0, 7.0, 8.0]).unwrap();
        let c = mm_add_ps(&a, &b).unwrap();
        assert_eq!(c.lanes(), &[6.0, 8.0, 10.0, 12.0]);
    }

    #[test]
    fn s14_3_sse_mm_mul_ps_and_shuffle() {
        let a = SimdVector::from_array(SimdType::F32x4, &[2.0, 3.0, 4.0, 5.0]).unwrap();
        let b = SimdVector::from_array(SimdType::F32x4, &[10.0, 10.0, 10.0, 10.0]).unwrap();
        let c = mm_mul_ps(&a, &b).unwrap();
        assert_eq!(c.lanes(), &[20.0, 30.0, 40.0, 50.0]);

        let shuffled = mm_shuffle_ps(&a, &[3, 2, 1, 0]).unwrap();
        assert_eq!(shuffled.lanes(), &[5.0, 4.0, 3.0, 2.0]);
    }

    #[test]
    fn s14_4_avx_mm256_add_and_fmadd() {
        let a = SimdVector::from_array(SimdType::F32x8, &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0])
            .unwrap();
        let b = SimdVector::splat(SimdType::F32x8, 1.0);
        let c = SimdVector::splat(SimdType::F32x8, 100.0);
        let sum = mm256_add_ps(&a, &b).unwrap();
        assert_eq!(sum.get(0).unwrap(), 2.0);
        assert_eq!(sum.get(7).unwrap(), 9.0);

        let fma = mm256_fmadd_ps(&a, &b, &c).unwrap();
        // a*b + c = 1*1+100=101, 2*1+100=102, ...
        assert_eq!(fma.get(0).unwrap(), 101.0);
        assert_eq!(fma.get(7).unwrap(), 108.0);
    }

    #[test]
    fn s14_5_avx512_add_and_mask() {
        let a = [1.0f32; 16];
        let b = [2.0f32; 16];
        let c = mm512_add_ps(&a, &b);
        assert_eq!(c[0], 3.0);
        assert_eq!(c[15], 3.0);

        let mut mask = [false; 16];
        mask[0] = true;
        mask[15] = true;
        let masked = mm512_mask_add(&a, &b, &mask);
        assert_eq!(masked[0], 3.0);
        assert_eq!(masked[1], 1.0); // not masked
        assert_eq!(masked[15], 3.0);
    }

    #[test]
    fn s14_6_neon_vaddq_vmulq_vld1q() {
        let data: [f32; 4] = [1.0, 2.0, 3.0, 4.0];
        let loaded = vld1q_f32(&data);
        assert_eq!(loaded.simd_type(), SimdType::F32x4);

        let b = SimdVector::splat(SimdType::F32x4, 2.0);
        let added = vaddq_f32(&loaded, &b).unwrap();
        assert_eq!(added.lanes(), &[3.0, 4.0, 5.0, 6.0]);

        let mulled = vmulq_f32(&loaded, &b).unwrap();
        assert_eq!(mulled.lanes(), &[2.0, 4.0, 6.0, 8.0]);
    }

    #[test]
    fn s14_7_sve_predicated_add() {
        let a = vec![1.0f32, 2.0, 3.0, 4.0];
        let b = vec![10.0f32, 20.0, 30.0, 40.0];
        let pred = vec![true, false, true, false];
        let result = sve_add_predicated(&a, &b, &pred, 4).unwrap();
        assert_eq!(result, vec![11.0, 2.0, 33.0, 4.0]);
    }

    #[test]
    fn s14_8_riscv_v_fadd_fmul() {
        let a = vec![1.0f32, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
        let b = vec![10.0f32, 20.0, 30.0, 40.0, 50.0, 60.0, 70.0, 80.0];
        let sum = rvv_fadd(&a, &b, 4).unwrap();
        assert_eq!(sum, vec![11.0, 22.0, 33.0, 44.0]);

        let prod = rvv_fmul(&a, &b, 4).unwrap();
        assert_eq!(prod, vec![10.0, 40.0, 90.0, 160.0]);
    }

    #[test]
    fn s14_9_aligned_buffer() {
        let mut buf = AlignedBuffer::new(64, 32).unwrap();
        assert_eq!(buf.alignment(), 32);
        assert_eq!(buf.size(), 64);

        buf.write(0, &[0xAA, 0xBB]).unwrap();
        assert_eq!(&buf.data()[0..2], &[0xAA, 0xBB]);

        // Bad alignment
        let err = AlignedBuffer::new(64, 7).unwrap_err();
        assert!(matches!(err, SimdError::AlignmentError { .. }));
    }

    #[test]
    fn s14_10_simd_annotation_and_platform_display() {
        let ann = SimdAnnotation::auto();
        assert!(ann.platform.is_none());
        assert!(!ann.force);

        let ann2 = SimdAnnotation::with_platform(SimdPlatform::ArmNeon);
        assert_eq!(ann2.platform, Some(SimdPlatform::ArmNeon));

        assert_eq!(SimdPlatform::X86Sse.to_string(), "x86-SSE");
        assert_eq!(SimdPlatform::Generic.to_string(), "generic");
    }

    // ── Sprint 15 — SIMD Tensor Operations (s15_1 .. s15_10) ────────

    #[test]
    fn s15_1_simd_matmul_4x4() {
        // Identity * A = A
        let identity = [
            1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        ];
        let a = [
            1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0,
        ];
        let result = simd_matmul_4x4(&identity, &a);
        assert_eq!(result, a);
    }

    #[test]
    fn s15_2_simd_elementwise_add() {
        let a = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let b = vec![10.0, 20.0, 30.0, 40.0, 50.0];
        let c = simd_elementwise_add(&a, &b).unwrap();
        assert_eq!(c, vec![11.0, 22.0, 33.0, 44.0, 55.0]);

        // Length mismatch
        let err = simd_elementwise_add(&[1.0], &[1.0, 2.0]).unwrap_err();
        assert!(matches!(err, SimdError::WidthMismatch { .. }));
    }

    #[test]
    fn s15_3_simd_elementwise_mul() {
        let a = vec![2.0, 3.0, 4.0];
        let b = vec![5.0, 6.0, 7.0];
        let c = simd_elementwise_mul(&a, &b).unwrap();
        assert_eq!(c, vec![10.0, 18.0, 28.0]);
    }

    #[test]
    fn s15_4_simd_reduction_sum_max_min() {
        let data = vec![3.0, 1.0, 4.0, 1.0, 5.0, 9.0, 2.0, 6.0];
        assert_eq!(simd_horizontal_sum(&data), 31.0);
        assert_eq!(simd_horizontal_max(&data), Some(9.0));
        assert_eq!(simd_horizontal_min(&data), Some(1.0));
        assert_eq!(simd_horizontal_max(&[]), None);
        assert_eq!(simd_horizontal_min(&[]), None);
    }

    #[test]
    fn s15_5_simd_activation_relu() {
        let data = vec![-2.0, -1.0, 0.0, 1.0, 2.0];
        let out = simd_relu(&data);
        assert_eq!(out, vec![0.0, 0.0, 0.0, 1.0, 2.0]);
    }

    #[test]
    fn s15_6_simd_activation_sigmoid_tanh() {
        let data = vec![0.0f32];
        let sig = simd_sigmoid(&data);
        assert!((sig[0] - 0.5).abs() < 1e-5);

        let th = simd_tanh(&data);
        assert!((th[0]).abs() < 1e-5);
    }

    #[test]
    fn s15_7_simd_softmax() {
        let data = vec![1.0, 2.0, 3.0];
        let sm = simd_softmax(&data);
        let total: f32 = sm.iter().sum();
        assert!((total - 1.0).abs() < 1e-5);
        // Softmax should be monotonically increasing for increasing input
        assert!(sm[0] < sm[1]);
        assert!(sm[1] < sm[2]);
    }

    #[test]
    fn s15_8_simd_dot_product() {
        let a = vec![1.0, 2.0, 3.0, 4.0];
        let b = vec![5.0, 6.0, 7.0, 8.0];
        let dot = simd_dot_product(&a, &b).unwrap();
        assert_eq!(dot, 70.0); // 5+12+21+32
    }

    #[test]
    fn s15_9_simd_conv1d_and_quantize() {
        let input = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let kernel = vec![1.0, 0.0, -1.0];
        let out = simd_conv1d(&input, &kernel).unwrap();
        // [1*1+2*0+3*(-1), 2*1+3*0+4*(-1), 3*1+4*0+5*(-1)] = [-2, -2, -2]
        assert_eq!(out, vec![-2.0, -2.0, -2.0]);

        let data = vec![0.5, -0.5, 1.5, -1.5];
        let q = simd_quantize_f32_to_i8(&data, 100.0);
        assert_eq!(q, vec![50, -50, 127, -128]); // clamped to [-128, 127]
    }

    #[test]
    fn s15_10_simd_benchmark() {
        let result = run_simd_benchmark("elementwise_add", 1024);
        assert_eq!(result.element_count, 1024);
        assert!(result.speedup > 1.0);

        let mut bench = SimdBenchmark::new();
        bench.record(result);
        assert_eq!(bench.results().len(), 1);
        assert!(bench.average_speedup() > 1.0);
    }

    // ── Sprint 16 — Auto-Vectorization (s16_1 .. s16_10) ────────────

    #[test]
    fn s16_1_vectorization_pass_creation() {
        let pass = VectorizationPass::new();
        assert!(pass.plans().is_empty());
        assert!(pass.skipped().is_empty());
    }

    #[test]
    fn s16_2_loop_info_basic() {
        let info = analyze_loop("i", 0, 64, vec![LoopOp::Add], false);
        assert_eq!(info.trip_count(), 64);
        assert!(info.is_vectorizable());
        assert!(!info.is_reduction());
        assert_eq!(info.induction_var, "i");
    }

    #[test]
    fn s16_3_loop_info_non_vectorizable() {
        let info = analyze_loop("i", 0, 16, vec![LoopOp::Unknown], false);
        assert!(!info.is_vectorizable());
        assert!(build_vectorization_plan(&info).is_none());
    }

    #[test]
    fn s16_4_trip_count_and_vector_width() {
        // Large trip count → width 8
        let info = analyze_loop("i", 0, 128, vec![LoopOp::Add], false);
        let plan = build_vectorization_plan(&info).unwrap();
        assert_eq!(plan.vector_width, 8);
        assert_eq!(plan.epilogue_count, 128 % 8);
    }

    #[test]
    fn s16_5_vectorization_plan_speedup() {
        let info = analyze_loop("i", 0, 32, vec![LoopOp::Mul], false);
        let plan = build_vectorization_plan(&info).unwrap();
        let speedup = estimate_speedup(&plan);
        assert!(speedup > 1.0);
    }

    #[test]
    fn s16_6_cost_model_division_penalty() {
        let info_add = analyze_loop("i", 0, 64, vec![LoopOp::Add], false);
        let info_div = analyze_loop("i", 0, 64, vec![LoopOp::Div], false);
        let plan_add = build_vectorization_plan(&info_add).unwrap();
        let plan_div = build_vectorization_plan(&info_div).unwrap();
        // Div should have lower estimated speedup than add
        assert!(plan_div.estimated_speedup < plan_add.estimated_speedup);
    }

    #[test]
    fn s16_7_reduction_vectorization() {
        let info = analyze_loop("i", 0, 64, vec![LoopOp::ReduceAdd], true);
        assert!(info.is_reduction());
        // Reductions with carried deps are still vectorizable
        let plan = build_vectorization_plan(&info);
        assert!(plan.is_some());
    }

    #[test]
    fn s16_8_conditional_vectorization_masked() {
        let info = analyze_loop("i", 0, 32, vec![LoopOp::Conditional], false);
        assert!(info.is_vectorizable());
        let plan = build_vectorization_plan(&info).unwrap();
        // Conditional has penalty but should still be above 1.0
        assert!(plan.estimated_speedup > 1.0);
    }

    #[test]
    fn s16_9_pass_analyze_multiple_loops() {
        let mut pass = VectorizationPass::new();
        // Vectorizable
        let l1 = analyze_loop("i", 0, 64, vec![LoopOp::Add], false);
        // Too small / unknown
        let l2 = analyze_loop("j", 0, 2, vec![LoopOp::Add], false);
        // Non-vectorizable
        let l3 = analyze_loop("k", 0, 64, vec![LoopOp::Unknown], false);
        pass.analyze(l1);
        pass.analyze(l2);
        pass.analyze(l3);
        assert_eq!(pass.plans().len(), 1);
        assert_eq!(pass.skipped().len(), 2);
    }

    #[test]
    fn s16_10_generate_report() {
        let mut pass = VectorizationPass::new();
        let l1 = analyze_loop("i", 0, 128, vec![LoopOp::Add, LoopOp::Mul], false);
        let l2 = analyze_loop("j", 0, 4, vec![LoopOp::Unknown], false);
        pass.analyze(l1);
        pass.analyze(l2);

        let report = generate_report(pass.plans(), pass.skipped());
        let text = report.to_string();
        assert!(text.contains("Vectorized loops: 1"));
        assert!(text.contains("Skipped loops: 1"));
        assert!(text.contains("[V0]"));
        assert!(text.contains("[S0]"));
        assert!(report.line_count() > 3);
    }
}
