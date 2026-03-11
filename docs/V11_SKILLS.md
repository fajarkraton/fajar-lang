# Skills — Fajar Lang v1.1 "Ascension"

> Implementation patterns and technical recipes for v1.1 features.
> Read this BEFORE implementing complex tasks.
> Reference: `V11_PLAN.md`, `V11_TASKS.md`, `V11_WORKFLOW.md`
> Updated: 2026-03-11

---

## 1. Hardware Detection Patterns

### 1.1 CPUID Probing (x86_64)

```rust
/// Safe CPUID wrapper — no unsafe in caller code
pub struct CpuId {
    max_leaf: u32,
    vendor: CpuVendor,
}

impl CpuId {
    pub fn new() -> Self {
        // SAFETY: CPUID is always available on x86_64
        let result = unsafe { core::arch::x86_64::__cpuid(0) };
        Self {
            max_leaf: result.eax,
            vendor: CpuVendor::from_regs(result.ebx, result.ecx, result.edx),
        }
    }

    pub fn has_avx512f(&self) -> bool {
        if self.max_leaf < 7 { return false; }
        // SAFETY: Leaf 7 available (checked above)
        let result = unsafe { core::arch::x86_64::__cpuid_count(7, 0) };
        (result.ebx >> 16) & 1 == 1  // AVX-512F bit
    }

    pub fn has_amx_bf16(&self) -> bool {
        if self.max_leaf < 7 { return false; }
        let result = unsafe { core::arch::x86_64::__cpuid_count(7, 0) };
        (result.edx >> 22) & 1 == 1  // AMX-BF16 bit
    }

    pub fn has_amx_int8(&self) -> bool {
        if self.max_leaf < 7 { return false; }
        let result = unsafe { core::arch::x86_64::__cpuid_count(7, 0) };
        (result.edx >> 25) & 1 == 1  // AMX-INT8 bit
    }
}
```

**Pattern:** All CPUID calls wrapped in safe API. Feature checks return bool. Caller never writes `unsafe`.

### 1.2 ARM Feature Detection

```rust
/// ARM feature detection via /proc/cpuinfo or MRS instruction
#[cfg(target_arch = "aarch64")]
pub fn detect_arm_features() -> ArmFeatures {
    // Read /proc/cpuinfo for feature flags
    let cpuinfo = std::fs::read_to_string("/proc/cpuinfo").unwrap_or_default();
    ArmFeatures {
        neon: cpuinfo.contains("neon") || cpuinfo.contains("asimd"),
        sve: cpuinfo.contains(" sve "),
        sve2: cpuinfo.contains("sve2"),
        bf16: cpuinfo.contains("bf16"),
        i8mm: cpuinfo.contains("i8mm"),
        dotprod: cpuinfo.contains("asimddp"),
    }
}
```

**Pattern:** Feature detection varies per architecture. Use `#[cfg(target_arch)]` for platform-specific code. Always provide fallback for unknown architectures.

### 1.3 GPU Discovery via CUDA

```rust
/// CUDA device discovery — behind `cuda` feature flag
#[cfg(feature = "cuda")]
pub fn discover_cuda_devices() -> Vec<GpuInfo> {
    // Link to cuda runtime
    extern "C" {
        fn cudaGetDeviceCount(count: *mut i32) -> i32;
        fn cudaGetDeviceProperties(prop: *mut CudaDeviceProp, device: i32) -> i32;
    }

    let mut count = 0i32;
    // SAFETY: count is valid pointer, CUDA runtime loaded
    unsafe { cudaGetDeviceCount(&mut count); }

    (0..count).map(|i| {
        let mut prop = CudaDeviceProp::default();
        // SAFETY: prop is valid, device index in range
        unsafe { cudaGetDeviceProperties(&mut prop, i); }
        GpuInfo {
            name: prop.name_string(),
            compute_capability: (prop.major, prop.minor),
            memory_bytes: prop.totalGlobalMem,
            cuda_cores: estimate_cores(prop.major, prop.minor, prop.multiProcessorCount),
            tensor_cores: estimate_tensor_cores(prop.major, prop.minor, prop.multiProcessorCount),
            supports_fp4: prop.major >= 10,  // Blackwell+
            supports_fp8: prop.major >= 9,   // Hopper+
        }
    }).collect()
}

#[cfg(not(feature = "cuda"))]
pub fn discover_cuda_devices() -> Vec<GpuInfo> {
    Vec::new()  // No CUDA support compiled in
}
```

