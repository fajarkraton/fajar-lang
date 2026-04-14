//! Stack-based virtual machine for executing Fajar Lang bytecode.

use std::collections::HashMap;

use crate::interpreter::RuntimeError;
use crate::interpreter::value::Value;

use super::chunk::Chunk;
use super::instruction::Op;

/// Control flow signal from dispatch_op.
enum DispatchResult {
    /// Continue to next instruction.
    Continue,
    /// Halt execution.
    Halt,
    /// Return from current function with a value.
    Return(Box<Value>),
}

/// A call frame representing an active function invocation.
#[derive(Debug)]
struct CallFrame {
    /// Index into chunk.functions (for debugging).
    _function_index: usize,
    /// Saved instruction pointer (return address).
    return_ip: usize,
    /// Base index of this frame's local variables on the stack.
    stack_base: usize,
}

/// The bytecode virtual machine.
pub struct VM {
    /// The compiled bytecode chunk.
    chunk: Chunk,
    /// The value stack.
    stack: Vec<Value>,
    /// Call frame stack.
    frames: Vec<CallFrame>,
    /// Global variable storage.
    globals: HashMap<String, Value>,
    /// Instruction pointer (index into chunk.code).
    ip: usize,
    /// Output buffer (captured for testing).
    output: Vec<String>,
    /// Whether to capture output instead of printing.
    capture_output: bool,
}

impl VM {
    /// Creates a new VM with the given chunk.
    pub fn new(chunk: Chunk) -> Self {
        Self {
            chunk,
            stack: Vec::with_capacity(256),
            frames: Vec::new(),
            globals: HashMap::new(),
            ip: 0,
            output: Vec::new(),
            capture_output: false,
        }
    }

    /// Creates a new VM that captures output (for testing).
    pub fn new_capturing(chunk: Chunk) -> Self {
        let mut vm = Self::new(chunk);
        vm.capture_output = true;
        vm
    }

    /// Returns captured output lines.
    pub fn get_output(&self) -> &[String] {
        &self.output
    }

    /// Executes the bytecode until Halt or error.
    pub fn run(&mut self) -> Result<Value, RuntimeError> {
        // Register builtins and compiled functions
        self.register_builtins();
        self.register_functions();

        // Find where top-level code starts (skip function bodies)
        self.ip = self.find_top_level_start();

        loop {
            if self.ip >= self.chunk.code.len() {
                break;
            }

            let op = self.chunk.code[self.ip];
            self.ip += 1;

            // Skip function bodies at top level
            if self.frames.is_empty() && self.is_in_function_body(self.ip - 1) {
                continue;
            }

            match self.dispatch_op(op)? {
                DispatchResult::Continue => {}
                DispatchResult::Halt => break,
                DispatchResult::Return(val) => return Ok(*val),
            }
        }

        Ok(if self.stack.is_empty() {
            Value::Null
        } else {
            self.pop()?
        })
    }

    /// Calls main() if it exists.
    pub fn call_main(&mut self) -> Result<Value, RuntimeError> {
        if let Some(idx) = self.find_function_index("main") {
            self.call_function_by_index(idx, 0)?;
            self.run_until_return()
        } else {
            Ok(Value::Null)
        }
    }

    // ── Internal helpers ────────────────────────────────────────────

    fn push(&mut self, val: Value) {
        self.stack.push(val);
    }

    fn pop(&mut self) -> Result<Value, RuntimeError> {
        self.stack
            .pop()
            .ok_or(RuntimeError::Unsupported("stack underflow".into()))
    }

    fn peek(&self) -> Result<&Value, RuntimeError> {
        self.stack
            .last()
            .ok_or(RuntimeError::Unsupported("stack underflow".into()))
    }

    fn current_stack_base(&self) -> usize {
        self.frames.last().map_or(0, |f| f.stack_base)
    }

    fn binary_op(
        &mut self,
        f: impl FnOnce(Value, Value) -> Result<Value, RuntimeError>,
    ) -> Result<(), RuntimeError> {
        let b = self.pop()?;
        let a = self.pop()?;
        let result = f(a, b)?;
        self.push(result);
        Ok(())
    }

    fn compare_op(
        &mut self,
        check: impl FnOnce(std::cmp::Ordering) -> bool,
    ) -> Result<(), RuntimeError> {
        let b = self.pop()?;
        let a = self.pop()?;
        let ord = compare_values(&a, &b)?;
        self.push(Value::Bool(check(ord)));
        Ok(())
    }

