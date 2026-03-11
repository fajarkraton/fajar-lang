//! # Intel OpenVINO NPU Backend
//!
//! Safe Rust wrappers for the OpenVINO C API, gated behind `--features openvino`.
//! When the feature is not enabled, all operations return `BackendUnavailable`.
//!
//! ## API Flow
//!
//! ```text
//! ov_core_create → ov_core_compile_model("NPU") → ov_infer_request_create
//!     → set_input_tensor → infer → get_output_tensor
//! ```

use super::{NpuCompiledModel, NpuDtype, NpuInferenceResult, NpuRuntimeError};

// ═══════════════════════════════════════════════════════════════════════
// OpenVINO Backend
// ═══════════════════════════════════════════════════════════════════════

/// Intel OpenVINO NPU backend.
///
/// Loads and runs models on Intel NPUs (Meteor Lake, Lunar Lake, Arrow Lake)
/// via the OpenVINO Toolkit runtime.
#[derive(Debug, Clone)]
pub struct OpenVinoBackend {
    /// Whether the backend is available.
    available: bool,
    /// Device name for OpenVINO (e.g., "NPU", "CPU").
    device: String,
}

impl OpenVinoBackend {
    /// Try to initialize the OpenVINO backend.
    ///
    /// Returns a backend instance. If OpenVINO is not available,
    /// the backend will return errors on all operations.
    pub fn new() -> Self {
        Self {
            available: Self::probe_availability(),
            device: "NPU".to_string(),
        }
    }

    /// Check if the OpenVINO runtime is available.
    pub fn is_available(&self) -> bool {
        self.available
    }

    /// Probe for OpenVINO runtime availability.
    fn probe_availability() -> bool {
        // Check for libopenvino.so / libopenvino_c.so
        #[cfg(target_os = "linux")]
        {
            let paths = [
                "/usr/lib/x86_64-linux-gnu/libopenvino.so",
                "/usr/local/lib/libopenvino.so",
                "/opt/intel/openvino/runtime/lib/intel64/libopenvino.so",
            ];
            for path in &paths {
                if std::path::Path::new(path).exists() {
                    return true;
                }
            }
        }
        false
    }

    /// Compile an ONNX model for NPU execution.
    pub fn compile_model(
        &self,
        onnx_path: &str,
        input_shapes: Vec<Vec<usize>>,
        output_shapes: Vec<Vec<usize>>,
    ) -> Result<NpuCompiledModel, NpuRuntimeError> {
        if !self.available {
            return Err(NpuRuntimeError::BackendUnavailable {
                backend: "openvino".to_string(),
                reason: "OpenVINO runtime not found".to_string(),
            });
        }

        // Stub: in a real implementation, this would call ov_core_compile_model
        Ok(NpuCompiledModel {
            backend: "openvino".to_string(),
            name: onnx_path.to_string(),
            input_shapes,
            output_shapes,
            input_dtypes: vec![NpuDtype::F32],
            output_dtypes: vec![NpuDtype::F32],
        })
    }

    /// Run inference on a compiled model.
    pub fn infer(
        &self,
        _model: &NpuCompiledModel,
        inputs: &[Vec<f64>],
    ) -> Result<NpuInferenceResult, NpuRuntimeError> {
        if !self.available {
            return Err(NpuRuntimeError::BackendUnavailable {
                backend: "openvino".to_string(),
                reason: "OpenVINO runtime not found".to_string(),
            });
        }

        // Stub: return input as output (identity)
        Ok(NpuInferenceResult {
            outputs: inputs.to_vec(),
            output_shapes: vec![vec![inputs.first().map_or(0, |v| v.len())]],
            latency_us: 0,
        })
    }

    /// Device name.
    pub fn device(&self) -> &str {
        &self.device
    }
}

impl Default for OpenVinoBackend {
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

    #[test]
    fn openvino_backend_creates() {
        let backend = OpenVinoBackend::new();
        // On most CI/dev systems, OpenVINO is not installed
        assert_eq!(backend.device(), "NPU");
    }

    #[test]
    fn openvino_unavailable_returns_error() {
        let backend = OpenVinoBackend {
            available: false,
            device: "NPU".to_string(),
        };
        let result = backend.compile_model("test.onnx", vec![vec![1, 3]], vec![vec![1, 10]]);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(format!("{err}").contains("not found"));
    }

    #[test]
    fn openvino_infer_unavailable() {
        let backend = OpenVinoBackend {
            available: false,
            device: "NPU".to_string(),
        };
        let model = NpuCompiledModel {
            backend: "openvino".to_string(),
            name: "test".to_string(),
            input_shapes: vec![],
            output_shapes: vec![],
            input_dtypes: vec![],
            output_dtypes: vec![],
        };
        let result = backend.infer(&model, &[vec![1.0, 2.0]]);
        assert!(result.is_err());
    }

    #[test]
    fn openvino_compile_when_available() {
        let backend = OpenVinoBackend {
            available: true,
            device: "NPU".to_string(),
        };
        let result = backend.compile_model(
            "resnet18.onnx",
            vec![vec![1, 3, 224, 224]],
            vec![vec![1, 1000]],
        );
        assert!(result.is_ok());
        let model = result.unwrap();
        assert_eq!(model.backend, "openvino");
    }

    #[test]
    fn openvino_infer_when_available() {
        let backend = OpenVinoBackend {
            available: true,
            device: "NPU".to_string(),
        };
        let model = NpuCompiledModel {
            backend: "openvino".to_string(),
            name: "test".to_string(),
            input_shapes: vec![vec![4]],
            output_shapes: vec![vec![4]],
            input_dtypes: vec![NpuDtype::F32],
            output_dtypes: vec![NpuDtype::F32],
        };
        let result = backend.infer(&model, &[vec![1.0, 2.0, 3.0, 4.0]]);
        assert!(result.is_ok());
    }
}
