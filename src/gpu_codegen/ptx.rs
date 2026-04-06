//! PTX Backend — map Fajar Lang types to PTX registers, generate kernel
//! entry points, thread indexing, arithmetic, memory ops, control flow,
//! warp primitives, grid launch configuration.

use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// S17.1: PTX IR Types
// ═══════════════════════════════════════════════════════════════════════

/// PTX register type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PtxType {
    /// Predicate register (1 bit).
    Pred,
    /// 8-bit unsigned int.
    U8,
    /// 16-bit unsigned int.
    U16,
    /// 32-bit unsigned int.
    U32,
    /// 64-bit unsigned int.
    U64,
    /// 32-bit signed int.
    S32,
    /// 64-bit signed int.
    S64,
    /// 32-bit float.
    F32,
    /// 64-bit float.
    F64,
    /// 16-bit float (half precision).
    F16,
    /// 16-bit brain float (bfloat16, Ada Lovelace+).
    Bf16,
    /// 8-bit float (FP8 E4M3, Ada Lovelace+).
    Fp8E4m3,
    /// 8-bit float (FP8 E5M2, Ada Lovelace+).
    Fp8E5m2,
}

impl fmt::Display for PtxType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PtxType::Pred => write!(f, ".pred"),
            PtxType::U8 => write!(f, ".u8"),
            PtxType::U16 => write!(f, ".u16"),
            PtxType::U32 => write!(f, ".u32"),
            PtxType::U64 => write!(f, ".u64"),
            PtxType::S32 => write!(f, ".s32"),
            PtxType::S64 => write!(f, ".s64"),
            PtxType::F32 => write!(f, ".f32"),
            PtxType::F64 => write!(f, ".f64"),
            PtxType::F16 => write!(f, ".f16"),
            PtxType::Bf16 => write!(f, ".bf16"),
            PtxType::Fp8E4m3 => write!(f, ".e4m3"),
            PtxType::Fp8E5m2 => write!(f, ".e5m2"),
        }
    }
}

/// Maps a Fajar Lang type name to a PTX register type.
pub fn map_type(fj_type: &str) -> Option<PtxType> {
    match fj_type {
        "bool" => Some(PtxType::Pred),
        "u8" => Some(PtxType::U8),
        "u16" => Some(PtxType::U16),
        "u32" | "usize" => Some(PtxType::U32),
        "u64" => Some(PtxType::U64),
        "i32" | "isize" => Some(PtxType::S32),
        "i64" => Some(PtxType::S64),
        "f32" => Some(PtxType::F32),
        "f64" => Some(PtxType::F64),
        "f16" => Some(PtxType::F16),
        _ => None,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S17.2: Kernel Entry
// ═══════════════════════════════════════════════════════════════════════

/// A PTX kernel parameter.
#[derive(Debug, Clone)]
pub struct KernelParam {
    /// Parameter name.
    pub name: String,
    /// PTX type of the parameter.
    pub ptx_type: PtxType,
    /// Whether this is a pointer parameter.
    pub is_pointer: bool,
}

impl fmt::Display for KernelParam {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_pointer {
            write!(f, ".param .u64 {}", self.name)
        } else {
            write!(f, ".param {} {}", self.ptx_type, self.name)
        }
    }
}

/// A PTX kernel entry function.
#[derive(Debug, Clone)]
pub struct KernelEntry {
    /// Kernel name.
    pub name: String,
    /// Parameters.
    pub params: Vec<KernelParam>,
    /// Body instructions.
    pub body: Vec<PtxInstruction>,
}

impl KernelEntry {
    /// Generates PTX assembly text for this kernel.
    pub fn emit(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!(".visible .entry {}(\n", self.name));
        for (i, p) in self.params.iter().enumerate() {
            if i + 1 < self.params.len() {
                out.push_str(&format!("    {},\n", p));
            } else {
                out.push_str(&format!("    {}\n", p));
            }
        }
        out.push_str(")\n{\n");
        for inst in &self.body {
            out.push_str(&format!("    {};\n", inst));
        }
        out.push_str("}\n");
        out
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S17.3: Thread Indexing
// ═══════════════════════════════════════════════════════════════════════

/// Special PTX register for thread/block/grid indexing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreadIndex {
    /// Thread ID within block.
    ThreadIdX,
    /// Thread ID Y.
    ThreadIdY,
    /// Thread ID Z.
    ThreadIdZ,
    /// Block ID within grid.
    BlockIdX,
    /// Block ID Y.
    BlockIdY,
    /// Block dimension (threads per block).
    BlockDimX,
    /// Block dimension Y.
    BlockDimY,
    /// Grid dimension (blocks per grid).
    GridDimX,
}

impl fmt::Display for ThreadIndex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ThreadIndex::ThreadIdX => write!(f, "%tid.x"),
            ThreadIndex::ThreadIdY => write!(f, "%tid.y"),
            ThreadIndex::ThreadIdZ => write!(f, "%tid.z"),
            ThreadIndex::BlockIdX => write!(f, "%ctaid.x"),
            ThreadIndex::BlockIdY => write!(f, "%ctaid.y"),
            ThreadIndex::BlockDimX => write!(f, "%ntid.x"),
            ThreadIndex::BlockDimY => write!(f, "%ntid.y"),
            ThreadIndex::GridDimX => write!(f, "%nctaid.x"),
        }
    }
}

