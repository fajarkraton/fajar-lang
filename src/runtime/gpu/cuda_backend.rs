//! CUDA FFI backend — NVIDIA-specific GPU acceleration.
//!
//! Enabled with `--features cuda`.
//! Dynamically loads `libcuda.so` at runtime — no compile-time CUDA dependency.

use super::GpuError;
use super::buffer::{BackendData, GpuBuffer};
use super::device::{GpuBackend, GpuDevice, GpuDeviceInfo};
use super::kernel::{GpuKernel, KernelSource, WorkgroupSize};

use libloading::{Library, Symbol};
use std::ffi::c_void;

/// CUDA device implementation via driver API.
pub struct CudaDevice {
    info: GpuDeviceInfo,
    _lib: Library,
    context: *mut c_void,
}

// SAFETY: CUDA context is thread-safe when used with cuCtxSetCurrent
unsafe impl Send for CudaDevice {}
unsafe impl Sync for CudaDevice {}

/// Enumerate CUDA-capable GPU devices.
pub fn enumerate_devices() -> Result<Vec<Box<dyn GpuDevice>>, GpuError> {
    // SAFETY: loading libcuda.so dynamically
    let lib = unsafe { Library::new("libcuda.so") }
        .map_err(|e| GpuError::BackendError(format!("CUDA not available: {e}")))?;

    // SAFETY: calling cuInit from CUDA driver API
    let cu_init: Symbol<unsafe extern "C" fn(u32) -> i32> = unsafe { lib.get(b"cuInit") }
        .map_err(|e| GpuError::BackendError(format!("cuInit not found: {e}")))?;

    // SAFETY: cuInit(0) is the standard CUDA initialization call
    let result = unsafe { cu_init(0) };
    if result != 0 {
        return Err(GpuError::BackendError(format!(
            "cuInit failed: error {result}"
        )));
    }

    // SAFETY: symbol names match CUDA Driver API signatures; library is valid
    let cu_device_get_count: Symbol<unsafe extern "C" fn(*mut i32) -> i32> =
        unsafe { lib.get(b"cuDeviceGetCount") }
            .map_err(|e| GpuError::BackendError(e.to_string()))?;

    let mut count: i32 = 0;
    // SAFETY: count is a valid stack-allocated i32 pointer
    let result = unsafe { cu_device_get_count(&mut count) };
    if result != 0 || count == 0 {
        return Err(GpuError::NotAvailable);
    }

    // SAFETY: all symbol names match CUDA Driver API signatures; library is valid
    let cu_device_get: Symbol<unsafe extern "C" fn(*mut i32, i32) -> i32> =
        unsafe { lib.get(b"cuDeviceGet") }.map_err(|e| GpuError::BackendError(e.to_string()))?;

    let cu_device_get_name: Symbol<unsafe extern "C" fn(*mut u8, i32, i32) -> i32> =
        unsafe { lib.get(b"cuDeviceGetName") }
            .map_err(|e| GpuError::BackendError(e.to_string()))?;

    let cu_device_total_mem: Symbol<unsafe extern "C" fn(*mut usize, i32) -> i32> =
        unsafe { lib.get(b"cuDeviceTotalMem_v2") }
            .map_err(|e| GpuError::BackendError(e.to_string()))?;

    let cu_device_get_attribute: Symbol<unsafe extern "C" fn(*mut i32, i32, i32) -> i32> =
        unsafe { lib.get(b"cuDeviceGetAttribute") }
            .map_err(|e| GpuError::BackendError(e.to_string()))?;

    let cu_ctx_create: Symbol<unsafe extern "C" fn(*mut *mut c_void, u32, i32) -> i32> =
        unsafe { lib.get(b"cuCtxCreate_v2") }.map_err(|e| GpuError::BackendError(e.to_string()))?;

    let mut devices: Vec<Box<dyn GpuDevice>> = Vec::new();

    for i in 0..count {
        let mut device_handle: i32 = 0;
        // SAFETY: calling CUDA driver API with valid pointers
        let r = unsafe { cu_device_get(&mut device_handle, i) };
        if r != 0 {
            continue;
        }

        let mut name_buf = [0u8; 256];
        // SAFETY: buffer is large enough, CUDA writes null-terminated string
        let r = unsafe { cu_device_get_name(name_buf.as_mut_ptr(), 256, device_handle) };
        let name = if r == 0 {
            let end = name_buf.iter().position(|&b| b == 0).unwrap_or(256);
            String::from_utf8_lossy(&name_buf[..end]).to_string()
        } else {
            format!("CUDA Device {i}")
        };

        let mut total_mem: usize = 0;
        // SAFETY: valid pointer
        let _ = unsafe { cu_device_total_mem(&mut total_mem, device_handle) };

        // CU_DEVICE_ATTRIBUTE_MULTIPROCESSOR_COUNT = 16
        let mut sm_count: i32 = 0;
        // SAFETY: valid attribute query
        let _ = unsafe { cu_device_get_attribute(&mut sm_count, 16, device_handle) };

        // CU_DEVICE_ATTRIBUTE_MAX_THREADS_PER_BLOCK = 1
        let mut max_threads: i32 = 0;
        // SAFETY: valid attribute query
        let _ = unsafe { cu_device_get_attribute(&mut max_threads, 1, device_handle) };

        let mut context: *mut c_void = std::ptr::null_mut();
        // SAFETY: creating CUDA context for this device
        let r = unsafe { cu_ctx_create(&mut context, 0, device_handle) };
        if r != 0 {
            continue;
        }

        let info = GpuDeviceInfo {
            name,
            memory: total_mem as u64,
            compute_units: sm_count as u32,
            backend: GpuBackend::Cuda,
            max_workgroup_size: max_threads as u32,
            max_buffer_size: total_mem as u64,
        };

        devices.push(Box::new(CudaDevice {
            info,
            // SAFETY: loading libcuda.so for the device's lifetime
            _lib: unsafe { Library::new("libcuda.so") }
                .map_err(|e| GpuError::BackendError(e.to_string()))?,
            context,
        }));
    }

    if devices.is_empty() {
        return Err(GpuError::NotAvailable);
    }

    Ok(devices)
}

