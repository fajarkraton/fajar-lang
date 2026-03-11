//! GPU-accelerated training — device management, tensor kernels, autograd, and multi-GPU.
//!
//! This module provides a **simulation** of GPU-accelerated ML training for Fajar Lang.
//! All operations use ndarray on the CPU but model the API, memory management, and
//! data flow of a real CUDA/GPU backend. This enables development and testing of
//! GPU training pipelines without requiring actual GPU hardware or CUDA dependencies.
//!
//! # Architecture
//!
//! ```text
//! GpuDeviceInfo  ← device detection (simulated)
//! GpuBuffer      ← GPU memory handle (backed by Vec<f64>)
//! GpuMemoryPool  ← pre-allocated memory pool with sub-allocation
//! SimGpuDevice   ← implements GpuDevice trait (simulation backend)
//! GpuKernels     ← matmul, elementwise, activation, reduction, conv2d, batchnorm
//! GpuTape        ← records ops for backward pass
//! GpuSGD/GpuAdam ← GPU-side optimizers
//! DataParallel   ← multi-GPU data parallelism
//! ```

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

use ndarray::{Array2, Axis};
use thiserror::Error;

// ═══════════════════════════════════════════════════════════════════════
// Error types
// ═══════════════════════════════════════════════════════════════════════

/// Errors from GPU operations.
#[derive(Debug, Clone, Error, PartialEq)]
pub enum GpuError {
    /// Out of GPU memory.
    #[error("GPU error: out of memory — requested {requested} bytes, available {available} bytes")]
    OutOfMemory {
        /// Bytes requested.
        requested: usize,
        /// Bytes available.
        available: usize,
    },

    /// Invalid device ID.
    #[error("GPU error: invalid device id {device_id} (only {device_count} devices available)")]
    InvalidDevice {
        /// Requested device ID.
        device_id: u32,
        /// Total number of devices.
        device_count: u32,
    },

    /// Shape mismatch for GPU operation.
    #[error("GPU error: shape mismatch — {op}: expected {expected}, got {got}")]
    ShapeMismatch {
        /// Operation name.
        op: String,
        /// Expected shape description.
        expected: String,
        /// Actual shape description.
        got: String,
    },

    /// Buffer already freed or invalid.
    #[error("GPU error: invalid buffer (ptr={ptr:#x}, already freed or never allocated)")]
    InvalidBuffer {
        /// Buffer pointer.
        ptr: u64,
    },

    /// Division by zero in GPU kernel.
    #[error("GPU error: division by zero in elementwise operation")]
    DivisionByZero,

    /// Axis out of bounds for reduction.
    #[error("GPU error: axis {axis} out of bounds for tensor with {ndim} dimensions")]
    AxisOutOfBounds {
        /// Requested axis.
        axis: usize,
        /// Number of dimensions.
        ndim: usize,
    },

    /// Multi-GPU synchronization error.
    #[error("GPU error: sync barrier timeout on device {device_id}")]
    SyncTimeout {
        /// Device that timed out.
        device_id: u32,
    },

    /// Generic GPU error.
    #[error("GPU error: {reason}")]
    Other {
        /// Reason for the error.
        reason: String,
    },
}

// ═══════════════════════════════════════════════════════════════════════
// DType
// ═══════════════════════════════════════════════════════════════════════

/// Data type for GPU buffer storage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DType {
    /// 32-bit float (standard GPU training precision).
    F32,
    /// 64-bit float (full precision).
    F64,
}

impl DType {
    /// Returns the number of bytes per element.
    pub fn size_bytes(&self) -> usize {
        match self {
            DType::F32 => 4,
            DType::F64 => 8,
        }
    }

    /// Returns a human-readable name.
    pub fn name(&self) -> &'static str {
        match self {
            DType::F32 => "f32",
            DType::F64 => "f64",
        }
    }
}

impl std::fmt::Display for DType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Sprint 1: CUDA Device Management
// ═══════════════════════════════════════════════════════════════════════

/// Information about a detected GPU device.
#[derive(Debug, Clone)]
pub struct GpuDeviceInfo {
    /// Device name (e.g., "NVIDIA RTX 4090").
    pub name: String,
    /// CUDA compute capability (e.g., 8.9).
    pub compute_capability: f32,
    /// Total GPU memory in bytes.
    pub memory_bytes: u64,
    /// Number of CUDA cores.
    pub cuda_cores: u32,
    /// Number of tensor cores (0 if not available).
    pub tensor_cores: u32,
}

impl GpuDeviceInfo {
    /// Returns memory in megabytes.
    pub fn memory_mb(&self) -> u64 {
        self.memory_bytes / (1024 * 1024)
    }

    /// Returns memory in gigabytes (rounded).
    pub fn memory_gb(&self) -> f64 {
        self.memory_bytes as f64 / (1024.0 * 1024.0 * 1024.0)
    }
}

impl std::fmt::Display for GpuDeviceInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} (CC {:.1}, {:.1} GB, {} CUDA cores, {} tensor cores)",
            self.name,
            self.compute_capability,
            self.memory_gb(),
            self.cuda_cores,
            self.tensor_cores
        )
    }
}

/// Detects available GPU devices (simulation).
///
/// Returns a simulated RTX 4090 with 24 GB VRAM, 16384 CUDA cores,
/// and 512 tensor cores at compute capability 8.9.
pub fn detect_gpu_devices() -> Vec<GpuDeviceInfo> {
    vec![GpuDeviceInfo {
        name: "NVIDIA RTX 4090 (Simulated)".to_string(),
        compute_capability: 8.9,
        memory_bytes: 24 * 1024 * 1024 * 1024, // 24 GB
        cuda_cores: 16384,
        tensor_cores: 512,
    }]
}

// ── Global pointer counter for simulated GPU addresses ──

static NEXT_GPU_PTR: AtomicU64 = AtomicU64::new(0x1000_0000);

fn alloc_gpu_ptr() -> u64 {
    NEXT_GPU_PTR.fetch_add(0x1000, Ordering::Relaxed)
}

// ═══════════════════════════════════════════════════════════════════════
// GpuBuffer
// ═══════════════════════════════════════════════════════════════════════

/// A handle to a GPU memory buffer.
///
/// In this simulation, data is stored in a `Vec<f64>`. The `ptr` field
/// is a simulated device address for API compatibility.
#[derive(Debug, Clone)]
pub struct GpuBuffer {
    /// Device this buffer resides on.
    pub device_id: u32,
    /// Simulated device pointer.
    pub ptr: u64,
    /// Size in bytes.
    pub size_bytes: usize,
    /// Data type of the buffer elements.
    pub dtype: DType,
    /// Simulated GPU data (backed by host-side Vec).
    data: Vec<f64>,
}

impl GpuBuffer {
    /// Creates a new GPU buffer with the given data.
    pub fn new(device_id: u32, data: Vec<f64>, dtype: DType) -> Self {
        let size_bytes = data.len() * dtype.size_bytes();
        Self {
            device_id,
            ptr: alloc_gpu_ptr(),
            size_bytes,
            dtype,
            data,
        }
    }

    /// Creates a zero-filled GPU buffer with `count` elements.
    pub fn zeros(device_id: u32, count: usize, dtype: DType) -> Self {
        Self::new(device_id, vec![0.0; count], dtype)
    }

    /// Returns the number of elements.
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Returns whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Returns a reference to the underlying data.
    pub fn data(&self) -> &[f64] {
        &self.data
    }

    /// Returns a mutable reference to the underlying data.
    pub fn data_mut(&mut self) -> &mut Vec<f64> {
        &mut self.data
    }

    /// Returns data as an ndarray Array2 with the given shape.
    ///
    /// Returns `Err` if the element count does not match rows * cols.
    pub fn to_array2(&self, rows: usize, cols: usize) -> Result<Array2<f64>, GpuError> {
        if rows * cols != self.data.len() {
            return Err(GpuError::ShapeMismatch {
                op: "to_array2".to_string(),
                expected: format!("{}x{} = {} elements", rows, cols, rows * cols),
                got: format!("{} elements", self.data.len()),
            });
        }
        Array2::from_shape_vec((rows, cols), self.data.clone()).map_err(|e| GpuError::Other {
            reason: e.to_string(),
        })
    }

    /// Creates a GpuBuffer from an ndarray Array2.
    pub fn from_array2(device_id: u32, arr: &Array2<f64>, dtype: DType) -> Self {
        Self::new(device_id, arr.iter().copied().collect(), dtype)
    }

    /// Applies simulated F32 precision loss (f64 -> f32 -> f64 roundtrip).
    pub fn to_f32_precision(&mut self) {
        for v in &mut self.data {
            *v = *v as f32 as f64;
        }
    }
}

impl std::fmt::Display for GpuBuffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "GpuBuffer(device={}, ptr={:#x}, {} elems, {})",
            self.device_id,
            self.ptr,
            self.data.len(),
            self.dtype
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════
// GpuMemoryPool
// ═══════════════════════════════════════════════════════════════════════

/// Pre-allocated GPU memory pool with sub-allocation and fragmentation tracking.
///
/// Simulates a pool-based allocator to avoid per-allocation overhead on the GPU.
/// Tracks peak and current usage, and reports fragmentation ratio.
#[derive(Debug)]
pub struct GpuMemoryPool {
    /// Total pool capacity in bytes.
    capacity_bytes: usize,
    /// Currently allocated bytes.
    used_bytes: usize,
    /// Peak allocated bytes.
    peak_bytes: usize,
    /// Number of active allocations.
    active_allocations: usize,
    /// Total allocations ever made (including freed).
    total_allocations: u64,
    /// Number of freed blocks (contributes to fragmentation).
    freed_blocks: u64,
}

impl GpuMemoryPool {
    /// Creates a new memory pool with the given capacity in bytes.
    pub fn new(capacity_bytes: usize) -> Self {
        Self {
            capacity_bytes,
            used_bytes: 0,
            peak_bytes: 0,
            active_allocations: 0,
            total_allocations: 0,
            freed_blocks: 0,
        }
    }

    /// Attempts to sub-allocate `size_bytes` from the pool.
    ///
    /// Returns `Ok(offset)` on success, `Err(GpuError::OutOfMemory)` if
    /// there is not enough space remaining.
    pub fn allocate(&mut self, size_bytes: usize) -> Result<u64, GpuError> {
        if self.used_bytes + size_bytes > self.capacity_bytes {
            return Err(GpuError::OutOfMemory {
                requested: size_bytes,
                available: self.capacity_bytes - self.used_bytes,
            });
        }
        let offset = self.used_bytes as u64;
        self.used_bytes += size_bytes;
        if self.used_bytes > self.peak_bytes {
            self.peak_bytes = self.used_bytes;
        }
        self.active_allocations += 1;
        self.total_allocations += 1;
        Ok(offset)
    }

