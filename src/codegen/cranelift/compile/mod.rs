//! Free-standing codegen functions for Fajar Lang compilation.
//!
//! These functions take `&mut FunctionBuilder` and `&mut CodegenCtx` as parameters
//! (avoids lifetime issues with mutable builder borrows).

mod arrays;
mod builtins;
mod control;
mod expr;
mod stmt;
mod strings;
mod structs;

pub(in crate::codegen::cranelift) use arrays::*;
pub(in crate::codegen::cranelift) use builtins::*;
pub(in crate::codegen::cranelift) use control::*;
pub(in crate::codegen::cranelift) use expr::*;
pub(in crate::codegen::cranelift) use stmt::*;
pub(in crate::codegen::cranelift) use strings::*;
pub(in crate::codegen::cranelift) use structs::*;

use cranelift_codegen::ir::condcodes::IntCC;
use cranelift_codegen::ir::{InstBuilder, Value as ClifValue};
use cranelift_frontend::FunctionBuilder;
use cranelift_module::Module;

use super::clif_types;
use super::context::{CodegenCtx, OwnedKind};
use crate::codegen::CodegenError;
use crate::parser::ast::{CallArg, Expr, LiteralKind};

// NOTE: compile_stmt has been extracted to stmt.rs.
// NOTE: compile_expr, compile_tuple, compile_cast, compile_literal,
// compile_ident, compile_path, compile_unary, compile_binop,
// compile_int_binop, compile_float_binop, compile_short_circuit
// have been extracted to expr.rs.

// ═══════════════════════════════════════════════════════════════════════
// Function call compilation
// ═══════════════════════════════════════════════════════════════════════

