//! AST → Bytecode compiler for the Fajar Lang VM.

use std::collections::HashMap;

use crate::interpreter::value::{FnValue, Value};
use crate::parser::ast::*;

use super::chunk::{Chunk, FunctionEntry};
use super::instruction::Op;

/// Scope for tracking local variables during compilation.
#[derive(Debug)]
struct Local {
    name: String,
    depth: u32,
}

/// Break/continue target for loop compilation.
#[derive(Debug, Clone)]
struct LoopContext {
    /// Where to jump for `continue`.
    continue_target: usize,
    /// Placeholder offsets for `break` that need patching.
    break_patches: Vec<usize>,
    /// Scope depth at loop entry (for cleanup).
    scope_depth: u32,
}

/// Compiles an AST into bytecode.
pub struct Compiler {
    /// The bytecode chunk being built.
    chunk: Chunk,
    /// Local variable stack for the current function scope.
    locals: Vec<Local>,
    /// Current scope depth (0 = global).
    scope_depth: u32,
    /// Active loop contexts for break/continue.
    loop_stack: Vec<LoopContext>,
    /// Map of function names → indices in chunk.functions.
    function_map: HashMap<String, usize>,
    /// Struct definitions (name → field names).
    struct_defs: HashMap<String, Vec<String>>,
    /// Whether we are currently compiling inside a function body.
    in_function: bool,
}

impl Default for Compiler {
    fn default() -> Self {
        Self::new()
    }
}

impl Compiler {
    /// Creates a new compiler.
    pub fn new() -> Self {
        Self {
            chunk: Chunk::new(),
            locals: Vec::new(),
            scope_depth: 0,
            loop_stack: Vec::new(),
            function_map: HashMap::new(),
            struct_defs: HashMap::new(),
            in_function: false,
        }
    }

    /// Compiles a program and returns the bytecode chunk.
    pub fn compile(mut self, program: &Program) -> Chunk {
        // First pass: register all function and struct definitions
        for item in &program.items {
            match item {
                Item::FnDef(f) => {
                    let idx = self.chunk.functions.len();
                    self.function_map.insert(f.name.clone(), idx);
                    // Placeholder entry, will be filled during compilation
                    self.chunk.functions.push(FunctionEntry {
                        name: f.name.clone(),
                        arity: f.params.len() as u8,
                        local_count: 0,
                        code_start: 0,
                        code_end: 0,
                    });
                }
                Item::StructDef(s) => {
                    let fields: Vec<String> = s.fields.iter().map(|f| f.name.clone()).collect();
                    self.struct_defs.insert(s.name.clone(), fields);
                }
                _ => {}
            }
        }

        // Second pass: compile top-level statements and function bodies
        for item in &program.items {
            self.compile_item(item);
        }

        self.emit(Op::Halt, 0);
        self.chunk
    }

    // ── Item compilation ────────────────────────────────────────────

    fn compile_item(&mut self, item: &Item) {
        match item {
            Item::FnDef(f) => self.compile_fn_def(f),
            Item::StructDef(_) => {} // Already registered
            Item::UnionDef(_) => {}  // Registered similarly to struct
            Item::EnumDef(e) => self.compile_enum_def(e),
            Item::ConstDef(c) => self.compile_const_def(c),
            Item::StaticDef(s) => {
                // For VM, treat static mut like a const def
                self.compile_const_def(&ConstDef {
                    is_pub: s.is_pub,
                    doc_comment: s.doc_comment.clone(),
                    annotation: s.annotation.clone(),
                    name: s.name.clone(),
                    ty: s.ty.clone(),
                    value: s.value.clone(),
                    span: s.span,
                });
            }
            Item::ServiceDef(svc) => {
                for handler in &svc.handlers {
                    self.compile_fn_def(handler);
                }
            }
            Item::Stmt(stmt) => self.compile_stmt(stmt),
            Item::ImplBlock(imp) => {
                for method in &imp.methods {
                    // Register as TypeName::method_name
                    let full_name = format!("{}::{}", imp.target_type, method.name);
                    let idx = self.chunk.functions.len();
                    self.function_map.insert(full_name.clone(), idx);
                    self.chunk.functions.push(FunctionEntry {
                        name: full_name,
                        arity: method.params.len() as u8,
                        local_count: 0,
                        code_start: 0,
                        code_end: 0,
                    });
                    self.compile_fn_def(method);
                }
            }
            Item::UseDecl(_)
            | Item::ModDecl(_)
            | Item::TraitDef(_)
            | Item::ExternFn(_)
            | Item::TypeAlias(_)
            | Item::GlobalAsm(_)
            | Item::EffectDecl(_)
            | Item::MacroRulesDef(_) => {
                // Module/use/trait/extern/type-alias/global_asm/effect/macros handled by pre-processing
            }
        }
    }

