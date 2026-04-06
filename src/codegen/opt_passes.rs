//! Comprehensive compiler optimization passes for Fajar Lang.
//!
//! Provides real, working implementations for optimization passes that complement
//! the existing `optimizer.rs` (constant folding detection, DCE, TCO, inlining)
//! and `analysis.rs` (stack estimation, call graph, memory layout).
//!
//! # Passes Included
//!
//! | Pass | Section | Description |
//! |------|---------|-------------|
//! | String Interning | OPT1.7 | Deduplicate string literals at compile time |
//! | Compilation Metrics | OPT1.17 | Phase-level timing and memory tracking |
//! | Constant Folding | OPT2.1 | Evaluate constant expressions at compile time |
//! | Loop Unrolling | OPT2.4 | Identify loops suitable for unrolling |
//! | Strength Reduction | OPT2.7 | Replace expensive ops with cheaper equivalents |
//! | Escape Analysis | OPT2.9 | Determine if values escape their defining scope |
//! | Copy Propagation | OPT2.15 | Propagate copies to eliminate redundant variables |
//! | Optimization Pipeline | OPT2.17 | Orchestrate passes by optimization level |
//! | Dead Function Elim | OPT3.1 | Remove unreachable functions |
//! | String Deduplication | OPT3.3 | Merge identical string constants |
//! | Size Profiling | OPT3.11 | Estimate per-function binary size contribution |
//! | Optimization Report | OPT3.28 | Generate markdown report of optimizations |

use std::collections::{HashMap, HashSet};

use crate::parser::ast::{BinOp, Expr, FnDef, Item, LiteralKind, Program, Stmt, UnaryOp};

// ═══════════════════════════════════════════════════════════════════════
// OPT1.7 — String Interning
// ═══════════════════════════════════════════════════════════════════════

/// A string interner that maps string values to compact integer indices.
///
/// Used during compilation to deduplicate string literals. Each unique string
/// is stored once and referenced by a `u32` index, reducing memory usage and
/// enabling cheap equality checks via index comparison.
///
/// # Example
///
/// ```ignore
/// let mut interner = StringInterner::new();
/// let id1 = interner.intern("hello");
/// let id2 = interner.intern("hello");
/// assert_eq!(id1, id2);  // same string => same index
/// assert_eq!(interner.resolve(id1), "hello");
/// ```
#[derive(Debug, Clone)]
pub struct StringInterner {
    /// Map from string content to its interned index.
    strings: HashMap<String, u32>,
    /// Reverse index: position -> string content.
    index: Vec<String>,
}

impl StringInterner {
    /// Creates a new empty string interner.
    pub fn new() -> Self {
        Self {
            strings: HashMap::new(),
            index: Vec::new(),
        }
    }

    /// Interns a string, returning its unique index.
    ///
    /// If the string has already been interned, returns the existing index.
    /// Otherwise, assigns a new index and stores the string.
    pub fn intern(&mut self, s: &str) -> u32 {
        if let Some(&id) = self.strings.get(s) {
            return id;
        }
        let id = self.index.len() as u32;
        self.strings.insert(s.to_string(), id);
        self.index.push(s.to_string());
        id
    }

    /// Resolves an interned index back to its string content.
    ///
    /// # Panics
    ///
    /// Panics if `id` is out of bounds. Use `id < interner.len()` to check.
    pub fn resolve(&self, id: u32) -> &str {
        &self.index[id as usize]
    }

    /// Returns the number of unique strings interned.
    pub fn len(&self) -> usize {
        self.index.len()
    }

    /// Returns true if no strings have been interned.
    pub fn is_empty(&self) -> bool {
        self.index.is_empty()
    }

    /// Returns true if the given string has already been interned.
    pub fn contains(&self, s: &str) -> bool {
        self.strings.contains_key(s)
    }

    /// Interns all string literals found in a program, returning the interner.
    pub fn from_program(program: &Program) -> Self {
        let mut interner = Self::new();
        for item in &program.items {
            intern_strings_in_item(item, &mut interner);
        }
        interner
    }
}

impl Default for StringInterner {
    fn default() -> Self {
        Self::new()
    }
}

/// Recursively interns string literals found in an AST item.
fn intern_strings_in_item(item: &Item, interner: &mut StringInterner) {
    match item {
        Item::FnDef(fndef) => intern_strings_in_expr(&fndef.body, interner),
        Item::Stmt(stmt) => intern_strings_in_stmt(stmt, interner),
        Item::ImplBlock(imp) => {
            for method in &imp.methods {
                intern_strings_in_expr(&method.body, interner);
            }
        }
        Item::ConstDef(c) => intern_strings_in_expr(&c.value, interner),
        Item::StaticDef(s) => intern_strings_in_expr(&s.value, interner),
        Item::ModDecl(m) => {
            if let Some(body) = &m.body {
                for sub_item in body {
                    intern_strings_in_item(sub_item, interner);
                }
            }
        }
        _ => {}
    }
}

/// Recursively interns string literals found in an expression.
fn intern_strings_in_expr(expr: &Expr, interner: &mut StringInterner) {
    match expr {
        Expr::Literal { kind, .. } => {
            if let LiteralKind::String(s) | LiteralKind::RawString(s) = kind {
                interner.intern(s);
            }
        }
        Expr::Binary { left, right, .. } => {
            intern_strings_in_expr(left, interner);
            intern_strings_in_expr(right, interner);
        }
        Expr::Unary { operand, .. } => {
            intern_strings_in_expr(operand, interner);
        }
        Expr::Call { callee, args, .. } => {
            intern_strings_in_expr(callee, interner);
            for arg in args {
                intern_strings_in_expr(&arg.value, interner);
            }
        }
        Expr::Block { stmts, expr, .. } => {
            for stmt in stmts {
                intern_strings_in_stmt(stmt, interner);
            }
            if let Some(e) = expr {
                intern_strings_in_expr(e, interner);
            }
        }
        Expr::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            intern_strings_in_expr(condition, interner);
            intern_strings_in_expr(then_branch, interner);
            if let Some(eb) = else_branch {
                intern_strings_in_expr(eb, interner);
            }
        }
        Expr::While {
            condition, body, ..
        } => {
            intern_strings_in_expr(condition, interner);
            intern_strings_in_expr(body, interner);
        }
        Expr::For { iterable, body, .. } => {
            intern_strings_in_expr(iterable, interner);
            intern_strings_in_expr(body, interner);
        }
        Expr::Loop { body, .. } => intern_strings_in_expr(body, interner),
        Expr::Match { subject, arms, .. } => {
            intern_strings_in_expr(subject, interner);
            for arm in arms {
                intern_strings_in_expr(&arm.body, interner);
            }
        }
        Expr::Array { elements, .. } | Expr::Tuple { elements, .. } => {
            for e in elements {
                intern_strings_in_expr(e, interner);
            }
        }
        Expr::FString { parts, .. } => {
            for part in parts {
                match part {
                    crate::parser::ast::FStringExprPart::Literal(s) => {
                        interner.intern(s);
                    }
                    crate::parser::ast::FStringExprPart::Expr(e) => {
                        intern_strings_in_expr(e, interner);
                    }
                }
            }
        }
        Expr::Grouped { expr, .. }
        | Expr::Cast { expr, .. }
        | Expr::Try { expr, .. }
        | Expr::Await { expr, .. } => {
            intern_strings_in_expr(expr, interner);
        }
        Expr::Closure { body, .. }
        | Expr::AsyncBlock { body, .. }
        | Expr::Comptime { body, .. } => {
            intern_strings_in_expr(body, interner);
        }
        Expr::Assign { target, value, .. } => {
            intern_strings_in_expr(target, interner);
            intern_strings_in_expr(value, interner);
        }
        Expr::Pipe { left, right, .. } => {
            intern_strings_in_expr(left, interner);
            intern_strings_in_expr(right, interner);
        }
        Expr::StructInit { fields, .. } => {
            for f in fields {
                intern_strings_in_expr(&f.value, interner);
            }
        }
        Expr::Index { object, index, .. } => {
            intern_strings_in_expr(object, interner);
            intern_strings_in_expr(index, interner);
        }
        Expr::Field { object, .. } => {
            intern_strings_in_expr(object, interner);
        }
        Expr::MethodCall { receiver, args, .. } => {
            intern_strings_in_expr(receiver, interner);
            for arg in args {
                intern_strings_in_expr(&arg.value, interner);
            }
        }
        Expr::ArrayRepeat { value, count, .. } => {
            intern_strings_in_expr(value, interner);
            intern_strings_in_expr(count, interner);
        }
        Expr::HandleEffect { body, handlers, .. } => {
            intern_strings_in_expr(body, interner);
            for h in handlers {
                intern_strings_in_expr(&h.body, interner);
            }
        }
        Expr::ResumeExpr { value, .. } => {
            intern_strings_in_expr(value, interner);
        }
        Expr::MacroInvocation { args, .. } => {
            for arg in args {
                intern_strings_in_expr(arg, interner);
            }
        }
        // Leaf nodes with no strings
        Expr::Ident { .. } | Expr::Path { .. } | Expr::Range { .. } | Expr::InlineAsm { .. } => {}
        Expr::Yield { .. } => {}
        Expr::MacroVar { .. } => {}
    }
}

/// Recursively interns string literals found in a statement.
fn intern_strings_in_stmt(stmt: &Stmt, interner: &mut StringInterner) {
    match stmt {
        Stmt::Let { value, .. } | Stmt::Const { value, .. } | Stmt::Expr { expr: value, .. } => {
            intern_strings_in_expr(value, interner);
        }
        Stmt::Return { value, .. } => {
            if let Some(v) = value {
                intern_strings_in_expr(v, interner);
            }
        }
        Stmt::Break { value, .. } => {
            if let Some(v) = value {
                intern_strings_in_expr(v, interner);
            }
        }
        Stmt::Continue { .. } => {}
        Stmt::Item(item) => intern_strings_in_item(item, interner),
    }
}

// ═══════════════════════════════════════════════════════════════════════
// OPT1.17 — Compilation Metrics
// ═══════════════════════════════════════════════════════════════════════

/// Tracks timing and resource usage across compilation phases.
///
/// Each phase (lex, parse, analyze, optimize, codegen) can be recorded
/// individually. The metrics provide a formatted report and bottleneck
/// identification to guide optimization efforts.
#[derive(Debug, Clone)]
pub struct CompilationMetrics {
    /// Time spent in lexing (microseconds).
    pub lex_time_us: u64,
    /// Time spent in parsing (microseconds).
    pub parse_time_us: u64,
    /// Time spent in semantic analysis (microseconds).
    pub analyze_time_us: u64,
    /// Time spent in optimization passes (microseconds).
    pub optimize_time_us: u64,
    /// Time spent in code generation (microseconds).
    pub codegen_time_us: u64,
    /// Total compilation time (microseconds).
    pub total_time_us: u64,
    /// Peak memory usage in bytes (estimated).
    pub peak_memory_bytes: usize,
    /// Number of functions compiled.
    pub functions_compiled: usize,
    /// Total number of optimization transformations applied.
    pub optimizations_applied: usize,
}

impl CompilationMetrics {
    /// Creates a new metrics tracker with all values zeroed.
    pub fn new() -> Self {
        Self {
            lex_time_us: 0,
            parse_time_us: 0,
            analyze_time_us: 0,
            optimize_time_us: 0,
            codegen_time_us: 0,
            total_time_us: 0,
            peak_memory_bytes: 0,
            functions_compiled: 0,
            optimizations_applied: 0,
        }
    }

    /// Records the duration of a named compilation phase.
    ///
    /// Supported phase names: `"lex"`, `"parse"`, `"analyze"`, `"optimize"`, `"codegen"`.
    /// Unrecognized names are silently ignored.
    pub fn record_phase(&mut self, phase: &str, duration_us: u64) {
        match phase {
            "lex" => self.lex_time_us = duration_us,
            "parse" => self.parse_time_us = duration_us,
            "analyze" => self.analyze_time_us = duration_us,
            "optimize" => self.optimize_time_us = duration_us,
            "codegen" => self.codegen_time_us = duration_us,
            _ => {}
        }
        self.total_time_us = self.lex_time_us
            + self.parse_time_us
            + self.analyze_time_us
            + self.optimize_time_us
            + self.codegen_time_us;
    }

    /// Generates a formatted text report of all compilation metrics.
    pub fn report(&self) -> String {
        let mut lines = Vec::new();
        lines.push("Compilation Metrics:".to_string());
        lines.push(format!("  lex:         {:>8} us", self.lex_time_us));
        lines.push(format!("  parse:       {:>8} us", self.parse_time_us));
        lines.push(format!("  analyze:     {:>8} us", self.analyze_time_us));
        lines.push(format!("  optimize:    {:>8} us", self.optimize_time_us));
        lines.push(format!("  codegen:     {:>8} us", self.codegen_time_us));
        lines.push(format!("  total:       {:>8} us", self.total_time_us));
        lines.push(format!(
            "  peak memory: {:>8} bytes",
            self.peak_memory_bytes
        ));
        lines.push(format!("  functions:   {:>8}", self.functions_compiled));
        lines.push(format!(
            "  optimizations: {:>6}",
            self.optimizations_applied
        ));
        lines.push(format!("  bottleneck:  {}", self.bottleneck()));
        lines.join("\n")
    }

    /// Returns the name of the phase that took the longest.
    ///
    /// If all phases have zero duration, returns `"none"`.
    pub fn bottleneck(&self) -> &str {
        let phases = [
            (self.lex_time_us, "lex"),
            (self.parse_time_us, "parse"),
            (self.analyze_time_us, "analyze"),
            (self.optimize_time_us, "optimize"),
            (self.codegen_time_us, "codegen"),
        ];
        phases
            .iter()
            .max_by_key(|(t, _)| *t)
            .filter(|(t, _)| *t > 0)
            .map_or("none", |(_, name)| name)
    }
}

