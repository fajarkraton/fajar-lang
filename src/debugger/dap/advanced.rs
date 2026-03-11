//! Advanced debugging features: watch expressions, conditional breakpoints,
//! hit-count breakpoints, log points, and exception breakpoints.
//!
//! # Features
//!
//! - **Watch expressions**: Track expression values across debug steps
//! - **Conditional breakpoints**: Break only when a condition is true
//! - **Hit-count breakpoints**: Break after N hits (exact, gte, multiple)
//! - **Log points**: Print interpolated messages without stopping
//! - **Exception breakpoints**: Break on exceptions (all, uncaught, specific)
//! - **Variable modification**: Change variable values during debugging

use std::sync::atomic::{AtomicU32, Ordering};

use super::{DapDebugState, DebugError};

/// Global watch expression ID counter.
static NEXT_WATCH_ID: AtomicU32 = AtomicU32::new(1);

// ═══════════════════════════════════════════════════════════════════════
// Watch Expressions
// ═══════════════════════════════════════════════════════════════════════

/// A single watch expression being tracked by the debugger.
#[derive(Debug, Clone)]
pub struct WatchExpression {
    /// Unique watch ID.
    pub id: u32,
    /// The expression source text.
    pub expression: String,
    /// The last evaluated value (if any).
    pub last_value: Option<String>,
}

impl WatchExpression {
    /// Creates a new watch expression with a unique ID.
    fn new(expression: impl Into<String>) -> Self {
        Self {
            id: NEXT_WATCH_ID.fetch_add(1, Ordering::Relaxed),
            expression: expression.into(),
            last_value: None,
        }
    }
}

/// Manages a list of watch expressions.
///
/// Watch expressions are evaluated at each debug stop to show
/// the developer how values change during execution.
#[derive(Debug, Clone, Default)]
pub struct WatchList {
    /// Active watch expressions.
    expressions: Vec<WatchExpression>,
}

impl WatchList {
    /// Creates an empty watch list.
    pub fn new() -> Self {
        Self {
            expressions: Vec::new(),
        }
    }

    /// Adds a watch expression. Returns its unique ID.
    pub fn add_watch(&mut self, expr: impl Into<String>) -> u32 {
        let watch = WatchExpression::new(expr);
        let id = watch.id;
        self.expressions.push(watch);
        id
    }

    /// Removes a watch expression by ID.
    pub fn remove_watch(&mut self, id: u32) {
        self.expressions.retain(|w| w.id != id);
    }

    /// Returns the number of active watches.
    pub fn count(&self) -> usize {
        self.expressions.len()
    }

    /// Returns a watch expression by ID.
    pub fn get(&self, id: u32) -> Option<&WatchExpression> {
        self.expressions.iter().find(|w| w.id == id)
    }

    /// Evaluates all watch expressions using the provided evaluator.
    ///
    /// The evaluator receives the expression text and returns the result string.
    /// Returns a list of (watch_id, result_value) pairs.
    pub fn evaluate_all(&mut self, eval_fn: &mut dyn FnMut(&str) -> String) -> Vec<(u32, String)> {
        let mut results = Vec::new();
        for watch in &mut self.expressions {
            let value = eval_fn(&watch.expression);
            watch.last_value = Some(value.clone());
            results.push((watch.id, value));
        }
        results
    }

