//! SPIR-V Backend — generate SPIR-V binary modules, compute shaders,
//! storage buffers, workgroup memory, barriers, validation, Vulkan dispatch.

use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// S18.1: SPIR-V Module
// ═══════════════════════════════════════════════════════════════════════

/// SPIR-V magic number.
pub const SPIRV_MAGIC: u32 = 0x07230203;

/// SPIR-V version (1.5).
pub const SPIRV_VERSION_1_5: u32 = 0x00010500;

/// A SPIR-V module.
#[derive(Debug, Clone)]
pub struct SpirVModule {
    /// SPIR-V version.
    pub version: u32,
    /// Bound (highest ID + 1).
    pub bound: u32,
    /// Capabilities.
    pub capabilities: Vec<Capability>,
    /// Memory model.
    pub memory_model: MemoryModel,
    /// Entry points.
    pub entry_points: Vec<EntryPoint>,
    /// Type declarations.
    pub types: Vec<SpirVType>,
    /// Variable declarations.
    pub variables: Vec<SpirVVariable>,
    /// Functions.
    pub functions: Vec<SpirVFunction>,
}

impl SpirVModule {
    /// Creates a new compute shader module.
    pub fn new_compute() -> Self {
        Self {
            version: SPIRV_VERSION_1_5,
            bound: 1,
            capabilities: vec![Capability::Shader],
            memory_model: MemoryModel::Glsl450Logical,
            entry_points: Vec::new(),
            types: Vec::new(),
            variables: Vec::new(),
            functions: Vec::new(),
        }
    }

    /// Allocates a new ID.
    pub fn alloc_id(&mut self) -> u32 {
        let id = self.bound;
        self.bound += 1;
        id
    }

    /// Generates a binary word stream (simplified — header only for validation).
    pub fn emit_words(&self) -> Vec<u32> {
        vec![
            SPIRV_MAGIC,
            self.version,
            0, // Generator
            self.bound,
            0, // Schema (reserved)
        ]
    }

    /// V16 G2: Emit a complete SPIR-V binary for a minimal compute shader.
    /// Generates a valid SPIR-V module with:
    /// - OpCapability Shader
    /// - OpMemoryModel Logical GLSL450
    /// - OpEntryPoint GLCompute "main"
    /// - OpExecutionMode LocalSize(1,1,1)
    /// - void main() { return }
    pub fn emit_minimal_compute(&mut self, entry_name: &str) -> Vec<u8> {
        let mut words: Vec<u32> = Vec::new();

        // Allocate IDs
        let id_void = self.alloc_id(); // %1
        let id_void_fn = self.alloc_id(); // %2
        let id_main = self.alloc_id(); // %3
        let id_label = self.alloc_id(); // %4

        // Header
        words.push(SPIRV_MAGIC);
        words.push(SPIRV_VERSION_1_5);
        words.push(0x464A0001); // Generator: "FJ" + version 1
        words.push(self.bound);
        words.push(0); // Schema

        // OpCapability Shader (17 | 2<<16 = 0x00020011)
        words.push(0x00020011);
        words.push(1); // Shader capability

        // OpMemoryModel Logical GLSL450 (14 | 3<<16)
        words.push(0x0003000E);
        words.push(0); // Logical
        words.push(1); // GLSL450

        // OpEntryPoint GLCompute %main "main" (15 | (3+name_words)<<16)
        let name_bytes = entry_name.as_bytes();
        let name_word_count = (name_bytes.len() + 4) / 4; // +1 null +3 round up
        let ep_word_count = 3 + name_word_count;
        words.push(0x0000000F | ((ep_word_count as u32) << 16));
        words.push(5); // GLCompute
        words.push(id_main);
        // Encode name as word-aligned null-terminated string
        let mut name_words = vec![0u32; name_word_count];
        for (i, &b) in name_bytes.iter().enumerate() {
            let word_idx = i / 4;
            let byte_idx = i % 4;
            name_words[word_idx] |= (b as u32) << (byte_idx * 8);
        }
        words.extend_from_slice(&name_words);

        // OpExecutionMode %main LocalSize 1 1 1 (16 | 6<<16)
        words.push(0x00060010);
        words.push(id_main);
        words.push(17); // LocalSize
        words.push(1);
        words.push(1);
        words.push(1);

        // OpTypeVoid %void (19 | 2<<16)
        words.push(0x00020013);
        words.push(id_void);

        // OpTypeFunction %void_fn %void (33 | 3<<16)
        words.push(0x00030021);
        words.push(id_void_fn);
        words.push(id_void);

        // OpFunction %void %main None %void_fn (54 | 5<<16)
        words.push(0x00050036);
        words.push(id_void);
        words.push(id_main);
        words.push(0); // None
        words.push(id_void_fn);

        // OpLabel %label (248 | 2<<16)
        words.push(0x000200F8);
        words.push(id_label);

        // OpReturn (253 | 1<<16)
        words.push(0x000100FD);

        // OpFunctionEnd (56 | 1<<16)
        words.push(0x00010038);

        // Fix bound
        words[3] = self.bound;

        // Convert to bytes (little-endian)
        let mut bytes = Vec::with_capacity(words.len() * 4);
        for w in &words {
            bytes.extend_from_slice(&w.to_le_bytes());
        }
        bytes
    }

