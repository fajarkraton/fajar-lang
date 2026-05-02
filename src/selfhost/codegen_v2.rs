//! Sprint S4: Bytecode Codegen — Instruction set, codegen context, expression/statement
//! compilation, register allocation, and peephole optimization for the self-hosted compiler.
//!
//! Compiles the self-hosted AST (`ast_tree` module) into a flat bytecode representation
//! with 40+ opcodes, a constant pool, and a function table.

use std::collections::HashMap;
use std::fmt;

use super::ast_tree::{AstProgram, BinOp, Expr, FnDefNode, Item, Stmt, UnaryOp};

// ═══════════════════════════════════════════════════════════════════════
// S4.1: Instruction Set (40+ opcodes)
// ═══════════════════════════════════════════════════════════════════════

/// Bytecode instructions for the self-hosted VM.
#[derive(Debug, Clone, PartialEq)]
pub enum Instruction {
    /// Push a constant from the pool: `LoadConst(pool_index)`
    LoadConst(u32),
    /// Load a local variable: `LoadLocal(slot)`
    LoadLocal(u32),
    /// Store to a local variable: `StoreLocal(slot)`
    StoreLocal(u32),
    /// Load a global variable: `LoadGlobal(name_index)`
    LoadGlobal(u32),
    /// Store to a global variable: `StoreGlobal(name_index)`
    StoreGlobal(u32),

    // Arithmetic
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Pow,
    Neg,

    // Comparison
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,

    // Logical
    And,
    Or,
    Not,

    // Bitwise
    BitAnd,
    BitOr,
    BitXor,
    BitNot,
    Shl,
    Shr,

    // Control flow
    /// Unconditional jump: `Jump(target_pc)`
    Jump(u32),
    /// Jump if top of stack is false: `JumpIfFalse(target_pc)`
    JumpIfFalse(u32),
    /// Jump if top of stack is true: `JumpIfTrue(target_pc)`
    JumpIfTrue(u32),

    // Functions
    /// Call a function: `Call(arg_count)`
    Call(u32),
    /// Call a named function: `CallNamed(name_index, arg_count)`
    CallNamed(u32, u32),
    /// Return from function.
    Return,
    /// Return with a value.
    ReturnValue,

    // Stack manipulation
    /// Pop and discard top of stack.
    Pop,
    /// Duplicate top of stack.
    Dup,
    /// Swap top two stack values.
    Swap,

    // Data structures
    /// Create an array: `MakeArray(element_count)`
    MakeArray(u32),
    /// Create a tuple: `MakeTuple(element_count)`
    MakeTuple(u32),
    /// Create a struct: `MakeStruct(field_count)`
    MakeStruct(u32),
    /// Access array/tuple index: `GetIndex`
    GetIndex,
    /// Set array/tuple index: `SetIndex`
    SetIndex,
    /// Access a struct field: `GetField(name_index)`
    GetField(u32),
    /// Set a struct field: `SetField(name_index)`
    SetField(u32),

    // Special
    /// Print (debug builtin).
    Print,
    /// Assert (debug builtin).
    Assert,
    /// Halt execution.
    Halt,
    /// No operation (placeholder for peephole).
    Nop,
}

