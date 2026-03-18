//! Control flow compilation: if/else, while, loop, for, match.

use cranelift_codegen::ir::condcodes::IntCC;
use cranelift_codegen::ir::instructions::BlockArg;
use cranelift_codegen::ir::{InstBuilder, MemFlags, Value as ClifValue};
use cranelift_frontend::FunctionBuilder;
use cranelift_module::Module;

use super::super::clif_types;
use super::super::context::CodegenCtx;
use super::{compile_array_literal, compile_expr, infer_generic_call_suffix};
use crate::codegen::CodegenError;
use crate::parser::ast::{BinOp, Expr, LiteralKind, MatchArm, Stmt, UnaryOp};

/// Coerces a Cranelift value to the target type if they differ.
///
/// Handles i8->i64 (uextend) and i64->i8 (ireduce) conversions that arise
/// when if/else branches produce different-width integer results.
fn coerce_to_type(
    builder: &mut FunctionBuilder,
    val: ClifValue,
    target_type: cranelift_codegen::ir::Type,
) -> ClifValue {
    let val_type = builder.func.dfg.value_type(val);
    if val_type == target_type {
        return val;
    }
    // Integer widening (e.g., i8 bool -> i64)
    if val_type.is_int() && target_type.is_int() && val_type.bits() < target_type.bits() {
        return builder.ins().uextend(target_type, val);
    }
    // Integer narrowing (e.g., i64 -> i8)
    if val_type.is_int() && target_type.is_int() && val_type.bits() > target_type.bits() {
        return builder.ins().ireduce(target_type, val);
    }
    val
}

/// Compiles an if/else expression.
///
/// The merge block's parameter type is inferred from the then-branch result.
pub(in crate::codegen::cranelift) fn compile_if<M: Module>(
    builder: &mut FunctionBuilder,
    cx: &mut CodegenCtx<'_, M>,
    condition: &Expr,
    then_branch: &Expr,
    else_branch: &Option<Box<Expr>>,
) -> Result<ClifValue, CodegenError> {
    let cond_val = compile_expr(builder, cx, condition)?;

    let then_block = builder.create_block();
    let else_block = builder.create_block();
    let merge_block = builder.create_block();

    // Infer merge type from the branch value type.
    let merge_type = infer_expr_type(cx, then_branch);
    let is_string_branch =
        merge_type == clif_types::pointer_type() && is_string_producing_expr(then_branch);
    builder.append_block_param(merge_block, merge_type);
    // If branches produce strings, add a second merge param for the length
    if is_string_branch {
        builder.append_block_param(merge_block, clif_types::default_int_type());
    }

    builder
        .ins()
        .brif(cond_val, then_block, &[], else_block, &[]);

    // Then branch
    builder.switch_to_block(then_block);
    builder.seal_block(then_block);
    let then_val = compile_expr(builder, cx, then_branch)?;
    let then_str_len = if is_string_branch {
        cx.last_string_len.take()
    } else {
        None
    };
    let then_terminated = builder.is_unreachable();
    if !then_terminated {
        // Coerce value type to match merge block parameter type
        let then_coerced = coerce_to_type(builder, then_val, merge_type);
        if is_string_branch {
            let len = then_str_len
                .unwrap_or_else(|| builder.ins().iconst(clif_types::default_int_type(), 0));
            builder.ins().jump(
                merge_block,
                &[BlockArg::Value(then_coerced), BlockArg::Value(len)],
            );
        } else {
            builder
                .ins()
                .jump(merge_block, &[BlockArg::Value(then_coerced)]);
        }
    }

    // Else branch
    builder.switch_to_block(else_block);
    builder.seal_block(else_block);
    let else_val = if let Some(else_expr) = else_branch {
        compile_expr(builder, cx, else_expr)?
    } else if clif_types::is_float(merge_type) {
        builder.ins().f64const(0.0)
    } else {
        builder.ins().iconst(merge_type, 0)
    };
    let else_str_len = if is_string_branch {
        cx.last_string_len.take()
    } else {
        None
    };
    let else_terminated = builder.is_unreachable();
    if !else_terminated {
        // Coerce value type to match merge block parameter type
        let else_coerced = coerce_to_type(builder, else_val, merge_type);
        if is_string_branch {
            let len = else_str_len
                .unwrap_or_else(|| builder.ins().iconst(clif_types::default_int_type(), 0));
            builder.ins().jump(
                merge_block,
                &[BlockArg::Value(else_coerced), BlockArg::Value(len)],
            );
        } else {
            builder
                .ins()
                .jump(merge_block, &[BlockArg::Value(else_coerced)]);
        }
    }

    // Merge block
    builder.switch_to_block(merge_block);
    builder.seal_block(merge_block);
    cx.last_expr_type = Some(merge_type);
    if is_string_branch {
        let params = builder.block_params(merge_block);
        cx.last_string_len = Some(params[1]);
    }
    if then_terminated && else_terminated {
        if clif_types::is_float(merge_type) {
            Ok(builder.ins().f64const(0.0))
        } else {
            Ok(builder.ins().iconst(merge_type, 0))
        }
    } else {
        Ok(builder.block_params(merge_block)[0])
    }
}

/// Returns true if the expression is likely to produce a string value.
pub(in crate::codegen::cranelift) fn is_string_producing_expr(expr: &Expr) -> bool {
    match expr {
        Expr::Literal {
            kind: LiteralKind::String(_) | LiteralKind::RawString(_),
            ..
        } => true,
        Expr::Block { expr: Some(e), .. } => is_string_producing_expr(e),
        Expr::Block {
            stmts, expr: None, ..
        } => {
            // Last statement might be an expression statement
            if let Some(Stmt::Expr { expr: e, .. }) = stmts.last() {
                is_string_producing_expr(e)
            } else {
                false
            }
        }
        Expr::If { then_branch, .. } => is_string_producing_expr(then_branch),
        _ => false,
    }
}