    /// V16 G2: Emit SPIR-V binary to file.
    pub fn emit_to_file(&mut self, path: &str, entry_name: &str) -> Result<(), String> {
        let bytes = self.emit_minimal_compute(entry_name);
        std::fs::write(path, &bytes).map_err(|e| format!("Failed to write SPIR-V: {e}"))
    }

    /// Validates the module structure.
    pub fn validate(&self) -> Vec<ValidationError> {
        let mut errors = Vec::new();

        if self.capabilities.is_empty() {
            errors.push(ValidationError {
                message: "Module must declare at least one capability".into(),
            });
        }

        if self.entry_points.is_empty() {
            errors.push(ValidationError {
                message: "Module must have at least one entry point".into(),
            });
        }

        for ep in &self.entry_points {
            if ep.name.is_empty() {
                errors.push(ValidationError {
                    message: "Entry point name must not be empty".into(),
                });
            }
            if ep.execution_model != ExecutionModel::GLCompute {
                errors.push(ValidationError {
                    message: format!(
                        "Entry point '{}' must use GLCompute execution model for compute shaders",
                        ep.name
                    ),
                });
            }
        }

        errors
    }
}

/// SPIR-V validation error.
#[derive(Debug, Clone)]
pub struct ValidationError {
    /// Error message.
    pub message: String,
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SPIR-V validation: {}", self.message)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S18.1 contd: Capabilities & Memory Model
// ═══════════════════════════════════════════════════════════════════════

/// SPIR-V capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Capability {
    /// Shader capability.
    Shader,
    /// Float16 support.
    Float16,
    /// Float64 support.
    Float64,
    /// Int8 support.
    Int8,
    /// Int16 support.
    Int16,
    /// Variable pointers.
    VariablePointers,
}

impl Capability {
    /// Returns the SPIR-V opcode value.
    pub fn value(&self) -> u32 {
        match self {
            Capability::Shader => 1,
            Capability::Float16 => 9,
            Capability::Float64 => 10,
            Capability::Int8 => 39,
            Capability::Int16 => 22,
            Capability::VariablePointers => 4442,
        }
    }
}

/// SPIR-V memory model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryModel {
    /// GLSL450 + Logical addressing.
    Glsl450Logical,
    /// GLSL450 + Physical32.
    Glsl450Physical32,
    /// GLSL450 + Physical64.
    Glsl450Physical64,
}

// ═══════════════════════════════════════════════════════════════════════
// S18.2: SPIR-V Types
// ═══════════════════════════════════════════════════════════════════════

