//! # FP8 Numeric Types
//!
//! 8-bit floating-point formats for modern AI inference and training:
//!
//! | Format | Exponent | Mantissa | Bias | Range | Use Case |
//! |--------|----------|----------|------|-------|----------|
//! | E5M2 | 5 bits | 2 bits | 15 | ±57344 | Gradients (wider range) |
//! | E4M3 | 4 bits | 3 bits | 7 | ±448 | Weights/activations (more precision) |
//!
//! Both formats follow the OFP8 specification (Open Compute Project).

use super::tensor::{TensorError, TensorValue};
use serde::Serialize;

// ═══════════════════════════════════════════════════════════════════════
// FP8 Format Enum
// ═══════════════════════════════════════════════════════════════════════

/// FP8 format variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Fp8Format {
    /// E5M2: 5-bit exponent, 2-bit mantissa, bias 15. IEEE 754-like.
    E5M2,
    /// E4M3: 4-bit exponent, 3-bit mantissa, bias 7. Single NaN encoding.
    E4M3,
}

impl std::fmt::Display for Fp8Format {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Fp8Format::E5M2 => write!(f, "E5M2"),
            Fp8Format::E4M3 => write!(f, "E4M3"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// E5M2 (5-bit exponent, 2-bit mantissa)
// ═══════════════════════════════════════════════════════════════════════

/// 8-bit float with 5-bit exponent, 2-bit mantissa (IEEE 754 proposal).
///
/// Bit layout: `[S][EEEEE][MM]`
/// - Sign: 1 bit
/// - Exponent: 5 bits, bias 15
/// - Mantissa: 2 bits
/// - Special values: ±Inf (E=31, M=0), NaN (E=31, M≠0), ±0 (E=0, M=0)
/// - Subnormals: E=0, M≠0 → value = (-1)^S × 2^(-14) × (0.M)
/// - Max normal: ±57344
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Fp8E5M2(pub u8);

impl Fp8E5M2 {
    /// Zero value.
    pub const ZERO: Self = Self(0x00);
    /// Positive infinity.
    pub const INF: Self = Self(0x7C);
    /// Negative infinity.
    pub const NEG_INF: Self = Self(0xFC);
    /// Not-a-Number (canonical).
    pub const NAN: Self = Self(0x7F);
    /// Maximum positive normal value (57344.0).
    pub const MAX: Self = Self(0x7B);
    /// Smallest positive subnormal (2^-16 = 0.0000152588).
    pub const MIN_SUBNORMAL: Self = Self(0x01);

    const _EXP_BITS: u32 = 5;
    const MAN_BITS: u32 = 2;
    const EXP_BIAS: i32 = 15;
    const EXP_MASK: u8 = 0x7C; // 0b0_11111_00
    const MAN_MASK: u8 = 0x03; // 0b0_00000_11

    /// Convert from f32 to E5M2 with round-to-nearest-even.
    pub fn from_f32(value: f32) -> Self {
        if value.is_nan() {
            return Self::NAN;
        }

        let sign = if value.is_sign_negative() { 1u8 } else { 0u8 };
        let abs_val = value.abs();

        if abs_val == 0.0 {
            return Self(sign << 7);
        }

        if abs_val.is_infinite() {
            return Self((sign << 7) | Self::EXP_MASK);
        }

        // Maximum representable value: 57344.0
        let max_val = 57344.0_f32;
        if abs_val > max_val {
            // Saturate to max or infinity
            return Self((sign << 7) | Self::EXP_MASK); // Inf
        }

        // Minimum subnormal: 2^-16
        let min_subnormal = 2.0_f32.powi(-16);
        if abs_val < min_subnormal * 0.5 {
            // Round to zero
            return Self(sign << 7);
        }

        // Convert via f32 bit manipulation
        let f32_bits = abs_val.to_bits();
        let f32_exp = ((f32_bits >> 23) & 0xFF) as i32 - 127;
        let f32_man = f32_bits & 0x7FFFFF;

        if f32_exp < -14 {
            // Subnormal in E5M2
            let shift = (-14 - f32_exp) as u32;
            let subnormal_man = (0x400000 | f32_man) >> (21 + shift);
            // Round-to-nearest-even
            let remainder_bit = if 21 + shift < 23 {
                (f32_man >> (20 + shift)) & 1
            } else {
                0
            };
            let man = (subnormal_man as u8) + if remainder_bit == 1 { 1 } else { 0 };
            return Self((sign << 7) | (man & Self::MAN_MASK));
        }

        // Normal number
        let biased_exp = (f32_exp + Self::EXP_BIAS) as u8;
        let man = (f32_man >> 21) as u8;

        // Round-to-nearest-even using the bits we're discarding
        let round_bit = (f32_man >> 20) & 1;
        let sticky_bits = f32_man & 0xFFFFF;
        let round_up = round_bit == 1 && (sticky_bits != 0 || (man & 1) == 1);

        let mut result_man = man;
        let mut result_exp = biased_exp;

        if round_up {
            result_man += 1;
            if result_man > Self::MAN_MASK {
                result_man = 0;
                result_exp += 1;
                if result_exp >= 31 {
                    return Self((sign << 7) | Self::EXP_MASK); // Overflow to Inf
                }
            }
        }

        Self((sign << 7) | (result_exp << Self::MAN_BITS) | result_man)
    }

    /// Convert E5M2 to f32 (lossless).
    pub fn to_f32(self) -> f32 {
        let sign = (self.0 >> 7) & 1;
        let exp = (self.0 & Self::EXP_MASK) >> Self::MAN_BITS;
        let man = self.0 & Self::MAN_MASK;

        if exp == 31 {
            if man == 0 {
                return if sign == 1 {
                    f32::NEG_INFINITY
                } else {
                    f32::INFINITY
                };
            }
            return f32::NAN;
        }

        if exp == 0 {
            if man == 0 {
                return if sign == 1 { -0.0 } else { 0.0 };
            }
            // Subnormal: value = (-1)^S × 2^(-14) × (0.mantissa)
            let value = (man as f32) / (1 << Self::MAN_BITS) as f32 * 2.0_f32.powi(-14);
            return if sign == 1 { -value } else { value };
        }

        // Normal: value = (-1)^S × 2^(E-15) × (1.mantissa)
        let real_exp = exp as i32 - Self::EXP_BIAS;
        let mantissa = 1.0 + (man as f32) / (1 << Self::MAN_BITS) as f32;
        let value = mantissa * 2.0_f32.powi(real_exp);
        if sign == 1 { -value } else { value }
    }

    /// Check if this value is NaN.
    pub fn is_nan(self) -> bool {
        let exp = (self.0 & Self::EXP_MASK) >> Self::MAN_BITS;
        let man = self.0 & Self::MAN_MASK;
        exp == 31 && man != 0
    }

    /// Check if this value is infinite.
    pub fn is_infinite(self) -> bool {
        let exp = (self.0 & Self::EXP_MASK) >> Self::MAN_BITS;
        let man = self.0 & Self::MAN_MASK;
        exp == 31 && man == 0
    }

    /// Check if this value is zero.
    pub fn is_zero(self) -> bool {
        (self.0 & 0x7F) == 0
    }

    /// Raw bit representation.
    pub fn to_bits(self) -> u8 {
        self.0
    }
}

impl std::fmt::Debug for Fp8E5M2 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Fp8E5M2({}, bits=0x{:02X})", self.to_f32(), self.0)
    }
}

