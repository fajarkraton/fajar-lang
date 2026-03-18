//! no_std runtime support for bare-metal targets.
//!
//! Provides compile-time checks and constraints for no_std compilation:
//! - No heap allocation (malloc/free)
//! - No filesystem operations
//! - No stdio (printf, scanf)
//! - No networking
//! - Static memory allocation only
//!
//! Used by `@kernel` context and bare-metal targets.

use crate::parser::ast::{Expr, Item, LiteralKind, Program, Stmt};

/// Functions that require the standard library (forbidden in no_std mode).
const FORBIDDEN_STD_BUILTINS: &[&str] = &[
    // I/O
    "read_file",
    "write_file",
    "append_file",
    "file_exists",
    // Heap-dependent tensor ops
    "tensor_zeros",
    "tensor_ones",
    "tensor_randn",
    "tensor_xavier",
    "tensor_from_data",
    "tensor_eye",
    "tensor_arange",
    "tensor_linspace",
    "tensor_matmul",
    "tensor_add",
    "tensor_sub",
    "tensor_mul",
    "tensor_div",
    "tensor_relu",
    "tensor_sigmoid",
    "tensor_tanh",
    "tensor_softmax",
    "tensor_reshape",
    "tensor_transpose",
    "tensor_flatten",
    // String allocation
    "split",
    "replace",
    "repeat",
    "to_uppercase",
    "to_lowercase",
    // Optimizer/layer (heap-heavy)
    "optimizer_sgd",
    "optimizer_adam",
    "layer_dense",
];

/// A violation found during no_std analysis.
#[derive(Debug, Clone)]
pub struct NoStdViolation {
    /// Description of the violation.
    pub message: String,
    /// The function or context where the violation was found.
    pub location: String,
}

impl std::fmt::Display for NoStdViolation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[NS001] no_std violation in {}: {}",
            self.location, self.message
        )
    }
}

/// Configuration for no_std compilation.
#[derive(Debug, Clone)]
pub struct NoStdConfig {
    /// Whether heap allocation is allowed.
    pub allow_heap: bool,
    /// Whether floating-point operations are allowed.
    pub allow_float: bool,
    /// Whether string operations (heap-allocated) are allowed.
    pub allow_strings: bool,
    /// Maximum stack size in bytes.
    pub max_stack_bytes: usize,
}

impl Default for NoStdConfig {
    fn default() -> Self {
        Self {
            allow_heap: false,
            allow_float: true,
            allow_strings: false,
            max_stack_bytes: 8192, // 8KB default stack for embedded
        }
    }
}

impl NoStdConfig {
    /// Configuration for @kernel context: no heap, no float.
    pub fn kernel() -> Self {
        Self {
            allow_heap: false,
            allow_float: false,
            allow_strings: false,
            max_stack_bytes: 4096, // 4KB kernel stack
        }
    }

    /// Configuration for bare-metal: no heap, float allowed, static strings allowed.
    /// String literals compile to .rodata section (read-only static data, no heap needed).
    pub fn bare_metal() -> Self {
        Self {
            allow_heap: false,
            allow_float: true,
            allow_strings: true, // strings → .rodata, not heap
            max_stack_bytes: 8192,
        }
    }
}

/// Analyzes a program for no_std compliance.
///
/// Returns a list of violations found. An empty list means the program
/// is safe for no_std compilation.
pub fn check_nostd_compliance(program: &Program, config: &NoStdConfig) -> Vec<NoStdViolation> {
    let mut violations = Vec::new();

    for item in &program.items {
        if let Item::FnDef(fndef) = item {
            check_expr_nostd(&fndef.body, &fndef.name, config, &mut violations);
        }
    }

    violations
}