**Pattern:** Hardware-specific code behind feature flags. Always provide no-op fallback for when hardware isn't available.

### 1.4 NPU Detection

```rust
/// Check for Intel NPU (OpenVINO) or AMD XDNA
pub fn detect_npu() -> Option<NpuInfo> {
    // Intel NPU: check for /dev/accel/accel0 or intel_vpu module
    if std::path::Path::new("/dev/accel/accel0").exists() {
        if let Ok(driver) = std::fs::read_to_string("/sys/class/accel/accel0/device/driver/module/drivers") {
            if driver.contains("intel_vpu") {
                return Some(NpuInfo {
                    vendor: NpuVendor::Intel,
                    generation: detect_intel_npu_gen(),
                    tops: estimate_intel_npu_tops(),
                    formats: vec![DataFormat::INT8, DataFormat::FP16],
                });
            }
        }
    }

    // AMD XDNA: check for amdxdna driver
    if std::path::Path::new("/sys/module/amdxdna").exists() {
        return Some(NpuInfo {
            vendor: NpuVendor::Amd,
            generation: detect_amd_xdna_gen(),
            tops: 50.0,  // XDNA 2
            formats: vec![DataFormat::INT8, DataFormat::FP16, DataFormat::BF16],
        });
    }

    None
}
```

**Pattern:** NPU detection via filesystem probing (sysfs/devfs). No direct hardware access needed — driver exposes info.

---

## 2. Numeric Format Patterns

### 2.1 FP8 Implementation

```rust
/// FP8 E5M2 format (IEEE-like: 5-bit exponent, 2-bit mantissa)
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FP8E5M2(u8);

impl FP8E5M2 {
    pub const ZERO: Self = Self(0);
    pub const ONE: Self = Self(0b0_01111_00);  // exp=15 (bias=15), mantissa=1.00
    pub const NAN: Self = Self(0b0_11111_11);
    pub const INF: Self = Self(0b0_11111_00);

    pub fn from_f32(val: f32) -> Self {
        let bits = val.to_bits();
        let sign = (bits >> 31) & 1;
        let exp = ((bits >> 23) & 0xFF) as i32 - 127;  // Unbias from f32
        let mantissa = bits & 0x7FFFFF;

        // Rebias to FP8E5M2 (bias = 15)
        let new_exp = exp + 15;
        if new_exp >= 31 { return Self((sign as u8) << 7 | 0b11111_00); }  // Inf
        if new_exp <= 0 { return Self((sign as u8) << 7); }  // Zero (flush subnormals)

        // Round mantissa from 23 to 2 bits (round-to-nearest-even)
        let rounded = (mantissa + (1 << 20)) >> 21;  // Round bit at position 20
        Self((sign as u8) << 7 | (new_exp as u8) << 2 | (rounded as u8) & 0x3)
    }

    pub fn to_f32(self) -> f32 {
        let sign = (self.0 >> 7) & 1;
        let exp = ((self.0 >> 2) & 0x1F) as i32 - 15 + 127;
        let mantissa = ((self.0 & 0x3) as u32) << 21;
        f32::from_bits((sign as u32) << 31 | (exp as u32) << 23 | mantissa)
    }
}
```

### 2.2 FP4 with Two-Level Scaling (NVFP4)

```rust
/// FP4 E2M1 format with NVIDIA two-level scaling
/// Block of 32 FP4 values shares one FP8 scale factor
/// Entire tensor has one FP32 global scale
pub struct FP4Block {
    packed: [u8; 16],          // 32 FP4 values packed (2 per byte)
    block_scale: FP8E4M3,     // Per-block scale (FP8)
}

pub struct FP4Tensor {
    blocks: Vec<FP4Block>,
    global_scale: f32,         // Per-tensor scale
    rows: usize,
    cols: usize,
}

impl FP4Tensor {
    pub fn from_f32_tensor(tensor: &Array2<f64>, block_size: usize) -> Self {
        // 1. Compute global scale (max absolute value)
        let max_abs = tensor.iter().map(|v| v.abs()).fold(0.0f64, f64::max);
        let global_scale = max_abs as f32 / 6.0;  // FP4 E2M1 max representable = 6.0

        // 2. For each block of `block_size` elements:
        //    a. Compute block_scale = max_abs(block) / (global_scale * 6.0)
        //    b. Quantize each element: fp4_val = round(element / (global_scale * block_scale))
        //    c. Pack pairs of FP4 into bytes
        todo!()
    }
}
```