/// Infers the Cranelift type of an expression without compiling it.
///
/// Used for determining merge block parameter types in if/else.
pub(in crate::codegen::cranelift) fn infer_expr_type<M: Module>(
    cx: &CodegenCtx<'_, M>,
    expr: &Expr,
) -> cranelift_codegen::ir::Type {
    match expr {
        Expr::Literal { kind, .. } => match kind {
            LiteralKind::Float(_) => clif_types::default_float_type(),
            LiteralKind::Bool(_) => clif_types::bool_type(),
            LiteralKind::String(_) | LiteralKind::RawString(_) => clif_types::pointer_type(),
            _ => clif_types::default_int_type(),
        },
        Expr::Ident { name, .. } => {
            // If the variable is an array, it's stored as a stack pointer (i64),
            // even though var_types stores the element type for indexing.
            if cx.array_meta.contains_key(name) {
                return clif_types::pointer_type();
            }
            cx.var_types
                .get(name)
                .copied()
                .unwrap_or(clif_types::default_int_type())
        }
        Expr::Call { callee, args, .. } => {
            if let Expr::Ident { name, .. } = callee.as_ref() {
                // For generic functions, try type-aware resolution
                let resolved = if cx.mono_map.contains_key(name) {
                    let type_suffix = infer_generic_call_suffix(cx, name, args);
                    let typed_name = format!("{name}__mono_{type_suffix}");
                    if cx.fn_return_types.contains_key(&typed_name) {
                        typed_name
                    } else {
                        cx.mono_map.get(name).cloned().unwrap_or(name.clone())
                    }
                } else {
                    name.clone()
                };
                cx.fn_return_types
                    .get(&resolved)
                    .or_else(|| cx.fn_return_types.get(name))
                    .copied()
                    .unwrap_or(clif_types::default_int_type())
            } else if let Expr::Path { segments, .. } = callee.as_ref() {
                // Type::method() — look up impl_methods
                if segments.len() == 2 {
                    let key = (segments[0].clone(), segments[1].clone());
                    if let Some(mangled) = cx.impl_methods.get(&key) {
                        return cx
                            .fn_return_types
                            .get(mangled)
                            .copied()
                            .unwrap_or(clif_types::default_int_type());
                    }
                }
                clif_types::default_int_type()
            } else {
                clif_types::default_int_type()
            }
        }
        Expr::Binary { op, left, .. } => {
            if matches!(
                op,
                BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge
            ) {
                clif_types::default_int_type()
            } else {
                infer_expr_type(cx, left)
            }
        }
        Expr::Unary { op, operand, .. } => match op {
            UnaryOp::Not | UnaryOp::BitNot => clif_types::default_int_type(),
            _ => infer_expr_type(cx, operand),
        },
        Expr::Grouped { expr: inner, .. } => infer_expr_type(cx, inner),
        Expr::Block {
            expr: Some(tail), ..
        } => infer_expr_type(cx, tail),
        Expr::If { then_branch, .. } => infer_expr_type(cx, then_branch),
        Expr::Match { arms, .. } => {
            // Try all arms since the first arm may reference pattern bindings
            // not yet in scope (e.g., `match v { Float(x) => x, ... }`)
            for arm in arms {
                let ty = infer_expr_type(cx, &arm.body);
                if ty != clif_types::default_int_type() {
                    return ty;
                }
            }
            clif_types::default_int_type()
        }
        Expr::MethodCall {
            receiver, method, ..
        } => {
            // Try struct method return type lookup
            if let Expr::Ident { name, .. } = receiver.as_ref() {
                if let Some((_, struct_name)) = cx.struct_slots.get(name) {
                    let key = (struct_name.clone(), method.clone());
                    if let Some(mangled) = cx.impl_methods.get(&key) {
                        return cx
                            .fn_return_types
                            .get(mangled)
                            .copied()
                            .unwrap_or(clif_types::default_int_type());
                    }
                }
            }
            clif_types::default_int_type()
        }
        Expr::Field { object, field, .. } => {
            if let Expr::Ident { name, .. } = object.as_ref() {
                // Try struct field type lookup
                if let Some((_, struct_name)) = cx.struct_slots.get(name) {
                    if let Some(field_defs) = cx.struct_defs.get(struct_name) {
                        if let Some((_, ty)) = field_defs.iter().find(|(n, _)| n == field) {
                            return *ty;
                        }
                    }
                }
                // Try tuple element type lookup
                if let Some(elem_types) = cx.tuple_types.get(name) {
                    if let Ok(idx) = field.parse::<usize>() {
                        if let Some(&ty) = elem_types.get(idx) {
                            return ty;
                        }
                    }
                }
            }
            clif_types::default_int_type()
        }
        Expr::Index { .. } => clif_types::default_int_type(),
        Expr::Cast { ty, .. } => {
            let ty_name = match ty {
                crate::parser::ast::TypeExpr::Simple { name, .. } => name.as_str(),
                _ => "",
            };
            match ty_name {
                "f32" | "f64" => clif_types::default_float_type(),
                _ => clif_types::default_int_type(),
            }
        }
        Expr::StructInit { .. } | Expr::Tuple { .. } | Expr::Array { .. } => {
            clif_types::pointer_type()
        }
        Expr::Path { .. } => clif_types::default_int_type(),
        Expr::Pipe { right, .. } => {
            let fn_name = match right.as_ref() {
                Expr::Ident { name, .. } => Some(name.as_str()),
                Expr::Call { callee, .. } => {
                    if let Expr::Ident { name, .. } = callee.as_ref() {
                        Some(name.as_str())
                    } else {
                        None
                    }
                }
                _ => None,
            };
            fn_name
                .and_then(|n| cx.fn_return_types.get(n).copied())
                .unwrap_or(clif_types::default_int_type())
        }
        // Loops/assignments are void-valued (i64 zero)
        Expr::While { label: _, .. }
        | Expr::Loop { label: _, .. }
        | Expr::For { label: _, .. }
        | Expr::Assign { .. }
        | Expr::Range { .. } => clif_types::default_int_type(),
        _ => clif_types::default_int_type(),
    }
}