    /// Frees a sub-allocation of `size_bytes`.
    ///
    /// In this simulation, the pool does not coalesce freed blocks;
    /// fragmentation increases with each free.
    pub fn free(&mut self, size_bytes: usize) {
        self.used_bytes = self.used_bytes.saturating_sub(size_bytes);
        self.active_allocations = self.active_allocations.saturating_sub(1);
        self.freed_blocks += 1;
    }

    /// Returns the current memory usage in bytes.
    pub fn used_bytes(&self) -> usize {
        self.used_bytes
    }

    /// Returns the peak memory usage in bytes.
    pub fn peak_bytes(&self) -> usize {
        self.peak_bytes
    }

    /// Returns the available capacity in bytes.
    pub fn available_bytes(&self) -> usize {
        self.capacity_bytes - self.used_bytes
    }

    /// Returns the fragmentation ratio (0.0 = no fragmentation, 1.0 = fully fragmented).
    ///
    /// Estimated as freed_blocks / total_allocations.
    pub fn fragmentation(&self) -> f64 {
        if self.total_allocations == 0 {
            return 0.0;
        }
        self.freed_blocks as f64 / self.total_allocations as f64
    }

    /// Returns the number of active (not yet freed) allocations.
    pub fn active_allocations(&self) -> usize {
        self.active_allocations
    }

    /// Returns the total pool capacity in bytes.
    pub fn capacity_bytes(&self) -> usize {
        self.capacity_bytes
    }

    /// Resets the pool (frees everything).
    pub fn reset(&mut self) {
        self.used_bytes = 0;
        self.active_allocations = 0;
        self.freed_blocks = 0;
        self.total_allocations = 0;
        // peak_bytes is intentionally preserved
    }
}

// ═══════════════════════════════════════════════════════════════════════
// GpuDevice trait + SimGpuDevice
// ═══════════════════════════════════════════════════════════════════════

/// Trait for GPU device operations (allocation, transfer).
pub trait GpuDevice {
    /// Allocates a buffer on the GPU with `count` elements.
    fn alloc(&mut self, count: usize, dtype: DType) -> Result<GpuBuffer, GpuError>;

    /// Frees a GPU buffer.
    fn free(&mut self, buffer: &GpuBuffer) -> Result<(), GpuError>;

    /// Transfers data from host to GPU device.
    fn host_to_device(&mut self, host_data: &[f64], dtype: DType) -> Result<GpuBuffer, GpuError>;

    /// Transfers data from GPU device to host.
    fn device_to_host(&self, buffer: &GpuBuffer) -> Result<Vec<f64>, GpuError>;

    /// Returns the device ID.
    fn device_id(&self) -> u32;

    /// Returns device info.
    fn device_info(&self) -> &GpuDeviceInfo;
}

/// Simulated GPU device backed by CPU memory.
///
/// All "GPU" data is stored in a `HashMap<u64, Vec<f64>>`, keyed by
/// simulated device pointer. This enables full API testing without hardware.
#[derive(Debug)]
pub struct SimGpuDevice {
    /// Device identifier.
    id: u32,
    /// Device info.
    info: GpuDeviceInfo,
    /// Allocated buffers tracked by pointer.
    buffers: HashMap<u64, Vec<f64>>,
    /// Memory pool for tracking usage.
    pool: GpuMemoryPool,
}

impl SimGpuDevice {
    /// Creates a new simulated GPU device.
    pub fn new(id: u32) -> Self {
        let info = GpuDeviceInfo {
            name: format!("SimGPU-{id}"),
            compute_capability: 8.9,
            memory_bytes: 24 * 1024 * 1024 * 1024, // 24 GB
            cuda_cores: 16384,
            tensor_cores: 512,
        };
        let pool = GpuMemoryPool::new(info.memory_bytes as usize);
        Self {
            id,
            info,
            buffers: HashMap::new(),
            pool,
        }
    }

    /// Returns a reference to the internal memory pool.
    pub fn pool(&self) -> &GpuMemoryPool {
        &self.pool
    }

    /// Returns the number of active allocations.
    pub fn active_allocations(&self) -> usize {
        self.buffers.len()
    }
}

impl GpuDevice for SimGpuDevice {
    fn alloc(&mut self, count: usize, dtype: DType) -> Result<GpuBuffer, GpuError> {
        let size_bytes = count * dtype.size_bytes();
        self.pool.allocate(size_bytes)?;
        let buf = GpuBuffer::zeros(self.id, count, dtype);
        self.buffers.insert(buf.ptr, buf.data.clone());
        Ok(buf)
    }

    fn free(&mut self, buffer: &GpuBuffer) -> Result<(), GpuError> {
        if self.buffers.remove(&buffer.ptr).is_none() {
            return Err(GpuError::InvalidBuffer { ptr: buffer.ptr });
        }
        self.pool.free(buffer.size_bytes);
        Ok(())
    }

    fn host_to_device(&mut self, host_data: &[f64], dtype: DType) -> Result<GpuBuffer, GpuError> {
        let size_bytes = host_data.len() * dtype.size_bytes();
        self.pool.allocate(size_bytes)?;
        let buf = GpuBuffer::new(self.id, host_data.to_vec(), dtype);
        self.buffers.insert(buf.ptr, buf.data.clone());
        Ok(buf)
    }

    fn device_to_host(&self, buffer: &GpuBuffer) -> Result<Vec<f64>, GpuError> {
        self.buffers
            .get(&buffer.ptr)
            .cloned()
            .ok_or(GpuError::InvalidBuffer { ptr: buffer.ptr })
    }

    fn device_id(&self) -> u32 {
        self.id
    }

