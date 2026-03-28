//! Expression compilation for Fajar Lang native codegen.
//!
//! Contains `compile_expr` and all pure-expression helpers:
//! literals, idents, paths, unary ops, binary ops, tuples, casts, short-circuit logic.

use cranelift_codegen::ir::condcodes::IntCC;
use cranelift_codegen::ir::{InstBuilder, Value as ClifValue};
use cranelift_frontend::FunctionBuilder;
use cranelift_module::Module;

use super::super::clif_types;
use super::super::context::{CodegenCtx, emit_owned_cleanup, emit_scope_cleanup};
use crate::codegen::CodegenError;
use crate::lexer::token::Span;
use crate::parser::ast::{BinOp, Expr, LiteralKind, UnaryOp};

// Re-use sibling functions via the parent module's re-exports.
use super::{
    // arrays.rs
    compile_array_literal,
    compile_call,
    compile_field_access,
    compile_field_assign,
    // control.rs
    compile_for,
    compile_if,
    compile_index,
    compile_index_assign,
    compile_inline_asm,
    compile_loop,
    compile_match,
    compile_method_call,
    compile_stmt,
    // strings.rs
    compile_string_concat,
    compile_string_concat_vals,
    compile_string_literal,
    // structs.rs
    compile_struct_init,
    compile_while,
    is_string_producing_expr,
    resolve_variant_tag,
};