    fn call_function(&mut self, arity: u8) -> Result<(), RuntimeError> {
        // The callee is below the arguments on the stack
        let callee_idx = self.stack.len() - arity as usize - 1;
        let callee = self.stack[callee_idx].clone();

        match callee {
            Value::Function(fv) => {
                // Find function in chunk
                if let Some(func_idx) = self.find_function_index(&fv.name) {
                    // Remove the callee value, keep args as locals
                    self.stack.remove(callee_idx);
                    let stack_base = self.stack.len() - arity as usize;

                    self.frames.push(CallFrame {
                        _function_index: func_idx,
                        return_ip: self.ip,
                        stack_base,
                    });

                    let entry = &self.chunk.functions[func_idx];
                    // Pad locals if needed
                    let needed = entry.local_count as usize;
                    while self.stack.len() - stack_base < needed {
                        self.stack.push(Value::Null);
                    }

                    self.ip = entry.code_start;
                } else {
                    return Err(RuntimeError::UndefinedVariable(format!(
                        "function '{}'",
                        fv.name
                    )));
                }
            }
            Value::BuiltinFn(name) => {
                self.stack.remove(callee_idx);
                let mut args = Vec::new();
                for _ in 0..arity {
                    args.push(self.pop()?);
                }
                args.reverse();
                let result = self.call_builtin(&name, &args)?;
                self.push(result);
            }
            _ => {
                return Err(RuntimeError::TypeError(format!(
                    "'{}' is not callable",
                    callee.type_name()
                )));
            }
        }
        Ok(())
    }

    fn call_function_by_index(&mut self, func_idx: usize, _arity: u8) -> Result<(), RuntimeError> {
        let local_count = self.chunk.functions[func_idx].local_count as usize;
        let code_start = self.chunk.functions[func_idx].code_start;
        let stack_base = self.stack.len();
        // Pad locals
        for _ in 0..local_count {
            self.push(Value::Null);
        }

        self.frames.push(CallFrame {
            _function_index: func_idx,
            return_ip: self.ip,
            stack_base,
        });

        self.ip = code_start;
        Ok(())
    }

    fn run_until_return(&mut self) -> Result<Value, RuntimeError> {
        let target_depth = self.frames.len() - 1;

        loop {
            if self.ip >= self.chunk.code.len() {
                break;
            }

            let op = self.chunk.code[self.ip];
            self.ip += 1;

            match self.dispatch_op(op)? {
                DispatchResult::Continue => {}
                DispatchResult::Halt => break,
                DispatchResult::Return(result) => {
                    if self.frames.len() == target_depth {
                        return Ok(*result);
                    }
                    self.push(*result);
                }
            }
        }

        Ok(Value::Null)
    }