/// Compiles a while loop.
///
/// ```text
/// header_block:
///   cond = compile(condition)
///   brif cond, body_block, exit_block
/// body_block:
///   compile(body)
///   jump header_block
/// exit_block:
///   (result = 0, while loops are void-valued)
/// ```
pub(in crate::codegen::cranelift) fn compile_while<M: Module>(
    builder: &mut FunctionBuilder,
    cx: &mut CodegenCtx<'_, M>,
    condition: &Expr,
    body: &Expr,
) -> Result<ClifValue, CodegenError> {
    let header_block = builder.create_block();
    let body_block = builder.create_block();
    let exit_block = builder.create_block();

    builder.ins().jump(header_block, &[]);

    // Header: evaluate condition
    builder.switch_to_block(header_block);
    let cond_val = compile_expr(builder, cx, condition)?;
    builder
        .ins()
        .brif(cond_val, body_block, &[], exit_block, &[]);

    // Body — save/restore loop context for nested loops
    let prev_exit = cx.loop_exit.replace(exit_block);
    let prev_header = cx.loop_header.replace(header_block);

    builder.switch_to_block(body_block);
    builder.seal_block(body_block);
    let _ = compile_expr(builder, cx, body)?;
    if !builder.is_unreachable() {
        builder.ins().jump(header_block, &[]);
    }

    cx.loop_exit = prev_exit;
    cx.loop_header = prev_header;

    // Seal header after both predecessors (entry jump + body back-edge) are added
    builder.seal_block(header_block);

    // Exit
    builder.switch_to_block(exit_block);
    builder.seal_block(exit_block);
    cx.last_expr_type = Some(clif_types::default_int_type());
    Ok(builder.ins().iconst(clif_types::default_int_type(), 0))
}

/// Compiles an infinite `loop { body }`.
///
/// Uses `break` or `return` inside the body to exit.
/// Without either, the loop runs indefinitely.
pub(in crate::codegen::cranelift) fn compile_loop<M: Module>(
    builder: &mut FunctionBuilder,
    cx: &mut CodegenCtx<'_, M>,
    body: &Expr,
) -> Result<ClifValue, CodegenError> {
    let loop_block = builder.create_block();
    let exit_block = builder.create_block();

    // Save/restore loop context for nested loops
    let prev_exit = cx.loop_exit.replace(exit_block);
    let prev_header = cx.loop_header.replace(loop_block);

    builder.ins().jump(loop_block, &[]);

    // Loop body
    builder.switch_to_block(loop_block);
    let _ = compile_expr(builder, cx, body)?;
    // Only jump back if the block wasn't terminated by a return/break
    if !builder.is_unreachable() {
        builder.ins().jump(loop_block, &[]);
    }

    cx.loop_exit = prev_exit;
    cx.loop_header = prev_header;

    // Seal after all predecessors are added
    builder.seal_block(loop_block);

    // Exit block (reached by break or return inside the loop)
    builder.switch_to_block(exit_block);
    builder.seal_block(exit_block);
    cx.last_expr_type = Some(clif_types::default_int_type());
    Ok(builder.ins().iconst(clif_types::default_int_type(), 0))
}

