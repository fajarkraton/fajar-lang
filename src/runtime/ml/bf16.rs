//! # BFloat16 Numeric Type
//!
//! Brain floating-point format (bfloat16): 1 sign, 8 exponent, 7 mantissa.
//!
//! | Field | Bits | Description |
//! |-------|------|-------------|
//! | Sign | 1 | Sign bit |
//! | Exponent | 8 | Same range as f32 (bias 127) |
//! | Mantissa | 7 | Reduced precision (vs f32's 23) |
//!
//! Key property: same exponent range as f32, so no overflow/underflow on conversion.
//! Used heavily in AI training (Google TPU, NVIDIA Tensor Cores, Intel AMX).

use super::tensor::{TensorError, TensorValue};
use serde::Serialize;

// ═══════════════════════════════════════════════════════════════════════
// BF16 Type
// ═══════════════════════════════════════════════════════════════════════

/// Brain floating-point 16-bit format.
///
/// Stored as u16. Same exponent range as f32, but only 7 mantissa bits
/// (vs 23 in f32). Conversion is a simple truncation of the lower 16 bits.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub struct Bf16(pub u16);

impl Bf16 {
    /// Zero value.
    pub const ZERO: Self = Self(0x0000);
    /// One.
    pub const ONE: Self = Self(0x3F80);
    /// Positive infinity.
    pub const INF: Self = Self(0x7F80);
    /// Negative infinity.
    pub const NEG_INF: Self = Self(0xFF80);
    /// Not-a-Number (canonical).
    pub const NAN: Self = Self(0x7FC0);
    /// Maximum positive finite value (~3.39 × 10^38).
    pub const MAX: Self = Self(0x7F7F);
    /// Smallest positive normal (~1.18 × 10^-38).
    pub const MIN_POSITIVE: Self = Self(0x0080);

    /// Convert from f32 to bf16 via truncation (fast, rounds toward zero).
    pub fn from_f32_truncate(value: f32) -> Self {
        let bits = value.to_bits();
        Self((bits >> 16) as u16)
    }

    /// Convert from f32 to bf16 with round-to-nearest-even (accurate).
    pub fn from_f32(value: f32) -> Self {
        if value.is_nan() {
            return Self::NAN;
        }

        let bits = value.to_bits();
        let upper = (bits >> 16) as u16;
        let lower = bits & 0xFFFF;

        // Round-to-nearest-even
        let round_bit = lower >> 15;
        let sticky = lower & 0x7FFF;

        if round_bit == 1 && (sticky != 0 || (upper & 1) == 1) {
            Self(upper.wrapping_add(1))
        } else {
            Self(upper)
        }
    }

    /// Convert from f64 to bf16 (via f32 intermediate).
    pub fn from_f64(value: f64) -> Self {
        Self::from_f32(value as f32)
    }

    /// Convert bf16 to f32 (lossless — zero-extend mantissa).
    pub fn to_f32(self) -> f32 {
        let bits = (self.0 as u32) << 16;
        f32::from_bits(bits)
    }

    /// Convert bf16 to f64.
    pub fn to_f64(self) -> f64 {
        self.to_f32() as f64
    }

    /// Check if NaN.
    pub fn is_nan(self) -> bool {
        let exp = (self.0 >> 7) & 0xFF;
        let man = self.0 & 0x7F;
        exp == 0xFF && man != 0
    }

    /// Check if infinite.
    pub fn is_infinite(self) -> bool {
        let exp = (self.0 >> 7) & 0xFF;
        let man = self.0 & 0x7F;
        exp == 0xFF && man == 0
    }

    /// Check if zero.
    pub fn is_zero(self) -> bool {
        (self.0 & 0x7FFF) == 0
    }

    /// Raw bit representation.
    pub fn to_bits(self) -> u16 {
        self.0
    }

    // ── Arithmetic (via f32 upcast) ──────────────────────────────────

    /// Add two bf16 values.
    #[allow(clippy::should_implement_trait)]
    pub fn add(self, other: Self) -> Self {
        Self::from_f32(self.to_f32() + other.to_f32())
    }

    /// Subtract two bf16 values.
    #[allow(clippy::should_implement_trait)]
    pub fn sub(self, other: Self) -> Self {
        Self::from_f32(self.to_f32() - other.to_f32())
    }