/// Emits a PTX instruction to read a thread index into a register.
pub fn emit_thread_index(dst_reg: &str, index: ThreadIndex) -> PtxInstruction {
    PtxInstruction::MovSpecial {
        dst: dst_reg.to_string(),
        src: index,
    }
}

/// Computes a global thread ID: blockIdx.x * blockDim.x + threadIdx.x.
pub fn emit_global_thread_id(dst: &str, tid: &str, bid: &str, bdim: &str) -> Vec<PtxInstruction> {
    vec![PtxInstruction::Mad {
        op_type: PtxType::U32,
        dst: dst.to_string(),
        a: bid.to_string(),
        b: bdim.to_string(),
        c: tid.to_string(),
    }]
}

// ═══════════════════════════════════════════════════════════════════════
// S17.4: Arithmetic Ops
// ═══════════════════════════════════════════════════════════════════════

/// PTX arithmetic operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArithOp {
    /// Addition.
    Add,
    /// Subtraction.
    Sub,
    /// Multiplication.
    Mul,
    /// Division.
    Div,
    /// Remainder.
    Rem,
    /// Fused multiply-add.
    Fma,
}

impl fmt::Display for ArithOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ArithOp::Add => write!(f, "add"),
            ArithOp::Sub => write!(f, "sub"),
            ArithOp::Mul => write!(f, "mul"),
            ArithOp::Div => write!(f, "div"),
            ArithOp::Rem => write!(f, "rem"),
            ArithOp::Fma => write!(f, "fma"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S17.5: Memory Ops
// ═══════════════════════════════════════════════════════════════════════

/// PTX memory space.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemorySpace {
    /// Global device memory.
    Global,
    /// Shared memory (per-block).
    Shared,
    /// Local memory (per-thread, register spill).
    Local,
    /// Constant memory (read-only).
    Constant,
}