/// Compiles a match expression on an enum value.
///
/// Supports patterns: literal (int), wildcard `_`, ident binding,
/// and enum variant patterns like `Some(x)`, `None`.
///
/// Each arm becomes a branch block; results merge via a merge block.
pub(in crate::codegen::cranelift) fn compile_match<M: Module>(
    builder: &mut FunctionBuilder,
    cx: &mut CodegenCtx<'_, M>,
    subject: &Expr,
    arms: &[MatchArm],
) -> Result<ClifValue, CodegenError> {
    use crate::parser::ast::Pattern;

    let tag_val = compile_expr(builder, cx, subject)?;
    let payload_val = cx
        .last_enum_payload
        .unwrap_or_else(|| builder.ins().iconst(clif_types::default_int_type(), 0));
    // Capture multi-field payload info from the subject (if any)
    let multi_payload = cx.last_enum_multi_payload.take();
    // Capture subject's tuple types for type-aware tuple pattern destructuring.
    // For ident subjects, look up from tuple_types; for literal tuple subjects,
    // use last_tuple_elem_types set by compile_tuple.
    let subject_tuple_types = if let Expr::Ident { name, .. } = subject {
        cx.tuple_types.get(name).cloned()
    } else {
        cx.last_tuple_elem_types.take()
    };

    // Create merge block for the match result — infer type from arm bodies.
    // Try all arms since early arms may have unresolved pattern bindings.
    // Prefer f64 if any arm returns f64 (wider type).
    let mut merge_type = clif_types::default_int_type();
    for arm in arms {
        let ty = infer_expr_type(cx, &arm.body);
        if ty == cranelift_codegen::ir::types::F64 {
            merge_type = ty;
            break; // f64 is the widest scalar type we support
        }
        if ty != clif_types::default_int_type() && merge_type == clif_types::default_int_type() {
            merge_type = ty;
        }
    }
    let merge_block = builder.create_block();
    builder.append_block_param(merge_block, merge_type);

    // Detect if match arms produce strings (for string length propagation)
    let is_string_match = merge_type == clif_types::pointer_type()
        && arms.iter().any(|arm| is_string_producing_expr(&arm.body));
    if is_string_match {
        builder.append_block_param(merge_block, clif_types::default_int_type());
    }

    // For each arm: test block → body block
    let mut arm_blocks = Vec::new();
    for _ in arms {
        arm_blocks.push((builder.create_block(), builder.create_block()));
    }
    // Fallthrough block (unreachable if match is exhaustive)
    let fallthrough = builder.create_block();

    // Jump to first arm's test block
    if !arm_blocks.is_empty() {
        builder.ins().jump(arm_blocks[0].0, &[]);
    } else {
        builder.ins().jump(fallthrough, &[]);
    }

    for (i, arm) in arms.iter().enumerate() {
        let (test_block, body_block) = arm_blocks[i];
        let next_test = if i + 1 < arm_blocks.len() {
            arm_blocks[i + 1].0
        } else {
            fallthrough
        };

        // Test block: compare subject against pattern
        builder.switch_to_block(test_block);
        builder.seal_block(test_block);

        match &arm.pattern {
            Pattern::Wildcard { .. } => {
                builder.ins().jump(body_block, &[]);
            }
            Pattern::Ident { name, .. } => {
                // Check if the ident is a known enum variant (bare variant pattern).
                // If so, compare by tag instead of binding as a wildcard.
                let variant_tag = cx.enum_defs.values().find_map(|variants| {
                    variants
                        .iter()
                        .position(|v| v == name)
                        .map(|idx| idx as i64)
                });
                if let Some(tag) = variant_tag {
                    let expected = builder.ins().iconst(clif_types::default_int_type(), tag);
                    let cmp = builder.ins().icmp(IntCC::Equal, tag_val, expected);
                    builder.ins().brif(cmp, body_block, &[], next_test, &[]);
                } else {
                    // Regular binding: always matches
                    let subject_type = cx.last_expr_type.unwrap_or(clif_types::default_int_type());
                    let bind_var = builder.declare_var(subject_type);
                    builder.def_var(bind_var, tag_val);
                    cx.var_map.insert(name.clone(), bind_var);
                    cx.var_types.insert(name.clone(), subject_type);
                    builder.ins().jump(body_block, &[]);
                }
            }
            Pattern::Literal { kind, .. } => {
                let expected = match kind {
                    LiteralKind::Int(n) => *n,
                    LiteralKind::Bool(b) => i64::from(*b),
                    _ => 0,
                };
                let expected_val = builder
                    .ins()
                    .iconst(clif_types::default_int_type(), expected);
                let cmp = builder.ins().icmp(IntCC::Equal, tag_val, expected_val);
                builder.ins().brif(cmp, body_block, &[], next_test, &[]);
            }
            Pattern::Enum {
                variant, fields, ..
            } => {
                // Look up tag for this variant
                let tag_value = resolve_variant_tag(cx, variant)?;
                let expected_tag = builder
                    .ins()
                    .iconst(clif_types::default_int_type(), tag_value);
                let cmp = builder.ins().icmp(IntCC::Equal, tag_val, expected_tag);
                builder.ins().brif(cmp, body_block, &[], next_test, &[]);

                // Bind pattern fields in the body block
                builder.switch_to_block(body_block);
                builder.seal_block(body_block);
                if fields.len() > 1 {
                    // S1.4: Multi-field variant destructuring
                    // Load each field from the stack slot at 8-byte offsets.
                    if let Some((slot, ref field_types)) = multi_payload {
                        for (i, field_pat) in fields.iter().enumerate() {
                            if let Pattern::Ident { name, .. } = field_pat {
                                let ft = field_types
                                    .get(i)
                                    .copied()
                                    .unwrap_or(clif_types::default_int_type());
                                let offset = (i * 8) as i32;
                                let val = builder.ins().stack_load(ft, slot, offset);
                                let bind_var = builder.declare_var(ft);
                                builder.def_var(bind_var, val);
                                cx.var_map.insert(name.clone(), bind_var);
                                cx.var_types.insert(name.clone(), ft);
                            }
                        }
                    }
                } else if let Some(Pattern::Ident { name, .. }) = fields.first() {
                    // S1.3: Type-aware payload binding — use enum_variant_types
                    // to determine the declared type for this variant's payload.
                    // If it differs from the stored payload type, bitcast.
                    let actual_type = builder.func.dfg.value_type(payload_val);
                    let expected_type =
                        resolve_variant_payload_type(cx, variant).unwrap_or(actual_type);
                    let typed_payload = if actual_type != expected_type
                        && actual_type.bits() == expected_type.bits()
                    {
                        builder
                            .ins()
                            .bitcast(expected_type, MemFlags::new(), payload_val)
                    } else {
                        payload_val
                    };
                    let bind_var = builder.declare_var(expected_type);
                    builder.def_var(bind_var, typed_payload);
                    cx.var_map.insert(name.clone(), bind_var);
                    cx.var_types.insert(name.clone(), expected_type);
                }
                let result = compile_expr(builder, cx, &arm.body)?;
                if !builder.is_unreachable() {
                    if is_string_match {
                        let len = cx.last_string_len.take().unwrap_or_else(|| {
                            builder.ins().iconst(clif_types::default_int_type(), 0)
                        });
                        builder.ins().jump(
                            merge_block,
                            &[BlockArg::Value(result), BlockArg::Value(len)],
                        );
                    } else {
                        let arg = BlockArg::Value(result);
                        builder.ins().jump(merge_block, &[arg]);
                    }
                }
                continue; // body block was already handled
            }
            Pattern::Tuple { elements, .. } => {
                // Tuple pattern: subject is a pointer to stack slot
                // Bind each element by loading from offset, using correct types
                builder.ins().jump(body_block, &[]);
                builder.switch_to_block(body_block);
                builder.seal_block(body_block);
                for (idx, elem_pat) in elements.iter().enumerate() {
                    if let Pattern::Ident { name, .. } = elem_pat {
                        let elem_type = subject_tuple_types
                            .as_ref()
                            .and_then(|types| types.get(idx).copied())
                            .unwrap_or(clif_types::default_int_type());
                        let offset_val = builder
                            .ins()
                            .iconst(clif_types::default_int_type(), (idx as i64) * 8);
                        let addr = builder.ins().iadd(tag_val, offset_val);
                        let val = builder.ins().load(
                            elem_type,
                            cranelift_codegen::ir::MemFlags::new(),
                            addr,
                            0,
                        );
                        let bind_var = builder.declare_var(elem_type);
                        builder.def_var(bind_var, val);
                        cx.var_map.insert(name.clone(), bind_var);
                        cx.var_types.insert(name.clone(), elem_type);
                    }
                    // Wildcard in tuple position: skip binding
                }
                let result = compile_expr(builder, cx, &arm.body)?;
                if !builder.is_unreachable() {
                    if is_string_match {
                        let len = cx.last_string_len.take().unwrap_or_else(|| {
                            builder.ins().iconst(clif_types::default_int_type(), 0)
                        });
                        builder.ins().jump(
                            merge_block,
                            &[BlockArg::Value(result), BlockArg::Value(len)],
                        );
                    } else {
                        let arg = BlockArg::Value(result);
                        builder.ins().jump(merge_block, &[arg]);
                    }
                }
                continue;
            }
            Pattern::Struct {
                name: struct_name,
                fields: field_pats,
                ..
            } => {
                // Struct pattern: subject is a pointer to stack slot
                // Look up field order from struct_defs, bind by name
                builder.ins().jump(body_block, &[]);
                builder.switch_to_block(body_block);
                builder.seal_block(body_block);
                if let Some(field_defs) = cx.struct_defs.get(struct_name) {
                    for fp in field_pats {
                        if let Some(field_idx) =
                            field_defs.iter().position(|(fname, _)| fname == &fp.name)
                        {
                            let field_type = field_defs[field_idx].1;
                            let offset_val = builder
                                .ins()
                                .iconst(clif_types::default_int_type(), (field_idx as i64) * 8);
                            let addr = builder.ins().iadd(tag_val, offset_val);
                            let val = builder.ins().load(
                                field_type,
                                cranelift_codegen::ir::MemFlags::new(),
                                addr,
                                0,
                            );
                            // Bind: shorthand `x` or explicit `x: pat`
                            let bind_name = if fp.pattern.is_none() {
                                &fp.name
                            } else if let Some(Pattern::Ident { name, .. }) = &fp.pattern {
                                name
                            } else {
                                continue;
                            };
                            let bind_var = builder.declare_var(field_type);
                            builder.def_var(bind_var, val);
                            cx.var_map.insert(bind_name.clone(), bind_var);
                            cx.var_types.insert(bind_name.clone(), field_type);
                        }
                    }
                }
                let result = compile_expr(builder, cx, &arm.body)?;
                if !builder.is_unreachable() {
                    if is_string_match {
                        let len = cx.last_string_len.take().unwrap_or_else(|| {
                            builder.ins().iconst(clif_types::default_int_type(), 0)
                        });
                        builder.ins().jump(
                            merge_block,
                            &[BlockArg::Value(result), BlockArg::Value(len)],
                        );
                    } else {
                        let arg = BlockArg::Value(result);
                        builder.ins().jump(merge_block, &[arg]);
                    }
                }
                continue;
            }
            Pattern::Range {
                start,
                end,
                inclusive,
                ..
            } => {
                // Range pattern: check subject >= start && subject < end (or <= for inclusive)
                let start_val = compile_expr(builder, cx, start)?;
                let end_val = compile_expr(builder, cx, end)?;
                let ge_start =
                    builder
                        .ins()
                        .icmp(IntCC::SignedGreaterThanOrEqual, tag_val, start_val);
                let cmp_end = if *inclusive {
                    builder
                        .ins()
                        .icmp(IntCC::SignedLessThanOrEqual, tag_val, end_val)
                } else {
                    builder.ins().icmp(IntCC::SignedLessThan, tag_val, end_val)
                };
                let in_range = builder.ins().band(ge_start, cmp_end);
                builder
                    .ins()
                    .brif(in_range, body_block, &[], next_test, &[]);
            }
        }

        // Body block for non-Enum patterns
        builder.switch_to_block(body_block);
        builder.seal_block(body_block);
        let result = compile_expr(builder, cx, &arm.body)?;
        if !builder.is_unreachable() {
            if is_string_match {
                let len = cx
                    .last_string_len
                    .take()
                    .unwrap_or_else(|| builder.ins().iconst(clif_types::default_int_type(), 0));
                builder.ins().jump(
                    merge_block,
                    &[BlockArg::Value(result), BlockArg::Value(len)],
                );
            } else {
                let arg = BlockArg::Value(result);
                builder.ins().jump(merge_block, &[arg]);
            }
        }
    }

    // Fallthrough: should not be reached if match is exhaustive
    builder.switch_to_block(fallthrough);
    builder.seal_block(fallthrough);
    let default_val = if clif_types::is_float(merge_type) {
        builder.ins().f64const(0.0)
    } else {
        builder.ins().iconst(merge_type, 0)
    };
    if is_string_match {
        let zero_len = builder.ins().iconst(clif_types::default_int_type(), 0);
        builder.ins().jump(
            merge_block,
            &[BlockArg::Value(default_val), BlockArg::Value(zero_len)],
        );
    } else {
        let default_arg = BlockArg::Value(default_val);
        builder.ins().jump(merge_block, &[default_arg]);
    }

    // Merge block
    builder.switch_to_block(merge_block);
    builder.seal_block(merge_block);
    cx.last_expr_type = Some(merge_type);
    if is_string_match {
        let params = builder.block_params(merge_block);
        cx.last_string_len = Some(params[1]);
    }
    Ok(builder.block_params(merge_block)[0])
}