/// Compiles an expression to a Cranelift IR value.
pub(in crate::codegen::cranelift) fn compile_expr<M: Module>(
    builder: &mut FunctionBuilder,
    cx: &mut CodegenCtx<'_, M>,
    expr: &Expr,
) -> Result<ClifValue, CodegenError> {
    match expr {
        Expr::Literal { kind, .. } => compile_literal(builder, cx, kind),
        Expr::Ident { name, .. } => compile_ident(builder, cx, name),
        Expr::Path { segments, .. } => compile_path(builder, cx, segments),
        Expr::Binary {
            op, left, right, ..
        } => compile_binop(builder, cx, op, left, right),
        Expr::Unary { op, operand, .. } => compile_unary(builder, cx, op, operand),
        Expr::Call { callee, args, .. } => compile_call(builder, cx, callee, args),
        Expr::Block {
            stmts, expr: tail, ..
        } => {
            // S3.2: Push a new scope for block-level RAII cleanup.
            // The function body's sentinel scope (depth 0) is pushed in define_function
            // and is NOT cleaned up here — function-level cleanup uses emit_owned_cleanup.
            // Only nested blocks (depth > 0) get scope cleanup.
            let depth_before = cx.scope_stack.len();
            cx.scope_stack.push(Vec::new());
            let mut last = builder.ins().iconst(clif_types::default_int_type(), 0);
            for stmt in stmts {
                if let Some(val) = compile_stmt(builder, cx, stmt)? {
                    last = val;
                }
            }
            if let Some(tail_expr) = tail {
                last = compile_expr(builder, cx, tail_expr)?;
            }
            // S3.3: Cleanup scope-local resources (skip freeing the tail value).
            // Only clean up nested scopes (depth > 1 means we're inside the fn body).
            if depth_before >= 1 {
                emit_scope_cleanup(builder, cx, Some(last))?;
            } else {
                // Function body scope — just pop without cleanup
                cx.scope_stack.pop();
            }
            Ok(last)
        }
        Expr::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => compile_if(builder, cx, condition, then_branch, else_branch),
        Expr::Grouped { expr: inner, .. } => compile_expr(builder, cx, inner),
        Expr::While {
            label,
            condition,
            body,
            ..
        } => compile_while(builder, cx, condition, body, label.as_deref()),
        Expr::For {
            label,
            variable,
            iterable,
            body,
            ..
        } => compile_for(builder, cx, variable, iterable, body, label.as_deref()),
        Expr::Loop { label, body, .. } => compile_loop(builder, cx, body, label.as_deref()),
        Expr::Array { elements, .. } => compile_array_literal(builder, cx, elements),
        Expr::Index { object, index, .. } => compile_index(builder, cx, object, index),
        Expr::Assign {
            target, op, value, ..
        } => {
            use crate::parser::ast::AssignOp;
            if let Expr::Ident { name, .. } = target.as_ref() {
                let rhs = compile_expr(builder, cx, value)?;
                if let Some(&var) = cx.var_map.get(name) {
                    let is_float = cx
                        .var_types
                        .get(name)
                        .copied()
                        .is_some_and(clif_types::is_float);
                    let final_val = match op {
                        AssignOp::Assign => rhs,
                        _ => {
                            let current = builder.use_var(var);
                            if is_float {
                                match op {
                                    AssignOp::AddAssign => builder.ins().fadd(current, rhs),
                                    AssignOp::SubAssign => builder.ins().fsub(current, rhs),
                                    AssignOp::MulAssign => builder.ins().fmul(current, rhs),
                                    AssignOp::DivAssign => builder.ins().fdiv(current, rhs),
                                    _ => {
                                        return Err(CodegenError::NotImplemented(
                                            "float compound assign".into(),
                                        ));
                                    }
                                }
                            } else {
                                match op {
                                    AssignOp::AddAssign => builder.ins().iadd(current, rhs),
                                    AssignOp::SubAssign => builder.ins().isub(current, rhs),
                                    AssignOp::MulAssign => builder.ins().imul(current, rhs),
                                    AssignOp::DivAssign => builder.ins().sdiv(current, rhs),
                                    AssignOp::RemAssign => builder.ins().srem(current, rhs),
                                    AssignOp::BitAndAssign => builder.ins().band(current, rhs),
                                    AssignOp::BitOrAssign => builder.ins().bor(current, rhs),
                                    AssignOp::BitXorAssign => builder.ins().bxor(current, rhs),
                                    AssignOp::ShlAssign => builder.ins().ishl(current, rhs),
                                    AssignOp::ShrAssign => builder.ins().sshr(current, rhs),
                                    AssignOp::Assign => unreachable!(),
                                }
                            }
                        }
                    };
                    builder.def_var(var, final_val);
                    // When a heap array is reassigned from another heap array variable
                    // (e.g., `values = new_vals`), remove the target from owned_ptrs
                    // to prevent double-free at cleanup (both would alias the same ptr).
                    if matches!(op, AssignOp::Assign) && cx.heap_arrays.contains(name) {
                        if let Expr::Ident { name: rhs_name, .. } = value.as_ref() {
                            if cx.heap_arrays.contains(rhs_name) {
                                cx.owned_ptrs.retain(|(n, _)| n != name);
                            }
                        }
                    }
                    // Update string length tracking on reassignment
                    if let Some(len_val) = cx.last_string_len.take() {
                        if let Some(&existing_len_var) = cx.string_lens.get(name) {
                            // Reuse existing length variable
                            builder.def_var(existing_len_var, len_val);
                        } else {
                            // Create new length variable
                            let len_var = builder.declare_var(clif_types::default_int_type());
                            builder.def_var(len_var, len_val);
                            cx.string_lens.insert(name.clone(), len_var);
                        }
                    }
                    Ok(final_val)
                } else {
                    Err(CodegenError::UndefinedVariable(name.clone()))
                }
            } else if let Expr::Index { object, index, .. } = target.as_ref() {
                compile_index_assign(builder, cx, object, index, value, op)
            } else if let Expr::Field {
                object,
                field: fname,
                ..
            } = target.as_ref()
            {
                compile_field_assign(builder, cx, object, fname, value, op)
            } else {
                Err(CodegenError::UnsupportedExpr(
                    "complex assign target".into(),
                ))
            }
        }
        Expr::MethodCall {
            receiver,
            method,
            args,
            ..
        } => compile_method_call(builder, cx, receiver, method, args),
        Expr::Match { subject, arms, .. } => compile_match(builder, cx, subject, arms),
        Expr::StructInit { name, fields, .. } => compile_struct_init(builder, cx, name, fields),
        Expr::Field { object, field, .. } => compile_field_access(builder, cx, object, field),
        Expr::Tuple { elements, .. } => compile_tuple(builder, cx, elements),
        Expr::Cast { expr, ty, .. } => compile_cast(builder, cx, expr, ty),
        Expr::Pipe { left, right, .. } => {
            // `x |> f` desugars to `f(x)`, `x |> f(y)` desugars to `f(x, y)`
            let arg_val = compile_expr(builder, cx, left)?;
            let (fn_name, mut extra_args) = match right.as_ref() {
                Expr::Ident { name, .. } => (name.clone(), Vec::new()),
                Expr::Call { callee, args, .. } => {
                    let name = match callee.as_ref() {
                        Expr::Ident { name, .. } => name.clone(),
                        _ => {
                            return Err(CodegenError::NotImplemented(
                                "pipe to non-ident callee".into(),
                            ));
                        }
                    };
                    let mut compiled_args = Vec::new();
                    for a in args {
                        compiled_args.push(compile_expr(builder, cx, &a.value)?);
                    }
                    (name, compiled_args)
                }
                _ => {
                    return Err(CodegenError::NotImplemented(
                        "pipe to non-ident/call expression".into(),
                    ));
                }
            };
            let func_id = *cx
                .functions
                .get(&fn_name)
                .ok_or_else(|| CodegenError::UndefinedVariable(fn_name.clone()))?;
            let local_callee = cx.module.declare_func_in_func(func_id, builder.func);
            // Prepend the piped value as the first argument
            let mut all_args = vec![arg_val];
            all_args.append(&mut extra_args);
            let call = builder.ins().call(local_callee, &all_args);
            let results = builder.inst_results(call);
            // Set last_expr_type from the piped function's return type
            if let Some(&ret_ty) = cx.fn_return_types.get(&fn_name) {
                cx.last_expr_type = Some(ret_ty);
            } else {
                cx.last_expr_type = Some(clif_types::default_int_type());
            }
            if results.is_empty() {
                Ok(builder.ins().iconst(clif_types::default_int_type(), 0))
            } else {
                Ok(results[0])
            }
        }
        Expr::Try { expr: inner, .. } => {
            // Compile the inner expression (should produce an enum: tag + payload)
            let tag_val = compile_expr(builder, cx, inner)?;
            let payload = cx
                .last_enum_payload
                .take()
                .unwrap_or_else(|| builder.ins().iconst(clif_types::default_int_type(), 0));
            let payload_type = cx
                .last_enum_payload_type
                .take()
                .unwrap_or(clif_types::default_int_type());

            // Check if tag == 1 (Err) or tag != 0 (None)
            let is_err = builder.ins().icmp_imm(IntCC::NotEqual, tag_val, 0);
            let ok_block = builder.create_block();
            let err_block = builder.create_block();
            builder.ins().brif(is_err, err_block, &[], ok_block, &[]);

            // Err block: emit cleanup and return the error
            builder.switch_to_block(err_block);
            builder.seal_block(err_block);
            emit_owned_cleanup(builder, cx, None)?;
            if cx.is_enum_return_fn {
                // Typed Result propagation: return (tag, payload) so caller gets proper Result
                builder.ins().return_(&[tag_val, payload]);
            } else {
                // Simple error propagation: return just the error value
                builder.ins().return_(&[payload]);
            }

            // Ok block: continue with the unwrapped payload
            builder.switch_to_block(ok_block);
            builder.seal_block(ok_block);
            cx.last_expr_type = Some(payload_type);
            Ok(payload)
        }
        Expr::Closure { span, .. } => {
            // Closure expressions are compiled as separate functions during pre-scan.
            // Look up the pre-scanned function name by span and return its address.
            if let Some(fn_name) = cx.closure_span_to_fn.get(&(span.start, span.end)).cloned() {
                if let Some(&func_id) = cx.functions.get(&fn_name) {
                    let func_ref = cx.module.declare_func_in_func(func_id, builder.func);
                    let addr = builder
                        .ins()
                        .func_addr(clif_types::pointer_type(), func_ref);

                    // If closure has captures, package into a ClosureHandle
                    let captures = cx.closure_captures.get(&fn_name).cloned();
                    if let Some(ref caps) = captures {
                        if !caps.is_empty() {
                            let cap_count = builder
                                .ins()
                                .iconst(clif_types::default_int_type(), caps.len() as i64);
                            let new_id =
                                *cx.functions.get("__closure_handle_new").ok_or_else(|| {
                                    CodegenError::Internal(
                                        "__closure_handle_new not declared".into(),
                                    )
                                })?;
                            let callee = cx.module.declare_func_in_func(new_id, builder.func);
                            let call = builder.ins().call(callee, &[addr, cap_count]);
                            let handle = builder.inst_results(call)[0];

                            // Store each captured variable value
                            let set_id =
                                *cx.functions.get("__closure_set_capture").ok_or_else(|| {
                                    CodegenError::Internal(
                                        "__closure_set_capture not declared".into(),
                                    )
                                })?;
                            let set_callee = cx.module.declare_func_in_func(set_id, builder.func);
                            for (i, cap_name) in caps.iter().enumerate() {
                                let idx = builder
                                    .ins()
                                    .iconst(clif_types::default_int_type(), i as i64);
                                let val = if let Some(&var) = cx.var_map.get(cap_name) {
                                    builder.use_var(var)
                                } else {
                                    builder.ins().iconst(clif_types::default_int_type(), 0)
                                };
                                builder.ins().call(set_callee, &[handle, idx, val]);
                            }

                            cx.last_expr_type = Some(clif_types::pointer_type());
                            cx.last_closure_handle = true;
                            return Ok(handle);
                        }
                    }

                    cx.last_expr_type = Some(clif_types::pointer_type());
                    return Ok(addr);
                }
            }
            // Fallback: return sentinel 0 (e.g., for let-bound closures handled elsewhere)
            Ok(builder.ins().iconst(clif_types::default_int_type(), 0))
        }
        Expr::InlineAsm {
            template,
            operands,
            options,
            clobber_abi,
            ..
        } => compile_inline_asm(builder, cx, template, operands, options, clobber_abi),
        Expr::Await { expr: inner, .. } => {
            // Compile the inner expression (produces a future handle pointer)
            let future_ptr = compile_expr(builder, cx, inner)?;

            // Poll the future: spin-poll until Ready (eager model: always Ready on first poll)
            let poll_block = builder.create_block();
            let ready_block = builder.create_block();

            builder.ins().jump(poll_block, &[]);
            builder.switch_to_block(poll_block);

            let poll_id = cx
                .functions
                .get("__future_poll")
                .ok_or_else(|| CodegenError::Internal("__future_poll not declared".into()))?;
            let poll_callee = cx.module.declare_func_in_func(*poll_id, builder.func);
            let poll_call = builder.ins().call(poll_callee, &[future_ptr]);
            let poll_result = builder.inst_results(poll_call)[0];
            let one = builder.ins().iconst(cranelift_codegen::ir::types::I64, 1);
            let is_ready = builder.ins().icmp(
                cranelift_codegen::ir::condcodes::IntCC::Equal,
                poll_result,
                one,
            );
            builder
                .ins()
                .brif(is_ready, ready_block, &[], poll_block, &[]);

            builder.seal_block(poll_block);
            builder.switch_to_block(ready_block);
            builder.seal_block(ready_block);

            // Extract the result from the ready future
            let get_result_id = cx
                .functions
                .get("__future_get_result")
                .ok_or_else(|| CodegenError::Internal("__future_get_result not declared".into()))?;
            let get_callee = cx.module.declare_func_in_func(*get_result_id, builder.func);
            let call_inst = builder.ins().call(get_callee, &[future_ptr]);
            let result = builder.inst_results(call_inst)[0];

            // Free the future handle
            let free_id = cx
                .functions
                .get("__future_free")
                .ok_or_else(|| CodegenError::Internal("__future_free not declared".into()))?;
            let free_callee = cx.module.declare_func_in_func(*free_id, builder.func);
            builder.ins().call(free_callee, &[future_ptr]);

            Ok(result)
        }
        Expr::AsyncBlock { body, .. } => {
            // async { body } → create future, compile body, set result, return future_ptr
            let new_id = cx
                .functions
                .get("__future_new")
                .ok_or_else(|| CodegenError::Internal("__future_new not declared".into()))?;
            let new_callee = cx.module.declare_func_in_func(*new_id, builder.func);
            let new_call = builder.ins().call(new_callee, &[]);
            let future_ptr = builder.inst_results(new_call)[0];

            let result = compile_expr(builder, cx, body)?;

            let set_id = cx
                .functions
                .get("__future_set_result")
                .ok_or_else(|| CodegenError::Internal("__future_set_result not declared".into()))?;
            let set_callee = cx.module.declare_func_in_func(*set_id, builder.func);
            builder.ins().call(set_callee, &[future_ptr, result]);

            cx.last_future_new = true;
            Ok(future_ptr)
        }
        Expr::FString { parts, .. } => {
            // Convert f-string to format() call:
            // f"Hello {name}, age {age}" → format("Hello {}, age {}", name, age)
            use crate::parser::ast::{CallArg, FStringExprPart};

            // Build template string: replace {expr} with {}
            let mut template = String::new();
            let mut format_args: Vec<CallArg> = Vec::new();

            for part in parts {
                match part {
                    FStringExprPart::Literal(s) => template.push_str(s),
                    FStringExprPart::Expr(e) => {
                        template.push_str("{}");
                        format_args.push(CallArg {
                            name: None,
                            value: *e.clone(),
                            span: Span::new(0, 0),
                        });
                    }
                }
            }

            // Build args: [template_literal, arg1, arg2, ...]
            let dummy_span = Span::new(0, 0);
            let mut all_args = vec![CallArg {
                name: None,
                value: Expr::Literal {
                    kind: LiteralKind::String(template),
                    span: dummy_span,
                },
                span: dummy_span,
            }];
            for mut fa in format_args {
                fa.span = dummy_span;
                all_args.push(fa);
            }

            // Delegate to compile_format_builtin
            super::compile_format_builtin(builder, cx, &all_args)
        }
        _ => Err(CodegenError::UnsupportedExpr(format!(
            "{:?}",
            std::mem::discriminant(expr)
        ))),
    }
}

