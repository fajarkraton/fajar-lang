//! Do-notation — syntactic sugar for monadic bind chains,
//! let-in-do, pattern matching in do, type inference.

use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// S3.1: Do-Block Syntax
// ═══════════════════════════════════════════════════════════════════════

/// A statement inside a do-block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DoStmt {
    /// `x <- expr` — monadic bind.
    Bind { pattern: DoPattern, expr: String },
    /// `let x = expr` — non-monadic binding.
    Let { pattern: DoPattern, expr: String },
    /// `expr` — a plain expression (last one is the return value).
    Expr(String),
    /// `return expr` — explicit return.
    Return(String),
}

/// A pattern in a do-block binding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DoPattern {
    /// A simple variable binding: `x`.
    Var(String),
    /// A tuple destructuring: `(a, b)`.
    Tuple(Vec<DoPattern>),
    /// A wildcard: `_`.
    Wildcard,
}

impl fmt::Display for DoPattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DoPattern::Var(name) => write!(f, "{name}"),
            DoPattern::Tuple(pats) => {
                let inner: Vec<String> = pats.iter().map(|p| p.to_string()).collect();
                write!(f, "({})", inner.join(", "))
            }
            DoPattern::Wildcard => write!(f, "_"),
        }
    }
}

/// A parsed do-block.
#[derive(Debug, Clone)]
pub struct DoBlock {
    /// Statements in the do-block.
    pub stmts: Vec<DoStmt>,
    /// Inferred monad type (e.g., "Option", "Result").
    pub monad_type: Option<String>,
}

// ═══════════════════════════════════════════════════════════════════════
// S3.2: Desugaring
// ═══════════════════════════════════════════════════════════════════════

/// Desugars a do-block into nested bind calls.
///
/// `do { x <- e1; e2 }` becomes `bind(e1, fn(x) { e2 })`
pub fn desugar_do_block(block: &DoBlock) -> String {
    desugar_stmts(&block.stmts)
}

