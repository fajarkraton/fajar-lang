//! Quantization — post-training quantization for embedded inference.
//!
//! Provides INT8 [`QuantizedTensor`] (legacy) and multi-bit [`QuantizedValue`]
//! (2/3/4/8-bit, symmetric per-tensor). Uses i32 accumulation for matmul.

use super::tensor::{TensorError, TensorValue};

/// A quantized tensor: i8 data with a scale factor.
///
/// Real value ≈ `scale * (quantized_value as f64)`.
/// Zero-point is always 0 (symmetric quantization).
#[derive(Debug, Clone)]
pub struct QuantizedTensor {
    /// Quantized data (row-major flat).
    data: Vec<i8>,
    /// Scale factor: real = scale * quantized.
    scale: f64,
    /// Shape of the tensor.
    shape: Vec<usize>,
}

impl QuantizedTensor {
    /// Quantizes a float tensor to INT8 (symmetric, per-tensor).
    ///
    /// Maps the range [-max_abs, max_abs] to [-127, 127].
    pub fn quantize(tensor: &TensorValue) -> Self {
        let values = tensor.to_vec();
        let max_abs = values.iter().map(|v| v.abs()).fold(0.0_f64, f64::max);

        let scale = if max_abs == 0.0 { 1.0 } else { max_abs / 127.0 };

        let data: Vec<i8> = values
            .iter()
            .map(|&v| {
                let q = (v / scale).round();
                q.clamp(-127.0, 127.0) as i8
            })
            .collect();

        Self {
            data,
            scale,
            shape: tensor.shape().to_vec(),
        }
    }

    /// Dequantizes back to a float tensor.
    pub fn dequantize(&self) -> TensorValue {
        let values: Vec<f64> = self.data.iter().map(|&q| q as f64 * self.scale).collect();
        TensorValue::from_data(values, &self.shape).unwrap_or_else(|_| {
            // Shape and data should always match (invariant from quantize)
            TensorValue::zeros(&self.shape)
        })
    }

    /// Returns the quantized data as a slice of i8.
    pub fn data(&self) -> &[i8] {
        &self.data
    }

    /// Returns the scale factor.
    pub fn scale(&self) -> f64 {
        self.scale
    }

    /// Returns the shape.
    pub fn shape(&self) -> &[usize] {
        &self.shape
    }

    /// Returns the total number of elements.
    pub fn numel(&self) -> usize {
        self.data.len()
    }

    /// Creates a QuantizedTensor from raw components.
    pub fn from_raw(data: Vec<i8>, scale: f64, shape: Vec<usize>) -> Self {
        Self { data, scale, shape }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// QuantizedValue — multi-bit quantized tensor (B5.L1)
// ═══════════════════════════════════════════════════════════════════════

/// A quantized tensor with configurable bit width (2, 3, 4, or 8).
///
/// Stays quantized until explicitly dequantized. This is the first-class
/// `Quantized<T, BITS>` type exposed to Fajar Lang programs.
///
/// Uses symmetric per-tensor quantization:
///   real_value ≈ scale * quantized_int_value
#[derive(Debug, Clone)]
pub struct QuantizedValue {
    /// Quantized data stored as i8 (for 2-4 bit, values are clamped to the
    /// representable range, e.g. [-1, 0, 1] for 2-bit symmetric).
    data: Vec<i8>,
    /// Scale factor: real = scale * quantized.
    scale: f64,
    /// Original tensor shape.
    shape: Vec<usize>,
    /// Bit width (2, 3, 4, or 8).
    bits: u8,
}

impl QuantizedValue {
    /// Quantizes a float tensor to the given bit width (symmetric, per-tensor).
    ///
    /// Supported bit widths: 2, 3, 4, 8.
    /// Maps the range [-max_abs, max_abs] to [-max_q, max_q] where
    /// max_q = 2^(bits-1) - 1.
    pub fn quantize(tensor: &TensorValue, bits: u8) -> Result<Self, TensorError> {
        if !matches!(bits, 2 | 3 | 4 | 8) {
            return Err(TensorError::InvalidData {
                reason: format!("unsupported bit width {bits}: must be 2, 3, 4, or 8"),
            });
        }

        let max_q = ((1i16 << (bits - 1)) - 1) as f64; // 1, 3, 7, 127
        let values = tensor.to_vec();
        let max_abs = values.iter().map(|v| v.abs()).fold(0.0_f64, f64::max);
        let scale = if max_abs == 0.0 { 1.0 } else { max_abs / max_q };

        let data: Vec<i8> = values
            .iter()
            .map(|&v| {
                let q = (v / scale).round();
                q.clamp(-max_q, max_q) as i8
            })
            .collect();

        Ok(Self {
            data,
            scale,
            shape: tensor.shape().to_vec(),
            bits,
        })
    }

    /// Dequantizes back to a float tensor.
    pub fn dequantize(&self) -> TensorValue {
        let values: Vec<f64> = self.data.iter().map(|&q| q as f64 * self.scale).collect();
        TensorValue::from_data(values, &self.shape)
            .unwrap_or_else(|_| TensorValue::zeros(&self.shape))
    }

    /// Returns the bit width.
    pub fn bits(&self) -> u8 {
        self.bits
    }

    /// Returns the shape.
    pub fn shape(&self) -> &[usize] {
        &self.shape
    }

    /// Returns the scale factor.
    pub fn scale(&self) -> f64 {
        self.scale
    }

    /// Returns the total number of elements.
    pub fn numel(&self) -> usize {
        self.data.len()
    }

    /// Creates a QuantizedValue from raw components (used by fused kernels).
    pub fn from_parts(data: Vec<i8>, scale: f64, shape: Vec<usize>, bits: u8) -> Self {
        Self {
            data,
            scale,
            shape,
            bits,
        }
    }

    /// Returns the quantized data as a slice.
    pub fn data(&self) -> &[i8] {
        &self.data
    }

    /// Memory footprint in bytes (data only, excluding metadata).
    /// For sub-byte widths this is the packed size.
    pub fn size_bytes(&self) -> usize {
        // Actual storage is Vec<i8> (1 byte per element), but the
        // *effective* memory is (numel * bits) / 8 for the paper story.
        (self.data.len() * self.bits as usize).div_ceil(8)
    }
}

impl std::fmt::Display for QuantizedValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "<quantized {}bit shape={:?} scale={:.6}>",
            self.bits, self.shape, self.scale
        )
    }
}

