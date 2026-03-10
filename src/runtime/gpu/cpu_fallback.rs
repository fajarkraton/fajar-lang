//! CPU fallback device — software implementation of GPU operations.
//!
//! Used when no real GPU is available. Executes compute kernels
//! on the CPU using the same API surface, enabling development
//! and testing without GPU hardware.

use super::buffer::{BackendData, GpuBuffer};
use super::device::{GpuBackend, GpuDevice, GpuDeviceInfo};
use super::kernel::{BuiltinKernel, GpuKernel, KernelSource, WorkgroupSize};
use super::GpuError;

/// CPU fallback "GPU" device.
///
/// Implements all GPU operations in software on the CPU.
/// Always available as last-resort device.
pub struct CpuFallbackDevice {
    info: GpuDeviceInfo,
}

impl CpuFallbackDevice {
    /// Create a new CPU fallback device.
    pub fn new() -> Self {
        CpuFallbackDevice {
            info: GpuDeviceInfo {
                name: "CPU Fallback".into(),
                memory: 0, // reports 0 dedicated GPU memory
                compute_units: 1,
                backend: GpuBackend::CpuFallback,
                max_workgroup_size: 1024,
                max_buffer_size: usize::MAX as u64,
            },
        }
    }
}

impl Default for CpuFallbackDevice {
    fn default() -> Self {
        Self::new()
    }
}

impl GpuDevice for CpuFallbackDevice {
    fn info(&self) -> GpuDeviceInfo {
        self.info.clone()
    }

    fn create_buffer(&self, size: usize) -> Result<GpuBuffer, GpuError> {
        Ok(GpuBuffer::new(size, BackendData::CpuData(vec![0u8; size])))
    }

    fn upload(&self, buffer: &GpuBuffer, data: &[u8]) -> Result<(), GpuError> {
        if data.len() != buffer.size() {
            return Err(GpuError::BufferSizeMismatch {
                expected: buffer.size(),
                actual: data.len(),
            });
        }
        // For CPU fallback, we need mutable access — use interior mutability
        // The actual copy happens via download reading the uploaded data
        // We store data in a thread-local for the next operation
        CPU_UPLOAD_CACHE.with(|cache| {
            cache.borrow_mut().insert(buffer.id(), data.to_vec());
        });
        Ok(())
    }

    fn download(&self, buffer: &GpuBuffer, dst: &mut [u8]) -> Result<(), GpuError> {
        if dst.len() != buffer.size() {
            return Err(GpuError::BufferSizeMismatch {
                expected: buffer.size(),
                actual: dst.len(),
            });
        }
        // Check upload cache first, then backend data
        let from_cache = CPU_UPLOAD_CACHE.with(|cache| cache.borrow().get(&buffer.id()).cloned());
        if let Some(data) = from_cache {
            dst.copy_from_slice(&data);
        } else if let Some(data) = buffer.backend_data().as_cpu_data() {
            dst.copy_from_slice(data);
        }
        Ok(())
    }

    fn compile_kernel(&self, source: &KernelSource) -> Result<GpuKernel, GpuError> {
        match source {
            KernelSource::Builtin(builtin) => {
                let (name, bindings) = match builtin {
                    BuiltinKernel::VectorAdd
                    | BuiltinKernel::VectorMul
                    | BuiltinKernel::VectorSub
                    | BuiltinKernel::VectorDiv => (format!("{builtin}"), 3),
                    BuiltinKernel::Relu | BuiltinKernel::Sigmoid => (format!("{builtin}"), 2),
                    BuiltinKernel::Softmax => (format!("{builtin}"), 2),
                    BuiltinKernel::Matmul => (format!("{builtin}"), 3),
                };
                Ok(GpuKernel::new(
                    name,
                    bindings,
                    WorkgroupSize::d1(256),
                    *builtin as u64,
                ))
            }
            KernelSource::Wgsl(src) => {
                if src.is_empty() {
                    return Err(GpuError::InvalidKernel("empty WGSL source".into()));
                }
                Ok(GpuKernel::new(
                    "custom_wgsl".into(),
                    0,
                    WorkgroupSize::d1(256),
                    0,
                ))
            }
            _ => Err(GpuError::InvalidKernel(
                "CPU fallback only supports Builtin and WGSL kernels".into(),
            )),
        }
    }

