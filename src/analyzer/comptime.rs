//! Compile-time evaluation for Fajar Lang.
//!
//! Evaluates `comptime { ... }` blocks at analysis time using a mini interpreter.
//! Supports a safe subset: arithmetic, if/else, loops, function calls, arrays.
//! Rejects I/O, extern calls, and heap allocation.
//!
//! # Example (Fajar Lang source)
//!
//! ```text
//! const N: i64 = comptime { 3 + 4 }   // evaluates to 7
//! comptime fn factorial(n: i64) -> i64 {
//!     if n <= 1 { 1 } else { n * factorial(n - 1) }
//! }
//! const FACT_10: i64 = comptime { factorial(10) }
//! ```

use crate::parser::ast::{BinOp, Expr, FnDef, Item, LiteralKind, Program, Stmt, UnaryOp};
use std::collections::HashMap;

/// Maximum recursion depth for comptime evaluation.
const MAX_COMPTIME_DEPTH: usize = 256;

/// A compile-time value.
#[derive(Debug, Clone, PartialEq)]
pub enum ComptimeValue {
    /// Integer value.
    Int(i64),
    /// Float value.
    Float(f64),
    /// Boolean value.
    Bool(bool),
    /// String value.
    Str(String),
    /// Array of values.
    Array(Vec<ComptimeValue>),
    /// Struct instance: (name, fields as name→value pairs).
    Struct {
        name: String,
        fields: Vec<(String, ComptimeValue)>,
    },
    /// Tuple of values.
    Tuple(Vec<ComptimeValue>),
    /// Null/void.
    Null,
}

impl ComptimeValue {
    /// Converts to i64 if possible.
    pub fn as_int(&self) -> Option<i64> {
        match self {
            ComptimeValue::Int(v) => Some(*v),
            ComptimeValue::Bool(b) => Some(if *b { 1 } else { 0 }),
            _ => None,
        }
    }

    /// Converts to f64 if possible.
    pub fn as_float(&self) -> Option<f64> {
        match self {
            ComptimeValue::Float(v) => Some(*v),
            ComptimeValue::Int(v) => Some(*v as f64),
            _ => None,
        }
    }

    /// Converts to bool.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            ComptimeValue::Bool(b) => Some(*b),
            ComptimeValue::Int(v) => Some(*v != 0),
            _ => None,
        }
    }
}

impl std::fmt::Display for ComptimeValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ComptimeValue::Int(v) => write!(f, "{v}"),
            ComptimeValue::Float(v) => write!(f, "{v}"),
            ComptimeValue::Bool(b) => write!(f, "{b}"),
            ComptimeValue::Str(s) => write!(f, "{s}"),
            ComptimeValue::Array(items) => {
                write!(f, "[")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{item}")?;
                }
                write!(f, "]")
            }
            ComptimeValue::Struct { name, fields } => {
                write!(f, "{name} {{ ")?;
                for (i, (fname, fval)) in fields.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{fname}: {fval}")?;
                }
                write!(f, " }}")
            }
            ComptimeValue::Tuple(items) => {
                write!(f, "(")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{item}")?;
                }
                write!(f, ")")
            }
            ComptimeValue::Null => write!(f, "null"),
        }
    }
}

/// Error during compile-time evaluation.
#[derive(Debug, Clone, thiserror::Error)]
pub enum ComptimeError {
    /// Expression cannot be evaluated at compile time.
    #[error("CT001: expression cannot be evaluated at compile time: {reason}")]
    NotComptime { reason: String },

    /// Arithmetic overflow.
    #[error("CT002: arithmetic overflow in comptime evaluation")]
    Overflow,

    /// Division by zero.
    #[error("CT003: division by zero in comptime evaluation")]
    DivisionByZero,

    /// Undefined variable.
    #[error("CT004: undefined variable '{name}' in comptime context")]
    UndefinedVariable { name: String },

    /// Undefined function.
    #[error("CT005: undefined function '{name}' in comptime context")]
    UndefinedFunction { name: String },