/// Compiles a function call expression.
///
/// Handles builtins (println, print, abs, sqrt, etc.), enum constructors,
/// closure calls, generic dispatch, and regular user-defined function calls.
pub(in crate::codegen::cranelift) fn compile_call<M: Module>(
    builder: &mut FunctionBuilder,
    cx: &mut CodegenCtx<'_, M>,
    callee: &Expr,
    args: &[CallArg],
) -> Result<ClifValue, CodegenError> {
    // Path-based calls: Type::method(args) or Enum::Variant(args)
    if let Expr::Path { segments, .. } = callee {
        if segments.len() == 2 {
            return compile_path_call(builder, cx, &segments[0], &segments[1], args);
        }
    }

    let fn_name = match callee {
        Expr::Ident { name, .. } => name.clone(),
        _ => {
            return Err(CodegenError::NotImplemented(
                "call on non-ident callee".into(),
            ))
        }
    };

    // ── Enum constructors ─────────────────────────────────────────────
    match fn_name.as_str() {
        "Some" | "Ok" | "Err" => {
            return compile_enum_constructor(builder, cx, &fn_name, args);
        }
        "None" => {
            cx.last_expr_type = Some(clif_types::default_int_type());
            cx.last_enum_payload = Some(builder.ins().iconst(clif_types::default_int_type(), 0));
            cx.last_enum_payload_type = Some(clif_types::default_int_type());
            return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
        }
        _ => {}
    }
    // User-defined enum constructors (delegates to compile_enum_constructor
    // which handles both single-field and multi-field variants)
    {
        let mut found_variant = false;
        for variants in cx.enum_defs.values() {
            if variants.iter().any(|v| v == &fn_name) {
                found_variant = true;
                break;
            }
        }
        if found_variant {
            return compile_enum_constructor(builder, cx, &fn_name, args);
        }
    }

    // ── Builtin dispatch (only if no user-defined function with that name) ──
    let is_user_fn = cx.functions.contains_key(&fn_name)
        && !matches!(
            fn_name.as_str(),
            "println" | "print" | "eprintln" | "eprint"
        );

    if !is_user_fn {
        match fn_name.as_str() {
            "println" | "print" | "eprintln" | "eprint" => {
                return compile_print_builtin(builder, cx, &fn_name, args);
            }
            "dbg" => {
                return compile_dbg_builtin(builder, cx, args);
            }
            "abs" | "sqrt" | "floor" | "ceil" | "round" => {
                return compile_math_unary_builtin(builder, cx, &fn_name, args);
            }
            "sin" | "cos" | "tan" | "log" | "log2" | "log10" => {
                return compile_math_rt_builtin(builder, cx, &fn_name, args);
            }
            "pow" => {
                return compile_pow_builtin(builder, cx, args);
            }
            "min" | "max" => {
                return compile_min_max_builtin(builder, cx, &fn_name, args);
            }
            "clamp" => {
                return compile_clamp_builtin(builder, cx, args);
            }
            "len" => {
                return compile_len_builtin(builder, cx, args);
            }
            "to_string" => {
                return compile_to_string_builtin(builder, cx, args);
            }
            "to_int" | "to_float" => {
                return compile_convert_builtin(builder, cx, &fn_name, args);
            }
            "type_of" => {
                return compile_type_of_builtin(builder, cx, args);
            }
            "assert" => {
                return compile_assert_builtin(builder, cx, args);
            }
            "assert_eq" => {
                return compile_assert_eq_builtin(builder, cx, args);
            }
            "panic" | "todo" => {
                return compile_panic_builtin(builder, cx, &fn_name, args);
            }
            "format" => {
                return compile_format_builtin(builder, cx, args);
            }
            "write_file" | "read_file" | "append_file" | "file_exists" | "async_read_file"
            | "async_write_file" => {
                return compile_file_builtin(builder, cx, &fn_name, args);
            }
            "wrapping_add" | "wrapping_sub" | "wrapping_mul" | "saturating_add"
            | "saturating_sub" | "saturating_mul" | "checked_add" | "checked_sub"
            | "checked_mul" => {
                return compile_wrapping_builtin(builder, cx, &fn_name, args);
            }
            "sleep" => {
                let millis = if args.is_empty() {
                    builder.ins().iconst(clif_types::default_int_type(), 0)
                } else {
                    compile_expr(builder, cx, &args[0].value)?
                };
                let sleep_id = *cx
                    .functions
                    .get("__sleep")
                    .ok_or_else(|| CodegenError::Internal("__sleep not declared".into()))?;
                let callee = cx.module.declare_func_in_func(sleep_id, builder.func);
                builder.ins().call(callee, &[millis]);
                cx.last_expr_type = Some(clif_types::default_int_type());
                return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
            }
            "channel_select" => {
                if args.len() < 2 {
                    return Err(CodegenError::NotImplemented(
                        "channel_select requires 2 channel arguments".into(),
                    ));
                }
                let ch1 = compile_expr(builder, cx, &args[0].value)?;
                let ch2 = compile_expr(builder, cx, &args[1].value)?;
                let fn_id = *cx.functions.get("__channel_select2").ok_or_else(|| {
                    CodegenError::Internal("__channel_select2 not declared".into())
                })?;
                let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                let call = builder.ins().call(callee, &[ch1, ch2]);
                let result = builder.inst_results(call)[0];
                cx.last_expr_type = Some(clif_types::default_int_type());
                return Ok(result);
            }
            "tls_set" => {
                if args.len() < 2 {
                    return Err(CodegenError::NotImplemented(
                        "tls_set requires 2 arguments (key, value)".into(),
                    ));
                }
                let key = compile_expr(builder, cx, &args[0].value)?;
                let value = compile_expr(builder, cx, &args[1].value)?;
                let fn_id = *cx
                    .functions
                    .get("__tls_set")
                    .ok_or_else(|| CodegenError::Internal("__tls_set not declared".into()))?;
                let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                builder.ins().call(callee, &[key, value]);
                cx.last_expr_type = Some(clif_types::default_int_type());
                return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
            }
            "tls_get" => {
                if args.is_empty() {
                    return Err(CodegenError::NotImplemented(
                        "tls_get requires 1 argument (key)".into(),
                    ));
                }
                let key = compile_expr(builder, cx, &args[0].value)?;
                let fn_id = *cx
                    .functions
                    .get("__tls_get")
                    .ok_or_else(|| CodegenError::Internal("__tls_get not declared".into()))?;
                let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                let call = builder.ins().call(callee, &[key]);
                let result = builder.inst_results(call)[0];
                cx.last_expr_type = Some(clif_types::default_int_type());
                return Ok(result);
            }
            "volatile_read" => {
                if args.is_empty() {
                    return Err(CodegenError::NotImplemented(
                        "volatile_read requires 1 argument (address)".into(),
                    ));
                }
                let addr = compile_expr(builder, cx, &args[0].value)?;
                let fn_id = *cx
                    .functions
                    .get("__volatile_read")
                    .ok_or_else(|| CodegenError::Internal("__volatile_read not declared".into()))?;
                let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                let call = builder.ins().call(callee, &[addr]);
                let result = builder.inst_results(call)[0];
                cx.last_expr_type = Some(clif_types::default_int_type());
                return Ok(result);
            }
            "volatile_write" => {
                if args.len() < 2 {
                    return Err(CodegenError::NotImplemented(
                        "volatile_write requires 2 arguments (address, value)".into(),
                    ));
                }
                let addr = compile_expr(builder, cx, &args[0].value)?;
                let value = compile_expr(builder, cx, &args[1].value)?;
                let fn_id = *cx.functions.get("__volatile_write").ok_or_else(|| {
                    CodegenError::Internal("__volatile_write not declared".into())
                })?;
                let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                builder.ins().call(callee, &[addr, value]);
                cx.last_expr_type = Some(clif_types::default_int_type());
                return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
            }
            "volatile_read_u8" | "volatile_read_u16" | "volatile_read_u32" => {
                if args.is_empty() {
                    return Err(CodegenError::NotImplemented(format!(
                        "{fn_name} requires 1 argument (address)"
                    )));
                }
                let addr = compile_expr(builder, cx, &args[0].value)?;
                let internal = format!("__{fn_name}");
                let fn_id = *cx
                    .functions
                    .get(&internal)
                    .ok_or_else(|| CodegenError::Internal(format!("{internal} not declared")))?;
                let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                let call = builder.ins().call(callee, &[addr]);
                let result = builder.inst_results(call)[0];
                cx.last_expr_type = Some(clif_types::default_int_type());
                return Ok(result);
            }
            "volatile_write_u8" | "volatile_write_u16" | "volatile_write_u32" => {
                if args.len() < 2 {
                    return Err(CodegenError::NotImplemented(format!(
                        "{fn_name} requires 2 arguments (address, value)"
                    )));
                }
                let addr = compile_expr(builder, cx, &args[0].value)?;
                let value = compile_expr(builder, cx, &args[1].value)?;
                let internal = format!("__{fn_name}");
                let fn_id = *cx
                    .functions
                    .get(&internal)
                    .ok_or_else(|| CodegenError::Internal(format!("{internal} not declared")))?;
                let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                builder.ins().call(callee, &[addr, value]);
                cx.last_expr_type = Some(clif_types::default_int_type());
                return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
            }
            "compiler_fence" => {
                let fn_id = *cx.functions.get("__compiler_fence").ok_or_else(|| {
                    CodegenError::Internal("__compiler_fence not declared".into())
                })?;
                let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                builder.ins().call(callee, &[]);
                cx.last_expr_type = Some(clif_types::default_int_type());
                return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
            }
            "memory_fence" => {
                let fn_id = *cx
                    .functions
                    .get("__memory_fence")
                    .ok_or_else(|| CodegenError::Internal("__memory_fence not declared".into()))?;
                let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                builder.ins().call(callee, &[]);
                cx.last_expr_type = Some(clif_types::default_int_type());
                return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
            }
            "alloc" => {
                let size = if args.is_empty() {
                    builder.ins().iconst(clif_types::default_int_type(), 8)
                } else {
                    compile_expr(builder, cx, &args[0].value)?
                };
                let fn_id = *cx
                    .functions
                    .get("__alloc")
                    .ok_or_else(|| CodegenError::Internal("__alloc not declared".into()))?;
                let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                let call = builder.ins().call(callee, &[size]);
                let result = builder.inst_results(call)[0];
                cx.last_expr_type = Some(clif_types::default_int_type());
                return Ok(result);
            }
            "dealloc" => {
                if args.len() < 2 {
                    return Err(CodegenError::NotImplemented(
                        "dealloc requires 2 arguments (ptr, size)".into(),
                    ));
                }
                let ptr = compile_expr(builder, cx, &args[0].value)?;
                let size = compile_expr(builder, cx, &args[1].value)?;
                let fn_id = *cx
                    .functions
                    .get("__free")
                    .ok_or_else(|| CodegenError::Internal("__free not declared".into()))?;
                let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                builder.ins().call(callee, &[ptr, size]);
                cx.last_expr_type = Some(clif_types::default_int_type());
                return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
            }
            "mem_read" => {
                if args.len() < 2 {
                    return Err(CodegenError::NotImplemented(
                        "mem_read requires 2 arguments (ptr, offset)".into(),
                    ));
                }
                let ptr = compile_expr(builder, cx, &args[0].value)?;
                let offset = compile_expr(builder, cx, &args[1].value)?;
                let fn_id = *cx
                    .functions
                    .get("__mem_read")
                    .ok_or_else(|| CodegenError::Internal("__mem_read not declared".into()))?;
                let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                let call = builder.ins().call(callee, &[ptr, offset]);
                let result = builder.inst_results(call)[0];
                cx.last_expr_type = Some(clif_types::default_int_type());
                return Ok(result);
            }
            "mem_write" => {
                if args.len() < 3 {
                    return Err(CodegenError::NotImplemented(
                        "mem_write requires 3 arguments (ptr, offset, value)".into(),
                    ));
                }
                let ptr = compile_expr(builder, cx, &args[0].value)?;
                let offset = compile_expr(builder, cx, &args[1].value)?;
                let value = compile_expr(builder, cx, &args[2].value)?;
                let fn_id = *cx
                    .functions
                    .get("__mem_write")
                    .ok_or_else(|| CodegenError::Internal("__mem_write not declared".into()))?;
                let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                builder.ins().call(callee, &[ptr, offset, value]);
                cx.last_expr_type = Some(clif_types::default_int_type());
                return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
            }
            "tensor_zeros" | "tensor_ones" | "zeros" | "ones" => {
                if args.len() < 2 {
                    return Err(CodegenError::NotImplemented(format!(
                        "{fn_name} requires 2 arguments (rows, cols)"
                    )));
                }
                let rows = compile_expr(builder, cx, &args[0].value)?;
                let cols = compile_expr(builder, cx, &args[1].value)?;
                let canon = match fn_name.as_str() {
                    "zeros" => "tensor_zeros",
                    "ones" => "tensor_ones",
                    other => other,
                };
                let key = format!("__{canon}");
                let fn_id = *cx
                    .functions
                    .get(&key)
                    .ok_or_else(|| CodegenError::Internal(format!("{key} not declared")))?;
                let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                let call = builder.ins().call(callee, &[rows, cols]);
                let result = builder.inst_results(call)[0];
                cx.last_expr_type = Some(clif_types::default_int_type());
                return Ok(result);
            }
            "tensor_add" | "tensor_sub" | "tensor_mul" | "tensor_matmul" | "matmul" => {
                if args.len() < 2 {
                    return Err(CodegenError::NotImplemented(format!(
                        "{fn_name} requires 2 arguments"
                    )));
                }
                let a = compile_expr(builder, cx, &args[0].value)?;
                let b = compile_expr(builder, cx, &args[1].value)?;
                let canon = match fn_name.as_str() {
                    "matmul" => "tensor_matmul",
                    other => other,
                };
                let key = format!("__{canon}");
                let fn_id = *cx
                    .functions
                    .get(&key)
                    .ok_or_else(|| CodegenError::Internal(format!("{key} not declared")))?;
                let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                let call = builder.ins().call(callee, &[a, b]);
                let result = builder.inst_results(call)[0];
                cx.last_expr_type = Some(clif_types::default_int_type());
                return Ok(result);
            }
            "tensor_reshape" => {
                if args.len() < 3 {
                    return Err(CodegenError::NotImplemented(
                        "tensor_reshape requires 3 arguments (tensor, rows, cols)".into(),
                    ));
                }
                let t = compile_expr(builder, cx, &args[0].value)?;
                let rows = compile_expr(builder, cx, &args[1].value)?;
                let cols = compile_expr(builder, cx, &args[2].value)?;
                let fn_id = *cx.functions.get("__tensor_reshape").ok_or_else(|| {
                    CodegenError::Internal("__tensor_reshape not declared".into())
                })?;
                let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                let call = builder.ins().call(callee, &[t, rows, cols]);
                let result = builder.inst_results(call)[0];
                cx.last_expr_type = Some(clif_types::default_int_type());
                return Ok(result);
            }
            "tensor_transpose" | "tensor_relu" | "tensor_softmax" | "tensor_sigmoid"
            | "tensor_flatten" | "relu" | "softmax" | "sigmoid" | "transpose" => {
                if args.is_empty() {
                    return Err(CodegenError::NotImplemented(format!(
                        "{fn_name} requires 1 argument"
                    )));
                }
                let t = compile_expr(builder, cx, &args[0].value)?;
                let canon = match fn_name.as_str() {
                    "relu" => "tensor_relu",
                    "softmax" => "tensor_softmax",
                    "sigmoid" => "tensor_sigmoid",
                    "transpose" => "tensor_transpose",
                    other => other,
                };
                let key = format!("__{canon}");
                let fn_id = *cx
                    .functions
                    .get(&key)
                    .ok_or_else(|| CodegenError::Internal(format!("{key} not declared")))?;
                let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                let call = builder.ins().call(callee, &[t]);
                let result = builder.inst_results(call)[0];
                cx.last_expr_type = Some(clif_types::default_int_type());
                return Ok(result);
            }
            "tensor_rows" | "tensor_cols" | "tensor_sum" => {
                if args.is_empty() {
                    return Err(CodegenError::NotImplemented(format!(
                        "{fn_name} requires 1 argument"
                    )));
                }
                let t = compile_expr(builder, cx, &args[0].value)?;
                let key = format!("__{fn_name}");
                let fn_id = *cx
                    .functions
                    .get(&key)
                    .ok_or_else(|| CodegenError::Internal(format!("{key} not declared")))?;
                let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                let call = builder.ins().call(callee, &[t]);
                let result = builder.inst_results(call)[0];
                cx.last_expr_type = Some(clif_types::default_int_type());
                return Ok(result);
            }
            "tensor_get" => {
                if args.len() < 3 {
                    return Err(CodegenError::NotImplemented(
                        "tensor_get requires 3 arguments (tensor, row, col)".into(),
                    ));
                }
                let t = compile_expr(builder, cx, &args[0].value)?;
                let row = compile_expr(builder, cx, &args[1].value)?;
                let col = compile_expr(builder, cx, &args[2].value)?;
                let fn_id = *cx
                    .functions
                    .get("__tensor_get")
                    .ok_or_else(|| CodegenError::Internal("__tensor_get not declared".into()))?;
                let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                let call = builder.ins().call(callee, &[t, row, col]);
                let result = builder.inst_results(call)[0];
                cx.last_expr_type = Some(cranelift_codegen::ir::types::F64);
                return Ok(result);
            }
            "tensor_set" => {
                if args.len() < 4 {
                    return Err(CodegenError::NotImplemented(
                        "tensor_set requires 4 arguments (tensor, row, col, value)".into(),
                    ));
                }
                let t = compile_expr(builder, cx, &args[0].value)?;
                let row = compile_expr(builder, cx, &args[1].value)?;
                let col = compile_expr(builder, cx, &args[2].value)?;
                let val = compile_expr(builder, cx, &args[3].value)?;
                let fn_id = *cx
                    .functions
                    .get("__tensor_set")
                    .ok_or_else(|| CodegenError::Internal("__tensor_set not declared".into()))?;
                let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                builder.ins().call(callee, &[t, row, col, val]);
                cx.last_expr_type = Some(clif_types::default_int_type());
                return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
            }
            "tensor_free" => {
                if args.is_empty() {
                    return Err(CodegenError::NotImplemented(
                        "tensor_free requires 1 argument".into(),
                    ));
                }
                let t = compile_expr(builder, cx, &args[0].value)?;
                let fn_id = *cx
                    .functions
                    .get("__tensor_free")
                    .ok_or_else(|| CodegenError::Internal("__tensor_free not declared".into()))?;
                let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                builder.ins().call(callee, &[t]);
                cx.last_expr_type = Some(clif_types::default_int_type());
                return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
            }
            // --- Autograd builtins ---
            "backward" => {
                // backward(loss_tensor) — autograd backward pass (simplified no-op in native)
                cx.last_expr_type = Some(clif_types::default_int_type());
                return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
            }
            "requires_grad" | "set_requires_grad" => {
                if args.is_empty() {
                    return Err(CodegenError::NotImplemented(
                        "requires_grad requires 1 argument".into(),
                    ));
                }
                let t = compile_expr(builder, cx, &args[0].value)?;
                let fn_id = *cx.functions.get("__tensor_requires_grad").ok_or_else(|| {
                    CodegenError::Internal("__tensor_requires_grad not declared".into())
                })?;
                let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                let call = builder.ins().call(callee, &[t]);
                let result = builder.inst_results(call)[0];
                cx.last_expr_type = Some(clif_types::pointer_type());
                return Ok(result);
            }
            "mse_loss" => {
                if args.len() < 2 {
                    return Err(CodegenError::NotImplemented(
                        "mse_loss requires 2 arguments".into(),
                    ));
                }
                let pred = compile_expr(builder, cx, &args[0].value)?;
                let target = compile_expr(builder, cx, &args[1].value)?;
                let fn_id = *cx
                    .functions
                    .get("__mse_loss")
                    .ok_or_else(|| CodegenError::Internal("__mse_loss not declared".into()))?;
                let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                let call = builder.ins().call(callee, &[pred, target]);
                let result = builder.inst_results(call)[0];
                cx.last_expr_type = Some(clif_types::default_int_type());
                return Ok(result);
            }
            "cross_entropy_loss" | "cross_entropy" => {
                if args.len() < 2 {
                    return Err(CodegenError::NotImplemented(
                        "cross_entropy_loss requires 2 arguments".into(),
                    ));
                }
                let pred = compile_expr(builder, cx, &args[0].value)?;
                let target = compile_expr(builder, cx, &args[1].value)?;
                let fn_id = *cx.functions.get("__cross_entropy_loss").ok_or_else(|| {
                    CodegenError::Internal("__cross_entropy_loss not declared".into())
                })?;
                let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                let call = builder.ins().call(callee, &[pred, target]);
                let result = builder.inst_results(call)[0];
                cx.last_expr_type = Some(clif_types::default_int_type());
                return Ok(result);
            }
            "tensor_grad" | "grad" => {
                if args.is_empty() {
                    return Err(CodegenError::NotImplemented(
                        "tensor_grad requires 1 argument".into(),
                    ));
                }
                let t = compile_expr(builder, cx, &args[0].value)?;
                let fn_id = *cx
                    .functions
                    .get("__tensor_grad")
                    .ok_or_else(|| CodegenError::Internal("__tensor_grad not declared".into()))?;
                let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                let call = builder.ins().call(callee, &[t]);
                let result = builder.inst_results(call)[0];
                cx.last_expr_type = Some(clif_types::pointer_type());
                return Ok(result);
            }
            "zero_grad" => {
                if args.is_empty() {
                    return Err(CodegenError::NotImplemented(
                        "zero_grad requires 1 argument".into(),
                    ));
                }
                let t = compile_expr(builder, cx, &args[0].value)?;
                let fn_id = *cx.functions.get("__tensor_zero_grad").ok_or_else(|| {
                    CodegenError::Internal("__tensor_zero_grad not declared".into())
                })?;
                let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                builder.ins().call(callee, &[t]);
                cx.last_expr_type = Some(clif_types::default_int_type());
                return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
            }
            "grad_tensor_data" => {
                if args.is_empty() {
                    return Err(CodegenError::NotImplemented(
                        "grad_tensor_data requires 1 argument".into(),
                    ));
                }
                let t = compile_expr(builder, cx, &args[0].value)?;
                let fn_id = *cx.functions.get("__grad_tensor_data").ok_or_else(|| {
                    CodegenError::Internal("__grad_tensor_data not declared".into())
                })?;
                let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                let call = builder.ins().call(callee, &[t]);
                let result = builder.inst_results(call)[0];
                cx.last_expr_type = Some(clif_types::pointer_type());
                return Ok(result);
            }
            "grad_tensor_free" => {
                if args.is_empty() {
                    return Err(CodegenError::NotImplemented(
                        "grad_tensor_free requires 1 argument".into(),
                    ));
                }
                let t = compile_expr(builder, cx, &args[0].value)?;
                let fn_id = *cx.functions.get("__grad_tensor_free").ok_or_else(|| {
                    CodegenError::Internal("__grad_tensor_free not declared".into())
                })?;
                let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                builder.ins().call(callee, &[t]);
                cx.last_expr_type = Some(clif_types::default_int_type());
                return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
            }
            // --- S32.3: Gradient through matmul, relu, sigmoid, softmax ---
            "grad_relu" | "grad_sigmoid" | "grad_softmax" => {
                if args.is_empty() {
                    return Err(CodegenError::NotImplemented(format!(
                        "{fn_name} requires 1 argument (grad_tensor)"
                    )));
                }
                let t = compile_expr(builder, cx, &args[0].value)?;
                let key = format!("__{fn_name}");
                let fn_id = *cx
                    .functions
                    .get(&key)
                    .ok_or_else(|| CodegenError::Internal(format!("{key} not declared")))?;
                let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                let call = builder.ins().call(callee, &[t]);
                let result = builder.inst_results(call)[0];
                cx.last_expr_type = Some(clif_types::pointer_type());
                return Ok(result);
            }
            "grad_matmul" => {
                if args.len() < 2 {
                    return Err(CodegenError::NotImplemented(
                        "grad_matmul requires 2 arguments (grad_tensor_a, tensor_b)".into(),
                    ));
                }
                let a = compile_expr(builder, cx, &args[0].value)?;
                let b = compile_expr(builder, cx, &args[1].value)?;
                let fn_id = *cx
                    .functions
                    .get("__grad_matmul")
                    .ok_or_else(|| CodegenError::Internal("__grad_matmul not declared".into()))?;
                let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                let call = builder.ins().call(callee, &[a, b]);
                let result = builder.inst_results(call)[0];
                cx.last_expr_type = Some(clif_types::pointer_type());
                return Ok(result);
            }
            // --- S33: Optimizer builtins ---
            "sgd_new" => {
                if args.is_empty() {
                    return Err(CodegenError::NotImplemented(
                        "sgd_new requires 1 argument (learning rate)".into(),
                    ));
                }
                let lr = compile_expr(builder, cx, &args[0].value)?;
                let fn_id = *cx
                    .functions
                    .get("__sgd_new")
                    .ok_or_else(|| CodegenError::Internal("__sgd_new not declared".into()))?;
                let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                let call = builder.ins().call(callee, &[lr]);
                let result = builder.inst_results(call)[0];
                cx.last_expr_type = Some(clif_types::pointer_type());
                return Ok(result);
            }
            "adam_new" => {
                if args.is_empty() {
                    return Err(CodegenError::NotImplemented(
                        "adam_new requires 1 argument (learning rate)".into(),
                    ));
                }
                let lr = compile_expr(builder, cx, &args[0].value)?;
                let fn_id = *cx
                    .functions
                    .get("__adam_new")
                    .ok_or_else(|| CodegenError::Internal("__adam_new not declared".into()))?;
                let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                let call = builder.ins().call(callee, &[lr]);
                let result = builder.inst_results(call)[0];
                cx.last_expr_type = Some(clif_types::pointer_type());
                return Ok(result);
            }
            "sgd_step" => {
                if args.len() < 2 {
                    return Err(CodegenError::NotImplemented(
                        "sgd_step requires 2 arguments (optimizer, param)".into(),
                    ));
                }
                let opt = compile_expr(builder, cx, &args[0].value)?;
                let param = compile_expr(builder, cx, &args[1].value)?;
                let fn_id = *cx
                    .functions
                    .get("__sgd_step")
                    .ok_or_else(|| CodegenError::Internal("__sgd_step not declared".into()))?;
                let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                builder.ins().call(callee, &[opt, param]);
                cx.last_expr_type = Some(clif_types::default_int_type());
                return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
            }
            "adam_step" => {
                if args.len() < 2 {
                    return Err(CodegenError::NotImplemented(
                        "adam_step requires 2 arguments (optimizer, param)".into(),
                    ));
                }
                let opt = compile_expr(builder, cx, &args[0].value)?;
                let param = compile_expr(builder, cx, &args[1].value)?;
                let fn_id = *cx
                    .functions
                    .get("__adam_step")
                    .ok_or_else(|| CodegenError::Internal("__adam_step not declared".into()))?;
                let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                builder.ins().call(callee, &[opt, param]);
                cx.last_expr_type = Some(clif_types::default_int_type());
                return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
            }
            "optimizer_free" => {
                if args.len() < 2 {
                    return Err(CodegenError::NotImplemented(
                        "optimizer_free requires 2 arguments (ptr, tag)".into(),
                    ));
                }
                let ptr = compile_expr(builder, cx, &args[0].value)?;
                let tag = compile_expr(builder, cx, &args[1].value)?;
                let fn_id = *cx.functions.get("__optimizer_free").ok_or_else(|| {
                    CodegenError::Internal("__optimizer_free not declared".into())
                })?;
                let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                builder.ins().call(callee, &[ptr, tag]);
                cx.last_expr_type = Some(clif_types::default_int_type());
                return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
            }
            // --- S39: Mixed precision builtins ---
            "f32_to_f16" | "f16_to_f32" => {
                if args.is_empty() {
                    return Err(CodegenError::NotImplemented(format!(
                        "{fn_name} requires 1 argument"
                    )));
                }
                let val = compile_expr(builder, cx, &args[0].value)?;
                let key = format!("__{fn_name}");
                let fn_id = *cx
                    .functions
                    .get(&key)
                    .ok_or_else(|| CodegenError::Internal(format!("{key} not declared")))?;
                let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                let call = builder.ins().call(callee, &[val]);
                let result = builder.inst_results(call)[0];
                cx.last_expr_type = Some(clif_types::default_int_type());
                return Ok(result);
            }
            "tensor_to_f16" => {
                if args.is_empty() {
                    return Err(CodegenError::NotImplemented(
                        "tensor_to_f16 requires 1 argument".into(),
                    ));
                }
                let t = compile_expr(builder, cx, &args[0].value)?;
                let fn_id = *cx
                    .functions
                    .get("__tensor_to_f16")
                    .ok_or_else(|| CodegenError::Internal("__tensor_to_f16 not declared".into()))?;
                let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                let call = builder.ins().call(callee, &[t]);
                let result = builder.inst_results(call)[0];
                cx.last_expr_type = Some(clif_types::default_int_type());
                return Ok(result);
            }
            // --- S39.3: Loss scaling ---
            "loss_scale" | "loss_unscale" => {
                if args.len() < 2 {
                    return Err(CodegenError::NotImplemented(format!(
                        "{fn_name} requires 2 arguments (tensor, scale)"
                    )));
                }
                let t = compile_expr(builder, cx, &args[0].value)?;
                let scale = compile_expr(builder, cx, &args[1].value)?;
                // Ensure scale is f64
                let scale_f = if !clif_types::is_float(builder.func.dfg.value_type(scale)) {
                    builder
                        .ins()
                        .fcvt_from_sint(clif_types::default_float_type(), scale)
                } else {
                    scale
                };
                let key = format!("__{fn_name}");
                let fn_id = *cx
                    .functions
                    .get(&key)
                    .ok_or_else(|| CodegenError::Internal(format!("{key} not declared")))?;
                let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                let call = builder.ins().call(callee, &[t, scale_f]);
                let result = builder.inst_results(call)[0];
                cx.last_expr_type = Some(clif_types::default_int_type());
                return Ok(result);
            }
            // --- S39.4: Post-training quantization ---
            "tensor_quantize_int8" => {
                if args.is_empty() {
                    return Err(CodegenError::NotImplemented(
                        "tensor_quantize_int8 requires 1 argument".into(),
                    ));
                }
                let t = compile_expr(builder, cx, &args[0].value)?;
                let fn_id = *cx.functions.get("__tensor_quantize_int8").ok_or_else(|| {
                    CodegenError::Internal("__tensor_quantize_int8 not declared".into())
                })?;
                let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                let call = builder.ins().call(callee, &[t]);
                let result = builder.inst_results(call)[0];
                cx.last_expr_type = Some(clif_types::default_int_type());
                return Ok(result);
            }
            "tensor_quant_scale" | "tensor_quant_zero_point" => {
                let key = format!("__{fn_name}");
                let fn_id = *cx
                    .functions
                    .get(&key)
                    .ok_or_else(|| CodegenError::Internal(format!("{key} not declared")))?;
                let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                let call = builder.ins().call(callee, &[]);
                let result = builder.inst_results(call)[0];
                cx.last_expr_type = Some(clif_types::default_float_type());
                return Ok(result);
            }
            "tensor_dequantize_int8" => {
                if args.len() < 3 {
                    return Err(CodegenError::NotImplemented(
                        "tensor_dequantize_int8 requires 3 arguments (tensor, scale, zero_point)"
                            .into(),
                    ));
                }
                let t = compile_expr(builder, cx, &args[0].value)?;
                let scale = compile_expr(builder, cx, &args[1].value)?;
                let zp = compile_expr(builder, cx, &args[2].value)?;
                // Ensure scale and zp are f64
                let scale_f = if !clif_types::is_float(builder.func.dfg.value_type(scale)) {
                    builder
                        .ins()
                        .fcvt_from_sint(clif_types::default_float_type(), scale)
                } else {
                    scale
                };
                let zp_f = if !clif_types::is_float(builder.func.dfg.value_type(zp)) {
                    builder
                        .ins()
                        .fcvt_from_sint(clif_types::default_float_type(), zp)
                } else {
                    zp
                };
                let fn_id = *cx
                    .functions
                    .get("__tensor_dequantize_int8")
                    .ok_or_else(|| {
                        CodegenError::Internal("__tensor_dequantize_int8 not declared".into())
                    })?;
                let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                let call = builder.ins().call(callee, &[t, scale_f, zp_f]);
                let result = builder.inst_results(call)[0];
                cx.last_expr_type = Some(clif_types::default_int_type());
                return Ok(result);
            }
            // --- S34: Distributed training builtins ---
            "dist_init" => {
                if args.len() < 2 {
                    return Err(CodegenError::NotImplemented(
                        "dist_init requires 2 arguments (world_size, rank)".into(),
                    ));
                }
                let ws = compile_expr(builder, cx, &args[0].value)?;
                let rank = compile_expr(builder, cx, &args[1].value)?;
                let fn_id = *cx
                    .functions
                    .get("__dist_init")
                    .ok_or_else(|| CodegenError::Internal("__dist_init not declared".into()))?;
                let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                let call = builder.ins().call(callee, &[ws, rank]);
                let result = builder.inst_results(call)[0];
                cx.last_expr_type = Some(clif_types::default_int_type());
                return Ok(result);
            }
            "dist_world_size" | "dist_rank" => {
                if args.is_empty() {
                    return Err(CodegenError::NotImplemented(format!(
                        "{fn_name} requires 1 argument (ctx)"
                    )));
                }
                let ctx = compile_expr(builder, cx, &args[0].value)?;
                let key = format!("__{fn_name}");
                let fn_id = *cx
                    .functions
                    .get(&key)
                    .ok_or_else(|| CodegenError::Internal(format!("{key} not declared")))?;
                let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                let call = builder.ins().call(callee, &[ctx]);
                let result = builder.inst_results(call)[0];
                cx.last_expr_type = Some(clif_types::default_int_type());
                return Ok(result);
            }
            "dist_all_reduce_sum" => {
                if args.len() < 2 {
                    return Err(CodegenError::NotImplemented(
                        "dist_all_reduce_sum requires 2 arguments (ctx, tensor)".into(),
                    ));
                }
                let ctx = compile_expr(builder, cx, &args[0].value)?;
                let t = compile_expr(builder, cx, &args[1].value)?;
                let fn_id = *cx.functions.get("__dist_all_reduce_sum").ok_or_else(|| {
                    CodegenError::Internal("__dist_all_reduce_sum not declared".into())
                })?;
                let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                let call = builder.ins().call(callee, &[ctx, t]);
                let result = builder.inst_results(call)[0];
                cx.last_expr_type = Some(clif_types::default_int_type());
                return Ok(result);
            }
            "dist_broadcast" => {
                if args.len() < 3 {
                    return Err(CodegenError::NotImplemented(
                        "dist_broadcast requires 3 arguments (ctx, tensor, root)".into(),
                    ));
                }
                let ctx = compile_expr(builder, cx, &args[0].value)?;
                let t = compile_expr(builder, cx, &args[1].value)?;
                let root = compile_expr(builder, cx, &args[2].value)?;
                let fn_id = *cx.functions.get("__dist_broadcast").ok_or_else(|| {
                    CodegenError::Internal("__dist_broadcast not declared".into())
                })?;
                let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                let call = builder.ins().call(callee, &[ctx, t, root]);
                let result = builder.inst_results(call)[0];
                cx.last_expr_type = Some(clif_types::default_int_type());
                return Ok(result);
            }
            "dist_split_batch" => {
                if args.len() < 2 {
                    return Err(CodegenError::NotImplemented(
                        "dist_split_batch requires 2 arguments (ctx, tensor)".into(),
                    ));
                }
                let ctx = compile_expr(builder, cx, &args[0].value)?;
                let t = compile_expr(builder, cx, &args[1].value)?;
                let fn_id = *cx.functions.get("__dist_split_batch").ok_or_else(|| {
                    CodegenError::Internal("__dist_split_batch not declared".into())
                })?;
                let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                let call = builder.ins().call(callee, &[ctx, t]);
                let result = builder.inst_results(call)[0];
                cx.last_expr_type = Some(clif_types::default_int_type());
                return Ok(result);
            }
            "dist_free" => {
                if args.is_empty() {
                    return Err(CodegenError::NotImplemented(
                        "dist_free requires 1 argument (ctx)".into(),
                    ));
                }
                let ctx = compile_expr(builder, cx, &args[0].value)?;
                let fn_id = *cx
                    .functions
                    .get("__dist_free")
                    .ok_or_else(|| CodegenError::Internal("__dist_free not declared".into()))?;
                let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                builder.ins().call(callee, &[ctx]);
                cx.last_expr_type = Some(clif_types::default_int_type());
                return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
            }
            // --- S34.4: TCP gradient exchange ---
            "dist_tcp_bind" => {
                if args.is_empty() {
                    return Err(CodegenError::NotImplemented(
                        "dist_tcp_bind requires 1 argument (port)".into(),
                    ));
                }
                let port = compile_expr(builder, cx, &args[0].value)?;
                let fn_id = *cx
                    .functions
                    .get("__dist_tcp_bind")
                    .ok_or_else(|| CodegenError::Internal("__dist_tcp_bind not declared".into()))?;
                let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                let call = builder.ins().call(callee, &[port]);
                let result = builder.inst_results(call)[0];
                cx.last_expr_type = Some(clif_types::default_int_type());
                return Ok(result);
            }
            "dist_tcp_port" => {
                if args.is_empty() {
                    return Err(CodegenError::NotImplemented(
                        "dist_tcp_port requires 1 argument (handle)".into(),
                    ));
                }
                let handle = compile_expr(builder, cx, &args[0].value)?;
                let fn_id = *cx
                    .functions
                    .get("__dist_tcp_port")
                    .ok_or_else(|| CodegenError::Internal("__dist_tcp_port not declared".into()))?;
                let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                let call = builder.ins().call(callee, &[handle]);
                let result = builder.inst_results(call)[0];
                cx.last_expr_type = Some(clif_types::default_int_type());
                return Ok(result);
            }
            "dist_tcp_send" => {
                if args.len() < 2 {
                    return Err(CodegenError::NotImplemented(
                        "dist_tcp_send requires 2 arguments (port, tensor)".into(),
                    ));
                }
                let port = compile_expr(builder, cx, &args[0].value)?;
                let tensor = compile_expr(builder, cx, &args[1].value)?;
                let fn_id = *cx
                    .functions
                    .get("__dist_tcp_send")
                    .ok_or_else(|| CodegenError::Internal("__dist_tcp_send not declared".into()))?;
                let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                let call = builder.ins().call(callee, &[port, tensor]);
                let result = builder.inst_results(call)[0];
                cx.last_expr_type = Some(clif_types::default_int_type());
                return Ok(result);
            }
            "dist_tcp_recv" => {
                if args.is_empty() {
                    return Err(CodegenError::NotImplemented(
                        "dist_tcp_recv requires 1 argument (handle)".into(),
                    ));
                }
                let handle = compile_expr(builder, cx, &args[0].value)?;
                let fn_id = *cx
                    .functions
                    .get("__dist_tcp_recv")
                    .ok_or_else(|| CodegenError::Internal("__dist_tcp_recv not declared".into()))?;
                let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                let call = builder.ins().call(callee, &[handle]);
                let result = builder.inst_results(call)[0];
                cx.last_expr_type = Some(clif_types::default_int_type());
                return Ok(result);
            }
            "dist_tcp_free" => {
                if args.is_empty() {
                    return Err(CodegenError::NotImplemented(
                        "dist_tcp_free requires 1 argument (handle)".into(),
                    ));
                }
                let handle = compile_expr(builder, cx, &args[0].value)?;
                let fn_id = *cx
                    .functions
                    .get("__dist_tcp_free")
                    .ok_or_else(|| CodegenError::Internal("__dist_tcp_free not declared".into()))?;
                let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                builder.ins().call(callee, &[handle]);
                cx.last_expr_type = Some(clif_types::default_int_type());
                return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
            }
            // --- S36: Data Pipeline builtins ---
            "dataloader_new" => {
                if args.len() < 3 {
                    return Err(CodegenError::NotImplemented(
                        "dataloader_new requires 3 arguments (data, labels, batch_size)".into(),
                    ));
                }
                let data = compile_expr(builder, cx, &args[0].value)?;
                let labels = compile_expr(builder, cx, &args[1].value)?;
                let batch = compile_expr(builder, cx, &args[2].value)?;
                let fn_id = *cx.functions.get("__dataloader_new").ok_or_else(|| {
                    CodegenError::Internal("__dataloader_new not declared".into())
                })?;
                let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                let call = builder.ins().call(callee, &[data, labels, batch]);
                let result = builder.inst_results(call)[0];
                cx.last_expr_type = Some(clif_types::pointer_type());
                return Ok(result);
            }
            "dataloader_len" => {
                if args.is_empty() {
                    return Err(CodegenError::NotImplemented(
                        "dataloader_len requires 1 argument".into(),
                    ));
                }
                let dl = compile_expr(builder, cx, &args[0].value)?;
                let fn_id = *cx.functions.get("__dataloader_len").ok_or_else(|| {
                    CodegenError::Internal("__dataloader_len not declared".into())
                })?;
                let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                let call = builder.ins().call(callee, &[dl]);
                let result = builder.inst_results(call)[0];
                cx.last_expr_type = Some(clif_types::default_int_type());
                return Ok(result);
            }
            "dataloader_reset" => {
                if args.len() < 2 {
                    return Err(CodegenError::NotImplemented(
                        "dataloader_reset requires 2 arguments (dl, shuffle)".into(),
                    ));
                }
                let dl = compile_expr(builder, cx, &args[0].value)?;
                let shuffle = compile_expr(builder, cx, &args[1].value)?;
                let fn_id = *cx.functions.get("__dataloader_reset").ok_or_else(|| {
                    CodegenError::Internal("__dataloader_reset not declared".into())
                })?;
                let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                builder.ins().call(callee, &[dl, shuffle]);
                cx.last_expr_type = Some(clif_types::default_int_type());
                return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
            }
            "dataloader_next_data" => {
                if args.is_empty() {
                    return Err(CodegenError::NotImplemented(
                        "dataloader_next_data requires 1 argument".into(),
                    ));
                }
                let dl = compile_expr(builder, cx, &args[0].value)?;
                let fn_id = *cx.functions.get("__dataloader_next_data").ok_or_else(|| {
                    CodegenError::Internal("__dataloader_next_data not declared".into())
                })?;
                let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                let call = builder.ins().call(callee, &[dl]);
                let result = builder.inst_results(call)[0];
                cx.last_expr_type = Some(clif_types::pointer_type());
                return Ok(result);
            }
            "dataloader_next_labels" => {
                if args.is_empty() {
                    return Err(CodegenError::NotImplemented(
                        "dataloader_next_labels requires 1 argument".into(),
                    ));
                }
                let dl = compile_expr(builder, cx, &args[0].value)?;
                let fn_id = *cx
                    .functions
                    .get("__dataloader_next_labels")
                    .ok_or_else(|| {
                        CodegenError::Internal("__dataloader_next_labels not declared".into())
                    })?;
                let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                let call = builder.ins().call(callee, &[dl]);
                let result = builder.inst_results(call)[0];
                cx.last_expr_type = Some(clif_types::pointer_type());
                return Ok(result);
            }
            "dataloader_num_samples" => {
                if args.is_empty() {
                    return Err(CodegenError::NotImplemented(
                        "dataloader_num_samples requires 1 argument".into(),
                    ));
                }
                let dl = compile_expr(builder, cx, &args[0].value)?;
                let fn_id = *cx
                    .functions
                    .get("__dataloader_num_samples")
                    .ok_or_else(|| {
                        CodegenError::Internal("__dataloader_num_samples not declared".into())
                    })?;
                let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                let call = builder.ins().call(callee, &[dl]);
                let result = builder.inst_results(call)[0];
                cx.last_expr_type = Some(clif_types::default_int_type());
                return Ok(result);
            }
            "dataloader_free" => {
                if args.is_empty() {
                    return Err(CodegenError::NotImplemented(
                        "dataloader_free requires 1 argument".into(),
                    ));
                }
                let dl = compile_expr(builder, cx, &args[0].value)?;
                let fn_id = *cx.functions.get("__dataloader_free").ok_or_else(|| {
                    CodegenError::Internal("__dataloader_free not declared".into())
                })?;
                let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                builder.ins().call(callee, &[dl]);
                cx.last_expr_type = Some(clif_types::default_int_type());
                return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
            }
            "tensor_normalize" => {
                if args.is_empty() {
                    return Err(CodegenError::NotImplemented(
                        "tensor_normalize requires 1 argument".into(),
                    ));
                }
                let t = compile_expr(builder, cx, &args[0].value)?;
                let fn_id = *cx.functions.get("__tensor_normalize").ok_or_else(|| {
                    CodegenError::Internal("__tensor_normalize not declared".into())
                })?;
                let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                let call = builder.ins().call(callee, &[t]);
                let result = builder.inst_results(call)[0];
                cx.last_expr_type = Some(clif_types::pointer_type());
                return Ok(result);
            }
            // --- S37: Model Serialization builtins ---
            "tensor_save" => {
                if args.len() < 2 {
                    return Err(CodegenError::NotImplemented(
                        "tensor_save requires 2 arguments (tensor, path)".into(),
                    ));
                }
                let tensor = compile_expr(builder, cx, &args[0].value)?;
                let path = compile_expr(builder, cx, &args[1].value)?;
                let path_len = cx
                    .last_string_len
                    .take()
                    .unwrap_or_else(|| builder.ins().iconst(clif_types::default_int_type(), 0));
                let fn_id = *cx
                    .functions
                    .get("__tensor_save")
                    .ok_or_else(|| CodegenError::Internal("__tensor_save not declared".into()))?;
                let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                let call = builder.ins().call(callee, &[tensor, path, path_len]);
                let result = builder.inst_results(call)[0];
                cx.last_expr_type = Some(clif_types::default_int_type());
                return Ok(result);
            }
            "tensor_load" => {
                if args.is_empty() {
                    return Err(CodegenError::NotImplemented(
                        "tensor_load requires 1 argument (path)".into(),
                    ));
                }
                let path = compile_expr(builder, cx, &args[0].value)?;
                let path_len = cx
                    .last_string_len
                    .take()
                    .unwrap_or_else(|| builder.ins().iconst(clif_types::default_int_type(), 0));
                let fn_id = *cx
                    .functions
                    .get("__tensor_load")
                    .ok_or_else(|| CodegenError::Internal("__tensor_load not declared".into()))?;
                let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                let call = builder.ins().call(callee, &[path, path_len]);
                let result = builder.inst_results(call)[0];
                cx.last_expr_type = Some(clif_types::pointer_type());
                return Ok(result);
            }
            "checkpoint_save" => {
                if args.len() < 4 {
                    return Err(CodegenError::NotImplemented(
                        "checkpoint_save requires 4 args (tensor, path, epoch, loss)".into(),
                    ));
                }
                let tensor = compile_expr(builder, cx, &args[0].value)?;
                let path = compile_expr(builder, cx, &args[1].value)?;
                let path_len = cx
                    .last_string_len
                    .take()
                    .unwrap_or_else(|| builder.ins().iconst(clif_types::default_int_type(), 0));
                let epoch = compile_expr(builder, cx, &args[2].value)?;
                let loss = compile_expr(builder, cx, &args[3].value)?;
                let fn_id = *cx.functions.get("__checkpoint_save").ok_or_else(|| {
                    CodegenError::Internal("__checkpoint_save not declared".into())
                })?;
                let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                let call = builder
                    .ins()
                    .call(callee, &[tensor, path, path_len, epoch, loss]);
                let result = builder.inst_results(call)[0];
                cx.last_expr_type = Some(clif_types::default_int_type());
                return Ok(result);
            }
            "checkpoint_load" => {
                if args.is_empty() {
                    return Err(CodegenError::NotImplemented(
                        "checkpoint_load requires 1 argument (path)".into(),
                    ));
                }
                let path = compile_expr(builder, cx, &args[0].value)?;
                let path_len = cx
                    .last_string_len
                    .take()
                    .unwrap_or_else(|| builder.ins().iconst(clif_types::default_int_type(), 0));
                let fn_id = *cx.functions.get("__checkpoint_load").ok_or_else(|| {
                    CodegenError::Internal("__checkpoint_load not declared".into())
                })?;
                let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                let call = builder.ins().call(callee, &[path, path_len]);
                let result = builder.inst_results(call)[0];
                cx.last_expr_type = Some(clif_types::pointer_type());
                return Ok(result);
            }
            "checkpoint_epoch" => {
                if args.is_empty() {
                    return Err(CodegenError::NotImplemented(
                        "checkpoint_epoch requires 1 argument (path)".into(),
                    ));
                }
                let path = compile_expr(builder, cx, &args[0].value)?;
                let path_len = cx
                    .last_string_len
                    .take()
                    .unwrap_or_else(|| builder.ins().iconst(clif_types::default_int_type(), 0));
                let fn_id = *cx.functions.get("__checkpoint_epoch").ok_or_else(|| {
                    CodegenError::Internal("__checkpoint_epoch not declared".into())
                })?;
                let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                let call = builder.ins().call(callee, &[path, path_len]);
                let result = builder.inst_results(call)[0];
                cx.last_expr_type = Some(clif_types::default_int_type());
                return Ok(result);
            }
            "checkpoint_loss" => {
                if args.is_empty() {
                    return Err(CodegenError::NotImplemented(
                        "checkpoint_loss requires 1 argument (path)".into(),
                    ));
                }
                let path = compile_expr(builder, cx, &args[0].value)?;
                let path_len = cx
                    .last_string_len
                    .take()
                    .unwrap_or_else(|| builder.ins().iconst(clif_types::default_int_type(), 0));
                let fn_id = *cx.functions.get("__checkpoint_loss").ok_or_else(|| {
                    CodegenError::Internal("__checkpoint_loss not declared".into())
                })?;
                let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                let call = builder.ins().call(callee, &[path, path_len]);
                let result = builder.inst_results(call)[0];
                cx.last_expr_type = Some(clif_types::default_int_type());
                return Ok(result);
            }
            // --- Additional tensor & utility builtins ---
            "tensor_mean" => {
                if args.is_empty() {
                    return Err(CodegenError::NotImplemented(
                        "tensor_mean requires 1 arg".into(),
                    ));
                }
                let t = compile_expr(builder, cx, &args[0].value)?;
                let fn_id = *cx
                    .functions
                    .get("__tensor_mean")
                    .ok_or_else(|| CodegenError::Internal("__tensor_mean not declared".into()))?;
                let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                let call = builder.ins().call(callee, &[t]);
                let result = builder.inst_results(call)[0];
                cx.last_expr_type = Some(clif_types::default_int_type());
                return Ok(result);
            }
            "tensor_row" | "row" => {
                if args.len() < 2 {
                    return Err(CodegenError::NotImplemented(
                        "tensor_row requires 2 args (tensor, row_idx)".into(),
                    ));
                }
                let t = compile_expr(builder, cx, &args[0].value)?;
                let row = compile_expr(builder, cx, &args[1].value)?;
                let fn_id = *cx
                    .functions
                    .get("__tensor_row")
                    .ok_or_else(|| CodegenError::Internal("__tensor_row not declared".into()))?;
                let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                let call = builder.ins().call(callee, &[t, row]);
                let result = builder.inst_results(call)[0];
                cx.last_expr_type = Some(clif_types::pointer_type());
                return Ok(result);
            }
            "tensor_abs" => {
                if args.is_empty() {
                    return Err(CodegenError::NotImplemented(
                        "tensor_abs requires 1 arg".into(),
                    ));
                }
                let t = compile_expr(builder, cx, &args[0].value)?;
                let fn_id = *cx
                    .functions
                    .get("__tensor_abs")
                    .ok_or_else(|| CodegenError::Internal("__tensor_abs not declared".into()))?;
                let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                let call = builder.ins().call(callee, &[t]);
                let result = builder.inst_results(call)[0];
                cx.last_expr_type = Some(clif_types::pointer_type());
                return Ok(result);
            }
            "tensor_fill" => {
                if args.len() < 3 {
                    return Err(CodegenError::NotImplemented(
                        "tensor_fill requires 3 args (rows, cols, val_bits)".into(),
                    ));
                }
                let rows = compile_expr(builder, cx, &args[0].value)?;
                let cols = compile_expr(builder, cx, &args[1].value)?;
                let val = compile_expr(builder, cx, &args[2].value)?;
                let fn_id = *cx
                    .functions
                    .get("__tensor_fill")
                    .ok_or_else(|| CodegenError::Internal("__tensor_fill not declared".into()))?;
                let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                let call = builder.ins().call(callee, &[rows, cols, val]);
                let result = builder.inst_results(call)[0];
                cx.last_expr_type = Some(clif_types::pointer_type());
                return Ok(result);
            }
            "tensor_rand" | "randn" => {
                if args.len() < 2 {
                    return Err(CodegenError::NotImplemented(
                        "tensor_rand requires 2 args (rows, cols)".into(),
                    ));
                }
                let rows = compile_expr(builder, cx, &args[0].value)?;
                let cols = compile_expr(builder, cx, &args[1].value)?;
                let fn_id = *cx
                    .functions
                    .get("__tensor_rand")
                    .ok_or_else(|| CodegenError::Internal("__tensor_rand not declared".into()))?;
                let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                let call = builder.ins().call(callee, &[rows, cols]);
                let result = builder.inst_results(call)[0];
                cx.last_expr_type = Some(clif_types::pointer_type());
                return Ok(result);
            }
            "tensor_xavier" | "xavier" => {
                if args.len() < 2 {
                    return Err(CodegenError::NotImplemented(
                        "tensor_xavier requires 2 args (rows, cols)".into(),
                    ));
                }
                let rows = compile_expr(builder, cx, &args[0].value)?;
                let cols = compile_expr(builder, cx, &args[1].value)?;
                let fn_id = *cx
                    .functions
                    .get("__tensor_xavier")
                    .ok_or_else(|| CodegenError::Internal("__tensor_xavier not declared".into()))?;
                let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                let call = builder.ins().call(callee, &[rows, cols]);
                let result = builder.inst_results(call)[0];
                cx.last_expr_type = Some(clif_types::pointer_type());
                return Ok(result);
            }
            "tensor_argmax" | "argmax" => {
                if args.is_empty() {
                    return Err(CodegenError::NotImplemented(
                        "tensor_argmax requires 1 arg (tensor)".into(),
                    ));
                }
                let t = compile_expr(builder, cx, &args[0].value)?;
                let fn_id = *cx
                    .functions
                    .get("__tensor_argmax")
                    .ok_or_else(|| CodegenError::Internal("__tensor_argmax not declared".into()))?;
                let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                let call = builder.ins().call(callee, &[t]);
                let result = builder.inst_results(call)[0];
                cx.last_expr_type = Some(clif_types::default_int_type());
                return Ok(result);
            }
            "tensor_from_data" => {
                if args.len() < 4 {
                    return Err(CodegenError::NotImplemented(
                        "tensor_from_data requires 4 args (data_ptr, n_elems, rows, cols)".into(),
                    ));
                }
                let data_ptr = compile_expr(builder, cx, &args[0].value)?;
                let n_elems = compile_expr(builder, cx, &args[1].value)?;
                let rows = compile_expr(builder, cx, &args[2].value)?;
                let cols = compile_expr(builder, cx, &args[3].value)?;
                let fn_id = *cx.functions.get("__tensor_from_data").ok_or_else(|| {
                    CodegenError::Internal("__tensor_from_data not declared".into())
                })?;
                let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                let call = builder.ins().call(callee, &[data_ptr, n_elems, rows, cols]);
                let result = builder.inst_results(call)[0];
                cx.last_expr_type = Some(clif_types::pointer_type());
                return Ok(result);
            }
            "tensor_scale" => {
                if args.len() < 2 {
                    return Err(CodegenError::NotImplemented(
                        "tensor_scale requires 2 args (tensor, scalar_bits)".into(),
                    ));
                }
                let t = compile_expr(builder, cx, &args[0].value)?;
                let scalar = compile_expr(builder, cx, &args[1].value)?;
                let fn_id = *cx
                    .functions
                    .get("__tensor_scale")
                    .ok_or_else(|| CodegenError::Internal("__tensor_scale not declared".into()))?;
                let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                let call = builder.ins().call(callee, &[t, scalar]);
                let result = builder.inst_results(call)[0];
                cx.last_expr_type = Some(clif_types::pointer_type());
                return Ok(result);
            }
            "random_int" => {
                if args.is_empty() {
                    return Err(CodegenError::NotImplemented(
                        "random_int requires 1 arg (max)".into(),
                    ));
                }
                let max = compile_expr(builder, cx, &args[0].value)?;
                let fn_id = *cx
                    .functions
                    .get("__random_int")
                    .ok_or_else(|| CodegenError::Internal("__random_int not declared".into()))?;
                let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                let call = builder.ins().call(callee, &[max]);
                let result = builder.inst_results(call)[0];
                cx.last_expr_type = Some(clif_types::default_int_type());
                return Ok(result);
            }
            "map_new" => {
                let fn_id = *cx
                    .functions
                    .get("__map_new")
                    .ok_or_else(|| CodegenError::Internal("__map_new not declared".into()))?;
                let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                let call = builder.ins().call(callee, &[]);
                let result = builder.inst_results(call)[0];
                cx.last_expr_type = Some(clif_types::pointer_type());
                cx.last_map_new = true;
                return Ok(result);
            }
            "map_insert" => {
                // map_insert(map, key, value) → fj_rt_map_insert_int(map, key_ptr, key_len, value)
                if args.len() < 3 {
                    return Err(CodegenError::NotImplemented(
                        "map_insert requires 3 args (map, key, value)".into(),
                    ));
                }
                let map_ptr = compile_expr(builder, cx, &args[0].value)?;
                let key_val = compile_expr(builder, cx, &args[1].value)?;
                let key_len = cx.last_string_len.take().ok_or_else(|| {
                    CodegenError::NotImplemented("map_insert key must be a string".into())
                })?;
                let val = compile_expr(builder, cx, &args[2].value)?;
                let func_id = *cx.functions.get("__map_insert_int").ok_or_else(|| {
                    CodegenError::Internal("__map_insert_int not declared".into())
                })?;
                let local = cx.module.declare_func_in_func(func_id, builder.func);
                builder.ins().call(local, &[map_ptr, key_val, key_len, val]);
                cx.last_expr_type = Some(clif_types::pointer_type());
                return Ok(map_ptr);
            }
            "map_get" => {
                // map_get(map, key) → fj_rt_map_get_int(map, key_ptr, key_len)
                if args.len() < 2 {
                    return Err(CodegenError::NotImplemented(
                        "map_get requires 2 args (map, key)".into(),
                    ));
                }
                let map_ptr = compile_expr(builder, cx, &args[0].value)?;
                let key_val = compile_expr(builder, cx, &args[1].value)?;
                let key_len = cx.last_string_len.take().ok_or_else(|| {
                    CodegenError::NotImplemented("map_get key must be a string".into())
                })?;
                let func_id = *cx
                    .functions
                    .get("__map_get_int")
                    .ok_or_else(|| CodegenError::Internal("__map_get_int not declared".into()))?;
                let local = cx.module.declare_func_in_func(func_id, builder.func);
                let call = builder.ins().call(local, &[map_ptr, key_val, key_len]);
                let result = builder.inst_results(call)[0];
                cx.last_expr_type = Some(clif_types::default_int_type());
                return Ok(result);
            }
            "map_len" => {
                // map_len(map) → fj_rt_map_len(map)
                if args.is_empty() {
                    return Err(CodegenError::NotImplemented(
                        "map_len requires 1 arg (map)".into(),
                    ));
                }
                let map_ptr = compile_expr(builder, cx, &args[0].value)?;
                let func_id = *cx
                    .functions
                    .get("__map_len")
                    .ok_or_else(|| CodegenError::Internal("__map_len not declared".into()))?;
                let local = cx.module.declare_func_in_func(func_id, builder.func);
                let call = builder.ins().call(local, &[map_ptr]);
                let result = builder.inst_results(call)[0];
                cx.last_expr_type = Some(clif_types::default_int_type());
                return Ok(result);
            }
            "map_keys" => {
                // map_keys(map) → fj_rt_map_keys(map, count_out)
                if args.is_empty() {
                    return Err(CodegenError::NotImplemented(
                        "map_keys requires 1 arg (map)".into(),
                    ));
                }
                let map_ptr = compile_expr(builder, cx, &args[0].value)?;
                let count_slot =
                    builder.create_sized_stack_slot(cranelift_codegen::ir::StackSlotData::new(
                        cranelift_codegen::ir::StackSlotKind::ExplicitSlot,
                        8,
                        3,
                    ));
                let count_addr =
                    builder
                        .ins()
                        .stack_addr(clif_types::default_int_type(), count_slot, 0);
                let func_id = *cx
                    .functions
                    .get("__map_keys")
                    .ok_or_else(|| CodegenError::Internal("__map_keys not declared".into()))?;
                let local = cx.module.declare_func_in_func(func_id, builder.func);
                let call = builder.ins().call(local, &[map_ptr, count_addr]);
                let result = builder.inst_results(call)[0];
                cx.last_expr_type = Some(clif_types::pointer_type());
                cx.last_split_result = Some(result);
                return Ok(result);
            }
            "map_contains" => {
                // map_contains(map, key) → fj_rt_map_contains(map, key_ptr, key_len)
                if args.len() < 2 {
                    return Err(CodegenError::NotImplemented(
                        "map_contains requires 2 args (map, key)".into(),
                    ));
                }
                let map_ptr = compile_expr(builder, cx, &args[0].value)?;
                let key_val = compile_expr(builder, cx, &args[1].value)?;
                let key_len = cx.last_string_len.take().ok_or_else(|| {
                    CodegenError::NotImplemented("map_contains key must be a string".into())
                })?;
                let func_id = *cx
                    .functions
                    .get("__map_contains")
                    .ok_or_else(|| CodegenError::Internal("__map_contains not declared".into()))?;
                let local = cx.module.declare_func_in_func(func_id, builder.func);
                let call = builder.ins().call(local, &[map_ptr, key_val, key_len]);
                let result = builder.inst_results(call)[0];
                cx.last_expr_type = Some(clif_types::default_int_type());
                return Ok(result);
            }
            "map_remove" => {
                // map_remove(map, key) → fj_rt_map_remove(map, key_ptr, key_len)
                if args.len() < 2 {
                    return Err(CodegenError::NotImplemented(
                        "map_remove requires 2 args (map, key)".into(),
                    ));
                }
                let map_ptr = compile_expr(builder, cx, &args[0].value)?;
                let key_val = compile_expr(builder, cx, &args[1].value)?;
                let key_len = cx.last_string_len.take().ok_or_else(|| {
                    CodegenError::NotImplemented("map_remove key must be a string".into())
                })?;
                let func_id = *cx
                    .functions
                    .get("__map_remove")
                    .ok_or_else(|| CodegenError::Internal("__map_remove not declared".into()))?;
                let local = cx.module.declare_func_in_func(func_id, builder.func);
                builder.ins().call(local, &[map_ptr, key_val, key_len]);
                cx.last_expr_type = Some(clif_types::pointer_type());
                return Ok(map_ptr);
            }
            "is_some" => {
                // is_some(val): Some has tag=1, so check tag != 0
                if args.is_empty() {
                    return Err(CodegenError::NotImplemented(
                        "is_some requires 1 argument".into(),
                    ));
                }
                let tag = compile_expr(builder, cx, &args[0].value)?;
                let result = builder.ins().icmp_imm(IntCC::NotEqual, tag, 0);
                let widened = builder
                    .ins()
                    .uextend(clif_types::default_int_type(), result);
                cx.last_expr_type = Some(clif_types::default_int_type());
                return Ok(widened);
            }
            "is_none" => {
                // is_none(val): None has tag=0, so check tag == 0
                if args.is_empty() {
                    return Err(CodegenError::NotImplemented(
                        "is_none requires 1 argument".into(),
                    ));
                }
                let tag = compile_expr(builder, cx, &args[0].value)?;
                let result = builder.ins().icmp_imm(IntCC::Equal, tag, 0);
                let widened = builder
                    .ins()
                    .uextend(clif_types::default_int_type(), result);
                cx.last_expr_type = Some(clif_types::default_int_type());
                return Ok(widened);
            }
            "is_ok" => {
                // is_ok(val): Ok has tag=0, so check tag == 0
                if args.is_empty() {
                    return Err(CodegenError::NotImplemented(
                        "is_ok requires 1 argument".into(),
                    ));
                }
                let tag = compile_expr(builder, cx, &args[0].value)?;
                let result = builder.ins().icmp_imm(IntCC::Equal, tag, 0);
                let widened = builder
                    .ins()
                    .uextend(clif_types::default_int_type(), result);
                cx.last_expr_type = Some(clif_types::default_int_type());
                return Ok(widened);
            }
            "is_err" => {
                // is_err(val): Err has tag=1, so check tag != 0
                if args.is_empty() {
                    return Err(CodegenError::NotImplemented(
                        "is_err requires 1 argument".into(),
                    ));
                }
                let tag = compile_expr(builder, cx, &args[0].value)?;
                let result = builder.ins().icmp_imm(IntCC::NotEqual, tag, 0);
                let widened = builder
                    .ins()
                    .uextend(clif_types::default_int_type(), result);
                cx.last_expr_type = Some(clif_types::default_int_type());
                return Ok(widened);
            }
            "unwrap" => {
                // unwrap(val): get payload; trap if None(tag=0) or Err(tag!=0)
                // Convention: unwrap for Option checks tag!=0 (Some), for Result checks tag==0 (Ok)
                // Since we can't distinguish, use Option convention: payload is in Some(tag=1)
                // For Result, use unwrap_ok() (future)
                // MVP: trap if tag == 0 (None), return payload of Some(tag=1)
                if args.is_empty() {
                    return Err(CodegenError::NotImplemented(
                        "unwrap requires 1 argument".into(),
                    ));
                }
                let tag = compile_expr(builder, cx, &args[0].value)?;
                let payload = cx
                    .last_enum_payload
                    .take()
                    .unwrap_or_else(|| builder.ins().iconst(clif_types::default_int_type(), 0));
                let payload_type = cx
                    .last_enum_payload_type
                    .take()
                    .unwrap_or(clif_types::default_int_type());
                // Trap if tag == 0 (None)
                let is_none = builder.ins().icmp_imm(IntCC::Equal, tag, 0);
                builder.ins().trapnz(
                    is_none,
                    cranelift_codegen::ir::TrapCode::user(1).expect("valid trap"),
                );
                cx.last_expr_type = Some(payload_type);
                return Ok(payload);
            }
            "unwrap_or" => {
                // unwrap_or(val, default): use Option convention
                // If tag != 0 (Some), return payload; else return default
                if args.len() < 2 {
                    return Err(CodegenError::NotImplemented(
                        "unwrap_or requires 2 arguments".into(),
                    ));
                }
                let tag = compile_expr(builder, cx, &args[0].value)?;
                let payload = cx
                    .last_enum_payload
                    .take()
                    .unwrap_or_else(|| builder.ins().iconst(clif_types::default_int_type(), 0));
                let payload_type = cx
                    .last_enum_payload_type
                    .take()
                    .unwrap_or(clif_types::default_int_type());
                let default_val = compile_expr(builder, cx, &args[1].value)?;
                // Select: if tag != 0 (Some), use payload; else use default
                let is_some = builder.ins().icmp_imm(IntCC::NotEqual, tag, 0);
                let result = builder.ins().select(is_some, payload, default_val);
                cx.last_expr_type = Some(payload_type);
                return Ok(result);
            }
            _ => {}
        }
    } // end if !is_user_fn

    // ── Closure call ──────────────────────────────────────────────────
    if let Some(closure_fn_name) = cx.closure_fn_map.get(&fn_name).cloned() {
        let captures = cx
            .closure_captures
            .get(&closure_fn_name)
            .cloned()
            .unwrap_or_default();
        let func_id = *cx
            .functions
            .get(&closure_fn_name)
            .ok_or_else(|| CodegenError::UndefinedFunction(closure_fn_name.clone()))?;
        let local_callee = cx.module.declare_func_in_func(func_id, builder.func);

        // Build args: captured vars first, then explicit args
        let mut call_args = Vec::new();
        for cap_name in &captures {
            let cap_var = *cx
                .var_map
                .get(cap_name)
                .ok_or_else(|| CodegenError::UndefinedVariable(cap_name.clone()))?;
            call_args.push(builder.use_var(cap_var));
        }
        for a in args {
            call_args.push(compile_expr(builder, cx, &a.value)?);
        }

        let call = builder.ins().call(local_callee, &call_args);
        let results = builder.inst_results(call);
        if let Some(&ret_ty) = cx.fn_return_types.get(&closure_fn_name) {
            cx.last_expr_type = Some(ret_ty);
        }
        if results.is_empty() {
            return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
        }
        // Handle string return (dual values)
        if cx.fn_returns_string.contains(&closure_fn_name) && results.len() >= 2 {
            cx.last_string_len = Some(results[1]);
            cx.last_expr_type = Some(clif_types::pointer_type());
        }
        return Ok(results[0]);
    }

    // ── Generic function dispatch ─────────────────────────────────────
    if cx.mono_map.contains_key(&fn_name) {
        return compile_generic_call(builder, cx, &fn_name, args);
    }

    // ── Closure handle call (returned closures with captures) ─────────
    if cx.closure_handle_vars.contains(&fn_name) {
        let handle_var = *cx
            .var_map
            .get(&fn_name)
            .ok_or_else(|| CodegenError::UndefinedVariable(fn_name.clone()))?;
        let handle_ptr = builder.use_var(handle_var);

        // Dispatch to fj_rt_closure_call_N based on user arg count
        let user_arg_count = args.len();
        let call_fn_name = match user_arg_count {
            0 => "__closure_call_0",
            1 => "__closure_call_1",
            2 => "__closure_call_2",
            _ => {
                return Err(CodegenError::NotImplemented(format!(
                    "closure handle call with {} args not supported (max 2)",
                    user_arg_count
                )));
            }
        };

        let fn_id = *cx
            .functions
            .get(call_fn_name)
            .ok_or_else(|| CodegenError::Internal(format!("{call_fn_name} not declared")))?;
        let callee = cx.module.declare_func_in_func(fn_id, builder.func);

        let mut call_args = vec![handle_ptr];
        for a in args {
            call_args.push(compile_expr(builder, cx, &a.value)?);
        }

        let call = builder.ins().call(callee, &call_args);
        let results = builder.inst_results(call);
        cx.last_expr_type = Some(clif_types::default_int_type());
        if results.is_empty() {
            return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
        }
        return Ok(results[0]);
    }

    // ── Function pointer indirect call ────────────────────────────────
    if let Some((param_types, ret_type)) = cx.fn_ptr_sigs.get(&fn_name).cloned() {
        return compile_fn_ptr_call(builder, cx, &fn_name, args, &param_types, ret_type);
    }

    // ── Regular function call ─────────────────────────────────────────
    compile_regular_call(builder, cx, &fn_name, args)
}

