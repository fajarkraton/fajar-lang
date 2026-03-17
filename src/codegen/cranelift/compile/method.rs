//! Method call compilation for Cranelift codegen.
//!
//! Contains: compile_method_call, compile_map_method.

use cranelift_codegen::ir::condcodes::IntCC;
use cranelift_codegen::ir::{InstBuilder, Value as ClifValue};
use cranelift_frontend::FunctionBuilder;
use cranelift_module::Module;

use super::super::clif_types;
use super::super::context::CodegenCtx;
#[allow(unused_imports)]
use super::*;
use crate::codegen::CodegenError;
use crate::parser::ast::{CallArg, Expr};

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
