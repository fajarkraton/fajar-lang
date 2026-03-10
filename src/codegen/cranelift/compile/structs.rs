//! Struct compilation: init, field access, field assignment.

use cranelift_codegen::ir::{InstBuilder, Value as ClifValue};
use cranelift_frontend::FunctionBuilder;
use cranelift_module::Module;

use super::super::clif_types;
use super::super::context::CodegenCtx;
use super::compile_expr;
use crate::codegen::CodegenError;
use crate::parser::ast::{Expr, FieldInit};

/// Compiles a struct initializer expression: `Point { x: 1, y: 2 }`.
///
/// Allocates a stack slot with 8 bytes per field and stores each field value
/// at its offset. Returns a pointer to the stack slot.
pub(in crate::codegen::cranelift) fn compile_struct_init<M: Module>(
    builder: &mut FunctionBuilder,
    cx: &mut CodegenCtx<'_, M>,
    name: &str,
    init_fields: &[FieldInit],
) -> Result<ClifValue, CodegenError> {
    let field_defs =
        cx.struct_defs.get(name).cloned().ok_or_else(|| {
            CodegenError::UndefinedVariable(format!("struct '{name}' not defined"))
        })?;

    let num_fields = field_defs.len();
    let slot_size = (num_fields as u32).checked_mul(8).ok_or_else(|| {
        CodegenError::NotImplemented(format!(
            "struct '{name}' has too many fields ({num_fields}) for stack allocation"
        ))
    })?;
    let slot_data = cranelift_codegen::ir::StackSlotData::new(
        cranelift_codegen::ir::StackSlotKind::ExplicitSlot,
        slot_size,
        0,
    );
    let slot = builder.create_sized_stack_slot(slot_data);

    // Check if this struct has a bitfield layout
    let bf_layout = cx.bitfield_layouts.get(name).cloned();

    if let Some(ref layout) = bf_layout {
        // Bitfield struct: pack all bitfield fields into a single i64 at offset 0
        let mut packed = builder.ins().iconst(clif_types::default_int_type(), 0);
        for fi in init_fields {
            if let Some((_, bit_offset, width)) = layout.iter().find(|(n, _, _)| n == &fi.name) {
                let val = compile_expr(builder, cx, &fi.value)?;
                let mask = (1i64 << *width) - 1;
                let masked = builder.ins().band_imm(val, mask);
                let shifted = if *bit_offset > 0 {
                    let shift = builder
                        .ins()
                        .iconst(clif_types::default_int_type(), *bit_offset as i64);
                    builder.ins().ishl(masked, shift)
                } else {
                    masked
                };
                packed = builder.ins().bor(packed, shifted);
            } else {
                // Non-bitfield field in a bitfield struct: store at its slot offset
                let idx = field_defs
                    .iter()
                    .position(|(fname, _)| fname == &fi.name)
                    .ok_or_else(|| {
                        CodegenError::NotImplemented(format!(
                            "struct '{name}' has no field '{}'",
                            fi.name
                        ))
                    })?;
                let val = compile_expr(builder, cx, &fi.value)?;
                let offset = (idx as i32) * 8;
                builder.ins().stack_store(
                    val,
                    slot,
                    cranelift_codegen::ir::immediates::Offset32::new(offset),
                );
            }
        }
        // Store the packed bitfield word at offset 0
        builder.ins().stack_store(
            packed,
            slot,
            cranelift_codegen::ir::immediates::Offset32::new(0),
        );
    } else {
        // Normal struct: store each field at its offset
        for fi in init_fields {
            let idx = field_defs
                .iter()
                .position(|(fname, _)| fname == &fi.name)
                .ok_or_else(|| {
                    CodegenError::NotImplemented(format!(
                        "struct '{name}' has no field '{}'",
                        fi.name
                    ))
                })?;
            let field_type = field_defs[idx].1;
            let val = compile_expr(builder, cx, &fi.value)?;
            let is_union = cx.union_names.contains(name);
            let offset = if is_union { 0 } else { (idx as i32) * 8 };
            // For f64 values stored in i64-width slots, bitcast to i64 first
            let store_val = if clif_types::is_float(field_type)
                && !clif_types::is_float(builder.func.dfg.value_type(val))
            {
                // int → float: convert
                builder.ins().fcvt_from_sint(field_type, val)
            } else {
                val
            };
            builder.ins().stack_store(
                store_val,
                slot,
                cranelift_codegen::ir::immediates::Offset32::new(offset),
            );
        }
    }

    // Record the struct init for the Let handler to pick up
    cx.last_struct_init = Some((slot, name.to_string()));

    // Return the stack slot address as a pointer
    let ptr = builder
        .ins()
        .stack_addr(clif_types::pointer_type(), slot, 0);
    cx.last_expr_type = Some(clif_types::pointer_type());
    Ok(ptr)
}

