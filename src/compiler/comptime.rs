//! # Compile-Time Evaluation (Phase 2)
//!
//! Provides `const fn` evaluation, `comptime {}` blocks, compile-time tensor
//! shape inference, and memoized constant folding for the Fajar Lang compiler.
//!
//! ## Architecture
//!
//! ```text
//! const fn / comptime blocks → ConstExpr → ConstEval → ConstValue
//!                                           ├── ConstCache (memoization)
//!                                           ├── Shape inference (tensors)
//!                                           └── ComptimeInterpreter (blocks)
//! ```
//!
//! ## Sprint Overview
//!
//! - **Sprint 5:** `const fn` foundations — `ConstEval`, `ConstValue`, `ConstExpr`, evaluator
//! - **Sprint 6:** `comptime {}` blocks — string/array generation, assertions
//! - **Sprint 7:** Compile-time tensor shapes — shape arithmetic, broadcast, conv2d
//! - **Sprint 8:** Optimization — caching, recursion limits, bounded loops, metrics

use std::collections::HashMap;
use std::time::Instant;

use thiserror::Error;

// ═══════════════════════════════════════════════════════════════════════
// Constants
// ═══════════════════════════════════════════════════════════════════════

/// Maximum recursion depth for compile-time function evaluation.
pub const MAX_CONST_RECURSION: usize = 128;

/// Maximum number of loop iterations allowed in compile-time evaluation.
const MAX_CONST_LOOP_ITERS: usize = 10_000;

// ═══════════════════════════════════════════════════════════════════════
// Error types (Sprint 5)
// ═══════════════════════════════════════════════════════════════════════

/// Errors arising from compile-time evaluation.
#[derive(Debug, Error, Clone, PartialEq)]
pub enum ConstError {
    /// An expression cannot be evaluated at compile time.
    #[error("expression is not const-evaluable: {reason}")]
    NotConst {
        /// Why the expression is not const.
        reason: String,
    },

    /// Integer overflow during compile-time arithmetic.
    #[error("compile-time integer overflow")]
    Overflow,

    /// Division by zero during compile-time arithmetic.
    #[error("compile-time division by zero")]
    DivByZero,

    /// Compile-time recursion exceeded the limit.
    #[error("compile-time recursion limit ({depth}/{max}) exceeded")]
    RecursionLimit {
        /// Current recursion depth when the limit was hit.
        depth: usize,
        /// The configured maximum recursion depth.
        max: usize,
    },

    /// Type mismatch during compile-time evaluation.
    #[error("compile-time type mismatch: expected {expected}, got {got}")]
    TypeMismatch {
        /// The expected type description.
        expected: String,
        /// The actual type description.
        got: String,
    },

    /// Reference to an undefined variable in const context.
    #[error("undefined compile-time variable: {name}")]
    UndefinedVar {
        /// The variable name that was not found.
        name: String,
    },

    /// Reference to an undefined function in const context.
    #[error("undefined compile-time function: {name}")]
    UndefinedFn {
        /// The function name that was not found.
        name: String,
    },

    /// Tensor shape mismatch at compile time (TE009).
    #[error("compile-time shape mismatch: {message}")]
    ShapeMismatch {
        /// Description of the shape mismatch.
        message: String,
    },

    /// A comptime assertion failed.
    #[error("compile-time assertion failed: {message}")]
    AssertionFailed {
        /// The assertion failure message.
        message: String,
    },

    /// Loop iteration limit exceeded at compile time.
    #[error("compile-time loop exceeded {max} iterations")]
    LoopLimit {
        /// The maximum allowed iterations.
        max: usize,
    },
}

// ═══════════════════════════════════════════════════════════════════════
// ConstValue (Sprint 5)
// ═══════════════════════════════════════════════════════════════════════

/// A value known at compile time.
///
/// Represents the result of evaluating a constant expression. All variants
/// are immutable and can be freely cloned for use in constant folding and
/// compile-time code generation.
#[derive(Debug, Clone, PartialEq)]
pub enum ConstValue {
    /// A 128-bit signed integer (covers all Fajar integer types).
    Int(i128),
    /// A 64-bit floating-point number.
    Float(f64),
    /// A boolean value.
    Bool(bool),
    /// A string value.
    Str(String),
    /// A fixed-size array of const values.
    Array(Vec<ConstValue>),
    /// A tuple of const values.
    Tuple(Vec<ConstValue>),
    /// A struct with named fields.
    Struct {
        /// The struct type name.
        name: String,
        /// Field name to value mapping.
        fields: HashMap<String, ConstValue>,
    },
    /// An enum variant, optionally carrying data.
    Enum {
        /// The variant name.
        variant: String,
        /// Optional associated data.
        data: Option<Box<ConstValue>>,
    },
    /// A function pointer (by name) for const fn references.
    FnPtr(String),
    /// The null/unit value.
    Null,
}

impl ConstValue {
    /// Returns a human-readable type name for error messages.
    pub fn type_name(&self) -> &'static str {
        match self {
            ConstValue::Int(_) => "int",
            ConstValue::Float(_) => "float",
            ConstValue::Bool(_) => "bool",
            ConstValue::Str(_) => "str",
            ConstValue::Array(_) => "array",
            ConstValue::Tuple(_) => "tuple",
            ConstValue::Struct { .. } => "struct",
            ConstValue::Enum { .. } => "enum",
            ConstValue::FnPtr(_) => "fn",
            ConstValue::Null => "null",
        }
    }

    /// Returns `true` if this value is truthy for control flow purposes.
    pub fn is_truthy(&self) -> bool {
        match self {
            ConstValue::Bool(b) => *b,
            ConstValue::Int(n) => *n != 0,
            ConstValue::Null => false,
            _ => true,
        }
    }
}

impl std::fmt::Display for ConstValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConstValue::Int(n) => write!(f, "{n}"),
            ConstValue::Float(v) => write!(f, "{v}"),
            ConstValue::Bool(b) => write!(f, "{b}"),
            ConstValue::Str(s) => write!(f, "\"{s}\""),
            ConstValue::Array(elems) => write_const_array(f, elems),
            ConstValue::Tuple(elems) => write_const_tuple(f, elems),
            ConstValue::Struct { name, .. } => write!(f, "{name} {{ ... }}"),
            ConstValue::Enum { variant, data } => write_const_enum(f, variant, data),
            ConstValue::FnPtr(name) => write!(f, "<fn {name}>"),
            ConstValue::Null => write!(f, "null"),
        }
    }
}

/// Formats a const array for display.
fn write_const_array(f: &mut std::fmt::Formatter<'_>, elems: &[ConstValue]) -> std::fmt::Result {
    write!(f, "[")?;
    for (i, elem) in elems.iter().enumerate() {
        if i > 0 {
            write!(f, ", ")?;
        }
        write!(f, "{elem}")?;
    }
    write!(f, "]")
}

/// Formats a const tuple for display.
fn write_const_tuple(f: &mut std::fmt::Formatter<'_>, elems: &[ConstValue]) -> std::fmt::Result {
    write!(f, "(")?;
    for (i, elem) in elems.iter().enumerate() {
        if i > 0 {
            write!(f, ", ")?;
        }
        write!(f, "{elem}")?;
    }
    write!(f, ")")
}