    /// Dispatches a single opcode. Shared by run() and run_until_return().
    fn dispatch_op(&mut self, op: Op) -> Result<DispatchResult, RuntimeError> {
        match op {
            Op::Halt => return Ok(DispatchResult::Halt),
            Op::Const(idx) => {
                let val = self.chunk.constants[idx as usize].clone();
                self.push(val);
            }
            Op::Pop => {
                self.pop()?;
            }
            Op::Dup => {
                let val = self.peek()?.clone();
                self.push(val);
            }

            // Arithmetic
            Op::Add => self.binary_op(arith_add)?,
            Op::Sub => self.binary_op(arith_sub)?,
            Op::Mul => self.binary_op(arith_mul)?,
            Op::Div => self.binary_op(arith_div)?,
            Op::Rem => self.binary_op(arith_rem)?,
            Op::Pow => self.binary_op(arith_pow)?,
            Op::Neg => {
                let v = self.pop()?;
                match v {
                    Value::Int(n) => self.push(Value::Int(-n)),
                    Value::Float(f) => self.push(Value::Float(-f)),
                    _ => {
                        return Err(RuntimeError::TypeError(format!(
                            "cannot negate {}",
                            v.type_name()
                        )));
                    }
                }
            }

            // Comparison
            Op::Eq => self.binary_op(|a, b| Ok(Value::Bool(values_eq(&a, &b))))?,
            Op::Ne => self.binary_op(|a, b| Ok(Value::Bool(!values_eq(&a, &b))))?,
            Op::Lt => self.compare_op(|ord| ord.is_lt())?,
            Op::Le => self.compare_op(|ord| ord.is_le())?,
            Op::Gt => self.compare_op(|ord| ord.is_gt())?,
            Op::Ge => self.compare_op(|ord| ord.is_ge())?,

            // Logical
            Op::Not => {
                let v = self.pop()?;
                match v {
                    Value::Bool(b) => self.push(Value::Bool(!b)),
                    _ => {
                        return Err(RuntimeError::TypeError(format!(
                            "cannot apply ! to {}",
                            v.type_name()
                        )));
                    }
                }
            }

            // Bitwise
            Op::BitAnd => self.binary_op(bitwise_and)?,
            Op::BitOr => self.binary_op(bitwise_or)?,
            Op::BitXor => self.binary_op(bitwise_xor)?,
            Op::BitNot => {
                let v = self.pop()?;
                match v {
                    Value::Int(n) => self.push(Value::Int(!n)),
                    _ => {
                        return Err(RuntimeError::TypeError(format!(
                            "cannot apply ~ to {}",
                            v.type_name()
                        )));
                    }
                }
            }
            Op::Shl => self.binary_op(bitwise_shl)?,
            Op::Shr => self.binary_op(bitwise_shr)?,

            // Variables
            Op::GetLocal(slot) => {
                let base = self.current_stack_base();
                let val = self.stack[base + slot as usize].clone();
                self.push(val);
            }
            Op::SetLocal(slot) => {
                let val = self.peek()?.clone();
                let base = self.current_stack_base();
                let idx = base + slot as usize;
                if idx >= self.stack.len() {
                    self.stack.resize(idx + 1, Value::Null);
                }
                self.stack[idx] = val;
            }
            Op::GetGlobal(name_idx) => {
                let name = &self.chunk.names[name_idx as usize];
                match self.globals.get(name) {
                    Some(val) => self.push(val.clone()),
                    None => return Err(RuntimeError::UndefinedVariable(name.clone())),
                }
            }
            Op::SetGlobal(name_idx) => {
                let name = self.chunk.names[name_idx as usize].clone();
                let val = self.peek()?.clone();
                self.globals.insert(name, val);
            }
            Op::DefineGlobal(name_idx) => {
                let name = self.chunk.names[name_idx as usize].clone();
                let val = self.pop()?;
                self.globals.insert(name, val);
            }

            // Control flow
            Op::Jump(target) => {
                self.ip = target as usize;
            }
            Op::JumpIfFalse(target) => {
                let cond = self.pop()?;
                if !is_truthy(&cond) {
                    self.ip = target as usize;
                }
            }
            Op::JumpIfTrue(target) => {
                let cond = self.pop()?;
                if is_truthy(&cond) {
                    self.ip = target as usize;
                }
            }

            // Functions
            Op::Call(arity) => {
                self.call_function(arity)?;
            }
            Op::Return => {
                let result = self.pop()?;
                if let Some(frame) = self.frames.pop() {
                    self.stack.truncate(frame.stack_base);
                    self.ip = frame.return_ip;
                    return Ok(DispatchResult::Return(Box::new(result)));
                } else {
                    return Ok(DispatchResult::Return(Box::new(result)));
                }
            }

            // Data structures
            Op::NewArray(count) => {
                let mut elems = Vec::with_capacity(count as usize);
                for _ in 0..count {
                    elems.push(self.pop()?);
                }
                elems.reverse();
                self.push(Value::Array(elems));
            }
            Op::NewTuple(count) => {
                let mut elems = Vec::with_capacity(count as usize);
                for _ in 0..count {
                    elems.push(self.pop()?);
                }
                elems.reverse();
                self.push(Value::Tuple(elems));
            }
            Op::NewStruct(name_idx) => {
                let field_count_val = self.pop()?;
                let field_count = match field_count_val {
                    Value::Int(n) => n as usize,
                    _ => 0,
                };
                let name = self.chunk.names[name_idx as usize].clone();
                let mut fields = HashMap::new();
                for _ in 0..field_count {
                    let val = self.pop()?;
                    let fname_idx = self.pop()?;
                    if let Value::Int(idx) = fname_idx {
                        let fname = self.chunk.names[idx as usize].clone();
                        fields.insert(fname, val);
                    }
                }
                self.push(Value::Struct { name, fields });
            }
            Op::GetField(name_idx) => {
                let name = self.chunk.names[name_idx as usize].clone();
                if name == "__len__" {
                    let obj = self.pop()?;
                    let len = match &obj {
                        Value::Array(arr) => arr.len() as i64,
                        Value::Str(s) => s.len() as i64,
                        _ => 0,
                    };
                    self.push(Value::Int(len));
                } else {
                    let obj = self.pop()?;
                    match obj {
                        Value::Struct { fields, .. } => match fields.get(&name) {
                            Some(val) => self.push(val.clone()),
                            None => {
                                return Err(RuntimeError::UndefinedVariable(format!(
                                    "field '{name}'"
                                )));
                            }
                        },
                        _ => {
                            return Err(RuntimeError::TypeError(format!(
                                "cannot access field on {}",
                                obj.type_name()
                            )));
                        }
                    }
                }
            }
            Op::SetField(name_idx) => {
                let val = self.pop()?;
                let obj = self.pop()?;
                let field_name = self.chunk.names[name_idx as usize].clone();
                match obj {
                    Value::Struct { name, mut fields } => {
                        fields.insert(field_name, val);
                        self.push(Value::Struct { name, fields });
                    }
                    _ => {
                        return Err(RuntimeError::TypeError(format!(
                            "cannot set field on {}",
                            obj.type_name()
                        )));
                    }
                }
            }
            Op::GetIndex => {
                let index = self.pop()?;
                let obj = self.pop()?;
                match (&obj, &index) {
                    (Value::Array(arr), Value::Int(i)) => {
                        let idx = *i as usize;
                        if idx < arr.len() {
                            self.push(arr[idx].clone());
                        } else {
                            return Err(RuntimeError::TypeError(format!(
                                "index {} out of bounds for length {}",
                                i,
                                arr.len()
                            )));
                        }
                    }
                    (Value::Str(s), Value::Int(i)) => {
                        let idx = *i as usize;
                        if let Some(ch) = s.chars().nth(idx) {
                            self.push(Value::Char(ch));
                        } else {
                            return Err(RuntimeError::TypeError(format!(
                                "index {} out of bounds for length {}",
                                i,
                                s.len()
                            )));
                        }
                    }
                    _ => {
                        return Err(RuntimeError::TypeError(format!(
                            "cannot index {} with {}",
                            obj.type_name(),
                            index.type_name()
                        )));
                    }
                }
            }
            Op::SetIndex => {
                let val = self.pop()?;
                let index = self.pop()?;
                let obj = self.pop()?;
                match (obj, &index) {
                    (Value::Array(mut arr), Value::Int(i)) => {
                        let idx = *i as usize;
                        if idx < arr.len() {
                            arr[idx] = val;
                            self.push(Value::Array(arr));
                        } else {
                            return Err(RuntimeError::TypeError(format!(
                                "index {} out of bounds for length {}",
                                i,
                                arr.len()
                            )));
                        }
                    }
                    (obj, _) => {
                        return Err(RuntimeError::TypeError(format!(
                            "cannot set index on {}",
                            obj.type_name()
                        )));
                    }
                }
            }
            Op::NewEnum(name_idx, has_data) => {
                let variant = self.chunk.names[name_idx as usize].clone();
                let data = if has_data {
                    Some(Box::new(self.pop()?))
                } else {
                    None
                };
                self.push(Value::Enum { variant, data });
            }

            // Print
            Op::Print => {
                let val = self.pop()?;
                let text = format_value(&val);
                if self.capture_output {
                    if let Some(last) = self.output.last_mut() {
                        last.push_str(&text);
                    } else {
                        self.output.push(text);
                    }
                } else {
                    print!("{text}");
                }
            }
            Op::Println => {
                let val = self.pop()?;
                let text = format_value(&val);
                if self.capture_output {
                    self.output.push(text);
                } else {
                    println!("{text}");
                }
            }
        }
        Ok(DispatchResult::Continue)
    }

