//! # NPU Runtime Backend
//!
//! Dispatch infrastructure for Intel OpenVINO, AMD XDNA, and future NPU
//! backends. All backends are feature-gated; at runtime the best available
//! backend is selected via the accelerator registry.
//!
//! ## Backends
//!
//! | Backend | Feature Gate | NPU Vendor |
//! |---------|-------------|------------|
//! | OpenVINO | `--features openvino` | Intel (Meteor Lake, Lunar Lake) |
//! | XDNA | `--features xdna` | AMD (Ryzen AI 300+) |
//!
//! ## Pipeline
//!
//! ```text
//! ONNX model → optimize → quantize → partition → compile → run
//! ```

pub mod openvino;
pub mod pipeline;
pub mod qnn;
pub mod xdna;

use serde::Serialize;

// ═══════════════════════════════════════════════════════════════════════
// NPU Backend Trait
// ═══════════════════════════════════════════════════════════════════════

/// Error from NPU execution.
#[derive(Debug, Clone, thiserror::Error)]
pub enum NpuRuntimeError {
    /// Backend not available (driver/library missing).
    #[error("NPU backend '{backend}' not available: {reason}")]
    BackendUnavailable {
        /// Backend name.
        backend: String,
        /// Reason for unavailability.
        reason: String,
    },

    /// Model compilation failed.
    #[error("NPU model compilation failed: {0}")]
    CompilationFailed(String),

    /// Inference execution failed.
    #[error("NPU inference failed: {0}")]
    InferenceFailed(String),

    /// Shape mismatch between input and model expectation.
    #[error("NPU input shape mismatch: expected {expected:?}, got {got:?}")]
    ShapeMismatch {
        /// Expected shape.
        expected: Vec<usize>,
        /// Actual shape.
        got: Vec<usize>,
    },

    /// Unsupported data type for this NPU.
    #[error("NPU does not support data type '{dtype}'")]
    UnsupportedDtype {
        /// The unsupported type name.
        dtype: String,
    },
}

/// Data type for NPU tensors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum NpuDtype {
    /// 32-bit float.
    F32,
    /// 16-bit float.
    F16,
    /// BFloat16.
    BF16,
    /// 8-bit signed integer.
    INT8,
    /// 8-bit unsigned integer.
    UINT8,
}

impl std::fmt::Display for NpuDtype {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NpuDtype::F32 => write!(f, "f32"),
            NpuDtype::F16 => write!(f, "f16"),
            NpuDtype::BF16 => write!(f, "bf16"),
            NpuDtype::INT8 => write!(f, "int8"),
            NpuDtype::UINT8 => write!(f, "uint8"),
        }
    }
}

/// A compiled NPU model ready for inference.
#[derive(Debug, Clone)]
pub struct NpuCompiledModel {
    /// Backend that compiled this model.
    pub backend: String,
    /// Model name / identifier.
    pub name: String,
    /// Input shapes.
    pub input_shapes: Vec<Vec<usize>>,
    /// Output shapes.
    pub output_shapes: Vec<Vec<usize>>,
    /// Input data types.
    pub input_dtypes: Vec<NpuDtype>,
    /// Output data types.
    pub output_dtypes: Vec<NpuDtype>,
}

/// Result of an NPU inference run.
#[derive(Debug, Clone)]
pub struct NpuInferenceResult {
    /// Output tensor data (flattened f64).
    pub outputs: Vec<Vec<f64>>,
    /// Output shapes.
    pub output_shapes: Vec<Vec<usize>>,
    /// Inference latency in microseconds.
    pub latency_us: u64,
}

// ═══════════════════════════════════════════════════════════════════════
// QNN Buffer — Tensor ↔ NPU data conversion (Sprint 12.6 / 12.7)
// ═══════════════════════════════════════════════════════════════════════

/// Quantization parameters for QNN buffer conversion.
///
/// QNN HTP (Hexagon Tensor Processor) uses **asymmetric affine** quantization:
///
/// ```text
/// quantized = clamp(round(real / scale) + zero_point, qmin, qmax)
/// real      = (quantized - zero_point) * scale
/// ```
#[derive(Debug, Clone)]
pub struct QnnQuantParams {
    /// Scale factor: maps one quantum step to a real-value range.
    pub scale: f64,
    /// Zero point: the quantized value that represents real 0.0.
    pub zero_point: i32,
    /// Target data type.
    pub dtype: NpuDtype,
}