    /// Multiply two bf16 values.
    #[allow(clippy::should_implement_trait)]
    pub fn mul(self, other: Self) -> Self {
        Self::from_f32(self.to_f32() * other.to_f32())
    }

    /// Divide two bf16 values.
    #[allow(clippy::should_implement_trait)]
    pub fn div(self, other: Self) -> Self {
        Self::from_f32(self.to_f32() / other.to_f32())
    }
}

impl std::fmt::Debug for Bf16 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Bf16({}, bits=0x{:04X})", self.to_f32(), self.0)
    }
}

impl std::fmt::Display for Bf16 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_f32())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// BF16 Tensor
// ═══════════════════════════════════════════════════════════════════════

/// A tensor stored in BF16 format.
///
/// No scale factor needed — bf16 has the same exponent range as f32.
#[derive(Debug, Clone)]
pub struct Bf16Tensor {
    /// BF16 data (one u16 per element).
    data: Vec<u16>,
    /// Tensor shape.
    shape: Vec<usize>,
}

impl Bf16Tensor {
    /// Convert a float tensor to BF16.
    pub fn from_tensor(tensor: &TensorValue) -> Self {
        let values = tensor.to_vec();
        let data: Vec<u16> = values.iter().map(|&v| Bf16::from_f64(v).0).collect();
        Self {
            data,
            shape: tensor.shape().to_vec(),
        }
    }

    /// Convert back to a float tensor.
    pub fn to_tensor(&self) -> TensorValue {
        let values: Vec<f64> = self.data.iter().map(|&b| Bf16(b).to_f64()).collect();
        TensorValue::from_data(values, &self.shape)
            .unwrap_or_else(|_| TensorValue::zeros(&self.shape))
    }

    /// Number of elements.
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Memory size in bytes.
    pub fn byte_size(&self) -> usize {
        self.data.len() * 2 // 2 bytes per bf16
    }

    /// Compression ratio vs f64.
    pub fn compression_ratio(&self) -> f64 {
        4.0 // f64 = 8 bytes, bf16 = 2 bytes
    }

    /// Tensor shape.
    pub fn shape(&self) -> &[usize] {
        &self.shape
    }

    /// Raw data.
    pub fn data(&self) -> &[u16] {
        &self.data
    }