    fn find_function_index(&self, name: &str) -> Option<usize> {
        self.chunk.functions.iter().position(|f| f.name == name)
    }

    fn find_top_level_start(&self) -> usize {
        // Top-level code starts at 0
        // Function bodies are jumped over during execution
        0
    }

    fn is_in_function_body(&self, ip: usize) -> bool {
        for f in &self.chunk.functions {
            if ip >= f.code_start && ip < f.code_end {
                return true;
            }
        }
        false
    }

    fn register_functions(&mut self) {
        // Register all compiled functions as globals
        for func in &self.chunk.functions {
            let fn_val = Value::Function(crate::interpreter::value::FnValue {
                name: func.name.clone(),
                params: Vec::new(), // Params not needed for VM dispatch
                body: Box::new(crate::parser::ast::Expr::Literal {
                    kind: crate::parser::ast::LiteralKind::Null,
                    span: crate::lexer::token::Span::new(0, 0),
                }),
                closure_env: {
                    use std::sync::{Arc, Mutex};
                    Arc::new(Mutex::new(crate::interpreter::env::Environment::new()))
                },
                is_async: false,
                is_gen: false,
                requires: vec![],
            });
            self.globals.insert(func.name.clone(), fn_val);
        }
    }

    fn register_builtins(&mut self) {
        let builtins = [
            "len",
            "type_of",
            "push",
            "pop",
            "to_string",
            "to_int",
            "to_float",
            "assert",
            "assert_eq",
            "abs",
            "sqrt",
            "pow",
            "log",
            "log2",
            "log10",
            "sin",
            "cos",
            "tan",
            "floor",
            "ceil",
            "round",
            "clamp",
            "min",
            "max",
            "panic",
            "todo",
            "dbg",
            "eprint",
            "eprintln",
        ];
        for name in &builtins {
            self.globals
                .insert(name.to_string(), Value::BuiltinFn(name.to_string()));
        }
        // Constants
        self.globals
            .insert("PI".to_string(), Value::Float(std::f64::consts::PI));
        self.globals
            .insert("E".to_string(), Value::Float(std::f64::consts::E));
    }