impl Default for CompilationMetrics {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// OPT2.1 — Constant Folding Pass
// ═══════════════════════════════════════════════════════════════════════

/// Attempts to evaluate a constant expression at compile time.
///
/// Returns `Some(folded_expr)` if the expression can be fully reduced to a
/// literal value, or `None` if the expression contains runtime-dependent parts.
///
/// Supported folding operations:
/// - Integer arithmetic: `2 + 3` -> `5`, `10 * 4` -> `40`
/// - Float arithmetic: `1.5 + 2.5` -> `4.0`
/// - Boolean logic: `true && false` -> `false`, `!true` -> `false`
/// - String concatenation: `"a" + "b"` -> `"ab"`
/// - Unary operations: `-5` -> `-5`, `!true` -> `false`
/// - Comparison: `3 < 5` -> `true`
pub fn constant_fold(expr: &Expr) -> Option<Expr> {
    match expr {
        Expr::Binary {
            left,
            op,
            right,
            span,
        } => {
            // Try to fold both sides first
            let l = constant_fold(left).unwrap_or_else(|| *left.clone());
            let r = constant_fold(right).unwrap_or_else(|| *right.clone());

            match (&l, op, &r) {
                // Integer operations
                (
                    Expr::Literal {
                        kind: LiteralKind::Int(a),
                        ..
                    },
                    op,
                    Expr::Literal {
                        kind: LiteralKind::Int(b),
                        ..
                    },
                ) => fold_int_binop(*a, *op, *b, *span),

                // Float operations
                (
                    Expr::Literal {
                        kind: LiteralKind::Float(a),
                        ..
                    },
                    op,
                    Expr::Literal {
                        kind: LiteralKind::Float(b),
                        ..
                    },
                ) => fold_float_binop(*a, *op, *b, *span),

                // Bool operations
                (
                    Expr::Literal {
                        kind: LiteralKind::Bool(a),
                        ..
                    },
                    op,
                    Expr::Literal {
                        kind: LiteralKind::Bool(b),
                        ..
                    },
                ) => fold_bool_binop(*a, *op, *b, *span),

                // String concatenation
                (
                    Expr::Literal {
                        kind: LiteralKind::String(a),
                        ..
                    },
                    BinOp::Add,
                    Expr::Literal {
                        kind: LiteralKind::String(b),
                        ..
                    },
                ) => Some(Expr::Literal {
                    kind: LiteralKind::String(format!("{}{}", a, b)),
                    span: *span,
                }),

                _ => None,
            }
        }

        Expr::Unary { op, operand, span } => {
            let inner = constant_fold(operand).unwrap_or_else(|| *operand.clone());
            match (op, &inner) {
                (
                    UnaryOp::Neg,
                    Expr::Literal {
                        kind: LiteralKind::Int(v),
                        ..
                    },
                ) => Some(Expr::Literal {
                    kind: LiteralKind::Int(-v),
                    span: *span,
                }),
                (
                    UnaryOp::Neg,
                    Expr::Literal {
                        kind: LiteralKind::Float(v),
                        ..
                    },
                ) => Some(Expr::Literal {
                    kind: LiteralKind::Float(-v),
                    span: *span,
                }),
                (
                    UnaryOp::Not,
                    Expr::Literal {
                        kind: LiteralKind::Bool(v),
                        ..
                    },
                ) => Some(Expr::Literal {
                    kind: LiteralKind::Bool(!v),
                    span: *span,
                }),
                (
                    UnaryOp::BitNot,
                    Expr::Literal {
                        kind: LiteralKind::Int(v),
                        ..
                    },
                ) => Some(Expr::Literal {
                    kind: LiteralKind::Int(!v),
                    span: *span,
                }),
                _ => None,
            }
        }

        // Grouped expression: fold the inner
        Expr::Grouped { expr: inner, .. } => constant_fold(inner),

        // Comptime blocks are always considered foldable
        Expr::Comptime { body, .. } => constant_fold(body),

        // Literals are already folded
        Expr::Literal { .. } => Some(expr.clone()),

        _ => None,
    }
}

/// Folds a binary operation on two integers.
fn fold_int_binop(a: i64, op: BinOp, b: i64, span: crate::lexer::token::Span) -> Option<Expr> {
    let result = match op {
        BinOp::Add => Some(LiteralKind::Int(a.wrapping_add(b))),
        BinOp::Sub => Some(LiteralKind::Int(a.wrapping_sub(b))),
        BinOp::Mul => Some(LiteralKind::Int(a.wrapping_mul(b))),
        BinOp::Div => {
            if b == 0 {
                return None; // Division by zero cannot be folded
            }
            Some(LiteralKind::Int(a / b))
        }
        BinOp::Rem => {
            if b == 0 {
                return None;
            }
            Some(LiteralKind::Int(a % b))
        }
        BinOp::Pow => {
            if (0..=63).contains(&b) {
                Some(LiteralKind::Int(a.wrapping_pow(b as u32)))
            } else {
                None
            }
        }
        BinOp::BitAnd => Some(LiteralKind::Int(a & b)),
        BinOp::BitOr => Some(LiteralKind::Int(a | b)),
        BinOp::BitXor => Some(LiteralKind::Int(a ^ b)),
        BinOp::Shl => {
            if (0..64).contains(&b) {
                Some(LiteralKind::Int(a.wrapping_shl(b as u32)))
            } else {
                None
            }
        }
        BinOp::Shr => {
            if (0..64).contains(&b) {
                Some(LiteralKind::Int(a.wrapping_shr(b as u32)))
            } else {
                None
            }
        }
        // Comparison ops return bool
        BinOp::Eq => Some(LiteralKind::Bool(a == b)),
        BinOp::Ne => Some(LiteralKind::Bool(a != b)),
        BinOp::Lt => Some(LiteralKind::Bool(a < b)),
        BinOp::Gt => Some(LiteralKind::Bool(a > b)),
        BinOp::Le => Some(LiteralKind::Bool(a <= b)),
        BinOp::Ge => Some(LiteralKind::Bool(a >= b)),
        // Logical ops don't apply to integers
        BinOp::And | BinOp::Or | BinOp::MatMul => None,
    };
    result.map(|kind| Expr::Literal { kind, span })
}

/// Folds a binary operation on two floats.
fn fold_float_binop(a: f64, op: BinOp, b: f64, span: crate::lexer::token::Span) -> Option<Expr> {
    let result = match op {
        BinOp::Add => Some(LiteralKind::Float(a + b)),
        BinOp::Sub => Some(LiteralKind::Float(a - b)),
        BinOp::Mul => Some(LiteralKind::Float(a * b)),
        BinOp::Div => {
            if b == 0.0 {
                return None;
            }
            Some(LiteralKind::Float(a / b))
        }
        BinOp::Rem => {
            if b == 0.0 {
                return None;
            }
            Some(LiteralKind::Float(a % b))
        }
        BinOp::Pow => Some(LiteralKind::Float(a.powf(b))),
        BinOp::Eq => Some(LiteralKind::Bool(a == b)),
        BinOp::Ne => Some(LiteralKind::Bool(a != b)),
        BinOp::Lt => Some(LiteralKind::Bool(a < b)),
        BinOp::Gt => Some(LiteralKind::Bool(a > b)),
        BinOp::Le => Some(LiteralKind::Bool(a <= b)),
        BinOp::Ge => Some(LiteralKind::Bool(a >= b)),
        _ => None,
    };
    result.map(|kind| Expr::Literal { kind, span })
}

/// Folds a binary operation on two booleans.
fn fold_bool_binop(a: bool, op: BinOp, b: bool, span: crate::lexer::token::Span) -> Option<Expr> {
    let result = match op {
        BinOp::And => Some(LiteralKind::Bool(a && b)),
        BinOp::Or => Some(LiteralKind::Bool(a || b)),
        BinOp::Eq => Some(LiteralKind::Bool(a == b)),
        BinOp::Ne => Some(LiteralKind::Bool(a != b)),
        BinOp::BitAnd => Some(LiteralKind::Bool(a & b)),
        BinOp::BitOr => Some(LiteralKind::Bool(a | b)),
        BinOp::BitXor => Some(LiteralKind::Bool(a ^ b)),
        _ => None,
    };
    result.map(|kind| Expr::Literal { kind, span })
}

// ═══════════════════════════════════════════════════════════════════════
// OPT2.4 — Loop Unrolling Analysis
// ═══════════════════════════════════════════════════════════════════════

/// Information about a loop that is a candidate for unrolling.
#[derive(Debug, Clone)]
pub struct UnrollCandidate {
    /// Source line where the loop begins (from span offset).
    pub loop_line: usize,
    /// Statically-known trip count (iterations), if determinable.
    pub trip_count: u64,
    /// Estimated cost of the loop body in AST nodes.
    pub body_cost: u32,
    /// Recommended unroll factor (1 = no unroll, 2 = double, etc.).
    pub recommended_factor: u32,
}

/// Maximum body cost for which unrolling is considered beneficial.
const UNROLL_MAX_BODY_COST: u32 = 50;

/// Maximum trip count for full unrolling.
const UNROLL_MAX_FULL: u64 = 16;

/// Finds loops in a program that are candidates for unrolling.
///
/// A loop is considered for unrolling when:
/// 1. It is a `for` loop with a range literal (`for i in 0..N`)
/// 2. The trip count is statically known
/// 3. The body cost is small enough to benefit from unrolling
pub fn find_unroll_candidates(program: &Program) -> Vec<UnrollCandidate> {
    let mut candidates = Vec::new();
    for item in &program.items {
        if let Item::FnDef(fndef) = item {
            find_unroll_in_expr(&fndef.body, &mut candidates);
        }
    }
    candidates
}

/// Recursively searches for unrollable loops in an expression.
fn find_unroll_in_expr(expr: &Expr, candidates: &mut Vec<UnrollCandidate>) {
    match expr {
        Expr::For { iterable, body, .. } => {
            // Check if iterable is a range with known bounds
            if let Some(trip_count) = extract_range_trip_count(iterable) {
                let body_cost = estimate_body_cost(body) as u32;
                if body_cost <= UNROLL_MAX_BODY_COST {
                    let recommended_factor = if trip_count <= UNROLL_MAX_FULL && body_cost <= 10 {
                        // Full unroll for small loops
                        trip_count as u32
                    } else if trip_count % 4 == 0 {
                        4
                    } else if trip_count % 2 == 0 {
                        2
                    } else {
                        1
                    };
                    candidates.push(UnrollCandidate {
                        loop_line: expr.span().start,
                        trip_count,
                        body_cost,
                        recommended_factor,
                    });
                }
            }
            find_unroll_in_expr(body, candidates);
        }
        Expr::Block { stmts, expr, .. } => {
            for stmt in stmts {
                if let Stmt::Expr { expr, .. } | Stmt::Let { value: expr, .. } = stmt {
                    find_unroll_in_expr(expr, candidates);
                }
            }
            if let Some(e) = expr {
                find_unroll_in_expr(e, candidates);
            }
        }
        Expr::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            find_unroll_in_expr(condition, candidates);
            find_unroll_in_expr(then_branch, candidates);
            if let Some(eb) = else_branch {
                find_unroll_in_expr(eb, candidates);
            }
        }
        Expr::While { body, .. } | Expr::Loop { body, .. } => {
            find_unroll_in_expr(body, candidates);
        }
        _ => {}
    }
}

/// Attempts to extract a static trip count from a range expression.
///
/// Handles: `0..N` (trip count = N), `A..B` (trip count = B - A).
fn extract_range_trip_count(expr: &Expr) -> Option<u64> {
    if let Expr::Range {
        start,
        end,
        inclusive,
        ..
    } = expr
    {
        let start_val = match start {
            Some(s) => extract_int_literal(s)?,
            None => 0,
        };
        let end_val = extract_int_literal(end.as_ref()?)?;
        let count = if *inclusive {
            (end_val - start_val + 1).max(0) as u64
        } else {
            (end_val - start_val).max(0) as u64
        };
        Some(count)
    } else {
        None
    }
}

/// Extracts an integer literal value from an expression.
fn extract_int_literal(expr: &Expr) -> Option<i64> {
    match expr {
        Expr::Literal {
            kind: LiteralKind::Int(v),
            ..
        } => Some(*v),
        Expr::Grouped { expr, .. } => extract_int_literal(expr),
        _ => None,
    }
}

/// Estimates the "cost" of a loop body in terms of AST node count.
fn estimate_body_cost(expr: &Expr) -> usize {
    match expr {
        Expr::Literal { .. } | Expr::Ident { .. } => 1,
        Expr::Binary { left, right, .. } => {
            1 + estimate_body_cost(left) + estimate_body_cost(right)
        }
        Expr::Unary { operand, .. } => 1 + estimate_body_cost(operand),
        Expr::Call { args, .. } => {
            1 + args
                .iter()
                .map(|a| estimate_body_cost(&a.value))
                .sum::<usize>()
        }
        Expr::Block { stmts, expr, .. } => {
            let s: usize = stmts
                .iter()
                .map(|st| match st {
                    Stmt::Let { value, .. }
                    | Stmt::Const { value, .. }
                    | Stmt::Expr { expr: value, .. } => 1 + estimate_body_cost(value),
                    _ => 1,
                })
                .sum();
            s + expr.as_ref().map_or(0, |e| estimate_body_cost(e))
        }
        Expr::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            1 + estimate_body_cost(condition)
                + estimate_body_cost(then_branch)
                + else_branch.as_ref().map_or(0, |e| estimate_body_cost(e))
        }
        Expr::Assign { target, value, .. } => {
            1 + estimate_body_cost(target) + estimate_body_cost(value)
        }
        Expr::Index { object, index, .. } => {
            1 + estimate_body_cost(object) + estimate_body_cost(index)
        }
        Expr::Field { object, .. } => 1 + estimate_body_cost(object),
        Expr::MethodCall { receiver, args, .. } => {
            1 + estimate_body_cost(receiver)
                + args
                    .iter()
                    .map(|a| estimate_body_cost(&a.value))
                    .sum::<usize>()
        }
        _ => 1,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// OPT2.7 — Strength Reduction
// ═══════════════════════════════════════════════════════════════════════

/// A strength reduction opportunity: replace an expensive operation with a cheaper one.
#[derive(Debug, Clone)]
pub struct StrengthReduction {
    /// Human-readable description of the original operation.
    pub original: String,
    /// Human-readable description of the replacement.
    pub replacement: String,
    /// Estimated savings category.
    pub savings: &'static str,
}

/// Scans a program for operations that can be replaced with cheaper equivalents.
///
/// Detects these patterns:
/// - `x * 2` -> `x << 1` (mul-to-shift)
/// - `x * 4` -> `x << 2`
/// - `x * 8` -> `x << 3`
/// - `x / 2` -> `x >> 1` (div-to-shift)
/// - `x / 4` -> `x >> 2`
/// - `x % 2` -> `x & 1` (mod-to-and)
/// - `x % 4` -> `x & 3`
/// - `x * 0` -> `0` (mul-by-zero)
/// - `x * 1` -> `x` (mul-by-one, identity)
/// - `x ** 2` -> `x * x` (pow-to-mul)
pub fn find_strength_reductions(program: &Program) -> Vec<StrengthReduction> {
    let mut reductions = Vec::new();
    for item in &program.items {
        if let Item::FnDef(fndef) = item {
            find_sr_in_expr(&fndef.body, &mut reductions);
        }
    }
    reductions
}

/// Recursively searches for strength reduction opportunities in an expression.
fn find_sr_in_expr(expr: &Expr, reductions: &mut Vec<StrengthReduction>) {
    match expr {
        Expr::Binary {
            left, op, right, ..
        } => {
            // Check for power-of-2 multiply
            if let (BinOp::Mul, Some(power)) = (op, extract_power_of_2(right)) {
                reductions.push(StrengthReduction {
                    original: format!("x * {}", 1u64 << power),
                    replacement: format!("x << {}", power),
                    savings: "mul-to-shift",
                });
            }
            // Check for multiply by power of 2 on left side
            if let (BinOp::Mul, Some(power)) = (op, extract_power_of_2(left)) {
                reductions.push(StrengthReduction {
                    original: format!("{} * x", 1u64 << power),
                    replacement: format!("x << {}", power),
                    savings: "mul-to-shift",
                });
            }

            // Check for division by power of 2
            if let (BinOp::Div, Some(power)) = (op, extract_power_of_2(right)) {
                reductions.push(StrengthReduction {
                    original: format!("x / {}", 1u64 << power),
                    replacement: format!("x >> {}", power),
                    savings: "div-to-shift",
                });
            }

            // Check for modulo by power of 2
            if let (BinOp::Rem, Some(power)) = (op, extract_power_of_2(right)) {
                let mask = (1u64 << power) - 1;
                reductions.push(StrengthReduction {
                    original: format!("x % {}", 1u64 << power),
                    replacement: format!("x & {}", mask),
                    savings: "mod-to-and",
                });
            }

            // Check for multiply by 0
            if *op == BinOp::Mul {
                if is_zero_literal(right) || is_zero_literal(left) {
                    reductions.push(StrengthReduction {
                        original: "x * 0".to_string(),
                        replacement: "0".to_string(),
                        savings: "mul-by-zero",
                    });
                }
            }

            // Check for multiply by 1 (identity)
            if *op == BinOp::Mul {
                if is_one_literal(right) || is_one_literal(left) {
                    reductions.push(StrengthReduction {
                        original: "x * 1".to_string(),
                        replacement: "x".to_string(),
                        savings: "identity",
                    });
                }
            }

            // Check for power of 2
            if *op == BinOp::Pow {
                if let Some(Expr::Literal {
                    kind: LiteralKind::Int(2),
                    ..
                }) = right.as_ref().into()
                {
                    reductions.push(StrengthReduction {
                        original: "x ** 2".to_string(),
                        replacement: "x * x".to_string(),
                        savings: "pow-to-mul",
                    });
                }
            }

            // Recurse into sub-expressions
            find_sr_in_expr(left, reductions);
            find_sr_in_expr(right, reductions);
        }
        Expr::Block { stmts, expr, .. } => {
            for stmt in stmts {
                if let Stmt::Expr { expr, .. } | Stmt::Let { value: expr, .. } = stmt {
                    find_sr_in_expr(expr, reductions);
                }
            }
            if let Some(e) = expr {
                find_sr_in_expr(e, reductions);
            }
        }
        Expr::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            find_sr_in_expr(condition, reductions);
            find_sr_in_expr(then_branch, reductions);
            if let Some(eb) = else_branch {
                find_sr_in_expr(eb, reductions);
            }
        }
        Expr::While {
            condition, body, ..
        } => {
            find_sr_in_expr(condition, reductions);
            find_sr_in_expr(body, reductions);
        }
        Expr::For { iterable, body, .. } => {
            find_sr_in_expr(iterable, reductions);
            find_sr_in_expr(body, reductions);
        }
        Expr::Unary { operand, .. } => find_sr_in_expr(operand, reductions),
        Expr::Call { args, .. } => {
            for arg in args {
                find_sr_in_expr(&arg.value, reductions);
            }
        }
        Expr::Assign { value, .. } => find_sr_in_expr(value, reductions),
        _ => {}
    }
}

