//! Static analysis for compiled code — stack usage estimation and memory layout.
//!
//! Analyzes AST functions to estimate stack frame sizes, detect deep recursion,
//! and generate static memory map reports for embedded targets.

use std::collections::{HashMap, HashSet};

use crate::parser::ast::{Expr, FnDef, Item, Program, Stmt, TypeExpr};

/// Estimated size of each type in bytes on a 64-bit target.
const PTR_SIZE: usize = 8;

/// Maximum recommended stack depth before warning.
const MAX_SAFE_DEPTH: usize = 64;

/// Maximum recommended total stack usage before warning (128KB).
const MAX_SAFE_STACK: usize = 128 * 1024;

/// Stack usage estimate for a single function.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FnStackInfo {
    /// Function name.
    pub name: String,
    /// Estimated stack frame size in bytes (locals + temporaries).
    pub frame_bytes: usize,
    /// Number of local variables.
    pub local_count: usize,
    /// Functions called directly by this function.
    pub calls: Vec<String>,
    /// Whether this function is recursive (calls itself directly or indirectly).
    pub is_recursive: bool,
}

/// A warning about stack usage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StackWarning {
    /// A single function has a large stack frame.
    LargeFrame {
        /// Function name.
        function: String,
        /// Frame size in bytes.
        bytes: usize,
    },
    /// Deep call chain detected (exceeds MAX_SAFE_DEPTH).
    DeepCallChain {
        /// The call chain (function names).
        chain: Vec<String>,
        /// Depth of the chain.
        depth: usize,
    },
    /// Direct or indirect recursion detected.
    Recursion {
        /// Function that recurses.
        function: String,
        /// The cycle (function names).
        cycle: Vec<String>,
    },
    /// Total stack usage in a call chain exceeds threshold.
    TotalStackExceeded {
        /// The call chain consuming the most stack.
        chain: Vec<String>,
        /// Total estimated bytes.
        total_bytes: usize,
    },
}

/// Static memory map section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemorySection {
    /// Section name (e.g., ".text", ".data", ".bss", ".stack").
    pub name: String,
    /// Estimated size in bytes.
    pub size: usize,
}

/// Complete analysis report for a program.
#[derive(Debug, Clone)]
pub struct AnalysisReport {
    /// Per-function stack information.
    pub functions: Vec<FnStackInfo>,
    /// Warnings about potential issues.
    pub warnings: Vec<StackWarning>,
    /// Static memory sections estimate.
    pub memory_map: Vec<MemorySection>,
}

/// Estimates the byte size of a type expression.
fn estimate_type_size(ty: &TypeExpr) -> usize {
    match ty {
        TypeExpr::Simple { name, .. } => match name.as_str() {
            "bool" => 1,
            "i8" | "u8" => 1,
            "i16" | "u16" => 2,
            "i32" | "u32" | "f32" => 4,
            "i64" | "u64" | "f64" | "isize" | "usize" => 8,
            "i128" | "u128" => 16,
            "char" => 4,
            "void" | "never" => 0,
            "str" | "String" => PTR_SIZE * 2, // ptr + len (no cap in codegen)
            _ => PTR_SIZE,                    // Unknown struct/enum — assume pointer-sized
        },
        TypeExpr::Array { element, size, .. } => {
            let elem_size = estimate_type_size(element);
            (*size as usize) * elem_size
        }
        TypeExpr::Tuple { elements, .. } => elements.iter().map(estimate_type_size).sum(),
        TypeExpr::Pointer { .. } | TypeExpr::Reference { .. } => PTR_SIZE,
        TypeExpr::Fn { .. } => PTR_SIZE * 2, // fn ptr + env ptr
        TypeExpr::Generic { .. }
        | TypeExpr::Tensor { .. }
        | TypeExpr::Slice { .. }
        | TypeExpr::Path { .. } => PTR_SIZE,
        TypeExpr::DynTrait { .. } => PTR_SIZE * 2, // fat pointer: data_ptr + vtable_ptr
    }
}

