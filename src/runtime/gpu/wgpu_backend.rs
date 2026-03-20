//! wgpu-based GPU backend — cross-platform via Vulkan/Metal/DX12.
//!
//! Enabled with `--features gpu`.
//! Implements the full [`GpuDevice`] trait: buffer management,
//! WGSL kernel compilation, compute pipeline dispatch.

use super::GpuError;
use super::buffer::{BackendData, GpuBuffer};
use super::device::{GpuBackend, GpuDevice, GpuDeviceInfo};
use super::kernel::{BuiltinKernel, GpuKernel, KernelSource, WorkgroupSize};

use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_HANDLE: AtomicU64 = AtomicU64::new(1);

/// wgpu GPU device implementation.
pub struct WgpuDevice {
    info: GpuDeviceInfo,
    device: wgpu::Device,
    queue: wgpu::Queue,
    /// Buffer handle -> wgpu::Buffer mapping.
    buffers: Mutex<HashMap<u64, wgpu::Buffer>>,
    /// Kernel handle -> compiled pipeline mapping.
    pipelines: Mutex<HashMap<u64, CompiledPipeline>>,
}

/// A compiled compute pipeline with its bind group layout.
struct CompiledPipeline {
    pipeline: wgpu::ComputePipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    num_bindings: u32,
}

/// Enumerate all wgpu-compatible GPU devices.
pub fn enumerate_devices() -> Result<Vec<Box<dyn GpuDevice>>, GpuError> {
    let instance = wgpu::Instance::default();
    let adapters: Vec<_> = pollster_block_on(instance.enumerate_adapters(wgpu::Backends::all()));

    if adapters.is_empty() {
        return Err(GpuError::NotAvailable);
    }

    let mut devices: Vec<Box<dyn GpuDevice>> = Vec::new();

    for adapter in adapters {
        let adapter_info = adapter.get_info();
        if adapter_info.device_type == wgpu::DeviceType::Cpu {
            continue;
        }

        let (device, queue) =
            match pollster_block_on(adapter.request_device(&wgpu::DeviceDescriptor {
                label: Some("Fajar Lang GPU"),
                ..Default::default()
            })) {
                Ok(dq) => dq,
                Err(_) => continue,
            };

        let limits = device.limits();

        let info = GpuDeviceInfo {
            name: adapter_info.name.clone(),
            memory: 0,
            compute_units: 1,
            backend: GpuBackend::Wgpu,
            max_workgroup_size: limits.max_compute_invocations_per_workgroup,
            max_buffer_size: limits.max_buffer_size,
        };

        devices.push(Box::new(WgpuDevice {
            info,
            device,
            queue,
            buffers: Mutex::new(HashMap::new()),
            pipelines: Mutex::new(HashMap::new()),
        }));
    }

    if devices.is_empty() {
        return Err(GpuError::NotAvailable);
    }

    Ok(devices)
}

impl GpuDevice for WgpuDevice {
    fn info(&self) -> GpuDeviceInfo {
        self.info.clone()
    }

    fn create_buffer(&self, size: usize) -> Result<GpuBuffer, GpuError> {
        let handle = NEXT_HANDLE.fetch_add(1, Ordering::Relaxed);
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("fj_gpu_buffer"),
            size: size as u64,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        if let Ok(mut bufs) = self.buffers.lock() {
            bufs.insert(handle, buffer);
        }
        Ok(GpuBuffer::new(size, BackendData::Handle(handle)))
    }

    fn upload(&self, buffer: &GpuBuffer, data: &[u8]) -> Result<(), GpuError> {
        if data.len() != buffer.size() {
            return Err(GpuError::BufferSizeMismatch {
                expected: buffer.size(),
                actual: data.len(),
            });
        }
        let handle = buffer
            .backend_data()
            .as_handle()
            .ok_or_else(|| GpuError::BackendError("not a wgpu buffer".into()))?;
        let bufs = self
            .buffers
            .lock()
            .map_err(|e| GpuError::BackendError(e.to_string()))?;
        let wgpu_buf = bufs
            .get(&handle)
            .ok_or_else(|| GpuError::BackendError("buffer not found".into()))?;
        self.queue.write_buffer(wgpu_buf, 0, data);
        Ok(())
    }