/// Checks if an integer literal is a power of 2 (>= 2) and returns the exponent.
fn extract_power_of_2(expr: &Expr) -> Option<u32> {
    if let Expr::Literal {
        kind: LiteralKind::Int(v),
        ..
    } = expr
    {
        let v = *v;
        if v >= 2 && v.count_ones() == 1 {
            Some(v.trailing_zeros())
        } else {
            None
        }
    } else {
        None
    }
}

/// Checks if an expression is the integer literal `0`.
fn is_zero_literal(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::Literal {
            kind: LiteralKind::Int(0),
            ..
        }
    )
}

/// Checks if an expression is the integer literal `1`.
fn is_one_literal(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::Literal {
            kind: LiteralKind::Int(1),
            ..
        }
    )
}

// ═══════════════════════════════════════════════════════════════════════
// OPT2.9 — Escape Analysis
// ═══════════════════════════════════════════════════════════════════════

/// Describes how a value escapes from its defining scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EscapeState {
    /// Value does not escape the function — safe for stack allocation.
    NoEscape,
    /// Value escapes through a function argument (callee may store it).
    ArgEscape,
    /// Value escapes to global scope (returned, stored in global, etc.).
    GlobalEscape,
}

/// Analyzes whether a variable escapes its defining function.
///
/// A variable "escapes" when its reference or value leaves the function scope.
/// This analysis is used to determine if values can be stack-allocated instead
/// of heap-allocated.
///
/// # Escape Rules
///
/// - **NoEscape**: Variable is only used within its defining function and never
///   passed to a function or returned.
/// - **ArgEscape**: Variable is passed as an argument to another function call.
///   The callee might store it, so it cannot be stack-only.
/// - **GlobalEscape**: Variable is returned from the function or assigned to
///   a global/field that outlives the function.
pub fn analyze_escape(fn_name: &str, var_name: &str, program: &Program) -> EscapeState {
    for item in &program.items {
        if let Item::FnDef(fndef) = item {
            if fndef.name == fn_name {
                return analyze_escape_in_expr(var_name, &fndef.body);
            }
        }
    }
    EscapeState::NoEscape
}

/// Recursively analyzes escape state within an expression.
fn analyze_escape_in_expr(var_name: &str, expr: &Expr) -> EscapeState {
    match expr {
        // Bare identifier reference — does not escape by itself
        Expr::Ident { .. } => EscapeState::NoEscape,

        // Function call: if var is an argument, it escapes through the arg
        Expr::Call { args, .. } => {
            for arg in args {
                if references_var(var_name, &arg.value) {
                    return EscapeState::ArgEscape;
                }
            }
            EscapeState::NoEscape
        }

        // Method call: if var is an argument (not receiver), it escapes
        Expr::MethodCall { receiver, args, .. } => {
            // Receiver doesn't necessarily cause escape (it's `self`)
            let mut state = EscapeState::NoEscape;
            for arg in args {
                if references_var(var_name, &arg.value) {
                    state = EscapeState::ArgEscape;
                }
            }
            // But if receiver is a global store method, it might escape
            if references_var(var_name, receiver) {
                if state == EscapeState::NoEscape {
                    state = EscapeState::NoEscape; // receiver use is not necessarily escape
                }
            }
            state
        }

        // Block: check all statements, and the tail expression for return escape
        Expr::Block { stmts, expr, .. } => {
            let mut worst = EscapeState::NoEscape;
            for stmt in stmts {
                let s = analyze_escape_in_stmt(var_name, stmt);
                worst = merge_escape(worst, s);
            }
            if let Some(tail) = expr {
                // If the tail expression IS the variable directly, it escapes globally
                // (returned as the block's value). But if it's a call/binary/etc that
                // merely uses the variable, we analyze it normally.
                if is_direct_var_ref(var_name, tail) {
                    worst = merge_escape(worst, EscapeState::GlobalEscape);
                } else {
                    let s = analyze_escape_in_expr(var_name, tail);
                    worst = merge_escape(worst, s);
                }
            }
            worst
        }

        Expr::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            let mut worst = analyze_escape_in_expr(var_name, condition);
            worst = merge_escape(worst, analyze_escape_in_expr(var_name, then_branch));
            if let Some(eb) = else_branch {
                worst = merge_escape(worst, analyze_escape_in_expr(var_name, eb));
            }
            worst
        }

        Expr::While {
            condition, body, ..
        } => {
            let w = analyze_escape_in_expr(var_name, condition);
            merge_escape(w, analyze_escape_in_expr(var_name, body))
        }

        Expr::For { body, .. } | Expr::Loop { body, .. } => analyze_escape_in_expr(var_name, body),

        // Assignment to a field: the var escapes globally if stored in a struct field
        Expr::Assign { value, target, .. } => {
            if references_var(var_name, value) {
                // If target is a field access, the var escapes globally
                if matches!(target.as_ref(), Expr::Field { .. } | Expr::Index { .. }) {
                    return EscapeState::GlobalEscape;
                }
            }
            EscapeState::NoEscape
        }

        // Binary/unary: just recurse, no escape
        Expr::Binary { left, right, .. } => merge_escape(
            analyze_escape_in_expr(var_name, left),
            analyze_escape_in_expr(var_name, right),
        ),
        Expr::Unary { operand, .. } => analyze_escape_in_expr(var_name, operand),

        // Closure capturing: if the closure captures the variable, it escapes via arg
        Expr::Closure { body, .. } => {
            if references_var(var_name, body) {
                EscapeState::ArgEscape
            } else {
                EscapeState::NoEscape
            }
        }

        // Grouped/cast/try: transparent
        Expr::Grouped { expr, .. } | Expr::Cast { expr, .. } | Expr::Try { expr, .. } => {
            analyze_escape_in_expr(var_name, expr)
        }

        _ => EscapeState::NoEscape,
    }
}

/// Analyzes escape state in a statement.
fn analyze_escape_in_stmt(var_name: &str, stmt: &Stmt) -> EscapeState {
    match stmt {
        Stmt::Let { value, .. } | Stmt::Const { value, .. } | Stmt::Expr { expr: value, .. } => {
            analyze_escape_in_expr(var_name, value)
        }
        Stmt::Return { value: Some(v), .. } => {
            if references_var(var_name, v) {
                EscapeState::GlobalEscape
            } else {
                analyze_escape_in_expr(var_name, v)
            }
        }
        Stmt::Return { value: None, .. } | Stmt::Break { .. } | Stmt::Continue { .. } => {
            EscapeState::NoEscape
        }
        Stmt::Item(item) => {
            if let Item::FnDef(fndef) = item.as_ref() {
                if references_var(var_name, &fndef.body) {
                    EscapeState::ArgEscape // captured by nested function
                } else {
                    EscapeState::NoEscape
                }
            } else {
                EscapeState::NoEscape
            }
        }
    }
}

/// Checks whether an expression is a direct reference to the named variable.
///
/// Returns true only for `Ident { name }` or `Grouped { Ident { name } }`.
/// Compound expressions (calls, binaries) that *use* the variable are not
/// direct references.
fn is_direct_var_ref(var_name: &str, expr: &Expr) -> bool {
    match expr {
        Expr::Ident { name, .. } => name == var_name,
        Expr::Grouped { expr, .. } => is_direct_var_ref(var_name, expr),
        _ => false,
    }
}

/// Checks whether an expression references a variable by name.
fn references_var(var_name: &str, expr: &Expr) -> bool {
    match expr {
        Expr::Ident { name, .. } => name == var_name,
        Expr::Binary { left, right, .. } | Expr::Pipe { left, right, .. } => {
            references_var(var_name, left) || references_var(var_name, right)
        }
        Expr::Unary { operand, .. } => references_var(var_name, operand),
        Expr::Call { callee, args, .. } => {
            references_var(var_name, callee)
                || args.iter().any(|a| references_var(var_name, &a.value))
        }
        Expr::MethodCall { receiver, args, .. } => {
            references_var(var_name, receiver)
                || args.iter().any(|a| references_var(var_name, &a.value))
        }
        Expr::Block { stmts, expr, .. } => {
            stmts.iter().any(|s| match s {
                Stmt::Let { value, .. }
                | Stmt::Const { value, .. }
                | Stmt::Expr { expr: value, .. } => references_var(var_name, value),
                Stmt::Return { value: Some(v), .. } | Stmt::Break { value: Some(v), .. } => {
                    references_var(var_name, v)
                }
                _ => false,
            }) || expr.as_ref().is_some_and(|e| references_var(var_name, e))
        }
        Expr::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            references_var(var_name, condition)
                || references_var(var_name, then_branch)
                || else_branch
                    .as_ref()
                    .is_some_and(|e| references_var(var_name, e))
        }
        Expr::While {
            condition, body, ..
        } => references_var(var_name, condition) || references_var(var_name, body),
        Expr::For { iterable, body, .. } => {
            references_var(var_name, iterable) || references_var(var_name, body)
        }
        Expr::Loop { body, .. } => references_var(var_name, body),
        Expr::Assign { target, value, .. } => {
            references_var(var_name, target) || references_var(var_name, value)
        }
        Expr::Index { object, index, .. } => {
            references_var(var_name, object) || references_var(var_name, index)
        }
        Expr::Field { object, .. } => references_var(var_name, object),
        Expr::Array { elements, .. } | Expr::Tuple { elements, .. } => {
            elements.iter().any(|e| references_var(var_name, e))
        }
        Expr::Grouped { expr, .. }
        | Expr::Cast { expr, .. }
        | Expr::Try { expr, .. }
        | Expr::Await { expr, .. } => references_var(var_name, expr),
        Expr::Closure { body, .. }
        | Expr::AsyncBlock { body, .. }
        | Expr::Comptime { body, .. } => references_var(var_name, body),
        Expr::StructInit { fields, .. } => {
            fields.iter().any(|f| references_var(var_name, &f.value))
        }
        Expr::Match { subject, arms, .. } => {
            references_var(var_name, subject)
                || arms.iter().any(|arm| references_var(var_name, &arm.body))
        }
        Expr::ArrayRepeat { value, count, .. } => {
            references_var(var_name, value) || references_var(var_name, count)
        }
        Expr::HandleEffect { body, handlers, .. } => {
            references_var(var_name, body)
                || handlers.iter().any(|h| references_var(var_name, &h.body))
        }
        Expr::ResumeExpr { value, .. } => references_var(var_name, value),
        Expr::MacroInvocation { args, .. } => args.iter().any(|a| references_var(var_name, a)),
        _ => false,
    }
}