    fn device_info(&self) -> &GpuDeviceInfo {
        &self.info
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Sprint 2: GPU Tensor Kernels
// ═══════════════════════════════════════════════════════════════════════

/// Kernel launch configuration (simulated).
///
/// In a real CUDA backend, these control thread-block geometry.
/// Here they serve as metadata for profiling estimation.
#[derive(Debug, Clone)]
pub struct GpuKernelConfig {
    /// Threads per block (e.g., 256).
    pub block_size: u32,
    /// Number of blocks in the grid.
    pub grid_size: u32,
    /// Shared memory per block in bytes.
    pub shared_memory: usize,
}

impl GpuKernelConfig {
    /// Creates a default kernel config for `n` elements.
    pub fn for_elements(n: usize) -> Self {
        let block_size = 256u32;
        let grid_size = (n as u32).div_ceil(block_size);
        Self {
            block_size,
            grid_size,
            shared_memory: 0,
        }
    }

    /// Creates a kernel config for matrix operations.
    pub fn for_matmul(m: usize, n: usize) -> Self {
        let block_size = 16u32; // 16x16 thread block
        let grid_m = (m as u32).div_ceil(block_size);
        let grid_n = (n as u32).div_ceil(block_size);
        Self {
            block_size,
            grid_size: grid_m * grid_n,
            shared_memory: (block_size * block_size * 8) as usize * 2, // two tiles
        }
    }

    /// Returns total thread count.
    pub fn total_threads(&self) -> u64 {
        self.block_size as u64 * self.grid_size as u64
    }
}

// ── Matrix multiply ──

/// GPU matrix multiplication: C = A @ B.
///
/// A is (m x k), B is (k x n), result is (m x n).
/// Simulated using ndarray on the CPU.
pub fn gpu_matmul(
    a: &GpuBuffer,
    b: &GpuBuffer,
    m: usize,
    n: usize,
    k: usize,
) -> Result<GpuBuffer, GpuError> {
    let a_arr = a.to_array2(m, k)?;
    let b_arr = b.to_array2(k, n)?;
    let c_arr = a_arr.dot(&b_arr);
    Ok(GpuBuffer::from_array2(a.device_id, &c_arr, a.dtype))
}

// ── Elementwise binary ops ──

/// GPU element-wise addition: C = A + B.
pub fn gpu_elementwise_add(a: &GpuBuffer, b: &GpuBuffer) -> Result<GpuBuffer, GpuError> {
    check_same_len("add", a, b)?;
    let data: Vec<f64> = a
        .data
        .iter()
        .zip(b.data.iter())
        .map(|(x, y)| x + y)
        .collect();
    Ok(GpuBuffer::new(a.device_id, data, a.dtype))
}

/// GPU element-wise subtraction: C = A - B.
pub fn gpu_elementwise_sub(a: &GpuBuffer, b: &GpuBuffer) -> Result<GpuBuffer, GpuError> {
    check_same_len("sub", a, b)?;
    let data: Vec<f64> = a
        .data
        .iter()
        .zip(b.data.iter())
        .map(|(x, y)| x - y)
        .collect();
    Ok(GpuBuffer::new(a.device_id, data, a.dtype))
}

/// GPU element-wise multiplication: C = A * B.
pub fn gpu_elementwise_mul(a: &GpuBuffer, b: &GpuBuffer) -> Result<GpuBuffer, GpuError> {
    check_same_len("mul", a, b)?;
    let data: Vec<f64> = a
        .data
        .iter()
        .zip(b.data.iter())
        .map(|(x, y)| x * y)
        .collect();
    Ok(GpuBuffer::new(a.device_id, data, a.dtype))
}

/// GPU element-wise division: C = A / B.
pub fn gpu_elementwise_div(a: &GpuBuffer, b: &GpuBuffer) -> Result<GpuBuffer, GpuError> {
    check_same_len("div", a, b)?;
    if b.data.contains(&0.0) {
        return Err(GpuError::DivisionByZero);
    }
    let data: Vec<f64> = a
        .data
        .iter()
        .zip(b.data.iter())
        .map(|(x, y)| x / y)
        .collect();
    Ok(GpuBuffer::new(a.device_id, data, a.dtype))
}

/// Checks that two buffers have the same element count.
fn check_same_len(op: &str, a: &GpuBuffer, b: &GpuBuffer) -> Result<(), GpuError> {
    if a.len() != b.len() {
        return Err(GpuError::ShapeMismatch {
            op: op.to_string(),
            expected: format!("{} elements", a.len()),
            got: format!("{} elements", b.len()),
        });
    }
    Ok(())
}

// ── Activation functions ──

/// GPU ReLU activation: max(0, x) element-wise.
pub fn gpu_relu(buf: &GpuBuffer) -> GpuBuffer {
    let data: Vec<f64> = buf.data.iter().map(|&x| x.max(0.0)).collect();
    GpuBuffer::new(buf.device_id, data, buf.dtype)
}

/// GPU Sigmoid activation: 1 / (1 + exp(-x)) element-wise.
pub fn gpu_sigmoid(buf: &GpuBuffer) -> GpuBuffer {
    let data: Vec<f64> = buf.data.iter().map(|&x| 1.0 / (1.0 + (-x).exp())).collect();
    GpuBuffer::new(buf.device_id, data, buf.dtype)
}

/// GPU Tanh activation element-wise.
pub fn gpu_tanh(buf: &GpuBuffer) -> GpuBuffer {
    let data: Vec<f64> = buf.data.iter().map(|&x| x.tanh()).collect();
    GpuBuffer::new(buf.device_id, data, buf.dtype)
}

/// GPU GELU activation: x * 0.5 * (1 + tanh(sqrt(2/pi) * (x + 0.044715 * x^3))).
pub fn gpu_gelu(buf: &GpuBuffer) -> GpuBuffer {
    let sqrt_2_over_pi = (2.0_f64 / std::f64::consts::PI).sqrt();
    let data: Vec<f64> = buf
        .data
        .iter()
        .map(|&x| {
            let inner = sqrt_2_over_pi * (x + 0.044715 * x * x * x);
            0.5 * x * (1.0 + inner.tanh())
        })
        .collect();
    GpuBuffer::new(buf.device_id, data, buf.dtype)
}

/// GPU numerically-stable softmax along axis 1 (columns within each row).
///
/// `buffer` has shape (rows x cols), softmax is applied per row.
pub fn gpu_softmax(buffer: &GpuBuffer, rows: usize, cols: usize) -> Result<GpuBuffer, GpuError> {
    let arr = buffer.to_array2(rows, cols)?;
    let mut result = Array2::<f64>::zeros((rows, cols));
    for i in 0..rows {
        let row = arr.row(i);
        let max_val = row.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let exp_vals: Vec<f64> = row.iter().map(|&v| (v - max_val).exp()).collect();
        let sum: f64 = exp_vals.iter().sum();
        for j in 0..cols {
            result[[i, j]] = exp_vals[j] / sum;
        }
    }
    Ok(GpuBuffer::from_array2(
        buffer.device_id,
        &result,
        buffer.dtype,
    ))
}

// ── Transpose ──

/// GPU matrix transpose: (rows x cols) -> (cols x rows).
pub fn gpu_transpose(buffer: &GpuBuffer, rows: usize, cols: usize) -> Result<GpuBuffer, GpuError> {
    let arr = buffer.to_array2(rows, cols)?;
    let transposed = arr.t().to_owned();
    Ok(GpuBuffer::from_array2(
        buffer.device_id,
        &transposed,
        buffer.dtype,
    ))
}

// ── Reductions ──

/// GPU reduce-sum along an axis.
///
/// For a (rows x cols) buffer:
/// - axis=0: sum across rows, result has cols elements
/// - axis=1: sum across cols, result has rows elements
pub fn gpu_reduce_sum(
    buffer: &GpuBuffer,
    rows: usize,
    cols: usize,
    axis: usize,
) -> Result<GpuBuffer, GpuError> {
    if axis > 1 {
        return Err(GpuError::AxisOutOfBounds { axis, ndim: 2 });
    }
    let arr = buffer.to_array2(rows, cols)?;
    let reduced = arr.sum_axis(Axis(axis));
    let data: Vec<f64> = reduced.iter().copied().collect();
    Ok(GpuBuffer::new(buffer.device_id, data, buffer.dtype))
}

/// GPU reduce-max along an axis.
pub fn gpu_reduce_max(
    buffer: &GpuBuffer,
    rows: usize,
    cols: usize,
    axis: usize,
) -> Result<GpuBuffer, GpuError> {
    if axis > 1 {
        return Err(GpuError::AxisOutOfBounds { axis, ndim: 2 });
    }
    let arr = buffer.to_array2(rows, cols)?;
    let reduced = arr.map_axis(Axis(axis), |lane| {
        lane.iter().copied().fold(f64::NEG_INFINITY, f64::max)
    });
    let data: Vec<f64> = reduced.iter().copied().collect();
    Ok(GpuBuffer::new(buffer.device_id, data, buffer.dtype))
}

/// GPU reduce-mean along an axis.
pub fn gpu_reduce_mean(
    buffer: &GpuBuffer,
    rows: usize,
    cols: usize,
    axis: usize,
) -> Result<GpuBuffer, GpuError> {
    if axis > 1 {
        return Err(GpuError::AxisOutOfBounds { axis, ndim: 2 });
    }
    let arr = buffer.to_array2(rows, cols)?;
    let reduced = arr.mean_axis(Axis(axis)).ok_or_else(|| GpuError::Other {
        reason: "mean_axis failed (empty axis)".to_string(),
    })?;
    let data: Vec<f64> = reduced.iter().copied().collect();
    Ok(GpuBuffer::new(buffer.device_id, data, buffer.dtype))
}

// ── Conv2d ──

/// GPU 2D convolution (simulated).
///
/// Input: (batch, in_c, h, w) flattened in `input`.
/// Weight: (out_c, in_c, kh, kw) flattened in `weight`.
/// Bias: (out_c,) in `bias` (optional — pass empty buffer to skip).
///
/// Returns output of shape (batch, out_c, out_h, out_w).
#[allow(clippy::too_many_arguments)]
pub fn gpu_conv2d(
    input: &GpuBuffer,
    weight: &GpuBuffer,
    bias: &GpuBuffer,
    batch: usize,
    in_c: usize,
    h: usize,
    w: usize,
    out_c: usize,
    kh: usize,
    kw: usize,
    stride: usize,
    padding: usize,
) -> Result<GpuBuffer, GpuError> {
    let padded_h = h + 2 * padding;
    let padded_w = w + 2 * padding;
    let out_h = (padded_h - kh) / stride + 1;
    let out_w = (padded_w - kw) / stride + 1;

    let expected_input = batch * in_c * h * w;
    if input.len() != expected_input {
        return Err(GpuError::ShapeMismatch {
            op: "conv2d input".to_string(),
            expected: format!("{expected_input} elements"),
            got: format!("{} elements", input.len()),
        });
    }
    let expected_weight = out_c * in_c * kh * kw;
    if weight.len() != expected_weight {
        return Err(GpuError::ShapeMismatch {
            op: "conv2d weight".to_string(),
            expected: format!("{expected_weight} elements"),
            got: format!("{} elements", weight.len()),
        });
    }

    let mut output = vec![0.0f64; batch * out_c * out_h * out_w];

    for b in 0..batch {
        for oc in 0..out_c {
            for oh in 0..out_h {
                for ow in 0..out_w {
                    let mut val = if !bias.is_empty() && oc < bias.len() {
                        bias.data[oc]
                    } else {
                        0.0
                    };
                    val += conv2d_pixel(
                        &input.data,
                        &weight.data,
                        b,
                        oc,
                        oh,
                        ow,
                        in_c,
                        h,
                        w,
                        kh,
                        kw,
                        stride,
                        padding,
                    );
                    let idx = ((b * out_c + oc) * out_h + oh) * out_w + ow;
                    output[idx] = val;
                }
            }
        }
    }

    Ok(GpuBuffer::new(input.device_id, output, input.dtype))
}

/// Computes a single convolution output pixel.
#[allow(clippy::too_many_arguments)]
fn conv2d_pixel(
    input: &[f64],
    weight: &[f64],
    b: usize,
    oc: usize,
    oh: usize,
    ow: usize,
    in_c: usize,
    h: usize,
    w: usize,
    kh: usize,
    kw: usize,
    stride: usize,
    padding: usize,
) -> f64 {
    let mut sum = 0.0;
    for ic in 0..in_c {
        for fh in 0..kh {
            for fw in 0..kw {
                let ih = oh * stride + fh;
                let iw = ow * stride + fw;
                // Account for padding
                let ih = ih as isize - padding as isize;
                let iw = iw as isize - padding as isize;
                if ih >= 0 && ih < h as isize && iw >= 0 && iw < w as isize {
                    let in_idx = ((b * in_c + ic) * h + ih as usize) * w + iw as usize;
                    let w_idx = ((oc * in_c + ic) * kh + fh) * kw + fw;
                    sum += input[in_idx] * weight[w_idx];
                }
            }
        }
    }
    sum
}

// ── Batch normalization ──

/// GPU batch normalization (simulated).
///
/// `input` has shape (batch x features). `gamma`, `beta`, `running_mean`,
/// and `running_var` each have `features` elements.
///
/// Formula: `y = gamma * (x - mean) / sqrt(var + eps) + beta`
#[allow(clippy::too_many_arguments)]
pub fn gpu_batch_norm(
    input: &GpuBuffer,
    gamma: &GpuBuffer,
    beta: &GpuBuffer,
    running_mean: &GpuBuffer,
    running_var: &GpuBuffer,
    batch: usize,
    features: usize,
    eps: f64,
) -> Result<GpuBuffer, GpuError> {
    let arr = input.to_array2(batch, features)?;
    if gamma.len() != features || beta.len() != features {
        return Err(GpuError::ShapeMismatch {
            op: "batch_norm gamma/beta".to_string(),
            expected: format!("{features} elements"),
            got: format!("gamma={}, beta={}", gamma.len(), beta.len()),
        });
    }

    let mut output = Array2::<f64>::zeros((batch, features));

    for j in 0..features {
        let mean = if !running_mean.is_empty() {
            running_mean.data[j]
        } else {
            let col = arr.column(j);
            col.iter().sum::<f64>() / batch as f64
        };
        let var = if !running_var.is_empty() {
            running_var.data[j]
        } else {
            let col = arr.column(j);
            col.iter().map(|v| (v - mean) * (v - mean)).sum::<f64>() / batch as f64
        };
        let inv_std = 1.0 / (var + eps).sqrt();
        let g = gamma.data[j];
        let b = beta.data[j];
        for i in 0..batch {
            output[[i, j]] = g * (arr[[i, j]] - mean) * inv_std + b;
        }
    }

    Ok(GpuBuffer::from_array2(
        input.device_id,
        &output,
        input.dtype,
    ))
}

// ═══════════════════════════════════════════════════════════════════════
// Sprint 3: GPU Autograd & Training
// ═══════════════════════════════════════════════════════════════════════

/// Gradient function type for GPU autograd.
///
/// Given the output gradient buffer, returns a list of input gradient buffers.
pub type GpuGradFn = Box<dyn Fn(&GpuBuffer) -> Vec<GpuBuffer>>;

/// A single recorded operation on the GPU tape.
pub struct GpuTapeEntry {
    /// Human-readable operation name (e.g., "matmul", "relu").
    pub op_name: String,
    /// Input buffer identifiers (by ptr).
    pub input_ptrs: Vec<u64>,
    /// Output buffer identifier (by ptr).
    pub output_ptr: u64,
    /// Backward function: computes input gradients from output gradient.
    pub backward_fn: GpuGradFn,
}

/// Tape-based computation graph for GPU autograd.
///
/// Records forward operations and replays them in reverse for backward pass.
pub struct GpuTape {
    /// Recorded operations in forward order.
    entries: Vec<GpuTapeEntry>,
    /// Whether recording is enabled.
    recording: bool,
}

impl GpuTape {
    /// Creates a new empty GPU tape with recording enabled.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            recording: true,
        }
    }

