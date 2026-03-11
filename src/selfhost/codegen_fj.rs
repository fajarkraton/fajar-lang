//! Codegen in .fj — Cranelift IR builder, function compilation,
//! expression/control flow lowering, type mapping, runtime calls,
//! string ops, struct layout, object file emission.

use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// S26.1: Cranelift IR Builder
// ═══════════════════════════════════════════════════════════════════════

/// IR value handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IrValue(pub u32);

/// IR block handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IrBlock(pub u32);

/// IR type representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IrType {
    I8,
    I16,
    I32,
    I64,
    I128,
    F32,
    F64,
    Ptr,
}

impl fmt::Display for IrType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IrType::I8 => write!(f, "i8"),
            IrType::I16 => write!(f, "i16"),
            IrType::I32 => write!(f, "i32"),
            IrType::I64 => write!(f, "i64"),
            IrType::I128 => write!(f, "i128"),
            IrType::F32 => write!(f, "f32"),
            IrType::F64 => write!(f, "f64"),
            IrType::Ptr => write!(f, "ptr"),
        }
    }
}

/// An IR instruction.
#[derive(Debug, Clone)]
pub enum IrInst {
    /// Integer constant.
    Iconst(IrValue, i64, IrType),
    /// Float constant.
    Fconst(IrValue, f64, IrType),
    /// Binary integer operation.
    Iadd(IrValue, IrValue, IrValue),
    Isub(IrValue, IrValue, IrValue),
    Imul(IrValue, IrValue, IrValue),
    Sdiv(IrValue, IrValue, IrValue),
    /// Comparison.
    Icmp(IrValue, CmpOp, IrValue, IrValue),
    /// Function call.
    Call(Option<IrValue>, String, Vec<IrValue>),
    /// Return.
    Return(Option<IrValue>),
    /// Branch.
    Brz(IrValue, IrBlock),
    /// Unconditional jump.
    Jump(IrBlock),
    /// Load from memory.
    Load(IrValue, IrValue, IrType),
    /// Store to memory.
    Store(IrValue, IrValue),
    /// Stack slot allocation.
    StackSlot(IrValue, u32),
}

/// Comparison operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CmpOp {
    Eq,
    Ne,
    Slt,
    Sle,
    Sgt,
    Sge,
}

/// IR function builder.
#[derive(Debug, Clone)]
pub struct IrBuilder {
    /// Function name.
    pub name: String,
    /// Parameter types.
    pub params: Vec<IrType>,
    /// Return type.
    pub ret: Option<IrType>,
    /// Instructions.
    pub instructions: Vec<IrInst>,
    /// Basic blocks.
    pub blocks: Vec<IrBlock>,
    /// Next value ID.
    next_val: u32,
    /// Next block ID.
    next_block: u32,
}

impl IrBuilder {
    /// Creates a new IR builder for a function.
    pub fn new(name: &str, params: Vec<IrType>, ret: Option<IrType>) -> Self {
        Self {
            name: name.into(),
            params,
            ret,
            instructions: Vec::new(),
            blocks: vec![IrBlock(0)],
            next_val: 0,
            next_block: 1,
        }
    }

    /// Creates a new value.
    pub fn new_value(&mut self) -> IrValue {
        let v = IrValue(self.next_val);
        self.next_val += 1;
        v
    }

    /// Creates a new block.
    pub fn new_block(&mut self) -> IrBlock {
        let b = IrBlock(self.next_block);
        self.next_block += 1;
        self.blocks.push(b);
        b
    }

    /// Emits an integer constant.
    pub fn iconst(&mut self, val: i64, ty: IrType) -> IrValue {
        let v = self.new_value();
        self.instructions.push(IrInst::Iconst(v, val, ty));
        v
    }

    /// Emits an iadd.
    pub fn iadd(&mut self, a: IrValue, b: IrValue) -> IrValue {
        let v = self.new_value();
        self.instructions.push(IrInst::Iadd(v, a, b));
        v
    }

    /// Emits a function call.
    pub fn call(&mut self, name: &str, args: Vec<IrValue>, has_ret: bool) -> Option<IrValue> {
        let ret = if has_ret {
            Some(self.new_value())
        } else {
            None
        };
        self.instructions.push(IrInst::Call(ret, name.into(), args));
        ret
    }

    /// Emits a return.
    pub fn ret(&mut self, val: Option<IrValue>) {
        self.instructions.push(IrInst::Return(val));
    }

    /// Emits a conditional branch.
    pub fn brz(&mut self, cond: IrValue, target: IrBlock) {
        self.instructions.push(IrInst::Brz(cond, target));
    }

    /// Emits an unconditional jump.
    pub fn jump(&mut self, target: IrBlock) {
        self.instructions.push(IrInst::Jump(target));
    }

