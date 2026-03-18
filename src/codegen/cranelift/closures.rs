//! Closure compilation helpers.
//!
//! Provides free variable collection and closure pre-scanning for lambda
//! expressions in Fajar Lang. Closures are desugared into regular functions
//! with captured variables as extra parameters.

use std::collections::HashSet;
use std::sync::atomic::Ordering;

use crate::parser::ast::{Expr, FnDef, Stmt};

use super::CLOSURE_COUNTER;

/// Info about a closure found during pre-scanning.
pub(crate) struct ClosureInfo {
    /// The variable name the closure is assigned to (e.g., `f` in `let f = |x| ...`).
    pub var_name: String,
    /// Generated unique function name for this closure.
    pub fn_name: String,
    /// The synthetic FnDef for compilation.
    pub fndef: FnDef,
    /// Names of captured variables (in order), prepended as extra params.
    pub captures: Vec<String>,
    /// Source span of the original `Expr::Closure` (for inline closure lookup).
    pub span: crate::lexer::token::Span,
}

/// Collects free variables in an expression that are not in the `bound` set.
///
/// Used by closure compilation to determine which variables from the enclosing
/// scope need to be passed as extra arguments.
pub(crate) fn collect_free_vars(expr: &Expr, bound: &HashSet<String>) -> HashSet<String> {
    let mut free = HashSet::new();
    collect_free_vars_inner(expr, bound, &mut free);
    free
}

/// Recursive helper for free variable collection.
fn collect_free_vars_inner(expr: &Expr, bound: &HashSet<String>, free: &mut HashSet<String>) {
    match expr {
        Expr::Ident { name, .. } => {
            if !bound.contains(name) {
                free.insert(name.clone());
            }
        }
        Expr::Binary { left, right, .. } => {
            collect_free_vars_inner(left, bound, free);
            collect_free_vars_inner(right, bound, free);
        }
        Expr::Unary { operand, .. } => {
            collect_free_vars_inner(operand, bound, free);
        }
        Expr::Call { callee, args, .. } => {
            collect_free_vars_inner(callee, bound, free);
            for arg in args {
                collect_free_vars_inner(&arg.value, bound, free);
            }
        }
        Expr::MethodCall { receiver, args, .. } => {
            collect_free_vars_inner(receiver, bound, free);
            for arg in args {
                collect_free_vars_inner(&arg.value, bound, free);
            }
        }
        Expr::Block { stmts, expr, .. } => {
            // Block may introduce new bindings
            let mut inner_bound = bound.clone();
            for stmt in stmts {
                match stmt {
                    Stmt::Let { name, value, .. } | Stmt::Const { name, value, .. } => {
                        collect_free_vars_inner(value, &inner_bound, free);
                        inner_bound.insert(name.clone());
                    }
                    Stmt::Expr { expr, .. } => {
                        collect_free_vars_inner(expr, &inner_bound, free);
                    }
                    Stmt::Return { value: Some(v), .. } | Stmt::Break { value: Some(v), .. } => {
                        collect_free_vars_inner(v, &inner_bound, free);
                    }
                    _ => {}
                }
            }
            if let Some(tail) = expr {
                collect_free_vars_inner(tail, &inner_bound, free);
            }
        }
        Expr::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            collect_free_vars_inner(condition, bound, free);
            collect_free_vars_inner(then_branch, bound, free);
            if let Some(eb) = else_branch {
                collect_free_vars_inner(eb, bound, free);
            }
        }
        Expr::While {
            label: _,
            condition,
            body,
            ..
        } => {
            collect_free_vars_inner(condition, bound, free);
            collect_free_vars_inner(body, bound, free);
        }
        Expr::For {
            label: _,
            variable,
            iterable,
            body,
            ..
        } => {
            collect_free_vars_inner(iterable, bound, free);
            let mut inner_bound = bound.clone();
            inner_bound.insert(variable.clone());
            collect_free_vars_inner(body, &inner_bound, free);
        }
        Expr::Loop { label: _, body, .. } => {
            collect_free_vars_inner(body, bound, free);
        }
        Expr::Array { elements, .. } => {
            for elem in elements {
                collect_free_vars_inner(elem, bound, free);
            }
        }
        Expr::Index { object, index, .. } => {
            collect_free_vars_inner(object, bound, free);
            collect_free_vars_inner(index, bound, free);
        }
        Expr::Assign { target, value, .. } => {
            collect_free_vars_inner(target, bound, free);
            collect_free_vars_inner(value, bound, free);
        }
        Expr::Grouped { expr, .. } => {
            collect_free_vars_inner(expr, bound, free);
        }
        Expr::Tuple { elements, .. } => {
            for elem in elements {
                collect_free_vars_inner(elem, bound, free);
            }
        }
        Expr::Match { subject, arms, .. } => {
            collect_free_vars_inner(subject, bound, free);
            for arm in arms {
                collect_free_vars_inner(&arm.body, bound, free);
            }
        }
        Expr::Field { object, .. } => {
            collect_free_vars_inner(object, bound, free);
        }
        Expr::StructInit { fields, .. } => {
            for f in fields {
                collect_free_vars_inner(&f.value, bound, free);
            }
        }
        Expr::Cast { expr, .. } => {
            collect_free_vars_inner(expr, bound, free);
        }
        Expr::Pipe { left, right, .. } => {
            collect_free_vars_inner(left, bound, free);
            collect_free_vars_inner(right, bound, free);
        }
        Expr::Closure { params, body, .. } => {
            // Nested closure: params are bound within the body
            let mut inner_bound = bound.clone();
            for p in params {
                inner_bound.insert(p.name.clone());
            }
            collect_free_vars_inner(body, &inner_bound, free);
        }
        // Literals and other expressions that don't reference variables
        _ => {}
    }
}