/// A buffer holding data in a QNN-compatible format.
///
/// This is the bridge between Fajar Lang's `TensorValue` (f64) and the
/// raw byte buffers that QNN SDK expects for inference.
#[derive(Debug, Clone)]
pub struct QnnBuffer {
    /// Raw quantized/converted data bytes.
    data: Vec<u8>,
    /// Tensor shape (e.g., [1, 224, 224, 3]).
    shape: Vec<usize>,
    /// Data type of the buffer contents.
    dtype: NpuDtype,
    /// Quantization parameters (for INT8/UINT8; scale=1.0, zp=0 for float types).
    quant_params: QnnQuantParams,
}

impl QnnBuffer {
    /// Converts a Fajar Lang tensor (f64) to a QNN-compatible buffer.
    ///
    /// Supported target dtypes:
    /// - `UINT8`: Asymmetric affine quantization (min/max calibration). Standard for QNN HTP.
    /// - `INT8`:  Symmetric quantization (zero_point=0). Compatible with existing `QuantizedTensor`.
    /// - `F32`:   Direct f64→f32 cast. For QNN CPU/GPU backends.
    /// - `F16`:   f64→f16 via f32 intermediate. For QNN GPU backend.
    pub fn from_tensor(
        tensor: &super::tensor::TensorValue,
        dtype: NpuDtype,
    ) -> Result<Self, NpuRuntimeError> {
        let values = tensor.to_vec();
        let shape = tensor.shape().to_vec();

        let (data, quant_params) = match dtype {
            NpuDtype::UINT8 => quantize_asymmetric_uint8(&values),
            NpuDtype::INT8 => quantize_symmetric_int8(&values),
            NpuDtype::F32 => convert_to_f32(&values),
            NpuDtype::F16 => convert_to_f16(&values),
            NpuDtype::BF16 => convert_to_bf16(&values),
        };

        Ok(Self {
            data,
            shape,
            dtype,
            quant_params,
        })
    }

    /// Converts a QNN buffer back to a Fajar Lang tensor (f64).
    ///
    /// Applies dequantization for integer types or widening for float types.
    pub fn to_tensor(&self) -> Result<super::tensor::TensorValue, NpuRuntimeError> {
        let values = match self.dtype {
            NpuDtype::UINT8 => dequantize_asymmetric_uint8(&self.data, &self.quant_params),
            NpuDtype::INT8 => dequantize_symmetric_int8(&self.data, &self.quant_params),
            NpuDtype::F32 => convert_from_f32(&self.data),
            NpuDtype::F16 => convert_from_f16(&self.data),
            NpuDtype::BF16 => convert_from_bf16(&self.data),
        };

        super::tensor::TensorValue::from_data(values, &self.shape).map_err(|e| {
            NpuRuntimeError::InferenceFailed(format!("buffer→tensor shape error: {e}"))
        })
    }

    /// Creates a QnnBuffer from raw bytes (e.g., QNN inference output).
    pub fn from_raw(
        data: Vec<u8>,
        shape: Vec<usize>,
        dtype: NpuDtype,
        quant_params: QnnQuantParams,
    ) -> Self {
        Self {
            data,
            shape,
            dtype,
            quant_params,
        }
    }

    /// Returns the raw data bytes.
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Returns the buffer shape.
    pub fn shape(&self) -> &[usize] {
        &self.shape
    }

    /// Returns the data type.
    pub fn dtype(&self) -> NpuDtype {
        self.dtype
    }

    /// Returns the quantization parameters.
    pub fn quant_params(&self) -> &QnnQuantParams {
        &self.quant_params
    }

    /// Returns the number of elements.
    pub fn numel(&self) -> usize {
        self.shape.iter().product()
    }

    /// Returns the buffer size in bytes.
    pub fn byte_size(&self) -> usize {
        self.data.len()
    }
}

