//! Generic function monomorphization helpers.
//!
//! Provides type inference, generic call collection, type substitution,
//! and function specialization for Fajar Lang's monomorphization strategy.

use std::collections::{HashMap, HashSet};

use crate::parser::ast::{AsmOperand, Expr, FnDef, LiteralKind, Stmt, TypeExpr};

/// Infers the type suffix of an expression from AST alone (no CodegenCtx needed).
///
/// Used during pre-scan to determine which type specializations are needed.
/// Returns "f64" for float expressions, "i64" for everything else.
pub(crate) fn infer_prescan_type(expr: &Expr, param_types: &HashMap<String, String>) -> String {
    match expr {
        Expr::Literal {
            kind: LiteralKind::Float(_),
            ..
        } => "f64".to_string(),
        Expr::Literal {
            kind: LiteralKind::String(_),
            ..
        } => "str".to_string(),
        Expr::Literal { .. } => "i64".to_string(),
        Expr::Ident { name, .. } => param_types
            .get(name)
            .cloned()
            .unwrap_or_else(|| "i64".to_string()),
        Expr::Unary { operand, .. } => infer_prescan_type(operand, param_types),
        Expr::Grouped { expr: inner, .. } => infer_prescan_type(inner, param_types),
        Expr::Binary { left, .. } => infer_prescan_type(left, param_types),
        Expr::MethodCall { receiver, .. } => infer_prescan_type(receiver, param_types),
        _ => "i64".to_string(),
    }
}

/// Collects calls to generic functions and determines type specializations.
///
/// Produces `mono_specs: HashSet<(fn_name, type_suffix)>` and populates
/// `mono_map` for backward-compatible call resolution (maps fn_name → default mangled).
///
/// Supports multi-type-param generics: `fn foo<T, U>(a: T, b: U)` produces
/// composite suffix like `"i64_f64"` by inferring each generic param's type
/// from the corresponding argument.
pub(crate) fn collect_generic_calls(
    expr: &Expr,
    generic_fns: &HashMap<String, FnDef>,
    mono_map: &mut HashMap<String, String>,
    mono_specs: &mut HashSet<(String, String)>,
    param_types: &HashMap<String, String>,
) {
    match expr {
        Expr::Call { callee, args, .. } => {
            if let Expr::Ident { name, .. } = callee.as_ref() {
                if let Some(generic_def) = generic_fns.get(name) {
                    let type_suffix = infer_composite_type_suffix(generic_def, args, param_types);
                    let mangled = format!("{name}__mono_{type_suffix}");
                    mono_specs.insert((name.clone(), type_suffix));
                    // Keep mono_map for backward compat (first seen specialization wins)
                    mono_map.entry(name.clone()).or_insert(mangled);
                }
            }
            // Also walk callee and args
            collect_generic_calls(callee, generic_fns, mono_map, mono_specs, param_types);
            for arg in args {
                collect_generic_calls(&arg.value, generic_fns, mono_map, mono_specs, param_types);
            }
        }
        Expr::Block { stmts, expr, .. } => {
            for stmt in stmts {
                collect_generic_calls_in_stmt(stmt, generic_fns, mono_map, mono_specs, param_types);
            }
            if let Some(tail) = expr {
                collect_generic_calls(tail, generic_fns, mono_map, mono_specs, param_types);
            }
        }
        Expr::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            collect_generic_calls(condition, generic_fns, mono_map, mono_specs, param_types);
            collect_generic_calls(then_branch, generic_fns, mono_map, mono_specs, param_types);
            if let Some(eb) = else_branch {
                collect_generic_calls(eb, generic_fns, mono_map, mono_specs, param_types);
            }
        }
        Expr::Binary { left, right, .. } => {
            collect_generic_calls(left, generic_fns, mono_map, mono_specs, param_types);
            collect_generic_calls(right, generic_fns, mono_map, mono_specs, param_types);
        }
        Expr::Unary { operand, .. } => {
            collect_generic_calls(operand, generic_fns, mono_map, mono_specs, param_types);
        }
        Expr::While {
            condition, body, ..
        } => {
            collect_generic_calls(condition, generic_fns, mono_map, mono_specs, param_types);
            collect_generic_calls(body, generic_fns, mono_map, mono_specs, param_types);
        }
        Expr::For { body, .. } | Expr::Loop { body, .. } => {
            collect_generic_calls(body, generic_fns, mono_map, mono_specs, param_types);
        }
        Expr::Assign { value, .. } => {
            collect_generic_calls(value, generic_fns, mono_map, mono_specs, param_types);
        }
        Expr::Index { object, index, .. } => {
            collect_generic_calls(object, generic_fns, mono_map, mono_specs, param_types);
            collect_generic_calls(index, generic_fns, mono_map, mono_specs, param_types);
        }
        Expr::Array { elements, .. } => {
            for e in elements {
                collect_generic_calls(e, generic_fns, mono_map, mono_specs, param_types);
            }
        }
        // Leaf expressions — no sub-expressions to walk
        _ => {}
    }
}

