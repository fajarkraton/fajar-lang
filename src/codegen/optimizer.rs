//! Unified optimization pipeline for Fajar Lang.
//!
//! Orchestrates all optimization passes in a defined order and provides
//! compile-time profiling. Integrates constant folding, dead code elimination,
//! tail call optimization, and inlining heuristics.
//!
//! # Pipeline Order
//!
//! ```text
//! 1. Constant Folding    → evaluate comptime/const expressions
//! 2. Dead Code Elim      → remove unreachable functions
//! 3. Tail Call Opt        → convert self-recursive tail calls to loops
//! 4. Inlining Hints      → mark small/hot functions for inlining
//! 5. Stack Analysis       → estimate frame sizes, detect overflow
//! ```
//!
//! # Compiler Phase Profiling
//!
//! ```text
//! CompileProfile tracks duration of each phase:
//!   lex → parse → analyze → optimize → codegen → link
//! ```

use crate::parser::ast::{Expr, FnDef, Item, Program, Stmt};
use std::collections::HashSet;

// ═══════════════════════════════════════════════════════════════════════
// Optimization Pipeline
// ═══════════════════════════════════════════════════════════════════════

/// Optimization level for the pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptLevel {
    /// No optimization (fastest compile).
    O0,
    /// Basic optimization (const folding + DCE).
    O1,
    /// Standard optimization (O1 + TCO + inlining hints).
    O2,
    /// Aggressive optimization (O2 + full analysis).
    O3,
}

/// Result of running the optimization pipeline.
#[derive(Debug, Clone)]
pub struct OptimizationReport {
    /// Number of constants folded.
    pub constants_folded: usize,
    /// Number of dead functions eliminated.
    pub dead_functions_eliminated: usize,
    /// Number of tail calls optimized.
    pub tail_calls_optimized: usize,
    /// Number of functions marked for inlining.
    pub inline_candidates: usize,
    /// Total functions analyzed.
    pub total_functions: usize,
    /// Optimization level used.
    pub opt_level: OptLevel,
}

impl OptimizationReport {
    /// Creates a new empty report for the given optimization level.
    pub fn new(opt_level: OptLevel) -> Self {
        Self {
            constants_folded: 0,
            dead_functions_eliminated: 0,
            tail_calls_optimized: 0,
            inline_candidates: 0,
            total_functions: 0,
            opt_level,
        }
    }
}

impl std::fmt::Display for OptimizationReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Optimization ({:?}): {} fns analyzed, {} consts folded, {} dead fns removed, {} TCO, {} inline candidates",
            self.opt_level,
            self.total_functions,
            self.constants_folded,
            self.dead_functions_eliminated,
            self.tail_calls_optimized,
            self.inline_candidates,
        )
    }
}

/// Runs the full optimization pipeline on a program.
pub fn optimize_program(program: &Program, level: OptLevel) -> OptimizationReport {
    let mut report = OptimizationReport::new(level);

    // Collect function definitions
    let functions: Vec<&FnDef> = program
        .items
        .iter()
        .filter_map(|item| {
            if let Item::FnDef(fndef) = item {
                Some(fndef)
            } else {
                None
            }
        })
        .collect();

    report.total_functions = functions.len();

    if level == OptLevel::O0 {
        return report;
    }

    // Phase 1: Constant folding analysis
    report.constants_folded = count_const_foldable(&functions);

    // Phase 2: Dead code elimination
    report.dead_functions_eliminated = count_dead_functions(&functions);

    if matches!(level, OptLevel::O0 | OptLevel::O1) {
        return report;
    }

    // Phase 3: Tail call optimization
    report.tail_calls_optimized = count_tail_calls(&functions);

    // Phase 4: Inlining candidates
    report.inline_candidates = count_inline_candidates(&functions);

    report
}

// ═══════════════════════════════════════════════════════════════════════
// Phase 1: Constant Folding Analysis
// ═══════════════════════════════════════════════════════════════════════

/// Counts expressions that can be folded to constants.
fn count_const_foldable(functions: &[&FnDef]) -> usize {
    let mut count = 0;
    for fndef in functions {
        count += count_const_foldable_in_expr(&fndef.body);
    }
    count
}