// ── UINT8 asymmetric quantization (standard for QNN HTP) ──

/// Quantizes f64 values to UINT8 using asymmetric affine quantization.
///
/// Maps [min, max] → [0, 255] with per-tensor scale and zero_point.
fn quantize_asymmetric_uint8(values: &[f64]) -> (Vec<u8>, QnnQuantParams) {
    if values.is_empty() {
        return (
            Vec::new(),
            QnnQuantParams {
                scale: 1.0,
                zero_point: 0,
                dtype: NpuDtype::UINT8,
            },
        );
    }

    let min_val = values.iter().copied().fold(f64::INFINITY, f64::min);
    let max_val = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);

    // Ensure range includes 0.0 for correct zero_point
    let min_val = min_val.min(0.0);
    let max_val = max_val.max(0.0);

    let range = max_val - min_val;
    let scale = if range == 0.0 { 1.0 } else { range / 255.0 };
    let zero_point = (-min_val / scale).round() as i32;
    let zero_point = zero_point.clamp(0, 255);

    let data: Vec<u8> = values
        .iter()
        .map(|&v| {
            let q = (v / scale).round() as i32 + zero_point;
            q.clamp(0, 255) as u8
        })
        .collect();

    (
        data,
        QnnQuantParams {
            scale,
            zero_point,
            dtype: NpuDtype::UINT8,
        },
    )
}

/// Dequantizes UINT8 data back to f64.
fn dequantize_asymmetric_uint8(data: &[u8], params: &QnnQuantParams) -> Vec<f64> {
    data.iter()
        .map(|&q| (q as i32 - params.zero_point) as f64 * params.scale)
        .collect()
}

// ── INT8 symmetric quantization ──

/// Quantizes f64 values to INT8 using symmetric quantization (zero_point=0).
fn quantize_symmetric_int8(values: &[f64]) -> (Vec<u8>, QnnQuantParams) {
    let max_abs = values.iter().map(|v| v.abs()).fold(0.0_f64, f64::max);
    let scale = if max_abs == 0.0 { 1.0 } else { max_abs / 127.0 };

    let data: Vec<u8> = values
        .iter()
        .map(|&v| {
            let q = (v / scale).round().clamp(-127.0, 127.0) as i8;
            q as u8 // Store i8 as u8 bytes
        })
        .collect();

    (
        data,
        QnnQuantParams {
            scale,
            zero_point: 0,
            dtype: NpuDtype::INT8,
        },
    )
}

/// Dequantizes INT8 data back to f64.
fn dequantize_symmetric_int8(data: &[u8], params: &QnnQuantParams) -> Vec<f64> {
    data.iter()
        .map(|&q| (q as i8) as f64 * params.scale)
        .collect()
}

// ── F32 conversion ──

/// Converts f64 values to F32 bytes (little-endian).
fn convert_to_f32(values: &[f64]) -> (Vec<u8>, QnnQuantParams) {
    let data: Vec<u8> = values
        .iter()
        .flat_map(|&v| (v as f32).to_le_bytes())
        .collect();
    (
        data,
        QnnQuantParams {
            scale: 1.0,
            zero_point: 0,
            dtype: NpuDtype::F32,
        },
    )
}

/// Converts F32 bytes (little-endian) back to f64 values.
fn convert_from_f32(data: &[u8]) -> Vec<f64> {
    data.chunks_exact(4)
        .map(|chunk| {
            let bytes: [u8; 4] = [chunk[0], chunk[1], chunk[2], chunk[3]];
            f32::from_le_bytes(bytes) as f64
        })
        .collect()
}

// ── F16 conversion (IEEE 754 half-precision) ──

/// Converts f64 values to F16 bytes (little-endian).
fn convert_to_f16(values: &[f64]) -> (Vec<u8>, QnnQuantParams) {
    let data: Vec<u8> = values.iter().flat_map(|&v| f64_to_f16_bytes(v)).collect();
    (
        data,
        QnnQuantParams {
            scale: 1.0,
            zero_point: 0,
            dtype: NpuDtype::F16,
        },
    )
}

