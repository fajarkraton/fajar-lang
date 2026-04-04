//! Built-in function implementations for the Fajar Lang interpreter.
//!
//! Contains `call_builtin()` dispatch and all `builtin_*` implementation functions
//! for OS/HAL, tensor, GPU, timing, file I/O, and FajarOS Phase 3-8 builtins.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

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
        // Strict mode: reject simulated builtins
        if self.strict_mode && Self::is_simulated(name) {
            return Err(RuntimeError::TypeError(format!(
                "{name}() is simulated and not available in --strict mode. \
                 Use real hardware dispatch or remove the call."
            ))
            .into());
        }
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
                self.record_output(&output);
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
            "type_of" | "const_type_name" => {
                if args.len() != 1 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 1,
                        got: args.len(),
                    }
                    .into());
                }
                Ok(Value::Str(args[0].type_name().to_string()))
            }
            "const_field_names" => {
                if args.len() != 1 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 1,
                        got: args.len(),
                    }
                    .into());
                }
                match &args[0] {
                    Value::Struct { fields, .. } => {
                        let names: Vec<Value> =
                            fields.keys().map(|k| Value::Str(k.clone())).collect();
                        Ok(Value::Array(names))
                    }
                    Value::Map(m) => {
                        let names: Vec<Value> = m.keys().map(|k| Value::Str(k.clone())).collect();
                        Ok(Value::Array(names))
                    }
                    _ => Ok(Value::Array(vec![])),
                }
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
            "tensor_eye" | "eye" => self.builtin_tensor_eye(args),
            "tensor_full" => self.builtin_tensor_full(args),
            "tensor_from_data" | "from_data" => self.builtin_tensor_from_data(args),
            "tensor_shape" | "shape" => self.builtin_tensor_shape(args),
            "tensor_reshape" | "reshape" => self.builtin_tensor_reshape(args),
            "tensor_numel" => self.builtin_tensor_numel(args),
            "tensor_add" => self.builtin_tensor_binop(args, "add"),
            "tensor_sub" => self.builtin_tensor_binop(args, "sub"),
            "tensor_mul" => self.builtin_tensor_binop(args, "mul"),
            "tensor_div" => self.builtin_tensor_binop(args, "div"),
            "tensor_neg" => self.builtin_tensor_neg(args),
            "tensor_matmul" | "matmul" => self.builtin_tensor_matmul(args),
            "tensor_transpose" | "transpose" => self.builtin_tensor_transpose(args),
            "tensor_flatten" | "flatten" => self.builtin_tensor_unary(args, "flatten"),
            "tensor_concat" | "concat" => self.builtin_tensor_concat(args),
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
            "tensor_tanh" | "tanh" => self.builtin_tensor_activation(args, "tanh"),
            "tensor_softmax" | "softmax" => self.builtin_tensor_activation(args, "softmax"),
            "tensor_gelu" | "gelu" => self.builtin_tensor_activation(args, "gelu"),
            "tensor_leaky_relu" | "leaky_relu" => self.builtin_tensor_leaky_relu(args),
            // V20.5 Tier 4: New tensor/scalar operations
            "sign" => self.builtin_sign(args),
            "argmin" => self.builtin_argmin(args),
            "norm" => self.builtin_norm(args),
            "dot" => self.builtin_dot(args),
            "exp_tensor" => self.builtin_exp_tensor(args),
            "log_tensor" => self.builtin_log_tensor(args),
            "sqrt_tensor" => self.builtin_sqrt_tensor(args),
            "abs_tensor" => self.builtin_abs_tensor(args),
            "exp" => self.builtin_exp_scalar(args),
            "gamma" => self.builtin_gamma(args),
            "clamp_tensor" => self.builtin_clamp_tensor(args),
            "where_tensor" => self.builtin_where_tensor(args),
            // GPU discovery
            "gpu_discover" => self.builtin_gpu_discover(args),
            // TurboQuant (FajarQuant Phase 1)
            "turboquant_create" => self.builtin_turboquant_create(args),
            "turboquant_encode" => self.builtin_turboquant_encode(args),
            "turboquant_decode" => self.builtin_turboquant_decode(args),
            "turboquant_inner_product" => self.builtin_turboquant_inner_product(args),
            // FajarQuant Phase 2: Adaptive rotation
            "fajarquant_compare" => self.builtin_fajarquant_compare(args),
            // Loss functions
            "tensor_mse_loss" | "mse_loss" => self.builtin_tensor_loss(args, "mse"),
            "tensor_cross_entropy" | "cross_entropy_loss" | "cross_entropy" => {
                self.builtin_tensor_loss(args, "cross_entropy")
            }
            "tensor_bce_loss" => self.builtin_tensor_loss(args, "bce"),
            "tensor_l1_loss" => self.builtin_tensor_loss(args, "l1"),
            // Quantization
            "quantize_int8" => self.builtin_quantize_int8(args),
            // ── Autograd builtins ──
            "tensor_backward" | "backward" => {
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
            "tensor_grad" | "grad" => {
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
            "tensor_set_requires_grad" | "set_requires_grad" => {
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
            "optimizer_sgd" | "SGD" => {
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
            "optimizer_adam" | "Adam" => {
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
            "optimizer_step" | "optim_step" => {
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
            "optimizer_zero_grad" | "zero_grad" => {
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
            "layer_dense" | "Dense" => {
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
            "layer_conv2d" | "Conv2d" => {
                if args.len() < 3 || args.len() > 5 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 3, // minimum: in_channels, out_channels, kernel_size
                        got: args.len(),
                    }
                    .into());
                }
                let extract_usize = |idx: usize, name: &str| -> Result<usize, EvalError> {
                    match &args[idx] {
                        Value::Int(n) => Ok(*n as usize),
                        _ => Err(
                            RuntimeError::TypeError(format!("Conv2d: {name} must be int")).into(),
                        ),
                    }
                };
                let in_ch = extract_usize(0, "in_channels")?;
                let out_ch = extract_usize(1, "out_channels")?;
                let kernel = extract_usize(2, "kernel_size")?;
                let stride = if args.len() > 3 {
                    extract_usize(3, "stride")?
                } else {
                    1
                };
                let padding = if args.len() > 4 {
                    extract_usize(4, "padding")?
                } else {
                    0
                };
                Ok(Value::Layer(Box::new(LayerValue::Conv2d(
                    crate::runtime::ml::layers::Conv2d::new(in_ch, out_ch, kernel, stride, padding),
                ))))
            }
            "MultiHeadAttention" | "attention" => {
                // MultiHeadAttention(d_model, num_heads)
                if args.len() != 2 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 2,
                        got: args.len(),
                    }
                    .into());
                }
                let d_model = match &args[0] {
                    Value::Int(n) => *n as usize,
                    _ => {
                        return Err(RuntimeError::TypeError(
                            "MultiHeadAttention: d_model must be int".into(),
                        )
                        .into());
                    }
                };
                let num_heads = match &args[1] {
                    Value::Int(n) => *n as usize,
                    _ => {
                        return Err(RuntimeError::TypeError(
                            "MultiHeadAttention: num_heads must be int".into(),
                        )
                        .into());
                    }
                };
                Ok(Value::Layer(Box::new(LayerValue::Attention(Box::new(
                    crate::runtime::ml::layers::MultiHeadAttention::new(d_model, num_heads),
                )))))
            }
            "layer_forward" | "forward" => {
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
                    LayerValue::Conv2d(conv) => {
                        let output = conv
                            .forward(input)
                            .map_err(|e| RuntimeError::TypeError(format!("forward failed: {e}")))?;
                        Ok(Value::Tensor(output))
                    }
                    LayerValue::Attention(attn) => {
                        // Self-attention: Q=K=V=input
                        let output = attn.forward(input, input, input).map_err(|e| {
                            RuntimeError::TypeError(format!("attention forward: {e}"))
                        })?;
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
                        LayerValue::Conv2d(conv) => {
                            let params: Vec<Value> = vec![
                                Value::Tensor(conv.weight.clone()),
                                Value::Tensor(conv.bias.clone()),
                            ];
                            Ok(Value::Array(params))
                        }
                        LayerValue::Attention(attn) => {
                            let params: Vec<Value> = vec![
                                Value::Tensor(attn.w_q.clone()),
                                Value::Tensor(attn.w_k.clone()),
                                Value::Tensor(attn.w_v.clone()),
                                Value::Tensor(attn.w_o.clone()),
                            ];
                            Ok(Value::Array(params))
                        }
                    },
                    _ => {
                        Err(RuntimeError::TypeError("layer_params requires a layer".into()).into())
                    }
                }
            }
            // V20 3.1: Diffusion model creation (upgraded: real UNet)
            "diffusion_create" => {
                if args.len() != 1 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 1,
                        got: args.len(),
                    }
                    .into());
                }
                let steps = match &args[0] {
                    Value::Int(n) => *n as usize,
                    _ => {
                        return Err(
                            RuntimeError::TypeError("diffusion_create(steps: i64)".into()).into(),
                        );
                    }
                };
                let schedule = crate::ml_advanced::diffusion::linear_schedule(steps, 0.0001, 0.02);
                // Create real UNet model alongside schedule metadata
                let unet = crate::ml_advanced::diffusion_unet::DiffusionUNet::new(1, 32);
                let param_count = unet.param_count();
                let mut model = std::collections::HashMap::new();
                model.insert("_type".to_string(), Value::Str("DiffusionModel".into()));
                model.insert("steps".to_string(), Value::Int(steps as i64));
                model.insert("backend".to_string(), Value::Str("UNet (real)".into()));
                model.insert("params".to_string(), Value::Int(param_count as i64));
                model.insert(
                    "schedule".to_string(),
                    Value::Str(format!("{}", schedule.schedule_type)),
                );
                model.insert(
                    "alpha_cumprod_last".to_string(),
                    Value::Float(*schedule.alpha_cumprod.last().unwrap_or(&0.0)),
                );
                Ok(Value::Map(model))
            }
            // V20 3.2: Diffusion denoising step (upgraded: real UNet forward)
            "diffusion_denoise" => {
                if args.len() != 3 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 3,
                        got: args.len(),
                    }
                    .into());
                }
                let steps = match &args[0] {
                    Value::Map(m) => match m.get("steps") {
                        Some(Value::Int(n)) => *n as usize,
                        _ => 100,
                    },
                    _ => {
                        return Err(RuntimeError::TypeError(
                            "diffusion_denoise(model, tensor, step)".into(),
                        )
                        .into());
                    }
                };
                let step = match &args[2] {
                    Value::Int(n) => *n as usize,
                    _ => 0,
                };
                // Real UNet forward pass for denoising
                match &args[1] {
                    Value::Tensor(tv) => {
                        // Create UNet and run forward pass
                        let unet = crate::ml_advanced::diffusion_unet::DiffusionUNet::new(1, 32);
                        // Reshape to 4D if needed: [1, 1, H, W]
                        let input = if tv.ndim() == 2 {
                            let shape = tv.shape();
                            crate::runtime::ml::tensor::TensorValue::from_ndarray(
                                tv.data()
                                    .clone()
                                    .into_shape_with_order(ndarray::IxDyn(&[
                                        1, 1, shape[0], shape[1],
                                    ]))
                                    .unwrap_or_else(|_| tv.data().clone()),
                            )
                        } else {
                            tv.clone()
                        };
                        match unet.forward(&input, step) {
                            Ok(result) => Ok(Value::Tensor(result)),
                            Err(_e) => {
                                // Fallback to scaling if UNet shape doesn't match
                                let progress = step as f64 / steps.max(1) as f64;
                                let scale = 1.0 - progress * 0.5;
                                let denoised = tv.data().mapv(|x| x * scale);
                                Ok(Value::Tensor(
                                    crate::runtime::ml::tensor::TensorValue::from_ndarray(denoised),
                                ))
                            }
                        }
                    }
                    _ => Err(RuntimeError::TypeError(
                        "diffusion_denoise: second arg must be tensor".into(),
                    )
                    .into()),
                }
            }
            // V20 3.3: RL agent creation (upgraded: real CartPole + DQN)
            "rl_agent_create" => {
                if args.len() != 2 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 2,
                        got: args.len(),
                    }
                    .into());
                }
                let state_dim = match &args[0] {
                    Value::Int(n) => *n as usize,
                    _ => {
                        return Err(RuntimeError::TypeError(
                            "rl_agent_create(state_dim, action_dim)".into(),
                        )
                        .into());
                    }
                };
                let action_dim = match &args[1] {
                    Value::Int(n) => *n as usize,
                    _ => {
                        return Err(RuntimeError::TypeError(
                            "rl_agent_create(state_dim, action_dim)".into(),
                        )
                        .into());
                    }
                };
                // Use real CartPole env when state_dim=4, action_dim=2
                let backend = if state_dim == 4 && action_dim == 2 {
                    "CartPole (real physics)"
                } else {
                    "generic"
                };
                let env = crate::ml_advanced::reinforcement::Environment::new(
                    "agent", state_dim, action_dim, 200,
                );
                let mut agent = std::collections::HashMap::new();
                agent.insert("_type".to_string(), Value::Str("RLAgent".into()));
                agent.insert("backend".to_string(), Value::Str(backend.into()));
                agent.insert("state_dim".to_string(), Value::Int(state_dim as i64));
                agent.insert("action_dim".to_string(), Value::Int(action_dim as i64));
                agent.insert(
                    "state".to_string(),
                    Value::Array(env.state.iter().map(|s| Value::Float(*s)).collect()),
                );
                agent.insert("step".to_string(), Value::Int(0));
                agent.insert("total_reward".to_string(), Value::Float(0.0));
                Ok(Value::Map(agent))
            }
            // V20 3.3b: RL agent step (upgraded: real CartPole physics)
            "rl_agent_step" => {
                if args.len() != 2 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 2,
                        got: args.len(),
                    }
                    .into());
                }
                let (state_dim, action_dim) = match &args[0] {
                    Value::Map(m) => {
                        let sd = match m.get("state_dim") {
                            Some(Value::Int(n)) => *n as usize,
                            _ => 4,
                        };
                        let ad = match m.get("action_dim") {
                            Some(Value::Int(n)) => *n as usize,
                            _ => 2,
                        };
                        (sd, ad)
                    }
                    _ => {
                        return Err(
                            RuntimeError::TypeError("rl_agent_step(agent, action)".into()).into(),
                        );
                    }
                };
                let action = match &args[1] {
                    Value::Int(n) => *n as usize,
                    _ => 0,
                };
                // Use real CartPole physics when state_dim=4, action_dim=2
                let result = if state_dim == 4 && action_dim == 2 {
                    let mut cartpole = crate::ml_advanced::reinforcement::CartPoleEnv::new(200);
                    cartpole.reset(42);
                    cartpole.step(action)
                } else {
                    let mut env = crate::ml_advanced::reinforcement::Environment::new(
                        "agent", state_dim, action_dim, 200,
                    );
                    env.step(action)
                };
                let mut step_result = std::collections::HashMap::new();
                step_result.insert(
                    "state".to_string(),
                    Value::Array(result.state.iter().map(|s| Value::Float(*s)).collect()),
                );
                step_result.insert("reward".to_string(), Value::Float(result.reward));
                step_result.insert("done".to_string(), Value::Bool(result.done));
                Ok(Value::Map(step_result))
            }
            // Metrics builtins
            // ═══════════════════════════════════════════════════════════
            // V20 Phase 4: RT Pipeline Executor
            // ═══════════════════════════════════════════════════════════
            "pipeline_create" => {
                let mut pipe = std::collections::HashMap::new();
                pipe.insert("_type".to_string(), Value::Str("Pipeline".into()));
                pipe.insert("stages".to_string(), Value::Array(vec![]));
                pipe.insert("stage_count".to_string(), Value::Int(0));
                Ok(Value::Map(pipe))
            }
            "pipeline_add_stage" => {
                if args.len() != 3 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 3,
                        got: args.len(),
                    }
                    .into());
                }
                let name = match &args[1] {
                    Value::Str(s) => s.clone(),
                    _ => "stage".to_string(),
                };
                let fn_name = match &args[2] {
                    Value::Str(s) => s.clone(),
                    _ => "identity".to_string(),
                };
                let mut pipe = match &args[0] {
                    Value::Map(m) => m.clone(),
                    _ => {
                        return Err(RuntimeError::TypeError(
                            "pipeline_add_stage(pipe, name, fn_name)".into(),
                        )
                        .into());
                    }
                };
                // Append stage
                let mut stages = match pipe.remove("stages") {
                    Some(Value::Array(a)) => a,
                    _ => vec![],
                };
                let mut stage = std::collections::HashMap::new();
                stage.insert("name".to_string(), Value::Str(name));
                stage.insert("fn".to_string(), Value::Str(fn_name));
                stages.push(Value::Map(stage));
                let count = stages.len() as i64;
                pipe.insert("stages".to_string(), Value::Array(stages));
                pipe.insert("stage_count".to_string(), Value::Int(count));
                Ok(Value::Map(pipe))
            }
            "pipeline_run" => {
                if args.len() != 2 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 2,
                        got: args.len(),
                    }
                    .into());
                }
                let stages = match &args[0] {
                    Value::Map(m) => match m.get("stages") {
                        Some(Value::Array(a)) => a.clone(),
                        _ => vec![],
                    },
                    _ => {
                        return Err(
                            RuntimeError::TypeError("pipeline_run(pipe, input)".into()).into()
                        );
                    }
                };
                // Run each stage: call fn_name(current_value)
                let mut current = args[1].clone();
                for stage_val in &stages {
                    if let Value::Map(stage) = stage_val {
                        if let Some(Value::Str(fn_name)) = stage.get("fn") {
                            let stage_name = match stage.get("name") {
                                Some(Value::Str(n)) => n.clone(),
                                _ => "?".to_string(),
                            };
                            match self.call_fn(fn_name, vec![current.clone()]) {
                                Ok(result) => {
                                    self.record_output(&format!("[pipeline] {stage_name}: OK"));
                                    if self.capture_output {
                                        self.output.push(format!("[pipeline] {stage_name}: OK"));
                                    } else {
                                        println!("[pipeline] {stage_name}: OK");
                                    }
                                    current = result;
                                }
                                Err(e) => {
                                    return Err(RuntimeError::TypeError(format!(
                                        "pipeline stage '{stage_name}' failed: {e}"
                                    ))
                                    .into());
                                }
                            }
                        }
                    }
                }
                Ok(current)
            }
            // ═══════════════════════════════════════════════════════════
            // V20 Phase 5: Accelerator Dispatch
            // ═══════════════════════════════════════════════════════════
            "accelerate" => {
                if args.len() != 2 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 2,
                        got: args.len(),
                    }
                    .into());
                }
                let fn_name = match &args[0] {
                    Value::Str(s) => s.clone(),
                    _ => {
                        return Err(
                            RuntimeError::TypeError("accelerate(fn_name, input)".into()).into()
                        );
                    }
                };
                // Classify workload
                let (flops, bytes) = match &args[1] {
                    Value::Tensor(tv) => {
                        let n = tv.data().len();
                        (n as u64 * 2, n as u64 * 8)
                    }
                    Value::Array(a) => (a.len() as u64 * 2, a.len() as u64 * 8),
                    _ => (100, 800),
                };
                let wc = crate::accelerator::dispatch::classify_workload(flops, bytes, 1);
                // Real GPU detection via CUDA driver API
                let gpu_info = crate::hw::gpu::GpuDiscovery::detect();
                let (device_str, gpu_name) =
                    if gpu_info.cuda_available && !gpu_info.devices.is_empty() {
                        let dev = &gpu_info.devices[0];
                        match wc {
                            crate::accelerator::dispatch::WorkloadClass::ComputeBound => {
                                ("GPU".to_string(), dev.name.clone())
                            }
                            _ => ("CPU".to_string(), format!("fallback (GPU: {})", dev.name)),
                        }
                    } else {
                        ("CPU".to_string(), "no GPU detected".to_string())
                    };
                // Execute the function on CPU (kernel launch deferred to future)
                let result = self.call_fn(&fn_name, vec![args[1].clone()])?;
                let mut out = std::collections::HashMap::new();
                out.insert("device".to_string(), Value::Str(device_str));
                out.insert("gpu".to_string(), Value::Str(gpu_name));
                out.insert("workload_class".to_string(), Value::Str(format!("{wc:?}")));
                out.insert(
                    "cuda_available".to_string(),
                    Value::Bool(gpu_info.cuda_available),
                );
                out.insert("result".to_string(), result);
                Ok(Value::Map(out))
            }
            // ═══════════════════════════════════════════════════════════
            // V20 Phase 6: Concurrency v2 — Actor Supervision
            // ═══════════════════════════════════════════════════════════
            "actor_spawn" => {
                self.warn_simulated("actor_spawn");
                if args.len() != 2 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 2,
                        got: args.len(),
                    }
                    .into());
                }
                let name = match &args[0] {
                    Value::Str(s) => s.clone(),
                    _ => {
                        return Err(
                            RuntimeError::TypeError("actor_spawn(name, fn_name)".into()).into()
                        );
                    }
                };
                let fn_name = match &args[1] {
                    Value::Str(s) => s.clone(),
                    _ => "handler".to_string(),
                };
                let actor = crate::concurrency_v2::actors::ActorInstance::new(
                    crate::concurrency_v2::actors::ActorAddr(
                        std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_nanos() as u64,
                    ),
                    &name,
                    16,
                );
                let mut m = std::collections::HashMap::new();
                m.insert("_type".to_string(), Value::Str("Actor".into()));
                m.insert("name".to_string(), Value::Str(name));
                m.insert("fn".to_string(), Value::Str(fn_name));
                m.insert("addr".to_string(), Value::Int(actor.addr.0 as i64));
                m.insert(
                    "status".to_string(),
                    Value::Str(format!("{:?}", actor.status)),
                );
                m.insert("restart_count".to_string(), Value::Int(0));
                Ok(Value::Map(m))
            }
            "actor_send" => {
                self.warn_simulated("actor_send");
                if args.len() != 2 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 2,
                        got: args.len(),
                    }
                    .into());
                }
                // Extract actor's handler fn and call it with the message
                let fn_name = match &args[0] {
                    Value::Map(m) => match m.get("fn") {
                        Some(Value::Str(s)) => s.clone(),
                        _ => "handler".to_string(),
                    },
                    _ => {
                        return Err(
                            RuntimeError::TypeError("actor_send(actor, message)".into()).into()
                        );
                    }
                };
                let result = self.call_fn(&fn_name, vec![args[1].clone()])?;
                Ok(result)
            }
            "actor_supervise" => {
                self.warn_simulated("actor_supervise");
                if args.len() != 2 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 2,
                        got: args.len(),
                    }
                    .into());
                }
                let strategy = match &args[1] {
                    Value::Str(s) => s.clone(),
                    _ => "one_for_one".to_string(),
                };
                // Apply supervision: return updated actor with strategy
                let mut actor = match &args[0] {
                    Value::Map(m) => m.clone(),
                    _ => {
                        return Err(RuntimeError::TypeError(
                            "actor_supervise(actor, strategy)".into(),
                        )
                        .into());
                    }
                };
                actor.insert("supervision".to_string(), Value::Str(strategy));
                Ok(Value::Map(actor))
            }
            // ═══════════════════════════════════════════════════════════
            // V20 Phase 7: Const Modules
            // ═══════════════════════════════════════════════════════════
            "const_alloc" => {
                self.warn_simulated("const_alloc");
                if args.len() != 1 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 1,
                        got: args.len(),
                    }
                    .into());
                }
                let size = match &args[0] {
                    Value::Int(n) => *n as usize,
                    _ => {
                        return Err(RuntimeError::TypeError("const_alloc(size: i64)".into()).into());
                    }
                };
                let _target = crate::const_alloc::TargetInfo::x86_64();
                let alloc = crate::const_alloc::ConstAllocation {
                    name: "const_buffer".to_string(),
                    bytes: vec![0u8; size],
                    align: 8,
                    section: ".rodata".to_string(),
                    type_desc: format!("[u8; {size}]"),
                };
                let mut m = std::collections::HashMap::new();
                m.insert("_type".to_string(), Value::Str("ConstAlloc".into()));
                m.insert("size".to_string(), Value::Int(alloc.size() as i64));
                m.insert("align".to_string(), Value::Int(alloc.align as i64));
                m.insert("section".to_string(), Value::Str(alloc.section));
                m.insert("target".to_string(), Value::Str("x86_64".into()));
                Ok(Value::Map(m))
            }
            "const_size_of" => {
                if args.len() != 1 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 1,
                        got: args.len(),
                    }
                    .into());
                }
                let size = match &args[0] {
                    Value::Int(_) => 8,
                    Value::Float(_) => 8,
                    Value::Bool(_) => 1,
                    Value::Char(_) => 4,
                    Value::Str(s) => s.len() as i64 + 24, // ptr + len + cap
                    Value::Array(a) => a.len() as i64 * 8 + 24,
                    Value::Tensor(tv) => tv.data().len() as i64 * 8 + 32,
                    _ => 0,
                };
                Ok(Value::Int(size))
            }
            "const_align_of" => {
                if args.len() != 1 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 1,
                        got: args.len(),
                    }
                    .into());
                }
                let align = match &args[0] {
                    Value::Bool(_) => 1,
                    Value::Char(_) => 4,
                    _ => 8,
                };
                Ok(Value::Int(align))
            }
            "metric_accuracy" | "accuracy" => {
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
            // String free functions (also available as methods)
            "split" => {
                if args.len() != 2 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 2,
                        got: args.len(),
                    }
                    .into());
                }
                match (&args[0], &args[1]) {
                    (Value::Str(s), Value::Str(sep)) => {
                        let parts: Vec<Value> = s
                            .split(sep.as_str())
                            .map(|p| Value::Str(p.to_string()))
                            .collect();
                        Ok(Value::Array(parts))
                    }
                    _ => Err(RuntimeError::TypeError("split(string, separator)".into()).into()),
                }
            }
            "trim" => {
                if args.len() != 1 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 1,
                        got: args.len(),
                    }
                    .into());
                }
                match &args[0] {
                    Value::Str(s) => Ok(Value::Str(s.trim().to_string())),
                    _ => Err(RuntimeError::TypeError("trim(string)".into()).into()),
                }
            }
            "contains" => {
                if args.len() != 2 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 2,
                        got: args.len(),
                    }
                    .into());
                }
                match (&args[0], &args[1]) {
                    (Value::Str(s), Value::Str(sub)) => Ok(Value::Bool(s.contains(sub.as_str()))),
                    _ => Err(RuntimeError::TypeError("contains(string, substring)".into()).into()),
                }
            }
            "starts_with" => {
                if args.len() != 2 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 2,
                        got: args.len(),
                    }
                    .into());
                }
                match (&args[0], &args[1]) {
                    (Value::Str(s), Value::Str(prefix)) => {
                        Ok(Value::Bool(s.starts_with(prefix.as_str())))
                    }
                    _ => Err(RuntimeError::TypeError("starts_with(string, prefix)".into()).into()),
                }
            }
            "ends_with" => {
                if args.len() != 2 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 2,
                        got: args.len(),
                    }
                    .into());
                }
                match (&args[0], &args[1]) {
                    (Value::Str(s), Value::Str(suffix)) => {
                        Ok(Value::Bool(s.ends_with(suffix.as_str())))
                    }
                    _ => Err(RuntimeError::TypeError("ends_with(string, suffix)".into()).into()),
                }
            }
            "replace" => {
                if args.len() != 3 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 3,
                        got: args.len(),
                    }
                    .into());
                }
                match (&args[0], &args[1], &args[2]) {
                    (Value::Str(s), Value::Str(from), Value::Str(to)) => {
                        Ok(Value::Str(s.replace(from.as_str(), to.as_str())))
                    }
                    _ => Err(RuntimeError::TypeError("replace(string, from, to)".into()).into()),
                }
            }
            // File I/O builtins
            "read_file_text" => {
                if args.len() != 1 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 1,
                        got: args.len(),
                    }
                    .into());
                }
                match &args[0] {
                    Value::Str(path) => match std::fs::read_to_string(path) {
                        Ok(content) => Ok(Value::Str(content)),
                        Err(e) => {
                            Err(RuntimeError::TypeError(format!("read_file_text: {e}")).into())
                        }
                    },
                    _ => Err(RuntimeError::TypeError("read_file_text(path: str)".into()).into()),
                }
            }
            "read_file" => {
                if args.len() != 1 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 1,
                        got: args.len(),
                    }
                    .into());
                }
                match &args[0] {
                    Value::Str(path) => {
                        // V15 P3.6: Block path traversal attacks
                        if path.contains("..") {
                            return Ok(Value::Enum {
                                variant: "Err".into(),
                                data: Some(Box::new(Value::Str(
                                    "path traversal blocked: '..' not allowed".into(),
                                ))),
                            });
                        }
                        match std::fs::read_to_string(path) {
                            Ok(content) => Ok(Value::Enum {
                                variant: "Ok".into(),
                                data: Some(Box::new(Value::Str(content))),
                            }),
                            Err(e) => Ok(Value::Enum {
                                variant: "Err".into(),
                                data: Some(Box::new(Value::Str(e.to_string()))),
                            }),
                        }
                    }
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
                        // V15 P3.6: Block path traversal
                        if path.contains("..") {
                            return Ok(Value::Enum {
                                variant: "Err".into(),
                                data: Some(Box::new(Value::Str(
                                    "path traversal blocked: '..' not allowed".into(),
                                ))),
                            });
                        }
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
            // V16 R1: MNIST data loader — loads IDX binary format directly into tensors
            "mnist_load_images" => {
                if args.len() != 2 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 2,
                        got: args.len(),
                    }
                    .into());
                }
                let path = match &args[0] {
                    Value::Str(s) => s.clone(),
                    _ => {
                        return Err(RuntimeError::TypeError(
                            "mnist_load_images: path must be string".into(),
                        )
                        .into());
                    }
                };
                let count = match &args[1] {
                    Value::Int(n) => *n as usize,
                    _ => {
                        return Err(RuntimeError::TypeError(
                            "mnist_load_images: count must be int".into(),
                        )
                        .into());
                    }
                };
                match std::fs::read(&path) {
                    Ok(bytes) => {
                        if bytes.len() < 16 {
                            return Err(RuntimeError::TypeError("Invalid IDX file".into()).into());
                        }
                        let n_images =
                            u32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]) as usize;
                        let rows =
                            u32::from_be_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]) as usize;
                        let cols = u32::from_be_bytes([bytes[12], bytes[13], bytes[14], bytes[15]])
                            as usize;
                        let load_n = count.min(n_images);
                        let img_size = rows * cols;
                        let mut data = ndarray::Array2::<f64>::zeros((load_n, img_size));
                        for i in 0..load_n {
                            for j in 0..img_size {
                                data[[i, j]] = bytes[16 + i * img_size + j] as f64 / 255.0;
                            }
                        }
                        Ok(Value::Tensor(
                            crate::runtime::ml::tensor::TensorValue::from_ndarray(data.into_dyn()),
                        ))
                    }
                    Err(e) => Ok(Value::Enum {
                        variant: "Err".into(),
                        data: Some(Box::new(Value::Str(e.to_string()))),
                    }),
                }
            }
            "mnist_load_labels" => {
                if args.len() != 2 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 2,
                        got: args.len(),
                    }
                    .into());
                }
                let path = match &args[0] {
                    Value::Str(s) => s.clone(),
                    _ => {
                        return Err(RuntimeError::TypeError(
                            "mnist_load_labels: path must be string".into(),
                        )
                        .into());
                    }
                };
                let count = match &args[1] {
                    Value::Int(n) => *n as usize,
                    _ => {
                        return Err(RuntimeError::TypeError(
                            "mnist_load_labels: count must be int".into(),
                        )
                        .into());
                    }
                };
                match std::fs::read(&path) {
                    Ok(bytes) => {
                        if bytes.len() < 8 {
                            return Err(RuntimeError::TypeError("Invalid IDX file".into()).into());
                        }
                        let n_labels =
                            u32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]) as usize;
                        let load_n = count.min(n_labels);
                        let mut labels = Vec::with_capacity(load_n);
                        for i in 0..load_n {
                            labels.push(Value::Int(bytes[8 + i] as i64));
                        }
                        Ok(Value::Array(labels))
                    }
                    Err(e) => Ok(Value::Enum {
                        variant: "Err".into(),
                        data: Some(Box::new(Value::Str(e.to_string()))),
                    }),
                }
            }
            // V16 G1.8: GPU thread indexing builtins (mock values in interpreter)
            "thread_idx" => {
                let _dim = args
                    .first()
                    .and_then(|v| match v {
                        Value::Int(n) => Some(*n),
                        _ => None,
                    })
                    .unwrap_or(0);
                // In interpreter mode, return 0 (mock single-thread)
                Ok(Value::Int(0))
            }
            "block_idx" => Ok(Value::Int(0)),
            "block_dim" => {
                let _dim = args
                    .first()
                    .and_then(|v| match v {
                        Value::Int(n) => Some(*n),
                        _ => None,
                    })
                    .unwrap_or(0);
                Ok(Value::Int(1)) // mock: 1 thread per block
            }
            "grid_dim" => Ok(Value::Int(1)),
            "gpu_sync" => Ok(Value::Null), // barrier sync (no-op in interpreter)
            // V16: Binary file I/O — read/write raw bytes as [i64] arrays
            "read_binary" => {
                if args.len() != 1 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 1,
                        got: args.len(),
                    }
                    .into());
                }
                match &args[0] {
                    Value::Str(path) => match std::fs::read(path) {
                        Ok(bytes) => {
                            let arr: Vec<Value> =
                                bytes.iter().map(|b| Value::Int(*b as i64)).collect();
                            Ok(Value::Enum {
                                variant: "Ok".into(),
                                data: Some(Box::new(Value::Array(arr))),
                            })
                        }
                        Err(e) => Ok(Value::Enum {
                            variant: "Err".into(),
                            data: Some(Box::new(Value::Str(e.to_string()))),
                        }),
                    },
                    _ => Err(RuntimeError::TypeError(
                        "read_binary(path: str) -> Result<[i64], str>".into(),
                    )
                    .into()),
                }
            }
            "write_binary" => {
                if args.len() != 2 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 2,
                        got: args.len(),
                    }
                    .into());
                }
                match (&args[0], &args[1]) {
                    (Value::Str(path), Value::Array(bytes)) => {
                        let data: Vec<u8> = bytes
                            .iter()
                            .map(|v| match v {
                                Value::Int(n) => *n as u8,
                                _ => 0,
                            })
                            .collect();
                        match std::fs::write(path, &data) {
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
                        "write_binary(path: str, bytes: [i64])".into(),
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
            "map_get_or" => {
                if args.len() != 3 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 3,
                        got: args.len(),
                    }
                    .into());
                }
                match (&args[0], &args[1]) {
                    (Value::Map(m), Value::Str(k)) => match m.get(k) {
                        Some(v) => Ok(v.clone()),
                        None => Ok(args[2].clone()),
                    },
                    _ => Err(
                        RuntimeError::TypeError("map_get_or(map, key: str, default)".into()).into(),
                    ),
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
                // spawn(future) → starts task via concurrency_v2 structured scope.
                // Uses AsyncScope to track spawned tasks with well-defined lifetimes.
                if args.len() != 1 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 1,
                        got: args.len(),
                    }
                    .into());
                }
                // Track spawn through structured concurrency scope
                let mut scope = crate::concurrency_v2::scopes::AsyncScope::new();
                let _task_id = scope.spawn("spawned_task");
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
                // V18 1.4: Synchronous HTTP client builtins
                if name == "http_get" {
                    if args.len() != 1 {
                        return Err(RuntimeError::ArityMismatch {
                            expected: 1,
                            got: args.len(),
                        }
                        .into());
                    }
                    if let Value::Str(url) = &args[0] {
                        return self.builtin_http_get_sync(url);
                    }
                    return Err(RuntimeError::TypeError("http_get(url: str) -> str".into()).into());
                }
                if name == "http_post" {
                    if args.len() != 2 {
                        return Err(RuntimeError::ArityMismatch {
                            expected: 2,
                            got: args.len(),
                        }
                        .into());
                    }
                    if let (Value::Str(url), Value::Str(body)) = (&args[0], &args[1]) {
                        return self.builtin_http_post_sync(url, body);
                    }
                    return Err(RuntimeError::TypeError(
                        "http_post(url: str, body: str) -> str".into(),
                    )
                    .into());
                }
                // V18 2.2: DNS resolve
                if name == "dns_resolve" {
                    if args.len() != 1 {
                        return Err(RuntimeError::ArityMismatch {
                            expected: 1,
                            got: args.len(),
                        }
                        .into());
                    }
                    if let Value::Str(hostname) = &args[0] {
                        use std::net::ToSocketAddrs;
                        let addr_str = format!("{hostname}:0");
                        match addr_str.to_socket_addrs() {
                            Ok(mut addrs) => {
                                if let Some(addr) = addrs.next() {
                                    return Ok(Value::Enum {
                                        variant: "Ok".into(),
                                        data: Some(Box::new(Value::Str(addr.ip().to_string()))),
                                    });
                                }
                                return Ok(Value::Enum {
                                    variant: "Err".into(),
                                    data: Some(Box::new(Value::Str("no addresses found".into()))),
                                });
                            }
                            Err(e) => {
                                return Ok(Value::Enum {
                                    variant: "Err".into(),
                                    data: Some(Box::new(Value::Str(format!("dns failed: {e}")))),
                                });
                            }
                        }
                    }
                    return Err(RuntimeError::TypeError(
                        "dns_resolve(hostname: str) -> str".into(),
                    )
                    .into());
                }
                // TQ12.1: HTTP server builtin
                if name == "http_listen" {
                    return self.builtin_http_listen(args);
                }

                // V18 4.5: Channel builtins for actor-style message passing
                if name == "channel_create" {
                    let (tx, rx) = std::sync::mpsc::channel();
                    let id = self.next_channel_id;
                    self.next_channel_id += 1;
                    self.channels.insert(id, (tx, Some(rx)));
                    return Ok(Value::Int(id));
                }
                if name == "channel_send" {
                    if args.len() != 2 {
                        return Err(RuntimeError::ArityMismatch {
                            expected: 2,
                            got: args.len(),
                        }
                        .into());
                    }
                    if let Value::Int(ch_id) = &args[0] {
                        if let Some((tx, _)) = self.channels.get(ch_id) {
                            let val = args[1].clone();
                            let _ = tx.send(val);
                            return Ok(Value::Bool(true));
                        }
                        return Ok(Value::Bool(false));
                    }
                    return Err(
                        RuntimeError::TypeError("channel_send(ch: i64, value)".into()).into(),
                    );
                }
                if name == "channel_recv" {
                    if args.len() != 1 {
                        return Err(RuntimeError::ArityMismatch {
                            expected: 1,
                            got: args.len(),
                        }
                        .into());
                    }
                    if let Value::Int(ch_id) = &args[0] {
                        if let Some((_, Some(rx))) = self.channels.get(ch_id) {
                            match rx.try_recv() {
                                Ok(v) => {
                                    return Ok(Value::Enum {
                                        variant: "Some".into(),
                                        data: Some(Box::new(v)),
                                    });
                                }
                                Err(_) => {
                                    return Ok(Value::Enum {
                                        variant: "None".into(),
                                        data: None,
                                    });
                                }
                            }
                        }
                        return Ok(Value::Enum {
                            variant: "None".into(),
                            data: None,
                        });
                    }
                    return Err(RuntimeError::TypeError("channel_recv(ch: i64)".into()).into());
                }
                // V18 2.8: FFI builtins
                if name == "ffi_load_library" {
                    if args.len() != 1 {
                        return Err(RuntimeError::ArityMismatch {
                            expected: 1,
                            got: args.len(),
                        }
                        .into());
                    }
                    if let Value::Str(path) = &args[0] {
                        match self
                            .ffi_manager
                            .load_library(std::path::Path::new(path.as_str()))
                        {
                            Ok(idx) => {
                                return Ok(Value::Enum {
                                    variant: "Ok".into(),
                                    data: Some(Box::new(Value::Int(idx as i64))),
                                });
                            }
                            Err(e) => {
                                return Ok(Value::Enum {
                                    variant: "Err".into(),
                                    data: Some(Box::new(Value::Str(e))),
                                });
                            }
                        }
                    }
                    return Err(RuntimeError::TypeError(
                        "ffi_load_library(path: str) -> Result".into(),
                    )
                    .into());
                }
                if name == "ffi_register" {
                    // ffi_register(lib_index, name, symbol, param_types, ret_type)
                    if args.len() < 3 {
                        return Err(RuntimeError::ArityMismatch {
                            expected: 3,
                            got: args.len(),
                        }
                        .into());
                    }
                    if let (Value::Int(lib_idx), Value::Str(fn_name), Value::Str(symbol)) =
                        (&args[0], &args[1], &args[2])
                    {
                        // Default: all params i64, return i64
                        use crate::interpreter::ffi::FfiType;
                        let param_count = if args.len() > 3 {
                            if let Value::Int(n) = &args[3] {
                                *n as usize
                            } else {
                                0
                            }
                        } else {
                            0
                        };
                        let param_types = vec![FfiType::I64; param_count];
                        let ret_type = FfiType::I64;
                        match self.ffi_manager.register_function(
                            fn_name,
                            *lib_idx as usize,
                            symbol,
                            param_types,
                            ret_type,
                        ) {
                            Ok(()) => return Ok(Value::Bool(true)),
                            Err(e) => {
                                return Ok(Value::Enum {
                                    variant: "Err".into(),
                                    data: Some(Box::new(Value::Str(e))),
                                });
                            }
                        }
                    }
                    return Err(RuntimeError::TypeError(
                        "ffi_register(lib: i64, name: str, symbol: str, param_count: i64)".into(),
                    )
                    .into());
                }
                if name == "ffi_call" {
                    if args.is_empty() {
                        return Err(RuntimeError::ArityMismatch {
                            expected: 1,
                            got: 0,
                        }
                        .into());
                    }
                    if let Value::Str(fn_name) = &args[0] {
                        let call_args = args[1..].to_vec();
                        match self.ffi_manager.call(fn_name, &call_args) {
                            Ok(v) => {
                                return Ok(Value::Enum {
                                    variant: "Ok".into(),
                                    data: Some(Box::new(v)),
                                });
                            }
                            Err(e) => {
                                return Ok(Value::Enum {
                                    variant: "Err".into(),
                                    data: Some(Box::new(Value::Str(e))),
                                });
                            }
                        }
                    }
                    return Err(RuntimeError::TypeError(
                        "ffi_call(name: str, ...args) -> Result".into(),
                    )
                    .into());
                }
                // V18 1.6: TCP socket builtins
                if name == "tcp_connect" {
                    if args.len() != 1 {
                        return Err(RuntimeError::ArityMismatch {
                            expected: 1,
                            got: args.len(),
                        }
                        .into());
                    }
                    if let Value::Str(addr) = &args[0] {
                        return self.builtin_tcp_connect(addr);
                    }
                    return Err(
                        RuntimeError::TypeError("tcp_connect(addr: str) -> i64".into()).into(),
                    );
                }
                if name == "tcp_send" {
                    if args.len() != 2 {
                        return Err(RuntimeError::ArityMismatch {
                            expected: 2,
                            got: args.len(),
                        }
                        .into());
                    }
                    if let (Value::Int(fd), Value::Str(data)) = (&args[0], &args[1]) {
                        return self.builtin_tcp_send(*fd, data);
                    }
                    return Err(RuntimeError::TypeError(
                        "tcp_send(fd: i64, data: str) -> i64".into(),
                    )
                    .into());
                }
                if name == "tcp_recv" {
                    if args.len() != 1 {
                        return Err(RuntimeError::ArityMismatch {
                            expected: 1,
                            got: args.len(),
                        }
                        .into());
                    }
                    if let Value::Int(fd) = &args[0] {
                        return self.builtin_tcp_recv(*fd);
                    }
                    return Err(RuntimeError::TypeError("tcp_recv(fd: i64) -> str".into()).into());
                }
                if name == "tcp_close" {
                    if args.len() != 1 {
                        return Err(RuntimeError::ArityMismatch {
                            expected: 1,
                            got: args.len(),
                        }
                        .into());
                    }
                    if let Value::Int(fd) = &args[0] {
                        self.tcp_connections.remove(&(*fd as usize));
                        return Ok(Value::Null);
                    }
                    return Err(RuntimeError::TypeError("tcp_close(fd: i64)".into()).into());
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

                // GUI builtins
                if name == "gui_window" {
                    return self.builtin_gui_window(args);
                }
                if name == "gui_label" {
                    return self.builtin_gui_label(args);
                }
                if name == "gui_button" {
                    return self.builtin_gui_button(args);
                }
                if name == "gui_rect" {
                    return self.builtin_gui_rect(args);
                }
                if name == "gui_layout" {
                    return self.builtin_gui_layout(args);
                }

                // Regex builtins
                if name == "regex_match" {
                    return self.builtin_regex_match(args);
                }
                if name == "regex_find" {
                    return self.builtin_regex_find(args);
                }
                if name == "regex_find_all" {
                    return self.builtin_regex_find_all(args);
                }
                if name == "regex_replace" {
                    return self.builtin_regex_replace(args);
                }
                if name == "regex_replace_all" {
                    return self.builtin_regex_replace_all(args);
                }
                if name == "regex_captures" {
                    return self.builtin_regex_captures(args);
                }

                // HTTP framework builtins (V10 P3)
                if name == "http_server" {
                    return self.builtin_http_server(args);
                }
                if name == "http_route" {
                    return self.builtin_http_route(args);
                }
                if name == "http_middleware" {
                    return self.builtin_http_middleware(args);
                }
                if name == "http_start" {
                    return self.builtin_http_start(args);
                }
                if name == "http_start_tls" {
                    return self.builtin_http_start_tls(args);
                }
                if name == "request_json" {
                    return self.builtin_request_json(args);
                }
                if name == "response_json" {
                    return self.builtin_response_json(args);
                }

                // Async builtins (V10)
                if name == "async_sleep" {
                    return self.builtin_async_sleep(args);
                }
                if name == "async_http_get" {
                    return self.builtin_async_http_get(args);
                }
                if name == "async_http_post" {
                    return self.builtin_async_http_post(args);
                }
                if name == "async_spawn" {
                    return self.builtin_async_spawn(args);
                }
                if name == "async_join" {
                    return self.builtin_async_join(args);
                }
                if name == "async_select" {
                    return self.builtin_async_select(args);
                }
                if name == "async_timeout" {
                    return self.builtin_async_timeout(args);
                }

                // V14: Check if this is an effect operation (prefixed with __effect__).
                if let Some(effect_op) = name.strip_prefix("__effect__") {
                    if let Some((effect_name, op_name)) = effect_op.split_once("::") {
                        if self.effect_handler_depth > 0 {
                            // V15: Check replay stack — walk from innermost to outermost
                            // handle, looking for a cached resume value that matches
                            // this effect's identity. This correctly handles nested
                            // handle expressions where different handles catch different
                            // effects.
                            for level in (0..self.effect_replay_stack.len()).rev() {
                                let (ref cache, ref mut idx) = self.effect_replay_stack[level];
                                if *idx < cache.len() {
                                    let (ref eff, ref op, ref val) = cache[*idx];
                                    if eff == effect_name && op == op_name {
                                        let v = val.clone();
                                        self.effect_replay_stack[level].1 += 1;
                                        return Ok(v);
                                    }
                                }
                            }
                            // Not cached at any level — raise the effect.
                            return Err(ControlFlow::EffectPerformed {
                                effect: effect_name.to_string(),
                                op: op_name.to_string(),
                                args,
                            }
                            .into());
                        }
                        // Outside any handle block — execute with default behavior.
                        // Default: IO effects print/read, others return Null.
                        return self.default_effect_handler(effect_name, op_name, args);
                    }
                }

                Err(RuntimeError::Unsupported(format!("unknown builtin '{name}'")).into())
            }
        }
    }

    /// V14: Default effect handler for unhandled effect operations.
    ///
    /// When an effect operation is called outside any `handle` block,
    /// built-in effects get default behavior (e.g., IO prints to stdout).
    /// User-defined effects without a handler return Null.
    fn default_effect_handler(&mut self, effect: &str, op: &str, args: Vec<Value>) -> EvalResult {
        match (effect, op) {
            ("IO", "print") => {
                let text = args.first().map(|v| v.to_string()).unwrap_or_default();
                if self.capture_output {
                    self.output.push(text);
                } else {
                    print!("{text}");
                }
                Ok(Value::Null)
            }
            ("IO", "read") => {
                // Default: return empty string (non-interactive).
                Ok(Value::Str(String::new()))
            }
            ("Panic", "panic") => {
                let msg = args.first().map(|v| v.to_string()).unwrap_or_default();
                Err(RuntimeError::Unsupported(format!("panic: {msg}")).into())
            }
            _ => {
                // User-defined effects without handlers return Null.
                Ok(Value::Null)
            }
        }
    }

    /// TQ12.1: Start an HTTP server that calls a Fajar handler function.
    /// Usage: http_listen(port, max_requests)
    /// Listens on 127.0.0.1:port, accepts max_requests connections,
    /// returns the number of requests served.
    /// V18 1.4: Synchronous HTTP GET using std::net::TcpStream
    fn builtin_http_get_sync(&mut self, url: &str) -> EvalResult {
        use std::io::{BufRead, BufReader, Write};
        use std::net::TcpStream;

        // Parse URL: http://host[:port]/path
        let url = url.trim();
        let without_scheme = url
            .strip_prefix("http://")
            .unwrap_or(url.strip_prefix("https://").unwrap_or(url));
        let (host_port, path) = match without_scheme.find('/') {
            Some(i) => (&without_scheme[..i], &without_scheme[i..]),
            None => (without_scheme, "/"),
        };
        let (host, port) = match host_port.find(':') {
            Some(i) => (
                &host_port[..i],
                host_port[i + 1..].parse::<u16>().unwrap_or(80),
            ),
            None => (host_port, 80),
        };

        let addr = format!("{host}:{port}");
        let mut stream = match TcpStream::connect(&addr) {
            Ok(s) => s,
            Err(e) => {
                return Ok(Value::Enum {
                    variant: "Err".into(),
                    data: Some(Box::new(Value::Str(format!("connect failed: {e}")))),
                });
            }
        };
        let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(10)));

        let request = format!("GET {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n");
        if let Err(e) = stream.write_all(request.as_bytes()) {
            return Ok(Value::Enum {
                variant: "Err".into(),
                data: Some(Box::new(Value::Str(format!("write failed: {e}")))),
            });
        }

        let reader = BufReader::new(&stream);
        let mut body = String::new();
        let mut in_body = false;
        for line in reader.lines() {
            match line {
                Ok(l) => {
                    if in_body {
                        body.push_str(&l);
                        body.push('\n');
                    } else if l.is_empty() {
                        in_body = true;
                    }
                }
                Err(_) => break,
            }
        }
        Ok(Value::Enum {
            variant: "Ok".into(),
            data: Some(Box::new(Value::Str(body))),
        })
    }

    /// V18 1.4: Synchronous HTTP POST using std::net::TcpStream
    fn builtin_http_post_sync(&mut self, url: &str, body: &str) -> EvalResult {
        use std::io::{BufRead, BufReader, Write};
        use std::net::TcpStream;

        let url = url.trim();
        let without_scheme = url
            .strip_prefix("http://")
            .unwrap_or(url.strip_prefix("https://").unwrap_or(url));
        let (host_port, path) = match without_scheme.find('/') {
            Some(i) => (&without_scheme[..i], &without_scheme[i..]),
            None => (without_scheme, "/"),
        };
        let (host, port) = match host_port.find(':') {
            Some(i) => (
                &host_port[..i],
                host_port[i + 1..].parse::<u16>().unwrap_or(80),
            ),
            None => (host_port, 80),
        };

        let addr = format!("{host}:{port}");
        let mut stream = match TcpStream::connect(&addr) {
            Ok(s) => s,
            Err(e) => {
                return Ok(Value::Enum {
                    variant: "Err".into(),
                    data: Some(Box::new(Value::Str(format!("connect failed: {e}")))),
                });
            }
        };
        let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(10)));

        let request = format!(
            "POST {path} HTTP/1.1\r\nHost: {host}\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        if let Err(e) = stream.write_all(request.as_bytes()) {
            return Ok(Value::Enum {
                variant: "Err".into(),
                data: Some(Box::new(Value::Str(format!("write failed: {e}")))),
            });
        }

        let reader = BufReader::new(&stream);
        let mut resp_body = String::new();
        let mut in_body = false;
        for line in reader.lines() {
            match line {
                Ok(l) => {
                    if in_body {
                        resp_body.push_str(&l);
                        resp_body.push('\n');
                    } else if l.is_empty() {
                        in_body = true;
                    }
                }
                Err(_) => break,
            }
        }
        Ok(Value::Enum {
            variant: "Ok".into(),
            data: Some(Box::new(Value::Str(resp_body))),
        })
    }

    /// V18 1.6: TCP connect — returns file descriptor
    fn builtin_tcp_connect(&mut self, addr: &str) -> EvalResult {
        use std::net::TcpStream;
        match TcpStream::connect(addr) {
            Ok(stream) => {
                let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(10)));
                let fd = self.next_tcp_fd;
                self.next_tcp_fd += 1;
                self.tcp_connections.insert(fd, stream);
                Ok(Value::Enum {
                    variant: "Ok".into(),
                    data: Some(Box::new(Value::Int(fd as i64))),
                })
            }
            Err(e) => Ok(Value::Enum {
                variant: "Err".into(),
                data: Some(Box::new(Value::Str(format!("tcp connect failed: {e}")))),
            }),
        }
    }

    /// V18 1.6: TCP send — returns bytes written
    fn builtin_tcp_send(&mut self, fd: i64, data: &str) -> EvalResult {
        use std::io::Write;
        let fd = fd as usize;
        if let Some(stream) = self.tcp_connections.get_mut(&fd) {
            match stream.write_all(data.as_bytes()) {
                Ok(()) => Ok(Value::Enum {
                    variant: "Ok".into(),
                    data: Some(Box::new(Value::Int(data.len() as i64))),
                }),
                Err(e) => Ok(Value::Enum {
                    variant: "Err".into(),
                    data: Some(Box::new(Value::Str(format!("tcp send failed: {e}")))),
                }),
            }
        } else {
            Ok(Value::Enum {
                variant: "Err".into(),
                data: Some(Box::new(Value::Str("invalid fd".into()))),
            })
        }
    }

    /// V18 1.6: TCP recv — returns received string
    fn builtin_tcp_recv(&mut self, fd: i64) -> EvalResult {
        use std::io::Read;
        let fd = fd as usize;
        if let Some(stream) = self.tcp_connections.get_mut(&fd) {
            let mut buf = vec![0u8; 4096];
            match stream.read(&mut buf) {
                Ok(n) => {
                    let s = String::from_utf8_lossy(&buf[..n]).to_string();
                    Ok(Value::Enum {
                        variant: "Ok".into(),
                        data: Some(Box::new(Value::Str(s))),
                    })
                }
                Err(e) => Ok(Value::Enum {
                    variant: "Err".into(),
                    data: Some(Box::new(Value::Str(format!("tcp recv failed: {e}")))),
                }),
            }
        } else {
            Ok(Value::Enum {
                variant: "Err".into(),
                data: Some(Box::new(Value::Str("invalid fd".into()))),
            })
        }
    }

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
        if args.len() == 1 {
            // Single arg: nested array → auto-detect shape
            return self.tensor_from_nested_array(&args[0]);
        }
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

    /// Recursively flattens a nested array and infers its shape.
    fn tensor_from_nested_array(&self, val: &Value) -> EvalResult {
        let mut data = Vec::new();
        let mut shape = Vec::new();
        Self::flatten_nested(val, &mut data, &mut shape, 0)?;
        match TensorValue::from_data(data, &shape) {
            Ok(t) => Ok(Value::Tensor(t)),
            Err(e) => Err(RuntimeError::TypeError(e.to_string()).into()),
        }
    }

    /// Recursively flattens a Value (nested arrays) into flat f64 data + shape.
    fn flatten_nested(
        val: &Value,
        data: &mut Vec<f64>,
        shape: &mut Vec<usize>,
        depth: usize,
    ) -> Result<(), EvalError> {
        match val {
            Value::Float(f) => {
                data.push(*f);
                Ok(())
            }
            Value::Int(i) => {
                data.push(*i as f64);
                Ok(())
            }
            Value::Array(arr) => {
                if depth >= shape.len() {
                    shape.push(arr.len());
                }
                for item in arr {
                    Self::flatten_nested(item, data, shape, depth + 1)?;
                }
                Ok(())
            }
            _ => Err(
                RuntimeError::TypeError("from_data: expected nested numeric array".into()).into(),
            ),
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
    pub(crate) fn builtin_tensor_binop(&mut self, args: Vec<Value>, op: &str) -> EvalResult {
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
        // Auto-reshape 1D → 2D for convenience: [N] → [1,N] for first, [N] → [N,1] for second
        // If both are 1D, compute dot product instead
        if a.ndim() == 1 && b.ndim() == 1 {
            // 1D × 1D = dot product (scalar)
            let dot_val = tensor_ops::dot(&a, &b);
            return Ok(Value::Float(dot_val));
        }
        let a_2d = if a.ndim() == 1 {
            let n = a.numel();
            TensorValue::from_ndarray(
                a.data()
                    .clone()
                    .into_shape_with_order(ndarray::IxDyn(&[1, n]))
                    .unwrap_or_else(|_| a.data().clone()),
            )
        } else {
            a.clone()
        };
        let b_2d = if b.ndim() == 1 {
            let n = b.numel();
            TensorValue::from_ndarray(
                b.data()
                    .clone()
                    .into_shape_with_order(ndarray::IxDyn(&[n, 1]))
                    .unwrap_or_else(|_| b.data().clone()),
            )
        } else {
            b.clone()
        };
        let use_tracked = (a.requires_grad() || b.requires_grad()) && self.tape.is_recording();
        let result = if use_tracked {
            tensor_ops::matmul_tracked(&a_2d, &b_2d, &mut self.tape)
        } else {
            tensor_ops::matmul(&a_2d, &b_2d)
        };
        match result {
            Ok(t) => {
                // If original inputs were 1D, flatten result back
                if a.ndim() == 1 && b.ndim() == 2 {
                    // [1,N] × [N,M] → [1,M] → flatten to [M]
                    let flat = TensorValue::from_ndarray(
                        t.data()
                            .clone()
                            .into_shape_with_order(ndarray::IxDyn(&[t.numel()]))
                            .unwrap_or_else(|_| t.data().clone()),
                    );
                    Ok(Value::Tensor(flat))
                } else if a.ndim() == 2 && b.ndim() == 1 {
                    // [M,N] × [N,1] → [M,1] → flatten to [M]
                    let flat = TensorValue::from_ndarray(
                        t.data()
                            .clone()
                            .into_shape_with_order(ndarray::IxDyn(&[t.numel()]))
                            .unwrap_or_else(|_| t.data().clone()),
                    );
                    Ok(Value::Tensor(flat))
                } else {
                    Ok(Value::Tensor(t))
                }
            }
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

    /// V15 B2.7: `concat(t1, t2, axis)` — concatenate tensors along axis.
    fn builtin_tensor_concat(&self, args: Vec<Value>) -> EvalResult {
        if args.len() != 3 {
            return Err(RuntimeError::ArityMismatch {
                expected: 3,
                got: args.len(),
            }
            .into());
        }
        let axis = match &args[2] {
            Value::Int(a) => *a as usize,
            _ => {
                return Err(
                    RuntimeError::TypeError("concat: third arg must be axis (int)".into()).into(),
                );
            }
        };
        let t1 = match &args[0] {
            Value::Tensor(t) => t,
            _ => {
                return Err(
                    RuntimeError::TypeError("concat: first arg must be a tensor".into()).into(),
                );
            }
        };
        let t2 = match &args[1] {
            Value::Tensor(t) => t,
            _ => {
                return Err(
                    RuntimeError::TypeError("concat: second arg must be a tensor".into()).into(),
                );
            }
        };
        tensor_ops::concat(&[t1.clone(), t2.clone()], axis)
            .map(Value::Tensor)
            .map_err(|e| RuntimeError::TypeError(e.to_string()).into())
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

    // ═══════════════════════════════════════════════════════════════════════
    // V20.5 Tier 4: New tensor/scalar operations
    // ═══════════════════════════════════════════════════════════════════════

    /// Element-wise sign: -1, 0, or 1.
    fn builtin_sign(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        match &args[0] {
            Value::Tensor(t) => Ok(Value::Tensor(tensor_ops::sign(t))),
            Value::Int(n) => Ok(Value::Int(n.signum())),
            Value::Float(f) => Ok(Value::Float(if *f > 0.0 {
                1.0
            } else if *f < 0.0 {
                -1.0
            } else {
                0.0
            })),
            _ => Err(RuntimeError::TypeError("sign(tensor|number)".into()).into()),
        }
    }

    /// Index of minimum element.
    fn builtin_argmin(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        match &args[0] {
            Value::Tensor(t) => Ok(Value::Int(tensor_ops::argmin(t) as i64)),
            Value::Array(a) => {
                let idx = a
                    .iter()
                    .enumerate()
                    .min_by(|(_, x), (_, y)| {
                        let xf = match x {
                            Value::Int(n) => *n as f64,
                            Value::Float(f) => *f,
                            _ => f64::MAX,
                        };
                        let yf = match y {
                            Value::Int(n) => *n as f64,
                            Value::Float(f) => *f,
                            _ => f64::MAX,
                        };
                        xf.partial_cmp(&yf).unwrap_or(std::cmp::Ordering::Equal)
                    })
                    .map(|(i, _)| i)
                    .unwrap_or(0);
                Ok(Value::Int(idx as i64))
            }
            _ => Err(RuntimeError::TypeError("argmin(tensor|array)".into()).into()),
        }
    }

    /// L2 norm (Euclidean).
    fn builtin_norm(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        match &args[0] {
            Value::Tensor(t) => Ok(Value::Float(tensor_ops::norm(t))),
            _ => Err(RuntimeError::TypeError("norm(tensor)".into()).into()),
        }
    }

    /// Dot product of two tensors/arrays.
    fn builtin_dot(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into());
        }
        match (&args[0], &args[1]) {
            (Value::Tensor(a), Value::Tensor(b)) => Ok(Value::Float(tensor_ops::dot(a, b))),
            _ => Err(RuntimeError::TypeError("dot(tensor, tensor)".into()).into()),
        }
    }

    /// Element-wise e^x for tensors.
    fn builtin_exp_tensor(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        match &args[0] {
            Value::Tensor(t) => Ok(Value::Tensor(tensor_ops::exp_tensor(t))),
            _ => Err(RuntimeError::TypeError("exp_tensor(tensor)".into()).into()),
        }
    }

    /// Element-wise ln(x) for tensors.
    fn builtin_log_tensor(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        match &args[0] {
            Value::Tensor(t) => Ok(Value::Tensor(tensor_ops::log_tensor(t))),
            _ => Err(RuntimeError::TypeError("log_tensor(tensor)".into()).into()),
        }
    }

    /// Element-wise sqrt for tensors.
    fn builtin_sqrt_tensor(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        match &args[0] {
            Value::Tensor(t) => Ok(Value::Tensor(tensor_ops::sqrt_tensor(t))),
            _ => Err(RuntimeError::TypeError("sqrt_tensor(tensor)".into()).into()),
        }
    }

    /// Element-wise absolute value for tensors.
    fn builtin_abs_tensor(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        match &args[0] {
            Value::Tensor(t) => Ok(Value::Tensor(tensor_ops::abs_tensor(t))),
            _ => Err(RuntimeError::TypeError("abs_tensor(tensor)".into()).into()),
        }
    }

    /// Scalar e^x.
    fn builtin_exp_scalar(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        match &args[0] {
            Value::Float(f) => Ok(Value::Float(f.exp())),
            Value::Int(n) => Ok(Value::Float((*n as f64).exp())),
            _ => Err(RuntimeError::TypeError("exp(number)".into()).into()),
        }
    }

    /// Gamma function.
    fn builtin_gamma(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        match &args[0] {
            Value::Float(f) => Ok(Value::Float(tensor_ops::gamma(*f))),
            Value::Int(n) => Ok(Value::Float(tensor_ops::gamma(*n as f64))),
            _ => Err(RuntimeError::TypeError("gamma(number)".into()).into()),
        }
    }

    /// Element-wise clamp to [min, max].
    fn builtin_clamp_tensor(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 3 {
            return Err(RuntimeError::ArityMismatch {
                expected: 3,
                got: args.len(),
            }
            .into());
        }
        let min_val = match &args[1] {
            Value::Float(f) => *f,
            Value::Int(n) => *n as f64,
            _ => return Err(RuntimeError::TypeError("clamp_tensor(t, min, max)".into()).into()),
        };
        let max_val = match &args[2] {
            Value::Float(f) => *f,
            Value::Int(n) => *n as f64,
            _ => return Err(RuntimeError::TypeError("clamp_tensor(t, min, max)".into()).into()),
        };
        match &args[0] {
            Value::Tensor(t) => Ok(Value::Tensor(tensor_ops::clamp_tensor(t, min_val, max_val))),
            _ => Err(RuntimeError::TypeError("clamp_tensor(tensor, min, max)".into()).into()),
        }
    }

    /// Conditional select: where cond > 0, take x; else take y.
    fn builtin_where_tensor(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 3 {
            return Err(RuntimeError::ArityMismatch {
                expected: 3,
                got: args.len(),
            }
            .into());
        }
        match (&args[0], &args[1], &args[2]) {
            (Value::Tensor(cond), Value::Tensor(x), Value::Tensor(y)) => {
                Ok(Value::Tensor(tensor_ops::where_tensor(cond, x, y)))
            }
            _ => Err(RuntimeError::TypeError(
                "where_tensor(cond_tensor, x_tensor, y_tensor)".into(),
            )
            .into()),
        }
    }

    // ═══════════════════════════════════════════════════════════════════════
    // FajarQuant Phase 1: TurboQuant builtins
    // ═══════════════════════════════════════════════════════════════════════

    /// Create a TurboQuant configuration: turboquant_create(dim, bits).
    fn builtin_turboquant_create(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into());
        }
        let dim = match &args[0] {
            Value::Int(n) => *n as usize,
            _ => return Err(RuntimeError::TypeError("turboquant_create(dim, bits)".into()).into()),
        };
        let bits = match &args[1] {
            Value::Int(n) => *n as u8,
            _ => return Err(RuntimeError::TypeError("turboquant_create(dim, bits)".into()).into()),
        };
        if dim == 0 || bits == 0 || bits > 8 {
            return Err(RuntimeError::TypeError(
                "turboquant_create: dim > 0 and 1 <= bits <= 8".into(),
            )
            .into());
        }
        let mut m = std::collections::HashMap::new();
        m.insert("_type".to_string(), Value::Str("TurboQuantConfig".into()));
        m.insert("dim".to_string(), Value::Int(dim as i64));
        m.insert("bits".to_string(), Value::Int(bits as i64));
        m.insert("codebook_size".to_string(), Value::Int(1i64 << bits));
        Ok(Value::Map(m))
    }

    /// Encode a vector: turboquant_encode(config, tensor) -> map.
    fn builtin_turboquant_encode(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into());
        }
        let (dim, bits) = match &args[0] {
            Value::Map(m) => {
                let d = match m.get("dim") {
                    Some(Value::Int(n)) => *n as usize,
                    _ => return Err(RuntimeError::TypeError("invalid config".into()).into()),
                };
                let b = match m.get("bits") {
                    Some(Value::Int(n)) => *n as u8,
                    _ => return Err(RuntimeError::TypeError("invalid config".into()).into()),
                };
                (d, b)
            }
            _ => {
                return Err(
                    RuntimeError::TypeError("turboquant_encode(config, tensor)".into()).into(),
                );
            }
        };
        let data = match &args[1] {
            Value::Tensor(tv) => tv.data().iter().cloned().collect::<Vec<f64>>(),
            _ => {
                return Err(RuntimeError::TypeError(
                    "turboquant_encode: arg 2 must be tensor".into(),
                )
                .into());
            }
        };
        // Pad or truncate to match dim
        let mut x_vec = data;
        x_vec.resize(dim, 0.0);
        let x = ndarray::Array1::from_vec(x_vec);
        let config = crate::runtime::ml::turboquant::create_config(dim, bits);
        let qv = crate::runtime::ml::turboquant::quant_mse(&x, &config);
        let mut result = std::collections::HashMap::new();
        result.insert("_type".to_string(), Value::Str("QuantizedVector".into()));
        result.insert(
            "indices".to_string(),
            Value::Array(qv.indices.iter().map(|&i| Value::Int(i as i64)).collect()),
        );
        result.insert("norm".to_string(), Value::Float(qv.norm));
        result.insert("dim".to_string(), Value::Int(dim as i64));
        result.insert("bits".to_string(), Value::Int(bits as i64));
        Ok(Value::Map(result))
    }

    /// Decode a quantized vector: turboquant_decode(config, encoded) -> tensor.
    fn builtin_turboquant_decode(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into());
        }
        let (dim, bits) = match &args[0] {
            Value::Map(m) => {
                let d = match m.get("dim") {
                    Some(Value::Int(n)) => *n as usize,
                    _ => return Err(RuntimeError::TypeError("invalid config".into()).into()),
                };
                let b = match m.get("bits") {
                    Some(Value::Int(n)) => *n as u8,
                    _ => return Err(RuntimeError::TypeError("invalid config".into()).into()),
                };
                (d, b)
            }
            _ => {
                return Err(
                    RuntimeError::TypeError("turboquant_decode(config, encoded)".into()).into(),
                );
            }
        };
        let indices = match &args[1] {
            Value::Map(m) => match m.get("indices") {
                Some(Value::Array(arr)) => arr
                    .iter()
                    .map(|v| match v {
                        Value::Int(n) => *n as u8,
                        _ => 0,
                    })
                    .collect::<Vec<u8>>(),
                _ => return Err(RuntimeError::TypeError("invalid encoded data".into()).into()),
            },
            _ => {
                return Err(
                    RuntimeError::TypeError("turboquant_decode(config, encoded)".into()).into(),
                );
            }
        };
        let config = crate::runtime::ml::turboquant::create_config(dim, bits);
        let qv = crate::runtime::ml::turboquant::QuantizedVector {
            indices,
            norm: 0.0,
            rotation_id: 0,
        };
        let decoded = crate::runtime::ml::turboquant::dequant_mse(&qv, &config);
        let tv = crate::runtime::ml::tensor::TensorValue::from_ndarray(
            decoded
                .into_shape_with_order(ndarray::IxDyn(&[dim]))
                .unwrap_or_else(|_| ndarray::ArrayD::zeros(ndarray::IxDyn(&[dim]))),
        );
        Ok(Value::Tensor(tv))
    }

    /// Compute inner product via quantized representations.
    fn builtin_turboquant_inner_product(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 3 {
            return Err(RuntimeError::ArityMismatch {
                expected: 3,
                got: args.len(),
            }
            .into());
        }
        // turboquant_inner_product(config, x_tensor, y_tensor)
        let (dim, bits) = match &args[0] {
            Value::Map(m) => {
                let d = match m.get("dim") {
                    Some(Value::Int(n)) => *n as usize,
                    _ => return Err(RuntimeError::TypeError("invalid config".into()).into()),
                };
                let b = match m.get("bits") {
                    Some(Value::Int(n)) => *n as u8,
                    _ => return Err(RuntimeError::TypeError("invalid config".into()).into()),
                };
                (d, b)
            }
            _ => {
                return Err(RuntimeError::TypeError(
                    "turboquant_inner_product(config, x, y)".into(),
                )
                .into());
            }
        };
        let x_data = match &args[1] {
            Value::Tensor(tv) => tv.data().iter().cloned().collect::<Vec<f64>>(),
            _ => return Err(RuntimeError::TypeError("arg 2 must be tensor".into()).into()),
        };
        let y_data = match &args[2] {
            Value::Tensor(tv) => tv.data().iter().cloned().collect::<Vec<f64>>(),
            _ => return Err(RuntimeError::TypeError("arg 3 must be tensor".into()).into()),
        };
        let mut x = x_data;
        x.resize(dim, 0.0);
        let mut y = y_data;
        y.resize(dim, 0.0);
        let x_arr = ndarray::Array1::from_vec(x);
        let y_arr = ndarray::Array1::from_vec(y);
        // True inner product
        let true_ip: f64 = x_arr.iter().zip(y_arr.iter()).map(|(a, b)| a * b).sum();
        // Quantized inner product via encode-decode
        let config = crate::runtime::ml::turboquant::create_config(dim, bits);
        let qx = crate::runtime::ml::turboquant::quant_mse(&x_arr, &config);
        let x_hat = crate::runtime::ml::turboquant::dequant_mse(&qx, &config);
        let approx_ip: f64 = x_hat.iter().zip(y_arr.iter()).map(|(a, b)| a * b).sum();
        let mut result = std::collections::HashMap::new();
        result.insert("true_ip".to_string(), Value::Float(true_ip));
        result.insert("approx_ip".to_string(), Value::Float(approx_ip));
        result.insert(
            "error".to_string(),
            Value::Float((true_ip - approx_ip).abs()),
        );
        Ok(Value::Map(result))
    }

    // ═══════════════════════════════════════════════════════════════════════
    // FajarQuant Phase 2: Adaptive rotation comparison
    // ═══════════════════════════════════════════════════════════════════════

    // ═══════════════════════════════════════════════════════════════════════
    // GPU Discovery + Info
    // ═══════════════════════════════════════════════════════════════════════

    /// Discover available GPUs: gpu_discover() -> map.
    fn builtin_gpu_discover(&mut self, args: Vec<Value>) -> EvalResult {
        if !args.is_empty() {
            return Err(RuntimeError::ArityMismatch {
                expected: 0,
                got: args.len(),
            }
            .into());
        }
        let discovery = crate::hw::gpu::GpuDiscovery::detect();
        let mut result = std::collections::HashMap::new();
        result.insert(
            "cuda_available".to_string(),
            Value::Bool(discovery.cuda_available),
        );
        result.insert(
            "device_count".to_string(),
            Value::Int(discovery.devices.len() as i64),
        );
        let devices: Vec<Value> = discovery
            .devices
            .iter()
            .map(|d| {
                let mut dev = std::collections::HashMap::new();
                dev.insert("name".to_string(), Value::Str(d.name.clone()));
                dev.insert(
                    "compute_capability".to_string(),
                    Value::Str(d.compute_capability_str()),
                );
                dev.insert(
                    "vram_mb".to_string(),
                    Value::Int((d.vram_total_bytes / (1024 * 1024)) as i64),
                );
                dev.insert("sm_count".to_string(), Value::Int(d.sm_count as i64));
                dev.insert("cuda_cores".to_string(), Value::Int(d.cuda_cores as i64));
                Value::Map(dev)
            })
            .collect();
        result.insert("devices".to_string(), Value::Array(devices));
        if let Some(ver) = discovery.driver_version {
            result.insert("driver_version".to_string(), Value::Int(ver as i64));
        }
        Ok(Value::Map(result))
    }

    /// Compare adaptive vs random quantization: fajarquant_compare(dim, bits, n_samples).
    fn builtin_fajarquant_compare(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 3 {
            return Err(RuntimeError::ArityMismatch {
                expected: 3,
                got: args.len(),
            }
            .into());
        }
        let dim = match &args[0] {
            Value::Int(n) => *n as usize,
            _ => {
                return Err(RuntimeError::TypeError(
                    "fajarquant_compare(dim, bits, n_samples)".into(),
                )
                .into());
            }
        };
        let bits = match &args[1] {
            Value::Int(n) => *n as u8,
            _ => {
                return Err(RuntimeError::TypeError(
                    "fajarquant_compare(dim, bits, n_samples)".into(),
                )
                .into());
            }
        };
        let n_samples = match &args[2] {
            Value::Int(n) => *n as usize,
            _ => {
                return Err(RuntimeError::TypeError(
                    "fajarquant_compare(dim, bits, n_samples)".into(),
                )
                .into());
            }
        };
        // Generate structured (low-rank) test data
        let mut rng: u64 = 42;
        let data: Vec<ndarray::Array1<f64>> = (0..n_samples)
            .map(|_| {
                let mut v = ndarray::Array1::zeros(dim);
                // Strong signal in first 25% of dimensions
                let strong = dim / 4;
                for i in 0..dim {
                    let u1 = crate::runtime::ml::turboquant::lcg_next_f64(&mut rng);
                    let u2 = crate::runtime::ml::turboquant::lcg_next_f64(&mut rng);
                    let g = (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos();
                    v[i] = if i < strong { g * 1.0 } else { g * 0.05 };
                }
                v
            })
            .collect();
        let (adaptive_mse, random_mse) =
            crate::runtime::ml::fajarquant::adaptive::compare_adaptive_vs_random(&data, dim, bits);
        let improvement = if random_mse > 1e-15 {
            (1.0 - adaptive_mse / random_mse) * 100.0
        } else {
            0.0
        };
        let mut result = std::collections::HashMap::new();
        result.insert("adaptive_mse".to_string(), Value::Float(adaptive_mse));
        result.insert("random_mse".to_string(), Value::Float(random_mse));
        result.insert("improvement_pct".to_string(), Value::Float(improvement));
        Ok(Value::Map(result))
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

    /// `quantize_int8(tensor) -> Tensor` — Quantize a tensor to INT8 and dequantize back.
    fn builtin_quantize_int8(&self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        match &args[0] {
            Value::Tensor(t) => {
                let qt = crate::runtime::ml::quantize::QuantizedTensor::quantize(t);
                Ok(Value::Tensor(qt.dequantize()))
            }
            _ => Err(
                RuntimeError::TypeError("quantize_int8: expected a Tensor argument".into()).into(),
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
        let block_env = Arc::new(Mutex::new(Environment::new_with_parent(Arc::clone(
            &self.env,
        ))));
        let prev_env = Arc::clone(&self.env);
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
        self.env.lock().expect("env lock").drop_locals();

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
            let loop_env = Arc::new(Mutex::new(Environment::new_with_parent(Arc::clone(
                &self.env,
            ))));
            loop_env
                .lock()
                .expect("env lock")
                .define(variable.to_string(), item);

            let prev_env = Arc::clone(&self.env);
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
        iter_rc: Arc<Mutex<IteratorValue>>,
        body: &Expr,
        label: Option<&str>,
    ) -> EvalResult {
        loop {
            let item = self.iter_next(&iter_rc)?;
            let item = match item {
                Some(v) => v,
                None => break,
            };

            let loop_env = Arc::new(Mutex::new(Environment::new_with_parent(Arc::clone(
                &self.env,
            ))));
            loop_env
                .lock()
                .expect("env lock")
                .define(variable.to_string(), item);

            let prev_env = Arc::clone(&self.env);
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
        iter_rc: &Arc<Mutex<IteratorValue>>,
    ) -> Result<Option<Value>, EvalError> {
        let mut iter = iter_rc.lock().expect("iter lock");
        match &mut *iter {
            IteratorValue::MappedIter { inner, func } => {
                let inner_clone = inner.clone();
                let func_clone = func.clone();
                drop(iter); // Release lock before calling function
                let inner_rc = Arc::new(Mutex::new(*inner_clone));
                let val = self.iter_next(&inner_rc)?;
                // Write back the advanced inner iterator
                let advanced = inner_rc.lock().expect("iter lock").clone();
                let mut iter = iter_rc.lock().expect("iter lock");
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
                let inner_rc = Arc::new(Mutex::new(*inner_clone));
                loop {
                    let val = self.iter_next(&inner_rc)?;
                    // Write back
                    let advanced = inner_rc.lock().expect("iter lock").clone();
                    let mut iter = iter_rc.lock().expect("iter lock");
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
                            let iter = iter_rc.lock().expect("iter lock");
                            if let IteratorValue::FilterIter { inner, .. } = &*iter {
                                *inner_rc.lock().expect("iter lock") = *inner.clone();
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
                if !self.env.lock().expect("env lock").assign(name, final_val) {
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
                            if !self
                                .env
                                .lock()
                                .expect("env lock")
                                .assign(name, Value::Array(new_arr))
                            {
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
                            if !self.env.lock().expect("env lock").assign(name, new_struct) {
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
                    let guard_env = Arc::new(Mutex::new(Environment::new_with_parent(Arc::clone(
                        &self.env,
                    ))));
                    for (k, v) in &bindings {
                        guard_env
                            .lock()
                            .expect("env lock")
                            .define(k.clone(), v.clone());
                    }
                    let prev = Arc::clone(&self.env);
                    self.env = guard_env;
                    let guard_val = self.eval_expr(guard)?;
                    self.env = prev;
                    if !guard_val.is_truthy() {
                        continue;
                    }
                }

                // Create scope with pattern bindings and evaluate body
                let arm_env = Arc::new(Mutex::new(Environment::new_with_parent(Arc::clone(
                    &self.env,
                ))));
                for (k, v) in bindings {
                    arm_env.lock().expect("env lock").define(k, v);
                }
                let prev = Arc::clone(&self.env);
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
                }) = self.env.lock().expect("env lock").lookup(name)
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
            // V16 L2.6: Array destructuring
            Pattern::Array { elements, rest, .. } => {
                if let Value::Array(vals) = value {
                    if rest.is_some() {
                        // With rest: need at least `elements.len()` items
                        if vals.len() < elements.len() {
                            return None;
                        }
                    } else if vals.len() != elements.len() {
                        return None;
                    }
                    let mut bindings = HashMap::new();
                    for (i, pat) in elements.iter().enumerate() {
                        let sub = self.match_pattern(pat, &vals[i])?;
                        bindings.extend(sub);
                    }
                    if let Some(rest_name) = rest {
                        let rest_vals: Vec<Value> =
                            vals.iter().skip(elements.len()).cloned().collect();
                        bindings.insert(rest_name.clone(), Value::Array(rest_vals));
                    }
                    Some(bindings)
                } else {
                    None
                }
            }
            // V16 L2.7: Binding pattern — `name @ pattern`
            Pattern::Binding { name, pattern, .. } => {
                let sub = self.match_pattern(pattern, value)?;
                let mut bindings = sub;
                bindings.insert(name.clone(), value.clone());
                Some(bindings)
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
            closure_env: Arc::clone(&self.env),
            is_async: false,
            is_gen: false,
            requires: vec![],
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
                closure_env: Arc::clone(&self.env),
                is_async: false,
                is_gen: false,
                requires: vec![],
            };

            // Check if this is a static method (no `self` param) — also register globally
            let is_static = method.params.first().is_none_or(|p| p.name != "self");
            if is_static {
                // Register as `TypeName::method_name` in global env for path access
                let qualified = format!("{}::{}", type_name, method.name);
                self.env
                    .lock()
                    .expect("env lock")
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
        iter_rc: Arc<Mutex<IteratorValue>>,
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
                let inner = iter_rc.lock().expect("iter lock").clone();
                let mapped = IteratorValue::MappedIter {
                    inner: Box::new(inner),
                    func,
                };
                Ok(Value::Iterator(Arc::new(Mutex::new(mapped))))
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
                let inner = iter_rc.lock().expect("iter lock").clone();
                let filtered = IteratorValue::FilterIter {
                    inner: Box::new(inner),
                    func,
                };
                Ok(Value::Iterator(Arc::new(Mutex::new(filtered))))
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
                let inner = iter_rc.lock().expect("iter lock").clone();
                let taken = IteratorValue::TakeIter {
                    inner: Box::new(inner),
                    remaining: n,
                };
                Ok(Value::Iterator(Arc::new(Mutex::new(taken))))
            }
            "enumerate" => {
                let inner = iter_rc.lock().expect("iter lock").clone();
                let enumerated = IteratorValue::EnumerateIter {
                    inner: Box::new(inner),
                    index: 0,
                };
                Ok(Value::Iterator(Arc::new(Mutex::new(enumerated))))
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
                        closure_env: Arc::clone(&self.env),
                        is_async: false,
                        is_gen: false,
                        requires: vec![],
                    };
                    let val = Value::Function(fn_val);
                    mod_symbols.insert(fndef.name.clone(), val.clone());
                    if fndef.is_pub {
                        pub_items.insert(fndef.name.clone());
                    }
                    let qualified = format!("{}::{}", mod_name, fndef.name);
                    self.env.lock().expect("env lock").define(qualified, val);
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
                    self.env.lock().expect("env lock").define(qualified, val);
                }
                Item::ModDecl(inner_mod) => {
                    // Nested module: evaluate and store with nested qualified names
                    self.eval_mod_decl(inner_mod)?;
                    if let Some(inner_symbols) = self.modules.get(&inner_mod.name).cloned() {
                        let nested_name = format!("{}::{}", mod_name, inner_mod.name);
                        for (sym_name, sym_val) in &inner_symbols {
                            let qualified = format!("{}::{}", nested_name, sym_name);
                            self.env
                                .lock()
                                .expect("env lock")
                                .define(qualified, sym_val.clone());
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
                            self.env.lock().expect("env lock").define(qualified, val);
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

        // V15 I2.3: File-based module loading.
        // If the module isn't already loaded, try to find and load <name>.fj
        // from the source directory.
        if !path.is_empty() {
            let mod_name = &path[0];
            if !self.modules.contains_key(mod_name) {
                if let Some(ref source_dir) = self.source_dir.clone() {
                    let fj_path = source_dir.join(format!("{mod_name}.fj"));
                    if fj_path.exists() {
                        if let Ok(source) = std::fs::read_to_string(&fj_path) {
                            if let Ok(tokens) = crate::lexer::tokenize(&source) {
                                if let Ok(program) = crate::parser::parse(tokens) {
                                    // Evaluate the module file to define its functions
                                    let _ = self.eval_program(&program);
                                }
                            }
                        }
                    }
                }
            }
        }

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
                    let resolved = self
                        .env
                        .lock()
                        .expect("env lock")
                        .lookup(&qualified)
                        .or_else(|| {
                            self.modules
                                .get(&mod_path)
                                .and_then(|m| m.get(item_name).cloned())
                        });
                    if let Some(val) = resolved {
                        self.env
                            .lock()
                            .expect("env lock")
                            .define(item_name.clone(), val);
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
                            self.env.lock().expect("env lock").define(name, val);
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
                    let resolved = self
                        .env
                        .lock()
                        .expect("env lock")
                        .lookup(&qualified)
                        .or_else(|| {
                            self.modules
                                .get(&mod_path)
                                .and_then(|m| m.get(name).cloned())
                        });
                    if let Some(val) = resolved {
                        imports.push((name.clone(), val));
                    }
                }
                for (name, val) in imports {
                    self.env.lock().expect("env lock").define(name, val);
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
    /// Opens a WebSocket connection. With `--features websocket`, connects via
    /// real TCP + RFC 6455 handshake. Without the feature, uses in-memory echo.
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

        #[cfg(feature = "websocket")]
        {
            match tungstenite::connect(&url) {
                Ok((ws, _response)) => {
                    // Set underlying TCP stream to non-blocking for ws_recv.
                    match ws.get_ref() {
                        tungstenite::stream::MaybeTlsStream::Plain(tcp) => {
                            let _ = tcp.set_nonblocking(true);
                        }
                        tungstenite::stream::MaybeTlsStream::NativeTls(tls) => {
                            let _ = tls.get_ref().set_nonblocking(true);
                        }
                        _ => {}
                    }
                    self.ws_connections.insert(
                        id,
                        super::WsConnection {
                            url,
                            connected: true,
                            send_buffer: Vec::new(),
                            recv_buffer: std::collections::VecDeque::new(),
                            socket: Some(ws),
                        },
                    );
                }
                Err(e) => {
                    return Err(RuntimeError::TypeError(format!(
                        "ws_connect: failed to connect to {url}: {e}"
                    ))
                    .into());
                }
            }
        }

        #[cfg(not(feature = "websocket"))]
        {
            self.ws_connections.insert(
                id,
                super::WsConnection {
                    url,
                    connected: true,
                    send_buffer: Vec::new(),
                    recv_buffer: std::collections::VecDeque::new(),
                },
            );
        }

        Ok(Value::Int(id))
    }

    /// ws_send(handle: i64, message: str) -> i64
    ///
    /// Sends a text message over a WebSocket connection. Returns message length.
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
        let len = message.len() as i64;

        #[cfg(feature = "websocket")]
        {
            if let Some(ref mut ws) = conn.socket {
                ws.send(tungstenite::Message::Text(message.clone().into()))
                    .map_err(|e| RuntimeError::TypeError(format!("ws_send: {e}")))?;
            }
        }

        #[cfg(not(feature = "websocket"))]
        {
            // Simulation: echo message to recv buffer.
            conn.recv_buffer.push_back(message.clone());
        }

        conn.send_buffer.push(message);
        Ok(Value::Int(len))
    }

    /// ws_recv(handle: i64) -> str | null
    ///
    /// Receives the next pending message. Returns `null` if no message is available.
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

        #[cfg(feature = "websocket")]
        {
            if let Some(ref mut ws) = conn.socket {
                match ws.read() {
                    Ok(tungstenite::Message::Text(s)) => {
                        return Ok(Value::Str(s.to_string()));
                    }
                    Ok(tungstenite::Message::Binary(b)) => {
                        return Ok(Value::Str(String::from_utf8_lossy(&b).to_string()));
                    }
                    Ok(tungstenite::Message::Close(_)) => {
                        conn.connected = false;
                        return Ok(Value::Null);
                    }
                    Err(tungstenite::Error::Io(ref e))
                        if e.kind() == std::io::ErrorKind::WouldBlock =>
                    {
                        return Ok(Value::Null); // no data available yet
                    }
                    Err(_) => return Ok(Value::Null),
                    _ => return Ok(Value::Null), // Ping/Pong/Frame
                }
            }
            return Ok(Value::Null);
        }

        #[cfg(not(feature = "websocket"))]
        {
            match conn.recv_buffer.pop_front() {
                Some(msg) => Ok(Value::Str(msg)),
                None => Ok(Value::Null),
            }
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
            #[cfg(feature = "websocket")]
            {
                if let Some(ref mut ws) = conn.socket {
                    let _ = ws.close(None);
                }
            }
        }
        self.ws_connections.remove(&handle);
        Ok(Value::Null)
    }

    // ── MQTT builtins ──

    /// Parse "mqtt://host:port" → (host, port). Defaults to port 1883.
    #[cfg(feature = "mqtt")]
    fn parse_mqtt_url(url: &str) -> (String, u16) {
        let stripped = url
            .strip_prefix("mqtt://")
            .or_else(|| url.strip_prefix("mqtts://"))
            .unwrap_or(url);
        let parts: Vec<&str> = stripped.splitn(2, ':').collect();
        let host = parts[0].to_string();
        let port = parts.get(1).and_then(|p| p.parse().ok()).unwrap_or(1883);
        (host, port)
    }

    /// mqtt_connect(broker: str) -> i64
    ///
    /// With `--features mqtt`: connects to a real MQTT broker via TCP.
    /// Without the feature: in-memory broker simulation.
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

        #[cfg(feature = "mqtt")]
        {
            let (host, port) = Self::parse_mqtt_url(&broker);
            let mut opts = rumqttc::MqttOptions::new(format!("fj-client-{id}"), host.clone(), port);
            opts.set_keep_alive(std::time::Duration::from_secs(30));

            match rumqttc::Client::new(opts, 64) {
                (client, connection) => {
                    let (tx, rx) = std::sync::mpsc::channel();
                    let thread = std::thread::spawn(move || {
                        let mut conn = connection;
                        for event in conn.iter() {
                            match event {
                                Ok(rumqttc::Event::Incoming(rumqttc::Packet::Publish(p))) => {
                                    let topic = p.topic.clone();
                                    let payload = String::from_utf8_lossy(&p.payload).to_string();
                                    if tx.send((topic, payload)).is_err() {
                                        break;
                                    }
                                }
                                Err(_) => break,
                                _ => {}
                            }
                        }
                    });
                    self.mqtt_clients.insert(
                        id,
                        super::MqttClientState {
                            broker_addr: broker,
                            connected: true,
                            subscriptions: Vec::new(),
                            real_client: Some(super::RealMqttClient {
                                client,
                                receiver: rx,
                                _thread: Some(thread),
                            }),
                        },
                    );
                }
            }
        }

        #[cfg(not(feature = "mqtt"))]
        {
            self.mqtt_clients.insert(
                id,
                super::MqttClientState {
                    broker_addr: broker,
                    connected: true,
                    subscriptions: Vec::new(),
                },
            );
        }

        Ok(Value::Int(id))
    }

    /// mqtt_publish(handle: i64, topic: str, payload: str) -> null
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
        let client = self.mqtt_clients.get_mut(&handle).ok_or_else(|| {
            RuntimeError::TypeError(format!("mqtt_publish: invalid handle {handle}"))
        })?;
        if !client.connected {
            return Err(RuntimeError::TypeError("mqtt_publish: not connected".into()).into());
        }

        #[cfg(feature = "mqtt")]
        {
            if let Some(ref mut rc) = client.real_client {
                rc.client
                    .publish(&topic, rumqttc::QoS::AtLeastOnce, false, payload.as_bytes())
                    .map_err(|e| RuntimeError::TypeError(format!("mqtt_publish: {e}")))?;
                return Ok(Value::Null);
            }
        }

        #[cfg(not(feature = "mqtt"))]
        {
            self.mqtt_broker.publish(&topic, &payload);
        }

        Ok(Value::Null)
    }

    /// mqtt_subscribe(handle: i64, topic: str) -> null
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

        #[cfg(feature = "mqtt")]
        {
            if let Some(ref mut rc) = client.real_client {
                rc.client
                    .subscribe(&topic, rumqttc::QoS::AtLeastOnce)
                    .map_err(|e| RuntimeError::TypeError(format!("mqtt_subscribe: {e}")))?;
                return Ok(Value::Null);
            }
        }

        #[cfg(not(feature = "mqtt"))]
        {
            self.mqtt_broker.subscribe(handle, &topic);
        }

        Ok(Value::Null)
    }

    /// mqtt_recv(handle: i64) -> Map | null
    ///
    /// Returns next message as `{ "topic": str, "payload": str }`, or null.
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

        #[cfg(feature = "mqtt")]
        {
            if let Some(client) = self.mqtt_clients.get_mut(&handle) {
                if let Some(ref mut rc) = client.real_client {
                    match rc.receiver.try_recv() {
                        Ok((topic, payload)) => {
                            let mut map = std::collections::HashMap::new();
                            map.insert("topic".to_string(), Value::Str(topic));
                            map.insert("payload".to_string(), Value::Str(payload));
                            return Ok(Value::Map(map));
                        }
                        Err(_) => return Ok(Value::Null),
                    }
                }
            }
        }

        #[cfg(not(feature = "mqtt"))]
        {
            match self.mqtt_broker.receive(handle) {
                Some((topic, payload)) => {
                    let mut map = std::collections::HashMap::new();
                    map.insert("topic".to_string(), Value::Str(topic));
                    map.insert("payload".to_string(), Value::Str(payload));
                    return Ok(Value::Map(map));
                }
                None => return Ok(Value::Null),
            }
        }

        #[allow(unreachable_code)]
        Ok(Value::Null)
    }

    /// mqtt_disconnect(handle: i64) -> null
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

        #[cfg(not(feature = "mqtt"))]
        {
            self.mqtt_broker.unsubscribe_all(handle);
        }

        // Dropping the client + receiver causes the background thread to exit.
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

        #[cfg(feature = "ble")]
        {
            use btleplug::api::{Central, Manager as _, Peripheral as _, ScanFilter};
            use btleplug::platform::Manager;

            let rt = tokio::runtime::Runtime::new()
                .map_err(|e| RuntimeError::TypeError(format!("ble_scan: runtime error: {e}")))?;
            let devices = rt.block_on(async {
                let manager = Manager::new().await.map_err(|e| format!("ble_scan: {e}"))?;
                let adapters = manager
                    .adapters()
                    .await
                    .map_err(|e| format!("ble_scan: {e}"))?;
                let adapter = adapters
                    .into_iter()
                    .next()
                    .ok_or_else(|| "ble_scan: no Bluetooth adapter found".to_string())?;
                adapter
                    .start_scan(ScanFilter::default())
                    .await
                    .map_err(|e| format!("ble_scan: {e}"))?;
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                let peripherals = adapter
                    .peripherals()
                    .await
                    .map_err(|e| format!("ble_scan: {e}"))?;
                let mut result = Vec::new();
                for p in peripherals {
                    if let Ok(Some(props)) = p.properties().await {
                        let addr = props.address.to_string();
                        let name = props.local_name.unwrap_or_else(|| "Unknown".to_string());
                        result.push((addr, name));
                    }
                }
                Ok::<Vec<(String, String)>, String>(result)
            });
            match devices {
                Ok(devs) => {
                    let result: Vec<Value> = devs
                        .into_iter()
                        .map(|(addr, name)| {
                            let mut map = HashMap::new();
                            map.insert("addr".to_string(), Value::Str(addr));
                            map.insert("name".to_string(), Value::Str(name));
                            Value::Map(map)
                        })
                        .collect();
                    return Ok(Value::Array(result));
                }
                Err(e) => {
                    return Err(RuntimeError::TypeError(e).into());
                }
            }
        }

        #[cfg(not(feature = "ble"))]
        {
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

        #[cfg(feature = "ble")]
        {
            use btleplug::api::{Central, Manager as _, Peripheral as _};
            use btleplug::platform::Manager;
            let rt = self.ensure_tokio_runtime();
            let result = rt.block_on(async {
                let manager = Manager::new().await.map_err(|e| format!("{e}"))?;
                let adapters = manager.adapters().await.map_err(|e| format!("{e}"))?;
                let adapter = adapters.into_iter().next().ok_or("no adapter")?;
                let peripherals = adapter.peripherals().await.map_err(|e| format!("{e}"))?;
                for p in peripherals {
                    if let Ok(Some(props)) = p.properties().await {
                        if props.address.to_string() == addr {
                            p.connect().await.map_err(|e| format!("{e}"))?;
                            p.discover_services().await.map_err(|e| format!("{e}"))?;
                            return Ok(1i64); // connected
                        }
                    }
                }
                Err("device not found".to_string())
            });
            return match result {
                Ok(h) => Ok(Value::Int(h)),
                Err(_) => Ok(Value::Int(-1)),
            };
        }

        #[cfg(not(feature = "ble"))]
        {
            match self.ble_adapter.connect(&addr) {
                Some(handle) => Ok(Value::Int(handle)),
                None => Ok(Value::Int(-1)),
            }
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

        #[cfg(feature = "ble")]
        {
            use btleplug::api::{Central, Manager as _, Peripheral as _};
            use btleplug::platform::Manager;
            let rt = self.ensure_tokio_runtime();
            let result = rt.block_on(async {
                let manager = Manager::new().await.map_err(|e| format!("{e}"))?;
                let adapters = manager.adapters().await.map_err(|e| format!("{e}"))?;
                let adapter = adapters.into_iter().next().ok_or("no adapter")?;
                let peripherals = adapter.peripherals().await.map_err(|e| format!("{e}"))?;
                // Find connected peripheral and read characteristic
                for p in peripherals {
                    if p.is_connected().await.unwrap_or(false) {
                        for ch in p.characteristics() {
                            if ch.uuid.to_string() == uuid {
                                let data = p.read(&ch).await.map_err(|e| format!("{e}"))?;
                                return Ok(data);
                            }
                        }
                    }
                }
                Err("characteristic not found".to_string())
            });
            let _ = handle; // handle used for simulation fallback
            return match result {
                Ok(bytes) => {
                    let arr: Vec<Value> = bytes.into_iter().map(|b| Value::Int(b as i64)).collect();
                    Ok(Value::Array(arr))
                }
                Err(_) => Ok(Value::Null),
            };
        }

        #[cfg(not(feature = "ble"))]
        {
            match self.ble_adapter.read(handle, &uuid) {
                Some(bytes) => {
                    let arr: Vec<Value> = bytes.into_iter().map(|b| Value::Int(b as i64)).collect();
                    Ok(Value::Array(arr))
                }
                None => Ok(Value::Null),
            }
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
        let data: Vec<u8> = match &args[2] {
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

        #[cfg(feature = "ble")]
        {
            use btleplug::api::{Central, Manager as _, Peripheral as _, WriteType};
            use btleplug::platform::Manager;
            let rt = self.ensure_tokio_runtime();
            let write_data = data.clone();
            let result: Result<bool, String> = rt.block_on(async {
                let manager = Manager::new().await.map_err(|e| format!("{e}"))?;
                let adapters = manager.adapters().await.map_err(|e| format!("{e}"))?;
                let adapter = adapters
                    .into_iter()
                    .next()
                    .ok_or_else(|| "no adapter".to_string())?;
                let peripherals = adapter.peripherals().await.map_err(|e| format!("{e}"))?;
                for p in peripherals {
                    if p.is_connected().await.unwrap_or(false) {
                        for ch in p.characteristics() {
                            if ch.uuid.to_string() == uuid {
                                p.write(&ch, &write_data, WriteType::WithResponse)
                                    .await
                                    .map_err(|e| format!("{e}"))?;
                                return Ok(true);
                            }
                        }
                    }
                }
                Ok(false)
            });
            let _ = handle;
            return Ok(Value::Bool(result.unwrap_or(false)));
        }

        #[cfg(not(feature = "ble"))]
        {
            Ok(Value::Bool(self.ble_adapter.write(handle, &uuid, data)))
        }
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

        #[cfg(feature = "ble")]
        {
            use btleplug::api::{Central, Manager as _, Peripheral as _};
            use btleplug::platform::Manager;
            let rt = self.ensure_tokio_runtime();
            let _ = rt.block_on(async {
                let manager = Manager::new().await.ok()?;
                let adapters = manager.adapters().await.ok()?;
                let adapter = adapters.into_iter().next()?;
                let peripherals = adapter.peripherals().await.ok()?;
                for p in peripherals {
                    if p.is_connected().await.unwrap_or(false) {
                        let _ = p.disconnect().await;
                        return Some(());
                    }
                }
                None
            });
            let _ = handle;
            return Ok(Value::Null);
        }

        #[cfg(not(feature = "ble"))]
        {
            self.ble_adapter.disconnect(handle);
            Ok(Value::Null)
        }
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

    // ── GUI builtins ──────────────────────────────────────────────

    /// gui_window(title: str, width: i64, height: i64) -> null
    ///
    /// Configures the GUI window title and dimensions. Called before
    /// adding widgets. The window is displayed when the program exits
    /// and `fj gui` reads the accumulated state.
    fn builtin_gui_window(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() < 3 {
            return Err(RuntimeError::ArityMismatch {
                expected: 3,
                got: args.len(),
            }
            .into());
        }
        let title = match &args[0] {
            Value::Str(s) => s.clone(),
            _ => {
                return Err(
                    RuntimeError::TypeError("gui_window: expected string title".into()).into(),
                );
            }
        };
        let width = match &args[1] {
            Value::Int(n) => *n as u32,
            _ => {
                return Err(
                    RuntimeError::TypeError("gui_window: expected int width".into()).into(),
                );
            }
        };
        let height = match &args[2] {
            Value::Int(n) => *n as u32,
            _ => {
                return Err(
                    RuntimeError::TypeError("gui_window: expected int height".into()).into(),
                );
            }
        };
        self.gui_state.title = title;
        self.gui_state.width = width;
        self.gui_state.height = height;
        Ok(Value::Null)
    }

    /// gui_label(text: str, x: i64, y: i64) -> null
    ///
    /// Adds a text label at (x, y).
    fn builtin_gui_label(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() < 3 {
            return Err(RuntimeError::ArityMismatch {
                expected: 3,
                got: args.len(),
            }
            .into());
        }
        let text = match &args[0] {
            Value::Str(s) => s.clone(),
            _ => {
                return Err(
                    RuntimeError::TypeError("gui_label: expected string text".into()).into(),
                );
            }
        };
        let x = match &args[1] {
            Value::Int(n) => *n as u32,
            _ => return Err(RuntimeError::TypeError("gui_label: expected int x".into()).into()),
        };
        let y = match &args[2] {
            Value::Int(n) => *n as u32,
            _ => return Err(RuntimeError::TypeError("gui_label: expected int y".into()).into()),
        };
        // Approximate label width from text length.
        let w = (text.len() as u32) * 8 + 8;
        self.gui_state.widgets.push(super::GuiWidget {
            kind: "label".to_string(),
            text,
            x,
            y,
            w,
            h: 20,
            color: 0xFF_E0_E0_E0,
            on_click: None,
        });
        Ok(Value::Null)
    }

    /// gui_button(text: str, x: i64, y: i64, w: i64, h: i64, on_click: str) -> null
    ///
    /// Adds a button widget at (x, y) with dimensions w×h.
    /// The `on_click` argument is a function name invoked when the button is clicked
    /// (pass `""` for no callback).
    fn builtin_gui_button(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() < 5 {
            return Err(RuntimeError::ArityMismatch {
                expected: 6,
                got: args.len(),
            }
            .into());
        }
        let text = match &args[0] {
            Value::Str(s) => s.clone(),
            _ => {
                return Err(
                    RuntimeError::TypeError("gui_button: expected string text".into()).into(),
                );
            }
        };
        let x = match &args[1] {
            Value::Int(n) => *n as u32,
            _ => return Err(RuntimeError::TypeError("gui_button: expected int x".into()).into()),
        };
        let y = match &args[2] {
            Value::Int(n) => *n as u32,
            _ => return Err(RuntimeError::TypeError("gui_button: expected int y".into()).into()),
        };
        let w = match &args[3] {
            Value::Int(n) => *n as u32,
            _ => return Err(RuntimeError::TypeError("gui_button: expected int w".into()).into()),
        };
        let h = match &args[4] {
            Value::Int(n) => *n as u32,
            _ => return Err(RuntimeError::TypeError("gui_button: expected int h".into()).into()),
        };
        // 6th arg: callback function name ("" = no callback).
        let on_click = if args.len() > 5 {
            match &args[5] {
                Value::Str(s) if !s.is_empty() => Some(s.clone()),
                _ => None,
            }
        } else {
            None
        };
        self.gui_state.widgets.push(super::GuiWidget {
            kind: "button".to_string(),
            text,
            x,
            y,
            w,
            h,
            color: 0xFF_40_80_C0,
            on_click,
        });
        Ok(Value::Null)
    }

    /// gui_layout(mode: str, gap: i64, padding: i64) -> null
    ///
    /// Sets the layout mode for the GUI window.
    /// Modes: "row" (horizontal flex), "column" (vertical flex), "none" (manual).
    fn builtin_gui_layout(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() < 3 {
            return Err(RuntimeError::ArityMismatch {
                expected: 3,
                got: args.len(),
            }
            .into());
        }
        let mode = match &args[0] {
            Value::Str(s) => s.clone(),
            _ => {
                return Err(
                    RuntimeError::TypeError("gui_layout: expected string mode".into()).into(),
                );
            }
        };
        let gap = match &args[1] {
            Value::Int(n) => *n as u32,
            _ => {
                return Err(RuntimeError::TypeError("gui_layout: expected int gap".into()).into());
            }
        };
        let padding = match &args[2] {
            Value::Int(n) => *n as u32,
            _ => {
                return Err(
                    RuntimeError::TypeError("gui_layout: expected int padding".into()).into(),
                );
            }
        };
        self.gui_state.layout_mode = mode;
        self.gui_state.layout_gap = gap;
        self.gui_state.layout_padding = padding;
        Ok(Value::Null)
    }

    /// gui_rect(x: i64, y: i64, w: i64, h: i64, color: i64) -> null
    ///
    /// Draws a filled rectangle at (x, y) with dimensions w×h and color (0xRRGGBB).
    fn builtin_gui_rect(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() < 5 {
            return Err(RuntimeError::ArityMismatch {
                expected: 5,
                got: args.len(),
            }
            .into());
        }
        let x = match &args[0] {
            Value::Int(n) => *n as u32,
            _ => return Err(RuntimeError::TypeError("gui_rect: expected int x".into()).into()),
        };
        let y = match &args[1] {
            Value::Int(n) => *n as u32,
            _ => return Err(RuntimeError::TypeError("gui_rect: expected int y".into()).into()),
        };
        let w = match &args[2] {
            Value::Int(n) => *n as u32,
            _ => return Err(RuntimeError::TypeError("gui_rect: expected int w".into()).into()),
        };
        let h = match &args[3] {
            Value::Int(n) => *n as u32,
            _ => return Err(RuntimeError::TypeError("gui_rect: expected int h".into()).into()),
        };
        let color = match &args[4] {
            Value::Int(n) => 0xFF_00_00_00 | (*n as u32 & 0x00_FF_FF_FF),
            _ => {
                return Err(RuntimeError::TypeError("gui_rect: expected int color".into()).into());
            }
        };
        self.gui_state.widgets.push(super::GuiWidget {
            kind: "rect".to_string(),
            text: String::new(),
            x,
            y,
            w,
            h,
            color,
            on_click: None,
        });
        Ok(Value::Null)
    }

    // ── Regex builtins ──────────────────────────────────────────────

    /// regex_match(pattern: str, text: str) -> bool
    fn builtin_regex_match(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() < 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into());
        }
        let pattern = match &args[0] {
            Value::Str(s) => s.clone(),
            _ => {
                return Err(
                    RuntimeError::TypeError("regex_match: expected string pattern".into()).into(),
                );
            }
        };
        let text = match &args[1] {
            Value::Str(s) => s.clone(),
            _ => {
                return Err(
                    RuntimeError::TypeError("regex_match: expected string text".into()).into(),
                );
            }
        };
        Ok(Value::Bool(crate::stdlib_v3::formats::regex_is_match(
            &pattern, &text,
        )))
    }

    /// regex_find(pattern: str, text: str) -> str | null
    fn builtin_regex_find(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() < 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into());
        }
        let pattern = match &args[0] {
            Value::Str(s) => s.clone(),
            _ => {
                return Err(
                    RuntimeError::TypeError("regex_find: expected string pattern".into()).into(),
                );
            }
        };
        let text = match &args[1] {
            Value::Str(s) => s.clone(),
            _ => {
                return Err(
                    RuntimeError::TypeError("regex_find: expected string text".into()).into(),
                );
            }
        };
        match crate::stdlib_v3::formats::regex_find(&pattern, &text) {
            Some(m) => Ok(Value::Str(m)),
            None => Ok(Value::Null),
        }
    }

    /// regex_find_all(pattern: str, text: str) -> [str]
    fn builtin_regex_find_all(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() < 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into());
        }
        let pattern = match &args[0] {
            Value::Str(s) => s.clone(),
            _ => {
                return Err(
                    RuntimeError::TypeError("regex_find_all: expected string".into()).into(),
                );
            }
        };
        let text = match &args[1] {
            Value::Str(s) => s.clone(),
            _ => {
                return Err(
                    RuntimeError::TypeError("regex_find_all: expected string".into()).into(),
                );
            }
        };
        let matches = crate::stdlib_v3::formats::regex_find_all(&pattern, &text);
        Ok(Value::Array(matches.into_iter().map(Value::Str).collect()))
    }

    /// regex_replace(pattern: str, text: str, replacement: str) -> str
    fn builtin_regex_replace(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() < 3 {
            return Err(RuntimeError::ArityMismatch {
                expected: 3,
                got: args.len(),
            }
            .into());
        }
        let pattern = match &args[0] {
            Value::Str(s) => s.clone(),
            _ => {
                return Err(
                    RuntimeError::TypeError("regex_replace: expected string".into()).into(),
                );
            }
        };
        let text = match &args[1] {
            Value::Str(s) => s.clone(),
            _ => {
                return Err(
                    RuntimeError::TypeError("regex_replace: expected string".into()).into(),
                );
            }
        };
        let repl = match &args[2] {
            Value::Str(s) => s.clone(),
            _ => {
                return Err(
                    RuntimeError::TypeError("regex_replace: expected string".into()).into(),
                );
            }
        };
        Ok(Value::Str(crate::stdlib_v3::formats::regex_replace(
            &pattern, &text, &repl,
        )))
    }

    /// regex_replace_all(pattern: str, text: str, replacement: str) -> str
    fn builtin_regex_replace_all(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() < 3 {
            return Err(RuntimeError::ArityMismatch {
                expected: 3,
                got: args.len(),
            }
            .into());
        }
        let pattern = match &args[0] {
            Value::Str(s) => s.clone(),
            _ => {
                return Err(
                    RuntimeError::TypeError("regex_replace_all: expected string".into()).into(),
                );
            }
        };
        let text = match &args[1] {
            Value::Str(s) => s.clone(),
            _ => {
                return Err(
                    RuntimeError::TypeError("regex_replace_all: expected string".into()).into(),
                );
            }
        };
        let repl = match &args[2] {
            Value::Str(s) => s.clone(),
            _ => {
                return Err(
                    RuntimeError::TypeError("regex_replace_all: expected string".into()).into(),
                );
            }
        };
        Ok(Value::Str(crate::stdlib_v3::formats::regex_replace_all(
            &pattern, &text, &repl,
        )))
    }

    /// regex_captures(pattern: str, text: str) -> [str] | null
    fn builtin_regex_captures(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() < 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into());
        }
        let pattern = match &args[0] {
            Value::Str(s) => s.clone(),
            _ => {
                return Err(
                    RuntimeError::TypeError("regex_captures: expected string".into()).into(),
                );
            }
        };
        let text = match &args[1] {
            Value::Str(s) => s.clone(),
            _ => {
                return Err(
                    RuntimeError::TypeError("regex_captures: expected string".into()).into(),
                );
            }
        };
        match crate::stdlib_v3::formats::regex_captures(&pattern, &text) {
            Some(caps) => Ok(Value::Array(caps.into_iter().map(Value::Str).collect())),
            None => Ok(Value::Null),
        }
    }

    // ── Async builtins (V10) ────────────────────────────────────────

    /// async_sleep(ms: i64) -> Null
    ///
    /// Sleeps for `ms` milliseconds using real tokio::time::sleep.
    /// V19: Blocks directly instead of returning a Future.
    fn builtin_async_sleep(&mut self, args: Vec<Value>) -> EvalResult {
        if args.is_empty() {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: 0,
            }
            .into());
        }
        let ms = match &args[0] {
            Value::Int(n) => *n as u64,
            _ => {
                return Err(RuntimeError::TypeError(
                    "async_sleep: expected int milliseconds".into(),
                )
                .into());
            }
        };
        let rt = self.ensure_tokio_runtime();
        rt.block_on(async {
            tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
        });
        Ok(Value::Null)
    }

    /// async_http_get(url: str) -> Future
    ///
    /// Returns a Future that, when awaited, performs a real HTTP GET request
    /// using tokio::net::TcpStream.
    fn builtin_async_http_get(&mut self, args: Vec<Value>) -> EvalResult {
        if args.is_empty() {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: 0,
            }
            .into());
        }
        let url = match &args[0] {
            Value::Str(s) => s.clone(),
            _ => {
                return Err(
                    RuntimeError::TypeError("async_http_get: expected string URL".into()).into(),
                );
            }
        };
        let task_id = self.next_task_id;
        self.next_task_id += 1;
        self.async_ops
            .insert(task_id, super::AsyncOperation::HttpGet(url));
        Ok(Value::Future { task_id })
    }

    /// async_http_post(url: str, body: str) -> Future
    ///
    /// Returns a Future that, when awaited, performs a real HTTP POST request.
    fn builtin_async_http_post(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() < 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into());
        }
        let url = match &args[0] {
            Value::Str(s) => s.clone(),
            _ => {
                return Err(
                    RuntimeError::TypeError("async_http_post: expected string URL".into()).into(),
                );
            }
        };
        let body = match &args[1] {
            Value::Str(s) => s.clone(),
            _ => {
                return Err(RuntimeError::TypeError(
                    "async_http_post: expected string body".into(),
                )
                .into());
            }
        };
        let task_id = self.next_task_id;
        self.next_task_id += 1;
        self.async_ops
            .insert(task_id, super::AsyncOperation::HttpPost(url, body));
        Ok(Value::Future { task_id })
    }

    /// async_spawn(fn_name: str, ...args) -> i64 (task_id)
    ///
    /// V19: Spawns an async I/O operation on the tokio runtime.
    /// For builtin async ops (async_http_get, etc.), spawns a real tokio task.
    /// For user functions, stores for cooperative evaluation at join time.
    /// Returns a task ID (integer) that can be passed to async_join.
    fn builtin_async_spawn(&mut self, args: Vec<Value>) -> EvalResult {
        if args.is_empty() {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: 0,
            }
            .into());
        }
        let fn_name = match &args[0] {
            Value::Str(s) => s.clone(),
            _ => {
                return Err(
                    RuntimeError::TypeError("async_spawn: expected string fn name".into()).into(),
                );
            }
        };
        let extra_args: Vec<Value> = args.into_iter().skip(1).collect();
        let task_id = self.next_task_id;
        self.next_task_id += 1;

        // For known async I/O builtins, create the appropriate AsyncOperation
        match fn_name.as_str() {
            "async_http_get" => {
                let url = match extra_args.first() {
                    Some(Value::Str(s)) => s.clone(),
                    _ => {
                        return Err(RuntimeError::TypeError(
                            "async_spawn: async_http_get requires string URL arg".into(),
                        )
                        .into());
                    }
                };
                self.async_ops
                    .insert(task_id, super::AsyncOperation::HttpGet(url));
            }
            "async_http_post" => {
                let url = match extra_args.first() {
                    Some(Value::Str(s)) => s.clone(),
                    _ => {
                        return Err(RuntimeError::TypeError(
                            "async_spawn: async_http_post requires string URL arg".into(),
                        )
                        .into());
                    }
                };
                let body = match extra_args.get(1) {
                    Some(Value::Str(s)) => s.clone(),
                    _ => String::new(),
                };
                self.async_ops
                    .insert(task_id, super::AsyncOperation::HttpPost(url, body));
            }
            _ => {
                // Look up user function
                let fn_val = self
                    .env
                    .lock()
                    .expect("env lock")
                    .lookup(&fn_name)
                    .ok_or_else(|| {
                        RuntimeError::TypeError(format!(
                            "async_spawn: function '{fn_name}' not found"
                        ))
                    })?;
                match fn_val {
                    Value::Function(fv) => {
                        self.async_ops.insert(
                            task_id,
                            super::AsyncOperation::Spawn(fv.body.clone(), fv.closure_env.clone()),
                        );
                    }
                    _ => {
                        return Err(RuntimeError::TypeError(format!(
                            "async_spawn: '{fn_name}' is not a function"
                        ))
                        .into());
                    }
                }
            }
        }

        Ok(Value::Int(task_id as i64))
    }

    /// async_join(task_id_or_futures...) -> result or [results]
    ///
    /// V19: Waits for task(s) to complete. Accepts:
    /// - Single Int task_id → returns the task result directly
    /// - Future values or arrays → returns array of results
    fn builtin_async_join(&mut self, args: Vec<Value>) -> EvalResult {
        let mut task_ids: Vec<u64> = Vec::new();
        let mut single_int = false;

        // V19: Accept Int task IDs from async_spawn
        if args.len() == 1 {
            match &args[0] {
                Value::Int(id) => {
                    task_ids.push(*id as u64);
                    single_int = true;
                }
                Value::Future { task_id } => {
                    task_ids.push(*task_id);
                    single_int = true;
                }
                _ => {}
            }
        }
        if task_ids.is_empty() {
            for arg in &args {
                match arg {
                    Value::Int(id) => task_ids.push(*id as u64),
                    Value::Future { task_id } => task_ids.push(*task_id),
                    Value::Array(arr) => {
                        for v in arr {
                            match v {
                                Value::Int(id) => task_ids.push(*id as u64),
                                Value::Future { task_id } => task_ids.push(*task_id),
                                _ => {}
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        if task_ids.is_empty() {
            return Ok(Value::Null);
        }

        // Execute all pending tasks and collect results
        let mut results = Vec::new();
        for tid in &task_ids {
            if let Some(op) = self.async_ops.remove(tid) {
                match self.execute_async_op(op) {
                    Ok(val) => results.push(val),
                    Err(e) => results.push(Value::Enum {
                        variant: "Err".to_string(),
                        data: Some(Box::new(Value::Str(format!("{e}")))),
                    }),
                }
            } else if let Some((body, env)) = self.async_tasks.remove(tid) {
                let prev_env = self.env.clone();
                self.env = env;
                match self.eval_expr(&body) {
                    Ok(val) => results.push(val),
                    Err(e) => results.push(Value::Enum {
                        variant: "Err".to_string(),
                        data: Some(Box::new(Value::Str(format!("{e}")))),
                    }),
                }
                self.env = prev_env;
            } else {
                results.push(Value::Null);
            }
        }

        // Single task → return value directly; multiple → array
        if single_int && results.len() == 1 {
            Ok(results.into_iter().next().unwrap_or(Value::Null))
        } else {
            Ok(Value::Array(results))
        }
    }

    /// async_select(futures...) -> first result
    ///
    /// Returns the result of the first future to complete.
    fn builtin_async_select(&mut self, args: Vec<Value>) -> EvalResult {
        let mut task_ids = Vec::new();
        for arg in &args {
            match arg {
                Value::Future { task_id } => task_ids.push(*task_id),
                Value::Array(arr) => {
                    for v in arr {
                        if let Value::Future { task_id } = v {
                            task_ids.push(*task_id);
                        }
                    }
                }
                _ => {}
            }
        }
        if task_ids.is_empty() {
            return Ok(Value::Null);
        }
        let select_id = self.next_task_id;
        self.next_task_id += 1;
        self.async_ops
            .insert(select_id, super::AsyncOperation::Select(task_ids));
        if let Some(op) = self.async_ops.remove(&select_id) {
            self.execute_async_op(op).map_err(EvalError::Runtime)
        } else {
            Ok(Value::Null)
        }
    }

    /// async_timeout(ms: i64, fn_name: str, ...args) -> Result
    ///
    /// V19: Spawns an async operation with a timeout. Returns Ok(result) if
    /// the operation completes within ms milliseconds, Err("timeout") otherwise.
    fn builtin_async_timeout(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() < 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into());
        }
        let timeout_ms = match &args[0] {
            Value::Int(n) => *n as u64,
            _ => {
                return Err(RuntimeError::TypeError(
                    "async_timeout: first arg must be int ms".into(),
                )
                .into());
            }
        };
        // Spawn the operation
        let spawn_args: Vec<Value> = args.into_iter().skip(1).collect();
        let task_id_val = self.builtin_async_spawn(spawn_args)?;
        let task_id = match &task_id_val {
            Value::Int(id) => *id as u64,
            _ => {
                return Ok(Value::Enum {
                    variant: "Err".to_string(),
                    data: Some(Box::new(Value::Str("timeout: spawn failed".into()))),
                });
            }
        };

        // Execute with timeout (execute_async_op handles its own tokio runtime)
        if let Some(op) = self.async_ops.remove(&task_id) {
            let timeout_dur = std::time::Duration::from_millis(timeout_ms);
            let start = std::time::Instant::now();
            let result = self.execute_async_op(op);
            if start.elapsed() > timeout_dur {
                return Ok(Value::Enum {
                    variant: "Err".to_string(),
                    data: Some(Box::new(Value::Str("timeout".into()))),
                });
            }
            match result {
                Ok(val) => Ok(Value::Enum {
                    variant: "Ok".to_string(),
                    data: Some(Box::new(val)),
                }),
                Err(e) => Ok(Value::Enum {
                    variant: "Err".to_string(),
                    data: Some(Box::new(Value::Str(format!("{e}")))),
                }),
            }
        } else {
            let join_result = self.builtin_async_join(vec![task_id_val])?;
            Ok(Value::Enum {
                variant: "Ok".to_string(),
                data: Some(Box::new(join_result)),
            })
        }
    }

    // ── HTTP Framework builtins (V10 P3) ────────────────────────────

    /// http_server(port: i64) -> i64 handle
    fn builtin_http_server(&mut self, args: Vec<Value>) -> EvalResult {
        if args.is_empty() {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: 0,
            }
            .into());
        }
        let port = match &args[0] {
            Value::Int(n) => *n as u16,
            _ => {
                return Err(
                    RuntimeError::TypeError("http_server: expected int port".into()).into(),
                );
            }
        };
        let id = self.next_http_server_id;
        self.next_http_server_id += 1;
        self.http_servers.insert(
            id,
            super::HttpFrameworkServer {
                port,
                routes: Vec::new(),
                middlewares: Vec::new(),
            },
        );
        Ok(Value::Int(id))
    }

    /// http_route(handle: i64, method: str, pattern: str, handler_fn: str) -> void
    fn builtin_http_route(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() < 4 {
            return Err(RuntimeError::ArityMismatch {
                expected: 4,
                got: args.len(),
            }
            .into());
        }
        let handle = match &args[0] {
            Value::Int(h) => *h,
            _ => {
                return Err(
                    RuntimeError::TypeError("http_route: expected int handle".into()).into(),
                );
            }
        };
        let method = match &args[1] {
            Value::Str(s) => s.to_uppercase(),
            _ => {
                return Err(
                    RuntimeError::TypeError("http_route: expected string method".into()).into(),
                );
            }
        };
        let pattern = match &args[2] {
            Value::Str(s) => s.clone(),
            _ => {
                return Err(
                    RuntimeError::TypeError("http_route: expected string pattern".into()).into(),
                );
            }
        };
        let handler = match &args[3] {
            Value::Str(s) => s.clone(),
            _ => {
                return Err(
                    RuntimeError::TypeError("http_route: expected string handler".into()).into(),
                );
            }
        };
        let server = self.http_servers.get_mut(&handle).ok_or_else(|| {
            RuntimeError::TypeError(format!("http_route: invalid server handle {handle}"))
        })?;
        server.routes.push((method, pattern, handler));
        Ok(Value::Null)
    }

    /// http_middleware(handle: i64, middleware_fn: str) -> void
    fn builtin_http_middleware(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() < 2 {
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
                    RuntimeError::TypeError("http_middleware: expected int handle".into()).into(),
                );
            }
        };
        let mw = match &args[1] {
            Value::Str(s) => s.clone(),
            _ => {
                return Err(RuntimeError::TypeError(
                    "http_middleware: expected string name".into(),
                )
                .into());
            }
        };
        let server = self.http_servers.get_mut(&handle).ok_or_else(|| {
            RuntimeError::TypeError(format!("http_middleware: invalid handle {handle}"))
        })?;
        server.middlewares.push(mw);
        Ok(Value::Null)
    }

    /// http_start(handle: i64, max_requests: i64) -> i64 (requests served)
    ///
    /// Starts the HTTP server, dispatches requests through middleware + router,
    /// invokes handler functions defined in the .fj program.
    fn builtin_http_start(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() < 2 {
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
                    RuntimeError::TypeError("http_start: expected int handle".into()).into(),
                );
            }
        };
        let max_requests = match &args[1] {
            Value::Int(n) => *n as usize,
            _ => {
                return Err(RuntimeError::TypeError(
                    "http_start: expected int max_requests".into(),
                )
                .into());
            }
        };

        // Take server config (remove from map to avoid borrow issues).
        let server = self.http_servers.remove(&handle).ok_or_else(|| {
            RuntimeError::TypeError(format!("http_start: invalid handle {handle}"))
        })?;

        let addr = format!("127.0.0.1:{}", server.port);
        let listener = std::net::TcpListener::bind(&addr)
            .map_err(|e| RuntimeError::TypeError(format!("http_start: bind {addr}: {e}")))?;

        if self.capture_output {
            self.output
                .push(format!("[http] Listening on {addr} (max {max_requests})"));
        } else {
            println!("[http] Listening on {addr} (max {max_requests} requests)");
        }

        let mut served = 0i64;
        for stream in listener.incoming().take(max_requests) {
            match stream {
                Ok(mut stream) => {
                    use std::io::{BufRead, BufReader, Write};
                    let mut reader = BufReader::new(&stream);

                    // Parse request line.
                    let mut request_line = String::new();
                    if reader.read_line(&mut request_line).is_err() {
                        continue;
                    }
                    let parts: Vec<&str> = request_line.trim().splitn(3, ' ').collect();
                    let (method, path) = if parts.len() >= 2 {
                        (parts[0].to_string(), parts[1].to_string())
                    } else {
                        continue;
                    };

                    // Read headers + body.
                    let mut headers = std::collections::HashMap::new();
                    let mut content_length = 0usize;
                    loop {
                        let mut hdr = String::new();
                        if reader.read_line(&mut hdr).is_err() || hdr.trim().is_empty() {
                            break;
                        }
                        if let Some((k, v)) = hdr.split_once(':') {
                            let key = k.trim().to_lowercase();
                            let val = v.trim().to_string();
                            if key == "content-length" {
                                content_length = val.parse().unwrap_or(0);
                            }
                            headers.insert(key, val);
                        }
                    }
                    let mut body = vec![0u8; content_length];
                    if content_length > 0 {
                        let _ = std::io::Read::read_exact(&mut reader, &mut body);
                    }
                    let body_str = String::from_utf8_lossy(&body).to_string();

                    // Run middleware (each middleware fn gets method, path, body as args).
                    for mw in &server.middlewares {
                        let call = format!(
                            "{mw}(\"{method}\", \"{path}\", \"{}\")",
                            body_str.replace('\"', "\\\"")
                        );
                        let _ = self.eval_source(&call);
                    }

                    // Route matching.
                    let mut response_body = String::new();
                    let mut status = 404u16;
                    let mut matched = false;

                    for (route_method, pattern, handler) in &server.routes {
                        if *route_method != method {
                            continue;
                        }
                        if let Some(params) = crate::stdlib_v3::net::match_route(pattern, &path) {
                            // Build handler call with method, path, body, params_json.
                            let params_json = format!(
                                "{{{}}}",
                                params
                                    .iter()
                                    .map(|(k, v)| format!("\"{k}\": \"{v}\""))
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            );
                            let call = format!(
                                "{handler}(\"{method}\", \"{path}\", \"{}\", '{params_json}')",
                                body_str.replace('\"', "\\\"")
                            );
                            match self.eval_source(&call) {
                                Ok(Value::Str(s)) => {
                                    response_body = s;
                                    status = 200;
                                }
                                Ok(Value::Int(code)) => {
                                    status = code as u16;
                                }
                                Ok(other) => {
                                    response_body = format!("{other}");
                                    status = 200;
                                }
                                Err(e) => {
                                    response_body = format!("{{\"error\": \"{e}\"}}");
                                    status = 500;
                                }
                            }
                            matched = true;
                            break;
                        }
                    }

                    if !matched {
                        response_body = format!("{{\"error\": \"no route: {method} {path}\"}}");
                    }

                    let status_text = match status {
                        200 => "OK",
                        201 => "Created",
                        204 => "No Content",
                        400 => "Bad Request",
                        404 => "Not Found",
                        500 => "Internal Server Error",
                        _ => "Unknown",
                    };
                    let resp = format!(
                        "HTTP/1.1 {status} {status_text}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response_body}",
                        response_body.len()
                    );
                    let _ = stream.write_all(resp.as_bytes());
                    served += 1;
                }
                Err(_) => break,
            }
        }
        Ok(Value::Int(served))
    }

    /// http_start_tls(handle: i64, max_requests: i64, cert_path: str, key_path: str) -> i64
    ///
    /// Starts an HTTPS server with TLS using a PEM certificate and key.
    /// Requires `--features https`.
    fn builtin_http_start_tls(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() < 4 {
            return Err(RuntimeError::ArityMismatch {
                expected: 4,
                got: args.len(),
            }
            .into());
        }
        let handle = match &args[0] {
            Value::Int(h) => *h,
            _ => {
                return Err(
                    RuntimeError::TypeError("http_start_tls: expected int handle".into()).into(),
                );
            }
        };
        let max_requests = match &args[1] {
            Value::Int(n) => *n as usize,
            _ => {
                return Err(RuntimeError::TypeError(
                    "http_start_tls: expected int max_requests".into(),
                )
                .into());
            }
        };
        let cert_path = match &args[2] {
            Value::Str(s) => s.clone(),
            _ => {
                return Err(RuntimeError::TypeError(
                    "http_start_tls: expected string cert_path".into(),
                )
                .into());
            }
        };
        let key_path = match &args[3] {
            Value::Str(s) => s.clone(),
            _ => {
                return Err(RuntimeError::TypeError(
                    "http_start_tls: expected string key_path".into(),
                )
                .into());
            }
        };

        #[cfg(feature = "https")]
        {
            let server = self.http_servers.remove(&handle).ok_or_else(|| {
                RuntimeError::TypeError(format!("http_start_tls: invalid handle {handle}"))
            })?;

            // Read certificate and key PEM files.
            let cert_pem = std::fs::read(&cert_path)
                .map_err(|e| RuntimeError::TypeError(format!("http_start_tls: read cert: {e}")))?;
            let key_pem = std::fs::read(&key_path)
                .map_err(|e| RuntimeError::TypeError(format!("http_start_tls: read key: {e}")))?;

            let identity = native_tls::Identity::from_pkcs8(&cert_pem, &key_pem)
                .map_err(|e| RuntimeError::TypeError(format!("http_start_tls: identity: {e}")))?;
            let acceptor = native_tls::TlsAcceptor::new(identity)
                .map_err(|e| RuntimeError::TypeError(format!("http_start_tls: acceptor: {e}")))?;

            let addr = format!("127.0.0.1:{}", server.port);
            let listener = std::net::TcpListener::bind(&addr).map_err(|e| {
                RuntimeError::TypeError(format!("http_start_tls: bind {addr}: {e}"))
            })?;

            if self.capture_output {
                self.output
                    .push(format!("[https] Listening on {addr} (TLS)"));
            } else {
                println!("[https] Listening on {addr} (TLS, max {max_requests} requests)");
            }

            let mut served = 0i64;
            for stream in listener.incoming().take(max_requests) {
                match stream {
                    Ok(tcp_stream) => {
                        let tls_stream = match acceptor.accept(tcp_stream) {
                            Ok(s) => s,
                            Err(_) => continue,
                        };
                        use std::io::{Read as IoRead, Write};
                        let mut tls_stream = tls_stream;
                        // Read the full request (up to 8KB).
                        let mut buf = vec![0u8; 8192];
                        let n = tls_stream.read(&mut buf).unwrap_or(0);
                        let request_str = String::from_utf8_lossy(&buf[..n]);
                        let first_line = request_str.lines().next().unwrap_or("");
                        let parts: Vec<&str> = first_line.splitn(3, ' ').collect();
                        let (method, path) = if parts.len() >= 2 {
                            (parts[0].to_string(), parts[1].to_string())
                        } else {
                            continue;
                        };

                        // Route matching (same as http_start).
                        let mut response_body = String::new();
                        let mut status = 404u16;
                        for (route_method, pattern, handler) in &server.routes {
                            if *route_method != method {
                                continue;
                            }
                            if crate::stdlib_v3::net::match_route(pattern, &path).is_some() {
                                let call =
                                    format!("{handler}(\"{method}\", \"{path}\", \"\", \"{{}}\")");
                                match self.eval_source(&call) {
                                    Ok(Value::Str(s)) => {
                                        response_body = s;
                                        status = 200;
                                    }
                                    Ok(other) => {
                                        response_body = format!("{other}");
                                        status = 200;
                                    }
                                    Err(e) => {
                                        response_body = format!("{{\"error\": \"{e}\"}}");
                                        status = 500;
                                    }
                                }
                                break;
                            }
                        }
                        if response_body.is_empty() && status == 404 {
                            response_body = format!("{{\"error\": \"no route: {method} {path}\"}}");
                        }

                        let status_text = match status {
                            200 => "OK",
                            404 => "Not Found",
                            500 => "Internal Server Error",
                            _ => "Unknown",
                        };
                        let resp = format!(
                            "HTTP/1.1 {status} {status_text}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response_body}",
                            response_body.len()
                        );
                        let _ = tls_stream.write_all(resp.as_bytes());
                        served += 1;
                    }
                    Err(_) => break,
                }
            }
            return Ok(Value::Int(served));
        }

        #[cfg(not(feature = "https"))]
        {
            let _ = (handle, max_requests, cert_path, key_path);
            Err(RuntimeError::TypeError(
                "http_start_tls: requires --features https (native-tls)".into(),
            )
            .into())
        }
    }

    /// request_json(body: str) -> Map
    ///
    /// Parse a JSON string into a Map value.
    fn builtin_request_json(&mut self, args: Vec<Value>) -> EvalResult {
        if args.is_empty() {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: 0,
            }
            .into());
        }
        let body = match &args[0] {
            Value::Str(s) => s.clone(),
            _ => {
                return Err(RuntimeError::TypeError("request_json: expected string".into()).into());
            }
        };
        // Parse JSON string into a Map value.
        use crate::stdlib_v3::formats::JsonValue;
        fn json_to_value(jv: &JsonValue) -> Value {
            match jv {
                JsonValue::String(s) => Value::Str(s.clone()),
                JsonValue::Number(n) => {
                    if n.fract() == 0.0 {
                        Value::Int(*n as i64)
                    } else {
                        Value::Float(*n)
                    }
                }
                JsonValue::Bool(b) => Value::Bool(*b),
                JsonValue::Null => Value::Null,
                JsonValue::Array(arr) => Value::Array(arr.iter().map(json_to_value).collect()),
                JsonValue::Object(entries) => {
                    let map: HashMap<String, Value> = entries
                        .iter()
                        .map(|(k, v)| (k.clone(), json_to_value(v)))
                        .collect();
                    Value::Map(map)
                }
            }
        }
        match crate::stdlib_v3::formats::json_parse(&body) {
            Ok(json_val) => Ok(json_to_value(&json_val)),
            Err(_) => Ok(Value::Null),
        }
    }

    /// response_json(status: i64, body: str) -> str
    ///
    /// Format a JSON response string with proper HTTP status.
    fn builtin_response_json(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() < 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into());
        }
        let status = match &args[0] {
            Value::Int(n) => *n,
            _ => {
                return Err(
                    RuntimeError::TypeError("response_json: expected int status".into()).into(),
                );
            }
        };
        let body = match &args[1] {
            Value::Str(s) => s.clone(),
            _ => {
                return Err(
                    RuntimeError::TypeError("response_json: expected string body".into()).into(),
                );
            }
        };
        Ok(Value::Str(format!(
            "{{\"status\": {status}, \"data\": {body}}}"
        )))
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