    fn compile_fn_def(&mut self, f: &FnDef) {
        let func_idx = match self.function_map.get(&f.name) {
            Some(&idx) => idx,
            None => return, // impl method already registered with full name
        };

        let saved_locals = std::mem::take(&mut self.locals);
        let saved_depth = self.scope_depth;
        let saved_in_fn = self.in_function;

        self.scope_depth = 1;
        self.in_function = true;

        // Parameters become local variables
        for param in &f.params {
            self.locals.push(Local {
                name: param.name.clone(),
                depth: 1,
            });
        }

        let code_start = self.chunk.current_offset();

        // Compile body
        self.compile_expr(&f.body);
        self.emit(Op::Return, 0);

        let code_end = self.chunk.current_offset();
        let local_count = self.locals.len() as u32;

        // Update function entry
        self.chunk.functions[func_idx].code_start = code_start;
        self.chunk.functions[func_idx].code_end = code_end;
        self.chunk.functions[func_idx].local_count = local_count;

        self.locals = saved_locals;
        self.scope_depth = saved_depth;
        self.in_function = saved_in_fn;
    }

    fn compile_enum_def(&mut self, e: &crate::parser::ast::EnumDef) {
        for variant in &e.variants {
            let qualified = format!("{}::{}", e.name, variant.name);
            let has_data = !variant.fields.is_empty();
            if !has_data {
                // Unit variant: register as global enum value
                let name_idx = self.chunk.add_name(&variant.name);
                self.emit(Op::NewEnum(name_idx, false), 0);
                let global_idx = self.chunk.add_name(&qualified);
                self.emit(Op::DefineGlobal(global_idx), 0);
            }
            // Variants with data are constructed at call site
        }
    }

    fn compile_const_def(&mut self, c: &ConstDef) {
        self.compile_expr(&c.value);
        let name_idx = self.chunk.add_name(&c.name);
        self.emit(Op::DefineGlobal(name_idx), 0);
    }

    // ── Statement compilation ───────────────────────────────────────