impl std::fmt::Display for Fp8E5M2 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_f32())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// E4M3 (4-bit exponent, 3-bit mantissa)
// ═══════════════════════════════════════════════════════════════════════

/// 8-bit float with 4-bit exponent, 3-bit mantissa.
///
/// Bit layout: `[S][EEEE][MMM]`
/// - Sign: 1 bit
/// - Exponent: 4 bits, bias 7
/// - Mantissa: 3 bits
/// - Special: NaN = 0x7F only (single NaN encoding, no ±Inf)
/// - Subnormals: E=0, M≠0 → value = (-1)^S × 2^(-6) × (0.MMM)
/// - Max normal: ±448
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Fp8E4M3(pub u8);

impl Fp8E4M3 {
    /// Zero value.
    pub const ZERO: Self = Self(0x00);
    /// Not-a-Number (single encoding: 0b0_1111_111 = 0x7F).
    pub const NAN: Self = Self(0x7F);
    /// Negative NaN (0xFF).
    pub const NEG_NAN: Self = Self(0xFF);
    /// Maximum positive normal value (448.0).
    pub const MAX: Self = Self(0x7E);
    /// Smallest positive subnormal (2^-9 = 0.001953125).
    pub const MIN_SUBNORMAL: Self = Self(0x01);

    const _EXP_BITS: u32 = 4;
    const MAN_BITS: u32 = 3;
    const EXP_BIAS: i32 = 7;
    const EXP_MASK: u8 = 0x78; // 0b0_1111_000
    const MAN_MASK: u8 = 0x07; // 0b0_0000_111