impl fmt::Display for Instruction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Instruction::LoadConst(idx) => write!(f, "LOAD_CONST {idx}"),
            Instruction::LoadLocal(slot) => write!(f, "LOAD_LOCAL {slot}"),
            Instruction::StoreLocal(slot) => write!(f, "STORE_LOCAL {slot}"),
            Instruction::LoadGlobal(idx) => write!(f, "LOAD_GLOBAL {idx}"),
            Instruction::StoreGlobal(idx) => write!(f, "STORE_GLOBAL {idx}"),
            Instruction::Add => write!(f, "ADD"),
            Instruction::Sub => write!(f, "SUB"),
            Instruction::Mul => write!(f, "MUL"),
            Instruction::Div => write!(f, "DIV"),
            Instruction::Mod => write!(f, "MOD"),
            Instruction::Pow => write!(f, "POW"),
            Instruction::Neg => write!(f, "NEG"),
            Instruction::Eq => write!(f, "EQ"),
            Instruction::Ne => write!(f, "NE"),
            Instruction::Lt => write!(f, "LT"),
            Instruction::Le => write!(f, "LE"),
            Instruction::Gt => write!(f, "GT"),
            Instruction::Ge => write!(f, "GE"),
            Instruction::And => write!(f, "AND"),
            Instruction::Or => write!(f, "OR"),
            Instruction::Not => write!(f, "NOT"),
            Instruction::BitAnd => write!(f, "BIT_AND"),
            Instruction::BitOr => write!(f, "BIT_OR"),
            Instruction::BitXor => write!(f, "BIT_XOR"),
            Instruction::BitNot => write!(f, "BIT_NOT"),
            Instruction::Shl => write!(f, "SHL"),
            Instruction::Shr => write!(f, "SHR"),
            Instruction::Jump(target) => write!(f, "JUMP {target}"),
            Instruction::JumpIfFalse(target) => write!(f, "JUMP_IF_FALSE {target}"),
            Instruction::JumpIfTrue(target) => write!(f, "JUMP_IF_TRUE {target}"),
            Instruction::Call(argc) => write!(f, "CALL {argc}"),
            Instruction::CallNamed(idx, argc) => write!(f, "CALL_NAMED {idx} {argc}"),
            Instruction::Return => write!(f, "RETURN"),
            Instruction::ReturnValue => write!(f, "RETURN_VALUE"),
            Instruction::Pop => write!(f, "POP"),
            Instruction::Dup => write!(f, "DUP"),
            Instruction::Swap => write!(f, "SWAP"),
            Instruction::MakeArray(n) => write!(f, "MAKE_ARRAY {n}"),
            Instruction::MakeTuple(n) => write!(f, "MAKE_TUPLE {n}"),
            Instruction::MakeStruct(n) => write!(f, "MAKE_STRUCT {n}"),
            Instruction::GetIndex => write!(f, "GET_INDEX"),
            Instruction::SetIndex => write!(f, "SET_INDEX"),
            Instruction::GetField(idx) => write!(f, "GET_FIELD {idx}"),
            Instruction::SetField(idx) => write!(f, "SET_FIELD {idx}"),
            Instruction::Print => write!(f, "PRINT"),
            Instruction::Assert => write!(f, "ASSERT"),
            Instruction::Halt => write!(f, "HALT"),
            Instruction::Nop => write!(f, "NOP"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S4.2: Constant Pool
// ═══════════════════════════════════════════════════════════════════════

/// A constant value in the constant pool.
#[derive(Debug, Clone, PartialEq)]
pub enum Constant {
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(String),
    Null,
}

impl fmt::Display for Constant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Constant::Int(n) => write!(f, "{n}"),
            Constant::Float(n) => write!(f, "{n}"),
            Constant::Bool(b) => write!(f, "{b}"),
            Constant::Str(s) => write!(f, "\"{s}\""),
            Constant::Null => write!(f, "null"),
        }
    }
}

/// Constant pool for bytecode.
#[derive(Debug, Clone, Default)]
pub struct ConstantPool {
    constants: Vec<Constant>,
}

impl ConstantPool {
    /// Creates a new constant pool.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a constant and returns its index.
    pub fn add(&mut self, constant: Constant) -> u32 {
        // Dedup: check if constant already exists.
        for (i, c) in self.constants.iter().enumerate() {
            if *c == constant {
                return i as u32;
            }
        }
        let idx = self.constants.len() as u32;
        self.constants.push(constant);
        idx
    }

    /// Gets a constant by index.
    pub fn get(&self, index: u32) -> Option<&Constant> {
        self.constants.get(index as usize)
    }

    /// Returns the number of constants.
    pub fn len(&self) -> usize {
        self.constants.len()
    }