/// Default size for a variable without type annotation (i64/f64/pointer).
const DEFAULT_VAR_SIZE: usize = 8;

/// Counts local variables and estimates stack usage for a function body.
fn analyze_fn_body(body: &Expr) -> (usize, usize, Vec<String>) {
    let mut local_count = 0usize;
    let mut frame_bytes = 0usize;
    let mut calls = Vec::new();

    analyze_expr(body, &mut local_count, &mut frame_bytes, &mut calls);

    (local_count, frame_bytes, calls)
}

/// Recursively walks an expression to count locals and calls.
fn analyze_expr(
    expr: &Expr,
    local_count: &mut usize,
    frame_bytes: &mut usize,
    calls: &mut Vec<String>,
) {
    match expr {
        Expr::Block {
            stmts, expr: tail, ..
        } => {
            for stmt in stmts {
                analyze_stmt(stmt, local_count, frame_bytes, calls);
            }
            if let Some(e) = tail {
                analyze_expr(e, local_count, frame_bytes, calls);
            }
        }
        Expr::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            analyze_expr(condition, local_count, frame_bytes, calls);
            analyze_expr(then_branch, local_count, frame_bytes, calls);
            if let Some(eb) = else_branch {
                analyze_expr(eb, local_count, frame_bytes, calls);
            }
        }
        Expr::While {
            label: _,
            condition,
            body,
            ..
        } => {
            analyze_expr(condition, local_count, frame_bytes, calls);
            analyze_expr(body, local_count, frame_bytes, calls);
        }
        Expr::For {
            label: _,
            body,
            iterable,
            ..
        } => {
            *local_count += 1; // loop variable
            *frame_bytes += DEFAULT_VAR_SIZE;
            analyze_expr(iterable, local_count, frame_bytes, calls);
            analyze_expr(body, local_count, frame_bytes, calls);
        }
        Expr::Loop { label: _, body, .. } => {
            analyze_expr(body, local_count, frame_bytes, calls);
        }
        Expr::Call { callee, args, .. } => {
            if let Expr::Ident { name, .. } = callee.as_ref() {
                if !calls.contains(name) {
                    calls.push(name.clone());
                }
            }
            for arg in args {
                analyze_expr(&arg.value, local_count, frame_bytes, calls);
            }
        }
        Expr::MethodCall { receiver, args, .. } => {
            analyze_expr(receiver, local_count, frame_bytes, calls);
            for arg in args {
                analyze_expr(&arg.value, local_count, frame_bytes, calls);
            }
        }
        Expr::Binary { left, right, .. } => {
            analyze_expr(left, local_count, frame_bytes, calls);
            analyze_expr(right, local_count, frame_bytes, calls);
        }
        Expr::Unary { operand, .. } => {
            analyze_expr(operand, local_count, frame_bytes, calls);
        }
        Expr::Assign { target, value, .. } => {
            analyze_expr(target, local_count, frame_bytes, calls);
            analyze_expr(value, local_count, frame_bytes, calls);
        }
        Expr::Array { elements, .. } | Expr::Tuple { elements, .. } => {
            for e in elements {
                analyze_expr(e, local_count, frame_bytes, calls);
            }
        }
        Expr::ArrayRepeat { value, count, .. } => {
            analyze_expr(value, local_count, frame_bytes, calls);
            analyze_expr(count, local_count, frame_bytes, calls);
        }
        Expr::Match { subject, arms, .. } => {
            analyze_expr(subject, local_count, frame_bytes, calls);
            for arm in arms {
                analyze_expr(&arm.body, local_count, frame_bytes, calls);
            }
        }
        Expr::Index { object, index, .. } => {
            analyze_expr(object, local_count, frame_bytes, calls);
            analyze_expr(index, local_count, frame_bytes, calls);
        }
        Expr::Field { object, .. } => {
            analyze_expr(object, local_count, frame_bytes, calls);
        }
        Expr::Closure { body, params, .. } => {
            *local_count += params.len();
            *frame_bytes += params.len() * DEFAULT_VAR_SIZE;
            analyze_expr(body, local_count, frame_bytes, calls);
        }
        Expr::Pipe { left, right, .. } => {
            analyze_expr(left, local_count, frame_bytes, calls);
            analyze_expr(right, local_count, frame_bytes, calls);
        }
        Expr::Cast { expr, .. }
        | Expr::Try { expr, .. }
        | Expr::Grouped { expr, .. }
        | Expr::Await { expr, .. } => {
            analyze_expr(expr, local_count, frame_bytes, calls);
        }
        Expr::AsyncBlock { body, .. } => {
            analyze_expr(body, local_count, frame_bytes, calls);
        }
        Expr::StructInit { fields, .. } => {
            for f in fields {
                analyze_expr(&f.value, local_count, frame_bytes, calls);
            }
        }
        Expr::HandleEffect { body, handlers, .. } => {
            analyze_expr(body, local_count, frame_bytes, calls);
            for handler in handlers {
                analyze_expr(&handler.body, local_count, frame_bytes, calls);
            }
        }
        Expr::ResumeExpr { value, .. } => {
            analyze_expr(value, local_count, frame_bytes, calls);
        }
        Expr::Comptime { body, .. } => {
            analyze_expr(body, local_count, frame_bytes, calls);
        }
        // Leaf nodes — no locals, no calls
        Expr::Literal { .. }
        | Expr::Ident { .. }
        | Expr::Range { .. }
        | Expr::Path { .. }
        | Expr::InlineAsm { .. }
        | Expr::FString { .. }
        | Expr::MacroInvocation { .. } => {}
    }
}

