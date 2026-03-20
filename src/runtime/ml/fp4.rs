//! # FP4 Numeric Types
//!
//! 4-bit floating-point format with two-level scaling (NVIDIA NVFP4 style):
//!
//! | Field | Bits | Description |
//! |-------|------|-------------|
//! | Sign | 1 | Sign bit |
//! | Exponent | 2 | Biased exponent (bias 1) |
//! | Mantissa | 1 | Implicit leading 1 for normal |
//!
//! Representable values: 0, ±0.5, ±1.0, ±1.5, ±2.0, ±3.0, ±4.0, ±6.0
//!
//! ## Two-Level Scaling (NVFP4)
//!
//! Real value = `fp4_value × block_scale × tensor_scale`
//! - **Block scale**: per-32-element f16 scale factor
//! - **Tensor scale**: single f32 scale for the entire tensor

use super::tensor::{TensorError, TensorValue};
use serde::Serialize;

// ═══════════════════════════════════════════════════════════════════════
// FP4 E2M1 Format
// ═══════════════════════════════════════════════════════════════════════

/// 4-bit float with 2-bit exponent, 1-bit mantissa.
///
/// Bit layout: `[S][EE][M]` (4 bits)
/// - Exponent bias: 1
/// - Values: 0, ±0.5, ±1.0, ±1.5, ±2.0, ±3.0, ±4.0, ±6.0
/// - No Inf, no NaN
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub struct Fp4E2M1(pub u8); // Only lower 4 bits used

impl Fp4E2M1 {
    /// Zero value.
    pub const ZERO: Self = Self(0x0);
    /// Maximum positive value (6.0).
    pub const MAX: Self = Self(0x7);
    /// Minimum positive subnormal (0.5).
    pub const MIN_POSITIVE: Self = Self(0x1);

    const _EXP_BIAS: i32 = 1;

    /// All 8 non-negative representable values (indexed by 4-bit code).
    const VALUES: [f32; 8] = [0.0, 0.5, 1.0, 1.5, 2.0, 3.0, 4.0, 6.0];

    /// Convert from f32 to E2M1 (nearest value, saturating).
    pub fn from_f32(value: f32) -> Self {
        if value.is_nan() {
            return Self::ZERO; // No NaN in FP4
        }

        let sign = if value < 0.0 { 1u8 } else { 0u8 };
        let abs_val = value.abs();

        // Find nearest representable value
        let mut best_idx = 0u8;
        let mut best_dist = f32::MAX;

        for (idx, &repr) in Self::VALUES.iter().enumerate() {
            let dist = (abs_val - repr).abs();
            if dist < best_dist {
                best_dist = dist;
                best_idx = idx as u8;
            }
        }

        Self((sign << 3) | best_idx)
    }

    /// Convert E2M1 to f32 (exact).
    pub fn to_f32(self) -> f32 {
        let sign = (self.0 >> 3) & 1;
        let idx = self.0 & 0x07;
        let value = Self::VALUES[idx as usize];
        if sign == 1 { -value } else { value }
    }

    /// Check if zero.
    pub fn is_zero(self) -> bool {
        (self.0 & 0x07) == 0
    }

    /// Raw 4-bit representation.
    pub fn to_bits(self) -> u8 {
        self.0 & 0x0F
    }
}

impl std::fmt::Display for Fp4E2M1 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_f32())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Packing: 8 FP4 values per u32
// ═══════════════════════════════════════════════════════════════════════

/// Pack 8 FP4 values into a single u32.
///
/// Layout: `[v7:4][v6:4][v5:4][v4:4][v3:4][v2:4][v1:4][v0:4]`
pub fn pack_8_fp4(values: &[Fp4E2M1; 8]) -> u32 {
    let mut packed = 0u32;
    for (i, v) in values.iter().enumerate() {
        packed |= (v.to_bits() as u32) << (i * 4);
    }
    packed
}

/// Unpack a u32 into 8 FP4 values.
pub fn unpack_8_fp4(packed: u32) -> [Fp4E2M1; 8] {
    let mut values = [Fp4E2M1::ZERO; 8];
    for (i, v) in values.iter_mut().enumerate() {
        *v = Fp4E2M1(((packed >> (i * 4)) & 0x0F) as u8);
    }
    values
}

// ═══════════════════════════════════════════════════════════════════════
// FP4 Tensor with Two-Level Scaling
// ═══════════════════════════════════════════════════════════════════════

