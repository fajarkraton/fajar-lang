//! GPU device trait — backend-agnostic interface for GPU computation.

use super::GpuError;
use super::buffer::GpuBuffer;
use super::kernel::{GpuKernel, KernelSource, WorkgroupSize};

/// GPU backend identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuBackend {
    /// wgpu (Vulkan/Metal/DX12).
    Wgpu,
    /// NVIDIA CUDA.
    Cuda,
    /// CPU fallback (no real GPU).
    CpuFallback,
}

impl std::fmt::Display for GpuBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GpuBackend::Wgpu => write!(f, "wgpu"),
            GpuBackend::Cuda => write!(f, "CUDA"),
            GpuBackend::CpuFallback => write!(f, "CPU (fallback)"),
        }
    }
}

/// Static information about a GPU device.
#[derive(Debug, Clone)]
pub struct GpuDeviceInfo {
    /// Human-readable device name.
    pub name: String,
    /// Total device memory in bytes.
    pub memory: u64,
    /// Number of compute units (shader cores / SMs).
    pub compute_units: u32,
    /// Backend type.
    pub backend: GpuBackend,
    /// Maximum workgroup size (x * y * z).
    pub max_workgroup_size: u32,
    /// Maximum buffer size in bytes.
    pub max_buffer_size: u64,
}

/// Backend-agnostic GPU device interface.
///
/// Implementations provide device-specific buffer allocation,
/// kernel compilation, and compute dispatch.
pub trait GpuDevice: Send + Sync {
    /// Get device information.
    fn info(&self) -> GpuDeviceInfo;

    /// Create a device buffer of the given size in bytes.
    fn create_buffer(&self, size: usize) -> Result<GpuBuffer, GpuError>;

    /// Upload data from host to device buffer.
    fn upload(&self, buffer: &GpuBuffer, data: &[u8]) -> Result<(), GpuError>;

    /// Download data from device buffer to host.
    fn download(&self, buffer: &GpuBuffer, dst: &mut [u8]) -> Result<(), GpuError>;

    /// Compile a compute kernel from source.
    fn compile_kernel(&self, source: &KernelSource) -> Result<GpuKernel, GpuError>;

    /// Execute a compute kernel.
    ///
    /// # Arguments
    /// * `kernel` — compiled kernel to execute
    /// * `workgroups` — number of workgroups (x, y, z)
    /// * `buffers` — buffer arguments bound to the kernel
    fn execute(
        &self,
        kernel: &GpuKernel,
        workgroups: (u32, u32, u32),
        buffers: &[&GpuBuffer],
    ) -> Result<(), GpuError>;

    /// Free a device buffer's GPU memory.
    ///
    /// Implementations should call the backend-specific free (e.g., cuMemFree).
    /// Default: no-op (CPU fallback doesn't need explicit free).
    fn free_buffer(&self, _buffer: &GpuBuffer) {
        // Default no-op for backends that don't need explicit free
    }

    /// Execute a compute kernel with explicit workgroup size.
    fn execute_with_workgroup_size(
        &self,
        kernel: &GpuKernel,
        workgroups: (u32, u32, u32),
        _workgroup_size: WorkgroupSize,
        buffers: &[&GpuBuffer],
    ) -> Result<(), GpuError> {
        // Default: ignore workgroup size, use kernel-defined size
        self.execute(kernel, workgroups, buffers)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gpu_backend_display() {
        assert_eq!(format!("{}", GpuBackend::Wgpu), "wgpu");
        assert_eq!(format!("{}", GpuBackend::Cuda), "CUDA");
        assert_eq!(format!("{}", GpuBackend::CpuFallback), "CPU (fallback)");
    }

    #[test]
    fn device_info_fields() {
        let info = GpuDeviceInfo {
            name: "Test GPU".into(),
            memory: 8 * 1024 * 1024 * 1024,
            compute_units: 128,
            backend: GpuBackend::Wgpu,
            max_workgroup_size: 256,
            max_buffer_size: 2 * 1024 * 1024 * 1024,
        };
        assert_eq!(info.name, "Test GPU");
        assert_eq!(info.memory, 8_589_934_592);
        assert_eq!(info.compute_units, 128);
        assert_eq!(info.backend, GpuBackend::Wgpu);
    }
}