/// Resolves a variant name to its tag value.
///
/// Built-in tags: None=0, Some=1, Ok=0, Err=1.
/// User-defined: index in the enum definition.
pub(in crate::codegen::cranelift) fn resolve_variant_tag<M: Module>(
    cx: &CodegenCtx<'_, M>,
    variant: &str,
) -> Result<i64, CodegenError> {
    // User-defined enums take priority over built-in names
    for variants in cx.enum_defs.values() {
        if let Some(idx) = variants.iter().position(|v| v == variant) {
            return Ok(idx as i64);
        }
    }
    // Built-in Option/Result (when no user-defined enum contains these variants)
    match variant {
        "None" => return Ok(0),
        "Some" => return Ok(1),
        "Ok" => return Ok(0),
        "Err" => return Ok(1),
        _ => {}
    }
    Err(CodegenError::UndefinedVariable(format!(
        "unknown enum variant '{variant}'"
    )))
}

/// Resolves a variant name to its expected payload Cranelift type.
///
/// Looks up the variant in `enum_variant_types` to determine the declared
/// payload type. Returns `None` if the variant has no payload, is unknown,
/// or belongs to a generic enum (where the type is inferred at construction).
fn resolve_variant_payload_type<M: Module>(
    cx: &CodegenCtx<'_, M>,
    variant: &str,
) -> Option<cranelift_codegen::ir::Type> {
    for ((enum_name, var_name), types) in cx.enum_variant_types.iter() {
        if var_name == variant {
            // Generic enums have unresolved type params (T → I64 default).
            // Don't override — the actual type is inferred from construction.
            if cx.generic_enum_defs.contains_key(enum_name) {
                return None;
            }
            return types.first().copied();
        }
    }
    None
}