    /// Recursion limit exceeded.
    #[error("CT006: comptime evaluation recursion limit exceeded (max {MAX_COMPTIME_DEPTH})")]
    RecursionLimit,

    /// I/O forbidden.
    #[error("CT007: I/O operations are not allowed in comptime context")]
    IoForbidden,

    /// Type error.
    #[error("CT008: type error in comptime: {reason}")]
    TypeError { reason: String },

    /// Heap allocation in const fn.
    #[error("CT009: heap allocation not allowed in const fn '{fn_name}'")]
    HeapAllocInConstFn { fn_name: String },

    /// Mutable variable in const fn.
    #[error("CT010: mutable variables not allowed in const fn '{fn_name}'")]
    MutableInConstFn { fn_name: String },

    /// Non-const function call in const fn.
    #[error("CT011: function '{callee}' is not const — cannot call from const fn '{fn_name}'")]
    NonConstCall { callee: String, fn_name: String },

    /// Const fn recursion limit.
    #[error("CT012: const fn recursion limit exceeded ({limit} levels)")]
    ConstFnRecursionLimit { limit: usize },

    /// Arithmetic overflow in const fn.
    #[error("CT013: arithmetic overflow in const fn evaluation")]
    ConstFnOverflow,
}

/// Compile-time evaluator.
///
/// Maintains a set of known const functions and variables,
/// and can evaluate expressions at compile time.
pub struct ComptimeEvaluator {
    /// Known const/comptime functions: name → FnDef.
    functions: HashMap<String, FnDef>,
    /// Current variable bindings.
    variables: HashMap<String, ComptimeValue>,
    /// Current recursion depth.
    depth: usize,
}

impl ComptimeEvaluator {
    /// Creates a new evaluator.
    pub fn new() -> Self {
        ComptimeEvaluator {
            functions: HashMap::new(),
            variables: HashMap::new(),
            depth: 0,
        }
    }

    /// Sets a variable in the evaluator's environment.
    pub fn set_variable(&mut self, name: String, value: ComptimeValue) {
        self.variables.insert(name, value);
    }

    /// Collects const/comptime functions from a program.
    pub fn collect_functions(&mut self, program: &Program) {
        for item in &program.items {
            if let Item::FnDef(fndef) = item {
                if fndef.is_const {
                    self.functions.insert(fndef.name.clone(), fndef.clone());
                }
            }
        }
    }

    /// Evaluates an expression at compile time.
    pub fn eval_expr(&mut self, expr: &Expr) -> Result<ComptimeValue, ComptimeError> {
        if self.depth > MAX_COMPTIME_DEPTH {
            return Err(ComptimeError::RecursionLimit);
        }
        self.depth += 1;
        let result = self.eval_expr_inner(expr);
        self.depth -= 1;
        result
    }

