//! Bytecode chunk — container for compiled code, constants, and debug info.

use super::instruction::Op;
use crate::interpreter::value::Value;

/// A compiled function entry.
#[derive(Debug, Clone)]
pub struct FunctionEntry {
    /// Function name.
    pub name: String,
    /// Number of parameters.
    pub arity: u8,
    /// Number of local variable slots (including params).
    pub local_count: u32,
    /// Starting offset in the chunk's code vector.
    pub code_start: usize,
    /// Ending offset (exclusive) in the chunk's code vector.
    pub code_end: usize,
}

/// A compiled bytecode chunk containing code, constants, and metadata.
#[derive(Debug, Clone)]
pub struct Chunk {
    /// The bytecode instructions.
    pub code: Vec<Op>,
    /// Constant pool (literals and interned strings).
    pub constants: Vec<Value>,
    /// String pool for identifiers and field names.
    pub names: Vec<String>,
    /// Function entries.
    pub functions: Vec<FunctionEntry>,
    /// Source line number for each instruction (for error reporting).
    pub lines: Vec<u32>,
}

impl Chunk {
    /// Creates a new empty chunk.
    pub fn new() -> Self {
        Self {
            code: Vec::new(),
            constants: Vec::new(),
            names: Vec::new(),
            functions: Vec::new(),
            lines: Vec::new(),
        }
    }

    /// Emits an instruction and records its source line.
    pub fn emit(&mut self, op: Op, line: u32) -> usize {
        let idx = self.code.len();
        self.code.push(op);
        self.lines.push(line);
        idx
    }

    /// Adds a constant to the pool and returns its index.
    pub fn add_constant(&mut self, value: Value) -> u32 {
        // Deduplicate simple constants
        for (i, existing) in self.constants.iter().enumerate() {
            if values_equal(existing, &value) {
                return i as u32;
            }
        }
        let idx = self.constants.len() as u32;
        self.constants.push(value);
        idx
    }

    /// Adds a name to the name pool and returns its index.
    pub fn add_name(&mut self, name: &str) -> u32 {
        if let Some(idx) = self.names.iter().position(|n| n == name) {
            return idx as u32;
        }
        let idx = self.names.len() as u32;
        self.names.push(name.to_string());
        idx
    }

    /// Returns the current code offset (next instruction index).
    pub fn current_offset(&self) -> usize {
        self.code.len()
    }

    /// Patches a jump instruction at `offset` to jump to `target`.
    pub fn patch_jump(&mut self, offset: usize, target: usize) {
        match &mut self.code[offset] {
            Op::Jump(t) | Op::JumpIfFalse(t) | Op::JumpIfTrue(t) => {
                *t = target as u32;
            }
            _ => panic!("patch_jump called on non-jump instruction"),
        }
    }
}

impl Default for Chunk {
    fn default() -> Self {
        Self::new()
    }
}

/// Simple value equality for constant pool deduplication.
fn values_equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Int(a), Value::Int(b)) => a == b,
        (Value::Float(a), Value::Float(b)) => a == b,
        (Value::Bool(a), Value::Bool(b)) => a == b,
        (Value::Str(a), Value::Str(b)) => a == b,
        (Value::Char(a), Value::Char(b)) => a == b,
        (Value::Null, Value::Null) => true,
        _ => false,
    }
}