    fn compile_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Let { name, value, .. } => {
                self.compile_expr(value);
                if self.scope_depth > 0 && self.in_function {
                    // Local variable
                    self.locals.push(Local {
                        name: name.clone(),
                        depth: self.scope_depth,
                    });
                    let slot = (self.locals.len() - 1) as u32;
                    self.emit(Op::SetLocal(slot), 0);
                } else {
                    // Global variable
                    let name_idx = self.chunk.add_name(name);
                    self.emit(Op::DefineGlobal(name_idx), 0);
                }
            }
            Stmt::Const { name, value, .. } => {
                self.compile_expr(value);
                let name_idx = self.chunk.add_name(name);
                self.emit(Op::DefineGlobal(name_idx), 0);
            }
            Stmt::Expr { expr, .. } => {
                self.compile_expr(expr);
                self.emit(Op::Pop, 0);
            }
            Stmt::Return { value, .. } => {
                if let Some(val) = value {
                    self.compile_expr(val);
                } else {
                    let idx = self.chunk.add_constant(Value::Null);
                    self.emit(Op::Const(idx), 0);
                }
                self.emit(Op::Return, 0);
            }
            Stmt::Break { value, .. } => {
                if let Some(val) = value {
                    self.compile_expr(val);
                } else {
                    let idx = self.chunk.add_constant(Value::Null);
                    self.emit(Op::Const(idx), 0);
                }
                // Pop locals declared inside loop body before jumping out
                if let Some(ctx) = self.loop_stack.last() {
                    let loop_depth = ctx.scope_depth;
                    let locals_to_pop = self
                        .locals
                        .iter()
                        .rev()
                        .take_while(|l| l.depth > loop_depth)
                        .count();
                    for _ in 0..locals_to_pop {
                        self.emit(Op::Pop, 0);
                    }
                }
                let jump_offset = self.emit(Op::Jump(0), 0);
                if let Some(ctx) = self.loop_stack.last_mut() {
                    ctx.break_patches.push(jump_offset);
                }
            }
            Stmt::Continue { .. } => {
                // Pop locals declared inside loop body before jumping to continue
                if let Some(ctx) = self.loop_stack.last() {
                    let loop_depth = ctx.scope_depth;
                    let target = ctx.continue_target;
                    let locals_to_pop = self
                        .locals
                        .iter()
                        .rev()
                        .take_while(|l| l.depth > loop_depth)
                        .count();
                    for _ in 0..locals_to_pop {
                        self.emit(Op::Pop, 0);
                    }
                    self.emit(Op::Jump(target as u32), 0);
                }
            }
            Stmt::Item(item) => self.compile_item(item),
        }
    }

    // ── Expression compilation ──────────────────────────────────────

    fn compile_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Literal { kind, .. } => self.compile_literal(kind),
            Expr::Ident { name, .. } => self.compile_ident(name),
            Expr::Binary {
                left, op, right, ..
            } => {
                match op {
                    BinOp::And => {
                        // Short-circuit: if LHS false, skip RHS, result is false
                        self.compile_expr(left);
                        self.emit(Op::Dup, 0);
                        let jump = self.emit(Op::JumpIfFalse(0), 0);
                        self.emit(Op::Pop, 0); // discard LHS true value
                        self.compile_expr(right);
                        let end = self.chunk.current_offset();
                        self.chunk.patch_jump(jump, end);
                    }
                    BinOp::Or => {
                        // Short-circuit: if LHS true, skip RHS, result is true
                        self.compile_expr(left);
                        self.emit(Op::Dup, 0);
                        let jump = self.emit(Op::JumpIfTrue(0), 0);
                        self.emit(Op::Pop, 0); // discard LHS false value
                        self.compile_expr(right);
                        let end = self.chunk.current_offset();
                        self.chunk.patch_jump(jump, end);
                    }
                    _ => {
                        self.compile_expr(left);
                        self.compile_expr(right);
                        self.emit_binop(*op);
                    }
                }
            }
            Expr::Unary { op, operand, .. } => {
                self.compile_expr(operand);
                match op {
                    UnaryOp::Neg => self.emit(Op::Neg, 0),
                    UnaryOp::Not => self.emit(Op::Not, 0),
                    UnaryOp::BitNot => self.emit(Op::BitNot, 0),
                    _ => {
                        // Ref, RefMut, Deref — not supported in VM yet
                        0
                    }
                };
            }
            Expr::Call { callee, args, .. } => {
                self.compile_call(callee, args);
            }
            Expr::MethodCall {
                receiver,
                method,
                args,
                ..
            } => {
                self.compile_method_call(receiver, method, args);
            }
            Expr::Field { object, field, .. } => {
                self.compile_expr(object);
                let name_idx = self.chunk.add_name(field);
                self.emit(Op::GetField(name_idx), 0);
            }
            Expr::Index { object, index, .. } => {
                self.compile_expr(object);
                self.compile_expr(index);
                self.emit(Op::GetIndex, 0);
            }
            Expr::Block { stmts, expr, .. } => {
                self.begin_scope();
                for stmt in stmts {
                    self.compile_stmt(stmt);
                }
                if let Some(tail) = expr {
                    self.compile_expr(tail);
                } else {
                    let idx = self.chunk.add_constant(Value::Null);
                    self.emit(Op::Const(idx), 0);
                }
                self.end_scope();
            }
            Expr::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                self.compile_expr(condition);
                let jump_false = self.emit(Op::JumpIfFalse(0), 0);
                self.compile_expr(then_branch);
                let jump_end = self.emit(Op::Jump(0), 0);
                let else_start = self.chunk.current_offset();
                self.chunk.patch_jump(jump_false, else_start);
                if let Some(else_b) = else_branch {
                    self.compile_expr(else_b);
                } else {
                    let idx = self.chunk.add_constant(Value::Null);
                    self.emit(Op::Const(idx), 0);
                }
                let end = self.chunk.current_offset();
                self.chunk.patch_jump(jump_end, end);
            }
            Expr::While {
                label: _,
                condition,
                body,
                ..
            } => {
                let loop_start = self.chunk.current_offset();
                self.loop_stack.push(LoopContext {
                    continue_target: loop_start,
                    break_patches: Vec::new(),
                    scope_depth: self.scope_depth,
                });

                self.compile_expr(condition);
                let exit_jump = self.emit(Op::JumpIfFalse(0), 0);

                self.compile_expr(body);
                self.emit(Op::Pop, 0); // discard body value

                self.emit(Op::Jump(loop_start as u32), 0);

                let after = self.chunk.current_offset();
                self.chunk.patch_jump(exit_jump, after);

                // Push null as the while expression's value
                let idx = self.chunk.add_constant(Value::Null);
                self.emit(Op::Const(idx), 0);

                let ctx = self
                    .loop_stack
                    .pop()
                    .expect("loop stack empty: while loop has no matching context");
                for patch in ctx.break_patches {
                    self.chunk.patch_jump(patch, after);
                }
            }
            Expr::Loop { label: _, body, .. } => {
                let loop_start = self.chunk.current_offset();
                self.loop_stack.push(LoopContext {
                    continue_target: loop_start,
                    break_patches: Vec::new(),
                    scope_depth: self.scope_depth,
                });

                self.compile_expr(body);
                self.emit(Op::Pop, 0); // discard body value

                self.emit(Op::Jump(loop_start as u32), 0);

                let after = self.chunk.current_offset();
                let ctx = self
                    .loop_stack
                    .pop()
                    .expect("loop stack empty: loop has no matching context");
                for patch in ctx.break_patches {
                    self.chunk.patch_jump(patch, after);
                }
            }
            Expr::For {
                label: _,
                variable,
                iterable,
                body,
                ..
            } => {
                self.compile_for(variable, iterable, body);
            }
            Expr::Match { subject, arms, .. } => {
                self.compile_match(subject, arms);
            }
            Expr::Assign {
                target, op, value, ..
            } => {
                self.compile_assign(target, op, value);
            }
            Expr::Pipe { left, right, .. } => {
                // x |> f  → f(x)
                // Stack needs: [callee, arg] then Call(1)
                self.compile_expr(right); // push callee (the function)
                self.compile_expr(left); // push arg (the value)
                self.emit(Op::Call(1), 0);
            }
            Expr::Array { elements, .. } => {
                for el in elements {
                    self.compile_expr(el);
                }
                self.emit(Op::NewArray(elements.len() as u32), 0);
            }
            Expr::ArrayRepeat { value, count, .. } => {
                // Evaluate count at compile time if it's an int literal;
                // otherwise fall back to emitting a single-element array.
                let n = if let Expr::Literal {
                    kind: LiteralKind::Int(n),
                    ..
                } = count.as_ref()
                {
                    *n as usize
                } else {
                    1
                };
                for _ in 0..n {
                    self.compile_expr(value);
                }
                self.emit(Op::NewArray(n as u32), 0);
            }
            Expr::Tuple { elements, .. } => {
                for el in elements {
                    self.compile_expr(el);
                }
                self.emit(Op::NewTuple(elements.len() as u32), 0);
            }
            Expr::Range {
                start,
                end,
                inclusive,
                ..
            } => {
                // Compile range as an array for now (for-in will consume it)
                self.compile_range(start.as_deref(), end.as_deref(), *inclusive);
            }
            Expr::Cast { expr, ty, .. } => {
                self.compile_expr(expr);
                // Cast is a no-op in the VM for now (runtime types handle it)
                self.compile_cast(ty);
            }
            Expr::Try { expr, .. } => {
                // ? operator — compile expr, let VM handle unwrapping
                self.compile_expr(expr);
            }
            Expr::Closure { params, body, .. } => {
                self.compile_closure(params, body);
            }
            Expr::StructInit { name, fields, .. } => {
                for f in fields {
                    let name_idx = self.chunk.add_name(&f.name);
                    let idx = self.chunk.add_constant(Value::Int(name_idx as i64));
                    self.emit(Op::Const(idx), 0);
                    self.compile_expr(&f.value);
                }
                // Push field count before NewStruct (engine pops it first)
                let fc = self.chunk.add_constant(Value::Int(fields.len() as i64));
                self.emit(Op::Const(fc), 0);
                let name_idx = self.chunk.add_name(name);
                self.emit(Op::NewStruct(name_idx), 0);
            }
            Expr::Grouped { expr, .. } => {
                self.compile_expr(expr);
            }
            Expr::Path { segments, .. } => {
                let full = segments.join("::");
                self.compile_ident(&full);
            }
            Expr::Await { expr, .. } => {
                // Await is not supported in VM mode; compile inner expr
                self.compile_expr(expr);
            }
            Expr::AsyncBlock { body, .. } => {
                // Async block not supported in VM mode; compile body
                self.compile_expr(body);
            }
            Expr::InlineAsm { .. } => {
                // Inline assembly is not supported in VM mode
            }
            Expr::FString { .. } => {
                // F-strings are not supported in VM mode
            }
            Expr::HandleEffect { .. } | Expr::ResumeExpr { .. } => {
                // Effect system is not supported in VM mode
            }
            Expr::Comptime { body, .. } => {
                // In VM mode, comptime blocks are compiled normally.
                self.compile_expr(body);
            }
            Expr::MacroInvocation { .. } => {
                // Macro invocations should be expanded before compilation.
            }
            Expr::Yield { .. } => {
                // Yield is not supported in VM mode.
            }
        }
    }

    fn compile_literal(&mut self, kind: &LiteralKind) {
        let value = match kind {
            LiteralKind::Int(n) => Value::Int(*n),
            LiteralKind::Float(f) => Value::Float(*f),
            LiteralKind::String(s) => Value::Str(s.clone()),
            LiteralKind::RawString(s) => Value::Str(s.clone()),
            LiteralKind::Char(c) => Value::Char(*c),
            LiteralKind::Bool(b) => Value::Bool(*b),
            LiteralKind::Null => Value::Null,
        };
        let idx = self.chunk.add_constant(value);
        self.emit(Op::Const(idx), 0);
    }

    fn compile_ident(&mut self, name: &str) {
        // Check locals first (innermost scope)
        for (i, local) in self.locals.iter().enumerate().rev() {
            if local.name == name {
                self.emit(Op::GetLocal(i as u32), 0);
                return;
            }
        }
        // Global
        let name_idx = self.chunk.add_name(name);
        self.emit(Op::GetGlobal(name_idx), 0);
    }

    fn emit_binop(&mut self, op: BinOp) {
        let instruction = match op {
            BinOp::Add => Op::Add,
            BinOp::Sub => Op::Sub,
            BinOp::Mul => Op::Mul,
            BinOp::Div => Op::Div,
            BinOp::Rem => Op::Rem,
            BinOp::Pow => Op::Pow,
            BinOp::Eq => Op::Eq,
            BinOp::Ne => Op::Ne,
            BinOp::Lt => Op::Lt,
            BinOp::Gt => Op::Gt,
            BinOp::Le => Op::Le,
            BinOp::Ge => Op::Ge,
            BinOp::And | BinOp::Or => unreachable!("handled in compile_expr"),
            BinOp::BitAnd => Op::BitAnd,
            BinOp::BitOr => Op::BitOr,
            BinOp::BitXor => Op::BitXor,
            BinOp::Shl => Op::Shl,
            BinOp::Shr => Op::Shr,
            BinOp::MatMul => Op::Mul, // placeholder
        };
        self.emit(instruction, 0);
    }

    fn compile_call(&mut self, callee: &Expr, args: &[CallArg]) {
        // Check if callee is a known function name
        if let Expr::Ident { name, .. } = callee {
            // Check for builtins: print, println
            if name == "print" {
                for arg in args {
                    self.compile_expr(&arg.value);
                }
                self.emit(Op::Print, 0);
                let idx = self.chunk.add_constant(Value::Null);
                self.emit(Op::Const(idx), 0);
                return;
            }
            if name == "println" {
                for arg in args {
                    self.compile_expr(&arg.value);
                }
                self.emit(Op::Println, 0);
                let idx = self.chunk.add_constant(Value::Null);
                self.emit(Op::Const(idx), 0);
                return;
            }
        }

        // General function call: push callee, push args, Call
        self.compile_expr(callee);
        for arg in args {
            self.compile_expr(&arg.value);
        }
        self.emit(Op::Call(args.len() as u8), 0);
    }

    fn compile_method_call(&mut self, receiver: &Expr, method: &str, args: &[CallArg]) {
        // Compile as: TypeName::method(receiver, args...)
        // For now, compile receiver + args and use a dynamic dispatch
        self.compile_expr(receiver);
        for arg in args {
            self.compile_expr(&arg.value);
        }
        // Push method name and use Call with receiver
        let name_idx = self.chunk.add_name(method);
        self.emit(Op::GetField(name_idx), 0); // Will be handled by VM
    }

    fn compile_assign(&mut self, target: &Expr, op: &AssignOp, value: &Expr) {
        match op {
            AssignOp::Assign => {
                self.compile_expr(value);
            }
            _ => {
                // Compound assignment: load target, compute, store
                self.compile_expr(target);
                self.compile_expr(value);
                match op {
                    AssignOp::AddAssign => self.emit(Op::Add, 0),
                    AssignOp::SubAssign => self.emit(Op::Sub, 0),
                    AssignOp::MulAssign => self.emit(Op::Mul, 0),
                    AssignOp::DivAssign => self.emit(Op::Div, 0),
                    AssignOp::RemAssign => self.emit(Op::Rem, 0),
                    AssignOp::BitAndAssign => self.emit(Op::BitAnd, 0),
                    AssignOp::BitOrAssign => self.emit(Op::BitOr, 0),
                    AssignOp::BitXorAssign => self.emit(Op::BitXor, 0),
                    AssignOp::ShlAssign => self.emit(Op::Shl, 0),
                    AssignOp::ShrAssign => self.emit(Op::Shr, 0),
                    AssignOp::Assign => unreachable!(),
                };
            }
        }

        // Store to target
        match target {
            Expr::Ident { name, .. } => {
                for (i, local) in self.locals.iter().enumerate().rev() {
                    if local.name == *name {
                        self.emit(Op::SetLocal(i as u32), 0);
                        // Push the value back as the expression result
                        self.emit(Op::GetLocal(i as u32), 0);
                        return;
                    }
                }
                let name_idx = self.chunk.add_name(name);
                self.emit(Op::SetGlobal(name_idx), 0);
                self.emit(Op::GetGlobal(name_idx), 0);
            }
            Expr::Index { object, index, .. } => {
                self.compile_expr(object);
                self.compile_expr(index);
                self.emit(Op::SetIndex, 0);
                let idx = self.chunk.add_constant(Value::Null);
                self.emit(Op::Const(idx), 0);
            }
            Expr::Field { object, field, .. } => {
                self.compile_expr(object);
                let name_idx = self.chunk.add_name(field);
                self.emit(Op::SetField(name_idx), 0);
                let idx = self.chunk.add_constant(Value::Null);
                self.emit(Op::Const(idx), 0);
            }
            _ => {
                let idx = self.chunk.add_constant(Value::Null);
                self.emit(Op::Const(idx), 0);
            }
        }
    }

    fn compile_for(&mut self, variable: &str, iterable: &Expr, body: &Expr) {
        // For range expressions, use a simple counter-based loop
        if let Expr::Range {
            start,
            end,
            inclusive,
            ..
        } = iterable
        {
            self.compile_for_range(variable, start.as_deref(), end.as_deref(), *inclusive, body);
            return;
        }

        // For arrays: compile iterable, then iterate via index
        self.compile_expr(iterable);

        self.begin_scope();
        let iter_slot = self.add_local("__iter__");
        self.emit(Op::SetLocal(iter_slot), 0);

        let zero = self.chunk.add_constant(Value::Int(0));
        self.emit(Op::Const(zero), 0);
        let idx_slot = self.add_local("__idx__");
        self.emit(Op::SetLocal(idx_slot), 0);

        let null = self.chunk.add_constant(Value::Null);
        self.emit(Op::Const(null), 0);
        let var_slot = self.add_local(variable);
        self.emit(Op::SetLocal(var_slot), 0);

        let loop_start = self.chunk.current_offset();
        self.loop_stack.push(LoopContext {
            continue_target: loop_start,
            break_patches: Vec::new(),
            scope_depth: self.scope_depth,
        });

        // Check: idx < len(iterable)
        self.emit(Op::GetLocal(idx_slot), 0);
        self.emit(Op::GetLocal(iter_slot), 0);
        let len_name = self.chunk.add_name("__len__");
        self.emit(Op::GetField(len_name), 0);
        self.emit(Op::Lt, 0);
        let exit_jump = self.emit(Op::JumpIfFalse(0), 0);

        // var = iterable[idx]
        self.emit(Op::GetLocal(iter_slot), 0);
        self.emit(Op::GetLocal(idx_slot), 0);
        self.emit(Op::GetIndex, 0);
        self.emit(Op::SetLocal(var_slot), 0);

        self.compile_expr(body);
        self.emit(Op::Pop, 0);

        // idx += 1
        self.emit(Op::GetLocal(idx_slot), 0);
        let one = self.chunk.add_constant(Value::Int(1));
        self.emit(Op::Const(one), 0);
        self.emit(Op::Add, 0);
        self.emit(Op::SetLocal(idx_slot), 0);

        self.emit(Op::Jump(loop_start as u32), 0);

        let after = self.chunk.current_offset();
        self.chunk.patch_jump(exit_jump, after);

        let null2 = self.chunk.add_constant(Value::Null);
        self.emit(Op::Const(null2), 0);

        let ctx = self
            .loop_stack
            .pop()
            .expect("loop stack empty: for-in loop has no matching context");
        for patch in ctx.break_patches {
            self.chunk.patch_jump(patch, after);
        }

        self.end_scope();
    }

    fn compile_for_range(
        &mut self,
        variable: &str,
        start: Option<&Expr>,
        end: Option<&Expr>,
        inclusive: bool,
        body: &Expr,
    ) {
        self.begin_scope();

        // Initialize counter with start value
        if let Some(s) = start {
            self.compile_expr(s);
        } else {
            let zero = self.chunk.add_constant(Value::Int(0));
            self.emit(Op::Const(zero), 0);
        }
        let var_slot = self.add_local(variable);
        self.emit(Op::SetLocal(var_slot), 0);

        // Compile end value into a hidden local
        if let Some(e) = end {
            self.compile_expr(e);
        } else {
            let max = self.chunk.add_constant(Value::Int(i64::MAX));
            self.emit(Op::Const(max), 0);
        }
        let end_slot = self.add_local("__end__");
        self.emit(Op::SetLocal(end_slot), 0);

        let loop_start = self.chunk.current_offset();
        self.loop_stack.push(LoopContext {
            continue_target: loop_start,
            break_patches: Vec::new(),
            scope_depth: self.scope_depth,
        });

        // Check: variable < end (or <= for inclusive)
        self.emit(Op::GetLocal(var_slot), 0);
        self.emit(Op::GetLocal(end_slot), 0);
        if inclusive {
            self.emit(Op::Le, 0);
        } else {
            self.emit(Op::Lt, 0);
        }
        let exit_jump = self.emit(Op::JumpIfFalse(0), 0);

        // Compile body
        self.compile_expr(body);
        self.emit(Op::Pop, 0);

        // variable += 1
        self.emit(Op::GetLocal(var_slot), 0);
        let one = self.chunk.add_constant(Value::Int(1));
        self.emit(Op::Const(one), 0);
        self.emit(Op::Add, 0);
        self.emit(Op::SetLocal(var_slot), 0);

        self.emit(Op::Jump(loop_start as u32), 0);

        let after = self.chunk.current_offset();
        self.chunk.patch_jump(exit_jump, after);

        let null = self.chunk.add_constant(Value::Null);
        self.emit(Op::Const(null), 0);

        let ctx = self
            .loop_stack
            .pop()
            .expect("loop stack empty: for-range loop has no matching context");
        for patch in ctx.break_patches {
            self.chunk.patch_jump(patch, after);
        }

        self.end_scope();
    }

    fn compile_match(&mut self, subject: &Expr, arms: &[MatchArm]) {
        self.compile_expr(subject);

        let mut end_patches = Vec::new();

        for (i, arm) in arms.iter().enumerate() {
            let is_last = i == arms.len() - 1;

            // Duplicate subject for comparison
            self.emit(Op::Dup, 0);

            match &arm.pattern {
                Pattern::Wildcard { .. } | Pattern::Ident { .. } => {
                    // Always matches — if it's an ident, bind it
                    if let Pattern::Ident { name, .. } = &arm.pattern {
                        self.begin_scope();
                        let slot = self.add_local(name);
                        self.emit(Op::SetLocal(slot), 0);
                    } else {
                        self.emit(Op::Pop, 0); // pop the duplicated subject
                    }
                    // Pop original subject
                    // Swap: remove the subject under the dup
                    // Actually for wildcard on last arm, just pop dup and compile body
                    self.emit(Op::Pop, 0); // pop original subject (under the dup we already popped)
                    // Wait, let me think about this more carefully...
                    // Stack: [subject, subject_dup]
                    // Wildcard: pop dup, then pop original, compile body
                    // No — we Dup'd, so stack is [subject, subject_copy]
                    // For wildcard: pop the copy (done above), then we still have original
                    // We need to pop the original before body
                    self.compile_expr(&arm.body);
                    if let Pattern::Ident { .. } = &arm.pattern {
                        self.end_scope();
                    }
                    let end_jump = self.emit(Op::Jump(0), 0);
                    end_patches.push(end_jump);
                }
                Pattern::Literal { kind, .. } => {
                    // Compare with literal
                    self.compile_literal(kind);
                    self.emit(Op::Eq, 0);
                    let next_arm = self.emit(Op::JumpIfFalse(0), 0);

                    self.emit(Op::Pop, 0); // pop subject
                    self.compile_expr(&arm.body);
                    let end_jump = self.emit(Op::Jump(0), 0);
                    end_patches.push(end_jump);

                    let next = self.chunk.current_offset();
                    self.chunk.patch_jump(next_arm, next);

                    if is_last {
                        self.emit(Op::Pop, 0); // pop subject if no match
                        let idx = self.chunk.add_constant(Value::Null);
                        self.emit(Op::Const(idx), 0);
                    }
                }
                _ => {
                    // For complex patterns, just pop and use null
                    self.emit(Op::Pop, 0);
                    if is_last {
                        self.emit(Op::Pop, 0);
                        let idx = self.chunk.add_constant(Value::Null);
                        self.emit(Op::Const(idx), 0);
                    }
                }
            }
        }

        let end = self.chunk.current_offset();
        for patch in end_patches {
            self.chunk.patch_jump(patch, end);
        }
    }

    fn compile_range(&mut self, start: Option<&Expr>, end: Option<&Expr>, _inclusive: bool) {
        // For now, compile range as a pair of values
        // The VM will expand it when used in for-in
        if let Some(s) = start {
            self.compile_expr(s);
        } else {
            let idx = self.chunk.add_constant(Value::Int(0));
            self.emit(Op::Const(idx), 0);
        }
        if let Some(e) = end {
            self.compile_expr(e);
        } else {
            let idx = self.chunk.add_constant(Value::Int(i64::MAX));
            self.emit(Op::Const(idx), 0);
        }
        // Create a range array [start, start+1, ..., end-1]
        // This is handled specially by the VM
        self.emit(Op::NewArray(2), 0); // marker: 2-element array = range bounds
    }

    fn compile_cast(&mut self, _ty: &TypeExpr) {
        // Cast is mostly a type-system concern; at runtime values are dynamic
        // No-op for now
    }

    fn compile_closure(&mut self, params: &[ClosureParam], body: &Expr) {
        // Find free variables: identifiers used in body that are in outer locals
        let param_names: Vec<String> = params.iter().map(|p| p.name.clone()).collect();
        let free_vars = self.find_free_vars(body, &param_names);

        // For each free variable, capture its current value from outer scope
        // We'll pass them as hidden extra arguments
        let captured_slots: Vec<(String, u32)> = free_vars
            .iter()
            .filter_map(|name| {
                // Find in current locals (outer scope)
                self.locals
                    .iter()
                    .enumerate()
                    .rev()
                    .find(|(_, l)| l.name == *name)
                    .map(|(i, _)| (name.clone(), i as u32))
            })
            .collect();

        let name = format!("__closure_{}__", self.chunk.functions.len());
        let func_idx = self.chunk.functions.len();
        self.function_map.insert(name.clone(), func_idx);
        self.chunk.functions.push(FunctionEntry {
            name: name.clone(),
            arity: params.len() as u8,
            local_count: 0,
            code_start: 0,
            code_end: 0,
        });

        let saved_locals = std::mem::take(&mut self.locals);
        let saved_depth = self.scope_depth;
        let saved_in_fn = self.in_function;

        self.scope_depth = 1;
        self.in_function = true;

        // Add params as locals
        for p in params {
            self.locals.push(Local {
                name: p.name.clone(),
                depth: 1,
            });
        }
        // Captured variables are stored as globals with unique names.
        // Map them so compile_ident resolves them as GetGlobal.
        // (They're NOT in self.locals, so compile_ident falls through to global lookup)

        // Jump over the closure body (it's called, not fallen into)
        let skip_jump = self.emit(Op::Jump(0), 0);

        let code_start = self.chunk.current_offset();
        self.compile_expr(body);
        self.emit(Op::Return, 0);
        let code_end = self.chunk.current_offset();
        let local_count = self.locals.len() as u32;

        // Patch the skip jump to after the closure body
        self.chunk.patch_jump(skip_jump, code_end);

        self.chunk.functions[func_idx].code_start = code_start;
        self.chunk.functions[func_idx].code_end = code_end;
        self.chunk.functions[func_idx].local_count = local_count;

        self.locals = saved_locals;
        self.scope_depth = saved_depth;
        self.in_function = saved_in_fn;

        // Emit code to push the closure value
        let fn_val = Value::Function(FnValue {
            name: name.clone(),
            params: params
                .iter()
                .map(|p| Param {
                    name: p.name.clone(),
                    ty: p.ty.clone().unwrap_or(TypeExpr::Simple {
                        name: "any".into(),
                        span: crate::lexer::token::Span::new(0, 0),
                    }),
                    span: p.span,
                })
                .collect(),
            body: Box::new(body.clone()),
            closure_env: {
                use crate::interpreter::env::Environment;
                use std::cell::RefCell;
                use std::rc::Rc;
                Rc::new(RefCell::new(Environment::new()))
            },
            is_async: false,
        });

        // Copy captured locals to globals so the closure body can access them
        for (cap_name, outer_slot) in &captured_slots {
            self.emit(Op::GetLocal(*outer_slot), 0);
            let name_idx = self.chunk.add_name(cap_name);
            self.emit(Op::DefineGlobal(name_idx), 0);
        }
        let idx = self.chunk.add_constant(fn_val);
        self.emit(Op::Const(idx), 0);
    }

    /// Find free variables in an expression (identifiers not in params and not globals).
    fn find_free_vars(&self, expr: &Expr, param_names: &[String]) -> Vec<String> {
        let mut free = Vec::new();
        self.collect_free_vars(expr, param_names, &mut free);
        free.sort();
        free.dedup();
        free
    }

    fn collect_free_vars(&self, expr: &Expr, param_names: &[String], free: &mut Vec<String>) {
        match expr {
            Expr::Ident { name, .. } => {
                // Is it a param? Skip.
                if param_names.contains(name) {
                    return;
                }
                // Is it a local in the current (outer) scope?
                if self.locals.iter().any(|l| l.name == *name) {
                    free.push(name.clone());
                }
                // Otherwise it's a global — don't need to capture
            }
            Expr::Binary { left, right, .. } => {
                self.collect_free_vars(left, param_names, free);
                self.collect_free_vars(right, param_names, free);
            }
            Expr::Unary { operand, .. } => {
                self.collect_free_vars(operand, param_names, free);
            }
            Expr::Call { callee, args, .. } => {
                self.collect_free_vars(callee, param_names, free);
                for arg in args {
                    self.collect_free_vars(&arg.value, param_names, free);
                }
            }
            Expr::Block { stmts, expr, .. } => {
                for stmt in stmts {
                    match stmt {
                        Stmt::Expr { expr, .. } => {
                            self.collect_free_vars(expr, param_names, free);
                        }
                        Stmt::Let { value, .. } => {
                            self.collect_free_vars(value, param_names, free);
                        }
                        _ => {}
                    }
                }
                if let Some(tail) = expr {
                    self.collect_free_vars(tail, param_names, free);
                }
            }
            Expr::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                self.collect_free_vars(condition, param_names, free);
                self.collect_free_vars(then_branch, param_names, free);
                if let Some(eb) = else_branch {
                    self.collect_free_vars(eb, param_names, free);
                }
            }
            Expr::Grouped { expr, .. } => {
                self.collect_free_vars(expr, param_names, free);
            }
            _ => {} // Other expressions: don't recurse for simplicity
        }
    }

    // ── Scope helpers ───────────────────────────────────────────────

    fn begin_scope(&mut self) {
        self.scope_depth += 1;
    }

    fn end_scope(&mut self) {
        self.scope_depth -= 1;
        while let Some(local) = self.locals.last() {
            if local.depth > self.scope_depth {
                self.locals.pop();
            } else {
                break;
            }
        }
    }

    fn add_local(&mut self, name: &str) -> u32 {
        let slot = self.locals.len() as u32;
        self.locals.push(Local {
            name: name.to_string(),
            depth: self.scope_depth,
        });
        slot
    }

    fn emit(&mut self, op: Op, line: u32) -> usize {
        self.chunk.emit(op, line)
    }
}