/// Merges two escape states, returning the more pessimistic (wider) one.
fn merge_escape(a: EscapeState, b: EscapeState) -> EscapeState {
    match (a, b) {
        (EscapeState::GlobalEscape, _) | (_, EscapeState::GlobalEscape) => {
            EscapeState::GlobalEscape
        }
        (EscapeState::ArgEscape, _) | (_, EscapeState::ArgEscape) => EscapeState::ArgEscape,
        _ => EscapeState::NoEscape,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// OPT2.15 — Copy Propagation
// ═══════════════════════════════════════════════════════════════════════

/// A detected copy propagation opportunity.
///
/// When `let to = from` and `from` is not reassigned, references to `to`
/// can be replaced with `from`, eliminating the copy.
#[derive(Debug, Clone)]
pub struct CopyProp {
    /// Source variable name.
    pub from: String,
    /// Destination variable name (the copy).
    pub to: String,
    /// Approximate source location (byte offset).
    pub line: usize,
}

/// Finds copy propagation opportunities in a program.
///
/// A copy propagation is possible when:
/// 1. A `let` binding directly copies another variable: `let y = x`
/// 2. The source variable `x` is not mutated after the copy
///
/// In these cases, uses of `y` can be replaced with `x` directly.
pub fn find_copy_propagations(program: &Program) -> Vec<CopyProp> {
    let mut copies = Vec::new();
    for item in &program.items {
        if let Item::FnDef(fndef) = item {
            find_copies_in_expr(&fndef.body, &mut copies);
        }
    }
    copies
}

/// Recursively finds copy propagation opportunities in an expression.
fn find_copies_in_expr(expr: &Expr, copies: &mut Vec<CopyProp>) {
    match expr {
        Expr::Block { stmts, expr, .. } => {
            // Collect all mutations in this block
            let mut mutated: HashSet<String> = HashSet::new();
            collect_mutations(expr.as_deref(), stmts, &mut mutated);

            // Find let bindings that are simple copies of immutable variables
            for stmt in stmts {
                if let Stmt::Let {
                    mutable: false,
                    name,
                    value,
                    span,
                    ..
                } = stmt
                {
                    if let Expr::Ident {
                        name: source_name, ..
                    } = value.as_ref()
                    {
                        // Only propagate if the source is not mutated
                        if !mutated.contains(source_name) {
                            copies.push(CopyProp {
                                from: source_name.clone(),
                                to: name.clone(),
                                line: span.start,
                            });
                        }
                    }
                }
                // Recurse into statement expressions
                match stmt {
                    Stmt::Let { value, .. }
                    | Stmt::Const { value, .. }
                    | Stmt::Expr { expr: value, .. } => {
                        find_copies_in_expr(value, copies);
                    }
                    Stmt::Return { value: Some(v), .. } => find_copies_in_expr(v, copies),
                    _ => {}
                }
            }
            if let Some(e) = expr {
                find_copies_in_expr(e, copies);
            }
        }
        Expr::If {
            then_branch,
            else_branch,
            ..
        } => {
            find_copies_in_expr(then_branch, copies);
            if let Some(eb) = else_branch {
                find_copies_in_expr(eb, copies);
            }
        }
        Expr::While { body, .. } | Expr::For { body, .. } | Expr::Loop { body, .. } => {
            find_copies_in_expr(body, copies);
        }
        _ => {}
    }
}

/// Collects the set of variable names that are mutated (assigned to) in a block.
fn collect_mutations(tail: Option<&Expr>, stmts: &[Stmt], mutated: &mut HashSet<String>) {
    for stmt in stmts {
        match stmt {
            Stmt::Expr { expr, .. } | Stmt::Let { value: expr, .. } => {
                collect_mutations_in_expr(expr, mutated);
            }
            Stmt::Return { value: Some(v), .. } => collect_mutations_in_expr(v, mutated),
            _ => {}
        }
    }
    if let Some(e) = tail {
        collect_mutations_in_expr(e, mutated);
    }
}

/// Recursively collects mutation targets in an expression.
fn collect_mutations_in_expr(expr: &Expr, mutated: &mut HashSet<String>) {
    match expr {
        Expr::Assign { target, value, .. } => {
            if let Expr::Ident { name, .. } = target.as_ref() {
                mutated.insert(name.clone());
            }
            collect_mutations_in_expr(value, mutated);
        }
        Expr::Block { stmts, expr, .. } => {
            collect_mutations(expr.as_deref(), stmts, mutated);
        }
        Expr::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            collect_mutations_in_expr(condition, mutated);
            collect_mutations_in_expr(then_branch, mutated);
            if let Some(eb) = else_branch {
                collect_mutations_in_expr(eb, mutated);
            }
        }
        Expr::While {
            condition, body, ..
        } => {
            collect_mutations_in_expr(condition, mutated);
            collect_mutations_in_expr(body, mutated);
        }
        Expr::For { body, .. } | Expr::Loop { body, .. } => {
            collect_mutations_in_expr(body, mutated);
        }
        Expr::Call { args, .. } => {
            for arg in args {
                collect_mutations_in_expr(&arg.value, mutated);
            }
        }
        Expr::Binary { left, right, .. } => {
            collect_mutations_in_expr(left, mutated);
            collect_mutations_in_expr(right, mutated);
        }
        _ => {}
    }
}

// ═══════════════════════════════════════════════════════════════════════
// OPT2.17 — Optimization Pipeline
// ═══════════════════════════════════════════════════════════════════════

/// Optimization level controlling which passes are enabled.
///
/// Higher levels apply more aggressive optimizations at the cost of
/// increased compile time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OptLevel {
    /// No optimization (fastest compile, debugging-friendly).
    #[default]
    O0,
    /// Basic optimization: constant folding, copy propagation, DCE.
    O1,
    /// Standard optimization: O1 + strength reduction, LICM, CSE, inlining.
    O2,
    /// Aggressive optimization: O2 + loop unrolling, escape analysis, devirtualization.
    O3,
    /// Size optimization: O2 minus unrolling, plus string dedup and function merging.
    Os,
}

/// The optimization pipeline that orchestrates all passes.
#[derive(Debug, Clone)]
pub struct OptPipeline {
    /// Selected optimization level.
    pub level: OptLevel,
    /// Ordered list of pass names that will be run.
    pub passes: Vec<&'static str>,
}

impl OptPipeline {
    /// Creates a new pipeline for the given optimization level.
    ///
    /// Pass selection by level:
    /// - **O0**: No passes
    /// - **O1**: constant_fold, copy_prop, dce
    /// - **O2**: O1 + strength_reduce, licm, cse, inline
    /// - **O3**: O2 + unroll, escape_analysis, devirtualize, vectorize
    /// - **Os**: O2 - unroll + string_dedup + fn_merge
    pub fn new(level: OptLevel) -> Self {
        let passes = match level {
            OptLevel::O0 => vec![],
            OptLevel::O1 => vec!["constant_fold", "copy_prop", "dce"],
            OptLevel::O2 => vec![
                "constant_fold",
                "copy_prop",
                "dce",
                "strength_reduce",
                "licm",
                "cse",
                "inline",
            ],
            OptLevel::O3 => vec![
                "constant_fold",
                "copy_prop",
                "dce",
                "strength_reduce",
                "licm",
                "cse",
                "inline",
                "unroll",
                "escape_analysis",
                "devirtualize",
                "vectorize",
            ],
            OptLevel::Os => vec![
                "constant_fold",
                "copy_prop",
                "dce",
                "strength_reduce",
                "licm",
                "cse",
                "inline",
                "string_dedup",
                "fn_merge",
            ],
        };
        Self { level, passes }
    }

    /// Runs the pipeline on a program and returns an optimization report.
    ///
    /// Each pass analyzes the program and collects statistics. The report
    /// summarizes what was found and the estimated impact.
    pub fn run(&self, program: &Program) -> OptReport {
        let mut report = OptReport {
            passes_run: Vec::new(),
            optimizations_applied: 0,
            estimated_speedup: 1.0,
        };

        for pass in &self.passes {
            let count = run_pass(pass, program);
            report.passes_run.push(pass.to_string());
            report.optimizations_applied += count;
        }

        // Estimate speedup based on optimizations applied
        report.estimated_speedup = estimate_speedup(self.level, report.optimizations_applied);

        report
    }
}

/// Runs a single named optimization pass and returns the number of opportunities found.
fn run_pass(pass_name: &str, program: &Program) -> usize {
    match pass_name {
        "constant_fold" => count_constant_folds(program),
        "copy_prop" => find_copy_propagations(program).len(),
        "dce" => find_dead_functions(program).len(),
        "strength_reduce" => find_strength_reductions(program).len(),
        "unroll" => find_unroll_candidates(program).len(),
        "escape_analysis" => count_escape_analysis(program),
        "string_dedup" => {
            let result = dedup_strings(program);
            result.original_count - result.unique_count
        }
        "inline" => count_inline_opportunities(program),
        "licm" => count_licm_opportunities(program),
        "cse" => count_cse_opportunities(program),
        "devirtualize" => count_devirtualize_opportunities(program),
        "vectorize" => count_vectorize_opportunities(program),
        "fn_merge" => count_fn_merge_candidates(program),
        _ => 0,
    }
}

/// Counts how many functions have small bodies suitable for inlining.
fn count_inline_opportunities(program: &Program) -> usize {
    let threshold = 20;
    let mut count = 0;
    for item in &program.items {
        if let Item::FnDef(fndef) = item {
            if fndef.name != "main" && estimate_body_cost(&fndef.body) <= threshold {
                count += 1;
            }
        }
    }
    count
}

// ═══════════════════════════════════════════════════════════════════════
// LICM — Loop-Invariant Code Motion
// ═══════════════════════════════════════════════════════════════════════

/// Counts expressions in loop bodies that could be hoisted out (loop-invariant).
///
/// An expression is loop-invariant when it is a pure computation (no calls,
/// no assignments, no index writes) and every variable it reads is not
/// modified inside the loop body.  Such expressions compute the same value
/// on every iteration and can be moved before the loop.
fn count_licm_opportunities(program: &Program) -> usize {
    let mut count = 0;
    for item in &program.items {
        if let Item::FnDef(fndef) = item {
            count_licm_in_expr(&fndef.body, &mut count);
        }
    }
    count
}

/// Recursively walks expressions, counting LICM opportunities inside loops.
fn count_licm_in_expr(expr: &Expr, count: &mut usize) {
    match expr {
        Expr::For {
            variable,
            iterable,
            body,
            ..
        } => {
            // Collect names written inside the loop body (loop-variant set).
            let mut written: HashSet<String> = HashSet::new();
            written.insert(variable.clone()); // the loop variable itself is variant
            collect_written_names(body, &mut written);
            collect_written_names(iterable, &mut written);

            // Count sub-expressions of the body that are loop-invariant.
            *count += count_invariant_exprs(body, &written);

            // Recurse into nested loops.
            count_licm_in_expr(body, count);
        }
        Expr::While {
            condition, body, ..
        } => {
            let mut written: HashSet<String> = HashSet::new();
            collect_written_names(body, &mut written);
            collect_written_names(condition, &mut written);

            *count += count_invariant_exprs(body, &written);
            count_licm_in_expr(body, count);
        }
        Expr::Loop { body, .. } => {
            let mut written: HashSet<String> = HashSet::new();
            collect_written_names(body, &mut written);

            *count += count_invariant_exprs(body, &written);
            count_licm_in_expr(body, count);
        }
        Expr::Block { stmts, expr, .. } => {
            for stmt in stmts {
                match stmt {
                    Stmt::Let { value, .. }
                    | Stmt::Const { value, .. }
                    | Stmt::Expr { expr: value, .. } => count_licm_in_expr(value, count),
                    _ => {}
                }
            }
            if let Some(e) = expr {
                count_licm_in_expr(e, count);
            }
        }
        Expr::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            count_licm_in_expr(condition, count);
            count_licm_in_expr(then_branch, count);
            if let Some(eb) = else_branch {
                count_licm_in_expr(eb, count);
            }
        }
        _ => {}
    }
}

/// Collects all variable names that are assigned (written) inside an expression tree.
fn collect_written_names(expr: &Expr, written: &mut HashSet<String>) {
    match expr {
        Expr::Assign { target, value, .. } => {
            if let Expr::Ident { name, .. } = target.as_ref() {
                written.insert(name.clone());
            }
            collect_written_names(value, written);
        }
        Expr::Block { stmts, expr, .. } => {
            for stmt in stmts {
                match stmt {
                    Stmt::Let { name, value, .. } | Stmt::Const { name, value, .. } => {
                        written.insert(name.clone());
                        collect_written_names(value, written);
                    }
                    Stmt::Expr { expr, .. } => collect_written_names(expr, written),
                    _ => {}
                }
            }
            if let Some(e) = expr {
                collect_written_names(e, written);
            }
        }
        Expr::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            collect_written_names(condition, written);
            collect_written_names(then_branch, written);
            if let Some(eb) = else_branch {
                collect_written_names(eb, written);
            }
        }
        Expr::While {
            condition, body, ..
        }
        | Expr::For {
            iterable: condition,
            body,
            ..
        } => {
            collect_written_names(condition, written);
            collect_written_names(body, written);
        }
        Expr::Loop { body, .. } => collect_written_names(body, written),
        Expr::Binary { left, right, .. } | Expr::Pipe { left, right, .. } => {
            collect_written_names(left, written);
            collect_written_names(right, written);
        }
        Expr::Unary { operand, .. } => collect_written_names(operand, written),
        _ => {}
    }
}

/// Returns true when an expression is "pure" — no calls, no assignments, no side effects.
fn is_pure_expr(expr: &Expr) -> bool {
    match expr {
        Expr::Literal { .. } | Expr::Ident { .. } => true,
        Expr::Binary { left, right, .. } | Expr::Pipe { left, right, .. } => {
            is_pure_expr(left) && is_pure_expr(right)
        }
        Expr::Unary { operand, .. } => is_pure_expr(operand),
        Expr::Grouped { expr, .. } | Expr::Cast { expr, .. } => is_pure_expr(expr),
        Expr::Field { object, .. } => is_pure_expr(object),
        Expr::Index { object, index, .. } => is_pure_expr(object) && is_pure_expr(index),
        // Calls and assignments are never pure
        _ => false,
    }
}

/// Counts top-level sub-expressions inside a loop body that are loop-invariant.
///
/// An expression qualifies when it is pure and reads no variable in `written`.
fn count_invariant_exprs(expr: &Expr, written: &HashSet<String>) -> usize {
    /// Returns true when `expr` reads a variable that is in `written`.
    fn reads_written(expr: &Expr, written: &HashSet<String>) -> bool {
        match expr {
            Expr::Ident { name, .. } => written.contains(name.as_str()),
            Expr::Binary { left, right, .. } | Expr::Pipe { left, right, .. } => {
                reads_written(left, written) || reads_written(right, written)
            }
            Expr::Unary { operand, .. } => reads_written(operand, written),
            Expr::Grouped { expr, .. } | Expr::Cast { expr, .. } => reads_written(expr, written),
            Expr::Field { object, .. } => reads_written(object, written),
            Expr::Index { object, index, .. } => {
                reads_written(object, written) || reads_written(index, written)
            }
            _ => false,
        }
    }

    let mut count = 0;
    // Only examine direct children of the block, not deeply nested loops
    if let Expr::Block { stmts, expr, .. } = expr {
        for stmt in stmts {
            let val_expr: Option<&Expr> = match stmt {
                Stmt::Let { value, .. } | Stmt::Const { value, .. } => Some(value),
                Stmt::Expr { expr, .. } => Some(expr),
                _ => None,
            };
            if let Some(e) = val_expr {
                if is_pure_expr(e) && !reads_written(e, written) {
                    count += 1;
                }
            }
        }
        if let Some(e) = expr {
            if is_pure_expr(e) && !reads_written(e, written) {
                count += 1;
            }
        }
    }
    count
}