/// Recursively walks a statement to count locals and calls.
fn analyze_stmt(
    stmt: &Stmt,
    local_count: &mut usize,
    frame_bytes: &mut usize,
    calls: &mut Vec<String>,
) {
    match stmt {
        Stmt::Let { ty, value, .. } => {
            *local_count += 1;
            *frame_bytes += ty
                .as_ref()
                .map(estimate_type_size)
                .unwrap_or(DEFAULT_VAR_SIZE);
            analyze_expr(value, local_count, frame_bytes, calls);
        }
        Stmt::Const { ty, value, .. } => {
            *local_count += 1;
            *frame_bytes += estimate_type_size(ty);
            analyze_expr(value, local_count, frame_bytes, calls);
        }
        Stmt::Expr { expr, .. } => {
            analyze_expr(expr, local_count, frame_bytes, calls);
        }
        Stmt::Return { value, .. } => {
            if let Some(v) = value {
                analyze_expr(v, local_count, frame_bytes, calls);
            }
        }
        Stmt::Break { value, .. } => {
            if let Some(v) = value {
                analyze_expr(v, local_count, frame_bytes, calls);
            }
        }
        Stmt::Continue { .. } => {}
        Stmt::Item(item) => {
            if let Item::FnDef(f) = item.as_ref() {
                // Nested function — counts as a local (closure env pointer)
                *local_count += 1;
                *frame_bytes += PTR_SIZE * 2;
                if !calls.contains(&f.name) {
                    calls.push(f.name.clone());
                }
            }
        }
    }
}

/// Collects all function definitions from a program.
fn collect_functions(program: &Program) -> Vec<&FnDef> {
    let mut fns = Vec::new();
    for item in &program.items {
        if let Item::FnDef(f) = item {
            fns.push(f);
        }
        if let Item::ImplBlock(imp) = item {
            for method in &imp.methods {
                fns.push(method);
            }
        }
    }
    fns
}