/// SPIR-V type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpirVType {
    /// Void type.
    Void { id: u32 },
    /// Boolean.
    Bool { id: u32 },
    /// Integer type.
    Int { id: u32, width: u32, signed: bool },
    /// Float type.
    Float { id: u32, width: u32 },
    /// Vector type.
    Vector {
        id: u32,
        component_id: u32,
        count: u32,
    },
    /// Array type.
    Array {
        id: u32,
        element_id: u32,
        length_id: u32,
    },
    /// Runtime array (unsized).
    RuntimeArray { id: u32, element_id: u32 },
    /// Struct type.
    Struct { id: u32, member_ids: Vec<u32> },
    /// Pointer type.
    Pointer {
        id: u32,
        storage_class: StorageClass,
        pointee_id: u32,
    },
    /// Function type.
    Function {
        id: u32,
        return_type_id: u32,
        param_type_ids: Vec<u32>,
    },
}

impl SpirVType {
    /// Returns the ID of this type.
    pub fn id(&self) -> u32 {
        match self {
            SpirVType::Void { id }
            | SpirVType::Bool { id }
            | SpirVType::Int { id, .. }
            | SpirVType::Float { id, .. }
            | SpirVType::Vector { id, .. }
            | SpirVType::Array { id, .. }
            | SpirVType::RuntimeArray { id, .. }
            | SpirVType::Struct { id, .. }
            | SpirVType::Pointer { id, .. }
            | SpirVType::Function { id, .. } => *id,
        }
    }
}

/// Maps a Fajar Lang type to a SPIR-V type descriptor.
pub fn map_fj_type(fj_type: &str) -> Option<SpirVTypeDesc> {
    match fj_type {
        "bool" => Some(SpirVTypeDesc::Bool),
        "i8" => Some(SpirVTypeDesc::Int(8, true)),
        "i16" => Some(SpirVTypeDesc::Int(16, true)),
        "i32" | "isize" => Some(SpirVTypeDesc::Int(32, true)),
        "i64" => Some(SpirVTypeDesc::Int(64, true)),
        "u8" => Some(SpirVTypeDesc::Int(8, false)),
        "u16" => Some(SpirVTypeDesc::Int(16, false)),
        "u32" | "usize" => Some(SpirVTypeDesc::Int(32, false)),
        "u64" => Some(SpirVTypeDesc::Int(64, false)),
        "f16" => Some(SpirVTypeDesc::Float(16)),
        "f32" => Some(SpirVTypeDesc::Float(32)),
        "f64" => Some(SpirVTypeDesc::Float(64)),
        _ => None,
    }
}

/// Simplified type descriptor for mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpirVTypeDesc {
    /// Boolean.
    Bool,
    /// Integer(width, signed).
    Int(u32, bool),
    /// Float(width).
    Float(u32),
}

// ═══════════════════════════════════════════════════════════════════════
// S18.3: Compute Shader Entry
// ═══════════════════════════════════════════════════════════════════════

/// SPIR-V execution model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionModel {
    /// Vertex shader.
    Vertex,
    /// Fragment shader.
    Fragment,
    /// Compute shader.
    GLCompute,
}

/// SPIR-V entry point.
#[derive(Debug, Clone)]
pub struct EntryPoint {
    /// Execution model.
    pub execution_model: ExecutionModel,
    /// Function ID.
    pub function_id: u32,
    /// Entry point name.
    pub name: String,
    /// Interface variable IDs.
    pub interface_ids: Vec<u32>,
    /// Local workgroup size.
    pub local_size: [u32; 3],
}

/// Built-in decoration for compute shaders.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltIn {
    /// gl_GlobalInvocationID.
    GlobalInvocationId,
    /// gl_LocalInvocationID.
    LocalInvocationId,
    /// gl_WorkGroupID.
    WorkGroupId,
    /// gl_NumWorkGroups.
    NumWorkGroups,
    /// gl_WorkGroupSize.
    WorkGroupSize,
}