/// Formats a const enum variant for display.
fn write_const_enum(
    f: &mut std::fmt::Formatter<'_>,
    variant: &str,
    data: &Option<Box<ConstValue>>,
) -> std::fmt::Result {
    write!(f, "{variant}")?;
    if let Some(d) = data {
        write!(f, "({d})")?;
    }
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// BinaryOp / UnaryOp
// ═══════════════════════════════════════════════════════════════════════

/// Binary operators supported in compile-time expressions.
#[derive(Debug, Clone, PartialEq)]
pub enum ConstBinaryOp {
    /// Addition (`+`).
    Add,
    /// Subtraction (`-`).
    Sub,
    /// Multiplication (`*`).
    Mul,
    /// Division (`/`).
    Div,
    /// Remainder/modulus (`%`).
    Rem,
    /// Equality (`==`).
    Eq,
    /// Inequality (`!=`).
    Ne,
    /// Less than (`<`).
    Lt,
    /// Less than or equal (`<=`).
    Le,
    /// Greater than (`>`).
    Gt,
    /// Greater than or equal (`>=`).
    Ge,
    /// Logical AND (`&&`).
    And,
    /// Logical OR (`||`).
    Or,
    /// Bitwise AND (`&`).
    BitAnd,
    /// Bitwise OR (`|`).
    BitOr,
    /// Bitwise XOR (`^`).
    BitXor,
    /// Left shift (`<<`).
    Shl,
    /// Right shift (`>>`).
    Shr,
}

/// Unary operators supported in compile-time expressions.
#[derive(Debug, Clone, PartialEq)]
pub enum ConstUnaryOp {
    /// Arithmetic negation (`-`).
    Neg,
    /// Logical NOT (`!`).
    Not,
    /// Bitwise NOT (`~`).
    BitNot,
}

// ═══════════════════════════════════════════════════════════════════════
// ConstExpr (Sprint 5)
// ═══════════════════════════════════════════════════════════════════════

/// A compile-time expression that can be evaluated by [`ConstEval`].
///
/// This is a simplified AST subset containing only constructs that are
/// valid in `const fn` bodies and `comptime {}` blocks.
#[derive(Debug, Clone, PartialEq)]
pub enum ConstExpr {
    /// A literal value.
    Literal(ConstValue),
    /// A binary operation.
    BinaryOp {
        /// The operator.
        op: ConstBinaryOp,
        /// Left-hand side expression.
        lhs: Box<ConstExpr>,
        /// Right-hand side expression.
        rhs: Box<ConstExpr>,
    },
    /// A unary operation.
    UnaryOp {
        /// The operator.
        op: ConstUnaryOp,
        /// The operand expression.
        expr: Box<ConstExpr>,
    },
    /// A conditional expression (`if cond { then } else { else }`).
    If {
        /// The condition expression (must evaluate to bool).
        condition: Box<ConstExpr>,
        /// The then-branch expression.
        then_branch: Box<ConstExpr>,
        /// The else-branch expression.
        else_branch: Box<ConstExpr>,
    },
    /// A match expression over a const value.
    Match {
        /// The scrutinee expression.
        scrutinee: Box<ConstExpr>,
        /// Arms as (pattern_value, body) pairs; `None` pattern = wildcard.
        arms: Vec<(Option<ConstValue>, ConstExpr)>,
    },
    /// A compile-time function call.
    FnCall {
        /// The function name.
        name: String,
        /// The argument expressions.
        args: Vec<ConstExpr>,
    },
    /// A variable reference.
    Var(String),
    /// A field access on a struct value.
    Field {
        /// The struct expression.
        expr: Box<ConstExpr>,
        /// The field name.
        name: String,
    },
    /// An index operation on an array or tuple.
    Index {
        /// The collection expression.
        expr: Box<ConstExpr>,
        /// The index expression (must evaluate to int).
        index: Box<ConstExpr>,
    },
    /// An array constructor expression.
    Array(Vec<ConstExpr>),
    /// A tuple constructor expression.
    Tuple(Vec<ConstExpr>),
    /// A block of sequential expressions (last is the result).
    Block(Vec<ConstExpr>),
    /// A let binding within a const block.
    Let {
        /// The variable name.
        name: String,
        /// The initializer expression.
        value: Box<ConstExpr>,
    },
    /// A bounded for loop: `for name in start..end { body }`.
    ForLoop {
        /// The loop variable name.
        name: String,
        /// Start of the range (inclusive).
        start: Box<ConstExpr>,
        /// End of the range (exclusive).
        end: Box<ConstExpr>,
        /// The loop body expression.
        body: Box<ConstExpr>,
    },
}

// ═══════════════════════════════════════════════════════════════════════
// ConstFn definition
// ═══════════════════════════════════════════════════════════════════════

/// A compile-time function definition.
#[derive(Debug, Clone)]
pub struct ConstFnDef {
    /// The function name.
    pub name: String,
    /// Parameter names.
    pub params: Vec<String>,
    /// The function body expression.
    pub body: ConstExpr,
}

// ═══════════════════════════════════════════════════════════════════════
// ConstEval (Sprint 5)
// ═══════════════════════════════════════════════════════════════════════

/// Compile-time expression evaluator.
///
/// Evaluates `ConstExpr` trees into `ConstValue` results. Supports
/// arithmetic, comparison, logical operations, control flow, function
/// calls, and variable bindings within a scoped environment.
#[derive(Debug)]
pub struct ConstEval {
    /// Variable bindings in the current evaluation scope.
    env: Vec<HashMap<String, ConstValue>>,
    /// Registered compile-time functions.
    functions: HashMap<String, ConstFnDef>,
    /// Current recursion depth.
    depth: usize,
    /// Maximum allowed recursion depth.
    max_depth: usize,
}

impl ConstEval {
    /// Creates a new compile-time evaluator with default settings.
    pub fn new() -> Self {
        Self {
            env: vec![HashMap::new()],
            functions: HashMap::new(),
            depth: 0,
            max_depth: MAX_CONST_RECURSION,
        }
    }

    /// Registers a compile-time function for later evaluation.
    pub fn register_fn(&mut self, def: ConstFnDef) {
        self.functions.insert(def.name.clone(), def);
    }

    /// Evaluates a compile-time expression to a constant value.
    pub fn eval(&mut self, expr: &ConstExpr) -> Result<ConstValue, ConstError> {
        match expr {
            ConstExpr::Literal(v) => Ok(v.clone()),
            ConstExpr::BinaryOp { op, lhs, rhs } => self.eval_binary(op, lhs, rhs),
            ConstExpr::UnaryOp { op, expr } => self.eval_unary(op, expr),
            ConstExpr::If {
                condition,
                then_branch,
                else_branch,
            } => self.eval_if(condition, then_branch, else_branch),
            ConstExpr::Match { scrutinee, arms } => self.eval_match(scrutinee, arms),
            ConstExpr::FnCall { name, args } => self.eval_fn_call(name, args),
            ConstExpr::Var(name) => self.lookup_var(name),
            ConstExpr::Field { expr, name } => self.eval_field(expr, name),
            ConstExpr::Index { expr, index } => self.eval_index(expr, index),
            ConstExpr::Array(elems) => self.eval_array(elems),
            ConstExpr::Tuple(elems) => self.eval_tuple(elems),
            ConstExpr::Block(exprs) => self.eval_block(exprs),
            ConstExpr::Let { name, value } => self.eval_let(name, value),
            ConstExpr::ForLoop {
                name,
                start,
                end,
                body,
            } => self.eval_for_loop(name, start, end, body),
        }
    }

    /// Sets a variable in the current scope.
    pub fn set_var(&mut self, name: String, value: ConstValue) {
        if let Some(scope) = self.env.last_mut() {
            scope.insert(name, value);
        }
    }

    /// Pushes a new variable scope.
    fn push_scope(&mut self) {
        self.env.push(HashMap::new());
    }

    /// Pops the current variable scope.
    fn pop_scope(&mut self) {
        self.env.pop();
    }
}

impl Default for ConstEval {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// ConstEval — expression evaluation helpers (Sprint 5)
// ═══════════════════════════════════════════════════════════════════════

impl ConstEval {
    /// Evaluates a binary operation on two const expressions.
    fn eval_binary(
        &mut self,
        op: &ConstBinaryOp,
        lhs: &ConstExpr,
        rhs: &ConstExpr,
    ) -> Result<ConstValue, ConstError> {
        let left = self.eval(lhs)?;
        let right = self.eval(rhs)?;
        eval_binary_values(op, &left, &right)
    }

    /// Evaluates a unary operation on a const expression.
    fn eval_unary(
        &mut self,
        op: &ConstUnaryOp,
        expr: &ConstExpr,
    ) -> Result<ConstValue, ConstError> {
        let val = self.eval(expr)?;
        eval_unary_value(op, &val)
    }

    /// Evaluates an if/else expression at compile time.
    fn eval_if(
        &mut self,
        condition: &ConstExpr,
        then_branch: &ConstExpr,
        else_branch: &ConstExpr,
    ) -> Result<ConstValue, ConstError> {
        let cond = self.eval(condition)?;
        if cond.is_truthy() {
            self.eval(then_branch)
        } else {
            self.eval(else_branch)
        }
    }

    /// Evaluates a match expression at compile time.
    fn eval_match(
        &mut self,
        scrutinee: &ConstExpr,
        arms: &[(Option<ConstValue>, ConstExpr)],
    ) -> Result<ConstValue, ConstError> {
        let value = self.eval(scrutinee)?;
        for (pattern, body) in arms {
            if match_const_pattern(&value, pattern) {
                return self.eval(body);
            }
        }
        Ok(ConstValue::Null)
    }

    /// Evaluates a compile-time function call.
    fn eval_fn_call(&mut self, name: &str, args: &[ConstExpr]) -> Result<ConstValue, ConstError> {
        if self.depth >= self.max_depth {
            return Err(ConstError::RecursionLimit {
                depth: self.depth,
                max: self.max_depth,
            });
        }
        let evaluated_args = self.eval_arg_list(args)?;
        let def = self
            .functions
            .get(name)
            .cloned()
            .ok_or_else(|| ConstError::UndefinedFn {
                name: name.to_string(),
            })?;
        self.call_const_fn(&def, &evaluated_args)
    }

    /// Evaluates a list of argument expressions.
    fn eval_arg_list(&mut self, args: &[ConstExpr]) -> Result<Vec<ConstValue>, ConstError> {
        args.iter().map(|a| self.eval(a)).collect()
    }

    /// Invokes a const fn definition with the given argument values.
    fn call_const_fn(
        &mut self,
        def: &ConstFnDef,
        args: &[ConstValue],
    ) -> Result<ConstValue, ConstError> {
        self.depth += 1;
        self.push_scope();
        for (param, arg) in def.params.iter().zip(args.iter()) {
            self.set_var(param.clone(), arg.clone());
        }
        let result = self.eval(&def.body);
        self.pop_scope();
        self.depth -= 1;
        result
    }
}

// ═══════════════════════════════════════════════════════════════════════
// ConstEval — variable, field, index, array, tuple, block (Sprint 5)
// ═══════════════════════════════════════════════════════════════════════

impl ConstEval {
    /// Looks up a variable by searching scopes from innermost to outermost.
    fn lookup_var(&self, name: &str) -> Result<ConstValue, ConstError> {
        for scope in self.env.iter().rev() {
            if let Some(val) = scope.get(name) {
                return Ok(val.clone());
            }
        }
        Err(ConstError::UndefinedVar {
            name: name.to_string(),
        })
    }

    /// Evaluates a field access on a struct const value.
    fn eval_field(&mut self, expr: &ConstExpr, name: &str) -> Result<ConstValue, ConstError> {
        let val = self.eval(expr)?;
        match val {
            ConstValue::Struct { fields, .. } => {
                fields
                    .get(name)
                    .cloned()
                    .ok_or_else(|| ConstError::UndefinedVar {
                        name: name.to_string(),
                    })
            }
            other => Err(ConstError::TypeMismatch {
                expected: "struct".to_string(),
                got: other.type_name().to_string(),
            }),
        }
    }

    /// Evaluates an index operation on an array or tuple.
    fn eval_index(
        &mut self,
        expr: &ConstExpr,
        index: &ConstExpr,
    ) -> Result<ConstValue, ConstError> {
        let collection = self.eval(expr)?;
        let idx_val = self.eval(index)?;
        let idx = extract_index(&idx_val)?;
        index_into_collection(&collection, idx)
    }

    /// Evaluates an array constructor.
    fn eval_array(&mut self, elems: &[ConstExpr]) -> Result<ConstValue, ConstError> {
        let values: Vec<ConstValue> = elems
            .iter()
            .map(|e| self.eval(e))
            .collect::<Result<_, _>>()?;
        Ok(ConstValue::Array(values))
    }

    /// Evaluates a tuple constructor.
    fn eval_tuple(&mut self, elems: &[ConstExpr]) -> Result<ConstValue, ConstError> {
        let values: Vec<ConstValue> = elems
            .iter()
            .map(|e| self.eval(e))
            .collect::<Result<_, _>>()?;
        Ok(ConstValue::Tuple(values))
    }

    /// Evaluates a block of sequential expressions.
    fn eval_block(&mut self, exprs: &[ConstExpr]) -> Result<ConstValue, ConstError> {
        self.push_scope();
        let mut result = ConstValue::Null;
        for expr in exprs {
            result = self.eval(expr)?;
        }
        self.pop_scope();
        Ok(result)
    }

    /// Evaluates a let binding; updates existing binding or creates new.
    fn eval_let(&mut self, name: &str, value: &ConstExpr) -> Result<ConstValue, ConstError> {
        let val = self.eval(value)?;
        self.assign_or_set(name.to_string(), val);
        Ok(ConstValue::Null)
    }

    /// Assigns to an existing variable in any scope, or creates in current scope.
    fn assign_or_set(&mut self, name: String, value: ConstValue) {
        for scope in self.env.iter_mut().rev() {
            if let std::collections::hash_map::Entry::Occupied(mut e) = scope.entry(name.clone()) {
                e.insert(value);
                return;
            }
        }
        if let Some(scope) = self.env.last_mut() {
            scope.insert(name, value);
        }
    }

    /// Evaluates a bounded for loop at compile time.
    fn eval_for_loop(
        &mut self,
        name: &str,
        start: &ConstExpr,
        end: &ConstExpr,
        body: &ConstExpr,
    ) -> Result<ConstValue, ConstError> {
        let start_val = extract_int(&self.eval(start)?)?;
        let end_val = extract_int(&self.eval(end)?)?;
        let iter_count = (end_val - start_val).unsigned_abs() as usize;
        if iter_count > MAX_CONST_LOOP_ITERS {
            return Err(ConstError::LoopLimit {
                max: MAX_CONST_LOOP_ITERS,
            });
        }
        self.push_scope();
        let mut result = ConstValue::Null;
        for i in start_val..end_val {
            self.set_var(name.to_string(), ConstValue::Int(i));
            result = self.eval(body)?;
        }
        self.pop_scope();
        Ok(result)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Pure evaluation helpers (Sprint 5)
// ═══════════════════════════════════════════════════════════════════════

/// Extracts an i128 from a ConstValue, returning an error on type mismatch.
fn extract_int(val: &ConstValue) -> Result<i128, ConstError> {
    match val {
        ConstValue::Int(n) => Ok(*n),
        other => Err(ConstError::TypeMismatch {
            expected: "int".to_string(),
            got: other.type_name().to_string(),
        }),
    }
}

/// Extracts a usize index from a ConstValue::Int.
fn extract_index(val: &ConstValue) -> Result<usize, ConstError> {
    let n = extract_int(val)?;
    if n < 0 {
        return Err(ConstError::TypeMismatch {
            expected: "non-negative index".to_string(),
            got: format!("{n}"),
        });
    }
    Ok(n as usize)
}

/// Indexes into a const array or tuple.
fn index_into_collection(collection: &ConstValue, idx: usize) -> Result<ConstValue, ConstError> {
    match collection {
        ConstValue::Array(elems) | ConstValue::Tuple(elems) => {
            elems.get(idx).cloned().ok_or_else(|| ConstError::NotConst {
                reason: format!("index {idx} out of bounds (len {})", elems.len()),
            })
        }
        other => Err(ConstError::TypeMismatch {
            expected: "array or tuple".to_string(),
            got: other.type_name().to_string(),
        }),
    }
}

/// Checks if a const value matches a pattern (None = wildcard).
fn match_const_pattern(value: &ConstValue, pattern: &Option<ConstValue>) -> bool {
    match pattern {
        None => true,
        Some(p) => value == p,
    }
}

/// Evaluates a binary operation on two resolved const values.
fn eval_binary_values(
    op: &ConstBinaryOp,
    left: &ConstValue,
    right: &ConstValue,
) -> Result<ConstValue, ConstError> {
    match (op, left, right) {
        // Integer arithmetic
        (ConstBinaryOp::Add, ConstValue::Int(a), ConstValue::Int(b)) => a
            .checked_add(*b)
            .map(ConstValue::Int)
            .ok_or(ConstError::Overflow),
        (ConstBinaryOp::Sub, ConstValue::Int(a), ConstValue::Int(b)) => a
            .checked_sub(*b)
            .map(ConstValue::Int)
            .ok_or(ConstError::Overflow),
        (ConstBinaryOp::Mul, ConstValue::Int(a), ConstValue::Int(b)) => a
            .checked_mul(*b)
            .map(ConstValue::Int)
            .ok_or(ConstError::Overflow),
        (ConstBinaryOp::Div, ConstValue::Int(_), ConstValue::Int(0)) => Err(ConstError::DivByZero),
        (ConstBinaryOp::Div, ConstValue::Int(a), ConstValue::Int(b)) => a
            .checked_div(*b)
            .map(ConstValue::Int)
            .ok_or(ConstError::Overflow),
        (ConstBinaryOp::Rem, ConstValue::Int(_), ConstValue::Int(0)) => Err(ConstError::DivByZero),
        (ConstBinaryOp::Rem, ConstValue::Int(a), ConstValue::Int(b)) => a
            .checked_rem(*b)
            .map(ConstValue::Int)
            .ok_or(ConstError::Overflow),
        // Float arithmetic
        (ConstBinaryOp::Add, ConstValue::Float(a), ConstValue::Float(b)) => {
            Ok(ConstValue::Float(a + b))
        }
        (ConstBinaryOp::Sub, ConstValue::Float(a), ConstValue::Float(b)) => {
            Ok(ConstValue::Float(a - b))
        }
        (ConstBinaryOp::Mul, ConstValue::Float(a), ConstValue::Float(b)) => {
            Ok(ConstValue::Float(a * b))
        }
        (ConstBinaryOp::Div, ConstValue::Float(_), ConstValue::Float(b)) if *b == 0.0 => {
            Err(ConstError::DivByZero)
        }
        (ConstBinaryOp::Div, ConstValue::Float(a), ConstValue::Float(b)) => {
            Ok(ConstValue::Float(a / b))
        }
        // String concatenation
        (ConstBinaryOp::Add, ConstValue::Str(a), ConstValue::Str(b)) => {
            Ok(ConstValue::Str(format!("{a}{b}")))
        }
        // Comparison (integer)
        (ConstBinaryOp::Eq, ConstValue::Int(a), ConstValue::Int(b)) => Ok(ConstValue::Bool(a == b)),
        (ConstBinaryOp::Ne, ConstValue::Int(a), ConstValue::Int(b)) => Ok(ConstValue::Bool(a != b)),
        (ConstBinaryOp::Lt, ConstValue::Int(a), ConstValue::Int(b)) => Ok(ConstValue::Bool(a < b)),
        (ConstBinaryOp::Le, ConstValue::Int(a), ConstValue::Int(b)) => Ok(ConstValue::Bool(a <= b)),
        (ConstBinaryOp::Gt, ConstValue::Int(a), ConstValue::Int(b)) => Ok(ConstValue::Bool(a > b)),
        (ConstBinaryOp::Ge, ConstValue::Int(a), ConstValue::Int(b)) => Ok(ConstValue::Bool(a >= b)),
        // Comparison (float)
        (ConstBinaryOp::Lt, ConstValue::Float(a), ConstValue::Float(b)) => {
            Ok(ConstValue::Bool(a < b))
        }
        (ConstBinaryOp::Eq, ConstValue::Float(a), ConstValue::Float(b)) => {
            Ok(ConstValue::Bool((a - b).abs() < f64::EPSILON))
        }
        // Logical operators
        (ConstBinaryOp::And, ConstValue::Bool(a), ConstValue::Bool(b)) => {
            Ok(ConstValue::Bool(*a && *b))
        }
        (ConstBinaryOp::Or, ConstValue::Bool(a), ConstValue::Bool(b)) => {
            Ok(ConstValue::Bool(*a || *b))
        }
        // Bitwise operators
        (ConstBinaryOp::BitAnd, ConstValue::Int(a), ConstValue::Int(b)) => {
            Ok(ConstValue::Int(a & b))
        }
        (ConstBinaryOp::BitOr, ConstValue::Int(a), ConstValue::Int(b)) => {
            Ok(ConstValue::Int(a | b))
        }
        (ConstBinaryOp::BitXor, ConstValue::Int(a), ConstValue::Int(b)) => {
            Ok(ConstValue::Int(a ^ b))
        }
        (ConstBinaryOp::Shl, ConstValue::Int(a), ConstValue::Int(b)) => eval_shift_left(*a, *b),
        (ConstBinaryOp::Shr, ConstValue::Int(a), ConstValue::Int(b)) => eval_shift_right(*a, *b),
        // Bool equality
        (ConstBinaryOp::Eq, ConstValue::Bool(a), ConstValue::Bool(b)) => {
            Ok(ConstValue::Bool(a == b))
        }
        (ConstBinaryOp::Ne, ConstValue::Bool(a), ConstValue::Bool(b)) => {
            Ok(ConstValue::Bool(a != b))
        }
        // String equality
        (ConstBinaryOp::Eq, ConstValue::Str(a), ConstValue::Str(b)) => Ok(ConstValue::Bool(a == b)),
        (ConstBinaryOp::Ne, ConstValue::Str(a), ConstValue::Str(b)) => Ok(ConstValue::Bool(a != b)),
        _ => Err(ConstError::TypeMismatch {
            expected: format!("compatible types for {op:?}"),
            got: format!("{} and {}", left.type_name(), right.type_name()),
        }),
    }
}

/// Evaluates a left shift, checking for excessive shift amounts.
fn eval_shift_left(a: i128, b: i128) -> Result<ConstValue, ConstError> {
    if !(0..128).contains(&b) {
        return Err(ConstError::Overflow);
    }
    Ok(ConstValue::Int(a << (b as u32)))
}

/// Evaluates a right shift, checking for excessive shift amounts.
fn eval_shift_right(a: i128, b: i128) -> Result<ConstValue, ConstError> {
    if !(0..128).contains(&b) {
        return Err(ConstError::Overflow);
    }
    Ok(ConstValue::Int(a >> (b as u32)))
}

/// Evaluates a unary operation on a resolved const value.
fn eval_unary_value(op: &ConstUnaryOp, val: &ConstValue) -> Result<ConstValue, ConstError> {
    match (op, val) {
        (ConstUnaryOp::Neg, ConstValue::Int(n)) => n
            .checked_neg()
            .map(ConstValue::Int)
            .ok_or(ConstError::Overflow),
        (ConstUnaryOp::Neg, ConstValue::Float(f)) => Ok(ConstValue::Float(-f)),
        (ConstUnaryOp::Not, ConstValue::Bool(b)) => Ok(ConstValue::Bool(!b)),
        (ConstUnaryOp::BitNot, ConstValue::Int(n)) => Ok(ConstValue::Int(!n)),
        _ => Err(ConstError::TypeMismatch {
            expected: format!("operand compatible with {op:?}"),
            got: val.type_name().to_string(),
        }),
    }
}

/// Validates that a const expression contains no non-const operations.
///
/// Returns `Ok(())` if the expression is const-evaluable, or an error
/// describing why it is not. IO, mutable global state, and unbounded
/// loops are rejected.
pub fn validate_const_expr(expr: &ConstExpr) -> Result<(), ConstError> {
    match expr {
        ConstExpr::Literal(_) | ConstExpr::Var(_) => Ok(()),
        ConstExpr::BinaryOp { lhs, rhs, .. } => {
            validate_const_expr(lhs)?;
            validate_const_expr(rhs)
        }
        ConstExpr::UnaryOp { expr, .. } => validate_const_expr(expr),
        ConstExpr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            validate_const_expr(condition)?;
            validate_const_expr(then_branch)?;
            validate_const_expr(else_branch)
        }
        ConstExpr::Match { scrutinee, arms } => {
            validate_const_expr(scrutinee)?;
            for (_, body) in arms {
                validate_const_expr(body)?;
            }
            Ok(())
        }
        ConstExpr::FnCall { name, args } => {
            validate_const_fn_call(name)?;
            for arg in args {
                validate_const_expr(arg)?;
            }
            Ok(())
        }
        ConstExpr::Field { expr, .. } => validate_const_expr(expr),
        ConstExpr::Index { expr, index } => {
            validate_const_expr(expr)?;
            validate_const_expr(index)
        }
        ConstExpr::Array(elems) | ConstExpr::Tuple(elems) => {
            for elem in elems {
                validate_const_expr(elem)?;
            }
            Ok(())
        }
        ConstExpr::Block(exprs) => {
            for e in exprs {
                validate_const_expr(e)?;
            }
            Ok(())
        }
        ConstExpr::Let { value, .. } => validate_const_expr(value),
        ConstExpr::ForLoop {
            start, end, body, ..
        } => {
            validate_const_expr(start)?;
            validate_const_expr(end)?;
            validate_const_expr(body)
        }
    }
}

/// Validates that a function name is allowed in const context.
fn validate_const_fn_call(name: &str) -> Result<(), ConstError> {
    const BANNED_FNS: &[&str] = &[
        "print",
        "println",
        "eprintln",
        "read_file",
        "write_file",
        "append_file",
        "panic",
        "todo",
    ];
    if BANNED_FNS.contains(&name) {
        return Err(ConstError::NotConst {
            reason: format!("function '{name}' performs IO and is not const-evaluable"),
        });
    }
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// ComptimeBlock (Sprint 6)
// ═══════════════════════════════════════════════════════════════════════

/// Represents a `comptime { }` block in source code.
///
/// Comptime blocks are evaluated during compilation and their results
/// are inlined into the program as constant values.
#[derive(Debug, Clone)]
pub struct ComptimeBlock {
    /// The block's body expression.
    pub body: ConstExpr,
    /// Whether the result should be stringified.
    pub stringify: bool,
}

impl ComptimeBlock {
    /// Creates a new comptime block with the given body.
    pub fn new(body: ConstExpr) -> Self {
        Self {
            body,
            stringify: false,
        }
    }

    /// Creates a new comptime block that stringifies its result.
    pub fn new_stringify(body: ConstExpr) -> Self {
        Self {
            body,
            stringify: true,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// ComptimeInterpreter (Sprint 6)
// ═══════════════════════════════════════════════════════════════════════

/// Subset interpreter for compile-time block execution.
///
/// Extends [`ConstEval`] with string manipulation, array generation,
/// and compile-time assertions. Used to evaluate `comptime { }` blocks
/// during the compilation phase.
#[derive(Debug)]
pub struct ComptimeInterpreter {
    /// The underlying const evaluator.
    evaluator: ConstEval,
}

impl ComptimeInterpreter {
    /// Creates a new comptime interpreter.
    pub fn new() -> Self {
        Self {
            evaluator: ConstEval::new(),
        }
    }

    /// Creates a comptime interpreter sharing the given evaluator's state.
    pub fn with_evaluator(evaluator: ConstEval) -> Self {
        Self { evaluator }
    }

    /// Evaluates a comptime block and returns the resulting const value.
    pub fn eval_block(&mut self, block: &ComptimeBlock) -> Result<ConstValue, ConstError> {
        let result = self.evaluator.eval(&block.body)?;
        if block.stringify {
            return Ok(ConstValue::Str(format!("{result}")));
        }
        Ok(result)
    }

    /// Registers a const fn definition for use within comptime blocks.
    pub fn register_fn(&mut self, def: ConstFnDef) {
        self.evaluator.register_fn(def);
    }

    /// Concatenates two compile-time strings.
    pub fn concat_strings(&self, a: &ConstValue, b: &ConstValue) -> Result<ConstValue, ConstError> {
        match (a, b) {
            (ConstValue::Str(sa), ConstValue::Str(sb)) => Ok(ConstValue::Str(format!("{sa}{sb}"))),
            _ => Err(ConstError::TypeMismatch {
                expected: "str".to_string(),
                got: format!("{} and {}", a.type_name(), b.type_name()),
            }),
        }
    }

    /// Formats a string by replacing `{}` placeholders with stringified values.
    pub fn format_string(
        &self,
        template: &str,
        args: &[ConstValue],
    ) -> Result<ConstValue, ConstError> {
        let mut result = template.to_string();
        for arg in args {
            if let Some(pos) = result.find("{}") {
                let replacement = format!("{arg}");
                result.replace_range(pos..pos + 2, &replacement);
            }
        }
        Ok(ConstValue::Str(result))
    }

    /// Stringifies a const value to its string representation.
    pub fn stringify(&self, value: &ConstValue) -> ConstValue {
        ConstValue::Str(format!("{value}"))
    }

    /// Generates a lookup table array from a range and a mapping function.
    pub fn generate_lookup_table(
        &mut self,
        size: usize,
        generator: &ConstExpr,
    ) -> Result<ConstValue, ConstError> {
        if size > MAX_CONST_LOOP_ITERS {
            return Err(ConstError::LoopLimit {
                max: MAX_CONST_LOOP_ITERS,
            });
        }
        let mut table = Vec::with_capacity(size);
        for i in 0..size {
            self.evaluator
                .set_var("_idx".to_string(), ConstValue::Int(i as i128));
            let val = self.evaluator.eval(generator)?;
            table.push(val);
        }
        Ok(ConstValue::Array(table))
    }

    /// Performs a compile-time assertion. Returns an error if the condition is false.
    pub fn comptime_assert(
        &mut self,
        condition: &ConstExpr,
        message: &str,
    ) -> Result<(), ConstError> {
        let val = self.evaluator.eval(condition)?;
        if !val.is_truthy() {
            return Err(ConstError::AssertionFailed {
                message: message.to_string(),
            });
        }
        Ok(())
    }

    /// Sets a variable in the evaluator for use within comptime blocks.
    pub fn set_var(&mut self, name: String, value: ConstValue) {
        self.evaluator.set_var(name, value);
    }
}

impl Default for ComptimeInterpreter {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Compile-Time Tensor Shapes (Sprint 7)
// ═══════════════════════════════════════════════════════════════════════

/// A single dimension in a compile-time tensor shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShapeDim {
    /// A dimension whose size is known at compile time.
    Const(usize),
    /// A dimension whose size is determined at runtime.
    Dynamic,
}

impl std::fmt::Display for ShapeDim {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ShapeDim::Const(n) => write!(f, "{n}"),
            ShapeDim::Dynamic => write!(f, "?"),
        }
    }
}

/// A compile-time tensor shape: an ordered list of dimensions.
///
/// Supports shape arithmetic (matmul, broadcast, conv2d, reshape) for
/// compile-time verification of tensor operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shape {
    /// The ordered list of dimension sizes.
    pub dims: Vec<ShapeDim>,
}

impl Shape {
    /// Creates a new shape from a list of dimensions.
    pub fn new(dims: Vec<ShapeDim>) -> Self {
        Self { dims }
    }

    /// Creates a shape with all constant dimensions.
    pub fn from_const(sizes: &[usize]) -> Self {
        Self {
            dims: sizes.iter().map(|&s| ShapeDim::Const(s)).collect(),
        }
    }

    /// Returns the number of dimensions (rank).
    pub fn rank(&self) -> usize {
        self.dims.len()
    }

    /// Returns the total number of elements if all dims are const.
    pub fn num_elements(&self) -> Option<usize> {
        let mut total = 1usize;
        for dim in &self.dims {
            match dim {
                ShapeDim::Const(n) => total = total.checked_mul(*n)?,
                ShapeDim::Dynamic => return None,
            }
        }
        Some(total)
    }

    /// Returns `true` if all dimensions are statically known.
    pub fn is_fully_const(&self) -> bool {
        self.dims.iter().all(|d| matches!(d, ShapeDim::Const(_)))
    }
}

impl std::fmt::Display for Shape {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[")?;
        for (i, dim) in self.dims.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{dim}")?;
        }
        write!(f, "]")
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Shape arithmetic (Sprint 7)
// ═══════════════════════════════════════════════════════════════════════

/// Infers the output shape of a matrix multiplication: (M,K) x (K,N) -> (M,N).
///
/// Returns a `ConstError::ShapeMismatch` (TE009) if the inner dimensions
/// are incompatible.
pub fn matmul_shape(lhs: &Shape, rhs: &Shape) -> Result<Shape, ConstError> {
    if lhs.rank() != 2 || rhs.rank() != 2 {
        return Err(ConstError::ShapeMismatch {
            message: format!(
                "matmul requires 2D tensors, got {}D and {}D",
                lhs.rank(),
                rhs.rank()
            ),
        });
    }
    validate_matmul_inner(&lhs.dims[1], &rhs.dims[0])?;
    Ok(Shape::new(vec![lhs.dims[0].clone(), rhs.dims[1].clone()]))
}

/// Validates that the inner dimensions of a matmul are compatible.
fn validate_matmul_inner(lhs_inner: &ShapeDim, rhs_inner: &ShapeDim) -> Result<(), ConstError> {
    match (lhs_inner, rhs_inner) {
        (ShapeDim::Const(k1), ShapeDim::Const(k2)) if k1 != k2 => Err(ConstError::ShapeMismatch {
            message: format!("matmul inner dimension mismatch: {k1} vs {k2}"),
        }),
        _ => Ok(()),
    }
}

/// Validates that two shapes are compatible for element-wise operations.
///
/// Returns a `ConstError::ShapeMismatch` if the shapes differ and are not
/// broadcastable.
pub fn validate_elementwise_shapes(a: &Shape, b: &Shape) -> Result<(), ConstError> {
    if a.rank() != b.rank() {
        return Err(ConstError::ShapeMismatch {
            message: format!("rank mismatch: {} vs {}", a.rank(), b.rank()),
        });
    }
    for (i, (da, db)) in a.dims.iter().zip(b.dims.iter()).enumerate() {
        match (da, db) {
            (ShapeDim::Const(sa), ShapeDim::Const(sb)) if sa != sb => {
                return Err(ConstError::ShapeMismatch {
                    message: format!("dimension {i} mismatch: {sa} vs {sb}"),
                });
            }
            _ => {}
        }
    }
    Ok(())
}

/// Computes the broadcast output shape using numpy-style broadcasting rules.
///
/// Dimensions are compared from the trailing end. A dimension of size 1
/// can be broadcast to match any other size.
pub fn broadcast_shape(a: &Shape, b: &Shape) -> Result<Shape, ConstError> {
    let max_rank = a.rank().max(b.rank());
    let mut result_dims = Vec::with_capacity(max_rank);

    let a_padded = pad_shape_left(a, max_rank);
    let b_padded = pad_shape_left(b, max_rank);

    for i in 0..max_rank {
        let dim = broadcast_dim(&a_padded[i], &b_padded[i], i)?;
        result_dims.push(dim);
    }
    Ok(Shape::new(result_dims))
}

/// Left-pads a shape with `Const(1)` dimensions to reach `target_rank`.
fn pad_shape_left(shape: &Shape, target_rank: usize) -> Vec<ShapeDim> {
    let pad_count = target_rank.saturating_sub(shape.rank());
    let mut padded = vec![ShapeDim::Const(1); pad_count];
    padded.extend(shape.dims.iter().cloned());
    padded
}

/// Broadcasts two dimensions together according to numpy rules.
fn broadcast_dim(a: &ShapeDim, b: &ShapeDim, dim_idx: usize) -> Result<ShapeDim, ConstError> {
    match (a, b) {
        (ShapeDim::Const(1), other) | (other, ShapeDim::Const(1)) => Ok(other.clone()),
        (ShapeDim::Const(sa), ShapeDim::Const(sb)) if sa == sb => Ok(ShapeDim::Const(*sa)),
        (ShapeDim::Const(sa), ShapeDim::Const(sb)) => Err(ConstError::ShapeMismatch {
            message: format!("cannot broadcast dimension {dim_idx}: {sa} vs {sb}"),
        }),
        (ShapeDim::Dynamic, _) | (_, ShapeDim::Dynamic) => Ok(ShapeDim::Dynamic),
    }
}

/// Computes the output shape of a Conv2d operation.
///
/// Formula: `out_dim = (input_dim - kernel_dim + 2 * padding) / stride + 1`
///
/// Input shape: `(N, C_in, H, W)`
/// Kernel shape: `(C_out, C_in, K_h, K_w)`
/// Output shape: `(N, C_out, H_out, W_out)`
pub fn conv2d_output_shape(
    input: &Shape,
    kernel: &Shape,
    padding: (usize, usize),
    stride: (usize, usize),
) -> Result<Shape, ConstError> {
    validate_conv2d_ranks(input, kernel)?;
    let (h_out, w_out) = compute_conv2d_spatial(input, kernel, padding, stride)?;
    Ok(Shape::new(vec![
        input.dims[0].clone(),
        kernel.dims[0].clone(),
        h_out,
        w_out,
    ]))
}

/// Validates that input and kernel have the correct ranks for Conv2d.
fn validate_conv2d_ranks(input: &Shape, kernel: &Shape) -> Result<(), ConstError> {
    if input.rank() != 4 || kernel.rank() != 4 {
        return Err(ConstError::ShapeMismatch {
            message: format!(
                "conv2d requires 4D input and kernel, got {}D and {}D",
                input.rank(),
                kernel.rank()
            ),
        });
    }
    Ok(())
}

/// Computes the spatial output dimensions (H_out, W_out) for Conv2d.
fn compute_conv2d_spatial(
    input: &Shape,
    kernel: &Shape,
    padding: (usize, usize),
    stride: (usize, usize),
) -> Result<(ShapeDim, ShapeDim), ConstError> {
    let h_out = compute_single_conv_dim(&input.dims[2], &kernel.dims[2], padding.0, stride.0)?;
    let w_out = compute_single_conv_dim(&input.dims[3], &kernel.dims[3], padding.1, stride.1)?;
    Ok((h_out, w_out))
}

/// Computes a single conv output dimension: `(in - k + 2*p) / s + 1`.
fn compute_single_conv_dim(
    input_dim: &ShapeDim,
    kernel_dim: &ShapeDim,
    padding: usize,
    stride: usize,
) -> Result<ShapeDim, ConstError> {
    match (input_dim, kernel_dim) {
        (ShapeDim::Const(i), ShapeDim::Const(k)) => {
            let numerator =
                (*i + 2 * padding)
                    .checked_sub(*k)
                    .ok_or_else(|| ConstError::ShapeMismatch {
                        message: format!(
                            "kernel size {k} exceeds input size {i} + 2*padding {padding}"
                        ),
                    })?;
            if stride == 0 {
                return Err(ConstError::DivByZero);
            }
            Ok(ShapeDim::Const(numerator / stride + 1))
        }
        _ => Ok(ShapeDim::Dynamic),
    }
}

/// Validates that a reshape operation preserves the total element count.
///
/// Returns an error if the source and target shapes have different numbers
/// of elements (when both are fully const).
pub fn validate_reshape(source: &Shape, target: &Shape) -> Result<(), ConstError> {
    let src_elems = source.num_elements();
    let tgt_elems = target.num_elements();
    match (src_elems, tgt_elems) {
        (Some(s), Some(t)) if s != t => Err(ConstError::ShapeMismatch {
            message: format!("reshape element count mismatch: source has {s}, target has {t}"),
        }),
        _ => Ok(()),
    }
}

/// Infers the output shape of a layer chain: Dense(in, out) -> Dense(out, out2).
///
/// Given input features and a sequence of `(in_features, out_features)` layer
/// specs, validates that each layer's input matches the previous output.
pub fn layer_chain_shape(
    input_features: usize,
    layers: &[(usize, usize)],
) -> Result<usize, ConstError> {
    let mut current = input_features;
    for (i, &(layer_in, layer_out)) in layers.iter().enumerate() {
        if current != layer_in {
            return Err(ConstError::ShapeMismatch {
                message: format!("layer {i}: expected input {current}, got {layer_in}"),
            });
        }
        current = layer_out;
    }
    Ok(current)
}

// ═══════════════════════════════════════════════════════════════════════
// ConstCache (Sprint 8)
// ═══════════════════════════════════════════════════════════════════════

/// A cache key for memoizing const fn results.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ConstCacheKey {
    /// The function name.
    fn_name: String,
    /// Serialized argument values.
    args_key: String,
}

/// Memoization cache for compile-time function evaluation results.
///
/// Caches the results of const fn calls to avoid redundant re-evaluation
/// of the same function with the same arguments.
#[derive(Debug)]
pub struct ConstCache {
    /// Cached results keyed by function name + argument values.
    entries: HashMap<ConstCacheKey, ConstValue>,
    /// Number of cache hits.
    hits: u64,
    /// Number of cache misses.
    misses: u64,
}

impl ConstCache {
    /// Creates a new empty const cache.
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            hits: 0,
            misses: 0,
        }
    }

    /// Looks up a cached result for the given function call.
    pub fn lookup(&mut self, fn_name: &str, args: &[ConstValue]) -> Option<ConstValue> {
        let key = make_cache_key(fn_name, args);
        match self.entries.get(&key) {
            Some(val) => {
                self.hits += 1;
                Some(val.clone())
            }
            None => {
                self.misses += 1;
                None
            }
        }
    }

    /// Stores a result in the cache for the given function call.
    pub fn store(&mut self, fn_name: &str, args: &[ConstValue], result: ConstValue) {
        let key = make_cache_key(fn_name, args);
        self.entries.insert(key, result);
    }

    /// Returns the number of entries in the cache.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if the cache contains no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns the number of cache hits.
    pub fn hit_count(&self) -> u64 {
        self.hits
    }

    /// Returns the number of cache misses.
    pub fn miss_count(&self) -> u64 {
        self.misses
    }

    /// Returns the cache hit rate as a percentage.
    pub fn hit_rate_pct(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            return 0.0;
        }
        (self.hits as f64 / total as f64) * 100.0
    }

    /// Clears all cached entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

impl Default for ConstCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Creates a cache key from a function name and argument values.
fn make_cache_key(fn_name: &str, args: &[ConstValue]) -> ConstCacheKey {
    let args_key = args
        .iter()
        .map(|a| format!("{a}"))
        .collect::<Vec<_>>()
        .join(",");
    ConstCacheKey {
        fn_name: fn_name.to_string(),
        args_key,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Const eval metrics (Sprint 8)
// ═══════════════════════════════════════════════════════════════════════

/// Timing and usage metrics for compile-time evaluation.
#[derive(Debug, Clone)]
pub struct ConstEvalMetrics {
    /// Total number of const expressions evaluated.
    pub expressions_evaluated: u64,
    /// Total number of const fn calls.
    pub fn_calls: u64,
    /// Total evaluation time in microseconds.
    pub total_time_us: u64,
    /// Peak recursion depth reached.
    pub peak_recursion_depth: usize,
    /// Number of cache hits (if caching enabled).
    pub cache_hits: u64,
    /// Number of cache misses (if caching enabled).
    pub cache_misses: u64,
}

impl ConstEvalMetrics {
    /// Creates zeroed metrics.
    pub fn new() -> Self {
        Self {
            expressions_evaluated: 0,
            fn_calls: 0,
            total_time_us: 0,
            peak_recursion_depth: 0,
            cache_hits: 0,
            cache_misses: 0,
        }
    }
}

impl Default for ConstEvalMetrics {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// CachedConstEval (Sprint 8)
// ═══════════════════════════════════════════════════════════════════════

/// A compile-time evaluator with memoization caching and metrics.
///
/// Wraps [`ConstEval`] with a [`ConstCache`] to avoid redundant evaluation
/// and collects [`ConstEvalMetrics`] for performance analysis.
#[derive(Debug)]
pub struct CachedConstEval {
    /// The underlying evaluator.
    evaluator: ConstEval,
    /// Memoization cache for fn call results.
    cache: ConstCache,
    /// Evaluation metrics.
    metrics: ConstEvalMetrics,
}

impl CachedConstEval {
    /// Creates a new cached evaluator.
    pub fn new() -> Self {
        Self {
            evaluator: ConstEval::new(),
            cache: ConstCache::new(),
            metrics: ConstEvalMetrics::new(),
        }
    }

    /// Registers a compile-time function definition.
    pub fn register_fn(&mut self, def: ConstFnDef) {
        self.evaluator.register_fn(def);
    }

    /// Evaluates a const expression, using the cache for fn calls.
    pub fn eval(&mut self, expr: &ConstExpr) -> Result<ConstValue, ConstError> {
        let start = Instant::now();
        self.metrics.expressions_evaluated += 1;
        let result = self.eval_inner(expr);
        let elapsed = start.elapsed().as_micros() as u64;
        self.metrics.total_time_us += elapsed;
        result
    }

    /// Inner evaluation that dispatches to cache-aware fn call handling.
    fn eval_inner(&mut self, expr: &ConstExpr) -> Result<ConstValue, ConstError> {
        match expr {
            ConstExpr::FnCall { name, args } => self.eval_cached_fn_call(name, args),
            other => self.evaluator.eval(other),
        }
    }

    /// Evaluates a fn call with cache lookup/store.
    fn eval_cached_fn_call(
        &mut self,
        name: &str,
        args: &[ConstExpr],
    ) -> Result<ConstValue, ConstError> {
        self.metrics.fn_calls += 1;
        let evaluated_args = self.eval_fn_args(args)?;
        if let Some(cached) = self.cache.lookup(name, &evaluated_args) {
            self.metrics.cache_hits += 1;
            return Ok(cached);
        }
        self.metrics.cache_misses += 1;
        let result = self.evaluator.eval_fn_call(name, args)?;
        self.cache.store(name, &evaluated_args, result.clone());
        self.record_peak_depth();
        Ok(result)
    }

    /// Evaluates fn call argument expressions.
    fn eval_fn_args(&mut self, args: &[ConstExpr]) -> Result<Vec<ConstValue>, ConstError> {
        args.iter().map(|a| self.evaluator.eval(a)).collect()
    }

    /// Records the peak recursion depth if current is higher.
    fn record_peak_depth(&mut self) {
        if self.evaluator.depth > self.metrics.peak_recursion_depth {
            self.metrics.peak_recursion_depth = self.evaluator.depth;
        }
    }

    /// Returns a snapshot of the current evaluation metrics.
    pub fn metrics(&self) -> &ConstEvalMetrics {
        &self.metrics
    }

    /// Returns a reference to the underlying cache.
    pub fn cache(&self) -> &ConstCache {
        &self.cache
    }

    /// Sets a variable in the evaluator.
    pub fn set_var(&mut self, name: String, value: ConstValue) {
        self.evaluator.set_var(name, value);
    }
}

impl Default for CachedConstEval {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Timed const evaluation helper (Sprint 8)
// ═══════════════════════════════════════════════════════════════════════

/// Evaluates a const expression and returns the result plus elapsed time in microseconds.
pub fn timed_eval(
    evaluator: &mut ConstEval,
    expr: &ConstExpr,
) -> Result<(ConstValue, u64), ConstError> {
    let start = Instant::now();
    let result = evaluator.eval(expr)?;
    let elapsed_us = start.elapsed().as_micros() as u64;
    Ok((result, elapsed_us))
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ───────────────────────────────────────────────────────────────
    // Sprint 5 — const fn Foundations (s5_1 through s5_10)
    // ───────────────────────────────────────────────────────────────

    #[test]
    fn s5_1_const_eval_int_arithmetic() {
        let mut eval = ConstEval::new();
        // (3 + 4) * 2 = 14
        let expr = ConstExpr::BinaryOp {
            op: ConstBinaryOp::Mul,
            lhs: Box::new(ConstExpr::BinaryOp {
                op: ConstBinaryOp::Add,
                lhs: Box::new(ConstExpr::Literal(ConstValue::Int(3))),
                rhs: Box::new(ConstExpr::Literal(ConstValue::Int(4))),
            }),
            rhs: Box::new(ConstExpr::Literal(ConstValue::Int(2))),
        };
        let result = eval.eval(&expr).unwrap();
        assert_eq!(result, ConstValue::Int(14));
    }

    #[test]
    fn s5_2_const_eval_float_arithmetic() {
        let mut eval = ConstEval::new();
        // 2.5 + 1.5 = 4.0
        let expr = ConstExpr::BinaryOp {
            op: ConstBinaryOp::Add,
            lhs: Box::new(ConstExpr::Literal(ConstValue::Float(2.5))),
            rhs: Box::new(ConstExpr::Literal(ConstValue::Float(1.5))),
        };
        let result = eval.eval(&expr).unwrap();
        assert_eq!(result, ConstValue::Float(4.0));
    }

    #[test]
    fn s5_3_const_eval_comparison_and_logical() {
        let mut eval = ConstEval::new();
        // (5 > 3) && (2 < 10) = true
        let expr = ConstExpr::BinaryOp {
            op: ConstBinaryOp::And,
            lhs: Box::new(ConstExpr::BinaryOp {
                op: ConstBinaryOp::Gt,
                lhs: Box::new(ConstExpr::Literal(ConstValue::Int(5))),
                rhs: Box::new(ConstExpr::Literal(ConstValue::Int(3))),
            }),
            rhs: Box::new(ConstExpr::BinaryOp {
                op: ConstBinaryOp::Lt,
                lhs: Box::new(ConstExpr::Literal(ConstValue::Int(2))),
                rhs: Box::new(ConstExpr::Literal(ConstValue::Int(10))),
            }),
        };
        let result = eval.eval(&expr).unwrap();
        assert_eq!(result, ConstValue::Bool(true));
    }

    #[test]
    fn s5_4_const_eval_if_else() {
        let mut eval = ConstEval::new();
        // if true { 42 } else { 0 }
        let expr = ConstExpr::If {
            condition: Box::new(ConstExpr::Literal(ConstValue::Bool(true))),
            then_branch: Box::new(ConstExpr::Literal(ConstValue::Int(42))),
            else_branch: Box::new(ConstExpr::Literal(ConstValue::Int(0))),
        };
        let result = eval.eval(&expr).unwrap();
        assert_eq!(result, ConstValue::Int(42));
    }

    #[test]
    fn s5_5_const_eval_match_expression() {
        let mut eval = ConstEval::new();
        // match 2 { 1 => 10, 2 => 20, _ => 0 }
        let expr = ConstExpr::Match {
            scrutinee: Box::new(ConstExpr::Literal(ConstValue::Int(2))),
            arms: vec![
                (
                    Some(ConstValue::Int(1)),
                    ConstExpr::Literal(ConstValue::Int(10)),
                ),
                (
                    Some(ConstValue::Int(2)),
                    ConstExpr::Literal(ConstValue::Int(20)),
                ),
                (None, ConstExpr::Literal(ConstValue::Int(0))),
            ],
        };
        let result = eval.eval(&expr).unwrap();
        assert_eq!(result, ConstValue::Int(20));
    }

    #[test]
    fn s5_6_const_eval_fn_call() {
        let mut eval = ConstEval::new();
        // const fn double(x) = x * 2; double(21) = 42
        eval.register_fn(ConstFnDef {
            name: "double".to_string(),
            params: vec!["x".to_string()],
            body: ConstExpr::BinaryOp {
                op: ConstBinaryOp::Mul,
                lhs: Box::new(ConstExpr::Var("x".to_string())),
                rhs: Box::new(ConstExpr::Literal(ConstValue::Int(2))),
            },
        });
        let expr = ConstExpr::FnCall {
            name: "double".to_string(),
            args: vec![ConstExpr::Literal(ConstValue::Int(21))],
        };
        let result = eval.eval(&expr).unwrap();
        assert_eq!(result, ConstValue::Int(42));
    }

    #[test]
    fn s5_7_const_eval_overflow_detection() {
        let mut eval = ConstEval::new();
        let expr = ConstExpr::BinaryOp {
            op: ConstBinaryOp::Add,
            lhs: Box::new(ConstExpr::Literal(ConstValue::Int(i128::MAX))),
            rhs: Box::new(ConstExpr::Literal(ConstValue::Int(1))),
        };
        let result = eval.eval(&expr);
        assert!(matches!(result, Err(ConstError::Overflow)));
    }

    #[test]
    fn s5_8_const_eval_div_by_zero() {
        let mut eval = ConstEval::new();
        let expr = ConstExpr::BinaryOp {
            op: ConstBinaryOp::Div,
            lhs: Box::new(ConstExpr::Literal(ConstValue::Int(10))),
            rhs: Box::new(ConstExpr::Literal(ConstValue::Int(0))),
        };
        let result = eval.eval(&expr);
        assert!(matches!(result, Err(ConstError::DivByZero)));
    }

    #[test]
    fn s5_9_const_validation_rejects_io() {
        let expr = ConstExpr::FnCall {
            name: "println".to_string(),
            args: vec![ConstExpr::Literal(ConstValue::Str("hello".to_string()))],
        };
        let result = validate_const_expr(&expr);
        assert!(matches!(result, Err(ConstError::NotConst { .. })));
    }

    #[test]
    fn s5_10_const_value_type_names_and_display() {
        assert_eq!(ConstValue::Int(42).type_name(), "int");
        assert_eq!(ConstValue::Float(3.14).type_name(), "float");
        assert_eq!(ConstValue::Bool(true).type_name(), "bool");
        assert_eq!(ConstValue::Str("hi".into()).type_name(), "str");
        assert_eq!(ConstValue::Null.type_name(), "null");
        assert_eq!(format!("{}", ConstValue::Int(42)), "42");
        assert_eq!(format!("{}", ConstValue::Bool(false)), "false");
        assert_eq!(format!("{}", ConstValue::Null), "null");
        assert_eq!(
            format!(
                "{}",
                ConstValue::Array(vec![ConstValue::Int(1), ConstValue::Int(2)])
            ),
            "[1, 2]"
        );
    }

    // ───────────────────────────────────────────────────────────────
    // Sprint 6 — comptime Blocks (s6_1 through s6_10)
    // ───────────────────────────────────────────────────────────────

    #[test]
    fn s6_1_comptime_block_basic_eval() {
        let mut interp = ComptimeInterpreter::new();
        let block = ComptimeBlock::new(ConstExpr::BinaryOp {
            op: ConstBinaryOp::Add,
            lhs: Box::new(ConstExpr::Literal(ConstValue::Int(10))),
            rhs: Box::new(ConstExpr::Literal(ConstValue::Int(20))),
        });
        let result = interp.eval_block(&block).unwrap();
        assert_eq!(result, ConstValue::Int(30));
    }

    #[test]
    fn s6_2_comptime_block_stringify() {
        let mut interp = ComptimeInterpreter::new();
        let block = ComptimeBlock::new_stringify(ConstExpr::Literal(ConstValue::Int(42)));
        let result = interp.eval_block(&block).unwrap();
        assert_eq!(result, ConstValue::Str("42".to_string()));
    }

    #[test]
    fn s6_3_comptime_string_concat() {
        let interp = ComptimeInterpreter::new();
        let a = ConstValue::Str("hello".to_string());
        let b = ConstValue::Str(" world".to_string());
        let result = interp.concat_strings(&a, &b).unwrap();
        assert_eq!(result, ConstValue::Str("hello world".to_string()));
    }

    #[test]
    fn s6_4_comptime_string_format() {
        let interp = ComptimeInterpreter::new();
        let result = interp
            .format_string("x={}, y={}", &[ConstValue::Int(1), ConstValue::Int(2)])
            .unwrap();
        assert_eq!(result, ConstValue::Str("x=1, y=2".to_string()));
    }

    #[test]
    fn s6_5_comptime_stringify_values() {
        let interp = ComptimeInterpreter::new();
        assert_eq!(
            interp.stringify(&ConstValue::Float(3.14)),
            ConstValue::Str("3.14".to_string())
        );
        assert_eq!(
            interp.stringify(&ConstValue::Bool(true)),
            ConstValue::Str("true".to_string())
        );
    }

    #[test]
    fn s6_6_comptime_lookup_table_generation() {
        let mut interp = ComptimeInterpreter::new();
        // Generate [0, 1, 4, 9, 16] (i * i for i in 0..5)
        let generator = ConstExpr::BinaryOp {
            op: ConstBinaryOp::Mul,
            lhs: Box::new(ConstExpr::Var("_idx".to_string())),
            rhs: Box::new(ConstExpr::Var("_idx".to_string())),
        };
        let result = interp.generate_lookup_table(5, &generator).unwrap();
        let expected = ConstValue::Array(vec![
            ConstValue::Int(0),
            ConstValue::Int(1),
            ConstValue::Int(4),
            ConstValue::Int(9),
            ConstValue::Int(16),
        ]);
        assert_eq!(result, expected);
    }

    #[test]
    fn s6_7_comptime_assert_pass() {
        let mut interp = ComptimeInterpreter::new();
        let cond = ConstExpr::BinaryOp {
            op: ConstBinaryOp::Eq,
            lhs: Box::new(ConstExpr::Literal(ConstValue::Int(2))),
            rhs: Box::new(ConstExpr::Literal(ConstValue::Int(2))),
        };
        let result = interp.comptime_assert(&cond, "2 should equal 2");
        assert!(result.is_ok());
    }

    #[test]
    fn s6_8_comptime_assert_fail() {
        let mut interp = ComptimeInterpreter::new();
        let cond = ConstExpr::BinaryOp {
            op: ConstBinaryOp::Gt,
            lhs: Box::new(ConstExpr::Literal(ConstValue::Int(1))),
            rhs: Box::new(ConstExpr::Literal(ConstValue::Int(5))),
        };
        let result = interp.comptime_assert(&cond, "1 > 5 is false");
        assert!(matches!(result, Err(ConstError::AssertionFailed { .. })));
    }

    #[test]
    fn s6_9_comptime_with_evaluator_state() {
        let mut eval = ConstEval::new();
        eval.set_var("BASE".to_string(), ConstValue::Int(100));
        let mut interp = ComptimeInterpreter::with_evaluator(eval);
        let block = ComptimeBlock::new(ConstExpr::BinaryOp {
            op: ConstBinaryOp::Add,
            lhs: Box::new(ConstExpr::Var("BASE".to_string())),
            rhs: Box::new(ConstExpr::Literal(ConstValue::Int(23))),
        });
        let result = interp.eval_block(&block).unwrap();
        assert_eq!(result, ConstValue::Int(123));
    }

    #[test]
    fn s6_10_comptime_register_and_call_fn() {
        let mut interp = ComptimeInterpreter::new();
        interp.register_fn(ConstFnDef {
            name: "square".to_string(),
            params: vec!["n".to_string()],
            body: ConstExpr::BinaryOp {
                op: ConstBinaryOp::Mul,
                lhs: Box::new(ConstExpr::Var("n".to_string())),
                rhs: Box::new(ConstExpr::Var("n".to_string())),
            },
        });
        let block = ComptimeBlock::new(ConstExpr::FnCall {
            name: "square".to_string(),
            args: vec![ConstExpr::Literal(ConstValue::Int(7))],
        });
        let result = interp.eval_block(&block).unwrap();
        assert_eq!(result, ConstValue::Int(49));
    }

    // ───────────────────────────────────────────────────────────────
    // Sprint 7 — Compile-Time Tensor Shapes (s7_1 through s7_10)
    // ───────────────────────────────────────────────────────────────

    #[test]
    fn s7_1_shape_creation_and_rank() {
        let shape = Shape::from_const(&[3, 4, 5]);
        assert_eq!(shape.rank(), 3);
        assert_eq!(shape.num_elements(), Some(60));
        assert!(shape.is_fully_const());
    }

    #[test]
    fn s7_2_matmul_shape_inference() {
        // (2,3) x (3,4) -> (2,4)
        let a = Shape::from_const(&[2, 3]);
        let b = Shape::from_const(&[3, 4]);
        let result = matmul_shape(&a, &b).unwrap();
        assert_eq!(result, Shape::from_const(&[2, 4]));
    }

    #[test]
    fn s7_3_matmul_shape_mismatch() {
        // (2,3) x (5,4) -> error: inner dim 3 != 5
        let a = Shape::from_const(&[2, 3]);
        let b = Shape::from_const(&[5, 4]);
        let result = matmul_shape(&a, &b);
        assert!(matches!(result, Err(ConstError::ShapeMismatch { .. })));
    }

    #[test]
    fn s7_4_broadcast_shape_same_rank() {
        // [3,1,5] broadcast [1,4,5] -> [3,4,5]
        let a = Shape::from_const(&[3, 1, 5]);
        let b = Shape::from_const(&[1, 4, 5]);
        let result = broadcast_shape(&a, &b).unwrap();
        assert_eq!(result, Shape::from_const(&[3, 4, 5]));
    }

    #[test]
    fn s7_5_broadcast_shape_different_rank() {
        // [5] broadcast [3,5] -> [3,5]
        let a = Shape::from_const(&[5]);
        let b = Shape::from_const(&[3, 5]);
        let result = broadcast_shape(&a, &b).unwrap();
        assert_eq!(result, Shape::from_const(&[3, 5]));
    }

    #[test]
    fn s7_6_broadcast_shape_incompatible() {
        // [3,4] broadcast [3,5] -> error
        let a = Shape::from_const(&[3, 4]);
        let b = Shape::from_const(&[3, 5]);
        let result = broadcast_shape(&a, &b);
        assert!(matches!(result, Err(ConstError::ShapeMismatch { .. })));
    }

    #[test]
    fn s7_7_conv2d_output_shape() {
        // Input: (1,1,28,28), Kernel: (16,1,5,5), padding=0, stride=1
        // Output: (1,16,24,24) because (28-5+0)/1+1=24
        let input = Shape::from_const(&[1, 1, 28, 28]);
        let kernel = Shape::from_const(&[16, 1, 5, 5]);
        let result = conv2d_output_shape(&input, &kernel, (0, 0), (1, 1)).unwrap();
        assert_eq!(result, Shape::from_const(&[1, 16, 24, 24]));
    }

    #[test]
    fn s7_8_conv2d_with_padding_and_stride() {
        // Input: (1,3,32,32), Kernel: (8,3,3,3), padding=1, stride=2
        // Output H: (32-3+2*1)/2+1 = 16, W: same = 16
        let input = Shape::from_const(&[1, 3, 32, 32]);
        let kernel = Shape::from_const(&[8, 3, 3, 3]);
        let result = conv2d_output_shape(&input, &kernel, (1, 1), (2, 2)).unwrap();
        assert_eq!(result, Shape::from_const(&[1, 8, 16, 16]));
    }

    #[test]
    fn s7_9_reshape_validation() {
        // [2,3,4] (24 elems) -> [6,4] (24 elems) OK
        let source = Shape::from_const(&[2, 3, 4]);
        let target = Shape::from_const(&[6, 4]);
        assert!(validate_reshape(&source, &target).is_ok());

        // [2,3,4] (24 elems) -> [5,5] (25 elems) FAIL
        let bad_target = Shape::from_const(&[5, 5]);
        assert!(matches!(
            validate_reshape(&source, &bad_target),
            Err(ConstError::ShapeMismatch { .. })
        ));
    }

    #[test]
    fn s7_10_layer_chain_inference() {
        // Dense(784,128) -> Dense(128,64) -> Dense(64,10)
        let layers = vec![(784, 128), (128, 64), (64, 10)];
        let output = layer_chain_shape(784, &layers).unwrap();
        assert_eq!(output, 10);

        // Mismatched chain: Dense(784,128) -> Dense(64,10) FAIL
        let bad_layers = vec![(784, 128), (64, 10)];
        let result = layer_chain_shape(784, &bad_layers);
        assert!(matches!(result, Err(ConstError::ShapeMismatch { .. })));
    }

    // ───────────────────────────────────────────────────────────────
    // Sprint 8 — Optimization (s8_1 through s8_10)
    // ───────────────────────────────────────────────────────────────

    #[test]
    fn s8_1_const_cache_store_and_lookup() {
        let mut cache = ConstCache::new();
        let args = vec![ConstValue::Int(5)];
        cache.store("factorial", &args, ConstValue::Int(120));

        let result = cache.lookup("factorial", &args);
        assert_eq!(result, Some(ConstValue::Int(120)));
        assert_eq!(cache.hit_count(), 1);
    }

    #[test]
    fn s8_2_const_cache_miss() {
        let mut cache = ConstCache::new();
        let result = cache.lookup("nonexistent", &[ConstValue::Int(1)]);
        assert_eq!(result, None);
        assert_eq!(cache.miss_count(), 1);
    }

    #[test]
    fn s8_3_const_recursion_limit() {
        let mut eval = ConstEval::new();
        // Register a recursive function that will exceed the limit
        eval.register_fn(ConstFnDef {
            name: "recurse".to_string(),
            params: vec!["n".to_string()],
            body: ConstExpr::FnCall {
                name: "recurse".to_string(),
                args: vec![ConstExpr::Var("n".to_string())],
            },
        });
        let expr = ConstExpr::FnCall {
            name: "recurse".to_string(),
            args: vec![ConstExpr::Literal(ConstValue::Int(0))],
        };
        let result = eval.eval(&expr);
        assert!(matches!(result, Err(ConstError::RecursionLimit { .. })));
    }

    #[test]
    fn s8_4_const_bounded_for_loop() {
        let mut eval = ConstEval::new();
        // sum = 0; for i in 0..5 { sum = sum + i }; sum
        // Result should be 0+1+2+3+4 = 10
        let expr = ConstExpr::Block(vec![
            ConstExpr::Let {
                name: "sum".to_string(),
                value: Box::new(ConstExpr::Literal(ConstValue::Int(0))),
            },
            ConstExpr::ForLoop {
                name: "i".to_string(),
                start: Box::new(ConstExpr::Literal(ConstValue::Int(0))),
                end: Box::new(ConstExpr::Literal(ConstValue::Int(5))),
                body: Box::new(ConstExpr::Let {
                    name: "sum".to_string(),
                    value: Box::new(ConstExpr::BinaryOp {
                        op: ConstBinaryOp::Add,
                        lhs: Box::new(ConstExpr::Var("sum".to_string())),
                        rhs: Box::new(ConstExpr::Var("i".to_string())),
                    }),
                }),
            },
            ConstExpr::Var("sum".to_string()),
        ]);
        let result = eval.eval(&expr).unwrap();
        assert_eq!(result, ConstValue::Int(10));
    }

    #[test]
    fn s8_5_const_struct_construction_and_field_access() {
        let mut eval = ConstEval::new();
        let mut fields = HashMap::new();
        fields.insert("x".to_string(), ConstValue::Int(10));
        fields.insert("y".to_string(), ConstValue::Int(20));
        eval.set_var(
            "point".to_string(),
            ConstValue::Struct {
                name: "Point".to_string(),
                fields,
            },
        );
        let expr = ConstExpr::Field {
            expr: Box::new(ConstExpr::Var("point".to_string())),
            name: "x".to_string(),
        };
        let result = eval.eval(&expr).unwrap();
        assert_eq!(result, ConstValue::Int(10));
    }

    #[test]
    fn s8_6_const_enum_construction() {
        let mut eval = ConstEval::new();
        eval.set_var(
            "opt".to_string(),
            ConstValue::Enum {
                variant: "Some".to_string(),
                data: Some(Box::new(ConstValue::Int(42))),
            },
        );
        let result = eval.eval(&ConstExpr::Var("opt".to_string())).unwrap();
        assert_eq!(
            result,
            ConstValue::Enum {
                variant: "Some".to_string(),
                data: Some(Box::new(ConstValue::Int(42)))
            }
        );
    }

    #[test]
    fn s8_7_const_fn_pointer() {
        let mut eval = ConstEval::new();
        eval.set_var(
            "callback".to_string(),
            ConstValue::FnPtr("my_handler".to_string()),
        );
        let result = eval.eval(&ConstExpr::Var("callback".to_string())).unwrap();
        assert_eq!(result, ConstValue::FnPtr("my_handler".to_string()));
    }

    #[test]
    fn s8_8_const_in_match_pattern() {
        let mut eval = ConstEval::new();
        // match Some(42) { Some(42) => "found", None => "empty", _ => "other" }
        let expr = ConstExpr::Match {
            scrutinee: Box::new(ConstExpr::Literal(ConstValue::Enum {
                variant: "Some".to_string(),
                data: Some(Box::new(ConstValue::Int(42))),
            })),
            arms: vec![
                (
                    Some(ConstValue::Enum {
                        variant: "Some".to_string(),
                        data: Some(Box::new(ConstValue::Int(42))),
                    }),
                    ConstExpr::Literal(ConstValue::Str("found".to_string())),
                ),
                (
                    Some(ConstValue::Enum {
                        variant: "None".to_string(),
                        data: None,
                    }),
                    ConstExpr::Literal(ConstValue::Str("empty".to_string())),
                ),
                (
                    None,
                    ConstExpr::Literal(ConstValue::Str("other".to_string())),
                ),
            ],
        };
        let result = eval.eval(&expr).unwrap();
        assert_eq!(result, ConstValue::Str("found".to_string()));
    }

    #[test]
    fn s8_9_cached_const_eval_metrics() {
        let mut cached = CachedConstEval::new();
        cached.register_fn(ConstFnDef {
            name: "triple".to_string(),
            params: vec!["x".to_string()],
            body: ConstExpr::BinaryOp {
                op: ConstBinaryOp::Mul,
                lhs: Box::new(ConstExpr::Var("x".to_string())),
                rhs: Box::new(ConstExpr::Literal(ConstValue::Int(3))),
            },
        });

        // First call: cache miss
        let expr = ConstExpr::FnCall {
            name: "triple".to_string(),
            args: vec![ConstExpr::Literal(ConstValue::Int(10))],
        };
        let r1 = cached.eval(&expr).unwrap();
        assert_eq!(r1, ConstValue::Int(30));

        // Second call with same args: cache hit
        let r2 = cached.eval(&expr).unwrap();
        assert_eq!(r2, ConstValue::Int(30));

        let metrics = cached.metrics();
        assert_eq!(metrics.expressions_evaluated, 2);
        assert_eq!(metrics.fn_calls, 2);
        assert_eq!(metrics.cache_hits, 1);
        assert_eq!(metrics.cache_misses, 1);
    }

    #[test]
    fn s8_10_timed_eval_returns_result_and_duration() {
        let mut eval = ConstEval::new();
        let expr = ConstExpr::BinaryOp {
            op: ConstBinaryOp::Add,
            lhs: Box::new(ConstExpr::Literal(ConstValue::Int(100))),
            rhs: Box::new(ConstExpr::Literal(ConstValue::Int(200))),
        };
        let (result, elapsed_us) = timed_eval(&mut eval, &expr).unwrap();
        assert_eq!(result, ConstValue::Int(300));
        // Elapsed time should be non-negative (it's u64, always true)
        // and reasonably small for a trivial operation
        assert!(elapsed_us < 1_000_000); // less than 1 second
    }
}