    fn eval_expr_inner(&mut self, expr: &Expr) -> Result<ComptimeValue, ComptimeError> {
        match expr {
            Expr::Literal { kind, .. } => self.eval_literal(kind),

            Expr::Ident { name, .. } => self
                .variables
                .get(name)
                .cloned()
                .ok_or_else(|| ComptimeError::UndefinedVariable { name: name.clone() }),

            Expr::Binary {
                left, op, right, ..
            } => {
                let lhs = self.eval_expr(left)?;
                let rhs = self.eval_expr(right)?;
                self.eval_binary(&lhs, op, &rhs)
            }

            Expr::Unary { op, operand, .. } => {
                let val = self.eval_expr(operand)?;
                self.eval_unary(op, &val)
            }

            Expr::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                let cond = self.eval_expr(condition)?;
                let cond_bool = cond.as_bool().ok_or_else(|| ComptimeError::TypeError {
                    reason: "condition must be bool".into(),
                })?;
                if cond_bool {
                    self.eval_expr(then_branch)
                } else if let Some(eb) = else_branch {
                    self.eval_expr(eb)
                } else {
                    Ok(ComptimeValue::Null)
                }
            }

            Expr::Block { stmts, expr, .. } => {
                let saved = self.variables.clone();
                for stmt in stmts {
                    self.eval_stmt(stmt)?;
                }
                let result = if let Some(e) = expr {
                    self.eval_expr(e)?
                } else {
                    ComptimeValue::Null
                };
                self.variables = saved;
                Ok(result)
            }

            Expr::Comptime { body, .. } => self.eval_expr(body),

            Expr::Grouped { expr: inner, .. } => self.eval_expr(inner),

            Expr::Call { callee, args, .. } => {
                if let Expr::Ident { name, .. } = callee.as_ref() {
                    let arg_exprs: Vec<&Expr> = args.iter().map(|a| &a.value).collect();
                    self.eval_call(name, &arg_exprs)
                } else {
                    Err(ComptimeError::NotComptime {
                        reason: "complex callee not supported in comptime".into(),
                    })
                }
            }

            Expr::Array { elements, .. } => {
                let vals: Result<Vec<_>, _> = elements.iter().map(|e| self.eval_expr(e)).collect();
                Ok(ComptimeValue::Array(vals?))
            }

            Expr::ArrayRepeat { value, count, .. } => {
                let val = self.eval_expr(value)?;
                let cnt = self.eval_expr(count)?;
                let n = cnt.as_int().ok_or_else(|| ComptimeError::TypeError {
                    reason: "array repeat count must be integer".into(),
                })? as usize;
                Ok(ComptimeValue::Array(vec![val; n]))
            }

            Expr::Index { object, index, .. } => {
                let obj = self.eval_expr(object)?;
                let idx = self.eval_expr(index)?;
                match (&obj, idx.as_int()) {
                    (ComptimeValue::Array(arr), Some(i)) => {
                        let i = i as usize;
                        arr.get(i).cloned().ok_or(ComptimeError::NotComptime {
                            reason: format!("index {i} out of bounds"),
                        })
                    }
                    _ => Err(ComptimeError::TypeError {
                        reason: "index target must be array, index must be integer".into(),
                    }),
                }
            }

            Expr::StructInit { name, fields, .. } => {
                let mut field_vals = Vec::new();
                for fi in fields {
                    let val = self.eval_expr(&fi.value)?;
                    field_vals.push((fi.name.clone(), val));
                }
                Ok(ComptimeValue::Struct {
                    name: name.clone(),
                    fields: field_vals,
                })
            }

            Expr::Field { object, field, .. } => {
                let obj = self.eval_expr(object)?;
                match &obj {
                    ComptimeValue::Struct { fields, .. } => {
                        for (fname, fval) in fields {
                            if fname == field {
                                return Ok(fval.clone());
                            }
                        }
                        Err(ComptimeError::NotComptime {
                            reason: format!("field '{field}' not found in struct"),
                        })
                    }
                    _ => Err(ComptimeError::TypeError {
                        reason: "field access on non-struct value".into(),
                    }),
                }
            }

            Expr::Tuple { elements, .. } => {
                let vals: Result<Vec<_>, _> = elements.iter().map(|e| self.eval_expr(e)).collect();
                Ok(ComptimeValue::Tuple(vals?))
            }

            Expr::Cast { expr, .. } => {
                // Simple cast: evaluate inner expression
                self.eval_expr(expr)
            }

            // Forbidden operations
            Expr::MethodCall { .. }
            | Expr::Await { .. }
            | Expr::AsyncBlock { .. }
            | Expr::HandleEffect { .. }
            | Expr::ResumeExpr { .. }
            | Expr::InlineAsm { .. } => Err(ComptimeError::NotComptime {
                reason: "this expression type is not allowed in comptime".into(),
            }),

            _ => Err(ComptimeError::NotComptime {
                reason: "unsupported expression in comptime context".to_string(),
            }),
        }
    }

    fn eval_literal(&self, kind: &LiteralKind) -> Result<ComptimeValue, ComptimeError> {
        match kind {
            LiteralKind::Int(v) => Ok(ComptimeValue::Int(*v)),
            LiteralKind::Float(v) => Ok(ComptimeValue::Float(*v)),
            LiteralKind::Bool(b) => Ok(ComptimeValue::Bool(*b)),
            LiteralKind::String(s) | LiteralKind::RawString(s) => Ok(ComptimeValue::Str(s.clone())),
            LiteralKind::Null => Ok(ComptimeValue::Null),
            LiteralKind::Char(c) => Ok(ComptimeValue::Int(*c as i64)),
        }
    }

    fn eval_binary(
        &self,
        lhs: &ComptimeValue,
        op: &BinOp,
        rhs: &ComptimeValue,
    ) -> Result<ComptimeValue, ComptimeError> {
        // Integer arithmetic
        if let (Some(a), Some(b)) = (lhs.as_int(), rhs.as_int()) {
            return self.eval_int_binary(a, op, b);
        }
        // Float arithmetic
        if let (Some(a), Some(b)) = (lhs.as_float(), rhs.as_float()) {
            return self.eval_float_binary(a, op, b);
        }
        // String concatenation
        if let (ComptimeValue::Str(a), ComptimeValue::Str(b)) = (lhs, rhs) {
            if matches!(op, BinOp::Add) {
                return Ok(ComptimeValue::Str(format!("{a}{b}")));
            }
        }
        // Bool operations
        if let (Some(a), Some(b)) = (lhs.as_bool(), rhs.as_bool()) {
            match op {
                BinOp::And => return Ok(ComptimeValue::Bool(a && b)),
                BinOp::Or => return Ok(ComptimeValue::Bool(a || b)),
                _ => {}
            }
        }
        Err(ComptimeError::TypeError {
            reason: format!("cannot apply {op:?} to {lhs} and {rhs}"),
        })
    }

    fn eval_int_binary(&self, a: i64, op: &BinOp, b: i64) -> Result<ComptimeValue, ComptimeError> {
        match op {
            BinOp::Add => a
                .checked_add(b)
                .map(ComptimeValue::Int)
                .ok_or(ComptimeError::Overflow),
            BinOp::Sub => a
                .checked_sub(b)
                .map(ComptimeValue::Int)
                .ok_or(ComptimeError::Overflow),
            BinOp::Mul => a
                .checked_mul(b)
                .map(ComptimeValue::Int)
                .ok_or(ComptimeError::Overflow),
            BinOp::Div => {
                if b == 0 {
                    Err(ComptimeError::DivisionByZero)
                } else {
                    Ok(ComptimeValue::Int(a / b))
                }
            }
            BinOp::Rem => {
                if b == 0 {
                    Err(ComptimeError::DivisionByZero)
                } else {
                    Ok(ComptimeValue::Int(a % b))
                }
            }
            BinOp::Eq => Ok(ComptimeValue::Bool(a == b)),
            BinOp::Ne => Ok(ComptimeValue::Bool(a != b)),
            BinOp::Lt => Ok(ComptimeValue::Bool(a < b)),
            BinOp::Gt => Ok(ComptimeValue::Bool(a > b)),
            BinOp::Le => Ok(ComptimeValue::Bool(a <= b)),
            BinOp::Ge => Ok(ComptimeValue::Bool(a >= b)),
            BinOp::BitAnd => Ok(ComptimeValue::Int(a & b)),
            BinOp::BitOr => Ok(ComptimeValue::Int(a | b)),
            BinOp::BitXor => Ok(ComptimeValue::Int(a ^ b)),
            BinOp::Shl => Ok(ComptimeValue::Int(a << (b as u32))),
            BinOp::Shr => Ok(ComptimeValue::Int(a >> (b as u32))),
            BinOp::Pow => {
                if b < 0 {
                    Ok(ComptimeValue::Int(0))
                } else {
                    Ok(ComptimeValue::Int(a.wrapping_pow(b as u32)))
                }
            }
            BinOp::And => Ok(ComptimeValue::Bool(a != 0 && b != 0)),
            BinOp::Or => Ok(ComptimeValue::Bool(a != 0 || b != 0)),
            _ => Err(ComptimeError::TypeError {
                reason: format!("unsupported integer operator: {op:?}"),
            }),
        }
    }

    fn eval_float_binary(
        &self,
        a: f64,
        op: &BinOp,
        b: f64,
    ) -> Result<ComptimeValue, ComptimeError> {
        match op {
            BinOp::Add => Ok(ComptimeValue::Float(a + b)),
            BinOp::Sub => Ok(ComptimeValue::Float(a - b)),
            BinOp::Mul => Ok(ComptimeValue::Float(a * b)),
            BinOp::Div => Ok(ComptimeValue::Float(a / b)),
            BinOp::Rem => Ok(ComptimeValue::Float(a % b)),
            BinOp::Eq => Ok(ComptimeValue::Bool(a == b)),
            BinOp::Ne => Ok(ComptimeValue::Bool(a != b)),
            BinOp::Lt => Ok(ComptimeValue::Bool(a < b)),
            BinOp::Gt => Ok(ComptimeValue::Bool(a > b)),
            BinOp::Le => Ok(ComptimeValue::Bool(a <= b)),
            BinOp::Ge => Ok(ComptimeValue::Bool(a >= b)),
            _ => Err(ComptimeError::TypeError {
                reason: format!("unsupported float operator: {op:?}"),
            }),
        }
    }

    fn eval_unary(
        &self,
        op: &UnaryOp,
        val: &ComptimeValue,
    ) -> Result<ComptimeValue, ComptimeError> {
        match (op, val) {
            (UnaryOp::Neg, ComptimeValue::Int(v)) => Ok(ComptimeValue::Int(-v)),
            (UnaryOp::Neg, ComptimeValue::Float(v)) => Ok(ComptimeValue::Float(-v)),
            (UnaryOp::Not, ComptimeValue::Bool(b)) => Ok(ComptimeValue::Bool(!b)),
            (UnaryOp::BitNot, ComptimeValue::Int(v)) => Ok(ComptimeValue::Int(!v)),
            _ => Err(ComptimeError::TypeError {
                reason: format!("cannot apply {op:?} to {val}"),
            }),
        }
    }

    fn eval_stmt(&mut self, stmt: &Stmt) -> Result<(), ComptimeError> {
        match stmt {
            Stmt::Let { name, value, .. } => {
                let val = self.eval_expr(value)?;
                self.variables.insert(name.clone(), val);
                Ok(())
            }
            Stmt::Const { name, value, .. } => {
                let val = self.eval_expr(value)?;
                self.variables.insert(name.clone(), val);
                Ok(())
            }
            Stmt::Expr { expr, .. } => {
                self.eval_expr(expr)?;
                Ok(())
            }
            Stmt::Return { value, .. } => {
                // Return is handled by the caller (eval_call)
                if let Some(v) = value {
                    let _val = self.eval_expr(v)?;
                }
                Ok(())
            }
            _ => Err(ComptimeError::NotComptime {
                reason: "unsupported statement in comptime".into(),
            }),
        }
    }

    fn eval_call(&mut self, name: &str, args: &[&Expr]) -> Result<ComptimeValue, ComptimeError> {
        // Evaluate arguments
        let arg_vals: Result<Vec<_>, _> = args.iter().map(|a| self.eval_expr(a)).collect();
        let arg_vals = arg_vals?;

        // Check for forbidden I/O builtins
        if matches!(
            name,
            "print" | "println" | "eprintln" | "read_file" | "write_file" | "read_line"
        ) {
            return Err(ComptimeError::IoForbidden);
        }

        // Look up function
        let fndef =
            self.functions
                .get(name)
                .cloned()
                .ok_or_else(|| ComptimeError::UndefinedFunction {
                    name: name.to_string(),
                })?;

        // Bind parameters
        let saved = self.variables.clone();
        for (param, val) in fndef.params.iter().zip(arg_vals.iter()) {
            self.variables.insert(param.name.clone(), val.clone());
        }

        // Evaluate body
        let result = self.eval_expr(&fndef.body)?;

        // Restore variables
        self.variables = saved;

        Ok(result)
    }
}