/// Compiles a regular (non-builtin) function call.
fn compile_regular_call<M: Module>(
    builder: &mut FunctionBuilder,
    cx: &mut CodegenCtx<'_, M>,
    fn_name: &str,
    args: &[CallArg],
) -> Result<ClifValue, CodegenError> {
    // Try direct name first, then module-prefixed name for intra-module calls
    let resolved_name = if cx.functions.contains_key(fn_name) {
        fn_name.to_string()
    } else if let Some(ref mod_prefix) = cx.current_module {
        let prefixed = format!("{}_{}", mod_prefix, fn_name);
        if cx.functions.contains_key(&prefixed) {
            prefixed
        } else {
            return Err(CodegenError::UndefinedFunction(fn_name.to_string()));
        }
    } else {
        return Err(CodegenError::UndefinedFunction(fn_name.to_string()));
    };
    let func_id = *cx.functions.get(&resolved_name).expect("just checked");

    let local_callee = cx.module.declare_func_in_func(func_id, builder.func);
    let mut call_args = Vec::new();
    for a in args {
        let val = compile_expr(builder, cx, &a.value)?;
        // If argument is a stack array, pass pointer
        if let Expr::Ident { name, .. } = &a.value {
            if let Some((slot, _)) = cx.array_meta.get(name) {
                let ptr = builder
                    .ins()
                    .stack_addr(clif_types::pointer_type(), *slot, 0);
                call_args.push(ptr);
                // String length is not applicable for arrays
                cx.last_string_len = None;
                continue;
            }
        }
        call_args.push(val);
        // If the argument produced a string value, pass the length too
        // (matches the ABI where str params get an extra len param)
        if let Some(str_len) = cx.last_string_len.take() {
            call_args.push(str_len);
        }
    }

    let call = builder.ins().call(local_callee, &call_args);
    let results: Vec<ClifValue> = builder.inst_results(call).to_vec();

    // Track return type
    if let Some(&ret_ty) = cx.fn_return_types.get(&resolved_name) {
        cx.last_expr_type = Some(ret_ty);
    } else if let Some(&ret_ty) = cx.fn_return_types.get(fn_name) {
        cx.last_expr_type = Some(ret_ty);
    } else {
        cx.last_expr_type = Some(clif_types::default_int_type());
    }

    if results.is_empty() {
        return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
    }

    // Handle string-returning functions (dual return: ptr, len)
    if (cx.fn_returns_string.contains(&resolved_name) || cx.fn_returns_string.contains(fn_name))
        && results.len() >= 2
    {
        cx.last_string_len = Some(results[1]);
        // User-defined functions may return string literals (not heap-allocated),
        // so we cannot assume ownership. Leaking heap strings from fn returns
        // is acceptable until we have proper ownership tracking across call boundaries.
        cx.last_string_owned = false;
        cx.last_expr_type = Some(clif_types::pointer_type());
        return Ok(results[0]);
    }

    // Handle enum-returning functions (dual return: tag, payload)
    if (cx.fn_returns_enum.contains(&resolved_name) || cx.fn_returns_enum.contains(fn_name))
        && results.len() >= 2
    {
        cx.last_enum_payload = Some(results[1]);
        cx.last_enum_payload_type = Some(clif_types::default_int_type());
        cx.last_expr_type = Some(clif_types::default_int_type());
        return Ok(results[0]);
    }

    // Handle functions returning closure handles
    if cx.fn_returns_closure_handle.contains(&resolved_name)
        || cx.fn_returns_closure_handle.contains(fn_name)
    {
        cx.last_closure_handle = true;
    }

    // Handle struct-returning functions (multi-value return)
    if let Some(sname) = cx
        .fn_returns_struct
        .get(&resolved_name)
        .or_else(|| cx.fn_returns_struct.get(fn_name))
        .cloned()
    {
        if let Some(fields) = cx.struct_defs.get(&sname).cloned() {
            let num_fields = fields.len();
            let slot_size = (num_fields as u32) * 8;
            let slot = builder.create_sized_stack_slot(cranelift_codegen::ir::StackSlotData::new(
                cranelift_codegen::ir::StackSlotKind::ExplicitSlot,
                slot_size,
                0,
            ));
            for (i, (_fname, _ftype)) in fields.iter().enumerate() {
                if i < results.len() {
                    builder.ins().stack_store(results[i], slot, (i as i32) * 8);
                }
            }
            cx.last_struct_init = Some((slot, sname));
            cx.last_expr_type = Some(clif_types::pointer_type());
            let ptr = builder
                .ins()
                .stack_addr(clif_types::pointer_type(), slot, 0);
            return Ok(ptr);
        }
    }

    // Handle array-returning functions
    if let Some((arr_len, elem_type)) = cx.fn_array_returns.get(fn_name).copied() {
        let slot = builder.create_sized_stack_slot(cranelift_codegen::ir::StackSlotData::new(
            cranelift_codegen::ir::StackSlotKind::ExplicitSlot,
            (arr_len as u32) * 8,
            3,
        ));
        let src_ptr = results[0];
        for idx in 0..arr_len {
            let offset = builder
                .ins()
                .iconst(clif_types::default_int_type(), (idx as i64) * 8);
            let src_addr = builder.ins().iadd(src_ptr, offset);
            let elem_val = builder.ins().load(
                elem_type,
                cranelift_codegen::ir::MemFlags::new(),
                src_addr,
                0,
            );
            builder.ins().stack_store(elem_val, slot, (idx as i32) * 8);
        }
        cx.last_array = Some((slot, arr_len));
        cx.last_expr_type = Some(elem_type);
        let ptr = builder
            .ins()
            .stack_addr(clif_types::pointer_type(), slot, 0);
        return Ok(ptr);
    }

    // Handle functions returning heap-allocated dynamic arrays (Slice type)
    if cx.fn_returns_heap_array.contains(&resolved_name)
        || cx.fn_returns_heap_array.contains(fn_name)
    {
        cx.last_heap_array_return = true;
        cx.last_expr_type = Some(clif_types::pointer_type());
    }

    Ok(results[0])
}

