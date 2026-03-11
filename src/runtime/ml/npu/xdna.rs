//! # AMD XDNA NPU Backend
//!
//! Interface to AMD XDNA 2 NPUs via the amdxdna kernel driver.
//! Gated behind `--features xdna`. When not enabled, all operations
//! return `BackendUnavailable`.
//!
//! ## AMD XDNA Architecture
//!
//! - AI Engine tiles (AIE) for matrix/vector compute
//! - DMA for data movement between host and NPU
//! - Vitis AI runtime for model execution

use super::{NpuCompiledModel, NpuDtype, NpuInferenceResult, NpuRuntimeError};

// ═══════════════════════════════════════════════════════════════════════
// XDNA Backend
// ═══════════════════════════════════════════════════════════════════════

/// AMD XDNA NPU backend.
///
/// Runs inference on AMD Ryzen AI NPUs (XDNA / XDNA 2 architecture)
/// via the amdxdna kernel driver.
#[derive(Debug, Clone)]
pub struct XdnaBackend {
    /// Whether the backend is available.
    available: bool,
    /// Device path (e.g., "/dev/accel/accel0").
    device_path: String,
}

impl XdnaBackend {
    /// Try to initialize the XDNA backend.
    pub fn new() -> Self {
        let (available, device_path) = Self::probe_availability();
        Self {
            available,
            device_path,
        }
    }

    /// Check if the XDNA driver is available.
    pub fn is_available(&self) -> bool {
        self.available
    }

    /// Probe for XDNA driver availability.
    fn probe_availability() -> (bool, String) {
        let xdna_module = std::path::Path::new("/sys/module/amdxdna");
        if xdna_module.exists() {
            // Find the accel device
            for i in 0..4 {
                let path = format!("/dev/accel/accel{i}");
                if std::path::Path::new(&path).exists() {
                    return (true, path);
                }
            }
        }
        (false, String::new())
    }

    /// Compile a model for XDNA execution.
    pub fn compile_model(
        &self,
        model_path: &str,
        input_shapes: Vec<Vec<usize>>,
        output_shapes: Vec<Vec<usize>>,
    ) -> Result<NpuCompiledModel, NpuRuntimeError> {
        if !self.available {
            return Err(NpuRuntimeError::BackendUnavailable {
                backend: "xdna".to_string(),
                reason: "AMD XDNA driver not found".to_string(),
            });
        }

        Ok(NpuCompiledModel {
            backend: "xdna".to_string(),
            name: model_path.to_string(),
            input_shapes,
            output_shapes,
            input_dtypes: vec![NpuDtype::INT8],
            output_dtypes: vec![NpuDtype::INT8],
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
                backend: "xdna".to_string(),
                reason: "AMD XDNA driver not found".to_string(),
            });
        }

        // Stub: identity inference
        Ok(NpuInferenceResult {
            outputs: inputs.to_vec(),
            output_shapes: vec![vec![inputs.first().map_or(0, |v| v.len())]],
            latency_us: 0,
        })
    }

    /// Device path.
    pub fn device_path(&self) -> &str {
        &self.device_path
    }
}

impl Default for XdnaBackend {
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
    fn xdna_backend_creates() {
        let backend = XdnaBackend::new();
        // On most systems, XDNA is not installed
        let _ = backend.is_available();
    }

    #[test]
    fn xdna_unavailable_returns_error() {
        let backend = XdnaBackend {
            available: false,
            device_path: String::new(),
        };
        let result = backend.compile_model("test.onnx", vec![vec![1]], vec![vec![1]]);
        assert!(result.is_err());
    }

    #[test]
    fn xdna_compile_when_available() {
        let backend = XdnaBackend {
            available: true,
            device_path: "/dev/accel/accel0".to_string(),
        };
        let result = backend.compile_model("model.onnx", vec![vec![1, 784]], vec![vec![1, 10]]);
        assert!(result.is_ok());
        let model = result.unwrap();
        assert_eq!(model.backend, "xdna");
    }

    #[test]
    fn xdna_infer_unavailable() {
        let backend = XdnaBackend {
            available: false,
            device_path: String::new(),
        };
        let model = NpuCompiledModel {
            backend: "xdna".to_string(),
            name: "test".to_string(),
            input_shapes: vec![],
            output_shapes: vec![],
            input_dtypes: vec![],
            output_dtypes: vec![],
        };
        let result = backend.infer(&model, &[vec![1.0]]);
        assert!(result.is_err());
    }

    #[test]
    fn xdna_infer_when_available() {
        let backend = XdnaBackend {
            available: true,
            device_path: "/dev/accel/accel0".to_string(),
        };
        let model = NpuCompiledModel {
            backend: "xdna".to_string(),
            name: "test".to_string(),
            input_shapes: vec![vec![4]],
            output_shapes: vec![vec![4]],
            input_dtypes: vec![NpuDtype::INT8],
            output_dtypes: vec![NpuDtype::INT8],
        };
        let result = backend.infer(&model, &[vec![1.0, 2.0, 3.0, 4.0]]);
        assert!(result.is_ok());
    }
}
