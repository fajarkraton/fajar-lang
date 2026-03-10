//! Array compilation: literals, indexing, heap/stack array methods.

use cranelift_codegen::ir::condcodes::IntCC;
use cranelift_codegen::ir::{InstBuilder, Value as ClifValue};
use cranelift_frontend::FunctionBuilder;
use cranelift_module::Module;

use super::super::clif_types;
use super::super::context::{push_owned, CodegenCtx, OwnedKind};
use super::compile_expr;
use crate::codegen::CodegenError;
use crate::parser::ast::{CallArg, Expr};

/// Compiles an array literal expression: `[a, b, c]`.
///
/// Elements are stored in a stack slot at 8-byte offsets.
/// Sets `cx.last_array` with the slot metadata.
pub(in crate::codegen::cranelift) fn compile_array_literal<M: Module>(
    builder: &mut FunctionBuilder,
    cx: &mut CodegenCtx<'_, M>,
    elements: &[Expr],
) -> Result<ClifValue, CodegenError> {
    let len = elements.len();
    if len == 0 {
        // Empty array — return null pointer with zero-length metadata
        cx.last_expr_type = Some(clif_types::pointer_type());
        cx.last_array = None;
        return Ok(builder.ins().iconst(clif_types::pointer_type(), 0));
    }

    let slot_size = (len as u32)
        .checked_mul(8)
        .ok_or_else(|| CodegenError::NotImplemented(format!("array too large ({len} elements)")))?;
    let slot = builder.create_sized_stack_slot(cranelift_codegen::ir::StackSlotData::new(
        cranelift_codegen::ir::StackSlotKind::ExplicitSlot,
        slot_size,
        3, // 8-byte alignment
    ));

    let mut elem_type = clif_types::default_int_type();
    for (i, elem) in elements.iter().enumerate() {
        let val = compile_expr(builder, cx, elem)?;
        if let Some(et) = cx.last_expr_type {
            elem_type = et;
        }
        builder.ins().stack_store(val, slot, (i as i32) * 8);
    }

    cx.last_array = Some((slot, len));
    cx.last_expr_type = Some(elem_type);
    let ptr = builder
        .ins()
        .stack_addr(clif_types::pointer_type(), slot, 0);
    Ok(ptr)
}

/// Compiles a heap-backed dynamic array initialization: `let arr: [i64] = []`.
pub(in crate::codegen::cranelift) fn compile_heap_array_init<M: Module>(
    builder: &mut FunctionBuilder,
    cx: &mut CodegenCtx<'_, M>,
    name: &str,
    _elements: &[Expr],
) -> Result<Option<ClifValue>, CodegenError> {
    let cap = builder.ins().iconst(clif_types::default_int_type(), 4);
    let new_id = *cx
        .functions
        .get("__array_new")
        .ok_or_else(|| CodegenError::Internal("__array_new not declared".into()))?;
    let callee = cx.module.declare_func_in_func(new_id, builder.func);
    let call = builder.ins().call(callee, &[cap]);
    let arr_ptr = builder.inst_results(call)[0];

    let var = builder.declare_var(clif_types::pointer_type());
    builder.def_var(var, arr_ptr);
    cx.var_map.insert(name.to_string(), var);
    cx.var_types
        .insert(name.to_string(), clif_types::pointer_type());
    cx.heap_arrays.insert(name.to_string());
    push_owned(cx, name.to_string(), OwnedKind::Array);
    Ok(None)
}

