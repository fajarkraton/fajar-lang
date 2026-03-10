//! GPU tensor bridge — transfer tensors between CPU (ndarray) and GPU.
//!
//! Connects the ML runtime's [`TensorValue`] to GPU compute via
//! the [`GpuDevice`] abstraction layer.

use super::buffer::GpuBuffer;
use super::device::GpuDevice;
use super::kernel::{BuiltinKernel, KernelSource};
use super::GpuError;

use crate::runtime::ml::tensor::TensorValue;
use ndarray::ArrayD;

/// A tensor residing on a GPU device.
pub struct GpuTensor {
    /// The GPU buffer containing tensor data (f32 format).
    buffer: GpuBuffer,
    /// Shape of the tensor.
    shape: Vec<usize>,
    /// Total number of elements.
    len: usize,
}

impl GpuTensor {
    /// Get the tensor shape.
    pub fn shape(&self) -> &[usize] {
        &self.shape
    }

    /// Get the total number of elements.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Check if the tensor is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Get a reference to the underlying GPU buffer.
    pub fn buffer(&self) -> &GpuBuffer {
        &self.buffer
    }
}

/// Upload a CPU TensorValue to GPU, returning a GpuTensor.
///
/// Converts f64 -> f32 for GPU computation.
pub fn tensor_to_gpu(device: &dyn GpuDevice, tensor: &TensorValue) -> Result<GpuTensor, GpuError> {
    let shape: Vec<usize> = tensor.shape().to_vec();
    let len = tensor.data().len();
    let byte_size = len * std::mem::size_of::<f32>();

    // Convert f64 -> f32 for GPU
    let f32_data: Vec<f32> = tensor.data().iter().map(|&v| v as f32).collect();
    let bytes: Vec<u8> = f32_data.iter().flat_map(|f| f.to_le_bytes()).collect();

    let buffer = device.create_buffer(byte_size)?;
    device.upload(&buffer, &bytes)?;

    Ok(GpuTensor { buffer, shape, len })
}

/// Download a GpuTensor back to CPU as a TensorValue.
///
/// Converts f32 -> f64.
pub fn tensor_to_cpu(
    device: &dyn GpuDevice,
    gpu_tensor: &GpuTensor,
) -> Result<TensorValue, GpuError> {
    let byte_size = gpu_tensor.len * std::mem::size_of::<f32>();
    let mut bytes = vec![0u8; byte_size];
    device.download(&gpu_tensor.buffer, &mut bytes)?;

    // Convert f32 -> f64
    let f64_data: Vec<f64> = bytes
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]) as f64)
        .collect();

    let shape = gpu_tensor.shape.clone();
    let array = ArrayD::from_shape_vec(shape, f64_data)
        .map_err(|e| GpuError::ShapeMismatch(format!("failed to reshape downloaded data: {e}")))?;

    Ok(TensorValue::from_ndarray(array))
}

/// Execute GPU matmul: C = A @ B.
///
/// A has shape [M, K], B has shape [K, N], result has shape [M, N].
pub fn gpu_matmul(
    device: &dyn GpuDevice,
    a: &GpuTensor,
    b: &GpuTensor,
) -> Result<GpuTensor, GpuError> {
    if a.shape.len() != 2 || b.shape.len() != 2 {
        return Err(GpuError::ShapeMismatch("matmul requires 2D tensors".into()));
    }
    let k_a = a.shape[1];
    let k_b = b.shape[0];
    if k_a != k_b {
        return Err(GpuError::ShapeMismatch(format!(
            "matmul inner dimension mismatch: {k_a} != {k_b}"
        )));
    }

    let m = a.shape[0];
    let n = b.shape[1];
    let out_len = m * n;
    let out_bytes = out_len * std::mem::size_of::<f32>();

    let out_buffer = device.create_buffer(out_bytes)?;
    let kernel = device.compile_kernel(&KernelSource::Builtin(BuiltinKernel::Matmul))?;

    // Dispatch with enough workgroups to cover output
    let wg_x = m.div_ceil(16) as u32;
    let wg_y = n.div_ceil(16) as u32;
    device.execute(
        &kernel,
        (wg_x, wg_y, 1),
        &[&a.buffer, &b.buffer, &out_buffer],
    )?;

    Ok(GpuTensor {
        buffer: out_buffer,
        shape: vec![m, n],
        len: out_len,
    })
}