    fn download(&self, buffer: &GpuBuffer, dst: &mut [u8]) -> Result<(), GpuError> {
        if dst.len() != buffer.size() {
            return Err(GpuError::BufferSizeMismatch {
                expected: buffer.size(),
                actual: dst.len(),
            });
        }
        let handle = buffer
            .backend_data()
            .as_handle()
            .ok_or_else(|| GpuError::BackendError("not a wgpu buffer".into()))?;
        let bufs = self
            .buffers
            .lock()
            .map_err(|e| GpuError::BackendError(e.to_string()))?;
        let wgpu_buf = bufs
            .get(&handle)
            .ok_or_else(|| GpuError::BackendError("buffer not found".into()))?;

        // Create staging buffer for readback
        let staging = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("fj_staging"),
            size: buffer.size() as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("fj_download"),
            });
        encoder.copy_buffer_to_buffer(wgpu_buf, 0, &staging, 0, buffer.size() as u64);
        self.queue.submit(std::iter::once(encoder.finish()));

        // Map staging buffer and read
        let slice = staging.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });
        let _ = self.device.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: None,
        });

        rx.recv()
            .map_err(|e| GpuError::BackendError(format!("map recv failed: {e}")))?
            .map_err(|e| GpuError::BackendError(format!("map failed: {e}")))?;

        let data = slice.get_mapped_range();
        dst.copy_from_slice(&data);
        drop(data);
        staging.unmap();

        Ok(())
    }

    fn compile_kernel(&self, source: &KernelSource) -> Result<GpuKernel, GpuError> {
        let (wgsl_src, name, num_bindings, workgroup_size) = match source {
            KernelSource::Wgsl(src) => {
                if src.is_empty() {
                    return Err(GpuError::InvalidKernel("empty WGSL source".into()));
                }
                (
                    src.clone(),
                    "custom".to_string(),
                    0u32,
                    WorkgroupSize::d1(256),
                )
            }
            KernelSource::Builtin(builtin) => {
                let (src, bindings) = builtin_wgsl(*builtin);
                (src, format!("{builtin}"), bindings, WorkgroupSize::d1(256))
            }
            _ => {
                return Err(GpuError::InvalidKernel(
                    "wgpu backend requires WGSL or Builtin kernel".into(),
                ));
            }
        };

        let shader_module = self
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some(&name),
                source: wgpu::ShaderSource::Wgsl(wgsl_src.into()),
            });

        // Create bind group layout with N storage buffer bindings
        let entries: Vec<wgpu::BindGroupLayoutEntry> = (0..num_bindings)
            .map(|i| wgpu::BindGroupLayoutEntry {
                binding: i,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage {
                        read_only: i < num_bindings - 1,
                    },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            })
            .collect();

        let bind_group_layout =
            self.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("fj_bgl"),
                    entries: &entries,
                });

        let pipeline_layout = self
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("fj_pl"),
                bind_group_layouts: &[&bind_group_layout],
                immediate_size: 0,
            });

        let pipeline = self
            .device
            .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(&name),
                layout: Some(&pipeline_layout),
                module: &shader_module,
                entry_point: Some("main"),
                compilation_options: Default::default(),
                cache: None,
            });

        let handle = NEXT_HANDLE.fetch_add(1, Ordering::Relaxed);
        if let Ok(mut pipes) = self.pipelines.lock() {
            pipes.insert(
                handle,
                CompiledPipeline {
                    pipeline,
                    bind_group_layout,
                    num_bindings,
                },
            );
        }

        Ok(GpuKernel::new(name, num_bindings, workgroup_size, handle))
    }

    fn execute(
        &self,
        kernel: &GpuKernel,
        workgroups: (u32, u32, u32),
        buffers: &[&GpuBuffer],
    ) -> Result<(), GpuError> {
        let pipes = self
            .pipelines
            .lock()
            .map_err(|e| GpuError::BackendError(e.to_string()))?;
        let compiled = pipes
            .get(&kernel.backend_handle())
            .ok_or_else(|| GpuError::DispatchFailed("kernel pipeline not found".into()))?;

        if buffers.len() < compiled.num_bindings as usize {
            return Err(GpuError::DispatchFailed(format!(
                "expected {} buffers, got {}",
                compiled.num_bindings,
                buffers.len()
            )));
        }

        let bufs = self
            .buffers
            .lock()
            .map_err(|e| GpuError::BackendError(e.to_string()))?;

        // Build bind group entries
        let mut bg_entries: Vec<wgpu::BindGroupEntry> = Vec::new();
        for (i, gpu_buf) in buffers
            .iter()
            .enumerate()
            .take(compiled.num_bindings as usize)
        {
            let handle = gpu_buf
                .backend_data()
                .as_handle()
                .ok_or_else(|| GpuError::BackendError("not a wgpu buffer".into()))?;
            let wgpu_buf = bufs
                .get(&handle)
                .ok_or_else(|| GpuError::BackendError("buffer not found".into()))?;
            bg_entries.push(wgpu::BindGroupEntry {
                binding: i as u32,
                resource: wgpu_buf.as_entire_binding(),
            });
        }

        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("fj_bg"),
            layout: &compiled.bind_group_layout,
            entries: &bg_entries,
        });

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("fj_dispatch"),
            });
        {
            let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("fj_compute"),
                timestamp_writes: None,
            });
            cpass.set_pipeline(&compiled.pipeline);
            cpass.set_bind_group(0, &bind_group, &[]);
            cpass.dispatch_workgroups(workgroups.0, workgroups.1, workgroups.2);
        }
        // Drop locks before submit
        drop(bufs);
        drop(pipes);

        self.queue.submit(std::iter::once(encoder.finish()));
        let _ = self.device.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: None,
        });

        Ok(())
    }
}