/// Compiles an index expression: `obj[index]`.
pub(in crate::codegen::cranelift) fn compile_index<M: Module>(
    builder: &mut FunctionBuilder,
    cx: &mut CodegenCtx<'_, M>,
    object: &Expr,
    index: &Expr,
) -> Result<ClifValue, CodegenError> {
    let obj_name = match object {
        Expr::Ident { name, .. } => Some(name.clone()),
        _ => None,
    };

    // Stack array index: arr[i]
    if let Some(ref name) = obj_name {
        if let Some((slot, _len)) = cx.array_meta.get(name).copied() {
            let idx_val = compile_expr(builder, cx, index)?;
            let elem_type = cx
                .var_types
                .get(name)
                .copied()
                .unwrap_or(clif_types::default_int_type());
            let slot_addr = builder
                .ins()
                .stack_addr(clif_types::pointer_type(), slot, 0);
            let byte_offset = builder.ins().imul_imm(idx_val, 8);
            let elem_addr = builder.ins().iadd(slot_addr, byte_offset);
            let val = builder.ins().load(
                elem_type,
                cranelift_codegen::ir::MemFlags::new(),
                elem_addr,
                0,
            );
            cx.last_expr_type = Some(elem_type);
            return Ok(val);
        }
    }

    // Heap array index: arr[i] via runtime call
    if let Some(ref name) = obj_name {
        if cx.heap_arrays.contains(name) {
            let arr_var = *cx
                .var_map
                .get(name)
                .ok_or_else(|| CodegenError::UndefinedVariable(name.clone()))?;
            let arr_ptr = builder.use_var(arr_var);
            let idx_val = compile_expr(builder, cx, index)?;
            let get_id = *cx
                .functions
                .get("__array_get")
                .ok_or_else(|| CodegenError::Internal("__array_get not declared".into()))?;
            let callee = cx.module.declare_func_in_func(get_id, builder.func);
            let call = builder.ins().call(callee, &[arr_ptr, idx_val]);
            cx.last_expr_type = Some(clif_types::default_int_type());
            return Ok(builder.inst_results(call)[0]);
        }
    }

    // String index: str[i] — load single byte
    if let Some(ref name) = obj_name {
        if cx.string_lens.contains_key(name) {
            let str_var = *cx
                .var_map
                .get(name)
                .ok_or_else(|| CodegenError::UndefinedVariable(name.clone()))?;
            let str_ptr = builder.use_var(str_var);
            let idx_val = compile_expr(builder, cx, index)?;
            let byte_addr = builder.ins().iadd(str_ptr, idx_val);
            let byte_val = builder.ins().load(
                cranelift_codegen::ir::types::I8,
                cranelift_codegen::ir::MemFlags::new(),
                byte_addr,
                0,
            );
            let result = builder
                .ins()
                .uextend(clif_types::default_int_type(), byte_val);
            // Set up as a single-char string for println dispatch
            // The index result is a char code (integer), not a string
            cx.last_expr_type = Some(clif_types::default_int_type());
            return Ok(result);
        }
    }

    // Split result index: parts[i]
    if let Some(ref name) = obj_name {
        if cx.split_vars.contains(name) {
            let arr_var = *cx
                .var_map
                .get(name)
                .ok_or_else(|| CodegenError::UndefinedVariable(name.clone()))?;
            let arr_ptr = builder.use_var(arr_var);
            let idx_val = compile_expr(builder, cx, index)?;

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

            let split_get_id = *cx
                .functions
                .get("__split_get")
                .ok_or_else(|| CodegenError::Internal("__split_get not declared".into()))?;
            let callee = cx.module.declare_func_in_func(split_get_id, builder.func);
            builder
                .ins()
                .call(callee, &[arr_ptr, idx_val, out_ptr_addr, out_len_addr]);

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
            cx.last_expr_type = Some(clif_types::pointer_type());
            return Ok(result_ptr);
        }
    }

    // Fallback: try compiling object as expression
    let obj_val = compile_expr(builder, cx, object)?;
    let idx_val = compile_expr(builder, cx, index)?;
    let byte_offset = builder.ins().imul_imm(idx_val, 8);
    let addr = builder.ins().iadd(obj_val, byte_offset);
    let val = builder.ins().load(
        clif_types::default_int_type(),
        cranelift_codegen::ir::MemFlags::new(),
        addr,
        0,
    );
    cx.last_expr_type = Some(clif_types::default_int_type());
    Ok(val)
}