    /// Compute quantization error vs original.
    pub fn quantization_error(&self, original: &TensorValue) -> Result<f64, TensorError> {
        let converted = self.to_tensor();
        let orig_vals = original.to_vec();
        let conv_vals = converted.to_vec();

        if orig_vals.len() != conv_vals.len() {
            return Err(TensorError::ShapeMismatch {
                expected: original.shape().to_vec(),
                got: self.shape.clone(),
            });
        }

        let mse: f64 = orig_vals
            .iter()
            .zip(conv_vals.iter())
            .map(|(a, b)| (a - b).powi(2))
            .sum::<f64>()
            / orig_vals.len().max(1) as f64;

        Ok(mse.sqrt())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Mixed-Precision Training Support
// ═══════════════════════════════════════════════════════════════════════

/// Dynamic loss scaling state for BF16 mixed-precision training.
///
/// Scales up the loss before backward pass to prevent gradient underflow
/// in BF16. If overflow is detected, halves the scale and skips the step.
#[derive(Debug, Clone)]
pub struct DynamicLossScaler {
    /// Current loss scale factor.
    scale: f64,
    /// Number of consecutive steps without overflow.
    steps_without_overflow: u32,
    /// Steps before trying to increase the scale.
    growth_interval: u32,
    /// Factor to multiply scale on growth.
    growth_factor: f64,
    /// Factor to multiply scale on overflow.
    backoff_factor: f64,
    /// Minimum allowed scale.
    min_scale: f64,
}

impl DynamicLossScaler {
    /// Create a new loss scaler with default parameters.
    pub fn new() -> Self {
        Self {
            scale: 65536.0, // Initial scale
            steps_without_overflow: 0,
            growth_interval: 2000,
            growth_factor: 2.0,
            backoff_factor: 0.5,
            min_scale: 1.0,
        }
    }

    /// Create with custom initial scale.
    pub fn with_scale(scale: f64) -> Self {
        Self {
            scale,
            ..Self::new()
        }
    }

    /// Current loss scale.
    pub fn scale(&self) -> f64 {
        self.scale
    }

    /// Scale a loss value for backward pass.
    pub fn scale_loss(&self, loss: f64) -> f64 {
        loss * self.scale
    }

    /// Unscale gradients after backward pass.
    pub fn unscale_gradient(&self, grad: f64) -> f64 {
        grad / self.scale
    }

    /// Check gradients for overflow and update scale.
    ///
    /// Returns `true` if gradients are valid (optimizer should step).
    /// Returns `false` if overflow detected (skip this step).
    pub fn update(&mut self, has_overflow: bool) -> bool {
        if has_overflow {
            self.scale = (self.scale * self.backoff_factor).max(self.min_scale);
            self.steps_without_overflow = 0;
            false
        } else {
            self.steps_without_overflow += 1;
            if self.steps_without_overflow >= self.growth_interval {
                self.scale *= self.growth_factor;
                self.steps_without_overflow = 0;
            }
            true
        }
    }

    /// Check if a gradient tensor contains overflow (Inf or NaN).
    pub fn check_overflow(values: &[f64]) -> bool {
        values.iter().any(|v| v.is_nan() || v.is_infinite())
    }
}

impl Default for DynamicLossScaler {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── S7.1: BF16 Format ─────────────────────────────────────────────

    #[test]
    fn bf16_zero() {
        assert_eq!(Bf16::ZERO.to_f32(), 0.0);
        assert!(Bf16::ZERO.is_zero());
    }

    #[test]
    fn bf16_one() {
        assert!((Bf16::ONE.to_f32() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn bf16_inf() {
        assert!(Bf16::INF.to_f32().is_infinite());
        assert!(Bf16::INF.is_infinite());
    }

    #[test]
    fn bf16_nan() {
        assert!(Bf16::NAN.to_f32().is_nan());
        assert!(Bf16::NAN.is_nan());
    }

    // ── S7.2: BF16 Arithmetic ─────────────────────────────────────────

    #[test]
    fn bf16_add() {
        let a = Bf16::from_f32(1.0);
        let b = Bf16::from_f32(2.0);
        let c = a.add(b);
        assert!((c.to_f32() - 3.0).abs() < 0.1);
    }

    #[test]
    fn bf16_mul() {
        let a = Bf16::from_f32(3.0);
        let b = Bf16::from_f32(4.0);
        let c = a.mul(b);
        assert!((c.to_f32() - 12.0).abs() < 0.1);
    }

    #[test]
    fn bf16_sub() {
        let a = Bf16::from_f32(5.0);
        let b = Bf16::from_f32(2.0);
        let c = a.sub(b);
        assert!((c.to_f32() - 3.0).abs() < 0.1);
    }

    #[test]
    fn bf16_div() {
        let a = Bf16::from_f32(10.0);
        let b = Bf16::from_f32(2.0);
        let c = a.div(b);
        assert!((c.to_f32() - 5.0).abs() < 0.1);
    }

    // ── S7.3: F32-to-BF16 Conversion ──────────────────────────────────

    #[test]
    fn bf16_from_f32_truncate() {
        let bf = Bf16::from_f32_truncate(1.5);
        assert!((bf.to_f32() - 1.5).abs() < 0.1);
    }

    #[test]
    fn bf16_from_f32_rne() {
        let bf = Bf16::from_f32(1.5);
        assert!((bf.to_f32() - 1.5).abs() < 0.1);
    }

    // ── S7.4: BF16-to-F32 Conversion ──────────────────────────────────

    #[test]
    fn bf16_to_f32_lossless() {
        // bf16 → f32 is always lossless (zero-extend)
        let bf = Bf16::from_f32(42.0);
        let back = bf.to_f32();
        assert!((back - 42.0).abs() < 1.0);
    }

    #[test]
    fn bf16_to_f64() {
        let bf = Bf16::from_f64(1.25);
        let back = bf.to_f64();
        assert!((back - 1.25).abs() < 0.05);
    }

    // ── S7.5: BF16 Tensor Ops ─────────────────────────────────────────

    #[test]
    fn bf16_tensor_from_tensor() {
        let tensor = TensorValue::from_data(vec![1.0, 2.0, 3.0, 4.0], &[2, 2]).unwrap();
        let bf = Bf16Tensor::from_tensor(&tensor);
        assert_eq!(bf.len(), 4);
        assert_eq!(bf.shape(), &[2, 2]);
    }

    #[test]
    fn bf16_tensor_round_trip() {
        let tensor = TensorValue::from_data(vec![1.0, 2.0, 3.0, 4.0], &[2, 2]).unwrap();
        let bf = Bf16Tensor::from_tensor(&tensor);
        let error = bf.quantization_error(&tensor).unwrap();
        assert!(error < 0.05, "BF16 error too high: {error}");
    }

    #[test]
    fn bf16_tensor_compression() {
        let bf = Bf16Tensor::from_tensor(&TensorValue::from_data(vec![1.0; 100], &[100]).unwrap());
        assert!((bf.compression_ratio() - 4.0).abs() < 1e-6);
    }

    // ── S7.6: Mixed-Precision Training ────────────────────────────────

    #[test]
    fn bf16_tensor_dequantize_preserves_shape() {
        let tensor = TensorValue::from_data(vec![1.0, 2.0, 3.0], &[3]).unwrap();
        let bf = Bf16Tensor::from_tensor(&tensor);
        let back = bf.to_tensor();
        assert_eq!(back.shape(), &[3]);
    }

    // ── S7.7: Dynamic Loss Scaling ────────────────────────────────────

    #[test]
    fn loss_scaler_initial_scale() {
        let scaler = DynamicLossScaler::new();
        assert!((scaler.scale() - 65536.0).abs() < 1e-6);
    }

    #[test]
    fn loss_scaler_scale_loss() {
        let scaler = DynamicLossScaler::new();
        let scaled = scaler.scale_loss(1.0);
        assert!((scaled - 65536.0).abs() < 1e-6);
    }

    #[test]
    fn loss_scaler_unscale_gradient() {
        let scaler = DynamicLossScaler::new();
        let unscaled = scaler.unscale_gradient(65536.0);
        assert!((unscaled - 1.0).abs() < 1e-6);
    }

    #[test]
    fn loss_scaler_backoff_on_overflow() {
        let mut scaler = DynamicLossScaler::new();
        let should_step = scaler.update(true);
        assert!(!should_step);
        assert!((scaler.scale() - 32768.0).abs() < 1e-6);
    }

    #[test]
    fn loss_scaler_growth_after_interval() {
        let mut scaler = DynamicLossScaler::new();
        scaler.growth_interval = 2; // Short interval for testing
        scaler.update(false);
        scaler.update(false);
        // After 2 steps without overflow, scale should double
        assert!((scaler.scale() - 131072.0).abs() < 1e-6);
    }

    #[test]
    fn loss_scaler_check_overflow() {
        assert!(!DynamicLossScaler::check_overflow(&[1.0, 2.0, 3.0]));
        assert!(DynamicLossScaler::check_overflow(&[1.0, f64::NAN, 3.0]));
        assert!(DynamicLossScaler::check_overflow(&[f64::INFINITY]));
    }

    // ── S7.8: Type ────────────────────────────────────────────────────

    #[test]
    fn bf16_display() {
        let bf = Bf16::from_f32(2.5);
        let s = format!("{bf}");
        assert!(s.contains("2.5"));
    }

    #[test]
    fn bf16_negative_zero() {
        let bf = Bf16::from_f32(-0.0);
        assert!(bf.is_zero());
    }

    // ── S7.10: Additional Coverage ────────────────────────────────────

    #[test]
    fn bf16_tensor_byte_size() {
        let bf = Bf16Tensor::from_tensor(&TensorValue::from_data(vec![1.0; 10], &[10]).unwrap());
        assert_eq!(bf.byte_size(), 20); // 10 * 2
    }

    #[test]
    fn bf16_tensor_empty() {
        let bf = Bf16Tensor::from_tensor(&TensorValue::from_data(vec![], &[0]).unwrap());
        assert!(bf.is_empty());
    }
}