// ═══════════════════════════════════════════════════════════════════════
// CSE — Common Subexpression Elimination
// ═══════════════════════════════════════════════════════════════════════

/// Counts duplicate sub-expressions inside each function body.
///
/// Two expressions are "identical" when they have the same structural
/// fingerprint (same operator tree + same leaf values/names).  For each
/// pair of identical expressions found in the same function scope, one
/// occurrence could be eliminated.
fn count_cse_opportunities(program: &Program) -> usize {
    let mut count = 0;
    for item in &program.items {
        if let Item::FnDef(fndef) = item {
            count += count_cse_in_expr(&fndef.body);
        }
    }
    count
}

/// Collects expression fingerprints from a block body and counts duplicates.
fn count_cse_in_expr(expr: &Expr) -> usize {
    let mut fingerprints: Vec<String> = Vec::new();
    collect_expr_fingerprints(expr, &mut fingerprints);

    // Count fingerprints that appear more than once
    let mut freq: HashMap<&str, usize> = HashMap::new();
    for fp in &fingerprints {
        *freq.entry(fp.as_str()).or_insert(0) += 1;
    }
    freq.values().filter(|&&c| c > 1).count()
}

/// Produces a canonical string fingerprint for an expression.
///
/// Only pure, non-trivial expressions (binary operations, unary on non-literal)
/// are fingerprinted — literals and bare identifiers are excluded because
/// they are already "free" to duplicate.
fn expr_fingerprint(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Binary {
            left, op, right, ..
        } => {
            let l = expr_fingerprint(left).unwrap_or_else(|| leaf_fp(left));
            let r = expr_fingerprint(right).unwrap_or_else(|| leaf_fp(right));
            Some(format!("({} {:?} {})", l, op, r))
        }
        Expr::Unary { op, operand, .. } => {
            // Only fingerprint unary on a non-literal operand
            if matches!(operand.as_ref(), Expr::Literal { .. }) {
                None
            } else {
                let inner = expr_fingerprint(operand).unwrap_or_else(|| leaf_fp(operand));
                Some(format!("({:?} {})", op, inner))
            }
        }
        Expr::Field { object, field, .. } => {
            let obj = expr_fingerprint(object).unwrap_or_else(|| leaf_fp(object));
            Some(format!("{}.{}", obj, field))
        }
        Expr::Index { object, index, .. } => {
            let obj = expr_fingerprint(object).unwrap_or_else(|| leaf_fp(object));
            let idx = expr_fingerprint(index).unwrap_or_else(|| leaf_fp(index));
            Some(format!("{}[{}]", obj, idx))
        }
        Expr::Grouped { expr, .. } => expr_fingerprint(expr),
        Expr::Cast { expr, .. } => expr_fingerprint(expr),
        _ => None,
    }
}

/// Returns a simple leaf string for identifiers and literals (used by `expr_fingerprint`).
fn leaf_fp(expr: &Expr) -> String {
    match expr {
        Expr::Ident { name, .. } => name.clone(),
        Expr::Literal { kind, .. } => format!("{:?}", kind),
        _ => "_".to_string(),
    }
}

