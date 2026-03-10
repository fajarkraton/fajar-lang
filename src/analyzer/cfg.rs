//! Non-Lexical Lifetime (NLL) analysis for borrow checking.
//!
//! Computes variable liveness information so borrows end at their
//! last use point rather than at scope boundaries.
//!
//! # Algorithm
//!
//! 1. Pre-pass: walk the function body AST and record every variable use
//!    with its source position (Span).
//! 2. For loops: extend variable uses inside the loop body to the loop's
//!    end position (conservative — variables used in a loop are live for
//!    the whole loop).
//! 3. Result: for each variable, the maximum source position where it's used.
//!
//! During type checking, before each statement, borrows whose binding
//! variable's last use position is before the current statement are released.

use std::collections::{HashMap, HashSet};

use crate::parser::ast::*;

/// Result of NLL pre-analysis for a function body.
///
/// Maps each variable name to the furthest source position where it's used.
/// This allows the borrow checker to release borrows at last-use rather than
/// at scope exit.
#[derive(Debug)]
pub struct NllInfo {
    /// For each variable name: the maximum source position (span.end) where it's used.
    last_use_pos: HashMap<String, usize>,
}

impl NllInfo {
    /// Analyzes a function body and computes NLL liveness information.
    pub fn analyze(body: &Expr) -> Self {
        let mut collector = UseCollector::new();
        collector.visit_expr(body);
        Self {
            last_use_pos: collector.uses,
        }
    }

    /// Returns the last use position for a variable, if any.
    pub fn last_use(&self, name: &str) -> Option<usize> {
        self.last_use_pos.get(name).copied()
    }

    /// Checks if a variable is live (has uses at or after the given position).
    pub fn is_live_at(&self, name: &str, pos: usize) -> bool {
        self.last_use_pos.get(name).is_some_and(|&last| last >= pos)
    }
}

/// Walks the AST and collects variable use positions.
struct UseCollector {
    /// Variable name → maximum source position (span.end) where it's used.
    uses: HashMap<String, usize>,
    /// Stack for loop handling: variables used inside the current loop.
    loop_stack: Vec<HashSet<String>>,
}

impl UseCollector {
    fn new() -> Self {
        Self {
            uses: HashMap::new(),
            loop_stack: Vec::new(),
        }
    }

    /// Records a use of a variable at the given source position.
    fn record_use(&mut self, name: &str, pos: usize) {
        let entry = self.uses.entry(name.to_string()).or_insert(0);
        *entry = (*entry).max(pos);
        // Also record in loop stack if we're inside a loop
        if let Some(loop_vars) = self.loop_stack.last_mut() {
            loop_vars.insert(name.to_string());
        }
    }

    /// Extends loop variables' last-use to the loop end position,
    /// and propagates them to the parent loop (if nested).
    fn extend_loop_vars(&mut self, loop_vars: &HashSet<String>, loop_end: usize) {
        for var in loop_vars {
            let entry = self.uses.entry(var.clone()).or_insert(0);
            *entry = (*entry).max(loop_end);
            // Propagate to parent loop if nested
            if let Some(parent_loop) = self.loop_stack.last_mut() {
                parent_loop.insert(var.clone());
            }
        }
    }

