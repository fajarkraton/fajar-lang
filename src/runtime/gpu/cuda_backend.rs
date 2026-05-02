//! CUDA FFI backend — NVIDIA-specific GPU acceleration.
//!
//! Enabled with `--features cuda`.
//! Dynamically loads `libcuda.so` at runtime — no compile-time CUDA dependency.

use super::GpuError;
use super::buffer::{BackendData, GpuBuffer};
use super::device::{GpuBackend, GpuDevice, GpuDeviceInfo};
use super::kernel::{BuiltinKernel, GpuKernel, KernelSource, WorkgroupSize};

use libloading::{Library, Symbol};
use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::Mutex;

// ═══════════════════════════════════════════════════════════════════════
// PTX kernel templates for builtin operations (sm_89 / Ada Lovelace)
// ═══════════════════════════════════════════════════════════════════════

/// Generate PTX for a 1D elementwise binary kernel: C[i] = A[i] op B[i].
fn ptx_elementwise(name: &str, op_instruction: &str) -> String {
    format!(
        r#".version 8.3
.target sm_89
.address_size 64

.visible .entry {name}(
    .param .u64 param_A,
    .param .u64 param_B,
    .param .u64 param_C,
    .param .u32 param_N
) {{
    .reg .u32  %rtid, %rbid, %rbdim, %gid, %n;
    .reg .u64  %a, %b, %c, %off;
    .reg .f32  %va, %vb, %vc;
    .reg .pred %p;

    ld.param.u64 %a, [param_A];
    ld.param.u64 %b, [param_B];
    ld.param.u64 %c, [param_C];
    ld.param.u32 %n, [param_N];

    mov.u32 %rtid, %tid.x;
    mov.u32 %rbid, %ctaid.x;
    mov.u32 %rbdim, %ntid.x;
    mad.lo.u32 %gid, %rbid, %rbdim, %rtid;

    setp.ge.u32 %p, %gid, %n;
    @%p bra DONE;

    mul.wide.u32 %off, %gid, 4;
    add.u64 %a, %a, %off;
    add.u64 %b, %b, %off;
    add.u64 %c, %c, %off;

    ld.global.f32 %va, [%a];
    ld.global.f32 %vb, [%b];
    {op_instruction}
    st.global.f32 [%c], %vc;

DONE:
    ret;
}}
"#
    )
}

/// Generate PTX for a 1D unary activation kernel: Out[i] = f(In[i]).
fn ptx_activation(name: &str, body: &str) -> String {
    format!(
        r#".version 8.3
.target sm_89
.address_size 64

.visible .entry {name}(
    .param .u64 param_In,
    .param .u64 param_Out,
    .param .u32 param_N
) {{
    .reg .u32  %rtid, %rbid, %rbdim, %gid, %n;
    .reg .u64  %in_ptr, %out_ptr, %off;
    .reg .f32  %val, %result, %tmp, %one, %zero, %neg, %alpha;
    .reg .pred %p, %q;

    ld.param.u64 %in_ptr, [param_In];
    ld.param.u64 %out_ptr, [param_Out];
    ld.param.u32 %n, [param_N];

    mov.u32 %rtid, %tid.x;
    mov.u32 %rbid, %ctaid.x;
    mov.u32 %rbdim, %ntid.x;
    mad.lo.u32 %gid, %rbid, %rbdim, %rtid;

    setp.ge.u32 %p, %gid, %n;
    @%p bra DONE;

    mul.wide.u32 %off, %gid, 4;
    add.u64 %in_ptr, %in_ptr, %off;
    add.u64 %out_ptr, %out_ptr, %off;

    ld.global.f32 %val, [%in_ptr];
    {body}
    st.global.f32 [%out_ptr], %result;

DONE:
    ret;
}}
"#
    )
}