/// Default block size for FP4 quantization.
pub const FP4_BLOCK_SIZE: usize = 32;

/// A tensor quantized to FP4 with NVFP4-style two-level scaling.
///
/// Real value = `fp4_value × block_scale[block_idx] × tensor_scale`
#[derive(Debug, Clone)]
pub struct Fp4Tensor {
    /// Packed FP4 data (8 values per u32).
    packed_data: Vec<u32>,
    /// Per-block scale factors (one per `block_size` elements).
    block_scales: Vec<f32>,
    /// Global tensor-level scale factor.
    tensor_scale: f64,
    /// Tensor shape.
    shape: Vec<usize>,
    /// Block size (default 32).
    block_size: usize,
    /// Total number of elements (may not fill last u32 completely).
    num_elements: usize,
}

impl Fp4Tensor {
    /// Quantize a float tensor to FP4 with two-level scaling.
    ///
    /// Uses absmax calibration per block to compute block scales.
    pub fn quantize(tensor: &TensorValue, block_size: usize) -> Self {
        let values = tensor.to_vec();
        let n = values.len();
        let bs = if block_size == 0 {
            FP4_BLOCK_SIZE
        } else {
            block_size
        };

        // Tensor-level scale: global absmax
        let global_max = values.iter().map(|v| v.abs()).fold(0.0_f64, f64::max);
        let tensor_scale = if global_max == 0.0 {
            1.0
        } else {
            global_max / 6.0 // FP4 max representable = 6.0
        };

        // Per-block scales
        let num_blocks = n.div_ceil(bs);
        let mut block_scales = Vec::with_capacity(num_blocks);
        let mut fp4_values = Vec::with_capacity(n);

        for block_idx in 0..num_blocks {
            let start = block_idx * bs;
            let end = (start + bs).min(n);
            let block = &values[start..end];

            // Block absmax
            let block_max = block.iter().map(|v| v.abs()).fold(0.0_f64, f64::max);
            let block_scale = if block_max == 0.0 || tensor_scale == 0.0 {
                1.0_f32
            } else {
                (block_max / (tensor_scale * 6.0)) as f32
            };
            block_scales.push(block_scale);

            // Quantize each element
            for &val in block {
                let scaled = if block_scale == 0.0 || tensor_scale == 0.0 {
                    0.0
                } else {
                    (val / (tensor_scale * block_scale as f64)) as f32
                };
                fp4_values.push(Fp4E2M1::from_f32(scaled));
            }
        }

        // Pack into u32s (8 values per u32)
        let num_u32s = n.div_ceil(8);
        let mut packed_data = Vec::with_capacity(num_u32s);
        for chunk in fp4_values.chunks(8) {
            let mut group = [Fp4E2M1::ZERO; 8];
            for (i, &v) in chunk.iter().enumerate() {
                group[i] = v;
            }
            packed_data.push(pack_8_fp4(&group));
        }

        Self {
            packed_data,
            block_scales,
            tensor_scale,
            shape: tensor.shape().to_vec(),
            block_size: bs,
            num_elements: n,
        }
    }

    /// Dequantize back to a float tensor.
    pub fn dequantize(&self) -> TensorValue {
        let mut values = Vec::with_capacity(self.num_elements);

        for (u32_idx, &packed) in self.packed_data.iter().enumerate() {
            let unpacked = unpack_8_fp4(packed);
            for (sub_idx, &fp4) in unpacked.iter().enumerate() {
                let elem_idx = u32_idx * 8 + sub_idx;
                if elem_idx >= self.num_elements {
                    break;
                }
                let block_idx = elem_idx / self.block_size;
                let block_scale = self.block_scales.get(block_idx).copied().unwrap_or(1.0);
                let real_value = fp4.to_f32() as f64 * block_scale as f64 * self.tensor_scale;
                values.push(real_value);
            }
        }

        TensorValue::from_data(values, &self.shape)
            .unwrap_or_else(|_| TensorValue::zeros(&self.shape))
    }

    /// Number of elements.
    pub fn len(&self) -> usize {
        self.num_elements
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.num_elements == 0
    }

    /// Memory size in bytes (packed data + scales).
    pub fn byte_size(&self) -> usize {
        self.packed_data.len() * 4 + self.block_scales.len() * 4 + 8 // + tensor_scale
    }

    /// Compression ratio vs f64.
    pub fn compression_ratio(&self) -> f64 {
        if self.num_elements == 0 {
            return 1.0;
        }
        let original_bytes = self.num_elements * 8; // f64
        original_bytes as f64 / self.byte_size() as f64
    }