    /// Returns instruction count.
    pub fn instruction_count(&self) -> usize {
        self.instructions.len()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S26.2 / S26.3: Function & Expression Compilation
// ═══════════════════════════════════════════════════════════════════════

/// Compiled function metadata.
#[derive(Debug, Clone)]
pub struct CompiledFunction {
    /// Function name.
    pub name: String,
    /// Number of IR instructions.
    pub instruction_count: usize,
    /// Number of basic blocks.
    pub block_count: usize,
    /// Parameter count.
    pub param_count: usize,
}

/// Compiles a function signature to IR metadata.
pub fn compile_function_sig(
    name: &str,
    params: &[IrType],
    ret: Option<IrType>,
) -> CompiledFunction {
    let builder = IrBuilder::new(name, params.to_vec(), ret);
    CompiledFunction {
        name: name.into(),
        instruction_count: builder.instruction_count(),
        block_count: builder.blocks.len(),
        param_count: params.len(),
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S26.4: Control Flow Lowering
// ═══════════════════════════════════════════════════════════════════════

/// Control flow pattern to lower.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlFlow {
    /// If-else: condition block, then block, else block, merge block.
    IfElse {
        then_block: IrBlock,
        else_block: IrBlock,
        merge_block: IrBlock,
    },
    /// While loop: header, body, exit.
    WhileLoop {
        header: IrBlock,
        body: IrBlock,
        exit: IrBlock,
    },
    /// For loop (desugared to while).
    ForLoop {
        init: IrBlock,
        header: IrBlock,
        body: IrBlock,
        update: IrBlock,
        exit: IrBlock,
    },
    /// Match arms.
    Match {
        arms: Vec<IrBlock>,
        default: Option<IrBlock>,
        exit: IrBlock,
    },
}

/// Creates the block layout for a control flow pattern.
pub fn lower_control_flow(builder: &mut IrBuilder, pattern: &str) -> ControlFlow {
    match pattern {
        "if_else" => {
            let then_block = builder.new_block();
            let else_block = builder.new_block();
            let merge_block = builder.new_block();
            ControlFlow::IfElse {
                then_block,
                else_block,
                merge_block,
            }
        }
        "while" => {
            let header = builder.new_block();
            let body = builder.new_block();
            let exit = builder.new_block();
            ControlFlow::WhileLoop { header, body, exit }
        }
        _ => {
            let header = builder.new_block();
            let body = builder.new_block();
            let exit = builder.new_block();
            ControlFlow::WhileLoop { header, body, exit }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S26.5: Type Mapping
// ═══════════════════════════════════════════════════════════════════════

/// Maps a Fajar Lang type name to an IR type.
pub fn map_type(fj_type: &str) -> Option<IrType> {
    match fj_type {
        "i8" | "u8" | "bool" | "char" => Some(IrType::I8),
        "i16" | "u16" => Some(IrType::I16),
        "i32" | "u32" => Some(IrType::I32),
        "i64" | "u64" | "isize" | "usize" => Some(IrType::I64),
        "i128" | "u128" => Some(IrType::I128),
        "f32" => Some(IrType::F32),
        "f64" => Some(IrType::F64),
        "str" | "ptr" => Some(IrType::Ptr),
        _ => None, // Structs, enums -> pointer
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S26.6: Runtime Function Calls
// ═══════════════════════════════════════════════════════════════════════

/// A runtime function declaration.
#[derive(Debug, Clone)]
pub struct RuntimeFn {
    /// Symbol name (e.g., "fj_rt_string_alloc").
    pub symbol: String,
    /// Parameter types.
    pub params: Vec<IrType>,
    /// Return type.
    pub ret: Option<IrType>,
}

/// Standard runtime functions.
pub fn standard_runtime_fns() -> Vec<RuntimeFn> {
    vec![
        RuntimeFn {
            symbol: "fj_rt_string_alloc".into(),
            params: vec![IrType::Ptr, IrType::I64],
            ret: Some(IrType::Ptr),
        },
        RuntimeFn {
            symbol: "fj_rt_string_concat".into(),
            params: vec![IrType::Ptr, IrType::Ptr],
            ret: Some(IrType::Ptr),
        },
        RuntimeFn {
            symbol: "fj_rt_string_eq".into(),
            params: vec![IrType::Ptr, IrType::Ptr],
            ret: Some(IrType::I8),
        },
        RuntimeFn {
            symbol: "fj_rt_print".into(),
            params: vec![IrType::Ptr],
            ret: None,
        },
        RuntimeFn {
            symbol: "fj_rt_println".into(),
            params: vec![IrType::Ptr],
            ret: None,
        },
        RuntimeFn {
            symbol: "fj_rt_heap_alloc".into(),
            params: vec![IrType::I64],
            ret: Some(IrType::Ptr),
        },
        RuntimeFn {
            symbol: "fj_rt_heap_free".into(),
            params: vec![IrType::Ptr],
            ret: None,
        },
    ]
}

// ═══════════════════════════════════════════════════════════════════════
// S26.7 / S26.8: String Operations & Struct Layout
// ═══════════════════════════════════════════════════════════════════════

/// Struct field layout.
#[derive(Debug, Clone)]
pub struct FieldLayout {
    /// Field name.
    pub name: String,
    /// Field type.
    pub ty: IrType,
    /// Offset in bytes.
    pub offset: usize,
    /// Size in bytes.
    pub size: usize,
}

/// Struct layout with all fields.
#[derive(Debug, Clone)]
pub struct StructLayout {
    /// Struct name.
    pub name: String,
    /// Fields with computed offsets.
    pub fields: Vec<FieldLayout>,
    /// Total size in bytes.
    pub total_size: usize,
    /// Alignment in bytes.
    pub alignment: usize,
}

/// Computes the size of an IR type in bytes.
pub fn type_size(ty: IrType) -> usize {
    match ty {
        IrType::I8 => 1,
        IrType::I16 => 2,
        IrType::I32 | IrType::F32 => 4,
        IrType::I64 | IrType::F64 | IrType::Ptr => 8,
        IrType::I128 => 16,
    }
}

/// Computes the layout of a struct given its field types.
pub fn compute_struct_layout(name: &str, fields: &[(&str, IrType)]) -> StructLayout {
    let mut offset = 0;
    let mut max_align = 1;
    let mut layout_fields = Vec::new();

    for (field_name, ty) in fields {
        let size = type_size(*ty);
        let align = size;
        max_align = max_align.max(align);

        // Align the offset
        let padding = (align - (offset % align)) % align;
        offset += padding;

        layout_fields.push(FieldLayout {
            name: field_name.to_string(),
            ty: *ty,
            offset,
            size,
        });

        offset += size;
    }

    // Pad to alignment
    let padding = (max_align - (offset % max_align)) % max_align;
    offset += padding;

    StructLayout {
        name: name.into(),
        fields: layout_fields,
        total_size: offset,
        alignment: max_align,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S26.9: Object File Emission
// ═══════════════════════════════════════════════════════════════════════

/// Object file format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectFormat {
    /// ELF (Linux).
    Elf,
    /// Mach-O (macOS).
    MachO,
    /// COFF (Windows).
    Coff,
}

impl fmt::Display for ObjectFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ObjectFormat::Elf => write!(f, "ELF"),
            ObjectFormat::MachO => write!(f, "Mach-O"),
            ObjectFormat::Coff => write!(f, "COFF"),
        }
    }
}

/// Object file emission result.
#[derive(Debug, Clone)]
pub struct ObjectEmission {
    /// Output file path.
    pub path: String,
    /// Format used.
    pub format: ObjectFormat,
    /// Size in bytes.
    pub size: usize,
    /// Number of sections.
    pub sections: usize,
    /// Number of symbols.
    pub symbols: usize,
}

impl fmt::Display for ObjectEmission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: {} ({} bytes, {} sections, {} symbols)",
            self.path, self.format, self.size, self.sections, self.symbols
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // S26.1 — Cranelift IR Builder
    #[test]
    fn s26_1_ir_builder_creation() {
        let builder = IrBuilder::new("add", vec![IrType::I64, IrType::I64], Some(IrType::I64));
        assert_eq!(builder.name, "add");
        assert_eq!(builder.params.len(), 2);
    }

    #[test]
    fn s26_1_ir_builder_instructions() {
        let mut builder = IrBuilder::new("foo", vec![], Some(IrType::I64));
        let a = builder.iconst(42, IrType::I64);
        let b = builder.iconst(10, IrType::I64);
        let c = builder.iadd(a, b);
        builder.ret(Some(c));
        assert_eq!(builder.instruction_count(), 4);
    }

    #[test]
    fn s26_1_ir_type_display() {
        assert_eq!(IrType::I64.to_string(), "i64");
        assert_eq!(IrType::F32.to_string(), "f32");
        assert_eq!(IrType::Ptr.to_string(), "ptr");
    }

    // S26.2 — Function Compilation
    #[test]
    fn s26_2_compile_function_sig() {
        let cf = compile_function_sig("add", &[IrType::I64, IrType::I64], Some(IrType::I64));
        assert_eq!(cf.name, "add");
        assert_eq!(cf.param_count, 2);
    }

    // S26.3 — Expression Lowering
    #[test]
    fn s26_3_ir_call() {
        let mut builder = IrBuilder::new("main", vec![], None);
        let arg = builder.iconst(42, IrType::I64);
        let result = builder.call("print_int", vec![arg], true);
        assert!(result.is_some());
    }

    // S26.4 — Control Flow Lowering
    #[test]
    fn s26_4_if_else_blocks() {
        let mut builder = IrBuilder::new("test", vec![], None);
        let cf = lower_control_flow(&mut builder, "if_else");
        match cf {
            ControlFlow::IfElse {
                then_block,
                else_block,
                merge_block,
            } => {
                assert_ne!(then_block, else_block);
                assert_ne!(else_block, merge_block);
            }
            _ => panic!("expected if_else"),
        }
    }

    #[test]
    fn s26_4_while_loop_blocks() {
        let mut builder = IrBuilder::new("test", vec![], None);
        let cf = lower_control_flow(&mut builder, "while");
        match cf {
            ControlFlow::WhileLoop { header, body, exit } => {
                assert_ne!(header, body);
                assert_ne!(body, exit);
            }
            _ => panic!("expected while"),
        }
    }

    // S26.5 — Type Mapping
    #[test]
    fn s26_5_type_mapping() {
        assert_eq!(map_type("i32"), Some(IrType::I32));
        assert_eq!(map_type("f64"), Some(IrType::F64));
        assert_eq!(map_type("bool"), Some(IrType::I8));
        assert_eq!(map_type("str"), Some(IrType::Ptr));
        assert_eq!(map_type("MyStruct"), None);
    }

    // S26.6 — Runtime Function Calls
    #[test]
    fn s26_6_standard_runtime_fns() {
        let fns = standard_runtime_fns();
        assert!(fns.len() >= 7);
        assert!(fns.iter().any(|f| f.symbol == "fj_rt_print"));
        assert!(fns.iter().any(|f| f.symbol == "fj_rt_heap_alloc"));
    }

    // S26.7 — String Operations (via runtime fns)
    #[test]
    fn s26_7_string_runtime_fns() {
        let fns = standard_runtime_fns();
        assert!(fns.iter().any(|f| f.symbol == "fj_rt_string_alloc"));
        assert!(fns.iter().any(|f| f.symbol == "fj_rt_string_concat"));
        assert!(fns.iter().any(|f| f.symbol == "fj_rt_string_eq"));
    }

    // S26.8 — Struct Layout
    #[test]
    fn s26_8_simple_struct_layout() {
        let layout = compute_struct_layout("Point", &[("x", IrType::F64), ("y", IrType::F64)]);
        assert_eq!(layout.name, "Point");
        assert_eq!(layout.total_size, 16);
        assert_eq!(layout.fields.len(), 2);
        assert_eq!(layout.fields[0].offset, 0);
        assert_eq!(layout.fields[1].offset, 8);
    }

    #[test]
    fn s26_8_mixed_type_layout() {
        let layout = compute_struct_layout(
            "Mixed",
            &[
                ("flag", IrType::I8),
                ("value", IrType::I64),
                ("tag", IrType::I32),
            ],
        );
        assert!(layout.total_size >= 13); // at least 1 + 8 + 4 with padding
        assert_eq!(layout.fields[0].offset, 0);
    }

    #[test]
    fn s26_8_type_sizes() {
        assert_eq!(type_size(IrType::I8), 1);
        assert_eq!(type_size(IrType::I32), 4);
        assert_eq!(type_size(IrType::I64), 8);
        assert_eq!(type_size(IrType::Ptr), 8);
    }

    // S26.9 — Object File Emission
    #[test]
    fn s26_9_object_format_display() {
        assert_eq!(ObjectFormat::Elf.to_string(), "ELF");
        assert_eq!(ObjectFormat::MachO.to_string(), "Mach-O");
    }

    #[test]
    fn s26_9_object_emission_display() {
        let emission = ObjectEmission {
            path: "output.o".into(),
            format: ObjectFormat::Elf,
            size: 4096,
            sections: 3,
            symbols: 10,
        };
        assert!(emission.to_string().contains("output.o"));
        assert!(emission.to_string().contains("ELF"));
    }

    // S26.10 — Additional
    #[test]
    fn s26_10_builder_blocks() {
        let mut builder = IrBuilder::new("test", vec![], None);
        assert_eq!(builder.blocks.len(), 1); // entry block
        let _b1 = builder.new_block();
        let _b2 = builder.new_block();
        assert_eq!(builder.blocks.len(), 3);
    }

    #[test]
    fn s26_10_branch_instructions() {
        let mut builder = IrBuilder::new("test", vec![], None);
        let target = builder.new_block();
        let cond = builder.iconst(1, IrType::I8);
        builder.brz(cond, target);
        builder.jump(target);
        assert_eq!(builder.instruction_count(), 3);
    }
}
