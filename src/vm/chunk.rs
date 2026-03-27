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
            other => unreachable!(
                "patch_jump called on {:?}, expected Jump/JumpIfFalse/JumpIfTrue",
                other
            ),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunk_new_is_empty() {
        let chunk = Chunk::new();
        assert!(chunk.code.is_empty());
        assert!(chunk.constants.is_empty());
        assert!(chunk.names.is_empty());
        assert!(chunk.functions.is_empty());
    }

    #[test]
    fn chunk_emit_and_offset() {
        let mut chunk = Chunk::new();
        assert_eq!(chunk.current_offset(), 0);
        chunk.emit(Op::Const(0), 1);
        assert_eq!(chunk.current_offset(), 1);
        chunk.emit(Op::Add, 1);
        assert_eq!(chunk.current_offset(), 2);
        assert_eq!(chunk.code[0], Op::Const(0));
        assert_eq!(chunk.code[1], Op::Add);
    }

    #[test]
    fn chunk_add_constant_deduplicates() {
        let mut chunk = Chunk::new();
        let idx1 = chunk.add_constant(Value::Int(42));
        let idx2 = chunk.add_constant(Value::Int(42));
        assert_eq!(idx1, idx2);
        assert_eq!(chunk.constants.len(), 1);
    }

    #[test]
    fn chunk_add_constant_different_values() {
        let mut chunk = Chunk::new();
        let idx1 = chunk.add_constant(Value::Int(1));
        let idx2 = chunk.add_constant(Value::Int(2));
        assert_ne!(idx1, idx2);
        assert_eq!(chunk.constants.len(), 2);
    }

    #[test]
    fn chunk_add_name() {
        let mut chunk = Chunk::new();
        let idx = chunk.add_name("hello");
        assert_eq!(chunk.names[idx as usize], "hello");
    }

    #[test]
    fn chunk_patch_jump() {
        let mut chunk = Chunk::new();
        let offset = chunk.emit(Op::Jump(0), 1);
        chunk.emit(Op::Const(0), 2);
        chunk.emit(Op::Pop, 3);
        chunk.patch_jump(offset, 3);
        assert_eq!(chunk.code[offset], Op::Jump(3));
    }

    #[test]
    fn chunk_default_is_new() {
        let chunk = Chunk::default();
        assert!(chunk.code.is_empty());
    }

    #[test]
    fn values_equal_ints() {
        assert!(values_equal(&Value::Int(42), &Value::Int(42)));
        assert!(!values_equal(&Value::Int(1), &Value::Int(2)));
    }

    #[test]
    fn values_equal_different_types() {
        assert!(!values_equal(&Value::Int(1), &Value::Float(1.0)));
        assert!(!values_equal(&Value::Bool(true), &Value::Int(1)));
    }
}
