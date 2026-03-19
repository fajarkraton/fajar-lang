//! Function call compilation for Cranelift codegen.
//!
//! Contains: compile_call, compile_regular_call, compile_path_call,
//! compile_enum_constructor, compile_generic_call, infer_semantic_type,
//! compile_fn_ptr_call.

use cranelift_codegen::ir::condcodes::IntCC;
use cranelift_codegen::ir::{InstBuilder, Value as ClifValue};
use cranelift_frontend::FunctionBuilder;
use cranelift_module::Module;

use super::super::clif_types;
use super::super::context::{CodegenCtx, OwnedKind};
#[allow(unused_imports)]
use super::*;
use crate::codegen::CodegenError;
use crate::parser::ast::{CallArg, Expr, LiteralKind};

// ═══════════════════════════════════════════════════════════════════════
// @kernel context enforcement
// ═══════════════════════════════════════════════════════════════════════

/// Returns true if a builtin is forbidden in @kernel context.
///
/// @kernel functions cannot use heap allocation or tensor operations.
/// File I/O and string-allocating builtins are also blocked.
fn is_kernel_forbidden_builtin(name: &str) -> bool {
    matches!(
        name,
        // Tensor operations (heap-heavy)
        "tensor_zeros"
            | "tensor_ones"
            | "tensor_randn"
            | "tensor_xavier"
            | "tensor_from_data"
            | "tensor_eye"
            | "tensor_arange"
            | "tensor_linspace"
            | "tensor_matmul"
            | "tensor_add"
            | "tensor_sub"
            | "tensor_mul"
            | "tensor_div"
            | "tensor_relu"
            | "tensor_sigmoid"
            | "tensor_tanh"
            | "tensor_softmax"
            | "tensor_reshape"
            | "tensor_transpose"
            | "tensor_flatten"
            | "zeros"
            | "ones"
            | "randn"
            | "xavier"
            | "matmul"
            | "relu"
            | "sigmoid"
            | "softmax"
            // File I/O
            | "read_file"
            | "write_file"
            | "append_file"
            | "file_exists"
            | "async_read_file"
            | "async_write_file"
            // Heap-allocating string ops
            | "split"
            | "replace"
            | "repeat"
            | "to_uppercase"
            | "to_lowercase"
            | "format"
            // Optimizer/layer (heap-heavy ML)
            | "optimizer_sgd"
            | "optimizer_adam"
            | "layer_dense"
    )
}

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

    // ── @kernel context enforcement ───────────────────────────────────
    // Block heap-allocating and tensor builtins in @kernel functions.
    if cx.current_context.as_deref() == Some("kernel") && is_kernel_forbidden_builtin(&fn_name) {
        return Err(CodegenError::NotImplemented(format!(
            "[CE011] @kernel function cannot call `{fn_name}` (requires heap/tensor)"
        )));
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
            // fn_addr("function_name") → returns the link-time address of a function
            "fn_addr" => {
                if args.is_empty() {
                    return Err(CodegenError::NotImplemented(
                        "fn_addr requires 1 argument (function name as string)".into(),
                    ));
                }
                // Extract function name from identifier or string argument
                let target_name = match &args[0].value {
                    crate::parser::ast::Expr::Ident { name, .. } => name.clone(),
                    crate::parser::ast::Expr::Literal {
                        kind: crate::parser::ast::LiteralKind::String(s),
                        ..
                    } => s.clone(),
                    _ => {
                        return Err(CodegenError::NotImplemented(
                            "fn_addr argument must be a function name".into(),
                        ));
                    }
                };
                let func_id = *cx
                    .functions
                    .get(&target_name)
                    .ok_or_else(|| CodegenError::UndefinedFunction(target_name.clone()))?;
                let func_ref = cx.module.declare_func_in_func(func_id, builder.func);
                let addr = builder
                    .ins()
                    .func_addr(clif_types::default_int_type(), func_ref);
                cx.last_expr_type = Some(clif_types::default_int_type());
                return Ok(addr);
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
        // Exception: str_byte_at and str_len take raw pointer only (no length)
        if let Some(str_len) = cx.last_string_len.take() {
            if resolved_name != "str_byte_at" && resolved_name != "str_len" {
                call_args.push(str_len);
            }
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