/// Compiles index assignment: `arr[i] = val` or `arr[i] += val`.
pub(in crate::codegen::cranelift) fn compile_index_assign<M: Module>(
    builder: &mut FunctionBuilder,
    cx: &mut CodegenCtx<'_, M>,
    object: &Expr,
    index: &Expr,
    value: &Expr,
    op: &crate::parser::ast::AssignOp,
) -> Result<ClifValue, CodegenError> {
    use crate::parser::ast::AssignOp;

    let obj_name = match object {
        Expr::Ident { name, .. } => name.clone(),
        _ => {
            return Err(CodegenError::NotImplemented(
                "index assign on non-ident".into(),
            ))
        }
    };

    // Heap array: arr[i] = val via runtime
    if cx.heap_arrays.contains(&obj_name) {
        let arr_var = *cx
            .var_map
            .get(&obj_name)
            .ok_or_else(|| CodegenError::UndefinedVariable(obj_name.clone()))?;
        let arr_ptr = builder.use_var(arr_var);
        let idx_val = compile_expr(builder, cx, index)?;
        let rhs = compile_expr(builder, cx, value)?;

        let final_val = if *op == AssignOp::Assign {
            rhs
        } else {
            // Read current value first
            let get_id = *cx
                .functions
                .get("__array_get")
                .ok_or_else(|| CodegenError::Internal("__array_get not declared".into()))?;
            let get_callee = cx.module.declare_func_in_func(get_id, builder.func);
            let get_call = builder.ins().call(get_callee, &[arr_ptr, idx_val]);
            let current = builder.inst_results(get_call)[0];
            match op {
                AssignOp::AddAssign => builder.ins().iadd(current, rhs),
                AssignOp::SubAssign => builder.ins().isub(current, rhs),
                AssignOp::MulAssign => builder.ins().imul(current, rhs),
                AssignOp::DivAssign => builder.ins().sdiv(current, rhs),
                _ => {
                    return Err(CodegenError::NotImplemented(
                        "compound heap array index assign".into(),
                    ))
                }
            }
        };

        let set_id = *cx
            .functions
            .get("__array_set")
            .ok_or_else(|| CodegenError::Internal("__array_set not declared".into()))?;
        let set_callee = cx.module.declare_func_in_func(set_id, builder.func);
        let arr_ptr2 = builder.use_var(arr_var);
        builder
            .ins()
            .call(set_callee, &[arr_ptr2, idx_val, final_val]);
        cx.last_expr_type = Some(clif_types::default_int_type());
        return Ok(final_val);
    }

    // Stack array: arr[i] = val
    if let Some((slot, _len)) = cx.array_meta.get(&obj_name).copied() {
        let idx_val = compile_expr(builder, cx, index)?;
        let rhs = compile_expr(builder, cx, value)?;
        let elem_type = cx
            .var_types
            .get(&obj_name)
            .copied()
            .unwrap_or(clif_types::default_int_type());

        let slot_addr = builder
            .ins()
            .stack_addr(clif_types::pointer_type(), slot, 0);
        let byte_offset = builder.ins().imul_imm(idx_val, 8);
        let elem_addr = builder.ins().iadd(slot_addr, byte_offset);

        let final_val = if *op == AssignOp::Assign {
            rhs
        } else {
            let current = builder.ins().load(
                elem_type,
                cranelift_codegen::ir::MemFlags::new(),
                elem_addr,
                0,
            );
            let is_float = clif_types::is_float(elem_type);
            if is_float {
                match op {
                    AssignOp::AddAssign => builder.ins().fadd(current, rhs),
                    AssignOp::SubAssign => builder.ins().fsub(current, rhs),
                    AssignOp::MulAssign => builder.ins().fmul(current, rhs),
                    AssignOp::DivAssign => builder.ins().fdiv(current, rhs),
                    _ => {
                        return Err(CodegenError::NotImplemented(
                            "float compound array index assign".into(),
                        ))
                    }
                }
            } else {
                match op {
                    AssignOp::AddAssign => builder.ins().iadd(current, rhs),
                    AssignOp::SubAssign => builder.ins().isub(current, rhs),
                    AssignOp::MulAssign => builder.ins().imul(current, rhs),
                    AssignOp::DivAssign => builder.ins().sdiv(current, rhs),
                    AssignOp::RemAssign => builder.ins().srem(current, rhs),
                    _ => {
                        return Err(CodegenError::NotImplemented(
                            "compound array index assign".into(),
                        ))
                    }
                }
            }
        };

        builder.ins().store(
            cranelift_codegen::ir::MemFlags::new(),
            final_val,
            elem_addr,
            0,
        );
        cx.last_expr_type = Some(elem_type);
        return Ok(final_val);
    }

    Err(CodegenError::NotImplemented(format!(
        "index assign on '{obj_name}'"
    )))
}