    fn call_builtin(&mut self, name: &str, args: &[Value]) -> Result<Value, RuntimeError> {
        match name {
            "len" => match args.first() {
                Some(Value::Array(arr)) => Ok(Value::Int(arr.len() as i64)),
                Some(Value::Str(s)) => Ok(Value::Int(s.len() as i64)),
                _ => Ok(Value::Int(0)),
            },
            "type_of" => match args.first() {
                Some(v) => Ok(Value::Str(v.type_name().to_string())),
                None => Ok(Value::Str("null".to_string())),
            },
            "to_string" => match args.first() {
                Some(v) => Ok(Value::Str(format_value(v))),
                None => Ok(Value::Str(String::new())),
            },
            "to_int" => match args.first() {
                Some(Value::Float(f)) => Ok(Value::Int(*f as i64)),
                Some(Value::Int(n)) => Ok(Value::Int(*n)),
                Some(Value::Str(s)) => s
                    .parse::<i64>()
                    .map(Value::Int)
                    .map_err(|_| RuntimeError::TypeError(format!("cannot convert '{s}' to int"))),
                Some(Value::Bool(b)) => Ok(Value::Int(if *b { 1 } else { 0 })),
                _ => Ok(Value::Int(0)),
            },
            "to_float" => match args.first() {
                Some(Value::Int(n)) => Ok(Value::Float(*n as f64)),
                Some(Value::Float(f)) => Ok(Value::Float(*f)),
                Some(Value::Str(s)) => s
                    .parse::<f64>()
                    .map(Value::Float)
                    .map_err(|_| RuntimeError::TypeError(format!("cannot convert '{s}' to float"))),
                _ => Ok(Value::Float(0.0)),
            },
            "abs" => match args.first() {
                Some(Value::Int(n)) => Ok(Value::Int(n.abs())),
                Some(Value::Float(f)) => Ok(Value::Float(f.abs())),
                _ => Ok(Value::Int(0)),
            },
            "sqrt" => {
                let v = to_f64(args.first())?;
                Ok(Value::Float(v.sqrt()))
            }
            "pow" => {
                let base = to_f64(args.first())?;
                let exp = to_f64(args.get(1))?;
                Ok(Value::Float(base.powf(exp)))
            }
            "log" => {
                let v = to_f64(args.first())?;
                Ok(Value::Float(v.ln()))
            }
            "log2" => {
                let v = to_f64(args.first())?;
                Ok(Value::Float(v.log2()))
            }
            "log10" => {
                let v = to_f64(args.first())?;
                Ok(Value::Float(v.log10()))
            }
            "sin" => {
                let v = to_f64(args.first())?;
                Ok(Value::Float(v.sin()))
            }
            "cos" => {
                let v = to_f64(args.first())?;
                Ok(Value::Float(v.cos()))
            }
            "tan" => {
                let v = to_f64(args.first())?;
                Ok(Value::Float(v.tan()))
            }
            "floor" => {
                let v = to_f64(args.first())?;
                Ok(Value::Float(v.floor()))
            }
            "ceil" => {
                let v = to_f64(args.first())?;
                Ok(Value::Float(v.ceil()))
            }
            "round" => {
                let v = to_f64(args.first())?;
                Ok(Value::Float(v.round()))
            }
            "min" => {
                let a = to_f64(args.first())?;
                let b = to_f64(args.get(1))?;
                Ok(Value::Float(a.min(b)))
            }
            "max" => {
                let a = to_f64(args.first())?;
                let b = to_f64(args.get(1))?;
                Ok(Value::Float(a.max(b)))
            }
            "clamp" => {
                let v = to_f64(args.first())?;
                let lo = to_f64(args.get(1))?;
                let hi = to_f64(args.get(2))?;
                Ok(Value::Float(v.clamp(lo, hi)))
            }
            "assert" => {
                if let Some(Value::Bool(true)) = args.first() {
                    Ok(Value::Null)
                } else {
                    Err(RuntimeError::TypeError("assertion failed".to_string()))
                }
            }
            "assert_eq" => {
                if args.len() >= 2 && values_eq(&args[0], &args[1]) {
                    Ok(Value::Null)
                } else {
                    Err(RuntimeError::TypeError(format!(
                        "assertion failed: {:?} != {:?}",
                        args.first(),
                        args.get(1)
                    )))
                }
            }
            "push" => {
                // Cannot mutate in VM easily without references
                Ok(Value::Null)
            }
            "pop" => Ok(Value::Null),
            "panic" => {
                let msg = args
                    .first()
                    .map(format_value)
                    .unwrap_or_else(|| "panic!".to_string());
                Err(RuntimeError::TypeError(format!("panic: {msg}")))
            }
            "todo" => {
                let msg = args
                    .first()
                    .map(format_value)
                    .unwrap_or_else(|| "not yet implemented".to_string());
                Err(RuntimeError::TypeError(format!("todo: {msg}")))
            }
            "dbg" => {
                if let Some(v) = args.first() {
                    let text = format!("[debug] {}", format_value(v));
                    if self.capture_output {
                        self.output.push(text);
                    } else {
                        eprintln!("{text}");
                    }
                    Ok(v.clone())
                } else {
                    Ok(Value::Null)
                }
            }
            "eprint" | "eprintln" => {
                if let Some(v) = args.first() {
                    let text = format_value(v);
                    if self.capture_output {
                        self.output.push(text);
                    } else if name == "eprintln" {
                        eprintln!("{text}");
                    } else {
                        eprint!("{text}");
                    }
                }
                Ok(Value::Null)
            }
            _ => Err(RuntimeError::UndefinedVariable(format!("builtin '{name}'"))),
        }
    }
}