/// Compiles a Path::call, e.g., `Type::method(args)` or `Enum::Variant(payload)`.
fn compile_path_call<M: Module>(
    builder: &mut FunctionBuilder,
    cx: &mut CodegenCtx<'_, M>,
    type_name: &str,
    method_name: &str,
    args: &[CallArg],
) -> Result<ClifValue, CodegenError> {
    // thread::spawn(fn) → call fj_rt_thread_spawn / fj_rt_thread_spawn_noarg
    if type_name == "thread" && method_name == "spawn" {
        if args.is_empty() {
            return Err(CodegenError::NotImplemented(
                "thread::spawn requires a function argument".into(),
            ));
        }
        let fn_addr = compile_expr(builder, cx, &args[0].value)?;
        // Check if there's a second arg (the argument to pass to the thread function)
        let (spawn_key, call_args): (&str, Vec<ClifValue>) = if args.len() >= 2 {
            let arg_val = compile_expr(builder, cx, &args[1].value)?;
            ("__thread_spawn", vec![fn_addr, arg_val])
        } else {
            ("__thread_spawn_noarg", vec![fn_addr])
        };
        let spawn_id = *cx
            .functions
            .get(spawn_key)
            .ok_or_else(|| CodegenError::Internal(format!("{spawn_key} not declared")))?;
        let local_callee = cx.module.declare_func_in_func(spawn_id, builder.func);
        let call = builder.ins().call(local_callee, &call_args);
        let results = builder.inst_results(call);
        cx.last_expr_type = Some(clif_types::pointer_type());
        cx.last_thread_spawn = true;
        return Ok(results[0]);
    }

    // Mutex::new(val) → call fj_rt_mutex_new(val)
    if type_name == "Mutex" && method_name == "new" {
        let initial = if args.is_empty() {
            builder.ins().iconst(clif_types::default_int_type(), 0)
        } else {
            compile_expr(builder, cx, &args[0].value)?
        };
        let mutex_new_id = *cx
            .functions
            .get("__mutex_new")
            .ok_or_else(|| CodegenError::Internal("__mutex_new not declared".into()))?;
        let local_callee = cx.module.declare_func_in_func(mutex_new_id, builder.func);
        let call = builder.ins().call(local_callee, &[initial]);
        let results = builder.inst_results(call);
        cx.last_expr_type = Some(clif_types::pointer_type());
        cx.last_mutex_new = true;
        return Ok(results[0]);
    }

    // channel::new() → call fj_rt_channel_new()
    if type_name == "channel" && method_name == "new" {
        let channel_new_id = *cx
            .functions
            .get("__channel_new")
            .ok_or_else(|| CodegenError::Internal("__channel_new not declared".into()))?;
        let local_callee = cx.module.declare_func_in_func(channel_new_id, builder.func);
        let call = builder.ins().call(local_callee, &[]);
        let results = builder.inst_results(call);
        cx.last_expr_type = Some(clif_types::pointer_type());
        cx.last_channel_new = true;
        return Ok(results[0]);
    }

    // channel::bounded(capacity) → call fj_rt_channel_bounded(capacity)
    if type_name == "channel" && method_name == "bounded" {
        let capacity = if args.is_empty() {
            builder.ins().iconst(clif_types::default_int_type(), 1)
        } else {
            compile_expr(builder, cx, &args[0].value)?
        };
        let bounded_id = *cx
            .functions
            .get("__channel_bounded")
            .ok_or_else(|| CodegenError::Internal("__channel_bounded not declared".into()))?;
        let local_callee = cx.module.declare_func_in_func(bounded_id, builder.func);
        let call = builder.ins().call(local_callee, &[capacity]);
        let results = builder.inst_results(call);
        cx.last_expr_type = Some(clif_types::pointer_type());
        cx.last_bounded_channel = true;
        return Ok(results[0]);
    }

    // Atomic::new / AtomicI32::new / AtomicI64::new / AtomicBool::new
    if matches!(
        type_name,
        "Atomic" | "AtomicI32" | "AtomicI64" | "AtomicBool"
    ) && method_name == "new"
    {
        let initial = if args.is_empty() {
            builder.ins().iconst(clif_types::default_int_type(), 0)
        } else {
            compile_expr(builder, cx, &args[0].value)?
        };
        let (fn_name, subtype) = match type_name {
            "AtomicI32" => ("__atomic_i32_new", "i32"),
            "AtomicBool" => ("__atomic_bool_new", "bool"),
            _ => ("__atomic_new", "i64"), // Atomic | AtomicI64
        };
        let atomic_new_id = *cx
            .functions
            .get(fn_name)
            .ok_or_else(|| CodegenError::Internal(format!("{fn_name} not declared")))?;
        let local_callee = cx.module.declare_func_in_func(atomic_new_id, builder.func);
        let call = builder.ins().call(local_callee, &[initial]);
        let results = builder.inst_results(call);
        cx.last_expr_type = Some(clif_types::pointer_type());
        cx.last_atomic_new = true;
        cx.last_atomic_subtype = subtype.to_string();
        return Ok(results[0]);
    }

    // Barrier::new(n) → call fj_rt_barrier_new(n)
    if type_name == "Barrier" && method_name == "new" {
        let n = if args.is_empty() {
            builder.ins().iconst(clif_types::default_int_type(), 1)
        } else {
            compile_expr(builder, cx, &args[0].value)?
        };
        let barrier_new_id = *cx
            .functions
            .get("__barrier_new")
            .ok_or_else(|| CodegenError::Internal("__barrier_new not declared".into()))?;
        let local_callee = cx.module.declare_func_in_func(barrier_new_id, builder.func);
        let call = builder.ins().call(local_callee, &[n]);
        let results = builder.inst_results(call);
        cx.last_expr_type = Some(clif_types::pointer_type());
        cx.last_barrier_new = true;
        return Ok(results[0]);
    }

    // Condvar::new() → call fj_rt_condvar_new()
    if type_name == "Condvar" && method_name == "new" {
        let condvar_new_id = *cx
            .functions
            .get("__condvar_new")
            .ok_or_else(|| CodegenError::Internal("__condvar_new not declared".into()))?;
        let local_callee = cx.module.declare_func_in_func(condvar_new_id, builder.func);
        let call = builder.ins().call(local_callee, &[]);
        let results = builder.inst_results(call);
        cx.last_expr_type = Some(clif_types::pointer_type());
        cx.last_condvar_new = true;
        return Ok(results[0]);
    }

    // RwLock::new(value) → call fj_rt_rwlock_new(value)
    if type_name == "RwLock" && method_name == "new" {
        let initial = if args.is_empty() {
            builder.ins().iconst(clif_types::default_int_type(), 0)
        } else {
            compile_expr(builder, cx, &args[0].value)?
        };
        let rwlock_new_id = *cx
            .functions
            .get("__rwlock_new")
            .ok_or_else(|| CodegenError::Internal("__rwlock_new not declared".into()))?;
        let local_callee = cx.module.declare_func_in_func(rwlock_new_id, builder.func);
        let call = builder.ins().call(local_callee, &[initial]);
        let results = builder.inst_results(call);
        cx.last_expr_type = Some(clif_types::pointer_type());
        cx.last_rwlock_new = true;
        return Ok(results[0]);
    }

    // Arc::new(value) → call fj_rt_arc_new(value)
    if type_name == "Arc" && method_name == "new" {
        let initial = if args.is_empty() {
            builder.ins().iconst(clif_types::default_int_type(), 0)
        } else {
            compile_expr(builder, cx, &args[0].value)?
        };
        let arc_new_id = *cx
            .functions
            .get("__arc_new")
            .ok_or_else(|| CodegenError::Internal("__arc_new not declared".into()))?;
        let local_callee = cx.module.declare_func_in_func(arc_new_id, builder.func);
        let call = builder.ins().call(local_callee, &[initial]);
        let results = builder.inst_results(call);
        cx.last_expr_type = Some(clif_types::pointer_type());
        cx.last_arc_new = true;
        return Ok(results[0]);
    }

    // VolatilePtr::new(addr) → store raw address, no heap allocation
    if type_name == "VolatilePtr" && method_name == "new" {
        let addr = if args.is_empty() {
            builder.ins().iconst(clif_types::default_int_type(), 0)
        } else {
            compile_expr(builder, cx, &args[0].value)?
        };
        cx.last_expr_type = Some(clif_types::default_int_type());
        cx.last_volatile_ptr_new = true;
        return Ok(addr);
    }

    // MmioRegion::new(base, size) → store base and size, no heap allocation
    if type_name == "MmioRegion" && method_name == "new" {
        let base = if args.is_empty() {
            builder.ins().iconst(clif_types::default_int_type(), 0)
        } else {
            compile_expr(builder, cx, &args[0].value)?
        };
        let size = if args.len() < 2 {
            builder.ins().iconst(clif_types::default_int_type(), 0)
        } else {
            compile_expr(builder, cx, &args[1].value)?
        };
        cx.last_expr_type = Some(clif_types::default_int_type());
        cx.last_mmio_new = true;
        cx.last_mmio_vals = Some((base, size));
        return Ok(base); // Return base as the "handle" value
    }

    // BumpAllocator::new(capacity) → call fj_rt_bump_new(capacity)
    if type_name == "BumpAllocator" && method_name == "new" {
        let cap = if args.is_empty() {
            builder.ins().iconst(clif_types::default_int_type(), 256)
        } else {
            compile_expr(builder, cx, &args[0].value)?
        };
        let fn_id = *cx
            .functions
            .get("__bump_new")
            .ok_or_else(|| CodegenError::Internal("__bump_new not declared".into()))?;
        let callee = cx.module.declare_func_in_func(fn_id, builder.func);
        let call = builder.ins().call(callee, &[cap]);
        let results = builder.inst_results(call);
        cx.last_expr_type = Some(clif_types::pointer_type());
        cx.last_bump_alloc_new = true;
        return Ok(results[0]);
    }

    // FreeListAllocator::new(capacity) → call fj_rt_freelist_new(capacity)
    if type_name == "FreeListAllocator" && method_name == "new" {
        let cap = if args.is_empty() {
            builder.ins().iconst(clif_types::default_int_type(), 256)
        } else {
            compile_expr(builder, cx, &args[0].value)?
        };
        let fn_id = *cx
            .functions
            .get("__freelist_new")
            .ok_or_else(|| CodegenError::Internal("__freelist_new not declared".into()))?;
        let callee = cx.module.declare_func_in_func(fn_id, builder.func);
        let call = builder.ins().call(callee, &[cap]);
        let results = builder.inst_results(call);
        cx.last_expr_type = Some(clif_types::pointer_type());
        cx.last_freelist_alloc_new = true;
        return Ok(results[0]);
    }

    // PoolAllocator::new(block_size, block_count) → call fj_rt_pool_new(block_size, count)
    if type_name == "PoolAllocator" && method_name == "new" {
        let block_size = if args.is_empty() {
            builder.ins().iconst(clif_types::default_int_type(), 8)
        } else {
            compile_expr(builder, cx, &args[0].value)?
        };
        let block_count = if args.len() < 2 {
            builder.ins().iconst(clif_types::default_int_type(), 16)
        } else {
            compile_expr(builder, cx, &args[1].value)?
        };
        let fn_id = *cx
            .functions
            .get("__pool_new")
            .ok_or_else(|| CodegenError::Internal("__pool_new not declared".into()))?;
        let callee = cx.module.declare_func_in_func(fn_id, builder.func);
        let call = builder.ins().call(callee, &[block_size, block_count]);
        let results = builder.inst_results(call);
        cx.last_expr_type = Some(clif_types::pointer_type());
        cx.last_pool_alloc_new = true;
        return Ok(results[0]);
    }

    // Waker::new() → call fj_rt_waker_new()
    if type_name == "Waker" && method_name == "new" {
        let fn_id = *cx
            .functions
            .get("__waker_new")
            .ok_or_else(|| CodegenError::Internal("__waker_new not declared".into()))?;
        let callee = cx.module.declare_func_in_func(fn_id, builder.func);
        let call = builder.ins().call(callee, &[]);
        let results = builder.inst_results(call);
        cx.last_expr_type = Some(clif_types::pointer_type());
        cx.last_waker_new = true;
        return Ok(results[0]);
    }

    // Timer::new() → call fj_rt_timer_new()
    if type_name == "Timer" && method_name == "new" {
        let fn_id = *cx
            .functions
            .get("__timer_new")
            .ok_or_else(|| CodegenError::Internal("__timer_new not declared".into()))?;
        let callee = cx.module.declare_func_in_func(fn_id, builder.func);
        let call = builder.ins().call(callee, &[]);
        let results = builder.inst_results(call);
        cx.last_expr_type = Some(clif_types::pointer_type());
        cx.last_timer_new = true;
        return Ok(results[0]);
    }

    // ThreadPool::new(n) → call fj_rt_threadpool_new(n)
    if type_name == "ThreadPool" && method_name == "new" {
        let n = if args.is_empty() {
            builder.ins().iconst(clif_types::default_int_type(), 4) // default 4 threads
        } else {
            compile_expr(builder, cx, &args[0].value)?
        };
        let fn_id = *cx
            .functions
            .get("__threadpool_new")
            .ok_or_else(|| CodegenError::Internal("__threadpool_new not declared".into()))?;
        let callee = cx.module.declare_func_in_func(fn_id, builder.func);
        let call = builder.ins().call(callee, &[n]);
        let results = builder.inst_results(call);
        cx.last_expr_type = Some(clif_types::pointer_type());
        cx.last_threadpool_new = true;
        return Ok(results[0]);
    }

    // Executor::new() → call fj_rt_executor_new()
    if type_name == "Executor" && method_name == "new" {
        let fn_id = *cx
            .functions
            .get("__executor_new")
            .ok_or_else(|| CodegenError::Internal("__executor_new not declared".into()))?;
        let callee = cx.module.declare_func_in_func(fn_id, builder.func);
        let call = builder.ins().call(callee, &[]);
        let results = builder.inst_results(call);
        cx.last_expr_type = Some(clif_types::pointer_type());
        cx.last_executor_new = true;
        return Ok(results[0]);
    }

    // AsyncChannel::new() → call fj_rt_async_channel_new()
    if type_name == "AsyncChannel" && method_name == "new" {
        let fn_id = *cx
            .functions
            .get("__async_channel_new")
            .ok_or_else(|| CodegenError::Internal("__async_channel_new not declared".into()))?;
        let callee = cx.module.declare_func_in_func(fn_id, builder.func);
        let call = builder.ins().call(callee, &[]);
        let results = builder.inst_results(call);
        cx.last_expr_type = Some(clif_types::pointer_type());
        cx.last_async_channel_new = true;
        return Ok(results[0]);
    }

    // AsyncChannel::bounded(n) → call fj_rt_async_channel_bounded(n)
    if type_name == "AsyncChannel" && method_name == "bounded" {
        let n = if args.is_empty() {
            builder.ins().iconst(clif_types::default_int_type(), 16) // default 16
        } else {
            compile_expr(builder, cx, &args[0].value)?
        };
        let fn_id = *cx
            .functions
            .get("__async_channel_bounded")
            .ok_or_else(|| CodegenError::Internal("__async_channel_bounded not declared".into()))?;
        let callee = cx.module.declare_func_in_func(fn_id, builder.func);
        let call = builder.ins().call(callee, &[n]);
        let results = builder.inst_results(call);
        cx.last_expr_type = Some(clif_types::pointer_type());
        cx.last_async_bchannel_new = true;
        return Ok(results[0]);
    }

    // ── SIMD constructors ─────────────────────────────────────────────

    // Helper: bitcast F64 → I64 (for passing float args to runtime fns with I64 params)
    #[inline]
    fn ensure_i64(
        builder: &mut cranelift_frontend::FunctionBuilder,
        val: cranelift_codegen::ir::Value,
    ) -> cranelift_codegen::ir::Value {
        if clif_types::is_float(builder.func.dfg.value_type(val)) {
            builder.ins().bitcast(
                clif_types::default_int_type(),
                cranelift_codegen::ir::MemFlags::new(),
                val,
            )
        } else {
            val
        }
    }

    // f32x4::new(a, b, c, d)
    if type_name == "f32x4" && method_name == "new" {
        let a = if args.is_empty() {
            builder.ins().iconst(clif_types::default_int_type(), 0)
        } else {
            let v = compile_expr(builder, cx, &args[0].value)?;
            ensure_i64(builder, v)
        };
        let b = if args.len() < 2 {
            builder.ins().iconst(clif_types::default_int_type(), 0)
        } else {
            let v = compile_expr(builder, cx, &args[1].value)?;
            ensure_i64(builder, v)
        };
        let c = if args.len() < 3 {
            builder.ins().iconst(clif_types::default_int_type(), 0)
        } else {
            let v = compile_expr(builder, cx, &args[2].value)?;
            ensure_i64(builder, v)
        };
        let d = if args.len() < 4 {
            builder.ins().iconst(clif_types::default_int_type(), 0)
        } else {
            let v = compile_expr(builder, cx, &args[3].value)?;
            ensure_i64(builder, v)
        };
        let fn_id = *cx
            .functions
            .get("__simd_f32x4_new")
            .ok_or_else(|| CodegenError::Internal("__simd_f32x4_new not declared".into()))?;
        let callee = cx.module.declare_func_in_func(fn_id, builder.func);
        let call = builder.ins().call(callee, &[a, b, c, d]);
        let results = builder.inst_results(call);
        cx.last_expr_type = Some(clif_types::pointer_type());
        cx.last_simd_f32x4_new = true;
        return Ok(results[0]);
    }

    // f32x4::splat(val)
    if type_name == "f32x4" && method_name == "splat" {
        let val = if args.is_empty() {
            builder.ins().iconst(clif_types::default_int_type(), 0)
        } else {
            let v = compile_expr(builder, cx, &args[0].value)?;
            ensure_i64(builder, v)
        };
        let fn_id = *cx
            .functions
            .get("__simd_f32x4_splat")
            .ok_or_else(|| CodegenError::Internal("__simd_f32x4_splat not declared".into()))?;
        let callee = cx.module.declare_func_in_func(fn_id, builder.func);
        let call = builder.ins().call(callee, &[val]);
        let results = builder.inst_results(call);
        cx.last_expr_type = Some(clif_types::pointer_type());
        cx.last_simd_f32x4_new = true;
        return Ok(results[0]);
    }

    // f32x4::zeros()
    if type_name == "f32x4" && method_name == "zeros" {
        let fn_id = *cx
            .functions
            .get("__simd_f32x4_zeros")
            .ok_or_else(|| CodegenError::Internal("__simd_f32x4_zeros not declared".into()))?;
        let callee = cx.module.declare_func_in_func(fn_id, builder.func);
        let call = builder.ins().call(callee, &[]);
        let results = builder.inst_results(call);
        cx.last_expr_type = Some(clif_types::pointer_type());
        cx.last_simd_f32x4_new = true;
        return Ok(results[0]);
    }

    // i32x4::new(a, b, c, d)
    if type_name == "i32x4" && method_name == "new" {
        let a = if args.is_empty() {
            builder.ins().iconst(clif_types::default_int_type(), 0)
        } else {
            compile_expr(builder, cx, &args[0].value)?
        };
        let b = if args.len() < 2 {
            builder.ins().iconst(clif_types::default_int_type(), 0)
        } else {
            compile_expr(builder, cx, &args[1].value)?
        };
        let c = if args.len() < 3 {
            builder.ins().iconst(clif_types::default_int_type(), 0)
        } else {
            compile_expr(builder, cx, &args[2].value)?
        };
        let d = if args.len() < 4 {
            builder.ins().iconst(clif_types::default_int_type(), 0)
        } else {
            compile_expr(builder, cx, &args[3].value)?
        };
        let fn_id = *cx
            .functions
            .get("__simd_i32x4_new")
            .ok_or_else(|| CodegenError::Internal("__simd_i32x4_new not declared".into()))?;
        let callee = cx.module.declare_func_in_func(fn_id, builder.func);
        let call = builder.ins().call(callee, &[a, b, c, d]);
        let results = builder.inst_results(call);
        cx.last_expr_type = Some(clif_types::pointer_type());
        cx.last_simd_i32x4_new = true;
        return Ok(results[0]);
    }

    // i32x4::splat(val)
    if type_name == "i32x4" && method_name == "splat" {
        let val = if args.is_empty() {
            builder.ins().iconst(clif_types::default_int_type(), 0)
        } else {
            compile_expr(builder, cx, &args[0].value)?
        };
        let fn_id = *cx
            .functions
            .get("__simd_i32x4_splat")
            .ok_or_else(|| CodegenError::Internal("__simd_i32x4_splat not declared".into()))?;
        let callee = cx.module.declare_func_in_func(fn_id, builder.func);
        let call = builder.ins().call(callee, &[val]);
        let results = builder.inst_results(call);
        cx.last_expr_type = Some(clif_types::pointer_type());
        cx.last_simd_i32x4_new = true;
        return Ok(results[0]);
    }

    // f32x8::splat(val)
    if type_name == "f32x8" && method_name == "splat" {
        let val = if args.is_empty() {
            builder.ins().iconst(clif_types::default_int_type(), 0)
        } else {
            let v = compile_expr(builder, cx, &args[0].value)?;
            ensure_i64(builder, v)
        };
        let fn_id = *cx
            .functions
            .get("__simd_f32x8_splat")
            .ok_or_else(|| CodegenError::Internal("__simd_f32x8_splat not declared".into()))?;
        let callee = cx.module.declare_func_in_func(fn_id, builder.func);
        let call = builder.ins().call(callee, &[val]);
        let results = builder.inst_results(call);
        cx.last_expr_type = Some(clif_types::pointer_type());
        cx.last_simd_f32x8_new = true;
        return Ok(results[0]);
    }

    // i32x8::splat(val)
    if type_name == "i32x8" && method_name == "splat" {
        let val = if args.is_empty() {
            builder.ins().iconst(clif_types::default_int_type(), 0)
        } else {
            compile_expr(builder, cx, &args[0].value)?
        };
        let fn_id = *cx
            .functions
            .get("__simd_i32x8_splat")
            .ok_or_else(|| CodegenError::Internal("__simd_i32x8_splat not declared".into()))?;
        let callee = cx.module.declare_func_in_func(fn_id, builder.func);
        let call = builder.ins().call(callee, &[val]);
        let results = builder.inst_results(call);
        cx.last_expr_type = Some(clif_types::pointer_type());
        cx.last_simd_i32x8_new = true;
        return Ok(results[0]);
    }

    // OnnxModel::new()
    if type_name == "OnnxModel" && method_name == "new" {
        let fn_id = *cx
            .functions
            .get("__onnx_new")
            .ok_or_else(|| CodegenError::Internal("__onnx_new not declared".into()))?;
        let callee = cx.module.declare_func_in_func(fn_id, builder.func);
        let call = builder.ins().call(callee, &[]);
        let results = builder.inst_results(call);
        cx.last_expr_type = Some(clif_types::pointer_type());
        cx.last_onnx_new = true;
        return Ok(results[0]);
    }

    // Stream::new() → call fj_rt_stream_new()
    if type_name == "Stream" && method_name == "new" {
        let fn_id = *cx
            .functions
            .get("__stream_new")
            .ok_or_else(|| CodegenError::Internal("__stream_new not declared".into()))?;
        let callee = cx.module.declare_func_in_func(fn_id, builder.func);
        let call = builder.ins().call(callee, &[]);
        let results = builder.inst_results(call);
        cx.last_expr_type = Some(clif_types::pointer_type());
        cx.last_stream_new = true;
        return Ok(results[0]);
    }

    // Stream::from_range(start, end) → call fj_rt_stream_from_range(start, end)
    if type_name == "Stream" && method_name == "from_range" {
        let start = if args.is_empty() {
            builder.ins().iconst(clif_types::default_int_type(), 0)
        } else {
            compile_expr(builder, cx, &args[0].value)?
        };
        let end = if args.len() < 2 {
            builder.ins().iconst(clif_types::default_int_type(), 10)
        } else {
            compile_expr(builder, cx, &args[1].value)?
        };
        let fn_id = *cx
            .functions
            .get("__stream_from_range")
            .ok_or_else(|| CodegenError::Internal("__stream_from_range not declared".into()))?;
        let callee = cx.module.declare_func_in_func(fn_id, builder.func);
        let call = builder.ins().call(callee, &[start, end]);
        let results = builder.inst_results(call);
        cx.last_expr_type = Some(clif_types::pointer_type());
        cx.last_stream_new = true;
        return Ok(results[0]);
    }

    // HashMap::new() → call fj_rt_map_new()
    if type_name == "HashMap" && method_name == "new" {
        let map_new_id = *cx
            .functions
            .get("__map_new")
            .ok_or_else(|| CodegenError::Internal("__map_new not declared".into()))?;
        let local_callee = cx.module.declare_func_in_func(map_new_id, builder.func);
        let call = builder.ins().call(local_callee, &[]);
        let results = builder.inst_results(call);
        cx.last_expr_type = Some(clif_types::pointer_type());
        cx.last_map_new = true;
        return Ok(results[0]);
    }

    // Enum constructor: Enum::Variant(payload)
    if let Some(variants) = cx.enum_defs.get(type_name).cloned() {
        if let Some(tag_idx) = variants.iter().position(|v| v == method_name) {
            if !args.is_empty() {
                let payload = compile_expr(builder, cx, &args[0].value)?;
                let payload_type = cx.last_expr_type.unwrap_or(clif_types::default_int_type());
                cx.last_enum_payload = Some(payload);
                cx.last_enum_payload_type = Some(payload_type);
            } else {
                cx.last_enum_payload =
                    Some(builder.ins().iconst(clif_types::default_int_type(), 0));
                cx.last_enum_payload_type = Some(clif_types::default_int_type());
            }
            cx.last_expr_type = Some(clif_types::default_int_type());
            return Ok(builder
                .ins()
                .iconst(clif_types::default_int_type(), tag_idx as i64));
        }
    }

    // Static method: Type::method(args) → mangled as Type_method
    let key = (type_name.to_string(), method_name.to_string());
    if let Some(mangled) = cx.impl_methods.get(&key).cloned() {
        let func_id = *cx
            .functions
            .get(&mangled)
            .ok_or_else(|| CodegenError::UndefinedFunction(mangled.clone()))?;
        let local_callee = cx.module.declare_func_in_func(func_id, builder.func);
        let mut call_args = Vec::new();
        for a in args {
            call_args.push(compile_expr(builder, cx, &a.value)?);
        }
        let call = builder.ins().call(local_callee, &call_args);
        let results: Vec<_> = builder.inst_results(call).to_vec();
        if let Some(&ret_ty) = cx.fn_return_types.get(&mangled) {
            cx.last_expr_type = Some(ret_ty);
        }

        // Handle string return from static method
        if cx.fn_returns_string.contains(&mangled) && results.len() >= 2 {
            cx.last_string_len = Some(results[1]);
            cx.last_string_owned = false;
            cx.last_expr_type = Some(clif_types::pointer_type());
        }

        // Handle struct return from static method (e.g. Point::new(x, y))
        if let Some(sname) = cx.fn_returns_struct.get(&mangled).cloned() {
            if let Some(fields) = cx.struct_defs.get(&sname).cloned() {
                let num_fields = fields.len();
                let ret_slot =
                    builder.create_sized_stack_slot(cranelift_codegen::ir::StackSlotData::new(
                        cranelift_codegen::ir::StackSlotKind::ExplicitSlot,
                        (num_fields as u32) * 8,
                        0,
                    ));
                for (i, _) in fields.iter().enumerate() {
                    if i < results.len() {
                        builder
                            .ins()
                            .stack_store(results[i], ret_slot, (i as i32) * 8);
                    }
                }
                cx.last_struct_init = Some((ret_slot, sname));
                cx.last_expr_type = Some(clif_types::pointer_type());
                let ptr = builder
                    .ins()
                    .stack_addr(clif_types::pointer_type(), ret_slot, 0);
                return Ok(ptr);
            }
        }

        if results.is_empty() {
            return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
        }
        return Ok(results[0]);
    }

    // Trait qualified call: Trait::method(obj, ...) → resolve via trait_impls
    if cx.trait_defs.contains_key(type_name) && !args.is_empty() {
        // Infer the struct type of the first argument
        let arg_struct_type = match &args[0].value {
            Expr::Ident { name, .. } => cx.struct_slots.get(name).map(|(_, sname)| sname.clone()),
            _ => None,
        };
        if let Some(struct_type) = arg_struct_type {
            let key = (struct_type.clone(), method_name.to_string());
            if let Some(mangled) = cx.impl_methods.get(&key).cloned() {
                let func_id = *cx
                    .functions
                    .get(&mangled)
                    .ok_or_else(|| CodegenError::UndefinedFunction(mangled.clone()))?;
                let local_callee = cx.module.declare_func_in_func(func_id, builder.func);
                let mut call_args = Vec::new();
                for a in args {
                    call_args.push(compile_expr(builder, cx, &a.value)?);
                }
                let call = builder.ins().call(local_callee, &call_args);
                let results: Vec<ClifValue> = builder.inst_results(call).to_vec();
                if let Some(&ret_ty) = cx.fn_return_types.get(&mangled) {
                    cx.last_expr_type = Some(ret_ty);
                }
                if results.is_empty() {
                    return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
                }
                return Ok(results[0]);
            }
        }
    }

    // Module-qualified call: mod::function → mod_function
    let mod_fn_name = format!("{}_{}", type_name, method_name);
    if let Some(&func_id) = cx.functions.get(&mod_fn_name) {
        let local_callee = cx.module.declare_func_in_func(func_id, builder.func);
        let mut call_args = Vec::new();
        for a in args {
            call_args.push(compile_expr(builder, cx, &a.value)?);
        }
        let call = builder.ins().call(local_callee, &call_args);
        let results = builder.inst_results(call);
        if let Some(&ret_ty) = cx.fn_return_types.get(&mod_fn_name) {
            cx.last_expr_type = Some(ret_ty);
        }
        if results.is_empty() {
            return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
        }
        // Check if function returns a string
        if cx.fn_returns_string.contains(&mod_fn_name) {
            if results.len() >= 2 {
                cx.last_string_len = Some(results[1]);
            }
            cx.last_expr_type = Some(clif_types::pointer_type());
        }
        return Ok(results[0]);
    }

    // Built-in enum constructors: Option::Some(val), Option::None
    if type_name == "Option" {
        match method_name {
            "Some" => {
                return compile_enum_constructor(builder, cx, "Some", args);
            }
            "None" => {
                cx.last_expr_type = Some(clif_types::default_int_type());
                cx.last_enum_payload =
                    Some(builder.ins().iconst(clif_types::default_int_type(), 0));
                cx.last_enum_payload_type = Some(clif_types::default_int_type());
                return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
            }
            _ => {}
        }
    }

    // User-defined enum variant constructors: Enum::Variant(payload)
    if let Some(variants) = cx.enum_defs.get(type_name) {
        if let Some(tag_idx) = variants.iter().position(|v| v == method_name) {
            if !args.is_empty() {
                let payload = compile_expr(builder, cx, &args[0].value)?;
                let payload_type = cx.last_expr_type.unwrap_or(clif_types::default_int_type());
                cx.last_enum_payload = Some(payload);
                cx.last_enum_payload_type = Some(payload_type);
            } else {
                cx.last_enum_payload =
                    Some(builder.ins().iconst(clif_types::default_int_type(), 0));
                cx.last_enum_payload_type = Some(clif_types::default_int_type());
            }
            cx.last_expr_type = Some(clif_types::default_int_type());
            return Ok(builder
                .ins()
                .iconst(clif_types::default_int_type(), tag_idx as i64));
        }
    }

    Err(CodegenError::UndefinedFunction(format!(
        "{}::{}",
        type_name, method_name
    )))
}

