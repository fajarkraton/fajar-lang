//! Bytecode instruction set for the Fajar Lang VM.
//!
//! Stack-based virtual machine with ~45 opcodes.

/// A single VM instruction (opcode).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Op {
    // ── Stack manipulation ──────────────────────────────────────────
    /// Push constant from pool onto stack.
    Const(u32),
    /// Discard top of stack.
    Pop,
    /// Duplicate top of stack.
    Dup,

    // ── Arithmetic ──────────────────────────────────────────────────
    /// Pop two values, push a + b.
    Add,
    /// Pop two values, push a - b.
    Sub,
    /// Pop two values, push a * b.
    Mul,
    /// Pop two values, push a / b.
    Div,
    /// Pop two values, push a % b.
    Rem,
    /// Pop one value, push -a.
    Neg,
    /// Pop two values, push a ** b.
    Pow,

    // ── Comparison ──────────────────────────────────────────────────
    /// Pop two values, push a == b.
    Eq,
    /// Pop two values, push a != b.
    Ne,
    /// Pop two values, push a < b.
    Lt,
    /// Pop two values, push a <= b.
    Le,
    /// Pop two values, push a > b.
    Gt,
    /// Pop two values, push a >= b.
    Ge,

    // ── Logical ─────────────────────────────────────────────────────
    /// Pop one value, push !a.
    Not,

    // ── Bitwise ─────────────────────────────────────────────────────
    /// Pop two values, push a & b.
    BitAnd,
    /// Pop two values, push a | b.
    BitOr,
    /// Pop two values, push a ^ b.
    BitXor,
    /// Pop one value, push ~a.
    BitNot,
    /// Pop two values, push a << b.
    Shl,
    /// Pop two values, push a >> b.
    Shr,

    // ── Variables ───────────────────────────────────────────────────
    /// Push local variable at slot index onto stack.
    GetLocal(u32),
    /// Pop TOS into local variable at slot index.
    SetLocal(u32),
    /// Push global variable by name index onto stack.
    GetGlobal(u32),
    /// Pop TOS into global variable by name index.
    SetGlobal(u32),
    /// Define a new global variable by name index (pop TOS as value).
    DefineGlobal(u32),

    // ── Control flow ────────────────────────────────────────────────
    /// Unconditional jump (absolute target).
    Jump(u32),
    /// Jump if TOS is false (absolute target). Pops condition.
    JumpIfFalse(u32),
    /// Jump if TOS is true (absolute target). Pops condition.
    JumpIfTrue(u32),

    // ── Functions ───────────────────────────────────────────────────
    /// Call function with N arguments. Callee is on stack below args.
    Call(u8),
    /// Return from current function (TOS is return value).
    Return,

    // ── Data structures ─────────────────────────────────────────────
    /// Create array from N items on stack.
    NewArray(u32),
    /// Create tuple from N items on stack.
    NewTuple(u32),
    /// Create struct: name index on stack, then N field name+value pairs.
    NewStruct(u32),
    /// Get field by name index from struct on TOS.
    GetField(u32),
    /// Set field by name index on struct (value on TOS, struct below).
    SetField(u32),
    /// Array/string index: pop index, pop object, push result.
    GetIndex,
    /// Array index set: pop value, pop index, pop array.
    SetIndex,

    // ── Enum ────────────────────────────────────────────────────────
    /// Create enum variant: name index, with optional data from TOS.
    NewEnum(u32, bool),

    // ── Print ───────────────────────────────────────────────────────
    /// Pop TOS and print (no newline).
    Print,
    /// Pop TOS and print (with newline).
    Println,

    // ── Halt ────────────────────────────────────────────────────────
    /// Stop execution.
    Halt,
}