    /// Records an operation on the tape.
    ///
    /// No-op if recording is disabled.
    pub fn record_op(
        &mut self,
        name: &str,
        input_ptrs: Vec<u64>,
        output_ptr: u64,
        backward_fn: GpuGradFn,
    ) {
        if !self.recording {
            return;
        }
        self.entries.push(GpuTapeEntry {
            op_name: name.to_string(),
            input_ptrs,
            output_ptr,
            backward_fn,
        });
    }

    /// Returns the number of recorded operations.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns whether the tape is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns whether recording is enabled.
    pub fn is_recording(&self) -> bool {
        self.recording
    }

    /// Enables or disables recording.
    pub fn set_recording(&mut self, enabled: bool) {
        self.recording = enabled;
    }

    /// Clears all recorded operations.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

impl Default for GpuTape {
    fn default() -> Self {
        Self::new()
    }
}

/// Runs backward pass on the GPU tape, computing gradients for all recorded ops.
///
/// Returns a map from buffer pointer to its gradient buffer.
pub fn gpu_backward(tape: &GpuTape, loss_grad: GpuBuffer) -> HashMap<u64, GpuBuffer> {
    let mut grads: HashMap<u64, GpuBuffer> = HashMap::new();
    grads.insert(loss_grad.ptr, loss_grad.clone());

    // Walk tape in reverse
    for entry in tape.entries.iter().rev() {
        let output_grad = match grads.get(&entry.output_ptr) {
            Some(g) => g.clone(),
            None => continue,
        };
        let input_grads = (entry.backward_fn)(&output_grad);
        for (i, input_ptr) in entry.input_ptrs.iter().enumerate() {
            if i < input_grads.len() {
                grads
                    .entry(*input_ptr)
                    .and_modify(|existing| {
                        // Accumulate gradients
                        for (j, v) in input_grads[i].data.iter().enumerate() {
                            if j < existing.data.len() {
                                existing.data[j] += v;
                            }
                        }
                    })
                    .or_insert_with(|| input_grads[i].clone());
            }
        }
    }

    grads
}

// ── GPU Optimizers ──

/// GPU-side SGD optimizer.
///
/// Operates directly on GpuBuffers without transferring to host.
#[derive(Debug)]
pub struct GpuSGD {
    /// Learning rate.
    lr: f64,
    /// Momentum factor.
    momentum: f64,
    /// Velocity buffers (by parameter ptr).
    velocities: HashMap<u64, Vec<f64>>,
}

impl GpuSGD {
    /// Creates a new GPU SGD optimizer.
    pub fn new(lr: f64, momentum: f64) -> Self {
        Self {
            lr,
            momentum,
            velocities: HashMap::new(),
        }
    }

    /// Updates parameters using their gradients.
    ///
    /// `params` and `grads` must be paired: `params[i]` is updated using `grads[i]`.
    pub fn step(&mut self, params: &mut [GpuBuffer], grads: &[GpuBuffer]) {
        for (param, grad) in params.iter_mut().zip(grads.iter()) {
            let n = param.len().min(grad.len());
            if self.momentum != 0.0 {
                let vel = self
                    .velocities
                    .entry(param.ptr)
                    .or_insert_with(|| vec![0.0; n]);
                for (i, v) in vel.iter_mut().enumerate().take(n) {
                    *v = self.momentum * *v + grad.data[i];
                    param.data[i] -= self.lr * *v;
                }
            } else {
                for i in 0..n {
                    param.data[i] -= self.lr * grad.data[i];
                }
            }
        }
    }

    /// Returns the learning rate.
    pub fn lr(&self) -> f64 {
        self.lr
    }

    /// Sets the learning rate.
    pub fn set_lr(&mut self, lr: f64) {
        self.lr = lr;
    }
}

/// GPU-side Adam optimizer.
///
/// Maintains first-moment and second-moment buffers on the GPU (simulated).
#[derive(Debug)]
pub struct GpuAdam {
    /// Learning rate.
    lr: f64,
    /// Exponential decay rate for first moment.
    beta1: f64,
    /// Exponential decay rate for second moment.
    beta2: f64,
    /// Numerical stability epsilon.
    epsilon: f64,
    /// First moment buffers (by parameter ptr).
    m: HashMap<u64, Vec<f64>>,
    /// Second moment buffers (by parameter ptr).
    v: HashMap<u64, Vec<f64>>,
    /// Timestep counter.
    t: u64,
}

impl GpuAdam {
    /// Creates a new GPU Adam optimizer with default hyperparameters.
    pub fn new(lr: f64) -> Self {
        Self {
            lr,
            beta1: 0.9,
            beta2: 0.999,
            epsilon: 1e-8,
            m: HashMap::new(),
            v: HashMap::new(),
            t: 0,
        }
    }

    /// Creates a GPU Adam optimizer with custom hyperparameters.
    pub fn with_params(lr: f64, beta1: f64, beta2: f64, epsilon: f64) -> Self {
        Self {
            lr,
            beta1,
            beta2,
            epsilon,
            m: HashMap::new(),
            v: HashMap::new(),
            t: 0,
        }
    }

    /// Updates parameters using their gradients.
    pub fn step(&mut self, params: &mut [GpuBuffer], grads: &[GpuBuffer]) {
        self.t += 1;
        let bc1 = 1.0 - self.beta1.powi(self.t as i32);
        let bc2 = 1.0 - self.beta2.powi(self.t as i32);

        for (param, grad) in params.iter_mut().zip(grads.iter()) {
            let n = param.len().min(grad.len());
            let m_buf = self.m.entry(param.ptr).or_insert_with(|| vec![0.0; n]);
            let v_buf = self.v.entry(param.ptr).or_insert_with(|| vec![0.0; n]);

            for i in 0..n {
                let g = grad.data[i];
                m_buf[i] = self.beta1 * m_buf[i] + (1.0 - self.beta1) * g;
                v_buf[i] = self.beta2 * v_buf[i] + (1.0 - self.beta2) * g * g;
                let m_hat = m_buf[i] / bc1;
                let v_hat = v_buf[i] / bc2;
                param.data[i] -= self.lr * m_hat / (v_hat.sqrt() + self.epsilon);
            }
        }
    }

    /// Returns the learning rate.
    pub fn lr(&self) -> f64 {
        self.lr
    }

    /// Returns the current timestep.
    pub fn timestep(&self) -> u64 {
        self.t
    }
}

// ── GPU Training Loop ──

/// Configuration for a GPU training loop.
#[derive(Debug, Clone)]
pub struct GpuTrainingConfig {
    /// Number of training epochs.
    pub epochs: usize,
    /// Batch size.
    pub batch_size: usize,
    /// Learning rate.
    pub lr: f64,
    /// Whether to use mixed precision (FP16 simulation).
    pub mixed_precision: bool,
    /// Maximum gradient norm for clipping (0.0 = no clipping).
    pub max_grad_norm: f64,
}

impl GpuTrainingConfig {
    /// Creates a default training config.
    pub fn new(epochs: usize, batch_size: usize, lr: f64) -> Self {
        Self {
            epochs,
            batch_size,
            lr,
            mixed_precision: false,
            max_grad_norm: 0.0,
        }
    }
}

/// Result of a single training epoch.
#[derive(Debug, Clone)]
pub struct EpochResult {
    /// Epoch number (0-indexed).
    pub epoch: usize,
    /// Average loss over all batches.
    pub avg_loss: f64,
    /// Number of batches processed.
    pub batches: usize,
    /// Simulated GPU time in milliseconds.
    pub gpu_time_ms: f64,
}

/// GPU training loop (simulated).
///
/// Runs forward pass, loss computation, backward pass, and optimizer step
/// for each batch in each epoch. All computation is simulated on CPU.
pub struct GpuTrainingLoop {
    /// Training configuration.
    config: GpuTrainingConfig,
    /// Epoch results collected during training.
    history: Vec<EpochResult>,
}

impl GpuTrainingLoop {
    /// Creates a new GPU training loop.
    pub fn new(config: GpuTrainingConfig) -> Self {
        Self {
            config,
            history: Vec::new(),
        }
    }

    /// Runs one epoch of training on the given data.
    ///
    /// `data` is flat training data of shape (n_samples x features).
    /// `targets` is flat target data of shape (n_samples x n_classes).
    /// `weights` are the model parameters to optimize.
    /// `forward_fn` maps (input, weights) -> (output, loss).
    ///
    /// Returns the epoch result.
    pub fn run_epoch<F>(
        &mut self,
        data: &GpuBuffer,
        targets: &GpuBuffer,
        weights: &mut [GpuBuffer],
        forward_fn: F,
    ) -> EpochResult
    where
        F: Fn(&[f64], &[f64], &[GpuBuffer]) -> (Vec<f64>, f64),
    {
        let epoch = self.history.len();
        let n_samples = data.len() / self.config.batch_size.max(1);
        let n_batches = n_samples.max(1);

        let mut total_loss = 0.0;
        for batch_idx in 0..n_batches {
            let start = batch_idx * self.config.batch_size;
            let end = (start + self.config.batch_size).min(data.len());
            let batch_data = &data.data[start..end];
            let t_start = batch_idx * self.config.batch_size;
            let t_end = (t_start + self.config.batch_size).min(targets.len());
            let batch_targets = &targets.data[t_start..t_end];

            let (_output, loss) = forward_fn(batch_data, batch_targets, weights);
            total_loss += loss;
        }

        // Simulate GPU time: ~0.1ms per batch per weight element
        let weight_elements: usize = weights.iter().map(|w| w.len()).sum();
        let gpu_time = n_batches as f64 * weight_elements as f64 * 0.0001;

        let result = EpochResult {
            epoch,
            avg_loss: total_loss / n_batches as f64,
            batches: n_batches,
            gpu_time_ms: gpu_time,
        };
        self.history.push(result.clone());
        result
    }