    /// Returns all watch expressions.
    pub fn all(&self) -> &[WatchExpression] {
        &self.expressions
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Conditional Breakpoints
// ═══════════════════════════════════════════════════════════════════════

/// A condition attached to a breakpoint.
#[derive(Debug, Clone, PartialEq)]
pub struct BreakpointCondition {
    /// The condition expression (Fajar Lang source).
    pub expression: String,
    /// Number of times the condition has been evaluated.
    pub evaluation_count: u32,
}

impl BreakpointCondition {
    /// Creates a new breakpoint condition.
    pub fn new(expression: impl Into<String>) -> Self {
        Self {
            expression: expression.into(),
            evaluation_count: 0,
        }
    }
}

/// Checks a conditional breakpoint by evaluating its condition expression.
///
/// The `eval_fn` receives the condition text and returns whether it is truthy.
/// Increments the evaluation count on each call.
pub fn check_condition(
    condition: &mut BreakpointCondition,
    eval_fn: &mut dyn FnMut(&str) -> bool,
) -> bool {
    condition.evaluation_count += 1;
    eval_fn(&condition.expression)
}

// ═══════════════════════════════════════════════════════════════════════
// Hit-Count Breakpoints
// ═══════════════════════════════════════════════════════════════════════

/// Condition for hit-count breakpoints.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HitCondition {
    /// Break when hit count equals this value exactly.
    Eq(u32),
    /// Break when hit count is greater than or equal to this value.
    Gte(u32),
    /// Break when hit count is a multiple of this value.
    Multiple(u32),
}

impl HitCondition {
    /// Checks whether the condition is satisfied for the given hit count.
    pub fn is_satisfied(&self, hit_count: u32) -> bool {
        match self {
            HitCondition::Eq(n) => hit_count == *n,
            HitCondition::Gte(n) => hit_count >= *n,
            HitCondition::Multiple(n) => *n > 0 && hit_count > 0 && hit_count.is_multiple_of(*n),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Log Points
// ═══════════════════════════════════════════════════════════════════════

/// A log point that prints a message without stopping execution.
///
/// The message template supports `{variable_name}` interpolation.
#[derive(Debug, Clone, PartialEq)]
pub struct LogPoint {
    /// Message template (e.g., "x = {x}, y = {y}").
    pub message_template: String,
}

impl LogPoint {
    /// Creates a new log point with the given message template.
    pub fn new(template: impl Into<String>) -> Self {
        Self {
            message_template: template.into(),
        }
    }

    /// Evaluates the log point, interpolating `{var}` placeholders.
    ///
    /// The `resolve_fn` receives a variable name and returns its string value.
    pub fn evaluate(&self, resolve_fn: &dyn Fn(&str) -> String) -> String {
        let mut result = String::new();
        let mut chars = self.message_template.chars().peekable();

        while let Some(ch) = chars.next() {
            if ch == '{' {
                let mut var_name = String::new();
                for inner in chars.by_ref() {
                    if inner == '}' {
                        break;
                    }
                    var_name.push(inner);
                }
                if !var_name.is_empty() {
                    result.push_str(&resolve_fn(&var_name));
                }
            } else {
                result.push(ch);
            }
        }
        result
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Exception Breakpoints
// ═══════════════════════════════════════════════════════════════════════

/// Exception breakpoint configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExceptionBreakpoint {
    /// Break on all exceptions.
    AllExceptions,
    /// Break only on uncaught exceptions.
    Uncaught,
    /// Break on a specific exception type.
    Specific(String),
}

impl ExceptionBreakpoint {
    /// Checks whether this breakpoint matches a given exception type.
    pub fn matches(&self, exception_type: &str) -> bool {
        match self {
            ExceptionBreakpoint::AllExceptions => true,
            ExceptionBreakpoint::Uncaught => true,
            ExceptionBreakpoint::Specific(t) => t == exception_type,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Expression Evaluation
// ═══════════════════════════════════════════════════════════════════════

/// Evaluates an expression in the context of a debug stack frame.
///
/// First checks local variables in the frame, then falls back to
/// the provided evaluation function for complex expressions.
pub fn evaluate_expression(
    expr: &str,
    frame_id: u32,
    state: &DapDebugState,
    eval_fn: &dyn Fn(&str) -> Result<String, String>,
) -> Result<String, DebugError> {
    // Try to find the frame
    let frame = state
        .find_frame(frame_id)
        .ok_or(DebugError::FrameNotFound { frame_id })?;

    // Check if expr is a simple variable name in locals
    if let Some(var) = frame.find_local(expr) {
        return Ok(var.value.clone());
    }

    // Fall back to the evaluator
    eval_fn(expr).map_err(|msg| DebugError::EvalError { message: msg })
}

// ═══════════════════════════════════════════════════════════════════════
// Variable Modification
// ═══════════════════════════════════════════════════════════════════════

/// Sets (modifies) a variable's value during debugging.
///
/// Finds the variable in the specified stack frame and updates its
/// display value. Returns an error if the frame or variable is not found.
pub fn set_variable(
    name: &str,
    value: &str,
    frame_id: u32,
    state: &mut DapDebugState,
) -> Result<(), DebugError> {
    let frame = state
        .find_frame_mut(frame_id)
        .ok_or(DebugError::FrameNotFound { frame_id })?;

    let var =
        frame
            .locals
            .iter_mut()
            .find(|v| v.name == name)
            .ok_or(DebugError::VariableNotFound {
                name: name.to_string(),
            })?;

    var.value = value.to_string();
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::debugger::dap::{DapSourceLocation, DapStackFrame, VarScope, VariableInfo};

    fn make_state_with_frame() -> (DapDebugState, u32) {
        let mut state = DapDebugState::new();
        let mut frame = DapStackFrame::new("test_fn", DapSourceLocation::new("test.fj", 1, 1));
        frame.add_local(VariableInfo::new("x", "i64", VarScope::Local, "42"));
        frame.add_local(VariableInfo::new("y", "f64", VarScope::Local, "3.14"));
        let fid = frame.id;
        state.push_frame(frame);
        (state, fid)
    }

    #[test]
    fn watch_list_add_and_remove() {
        let mut watches = WatchList::new();
        assert_eq!(watches.count(), 0);

        let id1 = watches.add_watch("x + 1");
        let id2 = watches.add_watch("y * 2");
        assert_eq!(watches.count(), 2);
        assert!(watches.get(id1).is_some());

        watches.remove_watch(id1);
        assert_eq!(watches.count(), 1);
        assert!(watches.get(id1).is_none());
        assert!(watches.get(id2).is_some());
    }

    #[test]
    fn watch_list_evaluate_all() {
        let mut watches = WatchList::new();
        let id1 = watches.add_watch("x");
        let id2 = watches.add_watch("y");

        let results = watches.evaluate_all(&mut |expr| match expr {
            "x" => "42".to_string(),
            "y" => "3.14".to_string(),
            _ => "<unknown>".to_string(),
        });

        assert_eq!(results.len(), 2);
        assert_eq!(results[0], (id1, "42".to_string()));
        assert_eq!(results[1], (id2, "3.14".to_string()));

        // Check last_value was updated
        assert_eq!(watches.get(id1).unwrap().last_value.as_deref(), Some("42"));
    }

    #[test]
    fn breakpoint_condition_check() {
        let mut cond = BreakpointCondition::new("x > 5");
        assert_eq!(cond.evaluation_count, 0);

        let result = check_condition(&mut cond, &mut |_| true);
        assert!(result);
        assert_eq!(cond.evaluation_count, 1);

        let result = check_condition(&mut cond, &mut |_| false);
        assert!(!result);
        assert_eq!(cond.evaluation_count, 2);
    }

    #[test]
    fn hit_condition_eq() {
        let cond = HitCondition::Eq(3);
        assert!(!cond.is_satisfied(1));
        assert!(!cond.is_satisfied(2));
        assert!(cond.is_satisfied(3));
        assert!(!cond.is_satisfied(4));
    }

    #[test]
    fn hit_condition_gte() {
        let cond = HitCondition::Gte(5);
        assert!(!cond.is_satisfied(4));
        assert!(cond.is_satisfied(5));
        assert!(cond.is_satisfied(100));
    }

    #[test]
    fn hit_condition_multiple() {
        let cond = HitCondition::Multiple(3);
        assert!(cond.is_satisfied(3));
        assert!(cond.is_satisfied(6));
        assert!(cond.is_satisfied(9));
        assert!(!cond.is_satisfied(4));
        assert!(!cond.is_satisfied(0)); // hit_count 0 should not trigger

        // Multiple(0) should never match (avoid div by zero)
        let cond_zero = HitCondition::Multiple(0);
        assert!(!cond_zero.is_satisfied(0));
        assert!(!cond_zero.is_satisfied(5));
    }

    #[test]
    fn log_point_interpolation() {
        let lp = LogPoint::new("x = {x}, sum = {sum}");
        let result = lp.evaluate(&|var| match var {
            "x" => "42".to_string(),
            "sum" => "100".to_string(),
            _ => "?".to_string(),
        });
        assert_eq!(result, "x = 42, sum = 100");
    }

    #[test]
    fn log_point_no_placeholders() {
        let lp = LogPoint::new("hello world");
        let result = lp.evaluate(&|_| "unused".to_string());
        assert_eq!(result, "hello world");
    }

    #[test]
    fn exception_breakpoint_matching() {
        assert!(ExceptionBreakpoint::AllExceptions.matches("RuntimeError"));
        assert!(ExceptionBreakpoint::Uncaught.matches("TypeError"));
        assert!(ExceptionBreakpoint::Specific("DivByZero".into()).matches("DivByZero"));
        assert!(!ExceptionBreakpoint::Specific("DivByZero".into()).matches("Overflow"));
    }

    #[test]
    fn evaluate_expression_finds_local() {
        let (state, fid) = make_state_with_frame();
        let result = evaluate_expression("x", fid, &state, &|_| Err("not found".to_string()));
        assert_eq!(result.unwrap(), "42");
    }

    #[test]
    fn evaluate_expression_fallback() {
        let (state, fid) = make_state_with_frame();
        let result = evaluate_expression("x + 1", fid, &state, &|_expr| Ok("43".to_string()));
        assert_eq!(result.unwrap(), "43");
    }

    #[test]
    fn evaluate_expression_frame_not_found() {
        let state = DapDebugState::new();
        let result = evaluate_expression("x", 999, &state, &|_| Ok("nope".to_string()));
        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(err_msg.contains("frame"));
    }

    #[test]
    fn set_variable_success() {
        let (mut state, fid) = make_state_with_frame();
        set_variable("x", "99", fid, &mut state).unwrap();

        let frame = state.find_frame(fid).unwrap();
        let var = frame.find_local("x").unwrap();
        assert_eq!(var.value, "99");
    }

    #[test]
    fn set_variable_not_found() {
        let (mut state, fid) = make_state_with_frame();
        let result = set_variable("nonexistent", "0", fid, &mut state);
        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(err_msg.contains("nonexistent"));
    }
}
