//! Vulkan compute pipeline for Dragon Q6A Adreno 643 GPU.
//!
//! Uses Mesa Turnip driver (Vulkan 1.3) for GPU-accelerated tensor operations.
//! Provides a direct Vulkan compute pipeline optimized for the Adreno 643,
//! with SPIR-V compute shaders for element-wise ops, matrix multiply,
//! and activation functions.
//!
//! # Architecture
//!
//! ```text
//! VulkanCompute
//!   ├── ash::Entry (dynamic Vulkan loader)
//!   ├── ash::Instance
//!   ├── ash::Device (logical device)
//!   ├── vk::Queue (compute queue)
//!   ├── vk::CommandPool
//!   ├── ComputeKernel (cached pipelines per operation)
//!   │    ├── vk::Pipeline
//!   │    ├── vk::PipelineLayout
//!   │    └── vk::DescriptorSetLayout
//!   └── VulkanBuffer (device memory with upload/download)
//! ```
//!
//! # Usage
//!
//! ```rust,no_run
//! use fajar_lang::bsp::dragon_q6a::vulkan::VulkanCompute;
//!
//! let vk = VulkanCompute::new().expect("Vulkan init failed");
//! println!("GPU: {}", vk.device_name());
//! ```

use ash::vk;
use std::collections::HashMap;
use std::ffi::CString;
use std::sync::Mutex;
use thiserror::Error;

// ═══════════════════════════════════════════════════════════════════════
// Error Types
// ═══════════════════════════════════════════════════════════════════════

/// Vulkan compute error codes.
#[derive(Debug, Error)]
pub enum VulkanError {
    /// VE001 — Vulkan library not loadable.
    #[error("VE001: Vulkan not available: {0}")]
    NotAvailable(String),

    /// VE002 — No compute-capable GPU found.
    #[error("VE002: no compute-capable GPU found")]
    NoComputeDevice,

    /// VE003 — Shader module creation failed.
    #[error("VE003: shader error: {0}")]
    ShaderError(String),

    /// VE004 — Buffer allocation failed.
    #[error("VE004: buffer error: {0}")]
    BufferError(String),

    /// VE005 — Compute dispatch failed.
    #[error("VE005: dispatch error: {0}")]
    DispatchError(String),

    /// VE006 — No suitable memory type found.
    #[error("VE006: no suitable memory type")]
    NoSuitableMemory,

    /// VE007 — Tensor shape mismatch.
    #[error("VE007: shape mismatch: {0}")]
    ShapeMismatch(String),
}

// ═══════════════════════════════════════════════════════════════════════
// Device Info
// ═══════════════════════════════════════════════════════════════════════

/// Vulkan device information queried from the hardware.
#[derive(Debug, Clone)]
pub struct VulkanDeviceInfo {
    /// Device name (e.g., "Turnip Adreno (TM) 643").
    pub name: String,
    /// Vulkan API version string.
    pub api_version: String,
    /// Driver version string.
    pub driver_version: String,
    /// Device type (Integrated, Discrete, CPU, etc.).
    pub device_type: String,
    /// Maximum compute workgroup count per dimension.
    pub max_workgroup_count: [u32; 3],
    /// Maximum compute workgroup size per dimension.
    pub max_workgroup_size: [u32; 3],
    /// Maximum compute shared memory in bytes.
    pub max_shared_memory: u32,
    /// Subgroup size (wavefront/warp size).
    pub subgroup_size: u32,
}

impl std::fmt::Display for VulkanDeviceInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} (Vulkan {}, {}, subgroup={})",
            self.name, self.api_version, self.device_type, self.subgroup_size,
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Compute Kernel Types
// ═══════════════════════════════════════════════════════════════════════

/// Supported compute kernel operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KernelOp {
    /// Element-wise vector addition: c[i] = a[i] + b[i].
    VectorAdd,
    /// Element-wise vector multiplication: c[i] = a[i] * b[i].
    VectorMul,
    /// Element-wise vector subtraction: c[i] = a[i] - b[i].
    VectorSub,
    /// ReLU activation: y[i] = max(0, x[i]).
    Relu,
    /// Sigmoid activation: y[i] = 1 / (1 + exp(-x[i])).
    Sigmoid,
    /// Matrix multiplication: C = A * B.
    Matmul,
}

impl std::fmt::Display for KernelOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KernelOp::VectorAdd => write!(f, "vector_add"),
            KernelOp::VectorMul => write!(f, "vector_mul"),
            KernelOp::VectorSub => write!(f, "vector_sub"),
            KernelOp::Relu => write!(f, "relu"),
            KernelOp::Sigmoid => write!(f, "sigmoid"),
            KernelOp::Matmul => write!(f, "matmul"),
        }
    }
}

/// A cached, compiled compute pipeline.
struct ComputeKernel {
    pipeline: vk::Pipeline,
    pipeline_layout: vk::PipelineLayout,
    descriptor_set_layout: vk::DescriptorSetLayout,
    _num_buffers: u32,
}

// ═══════════════════════════════════════════════════════════════════════
// Vulkan Buffer
// ═══════════════════════════════════════════════════════════════════════

/// A Vulkan device buffer for tensor data.
pub struct VulkanBuffer {
    buffer: vk::Buffer,
    memory: vk::DeviceMemory,
    size: vk::DeviceSize,
}

impl VulkanBuffer {
    /// Buffer size in bytes.
    pub fn size(&self) -> vk::DeviceSize {
        self.size
    }
}

// ═══════════════════════════════════════════════════════════════════════
// VulkanCompute — Main Compute Pipeline
// ═══════════════════════════════════════════════════════════════════════

/// Vulkan compute pipeline for GPU-accelerated tensor operations.
///
/// Manages a Vulkan device, command pool, and cached compute pipelines
/// for common tensor operations (add, mul, relu, sigmoid, matmul).
pub struct VulkanCompute {
    _entry: ash::Entry,
    instance: ash::Instance,
    physical_device: vk::PhysicalDevice,
    device: ash::Device,
    compute_queue: vk::Queue,
    _compute_queue_family: u32,
    command_pool: vk::CommandPool,
    descriptor_pool: vk::DescriptorPool,
    info: VulkanDeviceInfo,
    kernels: Mutex<HashMap<KernelOp, ComputeKernel>>,
}

impl VulkanCompute {
    /// Initialize Vulkan compute pipeline.
    ///
    /// Loads the Vulkan library, creates instance/device/queue,
    /// and pre-compiles compute pipelines for all supported operations.
    pub fn new() -> Result<Self, VulkanError> {
        // Load Vulkan dynamically
        // SAFETY: Vulkan entry point is loaded dynamically; no preconditions required.
        let entry = unsafe { ash::Entry::load() }
            .map_err(|e| VulkanError::NotAvailable(format!("failed to load libvulkan: {e}")))?;

        // Create instance
        let app_name = CString::new("FajarLang").unwrap();
        let engine_name = CString::new("FajarCompute").unwrap();
        let app_info = vk::ApplicationInfo::default()
            .application_name(&app_name)
            .application_version(vk::make_api_version(0, 3, 0, 0))
            .engine_name(&engine_name)
            .engine_version(vk::make_api_version(0, 1, 0, 0))
            .api_version(vk::make_api_version(0, 1, 3, 0));

        let create_info = vk::InstanceCreateInfo::default().application_info(&app_info);

        // SAFETY: entry is valid; create_info and app_info are well-formed per Vulkan spec.
        let instance = unsafe { entry.create_instance(&create_info, None) }
            .map_err(|e| VulkanError::NotAvailable(format!("vkCreateInstance failed: {e}")))?;

        // Select physical device (prefer integrated/discrete GPU over CPU)
        // SAFETY: instance is valid; Vulkan spec guarantees safe enumeration.
        let physical_devices = unsafe { instance.enumerate_physical_devices() }
            .map_err(|e| VulkanError::NotAvailable(format!("enumerate devices failed: {e}")))?;

        if physical_devices.is_empty() {
            return Err(VulkanError::NoComputeDevice);
        }

        // Prefer discrete GPU (NVIDIA/AMD) over integrated (Intel)
        let mut selected = None;
        let mut selected_priority = 0u32;
        for &pd in &physical_devices {
            // SAFETY: instance is valid; pd was returned by enumerate_physical_devices.
            let props = unsafe { instance.get_physical_device_properties(pd) };
            let priority = match props.device_type {
                vk::PhysicalDeviceType::DISCRETE_GPU => 4,
                vk::PhysicalDeviceType::VIRTUAL_GPU => 3,
                vk::PhysicalDeviceType::INTEGRATED_GPU => 2,
                vk::PhysicalDeviceType::CPU => 1,
                _ => 0,
            };
            if priority > selected_priority {
                selected = Some(pd);
                selected_priority = priority;
            }
        }

        let physical_device = selected.ok_or(VulkanError::NoComputeDevice)?;
        // SAFETY: instance and physical_device are valid; returned by enumerate above.
        let props = unsafe { instance.get_physical_device_properties(physical_device) };
        // SAFETY: instance and physical_device are valid; querying memory properties is safe.
        let mem_props = unsafe { instance.get_physical_device_memory_properties(physical_device) };

        // Find compute queue family
        // SAFETY: instance and physical_device are valid; querying queue families is safe.
        let queue_families =
            unsafe { instance.get_physical_device_queue_family_properties(physical_device) };

        let compute_family = queue_families
            .iter()
            .enumerate()
            .find(|(_, qf)| qf.queue_flags.contains(vk::QueueFlags::COMPUTE))
            .map(|(i, _)| i as u32)
            .ok_or(VulkanError::NoComputeDevice)?;

        // Create logical device with compute queue
        let queue_priority = [1.0_f32];
        let queue_create_info = vk::DeviceQueueCreateInfo::default()
            .queue_family_index(compute_family)
            .queue_priorities(&queue_priority);
        let queue_create_infos = [queue_create_info];

        let device_create_info =
            vk::DeviceCreateInfo::default().queue_create_infos(&queue_create_infos);

        // SAFETY: instance and physical_device are valid; device_create_info matches Vulkan spec.
        let device = unsafe { instance.create_device(physical_device, &device_create_info, None) }
            .map_err(|e| VulkanError::NotAvailable(format!("vkCreateDevice failed: {e}")))?;

        // SAFETY: device is valid; compute_family was verified to exist with COMPUTE capability.
        let compute_queue = unsafe { device.get_device_queue(compute_family, 0) };

        // Create command pool
        let pool_info = vk::CommandPoolCreateInfo::default()
            .queue_family_index(compute_family)
            .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER);