fn desugar_stmts(stmts: &[DoStmt]) -> String {
    if stmts.is_empty() {
        return "()".into();
    }

    if stmts.len() == 1 {
        return match &stmts[0] {
            DoStmt::Expr(e) => e.clone(),
            DoStmt::Return(e) => format!("pure({e})"),
            DoStmt::Bind { expr, .. } => expr.clone(),
            DoStmt::Let { expr, .. } => expr.clone(),
        };
    }

    let first = &stmts[0];
    let rest = desugar_stmts(&stmts[1..]);

    match first {
        DoStmt::Bind { pattern, expr } => {
            format!("bind({expr}, fn({pattern}) {{ {rest} }})")
        }
        DoStmt::Let { pattern, expr } => {
            format!("{{ let {pattern} = {expr}; {rest} }}")
        }
        DoStmt::Expr(e) => {
            format!("bind({e}, fn(_) {{ {rest} }})")
        }
        DoStmt::Return(e) => {
            format!("pure({e})")
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S3.3-S3.4: Let-in-Do & Pattern Matching
// ═══════════════════════════════════════════════════════════════════════

/// Creates a do-block statement from a bind expression.
pub fn make_bind(var: &str, expr: &str) -> DoStmt {
    DoStmt::Bind {
        pattern: DoPattern::Var(var.into()),
        expr: expr.into(),
    }
}

/// Creates a let-in-do statement.
pub fn make_let(var: &str, expr: &str) -> DoStmt {
    DoStmt::Let {
        pattern: DoPattern::Var(var.into()),
        expr: expr.into(),
    }
}

/// Creates a tuple pattern bind.
pub fn make_tuple_bind(vars: &[&str], expr: &str) -> DoStmt {
    DoStmt::Bind {
        pattern: DoPattern::Tuple(vars.iter().map(|v| DoPattern::Var(v.to_string())).collect()),
        expr: expr.into(),
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S3.5: Type Inference in Do
// ═══════════════════════════════════════════════════════════════════════

/// Inferred monad context for a do-block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MonadContext {
    /// Option monad.
    Option,
    /// Result monad.
    Result,
    /// Async/Future monad.
    Async,
    /// Vec/List monad.
    Vec,
    /// Unknown — needs more information.
    Unknown,
}

impl fmt::Display for MonadContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MonadContext::Option => write!(f, "Option"),
            MonadContext::Result => write!(f, "Result"),
            MonadContext::Async => write!(f, "Future"),
            MonadContext::Vec => write!(f, "Vec"),
            MonadContext::Unknown => write!(f, "?Monad"),
        }
    }
}

/// Infers the monad context from the first bind expression's type.
pub fn infer_monad_context(first_expr_type: &str) -> MonadContext {
    if first_expr_type.starts_with("Option") {
        MonadContext::Option
    } else if first_expr_type.starts_with("Result") {
        MonadContext::Result
    } else if first_expr_type.starts_with("Future") || first_expr_type.contains("async") {
        MonadContext::Async
    } else if first_expr_type.starts_with("Vec") {
        MonadContext::Vec
    } else {
        MonadContext::Unknown
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S3.6-S3.8: Do for Option / Result / Async
// ═══════════════════════════════════════════════════════════════════════

/// Specialized desugaring for Option do-blocks.
pub fn desugar_option_do(stmts: &[DoStmt]) -> String {
    let mut code = desugar_stmts(stmts);
    // Replace generic bind with Option-specific and_then
    code = code.replace("bind(", "and_then(");
    code = code.replace("pure(", "Some(");
    code
}

/// Specialized desugaring for Result do-blocks.
pub fn desugar_result_do(stmts: &[DoStmt]) -> String {
    let mut code = desugar_stmts(stmts);
    code = code.replace("bind(", "and_then(");
    code = code.replace("pure(", "Ok(");
    code
}

/// Specialized desugaring for async do-blocks.
pub fn desugar_async_do(stmts: &[DoStmt]) -> String {
    let mut result = String::from("async {\n");
    for stmt in stmts {
        match stmt {
            DoStmt::Bind { pattern, expr } => {
                result.push_str(&format!("    let {pattern} = {expr}.await;\n"));
            }
            DoStmt::Let { pattern, expr } => {
                result.push_str(&format!("    let {pattern} = {expr};\n"));
            }
            DoStmt::Expr(e) => {
                result.push_str(&format!("    {e};\n"));
            }
            DoStmt::Return(e) => {
                result.push_str(&format!("    {e}\n"));
            }
        }
    }
    result.push('}');
    result
}

// ═══════════════════════════════════════════════════════════════════════
// S3.9: Nested Do Blocks
// ═══════════════════════════════════════════════════════════════════════

/// Validates that nested do-blocks have consistent monad types.
pub fn validate_nesting(outer: &MonadContext, inner: &MonadContext) -> Result<(), String> {
    match (outer, inner) {
        (MonadContext::Unknown, _) | (_, MonadContext::Unknown) => Ok(()),
        (a, b) if a == b => Ok(()),
        (a, b) => Err(format!(
            "nested do-block has monad type `{b}` but outer do-block uses `{a}`"
        )),
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // S3.1 — Do-Block Syntax
    #[test]
    fn s3_1_do_pattern_display() {
        assert_eq!(DoPattern::Var("x".into()).to_string(), "x");
        assert_eq!(DoPattern::Wildcard.to_string(), "_");
        assert_eq!(
            DoPattern::Tuple(vec![DoPattern::Var("a".into()), DoPattern::Var("b".into()),])
                .to_string(),
            "(a, b)"
        );
    }

    #[test]
    fn s3_1_do_block_structure() {
        let block = DoBlock {
            stmts: vec![
                make_bind("x", "get_value()"),
                DoStmt::Return("x + 1".into()),
            ],
            monad_type: Some("Option".into()),
        };
        assert_eq!(block.stmts.len(), 2);
    }

    // S3.2 — Desugaring
    #[test]
    fn s3_2_simple_bind_desugar() {
        let block = DoBlock {
            stmts: vec![
                make_bind("x", "get_user(id)"),
                DoStmt::Return("x.name".into()),
            ],
            monad_type: None,
        };
        let desugared = desugar_do_block(&block);
        assert!(desugared.contains("bind(get_user(id)"));
        assert!(desugared.contains("pure(x.name)"));
    }

    #[test]
    fn s3_2_chain_desugar() {
        let block = DoBlock {
            stmts: vec![
                make_bind("x", "e1"),
                make_bind("y", "e2"),
                DoStmt::Return("x + y".into()),
            ],
            monad_type: None,
        };
        let desugared = desugar_do_block(&block);
        // Should be nested: bind(e1, fn(x) { bind(e2, fn(y) { pure(x + y) }) })
        assert!(desugared.contains("bind(e1"));
        assert!(desugared.contains("bind(e2"));
    }

    // S3.3 — Let-in-Do
    #[test]
    fn s3_3_let_in_do() {
        let block = DoBlock {
            stmts: vec![
                make_bind("x", "get_value()"),
                make_let("y", "x + 1"),
                DoStmt::Return("y".into()),
            ],
            monad_type: None,
        };
        let desugared = desugar_do_block(&block);
        assert!(desugared.contains("let y = x + 1"));
    }

    // S3.4 — Pattern Matching in Do
    #[test]
    fn s3_4_tuple_pattern() {
        let stmt = make_tuple_bind(&["a", "b"], "get_pair()");
        if let DoStmt::Bind { pattern, .. } = stmt {
            assert_eq!(pattern.to_string(), "(a, b)");
        }
    }

    // S3.5 — Type Inference in Do
    #[test]
    fn s3_5_infer_option() {
        assert_eq!(infer_monad_context("Option<i32>"), MonadContext::Option);
    }

    #[test]
    fn s3_5_infer_result() {
        assert_eq!(
            infer_monad_context("Result<User, Error>"),
            MonadContext::Result
        );
    }

    #[test]
    fn s3_5_infer_async() {
        assert_eq!(infer_monad_context("Future<i32>"), MonadContext::Async);
    }

    #[test]
    fn s3_5_infer_unknown() {
        assert_eq!(infer_monad_context("SomeType"), MonadContext::Unknown);
    }

    // S3.6 — Do for Option
    #[test]
    fn s3_6_option_do() {
        let stmts = vec![
            make_bind("x", "get_user(id)"),
            DoStmt::Return("x.email".into()),
        ];
        let code = desugar_option_do(&stmts);
        assert!(code.contains("and_then("));
        assert!(code.contains("Some("));
    }

    // S3.7 — Do for Result
    #[test]
    fn s3_7_result_do() {
        let stmts = vec![make_bind("x", "parse(s)"), DoStmt::Return("x".into())];
        let code = desugar_result_do(&stmts);
        assert!(code.contains("and_then("));
        assert!(code.contains("Ok("));
    }

    // S3.8 — Do for Async
    #[test]
    fn s3_8_async_do() {
        let stmts = vec![
            make_bind("x", "fetch(url)"),
            make_bind("y", "parse(x)"),
            DoStmt::Return("y".into()),
        ];
        let code = desugar_async_do(&stmts);
        assert!(code.contains("async {"));
        assert!(code.contains(".await"));
    }

    // S3.9 — Nested Do Blocks
    #[test]
    fn s3_9_valid_nesting() {
        assert!(validate_nesting(&MonadContext::Option, &MonadContext::Option).is_ok());
        assert!(validate_nesting(&MonadContext::Unknown, &MonadContext::Result).is_ok());
    }

    #[test]
    fn s3_9_invalid_nesting() {
        let result = validate_nesting(&MonadContext::Option, &MonadContext::Result);
        assert!(result.is_err());
    }

    // S3.10 — Integration
    #[test]
    fn s3_10_single_expr_do() {
        let block = DoBlock {
            stmts: vec![DoStmt::Expr("pure(42)".into())],
            monad_type: None,
        };
        assert_eq!(desugar_do_block(&block), "pure(42)");
    }

    #[test]
    fn s3_10_monad_context_display() {
        assert_eq!(MonadContext::Option.to_string(), "Option");
        assert_eq!(MonadContext::Async.to_string(), "Future");
    }
}