/// Execute GPU element-wise operation.
fn gpu_elementwise(
    device: &dyn GpuDevice,
    a: &GpuTensor,
    b: &GpuTensor,
    op: BuiltinKernel,
) -> Result<GpuTensor, GpuError> {
    if a.shape != b.shape {
        return Err(GpuError::ShapeMismatch(format!(
            "elementwise shape mismatch: {:?} vs {:?}",
            a.shape, b.shape
        )));
    }

    let out_bytes = a.len * std::mem::size_of::<f32>();
    let out_buffer = device.create_buffer(out_bytes)?;
    let kernel = device.compile_kernel(&KernelSource::Builtin(op))?;

    let wg = a.len.div_ceil(256) as u32;
    device.execute(&kernel, (wg, 1, 1), &[&a.buffer, &b.buffer, &out_buffer])?;

    Ok(GpuTensor {
        buffer: out_buffer,
        shape: a.shape.clone(),
        len: a.len,
    })
}

/// GPU element-wise addition.
pub fn gpu_add(
    device: &dyn GpuDevice,
    a: &GpuTensor,
    b: &GpuTensor,
) -> Result<GpuTensor, GpuError> {
    gpu_elementwise(device, a, b, BuiltinKernel::VectorAdd)
}

/// GPU element-wise subtraction.
pub fn gpu_sub(
    device: &dyn GpuDevice,
    a: &GpuTensor,
    b: &GpuTensor,
) -> Result<GpuTensor, GpuError> {
    gpu_elementwise(device, a, b, BuiltinKernel::VectorSub)
}

/// GPU element-wise multiplication.
pub fn gpu_mul(
    device: &dyn GpuDevice,
    a: &GpuTensor,
    b: &GpuTensor,
) -> Result<GpuTensor, GpuError> {
    gpu_elementwise(device, a, b, BuiltinKernel::VectorMul)
}

/// GPU element-wise division.
pub fn gpu_div(
    device: &dyn GpuDevice,
    a: &GpuTensor,
    b: &GpuTensor,
) -> Result<GpuTensor, GpuError> {
    gpu_elementwise(device, a, b, BuiltinKernel::VectorDiv)
}

/// Execute GPU unary activation.
fn gpu_activation(
    device: &dyn GpuDevice,
    input: &GpuTensor,
    op: BuiltinKernel,
) -> Result<GpuTensor, GpuError> {
    let out_bytes = input.len * std::mem::size_of::<f32>();
    let out_buffer = device.create_buffer(out_bytes)?;
    let kernel = device.compile_kernel(&KernelSource::Builtin(op))?;

    let wg = input.len.div_ceil(256) as u32;
    device.execute(&kernel, (wg, 1, 1), &[&input.buffer, &out_buffer])?;

    Ok(GpuTensor {
        buffer: out_buffer,
        shape: input.shape.clone(),
        len: input.len,
    })
}

/// GPU ReLU activation.
pub fn gpu_relu(device: &dyn GpuDevice, input: &GpuTensor) -> Result<GpuTensor, GpuError> {
    gpu_activation(device, input, BuiltinKernel::Relu)
}

/// GPU sigmoid activation.
pub fn gpu_sigmoid(device: &dyn GpuDevice, input: &GpuTensor) -> Result<GpuTensor, GpuError> {
    gpu_activation(device, input, BuiltinKernel::Sigmoid)
}

/// GPU softmax (element-wise exp — caller normalizes).
pub fn gpu_softmax(device: &dyn GpuDevice, input: &GpuTensor) -> Result<GpuTensor, GpuError> {
    gpu_activation(device, input, BuiltinKernel::Softmax)
}