        // SAFETY: device is valid; pool_info references a valid queue family index.
        let command_pool = unsafe { device.create_command_pool(&pool_info, None) }
            .map_err(|e| VulkanError::DispatchError(format!("create command pool: {e}")))?;

        // Create descriptor pool (enough for all kernel types)
        let pool_sizes = [vk::DescriptorPoolSize {
            ty: vk::DescriptorType::STORAGE_BUFFER,
            descriptor_count: 64, // enough for many dispatches
        }];
        let dp_info = vk::DescriptorPoolCreateInfo::default()
            .max_sets(32)
            .pool_sizes(&pool_sizes)
            .flags(vk::DescriptorPoolCreateFlags::FREE_DESCRIPTOR_SET);

        // SAFETY: device is valid; dp_info parameters are within Vulkan spec limits.
        let descriptor_pool = unsafe { device.create_descriptor_pool(&dp_info, None) }
            .map_err(|e| VulkanError::DispatchError(format!("create descriptor pool: {e}")))?;

        // Build device info
        // SAFETY: props.device_name is a null-terminated C string from Vulkan driver.
        let device_name = unsafe {
            std::ffi::CStr::from_ptr(props.device_name.as_ptr())
                .to_string_lossy()
                .into_owned()
        };
        let api_ver = format!(
            "{}.{}.{}",
            vk::api_version_major(props.api_version),
            vk::api_version_minor(props.api_version),
            vk::api_version_patch(props.api_version),
        );
        let driver_ver = format!(
            "{}.{}.{}",
            vk::api_version_major(props.driver_version),
            vk::api_version_minor(props.driver_version),
            vk::api_version_patch(props.driver_version),
        );
        let device_type = match props.device_type {
            vk::PhysicalDeviceType::INTEGRATED_GPU => "Integrated GPU",
            vk::PhysicalDeviceType::DISCRETE_GPU => "Discrete GPU",
            vk::PhysicalDeviceType::VIRTUAL_GPU => "Virtual GPU",
            vk::PhysicalDeviceType::CPU => "CPU",
            _ => "Other",
        }
        .to_string();

        let limits = &props.limits;
        let info = VulkanDeviceInfo {
            name: device_name,
            api_version: api_ver,
            driver_version: driver_ver,
            device_type,
            max_workgroup_count: limits.max_compute_work_group_count,
            max_workgroup_size: limits.max_compute_work_group_size,
            max_shared_memory: limits.max_compute_shared_memory_size,
            subgroup_size: 128, // Adreno 643 default; could query VK_EXT_subgroup_size_control
        };

        let compute = Self {
            _entry: entry,
            instance,
            physical_device,
            device,
            compute_queue,
            _compute_queue_family: compute_family,
            command_pool,
            descriptor_pool,
            info,
            kernels: Mutex::new(HashMap::new()),
        };

        // Kernels are compiled lazily on first use.
        // This avoids segfaults on drivers that strictly validate SPIR-V
        // at vkCreateShaderModule/vkCreateComputePipelines time.
        let _ = mem_props;

