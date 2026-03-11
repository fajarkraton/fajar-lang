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
}