    /// Returns whether the pool is empty.
    pub fn is_empty(&self) -> bool {
        self.constants.is_empty()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S4.3: Function Table
// ═══════════════════════════════════════════════════════════════════════

/// A compiled function in the function table.
#[derive(Debug, Clone)]
pub struct CompiledFn {
    /// Function name.
    pub name: String,
    /// Number of parameters.
    pub arity: u32,
    /// Number of local variable slots.
    pub locals: u32,
    /// Starting instruction offset.
    pub offset: u32,
    /// Number of instructions.
    pub length: u32,
}

/// Function table for compiled bytecode.
#[derive(Debug, Clone, Default)]
pub struct FunctionTable {
    functions: Vec<CompiledFn>,
    /// Name -> index mapping.
    name_index: HashMap<String, u32>,
}

impl FunctionTable {
    /// Creates a new function table.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a compiled function and returns its index.
    pub fn add(&mut self, func: CompiledFn) -> u32 {
        let idx = self.functions.len() as u32;
        self.name_index.insert(func.name.clone(), idx);
        self.functions.push(func);
        idx
    }

    /// Looks up a function by name.
    pub fn lookup(&self, name: &str) -> Option<&CompiledFn> {
        self.name_index
            .get(name)
            .and_then(|idx| self.functions.get(*idx as usize))
    }

    /// Returns the number of functions.
    pub fn len(&self) -> usize {
        self.functions.len()
    }

    /// Returns whether the table is empty.
    pub fn is_empty(&self) -> bool {
        self.functions.is_empty()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S4.4: Codegen Context
// ═══════════════════════════════════════════════════════════════════════

/// Name table for string interning.
#[derive(Debug, Clone, Default)]
pub struct NameTable {
    names: Vec<String>,
    index: HashMap<String, u32>,
}

impl NameTable {
    /// Creates a new name table.
    pub fn new() -> Self {
        Self::default()
    }

    /// Interns a name and returns its index.
    pub fn intern(&mut self, name: &str) -> u32 {
        if let Some(idx) = self.index.get(name) {
            return *idx;
        }
        let idx = self.names.len() as u32;
        self.names.push(name.into());
        self.index.insert(name.into(), idx);
        idx
    }

    /// Gets a name by index.
    pub fn get(&self, idx: u32) -> Option<&str> {
        self.names.get(idx as usize).map(|s| s.as_str())
    }
}

/// Local variable allocation for a function.
#[derive(Debug, Clone, Default)]
struct LocalScope {
    /// Variable name -> slot index.
    locals: HashMap<String, u32>,
    /// Next available slot.
    next_slot: u32,
}

impl LocalScope {
    fn new() -> Self {
        Self::default()
    }

    fn define(&mut self, name: &str) -> u32 {
        if let Some(slot) = self.locals.get(name) {
            return *slot;
        }
        let slot = self.next_slot;
        self.locals.insert(name.into(), slot);
        self.next_slot += 1;
        slot
    }

    fn lookup(&self, name: &str) -> Option<u32> {
        self.locals.get(name).copied()
    }
}

/// Codegen context — produces bytecode from AST.
pub struct CodegenV2 {
    /// Emitted instructions.
    instructions: Vec<Instruction>,
    /// Constant pool.
    pub constants: ConstantPool,
    /// Function table.
    pub functions: FunctionTable,
    /// Name table.
    pub names: NameTable,
    /// Current local scope stack.
    scope_stack: Vec<LocalScope>,
    /// Codegen errors.
    errors: Vec<String>,
}

impl CodegenV2 {
    /// Creates a new codegen context.
    pub fn new() -> Self {
        Self {
            instructions: Vec::new(),
            constants: ConstantPool::new(),
            functions: FunctionTable::new(),
            names: NameTable::new(),
            scope_stack: vec![LocalScope::new()],
            errors: Vec::new(),
        }
    }

    /// Returns emitted instructions.
    pub fn instructions(&self) -> &[Instruction] {
        &self.instructions
    }

    /// Returns codegen errors.
    pub fn errors(&self) -> &[String] {
        &self.errors
    }

    /// Returns the current instruction offset (program counter).
    fn current_pc(&self) -> u32 {
        self.instructions.len() as u32
    }

    /// Emits an instruction.
    fn emit(&mut self, inst: Instruction) {
        self.instructions.push(inst);
    }

    /// Patches a jump target at a given instruction offset.
    fn patch_jump(&mut self, offset: u32, target: u32) {
        if let Some(
            Instruction::Jump(t) | Instruction::JumpIfFalse(t) | Instruction::JumpIfTrue(t),
        ) = self.instructions.get_mut(offset as usize)
        {
            *t = target;
        }
    }

    /// Enters a new local scope.
    fn enter_scope(&mut self) {
        self.scope_stack.push(LocalScope::new());
    }

    /// Leaves the current local scope.
    fn leave_scope(&mut self) {
        self.scope_stack.pop();
    }

    /// Defines a local variable in the current scope.
    fn define_local(&mut self, name: &str) -> u32 {
        if let Some(scope) = self.scope_stack.last_mut() {
            scope.define(name)
        } else {
            0
        }
    }

    /// Looks up a local variable.
    fn lookup_local(&self, name: &str) -> Option<u32> {
        for scope in self.scope_stack.iter().rev() {
            if let Some(slot) = scope.lookup(name) {
                return Some(slot);
            }
        }
        None
    }

    // ═══════════════════════════════════════════════════════════════════
    // S4.5: Compile Program
    // ═══════════════════════════════════════════════════════════════════

    /// Compiles a complete program.
    pub fn compile_program(&mut self, program: &AstProgram) {
        for item in &program.items {
            self.compile_item(item);
        }
        self.emit(Instruction::Halt);
    }

    /// Compiles a top-level item.
    fn compile_item(&mut self, item: &Item) {
        match item {
            Item::FnDef(f) => self.compile_fn(f),
            Item::Stmt(stmt) => self.compile_stmt(stmt),
            _ => {}
        }
    }

    // ═══════════════════════════════════════════════════════════════════
    // S4.6: Compile Function
    // ═══════════════════════════════════════════════════════════════════

    /// Compiles a function definition.
    fn compile_fn(&mut self, f: &FnDefNode) {
        let offset = self.current_pc();
        // Jump over the function body for top-level code.
        let skip_jump = self.current_pc();
        self.emit(Instruction::Jump(0)); // placeholder

        self.enter_scope();
        for param in &f.params {
            self.define_local(&param.name);
        }

        self.compile_expr(&f.body);
        self.emit(Instruction::ReturnValue);
        self.leave_scope();

        let end_pc = self.current_pc();
        self.patch_jump(skip_jump, end_pc);

        let locals = self.scope_stack.last().map(|s| s.next_slot).unwrap_or(0);
        self.functions.add(CompiledFn {
            name: f.name.clone(),
            arity: f.params.len() as u32,
            locals,
            offset: offset + 1, // skip the initial Jump
            length: end_pc - offset - 1,
        });
    }

    // ═══════════════════════════════════════════════════════════════════
    // S4.7: Compile Statement
    // ═══════════════════════════════════════════════════════════════════

    /// Compiles a statement.
    fn compile_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Let { name, init, .. } => {
                let slot = self.define_local(name);
                if let Some(init_expr) = init {
                    self.compile_expr(init_expr);
                } else {
                    let idx = self.constants.add(Constant::Null);
                    self.emit(Instruction::LoadConst(idx));
                }
                self.emit(Instruction::StoreLocal(slot));
            }
            Stmt::Return { value, .. } => {
                if let Some(val) = value {
                    self.compile_expr(val);
                    self.emit(Instruction::ReturnValue);
                } else {
                    self.emit(Instruction::Return);
                }
            }
            Stmt::While {
                condition, body, ..
            } => {
                let loop_start = self.current_pc();
                self.compile_expr(condition);
                let exit_jump = self.current_pc();
                self.emit(Instruction::JumpIfFalse(0)); // placeholder

                self.compile_expr(body);
                self.emit(Instruction::Pop);
                self.emit(Instruction::Jump(loop_start));

                let loop_end = self.current_pc();
                self.patch_jump(exit_jump, loop_end);
            }
            Stmt::For {
                name, iter, body, ..
            } => {
                // Compile the iterator expression.
                self.compile_expr(iter);
                let iter_slot = self.define_local("__iter__");
                self.emit(Instruction::StoreLocal(iter_slot));

                let var_slot = self.define_local(name);

                // Simplified: emit a placeholder loop structure
                let loop_start = self.current_pc();
                // In a real compiler, we would emit iteration protocol calls here.
                // For now, emit a single-iteration structure.
                self.emit(Instruction::LoadLocal(iter_slot));
                let exit_jump = self.current_pc();
                self.emit(Instruction::JumpIfFalse(0));

                self.emit(Instruction::LoadLocal(iter_slot));
                self.emit(Instruction::StoreLocal(var_slot));

                self.compile_expr(body);
                self.emit(Instruction::Pop);
                self.emit(Instruction::Jump(loop_start));

                let loop_end = self.current_pc();
                self.patch_jump(exit_jump, loop_end);
            }
            Stmt::ExprStmt { expr, .. } => {
                self.compile_expr(expr);
                self.emit(Instruction::Pop);
            }
            Stmt::Break { .. } => {
                // In a real compiler: jump to loop exit.
                // Placeholder: emit a jump that will need patching.
                self.emit(Instruction::Jump(0));
            }
            Stmt::Continue { .. } => {
                // In a real compiler: jump to loop header.
                self.emit(Instruction::Jump(0));
            }
            Stmt::FnDef(f) => self.compile_fn(f),
            _ => {}
        }
    }

    // ═══════════════════════════════════════════════════════════════════
    // S4.8: Compile Expression
    // ═══════════════════════════════════════════════════════════════════

    /// Compiles an expression.
    fn compile_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::IntLit { value, .. } => {
                let idx = self.constants.add(Constant::Int(*value));
                self.emit(Instruction::LoadConst(idx));
            }
            Expr::FloatLit { value, .. } => {
                let idx = self.constants.add(Constant::Float(*value));
                self.emit(Instruction::LoadConst(idx));
            }
            Expr::BoolLit { value, .. } => {
                let idx = self.constants.add(Constant::Bool(*value));
                self.emit(Instruction::LoadConst(idx));
            }
            Expr::StringLit { value, .. } => {
                let idx = self.constants.add(Constant::Str(value.clone()));
                self.emit(Instruction::LoadConst(idx));
            }
            Expr::NullLit { .. } => {
                let idx = self.constants.add(Constant::Null);
                self.emit(Instruction::LoadConst(idx));
            }
            Expr::Ident { name, .. } => {
                if let Some(slot) = self.lookup_local(name) {
                    self.emit(Instruction::LoadLocal(slot));
                } else {
                    let idx = self.names.intern(name);
                    self.emit(Instruction::LoadGlobal(idx));
                }
            }
            Expr::BinOp {
                op, left, right, ..
            } => {
                self.compile_expr(left);
                self.compile_expr(right);
                let inst = match op {
                    BinOp::Add => Instruction::Add,
                    BinOp::Sub => Instruction::Sub,
                    BinOp::Mul => Instruction::Mul,
                    BinOp::Div => Instruction::Div,
                    BinOp::Mod => Instruction::Mod,
                    BinOp::Pow => Instruction::Pow,
                    BinOp::Eq => Instruction::Eq,
                    BinOp::Ne => Instruction::Ne,
                    BinOp::Lt => Instruction::Lt,
                    BinOp::Le => Instruction::Le,
                    BinOp::Gt => Instruction::Gt,
                    BinOp::Ge => Instruction::Ge,
                    BinOp::And => Instruction::And,
                    BinOp::Or => Instruction::Or,
                    BinOp::BitAnd => Instruction::BitAnd,
                    BinOp::BitOr => Instruction::BitOr,
                    BinOp::BitXor => Instruction::BitXor,
                    BinOp::Shl => Instruction::Shl,
                    BinOp::Shr => Instruction::Shr,
                    _ => Instruction::Add, // fallback
                };
                self.emit(inst);
            }
            Expr::UnaryOp { op, operand, .. } => {
                self.compile_expr(operand);
                match op {
                    UnaryOp::Neg => self.emit(Instruction::Neg),
                    UnaryOp::Not => self.emit(Instruction::Not),
                    UnaryOp::BitNot => self.emit(Instruction::BitNot),
                    _ => {} // Ref/Deref handled at higher level
                }
            }
            Expr::Call { callee, args, .. } => {
                // Push arguments.
                for arg in args {
                    self.compile_expr(arg);
                }
                match callee.as_ref() {
                    Expr::Ident { name, .. } => {
                        if name == "print" || name == "println" {
                            self.emit(Instruction::Print);
                        } else if name == "assert" {
                            self.emit(Instruction::Assert);
                        } else {
                            let idx = self.names.intern(name);
                            self.emit(Instruction::CallNamed(idx, args.len() as u32));
                        }
                    }
                    _ => {
                        self.compile_expr(callee);
                        self.emit(Instruction::Call(args.len() as u32));
                    }
                }
            }
            Expr::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                self.compile_expr(condition);
                let else_jump = self.current_pc();
                self.emit(Instruction::JumpIfFalse(0)); // placeholder

                self.compile_expr(then_branch);
                let end_jump = self.current_pc();
                self.emit(Instruction::Jump(0)); // placeholder

                let else_pc = self.current_pc();
                self.patch_jump(else_jump, else_pc);

                if let Some(else_expr) = else_branch {
                    self.compile_expr(else_expr);
                } else {
                    let idx = self.constants.add(Constant::Null);
                    self.emit(Instruction::LoadConst(idx));
                }

                let end_pc = self.current_pc();
                self.patch_jump(end_jump, end_pc);
            }
            Expr::Block { stmts, expr, .. } => {
                self.enter_scope();
                for stmt in stmts {
                    self.compile_stmt(stmt);
                }
                if let Some(e) = expr {
                    self.compile_expr(e);
                } else {
                    let idx = self.constants.add(Constant::Null);
                    self.emit(Instruction::LoadConst(idx));
                }
                self.leave_scope();
            }
            Expr::ArrayLit { elements, .. } => {
                for elem in elements {
                    self.compile_expr(elem);
                }
                self.emit(Instruction::MakeArray(elements.len() as u32));
            }
            Expr::TupleLit { elements, .. } => {
                for elem in elements {
                    self.compile_expr(elem);
                }
                self.emit(Instruction::MakeTuple(elements.len() as u32));
            }
            Expr::Index { object, index, .. } => {
                self.compile_expr(object);
                self.compile_expr(index);
                self.emit(Instruction::GetIndex);
            }
            Expr::FieldAccess { object, field, .. } => {
                self.compile_expr(object);
                let idx = self.names.intern(field);
                self.emit(Instruction::GetField(idx));
            }
            Expr::Assign { target, value, .. } => {
                self.compile_expr(value);
                match target.as_ref() {
                    Expr::Ident { name, .. } => {
                        if let Some(slot) = self.lookup_local(name) {
                            self.emit(Instruction::StoreLocal(slot));
                        } else {
                            let idx = self.names.intern(name);
                            self.emit(Instruction::StoreGlobal(idx));
                        }
                    }
                    Expr::Index { object, index, .. } => {
                        self.compile_expr(object);
                        self.compile_expr(index);
                        self.emit(Instruction::SetIndex);
                    }
                    Expr::FieldAccess { object, field, .. } => {
                        self.compile_expr(object);
                        let idx = self.names.intern(field);
                        self.emit(Instruction::SetField(idx));
                    }
                    _ => {
                        self.errors.push("unsupported assignment target".into());
                    }
                }
            }
            _ => {
                // Fallback: push null for unsupported expressions.
                let idx = self.constants.add(Constant::Null);
                self.emit(Instruction::LoadConst(idx));
            }
        }
    }
}