impl BuiltIn {
    /// SPIR-V built-in value.
    pub fn value(&self) -> u32 {
        match self {
            BuiltIn::GlobalInvocationId => 28,
            BuiltIn::LocalInvocationId => 27,
            BuiltIn::WorkGroupId => 26,
            BuiltIn::NumWorkGroups => 24,
            BuiltIn::WorkGroupSize => 25,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S18.4-S18.5: Storage Buffers & Workgroup Memory
// ═══════════════════════════════════════════════════════════════════════

/// SPIR-V storage class.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageClass {
    /// Uniform constant (read-only).
    UniformConstant,
    /// Input (built-in variables).
    Input,
    /// Output.
    Output,
    /// Workgroup (shared memory).
    Workgroup,
    /// Storage buffer (SSBO).
    StorageBuffer,
    /// Function (local).
    Function,
    /// Push constant.
    PushConstant,
}

impl StorageClass {
    /// SPIR-V storage class value.
    pub fn value(&self) -> u32 {
        match self {
            StorageClass::UniformConstant => 0,
            StorageClass::Input => 1,
            StorageClass::Output => 3,
            StorageClass::Workgroup => 4,
            StorageClass::StorageBuffer => 12,
            StorageClass::Function => 7,
            StorageClass::PushConstant => 9,
        }
    }
}

/// SPIR-V variable declaration.
#[derive(Debug, Clone)]
pub struct SpirVVariable {
    /// Result ID.
    pub id: u32,
    /// Pointer type ID.
    pub type_id: u32,
    /// Storage class.
    pub storage_class: StorageClass,
    /// Binding number (for descriptors).
    pub binding: Option<u32>,
    /// Descriptor set.
    pub descriptor_set: Option<u32>,
}

/// Creates a storage buffer (SSBO) variable descriptor.
pub fn create_ssbo(id: u32, type_id: u32, binding: u32, set: u32) -> SpirVVariable {
    SpirVVariable {
        id,
        type_id,
        storage_class: StorageClass::StorageBuffer,
        binding: Some(binding),
        descriptor_set: Some(set),
    }
}

/// Creates a workgroup (shared memory) variable descriptor.
pub fn create_workgroup_var(id: u32, type_id: u32) -> SpirVVariable {
    SpirVVariable {
        id,
        type_id,
        storage_class: StorageClass::Workgroup,
        binding: None,
        descriptor_set: None,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S18.6: Barrier
// ═══════════════════════════════════════════════════════════════════════

/// Barrier scope for OpControlBarrier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BarrierScope {
    /// Workgroup scope.
    Workgroup,
    /// Device scope.
    Device,
    /// Subgroup scope.
    Subgroup,
}

impl BarrierScope {
    /// SPIR-V scope value.
    pub fn value(&self) -> u32 {
        match self {
            BarrierScope::Workgroup => 2,
            BarrierScope::Device => 1,
            BarrierScope::Subgroup => 3,
        }
    }
}

/// Memory semantics for barriers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemorySemantics {
    /// Acquire + WorkgroupMemory.
    AcquireWorkgroup,
    /// Release + WorkgroupMemory.
    ReleaseWorkgroup,
    /// AcquireRelease + WorkgroupMemory.
    AcquireReleaseWorkgroup,
}

impl MemorySemantics {
    /// SPIR-V memory semantics bitmask.
    pub fn value(&self) -> u32 {
        match self {
            MemorySemantics::AcquireWorkgroup => 0x2 | 0x100,
            MemorySemantics::ReleaseWorkgroup => 0x4 | 0x100,
            MemorySemantics::AcquireReleaseWorkgroup => 0x8 | 0x100,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S18.7: SPIR-V Validation (covered by SpirVModule::validate above)
// ═══════════════════════════════════════════════════════════════════════

// ═══════════════════════════════════════════════════════════════════════
// S18.8: Vulkan Dispatch
// ═══════════════════════════════════════════════════════════════════════

/// Vulkan dispatch configuration.
#[derive(Debug, Clone)]
pub struct VulkanDispatch {
    /// Workgroup count X.
    pub group_count_x: u32,
    /// Workgroup count Y.
    pub group_count_y: u32,
    /// Workgroup count Z.
    pub group_count_z: u32,
    /// Descriptor bindings (binding index → buffer size).
    pub buffer_bindings: Vec<BufferBinding>,
}

/// A buffer binding for Vulkan dispatch.
#[derive(Debug, Clone)]
pub struct BufferBinding {
    /// Binding index.
    pub binding: u32,
    /// Descriptor set.
    pub set: u32,
    /// Buffer size in bytes.
    pub size_bytes: usize,
    /// Whether the buffer is read-only.
    pub read_only: bool,
}

/// Computes Vulkan dispatch groups for a 1D workload.
pub fn compute_dispatch_1d(num_elements: usize, local_size_x: u32) -> VulkanDispatch {
    let groups = (num_elements as u32).div_ceil(local_size_x);
    VulkanDispatch {
        group_count_x: groups,
        group_count_y: 1,
        group_count_z: 1,
        buffer_bindings: Vec::new(),
    }
}

/// Computes Vulkan dispatch groups for a 2D workload.
pub fn compute_dispatch_2d(
    width: usize,
    height: usize,
    local_x: u32,
    local_y: u32,
) -> VulkanDispatch {
    VulkanDispatch {
        group_count_x: (width as u32).div_ceil(local_x),
        group_count_y: (height as u32).div_ceil(local_y),
        group_count_z: 1,
        buffer_bindings: Vec::new(),
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S18.9: Backend Selection
// ═══════════════════════════════════════════════════════════════════════

/// GPU backend selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuBackend {
    /// NVIDIA PTX.
    Ptx,
    /// Vulkan SPIR-V.
    SpirV,
    /// Auto-detect (NVIDIA → PTX, others → SPIR-V).
    Auto,
}

impl fmt::Display for GpuBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GpuBackend::Ptx => write!(f, "ptx"),
            GpuBackend::SpirV => write!(f, "spirv"),
            GpuBackend::Auto => write!(f, "auto"),
        }
    }
}

/// Parses a backend string from CLI flag.
pub fn parse_backend(s: &str) -> Option<GpuBackend> {
    match s {
        "ptx" => Some(GpuBackend::Ptx),
        "spirv" => Some(GpuBackend::SpirV),
        "auto" => Some(GpuBackend::Auto),
        _ => None,
    }
}

/// Resolves auto backend based on GPU vendor.
pub fn resolve_backend(backend: GpuBackend, gpu_vendor: &str) -> GpuBackend {
    match backend {
        GpuBackend::Auto => {
            if gpu_vendor.to_lowercase().contains("nvidia") {
                GpuBackend::Ptx
            } else {
                GpuBackend::SpirV
            }
        }
        other => other,
    }
}

/// SPIR-V function.
#[derive(Debug, Clone)]
pub struct SpirVFunction {
    /// Function result ID.
    pub id: u32,
    /// Return type ID.
    pub return_type_id: u32,
    /// Function type ID.
    pub function_type_id: u32,
    /// Parameter IDs.
    pub param_ids: Vec<u32>,
    /// Basic blocks.
    pub blocks: Vec<SpirVBlock>,
}

/// A basic block in a SPIR-V function.
#[derive(Debug, Clone)]
pub struct SpirVBlock {
    /// Label ID.
    pub label_id: u32,
    /// Instructions (as opcode words — simplified).
    pub instruction_count: usize,
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // S18.1 — SPIR-V Module
    #[test]
    fn s18_1_new_compute_module() {
        let m = SpirVModule::new_compute();
        assert_eq!(m.version, SPIRV_VERSION_1_5);
        assert_eq!(m.capabilities, vec![Capability::Shader]);
    }

    #[test]
    fn s18_1_alloc_id() {
        let mut m = SpirVModule::new_compute();
        let id1 = m.alloc_id();
        let id2 = m.alloc_id();
        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
        assert_eq!(m.bound, 3);
    }

    #[test]
    fn s18_1_emit_header() {
        let m = SpirVModule::new_compute();
        let words = m.emit_words();
        assert_eq!(words[0], SPIRV_MAGIC);
        assert_eq!(words[1], SPIRV_VERSION_1_5);
        assert_eq!(words.len(), 5);
    }

    // S18.2 — SPIR-V Types
    #[test]
    fn s18_2_type_mapping() {
        assert_eq!(map_fj_type("i32"), Some(SpirVTypeDesc::Int(32, true)));
        assert_eq!(map_fj_type("f32"), Some(SpirVTypeDesc::Float(32)));
        assert_eq!(map_fj_type("bool"), Some(SpirVTypeDesc::Bool));
        assert_eq!(map_fj_type("u64"), Some(SpirVTypeDesc::Int(64, false)));
        assert_eq!(map_fj_type("f16"), Some(SpirVTypeDesc::Float(16)));
        assert_eq!(map_fj_type("string"), None);
    }

    #[test]
    fn s18_2_type_id() {
        let t = SpirVType::Int {
            id: 5,
            width: 32,
            signed: true,
        };
        assert_eq!(t.id(), 5);

        let t2 = SpirVType::Float { id: 6, width: 32 };
        assert_eq!(t2.id(), 6);
    }

    // S18.3 — Compute Shader Entry
    #[test]
    fn s18_3_entry_point() {
        let ep = EntryPoint {
            execution_model: ExecutionModel::GLCompute,
            function_id: 4,
            name: "main".into(),
            interface_ids: vec![1, 2, 3],
            local_size: [256, 1, 1],
        };
        assert_eq!(ep.execution_model, ExecutionModel::GLCompute);
        assert_eq!(ep.local_size[0], 256);
    }

    #[test]
    fn s18_3_builtin_values() {
        assert_eq!(BuiltIn::GlobalInvocationId.value(), 28);
        assert_eq!(BuiltIn::LocalInvocationId.value(), 27);
        assert_eq!(BuiltIn::WorkGroupId.value(), 26);
    }

    // S18.4 — Storage Buffers
    #[test]
    fn s18_4_create_ssbo() {
        let ssbo = create_ssbo(10, 5, 0, 0);
        assert_eq!(ssbo.storage_class, StorageClass::StorageBuffer);
        assert_eq!(ssbo.binding, Some(0));
        assert_eq!(ssbo.descriptor_set, Some(0));
    }

    #[test]
    fn s18_4_storage_class_values() {
        assert_eq!(StorageClass::StorageBuffer.value(), 12);
        assert_eq!(StorageClass::Workgroup.value(), 4);
        assert_eq!(StorageClass::Input.value(), 1);
    }

    // S18.5 — Workgroup Memory
    #[test]
    fn s18_5_workgroup_var() {
        let wg = create_workgroup_var(20, 15);
        assert_eq!(wg.storage_class, StorageClass::Workgroup);
        assert_eq!(wg.binding, None);
    }

    // S18.6 — Barrier
    #[test]
    fn s18_6_barrier_scope() {
        assert_eq!(BarrierScope::Workgroup.value(), 2);
        assert_eq!(BarrierScope::Device.value(), 1);
        assert_eq!(BarrierScope::Subgroup.value(), 3);
    }

    #[test]
    fn s18_6_memory_semantics() {
        let sem = MemorySemantics::AcquireReleaseWorkgroup;
        // AcquireRelease (0x8) | WorkgroupMemory (0x100) = 0x108 = 264
        assert_eq!(sem.value(), 0x108);
    }

    // S18.7 — SPIR-V Validation
    #[test]
    fn s18_7_validate_empty_entry() {
        let m = SpirVModule::new_compute();
        let errors = m.validate();
        assert!(!errors.is_empty());
        assert!(errors.iter().any(|e| e.message.contains("entry point")));
    }

    #[test]
    fn s18_7_validate_valid_module() {
        let mut m = SpirVModule::new_compute();
        m.entry_points.push(EntryPoint {
            execution_model: ExecutionModel::GLCompute,
            function_id: 4,
            name: "main".into(),
            interface_ids: vec![],
            local_size: [64, 1, 1],
        });
        let errors = m.validate();
        assert!(errors.is_empty());
    }

    #[test]
    fn s18_7_validate_wrong_execution_model() {
        let mut m = SpirVModule::new_compute();
        m.entry_points.push(EntryPoint {
            execution_model: ExecutionModel::Fragment,
            function_id: 4,
            name: "main".into(),
            interface_ids: vec![],
            local_size: [1, 1, 1],
        });
        let errors = m.validate();
        assert!(errors.iter().any(|e| e.message.contains("GLCompute")));
    }

    // S18.8 — Vulkan Dispatch
    #[test]
    fn s18_8_dispatch_1d() {
        let d = compute_dispatch_1d(1024, 256);
        assert_eq!(d.group_count_x, 4);
        assert_eq!(d.group_count_y, 1);
        assert_eq!(d.group_count_z, 1);
    }

    #[test]
    fn s18_8_dispatch_2d() {
        let d = compute_dispatch_2d(512, 256, 16, 16);
        assert_eq!(d.group_count_x, 32);
        assert_eq!(d.group_count_y, 16);
    }

    // S18.9 — Backend Selection
    #[test]
    fn s18_9_parse_backend() {
        assert_eq!(parse_backend("ptx"), Some(GpuBackend::Ptx));
        assert_eq!(parse_backend("spirv"), Some(GpuBackend::SpirV));
        assert_eq!(parse_backend("auto"), Some(GpuBackend::Auto));
        assert_eq!(parse_backend("unknown"), None);
    }

    #[test]
    fn s18_9_resolve_auto_nvidia() {
        let b = resolve_backend(GpuBackend::Auto, "NVIDIA Corporation");
        assert_eq!(b, GpuBackend::Ptx);
    }

    #[test]
    fn s18_9_resolve_auto_amd() {
        let b = resolve_backend(GpuBackend::Auto, "AMD");
        assert_eq!(b, GpuBackend::SpirV);
    }

    #[test]
    fn s18_9_resolve_explicit() {
        let b = resolve_backend(GpuBackend::Ptx, "AMD");
        assert_eq!(b, GpuBackend::Ptx);
    }

    #[test]
    fn s18_9_backend_display() {
        assert_eq!(GpuBackend::Ptx.to_string(), "ptx");
        assert_eq!(GpuBackend::SpirV.to_string(), "spirv");
        assert_eq!(GpuBackend::Auto.to_string(), "auto");
    }

    // V16 G2: SPIR-V binary emission
    #[test]
    fn v16_g2_spirv_emit_minimal_compute() {
        let mut module = SpirVModule::new_compute();
        let bytes = module.emit_minimal_compute("main");
        // SPIR-V magic number (little-endian)
        assert_eq!(bytes[0], 0x03);
        assert_eq!(bytes[1], 0x02);
        assert_eq!(bytes[2], 0x23);
        assert_eq!(bytes[3], 0x07);
        // Must be non-empty
        assert!(
            bytes.len() > 20,
            "SPIR-V binary too small: {} bytes",
            bytes.len()
        );
    }

    #[test]
    fn v16_g2_spirv_emit_to_file() {
        let mut module = SpirVModule::new_compute();
        let path = "/tmp/fj_test_compute.spv";
        let result = module.emit_to_file(path, "main");
        assert!(result.is_ok(), "emit_to_file failed: {:?}", result.err());
        // Verify file exists and has correct magic
        let bytes = std::fs::read(path).unwrap();
        assert!(bytes.len() > 20);
        assert_eq!(bytes[0..4], [0x03, 0x02, 0x23, 0x07]);
        let _ = std::fs::remove_file(path);
    }
}