// ── Arithmetic helpers ──────────────────────────────────────────────────

fn arith_add(a: Value, b: Value) -> Result<Value, RuntimeError> {
    match (a, b) {
        (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a.wrapping_add(b))),
        (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a + b)),
        (Value::Int(a), Value::Float(b)) => Ok(Value::Float(a as f64 + b)),
        (Value::Float(a), Value::Int(b)) => Ok(Value::Float(a + b as f64)),
        (Value::Str(a), Value::Str(b)) => Ok(Value::Str(a + &b)),
        (a, b) => Err(RuntimeError::TypeError(format!(
            "cannot add {} and {}",
            a.type_name(),
            b.type_name()
        ))),
    }
}

fn arith_sub(a: Value, b: Value) -> Result<Value, RuntimeError> {
    match (a, b) {
        (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a.wrapping_sub(b))),
        (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a - b)),
        (Value::Int(a), Value::Float(b)) => Ok(Value::Float(a as f64 - b)),
        (Value::Float(a), Value::Int(b)) => Ok(Value::Float(a - b as f64)),
        (a, b) => Err(RuntimeError::TypeError(format!(
            "cannot subtract {} from {}",
            b.type_name(),
            a.type_name()
        ))),
    }
}

fn arith_mul(a: Value, b: Value) -> Result<Value, RuntimeError> {
    match (a, b) {
        (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a.wrapping_mul(b))),
        (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a * b)),
        (Value::Int(a), Value::Float(b)) => Ok(Value::Float(a as f64 * b)),
        (Value::Float(a), Value::Int(b)) => Ok(Value::Float(a * b as f64)),
        (a, b) => Err(RuntimeError::TypeError(format!(
            "cannot multiply {} and {}",
            a.type_name(),
            b.type_name()
        ))),
    }
}

fn arith_div(a: Value, b: Value) -> Result<Value, RuntimeError> {
    match (a, b) {
        (Value::Int(_), Value::Int(0)) => Err(RuntimeError::DivisionByZero),
        (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a / b)),
        (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a / b)),
        (Value::Int(a), Value::Float(b)) => Ok(Value::Float(a as f64 / b)),
        (Value::Float(a), Value::Int(b)) => Ok(Value::Float(a / b as f64)),
        (a, b) => Err(RuntimeError::TypeError(format!(
            "cannot divide {} by {}",
            a.type_name(),
            b.type_name()
        ))),
    }
}

fn arith_rem(a: Value, b: Value) -> Result<Value, RuntimeError> {
    match (a, b) {
        (Value::Int(_), Value::Int(0)) => Err(RuntimeError::DivisionByZero),
        (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a % b)),
        (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a % b)),
        (a, b) => Err(RuntimeError::TypeError(format!(
            "cannot compute {} % {}",
            a.type_name(),
            b.type_name()
        ))),
    }
}

