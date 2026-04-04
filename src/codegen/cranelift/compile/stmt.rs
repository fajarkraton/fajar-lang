//! Statement compilation for Fajar Lang native codegen.
//!
//! Contains `compile_stmt` — the main dispatch for Let, Const, Return,
//! Break, Continue, and expression statements.

use cranelift_codegen::ir::{InstBuilder, Value as ClifValue};
use cranelift_frontend::FunctionBuilder;
use cranelift_module::Module;

use super::super::clif_types;
use super::super::context::{CodegenCtx, OwnedKind, emit_owned_cleanup, push_owned};
use crate::codegen::CodegenError;
use crate::parser::ast::{BinOp, Expr, FnDef, LiteralKind, Stmt, TypeExpr, UnaryOp};

// Re-use sibling functions via the parent module's re-exports.
use super::{compile_expr, compile_heap_array_init};

/// Extracts the function name from the tail expression of a block, if it's a
/// bare `Expr::Ident`. Used to detect patterns like
/// `let f = if cond { handler_a } else { handler_b }`.
fn extract_block_tail_fn_name(stmts: &[Stmt], tail: &Option<Box<Expr>>) -> Option<String> {
    // If there's an explicit tail expression, check it.
    if let Some(tail_expr) = tail {
        return extract_expr_fn_name(tail_expr);
    }
    // Otherwise check the last statement — if it's an expression statement.
    if let Some(Stmt::Expr { expr, .. }) = stmts.last() {
        return extract_expr_fn_name(expr);
    }
    None
}

/// Extracts a function name from an expression that might be a bare ident,
/// a grouped ident, or a block containing a single ident.
fn extract_expr_fn_name(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Ident { name, .. } => Some(name.clone()),
        Expr::Grouped { expr: inner, .. } => extract_expr_fn_name(inner),
        Expr::Block {
            stmts, expr: tail, ..
        } => extract_block_tail_fn_name(stmts, tail),
        _ => None,
    }
}

/// Collects ALL leaf function names from an if/else/else-if chain.
/// Returns `Some(vec)` only if every leaf branch is a known function ident.
fn collect_if_chain_fn_names(expr: &Expr) -> Option<Vec<String>> {
    match expr {
        Expr::If {
            then_branch,
            else_branch: Some(else_branch),
            ..
        } => {
            let then_fn = match then_branch.as_ref() {
                Expr::Block {
                    stmts, expr: tail, ..
                } => extract_block_tail_fn_name(stmts, tail),
                other => extract_expr_fn_name(other),
            };
            let then_name = then_fn?;
            // Else branch may be another if-expression (else if chain)
            let else_names = match else_branch.as_ref() {
                Expr::If { .. } => collect_if_chain_fn_names(else_branch)?,
                Expr::Block {
                    stmts, expr: tail, ..
                } => vec![extract_block_tail_fn_name(stmts, tail)?],
                other => vec![extract_expr_fn_name(other)?],
            };
            let mut all = vec![then_name];
            all.extend(else_names);
            Some(all)
        }
        _ => None,
    }
}

/// Attempts to evaluate a constant expression at compile time.
///
/// Supports integer literals, arithmetic (`+`, `-`, `*`, `/`, `%`),
/// bitwise operations (`&`, `|`, `^`, `<<`, `>>`), power (`**`),
/// unary negation (`-`), bitwise NOT (`~`), and references to previously
/// defined constants via `const_table`.
/// Returns `Some(value)` if the expression is fully constant, `None` otherwise.
pub(in crate::codegen::cranelift) fn try_const_eval(
    expr: &Expr,
    const_table: &std::collections::HashMap<String, i64>,
) -> Option<i64> {
    try_const_eval_with_fns(expr, const_table, &std::collections::HashMap::new(), 0)
}

/// Try to evaluate an array expression at compile time.
/// Returns the array elements as a Vec<i64>, or None if not const-evaluable.
#[allow(dead_code)]
pub(in crate::codegen::cranelift) fn try_const_eval_array(
    expr: &Expr,
    const_table: &std::collections::HashMap<String, i64>,
    const_arrays: &std::collections::HashMap<String, Vec<i64>>,
    const_fns: &std::collections::HashMap<String, &FnDef>,
) -> Option<Vec<i64>> {
    match expr {
        Expr::Array { elements, .. } => {
            let mut vals = Vec::new();
            for elem in elements {
                let v = try_const_eval_with_fns(elem, const_table, const_fns, 0)?;
                vals.push(v);
            }
            Some(vals)
        }
        Expr::ArrayRepeat { value, count, .. } => {
            let v = try_const_eval_with_fns(value, const_table, const_fns, 0)?;
            let n = try_const_eval_with_fns(count, const_table, const_fns, 0)?;
            if !(0..=65536).contains(&n) {
                return None;
            }
            Some(vec![v; n as usize])
        }
        Expr::Ident { name, .. } => const_arrays.get(name).cloned(),
        _ => None,
    }
}

