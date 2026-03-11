//! Handler composition — effect handler syntax, resume continuation,
//! nested handlers, composition, tunneling, State/Exception effects.

use std::fmt;

use super::inference::EffectLabel;

// ═══════════════════════════════════════════════════════════════════════
// S19.1: Effect Handler Syntax
// ═══════════════════════════════════════════════════════════════════════

/// An effect operation that can be performed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectOperation {
    /// Effect this operation belongs to.
    pub effect: EffectLabel,
    /// Operation name.
    pub name: String,
    /// Parameter types.
    pub param_types: Vec<String>,
    /// Return type.
    pub return_type: String,
}

impl fmt::Display for EffectOperation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}::{}", self.effect, self.name)
    }
}

/// An effect handler clause.
#[derive(Debug, Clone)]
pub struct HandlerClause {
    /// Operation being handled.
    pub operation: EffectOperation,
    /// Parameter bindings.
    pub param_names: Vec<String>,
    /// Handler body description.
    pub body: HandlerBody,
}

/// Handler body — what happens when the operation is intercepted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HandlerBody {
    /// Resume with a value — `resume(value)`.
    Resume(String),
    /// Abort with a value — short-circuit the computation.
    Abort(String),
    /// Transform and resume — modify the value before resuming.
    TransformResume {
        transform: String,
        resume_value: String,
    },
}

/// A complete effect handler block.
#[derive(Debug, Clone)]
pub struct EffectHandler {
    /// Handler name (optional).
    pub name: Option<String>,
    /// Handled effects.
    pub clauses: Vec<HandlerClause>,
    /// Return clause (for final value transformation).
    pub return_clause: Option<String>,
}

impl EffectHandler {
    /// Creates a new handler with the given clauses.
    pub fn new(clauses: Vec<HandlerClause>) -> Self {
        Self {
            name: None,
            clauses,
            return_clause: None,
        }
    }

    /// Sets the handler name.
    pub fn with_name(mut self, name: &str) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Sets the return clause.
    pub fn with_return(mut self, clause: &str) -> Self {
        self.return_clause = Some(clause.into());
        self
    }

    /// Returns the operations handled by this handler.
    pub fn handled_operations(&self) -> Vec<&EffectOperation> {
        self.clauses.iter().map(|c| &c.operation).collect()
    }

    /// Checks if this handler handles a specific operation.
    pub fn handles(&self, effect: &str, op: &str) -> bool {
        self.clauses
            .iter()
            .any(|c| c.operation.effect.0 == effect && c.operation.name == op)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S19.2: Resume Continuation
// ═══════════════════════════════════════════════════════════════════════

/// Resume status after handling an effect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResumeAction {
    /// Continue with the given value.
    Continue(String),
    /// Abort the computation.
    Abort(String),
}

// ═══════════════════════════════════════════════════════════════════════
// S19.3 / S19.4: Handler Semantics & Nesting
// ═══════════════════════════════════════════════════════════════════════

/// A handler stack for nested handler resolution.
#[derive(Debug, Clone, Default)]
pub struct HandlerStack {
    /// Handlers from outermost to innermost.
    handlers: Vec<EffectHandler>,
}

impl HandlerStack {
    /// Creates an empty handler stack.
    pub fn new() -> Self {
        Self::default()
    }

    /// Pushes a handler onto the stack (innermost).
    pub fn push(&mut self, handler: EffectHandler) {
        self.handlers.push(handler);
    }

    /// Pops the innermost handler.
    pub fn pop(&mut self) -> Option<EffectHandler> {
        self.handlers.pop()
    }

    /// Finds the innermost handler for a given operation.
    pub fn find_handler(&self, effect: &str, op: &str) -> Option<&EffectHandler> {
        self.handlers.iter().rev().find(|h| h.handles(effect, op))
    }