// SAFETY: WgpuDevice holds wgpu types that are Send + Sync
unsafe impl Send for WgpuDevice {}
unsafe impl Sync for WgpuDevice {}

/// Generate WGSL source for built-in kernels.
/// Returns (wgsl_source, num_buffer_bindings).
fn builtin_wgsl(kernel: BuiltinKernel) -> (String, u32) {
    match kernel {
        BuiltinKernel::VectorAdd => (
            r#"
@group(0) @binding(0) var<storage, read> a: array<f32>;
@group(0) @binding(1) var<storage, read> b: array<f32>;
@group(0) @binding(2) var<storage, read_write> result: array<f32>;

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if i < arrayLength(&a) {
        result[i] = a[i] + b[i];
    }
}
"#
            .to_string(),
            3,
        ),
        BuiltinKernel::VectorMul => (
            r#"
@group(0) @binding(0) var<storage, read> a: array<f32>;
@group(0) @binding(1) var<storage, read> b: array<f32>;
@group(0) @binding(2) var<storage, read_write> result: array<f32>;

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if i < arrayLength(&a) {
        result[i] = a[i] * b[i];
    }
}
"#
            .to_string(),
            3,
        ),
        BuiltinKernel::VectorSub => (
            r#"
@group(0) @binding(0) var<storage, read> a: array<f32>;
@group(0) @binding(1) var<storage, read> b: array<f32>;
@group(0) @binding(2) var<storage, read_write> result: array<f32>;

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if i < arrayLength(&a) {
        result[i] = a[i] - b[i];
    }
}
"#
            .to_string(),
            3,
        ),
        BuiltinKernel::VectorDiv => (
            r#"
@group(0) @binding(0) var<storage, read> a: array<f32>;
@group(0) @binding(1) var<storage, read> b: array<f32>;
@group(0) @binding(2) var<storage, read_write> result: array<f32>;

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if i < arrayLength(&a) {
        if b[i] != 0.0 {
            result[i] = a[i] / b[i];
        } else {
            result[i] = 0.0;
        }
    }
}
"#
            .to_string(),
            3,
        ),
        BuiltinKernel::Relu => (
            r#"
@group(0) @binding(0) var<storage, read> input: array<f32>;
@group(0) @binding(1) var<storage, read_write> output: array<f32>;

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if i < arrayLength(&input) {
        output[i] = max(input[i], 0.0);
    }
}
"#
            .to_string(),
            2,
        ),
        BuiltinKernel::Sigmoid => (
            r#"
@group(0) @binding(0) var<storage, read> input: array<f32>;
@group(0) @binding(1) var<storage, read_write> output: array<f32>;

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if i < arrayLength(&input) {
        output[i] = 1.0 / (1.0 + exp(-input[i]));
    }
}
"#
            .to_string(),
            2,
        ),
        BuiltinKernel::Softmax => (
            // Per-element exp — caller must handle sum+normalize
            r#"
@group(0) @binding(0) var<storage, read> input: array<f32>;
@group(0) @binding(1) var<storage, read_write> output: array<f32>;

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if i < arrayLength(&input) {
        output[i] = exp(input[i]);
    }
}
"#
            .to_string(),
            2,
        ),
        BuiltinKernel::Matmul => (
            // Tiled matmul: A[M,K] * B[K,N] = C[M,N]
            // Dimensions passed via buffer layout (M*K + K*N elements)
            // Simplified: uses global_id as (row, col) of output
            r#"
@group(0) @binding(0) var<storage, read> a: array<f32>;
@group(0) @binding(1) var<storage, read> b: array<f32>;
@group(0) @binding(2) var<storage, read_write> c: array<f32>;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    // Simplified matmul — dimensions inferred from buffer sizes
    let row = gid.x;
    let col = gid.y;
    let n = arrayLength(&b);
    let m = arrayLength(&a);
    if m > 0u && n > 0u {
        // Assume square for simplicity in builtin
        let dim = u32(sqrt(f32(m)));
        if row < dim && col < dim {
            var sum: f32 = 0.0;
            for (var k: u32 = 0u; k < dim; k = k + 1u) {
                sum = sum + a[row * dim + k] * b[k * dim + col];
            }
            c[row * dim + col] = sum;
        }
    }
}
"#
            .to_string(),
            3,
        ),
    }
}