/// Generate PTX for tiled matrix multiplication with shared memory.
/// Uses 16x16 tiles to exploit shared memory locality.
/// Grid: (ceil(N/16), ceil(M/16), 1), Block: (16, 16, 1).
/// Each tile: load A tile + B tile → shared mem → barrier → compute partial sum → barrier.
fn ptx_matmul() -> String {
    // TILE = 16, matching block dimensions (16x16 = 256 threads per block)
    r#".version 8.3
.target sm_89
.address_size 64

.visible .entry matmul(
    .param .u64 param_A,
    .param .u64 param_B,
    .param .u64 param_C,
    .param .u32 param_M,
    .param .u32 param_K,
    .param .u32 param_N
) {
    // Shared memory tiles: 16x16 floats each = 1024 bytes each
    .shared .align 4 .f32 tile_A[256];
    .shared .align 4 .f32 tile_B[256];

    .reg .u32  %row, %col, %m, %kk, %n;
    .reg .u32  %tidx, %tidy, %bidx, %bidy;
    .reg .u32  %t, %num_tiles, %tile_base;
    .reg .u32  %a_row, %a_col, %b_row, %b_col;
    .reg .u32  %a_idx, %b_idx, %c_idx, %s_idx;
    .reg .u32  %i, %tile_k;
    .reg .u64  %a_base, %b_base, %c_base, %addr, %saddr;
    .reg .f32  %sum, %va, %vb, %zero_f;
    .reg .pred %p_bounds, %p_tile_loop, %p_k_loop;
    .reg .pred %p_a_bounds, %p_b_bounds;

    ld.param.u64 %a_base, [param_A];
    ld.param.u64 %b_base, [param_B];
    ld.param.u64 %c_base, [param_C];
    ld.param.u32 %m, [param_M];
    ld.param.u32 %kk, [param_K];
    ld.param.u32 %n, [param_N];

    mov.u32 %tidy, %tid.y;
    mov.u32 %tidx, %tid.x;
    mov.u32 %bidy, %ctaid.y;
    mov.u32 %bidx, %ctaid.x;

    // row = blockIdx.y * 16 + threadIdx.y
    mad.lo.u32 %row, %bidy, 16, %tidy;
    // col = blockIdx.x * 16 + threadIdx.x
    mad.lo.u32 %col, %bidx, 16, %tidx;

    // num_tiles = ceil(K / 16)
    add.u32 %num_tiles, %kk, 15;
    shr.u32 %num_tiles, %num_tiles, 4;

    mov.f32 %sum, 0f00000000;
    mov.f32 %zero_f, 0f00000000;
    mov.u32 %t, 0;

TILE_LOOP:
    setp.ge.u32 %p_tile_loop, %t, %num_tiles;
    @%p_tile_loop bra WRITE_C;

    // tile_base = t * 16
    shl.b32 %tile_base, %t, 4;

    // --- Load A tile: tile_A[tidy][tidx] = A[row, tile_base + tidx] ---
    // a_col = tile_base + tidx
    add.u32 %a_col, %tile_base, %tidx;
    // Shared mem index: tidy * 16 + tidx
    mad.lo.u32 %s_idx, %tidy, 16, %tidx;
    // Bounds check: row < M && a_col < K
    setp.lt.u32 %p_a_bounds, %row, %m;
    setp.lt.and.u32 %p_a_bounds, %a_col, %kk, %p_a_bounds;
    // Load or zero
    @!%p_a_bounds mov.f32 %va, 0f00000000;
    @!%p_a_bounds bra SKIP_LOAD_A;
    mad.lo.u32 %a_idx, %row, %kk, %a_col;
    mul.wide.u32 %addr, %a_idx, 4;
    add.u64 %addr, %a_base, %addr;
    ld.global.f32 %va, [%addr];
SKIP_LOAD_A:
    mul.wide.u32 %saddr, %s_idx, 4;
    mov.u64 %addr, tile_A;
    add.u64 %addr, %addr, %saddr;
    st.shared.f32 [%addr], %va;

    // --- Load B tile: tile_B[tidy][tidx] = B[tile_base + tidy, col] ---
    // b_row = tile_base + tidy
    add.u32 %b_row, %tile_base, %tidy;
    // Bounds check: b_row < K && col < N
    setp.lt.u32 %p_b_bounds, %b_row, %kk;
    setp.lt.and.u32 %p_b_bounds, %col, %n, %p_b_bounds;
    @!%p_b_bounds mov.f32 %vb, 0f00000000;
    @!%p_b_bounds bra SKIP_LOAD_B;
    mad.lo.u32 %b_idx, %b_row, %n, %col;
    mul.wide.u32 %addr, %b_idx, 4;
    add.u64 %addr, %b_base, %addr;
    ld.global.f32 %vb, [%addr];
SKIP_LOAD_B:
    // s_idx already = tidy * 16 + tidx
    mul.wide.u32 %saddr, %s_idx, 4;
    mov.u64 %addr, tile_B;
    add.u64 %addr, %addr, %saddr;
    st.shared.f32 [%addr], %vb;

    // --- Barrier: wait for all threads to finish loading ---
    bar.sync 0;

    // --- Compute partial dot product over the tile ---
    mov.u32 %i, 0;
K_LOOP:
    setp.ge.u32 %p_k_loop, %i, 16;
    @%p_k_loop bra K_DONE;

    // Check that tile_base + i < K
    add.u32 %tile_k, %tile_base, %i;
    setp.ge.u32 %p_k_loop, %tile_k, %kk;
    @%p_k_loop bra K_DONE;

    // va = tile_A[tidy][i] = tile_A[tidy * 16 + i]
    mad.lo.u32 %a_idx, %tidy, 16, %i;
    mul.wide.u32 %saddr, %a_idx, 4;
    mov.u64 %addr, tile_A;
    add.u64 %addr, %addr, %saddr;
    ld.shared.f32 %va, [%addr];

    // vb = tile_B[i][tidx] = tile_B[i * 16 + tidx]
    mad.lo.u32 %b_idx, %i, 16, %tidx;
    mul.wide.u32 %saddr, %b_idx, 4;
    mov.u64 %addr, tile_B;
    add.u64 %addr, %addr, %saddr;
    ld.shared.f32 %vb, [%addr];

    fma.rn.f32 %sum, %va, %vb, %sum;

    add.u32 %i, %i, 1;
    bra K_LOOP;

K_DONE:
    // --- Barrier: wait before next tile overwrites shared mem ---
    bar.sync 0;

    add.u32 %t, %t, 1;
    bra TILE_LOOP;

WRITE_C:
    // Bounds check: row < M && col < N
    setp.ge.u32 %p_bounds, %row, %m;
    @%p_bounds bra DONE;
    setp.ge.u32 %p_bounds, %col, %n;
    @%p_bounds bra DONE;

    // C[row * N + col] = sum
    mad.lo.u32 %c_idx, %row, %n, %col;
    mul.wide.u32 %addr, %c_idx, 4;
    add.u64 %addr, %c_base, %addr;
    st.global.f32 [%addr], %sum;

DONE:
    ret;
}
"#
    .to_string()
}

/// Generate PTX source for a builtin kernel.
fn generate_builtin_ptx(builtin: BuiltinKernel) -> (String, String) {
    match builtin {
        BuiltinKernel::VectorAdd => (
            "vector_add".into(),
            ptx_elementwise("vector_add", "add.f32 %vc, %va, %vb;"),
        ),
        BuiltinKernel::VectorSub => (
            "vector_sub".into(),
            ptx_elementwise("vector_sub", "sub.f32 %vc, %va, %vb;"),
        ),
        BuiltinKernel::VectorMul => (
            "vector_mul".into(),
            ptx_elementwise("vector_mul", "mul.f32 %vc, %va, %vb;"),
        ),
        BuiltinKernel::VectorDiv => (
            "vector_div".into(),
            ptx_elementwise("vector_div", "div.rn.f32 %vc, %va, %vb;"),
        ),
        BuiltinKernel::Relu => (
            "relu".into(),
            ptx_activation(
                "relu",
                "mov.f32 %zero, 0f00000000;\n    max.f32 %result, %val, %zero;",
            ),
        ),
        BuiltinKernel::Sigmoid => (
            "sigmoid".into(),
            ptx_activation(
                "sigmoid",
                concat!(
                    "neg.f32 %neg, %val;\n",
                    "    // exp(-x): use ex2 approximation: exp(x) = 2^(x * log2(e))\n",
                    "    mul.f32 %tmp, %neg, 0f3FB8AA3B;  // log2(e) = 1.4427\n",
                    "    ex2.approx.f32 %tmp, %tmp;\n",
                    "    mov.f32 %one, 0f3F800000;  // 1.0\n",
                    "    add.f32 %tmp, %one, %tmp;\n",
                    "    div.rn.f32 %result, %one, %tmp;"
                ),
            ),
        ),
        BuiltinKernel::Softmax => {
            // Per-element exp — caller normalizes (same as CPU fallback)
            (
                "softmax".into(),
                ptx_activation(
                    "softmax",
                    concat!(
                        "mul.f32 %tmp, %val, 0f3FB8AA3B;  // log2(e)\n",
                        "    ex2.approx.f32 %result, %tmp;"
                    ),
                ),
            )
        }
        BuiltinKernel::Matmul => ("matmul".into(), ptx_matmul()),
        BuiltinKernel::CodebookDot => ("codebook_dot".into(), ptx_codebook_dot()),
    }
}