/// Compiles an enum constructor call: Some(x), Ok(x), Err(x).
fn compile_enum_constructor<M: Module>(
    builder: &mut FunctionBuilder,
    cx: &mut CodegenCtx<'_, M>,
    variant: &str,
    args: &[CallArg],
) -> Result<ClifValue, CodegenError> {
    use cranelift_codegen::ir::StackSlotData;
    use cranelift_codegen::ir::StackSlotKind::ExplicitSlot;

    let tag = resolve_variant_tag(cx, variant)?;

    cx.last_enum_multi_payload = None; // Reset multi-payload tracking

    if args.len() > 1 {
        // Multi-field variant: allocate stack slot and store each field
        let mut field_types = Vec::new();
        let mut field_values = Vec::new();
        for arg in args {
            let val = compile_expr(builder, cx, &arg.value)?;
            let ty = cx.last_expr_type.unwrap_or(clif_types::default_int_type());
            field_values.push(val);
            field_types.push(ty);
        }
        let slot_size = (field_types.len() * 8) as u32;
        let slot = builder.create_sized_stack_slot(StackSlotData::new(ExplicitSlot, slot_size, 0));
        for (i, (val, ty)) in field_values.iter().zip(field_types.iter()).enumerate() {
            let offset = (i * 8) as i32;
            builder.ins().stack_store(*val, slot, offset);
            let _ = ty; // type tracked in field_types
        }
        let slot_addr = builder
            .ins()
            .stack_addr(clif_types::default_int_type(), slot, 0);
        cx.last_enum_payload = Some(slot_addr);
        cx.last_enum_payload_type = Some(clif_types::default_int_type());
        cx.last_enum_multi_payload = Some((slot, field_types));
    } else if !args.is_empty() {
        let payload = compile_expr(builder, cx, &args[0].value)?;
        let payload_type = cx.last_expr_type.unwrap_or(clif_types::default_int_type());
        cx.last_enum_payload = Some(payload);
        cx.last_enum_payload_type = Some(payload_type);
    } else {
        cx.last_enum_payload = Some(builder.ins().iconst(clif_types::default_int_type(), 0));
        cx.last_enum_payload_type = Some(clif_types::default_int_type());
    }
    cx.last_expr_type = Some(clif_types::default_int_type());
    Ok(builder.ins().iconst(clif_types::default_int_type(), tag))
}

/// Compiles a generic function call with type-aware monomorphization dispatch.
///
/// Supports multi-type-param generics: for `fn foo<T, U>(a: T, b: U)`,
/// infers each generic param's type from the corresponding argument,
/// producing composite suffix like `"i64_f64"`.
fn compile_generic_call<M: Module>(
    builder: &mut FunctionBuilder,
    cx: &mut CodegenCtx<'_, M>,
    fn_name: &str,
    args: &[CallArg],
) -> Result<ClifValue, CodegenError> {
    let type_suffix = infer_generic_call_suffix(cx, fn_name, args);

    let typed_name = format!("{fn_name}__mono_{type_suffix}");
    let resolved = if cx.functions.contains_key(&typed_name) {
        typed_name
    } else if let Some(mangled) = cx.mono_map.get(fn_name) {
        mangled.clone()
    } else {
        fn_name.to_string()
    };

    compile_regular_call(builder, cx, &resolved, args)
}

/// Infers composite type suffix for a generic function call.
///
/// For multi-param generics like `fn foo<T, U>(a: T, b: U)` called with `foo(1, 3.14)`,
/// returns `"i64_f64"`. For single-param generics, returns `"i64"`, `"f64"`, or `"str"`.
pub(crate) fn infer_generic_call_suffix<M: Module>(
    cx: &CodegenCtx<'_, M>,
    fn_name: &str,
    args: &[CallArg],
) -> String {
    if let Some(param_mappings) = cx.generic_fn_params.get(fn_name) {
        if param_mappings.len() > 1 {
            let mut seen_params: Vec<String> = Vec::new();
            let mut type_parts: Vec<String> = Vec::new();
            for &(arg_idx, ref gp_name) in param_mappings {
                if seen_params.contains(gp_name) {
                    continue;
                }
                seen_params.push(gp_name.clone());
                let suffix = if let Some(arg) = args.get(arg_idx) {
                    infer_semantic_type(cx, &arg.value)
                } else {
                    "i64".to_string()
                };
                type_parts.push(suffix);
            }
            return type_parts.join("_");
        }
    }
    // Single-param fallback
    if let Some(first_arg) = args.first() {
        infer_semantic_type(cx, &first_arg.value)
    } else {
        "i64".to_string()
    }
}

/// Infers the semantic type name of an expression for generic dispatch.
///
/// Returns `"str"` for string literals/variables, `"f64"` for floats, `"i64"` otherwise.
/// This works at the AST level (unlike `infer_expr_type` which returns Cranelift IR types
/// where both i64 and str map to the same `I64` pointer type on x86_64).
fn infer_semantic_type<M: Module>(cx: &CodegenCtx<'_, M>, expr: &Expr) -> String {
    match expr {
        Expr::Literal {
            kind: LiteralKind::String(_) | LiteralKind::RawString(_),
            ..
        } => "str".to_string(),
        Expr::Literal {
            kind: LiteralKind::Float(_),
            ..
        } => "f64".to_string(),
        Expr::Ident { name, .. } => {
            // Check if this variable is known to be a string from ownership tracking
            if cx
                .owned_ptrs
                .iter()
                .any(|(n, k)| n == name && matches!(k, OwnedKind::String))
            {
                return "str".to_string();
            }
            // Check Cranelift type for float detection
            let clif_ty = cx
                .var_types
                .get(name)
                .copied()
                .unwrap_or(clif_types::default_int_type());
            if clif_types::is_float(clif_ty) {
                "f64".to_string()
            } else {
                "i64".to_string()
            }
        }
        Expr::Grouped { expr: inner, .. } | Expr::Unary { operand: inner, .. } => {
            infer_semantic_type(cx, inner)
        }
        _ => {
            // Fallback: use Cranelift type inference for float detection
            let inferred = infer_expr_type(cx, expr);
            if clif_types::is_float(inferred) {
                "f64".to_string()
            } else {
                "i64".to_string()
            }
        }
    }
}