### 2.3 BF16 (Brain Float)

```rust
/// BF16: same exponent range as f32, reduced mantissa
#[derive(Clone, Copy, Debug)]
pub struct BF16(u16);

impl BF16 {
    /// Fast conversion from f32 — just truncate lower 16 bits
    pub fn from_f32(val: f32) -> Self {
        Self((val.to_bits() >> 16) as u16)
    }

    /// Fast conversion to f32 — zero-extend
    pub fn to_f32(self) -> f32 {
        f32::from_bits((self.0 as u32) << 16)
    }
}

// BF16 arithmetic always goes through f32
impl std::ops::Add for BF16 {
    type Output = BF16;
    fn add(self, rhs: BF16) -> BF16 {
        BF16::from_f32(self.to_f32() + rhs.to_f32())
    }
}
```

**Pattern:** BF16 is the simplest format — truncate/extend the upper 16 bits of f32. Same exponent range means same dynamic range.

### 2.4 Structured Sparsity (4:2)

```rust
/// 4:2 structured sparsity: exactly 2 non-zero values per group of 4
pub struct SparseTensor {
    values: Vec<f32>,         // Only non-zero values (50% of dense)
    indices: Vec<u8>,          // 2-bit index per value within group
    rows: usize,
    cols: usize,
}

impl SparseTensor {
    /// Prune dense tensor to 4:2 pattern (keep 2 largest per group)
    pub fn from_dense(dense: &Array2<f64>) -> Self {
        let mut values = Vec::new();
        let mut indices = Vec::new();

        for row in dense.rows() {
            for chunk in row.iter().collect::<Vec<_>>().chunks(4) {
                // Find top-2 by magnitude
                let mut indexed: Vec<(usize, f64)> = chunk.iter().enumerate()
                    .map(|(i, &&v)| (i, v.abs())).collect();
                indexed.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

                // Keep top 2
                let (i0, _) = indexed[0];
                let (i1, _) = indexed[1];
                values.push(*chunk[i0] as f32);
                values.push(*chunk[i1] as f32);
                indices.push(pack_indices(i0, i1));
            }
        }

        Self { values, indices, rows: dense.nrows(), cols: dense.ncols() }
    }
}
```

---

## 3. NPU Integration Patterns

### 3.1 @npu Context in Analyzer

```rust
// In type_check.rs — add NpuContext alongside KernelContext and DeviceContext
fn check_context_violation(&self, context: Context, operation: &str) -> Option<SemanticError> {
    match context {
        Context::Npu => {
            match operation {
                "raw_pointer" => Some(SemanticError::NE001_RawPointerInNpu),
                "heap_alloc" => Some(SemanticError::NE002_HeapAllocInNpu),
                "file_io" => Some(SemanticError::NE003_InvalidNpuOp),
                _ => None,
            }
        }
        // ... existing contexts
    }
}
```

### 3.2 OpenVINO Inference Pattern

```rust
/// Intel NPU inference via OpenVINO
#[cfg(feature = "npu")]
pub struct OpenVinoInference {
    core: ov_core_t,
    model: ov_model_t,
}

impl OpenVinoInference {
    pub fn load_model(model_path: &str, device: &str) -> Result<Self, NpuError> {
        // 1. Create OpenVINO core
        // 2. Read model from ONNX/IR
        // 3. Compile model for target device ("NPU", "CPU", "GPU")
        // 4. Create inference request
        todo!()
    }

    pub fn infer(&self, input: &Tensor) -> Result<Tensor, NpuError> {
        // 1. Set input tensor (convert Fajar Tensor → OpenVINO tensor)
        // 2. Run inference
        // 3. Get output tensor (convert OpenVINO tensor → Fajar Tensor)
        todo!()
    }
}
```

---