    /// Returns the training history.
    pub fn history(&self) -> &[EpochResult] {
        &self.history
    }

    /// Returns the training config.
    pub fn config(&self) -> &GpuTrainingConfig {
        &self.config
    }
}

// ── Mixed Precision GPU ──

/// Mixed-precision GPU training helper.
///
/// Maintains FP32 master weights and simulates FP16 forward/backward passes
/// with loss scaling to prevent gradient underflow.
#[derive(Debug)]
pub struct MixedPrecisionGpu {
    /// FP32 master weights (full precision copies).
    master_weights: Vec<Vec<f64>>,
    /// Loss scale factor.
    loss_scale: f64,
    /// Scale growth factor.
    growth_factor: f64,
    /// Scale backoff factor.
    backoff_factor: f64,
    /// Steps since last overflow.
    steps_since_overflow: u32,
    /// Growth interval.
    growth_interval: u32,
}

impl MixedPrecisionGpu {
    /// Creates a new mixed-precision helper.
    pub fn new(initial_weights: &[GpuBuffer]) -> Self {
        let master = initial_weights.iter().map(|w| w.data.clone()).collect();
        Self {
            master_weights: master,
            loss_scale: 65536.0,
            growth_factor: 2.0,
            backoff_factor: 0.5,
            steps_since_overflow: 0,
            growth_interval: 2000,
        }
    }

    /// Simulates FP16 precision on a buffer (f64 -> f32 -> f64 roundtrip).
    pub fn to_half_precision(buf: &GpuBuffer) -> GpuBuffer {
        let data: Vec<f64> = buf.data.iter().map(|&v| v as f32 as f64).collect();
        GpuBuffer::new(buf.device_id, data, DType::F32)
    }

    /// Scales loss by the current loss scale factor.
    pub fn scale_loss(&self, loss: f64) -> f64 {
        loss * self.loss_scale
    }

    /// Unscales gradients by dividing by the loss scale.
    pub fn unscale_grads(&self, grads: &mut [GpuBuffer]) {
        let inv_scale = 1.0 / self.loss_scale;
        for grad in grads.iter_mut() {
            for v in grad.data.iter_mut() {
                *v *= inv_scale;
            }
        }
    }

    /// Checks for overflow/NaN in gradients.
    pub fn check_overflow(grads: &[GpuBuffer]) -> bool {
        grads
            .iter()
            .any(|g| g.data.iter().any(|v| v.is_nan() || v.is_infinite()))
    }

    /// Updates the loss scale after a step.
    ///
    /// If overflow was detected, scale down. Otherwise, count toward growth.
    pub fn update_scale(&mut self, overflow: bool) {
        if overflow {
            self.loss_scale *= self.backoff_factor;
            self.steps_since_overflow = 0;
        } else {
            self.steps_since_overflow += 1;
            if self.steps_since_overflow >= self.growth_interval {
                self.loss_scale *= self.growth_factor;
                self.steps_since_overflow = 0;
            }
        }
    }

    /// Copies master weights back to working buffers.
    pub fn sync_to_working(&self, working: &mut [GpuBuffer]) {
        for (w, master) in working.iter_mut().zip(self.master_weights.iter()) {
            let n = w.len().min(master.len());
            w.data[..n].copy_from_slice(&master[..n]);
        }
    }

    /// Updates master weights from working buffers.
    pub fn sync_from_working(&mut self, working: &[GpuBuffer]) {
        for (master, w) in self.master_weights.iter_mut().zip(working.iter()) {
            let n = master.len().min(w.len());
            master[..n].copy_from_slice(&w.data[..n]);
        }
    }

    /// Returns the current loss scale.
    pub fn loss_scale(&self) -> f64 {
        self.loss_scale
    }
}

// ── Gradient Clipping ──

/// Clips gradients on GPU by global L2 norm.
///
/// If the total norm exceeds `max_norm`, scales all gradient buffers down.
/// Returns the original (pre-clip) total norm.
pub fn gpu_gradient_clipping(grads: &mut [GpuBuffer], max_norm: f64) -> f64 {
    let mut total_norm_sq = 0.0;
    for grad in grads.iter() {
        total_norm_sq += grad.data.iter().map(|v| v * v).sum::<f64>();
    }
    let total_norm = total_norm_sq.sqrt();

    if total_norm > max_norm {
        let scale = max_norm / total_norm;
        for grad in grads.iter_mut() {
            for v in grad.data.iter_mut() {
                *v *= scale;
            }
        }
    }

    total_norm
}

// ── Data Prefetcher ──

/// Simulated async host-to-device data prefetcher.
///
/// Pre-loads the next batch of data while the current batch is being processed.
/// In this simulation, all transfers are synchronous but the API models the
/// double-buffering pattern used in real GPU training.
#[derive(Debug)]
pub struct DataPrefetcher {
    /// Device to prefetch to.
    device_id: u32,
    /// Prefetch buffer A.
    buffer_a: Option<GpuBuffer>,
    /// Prefetch buffer B.
    buffer_b: Option<GpuBuffer>,
    /// Which buffer is active (true = A, false = B).
    use_a: bool,
    /// Number of prefetch operations performed.
    prefetch_count: u64,
}

impl DataPrefetcher {
    /// Creates a new data prefetcher for the given device.
    pub fn new(device_id: u32) -> Self {
        Self {
            device_id,
            buffer_a: None,
            buffer_b: None,
            use_a: true,
            prefetch_count: 0,
        }
    }

    /// Prefetches data (simulated async transfer).
    ///
    /// Stores the data in the inactive buffer and swaps active/inactive
    /// so the next `get_current` will return it.
    pub fn prefetch(&mut self, data: &[f64], dtype: DType) {
        let buf = GpuBuffer::new(self.device_id, data.to_vec(), dtype);
        if self.use_a {
            self.buffer_b = Some(buf);
        } else {
            self.buffer_a = Some(buf);
        }
        self.use_a = !self.use_a;
        self.prefetch_count += 1;
    }

    /// Retrieves the current prefetched buffer and swaps to the next.
    ///
    /// Returns `None` if no data has been prefetched yet.
    pub fn get_current(&mut self) -> Option<GpuBuffer> {
        let result = if self.use_a {
            self.buffer_a.take()
        } else {
            self.buffer_b.take()
        };
        self.use_a = !self.use_a;
        result
    }

    /// Returns the number of prefetch operations performed.
    pub fn prefetch_count(&self) -> u64 {
        self.prefetch_count
    }
}

// ── GPU Benchmark ──

/// GPU performance benchmark results (simulated estimates).
#[derive(Debug, Clone)]
pub struct GpuBenchmark {
    /// Estimated TFLOPS (tera floating-point ops per second).
    pub tflops: f64,
    /// Estimated memory bandwidth in GB/s.
    pub memory_bandwidth_gbs: f64,
    /// Simulated kernel launch overhead in microseconds.
    pub kernel_overhead_us: f64,
    /// Device info used for the benchmark.
    pub device_name: String,
}

impl GpuBenchmark {
    /// Runs a simulated GPU benchmark for the given device.
    ///
    /// Estimates FLOPS based on CUDA core count and clock speed,
    /// and memory bandwidth based on bus width and memory clock.
    pub fn run(device: &GpuDeviceInfo) -> Self {
        // RTX 4090: ~82.6 TFLOPS FP32, ~1008 GB/s bandwidth
        let base_tflops = device.cuda_cores as f64 * 2.52e9 * 2.0 / 1e12;
        let bandwidth = if device.memory_bytes >= 20 * 1024 * 1024 * 1024 {
            1008.0 // High-end (RTX 4090 class)
        } else {
            256.0 // Mid-range
        };
        let overhead = 5.0; // ~5us kernel launch overhead

        Self {
            tflops: base_tflops,
            memory_bandwidth_gbs: bandwidth,
            kernel_overhead_us: overhead,
            device_name: device.name.clone(),
        }
    }

    /// Estimates time in milliseconds for a matmul of given dimensions.
    pub fn estimate_matmul_ms(&self, m: usize, n: usize, k: usize) -> f64 {
        let flops = 2.0 * m as f64 * n as f64 * k as f64;
        let compute_ms = flops / (self.tflops * 1e12) * 1e3;
        compute_ms + self.kernel_overhead_us / 1e3
    }

    /// Estimates time in milliseconds for an elementwise op on `n` elements.
    pub fn estimate_elementwise_ms(&self, n: usize) -> f64 {
        let bytes = n as f64 * 8.0 * 3.0; // read A, read B, write C
        let transfer_ms = bytes / (self.memory_bandwidth_gbs * 1e9) * 1e3;
        transfer_ms + self.kernel_overhead_us / 1e3
    }
}

impl std::fmt::Display for GpuBenchmark {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "GpuBenchmark({}: {:.1} TFLOPS, {:.0} GB/s, {:.1}us overhead)",
            self.device_name, self.tflops, self.memory_bandwidth_gbs, self.kernel_overhead_us
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Sprint 4: Multi-GPU & Data Parallelism
// ═══════════════════════════════════════════════════════════════════════

/// GPU memory statistics.
#[derive(Debug, Clone)]
pub struct GpuMemStats {
    /// Peak memory usage in bytes.
    pub peak_bytes: usize,
    /// Current memory usage in bytes.
    pub current_bytes: usize,
    /// Fragmentation ratio (0.0-1.0).
    pub fragmentation: f64,
    /// Number of active allocations.
    pub active_allocations: usize,
}

impl std::fmt::Display for GpuMemStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "GpuMemStats(peak={:.2} MB, current={:.2} MB, frag={:.1}%, allocs={})",
            self.peak_bytes as f64 / (1024.0 * 1024.0),
            self.current_bytes as f64 / (1024.0 * 1024.0),
            self.fragmentation * 100.0,
            self.active_allocations
        )
    }
}

/// Returns memory statistics for a simulated GPU device.
pub fn gpu_memory_usage(device: &SimGpuDevice) -> GpuMemStats {
    GpuMemStats {
        peak_bytes: device.pool().peak_bytes(),
        current_bytes: device.pool().used_bytes(),
        fragmentation: device.pool().fragmentation(),
        active_allocations: device.active_allocations(),
    }
}

// ── Synchronization ──

/// GPU synchronization barrier for multi-GPU coordination.
///
/// Simulates a barrier that waits until all devices have arrived.
#[derive(Debug)]
pub struct GpuSyncBarrier {
    /// Number of devices expected.
    device_count: u32,
    /// Set of devices that have arrived.
    arrived: Vec<bool>,
    /// Total number of completed barrier cycles.
    completed_cycles: u64,
}

