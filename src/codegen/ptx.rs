//! Blackwell PTX code generation for Fajar Lang.
//!
//! Emits NVIDIA PTX (Parallel Thread Execution) assembly text for
//! Blackwell GPU targets (sm_100, sm_101), including:
//!
//! - 5th-gen Tensor Core MMA (`tcgen05.mma`)
//! - Tensor Memory (TMEM) 256KB per-warp access
//! - FP4/FP8/BF16 Tensor Core dispatch
//! - TMA (Tensor Memory Accelerator) bulk copy
//! - Cluster launch for multi-SM coordination
//!
//! # Pipeline
//!
//! ```text
//! Fajar Lang AST → PTX text → ptxas → CUBIN → cuModuleLoad
//! ```

use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// PTX Target
// ═══════════════════════════════════════════════════════════════════════

/// PTX target architecture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PtxTarget {
    /// Blackwell (compute capability 10.0).
    Sm100,
    /// Blackwell (compute capability 10.1).
    Sm101,
}

impl PtxTarget {
    /// Returns the sm_XX string for ptxas.
    pub fn sm_string(&self) -> &'static str {
        match self {
            PtxTarget::Sm100 => "sm_100",
            PtxTarget::Sm101 => "sm_101",
        }
    }

    /// Returns the PTX ISA version for this target.
    pub fn ptx_version(&self) -> &'static str {
        "8.6" // Blackwell PTX ISA
    }

    /// Whether this target supports 5th-gen Tensor Cores.
    pub fn has_tcgen05(&self) -> bool {
        true // All Blackwell targets
    }

    /// Whether TMA is supported.
    pub fn has_tma(&self) -> bool {
        true
    }

    /// Whether cluster launch is supported.
    pub fn has_cluster_launch(&self) -> bool {
        true
    }
}

impl fmt::Display for PtxTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.sm_string())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// PTX Data Types
// ═══════════════════════════════════════════════════════════════════════

/// PTX register types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PtxType {
    /// 1-bit predicate.
    Pred,
    /// 16-bit unsigned.
    U16,
    /// 32-bit unsigned.
    U32,
    /// 64-bit unsigned.
    U64,
    /// 16-bit signed.
    S16,
    /// 32-bit signed.
    S32,
    /// 64-bit signed.
    S64,
    /// 16-bit float (IEEE FP16).
    F16,
    /// 16-bit float (BFloat16).
    Bf16,
    /// 32-bit float.
    F32,
    /// 64-bit float.
    F64,
    /// 8-bit FP (E4M3).
    Fp8E4m3,
    /// 8-bit FP (E5M2).
    Fp8E5m2,
    /// 4-bit FP (E2M1).
    Fp4E2m1,
    /// 32-bit unsigned (for bit manipulation).
    B32,
    /// 64-bit unsigned (for bit manipulation).
    B64,
}

impl fmt::Display for PtxType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            PtxType::Pred => ".pred",
            PtxType::U16 => ".u16",
            PtxType::U32 => ".u32",
            PtxType::U64 => ".u64",
            PtxType::S16 => ".s16",
            PtxType::S32 => ".s32",
            PtxType::S64 => ".s64",
            PtxType::F16 => ".f16",
            PtxType::Bf16 => ".bf16",
            PtxType::F32 => ".f32",
            PtxType::F64 => ".f64",
            PtxType::Fp8E4m3 => ".e4m3",
            PtxType::Fp8E5m2 => ".e5m2",
            PtxType::Fp4E2m1 => ".e2m1",
            PtxType::B32 => ".b32",
            PtxType::B64 => ".b64",
        };
        write!(f, "{}", name)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tensor Core MMA (tcgen05)
// ═══════════════════════════════════════════════════════════════════════

/// Tensor Core data type for MMA operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TcDataType {
    /// FP32 (accumulator type).
    Fp32,
    /// FP16 (IEEE half).
    Fp16,
    /// BFloat16.
    Bf16,
    /// FP8 E4M3 (forward pass).
    Fp8E4m3,
    /// FP8 E5M2 (backward pass).
    Fp8E5m2,
    /// FP4 E2M1 (ultra-low precision).
    Fp4E2m1,
    /// INT8.
    Int8,
}