/// Compiles an indirect call through a function pointer variable.
fn compile_fn_ptr_call<M: Module>(
    builder: &mut FunctionBuilder,
    cx: &mut CodegenCtx<'_, M>,
    var_name: &str,
    args: &[CallArg],
    param_types: &[cranelift_codegen::ir::Type],
    ret_type: Option<cranelift_codegen::ir::Type>,
) -> Result<ClifValue, CodegenError> {
    let var = *cx
        .var_map
        .get(var_name)
        .ok_or_else(|| CodegenError::UndefinedVariable(var_name.to_string()))?;
    let fn_addr = builder.use_var(var);

    // Build call signature
    let mut sig = cranelift_codegen::ir::Signature::new(cranelift_codegen::isa::CallConv::SystemV);
    for &pt in param_types {
        sig.params.push(cranelift_codegen::ir::AbiParam::new(pt));
    }
    if let Some(rt) = ret_type {
        sig.returns.push(cranelift_codegen::ir::AbiParam::new(rt));
    }
    let sig_ref = builder.import_signature(sig);

    // Compile arguments
    let mut call_args = Vec::new();
    for a in args {
        call_args.push(compile_expr(builder, cx, &a.value)?);
    }

    let call = builder.ins().call_indirect(sig_ref, fn_addr, &call_args);
    let results = builder.inst_results(call);

    if let Some(rt) = ret_type {
        cx.last_expr_type = Some(rt);
        if results.is_empty() {
            Ok(builder.ins().iconst(clif_types::default_int_type(), 0))
        } else {
            Ok(results[0])
        }
    } else {
        cx.last_expr_type = Some(clif_types::default_int_type());
        Ok(builder.ins().iconst(clif_types::default_int_type(), 0))
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Method call compilation
// ═══════════════════════════════════════════════════════════════════════

/// Compiles a method call: `receiver.method(args)`.
///
/// Dispatches string methods, array methods, and struct impl methods.
pub(in crate::codegen::cranelift) fn compile_method_call<M: Module>(
    builder: &mut FunctionBuilder,
    cx: &mut CodegenCtx<'_, M>,
    receiver: &Expr,
    method: &str,
    args: &[CallArg],
) -> Result<ClifValue, CodegenError> {
    let recv_name = match receiver {
        Expr::Ident { name, .. } => Some(name.clone()),
        _ => None,
    };

    // ── String methods ────────────────────────────────────────────────
    let is_string_recv = recv_name
        .as_ref()
        .is_some_and(|n| cx.string_lens.contains_key(n))
        || is_string_producing_expr(receiver);

    if is_string_recv {
        return compile_string_method(builder, cx, receiver, method, args);
    }

    // ── Thread handle methods ──────────────────────────────────────────
    if let Some(ref name) = recv_name {
        if cx.thread_handles.contains(name) {
            let handle_var = *cx
                .var_map
                .get(name)
                .ok_or_else(|| CodegenError::UndefinedVariable(name.clone()))?;
            let handle_ptr = builder.use_var(handle_var);
            match method {
                "join" => {
                    let join_id = *cx.functions.get("__thread_join").ok_or_else(|| {
                        CodegenError::Internal("__thread_join not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(join_id, builder.func);
                    let call = builder.ins().call(callee, &[handle_ptr]);
                    let results = builder.inst_results(call);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(results[0]);
                }
                "is_finished" => {
                    let fin_id = *cx.functions.get("__thread_is_finished").ok_or_else(|| {
                        CodegenError::Internal("__thread_is_finished not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(fin_id, builder.func);
                    let call = builder.ins().call(callee, &[handle_ptr]);
                    let results = builder.inst_results(call);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(results[0]);
                }
                _ => {
                    return Err(CodegenError::NotImplemented(format!(
                        "thread handle method '{method}'"
                    )));
                }
            }
        }
    }

    // ── Mutex methods ────────────────────────────────────────────────
    if let Some(ref name) = recv_name {
        if cx.mutex_handles.contains(name) {
            let mutex_var = *cx
                .var_map
                .get(name)
                .ok_or_else(|| CodegenError::UndefinedVariable(name.clone()))?;
            let mutex_ptr = builder.use_var(mutex_var);
            match method {
                "lock" => {
                    let lock_id = *cx.functions.get("__mutex_lock").ok_or_else(|| {
                        CodegenError::Internal("__mutex_lock not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(lock_id, builder.func);
                    let call = builder.ins().call(callee, &[mutex_ptr]);
                    let results = builder.inst_results(call);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(results[0]);
                }
                "store" => {
                    if args.is_empty() {
                        return Err(CodegenError::NotImplemented(
                            "mutex.store requires a value argument".into(),
                        ));
                    }
                    let val = compile_expr(builder, cx, &args[0].value)?;
                    let store_id = *cx.functions.get("__mutex_store").ok_or_else(|| {
                        CodegenError::Internal("__mutex_store not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(store_id, builder.func);
                    builder.ins().call(callee, &[mutex_ptr, val]);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
                }
                "try_lock" => {
                    // S2.1: Returns Option<i64> — Some(value) on success, None on failure.
                    // Some tag=0, None tag=1.
                    let out_slot =
                        builder.create_sized_stack_slot(cranelift_codegen::ir::StackSlotData::new(
                            cranelift_codegen::ir::StackSlotKind::ExplicitSlot,
                            8,
                            0,
                        ));
                    let out_addr =
                        builder
                            .ins()
                            .stack_addr(clif_types::pointer_type(), out_slot, 0);
                    let try_lock_id = *cx.functions.get("__mutex_try_lock").ok_or_else(|| {
                        CodegenError::Internal("__mutex_try_lock not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(try_lock_id, builder.func);
                    let call = builder.ins().call(callee, &[mutex_ptr, out_addr]);
                    let success = builder.inst_results(call)[0]; // 1=success, 0=fail
                                                                 // Convert to Option: Some=1, None=0 (built-in tag convention)
                                                                 // success already maps: 1→Some(1), 0→None(0)
                    let tag = success;
                    // payload = select(success, loaded_value, 0)
                    let payload_val =
                        builder
                            .ins()
                            .stack_load(clif_types::default_int_type(), out_slot, 0);
                    let zero = builder.ins().iconst(clif_types::default_int_type(), 0);
                    let payload = builder.ins().select(success, payload_val, zero);
                    cx.last_enum_payload = Some(payload);
                    cx.last_enum_payload_type = Some(clif_types::default_int_type());
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(tag);
                }
                "lock_guard" => {
                    // S3.5: Lock mutex and return a guard handle (RAII auto-unlock)
                    let lock_id = *cx.functions.get("__mutex_guard_lock").ok_or_else(|| {
                        CodegenError::Internal("__mutex_guard_lock not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(lock_id, builder.func);
                    let call = builder.ins().call(callee, &[mutex_ptr]);
                    let guard_ptr = builder.inst_results(call)[0];
                    cx.last_expr_type = Some(clif_types::pointer_type());
                    cx.last_mutex_guard_new = true;
                    return Ok(guard_ptr);
                }
                _ => {
                    return Err(CodegenError::NotImplemented(format!(
                        "mutex method '{method}'"
                    )));
                }
            }
        }
    }

    // ── MutexGuard methods ────────────────────────────────────────────
    if let Some(ref name) = recv_name {
        if cx.mutex_guard_handles.contains(name) {
            let guard_var = *cx
                .var_map
                .get(name)
                .ok_or_else(|| CodegenError::UndefinedVariable(name.clone()))?;
            let guard_ptr = builder.use_var(guard_var);
            match method {
                "get" => {
                    let get_id = *cx.functions.get("__mutex_guard_get").ok_or_else(|| {
                        CodegenError::Internal("__mutex_guard_get not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(get_id, builder.func);
                    let call = builder.ins().call(callee, &[guard_ptr]);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(builder.inst_results(call)[0]);
                }
                "set" => {
                    if args.is_empty() {
                        return Err(CodegenError::NotImplemented(
                            "guard.set requires a value argument".into(),
                        ));
                    }
                    let val = compile_expr(builder, cx, &args[0].value)?;
                    let set_id = *cx.functions.get("__mutex_guard_set").ok_or_else(|| {
                        CodegenError::Internal("__mutex_guard_set not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(set_id, builder.func);
                    builder.ins().call(callee, &[guard_ptr, val]);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
                }
                _ => {
                    return Err(CodegenError::NotImplemented(format!(
                        "mutex_guard method '{method}'"
                    )));
                }
            }
        }
    }

    // ── Channel methods ───────────────────────────────────────────────
    if let Some(ref name) = recv_name {
        if cx.channel_handles.contains(name) {
            let ch_var = *cx
                .var_map
                .get(name)
                .ok_or_else(|| CodegenError::UndefinedVariable(name.clone()))?;
            let ch_ptr = builder.use_var(ch_var);
            match method {
                "send" => {
                    if args.is_empty() {
                        return Err(CodegenError::NotImplemented(
                            "channel.send requires a value argument".into(),
                        ));
                    }
                    let val = compile_expr(builder, cx, &args[0].value)?;
                    let send_id = *cx.functions.get("__channel_send").ok_or_else(|| {
                        CodegenError::Internal("__channel_send not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(send_id, builder.func);
                    builder.ins().call(callee, &[ch_ptr, val]);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
                }
                "recv" => {
                    let recv_id = *cx.functions.get("__channel_recv").ok_or_else(|| {
                        CodegenError::Internal("__channel_recv not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(recv_id, builder.func);
                    let call = builder.ins().call(callee, &[ch_ptr]);
                    let results = builder.inst_results(call);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(results[0]);
                }
                "close" => {
                    let close_id = *cx.functions.get("__channel_close").ok_or_else(|| {
                        CodegenError::Internal("__channel_close not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(close_id, builder.func);
                    builder.ins().call(callee, &[ch_ptr]);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
                }
                _ => {
                    return Err(CodegenError::NotImplemented(format!(
                        "channel method '{method}'"
                    )));
                }
            }
        }
    }

    // ── Bounded channel methods ───────────────────────────────────────
    if let Some(ref name) = recv_name {
        if cx.bounded_channel_handles.contains(name) {
            let ch_var = *cx
                .var_map
                .get(name)
                .ok_or_else(|| CodegenError::UndefinedVariable(name.clone()))?;
            let ch_ptr = builder.use_var(ch_var);
            match method {
                "send" => {
                    if args.is_empty() {
                        return Err(CodegenError::NotImplemented(
                            "bounded channel.send requires a value argument".into(),
                        ));
                    }
                    let val = compile_expr(builder, cx, &args[0].value)?;
                    let send_id = *cx.functions.get("__channel_bounded_send").ok_or_else(|| {
                        CodegenError::Internal("__channel_bounded_send not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(send_id, builder.func);
                    builder.ins().call(callee, &[ch_ptr, val]);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
                }
                "recv" => {
                    let recv_id = *cx.functions.get("__channel_bounded_recv").ok_or_else(|| {
                        CodegenError::Internal("__channel_bounded_recv not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(recv_id, builder.func);
                    let call = builder.ins().call(callee, &[ch_ptr]);
                    let results = builder.inst_results(call);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(results[0]);
                }
                "try_send" => {
                    if args.is_empty() {
                        return Err(CodegenError::NotImplemented(
                            "bounded channel.try_send requires a value argument".into(),
                        ));
                    }
                    let val = compile_expr(builder, cx, &args[0].value)?;
                    let try_send_id = *cx.functions.get("__channel_try_send").ok_or_else(|| {
                        CodegenError::Internal("__channel_try_send not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(try_send_id, builder.func);
                    let call = builder.ins().call(callee, &[ch_ptr, val]);
                    let results = builder.inst_results(call);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(results[0]);
                }
                _ => {
                    return Err(CodegenError::NotImplemented(format!(
                        "bounded channel method '{method}'"
                    )));
                }
            }
        }
    }

    // ── Atomic methods ─────────────────────────────────────────────────
    if let Some(ref name) = recv_name {
        if cx.atomic_handles.contains(name) {
            let a_var = *cx
                .var_map
                .get(name)
                .ok_or_else(|| CodegenError::UndefinedVariable(name.clone()))?;
            let a_ptr = builder.use_var(a_var);
            let subtype = cx
                .atomic_subtypes
                .get(name)
                .cloned()
                .unwrap_or_else(|| "i64".to_string());
            match method {
                "load" => {
                    let fn_name = match subtype.as_str() {
                        "i32" => "__atomic_i32_load",
                        "bool" => "__atomic_bool_load",
                        _ => "__atomic_load",
                    };
                    let load_id = *cx
                        .functions
                        .get(fn_name)
                        .ok_or_else(|| CodegenError::Internal(format!("{fn_name} not declared")))?;
                    let callee = cx.module.declare_func_in_func(load_id, builder.func);
                    let call = builder.ins().call(callee, &[a_ptr]);
                    let results = builder.inst_results(call);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(results[0]);
                }
                "store" => {
                    if args.is_empty() {
                        return Err(CodegenError::NotImplemented(
                            "atomic.store requires a value argument".into(),
                        ));
                    }
                    let val = compile_expr(builder, cx, &args[0].value)?;
                    let fn_name = match subtype.as_str() {
                        "i32" => "__atomic_i32_store",
                        "bool" => "__atomic_bool_store",
                        _ => "__atomic_store",
                    };
                    let store_id = *cx
                        .functions
                        .get(fn_name)
                        .ok_or_else(|| CodegenError::Internal(format!("{fn_name} not declared")))?;
                    let callee = cx.module.declare_func_in_func(store_id, builder.func);
                    builder.ins().call(callee, &[a_ptr, val]);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
                }
                "load_relaxed" => {
                    let load_id = *cx.functions.get("__atomic_load_relaxed").ok_or_else(|| {
                        CodegenError::Internal("__atomic_load_relaxed not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(load_id, builder.func);
                    let call = builder.ins().call(callee, &[a_ptr]);
                    let results = builder.inst_results(call);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(results[0]);
                }
                "load_acquire" => {
                    let load_id = *cx.functions.get("__atomic_load_acquire").ok_or_else(|| {
                        CodegenError::Internal("__atomic_load_acquire not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(load_id, builder.func);
                    let call = builder.ins().call(callee, &[a_ptr]);
                    let results = builder.inst_results(call);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(results[0]);
                }
                "store_relaxed" => {
                    if args.is_empty() {
                        return Err(CodegenError::NotImplemented(
                            "atomic.store_relaxed requires a value argument".into(),
                        ));
                    }
                    let val = compile_expr(builder, cx, &args[0].value)?;
                    let store_id =
                        *cx.functions.get("__atomic_store_relaxed").ok_or_else(|| {
                            CodegenError::Internal("__atomic_store_relaxed not declared".into())
                        })?;
                    let callee = cx.module.declare_func_in_func(store_id, builder.func);
                    builder.ins().call(callee, &[a_ptr, val]);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
                }
                "store_release" => {
                    if args.is_empty() {
                        return Err(CodegenError::NotImplemented(
                            "atomic.store_release requires a value argument".into(),
                        ));
                    }
                    let val = compile_expr(builder, cx, &args[0].value)?;
                    let store_id =
                        *cx.functions.get("__atomic_store_release").ok_or_else(|| {
                            CodegenError::Internal("__atomic_store_release not declared".into())
                        })?;
                    let callee = cx.module.declare_func_in_func(store_id, builder.func);
                    builder.ins().call(callee, &[a_ptr, val]);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
                }
                "add" | "fetch_add" => {
                    if args.is_empty() {
                        return Err(CodegenError::NotImplemented(
                            "atomic.add requires a value argument".into(),
                        ));
                    }
                    let val = compile_expr(builder, cx, &args[0].value)?;
                    let add_id = *cx.functions.get("__atomic_add").ok_or_else(|| {
                        CodegenError::Internal("__atomic_add not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(add_id, builder.func);
                    let call = builder.ins().call(callee, &[a_ptr, val]);
                    let results = builder.inst_results(call);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(results[0]);
                }
                "sub" | "fetch_sub" => {
                    if args.is_empty() {
                        return Err(CodegenError::NotImplemented(
                            "atomic.sub requires a value argument".into(),
                        ));
                    }
                    let val = compile_expr(builder, cx, &args[0].value)?;
                    let sub_id = *cx.functions.get("__atomic_sub").ok_or_else(|| {
                        CodegenError::Internal("__atomic_sub not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(sub_id, builder.func);
                    let call = builder.ins().call(callee, &[a_ptr, val]);
                    let results = builder.inst_results(call);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(results[0]);
                }
                "compare_exchange" | "cas" => {
                    if args.len() < 2 {
                        return Err(CodegenError::NotImplemented(
                            "atomic.cas requires (expected, desired) arguments".into(),
                        ));
                    }
                    let expected = compile_expr(builder, cx, &args[0].value)?;
                    let desired = compile_expr(builder, cx, &args[1].value)?;
                    let cas_id = *cx.functions.get("__atomic_cas").ok_or_else(|| {
                        CodegenError::Internal("__atomic_cas not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(cas_id, builder.func);
                    let call = builder.ins().call(callee, &[a_ptr, expected, desired]);
                    let results = builder.inst_results(call);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(results[0]);
                }
                "fetch_and" => {
                    if args.is_empty() {
                        return Err(CodegenError::NotImplemented(
                            "atomic.fetch_and requires a value argument".into(),
                        ));
                    }
                    let val = compile_expr(builder, cx, &args[0].value)?;
                    let and_id = *cx.functions.get("__atomic_and").ok_or_else(|| {
                        CodegenError::Internal("__atomic_and not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(and_id, builder.func);
                    let call = builder.ins().call(callee, &[a_ptr, val]);
                    let results = builder.inst_results(call);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(results[0]);
                }
                "fetch_or" => {
                    if args.is_empty() {
                        return Err(CodegenError::NotImplemented(
                            "atomic.fetch_or requires a value argument".into(),
                        ));
                    }
                    let val = compile_expr(builder, cx, &args[0].value)?;
                    let or_id = *cx
                        .functions
                        .get("__atomic_or")
                        .ok_or_else(|| CodegenError::Internal("__atomic_or not declared".into()))?;
                    let callee = cx.module.declare_func_in_func(or_id, builder.func);
                    let call = builder.ins().call(callee, &[a_ptr, val]);
                    let results = builder.inst_results(call);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(results[0]);
                }
                "fetch_xor" => {
                    if args.is_empty() {
                        return Err(CodegenError::NotImplemented(
                            "atomic.fetch_xor requires a value argument".into(),
                        ));
                    }
                    let val = compile_expr(builder, cx, &args[0].value)?;
                    let xor_id = *cx.functions.get("__atomic_xor").ok_or_else(|| {
                        CodegenError::Internal("__atomic_xor not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(xor_id, builder.func);
                    let call = builder.ins().call(callee, &[a_ptr, val]);
                    let results = builder.inst_results(call);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(results[0]);
                }
                _ => {
                    return Err(CodegenError::NotImplemented(format!(
                        "atomic method '{method}'"
                    )));
                }
            }
        }
    }

    // ── Barrier methods ─────────────────────────────────────────────────
    if let Some(ref name) = recv_name {
        if cx.barrier_handles.contains(name) {
            let b_var = *cx
                .var_map
                .get(name)
                .ok_or_else(|| CodegenError::UndefinedVariable(name.clone()))?;
            let b_ptr = builder.use_var(b_var);
            match method {
                "wait" => {
                    let wait_id = *cx.functions.get("__barrier_wait").ok_or_else(|| {
                        CodegenError::Internal("__barrier_wait not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(wait_id, builder.func);
                    builder.ins().call(callee, &[b_ptr]);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
                }
                _ => {
                    return Err(CodegenError::NotImplemented(format!(
                        "barrier method '{method}'"
                    )));
                }
            }
        }
    }

    // ── Condvar methods ─────────────────────────────────────────────────
    if let Some(ref name) = recv_name {
        if cx.condvar_handles.contains(name) {
            let cv_var = *cx
                .var_map
                .get(name)
                .ok_or_else(|| CodegenError::UndefinedVariable(name.clone()))?;
            let cv_ptr = builder.use_var(cv_var);
            match method {
                "wait" => {
                    // .wait(mutex) → fj_rt_condvar_wait(condvar_ptr, mutex_ptr)
                    if args.is_empty() {
                        return Err(CodegenError::NotImplemented(
                            "condvar.wait requires a mutex argument".into(),
                        ));
                    }
                    let mutex_val = compile_expr(builder, cx, &args[0].value)?;
                    let wait_id = *cx.functions.get("__condvar_wait").ok_or_else(|| {
                        CodegenError::Internal("__condvar_wait not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(wait_id, builder.func);
                    let call = builder.ins().call(callee, &[cv_ptr, mutex_val]);
                    let results = builder.inst_results(call);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(results[0]);
                }
                "notify_one" => {
                    let notify_id = *cx.functions.get("__condvar_notify_one").ok_or_else(|| {
                        CodegenError::Internal("__condvar_notify_one not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(notify_id, builder.func);
                    builder.ins().call(callee, &[cv_ptr]);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
                }
                "notify_all" => {
                    let notify_id = *cx.functions.get("__condvar_notify_all").ok_or_else(|| {
                        CodegenError::Internal("__condvar_notify_all not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(notify_id, builder.func);
                    builder.ins().call(callee, &[cv_ptr]);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
                }
                _ => {
                    return Err(CodegenError::NotImplemented(format!(
                        "condvar method '{method}'"
                    )));
                }
            }
        }
    }

    // ── RwLock methods ──────────────────────────────────────────────────
    if let Some(ref name) = recv_name {
        if cx.rwlock_handles.contains(name) {
            let rw_var = *cx
                .var_map
                .get(name)
                .ok_or_else(|| CodegenError::UndefinedVariable(name.clone()))?;
            let rw_ptr = builder.use_var(rw_var);
            match method {
                "read" => {
                    let read_id = *cx.functions.get("__rwlock_read").ok_or_else(|| {
                        CodegenError::Internal("__rwlock_read not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(read_id, builder.func);
                    let call = builder.ins().call(callee, &[rw_ptr]);
                    let results = builder.inst_results(call);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(results[0]);
                }
                "write" => {
                    if args.is_empty() {
                        return Err(CodegenError::NotImplemented(
                            "rwlock.write requires a value argument".into(),
                        ));
                    }
                    let val = compile_expr(builder, cx, &args[0].value)?;
                    let write_id = *cx.functions.get("__rwlock_write").ok_or_else(|| {
                        CodegenError::Internal("__rwlock_write not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(write_id, builder.func);
                    builder.ins().call(callee, &[rw_ptr, val]);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
                }
                _ => {
                    return Err(CodegenError::NotImplemented(format!(
                        "rwlock method '{method}'"
                    )));
                }
            }
        }
    }

    // ── Arc methods ──────────────────────────────────────────────────
    if let Some(ref name) = recv_name {
        if cx.arc_handles.contains(name) {
            let arc_var = *cx
                .var_map
                .get(name)
                .ok_or_else(|| CodegenError::UndefinedVariable(name.clone()))?;
            let arc_ptr = builder.use_var(arc_var);
            match method {
                "load" => {
                    let load_id = *cx
                        .functions
                        .get("__arc_load")
                        .ok_or_else(|| CodegenError::Internal("__arc_load not declared".into()))?;
                    let callee = cx.module.declare_func_in_func(load_id, builder.func);
                    let call = builder.ins().call(callee, &[arc_ptr]);
                    let results = builder.inst_results(call);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(results[0]);
                }
                "store" => {
                    if args.is_empty() {
                        return Err(CodegenError::NotImplemented(
                            "arc.store requires a value argument".into(),
                        ));
                    }
                    let val = compile_expr(builder, cx, &args[0].value)?;
                    let store_id = *cx
                        .functions
                        .get("__arc_store")
                        .ok_or_else(|| CodegenError::Internal("__arc_store not declared".into()))?;
                    let callee = cx.module.declare_func_in_func(store_id, builder.func);
                    builder.ins().call(callee, &[arc_ptr, val]);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
                }
                "clone" => {
                    let clone_id = *cx
                        .functions
                        .get("__arc_clone")
                        .ok_or_else(|| CodegenError::Internal("__arc_clone not declared".into()))?;
                    let callee = cx.module.declare_func_in_func(clone_id, builder.func);
                    let call = builder.ins().call(callee, &[arc_ptr]);
                    let results = builder.inst_results(call);
                    cx.last_expr_type = Some(clif_types::pointer_type());
                    cx.last_arc_new = true;
                    return Ok(results[0]);
                }
                "strong_count" => {
                    let count_id = *cx.functions.get("__arc_strong_count").ok_or_else(|| {
                        CodegenError::Internal("__arc_strong_count not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(count_id, builder.func);
                    let call = builder.ins().call(callee, &[arc_ptr]);
                    let results = builder.inst_results(call);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(results[0]);
                }
                _ => {
                    return Err(CodegenError::NotImplemented(format!(
                        "arc method '{method}'"
                    )));
                }
            }
        }
    }

    // ── BumpAllocator methods ──────────────────────────────────────────
    if let Some(ref name) = recv_name {
        if cx.bump_alloc_handles.contains(name) {
            let handle_var = *cx
                .var_map
                .get(name)
                .ok_or_else(|| CodegenError::UndefinedVariable(name.clone()))?;
            let handle = builder.use_var(handle_var);
            match method {
                "alloc" => {
                    if args.is_empty() {
                        return Err(CodegenError::NotImplemented(
                            "BumpAllocator.alloc requires a size argument".into(),
                        ));
                    }
                    let size = compile_expr(builder, cx, &args[0].value)?;
                    let fn_id = *cx.functions.get("__bump_alloc").ok_or_else(|| {
                        CodegenError::Internal("__bump_alloc not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                    let call = builder.ins().call(callee, &[handle, size]);
                    let results = builder.inst_results(call);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(results[0]);
                }
                "reset" => {
                    let fn_id = *cx.functions.get("__bump_reset").ok_or_else(|| {
                        CodegenError::Internal("__bump_reset not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                    builder.ins().call(callee, &[handle]);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
                }
                "destroy" => {
                    let fn_id = *cx.functions.get("__bump_destroy").ok_or_else(|| {
                        CodegenError::Internal("__bump_destroy not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                    builder.ins().call(callee, &[handle]);
                    // Remove from auto-cleanup to prevent double-free
                    cx.owned_ptrs.retain(|(n, _)| n != name);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
                }
                _ => {
                    return Err(CodegenError::NotImplemented(format!(
                        "BumpAllocator method '{method}'"
                    )));
                }
            }
        }
    }

    // ── FreeListAllocator methods ────────────────────────────────────────
    if let Some(ref name) = recv_name {
        if cx.freelist_alloc_handles.contains(name) {
            let handle_var = *cx
                .var_map
                .get(name)
                .ok_or_else(|| CodegenError::UndefinedVariable(name.clone()))?;
            let handle = builder.use_var(handle_var);
            match method {
                "alloc" => {
                    if args.is_empty() {
                        return Err(CodegenError::NotImplemented(
                            "FreeListAllocator.alloc requires a size argument".into(),
                        ));
                    }
                    let size = compile_expr(builder, cx, &args[0].value)?;
                    let fn_id = *cx.functions.get("__freelist_alloc").ok_or_else(|| {
                        CodegenError::Internal("__freelist_alloc not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                    let call = builder.ins().call(callee, &[handle, size]);
                    let results = builder.inst_results(call);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(results[0]);
                }
                "free" => {
                    if args.len() < 2 {
                        return Err(CodegenError::NotImplemented(
                            "FreeListAllocator.free requires (ptr, size) arguments".into(),
                        ));
                    }
                    let alloc_ptr = compile_expr(builder, cx, &args[0].value)?;
                    let size = compile_expr(builder, cx, &args[1].value)?;
                    let fn_id = *cx.functions.get("__freelist_free").ok_or_else(|| {
                        CodegenError::Internal("__freelist_free not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                    builder.ins().call(callee, &[handle, alloc_ptr, size]);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
                }
                "destroy" => {
                    let fn_id = *cx.functions.get("__freelist_destroy").ok_or_else(|| {
                        CodegenError::Internal("__freelist_destroy not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                    builder.ins().call(callee, &[handle]);
                    cx.owned_ptrs.retain(|(n, _)| n != name);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
                }
                _ => {
                    return Err(CodegenError::NotImplemented(format!(
                        "FreeListAllocator method '{method}'"
                    )));
                }
            }
        }
    }

    // ── PoolAllocator methods ────────────────────────────────────────────
    if let Some(ref name) = recv_name {
        if cx.pool_alloc_handles.contains(name) {
            let handle_var = *cx
                .var_map
                .get(name)
                .ok_or_else(|| CodegenError::UndefinedVariable(name.clone()))?;
            let handle = builder.use_var(handle_var);
            match method {
                "alloc" => {
                    let fn_id = *cx.functions.get("__pool_alloc").ok_or_else(|| {
                        CodegenError::Internal("__pool_alloc not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                    let call = builder.ins().call(callee, &[handle]);
                    let results = builder.inst_results(call);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(results[0]);
                }
                "free" => {
                    if args.is_empty() {
                        return Err(CodegenError::NotImplemented(
                            "PoolAllocator.free requires a pointer argument".into(),
                        ));
                    }
                    let alloc_ptr = compile_expr(builder, cx, &args[0].value)?;
                    let fn_id = *cx
                        .functions
                        .get("__pool_free")
                        .ok_or_else(|| CodegenError::Internal("__pool_free not declared".into()))?;
                    let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                    builder.ins().call(callee, &[handle, alloc_ptr]);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
                }
                "destroy" => {
                    let fn_id = *cx.functions.get("__pool_destroy").ok_or_else(|| {
                        CodegenError::Internal("__pool_destroy not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                    builder.ins().call(callee, &[handle]);
                    cx.owned_ptrs.retain(|(n, _)| n != name);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
                }
                _ => {
                    return Err(CodegenError::NotImplemented(format!(
                        "PoolAllocator method '{method}'"
                    )));
                }
            }
        }
    }

    // ── Executor methods ──────────────────────────────────────────────
    if let Some(ref name) = recv_name {
        if cx.executor_handles.contains(name) {
            let handle_var = *cx
                .var_map
                .get(name)
                .ok_or_else(|| CodegenError::UndefinedVariable(name.clone()))?;
            let handle = builder.use_var(handle_var);
            match method {
                "block_on" => {
                    // exec.block_on(future) → fj_rt_executor_block_on(future_ptr)
                    if args.is_empty() {
                        return Err(CodegenError::NotImplemented(
                            "Executor.block_on requires a future argument".into(),
                        ));
                    }
                    let future_ptr = compile_expr(builder, cx, &args[0].value)?;
                    let fn_id = *cx.functions.get("__executor_block_on").ok_or_else(|| {
                        CodegenError::Internal("__executor_block_on not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                    let call = builder.ins().call(callee, &[future_ptr]);
                    let results = builder.inst_results(call);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(results[0]);
                }
                "spawn" => {
                    // exec.spawn(future) → fj_rt_executor_spawn(exec, future_ptr)
                    if args.is_empty() {
                        return Err(CodegenError::NotImplemented(
                            "Executor.spawn requires a future argument".into(),
                        ));
                    }
                    let future_ptr = compile_expr(builder, cx, &args[0].value)?;
                    let fn_id = *cx.functions.get("__executor_spawn").ok_or_else(|| {
                        CodegenError::Internal("__executor_spawn not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                    builder.ins().call(callee, &[handle, future_ptr]);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
                }
                "run" => {
                    // exec.run() → fj_rt_executor_run(exec) → completed count
                    let fn_id = *cx.functions.get("__executor_run").ok_or_else(|| {
                        CodegenError::Internal("__executor_run not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                    let call = builder.ins().call(callee, &[handle]);
                    let results = builder.inst_results(call);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(results[0]);
                }
                "get_result" => {
                    // exec.get_result(index) → fj_rt_executor_get_result(exec, index)
                    if args.is_empty() {
                        return Err(CodegenError::NotImplemented(
                            "Executor.get_result requires an index argument".into(),
                        ));
                    }
                    let index = compile_expr(builder, cx, &args[0].value)?;
                    let fn_id = *cx.functions.get("__executor_get_result").ok_or_else(|| {
                        CodegenError::Internal("__executor_get_result not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                    let call = builder.ins().call(callee, &[handle, index]);
                    let results = builder.inst_results(call);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(results[0]);
                }
                "free" => {
                    // exec.free() → fj_rt_executor_free(exec)
                    let fn_id = *cx.functions.get("__executor_free").ok_or_else(|| {
                        CodegenError::Internal("__executor_free not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                    builder.ins().call(callee, &[handle]);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
                }
                _ => {
                    return Err(CodegenError::NotImplemented(format!(
                        "Executor method '{method}'"
                    )));
                }
            }
        }
    }

    // ── Waker methods ─────────────────────────────────────────────────
    if let Some(ref name) = recv_name {
        if cx.waker_handles.contains(name) {
            let handle_var = *cx
                .var_map
                .get(name)
                .ok_or_else(|| CodegenError::UndefinedVariable(name.clone()))?;
            let handle = builder.use_var(handle_var);
            match method {
                "wake" => {
                    let fn_id = *cx.functions.get("__waker_wake").ok_or_else(|| {
                        CodegenError::Internal("__waker_wake not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                    builder.ins().call(callee, &[handle]);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
                }
                "is_woken" => {
                    let fn_id = *cx.functions.get("__waker_is_woken").ok_or_else(|| {
                        CodegenError::Internal("__waker_is_woken not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                    let call = builder.ins().call(callee, &[handle]);
                    let results = builder.inst_results(call);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(results[0]);
                }
                "reset" => {
                    let fn_id = *cx.functions.get("__waker_reset").ok_or_else(|| {
                        CodegenError::Internal("__waker_reset not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                    builder.ins().call(callee, &[handle]);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
                }
                "clone" => {
                    let fn_id = *cx.functions.get("__waker_clone").ok_or_else(|| {
                        CodegenError::Internal("__waker_clone not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                    let call = builder.ins().call(callee, &[handle]);
                    let results = builder.inst_results(call);
                    cx.last_expr_type = Some(clif_types::pointer_type());
                    cx.last_waker_new = true;
                    return Ok(results[0]);
                }
                "drop" => {
                    let fn_id = *cx.functions.get("__waker_drop").ok_or_else(|| {
                        CodegenError::Internal("__waker_drop not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                    builder.ins().call(callee, &[handle]);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
                }
                _ => {
                    return Err(CodegenError::NotImplemented(format!(
                        "Waker method '{method}'"
                    )));
                }
            }
        }
    }

    // ── Timer methods ────────────────────────────────────────────────
    if let Some(ref name) = recv_name {
        if cx.timer_handles.contains(name) {
            let handle_var = *cx
                .var_map
                .get(name)
                .ok_or_else(|| CodegenError::UndefinedVariable(name.clone()))?;
            let handle = builder.use_var(handle_var);
            match method {
                "schedule" => {
                    // timer.schedule(millis, waker) → fj_rt_timer_schedule(timer, millis, waker)
                    if args.len() < 2 {
                        return Err(CodegenError::NotImplemented(
                            "Timer.schedule requires (millis, waker) arguments".into(),
                        ));
                    }
                    let millis = compile_expr(builder, cx, &args[0].value)?;
                    let waker = compile_expr(builder, cx, &args[1].value)?;
                    let fn_id = *cx.functions.get("__timer_schedule").ok_or_else(|| {
                        CodegenError::Internal("__timer_schedule not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                    let call = builder.ins().call(callee, &[handle, millis, waker]);
                    let results = builder.inst_results(call);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(results[0]);
                }
                "tick" => {
                    // timer.tick() → fj_rt_timer_tick(timer) → fired count
                    let fn_id = *cx.functions.get("__timer_tick").ok_or_else(|| {
                        CodegenError::Internal("__timer_tick not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                    let call = builder.ins().call(callee, &[handle]);
                    let results = builder.inst_results(call);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(results[0]);
                }
                "pending" => {
                    // timer.pending() → fj_rt_timer_pending(timer) → pending count
                    let fn_id = *cx.functions.get("__timer_pending").ok_or_else(|| {
                        CodegenError::Internal("__timer_pending not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                    let call = builder.ins().call(callee, &[handle]);
                    let results = builder.inst_results(call);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(results[0]);
                }
                "free" => {
                    // timer.free() → fj_rt_timer_free(timer)
                    let fn_id = *cx.functions.get("__timer_free").ok_or_else(|| {
                        CodegenError::Internal("__timer_free not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                    builder.ins().call(callee, &[handle]);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
                }
                _ => {
                    return Err(CodegenError::NotImplemented(format!(
                        "Timer method '{method}'"
                    )));
                }
            }
        }
    }

    // ── ThreadPool methods ────────────────────────────────────────────
    if let Some(ref name) = recv_name {
        if cx.threadpool_handles.contains(name) {
            let handle_var = *cx
                .var_map
                .get(name)
                .ok_or_else(|| CodegenError::UndefinedVariable(name.clone()))?;
            let handle = builder.use_var(handle_var);
            match method {
                "spawn" => {
                    // pool.spawn(future) → fj_rt_threadpool_spawn(pool, future)
                    if args.is_empty() {
                        return Err(CodegenError::NotImplemented(
                            "ThreadPool.spawn requires a future argument".into(),
                        ));
                    }
                    let future_ptr = compile_expr(builder, cx, &args[0].value)?;
                    let fn_id = *cx.functions.get("__threadpool_spawn").ok_or_else(|| {
                        CodegenError::Internal("__threadpool_spawn not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                    let call = builder.ins().call(callee, &[handle, future_ptr]);
                    let results = builder.inst_results(call);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(results[0]);
                }
                "run" => {
                    // pool.run() → fj_rt_threadpool_run(pool) → completed count
                    let fn_id = *cx.functions.get("__threadpool_run").ok_or_else(|| {
                        CodegenError::Internal("__threadpool_run not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                    let call = builder.ins().call(callee, &[handle]);
                    let results = builder.inst_results(call);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(results[0]);
                }
                "get_result" => {
                    // pool.get_result(i) → fj_rt_threadpool_get_result(pool, i)
                    if args.is_empty() {
                        return Err(CodegenError::NotImplemented(
                            "ThreadPool.get_result requires an index argument".into(),
                        ));
                    }
                    let index = compile_expr(builder, cx, &args[0].value)?;
                    let fn_id = *cx.functions.get("__threadpool_get_result").ok_or_else(|| {
                        CodegenError::Internal("__threadpool_get_result not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                    let call = builder.ins().call(callee, &[handle, index]);
                    let results = builder.inst_results(call);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(results[0]);
                }
                "thread_count" => {
                    let fn_id =
                        *cx.functions
                            .get("__threadpool_thread_count")
                            .ok_or_else(|| {
                                CodegenError::Internal(
                                    "__threadpool_thread_count not declared".into(),
                                )
                            })?;
                    let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                    let call = builder.ins().call(callee, &[handle]);
                    let results = builder.inst_results(call);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(results[0]);
                }
                "spawn_join" => {
                    // pool.spawn_join(future) → fj_rt_threadpool_spawn_join(pool, future) → JoinHandle ptr
                    if args.is_empty() {
                        return Err(CodegenError::NotImplemented(
                            "ThreadPool.spawn_join requires a future argument".into(),
                        ));
                    }
                    let future_ptr = compile_expr(builder, cx, &args[0].value)?;
                    let fn_id = *cx.functions.get("__threadpool_spawn_join").ok_or_else(|| {
                        CodegenError::Internal("__threadpool_spawn_join not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                    let call = builder.ins().call(callee, &[handle, future_ptr]);
                    let results = builder.inst_results(call);
                    cx.last_expr_type = Some(clif_types::pointer_type());
                    cx.last_joinhandle_new = true;
                    return Ok(results[0]);
                }
                "free" => {
                    let fn_id = *cx.functions.get("__threadpool_free").ok_or_else(|| {
                        CodegenError::Internal("__threadpool_free not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                    builder.ins().call(callee, &[handle]);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
                }
                _ => {
                    return Err(CodegenError::NotImplemented(format!(
                        "ThreadPool method '{method}'"
                    )));
                }
            }
        }
    }

    // ── JoinHandle methods ─────────────────────────────────────────────
    if let Some(ref name) = recv_name {
        if cx.joinhandle_handles.contains(name) {
            let handle_var = *cx
                .var_map
                .get(name)
                .ok_or_else(|| CodegenError::UndefinedVariable(name.clone()))?;
            let handle = builder.use_var(handle_var);
            match method {
                "get" => {
                    // jh.get() → fj_rt_joinhandle_get_result(jh) → i64 (blocks until ready)
                    let fn_id = *cx.functions.get("__joinhandle_get_result").ok_or_else(|| {
                        CodegenError::Internal("__joinhandle_get_result not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                    let call = builder.ins().call(callee, &[handle]);
                    let results = builder.inst_results(call);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(results[0]);
                }
                "is_ready" => {
                    let fn_id = *cx.functions.get("__joinhandle_is_ready").ok_or_else(|| {
                        CodegenError::Internal("__joinhandle_is_ready not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                    let call = builder.ins().call(callee, &[handle]);
                    let results = builder.inst_results(call);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(results[0]);
                }
                "abort" => {
                    let fn_id = *cx.functions.get("__joinhandle_abort").ok_or_else(|| {
                        CodegenError::Internal("__joinhandle_abort not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                    builder.ins().call(callee, &[handle]);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
                }
                "is_cancelled" => {
                    let fn_id =
                        *cx.functions
                            .get("__joinhandle_is_cancelled")
                            .ok_or_else(|| {
                                CodegenError::Internal(
                                    "__joinhandle_is_cancelled not declared".into(),
                                )
                            })?;
                    let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                    let call = builder.ins().call(callee, &[handle]);
                    let results = builder.inst_results(call);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(results[0]);
                }
                "free" => {
                    let fn_id = *cx.functions.get("__joinhandle_free").ok_or_else(|| {
                        CodegenError::Internal("__joinhandle_free not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                    builder.ins().call(callee, &[handle]);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
                }
                _ => {
                    return Err(CodegenError::NotImplemented(format!(
                        "JoinHandle method '{method}'"
                    )));
                }
            }
        }
    }

    // ── Async channel methods ──────────────────────────────────────────
    if let Some(ref name) = recv_name {
        if cx.async_channel_handles.contains(name) {
            let handle_var = *cx
                .var_map
                .get(name)
                .ok_or_else(|| CodegenError::UndefinedVariable(name.clone()))?;
            let handle = builder.use_var(handle_var);
            match method {
                "send" => {
                    if args.is_empty() {
                        return Err(CodegenError::NotImplemented(
                            "AsyncChannel.send requires a value argument".into(),
                        ));
                    }
                    let val = compile_expr(builder, cx, &args[0].value)?;
                    let fn_id = *cx.functions.get("__async_channel_send").ok_or_else(|| {
                        CodegenError::Internal("__async_channel_send not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                    let call = builder.ins().call(callee, &[handle, val]);
                    let results = builder.inst_results(call);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(results[0]);
                }
                "recv" => {
                    let fn_id = *cx.functions.get("__async_channel_recv").ok_or_else(|| {
                        CodegenError::Internal("__async_channel_recv not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                    let call = builder.ins().call(callee, &[handle]);
                    let results = builder.inst_results(call);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(results[0]);
                }
                "close" => {
                    let fn_id = *cx.functions.get("__async_channel_close").ok_or_else(|| {
                        CodegenError::Internal("__async_channel_close not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                    builder.ins().call(callee, &[handle]);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
                }
                "free" => {
                    let fn_id = *cx.functions.get("__async_channel_free").ok_or_else(|| {
                        CodegenError::Internal("__async_channel_free not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                    builder.ins().call(callee, &[handle]);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
                }
                _ => {
                    return Err(CodegenError::NotImplemented(format!(
                        "AsyncChannel method '{method}'"
                    )));
                }
            }
        }
    }

    // ── Async bounded channel methods ──────────────────────────────────
    if let Some(ref name) = recv_name {
        if cx.async_bchannel_handles.contains(name) {
            let handle_var = *cx
                .var_map
                .get(name)
                .ok_or_else(|| CodegenError::UndefinedVariable(name.clone()))?;
            let handle = builder.use_var(handle_var);
            match method {
                "send" => {
                    if args.is_empty() {
                        return Err(CodegenError::NotImplemented(
                            "AsyncBoundedChannel.send requires a value argument".into(),
                        ));
                    }
                    let val = compile_expr(builder, cx, &args[0].value)?;
                    let fn_id = *cx.functions.get("__async_bchannel_send").ok_or_else(|| {
                        CodegenError::Internal("__async_bchannel_send not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                    let call = builder.ins().call(callee, &[handle, val]);
                    let results = builder.inst_results(call);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(results[0]);
                }
                "recv" => {
                    let fn_id = *cx.functions.get("__async_bchannel_recv").ok_or_else(|| {
                        CodegenError::Internal("__async_bchannel_recv not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                    let call = builder.ins().call(callee, &[handle]);
                    let results = builder.inst_results(call);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(results[0]);
                }
                "close" => {
                    let fn_id = *cx.functions.get("__async_bchannel_close").ok_or_else(|| {
                        CodegenError::Internal("__async_bchannel_close not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                    builder.ins().call(callee, &[handle]);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
                }
                "free" => {
                    let fn_id = *cx.functions.get("__async_bchannel_free").ok_or_else(|| {
                        CodegenError::Internal("__async_bchannel_free not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                    builder.ins().call(callee, &[handle]);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
                }
                _ => {
                    return Err(CodegenError::NotImplemented(format!(
                        "AsyncBoundedChannel method '{method}'"
                    )));
                }
            }
        }
    }

    // ── Async I/O handle methods ──────────────────────────────────────
    if let Some(ref name) = recv_name {
        if cx.async_io_handles.contains(name) {
            let handle_var = *cx
                .var_map
                .get(name)
                .ok_or_else(|| CodegenError::UndefinedVariable(name.clone()))?;
            let handle = builder.use_var(handle_var);
            match method {
                "poll" => {
                    let fn_id = *cx.functions.get("__async_io_poll").ok_or_else(|| {
                        CodegenError::Internal("__async_io_poll not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                    let call = builder.ins().call(callee, &[handle]);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(builder.inst_results(call)[0]);
                }
                "status" => {
                    let fn_id = *cx.functions.get("__async_io_status").ok_or_else(|| {
                        CodegenError::Internal("__async_io_status not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                    let call = builder.ins().call(callee, &[handle]);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(builder.inst_results(call)[0]);
                }
                "result_ptr" => {
                    let fn_id = *cx.functions.get("__async_io_result_ptr").ok_or_else(|| {
                        CodegenError::Internal("__async_io_result_ptr not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                    let call = builder.ins().call(callee, &[handle]);
                    cx.last_expr_type = Some(clif_types::pointer_type());
                    return Ok(builder.inst_results(call)[0]);
                }
                "result_len" => {
                    let fn_id = *cx.functions.get("__async_io_result_len").ok_or_else(|| {
                        CodegenError::Internal("__async_io_result_len not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                    let call = builder.ins().call(callee, &[handle]);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    cx.last_string_len = Some(builder.inst_results(call)[0]);
                    return Ok(builder.inst_results(call)[0]);
                }
                "free" => {
                    let fn_id = *cx.functions.get("__async_io_free").ok_or_else(|| {
                        CodegenError::Internal("__async_io_free not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                    builder.ins().call(callee, &[handle]);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
                }
                _ => {
                    return Err(CodegenError::NotImplemented(format!(
                        "AsyncIo method '{method}'"
                    )));
                }
            }
        }
    }

    // ── ONNX model methods ─────────────────────────────────────────────
    if let Some(ref name) = recv_name {
        if cx.onnx_handles.contains(name) {
            let handle_var = *cx
                .var_map
                .get(name)
                .ok_or_else(|| CodegenError::UndefinedVariable(name.clone()))?;
            let handle = builder.use_var(handle_var);
            match method {
                "add_dense" => {
                    // add_dense(weight_tensor, bias_tensor, layer_idx)
                    if args.len() < 3 {
                        return Err(CodegenError::NotImplemented(
                            "OnnxModel.add_dense requires (weight, bias, layer_idx)".into(),
                        ));
                    }
                    let w = compile_expr(builder, cx, &args[0].value)?;
                    let b = compile_expr(builder, cx, &args[1].value)?;
                    let idx = compile_expr(builder, cx, &args[2].value)?;
                    let fn_id = *cx.functions.get("__onnx_add_dense").ok_or_else(|| {
                        CodegenError::Internal("__onnx_add_dense not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                    builder.ins().call(callee, &[handle, w, b, idx]);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
                }
                "add_relu" => {
                    let idx = if args.is_empty() {
                        builder.ins().iconst(clif_types::default_int_type(), 0)
                    } else {
                        compile_expr(builder, cx, &args[0].value)?
                    };
                    let fn_id = *cx.functions.get("__onnx_add_relu").ok_or_else(|| {
                        CodegenError::Internal("__onnx_add_relu not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                    builder.ins().call(callee, &[handle, idx]);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
                }
                "set_input" => {
                    if args.len() < 2 {
                        return Err(CodegenError::NotImplemented(
                            "OnnxModel.set_input requires (batch, features)".into(),
                        ));
                    }
                    let batch = compile_expr(builder, cx, &args[0].value)?;
                    let features = compile_expr(builder, cx, &args[1].value)?;
                    let fn_id = *cx.functions.get("__onnx_set_input").ok_or_else(|| {
                        CodegenError::Internal("__onnx_set_input not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                    builder.ins().call(callee, &[handle, batch, features]);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
                }
                "node_count" => {
                    let fn_id = *cx.functions.get("__onnx_node_count").ok_or_else(|| {
                        CodegenError::Internal("__onnx_node_count not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                    let call = builder.ins().call(callee, &[handle]);
                    let results = builder.inst_results(call);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(results[0]);
                }
                "initializer_count" => {
                    let fn_id = *cx
                        .functions
                        .get("__onnx_initializer_count")
                        .ok_or_else(|| {
                            CodegenError::Internal("__onnx_initializer_count not declared".into())
                        })?;
                    let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                    let call = builder.ins().call(callee, &[handle]);
                    let results = builder.inst_results(call);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(results[0]);
                }
                "free" => {
                    let fn_id = *cx
                        .functions
                        .get("__onnx_free")
                        .ok_or_else(|| CodegenError::Internal("__onnx_free not declared".into()))?;
                    let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                    builder.ins().call(callee, &[handle]);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
                }
                _ => {}
            }
        }
    }

    // ── SIMD f32x4 methods ─────────────────────────────────────────────
    if let Some(ref name) = recv_name {
        if cx.simd_f32x4_handles.contains(name) {
            let handle_var = *cx
                .var_map
                .get(name)
                .ok_or_else(|| CodegenError::UndefinedVariable(name.clone()))?;
            let handle = builder.use_var(handle_var);
            match method {
                "get" => {
                    let idx = if args.is_empty() {
                        builder.ins().iconst(clif_types::default_int_type(), 0)
                    } else {
                        compile_expr(builder, cx, &args[0].value)?
                    };
                    let fn_id = *cx.functions.get("__simd_f32x4_get").ok_or_else(|| {
                        CodegenError::Internal("__simd_f32x4_get not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                    let call = builder.ins().call(callee, &[handle, idx]);
                    let result_val = {
                        let results = builder.inst_results(call);
                        results[0]
                    };
                    // Runtime returns i64 (f64 bits) — bitcast to F64
                    let f_val = builder.ins().bitcast(
                        cranelift_codegen::ir::types::F64,
                        cranelift_codegen::ir::MemFlags::new(),
                        result_val,
                    );
                    cx.last_expr_type = Some(cranelift_codegen::ir::types::F64);
                    return Ok(f_val);
                }
                "add" | "sub" | "mul" | "div" => {
                    if args.is_empty() {
                        return Err(CodegenError::NotImplemented(format!(
                            "f32x4.{method} requires an argument"
                        )));
                    }
                    let other = compile_expr(builder, cx, &args[0].value)?;
                    let key = format!("__simd_f32x4_{method}");
                    let fn_id = *cx
                        .functions
                        .get(&key)
                        .ok_or_else(|| CodegenError::Internal(format!("{key} not declared")))?;
                    let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                    let call = builder.ins().call(callee, &[handle, other]);
                    let results = builder.inst_results(call);
                    cx.last_expr_type = Some(clif_types::pointer_type());
                    cx.last_simd_f32x4_new = true;
                    return Ok(results[0]);
                }
                "sum" | "min" | "max" => {
                    let key = format!("__simd_f32x4_{method}");
                    let fn_id = *cx
                        .functions
                        .get(&key)
                        .ok_or_else(|| CodegenError::Internal(format!("{key} not declared")))?;
                    let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                    let call = builder.ins().call(callee, &[handle]);
                    let result_val = {
                        let results = builder.inst_results(call);
                        results[0]
                    };
                    // Runtime returns i64 (f64 bits) — bitcast to F64
                    let f_val = builder.ins().bitcast(
                        cranelift_codegen::ir::types::F64,
                        cranelift_codegen::ir::MemFlags::new(),
                        result_val,
                    );
                    cx.last_expr_type = Some(cranelift_codegen::ir::types::F64);
                    return Ok(f_val);
                }
                "store" => {
                    if args.len() < 2 {
                        return Err(CodegenError::NotImplemented(
                            "f32x4.store requires (array, offset)".into(),
                        ));
                    }
                    let arr = compile_expr(builder, cx, &args[0].value)?;
                    let offset = compile_expr(builder, cx, &args[1].value)?;
                    let fn_id = *cx.functions.get("__simd_f32x4_store").ok_or_else(|| {
                        CodegenError::Internal("__simd_f32x4_store not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                    builder.ins().call(callee, &[handle, arr, offset]);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
                }
                "free" => {
                    let fn_id = *cx.functions.get("__simd_f32x4_free").ok_or_else(|| {
                        CodegenError::Internal("__simd_f32x4_free not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                    builder.ins().call(callee, &[handle]);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
                }
                _ => {}
            }
        }
    }

    // ── SIMD i32x4 methods ──────────────────────────────────────────────
    if let Some(ref name) = recv_name {
        if cx.simd_i32x4_handles.contains(name) {
            let handle_var = *cx
                .var_map
                .get(name)
                .ok_or_else(|| CodegenError::UndefinedVariable(name.clone()))?;
            let handle = builder.use_var(handle_var);
            match method {
                "get" => {
                    let idx = if args.is_empty() {
                        builder.ins().iconst(clif_types::default_int_type(), 0)
                    } else {
                        compile_expr(builder, cx, &args[0].value)?
                    };
                    let fn_id = *cx.functions.get("__simd_i32x4_get").ok_or_else(|| {
                        CodegenError::Internal("__simd_i32x4_get not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                    let call = builder.ins().call(callee, &[handle, idx]);
                    let results = builder.inst_results(call);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(results[0]);
                }
                "add" | "sub" | "mul" => {
                    if args.is_empty() {
                        return Err(CodegenError::NotImplemented(format!(
                            "i32x4.{method} requires an argument"
                        )));
                    }
                    let other = compile_expr(builder, cx, &args[0].value)?;
                    let key = format!("__simd_i32x4_{method}");
                    let fn_id = *cx
                        .functions
                        .get(&key)
                        .ok_or_else(|| CodegenError::Internal(format!("{key} not declared")))?;
                    let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                    let call = builder.ins().call(callee, &[handle, other]);
                    let results = builder.inst_results(call);
                    cx.last_expr_type = Some(clif_types::pointer_type());
                    cx.last_simd_i32x4_new = true;
                    return Ok(results[0]);
                }
                "sum" | "min" | "max" => {
                    let key = format!("__simd_i32x4_{method}");
                    let fn_id = *cx
                        .functions
                        .get(&key)
                        .ok_or_else(|| CodegenError::Internal(format!("{key} not declared")))?;
                    let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                    let call = builder.ins().call(callee, &[handle]);
                    let results = builder.inst_results(call);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(results[0]);
                }
                "store" => {
                    if args.len() < 2 {
                        return Err(CodegenError::NotImplemented(
                            "i32x4.store requires (array, offset)".into(),
                        ));
                    }
                    let arr = compile_expr(builder, cx, &args[0].value)?;
                    let offset = compile_expr(builder, cx, &args[1].value)?;
                    let fn_id = *cx.functions.get("__simd_i32x4_store").ok_or_else(|| {
                        CodegenError::Internal("__simd_i32x4_store not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                    builder.ins().call(callee, &[handle, arr, offset]);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
                }
                "free" => {
                    let fn_id = *cx.functions.get("__simd_i32x4_free").ok_or_else(|| {
                        CodegenError::Internal("__simd_i32x4_free not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                    builder.ins().call(callee, &[handle]);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
                }
                _ => {}
            }
        }
    }

    // ── SIMD f32x8 methods ──────────────────────────────────────────────
    if let Some(ref name) = recv_name {
        if cx.simd_f32x8_handles.contains(name) {
            let handle_var = *cx
                .var_map
                .get(name)
                .ok_or_else(|| CodegenError::UndefinedVariable(name.clone()))?;
            let handle = builder.use_var(handle_var);
            match method {
                "get" => {
                    let idx = if args.is_empty() {
                        builder.ins().iconst(clif_types::default_int_type(), 0)
                    } else {
                        compile_expr(builder, cx, &args[0].value)?
                    };
                    let fn_id = *cx.functions.get("__simd_f32x8_get").ok_or_else(|| {
                        CodegenError::Internal("__simd_f32x8_get not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                    let call = builder.ins().call(callee, &[handle, idx]);
                    let result_val = {
                        let results = builder.inst_results(call);
                        results[0]
                    };
                    let f_val = builder.ins().bitcast(
                        cranelift_codegen::ir::types::F64,
                        cranelift_codegen::ir::MemFlags::new(),
                        result_val,
                    );
                    cx.last_expr_type = Some(cranelift_codegen::ir::types::F64);
                    return Ok(f_val);
                }
                "add" | "mul" => {
                    if args.is_empty() {
                        return Err(CodegenError::NotImplemented(format!(
                            "f32x8.{method} requires an argument"
                        )));
                    }
                    let other = compile_expr(builder, cx, &args[0].value)?;
                    let key = format!("__simd_f32x8_{method}");
                    let fn_id = *cx
                        .functions
                        .get(&key)
                        .ok_or_else(|| CodegenError::Internal(format!("{key} not declared")))?;
                    let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                    let call = builder.ins().call(callee, &[handle, other]);
                    let results = builder.inst_results(call);
                    cx.last_expr_type = Some(clif_types::pointer_type());
                    cx.last_simd_f32x8_new = true;
                    return Ok(results[0]);
                }
                "sum" => {
                    let fn_id = *cx.functions.get("__simd_f32x8_sum").ok_or_else(|| {
                        CodegenError::Internal("__simd_f32x8_sum not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                    let call = builder.ins().call(callee, &[handle]);
                    let result_val = {
                        let results = builder.inst_results(call);
                        results[0]
                    };
                    let f_val = builder.ins().bitcast(
                        cranelift_codegen::ir::types::F64,
                        cranelift_codegen::ir::MemFlags::new(),
                        result_val,
                    );
                    cx.last_expr_type = Some(cranelift_codegen::ir::types::F64);
                    return Ok(f_val);
                }
                "free" => {
                    let fn_id = *cx.functions.get("__simd_f32x8_free").ok_or_else(|| {
                        CodegenError::Internal("__simd_f32x8_free not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                    builder.ins().call(callee, &[handle]);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
                }
                _ => {}
            }
        }
    }

    // ── SIMD i32x8 methods ──────────────────────────────────────────────
    if let Some(ref name) = recv_name {
        if cx.simd_i32x8_handles.contains(name) {
            let handle_var = *cx
                .var_map
                .get(name)
                .ok_or_else(|| CodegenError::UndefinedVariable(name.clone()))?;
            let handle = builder.use_var(handle_var);
            match method {
                "get" => {
                    let idx = if args.is_empty() {
                        builder.ins().iconst(clif_types::default_int_type(), 0)
                    } else {
                        compile_expr(builder, cx, &args[0].value)?
                    };
                    let fn_id = *cx.functions.get("__simd_i32x8_get").ok_or_else(|| {
                        CodegenError::Internal("__simd_i32x8_get not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                    let call = builder.ins().call(callee, &[handle, idx]);
                    let results = builder.inst_results(call);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(results[0]);
                }
                "add" | "mul" => {
                    if args.is_empty() {
                        return Err(CodegenError::NotImplemented(format!(
                            "i32x8.{method} requires an argument"
                        )));
                    }
                    let other = compile_expr(builder, cx, &args[0].value)?;
                    let key = format!("__simd_i32x8_{method}");
                    let fn_id = *cx
                        .functions
                        .get(&key)
                        .ok_or_else(|| CodegenError::Internal(format!("{key} not declared")))?;
                    let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                    let call = builder.ins().call(callee, &[handle, other]);
                    let results = builder.inst_results(call);
                    cx.last_expr_type = Some(clif_types::pointer_type());
                    cx.last_simd_i32x8_new = true;
                    return Ok(results[0]);
                }
                "sum" => {
                    let fn_id = *cx.functions.get("__simd_i32x8_sum").ok_or_else(|| {
                        CodegenError::Internal("__simd_i32x8_sum not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                    let call = builder.ins().call(callee, &[handle]);
                    let results = builder.inst_results(call);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(results[0]);
                }
                "free" => {
                    let fn_id = *cx.functions.get("__simd_i32x8_free").ok_or_else(|| {
                        CodegenError::Internal("__simd_i32x8_free not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                    builder.ins().call(callee, &[handle]);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
                }
                _ => {}
            }
        }
    }

    // ── Stream methods ──────────────────────────────────────────────────
    if let Some(ref name) = recv_name {
        if cx.stream_handles.contains(name) {
            let handle_var = *cx
                .var_map
                .get(name)
                .ok_or_else(|| CodegenError::UndefinedVariable(name.clone()))?;
            let handle = builder.use_var(handle_var);
            match method {
                "push" => {
                    if args.is_empty() {
                        return Err(CodegenError::NotImplemented(
                            "Stream.push requires a value argument".into(),
                        ));
                    }
                    let val = compile_expr(builder, cx, &args[0].value)?;
                    let fn_id = *cx.functions.get("__stream_push").ok_or_else(|| {
                        CodegenError::Internal("__stream_push not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                    builder.ins().call(callee, &[handle, val]);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
                }
                "next" => {
                    let fn_id = *cx.functions.get("__stream_next").ok_or_else(|| {
                        CodegenError::Internal("__stream_next not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                    let call = builder.ins().call(callee, &[handle]);
                    let results = builder.inst_results(call);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(results[0]);
                }
                "has_next" => {
                    let fn_id = *cx.functions.get("__stream_has_next").ok_or_else(|| {
                        CodegenError::Internal("__stream_has_next not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                    let call = builder.ins().call(callee, &[handle]);
                    let results = builder.inst_results(call);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(results[0]);
                }
                "sum" => {
                    let fn_id = *cx.functions.get("__stream_sum").ok_or_else(|| {
                        CodegenError::Internal("__stream_sum not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                    let call = builder.ins().call(callee, &[handle]);
                    let results = builder.inst_results(call);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(results[0]);
                }
                "count" => {
                    let fn_id = *cx.functions.get("__stream_count").ok_or_else(|| {
                        CodegenError::Internal("__stream_count not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                    let call = builder.ins().call(callee, &[handle]);
                    let results = builder.inst_results(call);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(results[0]);
                }
                "map" => {
                    if args.is_empty() {
                        return Err(CodegenError::NotImplemented(
                            "Stream.map requires a function argument".into(),
                        ));
                    }
                    let fn_ptr = compile_expr(builder, cx, &args[0].value)?;
                    let fn_id = *cx.functions.get("__stream_map").ok_or_else(|| {
                        CodegenError::Internal("__stream_map not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                    let call = builder.ins().call(callee, &[handle, fn_ptr]);
                    let results = builder.inst_results(call);
                    cx.last_expr_type = Some(clif_types::pointer_type());
                    cx.last_stream_new = true;
                    return Ok(results[0]);
                }
                "filter" => {
                    if args.is_empty() {
                        return Err(CodegenError::NotImplemented(
                            "Stream.filter requires a function argument".into(),
                        ));
                    }
                    let fn_ptr = compile_expr(builder, cx, &args[0].value)?;
                    let fn_id = *cx.functions.get("__stream_filter").ok_or_else(|| {
                        CodegenError::Internal("__stream_filter not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                    let call = builder.ins().call(callee, &[handle, fn_ptr]);
                    let results = builder.inst_results(call);
                    cx.last_expr_type = Some(clif_types::pointer_type());
                    cx.last_stream_new = true;
                    return Ok(results[0]);
                }
                "take" => {
                    if args.is_empty() {
                        return Err(CodegenError::NotImplemented(
                            "Stream.take requires a count argument".into(),
                        ));
                    }
                    let n = compile_expr(builder, cx, &args[0].value)?;
                    let fn_id = *cx.functions.get("__stream_take").ok_or_else(|| {
                        CodegenError::Internal("__stream_take not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                    let call = builder.ins().call(callee, &[handle, n]);
                    let results = builder.inst_results(call);
                    cx.last_expr_type = Some(clif_types::pointer_type());
                    cx.last_stream_new = true;
                    return Ok(results[0]);
                }
                "close" => {
                    let fn_id = *cx.functions.get("__stream_close").ok_or_else(|| {
                        CodegenError::Internal("__stream_close not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                    builder.ins().call(callee, &[handle]);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
                }
                "free" => {
                    let fn_id = *cx.functions.get("__stream_free").ok_or_else(|| {
                        CodegenError::Internal("__stream_free not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                    builder.ins().call(callee, &[handle]);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
                }
                _ => {
                    return Err(CodegenError::NotImplemented(format!(
                        "Stream method '{method}'"
                    )));
                }
            }
        }
    }

    // ── MmioRegion methods ─────────────────────────────────────────────
    if let Some(ref name) = recv_name {
        if let Some(&(base_var, size_var)) = cx.mmio_regions.get(name) {
            let base = builder.use_var(base_var);
            let size = builder.use_var(size_var);
            match method {
                "read_u32" => {
                    // read_u32(offset): bounds check, then volatile_read(base + offset)
                    if args.is_empty() {
                        return Err(CodegenError::NotImplemented(
                            "MmioRegion.read_u32 requires an offset argument".into(),
                        ));
                    }
                    let offset = compile_expr(builder, cx, &args[0].value)?;
                    // Bounds check: trap if offset >= size
                    let oob = builder
                        .ins()
                        .icmp(IntCC::UnsignedGreaterThanOrEqual, offset, size);
                    builder.ins().trapnz(
                        oob,
                        cranelift_codegen::ir::TrapCode::user(1).expect("valid trap"),
                    );
                    let addr = builder.ins().iadd(base, offset);
                    let fn_id = *cx.functions.get("__volatile_read").ok_or_else(|| {
                        CodegenError::Internal("__volatile_read not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                    let call = builder.ins().call(callee, &[addr]);
                    let results = builder.inst_results(call);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(results[0]);
                }
                "write_u32" => {
                    // write_u32(offset, value): bounds check, then volatile_write(base + offset, value)
                    if args.len() < 2 {
                        return Err(CodegenError::NotImplemented(
                            "MmioRegion.write_u32 requires offset and value arguments".into(),
                        ));
                    }
                    let offset = compile_expr(builder, cx, &args[0].value)?;
                    let val = compile_expr(builder, cx, &args[1].value)?;
                    // Bounds check: trap if offset >= size
                    let oob = builder
                        .ins()
                        .icmp(IntCC::UnsignedGreaterThanOrEqual, offset, size);
                    builder.ins().trapnz(
                        oob,
                        cranelift_codegen::ir::TrapCode::user(1).expect("valid trap"),
                    );
                    let addr = builder.ins().iadd(base, offset);
                    let fn_id = *cx.functions.get("__volatile_write").ok_or_else(|| {
                        CodegenError::Internal("__volatile_write not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                    builder.ins().call(callee, &[addr, val]);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
                }
                "base" => {
                    // Return the base address
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(base);
                }
                "size" => {
                    // Return the region size
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(size);
                }
                _ => {
                    return Err(CodegenError::NotImplemented(format!(
                        "MmioRegion method '{method}'"
                    )));
                }
            }
        }
    }

    // ── VolatilePtr methods ────────────────────────────────────────────
    if let Some(ref name) = recv_name {
        if cx.volatile_ptr_handles.contains(name) {
            let vp_var = *cx
                .var_map
                .get(name)
                .ok_or_else(|| CodegenError::UndefinedVariable(name.clone()))?;
            let addr = builder.use_var(vp_var);
            match method {
                "read" => {
                    let fn_id = *cx.functions.get("__volatile_read").ok_or_else(|| {
                        CodegenError::Internal("__volatile_read not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                    let call = builder.ins().call(callee, &[addr]);
                    let results = builder.inst_results(call);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(results[0]);
                }
                "write" => {
                    if args.is_empty() {
                        return Err(CodegenError::NotImplemented(
                            "VolatilePtr.write requires a value argument".into(),
                        ));
                    }
                    let val = compile_expr(builder, cx, &args[0].value)?;
                    let fn_id = *cx.functions.get("__volatile_write").ok_or_else(|| {
                        CodegenError::Internal("__volatile_write not declared".into())
                    })?;
                    let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                    builder.ins().call(callee, &[addr, val]);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
                }
                "update" => {
                    // read-modify-write: val = read(addr); write(addr, f(val))
                    // The function argument must be a simple function reference
                    if args.is_empty() {
                        return Err(CodegenError::NotImplemented(
                            "VolatilePtr.update requires a function argument".into(),
                        ));
                    }
                    // Read current value
                    let read_id = *cx.functions.get("__volatile_read").ok_or_else(|| {
                        CodegenError::Internal("__volatile_read not declared".into())
                    })?;
                    let read_callee = cx.module.declare_func_in_func(read_id, builder.func);
                    let read_call = builder.ins().call(read_callee, &[addr]);
                    let current = builder.inst_results(read_call)[0];
                    // Apply function: f(current)
                    let fn_arg = compile_expr(builder, cx, &args[0].value)?;
                    // fn_arg is a function address — call it indirectly
                    let mut sig = cranelift_codegen::ir::Signature::new(
                        cranelift_codegen::isa::CallConv::SystemV,
                    );
                    sig.params.push(cranelift_codegen::ir::AbiParam::new(
                        clif_types::default_int_type(),
                    ));
                    sig.returns.push(cranelift_codegen::ir::AbiParam::new(
                        clif_types::default_int_type(),
                    ));
                    let sig_ref = builder.import_signature(sig);
                    let call = builder.ins().call_indirect(sig_ref, fn_arg, &[current]);
                    let new_val = builder.inst_results(call)[0];
                    // Write back
                    let write_id = *cx.functions.get("__volatile_write").ok_or_else(|| {
                        CodegenError::Internal("__volatile_write not declared".into())
                    })?;
                    let write_callee = cx.module.declare_func_in_func(write_id, builder.func);
                    builder.ins().call(write_callee, &[addr, new_val]);
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
                }
                "addr" => {
                    // Return the raw address
                    cx.last_expr_type = Some(clif_types::default_int_type());
                    return Ok(addr);
                }
                _ => {
                    return Err(CodegenError::NotImplemented(format!(
                        "VolatilePtr method '{method}'"
                    )));
                }
            }
        }
    }

    // ── HashMap methods ───────────────────────────────────────────────
    if let Some(ref name) = recv_name {
        if cx.heap_maps.contains(name) {
            return compile_map_method(builder, cx, name, method, args);
        }
    }

    // ── Heap array methods ────────────────────────────────────────────
    if let Some(ref name) = recv_name {
        if cx.heap_arrays.contains(name) {
            return compile_heap_array_method(builder, cx, name, method, args);
        }
    }

    // ── Stack array methods ───────────────────────────────────────────
    if let Some(ref name) = recv_name {
        if cx.array_meta.contains_key(name) {
            // Methods that need args are handled here; rest go to compile_stack_array_method
            match method {
                "contains" => {
                    return compile_stack_array_contains(builder, cx, name, args);
                }
                "join" => {
                    return compile_stack_array_join(builder, cx, name, args);
                }
                _ => return compile_stack_array_method(builder, cx, name, method),
            }
        }
    }

    // ── Split result methods ──────────────────────────────────────────
    if let Some(ref name) = recv_name {
        if cx.split_vars.contains(name) && method == "len" {
            let arr_var = *cx
                .var_map
                .get(name)
                .ok_or_else(|| CodegenError::UndefinedVariable(name.clone()))?;
            let arr_ptr = builder.use_var(arr_var);
            let len_id = *cx
                .functions
                .get("__split_len")
                .ok_or_else(|| CodegenError::Internal("__split_len not declared".into()))?;
            let callee = cx.module.declare_func_in_func(len_id, builder.func);
            let call = builder.ins().call(callee, &[arr_ptr]);
            cx.last_expr_type = Some(clif_types::default_int_type());
            return Ok(builder.inst_results(call)[0]);
        }
    }

    // ── Struct impl methods ───────────────────────────────────────────
    if let Some(ref name) = recv_name {
        if let Some((slot, struct_name)) = cx.struct_slots.get(name).cloned() {
            let key = (struct_name.clone(), method.to_string());
            if let Some(mangled) = cx.impl_methods.get(&key).cloned() {
                let func_id = *cx
                    .functions
                    .get(&mangled)
                    .ok_or_else(|| CodegenError::UndefinedFunction(mangled.clone()))?;
                let local_callee = cx.module.declare_func_in_func(func_id, builder.func);

                // Pass `self` as pointer to struct's stack slot
                let self_ptr = builder
                    .ins()
                    .stack_addr(clif_types::pointer_type(), slot, 0);
                let mut call_args = vec![self_ptr];
                for a in args {
                    call_args.push(compile_expr(builder, cx, &a.value)?);
                }
                let call = builder.ins().call(local_callee, &call_args);
                let results: Vec<ClifValue> = builder.inst_results(call).to_vec();

                if let Some(&ret_ty) = cx.fn_return_types.get(&mangled) {
                    cx.last_expr_type = Some(ret_ty);
                }

                if results.is_empty() {
                    return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
                }

                // Handle string return
                if cx.fn_returns_string.contains(&mangled) && results.len() >= 2 {
                    cx.last_string_len = Some(results[1]);
                    // Cannot assume ownership — fn may return string literals
                    cx.last_string_owned = false;
                    cx.last_expr_type = Some(clif_types::pointer_type());
                }

                // Handle struct return
                if let Some(sname) = cx.fn_returns_struct.get(&mangled).cloned() {
                    if let Some(fields) = cx.struct_defs.get(&sname).cloned() {
                        let num_fields = fields.len();
                        let ret_slot = builder.create_sized_stack_slot(
                            cranelift_codegen::ir::StackSlotData::new(
                                cranelift_codegen::ir::StackSlotKind::ExplicitSlot,
                                (num_fields as u32) * 8,
                                0,
                            ),
                        );
                        for (i, _) in fields.iter().enumerate() {
                            if i < results.len() {
                                builder
                                    .ins()
                                    .stack_store(results[i], ret_slot, (i as i32) * 8);
                            }
                        }
                        cx.last_struct_init = Some((ret_slot, sname));
                        cx.last_expr_type = Some(clif_types::pointer_type());
                        let ptr = builder
                            .ins()
                            .stack_addr(clif_types::pointer_type(), ret_slot, 0);
                        return Ok(ptr);
                    }
                }

                return Ok(results[0]);
            }
        }
    }

    Err(CodegenError::NotImplemented(format!(
        "method call '.{method}()' on {:?}",
        recv_name
    )))
}

// ═══════════════════════════════════════════════════════════════════════
// HashMap method compilation
// ═══════════════════════════════════════════════════════════════════════

/// Compiles a method call on a HashMap variable.
fn compile_map_method<M: Module>(
    builder: &mut FunctionBuilder,
    cx: &mut CodegenCtx<'_, M>,
    map_name: &str,
    method: &str,
    args: &[CallArg],
) -> Result<ClifValue, CodegenError> {
    let map_var = *cx
        .var_map
        .get(map_name)
        .ok_or_else(|| CodegenError::UndefinedVariable(map_name.to_string()))?;
    let map_ptr = builder.use_var(map_var);

    match method {
        "insert" => {
            // map.insert("key", value) → fj_rt_map_insert_int(map, key_ptr, key_len, value)
            if args.len() < 2 {
                return Err(CodegenError::NotImplemented(
                    "map.insert requires 2 arguments".into(),
                ));
            }
            // Compile key (must be a string)
            let key_val = compile_expr(builder, cx, &args[0].value)?;
            let key_len = cx
                .last_string_len
                .take()
                .ok_or_else(|| CodegenError::NotImplemented("map key must be a string".into()))?;
            // Compile value
            let val = compile_expr(builder, cx, &args[1].value)?;
            let val_type = cx.last_expr_type.unwrap_or(clif_types::default_int_type());

            if clif_types::is_float(val_type) {
                let func_id = *cx.functions.get("__map_insert_float").ok_or_else(|| {
                    CodegenError::Internal("__map_insert_float not declared".into())
                })?;
                let local = cx.module.declare_func_in_func(func_id, builder.func);
                builder.ins().call(local, &[map_ptr, key_val, key_len, val]);
            } else if cx.string_lens.contains_key(
                if let Expr::Ident { name, .. } = &args[1].value {
                    name.as_str()
                } else {
                    ""
                },
            ) || cx.last_string_len.is_some()
            {
                // String value — track this map as containing strings
                cx.map_str_values.insert(map_name.to_string());
                let str_len = cx.last_string_len.take().unwrap_or(key_len);
                let func_id = *cx.functions.get("__map_insert_str").ok_or_else(|| {
                    CodegenError::Internal("__map_insert_str not declared".into())
                })?;
                let local = cx.module.declare_func_in_func(func_id, builder.func);
                builder
                    .ins()
                    .call(local, &[map_ptr, key_val, key_len, val, str_len]);
            } else {
                let func_id = *cx.functions.get("__map_insert_int").ok_or_else(|| {
                    CodegenError::Internal("__map_insert_int not declared".into())
                })?;
                let local = cx.module.declare_func_in_func(func_id, builder.func);
                builder.ins().call(local, &[map_ptr, key_val, key_len, val]);
            }
            cx.last_expr_type = Some(clif_types::default_int_type());
            Ok(builder.ins().iconst(clif_types::default_int_type(), 0))
        }
        "get" => {
            // map.get("key") → dispatch to string or int variant
            if args.is_empty() {
                return Err(CodegenError::NotImplemented(
                    "map.get requires 1 argument".into(),
                ));
            }
            let key_val = compile_expr(builder, cx, &args[0].value)?;
            let key_len = cx
                .last_string_len
                .take()
                .ok_or_else(|| CodegenError::NotImplemented("map key must be a string".into()))?;

            if cx.map_str_values.contains(map_name) {
                // String map: use out-param pattern
                let out_ptr_slot =
                    builder.create_sized_stack_slot(cranelift_codegen::ir::StackSlotData::new(
                        cranelift_codegen::ir::StackSlotKind::ExplicitSlot,
                        8,
                        0,
                    ));
                let out_len_slot =
                    builder.create_sized_stack_slot(cranelift_codegen::ir::StackSlotData::new(
                        cranelift_codegen::ir::StackSlotKind::ExplicitSlot,
                        8,
                        0,
                    ));
                let out_ptr_addr =
                    builder
                        .ins()
                        .stack_addr(clif_types::pointer_type(), out_ptr_slot, 0);
                let out_len_addr =
                    builder
                        .ins()
                        .stack_addr(clif_types::pointer_type(), out_len_slot, 0);

                let func_id = *cx
                    .functions
                    .get("__map_get_str")
                    .ok_or_else(|| CodegenError::Internal("__map_get_str not declared".into()))?;
                let local = cx.module.declare_func_in_func(func_id, builder.func);
                builder.ins().call(
                    local,
                    &[map_ptr, key_val, key_len, out_ptr_addr, out_len_addr],
                );

                let result_ptr = builder.ins().load(
                    clif_types::pointer_type(),
                    cranelift_codegen::ir::MemFlags::new(),
                    out_ptr_addr,
                    0,
                );
                let result_len = builder.ins().load(
                    clif_types::default_int_type(),
                    cranelift_codegen::ir::MemFlags::new(),
                    out_len_addr,
                    0,
                );
                cx.last_string_len = Some(result_len);
                cx.last_string_owned = false;
                cx.last_expr_type = Some(clif_types::pointer_type());
                Ok(result_ptr)
            } else {
                // Integer/float map: return i64 directly
                let func_id = *cx
                    .functions
                    .get("__map_get_int")
                    .ok_or_else(|| CodegenError::Internal("__map_get_int not declared".into()))?;
                let local = cx.module.declare_func_in_func(func_id, builder.func);
                let call = builder.ins().call(local, &[map_ptr, key_val, key_len]);
                let results = builder.inst_results(call);
                cx.last_expr_type = Some(clif_types::default_int_type());
                Ok(results[0])
            }
        }
        "contains_key" => {
            // map.contains_key("key") → fj_rt_map_contains(map, key_ptr, key_len) -> i64
            if args.is_empty() {
                return Err(CodegenError::NotImplemented(
                    "map.contains_key requires 1 argument".into(),
                ));
            }
            let key_val = compile_expr(builder, cx, &args[0].value)?;
            let key_len = cx
                .last_string_len
                .take()
                .ok_or_else(|| CodegenError::NotImplemented("map key must be a string".into()))?;
            let func_id = *cx
                .functions
                .get("__map_contains")
                .ok_or_else(|| CodegenError::Internal("__map_contains not declared".into()))?;
            let local = cx.module.declare_func_in_func(func_id, builder.func);
            let call = builder.ins().call(local, &[map_ptr, key_val, key_len]);
            let results = builder.inst_results(call);
            cx.last_expr_type = Some(clif_types::default_int_type());
            Ok(results[0])
        }
        "remove" => {
            // map.remove("key") → fj_rt_map_remove(map, key_ptr, key_len) -> i64
            if args.is_empty() {
                return Err(CodegenError::NotImplemented(
                    "map.remove requires 1 argument".into(),
                ));
            }
            let key_val = compile_expr(builder, cx, &args[0].value)?;
            let key_len = cx
                .last_string_len
                .take()
                .ok_or_else(|| CodegenError::NotImplemented("map key must be a string".into()))?;
            let func_id = *cx
                .functions
                .get("__map_remove")
                .ok_or_else(|| CodegenError::Internal("__map_remove not declared".into()))?;
            let local = cx.module.declare_func_in_func(func_id, builder.func);
            let call = builder.ins().call(local, &[map_ptr, key_val, key_len]);
            let results = builder.inst_results(call);
            cx.last_expr_type = Some(clif_types::default_int_type());
            Ok(results[0])
        }
        "len" => {
            // map.len() → fj_rt_map_len(map) -> i64
            let func_id = *cx
                .functions
                .get("__map_len")
                .ok_or_else(|| CodegenError::Internal("__map_len not declared".into()))?;
            let local = cx.module.declare_func_in_func(func_id, builder.func);
            let call = builder.ins().call(local, &[map_ptr]);
            let results = builder.inst_results(call);
            cx.last_expr_type = Some(clif_types::default_int_type());
            Ok(results[0])
        }
        "clear" => {
            // map.clear() → fj_rt_map_clear(map)
            let func_id = *cx
                .functions
                .get("__map_clear")
                .ok_or_else(|| CodegenError::Internal("__map_clear not declared".into()))?;
            let local = cx.module.declare_func_in_func(func_id, builder.func);
            builder.ins().call(local, &[map_ptr]);
            cx.last_expr_type = Some(clif_types::default_int_type());
            Ok(builder.ins().iconst(clif_types::default_int_type(), 0))
        }
        "values" => {
            // map.values() → fj_rt_map_values(map) → Box<Vec<i64>> heap array
            let func_id = *cx
                .functions
                .get("__map_values")
                .ok_or_else(|| CodegenError::Internal("__map_values not declared".into()))?;
            let local = cx.module.declare_func_in_func(func_id, builder.func);
            let call = builder.ins().call(local, &[map_ptr]);
            let arr_ptr = builder.inst_results(call)[0];
            cx.last_expr_type = Some(clif_types::default_int_type());
            cx.last_heap_array = true;
            Ok(arr_ptr)
        }
        "keys" => {
            // map.keys() → fj_rt_map_keys(map, &count) → Box<Vec<i64>> of (ptr, len) pairs
            // Compatible with fj_rt_split_len / fj_rt_split_get for iteration
            let count_slot =
                builder.create_sized_stack_slot(cranelift_codegen::ir::StackSlotData::new(
                    cranelift_codegen::ir::StackSlotKind::ExplicitSlot,
                    8,
                    3,
                ));
            let count_addr =
                builder
                    .ins()
                    .stack_addr(clif_types::default_int_type(), count_slot, 0);
            let func_id = *cx
                .functions
                .get("__map_keys")
                .ok_or_else(|| CodegenError::Internal("__map_keys not declared".into()))?;
            let local = cx.module.declare_func_in_func(func_id, builder.func);
            let call = builder.ins().call(local, &[map_ptr, count_addr]);
            let arr_ptr = builder.inst_results(call)[0];
            cx.last_expr_type = Some(clif_types::pointer_type());
            // Mark as split result so `let k = map.keys()` enters split_vars
            cx.last_split_result = Some(arr_ptr);
            Ok(arr_ptr)
        }
        _ => Err(CodegenError::NotImplemented(format!(
            "HashMap method '.{method}()'"
        ))),
    }
}

/// Compiles an inline assembly expression.
///
/// Supports known template patterns mapped to Cranelift IR, with operand handling:
/// - `in(reg) expr` / `in("specific") expr` — input value
/// - `out(reg) expr` — output variable
/// - `inout(reg) expr` — read + write to same variable
/// - `const expr` — compile-time constant
/// - `sym name` — function symbol address
pub(crate) fn compile_inline_asm<M: Module>(
    builder: &mut FunctionBuilder,
    cx: &mut CodegenCtx<'_, M>,
    template: &str,
    operands: &[crate::parser::ast::AsmOperand],
    _options: &[crate::parser::ast::AsmOption],
    _clobber_abi: &Option<String>,
) -> Result<ClifValue, CodegenError> {
    use crate::parser::ast::AsmOperand;
    let tmpl = template.trim();

    // No-operand templates
    if operands.is_empty() {
        match tmpl {
            "nop" => {
                builder.ins().nop();
                return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
            }
            "mfence" | "lfence" | "sfence" | "fence" => {
                builder.ins().fence();
                return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
            }
            _ => {}
        }
    }

    // Compile all input values and collect operand info.
    // Register allocation: operand constraints (reg, freg, specific registers like "rax")
    // are validated for type correctness. Since asm! is lowered to Cranelift IR operations,
    // Cranelift's register allocator handles the actual physical register assignment.
    // Specific register names in constraints serve as documentation/validation only.
    let mut input_vals: Vec<ClifValue> = Vec::new();
    let mut out_names: Vec<Option<String>> = Vec::new();
    let mut const_vals: Vec<i64> = Vec::new();
    let mut sym_names: Vec<String> = Vec::new();

    for op in operands {
        match op {
            AsmOperand::In { constraint, expr } => {
                let val = compile_expr(builder, cx, expr)?;
                validate_asm_operand_type(builder, constraint, val, "in")?;
                input_vals.push(val);
                out_names.push(None);
            }
            AsmOperand::Out { constraint, expr } | AsmOperand::LateOut { constraint, expr } => {
                let var_name = extract_ident_name(expr);
                if let Some(ref name) = var_name {
                    if let Some(&clif_ty) = cx.var_types.get(name) {
                        validate_asm_register_class(constraint, clif_ty, "out")?;
                    }
                }
                input_vals.push(builder.ins().iconst(clif_types::default_int_type(), 0));
                out_names.push(var_name);
            }
            AsmOperand::InOut { constraint, expr } => {
                let var_name = extract_ident_name(expr);
                let val = compile_expr(builder, cx, expr)?;
                validate_asm_operand_type(builder, constraint, val, "inout")?;
                input_vals.push(val);
                out_names.push(var_name);
            }
            AsmOperand::Const { expr } => {
                let val = compile_expr(builder, cx, expr)?;
                const_vals.push(0);
                if let crate::parser::ast::Expr::Literal {
                    kind: crate::parser::ast::LiteralKind::Int(n),
                    ..
                } = expr.as_ref()
                {
                    if let Some(last) = const_vals.last_mut() {
                        *last = *n;
                    }
                }
                input_vals.push(val);
                out_names.push(None);
            }
            AsmOperand::Sym { name } => {
                let func_id = cx
                    .functions
                    .get(name)
                    .ok_or_else(|| CodegenError::UndefinedFunction(name.clone()))?;
                let func_ref = cx.module.declare_func_in_func(*func_id, builder.func);
                let addr = builder
                    .ins()
                    .func_addr(clif_types::default_int_type(), func_ref);
                input_vals.push(addr);
                out_names.push(None);
                sym_names.push(name.clone());
            }
        }
    }

    // Clobber handling: when clobber_abi("C") is specified, the caller-saved registers
    // (rax, rcx, rdx, rsi, rdi, r8-r11, xmm0-xmm15 on x86_64) are considered clobbered.
    // Since we lower asm! to Cranelift IR operations, Cranelift's register allocator
    // automatically handles register pressure and spilling. The fence instruction ensures
    // that no reordering occurs across the asm block boundary.
    if _clobber_abi.is_some() {
        builder.ins().fence();
    }

    // Helper: write result to first output/inout operand variable
    let write_output = |builder: &mut FunctionBuilder, cx: &CodegenCtx<'_, M>, val: ClifValue| {
        for name in out_names.iter().flatten() {
            if let Some(&var) = cx.var_map.get(name) {
                builder.def_var(var, val);
                break;
            }
        }
    };

    // Extract the instruction mnemonic (first word of template)
    let mnemonic = tmpl.split_whitespace().next().unwrap_or(tmpl);

    let result = match mnemonic {
        // No-op and memory fences
        "nop" => {
            builder.ins().nop();
            builder.ins().iconst(clif_types::default_int_type(), 0)
        }
        "mfence" | "lfence" | "sfence" | "fence" => {
            builder.ins().fence();
            builder.ins().iconst(clif_types::default_int_type(), 0)
        }

        // Data movement
        "mov" => {
            if tmpl.contains("const") && !const_vals.is_empty() {
                let v = builder
                    .ins()
                    .iconst(clif_types::default_int_type(), const_vals[0]);
                write_output(builder, cx, v);
                v
            } else if input_vals.len() >= 2 {
                let v = input_vals[1];
                write_output(builder, cx, v);
                v
            } else {
                builder.ins().iconst(clif_types::default_int_type(), 0)
            }
        }
        "lea" => {
            let v = if input_vals.len() >= 2 {
                input_vals[1]
            } else if !input_vals.is_empty() {
                *input_vals.last().expect("at least one operand")
            } else {
                builder.ins().iconst(clif_types::default_int_type(), 0)
            };
            write_output(builder, cx, v);
            v
        }
        "xchg" => {
            // xchg {0}, {1} — swap two operands
            if input_vals.len() >= 2 {
                let a = input_vals[0];
                let b = input_vals[1];
                // Write b to first output, a to second output
                if let Some(Some(ref name)) = out_names.first() {
                    if let Some(&var) = cx.var_map.get(name) {
                        builder.def_var(var, b);
                    }
                }
                if let Some(Some(ref name)) = out_names.get(1) {
                    if let Some(&var) = cx.var_map.get(name) {
                        builder.def_var(var, a);
                    }
                }
                b
            } else {
                builder.ins().iconst(clif_types::default_int_type(), 0)
            }
        }

        // Arithmetic operations
        "add" => {
            if input_vals.len() >= 2 {
                let v = builder.ins().iadd(input_vals[0], input_vals[1]);
                write_output(builder, cx, v);
                v
            } else {
                builder.ins().iconst(clif_types::default_int_type(), 0)
            }
        }
        "sub" => {
            if input_vals.len() >= 2 {
                let v = builder.ins().isub(input_vals[0], input_vals[1]);
                write_output(builder, cx, v);
                v
            } else {
                builder.ins().iconst(clif_types::default_int_type(), 0)
            }
        }
        "imul" | "mul" => {
            if input_vals.len() >= 2 {
                let v = builder.ins().imul(input_vals[0], input_vals[1]);
                write_output(builder, cx, v);
                v
            } else {
                builder.ins().iconst(clif_types::default_int_type(), 0)
            }
        }
        "neg" => {
            if !input_vals.is_empty() {
                let v = builder.ins().ineg(input_vals[0]);
                write_output(builder, cx, v);
                v
            } else {
                builder.ins().iconst(clif_types::default_int_type(), 0)
            }
        }
        "inc" => {
            if !input_vals.is_empty() {
                let one = builder.ins().iconst(clif_types::default_int_type(), 1);
                let v = builder.ins().iadd(input_vals[0], one);
                write_output(builder, cx, v);
                v
            } else {
                builder.ins().iconst(clif_types::default_int_type(), 0)
            }
        }
        "dec" => {
            if !input_vals.is_empty() {
                let one = builder.ins().iconst(clif_types::default_int_type(), 1);
                let v = builder.ins().isub(input_vals[0], one);
                write_output(builder, cx, v);
                v
            } else {
                builder.ins().iconst(clif_types::default_int_type(), 0)
            }
        }

        // Bitwise operations
        "and" => {
            if input_vals.len() >= 2 {
                let v = builder.ins().band(input_vals[0], input_vals[1]);
                write_output(builder, cx, v);
                v
            } else {
                builder.ins().iconst(clif_types::default_int_type(), 0)
            }
        }
        "or" => {
            if input_vals.len() >= 2 {
                let v = builder.ins().bor(input_vals[0], input_vals[1]);
                write_output(builder, cx, v);
                v
            } else {
                builder.ins().iconst(clif_types::default_int_type(), 0)
            }
        }
        "xor" => {
            if input_vals.len() >= 2 {
                let v = builder.ins().bxor(input_vals[0], input_vals[1]);
                write_output(builder, cx, v);
                v
            } else {
                builder.ins().iconst(clif_types::default_int_type(), 0)
            }
        }
        "not" => {
            if !input_vals.is_empty() {
                let v = builder.ins().bnot(input_vals[0]);
                write_output(builder, cx, v);
                v
            } else {
                builder.ins().iconst(clif_types::default_int_type(), 0)
            }
        }
        "shl" | "sal" => {
            if input_vals.len() >= 2 {
                let v = builder.ins().ishl(input_vals[0], input_vals[1]);
                write_output(builder, cx, v);
                v
            } else {
                builder.ins().iconst(clif_types::default_int_type(), 0)
            }
        }
        "shr" => {
            if input_vals.len() >= 2 {
                let v = builder.ins().ushr(input_vals[0], input_vals[1]);
                write_output(builder, cx, v);
                v
            } else {
                builder.ins().iconst(clif_types::default_int_type(), 0)
            }
        }
        "sar" => {
            if input_vals.len() >= 2 {
                let v = builder.ins().sshr(input_vals[0], input_vals[1]);
                write_output(builder, cx, v);
                v
            } else {
                builder.ins().iconst(clif_types::default_int_type(), 0)
            }
        }
        "rol" => {
            if input_vals.len() >= 2 {
                let v = builder.ins().rotl(input_vals[0], input_vals[1]);
                write_output(builder, cx, v);
                v
            } else {
                builder.ins().iconst(clif_types::default_int_type(), 0)
            }
        }
        "ror" => {
            if input_vals.len() >= 2 {
                let v = builder.ins().rotr(input_vals[0], input_vals[1]);
                write_output(builder, cx, v);
                v
            } else {
                builder.ins().iconst(clif_types::default_int_type(), 0)
            }
        }

        // Comparison (sets output to 0 or 1)
        "cmp" => {
            // cmp {0}, {1} — compare and produce flags (result = a - b, for sete/setne etc.)
            if input_vals.len() >= 2 {
                let v = builder.ins().isub(input_vals[0], input_vals[1]);
                write_output(builder, cx, v);
                v
            } else {
                builder.ins().iconst(clif_types::default_int_type(), 0)
            }
        }
        "test" => {
            // test {0}, {1} — bitwise AND (for setz/setnz)
            if input_vals.len() >= 2 {
                let v = builder.ins().band(input_vals[0], input_vals[1]);
                write_output(builder, cx, v);
                v
            } else {
                builder.ins().iconst(clif_types::default_int_type(), 0)
            }
        }

        // Byte swap / count leading zeros
        "bswap" => {
            if !input_vals.is_empty() {
                let v = builder.ins().bswap(input_vals[0]);
                write_output(builder, cx, v);
                v
            } else {
                builder.ins().iconst(clif_types::default_int_type(), 0)
            }
        }
        "lzcnt" | "clz" => {
            if !input_vals.is_empty() {
                let v = builder.ins().clz(input_vals[0]);
                write_output(builder, cx, v);
                v
            } else {
                builder.ins().iconst(clif_types::default_int_type(), 0)
            }
        }
        "tzcnt" | "ctz" => {
            if !input_vals.is_empty() {
                let v = builder.ins().ctz(input_vals[0]);
                write_output(builder, cx, v);
                v
            } else {
                builder.ins().iconst(clif_types::default_int_type(), 0)
            }
        }
        "popcnt" => {
            if !input_vals.is_empty() {
                let v = builder.ins().popcnt(input_vals[0]);
                write_output(builder, cx, v);
                v
            } else {
                builder.ins().iconst(clif_types::default_int_type(), 0)
            }
        }

        // ARM64-specific instructions: route through assembly stubs for bare-metal AOT,
        // or encode as integer constants for JIT testing.
        _ if crate::codegen::aarch64_asm::is_arm64_specific(mnemonic) => {
            let operand_str = tmpl.strip_prefix(mnemonic).unwrap_or("").trim();
            let asm_operands: Vec<String> = if operand_str.is_empty() {
                vec![]
            } else {
                let mut result = Vec::new();
                let mut current = String::new();
                let mut bracket_depth = 0u32;
                for ch in operand_str.chars() {
                    match ch {
                        '[' => {
                            bracket_depth += 1;
                            current.push(ch);
                        }
                        ']' => {
                            bracket_depth = bracket_depth.saturating_sub(1);
                            current.push(ch);
                        }
                        ',' if bracket_depth == 0 => {
                            let trimmed = current.trim().to_string();
                            if !trimmed.is_empty() {
                                result.push(trimmed);
                            }
                            current.clear();
                        }
                        _ => current.push(ch),
                    }
                }
                let trimmed = current.trim().to_string();
                if !trimmed.is_empty() {
                    result.push(trimmed);
                }
                result
            };
            let asm_op_refs: Vec<&str> = asm_operands.iter().map(|s| s.as_str()).collect();

            // Validate encoding (always check syntax)
            let encoded = crate::codegen::aarch64_asm::encode_instruction(mnemonic, &asm_op_refs)?;

            // For bare-metal AOT: route mrs/msr/barriers through assembly stubs
            // that actually execute the instruction. For JIT: return encoded value.
            if cx.no_std {
                match mnemonic {
                    "mrs" if asm_operands.len() == 2 => {
                        // mrs Xd, SYSREG → call fj_rt_asm_mrs_<sysreg>() -> i64
                        let sysreg = asm_operands[1].to_lowercase().replace('.', "_");
                        let stub_name = format!("fj_rt_asm_mrs_{sysreg}");
                        let call_conv = cx.module.target_config().default_call_conv;
                        let mut sig = cranelift_codegen::ir::Signature::new(call_conv);
                        sig.returns.push(cranelift_codegen::ir::AbiParam::new(
                            clif_types::default_int_type(),
                        ));
                        let func_id = cx
                            .module
                            .declare_function(&stub_name, cranelift_module::Linkage::Import, &sig)
                            .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
                        let func_ref = cx.module.declare_func_in_func(func_id, builder.func);
                        let inst = builder.ins().call(func_ref, &[]);
                        let result = builder.inst_results(inst)[0];
                        write_output(builder, cx, result);
                        result
                    }
                    "msr" if asm_operands.len() == 2 && !input_vals.is_empty() => {
                        // msr SYSREG, Xt → call fj_rt_asm_msr_<sysreg>(val)
                        let sysreg = asm_operands[0].to_lowercase().replace('.', "_");
                        let stub_name = format!("fj_rt_asm_msr_{sysreg}");
                        let call_conv = cx.module.target_config().default_call_conv;
                        let mut sig = cranelift_codegen::ir::Signature::new(call_conv);
                        sig.params.push(cranelift_codegen::ir::AbiParam::new(
                            clif_types::default_int_type(),
                        ));
                        let func_id = cx
                            .module
                            .declare_function(&stub_name, cranelift_module::Linkage::Import, &sig)
                            .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
                        let func_ref = cx.module.declare_func_in_func(func_id, builder.func);
                        builder.ins().call(func_ref, &[input_vals[0]]);
                        builder.ins().iconst(clif_types::default_int_type(), 0)
                    }
                    "svc" | "isb" | "dsb" | "dmb" | "wfe" | "wfi" | "nop" | "eret" => {
                        // Side-effect instructions → call fj_rt_asm_<mnemonic>()
                        let stub_name = format!("fj_rt_asm_{mnemonic}");
                        let call_conv = cx.module.target_config().default_call_conv;
                        let sig = cranelift_codegen::ir::Signature::new(call_conv);
                        let func_id = cx
                            .module
                            .declare_function(&stub_name, cranelift_module::Linkage::Import, &sig)
                            .map_err(|e| CodegenError::FunctionError(e.to_string()))?;
                        let func_ref = cx.module.declare_func_in_func(func_id, builder.func);
                        builder.ins().call(func_ref, &[]);
                        builder.ins().iconst(clif_types::default_int_type(), 0)
                    }
                    _ => {
                        // Fallback: return encoded value
                        let val = builder
                            .ins()
                            .iconst(clif_types::default_int_type(), encoded as i64);
                        write_output(builder, cx, val);
                        val
                    }
                }
            } else {
                // JIT mode: return encoded instruction word for testing
                let val = builder
                    .ins()
                    .iconst(clif_types::default_int_type(), encoded as i64);
                write_output(builder, cx, val);
                val
            }
        }

        _ => {
            return Err(CodegenError::NotImplemented(format!(
                "inline assembly template not supported in codegen: \"{tmpl}\""
            )));
        }
    };

    // Post-asm clobber fence: ensures no reordering across asm boundary
    if _clobber_abi.is_some() {
        builder.ins().fence();
    }

    Ok(result)
}

/// Validates that an asm operand value's Cranelift type matches the declared register class.
///
/// - `"reg"` (general-purpose register): requires integer type, rejects float
/// - `"freg"` (floating-point register): requires float type, rejects integer
/// - Specific register names (e.g., `"rax"`, `"xmm0"`): validated by convention
/// - Empty or unrecognized constraints: accepted without restriction
fn validate_asm_operand_type(
    builder: &FunctionBuilder,
    constraint: &str,
    val: ClifValue,
    direction: &str,
) -> Result<(), CodegenError> {
    let val_ty = builder.func.dfg.value_type(val);
    validate_asm_register_class(constraint, val_ty, direction)
}

/// Validates a Cranelift type against an asm register class constraint string.
fn validate_asm_register_class(
    constraint: &str,
    val_ty: cranelift_codegen::ir::Type,
    direction: &str,
) -> Result<(), CodegenError> {
    let is_float = clif_types::is_float(val_ty);

    match constraint {
        // General-purpose integer register — reject float values
        "reg" => {
            if is_float {
                return Err(CodegenError::NotImplemented(format!(
                    "asm: float value in integer register ({direction}(reg) operand has type {val_ty})"
                )));
            }
        }
        // Floating-point register — reject integer values
        "freg" => {
            if !is_float {
                return Err(CodegenError::NotImplemented(format!(
                    "asm: integer value in float register ({direction}(freg) operand has type {val_ty})"
                )));
            }
        }
        // Specific x86 GP registers — reject float values
        c if matches!(
            c,
            "rax"
                | "rbx"
                | "rcx"
                | "rdx"
                | "rsi"
                | "rdi"
                | "rsp"
                | "rbp"
                | "eax"
                | "ebx"
                | "ecx"
                | "edx"
        ) =>
        {
            if is_float {
                return Err(CodegenError::NotImplemented(format!(
                    "asm: float value in integer register ({direction}(\"{c}\") operand has type {val_ty})"
                )));
            }
        }
        // Specific x86 SSE/AVX registers — reject integer values
        c if c.starts_with("xmm") || c.starts_with("ymm") || c.starts_with("zmm") => {
            if !is_float {
                return Err(CodegenError::NotImplemented(format!(
                    "asm: integer value in float register ({direction}(\"{c}\") operand has type {val_ty})"
                )));
            }
        }
        // ARM64 GP registers (x0-x30, w0-w30, sp, lr, xzr, wzr) — reject float values
        c if crate::codegen::aarch64_asm::reg_number(c).is_some() => {
            if is_float {
                return Err(CodegenError::NotImplemented(format!(
                    "asm: float value in ARM64 integer register ({direction}(\"{c}\") operand has type {val_ty})"
                )));
            }
        }
        // ARM64 NEON/FP registers (v0-v31, d0-d31, s0-s31) — reject integer values
        c if c.starts_with('v') || c.starts_with('d') || c.starts_with('s') => {
            let suffix = &c[1..];
            if let Ok(n) = suffix.parse::<u32>() {
                if n <= 31 && !is_float {
                    return Err(CodegenError::NotImplemented(format!(
                        "asm: integer value in ARM64 float register ({direction}(\"{c}\") operand has type {val_ty})"
                    )));
                }
            }
        }
        // All other constraints (empty, unknown, or platform-specific): accept any type
        _ => {}
    }
    Ok(())
}

/// Extracts the identifier name from an expression (for asm output operands).
fn extract_ident_name(expr: &crate::parser::ast::Expr) -> Option<String> {
    match expr {
        crate::parser::ast::Expr::Ident { name, .. } => Some(name.clone()),
        _ => None,
    }
}