    fn visit_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Ident { name, span } => {
                self.record_use(name, span.end);
            }
            Expr::Literal { .. } | Expr::Path { .. } => {}
            Expr::Binary { left, right, .. } => {
                self.visit_expr(left);
                self.visit_expr(right);
            }
            Expr::Unary { operand, .. } => {
                self.visit_expr(operand);
            }
            Expr::Call { callee, args, .. } => {
                self.visit_expr(callee);
                for arg in args {
                    self.visit_expr(&arg.value);
                }
            }
            Expr::MethodCall { receiver, args, .. } => {
                self.visit_expr(receiver);
                for arg in args {
                    self.visit_expr(&arg.value);
                }
            }
            Expr::Field { object, .. } => {
                self.visit_expr(object);
            }
            Expr::Index { object, index, .. } => {
                self.visit_expr(object);
                self.visit_expr(index);
            }
            Expr::Block { stmts, expr, .. } => {
                for stmt in stmts {
                    self.visit_stmt(stmt);
                }
                if let Some(e) = expr {
                    self.visit_expr(e);
                }
            }
            Expr::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                self.visit_expr(condition);
                self.visit_expr(then_branch);
                if let Some(else_br) = else_branch {
                    self.visit_expr(else_br);
                }
            }
            Expr::While {
                condition,
                body,
                span,
                ..
            } => {
                self.loop_stack.push(HashSet::new());
                self.visit_expr(condition);
                self.visit_expr(body);
                let loop_vars = self.loop_stack.pop().unwrap_or_default();
                self.extend_loop_vars(&loop_vars, span.end);
            }
            Expr::For {
                variable,
                iterable,
                body,
                span,
                ..
            } => {
                self.visit_expr(iterable);
                self.loop_stack.push(HashSet::new());
                // Loop variable is used in each iteration
                self.record_use(variable, span.end);
                self.visit_expr(body);
                let loop_vars = self.loop_stack.pop().unwrap_or_default();
                self.extend_loop_vars(&loop_vars, span.end);
            }
            Expr::Loop { body, span, .. } => {
                self.loop_stack.push(HashSet::new());
                self.visit_expr(body);
                let loop_vars = self.loop_stack.pop().unwrap_or_default();
                self.extend_loop_vars(&loop_vars, span.end);
            }
            Expr::Match { subject, arms, .. } => {
                self.visit_expr(subject);
                for arm in arms {
                    self.visit_pattern(&arm.pattern);
                    if let Some(guard) = &arm.guard {
                        self.visit_expr(guard);
                    }
                    self.visit_expr(&arm.body);
                }
            }
            Expr::Assign { target, value, .. } => {
                self.visit_expr(target);
                self.visit_expr(value);
            }
            Expr::Pipe { left, right, .. } => {
                self.visit_expr(left);
                self.visit_expr(right);
            }
            Expr::Array { elements, .. } | Expr::Tuple { elements, .. } => {
                for elem in elements {
                    self.visit_expr(elem);
                }
            }
            Expr::Range { start, end, .. } => {
                if let Some(s) = start {
                    self.visit_expr(s);
                }
                if let Some(e) = end {
                    self.visit_expr(e);
                }
            }
            Expr::Cast { expr, .. }
            | Expr::Try { expr, .. }
            | Expr::Grouped { expr, .. }
            | Expr::Await { expr, .. } => {
                self.visit_expr(expr);
            }
            Expr::InlineAsm { operands, .. } => {
                for op in operands {
                    match op {
                        AsmOperand::In { expr, .. }
                        | AsmOperand::Out { expr, .. }
                        | AsmOperand::InOut { expr, .. }
                        | AsmOperand::Const { expr } => {
                            self.visit_expr(expr);
                        }
                        AsmOperand::Sym { .. } => {}
                    }
                }
            }
            Expr::Closure { body, .. } => {
                // Variables used in closure body are captured — they're live
                // at the closure creation point.
                self.visit_expr(body);
            }
            Expr::StructInit { fields, .. } => {
                for field in fields {
                    self.visit_expr(&field.value);
                }
            }
        }
    }

    fn visit_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Let { value, .. } | Stmt::Const { value, .. } => {
                self.visit_expr(value);
            }
            Stmt::Expr { expr, .. } => {
                self.visit_expr(expr);
            }
            Stmt::Return { value, .. } | Stmt::Break { value, .. } => {
                if let Some(v) = value {
                    self.visit_expr(v);
                }
            }
            Stmt::Continue { .. } => {}
            Stmt::Item(item) => {
                self.visit_item(item);
            }
        }
    }

    fn visit_item(&mut self, item: &Item) {
        match item {
            // Don't traverse into nested function bodies — they have their own NLL
            Item::FnDef(_) => {}
            Item::Stmt(stmt) => {
                self.visit_stmt(stmt);
            }
            _ => {}
        }
    }

    fn visit_pattern(&mut self, pattern: &Pattern) {
        match pattern {
            Pattern::Ident { .. } | Pattern::Literal { .. } | Pattern::Wildcard { .. } => {}
            Pattern::Tuple { elements, .. } => {
                for p in elements {
                    self.visit_pattern(p);
                }
            }
            Pattern::Struct { fields, .. } => {
                for f in fields {
                    if let Some(p) = &f.pattern {
                        self.visit_pattern(p);
                    }
                }
            }
            Pattern::Enum { fields, .. } => {
                for p in fields {
                    self.visit_pattern(p);
                }
            }
            Pattern::Range { start, end, .. } => {
                self.visit_expr(start);
                self.visit_expr(end);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::token::Span;

    // Helper: create a simple Ident expr
    fn ident(name: &str, start: usize, end: usize) -> Expr {
        Expr::Ident {
            name: name.to_string(),
            span: Span::new(start, end),
        }
    }

    // Helper: create a literal expr
    fn int_lit(val: i64, start: usize, end: usize) -> Expr {
        Expr::Literal {
            kind: LiteralKind::Int(val),
            span: Span::new(start, end),
        }
    }

    // Helper: create a let statement
    fn let_stmt(name: &str, value: Expr, start: usize, end: usize) -> Stmt {
        Stmt::Let {
            mutable: false,
            name: name.to_string(),
            ty: None,
            value: Box::new(value),
            span: Span::new(start, end),
        }
    }

    // Helper: create an expr statement
    fn expr_stmt(expr: Expr, start: usize, end: usize) -> Stmt {
        Stmt::Expr {
            expr: Box::new(expr),
            span: Span::new(start, end),
        }
    }

    // Helper: create a block expression
    fn block(stmts: Vec<Stmt>, tail: Option<Expr>, start: usize, end: usize) -> Expr {
        Expr::Block {
            stmts,
            expr: tail.map(Box::new),
            span: Span::new(start, end),
        }
    }

    #[test]
    fn nll_simple_variable_last_use() {
        // let x = 1       (pos 0-10)
        // let y = x       (pos 15-25, uses x at pos 20-21)
        // let z = y       (pos 30-40, uses y at pos 35-36)
        let body = block(
            vec![
                let_stmt("x", int_lit(1, 5, 6), 0, 10),
                let_stmt("y", ident("x", 20, 21), 15, 25),
                let_stmt("z", ident("y", 35, 36), 30, 40),
            ],
            None,
            0,
            42,
        );
        let nll = NllInfo::analyze(&body);
        assert_eq!(nll.last_use("x"), Some(21));
        assert_eq!(nll.last_use("y"), Some(36));
        assert_eq!(nll.last_use("z"), None); // z is defined but never used
    }

    #[test]
    fn nll_variable_used_multiple_times() {
        // let x = 1       (pos 0-10)
        // expr x           (pos 15-20, uses x at 15-16)
        // expr x           (pos 25-30, uses x at 25-26)
        let body = block(
            vec![
                let_stmt("x", int_lit(1, 5, 6), 0, 10),
                expr_stmt(ident("x", 15, 16), 15, 20),
                expr_stmt(ident("x", 25, 26), 25, 30),
            ],
            None,
            0,
            32,
        );
        let nll = NllInfo::analyze(&body);
        assert_eq!(nll.last_use("x"), Some(26)); // max of 16 and 26
    }

    #[test]
    fn nll_variable_in_if_branches() {
        // let x = 1         (pos 0-10)
        // if cond {          (pos 15-60)
        //   expr x           (pos 20-25, uses x at 20-21)
        // } else {
        //   expr x           (pos 40-45, uses x at 40-41)
        // }
        let body = block(
            vec![
                let_stmt("x", int_lit(1, 5, 6), 0, 10),
                expr_stmt(
                    Expr::If {
                        condition: Box::new(ident("cond", 18, 22)),
                        then_branch: Box::new(block(
                            vec![expr_stmt(ident("x", 20, 21), 20, 25)],
                            None,
                            20,
                            30,
                        )),
                        else_branch: Some(Box::new(block(
                            vec![expr_stmt(ident("x", 40, 41), 40, 45)],
                            None,
                            40,
                            50,
                        ))),
                        span: Span::new(15, 60),
                    },
                    15,
                    60,
                ),
            ],
            None,
            0,
            62,
        );
        let nll = NllInfo::analyze(&body);
        // x is used in both branches — last use is max(21, 41) = 41
        assert_eq!(nll.last_use("x"), Some(41));
    }

    #[test]
    fn nll_variable_in_while_loop_extended() {
        // let x = 1         (pos 0-10)
        // while cond {       (pos 15-50)
        //   expr x           (pos 20-25, uses x at 20-21)
        // }
        let body = block(
            vec![
                let_stmt("x", int_lit(1, 5, 6), 0, 10),
                expr_stmt(
                    Expr::While {
                        condition: Box::new(ident("cond", 21, 25)),
                        body: Box::new(block(
                            vec![expr_stmt(ident("x", 30, 31), 30, 35)],
                            None,
                            28,
                            45,
                        )),
                        span: Span::new(15, 50),
                    },
                    15,
                    50,
                ),
            ],
            None,
            0,
            52,
        );
        let nll = NllInfo::analyze(&body);
        // x is used in while loop — extended to loop end (50)
        assert_eq!(nll.last_use("x"), Some(50));
    }

    #[test]
    fn nll_unused_variable_returns_none() {
        let body = block(vec![let_stmt("x", int_lit(1, 5, 6), 0, 10)], None, 0, 12);
        let nll = NllInfo::analyze(&body);
        assert_eq!(nll.last_use("x"), None);
    }

    #[test]
    fn nll_is_live_at_before_last_use() {
        // x used at pos 25
        let body = block(
            vec![
                let_stmt("x", int_lit(1, 5, 6), 0, 10),
                expr_stmt(ident("x", 20, 25), 20, 30),
            ],
            None,
            0,
            32,
        );
        let nll = NllInfo::analyze(&body);
        assert!(nll.is_live_at("x", 15)); // before last use — live
        assert!(nll.is_live_at("x", 25)); // at last use — live
        assert!(!nll.is_live_at("x", 26)); // after last use — dead
    }

    #[test]
    fn nll_nested_loop_extends_to_outer_loop() {
        // while {             (pos 0-100)
        //   while {           (pos 10-80)
        //     expr x          (pos 30-35, uses x at 30-31)
        //   }
        // }
        let body = block(
            vec![expr_stmt(
                Expr::While {
                    condition: Box::new(Expr::Literal {
                        kind: LiteralKind::Bool(true),
                        span: Span::new(6, 10),
                    }),
                    body: Box::new(block(
                        vec![expr_stmt(
                            Expr::While {
                                condition: Box::new(Expr::Literal {
                                    kind: LiteralKind::Bool(true),
                                    span: Span::new(16, 20),
                                }),
                                body: Box::new(block(
                                    vec![expr_stmt(ident("x", 30, 31), 30, 35)],
                                    None,
                                    28,
                                    70,
                                )),
                                span: Span::new(10, 80),
                            },
                            10,
                            80,
                        )],
                        None,
                        10,
                        85,
                    )),
                    span: Span::new(0, 100),
                },
                0,
                100,
            )],
            None,
            0,
            102,
        );
        let nll = NllInfo::analyze(&body);
        // x is used in inner loop (extended to 80), then outer loop extends to 100
        assert_eq!(nll.last_use("x"), Some(100));
    }

    #[test]
    fn nll_for_loop_variable_extended() {
        // for i in 0..10 {    (pos 0-50)
        //   expr i             (pos 20-25, uses i at 20-21)
        // }
        let body = block(
            vec![expr_stmt(
                Expr::For {
                    variable: "i".to_string(),
                    iterable: Box::new(Expr::Range {
                        start: Some(Box::new(int_lit(0, 7, 8))),
                        end: Some(Box::new(int_lit(10, 10, 12))),
                        inclusive: false,
                        span: Span::new(7, 12),
                    }),
                    body: Box::new(block(
                        vec![expr_stmt(ident("i", 20, 21), 20, 25)],
                        None,
                        18,
                        45,
                    )),
                    span: Span::new(0, 50),
                },
                0,
                50,
            )],
            None,
            0,
            52,
        );
        let nll = NllInfo::analyze(&body);
        // i is used in for loop — extended to loop end (50)
        assert_eq!(nll.last_use("i"), Some(50));
    }

    #[test]
    fn nll_closure_captures_extend_uses() {
        // let x = 1           (pos 0-10)
        // let f = |a| x + a   (pos 15-35, uses x at 25-26)
        let body = block(
            vec![
                let_stmt("x", int_lit(1, 5, 6), 0, 10),
                let_stmt(
                    "f",
                    Expr::Closure {
                        params: vec![ClosureParam {
                            name: "a".to_string(),
                            ty: None,
                            span: Span::new(20, 21),
                        }],
                        return_type: None,
                        body: Box::new(Expr::Binary {
                            left: Box::new(ident("x", 25, 26)),
                            op: BinOp::Add,
                            right: Box::new(ident("a", 29, 30)),
                            span: Span::new(25, 30),
                        }),
                        span: Span::new(19, 35),
                    },
                    15,
                    35,
                ),
            ],
            None,
            0,
            37,
        );
        let nll = NllInfo::analyze(&body);
        // x is used inside closure — counts as a use at the closure's position
        assert_eq!(nll.last_use("x"), Some(26));
    }
}