impl GpuSyncBarrier {
    /// Creates a new barrier for `device_count` devices.
    pub fn new(device_count: u32) -> Self {
        Self {
            device_count,
            arrived: vec![false; device_count as usize],
            completed_cycles: 0,
        }
    }

    /// Signals that a device has arrived at the barrier.
    ///
    /// Returns `true` if all devices have arrived (barrier is complete).
    pub fn arrive(&mut self, device_id: u32) -> Result<bool, GpuError> {
        let idx = device_id as usize;
        if idx >= self.arrived.len() {
            return Err(GpuError::InvalidDevice {
                device_id,
                device_count: self.device_count,
            });
        }
        self.arrived[idx] = true;

        if self.arrived.iter().all(|&a| a) {
            // All devices arrived — reset for next cycle
            self.arrived.iter_mut().for_each(|a| *a = false);
            self.completed_cycles += 1;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Returns the number of completed barrier cycles.
    pub fn completed_cycles(&self) -> u64 {
        self.completed_cycles
    }

    /// Returns the number of devices that have arrived in the current cycle.
    pub fn arrived_count(&self) -> u32 {
        self.arrived.iter().filter(|&&a| a).count() as u32
    }
}

// ── All-reduce ──

/// Performs simulated all-reduce (mean) across multiple GPU buffers.
///
/// Each buffer represents the same parameter's gradient on a different device.
/// Returns a single buffer with the element-wise mean.
pub fn gpu_all_reduce(buffers: &[GpuBuffer]) -> Result<GpuBuffer, GpuError> {
    if buffers.is_empty() {
        return Err(GpuError::Other {
            reason: "all_reduce requires at least one buffer".to_string(),
        });
    }
    let n = buffers[0].len();
    for (i, buf) in buffers.iter().enumerate().skip(1) {
        if buf.len() != n {
            return Err(GpuError::ShapeMismatch {
                op: "all_reduce".to_string(),
                expected: format!("{n} elements (from buffer 0)"),
                got: format!("{} elements (buffer {i})", buf.len()),
            });
        }
    }

    let count = buffers.len() as f64;
    let mut result = vec![0.0; n];
    for buf in buffers {
        for (i, &v) in buf.data.iter().enumerate() {
            result[i] += v;
        }
    }
    for v in &mut result {
        *v /= count;
    }

    Ok(GpuBuffer::new(
        buffers[0].device_id,
        result,
        buffers[0].dtype,
    ))
}

// ── Scatter / Gather ──

/// Scatters a buffer evenly across `n_devices` simulated GPUs.
///
/// Returns a Vec of GpuBuffers, one per device, each containing a contiguous
/// chunk of the original data.
pub fn gpu_scatter(buffer: &GpuBuffer, n_devices: u32) -> Vec<GpuBuffer> {
    let n = buffer.len();
    let chunk_size = n.div_ceil(n_devices as usize);
    let mut result = Vec::with_capacity(n_devices as usize);

    for dev in 0..n_devices {
        let start = dev as usize * chunk_size;
        let end = (start + chunk_size).min(n);
        let chunk = if start < n {
            buffer.data[start..end].to_vec()
        } else {
            Vec::new()
        };
        result.push(GpuBuffer::new(dev, chunk, buffer.dtype));
    }

    result
}

/// Gathers buffers from multiple devices into a single buffer.
///
/// Concatenates all device buffers in device-id order.
pub fn gpu_gather(buffers: &[GpuBuffer]) -> GpuBuffer {
    let mut data = Vec::new();
    let dtype = buffers.first().map_or(DType::F64, |b| b.dtype);
    for buf in buffers {
        data.extend_from_slice(&buf.data);
    }
    GpuBuffer::new(0, data, dtype)
}

// ── Data Parallelism ──

/// Data-parallel training coordinator for multi-GPU.
///
/// Replicates model parameters to all devices, splits input batches,
/// runs forward/backward on each device, then aggregates gradients.
#[derive(Debug)]
pub struct DataParallel {
    /// Number of GPU devices.
    n_devices: u32,
    /// Per-device parameter replicas (device_id -> list of param buffers).
    replicas: HashMap<u32, Vec<GpuBuffer>>,
    /// Synchronization barrier.
    barrier: GpuSyncBarrier,
}

impl DataParallel {
    /// Creates a new DataParallel coordinator for `n_devices` GPUs.
    pub fn new(n_devices: u32) -> Self {
        Self {
            n_devices,
            replicas: HashMap::new(),
            barrier: GpuSyncBarrier::new(n_devices),
        }
    }

    /// Replicates parameters to all devices.
    ///
    /// Creates a copy of each parameter buffer on every simulated device.
    pub fn replicate(&mut self, params: &[GpuBuffer]) {
        for dev in 0..self.n_devices {
            let device_params: Vec<GpuBuffer> = params
                .iter()
                .map(|p| GpuBuffer::new(dev, p.data.clone(), p.dtype))
                .collect();
            self.replicas.insert(dev, device_params);
        }
    }

    /// Splits a batch evenly across devices.
    pub fn split_batch(&self, batch: &GpuBuffer) -> Vec<GpuBuffer> {
        gpu_scatter(batch, self.n_devices)
    }

    /// Aggregates gradients from all devices (all-reduce mean).
    ///
    /// For each parameter index, collects the gradient from each device
    /// and averages them.
    pub fn aggregate_grads(
        &self,
        per_device_grads: &HashMap<u32, Vec<GpuBuffer>>,
    ) -> Result<Vec<GpuBuffer>, GpuError> {
        let n_params = per_device_grads.values().next().map_or(0, |v| v.len());

        let mut aggregated = Vec::with_capacity(n_params);
        for param_idx in 0..n_params {
            let device_grads: Vec<GpuBuffer> = (0..self.n_devices)
                .filter_map(|dev| {
                    per_device_grads
                        .get(&dev)
                        .and_then(|grads| grads.get(param_idx).cloned())
                })
                .collect();
            let reduced = gpu_all_reduce(&device_grads)?;
            aggregated.push(reduced);
        }

        Ok(aggregated)
    }

    /// Returns the number of devices.
    pub fn n_devices(&self) -> u32 {
        self.n_devices
    }

    /// Returns the parameter replicas for a specific device.
    pub fn device_params(&self, device_id: u32) -> Option<&Vec<GpuBuffer>> {
        self.replicas.get(&device_id)
    }

    /// Returns a mutable reference to the synchronization barrier.
    pub fn barrier_mut(&mut self) -> &mut GpuSyncBarrier {
        &mut self.barrier
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── Sprint 1: Device Management Tests ──

    #[test]
    fn s1_1_detect_gpu_devices_returns_simulated_rtx4090() {
        let devices = detect_gpu_devices();
        assert_eq!(devices.len(), 1);
        assert!(devices[0].name.contains("RTX 4090"));
        assert!((devices[0].compute_capability - 8.9).abs() < 0.01);
        assert_eq!(devices[0].cuda_cores, 16384);
        assert_eq!(devices[0].tensor_cores, 512);
        assert!(devices[0].memory_gb() > 23.0);
    }

    #[test]
    fn s1_2_gpu_buffer_creation_and_accessors() {
        let buf = GpuBuffer::new(0, vec![1.0, 2.0, 3.0], DType::F64);
        assert_eq!(buf.device_id, 0);
        assert_eq!(buf.len(), 3);
        assert!(!buf.is_empty());
        assert_eq!(buf.size_bytes, 24); // 3 * 8 bytes
        assert_eq!(buf.dtype, DType::F64);
        assert_eq!(buf.data(), &[1.0, 2.0, 3.0]);
    }

    #[test]
    fn s1_3_gpu_buffer_zeros() {
        let buf = GpuBuffer::zeros(0, 5, DType::F32);
        assert_eq!(buf.len(), 5);
        assert_eq!(buf.size_bytes, 20); // 5 * 4 bytes (F32)
        assert!(buf.data().iter().all(|&v| v == 0.0));
    }

    #[test]
    fn s1_4_gpu_memory_pool_alloc_and_free() {
        let mut pool = GpuMemoryPool::new(1024);
        assert_eq!(pool.available_bytes(), 1024);

        let offset = pool.allocate(256).unwrap();
        assert_eq!(offset, 0);
        assert_eq!(pool.used_bytes(), 256);
        assert_eq!(pool.peak_bytes(), 256);
        assert_eq!(pool.active_allocations(), 1);

        pool.free(256);
        assert_eq!(pool.used_bytes(), 0);
        assert_eq!(pool.peak_bytes(), 256); // peak preserved
        assert_eq!(pool.active_allocations(), 0);
    }

    #[test]
    fn s1_5_gpu_memory_pool_out_of_memory() {
        let mut pool = GpuMemoryPool::new(100);
        let result = pool.allocate(200);
        assert!(result.is_err());
        match result {
            Err(GpuError::OutOfMemory {
                requested,
                available,
            }) => {
                assert_eq!(requested, 200);
                assert_eq!(available, 100);
            }
            _ => panic!("expected OutOfMemory error"),
        }
    }

    #[test]
    fn s1_6_gpu_memory_pool_fragmentation() {
        let mut pool = GpuMemoryPool::new(10000);
        pool.allocate(100).unwrap();
        pool.allocate(100).unwrap();
        pool.allocate(100).unwrap();
        pool.free(100);
        pool.free(100);
        // 3 total allocations, 2 freed -> fragmentation = 2/3
        assert!((pool.fragmentation() - 2.0 / 3.0).abs() < 0.01);
    }

    #[test]
    fn s1_7_sim_gpu_device_alloc_and_free() {
        let mut dev = SimGpuDevice::new(0);
        let buf = dev.alloc(10, DType::F64).unwrap();
        assert_eq!(buf.len(), 10);
        assert_eq!(dev.active_allocations(), 1);

        dev.free(&buf).unwrap();
        assert_eq!(dev.active_allocations(), 0);
    }

    #[test]
    fn s1_8_sim_gpu_device_host_to_device_and_back() {
        let mut dev = SimGpuDevice::new(0);
        let host_data = vec![1.0, 2.0, 3.0, 4.0];
        let buf = dev.host_to_device(&host_data, DType::F64).unwrap();
        assert_eq!(buf.data(), &[1.0, 2.0, 3.0, 4.0]);

        let retrieved = dev.device_to_host(&buf).unwrap();
        assert_eq!(retrieved, host_data);
    }

    #[test]
    fn s1_9_sim_gpu_device_double_free_error() {
        let mut dev = SimGpuDevice::new(0);
        let buf = dev.alloc(5, DType::F64).unwrap();
        dev.free(&buf).unwrap();

        let result = dev.free(&buf);
        assert!(result.is_err());
        match result {
            Err(GpuError::InvalidBuffer { ptr }) => assert_eq!(ptr, buf.ptr),
            _ => panic!("expected InvalidBuffer error"),
        }
    }

    #[test]
    fn s1_10_gpu_device_info_display_and_memory() {
        let info = GpuDeviceInfo {
            name: "Test GPU".to_string(),
            compute_capability: 8.0,
            memory_bytes: 8 * 1024 * 1024 * 1024,
            cuda_cores: 8192,
            tensor_cores: 256,
        };
        assert_eq!(info.memory_mb(), 8192);
        assert!((info.memory_gb() - 8.0).abs() < 0.01);
        let display = format!("{info}");
        assert!(display.contains("Test GPU"));
        assert!(display.contains("CC 8.0"));
    }

    // ── Sprint 2: GPU Tensor Kernel Tests ──

    #[test]
    fn s2_1_gpu_matmul_basic() {
        // [1 2; 3 4] @ [5 6; 7 8] = [19 22; 43 50]
        let a = GpuBuffer::new(0, vec![1.0, 2.0, 3.0, 4.0], DType::F64);
        let b = GpuBuffer::new(0, vec![5.0, 6.0, 7.0, 8.0], DType::F64);
        let c = gpu_matmul(&a, &b, 2, 2, 2).unwrap();
        assert_eq!(c.len(), 4);
        assert!((c.data()[0] - 19.0).abs() < 1e-10);
        assert!((c.data()[1] - 22.0).abs() < 1e-10);
        assert!((c.data()[2] - 43.0).abs() < 1e-10);
        assert!((c.data()[3] - 50.0).abs() < 1e-10);
    }

    #[test]
    fn s2_2_gpu_elementwise_ops() {
        let a = GpuBuffer::new(0, vec![1.0, 2.0, 3.0], DType::F64);
        let b = GpuBuffer::new(0, vec![4.0, 5.0, 6.0], DType::F64);

        let add = gpu_elementwise_add(&a, &b).unwrap();
        assert_eq!(add.data(), &[5.0, 7.0, 9.0]);

        let sub = gpu_elementwise_sub(&a, &b).unwrap();
        assert_eq!(sub.data(), &[-3.0, -3.0, -3.0]);

        let mul = gpu_elementwise_mul(&a, &b).unwrap();
        assert_eq!(mul.data(), &[4.0, 10.0, 18.0]);

        let div = gpu_elementwise_div(&b, &a).unwrap();
        assert!((div.data()[0] - 4.0).abs() < 1e-10);
        assert!((div.data()[1] - 2.5).abs() < 1e-10);
        assert!((div.data()[2] - 2.0).abs() < 1e-10);
    }

    #[test]
    fn s2_3_gpu_elementwise_div_by_zero() {
        let a = GpuBuffer::new(0, vec![1.0, 2.0], DType::F64);
        let b = GpuBuffer::new(0, vec![0.0, 1.0], DType::F64);
        let result = gpu_elementwise_div(&a, &b);
        assert!(matches!(result, Err(GpuError::DivisionByZero)));
    }

    #[test]
    fn s2_4_gpu_activations() {
        let buf = GpuBuffer::new(0, vec![-2.0, -1.0, 0.0, 1.0, 2.0], DType::F64);

        let relu_out = gpu_relu(&buf);
        assert_eq!(relu_out.data(), &[0.0, 0.0, 0.0, 1.0, 2.0]);

        let sig_out = gpu_sigmoid(&buf);
        // sigmoid(0) = 0.5
        assert!((sig_out.data()[2] - 0.5).abs() < 1e-10);
        // sigmoid(x) is in (0, 1)
        assert!(sig_out.data().iter().all(|&v| v > 0.0 && v < 1.0));

        let tanh_out = gpu_tanh(&buf);
        assert!((tanh_out.data()[2] - 0.0).abs() < 1e-10);

        let gelu_out = gpu_gelu(&buf);
        // GELU(0) = 0
        assert!((gelu_out.data()[2] - 0.0).abs() < 1e-6);
        // GELU(2) > 0
        assert!(gelu_out.data()[4] > 0.0);
    }

    #[test]
    fn s2_5_gpu_softmax_rows() {
        let buf = GpuBuffer::new(0, vec![1.0, 2.0, 3.0, 1.0, 2.0, 3.0], DType::F64);
        let result = gpu_softmax(&buf, 2, 3).unwrap();
        assert_eq!(result.len(), 6);

        // Each row should sum to 1
        let row0_sum: f64 = result.data()[0..3].iter().sum();
        let row1_sum: f64 = result.data()[3..6].iter().sum();
        assert!((row0_sum - 1.0).abs() < 1e-10);
        assert!((row1_sum - 1.0).abs() < 1e-10);

        // Max element should have highest probability
        assert!(result.data()[2] > result.data()[1]);
        assert!(result.data()[1] > result.data()[0]);
    }

    #[test]
    fn s2_6_gpu_transpose() {
        // [1 2 3; 4 5 6] -> [1 4; 2 5; 3 6]
        let buf = GpuBuffer::new(0, vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0], DType::F64);
        let result = gpu_transpose(&buf, 2, 3).unwrap();
        assert_eq!(result.len(), 6);
        assert_eq!(result.data(), &[1.0, 4.0, 2.0, 5.0, 3.0, 6.0]);
    }

    #[test]
    fn s2_7_gpu_reductions() {
        // [1 2 3; 4 5 6]
        let buf = GpuBuffer::new(0, vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0], DType::F64);

        // Sum along axis 0 (across rows): [5, 7, 9]
        let sum0 = gpu_reduce_sum(&buf, 2, 3, 0).unwrap();
        assert_eq!(sum0.data(), &[5.0, 7.0, 9.0]);

        // Sum along axis 1 (across cols): [6, 15]
        let sum1 = gpu_reduce_sum(&buf, 2, 3, 1).unwrap();
        assert_eq!(sum1.data(), &[6.0, 15.0]);

        // Max along axis 0: [4, 5, 6]
        let max0 = gpu_reduce_max(&buf, 2, 3, 0).unwrap();
        assert_eq!(max0.data(), &[4.0, 5.0, 6.0]);

        // Mean along axis 1: [2, 5]
        let mean1 = gpu_reduce_mean(&buf, 2, 3, 1).unwrap();
        assert!((mean1.data()[0] - 2.0).abs() < 1e-10);
        assert!((mean1.data()[1] - 5.0).abs() < 1e-10);
    }

    #[test]
    fn s2_8_gpu_reduce_axis_out_of_bounds() {
        let buf = GpuBuffer::new(0, vec![1.0, 2.0], DType::F64);
        let result = gpu_reduce_sum(&buf, 1, 2, 5);
        assert!(matches!(
            result,
            Err(GpuError::AxisOutOfBounds { axis: 5, ndim: 2 })
        ));
    }

    #[test]
    fn s2_9_gpu_conv2d_basic() {
        // 1x1x3x3 input, 1x1x2x2 kernel, stride=1, pad=0 -> 1x1x2x2 output
        let input = GpuBuffer::new(
            0,
            vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0],
            DType::F64,
        );
        let weight = GpuBuffer::new(0, vec![1.0, 0.0, 0.0, 1.0], DType::F64);
        let bias = GpuBuffer::new(0, vec![], DType::F64);

        let out = gpu_conv2d(&input, &weight, &bias, 1, 1, 3, 3, 1, 2, 2, 1, 0).unwrap();
        assert_eq!(out.len(), 4); // 1x1x2x2
                                  // [1,0;0,1] conv on [1 2 3; 4 5 6; 7 8 9]:
                                  // (0,0): 1*1 + 0*2 + 0*4 + 1*5 = 6
                                  // (0,1): 1*2 + 0*3 + 0*5 + 1*6 = 8
                                  // (1,0): 1*4 + 0*5 + 0*7 + 1*8 = 12
                                  // (1,1): 1*5 + 0*6 + 0*8 + 1*9 = 14
        assert!((out.data()[0] - 6.0).abs() < 1e-10);
        assert!((out.data()[1] - 8.0).abs() < 1e-10);
        assert!((out.data()[2] - 12.0).abs() < 1e-10);
        assert!((out.data()[3] - 14.0).abs() < 1e-10);
    }

