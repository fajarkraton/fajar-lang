//! Builtin function compilation for Fajar Lang.
//!
//! Handles println/print, math functions, assertions, format, file I/O,
//! wrapping/saturating/checked arithmetic, and other built-in operations.

use cranelift_codegen::ir::condcodes::IntCC;
use cranelift_codegen::ir::{InstBuilder, Value as ClifValue};
use cranelift_frontend::FunctionBuilder;
use cranelift_module::Module;

use super::super::clif_types;
use super::super::context::CodegenCtx;
use super::{compile_expr, compile_string_literal};
use crate::codegen::CodegenError;
use crate::parser::ast::{CallArg, Expr};

// Re-import control flow helpers needed by print_builtin
use super::control::{infer_expr_type, is_string_producing_expr};

// ═══════════════════════════════════════════════════════════════════════
// Print/debug builtins
// ═══════════════════════════════════════════════════════════════════════

/// Returns true if the given builtin is forbidden in no_std mode.
/// Note: println/print are allowed in bare-metal no_std (via fj_rt_bare_print).
fn is_io_builtin(name: &str) -> bool {
    matches!(
        name,
        "read_file"
            | "write_file"
            | "append_file"
            | "file_exists"
            | "async_read_file"
            | "async_write_file"
    )
}

/// Compiles println/print/eprintln/eprint with type-aware dispatch.
pub(in crate::codegen::cranelift) fn compile_print_builtin<M: Module>(
    builder: &mut FunctionBuilder,
    cx: &mut CodegenCtx<'_, M>,
    fn_name: &str,
    args: &[CallArg],
) -> Result<ClifValue, CodegenError> {
    if cx.no_std && is_io_builtin(fn_name) {
        return Err(CodegenError::NotImplemented(format!(
            "'{fn_name}' is not available in no_std mode"
        )));
    }
    if args.is_empty() {
        // println() with no args — print empty newline
        if fn_name == "println" || fn_name == "eprintln" {
            let empty_str = compile_string_literal(builder, cx, "")?;
            let len = cx
                .last_string_len
                .take()
                .unwrap_or_else(|| builder.ins().iconst(clif_types::default_int_type(), 0));
            let rt_fn = if fn_name == "println" {
                "__println_str"
            } else {
                "__eprintln_str"
            };
            let fid = *cx
                .functions
                .get(rt_fn)
                .ok_or_else(|| CodegenError::Internal(format!("{rt_fn} not declared")))?;
            let callee = cx.module.declare_func_in_func(fid, builder.func);
            builder.ins().call(callee, &[empty_str, len]);
        }
        cx.last_expr_type = Some(clif_types::default_int_type());
        return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
    }

    let arg_expr = &args[0].value;

    // Determine the type before compiling (for dispatch)
    let inferred_type = infer_expr_type(cx, arg_expr);
    let is_string_arg = is_string_producing_expr(arg_expr)
        || matches!(arg_expr, Expr::Ident { name, .. } if cx.string_lens.contains_key(name));

    let val = compile_expr(builder, cx, arg_expr)?;
    let val_type = cx.last_expr_type.unwrap_or(clif_types::default_int_type());

    let is_ln = fn_name == "println" || fn_name == "eprintln";
    let is_err = fn_name == "eprintln" || fn_name == "eprint";

    // String dispatch
    if is_string_arg || cx.last_string_len.is_some() {
        let len = cx
            .last_string_len
            .take()
            .unwrap_or_else(|| builder.ins().iconst(clif_types::default_int_type(), 0));
        let rt_fn = if is_err {
            if is_ln {
                "__eprintln_str"
            } else {
                "__eprint_str"
            }
        } else if is_ln {
            "__println_str"
        } else {
            "__print_str"
        };
        let fid = *cx
            .functions
            .get(rt_fn)
            .ok_or_else(|| CodegenError::Internal(format!("{rt_fn} not declared")))?;
        let callee = cx.module.declare_func_in_func(fid, builder.func);
        builder.ins().call(callee, &[val, len]);
    }
    // Float dispatch
    else if clif_types::is_float(val_type) || clif_types::is_float(inferred_type) {
        let float_val = if !clif_types::is_float(builder.func.dfg.value_type(val)) {
            builder
                .ins()
                .fcvt_from_sint(clif_types::default_float_type(), val)
        } else {
            val
        };
        let rt_fn = if is_err {
            "__eprintln_f64"
        } else if is_ln {
            "__println_f64"
        } else {
            "__print_f64"
        };
        let fid = *cx
            .functions
            .get(rt_fn)
            .ok_or_else(|| CodegenError::Internal(format!("{rt_fn} not declared")))?;
        let callee = cx.module.declare_func_in_func(fid, builder.func);
        builder.ins().call(callee, &[float_val]);
    }
    // Bool dispatch
    else if val_type == clif_types::bool_type() || inferred_type == clif_types::bool_type() {
        let int_val = if builder.func.dfg.value_type(val) == clif_types::bool_type() {
            builder.ins().uextend(clif_types::default_int_type(), val)
        } else {
            val
        };
        let rt_fn = if is_err {
            "__eprintln_bool"
        } else if is_ln {
            "__println_bool"
        } else {
            "__print_bool"
        };
        let fid = *cx
            .functions
            .get(rt_fn)
            .ok_or_else(|| CodegenError::Internal(format!("{rt_fn} not declared")))?;
        let callee = cx.module.declare_func_in_func(fid, builder.func);
        builder.ins().call(callee, &[int_val]);
    }
    // Integer dispatch (default)
    else {
        let rt_fn = if is_err {
            if is_ln {
                "__eprintln_i64"
            } else {
                "__eprint_i64"
            }
        } else if is_ln {
            "println"
        } else {
            "print"
        };
        let fid = *cx
            .functions
            .get(rt_fn)
            .ok_or_else(|| CodegenError::Internal(format!("{rt_fn} not declared")))?;
        let callee = cx.module.declare_func_in_func(fid, builder.func);
        builder.ins().call(callee, &[val]);
    }

    cx.last_expr_type = Some(clif_types::default_int_type());
    Ok(builder.ins().iconst(clif_types::default_int_type(), 0))
}