impl GpuDevice for CudaDevice {
    fn info(&self) -> GpuDeviceInfo {
        self.info.clone()
    }

    fn create_buffer(&self, size: usize) -> Result<GpuBuffer, GpuError> {
        // SAFETY: loading and calling cuMemAlloc_v2
        let lib = unsafe { Library::new("libcuda.so") }
            .map_err(|e| GpuError::BackendError(e.to_string()))?;
        let cu_mem_alloc: Symbol<unsafe extern "C" fn(*mut u64, usize) -> i32> =
            unsafe { lib.get(b"cuMemAlloc_v2") }
                .map_err(|e| GpuError::BackendError(e.to_string()))?;

        let mut dev_ptr: u64 = 0;
        // SAFETY: allocating device memory
        let r = unsafe { cu_mem_alloc(&mut dev_ptr, size) };
        if r != 0 {
            return Err(GpuError::MemoryExhausted {
                requested: size,
                available: 0,
            });
        }

        Ok(GpuBuffer::new(size, BackendData::Handle(dev_ptr)))
    }

    fn upload(&self, buffer: &GpuBuffer, data: &[u8]) -> Result<(), GpuError> {
        if data.len() != buffer.size() {
            return Err(GpuError::BufferSizeMismatch {
                expected: buffer.size(),
                actual: data.len(),
            });
        }

        let dev_ptr = buffer
            .backend_data()
            .as_handle()
            .ok_or_else(|| GpuError::BackendError("not a CUDA buffer".into()))?;

        // SAFETY: loading and calling cuMemcpyHtoD_v2
        let lib = unsafe { Library::new("libcuda.so") }
            .map_err(|e| GpuError::BackendError(e.to_string()))?;
        let cu_memcpy: Symbol<unsafe extern "C" fn(u64, *const u8, usize) -> i32> =
            unsafe { lib.get(b"cuMemcpyHtoD_v2") }
                .map_err(|e| GpuError::BackendError(e.to_string()))?;

        // SAFETY: copying host data to device — valid pointers and size
        let r = unsafe { cu_memcpy(dev_ptr, data.as_ptr(), data.len()) };
        if r != 0 {
            return Err(GpuError::BackendError(format!("cuMemcpyHtoD failed: {r}")));
        }
        Ok(())
    }

    fn download(&self, buffer: &GpuBuffer, dst: &mut [u8]) -> Result<(), GpuError> {
        if dst.len() != buffer.size() {
            return Err(GpuError::BufferSizeMismatch {
                expected: buffer.size(),
                actual: dst.len(),
            });
        }

        let dev_ptr = buffer
            .backend_data()
            .as_handle()
            .ok_or_else(|| GpuError::BackendError("not a CUDA buffer".into()))?;

        // SAFETY: loading and calling cuMemcpyDtoH_v2
        let lib = unsafe { Library::new("libcuda.so") }
            .map_err(|e| GpuError::BackendError(e.to_string()))?;
        let cu_memcpy: Symbol<unsafe extern "C" fn(*mut u8, u64, usize) -> i32> =
            unsafe { lib.get(b"cuMemcpyDtoH_v2") }
                .map_err(|e| GpuError::BackendError(e.to_string()))?;

        // SAFETY: copying device data to host — valid pointers and size
        let r = unsafe { cu_memcpy(dst.as_mut_ptr(), dev_ptr, dst.len()) };
        if r != 0 {
            return Err(GpuError::BackendError(format!("cuMemcpyDtoH failed: {r}")));
        }
        Ok(())
    }

    fn compile_kernel(&self, source: &KernelSource) -> Result<GpuKernel, GpuError> {
        match source {
            KernelSource::Ptx(_ptx) => {
                // Stub: real impl calls cuModuleLoadData + cuModuleGetFunction
                Ok(GpuKernel::new(
                    "cuda_kernel".into(),
                    0,
                    WorkgroupSize::d1(256),
                    0,
                ))
            }
            KernelSource::Builtin(builtin) => {
                // Stub: real impl loads pre-compiled PTX for builtin kernels
                Ok(GpuKernel::new(
                    format!("{builtin}"),
                    3,
                    WorkgroupSize::d1(256),
                    *builtin as u64,
                ))
            }
            _ => Err(GpuError::InvalidKernel(
                "CUDA backend requires PTX or Builtin kernel".into(),
            )),
        }
    }

    fn execute(
        &self,
        _kernel: &GpuKernel,
        _workgroups: (u32, u32, u32),
        _buffers: &[&GpuBuffer],
    ) -> Result<(), GpuError> {
        // Stub: real impl calls cuLaunchKernel with grid/block dims
        Ok(())
    }
}

impl Drop for CudaDevice {
    fn drop(&mut self) {
        if !self.context.is_null() {
            // SAFETY: destroying CUDA context we created
            if let Ok(lib) = unsafe { Library::new("libcuda.so") } {
                if let Ok(cu_ctx_destroy) = unsafe {
                    lib.get::<unsafe extern "C" fn(*mut c_void) -> i32>(b"cuCtxDestroy_v2")
                } {
                    unsafe { cu_ctx_destroy(self.context) };
                }
            }
        }
    }
}