    /// Returns the current nesting depth.
    pub fn depth(&self) -> usize {
        self.handlers.len()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S19.5: Handler Composition
// ═══════════════════════════════════════════════════════════════════════

/// Composes two handlers — effects flow through both.
pub fn compose_handlers(outer: &EffectHandler, inner: &EffectHandler) -> EffectHandler {
    let mut clauses = inner.clauses.clone();
    for clause in &outer.clauses {
        if !clauses.iter().any(|c| c.operation == clause.operation) {
            clauses.push(clause.clone());
        }
    }
    EffectHandler {
        name: Some(format!(
            "{}>>{}",
            outer.name.as_deref().unwrap_or("anon"),
            inner.name.as_deref().unwrap_or("anon")
        )),
        clauses,
        return_clause: inner
            .return_clause
            .clone()
            .or_else(|| outer.return_clause.clone()),
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S19.6: Effect Tunneling
// ═══════════════════════════════════════════════════════════════════════

/// Checks which effects tunnel through a handler (unhandled).
pub fn tunneled_effects(handler: &EffectHandler, all_effects: &[&str]) -> Vec<String> {
    all_effects
        .iter()
        .filter(|e| !handler.clauses.iter().any(|c| c.operation.effect.0 == **e))
        .map(|e| e.to_string())
        .collect()
}

// ═══════════════════════════════════════════════════════════════════════
// S19.7: State Effect
// ═══════════════════════════════════════════════════════════════════════

/// State effect operations.
#[derive(Debug, Clone)]
pub struct StateEffect {
    /// State type name.
    pub state_type: String,
    /// Current state value description.
    pub initial_value: String,
}

impl StateEffect {
    /// Creates a State effect.
    pub fn new(state_type: &str, initial: &str) -> Self {
        Self {
            state_type: state_type.into(),
            initial_value: initial.into(),
        }
    }

    /// Returns the get operation.
    pub fn get_op(&self) -> EffectOperation {
        EffectOperation {
            effect: EffectLabel::new("State"),
            name: "get".into(),
            param_types: vec![],
            return_type: self.state_type.clone(),
        }
    }

    /// Returns the set operation.
    pub fn set_op(&self) -> EffectOperation {
        EffectOperation {
            effect: EffectLabel::new("State"),
            name: "set".into(),
            param_types: vec![self.state_type.clone()],
            return_type: "void".into(),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S19.8: Exception Effect
// ═══════════════════════════════════════════════════════════════════════

/// Exception effect.
#[derive(Debug, Clone)]
pub struct ExceptionEffect {
    /// Error type name.
    pub error_type: String,
}

impl ExceptionEffect {
    /// Creates an Exception effect.
    pub fn new(error_type: &str) -> Self {
        Self {
            error_type: error_type.into(),
        }
    }

    /// Returns the raise operation.
    pub fn raise_op(&self) -> EffectOperation {
        EffectOperation {
            effect: EffectLabel::new("Exception"),
            name: "raise".into(),
            param_types: vec![self.error_type.clone()],
            return_type: "never".into(),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S19.9: Handler Return Type
// ═══════════════════════════════════════════════════════════════════════

/// Handler type signature.
#[derive(Debug, Clone)]
pub struct HandlerType {
    /// Input computation return type.
    pub input_type: String,
    /// Handler output type (may differ).
    pub output_type: String,
    /// Effects handled (removed).
    pub handled_effects: Vec<EffectLabel>,
    /// Effects remaining (tunneled).
    pub remaining_effects: Vec<EffectLabel>,
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn io_op(name: &str) -> EffectOperation {
        EffectOperation {
            effect: EffectLabel::new("IO"),
            name: name.into(),
            param_types: vec!["str".into()],
            return_type: "void".into(),
        }
    }

    // S19.1 — Handler Syntax
    #[test]
    fn s19_1_effect_operation_display() {
        let op = io_op("print");
        assert_eq!(op.to_string(), "IO::print");
    }

    #[test]
    fn s19_1_handler_creation() {
        let handler = EffectHandler::new(vec![HandlerClause {
            operation: io_op("print"),
            param_names: vec!["msg".into()],
            body: HandlerBody::Resume("()".into()),
        }])
        .with_name("silent");
        assert_eq!(handler.name.as_deref(), Some("silent"));
        assert_eq!(handler.handled_operations().len(), 1);
    }

    // S19.2 — Resume Continuation
    #[test]
    fn s19_2_resume_actions() {
        assert_ne!(
            ResumeAction::Continue("42".into()),
            ResumeAction::Abort("error".into())
        );
    }

    // S19.3 — Handler Semantics
    #[test]
    fn s19_3_handler_handles_check() {
        let handler = EffectHandler::new(vec![HandlerClause {
            operation: io_op("print"),
            param_names: vec!["msg".into()],
            body: HandlerBody::Resume("()".into()),
        }]);
        assert!(handler.handles("IO", "print"));
        assert!(!handler.handles("IO", "read"));
        assert!(!handler.handles("Alloc", "new"));
    }

    // S19.4 — Nested Handlers
    #[test]
    fn s19_4_handler_stack() {
        let mut stack = HandlerStack::new();
        stack.push(EffectHandler::new(vec![HandlerClause {
            operation: io_op("print"),
            param_names: vec!["msg".into()],
            body: HandlerBody::Resume("()".into()),
        }]));
        assert_eq!(stack.depth(), 1);
        assert!(stack.find_handler("IO", "print").is_some());
        assert!(stack.find_handler("Alloc", "new").is_none());
        stack.pop();
        assert_eq!(stack.depth(), 0);
    }

    // S19.5 — Handler Composition
    #[test]
    fn s19_5_compose_handlers() {
        let outer = EffectHandler::new(vec![HandlerClause {
            operation: io_op("print"),
            param_names: vec!["msg".into()],
            body: HandlerBody::Resume("()".into()),
        }])
        .with_name("outer");
        let inner = EffectHandler::new(vec![HandlerClause {
            operation: EffectOperation {
                effect: EffectLabel::new("Alloc"),
                name: "alloc".into(),
                param_types: vec!["usize".into()],
                return_type: "ptr".into(),
            },
            param_names: vec!["size".into()],
            body: HandlerBody::Resume("null".into()),
        }])
        .with_name("inner");
        let composed = compose_handlers(&outer, &inner);
        assert_eq!(composed.clauses.len(), 2);
        assert!(composed.name.unwrap().contains(">>"));
    }

    // S19.6 — Tunneling
    #[test]
    fn s19_6_tunneled_effects() {
        let handler = EffectHandler::new(vec![HandlerClause {
            operation: io_op("print"),
            param_names: vec![],
            body: HandlerBody::Resume("()".into()),
        }]);
        let tunneled = tunneled_effects(&handler, &["IO", "Alloc", "Panic"]);
        assert_eq!(tunneled.len(), 2);
        assert!(tunneled.contains(&"Alloc".to_string()));
        assert!(tunneled.contains(&"Panic".to_string()));
    }

    // S19.7 — State Effect
    #[test]
    fn s19_7_state_effect() {
        let state = StateEffect::new("i32", "0");
        let get = state.get_op();
        assert_eq!(get.name, "get");
        assert_eq!(get.return_type, "i32");
        let set = state.set_op();
        assert_eq!(set.name, "set");
        assert_eq!(set.param_types[0], "i32");
    }

    // S19.8 — Exception Effect
    #[test]
    fn s19_8_exception_effect() {
        let exc = ExceptionEffect::new("String");
        let raise = exc.raise_op();
        assert_eq!(raise.name, "raise");
        assert_eq!(raise.return_type, "never");
    }

    // S19.9 — Handler Return Type
    #[test]
    fn s19_9_handler_type() {
        let ht = HandlerType {
            input_type: "i32".into(),
            output_type: "Result<i32, String>".into(),
            handled_effects: vec![EffectLabel::new("Exception")],
            remaining_effects: vec![EffectLabel::new("IO")],
        };
        assert_ne!(ht.input_type, ht.output_type);
        assert_eq!(ht.handled_effects.len(), 1);
    }

    // S19.10 — Additional
    #[test]
    fn s19_10_handler_body_variants() {
        assert_eq!(
            HandlerBody::Resume("42".into()),
            HandlerBody::Resume("42".into())
        );
        assert_ne!(
            HandlerBody::Resume("42".into()),
            HandlerBody::Abort("err".into())
        );
    }

    #[test]
    fn s19_10_handler_with_return_clause() {
        let handler = EffectHandler::new(vec![]).with_return("x * 2");
        assert_eq!(handler.return_clause.as_deref(), Some("x * 2"));
    }
}