/// Scans a function body for `let x = |params| body` patterns and collects closure info.
///
/// `known_names` contains function names, builtin names, enum variants, etc.
/// that should NOT be treated as captured variables.
pub(crate) fn scan_closures_in_body(
    body: &Expr,
    known_names: &HashSet<String>,
) -> Vec<ClosureInfo> {
    let mut closures = Vec::new();
    scan_closures_recursive(body, known_names, &mut closures);
    closures
}

/// Builds a `ClosureInfo` from a closure expression with a given variable name.
fn build_closure_info(
    var_name: &str,
    params: &[crate::parser::ast::ClosureParam],
    return_type: &Option<Box<crate::parser::ast::TypeExpr>>,
    body: &Expr,
    span: &crate::lexer::token::Span,
    known_names: &HashSet<String>,
) -> ClosureInfo {
    let closure_id = CLOSURE_COUNTER.fetch_add(1, Ordering::SeqCst);
    let fn_name = format!("__closure_{closure_id}");

    // Determine bound variables (closure params)
    let mut bound: HashSet<String> = known_names.clone();
    for p in params {
        bound.insert(p.name.clone());
    }

    // Collect free variables
    let free_vars = collect_free_vars(body, &bound);
    let mut captures: Vec<String> = free_vars.into_iter().collect();
    captures.sort(); // deterministic order

    // Build params: captured vars + closure params
    let dummy_span = crate::lexer::token::Span::new(0, 0);
    let mut all_params: Vec<crate::parser::ast::Param> = captures
        .iter()
        .map(|cap_name| crate::parser::ast::Param {
            name: cap_name.clone(),
            ty: crate::parser::ast::TypeExpr::Simple {
                name: "i64".to_string(),
                span: dummy_span,
            },
            span: dummy_span,
        })
        .collect();
    for cp in params {
        all_params.push(crate::parser::ast::Param {
            name: cp.name.clone(),
            ty: cp
                .ty
                .clone()
                .unwrap_or(crate::parser::ast::TypeExpr::Simple {
                    name: "i64".to_string(),
                    span: dummy_span,
                }),
            span: cp.span,
        });
    }

    let fndef = FnDef {
        is_pub: false,
        is_async: false,
        is_test: false,
        should_panic: false,
        is_ignored: false,
        doc_comment: None,
        annotation: None,
        name: fn_name.clone(),
        lifetime_params: Vec::new(),
        generic_params: Vec::new(),
        params: all_params,
        return_type: return_type.as_deref().cloned(),
        where_clauses: Vec::new(),
        body: Box::new(body.clone()),
        span: *span,
    };

    ClosureInfo {
        var_name: var_name.to_string(),
        fn_name,
        fndef,
        captures,
        span: *span,
    }
}