/// Walks a statement to find calls to generic functions.
pub(crate) fn collect_generic_calls_in_stmt(
    stmt: &Stmt,
    generic_fns: &HashMap<String, FnDef>,
    mono_map: &mut HashMap<String, String>,
    mono_specs: &mut HashSet<(String, String)>,
    param_types: &HashMap<String, String>,
) {
    match stmt {
        Stmt::Let { value, .. } | Stmt::Const { value, .. } => {
            collect_generic_calls(value, generic_fns, mono_map, mono_specs, param_types);
        }
        Stmt::Expr { expr, .. } => {
            collect_generic_calls(expr, generic_fns, mono_map, mono_specs, param_types);
        }
        Stmt::Return { value: Some(v), .. } => {
            collect_generic_calls(v, generic_fns, mono_map, mono_specs, param_types);
        }
        _ => {}
    }
}

/// Substitutes type parameter names with concrete types in a TypeExpr.
///
/// For example, with `subst = {"T" → "f64"}`, transforms
/// `TypeExpr::Simple { name: "T" }` → `TypeExpr::Simple { name: "f64" }`.
pub(crate) fn substitute_type(ty: &TypeExpr, subst: &HashMap<String, String>) -> TypeExpr {
    match ty {
        TypeExpr::Simple { name, span } => {
            if let Some(concrete) = subst.get(name) {
                TypeExpr::Simple {
                    name: concrete.clone(),
                    span: *span,
                }
            } else {
                ty.clone()
            }
        }
        _ => ty.clone(),
    }
}

/// Infers a composite type suffix for a generic function call.
///
/// For single-param generics (`fn add<T>(a: T, b: T)`), returns `"i64"` or `"f64"`.
/// For multi-param generics (`fn pair<T, U>(a: T, b: U)`), returns `"i64_f64"` etc.
///
/// Each generic param's type is inferred from the first function parameter
/// that uses that generic type.
fn infer_composite_type_suffix(
    generic_def: &FnDef,
    call_args: &[crate::parser::ast::CallArg],
    param_types: &HashMap<String, String>,
) -> String {
    let mut type_parts = Vec::new();
    for gp in &generic_def.generic_params {
        let mut found_type = "i64".to_string();
        // Find the first function parameter whose type matches this generic param
        for (i, param) in generic_def.params.iter().enumerate() {
            if let TypeExpr::Simple { name: ptype, .. } = &param.ty {
                if ptype == &gp.name {
                    // Infer type from the corresponding call argument
                    if let Some(arg) = call_args.get(i) {
                        found_type = infer_prescan_type(&arg.value, param_types);
                    }
                    break;
                }
            }
        }
        type_parts.push(found_type);
    }
    type_parts.join("_")
}