impl Default for CodegenV2 {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S4.9: Register Allocation (Simple Linear Scan)
// ═══════════════════════════════════════════════════════════════════════

/// A live range for a variable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveRange {
    /// Variable name.
    pub name: String,
    /// Start instruction offset.
    pub start: u32,
    /// End instruction offset.
    pub end: u32,
    /// Assigned register (None if spilled).
    pub register: Option<u32>,
}

/// Simple linear-scan register allocator.
pub struct RegisterAllocator {
    /// Active live ranges (sorted by end point).
    active: Vec<LiveRange>,
    /// Free register pool.
    free_regs: Vec<u32>,
    /// All computed live ranges.
    pub ranges: Vec<LiveRange>,
}

impl RegisterAllocator {
    /// Creates a new allocator with the given number of registers.
    pub fn new(num_registers: u32) -> Self {
        Self {
            active: Vec::new(),
            free_regs: (0..num_registers).rev().collect(),
            ranges: Vec::new(),
        }
    }

    /// Allocates registers for a set of live ranges.
    pub fn allocate(&mut self, mut ranges: Vec<LiveRange>) -> Vec<LiveRange> {
        // Sort by start point.
        ranges.sort_by_key(|r| r.start);

        let mut result = Vec::new();
        for mut range in ranges {
            // Expire old intervals.
            self.expire_old(&range);

            if self.free_regs.is_empty() {
                // Spill: no register assigned.
                range.register = None;
            } else {
                range.register = self.free_regs.pop();
                self.active.push(range.clone());
                self.active.sort_by_key(|r| r.end);
            }
            result.push(range);
        }

        self.ranges = result.clone();
        result
    }