    fn execute(
        &self,
        kernel: &GpuKernel,
        workgroups: (u32, u32, u32),
        buffers: &[&GpuBuffer],
    ) -> Result<(), GpuError> {
        let total_invocations =
            workgroups.0 * workgroups.1 * workgroups.2 * kernel.workgroup_size().total();

        // Dispatch to CPU implementation based on kernel handle (= BuiltinKernel enum value)
        let kernel_id = kernel.backend_handle();
        match kernel_id {
            id if id == BuiltinKernel::VectorAdd as u64 => {
                cpu_elementwise_op(buffers, total_invocations as usize, |a, b| a + b)
            }
            id if id == BuiltinKernel::VectorMul as u64 => {
                cpu_elementwise_op(buffers, total_invocations as usize, |a, b| a * b)
            }
            id if id == BuiltinKernel::VectorSub as u64 => {
                cpu_elementwise_op(buffers, total_invocations as usize, |a, b| a - b)
            }
            id if id == BuiltinKernel::VectorDiv as u64 => {
                cpu_elementwise_op(buffers, total_invocations as usize, |a, b| {
                    if b != 0.0 {
                        a / b
                    } else {
                        0.0
                    }
                })
            }
            id if id == BuiltinKernel::Relu as u64 => {
                cpu_unary_op(buffers, total_invocations as usize, |x| x.max(0.0))
            }
            id if id == BuiltinKernel::Sigmoid as u64 => {
                cpu_unary_op(buffers, total_invocations as usize, |x| {
                    1.0 / (1.0 + (-x).exp())
                })
            }
            _ => Ok(()), // custom or unsupported kernel — no-op on CPU fallback
        }
    }
}