/// Converts F16 bytes (little-endian) back to f64 values.
fn convert_from_f16(data: &[u8]) -> Vec<f64> {
    data.chunks_exact(2)
        .map(|chunk| {
            let bytes: [u8; 2] = [chunk[0], chunk[1]];
            f16_bytes_to_f64(bytes)
        })
        .collect()
}

/// Converts an f64 to IEEE 754 half-precision (2 bytes, little-endian).
fn f64_to_f16_bytes(v: f64) -> [u8; 2] {
    let f = v as f32;
    let bits = f.to_bits();
    let sign = (bits >> 31) & 1;
    let exp = ((bits >> 23) & 0xFF) as i32;
    let frac = bits & 0x7FFFFF;

    let h = if exp == 0xFF {
        // Inf/NaN
        (sign << 15) | 0x7C00 | if frac != 0 { 0x0200 } else { 0 }
    } else if exp > 142 {
        // Overflow → Inf
        (sign << 15) | 0x7C00
    } else if exp < 113 {
        // Underflow → zero (flush to zero for simplicity)
        sign << 15
    } else {
        let new_exp = ((exp - 127 + 15) as u32) & 0x1F;
        let new_frac = frac >> 13;
        (sign << 15) | (new_exp << 10) | new_frac
    };
    (h as u16).to_le_bytes()
}

/// Converts IEEE 754 half-precision bytes (little-endian) to f64.
fn f16_bytes_to_f64(bytes: [u8; 2]) -> f64 {
    let h = u16::from_le_bytes(bytes) as u32;
    let sign = (h >> 15) & 1;
    let exp = (h >> 10) & 0x1F;
    let frac = h & 0x3FF;

    let f32_bits = if exp == 0 {
        if frac == 0 {
            sign << 31 // Zero
        } else {
            // Subnormal: convert to f32 normal
            let mut e = 113_u32; // 127 - 14
            let mut f = frac;
            while (f & 0x400) == 0 {
                f <<= 1;
                e -= 1;
            }
            f &= 0x3FF;
            (sign << 31) | (e << 23) | (f << 13)
        }
    } else if exp == 0x1F {
        // Inf/NaN
        (sign << 31) | 0x7F800000 | if frac != 0 { frac << 13 } else { 0 }
    } else {
        let new_exp = exp + 112; // (exp - 15) + 127
        (sign << 31) | (new_exp << 23) | (frac << 13)
    };
    f32::from_bits(f32_bits) as f64
}

// ── BF16 conversion (Brain Float 16) ──

/// Converts f64 values to BF16 bytes (little-endian).
fn convert_to_bf16(values: &[f64]) -> (Vec<u8>, QnnQuantParams) {
    let data: Vec<u8> = values
        .iter()
        .flat_map(|&v| {
            let f = v as f32;
            let bits = f.to_bits();
            // BF16 = upper 16 bits of f32 (truncation)
            ((bits >> 16) as u16).to_le_bytes()
        })
        .collect();
    (
        data,
        QnnQuantParams {
            scale: 1.0,
            zero_point: 0,
            dtype: NpuDtype::BF16,
        },
    )
}