/// Recursively checks an expression for no_std violations.
fn check_expr_nostd(
    expr: &Expr,
    location: &str,
    config: &NoStdConfig,
    violations: &mut Vec<NoStdViolation>,
) {
    match expr {
        Expr::Call { callee, args, .. } => {
            // Check if calling a forbidden std builtin
            if let Expr::Ident { name, .. } = callee.as_ref() {
                if FORBIDDEN_STD_BUILTINS.contains(&name.as_str()) {
                    violations.push(NoStdViolation {
                        message: format!("call to `{name}` requires std library"),
                        location: location.to_string(),
                    });
                }
            }
            check_expr_nostd(callee, location, config, violations);
            for arg in args {
                check_expr_nostd(&arg.value, location, config, violations);
            }
        }

        Expr::Literal { kind, .. } => {
            if !config.allow_float {
                if let LiteralKind::Float(_) = kind {
                    violations.push(NoStdViolation {
                        message: "floating-point literal not allowed (soft-float mode)".to_string(),
                        location: location.to_string(),
                    });
                }
            }
            if !config.allow_strings {
                if let LiteralKind::String(_) = kind {
                    violations.push(NoStdViolation {
                        message: "string literal requires heap allocation".to_string(),
                        location: location.to_string(),
                    });
                }
            }
        }

        Expr::Block { stmts, expr, .. } => {
            for stmt in stmts {
                check_stmt_nostd(stmt, location, config, violations);
            }
            if let Some(tail) = expr {
                check_expr_nostd(tail, location, config, violations);
            }
        }

        Expr::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            check_expr_nostd(condition, location, config, violations);
            check_expr_nostd(then_branch, location, config, violations);
            if let Some(eb) = else_branch {
                check_expr_nostd(eb, location, config, violations);
            }
        }

        Expr::While {
            condition, body, ..
        } => {
            check_expr_nostd(condition, location, config, violations);
            check_expr_nostd(body, location, config, violations);
        }

        Expr::For { iterable, body, .. } => {
            check_expr_nostd(iterable, location, config, violations);
            check_expr_nostd(body, location, config, violations);
        }

        Expr::Loop { body, .. } => {
            check_expr_nostd(body, location, config, violations);
        }

        Expr::Binary { left, right, .. } => {
            check_expr_nostd(left, location, config, violations);
            check_expr_nostd(right, location, config, violations);
        }

        Expr::Unary { operand, .. } => {
            check_expr_nostd(operand, location, config, violations);
        }

        Expr::Index { object, index, .. } => {
            check_expr_nostd(object, location, config, violations);
            check_expr_nostd(index, location, config, violations);
        }

        Expr::Array { elements, .. } => {
            for el in elements {
                check_expr_nostd(el, location, config, violations);
            }
        }

        Expr::Tuple { elements, .. } => {
            for el in elements {
                check_expr_nostd(el, location, config, violations);
            }
        }

        Expr::MethodCall { receiver, args, .. } => {
            check_expr_nostd(receiver, location, config, violations);
            for arg in args {
                check_expr_nostd(&arg.value, location, config, violations);
            }
        }

        Expr::Field { object, .. } => {
            check_expr_nostd(object, location, config, violations);
        }

        Expr::Assign { target, value, .. } => {
            check_expr_nostd(target, location, config, violations);
            check_expr_nostd(value, location, config, violations);
        }

        Expr::Pipe { left, right, .. } => {
            check_expr_nostd(left, location, config, violations);
            check_expr_nostd(right, location, config, violations);
        }

        Expr::Cast { expr, .. } => {
            check_expr_nostd(expr, location, config, violations);
        }

        Expr::Match { subject, arms, .. } => {
            check_expr_nostd(subject, location, config, violations);
            for arm in arms {
                check_expr_nostd(&arm.body, location, config, violations);
            }
        }

        // Leaf nodes: Ident, Path, Range, Break, Continue, etc.
        _ => {}
    }
}

