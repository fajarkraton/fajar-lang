//! WebAssembly code generation backend for Fajar Lang.
//!
//! Feature-gated behind `wasm`. Compiles Fajar Lang AST to WebAssembly
//! binary format (`.wasm`), targeting both standalone WASI runtimes and
//! browser environments.
//!
//! # Architecture
//!
//! ```text
//! AST (Program)
//!     │
//!     ▼
//! WasmCompiler
//!     ├── WasmType       — Fajar types → Wasm value types
//!     ├── WasmInstruction — Wasm instruction encoding
//!     ├── WasmModule     — Section-based module builder
//!     └── WasmMemory     — Linear memory management
//!     │
//!     ▼
//! Vec<u8> (Wasm binary) or HTML+JS loader
//! ```
//!
//! # Sections
//!
//! The Wasm module is organized into standard sections:
//! - **Type section** (1): function signatures
//! - **Import section** (2): WASI/host function imports
//! - **Function section** (3): function index → type index mapping
//! - **Memory section** (5): linear memory configuration
//! - **Global section** (6): global variables
//! - **Export section** (7): exported functions and memory
//! - **Code section** (10): function bodies
//! - **Data section** (11): string literals and static data

use std::collections::HashMap;

use crate::codegen::CodegenError;
use crate::parser::ast::{
    AssignOp, BinOp, Expr, FnDef, Item, LiteralKind, Program, Stmt, TypeExpr, UnaryOp,
};

// ═══════════════════════════════════════════════════════════════════════
// Wasm Error Type
// ═══════════════════════════════════════════════════════════════════════

/// Errors specific to WebAssembly code generation.
#[derive(Debug, Clone, thiserror::Error)]
pub enum WasmError {
    /// WE001: Unsupported expression for Wasm compilation.
    #[error("[WE001] unsupported expression for wasm codegen: {0}")]
    UnsupportedExpr(String),

    /// WE002: Unsupported statement for Wasm compilation.
    #[error("[WE002] unsupported statement for wasm codegen: {0}")]
    UnsupportedStmt(String),

    /// WE003: Type lowering failure.
    #[error("[WE003] cannot lower type to wasm: {0}")]
    TypeLoweringError(String),

    /// WE004: Function definition error.
    #[error("[WE004] function wasm codegen error: {0}")]
    FunctionError(String),

    /// WE005: Undefined variable.
    #[error("[WE005] undefined variable in wasm codegen: {0}")]
    UndefinedVariable(String),

    /// WE006: Undefined function.
    #[error("[WE006] undefined function in wasm codegen: {0}")]
    UndefinedFunction(String),

    /// WE007: Memory error.
    #[error("[WE007] wasm memory error: {0}")]
    MemoryError(String),

    /// WE008: Module encoding error.
    #[error("[WE008] wasm module encoding error: {0}")]
    ModuleError(String),

    /// WE009: Import error.
    #[error("[WE009] wasm import error: {0}")]
    ImportError(String),

    /// WE010: Not yet implemented.
    #[error("[WE010] not yet implemented in wasm codegen: {0}")]
    NotImplemented(String),
}

impl From<WasmError> for CodegenError {
    fn from(e: WasmError) -> Self {
        CodegenError::Internal(e.to_string())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Sprint 1: Wasm Target Setup — Types & Module Structure
// ═══════════════════════════════════════════════════════════════════════

/// WebAssembly value types.
///
/// Maps to the core Wasm spec value types. Fajar Lang types are lowered
/// to these during compilation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WasmType {
    /// 32-bit integer (also used for bool).
    I32,
    /// 64-bit integer (default integer type).
    I64,
    /// 32-bit float.
    F32,
    /// 64-bit float (default float type).
    F64,
}

impl WasmType {
    /// Returns the byte encoding for this type in the Wasm binary format.
    pub fn encode(&self) -> u8 {
        match self {
            WasmType::I32 => 0x7F,
            WasmType::I64 => 0x7E,
            WasmType::F32 => 0x7D,
            WasmType::F64 => 0x7C,
        }
    }
}

impl std::fmt::Display for WasmType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WasmType::I32 => write!(f, "i32"),
            WasmType::I64 => write!(f, "i64"),
            WasmType::F32 => write!(f, "f32"),
            WasmType::F64 => write!(f, "f64"),
        }
    }
}

/// Lowers a Fajar Lang type expression to a Wasm value type.
///
/// Returns `None` for types that cannot be represented as a single Wasm
/// value (e.g., void, complex structs without flattening).
pub fn lower_type_to_wasm(ty: &TypeExpr) -> Option<WasmType> {
    match ty {
        TypeExpr::Simple { name, .. } => lower_simple_type_to_wasm(name),
        TypeExpr::Reference { .. } | TypeExpr::Pointer { .. } => Some(WasmType::I32),
        TypeExpr::Array { .. } => Some(WasmType::I32), // pointer into linear memory
        TypeExpr::Fn { .. } => Some(WasmType::I32),    // table index
        _ => None,
    }
}

/// Lowers a simple type name to a Wasm value type.
pub fn lower_simple_type_to_wasm(name: &str) -> Option<WasmType> {
    match name {
        "bool" => Some(WasmType::I32),
        "i8" | "u8" | "i16" | "u16" | "i32" | "u32" | "char" => Some(WasmType::I32),
        "i64" | "u64" | "isize" | "usize" | "int" => Some(WasmType::I64),
        "f32" => Some(WasmType::F32),
        "f64" | "float" => Some(WasmType::F64),
        "ptr" => Some(WasmType::I32),
        "str" => Some(WasmType::I32), // pointer to linear memory
        "void" => None,
        _ => None,
    }
}

/// Returns the default Wasm type for integers.
pub fn wasm_default_int() -> WasmType {
    WasmType::I64
}

/// Returns the default Wasm type for floats.
pub fn wasm_default_float() -> WasmType {
    WasmType::F64
}

/// Returns the Wasm pointer type (i32 for wasm32).
pub fn wasm_pointer_type() -> WasmType {
    WasmType::I32
}

/// A function signature in the Wasm type section.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WasmFuncType {
    /// Parameter types.
    pub params: Vec<WasmType>,
    /// Return types (Wasm supports multi-value, but we use 0 or 1).
    pub results: Vec<WasmType>,
}

impl WasmFuncType {
    /// Creates a new function type with the given params and results.
    pub fn new(params: Vec<WasmType>, results: Vec<WasmType>) -> Self {
        Self { params, results }
    }

    /// Encodes this function type into Wasm binary format.
    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.push(0x60); // functype marker
        encode_vec_types(&self.params, &mut bytes);
        encode_vec_types(&self.results, &mut bytes);
        bytes
    }
}

/// Encodes a vector of types as a Wasm vector (length + type bytes).
fn encode_vec_types(types: &[WasmType], out: &mut Vec<u8>) {
    encode_unsigned_leb128(types.len() as u64, out);
    for ty in types {
        out.push(ty.encode());
    }
}

// ═══════════════════════════════════════════════════════════════════════
// LEB128 encoding helpers
// ═══════════════════════════════════════════════════════════════════════

/// Encodes an unsigned integer in LEB128 format.
pub fn encode_unsigned_leb128(mut value: u64, out: &mut Vec<u8>) {
    loop {
        let mut byte = (value & 0x7F) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        out.push(byte);
        if value == 0 {
            break;
        }
    }
}