    /// Convert from f32 to E4M3 with round-to-nearest-even.
    pub fn from_f32(value: f32) -> Self {
        if value.is_nan() {
            return Self::NAN;
        }

        let sign = if value.is_sign_negative() { 1u8 } else { 0u8 };
        let abs_val = value.abs();

        if abs_val == 0.0 {
            return Self(sign << 7);
        }

        // E4M3 has no infinity — saturate to max (448.0)
        let max_val = 448.0_f32;
        if abs_val > max_val || abs_val.is_infinite() {
            return Self((sign << 7) | 0x7E); // Max normal, not NaN
        }

        // Minimum subnormal: 2^-9
        let min_subnormal = 2.0_f32.powi(-9);
        if abs_val < min_subnormal * 0.5 {
            return Self(sign << 7); // Round to zero
        }

        let f32_bits = abs_val.to_bits();
        let f32_exp = ((f32_bits >> 23) & 0xFF) as i32 - 127;
        let f32_man = f32_bits & 0x7FFFFF;

        if f32_exp < -6 {
            // Subnormal in E4M3
            let shift = (-6 - f32_exp) as u32;
            let subnormal_man = (0x800000 | f32_man) >> (20 + shift);
            let man = subnormal_man as u8;
            return Self((sign << 7) | (man & Self::MAN_MASK));
        }

        // Normal number
        let biased_exp = (f32_exp + Self::EXP_BIAS) as u8;

        // Check for E4M3 max exponent (no Inf encoding — E=15,M=7 = NaN)
        if biased_exp >= 15 {
            return Self((sign << 7) | 0x7E); // Max normal
        }

        let man = (f32_man >> 20) as u8;

        // Round-to-nearest-even
        let round_bit = (f32_man >> 19) & 1;
        let sticky_bits = f32_man & 0x7FFFF;
        let round_up = round_bit == 1 && (sticky_bits != 0 || (man & 1) == 1);

        let mut result_man = man;
        let mut result_exp = biased_exp;

        if round_up {
            result_man += 1;
            if result_man > Self::MAN_MASK {
                result_man = 0;
                result_exp += 1;
                if result_exp >= 15 {
                    // Can't represent — saturate to max
                    return Self((sign << 7) | 0x7E);
                }
            }
        }

        Self((sign << 7) | (result_exp << Self::MAN_BITS) | result_man)
    }

    /// Convert E4M3 to f32 (lossless).
    pub fn to_f32(self) -> f32 {
        let sign = (self.0 >> 7) & 1;
        let exp = (self.0 & Self::EXP_MASK) >> Self::MAN_BITS;
        let man = self.0 & Self::MAN_MASK;

        // NaN: E=15, M=7 (0x7F or 0xFF)
        if exp == 15 && man == 7 {
            return f32::NAN;
        }

        if exp == 0 {
            if man == 0 {
                return if sign == 1 { -0.0 } else { 0.0 };
            }
            // Subnormal: value = (-1)^S × 2^(-6) × (0.MMM)
            let value = (man as f32) / (1 << Self::MAN_BITS) as f32 * 2.0_f32.powi(-6);
            return if sign == 1 { -value } else { value };
        }

        // Normal: value = (-1)^S × 2^(E-7) × (1.MMM)
        let real_exp = exp as i32 - Self::EXP_BIAS;
        let mantissa = 1.0 + (man as f32) / (1 << Self::MAN_BITS) as f32;
        let value = mantissa * 2.0_f32.powi(real_exp);
        if sign == 1 { -value } else { value }
    }