/// Detects cycles in the call graph using DFS.
fn find_recursion(
    fn_name: &str,
    call_map: &HashMap<String, Vec<String>>,
    visited: &mut HashSet<String>,
    stack: &mut Vec<String>,
) -> Option<Vec<String>> {
    if stack.contains(&fn_name.to_string()) {
        let cycle_start = stack.iter().position(|n| n == fn_name).unwrap();
        let mut cycle: Vec<String> = stack[cycle_start..].to_vec();
        cycle.push(fn_name.to_string());
        return Some(cycle);
    }
    if visited.contains(fn_name) {
        return None;
    }

    visited.insert(fn_name.to_string());
    stack.push(fn_name.to_string());

    if let Some(callees) = call_map.get(fn_name) {
        for callee in callees {
            if let Some(cycle) = find_recursion(callee, call_map, visited, stack) {
                return Some(cycle);
            }
        }
    }

    stack.pop();
    None
}

/// Finds the deepest non-recursive call chain from a function.
fn deepest_chain(
    fn_name: &str,
    call_map: &HashMap<String, Vec<String>>,
    visited: &mut HashSet<String>,
) -> Vec<String> {
    if visited.contains(fn_name) {
        return vec![fn_name.to_string()];
    }
    visited.insert(fn_name.to_string());

    let mut best = vec![fn_name.to_string()];

    if let Some(callees) = call_map.get(fn_name) {
        for callee in callees {
            let mut chain = vec![fn_name.to_string()];
            chain.extend(deepest_chain(callee, call_map, visited));
            if chain.len() > best.len() {
                best = chain;
            }
        }
    }

    visited.remove(fn_name);
    best
}