    /// Tensor shape.
    pub fn shape(&self) -> &[usize] {
        &self.shape
    }

    /// Tensor-level scale.
    pub fn tensor_scale(&self) -> f64 {
        self.tensor_scale
    }

    /// Block scales.
    pub fn block_scales(&self) -> &[f32] {
        &self.block_scales
    }

    /// Block size.
    pub fn block_size(&self) -> usize {
        self.block_size
    }

    /// Compute quantization error (RMSE) against original.
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
            / orig_vals.len().max(1) as f64;

        Ok(mse.sqrt())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── S6.1: E2M1 Format ─────────────────────────────────────────────

    #[test]
    fn fp4_zero() {
        assert_eq!(Fp4E2M1::ZERO.to_f32(), 0.0);
        assert!(Fp4E2M1::ZERO.is_zero());
    }

    #[test]
    fn fp4_all_positive_values() {
        let expected = [0.0, 0.5, 1.0, 1.5, 2.0, 3.0, 4.0, 6.0];
        for (i, &exp) in expected.iter().enumerate() {
            let fp4 = Fp4E2M1(i as u8);
            assert!(
                (fp4.to_f32() - exp).abs() < 1e-6,
                "fp4({i}) = {}, expected {exp}",
                fp4.to_f32()
            );
        }
    }

    #[test]
    fn fp4_negative_values() {
        let neg = Fp4E2M1::from_f32(-3.0);
        assert!((neg.to_f32() - (-3.0)).abs() < 1e-6);
    }

    #[test]
    fn fp4_max_value() {
        assert!((Fp4E2M1::MAX.to_f32() - 6.0).abs() < 1e-6);
    }

    #[test]
    fn fp4_range_minus6_to_6() {
        let neg_max = Fp4E2M1::from_f32(-6.0);
        assert!((neg_max.to_f32() - (-6.0)).abs() < 1e-6);
        let pos_max = Fp4E2M1::from_f32(6.0);
        assert!((pos_max.to_f32() - 6.0).abs() < 1e-6);
    }

    // ── S6.2: NVFP4 Two-Level Scaling ─────────────────────────────────

    #[test]
    fn fp4_tensor_has_block_scales() {
        let data: Vec<f64> = (0..64).map(|i| i as f64 * 0.1).collect();
        let tensor = TensorValue::from_data(data, &[64]).unwrap();
        let quantized = Fp4Tensor::quantize(&tensor, 32);
        assert_eq!(quantized.block_scales().len(), 2); // 64 / 32 = 2 blocks
    }

    #[test]
    fn fp4_tensor_tensor_scale_positive() {
        let tensor = TensorValue::from_data(vec![1.0, 2.0, 3.0], &[3]).unwrap();
        let quantized = Fp4Tensor::quantize(&tensor, 32);
        assert!(quantized.tensor_scale() > 0.0);
    }

    // ── S6.3: Packing 8 Values per u32 ────────────────────────────────

    #[test]
    fn pack_unpack_round_trip() {
        let values = [
            Fp4E2M1(0x0),
            Fp4E2M1(0x1),
            Fp4E2M1(0x2),
            Fp4E2M1(0x3),
            Fp4E2M1(0x4),
            Fp4E2M1(0x5),
            Fp4E2M1(0x6),
            Fp4E2M1(0x7),
        ];
        let packed = pack_8_fp4(&values);
        let unpacked = unpack_8_fp4(packed);
        for (i, (&orig, &recovered)) in values.iter().zip(unpacked.iter()).enumerate() {
            assert_eq!(orig.to_bits(), recovered.to_bits(), "Mismatch at index {i}");
        }
    }

    #[test]
    fn pack_negative_values() {
        let values = [
            Fp4E2M1(0x8), // -0
            Fp4E2M1(0x9), // -0.5
            Fp4E2M1(0xA), // -1.0
            Fp4E2M1(0xB), // -1.5
            Fp4E2M1(0xC), // -2.0
            Fp4E2M1(0xD), // -3.0
            Fp4E2M1(0xE), // -4.0
            Fp4E2M1(0xF), // -6.0
        ];
        let packed = pack_8_fp4(&values);
        let unpacked = unpack_8_fp4(packed);
        for (i, (&orig, &recovered)) in values.iter().zip(unpacked.iter()).enumerate() {
            assert_eq!(orig.to_bits(), recovered.to_bits(), "Mismatch at index {i}");
        }
    }