/// Compiles a tuple expression like `(a, b, c)`.
///
/// Stores each element in a stack slot at 8-byte offsets, returns pointer.
pub(in crate::codegen::cranelift) fn compile_tuple<M: Module>(
    builder: &mut FunctionBuilder,
    cx: &mut CodegenCtx<'_, M>,
    elements: &[Expr],
) -> Result<ClifValue, CodegenError> {
    let num = elements.len() as u32;
    let slot_size = num.checked_mul(8).ok_or_else(|| {
        CodegenError::NotImplemented(format!(
            "tuple has too many elements ({num}) for stack allocation"
        ))
    })?;
    let slot_data = cranelift_codegen::ir::StackSlotData::new(
        cranelift_codegen::ir::StackSlotKind::ExplicitSlot,
        slot_size,
        0,
    );
    let slot = builder.create_sized_stack_slot(slot_data);

    let mut elem_types = Vec::with_capacity(elements.len());
    for (i, elem) in elements.iter().enumerate() {
        let val = compile_expr(builder, cx, elem)?;
        let elem_type = cx.last_expr_type.unwrap_or(clif_types::default_int_type());
        elem_types.push(elem_type);
        let offset = (i as i32) * 8;
        builder.ins().stack_store(
            val,
            slot,
            cranelift_codegen::ir::immediates::Offset32::new(offset),
        );
    }

    cx.last_tuple_elem_types = Some(elem_types);

    // Track as a tuple via struct_slots with a synthetic name
    let tuple_name = format!("__tuple_{num}");
    if !cx.struct_defs.contains_key(&tuple_name) {
        // We can't insert into struct_defs (immutable ref), so track tuple size
        // via the last_struct_init side-channel
    }
    cx.last_struct_init = Some((slot, tuple_name));

    let ptr = builder
        .ins()
        .stack_addr(clif_types::pointer_type(), slot, 0);
    cx.last_expr_type = Some(clif_types::pointer_type());
    Ok(ptr)
}