/// Creates a specialized (monomorphized) FnDef by substituting type parameters
/// with concrete types throughout the function signature.
///
/// Supports composite type suffixes for multi-param generics:
/// `"i64_f64"` maps to `T → i64, U → f64` for `fn foo<T, U>(...)`.
pub(crate) fn specialize_fndef(
    generic_def: &FnDef,
    mangled_name: &str,
    type_suffix: &str,
) -> FnDef {
    // Build substitution map: each generic param → its concrete type
    // For composite suffixes like "i64_f64", split into per-param types
    let type_parts: Vec<&str> = type_suffix.split('_').collect();
    let mut subst = HashMap::new();

    if type_parts.len() >= generic_def.generic_params.len() {
        // Multi-param: assign each generic param its own type from the suffix parts
        // Handle compound types like "i64" (1 part) and "i64_f64" (2 parts for 2 params)
        // We need to reconstruct compound type names: "i64" from ["i64"], "f64" from ["f64"]
        let mut part_idx = 0;
        for gp in &generic_def.generic_params {
            // Each type name is a single part (i64, f64, etc.)
            if part_idx < type_parts.len() {
                subst.insert(gp.name.clone(), type_parts[part_idx].to_string());
                part_idx += 1;
            } else {
                subst.insert(gp.name.clone(), "i64".to_string());
            }
        }
    } else {
        // Fallback: single type for all params (backward compat)
        for gp in &generic_def.generic_params {
            subst.insert(gp.name.clone(), type_suffix.to_string());
        }
    }

    let mut specialized = generic_def.clone();
    specialized.name = mangled_name.to_string();
    specialized.generic_params.clear();

    // Substitute types in parameters
    for param in &mut specialized.params {
        param.ty = substitute_type(&param.ty, &subst);
    }

    // Substitute return type
    if let Some(ref ret_ty) = specialized.return_type {
        specialized.return_type = Some(substitute_type(ret_ty, &subst));
    }

    specialized
}

// ═══════════════════════════════════════════════════════════════════════
// Dead Function Elimination — Reachability Analysis (S43.1)
// ═══════════════════════════════════════════════════════════════════════