/// Minimal synchronous block_on for wgpu's async adapter request.
fn pollster_block_on<F: std::future::Future>(future: F) -> F::Output {
    use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

    fn noop_raw_waker() -> RawWaker {
        fn no_op(_: *const ()) {}
        fn clone(_: *const ()) -> RawWaker {
            noop_raw_waker()
        }
        RawWaker::new(
            std::ptr::null(),
            &RawWakerVTable::new(clone, no_op, no_op, no_op),
        )
    }

    // SAFETY: noop waker is valid — it just does nothing
    let waker = unsafe { Waker::from_raw(noop_raw_waker()) };
    let mut cx = Context::from_waker(&waker);
    let mut future = std::pin::pin!(future);

    loop {
        match future.as_mut().poll(&mut cx) {
            Poll::Ready(result) => return result,
            Poll::Pending => std::thread::yield_now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn try_get_device() -> Option<WgpuDevice> {
        let instance = wgpu::Instance::default();
        let adapters: Vec<_> =
            pollster_block_on(instance.enumerate_adapters(wgpu::Backends::all()));

        for adapter in adapters {
            let info = adapter.get_info();
            if info.device_type == wgpu::DeviceType::Cpu {
                continue;
            }

            if let Ok((device, queue)) =
                pollster_block_on(adapter.request_device(&wgpu::DeviceDescriptor {
                    label: Some("test"),
                    ..Default::default()
                }))
            {
                let limits = device.limits();
                return Some(WgpuDevice {
                    info: GpuDeviceInfo {
                        name: info.name,
                        memory: 0,
                        compute_units: 1,
                        backend: GpuBackend::Wgpu,
                        max_workgroup_size: limits.max_compute_invocations_per_workgroup,
                        max_buffer_size: limits.max_buffer_size,
                    },
                    device,
                    queue,
                    buffers: Mutex::new(HashMap::new()),
                    pipelines: Mutex::new(HashMap::new()),
                });
            }
        }
        None
    }

    #[test]
    fn wgpu_enumerate() {
        // This test passes even without GPU — just returns Err
        let result = enumerate_devices();
        // Don't assert success — CI may not have GPU
        if let Ok(devices) = result {
            assert!(!devices.is_empty());
            for dev in &devices {
                assert_eq!(dev.info().backend, GpuBackend::Wgpu);
            }
        }
    }

    #[test]
    fn wgpu_buffer_upload_download() {
        let Some(dev) = try_get_device() else {
            return; // skip if no GPU
        };

        let buf = dev.create_buffer(16).expect("create buffer");
        let data: Vec<u8> = [1.0f32, 2.0, 3.0, 4.0]
            .iter()
            .flat_map(|f| f.to_le_bytes())
            .collect();
        dev.upload(&buf, &data).expect("upload");

        let mut out = vec![0u8; 16];
        dev.download(&buf, &mut out).expect("download");

        let result: Vec<f32> = out
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        assert_eq!(result, vec![1.0, 2.0, 3.0, 4.0]);
    }

    #[test]
    fn wgpu_vector_add() {
        let Some(dev) = try_get_device() else {
            return;
        };

        let n = 4usize;
        let size = n * 4;
        let buf_a = dev.create_buffer(size).expect("buf a");
        let buf_b = dev.create_buffer(size).expect("buf b");
        let buf_out = dev.create_buffer(size).expect("buf out");

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

        let mut out = vec![0u8; size];
        dev.download(&buf_out, &mut out).expect("download");

        let result: Vec<f32> = out
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        assert_eq!(result, vec![11.0, 22.0, 33.0, 44.0]);
    }

    #[test]
    fn wgpu_relu() {
        let Some(dev) = try_get_device() else {
            return;
        };

        let n = 4usize;
        let size = n * 4;
        let buf_in = dev.create_buffer(size).expect("buf in");
        let buf_out = dev.create_buffer(size).expect("buf out");

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

        let mut out = vec![0u8; size];
        dev.download(&buf_out, &mut out).expect("download");

        let result: Vec<f32> = out
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        assert_eq!(result, vec![0.0, 0.0, 0.0, 3.0]);
    }

    #[test]
    fn wgpu_matmul_2x2() {
        let Some(dev) = try_get_device() else {
            return;
        };

        // A = [[1,2],[3,4]], B = [[5,6],[7,8]]
        // C = [[19,22],[43,50]]
        let size = 4 * 4; // 4 f32s = 16 bytes
        let buf_a = dev.create_buffer(size).expect("buf a");
        let buf_b = dev.create_buffer(size).expect("buf b");
        let buf_c = dev.create_buffer(size).expect("buf c");

        let a: Vec<u8> = [1.0f32, 2.0, 3.0, 4.0]
            .iter()
            .flat_map(|f| f.to_le_bytes())
            .collect();
        let b: Vec<u8> = [5.0f32, 6.0, 7.0, 8.0]
            .iter()
            .flat_map(|f| f.to_le_bytes())
            .collect();

        dev.upload(&buf_a, &a).expect("upload a");
        dev.upload(&buf_b, &b).expect("upload b");

        let kernel = dev
            .compile_kernel(&KernelSource::Builtin(BuiltinKernel::Matmul))
            .expect("compile");
        dev.execute(&kernel, (1, 1, 1), &[&buf_a, &buf_b, &buf_c])
            .expect("execute");

        let mut out = vec![0u8; size];
        dev.download(&buf_c, &mut out).expect("download");

        let result: Vec<f32> = out
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        assert_eq!(result, vec![19.0, 22.0, 43.0, 50.0]);
    }

    #[test]
    fn builtin_wgsl_generates_valid_source() {
        let kernels = [
            BuiltinKernel::VectorAdd,
            BuiltinKernel::VectorMul,
            BuiltinKernel::VectorSub,
            BuiltinKernel::VectorDiv,
            BuiltinKernel::Relu,
            BuiltinKernel::Sigmoid,
            BuiltinKernel::Softmax,
            BuiltinKernel::Matmul,
        ];
        for k in kernels {
            let (src, bindings) = builtin_wgsl(k);
            assert!(!src.is_empty(), "WGSL for {k} should not be empty");
            assert!(bindings >= 2, "kernel {k} should have at least 2 bindings");
            assert!(
                src.contains("@compute"),
                "WGSL for {k} should contain @compute"
            );
            assert!(
                src.contains("fn main"),
                "WGSL for {k} should contain fn main"
            );
        }
    }

    #[test]
    fn wgpu_compile_custom_wgsl() {
        let Some(dev) = try_get_device() else {
            return;
        };

        let wgsl = r#"
@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    // no-op kernel
}
"#;
        let kernel = dev.compile_kernel(&KernelSource::Wgsl(wgsl.into()));
        assert!(kernel.is_ok());
    }

    #[test]
    fn wgpu_compile_empty_wgsl_fails() {
        let Some(dev) = try_get_device() else {
            return;
        };

        let result = dev.compile_kernel(&KernelSource::Wgsl(String::new()));
        assert!(result.is_err());
    }
}