/// Compiles field access on a struct variable: `obj.field`.
///
/// Handles two cases:
/// 1. Local struct variable → load from stack slot at field offset
/// 2. `self` pointer parameter (inside impl method) → load from pointer at field offset
pub(in crate::codegen::cranelift) fn compile_field_access<M: Module>(
    builder: &mut FunctionBuilder,
    cx: &mut CodegenCtx<'_, M>,
    object: &Expr,
    field: &str,
) -> Result<ClifValue, CodegenError> {
    let var_name = match object {
        Expr::Ident { name, .. } => name.clone(),
        _ => {
            return Err(CodegenError::NotImplemented(
                "field access on non-ident object".into(),
            ))
        }
    };

    // Case 0: tuple index access (t.0, t.1, etc.)
    if let Some((slot, ref type_name)) = cx.struct_slots.get(&var_name).cloned() {
        if type_name.starts_with("__tuple_") && field.chars().all(|c| c.is_ascii_digit()) {
            let idx: usize = field.parse().map_err(|_| {
                CodegenError::NotImplemented(format!("invalid tuple index '{field}'"))
            })?;
            // Look up the element type from tuple_types if available
            let elem_type = cx
                .tuple_types
                .get(&var_name)
                .and_then(|types| types.get(idx).copied())
                .unwrap_or(clif_types::default_int_type());
            let offset = (idx as i32) * 8;
            let val = builder.ins().stack_load(
                elem_type,
                slot,
                cranelift_codegen::ir::immediates::Offset32::new(offset),
            );
            cx.last_expr_type = Some(elem_type);
            return Ok(val);
        }
    }

    // Case 1: local struct variable with known stack slot
    if let Some((slot, struct_name)) = cx.struct_slots.get(&var_name).cloned() {
        // Check for bitfield access first
        if let Some(layout) = cx.bitfield_layouts.get(&struct_name).cloned() {
            if let Some((_, bit_offset, width)) = layout.iter().find(|(n, _, _)| n == field) {
                // Load the packed i64 from offset 0, extract the bitfield
                let packed = builder.ins().stack_load(
                    clif_types::default_int_type(),
                    slot,
                    cranelift_codegen::ir::immediates::Offset32::new(0),
                );
                let shifted = if *bit_offset > 0 {
                    let shift = builder
                        .ins()
                        .iconst(clif_types::default_int_type(), *bit_offset as i64);
                    builder.ins().ushr(packed, shift)
                } else {
                    packed
                };
                let mask = (1i64 << *width) - 1;
                let val = builder.ins().band_imm(shifted, mask);
                cx.last_expr_type = Some(clif_types::default_int_type());
                return Ok(val);
            }
        }

        let field_defs = cx.struct_defs.get(&struct_name).ok_or_else(|| {
            CodegenError::UndefinedVariable(format!("struct '{struct_name}' not defined"))
        })?;

        let idx = field_defs
            .iter()
            .position(|(fname, _)| fname == field)
            .ok_or_else(|| {
                CodegenError::NotImplemented(format!(
                    "struct '{struct_name}' has no field '{field}'"
                ))
            })?;

        let field_type = field_defs[idx].1;
        let is_union = cx.union_names.contains(&struct_name);
        let offset = if is_union { 0 } else { (idx as i32) * 8 };
        let val = builder.ins().stack_load(
            field_type,
            slot,
            cranelift_codegen::ir::immediates::Offset32::new(offset),
        );
        cx.last_expr_type = Some(field_type);
        return Ok(val);
    }

    // Case 2: `self` pointer parameter (inside impl method)
    // `self` is a pointer to a struct's stack slot passed from the caller
    if var_name == "self" {
        if let Some(&var) = cx.var_map.get("self") {
            // Use current_impl_type (set when compiling impl methods) for precise lookup.
            // Fall back to scanning impl_methods only if current_impl_type is not set.
            let struct_name = cx.current_impl_type.clone().or_else(|| {
                cx.impl_methods
                    .keys()
                    .map(|(type_name, _)| type_name.clone())
                    .find(|type_name| cx.struct_defs.contains_key(type_name))
            });

            if let Some(struct_name) = struct_name {
                let field_defs = &cx.struct_defs[&struct_name];
                let idx = field_defs
                    .iter()
                    .position(|(fname, _)| fname == field)
                    .ok_or_else(|| {
                        CodegenError::NotImplemented(format!(
                            "struct '{struct_name}' has no field '{field}'"
                        ))
                    })?;

                let field_type = field_defs[idx].1;
                let ptr = builder.use_var(var);
                let is_union = cx.union_names.contains(&struct_name);
                let offset = if is_union { 0 } else { (idx as i64) * 8 };
                let offset_val = builder.ins().iconst(clif_types::default_int_type(), offset);
                let field_ptr = builder.ins().iadd(ptr, offset_val);
                let val = builder.ins().load(
                    field_type,
                    cranelift_codegen::ir::MemFlags::new(),
                    field_ptr,
                    0,
                );
                cx.last_expr_type = Some(field_type);
                return Ok(val);
            }
        }
    }

    Err(CodegenError::UndefinedVariable(format!(
        "'{var_name}' is not a struct variable"
    )))
}