impl fmt::Display for TcDataType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            TcDataType::Fp32 => "fp32",
            TcDataType::Fp16 => "fp16",
            TcDataType::Bf16 => "bf16",
            TcDataType::Fp8E4m3 => "e4m3",
            TcDataType::Fp8E5m2 => "e5m2",
            TcDataType::Fp4E2m1 => "fp4",
            TcDataType::Int8 => "s8",
        };
        write!(f, "{}", s)
    }
}

/// Tensor Core MMA shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MmaShape {
    /// M dimension.
    pub m: u32,
    /// N dimension.
    pub n: u32,
    /// K dimension.
    pub k: u32,
}

impl MmaShape {
    /// Creates an MMA shape.
    pub fn new(m: u32, n: u32, k: u32) -> Self {
        Self { m, n, k }
    }

    /// Default shape for FP16/BF16: 16×8×16.
    pub fn default_fp16() -> Self {
        Self { m: 16, n: 8, k: 16 }
    }

    /// Default shape for FP8: 16×8×32.
    pub fn default_fp8() -> Self {
        Self { m: 16, n: 8, k: 32 }
    }

    /// Default shape for FP4: 16×8×64.
    pub fn default_fp4() -> Self {
        Self { m: 16, n: 8, k: 64 }
    }
}

impl fmt::Display for MmaShape {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "m{}n{}k{}", self.m, self.n, self.k)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// PTX Instructions
// ═══════════════════════════════════════════════════════════════════════

/// PTX instruction.
#[derive(Debug, Clone)]
pub enum PtxInstruction {
    /// Module header directive.
    Header {
        target: PtxTarget,
        address_size: u32,
    },
    /// Register declaration.
    RegDecl {
        ty: PtxType,
        name: String,
        count: u32,
    },
    /// Kernel entry point.
    KernelEntry { name: String },
    /// Kernel end.
    KernelEnd,
    /// 5th gen Tensor Core MMA.
    Tcgen05Mma {
        shape: MmaShape,
        a_type: TcDataType,
        b_type: TcDataType,
        c_type: TcDataType,
        d_type: TcDataType,
    },
    /// TMEM load (Tensor Memory, 256KB per warp).
    TmemLoad {
        dst: String,
        addr: String,
        ty: PtxType,
    },
    /// TMEM store.
    TmemStore {
        addr: String,
        src: String,
        ty: PtxType,
    },
    /// TMA bulk async copy (Tensor Memory Accelerator).
    TmaBulkCopy {
        dst_shared: String,
        src_global: String,
        size_bytes: u32,
    },
    /// Cluster barrier (cooperative groups).
    ClusterBarrier,
    /// Generic PTX instruction (for simple ops).
    Generic { text: String },
    /// Return from kernel.
    Ret,
}