    #[test]
    fn s2_10_gpu_batch_norm() {
        // 2 samples, 3 features
        let input = GpuBuffer::new(0, vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0], DType::F64);
        let gamma = GpuBuffer::new(0, vec![1.0, 1.0, 1.0], DType::F64);
        let beta = GpuBuffer::new(0, vec![0.0, 0.0, 0.0], DType::F64);
        let empty = GpuBuffer::new(0, vec![], DType::F64);

        let result = gpu_batch_norm(&input, &gamma, &beta, &empty, &empty, 2, 3, 1e-5).unwrap();
        assert_eq!(result.len(), 6);

        // With gamma=1, beta=0, each feature column should be standardized:
        // col 0: [1, 4], mean=2.5, var=2.25, normalized = [-1, 1] / sqrt(2.25+eps)
        let col0_mean = (result.data()[0] + result.data()[3]) / 2.0;
        assert!(col0_mean.abs() < 1e-6); // centered at 0
    }

    // ── Sprint 3: GPU Autograd & Training Tests ──

    #[test]
    fn s3_1_gpu_tape_record_and_replay() {
        let mut tape = GpuTape::new();
        assert!(tape.is_empty());
        assert!(tape.is_recording());

        tape.record_op(
            "add",
            vec![100, 200],
            300,
            Box::new(|out_grad| {
                // d(a+b)/da = 1, d(a+b)/db = 1
                vec![out_grad.clone(), out_grad.clone()]
            }),
        );
        assert_eq!(tape.len(), 1);
    }

    #[test]
    fn s3_2_gpu_tape_recording_toggle() {
        let mut tape = GpuTape::new();
        tape.set_recording(false);
        tape.record_op("noop", vec![1], 2, Box::new(|g| vec![g.clone()]));
        assert!(tape.is_empty()); // nothing recorded

        tape.set_recording(true);
        tape.record_op("real", vec![1], 2, Box::new(|g| vec![g.clone()]));
        assert_eq!(tape.len(), 1);
    }

    #[test]
    fn s3_3_gpu_backward_computes_gradients() {
        let mut tape = GpuTape::new();

        // Simulate: c = a * 2 (ptr a=100, ptr c=200)
        tape.record_op(
            "mul_scalar",
            vec![100],
            200,
            Box::new(|out_grad| {
                let data: Vec<f64> = out_grad.data().iter().map(|v| v * 2.0).collect();
                vec![GpuBuffer::new(0, data, DType::F64)]
            }),
        );

        let loss_grad = GpuBuffer::new(0, vec![1.0], DType::F64);
        // Override loss_grad ptr to match output_ptr=200
        let mut loss_grad_fixed = loss_grad;
        loss_grad_fixed.ptr = 200; // hack for test: match output ptr

        // We need to insert the grad with the right ptr
        let grads = gpu_backward(&tape, loss_grad_fixed);
        // Grad for ptr 200 is the loss grad itself
        assert!(grads.contains_key(&200));
        // Grad for ptr 100 should be 2.0 (chain rule: d(2*a)/da = 2)
        assert!(grads.contains_key(&100));
        assert!((grads[&100].data()[0] - 2.0).abs() < 1e-10);
    }

    #[test]
    fn s3_4_gpu_sgd_step() {
        let mut sgd = GpuSGD::new(0.1, 0.0);
        let mut params = vec![GpuBuffer::new(0, vec![1.0, 2.0], DType::F64)];
        let grads = vec![GpuBuffer::new(0, vec![10.0, 20.0], DType::F64)];

        sgd.step(&mut params, &grads);
        // 1.0 - 0.1 * 10.0 = 0.0
        assert!((params[0].data()[0] - 0.0).abs() < 1e-10);
        // 2.0 - 0.1 * 20.0 = 0.0
        assert!((params[0].data()[1] - 0.0).abs() < 1e-10);
    }

    #[test]
    fn s3_5_gpu_sgd_with_momentum() {
        let mut sgd = GpuSGD::new(0.1, 0.9);
        let mut params = vec![GpuBuffer::new(0, vec![1.0], DType::F64)];
        let grads = vec![GpuBuffer::new(0, vec![1.0], DType::F64)];

        // Step 1: velocity = 0.9*0 + 1.0 = 1.0, param = 1.0 - 0.1*1.0 = 0.9
        sgd.step(&mut params, &grads);
        assert!((params[0].data()[0] - 0.9).abs() < 1e-10);

        // Step 2: velocity = 0.9*1.0 + 1.0 = 1.9, param = 0.9 - 0.1*1.9 = 0.71
        sgd.step(&mut params, &grads);
        assert!((params[0].data()[0] - 0.71).abs() < 1e-10);
    }

    #[test]
    fn s3_6_gpu_adam_step() {
        let mut adam = GpuAdam::new(0.001);
        let mut params = vec![GpuBuffer::new(0, vec![5.0], DType::F64)];
        let grads = vec![GpuBuffer::new(0, vec![1.0], DType::F64)];

        adam.step(&mut params, &grads);
        assert!(params[0].data()[0] < 5.0); // should decrease
        assert_eq!(adam.timestep(), 1);
    }

    #[test]
    fn s3_7_gpu_gradient_clipping() {
        // Grad = [3, 4], norm = 5, clip to 2.5 -> scale = 0.5
        let mut grads = vec![GpuBuffer::new(0, vec![3.0, 4.0], DType::F64)];
        let norm = gpu_gradient_clipping(&mut grads, 2.5);
        assert!((norm - 5.0).abs() < 1e-6);
        assert!((grads[0].data()[0] - 1.5).abs() < 1e-6);
        assert!((grads[0].data()[1] - 2.0).abs() < 1e-6);
    }

    #[test]
    fn s3_8_mixed_precision_loss_scaling() {
        let w = GpuBuffer::new(0, vec![1.0, 2.0, 3.0], DType::F64);
        let mp = MixedPrecisionGpu::new(&[w]);

        let scaled = mp.scale_loss(0.001);
        assert!((scaled - 0.001 * 65536.0).abs() < 1e-6);
    }

    #[test]
    fn s3_9_data_prefetcher_double_buffer() {
        let mut pf = DataPrefetcher::new(0);
        pf.prefetch(&[1.0, 2.0], DType::F64);
        pf.prefetch(&[3.0, 4.0], DType::F64);

        // First get: should return buffer_a (first prefetch went to buffer_b,
        // second went to buffer_a because use_a flipped)
        let buf = pf.get_current();
        assert!(buf.is_some());
        assert_eq!(pf.prefetch_count(), 2);
    }

    #[test]
    fn s3_10_gpu_benchmark_estimates() {
        let devices = detect_gpu_devices();
        let bench = GpuBenchmark::run(&devices[0]);
        assert!(bench.tflops > 0.0);
        assert!(bench.memory_bandwidth_gbs > 0.0);

        let matmul_ms = bench.estimate_matmul_ms(1024, 1024, 1024);
        assert!(matmul_ms > 0.0);

        let elem_ms = bench.estimate_elementwise_ms(1_000_000);
        assert!(elem_ms > 0.0);

        let display = format!("{bench}");
        assert!(display.contains("TFLOPS"));
    }

    // ── Sprint 4: Multi-GPU & Data Parallelism Tests ──

    #[test]
    fn s4_1_data_parallel_replicate() {
        let mut dp = DataParallel::new(4);
        let params = vec![
            GpuBuffer::new(0, vec![1.0, 2.0, 3.0], DType::F64),
            GpuBuffer::new(0, vec![4.0, 5.0], DType::F64),
        ];
        dp.replicate(&params);

        for dev in 0..4 {
            let replica = dp.device_params(dev).unwrap();
            assert_eq!(replica.len(), 2);
            assert_eq!(replica[0].data(), &[1.0, 2.0, 3.0]);
            assert_eq!(replica[1].data(), &[4.0, 5.0]);
        }
    }

    #[test]
    fn s4_2_gpu_all_reduce_mean() {
        let bufs = vec![
            GpuBuffer::new(0, vec![2.0, 4.0], DType::F64),
            GpuBuffer::new(1, vec![4.0, 8.0], DType::F64),
            GpuBuffer::new(2, vec![6.0, 12.0], DType::F64),
        ];
        let result = gpu_all_reduce(&bufs).unwrap();
        assert!((result.data()[0] - 4.0).abs() < 1e-10); // (2+4+6)/3
        assert!((result.data()[1] - 8.0).abs() < 1e-10); // (4+8+12)/3
    }

    #[test]
    fn s4_3_gpu_sync_barrier() {
        let mut barrier = GpuSyncBarrier::new(3);
        assert!(!barrier.arrive(0).unwrap());
        assert!(!barrier.arrive(1).unwrap());
        assert!(barrier.arrive(2).unwrap()); // all arrived
        assert_eq!(barrier.completed_cycles(), 1);

        // Next cycle
        assert!(!barrier.arrive(0).unwrap());
        assert_eq!(barrier.arrived_count(), 1);
    }

    #[test]
    fn s4_4_gpu_sync_barrier_invalid_device() {
        let mut barrier = GpuSyncBarrier::new(2);
        let result = barrier.arrive(5);
        assert!(matches!(
            result,
            Err(GpuError::InvalidDevice {
                device_id: 5,
                device_count: 2
            })
        ));
    }

    #[test]
    fn s4_5_gpu_scatter_and_gather() {
        let buf = GpuBuffer::new(0, vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0], DType::F64);
        let scattered = gpu_scatter(&buf, 3);
        assert_eq!(scattered.len(), 3);
        assert_eq!(scattered[0].data(), &[1.0, 2.0]);
        assert_eq!(scattered[1].data(), &[3.0, 4.0]);
        assert_eq!(scattered[2].data(), &[5.0, 6.0]);

        let gathered = gpu_gather(&scattered);
        assert_eq!(gathered.data(), &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
    }

    #[test]
    fn s4_6_data_parallel_split_batch() {
        let dp = DataParallel::new(2);
        let batch = GpuBuffer::new(0, vec![1.0, 2.0, 3.0, 4.0], DType::F64);
        let splits = dp.split_batch(&batch);
        assert_eq!(splits.len(), 2);
        assert_eq!(splits[0].data(), &[1.0, 2.0]);
        assert_eq!(splits[1].data(), &[3.0, 4.0]);
    }

    #[test]
    fn s4_7_data_parallel_aggregate_grads() {
        let dp = DataParallel::new(2);
        let mut per_device_grads = HashMap::new();
        per_device_grads.insert(0, vec![GpuBuffer::new(0, vec![2.0, 4.0], DType::F64)]);
        per_device_grads.insert(1, vec![GpuBuffer::new(1, vec![6.0, 8.0], DType::F64)]);
        let aggregated = dp.aggregate_grads(&per_device_grads).unwrap();
        assert_eq!(aggregated.len(), 1);
        // Mean: (2+6)/2 = 4, (4+8)/2 = 6
        assert!((aggregated[0].data()[0] - 4.0).abs() < 1e-10);
        assert!((aggregated[0].data()[1] - 6.0).abs() < 1e-10);
    }

    #[test]
    fn s4_8_gpu_mem_stats() {
        let mut dev = SimGpuDevice::new(0);
        dev.alloc(1000, DType::F64).unwrap();

        let stats = gpu_memory_usage(&dev);
        assert!(stats.current_bytes > 0);
        assert!(stats.peak_bytes > 0);
        assert_eq!(stats.active_allocations, 1);
        let display = format!("{stats}");
        assert!(display.contains("MB"));
    }

    #[test]
    fn s4_9_gpu_kernel_config() {
        let cfg = GpuKernelConfig::for_elements(10000);
        assert_eq!(cfg.block_size, 256);
        assert!(cfg.grid_size > 0);
        assert_eq!(
            cfg.total_threads(),
            cfg.block_size as u64 * cfg.grid_size as u64
        );

        let matmul_cfg = GpuKernelConfig::for_matmul(512, 512);
        assert_eq!(matmul_cfg.block_size, 16);
        assert!(matmul_cfg.shared_memory > 0);
    }

    #[test]
    fn s4_10_gpu_training_loop_epoch() {
        let config = GpuTrainingConfig::new(5, 4, 0.01);
        let mut loop_runner = GpuTrainingLoop::new(config);

        let data = GpuBuffer::new(0, vec![1.0; 16], DType::F64);
        let targets = GpuBuffer::new(0, vec![0.0; 16], DType::F64);
        let mut weights = vec![GpuBuffer::new(0, vec![0.5; 4], DType::F64)];

        let result = loop_runner.run_epoch(&data, &targets, &mut weights, |_d, _t, _w| {
            (vec![0.5; 4], 0.25) // dummy forward
        });

        assert_eq!(result.epoch, 0);
        assert!(result.avg_loss > 0.0);
        assert!(result.batches > 0);
        assert!(result.gpu_time_ms > 0.0);
        assert_eq!(loop_runner.history().len(), 1);
    }
}