/// Converts BF16 bytes (little-endian) back to f64 values.
fn convert_from_bf16(data: &[u8]) -> Vec<f64> {
    data.chunks_exact(2)
        .map(|chunk| {
            let bf16 = u16::from_le_bytes([chunk[0], chunk[1]]);
            // BF16 → f32: shift left 16 bits, lower 16 = 0
            let f32_bits = (bf16 as u32) << 16;
            f32::from_bits(f32_bits) as f64
        })
        .collect()
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn npu_dtype_display() {
        assert_eq!(format!("{}", NpuDtype::F32), "f32");
        assert_eq!(format!("{}", NpuDtype::INT8), "int8");
        assert_eq!(format!("{}", NpuDtype::BF16), "bf16");
    }

    #[test]
    fn npu_runtime_error_display() {
        let err = NpuRuntimeError::BackendUnavailable {
            backend: "openvino".to_string(),
            reason: "libopenvino.so not found".to_string(),
        };
        assert!(format!("{err}").contains("openvino"));
    }

    #[test]
    fn npu_compiled_model_fields() {
        let model = NpuCompiledModel {
            backend: "openvino".to_string(),
            name: "resnet18".to_string(),
            input_shapes: vec![vec![1, 3, 224, 224]],
            output_shapes: vec![vec![1, 1000]],
            input_dtypes: vec![NpuDtype::F32],
            output_dtypes: vec![NpuDtype::F32],
        };
        assert_eq!(model.backend, "openvino");
        assert_eq!(model.input_shapes[0], vec![1, 3, 224, 224]);
    }

    #[test]
    fn npu_inference_result_fields() {
        let result = NpuInferenceResult {
            outputs: vec![vec![0.1, 0.9]],
            output_shapes: vec![vec![1, 2]],
            latency_us: 1500,
        };
        assert_eq!(result.latency_us, 1500);
        assert_eq!(result.outputs[0].len(), 2);
    }

    #[test]
    fn npu_shape_mismatch_error() {
        let err = NpuRuntimeError::ShapeMismatch {
            expected: vec![1, 3, 224, 224],
            got: vec![1, 3, 128, 128],
        };
        let msg = format!("{err}");
        assert!(msg.contains("224"));
        assert!(msg.contains("128"));
    }

    // ── QNN Buffer conversion tests (Sprint 12.6/12.7) ──

    fn make_tensor(data: Vec<f64>, shape: &[usize]) -> crate::runtime::ml::tensor::TensorValue {
        crate::runtime::ml::tensor::TensorValue::from_data(data, shape).unwrap()
    }

    // --- UINT8 asymmetric quantization ---

    #[test]
    fn qnn_uint8_roundtrip_positive_values() {
        let values = vec![0.0, 0.5, 1.0, 1.5, 2.0];
        let tensor = make_tensor(values.clone(), &[5]);
        let buf = QnnBuffer::from_tensor(&tensor, NpuDtype::UINT8).unwrap();
        assert_eq!(buf.dtype(), NpuDtype::UINT8);
        assert_eq!(buf.shape(), &[5]);
        assert_eq!(buf.numel(), 5);
        assert_eq!(buf.byte_size(), 5); // 1 byte per UINT8 element
        let restored = buf.to_tensor().unwrap();
        let restored_vals = restored.to_vec();
        for (&orig, &restored) in values.iter().zip(restored_vals.iter()) {
            assert!(
                (orig - restored).abs() < 0.02,
                "UINT8 roundtrip: {orig} vs {restored}"
            );
        }
    }

    #[test]
    fn qnn_uint8_roundtrip_negative_values() {
        let values = vec![-1.0, -0.5, 0.0, 0.5, 1.0];
        let tensor = make_tensor(values.clone(), &[5]);
        let buf = QnnBuffer::from_tensor(&tensor, NpuDtype::UINT8).unwrap();
        let restored = buf.to_tensor().unwrap();
        let restored_vals = restored.to_vec();
        for (&orig, &restored) in values.iter().zip(restored_vals.iter()) {
            assert!(
                (orig - restored).abs() < 0.02,
                "UINT8 neg roundtrip: {orig} vs {restored}"
            );
        }
    }

    #[test]
    fn qnn_uint8_zero_point_maps_zero() {
        let values = vec![-1.0, 0.0, 1.0];
        let (data, params) = quantize_asymmetric_uint8(&values);
        // The quantized value at zero_point should dequantize to ~0.0
        let dequant = dequantize_asymmetric_uint8(&data, &params);
        assert!(
            dequant[1].abs() < 0.01,
            "Zero maps back to ~0.0: got {}",
            dequant[1]
        );
    }

    #[test]
    fn qnn_uint8_empty_tensor() {
        let tensor = make_tensor(vec![], &[0]);
        let buf = QnnBuffer::from_tensor(&tensor, NpuDtype::UINT8).unwrap();
        assert_eq!(buf.byte_size(), 0);
        assert_eq!(buf.numel(), 0);
    }

    #[test]
    fn qnn_uint8_all_same_values() {
        let values = vec![3.0, 3.0, 3.0, 3.0];
        let (data, params) = quantize_asymmetric_uint8(&values);
        // All values identical → scale covers [0, 3.0]
        assert!(params.scale > 0.0);
        let dequant = dequantize_asymmetric_uint8(&data, &params);
        for v in &dequant {
            assert!((v - 3.0).abs() < 0.05, "same-value roundtrip: {v}");
        }
    }

    // --- INT8 symmetric quantization ---

    #[test]
    fn qnn_int8_roundtrip() {
        let values = vec![-2.0, -1.0, 0.0, 1.0, 2.0];
        let tensor = make_tensor(values.clone(), &[5]);
        let buf = QnnBuffer::from_tensor(&tensor, NpuDtype::INT8).unwrap();
        assert_eq!(buf.dtype(), NpuDtype::INT8);
        assert_eq!(buf.byte_size(), 5);
        let restored = buf.to_tensor().unwrap();
        let restored_vals = restored.to_vec();
        for (&orig, &restored) in values.iter().zip(restored_vals.iter()) {
            assert!(
                (orig - restored).abs() < 0.05,
                "INT8 roundtrip: {orig} vs {restored}"
            );
        }
    }

    #[test]
    fn qnn_int8_zero_point_is_zero() {
        let values = vec![-1.0, 0.0, 1.0];
        let (_, params) = quantize_symmetric_int8(&values);
        assert_eq!(
            params.zero_point, 0,
            "Symmetric quantization has zero_point=0"
        );
    }

    #[test]
    fn qnn_int8_maps_zero_exactly() {
        let values = vec![-5.0, 0.0, 5.0];
        let (data, params) = quantize_symmetric_int8(&values);
        let dequant = dequantize_symmetric_int8(&data, &params);
        assert_eq!(
            dequant[1], 0.0,
            "Zero maps exactly to 0.0 in symmetric quant"
        );
    }

    #[test]
    fn qnn_int8_all_zeros() {
        let values = vec![0.0, 0.0, 0.0];
        let (data, params) = quantize_symmetric_int8(&values);
        assert_eq!(params.scale, 1.0, "All-zeros: scale defaults to 1.0");
        for &b in &data {
            assert_eq!(b as i8, 0);
        }
    }

    // --- F32 conversion ---

    #[test]
    fn qnn_f32_roundtrip() {
        let values = vec![-1.5, 0.0, 3.14, 100.0];
        let tensor = make_tensor(values.clone(), &[2, 2]);
        let buf = QnnBuffer::from_tensor(&tensor, NpuDtype::F32).unwrap();
        assert_eq!(buf.dtype(), NpuDtype::F32);
        assert_eq!(buf.byte_size(), 16); // 4 bytes * 4 elements
        assert_eq!(buf.shape(), &[2, 2]);
        let restored = buf.to_tensor().unwrap();
        let restored_vals = restored.to_vec();
        for (&orig, &restored) in values.iter().zip(restored_vals.iter()) {
            assert!(
                (orig - restored).abs() < 1e-6,
                "F32 roundtrip: {orig} vs {restored}"
            );
        }
    }

    #[test]
    fn qnn_f32_quant_params_identity() {
        let values = vec![1.0, 2.0];
        let (_, params) = convert_to_f32(&values);
        assert_eq!(params.scale, 1.0);
        assert_eq!(params.zero_point, 0);
        assert_eq!(params.dtype, NpuDtype::F32);
    }

    // --- F16 conversion (IEEE 754 half-precision) ---

    #[test]
    fn qnn_f16_roundtrip() {
        let values = vec![0.0, 1.0, -1.0, 0.5, 65504.0]; // 65504 = max f16
        let tensor = make_tensor(values.clone(), &[5]);
        let buf = QnnBuffer::from_tensor(&tensor, NpuDtype::F16).unwrap();
        assert_eq!(buf.dtype(), NpuDtype::F16);
        assert_eq!(buf.byte_size(), 10); // 2 bytes * 5 elements
        let restored = buf.to_tensor().unwrap();
        let restored_vals = restored.to_vec();
        for (&orig, &restored) in values.iter().zip(restored_vals.iter()) {
            let tol = orig.abs() * 0.002 + 0.001; // relative + absolute tolerance
            assert!(
                (orig - restored).abs() < tol,
                "F16 roundtrip: {orig} vs {restored}"
            );
        }
    }

    #[test]
    fn qnn_f16_zero() {
        let bytes = f64_to_f16_bytes(0.0);
        assert_eq!(bytes, [0, 0], "F16 positive zero");
        let back = f16_bytes_to_f64(bytes);
        assert_eq!(back, 0.0);
    }

    #[test]
    fn qnn_f16_negative_zero() {
        let bytes = f64_to_f16_bytes(-0.0);
        let back = f16_bytes_to_f64(bytes);
        assert!(back == 0.0 || back == -0.0, "F16 negative zero roundtrips");
    }

    #[test]
    fn qnn_f16_overflow_to_inf() {
        // Values > 65504 should overflow to inf in f16
        let bytes = f64_to_f16_bytes(100000.0);
        let back = f16_bytes_to_f64(bytes);
        assert!(back.is_infinite(), "F16 overflow → inf");
    }

    #[test]
    fn qnn_f16_underflow_to_zero() {
        // Very small values flush to zero
        let bytes = f64_to_f16_bytes(1e-10);
        let back = f16_bytes_to_f64(bytes);
        assert_eq!(back, 0.0, "F16 underflow → 0.0");
    }

    // --- BF16 conversion (Brain Float 16) ---

    #[test]
    fn qnn_bf16_roundtrip() {
        let values = vec![0.0, 1.0, -1.0, 3.14, 100.0];
        let tensor = make_tensor(values.clone(), &[5]);
        let buf = QnnBuffer::from_tensor(&tensor, NpuDtype::BF16).unwrap();
        assert_eq!(buf.dtype(), NpuDtype::BF16);
        assert_eq!(buf.byte_size(), 10); // 2 bytes * 5 elements
        let restored = buf.to_tensor().unwrap();
        let restored_vals = restored.to_vec();
        for (&orig, &restored) in values.iter().zip(restored_vals.iter()) {
            let tol = orig.abs() * 0.01 + 0.01; // BF16 has lower precision
            assert!(
                (orig - restored).abs() < tol,
                "BF16 roundtrip: {orig} vs {restored}"
            );
        }
    }

    #[test]
    fn qnn_bf16_zero() {
        let values = vec![0.0];
        let (data, _) = convert_to_bf16(&values);
        let back = convert_from_bf16(&data);
        assert_eq!(back[0], 0.0);
    }

    #[test]
    fn qnn_bf16_large_range() {
        // BF16 has same exponent range as f32 (unlike f16)
        let values = vec![1e30, -1e30];
        let (data, _) = convert_to_bf16(&values);
        let back = convert_from_bf16(&data);
        assert!(
            (back[0] - 1e30).abs() / 1e30 < 0.01,
            "BF16 large value: {}",
            back[0]
        );
        assert!(
            (back[1] + 1e30).abs() / 1e30 < 0.01,
            "BF16 large neg: {}",
            back[1]
        );
    }

    // --- QnnBuffer API tests ---

    #[test]
    fn qnn_buffer_from_raw() {
        let buf = QnnBuffer::from_raw(
            vec![10, 20, 30],
            vec![3],
            NpuDtype::UINT8,
            QnnQuantParams {
                scale: 0.1,
                zero_point: 128,
                dtype: NpuDtype::UINT8,
            },
        );
        assert_eq!(buf.data(), &[10, 20, 30]);
        assert_eq!(buf.shape(), &[3]);
        assert_eq!(buf.numel(), 3);
        assert_eq!(buf.quant_params().zero_point, 128);
    }

    #[test]
    fn qnn_buffer_2d_shape() {
        let tensor = make_tensor(vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0], &[2, 3]);
        let buf = QnnBuffer::from_tensor(&tensor, NpuDtype::F32).unwrap();
        assert_eq!(buf.shape(), &[2, 3]);
        assert_eq!(buf.numel(), 6);
        let restored = buf.to_tensor().unwrap();
        assert_eq!(restored.shape(), &[2, 3]);
    }
}