/// Checks a statement for no_std violations.
fn check_stmt_nostd(
    stmt: &Stmt,
    location: &str,
    config: &NoStdConfig,
    violations: &mut Vec<NoStdViolation>,
) {
    match stmt {
        Stmt::Let { value, .. } | Stmt::Const { value, .. } => {
            check_expr_nostd(value, location, config, violations);
        }
        Stmt::Expr { expr, .. } => {
            check_expr_nostd(expr, location, config, violations);
        }
        Stmt::Return { value, .. } => {
            if let Some(v) = value {
                check_expr_nostd(v, location, config, violations);
            }
        }
        Stmt::Break { value, .. } => {
            if let Some(v) = value {
                check_expr_nostd(v, location, config, violations);
            }
        }
        Stmt::Item(item) => {
            if let Item::FnDef(fndef) = item.as_ref() {
                check_expr_nostd(&fndef.body, &fndef.name, config, violations);
            }
        }
        Stmt::Continue { .. } => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::tokenize;
    use crate::parser::parse;

    fn check(src: &str, config: &NoStdConfig) -> Vec<NoStdViolation> {
        let tokens = tokenize(src).unwrap();
        let program = parse(tokens).unwrap();
        check_nostd_compliance(&program, config)
    }

    #[test]
    fn nostd_pure_arithmetic_passes() {
        let src = r#"
            fn add(a: i64, b: i64) -> i64 { a + b }
            fn main() -> i64 { add(1, 2) }
        "#;
        let violations = check(src, &NoStdConfig::default());
        assert!(violations.is_empty());
    }

    #[test]
    fn nostd_loop_passes() {
        let src = r#"
            fn main() -> i64 {
                let mut sum = 0
                let mut i = 0
                while i < 10 {
                    sum = sum + i
                    i = i + 1
                }
                sum
            }
        "#;
        let violations = check(src, &NoStdConfig::default());
        assert!(violations.is_empty());
    }

    #[test]
    fn nostd_tensor_forbidden() {
        let src = r#"
            fn main() -> i64 {
                let t = tensor_zeros([2, 3])
                0
            }
        "#;
        let violations = check(src, &NoStdConfig::default());
        assert_eq!(violations.len(), 1);
        assert!(violations[0].message.contains("tensor_zeros"));
    }

    #[test]
    fn nostd_file_io_forbidden() {
        let src = r#"
            fn main() -> i64 {
                let data = read_file("config.txt")
                0
            }
        "#;
        let violations = check(src, &NoStdConfig::default());
        // 2 violations: read_file (std builtin) + "config.txt" (string literal)
        assert_eq!(violations.len(), 2);
        assert!(violations.iter().any(|v| v.message.contains("read_file")));
        assert!(violations.iter().any(|v| v.message.contains("string")));
    }

    #[test]
    fn nostd_kernel_no_float() {
        let src = r#"
            fn main() -> i64 {
                let x = 3.14
                0
            }
        "#;
        let config = NoStdConfig::kernel();
        let violations = check(src, &config);
        assert_eq!(violations.len(), 1);
        assert!(violations[0].message.contains("floating-point"));
    }

    #[test]
    fn nostd_kernel_no_string() {
        let src = r#"
            fn main() -> i64 {
                let msg = "hello"
                0
            }
        "#;
        let config = NoStdConfig::kernel();
        let violations = check(src, &config);
        assert_eq!(violations.len(), 1);
        assert!(violations[0].message.contains("string"));
    }

    #[test]
    fn nostd_bare_metal_float_allowed() {
        let src = r#"
            fn main() -> i64 {
                let x = 3.14
                0
            }
        "#;
        let config = NoStdConfig::bare_metal();
        let violations = check(src, &config);
        assert!(violations.is_empty());
    }

    #[test]
    fn nostd_multiple_violations() {
        let src = r#"
            fn main() -> i64 {
                let t = tensor_zeros([2, 3])
                let data = read_file("test.txt")
                write_file("out.txt", "data")
                0
            }
        "#;
        let violations = check(src, &NoStdConfig::default());
        assert!(violations.len() >= 3);
    }

    #[test]
    fn nostd_nested_violations() {
        let src = r#"
            fn main() -> i64 {
                if true {
                    let t = tensor_randn([4, 4])
                    0
                } else {
                    let d = read_file("x")
                    1
                }
            }
        "#;
        let violations = check(src, &NoStdConfig::default());
        // 3 violations: tensor_randn, read_file, "x" (string literal)
        assert_eq!(violations.len(), 3);
        assert!(violations
            .iter()
            .any(|v| v.message.contains("tensor_randn")));
        assert!(violations.iter().any(|v| v.message.contains("read_file")));
    }

    #[test]
    fn nostd_display_format() {
        let v = NoStdViolation {
            message: "call to `tensor_zeros` requires std library".to_string(),
            location: "main".to_string(),
        };
        let s = format!("{v}");
        assert!(s.contains("NS001"));
        assert!(s.contains("main"));
        assert!(s.contains("tensor_zeros"));
    }
}