/// Compiles dbg(expr) — prints value with debug info.
pub(in crate::codegen::cranelift) fn compile_dbg_builtin<M: Module>(
    builder: &mut FunctionBuilder,
    cx: &mut CodegenCtx<'_, M>,
    args: &[CallArg],
) -> Result<ClifValue, CodegenError> {
    if args.is_empty() {
        cx.last_expr_type = Some(clif_types::default_int_type());
        return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
    }
    let arg_expr = &args[0].value;
    let is_string = is_string_producing_expr(arg_expr)
        || matches!(arg_expr, Expr::Ident { name, .. } if cx.string_lens.contains_key(name));

    let val = compile_expr(builder, cx, arg_expr)?;
    let val_type = cx.last_expr_type.unwrap_or(clif_types::default_int_type());

    if is_string || cx.last_string_len.is_some() {
        let len = cx
            .last_string_len
            .take()
            .unwrap_or_else(|| builder.ins().iconst(clif_types::default_int_type(), 0));
        let fid = *cx
            .functions
            .get("__dbg_str")
            .ok_or_else(|| CodegenError::Internal("__dbg_str not declared".into()))?;
        let callee = cx.module.declare_func_in_func(fid, builder.func);
        builder.ins().call(callee, &[val, len]);
    } else if clif_types::is_float(val_type) {
        let fid = *cx
            .functions
            .get("__dbg_f64")
            .ok_or_else(|| CodegenError::Internal("__dbg_f64 not declared".into()))?;
        let callee = cx.module.declare_func_in_func(fid, builder.func);
        builder.ins().call(callee, &[val]);
    } else {
        let fid = *cx
            .functions
            .get("__dbg_i64")
            .ok_or_else(|| CodegenError::Internal("__dbg_i64 not declared".into()))?;
        let callee = cx.module.declare_func_in_func(fid, builder.func);
        builder.ins().call(callee, &[val]);
    }

    // dbg returns its argument
    cx.last_expr_type = Some(val_type);
    Ok(val)
}

// ═══════════════════════════════════════════════════════════════════════
// Math builtins
// ═══════════════════════════════════════════════════════════════════════

/// Compiles abs/sqrt/floor/ceil/round.
pub(in crate::codegen::cranelift) fn compile_math_unary_builtin<M: Module>(
    builder: &mut FunctionBuilder,
    cx: &mut CodegenCtx<'_, M>,
    fn_name: &str,
    args: &[CallArg],
) -> Result<ClifValue, CodegenError> {
    if args.is_empty() {
        return Err(CodegenError::NotImplemented(format!(
            "{fn_name}() requires 1 argument"
        )));
    }
    let val = compile_expr(builder, cx, &args[0].value)?;
    let is_float = cx.last_expr_type.is_some_and(clif_types::is_float);

    match fn_name {
        "abs" => {
            if is_float {
                cx.last_expr_type = Some(clif_types::default_float_type());
                Ok(builder.ins().fabs(val))
            } else {
                // Integer abs: if val < 0 then -val else val
                let zero = builder.ins().iconst(clif_types::default_int_type(), 0);
                let neg = builder.ins().ineg(val);
                let cmp = builder.ins().icmp(IntCC::SignedLessThan, val, zero);
                let result = builder.ins().select(cmp, neg, val);
                cx.last_expr_type = Some(clif_types::default_int_type());
                Ok(result)
            }
        }
        "sqrt" => {
            let fval = if is_float {
                val
            } else {
                builder
                    .ins()
                    .fcvt_from_sint(clif_types::default_float_type(), val)
            };
            cx.last_expr_type = Some(clif_types::default_float_type());
            Ok(builder.ins().sqrt(fval))
        }
        "floor" => {
            let fval = if is_float {
                val
            } else {
                builder
                    .ins()
                    .fcvt_from_sint(clif_types::default_float_type(), val)
            };
            cx.last_expr_type = Some(clif_types::default_float_type());
            Ok(builder.ins().floor(fval))
        }
        "ceil" => {
            let fval = if is_float {
                val
            } else {
                builder
                    .ins()
                    .fcvt_from_sint(clif_types::default_float_type(), val)
            };
            cx.last_expr_type = Some(clif_types::default_float_type());
            Ok(builder.ins().ceil(fval))
        }
        "round" => {
            let fval = if is_float {
                val
            } else {
                builder
                    .ins()
                    .fcvt_from_sint(clif_types::default_float_type(), val)
            };
            cx.last_expr_type = Some(clif_types::default_float_type());
            Ok(builder.ins().nearest(fval))
        }
        _ => unreachable!(),
    }
}

/// Compiles sin/cos/tan/log/log2/log10 via runtime calls.
pub(in crate::codegen::cranelift) fn compile_math_rt_builtin<M: Module>(
    builder: &mut FunctionBuilder,
    cx: &mut CodegenCtx<'_, M>,
    fn_name: &str,
    args: &[CallArg],
) -> Result<ClifValue, CodegenError> {
    if args.is_empty() {
        return Err(CodegenError::NotImplemented(format!(
            "{fn_name}() requires 1 argument"
        )));
    }
    let val = compile_expr(builder, cx, &args[0].value)?;
    let is_float = cx.last_expr_type.is_some_and(clif_types::is_float);
    let fval = if is_float {
        val
    } else {
        builder
            .ins()
            .fcvt_from_sint(clif_types::default_float_type(), val)
    };

    let rt_name = format!("__math_{fn_name}");
    let fid = *cx
        .functions
        .get(&rt_name)
        .ok_or_else(|| CodegenError::Internal(format!("{rt_name} not declared")))?;
    let callee = cx.module.declare_func_in_func(fid, builder.func);
    let call = builder.ins().call(callee, &[fval]);
    cx.last_expr_type = Some(clif_types::default_float_type());
    Ok(builder.inst_results(call)[0])
}