/// Compiles an `as` type cast expression.
///
/// Supports i64 ↔ f64 conversions.
pub(in crate::codegen::cranelift) fn compile_cast<M: Module>(
    builder: &mut FunctionBuilder,
    cx: &mut CodegenCtx<'_, M>,
    expr: &Expr,
    ty: &crate::parser::ast::TypeExpr,
) -> Result<ClifValue, CodegenError> {
    let val = compile_expr(builder, cx, expr)?;
    let src_is_float = cx.last_expr_type.is_some_and(clif_types::is_float);

    let target_name = match ty {
        crate::parser::ast::TypeExpr::Simple { name, .. } => name.as_str(),
        _ => return Err(CodegenError::NotImplemented("cast to complex type".into())),
    };

    match target_name {
        "f64" | "f32" => {
            if src_is_float {
                cx.last_expr_type = Some(cranelift_codegen::ir::types::F64);
                Ok(val)
            } else {
                let result = builder
                    .ins()
                    .fcvt_from_sint(cranelift_codegen::ir::types::F64, val);
                cx.last_expr_type = Some(cranelift_codegen::ir::types::F64);
                Ok(result)
            }
        }
        "i64" | "i32" | "i16" | "i8" | "isize" | "usize" | "u64" | "u32" | "u16" | "u8" => {
            if src_is_float {
                let result = builder
                    .ins()
                    .fcvt_to_sint(clif_types::default_int_type(), val);
                cx.last_expr_type = Some(clif_types::default_int_type());
                Ok(result)
            } else {
                let target_ty = clif_types::lower_simple_type(target_name)
                    .unwrap_or(clif_types::default_int_type());
                let src_ty = builder.func.dfg.value_type(val);

                let result = if target_ty.bits() < src_ty.bits() {
                    // Narrowing cast: truncate (e.g., i64 → u8 via ireduce)
                    let narrow = builder.ins().ireduce(target_ty, val);
                    // Extend back to i64 for uniform value representation
                    match target_name {
                        "i8" | "i16" | "i32" => builder
                            .ins()
                            .sextend(clif_types::default_int_type(), narrow),
                        _ => builder
                            .ins()
                            .uextend(clif_types::default_int_type(), narrow),
                    }
                } else if target_ty.bits() > src_ty.bits() {
                    // Widening cast: extend
                    match target_name {
                        "i16" | "i32" | "i64" | "isize" => builder.ins().sextend(target_ty, val),
                        _ => builder.ins().uextend(target_ty, val),
                    }
                } else {
                    // Same width: no-op
                    val
                };
                cx.last_expr_type = Some(clif_types::default_int_type());
                Ok(result)
            }
        }
        "bool" => {
            if src_is_float {
                // f64 → bool: val != 0.0
                let zero = builder.ins().f64const(0.0);
                let result = builder.ins().fcmp(
                    cranelift_codegen::ir::condcodes::FloatCC::NotEqual,
                    val,
                    zero,
                );
                let ext = builder
                    .ins()
                    .uextend(clif_types::default_int_type(), result);
                cx.last_expr_type = Some(clif_types::default_int_type());
                Ok(ext)
            } else {
                // int → bool: val != 0
                let zero = builder.ins().iconst(clif_types::default_int_type(), 0);
                let result = builder.ins().icmp(IntCC::NotEqual, val, zero);
                let ext = builder
                    .ins()
                    .uextend(clif_types::default_int_type(), result);
                cx.last_expr_type = Some(clif_types::default_int_type());
                Ok(ext)
            }
        }
        _ => Err(CodegenError::NotImplemented(format!(
            "cast to '{target_name}'"
        ))),
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Literal, Ident, Path, Unary compilation
// ═══════════════════════════════════════════════════════════════════════

/// Compiles a literal value to a Cranelift IR value.
pub(in crate::codegen::cranelift) fn compile_literal<M: Module>(
    builder: &mut FunctionBuilder,
    cx: &mut CodegenCtx<'_, M>,
    kind: &LiteralKind,
) -> Result<ClifValue, CodegenError> {
    match kind {
        LiteralKind::Int(n) => {
            cx.last_expr_type = Some(clif_types::default_int_type());
            Ok(builder.ins().iconst(clif_types::default_int_type(), *n))
        }
        LiteralKind::Float(f) => {
            cx.last_expr_type = Some(clif_types::default_float_type());
            Ok(builder.ins().f64const(*f))
        }
        LiteralKind::Bool(b) => {
            cx.last_expr_type = Some(clif_types::bool_type());
            Ok(builder
                .ins()
                .iconst(clif_types::bool_type(), if *b { 1 } else { 0 }))
        }
        LiteralKind::Char(c) => {
            cx.last_expr_type = Some(clif_types::default_int_type());
            Ok(builder
                .ins()
                .iconst(clif_types::default_int_type(), *c as i64))
        }
        LiteralKind::Null => {
            cx.last_expr_type = Some(clif_types::default_int_type());
            Ok(builder.ins().iconst(clif_types::default_int_type(), 0))
        }
        LiteralKind::String(s) | LiteralKind::RawString(s) => {
            compile_string_literal(builder, cx, s)
        }
    }
}

/// Compiles a variable reference (identifier lookup).
pub(in crate::codegen::cranelift) fn compile_ident<M: Module>(
    builder: &mut FunctionBuilder,
    cx: &mut CodegenCtx<'_, M>,
    name: &str,
) -> Result<ClifValue, CodegenError> {
    // Boolean literals
    if name == "true" {
        cx.last_expr_type = Some(clif_types::bool_type());
        return Ok(builder.ins().iconst(clif_types::bool_type(), 1));
    }
    if name == "false" {
        cx.last_expr_type = Some(clif_types::bool_type());
        return Ok(builder.ins().iconst(clif_types::bool_type(), 0));
    }
    // Null-like / unit enum variants
    if name == "None" {
        cx.last_expr_type = Some(clif_types::default_int_type());
        cx.last_enum_payload = Some(builder.ins().iconst(clif_types::default_int_type(), 0));
        cx.last_enum_payload_type = Some(clif_types::default_int_type());
        return Ok(builder.ins().iconst(clif_types::default_int_type(), 0)); // tag=0
    }

    // Check for bare enum variant (e.g., `Green` for `enum Color { Red, Green, Blue }`)
    for (_enum_name, variants) in cx.enum_defs.iter() {
        if let Some(tag_idx) = variants.iter().position(|v| v == name) {
            cx.last_expr_type = Some(clif_types::default_int_type());
            cx.last_enum_payload = Some(builder.ins().iconst(clif_types::default_int_type(), 0));
            cx.last_enum_payload_type = Some(clif_types::default_int_type());
            return Ok(builder
                .ins()
                .iconst(clif_types::default_int_type(), tag_idx as i64));
        }
    }

    // Closure variable: resolve to the lifted function's address
    // BUT if this variable holds a ClosureHandle, use the stored value instead
    if !cx.closure_handle_vars.contains(name) {
        if let Some(closure_fn_name) = cx.closure_fn_map.get(name).cloned() {
            if let Some(&func_id) = cx.functions.get(&closure_fn_name) {
                let func_ref = cx.module.declare_func_in_func(func_id, builder.func);
                let addr = builder
                    .ins()
                    .func_addr(clif_types::pointer_type(), func_ref);
                cx.last_expr_type = Some(clif_types::pointer_type());
                return Ok(addr);
            }
        }
    }

    // Function reference: function name used as a value → get its address
    if !cx.var_map.contains_key(name) {
        if let Some(&func_id) = cx.functions.get(name) {
            let func_ref = cx.module.declare_func_in_func(func_id, builder.func);
            let addr = builder
                .ins()
                .func_addr(clif_types::pointer_type(), func_ref);
            cx.last_expr_type = Some(clif_types::pointer_type());
            return Ok(addr);
        }
    }

    let var = *cx
        .var_map
        .get(name)
        .ok_or_else(|| CodegenError::UndefinedVariable(name.to_string()))?;
    let val = builder.use_var(var);

    let ty = cx
        .var_types
        .get(name)
        .copied()
        .unwrap_or(clif_types::default_int_type());
    cx.last_expr_type = Some(ty);

    // Propagate string length tracking
    if let Some(&len_var) = cx.string_lens.get(name) {
        cx.last_string_len = Some(builder.use_var(len_var));
        cx.last_string_owned = false;
    }
    // Propagate enum variable tracking
    if let Some((_, payload_var, payload_type)) = cx.enum_vars.get(name) {
        cx.last_enum_payload = Some(builder.use_var(*payload_var));
        cx.last_enum_payload_type = Some(*payload_type);
    }
    // Propagate multi-field enum tracking
    if let Some((slot, field_types)) = cx.enum_multi_vars.get(name) {
        cx.last_enum_multi_payload = Some((*slot, field_types.clone()));
    }
    // Propagate struct slot tracking
    if let Some((slot, sname)) = cx.struct_slots.get(name) {
        cx.last_struct_init = Some((*slot, sname.clone()));
    }
    // Propagate tuple type tracking
    if let Some(elem_types) = cx.tuple_types.get(name) {
        cx.last_tuple_elem_types = Some(elem_types.clone());
    }
    // Propagate split var tracking
    if cx.split_vars.contains(name) {
        cx.last_split_result = Some(val);
    }

    Ok(val)
}

/// Compiles a path expression (e.g., `Enum::Variant`, `Type::method`).
pub(in crate::codegen::cranelift) fn compile_path<M: Module>(
    builder: &mut FunctionBuilder,
    cx: &mut CodegenCtx<'_, M>,
    segments: &[String],
) -> Result<ClifValue, CodegenError> {
    if segments.len() == 2 {
        let type_name = &segments[0];
        let member = &segments[1];

        // Enum variant without payload: EnumName::Variant
        if let Some(variants) = cx.enum_defs.get(type_name) {
            if let Some(tag) = variants.iter().position(|v| v == member) {
                cx.last_expr_type = Some(clif_types::default_int_type());
                cx.last_enum_payload =
                    Some(builder.ins().iconst(clif_types::default_int_type(), 0));
                cx.last_enum_payload_type = Some(clif_types::default_int_type());
                return Ok(builder
                    .ins()
                    .iconst(clif_types::default_int_type(), tag as i64));
            }
        }
    }

    // Built-in Option::None
    if segments.len() == 2 && segments[0] == "Option" && segments[1] == "None" {
        cx.last_expr_type = Some(clif_types::default_int_type());
        cx.last_enum_payload = Some(builder.ins().iconst(clif_types::default_int_type(), 0));
        cx.last_enum_payload_type = Some(clif_types::default_int_type());
        return Ok(builder.ins().iconst(clif_types::default_int_type(), 0));
    }

    // Module-qualified variable/constant: mod::name → check var_map for mod_name
    if segments.len() == 2 {
        let mod_name = &segments[0];
        let member = &segments[1];
        let mangled = format!("{}_{}", mod_name, member);
        if let Some(&var) = cx.var_map.get(&mangled) {
            let val = builder.use_var(var);
            if let Some(&ty) = cx.var_types.get(&mangled) {
                cx.last_expr_type = Some(ty);
            }
            return Ok(val);
        }
        // Check if the member exists as a bare name (const defined without prefix)
        if let Some(&var) = cx.var_map.get(member) {
            let val = builder.use_var(var);
            if let Some(&ty) = cx.var_types.get(member) {
                cx.last_expr_type = Some(ty);
            }
            return Ok(val);
        }
    }

    // Single-segment path: could be an enum variant name
    if segments.len() == 1 {
        let name = &segments[0];
        if let Ok(tag) = resolve_variant_tag(cx, name) {
            cx.last_expr_type = Some(clif_types::default_int_type());
            cx.last_enum_payload = Some(builder.ins().iconst(clif_types::default_int_type(), 0));
            cx.last_enum_payload_type = Some(clif_types::default_int_type());
            return Ok(builder.ins().iconst(clif_types::default_int_type(), tag));
        }
    }

    Err(CodegenError::NotImplemented(format!(
        "path expression '{}'",
        segments.join("::")
    )))
}

/// Compiles a unary operation.
pub(in crate::codegen::cranelift) fn compile_unary<M: Module>(
    builder: &mut FunctionBuilder,
    cx: &mut CodegenCtx<'_, M>,
    op: &UnaryOp,
    operand: &Expr,
) -> Result<ClifValue, CodegenError> {
    let val = compile_expr(builder, cx, operand)?;
    let is_float = cx.last_expr_type.is_some_and(clif_types::is_float);
    let is_bool = cx.last_expr_type == Some(clif_types::bool_type());

    match op {
        UnaryOp::Neg => {
            if is_float {
                cx.last_expr_type = Some(clif_types::default_float_type());
                Ok(builder.ins().fneg(val))
            } else {
                cx.last_expr_type = Some(clif_types::default_int_type());
                Ok(builder.ins().ineg(val))
            }
        }
        UnaryOp::Not => {
            if is_bool {
                // Bool not: XOR with 1
                let one = builder.ins().iconst(clif_types::bool_type(), 1);
                let result = builder.ins().bxor(val, one);
                cx.last_expr_type = Some(clif_types::bool_type());
                Ok(result)
            } else {
                // Integer: compare == 0 to produce boolean
                let zero = builder.ins().iconst(clif_types::default_int_type(), 0);
                let cmp = builder.ins().icmp(IntCC::Equal, val, zero);
                let result = builder.ins().uextend(clif_types::default_int_type(), cmp);
                cx.last_expr_type = Some(clif_types::default_int_type());
                Ok(result)
            }
        }
        UnaryOp::BitNot => {
            cx.last_expr_type = Some(clif_types::default_int_type());
            Ok(builder.ins().bnot(val))
        }
        UnaryOp::Deref => {
            // Dereference pointer: emit a load from the address held in `val`.
            use cranelift_codegen::ir::MemFlags;
            let mem_flags = MemFlags::new();
            let loaded = builder
                .ins()
                .load(clif_types::default_int_type(), mem_flags, val, 0);
            cx.last_expr_type = Some(clif_types::default_int_type());
            Ok(loaded)
        }
        UnaryOp::Ref => {
            // Address-of: value must already be a stack slot reference (pointer).
            // For now, return the value as-is (variables are already I64 holding addresses).
            cx.last_expr_type = Some(clif_types::default_int_type());
            Ok(val)
        }
        _ => Err(CodegenError::NotImplemented(format!("unary op {:?}", op))),
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Binary operations
// ═══════════════════════════════════════════════════════════════════════

/// Compiles a binary operation.
pub(in crate::codegen::cranelift) fn compile_binop<M: Module>(
    builder: &mut FunctionBuilder,
    cx: &mut CodegenCtx<'_, M>,
    op: &BinOp,
    left: &Expr,
    right: &Expr,
) -> Result<ClifValue, CodegenError> {
    // Short-circuit for logical AND/OR
    if matches!(op, BinOp::And | BinOp::Or) {
        return compile_short_circuit(builder, cx, op, left, right);
    }

    // String concatenation: string + string
    if *op == BinOp::Add && is_string_producing_expr(left) {
        return compile_string_concat(builder, cx, left, right);
    }

    let lhs = compile_expr(builder, cx, left)?;
    let left_type = cx.last_expr_type;
    let left_str_len = cx.last_string_len.take();

    let rhs = compile_expr(builder, cx, right)?;
    let right_type = cx.last_expr_type;
    let right_str_len = cx.last_string_len.take();

    // String comparison: if both sides have string lengths, use runtime str_eq
    if matches!(op, BinOp::Eq | BinOp::Ne) {
        if let (Some(l_len), Some(r_len)) = (left_str_len, right_str_len) {
            let eq_id = *cx
                .functions
                .get("__str_eq")
                .ok_or_else(|| CodegenError::Internal("__str_eq not declared".into()))?;
            let callee = cx.module.declare_func_in_func(eq_id, builder.func);
            let call = builder.ins().call(callee, &[lhs, l_len, rhs, r_len]);
            let eq_result = builder.inst_results(call)[0];
            cx.last_expr_type = Some(clif_types::default_int_type());
            if *op == BinOp::Ne {
                // Invert: eq_result is 1 if equal, we want 1 if not equal
                let one = builder.ins().iconst(clif_types::default_int_type(), 1);
                return Ok(builder.ins().isub(one, eq_result));
            }
            return Ok(eq_result);
        }
    }

    let left_is_float = left_type.is_some_and(clif_types::is_float);
    let right_is_float = right_type.is_some_and(clif_types::is_float);
    let is_float = left_is_float || right_is_float;

    // If mixed int/float, widen the int operand
    let (lhs, rhs) = if left_is_float && !right_is_float {
        let widened = builder
            .ins()
            .fcvt_from_sint(clif_types::default_float_type(), rhs);
        (lhs, widened)
    } else if !left_is_float && right_is_float {
        let widened = builder
            .ins()
            .fcvt_from_sint(clif_types::default_float_type(), lhs);
        (widened, rhs)
    } else {
        (lhs, rhs)
    };

    // String concat fallback: if lhs turned out to be a string
    if *op == BinOp::Add && left_str_len.is_some() {
        // Already handled above, but if we get here with string lengths,
        // it means both sides are strings
        let rhs_len = right_str_len;
        if let (Some(l_len), Some(r_len)) = (left_str_len, rhs_len) {
            return compile_string_concat_vals(builder, cx, lhs, l_len, rhs, r_len);
        }
    }

    if is_float {
        compile_float_binop(builder, cx, op, lhs, rhs)
    } else {
        compile_int_binop(builder, cx, op, lhs, rhs, left_type, right_type)
    }
}

/// Compiles an integer binary operation.
///
/// B3.3: When both operands have the same sub-I64 semantic type (e.g., both u32),
/// the result is truncated to that width to ensure correct overflow behavior.
/// When operand types differ, the result is I64.
pub(super) fn compile_int_binop<M: Module>(
    builder: &mut FunctionBuilder,
    cx: &mut CodegenCtx<'_, M>,
    op: &BinOp,
    lhs: ClifValue,
    rhs: ClifValue,
    left_semantic_type: Option<cranelift_codegen::ir::Type>,
    right_semantic_type: Option<cranelift_codegen::ir::Type>,
) -> Result<ClifValue, CodegenError> {
    // Widen I8 (bool) operands to I64 for comparison/arithmetic
    let lhs_ty = builder.func.dfg.value_type(lhs);
    let rhs_ty = builder.func.dfg.value_type(rhs);
    let lhs = if lhs_ty == clif_types::bool_type() {
        builder.ins().uextend(clif_types::default_int_type(), lhs)
    } else {
        lhs
    };
    let rhs = if rhs_ty == clif_types::bool_type() {
        builder.ins().uextend(clif_types::default_int_type(), rhs)
    } else {
        rhs
    };

    // B3.3: Determine if both operands share a sub-I64 semantic type.
    // If so, arithmetic results will be truncated to that width.
    let narrow_type = match (left_semantic_type, right_semantic_type) {
        (Some(lt), Some(rt))
            if lt == rt
                && !clif_types::is_float(lt)
                && lt.bits() < 64
                && lt != clif_types::bool_type() =>
        {
            Some(lt)
        }
        _ => None,
    };

    let result = match op {
        BinOp::Add if cx.security_enabled => {
            // Security: use checked addition (aborts on overflow)
            if let Some(fn_id) = cx.functions.get("__checked_add").copied() {
                let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                let call = builder.ins().call(callee, &[lhs, rhs]);
                Ok(builder.inst_results(call)[0])
            } else {
                Ok(builder.ins().iadd(lhs, rhs))
            }
        }
        BinOp::Sub if cx.security_enabled => {
            if let Some(fn_id) = cx.functions.get("__checked_sub").copied() {
                let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                let call = builder.ins().call(callee, &[lhs, rhs]);
                Ok(builder.inst_results(call)[0])
            } else {
                Ok(builder.ins().isub(lhs, rhs))
            }
        }
        BinOp::Mul if cx.security_enabled => {
            if let Some(fn_id) = cx.functions.get("__checked_mul").copied() {
                let callee = cx.module.declare_func_in_func(fn_id, builder.func);
                let call = builder.ins().call(callee, &[lhs, rhs]);
                Ok(builder.inst_results(call)[0])
            } else {
                Ok(builder.ins().imul(lhs, rhs))
            }
        }
        BinOp::Add => Ok(builder.ins().iadd(lhs, rhs)),
        BinOp::Sub => Ok(builder.ins().isub(lhs, rhs)),
        BinOp::Mul => Ok(builder.ins().imul(lhs, rhs)),
        BinOp::Div => Ok(builder.ins().sdiv(lhs, rhs)),
        BinOp::Rem => Ok(builder.ins().srem(lhs, rhs)),
        BinOp::Pow => {
            // Integer power: convert to f64, call pow, convert back
            let lf = builder
                .ins()
                .fcvt_from_sint(clif_types::default_float_type(), lhs);
            let rf = builder
                .ins()
                .fcvt_from_sint(clif_types::default_float_type(), rhs);
            let pow_id = *cx
                .functions
                .get("__math_pow")
                .ok_or_else(|| CodegenError::Internal("__math_pow not declared".into()))?;
            let callee = cx.module.declare_func_in_func(pow_id, builder.func);
            let call = builder.ins().call(callee, &[lf, rf]);
            let result_f = builder.inst_results(call)[0];
            Ok(builder
                .ins()
                .fcvt_to_sint(clif_types::default_int_type(), result_f))
        }
        BinOp::Eq => {
            let cmp = builder.ins().icmp(IntCC::Equal, lhs, rhs);
            let result = builder.ins().uextend(clif_types::default_int_type(), cmp);
            cx.last_expr_type = Some(clif_types::default_int_type());
            return Ok(result); // Comparisons always return I64, skip truncation
        }
        BinOp::Ne => {
            let cmp = builder.ins().icmp(IntCC::NotEqual, lhs, rhs);
            let result = builder.ins().uextend(clif_types::default_int_type(), cmp);
            cx.last_expr_type = Some(clif_types::default_int_type());
            return Ok(result);
        }
        BinOp::Lt => {
            let cmp = builder.ins().icmp(IntCC::SignedLessThan, lhs, rhs);
            let result = builder.ins().uextend(clif_types::default_int_type(), cmp);
            cx.last_expr_type = Some(clif_types::default_int_type());
            return Ok(result);
        }
        BinOp::Gt => {
            let cmp = builder.ins().icmp(IntCC::SignedGreaterThan, lhs, rhs);
            let result = builder.ins().uextend(clif_types::default_int_type(), cmp);
            cx.last_expr_type = Some(clif_types::default_int_type());
            return Ok(result);
        }
        BinOp::Le => {
            let cmp = builder.ins().icmp(IntCC::SignedLessThanOrEqual, lhs, rhs);
            let result = builder.ins().uextend(clif_types::default_int_type(), cmp);
            cx.last_expr_type = Some(clif_types::default_int_type());
            return Ok(result);
        }
        BinOp::Ge => {
            let cmp = builder
                .ins()
                .icmp(IntCC::SignedGreaterThanOrEqual, lhs, rhs);
            let result = builder.ins().uextend(clif_types::default_int_type(), cmp);
            cx.last_expr_type = Some(clif_types::default_int_type());
            return Ok(result);
        }
        BinOp::BitAnd => Ok(builder.ins().band(lhs, rhs)),
        BinOp::BitOr => Ok(builder.ins().bor(lhs, rhs)),
        BinOp::BitXor => Ok(builder.ins().bxor(lhs, rhs)),
        BinOp::Shl => Ok(builder.ins().ishl(lhs, rhs)),
        BinOp::Shr => Ok(builder.ins().sshr(lhs, rhs)),
        _ => Err(CodegenError::NotImplemented(format!("int binop {:?}", op))),
    }?;

    // B3.3: If both operands share a sub-I64 type, truncate the result to that
    // width so that overflow wraps correctly (e.g., u32 max + 1 → 0).
    if let Some(nt) = narrow_type {
        let narrow = builder.ins().ireduce(nt, result);
        // Extend back to I64 for uniform representation (uextend for unsigned semantics)
        let extended = builder
            .ins()
            .uextend(clif_types::default_int_type(), narrow);
        cx.last_expr_type = Some(nt);
        Ok(extended)
    } else {
        cx.last_expr_type = Some(clif_types::default_int_type());
        Ok(result)
    }
}

/// Compiles a float binary operation.
pub(super) fn compile_float_binop<M: Module>(
    builder: &mut FunctionBuilder,
    cx: &mut CodegenCtx<'_, M>,
    op: &BinOp,
    lhs: ClifValue,
    rhs: ClifValue,
) -> Result<ClifValue, CodegenError> {
    use cranelift_codegen::ir::condcodes::FloatCC;
    match op {
        BinOp::Add => {
            cx.last_expr_type = Some(clif_types::default_float_type());
            Ok(builder.ins().fadd(lhs, rhs))
        }
        BinOp::Sub => {
            cx.last_expr_type = Some(clif_types::default_float_type());
            Ok(builder.ins().fsub(lhs, rhs))
        }
        BinOp::Mul => {
            cx.last_expr_type = Some(clif_types::default_float_type());
            Ok(builder.ins().fmul(lhs, rhs))
        }
        BinOp::Div => {
            cx.last_expr_type = Some(clif_types::default_float_type());
            Ok(builder.ins().fdiv(lhs, rhs))
        }
        BinOp::Rem => {
            // f64 remainder: a - floor(a/b) * b
            let div = builder.ins().fdiv(lhs, rhs);
            let floored = builder.ins().floor(div);
            let prod = builder.ins().fmul(floored, rhs);
            cx.last_expr_type = Some(clif_types::default_float_type());
            Ok(builder.ins().fsub(lhs, prod))
        }
        BinOp::Pow => {
            let pow_id = *cx
                .functions
                .get("__math_pow")
                .ok_or_else(|| CodegenError::Internal("__math_pow not declared".into()))?;
            let callee = cx.module.declare_func_in_func(pow_id, builder.func);
            let call = builder.ins().call(callee, &[lhs, rhs]);
            cx.last_expr_type = Some(clif_types::default_float_type());
            Ok(builder.inst_results(call)[0])
        }
        BinOp::Eq => {
            let cmp = builder.ins().fcmp(FloatCC::Equal, lhs, rhs);
            let result = builder.ins().uextend(clif_types::default_int_type(), cmp);
            cx.last_expr_type = Some(clif_types::default_int_type());
            Ok(result)
        }
        BinOp::Ne => {
            let cmp = builder.ins().fcmp(FloatCC::NotEqual, lhs, rhs);
            let result = builder.ins().uextend(clif_types::default_int_type(), cmp);
            cx.last_expr_type = Some(clif_types::default_int_type());
            Ok(result)
        }
        BinOp::Lt => {
            let cmp = builder.ins().fcmp(FloatCC::LessThan, lhs, rhs);
            let result = builder.ins().uextend(clif_types::default_int_type(), cmp);
            cx.last_expr_type = Some(clif_types::default_int_type());
            Ok(result)
        }
        BinOp::Gt => {
            let cmp = builder.ins().fcmp(FloatCC::GreaterThan, lhs, rhs);
            let result = builder.ins().uextend(clif_types::default_int_type(), cmp);
            cx.last_expr_type = Some(clif_types::default_int_type());
            Ok(result)
        }
        BinOp::Le => {
            let cmp = builder.ins().fcmp(FloatCC::LessThanOrEqual, lhs, rhs);
            let result = builder.ins().uextend(clif_types::default_int_type(), cmp);
            cx.last_expr_type = Some(clif_types::default_int_type());
            Ok(result)
        }
        BinOp::Ge => {
            let cmp = builder.ins().fcmp(FloatCC::GreaterThanOrEqual, lhs, rhs);
            let result = builder.ins().uextend(clif_types::default_int_type(), cmp);
            cx.last_expr_type = Some(clif_types::default_int_type());
            Ok(result)
        }
        _ => Err(CodegenError::NotImplemented(format!(
            "float binop {:?}",
            op
        ))),
    }
}

/// Compiles short-circuit logical AND/OR.
pub(super) fn compile_short_circuit<M: Module>(
    builder: &mut FunctionBuilder,
    cx: &mut CodegenCtx<'_, M>,
    op: &BinOp,
    left: &Expr,
    right: &Expr,
) -> Result<ClifValue, CodegenError> {
    let lhs = compile_expr(builder, cx, left)?;
    // Widen bool to i64 for branching
    let lhs_ty = builder.func.dfg.value_type(lhs);
    let lhs_wide = if lhs_ty == clif_types::bool_type() {
        builder.ins().uextend(clif_types::default_int_type(), lhs)
    } else {
        lhs
    };

    let rhs_block = builder.create_block();
    let merge_block = builder.create_block();

    // Short-circuit: use brif to rhs_block or skip_block, then jump to merge
    let skip_block = builder.create_block();
    match op {
        BinOp::And => {
            // false && _ = false (short-circuit)
            builder
                .ins()
                .brif(lhs_wide, rhs_block, &[], skip_block, &[]);
        }
        BinOp::Or => {
            // true || _ = true (short-circuit)
            builder
                .ins()
                .brif(lhs_wide, skip_block, &[], rhs_block, &[]);
        }
        _ => unreachable!(),
    }
    // Skip block: def_var + jump to merge
    builder.switch_to_block(skip_block);
    builder.seal_block(skip_block);
    let merge_var = builder.declare_var(clif_types::default_int_type());
    builder.def_var(merge_var, lhs_wide);
    builder.ins().jump(merge_block, &[]);

    builder.switch_to_block(rhs_block);
    builder.seal_block(rhs_block);
    let rhs = compile_expr(builder, cx, right)?;
    let rhs_ty = builder.func.dfg.value_type(rhs);
    let rhs_wide = if rhs_ty == clif_types::bool_type() {
        builder.ins().uextend(clif_types::default_int_type(), rhs)
    } else {
        rhs
    };
    builder.def_var(merge_var, rhs_wide);
    builder.ins().jump(merge_block, &[]);

    builder.switch_to_block(merge_block);
    builder.seal_block(merge_block);
    cx.last_expr_type = Some(clif_types::default_int_type());
    Ok(builder.use_var(merge_var))
}