impl fmt::Display for PtxInstruction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PtxInstruction::Header {
                target,
                address_size,
            } => {
                writeln!(f, ".version {}", target.ptx_version())?;
                writeln!(f, ".target {}", target.sm_string())?;
                write!(f, ".address_size {}", address_size)
            }
            PtxInstruction::RegDecl { ty, name, count } => {
                write!(f, "  .reg {} {}<{}>", ty, name, count)
            }
            PtxInstruction::KernelEntry { name } => {
                write!(f, ".visible .entry {}(", name)
            }
            PtxInstruction::KernelEnd => write!(f, "}}"),
            PtxInstruction::Tcgen05Mma {
                shape,
                a_type,
                b_type,
                c_type,
                d_type,
            } => write!(
                f,
                "  tcgen05.mma.{}.{}.{}.{}.{}",
                shape, d_type, a_type, b_type, c_type
            ),
            PtxInstruction::TmemLoad { dst, addr, ty } => {
                write!(f, "  tcgen05.ld.sync.aligned{} {}, [{}]", ty, dst, addr)
            }
            PtxInstruction::TmemStore { addr, src, ty } => {
                write!(f, "  tcgen05.st.sync.aligned{} [{}], {}", ty, addr, src)
            }
            PtxInstruction::TmaBulkCopy {
                dst_shared,
                src_global,
                size_bytes,
            } => write!(
                f,
                "  cp.async.bulk.shared.global [{}], [{}], {}",
                dst_shared, src_global, size_bytes
            ),
            PtxInstruction::ClusterBarrier => {
                write!(f, "  barrier.cluster.arrive.aligned")
            }
            PtxInstruction::Generic { text } => write!(f, "  {}", text),
            PtxInstruction::Ret => write!(f, "  ret"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// PTX Emitter
// ═══════════════════════════════════════════════════════════════════════

/// PTX code emitter for Blackwell targets.
#[derive(Debug)]
pub struct PtxEmitter {
    /// Target architecture.
    pub target: PtxTarget,
    /// Emitted instructions.
    instructions: Vec<PtxInstruction>,
    /// Next register index for naming.
    next_reg: u32,
}

impl PtxEmitter {
    /// Creates a new PTX emitter for the given target.
    pub fn new(target: PtxTarget) -> Self {
        Self {
            target,
            instructions: Vec::new(),
            next_reg: 0,
        }
    }

    /// Returns emitted instructions.
    pub fn instructions(&self) -> &[PtxInstruction] {
        &self.instructions
    }

    /// Emits the PTX module header.
    pub fn emit_header(&mut self) {
        self.instructions.push(PtxInstruction::Header {
            target: self.target,
            address_size: 64,
        });
    }

    /// Emits a register declaration.
    pub fn emit_reg_decl(&mut self, ty: PtxType, name: &str, count: u32) {
        self.instructions.push(PtxInstruction::RegDecl {
            ty,
            name: name.to_string(),
            count,
        });
    }

    /// Emits kernel entry.
    pub fn emit_kernel_entry(&mut self, name: &str) {
        self.instructions.push(PtxInstruction::KernelEntry {
            name: name.to_string(),
        });
    }

    /// Emits kernel end.
    pub fn emit_kernel_end(&mut self) {
        self.instructions.push(PtxInstruction::Ret);
        self.instructions.push(PtxInstruction::KernelEnd);
    }

    /// Emits 5th-gen Tensor Core MMA (tcgen05.mma).
    pub fn emit_tcgen05_mma(
        &mut self,
        shape: MmaShape,
        a_type: TcDataType,
        b_type: TcDataType,
        c_type: TcDataType,
        d_type: TcDataType,
    ) -> Result<(), String> {
        if !self.target.has_tcgen05() {
            return Err(format!("tcgen05.mma not supported on {}", self.target));
        }
        self.instructions.push(PtxInstruction::Tcgen05Mma {
            shape,
            a_type,
            b_type,
            c_type,
            d_type,
        });
        Ok(())
    }

    /// Emits FP4 Tensor Core dispatch.
    pub fn emit_fp4_mma(&mut self) -> Result<(), String> {
        self.emit_tcgen05_mma(
            MmaShape::default_fp4(),
            TcDataType::Fp4E2m1,
            TcDataType::Fp4E2m1,
            TcDataType::Fp32,
            TcDataType::Fp32,
        )
    }

    /// Emits FP8 E4M3 Tensor Core dispatch (forward pass).
    pub fn emit_fp8_e4m3_mma(&mut self) -> Result<(), String> {
        self.emit_tcgen05_mma(
            MmaShape::default_fp8(),
            TcDataType::Fp8E4m3,
            TcDataType::Fp8E4m3,
            TcDataType::Fp32,
            TcDataType::Fp32,
        )
    }

    /// Emits FP8 E5M2 Tensor Core dispatch (backward pass).
    pub fn emit_fp8_e5m2_mma(&mut self) -> Result<(), String> {
        self.emit_tcgen05_mma(
            MmaShape::default_fp8(),
            TcDataType::Fp8E5m2,
            TcDataType::Fp8E5m2,
            TcDataType::Fp32,
            TcDataType::Fp32,
        )
    }

    /// Emits BF16 Tensor Core dispatch.
    pub fn emit_bf16_mma(&mut self) -> Result<(), String> {
        self.emit_tcgen05_mma(
            MmaShape::default_fp16(),
            TcDataType::Bf16,
            TcDataType::Bf16,
            TcDataType::Fp32,
            TcDataType::Fp32,
        )
    }

    /// Emits TMEM load.
    pub fn emit_tmem_load(&mut self, dst: &str, addr: &str, ty: PtxType) {
        self.instructions.push(PtxInstruction::TmemLoad {
            dst: dst.to_string(),
            addr: addr.to_string(),
            ty,
        });
    }

    /// Emits TMEM store.
    pub fn emit_tmem_store(&mut self, addr: &str, src: &str, ty: PtxType) {
        self.instructions.push(PtxInstruction::TmemStore {
            addr: addr.to_string(),
            src: src.to_string(),
            ty,
        });
    }

    /// Emits TMA bulk async copy.
    pub fn emit_tma_bulk_copy(
        &mut self,
        dst_shared: &str,
        src_global: &str,
        size_bytes: u32,
    ) -> Result<(), String> {
        if !self.target.has_tma() {
            return Err("TMA not supported".to_string());
        }
        self.instructions.push(PtxInstruction::TmaBulkCopy {
            dst_shared: dst_shared.to_string(),
            src_global: src_global.to_string(),
            size_bytes,
        });
        Ok(())
    }

    /// Emits cluster barrier for multi-SM coordination.
    pub fn emit_cluster_barrier(&mut self) -> Result<(), String> {
        if !self.target.has_cluster_launch() {
            return Err("Cluster launch not supported".to_string());
        }
        self.instructions.push(PtxInstruction::ClusterBarrier);
        Ok(())
    }

    /// Emits a generic PTX instruction line.
    pub fn emit_raw(&mut self, text: &str) {
        self.instructions.push(PtxInstruction::Generic {
            text: text.to_string(),
        });
    }

    /// Allocates a temporary register name.
    pub fn alloc_reg(&mut self, prefix: &str) -> String {
        let name = format!("%{}_{}", prefix, self.next_reg);
        self.next_reg += 1;
        name
    }

    /// Renders the entire PTX module as text.
    pub fn render(&self) -> String {
        self.instructions
            .iter()
            .map(|i| format!("{}", i))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Generates the ptxas command to compile PTX to CUBIN.
    pub fn ptxas_command(&self, ptx_path: &str, cubin_path: &str) -> String {
        format!(
            "ptxas -arch={} -o {} {}",
            self.target.sm_string(),
            cubin_path,
            ptx_path
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ptx_target_sm100() {
        let t = PtxTarget::Sm100;
        assert_eq!(t.sm_string(), "sm_100");
        assert!(t.has_tcgen05());
        assert!(t.has_tma());
        assert!(t.has_cluster_launch());
    }

    #[test]
    fn ptx_target_sm101() {
        let t = PtxTarget::Sm101;
        assert_eq!(t.sm_string(), "sm_101");
        assert_eq!(t.ptx_version(), "8.6");
    }

    #[test]
    fn ptx_type_display() {
        assert_eq!(PtxType::F32.to_string(), ".f32");
        assert_eq!(PtxType::Bf16.to_string(), ".bf16");
        assert_eq!(PtxType::Fp8E4m3.to_string(), ".e4m3");
        assert_eq!(PtxType::Fp4E2m1.to_string(), ".e2m1");
        assert_eq!(PtxType::Pred.to_string(), ".pred");
    }

    #[test]
    fn tc_data_type_display() {
        assert_eq!(TcDataType::Fp32.to_string(), "fp32");
        assert_eq!(TcDataType::Bf16.to_string(), "bf16");
        assert_eq!(TcDataType::Fp8E4m3.to_string(), "e4m3");
        assert_eq!(TcDataType::Fp4E2m1.to_string(), "fp4");
    }

    #[test]
    fn mma_shape_display() {
        assert_eq!(MmaShape::default_fp16().to_string(), "m16n8k16");
        assert_eq!(MmaShape::default_fp8().to_string(), "m16n8k32");
        assert_eq!(MmaShape::default_fp4().to_string(), "m16n8k64");
    }

    #[test]
    fn emit_header() {
        let mut e = PtxEmitter::new(PtxTarget::Sm101);
        e.emit_header();
        let ptx = e.render();
        assert!(ptx.contains(".version 8.6"));
        assert!(ptx.contains(".target sm_101"));
        assert!(ptx.contains(".address_size 64"));
    }

    #[test]
    fn emit_kernel_entry_and_end() {
        let mut e = PtxEmitter::new(PtxTarget::Sm101);
        e.emit_kernel_entry("matmul_kernel");
        e.emit_kernel_end();
        let ptx = e.render();
        assert!(ptx.contains(".visible .entry matmul_kernel("));
        assert!(ptx.contains("ret"));
        assert!(ptx.contains("}"));
    }

    #[test]
    fn emit_tcgen05_mma_bf16() {
        let mut e = PtxEmitter::new(PtxTarget::Sm101);
        e.emit_bf16_mma().unwrap();
        let ptx = e.render();
        assert!(ptx.contains("tcgen05.mma"));
        assert!(ptx.contains("bf16"));
        assert!(ptx.contains("m16n8k16"));
    }

    #[test]
    fn emit_tcgen05_mma_fp4() {
        let mut e = PtxEmitter::new(PtxTarget::Sm101);
        e.emit_fp4_mma().unwrap();
        let ptx = e.render();
        assert!(ptx.contains("fp4"));
        assert!(ptx.contains("m16n8k64"));
    }

    #[test]
    fn emit_tcgen05_mma_fp8_e4m3() {
        let mut e = PtxEmitter::new(PtxTarget::Sm101);
        e.emit_fp8_e4m3_mma().unwrap();
        let ptx = e.render();
        assert!(ptx.contains("e4m3"));
        assert!(ptx.contains("m16n8k32"));
    }

    #[test]
    fn emit_tcgen05_mma_fp8_e5m2() {
        let mut e = PtxEmitter::new(PtxTarget::Sm101);
        e.emit_fp8_e5m2_mma().unwrap();
        let ptx = e.render();
        assert!(ptx.contains("e5m2"));
    }

    #[test]
    fn emit_tmem_load_store() {
        let mut e = PtxEmitter::new(PtxTarget::Sm101);
        e.emit_tmem_load("%r1", "tmem_addr", PtxType::F32);
        e.emit_tmem_store("tmem_addr", "%r1", PtxType::F32);
        let ptx = e.render();
        assert!(ptx.contains("tcgen05.ld.sync.aligned"));
        assert!(ptx.contains("tcgen05.st.sync.aligned"));
    }

    #[test]
    fn emit_tma_bulk_copy() {
        let mut e = PtxEmitter::new(PtxTarget::Sm101);
        e.emit_tma_bulk_copy("shared_buf", "global_buf", 4096)
            .unwrap();
        let ptx = e.render();
        assert!(ptx.contains("cp.async.bulk.shared.global"));
        assert!(ptx.contains("4096"));
    }

    #[test]
    fn emit_cluster_barrier() {
        let mut e = PtxEmitter::new(PtxTarget::Sm101);
        e.emit_cluster_barrier().unwrap();
        let ptx = e.render();
        assert!(ptx.contains("barrier.cluster.arrive.aligned"));
    }

    #[test]
    fn emit_reg_decl() {
        let mut e = PtxEmitter::new(PtxTarget::Sm101);
        e.emit_reg_decl(PtxType::F32, "%f", 16);
        let ptx = e.render();
        assert!(ptx.contains(".reg .f32 %f<16>"));
    }

    #[test]
    fn alloc_reg() {
        let mut e = PtxEmitter::new(PtxTarget::Sm101);
        let r1 = e.alloc_reg("f");
        let r2 = e.alloc_reg("f");
        assert_eq!(r1, "%f_0");
        assert_eq!(r2, "%f_1");
    }

    #[test]
    fn ptxas_command() {
        let e = PtxEmitter::new(PtxTarget::Sm101);
        let cmd = e.ptxas_command("kernel.ptx", "kernel.cubin");
        assert_eq!(cmd, "ptxas -arch=sm_101 -o kernel.cubin kernel.ptx");
    }

    #[test]
    fn full_ptx_kernel() {
        let mut e = PtxEmitter::new(PtxTarget::Sm101);
        e.emit_header();
        e.emit_kernel_entry("fp4_inference");
        e.emit_reg_decl(PtxType::F32, "%f", 8);
        e.emit_reg_decl(PtxType::B32, "%r", 4);
        e.emit_tma_bulk_copy("smem_a", "gmem_a", 16384).unwrap();
        e.emit_fp4_mma().unwrap();
        e.emit_tmem_store("smem_out", "%f_0", PtxType::F32);
        e.emit_cluster_barrier().unwrap();
        e.emit_kernel_end();

        let ptx = e.render();
        assert!(ptx.contains(".version 8.6"));
        assert!(ptx.contains("fp4_inference"));
        assert!(ptx.contains("tcgen05.mma"));
        assert!(ptx.contains("cp.async.bulk"));
        assert!(ptx.contains("barrier.cluster"));
        assert!(ptx.contains("ret"));
    }

    #[test]
    fn emit_raw_instruction() {
        let mut e = PtxEmitter::new(PtxTarget::Sm101);
        e.emit_raw("mov.u32 %r0, %tid.x");
        let ptx = e.render();
        assert!(ptx.contains("mov.u32 %r0, %tid.x"));
    }
}