/// Compiles a for-in-range loop.
///
/// Only supports `for var in start..end { body }` (exclusive range).
/// Lowers to a while loop with a counter variable.
pub(in crate::codegen::cranelift) fn compile_for<M: Module>(
    builder: &mut FunctionBuilder,
    cx: &mut CodegenCtx<'_, M>,
    variable: &str,
    iterable: &Expr,
    body: &Expr,
) -> Result<ClifValue, CodegenError> {
    // Range expressions: for i in start..end
    let (start_expr, end_expr, inclusive) = match iterable {
        Expr::Range {
            start: Some(s),
            end: Some(e),
            inclusive,
            ..
        } => (s.as_ref(), e.as_ref(), *inclusive),
        // Split result iteration: for x in parts { ... }
        Expr::Ident { name: arr_name, .. } if cx.split_vars.contains(arr_name) => {
            return compile_for_in_split(builder, cx, variable, arr_name, body);
        }
        // Inline map.keys() iteration: for k in map.keys() { ... }
        Expr::MethodCall {
            receiver,
            method,
            args,
            ..
        } if method == "keys" && args.is_empty() => {
            if let Expr::Ident { name: map_name, .. } = receiver.as_ref() {
                if cx.heap_maps.contains(map_name) {
                    return compile_for_in_map_keys(builder, cx, variable, map_name, body);
                }
            }
            return Err(CodegenError::NotImplemented(
                "for-in over non-map .keys()".into(),
            ));
        }
        // Heap array iteration: for x in heap_arr { ... }
        Expr::Ident { name: arr_name, .. } if cx.heap_arrays.contains(arr_name) => {
            return compile_for_in_heap_array(builder, cx, variable, arr_name, body);
        }
        // Stack array iteration: for x in arr { ... }
        Expr::Ident { name: arr_name, .. } if cx.array_meta.contains_key(arr_name) => {
            return compile_for_in_array(builder, cx, variable, arr_name, body);
        }
        // Inline array literal: for x in [1, 2, 3] { ... }
        Expr::Array { elements, .. } => {
            return compile_for_in_array_literal(builder, cx, variable, elements, body);
        }
        _ => {
            return Err(CodegenError::NotImplemented("for-in over non-range".into()));
        }
    };

    let start_val = compile_expr(builder, cx, start_expr)?;
    let end_val = compile_expr(builder, cx, end_expr)?;

    // Create loop variable
    let loop_var = builder.declare_var(clif_types::default_int_type());
    builder.def_var(loop_var, start_val);
    cx.var_map.insert(variable.to_string(), loop_var);

    let header_block = builder.create_block();
    let body_block = builder.create_block();
    let incr_block = builder.create_block(); // continue jumps here (not header)
    let exit_block = builder.create_block();

    builder.ins().jump(header_block, &[]);

    // Header: check i < end (or i <= end for inclusive)
    builder.switch_to_block(header_block);
    let current = builder.use_var(loop_var);
    let cmp = if inclusive {
        builder
            .ins()
            .icmp(IntCC::SignedLessThanOrEqual, current, end_val)
    } else {
        builder.ins().icmp(IntCC::SignedLessThan, current, end_val)
    };
    builder.ins().brif(cmp, body_block, &[], exit_block, &[]);

    // Body — `continue` jumps to incr_block (increment then re-check)
    let prev_exit = cx.loop_exit.replace(exit_block);
    let prev_header = cx.loop_header.replace(incr_block);

    builder.switch_to_block(body_block);
    builder.seal_block(body_block);
    let _ = compile_expr(builder, cx, body)?;

    // Fall through to increment block
    if !builder.is_unreachable() {
        builder.ins().jump(incr_block, &[]);
    }

    // Increment block: i = i + 1, then jump back to header
    builder.switch_to_block(incr_block);
    builder.seal_block(incr_block);
    let current_after = builder.use_var(loop_var);
    let one = builder.ins().iconst(clif_types::default_int_type(), 1);
    let next = builder.ins().iadd(current_after, one);
    builder.def_var(loop_var, next);
    builder.ins().jump(header_block, &[]);

    cx.loop_exit = prev_exit;
    cx.loop_header = prev_header;

    builder.seal_block(header_block);

    // Exit
    builder.switch_to_block(exit_block);
    builder.seal_block(exit_block);
    cx.last_expr_type = Some(clif_types::default_int_type());
    Ok(builder.ins().iconst(clif_types::default_int_type(), 0))
}

/// Compiles `for x in arr { body }` where `arr` is a named array variable.
pub(in crate::codegen::cranelift) fn compile_for_in_array<M: Module>(
    builder: &mut FunctionBuilder,
    cx: &mut CodegenCtx<'_, M>,
    variable: &str,
    arr_name: &str,
    body: &Expr,
) -> Result<ClifValue, CodegenError> {
    let (slot, len) =
        cx.array_meta.get(arr_name).copied().ok_or_else(|| {
            CodegenError::NotImplemented(format!("for-in on non-array '{arr_name}'"))
        })?;

    // Infer element type from the array variable's tracked type.
    // If the array was declared as f64[], use F64; otherwise default to i64.
    let elem_type = cx
        .var_types
        .get(arr_name)
        .copied()
        .unwrap_or(clif_types::default_int_type());

    let idx_var = builder.declare_var(clif_types::default_int_type());
    let zero = builder.ins().iconst(clif_types::default_int_type(), 0);
    builder.def_var(idx_var, zero);

    let elem_var = builder.declare_var(elem_type);
    let elem_zero = if elem_type == cranelift_codegen::ir::types::F64 {
        builder.ins().f64const(0.0)
    } else {
        zero
    };
    builder.def_var(elem_var, elem_zero);
    cx.var_map.insert(variable.to_string(), elem_var);
    cx.var_types.insert(variable.to_string(), elem_type);

    let header_block = builder.create_block();
    let body_block = builder.create_block();
    let incr_block = builder.create_block();
    let exit_block = builder.create_block();

    builder.ins().jump(header_block, &[]);

    // Header: check idx < len
    builder.switch_to_block(header_block);
    let current_idx = builder.use_var(idx_var);
    let len_val = builder
        .ins()
        .iconst(clif_types::default_int_type(), len as i64);
    let cmp = builder
        .ins()
        .icmp(IntCC::SignedLessThan, current_idx, len_val);
    builder.ins().brif(cmp, body_block, &[], exit_block, &[]);

    // Body: load element, set loop variable
    let prev_exit = cx.loop_exit.replace(exit_block);
    let prev_header = cx.loop_header.replace(incr_block);

    builder.switch_to_block(body_block);
    builder.seal_block(body_block);
    let idx_for_load = builder.use_var(idx_var);
    let slot_addr = builder
        .ins()
        .stack_addr(clif_types::pointer_type(), slot, 0);
    let byte_offset = builder.ins().imul_imm(idx_for_load, 8);
    let elem_addr = builder.ins().iadd(slot_addr, byte_offset);
    let elem_val = builder.ins().load(
        elem_type,
        cranelift_codegen::ir::MemFlags::new(),
        elem_addr,
        0,
    );
    builder.def_var(elem_var, elem_val);
    let _ = compile_expr(builder, cx, body)?;

    if !builder.is_unreachable() {
        builder.ins().jump(incr_block, &[]);
    }

    // Increment
    builder.switch_to_block(incr_block);
    builder.seal_block(incr_block);
    let cur = builder.use_var(idx_var);
    let one = builder.ins().iconst(clif_types::default_int_type(), 1);
    let next = builder.ins().iadd(cur, one);
    builder.def_var(idx_var, next);
    builder.ins().jump(header_block, &[]);

    cx.loop_exit = prev_exit;
    cx.loop_header = prev_header;
    builder.seal_block(header_block);

    builder.switch_to_block(exit_block);
    builder.seal_block(exit_block);
    Ok(builder.ins().iconst(clif_types::default_int_type(), 0))
}