/// Walks an expression tree and pushes fingerprints for every non-trivial sub-expression.
fn collect_expr_fingerprints(expr: &Expr, fps: &mut Vec<String>) {
    if let Some(fp) = expr_fingerprint(expr) {
        fps.push(fp);
    }
    // Recurse into children
    match expr {
        Expr::Binary { left, right, .. } | Expr::Pipe { left, right, .. } => {
            collect_expr_fingerprints(left, fps);
            collect_expr_fingerprints(right, fps);
        }
        Expr::Unary { operand, .. } => collect_expr_fingerprints(operand, fps),
        Expr::Block { stmts, expr, .. } => {
            for stmt in stmts {
                match stmt {
                    Stmt::Let { value, .. }
                    | Stmt::Const { value, .. }
                    | Stmt::Expr { expr: value, .. } => collect_expr_fingerprints(value, fps),
                    _ => {}
                }
            }
            if let Some(e) = expr {
                collect_expr_fingerprints(e, fps);
            }
        }
        Expr::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            collect_expr_fingerprints(condition, fps);
            collect_expr_fingerprints(then_branch, fps);
            if let Some(eb) = else_branch {
                collect_expr_fingerprints(eb, fps);
            }
        }
        Expr::For { iterable, body, .. } => {
            collect_expr_fingerprints(iterable, fps);
            collect_expr_fingerprints(body, fps);
        }
        Expr::While {
            condition, body, ..
        } => {
            collect_expr_fingerprints(condition, fps);
            collect_expr_fingerprints(body, fps);
        }
        Expr::Loop { body, .. } => collect_expr_fingerprints(body, fps),
        Expr::Assign { target, value, .. } => {
            collect_expr_fingerprints(target, fps);
            collect_expr_fingerprints(value, fps);
        }
        Expr::Field { object, .. } => collect_expr_fingerprints(object, fps),
        Expr::Index { object, index, .. } => {
            collect_expr_fingerprints(object, fps);
            collect_expr_fingerprints(index, fps);
        }
        Expr::Grouped { expr, .. } | Expr::Cast { expr, .. } | Expr::Try { expr, .. } => {
            collect_expr_fingerprints(expr, fps)
        }
        _ => {}
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Devirtualize — Direct dispatch for concrete-type method calls
// ═══════════════════════════════════════════════════════════════════════

/// Counts method calls that could be devirtualized because the receiver's
/// concrete struct type is known at the call site.
///
/// A call site qualifies when the receiver is an identifier that was bound
/// via a `StructInit` expression (concrete type, not a trait object).
fn count_devirtualize_opportunities(program: &Program) -> usize {
    let mut count = 0;
    for item in &program.items {
        if let Item::FnDef(fndef) = item {
            count += count_devirt_in_expr(&fndef.body, &HashMap::new());
        }
    }
    count
}

/// Walks the expression tree, tracking which variables are bound to known struct
/// types and counting method calls on those variables.
fn count_devirt_in_expr(expr: &Expr, known: &HashMap<String, String>) -> usize {
    match expr {
        Expr::MethodCall { receiver, args, .. } => {
            // Count this call if receiver is a concrete-typed variable
            let is_concrete = match receiver.as_ref() {
                Expr::Ident { name, .. } => known.contains_key(name.as_str()),
                _ => false,
            };
            let self_count = if is_concrete { 1 } else { 0 };
            let arg_count: usize = args
                .iter()
                .map(|a| count_devirt_in_expr(&a.value, known))
                .sum();
            self_count + count_devirt_in_expr(receiver, known) + arg_count
        }
        Expr::Block { stmts, expr, .. } => {
            // Build a new scope that inherits and extends `known`
            let mut local: HashMap<String, String> = known.clone();
            let mut count = 0;
            for stmt in stmts {
                match stmt {
                    Stmt::Let { name, value, .. } | Stmt::Const { name, value, .. } => {
                        // Track struct-init bindings
                        if let Expr::StructInit {
                            name: struct_name, ..
                        } = value.as_ref()
                        {
                            local.insert(name.clone(), struct_name.clone());
                        }
                        count += count_devirt_in_expr(value, &local);
                    }
                    Stmt::Expr { expr, .. } => count += count_devirt_in_expr(expr, &local),
                    _ => {}
                }
            }
            if let Some(e) = expr {
                count += count_devirt_in_expr(e, &local);
            }
            count
        }
        Expr::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            let mut c =
                count_devirt_in_expr(condition, known) + count_devirt_in_expr(then_branch, known);
            if let Some(eb) = else_branch {
                c += count_devirt_in_expr(eb, known);
            }
            c
        }
        Expr::For { iterable, body, .. }
        | Expr::While {
            condition: iterable,
            body,
            ..
        } => count_devirt_in_expr(iterable, known) + count_devirt_in_expr(body, known),
        Expr::Loop { body, .. } => count_devirt_in_expr(body, known),
        Expr::Binary { left, right, .. } | Expr::Pipe { left, right, .. } => {
            count_devirt_in_expr(left, known) + count_devirt_in_expr(right, known)
        }
        Expr::Unary { operand, .. } => count_devirt_in_expr(operand, known),
        Expr::Grouped { expr, .. } | Expr::Cast { expr, .. } | Expr::Try { expr, .. } => {
            count_devirt_in_expr(expr, known)
        }
        _ => 0,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Vectorize — Element-wise loop detection
// ═══════════════════════════════════════════════════════════════════════

/// Counts for-loops that iterate over a range and perform only element-wise
/// array operations (no loop-carried dependencies), making them candidates
/// for auto-vectorization (SIMD).
///
/// Pattern: `for i in 0..n { a[i] = b[i] op c[i] }` where `i` appears only
/// as an index and every iteration is independent.
fn count_vectorize_opportunities(program: &Program) -> usize {
    let mut count = 0;
    for item in &program.items {
        if let Item::FnDef(fndef) = item {
            count_vectorize_in_expr(&fndef.body, &mut count);
        }
    }
    count
}

/// Recursively searches for vectorizable loops.
fn count_vectorize_in_expr(expr: &Expr, count: &mut usize) {
    match expr {
        Expr::For {
            variable,
            iterable,
            body,
            ..
        } => {
            // Only consider range-iterated loops
            let is_range = matches!(iterable.as_ref(), Expr::Range { .. });
            if is_range && is_elementwise_body(body, variable) {
                *count += 1;
            }
            // Recurse into nested loops
            count_vectorize_in_expr(body, count);
        }
        Expr::Block { stmts, expr, .. } => {
            for stmt in stmts {
                match stmt {
                    Stmt::Let { value, .. }
                    | Stmt::Const { value, .. }
                    | Stmt::Expr { expr: value, .. } => count_vectorize_in_expr(value, count),
                    _ => {}
                }
            }
            if let Some(e) = expr {
                count_vectorize_in_expr(e, count);
            }
        }
        Expr::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            count_vectorize_in_expr(condition, count);
            count_vectorize_in_expr(then_branch, count);
            if let Some(eb) = else_branch {
                count_vectorize_in_expr(eb, count);
            }
        }
        Expr::While {
            condition, body, ..
        } => {
            count_vectorize_in_expr(condition, count);
            count_vectorize_in_expr(body, count);
        }
        Expr::Loop { body, .. } => {
            count_vectorize_in_expr(body, count);
        }
        _ => {}
    }
}

/// Returns true when a loop body consists only of element-wise operations.
///
/// "Element-wise" means: the loop variable `var` is used only as an array
/// index (inside `[]`), and the body is a flat block with no nested loops,
/// no conditionals, and no calls that could carry state across iterations.
fn is_elementwise_body(body: &Expr, var: &str) -> bool {
    match body {
        Expr::Block { stmts, expr, .. } => {
            // Every statement must be an assignment `arr[var] = expr`
            // or an expression-statement that is an element-wise operation
            let stmts_ok = stmts.iter().all(|s| match s {
                Stmt::Expr { expr, .. } => is_elementwise_expr(expr, var),
                Stmt::Let { value, .. } => is_elementwise_expr(value, var),
                _ => false,
            });
            let tail_ok = expr.as_ref().is_none_or(|e| is_elementwise_expr(e, var));
            stmts_ok && tail_ok
        }
        // A single expression body
        _ => is_elementwise_expr(body, var),
    }
}

/// Returns true when `expr` is an element-wise expression over the loop variable.
fn is_elementwise_expr(expr: &Expr, var: &str) -> bool {
    match expr {
        // `arr[var] = rhs` — assignment to indexed slot
        Expr::Assign { target, value, .. } => {
            is_index_of_var(target, var) && is_elementwise_expr(value, var)
        }
        // `arr[var]` — reading an indexed slot
        Expr::Index { .. } => is_index_of_var(expr, var),
        // Binary op on two elementwise sub-expressions
        Expr::Binary { left, right, .. } => {
            is_elementwise_expr(left, var) && is_elementwise_expr(right, var)
        }
        // Grouping / cast is transparent
        Expr::Grouped { expr, .. } | Expr::Cast { expr, .. } => is_elementwise_expr(expr, var),
        // Literals and non-loop identifiers are fine (broadcast scalars)
        Expr::Literal { .. } => true,
        Expr::Ident { name, .. } => name != var, // raw `var` outside index = dependency
        _ => false,
    }
}

/// Returns true when `expr` is of the form `something[var]`.
fn is_index_of_var(expr: &Expr, var: &str) -> bool {
    if let Expr::Index { index, .. } = expr {
        matches!(index.as_ref(), Expr::Ident { name, .. } if name == var)
    } else {
        false
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Function Merge — Structurally identical function detection
// ═══════════════════════════════════════════════════════════════════════

/// Counts functions in a program that have structurally identical bodies and
/// could be merged (one kept, others replaced with a call to the canonical copy).
///
/// Two functions are structurally identical when they have:
/// - The same number of parameters
/// - Bodies with the same statement-type sequence and expression-kind tree
///   (variable *names* are ignored — only shape matters)
fn count_fn_merge_candidates(program: &Program) -> usize {
    let fndefs: Vec<&FnDef> = program
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

    if fndefs.len() < 2 {
        return 0;
    }

    // Compute a structural signature for each function
    let sigs: Vec<String> = fndefs.iter().map(|f| fn_structure_sig(f)).collect();

    // Count how many functions share a signature with at least one other
    let mut freq: HashMap<&str, usize> = HashMap::new();
    for sig in &sigs {
        *freq.entry(sig.as_str()).or_insert(0) += 1;
    }

    // Every function beyond the first in a group is a merge candidate
    freq.values().filter(|&&c| c > 1).map(|&c| c - 1).sum()
}

/// Builds a structural signature for a function.
///
/// The signature captures parameter count, return type presence, and the
/// expression-shape of the body — ignoring concrete names so that two
/// functions that differ only in variable/field names produce the same string.
fn fn_structure_sig(fndef: &FnDef) -> String {
    let param_count = fndef.params.len();
    let has_ret = if fndef.return_type.is_some() {
        "R"
    } else {
        "V"
    };
    let body_sig = expr_shape_sig(&fndef.body);
    format!("{}:{}:{}", param_count, has_ret, body_sig)
}

/// Returns a shape-only string for an expression (no names, no literal values).
fn expr_shape_sig(expr: &Expr) -> String {
    match expr {
        Expr::Literal { .. } => "L".to_string(),
        Expr::Ident { .. } => "I".to_string(),
        Expr::Binary {
            left, op, right, ..
        } => {
            format!(
                "B({:?},{},{})",
                op,
                expr_shape_sig(left),
                expr_shape_sig(right)
            )
        }
        Expr::Unary { op, operand, .. } => {
            format!("U({:?},{})", op, expr_shape_sig(operand))
        }
        Expr::Call { args, .. } => {
            let arg_sigs: Vec<String> = args.iter().map(|a| expr_shape_sig(&a.value)).collect();
            format!("C({})", arg_sigs.join(","))
        }
        Expr::MethodCall { args, .. } => {
            let arg_sigs: Vec<String> = args.iter().map(|a| expr_shape_sig(&a.value)).collect();
            format!("M({})", arg_sigs.join(","))
        }
        Expr::Block { stmts, expr, .. } => {
            let stmt_sigs: Vec<String> = stmts.iter().map(stmt_shape_sig).collect();
            let tail = expr.as_ref().map_or("_".to_string(), |e| expr_shape_sig(e));
            format!("{{{}|{}}}", stmt_sigs.join(";"), tail)
        }
        Expr::If {
            then_branch,
            else_branch,
            ..
        } => {
            let else_sig = else_branch
                .as_ref()
                .map_or("_".to_string(), |e| expr_shape_sig(e));
            format!("IF({},{})", expr_shape_sig(then_branch), else_sig)
        }
        Expr::For { body, .. } => format!("FOR({})", expr_shape_sig(body)),
        Expr::While { body, .. } | Expr::Loop { body, .. } => {
            format!("WH({})", expr_shape_sig(body))
        }
        Expr::Assign { value, .. } => format!("ASS({})", expr_shape_sig(value)),
        Expr::Index { object, index, .. } => {
            format!("IDX({},{})", expr_shape_sig(object), expr_shape_sig(index))
        }
        Expr::Field { .. } => "FLD".to_string(),
        Expr::Grouped { expr, .. } | Expr::Cast { expr, .. } | Expr::Try { expr, .. } => {
            expr_shape_sig(expr)
        }
        _ => "?".to_string(),
    }
}

/// Returns the shape signature of a statement.
fn stmt_shape_sig(stmt: &Stmt) -> String {
    match stmt {
        Stmt::Let { value, .. } | Stmt::Const { value, .. } => {
            format!("LET({})", expr_shape_sig(value))
        }
        Stmt::Expr { expr, .. } => format!("EXPR({})", expr_shape_sig(expr)),
        Stmt::Return { value, .. } => {
            let v = value
                .as_ref()
                .map_or("_".to_string(), |e| expr_shape_sig(e));
            format!("RET({})", v)
        }
        Stmt::Break { .. } | Stmt::Continue { .. } => "BC".to_string(),
        Stmt::Item(_) => "ITEM".to_string(),
    }
}

/// Counts variables that do not escape in any function (useful for stack allocation).
fn count_escape_analysis(program: &Program) -> usize {
    let mut no_escape = 0;
    for item in &program.items {
        if let Item::FnDef(fndef) = item {
            let vars = collect_local_vars(&fndef.body);
            for var in &vars {
                if analyze_escape(&fndef.name, var, program) == EscapeState::NoEscape {
                    no_escape += 1;
                }
            }
        }
    }
    no_escape
}

/// Collects local variable names defined in an expression.
fn collect_local_vars(expr: &Expr) -> Vec<String> {
    let mut vars = Vec::new();
    collect_local_vars_inner(expr, &mut vars);
    vars
}

/// Recursively collects local variable names.
fn collect_local_vars_inner(expr: &Expr, vars: &mut Vec<String>) {
    if let Expr::Block { stmts, expr, .. } = expr {
        for stmt in stmts {
            if let Stmt::Let { name, .. } = stmt {
                vars.push(name.clone());
            }
            match stmt {
                Stmt::Let { value, .. }
                | Stmt::Const { value, .. }
                | Stmt::Expr { expr: value, .. } => {
                    collect_local_vars_inner(value, vars);
                }
                _ => {}
            }
        }
        if let Some(e) = expr {
            collect_local_vars_inner(e, vars);
        }
    }
}

/// Estimates a speed-up factor based on optimization level and count.
fn estimate_speedup(level: OptLevel, optimizations: usize) -> f64 {
    if optimizations == 0 {
        return 1.0;
    }
    let base = match level {
        OptLevel::O0 => 1.0,
        OptLevel::O1 => 1.05,
        OptLevel::O2 => 1.15,
        OptLevel::O3 => 1.25,
        OptLevel::Os => 1.10,
    };
    // Diminishing returns: each optimization adds less
    base + (optimizations as f64).ln() * 0.02
}

/// Report produced by running the optimization pipeline.
#[derive(Debug, Clone)]
pub struct OptReport {
    /// Names of passes that were executed.
    pub passes_run: Vec<String>,
    /// Total number of optimization opportunities found.
    pub optimizations_applied: usize,
    /// Estimated speed-up factor (1.0 = no change, 1.2 = 20% faster).
    pub estimated_speedup: f64,
}

impl Default for OptReport {
    fn default() -> Self {
        Self {
            passes_run: Vec::new(),
            optimizations_applied: 0,
            estimated_speedup: 1.0,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// OPT3.1 — Dead Function Elimination
// ═══════════════════════════════════════════════════════════════════════

/// Finds functions in a program that are never called and can be eliminated.
///
/// Entry points (reachable roots) include:
/// - `main` function
/// - `pub` functions (may be called externally)
/// - Test functions (`@test`)
///
/// All other functions must be transitively reachable from an entry point;
/// otherwise they are considered dead.
pub fn find_dead_functions(program: &Program) -> Vec<String> {
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

    if functions.is_empty() {
        return Vec::new();
    }

    // Entry points: main, pub, test, kernel, annotated entry points
    let mut reachable: HashSet<String> = HashSet::new();
    let mut worklist: Vec<String> = functions
        .iter()
        .filter(|f| {
            // Always keep: main, public, test functions
            f.name == "main"
                || f.is_pub
                || f.is_test
                // K2: Bare-metal entry points (called from asm/linker, not .fj code)
                || f.name == "kernel_main"
                || f.name == "_start"
                || f.name == "fj_exception_sync"
                || f.name == "fj_exception_irq"
                // K2: Annotated entry points
                || f.annotation
                    .as_ref()
                    .is_some_and(|a| {
                        a.name == "entry"
                            || a.name == "panic_handler"
                            || a.name == "kernel"
                            || a.name == "unsafe"
                    })
        })
        .map(|f| f.name.clone())
        .collect();

    // BFS through call graph
    while let Some(fn_name) = worklist.pop() {
        if !reachable.insert(fn_name.clone()) {
            continue;
        }
        if let Some(fndef) = functions.iter().find(|f| f.name == fn_name) {
            let callees = collect_all_callees(&fndef.body);
            for callee in callees {
                if !reachable.contains(&callee) {
                    worklist.push(callee);
                }
            }
        }
    }

    // Unreachable functions
    functions
        .iter()
        .filter(|f| !reachable.contains(&f.name))
        .map(|f| f.name.clone())
        .collect()
}

/// Collects all function names called within an expression tree.
fn collect_all_callees(expr: &Expr) -> Vec<String> {
    let mut callees = Vec::new();
    collect_callees_recursive(expr, &mut callees);
    callees
}

/// Recursively collects callee names.
fn collect_callees_recursive(expr: &Expr, callees: &mut Vec<String>) {
    match expr {
        Expr::Call { callee, args, .. } => {
            if let Expr::Ident { name, .. } = callee.as_ref() {
                if !callees.contains(name) {
                    callees.push(name.clone());
                }
            }
            // Also check if callee is a path (module::function)
            if let Expr::Path { segments, .. } = callee.as_ref() {
                if let Some(last) = segments.last() {
                    if !callees.contains(last) {
                        callees.push(last.clone());
                    }
                }
            }
            for arg in args {
                collect_callees_recursive(&arg.value, callees);
            }
        }
        Expr::MethodCall { receiver, args, .. } => {
            collect_callees_recursive(receiver, callees);
            for arg in args {
                collect_callees_recursive(&arg.value, callees);
            }
        }
        Expr::Block { stmts, expr, .. } => {
            for stmt in stmts {
                match stmt {
                    Stmt::Let { value, .. }
                    | Stmt::Const { value, .. }
                    | Stmt::Expr { expr: value, .. } => {
                        collect_callees_recursive(value, callees);
                    }
                    Stmt::Return { value: Some(v), .. } => collect_callees_recursive(v, callees),
                    _ => {}
                }
            }
            if let Some(e) = expr {
                collect_callees_recursive(e, callees);
            }
        }
        Expr::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            collect_callees_recursive(condition, callees);
            collect_callees_recursive(then_branch, callees);
            if let Some(eb) = else_branch {
                collect_callees_recursive(eb, callees);
            }
        }
        Expr::While {
            condition, body, ..
        } => {
            collect_callees_recursive(condition, callees);
            collect_callees_recursive(body, callees);
        }
        Expr::For { iterable, body, .. } => {
            collect_callees_recursive(iterable, callees);
            collect_callees_recursive(body, callees);
        }
        Expr::Loop { body, .. } => collect_callees_recursive(body, callees),
        Expr::Binary { left, right, .. } | Expr::Pipe { left, right, .. } => {
            collect_callees_recursive(left, callees);
            collect_callees_recursive(right, callees);
        }
        Expr::Unary { operand, .. } => collect_callees_recursive(operand, callees),
        Expr::Assign { value, .. } => collect_callees_recursive(value, callees),
        Expr::Grouped { expr, .. }
        | Expr::Cast { expr, .. }
        | Expr::Try { expr, .. }
        | Expr::Await { expr, .. } => {
            collect_callees_recursive(expr, callees);
        }
        Expr::Closure { body, .. }
        | Expr::AsyncBlock { body, .. }
        | Expr::Comptime { body, .. } => {
            collect_callees_recursive(body, callees);
        }
        Expr::Match { subject, arms, .. } => {
            collect_callees_recursive(subject, callees);
            for arm in arms {
                collect_callees_recursive(&arm.body, callees);
            }
        }
        Expr::Array { elements, .. } | Expr::Tuple { elements, .. } => {
            for e in elements {
                collect_callees_recursive(e, callees);
            }
        }
        Expr::Index { object, index, .. } => {
            collect_callees_recursive(object, callees);
            collect_callees_recursive(index, callees);
        }
        Expr::Field { object, .. } => collect_callees_recursive(object, callees),
        Expr::StructInit { fields, .. } => {
            for f in fields {
                collect_callees_recursive(&f.value, callees);
            }
        }
        Expr::HandleEffect { body, handlers, .. } => {
            collect_callees_recursive(body, callees);
            for h in handlers {
                collect_callees_recursive(&h.body, callees);
            }
        }
        Expr::ResumeExpr { value, .. } => collect_callees_recursive(value, callees),
        Expr::ArrayRepeat { value, count, .. } => {
            collect_callees_recursive(value, callees);
            collect_callees_recursive(count, callees);
        }
        Expr::MacroInvocation { args, .. } => {
            for a in args {
                collect_callees_recursive(a, callees);
            }
        }
        _ => {}
    }
}

// ═══════════════════════════════════════════════════════════════════════
// OPT3.3 — String Deduplication
// ═══════════════════════════════════════════════════════════════════════

/// Result of string deduplication analysis.
#[derive(Debug, Clone)]
pub struct DedupResult {
    /// Total number of string literal occurrences in the program.
    pub original_count: usize,
    /// Number of unique string values after deduplication.
    pub unique_count: usize,
    /// Estimated bytes saved by sharing deduplicated strings.
    pub bytes_saved: usize,
}

/// Analyzes a program for string literal deduplication opportunities.
///
/// Scans all string literals in the AST, counts duplicates, and estimates
/// memory savings from sharing identical strings at runtime.
pub fn dedup_strings(program: &Program) -> DedupResult {
    let mut all_strings: Vec<String> = Vec::new();
    for item in &program.items {
        collect_string_literals_in_item(item, &mut all_strings);
    }

    let original_count = all_strings.len();
    let mut unique: HashMap<&str, usize> = HashMap::new();
    for s in &all_strings {
        *unique.entry(s.as_str()).or_insert(0) += 1;
    }

    let unique_count = unique.len();

    // Estimate savings: for each duplicate, we save the string length + overhead
    let bytes_saved: usize = unique
        .iter()
        .filter(|(_, count)| **count > 1)
        .map(|(s, count)| (*count - 1) * (s.len() + 16)) // 16 bytes for pointer + length metadata
        .sum();

    DedupResult {
        original_count,
        unique_count,
        bytes_saved,
    }
}

/// Collects string literals from an AST item.
fn collect_string_literals_in_item(item: &Item, strings: &mut Vec<String>) {
    match item {
        Item::FnDef(fndef) => collect_string_literals_in_expr(&fndef.body, strings),
        Item::Stmt(stmt) => collect_string_literals_in_stmt(stmt, strings),
        Item::ImplBlock(imp) => {
            for method in &imp.methods {
                collect_string_literals_in_expr(&method.body, strings);
            }
        }
        Item::ConstDef(c) => collect_string_literals_in_expr(&c.value, strings),
        Item::StaticDef(s) => collect_string_literals_in_expr(&s.value, strings),
        Item::ModDecl(m) => {
            if let Some(body) = &m.body {
                for sub in body {
                    collect_string_literals_in_item(sub, strings);
                }
            }
        }
        _ => {}
    }
}

/// Collects string literals from an expression.
fn collect_string_literals_in_expr(expr: &Expr, strings: &mut Vec<String>) {
    match expr {
        Expr::Literal {
            kind: LiteralKind::String(s) | LiteralKind::RawString(s),
            ..
        } => {
            strings.push(s.clone());
        }
        Expr::Binary { left, right, .. } | Expr::Pipe { left, right, .. } => {
            collect_string_literals_in_expr(left, strings);
            collect_string_literals_in_expr(right, strings);
        }
        Expr::Unary { operand, .. } => collect_string_literals_in_expr(operand, strings),
        Expr::Call { callee, args, .. } => {
            collect_string_literals_in_expr(callee, strings);
            for arg in args {
                collect_string_literals_in_expr(&arg.value, strings);
            }
        }
        Expr::Block { stmts, expr, .. } => {
            for stmt in stmts {
                collect_string_literals_in_stmt(stmt, strings);
            }
            if let Some(e) = expr {
                collect_string_literals_in_expr(e, strings);
            }
        }
        Expr::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            collect_string_literals_in_expr(condition, strings);
            collect_string_literals_in_expr(then_branch, strings);
            if let Some(eb) = else_branch {
                collect_string_literals_in_expr(eb, strings);
            }
        }
        Expr::While {
            condition, body, ..
        } => {
            collect_string_literals_in_expr(condition, strings);
            collect_string_literals_in_expr(body, strings);
        }
        Expr::For { iterable, body, .. } => {
            collect_string_literals_in_expr(iterable, strings);
            collect_string_literals_in_expr(body, strings);
        }
        Expr::Loop { body, .. } => collect_string_literals_in_expr(body, strings),
        Expr::Match { subject, arms, .. } => {
            collect_string_literals_in_expr(subject, strings);
            for arm in arms {
                collect_string_literals_in_expr(&arm.body, strings);
            }
        }
        Expr::Array { elements, .. } | Expr::Tuple { elements, .. } => {
            for e in elements {
                collect_string_literals_in_expr(e, strings);
            }
        }
        Expr::Grouped { expr, .. }
        | Expr::Cast { expr, .. }
        | Expr::Try { expr, .. }
        | Expr::Await { expr, .. } => {
            collect_string_literals_in_expr(expr, strings);
        }
        Expr::Closure { body, .. }
        | Expr::AsyncBlock { body, .. }
        | Expr::Comptime { body, .. } => {
            collect_string_literals_in_expr(body, strings);
        }
        Expr::Assign { target, value, .. } => {
            collect_string_literals_in_expr(target, strings);
            collect_string_literals_in_expr(value, strings);
        }
        Expr::StructInit { fields, .. } => {
            for f in fields {
                collect_string_literals_in_expr(&f.value, strings);
            }
        }
        Expr::Index { object, index, .. } => {
            collect_string_literals_in_expr(object, strings);
            collect_string_literals_in_expr(index, strings);
        }
        Expr::Field { object, .. } => collect_string_literals_in_expr(object, strings),
        Expr::MethodCall { receiver, args, .. } => {
            collect_string_literals_in_expr(receiver, strings);
            for arg in args {
                collect_string_literals_in_expr(&arg.value, strings);
            }
        }
        Expr::FString { parts, .. } => {
            for part in parts {
                match part {
                    crate::parser::ast::FStringExprPart::Literal(s) => {
                        strings.push(s.clone());
                    }
                    crate::parser::ast::FStringExprPart::Expr(e) => {
                        collect_string_literals_in_expr(e, strings);
                    }
                }
            }
        }
        Expr::HandleEffect { body, handlers, .. } => {
            collect_string_literals_in_expr(body, strings);
            for h in handlers {
                collect_string_literals_in_expr(&h.body, strings);
            }
        }
        Expr::ResumeExpr { value, .. } => collect_string_literals_in_expr(value, strings),
        Expr::ArrayRepeat { value, count, .. } => {
            collect_string_literals_in_expr(value, strings);
            collect_string_literals_in_expr(count, strings);
        }
        Expr::MacroInvocation { args, .. } => {
            for a in args {
                collect_string_literals_in_expr(a, strings);
            }
        }
        _ => {}
    }
}

/// Collects string literals from a statement.
fn collect_string_literals_in_stmt(stmt: &Stmt, strings: &mut Vec<String>) {
    match stmt {
        Stmt::Let { value, .. } | Stmt::Const { value, .. } | Stmt::Expr { expr: value, .. } => {
            collect_string_literals_in_expr(value, strings);
        }
        Stmt::Return { value: Some(v), .. } | Stmt::Break { value: Some(v), .. } => {
            collect_string_literals_in_expr(v, strings);
        }
        Stmt::Return { value: None, .. }
        | Stmt::Break { value: None, .. }
        | Stmt::Continue { .. } => {}
        Stmt::Item(item) => collect_string_literals_in_item(item, strings),
    }
}

// ═══════════════════════════════════════════════════════════════════════
// OPT3.11 — Size Profiling
// ═══════════════════════════════════════════════════════════════════════

/// Per-function size estimate for binary size profiling.
#[derive(Debug, Clone)]
pub struct FnSizeEntry {
    /// Function name.
    pub name: String,
    /// Estimated compiled size in bytes.
    pub estimated_bytes: usize,
    /// Percentage of total estimated program size.
    pub percentage: f64,
}

/// Estimates per-function sizes for a program.
///
/// Each function's compiled size is approximated based on the number of AST
/// nodes in its body, using an average of 8 bytes per node as a heuristic
/// for native machine code.
///
/// Results are sorted by size in descending order.
pub fn profile_function_sizes(program: &Program) -> Vec<FnSizeEntry> {
    let functions: Vec<(&str, usize)> = program
        .items
        .iter()
        .filter_map(|item| {
            if let Item::FnDef(fndef) = item {
                let cost = estimate_body_cost(&fndef.body);
                // Heuristic: ~8 bytes of machine code per AST node
                let bytes = cost * 8;
                Some((fndef.name.as_str(), bytes))
            } else {
                None
            }
        })
        .collect();

    let total: usize = functions.iter().map(|(_, b)| b).sum();
    let total_f64 = if total == 0 { 1.0 } else { total as f64 };

    let mut entries: Vec<FnSizeEntry> = functions
        .into_iter()
        .map(|(name, bytes)| FnSizeEntry {
            name: name.to_string(),
            estimated_bytes: bytes,
            percentage: (bytes as f64 / total_f64) * 100.0,
        })
        .collect();

    // Sort by size descending
    entries.sort_by_key(|e| std::cmp::Reverse(e.estimated_bytes));
    entries
}

// ═══════════════════════════════════════════════════════════════════════
// OPT3.28 — Optimization Report Generation
// ═══════════════════════════════════════════════════════════════════════

/// Counts constant-foldable expressions in a program.
fn count_constant_folds(program: &Program) -> usize {
    let mut count = 0;
    for item in &program.items {
        if let Item::FnDef(fndef) = item {
            count += count_foldable_in_expr(&fndef.body);
        }
    }
    count
}

/// Recursively counts foldable expressions.
fn count_foldable_in_expr(expr: &Expr) -> usize {
    match expr {
        Expr::Binary { left, right, .. } => {
            let l_const = matches!(left.as_ref(), Expr::Literal { .. });
            let r_const = matches!(right.as_ref(), Expr::Literal { .. });
            let self_fold = if l_const && r_const { 1 } else { 0 };
            self_fold + count_foldable_in_expr(left) + count_foldable_in_expr(right)
        }
        Expr::Unary { operand, .. } => {
            let self_fold = if matches!(operand.as_ref(), Expr::Literal { .. }) {
                1
            } else {
                0
            };
            self_fold + count_foldable_in_expr(operand)
        }
        Expr::Block { stmts, expr, .. } => {
            let mut count = 0;
            for stmt in stmts {
                match stmt {
                    Stmt::Expr { expr, .. }
                    | Stmt::Let { value: expr, .. }
                    | Stmt::Const { value: expr, .. } => {
                        count += count_foldable_in_expr(expr);
                    }
                    _ => {}
                }
            }
            if let Some(e) = expr {
                count += count_foldable_in_expr(e);
            }
            count
        }
        Expr::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            let mut count = count_foldable_in_expr(condition) + count_foldable_in_expr(then_branch);
            if let Some(eb) = else_branch {
                count += count_foldable_in_expr(eb);
            }
            count
        }
        Expr::Comptime { .. } => 1,
        _ => 0,
    }
}

/// Generates a comprehensive optimization report in Markdown format.
///
/// The report includes sections for each optimization pass, listing what was
/// found and the estimated impact.
pub fn generate_opt_report(program: &Program, level: OptLevel) -> String {
    let pipeline = OptPipeline::new(level);
    let opt_report = pipeline.run(program);

    let mut lines = Vec::new();
    lines.push("# Optimization Report".to_string());
    lines.push(String::new());
    lines.push(format!("**Optimization Level:** {:?}", level));
    lines.push(format!(
        "**Passes Run:** {}",
        opt_report.passes_run.join(", ")
    ));
    lines.push(format!(
        "**Total Optimizations:** {}",
        opt_report.optimizations_applied
    ));
    lines.push(format!(
        "**Estimated Speedup:** {:.2}x",
        opt_report.estimated_speedup
    ));
    lines.push(String::new());

    // Constant folding details
    let folds = count_constant_folds(program);
    lines.push("## Constant Folding".to_string());
    lines.push(format!("- Foldable expressions: {}", folds));
    lines.push(String::new());

    // Dead function elimination
    let dead = find_dead_functions(program);
    lines.push("## Dead Function Elimination".to_string());
    lines.push(format!("- Dead functions: {}", dead.len()));
    for name in &dead {
        lines.push(format!("  - `{}`", name));
    }
    lines.push(String::new());

    // Strength reduction
    let reductions = find_strength_reductions(program);
    lines.push("## Strength Reduction".to_string());
    lines.push(format!("- Opportunities: {}", reductions.len()));
    for sr in &reductions {
        lines.push(format!(
            "  - {} -> {} ({})",
            sr.original, sr.replacement, sr.savings
        ));
    }
    lines.push(String::new());

    // Copy propagation
    let copies = find_copy_propagations(program);
    lines.push("## Copy Propagation".to_string());
    lines.push(format!("- Propagatable copies: {}", copies.len()));
    for cp in &copies {
        lines.push(format!(
            "  - `{}` = `{}` (offset {})",
            cp.to, cp.from, cp.line
        ));
    }
    lines.push(String::new());

    // Loop unrolling
    let unrolls = find_unroll_candidates(program);
    lines.push("## Loop Unrolling".to_string());
    lines.push(format!("- Candidates: {}", unrolls.len()));
    for u in &unrolls {
        lines.push(format!(
            "  - offset {}: {} iters, body cost {}, factor {}x",
            u.loop_line, u.trip_count, u.body_cost, u.recommended_factor
        ));
    }
    lines.push(String::new());

    // String deduplication
    let dedup = dedup_strings(program);
    lines.push("## String Deduplication".to_string());
    lines.push(format!("- Total strings: {}", dedup.original_count));
    lines.push(format!("- Unique strings: {}", dedup.unique_count));
    lines.push(format!("- Bytes saved: {}", dedup.bytes_saved));
    lines.push(String::new());

    // Size profile
    let sizes = profile_function_sizes(program);
    lines.push("## Function Size Profile".to_string());
    for entry in sizes.iter().take(10) {
        lines.push(format!(
            "- `{}`: ~{} bytes ({:.1}%)",
            entry.name, entry.estimated_bytes, entry.percentage
        ));
    }
    lines.push(String::new());

    // String interning
    let interner = StringInterner::from_program(program);
    lines.push("## String Interning".to_string());
    lines.push(format!("- Unique interned strings: {}", interner.len()));
    lines.push(String::new());

    lines.join("\n")
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::tokenize;
    use crate::parser::parse;

    /// Helper to parse Fajar Lang source into a Program.
    fn parse_program(source: &str) -> Program {
        let tokens = tokenize(source).expect("tokenize failed");
        parse(tokens).expect("parse failed")
    }

    // ── String Interning ──────────────────────────────────────────────

    #[test]
    fn string_interner_intern_and_resolve() {
        let mut interner = StringInterner::new();
        let id = interner.intern("hello");
        assert_eq!(interner.resolve(id), "hello");
    }

    #[test]
    fn string_interner_dedup() {
        let mut interner = StringInterner::new();
        let id1 = interner.intern("hello");
        let id2 = interner.intern("hello");
        assert_eq!(id1, id2);
        assert_eq!(interner.len(), 1);
    }

    #[test]
    fn string_interner_multiple_strings() {
        let mut interner = StringInterner::new();
        let a = interner.intern("alpha");
        let b = interner.intern("beta");
        let c = interner.intern("alpha");
        assert_eq!(a, c);
        assert_ne!(a, b);
        assert_eq!(interner.len(), 2);
    }

    #[test]
    fn string_interner_empty() {
        let interner = StringInterner::new();
        assert!(interner.is_empty());
        assert_eq!(interner.len(), 0);
    }

    #[test]
    fn string_interner_contains() {
        let mut interner = StringInterner::new();
        interner.intern("test");
        assert!(interner.contains("test"));
        assert!(!interner.contains("other"));
    }

    #[test]
    fn string_interner_from_program() {
        let program = parse_program("fn main() { \"hello\" }");
        let interner = StringInterner::from_program(&program);
        assert!(interner.contains("hello"));
    }

    // ── Compilation Metrics ──────────────────────────────────────────

    #[test]
    fn compilation_metrics_new_zeroed() {
        let m = CompilationMetrics::new();
        assert_eq!(m.lex_time_us, 0);
        assert_eq!(m.total_time_us, 0);
        assert_eq!(m.bottleneck(), "none");
    }

    #[test]
    fn compilation_metrics_record_phase() {
        let mut m = CompilationMetrics::new();
        m.record_phase("lex", 100);
        m.record_phase("parse", 200);
        assert_eq!(m.lex_time_us, 100);
        assert_eq!(m.parse_time_us, 200);
        assert_eq!(m.total_time_us, 300);
    }

    #[test]
    fn compilation_metrics_bottleneck() {
        let mut m = CompilationMetrics::new();
        m.record_phase("lex", 50);
        m.record_phase("codegen", 500);
        m.record_phase("parse", 100);
        assert_eq!(m.bottleneck(), "codegen");
    }

    #[test]
    fn compilation_metrics_report_format() {
        let mut m = CompilationMetrics::new();
        m.record_phase("lex", 100);
        m.functions_compiled = 5;
        let report = m.report();
        assert!(report.contains("Compilation Metrics:"));
        assert!(report.contains("100"));
        assert!(report.contains("functions:"));
    }

    // ── Constant Folding ──────────────────────────────────────────────

    #[test]
    fn constant_fold_int_add() {
        let program = parse_program("fn main() { 2 + 3 }");
        if let Item::FnDef(fndef) = &program.items[0] {
            // body is Block with tail expr = Binary(2+3)
            if let Expr::Block { expr: Some(e), .. } = fndef.body.as_ref() {
                let folded = constant_fold(e).expect("should fold");
                assert!(matches!(
                    folded,
                    Expr::Literal {
                        kind: LiteralKind::Int(5),
                        ..
                    }
                ));
            }
        }
    }

    #[test]
    fn constant_fold_int_mul() {
        let program = parse_program("fn main() { 4 * 5 }");
        if let Item::FnDef(fndef) = &program.items[0] {
            if let Expr::Block { expr: Some(e), .. } = fndef.body.as_ref() {
                let folded = constant_fold(e).expect("should fold");
                assert!(matches!(
                    folded,
                    Expr::Literal {
                        kind: LiteralKind::Int(20),
                        ..
                    }
                ));
            }
        }
    }

    #[test]
    fn constant_fold_float_add() {
        let program = parse_program("fn main() { 1.5 + 2.5 }");
        if let Item::FnDef(fndef) = &program.items[0] {
            if let Expr::Block { expr: Some(e), .. } = fndef.body.as_ref() {
                let folded = constant_fold(e).expect("should fold");
                if let Expr::Literal {
                    kind: LiteralKind::Float(v),
                    ..
                } = folded
                {
                    assert!((v - 4.0).abs() < 1e-10);
                } else {
                    panic!("expected float literal");
                }
            }
        }
    }

    #[test]
    fn constant_fold_bool_and() {
        let program = parse_program("fn main() { true && false }");
        if let Item::FnDef(fndef) = &program.items[0] {
            if let Expr::Block { expr: Some(e), .. } = fndef.body.as_ref() {
                let folded = constant_fold(e).expect("should fold");
                assert!(matches!(
                    folded,
                    Expr::Literal {
                        kind: LiteralKind::Bool(false),
                        ..
                    }
                ));
            }
        }
    }

    #[test]
    fn constant_fold_string_concat() {
        let program = parse_program(r#"fn main() { "hello" + " world" }"#);
        if let Item::FnDef(fndef) = &program.items[0] {
            if let Expr::Block { expr: Some(e), .. } = fndef.body.as_ref() {
                let folded = constant_fold(e).expect("should fold");
                if let Expr::Literal {
                    kind: LiteralKind::String(s),
                    ..
                } = folded
                {
                    assert_eq!(s, "hello world");
                } else {
                    panic!("expected string literal");
                }
            }
        }
    }

    #[test]
    fn constant_fold_unary_neg() {
        let program = parse_program("fn main() { -42 }");
        if let Item::FnDef(fndef) = &program.items[0] {
            if let Expr::Block { expr: Some(e), .. } = fndef.body.as_ref() {
                let folded = constant_fold(e).expect("should fold");
                assert!(matches!(
                    folded,
                    Expr::Literal {
                        kind: LiteralKind::Int(-42),
                        ..
                    }
                ));
            }
        }
    }

    #[test]
    fn constant_fold_not_bool() {
        let program = parse_program("fn main() { !true }");
        if let Item::FnDef(fndef) = &program.items[0] {
            if let Expr::Block { expr: Some(e), .. } = fndef.body.as_ref() {
                let folded = constant_fold(e).expect("should fold");
                assert!(matches!(
                    folded,
                    Expr::Literal {
                        kind: LiteralKind::Bool(false),
                        ..
                    }
                ));
            }
        }
    }

    #[test]
    fn constant_fold_div_by_zero_returns_none() {
        let program = parse_program("fn main() { 10 / 0 }");
        if let Item::FnDef(fndef) = &program.items[0] {
            if let Expr::Block { expr: Some(e), .. } = fndef.body.as_ref() {
                assert!(constant_fold(e).is_none());
            }
        }
    }

    #[test]
    fn constant_fold_non_const_returns_none() {
        let program = parse_program("fn main() { x + 3 }");
        if let Item::FnDef(fndef) = &program.items[0] {
            if let Expr::Block { expr: Some(e), .. } = fndef.body.as_ref() {
                assert!(constant_fold(e).is_none());
            }
        }
    }

    // ── Loop Unrolling ──────────────────────────────────────────────

    #[test]
    fn unroll_finds_simple_range_loop() {
        let program = parse_program("fn main() { for i in 0..8 { i } }");
        let candidates = find_unroll_candidates(&program);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].trip_count, 8);
    }

    #[test]
    fn unroll_no_candidates_for_runtime_range() {
        let program = parse_program("fn main() { for i in 0..n { i } }");
        let candidates = find_unroll_candidates(&program);
        assert_eq!(candidates.len(), 0);
    }

    #[test]
    fn unroll_factor_for_small_loop() {
        let program = parse_program("fn main() { for i in 0..4 { i } }");
        let candidates = find_unroll_candidates(&program);
        assert_eq!(candidates.len(), 1);
        // Small loop with trip count 4 => full unroll (factor 4)
        assert_eq!(candidates[0].recommended_factor, 4);
    }

    // ── Strength Reduction ──────────────────────────────────────────

    #[test]
    fn strength_reduce_mul_to_shift() {
        let program = parse_program("fn main() { x * 2 }");
        let reductions = find_strength_reductions(&program);
        assert!(!reductions.is_empty());
        assert!(reductions.iter().any(|r| r.savings == "mul-to-shift"));
    }

    #[test]
    fn strength_reduce_div_to_shift() {
        let program = parse_program("fn main() { x / 4 }");
        let reductions = find_strength_reductions(&program);
        assert!(reductions.iter().any(|r| r.savings == "div-to-shift"));
    }

    #[test]
    fn strength_reduce_mod_to_and() {
        let program = parse_program("fn main() { x % 2 }");
        let reductions = find_strength_reductions(&program);
        assert!(reductions.iter().any(|r| r.savings == "mod-to-and"));
    }

    #[test]
    fn strength_reduce_mul_by_3_no_match() {
        let program = parse_program("fn main() { x * 3 }");
        let reductions = find_strength_reductions(&program);
        // 3 is not a power of 2, so no mul-to-shift
        assert!(!reductions.iter().any(|r| r.savings == "mul-to-shift"));
    }

    // ── Escape Analysis ──────────────────────────────────────────────

    #[test]
    fn escape_no_escape_local_only() {
        let program = parse_program("fn foo() { let x = 42 }");
        let state = analyze_escape("foo", "x", &program);
        assert_eq!(state, EscapeState::NoEscape);
    }

    #[test]
    fn escape_arg_escape_passed_to_fn() {
        let program = parse_program("fn foo() { let x = 42\nbar(x) }");
        let state = analyze_escape("foo", "x", &program);
        assert_eq!(state, EscapeState::ArgEscape);
    }

    #[test]
    fn escape_global_escape_returned() {
        let program = parse_program("fn foo() -> i64 { let x = 42\nreturn x }");
        let state = analyze_escape("foo", "x", &program);
        assert_eq!(state, EscapeState::GlobalEscape);
    }

    // ── Copy Propagation ──────────────────────────────────────────────

    #[test]
    fn copy_prop_simple_copy() {
        let program = parse_program("fn main() { let x = 42\nlet y = x\ny }");
        let copies = find_copy_propagations(&program);
        assert!(!copies.is_empty());
        assert!(copies.iter().any(|c| c.from == "x" && c.to == "y"));
    }

    #[test]
    fn copy_prop_no_copy_for_non_ident() {
        let program = parse_program("fn main() { let x = 2 + 3 }");
        let copies = find_copy_propagations(&program);
        assert!(copies.is_empty());
    }

    // ── Optimization Pipeline ──────────────────────────────────────────

    #[test]
    fn pipeline_o0_no_passes() {
        let pipeline = OptPipeline::new(OptLevel::O0);
        assert!(pipeline.passes.is_empty());
    }

    #[test]
    fn pipeline_o1_has_basic_passes() {
        let pipeline = OptPipeline::new(OptLevel::O1);
        assert!(pipeline.passes.contains(&"constant_fold"));
        assert!(pipeline.passes.contains(&"copy_prop"));
        assert!(pipeline.passes.contains(&"dce"));
    }

    #[test]
    fn pipeline_o2_has_more_passes() {
        let pipeline = OptPipeline::new(OptLevel::O2);
        assert!(pipeline.passes.contains(&"strength_reduce"));
        assert!(pipeline.passes.contains(&"inline"));
    }

    #[test]
    fn pipeline_o3_has_aggressive_passes() {
        let pipeline = OptPipeline::new(OptLevel::O3);
        assert!(pipeline.passes.contains(&"unroll"));
        assert!(pipeline.passes.contains(&"escape_analysis"));
    }

    #[test]
    fn pipeline_os_has_size_passes() {
        let pipeline = OptPipeline::new(OptLevel::Os);
        assert!(pipeline.passes.contains(&"string_dedup"));
        assert!(pipeline.passes.contains(&"fn_merge"));
        assert!(!pipeline.passes.contains(&"unroll"));
    }

    #[test]
    fn pipeline_run_produces_report() {
        let program = parse_program("fn main() { 2 + 3 }");
        let pipeline = OptPipeline::new(OptLevel::O1);
        let report = pipeline.run(&program);
        assert!(!report.passes_run.is_empty());
        assert!(report.estimated_speedup >= 1.0);
    }

    // ── Dead Function Elimination ──────────────────────────────────────

    #[test]
    fn dead_fn_detects_unreachable() {
        let program = parse_program("fn used() { 1 }\nfn dead() { 2 }\nfn main() { used() }");
        let dead = find_dead_functions(&program);
        assert!(dead.contains(&"dead".to_string()));
        assert!(!dead.contains(&"used".to_string()));
        assert!(!dead.contains(&"main".to_string()));
    }

    #[test]
    fn dead_fn_no_dead_when_all_reachable() {
        let program = parse_program("fn helper() { 1 }\nfn main() { helper() }");
        let dead = find_dead_functions(&program);
        assert!(dead.is_empty());
    }

    #[test]
    fn dead_fn_pub_functions_are_roots() {
        let program = parse_program("pub fn api() { 1 }\nfn internal() { 2 }");
        let dead = find_dead_functions(&program);
        assert!(!dead.contains(&"api".to_string()));
        assert!(dead.contains(&"internal".to_string()));
    }

    // ── String Deduplication ──────────────────────────────────────────

    #[test]
    fn dedup_strings_finds_duplicates() {
        let program = parse_program(r#"fn main() { "hello" }"#);
        let result = dedup_strings(&program);
        assert_eq!(result.original_count, 1);
        assert_eq!(result.unique_count, 1);
        assert_eq!(result.bytes_saved, 0);
    }

    #[test]
    fn dedup_strings_counts_savings() {
        let program = parse_program(r#"fn main() { let a = "dup"; let b = "dup"; let c = "dup" }"#);
        let result = dedup_strings(&program);
        assert_eq!(result.original_count, 3);
        assert_eq!(result.unique_count, 1);
        assert!(result.bytes_saved > 0);
    }

    // ── Size Profiling ──────────────────────────────────────────────

    #[test]
    fn size_profile_estimates_function_sizes() {
        let program = parse_program("fn small() { 1 }\nfn big() { 1 + 2 + 3 + 4 + 5 }");
        let sizes = profile_function_sizes(&program);
        assert_eq!(sizes.len(), 2);
        // big should be larger than small
        let big = sizes
            .iter()
            .find(|s| s.name == "big")
            .expect("big not found");
        let small = sizes
            .iter()
            .find(|s| s.name == "small")
            .expect("small not found");
        assert!(big.estimated_bytes >= small.estimated_bytes);
    }

    #[test]
    fn size_profile_percentages_sum_to_100() {
        let program = parse_program("fn a() { 1 }\nfn b() { 2 }\nfn c() { 3 }");
        let sizes = profile_function_sizes(&program);
        let total_pct: f64 = sizes.iter().map(|s| s.percentage).sum();
        assert!((total_pct - 100.0).abs() < 0.1);
    }

    // ── Optimization Report ──────────────────────────────────────────

    #[test]
    fn opt_report_generates_markdown() {
        let program = parse_program(
            r#"fn used() { 1 }
fn dead() { 2 }
fn main() { used() }"#,
        );
        let report = generate_opt_report(&program, OptLevel::O2);
        assert!(report.contains("# Optimization Report"));
        assert!(report.contains("Dead Function Elimination"));
        assert!(report.contains("dead"));
    }

    #[test]
    fn opt_report_o0_minimal() {
        let program = parse_program("fn main() { 42 }");
        let report = generate_opt_report(&program, OptLevel::O0);
        assert!(report.contains("O0"));
    }

    // ── LICM ──────────────────────────────────────────────────────────

    #[test]
    fn test_licm_finds_invariant_in_loop() {
        // `2 + 3` inside the for-loop body does not depend on `i`,
        // so it should be detected as a hoistable expression.
        let program = parse_program("fn f() { for i in 0..10 { let x = 2 + 3 } }");
        let count = count_licm_opportunities(&program);
        assert!(
            count > 0,
            "expected at least 1 LICM opportunity, got {}",
            count
        );
    }

    #[test]
    fn test_licm_no_opportunity_when_variant() {
        // `i + 1` depends on the loop variable `i` — not invariant.
        let program = parse_program("fn f() { for i in 0..10 { let x = i + 1 } }");
        let count = count_licm_opportunities(&program);
        assert_eq!(count, 0, "i+1 is NOT loop-invariant");
    }

    // ── CSE ───────────────────────────────────────────────────────────

    #[test]
    fn test_cse_finds_duplicate_exprs() {
        // `a + b` appears twice → 1 group with count 2 → 1 CSE opportunity.
        let program =
            parse_program("fn f(a: i32, b: i32) -> i32 { let x = a + b\n let y = a + b\n x + y }");
        let count = count_cse_opportunities(&program);
        assert!(
            count > 0,
            "expected CSE opportunity for duplicate a+b, got {}",
            count
        );
    }

    #[test]
    fn test_cse_no_duplicates() {
        // All sub-expressions are unique.
        let program =
            parse_program("fn f(a: i32, b: i32) -> i32 { let x = a + b\n let y = a - b\n x }");
        let count = count_cse_opportunities(&program);
        assert_eq!(count, 0, "no duplicate expressions expected");
    }

    // ── Devirtualize ──────────────────────────────────────────────────

    #[test]
    fn test_devirtualize_known_type() {
        // `p` is bound to `Point { ... }` — a concrete struct type.
        // The call `p.area()` can be devirtualized.
        let program = parse_program("fn f() { let p = Point { x: 1, y: 2 }\n p.area() }");
        let count = count_devirtualize_opportunities(&program);
        assert!(
            count > 0,
            "expected devirtualize opportunity, got {}",
            count
        );
    }

    #[test]
    fn test_devirtualize_unknown_type() {
        // `x` is bound to a literal, not a struct — no method to devirtualize.
        let program = parse_program("fn f() { let x = 42\n x }");
        let count = count_devirtualize_opportunities(&program);
        assert_eq!(count, 0);
    }

    // ── Vectorize ─────────────────────────────────────────────────────

    #[test]
    fn test_vectorize_elementwise_loop() {
        // Classic SIMD pattern: `a[i] = b[i] + c[i]`
        let program = parse_program("fn f() { for i in 0..n { a[i] = b[i] + c[i] } }");
        let count = count_vectorize_opportunities(&program);
        assert!(count > 0, "expected vectorize opportunity, got {}", count);
    }

    #[test]
    fn test_vectorize_no_opportunity_non_elementwise() {
        // Loop body uses `i` directly (not as an index) — not vectorizable.
        let program = parse_program("fn f() { for i in 0..10 { let x = i * 2 } }");
        let count = count_vectorize_opportunities(&program);
        assert_eq!(
            count, 0,
            "scalar loop should not be flagged as vectorizable"
        );
    }

    // ── Function Merge ────────────────────────────────────────────────

    #[test]
    fn test_fn_merge_identical_bodies() {
        // Two functions with the same structure (return a+b / return x+y).
        let program = parse_program(
            "fn add_i(a: i32, b: i32) -> i32 { a + b }\nfn add_f(x: f64, y: f64) -> f64 { x + y }",
        );
        let count = count_fn_merge_candidates(&program);
        assert!(count > 0, "expected 1 merge candidate, got {}", count);
    }

    #[test]
    fn test_fn_merge_different_structures() {
        // Completely different bodies — no merge candidates.
        let program =
            parse_program("fn foo(a: i32) -> i32 { a + 1 }\nfn bar(a: i32) -> i32 { a * 2 }");
        let count = count_fn_merge_candidates(&program);
        assert_eq!(
            count, 0,
            "structurally different functions should not be merged"
        );
    }
}
