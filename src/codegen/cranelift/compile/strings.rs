//! String-related codegen functions for Fajar Lang compilation.
//!
//! Handles string literals, concatenation, method calls, transforms,
//! and parse methods.

use std::sync::atomic::Ordering;

use cranelift_codegen::ir::condcodes::IntCC;
use cranelift_codegen::ir::{InstBuilder, Value as ClifValue};
use cranelift_frontend::FunctionBuilder;
use cranelift_module::{DataDescription, Linkage, Module};

use super::super::context::CodegenCtx;
use super::super::{DATA_COUNTER, clif_types};
use super::compile_expr;
use crate::codegen::CodegenError;
use crate::parser::ast::{CallArg, Expr};

/// Compiles a string literal: stores bytes in a data section, returns (ptr, len).
pub(in crate::codegen::cranelift) fn compile_string_literal<M: Module>(
    builder: &mut FunctionBuilder,
    cx: &mut CodegenCtx<'_, M>,
    s: &str,
) -> Result<ClifValue, CodegenError> {
    let len = s.len() as i64;

    if let Some(&data_id) = cx.string_data.get(s) {
        let gv = cx.module.declare_data_in_func(data_id, builder.func);
        let ptr = builder.ins().global_value(clif_types::pointer_type(), gv);
        cx.last_string_len = Some(builder.ins().iconst(clif_types::default_int_type(), len));
        cx.last_string_owned = false;
        cx.last_expr_type = Some(clif_types::pointer_type());
        return Ok(ptr);
    }

    let counter = DATA_COUNTER.fetch_add(1, Ordering::SeqCst);
    let name = format!("__str_{counter}");

    let data_id = cx
        .module
        .declare_data(&name, Linkage::Local, false, false)
        .map_err(|e| CodegenError::Internal(e.to_string()))?;

    let mut desc = DataDescription::new();
    desc.define(s.as_bytes().to_vec().into_boxed_slice());
    cx.module
        .define_data(data_id, &desc)
        .map_err(|e| CodegenError::Internal(e.to_string()))?;

    cx.string_data.insert(s.to_string(), data_id);

    let gv = cx.module.declare_data_in_func(data_id, builder.func);
    let ptr = builder.ins().global_value(clif_types::pointer_type(), gv);
    cx.last_string_len = Some(builder.ins().iconst(clif_types::default_int_type(), len));
    cx.last_string_owned = false;
    cx.last_expr_type = Some(clif_types::pointer_type());
    Ok(ptr)
}

/// Compiles string concatenation (left + right where left is a string).
pub(in crate::codegen::cranelift) fn compile_string_concat<M: Module>(
    builder: &mut FunctionBuilder,
    cx: &mut CodegenCtx<'_, M>,
    left: &Expr,
    right: &Expr,
) -> Result<ClifValue, CodegenError> {
    let lhs = compile_expr(builder, cx, left)?;
    let l_len = cx
        .last_string_len
        .take()
        .unwrap_or_else(|| builder.ins().iconst(clif_types::default_int_type(), 0));

    let rhs = compile_expr(builder, cx, right)?;
    let r_len = cx
        .last_string_len
        .take()
        .unwrap_or_else(|| builder.ins().iconst(clif_types::default_int_type(), 0));

    compile_string_concat_vals(builder, cx, lhs, l_len, rhs, r_len)
}