/// Recursively scans statements for closure-bearing Let bindings and inline closures in call args.
fn scan_closures_recursive(
    expr: &Expr,
    known_names: &HashSet<String>,
    closures: &mut Vec<ClosureInfo>,
) {
    match expr {
        Expr::Block { stmts, expr, .. } => {
            for stmt in stmts {
                match stmt {
                    Stmt::Let { name, value, .. } | Stmt::Const { name, value, .. } => {
                        if let Expr::Closure {
                            params,
                            return_type,
                            body,
                            span,
                        } = value.as_ref()
                        {
                            closures.push(build_closure_info(
                                name,
                                params,
                                return_type,
                                body,
                                span,
                                known_names,
                            ));
                        }
                        // Also scan the value for nested closures
                        scan_closures_recursive(value, known_names, closures);
                    }
                    Stmt::Expr { expr, .. } => {
                        scan_closures_recursive(expr, known_names, closures);
                    }
                    _ => {}
                }
            }
            if let Some(tail) = expr {
                scan_closures_recursive(tail, known_names, closures);
            }
        }
        Expr::Call { args, callee, .. } => {
            // Scan callee for closures
            scan_closures_recursive(callee, known_names, closures);
            // Scan call arguments for inline closures like `apply(|x| x + 1)`
            for (i, arg) in args.iter().enumerate() {
                if let Expr::Closure {
                    params,
                    return_type,
                    body,
                    span,
                } = &arg.value
                {
                    let anon_name = format!("__anon_arg_{}", closures.len());
                    closures.push(build_closure_info(
                        &anon_name,
                        params,
                        return_type,
                        body,
                        span,
                        known_names,
                    ));
                    // Ignore i lint
                    let _ = i;
                } else {
                    scan_closures_recursive(&arg.value, known_names, closures);
                }
            }
        }
        Expr::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            scan_closures_recursive(condition, known_names, closures);
            scan_closures_recursive(then_branch, known_names, closures);
            if let Some(eb) = else_branch {
                scan_closures_recursive(eb, known_names, closures);
            }
        }
        Expr::While {
            label: _,
            condition,
            body,
            ..
        } => {
            scan_closures_recursive(condition, known_names, closures);
            scan_closures_recursive(body, known_names, closures);
        }
        Expr::Loop { label: _, body, .. } => {
            scan_closures_recursive(body, known_names, closures);
        }
        Expr::For { label: _, body, .. } => {
            scan_closures_recursive(body, known_names, closures);
        }
        Expr::Assign { value, .. } => {
            scan_closures_recursive(value, known_names, closures);
        }
        Expr::Binary { left, right, .. } => {
            scan_closures_recursive(left, known_names, closures);
            scan_closures_recursive(right, known_names, closures);
        }
        Expr::Unary { operand, .. } => {
            scan_closures_recursive(operand, known_names, closures);
        }
        Expr::Grouped { expr, .. } => {
            scan_closures_recursive(expr, known_names, closures);
        }
        Expr::Match { subject, arms, .. } => {
            scan_closures_recursive(subject, known_names, closures);
            for arm in arms {
                scan_closures_recursive(&arm.body, known_names, closures);
            }
        }
        _ => {}
    }
}