/// Generate PTX for FajarQuant codebook dot product.
/// Each thread computes one token's attention score:
///   score[token_id] = sum_j query[j] * codebook[indices[token_id * dim + j]]
///
/// Args: (query_ptr: f32*, indices_ptr: u8*, codebook_ptr: f32*, scores_ptr: f32*,
///         n_tokens: u32, dim: u32)
/// Grid: (ceil(n_tokens/256), 1, 1), Block: (256, 1, 1)
fn ptx_codebook_dot() -> String {
    r#".version 8.3
.target sm_89
.address_size 64

.visible .entry codebook_dot(
    .param .u64 param_query,
    .param .u64 param_indices,
    .param .u64 param_codebook,
    .param .u64 param_scores,
    .param .u32 param_n_tokens,
    .param .u32 param_dim
) {
    .reg .u32  %rtid, %rbid, %rbdim, %gid, %n_tokens, %dim;
    .reg .u32  %j, %idx_u32, %offset;
    .reg .u64  %q_base, %idx_base, %cb_base, %sc_base;
    .reg .u64  %addr, %off64;
    .reg .f32  %sum, %qval, %cbval;
    .reg .pred %p_bounds, %p_loop;

    ld.param.u64 %q_base, [param_query];
    ld.param.u64 %idx_base, [param_indices];
    ld.param.u64 %cb_base, [param_codebook];
    ld.param.u64 %sc_base, [param_scores];
    ld.param.u32 %n_tokens, [param_n_tokens];
    ld.param.u32 %dim, [param_dim];

    // gid = blockIdx.x * blockDim.x + threadIdx.x (token index)
    mov.u32 %rtid, %tid.x;
    mov.u32 %rbid, %ctaid.x;
    mov.u32 %rbdim, %ntid.x;
    mad.lo.u32 %gid, %rbid, %rbdim, %rtid;

    setp.ge.u32 %p_bounds, %gid, %n_tokens;
    @%p_bounds bra DONE;

    // sum = 0.0
    mov.f32 %sum, 0f00000000;

    // offset = gid * dim (start of this token's indices)
    mul.lo.u32 %offset, %gid, %dim;

    mov.u32 %j, 0;
LOOP:
    setp.ge.u32 %p_loop, %j, %dim;
    @%p_loop bra STORE;

    // Load query[j]
    mul.wide.u32 %off64, %j, 4;
    add.u64 %addr, %q_base, %off64;
    ld.global.f32 %qval, [%addr];

    // Load indices[offset + j] (u8)
    add.u32 %idx_u32, %offset, %j;
    cvt.u64.u32 %off64, %idx_u32;
    add.u64 %addr, %idx_base, %off64;
    ld.global.u8 %idx_u32, [%addr];

    // Load codebook[idx] (f32)
    mul.wide.u32 %off64, %idx_u32, 4;
    add.u64 %addr, %cb_base, %off64;
    ld.global.f32 %cbval, [%addr];

    // sum += query[j] * codebook[idx]
    fma.rn.f32 %sum, %qval, %cbval, %sum;

    add.u32 %j, %j, 1;
    bra LOOP;

STORE:
    // scores[gid] = sum
    mul.wide.u32 %off64, %gid, 4;
    add.u64 %addr, %sc_base, %off64;
    st.global.f32 [%addr], %sum;

DONE:
    ret;
}
"#
    .to_string()
}

// ═══════════════════════════════════════════════════════════════════════
// CUDA Device
// ═══════════════════════════════════════════════════════════════════════

/// Metadata for a compiled CUDA kernel.
struct CudaKernelData {
    /// CUfunction handle.
    function: *mut c_void,
    /// CUmodule handle (kept alive so CUfunction remains valid).
    _module: *mut c_void,
    /// Kernel kind for determining dispatch parameters.
    kind: CudaKernelKind,
}

// SAFETY: CUDA handles are safe to send between threads when using cuCtxSetCurrent
unsafe impl Send for CudaKernelData {}
unsafe impl Sync for CudaKernelData {}

/// Classification of compiled kernels for dispatch parameter setup.
#[derive(Debug, Clone, Copy)]
enum CudaKernelKind {
    /// C[i] = f(A[i], B[i]) — 3 buffer params + N
    Elementwise,
    /// Out[i] = f(In[i]) — 2 buffer params + N
    Activation,
    /// C = A @ B — 3 buffer params + M, K, N
    Matmul,
    /// FajarQuant codebook dot: 4 buffers (query, indices, codebook, scores) + n_tokens, dim
    CodebookDot,
    /// User-supplied PTX — pass buffer pointers only
    Custom,
}

/// CUDA device implementation via driver API.
pub struct CudaDevice {
    info: GpuDeviceInfo,
    _lib: Library,
    context: *mut c_void,
    /// CUDA stream for async operations (upload→compute→download pipeline).
    stream: *mut c_void,
    /// Loaded kernels, keyed by GpuKernel::id().
    kernels: Mutex<HashMap<u64, CudaKernelData>>,
    /// Compiled kernel cache: BuiltinKernel → cached GpuKernel.
    /// Avoids re-JIT-compiling the same PTX on every call.
    kernel_cache: Mutex<HashMap<u8, GpuKernel>>,
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

    // SAFETY: CUDA driver API symbol loaded via libloading — error handled by .map_err()
    let cu_device_get_name: Symbol<unsafe extern "C" fn(*mut u8, i32, i32) -> i32> =
        unsafe { lib.get(b"cuDeviceGetName") }
            .map_err(|e| GpuError::BackendError(e.to_string()))?;

    // SAFETY: CUDA driver API symbol loaded via libloading — error handled by .map_err()
    let cu_device_total_mem: Symbol<unsafe extern "C" fn(*mut usize, i32) -> i32> =
        unsafe { lib.get(b"cuDeviceTotalMem_v2") }
            .map_err(|e| GpuError::BackendError(e.to_string()))?;

    // SAFETY: CUDA driver API symbol loaded via libloading — error handled by .map_err()
    let cu_device_get_attribute: Symbol<unsafe extern "C" fn(*mut i32, i32, i32) -> i32> =
        unsafe { lib.get(b"cuDeviceGetAttribute") }
            .map_err(|e| GpuError::BackendError(e.to_string()))?;

    // SAFETY: CUDA driver API symbol loaded via libloading — error handled by .map_err()
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

        // Create a CUDA stream for async operations
        // SAFETY: cuStreamCreate is safe after cuCtxCreate succeeds
        let mut stream: *mut c_void = std::ptr::null_mut();
        let cu_stream_create: Result<
            Symbol<unsafe extern "C" fn(*mut *mut c_void, u32) -> i32>,
            _,
        > = unsafe { lib.get(b"cuStreamCreate") };
        if let Ok(create_fn) = cu_stream_create {
            // flags=0 = default stream behavior
            let _ = unsafe { create_fn(&mut stream, 0) };
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
            stream,
            kernels: Mutex::new(HashMap::new()),
            kernel_cache: Mutex::new(HashMap::new()),
        }));
    }

    if devices.is_empty() {
        return Err(GpuError::NotAvailable);
    }

    Ok(devices)
}