/// Emits a call to fj_rt_str_concat with pre-compiled values.
pub(in crate::codegen::cranelift) fn compile_string_concat_vals<M: Module>(
    builder: &mut FunctionBuilder,
    cx: &mut CodegenCtx<'_, M>,
    lhs: ClifValue,
    l_len: ClifValue,
    rhs: ClifValue,
    r_len: ClifValue,
) -> Result<ClifValue, CodegenError> {
    // Allocate stack slots for the output (ptr, len)
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

    let concat_id = *cx
        .functions
        .get("__str_concat")
        .ok_or_else(|| CodegenError::Internal("__str_concat not declared".into()))?;
    let callee = cx.module.declare_func_in_func(concat_id, builder.func);
    builder.ins().call(
        callee,
        &[lhs, l_len, rhs, r_len, out_ptr_addr, out_len_addr],
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

/// Compiles a string method call.
pub(in crate::codegen::cranelift) fn compile_string_method<M: Module>(
    builder: &mut FunctionBuilder,
    cx: &mut CodegenCtx<'_, M>,
    receiver: &Expr,
    method: &str,
    args: &[CallArg],
) -> Result<ClifValue, CodegenError> {
    let recv = compile_expr(builder, cx, receiver)?;
    let recv_len = cx
        .last_string_len
        .take()
        .unwrap_or_else(|| builder.ins().iconst(clif_types::default_int_type(), 0));

    match method {
        "len" => {
            cx.last_expr_type = Some(clif_types::default_int_type());
            Ok(recv_len)
        }
        "is_empty" => {
            let zero = builder.ins().iconst(clif_types::default_int_type(), 0);
            let cmp = builder.ins().icmp(IntCC::Equal, recv_len, zero);
            let result = builder.ins().uextend(clif_types::default_int_type(), cmp);
            cx.last_expr_type = Some(clif_types::default_int_type());
            Ok(result)
        }
        "contains" | "starts_with" | "ends_with" | "index_of" => {
            if args.is_empty() {
                return Err(CodegenError::NotImplemented(format!(
                    "str.{method}() requires 1 argument"
                )));
            }
            let needle = compile_expr(builder, cx, &args[0].value)?;
            let needle_len = cx
                .last_string_len
                .take()
                .unwrap_or_else(|| builder.ins().iconst(clif_types::default_int_type(), 0));
            let rt_fn = format!("__str_{method}");
            let fid = *cx
                .functions
                .get(&rt_fn)
                .ok_or_else(|| CodegenError::Internal(format!("{rt_fn} not declared")))?;
            let callee = cx.module.declare_func_in_func(fid, builder.func);
            let call = builder
                .ins()
                .call(callee, &[recv, recv_len, needle, needle_len]);
            cx.last_expr_type = Some(clif_types::default_int_type());
            Ok(builder.inst_results(call)[0])
        }
        "trim" | "trim_start" | "trim_end" | "to_uppercase" | "to_lowercase" | "rev" => {
            compile_string_transform(builder, cx, recv, recv_len, method)
        }
        "replace" => {
            if args.len() < 2 {
                return Err(CodegenError::NotImplemented(
                    "str.replace() requires 2 arguments".into(),
                ));
            }
            let old = compile_expr(builder, cx, &args[0].value)?;
            let old_len = cx
                .last_string_len
                .take()
                .unwrap_or_else(|| builder.ins().iconst(clif_types::default_int_type(), 0));
            let new = compile_expr(builder, cx, &args[1].value)?;
            let new_len = cx
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
                .get("__str_replace")
                .ok_or_else(|| CodegenError::Internal("__str_replace not declared".into()))?;
            let callee = cx.module.declare_func_in_func(fid, builder.func);
            builder.ins().call(
                callee,
                &[
                    recv,
                    recv_len,
                    old,
                    old_len,
                    new,
                    new_len,
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
        "substring" => {
            if args.len() < 2 {
                return Err(CodegenError::NotImplemented(
                    "str.substring() requires 2 arguments".into(),
                ));
            }
            let start = compile_expr(builder, cx, &args[0].value)?;
            let end = compile_expr(builder, cx, &args[1].value)?;

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
                .get("__str_substring")
                .ok_or_else(|| CodegenError::Internal("__str_substring not declared".into()))?;
            let callee = cx.module.declare_func_in_func(fid, builder.func);
            builder.ins().call(
                callee,
                &[recv, recv_len, start, end, out_ptr_addr, out_len_addr],
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
            // substring returns a view into the original string (not heap-allocated)
            cx.last_string_owned = false;
            cx.last_expr_type = Some(clif_types::pointer_type());
            Ok(result_ptr)
        }
        "repeat" => {
            if args.is_empty() {
                return Err(CodegenError::NotImplemented(
                    "str.repeat() requires 1 argument".into(),
                ));
            }
            let count = compile_expr(builder, cx, &args[0].value)?;

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
                .get("__str_repeat")
                .ok_or_else(|| CodegenError::Internal("__str_repeat not declared".into()))?;
            let callee = cx.module.declare_func_in_func(fid, builder.func);
            builder
                .ins()
                .call(callee, &[recv, recv_len, count, out_ptr_addr, out_len_addr]);

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
        "chars" | "bytes" => {
            let rt_fn = format!("__str_{method}");
            let fid = *cx
                .functions
                .get(&rt_fn)
                .ok_or_else(|| CodegenError::Internal(format!("{rt_fn} not declared")))?;
            let callee = cx.module.declare_func_in_func(fid, builder.func);
            let call = builder.ins().call(callee, &[recv, recv_len]);
            let result = builder.inst_results(call)[0];
            cx.last_heap_array = true;
            cx.last_expr_type = Some(clif_types::pointer_type());
            Ok(result)
        }
        "split" => {
            if args.is_empty() {
                return Err(CodegenError::NotImplemented(
                    "str.split() requires 1 argument".into(),
                ));
            }
            let sep = compile_expr(builder, cx, &args[0].value)?;
            let sep_len = cx
                .last_string_len
                .take()
                .unwrap_or_else(|| builder.ins().iconst(clif_types::default_int_type(), 0));
            let fid = *cx
                .functions
                .get("__str_split")
                .ok_or_else(|| CodegenError::Internal("__str_split not declared".into()))?;
            let callee = cx.module.declare_func_in_func(fid, builder.func);
            let call = builder.ins().call(callee, &[recv, recv_len, sep, sep_len]);
            let result = builder.inst_results(call)[0];
            cx.last_split_result = Some(result);
            cx.last_expr_type = Some(clif_types::pointer_type());
            Ok(result)
        }
        "parse_int" => compile_parse_method(builder, cx, recv, recv_len, "__parse_int"),
        "parse_float" => compile_parse_method(builder, cx, recv, recv_len, "__parse_float"),
        _ => Err(CodegenError::NotImplemented(format!(
            "string method '.{method}()'"
        ))),
    }
}

/// Compiles string transform methods (trim, to_uppercase, etc.)
/// that take (ptr, len, out_ptr, out_len) -> void.
pub(in crate::codegen::cranelift) fn compile_string_transform<M: Module>(
    builder: &mut FunctionBuilder,
    cx: &mut CodegenCtx<'_, M>,
    recv: ClifValue,
    recv_len: ClifValue,
    method: &str,
) -> Result<ClifValue, CodegenError> {
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

    let rt_fn = format!("__str_{method}");
    let fid = *cx
        .functions
        .get(&rt_fn)
        .ok_or_else(|| CodegenError::Internal(format!("{rt_fn} not declared")))?;
    let callee = cx.module.declare_func_in_func(fid, builder.func);
    builder
        .ins()
        .call(callee, &[recv, recv_len, out_ptr_addr, out_len_addr]);

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
    // trim/trim_start/trim_end return views into the original string (not heap-allocated),
    // while to_uppercase/to_lowercase/rev allocate new strings on the heap.
    cx.last_string_owned = !matches!(method, "trim" | "trim_start" | "trim_end");
    cx.last_expr_type = Some(clif_types::pointer_type());
    Ok(result_ptr)
}

/// Compiles parse_int/parse_float method on strings.
pub(in crate::codegen::cranelift) fn compile_parse_method<M: Module>(
    builder: &mut FunctionBuilder,
    cx: &mut CodegenCtx<'_, M>,
    recv: ClifValue,
    recv_len: ClifValue,
    rt_fn: &str,
) -> Result<ClifValue, CodegenError> {
    let out_tag_slot = builder.create_sized_stack_slot(cranelift_codegen::ir::StackSlotData::new(
        cranelift_codegen::ir::StackSlotKind::ExplicitSlot,
        8,
        0,
    ));
    let out_val_slot = builder.create_sized_stack_slot(cranelift_codegen::ir::StackSlotData::new(
        cranelift_codegen::ir::StackSlotKind::ExplicitSlot,
        8,
        0,
    ));
    let out_tag_addr = builder
        .ins()
        .stack_addr(clif_types::pointer_type(), out_tag_slot, 0);
    let out_val_addr = builder
        .ins()
        .stack_addr(clif_types::pointer_type(), out_val_slot, 0);

    let fid = *cx
        .functions
        .get(rt_fn)
        .ok_or_else(|| CodegenError::Internal(format!("{rt_fn} not declared")))?;
    let callee = cx.module.declare_func_in_func(fid, builder.func);
    builder
        .ins()
        .call(callee, &[recv, recv_len, out_tag_addr, out_val_addr]);

    // Return the tag (0=Ok, 1=Err) — payload available via enum_vars
    let tag = builder.ins().load(
        clif_types::default_int_type(),
        cranelift_codegen::ir::MemFlags::new(),
        out_tag_addr,
        0,
    );
    let val = builder.ins().load(
        clif_types::default_int_type(),
        cranelift_codegen::ir::MemFlags::new(),
        out_val_addr,
        0,
    );
    cx.last_enum_payload = Some(val);
    cx.last_enum_payload_type = Some(clif_types::default_int_type());
    cx.last_expr_type = Some(clif_types::default_int_type());
    Ok(tag)
}