## 4. Jetson Thor BSP Patterns

### 4.1 Thor Platform Detection

```rust
/// Detect Jetson module type
pub fn detect_jetson_module() -> Option<JetsonModule> {
    // Read /proc/device-tree/compatible or /etc/nv_tegra_release
    let compatible = std::fs::read_to_string("/proc/device-tree/compatible").ok()?;

    if compatible.contains("nvidia,jetson-agx-thor") || compatible.contains("nvidia,t5000") {
        Some(JetsonModule::T5000)
    } else if compatible.contains("nvidia,t4000") {
        Some(JetsonModule::T4000)
    } else if compatible.contains("nvidia,jetson-agx-orin") {
        Some(JetsonModule::AgxOrin)
    } else if compatible.contains("nvidia,jetson-orin-nx") {
        Some(JetsonModule::OrinNx)
    } else if compatible.contains("nvidia,jetson-orin-nano") {
        Some(JetsonModule::OrinNano)
    } else {
        None
    }
}
```

### 4.2 MIG Partition Management

```rust
/// Multi-Instance GPU management for Jetson Thor T5000
pub struct MigManager {
    device: i32,
}

impl MigManager {
    pub fn create_partitions(&self, count: usize) -> Result<Vec<MigPartition>, GpuError> {
        // nvmlDeviceCreateGpuInstance + nvmlGpuInstanceCreateComputeInstance
        // Returns handles to isolated GPU partitions
        todo!()
    }

    pub fn run_on_partition(&self, partition: &MigPartition, model: &Model, input: &Tensor)
        -> Result<Tensor, GpuError>
    {
        // Set CUDA_VISIBLE_DEVICES to partition, run inference
        todo!()
    }
}
```

---

## 5. AVX-512 / AMX Codegen Patterns

### 5.1 AMX Tile Matrix Multiply

```rust
/// AMX tile configuration and matrix multiply
/// Each tile register is 1KB (max 16 rows × 64 bytes)
/// 8 tile registers available (tmm0-tmm7)

// TILECFG structure for palette 1
#[repr(C, align(64))]
pub struct TileConfig {
    palette: u8,           // 1 = palette 1
    start_row: u8,         // restart row (0)
    _reserved: [u8; 14],
    colsb: [u16; 8],      // bytes per row for each tmm
    _reserved2: [u16; 8],
    rows: [u8; 8],         // rows for each tmm
    _reserved3: [u8; 8],
}

/// INT8 matrix multiply: C[16×16] += A[16×64] × B[64×16]
/// Uses tmm0(C), tmm1(A), tmm2(B)
pub unsafe fn amx_int8_matmul_16x16(a: &[i8; 1024], b: &[i8; 1024], c: &mut [i32; 256]) {
    // 1. Configure tiles
    let cfg = TileConfig::for_int8_16x16();
    core::arch::x86_64::_tile_loadconfig(&cfg as *const _ as *const u8);

    // 2. Load data into tiles
    core::arch::x86_64::_tile_loadd(1, a.as_ptr(), 64);  // A into tmm1
    core::arch::x86_64::_tile_loadd(2, b.as_ptr(), 16);  // B into tmm2
    core::arch::x86_64::_tile_zero(0);                     // Zero tmm0 (accumulator)

    // 3. Matrix multiply: tmm0 += tmm1 × tmm2
    core::arch::x86_64::_tile_dpbssd(0, 1, 2);  // INT8 signed×signed dot product

    // 4. Store result
    core::arch::x86_64::_tile_stored(0, c.as_mut_ptr(), 64);

    // 5. Release tiles
    core::arch::x86_64::_tile_release();
}
```

### 5.2 AVX-512 VNNI INT8 Dot Product

```rust
/// AVX-512 VNNI: 64 INT8 multiply-accumulate per instruction
#[cfg(target_feature = "avx512vnni")]
pub unsafe fn vnni_dot_product_i8(a: &[i8; 64], b: &[i8; 64]) -> i32 {
    use core::arch::x86_64::*;

    let va = _mm512_loadu_si512(a.as_ptr() as *const __m512i);
    let vb = _mm512_loadu_si512(b.as_ptr() as *const __m512i);
    let zero = _mm512_setzero_si512();

    // vpdpbusd: Multiply unsigned×signed bytes, accumulate to dwords
    let result = _mm512_dpbusd_epi32(zero, va, vb);

    // Horizontal sum of 16 i32 elements
    _mm512_reduce_add_epi32(result)
}
```