impl CudaDevice {
    /// Load a PTX module and extract a named function.
    fn load_ptx_kernel(
        &self,
        ptx_source: &str,
        entry_name: &str,
    ) -> Result<(*mut c_void, *mut c_void), GpuError> {
        // SAFETY: loading CUDA driver API symbols
        let lib = unsafe { Library::new("libcuda.so") }
            .map_err(|e| GpuError::BackendError(e.to_string()))?;

        // cuModuleLoadData(module*, ptx_image) — loads PTX JIT-compiled module
        let cu_module_load_data: Symbol<unsafe extern "C" fn(*mut *mut c_void, *const u8) -> i32> =
            unsafe { lib.get(b"cuModuleLoadData") }
                .map_err(|e| GpuError::BackendError(format!("cuModuleLoadData not found: {e}")))?;

        // cuModuleGetFunction(function*, module, name) — extracts kernel entry point
        let cu_module_get_function: Symbol<
            unsafe extern "C" fn(*mut *mut c_void, *mut c_void, *const u8) -> i32,
        > = unsafe { lib.get(b"cuModuleGetFunction") }
            .map_err(|e| GpuError::BackendError(format!("cuModuleGetFunction not found: {e}")))?;

        // Null-terminate the PTX source
        let mut ptx_bytes = ptx_source.as_bytes().to_vec();
        ptx_bytes.push(0);

        let mut module: *mut c_void = std::ptr::null_mut();
        // SAFETY: ptx_bytes is null-terminated valid PTX text; module is stack-allocated out param
        let r = unsafe { cu_module_load_data(&mut module, ptx_bytes.as_ptr()) };
        if r != 0 {
            return Err(GpuError::BackendError(format!(
                "cuModuleLoadData failed (error {r}): PTX JIT compilation error for '{entry_name}'"
            )));
        }

        // Null-terminate the entry name
        let mut name_bytes = entry_name.as_bytes().to_vec();
        name_bytes.push(0);

        let mut function: *mut c_void = std::ptr::null_mut();
        // SAFETY: module is valid (loaded above); name is null-terminated
        let r = unsafe { cu_module_get_function(&mut function, module, name_bytes.as_ptr()) };
        if r != 0 {
            return Err(GpuError::BackendError(format!(
                "cuModuleGetFunction failed (error {r}): entry '{entry_name}' not found in module"
            )));
        }

        Ok((module, function))
    }

    /// Launch a CUDA kernel with the given parameters.
    fn launch_kernel(
        &self,
        function: *mut c_void,
        grid: (u32, u32, u32),
        block: (u32, u32, u32),
        args: &mut [*mut c_void],
    ) -> Result<(), GpuError> {
        // SAFETY: loading CUDA driver API symbols
        let lib = unsafe { Library::new("libcuda.so") }
            .map_err(|e| GpuError::BackendError(e.to_string()))?;

        // cuLaunchKernel(f, gridX, gridY, gridZ, blockX, blockY, blockZ,
        //                sharedMem, stream, args, extra)
        #[allow(clippy::type_complexity)]
        let cu_launch_kernel: Symbol<
            unsafe extern "C" fn(
                *mut c_void,
                u32,
                u32,
                u32,
                u32,
                u32,
                u32,
                u32,
                *mut c_void,
                *mut *mut c_void,
                *mut *mut c_void,
            ) -> i32,
        > = unsafe { lib.get(b"cuLaunchKernel") }
            .map_err(|e| GpuError::BackendError(format!("cuLaunchKernel not found: {e}")))?;

        // SAFETY: function is a valid CUfunction, args point to valid kernel parameters
        // Launch on our stream (or default if stream is null)
        let r = unsafe {
            cu_launch_kernel(
                function,
                grid.0,
                grid.1,
                grid.2,
                block.0,
                block.1,
                block.2,
                0,                    // shared memory bytes
                self.stream,          // our stream (null = default)
                args.as_mut_ptr(),    // kernel arguments
                std::ptr::null_mut(), // extra (unused)
            )
        };
        if r != 0 {
            return Err(GpuError::DispatchFailed(format!(
                "cuLaunchKernel failed: error {r}"
            )));
        }

        // If using a stream, kernel runs async — sync happens in download().
        // If no stream (fallback), synchronize the context now.
        if self.stream.is_null() {
            let cu_ctx_synchronize: Symbol<unsafe extern "C" fn() -> i32> = unsafe {
                lib.get(b"cuCtxSynchronize")
            }
            .map_err(|e| GpuError::BackendError(format!("cuCtxSynchronize not found: {e}")))?;
            // SAFETY: synchronizes the current CUDA context
            let r = unsafe { cu_ctx_synchronize() };
            if r != 0 {
                return Err(GpuError::DispatchFailed(format!(
                    "cuCtxSynchronize failed: error {r}"
                )));
            }
        }

        Ok(())
    }
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

        // SAFETY: loading CUDA driver API symbols
        let lib = unsafe { Library::new("libcuda.so") }
            .map_err(|e| GpuError::BackendError(e.to_string()))?;

        // Use async memcpy on the device stream (no sync needed — kernel ordering handles it)
        if !self.stream.is_null() {
            let cu_memcpy_async: Result<
                Symbol<unsafe extern "C" fn(u64, *const u8, usize, *mut c_void) -> i32>,
                _,
            > = unsafe { lib.get(b"cuMemcpyHtoDAsync_v2") };
            if let Ok(memcpy_fn) = cu_memcpy_async {
                // SAFETY: async copy host→device on our stream
                let r = unsafe { memcpy_fn(dev_ptr, data.as_ptr(), data.len(), self.stream) };
                if r != 0 {
                    return Err(GpuError::BackendError(format!(
                        "cuMemcpyHtoDAsync failed: {r}"
                    )));
                }
                return Ok(());
            }
        }

        // Fallback: synchronous memcpy
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

        // SAFETY: loading CUDA driver API symbols
        let lib = unsafe { Library::new("libcuda.so") }
            .map_err(|e| GpuError::BackendError(e.to_string()))?;

        // Use async memcpy + stream sync (single synchronization point)
        if !self.stream.is_null() {
            let cu_memcpy_async: Result<
                Symbol<unsafe extern "C" fn(*mut u8, u64, usize, *mut c_void) -> i32>,
                _,
            > = unsafe { lib.get(b"cuMemcpyDtoHAsync_v2") };
            let cu_stream_sync: Result<Symbol<unsafe extern "C" fn(*mut c_void) -> i32>, _> =
                unsafe { lib.get(b"cuStreamSynchronize") };

            if let (Ok(memcpy_fn), Ok(sync_fn)) = (cu_memcpy_async, cu_stream_sync) {
                // SAFETY: async copy device→host on our stream
                let r = unsafe { memcpy_fn(dst.as_mut_ptr(), dev_ptr, dst.len(), self.stream) };
                if r != 0 {
                    return Err(GpuError::BackendError(format!(
                        "cuMemcpyDtoHAsync failed: {r}"
                    )));
                }
                // SAFETY: wait for all stream operations to complete
                let r = unsafe { sync_fn(self.stream) };
                if r != 0 {
                    return Err(GpuError::BackendError(format!(
                        "cuStreamSynchronize failed: {r}"
                    )));
                }
                return Ok(());
            }
        }

        // Fallback: synchronous memcpy
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

    fn free_buffer(&self, buffer: &GpuBuffer) {
        if let Some(dev_ptr) = buffer.backend_data().as_handle() {
            if dev_ptr == 0 {
                return;
            }
            // SAFETY: freeing device memory we allocated with cuMemAlloc_v2
            if let Ok(lib) = unsafe { Library::new("libcuda.so") } {
                if let Ok(cu_mem_free) =
                    unsafe { lib.get::<unsafe extern "C" fn(u64) -> i32>(b"cuMemFree_v2") }
                {
                    unsafe { cu_mem_free(dev_ptr) };
                }
            }
        }
    }

