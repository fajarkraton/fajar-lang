//! Inline assembly compilation for Cranelift codegen.
//!
//! Contains: compile_inline_asm, validate_asm_operand_type,
//! validate_asm_register_class, extract_ident_name.

use cranelift_codegen::ir::{InstBuilder, Value as ClifValue};
use cranelift_frontend::FunctionBuilder;
use cranelift_module::Module;

use super::super::clif_types;
use super::super::context::CodegenCtx;
#[allow(unused_imports)]
use super::*;
use crate::codegen::CodegenError;

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