/// Compiles pow(base, exp).
pub(in crate::codegen::cranelift) fn compile_pow_builtin<M: Module>(
    builder: &mut FunctionBuilder,
    cx: &mut CodegenCtx<'_, M>,
    args: &[CallArg],
) -> Result<ClifValue, CodegenError> {
    if args.len() < 2 {
        return Err(CodegenError::NotImplemented(
            "pow() requires 2 arguments".into(),
        ));
    }
    let base = compile_expr(builder, cx, &args[0].value)?;
    let base_float = cx.last_expr_type.is_some_and(clif_types::is_float);
    let exp = compile_expr(builder, cx, &args[1].value)?;
    let exp_float = cx.last_expr_type.is_some_and(clif_types::is_float);

    let bf = if base_float {
        base
    } else {
        builder
            .ins()
            .fcvt_from_sint(clif_types::default_float_type(), base)
    };
    let ef = if exp_float {
        exp
    } else {
        builder
            .ins()
            .fcvt_from_sint(clif_types::default_float_type(), exp)
    };

    let fid = *cx
        .functions
        .get("__math_pow")
        .ok_or_else(|| CodegenError::Internal("__math_pow not declared".into()))?;
    let callee = cx.module.declare_func_in_func(fid, builder.func);
    let call = builder.ins().call(callee, &[bf, ef]);
    cx.last_expr_type = Some(clif_types::default_float_type());
    Ok(builder.inst_results(call)[0])
}

/// Compiles min(a, b) / max(a, b).
pub(in crate::codegen::cranelift) fn compile_min_max_builtin<M: Module>(
    builder: &mut FunctionBuilder,
    cx: &mut CodegenCtx<'_, M>,
    fn_name: &str,
    args: &[CallArg],
) -> Result<ClifValue, CodegenError> {
    if args.len() < 2 {
        return Err(CodegenError::NotImplemented(format!(
            "{fn_name}() requires 2 arguments"
        )));
    }
    let a = compile_expr(builder, cx, &args[0].value)?;
    let a_float = cx.last_expr_type.is_some_and(clif_types::is_float);
    let b = compile_expr(builder, cx, &args[1].value)?;
    let b_float = cx.last_expr_type.is_some_and(clif_types::is_float);

    if a_float || b_float {
        let af = if a_float {
            a
        } else {
            builder
                .ins()
                .fcvt_from_sint(clif_types::default_float_type(), a)
        };
        let bf = if b_float {
            b
        } else {
            builder
                .ins()
                .fcvt_from_sint(clif_types::default_float_type(), b)
        };
        cx.last_expr_type = Some(clif_types::default_float_type());
        if fn_name == "min" {
            Ok(builder.ins().fmin(af, bf))
        } else {
            Ok(builder.ins().fmax(af, bf))
        }
    } else {
        let cc = if fn_name == "min" {
            IntCC::SignedLessThan
        } else {
            IntCC::SignedGreaterThan
        };
        let cmp = builder.ins().icmp(cc, a, b);
        let result = builder.ins().select(cmp, a, b);
        cx.last_expr_type = Some(clif_types::default_int_type());
        Ok(result)
    }
}