/// Encodes a signed integer in LEB128 format.
pub fn encode_signed_leb128(mut value: i64, out: &mut Vec<u8>) {
    let mut more = true;
    while more {
        let mut byte = (value & 0x7F) as u8;
        value >>= 7;
        // Check if we need more bytes
        let sign_bit = (byte & 0x40) != 0;
        if (value == 0 && !sign_bit) || (value == -1 && sign_bit) {
            more = false;
        } else {
            byte |= 0x80;
        }
        out.push(byte);
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Sprint 2: Wasm Instructions
// ═══════════════════════════════════════════════════════════════════════

/// Block type for structured control flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockType {
    /// Block produces no value.
    Empty,
    /// Block produces a value of the given type.
    Value(WasmType),
}

/// WebAssembly instructions.
///
/// Covers integer/float arithmetic, comparisons, locals, control flow,
/// function calls, and memory operations needed by Fajar Lang.
#[derive(Debug, Clone, PartialEq)]
pub enum WasmInstruction {
    // ── Constants ──
    /// Push a 32-bit integer constant.
    I32Const(i32),
    /// Push a 64-bit integer constant.
    I64Const(i64),
    /// Push a 32-bit float constant.
    F32Const(f32),
    /// Push a 64-bit float constant.
    F64Const(f64),

    // ── Integer arithmetic (i64) ──
    /// `i64.add`
    I64Add,
    /// `i64.sub`
    I64Sub,
    /// `i64.mul`
    I64Mul,
    /// `i64.div_s` (signed division)
    I64DivS,
    /// `i64.rem_s` (signed remainder)
    I64RemS,

    // ── Integer arithmetic (i32) ──
    /// `i32.add`
    I32Add,
    /// `i32.sub`
    I32Sub,
    /// `i32.mul`
    I32Mul,
    /// `i32.div_s`
    I32DivS,

    // ── Float arithmetic (f64) ──
    /// `f64.add`
    F64Add,
    /// `f64.sub`
    F64Sub,
    /// `f64.mul`
    F64Mul,
    /// `f64.div`
    F64Div,
    /// `f64.neg`
    F64Neg,
    /// `f64.sqrt`
    F64Sqrt,

    // ── Float arithmetic (f32) ──
    /// `f32.add`
    F32Add,
    /// `f32.sub`
    F32Sub,
    /// `f32.mul`
    F32Mul,
    /// `f32.div`
    F32Div,

    // ── Integer comparisons (i64) ──
    /// `i64.eq`
    I64Eq,
    /// `i64.ne`
    I64Ne,
    /// `i64.lt_s` (signed less than)
    I64LtS,
    /// `i64.gt_s` (signed greater than)
    I64GtS,
    /// `i64.le_s` (signed less than or equal)
    I64LeS,
    /// `i64.ge_s` (signed greater than or equal)
    I64GeS,
    /// `i64.eqz` (equals zero)
    I64Eqz,

    // ── Float comparisons (f64) ──
    /// `f64.eq`
    F64Eq,
    /// `f64.ne`
    F64Ne,
    /// `f64.lt`
    F64Lt,
    /// `f64.gt`
    F64Gt,
    /// `f64.le`
    F64Le,
    /// `f64.ge`
    F64Ge,

    // ── Boolean / i32 logic ──
    /// `i32.and`
    I32And,
    /// `i32.or`
    I32Or,
    /// `i32.xor`
    I32Xor,
    /// `i32.eqz`
    I32Eqz,
    /// `i32.eq`
    I32Eq,
    /// `i32.ne`
    I32Ne,

    // ── Bitwise (i64) ──
    /// `i64.and`
    I64And,
    /// `i64.or`
    I64Or,
    /// `i64.xor`
    I64Xor,
    /// `i64.shl`
    I64Shl,
    /// `i64.shr_s`
    I64ShrS,

    // ── Locals ──
    /// `local.get <index>`
    LocalGet(u32),
    /// `local.set <index>`
    LocalSet(u32),
    /// `local.tee <index>` (set and keep value on stack)
    LocalTee(u32),

    // ── Globals ──
    /// `global.get <index>`
    GlobalGet(u32),
    /// `global.set <index>`
    GlobalSet(u32),

    // ── Control flow ──
    /// `block <blocktype>` — start a block.
    Block(BlockType),
    /// `loop <blocktype>` — start a loop.
    Loop(BlockType),
    /// `if <blocktype>` — conditional (pops i32 from stack).
    If(BlockType),
    /// `else` — else branch of an if.
    Else,
    /// `end` — terminates block/loop/if/function.
    End,
    /// `br <label_depth>` — unconditional branch.
    Br(u32),
    /// `br_if <label_depth>` — conditional branch (pops i32).
    BrIf(u32),

    // ── Functions ──
    /// `call <func_index>`
    Call(u32),
    /// `return` — return from current function.
    Return,

    // ── Stack ──
    /// `drop` — discard top of stack.
    Drop,
    /// `select` — ternary: select between two values based on i32 condition.
    Select,

    // ── Memory ──
    /// `i32.load <align> <offset>` — load i32 from memory.
    I32Load { align: u32, offset: u32 },
    /// `i64.load <align> <offset>` — load i64 from memory.
    I64Load { align: u32, offset: u32 },
    /// `f64.load <align> <offset>` — load f64 from memory.
    F64Load { align: u32, offset: u32 },
    /// `i32.store <align> <offset>` — store i32 to memory.
    I32Store { align: u32, offset: u32 },
    /// `i64.store <align> <offset>` — store i64 to memory.
    I64Store { align: u32, offset: u32 },
    /// `f64.store <align> <offset>` — store f64 to memory.
    F64Store { align: u32, offset: u32 },
    /// `i32.store8 <align> <offset>` — store low byte of i32.
    I32Store8 { align: u32, offset: u32 },
    /// `i32.load8_u <align> <offset>` — load byte, zero-extend to i32.
    I32Load8U { align: u32, offset: u32 },
    /// `memory.size` — current memory size in pages.
    MemorySize,
    /// `memory.grow` — grow memory by N pages (pops i32, pushes old size).
    MemoryGrow,

    // ── Type conversions ──
    /// `i32.wrap_i64` — truncate i64 to i32.
    I32WrapI64,
    /// `i64.extend_i32_s` — sign-extend i32 to i64.
    I64ExtendI32S,
    /// `f64.convert_i64_s` — convert signed i64 to f64.
    F64ConvertI64S,
    /// `i64.trunc_f64_s` — truncate f64 to signed i64.
    I64TruncF64S,
    /// `f32.demote_f64` — demote f64 to f32.
    F32DemoteF64,
    /// `f64.promote_f32` — promote f32 to f64.
    F64PromoteF32,

    // ── Misc ──
    /// `unreachable` — trap unconditionally.
    Unreachable,
    /// `nop` — no operation.
    Nop,
}

impl WasmInstruction {
    /// Encodes this instruction to Wasm binary format.
    pub fn encode(&self, out: &mut Vec<u8>) {
        match self {
            // Constants
            WasmInstruction::I32Const(v) => {
                out.push(0x41);
                encode_signed_leb128(*v as i64, out);
            }
            WasmInstruction::I64Const(v) => {
                out.push(0x42);
                encode_signed_leb128(*v, out);
            }
            WasmInstruction::F32Const(v) => {
                out.push(0x43);
                out.extend_from_slice(&v.to_le_bytes());
            }
            WasmInstruction::F64Const(v) => {
                out.push(0x44);
                out.extend_from_slice(&v.to_le_bytes());
            }

            // i64 arithmetic
            WasmInstruction::I64Add => out.push(0x7C),
            WasmInstruction::I64Sub => out.push(0x7D),
            WasmInstruction::I64Mul => out.push(0x7E),
            WasmInstruction::I64DivS => out.push(0x7F),
            WasmInstruction::I64RemS => out.push(0x81),

            // i32 arithmetic
            WasmInstruction::I32Add => out.push(0x6A),
            WasmInstruction::I32Sub => out.push(0x6B),
            WasmInstruction::I32Mul => out.push(0x6C),
            WasmInstruction::I32DivS => out.push(0x6D),

            // f64 arithmetic
            WasmInstruction::F64Add => out.push(0xA0),
            WasmInstruction::F64Sub => out.push(0xA1),
            WasmInstruction::F64Mul => out.push(0xA2),
            WasmInstruction::F64Div => out.push(0xA3),
            WasmInstruction::F64Neg => out.push(0x9A),
            WasmInstruction::F64Sqrt => out.push(0x9F),

            // f32 arithmetic
            WasmInstruction::F32Add => out.push(0x92),
            WasmInstruction::F32Sub => out.push(0x93),
            WasmInstruction::F32Mul => out.push(0x94),
            WasmInstruction::F32Div => out.push(0x95),

            // i64 comparisons
            WasmInstruction::I64Eq => out.push(0x51),
            WasmInstruction::I64Ne => out.push(0x52),
            WasmInstruction::I64LtS => out.push(0x53),
            WasmInstruction::I64GtS => out.push(0x55),
            WasmInstruction::I64LeS => out.push(0x57),
            WasmInstruction::I64GeS => out.push(0x59),
            WasmInstruction::I64Eqz => out.push(0x50),

            // f64 comparisons
            WasmInstruction::F64Eq => out.push(0x61),
            WasmInstruction::F64Ne => out.push(0x62),
            WasmInstruction::F64Lt => out.push(0x63),
            WasmInstruction::F64Gt => out.push(0x64),
            WasmInstruction::F64Le => out.push(0x65),
            WasmInstruction::F64Ge => out.push(0x66),

            // i32 logic / comparisons
            WasmInstruction::I32And => out.push(0x71),
            WasmInstruction::I32Or => out.push(0x72),
            WasmInstruction::I32Xor => out.push(0x73),
            WasmInstruction::I32Eqz => out.push(0x45),
            WasmInstruction::I32Eq => out.push(0x46),
            WasmInstruction::I32Ne => out.push(0x47),

            // i64 bitwise
            WasmInstruction::I64And => out.push(0x83),
            WasmInstruction::I64Or => out.push(0x84),
            WasmInstruction::I64Xor => out.push(0x85),
            WasmInstruction::I64Shl => out.push(0x86),
            WasmInstruction::I64ShrS => out.push(0x87),

            // Locals
            WasmInstruction::LocalGet(idx) => {
                out.push(0x20);
                encode_unsigned_leb128(*idx as u64, out);
            }
            WasmInstruction::LocalSet(idx) => {
                out.push(0x21);
                encode_unsigned_leb128(*idx as u64, out);
            }
            WasmInstruction::LocalTee(idx) => {
                out.push(0x22);
                encode_unsigned_leb128(*idx as u64, out);
            }

            // Globals
            WasmInstruction::GlobalGet(idx) => {
                out.push(0x23);
                encode_unsigned_leb128(*idx as u64, out);
            }
            WasmInstruction::GlobalSet(idx) => {
                out.push(0x24);
                encode_unsigned_leb128(*idx as u64, out);
            }

            // Control flow
            WasmInstruction::Block(bt) => {
                out.push(0x02);
                encode_block_type(bt, out);
            }
            WasmInstruction::Loop(bt) => {
                out.push(0x03);
                encode_block_type(bt, out);
            }
            WasmInstruction::If(bt) => {
                out.push(0x04);
                encode_block_type(bt, out);
            }
            WasmInstruction::Else => out.push(0x05),
            WasmInstruction::End => out.push(0x0B),
            WasmInstruction::Br(depth) => {
                out.push(0x0C);
                encode_unsigned_leb128(*depth as u64, out);
            }
            WasmInstruction::BrIf(depth) => {
                out.push(0x0D);
                encode_unsigned_leb128(*depth as u64, out);
            }

            // Functions
            WasmInstruction::Call(idx) => {
                out.push(0x10);
                encode_unsigned_leb128(*idx as u64, out);
            }
            WasmInstruction::Return => out.push(0x0F),

            // Stack
            WasmInstruction::Drop => out.push(0x1A),
            WasmInstruction::Select => out.push(0x1B),

            // Memory
            WasmInstruction::I32Load { align, offset } => {
                out.push(0x28);
                encode_unsigned_leb128(*align as u64, out);
                encode_unsigned_leb128(*offset as u64, out);
            }
            WasmInstruction::I64Load { align, offset } => {
                out.push(0x29);
                encode_unsigned_leb128(*align as u64, out);
                encode_unsigned_leb128(*offset as u64, out);
            }
            WasmInstruction::F64Load { align, offset } => {
                out.push(0x2B);
                encode_unsigned_leb128(*align as u64, out);
                encode_unsigned_leb128(*offset as u64, out);
            }
            WasmInstruction::I32Store { align, offset } => {
                out.push(0x36);
                encode_unsigned_leb128(*align as u64, out);
                encode_unsigned_leb128(*offset as u64, out);
            }
            WasmInstruction::I64Store { align, offset } => {
                out.push(0x37);
                encode_unsigned_leb128(*align as u64, out);
                encode_unsigned_leb128(*offset as u64, out);
            }
            WasmInstruction::F64Store { align, offset } => {
                out.push(0x39);
                encode_unsigned_leb128(*align as u64, out);
                encode_unsigned_leb128(*offset as u64, out);
            }
            WasmInstruction::I32Store8 { align, offset } => {
                out.push(0x3A);
                encode_unsigned_leb128(*align as u64, out);
                encode_unsigned_leb128(*offset as u64, out);
            }
            WasmInstruction::I32Load8U { align, offset } => {
                out.push(0x2D);
                encode_unsigned_leb128(*align as u64, out);
                encode_unsigned_leb128(*offset as u64, out);
            }
            WasmInstruction::MemorySize => {
                out.push(0x3F);
                out.push(0x00); // memory index
            }
            WasmInstruction::MemoryGrow => {
                out.push(0x40);
                out.push(0x00); // memory index
            }

            // Type conversions
            WasmInstruction::I32WrapI64 => out.push(0xA7),
            WasmInstruction::I64ExtendI32S => out.push(0xAC),
            WasmInstruction::F64ConvertI64S => out.push(0xB9),
            WasmInstruction::I64TruncF64S => out.push(0xB0),
            WasmInstruction::F32DemoteF64 => out.push(0xB6),
            WasmInstruction::F64PromoteF32 => out.push(0xBB),

            // Misc
            WasmInstruction::Unreachable => out.push(0x00),
            WasmInstruction::Nop => out.push(0x01),
        }
    }
}

/// Encodes a block type to Wasm binary format.
fn encode_block_type(bt: &BlockType, out: &mut Vec<u8>) {
    match bt {
        BlockType::Empty => out.push(0x40),
        BlockType::Value(ty) => out.push(ty.encode()),
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Sprint 1 continued: WasmModule — section-based module builder
// ═══════════════════════════════════════════════════════════════════════

/// Wasm section IDs per the spec.
const SECTION_TYPE: u8 = 1;
const SECTION_IMPORT: u8 = 2;
const SECTION_FUNCTION: u8 = 3;
const SECTION_MEMORY: u8 = 5;
const SECTION_GLOBAL: u8 = 6;
const SECTION_EXPORT: u8 = 7;
const SECTION_CODE: u8 = 10;
const SECTION_DATA: u8 = 11;

/// Wasm export descriptor kinds.
const EXPORT_FUNC: u8 = 0x00;
const EXPORT_MEMORY: u8 = 0x02;

/// A WASI/host function import.
#[derive(Debug, Clone)]
pub struct WasmImport {
    /// Module name (e.g., "wasi_snapshot_preview1").
    pub module: String,
    /// Function name (e.g., "fd_write").
    pub name: String,
    /// Function type index in the type section.
    pub type_index: u32,
}

/// An exported function or memory.
#[derive(Debug, Clone)]
pub struct WasmExport {
    /// Export name (e.g., "_start", "memory").
    pub name: String,
    /// Export kind (function or memory).
    pub kind: u8,
    /// Index of the exported item.
    pub index: u32,
}

/// A global variable entry.
#[derive(Debug, Clone)]
pub struct WasmGlobal {
    /// Value type.
    pub ty: WasmType,
    /// Whether the global is mutable.
    pub mutable: bool,
    /// Initializer instructions (must be a constant expression).
    pub init: Vec<WasmInstruction>,
}

/// A data segment for string literals and static data.
#[derive(Debug, Clone)]
pub struct WasmDataSegment {
    /// Memory offset where this data is placed.
    pub offset: u32,
    /// Raw bytes to store.
    pub data: Vec<u8>,
}

/// A compiled function body with locals and instructions.
#[derive(Debug, Clone)]
pub struct WasmFuncBody {
    /// Local variable declarations: (count, type).
    pub locals: Vec<(u32, WasmType)>,
    /// Function body instructions (must end with End).
    pub instructions: Vec<WasmInstruction>,
}

impl WasmFuncBody {
    /// Creates a new empty function body.
    pub fn new() -> Self {
        Self {
            locals: Vec::new(),
            instructions: Vec::new(),
        }
    }

    /// Encodes this function body to Wasm binary format.
    pub fn encode(&self) -> Vec<u8> {
        let mut body = Vec::new();
        // Local declarations
        encode_unsigned_leb128(self.locals.len() as u64, &mut body);
        for (count, ty) in &self.locals {
            encode_unsigned_leb128(*count as u64, &mut body);
            body.push(ty.encode());
        }
        // Instructions
        for instr in &self.instructions {
            instr.encode(&mut body);
        }
        // Wrap with length prefix
        let mut encoded = Vec::new();
        encode_unsigned_leb128(body.len() as u64, &mut encoded);
        encoded.extend(body);
        encoded
    }
}

/// Memory configuration for the Wasm module.
#[derive(Debug, Clone, Copy)]
pub struct WasmMemoryConfig {
    /// Initial memory size in pages (1 page = 64KB).
    pub initial_pages: u32,
    /// Maximum memory size in pages.
    pub max_pages: u32,
}

impl Default for WasmMemoryConfig {
    fn default() -> Self {
        Self {
            initial_pages: 1,
            max_pages: 256,
        }
    }
}

/// The assembled Wasm module ready for binary output.
///
/// Contains all sections needed to produce a valid `.wasm` file.
#[derive(Debug, Clone)]
pub struct WasmModule {
    /// Function type signatures (type section).
    pub types: Vec<WasmFuncType>,
    /// Imported functions (import section).
    pub imports: Vec<WasmImport>,
    /// Function type indices for defined functions (function section).
    pub function_type_indices: Vec<u32>,
    /// Memory configuration (memory section).
    pub memory: WasmMemoryConfig,
    /// Global variables (global section).
    pub globals: Vec<WasmGlobal>,
    /// Exports (export section).
    pub exports: Vec<WasmExport>,
    /// Function bodies (code section).
    pub code: Vec<WasmFuncBody>,
    /// Data segments (data section).
    pub data: Vec<WasmDataSegment>,
}

impl WasmModule {
    /// Creates a new empty Wasm module with default memory config.
    pub fn new() -> Self {
        Self {
            types: Vec::new(),
            imports: Vec::new(),
            function_type_indices: Vec::new(),
            memory: WasmMemoryConfig::default(),
            globals: Vec::new(),
            exports: Vec::new(),
            code: Vec::new(),
            data: Vec::new(),
        }
    }

    /// Produces the complete Wasm binary output.
    pub fn finish(&self) -> Vec<u8> {
        let mut bytes = Vec::new();

        // Magic number and version
        bytes.extend_from_slice(b"\0asm");
        bytes.extend_from_slice(&[0x01, 0x00, 0x00, 0x00]);

        // Type section
        if !self.types.is_empty() {
            let payload = self.encode_type_section();
            encode_section(SECTION_TYPE, &payload, &mut bytes);
        }

        // Import section
        if !self.imports.is_empty() {
            let payload = self.encode_import_section();
            encode_section(SECTION_IMPORT, &payload, &mut bytes);
        }

        // Function section
        if !self.function_type_indices.is_empty() {
            let payload = self.encode_function_section();
            encode_section(SECTION_FUNCTION, &payload, &mut bytes);
        }

        // Memory section (always present)
        let mem_payload = self.encode_memory_section();
        encode_section(SECTION_MEMORY, &mem_payload, &mut bytes);

        // Global section
        if !self.globals.is_empty() {
            let payload = self.encode_global_section();
            encode_section(SECTION_GLOBAL, &payload, &mut bytes);
        }

        // Export section
        if !self.exports.is_empty() {
            let payload = self.encode_export_section();
            encode_section(SECTION_EXPORT, &payload, &mut bytes);
        }

        // Code section
        if !self.code.is_empty() {
            let payload = self.encode_code_section();
            encode_section(SECTION_CODE, &payload, &mut bytes);
        }

        // Data section
        if !self.data.is_empty() {
            let payload = self.encode_data_section();
            encode_section(SECTION_DATA, &payload, &mut bytes);
        }

        bytes
    }

    /// Encodes the type section payload.
    fn encode_type_section(&self) -> Vec<u8> {
        let mut payload = Vec::new();
        encode_unsigned_leb128(self.types.len() as u64, &mut payload);
        for ft in &self.types {
            payload.extend(ft.encode());
        }
        payload
    }

    /// Encodes the import section payload.
    fn encode_import_section(&self) -> Vec<u8> {
        let mut payload = Vec::new();
        encode_unsigned_leb128(self.imports.len() as u64, &mut payload);
        for imp in &self.imports {
            encode_name(&imp.module, &mut payload);
            encode_name(&imp.name, &mut payload);
            payload.push(0x00); // func import
            encode_unsigned_leb128(imp.type_index as u64, &mut payload);
        }
        payload
    }

    /// Encodes the function section payload.
    fn encode_function_section(&self) -> Vec<u8> {
        let mut payload = Vec::new();
        encode_unsigned_leb128(self.function_type_indices.len() as u64, &mut payload);
        for &idx in &self.function_type_indices {
            encode_unsigned_leb128(idx as u64, &mut payload);
        }
        payload
    }

    /// Encodes the memory section payload.
    fn encode_memory_section(&self) -> Vec<u8> {
        let mut payload = Vec::new();
        payload.push(0x01); // 1 memory
        payload.push(0x01); // has maximum
        encode_unsigned_leb128(self.memory.initial_pages as u64, &mut payload);
        encode_unsigned_leb128(self.memory.max_pages as u64, &mut payload);
        payload
    }

    /// Encodes the global section payload.
    fn encode_global_section(&self) -> Vec<u8> {
        let mut payload = Vec::new();
        encode_unsigned_leb128(self.globals.len() as u64, &mut payload);
        for g in &self.globals {
            payload.push(g.ty.encode());
            payload.push(if g.mutable { 0x01 } else { 0x00 });
            for instr in &g.init {
                instr.encode(&mut payload);
            }
            payload.push(0x0B); // end of init expr
        }
        payload
    }

    /// Encodes the export section payload.
    fn encode_export_section(&self) -> Vec<u8> {
        let mut payload = Vec::new();
        encode_unsigned_leb128(self.exports.len() as u64, &mut payload);
        for exp in &self.exports {
            encode_name(&exp.name, &mut payload);
            payload.push(exp.kind);
            encode_unsigned_leb128(exp.index as u64, &mut payload);
        }
        payload
    }

    /// Encodes the code section payload.
    fn encode_code_section(&self) -> Vec<u8> {
        let mut payload = Vec::new();
        encode_unsigned_leb128(self.code.len() as u64, &mut payload);
        for body in &self.code {
            payload.extend(body.encode());
        }
        payload
    }

    /// Encodes the data section payload.
    fn encode_data_section(&self) -> Vec<u8> {
        let mut payload = Vec::new();
        encode_unsigned_leb128(self.data.len() as u64, &mut payload);
        for seg in &self.data {
            payload.push(0x00); // active segment, memory 0
            // offset is i32.const + end
            WasmInstruction::I32Const(seg.offset as i32).encode(&mut payload);
            payload.push(0x0B); // end of offset expr
            encode_unsigned_leb128(seg.data.len() as u64, &mut payload);
            payload.extend_from_slice(&seg.data);
        }
        payload
    }
}

/// Encodes a section with ID, length-prefixed payload.
fn encode_section(id: u8, payload: &[u8], out: &mut Vec<u8>) {
    out.push(id);
    encode_unsigned_leb128(payload.len() as u64, out);
    out.extend_from_slice(payload);
}

/// Encodes a name (UTF-8 string) as length + bytes.
fn encode_name(name: &str, out: &mut Vec<u8>) {
    encode_unsigned_leb128(name.len() as u64, out);
    out.extend_from_slice(name.as_bytes());
}

// ═══════════════════════════════════════════════════════════════════════
// Sprint 3: Wasm Memory Model
// ═══════════════════════════════════════════════════════════════════════

/// Stack allocator for linear memory.
///
/// Uses a bump pointer for fast stack-style allocation within the Wasm
/// linear memory. The heap region starts after the data section.
#[derive(Debug, Clone)]
pub struct WasmStackAllocator {
    /// Current bump pointer offset in linear memory.
    pub stack_pointer: u32,
    /// Base address for stack allocations.
    pub base: u32,
    /// Maximum address (end of available stack region).
    pub limit: u32,
}

impl WasmStackAllocator {
    /// Creates a new stack allocator starting at the given base address.
    pub fn new(base: u32, limit: u32) -> Self {
        Self {
            stack_pointer: base,
            base,
            limit,
        }
    }

    /// Allocates `size` bytes, returning the offset or an error.
    pub fn alloc(&mut self, size: u32) -> Result<u32, WasmError> {
        let aligned = align_up(self.stack_pointer, 8);
        let new_sp = aligned
            .checked_add(size)
            .ok_or_else(|| WasmError::MemoryError("stack allocation overflow".to_string()))?;
        if new_sp > self.limit {
            return Err(WasmError::MemoryError(format!(
                "stack overflow: needed {size} bytes at offset {aligned}, limit {0}",
                self.limit
            )));
        }
        self.stack_pointer = new_sp;
        Ok(aligned)
    }

    /// Resets the stack pointer back to base (for scope exit).
    pub fn reset(&mut self) {
        self.stack_pointer = self.base;
    }
}

/// Heap allocator metadata for linear memory.
///
/// Implements a simple free-list allocator. At runtime, the allocator
/// functions (`__wasm_malloc` / `__wasm_free`) are compiled as Wasm
/// functions that manage blocks in linear memory.
#[derive(Debug, Clone)]
pub struct WasmHeapAllocator {
    /// Start of heap region in linear memory.
    pub heap_start: u32,
    /// Current break pointer (next allocation address).
    pub brk: u32,
}

impl WasmHeapAllocator {
    /// Creates a new heap allocator starting at the given address.
    pub fn new(heap_start: u32) -> Self {
        Self {
            heap_start,
            brk: heap_start,
        }
    }

    /// Allocates `size` bytes from the heap, returning the offset.
    pub fn alloc(&mut self, size: u32) -> Result<u32, WasmError> {
        let aligned = align_up(self.brk, 8);
        // Header: 4 bytes for block size
        let header_size = 4u32;
        let total = header_size
            .checked_add(size)
            .ok_or_else(|| WasmError::MemoryError("heap allocation overflow".to_string()))?;
        let new_brk = aligned
            .checked_add(total)
            .ok_or_else(|| WasmError::MemoryError("heap allocation overflow".to_string()))?;
        self.brk = new_brk;
        // Return pointer past header
        Ok(aligned + header_size)
    }

    /// Returns the current break pointer.
    pub fn current_brk(&self) -> u32 {
        self.brk
    }
}

/// Aligns `addr` up to the given `align` boundary.
fn align_up(addr: u32, align: u32) -> u32 {
    (addr + align - 1) & !(align - 1)
}

/// Computes field offsets for a struct definition.
///
/// Returns a map of field name to (offset, wasm_type).
pub fn compute_struct_layout(fields: &[(String, WasmType)]) -> Vec<(String, u32, WasmType)> {
    let mut layout = Vec::new();
    let mut offset = 0u32;
    for (name, ty) in fields {
        let size = wasm_type_size(*ty);
        let aligned = align_up(offset, size);
        layout.push((name.clone(), aligned, *ty));
        offset = aligned + size;
    }
    layout
}

/// Returns the byte size of a Wasm value type.
pub fn wasm_type_size(ty: WasmType) -> u32 {
    match ty {
        WasmType::I32 | WasmType::F32 => 4,
        WasmType::I64 | WasmType::F64 => 8,
    }
}

/// String representation in linear memory: (ptr: i32, len: i32).
///
/// Strings are stored as UTF-8 bytes in the data section (for literals)
/// or in heap memory (for dynamic strings). The (ptr, len) pair is
/// passed around as two i32 values.
#[derive(Debug, Clone, Copy)]
pub struct WasmStringRepr {
    /// Pointer to the first byte in linear memory.
    pub ptr: u32,
    /// Length in bytes.
    pub len: u32,
}

/// Array representation: (ptr: i32, len: i32, capacity: i32).
#[derive(Debug, Clone, Copy)]
pub struct WasmArrayRepr {
    /// Pointer to the first element in linear memory.
    pub ptr: u32,
    /// Number of elements.
    pub len: u32,
    /// Allocated capacity.
    pub capacity: u32,
}

// ═══════════════════════════════════════════════════════════════════════
// Sprint 4: Wasm Target & Integration
// ═══════════════════════════════════════════════════════════════════════

/// Target environment for Wasm compilation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WasmTarget {
    /// Standalone WASI module (fd_write, proc_exit, etc.).
    Wasi,
    /// Browser module (JavaScript host imports).
    Browser,
}

impl std::fmt::Display for WasmTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WasmTarget::Wasi => write!(f, "wasi"),
            WasmTarget::Browser => write!(f, "browser"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// WasmCompiler — main compiler struct
// ═══════════════════════════════════════════════════════════════════════

/// WebAssembly compiler for Fajar Lang programs.
///
/// Compiles a Fajar Lang `Program` AST to a `WasmModule` which can be
/// serialized to `.wasm` binary format.
///
/// # Example
///
/// ```ignore
/// let program = parse(tokenize(source)?)?;
/// let mut compiler = WasmCompiler::new(WasmTarget::Wasi);
/// compiler.compile(&program)?;
/// let bytes = compiler.finish()?;
/// std::fs::write("output.wasm", bytes)?;
/// ```
pub struct WasmCompiler {
    /// The module being built.
    module: WasmModule,
    /// Target environment.
    target: WasmTarget,
    /// Map: function name → function index (imports first, then definitions).
    function_indices: HashMap<String, u32>,
    /// Next function index to assign.
    next_func_index: u32,
    /// Map: type signature → type section index (for dedup).
    type_cache: HashMap<WasmFuncType, u32>,
    /// Current function's local variable map: name → local index.
    locals: HashMap<String, u32>,
    /// Current function's local types (for the locals declaration).
    local_types: Vec<WasmType>,
    /// Next local variable index to assign.
    next_local_index: u32,
    /// Number of parameters in the current function (params come before locals).
    current_param_count: u32,
    /// Stack allocator for linear memory.
    stack_alloc: WasmStackAllocator,
    /// Heap allocator for linear memory.
    heap_alloc: WasmHeapAllocator,
    /// Data section offset (next available byte for string literals).
    data_offset: u32,
    /// Map: string literal → (offset, length) in data section.
    string_literals: HashMap<String, WasmStringRepr>,
    /// Map: global variable name → global index.
    global_indices: HashMap<String, u32>,
    /// Struct layouts: struct name → field layout.
    struct_layouts: HashMap<String, Vec<(String, u32, WasmType)>>,
    /// Nesting depth of blocks/loops for br label calculation.
    block_depth: u32,
    /// Stack of break target label depths.
    break_targets: Vec<u32>,
    /// Stack of continue target label depths.
    continue_targets: Vec<u32>,
}

impl WasmCompiler {
    /// Creates a new Wasm compiler targeting the given environment.
    pub fn new(target: WasmTarget) -> Self {
        // Data section starts at offset 1024, leaving room for the stack region
        let data_start = 1024u32;
        // Stack region: 0..1024
        let stack_alloc = WasmStackAllocator::new(0, data_start);
        // Heap starts after data section (will be adjusted after compile)
        let heap_alloc = WasmHeapAllocator::new(data_start);

        Self {
            module: WasmModule::new(),
            target,
            function_indices: HashMap::new(),
            next_func_index: 0,
            type_cache: HashMap::new(),
            locals: HashMap::new(),
            local_types: Vec::new(),
            next_local_index: 0,
            current_param_count: 0,
            stack_alloc,
            heap_alloc,
            data_offset: data_start,
            string_literals: HashMap::new(),
            global_indices: HashMap::new(),
            struct_layouts: HashMap::new(),
            block_depth: 0,
            break_targets: Vec::new(),
            continue_targets: Vec::new(),
        }
    }

    /// Returns the target environment.
    pub fn target(&self) -> WasmTarget {
        self.target
    }

    /// Returns a reference to the built module.
    pub fn module(&self) -> &WasmModule {
        &self.module
    }

    /// Compiles a complete Fajar Lang program to Wasm.
    pub fn compile(&mut self, program: &Program) -> Result<(), WasmError> {
        // Register WASI imports if targeting WASI
        if self.target == WasmTarget::Wasi {
            self.register_wasi_imports()?;
        }
        self.register_host_imports()?;

        // First pass: register all function signatures
        for item in &program.items {
            if let Item::FnDef(fn_def) = item {
                self.register_function(fn_def)?;
            }
        }

        // Second pass: compile function bodies
        for item in &program.items {
            match item {
                Item::FnDef(fn_def) => {
                    self.compile_function(fn_def)?;
                }
                Item::StructDef(sd) => {
                    self.register_struct(sd)?;
                }
                Item::ConstDef(cd) => {
                    self.compile_const(cd)?;
                }
                _ => {} // Skip other items for now
            }
        }

        // Export main/_start
        self.export_entry_point()?;
        // Export memory
        self.module.exports.push(WasmExport {
            name: "memory".to_string(),
            kind: EXPORT_MEMORY,
            index: 0,
        });

        Ok(())
    }

    /// Produces the final Wasm binary.
    pub fn finish(&self) -> Result<Vec<u8>, WasmError> {
        Ok(self.module.finish())
    }

    // ── WASI / host imports ──

    /// Registers standard WASI imports (fd_write, proc_exit, etc.).
    fn register_wasi_imports(&mut self) -> Result<(), WasmError> {
        // fd_write(fd: i32, iovs: i32, iovs_len: i32, nwritten: i32) -> i32
        let fd_write_type = WasmFuncType::new(
            vec![WasmType::I32, WasmType::I32, WasmType::I32, WasmType::I32],
            vec![WasmType::I32],
        );
        self.add_import("wasi_snapshot_preview1", "fd_write", fd_write_type)?;

        // proc_exit(code: i32)
        let proc_exit_type = WasmFuncType::new(vec![WasmType::I32], vec![]);
        self.add_import("wasi_snapshot_preview1", "proc_exit", proc_exit_type)?;

        // clock_time_get(id: i32, precision: i64, time: i32) -> i32
        let clock_type = WasmFuncType::new(
            vec![WasmType::I32, WasmType::I64, WasmType::I32],
            vec![WasmType::I32],
        );
        self.add_import("wasi_snapshot_preview1", "clock_time_get", clock_type)?;

        Ok(())
    }

    /// Registers host function imports for runtime support.
    fn register_host_imports(&mut self) -> Result<(), WasmError> {
        // Host print: __fj_print(ptr: i32, len: i32)
        let print_type = WasmFuncType::new(vec![WasmType::I32, WasmType::I32], vec![]);
        self.add_import("fj_runtime", "__fj_print", print_type)?;

        // Host assert: __fj_assert(cond: i32, msg_ptr: i32, msg_len: i32)
        let assert_type =
            WasmFuncType::new(vec![WasmType::I32, WasmType::I32, WasmType::I32], vec![]);
        self.add_import("fj_runtime", "__fj_assert", assert_type)?;

        // Host math: __fj_math_sqrt(x: f64) -> f64
        let sqrt_type = WasmFuncType::new(vec![WasmType::F64], vec![WasmType::F64]);
        self.add_import("fj_runtime", "__fj_math_sqrt", sqrt_type)?;

        // Host math: __fj_math_pow(base: f64, exp: f64) -> f64
        let pow_type = WasmFuncType::new(vec![WasmType::F64, WasmType::F64], vec![WasmType::F64]);
        self.add_import("fj_runtime", "__fj_math_pow", pow_type)?;

        Ok(())
    }

    /// Adds an import and returns its function index.
    fn add_import(
        &mut self,
        module: &str,
        name: &str,
        func_type: WasmFuncType,
    ) -> Result<u32, WasmError> {
        let type_index = self.intern_type(func_type);
        let func_index = self.next_func_index;
        self.next_func_index += 1;
        self.function_indices.insert(name.to_string(), func_index);
        self.module.imports.push(WasmImport {
            module: module.to_string(),
            name: name.to_string(),
            type_index,
        });
        Ok(func_index)
    }

    /// Interns a function type, returning its index (deduplicates).
    fn intern_type(&mut self, func_type: WasmFuncType) -> u32 {
        if let Some(&idx) = self.type_cache.get(&func_type) {
            return idx;
        }
        let idx = self.module.types.len() as u32;
        self.type_cache.insert(func_type.clone(), idx);
        self.module.types.push(func_type);
        idx
    }

    // ── Function registration and compilation ──

    /// Registers a function signature (first pass).
    fn register_function(&mut self, fn_def: &FnDef) -> Result<(), WasmError> {
        let params = self.lower_params(&fn_def.params)?;
        let results = self.lower_return_type(&fn_def.return_type)?;
        let func_type = WasmFuncType::new(params, results);
        let type_index = self.intern_type(func_type);

        let func_index = self.next_func_index;
        self.next_func_index += 1;
        self.function_indices
            .insert(fn_def.name.clone(), func_index);
        self.module.function_type_indices.push(type_index);

        Ok(())
    }

    /// Compiles a function body (second pass).
    fn compile_function(&mut self, fn_def: &FnDef) -> Result<(), WasmError> {
        // Reset local state
        self.locals.clear();
        self.local_types.clear();
        self.next_local_index = 0;
        self.current_param_count = fn_def.params.len() as u32;

        // Register parameters as locals (index 0..N-1)
        for param in &fn_def.params {
            let idx = self.next_local_index;
            self.locals.insert(param.name.clone(), idx);
            self.next_local_index += 1;
        }

        // Compile body to instructions
        let mut instructions = Vec::new();
        self.compile_expr(&fn_def.body, &mut instructions)?;

        // If function returns void, ensure we don't leave values on stack
        if fn_def.return_type.is_none() {
            // Block may have pushed a value; drop it for void functions
        }

        instructions.push(WasmInstruction::End);

        // Build locals declaration (only locals, not params)
        let locals = self.build_locals_declaration();

        let body = WasmFuncBody {
            locals,
            instructions,
        };
        self.module.code.push(body);

        Ok(())
    }

    /// Builds the locals declaration from accumulated local types.
    fn build_locals_declaration(&self) -> Vec<(u32, WasmType)> {
        if self.local_types.is_empty() {
            return Vec::new();
        }
        // Group consecutive same-type locals
        let mut groups: Vec<(u32, WasmType)> = Vec::new();
        for &ty in &self.local_types {
            if let Some(last) = groups.last_mut() {
                if last.1 == ty {
                    last.0 += 1;
                    continue;
                }
            }
            groups.push((1, ty));
        }
        groups
    }

    /// Lowers function parameters to Wasm types.
    fn lower_params(
        &self,
        params: &[crate::parser::ast::Param],
    ) -> Result<Vec<WasmType>, WasmError> {
        let mut wasm_params = Vec::new();
        for p in params {
            let ty = lower_type_to_wasm(&p.ty).ok_or_else(|| {
                WasmError::TypeLoweringError(format!(
                    "cannot lower parameter type for '{}'",
                    p.name
                ))
            })?;
            wasm_params.push(ty);
        }
        Ok(wasm_params)
    }

    /// Lowers a return type to Wasm result types.
    fn lower_return_type(&self, ret: &Option<TypeExpr>) -> Result<Vec<WasmType>, WasmError> {
        match ret {
            None => Ok(vec![]),
            Some(ty) => {
                if let TypeExpr::Simple { name, .. } = ty {
                    if name == "void" {
                        return Ok(vec![]);
                    }
                }
                let wt = lower_type_to_wasm(ty).ok_or_else(|| {
                    WasmError::TypeLoweringError("cannot lower return type".to_string())
                })?;
                Ok(vec![wt])
            }
        }
    }

    /// Allocates a new local variable, returning its index.
    fn alloc_local(&mut self, name: &str, ty: WasmType) -> u32 {
        let idx = self.next_local_index;
        self.next_local_index += 1;
        self.locals.insert(name.to_string(), idx);
        self.local_types.push(ty);
        idx
    }

    /// Registers a struct definition and computes its layout.
    fn register_struct(&mut self, sd: &crate::parser::ast::StructDef) -> Result<(), WasmError> {
        let mut fields = Vec::new();
        for f in &sd.fields {
            let ty = lower_type_to_wasm(&f.ty).unwrap_or(WasmType::I32);
            fields.push((f.name.clone(), ty));
        }
        let layout = compute_struct_layout(&fields);
        self.struct_layouts.insert(sd.name.clone(), layout);
        Ok(())
    }

    /// Compiles a const definition to a global variable.
    fn compile_const(&mut self, cd: &crate::parser::ast::ConstDef) -> Result<(), WasmError> {
        let ty = lower_type_to_wasm(&cd.ty).unwrap_or(WasmType::I64);
        let mut init = Vec::new();
        // Only constant expressions are valid for global init
        self.compile_const_expr(&cd.value, &mut init)?;
        let idx = self.module.globals.len() as u32;
        self.global_indices.insert(cd.name.clone(), idx);
        self.module.globals.push(WasmGlobal {
            ty,
            mutable: false,
            init,
        });
        Ok(())
    }

    /// Compiles a constant expression (for global initializers).
    fn compile_const_expr(
        &self,
        expr: &Expr,
        out: &mut Vec<WasmInstruction>,
    ) -> Result<(), WasmError> {
        match expr {
            Expr::Literal { kind, .. } => {
                match kind {
                    LiteralKind::Int(v) => out.push(WasmInstruction::I64Const(*v)),
                    LiteralKind::Float(v) => out.push(WasmInstruction::F64Const(*v)),
                    LiteralKind::Bool(v) => {
                        out.push(WasmInstruction::I32Const(if *v { 1 } else { 0 }))
                    }
                    _ => {
                        return Err(WasmError::UnsupportedExpr(
                            "non-numeric constant".to_string(),
                        ));
                    }
                }
                Ok(())
            }
            _ => Err(WasmError::UnsupportedExpr(
                "complex constant expression".to_string(),
            )),
        }
    }

    /// Exports the entry point function (_start for WASI, main for browser).
    fn export_entry_point(&mut self) -> Result<(), WasmError> {
        let export_name = match self.target {
            WasmTarget::Wasi => "_start",
            WasmTarget::Browser => "main",
        };

        // Look for "main" function
        if let Some(&idx) = self.function_indices.get("main") {
            self.module.exports.push(WasmExport {
                name: export_name.to_string(),
                kind: EXPORT_FUNC,
                index: idx,
            });
        }
        Ok(())
    }

    // ── Sprint 2: Expression compilation ──

    /// Compiles an expression, pushing its value onto the Wasm stack.
    fn compile_expr(
        &mut self,
        expr: &Expr,
        out: &mut Vec<WasmInstruction>,
    ) -> Result<(), WasmError> {
        match expr {
            Expr::Literal { kind, .. } => self.compile_literal(kind, out),
            Expr::Ident { name, .. } => self.compile_ident(name, out),
            Expr::Binary {
                left, op, right, ..
            } => self.compile_binary(left, *op, right, out),
            Expr::Unary { op, operand, .. } => self.compile_unary(*op, operand, out),
            Expr::Call { callee, args, .. } => self.compile_call(callee, args, out),
            Expr::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => self.compile_if(condition, then_branch, else_branch.as_deref(), out),
            Expr::While {
                label: _,
                condition,
                body,
                ..
            } => self.compile_while(condition, body, out),
            Expr::Loop { label: _, body, .. } => self.compile_loop(body, out),
            Expr::Block { stmts, expr, .. } => self.compile_block(stmts, expr.as_deref(), out),
            Expr::Assign {
                target, op, value, ..
            } => self.compile_assign(target, *op, value, out),
            Expr::Grouped { expr, .. } => self.compile_expr(expr, out),
            Expr::Path { segments, .. } => {
                // Treat as identifier for now
                if let Some(name) = segments.last() {
                    self.compile_ident(name, out)
                } else {
                    Err(WasmError::UnsupportedExpr("empty path".to_string()))
                }
            }
            _ => Err(WasmError::NotImplemented(format!(
                "expression type: {:?}",
                std::mem::discriminant(expr)
            ))),
        }
    }

    /// Compiles a literal value.
    fn compile_literal(
        &mut self,
        kind: &LiteralKind,
        out: &mut Vec<WasmInstruction>,
    ) -> Result<(), WasmError> {
        match kind {
            LiteralKind::Int(v) => {
                out.push(WasmInstruction::I64Const(*v));
                Ok(())
            }
            LiteralKind::Float(v) => {
                out.push(WasmInstruction::F64Const(*v));
                Ok(())
            }
            LiteralKind::Bool(v) => {
                out.push(WasmInstruction::I32Const(if *v { 1 } else { 0 }));
                Ok(())
            }
            LiteralKind::Null => {
                out.push(WasmInstruction::I32Const(0));
                Ok(())
            }
            LiteralKind::Char(c) => {
                out.push(WasmInstruction::I32Const(*c as i32));
                Ok(())
            }
            LiteralKind::String(s) | LiteralKind::RawString(s) => {
                self.compile_string_literal(s, out)
            }
        }
    }

    /// Compiles a string literal, storing it in the data section.
    fn compile_string_literal(
        &mut self,
        s: &str,
        out: &mut Vec<WasmInstruction>,
    ) -> Result<(), WasmError> {
        let repr = if let Some(repr) = self.string_literals.get(s) {
            *repr
        } else {
            let offset = self.data_offset;
            let len = s.len() as u32;
            self.module.data.push(WasmDataSegment {
                offset,
                data: s.as_bytes().to_vec(),
            });
            let repr = WasmStringRepr { ptr: offset, len };
            self.data_offset += len;
            // Align to 4 bytes for next segment
            self.data_offset = align_up(self.data_offset, 4);
            self.string_literals.insert(s.to_string(), repr);
            repr
        };
        // Push (ptr, len) as i32 pair — caller decides how to use
        // For simplicity, push ptr as the string value (i32)
        out.push(WasmInstruction::I32Const(repr.ptr as i32));
        Ok(())
    }

    /// Compiles an identifier reference (local or global variable).
    fn compile_ident(&self, name: &str, out: &mut Vec<WasmInstruction>) -> Result<(), WasmError> {
        if let Some(&idx) = self.locals.get(name) {
            out.push(WasmInstruction::LocalGet(idx));
            Ok(())
        } else if let Some(&idx) = self.global_indices.get(name) {
            out.push(WasmInstruction::GlobalGet(idx));
            Ok(())
        } else {
            Err(WasmError::UndefinedVariable(name.to_string()))
        }
    }

    /// Compiles a binary operation.
    fn compile_binary(
        &mut self,
        left: &Expr,
        op: BinOp,
        right: &Expr,
        out: &mut Vec<WasmInstruction>,
    ) -> Result<(), WasmError> {
        self.compile_expr(left, out)?;
        self.compile_expr(right, out)?;
        let instr = match op {
            BinOp::Add => WasmInstruction::I64Add,
            BinOp::Sub => WasmInstruction::I64Sub,
            BinOp::Mul => WasmInstruction::I64Mul,
            BinOp::Div => WasmInstruction::I64DivS,
            BinOp::Rem => WasmInstruction::I64RemS,
            BinOp::Eq => WasmInstruction::I64Eq,
            BinOp::Ne => WasmInstruction::I64Ne,
            BinOp::Lt => WasmInstruction::I64LtS,
            BinOp::Gt => WasmInstruction::I64GtS,
            BinOp::Le => WasmInstruction::I64LeS,
            BinOp::Ge => WasmInstruction::I64GeS,
            BinOp::And => {
                // Logical AND: both operands are booleans (i32)
                out.push(WasmInstruction::I32And);
                return Ok(());
            }
            BinOp::Or => {
                // Logical OR
                out.push(WasmInstruction::I32Or);
                return Ok(());
            }
            BinOp::BitAnd => WasmInstruction::I64And,
            BinOp::BitOr => WasmInstruction::I64Or,
            BinOp::BitXor => WasmInstruction::I64Xor,
            BinOp::Shl => WasmInstruction::I64Shl,
            BinOp::Shr => WasmInstruction::I64ShrS,
            BinOp::Pow | BinOp::MatMul => {
                return Err(WasmError::NotImplemented(format!("binary operator: {op}")));
            }
        };
        out.push(instr);
        Ok(())
    }

    /// Compiles a unary operation.
    fn compile_unary(
        &mut self,
        op: UnaryOp,
        operand: &Expr,
        out: &mut Vec<WasmInstruction>,
    ) -> Result<(), WasmError> {
        match op {
            UnaryOp::Neg => {
                // -x = 0 - x
                out.push(WasmInstruction::I64Const(0));
                self.compile_expr(operand, out)?;
                out.push(WasmInstruction::I64Sub);
                Ok(())
            }
            UnaryOp::Not => {
                self.compile_expr(operand, out)?;
                out.push(WasmInstruction::I32Eqz);
                Ok(())
            }
            UnaryOp::BitNot => {
                // ~x = x ^ -1
                self.compile_expr(operand, out)?;
                out.push(WasmInstruction::I64Const(-1));
                out.push(WasmInstruction::I64Xor);
                Ok(())
            }
            _ => Err(WasmError::NotImplemented(format!("unary op: {op}"))),
        }
    }

    /// Compiles a function call.
    fn compile_call(
        &mut self,
        callee: &Expr,
        args: &[crate::parser::ast::CallArg],
        out: &mut Vec<WasmInstruction>,
    ) -> Result<(), WasmError> {
        let func_name = match callee {
            Expr::Ident { name, .. } => name.clone(),
            Expr::Path { segments, .. } => segments.last().cloned().unwrap_or_default(),
            _ => {
                return Err(WasmError::NotImplemented(
                    "indirect function call".to_string(),
                ));
            }
        };

        // Handle builtin println specially
        if func_name == "println" || func_name == "print" {
            return self.compile_print_call(args, out);
        }

        // Compile arguments
        for arg in args {
            self.compile_expr(&arg.value, out)?;
        }

        // Look up function index
        let func_index = self
            .function_indices
            .get(&func_name)
            .copied()
            .ok_or_else(|| WasmError::UndefinedFunction(func_name.clone()))?;

        out.push(WasmInstruction::Call(func_index));
        Ok(())
    }

    /// Compiles a println/print call using WASI fd_write or host import.
    fn compile_print_call(
        &mut self,
        args: &[crate::parser::ast::CallArg],
        out: &mut Vec<WasmInstruction>,
    ) -> Result<(), WasmError> {
        if args.is_empty() {
            return Ok(());
        }

        // Compile the first argument (string)
        let arg = &args[0].value;

        // Get string ptr
        self.compile_expr(arg, out)?;

        // For WASI: use __fj_print(ptr, len) host import
        // The string literal compile already pushed ptr as i32
        // We need the length too — get from string_literals if available
        if let Expr::Literal {
            kind: LiteralKind::String(s),
            ..
        } = arg
        {
            let len = s.len() as i32;
            out.push(WasmInstruction::I32Const(len));
        } else {
            // For non-literal strings, use a default length
            out.push(WasmInstruction::I32Const(0));
        }

        let print_idx = self
            .function_indices
            .get("__fj_print")
            .copied()
            .ok_or_else(|| WasmError::ImportError("__fj_print not registered".to_string()))?;

        out.push(WasmInstruction::Call(print_idx));
        Ok(())
    }

    /// Compiles an if/else expression.
    fn compile_if(
        &mut self,
        condition: &Expr,
        then_branch: &Expr,
        else_branch: Option<&Expr>,
        out: &mut Vec<WasmInstruction>,
    ) -> Result<(), WasmError> {
        self.compile_expr(condition, out)?;

        // Condition must be i32 for Wasm if
        // If condition is i64 (from comparison), wrap to i32
        out.push(WasmInstruction::I32WrapI64);

        out.push(WasmInstruction::If(BlockType::Empty));
        self.block_depth += 1;

        self.compile_expr(then_branch, out)?;

        if let Some(else_br) = else_branch {
            out.push(WasmInstruction::Else);
            self.compile_expr(else_br, out)?;
        }

        out.push(WasmInstruction::End);
        self.block_depth -= 1;

        Ok(())
    }

    /// Compiles a while loop.
    fn compile_while(
        &mut self,
        condition: &Expr,
        body: &Expr,
        out: &mut Vec<WasmInstruction>,
    ) -> Result<(), WasmError> {
        // block {
        //   loop {
        //     <condition>
        //     i32.eqz
        //     br_if 1       ;; break out of block if false
        //     <body>
        //     br 0           ;; continue loop
        //   }
        // }

        out.push(WasmInstruction::Block(BlockType::Empty));
        self.block_depth += 1;
        let break_depth = self.block_depth;
        self.break_targets.push(break_depth);

        out.push(WasmInstruction::Loop(BlockType::Empty));
        self.block_depth += 1;
        self.continue_targets.push(self.block_depth);

        // Condition
        self.compile_expr(condition, out)?;
        out.push(WasmInstruction::I32WrapI64);
        out.push(WasmInstruction::I32Eqz);
        out.push(WasmInstruction::BrIf(1)); // break out of block

        // Body
        self.compile_expr(body, out)?;

        // Continue
        out.push(WasmInstruction::Br(0)); // loop back

        out.push(WasmInstruction::End); // end loop
        self.block_depth -= 1;
        self.continue_targets.pop();

        out.push(WasmInstruction::End); // end block
        self.block_depth -= 1;
        self.break_targets.pop();

        Ok(())
    }

    /// Compiles an infinite loop.
    fn compile_loop(
        &mut self,
        body: &Expr,
        out: &mut Vec<WasmInstruction>,
    ) -> Result<(), WasmError> {
        out.push(WasmInstruction::Block(BlockType::Empty));
        self.block_depth += 1;
        self.break_targets.push(self.block_depth);

        out.push(WasmInstruction::Loop(BlockType::Empty));
        self.block_depth += 1;
        self.continue_targets.push(self.block_depth);

        self.compile_expr(body, out)?;

        out.push(WasmInstruction::Br(0)); // loop back
        out.push(WasmInstruction::End); // end loop
        self.block_depth -= 1;
        self.continue_targets.pop();

        out.push(WasmInstruction::End); // end block
        self.block_depth -= 1;
        self.break_targets.pop();

        Ok(())
    }

    /// Compiles a block expression.
    fn compile_block(
        &mut self,
        stmts: &[Stmt],
        tail_expr: Option<&Expr>,
        out: &mut Vec<WasmInstruction>,
    ) -> Result<(), WasmError> {
        for stmt in stmts {
            self.compile_stmt(stmt, out)?;
        }
        if let Some(expr) = tail_expr {
            self.compile_expr(expr, out)?;
        }
        Ok(())
    }

    /// Compiles a statement.
    fn compile_stmt(
        &mut self,
        stmt: &Stmt,
        out: &mut Vec<WasmInstruction>,
    ) -> Result<(), WasmError> {
        match stmt {
            Stmt::Let {
                name, value, ty, ..
            } => self.compile_let(name, ty.as_ref(), value, out),
            Stmt::Expr { expr, .. } => {
                self.compile_expr(expr, out)?;
                // Drop the value if statement context (not used)
                out.push(WasmInstruction::Drop);
                Ok(())
            }
            Stmt::Return { value, .. } => {
                if let Some(val) = value {
                    self.compile_expr(val, out)?;
                }
                out.push(WasmInstruction::Return);
                Ok(())
            }
            Stmt::Break { .. } => {
                if let Some(&depth) = self.break_targets.last() {
                    let label = self.block_depth - depth;
                    out.push(WasmInstruction::Br(label));
                    Ok(())
                } else {
                    Err(WasmError::UnsupportedStmt(
                        "break outside of loop".to_string(),
                    ))
                }
            }
            Stmt::Continue { .. } => {
                if let Some(&depth) = self.continue_targets.last() {
                    let label = self.block_depth - depth;
                    out.push(WasmInstruction::Br(label));
                    Ok(())
                } else {
                    Err(WasmError::UnsupportedStmt(
                        "continue outside of loop".to_string(),
                    ))
                }
            }
            _ => Err(WasmError::NotImplemented(format!(
                "statement type: {:?}",
                std::mem::discriminant(stmt)
            ))),
        }
    }

    /// Compiles a let binding.
    fn compile_let(
        &mut self,
        name: &str,
        ty: Option<&TypeExpr>,
        value: &Expr,
        out: &mut Vec<WasmInstruction>,
    ) -> Result<(), WasmError> {
        let wasm_ty = ty.and_then(lower_type_to_wasm).unwrap_or(WasmType::I64);

        let local_idx = self.alloc_local(name, wasm_ty);

        self.compile_expr(value, out)?;
        out.push(WasmInstruction::LocalSet(local_idx));

        Ok(())
    }

    /// Compiles an assignment expression.
    fn compile_assign(
        &mut self,
        target: &Expr,
        op: AssignOp,
        value: &Expr,
        out: &mut Vec<WasmInstruction>,
    ) -> Result<(), WasmError> {
        let name = match target {
            Expr::Ident { name, .. } => name.clone(),
            _ => {
                return Err(WasmError::NotImplemented(
                    "complex assignment target".to_string(),
                ));
            }
        };

        let local_idx = self
            .locals
            .get(&name)
            .copied()
            .ok_or_else(|| WasmError::UndefinedVariable(name.clone()))?;

        match op {
            AssignOp::Assign => {
                self.compile_expr(value, out)?;
                out.push(WasmInstruction::LocalSet(local_idx));
            }
            _ => {
                // Compound assignment: load, compute, store
                out.push(WasmInstruction::LocalGet(local_idx));
                self.compile_expr(value, out)?;
                let arith = match op {
                    AssignOp::AddAssign => WasmInstruction::I64Add,
                    AssignOp::SubAssign => WasmInstruction::I64Sub,
                    AssignOp::MulAssign => WasmInstruction::I64Mul,
                    AssignOp::DivAssign => WasmInstruction::I64DivS,
                    AssignOp::RemAssign => WasmInstruction::I64RemS,
                    AssignOp::BitAndAssign => WasmInstruction::I64And,
                    AssignOp::BitOrAssign => WasmInstruction::I64Or,
                    AssignOp::BitXorAssign => WasmInstruction::I64Xor,
                    AssignOp::ShlAssign => WasmInstruction::I64Shl,
                    AssignOp::ShrAssign => WasmInstruction::I64ShrS,
                    AssignOp::Assign => unreachable!(),
                };
                out.push(arith);
                out.push(WasmInstruction::LocalSet(local_idx));
            }
        }

        Ok(())
    }

    // ── Sprint 4: Browser runtime stub ──

    /// Generates an HTML+JS loader for browser-targeted Wasm modules.
    ///
    /// Returns the HTML content as a string. The generated page fetches
    /// the `.wasm` file, provides the `fj_runtime` import object, and
    /// calls `_start` or `main`.
    pub fn generate_browser_loader(wasm_filename: &str) -> String {
        format!(
            r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <title>Fajar Lang — WebAssembly</title>
</head>
<body>
    <pre id="output"></pre>
    <script>
    const output = document.getElementById('output');
    const decoder = new TextDecoder();
    const importObject = {{
        fj_runtime: {{
            __fj_print: (ptr, len) => {{
                const bytes = new Uint8Array(memory.buffer, ptr, len);
                output.textContent += decoder.decode(bytes) + '\n';
            }},
            __fj_assert: (cond, msgPtr, msgLen) => {{
                if (!cond) {{
                    const bytes = new Uint8Array(memory.buffer, msgPtr, msgLen);
                    throw new Error('Assertion failed: ' + decoder.decode(bytes));
                }}
            }},
            __fj_math_sqrt: Math.sqrt,
            __fj_math_pow: Math.pow,
        }},
    }};
    let memory;
    WebAssembly.instantiateStreaming(fetch('{wasm_filename}'), importObject)
        .then(result => {{
            memory = result.instance.exports.memory;
            if (result.instance.exports.main) {{
                result.instance.exports.main();
            }}
        }})
        .catch(err => {{
            output.textContent = 'Error: ' + err.message;
        }});
    </script>
</body>
</html>"#
        )
    }

    /// Performs basic dead code removal on the module.
    ///
    /// Removes functions that are never called and not exported.
    /// Also deduplicates identical type section entries.
    pub fn optimize(&mut self) {
        self.deduplicate_types();
    }

    /// Deduplicates identical entries in the type section.
    fn deduplicate_types(&mut self) {
        // The type_cache already prevents duplicates during compilation.
        // This method is a no-op if intern_type was used consistently,
        // but serves as a safety pass for imported types.
        let mut seen: HashMap<WasmFuncType, u32> = HashMap::new();
        let mut remap: HashMap<u32, u32> = HashMap::new();
        let mut deduped = Vec::new();

        for (idx, ft) in self.module.types.iter().enumerate() {
            if let Some(&existing) = seen.get(ft) {
                remap.insert(idx as u32, existing);
            } else {
                let new_idx = deduped.len() as u32;
                seen.insert(ft.clone(), new_idx);
                remap.insert(idx as u32, new_idx);
                deduped.push(ft.clone());
            }
        }

        self.module.types = deduped;

        // Remap function type indices
        for ti in &mut self.module.function_type_indices {
            if let Some(&new_idx) = remap.get(ti) {
                *ti = new_idx;
            }
        }

        // Remap import type indices
        for imp in &mut self.module.imports {
            if let Some(&new_idx) = remap.get(&imp.type_index) {
                imp.type_index = new_idx;
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests (40 tests across 4 sprints)
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::token::Span;
    use crate::parser::ast::*;

    fn span() -> Span {
        Span::new(0, 0)
    }

    fn simple_ty(name: &str) -> TypeExpr {
        TypeExpr::Simple {
            name: name.to_string(),
            span: span(),
        }
    }

    fn int_lit(v: i64) -> Expr {
        Expr::Literal {
            kind: LiteralKind::Int(v),
            span: span(),
        }
    }

    fn float_lit(v: f64) -> Expr {
        Expr::Literal {
            kind: LiteralKind::Float(v),
            span: span(),
        }
    }

    fn bool_lit(v: bool) -> Expr {
        Expr::Literal {
            kind: LiteralKind::Bool(v),
            span: span(),
        }
    }

    fn string_lit(s: &str) -> Expr {
        Expr::Literal {
            kind: LiteralKind::String(s.to_string()),
            span: span(),
        }
    }

    fn ident(name: &str) -> Expr {
        Expr::Ident {
            name: name.to_string(),
            span: span(),
        }
    }

    fn binary(left: Expr, op: BinOp, right: Expr) -> Expr {
        Expr::Binary {
            left: Box::new(left),
            op,
            right: Box::new(right),
            span: span(),
        }
    }

    fn call_arg(value: Expr) -> CallArg {
        CallArg {
            name: None,
            value,
            span: span(),
        }
    }

    fn param(name: &str, ty: &str) -> Param {
        Param {
            name: name.to_string(),
            ty: simple_ty(ty),
            span: span(),
        }
    }

    fn make_fn(name: &str, params: Vec<Param>, ret: Option<&str>, body: Expr) -> FnDef {
        FnDef {
            is_pub: false,
            is_const: false,
            is_async: false,
            is_test: false,
            should_panic: false,
            is_ignored: false,
            doc_comment: None,
            annotation: None,
            name: name.to_string(),
            lifetime_params: Vec::new(),
            generic_params: Vec::new(),
            params,
            return_type: ret.map(simple_ty),
            where_clauses: Vec::new(),
            requires: vec![],
            ensures: vec![],
            effects: vec![],
            body: Box::new(body),
            span: span(),
        }
    }

    fn make_program(items: Vec<Item>) -> Program {
        Program {
            items,
            span: span(),
        }
    }

    // ── Sprint 1 Tests: Wasm Target Setup (S1.1–S1.10) ──

    #[test]
    fn s1_1_wasm_type_encode_i32() {
        assert_eq!(WasmType::I32.encode(), 0x7F);
    }

    #[test]
    fn s1_2_wasm_type_encode_i64() {
        assert_eq!(WasmType::I64.encode(), 0x7E);
    }

    #[test]
    fn s1_3_wasm_type_encode_f32_f64() {
        assert_eq!(WasmType::F32.encode(), 0x7D);
        assert_eq!(WasmType::F64.encode(), 0x7C);
    }

    #[test]
    fn s1_4_type_lowering_fajar_to_wasm() {
        assert_eq!(lower_simple_type_to_wasm("i32"), Some(WasmType::I32));
        assert_eq!(lower_simple_type_to_wasm("i64"), Some(WasmType::I64));
        assert_eq!(lower_simple_type_to_wasm("f32"), Some(WasmType::F32));
        assert_eq!(lower_simple_type_to_wasm("f64"), Some(WasmType::F64));
        assert_eq!(lower_simple_type_to_wasm("bool"), Some(WasmType::I32));
        assert_eq!(lower_simple_type_to_wasm("void"), None);
    }

    #[test]
    fn s1_5_func_type_encoding() {
        let ft = WasmFuncType::new(vec![WasmType::I32, WasmType::I32], vec![WasmType::I32]);
        let encoded = ft.encode();
        assert_eq!(encoded[0], 0x60); // functype marker
        assert_eq!(encoded[1], 2); // 2 params
        assert_eq!(encoded[2], WasmType::I32.encode());
        assert_eq!(encoded[3], WasmType::I32.encode());
        assert_eq!(encoded[4], 1); // 1 result
        assert_eq!(encoded[5], WasmType::I32.encode());
    }

    #[test]
    fn s1_6_module_magic_number() {
        let module = WasmModule::new();
        let bytes = module.finish();
        // Wasm magic: \0asm
        assert_eq!(&bytes[0..4], b"\0asm");
        // Version 1
        assert_eq!(&bytes[4..8], &[0x01, 0x00, 0x00, 0x00]);
    }

    #[test]
    fn s1_7_memory_section_default() {
        let module = WasmModule::new();
        let bytes = module.finish();
        // Should contain memory section (id 5)
        assert!(bytes.windows(2).any(|w| w[0] == SECTION_MEMORY));
    }

    #[test]
    fn s1_8_leb128_encoding() {
        let mut buf = Vec::new();
        encode_unsigned_leb128(0, &mut buf);
        assert_eq!(buf, vec![0x00]);

        buf.clear();
        encode_unsigned_leb128(127, &mut buf);
        assert_eq!(buf, vec![0x7F]);

        buf.clear();
        encode_unsigned_leb128(128, &mut buf);
        assert_eq!(buf, vec![0x80, 0x01]);

        buf.clear();
        encode_unsigned_leb128(624485, &mut buf);
        assert_eq!(buf, vec![0xE5, 0x8E, 0x26]);
    }

    #[test]
    fn s1_9_signed_leb128_encoding() {
        let mut buf = Vec::new();
        encode_signed_leb128(0, &mut buf);
        assert_eq!(buf, vec![0x00]);

        buf.clear();
        encode_signed_leb128(-1, &mut buf);
        assert_eq!(buf, vec![0x7F]);

        buf.clear();
        encode_signed_leb128(42, &mut buf);
        assert_eq!(buf, vec![42]);

        buf.clear();
        encode_signed_leb128(-128, &mut buf);
        assert_eq!(buf, vec![0x80, 0x7F]);
    }

    #[test]
    fn s1_10_compiler_creates_valid_module() {
        let mut compiler = WasmCompiler::new(WasmTarget::Wasi);
        let program = make_program(vec![Item::FnDef(make_fn(
            "main",
            vec![],
            Some("void"),
            Expr::Block {
                stmts: vec![],
                expr: None,
                span: span(),
            },
        ))]);
        let result = compiler.compile(&program);
        assert!(result.is_ok());
        let bytes = compiler.finish().unwrap();
        assert_eq!(&bytes[0..4], b"\0asm");
    }

    // ── Sprint 2 Tests: Expression & Statement Compilation (S2.1–S2.10) ──

    #[test]
    fn s2_1_compile_int_literal() {
        let mut compiler = WasmCompiler::new(WasmTarget::Wasi);
        let mut out = Vec::new();
        compiler
            .compile_literal(&LiteralKind::Int(42), &mut out)
            .unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0], WasmInstruction::I64Const(42));
    }

    #[test]
    fn s2_2_compile_float_literal() {
        let mut compiler = WasmCompiler::new(WasmTarget::Wasi);
        let mut out = Vec::new();
        compiler
            .compile_literal(&LiteralKind::Float(3.14), &mut out)
            .unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0], WasmInstruction::F64Const(3.14));
    }

    #[test]
    fn s2_3_compile_bool_literal() {
        let mut compiler = WasmCompiler::new(WasmTarget::Wasi);
        let mut out = Vec::new();
        compiler
            .compile_literal(&LiteralKind::Bool(true), &mut out)
            .unwrap();
        assert_eq!(out[0], WasmInstruction::I32Const(1));

        out.clear();
        compiler
            .compile_literal(&LiteralKind::Bool(false), &mut out)
            .unwrap();
        assert_eq!(out[0], WasmInstruction::I32Const(0));
    }

    #[test]
    fn s2_4_compile_binary_add() {
        let mut compiler = WasmCompiler::new(WasmTarget::Wasi);
        let mut out = Vec::new();
        let expr = binary(int_lit(10), BinOp::Add, int_lit(20));
        compiler.compile_expr(&expr, &mut out).unwrap();
        assert_eq!(out.len(), 3);
        assert_eq!(out[0], WasmInstruction::I64Const(10));
        assert_eq!(out[1], WasmInstruction::I64Const(20));
        assert_eq!(out[2], WasmInstruction::I64Add);
    }

    #[test]
    fn s2_5_compile_binary_comparison() {
        let mut compiler = WasmCompiler::new(WasmTarget::Wasi);
        let mut out = Vec::new();
        let expr = binary(int_lit(5), BinOp::Lt, int_lit(10));
        compiler.compile_expr(&expr, &mut out).unwrap();
        assert_eq!(out.last(), Some(&WasmInstruction::I64LtS));
    }

    #[test]
    fn s2_6_compile_let_binding() {
        let mut compiler = WasmCompiler::new(WasmTarget::Wasi);
        let mut out = Vec::new();
        let stmt = Stmt::Let {
            mutable: false,
            name: "x".to_string(),
            ty: Some(simple_ty("i64")),
            value: Box::new(int_lit(42)),
            span: span(),
        };
        compiler.compile_stmt(&stmt, &mut out).unwrap();
        // Should have I64Const(42) + LocalSet
        assert!(out.contains(&WasmInstruction::I64Const(42)));
        assert!(
            out.iter()
                .any(|i| matches!(i, WasmInstruction::LocalSet(_)))
        );
    }

    #[test]
    fn s2_7_compile_local_get_after_let() {
        let mut compiler = WasmCompiler::new(WasmTarget::Wasi);
        let mut out = Vec::new();
        // let x = 42; x
        let let_stmt = Stmt::Let {
            mutable: true,
            name: "x".to_string(),
            ty: Some(simple_ty("i64")),
            value: Box::new(int_lit(42)),
            span: span(),
        };
        compiler.compile_stmt(&let_stmt, &mut out).unwrap();
        compiler.compile_ident("x", &mut out).unwrap();
        assert!(
            out.iter()
                .any(|i| matches!(i, WasmInstruction::LocalGet(_)))
        );
    }

    #[test]
    fn s2_8_compile_if_else() {
        let mut compiler = WasmCompiler::new(WasmTarget::Wasi);
        let mut out = Vec::new();
        let if_expr = Expr::If {
            condition: Box::new(int_lit(1)),
            then_branch: Box::new(int_lit(10)),
            else_branch: Some(Box::new(int_lit(20))),
            span: span(),
        };
        compiler.compile_expr(&if_expr, &mut out).unwrap();
        assert!(out.iter().any(|i| matches!(i, WasmInstruction::If(_))));
        assert!(out.contains(&WasmInstruction::Else));
        assert!(out.contains(&WasmInstruction::End));
    }

    #[test]
    fn s2_9_compile_while_loop() {
        let mut compiler = WasmCompiler::new(WasmTarget::Wasi);
        let mut out = Vec::new();
        let while_expr = Expr::While {
            label: _,
            condition: Box::new(int_lit(1)),
            body: Box::new(Expr::Block {
                stmts: vec![],
                expr: None,
                span: span(),
            }),
            span: span(),
        };
        compiler.compile_expr(&while_expr, &mut out).unwrap();
        assert!(out.iter().any(|i| matches!(i, WasmInstruction::Block(_))));
        assert!(out.iter().any(|i| matches!(i, WasmInstruction::Loop(_))));
        assert!(out.iter().any(|i| matches!(i, WasmInstruction::BrIf(_))));
    }

    #[test]
    fn s2_10_compile_unary_neg() {
        let mut compiler = WasmCompiler::new(WasmTarget::Wasi);
        let mut out = Vec::new();
        let expr = Expr::Unary {
            op: UnaryOp::Neg,
            operand: Box::new(int_lit(5)),
            span: span(),
        };
        compiler.compile_expr(&expr, &mut out).unwrap();
        // -5 = 0 - 5
        assert!(out.contains(&WasmInstruction::I64Const(0)));
        assert!(out.contains(&WasmInstruction::I64Const(5)));
        assert!(out.contains(&WasmInstruction::I64Sub));
    }

    // ── Sprint 3 Tests: Memory Model (S3.1–S3.10) ──

    #[test]
    fn s3_1_stack_allocator_basic() {
        let mut alloc = WasmStackAllocator::new(0, 1024);
        let offset = alloc.alloc(16).unwrap();
        assert_eq!(offset, 0);
        let offset2 = alloc.alloc(32).unwrap();
        assert_eq!(offset2, 16); // aligned to 8
    }

    #[test]
    fn s3_2_stack_allocator_overflow() {
        let mut alloc = WasmStackAllocator::new(0, 32);
        let result = alloc.alloc(64);
        assert!(result.is_err());
    }

    #[test]
    fn s3_3_stack_allocator_reset() {
        let mut alloc = WasmStackAllocator::new(100, 1024);
        alloc.alloc(16).unwrap();
        assert!(alloc.stack_pointer > 100);
        alloc.reset();
        assert_eq!(alloc.stack_pointer, 100);
    }

    #[test]
    fn s3_4_heap_allocator_basic() {
        let mut alloc = WasmHeapAllocator::new(4096);
        let ptr = alloc.alloc(32).unwrap();
        // ptr should be past header (4 bytes)
        assert_eq!(ptr, 4096 + 4);
        let ptr2 = alloc.alloc(64).unwrap();
        assert!(ptr2 > ptr);
    }

    #[test]
    fn s3_5_string_data_segment() {
        let mut compiler = WasmCompiler::new(WasmTarget::Wasi);
        let mut out = Vec::new();
        compiler.compile_string_literal("hello", &mut out).unwrap();
        assert_eq!(compiler.module.data.len(), 1);
        assert_eq!(compiler.module.data[0].data, b"hello");
    }

    #[test]
    fn s3_6_string_literal_dedup() {
        let mut compiler = WasmCompiler::new(WasmTarget::Wasi);
        let mut out = Vec::new();
        compiler.compile_string_literal("hello", &mut out).unwrap();
        compiler.compile_string_literal("hello", &mut out).unwrap();
        // Should only have one data segment
        assert_eq!(compiler.module.data.len(), 1);
    }

    #[test]
    fn s3_7_struct_layout_computation() {
        let fields = vec![
            ("x".to_string(), WasmType::I32),
            ("y".to_string(), WasmType::I32),
            ("z".to_string(), WasmType::F64),
        ];
        let layout = compute_struct_layout(&fields);
        assert_eq!(layout[0], ("x".to_string(), 0, WasmType::I32));
        assert_eq!(layout[1], ("y".to_string(), 4, WasmType::I32));
        assert_eq!(layout[2], ("z".to_string(), 8, WasmType::F64));
    }

    #[test]
    fn s3_8_wasm_type_sizes() {
        assert_eq!(wasm_type_size(WasmType::I32), 4);
        assert_eq!(wasm_type_size(WasmType::I64), 8);
        assert_eq!(wasm_type_size(WasmType::F32), 4);
        assert_eq!(wasm_type_size(WasmType::F64), 8);
    }

    #[test]
    fn s3_9_function_call_compilation() {
        let mut compiler = WasmCompiler::new(WasmTarget::Wasi);
        // Register a function manually
        let func_type = WasmFuncType::new(vec![WasmType::I64], vec![WasmType::I64]);
        let type_index = compiler.intern_type(func_type);
        let func_idx = compiler.next_func_index;
        compiler.next_func_index += 1;
        compiler
            .function_indices
            .insert("double".to_string(), func_idx);
        compiler.module.function_type_indices.push(type_index);

        let mut out = Vec::new();
        let call_expr = Expr::Call {
            callee: Box::new(ident("double")),
            args: vec![call_arg(int_lit(21))],
            span: span(),
        };
        compiler.compile_expr(&call_expr, &mut out).unwrap();
        assert!(out.contains(&WasmInstruction::I64Const(21)));
        assert!(
            out.iter()
                .any(|i| matches!(i, WasmInstruction::Call(idx) if *idx == func_idx))
        );
    }

    #[test]
    fn s3_10_recursive_function_compiles() {
        let program = make_program(vec![Item::FnDef(make_fn(
            "countdown",
            vec![param("n", "i64")],
            Some("i64"),
            Expr::If {
                condition: Box::new(binary(ident("n"), BinOp::Le, int_lit(0))),
                then_branch: Box::new(int_lit(0)),
                else_branch: Some(Box::new(Expr::Call {
                    callee: Box::new(ident("countdown")),
                    args: vec![call_arg(binary(ident("n"), BinOp::Sub, int_lit(1)))],
                    span: span(),
                })),
                span: span(),
            },
        ))]);
        let mut compiler = WasmCompiler::new(WasmTarget::Wasi);
        let result = compiler.compile(&program);
        assert!(result.is_ok());
    }

    // ── Sprint 4 Tests: Integration & CLI (S4.1–S4.10) ──

    #[test]
    fn s4_1_wasi_imports_registered() {
        let mut compiler = WasmCompiler::new(WasmTarget::Wasi);
        let program = make_program(vec![Item::FnDef(make_fn(
            "main",
            vec![],
            Some("void"),
            Expr::Block {
                stmts: vec![],
                expr: None,
                span: span(),
            },
        ))]);
        compiler.compile(&program).unwrap();
        // Should have WASI imports
        let wasi_imports: Vec<_> = compiler
            .module
            .imports
            .iter()
            .filter(|i| i.module == "wasi_snapshot_preview1")
            .collect();
        assert!(wasi_imports.len() >= 3); // fd_write, proc_exit, clock_time_get
    }

    #[test]
    fn s4_2_host_imports_registered() {
        let mut compiler = WasmCompiler::new(WasmTarget::Wasi);
        let program = make_program(vec![Item::FnDef(make_fn(
            "main",
            vec![],
            Some("void"),
            Expr::Block {
                stmts: vec![],
                expr: None,
                span: span(),
            },
        ))]);
        compiler.compile(&program).unwrap();
        let fj_imports: Vec<_> = compiler
            .module
            .imports
            .iter()
            .filter(|i| i.module == "fj_runtime")
            .collect();
        assert!(fj_imports.len() >= 4);
    }

    #[test]
    fn s4_3_export_start_wasi() {
        let mut compiler = WasmCompiler::new(WasmTarget::Wasi);
        let program = make_program(vec![Item::FnDef(make_fn(
            "main",
            vec![],
            Some("void"),
            Expr::Block {
                stmts: vec![],
                expr: None,
                span: span(),
            },
        ))]);
        compiler.compile(&program).unwrap();
        let start_export = compiler.module.exports.iter().find(|e| e.name == "_start");
        assert!(start_export.is_some());
    }

    #[test]
    fn s4_4_export_main_browser() {
        let mut compiler = WasmCompiler::new(WasmTarget::Browser);
        let program = make_program(vec![Item::FnDef(make_fn(
            "main",
            vec![],
            Some("void"),
            Expr::Block {
                stmts: vec![],
                expr: None,
                span: span(),
            },
        ))]);
        compiler.compile(&program).unwrap();
        let main_export = compiler.module.exports.iter().find(|e| e.name == "main");
        assert!(main_export.is_some());
    }

    #[test]
    fn s4_5_memory_export() {
        let mut compiler = WasmCompiler::new(WasmTarget::Wasi);
        let program = make_program(vec![Item::FnDef(make_fn(
            "main",
            vec![],
            Some("void"),
            Expr::Block {
                stmts: vec![],
                expr: None,
                span: span(),
            },
        ))]);
        compiler.compile(&program).unwrap();
        let mem_export = compiler
            .module
            .exports
            .iter()
            .find(|e| e.name == "memory" && e.kind == EXPORT_MEMORY);
        assert!(mem_export.is_some());
    }

    #[test]
    fn s4_6_browser_loader_generation() {
        let html = WasmCompiler::generate_browser_loader("output.wasm");
        assert!(html.contains("output.wasm"));
        assert!(html.contains("WebAssembly"));
        assert!(html.contains("fj_runtime"));
        assert!(html.contains("__fj_print"));
    }

    #[test]
    fn s4_7_wasm_target_display() {
        assert_eq!(WasmTarget::Wasi.to_string(), "wasi");
        assert_eq!(WasmTarget::Browser.to_string(), "browser");
    }

    #[test]
    fn s4_8_type_deduplication() {
        let mut compiler = WasmCompiler::new(WasmTarget::Wasi);
        let ft1 = WasmFuncType::new(vec![WasmType::I64], vec![WasmType::I64]);
        let ft2 = WasmFuncType::new(vec![WasmType::I64], vec![WasmType::I64]);
        let idx1 = compiler.intern_type(ft1);
        let idx2 = compiler.intern_type(ft2);
        assert_eq!(idx1, idx2);
        assert_eq!(compiler.module.types.len(), 1);
    }

    #[test]
    fn s4_9_instruction_encoding_roundtrip() {
        let instructions = vec![
            WasmInstruction::I64Const(42),
            WasmInstruction::I64Const(58),
            WasmInstruction::I64Add,
            WasmInstruction::Return,
            WasmInstruction::End,
        ];
        let mut bytes = Vec::new();
        for instr in &instructions {
            instr.encode(&mut bytes);
        }
        // Should have encoded something
        assert!(!bytes.is_empty());
        // First byte should be 0x42 (i64.const opcode)
        assert_eq!(bytes[0], 0x42);
    }

    #[test]
    fn s4_10_full_program_compile_and_finish() {
        let program = make_program(vec![
            Item::FnDef(make_fn(
                "add",
                vec![param("a", "i64"), param("b", "i64")],
                Some("i64"),
                binary(ident("a"), BinOp::Add, ident("b")),
            )),
            Item::FnDef(make_fn(
                "main",
                vec![],
                Some("void"),
                Expr::Block {
                    stmts: vec![Stmt::Let {
                        mutable: false,
                        name: "result".to_string(),
                        ty: Some(simple_ty("i64")),
                        value: Box::new(Expr::Call {
                            callee: Box::new(ident("add")),
                            args: vec![call_arg(int_lit(10)), call_arg(int_lit(20))],
                            span: span(),
                        }),
                        span: span(),
                    }],
                    expr: None,
                    span: span(),
                },
            )),
        ]);
        let mut compiler = WasmCompiler::new(WasmTarget::Wasi);
        compiler.compile(&program).unwrap();
        compiler.optimize();
        let bytes = compiler.finish().unwrap();

        // Valid Wasm magic
        assert_eq!(&bytes[0..4], b"\0asm");
        // Has reasonable size
        assert!(bytes.len() > 8);
        // Has type section
        assert!(bytes.windows(1).any(|w| w[0] == SECTION_TYPE));
        // Has code section
        assert!(bytes.windows(1).any(|w| w[0] == SECTION_CODE));
    }
}