    /// Check if this value is NaN.
    pub fn is_nan(self) -> bool {
        let exp = (self.0 & Self::EXP_MASK) >> Self::MAN_BITS;
        let man = self.0 & Self::MAN_MASK;
        exp == 15 && man == 7
    }

    /// Check if this value is zero.
    pub fn is_zero(self) -> bool {
        (self.0 & 0x7F) == 0
    }

    /// Raw bit representation.
    pub fn to_bits(self) -> u8 {
        self.0
    }
}

impl std::fmt::Debug for Fp8E4M3 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Fp8E4M3({}, bits=0x{:02X})", self.to_f32(), self.0)
    }
}

impl std::fmt::Display for Fp8E4M3 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_f32())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// FP8 Arithmetic (via f32 upcast)
// ═══════════════════════════════════════════════════════════════════════

/// Add two FP8 values (via f32 upcast + downcast).
pub fn fp8_add(a: u8, b: u8, format: Fp8Format) -> u8 {
    let (va, vb) = upcast_pair(a, b, format);
    downcast(va + vb, format)
}

/// Subtract two FP8 values.
pub fn fp8_sub(a: u8, b: u8, format: Fp8Format) -> u8 {
    let (va, vb) = upcast_pair(a, b, format);
    downcast(va - vb, format)
}

/// Multiply two FP8 values.
pub fn fp8_mul(a: u8, b: u8, format: Fp8Format) -> u8 {
    let (va, vb) = upcast_pair(a, b, format);
    downcast(va * vb, format)
}

/// Divide two FP8 values.
pub fn fp8_div(a: u8, b: u8, format: Fp8Format) -> u8 {
    let (va, vb) = upcast_pair(a, b, format);
    downcast(va / vb, format)
}

fn upcast_pair(a: u8, b: u8, format: Fp8Format) -> (f32, f32) {
    match format {
        Fp8Format::E5M2 => (Fp8E5M2(a).to_f32(), Fp8E5M2(b).to_f32()),
        Fp8Format::E4M3 => (Fp8E4M3(a).to_f32(), Fp8E4M3(b).to_f32()),
    }
}