impl fmt::Display for MemorySpace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MemorySpace::Global => write!(f, ".global"),
            MemorySpace::Shared => write!(f, ".shared"),
            MemorySpace::Local => write!(f, ".local"),
            MemorySpace::Constant => write!(f, ".const"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S17.6-S17.7: Control Flow & Special Functions
// ═══════════════════════════════════════════════════════════════════════

/// A PTX instruction.
#[derive(Debug, Clone)]
pub enum PtxInstruction {
    /// Move from special register.
    MovSpecial {
        /// Destination register.
        dst: String,
        /// Source thread index.
        src: ThreadIndex,
    },
    /// Arithmetic: dst = a op b.
    Arith {
        /// Operation.
        op: ArithOp,
        /// Type.
        op_type: PtxType,
        /// Destination register.
        dst: String,
        /// Source operand A.
        a: String,
        /// Source operand B.
        b: String,
    },
    /// Fused multiply-add: dst = a * b + c.
    Mad {
        /// Type.
        op_type: PtxType,
        /// Destination.
        dst: String,
        /// Multiply operand A.
        a: String,
        /// Multiply operand B.
        b: String,
        /// Add operand C.
        c: String,
    },
    /// Load from memory.
    Load {
        /// Memory space.
        space: MemorySpace,
        /// Data type.
        data_type: PtxType,
        /// Destination register.
        dst: String,
        /// Source address.
        addr: String,
    },
    /// Store to memory.
    Store {
        /// Memory space.
        space: MemorySpace,
        /// Data type.
        data_type: PtxType,
        /// Source register.
        src: String,
        /// Destination address.
        addr: String,
    },
    /// Set predicate: pred = (a cmp b).
    SetPred {
        /// Comparison.
        cmp: String,
        /// Data type.
        op_type: PtxType,
        /// Predicate destination.
        pred: String,
        /// Operand A.
        a: String,
        /// Operand B.
        b: String,
    },
    /// Conditional branch.
    BranchPred {
        /// Predicate register.
        pred: String,
        /// Whether to branch if NOT predicate.
        negate: bool,
        /// Target label.
        target: String,
    },
    /// Unconditional branch.
    Branch {
        /// Target label.
        target: String,
    },
    /// Label.
    Label {
        /// Label name.
        name: String,
    },
    /// Bar.sync (barrier synchronization).
    BarSync {
        /// Barrier ID (usually 0).
        barrier_id: u32,
    },
    /// Atomic operation.
    AtomicAdd {
        /// Memory space.
        space: MemorySpace,
        /// Data type.
        op_type: PtxType,
        /// Destination register (old value).
        dst: String,
        /// Memory address.
        addr: String,
        /// Value to add.
        val: String,
    },
    /// Warp shuffle (sync).
    ShflSync {
        /// Mode (up, down, bfly, idx).
        mode: String,
        /// Data type.
        op_type: PtxType,
        /// Destination.
        dst: String,
        /// Source.
        src: String,
        /// Offset or lane.
        offset: u32,
        /// Mask (usually 0xFFFFFFFF).
        mask: u32,
    },
    /// Return.
    Ret,
}

impl fmt::Display for PtxInstruction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PtxInstruction::MovSpecial { dst, src } => {
                write!(f, "mov.u32 {dst}, {src}")
            }
            PtxInstruction::Arith {
                op,
                op_type,
                dst,
                a,
                b,
            } => {
                write!(f, "{op}{op_type} {dst}, {a}, {b}")
            }
            PtxInstruction::Mad {
                op_type,
                dst,
                a,
                b,
                c,
            } => {
                write!(f, "mad.lo{op_type} {dst}, {a}, {b}, {c}")
            }
            PtxInstruction::Load {
                space,
                data_type,
                dst,
                addr,
            } => {
                write!(f, "ld{space}{data_type} {dst}, [{addr}]")
            }
            PtxInstruction::Store {
                space,
                data_type,
                src,
                addr,
            } => {
                write!(f, "st{space}{data_type} [{addr}], {src}")
            }
            PtxInstruction::SetPred {
                cmp,
                op_type,
                pred,
                a,
                b,
            } => {
                write!(f, "setp.{cmp}{op_type} {pred}, {a}, {b}")
            }
            PtxInstruction::BranchPred {
                pred,
                negate,
                target,
            } => {
                if *negate {
                    write!(f, "@!{pred} bra {target}")
                } else {
                    write!(f, "@{pred} bra {target}")
                }
            }
            PtxInstruction::Branch { target } => {
                write!(f, "bra {target}")
            }
            PtxInstruction::Label { name } => {
                write!(f, "{name}:")
            }
            PtxInstruction::BarSync { barrier_id } => {
                write!(f, "bar.sync {barrier_id}")
            }
            PtxInstruction::AtomicAdd {
                space,
                op_type,
                dst,
                addr,
                val,
            } => {
                write!(f, "atom{space}.add{op_type} {dst}, [{addr}], {val}")
            }
            PtxInstruction::ShflSync {
                mode,
                op_type,
                dst,
                src,
                offset,
                mask,
            } => {
                write!(
                    f,
                    "shfl.sync.{mode}{op_type} {dst}, {src}, {offset}, 0x{mask:08X}"
                )
            }
            PtxInstruction::Ret => write!(f, "ret"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S17.8: PTX Assembly Output
// ═══════════════════════════════════════════════════════════════════════

/// PTX module — a complete PTX assembly file.
#[derive(Debug, Clone)]
pub struct PtxModule {
    /// Target SM version (e.g., 70 for sm_70).
    pub sm_version: u32,
    /// PTX ISA version (e.g., 70 for 7.0).
    pub ptx_version: u32,
    /// Address size in bits (32 or 64).
    pub address_size: u32,
    /// Kernel entries.
    pub kernels: Vec<KernelEntry>,
    /// Shared memory declarations.
    pub shared_decls: Vec<SharedDecl>,
}

/// Shared memory declaration.
#[derive(Debug, Clone)]
pub struct SharedDecl {
    /// Variable name.
    pub name: String,
    /// Element type.
    pub elem_type: PtxType,
    /// Number of elements.
    pub count: usize,
}

impl PtxModule {
    /// Creates a module targeting RTX 4090 (Ada Lovelace, sm_89, PTX 8.3).
    pub fn for_rtx4090() -> Self {
        Self {
            ptx_version: 83,
            sm_version: 89,
            address_size: 64,
            kernels: Vec::new(),
            shared_decls: Vec::new(),
        }
    }

    /// Creates a module targeting the given compute capability.
    pub fn for_compute(sm: u32) -> Self {
        let ptx_ver = match sm {
            89 => 83,      // Ada Lovelace
            86 | 87 => 75, // Ampere
            80 => 75,      // A100
            75 => 65,      // Turing
            70 => 60,      // Volta
            _ => 75,       // default
        };
        Self {
            ptx_version: ptx_ver,
            sm_version: sm,
            address_size: 64,
            kernels: Vec::new(),
            shared_decls: Vec::new(),
        }
    }

    /// Emits the complete PTX assembly text.
    pub fn emit(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            ".version {}.{}\n",
            self.ptx_version / 10,
            self.ptx_version % 10
        ));
        out.push_str(&format!(".target sm_{}\n", self.sm_version));
        out.push_str(&format!(".address_size {}\n\n", self.address_size));

        for decl in &self.shared_decls {
            out.push_str(&format!(
                ".shared {} {}[{}];\n",
                decl.elem_type, decl.name, decl.count
            ));
        }
        if !self.shared_decls.is_empty() {
            out.push('\n');
        }

        for kernel in &self.kernels {
            out.push_str(&kernel.emit());
            out.push('\n');
        }
        out
    }

    /// V16 G3: Create a minimal compute kernel that does nothing (ret).
    pub fn add_minimal_kernel(&mut self, name: &str) {
        self.kernels.push(KernelEntry {
            name: name.to_string(),
            params: Vec::new(),
            body: vec![PtxInstruction::Ret],
        });
    }

    /// V16 G3: Emit PTX assembly to file.
    pub fn emit_to_file(&self, path: &str) -> Result<(), String> {
        std::fs::write(path, self.emit()).map_err(|e| format!("Failed to write PTX: {e}"))
    }

    /// V16 G3.2-G3.7: Add a real element-wise add kernel with:
    /// - Parameter: .param .u64 data_ptr
    /// - Thread index calculation: tid = blockIdx.x * blockDim.x + threadIdx.x
    /// - Memory load/store: ld.global.f32 / st.global.f32
    /// - Arithmetic: add.f32
    pub fn add_elementwise_add_kernel(&mut self, name: &str) {
        use PtxInstruction::*;

        let params = vec![KernelParam {
            name: "data_ptr".to_string(),
            ptx_type: PtxType::U64,
            is_pointer: true,
        }];

        let body = vec![
            // Get thread index: gid = bid * bdim + tid
            MovSpecial {
                dst: "%tid".to_string(),
                src: ThreadIndex::ThreadIdX,
            },
            MovSpecial {
                dst: "%bid".to_string(),
                src: ThreadIndex::BlockIdX,
            },
            MovSpecial {
                dst: "%bdim".to_string(),
                src: ThreadIndex::BlockDimX,
            },
            Mad {
                op_type: PtxType::U32,
                dst: "%gid".to_string(),
                a: "%bid".to_string(),
                b: "%bdim".to_string(),
                c: "%tid".to_string(),
            },
            // Load base address from parameter
            Load {
                space: MemorySpace::Global,
                data_type: PtxType::U64,
                dst: "%addr".to_string(),
                addr: "[data_ptr]".to_string(),
            },
            // Compute byte offset: offset = gid * 4 (sizeof f32)
            Arith {
                op: ArithOp::Mul,
                op_type: PtxType::U32,
                dst: "%offset".to_string(),
                a: "%gid".to_string(),
                b: "4".to_string(),
            },
            // Add offset to base (simplified: assume 32-bit addressing)
            Arith {
                op: ArithOp::Add,
                op_type: PtxType::U64,
                dst: "%elem_addr".to_string(),
                a: "%addr".to_string(),
                b: "%offset".to_string(),
            },
            // Load value: val = data[gid]
            Load {
                space: MemorySpace::Global,
                data_type: PtxType::F32,
                dst: "%val".to_string(),
                addr: "[%elem_addr]".to_string(),
            },
            // Add 1.0: result = val + 1.0
            Arith {
                op: ArithOp::Add,
                op_type: PtxType::F32,
                dst: "%result".to_string(),
                a: "%val".to_string(),
                b: "0f3F800000".to_string(),
            },
            // Store result: data[gid] = result
            Store {
                space: MemorySpace::Global,
                data_type: PtxType::F32,
                src: "%result".to_string(),
                addr: "[%elem_addr]".to_string(),
            },
            Ret,
        ];

        self.kernels.push(KernelEntry {
            name: name.to_string(),
            params,
            body,
        });
    }

    /// V16 G3.8: Emit compute shader to .ptx file with size info.
    pub fn emit_compute_to_file(&self, path: &str) -> Result<usize, String> {
        let ptx = self.emit();
        let len = ptx.len();
        std::fs::write(path, &ptx).map_err(|e| format!("Failed to write PTX: {e}"))?;
        Ok(len)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S17.9: Grid Launch Config
// ═══════════════════════════════════════════════════════════════════════

/// Grid launch configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GridConfig {
    /// Blocks per grid (x).
    pub grid_x: u32,
    /// Blocks per grid (y).
    pub grid_y: u32,
    /// Threads per block (x).
    pub block_x: u32,
    /// Threads per block (y).
    pub block_y: u32,
}

impl GridConfig {
    /// Total number of threads.
    pub fn total_threads(&self) -> u64 {
        self.grid_x as u64 * self.grid_y as u64 * self.block_x as u64 * self.block_y as u64
    }
}

/// Compute optimal 1D grid/block from total elements.
pub fn compute_grid_1d(num_elements: usize, block_size: u32) -> GridConfig {
    let grid_x = (num_elements as u32).div_ceil(block_size);
    GridConfig {
        grid_x,
        grid_y: 1,
        block_x: block_size,
        block_y: 1,
    }
}

/// Compute optimal 2D grid/block from matrix dimensions.
pub fn compute_grid_2d(rows: usize, cols: usize, tile_size: u32) -> GridConfig {
    let grid_x = (cols as u32).div_ceil(tile_size);
    let grid_y = (rows as u32).div_ceil(tile_size);
    GridConfig {
        grid_x,
        grid_y,
        block_x: tile_size,
        block_y: tile_size,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // S17.1 — PTX IR Types
    #[test]
    fn s17_1_type_mapping() {
        assert_eq!(map_type("i32"), Some(PtxType::S32));
        assert_eq!(map_type("f64"), Some(PtxType::F64));
        assert_eq!(map_type("bool"), Some(PtxType::Pred));
        assert_eq!(map_type("u32"), Some(PtxType::U32));
        assert_eq!(map_type("u64"), Some(PtxType::U64));
        assert_eq!(map_type("f16"), Some(PtxType::F16));
        assert_eq!(map_type("string"), None);
    }

    #[test]
    fn s17_1_type_display() {
        assert_eq!(PtxType::U32.to_string(), ".u32");
        assert_eq!(PtxType::F64.to_string(), ".f64");
        assert_eq!(PtxType::Pred.to_string(), ".pred");
    }

    // S17.2 — Kernel Entry
    #[test]
    fn s17_2_kernel_entry_emit() {
        let kernel = KernelEntry {
            name: "vector_add".into(),
            params: vec![
                KernelParam {
                    name: "a".into(),
                    ptx_type: PtxType::U64,
                    is_pointer: true,
                },
                KernelParam {
                    name: "b".into(),
                    ptx_type: PtxType::U64,
                    is_pointer: true,
                },
                KernelParam {
                    name: "n".into(),
                    ptx_type: PtxType::U32,
                    is_pointer: false,
                },
            ],
            body: vec![PtxInstruction::Ret],
        };
        let ptx = kernel.emit();
        assert!(ptx.contains(".visible .entry vector_add"));
        assert!(ptx.contains(".param .u64 a"));
        assert!(ptx.contains(".param .u32 n"));
        assert!(ptx.contains("ret;"));
    }

    // S17.3 — Thread Indexing
    #[test]
    fn s17_3_thread_index_display() {
        assert_eq!(ThreadIndex::ThreadIdX.to_string(), "%tid.x");
        assert_eq!(ThreadIndex::BlockIdX.to_string(), "%ctaid.x");
        assert_eq!(ThreadIndex::BlockDimX.to_string(), "%ntid.x");
        assert_eq!(ThreadIndex::GridDimX.to_string(), "%nctaid.x");
    }

    #[test]
    fn s17_3_emit_thread_index() {
        let inst = emit_thread_index("%r1", ThreadIndex::ThreadIdX);
        assert_eq!(inst.to_string(), "mov.u32 %r1, %tid.x");
    }

    #[test]
    fn s17_3_global_thread_id() {
        let insts = emit_global_thread_id("%r3", "%r1", "%r2", "%r0");
        assert_eq!(insts.len(), 1);
        assert!(insts[0].to_string().contains("mad.lo"));
    }

    // S17.4 — Arithmetic Ops
    #[test]
    fn s17_4_arith_instruction() {
        let inst = PtxInstruction::Arith {
            op: ArithOp::Add,
            op_type: PtxType::F32,
            dst: "%f3".into(),
            a: "%f1".into(),
            b: "%f2".into(),
        };
        assert_eq!(inst.to_string(), "add.f32 %f3, %f1, %f2");
    }

    #[test]
    fn s17_4_fma_instruction() {
        let inst = PtxInstruction::Mad {
            op_type: PtxType::F32,
            dst: "%f4".into(),
            a: "%f1".into(),
            b: "%f2".into(),
            c: "%f3".into(),
        };
        assert_eq!(inst.to_string(), "mad.lo.f32 %f4, %f1, %f2, %f3");
    }

    #[test]
    fn s17_4_arith_op_display() {
        assert_eq!(ArithOp::Add.to_string(), "add");
        assert_eq!(ArithOp::Mul.to_string(), "mul");
        assert_eq!(ArithOp::Fma.to_string(), "fma");
        assert_eq!(ArithOp::Rem.to_string(), "rem");
    }

    // S17.5 — Memory Ops
    #[test]
    fn s17_5_load_global() {
        let inst = PtxInstruction::Load {
            space: MemorySpace::Global,
            data_type: PtxType::F32,
            dst: "%f1".into(),
            addr: "%rd1".into(),
        };
        assert_eq!(inst.to_string(), "ld.global.f32 %f1, [%rd1]");
    }

    #[test]
    fn s17_5_store_shared() {
        let inst = PtxInstruction::Store {
            space: MemorySpace::Shared,
            data_type: PtxType::F32,
            src: "%f1".into(),
            addr: "%r1".into(),
        };
        assert_eq!(inst.to_string(), "st.shared.f32 [%r1], %f1");
    }

    #[test]
    fn s17_5_memory_space_display() {
        assert_eq!(MemorySpace::Global.to_string(), ".global");
        assert_eq!(MemorySpace::Shared.to_string(), ".shared");
        assert_eq!(MemorySpace::Local.to_string(), ".local");
        assert_eq!(MemorySpace::Constant.to_string(), ".const");
    }

    // S17.6 — Control Flow
    #[test]
    fn s17_6_conditional_branch() {
        let inst = PtxInstruction::BranchPred {
            pred: "%p1".into(),
            negate: false,
            target: "LOOP_BODY".into(),
        };
        assert_eq!(inst.to_string(), "@%p1 bra LOOP_BODY");
    }

    #[test]
    fn s17_6_negated_branch() {
        let inst = PtxInstruction::BranchPred {
            pred: "%p1".into(),
            negate: true,
            target: "EXIT".into(),
        };
        assert_eq!(inst.to_string(), "@!%p1 bra EXIT");
    }

    #[test]
    fn s17_6_set_predicate() {
        let inst = PtxInstruction::SetPred {
            cmp: "lt".into(),
            op_type: PtxType::U32,
            pred: "%p1".into(),
            a: "%r1".into(),
            b: "%r2".into(),
        };
        assert_eq!(inst.to_string(), "setp.lt.u32 %p1, %r1, %r2");
    }

    // S17.7 — Special Functions
    #[test]
    fn s17_7_bar_sync() {
        let inst = PtxInstruction::BarSync { barrier_id: 0 };
        assert_eq!(inst.to_string(), "bar.sync 0");
    }

    #[test]
    fn s17_7_atomic_add() {
        let inst = PtxInstruction::AtomicAdd {
            space: MemorySpace::Global,
            op_type: PtxType::F32,
            dst: "%f0".into(),
            addr: "%rd1".into(),
            val: "%f1".into(),
        };
        assert_eq!(inst.to_string(), "atom.global.add.f32 %f0, [%rd1], %f1");
    }

    #[test]
    fn s17_7_shfl_sync() {
        let inst = PtxInstruction::ShflSync {
            mode: "bfly".into(),
            op_type: PtxType::F32,
            dst: "%f1".into(),
            src: "%f0".into(),
            offset: 16,
            mask: 0xFFFFFFFF,
        };
        assert!(inst.to_string().contains("shfl.sync.bfly.f32"));
        assert!(inst.to_string().contains("0xFFFFFFFF"));
    }

    // S17.8 — PTX Assembly Output
    #[test]
    fn s17_8_ptx_module_emit() {
        let module = PtxModule {
            sm_version: 70,
            ptx_version: 70,
            address_size: 64,
            kernels: vec![KernelEntry {
                name: "test_kernel".into(),
                params: vec![],
                body: vec![PtxInstruction::Ret],
            }],
            shared_decls: vec![SharedDecl {
                name: "smem".into(),
                elem_type: PtxType::F32,
                count: 256,
            }],
        };
        let ptx = module.emit();
        assert!(ptx.contains(".version 7.0"));
        assert!(ptx.contains(".target sm_70"));
        assert!(ptx.contains(".address_size 64"));
        assert!(ptx.contains(".shared .f32 smem[256]"));
        assert!(ptx.contains(".visible .entry test_kernel"));
    }

    // S17.9 — Grid Launch Config
    #[test]
    fn s17_9_grid_1d() {
        let cfg = compute_grid_1d(1024, 256);
        assert_eq!(cfg.grid_x, 4);
        assert_eq!(cfg.block_x, 256);
        assert_eq!(cfg.total_threads(), 1024);
    }

    #[test]
    fn s17_9_grid_1d_non_divisible() {
        let cfg = compute_grid_1d(1000, 256);
        assert_eq!(cfg.grid_x, 4); // ceil(1000/256) = 4
        assert!(cfg.total_threads() >= 1000);
    }

    #[test]
    fn s17_9_grid_2d() {
        let cfg = compute_grid_2d(512, 512, 16);
        assert_eq!(cfg.grid_x, 32);
        assert_eq!(cfg.grid_y, 32);
        assert_eq!(cfg.block_x, 16);
        assert_eq!(cfg.block_y, 16);
    }

    #[test]
    fn v16_g3_ptx_minimal_kernel() {
        let mut module = PtxModule {
            ptx_version: 75,
            sm_version: 80,
            address_size: 64,
            kernels: Vec::new(),
            shared_decls: Vec::new(),
        };
        module.add_minimal_kernel("compute_main");
        let ptx = module.emit();
        assert!(ptx.contains(".version 7.5"));
        assert!(ptx.contains(".target sm_80"));
        assert!(ptx.contains(".visible .entry compute_main"));
        assert!(ptx.contains("ret"));
    }

    #[test]
    fn v16_g3_ptx_emit_to_file() {
        let mut module = PtxModule {
            ptx_version: 75,
            sm_version: 80,
            address_size: 64,
            kernels: Vec::new(),
            shared_decls: Vec::new(),
        };
        module.add_minimal_kernel("main");
        let path = "/tmp/fj_test_compute.ptx";
        let result = module.emit_to_file(path);
        assert!(result.is_ok());
        let content = std::fs::read_to_string(path).unwrap();
        assert!(content.contains(".visible .entry main"));
        let _ = std::fs::remove_file(path);
    }

    // V16 G3.2: Full element-wise add kernel
    #[test]
    fn v16_g3_full_elementwise_kernel() {
        let mut module = PtxModule {
            ptx_version: 75,
            sm_version: 80,
            address_size: 64,
            kernels: Vec::new(),
            shared_decls: Vec::new(),
        };
        module.add_elementwise_add_kernel("add_one");
        let ptx = module.emit();
        assert!(ptx.contains(".visible .entry add_one"));
        assert!(ptx.contains(".param .u64 data_ptr"));
        assert!(ptx.contains("mad.lo.u32")); // thread index calc
        assert!(ptx.contains("add.f32")); // arithmetic
        assert!(ptx.contains("ld.global.f32")); // memory load
        assert!(ptx.contains("st.global.f32")); // memory store
        assert!(ptx.contains("ret"));
    }

    // V16 G3.3: Type mapping in kernel
    #[test]
    fn v16_g3_kernel_type_mapping() {
        let mut module = PtxModule {
            ptx_version: 75,
            sm_version: 80,
            address_size: 64,
            kernels: Vec::new(),
            shared_decls: Vec::new(),
        };
        module.add_elementwise_add_kernel("compute");
        let ptx = module.emit();
        // Verify u32 and f32 types are used
        assert!(ptx.contains(".u32"));
        assert!(ptx.contains(".f32"));
        assert!(ptx.contains(".u64"));
    }

    // V23: sm_89 Ada Lovelace (RTX 4090) — PTX 8.3, BF16/FP8 types
    #[test]
    fn v23_ptx_sm89_ada_lovelace() {
        let mut module = PtxModule {
            ptx_version: 83, // PTX ISA 8.3 for Ada Lovelace
            sm_version: 89,
            address_size: 64,
            kernels: Vec::new(),
            shared_decls: Vec::new(),
        };
        module.add_elementwise_add_kernel("ada_compute");
        let ptx = module.emit();
        assert!(ptx.contains(".version 8.3"));
        assert!(ptx.contains(".target sm_89"));
        assert!(ptx.contains(".visible .entry ada_compute"));
        assert!(ptx.contains("add.f32"));
    }

    #[test]
    fn v23_ptx_bf16_fp8_types() {
        assert_eq!(format!("{}", PtxType::Bf16), ".bf16");
        assert_eq!(format!("{}", PtxType::Fp8E4m3), ".e4m3");
        assert_eq!(format!("{}", PtxType::Fp8E5m2), ".e5m2");
    }

    // V16 G3.8: emit_compute_to_file
    #[test]
    fn v16_g3_emit_compute_to_file() {
        let mut module = PtxModule {
            ptx_version: 75,
            sm_version: 80,
            address_size: 64,
            kernels: Vec::new(),
            shared_decls: Vec::new(),
        };
        module.add_elementwise_add_kernel("main");
        let path = "/tmp/fj_test_full_compute.ptx";
        let result = module.emit_compute_to_file(path);
        assert!(result.is_ok());
        let size = result.unwrap();
        assert!(size > 100, "PTX too small: {size}");
        let _ = std::fs::remove_file(path);
    }
}