        Ok(compute)
    }

    /// Device name string.
    pub fn device_name(&self) -> &str {
        &self.info.name
    }

    /// Full device info.
    pub fn device_info(&self) -> &VulkanDeviceInfo {
        &self.info
    }

    /// Check if Vulkan compute is available on this system.
    pub fn is_available() -> bool {
        // SAFETY: Vulkan entry point is loaded dynamically; no preconditions required.
        unsafe { ash::Entry::load() }.is_ok()
    }

    // ═══════════════════════════════════════════════════════════════════
    // Buffer Management (Task 17.5)
    // ═══════════════════════════════════════════════════════════════════

    /// Create a device buffer with host-visible memory.
    pub fn create_buffer(&self, size: usize) -> Result<VulkanBuffer, VulkanError> {
        let buffer_info = vk::BufferCreateInfo::default()
            .size(size as vk::DeviceSize)
            .usage(vk::BufferUsageFlags::STORAGE_BUFFER)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);

        // SAFETY: device is valid; buffer_info specifies valid usage flags and size.
        let buffer = unsafe { self.device.create_buffer(&buffer_info, None) }
            .map_err(|e| VulkanError::BufferError(format!("create buffer: {e}")))?;

        // SAFETY: device and buffer are valid; buffer was just created above.
        let mem_reqs = unsafe { self.device.get_buffer_memory_requirements(buffer) };

        let mem_type_index = self.find_memory_type(
            mem_reqs.memory_type_bits,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        )?;

        let alloc_info = vk::MemoryAllocateInfo::default()
            .allocation_size(mem_reqs.size)
            .memory_type_index(mem_type_index);

        // SAFETY: device is valid; alloc_info size matches memory requirements; type index is valid.
        let memory = unsafe { self.device.allocate_memory(&alloc_info, None) }
            .map_err(|e| VulkanError::BufferError(format!("allocate memory: {e}")))?;

        // SAFETY: buffer and memory are valid; offset 0 is within allocation; memory type is compatible.
        unsafe { self.device.bind_buffer_memory(buffer, memory, 0) }
            .map_err(|e| VulkanError::BufferError(format!("bind buffer memory: {e}")))?;

        Ok(VulkanBuffer {
            buffer,
            memory,
            size: size as vk::DeviceSize,
        })
    }

    /// Upload f32 data to a Vulkan buffer.
    pub fn upload_f32(&self, buffer: &VulkanBuffer, data: &[f32]) -> Result<(), VulkanError> {
        let byte_size = (std::mem::size_of_val(data)) as vk::DeviceSize;
        if byte_size > buffer.size {
            return Err(VulkanError::BufferError(format!(
                "data size {} exceeds buffer size {}",
                byte_size, buffer.size
            )));
        }

        // SAFETY: buffer memory is host-visible; byte_size verified <= buffer.size;
        // mapped pointer is valid for byte_size bytes; unmap balances the map call.
        unsafe {
            let ptr = self
                .device
                .map_memory(buffer.memory, 0, byte_size, vk::MemoryMapFlags::empty())
                .map_err(|e| VulkanError::BufferError(format!("map memory: {e}")))?;

            std::ptr::copy_nonoverlapping(
                data.as_ptr() as *const u8,
                ptr as *mut u8,
                byte_size as usize,
            );

            self.device.unmap_memory(buffer.memory);
        }
        Ok(())
    }

    /// Download f32 data from a Vulkan buffer.
    pub fn download_f32(
        &self,
        buffer: &VulkanBuffer,
        count: usize,
    ) -> Result<Vec<f32>, VulkanError> {
        let byte_size = (count * std::mem::size_of::<f32>()) as vk::DeviceSize;
        if byte_size > buffer.size {
            return Err(VulkanError::BufferError(format!(
                "requested {} bytes exceeds buffer size {}",
                byte_size, buffer.size
            )));
        }

        let mut result = vec![0.0f32; count];
        // SAFETY: buffer memory is host-visible and coherent; byte_size verified <= buffer.size;
        // mapped pointer is valid for byte_size bytes; result vec has sufficient capacity.
        unsafe {
            let ptr = self
                .device
                .map_memory(buffer.memory, 0, byte_size, vk::MemoryMapFlags::empty())
                .map_err(|e| VulkanError::BufferError(format!("map memory: {e}")))?;

            std::ptr::copy_nonoverlapping(
                ptr as *const u8,
                result.as_mut_ptr() as *mut u8,
                byte_size as usize,
            );

            self.device.unmap_memory(buffer.memory);
        }
        Ok(result)
    }

    /// Destroy a buffer and free its memory.
    pub fn destroy_buffer(&self, buffer: VulkanBuffer) {
        // SAFETY: device is valid; buffer and memory are valid and not in use (caller
        // ensures no pending GPU work references them); each is destroyed exactly once.
        unsafe {
            self.device.destroy_buffer(buffer.buffer, None);
            self.device.free_memory(buffer.memory, None);
        }
    }

    // ═══════════════════════════════════════════════════════════════════
    // Tensor Operations (high-level API)
    // ═══════════════════════════════════════════════════════════════════

    /// Element-wise addition: result[i] = a[i] + b[i].
    pub fn tensor_add(&self, a: &[f32], b: &[f32]) -> Result<Vec<f32>, VulkanError> {
        if a.len() != b.len() {
            return Err(VulkanError::ShapeMismatch(format!(
                "add: a.len()={} != b.len()={}",
                a.len(),
                b.len()
            )));
        }
        self.dispatch_binary(KernelOp::VectorAdd, a, b)
    }

    /// Element-wise multiplication: result[i] = a[i] * b[i].
    pub fn tensor_mul(&self, a: &[f32], b: &[f32]) -> Result<Vec<f32>, VulkanError> {
        if a.len() != b.len() {
            return Err(VulkanError::ShapeMismatch(format!(
                "mul: a.len()={} != b.len()={}",
                a.len(),
                b.len()
            )));
        }
        self.dispatch_binary(KernelOp::VectorMul, a, b)
    }

    /// Element-wise subtraction: result[i] = a[i] - b[i].
    pub fn tensor_sub(&self, a: &[f32], b: &[f32]) -> Result<Vec<f32>, VulkanError> {
        if a.len() != b.len() {
            return Err(VulkanError::ShapeMismatch(format!(
                "sub: a.len()={} != b.len()={}",
                a.len(),
                b.len()
            )));
        }
        self.dispatch_binary(KernelOp::VectorSub, a, b)
    }

    /// ReLU activation: result[i] = max(0, x[i]).
    pub fn tensor_relu(&self, x: &[f32]) -> Result<Vec<f32>, VulkanError> {
        self.dispatch_unary(KernelOp::Relu, x)
    }

    /// Sigmoid activation: result[i] = 1 / (1 + exp(-x[i])).
    pub fn tensor_sigmoid(&self, x: &[f32]) -> Result<Vec<f32>, VulkanError> {
        self.dispatch_unary(KernelOp::Sigmoid, x)
    }

    /// Matrix multiplication: C[m,n] = sum_k(A[m,k] * B[k,n]).
    ///
    /// `a` is m×k row-major, `b` is k×n row-major, result is m×n row-major.
    pub fn tensor_matmul(
        &self,
        a: &[f32],
        b: &[f32],
        m: u32,
        k: u32,
        n: u32,
    ) -> Result<Vec<f32>, VulkanError> {
        if a.len() != (m * k) as usize {
            return Err(VulkanError::ShapeMismatch(format!(
                "matmul: a.len()={} != m*k={}",
                a.len(),
                m * k
            )));
        }
        if b.len() != (k * n) as usize {
            return Err(VulkanError::ShapeMismatch(format!(
                "matmul: b.len()={} != k*n={}",
                b.len(),
                k * n
            )));
        }

        let result_len = (m * n) as usize;
        let buf_a = self.create_buffer(a.len() * 4)?;
        let buf_b = self.create_buffer(b.len() * 4)?;
        let buf_c = self.create_buffer(result_len * 4)?;
        // Push constants buffer for dimensions (m, k, n)
        let dims = [m, k, n];
        let buf_dims = self.create_buffer(12)?;

        self.upload_f32(&buf_a, a)?;
        self.upload_f32(&buf_b, b)?;
        // Upload dimensions as raw bytes
        // SAFETY: buf_dims memory is host-visible; 12 bytes (3 x u32) fits within the
        // 12-byte buffer; mapped pointer is valid; unmap balances the map call.
        unsafe {
            let ptr = self
                .device
                .map_memory(buf_dims.memory, 0, 12, vk::MemoryMapFlags::empty())
                .map_err(|e| VulkanError::BufferError(format!("map dims: {e}")))?;
            std::ptr::copy_nonoverlapping(dims.as_ptr() as *const u8, ptr as *mut u8, 12);
            self.device.unmap_memory(buf_dims.memory);
        }

        let buffers = [&buf_a, &buf_b, &buf_c, &buf_dims];
        let workgroups = (n.div_ceil(16), m.div_ceil(16), 1);
        self.dispatch_kernel(KernelOp::Matmul, &buffers, workgroups)?;

        let result = self.download_f32(&buf_c, result_len)?;

        self.destroy_buffer(buf_a);
        self.destroy_buffer(buf_b);
        self.destroy_buffer(buf_c);
        self.destroy_buffer(buf_dims);

        Ok(result)
    }

    // ═══════════════════════════════════════════════════════════════════
    // Internal: Dispatch Helpers
    // ═══════════════════════════════════════════════════════════════════

    fn dispatch_binary(&self, op: KernelOp, a: &[f32], b: &[f32]) -> Result<Vec<f32>, VulkanError> {
        let n = a.len();
        let buf_a = self.create_buffer(n * 4)?;
        let buf_b = self.create_buffer(n * 4)?;
        let buf_c = self.create_buffer(n * 4)?;

        self.upload_f32(&buf_a, a)?;
        self.upload_f32(&buf_b, b)?;

        let buffers = [&buf_a, &buf_b, &buf_c];
        let workgroups = ((n as u32).div_ceil(256), 1, 1);
        self.dispatch_kernel(op, &buffers, workgroups)?;

        let result = self.download_f32(&buf_c, n)?;

        self.destroy_buffer(buf_a);
        self.destroy_buffer(buf_b);
        self.destroy_buffer(buf_c);

        Ok(result)
    }

    fn dispatch_unary(&self, op: KernelOp, x: &[f32]) -> Result<Vec<f32>, VulkanError> {
        let n = x.len();
        let buf_in = self.create_buffer(n * 4)?;
        let buf_out = self.create_buffer(n * 4)?;

        self.upload_f32(&buf_in, x)?;

        let buffers = [&buf_in, &buf_out];
        let workgroups = ((n as u32).div_ceil(256), 1, 1);
        self.dispatch_kernel(op, &buffers, workgroups)?;

        let result = self.download_f32(&buf_out, n)?;

        self.destroy_buffer(buf_in);
        self.destroy_buffer(buf_out);

        Ok(result)
    }

    // ═══════════════════════════════════════════════════════════════════
    // Internal: Kernel Dispatch (Task 17.6)
    // ═══════════════════════════════════════════════════════════════════

    /// Ensure a kernel is compiled, compiling it lazily if needed.
    fn ensure_kernel(&self, op: KernelOp) -> Result<(), VulkanError> {
        let mut kernels = self.kernels.lock().unwrap();
        if kernels.contains_key(&op) {
            return Ok(());
        }
        let num_buffers = match op {
            KernelOp::VectorAdd | KernelOp::VectorMul | KernelOp::VectorSub => 3,
            KernelOp::Relu | KernelOp::Sigmoid => 2,
            KernelOp::Matmul => 4,
        };
        let spirv = spirv_for_op(op);
        let kernel = self.create_compute_pipeline(&spirv, num_buffers)?;
        kernels.insert(op, kernel);
        Ok(())
    }

    fn dispatch_kernel(
        &self,
        op: KernelOp,
        buffers: &[&VulkanBuffer],
        workgroups: (u32, u32, u32),
    ) -> Result<(), VulkanError> {
        self.ensure_kernel(op)?;
        let kernels = self.kernels.lock().unwrap();
        let kernel = kernels.get(&op).unwrap();

        // Allocate descriptor set
        let layouts = [kernel.descriptor_set_layout];
        let ds_alloc = vk::DescriptorSetAllocateInfo::default()
            .descriptor_pool(self.descriptor_pool)
            .set_layouts(&layouts);

        // SAFETY: device and descriptor_pool are valid; layout matches the kernel's descriptor set layout.
        let descriptor_sets = unsafe { self.device.allocate_descriptor_sets(&ds_alloc) }
            .map_err(|e| VulkanError::DispatchError(format!("allocate descriptor set: {e}")))?;
        let descriptor_set = descriptor_sets[0];

        // Write buffer descriptors
        let buffer_infos: Vec<vk::DescriptorBufferInfo> = buffers
            .iter()
            .map(|b| {
                vk::DescriptorBufferInfo::default()
                    .buffer(b.buffer)
                    .offset(0)
                    .range(b.size)
            })
            .collect();

        let writes: Vec<vk::WriteDescriptorSet> = buffer_infos
            .iter()
            .enumerate()
            .map(|(i, info)| {
                vk::WriteDescriptorSet::default()
                    .dst_set(descriptor_set)
                    .dst_binding(i as u32)
                    .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                    .buffer_info(std::slice::from_ref(info))
            })
            .collect();

        // SAFETY: device is valid; descriptor_set and buffer handles are valid; write descriptors
        // reference valid buffer_info entries with correct binding indices.
        unsafe { self.device.update_descriptor_sets(&writes, &[]) };

        // Allocate and record command buffer
        let cb_alloc = vk::CommandBufferAllocateInfo::default()
            .command_pool(self.command_pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(1);

        // SAFETY: device and command_pool are valid; allocating 1 primary command buffer.
        let command_buffers = unsafe { self.device.allocate_command_buffers(&cb_alloc) }
            .map_err(|e| VulkanError::DispatchError(format!("allocate command buffer: {e}")))?;
        let cmd = command_buffers[0];

        let begin_info = vk::CommandBufferBeginInfo::default()
            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);

        // SAFETY: device, cmd, pipeline, descriptor_set, and pipeline_layout are all valid;
        // command buffer is in initial state; workgroup counts are within device limits;
        // memory barrier ensures compute shader writes are visible to host before readback.
        unsafe {
            self.device
                .begin_command_buffer(cmd, &begin_info)
                .map_err(|e| VulkanError::DispatchError(format!("begin cmd: {e}")))?;

            self.device
                .cmd_bind_pipeline(cmd, vk::PipelineBindPoint::COMPUTE, kernel.pipeline);

            self.device.cmd_bind_descriptor_sets(
                cmd,
                vk::PipelineBindPoint::COMPUTE,
                kernel.pipeline_layout,
                0,
                &[descriptor_set],
                &[],
            );

            self.device
                .cmd_dispatch(cmd, workgroups.0, workgroups.1, workgroups.2);

            // Memory barrier to ensure compute writes are visible to host
            let barrier = vk::MemoryBarrier::default()
                .src_access_mask(vk::AccessFlags::SHADER_WRITE)
                .dst_access_mask(vk::AccessFlags::HOST_READ);

            self.device.cmd_pipeline_barrier(
                cmd,
                vk::PipelineStageFlags::COMPUTE_SHADER,
                vk::PipelineStageFlags::HOST,
                vk::DependencyFlags::empty(),
                &[barrier],
                &[],
                &[],
            );

            self.device
                .end_command_buffer(cmd)
                .map_err(|e| VulkanError::DispatchError(format!("end cmd: {e}")))?;
        }

        // Submit and wait
        let submit_info = vk::SubmitInfo::default().command_buffers(&command_buffers);

        let fence_info = vk::FenceCreateInfo::default();
        // SAFETY: device is valid; fence_info uses default (unsignaled) state.
        let fence = unsafe { self.device.create_fence(&fence_info, None) }
            .map_err(|e| VulkanError::DispatchError(format!("create fence: {e}")))?;

        // SAFETY: device, queue, fence, command_buffers, and descriptor_set are all valid;
        // wait_for_fences blocks until GPU work completes; resources are freed after completion.
        unsafe {
            self.device
                .queue_submit(self.compute_queue, &[submit_info], fence)
                .map_err(|e| VulkanError::DispatchError(format!("queue submit: {e}")))?;

            self.device
                .wait_for_fences(&[fence], true, u64::MAX)
                .map_err(|e| VulkanError::DispatchError(format!("wait fence: {e}")))?;

            self.device.destroy_fence(fence, None);
            self.device
                .free_command_buffers(self.command_pool, &command_buffers);
            self.device
                .free_descriptor_sets(self.descriptor_pool, &[descriptor_set])
                .ok(); // ignore error on free
        }

        Ok(())
    }

    // ═══════════════════════════════════════════════════════════════════
    // Internal: Memory Type Selection
    // ═══════════════════════════════════════════════════════════════════

    fn find_memory_type(
        &self,
        type_filter: u32,
        properties: vk::MemoryPropertyFlags,
    ) -> Result<u32, VulkanError> {
        // SAFETY: instance and physical_device are valid for the lifetime of VulkanCompute.
        let mem_props = unsafe {
            self.instance
                .get_physical_device_memory_properties(self.physical_device)
        };

        for i in 0..mem_props.memory_type_count {
            if (type_filter & (1 << i)) != 0
                && mem_props.memory_types[i as usize]
                    .property_flags
                    .contains(properties)
            {
                return Ok(i);
            }
        }

        Err(VulkanError::NoSuitableMemory)
    }

    // ═══════════════════════════════════════════════════════════════════
    // Internal: Shader Compilation (Tasks 17.3, 17.4, 17.6)
    // ═══════════════════════════════════════════════════════════════════

    fn create_compute_pipeline(
        &self,
        spirv: &[u32],
        num_buffers: u32,
    ) -> Result<ComputeKernel, VulkanError> {
        // Create shader module
        let shader_info = vk::ShaderModuleCreateInfo::default().code(spirv);

        // SAFETY: device is valid; shader_info contains well-formed SPIR-V bytecode.
        let shader_module = unsafe { self.device.create_shader_module(&shader_info, None) }
            .map_err(|e| VulkanError::ShaderError(format!("create shader module: {e}")))?;

        // Descriptor set layout
        let bindings: Vec<vk::DescriptorSetLayoutBinding> = (0..num_buffers)
            .map(|i| {
                vk::DescriptorSetLayoutBinding::default()
                    .binding(i)
                    .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                    .descriptor_count(1)
                    .stage_flags(vk::ShaderStageFlags::COMPUTE)
            })
            .collect();

        let dsl_info = vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings);

        // SAFETY: device is valid; dsl_info bindings match the SPIR-V shader's expected layout.
        let descriptor_set_layout =
            unsafe { self.device.create_descriptor_set_layout(&dsl_info, None) }
                .map_err(|e| VulkanError::ShaderError(format!("create dsl: {e}")))?;

        // Pipeline layout
        let set_layouts = [descriptor_set_layout];
        let pl_info = vk::PipelineLayoutCreateInfo::default().set_layouts(&set_layouts);

        // SAFETY: device is valid; set_layouts references the descriptor_set_layout created above.
        let pipeline_layout = unsafe { self.device.create_pipeline_layout(&pl_info, None) }
            .map_err(|e| VulkanError::ShaderError(format!("create pipeline layout: {e}")))?;

        // Compute pipeline
        let entry_name = CString::new("main").unwrap();
        let stage = vk::PipelineShaderStageCreateInfo::default()
            .stage(vk::ShaderStageFlags::COMPUTE)
            .module(shader_module)
            .name(&entry_name);

        let pipeline_info = vk::ComputePipelineCreateInfo::default()
            .stage(stage)
            .layout(pipeline_layout);

        // SAFETY: device is valid; shader module and pipeline layout are valid;
        // pipeline cache is null (not cached); pipeline_info matches Vulkan spec.
        let pipelines = unsafe {
            self.device
                .create_compute_pipelines(vk::PipelineCache::null(), &[pipeline_info], None)
        }
        .map_err(|(_pipelines, e)| VulkanError::ShaderError(format!("create pipeline: {e}")))?;

        // Cleanup shader module (not needed after pipeline creation)
        // SAFETY: device and shader_module are valid; pipeline has been created and no longer
        // references the module; destroyed exactly once.
        unsafe { self.device.destroy_shader_module(shader_module, None) };

        Ok(ComputeKernel {
            pipeline: pipelines[0],
            pipeline_layout,
            descriptor_set_layout,
            _num_buffers: num_buffers,
        })
    }
}

