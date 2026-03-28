//! Built-in function implementations for the Fajar Lang interpreter.
//!
//! Contains `call_builtin()` dispatch and all `builtin_*` implementation functions
//! for OS/HAL, tensor, GPU, timing, file I/O, and FajarOS Phase 3-8 builtins.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;

use crate::interpreter::env::Environment;
use crate::interpreter::value::{FnValue, IteratorValue, LayerValue, OptimizerValue, Value};
use crate::parser::ast::{
    AssignOp, BinOp, Expr, FieldInit, Item, LiteralKind, MatchArm, ModDecl, Pattern, Stmt,
    TypeExpr, UseDecl, UseKind,
};
use crate::runtime::ml::{TensorValue, tensor_ops};

use super::{ControlFlow, EvalError, EvalResult, Interpreter, RuntimeError};

impl Interpreter {
    /// Calls a built-in function.
    pub(crate) fn call_builtin(&mut self, name: &str, args: Vec<Value>) -> EvalResult {
        match name {
            "print" => {
                let text: Vec<String> = args.iter().map(|a| format!("{a}")).collect();
                let output = text.join(" ");
                if self.capture_output {
                    self.output.push(output);
                } else {
                    print!("{output}");
                }
                Ok(Value::Null)
            }
            "println" => {
                let text: Vec<String> = args.iter().map(|a| format!("{a}")).collect();
                let output = text.join(" ");
                if self.capture_output {
                    self.output.push(output);
                } else {
                    println!("{output}");
                }
                Ok(Value::Null)
            }
            "len" => {
                if args.len() != 1 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 1,
                        got: args.len(),
                    }
                    .into());
                }
                match &args[0] {
                    Value::Str(s) => Ok(Value::Int(s.len() as i64)),
                    Value::Array(a) => Ok(Value::Int(a.len() as i64)),
                    Value::Tuple(t) => Ok(Value::Int(t.len() as i64)),
                    Value::Map(m) => Ok(Value::Int(m.len() as i64)),
                    _ => Err(RuntimeError::TypeError(format!(
                        "len() not supported for {}",
                        args[0].type_name()
                    ))
                    .into()),
                }
            }
            "type_of" => {
                if args.len() != 1 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 1,
                        got: args.len(),
                    }
                    .into());
                }
                Ok(Value::Str(args[0].type_name().to_string()))
            }
            "push" => {
                if args.len() != 2 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 2,
                        got: args.len(),
                    }
                    .into());
                }
                match &args[0] {
                    Value::Array(a) => {
                        let mut new_arr = a.clone();
                        new_arr.push(args[1].clone());
                        Ok(Value::Array(new_arr))
                    }
                    _ => Err(RuntimeError::TypeError("push() requires an array".into()).into()),
                }
            }
            "pop" => {
                if args.len() != 1 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 1,
                        got: args.len(),
                    }
                    .into());
                }
                match &args[0] {
                    Value::Array(a) => {
                        if a.is_empty() {
                            Ok(Value::Null)
                        } else {
                            Ok(a.last().cloned().unwrap_or(Value::Null))
                        }
                    }
                    _ => Err(RuntimeError::TypeError("pop() requires an array".into()).into()),
                }
            }
            "to_string" => {
                if args.len() != 1 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 1,
                        got: args.len(),
                    }
                    .into());
                }
                Ok(Value::Str(format!("{}", args[0])))
            }
            "format" => {
                // format("Hello {}, age {}", name, age) → "Hello Alice, age 30"
                if args.is_empty() {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 1,
                        got: 0,
                    }
                    .into());
                }
                let template = match &args[0] {
                    Value::Str(s) => s.clone(),
                    _ => {
                        return Err(RuntimeError::TypeError(
                            "format() first argument must be a string".into(),
                        )
                        .into());
                    }
                };
                let mut result = String::new();
                let mut arg_idx = 1;
                let mut chars = template.chars().peekable();
                while let Some(c) = chars.next() {
                    if c == '{' && chars.peek() == Some(&'}') {
                        chars.next(); // consume '}'
                        if arg_idx < args.len() {
                            result.push_str(&format!("{}", args[arg_idx]));
                            arg_idx += 1;
                        } else {
                            result.push_str("{}");
                        }
                    } else {
                        result.push(c);
                    }
                }
                Ok(Value::Str(result))
            }
            "to_int" => {
                if args.len() != 1 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 1,
                        got: args.len(),
                    }
                    .into());
                }
                match &args[0] {
                    Value::Int(v) => Ok(Value::Int(*v)),
                    Value::Float(v) => Ok(Value::Int(*v as i64)),
                    Value::Str(s) => s.parse::<i64>().map(Value::Int).map_err(|_| {
                        RuntimeError::TypeError(format!("cannot convert '{s}' to int")).into()
                    }),
                    Value::Bool(b) => Ok(Value::Int(if *b { 1 } else { 0 })),
                    _ => Err(RuntimeError::TypeError(format!(
                        "cannot convert {} to int",
                        args[0].type_name()
                    ))
                    .into()),
                }
            }
            "to_float" => {
                if args.len() != 1 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 1,
                        got: args.len(),
                    }
                    .into());
                }
                match &args[0] {
                    Value::Float(v) => Ok(Value::Float(*v)),
                    Value::Int(v) => Ok(Value::Float(*v as f64)),
                    Value::Str(s) => s.parse::<f64>().map(Value::Float).map_err(|_| {
                        RuntimeError::TypeError(format!("cannot convert '{s}' to float")).into()
                    }),
                    _ => Err(RuntimeError::TypeError(format!(
                        "cannot convert {} to float",
                        args[0].type_name()
                    ))
                    .into()),
                }
            }
            "assert" => {
                if args.len() != 1 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 1,
                        got: args.len(),
                    }
                    .into());
                }
                if !args[0].is_truthy() {
                    return Err(RuntimeError::TypeError("assertion failed".into()).into());
                }
                Ok(Value::Null)
            }
            "assert_eq" => {
                if args.len() != 2 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 2,
                        got: args.len(),
                    }
                    .into());
                }
                if args[0] != args[1] {
                    return Err(RuntimeError::TypeError(format!(
                        "assertion failed: {} != {}",
                        args[0], args[1]
                    ))
                    .into());
                }
                Ok(Value::Null)
            }
            // ── Error/debug builtins ──
            "panic" => {
                let msg = if args.is_empty() {
                    "explicit panic".to_string()
                } else {
                    format!("{}", args[0])
                };
                Err(RuntimeError::TypeError(format!("panic: {msg}")).into())
            }
            "todo" => {
                let msg = if args.is_empty() {
                    "not yet implemented".to_string()
                } else {
                    format!("{}", args[0])
                };
                Err(RuntimeError::TypeError(format!("todo: {msg}")).into())
            }
            "dbg" => {
                if args.len() != 1 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 1,
                        got: args.len(),
                    }
                    .into());
                }
                let output = format!("[dbg] {}", args[0]);
                if self.capture_output {
                    self.output.push(output);
                } else {
                    eprintln!("{output}");
                }
                Ok(args.into_iter().next().unwrap_or(Value::Null))
            }
            "eprint" => {
                let text: Vec<String> = args.iter().map(|a| format!("{a}")).collect();
                let output = text.join(" ");
                if self.capture_output {
                    self.output.push(output);
                } else {
                    eprint!("{output}");
                }
                Ok(Value::Null)
            }
            "eprintln" => {
                let text: Vec<String> = args.iter().map(|a| format!("{a}")).collect();
                let output = text.join(" ");
                if self.capture_output {
                    self.output.push(output);
                } else {
                    eprintln!("{output}");
                }
                Ok(Value::Null)
            }
            // ── Math builtins ──
            "abs" => {
                if args.len() != 1 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 1,
                        got: args.len(),
                    }
                    .into());
                }
                match &args[0] {
                    Value::Int(n) => Ok(Value::Int(n.abs())),
                    Value::Float(f) => Ok(Value::Float(f.abs())),
                    _ => Err(RuntimeError::TypeError(format!(
                        "abs() not supported for {}",
                        args[0].type_name()
                    ))
                    .into()),
                }
            }
            "sqrt" => self.math_f64_unary(args, f64::sqrt),
            "log" => self.math_f64_unary(args, f64::ln),
            "log2" => self.math_f64_unary(args, f64::log2),
            "log10" => self.math_f64_unary(args, f64::log10),
            "sin" => self.math_f64_unary(args, f64::sin),
            "cos" => self.math_f64_unary(args, f64::cos),
            "tan" => self.math_f64_unary(args, f64::tan),
            "floor" => self.math_f64_unary(args, f64::floor),
            "ceil" => self.math_f64_unary(args, f64::ceil),
            "round" => self.math_f64_unary(args, f64::round),
            "pow" => self.math_f64_binary(args, f64::powf),
            "clamp" => {
                if args.len() != 3 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 3,
                        got: args.len(),
                    }
                    .into());
                }
                match (&args[0], &args[1], &args[2]) {
                    (Value::Float(v), Value::Float(lo), Value::Float(hi)) => {
                        Ok(Value::Float(v.clamp(*lo, *hi)))
                    }
                    (Value::Int(v), Value::Int(lo), Value::Int(hi)) => {
                        Ok(Value::Int((*v).clamp(*lo, *hi)))
                    }
                    _ => Err(RuntimeError::TypeError(
                        "clamp() requires matching numeric types".into(),
                    )
                    .into()),
                }
            }
            "min" => {
                if args.len() != 2 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 2,
                        got: args.len(),
                    }
                    .into());
                }
                match (&args[0], &args[1]) {
                    (Value::Int(a), Value::Int(b)) => Ok(Value::Int(*a.min(b))),
                    (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a.min(*b))),
                    _ => Err(RuntimeError::TypeError(
                        "min() requires matching numeric types".into(),
                    )
                    .into()),
                }
            }
            "max" => {
                if args.len() != 2 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 2,
                        got: args.len(),
                    }
                    .into());
                }
                match (&args[0], &args[1]) {
                    (Value::Int(a), Value::Int(b)) => Ok(Value::Int(*a.max(b))),
                    (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a.max(*b))),
                    _ => Err(RuntimeError::TypeError(
                        "max() requires matching numeric types".into(),
                    )
                    .into()),
                }
            }
            // ── Integer overflow control builtins ──
            "wrapping_add" => self.int_binop_builtin(args, "wrapping_add", i64::wrapping_add),
            "wrapping_sub" => self.int_binop_builtin(args, "wrapping_sub", i64::wrapping_sub),
            "wrapping_mul" => self.int_binop_builtin(args, "wrapping_mul", i64::wrapping_mul),
            "saturating_add" => self.int_binop_builtin(args, "saturating_add", i64::saturating_add),
            "saturating_sub" => self.int_binop_builtin(args, "saturating_sub", i64::saturating_sub),
            "saturating_mul" => self.int_binop_builtin(args, "saturating_mul", i64::saturating_mul),
            "checked_add" => self.checked_int_builtin(args, "checked_add", i64::checked_add),
            "checked_sub" => self.checked_int_builtin(args, "checked_sub", i64::checked_sub),
            "checked_mul" => self.checked_int_builtin(args, "checked_mul", i64::checked_mul),
            // ── OS runtime builtins ──
            "mem_alloc" => self.builtin_mem_alloc(args),
            "mem_free" => self.builtin_mem_free(args),
            "mem_read_u8" => self.builtin_mem_read_u8(args),
            "mem_read_u32" => self.builtin_mem_read_u32(args),
            "mem_read_u64" => self.builtin_mem_read_u64(args),
            "mem_write_u8" => self.builtin_mem_write_u8(args),
            "mem_write_u32" => self.builtin_mem_write_u32(args),
            "mem_write_u64" => self.builtin_mem_write_u64(args),
            "page_map" => self.builtin_page_map(args),
            "page_unmap" => self.builtin_page_unmap(args),
            "irq_register" => self.builtin_irq_register(args),
            "irq_unregister" => self.builtin_irq_unregister(args),
            "irq_enable" => self.builtin_irq_enable(args),
            "irq_disable" => self.builtin_irq_disable(args),
            "port_read" => self.builtin_port_read(args),
            "port_write" => self.builtin_port_write(args),
            "syscall_define" => self.builtin_syscall_define(args),
            "syscall_dispatch" => self.builtin_syscall_dispatch(args),
            // ML runtime builtins
            "tensor_zeros" | "zeros" => self.builtin_tensor_zeros(args),
            "tensor_ones" | "ones" => self.builtin_tensor_ones(args),
            "tensor_randn" | "tensor_rand" | "randn" => self.builtin_tensor_randn(args),
            "tensor_eye" => self.builtin_tensor_eye(args),
            "tensor_full" => self.builtin_tensor_full(args),
            "tensor_from_data" | "from_data" => self.builtin_tensor_from_data(args),
            "tensor_shape" => self.builtin_tensor_shape(args),
            "tensor_reshape" => self.builtin_tensor_reshape(args),
            "tensor_numel" => self.builtin_tensor_numel(args),
            "tensor_add" => self.builtin_tensor_binop(args, "add"),
            "tensor_sub" => self.builtin_tensor_binop(args, "sub"),
            "tensor_mul" => self.builtin_tensor_binop(args, "mul"),
            "tensor_div" => self.builtin_tensor_binop(args, "div"),
            "tensor_neg" => self.builtin_tensor_neg(args),
            "tensor_matmul" | "matmul" => self.builtin_tensor_matmul(args),
            "tensor_transpose" | "transpose" => self.builtin_tensor_transpose(args),
            "tensor_flatten" | "flatten" => self.builtin_tensor_unary(args, "flatten"),
            "tensor_squeeze" => self.builtin_tensor_squeeze(args),
            "tensor_unsqueeze" => self.builtin_tensor_unsqueeze(args),
            "tensor_sum" => self.builtin_tensor_reduce(args, "sum"),
            "tensor_mean" => self.builtin_tensor_reduce(args, "mean"),
            "tensor_max" => self.builtin_tensor_reduce(args, "max"),
            "tensor_min" => self.builtin_tensor_reduce(args, "min"),
            "tensor_argmax" | "argmax" => self.builtin_tensor_argmax(args),
            "tensor_arange" => self.builtin_tensor_arange(args),
            "tensor_linspace" => self.builtin_tensor_linspace(args),
            "tensor_xavier" | "xavier" => self.builtin_tensor_xavier(args),
            "tensor_free" => Ok(Value::Null), // no-op in interpreter (GC handles cleanup)
            "tensor_rows" => self.builtin_tensor_rows(args),
            "tensor_cols" => self.builtin_tensor_cols(args),
            "tensor_set" => self.builtin_tensor_set(args),
            "tensor_row" => self.builtin_tensor_row(args),
            "tensor_normalize" => self.builtin_tensor_normalize(args),
            "tensor_scale" => self.builtin_tensor_scale(args),
            // Activation functions
            "tensor_relu" | "relu" => self.builtin_tensor_activation(args, "relu"),
            "tensor_sigmoid" | "sigmoid" => self.builtin_tensor_activation(args, "sigmoid"),
            "tensor_tanh" => self.builtin_tensor_activation(args, "tanh"),
            "tensor_softmax" | "softmax" => self.builtin_tensor_activation(args, "softmax"),
            "tensor_gelu" | "gelu" => self.builtin_tensor_activation(args, "gelu"),
            "tensor_leaky_relu" => self.builtin_tensor_leaky_relu(args),
            // Loss functions
            "tensor_mse_loss" | "mse_loss" => self.builtin_tensor_loss(args, "mse"),
            "tensor_cross_entropy" | "cross_entropy_loss" => {
                self.builtin_tensor_loss(args, "cross_entropy")
            }
            "tensor_bce_loss" => self.builtin_tensor_loss(args, "bce"),
            "tensor_l1_loss" => self.builtin_tensor_loss(args, "l1"),
            // ── Autograd builtins ──
            "tensor_backward" => {
                if args.len() != 1 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 1,
                        got: args.len(),
                    }
                    .into());
                }
                match &args[0] {
                    Value::Tensor(t) => {
                        if let Some(tid) = t.id() {
                            let grads = self.tape.backward(tid, t.shape()).map_err(|e| {
                                RuntimeError::TypeError(format!("backward failed: {e}"))
                            })?;
                            self.last_grads = grads;
                        } else {
                            // No tape id — store ones as gradient (seed)
                            let seed = ndarray::ArrayD::ones(t.shape());
                            // Use a placeholder id of 0
                            self.last_grads.clear();
                            self.last_grads.insert(0, seed);
                        }
                        Ok(Value::Null)
                    }
                    _ => Err(
                        RuntimeError::TypeError("tensor_backward requires a tensor".into()).into(),
                    ),
                }
            }
            "tensor_grad" => {
                if args.len() != 1 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 1,
                        got: args.len(),
                    }
                    .into());
                }
                match &args[0] {
                    Value::Tensor(t) => {
                        // Check last_grads by tensor id
                        let found = t.id().and_then(|tid| self.last_grads.get(&tid).cloned());
                        if let Some(grad_data) = found {
                            Ok(Value::Tensor(TensorValue::from_ndarray(grad_data)))
                        } else if let Some(g) = t.grad() {
                            Ok(Value::Tensor(TensorValue::from_ndarray(g.clone())))
                        } else {
                            // Fallback: return any grad available
                            if let Some(g) = self.last_grads.values().next() {
                                Ok(Value::Tensor(TensorValue::from_ndarray(g.clone())))
                            } else {
                                Ok(Value::Null)
                            }
                        }
                    }
                    _ => {
                        Err(RuntimeError::TypeError("tensor_grad requires a tensor".into()).into())
                    }
                }
            }
            "tensor_requires_grad" => {
                if args.len() != 1 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 1,
                        got: args.len(),
                    }
                    .into());
                }
                match &args[0] {
                    Value::Tensor(t) => Ok(Value::Bool(t.requires_grad())),
                    _ => Err(RuntimeError::TypeError(
                        "tensor_requires_grad requires a tensor".into(),
                    )
                    .into()),
                }
            }
            "tensor_set_requires_grad" => {
                if args.len() != 2 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 2,
                        got: args.len(),
                    }
                    .into());
                }
                match (&args[0], &args[1]) {
                    (Value::Tensor(t), Value::Bool(b)) => {
                        let mut t2 = t.clone();
                        t2.set_requires_grad(*b);
                        if *b && t2.id().is_none() {
                            let id = self.tape.fresh_id();
                            t2.set_id(id);
                        }
                        Ok(Value::Tensor(t2))
                    }
                    _ => Err(RuntimeError::TypeError(
                        "tensor_set_requires_grad(tensor, bool)".into(),
                    )
                    .into()),
                }
            }
            "tensor_detach" => {
                if args.len() != 1 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 1,
                        got: args.len(),
                    }
                    .into());
                }
                match &args[0] {
                    Value::Tensor(t) => Ok(Value::Tensor(t.detach())),
                    _ => Err(
                        RuntimeError::TypeError("tensor_detach requires a tensor".into()).into(),
                    ),
                }
            }
            "tensor_no_grad_begin" => {
                self.tape.set_recording(false);
                Ok(Value::Null)
            }
            "tensor_no_grad_end" => {
                self.tape.set_recording(true);
                Ok(Value::Null)
            }
            "tensor_clear_tape" => {
                self.tape.clear();
                Ok(Value::Null)
            }
            // ── Optimizer builtins ──
            "optimizer_sgd" => {
                if args.len() != 2 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 2,
                        got: args.len(),
                    }
                    .into());
                }
                let lr = match &args[0] {
                    Value::Float(f) => *f,
                    Value::Int(n) => *n as f64,
                    _ => {
                        return Err(RuntimeError::TypeError(
                            "optimizer_sgd: lr must be a number".into(),
                        )
                        .into());
                    }
                };
                let momentum = match &args[1] {
                    Value::Float(f) => *f,
                    Value::Int(n) => *n as f64,
                    _ => {
                        return Err(RuntimeError::TypeError(
                            "optimizer_sgd: momentum must be a number".into(),
                        )
                        .into());
                    }
                };
                Ok(Value::Optimizer(OptimizerValue::Sgd(
                    crate::runtime::ml::optim::SGD::new(lr, momentum),
                )))
            }
            "optimizer_adam" => {
                if args.is_empty() {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 1,
                        got: 0,
                    }
                    .into());
                }
                let lr = match &args[0] {
                    Value::Float(f) => *f,
                    Value::Int(n) => *n as f64,
                    _ => {
                        return Err(RuntimeError::TypeError(
                            "optimizer_adam: lr must be a number".into(),
                        )
                        .into());
                    }
                };
                Ok(Value::Optimizer(OptimizerValue::Adam(
                    crate::runtime::ml::optim::Adam::new(lr),
                )))
            }
            "optimizer_step" => {
                if args.len() != 2 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 2,
                        got: args.len(),
                    }
                    .into());
                }
                let mut opt = match args[0].clone() {
                    Value::Optimizer(o) => o,
                    _ => {
                        return Err(RuntimeError::TypeError(
                            "optimizer_step: first arg must be an optimizer".into(),
                        )
                        .into());
                    }
                };
                let mut tensor = match args[1].clone() {
                    Value::Tensor(t) => t,
                    _ => {
                        return Err(RuntimeError::TypeError(
                            "optimizer_step: second arg must be a tensor".into(),
                        )
                        .into());
                    }
                };
                // Apply gradient stored from last backward
                if let Some(tid) = tensor.id() {
                    if let Some(grad_data) = self.last_grads.get(&tid) {
                        tensor.set_grad(grad_data.clone());
                    }
                }
                let mut params = vec![tensor];
                match &mut opt {
                    OptimizerValue::Sgd(sgd) => sgd.step(&mut params),
                    OptimizerValue::Adam(adam) => adam.step(&mut params),
                }
                Ok(Value::Tensor(params.into_iter().next().ok_or_else(
                    || {
                        EvalError::Runtime(RuntimeError::TypeError(
                            "optimizer step returned no parameters".into(),
                        ))
                    },
                )?))
            }
            "optimizer_zero_grad" => {
                if args.len() != 1 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 1,
                        got: args.len(),
                    }
                    .into());
                }
                match &args[0] {
                    Value::Tensor(t) => {
                        let mut t2 = t.clone();
                        t2.zero_grad();
                        Ok(Value::Tensor(t2))
                    }
                    _ => Err(RuntimeError::TypeError(
                        "optimizer_zero_grad requires a tensor".into(),
                    )
                    .into()),
                }
            }
            // ── Model export builtins ──
            "model_save" => {
                // model_save(path, name1, tensor1, name2, tensor2, ...)
                if args.len() < 3 || !(args.len() - 1).is_multiple_of(2) {
                    return Err(RuntimeError::TypeError(
                        "model_save(path, name1, tensor1, name2, tensor2, ...)".into(),
                    )
                    .into());
                }
                let path = match &args[0] {
                    Value::Str(s) => s.clone(),
                    _ => {
                        return Err(RuntimeError::TypeError(
                            "model_save: first arg must be a string path".into(),
                        )
                        .into());
                    }
                };
                let mut named = Vec::new();
                let mut i = 1;
                while i < args.len() {
                    let name = match &args[i] {
                        Value::Str(s) => s.clone(),
                        _ => {
                            return Err(RuntimeError::TypeError(
                                "model_save: name args must be strings".into(),
                            )
                            .into());
                        }
                    };
                    let tensor = match &args[i + 1] {
                        Value::Tensor(t) => t.clone(),
                        _ => {
                            return Err(RuntimeError::TypeError(
                                "model_save: tensor args must be tensors".into(),
                            )
                            .into());
                        }
                    };
                    named.push(crate::runtime::ml::serialize::NamedTensor { name, tensor });
                    i += 2;
                }
                let bytes = crate::runtime::ml::serialize::save(&named);
                match std::fs::write(&path, &bytes) {
                    Ok(()) => Ok(Value::Enum {
                        variant: "Ok".into(),
                        data: Some(Box::new(Value::Int(bytes.len() as i64))),
                    }),
                    Err(e) => Ok(Value::Enum {
                        variant: "Err".into(),
                        data: Some(Box::new(Value::Str(e.to_string()))),
                    }),
                }
            }
            "model_save_quantized" => {
                // model_save_quantized(path, name1, tensor1, name2, tensor2, ...)
                if args.len() < 3 || !(args.len() - 1).is_multiple_of(2) {
                    return Err(RuntimeError::TypeError(
                        "model_save_quantized(path, name1, tensor1, name2, tensor2, ...)".into(),
                    )
                    .into());
                }
                let path = match &args[0] {
                    Value::Str(s) => s.clone(),
                    _ => {
                        return Err(RuntimeError::TypeError(
                            "model_save_quantized: first arg must be a string path".into(),
                        )
                        .into());
                    }
                };
                let mut named = Vec::new();
                let mut i = 1;
                while i < args.len() {
                    let name = match &args[i] {
                        Value::Str(s) => s.clone(),
                        _ => {
                            return Err(RuntimeError::TypeError(
                                "model_save_quantized: name args must be strings".into(),
                            )
                            .into());
                        }
                    };
                    let tensor = match &args[i + 1] {
                        Value::Tensor(t) => t.clone(),
                        _ => {
                            return Err(RuntimeError::TypeError(
                                "model_save_quantized: tensor args must be tensors".into(),
                            )
                            .into());
                        }
                    };
                    let qt = crate::runtime::ml::quantize::QuantizedTensor::quantize(&tensor);
                    named.push(crate::runtime::ml::export::NamedQuantized { name, tensor: qt });
                    i += 2;
                }
                let bytes = crate::runtime::ml::export::export_quantized(&named);
                match std::fs::write(&path, &bytes) {
                    Ok(()) => Ok(Value::Enum {
                        variant: "Ok".into(),
                        data: Some(Box::new(Value::Int(bytes.len() as i64))),
                    }),
                    Err(e) => Ok(Value::Enum {
                        variant: "Err".into(),
                        data: Some(Box::new(Value::Str(e.to_string()))),
                    }),
                }
            }
            // ── Layer builtins ──
            "layer_dense" => {
                if args.len() != 2 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 2,
                        got: args.len(),
                    }
                    .into());
                }
                let in_f = match &args[0] {
                    Value::Int(n) => *n as usize,
                    _ => {
                        return Err(RuntimeError::TypeError(
                            "layer_dense: in_features must be int".into(),
                        )
                        .into());
                    }
                };
                let out_f = match &args[1] {
                    Value::Int(n) => *n as usize,
                    _ => {
                        return Err(RuntimeError::TypeError(
                            "layer_dense: out_features must be int".into(),
                        )
                        .into());
                    }
                };
                Ok(Value::Layer(Box::new(LayerValue::Dense(
                    crate::runtime::ml::layers::Dense::new(in_f, out_f),
                ))))
            }
            "layer_forward" => {
                if args.len() != 2 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 2,
                        got: args.len(),
                    }
                    .into());
                }
                let layer = match &args[0] {
                    Value::Layer(l) => l,
                    _ => {
                        return Err(RuntimeError::TypeError(
                            "layer_forward: first arg must be a layer".into(),
                        )
                        .into());
                    }
                };
                let input = match &args[1] {
                    Value::Tensor(t) => t,
                    _ => {
                        return Err(RuntimeError::TypeError(
                            "layer_forward: second arg must be a tensor".into(),
                        )
                        .into());
                    }
                };
                match layer.as_ref() {
                    LayerValue::Dense(dense) => {
                        let output = dense
                            .forward(input)
                            .map_err(|e| RuntimeError::TypeError(format!("forward failed: {e}")))?;
                        Ok(Value::Tensor(output))
                    }
                }
            }
            "layer_params" => {
                if args.len() != 1 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 1,
                        got: args.len(),
                    }
                    .into());
                }
                match &args[0] {
                    Value::Layer(layer) => match layer.as_ref() {
                        LayerValue::Dense(dense) => {
                            let params: Vec<Value> = dense
                                .parameters()
                                .into_iter()
                                .map(|p| Value::Tensor(p.clone()))
                                .collect();
                            Ok(Value::Array(params))
                        }
                    },
                    _ => {
                        Err(RuntimeError::TypeError("layer_params requires a layer".into()).into())
                    }
                }
            }
            // Metrics builtins
            "metric_accuracy" => {
                if args.len() != 2 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 2,
                        got: args.len(),
                    }
                    .into());
                }
                let preds = self.extract_i64_array(&args[0], "metric_accuracy")?;
                let labels = self.extract_i64_array(&args[1], "metric_accuracy")?;
                Ok(Value::Float(crate::runtime::ml::metrics::accuracy(
                    &preds, &labels,
                )))
            }
            "metric_precision" => {
                if args.len() != 3 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 3,
                        got: args.len(),
                    }
                    .into());
                }
                let preds = self.extract_i64_array(&args[0], "metric_precision")?;
                let labels = self.extract_i64_array(&args[1], "metric_precision")?;
                let class = match &args[2] {
                    Value::Int(n) => *n,
                    _ => return Err(RuntimeError::TypeError("class must be integer".into()).into()),
                };
                Ok(Value::Float(crate::runtime::ml::metrics::precision(
                    &preds, &labels, class,
                )))
            }
            "metric_recall" => {
                if args.len() != 3 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 3,
                        got: args.len(),
                    }
                    .into());
                }
                let preds = self.extract_i64_array(&args[0], "metric_recall")?;
                let labels = self.extract_i64_array(&args[1], "metric_recall")?;
                let class = match &args[2] {
                    Value::Int(n) => *n,
                    _ => return Err(RuntimeError::TypeError("class must be integer".into()).into()),
                };
                Ok(Value::Float(crate::runtime::ml::metrics::recall(
                    &preds, &labels, class,
                )))
            }
            "metric_f1_score" => {
                if args.len() != 3 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 3,
                        got: args.len(),
                    }
                    .into());
                }
                let preds = self.extract_i64_array(&args[0], "metric_f1_score")?;
                let labels = self.extract_i64_array(&args[1], "metric_f1_score")?;
                let class = match &args[2] {
                    Value::Int(n) => *n,
                    _ => return Err(RuntimeError::TypeError("class must be integer".into()).into()),
                };
                Ok(Value::Float(crate::runtime::ml::metrics::f1_score(
                    &preds, &labels, class,
                )))
            }
            // File I/O builtins
            "read_file" => {
                if args.len() != 1 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 1,
                        got: args.len(),
                    }
                    .into());
                }
                match &args[0] {
                    Value::Str(path) => match std::fs::read_to_string(path) {
                        Ok(content) => Ok(Value::Enum {
                            variant: "Ok".into(),
                            data: Some(Box::new(Value::Str(content))),
                        }),
                        Err(e) => Ok(Value::Enum {
                            variant: "Err".into(),
                            data: Some(Box::new(Value::Str(e.to_string()))),
                        }),
                    },
                    _ => Err(
                        RuntimeError::TypeError("read_file() requires a string path".into()).into(),
                    ),
                }
            }
            "write_file" => {
                if args.len() != 2 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 2,
                        got: args.len(),
                    }
                    .into());
                }
                match (&args[0], &args[1]) {
                    (Value::Str(path), Value::Str(content)) => {
                        match std::fs::write(path, content) {
                            Ok(()) => Ok(Value::Enum {
                                variant: "Ok".into(),
                                data: Some(Box::new(Value::Null)),
                            }),
                            Err(e) => Ok(Value::Enum {
                                variant: "Err".into(),
                                data: Some(Box::new(Value::Str(e.to_string()))),
                            }),
                        }
                    }
                    _ => Err(
                        RuntimeError::TypeError("write_file(path: str, content: str)".into())
                            .into(),
                    ),
                }
            }
            "append_file" => {
                if args.len() != 2 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 2,
                        got: args.len(),
                    }
                    .into());
                }
                match (&args[0], &args[1]) {
                    (Value::Str(path), Value::Str(content)) => {
                        use std::io::Write;
                        let result = std::fs::OpenOptions::new()
                            .create(true)
                            .append(true)
                            .open(path)
                            .and_then(|mut f| f.write_all(content.as_bytes()));
                        match result {
                            Ok(()) => Ok(Value::Enum {
                                variant: "Ok".into(),
                                data: Some(Box::new(Value::Null)),
                            }),
                            Err(e) => Ok(Value::Enum {
                                variant: "Err".into(),
                                data: Some(Box::new(Value::Str(e.to_string()))),
                            }),
                        }
                    }
                    _ => Err(RuntimeError::TypeError(
                        "append_file(path: str, content: str)".into(),
                    )
                    .into()),
                }
            }
            "file_exists" => {
                if args.len() != 1 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 1,
                        got: args.len(),
                    }
                    .into());
                }
                match &args[0] {
                    Value::Str(path) => Ok(Value::Bool(std::path::Path::new(path).exists())),
                    _ => Err(RuntimeError::TypeError(
                        "file_exists() requires a string path".into(),
                    )
                    .into()),
                }
            }
            // Collection builtins — HashMap
            "map_new" => {
                if !args.is_empty() {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 0,
                        got: args.len(),
                    }
                    .into());
                }
                Ok(Value::Map(HashMap::new()))
            }
            "map_insert" => {
                if args.len() != 3 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 3,
                        got: args.len(),
                    }
                    .into());
                }
                let mut args = args.into_iter();
                let map_val = args.next().ok_or_else(|| {
                    EvalError::Runtime(RuntimeError::ArityMismatch {
                        expected: 3,
                        got: 0,
                    })
                })?;
                let key_val = args.next().ok_or_else(|| {
                    EvalError::Runtime(RuntimeError::ArityMismatch {
                        expected: 3,
                        got: 1,
                    })
                })?;
                let value = args.next().ok_or_else(|| {
                    EvalError::Runtime(RuntimeError::ArityMismatch {
                        expected: 3,
                        got: 2,
                    })
                })?;
                match (map_val, key_val) {
                    (Value::Map(mut m), Value::Str(k)) => {
                        m.insert(k, value);
                        Ok(Value::Map(m))
                    }
                    _ => Err(
                        RuntimeError::TypeError("map_insert(map, key: str, value)".into()).into(),
                    ),
                }
            }
            "map_get" => {
                if args.len() != 2 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 2,
                        got: args.len(),
                    }
                    .into());
                }
                match (&args[0], &args[1]) {
                    (Value::Map(m), Value::Str(k)) => match m.get(k) {
                        Some(v) => Ok(Value::Enum {
                            variant: "Some".into(),
                            data: Some(Box::new(v.clone())),
                        }),
                        None => Ok(Value::Enum {
                            variant: "None".into(),
                            data: None,
                        }),
                    },
                    _ => Err(RuntimeError::TypeError("map_get(map, key: str)".into()).into()),
                }
            }
            "map_remove" => {
                if args.len() != 2 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 2,
                        got: args.len(),
                    }
                    .into());
                }
                let mut args = args.into_iter();
                let map_val = args.next().ok_or_else(|| {
                    EvalError::Runtime(RuntimeError::ArityMismatch {
                        expected: 2,
                        got: 0,
                    })
                })?;
                let key_val = args.next().ok_or_else(|| {
                    EvalError::Runtime(RuntimeError::ArityMismatch {
                        expected: 2,
                        got: 1,
                    })
                })?;
                match (map_val, key_val) {
                    (Value::Map(mut m), Value::Str(k)) => {
                        m.remove(&k);
                        Ok(Value::Map(m))
                    }
                    _ => Err(RuntimeError::TypeError("map_remove(map, key: str)".into()).into()),
                }
            }
            "map_contains_key" => {
                if args.len() != 2 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 2,
                        got: args.len(),
                    }
                    .into());
                }
                match (&args[0], &args[1]) {
                    (Value::Map(m), Value::Str(k)) => Ok(Value::Bool(m.contains_key(k))),
                    _ => Err(
                        RuntimeError::TypeError("map_contains_key(map, key: str)".into()).into(),
                    ),
                }
            }
            "map_keys" => {
                if args.len() != 1 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 1,
                        got: args.len(),
                    }
                    .into());
                }
                match &args[0] {
                    Value::Map(m) => {
                        let keys: Vec<Value> = m.keys().map(|k| Value::Str(k.clone())).collect();
                        Ok(Value::Array(keys))
                    }
                    _ => Err(RuntimeError::TypeError("map_keys(map)".into()).into()),
                }
            }
            "map_values" => {
                if args.len() != 1 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 1,
                        got: args.len(),
                    }
                    .into());
                }
                match &args[0] {
                    Value::Map(m) => {
                        let vals: Vec<Value> = m.values().cloned().collect();
                        Ok(Value::Array(vals))
                    }
                    _ => Err(RuntimeError::TypeError("map_values(map)".into()).into()),
                }
            }
            "map_len" => {
                if args.len() != 1 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 1,
                        got: args.len(),
                    }
                    .into());
                }
                match &args[0] {
                    Value::Map(m) => Ok(Value::Int(m.len() as i64)),
                    _ => Err(RuntimeError::TypeError("map_len(map)".into()).into()),
                }
            }
            // Option/Result constructors
            "Some" => {
                if args.len() != 1 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 1,
                        got: args.len(),
                    }
                    .into());
                }
                Ok(Value::Enum {
                    variant: "Some".to_string(),
                    data: Some(Box::new(args.into_iter().next().unwrap_or(Value::Null))),
                })
            }
            "Ok" => {
                if args.len() != 1 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 1,
                        got: args.len(),
                    }
                    .into());
                }
                Ok(Value::Enum {
                    variant: "Ok".to_string(),
                    data: Some(Box::new(args.into_iter().next().unwrap_or(Value::Null))),
                })
            }
            "Err" => {
                if args.len() != 1 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 1,
                        got: args.len(),
                    }
                    .into());
                }
                Ok(Value::Enum {
                    variant: "Err".to_string(),
                    data: Some(Box::new(args.into_iter().next().unwrap_or(Value::Null))),
                })
            }
            // Hardware detection builtins (v1.1)
            "hw_cpu_vendor" => {
                let cpu = crate::hw::CpuFeatures::cached();
                Ok(Value::Str(cpu.vendor.to_string()))
            }
            "hw_cpu_arch" => {
                let cpu = crate::hw::CpuFeatures::cached();
                Ok(Value::Str(cpu.arch.clone()))
            }
            "hw_has_avx2" => {
                let cpu = crate::hw::CpuFeatures::cached();
                Ok(Value::Bool(cpu.avx2))
            }
            "hw_has_avx512" => {
                let cpu = crate::hw::CpuFeatures::cached();
                Ok(Value::Bool(cpu.has_avx512()))
            }
            "hw_has_amx" => {
                let cpu = crate::hw::CpuFeatures::cached();
                Ok(Value::Bool(cpu.has_amx()))
            }
            "hw_has_neon" => {
                let cpu = crate::hw::CpuFeatures::cached();
                Ok(Value::Bool(cpu.neon))
            }
            "hw_has_sve" => {
                let cpu = crate::hw::CpuFeatures::cached();
                Ok(Value::Bool(cpu.has_sve()))
            }
            "hw_simd_width" => {
                let cpu = crate::hw::CpuFeatures::cached();
                Ok(Value::Int(cpu.best_simd_width() as i64))
            }
            // Accelerator registry builtins (v1.1 S4)
            "hw_gpu_count" => {
                let profile = crate::hw::HardwareProfile::detect();
                Ok(Value::Int(profile.gpu.devices.len() as i64))
            }
            "hw_npu_count" => {
                let profile = crate::hw::HardwareProfile::detect();
                Ok(Value::Int(profile.npu.devices.len() as i64))
            }
            "hw_best_accelerator" => {
                let profile = crate::hw::HardwareProfile::detect();
                let best = profile.select_best(crate::hw::TaskType::General);
                Ok(Value::Str(best.to_string()))
            }
            // GPIO builtins (v2.0 Q6A)
            "gpio_open" => self.builtin_gpio_open(args),
            "gpio_close" => self.builtin_gpio_close(args),
            "gpio_set_direction" => self.builtin_gpio_set_direction(args),
            "gpio_write" => self.builtin_gpio_write(args),
            "gpio_read" => self.builtin_gpio_read(args),
            "gpio_toggle" => self.builtin_gpio_toggle(args),
            // UART builtins (v2.0 Q6A)
            "uart_open" => self.builtin_uart_open(args),
            "uart_close" => self.builtin_uart_close(args),
            "uart_write_byte" => self.builtin_uart_write_byte(args),
            "uart_read_byte" => self.builtin_uart_read_byte(args),
            "uart_write_str" => self.builtin_uart_write_str(args),
            // PWM builtins (v2.0 Q6A)
            "pwm_open" => self.builtin_pwm_open(args),
            "pwm_close" => self.builtin_pwm_close(args),
            "pwm_set_frequency" => self.builtin_pwm_set_frequency(args),
            "pwm_set_duty" => self.builtin_pwm_set_duty(args),
            "pwm_enable" => self.builtin_pwm_enable(args),
            "pwm_disable" => self.builtin_pwm_disable(args),
            // SPI builtins (v2.0 Q6A)
            "spi_open" => self.builtin_spi_open(args),
            "spi_close" => self.builtin_spi_close(args),
            "spi_transfer" => self.builtin_spi_transfer(args),
            "spi_write" => self.builtin_spi_write(args),
            // NPU builtins (v2.0 Q6A)
            "npu_available" => self.builtin_npu_available(args),
            "npu_info" => self.builtin_npu_info(args),
            "npu_load" => self.builtin_npu_load(args),
            "npu_infer" => self.builtin_npu_infer(args),
            "qnn_quantize" => self.builtin_qnn_quantize(args),
            "qnn_dequantize" => self.builtin_qnn_dequantize(args),
            "qnn_version" => self.builtin_qnn_version(args),
            // Timing builtins (v2.0)
            "delay_ms" => self.builtin_delay_ms(args),
            "delay_us" => self.builtin_delay_us(args),
            // GPU/OpenCL builtins (v2.0 Q6A)
            "gpu_available" => self.builtin_gpu_available(args),
            "gpu_info" => self.builtin_gpu_info(args),
            "gpu_matmul" => self.builtin_gpu_matmul(args),
            "gpu_add" => self.builtin_gpu_add(args),
            "gpu_relu" => self.builtin_gpu_relu(args),
            "gpu_sigmoid" => self.builtin_gpu_sigmoid(args),
            "gpu_mul" => self.builtin_gpu_mul(args),
            "gpu_transpose" => self.builtin_gpu_transpose(args),
            "gpu_sum" => self.builtin_gpu_sum(args),
            // Edge AI / production builtins (v2.0 Q6A)
            "cpu_temp" => self.builtin_cpu_temp(args),
            "cpu_freq" => self.builtin_cpu_freq(args),
            "mem_usage" => self.builtin_mem_usage(args),
            "sys_uptime" => self.builtin_sys_uptime(args),
            "log_to_file" => self.builtin_log_to_file(args),
            // Watchdog / deployment builtins (v2.0 Q6A)
            "watchdog_start" => self.builtin_watchdog_start(args),
            "watchdog_kick" => self.builtin_watchdog_kick(args),
            "watchdog_stop" => self.builtin_watchdog_stop(args),
            "process_id" => Ok(Value::Int(std::process::id() as i64)),
            "sleep_ms" => self.builtin_sleep_ms(args),
            // Cache / file utilities (v2.0 Q6A)
            "cache_set" => self.builtin_cache_set(args),
            "cache_get" => self.builtin_cache_get(args),
            "cache_clear" => {
                self.inference_cache.clear();
                Ok(Value::Null)
            }
            "file_size" => self.builtin_file_size(args),
            "dir_list" => self.builtin_dir_list(args),
            "env_var" => self.builtin_env_var(args),
            // x86_64 port I/O builtins (FajarOS Nova) — simulation stubs
            "port_outb" | "x86_serial_init" => Ok(Value::Int(0)),
            "port_inb" => {
                // Simulate COM1 LSR: TX empty
                if !args.is_empty() {
                    if let Value::Int(port) = &args[0] {
                        if *port == 0x3FD {
                            return Ok(Value::Int(0x60));
                        }
                    }
                }
                Ok(Value::Int(0))
            }
            "set_uart_mode_x86" => Ok(Value::Null),
            // x86_64 CPUID stubs (simulation)
            "cpuid_eax" | "cpuid_ebx" | "cpuid_ecx" | "cpuid_edx" | "read_cr0" | "read_cr4" => {
                Ok(Value::Int(0))
            }
            "sse_enable" => Ok(Value::Null),
            "idt_init" | "pic_remap" | "pic_eoi" | "pit_init" => Ok(Value::Null),
            "read_timer_ticks" => Ok(Value::Int(0)),
            "str_byte_at" => {
                if args.len() >= 2 {
                    if let (Value::Str(s), Value::Int(idx)) = (&args[0], &args[1]) {
                        let i = *idx as usize;
                        if i < s.len() {
                            return Ok(Value::Int(s.as_bytes()[i] as i64));
                        }
                    }
                }
                Ok(Value::Int(0))
            }
            "str_len" => {
                if !args.is_empty() {
                    if let Value::Str(s) = &args[0] {
                        return Ok(Value::Int(s.len() as i64));
                    }
                }
                Ok(Value::Int(0))
            }
            // Process scheduler builtins (Phase 4)
            "proc_table_addr" => Ok(Value::Int(0x600000)),
            "get_current_pid" | "get_proc_count" => Ok(Value::Int(0)),
            "set_current_pid" | "yield_proc" => Ok(Value::Null),
            "proc_create" => Ok(Value::Int(0)),
            "tss_init" | "syscall_init" => Ok(Value::Null),
            "proc_create_user" => Ok(Value::Int(0)),
            "kb_read_scancode" => Ok(Value::Int(-1)),
            "kb_has_data" => Ok(Value::Int(0)),
            "pci_read32" => Ok(Value::Int(0xFFFFFFFF)),
            "pci_write32" => Ok(Value::Null),
            // Volatile read/write u64 — simulation stubs
            "volatile_read_u64" => Ok(Value::Int(0)),
            "volatile_write_u64" => Ok(Value::Null),
            // Buffer read/write — simulation stubs
            "buffer_read_u16_le" | "buffer_read_u32_le" | "buffer_read_u64_le"
            | "buffer_read_u16_be" | "buffer_read_u32_be" | "buffer_read_u64_be" => {
                Ok(Value::Int(0))
            }
            "buffer_write_u16_le"
            | "buffer_write_u32_le"
            | "buffer_write_u64_le"
            | "buffer_write_u16_be"
            | "buffer_write_u32_be"
            | "buffer_write_u64_be" => Ok(Value::Null),
            "acpi_shutdown" => Ok(Value::Null),
            "acpi_find_rsdp" | "acpi_get_cpu_count" => Ok(Value::Int(0)),
            "rdtsc" => Ok(Value::Int(0)),
            // FajarOS Nova v0.2: system builtins
            "hlt" | "cli" | "sti" | "swapgs" | "int_n" | "pause" | "stac" | "clac" => {
                Ok(Value::Null)
            }
            // FajarOS Nova v0.3 Stage A: Extended Port I/O
            "port_inw" | "port_ind" => Ok(Value::Int(0)),
            "port_outw" | "port_outd" => Ok(Value::Null),
            // FajarOS Nova v0.3 Stage A: CPU Control
            "ltr" | "lgdt_mem" | "lidt_mem" => Ok(Value::Null),
            // FajarOS Nova v0.3 Stage A: Buffer Operations
            "memcmp_buf" => Ok(Value::Int(0)),
            "memcpy_buf" | "memset_buf" => Ok(Value::Null),
            // FajarOS Nova v0.2: (legacy line preserved for other builtins)
            "cpuid" | "rdmsr" | "read_msr" => Ok(Value::Int(0)),
            "wrmsr" | "write_msr" => Ok(Value::Int(0)),
            "write_cr4" | "invlpg" | "fxsave" | "fxrstor" => Ok(Value::Null),
            "iretq_to_user" | "rdrand" => Ok(Value::Int(0)),
            // Phase 3 bare-metal HAL builtins (v3.0 FajarOS)
            // Simulation stubs — return 0/Null for interpreter mode without native feature
            "gpio_config" | "gpio_set_output" | "gpio_set_input" | "gpio_set_pull"
            | "gpio_set_irq" | "uart_init" | "uart_available" | "spi_init" | "spi_cs_set"
            | "i2c_init" | "i2c_write" | "i2c_read" | "dma_alloc" | "dma_config" | "dma_start"
            | "dma_wait" | "dma_status" => Ok(Value::Int(0)),
            "timer_get_ticks" => Ok(Value::Int(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_nanos() as i64)
                    .unwrap_or(0),
            )),
            "timer_get_freq" => Ok(Value::Int(1_000_000_000)), // 1 GHz (nanosecond resolution)
            "time_since_boot" => Ok(Value::Int(0)),
            "timer_set_deadline"
            | "timer_enable_virtual"
            | "timer_disable_virtual"
            | "sleep_us"
            | "timer_mark_boot"
            | "dma_free"
            | "dma_barrier" => Ok(Value::Null),
            // Phase 4: Storage stubs
            "nvme_init" | "sd_init" | "nvme_read" | "nvme_write" | "sd_read_block"
            | "sd_write_block" | "vfs_close" | "vfs_stat" | "vfs_read" => Ok(Value::Int(0)),
            "vfs_open" => Ok(Value::Int(3)), // return fd=3
            "vfs_write" => {
                // Return count of bytes "written"
                if args.len() >= 3 {
                    if let Value::Int(n) = &args[2] {
                        return Ok(Value::Int(*n));
                    }
                }
                Ok(Value::Int(0))
            }
            "vfs_mount" => Ok(Value::Int(0)),
            // Phase 5: Network stubs
            "eth_init" => Ok(Value::Int(0)),
            "net_socket" => Ok(Value::Int(0)),
            "net_bind" | "net_listen" | "net_connect" | "net_close" => Ok(Value::Int(0)),
            "net_accept" => Ok(Value::Int(1)), // return new socket
            "net_send" => {
                if args.len() >= 3 {
                    if let Value::Int(n) = &args[2] {
                        return Ok(Value::Int(*n));
                    }
                }
                Ok(Value::Int(0))
            }
            "net_recv" => Ok(Value::Int(0)), // nothing to receive
            // Phase 6: Display & Input stubs
            "fb_init" | "fb_write_pixel" | "fb_fill_rect" | "kb_init" => Ok(Value::Int(0)),
            "fb_width" => Ok(Value::Int(1920)),
            "fb_height" => Ok(Value::Int(1080)),
            "kb_read" | "kb_available" => Ok(Value::Int(0)),
            // Phase 8: OS Services stubs
            "proc_spawn" => Ok(Value::Int(2)),
            "proc_wait" | "proc_kill" => Ok(Value::Int(0)),
            "proc_self" => Ok(Value::Int(1)),
            "sys_cpu_temp" => Ok(Value::Int(45_000)),
            "sys_ram_total" => Ok(Value::Int(8 * 1024 * 1024 * 1024)),
            "sys_ram_free" => Ok(Value::Int(6 * 1024 * 1024 * 1024)),
            "proc_yield" | "sys_poweroff" | "sys_reboot" => Ok(Value::Null),

            // AA2: Async ecosystem builtins
            "join" => {
                // join(future1, future2, ...) → await all, return array of results
                let mut results = Vec::new();
                for arg in args {
                    match arg {
                        Value::Future { task_id } => {
                            if let Some((body, task_env)) = self.async_tasks.remove(&task_id) {
                                let prev_env = self.env.clone();
                                self.env = task_env;
                                let result = self.eval_expr(&body).unwrap_or(Value::Null);
                                self.env = prev_env;
                                results.push(result);
                            } else {
                                results.push(Value::Null);
                            }
                        }
                        other => results.push(other),
                    }
                }
                Ok(Value::Array(results))
            }
            "timeout" => {
                // timeout(ms, future) → resolve future (cooperative, no real timeout in interpreter)
                if args.len() != 2 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 2,
                        got: args.len(),
                    }
                    .into());
                }
                let _ms = &args[0];
                match &args[1] {
                    Value::Future { task_id } => {
                        let tid = *task_id;
                        if let Some((body, task_env)) = self.async_tasks.remove(&tid) {
                            let prev_env = self.env.clone();
                            self.env = task_env;
                            let result = self.eval_expr(&body).unwrap_or(Value::Null);
                            self.env = prev_env;
                            Ok(result)
                        } else {
                            Ok(Value::Null)
                        }
                    }
                    other => Ok(other.clone()),
                }
            }
            "spawn" => {
                // spawn(future) → starts task, returns future (already a future in our model)
                if args.len() != 1 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 1,
                        got: args.len(),
                    }
                    .into());
                }
                Ok(args.into_iter().next().unwrap_or(Value::Null))
            }

            _ => {
                // Check for enum constructor builtins
                if name.starts_with("__enum_") {
                    if args.len() == 1 {
                        let variant_name = name.rsplit('_').next().unwrap_or(name);
                        return Ok(Value::Enum {
                            variant: variant_name.to_string(),
                            data: Some(Box::new(args.into_iter().next().unwrap_or(Value::Null))),
                        });
                    }
                    // Multiple args — wrap in tuple
                    let variant_name = name.rsplit('_').next().unwrap_or(name);
                    return Ok(Value::Enum {
                        variant: variant_name.to_string(),
                        data: Some(Box::new(Value::Tuple(args))),
                    });
                }
                // TQ12.1: HTTP server builtin
                if name == "http_listen" {
                    return self.builtin_http_listen(args);
                }

                // TQ12.2: Database builtins
                if name == "db_open" {
                    return self.builtin_db_open(args);
                }
                if name == "db_execute" {
                    return self.builtin_db_execute(args);
                }
                if name == "db_query" {
                    return self.builtin_db_query(args);
                }
                if name == "db_close" {
                    return self.builtin_db_close(args);
                }
                if name == "db_begin" {
                    return self.builtin_db_begin(args);
                }
                if name == "db_commit" {
                    return self.builtin_db_commit(args);
                }
                if name == "db_rollback" {
                    return self.builtin_db_rollback(args);
                }

                // WebSocket builtins
                if name == "ws_connect" {
                    return self.builtin_ws_connect(args);
                }
                if name == "ws_send" {
                    return self.builtin_ws_send(args);
                }
                if name == "ws_recv" {
                    return self.builtin_ws_recv(args);
                }
                if name == "ws_close" {
                    return self.builtin_ws_close(args);
                }

                // MQTT builtins
                if name == "mqtt_connect" {
                    return self.builtin_mqtt_connect(args);
                }
                if name == "mqtt_publish" {
                    return self.builtin_mqtt_publish(args);
                }
                if name == "mqtt_subscribe" {
                    return self.builtin_mqtt_subscribe(args);
                }
                if name == "mqtt_recv" {
                    return self.builtin_mqtt_recv(args);
                }
                if name == "mqtt_disconnect" {
                    return self.builtin_mqtt_disconnect(args);
                }
                if name == "ble_scan" {
                    return self.builtin_ble_scan(args);
                }
                if name == "ble_connect" {
                    return self.builtin_ble_connect(args);
                }
                if name == "ble_read" {
                    return self.builtin_ble_read(args);
                }
                if name == "ble_write" {
                    return self.builtin_ble_write(args);
                }
                if name == "ble_disconnect" {
                    return self.builtin_ble_disconnect(args);
                }

                Err(RuntimeError::Unsupported(format!("unknown builtin '{name}'")).into())
            }
        }
    }

    /// TQ12.1: Start an HTTP server that calls a Fajar handler function.
    /// Usage: http_listen(port, max_requests)
    /// Listens on 127.0.0.1:port, accepts max_requests connections,
    /// returns the number of requests served.
    fn builtin_http_listen(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() < 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into());
        }
        let port = match &args[0] {
            Value::Int(p) => *p as u16,
            _ => {
                return Err(
                    RuntimeError::TypeError("http_listen: port must be integer".into()).into(),
                );
            }
        };
        let max_requests = match &args[1] {
            Value::Int(n) => *n as usize,
            _ => {
                return Err(RuntimeError::TypeError(
                    "http_listen: max_requests must be integer".into(),
                )
                .into());
            }
        };

        let addr = format!("127.0.0.1:{port}");
        let listener = std::net::TcpListener::bind(&addr)
            .map_err(|e| RuntimeError::TypeError(format!("http_listen: bind {addr}: {e}")))?;

        println!("[http] Listening on {addr} (max {max_requests} requests)");
        let mut served = 0i64;

        for stream in listener.incoming().take(max_requests) {
            match stream {
                Ok(mut stream) => {
                    use std::io::{BufRead, BufReader, Write};
                    let mut reader = BufReader::new(&stream);
                    let mut request_line = String::new();
                    let _ = reader.read_line(&mut request_line);
                    let parts: Vec<&str> = request_line.trim().splitn(3, ' ').collect();
                    let method = parts.first().unwrap_or(&"GET");
                    let path = parts.get(1).unwrap_or(&"/");

                    // Drain headers
                    loop {
                        let mut line = String::new();
                        let _ = reader.read_line(&mut line);
                        if line.trim().is_empty() {
                            break;
                        }
                    }

                    // Simple response: serve 200 with method + path as JSON
                    let body = format!(
                        r#"{{"method":"{}","path":"{}","served":{}}}"#,
                        method, path, served
                    );
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        body.len(),
                        body
                    );
                    let _ = stream.write_all(response.as_bytes());
                    served += 1;
                }
                Err(_) => break,
            }
        }

        println!("[http] Served {served} requests");
        Ok(Value::Int(served))
    }

    // ═══════════════════════════════════════════════════════════════════
    // TQ12.2: SQLite database builtins
    // ═══════════════════════════════════════════════════════════════════

    /// `db_open(path)` → Int handle.
    /// Opens a SQLite database. Use ":memory:" for in-memory databases.
    fn builtin_db_open(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        let path = match &args[0] {
            Value::Str(s) => s.clone(),
            _ => {
                return Err(RuntimeError::TypeError("db_open: path must be string".into()).into());
            }
        };
        let handle = self
            .db_manager
            .open(&path)
            .map_err(RuntimeError::TypeError)?;
        Ok(Value::Int(handle))
    }

    /// `db_execute(handle, sql)` or `db_execute(handle, sql, params_array)` → Int rows_changed.
    fn builtin_db_execute(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() < 2 || args.len() > 3 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into());
        }
        let handle = match &args[0] {
            Value::Int(h) => *h,
            _ => {
                return Err(
                    RuntimeError::TypeError("db_execute: handle must be integer".into()).into(),
                );
            }
        };
        let sql = match &args[1] {
            Value::Str(s) => s.clone(),
            _ => {
                return Err(
                    RuntimeError::TypeError("db_execute: sql must be string".into()).into(),
                );
            }
        };
        let params = if args.len() == 3 {
            Self::value_to_db_params(&args[2])?
        } else {
            vec![]
        };
        let changed = self
            .db_manager
            .execute(handle, &sql, &params)
            .map_err(RuntimeError::TypeError)?;
        Ok(Value::Int(changed))
    }

    /// `db_query(handle, sql)` or `db_query(handle, sql, params_array)` → Array of Maps.
    fn builtin_db_query(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() < 2 || args.len() > 3 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into());
        }
        let handle = match &args[0] {
            Value::Int(h) => *h,
            _ => {
                return Err(
                    RuntimeError::TypeError("db_query: handle must be integer".into()).into(),
                );
            }
        };
        let sql = match &args[1] {
            Value::Str(s) => s.clone(),
            _ => {
                return Err(RuntimeError::TypeError("db_query: sql must be string".into()).into());
            }
        };
        let params = if args.len() == 3 {
            Self::value_to_db_params(&args[2])?
        } else {
            vec![]
        };
        let rows = self
            .db_manager
            .query(handle, &sql, &params)
            .map_err(RuntimeError::TypeError)?;

        // Convert Vec<HashMap<String, DbValue>> → Value::Array(Vec<Value::Map>)
        let result: Vec<Value> = rows
            .into_iter()
            .map(|row| {
                let map: std::collections::HashMap<String, Value> = row
                    .into_iter()
                    .map(|(k, v)| {
                        let val = match v {
                            crate::stdlib_v3::database::DbValue::Int(n) => Value::Int(n),
                            crate::stdlib_v3::database::DbValue::Float(f) => Value::Float(f),
                            crate::stdlib_v3::database::DbValue::Text(s) => Value::Str(s),
                            crate::stdlib_v3::database::DbValue::Null => Value::Null,
                        };
                        (k, val)
                    })
                    .collect();
                Value::Map(map)
            })
            .collect();
        Ok(Value::Array(result))
    }

    /// `db_close(handle)` → Null.
    fn builtin_db_close(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        let handle = match &args[0] {
            Value::Int(h) => *h,
            _ => {
                return Err(
                    RuntimeError::TypeError("db_close: handle must be integer".into()).into(),
                );
            }
        };
        self.db_manager
            .close(handle)
            .map_err(RuntimeError::TypeError)?;
        Ok(Value::Null)
    }

    /// `db_begin(handle)` → Null. Begin a transaction.
    fn builtin_db_begin(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        let handle = match &args[0] {
            Value::Int(h) => *h,
            _ => {
                return Err(
                    RuntimeError::TypeError("db_begin: handle must be integer".into()).into(),
                );
            }
        };
        self.db_manager
            .begin(handle)
            .map_err(RuntimeError::TypeError)?;
        Ok(Value::Null)
    }

    /// `db_commit(handle)` → Null. Commit the current transaction.
    fn builtin_db_commit(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        let handle = match &args[0] {
            Value::Int(h) => *h,
            _ => {
                return Err(
                    RuntimeError::TypeError("db_commit: handle must be integer".into()).into(),
                );
            }
        };
        self.db_manager
            .commit(handle)
            .map_err(RuntimeError::TypeError)?;
        Ok(Value::Null)
    }

    /// `db_rollback(handle)` → Null. Rollback the current transaction.
    fn builtin_db_rollback(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        let handle = match &args[0] {
            Value::Int(h) => *h,
            _ => {
                return Err(
                    RuntimeError::TypeError("db_rollback: handle must be integer".into()).into(),
                );
            }
        };
        self.db_manager
            .rollback(handle)
            .map_err(RuntimeError::TypeError)?;
        Ok(Value::Null)
    }

    /// Convert a Value::Array of params to Vec<DbParam>.
    fn value_to_db_params(
        val: &Value,
    ) -> Result<Vec<crate::stdlib_v3::database::DbParam>, EvalError> {
        use crate::stdlib_v3::database::DbParam;
        match val {
            Value::Array(arr) => {
                let mut params = Vec::with_capacity(arr.len());
                for v in arr {
                    let p = match v {
                        Value::Int(n) => DbParam::Int(*n),
                        Value::Float(f) => DbParam::Float(*f),
                        Value::Str(s) => DbParam::Text(s.clone()),
                        Value::Null => DbParam::Null,
                        _ => {
                            return Err(RuntimeError::TypeError(
                                "db params: each element must be int, float, string, or null"
                                    .into(),
                            )
                            .into());
                        }
                    };
                    params.push(p);
                }
                Ok(params)
            }
            _ => Err(
                RuntimeError::TypeError("db params: third argument must be an array".into()).into(),
            ),
        }
    }

    // ═══════════════════════════════════════════════════════════════════
    // OS runtime builtins
    // ═══════════════════════════════════════════════════════════════════

    /// `mem_alloc(size, align)` → Pointer
    fn builtin_mem_alloc(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into());
        }
        let size = match &args[0] {
            Value::Int(n) => *n as usize,
            _ => return Err(RuntimeError::TypeError("mem_alloc: size must be int".into()).into()),
        };
        let align = match &args[1] {
            Value::Int(n) => *n as usize,
            _ => return Err(RuntimeError::TypeError("mem_alloc: align must be int".into()).into()),
        };
        match self.os.memory.alloc(size, align) {
            Ok(addr) => Ok(Value::Pointer(addr.0)),
            Err(e) => Err(RuntimeError::TypeError(e.to_string()).into()),
        }
    }

    /// `mem_free(ptr)` → null
    fn builtin_mem_free(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        let addr = match &args[0] {
            Value::Pointer(a) => crate::runtime::os::VirtAddr(*a),
            Value::Int(n) => crate::runtime::os::VirtAddr(*n as u64),
            _ => return Err(RuntimeError::TypeError("mem_free: expected pointer".into()).into()),
        };
        match self.os.memory.free(addr) {
            Ok(()) => Ok(Value::Null),
            Err(e) => Err(RuntimeError::TypeError(e.to_string()).into()),
        }
    }

    /// `mem_read_u8(ptr)` → Int
    fn builtin_mem_read_u8(&self, args: Vec<Value>) -> EvalResult {
        let addr = self.extract_addr("mem_read_u8", &args, 1)?;
        match self.os.memory.read_u8(addr) {
            Ok(v) => Ok(Value::Int(v as i64)),
            Err(e) => Err(RuntimeError::TypeError(e.to_string()).into()),
        }
    }

    /// `mem_read_u32(ptr)` → Int
    fn builtin_mem_read_u32(&self, args: Vec<Value>) -> EvalResult {
        let addr = self.extract_addr("mem_read_u32", &args, 1)?;
        match self.os.memory.read_u32(addr) {
            Ok(v) => Ok(Value::Int(v as i64)),
            Err(e) => Err(RuntimeError::TypeError(e.to_string()).into()),
        }
    }

    /// `mem_read_u64(ptr)` → Int
    fn builtin_mem_read_u64(&self, args: Vec<Value>) -> EvalResult {
        let addr = self.extract_addr("mem_read_u64", &args, 1)?;
        match self.os.memory.read_u64(addr) {
            Ok(v) => Ok(Value::Int(v as i64)),
            Err(e) => Err(RuntimeError::TypeError(e.to_string()).into()),
        }
    }

    /// `mem_write_u8(ptr, value)` → null
    fn builtin_mem_write_u8(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into());
        }
        let addr = self.val_to_addr("mem_write_u8", &args[0])?;
        let val = match &args[1] {
            Value::Int(n) => *n as u8,
            _ => {
                return Err(
                    RuntimeError::TypeError("mem_write_u8: value must be int".into()).into(),
                );
            }
        };
        match self.os.memory.write_u8(addr, val) {
            Ok(()) => Ok(Value::Null),
            Err(e) => Err(RuntimeError::TypeError(e.to_string()).into()),
        }
    }

    /// `mem_write_u32(ptr, value)` → null
    fn builtin_mem_write_u32(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into());
        }
        let addr = self.val_to_addr("mem_write_u32", &args[0])?;
        let val = match &args[1] {
            Value::Int(n) => *n as u32,
            _ => {
                return Err(
                    RuntimeError::TypeError("mem_write_u32: value must be int".into()).into(),
                );
            }
        };
        match self.os.memory.write_u32(addr, val) {
            Ok(()) => Ok(Value::Null),
            Err(e) => Err(RuntimeError::TypeError(e.to_string()).into()),
        }
    }

    /// `mem_write_u64(ptr, value)` → null
    fn builtin_mem_write_u64(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into());
        }
        let addr = self.val_to_addr("mem_write_u64", &args[0])?;
        let val = match &args[1] {
            Value::Int(n) => *n as u64,
            _ => {
                return Err(
                    RuntimeError::TypeError("mem_write_u64: value must be int".into()).into(),
                );
            }
        };
        match self.os.memory.write_u64(addr, val) {
            Ok(()) => Ok(Value::Null),
            Err(e) => Err(RuntimeError::TypeError(e.to_string()).into()),
        }
    }

    /// `page_map(virt_addr, phys_addr, flags)` → null
    fn builtin_page_map(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 3 {
            return Err(RuntimeError::ArityMismatch {
                expected: 3,
                got: args.len(),
            }
            .into());
        }
        let va = self.val_to_addr("page_map", &args[0])?;
        let pa = match &args[1] {
            Value::Pointer(a) => crate::runtime::os::PhysAddr(*a),
            Value::Int(a) => crate::runtime::os::PhysAddr(*a as u64),
            _ => {
                return Err(RuntimeError::TypeError(
                    "page_map: phys_addr must be int/pointer".into(),
                )
                .into());
            }
        };
        let flags_val = match &args[2] {
            Value::Int(n) => *n as u8,
            _ => return Err(RuntimeError::TypeError("page_map: flags must be int".into()).into()),
        };
        let flags = crate::runtime::os::PageFlags::from_bits(flags_val);
        match self.os.memory.page_table.map_page(va, pa, flags) {
            Ok(()) => Ok(Value::Null),
            Err(e) => Err(RuntimeError::TypeError(e.to_string()).into()),
        }
    }

    /// `page_unmap(virt_addr)` → null
    fn builtin_page_unmap(&mut self, args: Vec<Value>) -> EvalResult {
        let addr = self.extract_addr("page_unmap", &args, 1)?;
        match self.os.memory.page_table.unmap_page(addr) {
            Ok(()) => Ok(Value::Null),
            Err(e) => Err(RuntimeError::TypeError(e.to_string()).into()),
        }
    }

    /// `irq_register(num, handler_name)` → null
    fn builtin_irq_register(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into());
        }
        let irq_num = match &args[0] {
            Value::Int(n) => *n as u8,
            _ => {
                return Err(
                    RuntimeError::TypeError("irq_register: irq num must be int".into()).into(),
                );
            }
        };
        let handler = match &args[1] {
            Value::Str(s) => s.clone(),
            _ => {
                return Err(
                    RuntimeError::TypeError("irq_register: handler must be string".into()).into(),
                );
            }
        };
        match self.os.irq.register(irq_num, handler) {
            Ok(()) => Ok(Value::Null),
            Err(e) => Err(RuntimeError::TypeError(e.to_string()).into()),
        }
    }

    /// `irq_unregister(num)` → null
    fn builtin_irq_unregister(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        let irq_num = match &args[0] {
            Value::Int(n) => *n as u8,
            _ => {
                return Err(
                    RuntimeError::TypeError("irq_unregister: irq num must be int".into()).into(),
                );
            }
        };
        match self.os.irq.unregister(irq_num) {
            Ok(()) => Ok(Value::Null),
            Err(e) => Err(RuntimeError::TypeError(e.to_string()).into()),
        }
    }

    /// `irq_enable()` → null
    fn builtin_irq_enable(&mut self, args: Vec<Value>) -> EvalResult {
        if !args.is_empty() {
            return Err(RuntimeError::ArityMismatch {
                expected: 0,
                got: args.len(),
            }
            .into());
        }
        self.os.irq.enable();
        Ok(Value::Null)
    }

    /// `irq_disable()` → null
    fn builtin_irq_disable(&mut self, args: Vec<Value>) -> EvalResult {
        if !args.is_empty() {
            return Err(RuntimeError::ArityMismatch {
                expected: 0,
                got: args.len(),
            }
            .into());
        }
        self.os.irq.disable();
        Ok(Value::Null)
    }

    /// `port_read(port)` → Int
    fn builtin_port_read(&self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        let port = match &args[0] {
            Value::Int(n) => *n as u16,
            _ => return Err(RuntimeError::TypeError("port_read: port must be int".into()).into()),
        };
        Ok(Value::Int(self.os.port_io.read(port) as i64))
    }

    /// `port_write(port, value)` → null
    fn builtin_port_write(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into());
        }
        let port = match &args[0] {
            Value::Int(n) => *n as u16,
            _ => return Err(RuntimeError::TypeError("port_write: port must be int".into()).into()),
        };
        let value = match &args[1] {
            Value::Int(n) => *n as u8,
            _ => {
                return Err(RuntimeError::TypeError("port_write: value must be int".into()).into());
            }
        };
        self.os.port_io.write(port, value);
        Ok(Value::Null)
    }

    /// `syscall_define(num, handler_name, arg_count)` → null
    fn builtin_syscall_define(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 3 {
            return Err(RuntimeError::ArityMismatch {
                expected: 3,
                got: args.len(),
            }
            .into());
        }
        let num = match &args[0] {
            Value::Int(n) => *n as u64,
            _ => {
                return Err(
                    RuntimeError::TypeError("syscall_define: num must be int".into()).into(),
                );
            }
        };
        let handler_name = match &args[1] {
            Value::Str(s) => s.clone(),
            _ => {
                return Err(RuntimeError::TypeError(
                    "syscall_define: handler must be string".into(),
                )
                .into());
            }
        };
        let arg_count = match &args[2] {
            Value::Int(n) => *n as usize,
            _ => {
                return Err(RuntimeError::TypeError(
                    "syscall_define: arg_count must be int".into(),
                )
                .into());
            }
        };
        match self.os.syscall.define(num, handler_name, arg_count) {
            Ok(()) => Ok(Value::Null),
            Err(e) => Err(RuntimeError::TypeError(e.to_string()).into()),
        }
    }

    /// `syscall_dispatch(num, ...args)` → handler name (string) for the interpreter to resolve
    fn builtin_syscall_dispatch(&mut self, args: Vec<Value>) -> EvalResult {
        if args.is_empty() {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: 0,
            }
            .into());
        }
        let num = match &args[0] {
            Value::Int(n) => *n as u64,
            _ => {
                return Err(
                    RuntimeError::TypeError("syscall_dispatch: num must be int".into()).into(),
                );
            }
        };
        let syscall_args = args.len() - 1;
        match self.os.syscall.dispatch(num, syscall_args) {
            Ok(handler) => Ok(Value::Str(handler.name.clone())),
            Err(e) => Err(RuntimeError::TypeError(e.to_string()).into()),
        }
    }

    // ── GPIO builtins (v2.0 Q6A) ──

    /// `gpio_open(pin: i64) -> i64` — Open a GPIO pin; returns pin handle (the pin number).
    /// On x86_64 host, operates in simulation mode.
    fn builtin_gpio_open(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        let pin = match &args[0] {
            Value::Int(n) => *n,
            _ => return Err(RuntimeError::TypeError("gpio_open: pin must be int".into()).into()),
        };
        // Store pin state in gpio_pins map: pin -> (direction: 0=in/1=out, level: 0/1)
        self.gpio_pins.insert(pin, (0, 0));
        Ok(Value::Int(pin))
    }

    /// `gpio_close(pin: i64) -> null` — Close/release a GPIO pin.
    fn builtin_gpio_close(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        let pin = match &args[0] {
            Value::Int(n) => *n,
            _ => return Err(RuntimeError::TypeError("gpio_close: pin must be int".into()).into()),
        };
        self.gpio_pins.remove(&pin);
        Ok(Value::Null)
    }

    /// `gpio_set_direction(pin: i64, dir: str) -> null` — Set pin direction ("in" or "out").
    fn builtin_gpio_set_direction(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into());
        }
        let pin = match &args[0] {
            Value::Int(n) => *n,
            _ => {
                return Err(
                    RuntimeError::TypeError("gpio_set_direction: pin must be int".into()).into(),
                );
            }
        };
        let dir = match &args[1] {
            Value::Str(s) => s.clone(),
            _ => {
                return Err(RuntimeError::TypeError(
                    "gpio_set_direction: direction must be string (\"in\" or \"out\")".into(),
                )
                .into());
            }
        };
        let dir_val = match dir.as_str() {
            "in" | "input" => 0,
            "out" | "output" => 1,
            _ => {
                return Err(RuntimeError::TypeError(format!(
                    "gpio_set_direction: invalid direction '{}' (use \"in\" or \"out\")",
                    dir
                ))
                .into());
            }
        };
        if let Some(state) = self.gpio_pins.get_mut(&pin) {
            state.0 = dir_val;
        } else {
            return Err(RuntimeError::TypeError(format!(
                "gpio_set_direction: pin {} not opened",
                pin
            ))
            .into());
        }
        Ok(Value::Null)
    }

    /// `gpio_write(pin: i64, level: i64) -> null` — Write 0 (low) or 1 (high) to an output pin.
    fn builtin_gpio_write(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into());
        }
        let pin = match &args[0] {
            Value::Int(n) => *n,
            _ => return Err(RuntimeError::TypeError("gpio_write: pin must be int".into()).into()),
        };
        let level = match &args[1] {
            Value::Int(n) => {
                if *n != 0 {
                    1
                } else {
                    0
                }
            }
            Value::Bool(b) => {
                if *b {
                    1
                } else {
                    0
                }
            }
            _ => {
                return Err(RuntimeError::TypeError(
                    "gpio_write: level must be int or bool".into(),
                )
                .into());
            }
        };
        if let Some(state) = self.gpio_pins.get_mut(&pin) {
            if state.0 != 1 {
                return Err(RuntimeError::TypeError(format!(
                    "gpio_write: pin {} is not set to output",
                    pin
                ))
                .into());
            }
            state.1 = level;
        } else {
            return Err(
                RuntimeError::TypeError(format!("gpio_write: pin {} not opened", pin)).into(),
            );
        }
        Ok(Value::Null)
    }

    /// `gpio_read(pin: i64) -> i64` — Read current pin level (0 or 1).
    fn builtin_gpio_read(&self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        let pin = match &args[0] {
            Value::Int(n) => *n,
            _ => return Err(RuntimeError::TypeError("gpio_read: pin must be int".into()).into()),
        };
        if let Some(state) = self.gpio_pins.get(&pin) {
            Ok(Value::Int(state.1))
        } else {
            Err(RuntimeError::TypeError(format!("gpio_read: pin {} not opened", pin)).into())
        }
    }

    /// `gpio_toggle(pin: i64) -> null` — Toggle output pin level (0→1 or 1→0).
    fn builtin_gpio_toggle(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        let pin = match &args[0] {
            Value::Int(n) => *n,
            _ => return Err(RuntimeError::TypeError("gpio_toggle: pin must be int".into()).into()),
        };
        if let Some(state) = self.gpio_pins.get_mut(&pin) {
            if state.0 != 1 {
                return Err(RuntimeError::TypeError(format!(
                    "gpio_toggle: pin {} is not set to output",
                    pin
                ))
                .into());
            }
            state.1 = if state.1 == 0 { 1 } else { 0 };
        } else {
            return Err(
                RuntimeError::TypeError(format!("gpio_toggle: pin {} not opened", pin)).into(),
            );
        }
        Ok(Value::Null)
    }

    // ── UART builtins (v2.0 Q6A) ──

    /// `uart_open(port: i64, baud: i64) -> i64` — Open UART port; returns port handle.
    fn builtin_uart_open(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into());
        }
        let port = match &args[0] {
            Value::Int(n) => *n,
            _ => return Err(RuntimeError::TypeError("uart_open: port must be int".into()).into()),
        };
        let baud = match &args[1] {
            Value::Int(n) => *n,
            _ => return Err(RuntimeError::TypeError("uart_open: baud must be int".into()).into()),
        };
        // Store UART state: port -> (baud, tx_buffer)
        self.uart_ports.insert(port, (baud, Vec::new()));
        Ok(Value::Int(port))
    }

    /// `uart_close(port: i64) -> null` — Close UART port.
    fn builtin_uart_close(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        let port = match &args[0] {
            Value::Int(n) => *n,
            _ => return Err(RuntimeError::TypeError("uart_close: port must be int".into()).into()),
        };
        self.uart_ports.remove(&port);
        Ok(Value::Null)
    }

    /// `uart_write_byte(port: i64, byte: i64) -> null` — Write a byte to UART.
    fn builtin_uart_write_byte(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into());
        }
        let port = match &args[0] {
            Value::Int(n) => *n,
            _ => {
                return Err(
                    RuntimeError::TypeError("uart_write_byte: port must be int".into()).into(),
                );
            }
        };
        let byte = match &args[1] {
            Value::Int(n) => *n as u8,
            _ => {
                return Err(
                    RuntimeError::TypeError("uart_write_byte: byte must be int".into()).into(),
                );
            }
        };
        if let Some(state) = self.uart_ports.get_mut(&port) {
            state.1.push(byte);
        } else {
            return Err(RuntimeError::TypeError(format!(
                "uart_write_byte: port {} not opened",
                port
            ))
            .into());
        }
        Ok(Value::Null)
    }

    /// `uart_read_byte(port: i64) -> i64` — Read a byte from UART TX buffer (simulation: reads back written bytes).
    fn builtin_uart_read_byte(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        let port = match &args[0] {
            Value::Int(n) => *n,
            _ => {
                return Err(
                    RuntimeError::TypeError("uart_read_byte: port must be int".into()).into(),
                );
            }
        };
        if let Some(state) = self.uart_ports.get_mut(&port) {
            if state.1.is_empty() {
                Ok(Value::Int(-1)) // No data available
            } else {
                let byte = state.1.remove(0); // FIFO
                Ok(Value::Int(byte as i64))
            }
        } else {
            Err(RuntimeError::TypeError(format!("uart_read_byte: port {} not opened", port)).into())
        }
    }

    /// `uart_write_str(port: i64, s: str) -> null` — Write string bytes to UART.
    fn builtin_uart_write_str(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into());
        }
        let port = match &args[0] {
            Value::Int(n) => *n,
            _ => {
                return Err(
                    RuntimeError::TypeError("uart_write_str: port must be int".into()).into(),
                );
            }
        };
        let s = match &args[1] {
            Value::Str(s) => s.clone(),
            _ => {
                return Err(
                    RuntimeError::TypeError("uart_write_str: data must be string".into()).into(),
                );
            }
        };
        if let Some(state) = self.uart_ports.get_mut(&port) {
            state.1.extend_from_slice(s.as_bytes());
        } else {
            return Err(RuntimeError::TypeError(format!(
                "uart_write_str: port {} not opened",
                port
            ))
            .into());
        }
        Ok(Value::Null)
    }

    // ── Timing builtins (v2.0) ──

    /// `delay_ms(ms: i64) -> null` — Sleep for the given number of milliseconds.
    fn builtin_delay_ms(&self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        let ms = match &args[0] {
            Value::Int(n) => *n,
            _ => {
                return Err(
                    RuntimeError::TypeError("delay_ms: argument must be int".into()).into(),
                );
            }
        };
        if ms > 0 {
            std::thread::sleep(std::time::Duration::from_millis(ms as u64));
        }
        Ok(Value::Null)
    }

    /// `delay_us(us: i64) -> null` — Sleep for the given number of microseconds.
    fn builtin_delay_us(&self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        let us = match &args[0] {
            Value::Int(n) => *n,
            _ => {
                return Err(
                    RuntimeError::TypeError("delay_us: argument must be int".into()).into(),
                );
            }
        };
        if us > 0 {
            std::thread::sleep(std::time::Duration::from_micros(us as u64));
        }
        Ok(Value::Null)
    }

    // ── PWM builtins (v2.0 Q6A) ──

    /// `pwm_open(channel: i64) -> i64` — Open PWM channel; returns handle.
    fn builtin_pwm_open(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        let ch = match &args[0] {
            Value::Int(n) => *n,
            _ => {
                return Err(RuntimeError::TypeError("pwm_open: channel must be int".into()).into());
            }
        };
        // Store PWM state: channel -> (frequency_hz, duty_percent, enabled)
        self.pwm_channels.insert(ch, (1000, 0, false));
        Ok(Value::Int(ch))
    }

    /// `pwm_close(channel: i64) -> null`
    fn builtin_pwm_close(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        let ch = match &args[0] {
            Value::Int(n) => *n,
            _ => {
                return Err(
                    RuntimeError::TypeError("pwm_close: channel must be int".into()).into(),
                );
            }
        };
        self.pwm_channels.remove(&ch);
        Ok(Value::Null)
    }

    /// `pwm_set_frequency(channel: i64, hz: i64) -> null`
    fn builtin_pwm_set_frequency(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into());
        }
        let ch = match &args[0] {
            Value::Int(n) => *n,
            _ => {
                return Err(RuntimeError::TypeError(
                    "pwm_set_frequency: channel must be int".into(),
                )
                .into());
            }
        };
        let hz = match &args[1] {
            Value::Int(n) => *n,
            _ => {
                return Err(
                    RuntimeError::TypeError("pwm_set_frequency: hz must be int".into()).into(),
                );
            }
        };
        if let Some(state) = self.pwm_channels.get_mut(&ch) {
            state.0 = hz;
        } else {
            return Err(RuntimeError::TypeError(format!(
                "pwm_set_frequency: channel {} not opened",
                ch
            ))
            .into());
        }
        Ok(Value::Null)
    }

    /// `pwm_set_duty(channel: i64, percent: i64) -> null` — Set duty cycle (0-100).
    fn builtin_pwm_set_duty(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into());
        }
        let ch = match &args[0] {
            Value::Int(n) => *n,
            _ => {
                return Err(
                    RuntimeError::TypeError("pwm_set_duty: channel must be int".into()).into(),
                );
            }
        };
        let duty = match &args[1] {
            Value::Int(n) => *n,
            _ => {
                return Err(
                    RuntimeError::TypeError("pwm_set_duty: percent must be int".into()).into(),
                );
            }
        };
        if !(0..=100).contains(&duty) {
            return Err(RuntimeError::TypeError(format!(
                "pwm_set_duty: percent must be 0-100, got {}",
                duty
            ))
            .into());
        }
        if let Some(state) = self.pwm_channels.get_mut(&ch) {
            state.1 = duty;
        } else {
            return Err(RuntimeError::TypeError(format!(
                "pwm_set_duty: channel {} not opened",
                ch
            ))
            .into());
        }
        Ok(Value::Null)
    }

    /// `pwm_enable(channel: i64) -> null`
    fn builtin_pwm_enable(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        let ch = match &args[0] {
            Value::Int(n) => *n,
            _ => {
                return Err(
                    RuntimeError::TypeError("pwm_enable: channel must be int".into()).into(),
                );
            }
        };
        if let Some(state) = self.pwm_channels.get_mut(&ch) {
            state.2 = true;
        } else {
            return Err(
                RuntimeError::TypeError(format!("pwm_enable: channel {} not opened", ch)).into(),
            );
        }
        Ok(Value::Null)
    }

    /// `pwm_disable(channel: i64) -> null`
    fn builtin_pwm_disable(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        let ch = match &args[0] {
            Value::Int(n) => *n,
            _ => {
                return Err(
                    RuntimeError::TypeError("pwm_disable: channel must be int".into()).into(),
                );
            }
        };
        if let Some(state) = self.pwm_channels.get_mut(&ch) {
            state.2 = false;
        } else {
            return Err(
                RuntimeError::TypeError(format!("pwm_disable: channel {} not opened", ch)).into(),
            );
        }
        Ok(Value::Null)
    }

    // ── SPI builtins (v2.0 Q6A) ──

    /// `spi_open(bus: i64, speed_hz: i64) -> i64` — Open SPI bus; returns handle.
    fn builtin_spi_open(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into());
        }
        let bus = match &args[0] {
            Value::Int(n) => *n,
            _ => return Err(RuntimeError::TypeError("spi_open: bus must be int".into()).into()),
        };
        let speed = match &args[1] {
            Value::Int(n) => *n,
            _ => return Err(RuntimeError::TypeError("spi_open: speed must be int".into()).into()),
        };
        // Store SPI state: bus -> (speed_hz, rx_buffer)
        self.spi_buses.insert(bus, (speed, Vec::new()));
        Ok(Value::Int(bus))
    }

    /// `spi_close(bus: i64) -> null`
    fn builtin_spi_close(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        let bus = match &args[0] {
            Value::Int(n) => *n,
            _ => return Err(RuntimeError::TypeError("spi_close: bus must be int".into()).into()),
        };
        self.spi_buses.remove(&bus);
        Ok(Value::Null)
    }

    /// `spi_transfer(bus: i64, byte: i64) -> i64` — Full-duplex: send byte, receive byte.
    fn builtin_spi_transfer(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into());
        }
        let bus = match &args[0] {
            Value::Int(n) => *n,
            _ => {
                return Err(RuntimeError::TypeError("spi_transfer: bus must be int".into()).into());
            }
        };
        let byte = match &args[1] {
            Value::Int(n) => *n as u8,
            _ => {
                return Err(
                    RuntimeError::TypeError("spi_transfer: byte must be int".into()).into(),
                );
            }
        };
        if let Some(state) = self.spi_buses.get_mut(&bus) {
            // Simulation: loopback — received byte = sent byte (MOSI→MISO)
            state.1.push(byte);
            Ok(Value::Int(byte as i64))
        } else {
            Err(RuntimeError::TypeError(format!("spi_transfer: bus {} not opened", bus)).into())
        }
    }

    /// `spi_write(bus: i64, data: str) -> null` — Write string bytes to SPI.
    fn builtin_spi_write(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into());
        }
        let bus = match &args[0] {
            Value::Int(n) => *n,
            _ => return Err(RuntimeError::TypeError("spi_write: bus must be int".into()).into()),
        };
        let data = match &args[1] {
            Value::Str(s) => s.clone(),
            _ => {
                return Err(
                    RuntimeError::TypeError("spi_write: data must be string".into()).into(),
                );
            }
        };
        if let Some(state) = self.spi_buses.get_mut(&bus) {
            state.1.extend_from_slice(data.as_bytes());
        } else {
            return Err(
                RuntimeError::TypeError(format!("spi_write: bus {} not opened", bus)).into(),
            );
        }
        Ok(Value::Null)
    }

    // ── NPU builtins (v2.0 Q6A) ──

    /// `npu_available() -> bool` — Check if NPU (Hexagon 770) is available.
    fn builtin_npu_available(&self, args: Vec<Value>) -> EvalResult {
        if !args.is_empty() {
            return Err(RuntimeError::ArityMismatch {
                expected: 0,
                got: args.len(),
            }
            .into());
        }
        // Check for FastRPC device node (real hardware detection)
        let available = std::path::Path::new("/dev/fastrpc-cdsp").exists();
        Ok(Value::Bool(available))
    }

    /// `npu_info() -> str` — Return NPU info string.
    fn builtin_npu_info(&self, args: Vec<Value>) -> EvalResult {
        if !args.is_empty() {
            return Err(RuntimeError::ArityMismatch {
                expected: 0,
                got: args.len(),
            }
            .into());
        }
        let available = std::path::Path::new("/dev/fastrpc-cdsp").exists();
        if available {
            Ok(Value::Str("Hexagon 770 V68, 12 TOPS INT8, QNN SDK".into()))
        } else {
            Ok(Value::Str("NPU not available (simulation mode)".into()))
        }
    }

    /// `qnn_version() -> str` — Detect QNN SDK version from installed packages.
    fn builtin_qnn_version(&self, args: Vec<Value>) -> EvalResult {
        if !args.is_empty() {
            return Err(RuntimeError::ArityMismatch {
                expected: 0,
                got: args.len(),
            }
            .into());
        }
        // Try to detect QNN version from libqnn1 package
        let output = std::process::Command::new("dpkg-query")
            .args(["--showformat=${Version}", "-W", "libqnn1"])
            .output();
        match output {
            Ok(out) if out.status.success() => {
                let version = String::from_utf8_lossy(&out.stdout).to_string();
                Ok(Value::Str(format!("QNN {version}")))
            }
            _ => {
                // Fallback: check if libQnnHtp.so exists
                if std::path::Path::new("/usr/lib/libQnnHtp.so").exists() {
                    Ok(Value::Str("QNN (version unknown)".into()))
                } else {
                    Ok(Value::Str("QNN not installed".into()))
                }
            }
        }
    }

    /// `npu_load(path: str) -> i64` — Load NPU model; returns model handle.
    fn builtin_npu_load(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        let path = match &args[0] {
            Value::Str(s) => s.clone(),
            _ => {
                return Err(RuntimeError::TypeError("npu_load: path must be string".into()).into());
            }
        };
        // Simulation: assign incrementing model ID
        let model_id = self.npu_models.len() as i64 + 1;
        self.npu_models.insert(model_id, path);
        Ok(Value::Int(model_id))
    }

    /// `npu_infer(model: i64, input_data: i64) -> i64` — Run inference; returns result class index.
    fn builtin_npu_infer(&self, args: Vec<Value>) -> EvalResult {
        if args.len() != 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into());
        }
        let model_id = match &args[0] {
            Value::Int(n) => *n,
            _ => {
                return Err(
                    RuntimeError::TypeError("npu_infer: model must be int handle".into()).into(),
                );
            }
        };
        let _input = match &args[1] {
            Value::Int(n) => *n,
            v => {
                return Err(RuntimeError::TypeError(format!(
                    "npu_infer: input must be int, got {:?}",
                    v
                ))
                .into());
            }
        };
        if !self.npu_models.contains_key(&model_id) {
            return Err(RuntimeError::TypeError(format!(
                "npu_infer: model {} not loaded",
                model_id
            ))
            .into());
        }
        // Simulation: return class 0 (placeholder for real QNN inference)
        Ok(Value::Int(0))
    }

    /// `qnn_quantize(tensor: Tensor, dtype: str) -> i64` — Quantize tensor to QNN buffer; returns handle.
    ///
    /// Supported dtypes: "uint8", "int8", "f32", "f16", "bf16".
    fn builtin_qnn_quantize(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into());
        }
        let tensor = match &args[0] {
            Value::Tensor(t) => t.clone(),
            v => {
                return Err(RuntimeError::TypeError(format!(
                    "qnn_quantize: first argument must be Tensor, got {:?}",
                    v
                ))
                .into());
            }
        };
        let dtype_str = match &args[1] {
            Value::Str(s) => s.clone(),
            v => {
                return Err(RuntimeError::TypeError(format!(
                    "qnn_quantize: second argument must be string dtype, got {:?}",
                    v
                ))
                .into());
            }
        };
        let dtype = match dtype_str.as_str() {
            "uint8" => crate::runtime::ml::npu::NpuDtype::UINT8,
            "int8" => crate::runtime::ml::npu::NpuDtype::INT8,
            "f32" => crate::runtime::ml::npu::NpuDtype::F32,
            "f16" => crate::runtime::ml::npu::NpuDtype::F16,
            "bf16" => crate::runtime::ml::npu::NpuDtype::BF16,
            other => {
                return Err(RuntimeError::TypeError(format!(
                    "qnn_quantize: unsupported dtype '{}', expected uint8/int8/f32/f16/bf16",
                    other
                ))
                .into());
            }
        };
        let buf = crate::runtime::ml::npu::QnnBuffer::from_tensor(&tensor, dtype).map_err(|e| {
            EvalError::Runtime(RuntimeError::TypeError(format!("qnn_quantize: {e}")))
        })?;
        let handle = self.qnn_buffers.len() as i64 + 1;
        self.qnn_buffers.insert(handle, buf);
        Ok(Value::Int(handle))
    }

    /// `qnn_dequantize(handle: i64) -> Tensor` — Dequantize QNN buffer back to tensor.
    fn builtin_qnn_dequantize(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        let handle = match &args[0] {
            Value::Int(n) => *n,
            v => {
                return Err(RuntimeError::TypeError(format!(
                    "qnn_dequantize: argument must be int handle, got {:?}",
                    v
                ))
                .into());
            }
        };
        let buf = self.qnn_buffers.get(&handle).ok_or_else(|| {
            EvalError::Runtime(RuntimeError::TypeError(format!(
                "qnn_dequantize: buffer handle {} not found",
                handle
            )))
        })?;
        let tensor = buf.to_tensor().map_err(|e| {
            EvalError::Runtime(RuntimeError::TypeError(format!("qnn_dequantize: {e}")))
        })?;
        Ok(Value::Tensor(tensor))
    }

    // ── ML runtime builtins ──

    /// Helper: extract a shape (Vec<usize>) from a Value::Array of ints.
    fn extract_shape(args: &[Value], idx: usize) -> Result<Vec<usize>, EvalError> {
        match &args[idx] {
            Value::Array(arr) => {
                let mut shape = Vec::with_capacity(arr.len());
                for v in arr {
                    match v {
                        Value::Int(n) if *n >= 0 => shape.push(*n as usize),
                        _ => {
                            return Err(RuntimeError::TypeError(
                                "shape elements must be non-negative integers".into(),
                            )
                            .into());
                        }
                    }
                }
                Ok(shape)
            }
            _ => Err(RuntimeError::TypeError("expected array for shape".into()).into()),
        }
    }

    /// Helper: resolve tensor shape from args.
    /// Accepts either `(rows, cols)` as two ints or `([dim1, dim2, ...])` as one array.
    fn resolve_tensor_shape(&self, args: Vec<Value>) -> Result<Vec<usize>, EvalError> {
        if args.len() == 1 {
            Self::extract_shape(&args, 0)
        } else if args.len() >= 2 && args.iter().all(|a| matches!(a, Value::Int(_))) {
            let mut shape = Vec::with_capacity(args.len());
            for a in &args {
                if let Value::Int(n) = a {
                    if *n >= 0 {
                        shape.push(*n as usize);
                    } else {
                        return Err(RuntimeError::TypeError(
                            "shape dimensions must be non-negative".into(),
                        )
                        .into());
                    }
                }
            }
            Ok(shape)
        } else {
            Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into())
        }
    }

    /// `tensor_zeros(rows, cols)` or `tensor_zeros([dim1, dim2, ...])` → Tensor
    fn builtin_tensor_zeros(&self, args: Vec<Value>) -> EvalResult {
        let shape = self.resolve_tensor_shape(args)?;
        Ok(Value::Tensor(TensorValue::zeros(&shape)))
    }

    /// `tensor_ones(rows, cols)` or `tensor_ones([dim1, dim2, ...])` → Tensor
    fn builtin_tensor_ones(&self, args: Vec<Value>) -> EvalResult {
        let shape = self.resolve_tensor_shape(args)?;
        Ok(Value::Tensor(TensorValue::ones(&shape)))
    }

    /// `tensor_randn(rows, cols)` or `tensor_randn([dim1, dim2, ...])` → Tensor
    fn builtin_tensor_randn(&self, args: Vec<Value>) -> EvalResult {
        let shape = self.resolve_tensor_shape(args)?;
        Ok(Value::Tensor(TensorValue::randn(&shape)))
    }

    /// `tensor_eye(n)` → Tensor (identity matrix)
    fn builtin_tensor_eye(&self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        let n = match &args[0] {
            Value::Int(n) if *n > 0 => *n as usize,
            _ => return Err(RuntimeError::TypeError("eye: n must be positive int".into()).into()),
        };
        Ok(Value::Tensor(TensorValue::eye(n)))
    }

    /// `tensor_full([dim1, dim2, ...], value)` → Tensor
    fn builtin_tensor_full(&self, args: Vec<Value>) -> EvalResult {
        if args.len() != 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into());
        }
        let shape = Self::extract_shape(&args, 0)?;
        let val = match &args[1] {
            Value::Float(v) => *v,
            Value::Int(v) => *v as f64,
            _ => return Err(RuntimeError::TypeError("full: value must be numeric".into()).into()),
        };
        Ok(Value::Tensor(TensorValue::full(&shape, val)))
    }

    /// `tensor_from_data([d1, d2, ...], [dim1, dim2, ...])` → Tensor
    fn builtin_tensor_from_data(&self, args: Vec<Value>) -> EvalResult {
        if args.len() != 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into());
        }
        let data = match &args[0] {
            Value::Array(arr) => {
                let mut data = Vec::with_capacity(arr.len());
                for v in arr {
                    match v {
                        Value::Float(f) => data.push(*f),
                        Value::Int(i) => data.push(*i as f64),
                        _ => {
                            return Err(RuntimeError::TypeError(
                                "tensor data must be numeric".into(),
                            )
                            .into());
                        }
                    }
                }
                data
            }
            _ => return Err(RuntimeError::TypeError("expected array for data".into()).into()),
        };
        let shape = Self::extract_shape(&args, 1)?;
        match TensorValue::from_data(data, &shape) {
            Ok(t) => Ok(Value::Tensor(t)),
            Err(e) => Err(RuntimeError::TypeError(e.to_string()).into()),
        }
    }

    /// `tensor_shape(tensor)` → Array of ints
    fn builtin_tensor_shape(&self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        match &args[0] {
            Value::Tensor(t) => Ok(Value::Array(
                t.shape().iter().map(|&d| Value::Int(d as i64)).collect(),
            )),
            _ => Err(RuntimeError::TypeError("tensor_shape: expected tensor".into()).into()),
        }
    }

    /// `tensor_reshape(tensor, [new_shape])` → Tensor
    fn builtin_tensor_reshape(&self, args: Vec<Value>) -> EvalResult {
        if args.len() != 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into());
        }
        let tensor = match &args[0] {
            Value::Tensor(t) => t,
            _ => {
                return Err(
                    RuntimeError::TypeError("tensor_reshape: expected tensor".into()).into(),
                );
            }
        };
        let new_shape = Self::extract_shape(&args, 1)?;
        let new_numel: usize = new_shape.iter().product();
        if new_numel != tensor.numel() {
            return Err(RuntimeError::TypeError(format!(
                "cannot reshape {:?} ({} elements) to {:?} ({} elements)",
                tensor.shape(),
                tensor.numel(),
                new_shape,
                new_numel
            ))
            .into());
        }
        let data = tensor.to_vec();
        match TensorValue::from_data(data, &new_shape) {
            Ok(t) => Ok(Value::Tensor(t)),
            Err(e) => Err(RuntimeError::TypeError(e.to_string()).into()),
        }
    }

    /// `tensor_numel(tensor)` → Int
    fn builtin_tensor_numel(&self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        match &args[0] {
            Value::Tensor(t) => Ok(Value::Int(t.numel() as i64)),
            _ => Err(RuntimeError::TypeError("tensor_numel: expected tensor".into()).into()),
        }
    }

    /// Binary tensor operation: add/sub/mul/div.
    fn builtin_tensor_binop(&mut self, args: Vec<Value>, op: &str) -> EvalResult {
        if args.len() != 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into());
        }
        let a = match &args[0] {
            Value::Tensor(t) => t.clone(),
            _ => {
                return Err(RuntimeError::TypeError(format!(
                    "tensor_{op}: first arg must be tensor"
                ))
                .into());
            }
        };
        let b = match &args[1] {
            Value::Tensor(t) => t.clone(),
            _ => {
                return Err(RuntimeError::TypeError(format!(
                    "tensor_{op}: second arg must be tensor"
                ))
                .into());
            }
        };
        // Use tracked ops when either input requires grad and tape is recording
        let use_tracked = (a.requires_grad() || b.requires_grad()) && self.tape.is_recording();
        let result = if use_tracked {
            match op {
                "add" => tensor_ops::add_tracked(&a, &b, &mut self.tape),
                "sub" => tensor_ops::sub_tracked(&a, &b, &mut self.tape),
                "mul" => tensor_ops::mul_tracked(&a, &b, &mut self.tape),
                "div" => tensor_ops::div_tracked(&a, &b, &mut self.tape),
                _ => unreachable!(),
            }
        } else {
            match op {
                "add" => tensor_ops::add(&a, &b),
                "sub" => tensor_ops::sub(&a, &b),
                "mul" => tensor_ops::mul(&a, &b),
                "div" => tensor_ops::div(&a, &b),
                _ => unreachable!(),
            }
        };
        match result {
            Ok(t) => Ok(Value::Tensor(t)),
            Err(e) => Err(RuntimeError::TypeError(e.to_string()).into()),
        }
    }

    /// Unary tensor negation.
    fn builtin_tensor_neg(&self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        match &args[0] {
            Value::Tensor(t) => Ok(Value::Tensor(tensor_ops::neg(t))),
            _ => Err(RuntimeError::TypeError("tensor_neg: expected tensor".into()).into()),
        }
    }

    /// Matrix multiplication.
    fn builtin_tensor_matmul(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into());
        }
        let a = match &args[0] {
            Value::Tensor(t) => t.clone(),
            _ => {
                return Err(RuntimeError::TypeError(
                    "tensor_matmul: first arg must be tensor".into(),
                )
                .into());
            }
        };
        let b = match &args[1] {
            Value::Tensor(t) => t.clone(),
            _ => {
                return Err(RuntimeError::TypeError(
                    "tensor_matmul: second arg must be tensor".into(),
                )
                .into());
            }
        };
        let use_tracked = (a.requires_grad() || b.requires_grad()) && self.tape.is_recording();
        let result = if use_tracked {
            tensor_ops::matmul_tracked(&a, &b, &mut self.tape)
        } else {
            tensor_ops::matmul(&a, &b)
        };
        match result {
            Ok(t) => Ok(Value::Tensor(t)),
            Err(e) => Err(RuntimeError::TypeError(e.to_string()).into()),
        }
    }

    /// Tensor transpose.
    fn builtin_tensor_transpose(&self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        match &args[0] {
            Value::Tensor(t) => match tensor_ops::transpose(t) {
                Ok(r) => Ok(Value::Tensor(r)),
                Err(e) => Err(RuntimeError::TypeError(e.to_string()).into()),
            },
            _ => Err(RuntimeError::TypeError("tensor_transpose: expected tensor".into()).into()),
        }
    }

    /// Reduction operation: sum/mean.
    fn builtin_tensor_reduce(&mut self, args: Vec<Value>, op: &str) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        match &args[0] {
            Value::Tensor(t) => {
                let use_tracked = t.requires_grad() && self.tape.is_recording();
                let result = match op {
                    "sum" if use_tracked => tensor_ops::sum_tracked(t, &mut self.tape),
                    "sum" => tensor_ops::sum(t),
                    "mean" => tensor_ops::mean(t),
                    "max" => tensor_ops::max(t),
                    "min" => tensor_ops::min(t),
                    "argmax" => tensor_ops::argmax(t),
                    _ => unreachable!(),
                };
                Ok(Value::Tensor(result))
            }
            _ => Err(RuntimeError::TypeError(format!("tensor_{op}: expected tensor")).into()),
        }
    }

    /// Unary tensor operation (flatten, etc.).
    fn builtin_tensor_unary(&self, args: Vec<Value>, op: &str) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        match &args[0] {
            Value::Tensor(t) => {
                let result = match op {
                    "flatten" => tensor_ops::flatten(t),
                    _ => unreachable!(),
                };
                Ok(Value::Tensor(result))
            }
            _ => Err(RuntimeError::TypeError(format!("tensor_{op}: expected tensor")).into()),
        }
    }

    /// `tensor_squeeze(t, axis)` — remove dimension of size 1.
    fn builtin_tensor_squeeze(&self, args: Vec<Value>) -> EvalResult {
        if args.len() != 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into());
        }
        match (&args[0], &args[1]) {
            (Value::Tensor(t), Value::Int(axis)) => tensor_ops::squeeze(t, *axis as usize)
                .map(Value::Tensor)
                .map_err(|e| RuntimeError::TypeError(e.to_string()).into()),
            _ => {
                Err(RuntimeError::TypeError("tensor_squeeze: expected (tensor, int)".into()).into())
            }
        }
    }

    /// `tensor_unsqueeze(t, axis)` — insert dimension of size 1.
    fn builtin_tensor_unsqueeze(&self, args: Vec<Value>) -> EvalResult {
        if args.len() != 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into());
        }
        match (&args[0], &args[1]) {
            (Value::Tensor(t), Value::Int(axis)) => tensor_ops::unsqueeze(t, *axis as usize)
                .map(Value::Tensor)
                .map_err(|e| RuntimeError::TypeError(e.to_string()).into()),
            _ => Err(
                RuntimeError::TypeError("tensor_unsqueeze: expected (tensor, int)".into()).into(),
            ),
        }
    }

    /// `tensor_arange(start, end, step)` — range tensor.
    fn builtin_tensor_arange(&self, args: Vec<Value>) -> EvalResult {
        if args.len() != 3 {
            return Err(RuntimeError::ArityMismatch {
                expected: 3,
                got: args.len(),
            }
            .into());
        }
        let to_f64 = |v: &Value, name: &str| -> Result<f64, EvalError> {
            match v {
                Value::Float(f) => Ok(*f),
                Value::Int(i) => Ok(*i as f64),
                _ => Err(
                    RuntimeError::TypeError(format!("tensor_arange: {name} must be number")).into(),
                ),
            }
        };
        let start = to_f64(&args[0], "start")?;
        let end = to_f64(&args[1], "end")?;
        let step = to_f64(&args[2], "step")?;
        tensor_ops::arange(start, end, step)
            .map(Value::Tensor)
            .map_err(|e| RuntimeError::TypeError(e.to_string()).into())
    }

    /// `tensor_linspace(start, end, steps)` — evenly spaced tensor.
    fn builtin_tensor_linspace(&self, args: Vec<Value>) -> EvalResult {
        if args.len() != 3 {
            return Err(RuntimeError::ArityMismatch {
                expected: 3,
                got: args.len(),
            }
            .into());
        }
        let to_f64 = |v: &Value, name: &str| -> Result<f64, EvalError> {
            match v {
                Value::Float(f) => Ok(*f),
                Value::Int(i) => Ok(*i as f64),
                _ => Err(RuntimeError::TypeError(format!(
                    "tensor_linspace: {name} must be number"
                ))
                .into()),
            }
        };
        let start = to_f64(&args[0], "start")?;
        let end = to_f64(&args[1], "end")?;
        let steps = match &args[2] {
            Value::Int(i) => *i as usize,
            _ => {
                return Err(
                    RuntimeError::TypeError("tensor_linspace: steps must be int".into()).into(),
                );
            }
        };
        tensor_ops::linspace(start, end, steps)
            .map(Value::Tensor)
            .map_err(|e| RuntimeError::TypeError(e.to_string()).into())
    }

    /// `tensor_xavier(rows, cols)` — Xavier initialization.
    fn builtin_tensor_xavier(&self, args: Vec<Value>) -> EvalResult {
        if args.len() != 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into());
        }
        match (&args[0], &args[1]) {
            (Value::Int(rows), Value::Int(cols)) => Ok(Value::Tensor(tensor_ops::xavier(
                *rows as usize,
                *cols as usize,
            ))),
            _ => Err(RuntimeError::TypeError("tensor_xavier: expected (int, int)".into()).into()),
        }
    }

    /// `tensor_argmax(tensor)` — Returns the index of the maximum element as an integer.
    fn builtin_tensor_argmax(&self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        match &args[0] {
            Value::Tensor(t) => {
                let result = tensor_ops::argmax(t);
                // Convert scalar tensor to integer
                let idx = result.to_scalar().unwrap_or(0.0) as i64;
                Ok(Value::Int(idx))
            }
            _ => Err(RuntimeError::TypeError("tensor_argmax: expected tensor".into()).into()),
        }
    }

    /// `tensor_rows(tensor)` — Returns the number of rows.
    fn builtin_tensor_rows(&self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        match &args[0] {
            Value::Tensor(t) => {
                let shape = t.shape();
                let rows = if shape.is_empty() { 0 } else { shape[0] as i64 };
                Ok(Value::Int(rows))
            }
            _ => Err(RuntimeError::TypeError("tensor_rows: expected tensor".into()).into()),
        }
    }

    /// `tensor_cols(tensor)` — Returns the number of columns.
    fn builtin_tensor_cols(&self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        match &args[0] {
            Value::Tensor(t) => {
                let shape = t.shape();
                let cols = if shape.len() >= 2 {
                    shape[1] as i64
                } else if shape.len() == 1 {
                    shape[0] as i64
                } else {
                    0
                };
                Ok(Value::Int(cols))
            }
            _ => Err(RuntimeError::TypeError("tensor_cols: expected tensor".into()).into()),
        }
    }

    /// `tensor_set(tensor, row, col, value_bits)` — Set a tensor element (value as f64 bits).
    fn builtin_tensor_set(&self, args: Vec<Value>) -> EvalResult {
        if args.len() != 4 {
            return Err(RuntimeError::ArityMismatch {
                expected: 4,
                got: args.len(),
            }
            .into());
        }
        match (&args[0], &args[1], &args[2], &args[3]) {
            (Value::Tensor(t), Value::Int(row), Value::Int(col), Value::Int(val_bits)) => {
                let value = f64::from_bits(*val_bits as u64);
                let mut new_data = t.data().to_owned();
                let r = *row as usize;
                let c = *col as usize;
                if let Some(elem) = new_data.get_mut([r, c]) {
                    *elem = value;
                }
                // tensor_set is a mutation, but in interpreter we return Null
                // (the original tensor is immutable; this is a semantic no-op
                // unless we clone — native codegen mutates in place)
                Ok(Value::Null)
            }
            _ => Err(RuntimeError::TypeError(
                "tensor_set: expected (tensor, int, int, int)".into(),
            )
            .into()),
        }
    }

    /// `tensor_row(tensor, index)` — Extract a single row as a new tensor.
    fn builtin_tensor_row(&self, args: Vec<Value>) -> EvalResult {
        if args.len() != 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into());
        }
        match (&args[0], &args[1]) {
            (Value::Tensor(t), Value::Int(row_idx)) => {
                let shape = t.shape();
                if shape.len() != 2 {
                    return Err(
                        RuntimeError::TypeError("tensor_row: expected 2D tensor".into()).into(),
                    );
                }
                let cols = shape[1];
                let row = *row_idx as usize;
                if row >= shape[0] {
                    return Err(RuntimeError::TypeError(
                        "tensor_row: row index out of bounds".into(),
                    )
                    .into());
                }
                let row_data: Vec<f64> = (0..cols)
                    .map(|c| *t.data().get([row, c]).unwrap_or(&0.0))
                    .collect();
                match TensorValue::from_data(row_data, &[1, cols]) {
                    Ok(tv) => Ok(Value::Tensor(tv)),
                    Err(e) => Err(RuntimeError::TypeError(e.to_string()).into()),
                }
            }
            _ => Err(RuntimeError::TypeError("tensor_row: expected (tensor, int)".into()).into()),
        }
    }

    /// `tensor_normalize(tensor)` — Normalize tensor values to [0, 1] range.
    fn builtin_tensor_normalize(&self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        match &args[0] {
            Value::Tensor(t) => {
                let nd = t.data();
                let min_val = nd.iter().cloned().fold(f64::INFINITY, f64::min);
                let max_val = nd.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                let range = max_val - min_val;
                let normalized: Vec<f64> = if range == 0.0 {
                    vec![0.0; nd.len()]
                } else {
                    nd.iter().map(|&v| (v - min_val) / range).collect()
                };
                let shape = t.shape().to_vec();
                match TensorValue::from_data(normalized, &shape) {
                    Ok(tv) => Ok(Value::Tensor(tv)),
                    Err(e) => Err(RuntimeError::TypeError(e.to_string()).into()),
                }
            }
            _ => Err(RuntimeError::TypeError("tensor_normalize: expected tensor".into()).into()),
        }
    }

    /// `tensor_scale(tensor, scalar_bits)` — Scale tensor by a scalar (f64 bits as i64).
    fn builtin_tensor_scale(&self, args: Vec<Value>) -> EvalResult {
        if args.len() != 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into());
        }
        match (&args[0], &args[1]) {
            (Value::Tensor(t), Value::Int(scalar_bits)) => {
                let scalar = f64::from_bits(*scalar_bits as u64);
                let nd = t.data();
                let scaled: Vec<f64> = nd.iter().map(|&v| v * scalar).collect();
                let shape = t.shape().to_vec();
                match TensorValue::from_data(scaled, &shape) {
                    Ok(tv) => Ok(Value::Tensor(tv)),
                    Err(e) => Err(RuntimeError::TypeError(e.to_string()).into()),
                }
            }
            _ => Err(RuntimeError::TypeError("tensor_scale: expected (tensor, int)".into()).into()),
        }
    }

    /// Unary activation function: relu/sigmoid/tanh/softmax/gelu.
    fn builtin_tensor_activation(&mut self, args: Vec<Value>, op: &str) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        match &args[0] {
            Value::Tensor(t) => {
                let use_tracked = t.requires_grad() && self.tape.is_recording();
                let result = if use_tracked {
                    match op {
                        "relu" => tensor_ops::relu_tracked(t, &mut self.tape),
                        "sigmoid" => tensor_ops::sigmoid_tracked(t, &mut self.tape),
                        "tanh" => tensor_ops::tanh_tracked(t, &mut self.tape),
                        "softmax" => tensor_ops::softmax(t), // no tracked version yet
                        "gelu" => tensor_ops::gelu(t),       // no tracked version yet
                        _ => unreachable!(),
                    }
                } else {
                    match op {
                        "relu" => tensor_ops::relu(t),
                        "sigmoid" => tensor_ops::sigmoid(t),
                        "tanh" => tensor_ops::tanh_act(t),
                        "softmax" => tensor_ops::softmax(t),
                        "gelu" => tensor_ops::gelu(t),
                        _ => unreachable!(),
                    }
                };
                Ok(Value::Tensor(result))
            }
            _ => Err(RuntimeError::TypeError(format!("tensor_{op}: expected tensor")).into()),
        }
    }

    /// Leaky ReLU with optional alpha parameter.
    fn builtin_tensor_leaky_relu(&self, args: Vec<Value>) -> EvalResult {
        if args.is_empty() || args.len() > 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        let alpha = if args.len() == 2 {
            match &args[1] {
                Value::Float(f) => *f,
                Value::Int(i) => *i as f64,
                _ => {
                    return Err(RuntimeError::TypeError(
                        "tensor_leaky_relu: alpha must be a number".into(),
                    )
                    .into());
                }
            }
        } else {
            0.01 // default alpha
        };
        match &args[0] {
            Value::Tensor(t) => Ok(Value::Tensor(tensor_ops::leaky_relu(t, alpha))),
            _ => Err(RuntimeError::TypeError("tensor_leaky_relu: expected tensor".into()).into()),
        }
    }

    /// Loss function: mse/cross_entropy/bce.
    fn builtin_tensor_loss(&self, args: Vec<Value>, op: &str) -> EvalResult {
        if args.len() != 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into());
        }
        match (&args[0], &args[1]) {
            (Value::Tensor(pred), Value::Tensor(target)) => {
                let result = match op {
                    "mse" => tensor_ops::mse_loss(pred, target),
                    "cross_entropy" => tensor_ops::cross_entropy(pred, target),
                    "bce" => tensor_ops::bce_loss(pred, target),
                    "l1" => tensor_ops::l1_loss(pred, target),
                    _ => unreachable!(),
                };
                result
                    .map(Value::Tensor)
                    .map_err(|e| RuntimeError::TypeError(e.to_string()).into())
            }
            _ => Err(
                RuntimeError::TypeError(format!("tensor_{op}_loss: expected two tensors")).into(),
            ),
        }
    }

    /// Helper: extract a VirtAddr from args (validates length and type).
    fn extract_addr(
        &self,
        fn_name: &str,
        args: &[Value],
        expected: usize,
    ) -> Result<crate::runtime::os::VirtAddr, EvalError> {
        if args.len() != expected {
            return Err(RuntimeError::ArityMismatch {
                expected,
                got: args.len(),
            }
            .into());
        }
        self.val_to_addr(fn_name, &args[0])
    }

    /// Helper: convert Value to VirtAddr.
    fn val_to_addr(
        &self,
        fn_name: &str,
        val: &Value,
    ) -> Result<crate::runtime::os::VirtAddr, EvalError> {
        match val {
            Value::Pointer(a) => Ok(crate::runtime::os::VirtAddr(*a)),
            Value::Int(n) => Ok(crate::runtime::os::VirtAddr(*n as u64)),
            _ => Err(RuntimeError::TypeError(format!("{fn_name}: expected pointer/int")).into()),
        }
    }

    /// Evaluates a block expression.
    pub(super) fn eval_block(
        &mut self,
        stmts: &[Stmt],
        tail_expr: &Option<Box<Expr>>,
    ) -> EvalResult {
        // Create a new scope
        let block_env = Rc::new(RefCell::new(Environment::new_with_parent(Rc::clone(
            &self.env,
        ))));
        let prev_env = Rc::clone(&self.env);
        self.env = block_env;

        // Evaluate statements
        for stmt in stmts {
            self.eval_stmt(stmt)?;
        }

        // Evaluate tail expression (the block's value)
        let result = match tail_expr {
            Some(e) => self.eval_expr(e),
            None => Ok(Value::Null),
        };

        // Drop owned locals at scope exit (simulates destructors)
        self.env.borrow_mut().drop_locals();

        // Restore scope
        self.env = prev_env;
        result
    }

    /// Evaluates an if expression.
    pub(super) fn eval_if(
        &mut self,
        condition: &Expr,
        then_branch: &Expr,
        else_branch: &Option<Box<Expr>>,
    ) -> EvalResult {
        let cond = self.eval_expr(condition)?;
        if cond.is_truthy() {
            self.eval_expr(then_branch)
        } else if let Some(else_e) = else_branch {
            self.eval_expr(else_e)
        } else {
            Ok(Value::Null)
        }
    }

    /// Evaluates a while loop with optional label.
    pub(super) fn eval_while(
        &mut self,
        condition: &Expr,
        body: &Expr,
        label: Option<&str>,
    ) -> EvalResult {
        loop {
            let cond = self.eval_expr(condition)?;
            if !cond.is_truthy() {
                break;
            }
            match self.eval_expr(body) {
                Ok(_) => {}
                Err(EvalError::Control(cf)) => match *cf {
                    ControlFlow::Break(v, ref bl) => {
                        if bl.is_none() || bl.as_deref() == label {
                            return Ok(v);
                        }
                        return Err(EvalError::Control(Box::new(ControlFlow::Break(
                            v,
                            bl.clone(),
                        ))));
                    }
                    ControlFlow::Continue(ref cl) => {
                        if cl.is_none() || cl.as_deref() == label {
                            continue;
                        }
                        return Err(EvalError::Control(Box::new(ControlFlow::Continue(
                            cl.clone(),
                        ))));
                    }
                    cf => return Err(EvalError::Control(Box::new(cf))),
                },
                Err(e) => return Err(e),
            }
        }
        Ok(Value::Null)
    }

    /// Evaluates an infinite loop: `loop { body }` with optional label.
    pub(super) fn eval_loop(&mut self, body: &Expr, label: Option<&str>) -> EvalResult {
        loop {
            match self.eval_expr(body) {
                Ok(_) => {}
                Err(EvalError::Control(cf)) => match *cf {
                    ControlFlow::Break(v, ref bl) => {
                        if bl.is_none() || bl.as_deref() == label {
                            return Ok(v);
                        }
                        return Err(EvalError::Control(Box::new(ControlFlow::Break(
                            v,
                            bl.clone(),
                        ))));
                    }
                    ControlFlow::Continue(ref cl) => {
                        if cl.is_none() || cl.as_deref() == label {
                            continue;
                        }
                        return Err(EvalError::Control(Box::new(ControlFlow::Continue(
                            cl.clone(),
                        ))));
                    }
                    cf => return Err(EvalError::Control(Box::new(cf))),
                },
                Err(e) => return Err(e),
            }
        }
    }

    /// Evaluates a for loop with optional label.
    pub(super) fn eval_for(
        &mut self,
        variable: &str,
        iterable: &Expr,
        body: &Expr,
        label: Option<&str>,
    ) -> EvalResult {
        let iter_val = self.eval_expr(iterable)?;

        // If iterable is already an Iterator, use iterator protocol
        if let Value::Iterator(iter_rc) = iter_val {
            return self.for_loop_iterator(variable, iter_rc, body, label);
        }

        // Convert value to iterator or eagerly collect
        let items: Vec<Value> = match iter_val {
            Value::Array(arr) => arr,
            Value::Tuple(t) => t,
            Value::Str(s) => s.chars().map(Value::Char).collect(),
            Value::Map(m) => m
                .into_iter()
                .map(|(k, v)| Value::Tuple(vec![Value::Str(k), v]))
                .collect(),
            _ => {
                return Err(RuntimeError::TypeError(format!(
                    "cannot iterate over {}",
                    iter_val.type_name()
                ))
                .into());
            }
        };

        for item in items {
            let loop_env = Rc::new(RefCell::new(Environment::new_with_parent(Rc::clone(
                &self.env,
            ))));
            loop_env.borrow_mut().define(variable.to_string(), item);

            let prev_env = Rc::clone(&self.env);
            self.env = loop_env;

            let result = self.eval_expr(body);

            self.env = prev_env;

            match result {
                Ok(_) => {}
                Err(EvalError::Control(cf)) => match *cf {
                    ControlFlow::Break(v, ref bl) => {
                        if bl.is_none() || bl.as_deref() == label {
                            return Ok(v);
                        }
                        return Err(EvalError::Control(Box::new(ControlFlow::Break(
                            v,
                            bl.clone(),
                        ))));
                    }
                    ControlFlow::Continue(ref cl) => {
                        if cl.is_none() || cl.as_deref() == label {
                            continue;
                        }
                        return Err(EvalError::Control(Box::new(ControlFlow::Continue(
                            cl.clone(),
                        ))));
                    }
                    cf => return Err(EvalError::Control(Box::new(cf))),
                },
                Err(e) => return Err(e),
            }
        }

        Ok(Value::Null)
    }

    /// Runs a for loop using the iterator protocol (call next() until None).
    fn for_loop_iterator(
        &mut self,
        variable: &str,
        iter_rc: Rc<RefCell<IteratorValue>>,
        body: &Expr,
        label: Option<&str>,
    ) -> EvalResult {
        loop {
            let item = self.iter_next(&iter_rc)?;
            let item = match item {
                Some(v) => v,
                None => break,
            };

            let loop_env = Rc::new(RefCell::new(Environment::new_with_parent(Rc::clone(
                &self.env,
            ))));
            loop_env.borrow_mut().define(variable.to_string(), item);

            let prev_env = Rc::clone(&self.env);
            self.env = loop_env;

            let result = self.eval_expr(body);

            self.env = prev_env;

            match result {
                Ok(_) => {}
                Err(EvalError::Control(cf)) => match *cf {
                    ControlFlow::Break(v, ref bl) => {
                        if bl.is_none() || bl.as_deref() == label {
                            return Ok(v);
                        }
                        return Err(EvalError::Control(Box::new(ControlFlow::Break(
                            v,
                            bl.clone(),
                        ))));
                    }
                    ControlFlow::Continue(ref cl) => {
                        if cl.is_none() || cl.as_deref() == label {
                            continue;
                        }
                        return Err(EvalError::Control(Box::new(ControlFlow::Continue(
                            cl.clone(),
                        ))));
                    }
                    cf => return Err(EvalError::Control(Box::new(cf))),
                },
                Err(e) => return Err(e),
            }
        }
        Ok(Value::Null)
    }

    /// Advances an iterator, handling combinators that need function calls.
    fn iter_next(
        &mut self,
        iter_rc: &Rc<RefCell<IteratorValue>>,
    ) -> Result<Option<Value>, EvalError> {
        let mut iter = iter_rc.borrow_mut();
        match &mut *iter {
            IteratorValue::MappedIter { inner, func } => {
                let inner_clone = inner.clone();
                let func_clone = func.clone();
                drop(iter); // Release borrow before calling function
                let inner_rc = Rc::new(RefCell::new(*inner_clone));
                let val = self.iter_next(&inner_rc)?;
                // Write back the advanced inner iterator
                let advanced = inner_rc.borrow().clone();
                let mut iter = iter_rc.borrow_mut();
                if let IteratorValue::MappedIter { inner, .. } = &mut *iter {
                    **inner = advanced;
                }
                match val {
                    Some(v) => {
                        drop(iter);
                        let result = self.call_function(&func_clone, vec![v])?;
                        Ok(Some(result))
                    }
                    None => Ok(None),
                }
            }
            IteratorValue::FilterIter { inner, func } => {
                let inner_clone = inner.clone();
                let func_clone = func.clone();
                drop(iter);
                let inner_rc = Rc::new(RefCell::new(*inner_clone));
                loop {
                    let val = self.iter_next(&inner_rc)?;
                    // Write back
                    let advanced = inner_rc.borrow().clone();
                    let mut iter = iter_rc.borrow_mut();
                    if let IteratorValue::FilterIter { inner, .. } = &mut *iter {
                        **inner = advanced.clone();
                    }
                    drop(iter);
                    match val {
                        Some(v) => {
                            let pred = self.call_function(&func_clone, vec![v.clone()])?;
                            if matches!(pred, Value::Bool(true)) {
                                return Ok(Some(v));
                            }
                            // Update inner_rc for next iteration
                            let iter = iter_rc.borrow();
                            if let IteratorValue::FilterIter { inner, .. } = &*iter {
                                *inner_rc.borrow_mut() = *inner.clone();
                            }
                        }
                        None => return Ok(None),
                    }
                }
            }
            _ => Ok(iter.next_simple()),
        }
    }

    /// Evaluates an assignment expression.
    pub(super) fn eval_assign(&mut self, target: &Expr, op: AssignOp, value: &Expr) -> EvalResult {
        let new_val = self.eval_expr(value)?;

        match target {
            Expr::Ident { name, .. } => {
                let final_val = if op == AssignOp::Assign {
                    new_val
                } else {
                    let old = self.eval_ident(name)?;
                    self.apply_compound_assign(&old, op, &new_val)?
                };
                if !self.env.borrow_mut().assign(name, final_val) {
                    return Err(RuntimeError::UndefinedVariable(name.clone()).into());
                }
                Ok(Value::Null)
            }
            Expr::Index { object, index, .. } => {
                let idx_val = self.eval_expr(index)?;
                let obj_val = self.eval_expr(object)?;

                match (&obj_val, &idx_val) {
                    (Value::Array(arr), Value::Int(i)) => {
                        let idx = *i as usize;
                        if idx >= arr.len() {
                            return Err(RuntimeError::IndexOutOfBounds {
                                index: *i,
                                collection: "array".into(),
                                length: arr.len(),
                            }
                            .into());
                        }
                        let mut new_arr = arr.clone();
                        let final_val = if op == AssignOp::Assign {
                            new_val
                        } else {
                            self.apply_compound_assign(&arr[idx], op, &new_val)?
                        };
                        new_arr[idx] = final_val;
                        // Re-assign the whole array back
                        if let Expr::Ident { name, .. } = object.as_ref() {
                            if !self.env.borrow_mut().assign(name, Value::Array(new_arr)) {
                                return Err(RuntimeError::UndefinedVariable(name.clone()).into());
                            }
                        }
                        Ok(Value::Null)
                    }
                    _ => Err(RuntimeError::InvalidAssignTarget.into()),
                }
            }
            Expr::Field { object, field, .. } => {
                let obj_val = self.eval_expr(object)?;
                match obj_val {
                    Value::Struct {
                        name: sname,
                        mut fields,
                    } => {
                        let final_val = if op == AssignOp::Assign {
                            new_val
                        } else {
                            let old = fields
                                .get(field)
                                .ok_or(RuntimeError::TypeError(format!(
                                    "struct '{sname}' has no field '{field}'"
                                )))?
                                .clone();
                            self.apply_compound_assign(&old, op, &new_val)?
                        };
                        fields.insert(field.clone(), final_val);
                        // Re-assign struct
                        if let Expr::Ident { name, .. } = object.as_ref() {
                            let new_struct = Value::Struct {
                                name: sname,
                                fields,
                            };
                            if !self.env.borrow_mut().assign(name, new_struct) {
                                return Err(RuntimeError::UndefinedVariable(name.clone()).into());
                            }
                        }
                        Ok(Value::Null)
                    }
                    _ => Err(RuntimeError::InvalidAssignTarget.into()),
                }
            }
            _ => Err(RuntimeError::InvalidAssignTarget.into()),
        }
    }

    /// Applies a compound assignment operator (+=, -=, etc.).
    fn apply_compound_assign(&self, old: &Value, op: AssignOp, new_val: &Value) -> EvalResult {
        let binop = match op {
            AssignOp::AddAssign => BinOp::Add,
            AssignOp::SubAssign => BinOp::Sub,
            AssignOp::MulAssign => BinOp::Mul,
            AssignOp::DivAssign => BinOp::Div,
            AssignOp::RemAssign => BinOp::Rem,
            AssignOp::BitAndAssign => BinOp::BitAnd,
            AssignOp::BitOrAssign => BinOp::BitOr,
            AssignOp::BitXorAssign => BinOp::BitXor,
            AssignOp::ShlAssign => BinOp::Shl,
            AssignOp::ShrAssign => BinOp::Shr,
            AssignOp::Assign => unreachable!(),
        };
        match (old, new_val) {
            (Value::Int(a), Value::Int(b)) => self.eval_int_binop(*a, binop, *b),
            (Value::Float(a), Value::Float(b)) => self.eval_float_binop(*a, binop, *b),
            (Value::Int(a), Value::Float(b)) => self.eval_float_binop(*a as f64, binop, *b),
            (Value::Float(a), Value::Int(b)) => self.eval_float_binop(*a, binop, *b as f64),
            (Value::Str(a), Value::Str(b)) if binop == BinOp::Add => {
                Ok(Value::Str(format!("{a}{b}")))
            }
            // Pointer compound assignment: ptr += offset, ptr -= offset
            (Value::Pointer(addr), Value::Int(offset)) if binop == BinOp::Add => {
                Ok(Value::Pointer(addr.wrapping_add(*offset as u64)))
            }
            (Value::Pointer(addr), Value::Int(offset)) if binop == BinOp::Sub => {
                Ok(Value::Pointer(addr.wrapping_sub(*offset as u64)))
            }
            _ => Err(RuntimeError::TypeError(format!(
                "unsupported compound assignment for {} and {}",
                old.type_name(),
                new_val.type_name()
            ))
            .into()),
        }
    }

    /// Evaluates a match expression.
    pub(super) fn eval_match(&mut self, subject: &Expr, arms: &[MatchArm]) -> EvalResult {
        let subject_val = self.eval_expr(subject)?;

        for arm in arms {
            if let Some(bindings) = self.match_pattern(&arm.pattern, &subject_val) {
                // Check guard if present
                if let Some(guard) = &arm.guard {
                    // Create scope with bindings for guard evaluation
                    let guard_env = Rc::new(RefCell::new(Environment::new_with_parent(Rc::clone(
                        &self.env,
                    ))));
                    for (k, v) in &bindings {
                        guard_env.borrow_mut().define(k.clone(), v.clone());
                    }
                    let prev = Rc::clone(&self.env);
                    self.env = guard_env;
                    let guard_val = self.eval_expr(guard)?;
                    self.env = prev;
                    if !guard_val.is_truthy() {
                        continue;
                    }
                }

                // Create scope with pattern bindings and evaluate body
                let arm_env = Rc::new(RefCell::new(Environment::new_with_parent(Rc::clone(
                    &self.env,
                ))));
                for (k, v) in bindings {
                    arm_env.borrow_mut().define(k, v);
                }
                let prev = Rc::clone(&self.env);
                self.env = arm_env;
                let result = self.eval_expr(&arm.body);
                self.env = prev;
                return result;
            }
        }

        // No arm matched
        Ok(Value::Null)
    }

    /// Attempts to match a value against a pattern.
    ///
    /// Returns `Some(bindings)` if the pattern matches, `None` otherwise.
    fn match_pattern(&self, pattern: &Pattern, value: &Value) -> Option<HashMap<String, Value>> {
        match pattern {
            Pattern::Wildcard { .. } => Some(HashMap::new()),
            Pattern::Ident { name, .. } => {
                // Check if this is a known unit enum variant (e.g., None)
                if let Some(Value::Enum {
                    variant,
                    data: None,
                }) = self.env.borrow().lookup(name)
                {
                    // Compare as unit variant match
                    return if let Value::Enum {
                        variant: v,
                        data: d,
                    } = value
                    {
                        if &variant == v && d.is_none() {
                            Some(HashMap::new())
                        } else {
                            None
                        }
                    } else {
                        None
                    };
                }
                let mut bindings = HashMap::new();
                bindings.insert(name.clone(), value.clone());
                Some(bindings)
            }
            Pattern::Literal { kind, .. } => {
                let pat_val = match kind {
                    LiteralKind::Int(v) => Value::Int(*v),
                    LiteralKind::Float(v) => Value::Float(*v),
                    LiteralKind::String(s) | LiteralKind::RawString(s) => Value::Str(s.clone()),
                    LiteralKind::Char(c) => Value::Char(*c),
                    LiteralKind::Bool(b) => Value::Bool(*b),
                    LiteralKind::Null => Value::Null,
                };
                if &pat_val == value {
                    Some(HashMap::new())
                } else {
                    None
                }
            }
            Pattern::Tuple { elements, .. } => {
                if let Value::Tuple(vals) = value {
                    if elements.len() != vals.len() {
                        return None;
                    }
                    let mut bindings = HashMap::new();
                    for (pat, val) in elements.iter().zip(vals.iter()) {
                        let sub = self.match_pattern(pat, val)?;
                        bindings.extend(sub);
                    }
                    Some(bindings)
                } else {
                    None
                }
            }
            Pattern::Enum {
                variant, fields, ..
            } => {
                if let Value::Enum {
                    variant: v,
                    data: d,
                } = value
                {
                    if variant != v {
                        return None;
                    }
                    if fields.is_empty() {
                        // Unit variant pattern
                        if d.is_none() {
                            return Some(HashMap::new());
                        }
                        return None;
                    }
                    // Variant with data
                    if let Some(inner) = d {
                        if fields.len() == 1 {
                            return self.match_pattern(&fields[0], inner);
                        }
                        // Multiple fields — match against tuple
                        if let Value::Tuple(vals) = inner.as_ref() {
                            if fields.len() != vals.len() {
                                return None;
                            }
                            let mut bindings = HashMap::new();
                            for (pat, val) in fields.iter().zip(vals.iter()) {
                                let sub = self.match_pattern(pat, val)?;
                                bindings.extend(sub);
                            }
                            return Some(bindings);
                        }
                    }
                    None
                } else {
                    None
                }
            }
            Pattern::Struct {
                name: _,
                fields: pat_fields,
                ..
            } => {
                if let Value::Struct { fields, .. } = value {
                    let mut bindings = HashMap::new();
                    for fp in pat_fields {
                        let val = fields.get(&fp.name)?;
                        if let Some(ref pat) = fp.pattern {
                            let sub = self.match_pattern(pat, val)?;
                            bindings.extend(sub);
                        } else {
                            bindings.insert(fp.name.clone(), val.clone());
                        }
                    }
                    Some(bindings)
                } else {
                    None
                }
            }
            Pattern::Range {
                start,
                end,
                inclusive,
                ..
            } => {
                // Extract literal bounds from AST (range patterns use literal expressions)
                let in_range = match value {
                    Value::Int(v) => {
                        let lo = Self::expr_as_i64(start)?;
                        let hi = Self::expr_as_i64(end)?;
                        *v >= lo && (if *inclusive { *v <= hi } else { *v < hi })
                    }
                    Value::Float(v) => {
                        let lo = Self::expr_as_f64(start)?;
                        let hi = Self::expr_as_f64(end)?;
                        *v >= lo && (if *inclusive { *v <= hi } else { *v < hi })
                    }
                    Value::Char(v) => {
                        let lo = Self::expr_as_char(start)?;
                        let hi = Self::expr_as_char(end)?;
                        *v >= lo && (if *inclusive { *v <= hi } else { *v < hi })
                    }
                    _ => return None,
                };
                if in_range { Some(HashMap::new()) } else { None }
            }
            Pattern::Or { patterns, .. } => {
                // Try each alternative — first match wins
                for alt in patterns {
                    if let Some(bindings) = self.match_pattern(alt, value) {
                        return Some(bindings);
                    }
                }
                None
            }
        }
    }

    /// Extracts an i64 from a literal expression (for range pattern bounds).
    fn expr_as_i64(expr: &Expr) -> Option<i64> {
        if let Expr::Literal {
            kind: LiteralKind::Int(v),
            ..
        } = expr
        {
            Some(*v)
        } else {
            None
        }
    }

    /// Extracts an f64 from a literal expression (for range pattern bounds).
    fn expr_as_f64(expr: &Expr) -> Option<f64> {
        match expr {
            Expr::Literal {
                kind: LiteralKind::Float(v),
                ..
            } => Some(*v),
            Expr::Literal {
                kind: LiteralKind::Int(v),
                ..
            } => Some(*v as f64),
            _ => None,
        }
    }

    /// Extracts a char from a literal expression (for range pattern bounds).
    fn expr_as_char(expr: &Expr) -> Option<char> {
        if let Expr::Literal {
            kind: LiteralKind::Char(c),
            ..
        } = expr
        {
            Some(*c)
        } else {
            None
        }
    }

    /// Evaluates an array literal.
    pub(super) fn eval_array(&mut self, elements: &[Expr]) -> EvalResult {
        let mut vals = Vec::with_capacity(elements.len());
        for e in elements {
            vals.push(self.eval_expr(e)?);
        }
        Ok(Value::Array(vals))
    }

    /// Evaluates a tuple literal.
    pub(super) fn eval_tuple(&mut self, elements: &[Expr]) -> EvalResult {
        let mut vals = Vec::with_capacity(elements.len());
        for e in elements {
            vals.push(self.eval_expr(e)?);
        }
        Ok(Value::Tuple(vals))
    }

    /// Evaluates a pipeline expression: `x |> f` → `f(x)`.
    pub(super) fn eval_pipe(&mut self, left: &Expr, right: &Expr) -> EvalResult {
        let arg = self.eval_expr(left)?;
        let func = self.eval_expr(right)?;
        match func {
            Value::Function(fv) => self.call_function(&fv, vec![arg]),
            Value::BuiltinFn(name) => self.call_builtin(&name, vec![arg]),
            _ => Err(RuntimeError::NotAFunction(format!("{func}")).into()),
        }
    }

    /// Evaluates struct initialization: `Point { x: 1, y: 2 }`.
    pub(super) fn eval_struct_init(&mut self, name: &str, fields: &[FieldInit]) -> EvalResult {
        let mut field_map = HashMap::new();
        for fi in fields {
            let val = self.eval_expr(&fi.value)?;
            field_map.insert(fi.name.clone(), val);
        }
        Ok(Value::Struct {
            name: name.to_string(),
            fields: field_map,
        })
    }

    /// Evaluates field access: `obj.field`.
    pub(super) fn eval_field(&mut self, object: &Expr, field: &str) -> EvalResult {
        let obj = self.eval_expr(object)?;
        match &obj {
            Value::Struct { name, fields } => fields.get(field).cloned().ok_or_else(|| {
                RuntimeError::TypeError(format!("struct '{name}' has no field '{field}'")).into()
            }),
            Value::Tuple(elems) => {
                // Support tuple.0, tuple.1, etc.
                if let Ok(idx) = field.parse::<usize>() {
                    elems.get(idx).cloned().ok_or_else(|| {
                        RuntimeError::IndexOutOfBounds {
                            index: idx as i64,
                            collection: "tuple".into(),
                            length: elems.len(),
                        }
                        .into()
                    })
                } else {
                    Err(
                        RuntimeError::TypeError(format!("cannot access field '{field}' on tuple"))
                            .into(),
                    )
                }
            }
            _ => {
                // Check if this might be a method call without ()
                let type_name = obj.type_name();
                #[allow(clippy::needless_borrow)]
                let is_known_method = is_known_method_name(&type_name, field);
                let hint = if is_known_method {
                    format!(" — did you mean `{field}()`? (add parentheses for method call)")
                } else {
                    String::new()
                };
                Err(RuntimeError::TypeError(format!(
                    "cannot access field '{field}' on {type_name}{hint}"
                ))
                .into())
            }
        }
    }

    /// Evaluates index access: `arr[i]`.
    /// Evaluates index access: `arr[i]`.
    pub(super) fn eval_index(&mut self, object: &Expr, index: &Expr) -> EvalResult {
        let obj = self.eval_expr(object)?;
        let idx = self.eval_expr(index)?;

        match (&obj, &idx) {
            (Value::Array(arr), Value::Int(i)) => {
                let idx_usize = *i as usize;
                arr.get(idx_usize).cloned().ok_or_else(|| {
                    RuntimeError::IndexOutOfBounds {
                        index: *i,
                        collection: "array".into(),
                        length: arr.len(),
                    }
                    .into()
                })
            }
            (Value::Str(s), Value::Int(i)) => {
                let idx_usize = *i as usize;
                let char_len = s.chars().count();
                s.chars().nth(idx_usize).map(Value::Char).ok_or_else(|| {
                    RuntimeError::IndexOutOfBounds {
                        index: *i,
                        collection: "string".into(),
                        length: char_len,
                    }
                    .into()
                })
            }
            _ => Err(RuntimeError::TypeError(format!(
                "cannot index {} with {}",
                obj.type_name(),
                idx.type_name()
            ))
            .into()),
        }
    }

    /// Evaluates a range expression, producing an Array of integers.
    pub(super) fn eval_range(
        &mut self,
        start: &Option<Box<Expr>>,
        end: &Option<Box<Expr>>,
        inclusive: bool,
    ) -> EvalResult {
        let start_val = match start {
            Some(e) => match self.eval_expr(e)? {
                Value::Int(v) => v,
                _ => {
                    return Err(
                        RuntimeError::TypeError("range bounds must be integers".into()).into(),
                    );
                }
            },
            None => 0,
        };

        let end_val = match end {
            Some(e) => match self.eval_expr(e)? {
                Value::Int(v) => v,
                _ => {
                    return Err(
                        RuntimeError::TypeError("range bounds must be integers".into()).into(),
                    );
                }
            },
            None => {
                return Err(RuntimeError::TypeError("range must have an end bound".into()).into());
            }
        };

        let items: Vec<Value> = if inclusive {
            (start_val..=end_val).map(Value::Int).collect()
        } else {
            (start_val..end_val).map(Value::Int).collect()
        };

        Ok(Value::Array(items))
    }

    /// Evaluates a closure expression.
    pub(super) fn eval_closure(
        &mut self,
        params: &[crate::parser::ast::ClosureParam],
        body: &Expr,
    ) -> EvalResult {
        let closure_params: Vec<crate::parser::ast::Param> = params
            .iter()
            .map(|cp| crate::parser::ast::Param {
                name: cp.name.clone(),
                ty: cp
                    .ty
                    .clone()
                    .unwrap_or(crate::parser::ast::TypeExpr::Simple {
                        name: "any".to_string(),
                        span: crate::lexer::token::Span::new(0, 0),
                    }),
                span: cp.span,
            })
            .collect();

        Ok(Value::Function(FnValue {
            name: String::new(),
            params: closure_params,
            body: Box::new(body.clone()),
            closure_env: Rc::clone(&self.env),
            is_async: false,
        }))
    }

    /// Evaluates the `?` (try) operator.
    ///
    /// Unwraps `Ok(v)` or `Some(v)` to `v`.
    /// For `Err(e)` or `None`, early-returns from the enclosing function.
    /// Reorders named arguments to match parameter order.
    pub(super) fn reorder_named_args(
        &self,
        params: &[crate::parser::ast::Param],
        args: Vec<(Option<String>, Value)>,
    ) -> Result<Vec<Value>, EvalError> {
        let mut result = vec![Value::Null; params.len()];
        let mut filled = vec![false; params.len()];
        let mut positional_idx = 0;

        for (name, val) in args {
            if let Some(arg_name) = name {
                // Named argument: find matching parameter
                let pos = params
                    .iter()
                    .position(|p| p.name == arg_name)
                    .ok_or_else(|| {
                        RuntimeError::TypeError(format!("unknown parameter name '{arg_name}'"))
                    })?;
                result[pos] = val;
                filled[pos] = true;
            } else {
                // Positional argument: fill next unfilled slot
                while positional_idx < params.len() && filled[positional_idx] {
                    positional_idx += 1;
                }
                if positional_idx >= params.len() {
                    return Err(RuntimeError::ArityMismatch {
                        expected: params.len(),
                        got: positional_idx + 1,
                    }
                    .into());
                }
                result[positional_idx] = val;
                filled[positional_idx] = true;
                positional_idx += 1;
            }
        }
        Ok(result)
    }

    /// Helper for unary math functions that take and return f64.
    fn math_f64_unary(&self, args: Vec<Value>, f: fn(f64) -> f64) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        let v = match &args[0] {
            Value::Float(x) => *x,
            Value::Int(x) => *x as f64,
            _ => {
                return Err(
                    RuntimeError::TypeError("math function requires a number".into()).into(),
                );
            }
        };
        Ok(Value::Float(f(v)))
    }

    /// Helper for binary math functions that take and return f64.
    fn math_f64_binary(&self, args: Vec<Value>, f: fn(f64, f64) -> f64) -> EvalResult {
        if args.len() != 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into());
        }
        let a = match &args[0] {
            Value::Float(x) => *x,
            Value::Int(x) => *x as f64,
            _ => {
                return Err(
                    RuntimeError::TypeError("math function requires a number".into()).into(),
                );
            }
        };
        let b = match &args[1] {
            Value::Float(x) => *x,
            Value::Int(x) => *x as f64,
            _ => {
                return Err(
                    RuntimeError::TypeError("math function requires a number".into()).into(),
                );
            }
        };
        Ok(Value::Float(f(a, b)))
    }

    /// Helper for wrapping/saturating integer binary builtins.
    fn int_binop_builtin(
        &self,
        args: Vec<Value>,
        name: &str,
        f: fn(i64, i64) -> i64,
    ) -> EvalResult {
        if args.len() != 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into());
        }
        match (&args[0], &args[1]) {
            (Value::Int(a), Value::Int(b)) => Ok(Value::Int(f(*a, *b))),
            _ => Err(RuntimeError::TypeError(format!("{name}() requires two integers")).into()),
        }
    }

    /// Helper for checked integer binary builtins (returns Option-like Enum).
    fn checked_int_builtin(
        &self,
        args: Vec<Value>,
        name: &str,
        f: fn(i64, i64) -> Option<i64>,
    ) -> EvalResult {
        if args.len() != 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into());
        }
        match (&args[0], &args[1]) {
            (Value::Int(a), Value::Int(b)) => match f(*a, *b) {
                Some(result) => Ok(Value::Enum {
                    variant: "Some".into(),
                    data: Some(Box::new(Value::Int(result))),
                }),
                None => Ok(Value::Enum {
                    variant: "None".into(),
                    data: None,
                }),
            },
            _ => Err(RuntimeError::TypeError(format!("{name}() requires two integers")).into()),
        }
    }

    /// Evaluates a type cast expression: `expr as Type`.
    pub(super) fn eval_cast(&mut self, expr: &Expr, target_ty: &TypeExpr) -> EvalResult {
        let val = self.eval_expr(expr)?;
        let type_name = match target_ty {
            TypeExpr::Simple { name, .. } => name.as_str(),
            _ => {
                return Err(
                    RuntimeError::TypeError("cast to complex types not supported".into()).into(),
                );
            }
        };
        match (&val, type_name) {
            // Int → Float
            (Value::Int(n), "f64" | "f32") => Ok(Value::Float(*n as f64)),
            // Float → Int (truncate to target width)
            (Value::Float(f), "i64") => Ok(Value::Int(*f as i64)),
            (Value::Float(f), "i32") => Ok(Value::Int((*f as i32) as i64)),
            (Value::Float(f), "i16") => Ok(Value::Int((*f as i16) as i64)),
            (Value::Float(f), "i8") => Ok(Value::Int((*f as i8) as i64)),
            (Value::Float(f), "u8") => Ok(Value::Int((*f as u8) as i64)),
            (Value::Float(f), "u16") => Ok(Value::Int((*f as u16) as i64)),
            (Value::Float(f), "u32") => Ok(Value::Int((*f as u32) as i64)),
            (Value::Float(f), "u64") => Ok(Value::Int(*f as i64)),
            // Int → Int (narrowing casts truncate, widening preserves)
            (Value::Int(n), "u8") => Ok(Value::Int((*n as u8) as i64)),
            (Value::Int(n), "u16") => Ok(Value::Int((*n as u16) as i64)),
            (Value::Int(n), "u32") => Ok(Value::Int((*n as u32) as i64)),
            (Value::Int(n), "i8") => Ok(Value::Int((*n as i8) as i64)),
            (Value::Int(n), "i16") => Ok(Value::Int((*n as i16) as i64)),
            (Value::Int(n), "i32") => Ok(Value::Int((*n as i32) as i64)),
            (Value::Int(_), "i64" | "u64" | "isize" | "usize") => Ok(val),
            // Float → Float (stored as f64 internally)
            (Value::Float(_), "f64" | "f32") => Ok(val),
            // Bool → Int
            (Value::Bool(b), "i64" | "i32" | "i16" | "i8") => {
                Ok(Value::Int(if *b { 1 } else { 0 }))
            }
            // Int → Bool
            (Value::Int(n), "bool") => Ok(Value::Bool(*n != 0)),
            _ => Err(RuntimeError::TypeError(format!(
                "cannot cast {} to {type_name}",
                val.type_name()
            ))
            .into()),
        }
    }

    pub(super) fn eval_try(&mut self, expr: &Expr) -> EvalResult {
        let val = self.eval_expr(expr)?;
        match &val {
            Value::Enum { variant, data } => match variant.as_str() {
                "Ok" | "Some" => Ok(data.as_ref().map(|d| *d.clone()).unwrap_or(Value::Null)),
                "Err" | "None" => Err(ControlFlow::Return(val).into()),
                _ => Err(RuntimeError::TypeError(
                    "? operator requires Option or Result value".into(),
                )
                .into()),
            },
            _ => Err(
                RuntimeError::TypeError("? operator requires Option or Result value".into()).into(),
            ),
        }
    }

    /// Evaluates an impl block: registers methods in the impl_methods registry.
    pub(super) fn eval_impl_block(
        &mut self,
        impl_block: &crate::parser::ast::ImplBlock,
    ) -> EvalResult {
        let type_name = &impl_block.target_type;
        // Track trait impls for dynamic dispatch
        if let Some(ref trait_name) = impl_block.trait_name {
            self.trait_impls
                .insert((trait_name.clone(), type_name.clone()));
        }
        for method in &impl_block.methods {
            let fn_val = FnValue {
                name: method.name.clone(),
                params: method.params.clone(),
                body: method.body.clone(),
                closure_env: Rc::clone(&self.env),
                is_async: false,
            };

            // Check if this is a static method (no `self` param) — also register globally
            let is_static = method.params.first().is_none_or(|p| p.name != "self");
            if is_static {
                // Register as `TypeName::method_name` in global env for path access
                let qualified = format!("{}::{}", type_name, method.name);
                self.env
                    .borrow_mut()
                    .define(qualified, Value::Function(fn_val.clone()));
            }

            self.impl_methods
                .insert((type_name.clone(), method.name.clone()), fn_val);
        }
        Ok(Value::Null)
    }

    /// Evaluates a method call on an iterator value.
    pub(super) fn eval_iterator_method(
        &mut self,
        iter_rc: Rc<RefCell<IteratorValue>>,
        method: &str,
        args: Vec<Value>,
    ) -> EvalResult {
        match method {
            "next" => {
                let val = self.iter_next(&iter_rc)?;
                Ok(match val {
                    Some(v) => Value::Enum {
                        variant: "Some".into(),
                        data: Some(Box::new(v)),
                    },
                    None => Value::Enum {
                        variant: "None".into(),
                        data: None,
                    },
                })
            }
            "map" => {
                let func = match args.into_iter().next() {
                    Some(Value::Function(fv)) => fv,
                    _ => {
                        return Err(RuntimeError::TypeError(
                            ".map() requires a function argument".into(),
                        )
                        .into());
                    }
                };
                let inner = iter_rc.borrow().clone();
                let mapped = IteratorValue::MappedIter {
                    inner: Box::new(inner),
                    func,
                };
                Ok(Value::Iterator(Rc::new(RefCell::new(mapped))))
            }
            "filter" => {
                let func = match args.into_iter().next() {
                    Some(Value::Function(fv)) => fv,
                    _ => {
                        return Err(RuntimeError::TypeError(
                            ".filter() requires a function argument".into(),
                        )
                        .into());
                    }
                };
                let inner = iter_rc.borrow().clone();
                let filtered = IteratorValue::FilterIter {
                    inner: Box::new(inner),
                    func,
                };
                Ok(Value::Iterator(Rc::new(RefCell::new(filtered))))
            }
            "take" => {
                let n = match args.first() {
                    Some(Value::Int(n)) => *n as usize,
                    _ => {
                        return Err(RuntimeError::TypeError(
                            ".take() requires an integer argument".into(),
                        )
                        .into());
                    }
                };
                let inner = iter_rc.borrow().clone();
                let taken = IteratorValue::TakeIter {
                    inner: Box::new(inner),
                    remaining: n,
                };
                Ok(Value::Iterator(Rc::new(RefCell::new(taken))))
            }
            "enumerate" => {
                let inner = iter_rc.borrow().clone();
                let enumerated = IteratorValue::EnumerateIter {
                    inner: Box::new(inner),
                    index: 0,
                };
                Ok(Value::Iterator(Rc::new(RefCell::new(enumerated))))
            }
            "collect" => {
                let mut result = Vec::new();
                while let Some(v) = self.iter_next(&iter_rc)? {
                    result.push(v);
                }
                Ok(Value::Array(result))
            }
            "sum" => {
                let mut total: i64 = 0;
                while let Some(v) = self.iter_next(&iter_rc)? {
                    match v {
                        Value::Int(n) => total += n,
                        Value::Float(f) => total += f as i64,
                        _ => {
                            return Err(RuntimeError::TypeError(
                                ".sum() requires numeric iterator".into(),
                            )
                            .into());
                        }
                    }
                }
                Ok(Value::Int(total))
            }
            "count" => {
                let mut n: i64 = 0;
                while self.iter_next(&iter_rc)?.is_some() {
                    n += 1;
                }
                Ok(Value::Int(n))
            }
            "fold" => {
                if args.len() < 2 {
                    return Err(RuntimeError::TypeError(
                        ".fold() requires init value and function".into(),
                    )
                    .into());
                }
                let mut acc = args[0].clone();
                let func = match &args[1] {
                    Value::Function(fv) => fv.clone(),
                    _ => {
                        return Err(RuntimeError::TypeError(
                            ".fold() second argument must be a function".into(),
                        )
                        .into());
                    }
                };
                while let Some(v) = self.iter_next(&iter_rc)? {
                    acc = self.call_function(&func, vec![acc, v])?;
                }
                Ok(acc)
            }
            _ => Err(RuntimeError::TypeError(format!("no method '{method}' on iterator")).into()),
        }
    }

    /// Coerces a concrete value into a trait object (`dyn Trait`).
    ///
    /// Builds a vtable by looking up all trait methods in impl_methods for the
    /// concrete type.
    pub(super) fn coerce_to_trait_object(&self, val: Value, trait_name: &str) -> EvalResult {
        let concrete_type = match &val {
            Value::Struct { name, .. } => name.clone(),
            _ => {
                return Err(RuntimeError::TypeError(format!(
                    "cannot coerce {} to dyn {trait_name} (only structs can be trait objects)",
                    match &val {
                        Value::Int(_) => "int",
                        Value::Float(_) => "float",
                        Value::Bool(_) => "bool",
                        Value::Str(_) => "str",
                        _ => "value",
                    }
                ))
                .into());
            }
        };

        // Look up trait method names
        let method_names = match self.trait_defs.get(trait_name) {
            Some(names) => names.clone(),
            None => {
                return Err(
                    RuntimeError::TypeError(format!("unknown trait '{trait_name}'")).into(),
                );
            }
        };

        // Verify this type implements the trait
        if !self
            .trait_impls
            .contains(&(trait_name.to_string(), concrete_type.clone()))
        {
            return Err(RuntimeError::TypeError(format!(
                "type '{concrete_type}' does not implement trait '{trait_name}'"
            ))
            .into());
        }

        // Build vtable from impl_methods
        let mut vtable = HashMap::new();
        for method in &method_names {
            let key = (concrete_type.clone(), method.clone());
            if let Some(fv) = self.impl_methods.get(&key) {
                vtable.insert(method.clone(), fv.clone());
            }
        }

        Ok(Value::TraitObject {
            trait_name: trait_name.to_string(),
            concrete: Box::new(val),
            concrete_type,
            vtable,
        })
    }

    /// Evaluates a module declaration: `mod name { items }` or `mod name;`.
    ///
    /// For inline modules (body=Some), items are evaluated directly.
    /// For file-based modules (body=None), resolves `name.fj` from the source
    /// directory, parses it, and evaluates the resulting items.
    ///
    /// Each symbol is registered in the global environment under its qualified
    /// name (e.g., `math::square`) and stored in `self.modules[name]`.
    pub(super) fn eval_mod_decl(&mut self, mod_decl: &ModDecl) -> EvalResult {
        let mod_name = &mod_decl.name;

        // For file-based modules, check/track loading state across the full lifecycle
        let is_file_module = mod_decl.body.is_none();
        if is_file_module {
            if self.loading_modules.contains(mod_name) {
                return Err(RuntimeError::Unsupported(format!(
                    "circular module dependency detected: '{mod_name}'"
                ))
                .into());
            }
            self.loading_modules.insert(mod_name.to_string());
        }

        let items = match &mod_decl.body {
            Some(items) => items.clone(),
            None => self.resolve_file_module(mod_name)?,
        };

        let result = self.eval_mod_items(mod_name, &items);

        if is_file_module {
            self.loading_modules.remove(mod_name);
        }

        result
    }

    /// Resolves a file-based module (`mod name;`) to its parsed items.
    ///
    /// Searches for `name.fj` in the source directory and stdlib path.
    /// Detects circular dependencies.
    fn resolve_file_module(&mut self, mod_name: &str) -> Result<Vec<Item>, EvalError> {
        let source_dir = self
            .source_dir
            .clone()
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

        let file_path = source_dir.join(format!("{mod_name}.fj"));
        if !file_path.exists() {
            return Err(RuntimeError::Unsupported(format!(
                "[PE011] module file not found: '{}'",
                file_path.display()
            ))
            .into());
        }

        let source = std::fs::read_to_string(&file_path).map_err(|e| {
            RuntimeError::Unsupported(format!("cannot read module '{}': {e}", file_path.display()))
        })?;

        let tokens = crate::lexer::tokenize(&source).map_err(|errors| {
            let msg = errors
                .iter()
                .map(|e| e.to_string())
                .collect::<Vec<_>>()
                .join("; ");
            RuntimeError::Unsupported(format!(
                "lex error in module '{}': {msg}",
                file_path.display()
            ))
        })?;

        let program = crate::parser::parse(tokens).map_err(|errors| {
            let msg = errors
                .iter()
                .map(|e| e.to_string())
                .collect::<Vec<_>>()
                .join("; ");
            RuntimeError::Unsupported(format!(
                "parse error in module '{}': {msg}",
                file_path.display()
            ))
        })?;

        // Set source_dir to module file's directory for nested module resolution
        let old_dir = self.source_dir.clone();
        if let Some(parent) = file_path.parent() {
            self.source_dir = Some(parent.to_path_buf());
        }

        let items = program.items;

        // Restore source_dir
        self.source_dir = old_dir;

        Ok(items)
    }

    /// Evaluates a list of items belonging to a module.
    fn eval_mod_items(&mut self, mod_name: &str, items: &[Item]) -> EvalResult {
        let mut mod_symbols: HashMap<String, Value> = HashMap::new();
        let mut pub_items: HashSet<String> = HashSet::new();

        for item in items {
            match item {
                Item::FnDef(fndef) => {
                    let fn_val = FnValue {
                        name: fndef.name.clone(),
                        params: fndef.params.clone(),
                        body: fndef.body.clone(),
                        closure_env: Rc::clone(&self.env),
                        is_async: false,
                    };
                    let val = Value::Function(fn_val);
                    mod_symbols.insert(fndef.name.clone(), val.clone());
                    if fndef.is_pub {
                        pub_items.insert(fndef.name.clone());
                    }
                    let qualified = format!("{}::{}", mod_name, fndef.name);
                    self.env.borrow_mut().define(qualified, val);
                }
                Item::StructDef(sdef) => {
                    let val = Value::Str(format!("struct:{}", sdef.name));
                    mod_symbols.insert(sdef.name.clone(), val);
                    if sdef.is_pub {
                        pub_items.insert(sdef.name.clone());
                    }
                }
                Item::ConstDef(cdef) => {
                    let val = self.eval_expr(&cdef.value)?;
                    mod_symbols.insert(cdef.name.clone(), val.clone());
                    if cdef.is_pub {
                        pub_items.insert(cdef.name.clone());
                    }
                    let qualified = format!("{}::{}", mod_name, cdef.name);
                    self.env.borrow_mut().define(qualified, val);
                }
                Item::ModDecl(inner_mod) => {
                    // Nested module: evaluate and store with nested qualified names
                    self.eval_mod_decl(inner_mod)?;
                    if let Some(inner_symbols) = self.modules.get(&inner_mod.name).cloned() {
                        let nested_name = format!("{}::{}", mod_name, inner_mod.name);
                        for (sym_name, sym_val) in &inner_symbols {
                            let qualified = format!("{}::{}", nested_name, sym_name);
                            self.env.borrow_mut().define(qualified, sym_val.clone());
                        }
                        self.modules.insert(nested_name, inner_symbols);
                    }
                }
                Item::EnumDef(edef) => {
                    if edef.is_pub {
                        for variant in &edef.variants {
                            pub_items.insert(variant.name.clone());
                        }
                    }
                    for variant in &edef.variants {
                        if variant.fields.is_empty() {
                            let val = Value::Enum {
                                variant: variant.name.clone(),
                                data: None,
                            };
                            mod_symbols.insert(variant.name.clone(), val.clone());
                            let qualified = format!("{}::{}", mod_name, variant.name);
                            self.env.borrow_mut().define(qualified, val);
                        }
                    }
                }
                Item::ImplBlock(impl_block) => {
                    self.eval_impl_block(impl_block)?;
                }
                _ => {
                    self.eval_item(item)?;
                }
            }
        }

        self.modules.insert(mod_name.to_string(), mod_symbols);
        self.module_pub_items
            .insert(mod_name.to_string(), pub_items);
        Ok(Value::Null)
    }

    /// Checks if a symbol is accessible from outside a module.
    ///
    /// If the module has any `pub` items, only `pub` items are accessible.
    /// If the module has NO `pub` items (legacy), all items are accessible.
    fn is_item_visible(&self, mod_path: &str, item_name: &str) -> bool {
        match self.module_pub_items.get(mod_path) {
            Some(pub_set) if !pub_set.is_empty() => pub_set.contains(item_name),
            _ => true, // legacy: no pub markers → everything visible
        }
    }

    /// Evaluates a use declaration: `use path::item`, `use path::*`, `use path::{a, b}`.
    ///
    /// Imports symbols from a registered module into the current scope.
    /// Respects `pub` visibility: only public items can be imported.
    pub(super) fn eval_use_decl(&mut self, use_decl: &UseDecl) -> EvalResult {
        let path = &use_decl.path;

        match &use_decl.kind {
            UseKind::Simple => {
                // `use math::square` — import the last segment
                if path.len() >= 2 {
                    let mod_path = path[..path.len() - 1].join("::");
                    let item_name = &path[path.len() - 1];
                    if !self.is_item_visible(&mod_path, item_name) {
                        return Err(RuntimeError::TypeError(format!(
                            "'{item_name}' is private in module '{mod_path}'"
                        ))
                        .into());
                    }
                    let qualified = format!("{}::{}", mod_path, item_name);
                    let resolved = self.env.borrow().lookup(&qualified).or_else(|| {
                        self.modules
                            .get(&mod_path)
                            .and_then(|m| m.get(item_name).cloned())
                    });
                    if let Some(val) = resolved {
                        self.env.borrow_mut().define(item_name.clone(), val);
                    }
                }
                Ok(Value::Null)
            }
            UseKind::Glob => {
                // `use math::*` — import all PUBLIC symbols from module
                let mod_path = path.join("::");
                if let Some(mod_syms) = self.modules.get(&mod_path).cloned() {
                    for (name, val) in mod_syms {
                        if self.is_item_visible(&mod_path, &name) {
                            self.env.borrow_mut().define(name, val);
                        }
                    }
                }
                Ok(Value::Null)
            }
            UseKind::Group(names) => {
                // `use math::{square, cube}` — import specific items
                let mod_path = path.join("::");
                let mut imports = Vec::new();
                for name in names {
                    if !self.is_item_visible(&mod_path, name) {
                        return Err(RuntimeError::TypeError(format!(
                            "'{name}' is private in module '{mod_path}'"
                        ))
                        .into());
                    }
                    let qualified = format!("{}::{}", mod_path, name);
                    let resolved = self.env.borrow().lookup(&qualified).or_else(|| {
                        self.modules
                            .get(&mod_path)
                            .and_then(|m| m.get(name).cloned())
                    });
                    if let Some(val) = resolved {
                        imports.push((name.clone(), val));
                    }
                }
                for (name, val) in imports {
                    self.env.borrow_mut().define(name, val);
                }
                Ok(Value::Null)
            }
        }
    }

    /// Extracts an array of i64 values from a Value::Array.
    fn extract_i64_array(&self, val: &Value, fn_name: &str) -> Result<Vec<i64>, EvalError> {
        match val {
            Value::Array(arr) => {
                let mut result = Vec::with_capacity(arr.len());
                for v in arr {
                    match v {
                        Value::Int(n) => result.push(*n),
                        _ => {
                            return Err(RuntimeError::TypeError(format!(
                                "{fn_name} requires array of integers"
                            ))
                            .into());
                        }
                    }
                }
                Ok(result)
            }
            _ => Err(RuntimeError::TypeError(format!("{fn_name} requires array argument")).into()),
        }
    }

    /// Extract single i64 from args[0].
    #[allow(dead_code)]
    fn extract_i64(&self, args: &[Value], fn_name: &str) -> Result<i64, EvalError> {
        match args.first() {
            Some(Value::Int(n)) => Ok(*n),
            _ => Err(RuntimeError::TypeError(format!("{fn_name}: expected i64 argument")).into()),
        }
    }

    // ── WebSocket builtins ──

    /// ws_connect(url: str) -> i64
    ///
    /// Opens a simulated WebSocket connection. Returns a handle integer.
    fn builtin_ws_connect(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        let url = match &args[0] {
            Value::Str(s) => s.clone(),
            _ => {
                return Err(
                    RuntimeError::TypeError("ws_connect: expected string URL".into()).into(),
                );
            }
        };
        let id = self.next_ws_id;
        self.next_ws_id += 1;
        self.ws_connections.insert(
            id,
            super::WsConnection {
                url,
                connected: true,
                send_buffer: Vec::new(),
                recv_buffer: std::collections::VecDeque::new(),
            },
        );
        Ok(Value::Int(id))
    }

    /// ws_send(handle: i64, message: str) -> i64
    ///
    /// Sends a message over a WebSocket connection. In simulation the message
    /// is echo'd into the recv buffer. Returns the message length.
    fn builtin_ws_send(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into());
        }
        let handle = match &args[0] {
            Value::Int(h) => *h,
            _ => return Err(RuntimeError::TypeError("ws_send: expected int handle".into()).into()),
        };
        let message = match &args[1] {
            Value::Str(s) => s.clone(),
            _ => {
                return Err(
                    RuntimeError::TypeError("ws_send: expected string message".into()).into(),
                );
            }
        };
        let conn = self
            .ws_connections
            .get_mut(&handle)
            .ok_or_else(|| RuntimeError::TypeError(format!("ws_send: invalid handle {handle}")))?;
        if !conn.connected {
            return Err(RuntimeError::TypeError("ws_send: connection closed".into()).into());
        }
        // In simulation, sent messages echo back to recv buffer.
        conn.recv_buffer.push_back(message.clone());
        let len = message.len() as i64;
        conn.send_buffer.push(message);
        Ok(Value::Int(len))
    }

    /// ws_recv(handle: i64) -> str | null
    ///
    /// Receives the next pending message. Returns `null` if no message is queued.
    fn builtin_ws_recv(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        let handle = match &args[0] {
            Value::Int(h) => *h,
            _ => return Err(RuntimeError::TypeError("ws_recv: expected int handle".into()).into()),
        };
        let conn = self
            .ws_connections
            .get_mut(&handle)
            .ok_or_else(|| RuntimeError::TypeError(format!("ws_recv: invalid handle {handle}")))?;
        match conn.recv_buffer.pop_front() {
            Some(msg) => Ok(Value::Str(msg)),
            None => Ok(Value::Null),
        }
    }

    /// ws_close(handle: i64) -> null
    ///
    /// Closes the WebSocket connection and releases the handle.
    fn builtin_ws_close(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        let handle = match &args[0] {
            Value::Int(h) => *h,
            _ => {
                return Err(RuntimeError::TypeError("ws_close: expected int handle".into()).into());
            }
        };
        if let Some(conn) = self.ws_connections.get_mut(&handle) {
            conn.connected = false;
        }
        self.ws_connections.remove(&handle);
        Ok(Value::Null)
    }

    // ── MQTT builtins ──

    /// mqtt_connect(broker: str) -> i64
    ///
    /// Connects to a simulated MQTT broker. Returns a client handle.
    fn builtin_mqtt_connect(&mut self, args: Vec<Value>) -> EvalResult {
        if args.is_empty() {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: 0,
            }
            .into());
        }
        let broker = match &args[0] {
            Value::Str(s) => s.clone(),
            _ => {
                return Err(RuntimeError::TypeError(
                    "mqtt_connect: expected string broker address".into(),
                )
                .into());
            }
        };
        let id = self.next_mqtt_id;
        self.next_mqtt_id += 1;
        self.mqtt_clients.insert(
            id,
            super::MqttClientState {
                broker_addr: broker,
                connected: true,
                subscriptions: Vec::new(),
            },
        );
        Ok(Value::Int(id))
    }

    /// mqtt_publish(handle: i64, topic: str, payload: str) -> null
    ///
    /// Publishes `payload` to `topic` on the in-memory broker.
    fn builtin_mqtt_publish(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 3 {
            return Err(RuntimeError::ArityMismatch {
                expected: 3,
                got: args.len(),
            }
            .into());
        }
        let handle = match &args[0] {
            Value::Int(h) => *h,
            _ => {
                return Err(
                    RuntimeError::TypeError("mqtt_publish: expected int handle".into()).into(),
                );
            }
        };
        let topic = match &args[1] {
            Value::Str(s) => s.clone(),
            _ => {
                return Err(
                    RuntimeError::TypeError("mqtt_publish: expected string topic".into()).into(),
                );
            }
        };
        let payload = match &args[2] {
            Value::Str(s) => s.clone(),
            _ => {
                return Err(RuntimeError::TypeError(
                    "mqtt_publish: expected string payload".into(),
                )
                .into());
            }
        };
        let client = self.mqtt_clients.get(&handle).ok_or_else(|| {
            RuntimeError::TypeError(format!("mqtt_publish: invalid handle {handle}"))
        })?;
        if !client.connected {
            return Err(RuntimeError::TypeError("mqtt_publish: not connected".into()).into());
        }
        self.mqtt_broker.publish(&topic, &payload);
        Ok(Value::Null)
    }

    /// mqtt_subscribe(handle: i64, topic: str) -> null
    ///
    /// Subscribes the client to `topic`. Future `mqtt_publish` calls to that
    /// topic will be delivered via `mqtt_recv`.
    fn builtin_mqtt_subscribe(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into());
        }
        let handle = match &args[0] {
            Value::Int(h) => *h,
            _ => {
                return Err(
                    RuntimeError::TypeError("mqtt_subscribe: expected int handle".into()).into(),
                );
            }
        };
        let topic = match &args[1] {
            Value::Str(s) => s.clone(),
            _ => {
                return Err(RuntimeError::TypeError(
                    "mqtt_subscribe: expected string topic".into(),
                )
                .into());
            }
        };
        let client = self.mqtt_clients.get_mut(&handle).ok_or_else(|| {
            RuntimeError::TypeError(format!("mqtt_subscribe: invalid handle {handle}"))
        })?;
        if !client.connected {
            return Err(RuntimeError::TypeError("mqtt_subscribe: not connected".into()).into());
        }
        client.subscriptions.push(topic.clone());
        self.mqtt_broker.subscribe(handle, &topic);
        Ok(Value::Null)
    }

    /// mqtt_recv(handle: i64) -> Map | null
    ///
    /// Returns the next queued message as `{ "topic": str, "payload": str }`,
    /// or `null` if no message is pending.
    fn builtin_mqtt_recv(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        let handle = match &args[0] {
            Value::Int(h) => *h,
            _ => {
                return Err(
                    RuntimeError::TypeError("mqtt_recv: expected int handle".into()).into(),
                );
            }
        };
        if !self.mqtt_clients.contains_key(&handle) {
            return Err(
                RuntimeError::TypeError(format!("mqtt_recv: invalid handle {handle}")).into(),
            );
        }
        match self.mqtt_broker.receive(handle) {
            Some((topic, payload)) => {
                let mut map = std::collections::HashMap::new();
                map.insert("topic".to_string(), Value::Str(topic));
                map.insert("payload".to_string(), Value::Str(payload));
                Ok(Value::Map(map))
            }
            None => Ok(Value::Null),
        }
    }

    /// mqtt_disconnect(handle: i64) -> null
    ///
    /// Disconnects the MQTT client and releases the handle.
    fn builtin_mqtt_disconnect(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        let handle = match &args[0] {
            Value::Int(h) => *h,
            _ => {
                return Err(
                    RuntimeError::TypeError("mqtt_disconnect: expected int handle".into()).into(),
                );
            }
        };
        self.mqtt_broker.unsubscribe_all(handle);
        self.mqtt_clients.remove(&handle);
        Ok(Value::Null)
    }

    // ── BLE builtins ──────────────────────────────────────────────

    /// ble_scan() -> array of {addr, name} maps
    fn builtin_ble_scan(&mut self, args: Vec<Value>) -> EvalResult {
        if !args.is_empty() {
            return Err(RuntimeError::ArityMismatch {
                expected: 0,
                got: args.len(),
            }
            .into());
        }
        let devices = self.ble_adapter.scan();
        let result: Vec<Value> = devices
            .into_iter()
            .map(|(addr, name)| {
                let mut map = HashMap::new();
                map.insert("addr".to_string(), Value::Str(addr));
                map.insert("name".to_string(), Value::Str(name));
                Value::Map(map)
            })
            .collect();
        Ok(Value::Array(result))
    }

    /// ble_connect(addr: str) -> handle (i64) or -1 on failure
    fn builtin_ble_connect(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        let addr = match &args[0] {
            Value::Str(s) => s.clone(),
            _ => {
                return Err(
                    RuntimeError::TypeError("ble_connect: expected string address".into()).into(),
                );
            }
        };
        match self.ble_adapter.connect(&addr) {
            Some(handle) => Ok(Value::Int(handle)),
            None => Ok(Value::Int(-1)),
        }
    }

    /// ble_read(handle: i64, uuid: str) -> array of bytes or null
    fn builtin_ble_read(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into());
        }
        let handle = match &args[0] {
            Value::Int(h) => *h,
            _ => {
                return Err(RuntimeError::TypeError("ble_read: expected int handle".into()).into());
            }
        };
        let uuid = match &args[1] {
            Value::Str(s) => s.clone(),
            _ => {
                return Err(
                    RuntimeError::TypeError("ble_read: expected string UUID".into()).into(),
                );
            }
        };
        match self.ble_adapter.read(handle, &uuid) {
            Some(bytes) => {
                let arr: Vec<Value> = bytes.into_iter().map(|b| Value::Int(b as i64)).collect();
                Ok(Value::Array(arr))
            }
            None => Ok(Value::Null),
        }
    }

    /// ble_write(handle: i64, uuid: str, data: array) -> bool success
    fn builtin_ble_write(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 3 {
            return Err(RuntimeError::ArityMismatch {
                expected: 3,
                got: args.len(),
            }
            .into());
        }
        let handle = match &args[0] {
            Value::Int(h) => *h,
            _ => {
                return Err(
                    RuntimeError::TypeError("ble_write: expected int handle".into()).into(),
                );
            }
        };
        let uuid = match &args[1] {
            Value::Str(s) => s.clone(),
            _ => {
                return Err(
                    RuntimeError::TypeError("ble_write: expected string UUID".into()).into(),
                );
            }
        };
        let data = match &args[2] {
            Value::Array(arr) => arr
                .iter()
                .map(|v| match v {
                    Value::Int(b) => *b as u8,
                    _ => 0,
                })
                .collect(),
            _ => {
                return Err(
                    RuntimeError::TypeError("ble_write: expected array of bytes".into()).into(),
                );
            }
        };
        Ok(Value::Bool(self.ble_adapter.write(handle, &uuid, data)))
    }

    /// ble_disconnect(handle: i64)
    fn builtin_ble_disconnect(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        let handle = match &args[0] {
            Value::Int(h) => *h,
            _ => {
                return Err(
                    RuntimeError::TypeError("ble_disconnect: expected int handle".into()).into(),
                );
            }
        };
        self.ble_adapter.disconnect(handle);
        Ok(Value::Null)
    }

    /// Extract two i64 values from args.
    #[allow(dead_code)]
    fn extract_2i64(&self, args: &[Value], fn_name: &str) -> Result<(i64, i64), EvalError> {
        if args.len() < 2 {
            return Err(RuntimeError::TypeError(format!("{fn_name}: expected 2 arguments")).into());
        }
        let a = match &args[0] {
            Value::Int(n) => *n,
            _ => {
                return Err(
                    RuntimeError::TypeError(format!("{fn_name}: arg 0 must be i64")).into(),
                );
            }
        };
        let b = match &args[1] {
            Value::Int(n) => *n,
            _ => {
                return Err(
                    RuntimeError::TypeError(format!("{fn_name}: arg 1 must be i64")).into(),
                );
            }
        };
        Ok((a, b))
    }

    /// Extract three i64 values from args.
    #[allow(dead_code)]
    fn extract_3i64(&self, args: &[Value], fn_name: &str) -> Result<(i64, i64, i64), EvalError> {
        if args.len() < 3 {
            return Err(RuntimeError::TypeError(format!("{fn_name}: expected 3 arguments")).into());
        }
        let a = match &args[0] {
            Value::Int(n) => *n,
            _ => {
                return Err(
                    RuntimeError::TypeError(format!("{fn_name}: arg 0 must be i64")).into(),
                );
            }
        };
        let b = match &args[1] {
            Value::Int(n) => *n,
            _ => {
                return Err(
                    RuntimeError::TypeError(format!("{fn_name}: arg 1 must be i64")).into(),
                );
            }
        };
        let c = match &args[2] {
            Value::Int(n) => *n,
            _ => {
                return Err(
                    RuntimeError::TypeError(format!("{fn_name}: arg 2 must be i64")).into(),
                );
            }
        };
        Ok((a, b, c))
    }

    /// Extract four i64 values from args.
    #[allow(dead_code)]
    fn extract_4i64(
        &self,
        args: &[Value],
        fn_name: &str,
    ) -> Result<(i64, i64, i64, i64), EvalError> {
        if args.len() < 4 {
            return Err(RuntimeError::TypeError(format!("{fn_name}: expected 4 arguments")).into());
        }
        let vals: Result<Vec<i64>, _> = args[..4]
            .iter()
            .enumerate()
            .map(|(i, v)| match v {
                Value::Int(n) => Ok(*n),
                _ => Err(EvalError::from(RuntimeError::TypeError(format!(
                    "{fn_name}: arg {i} must be i64"
                )))),
            })
            .collect();
        let v = vals?;
        Ok((v[0], v[1], v[2], v[3]))
    }
}

/// Check if a field name is a known method on a type (for "did you mean X()?" hints).
fn is_known_method_name(type_name: &str, field: &str) -> bool {
    match type_name {
        "str" => matches!(
            field,
            "len"
                | "trim"
                | "to_uppercase"
                | "to_lowercase"
                | "chars"
                | "split"
                | "contains"
                | "starts_with"
                | "ends_with"
                | "replace"
                | "substring"
                | "parse_int"
                | "parse_float"
        ),
        "array" => matches!(
            field,
            "len"
                | "push"
                | "pop"
                | "sort"
                | "reverse"
                | "contains"
                | "iter"
                | "map"
                | "filter"
                | "collect"
        ),
        _ => false,
    }
}