/// Collects all function names referenced from an expression.
///
/// Collects direct calls, method calls, pipeline targets, and any identifier
/// that could be a function reference (function pointers, callbacks).
/// Over-approximation is safe — `compute_reachable` filters to known functions.
pub(crate) fn collect_called_fns(expr: &Expr, called: &mut HashSet<String>) {
    match expr {
        Expr::Call { callee, args, .. } => {
            match callee.as_ref() {
                Expr::Ident { name, .. } => {
                    called.insert(name.clone());
                }
                Expr::Field { field, .. } => {
                    called.insert(field.clone());
                }
                _ => {}
            }
            collect_called_fns(callee, called);
            for arg in args {
                collect_called_fns(&arg.value, called);
            }
        }
        Expr::MethodCall {
            receiver,
            method,
            args,
            ..
        } => {
            called.insert(method.clone());
            collect_called_fns(receiver, called);
            for arg in args {
                collect_called_fns(&arg.value, called);
            }
        }
        // Identifiers may be function references (fn pointers, callbacks)
        Expr::Ident { name, .. } => {
            called.insert(name.clone());
        }
        // Pipeline: `x |> f` means f(x) — f is in right position
        Expr::Pipe { left, right, .. } => {
            collect_called_fns(left, called);
            collect_called_fns(right, called);
        }
        // Module-qualified paths: `mod::func`
        Expr::Path { segments, .. } => {
            if segments.len() >= 2 {
                // Mangle as mod_fn for module function resolution
                let mangled = format!("{}_{}", segments[0], segments[1]);
                called.insert(mangled);
            }
            for seg in segments {
                called.insert(seg.clone());
            }
        }
        Expr::Block { stmts, expr, .. } => {
            for stmt in stmts {
                collect_called_fns_in_stmt(stmt, called);
            }
            if let Some(tail) = expr {
                collect_called_fns(tail, called);
            }
        }
        Expr::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            collect_called_fns(condition, called);
            collect_called_fns(then_branch, called);
            if let Some(eb) = else_branch {
                collect_called_fns(eb, called);
            }
        }
        Expr::Binary { left, right, .. } => {
            collect_called_fns(left, called);
            collect_called_fns(right, called);
        }
        Expr::Unary { operand, .. } => {
            collect_called_fns(operand, called);
        }
        Expr::While {
            condition, body, ..
        } => {
            collect_called_fns(condition, called);
            collect_called_fns(body, called);
        }
        Expr::For { body, iterable, .. } => {
            collect_called_fns(iterable, called);
            collect_called_fns(body, called);
        }
        Expr::Loop { body, .. } => {
            collect_called_fns(body, called);
        }
        Expr::Assign { value, .. } => {
            collect_called_fns(value, called);
        }
        Expr::Index { object, index, .. } => {
            collect_called_fns(object, called);
            collect_called_fns(index, called);
        }
        Expr::Array { elements, .. } | Expr::Tuple { elements, .. } => {
            for e in elements {
                collect_called_fns(e, called);
            }
        }
        Expr::Match { subject, arms, .. } => {
            collect_called_fns(subject, called);
            for arm in arms {
                collect_called_fns(&arm.body, called);
            }
        }
        Expr::StructInit { name, fields, .. } => {
            called.insert(name.clone());
            for fi in fields {
                collect_called_fns(&fi.value, called);
            }
        }
        Expr::Field { object, .. } => {
            collect_called_fns(object, called);
        }
        Expr::Closure { body, .. } => {
            collect_called_fns(body, called);
        }
        Expr::Await { expr, .. } => {
            collect_called_fns(expr, called);
        }
        Expr::Grouped { expr: inner, .. } => {
            collect_called_fns(inner, called);
        }
        Expr::Cast { expr: inner, .. } => {
            collect_called_fns(inner, called);
        }
        Expr::Range { start, end, .. } => {
            if let Some(s) = start {
                collect_called_fns(s, called);
            }
            if let Some(e) = end {
                collect_called_fns(e, called);
            }
        }
        Expr::Try { expr: inner, .. } => {
            collect_called_fns(inner, called);
        }
        Expr::AsyncBlock { body, .. } => {
            collect_called_fns(body, called);
        }
        Expr::InlineAsm { operands, .. } => {
            for op in operands {
                match op {
                    AsmOperand::Sym { name } => {
                        called.insert(name.clone());
                    }
                    AsmOperand::In { expr, .. }
                    | AsmOperand::Out { expr, .. }
                    | AsmOperand::InOut { expr, .. }
                    | AsmOperand::Const { expr } => {
                        collect_called_fns(expr, called);
                    }
                }
            }
        }
        // Leaf expressions (literals, break, continue) — no sub-expressions
        _ => {}
    }
}

/// Collects called function names from a statement.
pub(crate) fn collect_called_fns_in_stmt(stmt: &Stmt, called: &mut HashSet<String>) {
    match stmt {
        Stmt::Let { value, .. } | Stmt::Const { value, .. } => {
            collect_called_fns(value, called);
        }
        Stmt::Expr { expr, .. } => {
            collect_called_fns(expr, called);
        }
        Stmt::Return { value: Some(v), .. } => {
            collect_called_fns(v, called);
        }
        Stmt::Item(item) => {
            if let crate::parser::ast::Item::FnDef(fndef) = item.as_ref() {
                // Nested function: treat as reachable (it's defined inline)
                collect_called_fns(&fndef.body, called);
            }
        }
        _ => {}
    }
}

/// Computes the transitive closure of reachable functions starting from entry points.
///
/// Returns a set of function names that are reachable from `main`, `@entry`,
/// or `@panic_handler` annotated functions.
pub(crate) fn compute_reachable(
    entry_points: &[String],
    fn_bodies: &HashMap<String, &Expr>,
) -> HashSet<String> {
    let mut reachable: HashSet<String> = entry_points.iter().cloned().collect();
    let mut worklist: Vec<String> = entry_points.to_vec();

    while let Some(fn_name) = worklist.pop() {
        if let Some(body) = fn_bodies.get(&fn_name) {
            let mut called = HashSet::new();
            collect_called_fns(body, &mut called);
            for callee in called {
                if !reachable.contains(&callee) && fn_bodies.contains_key(&callee) {
                    reachable.insert(callee.clone());
                    worklist.push(callee);
                }
            }
        }
    }

    reachable
}