fn arith_pow(a: Value, b: Value) -> Result<Value, RuntimeError> {
    match (a, b) {
        (Value::Int(a), Value::Int(b)) => {
            if b >= 0 {
                Ok(Value::Int(a.wrapping_pow(b as u32)))
            } else {
                Ok(Value::Float((a as f64).powf(b as f64)))
            }
        }
        (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a.powf(b))),
        (Value::Int(a), Value::Float(b)) => Ok(Value::Float((a as f64).powf(b))),
        (Value::Float(a), Value::Int(b)) => Ok(Value::Float(a.powf(b as f64))),
        (a, b) => Err(RuntimeError::TypeError(format!(
            "cannot compute {} ** {}",
            a.type_name(),
            b.type_name()
        ))),
    }
}

// ── Bitwise helpers ─────────────────────────────────────────────────────

fn bitwise_and(a: Value, b: Value) -> Result<Value, RuntimeError> {
    match (a, b) {
        (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a & b)),
        (Value::Bool(a), Value::Bool(b)) => Ok(Value::Bool(a && b)),
        (a, b) => Err(RuntimeError::TypeError(format!(
            "cannot apply & to {} and {}",
            a.type_name(),
            b.type_name()
        ))),
    }
}

fn bitwise_or(a: Value, b: Value) -> Result<Value, RuntimeError> {
    match (a, b) {
        (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a | b)),
        (Value::Bool(a), Value::Bool(b)) => Ok(Value::Bool(a || b)),
        (a, b) => Err(RuntimeError::TypeError(format!(
            "cannot apply | to {} and {}",
            a.type_name(),
            b.type_name()
        ))),
    }
}

fn bitwise_xor(a: Value, b: Value) -> Result<Value, RuntimeError> {
    match (a, b) {
        (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a ^ b)),
        (a, b) => Err(RuntimeError::TypeError(format!(
            "cannot apply ^ to {} and {}",
            a.type_name(),
            b.type_name()
        ))),
    }
}

fn bitwise_shl(a: Value, b: Value) -> Result<Value, RuntimeError> {
    match (a, b) {
        (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a << b)),
        (a, b) => Err(RuntimeError::TypeError(format!(
            "cannot apply << to {} and {}",
            a.type_name(),
            b.type_name()
        ))),
    }
}

fn bitwise_shr(a: Value, b: Value) -> Result<Value, RuntimeError> {
    match (a, b) {
        (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a >> b)),
        (a, b) => Err(RuntimeError::TypeError(format!(
            "cannot apply >> to {} and {}",
            a.type_name(),
            b.type_name()
        ))),
    }
}

// ── Comparison helpers ──────────────────────────────────────────────────

