//! Device backend abstraction for multi-target tensor computation.
//!
//! Inspired by HuggingFace Candle's Device/Backend architecture.
//! Provides a unified interface for tensor operations that can execute
//! on CPU (ndarray), GPU (Vulkan/Adreno), or NPU (Hexagon/QNN).
//!
//! # Architecture
//!
//! ```text
//! @device fn inference(input: Tensor) -> Tensor {
//!     matmul(input, weights)   // dispatches to active backend
//! }
//!
//! Device::Cpu  → CpuBackend  (ndarray, NEON/AVX2)
//! Device::Gpu  → GpuBackend  (Vulkan compute, Adreno 643)
//! Device::Npu  → NpuBackend  (QNN SDK, Hexagon 770)
//! ```
//!
//! # Target Hardware
//!
//! | Platform | CPU | GPU | NPU |
//! |----------|-----|-----|-----|
//! | x86_64 | AVX2/AVX-512 | NVIDIA CUDA | — |
//! | ARM64 (Q6A) | NEON | Adreno 643 Vulkan | Hexagon 770 (12 TOPS) |

use ndarray::ArrayD;

// ═══════════════════════════════════════════════════════════════════════
// Device Enum
// ═══════════════════════════════════════════════════════════════════════

/// Compute device for tensor operations.
///
/// Determines which backend executes tensor operations.
/// Same code, different hardware — selected at runtime or compile time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Device {
    /// CPU computation (ndarray, SIMD where available).
    Cpu,
    /// GPU computation (Vulkan on Adreno, CUDA on NVIDIA).
    Gpu(u32),
    /// Neural Processing Unit (QNN SDK on Hexagon).
    Npu,
}

impl Device {
    /// Returns the default device (CPU).
    pub fn default_device() -> Self {
        Device::Cpu
    }

    /// Returns the best available device.
    ///
    /// Checks NPU first, then GPU, falls back to CPU.
    pub fn best_available() -> Self {
        // In a real implementation, this would probe hardware.
        // For now, default to CPU.
        Device::Cpu
    }

    /// Returns true if this device is available on the current platform.
    pub fn is_available(&self) -> bool {
        match self {
            Device::Cpu => true,     // CPU always available
            Device::Gpu(_) => false, // Needs runtime detection
            Device::Npu => false,    // Needs QNN SDK
        }
    }

    /// Returns a human-readable name.
    pub fn name(&self) -> &str {
        match self {
            Device::Cpu => "cpu",
            Device::Gpu(_) => "gpu",
            Device::Npu => "npu",
        }
    }
}

impl std::fmt::Display for Device {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Device::Cpu => write!(f, "cpu"),
            Device::Gpu(id) => write!(f, "gpu:{id}"),
            Device::Npu => write!(f, "npu"),
        }
    }
}