fn downcast(value: f32, format: Fp8Format) -> u8 {
    match format {
        Fp8Format::E5M2 => Fp8E5M2::from_f32(value).0,
        Fp8Format::E4M3 => Fp8E4M3::from_f32(value).0,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// FP8 Quantized Tensor
// ═══════════════════════════════════════════════════════════════════════

/// A tensor quantized to FP8 with a per-tensor scale factor.
///
/// Real value ≈ `scale * fp8_to_f32(data[i])`
#[derive(Debug, Clone)]
pub struct Fp8Tensor {
    /// Quantized data as packed u8 values.
    data: Vec<u8>,
    /// Per-tensor scale factor.
    scale: f64,
    /// Tensor shape.
    shape: Vec<usize>,
    /// FP8 format (E5M2 or E4M3).
    format: Fp8Format,
}

impl Fp8Tensor {
    /// Quantize a float tensor to FP8.
    ///
    /// Calibrates scale from the maximum absolute value in the tensor.
    pub fn quantize(tensor: &TensorValue, format: Fp8Format) -> Self {
        let values = tensor.to_vec();
        let max_abs = values.iter().map(|v| v.abs()).fold(0.0_f64, f64::max);

        // Scale to map values into FP8 representable range
        let fp8_max = match format {
            Fp8Format::E5M2 => 57344.0,
            Fp8Format::E4M3 => 448.0,
        };

        let scale = if max_abs == 0.0 {
            1.0
        } else {
            max_abs / fp8_max
        };

        let data: Vec<u8> = values
            .iter()
            .map(|&v| {
                let scaled = (v / scale) as f32;
                match format {
                    Fp8Format::E5M2 => Fp8E5M2::from_f32(scaled).0,
                    Fp8Format::E4M3 => Fp8E4M3::from_f32(scaled).0,
                }
            })
            .collect();

        Self {
            data,
            scale,
            shape: tensor.shape().to_vec(),
            format,
        }
    }

    /// Dequantize back to a float tensor.
    pub fn dequantize(&self) -> TensorValue {
        let values: Vec<f64> = self
            .data
            .iter()
            .map(|&q| {
                let fp8_val = match self.format {
                    Fp8Format::E5M2 => Fp8E5M2(q).to_f32() as f64,
                    Fp8Format::E4M3 => Fp8E4M3(q).to_f32() as f64,
                };
                fp8_val * self.scale
            })
            .collect();

        TensorValue::from_data(values, &self.shape)
            .unwrap_or_else(|_| TensorValue::zeros(&self.shape))
    }

    /// Returns the quantized data as a slice.
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Returns the scale factor.
    pub fn scale(&self) -> f64 {
        self.scale
    }

    /// Returns the tensor shape.
    pub fn shape(&self) -> &[usize] {
        &self.shape
    }

    /// Returns the FP8 format.
    pub fn format(&self) -> Fp8Format {
        self.format
    }

    /// Number of elements.
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Check if the tensor is empty.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Memory size in bytes (data only, no scale/shape overhead).
    pub fn byte_size(&self) -> usize {
        self.data.len() // 1 byte per element
    }

    /// Compression ratio vs f64 storage.
    pub fn compression_ratio(&self) -> f64 {
        8.0 // f64 = 8 bytes, fp8 = 1 byte
    }

    /// Compute quantization error (RMSE) against original tensor.
    pub fn quantization_error(&self, original: &TensorValue) -> Result<f64, TensorError> {
        let dequantized = self.dequantize();
        let orig_vals = original.to_vec();
        let deq_vals = dequantized.to_vec();

        if orig_vals.len() != deq_vals.len() {
            return Err(TensorError::ShapeMismatch {
                expected: original.shape().to_vec(),
                got: self.shape.clone(),
            });
        }

        let mse: f64 = orig_vals
            .iter()
            .zip(deq_vals.iter())
            .map(|(a, b)| (a - b).powi(2))
            .sum::<f64>()
            / orig_vals.len() as f64;

        Ok(mse.sqrt())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── S5.1: E5M2 Format ─────────────────────────────────────────────

    #[test]
    fn e5m2_zero() {
        assert_eq!(Fp8E5M2::ZERO.to_f32(), 0.0);
        assert!(Fp8E5M2::ZERO.is_zero());
    }

    #[test]
    fn e5m2_one() {
        let one = Fp8E5M2::from_f32(1.0);
        assert!((one.to_f32() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn e5m2_negative() {
        let neg = Fp8E5M2::from_f32(-2.0);
        assert!((neg.to_f32() - (-2.0)).abs() < 1e-6);
    }

    #[test]
    fn e5m2_inf() {
        assert!(Fp8E5M2::INF.to_f32().is_infinite());
        assert!(Fp8E5M2::INF.is_infinite());
        assert!(Fp8E5M2::NEG_INF.to_f32().is_infinite());
    }

    #[test]
    fn e5m2_nan() {
        assert!(Fp8E5M2::NAN.to_f32().is_nan());
        assert!(Fp8E5M2::NAN.is_nan());
    }

    #[test]
    fn e5m2_nan_propagation() {
        let nan = Fp8E5M2::from_f32(f32::NAN);
        assert!(nan.is_nan());
    }

    // ── S5.2: E4M3 Format ─────────────────────────────────────────────

    #[test]
    fn e4m3_zero() {
        assert_eq!(Fp8E4M3::ZERO.to_f32(), 0.0);
        assert!(Fp8E4M3::ZERO.is_zero());
    }

    #[test]
    fn e4m3_one() {
        let one = Fp8E4M3::from_f32(1.0);
        assert!((one.to_f32() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn e4m3_negative() {
        let neg = Fp8E4M3::from_f32(-3.5);
        assert!((neg.to_f32() - (-3.5)).abs() < 1e-6);
    }

    #[test]
    fn e4m3_nan() {
        assert!(Fp8E4M3::NAN.to_f32().is_nan());
        assert!(Fp8E4M3::NAN.is_nan());
    }

    #[test]
    fn e4m3_no_infinity() {
        // E4M3 has no infinity — overflow saturates to max (448)
        let big = Fp8E4M3::from_f32(f32::INFINITY);
        assert!(!big.is_nan());
        assert!((big.to_f32() - 448.0).abs() < 1e-6);
    }

    #[test]
    fn e4m3_max_value() {
        let max = Fp8E4M3::MAX;
        assert!((max.to_f32() - 448.0).abs() < 1e-6);
    }

    // ── S5.3: FP8 Arithmetic ──────────────────────────────────────────

    #[test]
    fn fp8_add_e5m2() {
        let a = Fp8E5M2::from_f32(1.0).0;
        let b = Fp8E5M2::from_f32(2.0).0;
        let result = fp8_add(a, b, Fp8Format::E5M2);
        let val = Fp8E5M2(result).to_f32();
        assert!((val - 3.0).abs() < 0.5);
    }

    #[test]
    fn fp8_mul_e4m3() {
        let a = Fp8E4M3::from_f32(2.0).0;
        let b = Fp8E4M3::from_f32(3.0).0;
        let result = fp8_mul(a, b, Fp8Format::E4M3);
        let val = Fp8E4M3(result).to_f32();
        assert!((val - 6.0).abs() < 0.5);
    }

    #[test]
    fn fp8_sub_e5m2() {
        let a = Fp8E5M2::from_f32(5.0).0;
        let b = Fp8E5M2::from_f32(3.0).0;
        let result = fp8_sub(a, b, Fp8Format::E5M2);
        let val = Fp8E5M2(result).to_f32();
        assert!((val - 2.0).abs() < 0.5);
    }

    #[test]
    fn fp8_div_e4m3() {
        let a = Fp8E4M3::from_f32(6.0).0;
        let b = Fp8E4M3::from_f32(2.0).0;
        let result = fp8_div(a, b, Fp8Format::E4M3);
        let val = Fp8E4M3(result).to_f32();
        assert!((val - 3.0).abs() < 0.5);
    }

    // ── S5.4: FP8-to-F32 Conversion ──────────────────────────────────

    #[test]
    fn e5m2_round_trip_powers_of_two() {
        for exp in -14..=15 {
            let val = 2.0_f32.powi(exp);
            let fp8 = Fp8E5M2::from_f32(val);
            let back = fp8.to_f32();
            assert!(
                (back - val).abs() / val.abs() < 0.3,
                "Round-trip failed for 2^{exp}: {val} -> {back}"
            );
        }
    }

    #[test]
    fn e4m3_round_trip_small_integers() {
        for i in 0..=8 {
            let val = i as f32;
            let fp8 = Fp8E4M3::from_f32(val);
            let back = fp8.to_f32();
            assert!(
                (back - val).abs() < 0.5,
                "Round-trip failed for {val}: got {back}"
            );
        }
    }

    // ── S5.5: F32-to-FP8 Conversion ──────────────────────────────────

    #[test]
    fn e5m2_overflow_to_inf() {
        let huge = Fp8E5M2::from_f32(100000.0);
        assert!(huge.is_infinite());
    }

    #[test]
    fn e4m3_overflow_saturates() {
        let huge = Fp8E4M3::from_f32(1000.0);
        assert!((huge.to_f32() - 448.0).abs() < 1e-6);
    }

    #[test]
    fn e5m2_underflow_to_zero() {
        let tiny = Fp8E5M2::from_f32(1e-10);
        assert!(tiny.is_zero());
    }

    // ── S5.6: FP8 Tensor Integration ──────────────────────────────────

    #[test]
    fn fp8_tensor_quantize_e4m3() {
        let tensor = TensorValue::from_data(vec![1.0, 2.0, 3.0, 4.0], &[2, 2]).unwrap();
        let quantized = Fp8Tensor::quantize(&tensor, Fp8Format::E4M3);
        assert_eq!(quantized.len(), 4);
        assert_eq!(quantized.shape(), &[2, 2]);
        assert_eq!(quantized.format(), Fp8Format::E4M3);
    }

    #[test]
    fn fp8_tensor_quantize_e5m2() {
        let tensor = TensorValue::from_data(vec![0.5, -1.0, 2.5, -0.25], &[4]).unwrap();
        let quantized = Fp8Tensor::quantize(&tensor, Fp8Format::E5M2);
        assert_eq!(quantized.len(), 4);
        assert_eq!(quantized.byte_size(), 4);
    }

    // ── S5.7: FP8 Quantization Pipeline ───────────────────────────────

    #[test]
    fn fp8_tensor_compression_ratio() {
        let tensor = TensorValue::from_data(vec![1.0; 100], &[100]).unwrap();
        let quantized = Fp8Tensor::quantize(&tensor, Fp8Format::E4M3);
        assert!((quantized.compression_ratio() - 8.0).abs() < 1e-6);
    }

    // ── S5.8: FP8 Dequantization ──────────────────────────────────────

    #[test]
    fn fp8_tensor_round_trip_low_error() {
        let tensor = TensorValue::from_data(vec![1.0, 2.0, 3.0, 4.0], &[2, 2]).unwrap();
        let quantized = Fp8Tensor::quantize(&tensor, Fp8Format::E4M3);
        let error = quantized.quantization_error(&tensor).unwrap();
        assert!(error < 1.0, "Quantization error too high: {error}");
    }

    #[test]
    fn fp8_tensor_dequantize_shape_preserved() {
        let tensor = TensorValue::from_data(vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0], &[2, 3]).unwrap();
        let quantized = Fp8Tensor::quantize(&tensor, Fp8Format::E5M2);
        let back = quantized.dequantize();
        assert_eq!(back.shape(), &[2, 3]);
    }

    // ── S5.9: Type System Integration ─────────────────────────────────
    // (lexer/parser/analyzer integration tested in integration tests)

    #[test]
    fn fp8_format_display() {
        assert_eq!(format!("{}", Fp8Format::E5M2), "E5M2");
        assert_eq!(format!("{}", Fp8Format::E4M3), "E4M3");
    }

    // ── S5.10: Additional Coverage ────────────────────────────────────

    #[test]
    fn e5m2_negative_zero() {
        let neg_zero = Fp8E5M2::from_f32(-0.0);
        assert!(neg_zero.is_zero());
        assert_eq!(neg_zero.to_f32().to_bits(), (-0.0_f32).to_bits());
    }

    #[test]
    fn e4m3_negative_zero() {
        let neg_zero = Fp8E4M3::from_f32(-0.0);
        assert!(neg_zero.is_zero());
    }

    #[test]
    fn e5m2_subnormal() {
        let min = Fp8E5M2::MIN_SUBNORMAL;
        let val = min.to_f32();
        assert!(val > 0.0);
        assert!(val < 2.0_f32.powi(-14));
    }

    #[test]
    fn e4m3_subnormal() {
        let min = Fp8E4M3::MIN_SUBNORMAL;
        let val = min.to_f32();
        assert!(val > 0.0);
        assert!(val < 2.0_f32.powi(-6));
    }

    #[test]
    fn fp8_tensor_empty() {
        let tensor = TensorValue::from_data(vec![], &[0]).unwrap();
        let quantized = Fp8Tensor::quantize(&tensor, Fp8Format::E4M3);
        assert!(quantized.is_empty());
    }

    #[test]
    fn fp8_tensor_scale_positive() {
        let tensor = TensorValue::from_data(vec![1.0, -1.0], &[2]).unwrap();
        let quantized = Fp8Tensor::quantize(&tensor, Fp8Format::E5M2);
        assert!(quantized.scale() > 0.0);
    }
}