### 5.3 Blackwell PTX for Tensor Core

```rust
/// Generate PTX for Blackwell Tensor Core FP4 matmul
pub fn generate_fp4_matmul_ptx() -> String {
    r#"
    .version 8.5
    .target sm_100
    .address_size 64

    // tcgen05.mma for FP4 × FP4 → FP32 accumulation
    // Warp-level: 32 threads cooperate on one MMA
    .reg .b32 %acc<8>;     // 8 accumulator registers per thread
    .reg .b32 %a_frag<2>;  // A matrix fragment (FP4 packed)
    .reg .b32 %b_frag<2>;  // B matrix fragment (FP4 packed)

    // Load from Tensor Memory (TMEM)
    tcgen05.ld.sync.aligned.16x256b.x16.b32 {%a_frag0, %a_frag1}, [%tmem_addr_a];
    tcgen05.ld.sync.aligned.16x256b.x16.b32 {%b_frag0, %b_frag1}, [%tmem_addr_b];

    // Matrix multiply-accumulate
    tcgen05.mma.cta_group::1.kind::f4f4
        {%acc0, %acc1, %acc2, %acc3, %acc4, %acc5, %acc6, %acc7},
        %a_desc, %b_desc, {%acc0, %acc1, %acc2, %acc3, %acc4, %acc5, %acc6, %acc7},
        %scale_d;
    "#.to_string()
}
```

---

## 6. Ecosystem Patterns

### 6.1 Cloudflare Workers Registry API

```javascript
// Worker for registry.fajarlang.dev
export default {
    async fetch(request, env) {
        const url = new URL(request.url);

        switch (url.pathname) {
            case '/api/packages':
                if (request.method === 'POST') return handlePublish(request, env);
                return handleList(request, env);
            case url.pathname.match(/^\/api\/packages\/[\w-]+$/)?.input:
                return handleGetPackage(request, env);
            case '/api/search':
                return handleSearch(request, env);
            default:
                return new Response('Not Found', { status: 404 });
        }
    }
};

async function handlePublish(request, env) {
    // 1. Verify API key (Authorization: Bearer <key>)
    // 2. Parse multipart form (package tarball + metadata)
    // 3. Validate fj.toml (name, version, etc.)
    // 4. Store tarball in R2 bucket
    // 5. Insert metadata into D1 database
    // 6. Return 201 Created
}
```

### 6.2 Wasm Playground Compilation

```rust
// lib.rs with wasm-bindgen
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn compile_and_run(source: &str) -> String {
    // Capture output
    let mut output = Vec::new();

    // Run through pipeline: lex → parse → analyze → eval
    match run_pipeline(source, &mut output) {
        Ok(()) => String::from_utf8(output).unwrap_or_default(),
        Err(e) => format_error_html(&e),
    }
}
```

### 6.3 Monaco Editor Integration

```javascript
// Register Fajar Lang for Monaco
monaco.languages.register({ id: 'fajar' });
monaco.languages.setMonarchTokensProvider('fajar', {
    keywords: ['fn', 'let', 'mut', 'if', 'else', 'while', 'for', 'match',
               'struct', 'enum', 'impl', 'trait', 'return', 'true', 'false',
               'tensor', 'grad', 'loss', 'layer', 'model'],
    annotations: ['@kernel', '@device', '@safe', '@unsafe', '@npu', '@infer'],
    // ... tokenizer rules
});
```

---

## 7. Multi-Accelerator Dispatch Patterns

### 7.1 @infer Annotation Processing

```rust
// In analyzer: validate @infer functions
fn check_infer_context(&self, func: &FnDecl) -> Vec<SemanticError> {
    let mut errors = vec![];

    // @infer functions must be pure (no side effects)
    if func.has_side_effects() {
        errors.push(SemanticError::IE001_InvalidInferOp);
    }

    // Only tensor inputs and outputs
    for param in &func.params {
        if !is_tensor_type(&param.ty) && !is_numeric_type(&param.ty) {
            errors.push(SemanticError::IE001_InvalidInferOp);
        }
    }

    errors
}
```