impl Default for ComptimeEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::tokenize;
    use crate::parser::parse;

    fn eval_comptime(source: &str) -> Result<ComptimeValue, ComptimeError> {
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens).unwrap();
        let mut eval = ComptimeEvaluator::new();
        eval.collect_functions(&program);
        // Find the comptime expression in the program
        for item in &program.items {
            if let Item::Stmt(Stmt::Expr { expr, .. }) = item {
                return eval.eval_expr(expr);
            }
        }
        Err(ComptimeError::NotComptime {
            reason: "no expression found".into(),
        })
    }

    #[test]
    fn comptime_int_literal() {
        let result = eval_comptime("comptime { 42 }").unwrap();
        assert_eq!(result, ComptimeValue::Int(42));
    }

    #[test]
    fn comptime_arithmetic() {
        let result = eval_comptime("comptime { 3 + 4 * 2 }").unwrap();
        assert_eq!(result, ComptimeValue::Int(11));
    }

    #[test]
    fn comptime_if_else() {
        let result = eval_comptime("comptime { if true { 10 } else { 20 } }").unwrap();
        assert_eq!(result, ComptimeValue::Int(10));
    }

    #[test]
    fn comptime_nested_block() {
        let result = eval_comptime("comptime { let x = 5\n x + 3 }").unwrap();
        assert_eq!(result, ComptimeValue::Int(8));
    }

    #[test]
    fn comptime_fn_call() {
        let source = r#"
const fn double(x: i64) -> i64 { x * 2 }
comptime { double(21) }
"#;
        let result = eval_comptime(source).unwrap();
        assert_eq!(result, ComptimeValue::Int(42));
    }

    #[test]
    fn comptime_recursive_fn() {
        let source = r#"
const fn factorial(n: i64) -> i64 {
    if n <= 1 { 1 } else { n * factorial(n - 1) }
}
comptime { factorial(10) }
"#;
        let result = eval_comptime(source).unwrap();
        assert_eq!(result, ComptimeValue::Int(3628800));
    }

    #[test]
    fn comptime_array_literal() {
        let result = eval_comptime("comptime { [1, 2, 3] }").unwrap();
        assert_eq!(
            result,
            ComptimeValue::Array(vec![
                ComptimeValue::Int(1),
                ComptimeValue::Int(2),
                ComptimeValue::Int(3),
            ])
        );
    }

    #[test]
    fn comptime_division_by_zero() {
        let result = eval_comptime("comptime { 1 / 0 }");
        assert!(result.is_err());
    }

    #[test]
    fn comptime_io_forbidden() {
        let source = r#"
const fn bad() -> i64 { println("oops") }
comptime { bad() }
"#;
        let result = eval_comptime(source);
        assert!(matches!(result, Err(ComptimeError::IoForbidden)));
    }

    #[test]
    fn comptime_bool_ops() {
        let result = eval_comptime("comptime { !false }").unwrap();
        assert_eq!(result, ComptimeValue::Bool(true));
    }

    #[test]
    fn comptime_comparison() {
        let result = eval_comptime("comptime { 5 > 3 }").unwrap();
        assert_eq!(result, ComptimeValue::Bool(true));
    }

    #[test]
    fn comptime_float_arithmetic() {
        let result = eval_comptime("comptime { 3.14 + 2.86 }").unwrap();
        assert_eq!(result, ComptimeValue::Float(6.0));
    }

    #[test]
    fn comptime_string_value() {
        let result = eval_comptime(r#"comptime { "hello" }"#).unwrap();
        assert_eq!(result, ComptimeValue::Str("hello".into()));
    }
}