/// Compiles `for x in heap_arr { body }` where `heap_arr` is a dynamic array.
pub(in crate::codegen::cranelift) fn compile_for_in_heap_array<M: Module>(
    builder: &mut FunctionBuilder,
    cx: &mut CodegenCtx<'_, M>,
    variable: &str,
    arr_name: &str,
    body: &Expr,
) -> Result<ClifValue, CodegenError> {
    let arr_var = *cx
        .var_map
        .get(arr_name)
        .ok_or_else(|| CodegenError::UndefinedVariable(arr_name.to_string()))?;

    let idx_var = builder.declare_var(clif_types::default_int_type());
    let zero = builder.ins().iconst(clif_types::default_int_type(), 0);
    builder.def_var(idx_var, zero);

    let elem_var = builder.declare_var(clif_types::default_int_type());
    builder.def_var(elem_var, zero);
    cx.var_map.insert(variable.to_string(), elem_var);

    let header_block = builder.create_block();
    let body_block = builder.create_block();
    let incr_block = builder.create_block();
    let exit_block = builder.create_block();

    builder.ins().jump(header_block, &[]);

    // Header: check idx < array_len(arr)
    builder.switch_to_block(header_block);
    let current_idx = builder.use_var(idx_var);
    let arr_ptr = builder.use_var(arr_var);
    let len_id = *cx
        .functions
        .get("__array_len")
        .ok_or_else(|| CodegenError::Internal("__array_len not declared".into()))?;
    let len_callee = cx.module.declare_func_in_func(len_id, builder.func);
    let len_call = builder.ins().call(len_callee, &[arr_ptr]);
    let len_val = builder.inst_results(len_call)[0];
    let cmp = builder
        .ins()
        .icmp(IntCC::SignedLessThan, current_idx, len_val);
    builder.ins().brif(cmp, body_block, &[], exit_block, &[]);

    // Body: load element via runtime call
    let prev_exit = cx.loop_exit.replace(exit_block);
    let prev_header = cx.loop_header.replace(incr_block);

    builder.switch_to_block(body_block);
    builder.seal_block(body_block);
    let idx_for_get = builder.use_var(idx_var);
    let arr_ptr2 = builder.use_var(arr_var);
    let get_id = *cx
        .functions
        .get("__array_get")
        .ok_or_else(|| CodegenError::Internal("__array_get not declared".into()))?;
    let get_callee = cx.module.declare_func_in_func(get_id, builder.func);
    let get_call = builder.ins().call(get_callee, &[arr_ptr2, idx_for_get]);
    let elem_val = builder.inst_results(get_call)[0];
    builder.def_var(elem_var, elem_val);
    let _ = compile_expr(builder, cx, body)?;

    if !builder.is_unreachable() {
        builder.ins().jump(incr_block, &[]);
    }

    // Increment
    builder.switch_to_block(incr_block);
    builder.seal_block(incr_block);
    let cur = builder.use_var(idx_var);
    let one = builder.ins().iconst(clif_types::default_int_type(), 1);
    let next = builder.ins().iadd(cur, one);
    builder.def_var(idx_var, next);
    builder.ins().jump(header_block, &[]);

    cx.loop_exit = prev_exit;
    cx.loop_header = prev_header;
    builder.seal_block(header_block);

    builder.switch_to_block(exit_block);
    builder.seal_block(exit_block);
    Ok(builder.ins().iconst(clif_types::default_int_type(), 0))
}