impl Default for Device {
    fn default() -> Self {
        Self::default_device()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Backend Trait
// ═══════════════════════════════════════════════════════════════════════

/// Tensor computation backend trait.
///
/// Implementors provide tensor operations for a specific device.
/// All operations take f64 arrays (internal representation) and
/// return f64 arrays. DType handling is done at a higher level.
pub trait TensorBackend {
    /// Returns the device this backend runs on.
    fn device(&self) -> Device;

    /// Matrix multiplication: C = A × B.
    fn matmul(&self, a: &ArrayD<f64>, b: &ArrayD<f64>) -> Result<ArrayD<f64>, BackendError>;

    /// Element-wise addition: C = A + B.
    fn add(&self, a: &ArrayD<f64>, b: &ArrayD<f64>) -> Result<ArrayD<f64>, BackendError>;

    /// Element-wise multiplication: C = A * B.
    fn mul(&self, a: &ArrayD<f64>, b: &ArrayD<f64>) -> Result<ArrayD<f64>, BackendError>;

    /// ReLU activation: max(0, x) element-wise.
    fn relu(&self, x: &ArrayD<f64>) -> ArrayD<f64>;

    /// Softmax: exp(x_i) / sum(exp(x_j)) along last axis.
    fn softmax(&self, x: &ArrayD<f64>) -> ArrayD<f64>;

    /// Sigmoid: 1 / (1 + exp(-x)) element-wise.
    fn sigmoid(&self, x: &ArrayD<f64>) -> ArrayD<f64>;

    /// Transpose a 2D matrix.
    fn transpose(&self, x: &ArrayD<f64>) -> Result<ArrayD<f64>, BackendError>;

    /// Sum all elements.
    fn sum(&self, x: &ArrayD<f64>) -> f64;

    /// Returns the backend name for debugging.
    fn name(&self) -> &str;
}

/// Error from a backend operation.
#[derive(Debug, Clone, thiserror::Error)]
pub enum BackendError {
    /// Shape mismatch in operation.
    #[error("shape mismatch: {op}: {detail}")]
    ShapeMismatch { op: String, detail: String },

    /// Device not available.
    #[error("device not available: {device}")]
    DeviceUnavailable { device: String },

    /// Operation not supported on this backend.
    #[error("operation '{op}' not supported on {backend}")]
    Unsupported { op: String, backend: String },
}

// ═══════════════════════════════════════════════════════════════════════
// CPU Backend (default, always available)
// ═══════════════════════════════════════════════════════════════════════

/// CPU-based tensor backend using ndarray.
///
/// This is the default backend, always available on every platform.
/// Uses SIMD (AVX2/NEON) when available via ndarray/BLAS.
pub struct CpuBackend;

impl CpuBackend {
    /// Creates a new CPU backend.
    pub fn new() -> Self {
        CpuBackend
    }
}

impl Default for CpuBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl TensorBackend for CpuBackend {
    fn device(&self) -> Device {
        Device::Cpu
    }

    fn matmul(&self, a: &ArrayD<f64>, b: &ArrayD<f64>) -> Result<ArrayD<f64>, BackendError> {
        let a_shape = a.shape();
        let b_shape = b.shape();

        if a_shape.len() != 2 || b_shape.len() != 2 {
            return Err(BackendError::ShapeMismatch {
                op: "matmul".into(),
                detail: format!(
                    "expected 2D arrays, got {}D and {}D",
                    a_shape.len(),
                    b_shape.len()
                ),
            });
        }

        let m = a_shape[0];
        let k = a_shape[1];
        let n = b_shape[1];

        if k != b_shape[0] {
            return Err(BackendError::ShapeMismatch {
                op: "matmul".into(),
                detail: format!(
                    "inner dimensions mismatch: {}x{} vs {}x{}",
                    m, k, b_shape[0], n
                ),
            });
        }

        let a_2d = a.view().into_shape_with_order((m, k)).unwrap();
        let b_2d = b.view().into_shape_with_order((k, n)).unwrap();
        let c = a_2d.dot(&b_2d);
        Ok(c.into_dyn())
    }

    fn add(&self, a: &ArrayD<f64>, b: &ArrayD<f64>) -> Result<ArrayD<f64>, BackendError> {
        if a.shape() != b.shape() {
            return Err(BackendError::ShapeMismatch {
                op: "add".into(),
                detail: format!("shapes {:?} vs {:?}", a.shape(), b.shape()),
            });
        }
        Ok(a + b)
    }

    fn mul(&self, a: &ArrayD<f64>, b: &ArrayD<f64>) -> Result<ArrayD<f64>, BackendError> {
        if a.shape() != b.shape() {
            return Err(BackendError::ShapeMismatch {
                op: "mul".into(),
                detail: format!("shapes {:?} vs {:?}", a.shape(), b.shape()),
            });
        }
        Ok(a * b)
    }

    fn relu(&self, x: &ArrayD<f64>) -> ArrayD<f64> {
        x.mapv(|v| if v > 0.0 { v } else { 0.0 })
    }

    fn softmax(&self, x: &ArrayD<f64>) -> ArrayD<f64> {
        let max = x.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let exp = x.mapv(|v| (v - max).exp());
        let sum: f64 = exp.iter().sum();
        if sum > 0.0 { exp / sum } else { exp }
    }

    fn sigmoid(&self, x: &ArrayD<f64>) -> ArrayD<f64> {
        x.mapv(|v| 1.0 / (1.0 + (-v).exp()))
    }

    fn transpose(&self, x: &ArrayD<f64>) -> Result<ArrayD<f64>, BackendError> {
        if x.ndim() != 2 {
            return Err(BackendError::ShapeMismatch {
                op: "transpose".into(),
                detail: format!("expected 2D, got {}D", x.ndim()),
            });
        }
        Ok(x.clone().reversed_axes())
    }

    fn sum(&self, x: &ArrayD<f64>) -> f64 {
        x.iter().sum()
    }

    fn name(&self) -> &str {
        "cpu (ndarray)"
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Global backend dispatch
// ═══════════════════════════════════════════════════════════════════════

/// Returns the active backend for a device.
///
/// Currently only CPU is implemented. GPU and NPU return CPU fallback.
pub fn get_backend(device: Device) -> Box<dyn TensorBackend> {
    match device {
        Device::Cpu => Box::new(CpuBackend::new()),
        Device::Gpu(_) => {
            // GPU not yet implemented — fallback to CPU
            Box::new(CpuBackend::new())
        }
        Device::Npu => {
            // NPU not yet implemented — fallback to CPU
            Box::new(CpuBackend::new())
        }
    }
}

/// Returns the default backend (CPU).
pub fn default_backend() -> Box<dyn TensorBackend> {
    get_backend(Device::Cpu)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::ArrayD;

    fn arr2(data: Vec<Vec<f64>>) -> ArrayD<f64> {
        let rows = data.len();
        let cols = data[0].len();
        let flat: Vec<f64> = data.into_iter().flatten().collect();
        ArrayD::from_shape_vec(vec![rows, cols], flat).unwrap()
    }

    fn arr1(data: Vec<f64>) -> ArrayD<f64> {
        ArrayD::from_shape_vec(vec![data.len()], data).unwrap()
    }

    #[test]
    fn device_cpu_default() {
        assert_eq!(Device::default_device(), Device::Cpu);
        assert!(Device::Cpu.is_available());
    }

    #[test]
    fn device_display() {
        assert_eq!(format!("{}", Device::Cpu), "cpu");
        assert_eq!(format!("{}", Device::Gpu(0)), "gpu:0");
        assert_eq!(format!("{}", Device::Npu), "npu");
    }

    #[test]
    fn cpu_matmul_2x3_3x2() {
        let backend = CpuBackend::new();
        let a = arr2(vec![vec![1.0, 2.0, 3.0], vec![4.0, 5.0, 6.0]]);
        let b = arr2(vec![vec![7.0, 8.0], vec![9.0, 10.0], vec![11.0, 12.0]]);
        let c = backend.matmul(&a, &b).unwrap();
        assert_eq!(c.shape(), &[2, 2]);
        let vals: Vec<f64> = c.iter().cloned().collect();
        assert!((vals[0] - 58.0).abs() < 1e-10); // 1*7+2*9+3*11
        assert!((vals[1] - 64.0).abs() < 1e-10); // 1*8+2*10+3*12
    }

    #[test]
    fn cpu_matmul_shape_mismatch() {
        let backend = CpuBackend::new();
        let a = arr2(vec![vec![1.0, 2.0]]);
        let b = arr2(vec![vec![1.0], vec![2.0], vec![3.0]]);
        assert!(backend.matmul(&a, &b).is_err());
    }

    #[test]
    fn cpu_add() {
        let backend = CpuBackend::new();
        let a = arr1(vec![1.0, 2.0, 3.0]);
        let b = arr1(vec![4.0, 5.0, 6.0]);
        let c = backend.add(&a, &b).unwrap();
        let vals: Vec<f64> = c.iter().cloned().collect();
        assert_eq!(vals, vec![5.0, 7.0, 9.0]);
    }

    #[test]
    fn cpu_mul() {
        let backend = CpuBackend::new();
        let a = arr1(vec![2.0, 3.0, 4.0]);
        let b = arr1(vec![5.0, 6.0, 7.0]);
        let c = backend.mul(&a, &b).unwrap();
        let vals: Vec<f64> = c.iter().cloned().collect();
        assert_eq!(vals, vec![10.0, 18.0, 28.0]);
    }

    #[test]
    fn cpu_relu() {
        let backend = CpuBackend::new();
        let x = arr1(vec![-2.0, -1.0, 0.0, 1.0, 2.0]);
        let y = backend.relu(&x);
        let vals: Vec<f64> = y.iter().cloned().collect();
        assert_eq!(vals, vec![0.0, 0.0, 0.0, 1.0, 2.0]);
    }

    #[test]
    fn cpu_softmax_sums_to_one() {
        let backend = CpuBackend::new();
        let x = arr1(vec![1.0, 2.0, 3.0]);
        let y = backend.softmax(&x);
        let sum: f64 = y.iter().sum();
        assert!((sum - 1.0).abs() < 1e-10);
    }

    #[test]
    fn cpu_sigmoid_range() {
        let backend = CpuBackend::new();
        let x = arr1(vec![-100.0, 0.0, 100.0]);
        let y = backend.sigmoid(&x);
        let vals: Vec<f64> = y.iter().cloned().collect();
        assert!(vals[0] < 0.01); // sigmoid(-100) ≈ 0
        assert!((vals[1] - 0.5).abs() < 1e-10); // sigmoid(0) = 0.5
        assert!(vals[2] > 0.99); // sigmoid(100) ≈ 1
    }

    #[test]
    fn cpu_transpose() {
        let backend = CpuBackend::new();
        let x = arr2(vec![vec![1.0, 2.0, 3.0], vec![4.0, 5.0, 6.0]]);
        let t = backend.transpose(&x).unwrap();
        assert_eq!(t.shape(), &[3, 2]);
    }

    #[test]
    fn cpu_sum() {
        let backend = CpuBackend::new();
        let x = arr1(vec![1.0, 2.0, 3.0, 4.0]);
        assert!((backend.sum(&x) - 10.0).abs() < 1e-10);
    }

    #[test]
    fn backend_name() {
        let backend = CpuBackend::new();
        assert_eq!(backend.name(), "cpu (ndarray)");
    }

    #[test]
    fn get_backend_cpu() {
        let b = get_backend(Device::Cpu);
        assert_eq!(b.device(), Device::Cpu);
    }

    #[test]
    fn get_backend_gpu_fallback() {
        let b = get_backend(Device::Gpu(0));
        assert_eq!(b.device(), Device::Cpu); // fallback
    }

    #[test]
    fn get_backend_npu_fallback() {
        let b = get_backend(Device::Npu);
        assert_eq!(b.device(), Device::Cpu); // fallback
    }
}
