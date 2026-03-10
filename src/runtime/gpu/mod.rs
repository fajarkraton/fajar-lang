//! GPU compute abstraction layer for Fajar Lang.
//!
//! Provides a backend-agnostic API for GPU computation:
//! - [`GpuDevice`] trait — device enumeration, buffer/kernel management
//! - [`GpuBuffer`] — device memory handle with upload/download
//! - [`GpuKernel`] — compiled compute shader
//! - [`available_devices()`] — enumerate all available GPU devices
//!
//! Backends:
//! - `wgpu` (feature `gpu`) — cross-platform via Vulkan/Metal/DX12
//! - CUDA FFI (feature `cuda`) — NVIDIA-specific optimization

mod buffer;
mod cpu_fallback;
mod device;
mod kernel;
pub mod tensor_bridge;

pub use buffer::GpuBuffer;
pub use cpu_fallback::CpuFallbackDevice;
pub use device::{GpuBackend, GpuDevice, GpuDeviceInfo};
pub use kernel::{GpuKernel, KernelSource, WorkgroupSize};

#[cfg(feature = "gpu")]
mod wgpu_backend;
#[cfg(feature = "gpu")]
pub use wgpu_backend::WgpuDevice;

#[cfg(feature = "cuda")]
mod cuda_backend;
#[cfg(feature = "cuda")]
pub use cuda_backend::CudaDevice;

use thiserror::Error;

/// GPU compute error codes.
#[derive(Debug, Error)]
pub enum GpuError {
    /// CE013 — No GPU device available.
    #[error("CE013: GPU compute not available on this system")]
    NotAvailable,

    /// TE009 — Tensor shape mismatch for GPU operation.
    #[error("TE009: GPU tensor shape mismatch: {0}")]
    ShapeMismatch(String),

    /// TE010 — GPU memory exhausted.
    #[error(
        "TE010: GPU memory exhausted: requested {requested} bytes, available {available} bytes"
    )]
    MemoryExhausted { requested: usize, available: usize },

    /// Buffer size mismatch during upload/download.
    #[error("buffer size mismatch: expected {expected} bytes, got {actual} bytes")]
    BufferSizeMismatch { expected: usize, actual: usize },

    /// Invalid kernel source.
    #[error("invalid kernel: {0}")]
    InvalidKernel(String),

    /// Backend-specific error.
    #[error("GPU backend error: {0}")]
    BackendError(String),

    /// Dispatch failed.
    #[error("GPU dispatch failed: {0}")]
    DispatchFailed(String),
}

/// Enumerate all available GPU devices across all backends.
///
/// Returns a CPU fallback device if no real GPU is found.
pub fn available_devices() -> Vec<Box<dyn GpuDevice>> {
    #[allow(unused_mut)]
    let mut devices: Vec<Box<dyn GpuDevice>> = vec![Box::new(CpuFallbackDevice::new())];

    #[cfg(feature = "gpu")]
    {
        if let Ok(wgpu_devices) = wgpu_backend::enumerate_devices() {
            for dev in wgpu_devices.into_iter().rev() {
                devices.insert(0, dev);
            }
        }
    }

    #[cfg(feature = "cuda")]
    {
        if let Ok(cuda_devices) = cuda_backend::enumerate_devices() {
            let insert_pos = devices.len().saturating_sub(1);
            for dev in cuda_devices.into_iter().rev() {
                devices.insert(insert_pos, dev);
            }
        }
    }

    devices
}

/// Get the best available GPU device, preferring real GPU over CPU fallback.
pub fn best_device() -> Box<dyn GpuDevice> {
    let devices = available_devices();
    // Return first non-CPU device, or CPU fallback
    for device in devices {
        if device.info().backend != GpuBackend::CpuFallback {
            return device;
        }
    }
    Box::new(CpuFallbackDevice::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn available_devices_always_returns_at_least_one() {
        let devices = available_devices();
        assert!(
            !devices.is_empty(),
            "should always have at least CPU fallback"
        );
    }

    #[test]
    fn best_device_returns_a_device() {
        let device = best_device();
        let info = device.info();
        assert!(!info.name.is_empty());
        assert!(info.compute_units > 0);
    }

    #[test]
    fn cpu_fallback_is_always_present() {
        let devices = available_devices();
        let has_cpu = devices
            .iter()
            .any(|d| d.info().backend == GpuBackend::CpuFallback);
        assert!(has_cpu, "CPU fallback should always be available");
    }
}