/// Extended const evaluation that supports const fn calls.
/// `const_fns` maps function name → FnDef for functions marked `const fn`.
/// `depth` tracks recursion to prevent infinite loops (max 128).
pub(in crate::codegen::cranelift) fn try_const_eval_with_fns(
    expr: &Expr,
    const_table: &std::collections::HashMap<String, i64>,
    const_fns: &std::collections::HashMap<String, &FnDef>,
    depth: usize,
) -> Option<i64> {
    if depth > 128 {
        return None; // Recursion limit
    }
    match expr {
        Expr::Literal {
            kind: LiteralKind::Int(n),
            ..
        } => Some(*n),
        Expr::Literal {
            kind: LiteralKind::Bool(b),
            ..
        } => Some(if *b { 1 } else { 0 }),
        Expr::Ident { name, .. } => const_table.get(name).copied(),
        Expr::Grouped { expr, .. } => try_const_eval_with_fns(expr, const_table, const_fns, depth),
        Expr::Unary { op, operand, .. } => {
            let val = try_const_eval_with_fns(operand, const_table, const_fns, depth)?;
            match op {
                UnaryOp::Neg => Some(-val),
                UnaryOp::BitNot => Some(!val),
                _ => None,
            }
        }
        Expr::Binary {
            op, left, right, ..
        } => {
            let l = try_const_eval_with_fns(left, const_table, const_fns, depth)?;
            let r = try_const_eval_with_fns(right, const_table, const_fns, depth)?;
            match op {
                BinOp::Add => l.checked_add(r),
                BinOp::Sub => l.checked_sub(r),
                BinOp::Mul => l.checked_mul(r),
                BinOp::Div => {
                    if r == 0 {
                        None
                    } else {
                        l.checked_div(r)
                    }
                }
                BinOp::Rem => {
                    if r == 0 {
                        None
                    } else {
                        l.checked_rem(r)
                    }
                }
                BinOp::BitAnd => Some(l & r),
                BinOp::BitOr => Some(l | r),
                BinOp::BitXor => Some(l ^ r),
                BinOp::Shl => Some(l << (r & 63)),
                BinOp::Shr => Some(l >> (r & 63)),
                BinOp::Pow => {
                    if r < 0 {
                        None
                    } else {
                        Some(l.wrapping_pow(r as u32))
                    }
                }
                BinOp::Eq => Some(if l == r { 1 } else { 0 }),
                BinOp::Ne => Some(if l != r { 1 } else { 0 }),
                BinOp::Lt => Some(if l < r { 1 } else { 0 }),
                BinOp::Le => Some(if l <= r { 1 } else { 0 }),
                BinOp::Gt => Some(if l > r { 1 } else { 0 }),
                BinOp::Ge => Some(if l >= r { 1 } else { 0 }),
                _ => None,
            }
        }
        // If-expression: evaluate condition, then branch
        Expr::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            let cond = try_const_eval_with_fns(condition, const_table, const_fns, depth)?;
            if cond != 0 {
                try_const_eval_with_fns(then_branch, const_table, const_fns, depth)
            } else if let Some(eb) = else_branch {
                try_const_eval_with_fns(eb, const_table, const_fns, depth)
            } else {
                Some(0)
            }
        }
        // Function call: try const fn evaluation
        Expr::Call { callee, args, .. } => {
            // Extract function name
            let fn_name = match callee.as_ref() {
                Expr::Ident { name, .. } => name.clone(),
                _ => return None,
            };
            // Look up const fn definition
            let fndef = const_fns.get(&fn_name)?;
            if !fndef.is_const {
                return None;
            }
            // Evaluate all arguments (CallArg has .value field)
            let mut arg_vals = Vec::new();
            for arg in args {
                let val = try_const_eval_with_fns(&arg.value, const_table, const_fns, depth)?;
                arg_vals.push(val);
            }
            if arg_vals.len() != fndef.params.len() {
                return None;
            }
            // Build local const table with parameter bindings
            let mut local_table = const_table.clone();
            for (param, val) in fndef.params.iter().zip(arg_vals.iter()) {
                local_table.insert(param.name.clone(), *val);
            }
            // Evaluate body
            try_const_eval_with_fns(&fndef.body, &local_table, const_fns, depth + 1)
        }
        // Block expression: evaluate statements, return last expression
        Expr::Block {
            stmts, expr: tail, ..
        } => {
            let mut local_table = const_table.clone();
            for stmt in stmts {
                match stmt {
                    Stmt::Let { name, value, .. } | Stmt::Const { name, value, .. } => {
                        let val = try_const_eval_with_fns(value, &local_table, const_fns, depth)?;
                        local_table.insert(name.clone(), val);
                    }
                    Stmt::Return { value, .. } => {
                        if let Some(v) = value {
                            return try_const_eval_with_fns(v, &local_table, const_fns, depth);
                        }
                        return Some(0);
                    }
                    Stmt::Expr { expr, .. } => {
                        try_const_eval_with_fns(expr, &local_table, const_fns, depth)?;
                    }
                    _ => return None, // Non-const statement
                }
            }
            if let Some(t) = tail {
                try_const_eval_with_fns(t, &local_table, const_fns, depth)
            } else {
                Some(0)
            }
        }
        // Array indexing: TABLE[2] where TABLE is a const array
        Expr::Index { object, index, .. } => {
            let idx = try_const_eval_with_fns(index, const_table, const_fns, depth)?;
            // Try to get the array from const_arrays (empty map for now — callers pass it separately)
            // For now, handle inline array literals: [1,2,3][1] → 2
            if let Expr::Array { elements, .. } = object.as_ref() {
                if idx < 0 || idx as usize >= elements.len() {
                    return None;
                }
                try_const_eval_with_fns(&elements[idx as usize], const_table, const_fns, depth)
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Coerces an integer value to match a declared sub-I64 integer type.
///
/// For types narrower than I64 (u8, u16, u32, i8, i16, i32), truncates
/// the value via `ireduce` then extends back to I64 for uniform storage.
/// Signed types use `sextend`, unsigned types use `uextend`.
/// Returns the value unchanged if no coercion is needed.
fn coerce_int_to_declared_type(
    builder: &mut FunctionBuilder,
    val: ClifValue,
    type_name: &str,
) -> ClifValue {
    // Check by name to distinguish u8/i8 from bool (both are I8 in Cranelift)
    let target_ty = match type_name {
        "u8" | "i8" => cranelift_codegen::ir::types::I8,
        "u16" | "i16" => cranelift_codegen::ir::types::I16,
        "u32" | "i32" => cranelift_codegen::ir::types::I32,
        _ => return val, // No coercion for i64, f64, bool, etc.
    };
    let val_ty = builder.func.dfg.value_type(val);
    if val_ty.bits() <= target_ty.bits() {
        return val; // Already narrow enough
    }
    // Truncate to target width, then extend back to I64 for uniform representation
    let narrow = builder.ins().ireduce(target_ty, val);
    match type_name {
        "i8" | "i16" | "i32" => builder
            .ins()
            .sextend(clif_types::default_int_type(), narrow),
        _ => builder
            .ins()
            .uextend(clif_types::default_int_type(), narrow),
    }
}

/// Coerces a return value to match the declared function return type.
/// Handles i64→i8 (bool), i8→i64, etc.
fn coerce_return_value(
    builder: &mut FunctionBuilder,
    val: ClifValue,
    expected: Option<cranelift_codegen::ir::Type>,
) -> ClifValue {
    let Some(expected_ty) = expected else {
        return val;
    };
    let actual_ty = builder.func.dfg.value_type(val);
    if actual_ty == expected_ty {
        return val;
    }
    if clif_types::is_float(actual_ty) || clif_types::is_float(expected_ty) {
        return val;
    }
    if actual_ty.bits() > expected_ty.bits() {
        builder.ins().ireduce(expected_ty, val)
    } else {
        builder.ins().uextend(expected_ty, val)
    }
}

/// Compiles a single statement.
pub(in crate::codegen::cranelift) fn compile_stmt<M: Module>(
    builder: &mut FunctionBuilder,
    cx: &mut CodegenCtx<'_, M>,
    stmt: &Stmt,
) -> Result<Option<ClifValue>, CodegenError> {
    match stmt {
        Stmt::Expr { expr, .. } => {
            let val = compile_expr(builder, cx, expr)?;
            Ok(Some(val))
        }
        Stmt::Let {
            name, value, ty, ..
        } => {
            // Detect empty array literal → create heap-backed dynamic array
            if let Expr::Array { elements, .. } = value.as_ref() {
                if elements.is_empty() {
                    return compile_heap_array_init(builder, cx, name, elements);
                }
            }

            let val = compile_expr(builder, cx, value)?;
            // Track element type before overriding for array pointer storage
            let elem_type_for_array = cx.last_expr_type;

            // B3.2: Determine semantic type from annotation (may be sub-I64)
            let semantic_type = ty.as_ref().and_then(clif_types::lower_type);

            // B3.2: Coerce value to match declared sub-I64 integer type.
            // Truncates (e.g., I64 → I32 → I64) to enforce width semantics
            // while keeping uniform I64 storage for all variables.
            let val = if let Some(TypeExpr::Simple { name: tn, .. }) = ty {
                coerce_int_to_declared_type(builder, val, tn)
            } else {
                val
            };

            // Variable always declared as I64 for uniform representation,
            // unless it's a bool (I8), float (F64/F32), or other special type.
            // Check the type NAME to distinguish u8/i8 (→ I64 storage) from bool (→ I8 storage).
            let is_sub_i64_int = matches!(
                ty.as_ref(),
                Some(TypeExpr::Simple { name, .. })
                    if matches!(name.as_str(), "u8" | "i8" | "u16" | "i16" | "u32" | "i32")
            );
            let var_type = if is_sub_i64_int {
                clif_types::default_int_type() // sub-I64 integers stored as I64
            } else if let Some(st) = semantic_type {
                st // bools, floats, i64, i128 stay as-is
            } else if cx.last_array.is_some() {
                clif_types::pointer_type()
            } else {
                // Infer type from the last compiled expression
                cx.last_expr_type.unwrap_or(clif_types::default_int_type())
            };
            let var = builder.declare_var(var_type);
            builder.def_var(var, val);
            cx.var_map.insert(name.clone(), var);

            // If RHS was an array literal, associate metadata with this variable
            // and store element type (not pointer type) in var_types for indexing
            if let Some(meta) = cx.last_array.take() {
                cx.array_meta.insert(name.clone(), meta);
                cx.var_types.insert(
                    name.clone(),
                    elem_type_for_array.unwrap_or(clif_types::default_int_type()),
                );
            } else {
                // B3.2: Store semantic type (e.g., I32 for u32) for type-aware
                // arithmetic propagation in B3.3
                cx.var_types
                    .insert(name.clone(), semantic_type.unwrap_or(var_type));
            }
            // If RHS was a string, save the length variable.
            // Only register as string if the variable actually holds a pointer
            // (not a comparison result or other non-string value).
            if let Some(len_val) = cx.last_string_len.take() {
                if var_type == clif_types::pointer_type()
                    || var_type == clif_types::default_int_type()
                {
                    // Check that the expression actually produces a string (not a comparison, etc.)
                    let is_string_rhs = matches!(
                        value.as_ref(),
                        Expr::Literal {
                            kind: LiteralKind::String(_) | LiteralKind::RawString(_),
                            ..
                        } | Expr::Call { .. }
                            | Expr::MethodCall { .. }
                            | Expr::If { .. }
                            | Expr::Index { .. }
                            | Expr::Match { .. }
                            | Expr::FString { .. }
                            | Expr::Block { .. }
                    ) || matches!(value.as_ref(), Expr::Ident { name: vn, .. } if cx.string_lens.contains_key(vn))
                        || matches!(value.as_ref(), Expr::Binary { op: BinOp::Add, .. });
                    if is_string_rhs {
                        let len_var = builder.declare_var(clif_types::default_int_type());
                        builder.def_var(len_var, len_val);
                        cx.string_lens.insert(name.clone(), len_var);
                        // Track heap-allocated strings for cleanup
                        if cx.last_string_owned {
                            push_owned(cx, name.clone(), OwnedKind::String);
                            cx.last_string_owned = false;
                        }
                    }
                }
            }
            // If RHS returned a heap array (e.g., str.chars()), register it
            if cx.last_heap_array {
                cx.heap_arrays.insert(name.clone());
                push_owned(cx, name.clone(), OwnedKind::Array);
                cx.last_heap_array = false;
            }
            // If RHS was a call to a function returning a heap array (Slice type)
            // Note: we do NOT add to owned_ptrs here because the returned pointer
            // may alias the caller's existing heap array (ownership not transferred).
            if cx.last_heap_array_return {
                cx.heap_arrays.insert(name.clone());
                cx.last_heap_array_return = false;
            }
            // If RHS is an identifier that's a heap array, propagate (e.g., let out = arr)
            if let Expr::Ident { name: rhs_name, .. } = value.as_ref() {
                if cx.heap_arrays.contains(rhs_name) {
                    cx.heap_arrays.insert(name.clone());
                }
            }
            // If RHS was a HashMap::new(), register for cleanup
            if cx.last_map_new {
                cx.heap_maps.insert(name.clone());
                push_owned(cx, name.clone(), OwnedKind::Map);
                cx.last_map_new = false;
            }
            // If RHS was a thread::spawn(), register handle for method dispatch
            if cx.last_thread_spawn {
                cx.thread_handles.insert(name.clone());
                cx.last_thread_spawn = false;
            }
            // If RHS was a Mutex::new(), register for method dispatch
            if cx.last_mutex_new {
                cx.mutex_handles.insert(name.clone());
                cx.last_mutex_new = false;
            }
            // If RHS was a mutex.lock_guard(), register for auto-cleanup
            if cx.last_mutex_guard_new {
                cx.mutex_guard_handles.insert(name.clone());
                push_owned(cx, name.clone(), OwnedKind::MutexGuard);
                cx.last_mutex_guard_new = false;
            }
            // If RHS was a channel::new(), register for method dispatch
            if cx.last_channel_new {
                cx.channel_handles.insert(name.clone());
                cx.last_channel_new = false;
            }
            // If RHS was an Atomic::new(), register for method dispatch
            if cx.last_atomic_new {
                cx.atomic_handles.insert(name.clone());
                cx.atomic_subtypes
                    .insert(name.clone(), cx.last_atomic_subtype.clone());
                cx.last_atomic_new = false;
            }
            // If RHS was a closure handle, register for handle-based dispatch
            if cx.last_closure_handle {
                cx.closure_handle_vars.insert(name.clone());
                cx.last_closure_handle = false;
            }
            // If RHS was a RwLock::new(), register for method dispatch
            if cx.last_rwlock_new {
                cx.rwlock_handles.insert(name.clone());
                cx.last_rwlock_new = false;
            }
            // If RHS was a Barrier::new(), register for method dispatch
            if cx.last_barrier_new {
                cx.barrier_handles.insert(name.clone());
                cx.last_barrier_new = false;
            }
            // If RHS was a Condvar::new(), register for method dispatch
            if cx.last_condvar_new {
                cx.condvar_handles.insert(name.clone());
                cx.last_condvar_new = false;
            }
            // If RHS was a channel::bounded(), register for method dispatch
            if cx.last_bounded_channel {
                cx.bounded_channel_handles.insert(name.clone());
                cx.last_bounded_channel = false;
            }
            // If RHS was an Arc::new(), register for method dispatch
            if cx.last_arc_new {
                cx.arc_handles.insert(name.clone());
                cx.last_arc_new = false;
            }
            // If RHS was a VolatilePtr::new(), register for method dispatch
            if cx.last_volatile_ptr_new {
                cx.volatile_ptr_handles.insert(name.clone());
                cx.last_volatile_ptr_new = false;
            }
            // If RHS was an MmioRegion::new(), register base+size vars
            if cx.last_mmio_new {
                if let Some((_base_val, size_val)) = cx.last_mmio_vals.take() {
                    let size_var = builder.declare_var(clif_types::default_int_type());
                    builder.def_var(size_var, size_val);
                    cx.mmio_regions.insert(name.clone(), (var, size_var));
                }
                cx.last_mmio_new = false;
            }
            // If RHS was a BumpAllocator::new(), register for method dispatch + cleanup
            if cx.last_bump_alloc_new {
                cx.bump_alloc_handles.insert(name.clone());
                push_owned(cx, name.clone(), OwnedKind::BumpAllocator);
                cx.last_bump_alloc_new = false;
            }
            // If RHS was a FreeListAllocator::new(), register for method dispatch + cleanup
            if cx.last_freelist_alloc_new {
                cx.freelist_alloc_handles.insert(name.clone());
                push_owned(cx, name.clone(), OwnedKind::FreeListAllocator);
                cx.last_freelist_alloc_new = false;
            }
            // If RHS was a PoolAllocator::new(), register for method dispatch + cleanup
            if cx.last_pool_alloc_new {
                cx.pool_alloc_handles.insert(name.clone());
                push_owned(cx, name.clone(), OwnedKind::PoolAllocator);
                cx.last_pool_alloc_new = false;
            }
            // If RHS was an Executor::new(), register for method dispatch
            if cx.last_executor_new {
                cx.executor_handles.insert(name.clone());
                cx.last_executor_new = false;
            }
            // If RHS was a Waker::new(), register for method dispatch
            if cx.last_waker_new {
                cx.waker_handles.insert(name.clone());
                cx.last_waker_new = false;
            }
            // If RHS was a Timer::new(), register for method dispatch
            if cx.last_timer_new {
                cx.timer_handles.insert(name.clone());
                cx.last_timer_new = false;
            }
            // If RHS was a ThreadPool::new(), register for method dispatch
            if cx.last_threadpool_new {
                cx.threadpool_handles.insert(name.clone());
                cx.last_threadpool_new = false;
            }
            // If RHS was a spawn_join(), register for JoinHandle method dispatch
            if cx.last_joinhandle_new {
                cx.joinhandle_handles.insert(name.clone());
                cx.last_joinhandle_new = false;
            }
            // If RHS was an AsyncChannel::new(), register for method dispatch
            if cx.last_async_channel_new {
                cx.async_channel_handles.insert(name.clone());
                cx.last_async_channel_new = false;
            }
            // If RHS was an AsyncChannel::bounded(), register for method dispatch
            if cx.last_async_bchannel_new {
                cx.async_bchannel_handles.insert(name.clone());
                cx.last_async_bchannel_new = false;
            }
            // If RHS was a Stream constructor/combinator, register for method dispatch
            if cx.last_stream_new {
                cx.stream_handles.insert(name.clone());
                cx.last_stream_new = false;
            }
            // ONNX handle tracking
            if cx.last_onnx_new {
                cx.onnx_handles.insert(name.clone());
                cx.last_onnx_new = false;
            }
            // SIMD handle tracking
            if cx.last_simd_f32x4_new {
                cx.simd_f32x4_handles.insert(name.clone());
                cx.last_simd_f32x4_new = false;
            }
            if cx.last_simd_i32x4_new {
                cx.simd_i32x4_handles.insert(name.clone());
                cx.last_simd_i32x4_new = false;
            }
            if cx.last_simd_f32x8_new {
                cx.simd_f32x8_handles.insert(name.clone());
                cx.last_simd_f32x8_new = false;
            }
            if cx.last_simd_i32x8_new {
                cx.simd_i32x8_handles.insert(name.clone());
                cx.last_simd_i32x8_new = false;
            }
            // Async I/O handle tracking
            if cx.last_async_io_new {
                cx.async_io_handles.insert(name.clone());
                cx.last_async_io_new = false;
            }
            // If RHS was a split() result, track it
            if cx.last_split_result.take().is_some() {
                cx.split_vars.insert(name.clone());
            }
            // If RHS was an enum constructor, save the payload variable + type
            if let Some(payload_val) = cx.last_enum_payload.take() {
                let payload_type = cx
                    .last_enum_payload_type
                    .take()
                    .unwrap_or(clif_types::default_int_type());
                let payload_var = builder.declare_var(payload_type);
                builder.def_var(payload_var, payload_val);
                cx.enum_vars
                    .insert(name.clone(), (var, payload_var, payload_type));
            }
            // If RHS was a multi-field enum constructor, track the stack slot + field types
            if let Some((slot, field_types)) = cx.last_enum_multi_payload.take() {
                cx.enum_multi_vars.insert(name.clone(), (slot, field_types));
            }
            // If RHS was a struct init, associate the stack slot with this variable
            if let Some((slot, struct_name)) = cx.last_struct_init.take() {
                // S3.4: If this struct implements Drop, register for auto-cleanup
                let drop_key = ("Drop".to_string(), struct_name.clone());
                if cx.trait_impls.contains_key(&drop_key) {
                    let drop_fn = format!("{}_drop", struct_name);
                    push_owned(cx, name.clone(), OwnedKind::Droppable(drop_fn));
                }
                cx.struct_slots.insert(name.clone(), (slot, struct_name));
            }
            // If RHS was a tuple, save element types for type-aware index access
            if let Some(elem_types) = cx.last_tuple_elem_types.take() {
                cx.tuple_types.insert(name.clone(), elem_types);
            }
            // If type annotation is a fn pointer, record signature for call_indirect
            if let Some(crate::parser::ast::TypeExpr::Fn {
                params,
                return_type,
                ..
            }) = ty
            {
                let param_types: Vec<_> = params
                    .iter()
                    .map(|p| clif_types::lower_type(p).unwrap_or(clif_types::default_int_type()))
                    .collect();
                let ret_type = clif_types::lower_type(return_type);
                cx.fn_ptr_sigs.insert(name.clone(), (param_types, ret_type));
            }
            // If RHS is a known function name, infer signature and record as fn pointer.
            // This enables `let f = add; f(3, 4)` without explicit type annotation.
            if let Expr::Ident { name: rhs_name, .. } = value.as_ref() {
                if !cx.fn_ptr_sigs.contains_key(name) {
                    if let Some(&fn_id) = cx.functions.get(rhs_name) {
                        // Get function signature from the module
                        let decl = cx.module.declarations().get_function_decl(fn_id);
                        let param_types: Vec<_> =
                            decl.signature.params.iter().map(|p| p.value_type).collect();
                        let ret_type = decl.signature.returns.first().map(|r| r.value_type);
                        // Store function address in the variable
                        let fn_ref = cx.module.declare_func_in_func(fn_id, builder.func);
                        let fn_addr = builder
                            .ins()
                            .func_addr(clif_types::default_int_type(), fn_ref);
                        builder.def_var(var, fn_addr);
                        cx.fn_ptr_sigs.insert(name.clone(), (param_types, ret_type));
                    }
                }
            }
            // If RHS is an if/else-if/else chain where ALL leaf branches resolve
            // to known function names with the same signature, infer fn_ptr_sigs.
            // Handles: `let f = if a { x } else if b { y } else { z }`
            if !cx.fn_ptr_sigs.contains_key(name) {
                if let Some(leaf_names) = collect_if_chain_fn_names(value.as_ref()) {
                    // Verify all leaf names are known functions with matching signatures
                    let fn_ids: Vec<_> = leaf_names
                        .iter()
                        .filter_map(|n| cx.functions.get(n).copied())
                        .collect();
                    if fn_ids.len() == leaf_names.len() && !fn_ids.is_empty() {
                        let first_decl = cx.module.declarations().get_function_decl(fn_ids[0]);
                        let all_same = fn_ids.iter().skip(1).all(|&fid| {
                            let d = cx.module.declarations().get_function_decl(fid);
                            d.signature.params.len() == first_decl.signature.params.len()
                                && d.signature.returns.len() == first_decl.signature.returns.len()
                        });
                        if all_same {
                            let param_types: Vec<_> = first_decl
                                .signature
                                .params
                                .iter()
                                .map(|p| p.value_type)
                                .collect();
                            let ret_type =
                                first_decl.signature.returns.first().map(|r| r.value_type);
                            cx.fn_ptr_sigs.insert(name.clone(), (param_types, ret_type));
                        }
                    }
                }
            }
            // If RHS is an array literal where all elements are known function
            // names with the same signature, record fn_ptr_sigs for the array
            // variable so that `arr[i](x)` can be compiled as call_indirect.
            if !cx.fn_ptr_sigs.contains_key(name) {
                if let Expr::Array { elements, .. } = value.as_ref() {
                    if !elements.is_empty() {
                        let fn_names: Vec<Option<String>> =
                            elements.iter().map(extract_expr_fn_name).collect();
                        if fn_names.iter().all(|n| n.is_some()) {
                            let fn_names: Vec<String> = fn_names.into_iter().flatten().collect();
                            // Check all names are known functions
                            let fn_ids: Vec<_> = fn_names
                                .iter()
                                .filter_map(|n| cx.functions.get(n).copied())
                                .collect();
                            if fn_ids.len() == fn_names.len() {
                                // Verify all have the same signature
                                let first_decl =
                                    cx.module.declarations().get_function_decl(fn_ids[0]);
                                let all_same = fn_ids.iter().skip(1).all(|&fid| {
                                    let d = cx.module.declarations().get_function_decl(fid);
                                    d.signature.params.len() == first_decl.signature.params.len()
                                        && d.signature.returns.len()
                                            == first_decl.signature.returns.len()
                                });
                                if all_same {
                                    let param_types: Vec<_> = first_decl
                                        .signature
                                        .params
                                        .iter()
                                        .map(|p| p.value_type)
                                        .collect();
                                    let ret_type =
                                        first_decl.signature.returns.first().map(|r| r.value_type);
                                    cx.fn_ptr_sigs.insert(name.clone(), (param_types, ret_type));
                                }
                            }
                        }
                    }
                }
            }
            // If RHS is a function call that returns a function pointer and we
            // have a fn-type annotation, the annotation handler above already
            // registered it. No extra work needed here.
            Ok(None)
        }
        Stmt::Const {
            name, value, ty, ..
        } => {
            // Try compile-time constant evaluation first
            let const_folded = try_const_eval(value, &cx.const_values);
            let val = if let Some(const_val) = const_folded {
                let ty = clif_types::default_int_type();
                builder.ins().iconst(ty, const_val)
            } else {
                compile_expr(builder, cx, value)?
            };
            // Store in const table for future const references
            if let Some(cv) = const_folded {
                cx.const_values.insert(name.clone(), cv);
            }
            let semantic_type = clif_types::lower_type(ty);
            // B3.2: Coerce value for sub-I64 integer types
            let val = if let TypeExpr::Simple { name: tn, .. } = ty {
                coerce_int_to_declared_type(builder, val, tn)
            } else {
                val
            };
            let is_sub_i64_int = matches!(
                ty,
                TypeExpr::Simple { name, .. }
                    if matches!(name.as_str(), "u8" | "i8" | "u16" | "i16" | "u32" | "i32")
            );
            let var_type = if is_sub_i64_int {
                clif_types::default_int_type()
            } else if let Some(st) = semantic_type {
                st
            } else {
                cx.last_expr_type.unwrap_or(clif_types::default_int_type())
            };
            let var = builder.declare_var(var_type);
            builder.def_var(var, val);
            cx.var_map.insert(name.clone(), var);
            cx.var_types
                .insert(name.clone(), semantic_type.unwrap_or(var_type));
            // Handle string/array/enum/struct metadata (same as Let)
            if let Some(meta) = cx.last_array.take() {
                cx.array_meta.insert(name.clone(), meta);
            }
            if let Some(len_val) = cx.last_string_len.take() {
                let len_var = builder.declare_var(clif_types::default_int_type());
                builder.def_var(len_var, len_val);
                cx.string_lens.insert(name.clone(), len_var);
                if cx.last_string_owned {
                    push_owned(cx, name.clone(), OwnedKind::String);
                    cx.last_string_owned = false;
                }
            }
            if cx.last_heap_array {
                cx.heap_arrays.insert(name.clone());
                push_owned(cx, name.clone(), OwnedKind::Array);
                cx.last_heap_array = false;
            }
            if let Some(payload_val) = cx.last_enum_payload.take() {
                let payload_type = cx
                    .last_enum_payload_type
                    .take()
                    .unwrap_or(clif_types::default_int_type());
                let payload_var = builder.declare_var(payload_type);
                builder.def_var(payload_var, payload_val);
                cx.enum_vars
                    .insert(name.clone(), (var, payload_var, payload_type));
            }
            if let Some((slot, field_types)) = cx.last_enum_multi_payload.take() {
                cx.enum_multi_vars.insert(name.clone(), (slot, field_types));
            }
            if let Some((slot, struct_name)) = cx.last_struct_init.take() {
                // S3.4: If this struct implements Drop, register for auto-cleanup
                let drop_key = ("Drop".to_string(), struct_name.clone());
                if cx.trait_impls.contains_key(&drop_key) {
                    let drop_fn = format!("{}_drop", struct_name);
                    push_owned(cx, name.clone(), OwnedKind::Droppable(drop_fn));
                }
                cx.struct_slots.insert(name.clone(), (slot, struct_name));
            }
            if let Some(elem_types) = cx.last_tuple_elem_types.take() {
                cx.tuple_types.insert(name.clone(), elem_types);
            }
            Ok(None)
        }
        Stmt::Return { value, .. } => {
            if let Some(expr) = value {
                let val = compile_expr(builder, cx, expr)?;
                // Check if this function returns a struct
                if let Some((slot, ref sname)) = cx.last_struct_init {
                    if let Some(fields) = cx.struct_defs.get(sname).cloned() {
                        let mut ret_vals = Vec::new();
                        for (i, (_fname, ftype)) in fields.iter().enumerate() {
                            let fv = builder.ins().stack_load(*ftype, slot, (i as i32) * 8);
                            ret_vals.push(fv);
                        }
                        cx.last_struct_init = None;
                        emit_owned_cleanup(builder, cx, None)?;
                        builder.ins().return_(&ret_vals);
                    } else {
                        emit_owned_cleanup(builder, cx, Some(val))?;
                        builder.ins().return_(&[val]);
                    }
                } else if cx.is_enum_return_fn {
                    // Enum-returning function: return both tag and payload
                    let payload = cx
                        .last_enum_payload
                        .take()
                        .unwrap_or_else(|| builder.ins().iconst(clif_types::default_int_type(), 0));
                    cx.last_enum_payload_type.take();
                    emit_owned_cleanup(builder, cx, Some(val))?;
                    builder.ins().return_(&[val, payload]);
                } else {
                    emit_owned_cleanup(builder, cx, Some(val))?;
                    // Coerce return value to match declared return type
                    let val = coerce_return_value(builder, val, cx.fn_ret_type);
                    builder.ins().return_(&[val]);
                }
            } else {
                emit_owned_cleanup(builder, cx, None)?;
                // Void return: always return i64(0) since Fajar Lang functions
                // default to i64 return type even when no explicit type specified
                let ret_ty = cx.fn_ret_type.unwrap_or(clif_types::default_int_type());
                let zero = builder.ins().iconst(ret_ty, 0);
                builder.ins().return_(&[zero]);
            }
            // Switch to a new unreachable block so subsequent instructions
            // don't try to add to the already-terminated block.
            let after_return = builder.create_block();
            builder.switch_to_block(after_return);
            builder.seal_block(after_return);
            Ok(None)
        }
        Stmt::Break { label, .. } => {
            let exit_block = if let Some(lbl) = label {
                // Labeled break: look up the named loop's exit block
                cx.labeled_loops
                    .get(lbl)
                    .map(|&(_, exit)| exit)
                    .ok_or_else(|| {
                        CodegenError::NotImplemented(format!("unknown loop label '{lbl}"))
                    })?
            } else {
                cx.loop_exit
                    .ok_or_else(|| CodegenError::NotImplemented("break outside loop".into()))?
            };
            builder.ins().jump(exit_block, &[]);
            // Switch to an unreachable block so subsequent stmts don't panic.
            let after = builder.create_block();
            builder.switch_to_block(after);
            builder.seal_block(after);
            Ok(None)
        }
        Stmt::Continue { label, .. } => {
            let header = if let Some(lbl) = label {
                // Labeled continue: look up the named loop's header block
                cx.labeled_loops
                    .get(lbl)
                    .map(|&(hdr, _)| hdr)
                    .ok_or_else(|| {
                        CodegenError::NotImplemented(format!("unknown loop label '{lbl}"))
                    })?
            } else {
                cx.loop_header
                    .ok_or_else(|| CodegenError::NotImplemented("continue outside loop".into()))?
            };
            builder.ins().jump(header, &[]);
            // Switch to an unreachable block so subsequent stmts don't panic.
            let after = builder.create_block();
            builder.switch_to_block(after);
            builder.seal_block(after);
            Ok(None)
        }
        // Nested items (fn, struct, enum, impl): skip — extracted at module level.
        Stmt::Item(_) => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::tokenize;
    use crate::parser::parse;
    use std::collections::HashMap;

    /// Helper: parse an expression from source and try const eval.
    fn const_eval_expr(src: &str) -> Option<i64> {
        let full = format!("fn main() -> i64 {{ {src} }}");
        let tokens = tokenize(&full).unwrap();
        let program = parse(tokens).unwrap();
        // Extract the body expression from the function
        if let crate::parser::ast::Item::FnDef(fndef) = &program.items[0] {
            if let Expr::Block { stmts, expr, .. } = &*fndef.body {
                if let Some(tail) = expr {
                    return try_const_eval(tail, &HashMap::new());
                }
                if let Some(crate::parser::ast::Stmt::Expr { expr, .. }) = stmts.first() {
                    return try_const_eval(expr, &HashMap::new());
                }
            }
        }
        None
    }

    #[test]
    fn const_eval_int_literal() {
        assert_eq!(const_eval_expr("42"), Some(42));
    }

    #[test]
    fn const_eval_arithmetic() {
        assert_eq!(const_eval_expr("4096 * 16"), Some(65536));
        assert_eq!(const_eval_expr("100 / 2 + 3"), Some(53));
        assert_eq!(const_eval_expr("10 - 3 * 2"), Some(4));
        assert_eq!(const_eval_expr("7 % 3"), Some(1));
    }

    #[test]
    fn const_eval_bitwise() {
        assert_eq!(const_eval_expr("0xFF & 0x0F"), Some(0x0F));
        assert_eq!(const_eval_expr("0x0F | 0xF0"), Some(0xFF));
        assert_eq!(const_eval_expr("0xFF ^ 0x0F"), Some(0xF0));
        assert_eq!(const_eval_expr("1 << 10"), Some(1024));
        assert_eq!(const_eval_expr("1024 >> 5"), Some(32));
    }

    #[test]
    fn const_eval_unary() {
        assert_eq!(const_eval_expr("-42"), Some(-42));
        assert_eq!(const_eval_expr("~0"), Some(-1));
    }

    #[test]
    fn const_eval_power() {
        assert_eq!(const_eval_expr("2 ** 10"), Some(1024));
        assert_eq!(const_eval_expr("3 ** 3"), Some(27));
    }

    #[test]
    fn const_eval_nested() {
        assert_eq!(const_eval_expr("(4096 + 512) * 2"), Some(9216));
        assert_eq!(const_eval_expr("(1 << 12) | (1 << 8)"), Some(4352));
    }

    #[test]
    fn const_eval_div_by_zero_returns_none() {
        assert_eq!(const_eval_expr("42 / 0"), None);
        assert_eq!(const_eval_expr("42 % 0"), None);
    }

    #[test]
    fn const_eval_non_const_returns_none() {
        // Function call is not const
        assert_eq!(const_eval_expr("foo()"), None);
    }

    #[test]
    fn const_eval_with_const_table() {
        let mut table = HashMap::new();
        table.insert("PAGE_SIZE".to_string(), 4096i64);
        table.insert("NUM_PAGES".to_string(), 16i64);

        let full = "fn main() -> i64 { PAGE_SIZE * NUM_PAGES }";
        let tokens = tokenize(full).unwrap();
        let program = parse(tokens).unwrap();
        if let crate::parser::ast::Item::FnDef(fndef) = &program.items[0] {
            if let Expr::Block {
                expr: Some(tail), ..
            } = &*fndef.body
            {
                assert_eq!(try_const_eval(tail, &table), Some(65536));
            }
        }
    }

    #[test]
    fn const_eval_bool() {
        assert_eq!(const_eval_expr("true"), Some(1));
        assert_eq!(const_eval_expr("false"), Some(0));
    }
}
