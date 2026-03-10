//! GPU kernel — compiled compute shader representation.

use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_KERNEL_ID: AtomicU64 = AtomicU64::new(1);

/// Source code for a GPU compute kernel.
#[derive(Debug, Clone)]
pub enum KernelSource {
    /// WGSL shader source (wgpu backend).
    Wgsl(String),
    /// SPIR-V binary (Vulkan).
    SpirV(Vec<u32>),
    /// PTX assembly text (CUDA).
    Ptx(String),
    /// Built-in kernel by name.
    Builtin(BuiltinKernel),
}

/// Pre-defined compute kernels for common ML operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinKernel {
    /// Element-wise vector addition.
    VectorAdd,
    /// Element-wise vector multiplication.
    VectorMul,
    /// Element-wise vector subtraction.
    VectorSub,
    /// Element-wise vector division.
    VectorDiv,
    /// ReLU activation: max(0, x).
    Relu,
    /// Sigmoid activation: 1 / (1 + exp(-x)).
    Sigmoid,
    /// Softmax activation (per-row).
    Softmax,
    /// Matrix multiplication.
    Matmul,
}

impl std::fmt::Display for BuiltinKernel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BuiltinKernel::VectorAdd => write!(f, "vector_add"),
            BuiltinKernel::VectorMul => write!(f, "vector_mul"),
            BuiltinKernel::VectorSub => write!(f, "vector_sub"),
            BuiltinKernel::VectorDiv => write!(f, "vector_div"),
            BuiltinKernel::Relu => write!(f, "relu"),
            BuiltinKernel::Sigmoid => write!(f, "sigmoid"),
            BuiltinKernel::Softmax => write!(f, "softmax"),
            BuiltinKernel::Matmul => write!(f, "matmul"),
        }
    }
}

/// Workgroup size for compute dispatch.
#[derive(Debug, Clone, Copy)]
pub struct WorkgroupSize {
    /// X dimension.
    pub x: u32,
    /// Y dimension.
    pub y: u32,
    /// Z dimension.
    pub z: u32,
}

impl WorkgroupSize {
    /// Create a 1D workgroup size.
    pub fn d1(x: u32) -> Self {
        WorkgroupSize { x, y: 1, z: 1 }
    }

    /// Create a 2D workgroup size.
    pub fn d2(x: u32, y: u32) -> Self {
        WorkgroupSize { x, y, z: 1 }
    }

    /// Total number of invocations per workgroup.
    pub fn total(&self) -> u32 {
        self.x * self.y * self.z
    }
}

impl Default for WorkgroupSize {
    fn default() -> Self {
        WorkgroupSize { x: 256, y: 1, z: 1 }
    }
}

/// A compiled GPU compute kernel, ready for dispatch.
#[derive(Debug)]
pub struct GpuKernel {
    /// Unique kernel identifier.
    id: u64,
    /// Human-readable name.
    name: String,
    /// Number of buffer bindings expected.
    num_bindings: u32,
    /// Workgroup size specified in the kernel.
    workgroup_size: WorkgroupSize,
    /// Backend-specific compiled data (e.g., pipeline index).
    backend_handle: u64,
}

impl GpuKernel {
    /// Create a new compiled kernel.
    pub fn new(
        name: String,
        num_bindings: u32,
        workgroup_size: WorkgroupSize,
        backend_handle: u64,
    ) -> Self {
        GpuKernel {
            id: NEXT_KERNEL_ID.fetch_add(1, Ordering::Relaxed),
            name,
            num_bindings,
            workgroup_size,
            backend_handle,
        }
    }

    /// Get the unique kernel ID.
    pub fn id(&self) -> u64 {
        self.id
    }

    /// Get the kernel name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the number of buffer bindings expected.
    pub fn num_bindings(&self) -> u32 {
        self.num_bindings
    }

    /// Get the workgroup size.
    pub fn workgroup_size(&self) -> WorkgroupSize {
        self.workgroup_size
    }

    /// Get the backend handle.
    pub fn backend_handle(&self) -> u64 {
        self.backend_handle
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kernel_creation() {
        let k = GpuKernel::new("vector_add".into(), 3, WorkgroupSize::d1(256), 0);
        assert_eq!(k.name(), "vector_add");
        assert_eq!(k.num_bindings(), 3);
        assert_eq!(k.workgroup_size().total(), 256);
    }

    #[test]
    fn kernel_unique_ids() {
        let k1 = GpuKernel::new("a".into(), 2, WorkgroupSize::default(), 0);
        let k2 = GpuKernel::new("b".into(), 2, WorkgroupSize::default(), 0);
        assert_ne!(k1.id(), k2.id());
    }

    #[test]
    fn workgroup_size_variants() {
        let d1 = WorkgroupSize::d1(64);
        assert_eq!(d1.total(), 64);

        let d2 = WorkgroupSize::d2(16, 16);
        assert_eq!(d2.total(), 256);

        let def = WorkgroupSize::default();
        assert_eq!(def.x, 256);
        assert_eq!(def.total(), 256);
    }

    #[test]
    fn builtin_kernel_display() {
        assert_eq!(format!("{}", BuiltinKernel::Relu), "relu");
        assert_eq!(format!("{}", BuiltinKernel::Matmul), "matmul");
        assert_eq!(format!("{}", BuiltinKernel::VectorAdd), "vector_add");
    }

    #[test]
    fn kernel_source_variants() {
        let wgsl = KernelSource::Wgsl("@compute fn main() {}".into());
        let builtin = KernelSource::Builtin(BuiltinKernel::Relu);
        let ptx = KernelSource::Ptx(".entry kernel() {}".into());

        assert!(matches!(wgsl, KernelSource::Wgsl(_)));
        assert!(matches!(
            builtin,
            KernelSource::Builtin(BuiltinKernel::Relu)
        ));
        assert!(matches!(ptx, KernelSource::Ptx(_)));
    }
}