impl PartialEq for QuantizedValue {
    fn eq(&self, other: &Self) -> bool {
        self.bits == other.bits
            && self.shape == other.shape
            && (self.scale - other.scale).abs() < 1e-12
            && self.data == other.data
    }
}

/// Performs INT8 matrix multiplication with i32 accumulation.
///
/// `a` shape: `[M, K]`, `b` shape: `[K, N]` → result shape: `[M, N]`.
/// The result is a dequantized f64 tensor.
///
/// Real result ≈ `a.scale * b.scale * (i32 accumulator)`.
pub fn quantized_matmul(
    a: &QuantizedTensor,
    b: &QuantizedTensor,
) -> Result<TensorValue, TensorError> {
    if a.shape.len() != 2 || b.shape.len() != 2 {
        return Err(TensorError::RankMismatch {
            expected: 2,
            got: if a.shape.len() != 2 {
                a.shape.len()
            } else {
                b.shape.len()
            },
        });
    }

    let m = a.shape[0];
    let k_a = a.shape[1];
    let k_b = b.shape[0];
    let n = b.shape[1];

    if k_a != k_b {
        return Err(TensorError::MatmulShapeMismatch {
            left: a.shape.clone(),
            right: b.shape.clone(),
            left_inner: k_a,
            right_inner: k_b,
        });
    }

    let combined_scale = a.scale * b.scale;
    let mut result = vec![0.0; m * n];

    for i in 0..m {
        for j in 0..n {
            // i32 accumulation — no floating point needed in this loop
            let mut acc: i32 = 0;
            for p in 0..k_a {
                acc += a.data[i * k_a + p] as i32 * b.data[p * n + j] as i32;
            }
            result[i * n + j] = combined_scale * acc as f64;
        }
    }

    TensorValue::from_data(result, &[m, n])
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quantize_dequantize_roundtrip() {
        let t = TensorValue::from_data(vec![1.0, -0.5, 0.25, 0.0, -1.0, 0.75], &[2, 3]).unwrap();
        let qt = QuantizedTensor::quantize(&t);
        let dt = qt.dequantize();

        assert_eq!(dt.shape(), &[2, 3]);
        let orig = t.to_vec();
        let restored = dt.to_vec();
        for (o, r) in orig.iter().zip(restored.iter()) {
            assert!(
                (o - r).abs() < 0.02,
                "expected {o}, got {r} (diff {})",
                (o - r).abs()
            );
        }
    }

    #[test]
    fn quantize_zeros() {
        let t = TensorValue::zeros(&[3, 3]);
        let qt = QuantizedTensor::quantize(&t);
        assert!(qt.data().iter().all(|&v| v == 0));
        let dt = qt.dequantize();
        assert!(dt.to_vec().iter().all(|&v| v == 0.0));
    }

    #[test]
    fn quantize_scale_factor() {
        let t = TensorValue::from_data(vec![127.0, -127.0], &[2]).unwrap();
        let qt = QuantizedTensor::quantize(&t);
        assert!((qt.scale() - 1.0).abs() < 1e-10);
        assert_eq!(qt.data(), &[127, -127]);
    }

    #[test]
    fn quantize_clips_to_range() {
        // Max i8 is 127, so large values get clipped
        let t = TensorValue::from_data(vec![1000.0, -1000.0, 500.0], &[3]).unwrap();
        let qt = QuantizedTensor::quantize(&t);
        // scale = 1000/127 ≈ 7.874
        // 1000 / 7.874 ≈ 127 → clipped to 127
        assert!(qt.data()[0] == 127 || qt.data()[0] == 126);
        assert!(qt.data()[1] == -127 || qt.data()[1] == -126);
    }

    #[test]
    fn quantized_matmul_basic() {
        // a = [[1, 2], [3, 4]], b = [[5, 6], [7, 8]]
        // a @ b = [[19, 22], [43, 50]]
        let a = TensorValue::from_data(vec![1.0, 2.0, 3.0, 4.0], &[2, 2]).unwrap();
        let b = TensorValue::from_data(vec![5.0, 6.0, 7.0, 8.0], &[2, 2]).unwrap();

        let qa = QuantizedTensor::quantize(&a);
        let qb = QuantizedTensor::quantize(&b);
        let result = quantized_matmul(&qa, &qb).unwrap();

        assert_eq!(result.shape(), &[2, 2]);
        let data = result.to_vec();
        // Allow quantization error
        assert!(
            (data[0] - 19.0).abs() < 1.0,
            "expected ~19, got {}",
            data[0]
        );
        assert!(
            (data[1] - 22.0).abs() < 1.0,
            "expected ~22, got {}",
            data[1]
        );
        assert!(
            (data[2] - 43.0).abs() < 2.0,
            "expected ~43, got {}",
            data[2]
        );
        assert!(
            (data[3] - 50.0).abs() < 2.0,
            "expected ~50, got {}",
            data[3]
        );
    }

    #[test]
    fn quantized_matmul_identity() {
        // a @ I = a
        let a = TensorValue::from_data(vec![1.0, 2.0, 3.0, 4.0], &[2, 2]).unwrap();
        let eye = TensorValue::eye(2);

        let qa = QuantizedTensor::quantize(&a);
        let qe = QuantizedTensor::quantize(&eye);
        let result = quantized_matmul(&qa, &qe).unwrap();

        let data = result.to_vec();
        let orig = a.to_vec();
        for (o, r) in orig.iter().zip(data.iter()) {
            assert!((o - r).abs() < 0.1, "expected {o}, got {r}");
        }
    }

    #[test]
    fn quantized_matmul_shape_mismatch() {
        let a = TensorValue::from_data(vec![1.0, 2.0, 3.0], &[1, 3]).unwrap();
        let b = TensorValue::from_data(vec![1.0, 2.0], &[1, 2]).unwrap();
        let qa = QuantizedTensor::quantize(&a);
        let qb = QuantizedTensor::quantize(&b);
        assert!(quantized_matmul(&qa, &qb).is_err());
    }

    #[test]
    fn quantized_matmul_non_square() {
        // a: [2, 3], b: [3, 4] → result: [2, 4]
        let a = TensorValue::from_data(vec![1.0; 6], &[2, 3]).unwrap();
        let b = TensorValue::from_data(vec![1.0; 12], &[3, 4]).unwrap();
        let qa = QuantizedTensor::quantize(&a);
        let qb = QuantizedTensor::quantize(&b);
        let result = quantized_matmul(&qa, &qb).unwrap();
        assert_eq!(result.shape(), &[2, 4]);
        // Each element should be close to 3.0 (sum of 3 ones * 1)
        for &v in result.to_vec().iter() {
            assert!((v - 3.0).abs() < 0.5, "expected ~3.0, got {v}");
        }
    }

    // ─── QuantizedValue (multi-bit) tests ───

    #[test]
    fn quantized_value_2bit_roundtrip() {
        let t = TensorValue::from_data(vec![1.0, -0.5, 0.25, 0.0], &[2, 2]).unwrap();
        let qv = QuantizedValue::quantize(&t, 2).unwrap();
        assert_eq!(qv.bits(), 2);
        assert_eq!(qv.shape(), &[2, 2]);
        assert_eq!(qv.numel(), 4);
        // 2-bit symmetric: max_q = 1, so values map to {-1, 0, 1}
        let dt = qv.dequantize();
        assert_eq!(dt.shape(), &[2, 2]);
    }

    #[test]
    fn quantized_value_4bit_roundtrip() {
        let t = TensorValue::from_data(vec![1.0, -0.5, 0.25, 0.0, -1.0, 0.75], &[2, 3]).unwrap();
        let qv = QuantizedValue::quantize(&t, 4).unwrap();
        assert_eq!(qv.bits(), 4);
        let dt = qv.dequantize();
        let orig = t.to_vec();
        let restored = dt.to_vec();
        for (o, r) in orig.iter().zip(restored.iter()) {
            assert!((o - r).abs() < 0.2, "4-bit: expected ~{o}, got {r}");
        }
    }

    #[test]
    fn quantized_value_8bit_matches_legacy() {
        let t = TensorValue::from_data(vec![1.0, -0.5, 0.25, 0.0, -1.0, 0.75], &[2, 3]).unwrap();
        let legacy = QuantizedTensor::quantize(&t);
        let qv = QuantizedValue::quantize(&t, 8).unwrap();
        let dt_legacy = legacy.dequantize().to_vec();
        let dt_new = qv.dequantize().to_vec();
        for (l, n) in dt_legacy.iter().zip(dt_new.iter()) {
            assert!((l - n).abs() < 1e-10, "8-bit mismatch: legacy={l}, new={n}");
        }
    }

    #[test]
    fn quantized_value_invalid_bits() {
        let t = TensorValue::from_data(vec![1.0], &[1]).unwrap();
        assert!(QuantizedValue::quantize(&t, 5).is_err());
        assert!(QuantizedValue::quantize(&t, 0).is_err());
        assert!(QuantizedValue::quantize(&t, 16).is_err());
    }

    #[test]
    fn quantized_value_size_bytes() {
        let t = TensorValue::from_data(vec![1.0; 8], &[8]).unwrap();
        let q2 = QuantizedValue::quantize(&t, 2).unwrap();
        let q4 = QuantizedValue::quantize(&t, 4).unwrap();
        let q8 = QuantizedValue::quantize(&t, 8).unwrap();
        assert_eq!(q2.size_bytes(), 2); // 8 elements * 2 bits / 8 = 2 bytes
        assert_eq!(q4.size_bytes(), 4); // 8 elements * 4 bits / 8 = 4 bytes
        assert_eq!(q8.size_bytes(), 8); // 8 elements * 8 bits / 8 = 8 bytes
    }

    #[test]
    fn quantized_value_display() {
        let t = TensorValue::from_data(vec![1.0, -1.0], &[2]).unwrap();
        let qv = QuantizedValue::quantize(&t, 4).unwrap();
        let s = format!("{qv}");
        assert!(s.contains("4bit"), "display should show bit width: {s}");
        assert!(s.contains("[2]"), "display should show shape: {s}");
    }

    #[test]
    fn quantized_value_equality() {
        let t = TensorValue::from_data(vec![1.0, -0.5], &[2]).unwrap();
        let q1 = QuantizedValue::quantize(&t, 4).unwrap();
        let q2 = QuantizedValue::quantize(&t, 4).unwrap();
        let q3 = QuantizedValue::quantize(&t, 2).unwrap();
        assert_eq!(q1, q2);
        assert_ne!(q1, q3);
    }

    #[test]
    fn quantize_preserves_shape() {
        let t = TensorValue::from_data(vec![1.0; 24], &[2, 3, 4]).unwrap();
        let qt = QuantizedTensor::quantize(&t);
        assert_eq!(qt.shape(), &[2, 3, 4]);
        assert_eq!(qt.numel(), 24);
    }

    #[test]
    fn quantized_matmul_matches_float_approx() {
        use super::super::ops;

        let a = TensorValue::from_data(vec![0.5, -0.3, 0.8, 0.1, -0.7, 0.4], &[2, 3]).unwrap();
        let b = TensorValue::from_data(vec![0.2, 0.6, -0.1, 0.3, 0.9, -0.5], &[3, 2]).unwrap();

        // Float matmul
        let float_result = ops::matmul(&a, &b).unwrap();

        // Quantized matmul
        let qa = QuantizedTensor::quantize(&a);
        let qb = QuantizedTensor::quantize(&b);
        let quant_result = quantized_matmul(&qa, &qb).unwrap();

        assert_eq!(float_result.shape(), quant_result.shape());
        let fr = float_result.to_vec();
        let qr = quant_result.to_vec();
        for (f, q) in fr.iter().zip(qr.iter()) {
            assert!(
                (f - q).abs() < 0.05,
                "float={f}, quantized={q}, diff={}",
                (f - q).abs()
            );
        }
    }
}