    fn compile_kernel(&self, source: &KernelSource) -> Result<GpuKernel, GpuError> {
        // Check cache for builtin kernels (avoids re-JIT-compiling same PTX)
        if let KernelSource::Builtin(builtin) = source {
            let cache_key = *builtin as u8;
            let cache = self
                .kernel_cache
                .lock()
                .map_err(|e| GpuError::BackendError(format!("cache lock poisoned: {e}")))?;
            if let Some(cached) = cache.get(&cache_key) {
                return Ok(cached.clone());
            }
        }

        let (entry_name, ptx_source, kind, num_bindings, workgroup_size) = match source {
            KernelSource::Ptx(ptx) => {
                // User-supplied PTX — extract entry name from first .entry directive
                let entry = ptx
                    .lines()
                    .find_map(|line| {
                        let trimmed = line.trim();
                        if trimmed.contains(".entry") {
                            // ".visible .entry name(" or ".entry name("
                            trimmed
                                .split(".entry")
                                .nth(1)
                                .and_then(|s| s.trim().split('(').next())
                                .map(|s| s.trim().to_string())
                        } else {
                            None
                        }
                    })
                    .unwrap_or_else(|| "kernel".to_string());
                (
                    entry,
                    ptx.clone(),
                    CudaKernelKind::Custom,
                    0u32,
                    WorkgroupSize::d1(256),
                )
            }
            KernelSource::Builtin(builtin) => {
                let (entry_name, ptx_source) = generate_builtin_ptx(*builtin);
                let (kind, num_bindings, wg) = match builtin {
                    BuiltinKernel::VectorAdd
                    | BuiltinKernel::VectorSub
                    | BuiltinKernel::VectorMul
                    | BuiltinKernel::VectorDiv => {
                        (CudaKernelKind::Elementwise, 3, WorkgroupSize::d1(256))
                    }
                    BuiltinKernel::Relu | BuiltinKernel::Sigmoid | BuiltinKernel::Softmax => {
                        (CudaKernelKind::Activation, 2, WorkgroupSize::d1(256))
                    }
                    BuiltinKernel::Matmul => (CudaKernelKind::Matmul, 3, WorkgroupSize::d2(16, 16)),
                    BuiltinKernel::CodebookDot => {
                        (CudaKernelKind::CodebookDot, 4, WorkgroupSize::d1(256))
                    }
                };
                (entry_name, ptx_source, kind, num_bindings, wg)
            }
            _ => {
                return Err(GpuError::InvalidKernel(
                    "CUDA backend requires PTX or Builtin kernel".into(),
                ));
            }
        };

        // JIT-compile PTX and extract kernel function
        let (module, function) = self.load_ptx_kernel(&ptx_source, &entry_name)?;

        let kernel = GpuKernel::new(entry_name, num_bindings, workgroup_size, 0);

        // Store kernel data for execute()
        let data = CudaKernelData {
            function,
            _module: module,
            kind,
        };

        let mut kernels = self
            .kernels
            .lock()
            .map_err(|e| GpuError::BackendError(format!("kernel lock poisoned: {e}")))?;
        kernels.insert(kernel.id(), data);

        // Cache builtin kernels for reuse
        if let KernelSource::Builtin(builtin) = source {
            if let Ok(mut cache) = self.kernel_cache.lock() {
                cache.insert(*builtin as u8, kernel.clone());
            }
        }

        Ok(kernel)
    }

    fn execute(
        &self,
        kernel: &GpuKernel,
        workgroups: (u32, u32, u32),
        buffers: &[&GpuBuffer],
    ) -> Result<(), GpuError> {
        let kernels = self
            .kernels
            .lock()
            .map_err(|e| GpuError::BackendError(format!("kernel lock poisoned: {e}")))?;

        let kdata = kernels.get(&kernel.id()).ok_or_else(|| {
            GpuError::BackendError(format!("kernel '{}' not found in device", kernel.name()))
        })?;

        let function = kdata.function;

        // Collect device pointers from buffers
        let dev_ptrs: Vec<u64> = buffers
            .iter()
            .map(|b| {
                b.backend_data()
                    .as_handle()
                    .ok_or_else(|| GpuError::BackendError("buffer is not a CUDA handle".into()))
            })
            .collect::<Result<Vec<_>, _>>()?;

        // Build kernel argument list + compute grid/block based on kernel kind
        match kdata.kind {
            CudaKernelKind::Elementwise => {
                // Kernel signature: (u64 A, u64 B, u64 C, u32 N)
                if dev_ptrs.len() < 3 {
                    return Err(GpuError::BackendError(
                        "elementwise kernel requires 3 buffers (A, B, C)".into(),
                    ));
                }
                let n: u32 = (buffers[0].size() / std::mem::size_of::<f32>()) as u32;
                let mut a_ptr = dev_ptrs[0];
                let mut b_ptr = dev_ptrs[1];
                let mut c_ptr = dev_ptrs[2];
                let mut n_val = n;

                let mut args: Vec<*mut c_void> = vec![
                    &mut a_ptr as *mut u64 as *mut c_void,
                    &mut b_ptr as *mut u64 as *mut c_void,
                    &mut c_ptr as *mut u64 as *mut c_void,
                    &mut n_val as *mut u32 as *mut c_void,
                ];

                let grid = (n.div_ceil(256), 1, 1);
                let block = (256, 1, 1);

                // Drop the lock before launch (launch_kernel loads its own lib)
                drop(kernels);
                self.launch_kernel(function, grid, block, &mut args)
            }
            CudaKernelKind::Activation => {
                // Kernel signature: (u64 In, u64 Out, u32 N)
                if dev_ptrs.len() < 2 {
                    return Err(GpuError::BackendError(
                        "activation kernel requires 2 buffers (In, Out)".into(),
                    ));
                }
                let n: u32 = (buffers[0].size() / std::mem::size_of::<f32>()) as u32;
                let mut in_ptr = dev_ptrs[0];
                let mut out_ptr = dev_ptrs[1];
                let mut n_val = n;

                let mut args: Vec<*mut c_void> = vec![
                    &mut in_ptr as *mut u64 as *mut c_void,
                    &mut out_ptr as *mut u64 as *mut c_void,
                    &mut n_val as *mut u32 as *mut c_void,
                ];

                let grid = (n.div_ceil(256), 1, 1);
                let block = (256, 1, 1);

                drop(kernels);
                self.launch_kernel(function, grid, block, &mut args)
            }
            CudaKernelKind::Matmul => {
                // Kernel signature: (u64 A, u64 B, u64 C, u32 M, u32 K, u32 N)
                // Derive M, K, N from buffer sizes:
                //   A = M*K*4, B = K*N*4, C = M*N*4
                //   K = sqrt(A_elems * B_elems / C_elems)
                if dev_ptrs.len() < 3 {
                    return Err(GpuError::BackendError(
                        "matmul kernel requires 3 buffers (A, B, C)".into(),
                    ));
                }
                let a_elems = buffers[0].size() / std::mem::size_of::<f32>();
                let b_elems = buffers[1].size() / std::mem::size_of::<f32>();
                let c_elems = buffers[2].size() / std::mem::size_of::<f32>();

                if c_elems == 0 {
                    return Ok(());
                }

                let k_sq = (a_elems * b_elems) / c_elems;
                let k = (k_sq as f64).sqrt() as usize;
                if k == 0 {
                    return Err(GpuError::ShapeMismatch(
                        "matmul: cannot derive K from buffer sizes".into(),
                    ));
                }
                let m = a_elems / k;
                let n = b_elems / k;

                let mut a_ptr = dev_ptrs[0];
                let mut b_ptr = dev_ptrs[1];
                let mut c_ptr = dev_ptrs[2];
                let mut m_val = m as u32;
                let mut k_val = k as u32;
                let mut n_val = n as u32;

                let mut args: Vec<*mut c_void> = vec![
                    &mut a_ptr as *mut u64 as *mut c_void,
                    &mut b_ptr as *mut u64 as *mut c_void,
                    &mut c_ptr as *mut u64 as *mut c_void,
                    &mut m_val as *mut u32 as *mut c_void,
                    &mut k_val as *mut u32 as *mut c_void,
                    &mut n_val as *mut u32 as *mut c_void,
                ];

                let grid = (n.div_ceil(16) as u32, m.div_ceil(16) as u32, 1);
                let block = (16, 16, 1);

                drop(kernels);
                self.launch_kernel(function, grid, block, &mut args)
            }
            CudaKernelKind::CodebookDot => {
                // Kernel: (query_ptr, indices_ptr, codebook_ptr, scores_ptr, n_tokens, dim)
                // Buffers: [query(f32*dim), indices(u8*n_tokens*dim), codebook(f32*2^b), scores(f32*n_tokens)]
                if dev_ptrs.len() < 4 {
                    return Err(GpuError::BackendError(
                        "codebook_dot requires 4 buffers (query, indices, codebook, scores)".into(),
                    ));
                }
                let query_elems = buffers[0].size() / std::mem::size_of::<f32>();
                let dim = query_elems;
                let scores_elems = buffers[3].size() / std::mem::size_of::<f32>();
                let n_tokens = scores_elems;

                let mut q_ptr = dev_ptrs[0];
                let mut idx_ptr = dev_ptrs[1];
                let mut cb_ptr = dev_ptrs[2];
                let mut sc_ptr = dev_ptrs[3];
                let mut n_tok = n_tokens as u32;
                let mut dim_val = dim as u32;

                let mut args: Vec<*mut c_void> = vec![
                    &mut q_ptr as *mut u64 as *mut c_void,
                    &mut idx_ptr as *mut u64 as *mut c_void,
                    &mut cb_ptr as *mut u64 as *mut c_void,
                    &mut sc_ptr as *mut u64 as *mut c_void,
                    &mut n_tok as *mut u32 as *mut c_void,
                    &mut dim_val as *mut u32 as *mut c_void,
                ];

                let grid = (n_tokens.div_ceil(256) as u32, 1, 1);
                let block = (256, 1, 1);

                drop(kernels);
                self.launch_kernel(function, grid, block, &mut args)
            }
            CudaKernelKind::Custom => {
                // Pass buffer device pointers as kernel arguments
                let mut ptrs: Vec<u64> = dev_ptrs;
                let mut args: Vec<*mut c_void> = ptrs
                    .iter_mut()
                    .map(|p| p as *mut u64 as *mut c_void)
                    .collect();

                let block = (
                    kernel.workgroup_size().x,
                    kernel.workgroup_size().y,
                    kernel.workgroup_size().z,
                );

                drop(kernels);
                self.launch_kernel(function, workgroups, block, &mut args)
            }
        }
    }
}