/// Compiles heap array method calls: push, pop, len.
pub(in crate::codegen::cranelift) fn compile_heap_array_method<M: Module>(
    builder: &mut FunctionBuilder,
    cx: &mut CodegenCtx<'_, M>,
    name: &str,
    method: &str,
    args: &[CallArg],
) -> Result<ClifValue, CodegenError> {
    let arr_var = *cx
        .var_map
        .get(name)
        .ok_or_else(|| CodegenError::UndefinedVariable(name.to_string()))?;
    let arr_ptr = builder.use_var(arr_var);

    match method {
        "push" => {
            if args.is_empty() {
                return Err(CodegenError::NotImplemented(
                    "array.push() requires 1 argument".into(),
                ));
            }
            let val = compile_expr(builder, cx, &args[0].value)?;
            let push_id = *cx
                .functions
                .get("__array_push")
                .ok_or_else(|| CodegenError::Internal("__array_push not declared".into()))?;
            let callee = cx.module.declare_func_in_func(push_id, builder.func);
            builder.ins().call(callee, &[arr_ptr, val]);
            cx.last_expr_type = Some(clif_types::default_int_type());
            // Return the array pointer so `arr = arr.push(val)` works correctly
            Ok(arr_ptr)
        }
        "pop" => {
            let pop_id = *cx
                .functions
                .get("__array_pop")
                .ok_or_else(|| CodegenError::Internal("__array_pop not declared".into()))?;
            let callee = cx.module.declare_func_in_func(pop_id, builder.func);
            let call = builder.ins().call(callee, &[arr_ptr]);
            cx.last_expr_type = Some(clif_types::default_int_type());
            Ok(builder.inst_results(call)[0])
        }
        "len" => {
            let len_id = *cx
                .functions
                .get("__array_len")
                .ok_or_else(|| CodegenError::Internal("__array_len not declared".into()))?;
            let callee = cx.module.declare_func_in_func(len_id, builder.func);
            let call = builder.ins().call(callee, &[arr_ptr]);
            cx.last_expr_type = Some(clif_types::default_int_type());
            Ok(builder.inst_results(call)[0])
        }
        "join" => {
            if args.is_empty() {
                return Err(CodegenError::NotImplemented(
                    "array.join() requires 1 argument".into(),
                ));
            }
            let sep = compile_expr(builder, cx, &args[0].value)?;
            let sep_len = cx
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
                .get("__array_join")
                .ok_or_else(|| CodegenError::Internal("__array_join not declared".into()))?;
            let callee = cx.module.declare_func_in_func(fid, builder.func);
            builder
                .ins()
                .call(callee, &[arr_ptr, sep, sep_len, out_ptr_addr, out_len_addr]);

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
        "contains" => {
            if args.is_empty() {
                return Err(CodegenError::NotImplemented(
                    "array.contains() requires 1 argument".into(),
                ));
            }
            let val = compile_expr(builder, cx, &args[0].value)?;
            let fid = *cx
                .functions
                .get("__array_contains")
                .ok_or_else(|| CodegenError::Internal("__array_contains not declared".into()))?;
            let callee = cx.module.declare_func_in_func(fid, builder.func);
            let call = builder.ins().call(callee, &[arr_ptr, val]);
            cx.last_expr_type = Some(clif_types::default_int_type());
            Ok(builder.inst_results(call)[0])
        }
        "is_empty" => {
            let fid = *cx
                .functions
                .get("__array_is_empty")
                .ok_or_else(|| CodegenError::Internal("__array_is_empty not declared".into()))?;
            let callee = cx.module.declare_func_in_func(fid, builder.func);
            let call = builder.ins().call(callee, &[arr_ptr]);
            cx.last_expr_type = Some(clif_types::default_int_type());
            Ok(builder.inst_results(call)[0])
        }
        "reverse" => {
            let fid = *cx
                .functions
                .get("__array_reverse")
                .ok_or_else(|| CodegenError::Internal("__array_reverse not declared".into()))?;
            let callee = cx.module.declare_func_in_func(fid, builder.func);
            builder.ins().call(callee, &[arr_ptr]);
            cx.last_expr_type = Some(clif_types::default_int_type());
            Ok(builder.ins().iconst(clif_types::default_int_type(), 0))
        }
        "map" => {
            // arr.map(fn_ptr) → new heap array with fn_ptr applied to each element
            if args.is_empty() {
                return Err(CodegenError::NotImplemented(
                    "array.map() requires a function argument".into(),
                ));
            }
            let fn_ptr = compile_expr(builder, cx, &args[0].value)?;

            // Get source array length
            let len_id = *cx
                .functions
                .get("__array_len")
                .ok_or_else(|| CodegenError::Internal("__array_len not declared".into()))?;
            let len_callee = cx.module.declare_func_in_func(len_id, builder.func);
            let len_call = builder.ins().call(len_callee, &[arr_ptr]);
            let arr_len = builder.inst_results(len_call)[0];

            // Create new heap array
            let new_id = *cx
                .functions
                .get("__array_new")
                .ok_or_else(|| CodegenError::Internal("__array_new not declared".into()))?;
            let new_callee = cx.module.declare_func_in_func(new_id, builder.func);
            let new_call = builder.ins().call(new_callee, &[arr_len]);
            let result_arr = builder.inst_results(new_call)[0];

            // Loop: for i in 0..len { new_arr.push(fn_ptr(old_arr[i])) }
            let header = builder.create_block();
            let body = builder.create_block();
            let exit = builder.create_block();

            // Initialize index variable
            let idx_var = builder.declare_var(clif_types::default_int_type());
            let zero = builder.ins().iconst(clif_types::default_int_type(), 0);
            builder.def_var(idx_var, zero);

            builder.ins().jump(header, &[]);
            builder.switch_to_block(header);

            let idx = builder.use_var(idx_var);
            let cond = builder.ins().icmp(
                cranelift_codegen::ir::condcodes::IntCC::SignedLessThan,
                idx,
                arr_len,
            );
            builder.ins().brif(cond, body, &[], exit, &[]);

            builder.switch_to_block(body);
            builder.seal_block(body);

            // Get element at index
            let get_id = *cx
                .functions
                .get("__array_get")
                .ok_or_else(|| CodegenError::Internal("__array_get not declared".into()))?;
            let get_callee = cx.module.declare_func_in_func(get_id, builder.func);
            let get_call = builder.ins().call(get_callee, &[arr_ptr, idx]);
            let elem = builder.inst_results(get_call)[0];

            // Call fn_ptr(elem) via call_indirect
            let sig = builder
                .func
                .import_signature(cranelift_codegen::ir::Signature {
                    params: vec![cranelift_codegen::ir::AbiParam::new(
                        clif_types::default_int_type(),
                    )],
                    returns: vec![cranelift_codegen::ir::AbiParam::new(
                        clif_types::default_int_type(),
                    )],
                    call_conv: cranelift_codegen::isa::CallConv::SystemV,
                });
            let mapped_call = builder.ins().call_indirect(sig, fn_ptr, &[elem]);
            let mapped_val = builder.inst_results(mapped_call)[0];

            // Push to result array
            let push_id = *cx
                .functions
                .get("__array_push")
                .ok_or_else(|| CodegenError::Internal("__array_push not declared".into()))?;
            let push_callee = cx.module.declare_func_in_func(push_id, builder.func);
            builder.ins().call(push_callee, &[result_arr, mapped_val]);

            // Increment index
            let one = builder.ins().iconst(clif_types::default_int_type(), 1);
            let next_idx = builder.ins().iadd(idx, one);
            builder.def_var(idx_var, next_idx);

            builder.ins().jump(header, &[]);
            builder.seal_block(header);
            builder.switch_to_block(exit);
            builder.seal_block(exit);

            cx.last_expr_type = Some(clif_types::default_int_type());
            cx.last_heap_array = true;
            Ok(result_arr)
        }
        "filter" => {
            // arr.filter(fn_ptr) → new heap array with elements where fn_ptr returns non-zero
            if args.is_empty() {
                return Err(CodegenError::NotImplemented(
                    "array.filter() requires a function argument".into(),
                ));
            }
            let fn_ptr = compile_expr(builder, cx, &args[0].value)?;

            // Get source array length
            let len_id = *cx
                .functions
                .get("__array_len")
                .ok_or_else(|| CodegenError::Internal("__array_len not declared".into()))?;
            let len_callee = cx.module.declare_func_in_func(len_id, builder.func);
            let len_call = builder.ins().call(len_callee, &[arr_ptr]);
            let arr_len = builder.inst_results(len_call)[0];

            // Create new heap array
            let new_id = *cx
                .functions
                .get("__array_new")
                .ok_or_else(|| CodegenError::Internal("__array_new not declared".into()))?;
            let new_callee = cx.module.declare_func_in_func(new_id, builder.func);
            let new_call = builder.ins().call(new_callee, &[arr_len]);
            let result_arr = builder.inst_results(new_call)[0];

            // Loop: for i in 0..len { if fn_ptr(elem) { new_arr.push(elem) } }
            let header = builder.create_block();
            let body = builder.create_block();
            let push_block = builder.create_block();
            let cont = builder.create_block();
            let exit = builder.create_block();

            let idx_var = builder.declare_var(clif_types::default_int_type());
            let zero = builder.ins().iconst(clif_types::default_int_type(), 0);
            builder.def_var(idx_var, zero);

            builder.ins().jump(header, &[]);
            builder.switch_to_block(header);

            let idx = builder.use_var(idx_var);
            let cond = builder.ins().icmp(
                cranelift_codegen::ir::condcodes::IntCC::SignedLessThan,
                idx,
                arr_len,
            );
            builder.ins().brif(cond, body, &[], exit, &[]);

            builder.switch_to_block(body);
            builder.seal_block(body);

            // Get element
            let get_id = *cx
                .functions
                .get("__array_get")
                .ok_or_else(|| CodegenError::Internal("__array_get not declared".into()))?;
            let get_callee = cx.module.declare_func_in_func(get_id, builder.func);
            let get_call = builder.ins().call(get_callee, &[arr_ptr, idx]);
            let elem = builder.inst_results(get_call)[0];

            // Call predicate: fn_ptr(elem)
            let sig = builder
                .func
                .import_signature(cranelift_codegen::ir::Signature {
                    params: vec![cranelift_codegen::ir::AbiParam::new(
                        clif_types::default_int_type(),
                    )],
                    returns: vec![cranelift_codegen::ir::AbiParam::new(
                        clif_types::default_int_type(),
                    )],
                    call_conv: cranelift_codegen::isa::CallConv::SystemV,
                });
            let pred_call = builder.ins().call_indirect(sig, fn_ptr, &[elem]);
            let pred_val = builder.inst_results(pred_call)[0];

            // If non-zero: push
            let zero_cmp = builder.ins().iconst(clif_types::default_int_type(), 0);
            let is_true = builder.ins().icmp(
                cranelift_codegen::ir::condcodes::IntCC::NotEqual,
                pred_val,
                zero_cmp,
            );
            builder.ins().brif(is_true, push_block, &[], cont, &[]);

            builder.switch_to_block(push_block);
            builder.seal_block(push_block);

            let push_id = *cx
                .functions
                .get("__array_push")
                .ok_or_else(|| CodegenError::Internal("__array_push not declared".into()))?;
            let push_callee = cx.module.declare_func_in_func(push_id, builder.func);
            builder.ins().call(push_callee, &[result_arr, elem]);
            builder.ins().jump(cont, &[]);

            builder.switch_to_block(cont);
            builder.seal_block(cont);

            // Increment index
            let one = builder.ins().iconst(clif_types::default_int_type(), 1);
            let next_idx = builder.ins().iadd(idx, one);
            builder.def_var(idx_var, next_idx);
            builder.ins().jump(header, &[]);

            builder.seal_block(header);
            builder.switch_to_block(exit);
            builder.seal_block(exit);

            cx.last_expr_type = Some(clif_types::default_int_type());
            cx.last_heap_array = true;
            Ok(result_arr)
        }
        "reduce" => {
            // arr.reduce(init, fn_ptr) → fold all elements with fn_ptr(acc, elem)
            if args.len() < 2 {
                return Err(CodegenError::NotImplemented(
                    "array.reduce() requires (init, fn) arguments".into(),
                ));
            }
            let init_val = compile_expr(builder, cx, &args[0].value)?;
            let fn_ptr = compile_expr(builder, cx, &args[1].value)?;

            // Get source array length
            let len_id = *cx
                .functions
                .get("__array_len")
                .ok_or_else(|| CodegenError::Internal("__array_len not declared".into()))?;
            let len_callee = cx.module.declare_func_in_func(len_id, builder.func);
            let len_call = builder.ins().call(len_callee, &[arr_ptr]);
            let arr_len = builder.inst_results(len_call)[0];

            // Loop: acc = fn_ptr(acc, arr[i]) for i in 0..len
            let header = builder.create_block();
            let body = builder.create_block();
            let exit = builder.create_block();

            let idx_var = builder.declare_var(clif_types::default_int_type());
            let acc_var = builder.declare_var(clif_types::default_int_type());
            let zero = builder.ins().iconst(clif_types::default_int_type(), 0);
            builder.def_var(idx_var, zero);
            builder.def_var(acc_var, init_val);

            builder.ins().jump(header, &[]);
            builder.switch_to_block(header);

            let idx = builder.use_var(idx_var);
            let cond = builder.ins().icmp(
                cranelift_codegen::ir::condcodes::IntCC::SignedLessThan,
                idx,
                arr_len,
            );
            builder.ins().brif(cond, body, &[], exit, &[]);

            builder.switch_to_block(body);
            builder.seal_block(body);

            // Get element
            let get_id = *cx
                .functions
                .get("__array_get")
                .ok_or_else(|| CodegenError::Internal("__array_get not declared".into()))?;
            let get_callee = cx.module.declare_func_in_func(get_id, builder.func);
            let get_call = builder.ins().call(get_callee, &[arr_ptr, idx]);
            let elem = builder.inst_results(get_call)[0];

            // Call fn_ptr(acc, elem)
            let sig = builder
                .func
                .import_signature(cranelift_codegen::ir::Signature {
                    params: vec![
                        cranelift_codegen::ir::AbiParam::new(clif_types::default_int_type()),
                        cranelift_codegen::ir::AbiParam::new(clif_types::default_int_type()),
                    ],
                    returns: vec![cranelift_codegen::ir::AbiParam::new(
                        clif_types::default_int_type(),
                    )],
                    call_conv: cranelift_codegen::isa::CallConv::SystemV,
                });
            let acc = builder.use_var(acc_var);
            let fold_call = builder.ins().call_indirect(sig, fn_ptr, &[acc, elem]);
            let new_acc = builder.inst_results(fold_call)[0];
            builder.def_var(acc_var, new_acc);

            // Increment index
            let one = builder.ins().iconst(clif_types::default_int_type(), 1);
            let next_idx = builder.ins().iadd(idx, one);
            builder.def_var(idx_var, next_idx);
            builder.ins().jump(header, &[]);

            builder.seal_block(header);
            builder.switch_to_block(exit);
            builder.seal_block(exit);

            let final_acc = builder.use_var(acc_var);
            cx.last_expr_type = Some(clif_types::default_int_type());
            Ok(final_acc)
        }
        _ => Err(CodegenError::NotImplemented(format!(
            "heap array method '.{method}()'"
        ))),
    }
}

/// Compiles stack array method calls: len, first, last, reverse.
pub(in crate::codegen::cranelift) fn compile_stack_array_method<M: Module>(
    builder: &mut FunctionBuilder,
    cx: &mut CodegenCtx<'_, M>,
    name: &str,
    method: &str,
) -> Result<ClifValue, CodegenError> {
    let (slot, len) = cx.array_meta.get(name).copied().ok_or_else(|| {
        CodegenError::NotImplemented(format!("array '{name}' metadata not found"))
    })?;
    match method {
        "len" => {
            cx.last_expr_type = Some(clif_types::default_int_type());
            Ok(builder
                .ins()
                .iconst(clif_types::default_int_type(), len as i64))
        }
        "first" => {
            // Returns Option-like: tag=1 (Some) + payload=first element, or tag=0 (None)
            if len == 0 {
                cx.last_enum_payload =
                    Some(builder.ins().iconst(clif_types::default_int_type(), 0));
                cx.last_enum_payload_type = Some(clif_types::default_int_type());
                cx.last_expr_type = Some(clif_types::default_int_type());
                return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
            }
            let ptr = builder
                .ins()
                .stack_addr(clif_types::pointer_type(), slot, 0);
            let elem = builder.ins().load(
                clif_types::default_int_type(),
                cranelift_codegen::ir::MemFlags::new(),
                ptr,
                0,
            );
            cx.last_enum_payload = Some(elem);
            cx.last_enum_payload_type = Some(clif_types::default_int_type());
            cx.last_expr_type = Some(clif_types::default_int_type());
            // tag=1 means Some
            Ok(builder.ins().iconst(clif_types::default_int_type(), 1))
        }
        "last" => {
            if len == 0 {
                cx.last_enum_payload =
                    Some(builder.ins().iconst(clif_types::default_int_type(), 0));
                cx.last_enum_payload_type = Some(clif_types::default_int_type());
                cx.last_expr_type = Some(clif_types::default_int_type());
                return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
            }
            let offset = ((len - 1) * 8) as i32;
            let ptr = builder
                .ins()
                .stack_addr(clif_types::pointer_type(), slot, 0);
            let elem = builder.ins().load(
                clif_types::default_int_type(),
                cranelift_codegen::ir::MemFlags::new(),
                ptr,
                offset,
            );
            cx.last_enum_payload = Some(elem);
            cx.last_enum_payload_type = Some(clif_types::default_int_type());
            cx.last_expr_type = Some(clif_types::default_int_type());
            Ok(builder.ins().iconst(clif_types::default_int_type(), 1))
        }
        "reverse" => {
            // In-place reverse of stack array elements
            let ptr = builder
                .ins()
                .stack_addr(clif_types::pointer_type(), slot, 0);
            for i in 0..len / 2 {
                let j = len - 1 - i;
                let off_i = (i * 8) as i32;
                let off_j = (j * 8) as i32;
                let val_i = builder.ins().load(
                    clif_types::default_int_type(),
                    cranelift_codegen::ir::MemFlags::new(),
                    ptr,
                    off_i,
                );
                let val_j = builder.ins().load(
                    clif_types::default_int_type(),
                    cranelift_codegen::ir::MemFlags::new(),
                    ptr,
                    off_j,
                );
                builder
                    .ins()
                    .store(cranelift_codegen::ir::MemFlags::new(), val_j, ptr, off_i);
                builder
                    .ins()
                    .store(cranelift_codegen::ir::MemFlags::new(), val_i, ptr, off_j);
            }
            cx.last_expr_type = Some(clif_types::default_int_type());
            Ok(builder.ins().iconst(clif_types::default_int_type(), 0))
        }
        "is_empty" => {
            // Stack array length is known at compile time
            cx.last_expr_type = Some(clif_types::default_int_type());
            let val = if len == 0 { 1 } else { 0 };
            Ok(builder.ins().iconst(clif_types::default_int_type(), val))
        }
        "contains" => {
            // Contains needs the argument from the method call, but compile_stack_array_method
            // doesn't receive args — handled in compile_method_call instead.
            // This shouldn't be reached; fall through to error.
            Err(CodegenError::NotImplemented(
                "stack array .contains() — should be handled in compile_method_call".into(),
            ))
        }
        _ => Err(CodegenError::NotImplemented(format!(
            "stack array method '.{method}()'"
        ))),
    }
}

/// Compiles `arr.contains(value)` for stack arrays (inline element scan).
pub(in crate::codegen::cranelift) fn compile_stack_array_contains<M: Module>(
    builder: &mut FunctionBuilder,
    cx: &mut CodegenCtx<'_, M>,
    name: &str,
    args: &[CallArg],
) -> Result<ClifValue, CodegenError> {
    if args.is_empty() {
        return Err(CodegenError::NotImplemented(
            "array.contains() requires 1 argument".into(),
        ));
    }
    let (slot, len) = cx.array_meta.get(name).copied().ok_or_else(|| {
        CodegenError::NotImplemented(format!("array '{name}' metadata not found"))
    })?;
    let needle = compile_expr(builder, cx, &args[0].value)?;
    let ptr = builder
        .ins()
        .stack_addr(clif_types::pointer_type(), slot, 0);

    // Inline scan: compare each element. Result variable starts at 0 (false).
    let result_var = builder.declare_var(clif_types::bool_type());
    let zero_val = builder.ins().iconst(clif_types::bool_type(), 0);
    builder.def_var(result_var, zero_val);

    for i in 0..len {
        let offset = (i * 8) as i32;
        let elem = builder.ins().load(
            clif_types::default_int_type(),
            cranelift_codegen::ir::MemFlags::new(),
            ptr,
            offset,
        );
        let cmp = builder.ins().icmp(IntCC::Equal, elem, needle);
        // If found, set result to 1
        let found_block = builder.create_block();
        let cont_block = builder.create_block();
        builder.ins().brif(cmp, found_block, &[], cont_block, &[]);
        builder.switch_to_block(found_block);
        builder.seal_block(found_block);
        let one_val = builder.ins().iconst(clif_types::bool_type(), 1);
        builder.def_var(result_var, one_val);
        builder.ins().jump(cont_block, &[]);
        builder.switch_to_block(cont_block);
        builder.seal_block(cont_block);
    }
    cx.last_expr_type = Some(clif_types::bool_type());
    Ok(builder.use_var(result_var))
}

/// Compiles `arr.join(sep)` for stack arrays by creating a temp heap array.
pub(in crate::codegen::cranelift) fn compile_stack_array_join<M: Module>(
    builder: &mut FunctionBuilder,
    cx: &mut CodegenCtx<'_, M>,
    name: &str,
    args: &[CallArg],
) -> Result<ClifValue, CodegenError> {
    if args.is_empty() {
        return Err(CodegenError::NotImplemented(
            "array.join() requires 1 argument".into(),
        ));
    }
    let (slot, len) = cx.array_meta.get(name).copied().ok_or_else(|| {
        CodegenError::NotImplemented(format!("array '{name}' metadata not found"))
    })?;

    // Create temp heap array, push all elements
    let new_id = *cx
        .functions
        .get("__array_new")
        .ok_or_else(|| CodegenError::Internal("__array_new not declared".into()))?;
    let new_callee = cx.module.declare_func_in_func(new_id, builder.func);
    let cap = builder
        .ins()
        .iconst(clif_types::default_int_type(), len as i64);
    let call = builder.ins().call(new_callee, &[cap]);
    let tmp_arr = builder.inst_results(call)[0];

    let push_id = *cx
        .functions
        .get("__array_push")
        .ok_or_else(|| CodegenError::Internal("__array_push not declared".into()))?;
    let push_callee = cx.module.declare_func_in_func(push_id, builder.func);
    let arr_ptr = builder
        .ins()
        .stack_addr(clif_types::pointer_type(), slot, 0);
    for i in 0..len {
        let offset = (i * 8) as i32;
        let elem = builder.ins().load(
            clif_types::default_int_type(),
            cranelift_codegen::ir::MemFlags::new(),
            arr_ptr,
            offset,
        );
        builder.ins().call(push_callee, &[tmp_arr, elem]);
    }

    // Compile separator
    let sep = compile_expr(builder, cx, &args[0].value)?;
    let sep_len = cx
        .last_string_len
        .take()
        .unwrap_or_else(|| builder.ins().iconst(clif_types::default_int_type(), 0));

    // Call fj_rt_array_join
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

    let join_id = *cx
        .functions
        .get("__array_join")
        .ok_or_else(|| CodegenError::Internal("__array_join not declared".into()))?;
    let join_callee = cx.module.declare_func_in_func(join_id, builder.func);
    builder.ins().call(
        join_callee,
        &[tmp_arr, sep, sep_len, out_ptr_addr, out_len_addr],
    );

    // Free temp array
    let free_id = *cx
        .functions
        .get("__array_free")
        .ok_or_else(|| CodegenError::Internal("__array_free not declared".into()))?;
    let free_callee = cx.module.declare_func_in_func(free_id, builder.func);
    builder.ins().call(free_callee, &[tmp_arr]);

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