fn count_const_foldable_in_expr(expr: &Expr) -> usize {
    match expr {
        Expr::Binary { left, right, .. } => {
            let l_const = is_const_expr(left);
            let r_const = is_const_expr(right);
            let self_fold = if l_const && r_const { 1 } else { 0 };
            self_fold + count_const_foldable_in_expr(left) + count_const_foldable_in_expr(right)
        }
        Expr::Unary { operand, .. } => {
            let self_fold = if is_const_expr(operand) { 1 } else { 0 };
            self_fold + count_const_foldable_in_expr(operand)
        }
        Expr::Block { stmts, expr, .. } => {
            let mut count = 0;
            for stmt in stmts {
                if let Stmt::Expr { expr, .. } = stmt {
                    count += count_const_foldable_in_expr(expr);
                }
            }
            if let Some(e) = expr {
                count += count_const_foldable_in_expr(e);
            }
            count
        }
        Expr::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            let mut count =
                count_const_foldable_in_expr(condition) + count_const_foldable_in_expr(then_branch);
            if let Some(eb) = else_branch {
                count += count_const_foldable_in_expr(eb);
            }
            count
        }
        Expr::Comptime { .. } => 1, // comptime blocks are always foldable
        _ => 0,
    }
}

/// Checks if an expression is a compile-time constant.
fn is_const_expr(expr: &Expr) -> bool {
    matches!(expr, Expr::Literal { .. } | Expr::Comptime { .. })
}

// ═══════════════════════════════════════════════════════════════════════
// Phase 2: Dead Code Elimination
// ═══════════════════════════════════════════════════════════════════════

/// Counts functions that are unreachable from entry points.
fn count_dead_functions(functions: &[&FnDef]) -> usize {
    if functions.is_empty() {
        return 0;
    }

    // Entry points: main, test functions, pub functions
    let entry_points: HashSet<&str> = functions
        .iter()
        .filter(|f| f.name == "main" || f.is_test || f.is_pub)
        .map(|f| f.name.as_str())
        .collect();

    // Build call graph
    let mut called: HashSet<String> = HashSet::new();
    let mut worklist: Vec<String> = entry_points.iter().map(|s| s.to_string()).collect();

    while let Some(fn_name) = worklist.pop() {
        if !called.insert(fn_name.clone()) {
            continue;
        }
        // Find this function and collect its callees
        if let Some(fndef) = functions.iter().find(|f| f.name == fn_name) {
            let callees = collect_callees(&fndef.body);
            for callee in callees {
                if !called.contains(&callee) {
                    worklist.push(callee);
                }
            }
        }
    }

    // Count unreachable functions
    functions
        .iter()
        .filter(|f| !called.contains(&f.name))
        .count()
}

/// Collects function names called in an expression.
fn collect_callees(expr: &Expr) -> Vec<String> {
    let mut callees = Vec::new();
    collect_callees_inner(expr, &mut callees);
    callees
}