/// Auto device selection: prefer GPU, fall back to CPU.
///
/// Returns the best available device.
pub fn auto_device() -> Box<dyn GpuDevice> {
    super::best_device()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::gpu::CpuFallbackDevice;

    fn make_tensor(shape: Vec<usize>, data: Vec<f64>) -> TensorValue {
        let array = ArrayD::from_shape_vec(shape, data).expect("valid shape");
        TensorValue::from_ndarray(array)
    }

    #[test]
    fn tensor_to_gpu_and_back() {
        let dev = CpuFallbackDevice::new();
        let t = make_tensor(vec![2, 3], vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);

        let gpu_t = tensor_to_gpu(&dev, &t).expect("to_gpu");
        assert_eq!(gpu_t.shape(), &[2, 3]);
        assert_eq!(gpu_t.len(), 6);

        let cpu_t = tensor_to_cpu(&dev, &gpu_t).expect("to_cpu");
        let expected: Vec<f64> = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        for (a, b) in cpu_t.data().iter().zip(expected.iter()) {
            assert!((a - b).abs() < 1e-5, "mismatch: {a} vs {b}");
        }
    }

    #[test]
    fn gpu_relu_on_cpu_fallback() {
        let dev = CpuFallbackDevice::new();
        let t = make_tensor(vec![4], vec![-2.0, -1.0, 0.0, 3.0]);

        let gpu_t = tensor_to_gpu(&dev, &t).expect("to_gpu");
        let result = gpu_relu(&dev, &gpu_t).expect("relu");
        let cpu_result = tensor_to_cpu(&dev, &result).expect("to_cpu");

        let expected = vec![0.0, 0.0, 0.0, 3.0];
        for (a, b) in cpu_result.data().iter().zip(expected.iter()) {
            assert!((a - b).abs() < 1e-5);
        }
    }

    #[test]
    fn gpu_sigmoid_on_cpu_fallback() {
        let dev = CpuFallbackDevice::new();
        let t = make_tensor(vec![1], vec![0.0]);

        let gpu_t = tensor_to_gpu(&dev, &t).expect("to_gpu");
        let result = gpu_sigmoid(&dev, &gpu_t).expect("sigmoid");
        let cpu_result = tensor_to_cpu(&dev, &result).expect("to_cpu");

        assert!((cpu_result.data()[0] - 0.5).abs() < 1e-5);
    }

    #[test]
    fn gpu_vector_add_on_cpu_fallback() {
        let dev = CpuFallbackDevice::new();
        let a = make_tensor(vec![3], vec![1.0, 2.0, 3.0]);
        let b = make_tensor(vec![3], vec![10.0, 20.0, 30.0]);

        let ga = tensor_to_gpu(&dev, &a).expect("a to gpu");
        let gb = tensor_to_gpu(&dev, &b).expect("b to gpu");
        let result = gpu_add(&dev, &ga, &gb).expect("add");
        let cpu_result = tensor_to_cpu(&dev, &result).expect("to_cpu");

        let expected = vec![11.0, 22.0, 33.0];
        for (a, b) in cpu_result.data().iter().zip(expected.iter()) {
            assert!((a - b).abs() < 1e-5);
        }
    }

    #[test]
    fn gpu_elementwise_shape_mismatch() {
        let dev = CpuFallbackDevice::new();
        let a = make_tensor(vec![3], vec![1.0, 2.0, 3.0]);
        let b = make_tensor(vec![4], vec![1.0, 2.0, 3.0, 4.0]);

        let ga = tensor_to_gpu(&dev, &a).expect("a to gpu");
        let gb = tensor_to_gpu(&dev, &b).expect("b to gpu");
        let result = gpu_add(&dev, &ga, &gb);
        assert!(result.is_err());
    }

    #[test]
    fn auto_device_returns_something() {
        let dev = auto_device();
        assert!(!dev.info().name.is_empty());
    }

    #[test]
    fn gpu_tensor_empty_check() {
        let dev = CpuFallbackDevice::new();
        let t = make_tensor(vec![0], vec![]);
        let gpu_t = tensor_to_gpu(&dev, &t).expect("to_gpu");
        assert!(gpu_t.is_empty());
        assert_eq!(gpu_t.len(), 0);
    }
}