/// Analyzes a program's stack usage and generates a report.
pub fn analyze_program(program: &Program) -> AnalysisReport {
    let fns = collect_functions(program);
    let mut fn_infos: Vec<FnStackInfo> = Vec::new();
    let mut call_map: HashMap<String, Vec<String>> = HashMap::new();
    let mut frame_map: HashMap<String, usize> = HashMap::new();

    // Phase 1: Analyze each function
    for f in &fns {
        let param_bytes: usize = f.params.iter().map(|p| estimate_type_size(&p.ty)).sum();

        let (local_count, body_bytes, calls) = analyze_fn_body(&f.body);
        let frame_bytes = param_bytes + body_bytes + PTR_SIZE; // +PTR_SIZE for return addr

        call_map.insert(f.name.clone(), calls.clone());
        frame_map.insert(f.name.clone(), frame_bytes);

        fn_infos.push(FnStackInfo {
            name: f.name.clone(),
            frame_bytes,
            local_count: local_count + f.params.len(),
            calls,
            is_recursive: false,
        });
    }

    // Phase 2: Detect recursion
    let mut all_visited = HashSet::new();
    let mut warnings = Vec::new();

    for info in &mut fn_infos {
        let mut visited = HashSet::new();
        let mut stack = Vec::new();
        if let Some(cycle) = find_recursion(&info.name, &call_map, &mut visited, &mut stack) {
            info.is_recursive = true;
            if !all_visited.contains(&info.name) {
                warnings.push(StackWarning::Recursion {
                    function: info.name.clone(),
                    cycle,
                });
                all_visited.insert(info.name.clone());
            }
        }
    }

    // Phase 3: Detect deep call chains & large frames
    for info in &fn_infos {
        if info.frame_bytes > 4096 {
            warnings.push(StackWarning::LargeFrame {
                function: info.name.clone(),
                bytes: info.frame_bytes,
            });
        }
    }

    for info in &fn_infos {
        if !info.is_recursive {
            let mut visited = HashSet::new();
            let chain = deepest_chain(&info.name, &call_map, &mut visited);
            if chain.len() > MAX_SAFE_DEPTH {
                warnings.push(StackWarning::DeepCallChain {
                    depth: chain.len(),
                    chain: chain.clone(),
                });
            }
            let total: usize = chain.iter().filter_map(|name| frame_map.get(name)).sum();
            if total > MAX_SAFE_STACK {
                warnings.push(StackWarning::TotalStackExceeded {
                    chain,
                    total_bytes: total,
                });
            }
        }
    }

    // Phase 4: Estimate memory sections
    let text_size: usize = fn_infos.iter().map(|f| f.frame_bytes * 4).sum();
    let data_size = program
        .items
        .iter()
        .filter(|i| matches!(i, Item::ConstDef(_)))
        .count()
        * 8;
    let bss_size = 0;
    let stack_size = fn_infos.iter().map(|f| f.frame_bytes).max().unwrap_or(0) * MAX_SAFE_DEPTH;

    let memory_map = vec![
        MemorySection {
            name: ".text".into(),
            size: text_size,
        },
        MemorySection {
            name: ".data".into(),
            size: data_size,
        },
        MemorySection {
            name: ".bss".into(),
            size: bss_size,
        },
        MemorySection {
            name: ".stack".into(),
            size: stack_size,
        },
    ];

    AnalysisReport {
        functions: fn_infos,
        warnings,
        memory_map,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::token::Span;
    use crate::parser::ast::*;

    fn dummy_span() -> Span {
        Span::new(0, 1)
    }

    fn int_lit(n: i64) -> Expr {
        Expr::Literal {
            kind: LiteralKind::Int(n),
            span: dummy_span(),
        }
    }

    fn ident(name: &str) -> Expr {
        Expr::Ident {
            name: name.into(),
            span: dummy_span(),
        }
    }

    fn call_expr(name: &str, args: Vec<Expr>) -> Expr {
        Expr::Call {
            callee: Box::new(ident(name)),
            args: args
                .into_iter()
                .map(|v| CallArg {
                    name: None,
                    value: v,
                    span: dummy_span(),
                })
                .collect(),
            span: dummy_span(),
        }
    }

    fn block(stmts: Vec<Stmt>, tail: Option<Expr>) -> Expr {
        Expr::Block {
            stmts,
            expr: tail.map(Box::new),
            span: dummy_span(),
        }
    }

    fn let_stmt(name: &str, ty: Option<TypeExpr>, value: Expr) -> Stmt {
        Stmt::Let {
            mutable: false,
            linear: false,
            name: name.into(),
            ty,
            value: Box::new(value),
            span: dummy_span(),
        }
    }

    fn make_fn(name: &str, params: Vec<Param>, body: Expr) -> FnDef {
        FnDef {
            is_pub: false,
            is_const: false,
            is_async: false,
            is_test: false,
            should_panic: false,
            is_ignored: false,
            doc_comment: None,
            annotation: None,
            name: name.into(),
            lifetime_params: vec![],
            generic_params: vec![],
            params,
            return_type: None,
            where_clauses: vec![],
            requires: vec![],
            ensures: vec![],
            effects: vec![],
            body: Box::new(body),
            span: dummy_span(),
        }
    }

    fn make_param(name: &str, ty: TypeExpr) -> Param {
        Param {
            name: name.into(),
            ty,
            span: dummy_span(),
        }
    }

    fn simple_type(name: &str) -> TypeExpr {
        TypeExpr::Simple {
            name: name.into(),
            span: dummy_span(),
        }
    }

    fn make_program(items: Vec<Item>) -> Program {
        Program {
            items,
            span: dummy_span(),
        }
    }

    // ── Type size estimation ──

    #[test]
    fn type_size_primitives() {
        assert_eq!(estimate_type_size(&simple_type("bool")), 1);
        assert_eq!(estimate_type_size(&simple_type("i8")), 1);
        assert_eq!(estimate_type_size(&simple_type("i32")), 4);
        assert_eq!(estimate_type_size(&simple_type("f64")), 8);
        assert_eq!(estimate_type_size(&simple_type("void")), 0);
    }

    #[test]
    fn type_size_string() {
        assert_eq!(estimate_type_size(&simple_type("String")), PTR_SIZE * 2);
    }

    #[test]
    fn type_size_array() {
        let arr = TypeExpr::Array {
            element: Box::new(simple_type("i32")),
            size: 10,
            span: dummy_span(),
        };
        assert_eq!(estimate_type_size(&arr), 40); // 10 * 4
    }

    #[test]
    fn type_size_tuple() {
        let tup = TypeExpr::Tuple {
            elements: vec![simple_type("i32"), simple_type("f64")],
            span: dummy_span(),
        };
        assert_eq!(estimate_type_size(&tup), 12); // 4 + 8
    }

    // ── Function analysis ──

    #[test]
    fn analyze_simple_function() {
        let f = make_fn(
            "add",
            vec![
                make_param("a", simple_type("i32")),
                make_param("b", simple_type("i32")),
            ],
            Expr::Binary {
                left: Box::new(ident("a")),
                op: BinOp::Add,
                right: Box::new(ident("b")),
                span: dummy_span(),
            },
        );
        let prog = make_program(vec![Item::FnDef(f)]);
        let report = analyze_program(&prog);

        assert_eq!(report.functions.len(), 1);
        let info = &report.functions[0];
        assert_eq!(info.name, "add");
        assert_eq!(info.local_count, 2); // 2 params
        assert_eq!(info.frame_bytes, 4 + 4 + PTR_SIZE); // 2 i32 params + return addr
        assert!(!info.is_recursive);
        assert!(info.calls.is_empty());
    }

    #[test]
    fn analyze_function_with_locals() {
        let f = make_fn(
            "compute",
            vec![make_param("x", simple_type("i64"))],
            block(
                vec![
                    let_stmt("a", Some(simple_type("i32")), int_lit(0)),
                    let_stmt("b", Some(simple_type("f64")), int_lit(1)),
                ],
                None,
            ),
        );
        let prog = make_program(vec![Item::FnDef(f)]);
        let report = analyze_program(&prog);

        let info = &report.functions[0];
        assert_eq!(info.local_count, 3); // 1 param + 2 locals
        // frame = 8 (i64 param) + 4 (i32 local) + 8 (f64 local) + 8 (ret addr)
        assert_eq!(info.frame_bytes, 8 + 4 + 8 + PTR_SIZE);
    }

    #[test]
    fn analyze_function_with_calls() {
        let f = make_fn(
            "caller",
            vec![],
            block(
                vec![
                    Stmt::Expr {
                        expr: Box::new(call_expr("foo", vec![])),
                        span: dummy_span(),
                    },
                    Stmt::Expr {
                        expr: Box::new(call_expr("bar", vec![])),
                        span: dummy_span(),
                    },
                ],
                None,
            ),
        );
        let prog = make_program(vec![Item::FnDef(f)]);
        let report = analyze_program(&prog);

        let info = &report.functions[0];
        assert_eq!(info.calls.len(), 2);
        assert!(info.calls.contains(&"foo".to_string()));
        assert!(info.calls.contains(&"bar".to_string()));
    }

    #[test]
    fn detect_direct_recursion() {
        let f = make_fn(
            "recur",
            vec![],
            block(
                vec![Stmt::Expr {
                    expr: Box::new(call_expr("recur", vec![])),
                    span: dummy_span(),
                }],
                None,
            ),
        );
        let prog = make_program(vec![Item::FnDef(f)]);
        let report = analyze_program(&prog);

        assert!(report.functions[0].is_recursive);
        assert!(report.warnings.iter().any(|w| matches!(w,
            StackWarning::Recursion { function, .. } if function == "recur"
        )));
    }

    #[test]
    fn detect_mutual_recursion() {
        let ping = make_fn(
            "ping",
            vec![],
            block(
                vec![Stmt::Expr {
                    expr: Box::new(call_expr("pong", vec![])),
                    span: dummy_span(),
                }],
                None,
            ),
        );
        let pong = make_fn(
            "pong",
            vec![],
            block(
                vec![Stmt::Expr {
                    expr: Box::new(call_expr("ping", vec![])),
                    span: dummy_span(),
                }],
                None,
            ),
        );
        let prog = make_program(vec![Item::FnDef(ping), Item::FnDef(pong)]);
        let report = analyze_program(&prog);

        let recursive_count = report.functions.iter().filter(|f| f.is_recursive).count();
        assert!(recursive_count >= 1);
        assert!(
            report
                .warnings
                .iter()
                .any(|w| matches!(w, StackWarning::Recursion { .. }))
        );
    }

    #[test]
    fn no_warnings_for_simple_code() {
        let f = make_fn("simple", vec![], int_lit(42));
        let prog = make_program(vec![Item::FnDef(f)]);
        let report = analyze_program(&prog);

        assert!(report.warnings.is_empty());
    }

    #[test]
    fn memory_map_generated() {
        let f = make_fn(
            "main",
            vec![],
            block(
                vec![let_stmt("x", Some(simple_type("i64")), int_lit(0))],
                None,
            ),
        );
        let prog = make_program(vec![Item::FnDef(f)]);
        let report = analyze_program(&prog);

        assert_eq!(report.memory_map.len(), 4);
        assert_eq!(report.memory_map[0].name, ".text");
        assert_eq!(report.memory_map[1].name, ".data");
        assert_eq!(report.memory_map[2].name, ".bss");
        assert_eq!(report.memory_map[3].name, ".stack");
        assert!(report.memory_map[0].size > 0);
        assert_eq!(report.memory_map[2].size, 0);
    }

    #[test]
    fn for_loop_counts_variable() {
        let f = make_fn(
            "looper",
            vec![],
            Expr::For {
                label: None,
                variable: "i".into(),
                iterable: Box::new(Expr::Range {
                    start: Some(Box::new(int_lit(0))),
                    end: Some(Box::new(int_lit(10))),
                    inclusive: false,
                    span: dummy_span(),
                }),
                body: Box::new(call_expr("foo", vec![])),
                span: dummy_span(),
            },
        );
        let prog = make_program(vec![Item::FnDef(f)]);
        let report = analyze_program(&prog);

        let info = &report.functions[0];
        assert_eq!(info.local_count, 1); // for variable 'i'
        assert!(info.calls.contains(&"foo".to_string()));
    }

    #[test]
    fn untyped_locals_use_default_size() {
        let f = make_fn(
            "f",
            vec![],
            block(vec![let_stmt("x", None, int_lit(42))], None),
        );
        let prog = make_program(vec![Item::FnDef(f)]);
        let report = analyze_program(&prog);

        let info = &report.functions[0];
        assert_eq!(info.local_count, 1);
        assert_eq!(info.frame_bytes, DEFAULT_VAR_SIZE + PTR_SIZE);
    }

    #[test]
    fn empty_program_analysis() {
        let prog = make_program(vec![]);
        let report = analyze_program(&prog);

        assert!(report.functions.is_empty());
        assert!(report.warnings.is_empty());
        assert_eq!(report.memory_map.len(), 4);
        assert_eq!(report.memory_map[0].size, 0);
    }

    #[test]
    fn multiple_functions_all_analyzed() {
        let f1 = make_fn("a", vec![], int_lit(1));
        let f2 = make_fn("b", vec![], int_lit(2));
        let f3 = make_fn("c", vec![], int_lit(3));
        let prog = make_program(vec![Item::FnDef(f1), Item::FnDef(f2), Item::FnDef(f3)]);
        let report = analyze_program(&prog);

        assert_eq!(report.functions.len(), 3);
        let names: Vec<&str> = report.functions.iter().map(|f| f.name.as_str()).collect();
        assert!(names.contains(&"a"));
        assert!(names.contains(&"b"));
        assert!(names.contains(&"c"));
    }
}