    // ── S6.4: FP4 Arithmetic ──────────────────────────────────────────

    #[test]
    fn fp4_round_trip_nearest() {
        // 2.5 should round to 2.0 or 3.0
        let fp4 = Fp4E2M1::from_f32(2.5);
        let val = fp4.to_f32();
        assert!(val == 2.0 || val == 3.0);
    }

    #[test]
    fn fp4_saturation() {
        // Values > 6.0 saturate to 6.0
        let fp4 = Fp4E2M1::from_f32(100.0);
        assert!((fp4.to_f32() - 6.0).abs() < 1e-6);
    }

    // ── S6.5/6.6: Conversion ──────────────────────────────────────────

    #[test]
    fn fp4_nan_becomes_zero() {
        let fp4 = Fp4E2M1::from_f32(f32::NAN);
        assert!(fp4.is_zero());
    }

    // ── S6.7: FP4 Tensor Storage ──────────────────────────────────────

    #[test]
    fn fp4_tensor_basic() {
        let tensor = TensorValue::from_data(vec![1.0, 2.0, 3.0, 4.0], &[4]).unwrap();
        let quantized = Fp4Tensor::quantize(&tensor, 32);
        assert_eq!(quantized.len(), 4);
        assert_eq!(quantized.shape(), &[4]);
    }

    #[test]
    fn fp4_tensor_compression() {
        let data: Vec<f64> = (0..256).map(|i| (i as f64) * 0.01).collect();
        let tensor = TensorValue::from_data(data, &[256]).unwrap();
        let quantized = Fp4Tensor::quantize(&tensor, 32);
        // 256 elements × 8 bytes = 2048 bytes original
        // 256/8 = 32 u32s = 128 bytes + 8 block scales × 4 = 32 + 8 = ~168 bytes
        assert!(quantized.compression_ratio() > 5.0);
    }

    // ── S6.8: Quantization API ────────────────────────────────────────

    #[test]
    fn fp4_tensor_dequantize_shape_preserved() {
        let tensor = TensorValue::from_data(vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0], &[2, 3]).unwrap();
        let quantized = Fp4Tensor::quantize(&tensor, 32);
        let back = quantized.dequantize();
        assert_eq!(back.shape(), &[2, 3]);
    }

    #[test]
    fn fp4_tensor_quantization_error() {
        let tensor = TensorValue::from_data(vec![1.0, 2.0, 3.0, 4.0], &[4]).unwrap();
        let quantized = Fp4Tensor::quantize(&tensor, 32);
        let error = quantized.quantization_error(&tensor).unwrap();
        assert!(error < 2.0, "FP4 quantization error too high: {error}");
    }

    // ── S6.9: Type System ─────────────────────────────────────────────

    #[test]
    fn fp4_display() {
        let fp4 = Fp4E2M1::from_f32(3.0);
        assert_eq!(format!("{fp4}"), "3");
    }

    // ── S6.10: Edge Cases ─────────────────────────────────────────────

    #[test]
    fn fp4_tensor_empty() {
        let tensor = TensorValue::from_data(vec![], &[0]).unwrap();
        let quantized = Fp4Tensor::quantize(&tensor, 32);
        assert!(quantized.is_empty());
    }

    #[test]
    fn fp4_tensor_single_element() {
        let tensor = TensorValue::from_data(vec![3.14], &[1]).unwrap();
        let quantized = Fp4Tensor::quantize(&tensor, 32);
        assert_eq!(quantized.len(), 1);
        let back = quantized.dequantize();
        assert_eq!(back.shape(), &[1]);
    }

    #[test]
    fn fp4_tensor_block_size() {
        let tensor = TensorValue::from_data(vec![1.0; 100], &[100]).unwrap();
        let quantized = Fp4Tensor::quantize(&tensor, 32);
        assert_eq!(quantized.block_size(), 32);
        // 100 / 32 = 4 blocks (ceil)
        assert_eq!(quantized.block_scales().len(), 4);
    }

    #[test]
    fn fp4_tensor_all_zeros() {
        let tensor = TensorValue::from_data(vec![0.0; 16], &[16]).unwrap();
        let quantized = Fp4Tensor::quantize(&tensor, 32);
        let back = quantized.dequantize();
        let vals = back.to_vec();
        for v in &vals {
            assert!((v - 0.0).abs() < 1e-10);
        }
    }
}