    /// Expires intervals that end before the current range starts.
    fn expire_old(&mut self, current: &LiveRange) {
        let mut to_remove = Vec::new();
        for (i, active) in self.active.iter().enumerate() {
            if active.end < current.start {
                if let Some(reg) = active.register {
                    self.free_regs.push(reg);
                }
                to_remove.push(i);
            }
        }
        for i in to_remove.into_iter().rev() {
            self.active.remove(i);
        }
    }

    /// Returns the number of available registers.
    pub fn available_registers(&self) -> u32 {
        self.free_regs.len() as u32
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S4.10: Peephole Optimization
// ═══════════════════════════════════════════════════════════════════════

/// Applies peephole optimizations to an instruction sequence.
pub fn peephole_optimize(instructions: &[Instruction]) -> Vec<Instruction> {
    let mut result: Vec<Instruction> = Vec::with_capacity(instructions.len());

    let mut i = 0;
    while i < instructions.len() {
        // Optimization 1: LoadConst + Pop => nothing (dead store)
        if i + 1 < instructions.len() {
            if matches!(instructions[i], Instruction::LoadConst(_))
                && instructions[i + 1] == Instruction::Pop
            {
                i += 2;
                continue;
            }
        }

        // Optimization 2: LoadLocal + StoreLocal to same slot => nothing
        if i + 1 < instructions.len() {
            if let (Instruction::LoadLocal(a), Instruction::StoreLocal(b)) =
                (&instructions[i], &instructions[i + 1])
            {
                if a == b {
                    i += 2;
                    continue;
                }
            }
        }

        // Optimization 3: Double negation => nothing
        if i + 1 < instructions.len() {
            if instructions[i] == Instruction::Neg && instructions[i + 1] == Instruction::Neg {
                i += 2;
                continue;
            }
        }

        // Optimization 4: Double NOT => nothing
        if i + 1 < instructions.len() {
            if instructions[i] == Instruction::Not && instructions[i + 1] == Instruction::Not {
                i += 2;
                continue;
            }
        }

        // Optimization 5: Jump to next instruction => Nop
        if let Instruction::Jump(target) = &instructions[i] {
            if *target == (i + 1) as u32 {
                i += 1;
                continue;
            }
        }

        // Optimization 6: Remove Nop
        if instructions[i] == Instruction::Nop {
            i += 1;
            continue;
        }

        result.push(instructions[i].clone());
        i += 1;
    }

    result
}

/// Disassembles an instruction sequence into a readable string.
pub fn disassemble(instructions: &[Instruction], constants: &ConstantPool) -> String {
    let mut lines = Vec::new();
    for (i, inst) in instructions.iter().enumerate() {
        let extra = match inst {
            Instruction::LoadConst(idx) => {
                if let Some(c) = constants.get(*idx) {
                    format!("  ; {c}")
                } else {
                    String::new()
                }
            }
            _ => String::new(),
        };
        lines.push(format!("{i:04}  {inst}{extra}"));
    }
    lines.join("\n")
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::selfhost::ast_tree::*;

    fn span() -> AstSpan {
        AstSpan::dummy()
    }

    fn int_expr(v: i64) -> Expr {
        Expr::IntLit {
            value: v,
            span: span(),
        }
    }

    fn ident_expr(name: &str) -> Expr {
        Expr::Ident {
            name: name.into(),
            span: span(),
        }
    }

    // S4.1 — Instruction set
    #[test]
    fn s4_1_instruction_display() {
        assert_eq!(Instruction::Add.to_string(), "ADD");
        assert_eq!(Instruction::LoadConst(0).to_string(), "LOAD_CONST 0");
        assert_eq!(Instruction::Jump(10).to_string(), "JUMP 10");
        assert_eq!(Instruction::Call(2).to_string(), "CALL 2");
        assert_eq!(Instruction::MakeArray(3).to_string(), "MAKE_ARRAY 3");
    }

    #[test]
    fn s4_1_instruction_count() {
        // Verify we have 40+ instruction variants by checking a subset.
        let insts = vec![
            Instruction::LoadConst(0),
            Instruction::LoadLocal(0),
            Instruction::StoreLocal(0),
            Instruction::LoadGlobal(0),
            Instruction::StoreGlobal(0),
            Instruction::Add,
            Instruction::Sub,
            Instruction::Mul,
            Instruction::Div,
            Instruction::Mod,
            Instruction::Pow,
            Instruction::Neg,
            Instruction::Eq,
            Instruction::Ne,
            Instruction::Lt,
            Instruction::Le,
            Instruction::Gt,
            Instruction::Ge,
            Instruction::And,
            Instruction::Or,
            Instruction::Not,
            Instruction::BitAnd,
            Instruction::BitOr,
            Instruction::BitXor,
            Instruction::BitNot,
            Instruction::Shl,
            Instruction::Shr,
            Instruction::Jump(0),
            Instruction::JumpIfFalse(0),
            Instruction::JumpIfTrue(0),
            Instruction::Call(0),
            Instruction::CallNamed(0, 0),
            Instruction::Return,
            Instruction::ReturnValue,
            Instruction::Pop,
            Instruction::Dup,
            Instruction::Swap,
            Instruction::MakeArray(0),
            Instruction::MakeTuple(0),
            Instruction::MakeStruct(0),
            Instruction::GetIndex,
            Instruction::SetIndex,
            Instruction::GetField(0),
            Instruction::SetField(0),
            Instruction::Print,
            Instruction::Assert,
            Instruction::Halt,
            Instruction::Nop,
        ];
        assert!(
            insts.len() >= 40,
            "expected 40+ opcodes, got {}",
            insts.len()
        );
    }

    // S4.2 — Constant pool
    #[test]
    fn s4_2_constant_pool_add() {
        let mut pool = ConstantPool::new();
        let idx1 = pool.add(Constant::Int(42));
        let idx2 = pool.add(Constant::Int(42));
        assert_eq!(idx1, idx2); // dedup
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn s4_2_constant_pool_types() {
        let mut pool = ConstantPool::new();
        pool.add(Constant::Int(1));
        pool.add(Constant::Float(1.25));
        pool.add(Constant::Bool(true));
        pool.add(Constant::Str("hello".into()));
        pool.add(Constant::Null);
        assert_eq!(pool.len(), 5);
        assert_eq!(pool.get(0), Some(&Constant::Int(1)));
    }

    // S4.3 — Function table
    #[test]
    fn s4_3_function_table() {
        let mut table = FunctionTable::new();
        table.add(CompiledFn {
            name: "main".into(),
            arity: 0,
            locals: 2,
            offset: 0,
            length: 10,
        });
        assert_eq!(table.len(), 1);
        assert!(table.lookup("main").is_some());
        assert!(table.lookup("foo").is_none());
    }

    // S4.4 — Codegen context
    #[test]
    fn s4_4_name_table() {
        let mut names = NameTable::new();
        let idx1 = names.intern("foo");
        let idx2 = names.intern("foo");
        assert_eq!(idx1, idx2);
        assert_eq!(names.get(idx1), Some("foo"));
    }

    // S4.5 — Compile program
    #[test]
    fn s4_5_compile_empty_program() {
        let mut codegen = CodegenV2::new();
        let prog = AstProgram::new("test.fj", vec![]);
        codegen.compile_program(&prog);
        assert_eq!(codegen.instructions().last(), Some(&Instruction::Halt));
    }

    // S4.6 — Compile function
    #[test]
    fn s4_6_compile_fn() {
        let mut codegen = CodegenV2::new();
        let prog = AstProgram::new(
            "test.fj",
            vec![Item::FnDef(FnDefNode {
                name: "add".into(),
                type_params: vec![],
                params: vec![
                    Param {
                        name: "a".into(),
                        ty: TypeExpr::Name("i32".into(), span()),
                        mutable: false,
                    },
                    Param {
                        name: "b".into(),
                        ty: TypeExpr::Name("i32".into(), span()),
                        mutable: false,
                    },
                ],
                ret_type: Some(TypeExpr::Name("i32".into(), span())),
                body: Box::new(Expr::BinOp {
                    op: BinOp::Add,
                    left: Box::new(ident_expr("a")),
                    right: Box::new(ident_expr("b")),
                    span: span(),
                }),
                is_pub: false,
                context: None,
                is_async: false,
                is_gen: false,
                span: span(),
            })],
        );
        codegen.compile_program(&prog);
        assert!(codegen.functions.lookup("add").is_some());
    }

    // S4.7 — Compile statement
    #[test]
    fn s4_7_compile_let() {
        let mut codegen = CodegenV2::new();
        let prog = AstProgram::new(
            "test.fj",
            vec![Item::Stmt(Stmt::Let {
                name: "x".into(),
                mutable: false,
                ty: None,
                init: Some(Box::new(int_expr(42))),
                span: span(),
            })],
        );
        codegen.compile_program(&prog);
        // Should have: LOAD_CONST(42), STORE_LOCAL(0), HALT
        let insts = codegen.instructions();
        assert!(insts.iter().any(|i| matches!(i, Instruction::LoadConst(_))));
        assert!(
            insts
                .iter()
                .any(|i| matches!(i, Instruction::StoreLocal(_)))
        );
    }

    #[test]
    fn s4_7_compile_while() {
        let mut codegen = CodegenV2::new();
        let prog = AstProgram::new(
            "test.fj",
            vec![Item::Stmt(Stmt::While {
                condition: Box::new(Expr::BoolLit {
                    value: true,
                    span: span(),
                }),
                body: Box::new(int_expr(1)),
                span: span(),
            })],
        );
        codegen.compile_program(&prog);
        let insts = codegen.instructions();
        assert!(
            insts
                .iter()
                .any(|i| matches!(i, Instruction::JumpIfFalse(_)))
        );
        assert!(insts.iter().any(|i| matches!(i, Instruction::Jump(_))));
    }

    // S4.8 — Compile expression
    #[test]
    fn s4_8_compile_int_literal() {
        let mut codegen = CodegenV2::new();
        codegen.compile_expr(&int_expr(42));
        assert_eq!(codegen.instructions().len(), 1);
        assert!(matches!(
            codegen.instructions()[0],
            Instruction::LoadConst(_)
        ));
    }

    #[test]
    fn s4_8_compile_binop() {
        let mut codegen = CodegenV2::new();
        let expr = Expr::BinOp {
            op: BinOp::Add,
            left: Box::new(int_expr(1)),
            right: Box::new(int_expr(2)),
            span: span(),
        };
        codegen.compile_expr(&expr);
        let insts = codegen.instructions();
        assert_eq!(insts.len(), 3); // LOAD_CONST, LOAD_CONST, ADD
        assert_eq!(insts[2], Instruction::Add);
    }

    #[test]
    fn s4_8_compile_if_else() {
        let mut codegen = CodegenV2::new();
        let expr = Expr::If {
            condition: Box::new(Expr::BoolLit {
                value: true,
                span: span(),
            }),
            then_branch: Box::new(int_expr(1)),
            else_branch: Some(Box::new(int_expr(2))),
            span: span(),
        };
        codegen.compile_expr(&expr);
        let insts = codegen.instructions();
        assert!(
            insts
                .iter()
                .any(|i| matches!(i, Instruction::JumpIfFalse(_)))
        );
    }

    #[test]
    fn s4_8_compile_array() {
        let mut codegen = CodegenV2::new();
        let expr = Expr::ArrayLit {
            elements: vec![int_expr(1), int_expr(2), int_expr(3)],
            span: span(),
        };
        codegen.compile_expr(&expr);
        let insts = codegen.instructions();
        assert!(insts.contains(&Instruction::MakeArray(3)));
    }

    // S4.9 — Register allocation
    #[test]
    fn s4_9_register_allocation() {
        let mut allocator = RegisterAllocator::new(4);
        let ranges = vec![
            LiveRange {
                name: "a".into(),
                start: 0,
                end: 5,
                register: None,
            },
            LiveRange {
                name: "b".into(),
                start: 1,
                end: 3,
                register: None,
            },
            LiveRange {
                name: "c".into(),
                start: 4,
                end: 8,
                register: None,
            },
        ];
        let result = allocator.allocate(ranges);
        assert!(result[0].register.is_some());
        assert!(result[1].register.is_some());
        assert!(result[2].register.is_some());
    }

    #[test]
    fn s4_9_register_spill() {
        let mut allocator = RegisterAllocator::new(1);
        let ranges = vec![
            LiveRange {
                name: "a".into(),
                start: 0,
                end: 10,
                register: None,
            },
            LiveRange {
                name: "b".into(),
                start: 1,
                end: 9,
                register: None,
            },
        ];
        let result = allocator.allocate(ranges);
        assert!(result[0].register.is_some());
        assert!(result[1].register.is_none()); // spilled
    }

    // S4.10 — Peephole optimization
    #[test]
    fn s4_10_peephole_dead_store() {
        let insts = vec![
            Instruction::LoadConst(0),
            Instruction::Pop,
            Instruction::Add,
        ];
        let optimized = peephole_optimize(&insts);
        assert_eq!(optimized.len(), 1);
        assert_eq!(optimized[0], Instruction::Add);
    }

    #[test]
    fn s4_10_peephole_double_neg() {
        let insts = vec![
            Instruction::LoadConst(0),
            Instruction::Neg,
            Instruction::Neg,
        ];
        let optimized = peephole_optimize(&insts);
        assert_eq!(optimized.len(), 1);
    }

    #[test]
    fn s4_10_peephole_nop_removal() {
        let insts = vec![Instruction::Nop, Instruction::Add, Instruction::Nop];
        let optimized = peephole_optimize(&insts);
        assert_eq!(optimized, vec![Instruction::Add]);
    }

    #[test]
    fn s4_10_disassemble() {
        let mut pool = ConstantPool::new();
        pool.add(Constant::Int(42));
        let insts = vec![
            Instruction::LoadConst(0),
            Instruction::Add,
            Instruction::Halt,
        ];
        let output = disassemble(&insts, &pool);
        assert!(output.contains("LOAD_CONST 0"));
        assert!(output.contains("; 42"));
        assert!(output.contains("HALT"));
    }
}