impl Drop for CudaDevice {
    fn drop(&mut self) {
        // Clean up loaded modules
        if let Ok(kernels) = self.kernels.lock() {
            // SAFETY: destroying CUDA modules we created
            if let Ok(lib) = unsafe { Library::new("libcuda.so") } {
                if let Ok(cu_module_unload) = unsafe {
                    lib.get::<unsafe extern "C" fn(*mut c_void) -> i32>(b"cuModuleUnload")
                } {
                    for (_id, kdata) in kernels.iter() {
                        if !kdata._module.is_null() {
                            // SAFETY: module was created by cuModuleLoadData in compile_kernel
                            unsafe { cu_module_unload(kdata._module) };
                        }
                    }
                }
            }
        }

        // Destroy CUDA stream
        if !self.stream.is_null() {
            // SAFETY: destroying CUDA stream we created
            if let Ok(lib) = unsafe { Library::new("libcuda.so") } {
                if let Ok(cu_stream_destroy) = unsafe {
                    lib.get::<unsafe extern "C" fn(*mut c_void) -> i32>(b"cuStreamDestroy_v2")
                } {
                    unsafe { cu_stream_destroy(self.stream) };
                }
            }
        }

        // Destroy CUDA context
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enumerate_devices_does_not_panic() {
        // On systems without CUDA, enumerate_devices should return an error, never panic.
        let result = enumerate_devices();
        // Either Ok (CUDA present) or Err (CUDA absent) — both are valid.
        if let Ok(devices) = &result {
            assert!(
                !devices.is_empty(),
                "if Ok, should have at least one device"
            );
        }
        // Err case expected when CUDA is not installed; no assertion needed.
    }

    #[test]
    fn enumerate_returns_cuda_backend_type() {
        // If CUDA is available, all returned devices should report GpuBackend::Cuda.
        if let Ok(devices) = enumerate_devices() {
            for dev in &devices {
                assert_eq!(
                    dev.info().backend,
                    GpuBackend::Cuda,
                    "CUDA enumerate should only return CUDA devices"
                );
            }
        }
    }

    #[test]
    fn enumerate_devices_info_fields_valid() {
        // If CUDA is available, device info should have non-empty name and nonzero memory.
        if let Ok(devices) = enumerate_devices() {
            for dev in &devices {
                let info = dev.info();
                assert!(!info.name.is_empty(), "device name should not be empty");
                assert!(info.memory > 0, "device should report nonzero memory");
                assert!(info.compute_units > 0, "device should have compute units");
                assert!(
                    info.max_workgroup_size > 0,
                    "max workgroup size should be positive"
                );
            }
        }
    }

    #[test]
    fn cuda_compile_ptx_kernel() {
        // If CUDA is available, compiling a PTX kernel should succeed.
        if let Ok(devices) = enumerate_devices() {
            for dev in &devices {
                let ptx = r#".version 8.3
.target sm_89
.address_size 64
.visible .entry test_kernel(
    .param .u64 param_A
) {
    ret;
}
"#;
                let kernel = dev
                    .compile_kernel(&KernelSource::Ptx(ptx.into()))
                    .expect("PTX kernel compilation should succeed");
                assert_eq!(kernel.name(), "test_kernel");
            }
        }
    }

    #[test]
    fn cuda_compile_builtin_kernel() {
        // If CUDA is available, compiling a builtin kernel should succeed.
        if let Ok(devices) = enumerate_devices() {
            for dev in &devices {
                let kernel = dev
                    .compile_kernel(&KernelSource::Builtin(BuiltinKernel::VectorAdd))
                    .expect("builtin kernel compilation should succeed");
                assert_eq!(kernel.name(), "vector_add");
                assert_eq!(kernel.num_bindings(), 3);
            }
        }
    }

    #[test]
    fn cuda_compile_wgsl_rejected() {
        // CUDA backend should reject WGSL kernel sources.
        if let Ok(devices) = enumerate_devices() {
            for dev in &devices {
                let result =
                    dev.compile_kernel(&KernelSource::Wgsl("@compute fn main() {}".into()));
                assert!(result.is_err(), "CUDA should reject WGSL kernel sources");
            }
        }
    }

    #[test]
    fn cuda_upload_size_mismatch() {
        // If CUDA is available, uploading wrong-sized data should error.
        if let Ok(devices) = enumerate_devices() {
            if let Some(dev) = devices.first() {
                if let Ok(buf) = dev.create_buffer(16) {
                    let result = dev.upload(&buf, &[1, 2, 3]); // wrong size
                    assert!(result.is_err(), "upload with mismatched size should fail");
                }
            }
        }
    }

    #[test]
    fn cuda_download_size_mismatch() {
        // If CUDA is available, downloading into wrong-sized buffer should error.
        if let Ok(devices) = enumerate_devices() {
            if let Some(dev) = devices.first() {
                if let Ok(buf) = dev.create_buffer(16) {
                    let mut dst = vec![0u8; 8]; // wrong size
                    let result = dev.download(&buf, &mut dst);
                    assert!(result.is_err(), "download with mismatched size should fail");
                }
            }
        }
    }

    #[test]
    fn cuda_vector_add_e2e() {
        // End-to-end: compile vector_add kernel, execute on real data, verify result.
        let devices = match enumerate_devices() {
            Ok(d) => d,
            Err(_) => return, // no CUDA
        };
        let dev = &devices[0];

        let n = 1024usize;
        let byte_size = n * std::mem::size_of::<f32>();

        // Prepare host data
        let a_data: Vec<f32> = (0..n).map(|i| i as f32).collect();
        let b_data: Vec<f32> = (0..n).map(|i| (n - i) as f32).collect();
        let a_bytes: Vec<u8> = a_data.iter().flat_map(|f| f.to_le_bytes()).collect();
        let b_bytes: Vec<u8> = b_data.iter().flat_map(|f| f.to_le_bytes()).collect();

        // Allocate + upload
        let a_buf = dev.create_buffer(byte_size).expect("alloc A");
        let b_buf = dev.create_buffer(byte_size).expect("alloc B");
        let c_buf = dev.create_buffer(byte_size).expect("alloc C");
        dev.upload(&a_buf, &a_bytes).expect("upload A");
        dev.upload(&b_buf, &b_bytes).expect("upload B");

        // Compile + execute
        let kernel = dev
            .compile_kernel(&KernelSource::Builtin(BuiltinKernel::VectorAdd))
            .expect("compile vector_add");
        dev.execute(&kernel, (4, 1, 1), &[&a_buf, &b_buf, &c_buf])
            .expect("execute vector_add");

        // Download + verify
        let mut c_bytes = vec![0u8; byte_size];
        dev.download(&c_buf, &mut c_bytes).expect("download C");
        let c_data: Vec<f32> = c_bytes
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();

        for i in 0..n {
            let expected = a_data[i] + b_data[i];
            assert!(
                (c_data[i] - expected).abs() < 1e-3,
                "mismatch at {i}: got {} expected {}",
                c_data[i],
                expected
            );
        }
    }

    #[test]
    fn cuda_matmul_e2e() {
        // End-to-end: compile matmul kernel, execute C = A @ B, verify against CPU.
        let devices = match enumerate_devices() {
            Ok(d) => d,
            Err(_) => return, // no CUDA
        };
        let dev = &devices[0];

        let m = 4usize;
        let k = 3usize;
        let n = 2usize;

        // A [4x3], B [3x2]
        let a_data: Vec<f32> = vec![
            1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0,
        ];
        let b_data: Vec<f32> = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];

        let a_bytes: Vec<u8> = a_data.iter().flat_map(|f| f.to_le_bytes()).collect();
        let b_bytes: Vec<u8> = b_data.iter().flat_map(|f| f.to_le_bytes()).collect();

        let a_buf = dev.create_buffer(m * k * 4).expect("alloc A");
        let b_buf = dev.create_buffer(k * n * 4).expect("alloc B");
        let c_buf = dev.create_buffer(m * n * 4).expect("alloc C");
        dev.upload(&a_buf, &a_bytes).expect("upload A");
        dev.upload(&b_buf, &b_bytes).expect("upload B");

        let kernel = dev
            .compile_kernel(&KernelSource::Builtin(BuiltinKernel::Matmul))
            .expect("compile matmul");
        dev.execute(&kernel, (1, 1, 1), &[&a_buf, &b_buf, &c_buf])
            .expect("execute matmul");

        let mut c_bytes = vec![0u8; m * n * 4];
        dev.download(&c_buf, &mut c_bytes).expect("download C");
        let c_data: Vec<f32> = c_bytes
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();

        // Expected: C = A @ B (manual calculation)
        // C[0,0] = 1*1 + 2*3 + 3*5 = 22
        // C[0,1] = 1*2 + 2*4 + 3*6 = 28
        // C[1,0] = 4*1 + 5*3 + 6*5 = 49
        // C[1,1] = 4*2 + 5*4 + 6*6 = 64
        // C[2,0] = 7*1 + 8*3 + 9*5 = 76
        // C[2,1] = 7*2 + 8*4 + 9*6 = 100
        // C[3,0] = 10*1 + 11*3 + 12*5 = 103
        // C[3,1] = 10*2 + 11*4 + 12*6 = 136
        let expected: Vec<f32> = vec![22.0, 28.0, 49.0, 64.0, 76.0, 100.0, 103.0, 136.0];

        for i in 0..expected.len() {
            assert!(
                (c_data[i] - expected[i]).abs() < 1e-2,
                "matmul mismatch at {i}: got {} expected {}",
                c_data[i],
                expected[i]
            );
        }
    }