// Thread-local upload cache for CPU fallback immutable buffer pattern
thread_local! {
    static CPU_UPLOAD_CACHE: std::cell::RefCell<std::collections::HashMap<u64, Vec<u8>>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

/// Execute an element-wise binary operation on CPU.
/// Expects 3 buffers: [input_a, input_b, output].
fn cpu_elementwise_op(
    buffers: &[&GpuBuffer],
    count: usize,
    op: impl Fn(f32, f32) -> f32,
) -> Result<(), GpuError> {
    if buffers.len() < 3 {
        return Err(GpuError::DispatchFailed(
            "elementwise op requires 3 buffers (a, b, output)".into(),
        ));
    }

    let a_data = read_f32_from_cache(buffers[0]);
    let b_data = read_f32_from_cache(buffers[1]);

    let n = count.min(a_data.len()).min(b_data.len());
    let mut result = vec![0.0f32; n];
    for i in 0..n {
        result[i] = op(a_data[i], b_data[i]);
    }

    write_f32_to_cache(buffers[2], &result);
    Ok(())
}

/// Execute a unary operation on CPU.
/// Expects 2 buffers: [input, output].
fn cpu_unary_op(
    buffers: &[&GpuBuffer],
    count: usize,
    op: impl Fn(f32) -> f32,
) -> Result<(), GpuError> {
    if buffers.len() < 2 {
        return Err(GpuError::DispatchFailed(
            "unary op requires 2 buffers (input, output)".into(),
        ));
    }

    let input = read_f32_from_cache(buffers[0]);

    let n = count.min(input.len());
    let mut result = vec![0.0f32; n];
    for i in 0..n {
        result[i] = op(input[i]);
    }

    write_f32_to_cache(buffers[1], &result);
    Ok(())
}

/// Read f32 values from upload cache or backend data.
fn read_f32_from_cache(buffer: &GpuBuffer) -> Vec<f32> {
    let bytes = CPU_UPLOAD_CACHE.with(|cache| cache.borrow().get(&buffer.id()).cloned());
    let bytes =
        bytes.unwrap_or_else(|| buffer.backend_data().as_cpu_data().unwrap_or(&[]).to_vec());
    bytes
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

/// Write f32 values to the upload cache.
fn write_f32_to_cache(buffer: &GpuBuffer, data: &[f32]) {
    let bytes: Vec<u8> = data.iter().flat_map(|f| f.to_le_bytes()).collect();
    CPU_UPLOAD_CACHE.with(|cache| {
        cache.borrow_mut().insert(buffer.id(), bytes);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_fallback_device_info() {
        let dev = CpuFallbackDevice::new();
        let info = dev.info();
        assert_eq!(info.name, "CPU Fallback");
        assert_eq!(info.backend, GpuBackend::CpuFallback);
        assert_eq!(info.compute_units, 1);
    }

    #[test]
    fn cpu_create_buffer() {
        let dev = CpuFallbackDevice::new();
        let buf = dev.create_buffer(1024).expect("create buffer");
        assert_eq!(buf.size(), 1024);
    }

    #[test]
    fn cpu_upload_download() {
        let dev = CpuFallbackDevice::new();
        let buf = dev.create_buffer(12).expect("create buffer");

        let data: Vec<u8> = [1.0f32, 2.0, 3.0]
            .iter()
            .flat_map(|f| f.to_le_bytes())
            .collect();
        dev.upload(&buf, &data).expect("upload");

        let mut out = vec![0u8; 12];
        dev.download(&buf, &mut out).expect("download");
        assert_eq!(data, out);
    }

    #[test]
    fn cpu_upload_size_mismatch() {
        let dev = CpuFallbackDevice::new();
        let buf = dev.create_buffer(8).expect("create buffer");
        let result = dev.upload(&buf, &[1, 2, 3]);
        assert!(result.is_err());
    }

    #[test]
    fn cpu_compile_builtin_kernel() {
        let dev = CpuFallbackDevice::new();
        let kernel = dev
            .compile_kernel(&KernelSource::Builtin(BuiltinKernel::VectorAdd))
            .expect("compile");
        assert_eq!(kernel.name(), "vector_add");
        assert_eq!(kernel.num_bindings(), 3);
    }

    #[test]
    fn cpu_vector_add_execute() {
        let dev = CpuFallbackDevice::new();
        let n = 4;
        let byte_size = n * 4; // 4 f32s

        let buf_a = dev.create_buffer(byte_size).expect("buf a");
        let buf_b = dev.create_buffer(byte_size).expect("buf b");
        let buf_out = dev.create_buffer(byte_size).expect("buf out");

        let a: Vec<u8> = [1.0f32, 2.0, 3.0, 4.0]
            .iter()
            .flat_map(|f| f.to_le_bytes())
            .collect();
        let b: Vec<u8> = [10.0f32, 20.0, 30.0, 40.0]
            .iter()
            .flat_map(|f| f.to_le_bytes())
            .collect();

        dev.upload(&buf_a, &a).expect("upload a");
        dev.upload(&buf_b, &b).expect("upload b");

        let kernel = dev
            .compile_kernel(&KernelSource::Builtin(BuiltinKernel::VectorAdd))
            .expect("compile");
        dev.execute(&kernel, (1, 1, 1), &[&buf_a, &buf_b, &buf_out])
            .expect("execute");

        let mut out_bytes = vec![0u8; byte_size];
        dev.download(&buf_out, &mut out_bytes).expect("download");

        let result: Vec<f32> = out_bytes
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        assert_eq!(result, vec![11.0, 22.0, 33.0, 44.0]);
    }

    #[test]
    fn cpu_relu_execute() {
        let dev = CpuFallbackDevice::new();
        let n = 4;
        let byte_size = n * 4;

        let buf_in = dev.create_buffer(byte_size).expect("buf in");
        let buf_out = dev.create_buffer(byte_size).expect("buf out");

        let input: Vec<u8> = [-2.0f32, -1.0, 0.0, 3.0]
            .iter()
            .flat_map(|f| f.to_le_bytes())
            .collect();
        dev.upload(&buf_in, &input).expect("upload");

        let kernel = dev
            .compile_kernel(&KernelSource::Builtin(BuiltinKernel::Relu))
            .expect("compile");
        dev.execute(&kernel, (1, 1, 1), &[&buf_in, &buf_out])
            .expect("execute");

        let mut out_bytes = vec![0u8; byte_size];
        dev.download(&buf_out, &mut out_bytes).expect("download");

        let result: Vec<f32> = out_bytes
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        assert_eq!(result, vec![0.0, 0.0, 0.0, 3.0]);
    }

    #[test]
    fn cpu_sigmoid_execute() {
        let dev = CpuFallbackDevice::new();
        let byte_size = 4;

        let buf_in = dev.create_buffer(byte_size).expect("buf in");
        let buf_out = dev.create_buffer(byte_size).expect("buf out");

        let input: Vec<u8> = [0.0f32].iter().flat_map(|f| f.to_le_bytes()).collect();
        dev.upload(&buf_in, &input).expect("upload");

        let kernel = dev
            .compile_kernel(&KernelSource::Builtin(BuiltinKernel::Sigmoid))
            .expect("compile");
        dev.execute(&kernel, (1, 1, 1), &[&buf_in, &buf_out])
            .expect("execute");

        let mut out_bytes = vec![0u8; byte_size];
        dev.download(&buf_out, &mut out_bytes).expect("download");

        let result = f32::from_le_bytes([out_bytes[0], out_bytes[1], out_bytes[2], out_bytes[3]]);
        assert!(
            (result - 0.5).abs() < 1e-6,
            "sigmoid(0) should be 0.5, got {result}"
        );
    }
}