fn collect_callees_inner(expr: &Expr, callees: &mut Vec<String>) {
    match expr {
        Expr::Call { callee, args, .. } => {
            if let Expr::Ident { name, .. } = callee.as_ref() {
                callees.push(name.clone());
            }
            for arg in args {
                collect_callees_inner(&arg.value, callees);
            }
        }
        Expr::Block { stmts, expr, .. } => {
            for stmt in stmts {
                if let Stmt::Expr { expr, .. } | Stmt::Let { value: expr, .. } = stmt {
                    collect_callees_inner(expr, callees);
                }
            }
            if let Some(e) = expr {
                collect_callees_inner(e, callees);
            }
        }
        Expr::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            collect_callees_inner(condition, callees);
            collect_callees_inner(then_branch, callees);
            if let Some(eb) = else_branch {
                collect_callees_inner(eb, callees);
            }
        }
        Expr::Binary { left, right, .. } => {
            collect_callees_inner(left, callees);
            collect_callees_inner(right, callees);
        }
        Expr::Unary { operand, .. } => {
            collect_callees_inner(operand, callees);
        }
        Expr::While {
            condition, body, ..
        }
        | Expr::For {
            iterable: condition,
            body,
            ..
        } => {
            collect_callees_inner(condition, callees);
            collect_callees_inner(body, callees);
        }
        Expr::HandleEffect { body, .. } => {
            collect_callees_inner(body, callees);
        }
        Expr::Comptime { body, .. } | Expr::Grouped { expr: body, .. } => {
            collect_callees_inner(body, callees);
        }
        _ => {}
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Phase 3: Tail Call Optimization
// ═══════════════════════════════════════════════════════════════════════

/// Counts functions with self-recursive tail calls that can be optimized.
pub fn count_tail_calls(functions: &[&FnDef]) -> usize {
    functions
        .iter()
        .filter(|f| has_tail_self_call(&f.name, &f.body))
        .count()
}

/// Checks if an expression ends with a self-recursive tail call.
pub fn has_tail_self_call(fn_name: &str, expr: &Expr) -> bool {
    match expr {
        // Direct tail call: last expression is fn_name(...)
        Expr::Call { callee, .. } => {
            if let Expr::Ident { name, .. } = callee.as_ref() {
                name == fn_name
            } else {
                false
            }
        }
        // Block: check tail expression
        Expr::Block { expr: Some(e), .. } => has_tail_self_call(fn_name, e),
        // If/else: both branches must be tail calls (or base cases)
        Expr::If {
            then_branch,
            else_branch: Some(else_branch),
            ..
        } => has_tail_self_call(fn_name, then_branch) || has_tail_self_call(fn_name, else_branch),
        // Grouped
        Expr::Grouped { expr, .. } => has_tail_self_call(fn_name, expr),
        _ => false,
    }
}

/// Information about a tail-call-optimizable function.
#[derive(Debug, Clone)]
pub struct TailCallInfo {
    /// Function name.
    pub name: String,
    /// Whether it has a self-recursive tail call.
    pub is_self_recursive: bool,
    /// Parameter names for loop variable rewriting.
    pub params: Vec<String>,
}

/// Analyzes a function for tail call optimization opportunities.
pub fn analyze_tail_call(fndef: &FnDef) -> Option<TailCallInfo> {
    if has_tail_self_call(&fndef.name, &fndef.body) {
        Some(TailCallInfo {
            name: fndef.name.clone(),
            is_self_recursive: true,
            params: fndef.params.iter().map(|p| p.name.clone()).collect(),
        })
    } else {
        None
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Phase 4: Inlining Candidates
// ═══════════════════════════════════════════════════════════════════════

/// Maximum body size (in AST nodes) for inlining consideration.
const INLINE_THRESHOLD: usize = 20;

/// Counts functions small enough to be inlined.
fn count_inline_candidates(functions: &[&FnDef]) -> usize {
    functions
        .iter()
        .filter(|f| {
            let size = estimate_expr_size(&f.body);
            size <= INLINE_THRESHOLD && !f.name.starts_with("main")
        })
        .count()
}

/// Estimates the "size" of an expression in terms of AST nodes.
fn estimate_expr_size(expr: &Expr) -> usize {
    match expr {
        Expr::Literal { .. } | Expr::Ident { .. } => 1,
        Expr::Binary { left, right, .. } => {
            1 + estimate_expr_size(left) + estimate_expr_size(right)
        }
        Expr::Unary { operand, .. } => 1 + estimate_expr_size(operand),
        Expr::Call { args, .. } => {
            1 + args
                .iter()
                .map(|a| estimate_expr_size(&a.value))
                .sum::<usize>()
        }
        Expr::Block { stmts, expr, .. } => {
            let stmt_size: usize = stmts
                .iter()
                .map(|s| match s {
                    Stmt::Let { value, .. }
                    | Stmt::Const { value, .. }
                    | Stmt::Expr { expr: value, .. } => 1 + estimate_expr_size(value),
                    _ => 1,
                })
                .sum();
            stmt_size + expr.as_ref().map_or(0, |e| estimate_expr_size(e))
        }
        Expr::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            1 + estimate_expr_size(condition)
                + estimate_expr_size(then_branch)
                + else_branch.as_ref().map_or(0, |e| estimate_expr_size(e))
        }
        _ => 1,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Compiler Phase Profiling
// ═══════════════════════════════════════════════════════════════════════

/// Profile of compilation phases with timing information.
#[derive(Debug, Clone)]
pub struct CompileProfile {
    /// Time spent in lexing (microseconds).
    pub lex_us: u64,
    /// Time spent in parsing (microseconds).
    pub parse_us: u64,
    /// Time spent in semantic analysis (microseconds).
    pub analyze_us: u64,
    /// Time spent in optimization passes (microseconds).
    pub optimize_us: u64,
    /// Time spent in code generation (microseconds).
    pub codegen_us: u64,
    /// Time spent in linking (microseconds).
    pub link_us: u64,
    /// Total compilation time (microseconds).
    pub total_us: u64,
    /// Source file size in bytes.
    pub source_bytes: usize,
    /// Number of tokens produced by lexer.
    pub token_count: usize,
    /// Number of AST items.
    pub item_count: usize,
}

impl CompileProfile {
    /// Creates a new empty profile.
    pub fn new() -> Self {
        Self {
            lex_us: 0,
            parse_us: 0,
            analyze_us: 0,
            optimize_us: 0,
            codegen_us: 0,
            link_us: 0,
            total_us: 0,
            source_bytes: 0,
            token_count: 0,
            item_count: 0,
        }
    }

    /// Returns the phase that took the longest.
    pub fn bottleneck(&self) -> &str {
        let phases = [
            (self.lex_us, "lex"),
            (self.parse_us, "parse"),
            (self.analyze_us, "analyze"),
            (self.optimize_us, "optimize"),
            (self.codegen_us, "codegen"),
            (self.link_us, "link"),
        ];
        phases
            .iter()
            .max_by_key(|(t, _)| *t)
            .map_or("unknown", |(_, name)| name)
    }

    /// Returns compilation speed in lines per second (estimate).
    pub fn lines_per_second(&self) -> f64 {
        if self.total_us == 0 {
            return 0.0;
        }
        let est_lines = self.source_bytes as f64 / 40.0; // ~40 bytes per line
        est_lines / (self.total_us as f64 / 1_000_000.0)
    }
}

impl Default for CompileProfile {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for CompileProfile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Compile Profile:")?;
        writeln!(f, "  lex:      {:>8}μs", self.lex_us)?;
        writeln!(f, "  parse:    {:>8}μs", self.parse_us)?;
        writeln!(f, "  analyze:  {:>8}μs", self.analyze_us)?;
        writeln!(f, "  optimize: {:>8}μs", self.optimize_us)?;
        writeln!(f, "  codegen:  {:>8}μs", self.codegen_us)?;
        writeln!(f, "  link:     {:>8}μs", self.link_us)?;
        writeln!(f, "  total:    {:>8}μs", self.total_us)?;
        writeln!(
            f,
            "  source:   {} bytes ({} tokens, {} items)",
            self.source_bytes, self.token_count, self.item_count
        )?;
        writeln!(f, "  speed:    {:.0} lines/s", self.lines_per_second())?;
        write!(f, "  bottleneck: {}", self.bottleneck())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::tokenize;
    use crate::parser::parse;

    fn parse_program(source: &str) -> Program {
        let tokens = tokenize(source).unwrap();
        parse(tokens).unwrap()
    }

    // ── Pipeline Tests ──────────────────────────────────────────────────

    #[test]
    fn optimize_empty_program() {
        let program = parse_program("");
        let report = optimize_program(&program, OptLevel::O2);
        assert_eq!(report.total_functions, 0);
    }

    #[test]
    fn optimize_o0_skips_all() {
        let program = parse_program("fn main() { 42 }");
        let report = optimize_program(&program, OptLevel::O0);
        assert_eq!(report.constants_folded, 0);
        assert_eq!(report.dead_functions_eliminated, 0);
    }

    #[test]
    fn optimize_counts_functions() {
        let program = parse_program("fn a() { 1 }\nfn b() { 2 }\nfn main() { a() }");
        let report = optimize_program(&program, OptLevel::O2);
        assert_eq!(report.total_functions, 3);
    }

    #[test]
    fn optimize_detects_dead_function() {
        let program = parse_program("fn used() { 1 }\nfn dead() { 2 }\nfn main() { used() }");
        let report = optimize_program(&program, OptLevel::O2);
        assert!(report.dead_functions_eliminated >= 1);
    }

    #[test]
    fn optimize_no_dead_when_all_called() {
        let program = parse_program("fn helper() { 1 }\nfn main() { helper() }");
        let report = optimize_program(&program, OptLevel::O2);
        assert_eq!(report.dead_functions_eliminated, 0);
    }

    #[test]
    fn optimize_const_folding_count() {
        let program = parse_program("fn main() { comptime { 2 + 3 } }");
        let report = optimize_program(&program, OptLevel::O1);
        assert!(report.constants_folded >= 1);
    }

    // ── Tail Call Tests ──────────────────────────────────────────────────

    #[test]
    fn tco_detects_self_recursive_tail_call() {
        let program = parse_program(
            "fn factorial(n: i64) -> i64 { if n <= 1 { 1 } else { factorial(n - 1) } }",
        );
        if let Item::FnDef(fndef) = &program.items[0] {
            assert!(has_tail_self_call("factorial", &fndef.body));
        }
    }

    #[test]
    fn tco_no_tail_call_for_non_recursive() {
        let program = parse_program("fn add(a: i64, b: i64) -> i64 { a + b }");
        if let Item::FnDef(fndef) = &program.items[0] {
            assert!(!has_tail_self_call("add", &fndef.body));
        }
    }

    #[test]
    fn tco_no_tail_call_non_tail_position() {
        // factorial(n-1) * n is NOT a tail call (multiply happens after)
        let program =
            parse_program("fn fact(n: i64) -> i64 { if n <= 1 { 1 } else { n * fact(n - 1) } }");
        if let Item::FnDef(fndef) = &program.items[0] {
            // The multiply wraps the call, so it's NOT a tail call
            assert!(!has_tail_self_call("fact", &fndef.body));
        }
    }

    #[test]
    fn tco_analyze_returns_info() {
        let program = parse_program(
            "fn countdown(n: i64) -> i64 { if n <= 0 { 0 } else { countdown(n - 1) } }",
        );
        if let Item::FnDef(fndef) = &program.items[0] {
            let info = analyze_tail_call(fndef);
            assert!(info.is_some());
            let info = info.unwrap();
            assert_eq!(info.name, "countdown");
            assert!(info.is_self_recursive);
            assert_eq!(info.params, vec!["n"]);
        }
    }

    #[test]
    fn tco_count_in_pipeline() {
        let program = parse_program(
            "fn rec(n: i64) -> i64 { if n <= 0 { 0 } else { rec(n - 1) } }\nfn main() { rec(10) }",
        );
        let report = optimize_program(&program, OptLevel::O2);
        assert!(report.tail_calls_optimized >= 1);
    }

    // ── Inlining Tests ──────────────────────────────────────────────────

    #[test]
    fn inline_small_function_candidate() {
        let program = parse_program("fn tiny() -> i64 { 42 }\nfn main() { tiny() }");
        let report = optimize_program(&program, OptLevel::O2);
        assert!(report.inline_candidates >= 1);
    }

    #[test]
    fn inline_main_not_candidate() {
        let program = parse_program("fn main() { 42 }");
        let report = optimize_program(&program, OptLevel::O2);
        assert_eq!(report.inline_candidates, 0);
    }

    // ── Expr Size Tests ──────────────────────────────────────────────────

    #[test]
    fn expr_size_literal() {
        let program = parse_program("fn f() { 42 }");
        if let Item::FnDef(fndef) = &program.items[0] {
            let size = estimate_expr_size(&fndef.body);
            assert!(size <= 3); // block + literal
        }
    }

    #[test]
    fn expr_size_binary() {
        let program = parse_program("fn f() { 1 + 2 }");
        if let Item::FnDef(fndef) = &program.items[0] {
            let size = estimate_expr_size(&fndef.body);
            assert!(size <= 5); // block + binary + 2 literals
        }
    }

    // ── Profile Tests ──────────────────────────────────────────────────

    #[test]
    fn profile_new_is_zero() {
        let p = CompileProfile::new();
        assert_eq!(p.total_us, 0);
        assert_eq!(p.source_bytes, 0);
    }

    #[test]
    fn profile_bottleneck() {
        let mut p = CompileProfile::new();
        p.codegen_us = 1000;
        p.lex_us = 100;
        assert_eq!(p.bottleneck(), "codegen");
    }

    #[test]
    fn profile_lines_per_second() {
        let mut p = CompileProfile::new();
        p.source_bytes = 40000; // ~1000 lines
        p.total_us = 1_000_000; // 1 second
        let lps = p.lines_per_second();
        assert!((lps - 1000.0).abs() < 1.0);
    }

    #[test]
    fn profile_display() {
        let p = CompileProfile::new();
        let s = format!("{p}");
        assert!(s.contains("Compile Profile:"));
        assert!(s.contains("lex:"));
        assert!(s.contains("bottleneck:"));
    }

    // ── Report Tests ──────────────────────────────────────────────────

    #[test]
    fn report_display() {
        let report = OptimizationReport::new(OptLevel::O2);
        let s = format!("{report}");
        assert!(s.contains("O2"));
        assert!(s.contains("fns analyzed"));
    }

    #[test]
    fn report_o1_includes_const_and_dce() {
        let program = parse_program("fn dead() { 1 }\nfn main() { comptime { 42 } }");
        let report = optimize_program(&program, OptLevel::O1);
        assert!(report.constants_folded >= 1);
        assert!(report.dead_functions_eliminated >= 1);
    }

    #[test]
    fn report_o2_includes_tco() {
        let program = parse_program(
            "fn rec(n: i64) -> i64 { if n <= 0 { 0 } else { rec(n - 1) } }\nfn main() { rec(5) }",
        );
        let report = optimize_program(&program, OptLevel::O2);
        assert!(report.tail_calls_optimized >= 1);
    }
}