/// Compiles `for part in split_result { body }`.
///
/// Each iteration calls `__split_get` to retrieve (ptr, len) for the i-th string part,
/// storing it as a string variable accessible in the loop body.
pub(in crate::codegen::cranelift) fn compile_for_in_split<M: Module>(
    builder: &mut FunctionBuilder,
    cx: &mut CodegenCtx<'_, M>,
    variable: &str,
    arr_name: &str,
    body: &Expr,
) -> Result<ClifValue, CodegenError> {
    let arr_var = *cx
        .var_map
        .get(arr_name)
        .ok_or_else(|| CodegenError::UndefinedVariable(arr_name.to_string()))?;

    let idx_var = builder.declare_var(clif_types::default_int_type());
    let zero = builder.ins().iconst(clif_types::default_int_type(), 0);
    builder.def_var(idx_var, zero);

    // Declare loop variable as a string pointer
    let elem_var = builder.declare_var(clif_types::pointer_type());
    let null_ptr = builder.ins().iconst(clif_types::pointer_type(), 0);
    builder.def_var(elem_var, null_ptr);
    cx.var_map.insert(variable.to_string(), elem_var);
    cx.var_types
        .insert(variable.to_string(), clif_types::pointer_type());

    // Declare a companion length variable
    let len_var = builder.declare_var(clif_types::default_int_type());
    builder.def_var(len_var, zero);
    cx.string_lens.insert(variable.to_string(), len_var);

    let header_block = builder.create_block();
    let body_block = builder.create_block();
    let incr_block = builder.create_block();
    let exit_block = builder.create_block();

    builder.ins().jump(header_block, &[]);

    // Header: check idx < split_len(arr)
    builder.switch_to_block(header_block);
    let current_idx = builder.use_var(idx_var);
    let arr_ptr = builder.use_var(arr_var);
    let split_len_id = *cx
        .functions
        .get("__split_len")
        .ok_or_else(|| CodegenError::Internal("__split_len not declared".into()))?;
    let len_callee = cx.module.declare_func_in_func(split_len_id, builder.func);
    let len_call = builder.ins().call(len_callee, &[arr_ptr]);
    let len_val = builder.inst_results(len_call)[0];
    let cmp = builder
        .ins()
        .icmp(IntCC::SignedLessThan, current_idx, len_val);
    builder.ins().brif(cmp, body_block, &[], exit_block, &[]);

    // Body: load element via split_get
    let prev_exit = cx.loop_exit.replace(exit_block);
    let prev_header = cx.loop_header.replace(incr_block);

    builder.switch_to_block(body_block);
    builder.seal_block(body_block);
    let idx_for_get = builder.use_var(idx_var);
    let arr_ptr2 = builder.use_var(arr_var);

    // Create stack slots for the out (ptr, len) from split_get
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

    let split_get_id = *cx
        .functions
        .get("__split_get")
        .ok_or_else(|| CodegenError::Internal("__split_get not declared".into()))?;
    let get_callee = cx.module.declare_func_in_func(split_get_id, builder.func);
    builder.ins().call(
        get_callee,
        &[arr_ptr2, idx_for_get, out_ptr_addr, out_len_addr],
    );

    let part_ptr = builder.ins().load(
        clif_types::pointer_type(),
        cranelift_codegen::ir::MemFlags::new(),
        out_ptr_addr,
        0,
    );
    let part_len = builder.ins().load(
        clif_types::default_int_type(),
        cranelift_codegen::ir::MemFlags::new(),
        out_len_addr,
        0,
    );
    builder.def_var(elem_var, part_ptr);
    builder.def_var(len_var, part_len);

    let _ = compile_expr(builder, cx, body)?;

    if !builder.is_unreachable() {
        builder.ins().jump(incr_block, &[]);
    }

    // Increment
    builder.switch_to_block(incr_block);
    builder.seal_block(incr_block);
    let cur = builder.use_var(idx_var);
    let one = builder.ins().iconst(clif_types::default_int_type(), 1);
    let next = builder.ins().iadd(cur, one);
    builder.def_var(idx_var, next);
    builder.ins().jump(header_block, &[]);

    cx.loop_exit = prev_exit;
    cx.loop_header = prev_header;
    builder.seal_block(header_block);

    builder.switch_to_block(exit_block);
    builder.seal_block(exit_block);

    // Clean up loop variable from string_lens
    cx.string_lens.remove(variable);
    Ok(builder.ins().iconst(clif_types::default_int_type(), 0))
}

/// Compiles `for k in map.keys() { body }`.
///
/// Calls `fj_rt_map_keys` to produce a split-compatible `Box<Vec<i64>>` of (ptr, len) pairs,
/// stores the result in a temporary variable, then delegates to `compile_for_in_split`.
pub(in crate::codegen::cranelift) fn compile_for_in_map_keys<M: Module>(
    builder: &mut FunctionBuilder,
    cx: &mut CodegenCtx<'_, M>,
    variable: &str,
    map_name: &str,
    body: &Expr,
) -> Result<ClifValue, CodegenError> {
    let map_var = *cx
        .var_map
        .get(map_name)
        .ok_or_else(|| CodegenError::UndefinedVariable(map_name.to_string()))?;
    let map_ptr = builder.use_var(map_var);

    // Allocate a stack slot for the count output (required by fj_rt_map_keys signature)
    let count_slot = builder.create_sized_stack_slot(cranelift_codegen::ir::StackSlotData::new(
        cranelift_codegen::ir::StackSlotKind::ExplicitSlot,
        8,
        0,
    ));
    let count_addr = builder
        .ins()
        .stack_addr(clif_types::default_int_type(), count_slot, 0);

    let func_id = *cx
        .functions
        .get("__map_keys")
        .ok_or_else(|| CodegenError::Internal("__map_keys not declared".into()))?;
    let callee = cx.module.declare_func_in_func(func_id, builder.func);
    let call = builder.ins().call(callee, &[map_ptr, count_addr]);
    let keys_ptr = builder.inst_results(call)[0];

    // Store the keys array in a temp variable and register as split_vars
    let temp_name = format!("__map_keys_tmp_{map_name}");
    let temp_var = builder.declare_var(clif_types::pointer_type());
    builder.def_var(temp_var, keys_ptr);
    cx.var_map.insert(temp_name.clone(), temp_var);
    cx.split_vars.insert(temp_name.clone());

    let result = compile_for_in_split(builder, cx, variable, &temp_name, body)?;

    // Clean up the temporary
    cx.split_vars.remove(&temp_name);
    cx.var_map.remove(&temp_name);

    Ok(result)
}

/// Compiles `for x in [1, 2, 3] { body }` with an inline array literal.
pub(in crate::codegen::cranelift) fn compile_for_in_array_literal<M: Module>(
    builder: &mut FunctionBuilder,
    cx: &mut CodegenCtx<'_, M>,
    variable: &str,
    elements: &[Expr],
    body: &Expr,
) -> Result<ClifValue, CodegenError> {
    // Compile the array literal to a stack slot, then iterate it
    let ptr = compile_array_literal(builder, cx, elements)?;

    // Extract the metadata from last_array
    let (slot, len) = cx
        .last_array
        .take()
        .ok_or_else(|| CodegenError::NotImplemented("for-in array literal metadata".into()))?;

    // Store under a temporary name so compile_for_in_array can find it
    let tmp_name = format!("__for_arr_{}", ptr.as_u32());
    cx.array_meta.insert(tmp_name.clone(), (slot, len));

    compile_for_in_array(builder, cx, variable, &tmp_name, body)
}