    #[test]
    fn cuda_relu_e2e() {
        // End-to-end: ReLU activation on GPU.
        let devices = match enumerate_devices() {
            Ok(d) => d,
            Err(_) => return,
        };
        let dev = &devices[0];

        let data: Vec<f32> = vec![-3.0, -1.0, 0.0, 0.5, 2.0, -0.1, 10.0, -100.0];
        let n = data.len();
        let bytes: Vec<u8> = data.iter().flat_map(|f| f.to_le_bytes()).collect();

        let in_buf = dev.create_buffer(n * 4).expect("alloc in");
        let out_buf = dev.create_buffer(n * 4).expect("alloc out");
        dev.upload(&in_buf, &bytes).expect("upload");

        let kernel = dev
            .compile_kernel(&KernelSource::Builtin(BuiltinKernel::Relu))
            .expect("compile relu");
        dev.execute(&kernel, (1, 1, 1), &[&in_buf, &out_buf])
            .expect("execute relu");

        let mut out_bytes = vec![0u8; n * 4];
        dev.download(&out_buf, &mut out_bytes).expect("download");
        let result: Vec<f32> = out_bytes
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();

        let expected: Vec<f32> = vec![0.0, 0.0, 0.0, 0.5, 2.0, 0.0, 10.0, 0.0];
        for i in 0..n {
            assert!(
                (result[i] - expected[i]).abs() < 1e-3,
                "relu mismatch at {i}: got {} expected {}",
                result[i],
                expected[i]
            );
        }
    }