/// Compiles clamp(val, min, max).
pub(in crate::codegen::cranelift) fn compile_clamp_builtin<M: Module>(
    builder: &mut FunctionBuilder,
    cx: &mut CodegenCtx<'_, M>,
    args: &[CallArg],
) -> Result<ClifValue, CodegenError> {
    if args.len() < 3 {
        return Err(CodegenError::NotImplemented(
            "clamp() requires 3 arguments".into(),
        ));
    }
    let val = compile_expr(builder, cx, &args[0].value)?;
    let is_float = cx.last_expr_type.is_some_and(clif_types::is_float);
    let lo = compile_expr(builder, cx, &args[1].value)?;
    let hi = compile_expr(builder, cx, &args[2].value)?;

    if is_float {
        let clamped_lo = builder.ins().fmax(val, lo);
        let result = builder.ins().fmin(clamped_lo, hi);
        cx.last_expr_type = Some(clif_types::default_float_type());
        Ok(result)
    } else {
        let cmp_lo = builder.ins().icmp(IntCC::SignedLessThan, val, lo);
        let step1 = builder.ins().select(cmp_lo, lo, val);
        let cmp_hi = builder.ins().icmp(IntCC::SignedGreaterThan, step1, hi);
        let result = builder.ins().select(cmp_hi, hi, step1);
        cx.last_expr_type = Some(clif_types::default_int_type());
        Ok(result)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Conversion / utility builtins
// ═══════════════════════════════════════════════════════════════════════

/// Compiles len(x) — string length, array length, or heap array length.
pub(in crate::codegen::cranelift) fn compile_len_builtin<M: Module>(
    builder: &mut FunctionBuilder,
    cx: &mut CodegenCtx<'_, M>,
    args: &[CallArg],
) -> Result<ClifValue, CodegenError> {
    if args.is_empty() {
        return Err(CodegenError::NotImplemented(
            "len() requires 1 argument".into(),
        ));
    }
    let arg_expr = &args[0].value;

    if let Expr::Ident { name, .. } = arg_expr {
        // String length
        if let Some(&len_var) = cx.string_lens.get(name) {
            cx.last_expr_type = Some(clif_types::default_int_type());
            return Ok(builder.use_var(len_var));
        }
        // Stack array length
        if let Some((_, len)) = cx.array_meta.get(name) {
            cx.last_expr_type = Some(clif_types::default_int_type());
            return Ok(builder
                .ins()
                .iconst(clif_types::default_int_type(), *len as i64));
        }
        // Heap array length via runtime
        if cx.heap_arrays.contains(name) {
            let arr_var = *cx
                .var_map
                .get(name)
                .ok_or_else(|| CodegenError::UndefinedVariable(name.clone()))?;
            let arr_ptr = builder.use_var(arr_var);
            let len_id = *cx
                .functions
                .get("__array_len")
                .ok_or_else(|| CodegenError::Internal("__array_len not declared".into()))?;
            let callee = cx.module.declare_func_in_func(len_id, builder.func);
            let call = builder.ins().call(callee, &[arr_ptr]);
            cx.last_expr_type = Some(clif_types::default_int_type());
            return Ok(builder.inst_results(call)[0]);
        }
        // Split result length
        if cx.split_vars.contains(name) {
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

    // Fallback: compile and return 0
    let _ = compile_expr(builder, cx, arg_expr)?;
    cx.last_expr_type = Some(clif_types::default_int_type());
    Ok(builder.ins().iconst(clif_types::default_int_type(), 0))
}

/// Compiles to_string(val).
pub(in crate::codegen::cranelift) fn compile_to_string_builtin<M: Module>(
    builder: &mut FunctionBuilder,
    cx: &mut CodegenCtx<'_, M>,
    args: &[CallArg],
) -> Result<ClifValue, CodegenError> {
    if args.is_empty() {
        return Err(CodegenError::NotImplemented(
            "to_string() requires 1 argument".into(),
        ));
    }
    let val = compile_expr(builder, cx, &args[0].value)?;
    let val_type = cx.last_expr_type.unwrap_or(clif_types::default_int_type());

    // Already a string
    if cx.last_string_len.is_some() {
        cx.last_expr_type = Some(clif_types::pointer_type());
        return Ok(val);
    }

    // Stack slots for output
    let out_ptr_slot = builder.create_sized_stack_slot(cranelift_codegen::ir::StackSlotData::new(
        cranelift_codegen::ir::StackSlotKind::ExplicitSlot,
        8,
        0,
    ));
    let out_len_slot = builder.create_sized_stack_slot(cranelift_codegen::ir::StackSlotData::new(
        cranelift_codegen::ir::StackSlotKind::ExplicitSlot,
        8,
        0,
    ));
    let out_ptr_addr = builder
        .ins()
        .stack_addr(clif_types::pointer_type(), out_ptr_slot, 0);
    let out_len_addr = builder
        .ins()
        .stack_addr(clif_types::pointer_type(), out_len_slot, 0);

    let rt_fn = if clif_types::is_float(val_type) {
        "__float_to_string"
    } else {
        "__int_to_string"
    };
    let fid = *cx
        .functions
        .get(rt_fn)
        .ok_or_else(|| CodegenError::Internal(format!("{rt_fn} not declared")))?;
    let callee = cx.module.declare_func_in_func(fid, builder.func);
    builder
        .ins()
        .call(callee, &[val, out_ptr_addr, out_len_addr]);

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
    cx.last_string_owned = true;
    cx.last_expr_type = Some(clif_types::pointer_type());
    Ok(result_ptr)
}

/// Compiles to_int(val) / to_float(val).
pub(in crate::codegen::cranelift) fn compile_convert_builtin<M: Module>(
    builder: &mut FunctionBuilder,
    cx: &mut CodegenCtx<'_, M>,
    fn_name: &str,
    args: &[CallArg],
) -> Result<ClifValue, CodegenError> {
    if args.is_empty() {
        return Err(CodegenError::NotImplemented(format!(
            "{fn_name}() requires 1 argument"
        )));
    }
    let val = compile_expr(builder, cx, &args[0].value)?;
    let is_float = cx.last_expr_type.is_some_and(clif_types::is_float);

    if fn_name == "to_float" {
        if is_float {
            cx.last_expr_type = Some(clif_types::default_float_type());
            Ok(val)
        } else {
            let result = builder
                .ins()
                .fcvt_from_sint(clif_types::default_float_type(), val);
            cx.last_expr_type = Some(clif_types::default_float_type());
            Ok(result)
        }
    } else {
        // to_int
        if is_float {
            let result = builder
                .ins()
                .fcvt_to_sint(clif_types::default_int_type(), val);
            cx.last_expr_type = Some(clif_types::default_int_type());
            Ok(result)
        } else {
            cx.last_expr_type = Some(clif_types::default_int_type());
            Ok(val)
        }
    }
}

/// Compiles type_of(expr) — returns a string.
pub(in crate::codegen::cranelift) fn compile_type_of_builtin<M: Module>(
    builder: &mut FunctionBuilder,
    cx: &mut CodegenCtx<'_, M>,
    args: &[CallArg],
) -> Result<ClifValue, CodegenError> {
    if args.is_empty() {
        return compile_string_literal(builder, cx, "void");
    }
    let arg_expr = &args[0].value;
    let is_string = is_string_producing_expr(arg_expr)
        || matches!(arg_expr, Expr::Ident { name, .. } if cx.string_lens.contains_key(name));
    let inferred = infer_expr_type(cx, arg_expr);

    let type_name = if is_string {
        "str"
    } else if clif_types::is_float(inferred) {
        "f64"
    } else if inferred == clif_types::bool_type() {
        "bool"
    } else {
        "i64"
    };

    // Compile the arg for side effects but discard
    let _ = compile_expr(builder, cx, arg_expr)?;
    cx.last_string_len = None;

    compile_string_literal(builder, cx, type_name)
}

/// Compiles assert(condition).
pub(in crate::codegen::cranelift) fn compile_assert_builtin<M: Module>(
    builder: &mut FunctionBuilder,
    cx: &mut CodegenCtx<'_, M>,
    args: &[CallArg],
) -> Result<ClifValue, CodegenError> {
    if args.is_empty() {
        return Err(CodegenError::NotImplemented(
            "assert() requires 1 argument".into(),
        ));
    }
    let val = compile_expr(builder, cx, &args[0].value)?;
    // Widen bool if needed
    let val_wide = if builder.func.dfg.value_type(val) == clif_types::bool_type() {
        builder.ins().uextend(clif_types::default_int_type(), val)
    } else {
        val
    };

    let ok_block = builder.create_block();
    let fail_block = builder.create_block();
    builder.ins().brif(val_wide, ok_block, &[], fail_block, &[]);

    builder.switch_to_block(fail_block);
    builder.seal_block(fail_block);
    builder
        .ins()
        .trap(cranelift_codegen::ir::TrapCode::user(1).expect("trap code 1 is valid"));

    builder.switch_to_block(ok_block);
    builder.seal_block(ok_block);
    cx.last_expr_type = Some(clif_types::default_int_type());
    Ok(builder.ins().iconst(clif_types::default_int_type(), 0))
}

/// Compiles assert_eq(a, b).
pub(in crate::codegen::cranelift) fn compile_assert_eq_builtin<M: Module>(
    builder: &mut FunctionBuilder,
    cx: &mut CodegenCtx<'_, M>,
    args: &[CallArg],
) -> Result<ClifValue, CodegenError> {
    if args.len() < 2 {
        return Err(CodegenError::NotImplemented(
            "assert_eq() requires 2 arguments".into(),
        ));
    }
    let a = compile_expr(builder, cx, &args[0].value)?;
    let a_type = cx.last_expr_type;
    let a_str_len = cx.last_string_len.take();
    let b = compile_expr(builder, cx, &args[1].value)?;
    let b_str_len = cx.last_string_len.take();

    // String comparison: use runtime fj_rt_str_eq(a_ptr, a_len, b_ptr, b_len) -> i64
    if let (Some(a_len), Some(b_len)) = (a_str_len, b_str_len) {
        if let Some(&str_eq_id) = cx.functions.get("__str_eq") {
            let str_eq_ref = cx.module.declare_func_in_func(str_eq_id, builder.func);
            let call = builder.ins().call(str_eq_ref, &[a, a_len, b, b_len]);
            let eq_result = builder.inst_results(call)[0];
            // str_eq returns 1 if equal, 0 if not
            let zero = builder.ins().iconst(clif_types::default_int_type(), 0);
            let not_equal = builder.ins().icmp(IntCC::Equal, eq_result, zero);
            builder.ins().trapnz(
                not_equal,
                cranelift_codegen::ir::TrapCode::user(1).expect("trap code 1 is valid"),
            );
            cx.last_expr_type = Some(clif_types::default_int_type());
            return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
        }
    }

    // Integer/float comparison
    let a_wide = if builder.func.dfg.value_type(a) == clif_types::bool_type() {
        builder.ins().uextend(clif_types::default_int_type(), a)
    } else {
        a
    };
    let b_wide = if builder.func.dfg.value_type(b) == clif_types::bool_type() {
        builder.ins().uextend(clif_types::default_int_type(), b)
    } else {
        b
    };

    let is_float = a_type.is_some_and(clif_types::is_float);
    let not_equal = if is_float {
        builder.ins().fcmp(
            cranelift_codegen::ir::condcodes::FloatCC::NotEqual,
            a_wide,
            b_wide,
        )
    } else {
        builder.ins().icmp(IntCC::NotEqual, a_wide, b_wide)
    };

    // Use trapnz: trap if values are NOT equal (no extra blocks needed)
    builder.ins().trapnz(
        not_equal,
        cranelift_codegen::ir::TrapCode::user(1).expect("trap code 1 is valid"),
    );

    cx.last_expr_type = Some(clif_types::default_int_type());
    Ok(builder.ins().iconst(clif_types::default_int_type(), 0))
}

/// Compiles panic(msg) / todo(msg).
pub(in crate::codegen::cranelift) fn compile_panic_builtin<M: Module>(
    builder: &mut FunctionBuilder,
    cx: &mut CodegenCtx<'_, M>,
    fn_name: &str,
    args: &[CallArg],
) -> Result<ClifValue, CodegenError> {
    // If a @panic_handler is set, call the user's function instead of default behavior
    if let Some(ref handler_name) = cx.panic_handler_fn.clone() {
        if let Some(&handler_id) = cx.functions.get(handler_name) {
            // Pass a panic code: 0 for default, or first arg if provided
            let code = if !args.is_empty() {
                compile_expr(builder, cx, &args[0].value)?
            } else {
                builder.ins().iconst(clif_types::default_int_type(), 0)
            };
            let callee = cx.module.declare_func_in_func(handler_id, builder.func);
            builder.ins().call(callee, &[code]);
        }
    } else if !cx.no_std {
        // Default behavior: print message
        if !args.is_empty() {
            compile_print_builtin(builder, cx, "eprintln", args)?;
        } else {
            let msg = if fn_name == "todo" {
                "not yet implemented"
            } else {
                "panic!"
            };
            let str_val = compile_string_literal(builder, cx, msg)?;
            let len = cx
                .last_string_len
                .take()
                .unwrap_or_else(|| builder.ins().iconst(clif_types::default_int_type(), 0));
            let fid = *cx
                .functions
                .get("__eprintln_str")
                .ok_or_else(|| CodegenError::Internal("__eprintln_str not declared".into()))?;
            let callee = cx.module.declare_func_in_func(fid, builder.func);
            builder.ins().call(callee, &[str_val, len]);
        }
    }

    builder
        .ins()
        .trap(cranelift_codegen::ir::TrapCode::user(1).expect("trap code 1 is valid"));

    // Unreachable after trap, but need a valid block
    let after = builder.create_block();
    builder.switch_to_block(after);
    builder.seal_block(after);
    cx.last_expr_type = Some(clif_types::default_int_type());
    Ok(builder.ins().iconst(clif_types::default_int_type(), 0))
}

/// Compiles format(template, args...).
pub(in crate::codegen::cranelift) fn compile_format_builtin<M: Module>(
    builder: &mut FunctionBuilder,
    cx: &mut CodegenCtx<'_, M>,
    args: &[CallArg],
) -> Result<ClifValue, CodegenError> {
    if args.is_empty() {
        return compile_string_literal(builder, cx, "");
    }

    // First arg is the template string
    let tpl = compile_expr(builder, cx, &args[0].value)?;
    let tpl_len = cx
        .last_string_len
        .take()
        .unwrap_or_else(|| builder.ins().iconst(clif_types::default_int_type(), 0));

    let num_fmt_args = args.len() - 1;

    if num_fmt_args == 0 {
        // No format args — return template as-is
        cx.last_string_len = Some(tpl_len);
        cx.last_expr_type = Some(clif_types::pointer_type());
        return Ok(tpl);
    }

    // Build format args on stack: each arg is (tag: i64, val1: i64, val2: i64)
    // tag: 0=i64, 1=f64, 2=string(ptr,len)
    let triple_size = 24u32; // 3 * 8 bytes per arg
    let args_slot_size = (num_fmt_args as u32) * triple_size;
    let args_slot = builder.create_sized_stack_slot(cranelift_codegen::ir::StackSlotData::new(
        cranelift_codegen::ir::StackSlotKind::ExplicitSlot,
        args_slot_size,
        0,
    ));

    for (i, arg) in args[1..].iter().enumerate() {
        let is_string_arg = is_string_producing_expr(&arg.value)
            || matches!(&arg.value, Expr::Ident { name, .. } if cx.string_lens.contains_key(name));
        let val = compile_expr(builder, cx, &arg.value)?;
        let val_type = cx.last_expr_type.unwrap_or(clif_types::default_int_type());
        let base_offset = (i as i32) * (triple_size as i32);

        // Detect bool-producing expressions for tag=2
        let is_bool_arg = matches!(
            &arg.value,
            Expr::Literal {
                kind: crate::parser::ast::LiteralKind::Bool(_),
                ..
            }
        ) || matches!(&arg.value, Expr::Ident { name, .. } if cx.var_types.get(name) == Some(&cranelift_codegen::ir::types::I8))
            || val_type == cranelift_codegen::ir::types::I8;

        if is_string_arg || cx.last_string_len.is_some() {
            // tag=3: string (ptr, len)
            let len = cx
                .last_string_len
                .take()
                .unwrap_or_else(|| builder.ins().iconst(clif_types::default_int_type(), 0));
            let tag = builder.ins().iconst(clif_types::default_int_type(), 3);
            builder.ins().stack_store(tag, args_slot, base_offset);
            builder.ins().stack_store(val, args_slot, base_offset + 8);
            builder.ins().stack_store(len, args_slot, base_offset + 16);
        } else if clif_types::is_float(val_type) {
            // tag=1: float
            let tag = builder.ins().iconst(clif_types::default_int_type(), 1);
            let bits = builder.ins().bitcast(
                clif_types::default_int_type(),
                cranelift_codegen::ir::MemFlags::new(),
                val,
            );
            builder.ins().stack_store(tag, args_slot, base_offset);
            builder.ins().stack_store(bits, args_slot, base_offset + 8);
            let zero = builder.ins().iconst(clif_types::default_int_type(), 0);
            builder.ins().stack_store(zero, args_slot, base_offset + 16);
        } else if is_bool_arg {
            // tag=2: bool
            let tag = builder.ins().iconst(clif_types::default_int_type(), 2);
            let bool_val = if val_type == cranelift_codegen::ir::types::I8 {
                builder.ins().uextend(clif_types::default_int_type(), val)
            } else {
                val
            };
            builder.ins().stack_store(tag, args_slot, base_offset);
            builder
                .ins()
                .stack_store(bool_val, args_slot, base_offset + 8);
            let zero = builder.ins().iconst(clif_types::default_int_type(), 0);
            builder.ins().stack_store(zero, args_slot, base_offset + 16);
        } else {
            // tag=0: int
            let tag = builder.ins().iconst(clif_types::default_int_type(), 0);
            builder.ins().stack_store(tag, args_slot, base_offset);
            builder.ins().stack_store(val, args_slot, base_offset + 8);
            let zero = builder.ins().iconst(clif_types::default_int_type(), 0);
            builder.ins().stack_store(zero, args_slot, base_offset + 16);
        }
    }

    let args_ptr = builder
        .ins()
        .stack_addr(clif_types::pointer_type(), args_slot, 0);
    let num_args_val = builder
        .ins()
        .iconst(clif_types::default_int_type(), num_fmt_args as i64);

    let out_ptr_slot = builder.create_sized_stack_slot(cranelift_codegen::ir::StackSlotData::new(
        cranelift_codegen::ir::StackSlotKind::ExplicitSlot,
        8,
        0,
    ));
    let out_len_slot = builder.create_sized_stack_slot(cranelift_codegen::ir::StackSlotData::new(
        cranelift_codegen::ir::StackSlotKind::ExplicitSlot,
        8,
        0,
    ));
    let out_ptr_addr = builder
        .ins()
        .stack_addr(clif_types::pointer_type(), out_ptr_slot, 0);
    let out_len_addr = builder
        .ins()
        .stack_addr(clif_types::pointer_type(), out_len_slot, 0);

    let fid = *cx
        .functions
        .get("__format")
        .ok_or_else(|| CodegenError::Internal("__format not declared".into()))?;
    let callee = cx.module.declare_func_in_func(fid, builder.func);
    builder.ins().call(
        callee,
        &[
            tpl,
            tpl_len,
            args_ptr,
            num_args_val,
            out_ptr_addr,
            out_len_addr,
        ],
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
    cx.last_string_owned = true;
    cx.last_expr_type = Some(clif_types::pointer_type());
    Ok(result_ptr)
}

/// Compiles file I/O builtins.
pub(in crate::codegen::cranelift) fn compile_file_builtin<M: Module>(
    builder: &mut FunctionBuilder,
    cx: &mut CodegenCtx<'_, M>,
    fn_name: &str,
    args: &[CallArg],
) -> Result<ClifValue, CodegenError> {
    if cx.no_std && is_io_builtin(fn_name) {
        return Err(CodegenError::NotImplemented(format!(
            "'{fn_name}' is not available in no_std mode"
        )));
    }
    match fn_name {
        "write_file" | "append_file" => {
            if args.len() < 2 {
                return Err(CodegenError::NotImplemented(format!(
                    "{fn_name}() requires 2 arguments"
                )));
            }
            let path = compile_expr(builder, cx, &args[0].value)?;
            let path_len = cx
                .last_string_len
                .take()
                .unwrap_or_else(|| builder.ins().iconst(clif_types::default_int_type(), 0));
            let content = compile_expr(builder, cx, &args[1].value)?;
            let content_len = cx
                .last_string_len
                .take()
                .unwrap_or_else(|| builder.ins().iconst(clif_types::default_int_type(), 0));
            let rt_name = if fn_name == "write_file" {
                "__write_file"
            } else {
                "__append_file"
            };
            let fid = *cx
                .functions
                .get(rt_name)
                .ok_or_else(|| CodegenError::Internal(format!("{rt_name} not declared")))?;
            let callee = cx.module.declare_func_in_func(fid, builder.func);
            let call = builder
                .ins()
                .call(callee, &[path, path_len, content, content_len]);
            cx.last_expr_type = Some(clif_types::default_int_type());
            Ok(builder.inst_results(call)[0])
        }
        "read_file" => {
            if args.is_empty() {
                return Err(CodegenError::NotImplemented(
                    "read_file() requires 1 argument".into(),
                ));
            }
            let path = compile_expr(builder, cx, &args[0].value)?;
            let path_len = cx
                .last_string_len
                .take()
                .unwrap_or_else(|| builder.ins().iconst(clif_types::default_int_type(), 0));

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

            let fid = *cx
                .functions
                .get("__read_file")
                .ok_or_else(|| CodegenError::Internal("__read_file not declared".into()))?;
            let callee = cx.module.declare_func_in_func(fid, builder.func);
            builder
                .ins()
                .call(callee, &[path, path_len, out_ptr_addr, out_len_addr]);

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
            cx.last_string_owned = true;
            cx.last_expr_type = Some(clif_types::pointer_type());
            Ok(result_ptr)
        }
        "file_exists" => {
            if args.is_empty() {
                return Err(CodegenError::NotImplemented(
                    "file_exists() requires 1 argument".into(),
                ));
            }
            let path = compile_expr(builder, cx, &args[0].value)?;
            let path_len = cx
                .last_string_len
                .take()
                .unwrap_or_else(|| builder.ins().iconst(clif_types::default_int_type(), 0));
            let fid = *cx
                .functions
                .get("__file_exists")
                .ok_or_else(|| CodegenError::Internal("__file_exists not declared".into()))?;
            let callee = cx.module.declare_func_in_func(fid, builder.func);
            let call = builder.ins().call(callee, &[path, path_len]);
            cx.last_expr_type = Some(clif_types::default_int_type());
            Ok(builder.inst_results(call)[0])
        }
        "async_read_file" => {
            if args.is_empty() {
                return Err(CodegenError::NotImplemented(
                    "async_read_file() requires 1 argument".into(),
                ));
            }
            let path = compile_expr(builder, cx, &args[0].value)?;
            let path_len = cx
                .last_string_len
                .take()
                .unwrap_or_else(|| builder.ins().iconst(clif_types::default_int_type(), 0));
            let fid = *cx
                .functions
                .get("__async_read_file")
                .ok_or_else(|| CodegenError::Internal("__async_read_file not declared".into()))?;
            let callee = cx.module.declare_func_in_func(fid, builder.func);
            let call = builder.ins().call(callee, &[path, path_len]);
            cx.last_async_io_new = true;
            cx.last_expr_type = Some(clif_types::default_int_type());
            Ok(builder.inst_results(call)[0])
        }
        "async_write_file" => {
            if args.len() < 2 {
                return Err(CodegenError::NotImplemented(
                    "async_write_file() requires 2 arguments".into(),
                ));
            }
            let path = compile_expr(builder, cx, &args[0].value)?;
            let path_len = cx
                .last_string_len
                .take()
                .unwrap_or_else(|| builder.ins().iconst(clif_types::default_int_type(), 0));
            let content = compile_expr(builder, cx, &args[1].value)?;
            let content_len = cx
                .last_string_len
                .take()
                .unwrap_or_else(|| builder.ins().iconst(clif_types::default_int_type(), 0));
            let fid = *cx
                .functions
                .get("__async_write_file")
                .ok_or_else(|| CodegenError::Internal("__async_write_file not declared".into()))?;
            let callee = cx.module.declare_func_in_func(fid, builder.func);
            let call = builder
                .ins()
                .call(callee, &[path, path_len, content, content_len]);
            cx.last_async_io_new = true;
            cx.last_expr_type = Some(clif_types::default_int_type());
            Ok(builder.inst_results(call)[0])
        }
        _ => Err(CodegenError::NotImplemented(format!(
            "file builtin '{fn_name}'"
        ))),
    }
}

/// Compiles wrapping/saturating/checked arithmetic builtins.
pub(in crate::codegen::cranelift) fn compile_wrapping_builtin<M: Module>(
    builder: &mut FunctionBuilder,
    cx: &mut CodegenCtx<'_, M>,
    fn_name: &str,
    args: &[CallArg],
) -> Result<ClifValue, CodegenError> {
    if args.len() < 2 {
        return Err(CodegenError::NotImplemented(format!(
            "{fn_name}() requires 2 arguments"
        )));
    }
    let a = compile_expr(builder, cx, &args[0].value)?;
    let b = compile_expr(builder, cx, &args[1].value)?;

    // Wrapping/saturating ops use regular i64 arithmetic in Cranelift
    // (wrapping is the default for integer ops).
    // Checked ops return a tag: 1=Some (no overflow), 0=None (overflow).
    match fn_name {
        "checked_add" | "checked_sub" | "checked_mul" => {
            // Always use inline overflow detection.
            // The runtime fj_rt_checked_* functions abort on overflow and return values,
            // but the language-level checked_add/sub/mul must return a tag (1=Some, 0=None).
            // Inline overflow detection for checked_add/sub:
            // For add: overflow if (a > 0 && b > 0 && result < 0) || (a < 0 && b < 0 && result > 0)
            // Simpler: compute result, then check if it overflowed
            let result = match fn_name {
                "checked_add" => builder.ins().iadd(a, b),
                "checked_sub" => builder.ins().isub(a, b),
                "checked_mul" => builder.ins().imul(a, b),
                _ => unreachable!(),
            };
            let zero = builder.ins().iconst(clif_types::default_int_type(), 0);
            let overflowed = match fn_name {
                "checked_add" => {
                    // Overflow if: sign(a) == sign(b) && sign(result) != sign(a)
                    let a_neg = builder.ins().icmp(IntCC::SignedLessThan, a, zero);
                    let b_neg = builder.ins().icmp(IntCC::SignedLessThan, b, zero);
                    let r_neg = builder.ins().icmp(IntCC::SignedLessThan, result, zero);
                    let same_sign = builder.ins().icmp(IntCC::Equal, a_neg, b_neg);
                    let diff_result = builder.ins().icmp(IntCC::NotEqual, a_neg, r_neg);
                    builder.ins().band(same_sign, diff_result)
                }
                "checked_sub" => {
                    // Sub overflow: sign(a) != sign(b) && sign(result) != sign(a)
                    let a_neg = builder.ins().icmp(IntCC::SignedLessThan, a, zero);
                    let b_neg = builder.ins().icmp(IntCC::SignedLessThan, b, zero);
                    let r_neg = builder.ins().icmp(IntCC::SignedLessThan, result, zero);
                    let diff_sign = builder.ins().icmp(IntCC::NotEqual, a_neg, b_neg);
                    let diff_result = builder.ins().icmp(IntCC::NotEqual, a_neg, r_neg);
                    builder.ins().band(diff_sign, diff_result)
                }
                "checked_mul" => {
                    // Mul overflow: if b != 0, result / b != a
                    let b_is_zero = builder.ins().icmp(IntCC::Equal, b, zero);
                    let div_result = builder.ins().sdiv(result, b);
                    let not_equal = builder.ins().icmp(IntCC::NotEqual, div_result, a);
                    // overflow = (b != 0) && (result / b != a)
                    let b_nonzero = builder.ins().bnot(b_is_zero);
                    builder.ins().band(b_nonzero, not_equal)
                }
                _ => unreachable!(),
            };
            // tag = if overflowed { 0 } else { 1 }
            let one = builder.ins().iconst(clif_types::default_int_type(), 1);
            let overflow_ext = builder
                .ins()
                .uextend(clif_types::default_int_type(), overflowed);
            let tag = builder.ins().isub(one, overflow_ext);
            cx.last_expr_type = Some(clif_types::default_int_type());
            Ok(tag)
        }
        "wrapping_add" => {
            let result = builder.ins().iadd(a, b);
            cx.last_expr_type = Some(clif_types::default_int_type());
            Ok(result)
        }
        "wrapping_sub" => {
            let result = builder.ins().isub(a, b);
            cx.last_expr_type = Some(clif_types::default_int_type());
            Ok(result)
        }
        "wrapping_mul" => {
            let result = builder.ins().imul(a, b);
            cx.last_expr_type = Some(clif_types::default_int_type());
            Ok(result)
        }
        "saturating_add" | "saturating_sub" | "saturating_mul" => {
            let rt_fn = format!("__{fn_name}");
            let fid = *cx
                .functions
                .get(&rt_fn)
                .ok_or_else(|| CodegenError::Internal(format!("{rt_fn} not declared")))?;
            let callee = cx.module.declare_func_in_func(fid, builder.func);
            let call = builder.ins().call(callee, &[a, b]);
            let result = builder.inst_results(call)[0];
            cx.last_expr_type = Some(clif_types::default_int_type());
            Ok(result)
        }
        _ => Err(CodegenError::NotImplemented(format!(
            "wrapping builtin '{fn_name}'"
        ))),
    }
}