impl Drop for VulkanCompute {
    fn drop(&mut self) {
        // SAFETY: destroying all Vulkan resources in reverse creation order;
        // device_wait_idle ensures no GPU work is in flight; each resource is
        // destroyed exactly once; device and instance are still valid.
        unsafe {
            let _ = self.device.device_wait_idle();

            // Destroy cached kernels
            if let Ok(kernels) = self.kernels.lock() {
                for (_, kernel) in kernels.iter() {
                    self.device.destroy_pipeline(kernel.pipeline, None);
                    self.device
                        .destroy_pipeline_layout(kernel.pipeline_layout, None);
                    self.device
                        .destroy_descriptor_set_layout(kernel.descriptor_set_layout, None);
                }
            }

            self.device
                .destroy_descriptor_pool(self.descriptor_pool, None);
            self.device.destroy_command_pool(self.command_pool, None);
            self.device.destroy_device(None);
            self.instance.destroy_instance(None);
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// SPIR-V Compute Shader Generation (Task 17.4)
// ═══════════════════════════════════════════════════════════════════════

/// Returns SPIR-V bytecode for the given kernel operation.
///
/// Shaders use `local_size_x = 256` workgroup size.
/// Binary ops: binding 0 = A (in), binding 1 = B (in), binding 2 = C (out).
/// Unary ops: binding 0 = input, binding 1 = output.
/// Matmul: binding 0 = A, binding 1 = B, binding 2 = C, binding 3 = dims (m,k,n).
fn spirv_for_op(op: KernelOp) -> Vec<u32> {
    match op {
        KernelOp::VectorAdd => build_binary_spirv(BinarySpirVOp::Add),
        KernelOp::VectorMul => build_binary_spirv(BinarySpirVOp::Mul),
        KernelOp::VectorSub => build_binary_spirv(BinarySpirVOp::Sub),
        KernelOp::Relu => build_relu_spirv(),
        KernelOp::Sigmoid => build_sigmoid_spirv(),
        KernelOp::Matmul => build_matmul_spirv(),
    }
}

#[derive(Clone, Copy)]
enum BinarySpirVOp {
    Add,
    Mul,
    Sub,
}

/// Build SPIR-V for element-wise binary operation.
///
/// GLSL equivalent:
/// ```glsl
/// #version 450
/// layout(local_size_x = 256) in;
/// layout(set=0, binding=0) buffer A { float a[]; };
/// layout(set=0, binding=1) buffer B { float b[]; };
/// layout(set=0, binding=2) buffer C { float c[]; };
/// void main() {
///     uint idx = gl_GlobalInvocationID.x;
///     c[idx] = a[idx] OP b[idx];
/// }
/// ```
fn build_binary_spirv(op: BinarySpirVOp) -> Vec<u32> {
    let mut s = SpirVBuilder::new();

    // Header
    s.capability(1); // Shader
    let glsl_ext = s.ext_inst_import("GLSL.std.450");
    s.memory_model(0, 1); // Logical, GLSL450

    // Types
    let void = s.type_void();
    let void_fn = s.type_function(void, &[]);
    let f32_ty = s.type_float(32);
    let u32_ty = s.type_int(32, 0);
    let u32x3 = s.type_vector(u32_ty, 3);
    let ptr_input_u32x3 = s.type_pointer(1, u32x3); // Input
    let ptr_input_u32 = s.type_pointer(1, u32_ty);
    let runtime_arr = s.type_runtime_array(f32_ty);
    let buf_struct = s.type_struct(&[runtime_arr]);
    let ptr_storage_struct = s.type_pointer(12, buf_struct); // StorageBuffer
    let ptr_storage_f32 = s.type_pointer(12, f32_ty);

    // Constants
    let const_0 = s.constant_u32(u32_ty, 0);

    // Variables
    let gl_gid = s.variable(ptr_input_u32x3, 1); // Input
    let buf_a = s.variable(ptr_storage_struct, 12); // StorageBuffer
    let buf_b = s.variable(ptr_storage_struct, 12);
    let buf_c = s.variable(ptr_storage_struct, 12);

    // Entry point
    let main_fn = s.next_id();
    s.entry_point(5, main_fn, "main", &[gl_gid]); // GLCompute
    s.execution_mode(main_fn, 17, &[256, 1, 1]); // LocalSize

    // Decorations
    s.decorate(gl_gid, 11, &[28]); // BuiltIn GlobalInvocationId
    s.decorate(buf_struct, 2, &[]); // Block
    s.member_decorate(buf_struct, 0, 35, &[0]); // Offset 0
    s.decorate(runtime_arr, 6, &[4]); // ArrayStride 4
    s.decorate(buf_a, 34, &[0]); // DescriptorSet 0
    s.decorate(buf_a, 33, &[0]); // Binding 0
    s.decorate(buf_b, 34, &[0]); // DescriptorSet 0
    s.decorate(buf_b, 33, &[1]); // Binding 1
    s.decorate(buf_c, 34, &[0]); // DescriptorSet 0
    s.decorate(buf_c, 33, &[2]); // Binding 2

    // Function
    s.function(void, main_fn, 0, void_fn);
    let entry_label = s.label();
    let gid_ptr = s.access_chain(ptr_input_u32, gl_gid, &[const_0]);
    let gid = s.load(u32_ty, gid_ptr);

    let a_ptr = s.access_chain(ptr_storage_f32, buf_a, &[const_0, gid]);
    let a_val = s.load(f32_ty, a_ptr);
    let b_ptr = s.access_chain(ptr_storage_f32, buf_b, &[const_0, gid]);
    let b_val = s.load(f32_ty, b_ptr);

    // Operation
    let result = match op {
        BinarySpirVOp::Add => s.f_add(f32_ty, a_val, b_val),
        BinarySpirVOp::Mul => s.f_mul(f32_ty, a_val, b_val),
        BinarySpirVOp::Sub => s.f_sub(f32_ty, a_val, b_val),
    };

    let c_ptr = s.access_chain(ptr_storage_f32, buf_c, &[const_0, gid]);
    s.store(c_ptr, result);
    s.op_return();
    s.function_end();

    let _ = (glsl_ext, entry_label);
    s.build()
}

/// Build SPIR-V for ReLU: y[i] = max(0, x[i]).
fn build_relu_spirv() -> Vec<u32> {
    let mut s = SpirVBuilder::new();

    s.capability(1);
    let glsl_ext = s.ext_inst_import("GLSL.std.450");
    s.memory_model(0, 1);

    let void = s.type_void();
    let void_fn = s.type_function(void, &[]);
    let f32_ty = s.type_float(32);
    let u32_ty = s.type_int(32, 0);
    let u32x3 = s.type_vector(u32_ty, 3);
    let ptr_input_u32x3 = s.type_pointer(1, u32x3);
    let ptr_input_u32 = s.type_pointer(1, u32_ty);
    let runtime_arr = s.type_runtime_array(f32_ty);
    let buf_struct = s.type_struct(&[runtime_arr]);
    let ptr_storage_struct = s.type_pointer(12, buf_struct);
    let ptr_storage_f32 = s.type_pointer(12, f32_ty);

    let const_0u = s.constant_u32(u32_ty, 0);
    let const_0f = s.constant_f32(f32_ty, 0.0);

    let gl_gid = s.variable(ptr_input_u32x3, 1);
    let buf_in = s.variable(ptr_storage_struct, 12);
    let buf_out = s.variable(ptr_storage_struct, 12);

    let main_fn = s.next_id();
    s.entry_point(5, main_fn, "main", &[gl_gid]);
    s.execution_mode(main_fn, 17, &[256, 1, 1]);

    s.decorate(gl_gid, 11, &[28]);
    s.decorate(buf_struct, 2, &[]);
    s.member_decorate(buf_struct, 0, 35, &[0]);
    s.decorate(runtime_arr, 6, &[4]);
    s.decorate(buf_in, 34, &[0]);
    s.decorate(buf_in, 33, &[0]);
    s.decorate(buf_out, 34, &[0]);
    s.decorate(buf_out, 33, &[1]);

    s.function(void, main_fn, 0, void_fn);
    let _label = s.label();
    let gid_ptr = s.access_chain(ptr_input_u32, gl_gid, &[const_0u]);
    let gid = s.load(u32_ty, gid_ptr);

    let in_ptr = s.access_chain(ptr_storage_f32, buf_in, &[const_0u, gid]);
    let x = s.load(f32_ty, in_ptr);

    // max(0.0, x) using GLSL.std.450 FMax (opcode 40)
    let result = s.ext_inst(f32_ty, glsl_ext, 40, &[const_0f, x]); // FMax

    let out_ptr = s.access_chain(ptr_storage_f32, buf_out, &[const_0u, gid]);
    s.store(out_ptr, result);
    s.op_return();
    s.function_end();

    s.build()
}

/// Build SPIR-V for Sigmoid: y[i] = 1 / (1 + exp(-x[i])).
fn build_sigmoid_spirv() -> Vec<u32> {
    let mut s = SpirVBuilder::new();

    s.capability(1);
    let glsl_ext = s.ext_inst_import("GLSL.std.450");
    s.memory_model(0, 1);

    let void = s.type_void();
    let void_fn = s.type_function(void, &[]);
    let f32_ty = s.type_float(32);
    let u32_ty = s.type_int(32, 0);
    let u32x3 = s.type_vector(u32_ty, 3);
    let ptr_input_u32x3 = s.type_pointer(1, u32x3);
    let ptr_input_u32 = s.type_pointer(1, u32_ty);
    let runtime_arr = s.type_runtime_array(f32_ty);
    let buf_struct = s.type_struct(&[runtime_arr]);
    let ptr_storage_struct = s.type_pointer(12, buf_struct);
    let ptr_storage_f32 = s.type_pointer(12, f32_ty);

    let const_0u = s.constant_u32(u32_ty, 0);
    let const_1f = s.constant_f32(f32_ty, 1.0);

    let gl_gid = s.variable(ptr_input_u32x3, 1);
    let buf_in = s.variable(ptr_storage_struct, 12);
    let buf_out = s.variable(ptr_storage_struct, 12);

    let main_fn = s.next_id();
    s.entry_point(5, main_fn, "main", &[gl_gid]);
    s.execution_mode(main_fn, 17, &[256, 1, 1]);

    s.decorate(gl_gid, 11, &[28]);
    s.decorate(buf_struct, 2, &[]);
    s.member_decorate(buf_struct, 0, 35, &[0]);
    s.decorate(runtime_arr, 6, &[4]);
    s.decorate(buf_in, 34, &[0]);
    s.decorate(buf_in, 33, &[0]);
    s.decorate(buf_out, 34, &[0]);
    s.decorate(buf_out, 33, &[1]);

    s.function(void, main_fn, 0, void_fn);
    let _label = s.label();
    let gid_ptr = s.access_chain(ptr_input_u32, gl_gid, &[const_0u]);
    let gid = s.load(u32_ty, gid_ptr);

    let in_ptr = s.access_chain(ptr_storage_f32, buf_in, &[const_0u, gid]);
    let x = s.load(f32_ty, in_ptr);

    // sigmoid(x) = 1.0 / (1.0 + exp(-x))
    let neg_x = s.f_negate(f32_ty, x);
    let exp_neg_x = s.ext_inst(f32_ty, glsl_ext, 27, &[neg_x]); // Exp
    let one_plus_exp = s.f_add(f32_ty, const_1f, exp_neg_x);
    let result = s.f_div(f32_ty, const_1f, one_plus_exp);

    let out_ptr = s.access_chain(ptr_storage_f32, buf_out, &[const_0u, gid]);
    s.store(out_ptr, result);
    s.op_return();
    s.function_end();

    s.build()
}

/// Build SPIR-V for matrix multiplication: C[m,n] = sum_k(A[m,k] * B[k,n]).
///
/// Uses 16x16 workgroup tiling. Binding 3 stores dimensions as uint3(m, k, n).
fn build_matmul_spirv() -> Vec<u32> {
    let mut s = SpirVBuilder::new();

    s.capability(1);
    let _glsl_ext = s.ext_inst_import("GLSL.std.450");
    s.memory_model(0, 1);

    let void = s.type_void();
    let void_fn = s.type_function(void, &[]);
    let f32_ty = s.type_float(32);
    let u32_ty = s.type_int(32, 0);
    let u32x3 = s.type_vector(u32_ty, 3);
    let bool_ty = s.type_bool();
    let ptr_input_u32x3 = s.type_pointer(1, u32x3);
    let ptr_input_u32 = s.type_pointer(1, u32_ty);
    let runtime_arr_f32 = s.type_runtime_array(f32_ty);
    let runtime_arr_u32 = s.type_runtime_array(u32_ty);
    let buf_f32_struct = s.type_struct(&[runtime_arr_f32]);
    let buf_u32_struct = s.type_struct(&[runtime_arr_u32]);
    let ptr_storage_f32_struct = s.type_pointer(12, buf_f32_struct);
    let ptr_storage_u32_struct = s.type_pointer(12, buf_u32_struct);
    let ptr_storage_f32 = s.type_pointer(12, f32_ty);
    let ptr_storage_u32 = s.type_pointer(12, u32_ty);
    let ptr_function_f32 = s.type_pointer(7, f32_ty); // Function
    let ptr_function_u32 = s.type_pointer(7, u32_ty);

    let const_0u = s.constant_u32(u32_ty, 0);
    let const_1u = s.constant_u32(u32_ty, 1);
    let const_2u = s.constant_u32(u32_ty, 2);
    let const_0f = s.constant_f32(f32_ty, 0.0);

    let gl_gid = s.variable(ptr_input_u32x3, 1);
    let buf_a = s.variable(ptr_storage_f32_struct, 12);
    let buf_b = s.variable(ptr_storage_f32_struct, 12);
    let buf_c = s.variable(ptr_storage_f32_struct, 12);
    let buf_dims = s.variable(ptr_storage_u32_struct, 12);

    let main_fn = s.next_id();
    s.entry_point(5, main_fn, "main", &[gl_gid]);
    s.execution_mode(main_fn, 17, &[16, 16, 1]);

    // Decorations
    s.decorate(gl_gid, 11, &[28]);
    s.decorate(buf_f32_struct, 2, &[]);
    s.member_decorate(buf_f32_struct, 0, 35, &[0]);
    s.decorate(runtime_arr_f32, 6, &[4]);
    s.decorate(buf_u32_struct, 2, &[]);
    s.member_decorate(buf_u32_struct, 0, 35, &[0]);
    s.decorate(runtime_arr_u32, 6, &[4]);
    s.decorate(buf_a, 34, &[0]);
    s.decorate(buf_a, 33, &[0]);
    s.decorate(buf_b, 34, &[0]);
    s.decorate(buf_b, 33, &[1]);
    s.decorate(buf_c, 34, &[0]);
    s.decorate(buf_c, 33, &[2]);
    s.decorate(buf_dims, 34, &[0]);
    s.decorate(buf_dims, 33, &[3]);

    // Function body
    s.function(void, main_fn, 0, void_fn);
    let _entry = s.label();

    // SPIR-V requires all Function-scope OpVariables in the entry block
    let sum_var = s.variable_function(ptr_function_f32, const_0f);
    let i_var = s.variable_function(ptr_function_u32, const_0u);

    // col = gid.x, row = gid.y
    let col_ptr = s.access_chain(ptr_input_u32, gl_gid, &[const_0u]);
    let col = s.load(u32_ty, col_ptr);
    let row_ptr = s.access_chain(ptr_input_u32, gl_gid, &[const_1u]);
    let row = s.load(u32_ty, row_ptr);

    // Load dimensions: m, k, n
    let m_ptr = s.access_chain(ptr_storage_u32, buf_dims, &[const_0u, const_0u]);
    let m = s.load(u32_ty, m_ptr);
    let k_ptr = s.access_chain(ptr_storage_u32, buf_dims, &[const_0u, const_1u]);
    let k = s.load(u32_ty, k_ptr);
    let n_ptr = s.access_chain(ptr_storage_u32, buf_dims, &[const_0u, const_2u]);
    let n = s.load(u32_ty, n_ptr);

    // Bounds check: if (row >= m || col >= n) return
    let row_ok = s.u_less_than(bool_ty, row, m);
    let col_ok = s.u_less_than(bool_ty, col, n);
    let in_bounds = s.logical_and(bool_ty, row_ok, col_ok);

    let body_label = s.next_id();
    let end_label = s.next_id();
    s.selection_merge(end_label);
    s.branch_conditional(in_bounds, body_label, end_label);

    // Body: accumulate dot product
    s.emit_label(body_label);
    // Reset sum and i for this invocation
    s.store(sum_var, const_0f);
    s.store(i_var, const_0u);

    // Loop header — OpLoopMerge must immediately precede the branch
    let loop_header = s.next_id();
    let loop_body = s.next_id();
    let loop_continue = s.next_id();
    let loop_merge = s.next_id();
    s.branch(loop_header);
    s.emit_label(loop_header);
    let i = s.load(u32_ty, i_var);
    let cond = s.u_less_than(bool_ty, i, k);
    s.loop_merge(loop_merge, loop_continue);
    s.branch_conditional(cond, loop_body, loop_merge);

    // Loop body: sum += A[row*k + i] * B[i*n + col]
    s.emit_label(loop_body);
    let row_k = s.i_mul(u32_ty, row, k);
    let a_idx = s.i_add(u32_ty, row_k, i);
    let a_elem_ptr = s.access_chain(ptr_storage_f32, buf_a, &[const_0u, a_idx]);
    let a_val = s.load(f32_ty, a_elem_ptr);

    let i_n = s.i_mul(u32_ty, i, n);
    let b_idx = s.i_add(u32_ty, i_n, col);
    let b_elem_ptr = s.access_chain(ptr_storage_f32, buf_b, &[const_0u, b_idx]);
    let b_val = s.load(f32_ty, b_elem_ptr);

    let prod = s.f_mul(f32_ty, a_val, b_val);
    let cur_sum = s.load(f32_ty, sum_var);
    let new_sum = s.f_add(f32_ty, cur_sum, prod);
    s.store(sum_var, new_sum);

    s.branch(loop_continue);
    s.emit_label(loop_continue);
    let next_i = s.i_add(u32_ty, i, const_1u);
    s.store(i_var, next_i);
    s.branch(loop_header);

    // After loop: C[row*n + col] = sum
    s.emit_label(loop_merge);
    let row_n = s.i_mul(u32_ty, row, n);
    let c_idx = s.i_add(u32_ty, row_n, col);
    let c_elem_ptr = s.access_chain(ptr_storage_f32, buf_c, &[const_0u, c_idx]);
    let final_sum = s.load(f32_ty, sum_var);
    s.store(c_elem_ptr, final_sum);

    s.branch(end_label);
    s.emit_label(end_label);
    s.op_return();
    s.function_end();

    s.build()
}

// ═══════════════════════════════════════════════════════════════════════
// SPIR-V Builder — Programmatic SPIR-V Generation
// ═══════════════════════════════════════════════════════════════════════

/// Minimal SPIR-V builder for constructing compute shader bytecode.
///
/// Generates valid SPIR-V 1.0 modules from Rust code, avoiding the need
/// for external shader compilers (glslangValidator, shaderc).
struct SpirVBuilder {
    /// Capability and extension declarations.
    capabilities: Vec<u32>,
    /// Extension imports.
    ext_imports: Vec<u32>,
    /// Memory model declaration.
    memory_model_words: Vec<u32>,
    /// Entry points.
    entry_points: Vec<u32>,
    /// Execution modes.
    execution_modes: Vec<u32>,
    /// Annotations (decorations).
    annotations: Vec<u32>,
    /// Type declarations.
    type_decls: Vec<u32>,
    /// Global variables.
    globals: Vec<u32>,
    /// Function bodies.
    functions: Vec<u32>,
    /// Next available result ID.
    id_counter: u32,
}

impl SpirVBuilder {
    fn new() -> Self {
        Self {
            capabilities: Vec::new(),
            ext_imports: Vec::new(),
            memory_model_words: Vec::new(),
            entry_points: Vec::new(),
            execution_modes: Vec::new(),
            annotations: Vec::new(),
            type_decls: Vec::new(),
            globals: Vec::new(),
            functions: Vec::new(),
            id_counter: 1,
        }
    }

    fn next_id(&mut self) -> u32 {
        let id = self.id_counter;
        self.id_counter += 1;
        id
    }

    fn build(self) -> Vec<u32> {
        // SPIR-V header
        let mut words = vec![
            0x07230203,      // Magic number
            0x00010300,      // Version 1.3 (StorageBuffer class requires 1.3+)
            0,               // Generator (unregistered)
            self.id_counter, // Bound
            0,               // Schema
        ];

        words.extend(&self.capabilities);
        words.extend(&self.ext_imports);
        words.extend(&self.memory_model_words);
        words.extend(&self.entry_points);
        words.extend(&self.execution_modes);
        words.extend(&self.annotations);
        words.extend(&self.type_decls);
        words.extend(&self.globals);
        words.extend(&self.functions);

        words
    }

    // ─── Instructions ────────────────────────────────────────────────

    fn capability(&mut self, cap: u32) {
        self.capabilities.push(encode_op(17, 2)); // OpCapability
        self.capabilities.push(cap);
    }

    fn ext_inst_import(&mut self, name: &str) -> u32 {
        let id = self.next_id();
        let name_words = encode_string(name);
        let len = 2 + name_words.len() as u16;
        self.ext_imports.push(encode_op(11, len)); // OpExtInstImport
        self.ext_imports.push(id);
        self.ext_imports.extend(&name_words);
        id
    }

    fn memory_model(&mut self, addressing: u32, memory: u32) {
        self.memory_model_words.push(encode_op(14, 3)); // OpMemoryModel
        self.memory_model_words.push(addressing);
        self.memory_model_words.push(memory);
    }

    fn entry_point(&mut self, execution_model: u32, func_id: u32, name: &str, interfaces: &[u32]) {
        let name_words = encode_string(name);
        let len = 3 + name_words.len() as u16 + interfaces.len() as u16;
        self.entry_points.push(encode_op(15, len)); // OpEntryPoint
        self.entry_points.push(execution_model);
        self.entry_points.push(func_id);
        self.entry_points.extend(&name_words);
        self.entry_points.extend(interfaces);
    }

    fn execution_mode(&mut self, func_id: u32, mode: u32, operands: &[u32]) {
        let len = 3 + operands.len() as u16;
        self.execution_modes.push(encode_op(16, len)); // OpExecutionMode
        self.execution_modes.push(func_id);
        self.execution_modes.push(mode);
        self.execution_modes.extend(operands);
    }

    fn decorate(&mut self, target: u32, decoration: u32, operands: &[u32]) {
        let len = 3 + operands.len() as u16;
        self.annotations.push(encode_op(71, len)); // OpDecorate
        self.annotations.push(target);
        self.annotations.push(decoration);
        self.annotations.extend(operands);
    }

    fn member_decorate(
        &mut self,
        struct_type: u32,
        member: u32,
        decoration: u32,
        operands: &[u32],
    ) {
        let len = 4 + operands.len() as u16;
        self.annotations.push(encode_op(72, len)); // OpMemberDecorate
        self.annotations.push(struct_type);
        self.annotations.push(member);
        self.annotations.push(decoration);
        self.annotations.extend(operands);
    }

    // ─── Types ───────────────────────────────────────────────────────

    fn type_void(&mut self) -> u32 {
        let id = self.next_id();
        self.type_decls.push(encode_op(19, 2)); // OpTypeVoid
        self.type_decls.push(id);
        id
    }

    fn type_bool(&mut self) -> u32 {
        let id = self.next_id();
        self.type_decls.push(encode_op(20, 2)); // OpTypeBool
        self.type_decls.push(id);
        id
    }

    fn type_float(&mut self, width: u32) -> u32 {
        let id = self.next_id();
        self.type_decls.push(encode_op(22, 3)); // OpTypeFloat
        self.type_decls.push(id);
        self.type_decls.push(width);
        id
    }

    fn type_int(&mut self, width: u32, signedness: u32) -> u32 {
        let id = self.next_id();
        self.type_decls.push(encode_op(21, 4)); // OpTypeInt
        self.type_decls.push(id);
        self.type_decls.push(width);
        self.type_decls.push(signedness);
        id
    }

    fn type_vector(&mut self, component: u32, count: u32) -> u32 {
        let id = self.next_id();
        self.type_decls.push(encode_op(23, 4)); // OpTypeVector
        self.type_decls.push(id);
        self.type_decls.push(component);
        self.type_decls.push(count);
        id
    }

    fn type_function(&mut self, return_type: u32, params: &[u32]) -> u32 {
        let id = self.next_id();
        let len = 3 + params.len() as u16;
        self.type_decls.push(encode_op(33, len)); // OpTypeFunction
        self.type_decls.push(id);
        self.type_decls.push(return_type);
        self.type_decls.extend(params);
        id
    }

    fn type_pointer(&mut self, storage_class: u32, pointee: u32) -> u32 {
        let id = self.next_id();
        self.type_decls.push(encode_op(32, 4)); // OpTypePointer
        self.type_decls.push(id);
        self.type_decls.push(storage_class);
        self.type_decls.push(pointee);
        id
    }

    fn type_runtime_array(&mut self, element: u32) -> u32 {
        let id = self.next_id();
        self.type_decls.push(encode_op(29, 3)); // OpTypeRuntimeArray
        self.type_decls.push(id);
        self.type_decls.push(element);
        id
    }

    fn type_struct(&mut self, members: &[u32]) -> u32 {
        let id = self.next_id();
        let len = 2 + members.len() as u16;
        self.type_decls.push(encode_op(30, len)); // OpTypeStruct
        self.type_decls.push(id);
        self.type_decls.extend(members);
        id
    }

    // ─── Constants ───────────────────────────────────────────────────

    fn constant_u32(&mut self, ty: u32, value: u32) -> u32 {
        let id = self.next_id();
        self.type_decls.push(encode_op(43, 4)); // OpConstant
        self.type_decls.push(ty);
        self.type_decls.push(id);
        self.type_decls.push(value);
        id
    }

    fn constant_f32(&mut self, ty: u32, value: f32) -> u32 {
        let id = self.next_id();
        self.type_decls.push(encode_op(43, 4)); // OpConstant
        self.type_decls.push(ty);
        self.type_decls.push(id);
        self.type_decls.push(value.to_bits());
        id
    }

    // ─── Variables ───────────────────────────────────────────────────

    fn variable(&mut self, ty: u32, storage_class: u32) -> u32 {
        let id = self.next_id();
        self.globals.push(encode_op(59, 4)); // OpVariable
        self.globals.push(ty);
        self.globals.push(id);
        self.globals.push(storage_class);
        id
    }

    fn variable_function(&mut self, ty: u32, initializer: u32) -> u32 {
        let id = self.next_id();
        self.functions.push(encode_op(59, 5)); // OpVariable with initializer
        self.functions.push(ty);
        self.functions.push(id);
        self.functions.push(7); // Function storage class
        self.functions.push(initializer);
        id
    }

    // ─── Function Instructions ───────────────────────────────────────

    fn function(&mut self, result_type: u32, result_id: u32, control: u32, fn_type: u32) {
        self.functions.push(encode_op(54, 5)); // OpFunction
        self.functions.push(result_type);
        self.functions.push(result_id);
        self.functions.push(control);
        self.functions.push(fn_type);
    }

    fn function_end(&mut self) {
        self.functions.push(encode_op(56, 1)); // OpFunctionEnd
    }

    fn label(&mut self) -> u32 {
        let id = self.next_id();
        self.functions.push(encode_op(248, 2)); // OpLabel
        self.functions.push(id);
        id
    }

    fn emit_label(&mut self, id: u32) {
        self.functions.push(encode_op(248, 2));
        self.functions.push(id);
    }

    fn op_return(&mut self) {
        self.functions.push(encode_op(253, 1)); // OpReturn
    }

    // ─── Memory Instructions ─────────────────────────────────────────

    fn access_chain(&mut self, result_type: u32, base: u32, indices: &[u32]) -> u32 {
        let id = self.next_id();
        let len = 4 + indices.len() as u16;
        self.functions.push(encode_op(65, len)); // OpAccessChain
        self.functions.push(result_type);
        self.functions.push(id);
        self.functions.push(base);
        self.functions.extend(indices);
        id
    }

    fn load(&mut self, result_type: u32, pointer: u32) -> u32 {
        let id = self.next_id();
        self.functions.push(encode_op(61, 4)); // OpLoad
        self.functions.push(result_type);
        self.functions.push(id);
        self.functions.push(pointer);
        id
    }

    fn store(&mut self, pointer: u32, value: u32) {
        self.functions.push(encode_op(62, 3)); // OpStore
        self.functions.push(pointer);
        self.functions.push(value);
    }

    // ─── Arithmetic Instructions ─────────────────────────────────────

    fn f_add(&mut self, result_type: u32, a: u32, b: u32) -> u32 {
        let id = self.next_id();
        self.functions.push(encode_op(129, 5)); // OpFAdd
        self.functions.push(result_type);
        self.functions.push(id);
        self.functions.push(a);
        self.functions.push(b);
        id
    }

    fn f_sub(&mut self, result_type: u32, a: u32, b: u32) -> u32 {
        let id = self.next_id();
        self.functions.push(encode_op(131, 5)); // OpFSub
        self.functions.push(result_type);
        self.functions.push(id);
        self.functions.push(a);
        self.functions.push(b);
        id
    }

    fn f_mul(&mut self, result_type: u32, a: u32, b: u32) -> u32 {
        let id = self.next_id();
        self.functions.push(encode_op(133, 5)); // OpFMul
        self.functions.push(result_type);
        self.functions.push(id);
        self.functions.push(a);
        self.functions.push(b);
        id
    }

    fn f_div(&mut self, result_type: u32, a: u32, b: u32) -> u32 {
        let id = self.next_id();
        self.functions.push(encode_op(136, 5)); // OpFDiv
        self.functions.push(result_type);
        self.functions.push(id);
        self.functions.push(a);
        self.functions.push(b);
        id
    }

    fn f_negate(&mut self, result_type: u32, a: u32) -> u32 {
        let id = self.next_id();
        self.functions.push(encode_op(127, 4)); // OpFNegate
        self.functions.push(result_type);
        self.functions.push(id);
        self.functions.push(a);
        id
    }

    fn i_add(&mut self, result_type: u32, a: u32, b: u32) -> u32 {
        let id = self.next_id();
        self.functions.push(encode_op(128, 5)); // OpIAdd
        self.functions.push(result_type);
        self.functions.push(id);
        self.functions.push(a);
        self.functions.push(b);
        id
    }

    fn i_mul(&mut self, result_type: u32, a: u32, b: u32) -> u32 {
        let id = self.next_id();
        self.functions.push(encode_op(132, 5)); // OpIMul
        self.functions.push(result_type);
        self.functions.push(id);
        self.functions.push(a);
        self.functions.push(b);
        id
    }

    // ─── Comparison Instructions ─────────────────────────────────────

    fn u_less_than(&mut self, result_type: u32, a: u32, b: u32) -> u32 {
        let id = self.next_id();
        self.functions.push(encode_op(176, 5)); // OpULessThan
        self.functions.push(result_type);
        self.functions.push(id);
        self.functions.push(a);
        self.functions.push(b);
        id
    }

    fn logical_and(&mut self, result_type: u32, a: u32, b: u32) -> u32 {
        let id = self.next_id();
        self.functions.push(encode_op(167, 5)); // OpLogicalAnd
        self.functions.push(result_type);
        self.functions.push(id);
        self.functions.push(a);
        self.functions.push(b);
        id
    }

    // ─── Control Flow ────────────────────────────────────────────────

    fn branch(&mut self, target: u32) {
        self.functions.push(encode_op(249, 2)); // OpBranch
        self.functions.push(target);
    }

    fn branch_conditional(&mut self, condition: u32, true_label: u32, false_label: u32) {
        self.functions.push(encode_op(250, 4)); // OpBranchConditional
        self.functions.push(condition);
        self.functions.push(true_label);
        self.functions.push(false_label);
    }

    fn selection_merge(&mut self, merge_block: u32) {
        self.functions.push(encode_op(247, 3)); // OpSelectionMerge
        self.functions.push(merge_block);
        self.functions.push(0); // None
    }

    fn loop_merge(&mut self, merge_block: u32, continue_target: u32) {
        self.functions.push(encode_op(246, 4)); // OpLoopMerge
        self.functions.push(merge_block);
        self.functions.push(continue_target);
        self.functions.push(0); // None
    }

    // ─── Extended Instructions ───────────────────────────────────────

    fn ext_inst(
        &mut self,
        result_type: u32,
        ext_set: u32,
        instruction: u32,
        operands: &[u32],
    ) -> u32 {
        let id = self.next_id();
        let len = 5 + operands.len() as u16;
        self.functions.push(encode_op(12, len)); // OpExtInst
        self.functions.push(result_type);
        self.functions.push(id);
        self.functions.push(ext_set);
        self.functions.push(instruction);
        self.functions.extend(operands);
        id
    }
}

/// Encode a SPIR-V instruction word (opcode + word count).
fn encode_op(opcode: u16, word_count: u16) -> u32 {
    ((word_count as u32) << 16) | (opcode as u32)
}

/// Encode a string as SPIR-V literal words (null-terminated, padded to 4 bytes).
fn encode_string(s: &str) -> Vec<u32> {
    let bytes = s.as_bytes();
    let mut padded = bytes.to_vec();
    padded.push(0); // null terminator
    while !padded.len().is_multiple_of(4) {
        padded.push(0); // pad to 4-byte alignment
    }
    padded
        .chunks(4)
        .map(|chunk| {
            u32::from_le_bytes([
                chunk[0],
                chunk.get(1).copied().unwrap_or(0),
                chunk.get(2).copied().unwrap_or(0),
                chunk.get(3).copied().unwrap_or(0),
            ])
        })
        .collect()
}

// ═══════════════════════════════════════════════════════════════════════
// Tests (Task 17.10)
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ─── SPIR-V builder tests (always run) ───────────────────────────

    #[test]
    fn spirv_encode_string_simple() {
        let words = encode_string("main");
        // "main" = 4 chars + null = 5 bytes → 2 words (8 bytes padded)
        assert_eq!(words.len(), 2);
        assert_eq!(words[0], u32::from_le_bytes([b'm', b'a', b'i', b'n']));
    }

    #[test]
    fn spirv_encode_op() {
        // OpCapability (opcode 17), word count 2
        let word = encode_op(17, 2);
        assert_eq!(word & 0xFFFF, 17);
        assert_eq!(word >> 16, 2);
    }

    #[test]
    fn spirv_binary_add_valid_header() {
        let spirv = build_binary_spirv(BinarySpirVOp::Add);
        assert_eq!(spirv[0], 0x07230203, "SPIR-V magic number");
        assert_eq!(spirv[1], 0x00010300, "SPIR-V version 1.3");
        assert!(spirv.len() > 20, "module should have content");
    }

    #[test]
    fn spirv_binary_mul_differs_from_add() {
        let add = build_binary_spirv(BinarySpirVOp::Add);
        let mul = build_binary_spirv(BinarySpirVOp::Mul);
        assert_ne!(add, mul, "add and mul shaders should differ");
    }

    #[test]
    fn spirv_relu_valid_header() {
        let spirv = build_relu_spirv();
        assert_eq!(spirv[0], 0x07230203);
        assert!(spirv.len() > 20);
    }

    #[test]
    fn spirv_sigmoid_valid_header() {
        let spirv = build_sigmoid_spirv();
        assert_eq!(spirv[0], 0x07230203);
        assert!(spirv.len() > 20);
    }

    #[test]
    fn spirv_matmul_valid_header() {
        let spirv = build_matmul_spirv();
        assert_eq!(spirv[0], 0x07230203);
        assert!(spirv.len() > 50, "matmul shader should be larger");
    }

    #[test]
    fn spirv_all_ops_generate() {
        for op in [
            KernelOp::VectorAdd,
            KernelOp::VectorMul,
            KernelOp::VectorSub,
            KernelOp::Relu,
            KernelOp::Sigmoid,
            KernelOp::Matmul,
        ] {
            let spirv = spirv_for_op(op);
            assert_eq!(spirv[0], 0x07230203, "valid SPIR-V for {op}");
            assert!(spirv.len() > 10, "non-empty SPIR-V for {op}");
        }
    }

    #[test]
    fn spirv_builder_id_allocation() {
        let mut b = SpirVBuilder::new();
        let id1 = b.next_id();
        let id2 = b.next_id();
        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
    }

    #[test]
    fn kernel_op_display() {
        assert_eq!(format!("{}", KernelOp::VectorAdd), "vector_add");
        assert_eq!(format!("{}", KernelOp::Matmul), "matmul");
        assert_eq!(format!("{}", KernelOp::Relu), "relu");
        assert_eq!(format!("{}", KernelOp::Sigmoid), "sigmoid");
    }

    #[test]
    fn vulkan_error_display() {
        let e = VulkanError::NotAvailable("test".to_string());
        assert!(format!("{e}").contains("VE001"));

        let e = VulkanError::NoComputeDevice;
        assert!(format!("{e}").contains("VE002"));

        let e = VulkanError::ShapeMismatch("x != y".to_string());
        assert!(format!("{e}").contains("VE007"));
    }

    #[test]
    fn vulkan_device_info_display() {
        let info = VulkanDeviceInfo {
            name: "Test GPU".to_string(),
            api_version: "1.3.318".to_string(),
            driver_version: "25.2.8".to_string(),
            device_type: "Integrated GPU".to_string(),
            max_workgroup_count: [65535, 65535, 65535],
            max_workgroup_size: [1024, 1024, 64],
            max_shared_memory: 32768,
            subgroup_size: 128,
        };
        let s = format!("{info}");
        assert!(s.contains("Test GPU"));
        assert!(s.contains("1.3.318"));
    }

    #[test]
    fn vulkan_availability_check() {
        // This test always passes — it checks the API, not the result
        let _available = VulkanCompute::is_available();
    }

    #[test]
    fn spirv_matmul_validate_with_spirv_val() {
        let spirv = build_matmul_spirv();
        let bytes: Vec<u8> = spirv.iter().flat_map(|w| w.to_le_bytes()).collect();
        let path = "/tmp/fj_matmul_test.spv";
        std::fs::write(path, &bytes).unwrap();
        let output = std::process::Command::new("spirv-val").arg(path).output();
        match output {
            Ok(o) => {
                let stderr = String::from_utf8_lossy(&o.stderr);
                assert!(o.status.success(), "spirv-val failed:\n{stderr}");
            }
            Err(_) => {
                // spirv-val not available, skip
            }
        }
    }

    // ─── Integration tests (require Vulkan) ──────────────────────────
    // These tests are gated on VulkanCompute::new() success, so they
    // gracefully skip on systems without Vulkan.

    #[test]
    fn vulkan_init_and_info() {
        let vk = match VulkanCompute::new() {
            Ok(v) => v,
            Err(_) => return, // skip on systems without Vulkan
        };
        let info = vk.device_info();
        assert!(!info.name.is_empty());
        assert!(!info.api_version.is_empty());
        assert!(info.max_shared_memory > 0);
    }

    #[test]
    fn vulkan_buffer_create_upload_download() {
        let vk = match VulkanCompute::new() {
            Ok(v) => v,
            Err(_) => return,
        };
        let data = vec![1.0f32, 2.0, 3.0, 4.0];
        let buf = vk.create_buffer(data.len() * 4).unwrap();
        vk.upload_f32(&buf, &data).unwrap();
        let result = vk.download_f32(&buf, data.len()).unwrap();
        assert_eq!(result, data);
        vk.destroy_buffer(buf);
    }

    #[test]
    fn vulkan_tensor_add() {
        let vk = match VulkanCompute::new() {
            Ok(v) => v,
            Err(_) => return,
        };
        let a = vec![1.0f32, 2.0, 3.0, 4.0];
        let b = vec![10.0f32, 20.0, 30.0, 40.0];
        let result = vk.tensor_add(&a, &b).unwrap();
        assert_eq!(result.len(), 4);
        for (i, &v) in result.iter().enumerate() {
            let expected = a[i] + b[i];
            assert!((v - expected).abs() < 1e-5, "add[{i}]: {v} != {expected}");
        }
    }

    #[test]
    fn vulkan_tensor_mul() {
        let vk = match VulkanCompute::new() {
            Ok(v) => v,
            Err(_) => return,
        };
        let a = vec![2.0f32, 3.0, 4.0, 5.0];
        let b = vec![10.0f32, 10.0, 10.0, 10.0];
        let result = vk.tensor_mul(&a, &b).unwrap();
        assert_eq!(result, vec![20.0, 30.0, 40.0, 50.0]);
    }

    #[test]
    fn vulkan_tensor_sub() {
        let vk = match VulkanCompute::new() {
            Ok(v) => v,
            Err(_) => return,
        };
        let a = vec![10.0f32, 20.0, 30.0];
        let b = vec![1.0f32, 2.0, 3.0];
        let result = vk.tensor_sub(&a, &b).unwrap();
        assert_eq!(result, vec![9.0, 18.0, 27.0]);
    }

    #[test]
    fn vulkan_tensor_relu() {
        let vk = match VulkanCompute::new() {
            Ok(v) => v,
            Err(_) => return,
        };
        let x = vec![-3.0f32, -1.0, 0.0, 1.0, 5.0];
        let result = vk.tensor_relu(&x).unwrap();
        assert_eq!(result, vec![0.0, 0.0, 0.0, 1.0, 5.0]);
    }

    #[test]
    fn vulkan_tensor_sigmoid() {
        let vk = match VulkanCompute::new() {
            Ok(v) => v,
            Err(_) => return,
        };
        let x = vec![0.0f32, 1.0, -1.0];
        let result = vk.tensor_sigmoid(&x).unwrap();
        assert!((result[0] - 0.5).abs() < 1e-4, "sigmoid(0) = 0.5");
        assert!(result[1] > 0.7 && result[1] < 0.75, "sigmoid(1) ≈ 0.731");
        assert!(result[2] > 0.25 && result[2] < 0.3, "sigmoid(-1) ≈ 0.269");
    }

    #[test]
    fn vulkan_tensor_matmul_2x2() {
        let vk = match VulkanCompute::new() {
            Ok(v) => v,
            Err(_) => return,
        };
        // A = [[1,2],[3,4]], B = [[5,6],[7,8]]
        // C = [[19,22],[43,50]]
        let a = vec![1.0f32, 2.0, 3.0, 4.0];
        let b = vec![5.0f32, 6.0, 7.0, 8.0];
        let result = vk.tensor_matmul(&a, &b, 2, 2, 2).unwrap();
        assert_eq!(result.len(), 4);
        assert!((result[0] - 19.0).abs() < 1e-4);
        assert!((result[1] - 22.0).abs() < 1e-4);
        assert!((result[2] - 43.0).abs() < 1e-4);
        assert!((result[3] - 50.0).abs() < 1e-4);
    }

    #[test]
    fn vulkan_shape_mismatch_error() {
        let vk = match VulkanCompute::new() {
            Ok(v) => v,
            Err(_) => return,
        };
        let a = vec![1.0f32, 2.0, 3.0];
        let b = vec![1.0f32, 2.0];
        let err = vk.tensor_add(&a, &b).unwrap_err();
        assert!(matches!(err, VulkanError::ShapeMismatch(_)));
    }

    #[test]
    fn vulkan_large_tensor() {
        let vk = match VulkanCompute::new() {
            Ok(v) => v,
            Err(_) => return,
        };
        let n = 1024;
        let a: Vec<f32> = (0..n).map(|i| i as f32).collect();
        let b: Vec<f32> = (0..n).map(|i| (n - i) as f32).collect();
        let result = vk.tensor_add(&a, &b).unwrap();
        assert_eq!(result.len(), n);
        for (i, &v) in result.iter().enumerate() {
            assert!((v - n as f32).abs() < 1e-3, "large add[{i}]: {v} != {n}");
        }
    }

    // ─── Item 4: Real GPU hardware detection + compute ──────────────

    #[test]
    fn vulkan_detect_discrete_gpu() {
        // Should prefer discrete GPU (RTX 4090) over integrated (Intel)
        let vk = match VulkanCompute::new() {
            Ok(v) => v,
            Err(_) => return, // Skip if no Vulkan
        };
        let name = vk.device_name().to_string();
        let info = vk.device_info();
        eprintln!("Detected GPU: {} (type: {})", name, info.device_type);
        eprintln!(
            "  Vulkan {}, subgroup={}",
            info.api_version, info.subgroup_size
        );
        // On systems with discrete GPU, it should NOT select Intel integrated
        if name.contains("Intel") {
            // This is still OK on systems without discrete GPU
            eprintln!("  Note: no discrete GPU found, using integrated");
        }
        assert!(VulkanCompute::is_available());
    }

    #[test]
    fn vulkan_matmul_correctness() {
        let vk = match VulkanCompute::new() {
            Ok(v) => v,
            Err(_) => return,
        };
        // 4x3 × 3x2 = 4x2 matmul
        let a = vec![
            1.0f32, 2.0, 3.0, // row 0
            4.0, 5.0, 6.0, // row 1
            7.0, 8.0, 9.0, // row 2
            10.0, 11.0, 12.0, // row 3
        ];
        let b = vec![
            1.0f32, 2.0, // row 0
            3.0, 4.0, // row 1
            5.0, 6.0, // row 2
        ];
        // Expected: A(4x3) × B(3x2) = C(4x2)
        // C[0,0] = 1*1 + 2*3 + 3*5 = 22, C[0,1] = 1*2 + 2*4 + 3*6 = 28
        // C[1,0] = 4*1 + 5*3 + 6*5 = 49, C[1,1] = 4*2 + 5*4 + 6*6 = 64
        // C[2,0] = 7*1 + 8*3 + 9*5 = 76, C[2,1] = 7*2 + 8*4 + 9*6 = 100
        // C[3,0] = 10*1+11*3+12*5 = 103, C[3,1] = 10*2+11*4+12*6 = 136
        let result = vk.tensor_matmul(&a, &b, 4, 3, 2).unwrap();
        assert_eq!(result.len(), 8);
        let expected = [22.0f32, 28.0, 49.0, 64.0, 76.0, 100.0, 103.0, 136.0];
        for (i, (&got, &exp)) in result.iter().zip(expected.iter()).enumerate() {
            assert!(
                (got - exp).abs() < 1e-2,
                "matmul[{i}]: got {got}, expected {exp}"
            );
        }
    }

    #[test]
    fn vulkan_relu_correctness() {
        let vk = match VulkanCompute::new() {
            Ok(v) => v,
            Err(_) => return,
        };
        let input = vec![-3.0f32, -1.0, 0.0, 1.0, 5.0, -0.5];
        let result = vk.tensor_relu(&input).unwrap();
        let expected = [0.0f32, 0.0, 0.0, 1.0, 5.0, 0.0];
        for (i, (&got, &exp)) in result.iter().zip(expected.iter()).enumerate() {
            assert!(
                (got - exp).abs() < 1e-4,
                "relu[{i}]: got {got}, expected {exp}"
            );
        }
    }

    #[test]
    fn vulkan_sigmoid_correctness() {
        let vk = match VulkanCompute::new() {
            Ok(v) => v,
            Err(_) => return,
        };
        let input = vec![0.0f32, 1.0, -1.0, 10.0, -10.0];
        let result = vk.tensor_sigmoid(&input).unwrap();
        // sigmoid(0) = 0.5, sigmoid(1) ≈ 0.731, sigmoid(-1) ≈ 0.269
        assert!((result[0] - 0.5).abs() < 1e-2);
        assert!((result[1] - 0.731).abs() < 1e-2);
        assert!((result[2] - 0.269).abs() < 1e-2);
        assert!(result[3] > 0.99); // sigmoid(10) ≈ 1
        assert!(result[4] < 0.01); // sigmoid(-10) ≈ 0
    }

    #[test]
    fn vulkan_matmul_256x256_benchmark() {
        let vk = match VulkanCompute::new() {
            Ok(v) => v,
            Err(_) => return,
        };
        let n = 256;
        let a: Vec<f32> = (0..n * n).map(|i| (i % 17) as f32 * 0.1).collect();
        let b: Vec<f32> = (0..n * n).map(|i| (i % 13) as f32 * 0.1).collect();

        let start = std::time::Instant::now();
        let result = vk
            .tensor_matmul(&a, &b, n as u32, n as u32, n as u32)
            .unwrap();
        let gpu_time = start.elapsed();

        assert_eq!(result.len(), n * n);
        eprintln!(
            "GPU matmul {n}x{n}: {:.2}ms ({})",
            gpu_time.as_secs_f64() * 1000.0,
            vk.device_name()
        );

        // CPU reference (naive)
        let cpu_start = std::time::Instant::now();
        let mut cpu_result = vec![0.0f32; n * n];
        for row in 0..n {
            for col in 0..n {
                let mut sum = 0.0f32;
                for k in 0..n {
                    sum += a[row * n + k] * b[k * n + col];
                }
                cpu_result[row * n + col] = sum;
            }
        }
        let cpu_time = cpu_start.elapsed();
        eprintln!(
            "CPU matmul {n}x{n}: {:.2}ms",
            cpu_time.as_secs_f64() * 1000.0
        );

        if cpu_time > gpu_time {
            eprintln!(
                "GPU speedup: {:.1}x",
                cpu_time.as_secs_f64() / gpu_time.as_secs_f64()
            );
        }

        // Verify correctness (spot-check first 10 elements)
        for i in 0..10.min(result.len()) {
            assert!(
                (result[i] - cpu_result[i]).abs() < 1.0,
                "matmul[{i}]: GPU={} vs CPU={}",
                result[i],
                cpu_result[i]
            );
        }
    }
}