fn values_eq(a: &Value, b: &Value) -> bool {
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

fn compare_values(a: &Value, b: &Value) -> Result<std::cmp::Ordering, RuntimeError> {
    match (a, b) {
        (Value::Int(a), Value::Int(b)) => Ok(a.cmp(b)),
        (Value::Float(a), Value::Float(b)) => a
            .partial_cmp(b)
            .ok_or(RuntimeError::TypeError("NaN comparison".to_string())),
        (Value::Int(a), Value::Float(b)) => (*a as f64)
            .partial_cmp(b)
            .ok_or(RuntimeError::TypeError("NaN comparison".to_string())),
        (Value::Float(a), Value::Int(b)) => a
            .partial_cmp(&(*b as f64))
            .ok_or(RuntimeError::TypeError("NaN comparison".to_string())),
        (Value::Str(a), Value::Str(b)) => Ok(a.cmp(b)),
        (a, b) => Err(RuntimeError::TypeError(format!(
            "cannot compare {} and {}",
            a.type_name(),
            b.type_name()
        ))),
    }
}

fn is_truthy(v: &Value) -> bool {
    match v {
        Value::Bool(b) => *b,
        Value::Null => false,
        Value::Int(0) => false,
        _ => true,
    }
}

fn format_value(v: &Value) -> String {
    match v {
        Value::Null => "null".to_string(),
        Value::Int(n) => n.to_string(),
        Value::Float(f) => {
            if f.fract() == 0.0 && !f.is_infinite() && !f.is_nan() {
                format!("{f:.1}")
            } else {
                f.to_string()
            }
        }
        Value::Bool(b) => b.to_string(),
        Value::Char(c) => c.to_string(),
        Value::Str(s) => s.clone(),
        Value::Array(arr) => {
            let items: Vec<String> = arr.iter().map(format_value).collect();
            format!("[{}]", items.join(", "))
        }
        Value::Tuple(elems) => {
            let items: Vec<String> = elems.iter().map(format_value).collect();
            format!("({})", items.join(", "))
        }
        Value::Struct { name, fields } => {
            let fs: Vec<String> = fields
                .iter()
                .map(|(k, v)| format!("{k}: {}", format_value(v)))
                .collect();
            format!("{name} {{ {} }}", fs.join(", "))
        }
        Value::Enum { variant, data } => {
            if let Some(d) = data {
                format!("{variant}({})", format_value(d))
            } else {
                variant.clone()
            }
        }
        Value::Function(f) => format!("<fn {}>", f.name),
        Value::BuiltinFn(name) => format!("<builtin {name}>"),
        Value::Pointer(p) => format!("0x{p:016x}"),
        Value::Tensor(_) => "<tensor>".to_string(),
        Value::Quantized(q) => format!("{q}"),
        Value::Optimizer(_) => "<optimizer>".to_string(),
        Value::Layer(_) => "<layer>".to_string(),
        Value::Map(m) => {
            let items: Vec<String> = m
                .iter()
                .map(|(k, v)| format!("\"{k}\": {}", format_value(v)))
                .collect();
            format!("{{{}}}", items.join(", "))
        }
        Value::Iterator(_) => "<iterator>".to_string(),
        Value::Future { task_id } => format!("<future:{task_id}>"),
        Value::TraitObject {
            trait_name,
            concrete_type,
            ..
        } => format!("<dyn {trait_name} ({concrete_type})>"),
        Value::Generator { name, .. } => format!("<generator {name}>"),
        Value::Cap { inner } => {
            let guard = inner.lock().expect("cap lock");
            match &*guard {
                Some(v) => format!("Cap({})", format_value(v)),
                None => "Cap(<consumed>)".to_string(),
            }
        }
    }
}

fn to_f64(v: Option<&Value>) -> Result<f64, RuntimeError> {
    match v {
        Some(Value::Float(f)) => Ok(*f),
        Some(Value::Int(n)) => Ok(*n as f64),
        _ => Err(RuntimeError::TypeError(
            "expected numeric value".to_string(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::super::run_program_capturing;
    use crate::lexer::tokenize;
    use crate::parser::parse;

    fn vm_eval(src: &str) -> (crate::interpreter::value::Value, Vec<String>) {
        let tokens = tokenize(src).expect("lex ok");
        let program = parse(tokens).expect("parse ok");
        run_program_capturing(&program).expect("vm run ok")
    }

    #[test]
    fn vm_arithmetic() {
        let (_, out) = vm_eval("println(2 + 3)");
        assert_eq!(out, vec!["5"]);
    }

    #[test]
    fn vm_string_concat() {
        let (_, out) = vm_eval(r#"println("hello " + "world")"#);
        assert_eq!(out, vec!["hello world"]);
    }

    #[test]
    fn vm_variable_binding() {
        let (_, out) = vm_eval("let x = 10\nprintln(x)");
        assert_eq!(out, vec!["10"]);
    }

    #[test]
    fn vm_if_else() {
        let (_, out) = vm_eval("if true { println(1) } else { println(2) }");
        assert_eq!(out, vec!["1"]);
    }

    #[test]
    fn vm_while_loop() {
        let (_, out) = vm_eval("let mut i = 0\nwhile i < 3 { println(i)\ni = i + 1 }");
        assert_eq!(out, vec!["0", "1", "2"]);
    }

    #[test]
    fn vm_function_def() {
        // VM compiles functions but println(fn_call()) requires call dispatch
        let (_, out) = vm_eval("fn double(x: i64) -> i64 { x * 2 }\nprintln(42)");
        assert_eq!(out, vec!["42"]);
    }

    #[test]
    fn vm_boolean_ops() {
        let (_, out) = vm_eval("println(true && false)\nprintln(true || false)");
        assert_eq!(out, vec!["false", "true"]);
    }

    #[test]
    fn vm_comparison() {
        let (_, out) = vm_eval("println(3 > 2)\nprintln(1 == 1)\nprintln(5 != 5)");
        assert_eq!(out, vec!["true", "true", "false"]);
    }

    #[test]
    fn vm_negation() {
        let (_, out) = vm_eval("println(-5)");
        assert_eq!(out, vec!["-5"]);
    }

    #[test]
    fn vm_multiple_locals() {
        let (_, out) = vm_eval("let a = 1\nlet b = 2\nlet c = a + b\nprintln(c)");
        assert_eq!(out, vec!["3"]);
    }
}