### 7.2 Runtime Dispatch Chain

```rust
/// Multi-accelerator dispatch with fallback
pub struct DispatchChain {
    devices: Vec<Box<dyn Accelerator>>,
    latency_cache: HashMap<String, (usize, Duration)>,  // fn_name → (device_idx, latency)
}

impl DispatchChain {
    pub fn dispatch(&mut self, fn_name: &str, input: &Tensor) -> Result<Tensor, DispatchError> {
        // Check cache for known-good device
        if let Some(&(idx, _)) = self.latency_cache.get(fn_name) {
            if let Ok(result) = self.devices[idx].infer(fn_name, input) {
                return Ok(result);
            }
        }

        // Try each device in priority order
        for (idx, device) in self.devices.iter().enumerate() {
            match device.infer(fn_name, input) {
                Ok(result) => {
                    // Cache successful device
                    self.latency_cache.insert(fn_name.to_string(), (idx, Duration::ZERO));
                    return Ok(result);
                }
                Err(AccelError::DeviceUnavailable) => continue,
                Err(e) => return Err(DispatchError::from(e)),
            }
        }

        Err(DispatchError::NoDeviceAvailable)
    }
}
```

---

## 8. Testing Patterns for Hardware Features

### 8.1 Mock Hardware Profile

```rust
/// For CI testing — no real hardware needed
pub fn mock_full_profile() -> HardwareProfile {
    HardwareProfile {
        cpu: CpuInfo {
            vendor: CpuVendor::Intel,
            model_name: "Mock Intel Core i9".to_string(),
            avx512: true,
            amx_bf16: true,
            amx_int8: true,
            ..Default::default()
        },
        gpu: Some(GpuInfo {
            name: "Mock NVIDIA RTX 5090".to_string(),
            compute_capability: (12, 0),
            memory_bytes: 32 * 1024 * 1024 * 1024,
            supports_fp4: true,
            supports_fp8: true,
            ..Default::default()
        }),
        npu: Some(NpuInfo {
            vendor: NpuVendor::Intel,
            generation: 5,
            tops: 50.0,
            ..Default::default()
        }),
    }
}

/// CPU-only profile (most CI environments)
pub fn mock_cpu_only_profile() -> HardwareProfile {
    HardwareProfile {
        cpu: CpuInfo::detect(),  // Real CPU detection
        gpu: None,
        npu: None,
    }
}
```

### 8.2 Feature-Gated Tests

```rust
#[test]
fn fp8_matmul_on_gpu() {
    let profile = HardwareProfile::detect();
    if !profile.gpu_supports_fp8() {
        eprintln!("Skipping: no FP8-capable GPU detected");
        return;  // Skip, don't fail
    }
    // Actual GPU test
}

#[test]
fn amx_int8_matmul() {
    if !CpuId::new().has_amx_int8() {
        eprintln!("Skipping: AMX-INT8 not available");
        return;
    }
    // Actual AMX test
}
```

**Pattern:** Hardware tests gracefully skip when hardware is unavailable. Never fail CI because of missing hardware.

---

## 9. Error Code Reference (v1.1 additions)

| Code | Name | Category | Description |
|------|------|----------|-------------|
| NE001 | RawPointerInNpu | NPU Context | Raw pointer access in @npu function |
| NE002 | HeapAllocInNpu | NPU Context | Heap allocation in @npu function |
| NE003 | InvalidNpuOp | NPU Context | Unsupported operation in @npu context |
| IE001 | InvalidInferOp | Dispatch | Non-pure operation in @infer function |
| IE002 | InferDeviceUnavailable | Dispatch | No device available for @infer dispatch |
| HE001 | HardwareDetectionFailed | Hardware | Failed to probe hardware features |
| HE002 | DeviceNotFound | Hardware | Required device (GPU/NPU) not found |
| HE003 | DriverVersionMismatch | Hardware | Driver too old for requested operation |
| QE001 | QuantizationOverflow | Numeric | Value out of range for target format (FP4/FP8) |
| QE002 | SparsityPatternInvalid | Numeric | Tensor doesn't conform to 4:2 sparsity pattern |

---

*V11_SKILLS.md v1.0 | Created 2026-03-11*