    #[test]
    fn cuda_matmul_128x128_tiled() {
        // Verify tiled matmul at a size that requires multiple tiles (128/16 = 8 tiles).
        let devices = match enumerate_devices() {
            Ok(d) => d,
            Err(_) => return,
        };
        let dev = &devices[0];

        let m = 128usize;
        let k = 64usize;
        let n = 128usize;

        // Generate deterministic data
        let a_data: Vec<f32> = (0..m * k).map(|i| ((i % 7) as f32) * 0.1).collect();
        let b_data: Vec<f32> = (0..k * n).map(|i| ((i % 11) as f32) * 0.1).collect();

        let a_bytes: Vec<u8> = a_data.iter().flat_map(|f| f.to_le_bytes()).collect();
        let b_bytes: Vec<u8> = b_data.iter().flat_map(|f| f.to_le_bytes()).collect();

        let a_buf = dev.create_buffer(m * k * 4).expect("alloc A");
        let b_buf = dev.create_buffer(k * n * 4).expect("alloc B");
        let c_buf = dev.create_buffer(m * n * 4).expect("alloc C");
        dev.upload(&a_buf, &a_bytes).expect("upload A");
        dev.upload(&b_buf, &b_bytes).expect("upload B");

        let kernel = dev
            .compile_kernel(&KernelSource::Builtin(BuiltinKernel::Matmul))
            .expect("compile matmul");
        dev.execute(&kernel, (8, 8, 1), &[&a_buf, &b_buf, &c_buf])
            .expect("execute matmul 128x128");

        let mut c_bytes = vec![0u8; m * n * 4];
        dev.download(&c_buf, &mut c_bytes).expect("download C");
        let c_gpu: Vec<f32> = c_bytes
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();

        // Compute CPU reference
        let mut c_cpu = vec![0.0f32; m * n];
        for row in 0..m {
            for col in 0..n {
                let mut sum = 0.0f32;
                for kk in 0..k {
                    sum += a_data[row * k + kk] * b_data[kk * n + col];
                }
                c_cpu[row * n + col] = sum;
            }
        }

        // Verify GPU matches CPU (f32 tolerance)
        let mut max_err: f32 = 0.0;
        for i in 0..m * n {
            let err = (c_gpu[i] - c_cpu[i]).abs();
            if err > max_err {
                max_err = err;
            }
            assert!(
                err < 0.1, // f32 accumulation tolerance
                "matmul 128x64 @ 64x128 mismatch at {i}: gpu={} cpu={} err={}",
                c_gpu[i],
                c_cpu[i],
                err,
            );
        }
    }

    #[test]
    fn cuda_all_builtin_kernels_compile() {
        // Verify all builtin kernels compile without error.
        let devices = match enumerate_devices() {
            Ok(d) => d,
            Err(_) => return,
        };
        let dev = &devices[0];

        let builtins = [
            BuiltinKernel::VectorAdd,
            BuiltinKernel::VectorSub,
            BuiltinKernel::VectorMul,
            BuiltinKernel::VectorDiv,
            BuiltinKernel::Relu,
            BuiltinKernel::Sigmoid,
            BuiltinKernel::Softmax,
            BuiltinKernel::Matmul,
            BuiltinKernel::CodebookDot,
        ];

        for builtin in &builtins {
            let result = dev.compile_kernel(&KernelSource::Builtin(*builtin));
            assert!(
                result.is_ok(),
                "failed to compile builtin {:?}: {:?}",
                builtin,
                result.err()
            );
        }
    }

    #[test]
    fn ptx_generation_contains_entry() {
        // Verify generated PTX contains proper .entry directive.
        let builtins = [
            (BuiltinKernel::VectorAdd, "vector_add"),
            (BuiltinKernel::Matmul, "matmul"),
            (BuiltinKernel::CodebookDot, "codebook_dot"),
            (BuiltinKernel::Relu, "relu"),
        ];
        for (builtin, expected_name) in &builtins {
            let (name, ptx) = generate_builtin_ptx(*builtin);
            assert_eq!(&name, expected_name);
            assert!(
                ptx.contains(&format!(".entry {expected_name}")),
                "PTX for {} should contain .entry {}",
                expected_name,
                expected_name
            );
            assert!(ptx.contains(".version 8.3"));
            assert!(ptx.contains(".target sm_89"));
        }
    }

    #[test]
    fn cuda_codebook_dot_e2e() {
        // FajarQuant × CUDA: codebook dot product on GPU.
        let devices = match enumerate_devices() {
            Ok(d) => d,
            Err(_) => return,
        };
        let dev = &devices[0];

        let dim = 4usize;
        let n_tokens = 3usize;
        let n_centroids = 4usize;

        // Query: [1.0, 2.0, 3.0, 4.0]
        let query: Vec<f32> = vec![1.0, 2.0, 3.0, 4.0];
        // Codebook: [0.1, 0.2, 0.3, 0.4]
        let codebook: Vec<f32> = vec![0.1, 0.2, 0.3, 0.4];
        // Token 0: [0,1,2,3] → 1*0.1+2*0.2+3*0.3+4*0.4 = 3.0
        // Token 1: [3,3,3,3] → (1+2+3+4)*0.4 = 4.0
        // Token 2: [0,0,0,0] → (1+2+3+4)*0.1 = 1.0
        let indices: Vec<u8> = vec![0, 1, 2, 3, 3, 3, 3, 3, 0, 0, 0, 0];

        let q_bytes: Vec<u8> = query.iter().flat_map(|f| f.to_le_bytes()).collect();
        let cb_bytes: Vec<u8> = codebook.iter().flat_map(|f| f.to_le_bytes()).collect();

        let q_buf = dev.create_buffer(dim * 4).expect("alloc query");
        let idx_buf = dev.create_buffer(n_tokens * dim).expect("alloc indices");
        let cb_buf = dev.create_buffer(n_centroids * 4).expect("alloc codebook");
        let sc_buf = dev.create_buffer(n_tokens * 4).expect("alloc scores");

        dev.upload(&q_buf, &q_bytes).expect("upload query");
        dev.upload(&idx_buf, &indices).expect("upload indices");
        dev.upload(&cb_buf, &cb_bytes).expect("upload codebook");

        let kernel = dev
            .compile_kernel(&KernelSource::Builtin(BuiltinKernel::CodebookDot))
            .expect("compile codebook_dot");
        dev.execute(&kernel, (1, 1, 1), &[&q_buf, &idx_buf, &cb_buf, &sc_buf])
            .expect("execute codebook_dot");

        let mut sc_bytes = vec![0u8; n_tokens * 4];
        dev.download(&sc_buf, &mut sc_bytes).expect("download");
        let scores: Vec<f32> = sc_bytes
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();

        assert!((scores[0] - 3.0).abs() < 0.01, "token 0: {}", scores[0]);
        assert!((scores[1] - 4.0).abs() < 0.01, "token 1: {}", scores[1]);
        assert!((scores[2] - 1.0).abs() < 0.01, "token 2: {}", scores[2]);
    }
}