/// Compiles field assignment: `obj.field = val` or `obj.field += val`.
pub(in crate::codegen::cranelift) fn compile_field_assign<M: Module>(
    builder: &mut FunctionBuilder,
    cx: &mut CodegenCtx<'_, M>,
    object: &Expr,
    field: &str,
    value: &Expr,
    op: &crate::parser::ast::AssignOp,
) -> Result<ClifValue, CodegenError> {
    use crate::parser::ast::AssignOp;

    let var_name = match object {
        Expr::Ident { name, .. } => name.clone(),
        _ => {
            return Err(CodegenError::NotImplemented(
                "field assign on non-ident object".into(),
            ))
        }
    };

    let (slot, struct_name) = cx.struct_slots.get(&var_name).cloned().ok_or_else(|| {
        CodegenError::UndefinedVariable(format!("'{var_name}' is not a struct variable"))
    })?;

    // Check for bitfield field assignment
    if let Some(layout) = cx.bitfield_layouts.get(&struct_name).cloned() {
        if let Some((_, bit_offset, width)) = layout.iter().find(|(n, _, _)| n == field) {
            let rhs = compile_expr(builder, cx, value)?;
            let mask = (1i64 << *width) - 1;

            let new_bits = match op {
                AssignOp::Assign => {
                    // Mask the new value to the field width
                    builder.ins().band_imm(rhs, mask)
                }
                _ => {
                    // Read current bitfield value first
                    let packed = builder.ins().stack_load(
                        clif_types::default_int_type(),
                        slot,
                        cranelift_codegen::ir::immediates::Offset32::new(0),
                    );
                    let current_shifted = if *bit_offset > 0 {
                        let shift = builder
                            .ins()
                            .iconst(clif_types::default_int_type(), *bit_offset as i64);
                        builder.ins().ushr(packed, shift)
                    } else {
                        packed
                    };
                    let current = builder.ins().band_imm(current_shifted, mask);
                    let combined = match op {
                        AssignOp::AddAssign => builder.ins().iadd(current, rhs),
                        AssignOp::SubAssign => builder.ins().isub(current, rhs),
                        AssignOp::MulAssign => builder.ins().imul(current, rhs),
                        AssignOp::BitAndAssign => builder.ins().band(current, rhs),
                        AssignOp::BitOrAssign => builder.ins().bor(current, rhs),
                        AssignOp::BitXorAssign => builder.ins().bxor(current, rhs),
                        _ => {
                            return Err(CodegenError::NotImplemented(format!(
                                "bitfield compound assign '{op}'"
                            )))
                        }
                    };
                    builder.ins().band_imm(combined, mask)
                }
            };

            // Load packed word, clear target bits, OR in new value
            let packed = builder.ins().stack_load(
                clif_types::default_int_type(),
                slot,
                cranelift_codegen::ir::immediates::Offset32::new(0),
            );
            let clear_mask = !(mask << (*bit_offset as i64));
            let cleared = builder.ins().band_imm(packed, clear_mask);
            let shifted_new = if *bit_offset > 0 {
                let shift = builder
                    .ins()
                    .iconst(clif_types::default_int_type(), *bit_offset as i64);
                builder.ins().ishl(new_bits, shift)
            } else {
                new_bits
            };
            let updated = builder.ins().bor(cleared, shifted_new);
            builder.ins().stack_store(
                updated,
                slot,
                cranelift_codegen::ir::immediates::Offset32::new(0),
            );
            cx.last_expr_type = Some(clif_types::default_int_type());
            return Ok(new_bits);
        }
    }

    let field_defs = cx.struct_defs.get(&struct_name).ok_or_else(|| {
        CodegenError::UndefinedVariable(format!("struct '{struct_name}' not defined"))
    })?;

    let idx = field_defs
        .iter()
        .position(|(fname, _)| fname == field)
        .ok_or_else(|| {
            CodegenError::NotImplemented(format!("struct '{struct_name}' has no field '{field}'"))
        })?;

    let field_type = field_defs[idx].1;
    let is_union = cx.union_names.contains(&struct_name);
    let raw_offset = if is_union { 0 } else { (idx as i32) * 8 };
    let offset = cranelift_codegen::ir::immediates::Offset32::new(raw_offset);
    let rhs = compile_expr(builder, cx, value)?;

    let final_val = match op {
        AssignOp::Assign => rhs,
        _ => {
            let current = builder.ins().stack_load(field_type, slot, offset);
            if clif_types::is_float(field_type) {
                match op {
                    AssignOp::AddAssign => builder.ins().fadd(current, rhs),
                    AssignOp::SubAssign => builder.ins().fsub(current, rhs),
                    AssignOp::MulAssign => builder.ins().fmul(current, rhs),
                    AssignOp::DivAssign => builder.ins().fdiv(current, rhs),
                    _ => {
                        return Err(CodegenError::NotImplemented(format!(
                            "float compound field assign '{op}'"
                        )))
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

    builder.ins().stack_store(final_val, slot, offset);
    Ok(final_val)
}
